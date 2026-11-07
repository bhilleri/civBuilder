//! Formes des cours d'eau à dessiner (géométrie 2D pure, partagée par les vues 2D et 3D).
//!
//! Pour chaque case : les rivières de ses arêtes (une arête est dessinée par une seule des deux
//! cases, celle pour qui elle est dans les directions 0 à 2), le fleuve qui l'occupe (une bande
//! d'eau courbe d'une arête à l'autre en passant près du centre, berges de part et d'autre) et son
//! petit lac. La largeur suit le débit ; les fleuves serpentent (point de passage de chaque arête
//! et du milieu de la case tirés de la graine, d'autant plus en plaine) et les rivières courbent
//! de coin en coin en se raccordant à leurs tronçons d'amont et d'aval. En plaine, un fleuve se
//! partage par endroits en deux bras autour d'une île. Chaque sommet porte une teinte (débit, limon). Les triangles
//! sont découpés finement pour épouser le relief en 3D.

use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::hydro::{FLEUVE_M3S, RIVER_M3S};
use crate::map::{Terrain, WorldMap};
use crate::mouths::MouthKind;
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
/// Demi-largeur d'une rivière à sa source, et au moins cette part de sa largeur plus bas (une rivière
/// déjà abondante ne part pas d'un point : un coin triangulaire).
const RIVER_SOURCE_HALF_WIDTH: f32 = 0.8;
const RIVER_SOURCE_SHARE: f32 = 0.55;
/// Demi-largeur d'un fleuve à son embouchure : au plus toute l'arête, et évasement d'un fleuve
/// étroit (part de sa demi-largeur).
const ESTUARY_HALF_WIDTH: f32 = HEX_SIZE * 0.5;
const MOUTH_FLARE: f32 = 1.5;
/// Estuaire, en entonnoir : demi-largeur à la côte (toute l'arête : au-delà, le relief des terres
/// voisines affleure dans l'eau), et élargissement au milieu de la case et sur l'arête d'amont.
const ESTUARY_END_HALF_WIDTH: f32 = HEX_SIZE * 0.5;
const ESTUARY_MID_WIDEN: f32 = 1.45;
const ESTUARY_UP_WIDEN: f32 = 1.25;
/// Delta digité : débits à partir desquels il a 3 ou 4 bras, part du fleuve gardée par chaque bras
/// (divisée par la racine du nombre de bras), demi-largeur la plus petite d'un bras, écart le plus
/// petit entre deux bords de bras (terre entre eux) et décalage aléatoire de chaque bras.
const DELTA_ARMS_M3S: [f32; 2] = [3_500.0, 10_000.0];
const DELTA_ARM_SHARE: f32 = 0.55;
const DELTA_ARM_MIN_HALF_WIDTH: f32 = 2.4;
const DELTA_LAND_GAP: f32 = 7.0;
/// Terre gardée entre le bras extrême d'un delta et le coin de l'arête (moins : un éclat de sable).
const DELTA_EDGE_MARGIN: f32 = 4.0;
const DELTA_ARM_JITTER: f32 = 1.2;
/// Delta digité : le fleuve se divise à cette part du chemin de l'arête d'amont au milieu de la case.
const DELTA_FORK: f32 = 0.35;
/// Ria et fjord : la mer remonte la vallée sur leur case et celle d'amont (longueur du fondu, en
/// unités ; une case en fait 55), et leur eau est plus étroite qu'un fleuve ordinaire.
const INLET_BLEND: f32 = 110.0;
const RIA_WIDTH: f32 = 0.8;
const FJORD_WIDTH: f32 = 0.7;
/// Rivière sur les coins : part du chemin vers la corde de ses voisines (le coin s'en rapproche, ce
/// qui aplatit le zigzag d'une rivière qui coule en travers de la grille), au moins et en plus selon
/// la graine, et décalage de côté, pour que deux rivières voisines n'ondulent pas pareil.
const NODE_PULL: (f32, f32) = (0.25, 0.5);
const NODE_JITTER: f32 = 3.5;
/// Rivage en 3D : déplacement le plus grand d'un coin et bombement le plus grand d'une arête.
const COAST_CORNER: f32 = 4.0;
const COAST_RIPPLE: f32 = 5.0;
/// Arrondi des angles du rivage : part du chemin d'un coin vers le milieu de ses deux voisins.
const COAST_ROUND: f32 = 0.35;
/// Demi-largeur d'un fleuve à sa source (sur l'arête de la voisine qui l'alimente).
const SOURCE_HALF_WIDTH: f32 = 1.4;
/// Intérieur d'un virage serré avant l'embouchure : l'eau va jusqu'à cette part du chemin vers les coins.
const BEND_FILL: f32 = 0.8;
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
/// en ont ; intensité minimale d'un rapide ; eau vive (sRGB) et part au plus ; assombrissement de
/// l'eau lisse au plus ; écume et rochers (sRGB), part de l'écume mêlée à l'eau.
const RAPIDS_FROM_M_PER_KM: f32 = 3.0;
const RAPIDS_FULL_M_PER_KM: f32 = 9.0;
const STRETCH: f32 = 22.0;
const RAPID_MIN: f32 = 0.25;
/// Taille des mailles des éléments de rapides (unités monde).
const MARK_STEP: f32 = 1.5;
const WHITEWATER_SRGB: [f32; 3] = [0.66, 0.84, 0.90];
const WHITEWATER_MAX: f32 = 0.35;
const SHADE_MAX: f32 = 0.2;
const FOAM_SRGB: [f32; 3] = [0.88, 0.94, 0.97];
const FOAM_MIX: f32 = 0.6;
const ROCK_SRGB: [f32; 3] = [0.45, 0.42, 0.39];
/// Affluent dans un fleuve : il le rejoint vers l'aval, le long de sa rive (part de la
/// demi-largeur du fleuve), puis sa couleur se fond dans celle du fleuve sur `PLUME` unités.
const INFLOW_BANK: f32 = 0.45;
/// Largeur du panache d'un affluent au plus (part de la demi-largeur du fleuve).
const INFLOW_FILL: f32 = 0.9;
const PLUME: f32 = 12.0;
/// Îles d'un fleuve en plaine : pente (m/km) au-delà de laquelle il n'y en a plus ; chance
/// d'une île (fleuve plat) au seuil d'un fleuve et pour les plus grands ; part de cette chance
/// pour une île à cheval sur une arête et pour un bras secondaire ; demi-largeur la plus petite
/// d'une île (au-dessous, elle ne sortirait pas de l'eau en 3D) ; découpage d'une traversée
/// avec île (segments par demi-case).
const ISLE_FLAT_M_PER_KM: f32 = 1.0;
const ISLE_CHANCE_MIN: f32 = 0.15;
const ISLE_CHANCE_MAX: f32 = 0.6;
const ISLE_EDGE_SHARE: f32 = 0.6;
const ISLE_SIDE_SHARE: f32 = 0.5;
const ISLE_MIN_HALF_WIDTH: f32 = 2.6;
const ISLE_STEPS: usize = 16;
/// Îles à cheval sur une arête : le fleuve s'élargit de la largeur des îles plus cette part de
/// sa demi-largeur, sur au moins `ISLE_SPREAD` unités de part et d'autre de l'arête.
const ISLE_ARM_EXTRA: f32 = 0.1;
const ISLE_SPREAD: f32 = HEX_SIZE * 0.85;
/// Au moins cette part de la demi-largeur d'un fleuve en eau, de part et d'autre, autour de ses
/// îles (tous bras confondus), et dans chaque bras entre deux îles.
const ISLE_WATER_MIN: f32 = 1.1;
const ISLE_ARM_MIN: f32 = 0.4;
/// Arbres d'une île : part de terre qu'il lui faut, distance au bord où ils poussent, écart le
/// long de l'île.
const ISLE_TREE_SOIL: f32 = 0.5;
const ISLE_TREE_ROOM: f32 = 2.0;
const ISLE_TREE_STEP: f32 = 4.5;
/// Dessus d'une île : hauteur sur l'eau au moins (banc de sable), en plus pour une île de terre.
const ISLE_LIFT_MIN: f32 = 0.3;
const ISLE_LIFT_SOIL: f32 = 1.0;
/// Bras secondaire : le fleuve se resserre d'autant (part de sa demi-largeur, au milieu du bras).
const ISLE_SIDE_NARROW: f32 = 0.3;
/// Variation de la largeur d'un fleuve le long de son cours (part, au plus, en plaine).
const BREADTH: f32 = 0.15;

/// Nature d'un morceau d'eau (couleurs distinctes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterKind {
    River,
    Fleuve,
    Lake,
    /// Rapides, posés sur l'eau : écume (tête, vagues, coussins des rochers, traînées) ;
    /// rocher qui affleure ; eau vive plus claire et eau lisse plus sombre (langue, remous derrière
    /// un rocher), dosées par sommet (`Tone::rapids`).
    Foam,
    Rock,
    Light,
    Shade,
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
            WaterKind::River | WaterKind::Foam | WaterKind::Rock | WaterKind::Light | WaterKind::Shade => Bed { near: 3.0, far: 6.0, below: 1.1, bed: 1.4 },
            WaterKind::Fleuve => Bed { near: 7.0, far: 15.0, below: 1.5, bed: 2.2 },
            WaterKind::Lake => Bed { near: 4.5, far: 9.0, below: 0.9, bed: 1.2 },
        }
    }
}

/// Axe d'un tronçon d'eau (coordonnées monde 2D) : segment `a`–`b` (un disque si a = b) de
/// demi-largeur `hw`. `inflow` : affluent qui entre dans un fleuve, avec la part du chemin vers le
/// fleuve faite au début et à la fin du tronçon (sa hauteur rejoint progressivement celle du fleuve).
/// Dessus d'une île : hauteur au-dessus de l'eau (au plus haut) et part de terre (0 : banc de
/// sable ou de limon nu, 1 : terre où poussent des arbres).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IsleTop {
    pub lift: f32,
    pub soil: f32,
}

/// Île d'un fleuve : ligne de milieu (points et demi-largeurs) et dessus.
#[derive(Clone, Debug, PartialEq)]
pub struct Isle {
    pub spine: Vec<(Vec2, f32)>,
    pub top: IsleTop,
}

impl Isle {
    /// Profondeur du point `p` dans l'île (distance à son bord, négative au-dehors).
    pub fn depth(&self, p: Vec2) -> f32 {
        self.spine.windows(2).map(|w| segment_depth(w[0], w[1], p)).fold(f32::MIN, f32::max)
    }
}

/// Arbres des îles de terre d'une case (point du monde, taille de 0,5 à 1 environ) : le long de
/// leur ligne de milieu, assez loin du bord (deux rangs sur une île large), un peu dispersés ;
/// seulement dans la case (une île à cheval sur une arête : chaque case plante sa moitié).
pub fn isle_trees(shape: &WaterShape, hex: Hex) -> Vec<(Vec2, f32)> {
    let mut trees = Vec::new();
    for isle in shape.isles.iter().filter(|i| i.top.soil >= ISLE_TREE_SOIL) {
        let mut walked = 0.0f32;
        for w in isle.spine.windows(2) {
            let (a, b) = (w[0].0, w[1].0);
            let length = a.distance(b);
            let across = (b - a).perp().normalize_or_zero();
            while walked <= length {
                let p = a.lerp(b, if length > 0.0 { walked / length } else { 0.0 });
                walked += ISLE_TREE_STEP;
                let room = isle.depth(p);
                if room < ISLE_TREE_ROOM {
                    continue;
                }
                let rows: &[f32] = if room > 2.0 * ISLE_TREE_ROOM { &[-0.45, 0.45] } else { &[0.0] };
                for &row in rows {
                    let jitter = hash01(p + Vec2::splat(row)) - 0.5;
                    let at = p + across * (row + 0.3 * jitter) * room;
                    if isle.depth(at) >= ISLE_TREE_ROOM * 0.8 && Hex::from_world(at.x, at.y) == hex {
                        trees.push((at, ((0.45 + 0.08 * room) * (0.85 + 0.3 * hash01(at))).min(1.0)));
                    }
                }
            }
            walked -= length;
        }
    }
    trees
}

/// Profondeur de `p` dans un tronçon d'île ou de lit (de `a` à `b`, demi-largeurs données).
fn segment_depth((a, ha): (Vec2, f32), (b, hb): (Vec2, f32), p: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-6 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    ha + (hb - ha) * t - p.distance(a + ab * t)
}

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
    /// Lit d'île : le sol y sort de l'eau (pas d'eau dessus).
    pub isle: Option<IsleTop>,
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

    /// Couleur (linéaire) de l'eau de couleur de base `base` (linéaire) avec cette teinte : tirée
    /// vers le limon, assombrie par le débit.
    pub fn apply(self, base: Vec3) -> Vec3 {
        let silt = Vec3::from(SILT_SRGB.map(srgb_to_linear));
        base.lerp(silt, self.murk.clamp(0.0, 1.0) * SILT_MAX) * (1.0 - DEEP_DARKEN * self.size.clamp(0.0, 1.0))
    }
}

impl WaterKind {
    /// Couleur (linéaire) d'un élément de rapides posé sur une eau de couleur `water` (linéaire),
    /// avec la teinte `tone` de son sommet ; `None` pour l'eau elle-même.
    pub fn rapids_color(self, tone: Tone, water: Vec3) -> Option<Vec3> {
        let amount = tone.rapids.clamp(0.0, 1.0);
        match self {
            // (comme une écume en partie transparente : mêlée à l'eau en dessous)
            WaterKind::Foam => Some(water.lerp(Vec3::from(FOAM_SRGB.map(srgb_to_linear)), FOAM_MIX)),
            WaterKind::Rock => Some(Vec3::from(ROCK_SRGB.map(srgb_to_linear))),
            WaterKind::Light => Some(water.lerp(Vec3::from(WHITEWATER_SRGB.map(srgb_to_linear)), amount * WHITEWATER_MAX)),
            WaterKind::Shade => Some(water * (1.0 - amount * SHADE_MAX)),
            _ => None,
        }
    }
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
    pub isles: Vec<Isle>,
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
        self.channels.push(WaterAxis { a, b, hw, hw_end, kind, also: self.also, inflow, isle: None });
    }

    /// Range une île (ligne de milieu, dessus), avec un lit d'île par tronçon.
    fn isle(&mut self, spine: Vec<(Vec2, f32)>, top: IsleTop) {
        for w in spine.windows(2) {
            self.channels.push(WaterAxis { a: w[0].0, b: w[1].0, hw: w[0].1, hw_end: w[1].1, kind: WaterKind::Fleuve, also: None, inflow: None, isle: Some(top) });
        }
        self.isles.push(Isle { spine, top });
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
            let ((a, ha, _), (b, hb, _)) = (path[k], path[k + 1]);
            self.channel(a, b, ha, hb, kind, [k as f32 / last as f32, (k + 1) as f32 / last as f32]);
            for j in 0..across {
                self.tri([at(k, j), at(k + 1, j), at(k + 1, j + 1)], [tone(k, j), tone(k + 1, j), tone(k + 1, j + 1)], kind);
                self.tri([at(k, j), at(k + 1, j + 1), at(k, j + 1)], [tone(k, j), tone(k + 1, j + 1), tone(k, j + 1)], kind);
            }
        }
    }

    /// Bande le long des points `range` (bornes comprises) d'une ligne d'eau, l'eau allant en
    /// travers de `span(k).0` à `span(k).1` (décalages le long de `normal[k]`) : bras d'un fleuve
    /// autour d'une île.
    fn offset_strip(&mut self, path: &[PathPoint], normal: &[Vec2], range: std::ops::RangeInclusive<usize>, span: impl Fn(usize) -> (f32, f32), kind: WaterKind) {
        let (first, last) = (*range.start(), *range.end());
        let across = if range.clone().any(|k| span(k).1 - span(k).0 > 12.0) { 4 } else { 1 };
        let at = |k: usize, j: usize| {
            let (lo, hi) = span(k);
            path[k].0 + normal[k] * (lo + (hi - lo) * j as f32 / across as f32)
        };
        for k in first..last {
            let (m0, m1) = ((span(k).0 + span(k).1) / 2.0, (span(k + 1).0 + span(k + 1).1) / 2.0);
            let (w0, w1) = ((span(k).1 - span(k).0) / 2.0, (span(k + 1).1 - span(k + 1).0) / 2.0);
            self.channel(path[k].0 + normal[k] * m0, path[k + 1].0 + normal[k + 1] * m1, w0, w1, kind, [0.0, 0.0]);
            let (ta, tb) = (path[k].2, path[k + 1].2);
            for j in 0..across {
                self.tri([at(k, j), at(k + 1, j), at(k + 1, j + 1)], [ta, tb, tb], kind);
                self.tri([at(k, j), at(k + 1, j + 1), at(k, j + 1)], [ta, tb, ta], kind);
            }
        }
    }

    /// Bande d'un fleuve le long d'une ligne d'eau (en travers : `fleuve_normals`).
    fn fleuve_strip(&mut self, path: &[PathPoint]) {
        let normal = fleuve_normals(path);
        self.offset_strip(path, &normal, 0..=path.len() - 1, |k| (-path[k].1, path[k].1), WaterKind::Fleuve);
    }

    /// Élément de rapides (polygone convexe, en éventail depuis son centre), teinte `tone`
    /// (`rapids` : dosage de l'eau vive ou de l'eau lisse).
    fn mark(&mut self, pts: &[Vec2], kind: WaterKind, tone: Tone) {
        let center = pts.iter().copied().sum::<Vec2>() / pts.len() as f32;
        let channel = self.channels.len() - 1;
        for k in 0..pts.len() {
            self.tris.push(WaterTri { pts: [center, pts[k], pts[(k + 1) % pts.len()]], tones: [tone; 3], kind, also: self.also, channel });
        }
    }

    /// Grille de `rows` × `cols` mailles (sommets `point(i, j)`, teintes `tone(i, j)`) : assez
    /// fine, un élément de rapides suit la pente de l'eau au lieu de la couper.
    fn grid(&mut self, rows: usize, cols: usize, point: impl Fn(usize, usize) -> Vec2, tone: impl Fn(usize, usize) -> Tone, kind: WaterKind) {
        let channel = self.channels.len() - 1;
        for i in 0..rows {
            for j in 0..cols {
                let (a, b, c, d) = ((i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1));
                let p = |(x, y): (usize, usize)| point(x, y);
                let t = |(x, y): (usize, usize)| tone(x, y);
                self.tris.push(WaterTri { pts: [p(a), p(b), p(c)], tones: [t(a), t(b), t(c)], kind, also: self.also, channel });
                self.tris.push(WaterTri { pts: [p(a), p(c), p(d)], tones: [t(a), t(c), t(d)], kind, also: self.also, channel });
            }
        }
    }

    /// Trait d'écume de `a` à `b`, d'épaisseur `thick`, effilé aux deux bouts (découpé finement).
    fn streak(&mut self, a: Vec2, b: Vec2, thick: f32) {
        let n = (b - a).perp().normalize_or_zero() * thick / 2.0;
        let rows = ((a.distance(b) / MARK_STEP).ceil() as usize).max(2);
        let point = |i: usize, j: usize| {
            let t = i as f32 / rows as f32;
            a.lerp(b, t) + n * (std::f32::consts::PI * t).sin().max(0.15) * (2.0 * j as f32 - 1.0)
        };
        self.grid(rows, 1, point, |_, _| Tone::default(), WaterKind::Foam);
    }

    /// Rapide composé sur une ligne d'eau, là où ses rapides sont les plus forts (`downstream` :
    /// la ligne est donnée d'amont en aval) : comme vu d'en haut sur de vrais rapides,
    /// - une eau vive plus claire sur tout le rapide, qui se fond dans l'eau autour ;
    /// - la tête : une amorce d'écume le long des rives ;
    /// - le train de vagues : des bandes d'écume en travers, espacées, qui s'affaiblissent ;
    /// - des rochers qui affleurent : coussin d'écume en amont, ailes en V vers l'aval, eau calme
    ///   et sombre derrière ;
    /// - en aval, l'écume se dissipe en traînées dans le sens du courant.
    ///
    /// Tout est tiré de la position du rapide : la même forme à chaque affichage.
    fn rapid(&mut self, path: &[PathPoint], downstream: bool) {
        let reach = Reach::new(path, downstream);
        let len = reach.length();
        // Intensité le long de la ligne : pente, et seulement par tronçons
        let (best, w) = reach.path.iter().enumerate().map(|(k, p)| (k, p.2.rapids.clamp(0.0, 1.0) * stretch(p.0))).fold((0, 0.0f32), |acc, x| if x.1 > acc.1 { x } else { acc });
        if w < RAPID_MIN || len < 6.0 {
            return;
        }
        let hw = reach.path[best].1;
        let site = (6.0 + 5.0 * hw).clamp(10.0, len * 0.9);
        let s0 = (reach.at[best] - 0.4 * site).clamp(0.0, len - site);
        let seed = reach.path[best].0;
        let at = |f: f32| s0 + f * site;
        let thick = (0.08 * hw).clamp(0.2, 0.8);
        let base = reach.sample(at(0.5)).3;
        let dosed = |amount: f32| Tone { rapids: amount, ..base };
        // Eau vive : sur toute la largeur, dosée par sommet (pas de bord net)
        let rows = ((site * 1.15 / MARK_STEP).ceil() as usize).max(8);
        // (l'eau vive s'éteint avant les bouts de la ligne d'eau : pas de marche sur l'arête)
        let ramp = (site * 0.12).max(1e-3);
        let end_fade = |s: f32| (s.min(len - s) / ramp).clamp(0.0, 1.0);
        let bump = |f: f32| (f / 0.15).min(1.0).min((1.15 - f) / 0.35).clamp(0.0, 1.0) * w;
        let f_of = |i: usize| i as f32 / rows as f32 * 1.15;
        self.grid(rows, 4, |i, j| reach.point(at(f_of(i) - 0.05), 0.97 * (j as f32 / 2.0 - 1.0)), |i, _| dosed(bump(f_of(i)) * end_fade(at(f_of(i) - 0.05))), WaterKind::Light);
        // Tête, discrète : un peu d'écume le long des rives là où l'eau s'engage (une amorce, pas
        // de V qui traverse la rivière, ni de langue sombre : trop visibles)
        for side in [1.0, -1.0] {
            self.streak(reach.point(at(0.02), side * 0.9), reach.point(at(0.14), side * 0.65), thick * 0.8);
        }
        // Train de vagues : bandes en travers, de plus en plus courtes et fines
        let waves = 2 + (2.0 * w).round() as usize;
        for i in 0..waves {
            let f = 0.38 + 0.12 * i as f32;
            if f > 0.9 {
                break;
            }
            let fade = 1.0 - 0.2 * i as f32;
            let reach_u = (0.8 - 0.1 * i as f32) * (0.85 + 0.3 * hash01(seed + Vec2::splat(i as f32)));
            let depth = (thick * 0.9 * fade).max(0.18);
            // Crête un peu bombée vers l'amont, effilée aux deux bouts
            let crest: Vec<(f32, f32)> = (0..=6).map(|j| {
                let u = -reach_u + 2.0 * reach_u * j as f32 / 6.0;
                (u, 1.0 - (u / reach_u).powi(2))
            }).collect();
            let front: Vec<Vec2> = crest.iter().map(|&(u, b)| reach.point(at(f) - 0.04 * site * b + depth * b.sqrt() / 2.0, u)).collect();
            let back: Vec<Vec2> = crest.iter().rev().map(|&(u, b)| reach.point(at(f) - 0.04 * site * b - depth * b.sqrt() / 2.0, u)).collect();
            let ring: Vec<Vec2> = front.into_iter().chain(back).collect();
            // (par quadrilatères : la crête est courbe)
            for j in 0..6 {
                let quad = [ring[j], ring[j + 1], ring[ring.len() - 2 - j], ring[ring.len() - 1 - j]];
                self.mark(&quad, WaterKind::Foam, Tone::default());
            }
        }
        // Rochers : coussin d'écume en amont, ailes en V vers l'aval, eau calme derrière
        let rocks = if hw < 1.5 { 0 } else if w > 0.6 && hw > 4.0 { 2 } else { 1 };
        for r in 0..rocks {
            let h = hash01(seed + Vec2::new(3.0, r as f32));
            let f = 0.5 + 0.3 * h;
            let u = (0.25 + 0.35 * hash01(seed + Vec2::new(r as f32, 5.0))) * if r % 2 == 0 { 1.0 } else { -1.0 };
            let (_, dir, hw_r, _) = reach.sample(at(f));
            let c = reach.point(at(f), u);
            let radius = (0.2 * hw_r).clamp(0.45, 2.0);
            let across = dir.perp();
            // Eau calme et sombre derrière le rocher
            let eddy: Vec<Vec2> = (0..8).map(|k| {
                let a = std::f32::consts::TAU * k as f32 / 8.0;
                c + dir * (radius * 1.8 + a.cos() * radius * 1.8) + across * a.sin() * radius
            }).collect();
            self.mark(&eddy, WaterKind::Shade, dosed(0.7 * w));
            // Ailes d'écume qui s'écartent vers l'aval
            for side in [1.0, -1.0] {
                let from = c + across * side * radius * 0.8;
                self.streak(from, from + dir * radius * 3.5 + across * side * radius * 1.4, thick);
            }
            // Coussin d'écume sur la face amont
            let pillow: Vec<Vec2> = (0..=6).map(|k| {
                let a = std::f32::consts::PI * (0.5 + k as f32 / 6.0);
                c + (dir * a.cos() + across * a.sin()) * radius * 1.45
            }).chain((0..=6).rev().map(|k| {
                let a = std::f32::consts::PI * (0.5 + k as f32 / 6.0);
                c + (dir * a.cos() + across * a.sin()) * radius * 0.9
            })).collect();
            for k in 0..6 {
                self.mark(&[pillow[k], pillow[k + 1], pillow[12 - k], pillow[13 - k]], WaterKind::Foam, Tone::default());
            }
            // Le rocher, irrégulier
            let stone: Vec<Vec2> = (0..7).map(|k| {
                let a = std::f32::consts::TAU * k as f32 / 7.0;
                c + (dir * a.cos() * 1.2 + across * a.sin()) * radius * (0.75 + 0.35 * hash01(c + Vec2::splat(k as f32)))
            }).collect();
            self.mark(&stone, WaterKind::Rock, Tone::default());
        }
        // En aval, l'écume se dissipe en traînées dans le sens du courant
        for k in 0..2 {
            let f = 0.88 + 0.08 * k as f32;
            let u = 0.6 * (2.0 * hash01(seed + Vec2::new(k as f32, 9.0)) - 1.0);
            self.streak(reach.point(at(f), u), reach.point(at(f) + 0.2 * site, u * 0.8), (thick * 0.6).max(0.2));
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

/// Ligne d'eau parcourue d'amont en aval, repérée par la distance `s` depuis son début (et le
/// côté `u`, de −1 à 1 d'une rive à l'autre).
struct Reach {
    path: Vec<PathPoint>,
    at: Vec<f32>,
}

impl Reach {
    fn new(path: &[PathPoint], downstream: bool) -> Self {
        let mut path = path.to_vec();
        if !downstream {
            path.reverse();
        }
        let mut at = vec![0.0];
        for w in path.windows(2) {
            at.push(at.last().unwrap() + w[0].0.distance(w[1].0));
        }
        Reach { path, at }
    }

    fn length(&self) -> f32 {
        *self.at.last().unwrap_or(&0.0)
    }

    /// Point à la distance `s` : position, sens du courant, demi-largeur, teinte.
    fn sample(&self, s: f32) -> (Vec2, Vec2, f32, Tone) {
        let s = s.clamp(0.0, self.length());
        let k = self.at.iter().rposition(|&x| x <= s).unwrap_or(0).min(self.path.len().saturating_sub(2));
        let (a, b) = (self.path[k], self.path[(k + 1).min(self.path.len() - 1)]);
        let span = (self.at[(k + 1).min(self.at.len() - 1)] - self.at[k]).max(1e-6);
        let t = ((s - self.at[k]) / span).clamp(0.0, 1.0);
        (a.0.lerp(b.0, t), (b.0 - a.0).normalize_or(Vec2::X), a.1 + (b.1 - a.1) * t, a.2.lerp(b.2, t))
    }

    /// Point à la distance `s`, au côté `u` (−1 à 1 d'une rive à l'autre).
    fn point(&self, s: f32, u: f32) -> Vec2 {
        let (p, dir, hw, _) = self.sample(s);
        p + dir.perp() * u * hw
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
///
/// (L'intérieur d'un virage serré juste avant l'embouchure est comblé sans ouvrir l'arête de terre :
/// voir `bend_edges`.)
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
    // (un delta digité sort seulement par son arête aval, en plusieurs bras : ouvrir aussi une arête
    // de mer voisine l'empêcherait de se diviser à l'entrée de sa case et laisserait un croissant de
    // terre coupé par la mer)
    // (un delta arqué a une seule sortie, une ria et un fjord restent une vallée étroite)
    let delta = map.mouth(hex).is_some_and(|m| matches!(m.kind, MouthKind::BirdFoot | MouthKind::Arcuate | MouthKind::Ria | MouthKind::Fjord));
    for d in 0..6 {
        if delta || links & (1 << d) == 0 || !water_side(d) {
            continue;
        }
        full |= 1 << d;
        for e in [(d + 1) % 6, (d + 5) % 6] {
            if water_side(e) {
                open |= 1 << e;
                full |= 1 << e;
            }
        }
    }
    Openings { open, full }
}

/// Arêtes de terre à l'intérieur d'un virage serré juste avant l'embouchure : l'eau d'un fleuve large
/// occupe ce côté de la case (un fleuve étroit garde la pointe de terre de son dernier virage).
fn bend_edges(map: &WorldMap, hex: Hex) -> Vec<usize> {
    let links = map.fleuve_links(hex);
    if fleuve_half_width(map.discharge(hex)) * MOUTH_FLARE < ESTUARY_HALF_WIDTH {
        return Vec::new();
    }
    let water = |d: usize| map.get(hex + DIRECTIONS[d]).is_some_and(|t| t.is_water());
    let mut edges = Vec::new();
    for d in (0..6).filter(|&d| links & (1 << d) != 0 && water(d)) {
        for (e, beyond) in [((d + 1) % 6, (d + 2) % 6), ((d + 5) % 6, (d + 4) % 6)] {
            if !water(e) && links & (1 << beyond) != 0 && links & (1 << e) == 0 && !map.is_fleuve(hex + DIRECTIONS[e]) {
                edges.push(e);
            }
        }
    }
    edges
}

/// Valeur (0 à 1) attachée à un coin de la carte : la même vue de ses trois cases, et de part et
/// d'autre du raccord est-ouest.
fn corner_hash(map: &WorldMap, p: Vec2, salt: u32) -> f32 {
    let span = map.width as f32 * HEX_SIZE * 3f32.sqrt();
    let x = if map.wrap().wraps() { p.x.rem_euclid(span) } else { p.x };
    let mut k = (x.round() as i32 as u32).wrapping_mul(0x9E37_79B1) ^ (p.y.round() as i32 as u32).wrapping_mul(0x85EB_CA77) ^ salt.wrapping_mul(0xC2B2_AE3D) ^ map.seed().wrapping_mul(0x27D4_EB2F);
    k ^= k >> 15;
    k = k.wrapping_mul(0x2C1B_3C6D);
    k ^= k >> 12;
    k = k.wrapping_mul(0x297A_2D39);
    k ^= k >> 15;
    k as f32 / u32::MAX as f32
}

/// Point où passent les rivières à un coin de cases `p` : déplacé vers la corde de ses voisines, de
/// plus ou moins selon la graine, pour casser la vague régulière d'une rivière qui suit les arêtes
/// en travers de la grille. Sur un rivage, il suit le rivage (`coast_corner`) ; près d'un fleuve et là
/// où une rivière commence ou finit, il reste sur le coin exact : les raccords s'y font.
pub fn river_node(map: &WorldMap, p: Vec2) -> Vec2 {
    // Une des trois cases autour du coin, toujours la même quel que soit le côté d'où l'on arrive
    let hex = Hex::from_world(p.x + HEX_SIZE * 0.4, p.y);
    let Some(j) = (0..6).find(|&j| corner(hex, j).distance(p) < 0.5) else { return p };
    let cells = [hex, hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]];
    if cells.iter().any(|&h| map.get(h).is_some_and(|t| t.is_water())) {
        return p + coast_corner(map, hex, j);
    }
    if cells.iter().any(|&h| map.is_fleuve(h) || map.get(h).is_none()) {
        return p;
    }
    let close = |a: Vec2, b: Vec2| a.distance(b) < 0.5;
    let rivers: Vec<EdgeRiver> = rivers_at_corner(map, hex, j, 6).collect();
    let from = rivers.iter().filter(|r| close(r.down, p)).max_by(|x, y| x.q.total_cmp(&y.q));
    let to = rivers.iter().filter(|r| close(r.up, p)).max_by(|x, y| x.q.total_cmp(&y.q));
    let (Some(from), Some(to)) = (from, to) else { return p };
    let chord = (from.up + to.down) / 2.0 - p;
    let along = (to.down - from.up).normalize_or_zero();
    let pull = NODE_PULL.0 + NODE_PULL.1 * corner_hash(map, p, 90);
    p + chord * pull + along.perp() * NODE_JITTER * (2.0 * corner_hash(map, p, 91) - 1.0)
}

/// Nature d'une case pour le rivage : terre, banquise, eau libre (`None` hors de la carte).
fn shore_kind(map: &WorldMap, h: Hex) -> Option<u8> {
    let t = map.get(h)?;
    Some(if !t.is_water() { 0 } else if map.is_ice(h) { 1 } else { 2 })
}

/// Arête `e` d'une case de fleuve par où passe son eau (vers l'eau libre, un lac ou la case suivante).
fn fleuve_open_edge(map: &WorldMap, h: Hex, e: usize) -> bool {
    map.is_fleuve(h) && fleuve_openings(map, h).open & (1 << e) != 0
}

/// Déplacement du rivage au coin `j` d'une case : l'angle arrondi (le coin se rapproche de ses deux
/// voisins le long de la côte) et un peu de hasard selon la graine. Le même vu des trois cases. Rien si
/// le coin n'est pas sur un rivage, au bord d'une falaise (ses parois suivent le coin exact), ni au bord
/// d'une arête où passe un fleuve : l'eau et les berges creusées de sa case partent de ce coin exact.
fn coast_corner(map: &WorldMap, hex: Hex, j: usize) -> Vec2 {
    let trio = [hex, hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]];
    let kinds = trio.map(|h| shore_kind(map, h));
    let (Some(k0), Some(k1), Some(k2)) = (kinds[0], kinds[1], kinds[2]) else { return Vec2::ZERO };
    if (k0 == k1 && k1 == k2) || (0..3).any(|k| map.cliff_with_sea(trio[k], trio[(k + 1) % 3])) {
        return Vec2::ZERO;
    }
    let p = corner(hex, j);
    let on_fleuve = trio.iter().any(|&h| (0..6).any(|e| fleuve_open_edge(map, h, e) && (corner(h, e).distance(p) < 0.5 || corner(h, (e + 1) % 6).distance(p) < 0.5)));
    if on_fleuve {
        return Vec2::ZERO;
    }
    let mut shift = Vec2::from_angle(corner_hash(map, p, 92) * std::f32::consts::TAU) * COAST_CORNER * corner_hash(map, p, 93);
    // La case qui fait pointe ou creux ici : celle dont la nature diffère des deux autres ; ses deux
    // arêtes qui partent de ce coin sont le rivage
    let odd = if k1 == k2 { Some(0) } else if k0 == k2 { Some(1) } else if k0 == k1 { Some(2) } else { None };
    if let Some(odd) = odd {
        let tip = trio[odd];
        let k = (0..6).find(|&k| corner(tip, k).distance(p) < 0.5).unwrap_or(0);
        shift += ((corner(tip, (k + 1) % 6) + corner(tip, (k + 5) % 6)) / 2.0 - p) * COAST_ROUND;
    }
    shift
}

/// Forme du rivage autour d'une case, en 3D : déplacement de ses six coins et bombement de ses six
/// arêtes (vers l'extérieur de la case), selon la graine. Commun aux cases qui se touchent : aucune
/// fente. Le rivage ondule et ses angles s'arrondissent, les cases se lisent toujours.
pub fn coast_warp(map: &WorldMap, hex: Hex) -> ([Vec2; 6], [f32; 6]) {
    let (mut corners, mut ripple) = ([Vec2::ZERO; 6], [0.0; 6]);
    let Some(own) = shore_kind(map, hex) else { return (corners, ripple) };
    // (entourée de cases de même nature : aucun de ses coins n'est sur un rivage)
    if hex.neighbors().iter().all(|&n| shore_kind(map, n) == Some(own)) {
        return (corners, ripple);
    }
    for (j, c) in corners.iter_mut().enumerate() {
        *c = coast_corner(map, hex, j);
    }
    for (d, r) in ripple.iter_mut().enumerate() {
        let n = hex + DIRECTIONS[d];
        let Some(other) = shore_kind(map, n) else { continue };
        if own == other || map.cliff_with_sea(hex, n) || edge_river(map, hex, d).is_some() || fleuve_open_edge(map, hex, d) || fleuve_open_edge(map, n, (d + 3) % 6) {
            continue;
        }
        // (compté vers la nature la plus « eau » : la même forme vue des deux cases)
        let bulge = COAST_RIPPLE * (2.0 * corner_hash(map, edge_mid(hex, d), 94) - 1.0);
        *r = if own > other { -bulge } else { bulge };
    }
    (corners, ripple)
}

/// Eau dans laquelle se fond un fleuve : celle sur laquelle sa case s'ouvre, ou (case en amont d'une
/// ria ou d'un fjord) celle de cette embouchure, et longueur du fondu si elle est particulière.
pub fn fleuve_fade(map: &WorldMap, hex: Hex) -> (Vec<Hex>, Option<f32>) {
    let open_water = |h: Hex| -> Vec<Hex> {
        let open = fleuve_openings(map, h).open;
        (0..6).filter(|d| open & (1 << d) != 0).map(|d| h + DIRECTIONS[d]).filter(|&n| map.get(n).is_some_and(|t| t.is_water())).collect()
    };
    let inlet = |h: Hex| map.mouth(h).is_some_and(|m| matches!(m.kind, MouthKind::Ria | MouthKind::Fjord));
    if inlet(hex) {
        return (open_water(hex), Some(INLET_BLEND));
    }
    if let Some(d) = map.downstream(hex)
        && inlet(hex + DIRECTIONS[d])
    {
        return (open_water(hex + DIRECTIONS[d]), Some(INLET_BLEND));
    }
    (open_water(hex), None)
}

/// Distance d'un point à la case de centre `c` (0 dedans ou sur son bord) : aux coins comme au
/// milieu des arêtes, contrairement à un cercle.
pub fn hex_gap(p: Vec2, c: Vec2) -> f32 {
    let d = p - c;
    let far = (0..6).map(|k| d.dot(Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_3))).fold(f32::MIN, f32::max);
    (far - HEX_SIZE * 0.866).max(0.0)
}

/// Morceaux d'un triangle du côté de la case `hex` de ses arêtes `dirs` (bits) : en 2D, l'eau d'un
/// fleuve dessinée par-dessus les tuiles s'arrête au bord de la mer où il se jette.
pub fn clip_to_edges(pts: [Vec2; 3], hex: Hex, dirs: u8) -> Vec<[Vec2; 3]> {
    let c = center(hex);
    let mut poly: Vec<Vec2> = pts.to_vec();
    for d in (0..6).filter(|d| dirs & (1 << d) != 0) {
        let n = Vec2::from_angle(d as f32 * std::f32::consts::FRAC_PI_3);
        let inside = |p: Vec2| (p - c).dot(n) - HEX_SIZE * 0.866;
        let mut kept = Vec::with_capacity(poly.len() + 1);
        for (i, &a) in poly.iter().enumerate() {
            let b = poly[(i + 1) % poly.len()];
            let (da, db) = (inside(a), inside(b));
            if da <= 0.0 {
                kept.push(a);
            }
            if (da <= 0.0) != (db <= 0.0) {
                kept.push(a.lerp(b, da / (da - db)));
            }
        }
        poly = kept;
        if poly.len() < 3 {
            return Vec::new();
        }
    }
    (1..poly.len() - 1).map(|i| [poly[0], poly[i], poly[i + 1]]).collect()
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
    if !map.is_fleuve(nb) || map.get(nb).is_some_and(|t| t.is_water()) {
        return fleuve_half_width(map.discharge(hex));
    }
    let mut hw = fleuve_half_width(map.discharge(hex).min(map.discharge(nb)));
    // (l'entonnoir d'un estuaire commence avant sa case)
    if mouth_kind(map, hex) == Some(MouthKind::Estuary) || mouth_kind(map, nb) == Some(MouthKind::Estuary) {
        hw *= ESTUARY_UP_WIDEN;
    }
    // (plus ou moins large d'une arête à l'autre, en plaine ; il ne déborde pas de l'arête)
    let (owner, od) = edge_owner(hex, d);
    let wide = hw * (1.0 + BREADTH * noise(map, owner, 40 + od as u32) * meander(map, hex, nb));
    wide.min(hw.max(HEX_SIZE * 0.5 - EDGE_MARGIN))
}

/// Sinuosité d'un fleuve vers sa case aval (1 sans aval connu).
fn sinuous(map: &WorldMap, hex: Hex) -> f32 {
    // (une ria est une vallée qui serpente, même au pied d'un relief)
    if mouth_kind(map, hex) == Some(MouthKind::Ria) {
        return 1.0;
    }
    map.downstream(hex).map_or(1.0, |d| meander(map, hex, hex + DIRECTIONS[d]))
}

/// Demi-largeur d'un fleuve au milieu de sa case : selon son débit, un peu plus ou moins large
/// d'une case à l'autre en plaine (il ne garde pas toujours la même largeur).
fn fleuve_mid_half_width(map: &WorldMap, hex: Hex) -> f32 {
    let hw = fleuve_half_width(map.discharge(hex)) * (1.0 + BREADTH * noise(map, hex, 41) * sinuous(map, hex));
    match mouth_kind(map, hex) {
        Some(MouthKind::Estuary) => (hw * ESTUARY_MID_WIDEN).min(FLEUVE_HALF_WIDTH),
        Some(MouthKind::Ria) => hw * RIA_WIDTH,
        Some(MouthKind::Fjord) => hw * FJORD_WIDTH,
        _ => hw,
    }
}

/// Forme de l'embouchure d'une case de fleuve, s'il y en a une.
fn mouth_kind(map: &WorldMap, hex: Hex) -> Option<MouthKind> {
    map.mouth(hex).map(|m| m.kind)
}

/// Bras d'un delta digité, de la case `hex` vers son arête de mer `d` : leur nombre selon le débit
/// (moins s'ils n'ont pas la place), pour chacun le décalage le long de l'arête et la demi-largeur.
fn delta_arms(map: &WorldMap, hex: Hex, hw: f32) -> Vec<(f32, f32)> {
    let q = map.discharge(hex);
    let wanted = 2 + DELTA_ARMS_M3S.iter().filter(|&&s| q >= s).count();
    let half_edge = HEX_SIZE * 0.5 - DELTA_EDGE_MARGIN;
    let width = |n: usize| (hw * DELTA_ARM_SHARE / (n as f32).sqrt()).max(DELTA_ARM_MIN_HALF_WIDTH);
    let fits = |n: usize| 2.0 * n as f32 * width(n) + (n - 1) as f32 * DELTA_LAND_GAP <= 2.0 * half_edge;
    let n = (2..=wanted).rev().find(|&n| fits(n)).unwrap_or(2);
    let w = width(n).min((2.0 * half_edge - DELTA_LAND_GAP) / 4.0);
    let spread = half_edge - w;
    let room = ((2.0 * spread / (n - 1) as f32 - 2.0 * w - DELTA_LAND_GAP) / 2.0).clamp(0.0, DELTA_ARM_JITTER);
    (0..n)
        .map(|k| {
            let at = -spread + 2.0 * spread * k as f32 / (n - 1) as f32;
            // (les bras extrêmes ne bougent que vers l'intérieur : ils restent dans l'arête)
            let jitter = noise(map, hex, 60 + k as u32) * room;
            let at = if k == 0 { at + jitter.abs() } else if k == n - 1 { at - jitter.abs() } else { at + jitter };
            (at, w * (1.0 + 0.15 * noise(map, hex, 70 + k as u32)))
        })
        .collect()
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
    c + along.perp() * noise(map, hex, 3) * fleuve_half_width(map.discharge(hex)) * CENTER_SHIFT * sinuous(map, hex)
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
    // (pas de rapide dans une embouchure : la chute vers la mer n'est pas la pente du fleuve)
    if map.mouth(hex).is_some() {
        return 0.0;
    }
    map.downstream(hex).map_or(0.0, |d| {
        // (la mer remonte la vallée d'une ria ou d'un fjord jusqu'ici)
        if map.mouth(hex + DIRECTIONS[d]).is_some_and(|m| matches!(m.kind, MouthKind::Ria | MouthKind::Fjord)) {
            return 0.0;
        }
        let drop = map.elevation(hex).unwrap_or(0.0) - map.elevation(hex + DIRECTIONS[d]).unwrap_or(0.0);
        rapids(drop / crate::climate::CELL_KM)
    })
}

/// Arêtes par où un fleuve ne fait que traverser sa case (ni embouchure, ni rive de lac, ni
/// confluent de fleuves).
fn crossed(map: &WorldMap, hex: Hex) -> Option<[usize; 2]> {
    if !map.is_fleuve(hex) {
        return None;
    }
    let Openings { open, full } = fleuve_openings(map, hex);
    let dirs: Vec<usize> = (0..6).filter(|d| open & (1 << d) != 0).collect();
    (dirs.len() == 2 && full == 0).then(|| [dirs[0], dirs[1]])
}

/// Pente (m/km) entre deux cases voisines.
fn drop_slope(map: &WorldMap, a: Hex, b: Hex) -> f32 {
    (map.elevation(a).unwrap_or(0.0) - map.elevation(b).unwrap_or(0.0)).abs() / crate::climate::CELL_KM
}

/// Chance d'une île sur un fleuve de débit `q` et de pente `slope` (m/km) : plus il est plat et
/// abondant, plus il se partage en bras.
fn isle_chance(q: f32, slope: f32) -> f32 {
    (ISLE_CHANCE_MIN + (ISLE_CHANCE_MAX - ISLE_CHANCE_MIN) * log_share(q, FLEUVE_M3S, FLEUVE_WIDE_M3S)) * (1.0 - slope / ISLE_FLAT_M_PER_KM).max(0.0)
}

/// Une rivière arrive au coin `j` de la case (affluent).
fn tributary_at(map: &WorldMap, hex: Hex, j: usize) -> bool {
    rivers_at_corner(map, hex, j, 6).next().is_some()
}

/// Valeur de la graine (0 à 1) attachée à une case et à un usage.
fn draw(map: &WorldMap, hex: Hex, salt: u32) -> f32 {
    (noise(map, hex, salt) + 1.0) / 2.0
}

/// Île tirée de la graine : demi-largeur et demi-longueur au plus large, décalage le long du
/// cours et en travers (dans les sens propres à l'arête qui la porte), dessus.
#[derive(Clone, Copy, Debug, PartialEq)]
struct IsleSpec {
    g: f32,
    l: f32,
    shift: f32,
    /// Décalage en travers dans un groupe (part de la place d'une île), dans le sens de l'arête.
    nudge: f32,
    top: IsleTop,
}

/// Dessus d'une île (même tirage vu des deux cases d'une arête) : plus elle est grande, plus
/// elle a de terre (sinon un banc de sable ou de limon nu, bas sur l'eau) et plus elle peut
/// s'élever.
fn isle_top(g: f32, l: f32, r1: f32, r2: f32) -> IsleTop {
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let soil = smooth((g - ISLE_MIN_HALF_WIDTH) / 2.0) * smooth((2.0 * l - 8.0) / 10.0);
    IsleTop { lift: ISLE_LIFT_MIN + 0.25 * r1 + ISLE_LIFT_SOIL * soil * (0.5 + 0.5 * r2), soil }
}

/// Îles à cheval sur l'arête `d` d'une case de fleuve, les mêmes vues des deux cases : le fleuve
/// traverse les deux, en plaine, sans affluent tout près. Une île le plus souvent, deux ou trois
/// côte à côte dans un grand fleuve (autant de bras de plus), de tailles et de longueurs
/// variées, décalées le long du cours ; aucune si la liste est vide.
fn edge_isles(map: &WorldMap, hex: Hex, d: usize) -> Vec<IsleSpec> {
    let nb = hex + DIRECTIONS[d];
    let back = (d + 3) % 6;
    // (pas d'île dans une embouchure : elle resterait isolée à la côte)
    if map.mouth(hex).is_some() || map.mouth(nb).is_some() {
        return Vec::new();
    }
    if !crossed(map, hex).is_some_and(|ds| ds.contains(&d)) || !crossed(map, nb).is_some_and(|ds| ds.contains(&back)) {
        return Vec::new();
    }
    // (coins de l'arête et des deux arêtes qui l'encadrent, de chaque côté : le panache d'un
    // affluent passerait sur l'île)
    let near = |h: Hex, e: usize| [(e + 5) % 6, e, (e + 1) % 6, (e + 2) % 6].into_iter().any(|j| tributary_at(map, h, j));
    if near(hex, d) || near(nb, back) {
        return Vec::new();
    }
    let (owner, od) = edge_owner(hex, d);
    let q = map.discharge(hex).min(map.discharge(nb));
    if draw(map, owner, 20 + od as u32) >= isle_chance(q, drop_slope(map, hex, nb)) * ISLE_EDGE_SHARE {
        return Vec::new();
    }
    let hw = fleuve_edge_half_width(map, hex, d);
    let r = |i: usize, j: u32| draw(map, owner, 100 + 24 * i as u32 + 4 * j + od as u32);
    let many = r(0, 5);
    let n = 1 + usize::from(hw >= 10.0 && many < 0.45) + usize::from(hw >= 12.0 && many < 0.2);
    (0..n)
        .map(|i| {
            let g = (hw * (0.12 + 0.43 * r(i, 0).powf(0.8)) / (n as f32).powf(0.3)).max(ISLE_MIN_HALF_WIDTH);
            let l = HEX_SIZE * (0.18 + 0.4 * r(i, 1));
            let nudge = if n > 1 { 0.3 * (r(i, 5) - 0.5) } else { 0.0 };
            IsleSpec { g, l, shift: (r(i, 2) - 0.5) * HEX_SIZE * 0.4, nudge, top: isle_top(g, l, r(i, 3), r(i, 4)) }
        })
        .collect()
}

/// Îles d'une case que le fleuve traverse par les arêtes `dirs` : groupes d'îles à cheval sur
/// ces arêtes, ou bras secondaire (l'île, le long de la traversée, et la demi-largeur de son
/// bras ; le côté est choisi avec la forme du fleuve).
#[derive(Default)]
struct Isles {
    edge: [Vec<IsleSpec>; 2],
    side: Option<(IsleSpec, f32)>,
}

impl Isles {
    fn of(map: &WorldMap, hex: Hex, dirs: [usize; 2]) -> Self {
        if map.mouth(hex).is_some() {
            return Isles::default();
        }
        let edge = dirs.map(|d| edge_isles(map, hex, d));
        if edge.iter().any(|e| !e.is_empty()) {
            return Isles { edge, side: None };
        }
        let q = map.discharge(hex);
        let slope = dirs.iter().map(|&d| drop_slope(map, hex, hex + DIRECTIONS[d])).fold(0.0, f32::max);
        if draw(map, hex, 30) >= isle_chance(q, slope) * ISLE_SIDE_SHARE {
            return Isles::default();
        }
        let hw = fleuve_half_width(q);
        let r = |salt: u32| draw(map, hex, salt);
        let g = (hw * (0.15 + 0.35 * r(31))).max(ISLE_MIN_HALF_WIDTH);
        // (longueur : part de la traversée, de 30 à 60 % ; décalage : son milieu)
        let (l, shift) = (0.15 + 0.15 * r(33), 0.4 + 0.2 * r(34));
        let isle = IsleSpec { g, l, shift, nudge: 0.0, top: isle_top(g, l * 2.0 * HEX_SIZE * 0.9, r(35), r(36)) };
        Isles { edge, side: Some((isle, hw * (0.15 + 0.15 * r(32)))) }
    }

    fn any(&self) -> bool {
        self.side.is_some() || self.edge.iter().any(|e| !e.is_empty())
    }
}

/// Directions en travers d'une ligne d'eau de fleuve : la moyenne de ses deux tronçons en chaque
/// point, mais exactement celle du premier et du dernier tronçon à ses deux bouts (le débord sur
/// l'arête et le point de passage) : la ligne y est perpendiculaire à l'arête, et le bord de
/// l'eau tombe au même endroit vu des deux cases, quel que soit leur découpage (sans quoi il fait
/// une marche d'une case à l'autre).
fn fleuve_normals(path: &[PathPoint]) -> Vec<Vec2> {
    let last = path.len() - 1;
    let chord = |a: usize, b: usize| (path[b].0 - path[a].0).perp().normalize_or_zero();
    (0..=last)
        .map(|k| match k {
            _ if last < 2 => chord(0, last),
            0 | 1 => chord(0, 1),
            _ if k + 1 >= last => chord(last - 1, last),
            _ => chord(k - 1, k + 1),
        })
        .collect()
}

/// Bosse (0 à 1, nulle aux bouts) d'une île ou d'un bras, à la position `x` (0 à 1) de sa
/// longueur ; `round` : plus petit, plus la forme est pleine.
fn lens(x: f32, round: f32) -> f32 {
    if (0.0..=1.0).contains(&x) { (std::f32::consts::PI * x).sin().max(0.0).powf(round) } else { 0.0 }
}

/// Une île sur la ligne d'eau : demi-largeur (`gap`) à chaque point, décalage en travers (part
/// de la place d'une île), place dans son groupe, dessus.
struct Cut {
    gap: Vec<f32>,
    nudge: f32,
    /// Rang en travers dans son groupe, taille du groupe.
    slot: (usize, usize),
    top: IsleTop,
}

/// Fleuve qui traverse sa case (ligne `path`, de son débord sur la première arête à celui sur
/// la seconde) et se partage autour d'îles. Le fleuve s'élargit et se resserre peu à peu, sur
/// une longueur plus grande que les îles (pas de renflement brusque), et l'eau garde au total
/// plus de la largeur du fleuve (il peut un peu déborder sur les cases voisines).
/// - À cheval sur une arête : une à trois îles côte à côte, l'eau partagée également entre les
///   bras ; la même forme vue des deux cases (centrée sur le point de passage, où la ligne est
///   perpendiculaire à l'arête des deux côtés ; positions en travers et le long du cours dans un
///   sens propre à l'arête).
/// - Bras secondaire : la rive d'un côté s'écarte, un bras s'y détache derrière une île puis
///   rejoint le fleuve, qui se resserre un peu (l'eau couvre toujours le centre) ; plutôt vers
///   le centre de la case (plus de place), jamais vers un affluent ni sur l'eau d'à côté.
///
/// Chaque île est aussi rangée dans `shape.isles` (relief et arbres), avec ses lits d'île.
fn add_isles(map: &WorldMap, hex: Hex, dirs: [usize; 2], path: &[PathPoint], isles: &Isles, shape: &mut WaterShape) {
    let last = path.len() - 1;
    let normal = fleuve_normals(path);
    let mut s = vec![0.0f32];
    for w in path.windows(2) {
        s.push(s.last().unwrap() + w[0].0.distance(w[1].0));
    }
    let (s_in, s_out) = (s[1], s[last - 1]);
    let hw = |k: usize| path[k].1;
    // En travers de chaque point : l'eau va de `lo` à `hi` (décalages le long de la normale),
    // l'île `i` est centrée en `at[i]` ; rives en sinus carré (sans cassure)
    let mut lo: Vec<f32> = (0..=last).map(|k| -hw(k)).collect();
    let mut hi: Vec<f32> = (0..=last).map(hw).collect();
    let mut cuts: Vec<Cut> = Vec::new();
    let mut at: Vec<Vec<f32>> = Vec::new();
    let side = isles.side.and_then(|(isle, side_arm)| {
        // Bras secondaire, sur le milieu de la traversée : la rive de son côté s'écarte (au plus
        // de l'île et du bras, moins un peu de la largeur du fleuve, qui se resserre), l'île
        // apparaît le long de la rive écartée ; le bras garde sa largeur
        let x = |k: usize| (s[k] - s_in) / (s_out - s_in);
        let spread = |k: usize| (2.0 * isle.g + 2.0 * side_arm - ISLE_SIDE_NARROW * hw(k)) * lens((x(k) - 0.05) / 0.9, 2.0);
        let mid = last / 2;
        let fits = |side: f32| {
            // Pas d'affluent de ce côté, pas d'eau voisine sous le bras
            let tributary = (0..6).any(|j| {
                let c = corner(hex, j);
                let m = (0..=last).min_by(|&a, &b| path[a].0.distance(c).total_cmp(&path[b].0.distance(c))).unwrap_or(mid);
                tributary_at(map, hex, j) && (c - path[m].0).dot(normal[m]) * side > 0.0
            });
            // (là où la rive s'est écartée)
            let wet = (0..=last).filter(|&k| spread(k) > 1.0).any(|k| {
                let p = path[k].0 + normal[k] * side * (hw(k) + spread(k));
                let h = Hex::from_world(p.x, p.y);
                h != hex && (map.get(h).is_some_and(|t| t.is_water()) || map.is_fleuve(h))
            });
            !tributary && !wet
        };
        let toward_center = if (center(hex) - path[mid].0).dot(normal[mid]) >= 0.0 { 1.0 } else { -1.0 };
        let side = [toward_center, -toward_center].into_iter().find(|&side| fits(side))?;
        let cross: Vec<(f32, f32, f32)> = (0..=last)
            .map(|k| {
                let outer = hw(k) + spread(k);
                // (le fleuve garde le centre de la case : son bord côté île reste à 60 % de sa
                // demi-largeur au moins)
                let g = (isle.g * lens((x(k) - isle.shift + isle.l) / (2.0 * isle.l), 0.8)).min(((outer - 2.0 * side_arm - 0.6 * hw(k)) / 2.0).max(0.0));
                (outer, outer - 2.0 * side_arm - g, g)
            })
            .collect();
        Some((side, cross, isle))
    });
    if let Some((side, cross, isle)) = side {
        let mut centers = Vec::new();
        let mut gap = Vec::new();
        for (k, &(outer, c, g)) in cross.iter().enumerate() {
            if side > 0.0 { hi[k] = outer } else { lo[k] = -outer }
            centers.push(side * c);
            gap.push(g);
        }
        cuts.push(Cut { gap, nudge: 0.0, slot: (0, 1), top: isle.top });
        at.push(centers);
    } else {
        // Groupes d'îles à cheval sur les arêtes : sens propres à chaque arête (le long du cours,
        // en travers), élargissement du fleuve (plus long que les îles), îles dans l'ordre en
        // travers de cette case
        let mut envelope = vec![0.0f32; last + 1];
        for (e, specs) in isles.edge.iter().enumerate() {
            if specs.is_empty() {
                continue;
            }
            let (k_e, s_e) = if e == 0 { (1, s_in) } else { (last - 1, s_out) };
            let (owner, od) = edge_owner(hex, dirs[e]);
            let (ca, cb) = owner.edge_corners(od);
            let along_edge = v(cb) - v(ca);
            let tangent = (path[k_e + 1].0 - path[k_e - 1].0).normalize_or_zero();
            let forward = if tangent.dot(along_edge.perp()) >= 0.0 { 1.0 } else { -1.0 };
            let across = if normal[k_e].dot(along_edge) >= 0.0 { 1.0 } else { -1.0 };
            let reach = specs.iter().map(|i| i.shift.abs() + i.l).fold(ISLE_SPREAD, f32::max) * 1.2;
            let widen: f32 = specs.iter().map(|i| i.g).sum::<f32>() + ISLE_ARM_EXTRA * path[k_e].1;
            for k in 0..=last {
                envelope[k] = envelope[k].max(widen * lens(0.5 + (s[k] - s_e) / (2.0 * reach), 2.0));
            }
            let mut group: Vec<Cut> = specs
                .iter()
                .map(|i| {
                    let gap: Vec<f32> = (0..=last).map(|k| i.g * lens(0.5 + (forward * (s[k] - s_e) - i.shift) / (2.0 * i.l), 0.6)).collect();
                    Cut { gap, nudge: i.nudge * across, slot: (0, specs.len()), top: i.top }
                })
                .collect();
            if across < 0.0 {
                group.reverse();
            }
            for (i, c) in group.iter_mut().enumerate() {
                c.slot.0 = i;
            }
            cuts.extend(group);
        }
        // Rives ; chaque île garde sa place en travers du fleuve (une part fixe de sa largeur selon
        // son rang dans son groupe, un peu décalée quand elles sont plusieurs : la même vue des
        // deux cases), sans toucher sa voisine (un bras au moins entre deux îles)
        at = cuts.iter().map(|_| vec![0.0; last + 1]).collect();
        for k in 0..=last {
            let gaps: f32 = cuts.iter().map(|c| c.gap[k]).sum();
            // (la marge d'eau en plus vient avec les îles : rien là où il n'y en a pas)
            let needed = gaps + (ISLE_WATER_MIN - 1.0) * hw(k) * (gaps / 2.0).min(1.0);
            hi[k] = hw(k) + envelope[k].max(needed);
            lo[k] = -hi[k];
            for (i, c) in cuts.iter_mut().enumerate() {
                let n = c.slot.1 as f32;
                let room = (hi[k] - lo[k]) / (n + 1.0) - ISLE_ARM_MIN * hw(k);
                c.gap[k] = c.gap[k].min((room / 2.0).max(0.0));
                at[i][k] = lo[k] + (hi[k] - lo[k]) * (c.slot.0 as f32 + 1.0 + c.nudge) / (n + 1.0);
            }
        }
    }
    // Morceaux : entre deux changements d'îles présentes, l'eau entre les îles
    let active = |k: usize| -> Vec<bool> { cuts.iter().map(|c| c.gap[k] > 0.0 || c.gap[(k + 1).min(last)] > 0.0).collect() };
    let mut k = 0;
    while k < last {
        let start = k;
        let here = active(k);
        while k < last && active(k) == here {
            k += 1;
        }
        let on: Vec<usize> = (0..cuts.len()).filter(|&i| here[i]).collect();
        for piece in 0..=on.len() {
            let left = |i: usize| if piece == 0 { lo[i] } else { at[on[piece - 1]][i] + cuts[on[piece - 1]].gap[i] };
            let right = |i: usize| if piece == on.len() { hi[i] } else { at[on[piece]][i] - cuts[on[piece]].gap[i] };
            shape.offset_strip(path, &normal, start..=k, |i| (left(i), right(i)), WaterKind::Fleuve);
        }
    }
    // Îles pour le relief et les arbres : ligne de milieu (points où l'île existe, et un de part
    // et d'autre pour ses pointes)
    for (i, c) in cuts.iter().enumerate() {
        let spine: Vec<(Vec2, f32)> = (0..=last)
            .filter(|&k| c.gap[k] > 0.0 || c.gap[k.saturating_sub(1)] > 0.0 || c.gap[(k + 1).min(last)] > 0.0)
            .map(|k| (path[k].0 + normal[k] * at[i][k], c.gap[k]))
            .collect();
        if spine.len() >= 2 {
            shape.isle(spine, c.top);
        }
    }
}

/// Rivières des arêtes qui touchent le coin `j` de la case, sauf celle de son arête `d` (les
/// trois arêtes d'un coin : deux de la case, une entre ses deux voisines).
fn rivers_at_corner(map: &WorldMap, hex: Hex, j: usize, d: usize) -> impl Iterator<Item = EdgeRiver> + '_ {
    let (n1, n2) = (hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]);
    let between = DIRECTIONS.iter().position(|&s| n1 + s == n2).unwrap_or(0);
    [(hex, (j + 5) % 6), (hex, j), (n1, between)].into_iter().filter(move |&(h, e)| !(h == hex && e == d)).filter_map(|(h, e)| edge_river(map, h, e))
}

/// Le point `p` est dans le lit d'un cours d'eau (à `margin` près) de la case ou de sa voisine
/// d'en face par l'arête `dir` (une rivière d'arête est portée par l'une ou l'autre) : pour ne pas
/// poser de décor dans l'eau.
pub fn in_water(map: &WorldMap, hex: Hex, dir: usize, p: Vec2, margin: f32) -> bool {
    [hex, hex + DIRECTIONS[dir % 6]].into_iter().any(|h| {
        hex_water(map, h).channels.iter().filter(|c| c.isle.is_none()).any(|c| {
            let ab = c.b - c.a;
            let t = if ab.length_squared() > 0.0 { ((p - c.a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            p.distance(c.a + ab * t) < c.hw + (c.hw_end - c.hw) * t + margin
        })
    })
}

/// Formes d'eau d'une case.
pub fn hex_water(map: &WorldMap, hex: Hex) -> WaterShape {
    let mut shape = WaterShape::default();
    // Rivières des arêtes « à soi » (directions 0 à 2) : une courbe de coin en coin qui se
    // raccorde à celles d'amont et d'aval, avec un rond à chaque bout (confluents)
    for (d, &step) in DIRECTIONS.iter().enumerate().take(3) {
        let Some(own) = edge_river(map, hex, d) else { continue };
        let (node_up, node_down) = (river_node(map, own.up), river_node(map, own.down));
        let along = (node_down - node_up).normalize_or_zero();
        // Tronçons voisins aux deux bouts, dans le sens du courant (en amont, le plus abondant)
        let (j_up, j_down) = if own.up.distance(corner(hex, d)) < 0.5 { (d, (d + 1) % 6) } else { ((d + 1) % 6, d) };
        let main = |list: Vec<EdgeRiver>| list.into_iter().max_by(|x, y| x.q.total_cmp(&y.q));
        let prev = main(rivers_at_corner(map, hex, j_up, d).filter(|r| r.down.distance(own.up) < 0.5).collect());
        let next = main(rivers_at_corner(map, hex, j_down, d).filter(|r| r.up.distance(own.down) < 0.5).collect());
        // Au coin, la direction moyenne des deux tronçons : pas de cassure d'une arête à l'autre
        let bisect = |r: &EdgeRiver| (along + (river_node(map, r.down) - river_node(map, r.up)).normalize_or_zero()).normalize_or(along);
        let (t_up, t_down) = (prev.as_ref().map_or(along, bisect), next.as_ref().map_or(along, bisect));
        let hw = river_half_width(own.q);
        // (à sa source, elle part fine : pas de bout rond déjà large)
        let (hw_up, tone_up) = prev.as_ref().map_or((hw.min(RIVER_SOURCE_HALF_WIDTH.max(hw * RIVER_SOURCE_SHARE)), own.tone), |r| (river_half_width(r.q), r.tone));
        let length = node_up.distance(node_down);
        let mut path: Vec<PathPoint> = vec![(node_up, hw_up, tone_up)];
        path.extend(hermite(node_up, t_up * length, node_down, t_down * length, RIVER_STEPS).map(|(s, p)| (p, hw_up + (hw - hw_up) * s, tone_up.lerp(own.tone, s))));
        shape.also = Some(hex + step);
        shape.strip(&path, WaterKind::River);
        shape.rapid(&path, true);
        shape.tone = tone_up;
        shape.disc(node_up, Vec2::splat(hw_up), WaterKind::River);
        shape.tone = own.tone;
        shape.disc(node_down, Vec2::splat(hw), WaterKind::River);
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
    let hw = fleuve_mid_half_width(map, hex);
    let murk = map.water_murk(hex);
    let own = Tone { rapids: fleuve_rapids(map, hex), ..tone(q, murk) };
    let Openings { open, full } = fleuve_openings(map, hex);
    let kind = mouth_kind(map, hex);
    // Bout d'un bras : point de passage, demi-largeur, teinte, direction vers l'extérieur
    let end = |d: usize| {
        let out = (edge_mid(hex, d) - c).normalize_or_zero();
        let nb = hex + DIRECTIONS[d];
        if full & (1 << d) != 0 {
            // Lac (embouchure, sortie ou rive) : toute l'arête, le lac et le fleuve forment une
            // seule nappe d'eau ; mer : le fleuve s'évase, d'autant plus qu'il est large
            let flare = if map.get(nb) == Some(Terrain::Lake) {
                ESTUARY_HALF_WIDTH
            } else if kind == Some(MouthKind::Estuary) && map.downstream(hex) == Some(d) {
                ESTUARY_END_HALF_WIDTH
            } else if kind == Some(MouthKind::Ria) {
                hw * 1.2
            } else if kind == Some(MouthKind::Fjord) {
                hw
            } else {
                (hw * MOUTH_FLARE).min(ESTUARY_HALF_WIDTH)
            };
            return (edge_mid(hex, d), flare, own, out);
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
    // Delta digité : plusieurs bras vers l'arête de mer, de la terre entre eux
    let fan = down_dir.filter(|&d| kind == Some(MouthKind::BirdFoot) && open & (1 << d) != 0);
    // (un seul fleuve qui arrive : il se divise dès l'entrée de la case, le delta l'occupe toute)
    let stem = fan.and_then(|f| (dirs.len() == 2).then(|| dirs.iter().copied().find(|&d| d != f)).flatten());
    // Source d'un fleuve (une seule arête, vers l'aval) : il prolonge sa rivière principale, depuis
    // le coin où elle arrive, ou s'effile vers la voisine qui l'alimente
    let head = (dirs.len() == 1 && kind.is_none()).then(|| dirs[0]).filter(|&d| down_dir == Some(d));
    let corner_river = |j: usize| {
        let (a, b) = (hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]);
        let dir = DIRECTIONS.iter().position(|&dd| a + dd == b)?;
        edge_river(map, a, dir).filter(|r| r.down.distance(corner(hex, j)) < 0.5)
    };
    let main_river = head.and_then(|_| (0..6).filter_map(|j| corner_river(j).map(|r| (j, r))).max_by(|x, y| x.1.q.total_cmp(&y.1.q)));
    if dirs.len() == 2 && !wet_pair && fan.is_none() {
        // Traversée : d'une arête au milieu du fleuve, puis à l'autre arête (plus finement
        // découpée autour d'îles)
        let ((p0, h0, t0, out0), (p1, h1, t1, out1)) = (end(dirs[0]), end(dirs[1]));
        let isles = Isles::of(map, hex, [dirs[0], dirs[1]]);
        let steps = if isles.any() { ISLE_STEPS } else { STEPS };
        let along = (p1 - p0).normalize_or(-out0);
        let mut path: Vec<PathPoint> = vec![(p0 + out0 * OVERLAP, h0, t0), (p0, h0, t0)];
        let l0 = p0.distance(mid);
        path.extend(hermite(p0, -out0 * l0, mid, along * l0, steps).map(|(s, p)| (p, h0 + (hw - h0) * s, t0.lerp(own, s))));
        let l1 = mid.distance(p1);
        path.extend(hermite(mid, along * l1, p1, out1 * l1, steps).map(|(s, p)| (p, hw + (h1 - hw) * s, own.lerp(t1, s))));
        path.push((p1 + out1 * OVERLAP, h1, t1));
        follow(&path, if down_dir == Some(dirs[0]) { -1.0 } else { 1.0 });
        if isles.any() {
            add_isles(map, hex, [dirs[0], dirs[1]], &path, &isles, shape);
        } else {
            shape.fleuve_strip(&path);
            shape.rapid(&path, down_dir != Some(dirs[0]));
        }
    } else {
        let fork = stem.map_or(mid, |u| fleuve_crossing(map, hex, u).lerp(mid, DELTA_FORK));
        if stem.is_none() && head.is_none() {
            shape.disc(mid, Vec2::splat(hw), WaterKind::Fleuve);
        }
        if let Some(d) = head {
            let feeding = (0..6).filter(|&e| e != d && map.downstream(hex + DIRECTIONS[e]) == Some((e + 3) % 6)).max_by(|&a, &b| map.discharge(hex + DIRECTIONS[a]).total_cmp(&map.discharge(hex + DIRECTIONS[b])));
            let (from, start, tip_tone, inward) = match &main_river {
                Some((j, r)) => (river_node(map, corner(hex, *j)), river_half_width(r.q), r.tone, (r.down - r.up).normalize_or_zero()),
                None => {
                    let from = edge_mid(hex, feeding.unwrap_or((d + 3) % 6));
                    (from, SOURCE_HALF_WIDTH, own, (mid - from).normalize_or_zero())
                }
            };
            let l = from.distance(mid);
            let (p, h, t, out) = end(d);
            let through = (p - from).normalize_or(inward);
            let mut path: Vec<PathPoint> = vec![(from, start, tip_tone)];
            path.extend(hermite(from, inward * l, mid, through * l, STEPS).map(|(s, x)| (x, start + (hw - start) * s.sqrt(), tip_tone.lerp(own, s))));
            let l1 = mid.distance(p);
            path.extend(hermite(mid, through * l1, p, out * l1, STEPS).map(|(s, x)| (x, hw + (h - hw) * s, own.lerp(t, s))));
            path.push((p + out * OVERLAP, h, t));
            follow(&path, 1.0);
            shape.fleuve_strip(&path);
            shape.rapid(&path, true);
        }
        for &d in dirs.iter().filter(|&&d| head != Some(d)) {
            if stem == Some(d) {
                // Le fleuve entre dans sa case, jusqu'à l'endroit où il se divise
                let (p, h, t, out) = end(d);
                let l = p.distance(fork);
                let mut path: Vec<PathPoint> = vec![(p + out * OVERLAP, h, t), (p, h, t)];
                path.extend(hermite(p, -out * l, fork, (mid - fork).normalize_or(-out) * l, STEPS).map(|(s, x)| (x, h + (hw * 0.8 - h) * s, t.lerp(own, s))));
                follow(&path, 1.0);
                shape.fleuve_strip(&path);
                continue;
            }
            if fan == Some(d) {
                let out = (edge_mid(hex, d) - c).normalize_or_zero();
                let along = out.perp();
                let arms = delta_arms(map, hex, hw);
                // (un bras passe par le milieu : l'eau couvre le centre, où sont les unités)
                let central = (0..arms.len()).min_by(|&a, &b| arms[a].0.abs().total_cmp(&arms[b].0.abs())).unwrap_or(0);
                for (k, &(at, w)) in arms.iter().enumerate() {
                    let p = edge_mid(hex, d) + along * at;
                    let start = hw * 0.7;
                    let through = if k == central { mid } else { fork.lerp(p, 0.45) + along * (at.signum() * 3.0) };
                    let (l0, l1) = (fork.distance(through), through.distance(p));
                    let middle = (p - fork).normalize_or(out);
                    let mid_width = start + (w - start) * 0.45;
                    let mut path: Vec<PathPoint> = vec![(fork, start, own)];
                    path.extend(hermite(fork, (through - fork).normalize_or(out) * l0, through, middle * l0, STEPS).map(|(s, x)| (x, start + (mid_width - start) * s, own)));
                    path.extend(hermite(through, middle * l1, p, out * l1, STEPS).map(|(s, x)| (x, mid_width + (w - mid_width) * s, own)));
                    path.push((p + out * OVERLAP, w, own));
                    follow(&path, 1.0);
                    shape.fleuve_strip(&path);
                }
                continue;
            }
            let (p, h, t, out) = end(d);
            let l = mid.distance(p);
            let mut path: Vec<PathPoint> = vec![(mid, hw, own)];
            path.extend(hermite(mid, (p - mid).normalize_or_zero() * l, p, out * l, STEPS).map(|(s, x)| (x, hw + (h - hw) * s, own.lerp(t, s))));
            path.push((p + out * OVERLAP, h, t));
            follow(&path, if down_dir == Some(d) { 1.0 } else { -1.0 });
            shape.fleuve_strip(&path);
            shape.rapid(&path, down_dir == Some(d));
        }
        // Deux arêtes ouvertes côte à côte, l'une sur toute sa largeur ou toutes deux sur l'eau
        // (embouchure dans un coin de mer) : l'eau remplit aussi le coin qui les sépare (pas de
        // pointe de terre entre elles)
        let whole = |d: usize| end(d).1 >= ESTUARY_HALF_WIDTH - 1e-3;
        let wet = |d: usize| full & (1 << d) != 0;
        for d in 0..6 {
            let e = (d + 1) % 6;
            // (pas entre les bras d'un delta et ses voisines : la terre y reste)
            let both = open & (1 << d) != 0 && open & (1 << e) != 0 && fan != Some(d) && fan != Some(e);
            if both && (whole(d) || whole(e) || (wet(d) && wet(e))) {
                let k = corner(hex, e);
                let k = k + (k - c).normalize_or_zero() * OVERLAP;
                // (estuaire : assez large pour ne laisser aucune pointe de terre près du coin)
                let tip = if kind == Some(MouthKind::Estuary) { HEX_SIZE * 0.45 } else if whole(d) || whole(e) { HEX_SIZE * 0.25 } else { end(d).1.min(end(e).1) };
                // Un seul éventail du milieu jusqu'au coin, entre les bouts des deux bras : ni barre
                // droite ni bout plat
                let reach = |x: usize| {
                    let (p, h, _, _) = end(x);
                    p + (k - p).normalize_or_zero() * h.min(p.distance(k))
                };
                shape.channel(mid, k, hw, tip, WaterKind::Fleuve, [0.0, 0.0]);
                shape.tri([mid, reach(d), k], [own; 3], WaterKind::Fleuve);
                shape.tri([mid, k, reach(e)], [own; 3], WaterKind::Fleuve);
            }
        }
    }
    // Intérieur d'un virage serré avant l'embouchure : l'eau occupe ce côté, sans atteindre l'arête
    for e in bend_edges(map, hex) {
        let (a, b) = (mid.lerp(corner(hex, e), BEND_FILL), mid.lerp(corner(hex, (e + 1) % 6), BEND_FILL));
        shape.channel(mid, a.lerp(b, 0.5), hw, HEX_SIZE * 0.3, WaterKind::Fleuve, [0.0, 0.0]);
        shape.tri([mid, a, b], [own; 3], WaterKind::Fleuve);
    }
    // Affluents (rivières d'arête) qui arrivent à un coin : ils continuent dans leur direction
    // et courbent vers l'aval jusqu'au fleuve, le long de sa rive ; dans le fleuve, leur couleur
    // se fond peu à peu dans la sienne (bords d'abord, puis le milieu, sur `PLUME` unités)
    if course.is_empty() {
        course.push((mid, Vec2::ZERO));
    }
    for j in 0..6 {
        // (la rivière principale d'une source ne la rejoint pas : le fleuve la prolonge)
        if main_river.as_ref().is_some_and(|m| m.0 == j) {
            continue;
        }
        let (a, b) = (hex + DIRECTIONS[(j + 5) % 6], hex + DIRECTIONS[j]);
        let Some(dir) = DIRECTIONS.iter().position(|&dd| a + dd == b) else { continue };
        let Some(trib) = edge_river(map, a, dir) else { continue };
        let k_exact = corner(hex, j);
        let k = river_node(map, k_exact);
        let &(axis, flow) = course.iter().min_by(|p, q| p.0.distance(k).total_cmp(&q.0.distance(k))).unwrap_or(&(mid, Vec2::ZERO));
        let toward = (axis - k).normalize_or_zero();
        let flow = if flow == Vec2::ZERO { toward } else { flow };
        // Du côté de l'affluent, un peu vers l'aval
        let side = flow.perp() * (k - axis).dot(flow.perp()).signum();
        // Le panache reste dans le fleuve : il s'élargit au plus jusqu'à `INFLOW_FILL` du fleuve,
        // et longe la rive d'autant plus près de l'axe que l'affluent est large (sinon il dépasse
        // de la berge, puis s'arrête net)
        let hw_r = river_half_width(trib.q);
        let widest = (hw_r * (1.0 + 0.2 * 3.0)).min(hw * INFLOW_FILL);
        let bank = (hw * INFLOW_BANK).min(hw - widest).max(0.0);
        let target = axis + side * bank + flow * hw * 0.6;
        let incoming = if trib.down.distance(k_exact) < 0.5 { (trib.down - trib.up).normalize_or(toward) } else { toward };
        let l = k.distance(target);
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
            at = a + f.perp() * bank_side * bank;
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
            let width = if i < first_wet { width } else { width.min(widest) };
            // (il s'amincit jusqu'au bout : pas d'arrêt carré dans le fleuve)
            let width = width * if i == last { 0.15 } else if i + 1 == last { 0.6 } else { 1.0 };
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

    /// Fleuve qui traverse une case et se jette dans la mer à l'est, avec cette forme d'embouchure.
    fn embouchure(kind: Option<MouthKind>, q: f32) -> (WorldMap, Hex) {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set(h + DIRECTIONS[0], Terrain::Ocean);
        map.set_fleuve_links(h, 1 << 0 | 1 << 3);
        map.set_discharge(h, q);
        map.set_downstream(h, 0);
        if let Some(kind) = kind {
            map.set_mouth(h, kind);
        }
        (map, h)
    }

    /// L'eau de la forme couvre ce point.
    fn couvert(shape: &WaterShape, p: Vec2) -> bool {
        shape.tris.iter().any(|t| {
            let d = |a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
            let (x, y, z) = (d(t.pts[0], t.pts[1]), d(t.pts[1], t.pts[2]), d(t.pts[2], t.pts[0]));
            (x >= 0.0 && y >= 0.0 && z >= 0.0) || (x <= 0.0 && y <= 0.0 && z <= 0.0)
        })
    }

    #[test]
    fn pas_d_ile_dans_une_embouchure() {
        // Fleuve de plaine, assez grand pour avoir des îles : aucune dans sa case d'embouchure
        let (mut map, row) = fleuve_en_plaine(12, 0.0, 40_000.0);
        assert!(row.iter().any(|&h| !hex_water(&map, h).isles.is_empty()), "ce fleuve a des îles");
        for &h in &row {
            map.set_mouth(h, MouthKind::Estuary);
        }
        assert!(row.iter().all(|&h| hex_water(&map, h).isles.is_empty()));
    }

    #[test]
    fn estuaire_en_entonnoir() {
        let (plain, h) = embouchure(None, 4_000.0);
        let (funnel, _) = embouchure(Some(MouthKind::Estuary), 4_000.0);
        // Sur la côte, plus loin que l'évasement ordinaire, jusqu'au bord de l'arête
        let along = (edge_mid(h, 0) - center(h)).normalize_or_zero().perp();
        let side = edge_mid(h, 0) - (edge_mid(h, 0) - center(h)).normalize_or_zero() * 2.0 + along * (HEX_SIZE * 0.44);
        assert!(!couvert(&hex_water(&plain, h), side), "sans type : terre à cet endroit");
        assert!(couvert(&hex_water(&funnel, h), side), "estuaire : l'eau y arrive");
        assert!(couvert(&hex_water(&funnel, h), center(h)), "l'eau couvre le centre");
        // L'entonnoir commence en amont, et la largeur reste la même vue des deux cases
        assert!(fleuve_mid_half_width(&funnel, h) > fleuve_mid_half_width(&plain, h));
        assert!(fleuve_edge_half_width(&funnel, h, 3) >= fleuve_edge_half_width(&plain, h, 3));
    }

    #[test]
    fn source_effilee_vers_la_voisine_qui_alimente() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set_fleuve_links(h, 1 << 0);
        map.set_discharge(h, 2_500.0);
        map.set_downstream(h, 0);
        map.set_downstream(h + DIRECTIONS[3], 0);
        map.set_discharge(h + DIRECTIONS[3], 1_500.0);
        let shape = hex_water(&map, h);
        assert!(couvert(&shape, center(h)), "l'eau couvre le centre");
        // Fine à l'arête d'amont : de l'eau au milieu de l'arête, rien à côté
        let side = (edge_mid(h, 3) - center(h)).normalize_or_zero().perp();
        let near = edge_mid(h, 3) + (center(h) - edge_mid(h, 3)).normalize_or_zero() * 0.5;
        assert!(couvert(&shape, near), "elle arrive jusqu'à l'arête");
        assert!(!couvert(&shape, near + side * 3.0), "effilée");
    }

    #[test]
    fn source_qui_prolonge_sa_riviere() {
        // Fleuve à sa source (une seule arête) ; une rivière arrive à un coin : pas de panache, le
        // fleuve part de ce coin, à la largeur de la rivière
        let (mut map, row) = fleuve_rangee(4);
        let h = row[0];
        let j = (0..6).find(|&j| {
            let (a, b) = (h + DIRECTIONS[(j + 5) % 6], h + DIRECTIONS[j]);
            let dir = DIRECTIONS.iter().position(|&dd| a + dd == b).unwrap();
            let mut m = map.clone();
            m.set_river(a, dir, 400.0);
            edge_river(&m, a, dir).is_some_and(|r| r.down.distance(corner(h, j)) < 0.5)
        }).expect("une rivière qui descend vers un coin");
        let (a, b) = (h + DIRECTIONS[(j + 5) % 6], h + DIRECTIONS[j]);
        map.set_river(a, DIRECTIONS.iter().position(|&dd| a + dd == b).unwrap(), 400.0);
        let shape = hex_water(&map, h);
        assert!(shape.channels.iter().all(|c| c.inflow.is_none()), "pas de panache");
        let start = shape.channels.iter().filter(|c| c.kind == WaterKind::Fleuve).find(|c| c.a.distance(corner(h, j)) < 1e-3).expect("le fleuve part du coin");
        assert!((start.hw - river_half_width(400.0)).abs() < 1e-3);
        assert!(couvert(&shape, center(h)));
    }

    #[test]
    fn riviere_effilee_a_sa_source() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set_river(h, 0, 2_000.0);
        let strips: Vec<WaterAxis> = hex_water(&map, h).channels.into_iter().filter(|c| c.a != c.b).collect();
        let full = river_half_width(2_000.0);
        assert!(strips[0].hw < full * 0.6, "{}", strips[0].hw);
        assert!((strips.last().unwrap().hw_end - full).abs() < 1e-3);
    }

    /// Rivière de 3 arêtes qui suit le bord d'une rangée de cases (en zigzag sur la grille).
    fn riviere_en_zigzag() -> (WorldMap, Vec2) {
        let mut map = WorldMap::filled(12, 12, Terrain::Plains);
        let h = Hex::from_offset(5, 5);
        // (deux arêtes consécutives de la même case, puis la suite)
        map.set_river(h, 0, 800.0);
        map.set_river(h, 1, 800.0);
        (map, corner(h, 1))
    }

    #[test]
    fn un_coin_de_riviere_se_deplace_selon_la_graine_et_sans_rupture() {
        let (map, k) = riviere_en_zigzag();
        let n = river_node(&map, k);
        assert_eq!(n, river_node(&map, k), "déterministe");
        assert!(n.distance(k) > 1.0, "le coin se déplace");
        assert!(n.distance(k) < HEX_SIZE * 0.5, "sans s'éloigner de l'arête");
        // Les deux arêtes s'y raccordent : le tracé reste continu
        let h = Hex::from_offset(5, 5);
        let shape = hex_water(&map, h);
        let strips: Vec<&WaterAxis> = shape.channels.iter().filter(|c| c.a != c.b).collect();
        assert_eq!(strips.len(), 2 * RIVER_STEPS);
        assert!(strips[RIVER_STEPS - 1].b.distance(n) < 1e-3 && strips[RIVER_STEPS].a.distance(n) < 1e-3);
    }

    #[test]
    fn un_coin_reste_en_place_pres_de_l_eau_ou_a_une_source() {
        let (mut map, k) = riviere_en_zigzag();
        let h = Hex::from_offset(5, 5);
        // Un coin où la rivière commence ou finit ne bouge pas
        assert_eq!(river_node(&map, corner(h, 0)), corner(h, 0));
        map.set_fleuve_links(h + DIRECTIONS[0], 1 << 3);
        assert_eq!(river_node(&map, k), k, "près d'un fleuve");
    }

    #[test]
    fn ria_et_fjord_la_mer_remonte_la_vallee() {
        let (mut map, h) = embouchure(Some(MouthKind::Ria), 4_000.0);
        let up = h + DIRECTIONS[3];
        map.set_fleuve_links(up, 1 << 0);
        map.set_downstream(up, 0);
        map.set_discharge(up, 4_000.0);
        let (waters, blend) = fleuve_fade(&map, h);
        assert_eq!(waters, vec![h + DIRECTIONS[0]]);
        assert_eq!(blend, Some(INLET_BLEND));
        assert_eq!(fleuve_fade(&map, up), (vec![h + DIRECTIONS[0]], Some(INLET_BLEND)), "et la case d'amont");
        // Pas de rapide dans cette vallée, même très en pente
        map.set_elevation(up, 900.0);
        map.set_elevation(h, 300.0);
        assert_eq!(fleuve_rapids(&map, up), 0.0);
        // Plus étroite qu'un fleuve ordinaire
        let (plain, _) = embouchure(None, 4_000.0);
        assert!(fleuve_mid_half_width(&map, h) < fleuve_mid_half_width(&plain, h));
        map.set_mouth(h, MouthKind::Fjord);
        assert!(fleuve_mid_half_width(&map, h) < fleuve_mid_half_width(&plain, h));
    }

    #[test]
    fn delta_arque_une_seule_sortie() {
        let (mut map, h) = embouchure(Some(MouthKind::Arcuate), 20_000.0);
        map.set(h + DIRECTIONS[1], Terrain::Ocean);
        assert_eq!(fleuve_openings(&map, h).open, 1 << 0 | 1 << 3);
        let out = (edge_mid(h, 0) - center(h)).normalize_or_zero();
        assert!(couvert(&hex_water(&map, h), edge_mid(h, 0) - out * 0.5), "pas de terre au milieu de la sortie");
    }

    #[test]
    fn le_rivage_ondule_sans_fente_entre_deux_cases() {
        // Côte droite : la colonne 6 et au-delà est en mer
        let mut map = WorldMap::filled(14, 14, Terrain::Plains);
        for h in map.hexes().collect::<Vec<_>>() {
            if h.to_offset().0 >= 6 {
                map.set(h, Terrain::Ocean);
            }
        }
        let shore = (0..6).find_map(|d| {
            let h = Hex::from_offset(5, 7);
            map.get(h + DIRECTIONS[d]).is_some_and(|t| t.is_sea()).then_some((h, d))
        });
        let (land, d) = shore.expect("une arête de côte");
        let sea = land + DIRECTIONS[d];
        let ((cl, rl), (cs, rs)) = (coast_warp(&map, land), coast_warp(&map, sea));
        assert!(rl[d] != 0.0, "l'arête de côte ondule");
        assert_eq!(rl[d], -rs[(d + 3) % 6], "la même forme vue des deux cases");
        // Les deux coins de cette arête bougent pareil vus des deux cases
        for (j, k) in [(d, (d + 3 + 1) % 6), ((d + 1) % 6, (d + 3) % 6)] {
            assert!(corner(land, j).distance(corner(sea, k)) < 1e-3, "même coin");
            assert!(cl[j].distance(cs[k]) < 1e-3, "{:?} {:?}", cl[j], cs[k]);
        }
        assert!(cl.iter().any(|c| c.length() > 0.0));
        // Loin de la mer : rien
        let far = coast_warp(&map, Hex::from_offset(1, 7));
        assert!(far.0.iter().all(|c| *c == Vec2::ZERO) && far.1.iter().all(|r| *r == 0.0));
        // Une falaise garde son coin exact
        map.set_elevation(land, 1_500.0);
        let cliff = coast_warp(&map, land);
        assert_eq!(cliff.1[d], 0.0);
        assert_eq!(cliff.0[d], Vec2::ZERO);
    }

    #[test]
    fn les_angles_du_rivage_s_arrondissent() {
        // Îlot d'une case : chaque coin se rapproche de son centre, en moyenne
        let mut map = WorldMap::filled(14, 14, Terrain::Ocean);
        let h = Hex::from_offset(7, 7);
        map.set(h, Terrain::Plains);
        let (corners, _) = coast_warp(&map, h);
        let inward: f32 = (0..6).map(|j| corners[j].dot((center(h) - corner(h, j)).normalize_or_zero())).sum::<f32>() / 6.0;
        assert!(inward > 2.0, "rentrée moyenne des coins : {inward}");
    }

    #[test]
    fn une_embouchure_garde_ses_coins_exacts() {
        // (les deltas et les rias n'ouvrent pas `full` sur leur arête de mer : ils comptent aussi)
        for kind in [None, Some(MouthKind::Estuary), Some(MouthKind::BirdFoot), Some(MouthKind::Arcuate), Some(MouthKind::Ria), Some(MouthKind::Fjord)] {
            let (map, h) = embouchure(kind, 4_000.0);
            let (corners, ripple) = coast_warp(&map, h);
            assert_eq!(ripple[0], 0.0, "{kind:?}");
            assert_eq!((corners[0], corners[1]), (Vec2::ZERO, Vec2::ZERO), "{kind:?}");
            // Vu de la case de mer aussi
            let (at_sea, at_sea_ripple) = coast_warp(&map, h + DIRECTIONS[0]);
            assert_eq!(at_sea_ripple[3], 0.0, "{kind:?}");
            assert_eq!((at_sea[3], at_sea[4]), (Vec2::ZERO, Vec2::ZERO), "{kind:?}");
        }
    }

    #[test]
    fn le_bord_de_la_banquise_s_arrondit() {
        let mut map = WorldMap::filled(14, 14, Terrain::Ocean);
        let h = Hex::from_offset(7, 7);
        map.freeze(h);
        let (corners, ripple) = coast_warp(&map, h);
        assert!(corners.iter().any(|c| c.length() > 1.0) && ripple.iter().any(|r| *r != 0.0));
    }

    #[test]
    fn une_riviere_cotiere_suit_le_rivage() {
        let mut map = WorldMap::filled(14, 14, Terrain::Plains);
        let h = Hex::from_offset(7, 7);
        map.set(h + DIRECTIONS[0], Terrain::Ocean);
        let k = corner(h, 0);
        assert_ne!(coast_corner(&map, h, 0), Vec2::ZERO);
        assert_eq!(river_node(&map, k), k + coast_corner(&map, h, 0));
    }

    #[test]
    fn eau_coupee_aux_aretes_de_mer() {
        let h = Hex::from_offset(3, 3);
        // Un triangle à cheval sur l'arête 0 : seule la partie dans la case reste
        let (m, out) = (edge_mid(h, 0), (edge_mid(h, 0) - center(h)).normalize_or_zero());
        let pieces = clip_to_edges([m - out * 5.0, m + out * 5.0 + out.perp() * 4.0, m + out * 5.0 - out.perp() * 4.0], h, 1 << 0);
        assert!(!pieces.is_empty());
        for p in pieces.iter().flatten() {
            assert!((*p - center(h)).dot(out) <= HEX_SIZE * 0.866 + 1e-3);
        }
        assert_eq!(clip_to_edges([m + out, m + out * 3.0, m + out * 2.0 + out.perp()], h, 1 << 0), Vec::<[Vec2; 3]>::new());
    }

    #[test]
    fn un_delta_ne_s_ouvre_que_par_son_arete_aval() {
        let (mut map, h) = embouchure(Some(MouthKind::BirdFoot), 5_000.0);
        // Mer aussi sur l'arête voisine : elle ne s'ouvre pas, le fleuve se divise à l'entrée
        map.set(h + DIRECTIONS[1], Terrain::Ocean);
        assert_eq!(fleuve_openings(&map, h).open, 1 << 0 | 1 << 3);
        map.set_mouth(h, MouthKind::Estuary);
        assert_eq!(fleuve_openings(&map, h).open, 1 << 0 | 1 << 1 | 1 << 3, "un estuaire, si");
    }

    #[test]
    fn pas_de_rapide_dans_une_embouchure() {
        let (mut map, h) = embouchure(None, 5_000.0);
        map.set_elevation(h, 900.0);
        assert!(fleuve_rapids(&map, h) > 0.0, "sans type : la chute vers la mer compte");
        map.set_mouth(h, MouthKind::Ria);
        assert_eq!(fleuve_rapids(&map, h), 0.0);
    }

    #[test]
    fn delta_digite_en_plusieurs_bras() {
        for (q, wanted) in [(2_500.0, 2), (5_000.0, 3), (20_000.0, 4)] {
            let (map, h) = embouchure(Some(MouthKind::BirdFoot), q);
            let arms = delta_arms(&map, h, fleuve_half_width(q));
            assert!(arms.len() >= 2 && arms.len() <= wanted, "{q} m³/s : {} bras", arms.len());
            let shape = hex_water(&map, h);
            let out = (edge_mid(h, 0) - center(h)).normalize_or_zero();
            // Près de la côte : de l'eau à chaque bras, de la terre entre deux bras
            let near = |at: f32| edge_mid(h, 0) - out * 0.5 + out.perp() * at;
            for &(at, w) in &arms {
                assert!(couvert(&shape, near(at)), "{q} m³/s : bras en {at}");
                assert!(at.abs() + w <= HEX_SIZE * 0.5 - 3.0, "{q} m³/s : bras en {at}, demi-largeur {w} : trop près du coin");
            }
            for pair in arms.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                assert!(b.0 - b.1 - (a.0 + a.1) > 4.0, "{q} m³/s : bras collés {a:?} {b:?}");
                assert!(!couvert(&shape, near((a.0 + a.1 + b.0 - b.1) / 2.0)), "{q} m³/s : terre entre les bras");
            }
            assert!(couvert(&shape, center(h)), "{q} m³/s : l'eau couvre le centre");
        }
    }

    #[test]
    fn virage_serre_avant_embouchure() {
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let h = Hex::from_offset(3, 3);
        map.set(h + DIRECTIONS[0], Terrain::Lake);
        // Le fleuve arrive par l'arête 2 et se jette dans le lac par l'arête 0 : l'intérieur du
        // virage (arête 1, de la terre) est rempli d'eau, sans que l'arête s'ouvre
        map.set_fleuve_links(h, 1 << 0 | 1 << 2);
        map.set_discharge(h, 50_000.0);
        let o = fleuve_openings(&map, h);
        assert_eq!(o.open, 1 << 0 | 1 << 2);
        assert_eq!(o.full, 1 << 0);
        assert_eq!(bend_edges(&map, h), vec![1]);
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

    /// Grand fleuve (débit `q`) en ligne droite sur `n` cases d'une plaine, qui descend de
    /// `drop_m` mètres d'une case à la suivante.
    fn fleuve_en_plaine(n: i32, drop_m: f32, q: f32) -> (WorldMap, Vec<Hex>) {
        let mut map = WorldMap::filled(n + 2, 6, Terrain::Plains);
        let row: Vec<Hex> = (1..=n).map(|c| Hex::from_offset(c, 3)).collect();
        for (k, &h) in row.iter().enumerate() {
            map.set_elevation(h, 4_000.0 - drop_m * k as f32);
        }
        for w in row.windows(2) {
            let d = DIRECTIONS.iter().position(|&dd| w[0] + dd == w[1]).unwrap();
            map.link_fleuve(w[0], d, q);
        }
        (map, row)
    }

    /// Îles des cases d'un fleuve : terres sèches entourées d'eau (sur une grille d'une unité, les
    /// zones sèches qui ne touchent pas le bord), et taille de chacune (en mailles).
    fn iles_dessinees(map: &WorldMap, row: &[Hex]) -> Vec<usize> {
        let tris: Vec<[Vec2; 3]> = row.iter().flat_map(|&h| hex_water(map, h).tris.into_iter().map(|t| t.pts)).collect();
        let (lo, hi) = tris.iter().flatten().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
        let (lo, hi) = (lo - Vec2::splat(3.0), hi + Vec2::splat(3.0));
        let (w, h) = ((hi.x - lo.x).ceil() as usize, (hi.y - lo.y).ceil() as usize);
        let mut wet = vec![false; w * h];
        for [a, b, c] in &tris {
            let (tl, tr) = (a.min(*b).min(*c), a.max(*b).max(*c));
            for y in ((tl.y - lo.y).floor() as usize)..=((tr.y - lo.y).ceil() as usize).min(h - 1) {
                for x in ((tl.x - lo.x).floor() as usize)..=((tr.x - lo.x).ceil() as usize).min(w - 1) {
                    let p = lo + Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let (d1, d2, d3) = ((*b - *a).perp_dot(p - *a), (*c - *b).perp_dot(p - *b), (*a - *c).perp_dot(p - *c));
                    if !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0)) {
                        wet[y * w + x] = true;
                    }
                }
            }
        }
        // Zones sèches : remplissage depuis chaque maille sèche pas encore vue
        let mut seen = wet.clone();
        let mut isles = Vec::new();
        for start in 0..w * h {
            if seen[start] {
                continue;
            }
            let (mut stack, mut size, mut border) = (vec![start], 0, false);
            seen[start] = true;
            while let Some(i) = stack.pop() {
                size += 1;
                let (x, y) = (i % w, i / w);
                border |= x == 0 || y == 0 || x == w - 1 || y == h - 1;
                for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                    if nx < w && ny < h && !seen[ny * w + nx] {
                        seen[ny * w + nx] = true;
                        stack.push(ny * w + nx);
                    }
                }
            }
            if !border && size >= 3 {
                isles.push(size);
            }
        }
        isles
    }

    #[test]
    fn iles_dans_un_grand_fleuve_de_plaine() {
        let (map, row) = fleuve_en_plaine(24, 5.0, 30_000.0);
        // Îles tirées : celles des arêtes, une par bras secondaire
        let mut edges = 0;
        let mut sides = 0;
        let mut plain = 0;
        let mut groups = 0;
        let mut sizes: Vec<f32> = Vec::new();
        for &h in &row {
            let Some(dirs) = crossed(&map, h) else { continue };
            let isles = Isles::of(&map, h, dirs);
            for (k, group) in isles.edge.iter().enumerate() {
                // Le même tirage vu des deux cases
                assert_eq!(*group, edge_isles(&map, h + DIRECTIONS[dirs[k]], (dirs[k] + 3) % 6));
                // (comptées par la case qui porte l'arête)
                if dirs[k] < 3 {
                    edges += group.len();
                    groups += usize::from(group.len() > 1);
                    sizes.extend(group.iter().map(|i| i.g));
                }
            }
            sides += usize::from(isles.side.is_some());
            plain += usize::from(!isles.any());
        }
        assert!(edges + sides >= 3, "îles : {edges} à cheval sur une arête, {sides} bras secondaires");
        assert!(plain >= 6, "des îles par endroits seulement ({plain} cases sans île)");
        assert!(groups >= 1, "plusieurs bras par endroits");
        let (small, big) = sizes.iter().fold((f32::MAX, 0.0f32), |(lo, hi), &g| (lo.min(g), hi.max(g)));
        assert!(big > 1.5 * small, "des îles de tailles variées ({small} à {big})");
        // Chacune est dessinée : une terre entourée d'eau, sans autre trou dans le fleuve
        let drawn = iles_dessinees(&map, &row);
        assert_eq!(drawn.len(), edges + sides, "{drawn:?}");
        assert!(drawn.iter().all(|&s| s >= 10), "{drawn:?}");
        // Le centre des cases reste dans l'eau (unités)
        let inside = |t: &WaterTri, p: Vec2| {
            let [a, b, c] = t.pts;
            let (d1, d2, d3) = ((b - a).perp_dot(p - a), (c - b).perp_dot(p - b), (a - c).perp_dot(p - c));
            !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
        };
        for &h in &row {
            assert!(hex_water(&map, h).tris.iter().any(|t| inside(t, center(h))), "{:?}", h.to_offset());
        }
        // Autour d'une île seule à cheval sur une arête, le fleuve s'élargit de la largeur de l'île
        for w in row.windows(2) {
            let d = DIRECTIONS.iter().position(|&dd| w[0] + dd == w[1]).unwrap();
            let [isle] = edge_isles(&map, w[0], d)[..] else { continue };
            if isle.shift.abs() > 0.5 * isle.l {
                continue;
            }
            let x = fleuve_crossing(&map, w[0], d);
            let (a, b) = w[0].edge_corners(d);
            let along = (v(b) - v(a)).normalize();
            let hw = fleuve_edge_half_width(&map, w[0], d);
            let shapes = [hex_water(&map, w[0]), hex_water(&map, w[1])];
            let wet = |p: Vec2| shapes.iter().any(|s| s.tris.iter().any(|t| inside(t, p)));
            assert!(!wet(x), "l'île est au point de passage");
            for side in [1.0, -1.0] {
                assert!(wet(x + along * side * (isle.g + hw)), "bras de part et d'autre, élargi");
            }
        }
        // Les îles sont rangées pour le relief (aucune dans l'eau dessinée) ; des arbres sur les
        // îles de terre, à l'intérieur
        let mut trees = 0;
        for &h in &row {
            let shape = hex_water(&map, h);
            for isle in &shape.isles {
                for &(p, g) in &isle.spine {
                    if g > 1.0 {
                        assert!(!shape.tris.iter().any(|t| inside(t, p)), "milieu d'une île sous l'eau en {p}");
                    }
                }
            }
            for (p, size) in isle_trees(&shape, h) {
                assert!(shape.isles.iter().any(|i| i.top.soil >= ISLE_TREE_SOIL && i.depth(p) >= ISLE_TREE_ROOM * 0.8));
                assert!(Hex::from_world(p.x, p.y) == h && (0.3..=1.0).contains(&size));
                trees += 1;
            }
        }
        assert!(trees > 0, "des arbres sur les grandes îles");
    }

    #[test]
    fn bords_de_l_eau_raccordes_sur_les_aretes() {
        // Cases avec et sans îles (découpées plus ou moins finement), débits variés
        for (map, row) in [fleuve_en_plaine(24, 5.0, 30_000.0), fleuve_rangee(4)] {
            for w in row.windows(2) {
                let d = DIRECTIONS.iter().position(|&dd| w[0] + dd == w[1]).unwrap();
                let (a, b) = w[0].edge_corners(d);
                let (a, b) = (v(a), v(b));
                let along = (b - a).normalize();
                // Points de l'eau posés sur l'arête : position le long de l'arête, la plus basse
                // et la plus haute (les deux bords)
                let span = |h: Hex| {
                    let on: Vec<f32> = hex_water(&map, h).tris.iter().flat_map(|t| t.pts).filter(|p| (*p - a).perp_dot(along).abs() < 1e-3).map(|p| (p - a).dot(along)).collect();
                    on.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)))
                };
                let (here, there) = (span(w[0]), span(w[1]));
                assert!((here.0 - there.0).abs() < 1e-2 && (here.1 - there.1).abs() < 1e-2, "{:?} {here:?} / {:?} {there:?}", w[0].to_offset(), w[1].to_offset());
            }
        }
    }

    #[test]
    fn petite_ile_sans_arbre_grande_ile_de_terre() {
        let (small, big) = (isle_top(ISLE_MIN_HALF_WIDTH, 6.0, 0.5, 0.5), isle_top(7.0, 16.0, 0.5, 0.5));
        assert!(small.soil < ISLE_TREE_SOIL && big.soil >= ISLE_TREE_SOIL);
        assert!(small.lift >= ISLE_LIFT_MIN && big.lift > small.lift);
    }

    #[test]
    fn pas_d_ile_dans_un_fleuve_en_pente() {
        // 150 m d'une case à la suivante : 3,75 m/km
        let (map, row) = fleuve_en_plaine(24, 150.0, 30_000.0);
        assert!(row.iter().all(|&h| crossed(&map, h).is_none_or(|dirs| !Isles::of(&map, h, dirs).any())));
        assert!(iles_dessinees(&map, &row).is_empty());
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
        // Fleuve qui traverse sa case ; une rivière arrive à un coin
        let (mut map, row) = fleuve_rangee(4);
        let h = row[1];
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
    fn panache_d_un_gros_affluent_dans_le_fleuve() {
        // Petit fleuve ; une rivière presque aussi large arrive à chacun de deux coins
        let (mut map, row) = fleuve_rangee(4);
        let h = row[1];
        for j in [1, 4] {
            let (a, b) = (h + DIRECTIONS[(j + 5) % 6], h + DIRECTIONS[j]);
            let dir = DIRECTIONS.iter().position(|&dd| a + dd == b).unwrap();
            map.set_river(a, dir, 1_800.0);
        }
        let shape = hex_water(&map, h);
        let fleuve: Vec<&WaterAxis> = shape.channels.iter().filter(|c| c.kind == WaterKind::Fleuve && c.isle.is_none()).collect();
        // Dans le fleuve, le panache ne dépasse pas de la berge (il s'arrêtait net au-dehors)
        let reach = |p: Vec2| fleuve.iter().map(|c| -segment_depth((c.a, c.hw), (c.b, c.hw_end), p)).fold(f32::MAX, f32::min);
        // (le panache : les tronçons d'un affluent après son arrivée, `STEPS` tronçons depuis son coin)
        let inflow: Vec<&WaterAxis> = shape.channels.iter().filter(|c| c.inflow.is_some()).collect();
        let starts: Vec<usize> = (0..inflow.len()).filter(|&i| [1, 4].iter().any(|&j| inflow[i].a.distance(corner(h, j)) < 1e-3)).collect();
        assert_eq!(starts.len(), 2);
        let plume: Vec<&&WaterAxis> = starts.iter().flat_map(|&s| inflow[s..].iter().take_while(|c| c.inflow.unwrap()[1] <= 1.0).skip(STEPS).take(3)).collect();
        assert!(!plume.is_empty());
        for c in plume.iter().filter(|c| reach(c.b) < -1.0) {
            assert!(reach(c.b) + c.hw_end <= 0.6, "panache hors du fleuve : {} au-delà de la berge", reach(c.b) + c.hw_end);
        }
    }

    #[test]
    fn rapide_compose_dans_le_sens_du_courant() {
        // Ligne d'eau droite vers l'est, en forte pente, là où le bruit des tronçons donne des rapides
        let start = (0..400).map(|k| Vec2::new(k as f32 * 7.3, 11.0)).find(|&p| stretch(p + Vec2::X * 10.0) > 0.9).unwrap();
        let tone = Tone { rapids: 1.0, ..Tone::default() };
        let path: Vec<PathPoint> = (0..=6).map(|k| (start + Vec2::X * 5.0 * k as f32, 4.0, tone)).collect();
        let mut shape = WaterShape::default();
        shape.strip(&path, WaterKind::River);
        shape.rapid(&path, true);
        let of = |kind: WaterKind| shape.tris.iter().filter(move |t| t.kind == kind);
        for kind in [WaterKind::Light, WaterKind::Shade, WaterKind::Foam, WaterKind::Rock] {
            assert!(of(kind).count() > 0, "{kind:?}");
        }
        // L'écume reste dans l'eau (à la largeur près, plus l'épaisseur d'un trait)
        let worst = of(WaterKind::Foam).flat_map(|t| t.pts).map(|p| (p.y - start.y).abs()).fold(0.0f32, f32::max);
        assert!(worst <= 4.0 + 1.0, "écume à {worst} de l'axe");
        // Dans le sens du courant : la tête (premières écumes, le long des rives) en amont du rocher
        let head_and_rock = |shape: &WaterShape| {
            let head = shape.tris.iter().find(|t| t.kind == WaterKind::Foam).unwrap().pts[0].x;
            let rock = shape.tris.iter().find(|t| t.kind == WaterKind::Rock).unwrap().pts[0].x;
            (head, rock)
        };
        let (head, rock) = head_and_rock(&shape);
        assert!(head < rock, "tête {head}, rocher {rock}");
        let mut reversed = WaterShape::default();
        reversed.strip(&path, WaterKind::River);
        reversed.rapid(&path, false);
        let (head, rock) = head_and_rock(&reversed);
        assert!(head > rock, "à l'envers : tête {head}, rocher {rock}");
        // Les éléments sont découpés finement (ils suivent la pente de l'eau)
        let longest = shape.tris.iter().filter(|t| t.kind != WaterKind::River).flat_map(|t| [t.pts[0].distance(t.pts[1]), t.pts[1].distance(t.pts[2]), t.pts[2].distance(t.pts[0])]).fold(0.0f32, f32::max);
        assert!(longest < 4.5, "maille de {longest}");
        // Sans pente, pas de rapide
        let calm: Vec<PathPoint> = path.iter().map(|&(p, h, _)| (p, h, Tone::default())).collect();
        let mut flat = WaterShape::default();
        flat.strip(&calm, WaterKind::River);
        flat.rapid(&calm, true);
        assert!(flat.tris.iter().all(|t| t.kind == WaterKind::River));
    }

    #[test]
    fn teinte_interpolee_dans_un_triangle() {
        let t = WaterTri { pts: [Vec2::ZERO, Vec2::X, Vec2::Y], tones: [Tone { size: 0.0, murk: 0.0, rapids: 0.0 }, Tone { size: 1.0, murk: 0.0, rapids: 0.0 }, Tone { size: 0.0, murk: 1.0, rapids: 0.0 }], kind: WaterKind::River, also: None, channel: 0 };
        let m = t.tone_at(Vec2::new(0.5, 0.25));
        assert!((m.size - 0.5).abs() < 1e-5 && (m.murk - 0.25).abs() < 1e-5);
        // Une eau limoneuse tire vers le brun, un grand fleuve est plus sombre
        let blue = Vec3::new(0.05, 0.2, 0.5);
        assert!(Tone { size: 0.0, murk: 1.0, rapids: 0.0 }.apply(blue).x > blue.x);
        assert!(Tone { size: 1.0, murk: 0.0, rapids: 0.0 }.apply(blue).z < blue.z);
    }
}

#[cfg(test)]
mod diag {
    /// Ouvertures, altitude et voisines d'une case de fleuve (`WATER_SEED`, `WATER_AT=col,lig` ;
    /// `WATER_SPAN=n` : les n cases vers l'est).
    #[test]
    #[ignore]
    fn ouvertures_d_une_case() {
        use super::*;
        let seed: u32 = std::env::var("WATER_SEED").map_or(777, |s| s.parse().unwrap());
        let at: Vec<i32> = std::env::var("WATER_AT").unwrap_or("106,32".into()).split(',').map(|x| x.parse().unwrap()).collect();
        let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let span = std::env::var("WATER_SPAN").map_or(1, |s| s.parse().unwrap());
        let around: Vec<(i32, i32)> = if span > 1 { (0..span).map(|k| (at[0] + k, at[1])).collect() } else { vec![(at[0], at[1]), (at[0], at[1] + 1), (at[0] + 1, at[1]), (at[0] - 1, at[1])] };
        for (c, r) in around {
            let h = Hex::from_offset(c, r);
            let o = fleuve_openings(&m, h);
            println!("{:?} fleuve {} liens {:06b} ouvert {:06b} plein {:06b} aval {:?} q {:.0} alt {:.0}", (c, r), m.is_fleuve(h), m.fleuve_links(h), o.open, o.full, m.downstream(h), m.discharge(h), m.surface_elevation(h).unwrap_or(0.0));
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
    #[test]
    #[ignore]
    fn diag_iles_arbres() {
        use super::*;
        let seed: u32 = std::env::var("WATER_SEED").map_or(3377500742, |s| s.parse().unwrap());
        let m = crate::map::WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        let (mut n, mut soil, mut trees) = (0, 0, 0);
        for h in m.hexes().filter(|&h| m.is_fleuve(h)) {
            let shape = hex_water(&m, h);
            for i in &shape.isles {
                n += 1;
                if i.top.soil >= ISLE_TREE_SOIL { soil += 1; }
                let g = i.spine.iter().map(|x| x.1).fold(0.0, f32::max);
                let len: f32 = i.spine.windows(2).map(|w| w[0].0.distance(w[1].0)).sum();
                println!("{:?} {:?} g {:.1} long {:.0} soil {:.2} lift {:.2}", h.to_offset(), m.get(h), g, len, i.top.soil, i.top.lift);
            }
            trees += isle_trees(&shape, h).len();
        }
        println!("demi-îles {n}, de terre {soil}, arbres {trees}");
    }
}
