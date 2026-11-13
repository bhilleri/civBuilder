//! Pose des villes et des villages sur leur case (logique pure) : stades, limite de la ville,
//! portes, emplacements des bâtiments et chemins. Rien ne se superpose, rien ne se pose sur l'eau,
//! et un stade ne fait qu'ajouter des bâtiments à ceux du précédent.
//!
//! Tout se calcule dans le repère de la case (origine au centre, unités du monde 2D). Le terrain
//! est lu à travers le trait `Site`. Voir `docs/conception/conception-structures.md`.

use crate::hex::HEX_SIZE;
use crate::map::Terrain;
use bevy::math::Vec2;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Nombre d'angles sur lesquels la limite d'une ville est calculée.
pub const LIMIT_SAMPLES: usize = 72;
/// Rayon de la muraille selon le stade (avant adaptation au terrain).
const WALL_RADIUS: [f32; 3] = [22.0, 25.0, 27.2];
/// Nombre de maisons visé à la fin de chaque stade.
const HOUSE_TARGET: [usize; 3] = [3, 7, 12];
const MARSH_HOUSE_TARGET: [usize; 3] = [3, 5, 7];
/// Limite de `in_hex` pour l'eau où aboutissent un pont ou le ponton qui l'accompagne (rivières de bord de case).
const BRIDGE_REACH: f32 = HEX_SIZE + 2.0;
const PIER_FROM_BRIDGE: f32 = 11.0;
const PIER_BRIDGE_ANGLES: [f32; 2] = [0.8, 0.4];
/// Marge intérieure entre un bâtiment et le tracé de la muraille.
const INSIDE_MARGIN: f32 = 1.1;
/// Rayon de la place centrale, laissée libre (garnison).
const PLACE_RADIUS: f32 = 5.5;
/// Écart entre deux emprises.
const GAP: f32 = 0.55;
/// Demi-largeur du tablier d'un pont : de quoi passer une charrette, et bien plus large que
/// l'échelle pour être vu de loin (choix de Benjamin, 09/11/2026), sans cacher la case.
pub const BRIDGE_HALF_WIDTH: f32 = 2.2;

/// Largeur de la rive et de la plage, sur laquelle on ne construit pas.
pub const BANK_WIDTH: f32 = 5.2;
/// Marge de construction au-delà de la rive.
const SHORE_MARGIN: f32 = 1.8;
/// Demi-profondeur du bâtiment d'une porte.
pub const GATE_DEPTH: f32 = 1.7;
/// Marais : galerie autour d'une maison, niveau des planchers, demi-côté du palier de la place, et
/// distance à l'eau vive (rivière, mer) sous laquelle on ne construit pas, même sur pilotis.
pub const GALLERY: f32 = 1.0;
pub const MARSH_DECK: f32 = 2.4;
pub const LANDING: f32 = 2.8;
const MARSH_MARGIN: f32 = 3.0;
/// Taille du grand toit par stade, en multiple d'une maison.
const HALL_SCALE: [f32; 3] = [1.45, 1.65, 1.85];

/// Terrain vu depuis une case. Les points sont relatifs au centre de la case.
pub trait Site {
    /// Hauteur du sol.
    fn height(&self, p: Vec2) -> f32;
    /// Distance à l'eau la plus proche (rivière, fleuve, mer, lac), négative dans l'eau.
    fn water_gap(&self, p: Vec2) -> f32;
    /// Cours d'eau proche d'un point, pour y jeter un pont (pas la mer ni un lac).
    fn river_near(&self, _p: Vec2) -> Option<RiverAxis> {
        None
    }
    /// Pont construit sur une rivière qui borde la case (voir `span_bridge`) : la ville y ouvre une
    /// porte, et le ponton s'en écarte.
    fn built_bridge(&self) -> Option<Bridge> {
        None
    }
}

/// Tronçon de cours d'eau : point de l'axe, sens du courant, demi-largeur de l'eau.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiverAxis {
    pub point: Vec2,
    pub dir: Vec2,
    pub half_width: f32,
}

/// Stade d'une ville selon sa population (0 : hameau, 1 : bourg, 2 : ville). Seuils provisoires.
pub fn stage_of_population(population: i32) -> usize {
    match population {
        i32::MIN..=2 => 0,
        3..=5 => 1,
        _ => 2,
    }
}

/// Famille d'un village d'après celle du lieu et la température de la saison la plus chaude (°C) :
/// sous `sites::HERDERS_BELOW`, rien n'y mûrit, ce sont des éleveurs (yourtes de la steppe).
pub fn village_family(family: Family, warmest: f32) -> Family {
    if warmest < crate::sites::HERDERS_BELOW && !family.herds() { Family::Steppe } else { family }
}

/// Famille de matériaux, d'après le lieu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Forest,
    Prairie,
    Boreal,
    Arid,
    Cold,
    Tropical,
    Mediterranean,
    Savanna,
    Steppe,
    Marsh,
}

impl Family {
    /// Toutes les familles.
    #[cfg(test)]
    pub const ALL: [Family; 10] = [Family::Forest, Family::Prairie, Family::Boreal, Family::Arid, Family::Cold, Family::Tropical, Family::Mediterranean, Family::Savanna, Family::Steppe, Family::Marsh];

    /// Famille de matériaux d'un terrain.
    pub fn of(terrain: Terrain) -> Family {
        match terrain {
            Terrain::Forest => Family::Forest,
            Terrain::Grassland | Terrain::Plains => Family::Prairie,
            Terrain::Taiga => Family::Boreal,
            Terrain::Desert | Terrain::Riverbank | Terrain::Oasis => Family::Arid,
            Terrain::Tundra | Terrain::Snow => Family::Cold,
            Terrain::Jungle | Terrain::WoodedSavanna => Family::Tropical,
            Terrain::Mediterranean => Family::Mediterranean,
            Terrain::Savanna => Family::Savanna,
            Terrain::Steppe => Family::Steppe,
            Terrain::Marsh => Family::Marsh,
            Terrain::Ocean | Terrain::Coast | Terrain::Lake => Family::Prairie,
        }
    }

    /// Habitat déjà fait pour des éleveurs (voir `village_family`).
    fn herds(self) -> bool {
        self == Family::Cold
    }

    /// Demi-dimensions d'une maison, débord du toit compris.
    pub fn half(self) -> Vec2 {
        match self {
            Family::Forest | Family::Prairie | Family::Boreal | Family::Tropical => Vec2::new(3.6, 2.7),
            Family::Arid => Vec2::new(3.1, 2.5),
            Family::Cold => Vec2::new(3.5, 3.5),
            Family::Mediterranean => Vec2::new(3.5, 2.8),
            Family::Savanna | Family::Steppe => Vec2::splat(3.4),
            Family::Marsh => Vec2::new(3.4, 2.3),
        }
    }

    /// Maisons sur pilotis.
    pub fn stilts(self) -> bool {
        matches!(self, Family::Tropical | Family::Marsh)
    }

    /// Dénivelé maximal sous une maison : un habitat léger cherche un sol doux, une maison accepte
    /// un peu plus, les pilotis davantage.
    fn max_drop(self) -> f32 {
        match self {
            Family::Cold | Family::Savanna | Family::Steppe => 2.7,
            Family::Tropical | Family::Marsh => 6.5,
            _ => 4.0,
        }
    }

    /// Muraille de bois (sinon de pierre ou de terre) : portail à linteau plutôt que voûte.
    pub fn wooden_wall(self) -> bool {
        matches!(self, Family::Forest | Family::Boreal | Family::Tropical | Family::Marsh | Family::Savanna | Family::Steppe)
    }
}

/// Nature d'un bâtiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    House,
    /// Grand toit, à la couleur du joueur.
    Hall,
    Granary,
    Market,
}

/// Bâtiment posé : `half` est sa demi-emprise, débord du toit compris (axe long selon `rot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    pub kind: BuildingKind,
    pub at: Vec2,
    pub rot: f32,
    pub half: Vec2,
    pub scale: f32,
}

impl Building {
    /// Coins de l'emprise.
    pub fn corners(&self) -> [Vec2; 4] {
        rect(self.at, self.half, self.rot)
    }
}

/// Ce que dessert une porte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateKind {
    Land,
    Pier,
    Bridge,
}

/// Porte de la muraille : au point `at` du tracé, normale `out` vers l'extérieur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub kind: GateKind,
    pub angle: f32,
    pub at: Vec2,
    pub out: Vec2,
    /// Porte principale d'une ville : deux tours.
    pub main: bool,
}

/// Ponton, du rivage (ou de la porte) vers l'eau. `start_height` : niveau du seuil d'une porte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pier {
    pub from: Vec2,
    pub to: Vec2,
    pub half_width: f32,
    pub start_height: Option<f32>,
    /// Barque amarrée : position et sens, là où l'eau est assez large pour elle.
    pub boat: Option<(Vec2, f32)>,
}

/// Pont en travers d'un cours d'eau : `near` du côté de la ville, `far` sur l'autre rive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bridge {
    pub near: Vec2,
    pub far: Vec2,
    pub half_width: f32,
    /// Une tour garde l'autre rive (ville fortifiée).
    pub head: bool,
}

/// Chemin de terre : suite de points, poids (0 à 1 : discrétion), estompé à son extrémité.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub points: Vec<Vec2>,
    pub weight: f32,
    pub fade: bool,
}

/// Pose complète d'une ville ou d'un village.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub family: Family,
    pub buildings: Vec<Building>,
    pub mast: Option<Vec2>,
    /// Foyer au centre (village).
    pub hearth: bool,
    /// Tracé de la muraille (si la ville est fortifiée), un rayon par angle.
    pub wall: Option<Vec<f32>>,
    pub gates: Vec<Gate>,
    pub pier: Option<Pier>,
    pub paths: Vec<Path>,
    /// Rayon moyen du sol de terre battue.
    pub ground_radius: f32,
    pub bridge: Option<Bridge>,
    /// Marais : niveau des planchers et des passerelles, par rapport au centre de la case.
    pub deck: Option<f32>,
    pub walks: Vec<Walk>,
    /// Escalier du palier de la place vers la terre.
    pub stairs: Option<(Vec2, Vec2)>,
    pub ponds: Vec<Pond>,
}

/// Passerelle de marais, au niveau des planchers ; `to_ground` : son bout descend à la terre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walk {
    pub from: Vec2,
    pub to: Vec2,
    pub to_ground: bool,
}

/// Mare peu profonde : centre, rayon, et rang qui fixe son contour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pond {
    pub at: Vec2,
    pub radius: f32,
    pub rank: usize,
}

impl Pond {
    /// Rayon de la mare dans une direction (contour irrégulier).
    pub fn radius_at(&self, a: f32) -> f32 {
        let i = self.rank as f32;
        self.radius * (1.0 + 0.26 * (3.0 * a + i * 2.1).sin() + 0.14 * (5.0 * a + i).sin() + 0.07 * (9.0 * a + i * 3.0).sin())
    }

    fn holds(&self, p: Vec2) -> bool {
        p.distance(self.at) < self.radius_at((p - self.at).to_angle())
    }
}

/// Mares d'une case de marais : réparties sur la case, et touchant l'eau vive si elle est proche.
fn marsh_ponds(site: &impl Site, seed: u64) -> Vec<Pond> {
    const BASE: [(f32, f32, f32); 8] = [(-13.0, 6.0, 6.0), (11.0, -11.0, 6.4), (13.0, 10.0, 5.4), (-6.0, -15.0, 5.0), (-21.0, -9.0, 3.6), (1.0, 17.0, 5.2), (-13.0, 20.0, 3.2), (22.0, -3.0, 3.4)];
    let turn = hash(9, seed) * TAU;
    let mut out: Vec<Pond> = Vec::new();
    for (x, y, r) in BASE {
        let at = Vec2::from_angle(turn).rotate(Vec2::new(x, y));
        if in_hex(at, HEX_SIZE - 1.0) && site.water_gap(at) > 0.0 {
            out.push(Pond { at, radius: r, rank: out.len() });
        }
    }
    for k in (0..12u64).step_by(3) {
        let d = Vec2::from_angle(turn + k as f32 / 12.0 * TAU);
        let mut r = 9.0;
        while r < 30.0 {
            if site.water_gap(d * r) <= 0.0 {
                let radius = 3.6 + 1.8 * hash(k, seed);
                let at = d * (r - radius * 0.5);
                if in_hex(at, HEX_SIZE - 1.0) && at.length() > 9.0 {
                    out.push(Pond { at, radius, rank: out.len() });
                }
                break;
            }
            r += 0.5;
        }
    }
    out
}

/// Une ville draine son sol : les mares rétrécissent au bourg, celles du cœur disparaissent à la ville.
fn drained(ponds: &[Pond], stage: usize) -> Vec<Pond> {
    match stage {
        0 => ponds.to_vec(),
        1 => ponds.iter().map(|p| Pond { radius: p.radius * 0.82, ..*p }).collect(),
        _ => ponds.iter().filter(|p| p.at.length() >= 14.0).map(|p| Pond { radius: p.radius * 0.66, ..*p }).collect(),
    }
}

/// Les emplacements au-dessus des mares d'abord : c'est là que servent les pilotis.
fn over_ponds_first(spots: Vec<Vec2>, ponds: &[Pond]) -> Vec<Vec2> {
    let (mut over, others): (Vec<Vec2>, Vec<Vec2>) = spots.into_iter().partition(|&p| ponds.iter().any(|q| q.holds(p)));
    over.extend(others);
    over
}

/// Galerie d'une maison sur pilotis.
pub fn gallery_of(b: &Building) -> [Vec2; 4] {
    rect(b.at, b.half + Vec2::splat(GALLERY), b.rot)
}

/// Réseau du marais : le palier de la place, une passerelle vers chaque bâtiment et chaque porte, sans
/// traverser de bâtiment, puis les tronçons au-delà des portes et l'escalier vers la terre.
fn marsh_network(buildings: &[Building], mast: Option<Vec2>, gates: &[Gate], pier: Option<Pier>, bridge: Option<Bridge>, fortified: bool) -> (Vec<Walk>, Option<(Vec2, Vec2)>) {
    let mut obstacles: Vec<Vec<Vec2>> = vec![rect(Vec2::ZERO, Vec2::splat(LANDING), 0.0).to_vec()];
    // L'escalier du palier descend vers la caméra, ou sur un autre côté si un bâtiment y est
    let mut stairs = None;
    for d in [-Vec2::Y, Vec2::X, -Vec2::X, Vec2::Y] {
        let (a, b) = (d * LANDING, d * (LANDING + 2.6));
        if !buildings.iter().any(|h| overlap(&rect((a + b) / 2.0, Vec2::new(1.3, 1.0), d.to_angle()), &gallery_of(h))) {
            stairs = Some((a, b));
            obstacles.push(rect((a + b) / 2.0, Vec2::new(1.3, 1.0), d.to_angle()).to_vec());
            break;
        }
    }
    let mut targets: Vec<(Vec2, usize)> = Vec::new();
    let mut order: Vec<&Building> = buildings.iter().collect();
    order.sort_by(|a, b| a.at.length().total_cmp(&b.at.length()));
    for b in order {
        let fp = match b.kind {
            BuildingKind::House | BuildingKind::Hall => gallery_of(b).to_vec(),
            _ => b.corners().to_vec(),
        };
        targets.push((b.at, obstacles.len()));
        obstacles.push(fp);
    }
    if let Some(m) = mast {
        obstacles.push(rect(m, Vec2::splat(0.8), 0.0).to_vec());
    }
    // Les sorties rejoignent la passerelle la plus proche, une fois les maisons reliées
    if let Some(p) = pier {
        let i = obstacles.len();
        obstacles.push(rect((p.from + p.to) / 2.0, Vec2::new(p.from.distance(p.to) / 2.0, 1.1), (p.to - p.from).to_angle()).to_vec());
        targets.push((p.from, i));
    }
    let mut walks = Vec::new();
    let mut low_ends: Vec<Vec2> = pier.iter().map(|p| p.from).collect();
    if !fortified && let Some(b) = bridge {
        targets.push((b.near, usize::MAX));
        low_ends.push(b.near);
    }
    if fortified {
        for gate in gates {
            targets.push((gate.at - gate.out * (GATE_DEPTH + 0.2), usize::MAX));
            let from = gate.at + gate.out * (GATE_DEPTH + 0.04);
            match (gate.kind, bridge) {
                (GateKind::Land, _) => walks.push(Walk { from, to: from + gate.out * 3.6, to_ground: true }),
                (GateKind::Bridge, Some(b)) => walks.push(Walk { from, to: b.near, to_ground: true }),
                _ => {}
            }
        }
    }
    let (links, _) = route((Vec2::ZERO, Some(0)), &targets, &obstacles, true);
    // (un pas fixe et fin : une passerelle arrive au ras de la galerie, quelle que soit sa longueur)
    let trim = |from: Vec2, to: Vec2| {
        let step = 0.05 / from.distance(to).max(0.05);
        let mut f = 0.0;
        while f < 1.0 && obstacles.iter().any(|o| contains(o, from.lerp(to, f))) {
            f += step;
        }
        from.lerp(to, f.min(1.0))
    };
    for &(a, b) in &links {
        let (a2, b2) = (trim(a, b), trim(b, a));
        // (le plancher descend vers le début du ponton ou du pont, au niveau du sol)
        walks.push(if low_ends.contains(&a) { Walk { from: b2, to: a, to_ground: true } } else { Walk { from: a2, to: b2, to_ground: false } });
    }
    (walks, stairs)
}

/// Rayon du tracé à un angle donné.
pub fn limit_at(limit: &[f32], angle: f32) -> f32 {
    let f = (angle / TAU).rem_euclid(1.0) * limit.len() as f32;
    let i = f.floor() as usize % limit.len();
    let j = (i + 1) % limit.len();
    limit[i] + (limit[j] - limit[i]) * (f - f.floor())
}

fn hash(i: u64, seed: u64) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ seed.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    (x % 1_000_000) as f32 / 1_000_000.0
}

/// Coins d'un rectangle (axe long selon `rot`).
pub fn rect(c: Vec2, half: Vec2, rot: f32) -> [Vec2; 4] {
    let (s, co) = rot.sin_cos();
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(a, b): (f32, f32)| {
        let (u, v) = (a * half.x, b * half.y);
        c + Vec2::new(u * co - v * s, u * s + v * co)
    })
}

fn in_hex(p: Vec2, r: f32) -> bool {
    p.x.abs() * 0.866 + p.y.abs() * 0.5 <= r * 0.866 && p.y.abs() <= r
}

/// Deux polygones convexes se touchent-ils ? (test de séparation)
pub fn overlap(a: &[Vec2], b: &[Vec2]) -> bool {
    for poly in [a, b] {
        for i in 0..poly.len() {
            let e = poly[(i + 1) % poly.len()] - poly[i];
            let n = Vec2::new(e.y, -e.x);
            let range = |q: &[Vec2]| q.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.dot(n)), hi.max(p.dot(n))));
            let (a0, a1) = range(a);
            let (b0, b1) = range(b);
            if a1 <= b0 || b1 <= a0 {
                return false;
            }
        }
    }
    true
}

/// Un point est-il dans un polygone convexe ?
fn contains(poly: &[Vec2], p: Vec2) -> bool {
    let mut sign = 0.0f32;
    for i in 0..poly.len() {
        let v = (poly[(i + 1) % poly.len()] - poly[i]).perp_dot(p - poly[i]);
        if v != 0.0 {
            if sign != 0.0 && v.signum() != sign {
                return false;
            }
            sign = v.signum();
        }
    }
    true
}

fn crosses(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let o = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
    o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
}

/// Limite de la ville par stade, angle par angle : elle s'arrête avant la rive et le bord de la
/// case et, sur un relief, cherche le tracé qui monte et descend le moins.
fn limits(site: &impl Site, marsh: bool) -> [Vec<f32>; 3] {
    let n = LIMIT_SAMPLES;
    let dir = |i: usize| Vec2::from_angle(i as f32 / n as f32 * TAU);
    let cut: Vec<f32> = (0..n)
        .map(|i| {
            let mut r = 6.0;
            while r < 31.0 {
                let p = dir(i) * r;
                // (en marais, seule l'eau vive arrête la ville : les mares se franchissent)
                if marsh && site.water_gap(p) < 1.6 {
                    return r - 2.8;
                }
                if (!marsh && site.water_gap(p) < BANK_WIDTH + SHORE_MARGIN - 0.7) || !in_hex(p, HEX_SIZE - 3.4) {
                    return r - 0.7;
                }
                r += 0.5;
            }
            99.0
        })
        .collect();
    let mut out: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut prev: Option<Vec<f32>> = None;
    for stage in 0..3 {
        let base = WALL_RADIUS[stage];
        let options = [base, base - 1.5, base - 3.0, base - 4.5];
        let ground = |i: usize, r: f32| site.height(dir(i) * r);
        let mut cost = [0.0f32; 4];
        let mut from: Vec<[usize; 4]> = Vec::with_capacity(n);
        for i in 1..n {
            let mut next = [0.0f32; 4];
            let mut back = [0usize; 4];
            for (k, &r) in options.iter().enumerate() {
                let (mut best, mut at) = (f32::MAX, 0);
                for (j, &q) in options.iter().enumerate() {
                    let v = cost[j] + (ground(i, r) - ground(i - 1, q)).abs() + 0.25 * (r - q).abs();
                    if v < best {
                        (best, at) = (v, j);
                    }
                }
                next[k] = best + 0.12 * (base - r);
                back[k] = at;
            }
            cost = next;
            from.push(back);
        }
        let mut k = (0..4).min_by(|&a, &b| cost[a].total_cmp(&cost[b])).unwrap_or(0);
        let mut path = vec![0.0; n];
        path[n - 1] = options[k];
        for i in (0..n - 1).rev() {
            k = from[i][k];
            path[i] = options[k];
        }
        for _ in 0..3 {
            path = (0..n).map(|i| (path[(i + n - 1) % n] + 2.0 * path[i] + path[(i + 1) % n]) / 4.0).collect();
        }
        let lim: Vec<f32> = (0..n).map(|i| cut[i].min(prev.as_ref().map_or(path[i], |p| path[i].max(p[i])))).collect();
        prev = Some(lim.clone());
        out[stage] = lim;
    }
    out
}

fn inside(fp: &[Vec2], limit: &[f32], margin: f32) -> bool {
    fp.iter().all(|c| c.length() <= limit_at(limit, c.to_angle()) - margin)
}

/// Sens et normale du tracé à un angle : la normale va vers l'extérieur.
fn wall_frame(limit: &[f32], angle: f32) -> (Vec2, Vec2) {
    let n = limit.len();
    let i = ((angle / TAU).rem_euclid(1.0) * n as f32).floor() as usize % n;
    let pt = |j: usize| Vec2::from_angle(j as f32 / n as f32 * TAU) * limit[j];
    let tangent = (pt((i + 1) % n) - pt(i)).normalize_or_zero();
    let at = Vec2::from_angle(angle) * limit_at(limit, angle);
    let mut out = Vec2::new(-tangent.y, tangent.x);
    if out.dot(at) < 0.0 {
        out = -out;
    }
    (at, out)
}

/// Registre des emprises réservées.
struct Ground<'a, S: Site> {
    site: &'a S,
    taken: Vec<Vec<Vec2>>,
    /// Marais : pilotis possibles près de l'eau, maisons dans l'axe, avec leur galerie.
    marsh: bool,
}

impl<S: Site> Ground<'_, S> {
    fn free(&self, fp: &[Vec2]) -> bool {
        !self.taken.iter().any(|o| overlap(fp, o))
    }

    fn no_build(&self, p: Vec2) -> bool {
        self.site.water_gap(p) < if self.marsh { MARSH_MARGIN } else { BANK_WIDTH + SHORE_MARGIN }
    }

    fn drop_under(&self, fp: &[Vec2], c: Vec2) -> f32 {
        let (lo, hi) = fp.iter().chain([&c]).map(|&p| self.site.height(p)).fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
        hi - lo
    }

    /// Orientation selon la pente : dans le sens des courbes de niveau, sinon face au centre.
    fn orient(&self, p: Vec2) -> f32 {
        let g = Vec2::new(self.site.height(p + Vec2::X) - self.site.height(p - Vec2::X), self.site.height(p + Vec2::Y) - self.site.height(p - Vec2::Y));
        if g.length() > 0.22 { g.x.atan2(-g.y) } else { p.to_angle() + FRAC_PI_2 }
    }

    /// Réserve la première place possible d'une liste (position, rotation éventuelle).
    fn reserve(&mut self, list: &[(Vec2, Option<f32>)], half: Vec2, max_drop: f32, ok: impl Fn(Vec2, f32) -> bool) -> Option<(Vec2, f32)> {
        for &(at, rot) in list {
            let rot = rot.unwrap_or_else(|| self.orient(at));
            let fp = rect(at, half + Vec2::splat(GAP), rot);
            if !fp.iter().all(|&c| in_hex(c, HEX_SIZE - 1.0)) || fp.iter().any(|&c| self.no_build(c)) || self.no_build(at) {
                continue;
            }
            if !ok(at, rot) || self.drop_under(&fp, at) > max_drop || !self.free(&fp) {
                continue;
            }
            self.taken.push(fp.to_vec());
            return Some((at, rot));
        }
        None
    }

    /// Ajoute jusqu'à `count` maisons. Les emplacements viennent d'une spirale fixe pour la case :
    /// un stade complète le précédent sans rien déplacer.
    fn fill(&mut self, spots: &[Vec2], used: &mut [bool], family: Family, count: usize, scale_of: impl Fn(Vec2) -> f32, ok: impl Fn(&[Vec2]) -> bool) -> Vec<Building> {
        let half = family.half();
        let mut out = Vec::new();
        for (i, &at) in spots.iter().enumerate() {
            if out.len() >= count {
                break;
            }
            if used[i] {
                continue;
            }
            // (en marais, maisons et passerelles suivent les mêmes axes : le réseau reste lisible)
            let rot = if self.marsh { if at.x.abs() > at.y.abs() { FRAC_PI_2 } else { 0.0 } } else { self.orient(at) };
            let scale = scale_of(at);
            let h = half * scale;
            let around = if self.marsh { GALLERY } else { 0.0 };
            let fp = rect(at, h + Vec2::splat(around + GAP), rot);
            if !fp.iter().all(|&c| in_hex(c, HEX_SIZE - 1.0)) || !ok(&fp) || fp.iter().any(|&c| self.no_build(c)) || self.no_build(at) {
                continue;
            }
            // (sur pilotis, un passage pour les passerelles entre deux galeries)
            let room = rect(at, h + Vec2::splat(around + if self.marsh { 0.7 } else { GAP }), rot);
            if self.drop_under(&fp, at) > family.max_drop() || !self.free(&room) {
                continue;
            }
            self.taken.push(fp.to_vec());
            used[i] = true;
            out.push(Building { kind: BuildingKind::House, at, rot, half: h, scale });
        }
        out
    }
}

/// Emplacements candidats : une spirale, du centre vers le bord.
fn spiral(seed: u64) -> Vec<Vec2> {
    let offset = hash(1, seed) * TAU;
    (0..340u64)
        .filter_map(|i| {
            let r = (i as f32 / 340.0).sqrt() * 27.0 + (hash(i, seed) - 0.5) * 1.4;
            (r >= 8.0).then(|| Vec2::from_angle(i as f32 * 2.39996 + offset) * r)
        })
        .collect()
}

/// Emplacements du marais : une trame à peine décalée, puis la spirale. Des galeries rectangulaires
/// s'y rangent mieux qu'en spirale, et leurs passerelles restent à angle droit.
fn marsh_spots(seed: u64, ponds: &[Pond]) -> Vec<Vec2> {
    // (rangées serrées de part et d'autre des axes, écartées de la place)
    const ROWS: [f32; 6] = [-20.0, -13.5, -6.0, 6.0, 13.5, 20.0];
    let mut grid: Vec<Vec2> = Vec::new();
    for (ix, &x) in ROWS.iter().enumerate() {
        for (iy, &y) in ROWS.iter().enumerate() {
            let i = (ix * 6 + iy) as u64;
            let jitter = Vec2::new(hash(i + 400, seed) - 0.5, hash(i + 440, seed) - 0.5) * 0.6;
            grid.push(Vec2::new(x, y) + jitter);
        }
    }
    grid.sort_by(|a, b| a.length().total_cmp(&b.length()));
    // (du centre vers le bord : aller d'abord aux mares éparpillerait les maisons et perdrait la place)
    grid.extend(over_ponds_first(spiral(seed), ponds));
    grid
}

/// Ponton vers l'eau la plus proche : du haut de la rive jusqu'un peu dans l'eau.
fn find_pier(site: &impl Site, bridge: Option<&Bridge>) -> Option<Pier> {
    let mut shores: Vec<(Vec2, f32)> = (0..72)
        .filter_map(|i| {
            let d = Vec2::from_angle(i as f32 / 72.0 * TAU);
            let mut r = 6.0;
            while r < 31.0 {
                if site.water_gap(d * r) <= 0.0 {
                    return Some((d, r));
                }
                r += 0.5;
            }
            None
        })
        .collect();
    shores.sort_by(|a, b| a.1.total_cmp(&b.1));
    // (écarté du pont, le ponton peut aller chercher l'eau jusqu'à la limite permise au pont)
    let reach = if bridge.is_some() { BRIDGE_REACH } else { HEX_SIZE - 1.0 };
    // Pont et ponton partent de deux endroits bien distincts de la ville : franchement écartés si la
    // rivière le permet, de justesse si elle ne frôle la ville que sur un petit arc
    for min_angle in PIER_BRIDGE_ANGLES {
        for &(dir, r) in &shores {
            if !in_hex(dir * r, reach) {
                // (sans pont, seul le rivage le plus proche compte)
                if bridge.is_some() {
                    continue;
                }
                return None;
            }
            let mut back = r;
            while back > 6.0 && site.water_gap(dir * back) < BANK_WIDTH {
                back -= 0.5;
            }
            let from = dir * back;
            if bridge.is_some_and(|b| {
                let span = (b.near + b.far) / 2.0;
                from.distance(span) < PIER_FROM_BRIDGE || dir.angle_to(span).abs() < min_angle
            }) {
                continue;
            }
            let mut pier = Pier { from, to: pier_end(site, dir * r, dir), half_width: 1.0, start_height: None, boat: None };
            pier.boat = boat_spot(site, &pier);
            return Some(pier);
        }
    }
    None
}

/// Place d'une barque près du ponton : d'abord le long de lui, sinon dans le sens du courant.
fn boat_spot(site: &impl Site, p: &Pier) -> Option<(Vec2, f32)> {
    let t = (p.to - p.from).normalize_or_zero();
    let s = t.perp();
    let afloat = |c: Vec2, dir: Vec2| site.water_gap(c) < -0.2 && site.water_gap(c + dir * 3.3) < -0.05 && site.water_gap(c - dir * 3.3) < -0.05;
    for (dir, side) in [(t, p.half_width + 1.9), (s, p.half_width + 4.3)] {
        // (le ponton n'avance que de peu sur l'eau : la barque peut mouiller au-delà de son bout)
        for along in [-2.0f32, -1.0, 0.0, 1.0, 2.0, 3.5, 5.0, 6.5] {
            for k in [1.0f32, -1.0] {
                let c = p.to + t * along + s * k * side;
                if afloat(c, dir) {
                    return Some((c, dir.to_angle()));
                }
            }
        }
    }
    None
}

/// Pont sur le cours d'eau proche, là où il est le plus près de la ville : `near` de son côté.
/// Choisi avant le ponton : une rivière qui ne frôle la ville que sur un arc n'y offre qu'une place.
/// Pont jeté sur le cours d'eau proche de `toward` : de la terre ferme d'une rive à celle de l'autre
/// (3,2 de la rive), perpendiculaire au courant, ou selon `across` s'il est donné (pont d'un fleuve,
/// d'une rive choisie à l'opposée). Rive cherchée jusqu'à `max_bank` de l'axe du cours d'eau ;
/// `near` est le bout le plus proche du centre du site. `None` : pas de cours d'eau, ou pas de
/// terre ferme à portée.
pub fn span_bridge(site: &impl Site, toward: Vec2, across: Option<Vec2>, max_bank: f32) -> Option<Bridge> {
    let axis = site.river_near(toward)?;
    let across = across.map_or(axis.dir.perp(), |a| a.normalize_or_zero());
    let bank = |side: f32| {
        let mut k = 0.0;
        while k < max_bank && site.water_gap(axis.point + across * side * k) < 3.2 {
            k += 0.25;
        }
        (k < max_bank).then_some(axis.point + across * side * k)
    };
    let (a, b) = (bank(1.0)?, bank(-1.0)?);
    let (near, far) = if a.length() < b.length() { (a, b) } else { (b, a) };
    Some(Bridge { near, far, half_width: BRIDGE_HALF_WIDTH, head: false })
}

/// Bout du ponton : une avancée dans l'eau, plus courte sur une rivière (l'eau y est étroite).
fn pier_end(site: &impl Site, water: Vec2, dir: Vec2) -> Vec2 {
    let mut len = 0.0;
    let mut best = site.water_gap(water);
    while len < 4.2 {
        let next = site.water_gap(water + dir * (len + 0.25));
        if next > best + 0.02 && len >= 1.0 {
            break;
        }
        best = best.min(next);
        len += 0.25;
    }
    water + dir * len
}

/// Réseau de chemins : relie des points à une racine, en ligne droite ou avec un coude, sans
/// traverser d'obstacle ni croiser un autre chemin. Rend les tronçons et les points non reliés.
fn route(root: (Vec2, Option<usize>), targets: &[(Vec2, usize)], obstacles: &[Vec<Vec2>], square: bool) -> (Vec<(Vec2, Vec2)>, Vec<Vec2>) {
    let mut links: Vec<(Vec2, Vec2)> = Vec::new();
    let mut joined: Vec<(Vec2, Option<usize>)> = vec![root];
    let mut missing = Vec::new();
    let clear = |a: Vec2, b: Vec2, skip: &[Option<usize>], links: &[(Vec2, Vec2)], width: f32| {
        let len = a.distance(b) / 2.0;
        let strip = rect((a + b) / 2.0, Vec2::new(len, width), (b - a).to_angle());
        !obstacles.iter().enumerate().any(|(i, o)| !skip.contains(&Some(i)) && overlap(&strip, o)) && !links.iter().any(|&(c, d)| crosses(a, b, c, d))
    };
    for &(p, own) in targets {
        let mut order = joined.clone();
        order.sort_by(|a, b| a.0.distance(p).total_cmp(&b.0.distance(p)));
        let mut done = false;
        let before = links.len();
        let mine = Some(own).filter(|&o| o != usize::MAX);
        // (si un passage est trop étroit, un second essai plus fin, avec plus de détours)
        const NARROW: [f32; 10] = [1.5, -1.5, 3.0, -3.0, 5.0, -5.0, 8.0, -8.0, 11.0, -11.0];
        let attempts: [(f32, &[f32]); 3] = [(0.75, &[5.0, -5.0, 8.0, -8.0]), (0.45, &NARROW), (0.45, &NARROW)];
        let free_point = |c: Vec2| in_hex(c, HEX_SIZE - 2.0) && !obstacles.iter().any(|o| contains(o, c));
        'targets: for (attempt, (width, detours)) in attempts.into_iter().enumerate() {
            for &(q, q_own) in &order {
                let skip = [mine, q_own];
                let axial = (p.x - q.x).abs().min((p.y - q.y).abs()) <= 0.3;
                if square {
                    // Marais : tout droit dans l'axe, ou en équerre le long des maisons…
                    if axial && clear(p, q, &skip, &links, width) {
                        links.push((p, q));
                        done = true;
                        break 'targets;
                    }
                    for c in [Vec2::new(q.x, p.y), Vec2::new(p.x, q.y)] {
                        if !axial && free_point(c) && clear(p, c, &[mine], &links, width) && clear(c, q, &[q_own], &links, width) {
                            links.extend([(p, c), (c, q)]);
                            done = true;
                            break 'targets;
                        }
                    }
                    // … ou en U pour contourner ce qui gêne, toujours à angle droit
                    for &k in detours {
                        let turns = [(Vec2::new(p.x, q.y + k), Vec2::new(q.x, q.y + k)), (Vec2::new(p.x, p.y + k), Vec2::new(q.x, p.y + k)), (Vec2::new(q.x + k, p.y), Vec2::new(q.x + k, q.y)), (Vec2::new(p.x + k, p.y), Vec2::new(p.x + k, q.y))];
                        for (c, e) in turns {
                            if free_point(c) && free_point(e) && clear(p, c, &[mine], &links, width) && clear(c, e, &[], &links, width) && clear(e, q, &[q_own], &links, width) {
                                links.extend([(p, c), (c, e), (e, q)]);
                                done = true;
                                break 'targets;
                            }
                        }
                    }
                }
                // (en marais, le biais reste le tout dernier recours)
                if square && attempt < 2 {
                    continue;
                }
                if clear(p, q, &skip, &links, width) {
                    links.push((p, q));
                    done = true;
                    break 'targets;
                }
                let m = (p + q) / 2.0;
                let side = (q - p).perp().normalize_or_zero();
                for &off in detours {
                    let c = m + side * off;
                    if free_point(c) && clear(p, c, &skip, &links, width) && clear(c, q, &skip, &links, width) {
                        links.extend([(p, c), (c, q)]);
                        done = true;
                        break 'targets;
                    }
                }
            }
        }
        if done {
            joined.push((p, Some(own).filter(|&o| o != usize::MAX)));
            // (on peut se raccorder à une avenue ou à une passerelle n'importe où)
            for &(a, b) in &links[before..] {
                let d = a.distance(b);
                let mut t = if square { 2.5 } else { 4.0 };
                while t < d - 1.0 {
                    let q = a.lerp(b, t / d);
                    if !obstacles.iter().any(|o| contains(o, q)) {
                        joined.push((q, None));
                    }
                    t += if square { 2.5 } else { 3.5 };
                }
            }
        } else {
            missing.push(p);
        }
    }
    (links, missing)
}

/// Pose d'une ville : `stage` de 0 à 2. Les bâtiments ne dépendent pas de `fortified` : construire
/// la muraille ne déplace rien.
pub fn city_layout(site: &impl Site, family: Family, stage: usize, fortified: bool, seed: u64) -> Layout {
    let stage = stage.min(2);
    let marsh = family == Family::Marsh;
    let inside_margin = if marsh { 1.7 } else { INSIDE_MARGIN };
    let lims = limits(site, marsh);
    // (en marais, la place est le palier : moins large qu'une place de terre)
    let mut g = Ground { site, taken: vec![circle(Vec2::ZERO, if marsh { LANDING + 1.0 } else { PLACE_RADIUS })], marsh };
    // (seulement un pont construit : pas de décor qui contredit les règles)
    let bridge_plan = site.built_bridge();
    if let Some(b) = bridge_plan {
        g.taken.push(rect(b.near, Vec2::splat(2.0), 0.0).to_vec());
    }
    let pier_plan = find_pier(site, bridge_plan.as_ref());
    if let Some(p) = pier_plan {
        g.taken.push(rect((p.from + p.to) / 2.0, Vec2::new(p.from.distance(p.to) / 2.0 + 0.3, 1.5), (p.to - p.from).to_angle()).to_vec());
    }
    // (en marais, l'escalier du palier a sa place, du côté de la caméra)
    if marsh {
        g.taken.push(rect(-Vec2::Y * (LANDING + 1.4), Vec2::new(1.4, 1.6), 0.0).to_vec());
    }
    // Grand toit : au bord de la place, côté amont sur une pente, à la plus grande taille possible
    let half = family.half();
    let slope = Vec2::new(site.height(Vec2::X * 8.0) - site.height(-Vec2::X * 8.0), site.height(Vec2::Y * 8.0) - site.height(-Vec2::Y * 8.0)) / 16.0;
    let angles: Vec<f32> = if marsh {
        vec![90.0, 180.0, 0.0, 270.0]
    } else if slope.length() > 0.1 {
        let up = slope.to_angle().to_degrees();
        vec![up, up + 45.0, up - 45.0, up + 90.0, up - 90.0, up + 180.0]
    } else {
        // (derrière la place, vue depuis la caméra : il ne la masque pas et sa porte reste visible)
        vec![90.0, 135.0, 45.0, 180.0, 0.0, 225.0, 315.0, 270.0]
    };
    let mut hall = None;
    // (dernier recours sur un terrain difficile : petit, et plus de limite de pente, car une ville a toujours son grand toit)
    'hall: for (m, drop) in [(1.85f32, family.max_drop() + 1.5), (1.4, family.max_drop() + 1.5), (1.05, family.max_drop() + 1.5), (1.05, 99.0)] {
        let h = half * m + Vec2::splat(if marsh { GALLERY + 1.1 } else { 0.0 });
        let r = PLACE_RADIUS + h.y + GAP + 0.4;
        for extra in [0.0, 22.5] {
            let list: Vec<(Vec2, Option<f32>)> = angles.iter().map(|a| { let q = (a + extra).to_radians(); (Vec2::from_angle(q) * r, Some(q + FRAC_PI_2)) }).collect();
            let fits = |at: Vec2, rot: f32| (0..3).all(|st| inside(&rect(at, half * m.min(HALL_SCALE[st]), rot), &lims[st], inside_margin - 0.1));
            if let Some(found) = g.reserve(&list, h, drop, fits) {
                hall = Some((found.0, found.1, m));
                break 'hall;
            }
        }
    }
    // Axes des portes : gardés libres dès le hameau, aucune maison ne vient les boucher
    let strip = |a: f32, max: f32| {
        let r = limit_at(&lims[2], a).min(max);
        let m = (r + 6.4) / 2.0;
        rect(Vec2::from_angle(a) * m, Vec2::new((r - 6.4).max(0.1) / 2.0, 0.85), a).to_vec()
    };
    let mut axes: Vec<(f32, GateKind)> = Vec::new();
    if let Some(p) = pier_plan {
        let a = p.from.to_angle();
        let fp = strip(a, p.from.length() - 0.6);
        if g.free(&fp) {
            g.taken.push(fp);
        }
        axes.push((a, GateKind::Pier));
    }
    if let Some(b) = bridge_plan {
        let a = b.near.to_angle();
        let fp = strip(a, b.near.length() - 0.6);
        if g.free(&fp) {
            g.taken.push(fp);
        }
        axes.push((a, GateKind::Bridge));
    }
    let mut land = 0;
    // (en marais, les portes restent dans l'axe des passerelles ; en biais seulement s'il manque des sorties)
    let directions: &[f32] = if marsh { &[0.0, 90.0, 180.0, -90.0, 45.0, 135.0, -45.0, -135.0, 30.0, 60.0, 120.0, 150.0, -30.0, -60.0, -120.0, -150.0] } else { &[45.0, 135.0, -45.0, 180.0, -90.0, -135.0, 90.0, 0.0, -15.0, 75.0, -75.0, 165.0, -165.0, 105.0, 15.0, -105.0] };
    for &d in directions {
        if land >= 4 || (marsh && land >= 2 && d % 90.0 != 0.0) {
            break;
        }
        let a = d.to_radians();
        let r = limit_at(&lims[2], a);
        let close = axes.iter().any(|&(b, _)| Vec2::from_angle(a).angle_to(Vec2::from_angle(b)).abs() < if marsh { 0.72 } else { 0.95 });
        if close || r < 12.0 || g.no_build(Vec2::from_angle(a) * (r + 2.6)) {
            continue;
        }
        let fp = strip(a, 99.0);
        if g.free(&fp) {
            g.taken.push(fp);
            axes.push((a, GateKind::Land));
            land += 1;
        }
    }
    let within = |st: usize, h: Vec2| { let lim = lims[st].clone(); move |at: Vec2, rot: f32| inside(&rect(at, h, rot), &lim, inside_margin - 0.1) };
    let v = |x: f32, y: f32| (Vec2::new(x, y), None);
    let mast = g.reserve(&[v(4.0, -11.0), v(-13.0, 3.0), v(10.0, -4.0), v(-3.0, 12.0), v(8.0, -7.0), v(-9.0, -7.0), v(-8.0, 8.0), v(7.0, 8.0)], Vec2::splat(0.8), 9.0, within(0, Vec2::splat(0.8)));
    let granary = g.reserve(&[v(12.0, -8.0), v(-13.5, 8.0), v(13.0, 6.0), v(-3.0, 15.0), v(2.0, -16.0)], Vec2::splat(2.6), family.max_drop(), within(1, Vec2::splat(2.6)));
    let market = g.reserve(&[v(-12.0, 8.0), v(-5.0, 16.0), v(13.0, -12.0), v(14.0, 8.0)], Vec2::new(4.0, 3.2), family.max_drop(), within(2, Vec2::new(4.0, 3.2)));
    // Maisons, stade après stade
    let ponds0 = if marsh { marsh_ponds(site, seed) } else { Vec::new() };
    let spots = if marsh { marsh_spots(seed, &ponds0) } else { spiral(seed) };
    let mut used = vec![false; spots.len()];
    let mut buildings: Vec<Building> = Vec::new();
    let mut houses = 0;
    // (une galerie prend de la place : le marais compte moins de maisons, chaque stade en ajoute)
    let target = if marsh { MARSH_HOUSE_TARGET } else { HOUSE_TARGET };
    for st in 0..=stage {
        let lim = lims[st].clone();
        let fit = move |fp: &[Vec2]| inside(fp, &lim, inside_margin);
        let found = g.fill(&spots, &mut used, family, target[st] - houses, |p| if marsh { 0.72 } else { 0.92 } + 0.2 * hash((p.x * 97.0 + p.y * 13.0).to_bits() as u64, seed), &fit);
        houses += found.len();
        buildings.extend(found);
        if houses < target[st] {
            let found = g.fill(&spots, &mut used, family, target[st] - houses, |_| if marsh { 0.6 } else { 0.78 }, &fit);
            houses += found.len();
            buildings.extend(found);
        }
    }
    let hall_scale = hall.map(|(_, _, m)| m.min(HALL_SCALE[stage]));
    if let (Some((at, rot, _)), Some(scale)) = (hall, hall_scale) {
        buildings.push(Building { kind: BuildingKind::Hall, at, rot, half: half * scale, scale });
    }
    if stage >= 1 && let Some((at, rot)) = granary {
        buildings.push(Building { kind: BuildingKind::Granary, at, rot, half: Vec2::splat(2.5), scale: 1.0 });
    }
    if stage >= 2 && let Some((at, rot)) = market {
        buildings.push(Building { kind: BuildingKind::Market, at, rot, half: Vec2::new(3.9, 3.1), scale: 1.0 });
    }
    // Portes : deux, trois puis quatre vers la terre avec une muraille ; deux directions sans
    let land_count = if fortified { [2, 3, 4][stage] } else { 2 };
    let mut seen = 0;
    let used_axes: Vec<(f32, GateKind)> = axes
        .iter()
        .copied()
        .filter(|&(_, k)| match k {
            GateKind::Pier => stage >= 1,
            GateKind::Bridge => true,
            GateKind::Land => {
                seen += 1;
                seen <= land_count
            }
        })
        .collect();
    let limit = &lims[stage];
    let mut gates: Vec<Gate> = Vec::new();
    let mut pier = pier_plan.filter(|_| stage >= 1);
    let bridge = bridge_plan.map(|b| Bridge { head: fortified, ..b });
    for &(a, kind) in &used_axes {
        // (la porte du pont est là où son axe coupe la muraille)
        let a = match (kind, bridge) {
            (GateKind::Bridge, Some(b)) if fortified => {
                let dir = (b.near - b.far).normalize_or_zero();
                let mut k = 0.0;
                while k < 40.0 && (b.far + dir * k).length() > limit_at(limit, (b.far + dir * k).to_angle()) {
                    k += 0.2;
                }
                (b.far + dir * k).to_angle()
            }
            _ => a,
        };
        let (at, out) = wall_frame(limit, a);
        gates.push(Gate { kind, angle: a, at, out, main: false });
        if fortified && kind == GateKind::Pier && let Some(p) = pier.as_mut() {
            // Le ponton part de la face extérieure de la porte, perpendiculaire au mur
            let start = at + out * (GATE_DEPTH + 0.04);
            let mut d = 0.0;
            while d < 18.0 && site.water_gap(start + out * d) > 0.0 {
                d += 0.25;
            }
            *p = Pier { from: start, to: pier_end(site, start + out * d, out), half_width: 1.25, start_height: Some(if marsh { MARSH_DECK } else { site.height(at) - site.height(Vec2::ZERO) + 0.04 }), boat: None };
            p.boat = boat_spot(site, p);
        }
    }
    if stage == 2
        && fortified
        && let Some(main) = gates.iter_mut().find(|g| g.kind == GateKind::Land)
    {
        main.main = true;
    }
    // Marais : une passerelle vers chaque maison, seul accès à la terre
    if marsh {
        let (walks, stairs) = marsh_network(&buildings, mast.map(|m| m.0), &gates, pier.filter(|_| !fortified), bridge, fortified);
        return Layout {
            family,
            buildings,
            mast: mast.map(|m| m.0),
            hearth: false,
            wall: fortified.then(|| limit.clone()),
            gates: if fortified { gates } else { Vec::new() },
            pier,
            paths: Vec::new(),
            ground_radius: [12.0, 18.0, 23.0][stage],
            bridge,
            deck: Some(MARSH_DECK),
            walks,
            stairs,
            ponds: drained(&ponds0, stage),
        };
    }
    // Chemins : un axe passe devant le grand toit et relie les sorties, des branches rejoignent
    // les grandes zones. Pas de chemin vers chaque maison.
    let mut obstacles: Vec<Vec<Vec2>> = buildings.iter().map(|b| b.corners().to_vec()).collect();
    if let Some(m) = mast {
        obstacles.push(rect(m.0, Vec2::splat(0.8), m.1).to_vec());
    }
    let front = hall.map_or(Vec2::ZERO, |(at, _, _)| at.normalize_or_zero() * 3.0);
    let mut targets: Vec<(Vec2, usize)> = vec![(Vec2::ZERO, usize::MAX)];
    let mut ends: Vec<(Vec2, Vec2, bool)> = Vec::new();
    for gate in &gates {
        let inner = gate.at - gate.out * (GATE_DEPTH + 0.2);
        targets.push((inner, usize::MAX));
        let outer = match (gate.kind, &pier, bridge) {
            (GateKind::Pier, Some(p), _) => p.from,
            (GateKind::Bridge, _, Some(b)) => b.near,
            _ => {
                let mut r = 3.6;
                while r > 0.5 && (!in_hex(gate.at + gate.out * r, HEX_SIZE - 0.8) || g.no_build(gate.at + gate.out * r)) {
                    r -= 0.5;
                }
                gate.at + gate.out * r
            }
        };
        ends.push((inner, outer, gate.kind == GateKind::Land));
    }
    for (i, b) in buildings.iter().enumerate() {
        if matches!(b.kind, BuildingKind::Granary | BuildingKind::Market) {
            targets.push((b.at, i));
        }
    }
    let (links, _) = route((front, None), &targets, &obstacles, false);
    let mut paths: Vec<Path> = links.iter().map(|&(a, b)| Path { points: vec![a, b], weight: 1.0, fade: false }).collect();
    for (inner, outer, fade) in ends {
        paths.push(Path { points: vec![inner, outer], weight: 1.0, fade });
    }
    // (le chemin continue sur l'autre rive et s'y perd)
    if let Some(b) = bridge {
        paths.push(Path { points: vec![b.far, b.far + (b.far - b.near).normalize_or_zero() * 6.0], weight: 0.8, fade: true });
    }
    obstacles.clear();
    Layout {
        family,
        buildings,
        mast: mast.map(|m| m.0),
        hearth: false,
        wall: fortified.then(|| limit.clone()),
        gates: if fortified { gates } else { Vec::new() },
        pier,
        paths,
        ground_radius: [12.0, 18.0, 23.0][stage],
        bridge,
        deck: None,
        walks: Vec::new(),
        stairs: None,
        ponds: Vec::new(),
    }
}

/// Pose d'un village : des habitations autour d'un foyer, une sente qui le traverse, un ponton.
pub fn village_layout(site: &impl Site, family: Family, seed: u64) -> Layout {
    let marsh = family == Family::Marsh;
    let mut g = Ground { site, taken: vec![circle(Vec2::ZERO, 4.5)], marsh };
    let pier = find_pier(site, None);
    let mut paths = Vec::new();
    if let Some(p) = pier {
        g.taken.push(rect((p.from + p.to) / 2.0, Vec2::new(p.from.distance(p.to) / 2.0 + 0.3, 1.5), (p.to - p.from).to_angle()).to_vec());
        let a = p.from.to_angle();
        let d = p.from.length();
        g.taken.push(rect(Vec2::from_angle(a) * ((d + 5.2) / 2.0), Vec2::new((d - 5.2).max(0.1) / 2.0, 0.85), a).to_vec());
        if !marsh {
            paths.push(Path { points: vec![Vec2::ZERO, p.from], weight: 0.72, fade: false });
        }
    }
    for a0 in [0.5f32, 1.2, -0.4, 2.1, -1.2, 2.8] {
        if marsh {
            break;
        }
        let ends = [Vec2::from_angle(a0) * 23.0, Vec2::from_angle(a0 + PI) * 23.0];
        let strips = ends.map(|e| rect(e * 0.6, Vec2::new(9.0, 0.85), e.to_angle()).to_vec());
        if ends.iter().any(|&e| g.no_build(e) || g.no_build(e * 0.5)) || strips.iter().any(|s| !g.free(s)) {
            continue;
        }
        g.taken.extend(strips);
        for e in ends {
            paths.push(Path { points: vec![Vec2::ZERO, e], weight: 0.72, fade: true });
        }
        break;
    }
    let ponds = if marsh { marsh_ponds(site, seed) } else { Vec::new() };
    let spots = over_ponds_first(spiral(seed.wrapping_add(3)), &ponds);
    let mut used = vec![false; spots.len()];
    let fit = |fp: &[Vec2]| fp.iter().all(|c| c.length() <= 22.0);
    let mut buildings = g.fill(&spots, &mut used, family, 6, |p| if marsh { 0.72 } else { 0.92 } + 0.2 * hash((p.x * 53.0 + p.y * 7.0).to_bits() as u64, seed), fit);
    if buildings.len() < 6 {
        let more = g.fill(&spots, &mut used, family, 6 - buildings.len(), |_| if marsh { 0.64 } else { 0.78 }, fit);
        buildings.extend(more);
    }
    let (walks, stairs) = if marsh { marsh_network(&buildings, None, &[], pier, None, false) } else { (Vec::new(), None) };
    Layout { family, buildings, mast: None, hearth: true, wall: None, gates: Vec::new(), pier, paths, ground_radius: 9.5, bridge: None, deck: marsh.then_some(MARSH_DECK), walks, stairs, ponds }
}

fn circle(c: Vec2, r: f32) -> Vec<Vec2> {
    (0..8).map(|i| c + Vec2::from_angle(i as f32 / 8.0 * TAU) * r).collect()
}

/// Type d'un camp hostile, d'après sa case. Il décide de son allure et du début de son nom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CampKind {
    /// Tentes basses et pieux en arc, partout ailleurs.
    Encampment,
    /// Palissade pleine et tour de guet, sur une case plate et dégagée.
    Fort,
    /// Adossé à des rochers, sur un relief.
    Lair,
}

impl CampKind {
    /// Type d'un camp selon le terrain et le relief de sa case.
    pub fn of(terrain: Terrain, relief: crate::map::Relief) -> CampKind {
        if relief != crate::map::Relief::Flat {
            return CampKind::Lair;
        }
        match terrain {
            Terrain::Grassland | Terrain::Plains | Terrain::Steppe | Terrain::Savanna | Terrain::Desert | Terrain::Riverbank | Terrain::Oasis | Terrain::Tundra | Terrain::Mediterranean | Terrain::Snow => CampKind::Fort,
            _ => CampKind::Encampment,
        }
    }

    /// Premier mot possible du nom du camp : on lit dans le nom ce qu'on voit sur la carte.
    pub fn name_words(self) -> &'static [&'static str] {
        match self {
            CampKind::Encampment => &["Camp"],
            CampKind::Fort => &["Fort"],
            CampKind::Lair => &["Repaire", "Antre"],
        }
    }
}

/// Tente : une seule, celle du chef, porte le rouge hostile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tent {
    pub at: Vec2,
    pub chief: bool,
}

/// Pieu : pied, sommet (incliné vers l'extérieur dans un arc de défense), hauteur, rayon du pied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stake {
    pub at: Vec2,
    pub tip: Vec2,
    pub height: f32,
    pub radius: f32,
}

/// Rocher d'un repaire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rock {
    pub at: Vec2,
    pub radius: f32,
    pub height: f32,
}

/// Pose d'un camp hostile.
#[derive(Clone, Debug, PartialEq)]
pub struct CampLayout {
    pub kind: CampKind,
    pub family: Family,
    pub tents: Vec<Tent>,
    pub stakes: Vec<Stake>,
    pub rocks: Vec<Rock>,
    /// Entrée sombre d'un repaire, au pied des rochers.
    pub cave: Option<Vec2>,
    pub banner: Option<Vec2>,
    pub tower: Option<Vec2>,
    pub trophy: Option<Vec2>,
    pub ground_radius: f32,
}

/// Pose d'un camp : foyer au centre, tentes sans superposition, défenses selon le type.
pub fn camp_layout(site: &impl Site, family: Family, kind: CampKind, seed: u64) -> CampLayout {
    let mut g = Ground { site, taken: vec![circle(Vec2::ZERO, 3.5)], marsh: false };
    let slope = Vec2::new(site.height(Vec2::X * 8.0) - site.height(-Vec2::X * 8.0), site.height(Vec2::Y * 8.0) - site.height(-Vec2::Y * 8.0));
    // Le côté du relief, ou un côté de la case choisi par elle sur le plat
    let back = if slope.length() > 0.5 { slope.to_angle() } else { hash(5, seed) * TAU };
    let mut rocks = Vec::new();
    let mut cave = None;
    if kind == CampKind::Lair {
        for (i, (off, dist, radius, height)) in [(0.0f32, 15.0f32, 7.5f32, 16.0f32), (0.8, 19.0, 6.5, 13.0), (-0.8, 19.0, 6.5, 13.0), (1.5, 21.0, 4.5, 9.0), (-1.5, 21.0, 4.5, 9.0)].into_iter().enumerate() {
            let at = Vec2::from_angle(back + off) * dist;
            if !in_hex(at, HEX_SIZE - radius * 0.6) || g.no_build(at) {
                continue;
            }
            rocks.push(Rock { at, radius, height: height + 2.0 * hash(i as u64, seed) });
            g.taken.push(circle(at, radius + 0.6));
        }
        if !rocks.is_empty() {
            let at = Vec2::from_angle(back) * 8.2;
            cave = Some(at);
            g.taken.push(rect(at, Vec2::splat(2.0), back).to_vec());
        }
    }
    let toward = |a: f32, r: f32| Vec2::from_angle(a) * r;
    let spot = |a: f32, r: f32| (toward(a, r), Some(0.0));
    let tower = if kind == CampKind::Fort {
        let list: Vec<_> = [back, back + 1.2, back - 1.2, back + 2.4].iter().map(|&a| spot(a, 12.0)).collect();
        g.reserve(&list, Vec2::splat(3.3), 5.0, |_, _| true).map(|f| f.0)
    } else {
        None
    };
    let banner_at: Vec<_> = [back + 0.9, back - 0.9, back + 2.0, back - 2.0, back + PI].iter().map(|&a| spot(a, 7.5)).collect();
    let banner = g.reserve(&banner_at, Vec2::splat(0.9), 9.0, |_, _| true).map(|f| f.0);
    let trophy = if kind == CampKind::Fort {
        None
    } else {
        let list: Vec<_> = [back + PI, back + PI - 0.7, back + PI + 0.7, back + 1.6].iter().map(|&a| spot(a, 16.5)).collect();
        g.reserve(&list, Vec2::splat(0.9), 9.0, |_, _| true).map(|f| f.0)
    };
    // Tentes
    let radius_max = if kind == CampKind::Fort { 19.5 } else { 17.5 };
    let wanted = if kind == CampKind::Encampment { 4 } else { 3 };
    let half = Vec2::new(3.4, 3.4);
    let mut tents = Vec::new();
    for at in spiral(seed.wrapping_add(7)) {
        if tents.len() >= wanted {
            break;
        }
        let fp = rect(at, half + Vec2::splat(GAP), 0.0);
        if at.length() + half.x > radius_max || !g.free(&fp) || fp.iter().any(|&c| g.no_build(c)) || g.no_build(at) || g.drop_under(&fp, at) > 2.7 {
            continue;
        }
        g.taken.push(fp.to_vec());
        tents.push(Tent { at, chief: tents.is_empty() });
    }
    // Pieux : un arc incliné vers l'extérieur, ou une palissade pleine percée d'une entrée
    let mut stakes = Vec::new();
    if kind == CampKind::Fort {
        // (entrée vers le sud, du côté de la caméra)
        let entrance = -FRAC_PI_2 + 0.3;
        // (pieux jointifs : une palissade pleine, qui se lit de loin)
        const PALISADE: usize = 76;
        for i in 0..PALISADE {
            let a = i as f32 / PALISADE as f32 * TAU + 0.1;
            let at = toward(a, 23.5);
            if Vec2::from_angle(a).angle_to(Vec2::from_angle(entrance)).abs() < 0.24 || !in_hex(at, HEX_SIZE - 1.0) || g.no_build(at) {
                continue;
            }
            stakes.push(Stake { at, tip: at, height: 8.5 + 1.6 * hash(i as u64 + 120, seed), radius: 1.05 });
        }
    } else {
        // (l'arc protège le côté ouvert du camp, à l'opposé du relief)
        let span = if kind == CampKind::Lair { 2.6 } else { 4.2 };
        for i in 0..18u64 {
            if i % 6 == 2 {
                continue;
            }
            let a = back + PI - span / 2.0 + span * i as f32 / 17.0 + 0.05 * hash(i, seed);
            let at = toward(a, 20.2 + 1.6 * hash(i + 40, seed));
            let fp = rect(at, Vec2::splat(0.8), 0.0);
            if !in_hex(at, HEX_SIZE - 1.0) || g.no_build(at) || !g.free(&fp) {
                continue;
            }
            g.taken.push(fp.to_vec());
            stakes.push(Stake { at, tip: at + at.normalize_or_zero() * 2.6, height: 7.5 + 1.5 * hash(i + 80, seed), radius: 1.0 });
        }
    }
    CampLayout { kind, family, tents, stakes, rocks, cave, banner, tower, trophy, ground_radius: if kind == CampKind::Fort { 22.0 } else { 20.0 } }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Terrain d'essai : plat, en pente, accidenté, ou avec de l'eau d'un côté.
    #[derive(Clone, Copy)]
    enum Test {
        Flat,
        Slope,
        Rugged,
        River,
        Coast,
    }

    impl Site for Test {
        fn river_near(&self, p: Vec2) -> Option<RiverAxis> {
            matches!(self, Test::River).then_some(RiverAxis { point: Vec2::new(p.x, 20.0), dir: Vec2::X, half_width: 3.0 })
        }
        fn height(&self, p: Vec2) -> f32 {
            match self {
                Test::Flat | Test::River | Test::Coast => 0.0,
                Test::Slope => 0.2 * (p.x - p.y),
                Test::Rugged => 6.0 * (p.x * 0.15 + 0.7).sin() * (p.y * 0.13 - 0.4).cos(),
            }
        }
        fn water_gap(&self, p: Vec2) -> f32 {
            match self {
                Test::River => (p.distance(Vec2::new(p.x, 20.0)) - 3.0).min((p.y + 40.0).abs()),
                Test::Coast => 22.0 - (p.x + p.y) * 0.707,
                _ => 1e6,
            }
        }
    }

    const SITES: [Test; 5] = [Test::Flat, Test::Slope, Test::Rugged, Test::River, Test::Coast];

    fn all_cities(mut check: impl FnMut(&Layout, Test, Family, usize, bool)) {
        for site in SITES {
            for family in Family::ALL {
                for stage in 0..3 {
                    for fort in [false, true] {
                        check(&city_layout(&site, family, stage, fort, 7), site, family, stage, fort);
                    }
                }
            }
        }
    }

    #[test]
    fn stades_selon_la_population() {
        assert_eq!([1, 2, 3, 5, 6, 40].map(stage_of_population), [0, 0, 1, 1, 2, 2]);
    }

    #[test]
    fn aucun_batiment_ne_se_superpose() {
        all_cities(|l, _, _, _, _| {
            for (i, a) in l.buildings.iter().enumerate() {
                for b in &l.buildings[i + 1..] {
                    assert!(!overlap(&a.corners(), &b.corners()), "{a:?} / {b:?}");
                }
            }
        });
    }

    #[test]
    fn aucun_batiment_pres_de_l_eau() {
        all_cities(|l, site, family, _, _| {
            // (en marais, les pilotis approchent davantage : seule l'eau vive les arrête)
            let margin = if family == Family::Marsh { 1.6 } else { BANK_WIDTH };
            for b in &l.buildings {
                for c in b.corners() {
                    assert!(site.water_gap(c) >= margin, "{c} à {}", site.water_gap(c));
                }
            }
        });
    }

    #[test]
    fn en_marais_les_galeries_ne_se_touchent_pas() {
        all_cities(|l, _, family, _, _| {
            if family != Family::Marsh {
                return;
            }
            let homes: Vec<&Building> = l.buildings.iter().filter(|b| matches!(b.kind, BuildingKind::House | BuildingKind::Hall)).collect();
            for (i, a) in homes.iter().enumerate() {
                for b in &homes[i + 1..] {
                    assert!(!overlap(&gallery_of(a), &gallery_of(b)));
                }
            }
        });
    }

    #[test]
    fn en_marais_chaque_batiment_rejoint_la_terre() {
        all_cities(|l, _, family, _, fort| {
            if family != Family::Marsh {
                return;
            }
            assert_eq!(l.deck, Some(MARSH_DECK));
            assert!(l.paths.is_empty());
            // Réseau parti du palier : chaque galerie est atteinte, sans en traverser une autre
            let close = |p: Vec2, a: Vec2, b: Vec2| { let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0); p.distance(a + (b - a) * t) < 0.1 };
            let on_landing = |p: Vec2| p.x.abs() <= LANDING + 0.3 && p.y.abs() <= LANDING + 0.3;
            // (une galerie relie les passerelles qui l'atteignent : on y fait le tour de la maison)
            let footprint = |b: &Building| rect(b.at, b.half + Vec2::splat(GALLERY + 0.2), b.rot);
            let mut walks: Vec<Walk> = l.walks.iter().copied().filter(|w| on_landing(w.from) || on_landing(w.to)).collect();
            let mut homes: Vec<Vec2> = Vec::new();
            let mut grew = true;
            while grew {
                grew = false;
                for b in &l.buildings {
                    if !homes.contains(&b.at) && walks.iter().any(|w| contains(&footprint(b), w.from) || contains(&footprint(b), w.to)) {
                        homes.push(b.at);
                        grew = true;
                    }
                }
                for w in &l.walks {
                    if walks.contains(w) {
                        continue;
                    }
                    let by_walk = walks.iter().any(|r| close(w.from, r.from, r.to) || close(w.to, r.from, r.to) || close(r.from, w.from, w.to) || close(r.to, w.from, w.to));
                    let by_home = l.buildings.iter().any(|b| homes.contains(&b.at) && (contains(&footprint(b), w.from) || contains(&footprint(b), w.to)));
                    if by_walk || by_home {
                        walks.push(*w);
                        grew = true;
                    }
                }
            }
            for b in l.buildings.iter().filter(|b| matches!(b.kind, BuildingKind::House | BuildingKind::Hall)) {
                assert!(homes.contains(&b.at), "{b:?} isolé ({} passerelles, muraille {fort})", l.walks.len());
            }
            // Une passerelle ne traverse aucune galerie
            for w in &l.walks {
                if w.from.distance(w.to) < 0.5 {
                    continue;
                }
                let strip = rect((w.from + w.to) / 2.0, Vec2::new(w.from.distance(w.to) / 2.0 - 0.3, 0.3), (w.to - w.from).to_angle());
                for b in l.buildings.iter().filter(|b| b.kind == BuildingKind::House) {
                    assert!(!overlap(&strip, &gallery_of(b)), "{w:?} traverse {b:?}");
                }
            }
        });
    }

    #[test]
    fn une_ville_draine_ses_mares() {
        let surface = |stage: usize| city_layout(&Test::Flat, Family::Marsh, stage, false, 7).ponds.iter().map(|p| p.radius * p.radius).sum::<f32>();
        assert!(surface(0) > surface(1) && surface(1) > surface(2));
        // (les maisons, elles, ne bougent pas)
        let houses = |stage: usize| city_layout(&Test::Flat, Family::Marsh, stage, false, 7).buildings.iter().filter(|b| b.kind == BuildingKind::House).map(|b| b.at).collect::<Vec<_>>();
        assert!(houses(0).iter().all(|p| houses(2).contains(p)));
        assert!(city_layout(&Test::Flat, Family::Prairie, 0, false, 7).ponds.is_empty());
    }

    #[test]
    fn le_seuil_du_ponton_ne_depend_pas_de_l_altitude() {
        struct High;
        impl Site for High {
            fn height(&self, _: Vec2) -> f32 {
                500.0
            }
            fn water_gap(&self, p: Vec2) -> f32 {
                22.0 - (p.x + p.y) * 0.707
            }
        }
        let l = city_layout(&High, Family::Prairie, 1, true, 7);
        assert!(l.pier.and_then(|p| p.start_height).is_some_and(|h| h.abs() < 1.0), "relatif au centre de la case");
    }

    #[test]
    fn un_stade_ajoute_sans_deplacer() {
        for site in SITES {
            for family in Family::ALL {
                let houses = |stage: usize| -> Vec<Vec2> { city_layout(&site, family, stage, false, 7).buildings.iter().filter(|b| b.kind == BuildingKind::House).map(|b| b.at).collect() };
                for stage in 1..3 {
                    let (before, after) = (houses(stage - 1), houses(stage));
                    assert!(before.iter().all(|p| after.contains(p)), "stade {stage}");
                }
            }
        }
    }

    #[test]
    fn la_muraille_ne_deplace_rien() {
        for site in SITES {
            for stage in 0..3 {
                let without = city_layout(&site, Family::Prairie, stage, false, 7);
                let with = city_layout(&site, Family::Prairie, stage, true, 7);
                assert_eq!(without.buildings, with.buildings);
            }
        }
    }

    #[test]
    fn les_batiments_restent_dans_les_murs() {
        all_cities(|l, _, _, _, fort| {
            if !fort {
                return;
            }
            let wall = l.wall.as_ref().unwrap();
            for b in &l.buildings {
                assert!(inside(&b.corners(), wall, 0.4), "{b:?}");
            }
        });
    }

    #[test]
    fn le_denivele_respecte_le_type_de_batiment() {
        all_cities(|l, site, family, _, _| {
            let g = Ground { site: &site, taken: Vec::new(), marsh: false };
            for b in l.buildings.iter().filter(|b| b.kind == BuildingKind::House) {
                let fp = rect(b.at, b.half + Vec2::splat(GAP), b.rot);
                assert!(g.drop_under(&fp, b.at) <= family.max_drop() + 1e-3);
            }
        });
    }

    #[test]
    fn un_grand_toit_et_au_moins_deux_portes() {
        all_cities(|l, _, _, _, fort| {
            assert_eq!(l.buildings.iter().filter(|b| b.kind == BuildingKind::Hall).count(), 1);
            if fort {
                assert!(l.gates.len() >= 2, "{} portes", l.gates.len());
            }
        });
    }

    #[test]
    fn pas_de_muraille_sans_fortification() {
        all_cities(|l, _, _, _, fort| {
            assert_eq!(l.wall.is_some(), fort);
            assert_eq!(!l.gates.is_empty(), fort);
        });
    }

    #[test]
    fn un_chemin_ne_traverse_aucun_batiment() {
        all_cities(|l, _, _, _, _| {
            for p in &l.paths {
                let (a, b) = (p.points[0], p.points[1]);
                let strip = rect((a + b) / 2.0, Vec2::new(a.distance(b) / 2.0, 0.5), (b - a).to_angle());
                for house in l.buildings.iter().filter(|h| h.kind == BuildingKind::House) {
                    assert!(!overlap(&strip, &house.corners()), "{a} {b} / {house:?}");
                }
            }
        });
    }

    #[test]
    fn le_ponton_part_de_la_porte() {
        for site in [Test::River, Test::Coast] {
            let l = city_layout(&site, Family::Prairie, 1, true, 7);
            let gate = l.gates.iter().find(|g| g.kind == GateKind::Pier).expect("porte d'eau");
            let pier = l.pier.expect("ponton");
            assert!(pier.from.distance(gate.at + gate.out * (GATE_DEPTH + 0.04)) < 1e-3);
            assert!(site.water_gap(pier.to) < 0.0, "le ponton atteint l'eau");
        }
    }

    /// Site avec un pont construit sur la rivière proche de `toward`.
    struct Bridged<S: Site>(S, Vec2);

    impl<S: Site> Site for Bridged<S> {
        fn height(&self, p: Vec2) -> f32 {
            self.0.height(p)
        }
        fn water_gap(&self, p: Vec2) -> f32 {
            self.0.water_gap(p)
        }
        fn river_near(&self, p: Vec2) -> Option<RiverAxis> {
            self.0.river_near(p)
        }
        fn built_bridge(&self) -> Option<Bridge> {
            span_bridge(&self.0, self.1, None, 14.0)
        }
    }

    #[test]
    fn seul_le_pont_construit_existe() {
        let river = Bridged(Test::River, Vec2::new(0.0, 20.0));
        for family in Family::ALL {
            for stage in 0..3 {
                assert!(city_layout(&Test::River, family, stage, true, 7).bridge.is_none(), "pas de pont décoratif");
                let city = city_layout(&river, family, stage, true, 7);
                let b = city.bridge.expect("le pont construit");
                assert!(b.head, "tour de tête de pont");
                assert!(Test::River.water_gap(b.near) >= 3.0 && Test::River.water_gap(b.far) >= 3.0, "les deux bouts sur la terre");
                assert!(Test::River.water_gap((b.near + b.far) / 2.0) < 0.0, "il franchit l'eau");
                assert!(city.gates.iter().any(|g| g.kind == GateKind::Bridge));
                if let Some(pier) = city.pier {
                    assert!(pier.to.angle_to(b.near).abs() > 0.5, "pont et ponton sont écartés : {pier:?} {b:?}");
                }
                assert!(!city_layout(&river, family, stage, false, 7).bridge.unwrap().head);
            }
        }
        assert!(span_bridge(&Test::Coast, Vec2::new(15.0, 15.0), None, 14.0).is_none(), "pas de pont sur la mer");
        assert!(span_bridge(&Test::Flat, Vec2::ZERO, None, 14.0).is_none());
    }

    /// Rivière qui ne frôle la ville que d'un côté, sur un petit arc.
    struct Frole;

    impl Site for Frole {
        fn height(&self, _p: Vec2) -> f32 {
            0.0
        }
        fn water_gap(&self, p: Vec2) -> f32 {
            (p.y - 29.5).abs() - 3.3
        }
        fn river_near(&self, p: Vec2) -> Option<RiverAxis> {
            Some(RiverAxis { point: Vec2::new(p.x, 29.5), dir: Vec2::X, half_width: 3.3 })
        }
    }

    #[test]
    fn une_riviere_qui_frole_la_ville_garde_pont_et_ponton() {
        for family in Family::ALL {
            let city = city_layout(&Bridged(Frole, Vec2::new(0.0, 29.5)), family, 2, true, 7);
            let b = city.bridge.expect("un pont");
            let pier = city.pier.expect("un ponton");
            assert!(b.near.x.abs() < 3.0, "le pont prend la meilleure place : {b:?}");
            let span = (b.near + b.far) / 2.0;
            assert!(pier.to.angle_to(span).abs() > 0.3, "{pier:?} {b:?}");
            assert!(pier.from.distance(span) >= PIER_FROM_BRIDGE - 2.0, "{pier:?} {b:?}");
            let gate = |kind| city.gates.iter().find(|g| g.kind == kind).unwrap().at;
            assert!(gate(GateKind::Pier).distance(gate(GateKind::Bridge)) >= 6.0, "deux portes distinctes");
        }
    }

    #[test]
    fn le_pont_ne_touche_aucun_batiment() {
        for family in Family::ALL {
            for stage in 0..3 {
                let plan = span_bridge(&Test::River, Vec2::new(0.0, 20.0), None, 14.0).unwrap();
                let l = city_layout(&Bridged(Test::River, Vec2::new(0.0, 20.0)), family, stage, true, 7);
                for b in &l.buildings {
                    assert!(!overlap(&b.corners(), &rect(plan.near, Vec2::splat(1.2), 0.0)), "{b:?}");
                }
            }
        }
    }

    #[test]
    fn la_barque_est_sur_l_eau() {
        all_cities(|l, site, _, _, _| {
            if let Some((at, _)) = l.pier.and_then(|p| p.boat) {
                assert!(site.water_gap(at) < 0.0);
            }
        });
        // (sur une rivière assez large ou une côte, il y a bien une barque)
        assert!(city_layout(&Test::Coast, Family::Marsh, 1, true, 7).pier.unwrap().boat.is_some());
        assert!(city_layout(&Test::Coast, Family::Prairie, 1, true, 7).pier.unwrap().boat.is_some());
    }

    #[test]
    fn en_marais_il_y_a_un_escalier_et_des_passerelles_a_angle_droit() {
        all_cities(|l, _, family, _, _| {
            if family != Family::Marsh {
                return;
            }
            assert!(l.stairs.is_some(), "un escalier vers la terre");
            for w in l.walks.iter().filter(|w| !w.to_ground) {
                let d = w.to - w.from;
                assert!(d.x.abs().min(d.y.abs()) <= 0.31, "passerelle en biais : {w:?}");
            }
        });
    }

    #[test]
    fn en_marais_le_stade_ville_ajoute_des_maisons() {
        let houses = |stage: usize| city_layout(&Test::Flat, Family::Marsh, stage, false, 7).buildings.iter().filter(|b| b.kind == BuildingKind::House).count();
        assert!(houses(0) < houses(1) && houses(1) < houses(2), "{} {} {}", houses(0), houses(1), houses(2));
    }

    #[test]
    fn la_pose_est_deterministe() {
        for site in SITES {
            assert_eq!(city_layout(&site, Family::Forest, 2, true, 11), city_layout(&site, Family::Forest, 2, true, 11));
            assert_eq!(village_layout(&site, Family::Steppe, 11), village_layout(&site, Family::Steppe, 11));
        }
    }

    fn all_camps(mut check: impl FnMut(&CampLayout, Test)) {
        for site in SITES {
            for family in Family::ALL {
                for kind in [CampKind::Encampment, CampKind::Fort, CampKind::Lair] {
                    for seed in [1, 7, 42] {
                        check(&camp_layout(&site, family, kind, seed), site);
                    }
                }
            }
        }
    }

    #[test]
    fn le_type_du_camp_vient_de_sa_case() {
        use crate::map::Relief;
        assert_eq!(CampKind::of(Terrain::Plains, Relief::Flat), CampKind::Fort);
        assert_eq!(CampKind::of(Terrain::Forest, Relief::Flat), CampKind::Encampment);
        assert_eq!(CampKind::of(Terrain::Forest, Relief::Rugged), CampKind::Lair);
        assert_eq!(CampKind::of(Terrain::Desert, Relief::Hills), CampKind::Lair);
        assert!(CampKind::Fort.name_words().contains(&"Fort"));
    }

    #[test]
    fn les_tentes_d_un_camp_ne_se_superposent_pas() {
        all_camps(|l, _| {
            for (i, a) in l.tents.iter().enumerate() {
                for b in &l.tents[i + 1..] {
                    assert!(a.at.distance(b.at) >= 6.8, "{a:?} {b:?}");
                }
                if let Some(t) = l.tower {
                    assert!(!overlap(&rect(a.at, Vec2::splat(3.4), 0.0), &rect(t, Vec2::splat(3.3), 0.0)));
                }
            }
        });
    }

    #[test]
    fn un_camp_reste_hors_de_l_eau() {
        all_camps(|l, site| {
            for t in &l.tents {
                assert!(site.water_gap(t.at) >= BANK_WIDTH);
            }
            for s in &l.stakes {
                assert!(site.water_gap(s.at) >= BANK_WIDTH);
            }
        });
    }

    #[test]
    fn chaque_camp_a_une_seule_tente_de_chef_et_ses_defenses() {
        all_camps(|l, _| {
            assert_eq!(l.tents.iter().filter(|t| t.chief).count(), usize::from(!l.tents.is_empty()));
            match l.kind {
                CampKind::Fort => assert!(l.tower.is_some() && l.rocks.is_empty()),
                CampKind::Lair => assert!(l.tower.is_none()),
                CampKind::Encampment => assert!(l.rocks.is_empty()),
            }
        });
        let flat = camp_layout(&Test::Flat, Family::Forest, CampKind::Lair, 3);
        assert!(!flat.rocks.is_empty() && flat.cave.is_some());
        assert!(camp_layout(&Test::Flat, Family::Forest, CampKind::Encampment, 3).tents.len() >= 3);
    }

    #[test]
    fn la_pose_d_un_camp_est_deterministe() {
        assert_eq!(camp_layout(&Test::Slope, Family::Arid, CampKind::Lair, 9), camp_layout(&Test::Slope, Family::Arid, CampKind::Lair, 9));
    }

    #[test]
    fn un_village_n_a_pas_de_muraille() {
        for site in SITES {
            let l = village_layout(&site, Family::Forest, 3);
            assert!(l.wall.is_none() && l.gates.is_empty() && l.hearth);
            for (i, a) in l.buildings.iter().enumerate() {
                for b in &l.buildings[i + 1..] {
                    assert!(!overlap(&a.corners(), &b.corners()));
                }
            }
        }
    }
}
