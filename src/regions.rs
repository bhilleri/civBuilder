//! Régions naturelles (logique pure, calculée à la génération) : forêts, jungles, taïgas,
//! déserts, steppes et plaines, savanes, maquis, marais, toundras.
//!
//! 1. **Groupe de biome** de chaque case de terre (la forêt tempérée, la jungle, la savane et la
//!    savane arborée…), **lissé** : une case entourée aux deux tiers par un même groupe en fait
//!    partie (petits trous tolérés : une clairière dans la forêt, une oasis dans le désert).
//! 2. **Régions** : amas d'un seul tenant d'un même groupe, au-delà d'une taille propre au groupe
//!    (un marais compte dès 6 cases, un désert à partir de 30).
//! 3. **Sous-régions** : une région de plus de 150 cases est découpée en morceaux d'une centaine
//!    de cases (comme une chaîne en massifs), nommés d'après leur position dans la région
//!    (« Forêt Noire du Nord »).

use crate::hex::Wrap;
use crate::map::Terrain;
use crate::mountains::{components, split};
use crate::seas::{Grid, shape};
use crate::toponymy::Etymology;

/// Groupe de biomes d'une région naturelle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Biome {
    Forest,
    Jungle,
    Taiga,
    Desert,
    Steppe,
    Savanna,
    Scrub,
    Marsh,
    Tundra,
}

impl Biome {
    /// Groupe d'un terrain (aucun pour l'eau et la neige).
    pub fn of(terrain: Terrain) -> Option<Biome> {
        Some(match terrain {
            Terrain::Forest => Biome::Forest,
            Terrain::Jungle => Biome::Jungle,
            Terrain::Taiga => Biome::Taiga,
            Terrain::Desert => Biome::Desert,
            Terrain::Steppe | Terrain::Plains | Terrain::Grassland => Biome::Steppe,
            Terrain::Savanna | Terrain::WoodedSavanna => Biome::Savanna,
            Terrain::Mediterranean => Biome::Scrub,
            Terrain::Marsh => Biome::Marsh,
            Terrain::Tundra => Biome::Tundra,
            Terrain::Ocean | Terrain::Coast | Terrain::Lake | Terrain::Snow => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Biome::Forest => "forêt",
            Biome::Jungle => "jungle",
            Biome::Taiga => "taïga",
            Biome::Desert => "désert",
            Biome::Steppe => "steppe ou plaines",
            Biome::Savanna => "savane",
            Biome::Scrub => "maquis",
            Biome::Marsh => "marais",
            Biome::Tundra => "toundra",
        }
    }

    /// Plus petite région nommée de ce groupe (cases).
    fn min_cells(self) -> usize {
        match self {
            Biome::Marsh => 6,
            Biome::Scrub => 15,
            Biome::Forest | Biome::Jungle | Biome::Taiga => 20,
            Biome::Savanna | Biome::Tundra => 25,
            Biome::Desert => 30,
            Biome::Steppe => 40,
        }
    }

    const ALL: [Biome; 9] = [Biome::Forest, Biome::Jungle, Biome::Taiga, Biome::Desert, Biome::Steppe, Biome::Savanna, Biome::Scrub, Biome::Marsh, Biome::Tundra];
}

/// Une région naturelle (ou une sous-région).
#[derive(Clone, Debug)]
pub struct Region {
    pub biome: Biome,
    /// Ses cases, avec la distance de chacune au bord de la région (en cases).
    pub cells: Vec<usize>,
    pub inner: Vec<u16>,
    /// Sous-région : sa région.
    pub parent: Option<usize>,
    /// Sous-région : sa position dans la région (« du Nord », « du Centre »…).
    pub side: Option<&'static str>,
    /// Demi-largeur d'ouest en est (unités du monde 2D).
    pub half_width: f32,
    pub name: String,
    pub feminine: bool,
    pub etymology: Option<Etymology>,
}

/// Régions naturelles d'une carte.
#[derive(Clone, Debug, Default)]
pub struct Regions {
    pub zones: Vec<Region>,
    /// Région de chaque case.
    pub region_of: Vec<Option<u16>>,
    /// Sous-région de chaque case.
    pub part_of: Vec<Option<u16>>,
}

impl Regions {
    /// Régions d'une case, de la plus précise à la plus vaste (sous-région, région).
    pub fn chain(&self, cell: usize) -> Vec<usize> {
        [&self.part_of, &self.region_of].iter().filter_map(|v| v.get(cell).copied().flatten()).map(|z| z as usize).collect()
    }
}

/// Lissage : une case rejoint le groupe d'au moins tant de ses six voisines (deux passes).
const SMOOTH_NEIGHBORS: usize = 4;
/// Une région de plus de tant de cases est découpée en sous-régions d'environ `PART_CELLS` cases ;
/// un morceau de moins de `PART_MIN_CELLS` rejoint son voisin.
pub const SPLIT_CELLS: usize = 150;
const PART_CELLS: usize = 110;
const PART_MIN_CELLS: usize = 40;

/// Régions naturelles. `terrain` : terrain de chaque case.
pub fn compute(width: i32, height: i32, wrap: Wrap, terrain: &[Terrain]) -> Regions {
    let grid = Grid { width, height, wrap };
    let n = terrain.len();
    let mut out = Regions { zones: Vec::new(), region_of: vec![None; n], part_of: vec![None; n] };
    // 1) Groupe de chaque case, lissé (sur la terre seulement)
    let mut group: Vec<Option<Biome>> = terrain.iter().map(|&t| Biome::of(t)).collect();
    for _ in 0..2 {
        let before = group.clone();
        for c in 0..n {
            if terrain[c].is_water() {
                continue;
            }
            for b in Biome::ALL {
                if before[c] != Some(b) && grid.neighbors(c).filter(|&j| before[j] == Some(b)).count() >= SMOOTH_NEIGHBORS {
                    group[c] = Some(b);
                    break;
                }
            }
        }
    }
    // 2) Régions, les plus grandes d'abord
    let mut found: Vec<(Biome, Vec<usize>)> = Vec::new();
    for b in Biome::ALL {
        for cells in components(&grid, |c| group[c] == Some(b)) {
            if cells.len() >= b.min_cells() {
                found.push((b, cells));
            }
        }
    }
    found.sort_by_key(|(_, cells)| (std::cmp::Reverse(cells.len()), cells[0]));
    let mut parts: Vec<(usize, Vec<usize>)> = Vec::new();
    for (biome, cells) in found {
        let id = out.zones.len();
        for &c in &cells {
            out.region_of[c] = Some(id as u16);
        }
        if cells.len() > SPLIT_CELLS {
            let mut member = vec![false; n];
            for &c in &cells {
                member[c] = true;
            }
            let k = (cells.len() as f32 / PART_CELLS as f32).round().max(2.0) as usize;
            for part in split(&grid, &cells, k, &member, PART_MIN_CELLS) {
                parts.push((id, part));
            }
        }
        out.zones.push(zone(&grid, biome, cells, None, &out.region_of));
    }
    // 3) Sous-régions, nommées d'après leur position dans la région
    let mut by_region: Vec<Vec<usize>> = vec![Vec::new(); out.zones.len()];
    for (region, cells) in parts {
        let id = out.zones.len();
        for &c in &cells {
            out.part_of[c] = Some(id as u16);
        }
        let biome = out.zones[region].biome;
        out.zones.push(zone(&grid, biome, cells, Some(region), &out.part_of));
        by_region[region].push(id);
    }
    for (region, subs) in by_region.into_iter().enumerate() {
        if subs.is_empty() {
            continue;
        }
        let center = centroid(&grid, &out.zones[region].cells);
        let radius = (out.zones[region].cells.len() as f32 / std::f32::consts::PI).sqrt();
        // Écart de chaque morceau au centre de la région ; les plus excentrés choisissent d'abord
        let mut offsets: Vec<(usize, f32, f32)> = subs
            .iter()
            .map(|&z| {
                let (x, y) = centroid(&grid, &out.zones[z].cells);
                let mut dx = x - center.0;
                if wrap.wraps() {
                    dx -= (dx / width as f32).round() * width as f32;
                }
                (z, dx, (y - center.1) * 0.87)
            })
            .collect();
        offsets.sort_by(|a, b| (b.1.hypot(b.2)).total_cmp(&a.1.hypot(a.2)).then(a.0.cmp(&b.0)));
        let mut used: Vec<&'static str> = Vec::new();
        for (z, dx, dy) in offsets {
            // (direction déjà prise : la plus proche encore libre, à 90° au plus ; sinon pas de
            // position, le morceau aura son propre nom)
            if let Some(side) = directions(dx, dy, radius).into_iter().find(|s| !used.contains(s)) {
                used.push(side);
                out.zones[z].side = Some(side);
            }
        }
    }
    out
}

/// Centre d'un ensemble de cases (colonne, ligne), d'un seul tenant malgré le raccord est-ouest.
fn centroid(grid: &Grid, cells: &[usize]) -> (f32, f32) {
    let w = grid.width as f32;
    let (mut s, mut c, mut y) = (0.0f32, 0.0f32, 0.0f32);
    for &cell in cells {
        let a = (cell as i32 % grid.width) as f32 / w * std::f32::consts::TAU;
        s += a.sin();
        c += a.cos();
        y += (cell as i32 / grid.width) as f32;
    }
    let x = s.atan2(c).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * w;
    (x, y / cells.len() as f32)
}

/// Positions possibles d'un morceau dans sa région d'après l'écart de son centre (colonnes,
/// lignes vers le sud), de la meilleure à la moins bonne : au centre s'il en est proche, sinon la
/// direction la plus proche parmi huit, puis ses voisines jusqu'à 90°.
fn directions(dx: f32, dy: f32, radius: f32) -> Vec<&'static str> {
    if dx.hypot(dy) < 0.3 * radius {
        return vec!["du Centre"];
    }
    // Angle depuis l'est, dans le sens inverse des aiguilles d'une montre (le nord en haut)
    let angle = (-dy).atan2(dx).to_degrees().rem_euclid(360.0);
    const SIDES: [&str; 8] = ["de l'Est", "du Nord-Est", "du Nord", "du Nord-Ouest", "de l'Ouest", "du Sud-Ouest", "du Sud", "du Sud-Est"];
    let mut order: Vec<usize> = (0..8).collect();
    let gap = |k: usize| {
        let d = (angle - k as f32 * 45.0).rem_euclid(360.0);
        d.min(360.0 - d)
    };
    order.sort_by(|&a, &b| gap(a).total_cmp(&gap(b)));
    order.into_iter().filter(|&k| gap(k) <= 90.0).map(|k| SIDES[k]).collect()
}

fn zone(grid: &Grid, biome: Biome, cells: Vec<usize>, parent: Option<usize>, owner: &[Option<u16>]) -> Region {
    let (inner, half_width) = shape(grid, &cells, owner);
    Region { biome, cells, inner, parent, side: None, half_width, name: String::new(), feminine: false, etymology: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foret_avec_clairiere_et_sous_regions() {
        // Une prairie de 60 × 30 cases ; une forêt de 40 × 20 cases à l'ouest, percée d'une
        // clairière d'une case ; un petit marais de 3 × 3 cases à l'est ; un désert trop petit
        let (w, h) = (60, 30);
        let terrain: Vec<Terrain> = (0..w * h)
            .map(|c| {
                let (x, y) = (c % w, c / w);
                if (2..42).contains(&x) && (5..25).contains(&y) && (x, y) != (20, 15) {
                    Terrain::Forest
                } else if (50..53).contains(&x) && (10..13).contains(&y) {
                    Terrain::Marsh
                } else if (50..53).contains(&x) && (20..23).contains(&y) {
                    Terrain::Desert
                } else {
                    Terrain::Grassland
                }
            })
            .collect();
        let r = compute(w, h, Wrap::NONE, &terrain);
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let forest = r.region_of[at(10, 10)].expect("une forêt");
        assert_eq!(r.zones[forest as usize].biome, Biome::Forest);
        assert_eq!(r.region_of[at(20, 15)], Some(forest), "la clairière est dans la forêt");
        // 800 cases : des sous-régions, l'ouest et l'est à des positions différentes
        let (west, east) = (r.part_of[at(4, 15)].expect("sous-région ouest"), r.part_of[at(40, 15)].expect("sous-région est"));
        assert_ne!(west, east);
        assert_eq!(r.zones[west as usize].parent, Some(forest as usize));
        assert!(r.zones[west as usize].side.is_some_and(|s| s.contains("Ouest")), "{:?}", r.zones[west as usize].side);
        assert!(r.zones[east as usize].side.is_some_and(|s| s.contains("Est")), "{:?}", r.zones[east as usize].side);
        // Le marais (9 cases) est une région, le désert (9 cases) non ; la prairie aussi
        assert_eq!(r.region_of[at(51, 11)].map(|z| r.zones[z as usize].biome), Some(Biome::Marsh));
        assert!(r.region_of[at(51, 21)].is_none_or(|z| r.zones[z as usize].biome != Biome::Desert));
        assert_eq!(r.region_of[at(55, 2)].map(|z| r.zones[z as usize].biome), Some(Biome::Steppe));
    }

    #[test]
    fn positions_dans_la_region() {
        assert_eq!(directions(0.0, -10.0, 5.0)[0], "du Nord");
        assert_eq!(directions(10.0, 0.0, 5.0)[0], "de l'Est");
        assert_eq!(directions(-7.0, 7.0, 5.0)[0], "du Sud-Ouest");
        assert_eq!(directions(0.5, 0.5, 5.0), vec!["du Centre"]);
        // Un peu au nord de l'est : l'est, puis le nord-est, puis le sud-est
        assert_eq!(&directions(10.0, -1.0, 5.0)[..3], &["de l'Est", "du Nord-Est", "du Sud-Est"]);
    }

    /// Bilan des régions naturelles : `cargo test bilan_regions -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bilan_regions() {
        for seed in [777, 4242, 2, 5, 31] {
            let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let r = m.regions();
            let top: Vec<&Region> = r.zones.iter().filter(|z| z.parent.is_none()).collect();
            let parts = r.zones.len() - top.len();
            let by_biome: Vec<String> = Biome::ALL.iter().map(|&b| format!("{} {}", b.label(), top.iter().filter(|z| z.biome == b).count())).collect();
            println!("graine {seed} : {} régions, {parts} sous-régions ({})", top.len(), by_biome.join(", "));
            for z in &r.zones {
                let parent = z.parent.map_or(String::new(), |p| format!(" (dans {})", r.zones[p].name));
                println!("  {:<18} {:<34} {:>5} cases{parent}", z.biome.label(), z.name, z.cells.len());
            }
        }
    }
}
