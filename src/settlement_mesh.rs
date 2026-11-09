//! Maillage d'une ville ou d'un village (`settlements::Layout`) : un seul maillage par site, couleurs
//! portées par les sommets. Le bas des murs suit le sol point par point : pas de remblai.
//!
//! Repère du maillage : origine sur le sol au centre de la case, x vers l'est, y vers le haut,
//! z vers le sud (z = −y du repère 2D de la case).

use crate::relief::MeshData;
use crate::settlements::{Bridge, Building, BuildingKind, CampKind, CampLayout, Family, GALLERY, GATE_DEPTH, Gate, LANDING, LIMIT_SAMPLES, Layout, Pier, Pond, rect};
use bevy::color::{Color, ColorToComponents, LinearRgba};
use bevy::math::{Vec2, Vec3};
use std::f32::consts::{FRAC_PI_4, PI, TAU};

type Rgb = Vec3;

fn srgb(hex: u32) -> Rgb {
    let c = |shift: u32| ((hex >> shift) & 0xFF) as f32 / 255.0;
    let l: LinearRgba = Color::srgb(c(16), c(8), c(0)).to_linear();
    Vec3::from_array(l.to_vec3().to_array())
}

/// Couleurs d'une famille : murs, toit.
fn palette(f: Family) -> (Rgb, Rgb) {
    let (w, r) = match f {
        Family::Forest => (0x8A6A47, 0x5E5346),
        Family::Prairie => (0xE0D6B8, 0xC6AB5E),
        Family::Boreal => (0x6A4B32, 0x40382F),
        Family::Arid => (0xC49A63, 0xD3AD78),
        Family::Cold => (0x807D70, 0x6F7C55),
        Family::Tropical => (0x94744A, 0xA8A04F),
        Family::Mediterranean => (0xD6CFBD, 0xB5613B),
        Family::Savanna => (0xA5774C, 0xD0B26A),
        Family::Steppe => (0xE7E0CC, 0xD7CEB6),
        Family::Marsh => (0xC4B47C, 0xCDBD83),
    };
    (srgb(w), srgb(r))
}

/// Terre battue et chemins d'une famille : proche du sol local, pour ne pas faire tache.
pub fn earth_color(f: Family) -> (Rgb, Rgb) {
    let (ground, trail) = match f {
        Family::Forest => (0x6E6443, 0x8F8360),
        Family::Prairie => (0x8D7F58, 0xA69970),
        Family::Boreal => (0x5F5A48, 0x7D7660),
        Family::Arid => (0xA88D5F, 0xBDA47A),
        Family::Cold => (0x8D8A78, 0xA5A290),
        Family::Tropical => (0x6B5F3C, 0x8B7E58),
        Family::Mediterranean => (0x94855E, 0xAD9F7A),
        Family::Savanna => (0x9A8450, 0xB19C6A),
        Family::Steppe => (0x8F875C, 0xA79F76),
        Family::Marsh => (0x6F6F58, 0x8A8A70),
    };
    (srgb(ground), srgb(trail))
}

/// Couleur de la muraille.
fn wall_color(f: Family) -> Rgb {
    srgb(match f {
        Family::Forest => 0x6F5A43,
        Family::Prairie => 0x9A9488,
        Family::Boreal => 0x5C4230,
        Family::Arid => 0xB08755,
        Family::Cold => 0x6B6D62,
        Family::Tropical | Family::Marsh => 0x6D4D30,
        Family::Mediterranean => 0xB1AA9A,
        Family::Savanna => 0x9B7147,
        Family::Steppe => 0x8A7350,
    })
}

/// Sol vu par le maillage : hauteur au point 2D de la case, par rapport au centre.
struct Baker<'a> {
    data: MeshData,
    ground: &'a dyn Fn(Vec2) -> f32,
    player: Rgb,
    /// Marais : niveau des planchers et des passerelles.
    deck: Option<f32>,
}

fn p3(p: Vec2, h: f32) -> Vec3 {
    Vec3::new(p.x, h, -p.y)
}

impl Baker<'_> {
    fn g(&self, p: Vec2) -> f32 {
        (self.ground)(p)
    }

    /// Face plane (polygone convexe), tournée à l'opposé du point `inside`.
    fn face(&mut self, poly: &[Vec3], c: Rgb, inside: Vec3) {
        let n = (poly[1] - poly[0]).cross(poly[2] - poly[0]).normalize_or_zero();
        let mid = poly.iter().copied().sum::<Vec3>() / poly.len() as f32;
        let flip = n.dot(mid - inside) < 0.0;
        let n = if flip { -n } else { n };
        let base = self.data.positions.len() as u32;
        for p in poly {
            self.data.positions.push(p.to_array());
            self.data.normals.push(n.to_array());
            self.data.colors.push([c.x, c.y, c.z, 1.0]);
        }
        for k in 1..poly.len() as u32 - 1 {
            if flip {
                self.data.indices.extend([base, base + k + 1, base + k]);
            } else {
                self.data.indices.extend([base, base + k, base + k + 1]);
            }
        }
    }

    /// Volume droit entre deux contours de même taille : le haut, le bas et les côtés.
    fn body(&mut self, low: &[Vec3], up: &[Vec3], c: Rgb) {
        let inside = (low.iter().chain(up).copied().sum::<Vec3>()) / (low.len() + up.len()) as f32;
        self.face(up, c, inside);
        for i in 0..low.len() {
            let j = (i + 1) % low.len();
            self.face(&[low[i], low[j], up[j], up[i]], c, inside);
        }
    }

    /// Volume dont le bas suit le sol et le haut est à la hauteur `top`.
    fn solid(&mut self, poly: &[Vec2], top: f32, c: Rgb) {
        let low: Vec<Vec3> = poly.iter().map(|&p| p3(p, self.g(p) - 0.05)).collect();
        let up: Vec<Vec3> = poly.iter().map(|&p| p3(p, top.max(self.g(p) + 0.02))).collect();
        self.body(&low, &up, c);
    }

    /// Volume horizontal entre deux hauteurs.
    fn prism(&mut self, poly: &[Vec2], z0: f32, z1: f32, c: Rgb) {
        let low: Vec<Vec3> = poly.iter().map(|&p| p3(p, z0)).collect();
        let up: Vec<Vec3> = poly.iter().map(|&p| p3(p, z1)).collect();
        self.body(&low, &up, c);
    }

    /// Cône (toit, tente), éventuellement posé sur le sol.
    fn cone(&mut self, at: Vec2, r: f32, z: f32, h: f32, c: Rgb, n: usize, follow: bool) {
        let ring: Vec<Vec3> = circle(at, r, n).iter().map(|&p| p3(p, if follow { self.g(p) - 0.05 } else { z })).collect();
        let apex = p3(at, z + h);
        let inside = p3(at, z + h * 0.3);
        for i in 0..n {
            self.face(&[ring[i], ring[(i + 1) % n], apex], c, inside);
        }
    }

    /// Toit à deux pentes (faîte selon `rot`) et ses pignons.
    fn gable(&mut self, at: Vec2, half: Vec2, rot: f32, z: f32, h: f32, roof: Rgb, wall: Rgb) {
        let [a, b, c, e] = rect(at, half, rot);
        let (r1, r2) = (p3((a + e) / 2.0, z + h), p3((b + c) / 2.0, z + h));
        let (a, b, c, e) = (p3(a, z), p3(b, z), p3(c, z), p3(e, z));
        let inside = p3(at, z + h * 0.3);
        self.face(&[a, b, r2, r1], roof, inside);
        self.face(&[e, c, r2, r1], roof, inside);
        self.face(&[a, e, r1], wall, inside);
        self.face(&[b, c, r2], wall, inside);
        // (dessous, vu depuis un débord)
        self.face(&[a, b, c, e], wall * 0.6, inside + Vec3::Y * h * 2.0);
    }

    fn post(&mut self, at: Vec2, r: f32, top: f32, c: Rgb) {
        self.solid(&circle(at, r, 5), top, c);
    }

    /// Porte sur un mur : sur la face tournée vers le sud-est (la caméra par défaut), ou vers `face`.
    fn door(&mut self, b: &Building, half: Vec2, base: f32, h: f32, face: Option<Vec2>) {
        let (s, c) = b.rot.sin_cos();
        let opts = [(Vec2::new(-s, c), half.y, b.rot), (Vec2::new(s, -c), half.y, b.rot), (Vec2::new(c, s), half.x, b.rot + PI / 2.0), (Vec2::new(-c, -s), half.x, b.rot + PI / 2.0)];
        let toward = face.map_or(Vec2::new(0.4, -1.0), |f| f - b.at).normalize_or_zero();
        let &(n, off, turn) = opts.iter().max_by(|a, b| a.0.dot(toward).total_cmp(&b.0.dot(toward))).unwrap_or(&opts[0]);
        let at = b.at + n * (off + 0.05);
        let z0 = self.g(at).max(base - 1.0);
        self.prism(&rect(at, Vec2::new(0.6, 0.09), turn), z0, base + h, srgb(0x33261B));
    }

    fn mean_under(&self, b: &Building) -> f32 {
        let corners = b.corners();
        (corners.iter().map(|&p| self.g(p)).sum::<f32>() + self.g(b.at)) / 5.0
    }

    fn building(&mut self, b: &Building, family: Family) {
        match b.kind {
            BuildingKind::Granary if self.deck.is_some() => {
                let z = self.deck.unwrap_or(0.0);
                let (wall, roof) = palette(family);
                for q in circle(b.at, 1.7, 4) {
                    self.post(q, 0.26, z, srgb(0x6D4D30));
                }
                self.prism(&circle(b.at, 2.2, 8), z - 0.25, z, srgb(0x6D4D30));
                self.prism(&circle(b.at, 1.8, 8), z, z + 2.6, wall.lerp(srgb(0x6D4D30), 0.3));
                self.cone(b.at, 2.5, z + 2.6, 2.4, roof, 8, false);
            }
            BuildingKind::Granary => {
                let base = self.mean_under(b);
                let (wall, roof) = palette(family);
                self.solid(&circle(b.at, 1.9, 8), base + 3.4, wall.lerp(srgb(0x6D4D30), 0.3));
                self.cone(b.at, 2.5, base + 3.4, 2.5, roof, 8, false);
            }
            BuildingKind::Market => {
                let base = self.deck.unwrap_or_else(|| self.mean_under(b));
                let (_, roof) = palette(family);
                if self.deck.is_some() {
                    self.prism(&rect(b.at, Vec2::new(3.8, 3.0), b.rot), base - 0.25, base, srgb(0x7D5C3B));
                }
                for q in rect(b.at, Vec2::new(3.3, 2.5), b.rot) {
                    self.post(q, 0.3, base + 2.6, srgb(0x6D4D30));
                }
                self.gable(b.at, Vec2::new(3.9, 3.1), b.rot, base + 2.6, 1.8, roof, srgb(0x6D4D30));
            }
            BuildingKind::House | BuildingKind::Hall => self.home(b, family),
        }
    }

    fn home(&mut self, b: &Building, family: Family) {
        let hall = b.kind == BuildingKind::Hall;
        let (wall, family_roof) = palette(family);
        let roof = if hall { self.player } else { family_roof };
        let sc = b.scale;
        let half = family.half() * sc;
        let mean = self.mean_under(b);
        // La grande maison ouvre sur la place
        let face = hall.then_some(Vec2::ZERO);
        if family.stilts() {
            let wood = srgb(0x6D4D30);
            // En marais, une galerie de planches fait le tour de la maison, au niveau des passerelles
            if let Some(deck) = self.deck {
                let base = deck.max(mean + 1.0);
                let g = half + Vec2::splat(GALLERY);
                let (a, d) = (g.x - 0.18, g.y - 0.18);
                for q in rect(b.at, Vec2::new(a, d), b.rot) {
                    self.post(q, 0.3, base, wood);
                }
                for (u, v) in [(a, 0.0), (-a, 0.0), (0.0, d), (0.0, -d)] {
                    let (s, c) = b.rot.sin_cos();
                    self.post(b.at + Vec2::new(u * c - v * s, u * s + v * c), 0.24, base, wood);
                }
                self.prism(&rect(b.at, g, b.rot), base - 0.28, base, srgb(0x886541));
                let body = half * Vec2::new(0.86, 0.78);
                let reed = wall.lerp(wood, 0.4);
                self.prism(&rect(b.at, body, b.rot), base, base + 1.7 * sc, reed);
                self.door(b, body, base, 1.3 * sc, face);
                self.gable(b.at, half, b.rot, base + 1.7 * sc, 2.6 * sc, roof, reed);
                return;
            }
            let base = mean + 2.6;
            for q in rect(b.at, half * 0.84, b.rot) {
                self.post(q, 0.32, base, wood);
            }
            self.prism(&rect(b.at, half * 0.94, b.rot), base - 0.25, base, wood);
            self.prism(&rect(b.at, half * Vec2::new(0.86, 0.82), b.rot), base, base + 2.0 * sc, wall);
            self.door(b, half * Vec2::new(0.86, 0.82), base, 1.5 * sc, face);
            self.gable(b.at, half, b.rot, base + 2.0 * sc, 2.7 * sc, roof, wall);
            return;
        }
        match family {
            Family::Steppe => {
                self.solid(&circle(b.at, 3.1 * sc, 12), mean + 2.0 * sc, wall);
                self.door(&Building { rot: -FRAC_PI_4, ..*b }, Vec2::splat(3.0 * sc), mean, 1.5 * sc, face);
                self.cone(b.at, half.x, mean + 2.0 * sc, 1.8 * sc, roof, 12, false);
                self.prism(&circle(b.at, 0.5 * sc, 6), mean + 3.4 * sc, mean + 3.7 * sc, srgb(0x57483A));
            }
            Family::Savanna => {
                self.solid(&circle(b.at, 2.7 * sc, 10), mean + 2.5 * sc, wall);
                self.door(&Building { rot: -FRAC_PI_4, ..*b }, Vec2::splat(2.6 * sc), mean, 1.8 * sc, face);
                self.cone(b.at, half.x, mean + 2.4 * sc, 3.8 * sc, roof, 10, false);
            }
            Family::Cold => {
                // Tente conique de peaux, ou maison basse à murs de pierre et toit de tourbe
                let tent = !hall && (((b.at.x * 2.3 + b.at.y) * 17.0).sin().abs() > 0.62);
                if tent {
                    self.cone(b.at, 3.0 * sc, mean, 6.6 * sc, srgb(0x8F8068), 8, true);
                    self.door(&Building { rot: -FRAC_PI_4, ..*b }, Vec2::splat(1.6 * sc), mean, 1.5 * sc, face);
                } else {
                    let (w, d, wh) = (half.x * 0.92, half.y * 0.62, 1.3 * sc);
                    self.solid(&rect(b.at, Vec2::new(w, d), b.rot), mean + wh, srgb(0x807D70));
                    self.door(b, Vec2::new(w, d), mean, wh, face);
                    self.gable(b.at, Vec2::new(w + 0.3, d + 0.45), b.rot, mean + wh, 1.9 * sc, roof, srgb(0x9C8A68));
                }
            }
            Family::Arid => {
                self.solid(&rect(b.at, half * 0.96, b.rot), mean + 3.5 * sc, wall);
                self.door(b, half * 0.96, mean, 1.9 * sc, face);
                self.prism(&rect(b.at, half, b.rot), mean + 3.5 * sc, mean + 3.9 * sc, roof);
            }
            _ => {
                let low = family == Family::Mediterranean;
                let wh = if low { 2.9 } else { 2.4 };
                let body = half * Vec2::new(0.88, 0.85);
                self.solid(&rect(b.at, body, b.rot), mean + wh * sc, wall);
                self.door(b, body, mean, 1.7 * sc, face);
                let rh = if family == Family::Boreal { 3.6 } else if low { 1.25 } else { 2.8 };
                self.gable(b.at, half, b.rot, mean + wh * sc, rh * sc, roof, wall);
            }
        }
    }

    /// Mât et fanion à la couleur du joueur.
    fn mast(&mut self, at: Vec2, height: f32) {
        let z = self.g(at);
        let top = z + height;
        self.post(at, 0.45, top, srgb(0x6D4D30));
        // (de face pour la caméra, qui regarde vers le nord)
        let tip = at + Vec2::new(4.4, 0.0);
        let flag = [p3(at, top), p3(at, top - 5.0), p3(tip, top - 2.5)];
        // (visible des deux côtés)
        let player = self.player;
        let side = Vec3::new(1.0, 0.0, 1.0);
        self.face(&flag, player, p3(at, top - 2.0) + side);
        self.face(&flag, player, p3(at, top - 2.0) - side);
    }

    fn wall(&mut self, limit: &[f32], gates: &[Gate], family: Family) {
        let mat = wall_color(family);
        let n = limit.len();
        let pts: Vec<Vec2> = (0..n).map(|i| Vec2::from_angle(i as f32 / n as f32 * TAU) * limit[i]).collect();
        for i in 0..n {
            let (p, q) = (pts[i], pts[(i + 1) % n]);
            let d = (q - p).normalize_or_zero();
            let side = d.perp() * 0.75;
            let (a, b) = (p - d * 0.2, q + d * 0.2);
            let (za, zb) = (self.g(p) + 4.0, self.g(q) + 4.0);
            let ring = [a + side, b + side, b - side, a - side];
            let low: Vec<Vec3> = ring.iter().map(|&c| p3(c, self.g(c) - 0.1)).collect();
            let up = [p3(ring[0], za), p3(ring[1], zb), p3(ring[2], zb), p3(ring[3], za)];
            self.body(&low, &up, mat);
        }
        // Tours : aux angles du tracé (là où il rejoint la rive) et régulières, jamais contre une porte
        let gate_at: Vec<usize> = gates.iter().map(|g| ((g.angle / TAU).rem_euclid(1.0) * n as f32).floor() as usize % n).collect();
        let close = |list: &[usize], i: usize, m: usize| list.iter().any(|&k| { let d = k.abs_diff(i); d.min(n - d) < m });
        let bend = |i: usize| (limit[(i + n - 2) % n] - limit[i]).abs() + (limit[(i + 2) % n] - limit[i]).abs();
        let mut kept: Vec<(usize, f32)> = Vec::new();
        let mut order: Vec<usize> = (0..n).filter(|&i| bend(i) > 2.6).collect();
        order.sort_by(|&a, &b| bend(b).total_cmp(&bend(a)));
        for i in order {
            let taken: Vec<usize> = kept.iter().map(|k| k.0).collect();
            if !close(&taken, i, 8) && !close(&gate_at, i, 6) {
                kept.push((i, 2.3));
            }
        }
        for i in (0..n).step_by(4) {
            let taken: Vec<usize> = kept.iter().map(|k| k.0).collect();
            if !close(&taken, i, 10) && !close(&gate_at, i, 6) {
                kept.push((i, 1.8));
            }
        }
        for (i, r) in kept {
            let p = pts[i];
            let z = self.g(p) + if r > 2.0 { 7.0 } else { 6.3 };
            self.solid(&circle(p, r, 8), z, mat);
            let player = self.player;
            self.cone(p, r + 0.35, z, 2.5, player, 8, false);
        }
        for g in gates {
            self.gate(g, mat, family);
        }
    }

    /// Porte de ville : un bâtiment à cheval sur le mur, percé d'un passage (voûte et herse pour la
    /// pierre et la terre, linteau et vantaux pour le bois). Dessiné sur ses deux faces.
    fn gate(&mut self, g: &Gate, mat: Rgb, family: Family) {
        let u = g.out.perp();
        let rot = u.to_angle();
        // (en marais, le passage est au niveau des passerelles)
        let z = self.deck.map_or(self.g(g.at), |d| d - 0.05);
        let wood = family.wooden_wall();
        let (hw, top) = (2.7, z + if wood { 6.0 } else { 7.4 });
        let player = self.player;
        if g.main {
            for k in [-1.0, 1.0] {
                let p = g.at + u * k * (hw + 1.2);
                let zt = self.g(p) + 8.2;
                self.solid(&circle(p, 1.95, 10), zt, mat);
                self.cone(p, 2.3, zt, 3.0, player, 10, false);
            }
        }
        let half = Vec2::new(hw, GATE_DEPTH);
        self.solid(&rect(g.at, half, rot), top, mat);
        if family == Family::Arid {
            self.prism(&rect(g.at, half + Vec2::splat(0.2), rot), top, top + 0.55, mat.lerp(Vec3::ONE, 0.18));
        } else {
            let over = rect(g.at, half + Vec2::splat(0.3), rot);
            let apex = p3(g.at, top + 2.5);
            let inside = p3(g.at, top + 0.5);
            for k in 0..4 {
                self.face(&[p3(over[k], top), p3(over[(k + 1) % 4], top), apex], player, inside);
            }
        }
        // Les deux faces du passage (la caméra peut tourner autour)
        for side in [1.0f32, -1.0] {
            let n = g.out * side;
            let at = move |a: f32, v: f32| p3(g.at + n * (GATE_DEPTH + 0.04) + u * a, z + v);
            let behind = p3(g.at, z + 2.0);
            if wood {
                // (le bas descend sous le sol : sur une pente, la porte ne flotte pas côté aval)
                self.face(&[at(-1.95, -1.2), at(1.95, -1.2), at(1.95, 3.75), at(-1.95, 3.75)], mat * 0.78, behind);
                for (a, b) in [(-1.5, -0.04), (0.04, 1.5)] {
                    self.face(&[at(a, -1.2), at(b, -1.2), at(b, 3.1), at(a, 3.1)], srgb(0x7A5835), behind);
                }
                for v in [0.75, 2.2] {
                    self.face(&[at(-1.5, v), at(1.5, v), at(1.5, v + 0.17), at(-1.5, v + 0.17)], srgb(0x3E2C1B), behind);
                }
                self.face(&[at(-2.2, 3.2), at(2.2, 3.2), at(2.2, 3.72), at(-2.2, 3.72)], srgb(0x55402B), behind);
            } else {
                let arch = |r: f32, base: f32| {
                    // (le bas descend sous le sol : sur une pente, le passage ne flotte pas côté aval)
                    let mut o = vec![at(-r, -1.2)];
                    for k in 0..=10 {
                        let b = PI - k as f32 / 10.0 * PI;
                        o.push(at(r * b.cos(), base + r * b.sin()));
                    }
                    o.push(at(r, -1.2));
                    o
                };
                // (l'arc est convexe : éventail depuis son premier sommet)
                self.face(&arch(1.62, 2.25), mat.lerp(Vec3::ONE, 0.3), behind);
                self.face(&arch(1.28, 2.2), srgb(0x1B130C), behind);
                for a in [-0.7f32, -0.23, 0.23, 0.7] {
                    let high = 2.2 + (1.28f32 * 1.28 - a * a).sqrt();
                    self.face(&[at(a - 0.05, 2.05), at(a + 0.05, 2.05), at(a + 0.05, high), at(a - 0.05, high)], srgb(0x6A5A45), behind);
                }
                for v in [2.35f32, 2.85] {
                    let w = (1.28f32 * 1.28 - (v - 2.2).max(0.0).powi(2)).max(0.0).sqrt();
                    self.face(&[at(-w, v), at(w, v), at(w, v + 0.09), at(-w, v + 0.09)], srgb(0x6A5A45), behind);
                }
            }
        }
    }

    /// Ponton : le plancher épouse le sol, part du seuil et reste au-dessus de l'eau.
    fn pier(&mut self, p: &Pier, water: f32) {
        let len = p.from.distance(p.to);
        if len < 0.1 {
            return;
        }
        let t = (p.to - p.from) / len;
        let s = t.perp();
        let z_at = |b: &Self, d: f32| {
            let q = p.from + t * d;
            (b.g(q) + 0.12).max(water + 0.75).max(p.start_height.map_or(f32::MIN, |h| h - d * 0.55))
        };
        let n = ((len / 0.95).round() as usize).max(4);
        for i in 0..n {
            let (d0, d1) = (i as f32 / n as f32 * len, (i + 1) as f32 / n as f32 * len);
            let (z0, z1) = (z_at(self, d0), z_at(self, d1));
            let (a, b) = (p.from + t * d0, p.from + t * d1);
            let w = s * p.half_width;
            let ring = [a - w, b - w, b + w, a + w];
            let zs = [z0, z1, z1, z0];
            let up: Vec<Vec3> = ring.iter().zip(zs).map(|(&c, z)| p3(c, z)).collect();
            let low: Vec<Vec3> = ring.iter().zip(zs).map(|(&c, z)| p3(c, z - 0.24)).collect();
            self.body(&low, &up, if i % 2 == 1 { srgb(0x7D5C3B) } else { srgb(0x8E6C48) });
            let mid = (a + b) / 2.0;
            if i % 2 == 1 && (z0 + z1) / 2.0 - self.g(mid) > 0.45 {
                for k in [-1.0, 1.0] {
                    self.post(mid + s * k * (p.half_width - 0.08), 0.2, (z0 + z1) / 2.0 + 0.42, srgb(0x6D4D30));
                }
            }
        }
        if let Some((at, rot)) = p.boat {
            self.boat(at, rot, water);
        }
    }

    /// Pont : tablier bombé sur piles, en pierre sous une muraille de pierre, en bois sinon.
    fn bridge(&mut self, b: &Bridge, family: Family) {
        let stone = !family.wooden_wall();
        let (deck_a, deck_b) = if stone { (srgb(0x9A9488), srgb(0x8E8880)) } else { (srgb(0x7D5C3B), srgb(0x8E6C48)) };
        let pile = if stone { srgb(0x847F76) } else { srgb(0x6D4D30) };
        let len = b.near.distance(b.far);
        if len < 0.5 {
            return;
        }
        let t = (b.far - b.near) / len;
        let s = t.perp();
        let (z_near, z_far) = (self.g(b.near) + 0.15, self.g(b.far) + 0.15);
        let z_at = |d: f32| { let f = d / len; z_near + (z_far - z_near) * f + 1.3 * 4.0 * f * (1.0 - f) };
        let n = ((len / 0.95).round() as usize).max(4);
        for i in 0..n {
            let (d0, d1) = (i as f32 / n as f32 * len, (i + 1) as f32 / n as f32 * len);
            let (p, q) = (b.near + t * (d0 - 0.02), b.near + t * (d1 + 0.02));
            let w = s * b.half_width;
            let ring = [p - w, q - w, q + w, p + w];
            let zs = [z_at(d0), z_at(d1), z_at(d1), z_at(d0)];
            let up: Vec<Vec3> = ring.iter().zip(zs).map(|(&c, z)| p3(c, z)).collect();
            let low: Vec<Vec3> = ring.iter().zip(zs).map(|(&c, z)| p3(c, z - 0.45)).collect();
            self.body(&low, &up, if i % 2 == 1 { deck_a } else { deck_b });
            // Garde-corps bas, le long du tablier
            for k in [-1.0f32, 1.0] {
                let side = s * k * (b.half_width - 0.08);
                let rail: Vec<Vec3> = [p + side - s * 0.08, q + side - s * 0.08, q + side + s * 0.08, p + side + s * 0.08].iter().zip(zs).map(|(&c, z)| p3(c, z + 0.45)).collect();
                let foot: Vec<Vec3> = [p + side - s * 0.08, q + side - s * 0.08, q + side + s * 0.08, p + side + s * 0.08].iter().zip(zs).map(|(&c, z)| p3(c, z)).collect();
                self.body(&foot, &rail, pile);
            }
        }
        // Piles dans le lit
        let piles = ((len / 4.0) as usize).max(1);
        for i in 1..=piles {
            let d = i as f32 / (piles + 1) as f32 * len;
            let at = b.near + t * d;
            let top = z_at(d) - 0.4;
            if stone {
                self.solid(&rect(at, Vec2::new(0.55, b.half_width * 0.8), t.to_angle()), top, pile);
            } else {
                for k in [-0.8f32, 0.8] {
                    self.post(at + s * k, 0.22, top, pile);
                }
            }
        }
        // Une tour garde l'autre rive
        if b.head {
            let mat = wall_color(family);
            let at = b.far + t * 1.7;
            let z = self.g(at) + 6.5;
            self.solid(&circle(at, 1.9, 8), z, mat);
            let player = self.player;
            self.cone(at, 2.25, z, 2.5, player, 8, false);
        }
    }

    /// Barque amarrée, à la place choisie par la pose (là où l'eau est assez large pour elle).
    fn boat(&mut self, at: Vec2, rot: f32, water: f32) {
        let t = Vec2::from_angle(rot);
        let (l, w) = (3.2f32, 1.05f32);
        let (sn, co) = rot.sin_cos();
        let shape = |k: f32| [(-l, 0.0), (-l * 0.55, w), (l * 0.55, w), (l, 0.0), (l * 0.55, -w), (-l * 0.55, -w)].map(|(a, b): (f32, f32)| at + Vec2::new(k * a * co - k * b * sn, k * a * sn + k * b * co));
        let low: Vec<Vec3> = shape(0.74).iter().map(|&q| p3(q, water)).collect();
        let up: Vec<Vec3> = shape(1.0).iter().map(|&q| p3(q, water + 0.75)).collect();
        self.body(&low, &up, srgb(0x70492A));
        let inner: Vec<Vec3> = shape(0.76).iter().map(|&q| p3(q, water + 0.76)).collect();
        self.face(&inner, srgb(0x33241A), p3(at, water));
        for a in [-0.9f32, 0.9] {
            let bench = rect(at + t * a, Vec2::new(0.24, w * 0.72), rot);
            let b: Vec<Vec3> = bench.iter().map(|&q| p3(q, water + 0.8)).collect();
            self.face(&b, srgb(0xA98A5E), p3(at, water));
        }
    }

    /// Passerelle : plancher continu sur poteaux, avec une rampe là où elle descend à la terre.
    fn walk(&mut self, from: Vec2, z_from: f32, to: Vec2, z_to: f32) {
        let len = from.distance(to);
        if len < 0.3 {
            return;
        }
        let t = (to - from) / len;
        let s = t.perp();
        let n = ((len / 0.9).round() as usize).max(1);
        let ramp = 3.6f32.min(len * 0.75);
        let z_at = |d: f32| if (z_from - z_to).abs() < 0.15 || len - d >= ramp { z_from } else { z_to + (z_from - z_to) * (len - d) / ramp };
        for i in 0..n {
            let (d0, d1) = (i as f32 / n as f32 * len, (i + 1) as f32 / n as f32 * len);
            let (a, b) = (from + t * (d0 - 0.02), from + t * (d1 + 0.02));
            let w = s * 0.7;
            let ring = [a - w, b - w, b + w, a + w];
            let (z0, z1) = (z_at(d0), z_at(d1));
            let zs = [z0, z1, z1, z0];
            let up: Vec<Vec3> = ring.iter().zip(zs).map(|(&c, z)| p3(c, z)).collect();
            let low: Vec<Vec3> = ring.iter().zip(zs).map(|(&c, z)| p3(c, z - 0.28)).collect();
            self.body(&low, &up, if i % 2 == 1 { srgb(0x7F5D3C) } else { srgb(0x886541) });
            let mid = (a + b) / 2.0;
            if i % 3 == 1 && (z0 + z1) / 2.0 - self.g(mid) > 0.5 {
                for k in [-0.62, 0.62] {
                    self.post(mid + s * k, 0.16, (z0 + z1) / 2.0 - 0.2, srgb(0x6D4D30));
                }
            }
        }
    }

    /// Escalier : des marches larges et claires, à plat, qui se lisent même de loin.
    fn stairs(&mut self, from: Vec2, z_from: f32, to: Vec2, z_to: f32) {
        let len = from.distance(to);
        if len < 0.3 {
            return;
        }
        let t = (to - from) / len;
        let n = 5;
        for i in 0..n {
            let f = (i as f32 + 0.5) / n as f32;
            let c = from + t * (len * f);
            let z = z_from + (z_to - z_from) * (i as f32 + 1.0) / n as f32;
            self.solid(&rect(c, Vec2::new(len / n as f32 / 2.0 + 0.02, 1.15), t.to_angle()), z, if i % 2 == 0 { srgb(0xA98A5E) } else { srgb(0x9A7C52) });
        }
    }

    /// Palier de la place, au niveau des planchers.
    fn landing(&mut self, z: f32) {
        let wood = srgb(0x6D4D30);
        let h = LANDING - 0.2;
        for q in rect(Vec2::ZERO, Vec2::splat(h), 0.0) {
            self.post(q, 0.26, z, wood);
        }
        for q in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            self.post(q * h, 0.22, z, wood);
        }
        self.prism(&rect(Vec2::ZERO, Vec2::splat(LANDING), 0.0), z - 0.28, z, srgb(0x886541));
    }

    /// Roseaux au bord des mares.
    fn reeds(&mut self, ponds: &[Pond]) {
        for (i, p) in ponds.iter().enumerate() {
            for k in 0..7 {
                let a = k as f32 * 0.9 + i as f32 * 1.7;
                let at = p.at + Vec2::from_angle(a) * (p.radius_at(a) + 0.2);
                let z = self.g(at);
                for (j, (u, v)) in [(0.5f32, 0.3f32), (-0.4, 0.2), (0.1, -0.5)].into_iter().enumerate() {
                    let foot = at + Vec2::new(u, v) * 0.7;
                    let blade = [p3(foot, z), p3(foot + Vec2::new(0.16, -0.16), z), p3(at + Vec2::new(u, v), z + 1.7 + 0.4 * j as f32)];
                    let side = Vec3::new(1.0, 0.0, 1.0);
                    self.face(&blade, srgb(0x8D9450), p3(at, z + 1.0) + side);
                    self.face(&blade, srgb(0x8D9450), p3(at, z + 1.0) - side);
                }
            }
        }
    }

    fn hearth(&mut self) {
        // (en marais, le foyer est sur le palier de la place)
        if let Some(z) = self.deck {
            for i in 0..6 {
                let at = Vec2::from_angle(i as f32 / 6.0 * TAU) * 1.2;
                self.prism(&rect(at, Vec2::splat(0.34), 0.0), z, z + 0.42, srgb(0x8D8F8C));
            }
            self.cone(Vec2::ZERO, 0.7, z, 1.6, srgb(0xF2A33A), 6, false);
            return;
        }
        let z = self.g(Vec2::ZERO);
        for i in 0..6 {
            let at = Vec2::from_angle(i as f32 / 6.0 * TAU) * 1.7;
            let top = self.g(at) + 0.6;
            self.solid(&rect(at, Vec2::splat(0.5), 0.0), top, srgb(0x8D8F8C));
        }
        self.cone(Vec2::ZERO, 1.0, z, 2.1, srgb(0xF2A33A), 6, false);
    }
}

fn circle(c: Vec2, r: f32, n: usize) -> Vec<Vec2> {
    (0..n).map(|i| c + Vec2::from_angle(i as f32 / n as f32 * TAU) * r).collect()
}

/// Maillage opaque d'un site : bâtiments, mât, muraille, portes, ponton.
/// `ground` donne la hauteur du sol au point 2D de la case, par rapport au centre de la case ;
/// `water` est le niveau de l'eau voisine, par rapport au même centre.
pub fn bake(layout: &Layout, ground: &dyn Fn(Vec2) -> f32, water: f32, player: Rgb) -> MeshData {
    let mut b = Baker { data: MeshData::default(), ground, player, deck: layout.deck };
    if let Some(z) = layout.deck {
        b.landing(z);
        for w in &layout.walks {
            let end = if w.to_ground { b.g(w.to) + 0.15 } else { z };
            b.walk(w.from, z, w.to, end);
        }
        if let Some((from, to)) = layout.stairs {
            let end = b.g(to) + 0.1;
            b.stairs(from, z, to, end);
        }
        b.reeds(&layout.ponds);
    }
    for building in &layout.buildings {
        b.building(building, layout.family);
    }
    if let Some(at) = layout.mast {
        let height = [16.0, 19.0, 22.0][((layout.ground_radius - 12.0) / 6.0).clamp(0.0, 2.0) as usize];
        b.mast(at, height);
    }
    if layout.hearth {
        b.hearth();
    }
    if let Some(p) = &layout.pier {
        b.pier(p, water);
    }
    if let Some(bridge) = &layout.bridge {
        b.bridge(bridge, layout.family);
    }
    if let Some(limit) = &layout.wall {
        debug_assert_eq!(limit.len(), LIMIT_SAMPLES);
        b.wall(limit, &layout.gates, layout.family);
    }
    b.data
}

/// Maillage d'un pont construit hors d'une ville (en bois pour l'instant ; les matériaux viendront
/// avec les ponts à construire). `ground` : hauteur du sol, relative à l'origine du maillage.
pub fn bake_bridge(bridge: &Bridge, ground: &dyn Fn(Vec2) -> f32) -> MeshData {
    let mut b = Baker { data: MeshData::default(), ground, player: srgb(0x3373F2), deck: None };
    b.bridge(bridge, Family::Prairie);
    b.data
}

/// Couverture des tentes : la matière du lieu. Seule la tente du chef porte le rouge hostile.
fn tent_color(f: Family) -> Rgb {
    srgb(match f {
        Family::Forest => 0x857258,
        Family::Prairie => 0xA89A72,
        Family::Boreal => 0x6E5A46,
        Family::Arid => 0xB89A6B,
        Family::Cold => 0x8A7D68,
        Family::Tropical => 0x7F6E43,
        Family::Mediterranean => 0x8C7E62,
        Family::Savanna => 0xA18857,
        Family::Steppe => 0xD4CAB1,
        Family::Marsh => 0xA29A68,
    })
}

/// Rouge hostile : tente du chef, étendard, toit de la tour de guet.
const HOSTILE_RED: u32 = 0xB5281F;

/// Maillage d'un camp hostile : pointes partout, un rouge réduit à l'essentiel.
pub fn bake_camp(layout: &CampLayout, ground: &dyn Fn(Vec2) -> f32) -> MeshData {
    let red = srgb(HOSTILE_RED);
    let wood = srgb(0x6D4D30);
    let mut b = Baker { data: MeshData::default(), ground, player: red, deck: None };
    // Rochers d'un repaire, et leur entrée sombre
    for rock in &layout.rocks {
        let z = b.g(rock.at);
        let ring: Vec<Vec3> = circle(rock.at, rock.radius, 6).iter().enumerate().map(|(i, &p)| {
            // (contour un peu irrégulier, fixé par la position)
            let wobble = ((rock.at.x * 3.0 + i as f32 * 1.7).sin()) * 0.8;
            let q = rock.at + (p - rock.at).normalize_or_zero() * (rock.radius + wobble);
            p3(q, b.g(q) - 0.5)
        }).collect();
        let apex = p3(rock.at + Vec2::splat(0.7), z + rock.height);
        let inside = p3(rock.at, z + rock.height * 0.3);
        for i in 0..6 {
            b.face(&[ring[i], ring[(i + 1) % 6], apex], srgb(0x68635D), inside);
        }
    }
    if let Some(at) = layout.cave {
        let back = at.normalize_or_zero();
        let side = back.perp();
        let foot = at + back * 1.2;
        let z = b.g(foot);
        // Tournée vers le centre du camp
        b.face(&[p3(foot - side * 2.4, z), p3(foot + side * 2.4, z), p3(foot, z + 5.2)], srgb(0x17120F), p3(foot + back * 3.0, z + 1.0));
    }
    for t in &layout.tents {
        let z = b.g(t.at);
        let scale = if t.chief { 1.2 } else { 1.0 };
        let color = if t.chief { red } else { tent_color(layout.family) };
        b.cone(t.at, 3.3 * scale, z, 6.4 * scale, color, 5, true);
    }
    for s in &layout.stakes {
        let foot: Vec<Vec3> = circle(s.at, s.radius, 4).iter().map(|&p| p3(p, b.g(p) - 0.1)).collect();
        let tip = p3(s.tip, b.g(s.at) + s.height);
        let inside = p3(s.at, b.g(s.at) + s.height * 0.3);
        for i in 0..4 {
            b.face(&[foot[i], foot[(i + 1) % 4], tip], wood, inside);
        }
    }
    if let Some(at) = layout.tower {
        let z = b.g(at);
        for q in rect(at, Vec2::splat(2.0), 0.0) {
            b.post(q, 0.42, z + 13.0, wood);
        }
        b.prism(&rect(at, Vec2::splat(3.0), 0.0), z + 12.7, z + 13.2, srgb(0x5D4128));
        for q in rect(at, Vec2::splat(2.8), 0.0) {
            b.prism(&circle(q, 0.3, 5), z + 13.0, z + 15.4, wood);
        }
        b.gable(at, Vec2::splat(3.2), FRAC_PI_4, z + 15.2, 2.8, red, wood);
    }
    if let Some(at) = layout.banner {
        b.mast(at, if layout.kind == CampKind::Fort { 19.0 } else { 24.0 });
    }
    if let Some(at) = layout.trophy {
        let z = b.g(at);
        b.post(at, 0.3, z + 6.0, wood);
        b.prism(&rect(at, Vec2::splat(0.7), 0.3), z + 5.2, z + 6.4, srgb(0xE6DFCC));
    }
    b.hearth();
    b.data
}

/// Mares d'un marais : l'eau et sa bordure de vase, posées sur le sol. Pour une matière transparente.
pub fn bake_ponds(ponds: &[Pond], ground: &dyn Fn(Vec2) -> f32, mud: Rgb) -> MeshData {
    let mut data = MeshData::default();
    let water = srgb(0x4F7F86);
    for p in ponds {
        for (extra, lift, color, alpha) in [(1.4f32, 0.38f32, mud, 0.5f32), (0.0, 0.46, water, 0.92)] {
            let base = data.positions.len() as u32;
            data.positions.push(p3(p.at, ground(p.at) + lift).to_array());
            data.normals.push([0.0, 1.0, 0.0]);
            data.colors.push([color.x, color.y, color.z, alpha]);
            const N: u32 = 36;
            for k in 0..N {
                let a = k as f32 / N as f32 * TAU;
                let q = p.at + Vec2::from_angle(a) * (p.radius_at(a) + extra);
                data.positions.push(p3(q, ground(q) + lift).to_array());
                data.normals.push([0.0, 1.0, 0.0]);
                data.colors.push([color.x, color.y, color.z, if extra > 0.0 { 0.0 } else { alpha }]);
            }
            for k in 0..N {
                data.indices.extend([base, base + 1 + k, base + 1 + (k + 1) % N]);
            }
        }
    }
    data
}

/// Terre piétinée d'un camp.
pub fn camp_earth() -> Rgb {
    srgb(0x4A3C2F)
}

/// Chemins : un ruban de terre usée posé sur le sol, sans bordure, dont la largeur et l'opacité
/// varient ; il se perd en sortant de la ville. Pour une matière transparente.
pub fn bake_paths(paths: &[crate::settlements::Path], ground: &dyn Fn(Vec2) -> f32, earth: Rgb) -> MeshData {
    let mut data = MeshData::default();
    for (pi, path) in paths.iter().enumerate() {
        for w in path.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.distance(b);
            if len < 0.2 {
                continue;
            }
            let dir = (b - a) / len;
            let side = dir.perp();
            let n = ((len / 0.8).round() as usize).max(1);
            let wobble = |f: f32, k: f32| ((f * 3.1 + pi as f32) * k).sin();
            let base = data.positions.len() as u32;
            for i in 0..=n {
                let f = i as f32 / n as f32;
                let center = a + dir * (len * f) + side * (wobble(f * len * 0.16, 1.0) * 0.6);
                let half = 0.55 + 0.25 * wobble(f * len * 0.13, 2.1);
                let fade = if path.fade { 1.0 - f } else { 1.0 };
                let alpha = (0.34 + 0.2 * wobble(f * len * 0.3, 3.3)) * path.weight * fade;
                for k in [-1.0f32, 1.0] {
                    let q = center + side * k * half;
                    data.positions.push(p3(q, ground(q) + 0.42).to_array());
                    data.normals.push([0.0, 1.0, 0.0]);
                    data.colors.push([earth.x, earth.y, earth.z, alpha.clamp(0.0, 1.0)]);
                }
            }
            for i in 0..n as u32 {
                let (p, q, r, s) = (base + i * 2, base + i * 2 + 1, base + i * 2 + 2, base + i * 2 + 3);
                // (les deux sens : le ruban se voit quel que soit l'ordre de ses points)
                data.indices.extend([p, r, q, q, r, s, p, q, r, q, s, r]);
            }
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlements::{GateKind, Site, city_layout, village_layout};

    struct Flat;
    impl Site for Flat {
        fn height(&self, _: Vec2) -> f32 {
            0.0
        }
        fn water_gap(&self, p: Vec2) -> f32 {
            20.0 - p.y
        }
    }

    #[test]
    fn le_maillage_est_coherent() {
        for family in Family::ALL {
            for stage in 0..3 {
                for fort in [false, true] {
                    let l = city_layout(&Flat, family, stage, fort, 5);
                    let d = bake(&l, &|_| 0.0, -0.8, Vec3::Z);
                    assert!(!d.indices.is_empty());
                    assert!(d.indices.iter().all(|&i| (i as usize) < d.positions.len()));
                    assert_eq!(d.positions.len(), d.normals.len());
                    assert_eq!(d.positions.len(), d.colors.len());
                    assert!(d.positions.iter().flatten().all(|v| v.is_finite()));
                }
            }
            let v = village_layout(&Flat, family, 5);
            assert!(!bake(&v, &|_| 0.0, -0.8, Vec3::Z).indices.is_empty());
        }
    }

    #[test]
    fn le_maillage_d_un_camp_est_coherent() {
        for family in Family::ALL {
            for kind in [CampKind::Encampment, CampKind::Fort, CampKind::Lair] {
                let l = crate::settlements::camp_layout(&Flat, family, kind, 5);
                let d = bake_camp(&l, &|_| 0.0);
                assert!(!d.indices.is_empty());
                assert!(d.indices.iter().all(|&i| (i as usize) < d.positions.len()));
                assert!(d.positions.iter().flatten().all(|v| v.is_finite()));
            }
        }
    }

    #[test]
    fn le_bas_des_murs_suit_le_sol() {
        struct Dry;
        impl Site for Dry {
            fn height(&self, p: Vec2) -> f32 {
                0.2 * (p.x - p.y)
            }
            fn water_gap(&self, _: Vec2) -> f32 {
                1e6
            }
        }
        let ground = |p: Vec2| Dry.height(p);
        // (sans muraille : le bas des portes descend exprès sous le sol)
        let l = city_layout(&Dry, Family::Prairie, 2, false, 5);
        let d = bake(&l, &ground, -5.0, Vec3::Z);
        // Aucun sommet de bâtiment ne descend franchement sous le sol local
        let low = d.positions.iter().filter(|p| p[1] < ground(Vec2::new(p[0], -p[2])) - 0.3).count();
        assert_eq!(low, 0, "{low} sommets sous le sol");
    }

    #[test]
    fn les_chemins_sont_discrets() {
        let l = city_layout(&Flat, Family::Prairie, 2, true, 5);
        let d = bake_paths(&l.paths, &|_| 0.0, Vec3::ONE);
        assert!(d.colors.iter().all(|c| c[3] <= 0.6), "pas de chemin opaque");
        assert!(l.gates.iter().any(|g| g.kind == GateKind::Pier));
    }
}
