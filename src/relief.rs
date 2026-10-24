//! Relief continu de la vue 3D (géométrie pure, sans Bevy hormis les types de maths).
//!
//! Chaque case a un centre plat à sa hauteur ; sa bordure rejoint les cases voisines « liées »
//! (les deux explorées, toutes deux terre ou toutes deux eau). La hauteur d'un coin est la
//! moyenne des cases liées qui s'y rejoignent, celle d'un milieu d'arête la moyenne des deux
//! cases : deux cases voisines ont donc exactement le même profil sur leur arête commune.
//! - Pente praticable : raccord doux, qui commence au bord du plateau central.
//! - Falaise : raccord abrupt, concentré près de l'arête ; la couleur tourne à la roche striée
//!   là où la pente est forte (fondu naturel aux extrémités).
//! - Arête non liée (rivage, inexploré, terrasse) : la case garde sa hauteur et une paroi
//!   verticale comble la différence.
//!
//! La même fonction [`HexTop::sample`] sert à construire le maillage et à poser les objets.

use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use bevy::math::{Vec2, Vec3};

/// Nombre de hauteurs d'eau mémorisées le long de l'axe d'un lit.
pub const CHANNEL_LEVELS: usize = 7;
/// Au bord de l'eau, le sol reste un peu sous la surface sur une petite largeur (le bord de
/// l'eau, droit et net, se voit sans que le maillage du sol ne le crénelle) ; au-delà, la berge
/// remonte jusqu'au terrain, de plus en plus raide.
const WATER_EDGE: f32 = -0.15;
const SHELF: f32 = 1.6;
const BANK_SLOPE: f32 = 0.6;
/// Au-delà du replat, le sol rejoint le terrain d'origine sur cette distance (vallée en pente
/// douce, sans paroi verticale) ; un remblai éventuel (berge plus basse que l'eau) se raccorde
/// plus vite.
const FILL_FADE: f32 = 8.0;
/// Portée d'un lit au-delà de sa demi-largeur (au-delà, il ne touche plus le terrain).
const CHANNEL_REACH: f32 = 12.0;
/// Largeur de la bande de terre humide au-delà du replat.
const BANK_TINT_WIDTH: f32 = 5.0;

/// Lit d'un cours d'eau (coordonnées monde 2D) : segment `a`–`b` (un disque si a = b), demi-
/// largeur de l'eau `hw`, hauteur de l'eau le long de l'axe (`levels`, de a à b) et profondeur
/// du lit sous l'eau (`bed`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    pub a: Vec2,
    pub b: Vec2,
    pub hw: f32,
    /// Demi-largeur au bout `b` (évasement d'une embouchure).
    pub hw_end: f32,
    pub levels: [f32; CHANNEL_LEVELS],
    pub bed: f32,
}

impl Channel {
    /// Position (0 à 1) du point de l'axe le plus proche de `p`.
    fn param(&self, p: Vec2) -> f32 {
        let ab = self.b - self.a;
        let len2 = ab.length_squared();
        if len2 < 1e-6 { 0.0 } else { ((p - self.a).dot(ab) / len2).clamp(0.0, 1.0) }
    }

    /// Point de l'axe le plus proche de `p`.
    pub fn nearest(&self, p: Vec2) -> Vec2 {
        self.a + (self.b - self.a) * self.param(p)
    }

    fn is_disc(&self) -> bool {
        self.a.distance_squared(self.b) < 1e-6
    }

    fn reach(&self) -> f32 {
        self.hw.max(self.hw_end) + SHELF + CHANNEL_REACH
    }

    /// Demi-largeur à la position `t` (0 à 1) le long de l'axe.
    fn half_width(&self, t: f32) -> f32 {
        self.hw + (self.hw_end - self.hw) * t
    }

    /// Hauteur de l'eau sur l'axe, en face de `p`.
    fn axis_water(&self, p: Vec2) -> f32 {
        let x = self.param(p) * (CHANNEL_LEVELS - 1) as f32;
        let k = (x.floor() as usize).min(CHANNEL_LEVELS - 2);
        let f = x - k as f32;
        self.levels[k] * (1.0 - f) + self.levels[k + 1] * f
    }

    /// Profil du lit à la distance `d` de l'axe (demi-largeur `hw` à cet endroit), par rapport à
    /// la surface de l'eau.
    fn profile(&self, d: f32, hw: f32) -> f32 {
        let half = 0.5 * hw;
        if d <= half {
            -self.bed
        } else if d <= hw {
            -self.bed + (self.bed + WATER_EDGE) * smoothstep((d - half) / half)
        } else {
            WATER_EDGE + (d - hw - SHELF).max(0.0) * BANK_SLOPE
        }
    }
}

/// Surface de l'eau en `p` : mélange des hauteurs des axes proches, pondérées par la proximité
/// (poids nul au-delà de la portée d'un lit). Une seule fonction du point pour la surface de
/// l'eau et pour le lit : pas de marche entre deux tronçons, une case et sa voisine, ni au
/// centre d'un fleuve qui tourne. Les disques (coins, centre d'un fleuve, petits lacs) ne
/// comptent que s'il n'y a pas de tronçon à portée.
pub fn water_surface(channels: &[Channel], p: Vec2) -> Option<f32> {
    let blend = |discs: bool| {
        let (mut sw, mut sh) = (0.0f32, 0.0f32);
        for c in channels.iter().filter(|c| c.is_disc() == discs) {
            let d = p.distance(c.nearest(p));
            let r = c.reach();
            if d < r {
                let w = (1.0 - d / r).powi(4);
                sw += w;
                sh += w * c.axis_water(p);
            }
        }
        (sw > 1e-6).then(|| sh / sw)
    };
    blend(false).or_else(|| blend(true))
}


/// Hauteur du sol en `p` une fois les lits taillés dans le terrain (`terrain`), et part de berge
/// (0 à 1 : terre remuée, pour la couleur).
pub fn carve(channels: &[Channel], p: Vec2, terrain: f32) -> (f32, f32) {
    // Un seul passage : forme du lit (le plus profond) et surface de l'eau (mélange pondéré,
    // tronçons d'abord, disques à défaut)
    let mut shape = f32::MAX;
    let mut bank = 0.0f32;
    let (mut cut, mut fill) = (0.0f32, 0.0f32);
    let (mut sw, mut sh, mut dw, mut dh) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for c in channels {
        let t = c.param(p);
        let d = p.distance(c.a + (c.b - c.a) * t);
        let r = c.reach();
        if d >= r {
            continue;
        }
        let hw = c.half_width(t);
        shape = shape.min(c.profile(d, hw));
        // Emprise du lit sur le terrain : totale jusqu'au bout du replat, puis décroissante
        let beyond = (d - hw - SHELF).max(0.0);
        cut = cut.max(1.0 - smoothstep(beyond / CHANNEL_REACH));
        fill = fill.max(1.0 - smoothstep(beyond / FILL_FADE));
        // Terre humide : selon la distance au bord de l'eau (variation douce, sans dents de scie)
        bank = bank.max(1.0 - smoothstep((d - hw) / (SHELF + BANK_TINT_WIDTH)));
        let w = (1.0 - d / r).powi(4);
        let x = t * (CHANNEL_LEVELS - 1) as f32;
        let k = (x.floor() as usize).min(CHANNEL_LEVELS - 2);
        let level = c.levels[k] + (c.levels[k + 1] - c.levels[k]) * (x - k as f32);
        if c.is_disc() {
            dw += w;
            dh += w * level;
        } else {
            sw += w;
            sh += w * level;
        }
    }
    let water = if sw > 1e-6 {
        sh / sw
    } else if dw > 1e-6 {
        dh / dw
    } else {
        return (terrain, 0.0);
    };
    // Le sol prend la forme du lit : creusé là où le terrain est plus haut, remblayé (berge) là
    // où il est plus bas, en se raccordant progressivement au terrain d'origine
    let target = water + shape;
    let weight = if target < terrain { cut } else { fill };
    let h = terrain + (target - terrain) * weight;
    // Berge entaillée : terre aussi, selon la profondeur de l'entaille
    (h, bank.max(smoothstep((terrain - h) / 4.0)))
}

/// Rayon (fraction de l'apothème) du plateau central plat.
pub const FLAT_RADIUS: f32 = 0.45;
/// Début de la descente vers une plage (fraction de l'apothème) : pente longue.
const SHORE_START: f32 = 0.35;
/// Subdivisions de chaque arête dans le maillage.
const EDGE_STEPS: usize = 8;
/// Assombrissement du bord des cases (séparation discrète entre cases).
const EDGE_SHADE: f32 = 0.84;
/// Anneaux du maillage (fractions de l'apothème) ; les deux derniers intervalles forment
/// l'escarpement des falaises.
const RINGS: [f32; 5] = [FLAT_RADIUS, 0.62, 0.78, 0.9, 1.0];
/// Maillage plus fin pour une case traversée ou bordée par un cours d'eau (lit et berges) ;
/// les points du bord des cases ordinaires en font partie (pas de fente entre les deux).
const FINE_RINGS: [f32; 11] = [0.15, 0.3, FLAT_RADIUS, 0.55, 0.62, 0.7, 0.78, 0.85, 0.9, 0.95, 1.0];
const FINE_EDGE_STEPS: usize = 8;
/// Couleur des berges (terre humide, linéaire) et part maximale dans la couleur du sol.
const BANK_MUD: Vec3 = Vec3::new(0.30, 0.23, 0.13);
const BANK_TINT: f32 = 0.45;

const APOTHEM: f32 = HEX_SIZE * 0.866_025_4;

/// Nature du raccord entre deux cases voisines (relation symétrique).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// Paroi verticale : chaque case garde sa hauteur.
    None,
    /// Pente douce.
    Gentle,
    /// Escarpement rocheux.
    Cliff,
    /// Plage : la terre descend en pente longue jusqu'à la mer, bordée de sable.
    Shore,
}

/// Profil du dessus d'une case : centre, coins (coin j à −30° + 60°·j) et milieux d'arêtes
/// (arête d entre les coins d et d+1, vers la voisine `DIRECTIONS[d]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HexTop {
    pub center: f32,
    pub corners: [f32; 6],
    pub mids: [f32; 6],
    pub links: [Link; 6],
    /// Début de la pente (fraction de l'apothème) aux coins et aux milieux d'arêtes ; une
    /// valeur partagée par toutes les cases qui se touchent là, pour des pentes identiques
    /// des deux côtés d'une arête.
    pub start_corners: [f32; 6],
    pub start_mids: [f32; 6],
    /// Pente sur le milieu de chaque arête, en hauteur par longueur de pente : la moitié de
    /// l'écart avec la voisine (même valeur, de signe opposé, vue depuis la voisine) ; nulle
    /// contre une surface fixe (la terre arrive à plat au bord de l'eau) ou une paroi.
    pub edge_slopes: [f32; 6],
    /// Hauteur de la voisine (pied de la paroi d'une arête non liée).
    pub wall_bottoms: [f32; 6],
}

impl HexTop {
    /// L'arête d est raccordée en pente (ni paroi, ni falaise).
    pub fn joined(&self, d: usize) -> bool {
        !matches!(self.links[d % 6], Link::None | Link::Cliff)
    }

    /// L'arête d est le haut d'une falaise.
    pub fn cliff_top(&self, d: usize) -> bool {
        self.links[d % 6] == Link::Cliff && self.center > self.wall_bottoms[d % 6]
    }
}

/// Début de la pente selon le raccord.
fn start_of(link: Link) -> Option<f32> {
    match link {
        Link::None => None,
        Link::Gentle => Some(FLAT_RADIUS),
        Link::Cliff => None,
        Link::Shore => Some(SHORE_START),
    }
}

/// Calcule le profil d'une case à partir de la hauteur affichée de chaque case et de la
/// nature des raccords.
///
/// `level_surface` : cases à surface fixe (l'eau, au niveau de la mer). Un raccord qui touche
/// une telle case se fait à sa hauteur, pas à mi-hauteur : la mer reste plate et c'est la
/// terre qui descend jusqu'à elle (plage, ou falaise côtière).
pub fn compute_top(
    hex: Hex,
    height: impl Fn(Hex) -> f32,
    link: impl Fn(Hex, Hex) -> Link,
    level_surface: impl Fn(Hex) -> bool,
) -> HexTop {
    // Hauteur d'un raccord entre cases liées : celle des surfaces fixes s'il y en a, sinon la moyenne
    let meeting = |group: &[Hex]| {
        let fixed: Vec<f32> = group.iter().filter(|&&g| level_surface(g)).map(|&g| height(g)).collect();
        let pool: Vec<f32> = if fixed.is_empty() { group.iter().map(|&g| height(g)).collect() } else { fixed };
        pool.iter().sum::<f32>() / pool.len() as f32
    };
    let h = height(hex);
    let n = |d: usize| hex + DIRECTIONS[d % 6];
    // Une falaise sépare les cases comme une paroi : chacune garde sa hauteur sur l'arête
    let joined = |a: Hex, b: Hex| !matches!(link(a, b), Link::None | Link::Cliff);
    let mut top = HexTop {
        center: h,
        corners: [h; 6],
        mids: [h; 6],
        links: [Link::None; 6],
        start_corners: [FLAT_RADIUS; 6],
        start_mids: [FLAT_RADIUS; 6],
        edge_slopes: [0.0; 6],
        wall_bottoms: [0.0; 6],
    };
    for d in 0..6 {
        top.links[d] = link(hex, n(d));
        top.wall_bottoms[d] = height(n(d));
        top.start_mids[d] = start_of(top.links[d]).unwrap_or(FLAT_RADIUS);
        if joined(hex, n(d)) {
            top.mids[d] = meeting(&[hex, n(d)]);
            if !level_surface(hex) && !level_surface(n(d)) {
                top.edge_slopes[d] = (height(n(d)) - h) / 2.0;
            }
        }
    }
    for j in 0..6 {
        // Le coin j touche les voisines des directions j−1 et j
        let (a, b) = (n(j + 5), n(j));
        let mut group = vec![hex];
        let (la, lb) = (joined(hex, a), joined(hex, b));
        if la {
            group.push(a);
        }
        if lb {
            group.push(b);
        }
        // Une voisine peut être atteinte en faisant le tour du coin par l'autre
        if la != lb && joined(a, b) {
            group.push(if la { b } else { a });
        }
        // Un coin ne descend au niveau de la mer que si chaque case de terre qui s'y rejoint
        // touche directement l'eau (plage) ; sinon (falaise côtière contournée par une plage
        // voisine) il reste à la hauteur des terres : la falaise ne s'écroule pas dans l'angle.
        let lands: Vec<Hex> = group.iter().copied().filter(|&g| !level_surface(g)).collect();
        let waters: Vec<Hex> = group.iter().copied().filter(|&g| level_surface(g)).collect();
        let beach = lands.iter().all(|&l| waters.iter().any(|&w| joined(l, w)));
        top.corners[j] = if level_surface(hex) || waters.is_empty() || beach {
            meeting(&group)
        } else {
            lands.iter().map(|&g| height(g)).sum::<f32>() / lands.len() as f32
        };
        // Début de pente au coin : moyenne des trois raccords qui s'y rejoignent (même valeur
        // pour les trois cases)
        let starts: Vec<f32> = [link(hex, a), link(hex, b), link(a, b)].into_iter().filter_map(start_of).collect();
        if !starts.is_empty() {
            top.start_corners[j] = starts.iter().sum::<f32>() / starts.len() as f32;
        }
    }
    top
}

/// Interpolation quadratique passant par a (u = 0), m (u = 0,5) et b (u = 1).
fn quadratic(a: f32, m: f32, b: f32, u: f32) -> f32 {
    a * (1.0 - u) * (1.0 - 2.0 * u) + m * 4.0 * u * (1.0 - u) + b * u * (2.0 * u - 1.0)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Secteur (arête) et position le long de l'arête (0 au coin d, 1 au coin d+1) d'une direction.
fn sector(angle_deg: f32) -> (usize, f32) {
    let a = angle_deg.rem_euclid(360.0);
    let d = (((a + 30.0) / 60.0).floor() as usize) % 6;
    let phi = (a - 60.0 * d as f32 + 180.0).rem_euclid(360.0) - 180.0; // −30° à +30°
    (d, (0.5 + APOTHEM * phi.to_radians().tan() / HEX_SIZE).clamp(0.0, 1.0))
}

impl HexTop {
    /// Hauteur du bord de la case sur l'arête d, à la position u.
    fn boundary(&self, d: usize, u: f32) -> f32 {
        // Courbe passant par le coin d, le milieu et le coin d+1 (pas de cassure au milieu)
        quadratic(self.corners[d], self.mids[d], self.corners[(d + 1) % 6], u)
    }

    /// Où commence la pente le long de l'arête d (valeurs partagées aux coins et au milieu).
    fn slope_start(&self, d: usize, u: f32) -> f32 {
        quadratic(self.start_corners[d], self.start_mids[d], self.start_corners[(d + 1) % 6], u)
    }

    /// Hauteur du sol au point `local` (décalage 2D depuis le centre de la case).
    pub fn sample(&self, local: Vec2) -> f32 {
        if local.length_squared() < 1e-6 {
            return self.center;
        }
        let (d, u) = sector(local.y.atan2(local.x).to_degrees());
        let normal = Vec2::from_angle((60.0 * d as f32).to_radians());
        let s = (local.dot(normal) / APOTHEM).clamp(0.0, 1.0); // 0 au centre, 1 sur le bord
        let start = self.slope_start(d, u);
        // Profil cubique : part à plat du plateau, arrive sur l'arête à la hauteur et avec la
        // pente communes aux deux cases (pente qui s'annule aux coins) : ni marche ni pli.
        let t = ((s - start) / (1.0 - start)).clamp(0.0, 1.0);
        let rise = self.boundary(d, u) - self.center;
        let end_slope = self.edge_slopes[d] * 4.0 * u * (1.0 - u);
        let (a2, a3) = (3.0 * rise - end_slope, end_slope - 2.0 * rise);
        self.center + a2 * t * t + a3 * t * t * t
    }
}

/// Points du bord d'une case, dans le sens trigonométrique en partant du coin 0 :
/// (point 2D local, indice d'arête).
fn boundary_points(steps: usize) -> Vec<(Vec2, usize)> {
    let corner = |j: usize| Vec2::from_angle((-30.0 + 60.0 * j as f32).to_radians()) * HEX_SIZE;
    let mut pts = Vec::with_capacity(6 * steps);
    for d in 0..6 {
        let (a, b) = (corner(d), corner(d + 1));
        for k in 0..steps {
            pts.push((a.lerp(b, k as f32 / steps as f32), d));
        }
    }
    pts
}

/// Géométrie d'une case : positions (repère du monde 3D), couleurs, indices.
#[derive(Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    fn vertex(&mut self, p: Vec3, n: Vec3, c: Vec3) -> u32 {
        self.positions.push(p.to_array());
        self.normals.push(n.to_array());
        self.colors.push([c.x, c.y, c.z, 1.0]);
        (self.positions.len() - 1) as u32
    }

    /// Paroi verticale sous le bord entre deux points (descend jusqu'au fond).
    fn wall(&mut self, top_a: Vec3, top_b: Vec3, c: Vec3) {
        let floor = |p: Vec3| Vec3::new(p.x, 0.0, p.z);
        // Normale horizontale, vers l'extérieur de la case
        let n = (floor(top_b) - top_a).cross(top_b - top_a).normalize_or_zero();
        let i = self.vertex(top_a, n, c);
        let j = self.vertex(top_b, n, c);
        let k = self.vertex(floor(top_b), n, c * 0.8);
        let l = self.vertex(floor(top_a), n, c * 0.8);
        self.indices.extend([i, k, j, i, l, k]);
    }

    /// Bord vertical d'une étendue d'eau, éclairé comme la surface.
    fn water_skirt(&mut self, top_a: Vec3, top_b: Vec3, c: Vec3) {
        let floor = |p: Vec3| Vec3::new(p.x, 0.0, p.z);
        let i = self.vertex(top_a, Vec3::Y, c);
        let j = self.vertex(top_b, Vec3::Y, c);
        let k = self.vertex(floor(top_b), Vec3::Y, c * 0.8);
        let l = self.vertex(floor(top_a), Vec3::Y, c * 0.8);
        self.indices.extend([i, k, j, i, l, k]);
    }

    /// Paroi de falaise : roche striée en facettes, rangées intermédiaires en saillie aléatoire,
    /// jusqu'au pied (`bottom`), puis prolongée jusqu'au fond.
    fn rock_wall(&mut self, top_a: Vec3, top_b: Vec3, bottom: f32) {
        const ROWS: usize = 4;
        let outward = (Vec3::new(top_b.x, 0.0, top_b.z) - top_a).cross(top_b - top_a).normalize_or_zero();
        let bottom = bottom.min(top_a.y.min(top_b.y));
        let point = |top: Vec3, row: usize| {
            let f = row as f32 / ROWS as f32;
            let mut p = Vec3::new(top.x, top.y + (bottom - top.y) * f, top.z);
            if row > 0 && row < ROWS {
                p += outward * (hash01(p) * ROCK_JUT);
            }
            p
        };
        let mut facet = |a: Vec3, b: Vec3, c: Vec3| {
            // Triangle à normale propre (facettes), sommets non partagés
            let n = (b - a).cross(c - a).normalize_or_zero();
            for p in [a, b, c] {
                let shade = rock_color(p.y) * (0.9 + 0.2 * hash01(p * 0.37));
                let k = self.vertex(p, n, shade);
                self.indices.push(k);
            }
        };
        for row in 0..ROWS {
            let (a0, b0, a1, b1) = (point(top_a, row), point(top_b, row), point(top_a, row + 1), point(top_b, row + 1));
            facet(a0, a1, b1);
            facet(a0, b1, b0);
        }
        // Sous le pied : simple prolongement jusqu'au fond (caché par la case basse)
        let (a, b) = (point(top_a, ROWS), point(top_b, ROWS));
        self.wall(a, b, rock_color(bottom) * 0.6);
    }
}

/// Liseré clair au sommet des falaises (linéaire).
const CREST: Vec3 = Vec3::new(0.52, 0.48, 0.42);
/// Saillie maximale des rochers d'une paroi de falaise.
const ROCK_JUT: f32 = 1.8;

/// Bruit déterministe dans [0, 1] selon une position (même valeur pour un sommet partagé).
fn hash01(p: Vec3) -> f32 {
    let k = (p.x * 12.9898 + p.y * 78.233 + p.z * 37.719).sin() * 43_758.547;
    k - k.floor()
}

/// Couleur de la roche (sRGB linéarisé approximatif), avec des strates horizontales.
const ROCK: Vec3 = Vec3::new(0.30, 0.26, 0.22);

fn rock_color(y: f32) -> Vec3 {
    let band = (y * 0.55).sin() * 0.5 + 0.5;
    let fine = (y * 2.3 + 1.7).sin() * 0.5 + 0.5;
    ROCK * (0.72 + 0.2 * band + 0.08 * fine)
}

/// Part de roche selon la pente (tangente de l'angle) : rien sous ~45°, roche franche au-delà de ~65°.
fn rock_weight(slope: f32) -> f32 {
    smoothstep((slope - 1.3) / 1.2)
}

/// Sable des plages (linéaire).
const SAND: Vec3 = Vec3::new(0.56, 0.48, 0.28);
/// Hauteur (au-dessus de la mer) jusqu'où monte le sable d'une plage.
const SAND_HEIGHT: f32 = 6.5;
/// Largeur de la bande de sable depuis le rivage.
const SAND_BAND: f32 = APOTHEM * 0.42;

/// Distance d'un point à un segment.
fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

impl HexTop {
    /// Gradient du sol, mesuré juste à l'intérieur de la case (au bord, la pente est la même
    /// des deux côtés : les deux cases calculent donc la même normale).
    fn gradient(&self, local: Vec2) -> Vec2 {
        let e = 0.4;
        let p = if local.length() > 1.0 { local - local.normalize() * (e + 0.2) } else { local };
        let dx = self.sample(p + Vec2::X * e) - self.sample(p - Vec2::X * e);
        let dy = self.sample(p + Vec2::Y * e) - self.sample(p - Vec2::Y * e);
        Vec2::new(dx, dy) / (2.0 * e)
    }

    /// Normale du sol en 3D (repère du monde : x, hauteur, −y).
    #[cfg(test)]
    fn normal(&self, local: Vec2) -> Vec3 {
        let g = self.gradient(local);
        Vec3::new(-g.x, 1.0, g.y).normalize()
    }
}

/// Dessus de la case, avec parois sous les arêtes non liées. Les couleurs sont portées par les
/// sommets : couleur du terrain (`color`, linéaire), qui tourne à la roche striée là où la pente
/// devient abrupte (falaises) ; bord de case légèrement assombri. Les cours d'eau proches
/// (`channels`, ceux de la case et de ses voisines : une arête est creusée pareil des deux côtés)
/// creusent leur lit, bordé de berges de terre humide ; le maillage est alors plus fin.
pub fn build_hex(hex: Hex, top: &HexTop, color: Vec3, sides: Option<&[Vec3; 6]>, channels: &[Channel], skirt: bool) -> MeshData {
    let (cx, cy) = hex.to_world();
    let origin = Vec2::new(cx, cy);
    let world = |p: Vec2, y: f32| Vec3::new(cx + p.x, y, -(cy + p.y));
    let fine = !channels.is_empty();
    // Rivages de la case (côté terre) : arête, hauteur de l'eau
    let corner = |j: usize| Vec2::from_angle((-30.0 + 60.0 * j as f32).to_radians()) * HEX_SIZE;
    let shores: Vec<(Vec2, Vec2, f32)> = (0..6)
        .filter(|&d| top.links[d] == Link::Shore && top.center > top.wall_bottoms[d] + 0.5)
        .map(|d| (corner(d), corner(d + 1), top.wall_bottoms[d]))
        .collect();
    let dig = |q: Vec2| {
        let terrain = top.sample(q);
        if fine { carve(channels, origin + q, terrain) } else { (terrain, 0.0) }
    };
    let height = |q: Vec2| dig(q).0;
    // Normale : gradient du terrain (`g`, mesuré comme `HexTop::gradient`), corrigé du creusement
    // là où le sol est entaillé
    let normal = |q: Vec2, g: Vec2, bank: f32| {
        let g = if bank > 0.0 {
            // (écart assez large pour lisser les angles du profil du lit)
            let e = 1.0;
            let cut = |v: Vec2| top.sample(v) - dig(v).0;
            let dd = |v: Vec2| cut(q + v) - cut(q - v);
            g - Vec2::new(dd(Vec2::X * e), dd(Vec2::Y * e)) / (2.0 * e)
        } else {
            g
        };
        Vec3::new(-g.x, 1.0, g.y).normalize()
    };
    // Couleur du sol : celle de la case, ou (case de fleuve) celle de la voisine de chaque côté,
    // les berges prolongeant ainsi les cases voisines
    let base = |q: Vec2| {
        let Some(sides) = sides else { return color };
        let len = q.length();
        if len < 1e-3 {
            return color;
        }
        let (mut sum, mut total) = (Vec3::ZERO, 0.0f32);
        for (d, &c) in sides.iter().enumerate() {
            let w = (q / len).dot(Vec2::from_angle((60.0 * d as f32).to_radians())).max(0.0).powi(6);
            sum += c * w;
            total += w;
        }
        let around = if total > 0.0 { sum / total } else { color };
        color.lerp(around, smoothstep((len / APOTHEM - 0.2) / 0.5))
    };
    let shade = |q: Vec2, y: f32, bank: f32, slope: f32, edge: bool| {
        let mut c = base(q);
        if bank > 0.0 {
            c = c.lerp(BANK_MUD, BANK_TINT * bank);
        }
        // Plage : une bande de sable le long des rivages (mer ou lac), d'autant plus marquée
        // qu'on est près de l'eau et peu au-dessus d'elle ; la distance au rivage est continue,
        // la plage suit donc la côte sans former d'étoile aux coins des cases
        let sand = shores
            .iter()
            .map(|&(a, b, water)| {
                let near = 1.0 - smoothstep(distance_to_segment(q, a, b) / SAND_BAND);
                near * (1.0 - smoothstep((y - water) / SAND_HEIGHT))
            })
            .fold(0.0f32, f32::max);
        if sand > 0.0 {
            c = c.lerp(SAND, sand);
        }
        let (d, _) = sector(q.y.atan2(q.x).to_degrees());
        c = c.lerp(rock_color(y), rock_weight(slope));
        // Liseré clair au sommet d'une falaise
        let s = q.dot(Vec2::from_angle((60.0 * d as f32).to_radians())) / APOTHEM;
        if top.cliff_top(d) && s > 0.85 {
            return c.lerp(CREST, smoothstep((s - 0.85) / 0.15));
        }
        if edge { c * EDGE_SHADE } else { c }
    };
    let mut mesh = MeshData::default();

    let border = boundary_points(if fine { FINE_EDGE_STEPS } else { EDGE_STEPS });
    let rings: &[f32] = if fine { &FINE_RINGS } else { &RINGS };
    let n = border.len() as u32;
    let vertex = |mesh: &mut MeshData, q: Vec2, edge: bool| {
        let (y, bank) = dig(q);
        let g = top.gradient(q);
        mesh.vertex(world(q, y), normal(q, g, bank), shade(q, y, bank, g.length(), edge))
    };
    let center = vertex(&mut mesh, Vec2::ZERO, false);
    let mut ring_start = Vec::new();
    for &r in rings {
        ring_start.push(mesh.positions.len() as u32);
        for &(p, _) in &border {
            vertex(&mut mesh, p * r, r >= 1.0);
        }
    }
    // Plateau central en éventail, puis bandes entre anneaux (sens trigonométrique vu du dessus)
    for i in 0..n {
        let (a, b) = (ring_start[0] + i, ring_start[0] + (i + 1) % n);
        mesh.indices.extend([center, a, b]);
    }
    for w in ring_start.windows(2) {
        for i in 0..n {
            let (a0, b0) = (w[0] + i, w[0] + (i + 1) % n);
            let (a1, b1) = (w[1] + i, w[1] + (i + 1) % n);
            mesh.indices.extend([a0, b1, b0, a0, a1, b1]);
        }
    }
    // Parois sous les arêtes non liées : roche striée et irrégulière en haut d'une falaise,
    // simple talus de la couleur du terrain ailleurs (rivage, bord de l'inexploré, terrasse)
    for (i, &(p, d)) in border.iter().enumerate() {
        // Un rivage garde aussi un talus : si un coin reste en hauteur (falaise voisine),
        // il ferme l'espace au-dessus de l'eau
        let shore_skirt = top.links[d] == Link::Shore && top.center > top.wall_bottoms[d];
        if top.joined(d) && !shore_skirt && !skirt {
            continue;
        }
        let q = border[(i + 1) % border.len()].0;
        let (ta, tb) = (world(p, height(p)), world(q, height(q)));
        if skirt && top.joined(d) {
            // Bord d'une étendue d'eau : vu dans l'entaille d'une embouchure, il doit avoir l'air
            // d'eau (même couleur, éclairé comme la surface)
            mesh.water_skirt(ta, tb, color);
        } else if top.cliff_top(d) {
            mesh.rock_wall(ta, tb, top.wall_bottoms[d]);
        } else {
            mesh.wall(ta, tb, color * EDGE_SHADE);
        }
    }
    mesh
}

/// Montagne en relief (repère local : origine au sol, au centre de la case) : un sommet
/// principal élancé, des sommets secondaires, des crêtes vers les cases de montagne voisines
/// (`ridges[d]` : la voisine de direction d est une montagne) pour former des chaînes,
/// facettes de roche sombre et neige au-dessus d'une certaine altitude.
/// `seed` varie la forme d'une case à l'autre.
pub fn mountain_mesh(seed: u32, ridges: [bool; 6]) -> MeshData {
    const RINGS: [f32; 10] = [0.0, 0.08, 0.17, 0.27, 0.38, 0.5, 0.62, 0.75, 0.88, 1.0];
    const SECTORS: usize = 24;
    let radius = HEX_SIZE * 1.02;
    let rnd = |k: u32| {
        let x = ((seed.wrapping_mul(2_654_435_761) ^ k.wrapping_mul(40_503)) % 10_007) as f32 / 10_007.0;
        (x * 12.9898 + k as f32 * 4.1414).sin().abs().fract()
    };
    let dir = |d: usize| Vec2::from_angle((60.0 * d as f32).to_radians());
    // Sommets : (position, hauteur, rayon d'influence)
    let main_h = 42.0 + rnd(1) * 14.0;
    let mut peaks = vec![(Vec2::new(rnd(2) - 0.5, rnd(3) - 0.5) * 7.0, main_h, radius * 0.8)];
    let subs = 2 + (rnd(4) * 2.0) as u32;
    for k in 0..subs {
        let angle = (rnd(10 + k) + k as f32 / subs as f32) * std::f32::consts::TAU;
        let at = Vec2::from_angle(angle) * radius * (0.35 + rnd(20 + k) * 0.2);
        peaks.push((at, main_h * (0.45 + rnd(30 + k) * 0.25), radius * (0.4 + rnd(40 + k) * 0.15)));
    }
    // Crêtes vers les montagnes voisines : un col à mi-hauteur sur l'arête commune
    for d in (0..6).filter(|&d| ridges[d]) {
        peaks.push((dir(d) * APOTHEM * 0.75, main_h * (0.42 + rnd(60 + d as u32) * 0.12), radius * 0.55));
    }
    let height = |p: Vec2| {
        let r = p.length() / radius;
        let mut h = peaks
            .iter()
            .map(|&(c, ph, pr)| ph * (1.0 - (p - c).length() / pr).max(0.0).powf(1.5))
            .fold(0.0_f32, f32::max);
        // Arêtes irrégulières, plus marquées en altitude (sauf au sommet même)
        if r > 0.04 {
            h += (hash01(Vec3::new(p.x, seed as f32 * 0.01, p.y)) - 0.5) * (2.0 + 5.0 * h / main_h);
        }
        // Le pied s'enfonce dans le sol, sauf du côté des crêtes qui rejoignent une voisine
        let toward = if r > 0.0 { p / p.length() } else { Vec2::X };
        let ridge = (0..6).filter(|&d| ridges[d]).map(|d| toward.dot(dir(d)).max(0.0).powi(6)).fold(0.0, f32::max);
        let edge = smoothstep((r - 0.72) / 0.28) * (1.0 - ridge);
        h * (1.0 - edge) - 12.0 * edge
    };
    let point = |ring: usize, sector: usize| {
        let angle = (sector as f32 + if ring % 2 == 1 { 0.5 } else { 0.0 }) / SECTORS as f32 * std::f32::consts::TAU;
        let p = Vec2::from_angle(angle + rnd(50) * 6.0) * radius * RINGS[ring];
        Vec3::new(p.x, height(p), -p.y)
    };
    let mut mesh = MeshData::default();
    let snow_line = main_h * 0.55;
    let mut facet = |a: Vec3, b: Vec3, c: Vec3| {
        let mut n = (b - a).cross(c - a).normalize_or_zero();
        let (a, c) = if n.y < 0.0 {
            n = -n;
            (c, a)
        } else {
            (a, c)
        };
        let mid = (a + b + c) / 3.0;
        let jitter = 0.85 + 0.3 * hash01(mid * 0.53);
        // Neige sur les faces hautes et pas trop raides ; roche sombre, plus brune au pied
        let color = if mid.y > snow_line + (hash01(mid) - 0.5) * 8.0 && n.y > 0.3 {
            Vec3::new(0.92, 0.94, 0.98) * (0.96 + 0.04 * jitter)
        } else {
            let low = Vec3::new(0.15, 0.13, 0.11);
            let high = Vec3::new(0.22, 0.21, 0.21);
            low.lerp(high, smoothstep(mid.y / snow_line)) * jitter
        };
        for p in [a, b, c] {
            let k = mesh.vertex(p, n, color);
            mesh.indices.push(k);
        }
    };
    for ring in 0..RINGS.len() - 1 {
        for s in 0..SECTORS {
            let (a0, b0) = (point(ring, s), point(ring, (s + 1) % SECTORS));
            let (a1, b1) = (point(ring + 1, s), point(ring + 1, (s + 1) % SECTORS));
            facet(a0, a1, b1);
            if ring > 0 {
                facet(a0, b1, b0);
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn tops(heights: &HashMap<Hex, f32>, link: impl Fn(Hex, Hex) -> Link + Copy) -> HashMap<Hex, HexTop> {
        let h = |x: Hex| *heights.get(&x).unwrap_or(&0.0);
        heights.keys().map(|&x| (x, compute_top(x, h, link, |_| false))).collect()
    }

    fn world_sample(tops: &HashMap<Hex, HexTop>, hex: Hex, p: Vec2) -> f32 {
        let (cx, cy) = hex.to_world();
        tops[&hex].sample(p - Vec2::new(cx, cy))
    }

    /// Un lit creusé le long d'une arête : même creux vu des deux cases, sol abaissé au milieu,
    /// intact loin des berges.
    #[test]
    fn lit_creuse_le_long_d_une_arete() {
        let a = Hex::new(0, 0);
        let b = a + DIRECTIONS[0];
        let ((ax, ay), (bx, by)) = a.edge_corners(0);
        let ch = [Channel { a: Vec2::new(ax, ay), b: Vec2::new(bx, by), hw: 3.0, hw_end: 3.0, levels: [9.0; CHANNEL_LEVELS], bed: 1.0 }];
        let flat = compute_top(a, |_| 10.0, |_, _| Link::Gentle, |_| false);
        let (ma, mb) = (build_hex(a, &flat, Vec3::ONE, None, &ch, false), build_hex(b, &flat, Vec3::ONE, None, &ch, false));
        let mid = Vec3::new((ax + bx) / 2.0, 0.0, -(ay + by) / 2.0);
        let at = |m: &MeshData| {
            m.positions.iter().map(|p| Vec3::from_array(*p)).filter(|p| (p.x - mid.x).abs() < 0.01 && (p.z - mid.z).abs() < 0.01).map(|p| p.y).next()
        };
        assert_eq!(at(&ma), Some(8.0), "fond du lit au milieu de l'arête");
        assert_eq!(at(&ma), at(&mb), "même creux des deux côtés");
        let (cx, _) = a.to_world();
        let center = ma.positions.iter().find(|p| (p[0] - cx).abs() < 0.01 && p[2].abs() < 0.01).unwrap();
        assert_eq!(center[1], 10.0, "le centre de la case n'est pas touché");
        assert!(ma.positions.len() > build_hex(a, &flat, Vec3::ONE, None, &[], false).positions.len(), "maillage plus fin");
    }

    #[test]
    fn raccord_sans_trou_entre_cases_liees() {
        let a = Hex::new(0, 0);
        let mut heights = HashMap::new();
        for (i, h) in a.range(2).enumerate() {
            heights.insert(h, 10.0 + (i % 4) as f32 * 6.0);
        }
        let link = |_: Hex, _: Hex| Link::Gentle;
        let t = tops(&heights, link);
        for (d, dir) in DIRECTIONS.iter().enumerate() {
            let b = a + *dir;
            let ((ax, ay), (bx, by)) = a.edge_corners(d);
            for k in 0..=8 {
                let p = Vec2::new(ax, ay).lerp(Vec2::new(bx, by), k as f32 / 8.0);
                let (ya, yb) = (world_sample(&t, a, p), world_sample(&t, b, p));
                assert!((ya - yb).abs() < 1e-3, "arête {d}, point {k} : {ya} ≠ {yb}");
                // Même pente des deux côtés : normales quasi identiques (pas de pli à la limite)
                let normal = |h: Hex| {
                    let (cx, cy) = h.to_world();
                    t[&h].normal(p - Vec2::new(cx, cy))
                };
                let angle = normal(a).angle_between(normal(b)).to_degrees();
                if (1..8).contains(&k) {
                    assert!(angle < 6.0, "arête {d}, point {k} : pli de {angle:.1}°");
                }
            }
        }
    }

    #[test]
    fn falaise_abrupte_et_paroi_conservee() {
        let a = Hex::new(0, 0);
        let mut heights: HashMap<Hex, f32> = a.range(1).map(|h| (h, 10.0)).collect();
        let (b, c) = (a + DIRECTIONS[0], a + DIRECTIONS[3]);
        heights.insert(b, 40.0);
        heights.insert(c, 40.0);
        // a–b : falaise ; a–c : paroi (terrasse) ; le reste : pentes douces
        let link = move |x: Hex, y: Hex| {
            if x == c || y == c {
                Link::None
            } else if x == b || y == b {
                Link::Cliff
            } else {
                Link::Gentle
            }
        };
        let t = tops(&heights, link);
        let top = &t[&a];
        assert_eq!(top.sample(Vec2::new(5.0, 3.0)), 10.0, "plateau central");
        let toward = |deg: f32, s: f32| Vec2::from_angle(deg.to_radians()) * APOTHEM * s;
        assert_eq!(top.sample(toward(0.0, 1.0)), 10.0, "pied de falaise : la case basse garde sa hauteur");
        assert_eq!(top.sample(toward(180.0, 1.0)), 10.0, "paroi : la case garde sa hauteur");
        let high = &t[&b];
        assert_eq!(high.sample(toward(180.0, 1.0)), 40.0, "haut de falaise : la case haute garde sa hauteur");
        assert!(high.cliff_top(3) && !top.cliff_top(0));
        // La case haute porte une paroi rocheuse ; la case basse n'a pas de roche
        let rocky = |m: &MeshData| m.colors.iter().filter(|c| (c[0] - ROCK.x).abs() < 0.12 && c[1] < c[0]).count();
        let green = Vec3::new(0.1, 0.5, 0.1);
        assert!(rocky(&build_hex(b, high, green, None, &[], false)) >= 24, "paroi de falaise en roche");
        assert_eq!(rocky(&build_hex(a, top, green, None, &[], false)), 0);
    }

    #[test]
    fn faces_tournees_vers_le_haut() {
        let top = HexTop {
            center: 10.0,
            corners: [12.0; 6],
            mids: [11.0; 6],
            links: [Link::Gentle; 6],
            start_corners: [FLAT_RADIUS; 6],
            start_mids: [FLAT_RADIUS; 6],
            edge_slopes: [0.0; 6],
            wall_bottoms: [0.0; 6],
        };
        let ground = build_hex(Hex::new(0, 0), &top, Vec3::ONE, None, &[], false);
        let normal = |m: &MeshData, t: usize| {
            let p = |k: usize| Vec3::from_array(m.positions[m.indices[t * 3 + k] as usize]);
            (p(1) - p(0)).cross(p(2) - p(0))
        };
        for t in 0..ground.indices.len() / 3 {
            assert!(normal(&ground, t).y > 0.0, "triangle {t} retourné");
        }
        // Paroi d'une arête non liée : tournée vers l'extérieur
        let walled = HexTop { links: [Link::Gentle, Link::None, Link::Gentle, Link::Gentle, Link::Gentle, Link::Gentle], ..top };
        let ground = build_hex(Hex::new(0, 0), &walled, Vec3::ONE, None, &[], false);
        let last = ground.indices.len() / 3 - 1;
        let out = Vec3::new(60f32.to_radians().cos(), 0.0, -60f32.to_radians().sin());
        assert!(normal(&ground, last).dot(out) > 0.0, "paroi tournée vers l'intérieur");
    }

    #[test]
    fn la_mer_reste_plate_et_la_cote_descend() {
        let land = Hex::new(0, 0);
        let mut heights: HashMap<Hex, f32> = land.range(2).map(|h| (h, 10.0)).collect();
        heights.insert(land, 24.0);
        let sea = |h: Hex| h != land; // tout le reste est de l'eau au niveau 10
        let h = |x: Hex| heights[&x];
        let all = |_: Hex, _: Hex| Link::Gentle;
        let top = compute_top(land, h, all, sea);
        assert!(top.mids.iter().chain(&top.corners).all(|&y| y == 10.0), "le rivage est au niveau de la mer");
        let water = compute_top(land + DIRECTIONS[0], h, all, sea);
        assert!(water.mids.iter().chain(&water.corners).all(|&y| y == 10.0), "la mer reste plate");
        assert_eq!(top.sample(Vec2::ZERO), 24.0, "le centre de la case de terre garde sa hauteur");
    }

    #[test]
    fn montagne_avec_sommets_et_neige() {
        for seed in 0..5 {
            let m = mountain_mesh(seed, [seed % 2 == 0, false, true, false, false, false]);
            let top = m.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            assert!(top > 30.0, "sommet trop bas : {top}");
            let snow = m.colors.iter().filter(|c| c[0] > 0.7).count();
            assert!(snow > 0, "neige au sommet");
            assert!(m.normals.iter().all(|n| n[1] >= 0.0), "faces tournées vers le haut");
        }
        assert_ne!(mountain_mesh(1, [false; 6]).positions, mountain_mesh(2, [false; 6]).positions, "formes variées");
    }

    #[test]
    fn falaise_cotiere_ne_tombe_pas_dans_l_angle() {
        // Plateau X (40) séparé de la mer W par une falaise ; plage basse L (16) voisine des deux
        let x = Hex::new(0, 0);
        let (w, l) = (x + DIRECTIONS[0], x + DIRECTIONS[1]); // coin 1 entre X, W et L
        let height = |h: Hex| if h == x { 40.0 } else if h == l { 16.0 } else { 8.0 };
        let water = move |h: Hex| h != x && h != l;
        let link = move |a: Hex, b: Hex| {
            let pair = |p: Hex, q: Hex| (a == p && b == q) || (a == q && b == p);
            if pair(x, w) {
                Link::Cliff
            } else if pair(x, l) || (water(a) && water(b)) {
                Link::Gentle
            } else {
                Link::Shore
            }
        };
        let top_x = compute_top(x, height, link, water);
        let top_l = compute_top(l, height, link, water);
        assert!(top_x.corners[1] > 20.0, "le coin du plateau reste en hauteur : {}", top_x.corners[1]);
        // Coin partagé avec la plage voisine (coin 5 de L : X et W y sont ses voisines 4 et 5)
        assert_eq!(top_x.corners[1], top_l.corners[5], "même hauteur des deux côtés, pas de trou");
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    #[test]
    #[ignore]
    fn cout_des_cases() {
        let a = Hex::new(0, 0);
        let flat = compute_top(a, |h| 10.0 + (h.q * 3 + h.r * 5) as f32, |_, _| Link::Gentle, |_| false);
        let ((ax, ay), (bx, by)) = a.edge_corners(0);
        let mut ch = Vec::new();
        for k in 0..10 {
            let o = Vec2::new(k as f32 * 3.0, 0.0);
            ch.push(Channel { a: Vec2::new(ax, ay) + o, b: Vec2::new(bx, by) + o, hw: 3.0, hw_end: 3.0, levels: [9.0; CHANNEL_LEVELS], bed: 1.0 });
        }
        let t = std::time::Instant::now();
        for _ in 0..200 { std::hint::black_box(compute_top(a, |h| 10.0 + (h.q * 3 + h.r * 5) as f32, |_, _| Link::Gentle, |_| false)); }
        println!("compute_top : {:.3} ms", t.elapsed().as_secs_f64() * 1000.0 / 200.0);
        let t = std::time::Instant::now();
        for _ in 0..200 { std::hint::black_box(build_hex(a, &flat, Vec3::ONE, None, &[], false)); }
        println!("case simple : {:.3} ms", t.elapsed().as_secs_f64() * 1000.0 / 200.0);
        let t = std::time::Instant::now();
        for _ in 0..50 { std::hint::black_box(build_hex(a, &flat, Vec3::ONE, None, &ch, false)); }
        println!("case fine (10 lits) : {:.3} ms", t.elapsed().as_secs_f64() * 1000.0 / 50.0);
    }
}
