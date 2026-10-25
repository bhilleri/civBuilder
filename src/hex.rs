//! Coordonnées hexagonales axiales (orientation « pointe en haut »).
//!
//! Référence : https://www.redblobgames.com/grids/hexagons/
//! Ce module est 100 % logique pure : aucune dépendance au rendu.

use std::ops::{Add, Sub};

/// Rayon (centre → sommet) d'un hexagone, en unités monde.
pub const HEX_SIZE: f32 = 32.0;

const SQRT_3: f32 = 1.732_050_8;

/// Coordonnée axiale (q, r). La troisième coordonnée cubique est s = -q - r.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
}

/// Les 6 directions, dans l'ordre : E, NE, NO, O, SO, SE.
pub const DIRECTIONS: [Hex; 6] = [
    Hex { q: 1, r: 0 },
    Hex { q: 1, r: -1 },
    Hex { q: 0, r: -1 },
    Hex { q: -1, r: 0 },
    Hex { q: -1, r: 1 },
    Hex { q: 0, r: 1 },
];

impl Hex {
    pub const fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    pub const fn s(self) -> i32 {
        -self.q - self.r
    }

    pub fn neighbors(self) -> [Hex; 6] {
        DIRECTIONS.map(|d| self + d)
    }

    pub fn distance(self, other: Hex) -> i32 {
        let d = self - other;
        (d.q.abs() + d.r.abs() + d.s().abs()) / 2
    }

    /// Tous les hexagones à une distance <= `radius` (centre inclus).
    pub fn range(self, radius: i32) -> impl Iterator<Item = Hex> {
        (-radius..=radius).flat_map(move |dq| {
            let r_min = (-radius).max(-dq - radius);
            let r_max = radius.min(-dq + radius);
            (r_min..=r_max).map(move |dr| self + Hex::new(dq, dr))
        })
    }

    /// Centre de l'hexagone en coordonnées monde (y vers le haut, r croît vers le bas).
    pub fn to_world(self) -> (f32, f32) {
        let x = HEX_SIZE * SQRT_3 * (self.q as f32 + self.r as f32 / 2.0);
        let y = -HEX_SIZE * 1.5 * self.r as f32;
        (x, y)
    }

    /// Hexagone contenant le point monde (x, y).
    pub fn from_world(x: f32, y: f32) -> Hex {
        let y = -y;
        let q = (SQRT_3 / 3.0 * x - y / 3.0) / HEX_SIZE;
        let r = (2.0 / 3.0 * y) / HEX_SIZE;
        Self::round(q, r)
    }

    /// Arrondi d'une coordonnée axiale fractionnaire vers l'hexagone le plus proche.
    fn round(q: f32, r: f32) -> Hex {
        let s = -q - r;
        let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
        let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
        if dq > dr && dq > ds {
            rq = -rr - rs;
        } else if dr > ds {
            rr = -rq - rs;
        }
        Hex::new(rq as i32, rr as i32)
    }

    /// Les deux sommets (monde) de l'arête partagée avec le voisin dans la direction `dir`.
    pub fn edge_corners(self, dir: usize) -> ((f32, f32), (f32, f32)) {
        // Sommets d'un hexagone « pointe en haut » : angle 30° + 60°·i (i=0 → bas-droite... ).
        // Direction E (0) ↔ sommets à -30° et +30°, NE (1) ↔ +30° et +90°, etc.
        let (cx, cy) = self.to_world();
        let corner = |deg: f32| {
            let a = deg.to_radians();
            (cx + HEX_SIZE * a.cos(), cy + HEX_SIZE * a.sin())
        };
        let start = -30.0 + 60.0 * dir as f32;
        (corner(start), corner(start + 60.0))
    }

    /// Conversion depuis des coordonnées « offset odd-r » (colonne, ligne) d'une carte rectangulaire.
    pub fn from_offset(col: i32, row: i32) -> Hex {
        Hex::new(col - (row - (row & 1)) / 2, row)
    }

    pub fn to_offset(self) -> (i32, i32) {
        (self.q + (self.r - (self.r & 1)) / 2, self.r)
    }
}

/// Largeur (monde) d'une colonne de cases.
pub const COLUMN_WIDTH: f32 = HEX_SIZE * SQRT_3;

impl Hex {
    /// La même case décalée de `cols` colonnes vers l'est (même ligne).
    pub const fn shifted(self, cols: i32) -> Hex {
        Hex::new(self.q + cols, self.r)
    }
}

/// Bouclage est-ouest d'une carte de `width` colonnes : la colonne `width` est la colonne 0
/// (planète). `width == 0` : pas de tour, la carte a des bords à l'est et à l'ouest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wrap {
    pub width: i32,
}

impl Wrap {
    /// Pas de tour (carte à bords).
    pub const NONE: Wrap = Wrap { width: 0 };

    pub const fn new(width: i32, wraps: bool) -> Self {
        Wrap { width: if wraps { width } else { 0 } }
    }

    pub const fn wraps(self) -> bool {
        self.width > 0
    }

    /// La case ramenée dans les colonnes 0 à `width − 1` (inchangée sans tour).
    pub fn hex(self, hex: Hex) -> Hex {
        if self.width <= 0 {
            return hex;
        }
        let (col, _) = hex.to_offset();
        let k = col.div_euclid(self.width);
        if k == 0 { hex } else { hex.shifted(-k * self.width) }
    }

    /// La copie de `to` la plus proche de `from` (hors de la carte si le plus court chemin passe
    /// par le raccord) : pour les directions, les tracés, la distance.
    pub fn nearest(self, from: Hex, to: Hex) -> Hex {
        if self.width <= 0 {
            return to;
        }
        let (ca, _) = from.to_offset();
        let (cb, _) = to.to_offset();
        let k = ((ca - cb) as f32 / self.width as f32).round() as i32;
        (k - 1..=k + 1).map(|k| to.shifted(k * self.width)).min_by_key(|&h| from.distance(h)).unwrap_or(to)
    }

    /// Distance en cases, par le plus court côté.
    pub fn distance(self, a: Hex, b: Hex) -> i32 {
        a.distance(self.nearest(a, b))
    }

    /// Les 6 voisines, ramenées dans la carte.
    pub fn neighbors(self, hex: Hex) -> [Hex; 6] {
        hex.neighbors().map(|h| self.hex(h))
    }

    /// Voisine dans la direction `dir`, ramenée dans la carte.
    pub fn neighbor(self, hex: Hex, dir: usize) -> Hex {
        self.hex(hex + DIRECTIONS[dir % 6])
    }

    /// Direction (0 à 5) de `from` vers sa voisine `to`.
    pub fn direction(self, from: Hex, to: Hex) -> Option<usize> {
        let to = self.nearest(from, to);
        DIRECTIONS.iter().position(|&d| from + d == to)
    }

    /// Cases à distance ≤ `radius`, ramenées dans la carte (sans doublon tant que le rayon est
    /// petit devant la largeur).
    pub fn range(self, center: Hex, radius: i32) -> impl Iterator<Item = Hex> {
        center.range(radius).map(move |h| self.hex(h))
    }

    /// Décalage (monde, en x) de cette copie de la case par rapport à la case dans la carte :
    /// 0 dans la carte, ± un tour au-delà du raccord.
    pub fn offset(self, hex: Hex) -> f32 {
        (hex.q - self.hex(hex).q) as f32 * COLUMN_WIDTH
    }

    /// Largeur du monde (unités monde) : un tour complet.
    pub fn world_width(self) -> f32 {
        self.width as f32 * COLUMN_WIDTH
    }
}

impl Add for Hex {
    type Output = Hex;
    fn add(self, o: Hex) -> Hex {
        Hex::new(self.q + o.q, self.r + o.r)
    }
}

impl Sub for Hex {
    type Output = Hex;
    fn sub(self, o: Hex) -> Hex {
        Hex::new(self.q - o.q, self.r - o.r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_voisins() {
        let h = Hex::new(3, -2);
        for n in h.neighbors() {
            assert_eq!(h.distance(n), 1);
        }
        assert_eq!(Hex::new(0, 0).distance(Hex::new(3, -1)), 3);
    }

    #[test]
    fn range_compte() {
        // 1 + 6 + 12 = 19 hexagones dans un rayon 2
        assert_eq!(Hex::new(5, 5).range(2).count(), 19);
        assert!(Hex::new(5, 5).range(2).all(|h| h.distance(Hex::new(5, 5)) <= 2));
    }

    #[test]
    fn aller_retour_monde() {
        for q in -10..10 {
            for r in -10..10 {
                let h = Hex::new(q, r);
                let (x, y) = h.to_world();
                assert_eq!(Hex::from_world(x, y), h);
                // un point légèrement décalé reste dans le même hexagone
                assert_eq!(Hex::from_world(x + 10.0, y - 10.0), h);
            }
        }
    }

    #[test]
    fn aller_retour_offset() {
        for col in 0..20 {
            for row in 0..20 {
                assert_eq!(Hex::from_offset(col, row).to_offset(), (col, row));
            }
        }
    }

    #[test]
    fn tour_est_ouest() {
        let w = Wrap::new(20, true);
        // Une colonne au-delà du bord est revient à l'ouest, et inversement
        assert_eq!(w.hex(Hex::from_offset(20, 3)), Hex::from_offset(0, 3));
        assert_eq!(w.hex(Hex::from_offset(-1, 4)), Hex::from_offset(19, 4));
        // Les voisines d'une case du bord est sont dans la carte
        for row in 0..6 {
            let h = Hex::from_offset(19, row);
            for n in w.neighbors(h) {
                let (c, _) = n.to_offset();
                assert!((0..20).contains(&c));
                assert_eq!(w.distance(h, n), 1);
                assert_eq!(w.direction(h, n).map(|d| w.neighbor(h, d)), Some(n));
            }
        }
        // Le plus court chemin passe par le raccord
        assert_eq!(w.distance(Hex::from_offset(1, 2), Hex::from_offset(18, 2)), 3);
        assert_eq!(Wrap::NONE.distance(Hex::from_offset(1, 2), Hex::from_offset(18, 2)), 17);
        assert_eq!(w.range(Hex::from_offset(0, 5), 2).count(), 19);
        assert!(w.range(Hex::from_offset(0, 5), 2).all(|h| (0..20).contains(&h.to_offset().0)));
    }

    #[test]
    fn arete_partagee() {
        // L'arête vers un voisin doit être la même vue des deux côtés.
        let h = Hex::new(2, 3);
        for (dir, &d) in DIRECTIONS.iter().enumerate() {
            let n = h + d;
            let (a, b) = h.edge_corners(dir);
            let (c, d) = n.edge_corners((dir + 3) % 6);
            let close = |p: (f32, f32), q: (f32, f32)| (p.0 - q.0).abs() < 0.01 && (p.1 - q.1).abs() < 0.01;
            assert!(close(a, d) && close(b, c), "direction {dir}");
        }
    }
}
