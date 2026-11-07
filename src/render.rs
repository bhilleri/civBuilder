//! Affichage : cases de la carte, brouillard, unités, villes, frontières (gizmos).

use crate::feedback::{HeldHp, Motion};
use crate::game::{City, Fog, GameMap, HexPos, Selection, ShownFog, Territory, Unit};
use crate::pathfinding::reachable;
use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::hostile::{Blocked, Camp, EnemyZoc, Hostile, Occupancy, Village};
use crate::input::{HoverPath, OrderPath};
use crate::map::KnownMap;
use crate::map::{LevelEdge, MAX_LEVEL, Relief, Terrain};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use crate::rules::UnitKind;
use crate::waterways::{WaterKind, WaterShape, hex_water};
use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};

const Z_TILE: f32 = 0.0;
const Z_HILLS: f32 = 0.3;
/// Hachures des cases partagées entre deux bassins (carte des bassins versants).
const Z_HATCH: f32 = 0.35;
const Z_WATER: f32 = 0.4;
const Z_BANK: f32 = 0.45;
const Z_SLOPE: f32 = 0.5;
const Z_REACH: f32 = 1.0;
const Z_CITY: f32 = 2.0;
const Z_SITE: f32 = 2.5;
const Z_UNIT: f32 = 3.0;
const Z_LABEL: f32 = 5.0;

const PLAYER_COLOR: Color = Color::srgb(0.20, 0.45, 0.95);
const HOSTILE_COLOR: Color = Color::srgb(0.78, 0.17, 0.15);
const BORDER_COLOR: Color = Color::srgb(0.45, 0.70, 1.0);

#[derive(Component)]
pub struct Tile(pub Hex);

/// Surbrillance d'une case atteignable ce tour-ci par l'unité sélectionnée.
#[derive(Component)]
struct ReachOverlay;

/// Zone de déplacement du tour : cases atteignables + case de l'unité.
#[derive(Resource, Default)]
pub struct ReachArea {
    pub cells: HashSet<Hex>,
    /// Ennemis visibles attaquables ce tour-ci par l'unité sélectionnée.
    pub attackable: Vec<Hex>,
}

/// Barre de PV sous un jeton d'unité (fond et remplissage).
#[derive(Component)]
struct HpBar;
#[derive(Component)]
struct HpFill;

const HP_BAR_WIDTH: f32 = 26.0;
const ATTACK_RING: Color = Color::srgb(1.0, 0.25, 0.2);

/// Groupe de gizmos dédié au contour de la zone de déplacement (trait plus épais).
#[derive(Default, Reflect, GizmoConfigGroup)]
struct ReachGizmos;

const REACH_OUTLINE: Color = Color::srgb(1.0, 0.97, 0.75);

/// Groupe de gizmos des frontières de plaques (trait épais).
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct PlateGizmos;

/// Flèches des mouvements de plaques et points chauds.
pub const PLATE_ARROW: Color = Color::srgba(1.0, 1.0, 1.0, 0.85);
pub const HOTSPOT: Color = Color::srgb(1.0, 0.55, 0.1);

/// Pastille « nombre de tours » affichée sur le chemin prévisualisé.
#[derive(Component)]
struct TurnLabel;

/// Stade et fortification affichés par le symbole d'une ville.
#[derive(Component)]
struct CitySymbol(u8);

/// Pièces du symbole d'une ville : enceinte, tours, toits et grand toit.
struct CityKit2d {
    outer: Handle<Mesh>,
    wall: Handle<Mesh>,
    inner: Handle<Mesh>,
    tower_outline: Handle<Mesh>,
    tower: Handle<Mesh>,
    roof: Handle<Mesh>,
    keep_outline: Handle<Mesh>,
    keep: Handle<Mesh>,
    stone: Handle<ColorMaterial>,
    paving: Handle<ColorMaterial>,
    tiles: Handle<ColorMaterial>,
}

/// Pièces des villages (huttes autour d'un feu) et des camps hostiles (tentes dans une palissade).
struct SiteKit2d {
    ground: Handle<Mesh>,
    ground_outline: Handle<Mesh>,
    hut_outline: Handle<Mesh>,
    hut: Handle<Mesh>,
    hut_top: Handle<Mesh>,
    stake: Handle<Mesh>,
    tent_outline: Handle<Mesh>,
    tent: Handle<Mesh>,
    tent_seam: Handle<Mesh>,
    fire: Handle<Mesh>,
    fire_core: Handle<Mesh>,
    straw: Handle<ColorMaterial>,
    straw_dark: Handle<ColorMaterial>,
    camp_ground: Handle<ColorMaterial>,
    rock: Handle<ColorMaterial>,
    wood: Handle<ColorMaterial>,
    tent_red: Handle<ColorMaterial>,
    fire_color: Handle<ColorMaterial>,
    fire_core_color: Handle<ColorMaterial>,
}

/// Matériaux partagés (une instance par terrain, version claire et version « hors de vue »).
#[derive(Resource)]
struct Palette {
    /// Par terrain et par niveau d'altitude.
    terrain: HashMap<(Terrain, u8), Handle<ColorMaterial>>,
    terrain_dim: HashMap<(Terrain, u8), Handle<ColorMaterial>>,
    /// Cartes de l'altitude et du relief : teintes (voir `altitude_index`, `relief_index`).
    altitude: Vec<Handle<ColorMaterial>>,
    altitude_dim: Vec<Handle<ColorMaterial>>,
    relief: Vec<Handle<ColorMaterial>>,
    relief_dim: Vec<Handle<ColorMaterial>>,
    /// Carte du climat : teintes de précipitations, puis eau profonde et eau côtière.
    climate: Vec<Handle<ColorMaterial>>,
    climate_dim: Vec<Handle<ColorMaterial>>,
    /// Cartes des bassins et des débits : teintes (voir `hydro_index`).
    basins: Vec<Handle<ColorMaterial>>,
    basins_dim: Vec<Handle<ColorMaterial>>,
    flow: Vec<Handle<ColorMaterial>>,
    flow_dim: Vec<Handle<ColorMaterial>>,
    /// Carte des plaques : terre et mer de chaque plaque (voir `plate_index`).
    plates: Vec<Handle<ColorMaterial>>,
    plates_dim: Vec<Handle<ColorMaterial>>,
    /// Carte des courants : teintes de la température de la mer, puis terre et lac (voir
    /// `currents_index`).
    currents: Vec<Handle<ColorMaterial>>,
    currents_dim: Vec<Handle<ColorMaterial>>,
    tides: Vec<Handle<ColorMaterial>>,
    tides_dim: Vec<Handle<ColorMaterial>>,
    waves: Vec<Handle<ColorMaterial>>,
    waves_dim: Vec<Handle<ColorMaterial>>,
    /// Carte géographique (voir `geo_index`).
    geo: Vec<Handle<ColorMaterial>>,
    geo_dim: Vec<Handle<ColorMaterial>>,
    unexplored: Handle<ColorMaterial>,
    /// Banquise, vive et assombrie (toutes les cartes qui la montrent).
    ice: Handle<ColorMaterial>,
    ice_dim: Handle<ColorMaterial>,
    unit: Handle<ColorMaterial>,
    hostile_unit: Handle<ColorMaterial>,
    hp_bg: Handle<ColorMaterial>,
    hp_good: Handle<ColorMaterial>,
    hp_mid: Handle<ColorMaterial>,
    hp_low: Handle<ColorMaterial>,
    hp_mesh: Handle<Mesh>,
    sites: SiteKit2d,
    outline: Handle<ColorMaterial>,
    city: CityKit2d,
    unit_mesh: Handle<Mesh>,
    outline_mesh: Handle<Mesh>,
    label_mesh: Handle<Mesh>,
    label_bg: Handle<ColorMaterial>,
    order_label_bg: Handle<ColorMaterial>,
    reach: Handle<ColorMaterial>,
    tile_mesh: Handle<Mesh>,
    icons: HashMap<UnitKind, Handle<Image>>,
}

/// Carte affichée : normale (terrains), topographique (relief : couleur par niveau d'altitude),
/// climatique (précipitations et vents dominants), des bassins versants ou des débits.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapLayer {
    #[default]
    Normal,
    /// Altitude (m), teintes continues.
    Altitude,
    /// Relief des cases : plat, vallonné, accidenté, montagne, pic.
    Relief,
    Climate,
    Basins,
    Flow,
    Network,
    /// Plaques tectoniques, leurs frontières et leurs mouvements.
    Plates,
    /// Courants marins : température de la mer et sens des courants.
    Currents,
    /// Marées : marnage de vives-eaux des mers.
    Tides,
    /// Houle : hauteur moyenne des mers et sens de la houle.
    Waves,
    /// Ressources : bancs de poissons (une icône par banc, la zone au survol).
    Resources,
    /// Géographie : le découpage du territoire en zones nommées (terres, mers, relief, régions
    /// naturelles), une couleur par zone, à deux niveaux selon le zoom.
    Geography,
}

impl MapLayer {
    pub const ALL: [MapLayer; 13] = [
        MapLayer::Normal,
        MapLayer::Altitude,
        MapLayer::Relief,
        MapLayer::Climate,
        MapLayer::Basins,
        MapLayer::Flow,
        MapLayer::Network,
        MapLayer::Plates,
        MapLayer::Currents,
        MapLayer::Tides,
        MapLayer::Waves,
        MapLayer::Resources,
        MapLayer::Geography,
    ];

    /// Nom court (bouton « Carte »).
    pub fn short_name(self) -> &'static str {
        match self {
            MapLayer::Normal => "normale",
            MapLayer::Altitude => "altitude",
            MapLayer::Relief => "relief",
            MapLayer::Climate => "climat",
            MapLayer::Basins => "bassins versants",
            MapLayer::Flow => "débits",
            MapLayer::Network => "réseau hydrographique",
            MapLayer::Plates => "plaques tectoniques",
            MapLayer::Currents => "courants marins",
            MapLayer::Tides => "marées",
            MapLayer::Waves => "houle",
            MapLayer::Resources => "ressources",
            MapLayer::Geography => "géographie",
        }
    }

    /// La banquise y recouvre la mer : pas sur les plaques ni la géographie, qui montrent ce qu'elle cache.
    pub fn shows_ice(self) -> bool {
        !matches!(self, MapLayer::Plates | MapLayer::Geography)
    }

    /// Cartes de l'eau (bassins, débits, réseau).
    pub fn hydrologic(self) -> bool {
        matches!(self, MapLayer::Basins | MapLayer::Flow | MapLayer::Network)
    }

    /// Carte suivante (touche T).
    pub fn next(self) -> MapLayer {
        let i = MapLayer::ALL.iter().position(|&l| l == self).unwrap_or(0);
        MapLayer::ALL[(i + 1) % MapLayer::ALL.len()]
    }

    /// Les cartes thématiques masquent la végétation 3D, pour lire les couleurs.
    pub fn thematic(self) -> bool {
        !matches!(self, MapLayer::Normal | MapLayer::Resources)
    }
}

/// Carte géographique : couleurs des zones (deux zones voisines n'ont jamais la même), puis la terre
/// et l'eau hors des zones de la famille montrée.
pub const GEO_PALETTE: [(f32, f32, f32); crate::geography::GEO_COLORS] = [
    (0.84, 0.56, 0.44),
    (0.54, 0.72, 0.44),
    (0.44, 0.60, 0.82),
    (0.88, 0.76, 0.44),
    (0.66, 0.50, 0.78),
    (0.42, 0.74, 0.72),
    (0.82, 0.56, 0.70),
    (0.70, 0.70, 0.40),
    (0.54, 0.56, 0.72),
    (0.80, 0.64, 0.50),
];
pub const GEO_LAND: (f32, f32, f32) = (0.30, 0.30, 0.29);
pub const GEO_WATER: (f32, f32, f32) = (0.10, 0.13, 0.20);

/// Couleur d'une teinte de la carte géographique : la couleur de la grande zone, en nuances pour
/// ses parties (plus claire, plus sombre, très claire), assourdie pour le reste de la grande zone.
pub fn geo_color(shade: crate::geography::GeoShade) -> (f32, f32, f32) {
    use crate::geography::GeoShade;
    match shade {
        GeoShade::Zone { color, tone } => {
            let (r, g, b) = GEO_PALETTE[usize::from(color) % GEO_PALETTE.len()];
            let light = |k: f32| (r + (1.0 - r) * k, g + (1.0 - g) * k, b + (1.0 - b) * k);
            match tone {
                0 => (r, g, b),
                1 => light(0.32),
                2 => (r * 0.70, g * 0.70, b * 0.70),
                3 => light(0.58),
                _ => (r * 0.40 + 0.13, g * 0.40 + 0.13, b * 0.40 + 0.13),
            }
        }
        GeoShade::Land => GEO_LAND,
        GeoShade::Water => GEO_WATER,
    }
}

/// Indice d'une teinte de la carte géographique dans la palette 2D : pour chaque couleur, ses
/// nuances et sa teinte sourde, puis terre et eau.
fn geo_index(shade: crate::geography::GeoShade) -> usize {
    use crate::geography::{GeoShade, MUTED};
    let (n, tones) = (GEO_PALETTE.len(), usize::from(MUTED) + 1);
    match shade {
        GeoShade::Zone { color, tone } => usize::from(color) % n * tones + usize::from(tone.min(MUTED)),
        GeoShade::Land => n * tones,
        GeoShade::Water => n * tones + 1,
    }
}

/// Carte géographique : famille montrée (légende) et niveau (selon le zoom).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct GeoView {
    pub family: crate::geography::GeoFamily,
    /// Niveau du dessous : les parties des grandes zones.
    pub deep: bool,
}

/// Teintes de la carte géographique pour la famille et le niveau montrés.
#[derive(Resource, Default)]
pub struct GeoColors(pub crate::geography::GeoMap);

/// Rayon d'une case à l'écran (pixels) au-delà duquel la carte géographique montre les parties des
/// grandes zones, et en deçà duquel elle revient aux grandes zones (un écart pour ne pas
/// clignoter au seuil) : le niveau des golfes, péninsules et massifs dans les noms.
const GEO_DEEP_PX: f32 = 15.0;
const GEO_TOP_PX: f32 = 13.0;

/// Groupe de gizmos des limites des grandes zones (carte géographique, 2D).
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct GeoGizmos;
const GEO_BORDER: Color = Color::srgba(0.05, 0.05, 0.07, 0.85);

/// Teintes de la carte géographique, refaites quand la famille, le niveau ou le monde changent.
fn update_geo_colors(layer: Res<MapLayer>, view: Res<GeoView>, map: Res<GameMap>, mut colors: ResMut<GeoColors>) {
    if *layer != MapLayer::Geography || !(layer.is_changed() || view.is_changed() || map.is_changed()) {
        return;
    }
    colors.0 = crate::geography::compute(&map.0, view.family, view.deep);
}

/// Niveau de la carte géographique selon le zoom : la taille à l'écran d'une case au point visé
/// par la caméra (comme le niveau des noms).
fn update_geo_depth(
    layer: Res<MapLayer>,
    mode: Res<crate::view3d::ViewMode>,
    cam3d: Res<crate::view3d::Cam3d>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut view: ResMut<GeoView>,
) {
    if *layer != MapLayer::Geography {
        return;
    }
    let Some((camera, transform, _)) = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active) else { return };
    let focus = if mode.is_3d { cam3d.target } else { Vec3::new(transform.translation.x, transform.translation.y, 0.0) };
    let Some(px) = crate::labels::view_level(camera, transform, focus) else { return };
    let deep = if view.deep { px >= GEO_TOP_PX } else { px >= GEO_DEEP_PX };
    if view.deep != deep {
        view.deep = deep;
    }
}

fn setup_geo_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<GeoGizmos>();
    config.line.width = 3.0;
}

/// Carte géographique, niveau du dessous : limites des grandes zones en trait sombre (les parties
/// sont en couleur, leur grande zone reste lisible).
#[allow(clippy::too_many_arguments)]
fn draw_geo_borders(
    layer: Res<MapLayer>,
    view: Res<GeoView>,
    colors: Res<GeoColors>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    mode: Res<crate::view3d::ViewMode>,
    relief: Option<Res<crate::view3d::Relief>>,
    mut cache: Local<Vec<[Vec2; 2]>>,
    mut lines: Gizmos<GeoGizmos>,
    mut lines_3d: Gizmos<crate::view3d::Gizmos3d>,
) {
    if *layer != MapLayer::Geography || !view.deep {
        cache.clear();
        return;
    }
    if cache.is_empty() || colors.is_changed() || fog.is_changed() {
        *cache = crate::geography::top_borders(&map.0, &colors.0, |h| fog.explored.contains(&map.0.canonical(h)));
    }
    if mode.is_3d {
        let Some(relief) = relief else { return };
        for [a, b] in cache.iter() {
            let m = a.lerp(*b, 0.5);
            lines_3d.linestrip([a, &m, b].map(|p| relief.on_ground(p.x, p.y, 2.5)), GEO_BORDER);
        }
    } else {
        for [a, b] in cache.iter() {
            lines.line_2d(*a, *b, GEO_BORDER);
        }
    }
}

/// Carte du climat : couleur selon les précipitations annuelles (mm), du plus sec au plus arrosé.
pub const CLIMATE_STOPS: [(f32, (f32, f32, f32)); 5] = [
    (0.0, (0.80, 0.62, 0.38)),
    (250.0, (0.88, 0.80, 0.50)),
    (500.0, (0.58, 0.76, 0.40)),
    (1_000.0, (0.26, 0.60, 0.42)),
    (2_000.0, (0.18, 0.42, 0.66)),
];
pub const CLIMATE_OCEAN: (f32, f32, f32) = (0.09, 0.12, 0.20);
pub const CLIMATE_COAST: (f32, f32, f32) = (0.13, 0.18, 0.28);
/// Nombre de teintes de la carte du climat en 2D.
const CLIMATE_SHADES: usize = 16;
const WIND_ARROW: Color = Color::srgba(1.0, 1.0, 1.0, 0.8);

/// Position (0 à 1) de précipitations annuelles (mm) sur l'échelle de couleurs.
fn rain_position(mm: f32) -> f32 {
    let last = CLIMATE_STOPS.len() - 1;
    let k = CLIMATE_STOPS.iter().position(|s| s.0 >= mm).unwrap_or(last).max(1);
    let (a, b) = (CLIMATE_STOPS[k - 1].0, CLIMATE_STOPS[k].0);
    let t = ((mm - a) / (b - a)).clamp(0.0, 1.0);
    (k as f32 - 1.0 + t) / last as f32
}

/// Couleur à une position (0 à 1) de l'échelle.
fn scale_color(position: f32) -> (f32, f32, f32) {
    let last = CLIMATE_STOPS.len() - 1;
    let x = position.clamp(0.0, 1.0) * last as f32;
    let k = (x.floor() as usize).min(last - 1);
    let t = x - k as f32;
    let (ca, cb) = (CLIMATE_STOPS[k].1, CLIMATE_STOPS[k + 1].1);
    (ca.0 + (cb.0 - ca.0) * t, ca.1 + (cb.1 - ca.1) * t, ca.2 + (cb.2 - ca.2) * t)
}

/// Couleur de la carte du climat pour des précipitations annuelles (mm).
pub fn rain_color(mm: f32) -> (f32, f32, f32) {
    scale_color(rain_position(mm))
}

/// Couleur d'une case sur la carte du climat.
pub fn climate_color(t: Terrain, rain: f32) -> (f32, f32, f32) {
    match t {
        Terrain::Ocean => CLIMATE_OCEAN,
        Terrain::Coast | Terrain::Lake => CLIMATE_COAST,
        _ => rain_color(rain),
    }
}

fn climate_shade(mm: f32) -> usize {
    ((rain_position(mm) * CLIMATE_SHADES as f32) as usize).min(CLIMATE_SHADES - 1)
}

/// Cases portant une flèche de vent (une sur six environ, en quinconce).
pub fn wind_arrow_site(hex: Hex) -> bool {
    let (col, row) = hex.to_offset();
    row % 2 == 0 && (col + row / 2) % 3 == 0
}

/// Flèche de vent d'une case : départ et arrivée (coordonnées monde 2D), ou rien en l'absence de vent.
pub fn wind_arrow(map: &crate::map::WorldMap, hex: Hex) -> Option<(Vec2, Vec2)> {
    let (wx, wy) = map.wind(hex)?;
    let speed = wx.hypot(wy);
    if speed < 0.05 {
        return None;
    }
    let (x, y) = hex.to_world();
    let half = HEX_SIZE * (0.35 + 0.45 * speed.min(1.2)) / 2.0;
    let d = Vec2::new(wx, wy) / speed * half;
    Some((Vec2::new(x, y) - d, Vec2::new(x, y) + d))
}

/// Carte des bassins versants : couleurs vives des grands bassins (ceux d'un fleuve nommé),
/// teintes sobres des autres (petits bassins côtiers, bassins fermés), puis l'eau.
pub const BASIN_VIVID: [(f32, f32, f32); crate::basins::VIVID_COLORS] = [
    (0.91, 0.55, 0.33),
    (0.38, 0.69, 0.44),
    (0.45, 0.60, 0.88),
    (0.88, 0.77, 0.32),
    (0.69, 0.48, 0.80),
    (0.36, 0.74, 0.74),
    (0.88, 0.48, 0.60),
    (0.62, 0.78, 0.32),
    (0.74, 0.54, 0.38),
    (0.56, 0.52, 0.92),
    (0.96, 0.68, 0.50),
    (0.52, 0.82, 0.62),
];
pub const BASIN_MUTED: [(f32, f32, f32); crate::basins::MUTED_COLORS] =
    [(0.60, 0.58, 0.54), (0.70, 0.68, 0.62), (0.64, 0.61, 0.52), (0.55, 0.59, 0.58)];
pub const HYDRO_OCEAN: (f32, f32, f32) = (0.09, 0.12, 0.20);
pub const HYDRO_LAKE: (f32, f32, f32) = (0.24, 0.36, 0.54);
/// Terres de la carte du réseau hydrographique : neutres, pour lire les tracés.
pub const NETWORK_LAND: (f32, f32, f32) = (0.86, 0.84, 0.78);
pub const NETWORK_LAKE: (f32, f32, f32) = (0.62, 0.74, 0.86);
pub const NETWORK_OCEAN: (f32, f32, f32) = (0.40, 0.52, 0.66);

/// Carte des débits : couleur selon le débit qui traverse la case (m³/s, échelle logarithmique).
pub const FLOW_STOPS: [(f32, (f32, f32, f32)); 5] = [
    (10.0, (0.95, 0.93, 0.85)),
    (100.0, (0.78, 0.87, 0.76)),
    (1_000.0, (0.44, 0.72, 0.80)),
    (10_000.0, (0.20, 0.40, 0.74)),
    (50_000.0, (0.32, 0.16, 0.52)),
];
/// Nombre de teintes de la carte des débits en 2D.
const FLOW_SHADES: usize = 20;

/// Position (0 à 1) d'un débit sur l'échelle de la carte des débits.
fn flow_position(q: f32) -> f32 {
    let lq = q.max(1.0).log10();
    let last = FLOW_STOPS.len() - 1;
    let k = FLOW_STOPS.iter().position(|s| s.0.log10() >= lq).unwrap_or(last).max(1);
    let (a, b) = (FLOW_STOPS[k - 1].0.log10(), FLOW_STOPS[k].0.log10());
    let t = ((lq - a) / (b - a)).clamp(0.0, 1.0);
    (k as f32 - 1.0 + t) / last as f32
}

fn flow_scale_color(position: f32) -> (f32, f32, f32) {
    let last = FLOW_STOPS.len() - 1;
    let x = position.clamp(0.0, 1.0) * last as f32;
    let k = (x.floor() as usize).min(last - 1);
    let t = x - k as f32;
    let (ca, cb) = (FLOW_STOPS[k].1, FLOW_STOPS[k + 1].1);
    (ca.0 + (cb.0 - ca.0) * t, ca.1 + (cb.1 - ca.1) * t, ca.2 + (cb.2 - ca.2) * t)
}

/// Couleur d'un débit (m³/s) sur la carte des débits.
pub fn flow_color(q: f32) -> (f32, f32, f32) {
    flow_scale_color(flow_position(q))
}

/// Teinte (indice) d'une case sur une carte de l'eau : pour les bassins, la couleur du bassin
/// (vives puis sobres) ; pour les débits, la teinte du débit ; puis eau profonde et lac.
fn hydro_index(map: &crate::map::WorldMap, hex: Hex, layer: MapLayer) -> usize {
    if layer == MapLayer::Network {
        // Teintes ajoutées après celles des débits : terre, lac, mer
        return match map.get(hex) {
            Some(t) if t.is_sea() => FLOW_SHADES + 4,
            Some(Terrain::Lake) => FLOW_SHADES + 3,
            _ => FLOW_SHADES + 2,
        };
    }
    let shades = if layer == MapLayer::Basins { BASIN_VIVID.len() + BASIN_MUTED.len() } else { FLOW_SHADES };
    match map.get(hex) {
        Some(t) if t.is_sea() => shades,
        Some(Terrain::Lake) => shades + 1,
        _ if layer == MapLayer::Basins => map.basin(hex).map_or(BASIN_VIVID.len(), |b| b.color),
        _ => ((flow_position(map.discharge(hex)) * FLOW_SHADES as f32) as usize).min(FLOW_SHADES - 1),
    }
}

/// Couleur d'une case sur une carte de l'eau (bassins ou débits).
pub fn hydro_color(map: &crate::map::WorldMap, hex: Hex, layer: MapLayer) -> (f32, f32, f32) {
    if layer == MapLayer::Network {
        return match map.get(hex) {
            Some(t) if t.is_sea() => NETWORK_OCEAN,
            Some(Terrain::Lake) => NETWORK_LAKE,
            _ => NETWORK_LAND,
        };
    }
    match map.get(hex) {
        Some(t) if t.is_sea() => HYDRO_OCEAN,
        Some(Terrain::Lake) => HYDRO_LAKE,
        _ if layer == MapLayer::Basins => map.basin(hex).map_or(BASIN_MUTED[0], |b| basin_color(b.color)),
        _ => flow_color(map.discharge(hex)),
    }
}

/// Couleur d'un indice de couleur de bassin.
pub fn basin_color(index: usize) -> (f32, f32, f32) {
    BASIN_VIVID.get(index).copied().unwrap_or_else(|| BASIN_MUTED[(index - BASIN_VIVID.len()) % BASIN_MUTED.len()])
}

/// Carte des plaques : teintes chaudes des plaques continentales, froides des plaques océaniques
/// (attribuées dans l'ordre des plaques de chaque nature) ; la mer est assombrie pour garder
/// le dessin des côtes.
pub const PLATE_CONTINENTAL: [(f32, f32, f32); 8] = [
    (0.84, 0.68, 0.42),
    (0.62, 0.72, 0.38),
    (0.88, 0.52, 0.36),
    (0.76, 0.60, 0.80),
    (0.92, 0.80, 0.44),
    (0.62, 0.52, 0.38),
    (0.84, 0.50, 0.60),
    (0.50, 0.68, 0.50),
];
pub const PLATE_OCEANIC: [(f32, f32, f32); 8] = [
    (0.22, 0.42, 0.72),
    (0.26, 0.62, 0.74),
    (0.36, 0.34, 0.70),
    (0.20, 0.54, 0.56),
    (0.42, 0.54, 0.86),
    (0.18, 0.32, 0.54),
    (0.46, 0.68, 0.84),
    (0.30, 0.46, 0.62),
];
/// Assombrissement de la mer sur la carte des plaques.
const PLATE_SEA: f32 = 0.62;

/// Couleur de chaque plaque.
pub fn plate_colors(plates: &crate::plates::Plates) -> Vec<(f32, f32, f32)> {
    use crate::plates::PlateKind;
    let (mut c, mut o) = (0, 0);
    plates
        .plates
        .iter()
        .map(|p| match p.kind {
            PlateKind::Continental => {
                c += 1;
                PLATE_CONTINENTAL[(c - 1) % PLATE_CONTINENTAL.len()]
            }
            PlateKind::Oceanic => {
                o += 1;
                PLATE_OCEANIC[(o - 1) % PLATE_OCEANIC.len()]
            }
        })
        .collect()
}

/// Couleurs de la carte des plaques, par paire (terre, mer) de chaque plaque.
fn plate_shades(plates: &crate::plates::Plates) -> Vec<(f32, f32, f32)> {
    plate_colors(plates).into_iter().flat_map(|(r, g, b)| [(r, g, b), (r * PLATE_SEA, g * PLATE_SEA, b * PLATE_SEA)]).collect()
}

/// Teinte (indice) d'une case sur la carte des plaques.
fn plate_index(map: &crate::map::WorldMap, hex: Hex) -> usize {
    let p = map.contains(hex).then(|| map.plates().plate_of.get(map.cell(hex)).copied()).flatten().map_or(0, usize::from);
    2 * p + usize::from(map.get(hex).is_some_and(|t| t.is_water()))
}

/// Couleur d'une case sur la carte des plaques (3D).
pub fn plate_color(map: &crate::map::WorldMap, hex: Hex) -> (f32, f32, f32) {
    plate_shades(map.plates()).get(plate_index(map, hex)).copied().unwrap_or((0.5, 0.5, 0.5))
}

/// Flèche du mouvement de la plaque d'une case (cases en quinconce comme les vents) : départ et
/// arrivée (monde 2D), longueur selon la vitesse.
pub fn plate_arrow(map: &crate::map::WorldMap, hex: Hex) -> Option<(Vec2, Vec2)> {
    let velocity = map.plate_velocity(hex)?;
    let speed = velocity.length();
    if speed < 0.1 {
        return None;
    }
    let (x, y) = hex.to_world();
    let half = HEX_SIZE * (0.3 + 0.07 * speed.min(9.0)) / 2.0;
    let d = velocity / speed * half;
    Some((Vec2::new(x, y) - d, Vec2::new(x, y) + d))
}

/// Carte des courants marins (sRGB) : teintes de la mer selon sa température (°C), terre et lacs
/// en gris (sans relief : la mer seule compte).
pub const SEA_TEMPERATURE_STOPS: [(f32, (f32, f32, f32)); 5] = [
    (-2.0, (0.10, 0.13, 0.34)),
    (8.0, (0.15, 0.30, 0.52)),
    (16.0, (0.22, 0.48, 0.62)),
    (22.0, (0.48, 0.48, 0.62)),
    (28.0, (0.64, 0.36, 0.44)),
];
pub const CURRENTS_LAND: (f32, f32, f32) = (0.36, 0.36, 0.34);
pub const CURRENTS_LAKE: (f32, f32, f32) = (0.30, 0.34, 0.40);
/// Nombre de teintes de la mer en 2D.
const SEA_SHADES: usize = 16;
/// Flèches des courants : eau nettement plus chaude (rouge) ou plus froide (bleu clair) que celle
/// de la latitude, d'au moins `CURRENT_ANOMALY_SHOWN` °C ; les autres en blanc.
pub const WARM_CURRENT: Color = Color::srgba(1.0, 0.42, 0.30, 0.95);
pub const COLD_CURRENT: Color = Color::srgba(0.55, 0.88, 1.0, 0.95);
pub const MILD_CURRENT: Color = Color::srgba(1.0, 1.0, 1.0, 0.8);
const CURRENT_ANOMALY_SHOWN: f32 = 1.5;

/// Couleur de la mer pour une température de surface (°C).
pub fn sea_temperature_color(t: f32) -> (f32, f32, f32) {
    let last = SEA_TEMPERATURE_STOPS.len() - 1;
    let t = t.clamp(SEA_TEMPERATURE_STOPS[0].0, SEA_TEMPERATURE_STOPS[last].0);
    let k = SEA_TEMPERATURE_STOPS.iter().position(|s| s.0 >= t).unwrap_or(last).max(1);
    let ((a, ca), (b, cb)) = (SEA_TEMPERATURE_STOPS[k - 1], SEA_TEMPERATURE_STOPS[k]);
    let x = (t - a) / (b - a);
    (ca.0 + (cb.0 - ca.0) * x, ca.1 + (cb.1 - ca.1) * x, ca.2 + (cb.2 - ca.2) * x)
}

/// Température (°C) au milieu d'une teinte de la mer en 2D.
fn sea_shade_degrees(index: usize) -> f32 {
    let (lo, hi) = (SEA_TEMPERATURE_STOPS[0].0, SEA_TEMPERATURE_STOPS[SEA_TEMPERATURE_STOPS.len() - 1].0);
    lo + (index as f32 + 0.5) / SEA_SHADES as f32 * (hi - lo)
}

/// Teinte (indice) d'une case sur la carte des courants : teintes de la mer, puis terre et lac.
fn currents_index(map: &crate::map::WorldMap, hex: Hex) -> usize {
    match (map.get(hex), map.sea_temperature(hex)) {
        (Some(Terrain::Lake), _) => SEA_SHADES + 1,
        (_, Some((t, _))) => {
            let (lo, hi) = (SEA_TEMPERATURE_STOPS[0].0, SEA_TEMPERATURE_STOPS[SEA_TEMPERATURE_STOPS.len() - 1].0);
            (((t - lo) / (hi - lo) * SEA_SHADES as f32).max(0.0) as usize).min(SEA_SHADES - 1)
        }
        _ => SEA_SHADES,
    }
}

/// Carte des marées : bornes des classes de marnage de vives-eaux (m) et leurs couleurs (sRGB), de
/// l'eau presque sans marée aux plus fortes marées.
pub const TIDE_CLASSES: [(f32, (f32, f32, f32)); 6] = [
    (0.0, (0.86, 0.93, 0.96)),
    (0.5, (0.62, 0.82, 0.90)),
    (1.0, (0.36, 0.64, 0.82)),
    (2.0, (0.30, 0.42, 0.72)),
    (4.0, (0.46, 0.27, 0.62)),
    (8.0, (0.66, 0.16, 0.42)),
];

/// Carte des marées : lignes d'égale heure de pleine mer et flèches du sens de la marée affichées
/// (boutons de la légende de cette carte, éteints au départ).
#[derive(Resource, Default)]
pub struct TideLines {
    pub lines: bool,
    pub arrows: bool,
}

/// Couleur des flèches du sens de la marée.
pub const TIDE_ARROW: Color = Color::srgb(0.98, 0.98, 1.0);

/// Flèches du sens de la marée des cases explorées (voir `tides::tide_arrows`).
pub fn tide_arrows(map: &crate::map::WorldMap, fog: &ShownFog) -> Vec<(Vec2, Vec2)> {
    let sea: Vec<bool> = (0..(map.width * map.height) as usize).map(|i| map.get(map.hex_at(i)).is_some_and(|t| t.is_sea())).collect();
    let t = map.tides();
    crate::tides::tide_arrows(map.width, map.height, map.wrap().wraps(), &t.high_water_h, &t.amplitude, &sea, HEX_SIZE * 1.1, |i| fog.explored.contains(&map.hex_at(i)))
}

/// Couleur de la ligne d'une heure de pleine mer : les heures font le tour du cercle des teintes
/// (le cycle de la marée).
pub fn tide_hour_color(hour: u8) -> Color {
    Color::hsl(hour as f32 / crate::tides::M2_PERIOD_H * 360.0, 0.85, 0.35)
}

/// Lignes d'heure de pleine mer et points amphidromiques des cases explorées (voir
/// `tides::cotidal_lines`).
pub fn tide_lines(map: &crate::map::WorldMap, fog: &ShownFog) -> (Vec<(u8, Vec2, Vec2)>, Vec<Vec2>) {
    let sea: Vec<bool> = (0..(map.width * map.height) as usize).map(|i| map.get(map.hex_at(i)).is_some_and(|t| t.is_sea())).collect();
    crate::tides::cotidal_lines(map.width, map.height, map.wrap().wraps(), &map.tides().high_water_h, &sea, |i| fog.explored.contains(&map.hex_at(i)))
}

/// Lignes d'heure et flèches gardées d'une image à l'autre (refaites quand l'exploration change).
#[derive(Default)]
struct TideLinesCache {
    lines: Option<(Vec<(u8, Vec2, Vec2)>, Vec<Vec2>)>,
    arrows: Option<Vec<(Vec2, Vec2)>>,
}

/// Carte des marées : lignes d'égale heure de pleine mer, points amphidromiques et sens de la
/// marée (en 2D).
fn draw_tide_lines_2d(layer: Res<MapLayer>, shown: Res<TideLines>, map: Res<GameMap>, fog: Res<ShownFog>, view: Res<crate::view3d::ViewMode>, mut cache: Local<TideLinesCache>, mut gizmos: Gizmos) {
    if fog.is_changed() {
        (cache.lines, cache.arrows) = (None, None);
    }
    if *layer != MapLayer::Tides || view.is_3d {
        return;
    }
    if shown.lines {
        let (lines, amphidromes) = cache.lines.get_or_insert_with(|| tide_lines(&map.0, &fog));
        for &(hour, a, b) in lines.iter() {
            gizmos.line_2d(a, b, tide_hour_color(hour));
        }
        for &p in amphidromes.iter() {
            gizmos.circle_2d(p, 7.0, Color::WHITE);
            gizmos.circle_2d(p, 3.0, Color::BLACK);
        }
    }
    if shown.arrows {
        for &(a, b) in cache.arrows.get_or_insert_with(|| tide_arrows(&map.0, &fog)).iter() {
            gizmos.arrow_2d(a, b, TIDE_ARROW).with_tip_length(7.0);
        }
    }
}

/// Classe de marnage (indice dans `TIDE_CLASSES`) d'une case de mer.
fn tide_class(range: f32) -> usize {
    TIDE_CLASSES.iter().rposition(|&(from, _)| range >= from).unwrap_or(0)
}

/// Teinte (indice) d'une case sur la carte des marées : classes de marnage, puis terre et lac.
fn tides_index(map: &crate::map::WorldMap, hex: Hex) -> usize {
    match (map.get(hex), map.tidal_range(hex)) {
        (Some(Terrain::Lake), _) => TIDE_CLASSES.len() + 1,
        (_, Some(r)) => tide_class(r),
        _ => TIDE_CLASSES.len(),
    }
}

/// Couleur d'une case sur la carte des marées (3D).
pub fn tides_color(map: &crate::map::WorldMap, hex: Hex) -> (f32, f32, f32) {
    match (map.get(hex), map.tidal_range(hex)) {
        (Some(Terrain::Lake), _) => CURRENTS_LAKE,
        (_, Some(r)) => TIDE_CLASSES[tide_class(r)].1,
        _ => CURRENTS_LAND,
    }
}

/// Carte de la houle : bornes des classes de hauteur moyenne (m) et leurs couleurs (sRGB), du calme
/// aux mers les plus agitées.
pub const WAVE_CLASSES: [(f32, (f32, f32, f32)); 6] = [
    (0.0, (0.82, 0.91, 0.78)),
    (0.5, (0.58, 0.82, 0.70)),
    (1.0, (0.30, 0.69, 0.68)),
    (1.5, (0.18, 0.50, 0.64)),
    (2.5, (0.20, 0.30, 0.58)),
    (4.0, (0.31, 0.13, 0.45)),
];
/// Couleur de la banquise (bleutée : à ne pas confondre avec la neige, et les flèches blanches y
/// restent lisibles), et assombrissement hors de vue.
pub const ICE_COLOR: (f32, f32, f32) = (0.80, 0.87, 0.93);
const ICE_DIM: f32 = 0.55;
/// Flèches de la houle : sombres sur les classes claires, blanches sur les foncées.
const WAVE_ARROW_DARK: Color = Color::srgb(0.03, 0.10, 0.16);
const WAVE_ARROW_LIGHT: Color = Color::srgb(0.98, 0.98, 1.0);
/// Pas de flèche sous cette hauteur (m) ni sans direction nette (part de l'énergie à ± 45° de la
/// direction dominante : 0,25 si elle arrive de partout).
const WAVE_ARROW_MIN_HEIGHT: f32 = 0.3;
const WAVE_ARROW_MIN_FOCUS: f32 = crate::waves::CLEAR_DIRECTION;

/// Classe de hauteur (indice dans `WAVE_CLASSES`) d'une case de mer.
fn wave_class(height: f32) -> usize {
    WAVE_CLASSES.iter().rposition(|&(from, _)| height >= from).unwrap_or(0)
}

/// Teinte (indice) d'une case sur la carte de la houle : classes, terre, lac.
fn waves_index(map: &crate::map::WorldMap, hex: Hex) -> usize {
    match (map.get(hex), map.wave_cell(hex)) {
        (Some(Terrain::Lake), _) => WAVE_CLASSES.len() + 1,
        (_, Some(i)) => wave_class(map.waves().height[i]),
        _ => WAVE_CLASSES.len(),
    }
}

/// Couleur d'une case sur la carte de la houle (3D).
pub fn waves_color(map: &crate::map::WorldMap, hex: Hex) -> (f32, f32, f32) {
    match (map.get(hex), map.wave_cell(hex)) {
        (Some(Terrain::Lake), _) => CURRENTS_LAKE,
        (_, Some(i)) => WAVE_CLASSES[wave_class(map.waves().height[i])].1,
        _ => CURRENTS_LAND,
    }
}

/// Flèche de la houle d'une case de mer (cases en quinconce comme les vents), vers où elle va :
/// départ, arrivée (monde 2D), plus longue pour une longue période, couleur lisible sur la classe.
pub fn wave_arrow(map: &crate::map::WorldMap, hex: Hex) -> Option<(Vec2, Vec2, Color)> {
    let i = map.wave_cell(hex)?;
    let w = map.waves();
    if map.is_ice(hex) || w.height[i] < WAVE_ARROW_MIN_HEIGHT || w.focus[i] < WAVE_ARROW_MIN_FOCUS {
        return None;
    }
    let (x, y) = hex.to_world();
    let half = HEX_SIZE * (0.35 + 0.55 * ((w.period[i] - 8.0) / 6.0).clamp(0.0, 1.0)) / 2.0;
    let d = Vec2::from_angle(w.direction[i]) * half;
    let color = if wave_class(w.height[i]) <= 2 { WAVE_ARROW_DARK } else { WAVE_ARROW_LIGHT };
    Some((Vec2::new(x, y) - d, Vec2::new(x, y) + d, color))
}

/// Couleur d'une case sur la carte des courants (3D).
pub fn currents_color(map: &crate::map::WorldMap, hex: Hex) -> (f32, f32, f32) {
    match (map.get(hex), map.sea_temperature(hex)) {
        (Some(Terrain::Lake), _) => CURRENTS_LAKE,
        (_, Some((t, _))) => sea_temperature_color(t),
        _ => CURRENTS_LAND,
    }
}

/// Flèche du courant d'une case de mer (cases en quinconce comme les vents) : départ, arrivée
/// (monde 2D), longueur selon la vitesse, couleur selon l'eau qu'il porte (chaude, froide).
pub fn current_arrow(map: &crate::map::WorldMap, hex: Hex) -> Option<(Vec2, Vec2, Color)> {
    let (u, v) = map.current(hex)?;
    let speed = u.hypot(v);
    if speed < 0.03 {
        return None;
    }
    let (x, y) = hex.to_world();
    let half = HEX_SIZE * (0.3 + 0.5 * (speed / 0.5).min(1.6)) / 2.0;
    let d = Vec2::new(u, v) / speed * half;
    let color = match map.sea_temperature(hex).map_or(0.0, |(_, a)| a) {
        a if a >= CURRENT_ANOMALY_SHOWN => WARM_CURRENT,
        a if a <= -CURRENT_ANOMALY_SHOWN => COLD_CURRENT,
        _ => MILD_CURRENT,
    };
    Some((Vec2::new(x, y) - d, Vec2::new(x, y) + d, color))
}

/// Carte de l'altitude (sRGB) : eau, puis teintes hypsométriques selon l'altitude (m).
pub const TOPO_OCEAN: (f32, f32, f32) = (0.12, 0.25, 0.50);
pub const TOPO_COAST: (f32, f32, f32) = (0.35, 0.56, 0.80);
/// Abysses (plus de 3 000 m) sur la carte de l'altitude.
pub const TOPO_ABYSS: (f32, f32, f32) = (0.06, 0.13, 0.32);

pub const ALTITUDE_STOPS: [(f32, (f32, f32, f32)); 7] = [
    (0.0, (0.28, 0.55, 0.30)),
    (300.0, (0.52, 0.71, 0.38)),
    (800.0, (0.84, 0.81, 0.46)),
    (1_500.0, (0.84, 0.60, 0.33)),
    (2_400.0, (0.62, 0.43, 0.32)),
    (3_400.0, (0.66, 0.62, 0.60)),
    (4_800.0, (0.97, 0.97, 0.98)),
];
/// Nombre de teintes de la carte de l'altitude en 2D.
const ALTITUDE_SHADES: usize = 24;

/// Couleur d'une altitude (m) sur la carte de l'altitude.
pub fn altitude_color(m: f32) -> (f32, f32, f32) {
    let last = ALTITUDE_STOPS.len() - 1;
    let m = m.clamp(0.0, ALTITUDE_STOPS[last].0);
    let k = ALTITUDE_STOPS.iter().position(|s| s.0 >= m).unwrap_or(last).max(1);
    let ((a, ca), (b, cb)) = (ALTITUDE_STOPS[k - 1], ALTITUDE_STOPS[k]);
    let t = if b > a { (m - a) / (b - a) } else { 1.0 };
    (ca.0 + (cb.0 - ca.0) * t, ca.1 + (cb.1 - ca.1) * t, ca.2 + (cb.2 - ca.2) * t)
}

/// Altitude (m) au milieu d'une teinte de la carte de l'altitude en 2D.
fn altitude_shade_meters(index: usize) -> f32 {
    (index as f32 + 0.5) / ALTITUDE_SHADES as f32 * ALTITUDE_STOPS[ALTITUDE_STOPS.len() - 1].0
}

/// Teinte (indice) d'une case sur la carte de l'altitude : teintes de la terre, puis mer et lac.
fn altitude_index(map: &crate::map::WorldMap, hex: Hex) -> usize {
    match map.get(hex) {
        Some(Terrain::Coast) | Some(Terrain::Lake) => ALTITUDE_SHADES + 1,
        Some(t) if t.is_sea() && map.depth(hex).is_some_and(|d| d >= crate::map::ABYSS_DEPTH_M) => ALTITUDE_SHADES + 2,
        Some(t) if t.is_sea() => ALTITUDE_SHADES,
        _ => {
            let m = map.elevation(hex).unwrap_or(0.0);
            ((m / ALTITUDE_STOPS[ALTITUDE_STOPS.len() - 1].0 * ALTITUDE_SHADES as f32) as usize).min(ALTITUDE_SHADES - 1)
        }
    }
}

/// Teinte (indice) d'une case sur la carte du relief : les 5 reliefs, puis mer et lac.
fn relief_index(map: &crate::map::WorldMap, hex: Hex) -> usize {
    match map.get(hex) {
        Some(t) if t.is_sea() => Relief::ALL.len(),
        Some(Terrain::Lake) => Relief::ALL.len() + 1,
        _ => Relief::ALL.iter().position(|&r| r == map.relief(hex)).unwrap_or(0),
    }
}

/// Couleur d'une case sur la carte de l'altitude ou du relief (3D).
pub fn elevation_layer_color(map: &crate::map::WorldMap, hex: Hex, layer: MapLayer) -> (f32, f32, f32) {
    match map.get(hex) {
        Some(Terrain::Ocean) if layer == MapLayer::Altitude && map.depth(hex).is_some_and(|d| d >= crate::map::ABYSS_DEPTH_M) => TOPO_ABYSS,
        Some(Terrain::Ocean) => TOPO_OCEAN,
        Some(Terrain::Coast | Terrain::Lake) => TOPO_COAST,
        _ if layer == MapLayer::Relief => map.relief(hex).color(),
        _ => altitude_color(map.elevation(hex).unwrap_or(0.0)),
    }
}

/// Teinte selon l'altitude : plus sombre en bas (littoral), plus claire en haut (massif).
pub fn level_tint((r, g, b): (f32, f32, f32), level: u8) -> (f32, f32, f32) {
    if level == 0 {
        return (r, g, b);
    }
    let mid = (1.0 + f32::from(MAX_LEVEL)) / 2.0;
    let f = (f32::from(level) - mid) / (mid - 1.0); // −1 (niveau 1) à +1 (dernier niveau)
    let shade = |c: f32| if f < 0.0 { c * (1.0 + 0.22 * f) } else { c + (1.0 - c) * 0.24 * f };
    (shade(r), shade(g), shade(b))
}

/// Ruptures de pente, dessinées en un seul maillage à couleurs par sommet (reconstruit quand le
/// brouillard change ; une arête n'apparaît que si ses deux cases sont explorées) :
/// - descente praticable (1 ou 2 niveaux) : léger reflet sur la case haute, ombre en dégradé
///   sur la case basse, plus marquée pour 2 niveaux ;
/// - falaise : ombre profonde, arête sombre et dents de hachure tournées vers le bas.
#[derive(Resource)]
struct SlopeOverlay {
    edges: Vec<LevelEdge>,
    mesh: Handle<Mesh>,
}

/// Construit le maillage des ruptures de pente.
#[derive(Default)]
struct SlopeMeshBuilder {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
}

impl SlopeMeshBuilder {
    fn tri(&mut self, pts: [(Vec2, [f32; 4]); 3]) {
        for (p, c) in pts {
            self.positions.push([p.x, p.y, 0.0]);
            self.colors.push(c);
        }
    }

    /// Bande entre l'arête (a, b) et sa version rapprochée du centre `c` (fraction `k`),
    /// couleur `edge` sur l'arête, fondue vers `inner`.
    fn band(&mut self, a: Vec2, b: Vec2, c: Vec2, k: f32, edge: [f32; 4], inner: [f32; 4]) {
        let (a2, b2) = (a + (c - a) * k, b + (c - b) * k);
        self.tri([(a, edge), (b, edge), (b2, inner)]);
        self.tri([(a, edge), (b2, inner), (a2, inner)]);
    }

    fn add(&mut self, e: &LevelEdge) {
        let (a, b) = e.high.edge_corners(e.dir);
        let (a, b) = (Vec2::new(a.0, a.1), Vec2::new(b.0, b.1));
        let center = |h: Hex| {
            let (x, y) = h.to_world();
            Vec2::new(x, y)
        };
        let (high, low) = (center(e.high), center(e.low()));
        let black = |alpha: f32| [0.0, 0.0, 0.0, alpha];
        let white = |alpha: f32| [1.0, 1.0, 1.0, alpha];
        if !e.is_cliff() {
            let (depth, alpha) = if e.diff >= 2 { (0.34, 0.42) } else { (0.24, 0.26) };
            self.band(a, b, high, 0.12, white(0.16), white(0.0));
            self.band(a, b, low, depth, black(alpha), black(0.0));
            return;
        }
        let dark = [0.06, 0.045, 0.035, 0.95];
        self.band(a, b, low, 0.45, black(0.55), black(0.0));
        // Arête sombre (légèrement débordante côté bas)
        let inward = (low - (a + b) / 2.0).normalize() * 2.6;
        self.tri([(a, dark), (b, dark), (b + inward, dark)]);
        self.tri([(a, dark), (b + inward, dark), (a + inward, dark)]);
        // Dents de hachure pointées vers le bas de la falaise
        const TEETH: usize = 5;
        for i in 0..TEETH {
            let t0 = (i as f32 + 0.2) / TEETH as f32;
            let t1 = (i as f32 + 0.8) / TEETH as f32;
            let (p0, p1) = (a.lerp(b, t0), a.lerp(b, t1));
            let apex = (p0 + p1) / 2.0 + (low - (a + b) / 2.0).normalize() * 8.0;
            self.tri([(p0, dark), (p1, dark), (apex, dark)]);
        }
    }

    fn build(mut self) -> Mesh {
        if self.positions.is_empty() {
            // Maillage vide interdit : un triangle dégénéré invisible
            self.tri([(Vec2::ZERO, [0.0; 4]), (Vec2::ZERO, [0.0; 4]), (Vec2::ZERO, [0.0; 4])]);
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
    }
}

fn setup_slope_overlay(
    mut commands: Commands,
    map: Res<GameMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mesh = meshes.add(SlopeMeshBuilder::default().build());
    // Matériau blanc : la couleur vient des sommets
    let material = materials.add(ColorMaterial { alpha_mode: bevy::sprite_render::AlphaMode2d::Blend, ..ColorMaterial::from_color(Color::WHITE) });
    commands.spawn((Mesh2d(mesh.clone()), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, Z_SLOPE)));
    commands.insert_resource(SlopeOverlay { edges: map.0.level_edges(), mesh });
}

fn update_slope_overlay(fog: Res<ShownFog>, map: Res<GameMap>, overlay: Res<SlopeOverlay>, mut meshes: ResMut<Assets<Mesh>>) {
    if !fog.is_changed() {
        return;
    }
    let mut builder = SlopeMeshBuilder::default();
    for e in overlay.edges.iter().filter(|e| fog.explored.contains(&e.high) && fog.explored.contains(&map.0.canonical(e.low()))) {
        builder.add(e);
    }
    if let Some(mut mesh) = meshes.get_mut(&overlay.mesh) {
        *mesh = builder.build();
    }
}

/// Cours d'eau en 2D (rivières, fleuves, petits lacs) : un seul maillage à couleurs par sommet,
/// reconstruit quand le brouillard change.
#[derive(Resource)]
struct WaterOverlay {
    shapes: Vec<(Hex, WaterShape)>,
    mesh: Handle<Mesh>,
}

/// Hachures d'une case : bandes diagonales parallèles (alignées d'une case à l'autre), chacune
/// découpée par l'hexagone et donnée par ses deux bords, échantillonnés en `steps` + 1 points
/// d'un bout à l'autre (pour épouser le relief en 3D).
pub fn hatch_strips(hex: Hex, steps: usize) -> Vec<Vec<(Vec2, Vec2)>> {
    const PERIOD: f32 = 9.0;
    const WIDTH: f32 = 3.2;
    let (cx, cy) = hex.to_world();
    let center = Vec2::new(cx, cy);
    let u = Vec2::new(1.0, 1.0).normalize();
    let n = u.perp();
    // Hexagone un peu rentré, pour ne pas déborder sur les voisines
    let apothem = HEX_SIZE * 0.866 * 0.96;
    let normals: Vec<Vec2> = (0..6).map(|d| Vec2::from_angle((60.0 * d as f32).to_radians())).collect();
    // Intervalle de la droite center + n·t + u·s à l'intérieur de l'hexagone
    let interval = |t: f32| -> Option<(f32, f32)> {
        let (mut lo, mut hi) = (f32::MIN, f32::MAX);
        for nd in &normals {
            let a = u.dot(*nd);
            let b = apothem - n.dot(*nd) * t;
            if a.abs() < 1e-6 {
                if b < 0.0 {
                    return None;
                }
            } else if a > 0.0 {
                hi = hi.min(b / a);
            } else {
                lo = lo.max(b / a);
            }
        }
        (lo < hi).then_some((lo, hi))
    };
    let phase = center.dot(n).rem_euclid(PERIOD);
    let mut strips = Vec::new();
    let mut t = -HEX_SIZE - phase;
    while t < HEX_SIZE {
        if let (Some((a0, a1)), Some((b0, b1))) = (interval(t), interval(t + WIDTH)) {
            let strip = (0..=steps)
                .map(|k| {
                    let f = k as f32 / steps as f32;
                    let p = center + n * t + u * (a0 + (a1 - a0) * f);
                    let q = center + n * (t + WIDTH) + u * (b0 + (b1 - b0) * f);
                    (p, q)
                })
                .collect();
            strips.push(strip);
        }
        t += PERIOD;
    }
    strips
}

/// Surcouche 2D des hachures (carte des bassins versants).
#[derive(Resource)]
struct HatchOverlay {
    mesh: Handle<Mesh>,
}

fn setup_hatch_overlay(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let mesh = meshes.add(SlopeMeshBuilder::default().build());
    let material = materials.add(ColorMaterial::from_color(Color::WHITE));
    commands.spawn((Mesh2d(mesh.clone()), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, Z_HATCH)));
    commands.insert_resource(HatchOverlay { mesh });
}

/// Cases explorées qui desservent deux bassins : hachées de la couleur de l'autre bassin.
fn update_hatch_overlay(fog: Res<ShownFog>, layer: Res<MapLayer>, map: Res<GameMap>, overlay: Res<HatchOverlay>, mut meshes: ResMut<Assets<Mesh>>) {
    if !fog.is_changed() && !layer.is_changed() {
        return;
    }
    let mut builder = SlopeMeshBuilder::default();
    if *layer == MapLayer::Basins {
        for (cell, other) in map.0.basins().basin_also.iter().enumerate() {
            let Some(other) = other else { continue };
            let hex = map.0.hex_at(cell);
            if !fog.explored.contains(&hex) {
                continue;
            }
            let (r, g, b) = basin_color(map.0.basins().basins[*other as usize].color);
            let k = if fog.visible.contains(&hex) { 1.0 } else { 0.5 };
            let c = Color::srgb(r * k, g * k, b * k * 1.1).to_linear();
            let c = [c.red, c.green, c.blue, 1.0];
            for strip in hatch_strips(hex, 1) {
                let ((p0, q0), (p1, q1)) = (strip[0], strip[1]);
                builder.tri([(p0, c), (p1, c), (q1, c)]);
                builder.tri([(p0, c), (q1, c), (q0, c)]);
            }
        }
    }
    if let Some(mut mesh) = meshes.get_mut(&overlay.mesh) {
        *mesh = builder.build();
    }
}

/// Surcouche 2D des bancs de poissons (carte des ressources).
#[derive(Resource)]
struct BankOverlay {
    mesh: Handle<Mesh>,
}

/// Teinte des bancs de poissons (sRGB) : voile sur la zone, liseré plus franc au bord.
const BANK_2D: Color = Color::srgb(0.30, 0.90, 0.85);
const BANK_FILL_ALPHA: f32 = 0.22;
const BANK_EDGE_ALPHA: f32 = 0.85;

fn setup_bank_overlay(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let mesh = meshes.add(SlopeMeshBuilder::default().build());
    let material = materials.add(ColorMaterial { alpha_mode: bevy::sprite_render::AlphaMode2d::Blend, ..ColorMaterial::from_color(Color::WHITE) });
    commands.spawn((Mesh2d(mesh.clone()), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, Z_BANK)));
    commands.insert_resource(BankOverlay { mesh });
}

/// Carte des ressources : zone connue de chaque banc de poissons, voilée et bordée.
fn update_bank_overlay(fog: Res<ShownFog>, layer: Res<MapLayer>, map: Res<GameMap>, overlay: Res<BankOverlay>, mut meshes: ResMut<Assets<Mesh>>) {
    if !fog.is_changed() && !layer.is_changed() && !map.is_changed() {
        return;
    }
    let mut builder = SlopeMeshBuilder::default();
    if *layer == MapLayer::Resources {
        let c = BANK_2D.to_linear();
        let rgba = |a: f32| [c.red, c.green, c.blue, a];
        let known = |h: Hex| fog.explored.contains(&h);
        for (b, bank) in map.0.banks().iter().enumerate() {
            for &cell in &bank.cells {
                let hex = map.0.hex_at(cell);
                if !known(hex) {
                    continue;
                }
                let (x, y) = hex.to_world();
                let center = Vec2::new(x, y);
                for (d, &step) in DIRECTIONS.iter().enumerate() {
                    let (a, e) = hex.edge_corners(d);
                    let (a, e) = (Vec2::new(a.0, a.1), Vec2::new(e.0, e.1));
                    builder.tri([(center, rgba(BANK_FILL_ALPHA)), (a, rgba(BANK_FILL_ALPHA)), (e, rgba(BANK_FILL_ALPHA))]);
                    // Bord du banc (ou de la partie connue) : liseré fondu vers l'intérieur
                    let n = map.0.canonical(hex + step);
                    let inside = map.0.contains(n) && known(n) && map.0.bank(n) == Some(b);
                    if !inside {
                        builder.band(a, e, center, 0.16, rgba(BANK_EDGE_ALPHA), rgba(0.0));
                    }
                }
            }
        }
    }
    if let Some(mut mesh) = meshes.get_mut(&overlay.mesh) {
        *mesh = builder.build();
    }
}

/// Couleurs (sRGB) de l'eau douce en 2D.
const RIVER_2D: Color = Color::srgb(0.20, 0.45, 0.80);
const FLEUVE_2D: Color = Color::srgb(0.18, 0.40, 0.72);
const SMALL_LAKE_2D: Color = Color::srgb(0.24, 0.50, 0.78);

fn setup_water_overlay(
    mut commands: Commands,
    map: Res<GameMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let shapes: Vec<(Hex, WaterShape)> =
        map.0.hexes().map(|h| (h, hex_water(&map.0, h))).filter(|(_, s)| !s.is_empty()).collect();
    let mesh = meshes.add(SlopeMeshBuilder::default().build());
    let material = materials.add(ColorMaterial::from_color(Color::WHITE));
    // (maillage créé vide puis rempli : sa boîte englobante ne suit pas, il serait jugé hors de l'écran)
    commands.spawn((Mesh2d(mesh.clone()), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, Z_WATER), bevy::camera::visibility::NoFrustumCulling));
    commands.insert_resource(WaterOverlay { shapes, mesh });
}

fn update_water_overlay(fog: Res<ShownFog>, map: Res<GameMap>, overlay: Res<WaterOverlay>, layer: Res<MapLayer>, geo: Res<GeoColors>, mut meshes: ResMut<Assets<Mesh>>) {
    // (le fondu vers la mer dépend de la couleur de la mer sur la carte affichée)
    if !fog.is_changed() && !layer.is_changed() && !(geo.is_changed() && *layer == MapLayer::Geography) {
        return;
    }
    let color = |kind: WaterKind, tone: crate::waterways::Tone, lit: bool| {
        let base = match kind {
            WaterKind::River => RIVER_2D,
            WaterKind::Fleuve => FLEUVE_2D,
            WaterKind::Lake => SMALL_LAKE_2D,
            // Éléments des rapides : posés sur l'eau d'une rivière
            WaterKind::Foam | WaterKind::Rock | WaterKind::Light | WaterKind::Shade => RIVER_2D,
        }
        .to_linear();
        // Teinte du sommet (débit, limon) ; écume des rapides ; hors de vue : assombri comme les cases
        let water = tone.apply(Vec3::new(base.red, base.green, base.blue));
        let c = kind.rapids_color(tone, water).unwrap_or(water);
        let k = if lit { 1.0 } else { 0.5 };
        [c.x * k, c.y * k, c.z * k * 1.1, 1.0]
    };
    let mut builder = SlopeMeshBuilder::default();
    // (une case vue au-delà du raccord est celle de la carte)
    let explored = |h: Hex| fog.explored.contains(&map.0.canonical(h));
    let is_water = |h: Hex| map.0.get(h).is_some_and(|t| t.is_water());
    for (hex, shape) in &overlay.shapes {
        let lit = fog.visible.contains(hex);
        // Eau où cette case se jette : un fleuve, par ses ouvertures ; une rivière, toute eau voisine
        let fleuve = map.0.is_fleuve(*hex);
        let open = if fleuve { crate::waterways::fleuve_openings(&map.0, *hex).open } else { 0 };
        let edges = (0..6u8).filter(|&d| open & (1 << d) != 0 && is_water(*hex + crate::hex::DIRECTIONS[d as usize])).fold(0u8, |bits, d| bits | (1 << d));
        let (waters, inlet) = if fleuve { crate::waterways::fleuve_fade(&map.0, *hex) } else { (hex.neighbors().into_iter().filter(|&n| is_water(n)).collect(), None) };
        let targets: Vec<(Vec2, Hex)> = waters.into_iter().filter(|&n| explored(n)).map(|n| (Vec2::from(n.to_world()), n)).collect();
        for t in shape.visible(*hex, explored) {
            let lit = lit || t.also.is_some_and(|o| fog.visible.contains(&map.0.canonical(o)));
            let bed = &shape.channels[t.channel];
            let river = bed.kind == WaterKind::River && bed.inflow.is_none();
            let blend = if river { crate::view3d::MOUTH_BLEND } else { inlet.unwrap_or(crate::view3d::MOUTH_BLEND_FLEUVE) };
            let vertex = |p: Vec2| {
                let own = color(t.kind, t.tone_at(p), lit);
                // Fondu vers la couleur de l'eau, telle que la tuile l'affiche (comme en 3D)
                let near = targets.iter().map(|&(c, n)| (crate::waterways::hex_gap(p, c), n)).min_by(|a, b| a.0.total_cmp(&b.0));
                let Some((d, n)) = near else { return (p, own) };
                let f = 1.0 - (d / blend).clamp(0.0, 1.0);
                if f <= 0.0 {
                    return (p, own);
                }
                let f = f * f * (3.0 - 2.0 * f);
                let (r, g, b) = shown_water_color(&map.0, *layer, &geo, n);
                let tile = if fog.visible.contains(&map.0.canonical(n)) { Color::srgb(r, g, b) } else { Color::srgb(r * 0.5, g * 0.5, b * 0.55) }.to_linear();
                (p, [own[0] + (tile.red - own[0]) * f, own[1] + (tile.green - own[1]) * f, own[2] + (tile.blue - own[2]) * f, 1.0])
            };
            // L'eau s'arrête au bord de la zone explorée, et à celui de la mer où elle se jette
            for pts in crate::waterways::clip_to_explored(t.pts, &explored) {
                for pts in crate::waterways::clip_to_edges(pts, *hex, edges) {
                    // (rivière : son dernier bout ne déborde pas sur la mer)
                    let middle = (pts[0] + pts[1] + pts[2]) / 3.0;
                    if !fleuve && map.0.get(Hex::from_world(middle.x, middle.y)).is_some_and(|t| t.is_sea()) {
                        continue;
                    }
                    builder.tri(pts.map(vertex));
                }
            }
        }
    }
    if let Some(mut mesh) = meshes.get_mut(&overlay.mesh) {
        *mesh = builder.build();
    }
}

/// Couleur (sRGB) d'une case d'eau telle que la tuile 2D l'affiche sur cette carte.
fn shown_water_color(map: &crate::map::WorldMap, layer: MapLayer, geo: &GeoColors, hex: Hex) -> (f32, f32, f32) {
    if layer.shows_ice() && map.is_ice(hex) {
        return ICE_COLOR;
    }
    let t = map.get(hex).unwrap_or(Terrain::Ocean);
    match layer {
        MapLayer::Normal | MapLayer::Resources => level_tint(t.color(), 0),
        MapLayer::Altitude | MapLayer::Relief => elevation_layer_color(map, hex, layer),
        MapLayer::Climate => climate_color(t, map.rain(hex).unwrap_or(500.0)),
        MapLayer::Plates => plate_color(map, hex),
        MapLayer::Currents => currents_color(map, hex),
        MapLayer::Tides => tides_color(map, hex),
        MapLayer::Waves => waves_color(map, hex),
        MapLayer::Geography => geo.0.shade.get(map.cell(map.canonical(hex))).map_or(GEO_LAND, |&s| geo_color(s)),
        MapLayer::Basins | MapLayer::Flow | MapLayer::Network => hydro_color(map, hex, layer),
    }
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReachArea>()
            .init_resource::<TideLines>()
            .init_resource::<MapLayer>()
            .init_resource::<GeoView>()
            .init_resource::<GeoColors>()
            .init_gizmo_group::<GeoGizmos>()
            .init_gizmo_group::<ReachGizmos>()
            .init_gizmo_group::<PlateGizmos>()
            .add_systems(Startup, (setup_map_visuals, setup_reach_gizmos, setup_plate_gizmos, setup_slope_overlay, setup_water_overlay, setup_hatch_overlay, setup_bank_overlay, setup_geo_gizmos))
            .add_systems(
            Update,
            (
                add_unit_visuals,
                add_city_visuals,
                add_site_visuals,
                update_entity_visibility,
                update_hp_bars,
                sync_transforms,
                refresh_edited_2d.before(update_tile_fog),
                update_tile_fog,
                update_turn_labels,
                update_reach_overlay,
                draw_reach_outline.after(update_reach_overlay),
                draw_overlays,
                draw_wind_2d,
                draw_currents_2d,
                draw_tide_lines_2d,
                draw_plates_2d,
                update_slope_overlay,
                update_water_overlay,
                update_hatch_overlay,
                update_bank_overlay,
            ),
        )
        .add_systems(Update, ((update_geo_depth, update_geo_colors).chain().before(update_tile_fog), draw_geo_borders.after(update_geo_colors), draw_waves_2d));
    }
}

pub fn hex_translation(hex: Hex, z: f32) -> Vec3 {
    let (x, y) = hex.to_world();
    Vec3::new(x, y, z)
}

fn setup_map_visuals(
    mut commands: Commands,
    map: Res<GameMap>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut mat = |c: Color| materials.add(ColorMaterial::from_color(c));
    let mut terrain = HashMap::new();
    let mut terrain_dim = HashMap::new();
    for t in Terrain::ALL {
        for level in 0..=MAX_LEVEL {
            // L'eau d'un lac garde sa couleur à toute altitude
            let (r, g, b) = level_tint(t.color(), if t.is_water() { 0 } else { level });
            terrain.insert((t, level), mat(Color::srgb(r, g, b)));
            terrain_dim.insert((t, level), mat(Color::srgb(r * 0.5, g * 0.5, b * 0.55)));
        }
    }
    let mut climate = Vec::new();
    let mut climate_dim = Vec::new();
    let shades = (0..CLIMATE_SHADES).map(|i| scale_color((i as f32 + 0.5) / CLIMATE_SHADES as f32));
    for (r, g, b) in shades.chain([CLIMATE_OCEAN, CLIMATE_COAST]) {
        climate.push(mat(Color::srgb(r, g, b)));
        climate_dim.push(mat(Color::srgb(r * 0.5, g * 0.5, b * 0.55)));
    }
    let mut pair = |colors: Vec<(f32, f32, f32)>| -> (Vec<_>, Vec<_>) {
        colors.into_iter().map(|(r, g, b)| (mat(Color::srgb(r, g, b)), mat(Color::srgb(r * 0.5, g * 0.5, b * 0.55)))).unzip()
    };
    let (basins, basins_dim) = pair(BASIN_VIVID.iter().chain(&BASIN_MUTED).copied().chain([HYDRO_OCEAN, HYDRO_LAKE]).collect());
    let (altitude, altitude_dim) =
        pair((0..ALTITUDE_SHADES).map(|i| altitude_color(altitude_shade_meters(i))).chain([TOPO_OCEAN, TOPO_COAST, TOPO_ABYSS]).collect());
    let (relief, relief_dim) = pair(Relief::ALL.iter().map(|r| r.color()).chain([TOPO_OCEAN, TOPO_COAST]).collect());
    let flow_shades = (0..FLOW_SHADES).map(|i| flow_scale_color((i as f32 + 0.5) / FLOW_SHADES as f32));
    let (flow, flow_dim) = pair(flow_shades.chain([HYDRO_OCEAN, HYDRO_LAKE, NETWORK_LAND, NETWORK_LAKE, NETWORK_OCEAN]).collect());
    let (plates, plates_dim) = pair(plate_shades(map.0.plates()));
    let sea_shades = (0..SEA_SHADES).map(|i| sea_temperature_color(sea_shade_degrees(i)));
    let (currents, currents_dim) = pair(sea_shades.chain([CURRENTS_LAND, CURRENTS_LAKE]).collect());
    let (tides, tides_dim) = pair(TIDE_CLASSES.iter().map(|c| c.1).chain([CURRENTS_LAND, CURRENTS_LAKE]).collect());
    let (waves, waves_dim) = pair(WAVE_CLASSES.iter().map(|c| c.1).chain([CURRENTS_LAND, CURRENTS_LAKE]).collect());
    let geo_shades = (0..GEO_PALETTE.len() as u8).flat_map(|color| (0..=crate::geography::MUTED).map(move |tone| crate::geography::GeoShade::Zone { color, tone }));
    let (geo, geo_dim) = pair(geo_shades.map(geo_color).chain([GEO_LAND, GEO_WATER]).collect());
    // Hexagone à peine plus petit que la case : la grille reste discrète.
    let tile_mesh = meshes.add(RegularPolygon::new(HEX_SIZE * 0.99, 6));
    let palette = Palette {
        tile_mesh: tile_mesh.clone(),
        terrain,
        terrain_dim,
        altitude,
        altitude_dim,
        relief,
        relief_dim,
        climate,
        climate_dim,
        basins,
        basins_dim,
        flow,
        flow_dim,
        plates,
        plates_dim,
        currents,
        currents_dim,
        tides,
        tides_dim,
        waves,
        waves_dim,
        geo,
        geo_dim,
        unexplored: mat(Color::srgb(0.04, 0.04, 0.06)),
        ice: mat(Color::srgb(ICE_COLOR.0, ICE_COLOR.1, ICE_COLOR.2)),
        ice_dim: mat(Color::srgb(ICE_COLOR.0 * ICE_DIM, ICE_COLOR.1 * ICE_DIM, ICE_COLOR.2 * ICE_DIM)),
        unit: mat(PLAYER_COLOR),
        hostile_unit: mat(HOSTILE_COLOR),
        hp_bg: mat(Color::srgb(0.05, 0.05, 0.07)),
        hp_good: mat(Color::srgb(0.30, 0.85, 0.35)),
        hp_mid: mat(Color::srgb(0.95, 0.75, 0.20)),
        hp_low: mat(Color::srgb(0.95, 0.25, 0.20)),
        hp_mesh: meshes.add(Rectangle::new(1.0, 1.0)),
        sites: SiteKit2d {
            ground: meshes.add(Circle::new(22.0)),
            ground_outline: meshes.add(Circle::new(23.5)),
            hut_outline: meshes.add(Circle::new(7.5)),
            hut: meshes.add(Circle::new(6.0)),
            hut_top: meshes.add(Circle::new(1.8)),
            stake: meshes.add(Circle::new(2.3)),
            tent_outline: meshes.add(Triangle2d::new(Vec2::new(0.0, 9.0), Vec2::new(-8.0, -6.0), Vec2::new(8.0, -6.0))),
            tent: meshes.add(Triangle2d::new(Vec2::new(0.0, 7.2), Vec2::new(-6.4, -4.9), Vec2::new(6.4, -4.9))),
            tent_seam: meshes.add(Rectangle::new(1.0, 11.0)),
            fire: meshes.add(Circle::new(3.8)),
            fire_core: meshes.add(Circle::new(1.8)),
            straw: mat(Color::srgb(0.86, 0.72, 0.40)),
            straw_dark: mat(Color::srgb(0.52, 0.40, 0.20)),
            camp_ground: mat(Color::srgb(0.24, 0.18, 0.13)),
            rock: mat(Color::srgb(0.48, 0.46, 0.44)),
            wood: mat(Color::srgb(0.50, 0.34, 0.19)),
            tent_red: mat(Color::srgb(0.72, 0.16, 0.12)),
            fire_color: mat(Color::srgb(1.0, 0.50, 0.10)),
            fire_core_color: mat(Color::srgb(1.0, 0.88, 0.45)),
        },
        outline: mat(Color::srgb(0.05, 0.05, 0.08)),
        city: CityKit2d {
            outer: meshes.add(RegularPolygon::new(HEX_SIZE * 0.9, 6)),
            wall: meshes.add(RegularPolygon::new(HEX_SIZE * 0.84, 6)),
            inner: meshes.add(RegularPolygon::new(HEX_SIZE * 0.7, 6)),
            tower_outline: meshes.add(Circle::new(5.5)),
            tower: meshes.add(Circle::new(4.0)),
            roof: meshes.add(Rectangle::new(5.0, 5.0)),
            keep_outline: meshes.add(Circle::new(10.5)),
            keep: meshes.add(Circle::new(9.0)),
            stone: mat(Color::srgb(0.66, 0.62, 0.55)),
            paving: mat(Color::srgb(0.30, 0.28, 0.26)),
            tiles: mat(Color::srgb(0.68, 0.33, 0.22)),
        },
        label_bg: mat(Color::srgba(0.05, 0.05, 0.08, 0.9)),
        order_label_bg: mat(Color::srgb(0.16, 0.42, 0.78)),
        reach: mat(Color::srgba(1.0, 0.97, 0.75, 0.16)),
        unit_mesh: meshes.add(Circle::new(14.0)),
        outline_mesh: meshes.add(Circle::new(16.5)),
        label_mesh: meshes.add(Circle::new(9.5)),
        icons: UnitKind::ALL.into_iter().map(|kind| (kind, assets.load(kind.icon()))).collect(),
    };

    // Collines : petit motif de trois bosses (côté éclairé et ombre) sur la couleur du terrain
    let glyphs = ReliefGlyphs {
        bump: meshes.add(CircularSector::new(7.5, std::f32::consts::FRAC_PI_2)),
        bump_light: materials.add(ColorMaterial::from_color(Color::srgba(1.0, 1.0, 1.0, 0.24))),
        bump_dark: materials.add(ColorMaterial::from_color(Color::srgba(0.0, 0.0, 0.0, 0.32))),
        rugged_dark: materials.add(ColorMaterial::from_color(Color::srgba(0.0, 0.0, 0.0, 0.48))),
        snow_cap: materials.add(ColorMaterial::from_color(Color::srgba(1.0, 1.0, 1.0, 0.9))),
        // Sommet : triangle entier (ombre), moitié éclairée, calotte de neige
        peak_shade: meshes.add(Triangle2d::new(Vec2::new(-10.0, -8.0), Vec2::new(10.0, -8.0), Vec2::new(0.0, 9.0))),
        peak_lit: meshes.add(Triangle2d::new(Vec2::new(-10.0, -8.0), Vec2::new(0.0, -8.0), Vec2::new(0.0, 9.0))),
        peak_cap: meshes.add(Triangle2d::new(Vec2::new(-3.5, 3.0), Vec2::new(3.5, 3.0), Vec2::new(0.0, 9.0))),
    };
    for hex in map.0.hexes() {
        commands.spawn((
            Tile(hex),
            Mesh2d(tile_mesh.clone()),
            MeshMaterial2d(palette.unexplored.clone()),
            Transform::from_translation(hex_translation(hex, Z_TILE)),
        ));
        spawn_relief_marks(&mut commands, &glyphs, map.0.relief(hex), hex, Visibility::Hidden);
    }
    commands.insert_resource(glyphs);
    commands.insert_resource(palette);
    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.03)));
}

/// Motif de collines d'une case (visible une fois la case explorée).
#[derive(Component)]
struct HillMark(Hex);

/// Maillages et matériaux des motifs de relief (gardés pour redessiner une case modifiée).
#[derive(Resource)]
struct ReliefGlyphs {
    bump: Handle<Mesh>,
    bump_light: Handle<ColorMaterial>,
    bump_dark: Handle<ColorMaterial>,
    rugged_dark: Handle<ColorMaterial>,
    snow_cap: Handle<ColorMaterial>,
    peak_shade: Handle<Mesh>,
    peak_lit: Handle<Mesh>,
    peak_cap: Handle<Mesh>,
}

/// Motif du relief d'une case : bosses (collines), bosses serrées et plus marquées (accidenté),
/// sommets triangulaires (montagne), grand sommet enneigé (pic).
fn spawn_relief_marks(commands: &mut Commands, g: &ReliefGlyphs, relief: Relief, hex: Hex, visibility: Visibility) {
    let mut glyph = |mesh: &Handle<Mesh>, material: &Handle<ColorMaterial>, at: Vec3, k: f32| {
        commands.spawn((
            HillMark(hex),
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(k)),
            visibility,
        ));
    };
    let base = hex_translation(hex, Z_HILLS);
    let bumps: &[(f32, f32, f32)] = match relief {
        Relief::Hills => &[(-9.0, -4.0, 1.0), (8.0, -6.0, 0.85), (0.0, 8.0, 0.8)],
        Relief::Rugged => &[(-10.0, -5.0, 0.8), (-1.0, -8.0, 0.75), (9.0, -3.0, 0.8), (-5.0, 6.0, 0.75), (6.0, 8.0, 0.7)],
        _ => &[],
    };
    let dark = if relief == Relief::Rugged { &g.rugged_dark } else { &g.bump_dark };
    for &(dx, dy, k) in bumps {
        let at = base + Vec3::new(dx, dy, 0.0);
        glyph(&g.bump, dark, at + Vec3::new(1.5, -1.0, 0.0), k);
        glyph(&g.bump, &g.bump_light, at + Vec3::Z * 0.01, k);
    }
    let peaks: &[(f32, f32, f32)] = match relief {
        Relief::Mountain => &[(-7.0, -3.0, 0.8), (7.0, 1.0, 0.95)],
        Relief::Peak => &[(0.0, -2.0, 1.45)],
        _ => &[],
    };
    for &(dx, dy, k) in peaks {
        let at = base + Vec3::new(dx, dy, 0.0);
        glyph(&g.peak_shade, &g.bump_dark, at, k);
        glyph(&g.peak_lit, &g.bump_light, at + Vec3::Z * 0.01, k);
        if relief == Relief::Peak {
            glyph(&g.peak_cap, &g.snow_cap, at + Vec3::Z * 0.02, k);
        }
    }
}

/// Cases modifiées (console de debug) : motifs de relief redessinés (couleurs et pentes suivent
/// le brouillard affiché, marqué changé par la console).
fn refresh_edited_2d(
    mut commands: Commands,
    mut edited: MessageReader<crate::console::MapEdited>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    glyphs: Option<Res<ReliefGlyphs>>,
    marks: Query<(Entity, &HillMark)>,
) {
    let cells: HashSet<Hex> = edited.read().flat_map(|e| e.0.iter().copied()).collect();
    let Some(glyphs) = glyphs else { return };
    if cells.is_empty() {
        return;
    }
    for (e, mark) in &marks {
        if cells.contains(&mark.0) {
            commands.entity(e).despawn();
        }
    }
    for &h in &cells {
        let visibility = if fog.explored.contains(&h) { Visibility::Inherited } else { Visibility::Hidden };
        spawn_relief_marks(&mut commands, &glyphs, map.0.relief(h), h, visibility);
    }
}

fn update_tile_fog(
    fog: Res<ShownFog>,
    map: Res<GameMap>,
    palette: Res<Palette>,
    mut tiles: Query<(&Tile, &mut MeshMaterial2d<ColorMaterial>)>,
    mut marks: Query<(&HillMark, &mut Visibility)>,
    layer: Res<MapLayer>,
    geo: Res<GeoColors>,
) {
    if !fog.is_changed() && !layer.is_changed() && !(geo.is_changed() && *layer == MapLayer::Geography) {
        return;
    }
    let (lit, dim) = (&palette.terrain, &palette.terrain_dim);
    // Cartes à teintes indexées (climat, bassins, débits) : teintes vives et assombries
    let indexed: Option<(&Vec<_>, &Vec<_>)> = match *layer {
        MapLayer::Climate => Some((&palette.climate, &palette.climate_dim)),
        MapLayer::Altitude => Some((&palette.altitude, &palette.altitude_dim)),
        MapLayer::Relief => Some((&palette.relief, &palette.relief_dim)),
        MapLayer::Basins => Some((&palette.basins, &palette.basins_dim)),
        MapLayer::Flow | MapLayer::Network => Some((&palette.flow, &palette.flow_dim)),
        MapLayer::Plates if !palette.plates.is_empty() => Some((&palette.plates, &palette.plates_dim)),
        MapLayer::Currents => Some((&palette.currents, &palette.currents_dim)),
        MapLayer::Tides => Some((&palette.tides, &palette.tides_dim)),
        MapLayer::Waves => Some((&palette.waves, &palette.waves_dim)),
        MapLayer::Geography if geo.0.shade.len() == (map.0.width * map.0.height) as usize => Some((&palette.geo, &palette.geo_dim)),
        _ => None,
    };
    let shade_index = |hex: Hex| match (*layer, map.0.get(hex)) {
        (MapLayer::Climate, Some(Terrain::Ocean)) => CLIMATE_SHADES,
        (MapLayer::Climate, Some(Terrain::Coast | Terrain::Lake)) => CLIMATE_SHADES + 1,
        (MapLayer::Climate, _) => climate_shade(map.0.rain(hex).unwrap_or(500.0)),
        (MapLayer::Altitude, _) => altitude_index(&map.0, hex),
        (MapLayer::Relief, _) => relief_index(&map.0, hex),
        (MapLayer::Plates, _) => plate_index(&map.0, hex),
        (MapLayer::Currents, _) => currents_index(&map.0, hex),
        (MapLayer::Tides, _) => tides_index(&map.0, hex),
        (MapLayer::Waves, _) => waves_index(&map.0, hex),
        (MapLayer::Geography, _) => geo_index(geo.0.shade[map.0.cell(hex)]),
        _ => hydro_index(&map.0, hex, *layer),
    };
    for (mark, mut vis) in &mut marks {
        let wanted = if fog.explored.contains(&mark.0) { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
    }
    for (tile, mut material) in &mut tiles {
        let key = (map.0.get(tile.0).unwrap(), map.0.level(tile.0).unwrap_or(0));
        let ice = layer.shows_ice() && map.0.is_ice(tile.0);
        let wanted = if fog.visible.contains(&tile.0) {
            if ice {
                &palette.ice
            } else if let Some((shades, _)) = indexed {
                &shades[shade_index(tile.0)]
            } else {
                &lit[&key]
            }
        } else if fog.explored.contains(&tile.0) {
            if ice {
                &palette.ice_dim
            } else if let Some((_, shades)) = indexed {
                &shades[shade_index(tile.0)]
            } else {
                &dim[&key]
            }
        } else {
            &palette.unexplored
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}

fn add_unit_visuals(
    mut commands: Commands,
    palette: Res<Palette>,
    units: Query<(Entity, &Unit, &HexPos, Has<Hostile>), Added<Unit>>,
) {
    for (entity, unit, pos, hostile) in &units {
        let color = if hostile { &palette.hostile_unit } else { &palette.unit };
        commands
            .entity(entity)
            .insert((
                Mesh2d(palette.outline_mesh.clone()),
                MeshMaterial2d(palette.outline.clone()),
                Transform::from_translation(hex_translation(pos.0, Z_UNIT)),
                // Les hostiles n'apparaissent que lorsqu'ils sont à portée de vue
                if hostile { Visibility::Hidden } else { Visibility::Inherited },
            ))
            .with_children(|p| {
                p.spawn((
                    Mesh2d(palette.unit_mesh.clone()),
                    MeshMaterial2d(color.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.01),
                ));
                p.spawn((
                    Sprite {
                        image: palette.icons[&unit.0.kind].clone(),
                        custom_size: Some(Vec2::splat(21.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 0.02),
                ));
                // Barre de PV (masquée tant que l'unité est en pleine santé)
                p.spawn((
                    HpBar,
                    Mesh2d(palette.hp_mesh.clone()),
                    MeshMaterial2d(palette.hp_bg.clone()),
                    Transform::from_xyz(0.0, -20.0, 0.03).with_scale(Vec3::new(HP_BAR_WIDTH + 2.0, 6.0, 1.0)),
                    Visibility::Hidden,
                ));
                p.spawn((
                    HpFill,
                    Mesh2d(palette.hp_mesh.clone()),
                    MeshMaterial2d(palette.hp_good.clone()),
                    Transform::from_xyz(0.0, -20.0, 0.04).with_scale(Vec3::new(HP_BAR_WIDTH, 4.0, 1.0)),
                    Visibility::Hidden,
                ));
            });
    }
}

/// Met à jour les barres de PV des unités dont l'état a changé.
/// Pendant un combat mis en scène, la barre garde les PV d'avant le coup jusqu'à l'impact.
fn update_hp_bars(
    palette: Res<Palette>,
    time: Res<Time>,
    units: Query<(&Unit, &Children, Option<&HeldHp>), Or<(Changed<Unit>, With<HeldHp>)>>,
    mut bars: Query<&mut Visibility, (With<HpBar>, Without<HpFill>)>,
    mut fills: Query<(&mut Transform, &mut MeshMaterial2d<ColorMaterial>, &mut Visibility), With<HpFill>>,
) {
    let now = time.elapsed_secs_f64();
    for (unit, children, held) in &units {
        let hp = HeldHp::shown(held, unit.0.hp, now).clamp(0, crate::rules::MAX_HP);
        let wounded = hp < crate::rules::MAX_HP;
        let shown = if wounded { Visibility::Inherited } else { Visibility::Hidden };
        for child in children.iter() {
            if let Ok(mut vis) = bars.get_mut(child)
                && *vis != shown
            {
                *vis = shown;
            }
            if let Ok((mut transform, mut material, mut vis)) = fills.get_mut(child) {
                if *vis != shown {
                    *vis = shown;
                }
                let ratio = hp as f32 / crate::rules::MAX_HP as f32;
                transform.scale.x = HP_BAR_WIDTH * ratio;
                transform.translation.x = -HP_BAR_WIDTH * (1.0 - ratio) / 2.0;
                let wanted = if ratio > 0.6 {
                    &palette.hp_good
                } else if ratio > 0.3 {
                    &palette.hp_mid
                } else {
                    &palette.hp_low
                };
                if material.0 != *wanted {
                    material.0 = wanted.clone();
                }
            }
        }
    }
}

/// Villages et camps vus de dessus, cachés tant que la case n'est pas explorée :
/// village = huttes de paille autour d'un feu ; camp hostile = tentes rouges, avec une défense
/// selon son type (fort : palissade pleine ; campement : arc de pieux ; repaire : rochers et pieux).
fn add_site_visuals(
    mut commands: Commands,
    palette: Res<Palette>,
    sites: Query<(Entity, &HexPos, Option<&Camp>), Or<(Added<Village>, Added<Camp>)>>,
) {
    let k = &palette.sites;
    let part = |mesh: &Handle<Mesh>, material: &Handle<ColorMaterial>, x: f32, y: f32, z: f32| {
        (Mesh2d(mesh.clone()), MeshMaterial2d(material.clone()), Transform::from_xyz(x, y, z))
    };
    let polar = |deg: f32, r: f32| {
        let (sin, cos) = deg.to_radians().sin_cos();
        (cos * r, sin * r)
    };
    for (entity, pos, camp) in &sites {
        let is_camp = camp.is_some();
        let kind = camp.map(|c| c.kind);
        let mut site = commands.entity(entity);
        site.insert((Transform::from_translation(hex_translation(pos.0, Z_SITE)), Visibility::Hidden));
        // Le village est posé directement sur le terrain de la case ; le camp a son sol de terre battue
        if is_camp {
            site.insert((Mesh2d(k.ground_outline.clone()), MeshMaterial2d(palette.outline.clone())));
        }
        site.with_children(|p| {
            if is_camp {
                p.spawn(part(&k.ground, &k.camp_ground, 0.0, 0.0, 0.01));
                let (from, span, count) = match kind {
                    Some(crate::settlements::CampKind::Fort) => (0.0, 360.0, 22),
                    Some(crate::settlements::CampKind::Lair) => (-60.0, 140.0, 7),
                    _ => (-40.0, 260.0, 11),
                };
                for i in 0..count {
                    let (x, y) = polar(from + span * i as f32 / count as f32, 20.5);
                    p.spawn(part(&k.stake, &k.wood, x, y, 0.02));
                }
                if kind == Some(crate::settlements::CampKind::Lair) {
                    for (a, r) in [(150.0, 17.0), (195.0, 18.5), (240.0, 16.5)] {
                        let (x, y) = polar(a, r);
                        p.spawn(part(&k.tent, &k.rock, x, y, 0.025));
                    }
                }
                for i in 0..3 {
                    let (x, y) = polar(90.0 + 120.0 * i as f32, 12.0);
                    p.spawn(part(&k.tent_outline, &palette.outline, x, y, 0.02));
                    p.spawn(part(&k.tent, &k.tent_red, x, y, 0.03));
                    p.spawn(part(&k.tent_seam, &palette.outline, x, y + 1.0, 0.04));
                }
            } else {
                for i in 0..5 {
                    let (x, y) = polar(90.0 + 72.0 * i as f32, 13.0);
                    p.spawn(part(&k.hut_outline, &palette.outline, x, y, 0.02));
                    p.spawn(part(&k.hut, &k.straw, x, y, 0.03));
                    p.spawn(part(&k.hut_top, &k.straw_dark, x, y, 0.04));
                }
            }
            p.spawn(part(&k.fire, &k.fire_color, 0.0, 0.0, 0.05));
            p.spawn(part(&k.fire_core, &k.fire_core_color, 0.0, 0.0, 0.06));
        });
    }
}

/// Visibilité selon le brouillard : sites une fois découverts, guerriers hostiles seulement à vue
/// (ou pendant un déplacement mis en scène, qui n'est montré que s'il passe à vue).
fn update_entity_visibility(
    fog: Res<ShownFog>,
    mut sites: Query<(&HexPos, &mut Visibility), (Or<(With<Village>, With<Camp>)>, Without<Hostile>)>,
    mut hostiles: Query<(&HexPos, &mut Visibility, Has<Motion>), With<Hostile>>,
) {
    let apply = |visible: bool, vis: &mut Visibility| {
        let wanted = if visible { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
    };
    for (pos, mut vis) in &mut sites {
        apply(fog.explored.contains(&pos.0), &mut vis);
    }
    for (pos, mut vis, moving) in &mut hostiles {
        apply(moving || fog.visible.contains(&pos.0), &mut vis);
    }
}

/// Symbole de ville vu de dessus, refait quand son stade ou sa fortification change : sol de terre,
/// toits de plus en plus nombreux, grand toit au centre aux couleurs du joueur ; une enceinte et
/// ses tours si la ville est fortifiée (le nom et la population sont dans la bannière, voir labels.rs).
fn add_city_visuals(mut commands: Commands, palette: Res<Palette>, cities: Query<(Entity, &HexPos, &City, Option<&CitySymbol>)>) {
    let k = &palette.city;
    for (entity, pos, city, symbol) in &cities {
        let stage = crate::settlements::stage_of_population(city.state.population);
        let code = stage as u8 * 2 + u8::from(city.state.fortified);
        if symbol.is_some_and(|s| s.0 == code) {
            continue;
        }
        let part = |mesh: &Handle<Mesh>, material: &Handle<ColorMaterial>, x: f32, y: f32, z: f32, scale: f32| {
            (Mesh2d(mesh.clone()), MeshMaterial2d(material.clone()), Transform::from_xyz(x, y, z).with_scale(Vec3::splat(scale)))
        };
        let size = [0.55, 0.72, 0.9][stage];
        let fortified = city.state.fortified;
        let mut root = commands.entity(entity);
        root.despawn_related::<Children>();
        root.insert((CitySymbol(code), Transform::from_translation(hex_translation(pos.0, Z_CITY)))).with_children(|p| {
            if fortified {
                p.spawn(part(&k.outer, &palette.outline, 0.0, 0.0, 0.0, size * 1.14));
                p.spawn(part(&k.wall, &k.stone, 0.0, 0.0, 0.01, size * 1.14));
            }
            p.spawn(part(&k.inner, &k.paving, 0.0, 0.0, 0.02, size * 1.14));
            let roofs = [3, 6, 10][stage];
            let reach = [7.5, 11.0, 15.0][stage];
            for i in 0..roofs {
                let (sin, cos) = (20.0 + 360.0 * i as f32 / roofs as f32).to_radians().sin_cos();
                p.spawn(part(&k.roof, &k.tiles, cos * reach, sin * reach, 0.03, 1.0));
            }
            if fortified {
                for i in 0..6 {
                    let (sin, cos) = (30.0 + 60.0 * i as f32).to_radians().sin_cos();
                    let (tx, ty) = (cos * HEX_SIZE * 0.92 * size, sin * HEX_SIZE * 0.92 * size);
                    p.spawn(part(&k.tower_outline, &palette.outline, tx, ty, 0.03, 0.7));
                    p.spawn(part(&k.tower, &palette.unit, tx, ty, 0.04, 0.7));
                }
            }
            p.spawn(part(&k.keep_outline, &palette.outline, 0.0, 0.0, 0.05, 0.62));
            p.spawn(part(&k.keep, &palette.unit, 0.0, 0.0, 0.06, 0.62));
        });
    }
}

/// Surligne les cases que l'unité sélectionnée peut atteindre ce tour-ci.
fn update_reach_overlay(
    mut commands: Commands,
    selection: Res<Selection>,
    map: Res<GameMap>,
    fog: Res<Fog>,
    blocked: Res<Blocked>,
    occupancy: Res<Occupancy>,
    enemy_zoc: Res<EnemyZoc>,
    palette: Res<Palette>,
    units: Query<(&Unit, &HexPos)>,
    overlays: Query<Entity, With<ReachOverlay>>,
    mut area: ResMut<ReachArea>,
    mut last: Local<Option<(Entity, Hex, u32, bool, bool)>>,
) {
    // Clé de l'état affiché : on ne reconstruit que si l'unité, sa position, ses PM,
    // son attaque du tour ou son arrêt en zone de contrôle changent
    let key = selection
        .0
        .and_then(|e| units.get(e).ok().map(|(u, p)| (e, p.0, u.0.moves_left, u.0.has_attacked, u.0.zoc_stopped)))
        .filter(|k| k.2 > 0);
    if *last == key
        && !fog.is_changed()
        && !blocked.is_changed()
        && !occupancy.is_changed()
        && !enemy_zoc.is_changed()
    {
        return;
    }
    *last = key;
    let current = key.map(|(e, p, m, _, _)| (e, p, m));
    for e in &overlays {
        commands.entity(e).despawn();
    }
    area.cells.clear();
    area.attackable.clear();
    let Some((entity, pos, moves)) = current else { return };
    // Carte connue : les cases du brouillard comptent comme des prairies (rien n'est révélé)
    let blocked_now = crate::game::movement_blocked(&blocked.0, &occupancy.own);
    let known = KnownMap::new(&map.0, &fog.explored).blocking(&blocked_now);
    let Ok((unit, _)) = units.get(entity) else { return };
    if unit.0.kind.can_attack() {
        let reach = moves as i32 + unit.0.kind.stats().range.max(1);
        let mut targets: Vec<Hex> = blocked
            .0
            .iter()
            .copied()
            .filter(|t| map.0.distance(*t, pos) <= reach)
            .filter(|t| crate::rules::attack_approach(&known, &map.0, &unit.0, pos, *t, &enemy_zoc.0).is_some())
            .collect();
        targets.sort();
        area.attackable = targets;
    }
    // Arrêtée par une zone de contrôle : plus de déplacement possible ce tour-ci
    if unit.0.zoc_stopped {
        return;
    }
    let cells = reachable(&crate::rules::ForUnit::new(&known, unit.0.kind), pos, moves);
    if cells.is_empty() {
        return;
    }
    area.cells = cells.iter().copied().chain([pos]).collect();
    for hex in cells {
        commands.spawn((
            ReachOverlay,
            Mesh2d(palette.tile_mesh.clone()),
            MeshMaterial2d(palette.reach.clone()),
            Transform::from_translation(hex_translation(hex, Z_REACH)),
        ));
    }
}

fn setup_reach_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<ReachGizmos>();
    config.line.width = 3.0;
}

/// Contour extérieur de la zone de déplacement : arêtes entre une case de la zone et une case hors zone.
fn draw_reach_outline(mut gizmos: Gizmos<ReachGizmos>, area: Res<ReachArea>, map: Res<GameMap>) {
    for &target in &area.attackable {
        let (x, y) = target.to_world();
        gizmos.circle_2d(Vec2::new(x, y), HEX_SIZE * 0.72, ATTACK_RING);
    }
    for &hex in &area.cells {
        for (dir, d) in DIRECTIONS.iter().enumerate() {
            if !area.cells.contains(&map.0.canonical(hex + *d)) {
                let (a, b) = hex.edge_corners(dir);
                gizmos.line_2d(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), REACH_OUTLINE);
            }
        }
    }
}

/// Place une pastille numérotée sur la dernière case atteinte à chaque tour du trajet.
/// Pastilles numérotées sur la dernière case atteinte à chaque tour :
/// en bleu pour l'ordre en cours, en sombre pour l'aperçu sous le curseur.
fn update_turn_labels(
    mut commands: Commands,
    hover: Res<HoverPath>,
    order: Res<OrderPath>,
    palette: Res<Palette>,
    labels: Query<Entity, With<TurnLabel>>,
) {
    if !hover.is_changed() && !order.is_changed() {
        return;
    }
    for e in &labels {
        commands.entity(e).despawn();
    }
    spawn_turn_labels(&mut commands, &palette, &order.path, &order.turns, &palette.order_label_bg, Z_LABEL);
    // L'aperçu est inutile s'il est identique à l'ordre déjà donné
    if hover.path != order.path {
        spawn_turn_labels(&mut commands, &palette, &hover.path, &hover.turns, &palette.label_bg, Z_LABEL + 0.5);
    }
}

fn spawn_turn_labels(
    commands: &mut Commands,
    palette: &Palette,
    path: &[Hex],
    turns: &[u32],
    background: &Handle<ColorMaterial>,
    z: f32,
) {
    for (i, (&hex, &turn)) in path.iter().zip(turns).enumerate() {
        let last_of_turn = turns.get(i + 1).is_none_or(|&next| next != turn);
        if !last_of_turn {
            continue;
        }
        commands.spawn((
            TurnLabel,
            Mesh2d(palette.label_mesh.clone()),
            MeshMaterial2d(background.clone()),
            Transform::from_translation(hex_translation(hex, z)),
            children![(
                Text2d::new(turn.to_string()),
                TextFont::from_font_size(13.0),
                TextColor(Color::WHITE),
                Transform::from_xyz(0.0, 0.0, 0.01),
            )],
        ));
    }
}

/// Recale les entités sur leur case quand elles changent de case (les animations de `feedback`
/// s'appliquent ensuite).
pub(crate) fn sync_transforms(mut moved: Query<(&HexPos, &mut Transform), Changed<HexPos>>) {
    for (pos, mut transform) in &mut moved {
        let z = transform.translation.z;
        transform.translation = hex_translation(pos.0, z);
    }
}

/// Frontières, sélection, chemin prévisualisé, cases exploitées.
fn draw_overlays(
    mut gizmos: Gizmos,
    map: Res<GameMap>,
    territory: Res<Territory>,
    selection: Res<Selection>,
    hover: Res<HoverPath>,
    positions: Query<&HexPos>,
    units: Query<&Unit>,
    cities: Query<&City>,
    motions: Query<&Motion>,
    time: Res<Time>,
) {
    // Frontières : arêtes entre une case possédée et une case non possédée (ou d'une autre ville)
    for (&hex, &owner) in &territory.0 {
        for (dir, d) in DIRECTIONS.iter().enumerate() {
            if territory.0.get(&map.0.canonical(hex + *d)) != Some(&owner) {
                let (a, b) = hex.edge_corners(dir);
                gizmos.line_2d(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), BORDER_COLOR);
            }
        }
    }

    let Some(selected) = selection.0 else { return };
    let Ok(pos) = positions.get(selected) else { return };
    // (l'anneau suit l'unité pendant son déplacement mis en scène)
    let (x, y) = pos.0.to_world();
    let ring = motions.get(selected).ok().and_then(|m| m.sample(time.elapsed_secs_f64())).map_or(Vec2::new(x, y), |p| p.at);
    gizmos.circle_2d(ring, HEX_SIZE * 0.85, Color::srgb(1.0, 0.9, 0.2));

    if let Ok(city) = cities.get(selected) {
        for h in &city.worked {
            let (wx, wy) = h.to_world();
            gizmos.circle_2d(Vec2::new(wx, wy), 6.0, Color::srgb(1.0, 0.9, 0.2));
        }
    }

    if let Ok(unit) = units.get(selected) {
        // Ordre en cours (bleu) puis prévisualisation sous le curseur (blanc)
        let draw_path = |gizmos: &mut Gizmos, path: &[Hex], color: Color| {
            // Chaque case prise du côté du raccord le plus proche de la précédente
            let mut prev = pos.0;
            let points = std::iter::once(pos.0).chain(path.iter().copied()).map(|h| {
                prev = map.0.wrap().nearest(prev, h);
                let (px, py) = prev.to_world();
                Vec2::new(px, py)
            });
            gizmos.linestrip_2d(points, color);
        };
        if !unit.0.path.is_empty() {
            draw_path(&mut gizmos, &unit.0.path, Color::srgb(0.4, 0.8, 1.0));
        }
        if !hover.path.is_empty() {
            draw_path(&mut gizmos, &hover.path, Color::WHITE);
            if let Some(&last) = hover.path.last() {
                let (lx, ly) = last.to_world();
                gizmos.circle_2d(Vec2::new(lx, ly), 8.0, Color::WHITE);
            }
        }
    }
}

/// Carte du climat : flèches des vents dominants sur les cases explorées.
fn draw_wind_2d(layer: Res<MapLayer>, map: Res<GameMap>, fog: Res<ShownFog>, mut gizmos: Gizmos) {
    if *layer != MapLayer::Climate {
        return;
    }
    for &hex in fog.explored.iter().filter(|&&h| wind_arrow_site(h)) {
        if let Some((from, to)) = wind_arrow(&map.0, hex) {
            gizmos.arrow_2d(from, to, WIND_ARROW).with_tip_length(7.0);
        }
    }
}

/// Carte des courants marins : flèches des courants sur les cases de mer explorées.
fn draw_currents_2d(layer: Res<MapLayer>, map: Res<GameMap>, fog: Res<ShownFog>, mut gizmos: Gizmos) {
    if *layer != MapLayer::Currents {
        return;
    }
    for &hex in fog.explored.iter().filter(|&&h| wind_arrow_site(h)) {
        if let Some((from, to, color)) = current_arrow(&map.0, hex) {
            gizmos.arrow_2d(from, to, color).with_tip_length(6.0);
        }
    }
}

/// Carte de la houle : flèches du sens de la houle sur les cases de mer explorées.
fn draw_waves_2d(layer: Res<MapLayer>, map: Res<GameMap>, fog: Res<ShownFog>, mut gizmos: Gizmos) {
    if *layer != MapLayer::Waves {
        return;
    }
    for &hex in fog.explored.iter().filter(|&&h| wind_arrow_site(h)) {
        if let Some((from, to, color)) = wave_arrow(&map.0, hex) {
            gizmos.arrow_2d(from, to, color).with_tip_length(6.0);
        }
    }
}

fn setup_plate_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<PlateGizmos>();
    config.line.width = 4.0;
}

/// Carte des plaques : frontières colorées selon leur nature (arêtes dont les deux cases sont
/// explorées), flèches du mouvement des plaques, points chauds.
fn draw_plates_2d(layer: Res<MapLayer>, map: Res<GameMap>, fog: Res<ShownFog>, view: Res<crate::view3d::ViewMode>, mut lines: Gizmos<PlateGizmos>, mut gizmos: Gizmos) {
    if *layer != MapLayer::Plates || view.is_3d {
        return;
    }
    let m = &map.0;
    for e in &m.plates().edges {
        let h = m.hex_at(e.cell);
        let other = m.canonical(h + DIRECTIONS[usize::from(e.dir)]);
        if !fog.explored.contains(&h) || !fog.explored.contains(&other) {
            continue;
        }
        let ((ax, ay), (bx, by)) = h.edge_corners(usize::from(e.dir));
        let (r, g, b) = e.kind.color();
        lines.line_2d(Vec2::new(ax, ay), Vec2::new(bx, by), Color::srgb(r, g, b));
    }
    for &hex in fog.explored.iter().filter(|&&h| wind_arrow_site(h)) {
        if let Some((from, to)) = plate_arrow(m, hex) {
            gizmos.arrow_2d(from, to, PLATE_ARROW).with_tip_length(6.0);
        }
    }
    for &h in m.plates().hotspots.iter().filter(|h| fog.explored.contains(*h)) {
        let (x, y) = h.to_world();
        gizmos.circle_2d(Vec2::new(x, y), HEX_SIZE * 0.45, HOTSPOT);
        gizmos.circle_2d(Vec2::new(x, y), HEX_SIZE * 0.2, HOTSPOT);
    }
}
