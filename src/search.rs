//! Accès aux lieux de la carte : recherche, légendes et étiquettes qui mènent aux lieux.
//!
//! - Un lieu (`MapTarget`) : une ville, un cours d'eau, un lac ou une mer intérieure, un bassin
//!   versant, une plaque tectonique.
//! - Un `TargetButton` (ligne de légende, étiquette sur la carte, résultat de recherche) : son
//!   survol met le lieu en avant (pourtour doré qui pulse pour une zone, surbrillance du réseau
//!   pour un cours d'eau ou un lac), son clic y amène la caméra (vol court, zoom qui cadre la
//!   partie connue du lieu), puis le lieu reste en avant quelques secondes.
//! - La recherche (Ctrl+F ou bouton) : un champ, des filtres (puces avec le nombre de résultats
//!   de chaque catégorie), une courte liste de résultats classés (début du nom, puis début d'un
//!   mot, puis n'importe où ; sans tenir compte des accents ni des majuscules), le passage trouvé
//!   en gras. Flèches pour choisir, Entrée pour y aller, Tab pour changer de filtre, Échap pour
//!   fermer. Seuls les lieux connus (en partie explorés) sont listés, sauf en mode debug.
//!
//! Pendant la saisie dans un champ (recherche, graine du menu), les raccourcis du jeu sont
//! coupés : les touches vont au champ.

use crate::basins::Basins;
use crate::game::{City, DebugMode, GameMap, HexPos, Selection, ShownFog, Territory};
use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::highlight::{Feature, PointedFeature};
use crate::input::PointerOverUi;
use crate::map::WorldMap;
use crate::view3d::{Cam3d, Gizmos3d, Relief, ViewMode};
use bevy::input::InputSystems;
use bevy::input::keyboard::Key;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle, TextEdit};
use bevy::window::PrimaryWindow;
use std::collections::{HashMap, HashSet};

/// Un lieu de la carte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapTarget {
    City(Entity),
    /// Indice dans `Basins::rivers`.
    River(usize),
    /// Indice dans `Basins::lakes` (lacs et mers intérieures).
    Lake(usize),
    /// Indice dans `Basins::basins`.
    Basin(usize),
    /// Indice de la plaque.
    Plate(usize),
}

/// Catégorie de lieu (filtres de la recherche).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    City,
    River,
    Lake,
    Basin,
    Plate,
}

impl Category {
    pub const ALL: [Category; 5] = [Category::City, Category::River, Category::Lake, Category::Basin, Category::Plate];

    pub fn label(self) -> &'static str {
        match self {
            Category::City => "Villes",
            Category::River => "Cours d'eau",
            Category::Lake => "Lacs et mers",
            Category::Basin => "Bassins versants",
            Category::Plate => "Plaques",
        }
    }

    fn color(self) -> Color {
        match self {
            Category::City => Color::srgb(0.30, 0.55, 1.0),
            Category::River => Color::srgb(0.45, 0.78, 1.0),
            Category::Lake => Color::srgb(0.20, 0.70, 0.75),
            Category::Basin => Color::srgb(0.45, 0.80, 0.35),
            Category::Plate => Color::srgb(0.95, 0.60, 0.25),
        }
    }
}

impl MapTarget {
    /// Élément du réseau hydrographique à mettre en surbrillance : le cours d'eau ou le lac, et
    /// pour un bassin, le lac ou le fleuve qui lui donne son nom.
    fn feature(self, basins: &Basins) -> Option<Feature> {
        match self {
            MapTarget::River(r) => Some(Feature::River(r)),
            MapTarget::Lake(l) => Some(Feature::Lake(l)),
            MapTarget::Basin(b) => {
                let basin = basins.basins.get(b)?;
                basin.lake.map(Feature::Lake).or(basin.river.map(Feature::River))
            }
            _ => None,
        }
    }

    /// Le lieu est une zone dont on dessine le pourtour (un cours d'eau ou un lac a déjà sa
    /// surbrillance).
    fn outlined(self) -> bool {
        matches!(self, MapTarget::City(_) | MapTarget::Basin(_) | MapTarget::Plate(_))
    }
}

/// Élément d'interface qui mène à un lieu.
#[derive(Component, Clone, Copy)]
pub struct TargetButton {
    pub target: MapTarget,
    /// Son survol met le lieu en avant (sinon seul le clic compte : les étiquettes des cours
    /// d'eau ont leur propre surbrillance, réglée par l'option).
    pub hover: bool,
}

impl TargetButton {
    pub fn new(target: MapTarget) -> Self {
        Self { target, hover: true }
    }

    pub fn click_only(target: MapTarget) -> Self {
        Self { target, hover: false }
    }
}

/// Couleurs de fond d'un bouton au repos et survolé.
#[derive(Component, Clone, Copy)]
pub struct HoverColors {
    pub normal: Color,
    pub hover: Color,
}

/// Lieu mis en avant : survolé à la souris, choisi au clavier dans la recherche, ou rejoint
/// récemment (pendant quelques secondes).
#[derive(Resource, Default)]
pub struct PointedTarget {
    hover: Option<MapTarget>,
    keyboard: Option<MapTarget>,
    flash: Option<(MapTarget, f32)>,
}

impl PointedTarget {
    pub fn current(&self) -> Option<MapTarget> {
        self.hover.or(self.keyboard).or(self.flash.map(|(t, _)| t))
    }
}

/// Demande d'amener la caméra sur un lieu.
#[derive(Message, Clone, Copy)]
pub struct FocusTarget(pub MapTarget);

/// Durée pendant laquelle un lieu rejoint reste en avant (s).
const FLASH_SECONDS: f32 = 3.0;
/// Couleur du pourtour d'une zone.
const OUTLINE_COLOR: Color = Color::srgb(1.0, 0.84, 0.25);

/// Pourtour d'une zone en 2D.
#[derive(Default, Reflect, GizmoConfigGroup)]
struct AreaGizmos2d;

/// Bouton « Rechercher » (en bas à droite).
#[derive(Component)]
pub struct SearchButton;

#[derive(Component)]
struct SearchRoot;
#[derive(Component)]
struct SearchInput;
#[derive(Component)]
struct SearchPlaceholder;
#[derive(Component)]
struct SearchClear;
/// Puce de filtre : toutes les catégories (`None`) ou une seule.
#[derive(Component)]
struct SearchChip(Option<Category>);
#[derive(Component)]
struct SearchResults;
/// Ligne de résultat (rang dans la liste affichée).
#[derive(Component)]
struct SearchRow(usize);

/// Un lieu de l'index de recherche.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    norm: Vec<char>,
    pub category: Category,
    pub detail: String,
    pub target: MapTarget,
    /// Pour départager à qualité de correspondance égale (débit, étendue, population).
    importance: f32,
}

/// Résultat : indice dans l'index, qualité de la correspondance, passage trouvé (en caractères).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hit {
    entry: usize,
    quality: u8,
    start: usize,
    len: usize,
}

/// État de la recherche.
#[derive(Resource, Default)]
pub struct SearchState {
    pub open: bool,
    filter: Option<Category>,
    query: String,
    entries: Vec<Entry>,
    /// Résultats de la requête dans le filtre choisi, classés.
    hits: Vec<Hit>,
    /// Nombre de résultats par catégorie (puces).
    counts: HashMap<Category, usize>,
    selected: usize,
    /// L'index est à refaire (ouverture, exploration, villes).
    stale: bool,
    /// La liste affichée est à refaire.
    dirty: bool,
}

/// Lignes de résultats montrées au plus.
const MAX_ROWS: usize = 8;

const PANEL_BG: Color = Color::srgba(0.05, 0.06, 0.09, 0.96);
const FIELD_BG: Color = Color::srgb(0.02, 0.03, 0.05);
const FIELD_BORDER: Color = Color::srgb(0.30, 0.45, 0.75);
const CHIP_BG: Color = Color::srgb(0.13, 0.15, 0.20);
const CHIP_HOVER: Color = Color::srgb(0.20, 0.23, 0.30);
const CHIP_ACTIVE: Color = Color::srgb(0.18, 0.40, 0.72);
const ROW_SELECTED: Color = Color::srgb(0.16, 0.22, 0.34);
const TEXT: Color = Color::srgb(0.92, 0.92, 0.95);
const MUTED: Color = Color::srgb(0.58, 0.61, 0.68);
const MATCH: Color = Color::srgb(1.0, 0.86, 0.40);

pub struct SearchPlugin;

impl Plugin for SearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointedTarget>()
            .init_resource::<SearchState>()
            .init_resource::<CameraFlight>()
            .add_message::<FocusTarget>()
            .init_gizmo_group::<AreaGizmos2d>()
            .add_systems(Startup, (setup_search, setup_gizmos))
            .add_systems(PreUpdate, capture_keyboard.after(InputSystems))
            .add_systems(
                Update,
                (
                    (open_search, close_search, build_index, read_query, search_keys, chip_buttons, refresh_results).chain(),
                    (target_buttons, sync_pointed_feature).chain().before(crate::highlight::update_hovered_feature),
                    focus_camera,
                    draw_pointed_area,
                ),
            );
    }
}

fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<AreaGizmos2d>();
    config.line.width = 4.0;
}

// ------------------------------------------------------------------ Noms et correspondances

/// Lettre sans accent ni majuscule (un caractère pour un caractère : les positions trouvées dans
/// le nom simplifié valent dans le vrai nom) ; tirets et apostrophes comptent comme des espaces.
pub fn fold(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' | 'À' | 'Â' | 'Ä' | 'Á' | 'Ã' | 'Å' | 'æ' | 'Æ' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'î' | 'ï' | 'í' | 'ì' | 'Î' | 'Ï' | 'Í' | 'Ì' => 'i',
        'ô' | 'ö' | 'ó' | 'ò' | 'õ' | 'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' | 'œ' | 'Œ' | 'ø' | 'Ø' => 'o',
        'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => 'u',
        'ç' | 'Ç' => 'c',
        'ÿ' | 'Ÿ' | 'ý' | 'Ý' => 'y',
        'ñ' | 'Ñ' => 'n',
        '-' | '\'' | '’' | '_' => ' ',
        c => c.to_lowercase().next().unwrap_or(c),
    }
}

pub fn normalize(s: &str) -> Vec<char> {
    s.chars().map(fold).collect()
}

/// Requête simplifiée : sans accents, espaces en trop retirés.
fn clean_query(q: &str) -> Vec<char> {
    let folded: String = q.chars().map(fold).collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ").chars().collect()
}

fn starts_with_at(name: &[char], at: usize, q: &[char]) -> bool {
    name.len() >= at + q.len() && name[at..at + q.len()] == *q
}

fn word_starts(name: &[char]) -> impl Iterator<Item = usize> + '_ {
    (0..name.len()).filter(|&i| name[i] != ' ' && (i == 0 || name[i - 1] == ' '))
}

/// Correspondance d'un nom simplifié avec une requête simplifiée : qualité (0 début du nom, 1
/// début d'un mot, 2 ailleurs ou tous les mots de la requête en début de mots du nom ; 3
/// requête vide) et passage trouvé (début, longueur, en caractères).
fn match_name(name: &[char], query: &[char]) -> Option<(u8, usize, usize)> {
    if query.is_empty() {
        return Some((3, 0, 0));
    }
    if starts_with_at(name, 0, query) {
        return Some((0, 0, query.len()));
    }
    if let Some(i) = word_starts(name).find(|&i| starts_with_at(name, i, query)) {
        return Some((1, i, query.len()));
    }
    if let Some(i) = (0..name.len()).find(|&i| starts_with_at(name, i, query)) {
        return Some((2, i, query.len()));
    }
    // Plusieurs mots : chacun au début d'un mot du nom (« lac gr » → « Grand Lac »)
    let words: Vec<&[char]> = query.split(|&c| c == ' ').filter(|w| !w.is_empty()).collect();
    if words.len() > 1 {
        let first = words.iter().map(|w| word_starts(name).find(|&i| starts_with_at(name, i, w))).collect::<Option<Vec<_>>>()?;
        return Some((2, first[0], words[0].len()));
    }
    None
}

/// Nombre avec espaces entre les milliers.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// Index des lieux connus : villes du joueur, cours d'eau et lacs dont une partie est explorée,
/// bassins versants et plaques qui ont une case explorée.
pub fn build_entries(map: &WorldMap, explored: &HashSet<Hex>, cities: &[(Entity, String, i32)]) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut push = |name: &str, category: Category, detail: String, target: MapTarget, importance: f32| {
        entries.push(Entry { name: name.to_string(), norm: normalize(name), category, detail, target, importance });
    };
    for (e, name, population) in cities {
        push(name, Category::City, format!("Ville · {population} hab."), MapTarget::City(*e), *population as f32);
    }
    let basins = map.basins();
    let known = |c: usize| explored.contains(&map.hex_at(c));
    for (r, river) in basins.rivers.iter().enumerate() {
        if !river.spots.iter().any(|&(a, b)| known(a) || known(b)) {
            continue;
        }
        let into = river.flows_into.map(|f| basins.rivers[f].name).or(river.into_lake.map(|l| basins.lakes[l].name)).unwrap_or("la mer");
        let kind = if river.fleuve { "Fleuve" } else { "Rivière" };
        let detail = format!("{kind} → {into} · {} m³/s", thousands(river.discharge.round() as u64));
        push(river.name, Category::River, detail, MapTarget::River(r), river.discharge);
    }
    for (l, lake) in basins.lakes.iter().enumerate() {
        if !lake.cells.iter().any(|&c| known(c)) {
            continue;
        }
        let mut kind = lake.kind.short().to_string();
        if let Some(first) = kind.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        let detail = format!("{kind} · {} cases", lake.cells.len());
        push(lake.name, Category::Lake, detail, MapTarget::Lake(l), lake.cells.len() as f32);
    }
    // Bassins et plaques : ceux qui ont une case explorée
    let plates = map.plates();
    let mut seen_basins: HashSet<usize> = HashSet::new();
    let mut seen_plates: HashSet<usize> = HashSet::new();
    for &h in explored {
        if !map.contains(h) {
            continue;
        }
        let c = map.cell(h);
        if let Some(b) = basins.basin_of[c] {
            seen_basins.insert(b as usize);
        }
        if let Some(&p) = plates.plate_of.get(c) {
            seen_plates.insert(p as usize);
        }
    }
    let mut seen_basins: Vec<usize> = seen_basins.into_iter().collect();
    seen_basins.sort_unstable();
    for b in seen_basins {
        let basin = &basins.basins[b];
        let Some(name) = basins.basin_name(basin) else { continue };
        let closed = if basin.closed { " fermé" } else { "" };
        let detail = format!("Bassin versant{closed} · {} cases", thousands(u64::from(basin.cells)));
        push(&format!("Bassin {name}"), Category::Basin, detail, MapTarget::Basin(b), basin.cells as f32);
    }
    let mut seen_plates: Vec<usize> = seen_plates.into_iter().collect();
    seen_plates.sort_unstable();
    for p in seen_plates {
        let Some(plate) = plates.plates.get(p) else { continue };
        let kind = match plate.kind {
            crate::plates::PlateKind::Continental => "Plaque continentale",
            crate::plates::PlateKind::Oceanic => "Plaque océanique",
        };
        let detail = format!("{kind} · {} cases", thousands(plate.cells as u64));
        push(&format!("Plaque {}", plate.name), Category::Plate, detail, MapTarget::Plate(p), plate.cells as f32);
    }
    entries
}

/// Résultats classés d'une requête dans un filtre, et nombre de résultats par catégorie.
fn run_query(entries: &[Entry], query: &str, filter: Option<Category>) -> (Vec<Hit>, HashMap<Category, usize>) {
    let q = clean_query(query);
    let mut counts: HashMap<Category, usize> = HashMap::new();
    let mut hits: Vec<Hit> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let Some((quality, start, len)) = match_name(&e.norm, &q) else { continue };
        *counts.entry(e.category).or_default() += 1;
        if filter.is_none_or(|f| f == e.category) {
            hits.push(Hit { entry: i, quality, start, len });
        }
    }
    let order = |c: Category| Category::ALL.iter().position(|&x| x == c).unwrap_or(0);
    hits.sort_by(|a, b| {
        let (ea, eb) = (&entries[a.entry], &entries[b.entry]);
        a.quality
            .cmp(&b.quality)
            .then(order(ea.category).cmp(&order(eb.category)))
            .then(eb.importance.total_cmp(&ea.importance))
            .then(ea.name.cmp(&eb.name))
    });
    (hits, counts)
}

// ------------------------------------------------------------------ Zones et cadrage

/// Cases connues d'un lieu (explorées ; le territoire d'une ville en entier).
fn target_hexes(map: &WorldMap, fog: &ShownFog, territory: &Territory, city_hex: Option<Hex>, target: MapTarget) -> Vec<Hex> {
    let explored = |h: &Hex| fog.explored.contains(h);
    let basins = map.basins();
    let mut hexes: Vec<Hex> = match target {
        MapTarget::City(e) => territory.0.iter().filter(|(_, owner)| **owner == e).map(|(h, _)| *h).chain(city_hex).collect(),
        MapTarget::River(r) => basins.rivers.get(r).map_or_else(Vec::new, |river| {
            river.spots.iter().flat_map(|&(a, b)| [a, b]).map(|c| map.hex_at(c)).filter(explored).collect()
        }),
        MapTarget::Lake(l) => basins.lakes.get(l).map_or_else(Vec::new, |lake| lake.cells.iter().map(|&c| map.hex_at(c)).filter(explored).collect()),
        MapTarget::Basin(b) => fog.explored.iter().copied().filter(|&h| map.contains(h) && basins.basin_of[map.cell(h)] == Some(b as u32)).collect(),
        MapTarget::Plate(p) => {
            let plate_of = &map.plates().plate_of;
            fog.explored.iter().copied().filter(|&h| map.contains(h) && plate_of.get(map.cell(h)).is_some_and(|&q| q as usize == p)).collect()
        }
    };
    hexes.sort_unstable_by_key(|h| (h.q, h.r));
    hexes.dedup();
    hexes
}

/// Centre et demi-étendue d'un ensemble de points (planète de largeur `world` > 0 : un ensemble
/// à cheval sur le raccord est pris d'un seul tenant).
pub fn area_bounds(points: &[Vec2], world: f32) -> Option<(Vec2, Vec2)> {
    let first = *points.first()?;
    let reference = if world > 0.0 {
        let (s, c) = points.iter().fold((0.0f32, 0.0f32), |(s, c), p| {
            let a = p.x / world * std::f32::consts::TAU;
            (s + a.sin(), c + a.cos())
        });
        if s.hypot(c) < 1e-3 * points.len() as f32 { first.x } else { s.atan2(c) / std::f32::consts::TAU * world }
    } else {
        first.x
    };
    let dx = |x: f32| if world > 0.0 { x - reference - ((x - reference) / world).round() * world } else { x - reference };
    let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for p in points {
        let q = Vec2::new(dx(p.x), p.y);
        min = min.min(q);
        max = max.max(q);
    }
    let mut center = Vec2::new(reference + (min.x + max.x) / 2.0, (min.y + max.y) / 2.0);
    if world > 0.0 {
        center.x = center.x.rem_euclid(world);
    }
    Some((center, (max - min) / 2.0))
}

/// Vol de la caméra vers un lieu : centre et demi-étendue (monde 2D).
#[derive(Resource, Default)]
struct CameraFlight(Option<Flight>);

struct Flight {
    center: Vec2,
    half: Vec2,
    elapsed: f32,
}

/// Amène la caméra (2D ou 3D) sur le lieu demandé, en douceur ; une action de l'utilisateur sur
/// la caméra interrompt le vol.
#[allow(clippy::too_many_arguments)]
fn focus_camera(
    mut requests: MessageReader<FocusTarget>,
    mut flight: ResMut<CameraFlight>,
    mut pointed: ResMut<PointedTarget>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    territory: Res<Territory>,
    cities: Query<&HexPos, With<City>>,
    mode: Res<ViewMode>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut cam3d: ResMut<Cam3d>,
    camera2d: Single<(&mut Transform, &mut Projection), (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
) {
    for &FocusTarget(target) in requests.read() {
        let city_hex = match target {
            MapTarget::City(e) => cities.get(e).ok().map(|p| p.0),
            _ => None,
        };
        let points: Vec<Vec2> = target_hexes(&map.0, &fog, &territory, city_hex, target).into_iter().map(|h| Vec2::from(h.to_world())).collect();
        if let Some((center, half)) = area_bounds(&points, map.0.wrap().world_width()) {
            flight.0 = Some(Flight { center, half: half + Vec2::splat(HEX_SIZE), elapsed: 0.0 });
            pointed.flash = Some((target, FLASH_SECONDS));
        }
    }
    let Some(f) = flight.0.as_mut() else { return };
    let moving = [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD, KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight];
    if keys.any_pressed(moving) || mouse.pressed(MouseButton::Middle) || scroll.delta.y != 0.0 {
        flight.0 = None;
        return;
    }
    let dt = time.delta_secs();
    f.elapsed += dt;
    let k = 1.0 - (-dt * 7.0).exp();
    let world = map.0.wrap().world_width();
    // Copie du centre la plus proche (planète)
    let near = |x: f32, from: f32| if world > 0.0 { x + ((from - x) / world).round() * world } else { x };
    let (w, h) = window.map_or((1400.0, 860.0), |w| (w.width(), w.height()));
    let mut arrived = f.elapsed > 2.5;
    if mode.is_3d {
        let target = Vec3::new(near(f.center.x, cam3d.target.x), 0.0, -f.center.y);
        let distance = (f.half.y.max(f.half.x * h / w) * 2.4 + 150.0).clamp(250.0, 2_800.0);
        let target_now = cam3d.target.lerp(target, k);
        let distance_now = cam3d.distance + (distance - cam3d.distance) * k;
        arrived |= target_now.distance(target) < 1.0 && (distance_now - distance).abs() < 1.0;
        cam3d.target = target_now;
        cam3d.distance = distance_now;
    } else {
        let (mut transform, mut projection) = camera2d.into_inner();
        let Projection::Orthographic(ortho) = projection.as_mut() else { return };
        // Le lieu occupe environ 60 % de la vue (l'interface couvre les bords)
        let scale = (f.half.x * 2.0 / (w * 0.6)).max(f.half.y * 2.0 / (h * 0.6)).clamp(0.6, 8.0);
        let goal = Vec2::new(near(f.center.x, transform.translation.x), f.center.y);
        let at = transform.translation.truncate().lerp(goal, k);
        let new_scale = (ortho.scale.ln() + (scale.ln() - ortho.scale.ln()) * k).exp();
        arrived |= at.distance(goal) < 0.5 && (new_scale / scale - 1.0).abs() < 0.005;
        transform.translation.x = at.x;
        transform.translation.y = at.y;
        ortho.scale = new_scale;
    }
    if arrived {
        flight.0 = None;
    }
}

/// Survol et clic des boutons qui mènent à un lieu ; couleur de fond au survol.
fn target_buttons(
    time: Res<Time>,
    mut pointed: ResMut<PointedTarget>,
    mut selection: ResMut<Selection>,
    mut focus: MessageWriter<FocusTarget>,
    buttons: Query<(Ref<Interaction>, &TargetButton, Option<&InheritedVisibility>)>,
    mut colored: Query<(&Interaction, &HoverColors, &mut BackgroundColor), Without<SearchRow>>,
) {
    let mut hover = None;
    for (interaction, button, visible) in &buttons {
        if visible.is_some_and(|v| !v.get()) {
            continue;
        }
        if button.hover && *interaction != Interaction::None {
            hover = Some(button.target);
        }
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            focus.write(FocusTarget(button.target));
            if let MapTarget::City(e) = button.target {
                selection.0 = Some(e);
            }
        }
    }
    if pointed.hover != hover {
        pointed.hover = hover;
    }
    if let Some((_, left)) = pointed.flash.as_mut() {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            pointed.flash = None;
        }
    }
    for (interaction, colors, mut bg) in &mut colored {
        let c = if *interaction == Interaction::None { colors.normal } else { colors.hover };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

/// Le lieu mis en avant passe à la surbrillance du réseau hydrographique.
fn sync_pointed_feature(pointed: Res<PointedTarget>, map: Res<GameMap>, mut feature: ResMut<PointedFeature>) {
    let wanted = pointed.current().and_then(|t| t.feature(map.0.basins()));
    if feature.0 != wanted {
        feature.0 = wanted;
    }
}

/// Arêtes du pourtour d'un ensemble de cases (monde 2D), un peu en retrait vers l'intérieur.
fn outline(map: &WorldMap, hexes: &[Hex]) -> Vec<[Vec2; 2]> {
    let set: HashSet<Hex> = hexes.iter().copied().collect();
    let mut edges = Vec::new();
    for &hex in hexes {
        let center = Vec2::from(hex.to_world());
        for (d, &step) in DIRECTIONS.iter().enumerate() {
            let n = map.canonical(hex + step);
            if map.contains(n) && set.contains(&n) {
                continue;
            }
            let (a, b) = hex.edge_corners(d);
            edges.push([Vec2::from(a).lerp(center, 0.08), Vec2::from(b).lerp(center, 0.08)]);
        }
    }
    edges
}

/// Pourtour doré (qui pulse) de la zone mise en avant : partie connue d'un bassin, d'une plaque,
/// territoire d'une ville.
#[allow(clippy::too_many_arguments)]
fn draw_pointed_area(
    pointed: Res<PointedTarget>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    territory: Res<Territory>,
    cities: Query<&HexPos, With<City>>,
    mode: Res<ViewMode>,
    relief: Option<Res<Relief>>,
    time: Res<Time>,
    mut cache: Local<Option<(MapTarget, Vec<[Vec2; 2]>)>>,
    mut gizmos_2d: Gizmos<AreaGizmos2d>,
    mut gizmos_3d: Gizmos<Gizmos3d>,
) {
    let Some(target) = pointed.current().filter(|t| t.outlined()) else {
        *cache = None;
        return;
    };
    if cache.as_ref().is_none_or(|(t, _)| *t != target) || fog.is_changed() || territory.is_changed() {
        let city_hex = match target {
            MapTarget::City(e) => cities.get(e).ok().map(|p| p.0),
            _ => None,
        };
        let hexes = target_hexes(&map.0, &fog, &territory, city_hex, target);
        *cache = Some((target, outline(&map.0, &hexes)));
    }
    let Some((_, edges)) = cache.as_ref() else { return };
    let pulse = 0.6 + 0.4 * (time.elapsed_secs() * 5.0).sin().abs();
    let color = OUTLINE_COLOR.with_alpha(pulse);
    if mode.is_3d {
        let Some(relief) = relief else { return };
        for [a, b] in edges {
            let m = a.lerp(*b, 0.5);
            gizmos_3d.linestrip([a, &m, b].map(|p| relief.on_ground(p.x, p.y, 2.0)), color);
        }
    } else {
        for [a, b] in edges {
            gizmos_2d.line_2d(*a, *b, color);
        }
    }
}

// ------------------------------------------------------------------ Saisie au clavier

/// Pendant la saisie dans un champ, les touches ne servent pas de raccourcis au jeu (Échap
/// passe : il ferme la recherche, ou le menu). Échap ferme la recherche ouverte.
fn capture_keyboard(
    focus: Res<InputFocus>,
    editables: Query<(), With<EditableText>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut search: ResMut<SearchState>,
) {
    if search.open && keys.just_pressed(KeyCode::Escape) {
        search.open = false;
        keys.reset(KeyCode::Escape);
    }
    if focus.get().is_some_and(|e| editables.contains(e)) {
        let escape = keys.just_pressed(KeyCode::Escape);
        keys.reset_all();
        if escape {
            keys.press(KeyCode::Escape);
        }
    }
}

// ------------------------------------------------------------------ Panneau de recherche

fn text(s: &str, size: f32, color: Color) -> impl Bundle + use<> {
    (Text::new(s.to_string()), TextFont::from_font_size(size), TextColor(color))
}

fn chip(filter: Option<Category>) -> impl Bundle {
    (
        SearchChip(filter),
        Button,
        Node {
            padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(11.0)),
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        },
        BackgroundColor(CHIP_BG),
        children![text("", 13.0, TEXT)],
    )
}

/// Loupe dessinée (un anneau et un manche).
fn magnifier() -> impl Bundle {
    (
        Node { width: Val::Px(16.0), height: Val::Px(16.0), flex_shrink: 0.0, ..default() },
        children![
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(11.0),
                    height: Val::Px(11.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(MUTED),
            ),
            (
                Node { position_type: PositionType::Absolute, left: Val::Px(9.0), top: Val::Px(11.0), width: Val::Px(7.0), height: Val::Px(2.5), ..default() },
                UiTransform { rotation: Rot2::degrees(45.0), ..default() },
                BackgroundColor(MUTED),
            ),
        ],
    )
}

fn setup_search(mut commands: Commands) {
    commands.spawn((
        SearchRoot,
        Node {
            position_type: PositionType::Absolute,
            // Sous la barre du haut
            top: Val::Px(50.0),
            left: Val::Percent(50.0),
            width: Val::Px(500.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            display: Display::None,
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
        BackgroundColor(PANEL_BG),
        BorderColor::all(Color::srgb(0.20, 0.23, 0.30)),
        Interaction::default(),
        GlobalZIndex(60),
        children![
            // Champ : loupe, saisie (et texte d'aide quand elle est vide), bouton pour effacer
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(FIELD_BG),
                BorderColor::all(FIELD_BORDER),
                children![
                    magnifier(),
                    (
                        Node { flex_grow: 1.0, ..default() },
                        children![
                            (
                                SearchPlaceholder,
                                text("Ville, fleuve, lac, bassin, plaque…", 16.0, MUTED),
                                Node { position_type: PositionType::Absolute, left: Val::Px(0.0), ..default() },
                            ),
                            (
                                SearchInput,
                                EditableText { max_characters: Some(40), ..default() },
                                TextFont::from_font_size(16.0),
                                TextColor(TEXT),
                                TextLayout::no_wrap(),
                                TextCursorStyle { color: Color::WHITE, selection_color: Color::srgb(0.20, 0.40, 0.75), ..default() },
                                Node { width: Val::Percent(100.0), ..default() },
                            ),
                        ],
                    ),
                    (
                        SearchClear,
                        Button,
                        Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                        BackgroundColor(Color::NONE),
                        HoverColors { normal: Color::NONE, hover: CHIP_HOVER },
                        children![text("×", 18.0, MUTED)],
                    ),
                ],
            ),
            // Filtres
            (
                Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), ..default() },
                children![
                    chip(None),
                    chip(Some(Category::City)),
                    chip(Some(Category::River)),
                    chip(Some(Category::Lake)),
                    chip(Some(Category::Basin)),
                    chip(Some(Category::Plate)),
                ],
            ),
            (SearchResults, Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() }),
            (
                Text::new("↑ ↓ choisir · Entrée : y aller · Tab : filtre suivant · Échap : fermer"),
                TextFont::from_font_size(11.5),
                TextColor(MUTED),
            ),
        ],
    ));
}

/// Ouverture : Ctrl+F ou le bouton. Le champ prend la main, son texte est sélectionné (on peut
/// retaper directement).
#[allow(clippy::too_many_arguments)]
fn open_search(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Res<crate::menu::GameMenu>,
    mut search: ResMut<SearchState>,
    mut focus: ResMut<InputFocus>,
    mut button: Query<(Ref<Interaction>, &mut BackgroundColor), With<SearchButton>>,
    mut input: Single<(Entity, &mut EditableText), With<SearchInput>>,
    mut root: Single<&mut Node, With<SearchRoot>>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    let mut open = ctrl && keys.just_pressed(KeyCode::KeyF);
    for (interaction, mut bg) in &mut button {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            open = !search.open;
            if !open {
                search.open = false;
            }
        }
        let c = if *interaction == Interaction::None { Color::srgba(0.16, 0.18, 0.24, 0.95) } else { Color::srgb(0.24, 0.28, 0.38) };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    if open && !menu.open {
        let (entity, ref mut editable) = *input;
        if !search.open {
            search.open = true;
            search.stale = true;
            search.selected = 0;
        }
        focus.set(entity, FocusCause::Navigated);
        editable.queue_edit(TextEdit::SelectAll);
    }
    let display = if search.open { Display::Flex } else { Display::None };
    if root.display != display {
        root.display = display;
    }
}

/// Fermeture : clic sur la carte (hors interface), menu ouvert ; le champ perd la main.
fn close_search(
    mouse: Res<ButtonInput<MouseButton>>,
    over_ui: Res<PointerOverUi>,
    menu: Res<crate::menu::GameMenu>,
    mut search: ResMut<SearchState>,
    mut focus: ResMut<InputFocus>,
    mut pointed: ResMut<PointedTarget>,
    input: Single<Entity, With<SearchInput>>,
) {
    if search.open && ((mouse.just_pressed(MouseButton::Left) && !over_ui.0) || menu.open) {
        search.open = false;
    }
    if !search.open {
        if focus.get() == Some(*input) {
            focus.clear();
        }
        if pointed.keyboard.is_some() {
            pointed.keyboard = None;
        }
    }
}

/// Index refait à l'ouverture, puis quand l'exploration, le mode debug ou les villes changent.
fn build_index(
    mut search: ResMut<SearchState>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    debug: Res<DebugMode>,
    cities: Query<(Entity, &City)>,
    changed_cities: Query<(), Changed<City>>,
    mut was_open: Local<bool>,
) {
    let opened = search.open && !*was_open;
    *was_open = search.open;
    if !search.open {
        return;
    }
    if !(opened || search.stale || fog.is_changed() || debug.is_changed() || !changed_cities.is_empty()) {
        return;
    }
    let mut list: Vec<(Entity, String, i32)> = cities.iter().map(|(e, c)| (e, c.state.name.clone(), c.state.population)).collect();
    list.sort_by(|a, b| a.1.cmp(&b.1));
    search.entries = build_entries(&map.0, &fog.explored, &list);
    search.stale = false;
    let (hits, counts) = run_query(&search.entries, &search.query, search.filter);
    search.hits = hits;
    search.counts = counts;
    search.selected = search.selected.min(search.hits.len().saturating_sub(1));
    search.dirty = true;
}

/// Texte saisi : nouveaux résultats.
fn read_query(mut search: ResMut<SearchState>, input: Single<&EditableText, With<SearchInput>>, mut placeholder: Single<&mut Visibility, With<SearchPlaceholder>>) {
    let value = input.value().to_string();
    let shown = if value.is_empty() { Visibility::Inherited } else { Visibility::Hidden };
    if **placeholder != shown {
        **placeholder = shown;
    }
    if !search.open || value == search.query {
        return;
    }
    let (hits, counts) = run_query(&search.entries, &value, search.filter);
    search.query = value;
    search.hits = hits;
    search.counts = counts;
    search.selected = 0;
    search.dirty = true;
}

/// Flèches, Entrée, Tab dans la recherche ; bouton pour effacer.
#[allow(clippy::too_many_arguments)]
fn search_keys(
    keys: Res<ButtonInput<Key>>,
    mut search: ResMut<SearchState>,
    mut focus_msg: MessageWriter<FocusTarget>,
    mut selection: ResMut<Selection>,
    mut focus: ResMut<InputFocus>,
    mut input: Single<(Entity, &mut EditableText), With<SearchInput>>,
    clear: Query<&Interaction, (Changed<Interaction>, With<SearchClear>)>,
) {
    if !search.open {
        return;
    }
    let (entity, ref mut editable) = *input;
    if clear.iter().any(|i| *i == Interaction::Pressed) {
        editable.queue_edit(TextEdit::SelectAll);
        editable.queue_edit(TextEdit::Backspace);
        focus.set(entity, FocusCause::Navigated);
    }
    let rows = search.hits.len().min(MAX_ROWS);
    if rows > 0 {
        if keys.just_pressed(Key::ArrowDown) {
            search.selected = (search.selected + 1) % rows;
            search.dirty = true;
        }
        if keys.just_pressed(Key::ArrowUp) {
            search.selected = (search.selected + rows - 1) % rows;
            search.dirty = true;
        }
    }
    if keys.just_pressed(Key::ArrowDown) || keys.just_pressed(Key::ArrowUp) {
        // (le champ d'une ligne y a vu un déplacement du curseur : on le remet au bout)
        editable.queue_edit(TextEdit::TextEnd(false));
    }
    if keys.just_pressed(Key::Tab) {
        let all: Vec<Option<Category>> = std::iter::once(None).chain(Category::ALL.map(Some)).collect();
        let at = all.iter().position(|&f| f == search.filter).unwrap_or(0);
        let next = if keys.pressed(Key::Shift) { (at + all.len() - 1) % all.len() } else { (at + 1) % all.len() };
        set_filter(&mut search, all[next]);
    }
    if keys.just_pressed(Key::Enter)
        && let Some(hit) = search.hits.get(search.selected)
    {
        let target = search.entries[hit.entry].target;
        focus_msg.write(FocusTarget(target));
        if let MapTarget::City(e) = target {
            selection.0 = Some(e);
        }
        search.open = false;
    }
}

fn set_filter(search: &mut SearchState, filter: Option<Category>) {
    search.filter = filter;
    let (hits, counts) = run_query(&search.entries, &search.query, filter);
    search.hits = hits;
    search.counts = counts;
    search.selected = 0;
    search.dirty = true;
}

/// Puces de filtre : clic pour filtrer (le champ garde la main).
fn chip_buttons(
    mut search: ResMut<SearchState>,
    mut focus: ResMut<InputFocus>,
    input: Single<Entity, With<SearchInput>>,
    mut chips: Query<(Ref<Interaction>, &SearchChip, &mut BackgroundColor, &Children)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    if !search.open {
        return;
    }
    for (interaction, chip, _, _) in &chips {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            set_filter(&mut search, chip.0);
            focus.set(*input, FocusCause::Navigated);
        }
    }
    for (interaction, chip, mut bg, children) in &mut chips {
        let count = match chip.0 {
            None => search.counts.values().sum(),
            Some(c) => search.counts.get(&c).copied().unwrap_or(0),
        };
        let active = search.filter == chip.0;
        let color = if active {
            CHIP_ACTIVE
        } else if *interaction != Interaction::None {
            CHIP_HOVER
        } else {
            CHIP_BG
        };
        if bg.0 != color {
            bg.0 = color;
        }
        let label = format!("{} {count}", chip.0.map_or("Tout", Category::label));
        if let Some(Ok((mut t, mut c))) = children.first().map(|&e| texts.get_mut(e)) {
            if t.0 != label {
                t.0 = label;
            }
            let tc = if count == 0 && !active { MUTED } else { TEXT };
            if c.0 != tc {
                c.0 = tc;
            }
        }
    }
}

/// Liste des résultats : refaite quand la requête, le filtre ou l'index changent ; la ligne
/// survolée devient la ligne choisie ; la ligne choisie est mise en avant sur la carte.
fn refresh_results(
    mut commands: Commands,
    mut search: ResMut<SearchState>,
    mut pointed: ResMut<PointedTarget>,
    list: Single<(Entity, Option<&Children>), With<SearchResults>>,
    mut rows: Query<(Ref<Interaction>, &SearchRow, &mut BackgroundColor)>,
) {
    if !search.open {
        return;
    }
    // Clic sur une ligne : la caméra y va (`target_buttons`), la recherche se ferme
    if rows.iter().any(|(i, ..)| i.is_changed() && *i == Interaction::Pressed) {
        search.open = false;
        return;
    }
    // Survol d'une ligne : elle devient la ligne choisie
    if let Some((_, row, _)) = rows.iter().find(|(i, ..)| **i != Interaction::None)
        && search.selected != row.0
    {
        search.selected = row.0;
    }
    let keyboard = search.hits.get(search.selected).map(|h| search.entries[h.entry].target);
    if pointed.keyboard != keyboard {
        pointed.keyboard = keyboard;
    }
    for (_, row, mut bg) in &mut rows {
        let c = if row.0 == search.selected { ROW_SELECTED } else { Color::NONE };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    if !search.dirty {
        return;
    }
    search.dirty = false;
    let (entity, children) = *list;
    for &child in children.into_iter().flatten() {
        commands.entity(child).despawn();
    }
    if search.hits.is_empty() {
        let message = if search.query.trim().is_empty() {
            "Aucun lieu connu dans cette catégorie.".to_string()
        } else {
            format!("Aucun lieu connu ne correspond à « {} ».", search.query.trim())
        };
        commands.entity(entity).with_children(|p| {
            p.spawn((text(&message, 14.0, TEXT), Node { margin: UiRect::axes(Val::Px(4.0), Val::Px(4.0)), ..default() }));
            p.spawn((text("Seuls les lieux déjà explorés sont listés. Essayez un autre filtre ou le début du nom.", 12.0, MUTED), Node { margin: UiRect::horizontal(Val::Px(4.0)), ..default() }));
        });
        return;
    }
    if search.query.trim().is_empty() {
        commands.entity(entity).with_child((text("Lieux connus", 12.0, MUTED), Node { margin: UiRect::axes(Val::Px(4.0), Val::Px(2.0)), ..default() }));
    }
    for (rank, hit) in search.hits.iter().take(MAX_ROWS).enumerate() {
        let e = &search.entries[hit.entry];
        let chars: Vec<char> = e.name.chars().collect();
        let part = |a: usize, b: usize| chars[a.min(chars.len())..b.min(chars.len())].iter().collect::<String>();
        let (before, found, after) = (part(0, hit.start), part(hit.start, hit.start + hit.len), part(hit.start + hit.len, chars.len()));
        let bg = if rank == search.selected { ROW_SELECTED } else { Color::NONE };
        commands.entity(entity).with_children(|p| {
            p.spawn((
                SearchRow(rank),
                TargetButton::new(e.target),
                Button,
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(9.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(bg),
            ))
            .with_children(|row| {
                row.spawn((
                    Node { width: Val::Px(9.0), height: Val::Px(9.0), border_radius: BorderRadius::MAX, flex_shrink: 0.0, ..default() },
                    BackgroundColor(e.category.color()),
                ));
                row.spawn((Text::new(before), TextFont::from_font_size(15.0), TextColor(TEXT), Node { flex_shrink: 0.0, ..default() })).with_children(|t| {
                    t.spawn((TextSpan::new(found), TextFont::from_font_size(15.0).with_font_weight(FontWeight::BOLD), TextColor(MATCH)));
                    t.spawn((TextSpan::new(after), TextFont::from_font_size(15.0), TextColor(TEXT)));
                });
                row.spawn(Node { flex_grow: 1.0, ..default() });
                row.spawn(text(&e.detail, 12.0, MUTED));
            });
        });
    }
    if search.hits.len() > MAX_ROWS {
        let more = search.hits.len() - MAX_ROWS;
        let s = if more > 1 { "s" } else { "" };
        commands.entity(entity).with_child((
            text(&format!("… et {more} autre{s} résultat{s} : précisez le nom ou choisissez un filtre", ), 12.0, MUTED),
            Node { margin: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), ..default() },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(name: &str, query: &str) -> Option<(u8, usize, usize)> {
        match_name(&normalize(name), &clean_query(query))
    }

    #[test]
    fn correspondances_sans_accents_ni_majuscules() {
        assert_eq!(q("Rhône", "rho"), Some((0, 0, 3)));
        assert_eq!(q("Rhône", "RHÔ"), Some((0, 0, 3)));
        assert_eq!(q("Grand Lac Salé", "sale"), Some((1, 10, 4)));
        assert_eq!(q("Saint-Étienne", "etienne"), Some((1, 6, 7)));
        assert_eq!(q("Garonne", "ron"), Some((2, 2, 3)));
        // Mots dans le désordre : chacun au début d'un mot
        assert_eq!(q("Grand Lac Salé", "lac gr"), Some((2, 6, 3)));
        assert_eq!(q("Loire", "seine"), None);
        assert_eq!(q("Loire", "  "), Some((3, 0, 0)));
    }

    #[test]
    fn classement_debut_puis_mot_puis_ailleurs() {
        let e = |name: &str, category: Category, importance: f32| Entry {
            name: name.into(),
            norm: normalize(name),
            category,
            detail: String::new(),
            target: MapTarget::River(0),
            importance,
        };
        let entries = vec![e("Arloire", Category::River, 9.0), e("Loire", Category::River, 1.0), e("Haute Loire", Category::River, 5.0), e("Loiret", Category::Lake, 3.0)];
        let (hits, counts) = run_query(&entries, "loire", None);
        let names: Vec<&str> = hits.iter().map(|h| entries[h.entry].name.as_str()).collect();
        assert_eq!(names, ["Loire", "Loiret", "Haute Loire", "Arloire"]);
        assert_eq!(counts[&Category::River], 3);
        assert_eq!(counts[&Category::Lake], 1);
        // Filtre : seulement les lacs, les nombres par catégorie restent
        let (hits, counts) = run_query(&entries, "loire", Some(Category::Lake));
        assert_eq!(hits.len(), 1);
        assert_eq!(counts[&Category::River], 3);
    }

    #[test]
    fn cadrage_a_cheval_sur_le_raccord() {
        let world = 1000.0;
        let (c, half) = area_bounds(&[Vec2::new(10.0, 0.0), Vec2::new(990.0, 20.0)], world).unwrap();
        assert!(c.x < 1.0 || c.x > 999.0, "centre {c}");
        assert!((half.x - 10.0).abs() < 0.01 && (half.y - 10.0).abs() < 0.01, "{half}");
        // Carte sans raccord : boîte ordinaire
        let (c, half) = area_bounds(&[Vec2::new(10.0, 0.0), Vec2::new(990.0, 20.0)], 0.0).unwrap();
        assert_eq!((c, half), (Vec2::new(500.0, 10.0), Vec2::new(490.0, 10.0)));
        assert!(area_bounds(&[], world).is_none());
    }

    #[test]
    fn index_des_lieux_connus() {
        let m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        let none = build_entries(&m, &HashSet::new(), &[]);
        assert!(none.is_empty(), "rien d'exploré : rien à chercher");
        let all: HashSet<Hex> = m.hexes().collect();
        let entries = build_entries(&m, &all, &[]);
        let count = |c: Category| entries.iter().filter(|e| e.category == c).count();
        assert_eq!(count(Category::River), m.basins().rivers.len());
        assert_eq!(count(Category::Lake), m.basins().lakes.len());
        assert!(count(Category::Plate) > 3);
        assert!(count(Category::Basin) > 0);
        // Chaque nom se retrouve par son début
        for e in &entries {
            let (hits, _) = run_query(&entries, &e.name[..e.name.char_indices().nth(4).map_or(e.name.len(), |(i, _)| i)], None);
            assert!(hits.iter().any(|h| entries[h.entry].target == e.target), "{}", e.name);
        }
    }
}
