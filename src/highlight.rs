//! Surbrillance d'un cours d'eau ou d'un lac : son tracé en rouge, ses affluents directs en
//! orange, leurs propres affluents (de proche en proche) en jaune, le cours d'eau ou le lac où il
//! se jette en bleu, et la suite du chemin de l'eau jusqu'à la mer en violet (en 2D comme en 3D,
//! sur les parties explorées).
//!
//! Carte du réseau hydrographique : tous les cours d'eau et lacs connus, colorés selon leur rang
//! par rapport à leur déversoir final, avec le sens du courant.
//!
//! Les tracés sont reliés à leurs confluents (un affluent rejoint le lit du cours où il se jette),
//! et des chevrons qui descendent le courant en montrent le sens.
//!
//! Selon l'option choisie, la surbrillance suit le survol du cours d'eau sur la carte, le survol
//! de son étiquette, ou rien ; un élément montré depuis l'interface (recherche, légende) passe
//! avant.

use crate::basins::{Basins, CoursePoint, NamedRiver};
use crate::game::{GameMap, ShownFog};
use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::input::HoveredHex;
use crate::map::WorldMap;
use crate::render::MapLayer;
use crate::view3d::{Gizmos3d, Relief, ViewMode};
use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};

/// Couleurs du tracé.
pub const HOVERED_COLOR: Color = Color::srgb(1.0, 0.22, 0.18);
pub const TRIBUTARY_COLOR: Color = Color::srgb(1.0, 0.62, 0.12);
pub const DEEP_TRIBUTARY_COLOR: Color = Color::srgb(1.0, 0.92, 0.25);
pub const RECEIVER_COLOR: Color = Color::srgb(0.35, 0.85, 1.0);
pub const DOWNSTREAM_COLOR: Color = Color::srgb(0.72, 0.45, 1.0);

/// Carte du réseau : couleur selon le rang par rapport au déversoir final (0 : se jette dans la
/// mer ou finit dans un lac salé, une mer intérieure ; 1 : affluent d'un rang 0…).
pub const LEVEL_COLORS: [(f32, f32, f32); 5] =
    [(0.10, 0.22, 0.62), (0.13, 0.50, 0.85), (0.08, 0.66, 0.64), (0.36, 0.68, 0.26), (0.66, 0.60, 0.16)];

pub fn level_color(level: u8) -> Color {
    let (r, g, b) = LEVEL_COLORS[(level as usize).min(LEVEL_COLORS.len() - 1)];
    Color::srgb(r, g, b)
}
/// Élément nommé du réseau hydrographique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    /// Cours d'eau (indice dans `Basins::rivers`).
    River(usize),
    /// Lac (indice dans `Basins::lakes`).
    Lake(usize),
}

impl Feature {
    pub fn name(self, basins: &Basins) -> &str {
        match self {
            Feature::River(r) => &basins.rivers[r].name,
            Feature::Lake(l) => &basins.lakes[l].name,
        }
    }
}

/// Comportement de la surbrillance.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightMode {
    /// Au survol du cours d'eau ou du lac sur la carte.
    #[default]
    OnFeature,
    /// Au survol de son étiquette.
    OnLabel,
    /// Jamais.
    Off,
}

/// Élément mis en surbrillance (survolé sur la carte ou par son étiquette, selon l'option).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct HoveredFeature(pub Option<Feature>);

/// Élément dont l'étiquette est sous la souris (tenu à jour par les étiquettes).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct HoveredLabel(pub Option<Feature>);

/// Élément montré depuis l'interface (résultat de recherche, ligne de légende survolée) : il
/// passe avant le survol de la carte, quelle que soit l'option de surbrillance.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct PointedFeature(pub Option<Feature>);

/// Lignes de surbrillance en 2D (plus épaisses que les lignes ordinaires).
#[derive(Default, Reflect, GizmoConfigGroup)]
struct RiverGizmos2d;
/// Carte du réseau en 2D : fleuves, puis rivières et pourtours des lacs.
#[derive(Default, Reflect, GizmoConfigGroup)]
struct NetworkGizmos2d;
#[derive(Default, Reflect, GizmoConfigGroup)]
struct StreamGizmos2d;

pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredFeature>()
            .init_resource::<HoveredLabel>()
            .init_resource::<PointedFeature>()
            .init_resource::<HighlightMode>()
            .init_gizmo_group::<RiverGizmos2d>()
            .init_gizmo_group::<NetworkGizmos2d>()
            .init_gizmo_group::<StreamGizmos2d>()
            .add_systems(Startup, setup_gizmos)
            .add_systems(Update, (update_hovered_feature, draw_network_map, draw_highlight).chain());
    }
}

fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<RiverGizmos2d>();
    config.line.width = 4.0;
    let (config, _) = store.config_mut::<NetworkGizmos2d>();
    config.line.width = 3.5;
    let (config, _) = store.config_mut::<StreamGizmos2d>();
    config.line.width = 2.0;
}

/// Cours d'eau ou lac nommé d'une case : le lac qui l'occupe, le fleuve qui l'occupe, sinon la
/// plus abondante des rivières nommées de ses arêtes.
pub fn feature_at(map: &WorldMap, hex: Hex) -> Option<Feature> {
    if !map.contains(hex) {
        return None;
    }
    let basins = map.basins();
    let cell = map.cell(hex);
    if let Some(l) = basins.lake_of[cell] {
        return Some(Feature::Lake(l as usize));
    }
    if let Some(r) = basins.river_of[cell] {
        return Some(Feature::River(r as usize));
    }
    (0..6)
        .filter(|&d| map.river_flow(hex, d) > 0.0)
        .filter_map(|d| basins.river_on_edge.get(&(cell as u32, d as u8)).map(|&r| (map.river_flow(hex, d), r as usize)))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, r)| Feature::River(r))
}

/// Un élément est connu dès qu'une partie en est explorée.
pub fn known(map: &WorldMap, fog: &ShownFog, feature: Feature) -> bool {
    let explored = |c: usize| fog.explored.contains(&map.hex_at(c));
    match feature {
        Feature::River(r) => map.basins().rivers[r].spots.iter().any(|&(a, b)| explored(a) || explored(b)),
        Feature::Lake(l) => map.basins().lakes[l].cells.iter().any(|&c| explored(c)),
    }
}

/// Où se jette un élément : un cours d'eau dans un autre ou dans un lac, un lac dans le cours
/// d'eau qui en sort.
pub fn receiver(basins: &Basins, feature: Feature) -> Option<Feature> {
    match feature {
        Feature::River(r) => {
            let river = &basins.rivers[r];
            river.flows_into.map(Feature::River).or(river.into_lake.map(Feature::Lake))
        }
        Feature::Lake(l) => basins.lakes[l].outflow.map(Feature::River),
    }
}

/// Réseau d'un élément : le récepteur, la suite du chemin de l'eau au-delà, les affluents
/// directs et, de proche en proche, les affluents des affluents.
#[derive(Debug, Default, PartialEq)]
pub struct Network {
    pub receiver: Option<Feature>,
    /// Après le récepteur, d'amont en aval, jusqu'au déversoir final.
    pub downstream: Vec<Feature>,
    pub direct: Vec<Feature>,
    pub deeper: Vec<Feature>,
}

pub fn network(basins: &Basins, main: Feature) -> Network {
    let all = (0..basins.rivers.len()).map(Feature::River).chain((0..basins.lakes.len()).map(Feature::Lake));
    let mut tributaries: HashMap<Feature, Vec<Feature>> = HashMap::new();
    for f in all {
        if let Some(r) = receiver(basins, f) {
            tributaries.entry(r).or_default().push(f);
        }
    }
    let mut seen: HashSet<Feature> = HashSet::from([main]);
    let direct: Vec<Feature> = tributaries.get(&main).into_iter().flatten().copied().filter(|f| seen.insert(*f)).collect();
    let mut deeper = Vec::new();
    let mut frontier = direct.clone();
    while let Some(f) = frontier.pop() {
        for &t in tributaries.get(&f).into_iter().flatten() {
            if seen.insert(t) {
                deeper.push(t);
                frontier.push(t);
            }
        }
    }
    let first = receiver(basins, main).filter(|r| seen.insert(*r));
    let mut downstream = Vec::new();
    let mut cur = first;
    while let Some(f) = cur.and_then(|f| receiver(basins, f)).filter(|r| seen.insert(*r)) {
        downstream.push(f);
        cur = Some(f);
    }
    Network { receiver: first, downstream, direct, deeper }
}

/// Élément en surbrillance : celui montré par l'interface, sinon selon l'option (survol de la
/// carte ou de l'étiquette ; une étiquette survolée compte aussi en mode « survol de la carte »,
/// puisqu'elle cache la carte en dessous).
pub fn update_hovered_feature(
    mode: Res<HighlightMode>,
    hovered: Res<HoveredHex>,
    label: Res<HoveredLabel>,
    pointed: Res<PointedFeature>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    mut feature: ResMut<HoveredFeature>,
) {
    let wanted = pointed.0.or(match *mode {
        HighlightMode::OnFeature => label.0.or_else(|| hovered.0.filter(|h| fog.explored.contains(h)).and_then(|h| feature_at(&map.0, h))),
        HighlightMode::OnLabel => label.0,
        HighlightMode::Off => None,
    });
    if feature.0 != wanted {
        feature.0 = wanted;
    }
}

/// Position (monde 2D) d'un point du tracé, et s'il est en terrain exploré.
fn course_point(map: &WorldMap, fog: &ShownFog, p: CoursePoint) -> (Vec2, bool) {
    let hex = map.hex_at(p.cell);
    let (cx, cy) = hex.to_world();
    let center = Vec2::new(cx, cy);
    if p.corner == CoursePoint::CENTER {
        return (center, fog.explored.contains(&hex));
    }
    if p.corner >= CoursePoint::EDGE {
        // Milieu d'une arête : connu si l'une des deux cases l'est
        let d = (p.corner - CoursePoint::EDGE) as usize;
        let ((ax, ay), (bx, by)) = hex.edge_corners(d);
        let known = fog.explored.contains(&hex) || fog.explored.contains(&map.canonical(hex + DIRECTIONS[d]));
        return ((Vec2::new(ax, ay) + Vec2::new(bx, by)) / 2.0, known);
    }
    let j = p.corner as usize;
    let at = center + Vec2::from_angle((-30.0 + 60.0 * j as f32).to_radians()) * HEX_SIZE;
    let known = [hex, hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]].iter().any(|&h| fog.explored.contains(&map.canonical(h)));
    (at, known)
}

/// Lignes à tracer pour un élément (suites de points explorés) : le cours d'un cours d'eau, ou
/// le pourtour d'un lac.
fn feature_lines(map: &WorldMap, fog: &ShownFog, feature: Feature) -> Vec<Vec<Vec2>> {
    match feature {
        Feature::River(r) => river_runs(map, fog, &map.basins().rivers[r]),
        Feature::Lake(l) => {
            let lake = &map.basins().lakes[l];
            let cells: HashSet<usize> = lake.cells.iter().copied().collect();
            let mut lines = Vec::new();
            for &c in &lake.cells {
                let hex = map.hex_at(c);
                if !fog.explored.contains(&hex) {
                    continue;
                }
                for (d, &step) in DIRECTIONS.iter().enumerate() {
                    let n = hex + step;
                    if map.contains(n) && cells.contains(&map.cell(n)) {
                        continue;
                    }
                    let ((ax, ay), (bx, by)) = hex.edge_corners(d);
                    // Un peu à l'intérieur du lac : le tracé reste sur l'eau
                    let (cx, cy) = hex.to_world();
                    let inset = |p: Vec2| p.lerp(Vec2::new(cx, cy), 0.06);
                    lines.push(vec![inset(Vec2::new(ax, ay)), inset(Vec2::new(bx, by))]);
                }
            }
            lines
        }
    }
}

/// Tracés connus d'un cours d'eau : son cours et ses petits bras, coupés là où ils sortent de la
/// zone explorée.
fn river_runs(map: &WorldMap, fog: &ShownFog, river: &NamedRiver) -> Vec<Vec<Vec2>> {
    let mut runs = Vec::new();
    let world = map.wrap().world_width();
    for course in std::iter::once(&river.course).chain(&river.arms) {
        let mut run: Vec<Vec2> = Vec::new();
        let mut last: Option<Vec2> = None;
        for &p in course {
            let (mut at, known) = course_point(map, fog, p);
            // Planète : chaque point pris du côté du raccord le plus proche du précédent (tracé
            // continu qui passe le raccord)
            if let Some(prev) = last
                && world > 0.0
            {
                at.x -= ((at.x - prev.x) / world).round() * world;
            }
            last = Some(at);
            if known {
                run.push(at);
            } else if !run.is_empty() {
                runs.push(std::mem::take(&mut run));
            }
        }
        if !run.is_empty() {
            runs.push(run);
        }
    }
    runs
}

/// Animation du courant : chevrons qui descendent le long du tracé.
const FLOW_SPACING: f32 = 26.0;
const FLOW_SPEED: f32 = 22.0;
const FLOW_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.9);

/// Chevrons du courant sur un tracé donné d'aval en amont : positions et directions (vers l'aval).
fn flow_marks(run: &[Vec2], time: f32) -> Vec<(Vec2, Vec2)> {
    // Parcours d'amont en aval
    let downhill: Vec<Vec2> = run.iter().rev().copied().collect();
    let total: f32 = downhill.windows(2).map(|w| w[0].distance(w[1])).sum();
    if total < 1.0 {
        return Vec::new();
    }
    let mut marks = Vec::new();
    let mut s = (time * FLOW_SPEED).rem_euclid(FLOW_SPACING);
    let mut walked = 0.0;
    for w in downhill.windows(2) {
        let len = w[0].distance(w[1]);
        while s <= walked + len && len > 0.0 {
            let dir = (w[1] - w[0]) / len;
            marks.push((w[0] + dir * (s - walked), dir));
            s += FLOW_SPACING;
        }
        walked += len;
    }
    marks
}

/// Dessine les tracés d'un élément (2D ou 3D) et, pour un cours d'eau, les chevrons du courant.
#[allow(clippy::too_many_arguments)]
fn draw_feature<G: GizmoConfigGroup>(
    runs: Vec<Vec<Vec2>>,
    river: bool,
    color: Color,
    chevron: Color,
    is_3d: bool,
    relief: Option<&Relief>,
    time: f32,
    gizmos_2d: &mut Gizmos<G>,
    gizmos_3d: &mut Gizmos<Gizmos3d>,
) {
    for run in runs {
        if run.len() == 1 {
            // Un seul point connu : un petit rond
            if is_3d {
                if let Some(relief) = relief {
                    let c = run[0];
                    let ring = (0..=12).map(|k| {
                        let p = c + Vec2::from_angle(k as f32 / 12.0 * std::f32::consts::TAU) * 4.0;
                        relief.on_ground(p.x, p.y, 1.5)
                    });
                    gizmos_3d.linestrip(ring, color);
                }
            } else {
                gizmos_2d.circle_2d(run[0], 4.0, color);
            }
            continue;
        }
        // Sens du courant (pas pour le pourtour d'un lac)
        let marks = if river { flow_marks(&run, time) } else { Vec::new() };
        if is_3d {
            let Some(relief) = relief else { continue };
            // Le tracé épouse le relief : points intermédiaires sur chaque segment
            let points = run
                .windows(2)
                .flat_map(|w| (0..4).map(move |k| w[0].lerp(w[1], k as f32 / 4.0)))
                .chain(run.last().copied())
                .map(|p| relief.on_ground(p.x, p.y, 1.5));
            gizmos_3d.linestrip(points.collect::<Vec<_>>(), color);
            for (p, dir) in marks {
                let side = dir.perp() * 2.6;
                let back = p - dir * 3.5;
                let at = |q: Vec2| relief.on_ground(q.x, q.y, 2.2);
                gizmos_3d.linestrip([at(back + side), at(p), at(back - side)], chevron);
            }
        } else {
            gizmos_2d.linestrip_2d(run, color);
            for (p, dir) in marks {
                let side = dir.perp() * 3.0;
                let back = p - dir * 4.0;
                gizmos_2d.linestrip_2d([back + side, p, back - side], chevron);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_highlight(
    hovered: Res<HoveredFeature>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    mode: Res<ViewMode>,
    relief: Option<Res<Relief>>,
    time: Res<Time>,
    mut gizmos_2d: Gizmos<RiverGizmos2d>,
    mut gizmos_3d: Gizmos<Gizmos3d>,
) {
    let Some(main) = hovered.0 else { return };
    let basins = map.0.basins();
    let net = network(basins, main);
    // Du plus lointain au plus proche : le cours survolé est dessiné en dernier, au-dessus
    let mut layers: Vec<(Feature, Color)> = Vec::new();
    layers.extend(net.deeper.iter().map(|&f| (f, DEEP_TRIBUTARY_COLOR)));
    layers.extend(net.direct.iter().map(|&f| (f, TRIBUTARY_COLOR)));
    layers.extend(net.downstream.iter().map(|&f| (f, DOWNSTREAM_COLOR)));
    layers.extend(net.receiver.map(|f| (f, RECEIVER_COLOR)));
    layers.push((main, HOVERED_COLOR));
    for (feature, color) in layers {
        let runs = feature_lines(&map.0, &fog, feature);
        let river = matches!(feature, Feature::River(_));
        draw_feature(runs, river, color, FLOW_COLOR, mode.is_3d, relief.as_deref(), time.elapsed_secs(), &mut gizmos_2d, &mut gizmos_3d);
    }
}

/// Carte du réseau hydrographique : tous les cours d'eau et lacs connus, colorés selon leur rang
/// par rapport au déversoir final, avec le sens du courant (chevrons d'une teinte plus sombre).
#[allow(clippy::too_many_arguments)]
fn draw_network_map(
    layer: Res<MapLayer>,
    hovered: Res<HoveredFeature>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    mode: Res<ViewMode>,
    relief: Option<Res<Relief>>,
    time: Res<Time>,
    mut fleuves_2d: Gizmos<NetworkGizmos2d>,
    mut streams_2d: Gizmos<StreamGizmos2d>,
    mut gizmos_3d: Gizmos<Gizmos3d>,
) {
    if *layer != MapLayer::Network {
        return;
    }
    let basins = map.0.basins();
    // Le réseau survolé est dessiné par la surbrillance, au-dessus
    let lit: HashSet<Feature> = hovered.0.map_or_else(HashSet::new, |main| {
        let net = network(basins, main);
        std::iter::once(main).chain(net.receiver).chain(net.downstream).chain(net.direct).chain(net.deeper).collect()
    });
    let t = time.elapsed_secs();
    let dark = |c: Color| {
        let s = c.to_srgba();
        Color::srgb(s.red * 0.45, s.green * 0.45, s.blue * 0.45)
    };
    // Les petits d'abord : les grands cours restent au-dessus
    for (l, lake) in basins.lakes.iter().enumerate().filter(|(l, _)| !lit.contains(&Feature::Lake(*l))) {
        let color = level_color(lake.level);
        draw_feature(feature_lines(&map.0, &fog, Feature::Lake(l)), false, color, color, mode.is_3d, relief.as_deref(), t, &mut streams_2d, &mut gizmos_3d);
    }
    for (r, river) in basins.rivers.iter().enumerate().rev() {
        if lit.contains(&Feature::River(r)) {
            continue;
        }
        let color = level_color(river.level);
        let runs = river_runs(&map.0, &fog, river);
        if runs.is_empty() {
            continue;
        }
        if river.fleuve {
            draw_feature(runs, true, color, dark(color), mode.is_3d, relief.as_deref(), t, &mut fleuves_2d, &mut gizmos_3d);
        } else {
            draw_feature(runs, true, color, dark(color), mode.is_3d, relief.as_deref(), t, &mut streams_2d, &mut gizmos_3d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affluents_de_proche_en_proche() {
        let m = WorldMap::generate_in(192, 111, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let b = m.basins();
        // Un fleuve qui a des affluents : chaque affluent direct s'y jette, chaque affluent plus
        // lointain se jette dans un élément du réseau
        let main = (0..b.rivers.len()).map(Feature::River).find(|&f| !network(b, f).direct.is_empty()).expect("un fleuve avec affluents");
        let net = network(b, main);
        assert!(net.direct.iter().all(|&f| receiver(b, f) == Some(main)));
        let members: HashSet<Feature> = std::iter::once(main).chain(net.direct.iter().copied()).chain(net.deeper.iter().copied()).collect();
        assert!(net.deeper.iter().all(|&f| receiver(b, f).is_some_and(|r| members.contains(&r))));
        assert!(!net.direct.contains(&main) && !net.deeper.contains(&main));
        // Vers l'aval : chaque élément se jette dans le précédent, le dernier n'a pas d'aval
        let tributary = (0..b.rivers.len()).map(Feature::River).find(|&f| !network(b, f).downstream.is_empty()).expect("un affluent d'affluent");
        let net = network(b, tributary);
        let mut prev = net.receiver.unwrap();
        for &f in &net.downstream {
            assert_eq!(receiver(b, prev), Some(f));
            prev = f;
        }
        assert!(receiver(b, prev).is_none_or(|r| r == tributary || net.downstream.contains(&r) || net.receiver == Some(r)));
        // Un lac nommé qui reçoit des cours d'eau les a pour affluents
        if let Some(l) = (0..b.lakes.len()).find(|&l| b.rivers.iter().any(|r| r.flows_into.is_none() && r.into_lake == Some(l))) {
            assert!(!network(b, Feature::Lake(l)).direct.is_empty());
        }
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;

    #[test]
    fn chevrons_vers_l_aval() {
        // Tracé d'aval en amont : (0,0) est l'aval, (100,0) l'amont
        let run = [Vec2::ZERO, Vec2::new(100.0, 0.0)];
        let marks = flow_marks(&run, 0.0);
        assert!(!marks.is_empty());
        // Le courant va de l'amont (x = 100) vers l'aval (x = 0)
        assert!(marks.iter().all(|(_, d)| d.x < 0.0));
        // Ils avancent avec le temps
        let later = flow_marks(&run, 0.5);
        assert!(later[0].0.x < marks[0].0.x);
    }
}
