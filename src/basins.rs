//! Bassins versants et noms des grands cours d'eau (logique pure, calculée à la génération).
//!
//! Bassin d'une case : la case où aboutit son eau en suivant la pente (dernière case de terre
//! avant la mer ou le bord de la carte, ou fond d'un lac sans issue). Toutes les cases qui
//! aboutissent au même endroit forment un bassin.
//!
//! Noms : chaque fleuve est découpé en tronçons, du plus fort débit vers l'amont (le cours
//! principal remonte toujours l'affluent le plus abondant ; un affluent assez long porte son
//! propre nom). Les noms eux-mêmes sont donnés ensuite (voir toponymy.rs), d'après la langue et
//! le caractère des lieux.

use crate::hex::{DIRECTIONS, Hex, Wrap};
use crate::hydro::RiverSegment;
use std::collections::HashMap;

/// Un affluent de fleuve d'au moins autant de cases est un « long affluent » (nommé de plus loin).
pub const MIN_NAMED_CELLS: usize = 3;
/// Un fleuve garde son nom à travers un lac d'au plus autant de cases.
pub const PASS_LAKE_CELLS: usize = 2;
/// Une étendue de mer fermée (sans contact avec le bord de la carte) d'au moins autant de cases est
/// une mer intérieure.
pub const MIN_INLAND_SEA_CELLS: usize = 3;
/// … et d'au plus autant de cases : sur une planète aux pôles couverts de terre, l'océan ne touche
/// aucun bord de la carte, ce n'est pas pour autant une mer intérieure.
pub const MAX_INLAND_SEA_CELLS: usize = 800;
/// Coins d'une case qui doivent envoyer leur eau vers un autre bassin pour qu'elle le dessert
/// (moitié : hachurée) ; au-delà, la case appartient à cet autre bassin.
pub const SHARED_MIN_CORNERS: usize = 3;
/// Nombre de couleurs vives (grands bassins, ceux d'un fleuve nommé) et sobres (les autres).
pub const VIVID_COLORS: usize = 12;
pub const MUTED_COLORS: usize = 4;

/// Ce que la simulation hydrologique fournit.
pub struct BasinInput<'a> {
    pub width: i32,
    pub height: i32,
    /// Direction de la case aval (rien : mer, bord, fond d'un lac sans issue).
    pub downstream: &'a [Option<u8>],
    /// Débit sortant de chaque case (m³/s).
    pub discharge: &'a [f32],
    pub fleuve: &'a [bool],
    /// Case de mer (n'appartient à aucun bassin).
    pub sea: &'a [bool],
    /// Case de lac.
    pub lake: &'a [bool],
    /// Tronçons de rivière d'arête (avec leur sens).
    pub segments: &'a [RiverSegment],
    /// Coin exutoire de chaque coin de chaque case (écoulement des rivières), et cases autour de
    /// chaque coin.
    pub corner_outlet: &'a [[u32; 6]],
    pub corner_cells: &'a [[u32; 3]],
    /// Latitude (degrés) de chaque ligne.
    pub latitude: &'a dyn Fn(i32) -> f32,
    /// Tour d'est en ouest (planète) : pas de bord à l'est ni à l'ouest.
    pub wrap: Wrap,
}

/// Un bassin versant.
#[derive(Clone, Debug, PartialEq)]
pub struct Basin {
    /// Case où aboutit son eau.
    pub outlet: usize,
    pub cells: u32,
    /// Débit à son exutoire (m³/s).
    pub outflow: f32,
    /// Fleuve nommé principal (le plus abondant du bassin).
    pub river: Option<usize>,
    /// Bassin fermé : son eau finit dans un lac sans issue (ni mer, ni bord de la carte) ou une
    /// mer intérieure.
    pub closed: bool,
    /// Lac salé ou mer intérieure nommés où finit l'eau d'un bassin fermé : il donne son nom au
    /// bassin (le fleuve qui s'y jette n'en est qu'un affluent).
    pub lake: Option<usize>,
    /// Couleur sur la carte des bassins : 0 à `VIVID_COLORS - 1` pour un grand bassin (avec un
    /// fleuve nommé), puis `MUTED_COLORS` teintes sobres pour les autres.
    pub color: usize,
}

/// Point du tracé d'un cours d'eau : l'un des coins d'une case (`corner` 0 à 5), son centre
/// (`CENTER`) ou le milieu de l'une de ses arêtes (`EDGE` + direction).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoursePoint {
    pub cell: usize,
    pub corner: u8,
}

impl CoursePoint {
    pub const CENTER: u8 = 6;
    pub const EDGE: u8 = 7;
}

/// Nature d'une étendue d'eau nommée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterBody {
    /// Lac qui se déverse (eau douce).
    FreshLake,
    /// Lac sans issue : l'eau s'en évapore (lac salé).
    SaltLake,
    /// Étendue de mer fermée au milieu des terres (salée).
    InlandSea,
}

impl WaterBody {
    /// Nature en quelques mots (infobulle).
    pub fn short(self) -> &'static str {
        match self {
            WaterBody::FreshLake => "lac d'eau douce",
            WaterBody::SaltLake => "lac salé",
            WaterBody::InlandSea => "mer intérieure",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            WaterBody::FreshLake => "lac d'eau douce (il se déverse)",
            WaterBody::SaltLake => "lac salé (sans issue, l'eau s'évapore)",
            WaterBody::InlandSea => "mer intérieure salée",
        }
    }
}

/// Un cours d'eau nommé : tronçon de fleuve (cases) ou de rivière (arêtes).
#[derive(Clone, Debug, PartialEq)]
pub struct NamedRiver {
    /// Nom (vide tant que la toponymie ne l'a pas donné).
    pub name: String,
    /// Genre du nom (« la Vaska », « le Taldor »).
    pub feminine: bool,
    /// Origine du nom propre (rien pour un nom descriptif : « Rivière Noire »).
    pub etymology: Option<crate::toponymy::Etymology>,
    /// Fleuve (dans les cases) ou rivière (sur les arêtes).
    pub fleuve: bool,
    /// Importance pour l'affichage : 0 long cours principal d'un fleuve, 1 long affluent de
    /// fleuve (au moins `MIN_NAMED_CELLS` cases), 2 tout le reste (tronçons courts, rivières).
    pub tier: u8,
    /// Emplacements le long du cours, d'aval en amont : une case de fleuve (a = b) ou le milieu
    /// de l'arête entre deux cases (rivière).
    pub spots: Vec<(usize, usize)>,
    /// Tracé, d'aval en amont (petits lacs traversés compris).
    pub course: Vec<CoursePoint>,
    /// Petits bras qui portent son nom (tronçons courts qui s'y jettent), chacun d'aval en amont,
    /// partant du milieu de son lit.
    pub arms: Vec<Vec<CoursePoint>>,
    /// Rang par rapport à son déversoir final : 0 se jette dans la mer (ou finit dans un lac salé,
    /// une mer intérieure), 1 affluent d'un cours de rang 0, etc. Le principal cours qui entre
    /// dans un lac garde le rang du lac.
    pub level: u8,
    /// Cours d'eau nommé dans lequel il se jette.
    pub flows_into: Option<usize>,
    /// Lac nommé dans lequel il se jette.
    pub into_lake: Option<usize>,
    /// Débit à son aval (m³/s).
    pub discharge: f32,
    pub latitude: f32,
}

impl NamedRiver {
    /// Case la plus en aval (pour un fleuve, la case de son embouchure ou de son confluent).
    pub fn first_cell(&self) -> usize {
        self.spots[0].0
    }
}

/// Un lac ou une mer intérieure nommés.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedLake {
    /// Nom complet (« Lac Kaldvik », « Mer d'Ilmar », « Lac Gelé »).
    pub name: String,
    pub feminine: bool,
    pub etymology: Option<crate::toponymy::Etymology>,
    pub kind: WaterBody,
    /// Cases du lac, de la plus proche de son centre à la plus lointaine.
    pub cells: Vec<usize>,
    /// Importance pour l'affichage (comme les cours d'eau) : 0 très grand, 1 grand, 2 moyen.
    pub tier: u8,
    pub latitude: f32,
    /// Cours d'eau nommé qui sort du lac (déversoir).
    pub outflow: Option<usize>,
    /// Rang par rapport au déversoir final : celui du cours d'eau qui en sort (0 sans issue).
    pub level: u8,
}

/// Bassins versants et fleuves nommés d'une carte.
#[derive(Clone, Debug, Default)]
pub struct Basins {
    /// Bassin de chaque case (rien pour la mer).
    pub basin_of: Vec<Option<u32>>,
    pub basins: Vec<Basin>,
    /// Fleuve nommé qui occupe la case.
    pub river_of: Vec<Option<u16>>,
    /// Rivière nommée sur une arête (case, direction), vue des deux cases.
    pub river_on_edge: HashMap<(u32, u8), u16>,
    /// Cours d'eau nommés : fleuves (cours principaux, longs affluents, autres tronçons), puis
    /// rivières, chacun par débit décroissant.
    pub rivers: Vec<NamedRiver>,
    /// Lacs et mers intérieures nommés, du plus grand au plus petit, et celui de chaque case.
    pub lakes: Vec<NamedLake>,
    pub lake_of: Vec<Option<u16>>,
    /// Autre bassin que dessert une case : au moins `SHARED_MIN_CORNERS` de ses coins y envoient
    /// leur eau (par les rivières des arêtes). Hachurée sur la carte des bassins.
    pub basin_also: Vec<Option<u32>>,
}

impl Basin {
    /// Grand bassin : celui d'un fleuve nommé ou d'un lac salé, d'une mer intérieure nommés.
    pub fn major(&self) -> bool {
        self.river.is_some() || self.lake.is_some()
    }
}

impl Basins {
    /// Nom d'un bassin : le lac ou la mer intérieure où il finit, sinon son fleuve principal.
    pub fn basin_name(&self, basin: &Basin) -> Option<&str> {
        basin.lake.map(|l| self.lakes[l].name.as_str()).or(basin.river.map(|r| self.rivers[r].name.as_str()))
    }

    /// Titre d'un bassin : « Bassin de la Vaska », « Bassin du Lac Gelé », « Bassin de la Mer
    /// d'Ilmar ».
    pub fn basin_title(&self, basin: &Basin) -> Option<String> {
        let (name, fem) = match (basin.lake, basin.river) {
            (Some(l), _) => (&self.lakes[l].name, self.lakes[l].feminine),
            (None, Some(r)) => (&self.rivers[r].name, self.rivers[r].feminine),
            _ => return None,
        };
        Some(format!("Bassin {}", crate::toponymy::de(name, fem, true)))
    }

    pub fn empty(n: usize) -> Self {
        Self { basin_of: vec![None; n], river_of: vec![None; n], lake_of: vec![None; n], basin_also: vec![None; n], ..Default::default() }
    }

    pub fn basin(&self, cell: usize) -> Option<&Basin> {
        self.basin_of.get(cell).copied().flatten().map(|b| &self.basins[b as usize])
    }

    pub fn river(&self, cell: usize) -> Option<&NamedRiver> {
        self.river_of.get(cell).copied().flatten().map(|r| &self.rivers[r as usize])
    }

    /// Lac nommé de la case.
    pub fn lake(&self, cell: usize) -> Option<&NamedLake> {
        self.lake_of.get(cell).copied().flatten().map(|l| &self.lakes[l as usize])
    }

    /// Rivière nommée sur l'arête `dir` de la case.
    pub fn river_on_edge(&self, cell: usize, dir: usize) -> Option<&NamedRiver> {
        self.river_on_edge.get(&(cell as u32, dir as u8)).map(|&r| &self.rivers[r as usize])
    }
}

/// Rangs par rapport au déversoir final (voir `NamedRiver::level`).
fn set_levels(rivers: &mut [NamedRiver], lakes: &mut [NamedLake]) {
    // Principal cours qui entre dans chaque lac : il continue le lac
    let mut main_inflow: Vec<Option<usize>> = vec![None; lakes.len()];
    for (r, river) in rivers.iter().enumerate() {
        if river.flows_into.is_none()
            && let Some(l) = river.into_lake
            && main_inflow[l].is_none_or(|m| rivers[m].discharge < river.discharge)
        {
            main_inflow[l] = Some(r);
        }
    }
    #[derive(Clone, Copy)]
    enum F {
        R(usize),
        L(usize),
    }
    const UNKNOWN: u8 = u8::MAX;
    let mut river_level = vec![UNKNOWN; rivers.len()];
    let mut lake_level = vec![UNKNOWN; lakes.len()];
    for start in 0..rivers.len() {
        // On descend jusqu'à un rang connu (ou au déversoir), puis on remonte le chemin
        let mut path: Vec<(F, u8)> = Vec::new();
        let mut f = F::R(start);
        let mut base = loop {
            let known = match f {
                F::R(r) => river_level[r],
                F::L(l) => lake_level[l],
            };
            if known != UNKNOWN {
                break known;
            }
            // Garde-fou contre une boucle
            if path.len() > rivers.len() + lakes.len() {
                break 0;
            }
            let (next, step) = match f {
                F::R(r) => match (rivers[r].flows_into, rivers[r].into_lake) {
                    (Some(x), _) => (Some(F::R(x)), 1),
                    (None, Some(l)) => (Some(F::L(l)), u8::from(main_inflow[l] != Some(r))),
                    _ => (None, 0),
                },
                F::L(l) => (lakes[l].outflow.map(F::R), 0),
            };
            path.push((f, step));
            match next {
                Some(n) => f = n,
                None => break 0,
            }
        };
        // On remonte en ajoutant les pas
        for &(g, step) in path.iter().rev() {
            // `step` : pas entre g et son aval (0 pour le dernier élément sans aval)
            base = base.saturating_add(step).min(UNKNOWN - 1);
            match g {
                F::R(r) => river_level[r] = base,
                F::L(l) => lake_level[l] = base,
            }
        }
    }
    for (l, lake) in lakes.iter_mut().enumerate() {
        // Lac où n'entre aucun cours nommé : rang du cours qui en sort
        lake.level = if lake_level[l] != UNKNOWN { lake_level[l] } else { lake.outflow.map_or(0, |r| river_level[r]) };
    }
    for (r, river) in rivers.iter_mut().enumerate() {
        river.level = river_level[r];
    }
}

/// Calcule les bassins et nomme les fleuves.
pub fn compute(input: &BasinInput) -> Basins {
    let (w, h) = (input.width, input.height);
    let n = (w * h) as usize;
    let hex = |i: usize| Hex::from_offset(i as i32 % w, i as i32 / w);
    let wrap = input.wrap;
    let index = |x: Hex| {
        let (c, r) = wrap.hex(x).to_offset();
        (c >= 0 && c < w && r >= 0 && r < h).then(|| (r * w + c) as usize)
    };
    // Case aval (terre ou lac) ; rien quand l'eau quitte la terre
    let next = |i: usize| -> Option<usize> {
        let d = input.downstream[i]?;
        let j = index(hex(i) + DIRECTIONS[d as usize])?;
        (!input.sea[j]).then_some(j)
    };

    // --- Exutoire de chaque case (en remontant un chemin puis en le marquant d'un coup)
    let mut outlet: Vec<Option<usize>> = vec![None; n];
    let mut path = Vec::new();
    for start in 0..n {
        if input.sea[start] || outlet[start].is_some() {
            continue;
        }
        path.clear();
        let mut i = start;
        let end = loop {
            if let Some(o) = outlet[i] {
                break o;
            }
            path.push(i);
            match next(i) {
                // Garde-fou contre une boucle (ne devrait pas arriver)
                Some(j) if path.len() <= n => i = j,
                _ => break i,
            }
        };
        for &k in &path {
            outlet[k] = Some(end);
        }
    }

    // --- Bassins
    let mut id_of_outlet = std::collections::HashMap::new();
    let mut basins: Vec<Basin> = Vec::new();
    let mut basin_of = vec![None; n];
    for i in 0..n {
        if let Some(o) = outlet[i] {
            let id = *id_of_outlet.entry(o).or_insert_with(|| {
                let (c, r) = (o as i32 % w, o as i32 / w);
                let on_edge = r == 0 || r == h - 1 || (!wrap.wraps() && (c == 0 || c == w - 1));
                let closed = input.downstream[o].is_none() && !on_edge;
                basins.push(Basin { outlet: o, cells: 0, outflow: input.discharge[o], river: None, closed, lake: None, color: 0 });
                basins.len() - 1
            });
            basins[id].cells += 1;
            basin_of[i] = Some(id as u32);
        }
    }

    // --- Lacs : groupes de cases de lac voisines
    let mut lake_group: Vec<Option<usize>> = vec![None; n];
    let mut lake_groups: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if !input.lake[start] || lake_group[start].is_some() {
            continue;
        }
        let id = lake_groups.len();
        let mut group = vec![start];
        lake_group[start] = Some(id);
        let mut k = 0;
        while k < group.len() {
            for nb in hex(group[k]).neighbors() {
                if let Some(j) = index(nb)
                    && input.lake[j]
                    && lake_group[j].is_none()
                {
                    lake_group[j] = Some(id);
                    group.push(j);
                }
            }
            k += 1;
        }
        lake_groups.push(group);
    }
    let small_lake = |i: usize| lake_group[i].is_some_and(|g| lake_groups[g].len() <= PASS_LAKE_CELLS);

    // --- Tronçons de fleuve, depuis les embouchures : on prend le tronçon libre le plus abondant
    // dont l'aval est déjà rangé (ou qui finit dans la mer, un grand lac…), et on le remonte en
    // suivant à chaque fois l'affluent le plus abondant ; un petit lac est traversé. Ainsi un
    // fleuve garde un seul nom de l'embouchure à sa source, même si son débit baisse vers l'aval
    // (pertes du lit en climat sec, évaporation d'un lac).
    let passable = |i: usize| input.fleuve[i] || small_lake(i);
    let mut donors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        if !input.sea[i]
            && let Some(j) = next(i)
        {
            donors[j].push(i);
        }
    }
    let key = |i: usize| (input.discharge[i].max(0.0).to_bits(), std::cmp::Reverse(i));
    let mut heap: std::collections::BinaryHeap<((u32, std::cmp::Reverse<usize>), usize)> =
        (0..n).filter(|&i| passable(i) && !next(i).is_some_and(passable)).map(|i| (key(i), i)).collect();
    let mut branch_of: Vec<Option<usize>> = vec![None; n];
    // Tronçon : cases traversées d'aval en amont (fleuve et petits lacs)
    let mut branches: Vec<Vec<usize>> = Vec::new();
    while let Some((_, start)) = heap.pop() {
        if branch_of[start].is_some() {
            continue;
        }
        let id = branches.len();
        let mut cells = vec![start];
        branch_of[start] = Some(id);
        let mut cur = start;
        loop {
            let ups: Vec<usize> = donors[cur].iter().copied().filter(|&d| passable(d) && branch_of[d].is_none()).collect();
            let Some(&up) = ups.iter().max_by_key(|&&d| key(d)) else { break };
            for &other in ups.iter().filter(|&&d| d != up) {
                heap.push((key(other), other));
            }
            branch_of[up] = Some(id);
            cells.push(up);
            cur = up;
        }
        branches.push(cells);
    }
    let fleuve_cells = |b: &[usize]| -> Vec<usize> { b.iter().copied().filter(|&c| input.fleuve[c]).collect() };
    // Tronçon récepteur (celui où se jette le tronçon), s'il y en a un
    let receiver = |b: &[usize]| next(b[0]).and_then(|j| branch_of[j]);
    // Importance : long cours principal, long affluent, court
    let fleuve_tier = |b: &[usize]| -> u8 {
        if fleuve_cells(b).len() < MIN_NAMED_CELLS {
            2
        } else if next(b[0]).is_none() {
            0
        } else {
            1
        }
    };
    // Un tronçon court qui se jette dans un autre en prend le nom (un petit bras ne mérite pas
    // un nom à lui) ; on remonte jusqu'au premier tronçon nommable
    let mut merged_into: Vec<Option<usize>> = vec![None; branches.len()];
    for b in 0..branches.len() {
        if fleuve_cells(&branches[b]).len() < MIN_NAMED_CELLS {
            merged_into[b] = receiver(&branches[b]);
        }
    }
    let root = |mut b: usize| {
        while let Some(r) = merged_into[b] {
            b = r;
        }
        b
    };
    let mut order: Vec<usize> = (0..branches.len()).filter(|&b| merged_into[b].is_none() && !fleuve_cells(&branches[b]).is_empty()).collect();
    // (débit de la première case de fleuve du tronçon : celui qu'on retient pour le cours nommé,
    // le tronçon pouvant commencer par un petit lac)
    order.sort_by_key(|&b| (fleuve_tier(&branches[b]), std::cmp::Reverse(key(fleuve_cells(&branches[b])[0]).0)));

    // --- Tronçons de rivière : même découpage sur les arêtes, de coin en coin, depuis l'aval
    let seg = input.segments;
    let mut into: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut leaving: HashMap<u32, usize> = HashMap::new();
    for (k, x) in seg.iter().enumerate() {
        into.entry(x.down).or_default().push(k);
        leaving.insert(x.up, k);
    }
    let seg_key = |k: usize| (seg[k].flow.max(0.0).to_bits(), std::cmp::Reverse(k));
    let mut seg_heap: std::collections::BinaryHeap<((u32, std::cmp::Reverse<usize>), usize)> =
        (0..seg.len()).filter(|&k| !leaving.contains_key(&seg[k].down)).map(|k| (seg_key(k), k)).collect();
    let mut stream_of = vec![None; seg.len()];
    let mut streams: Vec<Vec<usize>> = Vec::new();
    while let Some((_, start)) = seg_heap.pop() {
        if stream_of[start].is_some() {
            continue;
        }
        let id = streams.len();
        let mut branch = vec![start];
        stream_of[start] = Some(id);
        let mut cur = start;
        loop {
            let ups: Vec<usize> = into.get(&seg[cur].up).into_iter().flatten().copied().filter(|&k| stream_of[k].is_none()).collect();
            let Some(&up) = ups.iter().max_by_key(|&&k| seg_key(k)) else { break };
            for &other in ups.iter().filter(|&&k| k != up) {
                seg_heap.push((seg_key(other), other));
            }
            stream_of[up] = Some(id);
            branch.push(up);
            cur = up;
        }
        streams.push(branch);
    }
    let edge_cells = |k: usize| -> (usize, usize) {
        let x = seg[k];
        let a = x.cell as usize;
        (a, index(hex(a) + DIRECTIONS[x.dir as usize]).unwrap_or(a))
    };
    // Cases autour du coin aval d'une arête
    let down_cells = |k: usize| -> Vec<usize> {
        let x = seg[k];
        let (a, j) = (x.cell as usize, x.down_corner as usize);
        std::iter::once(a)
            .chain([hex(a) + DIRECTIONS[(j + 5) % 6], hex(a) + DIRECTIONS[j]].into_iter().filter_map(index))
            .collect()
    };

    // --- Étendues d'eau : lacs (sauf les petits lacs traversés par un fleuve, qui en font
    // partie) et mers intérieures (étendues de mer qui ne touchent pas le bord de la carte)
    let lat_of = |cell: usize| (input.latitude)(cell as i32 / w);
    let on_border = |c: usize| {
        let (col, row) = (c as i32 % w, c as i32 / w);
        row == 0 || row == h - 1 || (!wrap.wraps() && (col == 0 || col == w - 1))
    };
    let mut sea_group: Vec<Option<usize>> = vec![None; n];
    let mut seas: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if !input.sea[start] || sea_group[start].is_some() {
            continue;
        }
        let id = seas.len();
        let mut group = vec![start];
        sea_group[start] = Some(id);
        let mut k = 0;
        while k < group.len() {
            for nb in hex(group[k]).neighbors() {
                if let Some(j) = index(nb)
                    && input.sea[j]
                    && sea_group[j].is_none()
                {
                    sea_group[j] = Some(id);
                    group.push(j);
                }
            }
            k += 1;
        }
        seas.push(group);
    }
    let inland: Vec<bool> =
        seas.iter().map(|g| (MIN_INLAND_SEA_CELLS..=MAX_INLAND_SEA_CELLS).contains(&g.len()) && !g.iter().any(|&c| on_border(c))).collect();
    // Lac traversé par un fleuve : ses cases sont dans un tronçon
    let crossed = |g: usize| {
        lake_groups[g].len() <= PASS_LAKE_CELLS
            && lake_groups[g].iter().any(|&c| branch_of[c].is_some_and(|b| branches[b].iter().any(|&x| input.fleuve[x])))
    };
    // Lac salé : aucune de ses cases ne se déverse hors du lac
    let salty = |g: usize| {
        !lake_groups[g].iter().any(|&c| {
            input.downstream[c].and_then(|d| index(hex(c) + DIRECTIONS[d as usize])).is_some_and(|t| !input.lake[t])
        })
    };
    let mut bodies: Vec<(Vec<usize>, WaterBody)> = (0..lake_groups.len())
        .filter(|&g| !crossed(g))
        .map(|g| (lake_groups[g].clone(), if salty(g) { WaterBody::SaltLake } else { WaterBody::FreshLake }))
        .chain((0..seas.len()).filter(|&g| inland[g]).map(|g| (seas[g].clone(), WaterBody::InlandSea)))
        .collect();
    bodies.sort_by_key(|(cells, _)| (std::cmp::Reverse(cells.len()), cells[0]));
    let mut lakes = Vec::new();
    let mut lake_of = vec![None; n];
    for (group, kind) in bodies {
        // Positions prises du même côté du raccord que la première case
        let near = |c: usize| wrap.nearest(hex(group[0]), hex(c));
        let centroid = group
            .iter()
            .map(|&c| {
                let (x, y) = near(c).to_world();
                bevy::math::Vec2::new(x, y)
            })
            .sum::<bevy::math::Vec2>()
            / group.len() as f32;
        let mut cells = group.clone();
        cells.sort_by(|&a, &b| {
            let d = |c: usize| {
                let (x, y) = near(c).to_world();
                bevy::math::Vec2::new(x, y).distance(centroid)
            };
            d(a).total_cmp(&d(b)).then(a.cmp(&b))
        });
        let latitude = group.iter().map(|&c| lat_of(c)).sum::<f32>() / group.len() as f32;
        let id = lakes.len();
        for &c in &group {
            lake_of[c] = Some(id as u16);
        }
        let tier = if group.len() >= 10 {
            0
        } else if group.len() >= 5 || (kind == WaterBody::InlandSea) {
            1
        } else {
            2
        };
        lakes.push(NamedLake { name: String::new(), feminine: false, etymology: None, kind, cells, tier, latitude, outflow: None, level: 0 });
    }
    let lake_at = |cell: usize| lake_of[cell].map(|l| l as usize);
    // Une mer intérieure ferme les bassins qui s'y jettent ; un bassin fermé prend le nom du lac
    // ou de la mer intérieure où finit son eau
    for basin in &mut basins {
        let o = basin.outlet;
        let into = input.downstream[o].and_then(|d| index(hex(o) + DIRECTIONS[d as usize]));
        let into_inland = into.is_some_and(|t| sea_group[t].is_some_and(|g| inland[g]));
        basin.closed |= into_inland;
        if basin.closed {
            basin.lake = lake_at(o).or(into.and_then(lake_at));
        }
    }

    // --- Noms des cours d'eau
    let mut rivers: Vec<NamedRiver> = Vec::new();
    let mut river_of = vec![None; n];
    let mut river_on_edge = HashMap::new();
    let mut river_of_branch: HashMap<usize, usize> = HashMap::new();
    let direction = |from: usize, to: usize| (0..6).find(|&d| index(hex(from) + DIRECTIONS[d]) == Some(to)).map(|d| d as u8);
    // Tracé d'un fleuve, d'aval en amont, relié à ce qui l'entoure : il part du centre du fleuve
    // où il se jette (ou du bord du lac, de la mer), et remonte jusqu'au bord du lac d'où il sort
    let fleuve_course = |all: &[usize]| -> Vec<CoursePoint> {
        let mut course = Vec::with_capacity(all.len() + 2);
        let first = all[0];
        if let Some(d) = input.downstream[first]
            && let Some(t) = index(hex(first) + DIRECTIONS[d as usize])
        {
            if input.fleuve[t] {
                course.push(CoursePoint { cell: t, corner: CoursePoint::CENTER });
            } else {
                course.push(CoursePoint { cell: first, corner: CoursePoint::EDGE + d });
            }
        }
        course.extend(all.iter().map(|&c| CoursePoint { cell: c, corner: CoursePoint::CENTER }));
        let last = *all.last().unwrap();
        if let Some(&lake) = donors[last].iter().find(|&&d| input.lake[d] && branch_of[d] != branch_of[last])
            && let Some(d) = direction(last, lake)
        {
            course.push(CoursePoint { cell: last, corner: CoursePoint::EDGE + d });
        }
        course
    };
    for &b in &order {
        let all = &branches[b];
        let cells = fleuve_cells(all);
        let spots: Vec<(usize, usize)> = cells.iter().map(|&c| (c, c)).collect();
        let latitude = cells.iter().map(|&c| lat_of(c)).sum::<f32>() / cells.len() as f32;
        river_of_branch.insert(b, rivers.len());
        let course = fleuve_course(all);
        let tier = fleuve_tier(all);
        let discharge = input.discharge[cells[0]];
        rivers.push(NamedRiver { name: String::new(), feminine: false, etymology: None, fleuve: true, tier, spots, course, arms: Vec::new(), level: 0, flows_into: None, into_lake: None, discharge, latitude });
    }
    // Cases de fleuve : nom de leur tronçon, ou de celui où se jette leur petit bras (dont le
    // tracé s'ajoute à celui du cours qui lui donne son nom)
    for (b, all) in branches.iter().enumerate() {
        if let Some(&r) = river_of_branch.get(&root(b)) {
            if merged_into[b].is_some() {
                rivers[r].arms.push(fleuve_course(all));
            }
            // (petits lacs traversés compris : ils font partie du fleuve)
            for &c in all.iter().filter(|&&c| input.fleuve[c] || lake_of[c].is_none()) {
                river_of[c] = Some(r as u16);
            }
        }
    }
    // Où se jettent les fleuves : le tronçon récepteur (nommé), ou un lac nommé
    for (&b, &r) in &river_of_branch {
        let first = branches[b][0];
        rivers[r].flows_into = receiver(&branches[b]).and_then(|rb| river_of_branch.get(&root(rb)).copied());
        rivers[r].into_lake = input.downstream[first].and_then(|d| index(hex(first) + DIRECTIONS[d as usize])).and_then(lake_at);
    }
    let mut stream_river: Vec<(usize, usize)> = Vec::new();
    for (si, branch) in streams.iter().enumerate() {
        // Une arête isolée qui se jette dans une rivière plus grande n'est qu'un raccord
        if branch.len() < 2 && leaving.contains_key(&seg[branch[0]].down) {
            continue;
        }
        let spots: Vec<(usize, usize)> = branch.iter().map(|&k| edge_cells(k)).collect();
        let latitude = spots.iter().map(|&(a, b)| (lat_of(a) + lat_of(b)) / 2.0).sum::<f32>() / spots.len() as f32;
        let id = rivers.len() as u16;
        stream_river.push((si, rivers.len()));
        let mut course = Vec::with_capacity(branch.len() + 2);
        // Une rivière qui se jette dans un fleuve (ou un petit lac qu'il traverse) rejoint le
        // milieu de son lit
        if !leaving.contains_key(&seg[branch[0]].down)
            && let Some(&f) = down_cells(branch[0]).iter().find(|&&c| input.fleuve[c] || (input.lake[c] && river_of[c].is_some()))
        {
            course.push(CoursePoint { cell: f, corner: CoursePoint::CENTER });
        }
        for (i, &k) in branch.iter().enumerate() {
            let x = seg[k];
            let up = if x.down_corner == x.dir { (x.dir + 1) % 6 } else { x.dir };
            if i == 0 {
                course.push(CoursePoint { cell: x.cell as usize, corner: x.down_corner });
            }
            course.push(CoursePoint { cell: x.cell as usize, corner: up });
            let (a, b) = edge_cells(k);
            river_on_edge.insert((a as u32, x.dir), id);
            river_on_edge.insert((b as u32, (x.dir + 3) % 6), id);
        }
        rivers.push(NamedRiver {
            name: String::new(),
            feminine: false,
            etymology: None,
            fleuve: false,
            tier: 2,
            spots,
            course,
            arms: Vec::new(),
            level: 0,
            flows_into: None,
            into_lake: None,
            discharge: seg[branch[0]].flow,
            latitude,
        });
    }
    // Où se jettent les rivières : la rivière qui part de leur coin aval, ou le fleuve, le lac
    // qui touche ce coin
    for &(si, r) in &stream_river {
        let first = streams[si][0];
        let cells = down_cells(first);
        rivers[r].flows_into = leaving
            .get(&seg[first].down)
            .and_then(|&k| river_on_edge.get(&(edge_cells(k).0 as u32, seg[k].dir)).map(|&x| x as usize))
            .or_else(|| cells.iter().find_map(|&c| river_of[c].map(|x| x as usize)));
        rivers[r].into_lake = cells.iter().find_map(|&c| lake_at(c));
    }
    // Rivières qui sortent d'un lac : leur arête la plus en amont part d'un coin du lac
    let mut stream_sources: Vec<(usize, Vec<usize>)> = Vec::new();
    for &(si, r) in &stream_river {
        let x = seg[*streams[si].last().unwrap()];
        let (a, up) = (x.cell as usize, if x.down_corner == x.dir { (x.dir + 1) % 6 } else { x.dir } as usize);
        let around: Vec<usize> = std::iter::once(a)
            .chain([hex(a) + DIRECTIONS[(up + 5) % 6], hex(a) + DIRECTIONS[up]].into_iter().filter_map(index))
            .collect();
        stream_sources.push((r, around));
    }
    // Cours d'eau qui sort de chaque lac nommé : celui de la case où se déverse le lac, sinon la
    // plus abondante des rivières qui en partent (une mer intérieure ne se déverse pas)
    for lake in &mut lakes {
        if lake.kind == WaterBody::InlandSea {
            continue;
        }
        let members: std::collections::HashSet<usize> = lake.cells.iter().copied().collect();
        let from_stream = stream_sources
            .iter()
            .filter(|(_, around)| around.iter().any(|c| members.contains(c)))
            .max_by(|a, b| rivers[a.0].discharge.total_cmp(&rivers[b.0].discharge))
            .map(|(r, _)| *r);
        lake.outflow = lake
            .cells
            .iter()
            .filter_map(|&c| next(c))
            .find(|&j| !input.lake[j])
            .and_then(|j| river_of[j].map(|r| r as usize))
            .or(from_stream);
    }
    // Rang de chaque cours d'eau et lac par rapport à son déversoir final
    set_levels(&mut rivers, &mut lakes);

    // Fleuve principal d'un bassin : son premier fleuve nommé (cours principal, à défaut
    // affluent)
    for (r, river) in rivers.iter().enumerate().filter(|(_, r)| r.fleuve) {
        if let Some(b) = basin_of[river.first_cell()]
            && basins[b as usize].river.is_none()
        {
            basins[b as usize].river = Some(r);
        }
    }

    // --- Cases qui desservent deux bassins : leurs coins envoient l'eau (par les rivières des
    // arêtes) vers un autre bassin que le leur. Si la plupart des coins vont ailleurs, la case
    // rejoint ce bassin (les rivières restent dans leur bassin) ; à moitié, elle dessert les deux.
    let corner_basins = |i: usize, basin_of: &[Option<u32>]| -> Option<(u32, usize)> {
        let own = basin_of[i]?;
        let corners = input.corner_outlet.get(i)?;
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for &sink in corners {
            let Some(cells) = input.corner_cells.get(sink as usize) else { continue };
            // Bassins où arrive l'eau de ce coin : celui du fleuve ou du lac qu'il touche,
            // sinon (côte, fond de cuvette) celui des cases de terre autour ; si le bassin de la
            // case en fait partie, l'eau reste chez elle
            let around: Vec<usize> = cells.iter().filter(|&&c| (c as usize) < n && !input.sea[c as usize]).map(|&c| c as usize).collect();
            let wet: Vec<usize> = around.iter().copied().filter(|&c| input.fleuve[c] || input.lake[c]).collect();
            let candidates = if wet.is_empty() { around } else { wet };
            let basins_here: Vec<u32> = candidates.iter().filter_map(|&c| basin_of[c]).collect();
            if basins_here.is_empty() || basins_here.contains(&own) {
                continue;
            }
            *counts.entry(basins_here[0]).or_default() += 1;
        }
        counts.into_iter().max_by_key(|&(b, c)| (c, std::cmp::Reverse(b)))
    };
    let reassigned: Vec<Option<u32>> = (0..n)
        .map(|i| match corner_basins(i, &basin_of) {
            Some((other, count)) if count > SHARED_MIN_CORNERS && !input.fleuve[i] && !input.lake[i] => Some(other),
            _ => basin_of[i],
        })
        .collect();
    for i in 0..n {
        if reassigned[i] != basin_of[i] {
            basins[basin_of[i].unwrap() as usize].cells -= 1;
            basins[reassigned[i].unwrap() as usize].cells += 1;
        }
    }
    let basin_of = reassigned;
    let basin_also: Vec<Option<u32>> = (0..n)
        .map(|i| {
            let own = basin_of[i]?;
            let (other, count) = corner_basins(i, &basin_of)?;
            // Seulement autour des grands bassins (ceux d'un fleuve nommé)
            let major = |b: u32| basins[b as usize].major();
            (count >= SHARED_MIN_CORNERS && (major(own) || major(other))).then_some(other)
        })
        .collect();

    // --- Couleurs : deux bassins voisins de même catégorie n'ont pas la même couleur
    let mut neighbours: Vec<std::collections::BTreeSet<usize>> = vec![Default::default(); basins.len()];
    for i in 0..n {
        let Some(a) = basin_of[i] else { continue };
        for nb in hex(i).neighbors() {
            if let Some(j) = index(nb)
                && let Some(b) = basin_of[j]
                && a != b
            {
                neighbours[a as usize].insert(b as usize);
            }
        }
    }
    let mut by_size: Vec<usize> = (0..basins.len()).collect();
    by_size.sort_by(|&a, &b| basins[b].cells.cmp(&basins[a].cells).then(a.cmp(&b)));
    let mut colored = vec![false; basins.len()];
    let mut count = [0usize; VIVID_COLORS + MUTED_COLORS];
    for b in by_size {
        let (first, len) = if basins[b].major() { (0, VIVID_COLORS) } else { (VIVID_COLORS, MUTED_COLORS) };
        let taken: Vec<usize> = neighbours[b].iter().filter(|&&o| colored[o]).map(|&o| basins[o].color).collect();
        // Couleur libre la moins employée (les teintes se répartissent sur la carte)
        let color = (first..first + len)
            .filter(|c| !taken.contains(c))
            .min_by_key(|&c| (count[c], c))
            .unwrap_or_else(|| (first..first + len).min_by_key(|&c| (count[c], c)).unwrap());
        basins[b].color = color;
        count[color] += 1;
        colored[b] = true;
    }

    Basins { basin_of, basins, river_of, river_on_edge, rivers, lakes, lake_of, basin_also }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une ligne de cases dont on donne directement les directions d'écoulement.
    fn line_input(w: i32, downstream: &[Option<u8>], fleuve: &[bool], sea: &[bool], discharge: &[f32], segments: &[RiverSegment]) -> Basins {
        let lake = vec![false; downstream.len()];
        lake_input(w, downstream, fleuve, sea, &lake, discharge, segments)
    }

    fn lake_input(
        w: i32,
        downstream: &[Option<u8>],
        fleuve: &[bool],
        sea: &[bool],
        lake: &[bool],
        discharge: &[f32],
        segments: &[RiverSegment],
    ) -> Basins {
        let lat = |_: i32| 45.0;
        let corner_outlet = vec![[u32::MAX; 6]; downstream.len()];
        let corner_cells: Vec<[u32; 3]> = Vec::new();
        compute(&BasinInput {
            width: w,
            height: (downstream.len() as i32) / w,
            downstream,
            discharge,
            fleuve,
            sea,
            lake,
            segments,
            corner_outlet: &corner_outlet,
            corner_cells: &corner_cells,
            latitude: &lat,
            wrap: Wrap::NONE,
        })
    }

    #[test]
    fn un_seul_nom_meme_si_le_debit_baisse_vers_l_aval() {
        // Fleuve de 5 cases vers l'est ; le débit baisse vers l'aval (pertes du lit)
        let downstream = [Some(0), Some(0), Some(0), Some(0), Some(0), None];
        let sea = [false, false, false, false, false, true];
        let fleuve = [true, true, true, true, true, false];
        let discharge = [3_500.0, 3_400.0, 3_000.0, 2_600.0, 2_200.0, 0.0];
        let b = line_input(6, &downstream, &fleuve, &sea, &discharge, &[]);
        assert_eq!(b.rivers.len(), 1);
        assert_eq!(b.rivers[0].spots.len(), 5);
        assert_eq!(b.rivers[0].first_cell(), 4);
    }

    #[test]
    fn le_nom_traverse_un_petit_lac() {
        // Fleuve, lac de 2 cases, fleuve, puis la mer : un seul nom ; le lac n'a pas de nom
        let downstream = [Some(0); 8].into_iter().chain([None]).collect::<Vec<_>>();
        let fleuve = [true, true, true, false, false, true, true, true, false];
        let lake = [false, false, false, true, true, false, false, false, false];
        let sea = [false, false, false, false, false, false, false, false, true];
        let discharge = [2_100.0, 2_200.0, 2_300.0, 2_400.0, 2_400.0, 2_500.0, 2_600.0, 2_700.0, 0.0];
        let b = lake_input(9, &downstream, &fleuve, &sea, &lake, &discharge, &[]);
        assert_eq!(b.rivers.len(), 1);
        assert_eq!(b.rivers[0].spots.len(), 6);
        // (les 6 cases de fleuve, les 2 du lac, et le bord de la mer à l'embouchure)
        assert_eq!(b.rivers[0].course.len(), 9);
        assert!(b.lakes.is_empty());
    }

    #[test]
    fn grand_lac_nomme_et_fleuve_qui_s_y_jette() {
        // Fleuve de 3 cases qui se jette dans un lac de 3 cases
        let downstream = [Some(0), Some(0), Some(0), Some(0), Some(0), None];
        let fleuve = [true, true, true, false, false, false];
        let lake = [false, false, false, true, true, true];
        let sea = [false; 6];
        let discharge = [2_100.0, 2_200.0, 2_300.0, 2_300.0, 2_300.0, 2_300.0];
        let b = lake_input(6, &downstream, &fleuve, &sea, &lake, &discharge, &[]);
        assert_eq!(b.lakes.len(), 1);
        assert_eq!(b.lakes[0].cells.len(), 3);
        // Lac sans issue : un lac salé
        assert_eq!(b.lakes[0].kind, WaterBody::SaltLake);
        assert_eq!(b.lake_of[4], Some(0));
        assert_eq!(b.rivers.len(), 1);
        assert_eq!(b.rivers[0].into_lake, Some(0));
        assert_eq!(b.lakes[0].outflow, None);
    }

    #[test]
    fn fleuve_qui_sort_d_un_lac() {
        // Lac de 3 cases qui se déverse dans un fleuve de 3 cases, puis la mer
        let downstream = [Some(0); 6].into_iter().chain([None]).collect::<Vec<_>>();
        let lake = [true, true, true, false, false, false, false];
        let fleuve = [false, false, false, true, true, true, false];
        let sea = [false, false, false, false, false, false, true];
        let discharge = [2_100.0, 2_200.0, 2_300.0, 2_300.0, 2_400.0, 2_500.0, 0.0];
        let b = lake_input(7, &downstream, &fleuve, &sea, &lake, &discharge, &[]);
        assert_eq!(b.lakes.len(), 1);
        assert_eq!(b.lakes[0].kind, WaterBody::FreshLake);
        assert_eq!(b.lakes[0].outflow, Some(0));
        // Le tracé du fleuve part du bord de la mer et remonte jusqu'au bord du lac
        let course = &b.rivers[0].course;
        assert_eq!(course.first(), Some(&CoursePoint { cell: 5, corner: CoursePoint::EDGE }));
        assert_eq!(course.last(), Some(&CoursePoint { cell: 3, corner: CoursePoint::EDGE + 3 }));
    }

    #[test]
    fn mer_interieure_nommee() {
        // Grille 7 × 5 : terre partout, une mer de 3 cases au milieu (sans contact avec le bord)
        let (w, h) = (7, 5);
        let n = (w * h) as usize;
        let mut sea = vec![false; n];
        for c in [16, 17, 18] {
            sea[c] = true;
        }
        let downstream = vec![None; n];
        let b = lake_input(w, &downstream, &[false; 35], &sea, &[false; 35], &[1.0; 35], &[]);
        assert_eq!(b.lakes.len(), 1);
        assert_eq!(b.lakes[0].kind, WaterBody::InlandSea);
        assert_eq!(b.lake_of[17], Some(0));
    }

    #[test]
    fn petit_bras_rattache_au_fleuve_recepteur() {
        // Grille 5 × 2 : fleuve principal sur la ligne 0 (vers l'est, mer en bout), et un bras
        // d'une case (ligne 1) qui s'y jette
        let w = 5;
        let mut downstream = vec![None; 10];
        let mut fleuve = vec![false; 10];
        let mut sea = vec![false; 10];
        let mut discharge = vec![0.0; 10];
        for c in 0..4 {
            downstream[c] = Some(0);
            fleuve[c] = true;
            discharge[c] = 3_000.0 + c as f32 * 100.0;
        }
        sea[4] = true;
        // (1, 1) : en ligne impaire, la voisine nord-est (direction 1) est (2, 0)
        let arm = 5 + 1;
        downstream[arm] = Some(1);
        fleuve[arm] = true;
        discharge[arm] = 2_000.0;
        let b = line_input(w, &downstream, &fleuve, &sea, &discharge, &[]);
        assert_eq!(b.rivers.len(), 1);
        assert_eq!(b.river_of[arm], b.river_of[0]);
    }

    #[test]
    fn une_ligne_qui_coule_vers_la_mer() {
        // 5 cases de terre vers l'est, puis la mer
        let downstream = [Some(0), Some(0), Some(0), Some(0), Some(0), None];
        let sea = [false, false, false, false, false, true];
        let fleuve = [false, true, true, true, true, false];
        let discharge = [10.0, 2_000.0, 2_500.0, 3_000.0, 3_500.0, 0.0];
        let b = line_input(6, &downstream, &fleuve, &sea, &discharge, &[]);
        assert_eq!(b.basins.len(), 1);
        assert_eq!(b.basins[0].outlet, 4);
        assert_eq!(b.basins[0].cells, 5);
        assert!(b.basin_of[5].is_none());
        // Un seul fleuve nommé, d'aval en amont
        assert_eq!(b.rivers.len(), 1);
        assert_eq!(b.rivers[0].spots, vec![(4, 4), (3, 3), (2, 2), (1, 1)]);
        assert_eq!(b.rivers[0].tier, 0);
        assert_eq!(b.basins[0].river, Some(0));
        assert!(b.basins[0].color < VIVID_COLORS);
    }

    #[test]
    fn riviere_d_aretes_nommee() {
        // Trois arêtes enchaînées (coins 0 → 1 → 2 → 3) et une arête isolée qui se jette au coin 2
        let seg = |cell: u32, dir: u8, up: u32, down: u32, flow: f32| RiverSegment { cell, dir, up, down, down_corner: 0, flow };
        let segments = [seg(1, 5, 2, 3, 300.0), seg(1, 0, 1, 2, 200.0), seg(1, 1, 0, 1, 150.0), seg(4, 2, 9, 2, 100.0)];
        let segments = segments.map(|x| RiverSegment { down_corner: x.dir, ..x });
        let none = [false; 8];
        let b = line_input(4, &[None; 8], &none, &none, &[1.0; 8], &segments);
        // Le cours principal (3 arêtes) est nommé, l'arête isolée n'est qu'un raccord
        assert_eq!(b.rivers.len(), 1);
        let r = &b.rivers[0];
        assert!(!r.fleuve && r.tier == 2 && r.spots.len() == 3);
        // Visible des deux cases de chaque arête
        assert_eq!(b.river_on_edge(1, 5), Some(r));
        let other = r.spots[0].1;
        assert_eq!(b.river_on_edge(other, 2), Some(r));
        assert!(b.river_on_edge(4, 2).is_none());
    }

    #[test]
    fn deux_bassins_voisins_de_couleurs_differentes() {
        // Ligne de 4 : deux cases vers l'ouest (bord), deux vers l'est (bord)
        let downstream = [Some(3), Some(3), Some(0), Some(0)];
        let none = [false; 4];
        let b = line_input(4, &downstream, &none, &none, &[1.0; 4], &[]);
        assert_eq!(b.basins.len(), 2);
        assert_ne!(b.basin_of[1], b.basin_of[2]);
        assert_ne!(b.basins[0].color, b.basins[1].color);
        assert!(b.basins.iter().all(|x| x.color >= VIVID_COLORS && x.river.is_none()));
    }

    #[test]
    fn carte_generee() {
        let m = crate::map::WorldMap::generate_in(128, 74, 2, crate::climate::Region::planet(crate::climate::CELL_KM, 74));
        let b = m.basins();
        // Toutes les cases de terre ont un bassin
        for h in m.hexes() {
            let t = m.get(h).unwrap();
            assert_eq!(b.basin(m.cell(h)).is_some(), !t.is_sea(), "{h:?}");
        }
        let fleuves: Vec<&NamedRiver> = b.rivers.iter().filter(|r| r.fleuve).collect();
        assert!(!fleuves.is_empty());
        // Petits bras : tracés à part, partant du milieu du lit d'une case de leur cours (ou du
        // bord de leur première case, quand ils se jettent dans un petit lac du cours)
        for (r, river) in b.rivers.iter().enumerate() {
            for arm in &river.arms {
                assert!(arm.len() >= 2);
                assert!(arm[0].corner == CoursePoint::CENTER || arm[0].corner >= CoursePoint::EDGE, "{}", river.name);
                assert_eq!(b.river_of[arm[0].cell], Some(r as u16), "{}", river.name);
            }
        }
        // Rangs : 0 au déversoir, un de plus qu'un cours récepteur ; un lac a le rang du cours qui
        // en sort
        for river in &b.rivers {
            match (river.flows_into, river.into_lake) {
                (Some(x), _) => assert_eq!(river.level, b.rivers[x].level + 1, "{}", river.name),
                (None, Some(l)) => assert!(river.level == b.lakes[l].level || river.level == b.lakes[l].level + 1),
                (None, None) => assert_eq!(river.level, 0, "{}", river.name),
            }
        }
        // Un bassin fermé nommé d'après son lac : un lac salé ou une mer intérieure
        for basin in &b.basins {
            if let Some(l) = basin.lake {
                assert!(basin.closed);
                assert_ne!(b.lakes[l].kind, WaterBody::FreshLake, "{}", b.lakes[l].name);
                assert_eq!(b.basin_name(basin), Some(b.lakes[l].name.as_str()));
            }
        }
        for lake in &b.lakes {
            assert_eq!(lake.level, lake.outflow.map_or(0, |r| b.rivers[r].level), "{}", lake.name);
        }
        assert!(b.rivers.iter().any(|r| !r.fleuve), "des rivières nommées");
        // Fleuves d'abord, par importance puis débit décroissant ; chaque case de fleuve a un nom
        assert!(b.rivers.windows(2).all(|w| (!w[0].fleuve, w[0].tier) <= (!w[1].fleuve, w[1].tier)));
        for w in b.rivers.windows(2).filter(|w| w[0].fleuve == w[1].fleuve && w[0].tier == w[1].tier) {
            // (rivières d'arête : un affluent peut garder plus d'eau que la rivière où il se jette,
            // qui en perd dans son lit en climat sec ; ordre approché)
            let tolerance = if w[0].fleuve { 1.0 } else { 0.9 };
            assert!(w[0].discharge >= w[1].discharge * tolerance, "{} {} | {} {}", w[0].name, w[0].discharge, w[1].name, w[1].discharge);
        }
        for h in m.hexes().filter(|&h| m.is_fleuve(h)) {
            assert!(b.river(m.cell(h)).is_some(), "{h:?}");
        }
        // Le long d'un fleuve, le nom ne change qu'à un confluent : quand une case n'a pas le nom
        // de la case aval, celle-ci continue son nom par un autre affluent
        for h in m.hexes().filter(|&h| m.is_fleuve(h)) {
            let Some(d) = m.downstream(h) else { continue };
            let down = m.canonical(h + crate::hex::DIRECTIONS[d]);
            if !m.is_fleuve(down) {
                continue;
            }
            let (mine, theirs) = (b.river_of[m.cell(h)], b.river_of[m.cell(down)]);
            if mine != theirs {
                let continued = m.neighbors(down).iter().any(|&u| {
                    m.is_fleuve(u) && m.downstream(u).is_some_and(|du| m.canonical(u + crate::hex::DIRECTIONS[du]) == down) && b.river_of[m.cell(u)] == theirs
                });
                assert!(continued, "{h:?} : {mine:?} se jette dans {theirs:?} sans confluent");
            }
        }
        // Toute case de lac porte le nom de son lac, ou celui du fleuve qui le traverse
        for h in m.hexes().filter(|&h| m.get(h) == Some(crate::map::Terrain::Lake)) {
            let c = m.cell(h);
            assert!(b.lake(c).is_some() != b.river(c).is_some(), "{h:?}");
        }
        // Aucune case de fleuve n'est presque entourée par un lac
        for h in m.hexes().filter(|&h| m.is_fleuve(h)) {
            let lakes = h.neighbors().iter().filter(|&&nb| m.get(nb) == Some(crate::map::Terrain::Lake)).count();
            assert!(lakes < crate::map::FLEUVE_IN_LAKE, "{h:?}");
        }
        // Chaque cours d'eau et chaque lac a un nom, jamais le même qu'un autre
        let mut names = std::collections::HashSet::new();
        for name in b.rivers.iter().map(|r| &r.name).chain(b.lakes.iter().map(|l| &l.name)) {
            assert!(!name.is_empty());
            assert!(names.insert(name.to_lowercase()), "{name} répété");
        }
        // Le fleuve principal d'un bassin est son cours principal
        for (k, r) in b.rivers.iter().enumerate().filter(|(_, r)| r.tier == 0) {
            assert_eq!(b.basin(r.first_cell()).unwrap().river, Some(k));
        }
    }
}
