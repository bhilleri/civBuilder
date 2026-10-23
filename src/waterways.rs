//! Formes des cours d'eau à dessiner (géométrie 2D pure, partagée par les vues 2D et 3D).
//!
//! Pour chaque case : les rivières de ses arêtes (une arête est dessinée par une seule des deux
//! cases, celle pour qui elle est dans les directions 0 à 2), le fleuve qui l'occupe (une large
//! bande d'eau du centre vers chaque arête où il continue, berges de part et d'autre) et son
//! petit lac. Les triangles sont découpés finement pour épouser le relief en 3D.

use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::hydro::RIVER_M3S;
use crate::map::WorldMap;
use bevy::math::Vec2;

/// Demi-largeur de la bande d'eau d'un fleuve (unités monde ; l'arête mesure `HEX_SIZE`).
pub const FLEUVE_HALF_WIDTH: f32 = HEX_SIZE * 0.46;
/// Petit lac : décalage depuis le centre (le centre reste libre pour les unités) et rayons.
const SMALL_LAKE_AT: Vec2 = Vec2::new(10.0, 12.0);
const SMALL_LAKE_RADII: Vec2 = Vec2::new(8.5, 5.5);
/// Découpage des bandes (segments par arête ou par demi-case).
const STEPS: usize = 6;

/// Nature d'un morceau d'eau (couleurs distinctes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterKind {
    River,
    Fleuve,
    Lake,
}

/// Réglages 3D d'un type d'eau : la hauteur de l'eau est le terrain le plus bas autour de l'axe
/// (au centre et sur deux cercles de rayons `near` et `far`) moins `below` ; le lit descend de
/// `bed` sous l'eau.
#[derive(Clone, Copy, Debug)]
pub struct Bed {
    pub near: f32,
    pub far: f32,
    pub below: f32,
    pub bed: f32,
}

impl WaterKind {
    pub fn bed(self) -> Bed {
        match self {
            WaterKind::River => Bed { near: 3.0, far: 6.0, below: 1.1, bed: 1.4 },
            WaterKind::Fleuve => Bed { near: 7.0, far: 15.0, below: 1.5, bed: 2.2 },
            WaterKind::Lake => Bed { near: 4.5, far: 9.0, below: 0.9, bed: 1.2 },
        }
    }
}

/// Axe d'un tronçon d'eau (coordonnées monde 2D) : segment `a`–`b` (un disque si a = b) de
/// demi-largeur `hw`. `inflow` : affluent qui entre dans un fleuve (sa hauteur rejoint
/// progressivement celle du fleuve).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterAxis {
    pub a: Vec2,
    pub b: Vec2,
    pub hw: f32,
    pub kind: WaterKind,
    pub also: Option<Hex>,
    pub inflow: bool,
}

/// Triangle d'eau (coordonnées monde 2D). Une rivière d'arête se voit aussi depuis la case
/// d'en face (`also`) : elle apparaît dès que l'une des deux rives est explorée. `channel` :
/// indice du lit auquel il appartient.
pub struct WaterTri {
    pub pts: [Vec2; 3],
    pub kind: WaterKind,
    pub also: Option<Hex>,
    pub channel: usize,
}

/// Triangles d'eau d'une case, et lits correspondants (pour creuser le relief en 3D).
#[derive(Default)]
pub struct WaterShape {
    pub tris: Vec<WaterTri>,
    pub channels: Vec<WaterAxis>,
    /// Case d'en face des prochains triangles ajoutés, et affluent d'un fleuve.
    also: Option<Hex>,
    inflow: bool,
}

/// Demi-largeur d'une rivière selon son débit : elle s'élargit avec le logarithme du débit.
pub fn river_half_width(q: f32) -> f32 {
    (1.3 + 1.7 * (q / RIVER_M3S).log10().max(0.0)).min(5.0)
}

impl WaterShape {
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Triangles visibles quand les cases explorées sont celles pour qui `explored` répond oui
    /// (la case elle-même est `hex`).
    pub fn visible<'a>(&'a self, hex: Hex, explored: impl Fn(Hex) -> bool + 'a) -> impl Iterator<Item = &'a WaterTri> + 'a {
        let own = explored(hex);
        self.tris.iter().filter(move |t| own || t.also.is_some_and(&explored))
    }

    /// Lits visibles (mêmes règles que les triangles).
    pub fn visible_channels<'a>(&'a self, hex: Hex, explored: impl Fn(Hex) -> bool + 'a) -> impl Iterator<Item = &'a WaterAxis> + 'a {
        let own = explored(hex);
        self.channels.iter().filter(move |c| own || c.also.is_some_and(&explored))
    }

    fn tri(&mut self, pts: [Vec2; 3], kind: WaterKind) {
        let channel = self.channels.len() - 1;
        self.tris.push(WaterTri { pts, kind, also: self.also, channel });
    }

    /// Enregistre le lit d'un tronçon.
    fn channel(&mut self, a: Vec2, b: Vec2, hw: f32, kind: WaterKind) {
        self.channels.push(WaterAxis { a, b, hw, kind, also: self.also, inflow: self.inflow });
    }

    /// Bande droite de `a` à `b` (demi-largeur `hw`), découpée en `STEPS` tronçons.
    fn band(&mut self, a: Vec2, b: Vec2, hw: f32, kind: WaterKind) {
        self.channel(a, b, hw, kind);
        let n = (b - a).perp().normalize_or_zero() * hw;
        // Les bandes larges sont aussi découpées en travers (elles épousent mieux le relief)
        let across = if hw > 6.0 { 4 } else { 1 };
        let at = |k: usize, j: usize| a.lerp(b, k as f32 / STEPS as f32) + n * (2.0 * j as f32 / across as f32 - 1.0);
        for k in 0..STEPS {
            for j in 0..across {
                self.tri([at(k, j), at(k + 1, j), at(k + 1, j + 1)], kind);
                self.tri([at(k, j), at(k + 1, j + 1), at(k, j + 1)], kind);
            }
        }
    }

    /// Disque (polygone) de centre `c` et de rayons `r`.
    fn disc(&mut self, c: Vec2, r: Vec2, kind: WaterKind) {
        const SIDES: usize = 12;
        // Lit : segment le long du grand axe (une ellipse en largeur, un disque si r.x = r.y)
        let half = Vec2::X * (r.x - r.y).max(0.0);
        self.channel(c - half, c + half, r.y, kind);
        let at = |k: usize| {
            let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
            c + Vec2::new(a.cos() * r.x, a.sin() * r.y)
        };
        for k in 0..SIDES {
            self.tri([c, at(k), at(k + 1)], kind);
        }
    }
}

fn v((x, y): (f32, f32)) -> Vec2 {
    Vec2::new(x, y)
}

fn center(hex: Hex) -> Vec2 {
    v(hex.to_world())
}

/// Coin j d'une case (à −30° + 60°·j).
fn corner(hex: Hex, j: usize) -> Vec2 {
    center(hex) + Vec2::from_angle((-30.0 + 60.0 * j as f32).to_radians()) * HEX_SIZE
}

/// Formes d'eau d'une case.
pub fn hex_water(map: &WorldMap, hex: Hex) -> WaterShape {
    let mut shape = WaterShape::default();
    // Rivières des arêtes « à soi » (directions 0 à 2), avec un rond à chaque bout pour
    // raccorder les arêtes successives
    for (d, &step) in DIRECTIONS.iter().enumerate().take(3) {
        let q = map.river_flow(hex, d);
        if q <= 0.0 {
            continue;
        }
        let ((ax, ay), (bx, by)) = hex.edge_corners(d);
        let (a, b) = (Vec2::new(ax, ay), Vec2::new(bx, by));
        let hw = river_half_width(q);
        shape.also = Some(hex + step);
        shape.band(a, b, hw, WaterKind::River);
        shape.disc(a, Vec2::splat(hw), WaterKind::River);
        shape.disc(b, Vec2::splat(hw), WaterKind::River);
        shape.also = None;
    }
    if map.is_fleuve(hex) {
        let c = center(hex);
        let links = map.fleuve_links(hex);
        shape.disc(c, Vec2::splat(FLEUVE_HALF_WIDTH), WaterKind::Fleuve);
        for d in 0..6 {
            if links & (1 << d) != 0 {
                let ((ax, ay), (bx, by)) = hex.edge_corners(d);
                let mid = (Vec2::new(ax, ay) + Vec2::new(bx, by)) / 2.0;
                // Léger débord sur la case voisine : pas de liseré entre deux tronçons
                let mid = mid + (mid - c).normalize_or_zero() * 0.6;
                shape.band(c, mid, FLEUVE_HALF_WIDTH, WaterKind::Fleuve);
            }
        }
        // Affluents (rivières d'arête) qui arrivent à un coin : ils rejoignent le fleuve
        for j in 0..6 {
            let (a, b) = (hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]);
            let Some(dir) = DIRECTIONS.iter().position(|&dd| a + dd == b) else { continue };
            let q = map.river_flow(a, dir);
            if q > 0.0 {
                let k = corner(hex, j);
                shape.inflow = true;
                shape.band(k, k.lerp(c, 0.6), river_half_width(q), WaterKind::River);
                shape.inflow = false;
            }
        }
    }
    if map.small_lake(hex) && !map.is_fleuve(hex) {
        shape.disc(center(hex) + SMALL_LAKE_AT, SMALL_LAKE_RADII, WaterKind::Lake);
    }
    shape
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;

    #[test]
    fn une_arete_dessinee_une_seule_fois() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set_river(h, 4, 50.0); // arête vers le sud-ouest : dessinée par la voisine
        assert!(hex_water(&map, h).is_empty());
        assert!(!hex_water(&map, h + DIRECTIONS[4]).is_empty());
    }

    #[test]
    fn largeur_croissante() {
        assert!(river_half_width(RIVER_M3S) < river_half_width(200.0));
        assert!(river_half_width(200.0) < FLEUVE_HALF_WIDTH);
    }
}

#[cfg(test)]
mod diag {
    #[test]
    #[ignore]
    fn fleuves_planete() {
        use super::*;
        let m = WorldMap::generate_in(192, 111, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let f: Vec<Hex> = m.hexes().filter(|&h| m.is_fleuve(h)).collect();
        println!("fleuves {}", f.len());
        for h in f.iter().take(10) {
            println!("{:?} q {:.0} links {:06b} tris {} terrain {:?}", h.to_offset(), m.discharge(*h), m.fleuve_links(*h), hex_water(&m, *h).tris.len(), m.get(*h));
        }
    }
}
