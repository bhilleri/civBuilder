//! Étiquettes à l'écran ancrées sur la carte : bannières des villes (2D et 3D), noms des
//! fleuves et numéros de tour le long des trajets (3D ; la 2D a ses propres pastilles).
//!
//! Ce sont des nœuds d'interface placés chaque image à la projection de leur case par la
//! caméra active : ils restent lisibles quel que soit le zoom ou l'inclinaison.
//! Les barres de PV des unités en 3D suivent le même principe (la 2D dessine les siennes).

use crate::pointer::UiPointer;
use crate::feedback::{HeldHp, Motion};
use crate::game::{City, GameMap, ShownFog, HexPos, Unit};
use crate::hostile::Hostile;
use crate::rules::MAX_HP;
use crate::hex::Hex;
use crate::input::{HoverPath, OrderPath};
use crate::highlight::{Feature, HighlightMode, HoveredLabel};
use crate::search::{MapTarget, TargetButton};
use crate::view3d::{CITY_HEIGHT, ViewMode, shown_height, to_3d};
use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::ui::UiSystems;

const BANNER_BG: Color = Color::srgba(0.05, 0.06, 0.09, 0.85);
const PLAYER_COLOR: Color = Color::srgb(0.20, 0.45, 0.95);
const ORDER_LABEL: Color = Color::srgb(0.16, 0.42, 0.78);
const HOVER_LABEL: Color = Color::srgba(0.05, 0.05, 0.08, 0.9);

/// Point d'ancrage d'une étiquette.
#[derive(Component)]
struct Anchor {
    hex: Hex,
    /// Décalage vertical en vue 2D (unités du monde, vers le haut de l'écran).
    up_2d: f32,
    /// Hauteur au-dessus du relief en vue 3D.
    up_3d: f32,
    /// Étiquette propre à la vue 3D (masquée en 2D).
    only_3d: bool,
    /// Masquée quel que soit l'écran (unité en pleine santé, hors de vue…).
    hidden: bool,
    /// Barre de PV : posée sous le jeton, à une taille qui suit le zoom.
    under_token: bool,
    /// Pastille d'une unité : posée sur le bord haut-droit du jeton, quel que soit le zoom.
    token_corner: bool,
    /// Montrée seulement si sa case est explorée et assez grande à l'écran (rayon en pixels).
    explored_only: bool,
    min_px: f32,
    /// Point du monde 2D suivi à la place du centre de la case (unité en déplacement mis en scène).
    glide: Option<Vec2>,
}

impl Anchor {
    fn new(hex: Hex, up_2d: f32, up_3d: f32, only_3d: bool) -> Self {
        Anchor {
            hex,
            up_2d,
            up_3d,
            only_3d,
            hidden: false,
            under_token: false,
            token_corner: false,
            explored_only: false,
            min_px: 0.0,
            glide: None,
        }
    }
}

/// Pastille d'une unité immobilisée par la dette de PM (montagne) : tours sans bouger qui restent.
#[derive(Component)]
struct StuckBadge(Entity);
#[derive(Component)]
struct StuckText;
const STUCK_BG: Color = Color::srgb(0.88, 0.47, 0.10);
const STUCK_LIFT: f32 = 12.0;

/// Nom d'un village tribal ou d'un camp barbare, sous son icône.
#[derive(Component)]
struct SiteLabel(Entity);
const CAMP_LABEL_BG: Color = Color::srgba(0.35, 0.06, 0.05, 0.85);
const VILLAGE_LABEL_BG: Color = Color::srgba(0.30, 0.22, 0.10, 0.85);
/// Rayon d'une case à l'écran (pixels) à partir duquel les noms des sites apparaissent.
const SITE_LABEL_PX: f32 = 16.0;

/// Barre de PV d'une unité en vue 3D.
#[derive(Component)]
struct HpBar3d(Entity);
#[derive(Component)]
struct HpFill3d;

/// Rayon du jeton 3D d'une unité (unités du monde) et hauteur de son centre.
const TOKEN_RADIUS: f32 = 16.0;
const TOKEN_LIFT: f32 = 3.0;
const HP_GOOD: Color = Color::srgb(0.30, 0.85, 0.35);
const HP_MID: Color = Color::srgb(0.95, 0.75, 0.20);
const HP_LOW: Color = Color::srgb(0.95, 0.25, 0.20);

/// Bannière d'une ville : population + nom.
#[derive(Component)]
struct CityBanner(Entity);
#[derive(Component)]
struct BannerName;
#[derive(Component)]
struct BannerPop;

/// Étiquettes affichées (bouton « Étiquettes », touche N).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelFilter {
    pub rivers: bool,
    pub lakes: bool,
    pub seas: bool,
    pub lands: bool,
    pub reliefs: bool,
    pub regions: bool,
    pub cities: bool,
    pub sites: bool,
}

impl Default for LabelFilter {
    fn default() -> Self {
        Self { rivers: true, lakes: true, seas: true, lands: true, reliefs: true, regions: true, cities: true, sites: true }
    }
}

/// Nom d'un cours d'eau. Posé au plus près du milieu de son cours sur un emplacement exploré,
/// sans chevaucher une étiquette plus importante ; les cours d'eau secondaires ne se montrent
/// qu'assez près (les longs affluents de fleuve, puis tout le reste en zoom fin).
#[derive(Component)]
struct RiverName {
    /// Ordre de placement : par importance, les lacs juste après les cours d'eau de même
    /// importance, puis dans l'ordre des noms (débit ou taille décroissants).
    rank: usize,
    /// 0 cours principal d'un fleuve ou très grand lac, 1 long affluent ou grand lac, 2 le reste.
    tier: u8,
    /// Emplacements possibles : une case (a = b) ou une arête.
    spots: Vec<(Hex, Hex)>,
    /// Emplacements déjà rangés par préférence (lac : du centre vers les rives) ; sinon on part
    /// du milieu du cours.
    ordered: bool,
    /// Cours d'eau ou lac nommé.
    feature: Feature,
}
const LAKE_NAME_BG: Color = Color::srgba(0.02, 0.06, 0.14, 0.78);
const RIVER_NAME_BG: Color = Color::srgba(0.04, 0.10, 0.20, 0.72);
const STREAM_NAME_BG: Color = Color::srgba(0.04, 0.10, 0.20, 0.55);
/// Par importance : taille du texte, couleur, rayon d'une case à l'écran (pixels) en dessous
/// duquel le nom n'est pas montré.
const NAME_STYLES: [(f32, Color, f32); 3] = [
    (15.0, Color::srgb(0.80, 0.90, 1.0), 0.0),
    (12.0, Color::srgb(0.66, 0.78, 0.90), 14.0),
    (11.0, Color::srgb(0.62, 0.74, 0.86), 40.0),
];
/// Écart minimal entre deux étiquettes (pixels).
const LABEL_MARGIN: f32 = 4.0;

/// Pastille « tour d'arrivée » d'un trajet, en vue 3D.
#[derive(Component)]
struct TurnLabel3d;

pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_city_banners,
                update_city_banners,
                spawn_river_names,
                update_turn_labels_3d,
                spawn_hp_bars_3d,
                update_hp_bars_3d,
                spawn_stuck_badges,
                update_stuck_badges,
                spawn_site_labels,
                despawn_site_labels,
                spawn_sea_names,
                spawn_bank_icons,
                attach_fish_images,
                spawn_oasis_names,
                spawn_deposit_icons,
                spawn_city_status,
                update_city_status,
            ),
        )
        .init_resource::<LabelFilter>()
        .add_systems(Update, update_hovered_label)
        .add_systems(
            PostUpdate,
            (place_labels, place_river_names, place_sea_names, place_bank_icons, place_oasis_names, place_deposit_icons).chain().after(CameraUpdateSystems).before(UiSystems::Prepare),
        );
    }
}

fn spawn_city_banners(mut commands: Commands, cities: Query<(Entity, &City, &HexPos), Added<City>>) {
    for (entity, city, pos) in &cities {
        commands
            .spawn((
                CityBanner(entity),
                // Survol : le territoire de la ville ressort ; clic : la caméra y va
                (TargetButton::new(MapTarget::City(entity)), UiPointer::default()),
                // Au-dessus du jeton d'une unité en garnison
                Anchor::new(pos.0, 24.0, CITY_HEIGHT + 14.0, false),
                Node {
                    position_type: PositionType::Absolute,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::new(Val::Px(3.0), Val::Px(8.0), Val::Px(2.0), Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(11.0)),
                    ..default()
                },
                // Ancre au milieu du bord inférieur
                UiTransform::from_translation(Val2::percent(-50.0, -100.0)),
                BackgroundColor(BANNER_BG),
                BorderColor::all(PLAYER_COLOR),
                GlobalZIndex(-1),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        width: Val::Px(18.0),
                        height: Val::Px(18.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(9.0)),
                        ..default()
                    },
                    BackgroundColor(PLAYER_COLOR),
                    children![(
                        BannerPop,
                        Text::new(city.state.palier().to_string()),
                        TextFont::from_font_size(12.0),
                        TextColor(Color::WHITE),
                    )],
                ));
                p.spawn((
                    BannerName,
                    Text::new(city.state.name.clone()),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::WHITE),
                ));
            });
    }
}

/// Famille d'un nom de zone.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AreaFamily {
    Sea,
    Land,
    Relief,
    Region,
}

/// Nom d'une étendue marine (océan, mer, golfe, baie, détroit), d'une terre (continent, île,
/// archipel, péninsule, isthme), d'un relief (chaîne, massif, col, plateau, collines) ou d'une
/// région naturelle (forêt, désert…).
#[derive(Component)]
struct SeaName {
    /// Zone : indice dans `Seas::zones`, puis, au-delà, dans `Lands::zones` (décalé du nombre
    /// d'étendues marines), puis dans `Reliefs::zones`, puis dans `Regions::zones`.
    zone: usize,
    /// Un grand océan a son nom répété en plusieurs endroits (copie 0, 1…).
    copy: usize,
    /// Rayon d'une case à l'écran (pixels) entre lesquels le nom se montre (fondu aux bords).
    min_px: f32,
    max_px: f32,
    /// Priorité de placement (0 : océans et continents d'abord).
    rank: u8,
    /// Famille du nom (filtre, style) et sa couleur.
    family: AreaFamily,
    tint: (f32, f32, f32),
    /// Part de la largeur du nom que la zone doit couvrir à l'écran pour le porter.
    fit: f32,
    /// Importance dans son rang (nombre de cases) : les plus grandes zones d'abord.
    weight: usize,
    /// Partie d'une grande zone (carte géographique : montrée au niveau du dessous seulement).
    part: bool,
    /// Nom courbé le long de l'axe de la zone (chaîne, massif, golfe allongés).
    curve: Option<Curve>,
}

/// Nom courbé : une lettre par nœud, posées et tournées le long de l'axe de la zone.
struct Curve {
    letters: Vec<Entity>,
    /// Cases de l'axe (d'un seul tenant, raccord compris) et points lissés (monde 2D).
    hexes: Vec<Hex>,
    points: Vec<Vec2>,
    /// Espace ajouté entre deux lettres (pixels).
    gap: f32,
}

/// Lettre d'un nom courbé.
#[derive(Component)]
struct CurvedLetter;

/// Quotas de noms de zones à l'écran par rang (0 : océans et continents… 4 : îlots), pour un
/// écran de `QUOTA_SCREEN` pixels logiques : une densité régulière, les plus grandes zones d'abord.
const AREA_QUOTA: [f32; 5] = [8.0, 10.0, 12.0, 16.0, 20.0];
const QUOTA_SCREEN: f32 = 1600.0 * 900.0;
/// Écart minimal entre deux noms de zones (pixels, en plus de `LABEL_MARGIN`).
const AREA_GAP: f32 = 6.0;
/// Un nom courbé : l'axe doit être plus long que le nom d'au moins cette part ; d'une lettre à la
/// suivante, l'axe ne tourne pas de plus de tant (radians), sinon le nom reste droit.
const CURVE_SLACK: f32 = 1.1;
const CURVE_MAX_TURN: f32 = 0.5;
/// Un nom courbé ne penche pas de plus de tant (radians, 60°) : plus raide, il se lit mal.
const CURVE_MAX_TILT: f32 = 1.05;
/// Carte géographique : les noms de la famille montrée, en blanc, plus nombreux.
const GEO_NAME_COLOR: (f32, f32, f32) = (0.99, 0.98, 0.95);
const GEO_QUOTA: f32 = 1.5;
/// Hauteur d'écran de référence (pixels logiques) du niveau de zoom : une case se mesure comme sur
/// un écran de cette hauteur, pour que le niveau ne dépende pas de la taille de la fenêtre (sur un
/// grand écran, à distance égale, les cases paraissent plus grandes et les océans ne se
/// montraient plus, même dézoomé au maximum).
const LEVEL_SCREEN_HEIGHT: f32 = 900.0;

/// Niveau de zoom de la vue : taille à l'écran d'une case au point visé par la caméra (`focus`),
/// ramenée à un écran de `LEVEL_SCREEN_HEIGHT` pixels de haut. Le même pour tous les noms et la
/// carte géographique.
pub(crate) fn view_level(camera: &Camera, transform: &Transform, focus: Vec3) -> Option<f32> {
    let global = GlobalTransform::from(*transform);
    let a = camera.world_to_viewport(&global, focus).ok()?;
    let b = camera.world_to_viewport(&global, focus + transform.right() * crate::hex::HEX_SIZE).ok()?;
    let height = camera.logical_viewport_size().map_or(LEVEL_SCREEN_HEIGHT, |v| v.y.max(1.0));
    Some((a.distance(b) * LEVEL_SCREEN_HEIGHT / height).max(0.01))
}

/// Style d'un nom d'étendue marine : capitales espacées pour les océans et les mers, italique
/// bleu pour toutes ; taille et zoom selon l'importance.
fn sea_style(kind: crate::seas::SeaKind) -> (f32, f32, bool, f32, f32, u8) {
    use crate::seas::SeaKind;
    // (taille, espacement des lettres en rem, capitales, zoom min, zoom max, rang)
    match kind {
        SeaKind::Ocean => (22.0, 0.35, true, 0.0, 12.0, 0),
        SeaKind::Sea => (17.0, 0.22, true, 4.0, 22.0, 1),
        SeaKind::Gulf => (14.0, 0.06, false, 9.0, 55.0, 2),
        SeaKind::Bay => (12.5, 0.03, false, 15.0, 120.0, 3),
        SeaKind::Strait => (12.5, 0.03, false, 15.0, 120.0, 3),
    }
}
const SEA_NAME_COLOR: (f32, f32, f32) = (0.66, 0.82, 0.97);

/// Style d'un nom de terre : capitales espacées pour les continents, les archipels et les grandes
/// îles, droit (l'italique est réservé à l'eau) ; taille et zoom selon l'importance. Rend aussi la
/// part de la largeur du nom que la zone doit couvrir à l'écran (0 : une île porte son nom même
/// s'il déborde sur la mer).
fn land_style(kind: crate::lands::LandKind, cells: usize) -> ((f32, f32, bool, f32, f32, u8), f32) {
    use crate::lands::LandKind;
    match kind {
        LandKind::Continent => ((24.0, 0.45, true, 0.0, 10.0, 0), 0.7),
        LandKind::Archipelago => ((15.0, 0.2, true, 4.0, 22.0, 1), 0.0),
        LandKind::Island if cells >= 60 => ((15.0, 0.15, true, 4.0, 30.0, 1), 0.0),
        LandKind::Island if cells >= 20 => ((13.5, 0.04, false, 9.0, 80.0, 2), 0.0),
        LandKind::Island => ((12.5, 0.03, false, 15.0, 120.0, 3), 0.0),
        LandKind::Islet => ((11.5, 0.02, false, 30.0, 160.0, 4), 0.0),
        LandKind::Peninsula => ((14.0, 0.06, false, 9.0, 55.0, 2), 0.5),
        LandKind::Isthmus => ((12.5, 0.03, false, 15.0, 120.0, 3), 0.0),
    }
}
const LAND_NAME_COLOR: (f32, f32, f32) = (0.97, 0.92, 0.80);

/// Style d'un nom de relief (comme `land_style`) : capitales espacées pour les chaînes ; une chaîne
/// découpée en massifs s'efface quand ses massifs apparaissent.
fn relief_style(kind: crate::mountains::ReliefKind, split: bool) -> ((f32, f32, bool, f32, f32, u8), f32) {
    use crate::mountains::ReliefKind;
    match kind {
        ReliefKind::Range => ((16.0, 0.3, true, 4.0, if split { 16.0 } else { 55.0 }, 1), 0.0),
        ReliefKind::Massif => ((14.0, 0.06, false, 14.0, 70.0, 2), 0.0),
        ReliefKind::Plateau => ((14.0, 0.12, false, 7.0, 55.0, 2), 0.6),
        ReliefKind::Hills => ((12.5, 0.04, false, 15.0, 120.0, 3), 0.4),
        ReliefKind::Pass => ((12.0, 0.02, false, 25.0, 160.0, 3), 0.0),
    }
}
/// Relief : brun clair (sépia), comme les cartes en relief.
const RELIEF_NAME_COLOR: (f32, f32, f32) = (0.93, 0.80, 0.62);

/// Style d'un nom de région naturelle (comme `land_style`) : capitales espacées pour une grande
/// région, qui s'efface quand ses sous-régions apparaissent ; les petites régions (marais) de près.
fn region_style(cells: usize, sub: bool, split: bool) -> ((f32, f32, bool, f32, f32, u8), f32) {
    match (sub, split) {
        (true, _) => ((13.5, 0.05, false, 13.0, 55.0, 2), 0.6),
        (false, true) => ((15.0, 0.25, true, 4.0, 14.0, 1), 0.6),
        (false, false) if cells >= 60 => ((14.0, 0.12, true, 6.0, 45.0, 2), 0.6),
        (false, false) => ((12.5, 0.03, false, 14.0, 110.0, 3), 0.3),
    }
}

/// Couleur d'un nom de région naturelle : vert pour la végétation, sable pour les terres sèches,
/// vert d'eau pour les marais, blanc bleuté pour la toundra.
fn region_color(biome: crate::regions::Biome) -> (f32, f32, f32) {
    use crate::regions::Biome;
    match biome {
        Biome::Forest | Biome::Jungle | Biome::Taiga | Biome::Scrub => (0.74, 0.90, 0.64),
        Biome::Desert | Biome::Steppe | Biome::Savanna => (0.98, 0.86, 0.58),
        Biome::Marsh => (0.66, 0.88, 0.80),
        Biome::Tundra => (0.86, 0.92, 0.98),
    }
}
/// Un océan a son nom répété une fois par tranche de tant de cases (4 fois au plus), à au moins
/// `SEA_NAME_SPACING` cases d'écart.
const SEA_NAME_REPEAT_CELLS: usize = 2_500;
const SEA_NAME_SPACING: i32 = 35;

/// Une étiquette par étendue marine (refaites si la carte change de monde).
fn spawn_sea_names(mut commands: Commands, map: Res<GameMap>, old: Query<Entity, Or<(With<SeaName>, With<CurvedLetter>)>>) {
    if !map.is_added() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let seas = map.0.seas();
    let sea_names = seas.zones.iter().enumerate().map(|(z, zone)| {
        let copies = if zone.kind == crate::seas::SeaKind::Ocean { (zone.cells.len() / SEA_NAME_REPEAT_CELLS).clamp(1, 4) } else { 1 };
        (z, &zone.name, sea_style(zone.kind), 0.7, copies, MapTarget::Sea(z))
    });
    let land_names = map.0.lands().zones.iter().enumerate().map(|(z, zone)| {
        let (style, fit) = land_style(zone.kind, zone.cells.len());
        (seas.zones.len() + z, &zone.name, style, fit, 1, MapTarget::Land(z))
    });
    let reliefs = map.0.reliefs();
    let first_relief = seas.zones.len() + map.0.lands().zones.len();
    let relief_names = reliefs.zones.iter().enumerate().map(|(z, zone)| {
        let split = reliefs.zones.iter().any(|m| m.parent == Some(z) && m.kind == crate::mountains::ReliefKind::Massif);
        let (style, fit) = relief_style(zone.kind, split);
        (first_relief + z, &zone.name, style, fit, 1, MapTarget::Relief(z))
    });
    let regions = map.0.regions();
    let first_region = first_relief + reliefs.zones.len();
    let region_names = regions.zones.iter().enumerate().map(|(z, zone)| {
        let split = regions.zones.iter().any(|m| m.parent == Some(z));
        let (style, fit) = region_style(zone.cells.len(), zone.parent.is_some(), split);
        (first_region + z, &zone.name, style, fit, 1, MapTarget::Region(z))
    });
    // Zone de chaque nom (cases), pour son importance et son axe
    let cells_of = |target: MapTarget| -> &[usize] {
        match target {
            MapTarget::Sea(z) => &seas.zones[z].cells,
            MapTarget::Land(z) => &map.0.lands().zones[z].cells,
            MapTarget::Relief(z) => &reliefs.zones[z].cells,
            MapTarget::Region(z) => &regions.zones[z].cells,
            _ => &[],
        }
    };
    // Noms courbés le long de leur zone : chaînes, massifs, golfes
    let curved = |target: MapTarget| match target {
        MapTarget::Sea(z) => seas.zones[z].kind == crate::seas::SeaKind::Gulf,
        MapTarget::Relief(z) => matches!(reliefs.zones[z].kind, crate::mountains::ReliefKind::Range | crate::mountains::ReliefKind::Massif),
        _ => false,
    };
    for (area, name, (size, spacing, caps, min_px, max_px, rank), fit, copies, target) in sea_names.chain(land_names).chain(relief_names).chain(region_names) {
        let (family, tint) = match target {
            MapTarget::Land(_) => (AreaFamily::Land, LAND_NAME_COLOR),
            MapTarget::Relief(_) => (AreaFamily::Relief, RELIEF_NAME_COLOR),
            MapTarget::Region(z) => (AreaFamily::Region, region_color(regions.zones[z].biome)),
            _ => (AreaFamily::Sea, SEA_NAME_COLOR),
        };
        let text = if caps { name.to_uppercase() } else { name.clone() };
        // Eau en italique, terre droite
        let (font, shadow) = match family != AreaFamily::Sea {
            false => (TextFont { font_size: size.into(), style: bevy::text::FontStyle::Italic, ..TextFont::from_font_size(size) }, Color::srgba(0.0, 0.03, 0.08, 0.8)),
            true => (TextFont::from_font_size(size), Color::srgba(0.06, 0.04, 0.01, 0.85)),
        };
        let cells = cells_of(target);
        let hexes = if curved(target) { map.0.zone_spine(cells) } else { Vec::new() };
        let part = match target {
            MapTarget::Sea(z) => crate::geography::is_part(&map.0, crate::geography::GeoFamily::Seas, z),
            MapTarget::Land(z) => crate::geography::is_part(&map.0, crate::geography::GeoFamily::Lands, z),
            MapTarget::Relief(z) => crate::geography::is_part(&map.0, crate::geography::GeoFamily::Reliefs, z),
            MapTarget::Region(z) => crate::geography::is_part(&map.0, crate::geography::GeoFamily::Regions, z),
            _ => false,
        };
        for copy in 0..copies {
            let curve = (!hexes.is_empty()).then(|| {
                let letters = text
                    .chars()
                    .map(|ch| {
                        commands
                            .spawn((
                                CurvedLetter,
                                (TargetButton::new(target), UiPointer::default()),
                                Node { position_type: PositionType::Absolute, ..default() },
                                UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
                                Text::new(ch.to_string()),
                                font.clone(),
                                TextColor(Color::NONE),
                                TextShadow { offset: Vec2::new(1.0, 1.0), color: shadow },
                                GlobalZIndex(-4),
                                Visibility::Hidden,
                            ))
                            .id()
                    })
                    .collect();
                Curve { letters, hexes: hexes.clone(), points: smooth_line(&hexes), gap: spacing * size }
            });
            commands.spawn((
                SeaName { zone: area, copy, min_px, max_px, rank, family, tint, fit, weight: cells.len(), part, curve },
                (TargetButton::new(target), UiPointer::default()),
                Node { position_type: PositionType::Absolute, ..default() },
                UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
                Text::new(text.clone()),
                font.clone(),
                bevy::text::LetterSpacing::Rem(spacing),
                TextColor(Color::NONE),
                TextShadow { offset: Vec2::new(1.0, 1.0), color: shadow },
                GlobalZIndex(-4),
                Visibility::Hidden,
            ));
        }
    }
}

/// Axe lissé d'une zone (points du monde 2D) : chaque case de l'axe, moyennée avec ses voisines
/// le long de l'axe (deux passes sur cinq cases), les bouts gardés.
fn smooth_line(hexes: &[Hex]) -> Vec<Vec2> {
    let mut points: Vec<Vec2> = hexes.iter().map(|h| Vec2::from(h.to_world())).collect();
    for _ in 0..2 {
        let before = points.clone();
        for i in 1..points.len().saturating_sub(1) {
            let (a, b) = (i.saturating_sub(2), (i + 2).min(points.len() - 1));
            points[i] = before[a..=b].iter().copied().sum::<Vec2>() / (b - a + 1) as f32;
        }
    }
    points
}

/// Point et direction d'une ligne brisée à la distance `s` de son début (pixels).
fn along(line: &[Vec2], s: f32) -> (Vec2, Vec2) {
    let mut left = s.max(0.0);
    for w in line.windows(2) {
        let step = w[1] - w[0];
        let len = step.length();
        if left <= len && len > 0.0 {
            return (w[0] + step * (left / len), step / len);
        }
        left -= len;
    }
    let n = line.len();
    (line[n - 1], (line[n - 1] - line[n.saturating_sub(2)]).normalize_or_zero())
}

/// Lettres d'un nom courbé le long d'une ligne à l'écran (lue de gauche à droite), centrées sur la
/// ligne : position et angle de chaque lettre (largeurs `widths`, espace `gap`). Aucune si la ligne
/// est trop courte pour le nom, tourne trop d'une lettre à l'autre ou penche trop.
fn curve_letters(line: &[Vec2], widths: &[f32], gap: f32) -> Option<Vec<(Vec2, f32)>> {
    let mut line = line.to_vec();
    if line.len() < 2 {
        return None;
    }
    // (jamais à l'envers)
    if line[line.len() - 1].x < line[0].x {
        line.reverse();
    }
    let length: f32 = line.windows(2).map(|w| w[0].distance(w[1])).sum();
    let text = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    if length < text * CURVE_SLACK {
        return None;
    }
    let mut s = (length - text) / 2.0;
    let mut out: Vec<(Vec2, f32)> = Vec::with_capacity(widths.len());
    for &w in widths {
        let (at, _) = along(&line, s + w / 2.0);
        // Direction sur la largeur de la lettre (plus stable qu'au point même)
        let (before, _) = along(&line, s);
        let (after, _) = along(&line, s + w.max(4.0));
        let dir = after - before;
        let angle = dir.y.atan2(dir.x);
        if angle.abs() > CURVE_MAX_TILT {
            return None;
        }
        if let Some(&(_, prev)) = out.last() {
            let mut turn = (angle - prev).abs();
            if turn > std::f32::consts::PI {
                turn = std::f32::consts::TAU - turn;
            }
            if turn > CURVE_MAX_TURN {
                return None;
            }
        }
        out.push((at, angle));
        s += w + gap;
    }
    Some(out)
}

/// Noms des zones (étendues marines, terres, relief, régions) : sur la partie explorée la plus
/// éloignée de leurs bords, à leur niveau de zoom (fondu aux bords de la plage), si la zone est
/// assez large à l'écran pour son nom, sans chevaucher un nom plus important, dans le quota de son
/// rang (les plus grandes zones d'abord). Une chaîne, un massif ou un golfe allongés ont leur nom
/// courbé le long de leur axe quand il est entièrement exploré et assez long à l'écran.
/// Le niveau de zoom est celui du centre de la vue, le même pour tous les noms : en 3D, les cases
/// proches de la caméra paraissent plus grandes que les lointaines, et les grands noms (océans,
/// continents) s'effaçaient au premier plan au profit des noms de petites zones.
#[allow(clippy::too_many_arguments)]
fn place_sea_names(
    mode: Res<ViewMode>,
    cam3d: Res<crate::view3d::Cam3d>,
    (layer, geo): (Res<crate::render::MapLayer>, Res<crate::render::GeoView>),
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    filter: Res<LabelFilter>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut names: Query<(&SeaName, &mut Node, &mut Visibility, &mut TextColor, &ComputedNode), Without<RiverName>>,
    mut letters: Query<(&mut Node, &mut UiTransform, &mut Visibility, &mut TextColor, &ComputedNode), (With<CurvedLetter>, Without<SeaName>, Without<RiverName>)>,
    mut spots: Local<Vec<Vec<Hex>>>,
) {
    let seas = map.0.seas();
    // Zones des noms : étendues marines puis terres, relief, régions (cases, distance au bord,
    // demi-largeur, nombre de copies du nom)
    let areas: Vec<(&[usize], &[u16], f32, usize)> = seas
        .zones
        .iter()
        .map(|z| (z.cells.as_slice(), z.inner.as_slice(), z.half_width, if z.kind == crate::seas::SeaKind::Ocean { (z.cells.len() / SEA_NAME_REPEAT_CELLS).clamp(1, 4) } else { 1 }))
        .chain(map.0.lands().zones.iter().map(|z| (z.cells.as_slice(), z.inner.as_slice(), z.half_width, 1)))
        .chain(map.0.reliefs().zones.iter().map(|z| (z.cells.as_slice(), z.inner.as_slice(), z.half_width, 1)))
        .chain(map.0.regions().zones.iter().map(|z| (z.cells.as_slice(), z.inner.as_slice(), z.half_width, 1)))
        .collect();
    // Cases du nom de chaque zone, parmi ses cases explorées (refaites quand le brouillard
    // change) : la plus éloignée de ses bords, puis, pour un grand océan, d'autres cases bien au
    // large, loin des précédentes
    if fog.is_changed() || spots.len() != areas.len() {
        *spots = areas
            .iter()
            .map(|&(cells, inner, _, wanted)| {
                let known: Vec<(usize, u16)> = cells.iter().zip(inner).filter(|(c, _)| fog.explored.contains(&map.0.hex_at(**c))).map(|(c, d)| (*c, *d)).collect();
                // (assez de la zone connue pour la nommer)
                if known.len() < cells.len().min(4) {
                    return Vec::new();
                }
                let Some(&(first, best)) = known.iter().max_by_key(|(c, d)| (*d, std::cmp::Reverse(*c))) else { return Vec::new() };
                let mut anchors = vec![map.0.hex_at(first)];
                let offshore: Vec<Hex> = known.iter().filter(|(_, d)| u32::from(*d) * 5 >= u32::from(best) * 3).map(|(c, _)| map.0.hex_at(*c)).collect();
                while anchors.len() < wanted {
                    let far = offshore
                        .iter()
                        .map(|&h| (anchors.iter().map(|&a| map.0.distance(a, h)).min().unwrap_or(0), h))
                        .max_by_key(|(d, h)| (*d, std::cmp::Reverse((h.q, h.r))));
                    match far {
                        Some((d, h)) if d >= SEA_NAME_SPACING => anchors.push(h),
                        _ => break,
                    }
                }
                anchors
            })
            .collect();
    }
    let camera = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active);
    // Quotas à la taille de l'écran
    let viewport = camera.and_then(|(c, ..)| c.logical_viewport_size()).unwrap_or(Vec2::new(1600.0, 900.0));
    let screen = viewport.x * viewport.y;
    // (seuls les noms à l'écran comptent dans les quotas)
    let on_screen = |r: &ScreenRect| r.min.x < viewport.x && r.min.y < viewport.y && r.min.x + r.size.x > 0.0 && r.min.y + r.size.y > 0.0;
    // Carte géographique : seuls les noms de la famille montrée, ceux des zones colorées
    let geo_map = *layer == crate::render::MapLayer::Geography;
    let quota_scale = if geo_map { GEO_QUOTA } else { 1.0 };
    let quota = |rank: u8| (AREA_QUOTA[usize::from(rank).min(AREA_QUOTA.len() - 1)] * quota_scale * screen / QUOTA_SCREEN).round().max(1.0) as usize;
    let mut per_rank = [0usize; AREA_QUOTA.len()];
    // Niveau de zoom de la vue : taille d'une case au point visé par la caméra
    let view_px = camera.and_then(|(cam, transform, _)| {
        let focus = if mode.is_3d { cam3d.target } else { Vec3::new(transform.translation.x, transform.translation.y, 0.0) };
        view_level(cam, transform, focus)
    });
    let mut order: Vec<_> = names.iter_mut().collect();
    order.sort_by_key(|(n, ..)| (n.rank, n.copy, std::cmp::Reverse(n.weight), n.zone));
    let mut placed: Vec<ScreenRect> = Vec::new();
    for (name, mut node, mut vis, mut color, computed) in order {
        let tint = if geo_map { GEO_NAME_COLOR } else { name.tint };
        // (centre du nom droit, alpha) ou (lettres du nom courbé, alpha)
        let mut straight = None;
        let mut bent: Option<(Vec<(Vec2, f32)>, f32)> = None;
        let family = match name.family {
            AreaFamily::Sea => crate::geography::GeoFamily::Seas,
            AreaFamily::Land => crate::geography::GeoFamily::Lands,
            AreaFamily::Relief => crate::geography::GeoFamily::Reliefs,
            AreaFamily::Region => crate::geography::GeoFamily::Regions,
        };
        let wanted = if geo_map {
            family == geo.family && (!name.part || geo.deep)
        } else {
            match name.family {
                AreaFamily::Sea => filter.seas,
                AreaFamily::Land => filter.lands,
                AreaFamily::Relief => filter.reliefs,
                AreaFamily::Region => filter.regions,
            }
        };
        let rank = usize::from(name.rank).min(AREA_QUOTA.len() - 1);
        if let (Some((camera, transform, _)), true, Some(hex)) = (camera, wanted && per_rank[rank] < quota(name.rank), spots.get(name.zone).and_then(|a| a.get(name.copy))) {
            let cam_global = GlobalTransform::from(*transform);
            let lift = |h: Hex, p: Vec2| if mode.is_3d { Vec3::new(p.x, shown_height(&map, &fog, h) + 2.0, -p.y) } else { Vec3::new(p.x, p.y, 0.0) };
            let world = near_camera(&map, lift(*hex, Vec2::from(hex.to_world())), transform.translation.x);
            if let Ok(center) = camera.world_to_viewport(&cam_global, world)
                && let Ok(edge) = camera.world_to_viewport(&cam_global, world + transform.right() * crate::hex::HEX_SIZE)
            {
                // Taille d'une case au nom (le nom tient-il dans sa zone ?) et niveau de zoom de la vue
                let px = edge.distance(center).max(0.01);
                let level = view_px.unwrap_or(px);
                // Fondu aux bords de la plage de zoom
                let fade_in = if name.min_px <= 0.0 { 1.0 } else { ((level - name.min_px * 0.8) / (name.min_px * 0.4)).clamp(0.0, 1.0) };
                let fade_out = ((name.max_px * 1.2 - level) / (name.max_px * 0.4)).clamp(0.0, 1.0);
                // (carte géographique : le niveau de la carte décide, pas la plage du nom)
                let alpha = if geo_map { 1.0 } else { fade_in.min(fade_out) };
                // Nom courbé : axe exploré, projeté à l'écran (d'un seul tenant près de la caméra)
                let curve = name.curve.as_ref().filter(|c| alpha > 0.02 && c.hexes.iter().all(|h| fog.explored.contains(&map.0.canonical(*h))));
                if let Some(curve) = curve {
                    let mid = curve.points[curve.points.len() / 2];
                    let shift = near_camera(&map, Vec3::new(mid.x, 0.0, 0.0), transform.translation.x).x - mid.x;
                    let line: Option<Vec<Vec2>> = curve.hexes.iter().zip(&curve.points).map(|(&h, &p)| camera.world_to_viewport(&cam_global, lift(h, p + Vec2::new(shift, 0.0))).ok()).collect();
                    let sizes: Vec<Vec2> = curve.letters.iter().map(|&e| letters.get(e).map_or(Vec2::ZERO, |l| logical_size(l.4, Vec2::ZERO))).collect();
                    if let Some(line) = line
                        && sizes.iter().all(|s| s.x > 0.0)
                        && let Some(spots) = curve_letters(&line, &sizes.iter().map(|s| s.x).collect::<Vec<_>>(), curve.gap)
                    {
                        let height = sizes.iter().map(|s| s.y).fold(0.0, f32::max);
                        let (lo, hi) = spots.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), (p, _)| (lo.min(*p), hi.max(*p)));
                        let rect = ScreenRect { min: lo - Vec2::splat(height / 2.0 + AREA_GAP), size: hi - lo + Vec2::splat(height + 2.0 * AREA_GAP) };
                        if on_screen(&rect) && !placed.iter().any(|r| r.overlaps(&rect)) {
                            placed.push(rect);
                            bent = Some((spots, alpha));
                        }
                    }
                }
                if bent.is_none() {
                    let size = logical_size(computed, Vec2::new(80.0, 20.0));
                    // La zone doit être assez large à l'écran pour son nom
                    let half_width = areas[name.zone].2 / crate::hex::HEX_SIZE * px;
                    let fits = half_width * 2.0 >= size.x * name.fit;
                    let rect = ScreenRect { min: center - size / 2.0 - Vec2::splat(AREA_GAP), size: size + Vec2::splat(2.0 * AREA_GAP) };
                    if alpha > 0.02 && fits && on_screen(&rect) && !placed.iter().any(|r| r.overlaps(&rect)) {
                        placed.push(rect);
                        straight = Some((center, alpha));
                    }
                }
            }
        }
        if straight.is_some() || bent.is_some() {
            per_rank[rank] += 1;
        }
        let shade = |alpha: f32| Color::srgba(tint.0, tint.1, tint.2, alpha * 0.9);
        match straight {
            Some((p, alpha)) => {
                let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
                if node.left != left || node.top != top {
                    node.left = left;
                    node.top = top;
                }
                if color.0 != shade(alpha) {
                    color.0 = shade(alpha);
                }
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
            None => {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
        // Lettres du nom courbé : posées et tournées le long de l'axe, ou cachées
        if let Some(curve) = &name.curve {
            for (i, &e) in curve.letters.iter().enumerate() {
                let Ok((mut node, mut transform, mut vis, mut color, _)) = letters.get_mut(e) else { continue };
                match bent.as_ref().and_then(|(spots, alpha)| spots.get(i).map(|s| (*s, *alpha))) {
                    Some(((p, angle), alpha)) => {
                        let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
                        if node.left != left || node.top != top {
                            node.left = left;
                            node.top = top;
                        }
                        let rotation = Rot2::radians(angle);
                        if transform.rotation != rotation {
                            transform.rotation = rotation;
                        }
                        if color.0 != shade(alpha) {
                            color.0 = shade(alpha);
                        }
                        if *vis != Visibility::Inherited {
                            *vis = Visibility::Inherited;
                        }
                    }
                    None => {
                        if *vis != Visibility::Hidden {
                            *vis = Visibility::Hidden;
                        }
                    }
                }
            }
        }
    }
}

/// Icône de poisson (carte des ressources, légende).
#[derive(Component)]
pub struct FishIcon;
/// Icône d'un banc de poissons (carte des ressources) : un poisson discret et le nom du banc ;
/// la zone apparaît au survol (et à la sélection).
#[derive(Component)]
struct BankIcon {
    bank: usize,
}
#[derive(Component)]
struct BankName;
/// Rayon d'une case à l'écran (pixels) à partir duquel l'icône d'un banc, puis son nom, se montrent.
const BANK_ICON_PX: f32 = 5.0;
const BANK_NAME_PX: f32 = 12.0;

/// Image des icônes de poisson.
fn attach_fish_images(mut commands: Commands, assets: Res<AssetServer>, icons: Query<Entity, Added<FishIcon>>) {
    for e in &icons {
        commands.entity(e).insert(ImageNode::new(assets.load("icons/fish.png")));
    }
}

/// Une icône par banc de poissons (refaites si la carte change de monde).
fn spawn_bank_icons(mut commands: Commands, map: Res<GameMap>, old: Query<Entity, With<BankIcon>>) {
    if !map.is_added() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    for (b, bank) in map.0.banks().iter().enumerate() {
        commands.spawn((
            BankIcon { bank: b },
            (TargetButton::new(MapTarget::Bank(b)), UiPointer::default()),
            Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
            UiTransform::from_translation(Val2::percent(-50.0, -30.0)),
            GlobalZIndex(-3),
            Visibility::Hidden,
            children![
                (FishIcon, Node { width: Val::Px(20.0), height: Val::Px(20.0), ..default() }),
                (
                    BankName,
                    Text::new(bank.name.clone()),
                    TextFont::from_font_size(11.0),
                    TextColor(Color::srgb(0.98, 0.92, 0.75)),
                    TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) },
                ),
            ],
        ));
    }
}

/// Icônes des bancs : seulement sur la carte des ressources (ou quand le banc est mis en avant,
/// par la recherche par exemple), au centre de leur partie explorée ; poisson
/// discret (plus net au survol), nom en zoom plus fin.
#[allow(clippy::too_many_arguments)]
fn place_bank_icons(
    mode: Res<ViewMode>,
    layer: Res<crate::render::MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    pointed: Res<crate::search::PointedTarget>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut icons: Query<(&BankIcon, &UiPointer, &mut Node, &mut Visibility, &Children), (Without<RiverName>, Without<SeaName>, Without<Anchor>)>,
    mut images: Query<&mut ImageNode>,
    mut names: Query<&mut Node, (With<BankName>, Without<BankIcon>)>,
    mut spots: Local<Vec<Option<Hex>>>,
) {
    let banks = map.0.banks();
    if fog.is_changed() || spots.len() != banks.len() {
        // Case de l'icône : la case explorée la plus proche du centre de la partie explorée (un
        // banc en anneau autour d'une île a son icône sur l'anneau, au plus près du centre)
        *spots = banks
            .iter()
            .map(|b| {
                let known: Vec<Hex> = b.cells.iter().map(|&c| map.0.hex_at(c)).filter(|h| fog.explored.contains(h)).collect();
                let first = *known.first()?;
                // (planète : chaque case prise du côté du raccord le plus proche de la première)
                let points: Vec<Vec2> = known.iter().map(|&h| Vec2::from(map.0.wrap().nearest(first, h).to_world())).collect();
                let center = points.iter().sum::<Vec2>() / points.len() as f32;
                known.iter().zip(&points).min_by(|a, b| a.1.distance(center).total_cmp(&b.1.distance(center))).map(|(h, _)| *h)
            })
            .collect();
    }
    let camera = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active);
    for (icon, interaction, mut node, mut vis, children) in &mut icons {
        let lit = pointed.current() == Some(MapTarget::Bank(icon.bank));
        let mut shown = None;
        if let (Some((camera, transform, _)), Some(Some(hex))) = (camera, spots.get(icon.bank))
            && (*layer == crate::render::MapLayer::Resources || lit)
        {
            let cam_global = GlobalTransform::from(*transform);
            let world = if mode.is_3d {
                to_3d(*hex, shown_height(&map, &fog, *hex) + 3.0)
            } else {
                let (x, y) = hex.to_world();
                Vec3::new(x, y, 0.0)
            };
            let world = near_camera(&map, world, transform.translation.x);
            if let Ok(center) = camera.world_to_viewport(&cam_global, world)
                && let Ok(edge) = camera.world_to_viewport(&cam_global, world + transform.right() * crate::hex::HEX_SIZE)
            {
                let px = edge.distance(center);
                if px >= BANK_ICON_PX || lit {
                    shown = Some((center, px));
                }
            }
        }
        let Some((p, px)) = shown else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        let alpha = if lit || *interaction != UiPointer::None { 1.0 } else { 0.55 };
        for &child in children {
            if let Ok(mut image) = images.get_mut(child) {
                let c = Color::srgba(1.0, 0.95, 0.80, alpha);
                if image.color != c {
                    image.color = c;
                }
            }
            if let Ok(mut name) = names.get_mut(child) {
                let display = if px >= BANK_NAME_PX || lit { Display::Flex } else { Display::None };
                if name.display != display {
                    name.display = display;
                }
            }
        }
    }
}

/// Nom d'un lieu du désert (oasis, vallée) : seulement en zoom fin (comme celui d'un banc), une
/// fois sa case explorée ; une vallée, sur sa case la plus proche de son centre.
#[derive(Component)]
struct OasisName(Hex);
/// Rayon d'une case à l'écran (pixels) à partir duquel le nom d'une oasis ou d'une vallée se montre.
const OASIS_NAME_PX: f32 = 22.0;

/// Une étiquette par oasis et par vallée du désert (refaites si la carte change de monde ou si
/// l'eau est refaite).
fn spawn_oasis_names(mut commands: Commands, map: Res<GameMap>, old: Query<Entity, With<OasisName>>, mut renamed: MessageReader<crate::console::NamesChanged>) {
    if !map.is_added() && renamed.read().count() == 0 {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let places = map.0.desert_places();
    for (k, place) in places.iter().enumerate() {
        // Une étiquette par nom : sur le plus grand morceau d'une vallée coupée en plusieurs
        if places.iter().enumerate().any(|(j, p)| p.name == place.name && (p.cells.len(), std::cmp::Reverse(j)) > (place.cells.len(), std::cmp::Reverse(k))) {
            continue;
        }
        // (planète : chaque case prise du côté du raccord le plus proche de la première)
        let first = map.0.hex_at(place.cells[0]);
        let points: Vec<Vec2> = place.cells.iter().map(|&c| Vec2::from(map.0.wrap().nearest(first, map.0.hex_at(c)).to_world())).collect();
        let center = points.iter().sum::<Vec2>() / points.len() as f32;
        let spot = place.cells.iter().zip(&points).min_by(|a, b| a.1.distance(center).total_cmp(&b.1.distance(center))).map_or(first, |(&c, _)| map.0.hex_at(c));
        commands.spawn((
            OasisName(spot),
            Text::new(place.name.clone()),
            TextFont::from_font_size(11.0),
            TextColor(Color::srgb(0.95, 0.90, 0.70)),
            TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) },
            Node { position_type: PositionType::Absolute, ..default() },
            UiTransform::from_translation(Val2::percent(-50.0, 60.0)),
            GlobalZIndex(-3),
            Visibility::Hidden,
        ));
    }
}

/// Noms des oasis : sous le centre de leur case, en zoom fin, sur une case explorée.
fn place_oasis_names(
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut names: Query<(&OasisName, &mut Node, &mut Visibility), (Without<RiverName>, Without<SeaName>, Without<Anchor>, Without<BankIcon>)>,
) {
    let camera = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active);
    for (oasis, mut node, mut vis) in &mut names {
        let mut shown = None;
        if let Some((camera, transform, _)) = camera
            && fog.explored.contains(&oasis.0)
        {
            let cam_global = GlobalTransform::from(*transform);
            let world = if mode.is_3d {
                to_3d(oasis.0, shown_height(&map, &fog, oasis.0) + 3.0)
            } else {
                let (x, y) = oasis.0.to_world();
                Vec3::new(x, y, 0.0)
            };
            let world = near_camera(&map, world, transform.translation.x);
            if let Ok(center) = camera.world_to_viewport(&cam_global, world)
                && let Ok(edge) = camera.world_to_viewport(&cam_global, world + transform.right() * crate::hex::HEX_SIZE)
                && edge.distance(center) >= OASIS_NAME_PX
            {
                shown = Some(center);
            }
        }
        let Some(p) = shown else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}

/// État d'une ville pour la ressource choisie au tableau de bord (carte de disponibilité) : une
/// pastille sous sa bannière, « en trop », « suffit » ou « manque ».
#[derive(Component)]
struct CityStatus(Entity);

fn spawn_city_status(mut commands: Commands, cities: Query<(Entity, &HexPos), Added<City>>) {
    for (entity, pos) in &cities {
        let mut anchor = Anchor::new(pos.0, -26.0, CITY_HEIGHT - 6.0, false);
        anchor.hidden = true;
        commands.spawn((
            CityStatus(entity),
            anchor,
            Node { position_type: PositionType::Absolute, padding: UiRect::axes(Val::Px(7.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() },
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
            BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.9)),
            GlobalZIndex(-1),
            Visibility::Hidden,
            children![(Text::new(""), TextFont::from_font_size(12.0), TextColor(Color::WHITE))],
        ));
    }
}

fn update_city_status(
    mut commands: Commands,
    tableau: Res<crate::tableau::Tableau>,
    cities: Query<&City>,
    mut badges: Query<(Entity, &CityStatus, &mut Anchor, &Children)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for (e, status, mut anchor, children) in &mut badges {
        let Ok(city) = cities.get(status.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let etat = tableau.choisie.map(|r| (r, crate::tableau::EtatRessource::de(city.state.ressource(r), city.state.demande_echange(r))));
        let hidden = etat.is_none();
        if anchor.hidden != hidden {
            anchor.hidden = hidden;
        }
        if let Some((r, etat)) = etat
            && let Some(&c) = children.first()
            && let Ok((mut text, mut color)) = texts.get_mut(c)
        {
            let t = format!("{} : {}", r.nom(), etat.nom());
            if text.0 != t {
                text.0 = t;
            }
            if color.0 != etat.couleur() {
                color.0 = etat.couleur();
            }
        }
    }
}

/// Étiquette d'un gisement (carte des ressources) : une pastille à la couleur de la ressource et
/// son nom en entier (de loin : la pastille seule).
#[derive(Component)]
struct DepositIcon(Hex);
/// Nom de la ressource dans l'étiquette d'un gisement.
#[derive(Component)]
struct DepositName;
/// Rayon d'une case à l'écran (pixels) à partir duquel le nom de la ressource s'écrit.
const DEPOSIT_NAME_PX: f32 = 14.0;
/// Rayon d'une case à l'écran (pixels) à partir duquel les icônes des gisements se montrent.
const DEPOSIT_ICON_PX: f32 = 6.0;

/// Une icône par gisement de la carte (cachée tant qu'il n'est pas connu ; refaites si la carte
/// change de monde).
fn spawn_deposit_icons(mut commands: Commands, map: Res<GameMap>, old: Query<Entity, With<DepositIcon>>, mut edited: MessageReader<crate::console::MapEdited>) {
    if !map.is_added() && edited.read().count() == 0 {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    for (h, r) in map.0.cases_a_gisement() {
        let (cr, cg, cb) = r.couleur();
        let couleur = Color::srgb(cr, cg, cb);
        commands.spawn((
            DepositIcon(h),
            Node {
                position_type: PositionType::Absolute,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.85)),
            BorderColor::all(couleur),
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            GlobalZIndex(-3),
            Visibility::Hidden,
            children![
                (Node { width: Val::Px(12.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(couleur)),
                (DepositName, Node { margin: UiRect::right(Val::Px(3.0)), ..default() }, Text::new(r.nom()), TextFont::from_font_size(13.0), TextColor(Color::srgb(0.97, 0.97, 0.97))),
            ],
        ));
    }
}

/// Icônes des gisements : seulement sur la carte des ressources, pour les gisements connus (tous
/// en mode debug), assez près.
#[allow(clippy::too_many_arguments)]
fn place_deposit_icons(
    mode: Res<ViewMode>,
    layer: Res<crate::render::MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    gisements: Res<crate::game::Gisements>,
    debug: Res<crate::game::DebugMode>,
    tableau: Res<crate::tableau::Tableau>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut icons: Query<(&DepositIcon, &mut Node, &mut Visibility, &Children), (Without<RiverName>, Without<SeaName>, Without<Anchor>, Without<BankIcon>, Without<OasisName>, Without<DepositName>)>,
    mut noms: Query<&mut Node, (With<DepositName>, Without<DepositIcon>)>,
) {
    let camera = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active);
    // (carte des ressources, ou carte de disponibilité de la ressource choisie au tableau de bord)
    let carte = *layer == crate::render::MapLayer::Resources || tableau.choisie.is_some();
    for (icon, mut node, mut vis, enfants) in &mut icons {
        let mut shown = None;
        let mut rayon = 0.0;
        if carte
            && tableau.choisie.is_none_or(|r| map.0.gisement(icon.0) == Some(r))
            && let Some((camera, transform, _)) = camera
            && (debug.0 || gisements.connus.contains(&icon.0))
        {
            let cam_global = GlobalTransform::from(*transform);
            let world = if mode.is_3d {
                to_3d(icon.0, shown_height(&map, &fog, icon.0) + 3.0)
            } else {
                let (x, y) = icon.0.to_world();
                Vec3::new(x, y, 0.0)
            };
            let world = near_camera(&map, world, transform.translation.x);
            if let Ok(center) = camera.world_to_viewport(&cam_global, world)
                && let Ok(edge) = camera.world_to_viewport(&cam_global, world + transform.right() * crate::hex::HEX_SIZE)
                && edge.distance(center) >= DEPOSIT_ICON_PX
            {
                shown = Some(center);
                rayon = edge.distance(center);
            }
        }
        let Some(p) = shown else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        // (de loin, la pastille seule ; d'assez près, le nom en entier)
        let d = if rayon >= DEPOSIT_NAME_PX { Display::Flex } else { Display::None };
        for &e in enfants {
            if let Ok(mut n) = noms.get_mut(e)
                && n.display != d
            {
                n.display = d;
            }
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}

/// Nom des villages et des camps (cliquable : la caméra y va), une fois leur case explorée.
fn spawn_site_labels(mut commands: Commands, sites: Query<(Entity, &crate::hostile::SiteName, &HexPos, Has<crate::hostile::Camp>), Added<crate::hostile::SiteName>>) {
    for (entity, name, pos, camp) in &sites {
        commands.spawn((
            SiteLabel(entity),
            (TargetButton::new(MapTarget::Site(entity)), UiPointer::default()),
            // (en 3D, le nom passe sous le site : il ne masque ni le feu ni les bâtiments)
            Anchor { explored_only: true, min_px: SITE_LABEL_PX, ..Anchor::new(pos.0, -17.0, -20.0, false) },
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            // Ancre au milieu du bord supérieur (sous l'icône)
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
            BackgroundColor(if camp { CAMP_LABEL_BG } else { VILLAGE_LABEL_BG }),
            GlobalZIndex(-2),
            Visibility::Hidden,
            children![(Text::new(name.0.clone()), TextFont::from_font_size(11.0), TextColor(Color::srgb(0.96, 0.92, 0.85)))],
        ));
    }
}

/// Le nom disparaît avec son site (camp dispersé, village visité).
fn despawn_site_labels(mut commands: Commands, labels: Query<(Entity, &SiteLabel)>, sites: Query<(), With<crate::hostile::SiteName>>) {
    for (label, site) in &labels {
        if sites.get(site.0).is_err() {
            commands.entity(label).despawn();
        }
    }
}

/// Une étiquette par grand lac et par cours d'eau nommé (refaites si la carte change de
/// monde) ; les plus importants en plus gros.
fn spawn_river_names(mut commands: Commands, map: Res<GameMap>, old: Query<Entity, With<RiverName>>, mut renamed: MessageReader<crate::console::NamesChanged>) {
    // (et quand la console de debug refait l'eau : noms et cours d'eau changent)
    if !map.is_added() && renamed.read().count() == 0 {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let basins = map.0.basins();
    let rank = |tier: u8, lake: bool, i: usize| tier as usize * 1_000_000 + usize::from(lake) * 500_000 + i;
    for (i, lake) in basins.lakes.iter().enumerate() {
        let (size, color, _) = NAME_STYLES[lake.tier as usize];
        let spots = lake.cells.iter().map(|&c| (map.0.hex_at(c), map.0.hex_at(c))).collect();
        commands.spawn((
            RiverName { rank: rank(lake.tier, true, i), tier: lake.tier, spots, ordered: true, feature: Feature::Lake(i) },
            // Clic : la caméra va au lac (le survol passe par `HoveredLabel`, selon l'option)
            (TargetButton::click_only(MapTarget::Lake(i)), UiPointer::default()),
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            BackgroundColor(LAKE_NAME_BG),
            GlobalZIndex(-3),
            Visibility::Hidden,
            children![(Text::new(lake.name.clone()), TextFont::from_font_size(size), TextColor(color))],
        ));
    }
    for (i, river) in basins.rivers.iter().enumerate() {
        let (size, color, _) = NAME_STYLES[river.tier as usize];
        let spots = river.spots.iter().map(|&(a, b)| (map.0.hex_at(a), map.0.hex_at(b))).collect();
        commands.spawn((
            RiverName { rank: rank(river.tier, false, i), tier: river.tier, spots, ordered: false, feature: Feature::River(i) },
            (TargetButton::click_only(MapTarget::River(i)), UiPointer::default()),
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            BackgroundColor(if river.fleuve { RIVER_NAME_BG } else { STREAM_NAME_BG }),
            GlobalZIndex(-3),
            Visibility::Hidden,
            children![(Text::new(river.name.clone()), TextFont::from_font_size(size), TextColor(color))],
        ));
    }
}

/// Population et nom à jour ; la bannière disparaît avec sa ville.
fn update_city_banners(
    mut commands: Commands,
    cities: Query<Ref<City>>,
    banners: Query<(Entity, &CityBanner, &Children)>,
    pop_boxes: Query<&Children, Without<CityBanner>>,
    mut pops: Query<&mut Text, (With<BannerPop>, Without<BannerName>)>,
    mut names: Query<&mut Text, (With<BannerName>, Without<BannerPop>)>,
) {
    for (entity, banner, children) in &banners {
        let Ok(city) = cities.get(banner.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !city.is_changed() {
            continue;
        }
        for child in children.iter() {
            if let Ok(mut name) = names.get_mut(child)
                && name.0 != city.state.name
            {
                name.0 = city.state.name.clone();
            }
            for grandchild in pop_boxes.get(child).into_iter().flatten() {
                if let Ok(mut pop) = pops.get_mut(*grandchild) {
                    let wanted = city.state.palier().to_string();
                    if pop.0 != wanted {
                        pop.0 = wanted;
                    }
                }
            }
        }
    }
}

/// Pastilles numérotées sur la dernière case atteinte à chaque tour (même logique qu'en 2D).
fn update_turn_labels_3d(
    mut commands: Commands,
    hover: Res<HoverPath>,
    order: Res<OrderPath>,
    labels: Query<Entity, With<TurnLabel3d>>,
) {
    if !hover.is_changed() && !order.is_changed() {
        return;
    }
    for e in &labels {
        commands.entity(e).despawn();
    }
    spawn_turn_labels(&mut commands, &order.path, &order.turns, ORDER_LABEL);
    if hover.path != order.path {
        spawn_turn_labels(&mut commands, &hover.path, &hover.turns, HOVER_LABEL);
    }
}

fn spawn_turn_labels(commands: &mut Commands, path: &[Hex], turns: &[u32], background: Color) {
    for (i, (&hex, &turn)) in path.iter().zip(turns).enumerate() {
        let last_of_turn = turns.get(i + 1).is_none_or(|&next| next != turn);
        if !last_of_turn {
            continue;
        }
        commands.spawn((
            TurnLabel3d,
            // Juste au-dessus de la ligne du trajet
            Anchor::new(hex, 0.0, 7.0, true),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            BackgroundColor(background),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            GlobalZIndex(-1),
            Visibility::Hidden,
            children![(Text::new(turn.to_string()), TextFont::from_font_size(12.0), TextColor(Color::WHITE))],
        ));
    }
}

fn spawn_stuck_badges(mut commands: Commands, units: Query<(Entity, &HexPos), Added<Unit>>) {
    for (unit, pos) in &units {
        commands.spawn((
            StuckBadge(unit),
            Anchor { hidden: true, token_corner: true, ..Anchor::new(pos.0, 0.0, TOKEN_LIFT, false) },
            Node {
                position_type: PositionType::Absolute,
                min_width: Val::Px(20.0),
                height: Val::Px(20.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            BackgroundColor(STUCK_BG),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            GlobalZIndex(-1),
            Visibility::Hidden,
            children![(StuckText, Text::new(""), TextFont::from_font_size(13.0), TextColor(Color::WHITE))],
        ));
    }
}

/// Suit l'unité comme sa barre de PV ; montrée tant que la dette l'empêche de bouger un tour entier.
fn update_stuck_badges(
    mut commands: Commands,
    time: Res<Time>,
    units: Query<(&Unit, &HexPos, Has<Hostile>, Option<&Motion>)>,
    fog: Res<ShownFog>,
    mut badges: Query<(Entity, &StuckBadge, &mut Anchor, &Children)>,
    mut texts: Query<&mut Text, With<StuckText>>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, badge, mut anchor, children) in &mut badges {
        let Ok((unit, pos, hostile, motion)) = units.get(badge.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let turns = unit.0.stuck_turns();
        let glide = motion.and_then(|m| m.sample(now)).map(|p| p.at);
        let hidden = turns == 0 || (hostile && motion.is_none() && !fog.visible.contains(&pos.0));
        // En 3D, à hauteur des têtes des figurines
        let up_3d = TOKEN_LIFT + STUCK_LIFT;
        if anchor.hex != pos.0 || anchor.hidden != hidden || anchor.up_3d != up_3d || anchor.glide != glide {
            anchor.hex = pos.0;
            anchor.hidden = hidden;
            anchor.up_3d = up_3d;
            anchor.glide = glide;
        }
        let label = turns.to_string();
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != label
            {
                text.0 = label.clone();
            }
        }
    }
}

fn spawn_hp_bars_3d(mut commands: Commands, units: Query<(Entity, &HexPos), Added<Unit>>) {
    for (unit, pos) in &units {
        commands.spawn((
            HpBar3d(unit),
            Anchor { hidden: true, under_token: true, ..Anchor::new(pos.0, 0.0, TOKEN_LIFT, true) },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(26.0),
                height: Val::Px(6.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
            BackgroundColor(Color::srgb(0.05, 0.05, 0.07)),
            BorderColor::all(Color::srgb(0.05, 0.05, 0.07)),
            GlobalZIndex(-2),
            Visibility::Hidden,
            children![(HpFill3d, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(HP_GOOD))],
        ));
    }
}

/// Suit l'unité (case, toit de ville, brouillard) et règle le remplissage ; la barre disparaît avec l'unité.
fn update_hp_bars_3d(
    mut commands: Commands,
    time: Res<Time>,
    units: Query<(&Unit, &HexPos, Has<Hostile>, Option<&Motion>, Option<&HeldHp>)>,
    fog: Res<ShownFog>,
    mut bars: Query<(Entity, &HpBar3d, &mut Anchor, &Children)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), With<HpFill3d>>,
) {
    for (entity, bar, mut anchor, children) in &mut bars {
        let Ok((unit, pos, hostile, motion, held)) = units.get(bar.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let now = time.elapsed_secs_f64();
        let hp = HeldHp::shown(held, unit.0.hp, now).clamp(0, MAX_HP);
        let glide = motion.and_then(|m| m.sample(now)).map(|p| p.at);
        let hidden = hp >= MAX_HP || (hostile && motion.is_none() && !fog.visible.contains(&pos.0));
        let up_3d = TOKEN_LIFT;
        if anchor.hex != pos.0 || anchor.hidden != hidden || anchor.up_3d != up_3d || anchor.glide != glide {
            anchor.hex = pos.0;
            anchor.hidden = hidden;
            anchor.up_3d = up_3d;
            anchor.glide = glide;
        }
        let ratio = hp as f32 / MAX_HP as f32;
        let color = if ratio > 0.6 {
            HP_GOOD
        } else if ratio > 0.3 {
            HP_MID
        } else {
            HP_LOW
        };
        for child in children.iter() {
            if let Ok((mut node, mut bg)) = fills.get_mut(child) {
                let width = Val::Percent(ratio * 100.0);
                if node.width != width {
                    node.width = width;
                }
                if bg.0 != color {
                    bg.0 = color;
                }
            }
        }
    }
}

/// Place chaque étiquette à la projection de sa case par la caméra active.
/// Les transformations des caméras sont lues directement (pas de parent) pour éviter un décalage d'une image.
fn place_labels(
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    filter: Res<LabelFilter>,
    mut labels: Query<(&Anchor, &mut Node, &mut Visibility, Has<CityBanner>, Has<SiteLabel>), (Without<RiverName>, Without<SeaName>)>,
) {
    let Some((camera, transform, _)) = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active) else {
        return;
    };
    let cam_global = GlobalTransform::from(*transform);
    // Inclinaison de la caméra : écrase verticalement le disque d'un jeton à l'écran
    let tilt = (-transform.forward().y).clamp(0.2, 1.0);
    for (anchor, mut node, mut vis, is_city, is_site) in &mut labels {
        let world = if mode.is_3d {
            match anchor.glide {
                Some(p) => {
                    let under = map.0.canonical(Hex::from_world(p.x, p.y));
                    Vec3::new(p.x, shown_height(&map, &fog, under) + anchor.up_3d, -p.y)
                }
                None => to_3d(anchor.hex, shown_height(&map, &fog, anchor.hex) + anchor.up_3d),
            }
        } else {
            let (x, y) = anchor.hex.to_world();
            Vec3::new(x, y + anchor.up_2d, 0.0)
        };
        let world = near_camera(&map, world, transform.translation.x);
        let shown = !anchor.hidden
            && (mode.is_3d || !anchor.only_3d)
            && (filter.cities || !is_city)
            && (filter.sites || !is_site)
            && (!anchor.explored_only || fog.explored.contains(&anchor.hex));
        let mut screen = if shown { camera.world_to_viewport(&cam_global, world).ok() } else { None };
        // Assez de zoom (rayon d'une case à l'écran)
        if anchor.min_px > 0.0
            && let Some(center) = screen
        {
            let edge = camera.world_to_viewport(&cam_global, world + transform.right() * crate::hex::HEX_SIZE).ok();
            if edge.is_none_or(|e| e.distance(center) < anchor.min_px) {
                screen = None;
            }
        }
        if anchor.token_corner
            && let Some(center) = screen
        {
            // Bord haut-droit du disque du jeton (écrasé verticalement par l'inclinaison en 3D)
            let edge = camera.world_to_viewport(&cam_global, world + transform.right() * TOKEN_RADIUS).ok();
            let radius = edge.map_or(16.0, |e| e.distance(center)).max(8.0);
            let squash = if mode.is_3d { tilt } else { 1.0 };
            screen = Some(center + Vec2::new(radius * 0.7, -radius * squash * 0.7 - 2.0));
        }
        if anchor.under_token
            && let Some(center) = screen
        {
            // Rayon du jeton à l'écran (dépend du zoom) : barre juste sous le disque, à sa largeur
            let edge = camera.world_to_viewport(&cam_global, world + transform.right() * TOKEN_RADIUS).ok();
            let radius = edge.map_or(16.0, |e| e.distance(center));
            let width = Val::Px((radius * 1.6).clamp(14.0, 48.0).round());
            if node.width != width {
                node.width = width;
            }
            screen = Some(center + Vec2::Y * (radius * tilt + 5.0));
        }
        let wanted_vis = if screen.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted_vis {
            *vis = wanted_vis;
        }
        if let Some(p) = screen {
            let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
            if node.left != left || node.top != top {
                node.left = left;
                node.top = top;
            }
        }
    }
}

/// Rectangle d'une étiquette à l'écran (pixels logiques) : coin haut-gauche et taille.
#[derive(Clone, Copy)]
struct ScreenRect {
    min: Vec2,
    size: Vec2,
}

impl ScreenRect {
    fn overlaps(&self, other: &ScreenRect) -> bool {
        let (a_max, b_max) = (self.min + self.size, other.min + other.size);
        self.min.x < b_max.x + LABEL_MARGIN
            && other.min.x < a_max.x + LABEL_MARGIN
            && self.min.y < b_max.y + LABEL_MARGIN
            && other.min.y < a_max.y + LABEL_MARGIN
    }
}

/// Taille logique d'un nœud ; avant sa première mise en page, une estimation.
fn logical_size(computed: &ComputedNode, fallback: Vec2) -> Vec2 {
    let size = computed.size() * computed.inverse_scale_factor();
    if size.x > 0.0 { size } else { fallback }
}

/// Place les noms des cours d'eau, du plus important au moins important : sur l'emplacement
/// exploré le plus proche du milieu du cours où l'étiquette ne chevauche ni une bannière de ville
/// ni un nom déjà posé ; sinon le nom est masqué. Les cours d'eau secondaires ne sont nommés que
/// d'assez près.
fn place_river_names(
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    filter: Res<LabelFilter>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    banners: Query<(&Node, &ComputedNode, &Visibility), (With<CityBanner>, Without<RiverName>)>,
    mut names: Query<(&RiverName, &mut Node, &mut Visibility, &ComputedNode, &Children)>,
    texts: Query<&Text>,
) {
    let hide = |vis: &mut Visibility| {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
    };
    let camera = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active);
    let Some((camera, transform, _)) = camera else {
        for (_, _, mut vis, _, _) in &mut names {
            hide(&mut vis);
        }
        return;
    };
    let cam_global = GlobalTransform::from(*transform);
    // Point d'un emplacement : centre de la case, ou milieu de l'arête entre deux cases
    let world = |(a, b): (Hex, Hex)| {
        // (b pris du même côté du raccord que a)
        let b = map.0.wrap().nearest(a, b);
        let p = if mode.is_3d {
            let h = shown_height(&map, &fog, a).max(shown_height(&map, &fog, b)) + 4.0;
            (to_3d(a, h) + to_3d(b, h)) / 2.0
        } else {
            let ((xa, ya), (xb, yb)) = (a.to_world(), b.to_world());
            Vec3::new((xa + xb) / 2.0, (ya + yb) / 2.0, 0.0)
        };
        near_camera(&map, p, transform.translation.x)
    };
    // Bannières de villes visibles : déjà placées, elles ont la priorité
    let mut taken: Vec<ScreenRect> = banners
        .iter()
        .filter(|(_, _, vis)| **vis != Visibility::Hidden)
        .filter_map(|(node, computed, _)| {
            let (Val::Px(x), Val::Px(y)) = (node.left, node.top) else { return None };
            let size = logical_size(computed, Vec2::new(90.0, 22.0));
            // Ancrée au milieu de son bord inférieur
            Some(ScreenRect { min: Vec2::new(x - size.x / 2.0, y - size.y), size })
        })
        .collect();
    let mut order: Vec<_> = names.iter_mut().collect();
    order.sort_by_key(|(river, ..)| river.rank);
    for (river, mut node, mut vis, computed, children) in order {
        let enabled = match river.feature {
            Feature::River(_) => filter.rivers,
            Feature::Lake(_) => filter.lakes,
        };
        if !enabled {
            hide(&mut vis);
            continue;
        }
        let chars = children.first().and_then(|c| texts.get(*c).ok()).map_or(8, |t| t.0.chars().count());
        let (font, _, min_px) = NAME_STYLES[river.tier as usize];
        let size = logical_size(computed, Vec2::new(chars as f32 * font * 0.6 + 12.0, font + 4.0));
        // Emplacements explorés du cours, du plus proche du milieu au plus lointain
        let mid = river.spots.len() as i32 / 2;
        let explored = |&(a, b): &(Hex, Hex)| fog.explored.contains(&map.0.canonical(a)) || fog.explored.contains(&map.0.canonical(b));
        let mut candidates: Vec<(i32, (Hex, Hex))> = river
            .spots
            .iter()
            .enumerate()
            .filter(|(_, s)| explored(s))
            .map(|(i, &s)| (if river.ordered { i as i32 } else { (i as i32 - mid).abs() }, s))
            .collect();
        candidates.sort_by_key(|(d, _)| *d);
        let mut placed = None;
        for (_, spot) in candidates {
            let center = world(spot);
            let Ok(p) = camera.world_to_viewport(&cam_global, center) else { continue };
            if min_px > 0.0 {
                // Taille d'une case à l'écran : de loin, les cours d'eau secondaires se taisent
                let edge = camera.world_to_viewport(&cam_global, center + transform.right() * crate::hex::HEX_SIZE).ok();
                if edge.is_none_or(|e| e.distance(p) < min_px) {
                    break;
                }
            }
            let rect = ScreenRect { min: p - size / 2.0, size };
            if !taken.iter().any(|t| t.overlaps(&rect)) {
                placed = Some((p, rect));
                break;
            }
        }
        let Some((p, rect)) = placed else {
            hide(&mut vis);
            continue;
        };
        taken.push(rect);
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

/// Cours d'eau ou lac dont l'étiquette est sous la souris (les étiquettes réagissent toujours à
/// la souris : un clic y amène la caméra ; leur survol ne met en surbrillance que si l'option le
/// permet).
fn update_hovered_label(mode: Res<HighlightMode>, names: Query<(&RiverName, &UiPointer, &Visibility)>, mut hovered: ResMut<HoveredLabel>) {
    let wanted = names
        .iter()
        .find(|(_, i, v)| **i != UiPointer::None && **v != Visibility::Hidden)
        .map(|(n, _, _)| n.feature)
        .filter(|_| *mode != HighlightMode::Off);
    if hovered.0 != wanted {
        hovered.0 = wanted;
    }
}

/// Copie d'un point du monde la plus proche de la caméra (planète : un tour près) : une
/// étiquette vue au-delà du raccord s'affiche de ce côté-là.
pub(crate) fn near_camera(map: &GameMap, p: Vec3, camera_x: f32) -> Vec3 {
    let world = map.0.wrap().world_width();
    if world <= 0.0 {
        return p;
    }
    Vec3::new(p.x + ((camera_x - p.x) / world).round() * world, p.y, p.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lettres_le_long_d_une_ligne() {
        let widths = [10.0; 5];
        // Ligne droite de 200 px : lettres centrées, alignées, à plat
        let flat = curve_letters(&[Vec2::new(0.0, 50.0), Vec2::new(200.0, 50.0)], &widths, 2.0).expect("assez de place");
        assert_eq!(flat.len(), 5);
        assert!(flat.windows(2).all(|w| w[1].0.x > w[0].0.x), "de gauche à droite");
        assert!(flat.iter().all(|(p, a)| (p.y - 50.0).abs() < 0.01 && a.abs() < 0.01));
        let middle = (flat[0].0.x + flat[4].0.x) / 2.0;
        assert!((middle - 100.0).abs() < 0.5, "centré : {middle}");
        // Trop courte pour le nom
        assert!(curve_letters(&[Vec2::new(0.0, 0.0), Vec2::new(50.0, 0.0)], &widths, 2.0).is_none());
        // Tracée de droite à gauche : lue quand même de gauche à droite, jamais à l'envers
        let back = curve_letters(&[Vec2::new(200.0, 0.0), Vec2::new(0.0, 0.0)], &widths, 2.0).unwrap();
        assert!(back[0].0.x < back[4].0.x && back.iter().all(|(_, a)| a.abs() < 0.01));
        // En pente douce : les lettres suivent la pente
        let slope = curve_letters(&[Vec2::new(0.0, 0.0), Vec2::new(200.0, 100.0)], &widths, 2.0).unwrap();
        assert!(slope.iter().all(|(_, a)| (a - 0.5f32.atan()).abs() < 0.01));
        // Un coude à angle droit sous le nom : il reste droit
        let elbow = [Vec2::new(0.0, 0.0), Vec2::new(60.0, 0.0), Vec2::new(60.0, 60.0)];
        assert!(curve_letters(&elbow, &widths, 2.0).is_none());
        // Presque verticale : il reste droit
        assert!(curve_letters(&[Vec2::new(0.0, 0.0), Vec2::new(20.0, 200.0)], &widths, 2.0).is_none());
    }
}
