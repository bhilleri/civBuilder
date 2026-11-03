//! Formes des cours d'eau à dessiner (géométrie 2D pure, partagée par les vues 2D et 3D).
//!
//! Pour chaque case : les rivières de ses arêtes (une arête est dessinée par une seule des deux
//! cases, celle pour qui elle est dans les directions 0 à 2), le fleuve qui l'occupe (une bande
//! d'eau courbe d'une arête à l'autre en passant près du centre, berges de part et d'autre) et son
//! petit lac. La largeur suit le débit ; les fleuves serpentent (point de passage de chaque arête
//! et du milieu de la case tirés de la graine, d'autant plus en plaine) et les rivières courbent
//! de coin en coin en se raccordant à leurs tronçons d'amont et d'aval. Chaque sommet porte une teinte (débit, limon). Les triangles
//! sont découpés finement pour épouser le relief en 3D.

use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::hydro::{FLEUVE_M3S, RIVER_M3S};
use crate::map::{Terrain, WorldMap};
use bevy::math::{Vec2, Vec3};

/// Demi-largeur de la bande d'eau d'un fleuve au plus large (unités monde ; l'arête mesure
/// `HEX_SIZE`).
pub const FLEUVE_HALF_WIDTH: f32 = HEX_SIZE * 0.46;
/// Demi-largeur d'un fleuve à son seuil (`FLEUVE_M3S`).
const FLEUVE_MIN_HALF_WIDTH: f32 = HEX_SIZE * 0.25;
/// Débit à partir duquel un fleuve a toute sa largeur (m³/s ; le Mississippi, le Yangzi).
const FLEUVE_WIDE_M3S: f32 = 60_000.0;
/// Demi-largeur d'une rivière : au seuil (`RIVER_M3S`), gain par décade de débit, au plus.
const RIVER_MIN_HALF_WIDTH: f32 = 1.0;
const RIVER_WIDTH_PER_DECADE: f32 = 4.0;
const RIVER_MAX_HALF_WIDTH: f32 = 7.0;
/// Demi-largeur d'un fleuve à son embouchure : au plus toute l'arête, et évasement d'un fleuve
/// étroit (part de sa demi-largeur).
const ESTUARY_HALF_WIDTH: f32 = HEX_SIZE * 0.5;
const MOUTH_FLARE: f32 = 1.5;
/// Marge entre le bord de l'eau et le coin de l'arête qu'un fleuve franchit.
const EDGE_MARGIN: f32 = 1.5;
/// Écart du milieu d'un fleuve au centre de sa case, en part de sa demi-largeur (l'eau couvre
/// toujours le centre, où sont dessinées les unités).
const CENTER_SHIFT: f32 = 0.45;
/// Dénivelée (m) entre deux cases de fleuve au-delà de laquelle il ne serpente presque plus.
const MEANDER_DROP_M: f32 = 250.0;
/// Sinuosité la plus faible (fleuve en forte pente).
const MEANDER_MIN: f32 = 0.3;
/// Léger débord d'un tronçon sur la case voisine : pas de liseré entre deux tronçons.
const OVERLAP: f32 = 0.6;
/// Petit lac : décalage depuis le centre (le centre reste libre pour les unités) et rayons.
const SMALL_LAKE_AT: Vec2 = Vec2::new(10.0, 12.0);
const SMALL_LAKE_RADII: Vec2 = Vec2::new(8.5, 5.5);
/// Découpage des bandes (segments par arête ou par demi-case).
const STEPS: usize = 6;
/// Découpage d'une rivière d'arête.
const RIVER_STEPS: usize = 6;
/// Teinte : limon (sRGB) vers lequel tire une eau chargée, part au plus ; assombrissement des
/// plus grands débits.
const SILT_SRGB: [f32; 3] = [0.58, 0.52, 0.26];
const SILT_MAX: f32 = 0.65;
const DEEP_DARKEN: f32 = 0.3;
/// Rapides : pente (m/km) où ils commencent et où ils sont au plus fort ; taille des tronçons qui
/// en ont ; eau vive (sRGB) et part au plus ; écume et rochers (sRGB) ; amas d'écume au plus par
/// tronçon de lit (plus dans les larges), épaisseur d'un remous en V.
const RAPIDS_FROM_M_PER_KM: f32 = 3.0;
const RAPIDS_FULL_M_PER_KM: f32 = 9.0;
const STRETCH: f32 = 22.0;
const WHITEWATER_SRGB: [f32; 3] = [0.62, 0.80, 0.86];
const WHITEWATER_MAX: f32 = 0.7;
const FOAM_SRGB: [f32; 3] = [0.84, 0.92, 0.96];
const ROCK_SRGB: [f32; 3] = [0.42, 0.40, 0.38];
const FOAM_BLOBS: f32 = 7.0;
const CHEVRON_THICKNESS: f32 = 0.6;
/// Allongement des amas d'écume dans le sens du courant.
const FOAM_STRETCH: f32 = 2.4;
/// Affluent dans un fleuve : il le rejoint vers l'aval, le long de sa rive (part de la
/// demi-largeur du fleuve), puis sa couleur se fond dans celle du fleuve sur `PLUME` unités.
const INFLOW_BANK: f32 = 0.45;
const PLUME: f32 = 12.0;

/// Nature d'un morceau d'eau (couleurs distinctes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterKind {
    River,
    Fleuve,
    Lake,
    /// Écume des rapides : amas et remous posés sur l'eau.
    Foam,
    /// Rocher qui affleure dans des rapides.
    Rock,
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
            WaterKind::River | WaterKind::Foam | WaterKind::Rock => Bed { near: 3.0, far: 6.0, below: 1.1, bed: 1.4 },
            WaterKind::Fleuve => Bed { near: 7.0, far: 15.0, below: 1.5, bed: 2.2 },
            WaterKind::Lake => Bed { near: 4.5, far: 9.0, below: 0.9, bed: 1.2 },
        }
    }
}

/// Axe d'un tronçon d'eau (coordonnées monde 2D) : segment `a`–`b` (un disque si a = b) de
/// demi-largeur `hw`. `inflow` : affluent qui entre dans un fleuve, avec la part du chemin vers le
/// fleuve faite au début et à la fin du tronçon (sa hauteur rejoint progressivement celle du fleuve).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterAxis {
    pub a: Vec2,
    pub b: Vec2,
    pub hw: f32,
    /// Demi-largeur au bout `b` (un fleuve s'évase en arrivant dans un lac ou la mer).
    pub hw_end: f32,
    pub kind: WaterKind,
    pub also: Option<Hex>,
    pub inflow: Option<[f32; 2]>,
}

/// Teinte d'un point d'eau : abondance (0 : rivière au seuil, 0,5 : fleuve au seuil, 1 : plus
/// grands fleuves), turbidité (0 : claire, 1 : très chargée en limon) et rapides (0 : eau calme,
/// 1 : forte pente, écume par taches).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tone {
    pub size: f32,
    pub murk: f32,
    pub rapids: f32,
}

impl Tone {
    fn lerp(self, o: Tone, t: f32) -> Tone {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Tone { size: mix(self.size, o.size), murk: mix(self.murk, o.murk), rapids: mix(self.rapids, o.rapids) }
    }

    /// Couleur (linéaire) de l'eau de couleur de base `base` (linéaire) avec cette teinte au point
    /// `p` : tirée vers le limon, assombrie par le débit, éclaircie dans les rapides (eau vive).
    pub fn apply(self, base: Vec3, p: Vec2) -> Vec3 {
        let silt = Vec3::from(SILT_SRGB.map(srgb_to_linear));
        let water = base.lerp(silt, self.murk.clamp(0.0, 1.0) * SILT_MAX) * (1.0 - DEEP_DARKEN * self.size.clamp(0.0, 1.0));
        water.lerp(Vec3::from(WHITEWATER_SRGB.map(srgb_to_linear)), white_water(self.rapids, p) * WHITEWATER_MAX)
    }
}

/// Eau vive (0 à 1) en un point d'un cours d'eau de rapides `rapids` : seulement sur certains
/// tronçons, variation lente (la même quel que soit le découpage en triangles).
fn white_water(rapids: f32, p: Vec2) -> f32 {
    rapids.clamp(0.0, 1.0) * stretch(p)
}

/// Couleur (linéaire) de l'écume des rapides, et des rochers qui affleurent.
pub fn foam_color() -> Vec3 {
    Vec3::from(FOAM_SRGB.map(srgb_to_linear))
}
pub fn rock_color() -> Vec3 {
    Vec3::from(ROCK_SRGB.map(srgb_to_linear))
}

/// Valeur pseudo-aléatoire (0 à 1) attachée à un point (traînées d'écume).
fn hash01(p: Vec2) -> f32 {
    ((p.x * 12.9898 + p.y * 78.233).sin() * 43_758.547).rem_euclid(1.0)
}

/// Tronçons de rapides (0 à 1) : un bruit lisse à l'échelle de `STRETCH` unités, nul sur une
/// bonne moitié du cours (des rapides par endroits).
fn stretch(p: Vec2) -> f32 {
    let q = p / STRETCH;
    let (c, f) = (q.floor(), q - q.floor());
    let h = |x: f32, y: f32| ((x * 127.1 + y * 311.7).sin() * 43_758.547).rem_euclid(1.0);
    let s = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let top = h(c.x, c.y) + (h(c.x + 1.0, c.y) - h(c.x, c.y)) * s.x;
    let bottom = h(c.x, c.y + 1.0) + (h(c.x + 1.0, c.y + 1.0) - h(c.x, c.y + 1.0)) * s.x;
    ((top + (bottom - top) * s.y - 0.45) / 0.25).clamp(0.0, 1.0)
}

/// Rapides (0 à 1) d'un cours d'eau de pente `slope` (m/km).
fn rapids(slope: f32) -> f32 {
    let x = ((slope - RAPIDS_FROM_M_PER_KM) / (RAPIDS_FULL_M_PER_KM - RAPIDS_FROM_M_PER_KM)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Triangle d'eau (coordonnées monde 2D) et teinte de ses sommets. Une rivière d'arête se voit
/// aussi depuis la case d'en face (`also`) : elle apparaît dès que l'une des deux rives est
/// explorée. `channel` : indice du lit auquel il appartient.
pub struct WaterTri {
    pub pts: [Vec2; 3],
    pub tones: [Tone; 3],
    pub kind: WaterKind,
    pub also: Option<Hex>,
    pub channel: usize,
}

impl WaterTri {
    /// Teinte en un point du triangle (interpolée entre ses sommets).
    pub fn tone_at(&self, p: Vec2) -> Tone {
        let [a, b, c] = self.pts;
        let area = (b - a).perp_dot(c - a);
        if area.abs() < 1e-6 {
            return self.tones[0];
        }
        let wb = (p - a).perp_dot(c - a) / area;
        let wc = (b - a).perp_dot(p - a) / area;
        let wa = 1.0 - wb - wc;
        let [ta, tb, tc] = self.tones;
        let mix = |f: fn(&Tone) -> f32| f(&ta) * wa + f(&tb) * wb + f(&tc) * wc;
        Tone { size: mix(|t| t.size), murk: mix(|t| t.murk), rapids: mix(|t| t.rapids) }
    }
}

/// Triangles d'eau d'une case, et lits correspondants (pour creuser le relief en 3D).
#[derive(Default)]
pub struct WaterShape {
    pub tris: Vec<WaterTri>,
    pub channels: Vec<WaterAxis>,
    /// Case d'en face des prochains triangles ajoutés, affluent d'un fleuve, et teinte des
    /// prochains triangles de teinte uniforme.
    also: Option<Hex>,
    inflow: bool,
    tone: Tone,
}

/// Part d'un débit entre deux bornes, en échelle logarithmique (0 à 1).
fn log_share(q: f32, lo: f32, hi: f32) -> f32 {
    ((q / lo).ln() / (hi / lo).ln()).clamp(0.0, 1.0)
}

/// Demi-largeur d'une rivière selon son débit : elle s'élargit avec le logarithme du débit.
pub fn river_half_width(q: f32) -> f32 {
    (RIVER_MIN_HALF_WIDTH + RIVER_WIDTH_PER_DECADE * (q / RIVER_M3S).log10().max(0.0)).min(RIVER_MAX_HALF_WIDTH)
}

/// Demi-largeur d'un fleuve selon son débit : du seuil d'un fleuve (un quart de case) aux plus
/// grands (presque toute la case), avec le logarithme du débit.
pub fn fleuve_half_width(q: f32) -> f32 {
    FLEUVE_MIN_HALF_WIDTH + (FLEUVE_HALF_WIDTH - FLEUVE_MIN_HALF_WIDTH) * log_share(q, FLEUVE_M3S, FLEUVE_WIDE_M3S)
}

/// Teinte d'une eau de débit `q` (m³/s) et de turbidité `murk`.
pub fn tone(q: f32, murk: f32) -> Tone {
    let size = if q < FLEUVE_M3S { 0.5 * log_share(q, RIVER_M3S, FLEUVE_M3S) } else { 0.5 + 0.5 * log_share(q, FLEUVE_M3S, FLEUVE_WIDE_M3S) };
    Tone { size, murk, rapids: 0.0 }
}

/// Point d'une ligne d'eau : position, demi-largeur, teinte.
type PathPoint = (Vec2, f32, Tone);

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

    fn tri(&mut self, pts: [Vec2; 3], tones: [Tone; 3], kind: WaterKind) {
        let channel = self.channels.len() - 1;
        self.tris.push(WaterTri { pts, tones, kind, also: self.also, channel });
    }

    /// Enregistre le lit d'un tronçon (`span` : part du chemin d'un affluent vers son fleuve).
    fn channel(&mut self, a: Vec2, b: Vec2, hw: f32, hw_end: f32, kind: WaterKind, span: [f32; 2]) {
        let inflow = self.inflow.then_some(span);
        self.channels.push(WaterAxis { a, b, hw, hw_end, kind, also: self.also, inflow });
    }

    /// Bande droite de `a` à `b` dont la demi-largeur passe de `hw` à `hw_end`.
    fn flared_band(&mut self, a: Vec2, b: Vec2, hw: f32, hw_end: f32, kind: WaterKind) {
        let tone = self.tone;
        let path: Vec<PathPoint> = (0..=STEPS)
            .map(|k| {
                let t = k as f32 / STEPS as f32;
                (a.lerp(b, t), hw + (hw_end - hw) * t, tone)
            })
            .collect();
        self.strip(&path, kind);
    }

    /// Bande le long d'une ligne de points (un lit par tronçon) : en travers de chaque point, la
    /// direction moyenne de ses deux tronçons (pas de fente ni de recouvrement aux coudes).
    fn strip(&mut self, path: &[PathPoint], kind: WaterKind) {
        self.rimmed_strip(path, None, kind);
    }

    /// Bande dont les bords prennent une autre teinte que le milieu (`rims`, une par point) :
    /// une eau qui se fond dans celle où elle entre.
    fn rimmed_strip(&mut self, path: &[PathPoint], rims: Option<&[Tone]>, kind: WaterKind) {
        // Les bandes larges sont aussi découpées en travers (elles épousent mieux le relief)
        let across = if path.iter().any(|p| p.1 > 6.0) { 4 } else if rims.is_some() { 2 } else { 1 };
        let last = path.len() - 1;
        let side = |k: usize| {
            let (before, after) = (path[k.saturating_sub(1)].0, path[(k + 1).min(last)].0);
            (after - before).perp().normalize_or_zero()
        };
        let at = |k: usize, j: usize| path[k].0 + side(k) * path[k].1 * (2.0 * j as f32 / across as f32 - 1.0);
        let tone = |k: usize, j: usize| match rims {
            Some(r) if j == 0 || j == across => r[k],
            _ => path[k].2,
        };
        for k in 0..last {
            let ((a, ha, ta), (b, hb, tb)) = (path[k], path[k + 1]);
            self.channel(a, b, ha, hb, kind, [k as f32 / last as f32, (k + 1) as f32 / last as f32]);
            for j in 0..across {
                self.tri([at(k, j), at(k + 1, j), at(k + 1, j + 1)], [tone(k, j), tone(k + 1, j), tone(k + 1, j + 1)], kind);
                self.tri([at(k, j), at(k + 1, j + 1), at(k, j + 1)], [tone(k, j), tone(k + 1, j + 1), tone(k, j + 1)], kind);
            }
            // Rapides : amas d'écume sur toute la largeur, remous en V, quelques rochers
            let rapid = (ta.rapids + tb.rapids) / 2.0;
            if rapid > 0.0 && kind != WaterKind::Lake {
                self.rapids(a, b, ha, hb, white_water(rapid, (a + b) / 2.0));
            }
        }
    }

    /// Rapides sur un tronçon de lit de `a` à `b` (demi-largeurs `ha`, `hb`), d'intensité `w`
    /// (0 à 1) : amas d'écume irréguliers sur toute la largeur, un remous en V (pointe vers
    /// l'amont, ou l'aval : le sens du tronçon n'est pas toujours celui du courant), parfois un
    /// rocher. Tout est tiré au point, la même forme à chaque affichage.
    fn rapids(&mut self, a: Vec2, b: Vec2, ha: f32, hb: f32, w: f32) {
        if w < 0.05 {
            return;
        }
        let channel = self.channels.len() - 1;
        let along = (b - a).normalize_or_zero();
        let across = along.perp();
        let half = (ha + hb) / 2.0;
        let blob = |shape: &mut Self, c: Vec2, r: f32, kind: WaterKind, seed: f32| {
            const SIDES: usize = 6;
            let ring: Vec<Vec2> = (0..SIDES)
                .map(|k| {
                    let ang = std::f32::consts::TAU * (k as f32 + 0.5 * hash01(c + Vec2::splat(seed + k as f32))) / SIDES as f32;
                    // Allongé dans le sens du courant, bords irréguliers
                    let jitter = 0.6 + 0.6 * hash01(c * 1.7 + Vec2::splat(k as f32 + seed));
                    c + (along * ang.cos() * FOAM_STRETCH + across * ang.sin() * 0.7) * r * jitter
                })
                .collect();
            for k in 0..SIDES {
                shape.tris.push(WaterTri { pts: [c, ring[k], ring[(k + 1) % SIDES]], tones: [Tone::default(); 3], kind, also: shape.also, channel });
            }
        };
        // Amas d'écume : d'autant plus nombreux que l'eau est vive et la rivière large
        let count = (w * FOAM_BLOBS * (0.6 + half / 4.0)).round() as usize;
        for i in 0..count {
            let seed = i as f32 * 3.1;
            let t = hash01(a + Vec2::splat(seed));
            let c = a.lerp(b, t);
            let h = ha + (hb - ha) * t;
            // (assez grands pour se rejoindre en plaques d'eau blanche)
            let r = (0.3 + 0.35 * hash01(c + Vec2::splat(seed + 1.0))) * (0.6 + 0.4 * w) * h.max(1.2);
            let shift = (h - r).max(0.0) * (2.0 * hash01(c + Vec2::splat(seed + 2.0)) - 1.0);
            blob(self, c + across * shift, r, WaterKind::Foam, seed);
        }
        // Remous en V d'une rive à l'autre
        if hash01(a * 0.7) < w {
            let tip = a.lerp(b, 0.35);
            let back = a.lerp(b, 0.85);
            let t = CHEVRON_THICKNESS;
            for side in [1.0, -1.0] {
                let bank = back + across * side * half * 0.85;
                let n = (bank - tip).perp().normalize_or_zero() * t;
                self.tris.push(WaterTri { pts: [tip - n, bank - n, bank + n], tones: [Tone::default(); 3], kind: WaterKind::Foam, also: self.also, channel });
                self.tris.push(WaterTri { pts: [tip - n, bank + n, tip + n], tones: [Tone::default(); 3], kind: WaterKind::Foam, also: self.also, channel });
            }
        }
        // Un rocher qui affleure, parfois
        if hash01(b * 1.3) < w * 0.35 && half > 1.5 {
            let c = a.lerp(b, 0.5) + across * half * 0.5 * (2.0 * hash01(b) - 1.0);
            blob(self, c, (0.5 + 0.4 * hash01(c)) * half.min(3.0) / 2.5, WaterKind::Rock, 7.0);
        }
    }

    /// Disque (polygone) de centre `c` et de rayons `r`.
    fn disc(&mut self, c: Vec2, r: Vec2, kind: WaterKind) {
        const SIDES: usize = 12;
        // Lit : segment le long du grand axe (une ellipse en largeur, un disque si r.x = r.y)
        let half = Vec2::X * (r.x - r.y).max(0.0);
        self.channel(c - half, c + half, r.y, r.y, kind, [1.0, 1.0]);
        let at = |k: usize| {
            let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
            c + Vec2::new(a.cos() * r.x, a.sin() * r.y)
        };
        let tone = self.tone;
        for k in 0..SIDES {
            self.tri([c, at(k), at(k + 1)], [tone; 3], kind);
        }
    }
}

/// Points d'une courbe de Hermite de `p0` (tangente `m0`) à `p1` (tangente `m1`), `p0` exclu.
fn hermite(p0: Vec2, m0: Vec2, p1: Vec2, m1: Vec2, steps: usize) -> impl Iterator<Item = (f32, Vec2)> {
    (1..=steps).map(move |k| {
        let t = k as f32 / steps as f32;
        let (t2, t3) = (t * t, t * t * t);
        let p = p0 * (2.0 * t3 - 3.0 * t2 + 1.0) + m0 * (t3 - 2.0 * t2 + t) + p1 * (-2.0 * t3 + 3.0 * t2) + m1 * (t3 - t2);
        (t, p)
    })
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
///   et le tronçon amont) : l'eau d'un fleuve large l'occupe, au lieu d'une pointe de terre entre
///   le fleuve et le lac (un fleuve étroit la garde).
pub fn fleuve_openings(map: &WorldMap, hex: Hex) -> Openings {
    let links = map.fleuve_links(hex);
    // (un fleuve étroit garde la pointe de terre de son dernier virage : rien à combler)
    let wide = fleuve_half_width(map.discharge(hex)) * MOUTH_FLARE >= ESTUARY_HALF_WIDTH;
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
            if water_side(e) || (wide && links & (1 << beyond) != 0 && !map.is_fleuve(hex + DIRECTIONS[e])) {
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

/// Milieu de l'arête `d` d'une case.
fn edge_mid(hex: Hex, d: usize) -> Vec2 {
    let (a, b) = hex.edge_corners(d);
    (v(a) + v(b)) / 2.0
}

/// Valeur pseudo-aléatoire (−1 à 1) attachée à une case de la carte et à un usage (`salt`) :
/// la même pour une graine donnée, de quelque côté du raccord est-ouest qu'on la regarde.
fn noise(map: &WorldMap, hex: Hex, salt: u32) -> f32 {
    let h = map.canonical(hex);
    let mut x = (h.q as u32).wrapping_mul(0x9E37_79B1) ^ (h.r as u32).wrapping_mul(0x85EB_CA77) ^ salt.wrapping_mul(0xC2B2_AE3D) ^ map.seed().wrapping_mul(0x27D4_EB2F);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x as f32 / u32::MAX as f32 * 2.0 - 1.0
}

/// L'arête `d` d'une case vue de la case qui la porte (directions 0 à 2) : cette case (non
/// ramenée dans la carte, pour garder la géométrie) et la direction.
fn edge_owner(hex: Hex, d: usize) -> (Hex, usize) {
    if d < 3 { (hex, d) } else { (hex + DIRECTIONS[d], d - 3) }
}

/// Sinuosité d'un fleuve entre deux cases (`MEANDER_MIN` à 1) : il serpente en plaine et file
/// droit dans une pente.
fn meander(map: &WorldMap, a: Hex, b: Hex) -> f32 {
    let drop = (map.elevation(a).unwrap_or(0.0) - map.elevation(b).unwrap_or(0.0)).abs();
    (1.0 - drop / MEANDER_DROP_M).clamp(MEANDER_MIN, 1.0)
}

/// Demi-largeur d'un fleuve sur l'arête `d` de sa case : celle du plus petit des deux débits
/// (l'eau qui passe l'arête), la même vue des deux cases.
fn fleuve_edge_half_width(map: &WorldMap, hex: Hex, d: usize) -> f32 {
    let nb = hex + DIRECTIONS[d];
    let q = if map.is_fleuve(nb) { map.discharge(hex).min(map.discharge(nb)) } else { map.discharge(hex) };
    fleuve_half_width(q)
}

/// Point où un fleuve franchit l'arête `d` de sa case : vers une autre case de fleuve, glissé
/// le long de l'arête (graine, d'autant plus en plaine), sans que l'eau en déborde, et le même
/// vu des deux cases ; ailleurs, le milieu de l'arête.
pub fn fleuve_crossing(map: &WorldMap, hex: Hex, d: usize) -> Vec2 {
    let nb = hex + DIRECTIONS[d];
    if !map.is_fleuve(nb) || map.get(nb).is_some_and(|t| t.is_water()) {
        return edge_mid(hex, d);
    }
    let hw = fleuve_edge_half_width(map, hex, d);
    let room = ((HEX_SIZE * 0.5 - EDGE_MARGIN - hw) / HEX_SIZE).max(0.0);
    let (owner, od) = edge_owner(hex, d);
    let s = noise(map, owner, 10 + od as u32) * room * meander(map, hex, nb);
    let (a, b) = owner.edge_corners(od);
    v(a).lerp(v(b), 0.5 + s)
}

/// Point où passe un fleuve au milieu de sa case : un peu écarté du centre, en travers de son
/// cours, quand il y entre par une arête et en sort par une autre (l'eau couvre toujours le
/// centre, où sont dessinées les unités) ; le centre sinon.
pub fn fleuve_center(map: &WorldMap, hex: Hex) -> Vec2 {
    let c = center(hex);
    let links = map.fleuve_links(hex);
    let dirs: Vec<usize> = (0..6).filter(|d| links & (1 << d) != 0).collect();
    if dirs.len() != 2 {
        return c;
    }
    let along = (fleuve_crossing(map, hex, dirs[1]) - fleuve_crossing(map, hex, dirs[0])).normalize_or_zero();
    let sinuous = map.downstream(hex).map_or(1.0, |d| meander(map, hex, hex + DIRECTIONS[d]));
    c + along.perp() * noise(map, hex, 3) * fleuve_half_width(map.discharge(hex)) * CENTER_SHIFT * sinuous
}

/// Rivière d'une arête : bouts amont et aval, débit et teinte.
struct EdgeRiver {
    up: Vec2,
    down: Vec2,
    q: f32,
    tone: Tone,
}

/// Rivière de l'arête `d` d'une case (sans sens connu, du premier coin de l'arête au second).
fn edge_river(map: &WorldMap, hex: Hex, d: usize) -> Option<EdgeRiver> {
    let q = map.river_flow(hex, d);
    if q <= 0.0 {
        return None;
    }
    let (a, b) = hex.edge_corners(d);
    let (a, b) = (v(a), v(b));
    let down = map.river_down_corner(hex, d).map(|(h, j)| corner(h, j));
    let flows_to_a = down.is_some_and(|p| p.distance(a) < 0.5);
    let (up, down) = if flows_to_a { (b, a) } else { (a, b) };
    // Pente : altitude des coins (moyenne des trois cases autour), le long de l'arête
    let (j_up, j_down) = if flows_to_a { ((d + 1) % 6, d) } else { (d, (d + 1) % 6) };
    let edge_km = crate::climate::CELL_KM / 3f32.sqrt();
    let slope = (corner_elevation(map, hex, j_up) - corner_elevation(map, hex, j_down)) / edge_km;
    let tone = tone(q, (map.water_murk(hex) + map.water_murk(hex + DIRECTIONS[d])) / 2.0);
    Some(EdgeRiver { up, down, q, tone: Tone { rapids: rapids(slope), ..tone } })
}

/// Altitude (m) du coin `j` d'une case : moyenne des trois cases qui s'y touchent (la mer à 0).
fn corner_elevation(map: &WorldMap, hex: Hex, j: usize) -> f32 {
    [hex, hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]].iter().map(|&h| map.elevation(h).unwrap_or(0.0)).sum::<f32>() / 3.0
}

/// Rapides d'un fleuve : pente vers sa case aval (m/km).
fn fleuve_rapids(map: &WorldMap, hex: Hex) -> f32 {
    map.downstream(hex).map_or(0.0, |d| {
        let drop = map.elevation(hex).unwrap_or(0.0) - map.elevation(hex + DIRECTIONS[d]).unwrap_or(0.0);
        rapids(drop / crate::climate::CELL_KM)
    })
}

/// Rivières des arêtes qui touchent le coin `j` de la case, sauf celle de son arête `d` (les
/// trois arêtes d'un coin : deux de la case, une entre ses deux voisines).
fn rivers_at_corner(map: &WorldMap, hex: Hex, j: usize, d: usize) -> impl Iterator<Item = EdgeRiver> + '_ {
    let (n1, n2) = (hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]);
    let between = DIRECTIONS.iter().position(|&s| n1 + s == n2).unwrap_or(0);
    [(hex, (j + 5) % 6), (hex, j), (n1, between)].into_iter().filter(move |&(h, e)| !(h == hex && e == d)).filter_map(|(h, e)| edge_river(map, h, e))
}

/// Formes d'eau d'une case.
pub fn hex_water(map: &WorldMap, hex: Hex) -> WaterShape {
    let mut shape = WaterShape::default();
    // Rivières des arêtes « à soi » (directions 0 à 2) : une courbe de coin en coin qui se
    // raccorde à celles d'amont et d'aval, avec un rond à chaque bout (confluents)
    for (d, &step) in DIRECTIONS.iter().enumerate().take(3) {
        let Some(own) = edge_river(map, hex, d) else { continue };
        let along = (own.down - own.up).normalize_or_zero();
        // Tronçons voisins aux deux bouts, dans le sens du courant (en amont, le plus abondant)
        let (j_up, j_down) = if own.up.distance(corner(hex, d)) < 0.5 { (d, (d + 1) % 6) } else { ((d + 1) % 6, d) };
        let main = |list: Vec<EdgeRiver>| list.into_iter().max_by(|x, y| x.q.total_cmp(&y.q));
        let prev = main(rivers_at_corner(map, hex, j_up, d).filter(|r| r.down.distance(own.up) < 0.5).collect());
        let next = main(rivers_at_corner(map, hex, j_down, d).filter(|r| r.up.distance(own.down) < 0.5).collect());
        // Au coin, la direction moyenne des deux tronçons : pas de cassure d'une arête à l'autre
        let bisect = |r: &EdgeRiver| (along + (r.down - r.up).normalize_or_zero()).normalize_or(along);
        let (t_up, t_down) = (prev.as_ref().map_or(along, bisect), next.as_ref().map_or(along, bisect));
        let hw = river_half_width(own.q);
        let (hw_up, tone_up) = prev.as_ref().map_or((hw, own.tone), |r| (river_half_width(r.q), r.tone));
        let length = own.up.distance(own.down);
        let mut path: Vec<PathPoint> = vec![(own.up, hw_up, tone_up)];
        path.extend(hermite(own.up, t_up * length, own.down, t_down * length, RIVER_STEPS).map(|(s, p)| (p, hw_up + (hw - hw_up) * s, tone_up.lerp(own.tone, s))));
        shape.also = Some(hex + step);
        shape.strip(&path, WaterKind::River);
        shape.tone = tone_up;
        shape.disc(own.up, Vec2::splat(hw_up), WaterKind::River);
        shape.tone = own.tone;
        shape.disc(own.down, Vec2::splat(hw), WaterKind::River);
        shape.also = None;
    }
    if map.is_fleuve(hex) {
        add_fleuve(map, hex, &mut shape);
    }
    if map.small_lake(hex) && !map.is_fleuve(hex) {
        shape.tone = Tone::default();
        shape.disc(center(hex) + SMALL_LAKE_AT, SMALL_LAKE_RADII, WaterKind::Lake);
    }
    shape
}

/// Fleuve d'une case : une courbe d'une arête à l'autre par le milieu du fleuve quand il ne fait
/// que traverser la case ; sinon un rond au milieu et un bras courbe vers chaque arête ouverte
/// (confluent, source, embouchure, rive de lac). Les courbes arrivent perpendiculaires aux arêtes
/// (raccord lisse d'une case à l'autre).
fn add_fleuve(map: &WorldMap, hex: Hex, shape: &mut WaterShape) {
    let c = center(hex);
    let mid = fleuve_center(map, hex);
    let q = map.discharge(hex);
    let hw = fleuve_half_width(q);
    let murk = map.water_murk(hex);
    let own = Tone { rapids: fleuve_rapids(map, hex), ..tone(q, murk) };
    let Openings { open, full } = fleuve_openings(map, hex);
    // Bout d'un bras : point de passage, demi-largeur, teinte, direction vers l'extérieur
    let end = |d: usize| {
        let out = (edge_mid(hex, d) - c).normalize_or_zero();
        let nb = hex + DIRECTIONS[d];
        if full & (1 << d) != 0 {
            // Embouchure, rive d'un lac : le fleuve s'évase, d'autant plus qu'il est large (au plus
            // toute l'arête) ; les coins entre deux ouvertures sont remplis plus bas (le lac et le
            // fleuve forment une seule nappe d'eau)
            return (edge_mid(hex, d), (hw * MOUTH_FLARE).min(ESTUARY_HALF_WIDTH), own, out);
        }
        let t = if map.is_fleuve(nb) {
            let rapids = (own.rapids + fleuve_rapids(map, nb)) / 2.0;
            Tone { rapids, ..tone(q.min(map.discharge(nb)), (murk + map.water_murk(nb)) / 2.0) }
        } else {
            own
        };
        (fleuve_crossing(map, hex, d), fleuve_edge_half_width(map, hex, d), t, out)
    };
    let dirs: Vec<usize> = (0..6).filter(|d| open & (1 << d) != 0).collect();
    shape.tone = own;
    // Axe de l'eau du fleuve dans la case, avec le sens du courant (où les affluents le
    // rejoignent) : points d'une ligne parcourue dans le sens `sign` (+1 ou −1)
    let down_dir = map.downstream(hex);
    let mut course: Vec<(Vec2, Vec2)> = Vec::new();
    let mut follow = |path: &[PathPoint], sign: f32| {
        let last = path.len() - 1;
        for k in 0..=last {
            let (before, after) = (path[k.saturating_sub(1)].0, path[(k + 1).min(last)].0);
            course.push((path[k].0, (after - before).normalize_or_zero() * sign));
        }
    };
    // (deux ouvertures sur l'eau côte à côte : l'embouchure dans un coin, rond et coin rempli)
    let wet_pair = dirs.len() == 2 && dirs.iter().all(|&d| full & (1 << d) != 0);
    if dirs.len() == 2 && !wet_pair {
        // Traversée : d'une arête au milieu du fleuve, puis à l'autre arête
        let ((p0, h0, t0, out0), (p1, h1, t1, out1)) = (end(dirs[0]), end(dirs[1]));
        let along = (p1 - p0).normalize_or(-out0);
        let mut path: Vec<PathPoint> = vec![(p0 + out0 * OVERLAP, h0, t0), (p0, h0, t0)];
        let l0 = p0.distance(mid);
        path.extend(hermite(p0, -out0 * l0, mid, along * l0, STEPS).map(|(s, p)| (p, h0 + (hw - h0) * s, t0.lerp(own, s))));
        let l1 = mid.distance(p1);
        path.extend(hermite(mid, along * l1, p1, out1 * l1, STEPS).map(|(s, p)| (p, hw + (h1 - hw) * s, own.lerp(t1, s))));
        path.push((p1 + out1 * OVERLAP, h1, t1));
        follow(&path, if down_dir == Some(dirs[0]) { -1.0 } else { 1.0 });
        shape.strip(&path, WaterKind::Fleuve);
    } else {
        shape.disc(mid, Vec2::splat(hw), WaterKind::Fleuve);
        for &d in &dirs {
            let (p, h, t, out) = end(d);
            let l = mid.distance(p);
            let mut path: Vec<PathPoint> = vec![(mid, hw, own)];
            path.extend(hermite(mid, (p - mid).normalize_or_zero() * l, p, out * l, STEPS).map(|(s, x)| (x, hw + (h - hw) * s, own.lerp(t, s))));
            path.push((p + out * OVERLAP, h, t));
            follow(&path, if down_dir == Some(d) { 1.0 } else { -1.0 });
            shape.strip(&path, WaterKind::Fleuve);
        }
        // Deux arêtes ouvertes côte à côte, l'une sur toute sa largeur ou toutes deux sur l'eau
        // (embouchure dans un coin de mer) : l'eau remplit aussi le coin qui les sépare (pas de
        // pointe de terre entre elles)
        let whole = |d: usize| end(d).1 >= ESTUARY_HALF_WIDTH - 1e-3;
        let wet = |d: usize| full & (1 << d) != 0;
        for d in 0..6 {
            let e = (d + 1) % 6;
            let both = open & (1 << d) != 0 && open & (1 << e) != 0;
            if both && (whole(d) || whole(e) || (wet(d) && wet(e))) {
                let k = corner(hex, e);
                let k = k + (k - c).normalize_or_zero() * OVERLAP;
                let tip = if whole(d) || whole(e) { HEX_SIZE * 0.25 } else { end(d).1.min(end(e).1) };
                shape.flared_band(mid, k, hw, tip, WaterKind::Fleuve);
            }
        }
    }
    // Affluents (rivières d'arête) qui arrivent à un coin : ils continuent dans leur direction
    // et courbent vers l'aval jusqu'au fleuve, le long de sa rive ; dans le fleuve, leur couleur
    // se fond peu à peu dans la sienne (bords d'abord, puis le milieu, sur `PLUME` unités)
    if course.is_empty() {
        course.push((mid, Vec2::ZERO));
    }
    for j in 0..6 {
        let (a, b) = (hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]);
        let Some(dir) = DIRECTIONS.iter().position(|&dd| a + dd == b) else { continue };
        let Some(trib) = edge_river(map, a, dir) else { continue };
        let k = corner(hex, j);
        let &(axis, flow) = course.iter().min_by(|p, q| p.0.distance(k).total_cmp(&q.0.distance(k))).unwrap_or(&(mid, Vec2::ZERO));
        let toward = (axis - k).normalize_or_zero();
        let flow = if flow == Vec2::ZERO { toward } else { flow };
        // Du côté de l'affluent, un peu vers l'aval
        let side = flow.perp() * (k - axis).dot(flow.perp()).signum();
        let target = axis + side * hw * INFLOW_BANK + flow * hw * 0.6;
        let incoming = if trib.down.distance(k) < 0.5 { (trib.down - trib.up).normalize_or(toward) } else { toward };
        let (l, hw_r) = (k.distance(target), river_half_width(trib.q));
        let mut pts: Vec<Vec2> = vec![k];
        pts.extend(hermite(k, incoming * l, target, flow * l, STEPS).map(|(_, p)| p));
        // Puis le long de la rive, en suivant le fleuve (ses virages compris)
        let nearest = |p: Vec2| *course.iter().min_by(|a, b| a.0.distance(p).total_cmp(&b.0.distance(p))).unwrap_or(&(mid, flow));
        let bank_side = (k - axis).dot(flow.perp()).signum();
        let mut at = target;
        for _ in 0..3 {
            let ahead = at + nearest(at).1 * PLUME / 3.0;
            let (a, f) = nearest(ahead);
            if f == Vec2::ZERO || a.distance(ahead) > hw {
                break;
            }
            at = a + f.perp() * bank_side * hw * INFLOW_BANK;
            pts.push(at);
        }
        // Dans l'eau du fleuve : à moins de sa demi-largeur de son axe
        let wet = |p: Vec2| course.iter().any(|c| c.0.distance(p) < hw * 0.9);
        let first_wet = pts.iter().position(|&p| wet(p)).unwrap_or(pts.len() - 1);
        let last = pts.len() - 1;
        let mut path: Vec<PathPoint> = Vec::new();
        let mut rims: Vec<Tone> = Vec::new();
        for (i, &p) in pts.iter().enumerate() {
            let fade = if i < first_wet { 0.0 } else { (i - first_wet + 1) as f32 / (last - first_wet + 1) as f32 };
            let width = if i <= STEPS { hw_r } else { hw_r * (1.0 + 0.2 * (i - STEPS) as f32) };
            path.push((p, width, trib.tone.lerp(own, fade)));
            rims.push(if i < first_wet { trib.tone } else { own });
        }
        shape.inflow = true;
        shape.rimmed_strip(&path, Some(&rims), WaterKind::River);
        shape.inflow = false;
    }
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
        map.set_discharge(h, 50_000.0);
        let o = fleuve_openings(&map, h);
        assert_eq!(o.open, 1 << 0 | 1 << 1 | 1 << 2);
        assert_eq!(o.full, 1 << 0 | 1 << 1);
        // Un fleuve étroit garde la pointe de terre
        map.set_discharge(h, 2_500.0);
        assert_eq!(fleuve_openings(&map, h).open, 1 << 0 | 1 << 2);
    }

    #[test]
    fn largeur_croissante() {
        assert!(river_half_width(RIVER_M3S) < river_half_width(200.0));
        assert!(river_half_width(200.0) < FLEUVE_HALF_WIDTH);
        // Une grosse rivière n'est pas plus large qu'un petit fleuve ; les fleuves s'élargissent
        assert!(river_half_width(FLEUVE_M3S) <= fleuve_half_width(FLEUVE_M3S));
        assert!(fleuve_half_width(FLEUVE_M3S) < fleuve_half_width(20_000.0));
        assert!(fleuve_half_width(20_000.0) < fleuve_half_width(1e6) && fleuve_half_width(1e6) == FLEUVE_HALF_WIDTH);
        assert!(tone(RIVER_M3S, 0.0).size < tone(FLEUVE_M3S, 0.0).size && tone(FLEUVE_M3S, 0.0).size < tone(1e5, 0.0).size);
    }

    /// Un fleuve d'ouest en est sur toute une rangée, de débit croissant.
    fn fleuve_rangee(seed_row: i32) -> (WorldMap, Vec<Hex>) {
        let mut map = WorldMap::filled(10, 10, Terrain::Plains);
        let row: Vec<Hex> = (1..9).map(|c| Hex::from_offset(c, seed_row)).collect();
        for (k, w) in row.windows(2).enumerate() {
            let d = DIRECTIONS.iter().position(|&dd| w[0] + dd == w[1]).unwrap();
            map.link_fleuve(w[0], d, 2_500.0 * (k + 1) as f32);
        }
        (map, row)
    }

    #[test]
    fn meandres_raccordes_d_une_case_a_l_autre() {
        let (map, row) = fleuve_rangee(4);
        let mut shifted = 0;
        for w in row.windows(2) {
            let d = DIRECTIONS.iter().position(|&dd| w[0] + dd == w[1]).unwrap();
            let (here, there) = (fleuve_crossing(&map, w[0], d), fleuve_crossing(&map, w[1], (d + 3) % 6));
            assert!(here.distance(there) < 1e-3, "{here} {there}");
            // L'eau reste dans l'arête
            let (a, b) = w[0].edge_corners(d);
            let hw = fleuve_edge_half_width(&map, w[0], d);
            assert!(here.distance(v(a)) >= hw + EDGE_MARGIN - 1e-3 && here.distance(v(b)) >= hw + EDGE_MARGIN - 1e-3);
            if here.distance(edge_mid(w[0], d)) > 0.5 {
                shifted += 1;
            }
        }
        assert!(shifted >= 3, "le fleuve doit serpenter ({shifted} arêtes décalées)");
    }

    #[test]
    fn l_eau_d_un_fleuve_couvre_le_centre() {
        let (map, row) = fleuve_rangee(5);
        let inside = |t: &WaterTri, p: Vec2| {
            let [a, b, c] = t.pts;
            let (d1, d2, d3) = ((b - a).perp_dot(p - a), (c - b).perp_dot(p - b), (a - c).perp_dot(p - c));
            !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
        };
        for &h in &row {
            let shape = hex_water(&map, h);
            assert!(shape.tris.iter().any(|t| t.kind == WaterKind::Fleuve && inside(t, center(h))), "{:?}", h.to_offset());
        }
    }

    #[test]
    fn rivieres_raccordees_aux_coins() {
        // Deux arêtes de la même case : l'eau coule du coin 0 au coin 1, puis du coin 1 au coin 2
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set_river(h, 0, 300.0);
        map.set_river(h, 1, 600.0);
        map.orient_river(h, 0, 1);
        map.orient_river(h, 1, 2);
        let shape = hex_water(&map, h);
        let strips: Vec<&WaterAxis> = shape.channels.iter().filter(|c| c.a != c.b).collect();
        assert_eq!(strips.len(), 2 * RIVER_STEPS);
        let (last, first) = (strips[RIVER_STEPS - 1], strips[RIVER_STEPS]);
        // Même point, même largeur, presque même direction au coin : 60° entre les deux arêtes,
        // moins de 25° entre les cordes des deux petits tronçons qui l'encadrent (même tangente)
        assert!(last.b.distance(first.a) < 1e-3);
        let angle = (last.b - last.a).angle_to(first.b - first.a).abs().to_degrees();
        assert!(angle < 25.0, "cassure de {angle:.0}° au coin");
        assert!((last.hw_end - first.hw).abs() < 1e-3 && first.hw_end > first.hw);
    }

    #[test]
    fn affluent_relie_a_son_fleuve() {
        // Fleuve à sa source (une seule arête) ; une rivière arrive à un coin de sa case
        let (mut map, row) = fleuve_rangee(4);
        let h = row[0];
        let j = 2;
        let (a, b) = (h + DIRECTIONS[(j + 5) % 6], h + DIRECTIONS[j]);
        let dir = DIRECTIONS.iter().position(|&dd| a + dd == b).unwrap();
        map.set_river(a, dir, 400.0);
        let shape = hex_water(&map, h);
        let inflow: Vec<&WaterAxis> = shape.channels.iter().filter(|c| c.inflow.is_some()).collect();
        assert!(inflow.len() > STEPS && inflow.len() <= STEPS + 3);
        // Il part du coin, finit dans l'eau du fleuve, et rejoint sa hauteur d'un bout à l'autre
        assert!(inflow[0].a.distance(corner(h, j)) < 1e-3);
        let end = inflow.last().unwrap();
        let inside = |t: &WaterTri, p: Vec2| {
            let [a, b, c] = t.pts;
            let (d1, d2, d3) = ((b - a).perp_dot(p - a), (c - b).perp_dot(p - b), (a - c).perp_dot(p - c));
            !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
        };
        assert!(shape.tris.iter().any(|t| t.kind == WaterKind::Fleuve && inside(t, end.b)));
        assert_eq!(inflow[0].inflow.unwrap()[0], 0.0);
        assert_eq!(end.inflow.unwrap()[1], 1.0);
    }

    #[test]
    fn teinte_interpolee_dans_un_triangle() {
        let t = WaterTri { pts: [Vec2::ZERO, Vec2::X, Vec2::Y], tones: [Tone { size: 0.0, murk: 0.0, rapids: 0.0 }, Tone { size: 1.0, murk: 0.0, rapids: 0.0 }, Tone { size: 0.0, murk: 1.0, rapids: 0.0 }], kind: WaterKind::River, also: None, channel: 0 };
        let m = t.tone_at(Vec2::new(0.5, 0.25));
        assert!((m.size - 0.5).abs() < 1e-5 && (m.murk - 0.25).abs() < 1e-5);
        // Une eau limoneuse tire vers le brun, un grand fleuve est plus sombre
        let blue = Vec3::new(0.05, 0.2, 0.5);
        assert!(Tone { size: 0.0, murk: 1.0, rapids: 0.0 }.apply(blue, Vec2::ZERO).x > blue.x);
        assert!(Tone { size: 1.0, murk: 0.0, rapids: 0.0 }.apply(blue, Vec2::ZERO).z < blue.z);
    }
}

#[cfg(test)]
mod diag {
    /// Ouvertures et voisines d'une case de fleuve (`WATER_SEED`, `WATER_AT=col,lig`).
    #[test]
    #[ignore]
    fn ouvertures_d_une_case() {
        use super::*;
        let seed: u32 = std::env::var("WATER_SEED").map_or(777, |s| s.parse().unwrap());
        let at: Vec<i32> = std::env::var("WATER_AT").unwrap_or("106,32".into()).split(',').map(|x| x.parse().unwrap()).collect();
        let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        for (c, r) in [(at[0], at[1]), (at[0], at[1] + 1), (at[0] + 1, at[1]), (at[0] - 1, at[1])] {
            let h = Hex::from_offset(c, r);
            let o = fleuve_openings(&m, h);
            println!("{:?} fleuve {} liens {:06b} ouvert {:06b} plein {:06b} aval {:?} q {:.0}", (c, r), m.is_fleuve(h), m.fleuve_links(h), o.open, o.full, m.downstream(h), m.discharge(h));
            for (d, &step) in DIRECTIONS.iter().enumerate() {
                let n = h + step;
                println!("   {d} → {:?} {:?} fleuve {} riviere {:.0}", m.canonical(n).to_offset(), m.get(n), m.is_fleuve(n), m.river_flow(h, d));
            }
        }
    }

    /// Cases des cours d'eau et lacs nommés (`WATER_SEED`, `WATER_NAMES=Usinza,Tren`) : premières
    /// et dernières cases de leur cours.
    #[test]
    #[ignore]
    fn ou_sont_les_noms() {
        let seed: u32 = std::env::var("WATER_SEED").map_or(777, |s| s.parse().unwrap());
        let names = std::env::var("WATER_NAMES").unwrap_or_default();
        let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        for name in names.split(',') {
            for r in m.basins().rivers.iter().filter(|r| r.name.contains(name)) {
                let cells: Vec<(i32, i32)> = r.course.iter().map(|p| m.hex_at(p.cell).to_offset()).collect();
                println!("cours {} fleuve {} : {:?} … {:?}", r.name, r.fleuve, &cells[..cells.len().min(3)], &cells[cells.len().saturating_sub(3)..]);
            }
            for l in m.basins().lakes.iter().filter(|l| l.name.contains(name)) {
                println!("lac {} : {:?}", l.name, l.cells.iter().take(4).map(|&c| m.hex_at(c).to_offset()).collect::<Vec<_>>());
            }
        }
    }

    /// Remonte le cours principal (amont le plus fort) depuis `WATER_FROM=col,lig` (graine 777).
    #[test]
    #[ignore]
    fn amont_et_limon() {
        use super::*;
        let m = crate::map::WorldMap::generate_in(192, 111, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let from: Vec<i32> = std::env::var("WATER_FROM").unwrap_or("121,65".into()).split(',').map(|x| x.parse().unwrap()).collect();
        let mut h = Hex::from_offset(from[0], from[1]);
        for _ in 0..60 {
            println!("{:?} {:?} {:.0} m q {:.0} limon {:.2} fleuve {}", h.to_offset(), m.get(h), m.elevation(h).unwrap_or(0.0), m.discharge(h), m.water_murk(h), m.is_fleuve(h));
            let up = (0..6).map(|d| m.canonical(h + DIRECTIONS[d])).filter(|&n| m.downstream(n).is_some_and(|d| m.canonical(n + DIRECTIONS[d]) == h)).max_by(|a, b| m.discharge(*a).total_cmp(&m.discharge(*b)));
            let Some(n) = up else { break };
            h = n;
        }
    }

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


