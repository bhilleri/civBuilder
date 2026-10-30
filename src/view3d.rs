//! Vue 3D optionnelle (touche V ou bouton) : même partie, rendue en relief.
//!
//! La vue 2D reste la référence. La 3D est un second « monde visuel » sur la couche de rendu 1,
//! vu par une caméra perspective inclinée ; il reflète l'état du jeu (cases, brouillard, unités,
//! villes, villages, camps, sélection, trajets) sans rien changer à la logique.
//! Repère 3D : x = x 2D, y = hauteur, z = −y 2D (le nord est au fond de l'écran).

use crate::game::{City, GameMap, ShownFog, HexPos, Selection, Territory, Unit};
use crate::hex::{DIRECTIONS, HEX_SIZE, Hex, Wrap};
use crate::hostile::{Camp, Hostile, Village};
use crate::input::{HoverPath, OrderPath};
use crate::map::{Relief as CellRelief, Terrain};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use crate::render::{MapLayer, ReachArea, climate_color, current_arrow, currents_color, elevation_layer_color, hydro_color, plate_arrow, plate_color, wind_arrow, wind_arrow_site};
use crate::units::UnitKind;
use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use crate::relief::{
    BumpMap, Bumps, CHANNEL_LEVELS, Channel, HexTop, Link, MeshData, build_hex, carve, compute_top, lift_at, valley_fade, water_surface,
};
use bevy::mesh::Indices;
use rayon::prelude::*;
use crate::waterways::{WaterAxis, WaterKind, WaterShape, hex_water};

/// Couche de rendu du monde 3D (la 2D reste sur la couche 0).
pub const LAYER_3D: usize = 1;
/// Atténuation des couleurs de terrain en 3D (l'éclairage les rend plus vives qu'en 2D).
const TERRAIN_TINT: f32 = 0.8;
/// Inclinaison de la caméra (angle au-dessus de l'horizon).
const PITCH_DEG: f32 = 55.0;

/// Vue active.
#[derive(Resource, Default)]
pub struct ViewMode {
    pub is_3d: bool,
}

/// Caméra 3D : point visé au sol, distance et orientation autour de la verticale.
#[derive(Resource)]
pub struct Cam3d {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
}

/// Sol d'une case (maillage propre à la case, reconstruit quand le relief alentour change).
#[derive(Component)]
struct Tile3d;


/// Décor d'une case (arbres, bosses, pic) : posé sur le relief, visible une fois la case explorée.
#[derive(Component)]
struct Decor3d {
    hex: Hex,
    /// Position par rapport au centre de la case (repère 2D).
    offset: Vec2,
    /// Hauteur du point d'ancrage au-dessus du sol.
    lift: f32,
    /// Autre case qui doit aussi être explorée (rochers au pied d'une falaise).
    requires: Option<Hex>,
    /// Végétation et bosses de collines : masquées sur la carte topographique.
    vegetation: bool,
}

/// Relief 3D courant : profil de chaque case (voir `relief.rs`) et entités qui l'affichent.
#[derive(Resource)]
pub struct Relief {
    tops: HashMap<Hex, HexTop>,
    tiles: HashMap<Hex, Entity>,
    decor: HashMap<Hex, Vec<Entity>>,
    /// Cours d'eau de chaque case qui en a (forme 2D, entité qui l'affiche posée sur le relief).
    water: HashMap<Hex, (WaterShape, Entity)>,
    /// Lits creusés qui touchent chaque case (les siens et ceux de ses voisines), visibles.
    channels: HashMap<Hex, Vec<Channel>>,
    /// Cases d'eau (mer, lacs) : un cours d'eau qui s'y jette arrive à leur niveau ; celles qui
    /// sont explorées (leur relief affiché est connu).
    water_hexes: HashSet<Hex>,
    water_seen: HashSet<Hex>,
    /// Couleur (linéaire) de chaque case d'eau, pour fondre les embouchures dans leur eau.
    water_colors: HashMap<Hex, Vec3>,
    /// Lits déjà calculés (case, indice du tronçon) ; oubliés quand le relief alentour change.
    axis_cache: HashMap<Hex, Vec<Channel>>,
    /// Cases explorées : le relief des autres est inconnu (plat, provisoire) et ne doit pas
    /// guider la hauteur de l'eau.
    explored: HashSet<Hex>,
    /// Hachures posées sur le relief (cases partagées entre deux bassins, carte des bassins).
    hatches: HashMap<Hex, Entity>,
    /// Relief interne (collines, montagnes) des cases explorées, modelé dans le sol (adouci sous
    /// une ville, un camp ou un village).
    bumps: BumpMap,
    /// Nombre de reconstructions du sol de chaque case : ce qui y est posé (unités, villes…) est
    /// replacé quand il change.
    generation: HashMap<Hex, u32>,
    /// Version de la forme du sol (profils, relief interne, lits) : change quand la hauteur du sol
    /// peut avoir changé quelque part, pas pour un simple changement de couleurs. Les tracés posés
    /// sur le sol (cartes du réseau et des plaques) sont recalculés quand elle change.
    pub shape: u32,
    /// Tour d'est en ouest de la planète. Tout est rangé par case de la carte, la géométrie
    /// (lits, formes de l'eau) à la position de la case dans la carte : une case vue au-delà du
    /// raccord est lue dans la carte, sa géométrie décalée d'un tour.
    wrap: Wrap,
}

/// Sous une ville, un camp ou un village, le relief interne est adouci (pas aplani) : les
/// bâtiments suivent le terrain sans être de travers.
const SITE_RELIEF: f32 = 0.5;

/// Relief interne des cases explorées, adouci sous les sites (`sites`).
fn explored_bumps(map: &crate::map::WorldMap, fog: &ShownFog, sites: &HashSet<Hex>) -> BumpMap {
    fog.explored
        .iter()
        .filter(|&&h| map.contains(h))
        .filter_map(|&h| {
            bumps_for(map, h).map(|b| if sites.contains(&h) { (h, Bumps { amp: b.amp * SITE_RELIEF, summit: b.summit * SITE_RELIEF, ..b }) } else { (h, b) })
        })
        .collect::<BumpMap>()
        .with_wrap(map.wrap())
}

/// Relief interne d'une case de terre, modelé dans le sol 3D : ampleur des crêtes, sommet au
/// centre, limite des neiges (plus basse dans le froid). Rien pour une case plate.
fn bumps_for(map: &crate::map::WorldMap, hex: Hex) -> Option<Bumps> {
    let (amp, summit) = match map.relief(hex) {
        CellRelief::Flat => return None,
        // (pas de sommet propre à chaque montagne : un damier de cônes ; les crêtes viennent du
        // bruit continu, seuls les pics ont leur sommet)
        CellRelief::Hills => (5.0, 0.0),
        CellRelief::Rugged => (9.0, 0.0),
        CellRelief::Mountain => (17.0, 0.0),
        CellRelief::Peak => (16.0, 24.0),
    };
    let snowline = if map.get(hex) == Some(Terrain::Snow) {
        0.0
    } else {
        // (collines et reliefs bas ne sont enneigés que sur un terrain de neige)
        ((map.temperature(hex).unwrap_or(5.0) + 12.0) * 2.0).clamp(10.0, 40.0)
    };
    Some(Bumps { amp, summit, snowline })
}

impl Relief {
    /// Hauteur du sol au point 2D (x, y) du monde (lits des cours d'eau creusés).
    pub fn ground(&self, x: f32, y: f32) -> f32 {
        // (point ramené dans la carte : la géométrie y est rangée)
        let shift = self.wrap.offset(Hex::from_world(x, y));
        let (x, y) = (x - shift, y);
        let hex = Hex::from_world(x, y);
        let p = Vec2::new(x, y);
        // Relief interne (collines, montagnes), effacé près des cours d'eau
        let up = lift_at(p, &self.bumps).0;
        let up = if up > 0.0 { up * self.channels.get(&hex).map_or(1.0, |c| valley_fade(c, p)) } else { 0.0 };
        let terrain = self.surface(x, y) + up;
        self.channels.get(&hex).map_or(terrain, |c| carve(c, p, terrain).0)
    }

    /// Hauteur du sol sans les lits des cours d'eau.
    fn surface(&self, x: f32, y: f32) -> f32 {
        let hex = Hex::from_world(x, y);
        let (cx, cy) = hex.to_world();
        self.tops.get(&self.wrap.hex(hex)).map_or(unexplored_height(), |t| t.sample(Vec2::new(x - cx, y - cy)))
    }

    /// Mer ou lac exploré le plus proche d'un point : distance (approchée) et couleur.
    fn nearest_water(&self, p: Vec2) -> Option<(f32, Vec3)> {
        Hex::from_world(p.x, p.y)
            .range(2)
            .filter(|&h| self.water_seen.contains(&self.wrap.hex(h)))
            .filter_map(|h| {
                let (cx, cy) = h.to_world();
                let d = (p.distance(Vec2::new(cx, cy)) - HEX_SIZE * 0.866).max(0.0);
                self.water_colors.get(&self.wrap.hex(h)).map(|&c| (d, c))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// Lits visibles qui touchent une case : les siens et ceux de ses voisines (une arête est
    /// ainsi taillée pareil des deux côtés).
    fn channels_near(&mut self, hex: Hex, explored: impl Fn(Hex) -> bool + Copy) -> Vec<Channel> {
        let mut out: Vec<Channel> = Vec::new();
        let (cx, cy) = hex.to_world();
        let c = Vec2::new(cx, cy);
        for seen in hex.range(1) {
            // Voisine au-delà du raccord : celle de la carte, ses lits décalés d'un tour
            let (h, shift) = (self.wrap.hex(seen), Vec2::new(self.wrap.offset(seen), 0.0));
            let Some((shape, _)) = self.water.get(&h) else { continue };
            if !self.axis_cache.contains_key(&h) {
                let all: Vec<Channel> = shape.channels.iter().map(|a| self.channel(a)).collect();
                self.axis_cache.insert(h, all);
            }
            let shape = &self.water[&h].0;
            let cached = &self.axis_cache[&h];
            for (axis, &ch) in shape.channels.iter().zip(cached) {
                if !(explored(h) || axis.also.is_some_and(explored)) {
                    continue;
                }
                let ch = ch.shifted(shift);
                // Seulement les lits qui peuvent toucher la case
                if ch.nearest(c).distance(c) < HEX_SIZE + ch.hw.max(ch.hw_end) + 14.0 && !out.contains(&ch) {
                    out.push(ch);
                }
            }
        }
        out
    }

    /// Hauteur de l'eau d'un type donné en un point : le terrain le plus bas autour (au centre et
    /// sur deux cercles), un peu en dessous. Elle ne dépend que du point : deux tronçons qui se
    /// rejoignent ont la même hauteur au point commun, l'eau ne déborde jamais d'une berge plus
    /// basse que l'autre, et descend en pente douce vers la mer, un lac ou le bas d'une pente.
    fn water_level(&self, kind: WaterKind, p: Vec2) -> f32 {
        let bed = kind.bed();
        let ring = |r: f32| (0..6).map(move |k| p + Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_3 + 0.26) * r);
        // Terre : un peu sous le terrain alentour (le long d'un changement de niveau, l'eau coule
        // un peu au-dessus du bas de la pente : lit creusé côté haut, légèrement remblayé côté bas)
        // Seulement le terrain connu : une case inexplorée a un relief provisoire (plat), qui
        // ferait plonger ou monter l'eau au bord de la zone explorée
        let samples = |known: bool| {
            ring(bed.near)
                .chain(ring(bed.far))
                .chain([p, p])
                .filter(move |q| {
                    let h = self.wrap.hex(Hex::from_world(q.x, q.y));
                    !self.water_hexes.contains(&h) && (!known || self.explored.contains(&h))
                })
                .map(|q| self.surface(q.x, q.y))
                .fold((0.0f32, 0usize, f32::MAX), |(s, n, m), h| (s + h, n + 1, m.min(h)))
        };
        let (sum, count, low) = match samples(true) {
            (_, 0, _) => samples(false),
            known => known,
        };
        // Près du terrain le plus bas, un peu vers la moyenne : peu de remblai côté bas (pas de
        // levée de terre abrupte), une entaille modérée côté haut
        let land = if count > 0 { low + (sum / count as f32 - low) * LEVEL_TOWARD_MEAN - bed.below } else { f32::MAX };
        // Mer ou lac le plus proche : en s'en approchant, l'eau rejoint progressivement sa
        // surface (qu'elle soit plus haute, à la sortie d'un lac, ou plus basse, à l'embouchure)
        let here = Hex::from_world(p.x, p.y);
        let nearest = here
            .range(2)
            .filter(|&h| self.water_seen.contains(&self.wrap.hex(h)))
            .filter_map(|h| {
                let (cx, cy) = h.to_world();
                let d = (p.distance(Vec2::new(cx, cy)) - HEX_SIZE * 0.866).max(0.0);
                self.tops.get(&self.wrap.hex(h)).map(|t| (d, t.center))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        match nearest {
            None => land,
            Some((_, surface)) if land == f32::MAX => surface - MOUTH_BELOW,
            Some((d, surface)) => {
                let f = 1.0 - smoothstep01(d / bed.far);
                land + (surface - MOUTH_BELOW - land) * f
            }
        }
    }

    /// Lit 3D d'un tronçon : hauteurs de l'eau le long de l'axe. Un affluent qui entre dans un
    /// fleuve passe progressivement de la hauteur d'une rivière à celle du fleuve.
    fn channel(&self, axis: &WaterAxis) -> Channel {
        let levels = std::array::from_fn(|k| {
            let t = k as f32 / (CHANNEL_LEVELS - 1) as f32;
            let p = axis.a.lerp(axis.b, t);
            let own = self.water_level(axis.kind, p);
            if axis.inflow { own + (self.water_level(WaterKind::Fleuve, p) - own) * t } else { own }
        });
        let bed = if axis.inflow { WaterKind::Fleuve.bed().bed } else { axis.kind.bed().bed };
        Channel { a: axis.a, b: axis.b, hw: axis.hw, hw_end: axis.hw_end, levels, bed }
    }

    /// Point 3D posé sur le sol (plus `lift`) au point 2D (x, y).
    pub fn on_ground(&self, x: f32, y: f32, lift: f32) -> Vec3 {
        Vec3::new(x, self.ground(x, y) + lift, -y)
    }
}

/// Représentation 3D d'une entité du jeu (unité, ville, village, camp), posée sur le sol 3D.
#[derive(Component)]
struct Mirror3d {
    source: Entity,
    /// Figurine d'unité : posée sur le toit du donjon s'il y a une ville sur la case.
    is_unit: bool,
    /// Dernier placement (case, reconstruction de son sol, sur un donjon) : replacée quand il change.
    placed: Option<(Hex, u32, bool)>,
}

/// Élément d'une ville, d'un camp ou d'un village posé sur le sol à sa place (repère 2D par
/// rapport au centre de la case), d'après le sol sous son emprise (rayon `reach`).
#[derive(Component)]
struct Grounded {
    at: Vec2,
    reach: f32,
    rest: Rest,
}

/// Façon de poser un élément sur un sol en pente.
#[derive(Clone, Copy)]
enum Rest {
    /// Bâti (mur, tour, maison, hutte, pieu) : sa base descend jusqu'au point le plus bas du sol
    /// sous lui (fondations) et son sommet est à `height` au-dessus du point le plus haut ; pièce
    /// centrée, de hauteur `height` (étirée).
    Stand { height: f32 },
    /// Posé sur le point le plus haut (toit, plus `lift` jusqu'au centre de la pièce).
    Top { lift: f32 },
    /// Posé sur le point le plus bas, enfoncé côté haut (tente, feu, pierres).
    Low { lift: f32 },
}

/// Sol drapé sur le relief (pavés d'une ville, terre battue d'un camp) : contour (repère 2D),
/// refait quand le site est replacé.
#[derive(Component)]
struct Patch3d {
    outline: Vec<Vec2>,
}

/// Anneau au sol autour d'une figurine (couleur du camp) : incliné comme le sol.
#[derive(Component)]
struct Socle3d;

/// Barque sous une figurine sur un fleuve (orientée dans le sens du courant).
#[derive(Component)]
struct Boat3d;

/// Une figurine du groupe d'une unité, à sa place (repère 2D, par rapport au centre de la case),
/// posée sur le sol sous ses pieds.
#[derive(Component)]
struct Member {
    at: Vec2,
    /// Position posée (repère local, à la taille normale) : écartée avec l'agrandissement.
    base: Vec3,
}

/// Icône d'une unité, tournée vers la caméra, montrée seulement en vue lointaine.
#[derive(Component)]
struct FarIcon;

/// Hauteur du donjon d'une ville (une unité en garnison est posée dessus).
pub const CITY_HEIGHT: f32 = 12.0;

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct Gizmos3d;

/// Pièces d'une ville 3D : socle, remparts, tours d'angle, maisons et donjon central.
struct CityKit {
    wall: Handle<Mesh>,
    tower: Handle<Mesh>,
    tower_roof: Handle<Mesh>,
    house: Handle<Mesh>,
    house_roof: Handle<Mesh>,
    keep: Handle<Mesh>,
    keep_roof: Handle<Mesh>,
    stone: Handle<StandardMaterial>,
    paving: Handle<StandardMaterial>,
    plaster: Handle<StandardMaterial>,
    tiles: Handle<StandardMaterial>,
}

/// Pièces des villages (huttes de paille autour d'un feu) et des camps hostiles
/// (tentes rouges et étendard dans une palissade de pieux).
struct SiteKit {
    hut: Handle<Mesh>,
    hut_roof: Handle<Mesh>,
    stake: Handle<Mesh>,
    stake_tip: Handle<Mesh>,
    tent: Handle<Mesh>,
    pole: Handle<Mesh>,
    flag: Handle<Mesh>,
    fire: Handle<Mesh>,
    fire_stone: Handle<Mesh>,
    camp_ground: Handle<StandardMaterial>,
    mud: Handle<StandardMaterial>,
    straw: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    tent_red: Handle<StandardMaterial>,
    flame: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
}

/// Anneau aux couleurs du camp autour du groupe de figurines (rayon, épaisseur) et emprise des
/// pieds d'une figurine.
const SOCLE_RADIUS: f32 = 8.5;
const SOCLE_HEIGHT: f32 = 0.6;
const FIGURE_REACH: f32 = 3.0;
/// Barque : pont (où se tient la figurine) au-dessus de l'eau, centre de la coque au-dessus de l'eau ;
/// le groupe s'y resserre.
const BOAT_DECK: f32 = 1.4;
const BOAT_HULL_LIFT: f32 = 0.3;
const BOAT_SQUEEZE: f32 = 0.45;
/// Les figurines (avec leur anneau et leur barque) grandissent quand la caméra 3D s'éloigne :
/// taille normale jusqu'à cette distance, puis proportionnelle, au plus `UNIT_SCALE_MAX`.
const UNIT_SCALE_FROM: f32 = 450.0;
const UNIT_SCALE_MAX: f32 = 3.0;
/// Au-delà de cette distance de la caméra 3D, l'icône du type d'unité flotte au-dessus de la
/// figurine (de loin, la figurine seule ne se lit plus).
const FAR_ICON_DISTANCE: f32 = 1_100.0;
const FAR_ICON_LIFT: f32 = 20.0;
/// Taille de l'icône (fois le carré de 26) à la distance d'apparition.
const FAR_ICON_SCALE: f32 = 0.9;
/// Dimensions d'une ville 3D (repère local : origine sur le sol au centre de la case ; chaque
/// élément est posé sur le sol à sa place, voir `Grounded`).
const CITY_WALL_RADIUS: f32 = 26.0;
const CITY_WALL_HEIGHT: f32 = 7.0;
const CITY_TOWER_HEIGHT: f32 = 12.0;
const CITY_KEEP_ROOF: f32 = 1.5;
const CITY_KEEP_RADIUS: f32 = 10.0;

#[derive(Resource)]
struct Assets3d {
    /// Sol (la couleur du terrain est portée par les sommets) : [terre, eau], vu / hors de vue.
    ground: [Handle<StandardMaterial>; 2],
    ground_dim: [Handle<StandardMaterial>; 2],
    unexplored: Handle<StandardMaterial>,
    /// Eau douce (couleur portée par les sommets) : vue, hors de vue.
    fresh_water: [Handle<StandardMaterial>; 2],
    socle: Handle<Mesh>,
    /// Barque : coque, proue et poupe en pointe ; bois.
    boat_hull: Handle<Mesh>,
    boat_end: Handle<Mesh>,
    boat_wood: Handle<StandardMaterial>,
    icon_quad: Handle<Mesh>,
    /// Pastille aux couleurs du camp derrière l'icône lointaine.
    icon_disc: Handle<Mesh>,
    /// Matières (non éclairées) des pastilles : joueur, hostile.
    icon_backs: [Handle<StandardMaterial>; 2],
    figures: crate::figures::FigureKit,
    player: Handle<StandardMaterial>,
    hostile: Handle<StandardMaterial>,
    city: CityKit,
    sites: SiteKit,
    icons: HashMap<&'static str, Handle<StandardMaterial>>,
}

pub struct View3dPlugin;

impl Plugin for View3dPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewMode>()
            .init_gizmo_group::<Gizmos3d>()
            .insert_resource(GlobalAmbientLight { brightness: 420.0, ..default() })
            .add_systems(Startup, setup_3d)
            .add_systems(
                Update,
                (
                    toggle_view_key,
                    apply_view_mode,
                    camera_3d_controls,
                    update_relief_3d,
                    update_decor_3d,
                    spawn_mirrors,
                    sync_mirrors,
                    far_icons,
                    scale_units,
                    draw_overlays_3d,
                    draw_wind_3d,
                    draw_currents_3d,
                    draw_plates_3d,
                    sync_seam_copies,
                )
                    .chain(),
            );
    }
}

/// Niveau de la mer en 3D.
const SEA_LEVEL: f32 = 8.0;
/// Surface d'un lac sous les terres de son niveau.
const LAKE_DEPTH: f32 = 3.0;
/// Hauteur de l'eau des rivières, fleuves et petits lacs au-dessus du sol.
fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Hauteur de l'eau sur terre : entre le terrain le plus bas alentour (0) et le terrain moyen (1).
const LEVEL_TOWARD_MEAN: f32 = 0.3;

/// Un cours d'eau qui se jette dans la mer ou un lac arrive juste sous leur surface.
const MOUTH_BELOW: f32 = 0.06;
/// Distance sur laquelle l'eau d'une embouchure prend la couleur de la mer ou du lac.
const MOUTH_BLEND: f32 = 14.0;
/// Dénivelé minimal d'une chute d'eau.
const CASCADE_MIN_DROP: f32 = 4.0;

/// Hauteur 3D d'une altitude réelle (m) : 150 m ≈ 16, 2 400 m ≈ 56, 4 800 m ≈ 98 (les anciennes
/// tranches d'altitude tombent aux mêmes hauteurs).
pub fn meters_to_height(m: f32) -> f32 {
    SEA_LEVEL + LAND_BASE + m.max(0.0) * METER_SCALE
}
/// Hauteur de la terre au niveau de la mer au-dessus de la mer, et hauteur 3D par mètre.
const LAND_BASE: f32 = 6.0;
const METER_SCALE: f32 = 0.0175;

/// Hauteur du dessus d'une case selon son altitude réelle : mer au niveau de la mer, lac un peu
/// sous la surface de ses rives, terre à la hauteur de son altitude en mètres (le relief des
/// collines et montagnes est modelé par-dessus, voir `relief::lift_at`).
pub fn tile_height(map: &crate::map::WorldMap, hex: Hex) -> f32 {
    match map.get(hex) {
        Some(t) if t.is_sea() => SEA_LEVEL,
        Some(Terrain::Lake) => meters_to_height(map.surface_elevation(hex).unwrap_or(0.0)) - LAKE_DEPTH,
        Some(_) => meters_to_height(map.surface_elevation(hex).unwrap_or(0.0)),
        None => SEA_LEVEL,
    }
}

/// Hauteur provisoire d'une case inexplorée (plate, au niveau du littoral).
fn unexplored_height() -> f32 {
    meters_to_height(150.0)
}

/// Cases dont les lits creusés gardés ne sont plus ceux qu'on calculerait sur le relief actuel, et
/// plus grand écart de hauteur d'eau (vérification du mode capture, relief 3D à jour).
pub fn stale_channels(relief: &mut Relief, map: &GameMap, fog: &ShownFog) -> (Vec<Hex>, f32) {
    relief.axis_cache.clear();
    let mut stale = Vec::new();
    let mut worst = 0.0f32;
    let hexes: Vec<Hex> = relief.channels.keys().copied().collect();
    for hex in hexes {
        let fresh = relief.channels_near(hex, |h| fog.explored.contains(&map.0.canonical(h)));
        let kept = &relief.channels[&hex];
        if *kept != fresh {
            for (a, b) in kept.iter().zip(&fresh) {
                worst = a.levels.iter().zip(&b.levels).fold(worst, |w, (x, y)| w.max((x - y).abs()));
            }
            stale.push(hex);
        }
    }
    (stale, worst)
}

/// Hauteur affichée d'une case : inexplorée, elle reste plate (son relief n'est pas connu).
pub fn shown_height(map: &GameMap, fog: &ShownFog, hex: Hex) -> f32 {
    let hex = map.0.canonical(hex);
    if map.0.contains(hex) && fog.explored.contains(&hex) { tile_height(&map.0, hex) } else { unexplored_height() }
}

pub fn to_3d(hex: Hex, height: f32) -> Vec3 {
    let (x, y) = hex.to_world();
    Vec3::new(x, height, -y)
}

fn layer() -> RenderLayers {
    RenderLayers::layer(LAYER_3D)
}

fn camera_transform(cam: &Cam3d) -> Transform {
    let pitch = PITCH_DEG.to_radians();
    let offset = Vec3::new(cam.yaw.sin() * pitch.cos(), pitch.sin(), cam.yaw.cos() * pitch.cos()) * cam.distance;
    Transform::from_translation(cam.target + offset).looking_at(cam.target, Vec3::Y)
}

fn setup_3d(
    mut commands: Commands,
    map: Res<GameMap>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmo_store: ResMut<GizmoConfigStore>,
) {
    let start = map.0.find_start().unwrap_or_default();
    let cam = Cam3d { target: to_3d(start, 0.0), distance: 900.0, yaw: 0.0 };
    commands.spawn((
        Camera3d::default(),
        Camera { is_active: false, order: 1, ..default() },
        camera_transform(&cam),
        layer(),
    ));
    commands.insert_resource(cam);
    commands.spawn((
        DirectionalLight { illuminance: 5_000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(-400.0, 900.0, 600.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer(),
    ));

    // Lignes 3D (sélection, trajets…) : sur la couche 3D, légèrement en avant du relief
    let (config, _) = gizmo_store.config_mut::<Gizmos3d>();
    config.render_layers = layer();
    config.line.width = 3.0;
    config.depth_bias = -0.02;

    // Brouillard : non éclairé, pour rester noir comme en 2D
    let unexplored = materials.add(StandardMaterial {
        base_color: Color::srgb(0.035, 0.035, 0.05),
        unlit: true,
        ..default()
    });
    // Flammes des feux de camp : non éclairées, toujours vives
    let flame = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.55, 0.12),
        emissive: LinearRgba::rgb(6.0, 2.2, 0.3),
        unlit: true,
        ..default()
    });
    let mut mat = |color: Color, roughness: f32| {
        materials.add(StandardMaterial { base_color: color, perceptual_roughness: roughness, ..default() })
    };
    // Sol : blanc (vu) ou assombri (hors de vue), multiplié par la couleur des sommets
    let ground = [mat(Color::WHITE, 0.9), mat(Color::WHITE, 0.25)];
    let dim = Color::srgb(0.45, 0.45, 0.5);
    let ground_dim = [mat(dim, 0.9), mat(dim, 0.25)];
    let player = mat(Color::srgb(0.20, 0.45, 0.95), 0.6);
    let hostile = mat(Color::srgb(0.78, 0.17, 0.15), 0.6);
    let city_stone = mat(Color::srgb(0.62, 0.59, 0.54), 0.9);
    let city_paving = mat(Color::srgb(0.36, 0.34, 0.31), 0.95);
    let city_plaster = mat(Color::srgb(0.80, 0.74, 0.62), 0.9);
    let city_tiles = mat(Color::srgb(0.62, 0.29, 0.19), 0.8);
    let camp_ground = mat(Color::srgb(0.22, 0.17, 0.12), 0.95);
    let mud = mat(Color::srgb(0.55, 0.42, 0.28), 0.95);
    let straw = mat(Color::srgb(0.80, 0.66, 0.36), 0.9);
    let wood = mat(Color::srgb(0.42, 0.28, 0.15), 0.9);
    let tent_red = mat(Color::srgb(0.66, 0.14, 0.10), 0.85);
    let rock = mat(Color::srgb(0.50, 0.48, 0.47), 0.95);
    let boulder = mat(Color::srgb(0.46, 0.42, 0.38), 1.0);

    // Végétation des biomes : feuillus (houppier rond sur un tronc), conifères étroits (taïga),
    // grands arbres sombres (jungle), acacias à cime plate (savanes), buissons (broussailles), roseaux
    // et flaques (marais)
    let trunk_mesh = meshes.add(Cylinder::new(0.9, 8.0));
    let crown_mesh = meshes.add(Sphere::new(5.5).mesh().ico(1).unwrap());
    let conifer_mesh = meshes.add(Cone { radius: 3.8, height: 19.0 });
    let canopy_mesh = meshes.add(Cylinder::new(6.5, 1.8));
    let shrub_mesh = meshes.add(Sphere::new(3.0).mesh().ico(1).unwrap());
    let reed_mesh = meshes.add(Cone { radius: 0.55, height: 7.0 });
    let puddle_mesh = meshes.add(Cylinder::new(4.5, 0.25));
    let rock_mesh = meshes.add(Sphere::new(2.6).mesh().ico(0).unwrap());
    let trunk = mat(Color::srgb(0.32, 0.22, 0.13), 0.95);
    let broadleaf = mat(Color::srgb(0.17, 0.38, 0.14), 0.9);
    let jungle_leaf = mat(Color::srgb(0.05, 0.27, 0.09), 0.85);
    let conifer = mat(Color::srgb(0.07, 0.22, 0.15), 0.9);
    let acacia = mat(Color::srgb(0.33, 0.42, 0.15), 0.9);
    let shrub = mat(Color::srgb(0.30, 0.37, 0.20), 0.95);
    let reed = mat(Color::srgb(0.48, 0.50, 0.26), 0.95);
    let puddle = mat(Color::srgb(0.26, 0.42, 0.48), 0.3);
    let plant_meshes = [
        trunk_mesh.clone(),
        crown_mesh.clone(),
        conifer_mesh.clone(),
        canopy_mesh.clone(),
        shrub_mesh.clone(),
        reed_mesh.clone(),
        puddle_mesh.clone(),
    ];

    let mut icons = HashMap::new();
    let icon_paths: Vec<&'static str> =
        UnitKind::ALL.iter().map(|k| k.icon()).collect();
    for path in icon_paths {
        icons.insert(
            path,
            materials.add(StandardMaterial {
                base_color_texture: Some(assets.load(path)),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
        );
    }

    // Relief : au départ tout est inexploré, donc plat
    // Relief interne de toutes les cases (pour placer les arbres sous la limite des arbres)
    let wrap = map.0.wrap();
    let all_bumps: BumpMap = map.0.hexes().filter_map(|h| bumps_for(&map.0, h).map(|b| (h, b))).collect::<BumpMap>().with_wrap(wrap);
    let mut relief = Relief { tops: HashMap::new(), tiles: HashMap::new(), decor: HashMap::new(), water: HashMap::new(), channels: HashMap::new(), axis_cache: HashMap::new(), explored: HashSet::new(), hatches: HashMap::new(), bumps: BumpMap::new().with_wrap(wrap), generation: HashMap::new(), shape: 0, wrap, water_seen: HashSet::new(), water_colors: map.0.hexes().filter_map(|h| map.0.get(h).filter(|t| t.is_water()).map(|t| (h, terrain_color(t)))).collect(), water_hexes: map.0.hexes().filter(|&h| map.0.get(h).is_some_and(|t| t.is_water())).collect() };
    let fresh_water = [
        materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.25, ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.45, 0.5), perceptual_roughness: 0.25, ..default() }),
    ];
    for hex in map.0.hexes() {
        let shape = hex_water(&map.0, hex);
        if !shape.is_empty() {
            let mesh = meshes.add(to_mesh(MeshData::default()));
            let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(fresh_water[0].clone()), layer())).id();
            relief.water.insert(hex, (shape, e));
        }
    }
    for hex in map.0.hexes() {
        let top = compute_top(
            hex,
            |_| unexplored_height(),
            |a, b| if map.0.contains(a) && map.0.contains(b) { Link::Gentle } else { Link::None },
            |_| false,
        );
        relief.tops.insert(hex, top);
    }
    for hex in map.0.hexes() {
        let mesh = build_hex(hex, &relief.tops[&hex], Vec3::ONE, None, &[], false, &relief.bumps);
        let tile = commands.spawn((Tile3d, Mesh3d(meshes.add(to_mesh(mesh))), MeshMaterial3d(unexplored.clone()), layer())).id();
        relief.tiles.insert(hex, tile);

        // Décor : arbres en forêt, bosses en colline, pic en montagne (hauteurs fixées avec le relief)
        let t = map.0.get(hex).unwrap();
        let polar = |deg: f32, r: f32| Vec2::from_angle(deg.to_radians()) * r;
        let mut pieces: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Vec2, f32, Vec3)> = Vec::new();
        // Relief interne (collines, montagnes) : modelé dans le sol de la case (voir `Bumps`) ;
        // quelques rochers sur un relief accidenté
        let relief_kind = map.0.relief(hex);
        if relief_kind == CellRelief::Rugged {
            for (a, r) in [(30.0_f32, 6.0), (150.0, 20.0), (270.0, 18.0)] {
                pieces.push((rock_mesh.clone(), rock.clone(), polar(a, r), 1.5, Vec3::new(1.4, 1.0, 1.1)));
            }
        }
        // Végétation selon le biome (pas dans le lit d'un fleuve, ni sur un pic). Les plantes
        // restent près du bord : le centre est libre pour le jeton d'une unité. En montagne, les
        // arbres s'arrêtent à la limite des arbres (sous la limite des neiges).
        if !map.0.is_fleuve(hex) && relief_kind < CellRelief::Peak {
            let ring6 = [30.0_f32, 90.0, 150.0, 210.0, 270.0, 330.0];
            let (cx, cy) = hex.to_world();
            let below_treeline = |at: Vec2| {
                let (up, snow) = lift_at(Vec2::new(cx, cy) + at, &all_bumps);
                up < (snow * 0.6).min(14.0)
            };
            // Emplacements (position, hausse) sous la limite des arbres ; seconde couronne (entre
            // les premiers) pour les forêts denses
            let spots: Vec<(Vec2, f32)> = ring6.iter().map(|&a| (polar(a, 21.0), 0.0)).filter(|&(at, _)| below_treeline(at)).collect();
            let inner: Vec<Vec2> = ring6.iter().map(|&a| polar(a + 30.0, 24.5)).filter(|&at| below_treeline(at)).collect();
            // Feuillu : tronc et houppier rond
            let broad = |at: Vec2, up: f32, leaf: &Handle<StandardMaterial>, size: f32, pieces: &mut Vec<_>| {
                pieces.push((trunk_mesh.clone(), trunk.clone(), at, up + 4.0 * size, Vec3::new(size, size, size)));
                pieces.push((crown_mesh.clone(), leaf.clone(), at, up + 10.5 * size, Vec3::new(size, 0.85 * size, size)));
            };
            match t {
                Terrain::Forest => {
                    for &(at, up) in &spots {
                        broad(at, up, &broadleaf, 1.0, &mut pieces);
                    }
                }
                Terrain::Jungle => {
                    // Dense : grands arbres sur le pourtour et plus petits entre eux
                    for &(at, up) in &spots {
                        broad(at, up, &jungle_leaf, 1.2, &mut pieces);
                    }
                    for &at in &inner {
                        broad(at, 0.0, &jungle_leaf, 0.8, &mut pieces);
                    }
                }
                Terrain::Taiga => {
                    for &(at, up) in &spots {
                        pieces.push((conifer_mesh.clone(), conifer.clone(), at, up + 9.0, Vec3::ONE));
                    }
                    for &at in &inner {
                        pieces.push((conifer_mesh.clone(), conifer.clone(), at, 7.0, Vec3::splat(0.8)));
                    }
                }
                Terrain::WoodedSavanna | Terrain::Savanna => {
                    // Acacias : tronc fin et cime plate, épars (un seul en savane herbeuse)
                    let picks: &[usize] = if t == Terrain::Savanna { &[1] } else { &[0, 2, 4] };
                    for &k in picks.iter().filter(|_| !spots.is_empty()) {
                        let (at, up) = spots[k % spots.len()];
                        pieces.push((trunk_mesh.clone(), trunk.clone(), at, up + 4.0, Vec3::new(0.7, 1.2, 0.7)));
                        pieces.push((canopy_mesh.clone(), acacia.clone(), at, up + 9.5, Vec3::ONE));
                    }
                }
                Terrain::Mediterranean => {
                    // Broussailles : buissons bas, un arbre (olivier) de temps en temps
                    for (k, &(at, up)) in spots.iter().enumerate() {
                        if k % 3 == 0 {
                            broad(at, up, &shrub, 0.75, &mut pieces);
                        } else {
                            pieces.push((shrub_mesh.clone(), shrub.clone(), at, up + 2.0, Vec3::new(1.5, 0.9, 1.5)));
                        }
                    }
                }
                Terrain::Marsh => {
                    // Flaques et touffes de roseaux
                    for a in [60.0_f32, 240.0] {
                        pieces.push((puddle_mesh.clone(), puddle.clone(), polar(a, 18.0), 0.2, Vec3::ONE));
                    }
                    for a in [0.0_f32, 120.0, 180.0, 300.0] {
                        for (dx, dy) in [(0.0, 0.0), (1.6, 0.8), (-1.2, 1.4)] {
                            pieces.push((reed_mesh.clone(), reed.clone(), polar(a, 20.0) + Vec2::new(dx, dy), 3.0, Vec3::ONE));
                        }
                    }
                }
                _ => {}
            }
        }
        let mut ids = Vec::new();
        for (mesh, material, offset, lift, scale) in pieces {
            let vegetation = mesh == rock_mesh || plant_meshes.contains(&mesh);
            ids.push(
                commands
                    .spawn((
                        Decor3d { hex, offset, lift, requires: None, vegetation },
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        Transform::from_scale(scale),
                        Visibility::Hidden,
                        layer(),
                    ))
                    .id(),
            );
        }
        relief.decor.entry(hex).or_default().extend(ids);
    }
    // Rochers éboulés au pied des falaises (sur la case basse, le long de l'arête)
    let boulder_mesh = meshes.add(Sphere::new(3.2).mesh().ico(0).unwrap());
    for edge in map.0.level_edges().into_iter().filter(|e| e.is_cliff()) {
        // (case basse au-delà du raccord : position prise de ce côté, rangée dans la carte)
        let low = edge.low();
        let low_in_map = map.0.canonical(low);
        let ((ax, ay), (bx, by)) = edge.high.edge_corners(edge.dir);
        let (a, b) = (Vec2::new(ax, ay), Vec2::new(bx, by));
        let (lx, ly) = low.to_world();
        let center = Vec2::new(lx, ly);
        // Variation déterministe selon l'arête
        let seed = (low.q * 31 + low.r * 17 + edge.dir as i32 * 7).rem_euclid(97) as f32;
        let mut ids = Vec::new();
        for (k, u) in [0.22_f32, 0.5, 0.76].into_iter().enumerate() {
            let jitter = ((seed + k as f32 * 13.0).sin() * 0.5 + 0.5, (seed * 1.7 + k as f32 * 5.0).cos() * 0.5 + 0.5);
            let p = a.lerp(b, u + (jitter.0 - 0.5) * 0.12);
            let p = p + (center - p).normalize() * (3.0 + jitter.1 * 3.0);
            let size = 0.7 + jitter.0 * 0.6;
            ids.push(
                commands
                    .spawn((
                        Decor3d {
                            hex: low_in_map,
                            offset: p - center,
                            lift: 0.8 * size,
                            requires: Some(edge.high),
                            vegetation: false,
                        },
                        Mesh3d(boulder_mesh.clone()),
                        MeshMaterial3d(boulder.clone()),
                        Transform::from_scale(Vec3::new(size * 1.2, size * 0.8, size))
                            .with_rotation(Quat::from_rotation_y(seed + k as f32)),
                        Visibility::Hidden,
                        layer(),
                    ))
                    .id(),
            );
        }
        relief.decor.entry(low_in_map).or_default().extend(ids);
    }
    commands.insert_resource(relief);

    commands.insert_resource(Assets3d {
        ground,
        ground_dim,
        unexplored,
        fresh_water,
        socle: meshes.add(Torus::new(SOCLE_RADIUS - SOCLE_HEIGHT, SOCLE_RADIUS)),
        boat_hull: meshes.add(Cuboid::new(6.5, 2.2, 13.0)),
        boat_end: meshes.add(Cuboid::new(4.6, 2.2, 4.6)),
        boat_wood: materials.add(StandardMaterial { base_color: Color::srgb(0.40, 0.26, 0.14), perceptual_roughness: 0.9, ..default() }),
        icon_quad: meshes.add(Rectangle::new(26.0, 26.0)),
        icon_disc: meshes.add(Circle::new(16.0)),
        icon_backs: [Color::srgb(0.20, 0.45, 0.95), Color::srgb(0.78, 0.17, 0.15)].map(|c| materials.add(StandardMaterial { base_color: c, unlit: true, ..default() })),
        figures: crate::figures::FigureKit::new(&mut meshes, &mut materials),
        player,
        hostile,
        city: CityKit {
            wall: meshes.add(Cuboid::new(CITY_WALL_RADIUS, CITY_WALL_HEIGHT, 3.0)),
            tower: meshes.add(Cylinder::new(4.0, CITY_TOWER_HEIGHT)),
            tower_roof: meshes.add(Cone { radius: 5.2, height: 6.0 }),
            house: meshes.add(Cuboid::new(5.0, 5.0, 5.0)),
            house_roof: meshes.add(Cone { radius: 4.4, height: 4.0 }.mesh().resolution(4)),
            keep: meshes.add(Cylinder::new(CITY_KEEP_RADIUS, CITY_HEIGHT - CITY_KEEP_ROOF)),
            keep_roof: meshes.add(Cylinder::new(11.5, CITY_KEEP_ROOF)),
            stone: city_stone,
            paving: city_paving,
            plaster: city_plaster,
            tiles: city_tiles,
        },
        sites: SiteKit {
            hut: meshes.add(Cylinder::new(4.8, 5.0)),
            hut_roof: meshes.add(Cone { radius: 7.0, height: 7.5 }),
            stake: meshes.add(Cylinder::new(1.3, 8.0)),
            stake_tip: meshes.add(Cone { radius: 1.3, height: 2.5 }),
            tent: meshes.add(Cone { radius: 7.5, height: 10.0 }.mesh().resolution(3)),
            pole: meshes.add(Cylinder::new(0.6, 20.0)),
            flag: meshes.add(Cuboid::new(8.0, 5.0, 0.5)),
            fire: meshes.add(Cone { radius: 2.4, height: 5.0 }),
            fire_stone: meshes.add(Sphere::new(1.2)),
            camp_ground,
            mud,
            straw,
            wood,
            tent_red,
            flame,
            stone: rock.clone(),
        },
        icons,
    });
}

fn toggle_view_key(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<ViewMode>) {
    if keys.just_pressed(KeyCode::KeyV) {
        mode.is_3d = !mode.is_3d;
    }
}

/// Bascule de vue : active la bonne caméra, y rattache l'interface, et garde le même point de vue.
fn apply_view_mode(
    mut commands: Commands,
    mode: Res<ViewMode>,
    mut cam3d: ResMut<Cam3d>,
    mut cam2d_q: Query<(Entity, &mut Camera, &mut Transform), (With<Camera2d>, Without<Camera3d>, Without<crate::seam::GhostCamera>)>,
    mut cam3d_q: Query<(Entity, &mut Camera, &mut Transform), (With<Camera3d>, Without<Camera2d>, Without<crate::seam::GhostCamera>)>,
) {
    if !mode.is_changed() || mode.is_added() {
        return;
    }
    let (Ok((e2, mut c2, mut t2)), Ok((e3, mut c3, mut t3))) = (cam2d_q.single_mut(), cam3d_q.single_mut()) else {
        return;
    };
    if mode.is_3d {
        cam3d.target = Vec3::new(t2.translation.x, 0.0, -t2.translation.y);
        *t3 = camera_transform(&cam3d);
        commands.entity(e2).remove::<IsDefaultUiCamera>();
        commands.entity(e3).insert(IsDefaultUiCamera);
    } else {
        t2.translation.x = cam3d.target.x;
        t2.translation.y = -cam3d.target.z;
        commands.entity(e3).remove::<IsDefaultUiCamera>();
        commands.entity(e2).insert(IsDefaultUiCamera);
    }
    c2.is_active = !mode.is_3d;
    c3.is_active = mode.is_3d;
}

/// Caméra 3D : ZQSD/flèches pour se déplacer, molette pour zoomer, clic milieu pour glisser,
/// A et R (touches physiques Q et R d'un clavier QWERTY) pour tourner autour du point visé.
fn camera_3d_controls(
    mode: Res<ViewMode>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    over_ui: Res<crate::input::PointerOverUi>,
    time: Res<Time>,
    mut cam: ResMut<Cam3d>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<crate::seam::GhostCamera>)>,
) {
    if !mode.is_3d {
        return;
    }
    let before = (cam.target, cam.distance, cam.yaw);
    if scroll.delta.y != 0.0 && !over_ui.0 {
        let step = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * 0.1,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.002,
        };
        cam.distance = (cam.distance * (1.0 - step)).clamp(150.0, 2_800.0);
    }
    // Axes « avant » et « droite » au sol, selon l'orientation de la caméra
    let forward = Vec3::new(-cam.yaw.sin(), 0.0, -cam.yaw.cos());
    let right = Vec3::new(cam.yaw.cos(), 0.0, -cam.yaw.sin());
    let mut dir = Vec3::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir += forward;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir -= forward;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir -= right;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir += right;
    }
    let speed = cam.distance * 0.9;
    cam.target += dir.normalize_or_zero() * speed * time.delta_secs();
    if keys.pressed(KeyCode::KeyQ) {
        cam.yaw -= 1.5 * time.delta_secs();
    }
    if keys.pressed(KeyCode::KeyR) {
        cam.yaw += 1.5 * time.delta_secs();
    }
    if mouse.pressed(MouseButton::Middle) {
        let scale = cam.distance / 700.0;
        cam.target -= right * motion.delta.x * scale;
        cam.target += forward * motion.delta.y * scale;
    }
    if (cam.target, cam.distance, cam.yaw) != before || cam.is_changed() {
        for mut t in &mut camera {
            *t = camera_transform(&cam);
        }
    }
}

/// Teinte des bancs de poissons sur la carte des ressources (linéaire).
const BANK_TINT: Vec3 = Vec3::new(0.04, 0.42, 0.38);

/// Couleur d'un terrain en 3D (linéaire, atténuée), portée par les sommets du sol.
fn terrain_color(t: Terrain) -> Vec3 {
    let (r, g, b) = t.color();
    let c = Color::srgb(r * TERRAIN_TINT, g * TERRAIN_TINT, b * TERRAIN_TINT).to_linear();
    Vec3::new(c.red, c.green, c.blue)
}

/// Convertit la géométrie d'une case en maillage Bevy.
fn to_mesh(data: MeshData) -> Mesh {
    let MeshData { mut positions, mut normals, mut colors, mut indices } = data;
    if indices.is_empty() {
        // Maillage vide interdit : un triangle dégénéré
        positions = vec![[0.0; 3]; 3];
        normals = vec![[0.0, 1.0, 0.0]; 3];
        colors = vec![[1.0; 4]; 3];
        indices = vec![0, 1, 2];
    }
    // Normales calculées à partir de la fonction de hauteur (voir relief.rs) : identiques des
    // deux côtés d'une arête, l'éclairage ne marque donc pas la limite entre cases
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

/// Raccord entre deux cases voisines : toutes deux inexplorées (plates) → pente douce ;
/// toutes deux explorées → pente douce entre deux cases d'eau, ou entre terre et terre / terre
/// et mer quand l'écart de niveau le permet (plage : littoral et plaine basse), escarpement
/// rocheux sinon (falaise, falaise côtière) ; bord de l'inexploré → paroi. (Une ville ou un camp
/// suit le terrain : pas de terrasse.)
fn link(map: &GameMap, fog: &ShownFog, a: Hex, b: Hex) -> Link {
    let (a, b) = (map.0.canonical(a), map.0.canonical(b));
    let (Some(ta), Some(tb)) = (map.0.get(a), map.0.get(b)) else { return Link::None };
    match (fog.explored.contains(&a), fog.explored.contains(&b)) {
        (false, false) => Link::Gentle,
        (true, true) => match (ta.is_water(), tb.is_water()) {
            (true, true) => Link::Gentle,
            // Terre–terre comme terre–mer (la mer au niveau 0) : plage ou pente si l'écart le
            // permet, falaise sinon (la mer reste plate, voir `compute_top`)
            (wa, wb) => {
                let (la, lb) = (map.0.level(a).unwrap_or(0), map.0.level(b).unwrap_or(0));
                match (!crate::map::is_cliff(la, lb), wa || wb) {
                    (true, true) => Link::Shore,
                    (true, false) => Link::Gentle,
                    (false, _) => Link::Cliff,
                }
            }
        },
        _ => Link::None,
    }
}

/// Relief et brouillard : recalcule le sol des cases dont le voisinage a changé (case explorée,
/// ville fondée, camp dispersé), repose leur décor, puis applique les couleurs (vue / hors de vue).
/// Une case inexplorée reste plate et noire : rien n'est révélé.
/// Mise à jour du relief 3D, étalée sur plusieurs images : les cases à refaire (brouillard qui
/// change, ville posée, changement de carte) attendent dans une file, traitée pendant un temps
/// limité à chaque image en commençant par les plus proches de la caméra. La vue 2D prépare aussi
/// le relief (avec un temps plus court par image, autour de la caméra 2D) : en passant en 3D, les
/// cases explorées sont déjà prêtes.
fn update_relief_3d(
    fog: Res<ShownFog>,
    map: Res<GameMap>,
    (mode, time): (Res<ViewMode>, Res<Time<Real>>),
    cam: Res<Cam3d>,
    assets: Option<Res<Assets3d>>,
    relief: Option<ResMut<Relief>>,
    structures: Query<&HexPos, Or<(With<City>, With<Camp>, With<Village>)>>,
    mut known: Local<ReliefQueue>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut tiles: Query<(&Mesh3d, &mut MeshMaterial3d<StandardMaterial>), With<Tile3d>>,
    mut decor: Query<(&Decor3d, &mut Transform)>,
    mut water_materials: Query<&mut MeshMaterial3d<StandardMaterial>, Without<Tile3d>>,
    water_meshes: Query<&Mesh3d, Without<Tile3d>>,
    layer: Res<MapLayer>,
    cam2d: Query<&Transform, (With<Camera2d>, Without<crate::seam::GhostCamera>, Without<Decor3d>)>,
    mut commands: Commands,
) {
    let (Some(assets), Some(mut relief)) = (assets, relief) else { return };
    let queue = &mut *known;
    // Le budget de temps couvre tout le système (file, construction, surfaces d'eau)
    let started = std::time::Instant::now();
    // Les cases les plus proches du centre de la vue d'abord
    let focus = match cam2d.single() {
        Ok(t) if !mode.is_3d => Hex::from_world(t.translation.x, t.translation.y),
        _ => Hex::from_world(cam.target.x, -cam.target.z),
    };
    let focus = map.0.canonical(focus);
    // Cases d'une ville, d'un camp ou d'un village : relief adouci (le sol alentour est refait)
    let sites: HashSet<Hex> = structures.iter().map(|p| p.0).collect();
    let sites_changed = sites != queue.sites;
    if fog.is_changed() || sites_changed {
        let changed: Vec<Hex> = fog
            .explored
            // Symétrique : en quittant le mode debug, des cases redeviennent inconnues
            .symmetric_difference(&queue.explored)
            .chain(sites.symmetric_difference(&queue.sites))
            .copied()
            .collect();
        for h in changed.iter().flat_map(|&h| map.0.range(h, 2)).filter(|h| map.0.contains(*h)) {
            queue.geometry.insert(h);
            queue.push(h, map.0.distance(h, focus));
        }
        queue.explored = fog.explored.clone();
        queue.sites = sites.clone();
    }
    if fog.is_changed() || sites_changed {
        relief.bumps = explored_bumps(&map.0, &fog, &sites);
        relief.shape += 1;
    }
    // Changement de carte (normale / relief / climat) : seules les couleurs changent, le relief déjà
    // calculé est réutilisé. La vision peut déborder du bord de la carte : on ne garde que la carte.
    if layer.is_changed() {
        queue.color.extend(fog.explored.iter().copied().filter(|h| map.0.contains(*h)));
        // (beaucoup de cases d'un coup : l'ordre est refait en une fois)
        queue.order_focus = None;
    }

    // Matériaux (case vue, explorée ou inconnue) : peu coûteux, mis à jour aussitôt
    if fog.is_changed() {
        relief.water_seen = relief.water_hexes.intersection(&fog.explored).copied().collect();
        relief.explored = fog.explored.clone();
        for (&hex, &tile) in &relief.tiles {
            // Une case dont le relief reste à refaire garde son aspect jusqu'à sa reconstruction
            // (pas de case blanche, plate, en attendant)
            if queue.geometry.contains(&hex) {
                continue;
            }
            let water = usize::from(map.0.get(hex).is_some_and(|t| t.is_water()));
            let wanted = if fog.visible.contains(&hex) {
                &assets.ground[water]
            } else if fog.explored.contains(&hex) {
                &assets.ground_dim[water]
            } else {
                &assets.unexplored
            };
            if let Ok((_, mut material)) = tiles.get_mut(tile)
                && material.0 != *wanted
            {
                material.0 = wanted.clone();
            }
        }
        for (hex, e) in &relief.hatches {
            let wanted = &assets.fresh_water[usize::from(!fog.visible.contains(hex))];
            if let Ok(mut material) = water_materials.get_mut(*e)
                && material.0 != *wanted
            {
                material.0 = wanted.clone();
            }
        }
        for (hex, (_, e)) in &relief.water {
            let lit = fog.visible.contains(hex) || map.0.neighbors(*hex).iter().any(|n| fog.visible.contains(n));
            let wanted = &assets.fresh_water[usize::from(!lit)];
            if let Ok(mut material) = water_materials.get_mut(*e)
                && material.0 != *wanted
            {
                material.0 = wanted.clone();
            }
        }
    }
    if queue.geometry.is_empty() && queue.color.is_empty() {
        queue.order.clear();
        return;
    }
    // Images trop longues (moins de 60 par seconde, par exemple quand une carte dessine beaucoup
    // de tracés) : le relief prend moins de temps par image, puis revient peu à peu à son budget
    if time.delta_secs() * 1000.0 > RELIEF_SLOW_FRAME_MS {
        queue.slowdown = (queue.slowdown + 0.15).min(RELIEF_SLOWDOWN_MAX);
    } else {
        queue.slowdown = (queue.slowdown - 0.02).max(0.0);
    }
    let budget = if mode.is_3d { RELIEF_BUDGET_MS } else { RELIEF_BUDGET_2D_MS } * (1.0 - queue.slowdown);
    // Ordre de traitement gardé d'une image à l'autre : refait seulement quand la vue s'est
    // déplacée (ou après l'arrivée de beaucoup de cases d'un coup)
    if queue.order_focus.is_none_or(|f| map.0.distance(f, focus) > RELIEF_REFOCUS) || queue.order.is_empty() {
        queue.reorder(focus, |h| map.0.distance(h, focus));
    }

    let height = |h: Hex| shown_height(&map, &fog, h);
    let link = |a: Hex, b: Hex| link(&map, &fog, a, b);
    let water = |h: Hex| fog.explored.contains(&map.0.canonical(h)) && map.0.get(h).is_some_and(|t| t.is_water());
    // Couleur du sol d'une case selon la carte affichée
    let hex_color = |hex: Hex| match (fog.explored.contains(&map.0.canonical(hex)), *layer) {
        (false, _) => Vec3::ONE,
        // Lit d'un fleuve en forêt : prairie de berge (pas d'arbres dans le lit)
        (true, MapLayer::Normal | MapLayer::Resources) if map.0.is_fleuve(hex) && map.0.get(hex).is_some_and(|t| t.is_wooded()) => {
            terrain_color(Terrain::Grassland)
        }
        // Carte des ressources : les bancs de poissons teintés
        (true, MapLayer::Resources) if map.0.bank(hex).is_some() => terrain_color(map.0.get(hex).unwrap()).lerp(BANK_TINT, 0.45),
        (true, MapLayer::Normal | MapLayer::Resources) => terrain_color(map.0.get(hex).unwrap()),
        (true, thematic) => {
            let t = map.0.get(hex).unwrap();
            let (r, g, b) = match thematic {
                MapLayer::Altitude | MapLayer::Relief => elevation_layer_color(&map.0, hex, thematic),
                MapLayer::Climate => climate_color(t, map.0.rain(hex).unwrap_or(500.0)),
                MapLayer::Plates => plate_color(&map.0, hex),
                MapLayer::Currents => currents_color(&map.0, hex),
                _ => hydro_color(&map.0, hex, thematic),
            };
            let c = Color::srgb(r, g, b).to_linear();
            Vec3::new(c.red, c.green, c.blue)
        }
    };
    // (case, numéro du paquet où elle a été refaite)
    let mut reshaped: Vec<(Hex, usize)> = Vec::new();
    let mut processed: Vec<(Hex, usize)> = Vec::new();
    // Par paquets : préparation (relief, lits) dans l'ordre, puis maillages construits en
    // parallèle sur tous les cœurs (deux cases par cœur, pour équilibrer cases simples et fines),
    // tant que le paquet suivant et les surfaces d'eau de toutes les cases traitées, estimés d'après
    // les images précédentes, tiennent dans le temps alloué à l'image (au moins un paquet par image)
    let chunk_size = RELIEF_CHUNK_PER_THREAD * rayon::current_num_threads().max(1);
    let mut paquet = 0;
    loop {
        let elapsed = started.elapsed().as_secs_f32() * 1000.0;
        let estimate = queue.chunk_ms + queue.water_ms * (processed.len() + chunk_size) as f32;
        if paquet > 0 && elapsed + estimate > budget {
            break;
        }
        let chunk = queue.next_chunk(chunk_size);
        if chunk.is_empty() {
            break;
        }
        let chunk_started = std::time::Instant::now();
        paquet += 1;
        let count = chunk.len();
        let mut jobs = Vec::with_capacity(count);
        for hex in chunk {
            let reshape = queue.geometry.remove(&hex);
            queue.color.remove(&hex);
            if reshape || !relief.tops.contains_key(&hex) {
                relief.tops.insert(hex, compute_top(hex, height, link, water));
                relief.shape += 1;
                reshaped.push((hex, paquet));
                // Les hauteurs d'eau des lits proches dépendent de ce relief
                for h in map.0.range(hex, 2) {
                    relief.axis_cache.remove(&h);
                }
            }
            processed.push((hex, paquet));
        }
        for &(hex, _) in &processed[processed.len() - count..] {
            // Lits qui touchent la case (la surface d'un lac ou de la mer reste plate, une case
            // inexplorée aussi)
            let near = if water(hex) || !fog.explored.contains(&hex) { Vec::new() } else { relief.channels_near(hex, |h| fog.explored.contains(&map.0.canonical(h))) };
            let before = if near.is_empty() { relief.channels.remove(&hex) } else { relief.channels.insert(hex, near.clone()) };
            if before.unwrap_or_default() != near {
                relief.shape += 1;
            }
            let color = hex_color(hex);
            // Case de fleuve : ses berges prennent la couleur des cases voisines (de terre)
            let sides = (map.0.is_fleuve(hex) && fog.explored.contains(&hex)).then(|| {
                std::array::from_fn(|d| {
                    let n = map.0.wrap().neighbor(hex, d);
                    let land = map.0.get(n).is_some_and(|t| !t.is_water()) && !map.0.is_fleuve(n);
                    if land && fog.explored.contains(&n) { hex_color(n) } else { color }
                })
            });
            jobs.push((hex, relief.tops[&hex], color, sides, near, water(hex)));
        }
        let bumps = &relief.bumps;
        let built: Vec<(Hex, MeshData)> = jobs
            .into_par_iter()
            .map(|(hex, top, color, sides, near, skirt)| (hex, build_hex(hex, &top, color, sides.as_ref(), &near, skirt, bumps)))
            .collect();
        for (hex, data) in built {
            *relief.generation.entry(hex).or_default() += 1;
            let tile = relief.tiles[&hex];
            if let Ok((mesh, mut material)) = tiles.get_mut(tile) {
                if let Some(mut m) = meshes.get_mut(&mesh.0) {
                    *m = to_mesh(data);
                }
                let w = usize::from(map.0.get(hex).is_some_and(|t| t.is_water()));
                let wanted = if fog.visible.contains(&hex) {
                    &assets.ground[w]
                } else if fog.explored.contains(&hex) {
                    &assets.ground_dim[w]
                } else {
                    &assets.unexplored
                };
                if material.0 != *wanted {
                    material.0 = wanted.clone();
                }
            }
            // Hachures (carte des bassins) : refaites avec le sol de la case
            let other = (*layer == MapLayer::Basins && fog.explored.contains(&hex))
                .then(|| map.0.basins().basin_also[map.0.cell(hex)])
                .flatten();
            match other {
                Some(b) => {
                    let (r, g, bl) = crate::render::basin_color(map.0.basins().basins[b as usize].color);
                    let data = hatch_mesh(&relief, hex, Color::srgb(r, g, bl));
                    let lit = usize::from(!fog.visible.contains(&hex));
                    if let Some(&e) = relief.hatches.get(&hex)
                        && let Ok(mesh) = water_meshes.get(e)
                        && let Some(mut m) = meshes.get_mut(&mesh.0)
                    {
                        *m = to_mesh(data);
                    } else {
                        let e = commands.spawn((Mesh3d(meshes.add(to_mesh(data))), MeshMaterial3d(assets.fresh_water[lit].clone()), self::layer())).id();
                        relief.hatches.insert(hex, e);
                    }
                }
                None => {
                    if let Some(e) = relief.hatches.remove(&hex) {
                        commands.entity(e).despawn();
                    }
                }
            }
            // Décor reposé sur le sol (relief ou lits voisins changés)
            for &e in relief.decor.get(&hex).into_iter().flatten() {
                if let Ok((d, mut transform)) = decor.get_mut(e) {
                    let (cx, cy) = d.hex.to_world();
                    transform.translation = relief.on_ground(cx + d.offset.x, cy + d.offset.y, d.lift);
                }
            }
        }
        // Durée d'un paquet complet (le dernier, plus petit, ne compte pas)
        if count == chunk_size {
            queue.chunk_ms = smooth(queue.chunk_ms, chunk_started.elapsed().as_secs_f32() * 1000.0);
        }
    }
    let water_started = std::time::Instant::now();
    // Surfaces d'eau des cases reconstruites et de leurs voisines (une rivière d'arête est
    // dessinée par une seule des deux cases)
    // (un simple changement de carte ne change pas l'eau : rien à refaire ; fait avant la remise en
    // file ci-dessous, qui redemande les surfaces d'eau pour l'image suivante)
    let mut redo: HashSet<Hex> = HashSet::new();
    let reshaped_set: HashSet<Hex> = reshaped.iter().map(|&(h, _)| h).collect();
    for &(hex, _) in &processed {
        if !queue.levels.remove(&hex) && !reshaped_set.contains(&hex) {
            continue;
        }
        redo.extend(map.0.range(hex, 1).filter(|h| relief.water.contains_key(h)));
    }
    // Le relief d'une case change la hauteur de l'eau des lits voisins : les cases proches qui
    // ont des cours d'eau sont reconstruites (couleurs et lits seulement). Une case déjà refaite
    // dans un paquet précédent de cette image a taillé ses lits avec l'ancien relief : elle est
    // refaite aussi (sinon ses lits gardent une hauteur d'eau périmée, différente de celle des
    // voisines : marches, écume, eau sous la berge au bord de la zone qu'on explore)
    let done: HashMap<Hex, usize> = processed.iter().copied().collect();
    for &(hex, paquet) in &reshaped {
        for h in map.0.range(hex, 3) {
            if done.get(&h).is_none_or(|&p| p < paquet)
                && !queue.geometry.contains(&h)
                && relief.channels.contains_key(&h)
                && map.0.contains(h)
            {
                queue.color.insert(h);
                queue.levels.insert(h);
                queue.push(h, map.0.distance(h, focus));
            }
        }
    }
    let explored = |h: Hex| fog.explored.contains(&map.0.canonical(h));
    let relief = &*relief;
    // Maillages construits en parallèle (lecture seule), puis remplacés
    let built: Vec<(Entity, MeshData)> = redo
        .into_iter()
        .collect::<Vec<Hex>>()
        .par_iter()
        .map(|&hex| {
            let (shape, e) = &relief.water[&hex];
            // Case inexplorée qui porte une rivière d'arête visible (sa rive voisine est
            // explorée) : elle n'a pas de lits à elle, on prend ceux des voisines, qui ont taillé
            // le lit (sinon l'eau, à une hauteur fixe, passait sous la berge)
            let near: Vec<Channel> = match relief.channels.get(&hex) {
                Some(c) => c.clone(),
                None => {
                    let mut v: Vec<Channel> = Vec::new();
                    for seen in hex.range(1) {
                        // (lits d'une voisine au-delà du raccord : décalés d'un tour)
                        let shift = Vec2::new(map.0.wrap().offset(seen), 0.0);
                        for ch in relief.channels.get(&map.0.canonical(seen)).into_iter().flatten() {
                            let ch = ch.shifted(shift);
                            if !v.contains(&ch) {
                                v.push(ch);
                            }
                        }
                    }
                    v
                }
            };
            (*e, water_mesh(relief, &near, shape, shape.visible(hex, explored), explored))
        })
        .collect();
    for (e, data) in built {
        if let Ok(mesh) = water_meshes.get(e)
            && let Some(mut m) = meshes.get_mut(&mesh.0)
        {
            *m = to_mesh(data);
        }
    }
    // Coût des surfaces d'eau par case traitée (pour le budget des images suivantes)
    if !processed.is_empty() {
        queue.water_ms = smooth(queue.water_ms, water_started.elapsed().as_secs_f32() * 1000.0 / processed.len() as f32);
    }
}

/// Moyenne glissante d'une durée mesurée à chaque image.
fn smooth(average: f32, sample: f32) -> f32 {
    if average == 0.0 { sample } else { average + (sample - average) * 0.2 }
}

/// Maillage des cours d'eau d'une case : surface plate en travers du lit, un peu sous le niveau
/// du sol (prise sur l'axe du cours d'eau, lissée le long du cours), couleur selon le type.
fn water_mesh<'a>(
    relief: &Relief,
    near: &[Channel],
    shape: &WaterShape,
    tris: impl Iterator<Item = &'a crate::waterways::WaterTri>,
    explored: impl Fn(Hex) -> bool,
) -> MeshData {
    let mut data = MeshData::default();
    let mut channels: HashMap<usize, Channel> = HashMap::new();
    let linear = |c: Color| {
        let l = c.to_linear();
        [l.red, l.green, l.blue, 1.0]
    };
    // Eau douce : la couleur des lacs ; en arrivant dans la mer ou un lac, elle prend
    // progressivement la couleur de leur eau (l'embouchure se fond dans l'étendue d'eau)
    let fresh = terrain_color(Terrain::Lake);
    let tint = |p: Vec2| {
        let c = match relief.nearest_water(p) {
            Some((d, water)) => fresh.lerp(water, 1.0 - smoothstep01(d / MOUTH_BLEND)),
            None => fresh,
        };
        [c.x, c.y, c.z, 1.0]
    };
    // L'eau s'arrête net au bord de la zone explorée (une rivière d'arête se voit dès qu'une
    // rive est explorée : seule sa moitié côté exploré est dessinée)
    let pieces = tris.flat_map(|t| crate::waterways::clip_to_explored(t.pts, &explored).into_iter().map(move |pts| (t, pts)));
    for (t, [a, b, c]) in pieces {
        // Sens trigonométrique vu du dessus (repère 3D : z = −y)
        let ccw = (b - a).perp_dot(c - a) > 0.0;
        let pts = if ccw { [a, b, c] } else { [a, c, b] };
        // Même surface que celle qui a servi à tailler le lit (lits de la case et des voisines) ;
        // hors de portée des lits, la hauteur du tronçon (calcul coûteux : seulement dans ce cas)
        let ys = pts.map(|p| water_surface(near, p).unwrap_or_else(|| channels.entry(t.channel).or_insert_with(|| relief.channel(&shape.channels[t.channel])).levels[0]));
        // Chute d'eau : l'eau descend très fort le long du cours (elle franchit une falaise) →
        // couleur de l'écume
        let hi = (0..3).max_by(|&i, &j| ys[i].total_cmp(&ys[j])).unwrap();
        let lo = (0..3).min_by(|&i, &j| ys[i].total_cmp(&ys[j])).unwrap();
        let rise = ys[hi] - ys[lo];
        let foam = rise > CASCADE_MIN_DROP && rise > 1.2 * pts[hi].distance(pts[lo]);
        for (p, y) in pts.into_iter().zip(ys) {
            data.positions.push([p.x, y, -p.y]);
            data.normals.push([0.0, 1.0, 0.0]);
            data.colors.push(if foam { linear(Color::srgb(0.86, 0.93, 0.98)) } else { tint(p) });
            data.indices.push(data.indices.len() as u32);
        }
    }
    data
}

/// Hachures d'une case posées sur son sol (bandes un peu au-dessus du relief).
fn hatch_mesh(relief: &Relief, hex: Hex, color: Color) -> MeshData {
    let l = color.to_linear();
    let c = [l.red, l.green, l.blue, 1.0];
    let mut data = MeshData::default();
    for strip in crate::render::hatch_strips(hex, 6) {
        let base = data.positions.len() as u32;
        for &(p, q) in &strip {
            for v in [p, q] {
                data.positions.push(relief.on_ground(v.x, v.y, 0.45).to_array());
                data.normals.push([0.0, 1.0, 0.0]);
                data.colors.push(c);
            }
        }
        for k in 0..strip.len() as u32 - 1 {
            let (p0, q0, p1, q1) = (base + 2 * k, base + 2 * k + 1, base + 2 * k + 2, base + 2 * k + 3);
            // Sens trigonométrique vu du dessus, quel que soit le sens de la bande
            let a = Vec2::new(data.positions[p0 as usize][0], -data.positions[p0 as usize][2]);
            let b = Vec2::new(data.positions[p1 as usize][0], -data.positions[p1 as usize][2]);
            let cq = Vec2::new(data.positions[q1 as usize][0], -data.positions[q1 as usize][2]);
            if (b - a).perp_dot(cq - a) > 0.0 {
                data.indices.extend([p0, p1, q1, p0, q1, q0]);
            } else {
                data.indices.extend([p0, q1, p1, p0, q0, q1]);
            }
        }
    }
    if data.positions.is_empty() {
        // Maillage vide interdit : un triangle dégénéré invisible
        for _ in 0..3 {
            data.positions.push([0.0; 3]);
            data.normals.push([0.0, 1.0, 0.0]);
            data.colors.push([0.0; 4]);
            data.indices.push(data.indices.len() as u32);
        }
    }
    data
}

/// Cases du relief 3D en attente de mise à jour.
#[derive(Default)]
struct ReliefQueue {
    explored: HashSet<Hex>,
    sites: HashSet<Hex>,
    /// Relief à recalculer (et couleurs).
    geometry: HashSet<Hex>,
    /// Couleurs seulement (changement de carte).
    color: HashSet<Hex>,
    /// Hauteurs d'eau des lits à revoir (relief voisin changé) : surfaces d'eau à refaire aussi.
    levels: HashSet<Hex>,
    /// Ordre de traitement : cases en attente (relief ou couleurs), la plus proche du centre de la
    /// vue au sommet. Une case peut y être plusieurs fois, ou n'être plus en attente : elle est
    /// alors sautée en sortant.
    order: BinaryHeap<Reverse<(i32, Hex)>>,
    /// Centre de la vue quand l'ordre a été établi (`None` : à refaire).
    order_focus: Option<Hex>,
    /// Durée moyenne d'un paquet, et des surfaces d'eau par case traitée (ms).
    chunk_ms: f32,
    water_ms: f32,
    /// Part du budget retirée après des images trop longues (0 : budget entier).
    slowdown: f32,
}

impl ReliefQueue {
    /// Met une case dans l'ordre de traitement, à sa distance du centre de la vue.
    fn push(&mut self, hex: Hex, distance: i32) {
        self.order.push(Reverse((distance, hex)));
    }

    /// Refait l'ordre de traitement de toutes les cases en attente autour d'un nouveau centre.
    fn reorder(&mut self, focus: Hex, distance: impl Fn(Hex) -> i32) {
        let all: Vec<Reverse<(i32, Hex)>> = self.geometry.iter().chain(self.color.difference(&self.geometry)).map(|&h| Reverse((distance(h), h))).collect();
        self.order = BinaryHeap::from(all);
        self.order_focus = Some(focus);
    }

    /// Prochaines cases à traiter (au plus `size`), les plus proches d'abord.
    fn next_chunk(&mut self, size: usize) -> Vec<Hex> {
        let mut chunk = Vec::with_capacity(size);
        while chunk.len() < size
            && let Some(Reverse((_, hex))) = self.order.pop()
        {
            if (self.geometry.contains(&hex) || self.color.contains(&hex)) && !chunk.contains(&hex) {
                chunk.push(hex);
            }
        }
        chunk
    }
}

/// Temps de calcul maximal du relief 3D par image (ms).
const RELIEF_BUDGET_MS: f32 = 6.0;
/// Temps de préparation du relief 3D par image pendant qu'on joue en 2D (ms).
const RELIEF_BUDGET_2D_MS: f32 = 3.0;
/// Durée d'image au-delà de laquelle le budget du relief est réduit (un peu plus qu'une image à
/// 60 par seconde), et réduction maximale.
const RELIEF_SLOW_FRAME_MS: f32 = 17.5;
const RELIEF_SLOWDOWN_MAX: f32 = 0.75;
/// Cases préparées puis construites ensemble (en parallèle), par cœur.
const RELIEF_CHUNK_PER_THREAD: usize = 2;
/// Déplacement du centre de la vue (en cases) au-delà duquel l'ordre de traitement est refait.
const RELIEF_REFOCUS: i32 = 3;

/// Décor visible sur les cases explorées, sauf sous une ville, un village ou un camp
/// (il réapparaît quand le village ou le camp disparaît).
fn update_decor_3d(
    fog: Res<ShownFog>,
    structures: Query<&HexPos, Or<(With<City>, With<Village>, With<Camp>)>>,
    added: Query<(), Or<(Added<City>, Added<Village>, Added<Camp>)>>,
    mut removed_villages: RemovedComponents<Village>,
    mut removed_camps: RemovedComponents<Camp>,
    mut decor: Query<(&Decor3d, &mut Visibility)>,
    layer: Res<MapLayer>,
) {
    let removed = removed_villages.read().count() + removed_camps.read().count() > 0;
    if !fog.is_changed() && added.is_empty() && !removed && !layer.is_changed() {
        return;
    }
    let topo = layer.thematic();
    let occupied: bevy::platform::collections::HashSet<Hex> = structures.iter().map(|p| p.0).collect();
    for (d, mut vis) in &mut decor {
        let shown = fog.explored.contains(&d.hex)
            && d.requires.is_none_or(|r| fog.explored.contains(&r))
            && !(topo && d.vegetation)
            && !occupied.contains(&d.hex);
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
    }
}

/// Crée la représentation 3D des nouvelles unités, villes, villages et camps.
fn spawn_mirrors(
    mut commands: Commands,
    assets: Option<Res<Assets3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    units: Query<(Entity, &Unit, Has<Hostile>), Added<Unit>>,
    cities: Query<Entity, Added<City>>,
    villages: Query<Entity, Added<Village>>,
    camps: Query<Entity, Added<Camp>>,
) {
    let Some(a) = assets else { return };
    let upright = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for (source, unit, hostile) in &units {
        let material = if hostile { &a.hostile } else { &a.player };
        // Socle aux couleurs du camp (incliné comme le sol), figurine debout dessus, icône du type
        // au-dessus en vue lointaine
        commands
            .spawn((Mirror3d { source, is_unit: true, placed: None }, Transform::default(), Visibility::Hidden, layer()))
            .with_children(|p| {
                p.spawn((Socle3d, Mesh3d(a.socle.clone()), MeshMaterial3d(material.clone()), Transform::default(), Visibility::Inherited, layer()));
                // Barque (visible sur un fleuve) : coque et deux pointes
                p.spawn((Boat3d, Transform::default(), Visibility::Hidden, layer())).with_children(|b| {
                    let wood = MeshMaterial3d(a.boat_wood.clone());
                    b.spawn((Mesh3d(a.boat_hull.clone()), wood.clone(), Transform::default(), layer()));
                    for z in [6.5, -6.5] {
                        b.spawn((Mesh3d(a.boat_end.clone()), wood.clone(), Transform::from_xyz(0.0, 0.0, z).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)), layer()));
                    }
                });
                for at in crate::figures::group(unit.0.kind) {
                    p.spawn((Member { at, base: local3(at) }, Transform::from_translation(local3(at)), Visibility::Inherited, layer()))
                        .with_children(|f| crate::figures::spawn_figure(f, &a.figures, unit.0.kind, hostile, &layer()));
                }
                p.spawn((
                    FarIcon,
                    Mesh3d(a.icon_quad.clone()),
                    MeshMaterial3d(a.icons[unit.0.kind.icon()].clone()),
                    Transform::from_xyz(0.0, FAR_ICON_LIFT, 0.0).with_rotation(upright),
                    Visibility::Hidden,
                    layer(),
                ))
                .with_child((Mesh3d(a.icon_disc.clone()), MeshMaterial3d(a.icon_backs[usize::from(hostile)].clone()), Transform::from_xyz(0.0, 0.0, -0.2), layer()));
            });
    }
    for source in &cities {
        spawn_city_model(&mut commands, &a, &mut meshes, source);
    }
    for source in &villages {
        spawn_site_model(&mut commands, &a, &mut meshes, source, false);
    }
    for source in &camps {
        spawn_site_model(&mut commands, &a, &mut meshes, source, true);
    }
}

/// Direction (repère 2D) d'un angle en degrés.
fn dir2(deg: f32) -> Vec2 {
    Vec2::from_angle(deg.to_radians())
}

/// Point 3D local (sans hauteur) d'une position 2D par rapport au centre de la case.
fn local3(at: Vec2) -> Vec3 {
    Vec3::new(at.x, 0.0, -at.y)
}

/// Élément posé sur le sol : maillage, matière, position 2D, emprise, façon de le poser, rotation.
fn grounded(mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, at: Vec2, reach: f32, rest: Rest, turn: Quat) -> (Grounded, Mesh3d, MeshMaterial3d<StandardMaterial>, Transform, RenderLayers) {
    (Grounded { at, reach, rest }, Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::from_translation(local3(at)).with_rotation(turn), layer())
}

/// Sol drapé sur le relief (maillage propre au site, refait quand il est replacé).
fn patch(meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, outline: Vec<Vec2>) -> impl Bundle {
    (Patch3d { outline }, Mesh3d(meshes.add(to_mesh(MeshData::default()))), MeshMaterial3d(material.clone()), Transform::default(), layer())
}

/// Contour d'un polygone régulier (rayon, nombre de côtés, angle du premier sommet), chaque côté
/// découpé en `steps` segments.
fn outline(radius: f32, sides: usize, start: f32, steps: usize) -> Vec<Vec2> {
    let corner = |k: usize| dir2(start + 360.0 * k as f32 / sides as f32) * radius;
    (0..sides).flat_map(|k| (0..steps).map(move |s| corner(k).lerp(corner(k + 1), s as f32 / steps as f32))).collect()
}

/// Village : huttes rondes au toit de chaume autour d'un feu.
/// Camp hostile : palissade de pieux, trois tentes rouges, un feu et un étendard, sur un sol de
/// terre battue. Chaque élément est posé sur le sol à sa place.
fn spawn_site_model(commands: &mut Commands, a: &Assets3d, meshes: &mut Assets<Mesh>, source: Entity, is_camp: bool) {
    let k = &a.sites;
    let none = Quat::IDENTITY;
    commands
        .spawn((Mirror3d { source, is_unit: false, placed: None }, Transform::default(), Visibility::Hidden, layer()))
        .with_children(|p| {
            if is_camp {
                p.spawn(patch(meshes, &k.camp_ground, outline(25.0, 24, 0.0, 1)));
                for i in 0..18 {
                    let at = dir2(20.0 * i as f32) * 22.5;
                    p.spawn(grounded(&k.stake, &k.wood, at, 1.3, Rest::Stand { height: 8.0 }, none));
                    p.spawn(grounded(&k.stake_tip, &k.wood, at, 1.3, Rest::Top { lift: 9.25 }, none));
                }
                for i in 0..3 {
                    let angle = 90.0 + 120.0 * i as f32;
                    p.spawn(grounded(&k.tent, &k.tent_red, dir2(angle) * 11.5, 7.0, Rest::Low { lift: 5.0 }, Quat::from_rotation_y(angle.to_radians())));
                }
                // Étendard planté près du feu (le drapeau suit le mât)
                let pole = dir2(30.0) * 6.0;
                p.spawn(grounded(&k.pole, &k.wood, pole, 0.6, Rest::Stand { height: 20.0 }, none));
                let mut flag = grounded(&k.flag, &k.tent_red, pole, 0.6, Rest::Top { lift: 17.0 }, none);
                flag.3.translation.x += 4.0;
                p.spawn(flag);
            } else {
                for i in 0..5 {
                    let at = dir2(90.0 + 72.0 * i as f32) * 14.0;
                    p.spawn(grounded(&k.hut, &k.mud, at, 4.8, Rest::Stand { height: 5.0 }, none));
                    p.spawn(grounded(&k.hut_roof, &k.straw, at, 4.8, Rest::Top { lift: 8.75 }, none));
                }
            }
            p.spawn(grounded(&k.fire, &k.flame, Vec2::ZERO, 2.4, Rest::Low { lift: 2.5 }, none));
            for i in 0..6 {
                p.spawn(grounded(&k.fire_stone, &k.stone, dir2(60.0 * i as f32) * 3.4, 1.2, Rest::Low { lift: 0.6 }, none));
            }
        });
}

/// Ville 3D : sol pavé, remparts hexagonaux avec tours d'angle aux toits bleus, maisons entre les
/// remparts et le donjon central, sur lequel se tient la garnison. Elle suit le terrain : chaque
/// pan de rempart, tour, maison est posé sur le sol à sa place, le donjon au centre.
fn spawn_city_model(commands: &mut Commands, a: &Assets3d, meshes: &mut Assets<Mesh>, source: Entity) {
    let k = &a.city;
    let none = Quat::IDENTITY;
    commands
        .spawn((Mirror3d { source, is_unit: false, placed: None }, Transform::default(), Visibility::Hidden, layer()))
        .with_children(|p| {
            p.spawn(patch(meshes, &k.paving, outline(HEX_SIZE * 0.9, 6, 90.0, 4)));
            for i in 0..6 {
                // Remparts au milieu des côtés, tours aux sommets (comme les cases)
                let side = 60.0 * i as f32;
                let wall = dir2(side) * CITY_WALL_RADIUS * 0.866;
                p.spawn(grounded(&k.wall, &k.stone, wall, CITY_WALL_RADIUS / 2.0, Rest::Stand { height: CITY_WALL_HEIGHT }, Quat::from_rotation_y((side + 90.0).to_radians())));
                let corner = dir2(side + 30.0) * CITY_WALL_RADIUS;
                p.spawn(grounded(&k.tower, &k.stone, corner, 4.0, Rest::Stand { height: CITY_TOWER_HEIGHT }, none));
                p.spawn(grounded(&k.tower_roof, &a.player, corner, 4.0, Rest::Top { lift: CITY_TOWER_HEIGHT + 3.0 }, none));
                let house = dir2(side) * 16.5;
                let turn = Quat::from_rotation_y((side + 45.0).to_radians());
                p.spawn(grounded(&k.house, &k.plaster, house, 3.5, Rest::Stand { height: 5.0 }, turn));
                p.spawn(grounded(&k.house_roof, &k.tiles, house, 3.5, Rest::Top { lift: 7.0 }, turn));
            }
            p.spawn(grounded(&k.keep, &k.stone, Vec2::ZERO, CITY_KEEP_RADIUS, Rest::Stand { height: CITY_HEIGHT - CITY_KEEP_ROOF }, none));
            p.spawn(grounded(&k.keep_roof, &a.player, Vec2::ZERO, CITY_KEEP_RADIUS, Rest::Top { lift: CITY_HEIGHT - CITY_KEEP_ROOF / 2.0 }, none));
        });
}

/// Points le plus bas et le plus haut du sol sous une emprise (centre et six points à `reach`).
fn footprint(relief: &Relief, c: Vec2, reach: f32) -> (f32, f32) {
    let samples = std::iter::once(c).chain((0..6).map(|k| c + dir2(60.0 * k as f32) * reach));
    samples.map(|p| relief.ground(p.x, p.y)).fold((f32::MAX, f32::MIN), |(lo, hi), g| (lo.min(g), hi.max(g)))
}

/// Pente du sol en un point : normale 3D (calculée sur ± `reach`).
fn ground_normal(relief: &Relief, c: Vec2, reach: f32) -> Vec3 {
    let g = |dx: f32, dy: f32| relief.ground(c.x + dx, c.y + dy);
    let gx = (g(reach, 0.0) - g(-reach, 0.0)) / (2.0 * reach);
    let gy = (g(0.0, reach) - g(0.0, -reach)) / (2.0 * reach);
    // (repère 3D : z = −y)
    Vec3::new(-gx, 1.0, gy).normalize()
}

/// Sol drapé sur le relief à l'intérieur d'un contour (anneaux concentriques), en repère local
/// (origine `origin`, point 3D du centre du site).
fn patch_mesh(relief: &Relief, center: Vec2, origin: Vec3, outline: &[Vec2]) -> MeshData {
    const RINGS: usize = 5;
    let mut data = MeshData::default();
    let push = |p: Vec2, data: &mut MeshData| {
        let q = relief.on_ground(center.x + p.x, center.y + p.y, 0.35) - origin;
        data.positions.push(q.to_array());
        data.normals.push([0.0, 1.0, 0.0]);
        data.colors.push([1.0; 4]);
    };
    push(Vec2::ZERO, &mut data);
    let n = outline.len() as u32;
    for r in 1..=RINGS {
        let t = r as f32 / RINGS as f32;
        for &o in outline {
            push(o * t, &mut data);
        }
    }
    // Sens trigonométrique vu du dessus (repère 2D)
    for k in 0..n {
        data.indices.extend([0, 1 + k, 1 + (k + 1) % n]);
    }
    for r in 0..RINGS as u32 - 1 {
        let (inner, outer) = (1 + r * n, 1 + (r + 1) * n);
        for k in 0..n {
            let k1 = (k + 1) % n;
            data.indices.extend([inner + k, outer + k, outer + k1, inner + k, outer + k1, inner + k1]);
        }
    }
    data
}

/// Position et visibilité des représentations 3D ; suppression quand l'entité du jeu disparaît.
/// Chacune est posée sur le vrai sol 3D, et replacée quand le sol de sa case est refait.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_mirrors(
    mut commands: Commands,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    sources: Query<(&HexPos, Has<Hostile>, Has<Village>, Has<Camp>)>,
    cities: Query<&HexPos, With<City>>,
    mut mirrors: Query<(Entity, &mut Mirror3d, &mut Transform, &mut Visibility, Option<&Children>)>,
    mut parts: Query<(&Grounded, &mut Transform), Without<Mirror3d>>,
    mut socles: Query<(&mut Transform, &mut Visibility), (With<Socle3d>, Without<Mirror3d>, Without<Grounded>)>,
    mut boats: Query<(&mut Transform, &mut Visibility), (With<Boat3d>, Without<Socle3d>, Without<Mirror3d>, Without<Grounded>)>,
    mut members: Query<(&mut Member, &mut Transform), (Without<Boat3d>, Without<Socle3d>, Without<Mirror3d>, Without<Grounded>)>,
    patches: Query<(&Patch3d, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(relief) = relief else { return };
    let city_tiles: HashSet<Hex> = cities.iter().map(|p| map.0.canonical(p.0)).collect();
    for (entity, mut mirror, mut transform, mut vis, children) in &mut mirrors {
        let Ok((pos, hostile, village, camp)) = sources.get(mirror.source) else {
            commands.entity(entity).despawn();
            continue;
        };
        let hex = map.0.canonical(pos.0);
        let on_roof = mirror.is_unit && city_tiles.contains(&hex);
        let key = (hex, relief.generation.get(&hex).copied().unwrap_or(0), on_roof);
        if mirror.placed != Some(key) {
            mirror.placed = Some(key);
            let (cx, cy) = pos.0.to_world();
            let c = Vec2::new(cx, cy);
            let center = relief.ground(cx, cy);
            let children: Vec<Entity> = children.map(|c| c.iter().collect()).unwrap_or_default();
            if mirror.is_unit {
                // Sur un fleuve : dans une barque, à la surface de l'eau, dans le sens du courant
                let river = relief.channels.get(&hex).filter(|_| !on_roof && map.0.is_fleuve(hex));
                let afloat = river.and_then(|near| {
                    let water = crate::relief::water_surface(near, c)?;
                    let axis = near.iter().filter(|ch| !ch.is_disc()).min_by(|a, b| c.distance(a.nearest(c)).total_cmp(&c.distance(b.nearest(c))))?;
                    let d = axis.b - axis.a;
                    Some((water, Quat::from_rotation_y(d.x.atan2(-d.y))))
                });
                // Groupe : chaque figurine a ses pieds au plus haut du sol sous elle (jamais
                // enfoncée), ou toutes sur le toit du donjon, ou sur le pont de leur barque (le
                // groupe resserré) ; anneau au sol incliné comme le sol
                let root = match (on_roof, afloat) {
                    (true, _) => footprint(&relief, c, CITY_KEEP_RADIUS).1 + CITY_HEIGHT,
                    (false, Some((water, _))) => water + BOAT_DECK,
                    (false, None) => footprint(&relief, c, FIGURE_REACH).1,
                };
                transform.translation = Vec3::new(cx, root, -cy);
                let ring = (center - root + 0.3, Quat::from_rotation_arc(Vec3::Y, ground_normal(&relief, c, SOCLE_RADIUS)));
                for &child in &children {
                    if let Ok((mut t, mut v)) = socles.get_mut(child) {
                        // (pas d'anneau sur un toit ni sur l'eau)
                        *v = if on_roof || afloat.is_some() { Visibility::Hidden } else { Visibility::Inherited };
                        (t.translation.y, t.rotation) = ring;
                    } else if let Ok((mut t, mut v)) = boats.get_mut(child) {
                        *v = if afloat.is_some() { Visibility::Inherited } else { Visibility::Hidden };
                        if let Some((water, turn)) = afloat {
                            t.translation.y = water + BOAT_HULL_LIFT - root;
                            t.rotation = turn;
                        }
                    } else if let Ok((mut m, mut t)) = members.get_mut(child) {
                        m.base = match afloat {
                            Some((_, turn)) => turn * local3(m.at * BOAT_SQUEEZE),
                            None if on_roof => local3(m.at),
                            None => local3(m.at) + Vec3::Y * (footprint(&relief, c + m.at, FIGURE_REACH).1 - root),
                        };
                        let k = t.scale.x;
                        t.translation = Vec3::new(m.base.x * k, m.base.y, m.base.z * k);
                    }
                }
            } else {
                transform.translation = Vec3::new(cx, center, -cy);
                for &child in &children {
                    if let Ok((g, mut t)) = parts.get_mut(child) {
                        let (lo, hi) = footprint(&relief, c + g.at, g.reach);
                        match g.rest {
                            Rest::Stand { height } => {
                                let (bottom, top) = (lo, hi + height);
                                t.translation.y = (bottom + top) / 2.0 - center;
                                t.scale.y = (top - bottom) / height;
                            }
                            Rest::Top { lift } => t.translation.y = hi + lift - center,
                            Rest::Low { lift } => t.translation.y = lo + lift - center,
                        }
                    } else if let Ok((p, mesh)) = patches.get(child)
                        && let Some(mut m) = meshes.get_mut(&mesh.0)
                    {
                        *m = to_mesh(patch_mesh(&relief, c, Vec3::new(cx, center, -cy), &p.outline));
                    }
                }
            }
        }
        let visible = if hostile {
            fog.visible.contains(&pos.0)
        } else if village || camp {
            fog.explored.contains(&pos.0)
        } else {
            true
        };
        let wanted_vis = if visible { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted_vis {
            *vis = wanted_vis;
        }
    }
}

/// Taille des figurines (avec leur anneau et leur barque) selon la distance de la caméra 3D : elles
/// restent lisibles de loin.
#[allow(clippy::type_complexity)]
fn scale_units(mode: Res<ViewMode>, cam: Res<Cam3d>, mut parts: Query<(&mut Transform, Option<&Member>), Or<(With<Member>, With<Socle3d>, With<Boat3d>)>>) {
    if !mode.is_3d {
        return;
    }
    let k = (cam.distance / UNIT_SCALE_FROM).clamp(1.0, UNIT_SCALE_MAX);
    let size = Vec3::splat(k);
    for (mut t, member) in &mut parts {
        if t.scale != size {
            t.scale = size;
            // (le groupe s'écarte d'autant : les figurines ne se chevauchent pas)
            if let Some(m) = member {
                t.translation = Vec3::new(m.base.x * k, m.base.y, m.base.z * k);
            }
        }
    }
}

/// Icônes des unités en vue lointaine : tournées vers la caméra, cachées de près (la figurine
/// suffit).
fn far_icons(mode: Res<ViewMode>, cam: Res<Cam3d>, mut icons: Query<(&mut Transform, &mut Visibility), With<FarIcon>>) {
    if !mode.is_3d {
        return;
    }
    let far = cam.distance > FAR_ICON_DISTANCE;
    let facing = camera_transform(&cam).rotation;
    // (taille à l'écran à peu près constante : agrandie avec la distance)
    let size = Vec3::splat(cam.distance / FAR_ICON_DISTANCE * FAR_ICON_SCALE);
    for (mut t, mut vis) in &mut icons {
        let wanted = if far { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
        if far && (t.rotation != facing || t.scale != size) {
            t.rotation = facing;
            t.scale = size;
            t.translation.y = FAR_ICON_LIFT * size.y;
        }
    }
}

/// Lignes 3D : frontières, zone accessible, cibles attaquables, sélection, trajets.
fn draw_overlays_3d(
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    mut gizmos: Gizmos<Gizmos3d>,
    relief: Option<Res<Relief>>,
    territory: Res<Territory>,
    selection: Res<Selection>,
    area: Res<ReachArea>,
    hover: Res<HoverPath>,
    order: Res<OrderPath>,
    positions: Query<&HexPos>,
) {
    let Some(relief) = relief else { return };
    if !mode.is_3d {
        return;
    }
    // Les lignes épousent le relief : on échantillonne le sol le long de chaque tracé
    let edge = |gizmos: &mut Gizmos<Gizmos3d>, h: Hex, dir: usize, color: Color| {
        let ((ax, ay), (bx, by)) = h.edge_corners(dir);
        let (a, b) = (Vec2::new(ax, ay), Vec2::new(bx, by));
        // Légèrement en retrait vers le centre : on suit le bord de cette case (côté falaise compris)
        let (cx, cy) = h.to_world();
        let inset = |p: Vec2| p.lerp(Vec2::new(cx, cy), 0.04);
        let points = (0..=6).map(|k| {
            let p = inset(a.lerp(b, k as f32 / 6.0));
            relief.on_ground(p.x, p.y, 1.0)
        });
        gizmos.linestrip(points, color);
    };
    for (&hex, &owner) in &territory.0 {
        for (dir, d) in DIRECTIONS.iter().enumerate() {
            if territory.0.get(&map.0.canonical(hex + *d)) != Some(&owner) {
                edge(&mut gizmos, hex, dir, Color::srgb(0.45, 0.70, 1.0));
            }
        }
    }
    for &hex in &area.cells {
        for (dir, d) in DIRECTIONS.iter().enumerate() {
            if !area.cells.contains(&map.0.canonical(hex + *d)) {
                edge(&mut gizmos, hex, dir, Color::srgb(1.0, 0.97, 0.75));
            }
        }
    }
    let ground_circle = |gizmos: &mut Gizmos<Gizmos3d>, h: Hex, lift: f32, r: f32, color: Color| {
        let (cx, cy) = h.to_world();
        let points = (0..=32).map(|k| {
            let p = Vec2::new(cx, cy) + Vec2::from_angle(k as f32 / 32.0 * std::f32::consts::TAU) * r;
            relief.on_ground(p.x, p.y, lift)
        });
        gizmos.linestrip(points, color);
    };
    for &t in &area.attackable {
        ground_circle(&mut gizmos, t, 9.0, HEX_SIZE * 0.72, Color::srgb(1.0, 0.25, 0.2));
    }
    let Some(selected) = selection.0 else { return };
    let Ok(pos) = positions.get(selected) else { return };
    ground_circle(&mut gizmos, pos.0, 2.0, HEX_SIZE * 0.85, Color::srgb(1.0, 0.9, 0.2));
    let path_line = |gizmos: &mut Gizmos<Gizmos3d>, path: &[Hex], color: Color| {
        // Chaque case prise du côté du raccord le plus proche de la précédente (tracé continu)
        let mut prev = pos.0;
        let centers: Vec<Vec2> = std::iter::once(pos.0)
            .chain(path.iter().copied())
            .map(|h| {
                prev = map.0.wrap().nearest(prev, h);
                let (x, y) = prev.to_world();
                Vec2::new(x, y)
            })
            .collect();
        let points = centers.windows(2).flat_map(|w| (0..4).map(move |k| w[0].lerp(w[1], k as f32 / 4.0)));
        let last = *centers.last().unwrap();
        let points = points.chain(std::iter::once(last)).map(|p| relief.on_ground(p.x, p.y, 7.0));
        gizmos.linestrip(points, color);
    };
    if !order.path.is_empty() {
        path_line(&mut gizmos, &order.path, Color::srgb(0.4, 0.8, 1.0));
    }
    if !hover.path.is_empty() && hover.path != order.path {
        path_line(&mut gizmos, &hover.path, Color::WHITE);
    }
}

/// Carte du climat en 3D : flèches des vents dominants, posées au-dessus du relief.
fn draw_wind_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    let Some(relief) = relief else { return };
    if !mode.is_3d || *layer != MapLayer::Climate {
        return;
    }
    for &hex in fog.explored.iter().filter(|&&h| wind_arrow_site(h)) {
        if let Some((from, to)) = wind_arrow(&map.0, hex) {
            let lift = |p: Vec2| relief.on_ground(p.x, p.y, 6.0);
            gizmos.arrow(lift(from), lift(to), Color::srgba(1.0, 1.0, 1.0, 0.85)).with_tip_length(8.0);
        }
    }
}

/// Carte des courants marins en 3D : flèches des courants, posées sur l'eau.
fn draw_currents_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    let Some(relief) = relief else { return };
    if !mode.is_3d || *layer != MapLayer::Currents {
        return;
    }
    for &hex in fog.explored.iter().filter(|&&h| wind_arrow_site(h)) {
        if let Some((from, to, color)) = current_arrow(&map.0, hex) {
            let lift = |p: Vec2| relief.on_ground(p.x, p.y, 2.0);
            gizmos.arrow(lift(from), lift(to), color).with_tip_length(7.0);
        }
    }
}

/// Case sous le curseur en vue 3D : on avance le long du rayon de la caméra, du haut du
/// relief vers le sol, jusqu'à passer sous le dessus de la case survolée (falaises comprises).
pub fn hovered_hex_3d(ray: Ray3d, relief: &Relief, map: &GameMap) -> Option<Hex> {
    let top = meters_to_height(4_800.0);
    let ground = Vec3::Y;
    let start = ray.intersect_plane(Vec3::new(0.0, top, 0.0), InfinitePlane3d::new(ground))?;
    let end = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(ground))?;
    let steps = ((end - start) / 2.0).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let p = ray.get_point(start + (end - start) * i as f32 / steps as f32);
        let h = map.0.canonical(Hex::from_world(p.x, -p.z));
        if map.0.contains(h) && p.y <= relief.ground(p.x, -p.z) {
            return Some(h);
        }
    }
    let p = ray.get_point(end);
    Some(map.0.canonical(Hex::from_world(p.x, -p.z))).filter(|h| map.0.contains(*h))
}

/// Copie, de l'autre côté du raccord d'une planète, du sol ou de l'eau d'une case du bord
/// (même maillage, même matériau, décalée d'un tour). Sans elle, la vue « fantôme » du bord
/// opposé (voir `seam.rs`) et la vue principale se touchent pile sur le raccord, et le lissage
/// des bords (MSAA) y laisse un fin liseré sombre en pointillés ; avec elle, chaque vue
/// déborde d'une colonne sur l'autre.
#[derive(Component)]
struct SeamCopy {
    source: Entity,
}

fn sync_seam_copies(
    mut commands: Commands,
    map: Res<GameMap>,
    relief: Option<Res<Relief>>,
    mut spawned: Local<bool>,
    sources: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &Visibility), Without<SeamCopy>>,
    mut copies: Query<(&SeamCopy, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
) {
    let Some(relief) = relief else { return };
    let wrap = map.0.wrap();
    if !wrap.wraps() {
        return;
    }
    if !*spawned {
        *spawned = true;
        let world = wrap.world_width();
        let edge = |h: &Hex| {
            let (c, _) = h.to_offset();
            if c == 0 { Some(world) } else if c == map.0.width - 1 { Some(-world) } else { None }
        };
        let entities = relief.tiles.iter().map(|(h, &e)| (h, e)).chain(relief.water.iter().map(|(h, (_, e))| (h, *e)));
        for (hex, source) in entities {
            let Some(shift) = edge(hex) else { continue };
            let Ok((mesh, material, _)) = sources.get(source) else { continue };
            commands.spawn((
                SeamCopy { source },
                mesh.clone(),
                material.clone(),
                Transform::from_xyz(shift, 0.0, 0.0),
                layer(),
            ));
        }
        return;
    }
    // Le maillage est partagé (mis à jour avec l'original) ; le matériau et la visibilité suivent
    for (copy, mut material, mut vis) in &mut copies {
        let Ok((_, source_material, source_vis)) = sources.get(copy.source) else { continue };
        if material.0 != source_material.0 {
            material.0 = source_material.0.clone();
        }
        if *vis != *source_vis {
            *vis = *source_vis;
        }
    }
}

/// Carte des plaques en 3D : frontières (traits épais posés sur le relief), mouvements des
/// plaques et points chauds, comme en 2D. Les tracés posés sur le sol sont gardés d'une image à
/// l'autre et recalculés seulement quand l'exploration ou la forme du sol changent.
fn draw_plates_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    mut cache: Local<PlatesCache>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    // (l'exploration peut changer pendant qu'une autre carte est affichée)
    if fog.is_changed() {
        cache.shape = None;
    }
    let Some(relief) = relief else { return };
    if !mode.is_3d || *layer != MapLayer::Plates {
        return;
    }
    if cache.shape != Some(relief.shape) {
        *cache = plates_3d(&map.0, &fog, &relief);
    }
    for (points, color) in &cache.lines {
        gizmos.linestrip(points.iter().copied(), *color);
    }
    for &(from, to) in &cache.arrows {
        gizmos.arrow(from, to, crate::render::PLATE_ARROW).with_tip_length(7.0);
    }
}

/// Tracés de la carte des plaques en 3D, posés sur le sol (forme du sol `shape`).
#[derive(Default)]
struct PlatesCache {
    shape: Option<u32>,
    lines: Vec<(Vec<Vec3>, Color)>,
    arrows: Vec<(Vec3, Vec3)>,
}

/// Frontières, mouvements et points chauds des plaques explorées, posés sur le relief.
fn plates_3d(m: &crate::map::WorldMap, fog: &ShownFog, relief: &Relief) -> PlatesCache {
    let lift = |p: Vec2, up: f32| relief.on_ground(p.x, p.y, up);
    let mut lines = Vec::new();
    for e in &m.plates().edges {
        let h = m.hex_at(e.cell);
        let other = m.canonical(h + DIRECTIONS[usize::from(e.dir)]);
        if !fog.explored.contains(&h) || !fog.explored.contains(&other) {
            continue;
        }
        let ((ax, ay), (bx, by)) = h.edge_corners(usize::from(e.dir));
        let (a, b) = (Vec2::new(ax, ay), Vec2::new(bx, by));
        let (r, g, bl) = e.kind.color();
        // Deux traits côte à côte : plus épais que les autres lignes
        for up in [1.5, 2.5] {
            lines.push(((0..=4).map(|k| lift(a.lerp(b, k as f32 / 4.0), up)).collect(), Color::srgb(r, g, bl)));
        }
    }
    let arrows = fog.explored.iter().filter(|&&h| wind_arrow_site(h)).filter_map(|&hex| plate_arrow(m, hex)).map(|(from, to)| (lift(from, 6.0), lift(to, 6.0))).collect();
    for &h in m.plates().hotspots.iter().filter(|h| fog.explored.contains(*h)) {
        let (x, y) = h.to_world();
        for radius in [0.45, 0.2] {
            let points = (0..=24).map(|k| lift(Vec2::new(x, y) + Vec2::from_angle(k as f32 / 24.0 * std::f32::consts::TAU) * HEX_SIZE * radius, 3.0)).collect();
            lines.push((points, crate::render::HOTSPOT));
        }
    }
    PlatesCache { shape: Some(relief.shape), lines, arrows }
}
