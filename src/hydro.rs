//! Hydrologie de la génération du monde : du ruissellement aux rivières, fleuves et lacs.
//!
//! Le climat (voir `climate.rs`) donne pour chaque case de terre la lame d'eau qui ruisselle
//! (le trop-plein du sol, en mm par an). Cette eau descend le relief jusqu'à la mer :
//!
//! 1. **Écoulement de case en case.** Chaque case envoie son eau vers sa voisine la plus basse.
//!    Les cuvettes sans issue sont remplies virtuellement (algorithme « Priority-Flood », Barnes
//!    et al., 2014) : l'eau trouve toujours le col le plus bas.
//! 2. **Lacs.** Une cuvette reçoit de l'eau ; sa surface évapore (évaporation d'une eau libre
//!    moins la pluie qui y tombe). Si les apports dépassent ce que la cuvette pleine évapore, le
//!    lac se remplit jusqu'au col et déborde : le surplus repart vers l'aval. Sinon le lac
//!    s'arrête à la taille où évaporation et apports s'équilibrent (lac terminal, sans issue,
//!    comme la mer Morte ou le lac Tchad). Un lac d'au moins une demi-case devient des cases
//!    d'eau, même en altitude ; plus petit, c'est un petit lac dans une case de terre.
//! 3. **Débits et pertes.** Le débit s'accumule vers l'aval. En climat sec, le lit perd de l'eau
//!    (infiltration, proportionnelle à la largeur du lit, qui croît comme la racine du débit) :
//!    les petits oueds se perdent, les grands fleuves traversent le désert en s'affaiblissant.
//! 4. **Fleuves.** Au-delà d'un fort débit, le cours d'eau occupe toute la case (fleuve).
//! 5. **Rivières sur les arêtes.** Les rivières plus modestes coulent entre les cases, le long
//!    des arêtes : l'eau circule de coin en coin (chaque coin a l'altitude moyenne des trois
//!    cases qui s'y touchent), reçoit le ruissellement des cases voisines et se jette dans la
//!    mer, un lac ou un fleuve.
//!
//! Logique pure (aucun rendu). Les grandeurs sont physiques (m, m³/s, mm/an).

#![allow(clippy::needless_range_loop)] // grilles indexées : l'indice est l'identité de la case

use crate::hex::{DIRECTIONS, Hex};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Débit à partir duquel une rivière est tracée sur les arêtes (m³/s)…
pub const RIVER_M3S: f32 = 100.0;
/// … pour un bassin versant d'au moins ce nombre de cases : sous les climats très humides, un
/// tout petit bassin suffirait à faire une rivière, et la carte en serait quadrillée.
pub const RIVER_MIN_BASIN_CELLS: f32 = 6.0;
/// Débit à partir duquel le cours d'eau occupe toute la case : fleuve (m³/s).
pub const FLEUVE_M3S: f32 = 2_000.0;
/// Un fleuve reste un fleuve vers l'aval tant qu'il garde cette part du seuil (il ne redevient
/// pas une rivière d'arête au premier coude sec).
const FLEUVE_KEEP: f32 = 0.5;
/// Surface minimale d'un petit lac (km²) ; une cuvette qui retient moins d'eau reste une
/// simple zone humide ou une cuvette sèche.
pub const SMALL_LAKE_KM2: f32 = 15.0;
/// Profondeur d'une cuvette (sous le col) à partir de laquelle toute la case peut être noyée ;
/// une cuvette moins profonde ne retient un lac que sur une partie de la case (le relief réel
/// est plus fin que la case : une cuvette de quelques mètres ne fait qu'un étang ou un marais).
const FULL_LAKE_DEPTH_M: f32 = 60.0;
/// Part d'une case à partir de laquelle un lac devient une case d'eau.
const LAKE_CELL_SHARE: f32 = 0.5;
/// Évaporation d'une eau libre par rapport à l'évaporation potentielle d'un sol humide.
const OPEN_WATER_EVAPORATION: f32 = 1.1;
/// Infiltration à travers le lit en climat aride (m par jour), et largeur du lit
/// (géométrie hydraulique : largeur ≈ 4,5 · débit^0,5, en m pour un débit en m³/s).
const BED_INFILTRATION_M_PER_DAY: f32 = 0.2;
const WIDTH_PER_SQRT_M3S: f32 = 4.5;
/// Indice d'aridité au-delà duquel le lit ne perd plus d'eau (climat subhumide à humide :
/// la nappe alimente la rivière plutôt que l'inverse).
const LOSING_BELOW_ARIDITY: f32 = 0.65;
/// Deux cases d'une même cuvette ont le même niveau de remplissage, à cette tolérance près (m).
const SAME_POOL_M: f32 = 0.5;
/// Pente minimale imposée en traversant une zone plate ou remplie (m par case).
const EPSILON_M: f32 = 1e-3;
const SECONDS_PER_YEAR: f32 = 31_557_600.0;

/// Données d'entrée, une valeur par case (ligne par ligne, comme la carte).
pub struct HydroInput<'a> {
    pub width: i32,
    pub height: i32,
    /// Distance entre centres de cases (km).
    pub cell_km: f32,
    /// Altitude du sol (m).
    pub elevation: &'a [f32],
    /// Mer (océan ou côte) : l'eau s'y jette.
    pub sea: &'a [bool],
    /// Ruissellement, précipitations, évaporation potentielle (mm par an), indice d'aridité.
    pub runoff: &'a [f32],
    pub rain: &'a [f32],
    pub pet: &'a [f32],
    pub aridity: &'a [f32],
}

/// Résultat, une valeur par case.
#[derive(Clone, Debug, Default)]
pub struct Hydrology {
    /// Case d'eau d'un lac.
    pub lake: Vec<bool>,
    /// Altitude de la surface du lac (m) pour les cases de lac.
    pub lake_surface: Vec<f32>,
    /// Petit lac dans une case de terre.
    pub small_lake: Vec<bool>,
    /// Le cours d'eau occupe la case (fleuve).
    pub fleuve: Vec<bool>,
    /// Débit sortant de la case (m³/s), tous cours d'eau confondus.
    pub discharge: Vec<f32>,
    /// Direction (0 à 5, voir `hex::DIRECTIONS`) vers la case aval.
    pub downstream: Vec<Option<u8>>,
    /// Directions vers lesquelles le fleuve de la case touche l'arête (aval, affluents
    /// fleuves, lac qui se déverse dans la case) : bits 0 à 5.
    pub fleuve_links: Vec<u8>,
    /// Débit de la rivière sur chaque arête (m³/s, 0 sans rivière), le même vu des deux cases.
    pub edge_flow: Vec<[f32; 6]>,
    /// Bilan (m³/s) : ruissellement total, eau arrivée à la mer, évaporée par les lacs,
    /// perdue dans les lits (contrôle de la conservation).
    pub total_runoff: f32,
    pub to_sea: f32,
    pub lake_evaporation: f32,
    pub bed_losses: f32,
}

impl Hydrology {
    /// Aucune eau courante (carte faite à la main).
    pub fn empty(n: usize) -> Self {
        Self {
            lake: vec![false; n],
            lake_surface: vec![0.0; n],
            small_lake: vec![false; n],
            fleuve: vec![false; n],
            discharge: vec![0.0; n],
            downstream: vec![None; n],
            fleuve_links: vec![0; n],
            edge_flow: vec![[0.0; 6]; n],
            ..Default::default()
        }
    }
}

/// Grille rectangulaire (coordonnées offset odd-r, comme `WorldMap`).
#[derive(Clone, Copy)]
struct Grid {
    w: i32,
    h: i32,
}

impl Grid {
    fn index(self, hex: Hex) -> Option<usize> {
        let (col, row) = hex.to_offset();
        (col >= 0 && col < self.w && row >= 0 && row < self.h).then(|| (row * self.w + col) as usize)
    }
    fn hex(self, i: usize) -> Hex {
        Hex::from_offset(i as i32 % self.w, i as i32 / self.w)
    }
    /// Voisines dans la carte : (direction, indice).
    fn neighbors(self, i: usize) -> impl Iterator<Item = (usize, usize)> {
        let hex = self.hex(i);
        (0..6).filter_map(move |d| self.index(hex + DIRECTIONS[d]).map(|j| (d, j)))
    }
}

/// Clé de tri croissant d'un flottant (pour le tas de priorité).
fn key(f: f32) -> u32 {
    let b = f.to_bits();
    if b >> 31 == 1 { !b } else { b | 0x8000_0000 }
}

/// File de priorité « le plus bas d'abord », premier arrivé premier servi à égalité (les
/// zones plates s'écoulent ainsi depuis leur bord).
#[derive(Default)]
struct LowestFirst {
    heap: BinaryHeap<Reverse<(u32, u64, usize)>>,
    seq: u64,
}

impl LowestFirst {
    fn push(&mut self, z: f32, i: usize) {
        self.seq += 1;
        self.heap.push(Reverse((key(z), self.seq, i)));
    }
    fn pop(&mut self) -> Option<usize> {
        self.heap.pop().map(|Reverse((_, _, i))| i)
    }
}

/// Une cuvette : cases remplies au même niveau, point le plus bas, exutoire.
struct Pool {
    cells: Vec<usize>,
    pit: usize,
    /// Case hors de la cuvette par où elle déborde, et case de la cuvette qui la touche.
    outlet: usize,
    spill_cell: usize,
    /// Niveau du col (m).
    spill: f32,
}

/// Perte d'eau par infiltration dans le lit sur une longueur `length_m` (m³/s).
fn bed_loss(q: f32, aridity: f32, length_m: f32) -> f32 {
    if q <= 0.0 {
        return 0.0;
    }
    let dryness = (1.0 - aridity / LOSING_BELOW_ARIDITY).clamp(0.0, 1.0);
    let width = WIDTH_PER_SQRT_M3S * q.sqrt();
    (width * length_m * BED_INFILTRATION_M_PER_DAY / 86_400.0 * dryness).min(q)
}

/// Simule l'hydrologie de toute la carte.
pub fn simulate(input: &HydroInput) -> Hydrology {
    let grid = Grid { w: input.width, h: input.height };
    let n = (input.width * input.height) as usize;
    let mut out = Hydrology::empty(n);
    // Surface d'une case hexagonale (m²) et conversions
    let cell_m = input.cell_km * 1000.0;
    let cell_area = 3f32.sqrt() / 2.0 * cell_m * cell_m;
    let per_second = |mm_per_year: f32| mm_per_year / 1000.0 * cell_area / SECONDS_PER_YEAR;
    let land = |i: usize| !input.sea[i];

    // --- 1. Priority-Flood depuis la mer (et le bord de la carte)
    let mut filled = input.elevation.to_vec();
    let mut done = vec![false; n];
    let mut receiver: Vec<Option<usize>> = vec![None; n];
    let mut queue = LowestFirst::default();
    for i in 0..n {
        if input.sea[i] {
            done[i] = true;
            filled[i] = filled[i].min(0.0);
            queue.push(filled[i], i);
        }
    }
    // Carte sans mer (tests) : l'eau s'échappe par la case la plus basse
    if queue.heap.is_empty()
        && let Some(i) = (0..n).min_by(|&a, &b| input.elevation[a].total_cmp(&input.elevation[b]))
    {
        done[i] = true;
        queue.push(filled[i], i);
    }
    let mut raised = vec![false; n];
    while let Some(i) = queue.pop() {
        for (_, j) in grid.neighbors(i) {
            if done[j] {
                continue;
            }
            done[j] = true;
            receiver[j] = Some(i);
            raised[j] = input.elevation[j] < filled[i];
            filled[j] = input.elevation[j].max(filled[i] + EPSILON_M);
            queue.push(filled[j], j);
        }
    }

    // --- 2. Cuvettes : cases remplies voisines au même niveau
    let mut pool_of = vec![usize::MAX; n];
    let mut pools: Vec<Pool> = Vec::new();
    for start in 0..n {
        if !raised[start] || pool_of[start] != usize::MAX {
            continue;
        }
        let id = pools.len();
        let mut cells = vec![start];
        pool_of[start] = id;
        let mut k = 0;
        while k < cells.len() {
            let c = cells[k];
            k += 1;
            for (_, j) in grid.neighbors(c) {
                if raised[j] && pool_of[j] == usize::MAX && (filled[j] - filled[c]).abs() < SAME_POOL_M {
                    pool_of[j] = id;
                    cells.push(j);
                }
            }
        }
        let pit = *cells.iter().min_by(|&&a, &&b| input.elevation[a].total_cmp(&input.elevation[b]).then(a.cmp(&b))).unwrap();
        // Exutoire : la case hors cuvette la plus basse vers laquelle la cuvette s'écoulait
        let (spill_cell, outlet) = cells
            .iter()
            .filter_map(|&c| receiver[c].filter(|&r| pool_of[r] != id).map(|r| (c, r)))
            .min_by(|a, b| filled[a.1].total_cmp(&filled[b.1]).then(a.cmp(b)))
            .expect("une cuvette s'écoule toujours quelque part");
        let spill = filled[outlet].max(input.elevation[outlet]);
        pools.push(Pool { cells, pit, outlet, spill_cell, spill });
    }

    // Dans une cuvette, l'eau descend vers le fond : remplissage depuis le fond, du plus bas au
    // plus haut ; chaque case s'écoule vers celle qui l'a atteinte. L'ordre de remplissage
    // sert aussi à choisir les cases noyées par un lac (les plus basses, d'un seul tenant).
    let mut fill_order: Vec<Vec<usize>> = Vec::with_capacity(pools.len());
    for (id, pool) in pools.iter().enumerate() {
        let mut order = Vec::with_capacity(pool.cells.len());
        let mut q = LowestFirst::default();
        let mut seen: HashMap<usize, ()> = HashMap::from([(pool.pit, ())]);
        q.push(input.elevation[pool.pit], pool.pit);
        while let Some(c) = q.pop() {
            order.push(c);
            for (_, j) in grid.neighbors(c) {
                if pool_of[j] == id && seen.insert(j, ()).is_none() {
                    receiver[j] = Some(c);
                    q.push(input.elevation[j], j);
                }
            }
        }
        receiver[pool.pit] = Some(pool.outlet);
        fill_order.push(order);
    }

    // --- 3. Débits, d'amont en aval (ordre topologique). Deux passes : la première dit quels
    // lacs débordent ; l'eau d'un lac qui déborde traverse ensuite la cuvette, du fond jusqu'au
    // col, de case voisine en case voisine (le fleuve qui la traverse reste continu).
    let mut terminal = vec![false; pools.len()];
    let mut topo = Vec::with_capacity(n);
    // Case où se fait le bilan du lac : là où passe toute l'eau de la cuvette (le fond, puis le
    // col pour un lac qui déborde)
    let mut balance_at: Vec<usize> = pools.iter().map(|p| p.pit).collect();
    for pass in 0..2 {
        if pass == 1 {
            for (id, pool) in pools.iter().enumerate() {
                if terminal[id] {
                    continue;
                }
                // Chemin le plus court du fond au col, à l'intérieur de la cuvette
                let mut toward: HashMap<usize, usize> = HashMap::new();
                let mut front = vec![pool.spill_cell];
                toward.insert(pool.spill_cell, pool.outlet);
                let mut k = 0;
                while k < front.len() {
                    let c = front[k];
                    k += 1;
                    for (_, j) in grid.neighbors(c) {
                        if pool_of[j] == id && !toward.contains_key(&j) {
                            toward.insert(j, c);
                            front.push(j);
                        }
                    }
                }
                balance_at[id] = pool.spill_cell;
                let mut c = pool.pit;
                loop {
                    let next = toward[&c];
                    receiver[c] = Some(next);
                    if c == pool.spill_cell {
                        break;
                    }
                    c = next;
                }
            }
            out = Hydrology { total_runoff: 0.0, ..Hydrology::empty(n) };
            topo.clear();
        }
        let mut donors = vec![0u32; n];
        for i in 0..n {
            if land(i)
                && let Some(r) = receiver[i]
            {
                donors[r] += 1;
            }
        }
        let mut ready: Vec<usize> = (0..n).filter(|&i| land(i) && donors[i] == 0).collect();
        let mut inflow: Vec<f32> = (0..n).map(|i| if land(i) { per_second(input.runoff[i].max(0.0)) } else { 0.0 }).collect();
        out.total_runoff = inflow.iter().sum();
        while let Some(i) = ready.pop() {
            topo.push(i);
            let q = inflow[i];
            let id = pool_of[i];
            let q_out = if id != usize::MAX && balance_at[id] == i {
                // Fond d'une cuvette : bilan du lac
                let pool = &pools[id];
                let evaporation: f32 = pool
                    .cells
                    .iter()
                    .map(|&c| (OPEN_WATER_EVAPORATION * input.pet[c] - input.rain[c]).max(0.0))
                    .sum::<f32>()
                    / pool.cells.len() as f32;
                let e_net = evaporation / 1000.0 / SECONDS_PER_YEAR; // m/s
                let full_area: f32 = pool
                    .cells
                    .iter()
                    .map(|&c| ((pool.spill - input.elevation[c]) / FULL_LAKE_DEPTH_M).clamp(0.0, 1.0) * cell_area)
                    .sum();
                let area = if e_net * full_area <= q { full_area } else { q / e_net };
                let lost = (e_net * area).min(q);
                out.lake_evaporation += lost;
                terminal[id] = area < full_area;
                // Cases noyées : les plus basses, d'un seul tenant autour du fond
                let cells = if area >= LAKE_CELL_SHARE * cell_area { (area / cell_area).round().max(1.0) as usize } else { 0 };
                if cells > 0 {
                    let flooded = &fill_order[id][..cells.min(fill_order[id].len())];
                    let surface = if terminal[id] {
                        flooded.iter().map(|&c| input.elevation[c]).fold(f32::MIN, f32::max).min(pool.spill)
                    } else {
                        pool.spill
                    };
                    for &c in flooded {
                        out.lake[c] = true;
                        out.lake_surface[c] = surface;
                    }
                } else if area >= SMALL_LAKE_KM2 * 1e6 {
                    out.small_lake[pool.pit] = true;
                }
                q - lost
            } else if id != usize::MAX {
                // Dans une cuvette : l'eau rejoint le fond (le lac), sans perte de lit
                q
            } else {
                let length = cell_m;
                let loss = bed_loss(q, input.aridity[i], length);
                out.bed_losses += loss;
                q - loss
            };
            out.discharge[i] = q_out;
            if let Some(r) = receiver[i] {
                if land(r) {
                    inflow[r] += q_out;
                    donors[r] -= 1;
                    if donors[r] == 0 {
                        ready.push(r);
                    }
                } else {
                    out.to_sea += q_out;
                }
            } else {
                // Bord de la carte : l'eau sort de la carte (comptée comme rejoignant la mer)
                out.to_sea += q_out;
            }
        }
        debug_assert_eq!(topo.len(), (0..n).filter(|&i| land(i)).count(), "écoulement sans cycle");
    }

    // Direction vers l'aval (voisine), quand elle existe
    for i in 0..n {
        if let Some(r) = receiver[i] {
            let hex = grid.hex(i);
            out.downstream[i] = (0..6).find(|&d| grid.index(hex + DIRECTIONS[d]) == Some(r)).map(|d| d as u8);
        }
    }
    // Un lac plein se déverse par son col : la case de lac qui touche l'exutoire
    for (id, pool) in pools.iter().enumerate() {
        if !terminal[id] && out.lake[pool.spill_cell] {
            let hex = grid.hex(pool.spill_cell);
            out.downstream[pool.spill_cell] = (0..6).find(|&d| grid.index(hex + DIRECTIONS[d]) == Some(pool.outlet)).map(|d| d as u8);
        }
    }

    // --- 4. Fleuves (d'amont en aval)
    let mut fed = vec![false; n];
    for &i in &topo {
        let q = out.discharge[i];
        let is_fleuve = !out.lake[i] && (q >= FLEUVE_M3S || (fed[i] && q >= FLEUVE_M3S * FLEUVE_KEEP));
        if !is_fleuve {
            continue;
        }
        out.fleuve[i] = true;
        if let Some(r) = receiver[i] {
            fed[r] = true;
        }
    }
    for i in 0..n {
        if !out.fleuve[i] {
            continue;
        }
        for (d, j) in grid.neighbors(i) {
            let downstream = out.downstream[i] == Some(d as u8);
            let tributary = out.fleuve[j] && receiver[j] == Some(i);
            let lake_spill = out.lake[j] && out.downstream[j].is_some_and(|dj| grid.index(grid.hex(j) + DIRECTIONS[dj as usize]) == Some(i));
            // L'aval peut être la mer ou un lac : le fleuve va jusqu'à l'arête
            if (downstream && (out.fleuve[j] || input.sea[j] || out.lake[j])) || tributary || lake_spill {
                out.fleuve_links[i] |= 1 << d;
            }
        }
    }

    // Pas de petit lac dans le lit d'un fleuve
    for i in 0..n {
        out.small_lake[i] &= !out.fleuve[i];
    }

    // --- 5. Rivières sur les arêtes : écoulement de coin en coin
    edge_rivers(input, grid, &mut out, &pools, &terminal);
    out
}

/// Rivières le long des arêtes. Un coin (sommet commun à trois cases) a l'altitude moyenne de
/// ces cases ; le ruissellement de chaque case est réparti entre ses six coins ; l'eau suit la
/// pente de coin en coin (Priority-Flood depuis les exutoires) jusqu'à un exutoire : un coin qui
/// touche la mer, un lac, un fleuve, ou le fond d'une cuvette sans issue.
fn edge_rivers(input: &HydroInput, grid: Grid, out: &mut Hydrology, pools: &[Pool], terminal: &[bool]) {
    let n = (input.width * input.height) as usize;
    let cell_m = input.cell_km * 1000.0;
    let cell_area = 3f32.sqrt() / 2.0 * cell_m * cell_m;
    let edge_m = cell_m / 3f32.sqrt();
    let per_second = |mm_per_year: f32| mm_per_year / 1000.0 * cell_area / SECONDS_PER_YEAR;

    // Coins : le coin j d'une case touche ses voisines des directions j−1 et j
    let corner_cells = |hex: Hex, j: usize| {
        let mut c = [hex, hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]];
        c.sort();
        c
    };
    let mut ids: HashMap<[Hex; 3], usize> = HashMap::new();
    let mut corners: Vec<[Hex; 3]> = Vec::new();
    let mut corner_of = vec![[0usize; 6]; n];
    for i in 0..n {
        let hex = grid.hex(i);
        for j in 0..6 {
            let key = corner_cells(hex, j);
            let id = *ids.entry(key).or_insert_with(|| {
                corners.push(key);
                corners.len() - 1
            });
            corner_of[i][j] = id;
        }
    }
    let m = corners.len();
    // Arêtes entre coins : l'arête d d'une case relie ses coins d et d+1
    let mut adjacency: Vec<Vec<(usize, usize, u8)>> = vec![Vec::new(); m];
    for i in 0..n {
        for d in 0..6 {
            let (a, b) = (corner_of[i][d], corner_of[i][(d + 1) % 6]);
            if !adjacency[a].iter().any(|&(x, _, _)| x == b) {
                adjacency[a].push((b, i, d as u8));
                adjacency[b].push((a, i, d as u8));
            }
        }
    }

    // Exutoires : mer, fleuves, lacs terminaux (cases d'eau ou petit lac) et fonds de cuvettes
    // sans issue, où l'eau s'arrête. Un lac qui déborde laisse passer l'eau (elle le traverse
    // et repart par son col).
    let mut sink_cell: Vec<bool> = (0..n).map(|i| input.sea[i] || out.fleuve[i]).collect();
    for (pool, &t) in pools.iter().zip(terminal) {
        if t {
            sink_cell[pool.pit] = true;
            for &c in &pool.cells {
                sink_cell[c] |= out.lake[c];
            }
        }
    }
    let cell_index = |h: Hex| grid.index(h);
    let corner_z: Vec<f32> = corners
        .iter()
        .map(|c| {
            let zs: Vec<f32> = c.iter().filter_map(|&h| cell_index(h)).map(|i| input.elevation[i].max(0.0)).collect();
            zs.iter().sum::<f32>() / zs.len().max(1) as f32
        })
        .collect();
    let is_sink = |c: &[Hex; 3]| c.iter().any(|&h| cell_index(h).is_none_or(|i| sink_cell[i]));

    // Priority-Flood sur les coins
    let mut receiver: Vec<Option<(usize, usize, u8)>> = vec![None; m];
    let mut done = vec![false; m];
    let mut filled = corner_z.clone();
    let mut queue = LowestFirst::default();
    for c in 0..m {
        if is_sink(&corners[c]) {
            done[c] = true;
            queue.push(filled[c], c);
        }
    }
    let mut order = Vec::with_capacity(m);
    while let Some(c) = queue.pop() {
        order.push(c);
        for &(k, cell, d) in &adjacency[c] {
            if done[k] {
                continue;
            }
            done[k] = true;
            receiver[k] = Some((c, cell, d));
            filled[k] = corner_z[k].max(filled[c] + EPSILON_M);
            queue.push(filled[k], k);
        }
    }

    // Ruissellement des cases de terre (hors fleuves et lacs) réparti sur leurs coins
    let mut q = vec![0f32; m];
    let mut basin = vec![0f32; m];
    for i in 0..n {
        if input.sea[i] || out.fleuve[i] || out.lake[i] {
            continue;
        }
        let share = per_second(input.runoff[i].max(0.0)) / 6.0;
        for j in 0..6 {
            q[corner_of[i][j]] += share;
            basin[corner_of[i][j]] += 1.0 / 6.0;
        }
    }
    // D'amont en aval : ordre inverse du remplissage
    for &c in order.iter().rev() {
        let Some((r, cell, d)) = receiver[c] else { continue };
        let neighbor = grid.index(grid.hex(cell) + DIRECTIONS[d as usize]);
        let aridity = match neighbor {
            Some(j) => (input.aridity[cell] + input.aridity[j]) / 2.0,
            None => input.aridity[cell],
        };
        let flow = q[c] - bed_loss(q[c], aridity, edge_m);
        q[r] += flow;
        basin[r] += basin[c];
        // Rivière tracée entre deux cases de terre (ni mer, ni lac, ni fleuve)
        let Some(j) = neighbor else { continue };
        let plain = |i: usize| !input.sea[i] && !out.lake[i] && !out.fleuve[i];
        if flow >= RIVER_M3S && basin[c] >= RIVER_MIN_BASIN_CELLS && plain(cell) && plain(j) {
            out.edge_flow[cell][d as usize] = flow;
            out.edge_flow[j][(d as usize + 3) % 6] = flow;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Terrain artificiel : une pente douce vers la mer à l'ouest, pluie uniforme.
    fn slope(w: i32, h: i32, runoff: f32) -> (Vec<f32>, Vec<bool>, Vec<f32>) {
        let mut z = Vec::new();
        let mut sea = Vec::new();
        for row in 0..h {
            for col in 0..w {
                let _ = row;
                sea.push(col == 0);
                z.push(if col == 0 { 0.0 } else { col as f32 * 50.0 });
            }
        }
        let n = (w * h) as usize;
        (z, sea, vec![runoff; n])
    }

    fn run(w: i32, h: i32, z: &[f32], sea: &[bool], runoff: &[f32], aridity: f32) -> Hydrology {
        let n = (w * h) as usize;
        let rain = vec![1000.0; n];
        let pet = vec![900.0; n];
        let ar = vec![aridity; n];
        simulate(&HydroInput {
            width: w,
            height: h,
            cell_km: 40.0,
            elevation: z,
            sea,
            runoff,
            rain: &rain,
            pet: &pet,
            aridity: &ar,
        })
    }

    #[test]
    fn conservation_de_l_eau() {
        let (z, sea, runoff) = slope(30, 12, 300.0);
        let h = run(30, 12, &z, &sea, &runoff, 1.0);
        let balance = h.to_sea + h.lake_evaporation + h.bed_losses;
        assert!((balance - h.total_runoff).abs() < h.total_runoff * 1e-3, "{balance} ≠ {}", h.total_runoff);
        assert!(h.bed_losses == 0.0, "pas de pertes en climat humide");
    }

    #[test]
    fn l_eau_descend_vers_la_mer_et_grossit() {
        let (z, sea, runoff) = slope(30, 12, 300.0);
        let h = run(30, 12, &z, &sea, &runoff, 1.0);
        // Le débit augmente vers l'aval le long d'un chemin
        let g = Grid { w: 30, h: 12 };
        let mut i = g.index(Hex::from_offset(29, 6)).unwrap();
        let mut last = 0.0;
        while let Some(d) = h.downstream[i] {
            assert!(h.discharge[i] >= last);
            last = h.discharge[i];
            let next = g.index(g.hex(i) + DIRECTIONS[d as usize]).unwrap();
            if sea_at(&sea, next) {
                break;
            }
            i = next;
        }
        assert!(last > 0.0);
    }

    fn sea_at(sea: &[bool], i: usize) -> bool {
        sea[i]
    }

    #[test]
    fn cuvette_humide_deborde_et_cuvette_seche_reste_terminale() {
        // Cuvette au milieu de la pente
        let (mut z, sea, runoff) = slope(30, 12, 300.0);
        let g = Grid { w: 30, h: 12 };
        let pit = g.index(Hex::from_offset(15, 6)).unwrap();
        z[pit] = 100.0;
        let humid = run(30, 12, &z, &sea, &runoff, 1.0);
        assert!(humid.lake[pit], "la cuvette se remplit");
        let balance = humid.to_sea + humid.lake_evaporation + humid.bed_losses;
        assert!((balance - humid.total_runoff).abs() < humid.total_runoff * 1e-3);
        // Climat sec : très peu de ruissellement, forte évaporation → lac terminal ou petit lac
        let dry_runoff = vec![2.0; 360];
        let n = 360;
        let dry = simulate(&HydroInput {
            width: 30,
            height: 12,
            cell_km: 40.0,
            elevation: &z,
            sea: &sea,
            runoff: &dry_runoff,
            rain: &vec![100.0; n],
            pet: &vec![2500.0; n],
            aridity: &vec![0.05; n],
        });
        assert!(!dry.lake[pit]);
        let balance = dry.to_sea + dry.lake_evaporation + dry.bed_losses;
        assert!((balance - dry.total_runoff).abs() < dry.total_runoff * 1e-3);
    }

    #[test]
    fn les_rivieres_d_arete_sont_symetriques_et_entre_terres() {
        let (z, sea, runoff) = slope(40, 20, 600.0);
        let h = run(40, 20, &z, &sea, &runoff, 1.0);
        let g = Grid { w: 40, h: 20 };
        let mut count = 0;
        for i in 0..h.edge_flow.len() {
            for d in 0..6 {
                if h.edge_flow[i][d] > 0.0 {
                    count += 1;
                    let j = g.index(g.hex(i) + DIRECTIONS[d]).unwrap();
                    assert_eq!(h.edge_flow[j][(d + 3) % 6], h.edge_flow[i][d]);
                    assert!(!sea[i] && !sea[j] && !h.fleuve[i] && !h.fleuve[j]);
                }
            }
        }
        assert!(count > 0, "des rivières se forment");
    }

    #[test]
    fn fleuve_continu_jusqu_a_la_mer() {
        // Grand bassin très arrosé : un fleuve se forme et rejoint la mer de case en case
        let (z, sea, runoff) = slope(60, 30, 2500.0);
        let h = run(60, 30, &z, &sea, &runoff, 1.0);
        let g = Grid { w: 60, h: 30 };
        let fleuves: Vec<usize> = (0..h.fleuve.len()).filter(|&i| h.fleuve[i]).collect();
        assert!(!fleuves.is_empty());
        for &i in &fleuves {
            let d = h.downstream[i].expect("un fleuve s'écoule");
            let j = g.index(g.hex(i) + DIRECTIONS[d as usize]).unwrap();
            assert!(h.fleuve[j] || sea[j] || h.lake[j], "le fleuve continue");
            assert!(h.fleuve_links[i] & (1 << d) != 0);
        }
    }

    #[test]
    fn le_desert_assèche_les_petits_cours_d_eau() {
        let (z, sea, runoff) = slope(30, 12, 300.0);
        let wet = run(30, 12, &z, &sea, &runoff, 1.0);
        let dry = run(30, 12, &z, &sea, &runoff, 0.05);
        assert!(dry.bed_losses > 0.0);
        assert!(dry.to_sea < wet.to_sea);
    }
}
