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
//! d'îles volcaniques). Tout se calcule sur la carte elle-même, le monde du jeu : elle boucle
//! d'est en ouest (les plaques passent le raccord), les pôles sont ses bords, une case a la même
//! taille partout (pas de plaques étirées en bandes près des pôles comme sur une sphère
//! projetée).
//! Sur une planète, le relief en découle (`Plates::elevation`) : continents sur les plaques
//! continentales, chaînes, fosses, arcs d'îles, rifts et chapelets d'îles le long des frontières
//! et au-dessus des points chauds.

use crate::hex::{DIRECTIONS, Hex, Wrap};
use bevy::math::{Vec2, Vec3};
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
/// Passes d'adoucissement du relief des frontières.
const LIFT_SMOOTHING: usize = 4;
/// Rotation d'une plaque sur elle-même : au plus cette part de sa vitesse par 1 000 km.
const SPIN_SHARE: f32 = 0.35;

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

/// Une plaque : nom, nature, mouvement (dérive et rotation autour de son centre, sur la carte),
/// taille (cases) et case de départ de sa croissance.
#[derive(Clone, Debug)]
pub struct Plate {
    /// Nom (« plaque de Vaskar »), donné par la toponymie.
    pub name: String,
    pub kind: PlateKind,
    /// Dérive (cm/an ; x vers l'est, y vers le nord).
    pub drift: Vec2,
    /// Rotation autour de son centre (cm/an par 1 000 km d'éloignement, sens trigonométrique).
    pub spin: f32,
    /// Centre de la rotation (km sur la carte).
    pub center: Vec2,
    #[cfg_attr(not(test), allow(dead_code))]
    pub cells: usize,
    #[cfg_attr(not(test), allow(dead_code))]
    pub seed: Hex,
}

impl Plate {
    /// Vitesse (cm/an) de la plaque au point `p` (km) d'une carte large de `world_km`.
    pub fn velocity_at(&self, p: Vec2, world_km: f32) -> Vec2 {
        let d = offset(self.center, p, world_km);
        self.drift + Vec2::new(-d.y, d.x) * (self.spin / 1000.0)
    }
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

    /// Les plaques se rapprochent.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn converging(self) -> bool {
        matches!(self, Boundary::Collision | Boundary::Subduction | Boundary::IslandArc)
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
    /// Case voisine (de l'autre plaque).
    pub neighbor: usize,
    /// Plaque qui plonge sous l'autre (subduction : l'océanique ; arc d'îles : celle qui
    /// avance le plus vite vers la frontière), `u8::MAX` sinon.
    pub under: u8,
}

/// Les plaques de la planète.
#[derive(Clone, Debug, Default)]
pub struct Plates {
    pub plates: Vec<Plate>,
    /// Plaque de chaque case.
    pub plate_of: Vec<u8>,
    /// Vitesse de chaque case (cm/an ; x vers l'est, y vers le nord).
    pub velocity: Vec<Vec2>,
    /// Arêtes de frontière (chacune une seule fois).
    pub edges: Vec<BoundaryEdge>,
    /// Frontière la plus active qui touche chaque case.
    pub boundary: Vec<Option<Boundary>>,
    /// Distance de chaque case à la frontière la plus proche (km, sur la carte).
    #[cfg_attr(not(test), allow(dead_code))]
    pub boundary_km: Vec<f32>,
    /// Points chauds (cases).
    pub hotspots: Vec<Hex>,
    /// Arête de frontière la plus proche de chaque case (indice dans `edges`).
    pub nearest_edge: Vec<u32>,
    /// Position de chaque case sur la carte (km ; x vers l'est, y vers le nord) et tour de la
    /// planète (km : la largeur de la carte).
    pub pos: Vec<Vec2>,
    pub world_km: f32,
    /// Largeur de la carte (cases).
    pub width: i32,
}

impl Plates {
    /// Plaque d'une case (indice dans `plates`).
    pub fn plate(&self, cell: usize) -> Option<&Plate> {
        self.plate_of.get(cell).and_then(|&p| self.plates.get(usize::from(p)))
    }
}

/// Position d'une case sur la carte (km ; x vers l'est, y vers le nord, la ligne du haut à 0),
/// pour des cases de `cell_km` d'un centre à l'autre. Le monde du jeu est la carte elle-même :
/// elle boucle d'est en ouest (un cylindre) et les pôles sont ses bords. Une case a la même
/// taille partout ; seule la latitude (le climat) dit qu'on approche d'un pôle.
pub fn map_point(hex: Hex, cell_km: f32) -> Vec2 {
    let (x, y) = hex.to_world();
    Vec2::new(x, y) * (cell_km / crate::hex::COLUMN_WIDTH)
}

/// Décalage de `a` à `b` (km) par le plus court côté du raccord est-ouest.
pub fn offset(a: Vec2, b: Vec2, world_km: f32) -> Vec2 {
    let mut d = b - a;
    if world_km > 0.0 {
        d.x -= (d.x / world_km).round() * world_km;
    }
    d
}

/// Point du cylindre (km) qui porte la carte : les bruits y sont lus sans couture au raccord et à
/// la même échelle partout.
pub fn cylinder(p: Vec2, world_km: f32) -> Vec3 {
    let r = world_km / std::f32::consts::TAU;
    let a = p.x / r;
    Vec3::new(r * a.cos(), r * a.sin(), p.y)
}

/// Génère les plaques d'une carte de `width` × `height` cases de `cell_km` (calcul sur la carte,
/// qui boucle d'est en ouest).
pub fn generate(width: i32, height: i32, wrap: Wrap, seed: u32, cell_km: f32) -> Plates {
    let n = (width * height) as usize;
    let mut rng = StdRng::seed_from_u64(u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x51A7E5);
    let hex_of = |i: usize| Hex::from_offset(i as i32 % width, i as i32 / width);
    let index = |h: Hex| {
        let (c, r) = wrap.hex(h).to_offset();
        (c >= 0 && c < width && r >= 0 && r < height).then(|| (r * width + c) as usize)
    };
    let pos: Vec<Vec2> = (0..n).map(|i| map_point(hex_of(i), cell_km)).collect();
    let world_km = width as f32 * cell_km;
    let (y_lo, y_hi) = pos.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
    let dist = |a: Vec2, b: Vec2| offset(a, b, world_km).length();
    // Case la plus proche d'un point de la carte (au-delà d'un pôle : le bord)
    let to_world = crate::hex::COLUMN_WIDTH / cell_km;
    let cell_at = |p: Vec2| -> usize {
        let y = p.y.clamp(y_lo, y_hi);
        index(Hex::from_world(p.x.rem_euclid(world_km) * to_world, y * to_world)).unwrap_or(0)
    };
    // Bruits lus sur le cylindre de rayon 1 (une longueur d'onde de zoom⁻¹ × rayon)
    let radius = world_km / std::f32::consts::TAU;
    let unit: Vec<Vec3> = pos.iter().map(|&p| cylinder(p, world_km) / radius).collect();
    let at = |f: &Fbm<Perlin>, p: Vec3, zoom: f64| f.get([f64::from(p.x) * zoom, f64::from(p.y) * zoom, f64::from(p.z) * zoom]);

    // Germes : tirés uniformément sur la carte, bien répartis (distance minimale, relâchée si on
    // n'en trouve pas)
    let count = rng.random_range(MIN_PLATES..=MAX_PLATES);
    let mut seeds: Vec<Hex> = Vec::with_capacity(count);
    let mut spacing = (world_km * (y_hi - y_lo) / count as f32).sqrt() * 0.9;
    while seeds.len() < count {
        let mut placed = false;
        for _ in 0..200 {
            let p = Vec2::new(rng.random_range(0.0..world_km), rng.random_range(y_lo..=y_hi));
            let c = cell_at(p);
            if seeds.iter().all(|&s| dist(pos[index(s).unwrap_or(0)], pos[c]) >= spacing) {
                seeds.push(hex_of(c));
                placed = true;
                break;
            }
        }
        if !placed {
            spacing *= 0.8;
        }
    }

    // Croissance : chaque plaque s'étend depuis son germe à sa vitesse propre, sur un sol au coût
    // bruité (frontières sinueuses) ; une case revient à la plaque qui l'atteint d'abord
    let noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(271_828)).set_octaves(4).set_frequency(1.0);
    let roughness: Vec<f32> = unit.iter().map(|&p| (at(&noise, p, 3.0) as f32 * 0.5 + 0.5).clamp(0.0, 1.0)).collect();
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
    let neighbors_of = |i: usize| -> Vec<usize> { wrap.neighbors(hex_of(i)).into_iter().filter_map(index).collect() };
    while let Some(Reverse((_, i, p))) = queue.pop() {
        if plate_of[i] != u8::MAX {
            continue;
        }
        plate_of[i] = p;
        for j in neighbors_of(i) {
            if plate_of[j] != u8::MAX {
                continue;
            }
            let step = dist(pos[i], pos[j]) / cell_km * (1.0 + 2.5 * roughness[j]);
            let c = best[i] + step / growth[usize::from(p)];
            if c < best[j] {
                best[j] = c;
                queue.push(Reverse((key(c), j, p)));
            }
        }
    }
    // Frontières sinueuses : chaque case prend la plaque du point décalé par un bruit lent
    // (quelques centaines de km), puis les morceaux détachés d'une plaque rejoignent la plaque
    // qui les entoure
    let warp: [Fbm<Perlin>; 2] = [31_415u32, 92_653].map(|k| Fbm::new(seed.wrapping_add(k)).set_octaves(3).set_frequency(1.0));
    let reach_km = WARP_CELLS as f32 * cell_km;
    let warped: Vec<u8> = (0..n)
        .map(|i| {
            let d = Vec2::new(at(&warp[0], unit[i], 1.6) as f32, at(&warp[1], unit[i], 1.6) as f32);
            plate_of[cell_at(pos[i] + d * reach_km)]
        })
        .collect();
    let mut plate_of = warped;
    merge_fragments(&mut plate_of, &mut seeds, count, hex_of, neighbors_of);
    // Plaques avalées par leurs voisines : renumérotées
    let mut alive = vec![false; count];
    for &p in &plate_of {
        alive[usize::from(p)] = true;
    }
    let renumber: Vec<u8> = alive
        .iter()
        .scan(0u8, |next, &a| {
            let id = *next;
            if a {
                *next += 1;
            }
            Some(id)
        })
        .collect();
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
    // visée de la carte, les autres sont océaniques
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

    // Mouvements : chaque plaque dérive dans une direction tirée au hasard, à une vitesse selon
    // sa nature, en tournant un peu sur elle-même (les noms sont donnés ensuite par la toponymie)
    let plates: Vec<Plate> = (0..count)
        .map(|p| {
            let (lo, hi) = if kinds[p] == PlateKind::Oceanic { OCEANIC_SPEED } else { CONTINENTAL_SPEED };
            let speed = rng.random_range(lo..hi);
            let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
            let spin = rng.random_range(-1.0..1.0) * speed * SPIN_SHARE;
            let center = pos[index(seeds[p]).unwrap_or(0)];
            Plate { name: format!("plaque {}", p + 1), kind: kinds[p], drift: Vec2::from_angle(angle) * speed, spin, center, cells: sizes[p], seed: seeds[p] }
        })
        .collect();
    let speed_at = |p: u8, at: Vec2| plates[usize::from(p)].velocity_at(at, world_km);
    let velocity: Vec<Vec2> = (0..n).map(|i| speed_at(plate_of[i], pos[i])).collect();

    // Frontières : arêtes entre deux plaques (chacune une fois : directions 0 à 2), milieu et
    // direction de la case vers la voisine
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
                let along = offset(pos[i], pos[j], world_km);
                raw.push(Raw { cell: i, neighbor: j, dir: d as u8, mid: pos[i] + along / 2.0, normal: along.normalize_or(Vec2::X) });
            }
        }
    }
    // La frontière en zigzag d'une grille hexagonale : la direction d'une seule arête varie de
    // ±60°. On prend celle de la frontière alentour (moyenne des arêtes entre les deux mêmes
    // plaques à moins de `BOUNDARY_SMOOTH` cases), orientée de la plaque de la case vers l'autre
    let reach = BOUNDARY_SMOOTH * cell_km;
    let mut by_pair: std::collections::HashMap<(u8, u8), Vec<usize>> = std::collections::HashMap::new();
    for (k, e) in raw.iter().enumerate() {
        let (a, b) = (plate_of[e.cell], plate_of[e.neighbor]);
        by_pair.entry((a.min(b), a.max(b))).or_default().push(k);
    }
    let mut edges = Vec::with_capacity(raw.len());
    let mut boundary: Vec<Option<(Boundary, f32)>> = vec![None; n];
    let mut cell_edge = vec![u32::MAX; n];
    for e in &raw {
        let (a, b) = (plate_of[e.cell], plate_of[e.neighbor]);
        let mut normal = Vec2::ZERO;
        for &o in &by_pair[&(a.min(b), a.max(b))] {
            let other = &raw[o];
            if dist(other.mid, e.mid) <= reach {
                let flip = if plate_of[other.cell] == a { 1.0 } else { -1.0 };
                normal += other.normal * flip;
            }
        }
        let normal = normal.try_normalize().unwrap_or(e.normal);
        let (pa, pb) = (usize::from(a), usize::from(b));
        let (va, vb) = (speed_at(a, e.mid), speed_at(b, e.mid));
        let closing = -(vb - va).dot(normal);
        let kind = classify(plates[pa].kind, plates[pb].kind, closing);
        for c in [e.cell, e.neighbor] {
            if boundary[c].is_none_or(|(_, s)| closing.abs() > s) {
                boundary[c] = Some((kind, closing.abs()));
                cell_edge[c] = edges.len() as u32;
            }
        }
        let under = match kind {
            Boundary::Subduction => {
                if plates[pa].kind == PlateKind::Oceanic { a } else { b }
            }
            // Celle qui avance le plus vite vers la frontière
            Boundary::IslandArc => {
                if va.dot(normal) >= -vb.dot(normal) { a } else { b }
            }
            _ => u8::MAX,
        };
        edges.push(BoundaryEdge { cell: e.cell, dir: e.dir, kind, closing, neighbor: e.neighbor, under });
    }
    let boundary: Vec<Option<Boundary>> = boundary.into_iter().map(|b| b.map(|(k, _)| k)).collect();

    // Distance (km) de chaque case à la frontière la plus proche
    let mut boundary_km = vec![f32::MAX; n];
    let mut nearest_edge = cell_edge;
    let mut queue: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
    for (i, b) in boundary.iter().enumerate() {
        if b.is_some() {
            boundary_km[i] = 0.0;
            queue.push(Reverse((0, i)));
        }
    }
    while let Some(Reverse((k, i))) = queue.pop() {
        if k > key(boundary_km[i]) {
            continue;
        }
        for j in neighbors_of(i) {
            let d = boundary_km[i] + dist(pos[i], pos[j]);
            if d < boundary_km[j] {
                boundary_km[j] = d;
                nearest_edge[j] = nearest_edge[i];
                queue.push(Reverse((key(d), j)));
            }
        }
    }

    // Points chauds : loin des frontières, de préférence sous un océan, espacés
    let far: Vec<usize> = (0..n).filter(|&i| boundary_km[i] >= HOTSPOT_FROM_BOUNDARY as f32 * cell_km).collect();
    let mut keyed: Vec<(bool, f32, usize)> =
        far.into_iter().map(|i| (plates[usize::from(plate_of[i])].kind == PlateKind::Continental, rng.random_range(0.0..1.0f32), i)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let far: Vec<usize> = keyed.into_iter().map(|(_, _, i)| i).collect();
    let wanted = rng.random_range(HOTSPOTS.0..=HOTSPOTS.1);
    let mut hotspots: Vec<Hex> = Vec::new();
    for i in far {
        if hotspots.len() >= wanted {
            break;
        }
        if hotspots.iter().all(|&o| dist(pos[index(o).unwrap_or(0)], pos[i]) >= 2.0 * HOTSPOT_FROM_BOUNDARY as f32 * cell_km) {
            hotspots.push(hex_of(i));
        }
    }

    Plates { plates, plate_of, velocity, edges, boundary, boundary_km, hotspots, nearest_edge, pos, world_km, width }
}

/// Profil en cloche : 1 en 0, ≈ 0,37 à `width`.
fn bell(x: f32, width: f32) -> f32 {
    (-(x / width).powi(2)).exp()
}

impl Plates {
    /// Altitude relative (sans unité ; la mer en dessous de 0) de chaque case, issue des plaques :
    /// - socle : haut sur les plaques continentales, bas sur les océaniques, découpé par un
    ///   bruit lent (côtes, mers peu profondes, îles) ;
    /// - le long des frontières, selon leur nature et la vitesse de rapprochement : grande chaîne
    ///   et haut plateau d'une collision, cordillère côtière d'une subduction (côté continent) et
    ///   fosse au large (côté océan), arc d'îles, fossé et épaulements d'un rift, dorsale ;
    /// - chapelets d'îles des points chauds, vieillissant dans le sens du mouvement de la plaque ;
    /// - collines et vieux massifs à l'intérieur des continents.
    ///
    /// Tous les bruits sont lus sur le cylindre de la carte : même échelle partout.
    pub fn elevation(&self, seed: u32, cell_km: f32) -> Vec<f64> {
        let n = self.plate_of.len();
        let fbm = |k: u32, octaves: usize| -> Fbm<Perlin> { Fbm::new(seed.wrapping_add(k)).set_octaves(octaves).set_frequency(1.0) };
        let (coast, shelf, hills, arc) = (fbm(101, 5), fbm(202, 3), fbm(303, 4), fbm(404, 3));
        let old = noise::RidgedMulti::<Perlin>::new(seed.wrapping_add(505)).set_octaves(4);
        // Bruit au point p (km, sur le cylindre), pour une longueur d'onde donnée (km)
        let at = |f: &dyn NoiseFn<f64, 3>, p: Vec3, wavelength_km: f32| {
            let z = 1.0 / f64::from(wavelength_km);
            f.get([f64::from(p.x) * z, f64::from(p.y) * z, f64::from(p.z) * z]) as f32
        };
        let parts: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let p = cylinder(self.pos[i], self.world_km);
                let plate = &self.plates[usize::from(self.plate_of[i])];
                let continental = plate.kind == PlateKind::Continental;
                // Socle
                let mut e = if continental {
                    0.30 + 0.30 * at(&coast, p, 2_400.0) + 0.16 * at(&shelf, p, 5_000.0)
                } else {
                    -0.40 + 0.18 * at(&coast, p, 2_400.0)
                };
                if continental {
                    e += 0.10 * at(&hills, p, 700.0) + 0.12 * (at(&old, p, 1_800.0) * 0.5 + 0.2).max(0.0);
                    // Intérieur des continents un peu plus haut que leurs bords : l'eau s'écoule
                    // vers les côtes au lieu de stagner dans les creux du bruit
                    e += 0.14 * (1.0 - bell(self.boundary_km[i], 900.0));
                }
                let mut lift = 0.0;
                // Frontière la plus proche
                let edge = self.nearest_edge.get(i).copied().unwrap_or(u32::MAX);
                if let Some(edge) = self.edges.get(edge as usize) {
                    let d = self.boundary_km[i];
                    let mine = self.plate_of[i];
                    let sides = (self.plate_of[edge.cell], self.plate_of[edge.neighbor]);
                    let on_edge = mine == sides.0 || mine == sides.1;
                    let f = (edge.closing.abs() / 4.0).clamp(0.5, 1.6);
                    let under = edge.under == mine;
                    lift = match edge.kind {
                        _ if !on_edge => 0.0,
                        Boundary::Collision => f * (0.62 * bell(d, 180.0) + 0.26 * bell(d, 600.0)),
                        Boundary::Subduction if under => -0.28 * f * bell(d, 90.0),
                        Boundary::Subduction => f * (0.55 * bell(d - 150.0, 130.0) + 0.12 * bell(d, 450.0)),
                        Boundary::IslandArc if under => -0.28 * f * bell(d, 90.0),
                        Boundary::IslandArc => f * 0.62 * bell(d - 130.0, 75.0) * (0.45 + 0.55 * (at(&arc, p, 300.0) * 0.5 + 0.5)),
                        Boundary::Rift => f * (-0.24 * bell(d, 70.0) + 0.09 * bell(d - 130.0, 70.0)),
                        Boundary::Ridge if !continental => 0.16 * f * bell(d, 170.0),
                        Boundary::Ridge => 0.0,
                        Boundary::Transform => 0.05 * at(&hills, p, 300.0) * bell(d, 120.0),
                    };
                }
                (e, lift)
            })
            .collect();
        // Le relief des frontières vient de la frontière la plus proche : là où la plus proche
        // change (à mi-chemin de deux frontières), il saute. On l'adoucit (moyenne avec les
        // voisines, quelques passes) : plus d'arêtes droites au milieu des continents
        let neighbors: Vec<Vec<usize>> = self.neighbor_graph(cell_km).into_iter().map(|v| v.into_iter().map(|(j, _)| j).collect()).collect();
        let mut lift: Vec<f32> = parts.iter().map(|&(_, l)| l).collect();
        for _ in 0..LIFT_SMOOTHING {
            lift = (0..n)
                .map(|i| {
                    let around: f32 = neighbors[i].iter().map(|&j| lift[j]).sum();
                    (lift[i] * 2.0 + around) / (2.0 + neighbors[i].len() as f32)
                })
                .collect();
        }
        let mut out: Vec<f64> = (0..n).map(|i| f64::from(parts[i].0 + lift[i])).collect();
        // Points chauds : une île sur le point chaud, puis d'anciennes îles de plus en plus basses
        // là où la plaque les a emportées (dans le sens de son mouvement)
        let step = 2.6 * cell_km;
        for h in &self.hotspots {
            let (c, r) = h.to_offset();
            let Some(i) = self.cell_of(c, r) else { continue };
            let p = self.pos[i];
            let dir = self.velocity[i].normalize_or(Vec2::X);
            for k in 0..6 {
                let q = p + dir * (step * k as f32);
                let height = 0.75 * (1.0 - k as f32 / 6.5);
                let reach = (1.3 - 0.1 * k as f32) * cell_km;
                for (j, &pj) in self.pos.iter().enumerate() {
                    let dist = offset(q, pj, self.world_km).length();
                    if dist < 3.0 * reach {
                        out[j] = out[j].max(out[j] * 0.3 + f64::from(height * bell(dist, reach)) - 0.05);
                    }
                }
            }
        }
        out
    }

    /// Voisines de chaque case et distance (en tailles de case de `cell_km`) sur la carte.
    pub fn neighbor_graph(&self, cell_km: f32) -> Vec<Vec<(usize, f32)>> {
        let n = self.plate_of.len();
        let width = self.width.max(1);
        let height = n as i32 / width;
        let wrap = Wrap::new(width, true);
        (0..n)
            .map(|i| {
                let h = Hex::from_offset(i as i32 % width, i as i32 / width);
                wrap.neighbors(h)
                    .into_iter()
                    .filter_map(|nb| {
                        let (c, r) = nb.to_offset();
                        (0..height).contains(&r).then_some((r * width + c) as usize)
                    })
                    .map(|j| (j, offset(self.pos[i], self.pos[j], self.world_km).length() / cell_km))
                    .collect()
            })
            .collect()
    }

    /// Surface relative de chaque case : toutes égales (le monde du jeu est la carte).
    pub fn cell_area(&self) -> Vec<f32> {
        vec![1.0; self.plate_of.len()]
    }

    /// Indice d'une case (colonne, ligne).
    fn cell_of(&self, col: i32, row: i32) -> Option<usize> {
        let width = self.width;
        (width > 0 && col >= 0 && col < width && row >= 0).then(|| (row * width + col) as usize).filter(|&i| i < self.plate_of.len())
    }
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
        generate(192, 111, Wrap::new(192, true), seed, 40.0)
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
            let mut names: Vec<_> = p.plates.iter().map(|pl| pl.name.as_str()).collect();
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
    fn mouvement_sur_la_carte() {
        for seed in [3, 777] {
            let p = planet(seed);
            // Les cases ont toutes la même taille : une case et sa voisine sont à 40 km, raccord
            // compris
            let graph = p.neighbor_graph(40.0);
            assert!(graph.iter().flatten().all(|&(_, d)| (d - 1.0).abs() < 0.01));
            // Vitesse de chaque case = mouvement de sa plaque (dérive et rotation)
            for i in (0..192 * 111).step_by(37) {
                let plate = p.plate(i).unwrap();
                assert!((p.velocity[i] - plate.velocity_at(p.pos[i], p.world_km)).length() < 1e-4);
            }
            // Une plaque n'a pas la même vitesse partout (elle tourne un peu sur elle-même)
            let spread = (0..p.plates.len())
                .map(|k| {
                    let speeds: Vec<f32> = (0..192 * 111).filter(|&i| usize::from(p.plate_of[i]) == k).map(|i| p.velocity[i].length()).collect();
                    let (lo, hi) = speeds.iter().fold((f32::MAX, 0f32), |(a, b), &v| (a.min(v), b.max(v)));
                    hi - lo
                })
                .fold(0f32, f32::max);
            assert!(spread > 0.3, "graine {seed} : {spread}");
            // Distance aux frontières : nulle sur elles, et les points chauds en sont loin
            for i in 0..192 * 111 {
                assert_eq!(p.boundary_km[i] == 0.0, p.boundary[i].is_some());
            }
            for h in &p.hotspots {
                let (c, r) = h.to_offset();
                assert!(p.boundary_km[(r * 192 + c) as usize] >= HOTSPOT_FROM_BOUNDARY as f32 * 40.0);
            }
        }
    }

    #[test]
    fn relief_issu_des_plaques() {
        for seed in [777, 2] {
            let p = planet(seed);
            let e = p.elevation(seed, 40.0);
            let n = e.len();
            let continental = |i: usize| p.plate(i).unwrap().kind == PlateKind::Continental;
            // Les terres (au-dessus de 0) sont surtout sur les plaques continentales
            let land: Vec<usize> = (0..n).filter(|&i| e[i] >= 0.0).collect();
            let on_cont = land.iter().filter(|&&i| continental(i)).count();
            assert!(on_cont * 100 >= land.len() * 85, "graine {seed} : {on_cont}/{}", land.len());
            let share = land.len() as f32 / n as f32;
            assert!((0.3..0.55).contains(&share), "graine {seed} : {share}");
            // Le plus haut des terres est près des frontières qui se rapprochent
            let mut by_height = land.clone();
            by_height.sort_by(|&a, &b| e[b].total_cmp(&e[a]));
            let top = &by_height[..by_height.len() / 50];
            let near = top
                .iter()
                .filter(|&&i| p.boundary_km[i] < 800.0 && p.edges[p.nearest_edge[i] as usize].kind.converging())
                .count();
            assert!(near * 100 >= top.len() * 60, "graine {seed} : {near}/{} sommets près d'une collision ou subduction", top.len());
            // Les fosses : les fonds océaniques au bord d'une subduction sont plus profonds
            let ocean: Vec<usize> = (0..n).filter(|&i| !continental(i) && e[i] < 0.0).collect();
            let mean = |cells: &[usize]| cells.iter().map(|&i| e[i]).sum::<f64>() / cells.len().max(1) as f64;
            let trench: Vec<usize> = ocean
                .iter()
                .copied()
                .filter(|&i| p.boundary_km[i] < 60.0 && p.edges[p.nearest_edge[i] as usize].under == p.plate_of[i])
                .collect();
            if !trench.is_empty() {
                assert!(mean(&trench) < mean(&ocean), "graine {seed} : fosses");
            }
        }
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
