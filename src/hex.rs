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
