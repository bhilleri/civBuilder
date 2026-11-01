//! Relief nommé (logique pure, calculé à la génération) : chaînes de montagnes et leurs massifs,
//! cols, plateaux, collines.
//!
//! 1. **Chaînes** : amas de cases de montagne ou de pic (et de relief accidenté en altitude), une
//!    vallée d'une case entre deux montagnes ne les séparant pas. Une longue chaîne est découpée
//!    en **massifs** d'une vingtaine de cases (germes les plus éloignés les uns des autres le long
//!    de la chaîne, puis chaque case au germe le plus proche, recentrés trois fois).
//! 2. **Cols** : le passage le moins haut à travers la chaîne. Pour chaque case de la chaîne, la
//!    traversée en ligne droite qui ressort des deux côtés sur une terre basse (sans qu'on puisse
//!    contourner de près : pas le bout d'une chaîne) et dont le point le plus haut est le plus bas.
//!    Un col est une traversée plus basse que toutes ses voisines et nettement plus basse que la
//!    plupart : celle que prennent les routes.
//! 3. **Plateaux** : terres hautes (au moins 1 500 m) plates ou vallonnées, d'un seul tenant.
//! 4. **Collines** : amas de cases vallonnées.

use crate::hex::Wrap;
use crate::map::Relief;
use crate::seas::{Grid, shape};
use crate::toponymy::Etymology;
use bevy::platform::collections::HashMap;
use std::collections::VecDeque;

/// Nature d'un relief nommé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReliefKind {
    Range,
    Massif,
    Pass,
    Plateau,
    Hills,
}

impl ReliefKind {
    pub fn label(self) -> &'static str {
        match self {
            ReliefKind::Range => "chaîne de montagnes",
            ReliefKind::Massif => "massif",
            ReliefKind::Pass => "col",
            ReliefKind::Plateau => "plateau",
            ReliefKind::Hills => "collines",
        }
    }
}

/// Un relief nommé.
#[derive(Clone, Debug)]
pub struct ReliefZone {
    pub kind: ReliefKind,
    /// Ses cases, avec pour chacune sa place pour le nom (plus c'est grand, mieux c'est : la
    /// distance au bord, ou pour une chaîne la proximité de son centre le long de la crête).
    pub cells: Vec<usize>,
    pub inner: Vec<u16>,
    /// Zone qui la contient (massif : sa chaîne ; col : son massif, sinon sa chaîne).
    pub parent: Option<usize>,
    /// Demi-largeur d'ouest en est (unités du monde 2D).
    pub half_width: f32,
    /// Col : altitude (m) de son point le plus haut, la crête du passage.
    pub altitude: f32,
    pub name: String,
    pub feminine: bool,
    pub etymology: Option<Etymology>,
}

/// Relief nommé d'une carte.
#[derive(Clone, Debug, Default)]
pub struct Reliefs {
    pub zones: Vec<ReliefZone>,
    /// Chaîne de chaque case.
    pub range_of: Vec<Option<u16>>,
    /// Massif de chaque case.
    pub massif_of: Vec<Option<u16>>,
    /// Col de chaque case.
    pub pass_of: Vec<Option<u16>>,
    /// Plateau de chaque case.
    pub plateau_of: Vec<Option<u16>>,
    /// Collines de chaque case.
    pub hills_of: Vec<Option<u16>>,
}

impl Reliefs {
    /// Reliefs d'une case, du plus précis au plus vaste (col, massif, chaîne ; plateau ; collines).
    pub fn chain(&self, cell: usize) -> Vec<usize> {
        [&self.pass_of, &self.massif_of, &self.range_of, &self.plateau_of, &self.hills_of].iter().filter_map(|v| v.get(cell).copied().flatten()).map(|z| z as usize).collect()
    }
}

/// Relief accidenté qui compte comme montagne au-dessus de cette altitude (m).
const HIGH_RUGGED_M: f32 = 1_500.0;
/// Plus petite chaîne nommée (cases).
const RANGE_MIN_CELLS: usize = 8;
/// Une chaîne plus longue est découpée en massifs d'environ `MASSIF_CELLS` cases.
const MASSIF_SPLIT_CELLS: usize = 45;
const MASSIF_CELLS: usize = 22;
/// Col : traversée plus basse que toutes celles à moins de `PASS_REACH` cases, et d'au moins tant
/// de mètres que la plus haute de chaque côté (le long de la chaîne) ; traversée d'au plus `CROSSING_MAX` cases de chaîne,
/// dont les deux côtés ne se rejoignent pas à moins de `SIDES_REACH` cases.
const PASS_DROP_M: f32 = 100.0;
const PASS_REACH: i32 = 5;
const CROSSING_MAX: usize = 8;
const SIDES_REACH: i32 = 6;
/// Cols : au plus tant par chaîne non découpée ou par massif, à tant de cases au moins les uns
/// des autres, au plus `MAX_PASSES` en tout (les plus marqués d'abord).
const PASSES_PER_MASSIF: usize = 2;
const PASS_SPACING: i32 = 5;
const MAX_PASSES: usize = 40;
/// Plateau : altitude minimale (m), taille minimale (cases), au plus `MAX_PLATEAUS`.
pub const PLATEAU_MIN_M: f32 = 1_500.0;
const PLATEAU_MIN_CELLS: usize = 20;
const MAX_PLATEAUS: usize = 8;
/// Collines : taille minimale (cases), au plus `MAX_HILLS` (les plus grandes).
const HILLS_MIN_CELLS: usize = 12;
const MAX_HILLS: usize = 15;

/// Relief nommé. `land` : case de terre (ni lac ni mer) ; `relief` et `elevation` (m) : relief et
/// altitude du sol de chaque case.
pub fn compute(width: i32, height: i32, wrap: Wrap, land: &[bool], relief: &[Relief], elevation: &[f32]) -> Reliefs {
    let grid = Grid { width, height, wrap };
    let n = land.len();
    let mut out = Reliefs { zones: Vec::new(), range_of: vec![None; n], massif_of: vec![None; n], pass_of: vec![None; n], plateau_of: vec![None; n], hills_of: vec![None; n] };
    let high = |c: usize| land[c] && (relief[c] >= Relief::Mountain || (relief[c] == Relief::Rugged && elevation[c] >= HIGH_RUGGED_M));
    // Vallée d'une case qui relie deux montagnes (de part et d'autre : les montagnes voisines ne se
    // touchent pas entre elles)
    let bridge = |c: usize| land[c] && !high(c) && runs(&grid, c, &high) >= 2;
    let in_range: Vec<bool> = (0..n).map(|c| high(c) || bridge(c)).collect();

    // 1) Chaînes, puis massifs
    let ranges: Vec<Vec<usize>> = components(&grid, |c| in_range[c]).into_iter().filter(|cells| cells.iter().filter(|&&c| high(c)).count() >= RANGE_MIN_CELLS).collect();
    let mut massifs: Vec<(usize, Vec<usize>)> = Vec::new();
    for cells in &ranges {
        let id = out.zones.len();
        for &c in cells {
            out.range_of[c] = Some(id as u16);
        }
        let zone = centered_zone(&grid, ReliefKind::Range, cells.clone(), None, &in_range);
        out.zones.push(zone);
        if cells.len() > MASSIF_SPLIT_CELLS {
            let parts = (cells.len() as f32 / MASSIF_CELLS as f32).round().max(2.0) as usize;
            for part in split(&grid, cells, parts, &in_range) {
                massifs.push((id, part));
            }
        }
    }
    for (range, cells) in massifs {
        let id = out.zones.len();
        for &c in &cells {
            out.massif_of[c] = Some(id as u16);
        }
        let mut member = vec![false; n];
        for &c in &cells {
            member[c] = true;
        }
        out.zones.push(centered_zone(&grid, ReliefKind::Massif, cells, Some(range), &member));
    }

    // 2) Cols : le passage le moins haut à travers la chaîne. Pour chaque case de la chaîne, la
    //    traversée en ligne droite (le long d'un des trois axes) qui ressort des deux côtés sur une
    //    terre basse, celle dont le point le plus haut est le plus bas ; les deux côtés ne doivent pas
    //    se rejoindre de près en contournant la chaîne (pas le bout d'une chaîne). Un col est une
    //    traversée plus basse que toutes ses voisines et nettement plus basse que la plupart.
    //    (une traversée près d'un bout compte parmi les voisines, sans pouvoir être un col)
    let mut cross: Vec<Option<(f32, usize, bool)>> = vec![None; n];
    for c in (0..n).filter(|&c| out.range_of[c].is_some()) {
        for axis in 0..3 {
            let Some((top, crest, ends)) = crossing(&grid, c, axis, land, &high, elevation) else { continue };
            if cross[c].is_some_and(|(best, _, _)| best <= top) {
                continue;
            }
            cross[c] = Some((top, crest, !joined(&grid, c, ends, &high, land)));
        }
    }
    let mut saddles: Vec<(f32, usize, f32)> = Vec::new();
    for c in 0..n {
        let Some((top, crest, true)) = cross[c] else { continue };
        // (un pic, infranchissable, n'est pas un passage)
        if relief[crest] == Relief::Peak {
            continue;
        }
        // Les vraies traversées voisines : aucune plus basse ; de plus hautes des deux côtés le
        // long de la chaîne (le col est un creux de la crête, pas le bout de la chaîne)
        let near: Vec<(f32, usize)> =
            wrap.range(grid.hex(c), PASS_REACH).filter_map(|h| grid.index(h)).filter(|&j| j != c).filter_map(|j| cross[j].filter(|x| x.2).map(|(t, _, _)| (t, j))).collect();
        if near.iter().any(|&(t, j)| t < top || (t == top && j < c)) {
            continue;
        }
        let offset = |j: usize| {
            let ((x0, y0), (x1, y1)) = (grid.hex(c).to_world(), grid.hex(j).to_world());
            let world = wrap.world_width();
            let mut dx = x1 - x0;
            if world > 0.0 {
                dx -= (dx / world).round() * world;
            }
            (dx, y1 - y0)
        };
        let Some(&(_, highest)) = near.iter().max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1))) else { continue };
        let axis = offset(highest);
        let side = |positive: bool| {
            near.iter().filter(|&&(_, j)| {
                let (dx, dy) = offset(j);
                (dx * axis.0 + dy * axis.1 > 0.0) == positive
            }).map(|&(t, _)| t).fold(f32::NEG_INFINITY, f32::max)
        };
        let drop = side(true).min(side(false)) - top;
        if drop >= PASS_DROP_M {
            saddles.push((drop, crest, top));
        }
    }
    saddles.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut kept: Vec<(usize, f32)> = Vec::new();
    let mut per_group: HashMap<u16, usize> = HashMap::new();
    for (_, c, top) in saddles {
        if kept.len() >= MAX_PASSES {
            break;
        }
        let group = out.massif_of[c].or(out.range_of[c]).unwrap();
        if per_group.get(&group).copied().unwrap_or(0) >= PASSES_PER_MASSIF || kept.iter().any(|&(k, _)| wrap.distance(grid.hex(k), grid.hex(c)) < PASS_SPACING) {
            continue;
        }
        *per_group.entry(group).or_default() += 1;
        kept.push((c, top));
    }
    for (c, top) in kept {
        let id = out.zones.len();
        out.pass_of[c] = Some(id as u16);
        let parent = out.massif_of[c].or(out.range_of[c]).map(usize::from);
        out.zones.push(ReliefZone {
            kind: ReliefKind::Pass,
            cells: vec![c],
            inner: vec![1],
            parent,
            half_width: crate::hex::HEX_SIZE,
            altitude: top,
            name: String::new(),
            feminine: false,
            etymology: None,
        });
    }

    // 3) Plateaux : terres hautes plates ou vallonnées
    let flat_high = |c: usize| land[c] && relief[c] <= Relief::Hills && elevation[c] >= PLATEAU_MIN_M;
    let mut plateaus: Vec<Vec<usize>> = components(&grid, flat_high).into_iter().filter(|c| c.len() >= PLATEAU_MIN_CELLS).collect();
    plateaus.sort_by_key(|c| (std::cmp::Reverse(c.len()), c[0]));
    plateaus.truncate(MAX_PLATEAUS);
    for cells in plateaus {
        let id = out.zones.len() as u16;
        for &c in &cells {
            out.plateau_of[c] = Some(id);
        }
        out.zones.push(zone(&grid, ReliefKind::Plateau, cells, &out.plateau_of));
    }

    // 4) Collines : amas de cases vallonnées (hors plateaux, déjà nommés)
    let hilly = |c: usize| land[c] && relief[c] == Relief::Hills && out.plateau_of[c].is_none();
    let mut hills: Vec<Vec<usize>> = components(&grid, hilly).into_iter().filter(|c| c.len() >= HILLS_MIN_CELLS).collect();
    hills.sort_by_key(|c| (std::cmp::Reverse(c.len()), c[0]));
    hills.truncate(MAX_HILLS);
    for cells in hills {
        let id = out.zones.len() as u16;
        for &c in &cells {
            out.hills_of[c] = Some(id);
        }
        out.zones.push(zone(&grid, ReliefKind::Hills, cells, &out.hills_of));
    }
    out
}

/// Traversée d'une chaîne par la case `c` le long d'un axe (`axis` et son opposé) : altitude du
/// point le plus haut, cette case (la crête du passage) et les deux cases de terre basse où elle
/// ressort. Aucune si elle aboutit à l'eau, au bord de la carte ou dépasse `CROSSING_MAX` cases.
fn crossing(grid: &Grid, c: usize, axis: usize, land: &[bool], high: &impl Fn(usize) -> bool, elevation: &[f32]) -> Option<(f32, usize, [usize; 2])> {
    let (mut top, mut crest) = (elevation[c], c);
    let mut ends = [c; 2];
    let mut length = 1;
    for (side, d) in [axis, axis + 3].into_iter().enumerate() {
        let mut at = grid.hex(c);
        loop {
            at = at + crate::hex::DIRECTIONS[d];
            let j = grid.index(at)?;
            if !land[j] {
                return None;
            }
            if !high(j) {
                ends[side] = j;
                break;
            }
            length += 1;
            if length > CROSSING_MAX {
                return None;
            }
            if elevation[j] > top {
                (top, crest) = (elevation[j], j);
            }
        }
    }
    Some((top, crest, ends))
}

/// Les deux bouts d'une traversée se rejoignent-ils par la terre basse sans passer par `c`, à
/// moins de `SIDES_REACH` cases de `c` (on contourne un bout de chaîne : ce n'est pas un col) ?
fn joined(grid: &Grid, c: usize, ends: [usize; 2], high: &impl Fn(usize) -> bool, land: &[bool]) -> bool {
    let center = grid.hex(c);
    let mut seen: HashMap<usize, ()> = HashMap::new();
    let mut queue = VecDeque::from([ends[0]]);
    seen.insert(ends[0], ());
    while let Some(x) = queue.pop_front() {
        if x == ends[1] {
            return true;
        }
        for j in grid.neighbors(x) {
            if j != c && land[j] && !high(j) && !seen.contains_key(&j) && grid.wrap.distance(center, grid.hex(j)) <= SIDES_REACH {
                seen.insert(j, ());
                queue.push_back(j);
            }
        }
    }
    false
}

/// Nombre de suites de voisines retenues autour d'une case (en faisant le tour de ses six
/// voisines) : 2 et plus, la case relie des voisines qui ne se touchent pas.
fn runs(grid: &Grid, c: usize, keep: &impl Fn(usize) -> bool) -> usize {
    let h = grid.hex(c);
    let ring: Vec<bool> = crate::hex::DIRECTIONS.iter().map(|&d| grid.index(h + d).is_some_and(keep)).collect();
    (0..6).filter(|&i| ring[i] && !ring[(i + 5) % 6]).count()
}

/// Morceaux d'un seul tenant des cases retenues (cases rangées dans l'ordre).
fn components(grid: &Grid, keep: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
    let n = (grid.width * grid.height) as usize;
    let mut seen = vec![false; n];
    let mut all = Vec::new();
    for start in 0..n {
        if seen[start] || !keep(start) {
            continue;
        }
        seen[start] = true;
        let mut cells = vec![start];
        let mut i = 0;
        while i < cells.len() {
            let c = cells[i];
            i += 1;
            for j in grid.neighbors(c) {
                if !seen[j] && keep(j) {
                    seen[j] = true;
                    cells.push(j);
                }
            }
        }
        cells.sort_unstable();
        all.push(cells);
    }
    all
}

/// Distance (en cases, sans sortir de `member`) de chaque case aux cases de départ.
fn walk(grid: &Grid, from: &[usize], member: &[bool]) -> Vec<u16> {
    let mut d = vec![u16::MAX; member.len()];
    let mut queue = VecDeque::new();
    for &c in from {
        d[c] = 0;
        queue.push_back(c);
    }
    while let Some(c) = queue.pop_front() {
        for j in grid.neighbors(c) {
            if member[j] && d[j] == u16::MAX {
                d[j] = d[c] + 1;
                queue.push_back(j);
            }
        }
    }
    d
}

/// Découpe une chaîne en `parts` massifs : germes les plus éloignés les uns des autres le long
/// de la chaîne, chaque case au germe le plus proche (par la chaîne), chaque germe recentré
/// (la case la plus proche de toutes les autres de son massif), trois fois.
fn split(grid: &Grid, cells: &[usize], parts: usize, member: &[bool]) -> Vec<Vec<usize>> {
    // Premier germe : un bout de la chaîne (la case la plus éloignée de la première)
    let d = walk(grid, &cells[..1], member);
    let far = |d: &[u16]| cells.iter().copied().max_by_key(|&c| (d[c], std::cmp::Reverse(c))).unwrap();
    let mut seeds = vec![far(&d)];
    let mut nearest = walk(grid, &seeds, member);
    while seeds.len() < parts {
        let next = far(&nearest);
        seeds.push(next);
        let d = walk(grid, &[next], member);
        for &c in cells {
            nearest[c] = nearest[c].min(d[c]);
        }
    }
    let mut regions: Vec<Vec<usize>> = Vec::new();
    for _ in 0..4 {
        // Chaque case au germe le plus proche (parcours en largeur depuis tous les germes)
        let mut owner: HashMap<usize, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        for (k, &s) in seeds.iter().enumerate() {
            owner.insert(s, k);
            queue.push_back(s);
        }
        while let Some(c) = queue.pop_front() {
            let k = owner[&c];
            for j in grid.neighbors(c) {
                if member[j] && !owner.contains_key(&j) {
                    owner.insert(j, k);
                    queue.push_back(j);
                }
            }
        }
        regions = vec![Vec::new(); seeds.len()];
        for &c in cells {
            regions[owner[&c]].push(c);
        }
        // (cases rangées : `cells` l'est)
        // Germe recentré : la case la plus proche de toutes les autres de son massif
        for (k, region) in regions.iter().enumerate() {
            let center = region
                .iter()
                .copied()
                .min_by_key(|&a| (region.iter().map(|&b| grid.wrap.distance(grid.hex(a), grid.hex(b))).sum::<i32>(), a))
                .unwrap_or(seeds[k]);
            seeds[k] = center;
        }
    }
    // Un massif trop petit (un reste entre deux germes) rejoint son voisin le plus touché
    loop {
        regions.retain(|r| !r.is_empty());
        let Some(small) = (0..regions.len()).filter(|&k| regions[k].len() < MASSIF_CELLS / 2).min_by_key(|&k| (regions[k].len(), regions[k][0])) else { break };
        if regions.len() < 2 {
            break;
        }
        let mut touch: HashMap<usize, usize> = HashMap::new();
        for &c in &regions[small] {
            for j in grid.neighbors(c) {
                if let Some(k) = regions.iter().position(|r| k_contains(r, j))
                    && k != small
                {
                    *touch.entry(k).or_default() += 1;
                }
            }
        }
        let Some((&into, _)) = touch.iter().max_by_key(|(k, t)| (**t, std::cmp::Reverse(**k))) else { break };
        let moved = std::mem::take(&mut regions[small]);
        regions[into].extend(moved);
        regions[into].sort_unstable();
    }
    regions
}

/// Case dans une liste rangée.
fn k_contains(sorted: &[usize], c: usize) -> bool {
    sorted.binary_search(&c).is_ok()
}

/// Zone allongée (chaîne, massif) : le nom se pose près de son centre le long de la crête (la
/// case la plus proche de toutes les autres), pas sur sa partie la plus épaisse.
fn centered_zone(grid: &Grid, kind: ReliefKind, cells: Vec<usize>, parent: Option<usize>, member: &[bool]) -> ReliefZone {
    let owner: Vec<Option<u16>> = member.iter().map(|&m| m.then_some(0)).collect();
    let (_, half_width) = shape(grid, &cells, &owner);
    let center = cells.iter().copied().min_by_key(|&a| (cells.iter().map(|&b| grid.wrap.distance(grid.hex(a), grid.hex(b))).sum::<i32>(), a)).unwrap();
    let d = walk(grid, &[center], member);
    let far = cells.iter().map(|&c| d[c]).filter(|&x| x != u16::MAX).max().unwrap_or(0);
    let inner = cells.iter().map(|&c| far.saturating_sub(d[c].min(far))).collect();
    ReliefZone { kind, cells, inner, parent, half_width, altitude: 0.0, name: String::new(), feminine: false, etymology: None }
}

fn zone(grid: &Grid, kind: ReliefKind, cells: Vec<usize>, owner: &[Option<u16>]) -> ReliefZone {
    let (inner, half_width) = shape(grid, &cells, owner);
    ReliefZone { kind, cells, inner, parent: None, half_width, altitude: 0.0, name: String::new(), feminine: false, etymology: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Carte de 60 × 30 cases : une chaîne d'ouest en est sur la ligne 15 (3 cases d'épaisseur),
    /// avec deux sommets et une échancrure au milieu ; un plateau, des collines.
    fn world() -> (i32, Vec<bool>, Vec<Relief>, Vec<f32>) {
        let (w, h) = (60, 30);
        let n = (w * h) as usize;
        let land = vec![true; n];
        let mut relief = vec![Relief::Flat; n];
        let mut elevation = vec![300.0; n];
        for c in 0..n {
            let (x, y) = (c as i32 % w, c as i32 / w);
            if (14..17).contains(&y) && (5..55).contains(&x) {
                relief[c] = Relief::Mountain;
                // Deux sommets (x = 15 et x = 45), un col bas en x = 30
                elevation[c] = 3_000.0 - 60.0 * ((x - 15).abs().min((x - 45).abs())) as f32;
            }
            if (2..8).contains(&y) && (10..20).contains(&x) {
                elevation[c] = 2_000.0;
            }
            if (22..26).contains(&y) && (30..38).contains(&x) {
                relief[c] = Relief::Hills;
            }
        }
        (w, land, relief, elevation)
    }

    #[test]
    fn chaine_massifs_et_col() {
        let (w, land, relief, elevation) = world();
        let r = compute(w, 30, Wrap::NONE, &land, &relief, &elevation);
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let range = r.range_of[at(20, 15)].expect("une chaîne");
        assert_eq!(r.zones[range as usize].kind, ReliefKind::Range);
        assert_eq!(r.range_of[at(50, 14)], Some(range), "d'un seul tenant");
        // 150 cases : découpée en massifs, dans la chaîne
        let (a, b) = (r.massif_of[at(8, 15)].expect("massif ouest"), r.massif_of[at(52, 15)].expect("massif est"));
        assert_ne!(a, b);
        assert_eq!(r.zones[a as usize].parent, Some(range as usize));
        // Le col entre les deux sommets, au point bas de la crête
        let pass = r.zones.iter().find(|z| z.kind == ReliefKind::Pass).expect("un col");
        let x = pass.cells[0] as i32 % w;
        assert!((28..=32).contains(&x), "col en x = {x}");
        assert!(pass.altitude < 2_200.0, "{} m", pass.altitude);
    }

    #[test]
    fn plateau_et_collines() {
        let (w, land, relief, elevation) = world();
        let r = compute(w, 30, Wrap::NONE, &land, &relief, &elevation);
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let plateau = r.plateau_of[at(15, 4)].expect("un plateau");
        assert_eq!(r.zones[plateau as usize].kind, ReliefKind::Plateau);
        let hills = r.hills_of[at(33, 24)].expect("des collines");
        assert_eq!(r.zones[hills as usize].kind, ReliefKind::Hills);
        assert!(r.plateau_of[at(40, 10)].is_none() && r.hills_of[at(40, 10)].is_none());
    }

    /// Bilan du relief nommé : `cargo test bilan_relief -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bilan_relief() {
        for seed in [777, 4242, 2, 5, 31] {
            let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let r = m.reliefs();
            let count = |k: ReliefKind| r.zones.iter().filter(|z| z.kind == k).count();
            println!(
                "graine {seed} : chaînes {}, massifs {}, cols {}, plateaux {}, collines {}",
                count(ReliefKind::Range),
                count(ReliefKind::Massif),
                count(ReliefKind::Pass),
                count(ReliefKind::Plateau),
                count(ReliefKind::Hills)
            );
            for z in &r.zones {
                let parent = z.parent.map_or(String::new(), |p| format!(" (dans {})", r.zones[p].name));
                let extra = if z.kind == ReliefKind::Pass { format!(" {:.0} m", z.altitude) } else { String::new() };
                println!("  {:<20} {:<30} {:>5} cases{extra}{parent}", z.kind.label(), z.name, z.cells.len());
            }
        }
    }
}
