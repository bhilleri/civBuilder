//! Étiquettes à l'écran ancrées sur la carte : bannières des villes (2D et 3D), noms des
//! fleuves et numéros de tour le long des trajets (3D ; la 2D a ses propres pastilles).
//!
//! Ce sont des nœuds d'interface placés chaque image à la projection de leur case par la
//! caméra active : ils restent lisibles quel que soit le zoom ou l'inclinaison.
//! Les barres de PV des unités en 3D suivent le même principe (la 2D dessine les siennes).

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
    /// Montrée seulement si sa case est explorée et assez grande à l'écran (rayon en pixels).
    explored_only: bool,
    min_px: f32,
}

impl Anchor {
    fn new(hex: Hex, up_2d: f32, up_3d: f32, only_3d: bool) -> Self {
        Anchor { hex, up_2d, up_3d, only_3d, hidden: false, under_token: false, explored_only: false, min_px: 0.0 }
    }
}

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
    pub cities: bool,
}

impl Default for LabelFilter {
    fn default() -> Self {
        Self { rivers: true, lakes: true, cities: true }
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
                spawn_site_labels,
                despawn_site_labels,
            ),
        )
        .init_resource::<LabelFilter>()
        .add_systems(Update, update_hovered_label)
        .add_systems(
            PostUpdate,
            (place_labels, place_river_names).chain().after(CameraUpdateSystems).before(UiSystems::Prepare),
        );
    }
}

fn spawn_city_banners(mut commands: Commands, cities: Query<(Entity, &City, &HexPos), Added<City>>) {
    for (entity, city, pos) in &cities {
        commands
            .spawn((
                CityBanner(entity),
                // Survol : le territoire de la ville ressort ; clic : la caméra y va
                (TargetButton::new(MapTarget::City(entity)), Interaction::default()),
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
                        Text::new(city.state.population.to_string()),
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

/// Nom des villages et des camps (cliquable : la caméra y va), une fois leur case explorée.
fn spawn_site_labels(mut commands: Commands, sites: Query<(Entity, &crate::hostile::SiteName, &HexPos, Has<crate::hostile::Camp>), Added<crate::hostile::SiteName>>) {
    for (entity, name, pos, camp) in &sites {
        commands.spawn((
            SiteLabel(entity),
            (TargetButton::new(MapTarget::Site(entity)), Interaction::default()),
            Anchor { explored_only: true, min_px: SITE_LABEL_PX, ..Anchor::new(pos.0, -17.0, 6.0, false) },
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
fn spawn_river_names(mut commands: Commands, map: Res<GameMap>, old: Query<Entity, With<RiverName>>) {
    if !map.is_added() {
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
            (TargetButton::click_only(MapTarget::Lake(i)), Interaction::default()),
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
            (TargetButton::click_only(MapTarget::River(i)), Interaction::default()),
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
                    let wanted = city.state.population.to_string();
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
    units: Query<(&Unit, &HexPos, Has<Hostile>)>,
    cities: Query<&HexPos, With<City>>,
    fog: Res<ShownFog>,
    mut bars: Query<(Entity, &HpBar3d, &mut Anchor, &Children)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), With<HpFill3d>>,
) {
    for (entity, bar, mut anchor, children) in &mut bars {
        let Ok((unit, pos, hostile)) = units.get(bar.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let hp = unit.0.hp.clamp(0, MAX_HP);
        let hidden = hp >= MAX_HP || (hostile && !fog.visible.contains(&pos.0));
        let on_roof = if cities.iter().any(|c| c.0 == pos.0) { CITY_HEIGHT } else { 0.0 };
        let up_3d = TOKEN_LIFT + on_roof;
        if anchor.hex != pos.0 || anchor.hidden != hidden || anchor.up_3d != up_3d {
            anchor.hex = pos.0;
            anchor.hidden = hidden;
            anchor.up_3d = up_3d;
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
    mut labels: Query<(&Anchor, &mut Node, &mut Visibility, Has<CityBanner>), Without<RiverName>>,
) {
    let Some((camera, transform, _)) = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active) else {
        return;
    };
    let cam_global = GlobalTransform::from(*transform);
    // Inclinaison de la caméra : écrase verticalement le disque d'un jeton à l'écran
    let tilt = (-transform.forward().y).clamp(0.2, 1.0);
    for (anchor, mut node, mut vis, is_city) in &mut labels {
        let world = if mode.is_3d {
            to_3d(anchor.hex, shown_height(&map, &fog, anchor.hex) + anchor.up_3d)
        } else {
            let (x, y) = anchor.hex.to_world();
            Vec3::new(x, y + anchor.up_2d, 0.0)
        };
        let world = near_camera(&map, world, transform.translation.x);
        let shown = !anchor.hidden
            && (mode.is_3d || !anchor.only_3d)
            && (filter.cities || !is_city)
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
fn update_hovered_label(mode: Res<HighlightMode>, names: Query<(&RiverName, &Interaction, &Visibility)>, mut hovered: ResMut<HoveredLabel>) {
    let wanted = names
        .iter()
        .find(|(_, i, v)| **i != Interaction::None && **v != Visibility::Hidden)
        .map(|(n, _, _)| n.feature)
        .filter(|_| *mode != HighlightMode::Off);
    if hovered.0 != wanted {
        hovered.0 = wanted;
    }
}

/// Copie d'un point du monde la plus proche de la caméra (planète : un tour près) : une
/// étiquette vue au-delà du raccord s'affiche de ce côté-là.
fn near_camera(map: &GameMap, p: Vec3, camera_x: f32) -> Vec3 {
    let world = map.0.wrap().world_width();
    if world <= 0.0 {
        return p;
    }
    Vec3::new(p.x + ((camera_x - p.x) / world).round() * world, p.y, p.z)
}
