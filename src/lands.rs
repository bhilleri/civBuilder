//! Terres nommées (logique pure, calculée à la génération) : continents, îles, îlots, archipels,
//! péninsules et isthmes, emboîtés (péninsule › continent ou île › archipel).
//!
//! 1. **Terres d'un seul tenant** (une case de lac compte comme de la terre : seule la mer
//!    sépare). Les plus grandes sont des continents, les autres des îles ou des îlots.
//! 2. **Isthmes** : passages de terre de 1 à 3 cases entre deux mers (le long d'un des trois axes
//!    du réseau), qui relient deux parties notables d'une même terre. Un très grand continent est
//!    coupé en deux continents à ses isthmes quand chaque côté a la taille d'un continent.
//! 3. **Archipels** : au moins trois îles à 3 cases au plus les unes des autres.
//! 4. **Péninsules**, une question d'échelle (l'« ouverture morphologique » des golfes, côté
//!    terre) : un cercle venu du cœur de la terre la balaie (rayons de 2 à 8 cases) ; ce qu'il
//!    n'atteint pas est derrière un col étroit. Une telle partie est une péninsule si la mer
//!    l'entoure sur l'essentiel de son pourtour et qu'elle s'avance assez par rapport à sa base.

use crate::hex::{DIRECTIONS, Wrap};
use crate::seas::{Grid, shape};
use crate::toponymy::Etymology;
use bevy::platform::collections::{HashMap, HashSet};
use std::collections::VecDeque;

/// Nature d'une terre nommée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LandKind {
    Continent,
    Island,
    Islet,
    Archipelago,
    Peninsula,
    Isthmus,
}

impl LandKind {
    pub fn label(self) -> &'static str {
        match self {
            LandKind::Continent => "continent",
            LandKind::Island => "île",
            LandKind::Islet => "îlot",
            LandKind::Archipelago => "archipel",
            LandKind::Peninsula => "péninsule",
            LandKind::Isthmus => "isthme",
        }
    }
}

/// Une terre nommée.
#[derive(Clone, Debug)]
pub struct LandZone {
    pub kind: LandKind,
    /// Ses cases, avec la distance de chacune au bord de la zone (en cases).
    pub cells: Vec<usize>,
    pub inner: Vec<u16>,
    /// Zone qui la contient (péninsule, isthme : continent ou île ; île : archipel).
    pub parent: Option<usize>,
    /// Demi-largeur d'ouest en est (unités du monde 2D).
    pub half_width: f32,
    pub name: String,
    pub feminine: bool,
    pub etymology: Option<Etymology>,
}

/// Terres nommées d'une carte.
#[derive(Clone, Debug, Default)]
pub struct Lands {
    pub zones: Vec<LandZone>,
    /// Continent, île ou îlot de chaque case de terre.
    pub mass_of: Vec<Option<u16>>,
    /// Archipel de chaque case.
    pub group_of: Vec<Option<u16>>,
    /// Péninsule de chaque case.
    pub part_of: Vec<Option<u16>>,
    /// Isthme de chaque case.
    pub isthmus_of: Vec<Option<u16>>,
}

impl Lands {
    /// Zones d'une case, de la plus précise à la plus vaste (isthme, péninsule, continent ou île,
    /// archipel).
    pub fn chain(&self, cell: usize) -> Vec<usize> {
        [&self.isthmus_of, &self.part_of, &self.mass_of, &self.group_of].iter().filter_map(|v| v.get(cell).copied().flatten()).map(|z| z as usize).collect()
    }
}

/// Une terre d'au moins tant de cases (et au moins `CONTINENT_SHARE` des terres) est un continent.
const CONTINENT_MIN_CELLS: usize = 200;
const CONTINENT_SHARE: f32 = 0.03;
/// Une terre de moins de tant de cases est un îlot (nommé seulement de près).
pub const ISLET_MAX_CELLS: usize = 3;
/// Isthme : passage de terre d'au plus tant de cases de large entre deux mers, qui relie deux
/// parties d'au moins `ISTHMUS_MIN_SIDE` cases ; plus long que `ISTHMUS_MAX_CELLS` cases, c'est
/// une bande de terre, pas un passage.
const ISTHMUS_MAX_WIDTH: usize = 3;
const ISTHMUS_MIN_SIDE: usize = 25;
const ISTHMUS_MAX_CELLS: usize = 30;
const MAX_ISTHMUSES: usize = 16;
/// Archipel : îles à au plus tant de cases les unes des autres, au moins `ARCHIPELAGO_MIN_ISLANDS`
/// et `ARCHIPELAGO_MIN_CELLS` cases en tout ; une île qui en fait plus de `ARCHIPELAGO_MAIN_SHARE`
/// est une île avec ses îlots, pas un archipel.
const ARCHIPELAGO_GAP: i32 = 3;
const ARCHIPELAGO_MIN_ISLANDS: usize = 3;
const ARCHIPELAGO_MIN_CELLS: usize = 6;
const ARCHIPELAGO_MAIN_SHARE: f32 = 0.75;
/// Échelles du cercle venu du cœur des terres (rayon en cases).
const SCALES: [u16; 6] = [2, 3, 4, 5, 6, 8];
/// Péninsule : taille (au plus une part de sa terre), part de mer sur son pourtour, avancée
/// (surface rapportée au carré de la base, longueur de la base au bout).
const PENINSULA_MIN_CELLS: usize = 15;
const PENINSULA_MAX_SHARE: f32 = 0.35;
const PENINSULA_COAST: f32 = 0.6;
const PENINSULA_INDENT: f32 = 0.35;
const PENINSULA_DEPTH: u16 = 3;
const DEPTH_PER_BASE: f32 = 0.5;
/// Terre assez grande pour avoir des péninsules.
const PENINSULA_LAND_MIN: usize = 60;
const MAX_PENINSULAS: usize = 40;

/// Terres nommées. `land` : case de terre (ou de lac) ; tout le reste est la mer.
pub fn compute(width: i32, height: i32, wrap: Wrap, land: &[bool]) -> Lands {
    let grid = Grid { width, height, wrap };
    let n = land.len();
    let mut lands = Lands { zones: Vec::new(), mass_of: vec![None; n], group_of: vec![None; n], part_of: vec![None; n], isthmus_of: vec![None; n] };
    let total_land = land.iter().filter(|&&l| l).count();
    if total_land == 0 {
        return lands;
    }
    // 1) Terres d'un seul tenant
    let (_, masses) = components(&grid, |c| land[c]);
    // Distance à la mer de chaque case de terre (le bord de la carte, un pôle, n'est pas une côte)
    let mut dist = vec![u16::MAX; n];
    let mut queue = VecDeque::new();
    for c in 0..n {
        if !land[c] {
            dist[c] = 0;
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        for j in grid.neighbors(c) {
            if dist[j] == u16::MAX {
                dist[j] = dist[c] + 1;
                queue.push_back(j);
            }
        }
    }
    for d in dist.iter_mut() {
        if *d == u16::MAX {
            *d = 100;
        }
    }

    // 2) Isthmes : cases étroites (de la mer des deux côtés à moins de 3 cases de terre) ; ôtées,
    //    elles laissent les parties qu'elles reliaient
    let narrow: Vec<bool> = (0..n).map(|c| land[c] && is_narrow(&grid, land, c)).collect();
    let (part_id, parts) = components(&grid, |c| land[c] && !narrow[c]);
    let (_, clusters) = components(&grid, |c| narrow[c]);
    let mut isthmuses: Vec<(usize, usize)> = Vec::new();
    for (k, cells) in clusters.iter().enumerate() {
        if cells.len() > ISTHMUS_MAX_CELLS {
            continue;
        }
        let mut sides: Vec<usize> = cells.iter().flat_map(|&c| grid.neighbors(c)).filter(|&j| land[j] && !narrow[j]).map(|j| part_id[j]).collect();
        sides.sort_unstable();
        sides.dedup();
        let mut sizes: Vec<usize> = sides.iter().map(|&p| parts[p].len()).filter(|&s| s >= ISTHMUS_MIN_SIDE).collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        if sizes.len() >= 2 {
            isthmuses.push((sizes[1], k));
        }
    }
    isthmuses.sort_by_key(|&(w, k)| (std::cmp::Reverse(w), clusters[k][0]));
    isthmuses.truncate(MAX_ISTHMUSES);

    // 3) Continents, coupés à leurs passages étroits quand chaque côté a la taille d'un continent ;
    //    îles, îlots
    let continent_min = CONTINENT_MIN_CELLS.max((total_land as f32 * CONTINENT_SHARE) as usize);
    let mut masses_sorted: Vec<usize> = (0..masses.len()).collect();
    masses_sorted.sort_by_key(|&m| (std::cmp::Reverse(masses[m].len()), masses[m][0]));
    // (zone de terre de chaque case avant numérotation : une liste de cases par zone)
    let mut continents: Vec<Vec<usize>> = Vec::new();
    let mut islands: Vec<Vec<usize>> = Vec::new();
    let mut islets: Vec<Vec<usize>> = Vec::new();
    for &m in &masses_sorted {
        let cells = &masses[m];
        if cells.len() < continent_min {
            if cells.len() <= ISLET_MAX_CELLS { islets.push(cells.clone()) } else { islands.push(cells.clone()) }
            continue;
        }
        let mut big: Vec<usize> = cells.iter().filter(|&&c| !narrow[c]).map(|&c| part_id[c]).collect();
        big.sort_unstable();
        big.dedup();
        big.retain(|&p| parts[p].len() >= continent_min);
        big.sort_by_key(|&p| (std::cmp::Reverse(parts[p].len()), parts[p][0]));
        if big.len() < 2 {
            continents.push(cells.clone());
            continue;
        }
        // Chaque case va au continent le plus proche (par la terre)
        let mut owner: HashMap<usize, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        for (k, &p) in big.iter().enumerate() {
            for &c in &parts[p] {
                owner.insert(c, k);
                queue.push_back(c);
            }
        }
        while let Some(c) = queue.pop_front() {
            let k = owner[&c];
            for j in grid.neighbors(c) {
                if land[j] && !owner.contains_key(&j) {
                    owner.insert(j, k);
                    queue.push_back(j);
                }
            }
        }
        let mut split: Vec<Vec<usize>> = vec![Vec::new(); big.len()];
        for &c in cells {
            split[owner[&c]].push(c);
        }
        continents.extend(split);
    }
    for cells in continents.iter().map(|c| (LandKind::Continent, c)).chain(islands.iter().map(|c| (LandKind::Island, c))).chain(islets.iter().map(|c| (LandKind::Islet, c))) {
        let (kind, cells) = cells;
        let id = lands.zones.len() as u16;
        for &c in cells {
            lands.mass_of[c] = Some(id);
        }
        let mut cells = cells.clone();
        cells.sort_unstable();
        lands.zones.push(zone(&grid, kind, cells, None, &lands.mass_of));
    }

    // 4) Archipels : îles et îlots proches les uns des autres
    let small: Vec<usize> = (0..lands.zones.len()).filter(|&z| lands.zones[z].kind != LandKind::Continent).collect();
    let mut union: Vec<usize> = (0..lands.zones.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for &z in &small {
        for &c in &lands.zones[z].cells {
            let h = grid.hex(c);
            for near in wrap.range(h, ARCHIPELAGO_GAP) {
                let Some(j) = grid.index(near) else { continue };
                let Some(o) = lands.mass_of[j].map(usize::from) else { continue };
                if o != z && lands.zones[o].kind != LandKind::Continent {
                    let (a, b) = (find(&mut union, z), find(&mut union, o));
                    if a != b {
                        union[a.max(b)] = a.min(b);
                    }
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for &z in &small {
        groups.entry(find(&mut union, z)).or_default().push(z);
    }
    let size = |z: &usize| lands.zones[*z].cells.len();
    let mut groups: Vec<Vec<usize>> = groups
        .into_values()
        .filter(|g| {
            let total: usize = g.iter().map(size).sum();
            g.len() >= ARCHIPELAGO_MIN_ISLANDS && total >= ARCHIPELAGO_MIN_CELLS && g.iter().map(size).max().unwrap_or(0) as f32 <= ARCHIPELAGO_MAIN_SHARE * total as f32
        })
        .collect();
    groups.sort_by_key(|g| g[0]);
    for members in groups {
        let id = lands.zones.len();
        let mut cells: Vec<usize> = members.iter().flat_map(|&z| lands.zones[z].cells.iter().copied()).collect();
        cells.sort_unstable();
        for &c in &cells {
            lands.group_of[c] = Some(id as u16);
        }
        for &z in &members {
            lands.zones[z].parent = Some(id);
        }
        lands.zones.push(zone(&grid, LandKind::Archipelago, cells, None, &lands.group_of));
    }

    // 5) Péninsules, les plus grandes échelles d'abord
    let mut found = peninsulas(&grid, land, &dist, &lands.mass_of, &lands.zones);
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.len().cmp(&a.1.len())).then(a.1[0].cmp(&b.1[0])));
    let mut taken: Vec<Option<usize>> = vec![None; n];
    let mut kept: Vec<Vec<usize>> = Vec::new();
    for (_, cells) in found {
        if kept.len() >= MAX_PENINSULAS {
            break;
        }
        if cells.iter().filter(|&&c| taken[c].is_some()).count() * 2 > cells.len() {
            continue;
        }
        let cells = largest_piece(&grid, cells.into_iter().filter(|&c| taken[c].is_none()).collect());
        if cells.len() < PENINSULA_MIN_CELLS {
            continue;
        }
        for &c in &cells {
            taken[c] = Some(kept.len());
        }
        kept.push(cells);
    }
    let first = lands.zones.len();
    for (i, cells) in kept.iter().enumerate() {
        for &c in cells {
            lands.part_of[c] = Some((first + i) as u16);
        }
    }
    for cells in kept {
        let parent = lands.mass_of[cells[0]].map(usize::from);
        lands.zones.push(zone(&grid, LandKind::Peninsula, cells, parent, &lands.part_of));
    }

    // 6) Isthmes nommés, dans le continent ou l'île qui en a le plus de cases
    let first = lands.zones.len();
    for (i, &(_, k)) in isthmuses.iter().enumerate() {
        for &c in &clusters[k] {
            lands.isthmus_of[c] = Some((first + i) as u16);
        }
    }
    for (_, k) in isthmuses {
        let cells = clusters[k].clone();
        let mut count: HashMap<u16, usize> = HashMap::new();
        for &c in &cells {
            if let Some(m) = lands.mass_of[c] {
                *count.entry(m).or_default() += 1;
            }
        }
        let parent = count.into_iter().max_by_key(|&(m, k)| (k, std::cmp::Reverse(m))).map(|(m, _)| usize::from(m));
        lands.zones.push(zone(&grid, LandKind::Isthmus, cells, parent, &lands.isthmus_of));
    }
    lands
}

/// Morceaux d'un seul tenant des cases retenues : numéro de chaque case (`usize::MAX` hors des
/// cases retenues) et cases de chaque morceau (dans l'ordre des cases).
fn components(grid: &Grid, keep: impl Fn(usize) -> bool) -> (Vec<usize>, Vec<Vec<usize>>) {
    let n = (grid.width * grid.height) as usize;
    let mut id = vec![usize::MAX; n];
    let mut all: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if id[start] != usize::MAX || !keep(start) {
            continue;
        }
        let k = all.len();
        id[start] = k;
        let mut cells = vec![start];
        let mut i = 0;
        while i < cells.len() {
            let c = cells[i];
            i += 1;
            for j in grid.neighbors(c) {
                if id[j] == usize::MAX && keep(j) {
                    id[j] = k;
                    cells.push(j);
                }
            }
        }
        cells.sort_unstable();
        all.push(cells);
    }
    (id, all)
}

/// Case de terre étroite : le long d'un des trois axes, de la mer des deux côtés à au plus
/// `ISTHMUS_MAX_WIDTH` cases de terre en tout (la case comprise).
fn is_narrow(grid: &Grid, land: &[bool], c: usize) -> bool {
    let origin = grid.hex(c);
    let walk = |d: usize| -> Option<usize> {
        let mut at = origin;
        for steps in 1..=ISTHMUS_MAX_WIDTH {
            at = at + DIRECTIONS[d];
            let j = grid.index(at)?;
            if !land[j] {
                return Some(steps);
            }
        }
        None
    };
    (0..3).any(|a| matches!((walk(a), walk(a + 3)), (Some(s1), Some(s2)) if s1 + s2 - 1 <= ISTHMUS_MAX_WIDTH))
}

/// Le plus grand morceau d'un seul tenant d'un ensemble de cases.
fn largest_piece(grid: &Grid, cells: Vec<usize>) -> Vec<usize> {
    let members: HashSet<usize> = cells.iter().copied().collect();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut best: Vec<usize> = Vec::new();
    for &start in &cells {
        if !seen.insert(start) {
            continue;
        }
        let mut part = vec![start];
        let mut k = 0;
        while k < part.len() {
            let c = part[k];
            k += 1;
            for j in grid.neighbors(c) {
                if members.contains(&j) && seen.insert(j) {
                    part.push(j);
                }
            }
        }
        if part.len() > best.len() {
            best = part;
        }
    }
    best.sort_unstable();
    best
}

/// Péninsules possibles, à chaque échelle `r` de `SCALES` (avec leur échelle) : un cercle de rayon
/// r − 1 venu du cœur de chaque continent ou île (le plus grand groupe de ses cases à au moins r
/// de la mer) ne passe que par des cols d'au moins ~2r cases ; ce qu'il n'atteint pas, s'il a de
/// l'épaisseur, est derrière un col. Gardé si la mer l'entoure sur l'essentiel de son pourtour,
/// s'il s'avance assez par rapport à sa base (surface rapportée au carré de la base, longueur) et
/// s'il n'est qu'une partie de sa terre.
fn peninsulas(grid: &Grid, land: &[bool], dist: &[u16], mass_of: &[Option<u16>], zones: &[LandZone]) -> Vec<(u16, Vec<usize>)> {
    let n = land.len();
    let big = |c: usize| mass_of[c].is_some_and(|m| zones[usize::from(m)].cells.len() >= PENINSULA_LAND_MIN);
    let mut out = Vec::new();
    for &r in &SCALES {
        // Cœur de chaque terre : le plus grand groupe de cases à au moins r de la mer
        let (group, groups) = components(grid, |c| big(c) && dist[c] >= r);
        let mut core_of: HashMap<u16, usize> = HashMap::new();
        for (k, cells) in groups.iter().enumerate() {
            let m = mass_of[cells[0]].unwrap();
            let best = core_of.entry(m).or_insert(k);
            if cells.len() > groups[*best].len() {
                *best = k;
            }
        }
        let cores: HashSet<usize> = core_of.values().copied().collect();
        // Cases balayées par le cercle
        let mut steps = vec![u16::MAX; n];
        let mut queue = VecDeque::new();
        for c in 0..n {
            if group[c] != usize::MAX && cores.contains(&group[c]) {
                steps[c] = 0;
                queue.push_back(c);
            }
        }
        while let Some(c) = queue.pop_front() {
            if steps[c] + 1 >= r {
                continue;
            }
            for j in grid.neighbors(c) {
                if land[j] && steps[j] == u16::MAX && mass_of[j] == mass_of[c] {
                    steps[j] = steps[c] + 1;
                    queue.push_back(j);
                }
            }
        }
        // Éloignement des cases non balayées à la terre balayée : seules comptent les parties
        // épaisses (pas une frange d'une case le long d'une côte découpée)
        let mut shade = vec![u16::MAX; n];
        for c in 0..n {
            if steps[c] != u16::MAX {
                shade[c] = 0;
                queue.push_back(c);
            }
        }
        while let Some(c) = queue.pop_front() {
            for j in grid.neighbors(c) {
                if land[j] && shade[j] == u16::MAX && mass_of[j] == mass_of[c] {
                    shade[j] = shade[c] + 1;
                    queue.push_back(j);
                }
            }
        }
        let uncovered = |j: usize| big(j) && steps[j] == u16::MAX;
        let mut seen = vec![false; n];
        for start in 0..n {
            if !uncovered(start) || shade[start] < 2 || seen[start] || !mass_of[start].is_some_and(|m| core_of.contains_key(&m)) {
                continue;
            }
            let mass = mass_of[start];
            let mut pocket = vec![start];
            seen[start] = true;
            let mut k = 0;
            while k < pocket.len() {
                let c = pocket[k];
                k += 1;
                for j in grid.neighbors(c) {
                    if uncovered(j) && shade[j] >= 2 && !seen[j] && mass_of[j] == mass {
                        seen[j] = true;
                        pocket.push(j);
                    }
                }
            }
            // Avec sa frange (jusqu'à la terre balayée)
            let mut members: HashSet<usize> = pocket.iter().copied().collect();
            let mut front = pocket.clone();
            for _ in 0..2 {
                let mut next = Vec::new();
                for &c in &front {
                    for j in grid.neighbors(c) {
                        if uncovered(j) && mass_of[j] == mass && members.insert(j) {
                            next.push(j);
                        }
                    }
                }
                pocket.extend(&next);
                front = next;
            }
            // (le bord de la carte n'est pas une côte : rien contre un pôle)
            let polar = |c: usize| {
                let row = c as i32 / grid.width;
                row <= 1 || row >= grid.height - 2
            };
            let size = zones[usize::from(mass.unwrap())].cells.len();
            if pocket.len() < PENINSULA_MIN_CELLS || pocket.len() as f32 > PENINSULA_MAX_SHARE * size as f32 || pocket.iter().any(|&c| polar(c)) {
                continue;
            }
            // Base : cases qui touchent la terre balayée ; pourtour : mer / terre ; longueur :
            // éloignement maximal à la base
            let (mut sea_edges, mut land_edges) = (0usize, 0usize);
            let mut depth: HashMap<usize, u16> = HashMap::new();
            let mut front = VecDeque::new();
            for &c in &pocket {
                let mut base = false;
                for j in grid.neighbors(c) {
                    if !land[j] {
                        sea_edges += 1;
                    } else if !members.contains(&j) {
                        land_edges += 1;
                        base = true;
                    }
                }
                if base {
                    depth.insert(c, 1);
                    front.push_back(c);
                }
            }
            let base = front.len();
            if base == 0 {
                continue;
            }
            while let Some(c) = front.pop_front() {
                let d = depth[&c];
                for j in grid.neighbors(c) {
                    if members.contains(&j) && !depth.contains_key(&j) {
                        depth.insert(j, d + 1);
                        front.push_back(j);
                    }
                }
            }
            let deepest = depth.values().copied().max().unwrap_or(0);
            let quality = pocket.len() as f32 / (base as f32).powi(2);
            let coast = sea_edges as f32 / (sea_edges + land_edges).max(1) as f32;
            if coast < PENINSULA_COAST || quality < PENINSULA_INDENT || deepest < PENINSULA_DEPTH || (deepest as f32) < DEPTH_PER_BASE * base as f32 {
                continue;
            }
            pocket.sort_unstable();
            out.push((r, pocket));
        }
    }
    out
}

fn zone(grid: &Grid, kind: LandKind, cells: Vec<usize>, parent: Option<usize>, owner: &[Option<u16>]) -> LandZone {
    let (inner, half_width) = shape(grid, &cells, owner);
    LandZone { kind, cells, inner, parent, half_width, name: String::new(), feminine: false, etymology: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Carte faite de lignes de caractères : '#' terre, '.' mer.
    fn compute_rows(rows: &[String]) -> (i32, Lands) {
        let w = rows[0].len() as i32;
        let land: Vec<bool> = rows.iter().flat_map(|r| r.chars().map(|c| c == '#')).collect();
        (w, compute(w, rows.len() as i32, Wrap::NONE, &land))
    }

    fn rows(w: i32, h: i32, land: impl Fn(i32, i32) -> bool) -> Vec<String> {
        (0..h).map(|y| (0..w).map(|x| if land(x, y) { '#' } else { '.' }).collect()).collect()
    }

    #[test]
    fn deux_continents_relies_par_un_isthme() {
        // Deux blocs de 30 × 20 cases reliés par un passage de 2 cases de large sur 8 de long
        let (w, h) = (100, 30);
        let map = rows(w, h, |x, y| ((5..35).contains(&x) && (5..25).contains(&y)) || ((43..73).contains(&x) && (5..25).contains(&y)) || ((35..43).contains(&x) && (14..16).contains(&y)));
        let (w, lands) = compute_rows(&map);
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let (a, b) = (lands.mass_of[at(15, 15)].unwrap(), lands.mass_of[at(60, 15)].unwrap());
        assert_ne!(a, b, "l'isthme sépare deux continents");
        assert_eq!(lands.zones[a as usize].kind, LandKind::Continent);
        assert_eq!(lands.zones[b as usize].kind, LandKind::Continent);
        let isthmus = lands.isthmus_of[at(39, 14)].expect("un isthme dans le passage");
        assert_eq!(lands.zones[isthmus as usize].kind, LandKind::Isthmus);
        assert!(lands.isthmus_of[at(15, 15)].is_none());
    }

    #[test]
    fn peninsule_et_ile() {
        // Un continent de 40 × 25 cases, avec une péninsule de 5 cases de large qui descend de 12
        // cases au sud ; une île de 4 × 4 cases au large, un îlot d'une case
        let (w, h) = (90, 60);
        let map = rows(w, h, |x, y| ((10..50).contains(&x) && (5..30).contains(&y)) || ((20..25).contains(&x) && (30..42).contains(&y)) || ((70..74).contains(&x) && (40..44).contains(&y)) || (x == 80 && y == 10));
        let (w, lands) = compute_rows(&map);
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let continent = lands.mass_of[at(30, 15)].unwrap();
        let peninsula = lands.part_of[at(22, 38)].expect("une péninsule au sud");
        assert_eq!(lands.zones[peninsula as usize].kind, LandKind::Peninsula);
        assert_eq!(lands.zones[peninsula as usize].parent, Some(continent as usize));
        assert!(lands.part_of[at(30, 15)].is_none(), "le cœur du continent n'est pas une péninsule");
        let island = lands.mass_of[at(71, 41)].unwrap();
        assert_eq!(lands.zones[island as usize].kind, LandKind::Island);
        let islet = lands.mass_of[at(80, 10)].unwrap();
        assert_eq!(lands.zones[islet as usize].kind, LandKind::Islet);
    }

    #[test]
    fn archipel() {
        // Cinq îles de 2 × 2 cases à 2 cases d'eau les unes des autres, une île isolée plus loin
        let (w, h) = (80, 30);
        let map = rows(w, h, |x, y| ((10..30).step_by(4).any(|x0| (x0..x0 + 2).contains(&x)) && (10..12).contains(&y)) || ((60..63).contains(&x) && (20..23).contains(&y)));
        let (w, lands) = compute_rows(&map);
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let group = lands.group_of[at(10, 10)].expect("un archipel");
        assert_eq!(lands.zones[group as usize].kind, LandKind::Archipelago);
        assert_eq!(lands.group_of[at(26, 11)], Some(group));
        assert!(lands.group_of[at(61, 21)].is_none(), "l'île isolée n'en fait pas partie");
        let island = lands.mass_of[at(14, 10)].unwrap();
        assert_eq!(lands.zones[island as usize].parent, Some(group as usize));
    }

    /// Bilan des terres nommées : `cargo test bilan_terres -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bilan_terres() {
        for seed in [777, 4242, 2, 5, 31] {
            let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let l = m.lands();
            let count = |k: LandKind| l.zones.iter().filter(|z| z.kind == k).count();
            println!(
                "graine {seed} : continents {}, îles {}, îlots {}, archipels {}, péninsules {}, isthmes {}",
                count(LandKind::Continent),
                count(LandKind::Island),
                count(LandKind::Islet),
                count(LandKind::Archipelago),
                count(LandKind::Peninsula),
                count(LandKind::Isthmus)
            );
            for z in l.zones.iter().filter(|z| z.kind != LandKind::Islet && (z.kind != LandKind::Island || z.cells.len() >= 20)) {
                let parent = z.parent.map_or(String::new(), |p| format!(" (dans {})", l.zones[p].name));
                println!("  {:<10} {:<30} {:>5} cases{parent}", z.kind.label(), z.name, z.cells.len());
            }
        }
    }
}
