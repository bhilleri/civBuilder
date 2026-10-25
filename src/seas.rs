//! Étendues marines nommées (logique pure, calculée à la génération) : océans, mers, golfes,
//! baies et détroits.
//!
//! 1. **Distance à la côte** de chaque case de mer (en cases).
//! 2. **Océans et mers** : la mer est découpée là où elle se resserre. Les cases sont prises de
//!    la plus éloignée de la côte à la plus proche (le « relief » de la distance à la côte) : deux
//!    étendues qui se rejoignent restent séparées si le passage qui les relie est étroit par
//!    rapport à leur largeur (un passage moins large que ~55 % de la moins large des deux), sinon
//!    elles n'en font qu'une ; une petite étendue se fond toujours dans sa voisine. Les grandes
//!    étendues sont des océans ; une plus petite, fermée ou presque (bordée de terre sur au moins
//!    60 % de son pourtour), est une mer (mer polaire si elle touche un pôle) ; les autres font
//!    partie de leur voisine (une mer ouverte est une partie de l'océan).
//! 3. **Golfes et baies** : cases de mer « enveloppées » par la terre (12 rayons de 7 cases : au
//!    moins 8 touchent une terre). Un amas de telles cases d'au moins 25 cases est un golfe, de 4
//!    à 24 cases une baie. Il s'étend jusqu'à la ligne entre les deux caps de son entrée.
//! 4. **Détroits** : passages de 1 à 4 cases de large entre deux terres distinctes (une île et un
//!    continent, deux îles), chacune d'au moins quelques cases.
//! 5. **Mers bordières** : parties abritées d'un océan le long des côtes (entre une côte et des
//!    îles, devant une côte concave), hors golfes et détroits, de taille modeste (60 à 600 cases) ;
//!    l'océan n'est pas tout découpé, seules ses parties remarquables sont nommées.
//!
//! Chaque étendue garde la distance de chaque case à son bord (où poser son nom) et sa
//! demi-largeur.

use crate::hex::{DIRECTIONS, Hex, Wrap};
use crate::toponymy::Etymology;
use std::collections::{HashMap, HashSet, VecDeque};

/// Nature d'une étendue marine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeaKind {
    Ocean,
    Sea,
    Gulf,
    Bay,
    Strait,
}

impl SeaKind {
    pub fn label(self) -> &'static str {
        match self {
            SeaKind::Ocean => "océan",
            SeaKind::Sea => "mer",
            SeaKind::Gulf => "golfe",
            SeaKind::Bay => "baie",
            SeaKind::Strait => "détroit",
        }
    }
}

/// Une étendue marine nommée.
#[derive(Clone, Debug)]
pub struct SeaZone {
    pub kind: SeaKind,
    /// Ses cases, avec la distance de chacune au bord de l'étendue (en cases).
    pub cells: Vec<usize>,
    pub inner: Vec<u16>,
    /// Océan ou mer qui la contient (golfe, baie, détroit).
    pub parent: Option<usize>,
    /// Demi-largeur d'ouest en est (unités du monde 2D).
    pub half_width: f32,
    pub name: String,
    pub feminine: bool,
    pub etymology: Option<Etymology>,
}

/// Étendues marines d'une carte.
#[derive(Clone, Debug, Default)]
pub struct Seas {
    pub zones: Vec<SeaZone>,
    /// Océan ou mer de chaque case de mer.
    pub basin_of: Vec<Option<u16>>,
    /// Golfe ou baie de chaque case.
    pub inlet_of: Vec<Option<u16>>,
    /// Détroit de chaque case.
    pub strait_of: Vec<Option<u16>>,
    /// Mer bordière (partie abritée d'un océan, le long d'une côte) de chaque case.
    pub coastal_of: Vec<Option<u16>>,
}

impl Seas {
    /// Étendues d'une case, de la plus précise à la plus vaste (détroit, golfe ou baie, mer ou
    /// océan).
    pub fn chain(&self, cell: usize) -> Vec<usize> {
        [&self.strait_of, &self.inlet_of, &self.coastal_of, &self.basin_of].iter().filter_map(|v| v.get(cell).copied().flatten()).map(|z| z as usize).collect()
    }
}

/// Une étendue d'au moins autant de cases (et au moins `OCEAN_SHARE` de la mer) est un océan.
const OCEAN_MIN_CELLS: usize = 700;
const OCEAN_SHARE: f32 = 0.08;
/// Plus petite étendue gardée à part pendant le découpage.
const MIN_BASIN_CELLS: usize = 40;
/// Passage étroit : moins large que cette part de la moins large des deux étendues.
const NARROW_RATIO: f32 = 0.65;
/// Une mer est bordée de terre sur au moins cette part de son pourtour.
const SEA_LAND_SHARE: f32 = 0.6;
/// Rayons pour l'enveloppement : longueur et nombre de rayons qui doivent toucher une terre.
const RAY_LENGTH: i32 = 8;
const ENCLOSED_RAYS: usize = 8;
/// Entrée d'un golfe : largeur maximale entre ses deux caps (cases).
const HEADLAND_WIDTH: i32 = 24;
pub const GULF_MIN_CELLS: usize = 25;
pub const BAY_MIN_CELLS: usize = 4;
/// Un golfe n'occupe pas plus de cette part de sa mer (sinon c'est la mer elle-même).
const GULF_MAX_SHARE: f32 = 0.4;
/// Largeur maximale d'un détroit (cases) et taille minimale des terres qu'il sépare.
const STRAIT_MAX_WIDTH: usize = 4;
const STRAIT_MIN_LAND: usize = 4;
const MAX_STRAITS: usize = 30;
/// Mers bordières : cases à au plus `COASTAL_REACH` cases de la côte, abritées (au moins
/// `SHELTERED_RAYS` rayons sur 12 touchent une terre à moins de `SHELTER_RAY` cases), d'une taille
/// entre `COASTAL_MIN_CELLS` et `COASTAL_MAX_CELLS`.
const COASTAL_REACH: u16 = 10;
const SHELTER_RAY: i32 = 16;
const SHELTERED_RAYS: u8 = 6;
const COASTAL_MIN_CELLS: usize = 40;
const COASTAL_MAX_CELLS: usize = 900;
/// Une mer bordière n'est pas qu'un golfe : au moins cette part de ses cases hors golfes, baies et
/// détroits.
const COASTAL_OPEN_SHARE: f32 = 0.5;

/// Grille de la carte : voisines et position des cases.
struct Grid {
    width: i32,
    height: i32,
    wrap: Wrap,
}

impl Grid {
    fn hex(&self, c: usize) -> Hex {
        Hex::from_offset(c as i32 % self.width, c as i32 / self.width)
    }

    fn index(&self, h: Hex) -> Option<usize> {
        let (col, row) = self.wrap.hex(h).to_offset();
        (col >= 0 && col < self.width && row >= 0 && row < self.height).then(|| (row * self.width + col) as usize)
    }

    fn neighbors(&self, c: usize) -> impl Iterator<Item = usize> + '_ {
        let h = self.hex(c);
        DIRECTIONS.iter().filter_map(move |&d| self.index(h + d))
    }
}

/// Étendues marines. `water` : case de mer ouverte (ni lac, ni mer intérieure, déjà nommés).
pub fn compute(width: i32, height: i32, wrap: Wrap, water: &[bool]) -> Seas {
    let grid = Grid { width, height, wrap };
    let n = water.len();
    let mut seas = Seas { zones: Vec::new(), basin_of: vec![None; n], inlet_of: vec![None; n], strait_of: vec![None; n], coastal_of: vec![None; n] };
    let total_water = water.iter().filter(|&&w| w).count();
    if total_water == 0 {
        return seas;
    }
    // 1) Distance à la côte
    let mut dist = vec![u16::MAX; n];
    let mut queue = VecDeque::new();
    for c in 0..n {
        if !water[c] {
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
    // (une mer sans aucune terre : distance arbitraire)
    for d in dist.iter_mut() {
        if *d == u16::MAX {
            *d = 100;
        }
    }

    // 2) Découpage : des cases les plus éloignées de la côte aux plus proches
    let mut order: Vec<usize> = (0..n).filter(|&c| water[c]).collect();
    order.sort_by_key(|&c| (std::cmp::Reverse(dist[c]), c));
    let mut parent: Vec<usize> = (0..n).collect();
    let mut peak = vec![0u16; n];
    let mut size = vec![0usize; n];
    let mut added = vec![false; n];
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for &c in &order {
        added[c] = true;
        peak[c] = dist[c];
        size[c] = 1;
        let mut roots: Vec<usize> = grid.neighbors(c).filter(|&j| added[j] && j != c).map(|j| find(&mut parent, j)).collect();
        roots.sort_unstable();
        roots.dedup();
        // La case rejoint l'étendue la plus « haute » ; les autres fusionnent si le passage est large
        roots.sort_by_key(|&r| (std::cmp::Reverse(peak[r]), std::cmp::Reverse(size[r]), r));
        let Some(&main) = roots.first() else { continue };
        parent[c] = main;
        size[main] += 1;
        for &r in &roots[1..] {
            let (a, b) = (find(&mut parent, main), find(&mut parent, r));
            if a == b {
                continue;
            }
            let low = peak[a].min(peak[b]) as f32;
            let small = size[a].min(size[b]) < MIN_BASIN_CELLS;
            if small || f32::from(dist[c]) >= NARROW_RATIO * low {
                let (keep, gone) = if (peak[a], size[a]) >= (peak[b], size[b]) { (a, b) } else { (b, a) };
                parent[gone] = keep;
                size[keep] += size[gone];
                peak[keep] = peak[keep].max(peak[gone]);
            }
        }
    }
    let mut region: Vec<Option<usize>> = vec![None; n];
    for &c in &order {
        region[c] = Some(find(&mut parent, c));
    }

    // 3) Océans, mers ; les petites étendues ouvertes rejoignent leur voisine
    let ocean_min = OCEAN_MIN_CELLS.max((total_water as f32 * OCEAN_SHARE) as usize);
    loop {
        let mut cells_of: HashMap<usize, Vec<usize>> = HashMap::new();
        for &c in &order {
            cells_of.entry(region[c].unwrap()).or_default().push(c);
        }
        // Pourtour de chaque étendue : arêtes vers la terre, vers chaque voisine
        let mut merged = false;
        let mut regions: Vec<usize> = cells_of.keys().copied().collect();
        regions.sort_by_key(|r| (cells_of[r].len(), *r));
        for r in regions {
            let cells = &cells_of[&r];
            if cells.len() >= ocean_min {
                continue;
            }
            let mut land_edges = 0usize;
            let mut shared: HashMap<usize, usize> = HashMap::new();
            let mut polar = false;
            for &c in cells {
                let row = c as i32 / width;
                polar |= row == 0 || row == height - 1;
                for j in grid.neighbors(c) {
                    match region[j] {
                        None => land_edges += 1,
                        Some(o) if o != r => *shared.entry(o).or_default() += 1,
                        _ => {}
                    }
                }
            }
            let open: usize = shared.values().sum();
            let enclosed = land_edges as f32 / (land_edges + open).max(1) as f32 >= SEA_LAND_SHARE;
            let is_sea = cells.len() >= MIN_BASIN_CELLS && (enclosed || polar);
            if is_sea || shared.is_empty() {
                continue;
            }
            let (&into, _) = shared.iter().max_by_key(|(o, k)| (**k, std::cmp::Reverse(**o))).unwrap();
            for &c in cells {
                region[c] = Some(into);
            }
            merged = true;
            break;
        }
        if !merged {
            break;
        }
    }
    let mut cells_of: HashMap<usize, Vec<usize>> = HashMap::new();
    for &c in &order {
        cells_of.entry(region[c].unwrap()).or_default().push(c);
    }
    let mut basins: Vec<Vec<usize>> = cells_of.into_values().collect();
    basins.sort_by_key(|cells| (std::cmp::Reverse(cells.len()), cells.iter().min().copied()));
    let any_ocean = basins.first().is_some_and(|b| b.len() >= ocean_min);
    for (mut cells, k) in basins.into_iter().zip(0..) {
        cells.sort_unstable();
        let kind = if cells.len() >= ocean_min || (k == 0 && !any_ocean) { SeaKind::Ocean } else { SeaKind::Sea };
        let id = seas.zones.len();
        for &c in &cells {
            seas.basin_of[c] = Some(id as u16);
        }
        seas.zones.push(zone(&grid, kind, cells, None, &seas.basin_of));
    }

    // Terres d'un seul tenant (continents, îles)
    let mut land_id = vec![usize::MAX; n];
    let mut land_size: Vec<usize> = Vec::new();
    for start in 0..n {
        if water[start] || land_id[start] != usize::MAX {
            continue;
        }
        let id = land_size.len();
        let mut stack = vec![start];
        land_id[start] = id;
        let mut count = 0;
        while let Some(c) = stack.pop() {
            count += 1;
            for j in grid.neighbors(c) {
                if !water[j] && land_id[j] == usize::MAX {
                    land_id[j] = id;
                    stack.push(j);
                }
            }
        }
        land_size.push(count);
    }
    // 4) Golfes et baies : un cœur bien enveloppé par une terre, étendu aux cases voisines un
    //    peu moins enveloppées par cette terre, et jusqu'à la ligne entre les deux caps de
    //    l'entrée (cases qui ont cette terre des deux côtés)
    let rays: Vec<(usize, usize)> = (0..n).map(|c| if water[c] { enclosure(&grid, water, &land_id, c) } else { (0, usize::MAX) }).collect();
    let mut seen = vec![false; n];
    for start in 0..n {
        let (count, land) = rays[start];
        if count < ENCLOSED_RAYS || seen[start] {
            continue;
        }
        let basin = seas.basin_of[start];
        let inside = |j: usize| (rays[j].1 == land && rays[j].0 >= ENCLOSED_RAYS - 2) || between_headlands(&grid, water, &land_id, j, land);
        let mut cells = vec![start];
        seen[start] = true;
        let mut k = 0;
        while k < cells.len() {
            let c = cells[k];
            k += 1;
            for j in grid.neighbors(c) {
                if water[j] && !seen[j] && seas.basin_of[j] == basin && inside(j) {
                    seen[j] = true;
                    cells.push(j);
                }
            }
        }
        let Some(b) = basin.map(|b| b as usize) else { continue };
        let kind = if cells.len() >= GULF_MIN_CELLS {
            SeaKind::Gulf
        } else if cells.len() >= BAY_MIN_CELLS {
            SeaKind::Bay
        } else {
            continue;
        };
        if cells.len() as f32 > GULF_MAX_SHARE * seas.zones[b].cells.len() as f32 {
            continue;
        }
        cells.sort_unstable();
        let id = seas.zones.len();
        for &c in &cells {
            seas.inlet_of[c] = Some(id as u16);
        }
        seas.zones.push(zone(&grid, kind, cells, Some(b), &seas.inlet_of));
    }

    // 5) Détroits : passages étroits entre deux terres distinctes
    // Terres de part et d'autre d'une case étroite (les deux plus petites tailles comptent)
    let narrow: Vec<Option<(usize, usize)>> = (0..n).map(|c| if water[c] { narrow_sides(&grid, water, &land_id, c) } else { None }).collect();
    let mut seen = vec![false; n];
    let mut found: Vec<(usize, Vec<usize>)> = Vec::new();
    for start in 0..n {
        let Some(sides) = narrow[start] else { continue };
        if seen[start] || land_size[sides.0].min(land_size[sides.1]) < STRAIT_MIN_LAND {
            continue;
        }
        let mut cells = vec![start];
        seen[start] = true;
        let mut k = 0;
        while k < cells.len() {
            let c = cells[k];
            k += 1;
            for j in grid.neighbors(c) {
                if !seen[j] && narrow[j].is_some_and(|s| s == sides || (s.1, s.0) == sides) {
                    seen[j] = true;
                    cells.push(j);
                }
            }
        }
        let weight = land_size[sides.0].min(land_size[sides.1]);
        found.push((weight, cells));
    }
    found.sort_by_key(|(w, cells)| (std::cmp::Reverse(*w), cells[0]));
    for (_, mut cells) in found.into_iter().take(MAX_STRAITS) {
        cells.sort_unstable();
        let parent = seas.basin_of[cells[0]].map(|b| b as usize);
        let id = seas.zones.len();
        for &c in &cells {
            seas.strait_of[c] = Some(id as u16);
        }
        seas.zones.push(zone(&grid, SeaKind::Strait, cells, parent, &seas.strait_of));
    }

    // 6) Mers bordières : parties abritées des océans, le long des côtes (entre une côte et des
    //    îles, devant une côte concave), de taille modeste ; elles peuvent contenir des golfes,
    //    des baies, des détroits. Une partie trop grande ne garde que son cœur le plus abrité ; une
    //    trop petite n'est pas nommée, ni une qui ne serait guère qu'un golfe.
    let free: Vec<bool> = (0..n)
        .map(|c| water[c] && dist[c] <= COASTAL_REACH && seas.basin_of[c].is_some_and(|b| seas.zones[b as usize].kind == SeaKind::Ocean))
        .collect();
    // Abri de chaque case (rayons qui touchent une terre), moyenné sur 2 cases alentour : des
    // contours réguliers plutôt que des doigts
    let raw: Vec<f32> = (0..n)
        .map(|c| if water[c] && dist[c] <= COASTAL_REACH + 2 { (0..12).filter(|&r| ray_hit(&grid, water, &land_id, c, r, SHELTER_RAY).is_some()).count() as f32 } else { 0.0 })
        .collect();
    let sheltered: Vec<u8> = (0..n)
        .map(|c| {
            if !free[c] {
                return 0;
            }
            let mut around: Vec<usize> = vec![c];
            for _ in 0..2 {
                let next: Vec<usize> = around.iter().flat_map(|&a| grid.neighbors(a)).collect();
                around.extend(next);
                around.sort_unstable();
                around.dedup();
            }
            let wet: Vec<f32> = around.iter().filter(|&&a| water[a]).map(|&a| raw[a]).collect();
            // (en dixièmes de rayon)
            (wet.iter().sum::<f32>() / wet.len().max(1) as f32 * 10.0).round() as u8
        })
        .collect();
    let mut pending: Vec<(Vec<usize>, u8)> = Vec::new();
    {
        let all: Vec<usize> = (0..n).filter(|&c| free[c]).collect();
        pending.push((all, SHELTERED_RAYS * 10));
    }
    let mut coastal: Vec<Vec<usize>> = Vec::new();
    while let Some((pool, threshold)) = pending.pop() {
        let members: HashSet<usize> = pool.iter().copied().filter(|&c| sheltered[c] >= threshold).collect();
        let mut seen: HashSet<usize> = HashSet::new();
        for &start in &pool {
            if !members.contains(&start) || seen.contains(&start) {
                continue;
            }
            let basin = seas.basin_of[start];
            let mut cells = vec![start];
            seen.insert(start);
            let mut k = 0;
            while k < cells.len() {
                let c = cells[k];
                k += 1;
                for j in grid.neighbors(c) {
                    if members.contains(&j) && !seen.contains(&j) && seas.basin_of[j] == basin {
                        seen.insert(j);
                        cells.push(j);
                    }
                }
            }
            // Ce qui n'est pas déjà dans un golfe, une baie ou un détroit
            let open = cells.iter().filter(|&&c| seas.inlet_of[c].is_none() && seas.strait_of[c].is_none()).count();
            if cells.len() > COASTAL_MAX_CELLS && threshold < 115 {
                pending.push((cells, threshold + 5));
            } else if (COASTAL_MIN_CELLS..=COASTAL_MAX_CELLS).contains(&cells.len()) && open as f32 >= cells.len() as f32 * COASTAL_OPEN_SHARE {
                coastal.push(cells);
            }
        }
    }
    coastal.sort_by_key(|cells| (std::cmp::Reverse(cells.len()), cells.iter().min().copied()));
    // Contours lissés : une case libre entourée par la mer bordière (4 voisines sur 6) en fait partie
    let mut taken: HashSet<usize> = HashSet::new();
    for cells in &mut coastal {
        let basin = seas.basin_of[cells[0]];
        for _ in 0..3 {
            let members: HashSet<usize> = cells.iter().copied().collect();
            let mut extra: Vec<usize> = members
                .iter()
                .flat_map(|&c| grid.neighbors(c))
                .filter(|&j| water[j] && !members.contains(&j) && !taken.contains(&j) && seas.basin_of[j] == basin)
                .filter(|&j| grid.neighbors(j).filter(|q| members.contains(q)).count() >= 4)
                .collect();
            extra.sort_unstable();
            extra.dedup();
            if extra.is_empty() {
                break;
            }
            cells.extend(extra);
        }
        // Pointes retirées : une case qui n'a qu'une ou deux voisines dans la mer bordière
        for _ in 0..2 {
            let members: HashSet<usize> = cells.iter().copied().collect();
            cells.retain(|&c| grid.neighbors(c).filter(|q| members.contains(q)).count() >= 3);
        }
        taken.extend(cells.iter().copied());
    }
    coastal.retain(|cells| cells.len() >= COASTAL_MIN_CELLS);
    for mut cells in coastal {
        cells.sort_unstable();
        let parent = seas.basin_of[cells[0]].map(|b| b as usize);
        let id = seas.zones.len();
        for &c in &cells {
            seas.coastal_of[c] = Some(id as u16);
        }
        seas.zones.push(zone(&grid, SeaKind::Sea, cells, parent, &seas.coastal_of));
    }
    // Un golfe, une baie, un détroit dont la plupart des cases sont dans une mer bordière en font
    // partie
    for z in 0..seas.zones.len() {
        if !matches!(seas.zones[z].kind, SeaKind::Gulf | SeaKind::Bay | SeaKind::Strait) {
            continue;
        }
        let mut count: HashMap<u16, usize> = HashMap::new();
        for &c in &seas.zones[z].cells {
            if let Some(s) = seas.coastal_of[c] {
                *count.entry(s).or_default() += 1;
            }
        }
        if let Some((&s, &k)) = count.iter().max_by_key(|(s, k)| (**k, std::cmp::Reverse(**s)))
            && k * 2 > seas.zones[z].cells.len()
        {
            seas.zones[z].parent = Some(s as usize);
        }
    }
    seas
}

/// Premier obstacle le long d'un rayon (12 directions : les 6 du réseau et les 6 entre elles) :
/// nombre de pas et terre touchée, à moins de `reach` pas.
fn ray_hit(grid: &Grid, water: &[bool], land_id: &[usize], c: usize, ray: usize, reach: i32) -> Option<(i32, usize)> {
    let (a, b) = (DIRECTIONS[ray / 2], DIRECTIONS[(ray / 2 + ray % 2) % 6]);
    let mut at = grid.hex(c);
    for s in 1..=reach {
        at = at + if s % 2 == 1 { a } else { b };
        let j = grid.index(at)?;
        if !water[j] {
            return Some((s, land_id[j]));
        }
    }
    None
}

/// Nombre de rayons (sur 12) qui touchent une même terre à moins de `RAY_LENGTH` cases, et cette
/// terre (la plus touchée : un passage entre une île et un continent n'est pas un golfe).
fn enclosure(grid: &Grid, water: &[bool], land_id: &[usize], c: usize) -> (usize, usize) {
    let mut hits: HashMap<usize, usize> = HashMap::new();
    for ray in 0..12 {
        if let Some((_, land)) = ray_hit(grid, water, land_id, c, ray, RAY_LENGTH) {
            *hits.entry(land).or_default() += 1;
        }
    }
    hits.into_iter().max_by_key(|&(land, k)| (k, std::cmp::Reverse(land))).map(|(land, k)| (k, land)).unwrap_or((0, usize::MAX))
}

/// Case entre deux caps d'une même terre : dans une direction et son opposée, la terre `land` des
/// deux côtés, à au plus `HEADLAND_WIDTH` cases d'eau en tout (l'entrée d'un golfe).
fn between_headlands(grid: &Grid, water: &[bool], land_id: &[usize], c: usize, land: usize) -> bool {
    (0..6).any(|ray| {
        let (Some((s1, l1)), Some((s2, l2))) =
            (ray_hit(grid, water, land_id, c, ray, HEADLAND_WIDTH), ray_hit(grid, water, land_id, c, ray + 6, HEADLAND_WIDTH))
        else {
            return false;
        };
        l1 == land && l2 == land && s1 + s2 - 1 <= HEADLAND_WIDTH
    })
}

/// Case étroite : le long d'un des trois axes, de la terre des deux côtés à moins de
/// `STRAIT_MAX_WIDTH` cases d'eau en tout, et ces deux terres sont distinctes. Rend les deux terres.
fn narrow_sides(grid: &Grid, water: &[bool], land_id: &[usize], c: usize) -> Option<(usize, usize)> {
    let origin = grid.hex(c);
    let walk = |d: usize| -> Option<(usize, usize)> {
        let mut at = origin;
        for steps in 1..=STRAIT_MAX_WIDTH {
            at = at + DIRECTIONS[d];
            let j = grid.index(at)?;
            if !water[j] {
                return Some((steps, land_id[j]));
            }
        }
        None
    };
    (0..3).find_map(|a| {
        let ((s1, l1), (s2, l2)) = (walk(a)?, walk(a + 3)?);
        (s1 + s2 - 1 <= STRAIT_MAX_WIDTH && l1 != l2).then_some((l1.min(l2), l1.max(l2)))
    })
}

/// Étendue avec sa case de nom (la plus éloignée de son bord) et sa demi-largeur.
fn zone(grid: &Grid, kind: SeaKind, cells: Vec<usize>, parent: Option<usize>, owner: &[Option<u16>]) -> SeaZone {
    let me = owner[cells[0]];
    let members: HashSet<usize> = cells.iter().copied().collect();
    // Distance au bord de l'étendue
    let mut inner: HashMap<usize, u16> = HashMap::new();
    let mut queue = VecDeque::new();
    for &c in &cells {
        let edge = DIRECTIONS.iter().any(|&d| grid.index(grid.hex(c) + d).is_none_or(|j| owner[j] != me));
        if edge {
            inner.insert(c, 0);
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        let d = inner[&c];
        for j in grid.neighbors(c) {
            if members.contains(&j) && !inner.contains_key(&j) {
                inner.insert(j, d + 1);
                queue.push_back(j);
            }
        }
    }
    let inner: Vec<u16> = cells.iter().map(|c| inner.get(c).copied().unwrap_or(0)).collect();
    let label_cell = cells.iter().zip(&inner).max_by_key(|(c, d)| (**d, std::cmp::Reverse(**c))).map(|(c, _)| *c).unwrap_or(cells[0]);
    // Demi-largeur d'ouest en est autour de la case du nom (planète : par le plus court côté)
    let (lx, _) = grid.hex(label_cell).to_world();
    let world = grid.wrap.world_width();
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    for &c in &cells {
        let (x, _) = grid.hex(c).to_world();
        let mut dx = x - lx;
        if world > 0.0 {
            dx -= (dx / world).round() * world;
        }
        lo = lo.min(dx);
        hi = hi.max(dx);
    }
    let half_width = (hi - lo) / 2.0 + crate::hex::HEX_SIZE;
    SeaZone { kind, cells, inner, parent, half_width, name: String::new(), feminine: false, etymology: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Carte faite d'une chaîne de caractères : '#' terre, '.' mer.
    fn grid(rows: &[&str]) -> (i32, i32, Vec<bool>) {
        let w = rows[0].len() as i32;
        let water = rows.iter().flat_map(|r| r.chars().map(|c| c == '.')).collect();
        (w, rows.len() as i32, water)
    }

    #[test]
    fn une_mer_fermee_par_un_detroit() {
        // Un mur de terre (3 cases d'épaisseur) du nord au sud, percé d'une passe d'une case : deux
        // étendues de part et d'autre, deux terres distinctes de part et d'autre de la passe
        let mut rows: Vec<String> = Vec::new();
        for y in 0..40 {
            rows.push((0..90).map(|x| if (54..=56).contains(&x) && y != 20 { '#' } else { '.' }).collect());
        }
        let refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
        let (w, h, water) = grid(&refs);
        let seas = compute(w, h, Wrap::NONE, &water);
        let left = seas.basin_of[(20 * w + 20) as usize];
        let right = seas.basin_of[(20 * w + 75) as usize];
        assert!(left.is_some() && right.is_some());
        assert_ne!(left, right, "le passage étroit sépare les deux étendues");
        let strait = seas.strait_of[(20 * w + 55) as usize];
        assert!(strait.is_some(), "un détroit dans la passe");
        assert_eq!(seas.zones[strait.unwrap() as usize].kind, SeaKind::Strait);
    }

    #[test]
    fn golfe_et_baie() {
        // Une mer ouverte en bas ; un golfe large qui entre dans la terre en haut à gauche et une
        // petite baie en haut à droite
        let mut rows: Vec<String> = Vec::new();
        for y in 0..50 {
            let mut r = String::new();
            for x in 0..80 {
                let land = y < 25;
                let gulf = (9..19).contains(&x) && y >= 8;
                let bay = (60..63).contains(&x) && y >= 21;
                r.push(if land && !gulf && !bay { '#' } else { '.' });
            }
            rows.push(r);
        }
        let refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
        let (w, h, water) = grid(&refs);
        let seas = compute(w, h, Wrap::NONE, &water);
        let gulf = seas.inlet_of[(11 * w + 14) as usize].map(|z| seas.zones[z as usize].kind);
        assert_eq!(gulf, Some(SeaKind::Gulf));
        let bay = seas.inlet_of[(23 * w + 61) as usize].map(|z| seas.zones[z as usize].kind);
        assert_eq!(bay, Some(SeaKind::Bay));
        // La mer du large n'est pas enveloppée
        assert!(seas.inlet_of[(45 * w + 40) as usize].is_none());
    }

    #[test]
    fn monde_genere() {
        let m = crate::map::WorldMap::generate_in(192, 111, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let s = m.seas();
        let count = |k: SeaKind| s.zones.iter().filter(|z| z.kind == k).count();
        println!(
            "océans {}, mers {}, golfes {}, baies {}, détroits {}",
            count(SeaKind::Ocean),
            count(SeaKind::Sea),
            count(SeaKind::Gulf),
            count(SeaKind::Bay),
            count(SeaKind::Strait)
        );
        for z in &s.zones {
            println!("  {:<8} {:<28} {:>5} cases", z.kind.label(), z.name, z.cells.len());
        }
        assert!((1..=8).contains(&count(SeaKind::Ocean)));
        assert!(count(SeaKind::Sea) >= 2);
        assert!(count(SeaKind::Gulf) + count(SeaKind::Bay) >= 5);
        // Toute case de mer ouverte est dans un océan ou une mer ; chaque étendue a un nom unique
        for h in m.hexes() {
            let c = m.cell(h);
            if m.get(h).is_some_and(|t| t.is_sea()) && m.basins().lake_of[c].is_none() {
                assert!(s.basin_of[c].is_some(), "{h:?}");
            }
        }
        let mut names = HashSet::new();
        for z in &s.zones {
            assert!(!z.name.is_empty());
            assert!(names.insert(z.name.to_lowercase()), "{} répété", z.name);
        }
    }

    /// Image des étendues (graine 777) : `cargo test vue_des_mers -- --ignored`, puis le script.
    #[test]
    #[ignore]
    fn vue_des_mers() {
        let seed = std::env::var("SEAS_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(777);
        let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let s = m.seas();
        let sea = m.hexes().filter(|&h| m.get(h).is_some_and(|t| t.is_sea())).count();
        let terrains: std::collections::HashMap<String, usize> = m.hexes().fold(Default::default(), |mut acc, h| {
            *acc.entry(format!("{:?}", m.get(h))).or_default() += 1;
            acc
        });
        println!("  mer {sea} cases ; {terrains:?}");
        for z in &s.zones {
            println!("  {:<8} {:<28} {:>5} cases", z.kind.label(), z.name, z.cells.len());
        }
        let mut out = String::new();
        for h in m.hexes() {
            let c = m.cell(h);
            let land = !m.get(h).is_some_and(|t| t.is_sea()) || m.basins().lake_of[c].is_some();
            let f = |v: Option<u16>| v.map_or(-1, i32::from);
            out += &format!("{} {} {} {} {} {}\n", u8::from(land), f(s.basin_of[c]), f(s.inlet_of[c]), f(s.strait_of[c]), m.cell_info(h).map_or(0, |i| i.language as u8), f(s.coastal_of[c]));
        }
        std::fs::write(std::env::var("SEAS_DUMP").unwrap_or("/tmp/seas.txt".into()), out).unwrap();
    }
}
