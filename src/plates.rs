//! Plaques tectoniques (logique pure) : la planète est découpée en une douzaine de plaques de
//! forme irrégulière, continentales (épaisses, elles portent les terres) ou océaniques (fond
//! marin), qui se déplacent chacune dans sa direction. Le long des frontières, le mouvement
//! relatif des deux plaques dit ce qui s'y passe :
//! - elles se rapprochent : collision (deux continents : grande chaîne et haut plateau),
//!   subduction (l'océan plonge sous le continent : chaîne côtière et fosse au large), arc
//!   d'îles (deux océans) ;
//! - elles s'écartent : rift (fossé d'effondrement dans un continent) ou dorsale (sous la mer) ;
//! - elles glissent l'une contre l'autre : faille transformante.
//!
//! Des points chauds, fixes sous les plaques, percent la croûte loin des frontières (chapelets
//! d'îles volcaniques). La carte boucle d'est en ouest : les plaques passent le raccord.
//! Cette étape ne fait que les décrire (carte « Plaques ») ; les continents et les chaînes de
//! montagnes en découleront ensuite.

use crate::hex::{DIRECTIONS, Hex, Wrap};
use bevy::math::Vec2;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Nombre de plaques (tiré entre ces bornes).
const MIN_PLATES: usize = 10;
const MAX_PLATES: usize = 14;
/// Part de la surface couverte par les plaques continentales (visée).
const CONTINENTAL_SHARE: f32 = 0.4;
/// Vitesses (cm/an) : les plaques océaniques vont plus vite que les continentales.
const OCEANIC_SPEED: (f32, f32) = (3.0, 9.0);
const CONTINENTAL_SPEED: (f32, f32) = (1.0, 4.5);
/// En deçà de cette vitesse de rapprochement ou d'écartement (cm/an), la frontière est
/// transformante (les plaques glissent l'une contre l'autre).
const CONVERGENCE_MIN: f32 = 1.2;
/// Nombre de points chauds (tiré entre ces bornes) et distance minimale à une frontière (cases).
const HOTSPOTS: (usize, usize) = (3, 6);
const HOTSPOT_FROM_BOUNDARY: i32 = 5;
/// Ampleur de la déformation des frontières (cases).
const WARP_CELLS: f64 = 9.0;
/// Rayon (cases) sur lequel on lisse la direction d'une frontière.
const BOUNDARY_SMOOTH: f32 = 3.5;

/// Nature d'une plaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateKind {
    Continental,
    Oceanic,
}

impl PlateKind {
    pub fn name(self) -> &'static str {
        match self {
            PlateKind::Continental => "continentale",
            PlateKind::Oceanic => "océanique",
        }
    }
}

/// Une plaque : nom, nature, vitesse (cm/an ; x vers l'est, y vers le nord), taille (cases) et
/// case de départ de sa croissance.
#[derive(Clone, Debug)]
pub struct Plate {
    pub name: &'static str,
    pub kind: PlateKind,
    pub velocity: Vec2,
    #[cfg_attr(not(test), allow(dead_code))]
    pub cells: usize,
    #[cfg_attr(not(test), allow(dead_code))]
    pub seed: Hex,
}

/// Nature d'une frontière entre deux plaques.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Boundary {
    /// Deux continents qui se percutent.
    Collision,
    /// Un océan qui plonge sous un continent.
    Subduction,
    /// Deux océans qui se rapprochent : l'un plonge sous l'autre.
    IslandArc,
    /// Deux continents qui s'écartent.
    Rift,
    /// Plaques qui s'écartent, dont l'une au moins océanique.
    Ridge,
    /// Plaques qui glissent l'une contre l'autre.
    Transform,
}

impl Boundary {
    pub fn name(self) -> &'static str {
        match self {
            Boundary::Collision => "collision de continents",
            Boundary::Subduction => "subduction",
            Boundary::IslandArc => "arc d'îles",
            Boundary::Rift => "rift continental",
            Boundary::Ridge => "dorsale",
            Boundary::Transform => "faille transformante",
        }
    }

    /// Couleur de la frontière sur la carte (sRGB) : rapprochement en rouges, écartement en
    /// bleus, glissement en jaune.
    pub fn color(self) -> (f32, f32, f32) {
        match self {
            Boundary::Collision => (0.85, 0.12, 0.12),
            Boundary::Subduction => (0.95, 0.45, 0.10),
            Boundary::IslandArc => (0.85, 0.25, 0.65),
            Boundary::Rift => (0.20, 0.75, 0.70),
            Boundary::Ridge => (0.15, 0.45, 0.95),
            Boundary::Transform => (0.95, 0.85, 0.20),
        }
    }
}

/// Une arête de frontière : la case, la direction de sa voisine (de l'autre plaque), la nature
/// de la frontière et la vitesse de rapprochement (cm/an, négative : écartement).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryEdge {
    pub cell: usize,
    pub dir: u8,
    pub kind: Boundary,
    pub closing: f32,
}

/// Les plaques de la planète.
#[derive(Clone, Debug, Default)]
pub struct Plates {
    pub plates: Vec<Plate>,
    /// Plaque de chaque case.
    pub plate_of: Vec<u8>,
    /// Arêtes de frontière (chacune une seule fois).
    pub edges: Vec<BoundaryEdge>,
    /// Frontière la plus active qui touche chaque case.
    pub boundary: Vec<Option<Boundary>>,
    /// Points chauds (cases).
    pub hotspots: Vec<Hex>,
}

/// Noms (réels) des plaques océaniques, puis continentales.
const OCEANIC_NAMES: [&str; 9] = [
    "plaque pacifique",
    "plaque de Nazca",
    "plaque de Cocos",
    "plaque des Philippines",
    "plaque de Juan de Fuca",
    "plaque de Scotia",
    "plaque caraïbe",
    "plaque de Rivera",
    "plaque des Mariannes",
];
const CONTINENTAL_NAMES: [&str; 16] = [
    "plaque eurasienne",
    "plaque africaine",
    "plaque nord-américaine",
    "plaque sud-américaine",
    "plaque indienne",
    "plaque australienne",
    "plaque antarctique",
    "plaque arabique",
    "plaque somalienne",
    "plaque anatolienne",
    "plaque d'Okhotsk",
    "plaque de l'Amour",
    "plaque de la Sonde",
    "plaque du Yangtsé",
    "plaque iranienne",
    "plaque égéenne",
];

impl Plates {
    /// Plaque d'une case (indice dans `plates`).
    pub fn plate(&self, cell: usize) -> Option<&Plate> {
        self.plate_of.get(cell).and_then(|&p| self.plates.get(usize::from(p)))
    }
}

/// Génère les plaques d'une carte de `width` × `height` cases.
pub fn generate(width: i32, height: i32, wrap: Wrap, seed: u32) -> Plates {
    let n = (width * height) as usize;
    let mut rng = StdRng::seed_from_u64(u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x51A7E5);
    let hex_of = |i: usize| Hex::from_offset(i as i32 % width, i as i32 / width);
    let index = |h: Hex| {
        let (c, r) = wrap.hex(h).to_offset();
        (c >= 0 && c < width && r >= 0 && r < height).then(|| (r * width + c) as usize)
    };

    // Germes : bien répartis (distance minimale, relâchée si on n'en trouve pas)
    let count = rng.random_range(MIN_PLATES..=MAX_PLATES);
    let mut seeds: Vec<Hex> = Vec::with_capacity(count);
    let mut spacing = (width.min(height * 2) as f32 / (count as f32).sqrt() * 0.8) as i32;
    while seeds.len() < count {
        let mut placed = false;
        for _ in 0..200 {
            let h = Hex::from_offset(rng.random_range(0..width), rng.random_range(2..height - 2));
            if seeds.iter().all(|&s| wrap.distance(s, h) >= spacing) {
                seeds.push(h);
                placed = true;
                break;
            }
        }
        if !placed {
            spacing = (spacing * 4 / 5).max(1);
        }
    }

    // Croissance : chaque plaque s'étend depuis son germe à sa vitesse propre, sur un sol au
    // coût bruité (frontières sinueuses) ; une case revient à la plaque qui l'atteint d'abord
    let noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(271_828)).set_octaves(4).set_frequency(1.0);
    let scale = 6.0 / width as f64;
    let radius = width as f64 * scale / std::f64::consts::TAU;
    let roughness = |h: Hex| -> f32 {
        let (c, r) = h.to_offset();
        let v = if wrap.wraps() {
            let a = c as f64 / width as f64 * std::f64::consts::TAU;
            noise.get([a.cos() * radius, a.sin() * radius, r as f64 * scale])
        } else {
            noise.get([c as f64 * scale, r as f64 * scale])
        };
        (v as f32 * 0.5 + 0.5).clamp(0.0, 1.0)
    };
    let step_cost: Vec<f32> = (0..n).map(|i| 1.0 + 2.5 * roughness(hex_of(i))).collect();
    let growth: Vec<f32> = (0..count).map(|_| rng.random_range(0.6..1.5)).collect();
    let mut plate_of = vec![u8::MAX; n];
    let mut best = vec![f32::MAX; n];
    let key = |c: f32| (c * 1000.0) as u64;
    let mut queue: BinaryHeap<Reverse<(u64, usize, u8)>> = BinaryHeap::new();
    for (p, &s) in seeds.iter().enumerate() {
        let i = index(s).expect("germe sur la carte");
        best[i] = 0.0;
        queue.push(Reverse((0, i, p as u8)));
    }
    while let Some(Reverse((_, i, p))) = queue.pop() {
        if plate_of[i] != u8::MAX {
            continue;
        }
        plate_of[i] = p;
        for nb in wrap.neighbors(hex_of(i)) {
            let Some(j) = index(nb) else { continue };
            if plate_of[j] != u8::MAX {
                continue;
            }
            let c = best[i] + step_cost[j] / growth[usize::from(p)];
            if c < best[j] {
                best[j] = c;
                queue.push(Reverse((key(c), j, p)));
            }
        }
    }
    // Frontières sinueuses : chaque case prend la plaque d'une case voisine décalée par un bruit
    // lent (déformation du plan, quelques cases), puis les morceaux détachés d'une plaque
    // rejoignent la plaque qui les entoure
    let warp_x: Fbm<Perlin> = Fbm::new(seed.wrapping_add(31_415)).set_octaves(3).set_frequency(1.0);
    let warp_y: Fbm<Perlin> = Fbm::new(seed.wrapping_add(92_653)).set_octaves(3).set_frequency(1.0);
    let sample = |f: &Fbm<Perlin>, h: Hex, zoom: f64| -> f64 {
        let (c, r) = h.to_offset();
        if wrap.wraps() {
            let a = c as f64 / width as f64 * std::f64::consts::TAU;
            f.get([a.cos() * radius * zoom, a.sin() * radius * zoom, r as f64 * scale * zoom])
        } else {
            f.get([c as f64 * scale * zoom, r as f64 * scale * zoom])
        }
    };
    let warped: Vec<u8> = (0..n)
        .map(|i| {
            let h = hex_of(i);
            let (c, r) = h.to_offset();
            let dx = (sample(&warp_x, h, 0.8) * WARP_CELLS).round() as i32;
            let dy = (sample(&warp_y, h, 0.8) * WARP_CELLS * 0.7).round() as i32;
            let from = Hex::from_offset(c + dx, (r + dy).clamp(0, height - 1));
            index(from).map_or(plate_of[i], |j| plate_of[j])
        })
        .collect();
    let mut plate_of = warped;
    merge_fragments(&mut plate_of, &mut seeds, count, hex_of, |i| wrap.neighbors(hex_of(i)).into_iter().filter_map(index).collect());
    // Plaques avalées par leurs voisines : renumérotées
    let mut alive = vec![false; count];
    for &p in &plate_of {
        alive[usize::from(p)] = true;
    }
    let renumber: Vec<u8> = alive.iter().scan(0u8, |next, &a| {
        let id = *next;
        if a {
            *next += 1;
        }
        Some(id)
    }).collect();
    for p in plate_of.iter_mut() {
        *p = renumber[usize::from(*p)];
    }
    let seeds: Vec<Hex> = seeds.iter().zip(&alive).filter(|(_, a)| **a).map(|(s, _)| *s).collect();
    let count = seeds.len();
    let mut sizes = vec![0usize; count];
    for &p in &plate_of {
        sizes[usize::from(p)] += 1;
    }

    // Nature : des plaques tirées au hasard deviennent continentales jusqu'à couvrir la part
    // visée de la surface, les autres sont océaniques
    let mut order: Vec<usize> = (0..count).collect();
    order.shuffle(&mut rng);
    let mut kinds = vec![PlateKind::Oceanic; count];
    let mut covered = 0usize;
    for &p in &order {
        if (covered as f32) < CONTINENTAL_SHARE * n as f32 && (covered + sizes[p]) as f32 <= (CONTINENTAL_SHARE + 0.12) * n as f32 {
            kinds[p] = PlateKind::Continental;
            covered += sizes[p];
        }
    }

    // Mouvements et noms
    let mut oceanic: Vec<&'static str> = OCEANIC_NAMES.to_vec();
    let mut continental: Vec<&'static str> = CONTINENTAL_NAMES.to_vec();
    oceanic.shuffle(&mut rng);
    continental.shuffle(&mut rng);
    // La plus grande plaque océanique porte le nom de la plus grande plaque réelle
    oceanic.sort_by_key(|&nm| nm != "plaque pacifique");
    let mut by_size: Vec<usize> = (0..count).collect();
    by_size.sort_by_key(|&p| Reverse(sizes[p]));
    let mut names = vec![""; count];
    for &p in &by_size {
        let pool = if kinds[p] == PlateKind::Oceanic { &mut oceanic } else { &mut continental };
        names[p] = if pool.is_empty() { "plaque sans nom" } else { pool.remove(0) };
    }
    let plates: Vec<Plate> = (0..count)
        .map(|p| {
            let (lo, hi) = if kinds[p] == PlateKind::Oceanic { OCEANIC_SPEED } else { CONTINENTAL_SPEED };
            let speed = rng.random_range(lo..hi);
            let angle = rng.random_range(0.0..std::f32::consts::TAU);
            Plate { name: names[p], kind: kinds[p], velocity: Vec2::from_angle(angle) * speed, cells: sizes[p], seed: seeds[p] }
        })
        .collect();

    // Frontières : mouvement de la voisine par rapport à la case, projeté sur la direction qui
    // les joint (rapprochement) et sur la perpendiculaire (glissement)
    let center = |h: Hex| {
        let (x, y) = h.to_world();
        Vec2::new(x, y)
    };
    // Arêtes entre deux plaques (chacune une fois : directions 0 à 2) : milieu et direction
    // perpendiculaire (de la case vers la voisine)
    struct Raw {
        cell: usize,
        neighbor: usize,
        dir: u8,
        mid: Vec2,
        normal: Vec2,
    }
    let mut raw: Vec<Raw> = Vec::new();
    for i in 0..n {
        let h = hex_of(i);
        for (d, &step) in DIRECTIONS.iter().enumerate().take(3) {
            let Some(j) = index(h + step) else { continue };
            if plate_of[i] != plate_of[j] {
                let (a, b) = (center(h), center(h + step));
                raw.push(Raw { cell: i, neighbor: j, dir: d as u8, mid: (a + b) / 2.0, normal: (b - a).normalize() });
            }
        }
    }
    // La frontière en zigzag d'une grille hexagonale : la direction d'une seule arête varie de
    // ±60°. On prend celle de la frontière alentour (moyenne des arêtes entre les deux mêmes
    // plaques à moins de `BOUNDARY_SMOOTH` cases), orientée de la plaque de la case vers l'autre
    let world = wrap.world_width();
    let reach = BOUNDARY_SMOOTH * crate::hex::COLUMN_WIDTH;
    let mut by_pair: std::collections::HashMap<(u8, u8), Vec<usize>> = std::collections::HashMap::new();
    for (k, e) in raw.iter().enumerate() {
        let (a, b) = (plate_of[e.cell], plate_of[e.neighbor]);
        by_pair.entry((a.min(b), a.max(b))).or_default().push(k);
    }
    let mut edges = Vec::with_capacity(raw.len());
    let mut boundary: Vec<Option<(Boundary, f32)>> = vec![None; n];
    for (k, e) in raw.iter().enumerate() {
        let (a, b) = (plate_of[e.cell], plate_of[e.neighbor]);
        let mut normal = Vec2::ZERO;
        for &o in &by_pair[&(a.min(b), a.max(b))] {
            let other = &raw[o];
            let mut dx = other.mid.x - e.mid.x;
            if world > 0.0 {
                dx -= (dx / world).round() * world;
            }
            if Vec2::new(dx, other.mid.y - e.mid.y).length() <= reach {
                let flip = if plate_of[other.cell] == a { 1.0 } else { -1.0 };
                normal += other.normal * flip;
            }
        }
        let normal = normal.try_normalize().unwrap_or(raw[k].normal);
        let (pa, pb) = (usize::from(a), usize::from(b));
        let relative = plates[pb].velocity - plates[pa].velocity;
        let closing = -relative.dot(normal);
        let kind = classify(plates[pa].kind, plates[pb].kind, closing);
        for c in [e.cell, e.neighbor] {
            if boundary[c].is_none_or(|(_, s)| closing.abs() > s) {
                boundary[c] = Some((kind, closing.abs()));
            }
        }
        edges.push(BoundaryEdge { cell: e.cell, dir: e.dir, kind, closing });
    }
    let boundary: Vec<Option<Boundary>> = boundary.into_iter().map(|b| b.map(|(k, _)| k)).collect();

    // Points chauds : loin des frontières, de préférence sous un océan
    let mut far: Vec<usize> = (0..n)
        .filter(|&i| {
            let h = hex_of(i);
            let (_, r) = h.to_offset();
            r > 3 && r < height - 4 && wrap.range(h, HOTSPOT_FROM_BOUNDARY).filter_map(index).all(|j| boundary[j].is_none())
        })
        .collect();
    far.shuffle(&mut rng);
    far.sort_by_key(|&i| plates[usize::from(plate_of[i])].kind == PlateKind::Continental);
    let wanted = rng.random_range(HOTSPOTS.0..=HOTSPOTS.1);
    let mut hotspots: Vec<Hex> = Vec::new();
    for i in far {
        if hotspots.len() >= wanted {
            break;
        }
        let h = hex_of(i);
        if hotspots.iter().all(|&o| wrap.distance(o, h) >= 2 * HOTSPOT_FROM_BOUNDARY) {
            hotspots.push(h);
        }
    }

    Plates { plates, plate_of, edges, boundary, hotspots }
}

/// Morceaux détachés : seul le plus grand morceau de chaque plaque la garde ; les autres
/// rejoignent la plaque voisine la plus présente sur leur pourtour (de proche en proche). Le
/// germe de chaque plaque est replacé dans son morceau principal.
fn merge_fragments(plate_of: &mut [u8], seeds: &mut [Hex], count: usize, hex_of: impl Fn(usize) -> Hex, neighbors: impl Fn(usize) -> Vec<usize>) {
    let n = plate_of.len();
    loop {
        let mut component = vec![usize::MAX; n];
        let mut comps: Vec<(u8, Vec<usize>)> = Vec::new();
        for start in 0..n {
            if component[start] != usize::MAX {
                continue;
            }
            let id = comps.len();
            let p = plate_of[start];
            let mut cells = vec![start];
            component[start] = id;
            let mut k = 0;
            while k < cells.len() {
                for j in neighbors(cells[k]) {
                    if component[j] == usize::MAX && plate_of[j] == p {
                        component[j] = id;
                        cells.push(j);
                    }
                }
                k += 1;
            }
            comps.push((p, cells));
        }
        // Morceau principal de chaque plaque
        let mut main = vec![usize::MAX; count];
        for (id, (p, cells)) in comps.iter().enumerate() {
            let m = &mut main[usize::from(*p)];
            if *m == usize::MAX || comps[*m].1.len() < cells.len() {
                *m = id;
            }
        }
        let mut changed = false;
        for (id, (p, cells)) in comps.iter().enumerate() {
            if main[usize::from(*p)] == id {
                continue;
            }
            let mut votes = vec![0usize; count];
            for &c in cells {
                for j in neighbors(c) {
                    if plate_of[j] != *p {
                        votes[usize::from(plate_of[j])] += 1;
                    }
                }
            }
            if let Some((q, _)) = votes.iter().enumerate().filter(|(_, v)| **v > 0).max_by_key(|(q, v)| (**v, std::cmp::Reverse(*q))) {
                for &c in cells {
                    plate_of[c] = q as u8;
                }
                changed = true;
            }
        }
        if !changed {
            for (p, &id) in main.iter().enumerate() {
                if id != usize::MAX {
                    seeds[p] = hex_of(comps[id].1[comps[id].1.len() / 2]);
                }
            }
            return;
        }
    }
}

/// Nature d'une frontière selon la nature des deux plaques et leur rapprochement (cm/an).
pub fn classify(a: PlateKind, b: PlateKind, closing: f32) -> Boundary {
    use PlateKind::*;
    if closing >= CONVERGENCE_MIN {
        match (a, b) {
            (Continental, Continental) => Boundary::Collision,
            (Oceanic, Oceanic) => Boundary::IslandArc,
            _ => Boundary::Subduction,
        }
    } else if closing <= -CONVERGENCE_MIN {
        if a == Continental && b == Continental { Boundary::Rift } else { Boundary::Ridge }
    } else {
        Boundary::Transform
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planet(seed: u32) -> Plates {
        generate(192, 111, Wrap::new(192, true), seed)
    }

    #[test]
    fn plaques_couvrent_la_planete() {
        for seed in [1, 777, 2026] {
            let p = planet(seed);
            assert!((MIN_PLATES..=MAX_PLATES).contains(&p.plates.len()));
            assert!(p.plate_of.iter().all(|&k| usize::from(k) < p.plates.len()), "chaque case a sa plaque");
            assert_eq!(p.plates.iter().map(|pl| pl.cells).sum::<usize>(), 192 * 111);
            // Chaque plaque d'un seul tenant (connexe, raccord compris)
            let wrap = Wrap::new(192, true);
            for (k, plate) in p.plates.iter().enumerate() {
                let index = |h: Hex| {
                    let (c, r) = wrap.hex(h).to_offset();
                    (0..111).contains(&r).then(|| (r * 192 + c) as usize)
                };
                let mut seen = vec![false; 192 * 111];
                let start = index(plate.seed).unwrap();
                seen[start] = true;
                let mut stack = vec![plate.seed];
                let mut size = 1;
                while let Some(h) = stack.pop() {
                    for nb in wrap.neighbors(h) {
                        if let Some(j) = index(nb)
                            && !seen[j]
                            && usize::from(p.plate_of[j]) == k
                        {
                            seen[j] = true;
                            size += 1;
                            stack.push(nb);
                        }
                    }
                }
                assert_eq!(size, plate.cells, "graine {seed} : {} d'un seul tenant", plate.name);
            }
            // Environ 40 % de surface continentale, des deux natures
            let cont: usize = p.plates.iter().filter(|pl| pl.kind == PlateKind::Continental).map(|pl| pl.cells).sum();
            let share = cont as f32 / (192.0 * 111.0);
            assert!((0.25..0.6).contains(&share), "graine {seed} : {share}");
            // Noms tous différents
            let mut names: Vec<_> = p.plates.iter().map(|pl| pl.name).collect();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), p.plates.len());
            // Des frontières de plusieurs natures, et des points chauds loin d'elles
            let kinds: std::collections::HashSet<_> = p.edges.iter().map(|e| e.kind).collect();
            assert!(kinds.len() >= 3, "graine {seed} : {kinds:?}");
            assert!(!p.hotspots.is_empty());
        }
    }

    #[test]
    fn les_plaques_passent_le_raccord() {
        // Des plaques touchent le raccord des deux côtés, et les frontières le traversent sans
        // en créer une artificielle : de part et d'autre, une même plaque continue
        let p = planet(777);
        let same = (0..111).filter(|&r| p.plate_of[(r * 192) as usize] == p.plate_of[(r * 192 + 191) as usize]).count();
        assert!(same > 60, "{same} lignes sur 111 : la même plaque de part et d'autre du raccord");
    }

    #[test]
    fn nature_des_frontieres() {
        use PlateKind::*;
        assert_eq!(classify(Continental, Continental, 4.0), Boundary::Collision);
        assert_eq!(classify(Oceanic, Continental, 4.0), Boundary::Subduction);
        assert_eq!(classify(Continental, Oceanic, 4.0), Boundary::Subduction);
        assert_eq!(classify(Oceanic, Oceanic, 4.0), Boundary::IslandArc);
        assert_eq!(classify(Continental, Continental, -4.0), Boundary::Rift);
        assert_eq!(classify(Oceanic, Continental, -4.0), Boundary::Ridge);
        assert_eq!(classify(Oceanic, Oceanic, 0.5), Boundary::Transform);
    }

    #[test]
    fn deterministe() {
        let (a, b) = (planet(42), planet(42));
        assert_eq!(a.plate_of, b.plate_of);
        assert_ne!(planet(43).plate_of, a.plate_of);
    }
}
