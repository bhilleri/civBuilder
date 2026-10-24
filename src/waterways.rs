//! Formes des cours d'eau à dessiner (géométrie 2D pure, partagée par les vues 2D et 3D).
//!
//! Pour chaque case : les rivières de ses arêtes (une arête est dessinée par une seule des deux
//! cases, celle pour qui elle est dans les directions 0 à 2), le fleuve qui l'occupe (une large
//! bande d'eau du centre vers chaque arête où il continue, berges de part et d'autre) et son
//! petit lac. Les triangles sont découpés finement pour épouser le relief en 3D.

use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::hydro::RIVER_M3S;
use crate::map::{Terrain, WorldMap};
use bevy::math::Vec2;

/// Demi-largeur de la bande d'eau d'un fleuve (unités monde ; l'arête mesure `HEX_SIZE`).
pub const FLEUVE_HALF_WIDTH: f32 = HEX_SIZE * 0.46;
/// Demi-largeur d'un fleuve à son embouchure : toute l'arête.
const ESTUARY_HALF_WIDTH: f32 = HEX_SIZE * 0.5;
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
    /// Demi-largeur au bout `b` (un fleuve s'évase en arrivant dans un lac ou la mer).
    pub hw_end: f32,
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

    fn tri(&mut self, pts: [Vec2; 3], kind: WaterKind) {
        let channel = self.channels.len() - 1;
        self.tris.push(WaterTri { pts, kind, also: self.also, channel });
    }

    /// Enregistre le lit d'un tronçon.
    fn channel(&mut self, a: Vec2, b: Vec2, hw: f32, hw_end: f32, kind: WaterKind) {
        self.channels.push(WaterAxis { a, b, hw, hw_end, kind, also: self.also, inflow: self.inflow });
    }

    /// Bande droite de `a` à `b` (demi-largeur `hw`), découpée en `STEPS` tronçons.
    fn band(&mut self, a: Vec2, b: Vec2, hw: f32, kind: WaterKind) {
        self.flared_band(a, b, hw, hw, kind);
    }

    /// Bande de `a` à `b` dont la demi-largeur passe de `hw` à `hw_end`.
    fn flared_band(&mut self, a: Vec2, b: Vec2, hw: f32, hw_end: f32, kind: WaterKind) {
        self.channel(a, b, hw, hw_end, kind);
        let n = (b - a).perp().normalize_or_zero();
        // Les bandes larges sont aussi découpées en travers (elles épousent mieux le relief)
        let across = if hw.max(hw_end) > 6.0 { 4 } else { 1 };
        let at = |k: usize, j: usize| {
            let t = k as f32 / STEPS as f32;
            a.lerp(b, t) + n * (hw + (hw_end - hw) * t) * (2.0 * j as f32 / across as f32 - 1.0)
        };
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
        self.channel(c - half, c + half, r.y, r.y, kind);
        let at = |k: usize| {
            let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
            c + Vec2::new(a.cos() * r.x, a.sin() * r.y)
        };
        for k in 0..SIDES {
            self.tri([c, at(k), at(k + 1)], kind);
        }
    }
}

/// Arêtes d'une case de fleuve où l'eau est dessinée (`open`), et celles qu'elle occupe sur
/// toute leur largeur (`full`, sous-ensemble de `open`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Openings {
    pub open: u8,
    pub full: u8,
}

/// Arêtes par où l'eau d'un fleuve sort de sa case : celles de son cours, toutes celles qui
/// donnent sur un lac (sur toute leur largeur) ; à son embouchure, tout le large, et aussi :
/// - les arêtes voisines qui donnent aussi sur l'eau (lac ou mer) : un fleuve qui arrive dans
///   un coin de lac s'y jette sur toute sa largeur plutôt que de longer une mince langue de terre ;
/// - l'intérieur d'un virage serré juste avant l'embouchure (arête de terre entre l'embouchure
///   et le tronçon amont) : l'eau l'occupe, au lieu d'une pointe de terre entre le fleuve et le lac.
pub fn fleuve_openings(map: &WorldMap, hex: Hex) -> Openings {
    let links = map.fleuve_links(hex);
    let water_side = |d: usize| map.get(hex + DIRECTIONS[d]).is_some_and(|t| t.is_water());
    let mut open = links;
    let mut full = 0u8;
    // Un fleuve qui longe un lac s'y ouvre sur chaque arête commune (le lac et le fleuve
    // forment une seule nappe d'eau) ; pas la mer : un fleuve côtier garde sa rive
    for (d, &step) in DIRECTIONS.iter().enumerate() {
        if map.get(hex + step) == Some(Terrain::Lake) {
            open |= 1 << d;
            full |= 1 << d;
        }
    }
    for d in 0..6 {
        if links & (1 << d) == 0 || !water_side(d) {
            continue;
        }
        full |= 1 << d;
        for (e, beyond) in [((d + 1) % 6, (d + 2) % 6), ((d + 5) % 6, (d + 4) % 6)] {
            if water_side(e) || (links & (1 << beyond) != 0 && !map.is_fleuve(hex + DIRECTIONS[e])) {
                open |= 1 << e;
                full |= 1 << e;
            }
        }
    }
    Openings { open, full }
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
        let Openings { open, full } = fleuve_openings(map, hex);
        shape.disc(c, Vec2::splat(FLEUVE_HALF_WIDTH), WaterKind::Fleuve);
        for d in 0..6 {
            if open & (1 << d) != 0 {
                let ((ax, ay), (bx, by)) = hex.edge_corners(d);
                let mid = (Vec2::new(ax, ay) + Vec2::new(bx, by)) / 2.0;
                // Léger débord sur la case voisine : pas de liseré entre deux tronçons
                let mid = mid + (mid - c).normalize_or_zero() * 0.6;
                // Embouchure (mer, lac) : le fleuve s'évase jusqu'à occuper toute l'arête
                let end = if full & (1 << d) != 0 { ESTUARY_HALF_WIDTH } else { FLEUVE_HALF_WIDTH };
                shape.flared_band(c, mid, FLEUVE_HALF_WIDTH, end, WaterKind::Fleuve);
            }
        }
        // Deux arêtes ouvertes côte à côte dont l'une sur toute sa largeur : l'eau remplit aussi
        // le coin qui les sépare (pas de pointe de terre entre elles)
        for d in 0..6 {
            let e = (d + 1) % 6;
            let both = open & (1 << d) != 0 && open & (1 << e) != 0;
            if both && (full & (1 << d) != 0 || full & (1 << e) != 0) {
                let k = corner(hex, e);
                let k = k + (k - c).normalize_or_zero() * 0.6;
                shape.flared_band(c, k, FLEUVE_HALF_WIDTH, HEX_SIZE * 0.25, WaterKind::Fleuve);
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

/// Morceaux d'un triangle d'eau situés dans des cases explorées : le triangle est coupé par
/// l'arête qui sépare la case explorée qui le porte de chaque case inexplorée qu'il touche.
pub fn clip_to_explored(pts: [Vec2; 3], explored: &impl Fn(Hex) -> bool) -> Vec<[Vec2; 3]> {
    let hex_of = |p: Vec2| Hex::from_world(p.x, p.y);
    let centroid = (pts[0] + pts[1] + pts[2]) / 3.0;
    let touched: Vec<Hex> = std::iter::once(centroid).chain(pts).map(hex_of).collect();
    let unknown: Vec<Hex> = touched.iter().copied().filter(|&h| !explored(h)).collect();
    if unknown.is_empty() {
        return vec![pts];
    }
    let Some(keep) = touched.iter().copied().find(|&h| explored(h)) else { return Vec::new() };
    let center = |h: Hex| {
        let (x, y) = h.to_world();
        Vec2::new(x, y)
    };
    let mut poly: Vec<Vec2> = pts.to_vec();
    for u in unknown {
        // Demi-plan plus proche de la case gardée que de la case inconnue (leur arête commune)
        let (k, o) = (center(keep), center(u));
        let normal = o - k;
        let offset = normal.dot((k + o) / 2.0);
        let side = |p: Vec2| offset - normal.dot(p); // ≥ 0 : côté gardé
        let mut out = Vec::with_capacity(poly.len() + 2);
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                out.push(p + (q - p) * (sp / (sp - sq)));
            }
        }
        poly = out;
        if poly.len() < 3 {
            return Vec::new();
        }
    }
    (1..poly.len() - 1).map(|i| [poly[0], poly[i], poly[i + 1]]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eau_coupee_au_bord_de_l_exploration() {
        let a = Hex::from_offset(3, 3);
        let b = a + DIRECTIONS[0];
        let ((x0, y0), (x1, y1)) = a.edge_corners(0);
        let mid = Vec2::new((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (ca, cb) = (center(a), center(b));
        // Triangle à cheval sur l'arête A–B
        let tri = [mid + (ca - mid) * 0.3, mid + (cb - mid) * 0.3 + Vec2::Y * 4.0, mid + (cb - mid) * 0.3 - Vec2::Y * 4.0];
        let area = |t: &[Vec2; 3]| (t[1] - t[0]).perp_dot(t[2] - t[0]).abs() / 2.0;
        let whole = area(&tri);
        // Tout est exploré : intact ; B inconnue : seule la partie côté A reste ; rien d'exploré : rien
        assert_eq!(clip_to_explored(tri, &|_| true), vec![tri]);
        let kept = clip_to_explored(tri, &|h| h == a);
        let kept_area: f32 = kept.iter().map(area).sum();
        assert!(kept_area > 0.0 && kept_area < whole);
        for t in &kept {
            for p in t {
                assert!(p.distance(ca) <= p.distance(cb) + 1e-3);
            }
        }
        assert!(clip_to_explored(tri, &|_| false).is_empty());
    }

    #[test]
    fn une_arete_dessinee_une_seule_fois() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set_river(h, 4, 50.0); // arête vers le sud-ouest : dessinée par la voisine
        assert!(hex_water(&map, h).is_empty());
        assert!(!hex_water(&map, h + DIRECTIONS[4]).is_empty());
    }

    #[test]
    fn embouchure_dans_un_coin_de_lac() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set(h + DIRECTIONS[0], Terrain::Lake);
        map.set(h + DIRECTIONS[1], Terrain::Lake);
        map.set_fleuve_links(h, 1 << 0 | 1 << 3);
        // L'arête vers le second lac s'ouvre aussi ; pas celle vers la terre
        assert_eq!(fleuve_openings(&map, h).open, 1 << 0 | 1 << 1 | 1 << 3);
    }

    #[test]
    fn fleuve_qui_longe_un_lac() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set(h + DIRECTIONS[4], Terrain::Lake);
        map.set(h + DIRECTIONS[5], Terrain::Lake);
        // Le fleuve passe d'ouest en est sans se jeter dans le lac : il s'y ouvre quand même
        map.set_fleuve_links(h, 1 << 0 | 1 << 3);
        let o = fleuve_openings(&map, h);
        assert_eq!(o.open, 1 << 0 | 1 << 3 | 1 << 4 | 1 << 5);
        assert_eq!(o.full, 1 << 4 | 1 << 5);
        // La mer, elle, ne s'ouvre pas le long d'un fleuve côtier
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        map.set(h + DIRECTIONS[4], Terrain::Ocean);
        map.set_fleuve_links(h, 1 << 0 | 1 << 3);
        assert_eq!(fleuve_openings(&map, h).open, 1 << 0 | 1 << 3);
    }

    #[test]
    fn virage_serre_avant_embouchure() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set(h + DIRECTIONS[0], Terrain::Lake);
        // Le fleuve arrive par l'arête 2 et se jette dans le lac par l'arête 0 : l'intérieur du
        // virage (arête 1, de la terre) est rempli d'eau
        map.set_fleuve_links(h, 1 << 0 | 1 << 2);
        let o = fleuve_openings(&map, h);
        assert_eq!(o.open, 1 << 0 | 1 << 1 | 1 << 2);
        assert_eq!(o.full, 1 << 0 | 1 << 1);
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
        let m = crate::map::WorldMap::generate_in(192, 111, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let f: Vec<Hex> = m.hexes().filter(|&h| m.is_fleuve(h)).collect();
        println!("fleuves {}", f.len());
        for h in f.iter().take(10) {
            println!("{:?} q {:.0} links {:06b} tris {} terrain {:?}", h.to_offset(), m.discharge(*h), m.fleuve_links(*h), hex_water(&m, *h).tris.len(), m.get(*h));
        }
    }
}


