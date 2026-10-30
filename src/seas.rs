//! Étendues marines nommées (logique pure, calculée à la génération) : océans, mers, golfes,
//! baies et détroits, emboîtés (baie › golfe › mer fermée ou bordière › océan).
//!
//! 1. **Distance à la côte** de chaque case de mer (en cases).
//! 2. **Océans** : la mer est découpée là où elle se resserre. Les cases sont prises de la plus
//!    éloignée de la côte à la plus proche (le « relief » de la distance à la côte), chacune rejoignant sa voisine la plus au large : deux
//!    étendues qui se rejoignent restent séparées si le passage qui les relie est étroit par
//!    rapport à leur largeur, sinon elles n'en font qu'une ; une petite étendue se fond toujours
//!    dans sa voisine. Seules les grandes étendues restent : les océans. Au-delà du 60e
//!    parallèle, la mer de chaque pôle est l'océan polaire, à part, sans découpage.
//! 3. **Détroits** : passages de 1 à 4 cases de large entre deux terres distinctes (une île et un
//!    continent, deux îles), chacune d'au moins quelques cases.
//! 4. **Mers, golfes, baies**, une question d'échelle : un cercle venu du large balaie la mer
//!    (rayons de 1 à 12 cases) ; ce qu'il n'atteint pas est à l'abri derrière un goulet. Chaque
//!    poche est prolongée jusqu'à la ligne entre les caps de son entrée et gardée si elle s'enfonce
//!    assez par rapport à son entrée. Très fermée (seuil d'au plus un tiers de sa largeur ; le
//!    seuil est le passage le plus étroit près de l'entrée, où un haut-fond compte pour moitié) et
//!    d'au moins 150 cases : mer fermée, qui commence à son seuil, balayée à son tour par son propre cercle (ses golfes et
//!    baies sont dedans). Sinon, bordée surtout par une terre : golfe (21 cases et plus, sans
//!    limite haute) ou baie (3 à 20 cases) ; par plusieurs terres (île et continent, arc d'îles) :
//!    mer bordière, prolongée sur le plateau continental voisin. Contours réguliers, sans trou ni
//!    doigt.
//!
//! Chaque étendue garde la distance de chaque case à son bord (où poser son nom) et sa
//! demi-largeur.

use crate::hex::{DIRECTIONS, Hex, Wrap};
use crate::toponymy::Etymology;
use bevy::platform::collections::{HashMap, HashSet};
use std::collections::VecDeque;

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
    /// Mer (fermée, ou bordière : partie abritée d'un océan le long d'une côte) de chaque case.
    pub coastal_of: Vec<Option<u16>>,
    /// Baie de chaque case (dans un golfe, une mer…).
    pub bay_of: Vec<Option<u16>>,
}

impl Seas {
    /// Étendues d'une case, de la plus précise à la plus vaste (détroit, golfe ou baie, mer ou
    /// océan).
    pub fn chain(&self, cell: usize) -> Vec<usize> {
        [&self.strait_of, &self.bay_of, &self.inlet_of, &self.coastal_of, &self.basin_of].iter().filter_map(|v| v.get(cell).copied().flatten()).map(|z| z as usize).collect()
    }
}

/// Au-delà de cette latitude (degrés), la mer est l'océan polaire, d'un seul tenant (comme
/// l'océan Austral au-delà du 60e parallèle sud) : il n'est pas découpé là où une terre le
/// resserre, et s'arrête à ce parallèle.
pub const POLAR_OCEAN_LATITUDE: f32 = 60.0;
/// Une étendue d'au moins autant de cases (et au moins `OCEAN_SHARE` de la mer) est un océan.
const OCEAN_MIN_CELLS: usize = 700;
const OCEAN_SHARE: f32 = 0.08;
/// Plus petite étendue gardée à part pendant le découpage.
const MIN_BASIN_CELLS: usize = 40;
/// Passage étroit : moins large que cette part de la moins large des deux étendues.
const NARROW_RATIO: f32 = 0.65;
/// Échelles du cercle venu du large (rayon en cases) : des criques aux grands golfes.
const SCALES: [u16; 8] = [2, 3, 4, 5, 6, 8, 10, 13];
/// Enfoncement : surface rapportée au carré de l'entrée (un demi-cercle sur l'entrée, le test du
/// droit de la mer pour une baie, en fait ~0,45 ; on accepte aussi les grandes échancrures
/// ouvertes que les cartes appellent golfes, comme le golfe de Guinée ou d'Alaska) et profondeur
/// (cases entre l'entrée et le fond).
const GULF_INDENT: f32 = 0.15;
const GULF_DEPTH: u16 = 3;
const BAY_INDENT: f32 = 0.3;
const BAY_DEPTH: u16 = 2;
/// Profondeur au moins ce tiers de l'entrée : une frange le long d'une côte droite ne s'enfonce pas.
const DEPTH_PER_MOUTH: f32 = 0.33;
/// Golfe : au moins tant de cases, sans limite haute (le golfe du Mexique ferait ~1 100 cases de
/// 40 km) ; baie : de `BAY_MIN_CELLS` à `GULF_MIN_CELLS` − 1 cases.
pub const GULF_MIN_CELLS: usize = 21;
pub const BAY_MIN_CELLS: usize = 3;
const MAX_BAYS: usize = 40;
/// Une baie n'est nommée dans un golfe que si elle en est une petite partie.
const BAY_IN_GULF_SHARE: f32 = 0.4;
/// Une poche bordée à ce point par une seule terre est un golfe ou une baie.
const MAIN_LAND_SHARE: f32 = 0.7;
/// Terre qui compte pour border une mer : cette part du rivage au moins.
const SIDE_LAND_SHARE: f32 = 0.15;
/// Mers bordières : taille, part de terre sur leur pourtour.
const SEA_MIN_CELLS: usize = 50;
const SEA_MAX_CELLS: usize = 1200;
const SEA_LAND_BORDER: f32 = 0.5;
/// Une mer bordière s'étend sur le plateau continental voisin, jusqu'à tant de cases.
const SHELF_REACH: usize = 3;
/// Une poche qui fait presque toute sa mer fermée est cette mer, pas une partie.
const WHOLE_BASIN_SHARE: f32 = 0.85;
/// Mer fermée : poche dont l'entrée fait au plus cette part de sa largeur intérieure, et d'au moins
/// tant de cases (plus petite, ou plus ouverte : golfe).
const CLOSED_MOUTH_SHARE: f32 = 0.33;
const CLOSED_SEA_MIN_CELLS: usize = 150;
/// Poids d'une case peu profonde (plateau) dans la largeur d'un passage : un seuil sous-marin
/// ferme à demi (Gibraltar pour la Méditerranée, détroits danois pour la Baltique).
const SILL_WEIGHT: f32 = 0.5;
/// Identifiants provisoires des mers fermées vues comme des étendues à part (au-delà des océans).
const CLOSED_BASE: u16 = 30_000;
/// Largeur maximale d'un détroit (cases) et taille minimale des terres qu'il sépare.
const STRAIT_MAX_WIDTH: usize = 4;
const STRAIT_MIN_LAND: usize = 4;
const MAX_STRAITS: usize = 30;

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

/// Étendues marines. `water` : case de mer ouverte (ni lac, ni mer intérieure, déjà nommés) ;
/// `shelf` : case du plateau continental (moins de 200 m) ; `polar` : case au-delà de
/// `POLAR_OCEAN_LATITUDE` sur une carte qui va jusqu'au pôle.
pub fn compute(width: i32, height: i32, wrap: Wrap, water: &[bool], shelf: &[bool], polar: &[bool]) -> Seas {
    let grid = Grid { width, height, wrap };
    let n = water.len();
    let mut seas = Seas { zones: Vec::new(), basin_of: vec![None; n], inlet_of: vec![None; n], strait_of: vec![None; n], coastal_of: vec![None; n], bay_of: vec![None; n] };
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
    // Ordre de traitement : par distance décroissante ; à distance égale, en propageant depuis les
    // étendues déjà formées (sinon, prises dans l'ordre des lignes, les cases d'une côte rejoignent
    // de proche en proche l'étendue d'en haut, en une frange qui longe tout le rivage)
    // Océans polaires : au-delà du 60e parallèle, le plus grand morceau d'un seul tenant de chaque
    // pôle (0 : nord, 1 : sud) ; pas découpés, ils ne prennent pas part au partage des eaux
    let mut polar_of: Vec<Option<usize>> = vec![None; n];
    for side in 0..2 {
        let on_side = |c: usize| water[c] && polar[c] && usize::from(c as i32 / width >= height / 2) == side;
        let mut seen = vec![false; n];
        let mut best: Vec<usize> = Vec::new();
        for start in 0..n {
            if seen[start] || !on_side(start) {
                continue;
            }
            seen[start] = true;
            let mut cells = vec![start];
            let mut k = 0;
            while k < cells.len() {
                let c = cells[k];
                k += 1;
                for j in grid.neighbors(c) {
                    if !seen[j] && on_side(j) {
                        seen[j] = true;
                        cells.push(j);
                    }
                }
            }
            if cells.len() > best.len() {
                best = cells;
            }
        }
        for c in best {
            polar_of[c] = Some(side);
        }
    }
    let mut flood = Vec::with_capacity(order.len());
    let mut queued = vec![false; n];
    let mut level = 0;
    while level < order.len() {
        let d = dist[order[level]];
        let end = order[level..].iter().position(|&c| dist[c] != d).map_or(order.len(), |k| level + k);
        let mut queue: VecDeque<usize> = VecDeque::new();
        for &c in &order[level..end] {
            if grid.neighbors(c).any(|j| water[j] && dist[j] > d) {
                queued[c] = true;
                queue.push_back(c);
            }
        }
        let mut seeds = order[level..end].iter();
        loop {
            let c = match queue.pop_front() {
                Some(c) => c,
                None => match seeds.find(|&&c| !queued[c]) {
                    Some(&c) => {
                        queued[c] = true;
                        c
                    }
                    None => break,
                },
            };
            flood.push(c);
            for j in grid.neighbors(c) {
                if water[j] && dist[j] == d && !queued[j] {
                    queued[j] = true;
                    queue.push_back(j);
                }
            }
        }
        level = end;
    }
    for &c in &flood {
        if polar_of[c].is_some() {
            continue;
        }
        added[c] = true;
        peak[c] = dist[c];
        size[c] = 1;
        // La case rejoint l'étendue de sa voisine la plus au large (la pente la plus raide) ; les
        // autres fusionnent si le passage est large
        let mut near: Vec<(u16, usize)> = grid.neighbors(c).filter(|&j| added[j] && j != c).map(|j| (dist[j], find(&mut parent, j))).collect();
        near.sort_by_key(|&(d, r)| (std::cmp::Reverse(d), std::cmp::Reverse(peak[r]), std::cmp::Reverse(size[r]), r));
        let mut roots: Vec<usize> = Vec::new();
        for (_, r) in near {
            if !roots.contains(&r) {
                roots.push(r);
            }
        }
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
        region[c] = Some(polar_of[c].map_or_else(|| find(&mut parent, c), |side| n + side));
    }

    // 3) Océans : le découpage ne sert qu'à les séparer entre eux ; les étendues plus petites
    //    rejoignent leur voisine (les mers, fermées comprises, sont des poches à l'abri : étape 5)
    let ocean_min = OCEAN_MIN_CELLS.max((total_water as f32 * OCEAN_SHARE) as usize);
    loop {
        let mut cells_of: HashMap<usize, Vec<usize>> = HashMap::new();
        for &c in &order {
            cells_of.entry(region[c].unwrap()).or_default().push(c);
        }
        // Pourtour de chaque étendue : arêtes vers chaque voisine
        let mut merged = false;
        let mut regions: Vec<usize> = cells_of.keys().copied().collect();
        regions.sort_by_key(|r| (cells_of[r].len(), *r));
        for r in regions {
            let cells = &cells_of[&r];
            if cells.len() >= ocean_min {
                continue;
            }
            let mut shared: HashMap<usize, usize> = HashMap::new();
            for &c in cells {
                for j in grid.neighbors(c) {
                    if let Some(o) = region[j]
                        && o != r
                    {
                        *shared.entry(o).or_default() += 1;
                    }
                }
            }
            if shared.is_empty() {
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
    // 4) Détroits : passages étroits entre deux terres distinctes
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

    // 5) Mers, golfes, baies : ce qui reste à l'abri derrière un goulet quand un cercle venu du
    //    large balaie la mer, à plusieurs échelles (voir `candidates`).
    //    a) Mers fermées : les poches très fermées et assez grandes, les plus grandes échelles
    //       d'abord, contours lissés
    let by_scale = |found: &mut Vec<Candidate>| found.sort_by(|a, b| b.scale.cmp(&a.scale).then(b.cells.len().cmp(&a.cells.len())).then(a.cells[0].cmp(&b.cells[0])));
    let mut found = candidates(&grid, water, shelf, &dist, &seas.basin_of, &land_id, &land_size, ocean_min, true);
    by_scale(&mut found);
    let basin_size = |c: usize| seas.basin_of[c].map_or(0, |b| seas.zones[b as usize].cells.len());
    let mut sea_cells: Vec<Vec<usize>> = Vec::new();
    let mut sea_of: Vec<Option<usize>> = vec![None; n];
    for cand in found.iter().filter(|c| c.closed) {
        let len = cand.cells.len();
        if len as f32 > WHOLE_BASIN_SHARE * basin_size(cand.cells[0]) as f32 || cand.cells.iter().filter(|&&c| sea_of[c].is_some()).count() as f32 > 0.2 * len as f32 {
            continue;
        }
        let cells: Vec<usize> = cand.cells.iter().copied().filter(|&c| sea_of[c].is_none()).collect();
        let cells = tidy(&grid, water, &seas.basin_of, cells, true, |j| sea_of[j].is_some());
        if cells.len() < CLOSED_SEA_MIN_CELLS {
            continue;
        }
        let id = sea_cells.len();
        for &c in &cells {
            sea_of[c] = Some(id);
        }
        sea_cells.push(cells);
    }
    //    b) Golfes, baies, mers bordières : chaque mer fermée est balayée par son propre cercle
    //       (ses golfes sont dedans) ; une baie peut être dans un golfe, un golfe dans une mer
    let closed_size: Vec<usize> = sea_cells.iter().map(Vec::len).collect();
    let region_of: Vec<Option<u16>> = (0..n).map(|c| sea_of[c].map_or(seas.basin_of[c], |s| Some(CLOSED_BASE + s as u16))).collect();
    let mut found = candidates(&grid, water, shelf, &dist, &region_of, &land_id, &land_size, ocean_min, false);
    by_scale(&mut found);
    let region_size = |c: usize| match region_of[c] {
        Some(r) if r >= CLOSED_BASE => closed_size[usize::from(r - CLOSED_BASE)],
        _ => basin_size(c),
    };
    // Cases au large (hors mer fermée ou bordière) de chaque océan ou mer
    let mut open_left: HashMap<u16, usize> = HashMap::new();
    for c in 0..n {
        if let Some(r) = region_of[c]
            && sea_of[c].is_none()
        {
            *open_left.entry(r).or_default() += 1;
        }
    }
    let mut gulf_cells: Vec<Vec<usize>> = Vec::new();
    let mut gulf_of: Vec<Option<usize>> = vec![None; n];
    for cand in found.iter().filter(|c| c.kind != SeaKind::Bay) {
        let len = cand.cells.len();
        // Toute une mer (ou un tout petit océan) n'est pas une partie d'elle-même
        if len as f32 > WHOLE_BASIN_SHARE * region_size(cand.cells[0]) as f32 {
            continue;
        }
        let in_sea = cand.cells.iter().filter(|&&c| sea_of[c].is_some()).count();
        let in_gulf = cand.cells.iter().filter(|&&c| gulf_of[c].is_some()).count();
        if cand.kind == SeaKind::Sea {
            // (pas de mer bordière dans une mer fermée)
            if in_sea as f32 > 0.2 * len as f32 || in_gulf as f32 > 0.5 * len as f32 {
                continue;
            }
            let mut cells: Vec<usize> = cand.cells.iter().copied().filter(|&c| sea_of[c].is_none()).collect();
            // La mer s'étend sur le plateau continental voisin : sa limite suit le talus
            let basin = region_of[cells[0]];
            let mut inside: HashSet<usize> = cells.iter().copied().collect();
            let mut front = cells.clone();
            for _ in 0..SHELF_REACH {
                let mut next = Vec::new();
                for &c in &front {
                    for j in grid.neighbors(c) {
                        // (en épaisseur : pas de doigt le long d'une côte)
                        if water[j]
                            && shelf[j]
                            && sea_of[j].is_none()
                            && region_of[j] == basin
                            && !inside.contains(&j)
                            && grid.neighbors(j).filter(|q| inside.contains(q)).count() >= 2
                        {
                            inside.insert(j);
                            next.push(j);
                        }
                    }
                }
                cells.extend(&next);
                front = next;
            }
            let cells = tidy(&grid, water, &region_of, cells, true, |j| sea_of[j].is_some());
            if cells.len() < SEA_MIN_CELLS {
                continue;
            }
            // Les mers bordières laissent un large à leur océan (où poser son nom)
            let Some(region) = basin else { continue };
            let left = open_left[&region].saturating_sub(cells.len());
            if (left as f32) < (1.0 - WHOLE_BASIN_SHARE) * region_size(cells[0]) as f32 {
                continue;
            }
            open_left.insert(region, left);
            let id = sea_cells.len();
            for &c in &cells {
                sea_of[c] = Some(id);
            }
            sea_cells.push(cells);
        } else {
            if in_gulf as f32 > 0.5 * len as f32 {
                continue;
            }
            let cells: Vec<usize> = cand.cells.iter().copied().filter(|&c| gulf_of[c].is_none()).collect();
            let cells = tidy(&grid, water, &region_of, cells, true, |j| gulf_of[j].is_some());
            if cells.len() < GULF_MIN_CELLS {
                continue;
            }
            let id = gulf_cells.len();
            for &c in &cells {
                gulf_of[c] = Some(id);
            }
            gulf_cells.push(cells);
        }
    }
    // Baies : les plus marquées d'abord ; dans un golfe seulement si elles n'en sont qu'une petite
    // partie (sinon c'est le golfe lui-même, vu de plus près)
    let mut bays: Vec<&Candidate> = found.iter().filter(|c| c.kind == SeaKind::Bay).collect();
    bays.sort_by(|a, b| b.quality.total_cmp(&a.quality).then(a.cells[0].cmp(&b.cells[0])));
    let mut bay_cells: Vec<Vec<usize>> = Vec::new();
    let mut bay_of: Vec<Option<usize>> = vec![None; n];
    for cand in bays {
        if bay_cells.len() >= MAX_BAYS {
            break;
        }
        let len = cand.cells.len();
        if cand.cells.iter().filter(|&&c| bay_of[c].is_some()).count() as f32 > 0.3 * len as f32 {
            continue;
        }
        if let Some((g, k)) = majority(&cand.cells, &gulf_of)
            && k * 2 > len
            && len as f32 > BAY_IN_GULF_SHARE * gulf_cells[g].len() as f32
        {
            continue;
        }
        let cells: Vec<usize> = cand.cells.iter().copied().filter(|&c| bay_of[c].is_none()).collect();
        if cells.len() < BAY_MIN_CELLS {
            continue;
        }
        let id = bay_cells.len();
        for &c in &cells {
            bay_of[c] = Some(id);
        }
        bay_cells.push(cells);
    }
    // Étendues : mers bordières (dans leur océan), golfes (dans leur mer bordière, sinon leur
    // océan ou mer), baies (dans leur golfe, sinon leur mer…)
    let first_sea = seas.zones.len();
    for (i, cells) in sea_cells.iter().enumerate() {
        for &c in cells {
            seas.coastal_of[c] = Some((first_sea + i) as u16);
        }
    }
    for mut cells in sea_cells {
        cells.sort_unstable();
        let parent = seas.basin_of[cells[0]].map(|b| b as usize);
        seas.zones.push(zone(&grid, SeaKind::Sea, cells, parent, &seas.coastal_of));
    }
    let first_gulf = seas.zones.len();
    for (i, cells) in gulf_cells.iter().enumerate() {
        for &c in cells {
            seas.inlet_of[c] = Some((first_gulf + i) as u16);
        }
    }
    for mut cells in gulf_cells {
        cells.sort_unstable();
        let parent = parent_of(&cells, &[&seas.coastal_of, &seas.basin_of]);
        seas.zones.push(zone(&grid, SeaKind::Gulf, cells, parent, &seas.inlet_of));
    }
    let first_bay = seas.zones.len();
    for (i, cells) in bay_cells.iter().enumerate() {
        for &c in cells {
            seas.bay_of[c] = Some((first_bay + i) as u16);
        }
    }
    for mut cells in bay_cells {
        cells.sort_unstable();
        let parent = parent_of(&cells, &[&seas.inlet_of, &seas.coastal_of, &seas.basin_of]);
        seas.zones.push(zone(&grid, SeaKind::Bay, cells, parent, &seas.bay_of));
    }
    // Le nom d'un océan ou d'une mer se pose au large de ses golfes et mers bordières : leurs cases
    // comptent comme son bord
    for z in 0..seas.zones.len() {
        if seas.zones[z].parent.is_some() {
            continue;
        }
        let nested = |c: usize| seas.coastal_of[c].is_some() || seas.inlet_of[c].is_some() || seas.bay_of[c].is_some();
        let cells = &seas.zones[z].cells;
        if !cells.iter().any(|&c| nested(c)) {
            continue;
        }
        let members: HashSet<usize> = cells.iter().copied().filter(|&c| !nested(c)).collect();
        let mut inner: HashMap<usize, u16> = HashMap::new();
        let mut queue = VecDeque::new();
        for &c in &members {
            if DIRECTIONS.iter().any(|&d| grid.index(grid.hex(c) + d).is_none_or(|j| !members.contains(&j))) {
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
        let fresh: Vec<u16> = cells.iter().map(|c| inner.get(c).copied().unwrap_or(0)).collect();
        seas.zones[z].inner = fresh;
    }
    // Un détroit dont la plupart des cases sont dans un golfe ou une mer bordière en fait partie
    for z in 0..seas.zones.len() {
        if seas.zones[z].kind == SeaKind::Strait {
            let cells = seas.zones[z].cells.clone();
            if let Some(p) = parent_of(&cells, &[&seas.inlet_of, &seas.coastal_of, &seas.basin_of]) {
                seas.zones[z].parent = Some(p);
            }
        }
    }
    seas
}

/// Une poche candidate : golfe, baie ou mer bordière, trouvée à une échelle donnée.
struct Candidate {
    scale: u16,
    cells: Vec<usize>,
    kind: SeaKind,
    /// Surface rapportée au carré de l'entrée (plus c'est grand, plus c'est fermé).
    quality: f32,
    /// Mer fermée (entrée étroite par rapport à sa largeur, assez grande).
    closed: bool,
}

/// Poches à l'abri, à chaque échelle `r` de `SCALES` : un cercle de rayon r − 1 qui vient du large
/// (de la plus grande partie de chaque océan ou mer où il tient) ne peut entrer que par des
/// passages d'au moins ~2r cases ; ce qu'il n'atteint pas est derrière un goulet. Chaque poche est
/// prolongée jusqu'à la ligne entre les caps de son entrée, puis gardée si elle s'enfonce assez
/// (surface rapportée au carré de l'entrée, profondeur). Bordée surtout par une seule terre :
/// golfe (au moins `GULF_MIN_CELLS` cases) ou baie ; par plusieurs terres (île et continent, arc
/// d'îles) et assez grande : mer bordière. Très fermée et assez grande (`closed_seas`) : mer fermée,
/// quelles que soient ses terres, qui commence à son seuil (le passage le plus étroit près de
/// l'entrée, une case du plateau `shelf` y comptant pour moitié).
#[allow(clippy::too_many_arguments)]
fn candidates(grid: &Grid, water: &[bool], shelf: &[bool], dist: &[u16], basin_of: &[Option<u16>], land_id: &[usize], land_size: &[usize], ocean_min: usize, closed_seas: bool) -> Vec<Candidate> {
    let n = water.len();
    let mut out = Vec::new();
    for &r in &SCALES {
        // Centres possibles du cercle, regroupés par étendue ; le large = le plus grand groupe
        let mut group = vec![usize::MAX; n];
        let mut groups: Vec<(Option<u16>, usize)> = Vec::new();
        for start in 0..n {
            if !water[start] || dist[start] < r || group[start] != usize::MAX {
                continue;
            }
            let id = groups.len();
            let basin = basin_of[start];
            group[start] = id;
            let mut stack = vec![start];
            let mut count = 0;
            while let Some(c) = stack.pop() {
                count += 1;
                for j in grid.neighbors(c) {
                    if water[j] && dist[j] >= r && group[j] == usize::MAX && basin_of[j] == basin {
                        group[j] = id;
                        stack.push(j);
                    }
                }
            }
            groups.push((basin, count));
        }
        let mut open_group: HashMap<Option<u16>, usize> = HashMap::new();
        for (id, &(basin, count)) in groups.iter().enumerate() {
            let best = open_group.entry(basin).or_insert(id);
            if count > groups[*best].1 {
                *best = id;
            }
        }
        let open: HashSet<usize> = open_group.values().copied().collect();
        // Cases balayées par le cercle
        let mut steps = vec![u16::MAX; n];
        let mut queue = VecDeque::new();
        for c in 0..n {
            if group[c] != usize::MAX && open.contains(&group[c]) {
                steps[c] = 0;
                queue.push_back(c);
            }
        }
        while let Some(c) = queue.pop_front() {
            if steps[c] + 1 >= r {
                continue;
            }
            for j in grid.neighbors(c) {
                if water[j] && steps[j] == u16::MAX && basin_of[j] == basin_of[c] {
                    steps[j] = steps[c] + 1;
                    queue.push_back(j);
                }
            }
        }
        // Éloignement des cases non balayées à la mer balayée : une frange d'une case le long d'une
        // côte découpée n'est pas une poche, seulement ce qui a de l'épaisseur
        let mut shade = vec![u16::MAX; n];
        for c in 0..n {
            if steps[c] != u16::MAX {
                shade[c] = 0;
                queue.push_back(c);
            }
        }
        while let Some(c) = queue.pop_front() {
            for j in grid.neighbors(c) {
                if water[j] && shade[j] == u16::MAX && basin_of[j] == basin_of[c] {
                    shade[j] = shade[c] + 1;
                    queue.push_back(j);
                }
            }
        }
        let uncovered = |j: usize| water[j] && steps[j] == u16::MAX;
        // Poches : cœurs d'un seul tenant (à au moins 2 cases de la mer balayée), avec leur frange
        let mut seen = vec![false; n];
        for start in 0..n {
            if !uncovered(start) || shade[start] < 2 || seen[start] {
                continue;
            }
            let basin = basin_of[start];
            let mut pocket = vec![start];
            seen[start] = true;
            let mut k = 0;
            while k < pocket.len() {
                let c = pocket[k];
                k += 1;
                for j in grid.neighbors(c) {
                    if uncovered(j) && shade[j] >= 2 && !seen[j] && basin_of[j] == basin {
                        seen[j] = true;
                        pocket.push(j);
                    }
                }
            }
            let mut members: HashSet<usize> = pocket.iter().copied().collect();
            let mut front = pocket.clone();
            for _ in 0..2 {
                let mut next = Vec::new();
                for &c in &front {
                    for j in grid.neighbors(c) {
                        if uncovered(j) && basin_of[j] == basin && members.insert(j) {
                            next.push(j);
                        }
                    }
                }
                pocket.extend(&next);
                front = next;
            }
            // (le bord de la carte n'est pas une côte : rien à l'abri contre un pôle)
            let polar = |c: usize| {
                let row = c as i32 / grid.width;
                row <= 1 || row >= grid.height - 2
            };
            if pocket.len() < BAY_MIN_CELLS || pocket.iter().any(|&c| polar(c)) {
                continue;
            }
            // Terres qui la bordent
            let mut shores: HashMap<usize, usize> = HashMap::new();
            for &c in &pocket {
                for j in grid.neighbors(c) {
                    if !water[j] {
                        *shores.entry(land_id[j]).or_default() += 1;
                    }
                }
            }
            let total: usize = shores.values().sum();
            if total == 0 {
                continue;
            }
            let mut shores: Vec<(usize, usize)> = shores.into_iter().collect();
            shores.sort_by_key(|&(land, k)| (std::cmp::Reverse(k), land));
            let sides: Vec<usize> =
                shores.iter().filter(|&&(land, k)| k as f32 >= SIDE_LAND_SHARE * total as f32 && land_size[land] >= 3).map(|&(land, _)| land).collect();
            let single = shores[0].1 as f32 >= MAIN_LAND_SHARE * total as f32 || sides.len() <= 1;
            let lands: Vec<usize> = if single { vec![shores[0].0] } else { sides };
            // Jusqu'à la ligne entre les caps (cases qui ont ces terres des deux côtés)
            let width = (2 * i32::from(r) + 2).max(3 * i32::from(r));
            let mut k = 0;
            while k < pocket.len() {
                let c = pocket[k];
                k += 1;
                for j in grid.neighbors(c) {
                    if water[j] && basin_of[j] == basin && !members.contains(&j) && chord(grid, water, land_id, j, &lands, width) {
                        members.insert(j);
                        pocket.push(j);
                    }
                }
            }
            if pocket.iter().any(|&c| polar(c)) {
                continue;
            }
            // Entrée : cases qui donnent sur la mer du dehors ; pourtour : terre / mer ; profondeur :
            // éloignement maximal à l'entrée (une frange le long d'une côte n'a pas de profondeur)
            let (mut land_edges, mut open_edges) = (0usize, 0usize);
            let mut depth: HashMap<usize, u16> = HashMap::new();
            let mut front = VecDeque::new();
            for &c in &pocket {
                let mut outside = false;
                for j in grid.neighbors(c) {
                    if !water[j] {
                        land_edges += 1;
                    } else if !members.contains(&j) {
                        open_edges += 1;
                        outside = true;
                    }
                }
                if outside {
                    depth.insert(c, 1);
                    front.push_back(c);
                }
            }
            let mouth = front.len();
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
            let len = pocket.len();
            let quality = len as f32 / (mouth.max(1) as f32).powi(2);
            let bay = len < GULF_MIN_CELLS;
            let (min_quality, min_depth) = if bay { (BAY_INDENT, BAY_DEPTH) } else { (GULF_INDENT, GULF_DEPTH) };
            if quality < min_quality || deepest < min_depth || (deepest as f32) < DEPTH_PER_MOUTH * mouth as f32 {
                continue;
            }
            // Mer fermée : largeur intérieure (deux fois l'éloignement maximal à la côte) et seuil,
            // la plus étroite des tranches parallèles à l'entrée (cases à même éloignement de
            // l'entrée), dans la première moitié de la poche ; une case peu profonde (plateau) y
            // compte pour moitié : un haut-fond ferme à demi le passage
            let inside = 2 * pocket.iter().map(|&c| dist[c]).max().unwrap_or(0);
            let mut section = vec![0f32; usize::from(deepest) + 1];
            for (&c, &d) in &depth {
                section[usize::from(d)] += if shelf[c] { SILL_WEIGHT } else { 1.0 };
            }
            let (sill, narrowest) =
                (1..=usize::from((deepest / 2).max(1))).map(|k| (k, section[k])).min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0))).unwrap_or((1, mouth as f32));
            let closed = closed_seas && narrowest <= CLOSED_MOUTH_SHARE * f32::from(inside.saturating_sub(1)) && {
                // (la mer commence au seuil : ce qui est en deçà reste au large)
                let kept = pocket.iter().filter(|&&c| usize::from(depth[&c]) >= sill).count();
                kept >= CLOSED_SEA_MIN_CELLS
            };
            if closed {
                pocket.retain(|c| usize::from(depth[c]) >= sill);
            }
            let kind = if closed {
                SeaKind::Sea
            } else if single {
                if len >= GULF_MIN_CELLS { SeaKind::Gulf } else { SeaKind::Bay }
            } else if (SEA_MIN_CELLS..=SEA_MAX_CELLS).contains(&len) && len < ocean_min && land_edges as f32 >= SEA_LAND_BORDER * (land_edges + open_edges) as f32 {
                SeaKind::Sea
            } else {
                continue;
            };
            pocket.sort_unstable();
            out.push(Candidate { scale: r, cells: pocket, kind, quality, closed });
        }
    }
    out
}

/// Contour régulier : les cases de mer entourées par l'étendue (4 voisines sur 6) en font partie,
/// les trous d'eau qu'elle enferme sont bouchés, les pointes qui avancent dans la mer du dehors
/// (au moins 4 voisines de mer hors de l'étendue ; `strict` : aussi les doigts d'une case le long
/// d'une côte) retirées ; on garde le plus grand morceau.
/// `taken` : cases déjà prises par une étendue du même rang.
fn tidy(grid: &Grid, water: &[bool], basin_of: &[Option<u16>], mut cells: Vec<usize>, strict: bool, taken: impl Fn(usize) -> bool) -> Vec<usize> {
    if cells.is_empty() {
        return cells;
    }
    let basin = basin_of[cells[0]];
    let free = |j: usize| water[j] && basin_of[j] == basin && !taken(j);
    let mut members: HashSet<usize> = cells.iter().copied().collect();
    for _ in 0..3 {
        let mut extra: Vec<usize> = members
            .iter()
            .flat_map(|&c| grid.neighbors(c))
            .filter(|&j| !members.contains(&j) && free(j) && grid.neighbors(j).filter(|q| members.contains(q)).count() >= 4)
            .collect();
        extra.sort_unstable();
        extra.dedup();
        if extra.is_empty() {
            break;
        }
        members.extend(extra);
    }
    // Trous : morceaux de mer hors de l'étendue qui ne mènent nulle part ailleurs
    let limit = members.len() / 3 + 1;
    let mut checked: HashSet<usize> = HashSet::new();
    let border: Vec<usize> = members.iter().flat_map(|&c| grid.neighbors(c)).filter(|j| !members.contains(j) && water[*j]).collect();
    for start in border {
        if checked.contains(&start) || !free(start) {
            continue;
        }
        let mut hole = vec![start];
        checked.insert(start);
        let mut enclosed = true;
        let mut k = 0;
        while k < hole.len() && enclosed {
            let c = hole[k];
            k += 1;
            for j in grid.neighbors(c) {
                if !water[j] || members.contains(&j) || checked.contains(&j) {
                    continue;
                }
                if !free(j) || hole.len() >= limit {
                    enclosed = false;
                    break;
                }
                checked.insert(j);
                hole.push(j);
            }
        }
        if enclosed {
            members.extend(hole);
        }
    }
    for _ in 0..2 {
        let spikes: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&c| {
                let outside = grid.neighbors(c).filter(|j| water[*j] && !members.contains(j)).count();
                let within = grid.neighbors(c).filter(|j| members.contains(j)).count();
                outside >= 4 || (strict && within <= 2 && outside > 0)
            })
            .collect();
        for c in spikes {
            members.remove(&c);
        }
    }
    // Le plus grand morceau
    cells = members.iter().copied().collect();
    cells.sort_unstable();
    let mut best: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
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

/// Étendue qui contient le plus de ces cases, et combien.
fn majority(cells: &[usize], owner: &[Option<usize>]) -> Option<(usize, usize)> {
    let mut count: HashMap<usize, usize> = HashMap::new();
    for &c in cells {
        if let Some(o) = owner[c] {
            *count.entry(o).or_default() += 1;
        }
    }
    count.into_iter().max_by_key(|&(o, k)| (k, std::cmp::Reverse(o)))
}

/// Parent d'une étendue : la première, dans l'ordre donné, qui contient plus de la moitié de ses
/// cases (l'océan ou la mer en dernier recours).
fn parent_of(cells: &[usize], owners: &[&Vec<Option<u16>>]) -> Option<usize> {
    for (i, owner) in owners.iter().enumerate() {
        let mut count: HashMap<u16, usize> = HashMap::new();
        for &c in cells {
            if let Some(o) = owner[c] {
                *count.entry(o).or_default() += 1;
            }
        }
        if let Some((z, k)) = count.into_iter().max_by_key(|&(o, k)| (k, std::cmp::Reverse(o)))
            && (k * 2 > cells.len() || i + 1 == owners.len())
        {
            return Some(z as usize);
        }
    }
    None
}

/// Case sur une ligne entre deux caps : dans une direction et son opposée, une des terres `lands`
/// de chaque côté, à au plus `width` cases d'eau en tout.
fn chord(grid: &Grid, water: &[bool], land_id: &[usize], c: usize, lands: &[usize], width: i32) -> bool {
    (0..6).any(|ray| {
        let (Some((s1, l1)), Some((s2, l2))) = (ray_hit(grid, water, land_id, c, ray, width), ray_hit(grid, water, land_id, c, ray + 6, width)) else {
            return false;
        };
        lands.contains(&l1) && lands.contains(&l2) && s1 + s2 - 1 <= width
    })
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
    fn ocean_polaire_d_un_seul_tenant() {
        // Planète de 96 × 56 cases, bouclée : un continent au milieu, dont une péninsule monte
        // presque jusqu'au pôle nord (3 lignes de mer) ; au-delà du 60e parallèle (lignes 0 à 9 et
        // 46 à 55), la mer est l'océan polaire, d'un seul tenant malgré le resserrement
        let (w, h) = (96, 56);
        let land = |x: i32, y: i32| ((10..40).contains(&x) && (14..42).contains(&y)) || ((20..34).contains(&x) && (3..14).contains(&y));
        let water: Vec<bool> = (0..w * h).map(|c| !land(c % w, c / w)).collect();
        let polar: Vec<bool> = (0..w * h).map(|c| !(10..46).contains(&(c / w))).collect();
        let seas = compute(w, h, Wrap::new(w, true), &water, &vec![false; water.len()], &polar);
        let at = |x: i32, y: i32| seas.basin_of[(y * w + x) as usize];
        let north = at(60, 2).expect("océan polaire nord");
        assert_eq!(seas.zones[north as usize].kind, SeaKind::Ocean);
        for (c, _) in water.iter().enumerate().filter(|&(c, &wet)| wet && c / w as usize <= 9) {
            assert_eq!(seas.basin_of[c], Some(north), "case {c} hors de l'océan polaire nord");
        }
        // Il s'arrête au parallèle : le large du milieu et le pôle sud sont d'autres océans
        assert_ne!(at(70, 28), Some(north));
        assert_ne!(at(60, 52), Some(north));
        assert_ne!(at(70, 28), at(60, 52));
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
        let seas = compute(w, h, Wrap::NONE, &water, &vec![false; water.len()], &vec![false; water.len()]);
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
        let seas = compute(w, h, Wrap::NONE, &water, &vec![false; water.len()], &vec![false; water.len()]);
        let gulf = seas.inlet_of[(11 * w + 14) as usize].map(|z| seas.zones[z as usize].kind);
        assert_eq!(gulf, Some(SeaKind::Gulf));
        let bay = seas.bay_of[(23 * w + 61) as usize].map(|z| seas.zones[z as usize].kind);
        assert_eq!(bay, Some(SeaKind::Bay));
        // La mer du large n'est pas enveloppée
        assert!(seas.inlet_of[(45 * w + 40) as usize].is_none());
    }

    #[test]
    fn mer_fermee_avec_son_golfe() {
        // Un océan au sud ; au nord, dans la terre, une mer de 40 × 14 cases reliée à l'océan par un
        // chenal d'une case, avec un golfe sur sa rive nord
        let mut rows: Vec<String> = Vec::new();
        for y in 0..60 {
            rows.push(
                (0..100)
                    .map(|x| {
                        let ocean = y >= 40;
                        let sea = (8..22).contains(&y) && (25..65).contains(&x);
                        let channel = x == 45 && (22..40).contains(&y);
                        let gulf = (3..8).contains(&y) && (35..43).contains(&x);
                        if ocean || sea || channel || gulf { '.' } else { '#' }
                    })
                    .collect(),
            );
        }
        let refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
        let (w, h, water) = grid(&refs);
        let seas = compute(w, h, Wrap::NONE, &water, &vec![false; water.len()], &vec![false; water.len()]);
        let at = |x: i32, y: i32| (y * w + x) as usize;
        // La mer fermée : une mer dans l'océan (pas un océan à part), qui couvre tout le bassin
        let ocean = seas.basin_of[at(45, 50)].expect("océan");
        assert_eq!(seas.zones[ocean as usize].kind, SeaKind::Ocean);
        assert_eq!(seas.basin_of[at(45, 15)], Some(ocean), "la mer fermée est dans l'océan");
        let sea = seas.coastal_of[at(45, 15)].expect("mer fermée");
        assert_eq!(seas.zones[sea as usize].kind, SeaKind::Sea);
        assert_eq!(seas.zones[sea as usize].parent, Some(ocean as usize));
        for (x, y) in [(26, 9), (63, 20), (26, 20), (63, 9)] {
            assert_eq!(seas.coastal_of[at(x, y)], Some(sea), "coin ({x}, {y}) de la mer");
        }
        assert!(seas.coastal_of[at(45, 50)].is_none(), "le large n'est pas dans la mer");
        // Le golfe est dans la mer
        let gulf = seas.inlet_of[at(39, 5)].expect("golfe");
        assert_eq!(seas.zones[gulf as usize].kind, SeaKind::Gulf);
        assert_eq!(seas.zones[gulf as usize].parent, Some(sea as usize));
    }

    #[test]
    fn seuil_sous_marin() {
        // Une mer de 40 × 14 cases reliée à l'océan par un chenal de 6 cases de large : trop
        // ouvert pour une mer fermée ; avec un haut-fond (plateau) en travers du chenal, le passage
        // est à demi fermé, la mer fermée commence au seuil
        let (w, h) = (100, 60);
        let water: Vec<bool> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                y >= 40 || ((8..22).contains(&y) && (25..65).contains(&x)) || ((40..46).contains(&x) && (22..40).contains(&y))
            })
            .collect();
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let sill: Vec<bool> = (0..w * h).map(|i| water[i as usize] && (40..46).contains(&(i % w)) && (30..33).contains(&(i / w))).collect();
        let open = compute(w, h, Wrap::NONE, &water, &vec![false; water.len()], &vec![false; water.len()]);
        assert!(open.coastal_of[at(45, 15)].is_none(), "sans seuil : pas de mer fermée");
        let seas = compute(w, h, Wrap::NONE, &water, &sill, &vec![false; water.len()]);
        let sea = seas.coastal_of[at(45, 15)].expect("mer fermée derrière le seuil");
        assert_eq!(seas.zones[sea as usize].kind, SeaKind::Sea);
        assert_eq!(seas.coastal_of[at(42, 26)], Some(sea), "le chenal en deçà du seuil est dans la mer");
        assert!(seas.coastal_of[at(42, 37)].is_none(), "au-delà du seuil, le large");
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
        assert!(count(SeaKind::Ocean) + count(SeaKind::Sea) >= 2);
        assert!(count(SeaKind::Gulf) + count(SeaKind::Bay) >= 3);
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
        let terrains: bevy::platform::collections::HashMap<String, usize> = m.hexes().fold(Default::default(), |mut acc, h| {
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
            let shelf = m.depth(h).is_some_and(|d| d < crate::map::SHELF_DEPTH_M);
            out += &format!(
                "{} {} {} {} {} {} {} {}\n",
                u8::from(land),
                f(s.basin_of[c]),
                f(s.inlet_of[c]),
                f(s.strait_of[c]),
                m.cell_info(h).map_or(0, |i| i.language as u8),
                f(s.coastal_of[c]),
                f(s.bay_of[c]),
                u8::from(shelf)
            );
        }
        std::fs::write(std::env::var("SEAS_DUMP").unwrap_or("/tmp/seas.txt".into()), out).unwrap();
    }
}
