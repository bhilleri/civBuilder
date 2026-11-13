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
use crate::waterways::{WaterAxis, WaterKind, WaterShape, hex_gap, hex_water};

/// Couche de rendu du monde 3D (la 2D reste sur la couche 0).
pub const LAYER_3D: usize = 1;
/// Couche des copies du sol au raccord (`SeamCopy`), vue seulement par les caméras fantômes.
pub const LAYER_SEAM_COPY: usize = 2;
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
    explored: crate::cases::CaseSet,
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
const SITE_RELIEF: f32 = 0.75;

/// Relief interne des cases explorées, adouci sous les sites (`sites`).
fn explored_bumps(map: &crate::map::WorldMap, fog: &ShownFog, sites: &HashSet<Hex>) -> BumpMap {
    fog.explored
        .iter()
        .filter(|&h| map.contains(h))
        .filter_map(|h| {
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
        // (montagnes et pics ont un sommet au centre de la case, reliés en chaîne à leurs voisins
        // par une arête plus basse : pas de damier de cônes ; le pic est nettement plus haut)
        CellRelief::Hills => (5.0, 0.0),
        CellRelief::Rugged => (9.0, 0.0),
        CellRelief::Mountain => (19.0, 8.0),
        CellRelief::Peak => (16.0, 34.0),
    };
    let snowline = if map.get(hex) == Some(Terrain::Snow) {
        0.0
    } else {
        // (collines et reliefs bas ne sont enneigés que sur un terrain de neige)
        ((map.temperature(hex).unwrap_or(5.0) + 12.0) * 2.0).clamp(10.0, 40.0)
    };
    Some(Bumps { amp, summit, needle: map.relief(hex) == CellRelief::Peak, snowline })
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

    /// Mers et lacs explorés autour d'un point (à trois cases au plus) : centre et couleur.
    fn waters_around(&self, p: Vec2) -> Vec<(Vec2, Vec3)> {
        Hex::from_world(p.x, p.y)
            .range(3)
            .filter(|&h| self.water_seen.contains(&self.wrap.hex(h)))
            .filter_map(|h| {
                let (cx, cy) = h.to_world();
                self.water_colors.get(&self.wrap.hex(h)).map(|&c| (Vec2::new(cx, cy), c))
            })
            .collect()
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
                // (près de la mer, le lit ne descend pas sous son niveau : pas de tranchée dans la plage)
                let land = if surface <= SEA_LEVEL { land.max(surface - MOUTH_BELOW) } else { land };
                let f = 1.0 - smoothstep01(d / bed.far);
                land + (surface - MOUTH_BELOW - land) * f
            }
        }
    }

    /// Lit 3D d'un tronçon : hauteurs de l'eau le long de l'axe. Un affluent qui entre dans un
    /// fleuve passe progressivement de la hauteur d'une rivière à celle du fleuve.
    fn channel(&self, axis: &WaterAxis) -> Channel {
        // (un lit d'île n'a pas d'eau : pas de hauteur à calculer)
        if axis.isle.is_some() {
            return Channel { a: axis.a, b: axis.b, hw: axis.hw, hw_end: axis.hw_end, levels: [0.0; CHANNEL_LEVELS], bed: 0.0, isle: axis.isle };
        }
        let levels = std::array::from_fn(|k| {
            let t = k as f32 / (CHANNEL_LEVELS - 1) as f32;
            let p = axis.a.lerp(axis.b, t);
            let own = self.water_level(axis.kind, p);
            match axis.inflow {
                // (part du chemin vers le fleuve : du début à la fin du tronçon)
                Some([s0, s1]) => own + (self.water_level(WaterKind::Fleuve, p) - own) * (s0 + (s1 - s0) * t),
                None => own,
            }
        });
        let bed = if axis.inflow.is_some() { WaterKind::Fleuve.bed().bed } else { axis.kind.bed().bed };
        Channel { a: axis.a, b: axis.b, hw: axis.hw, hw_end: axis.hw_end, levels, bed, isle: axis.isle }
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
    /// Figurine d'unité (dans une ville, elle se tient sur la place, laissée libre).
    is_unit: bool,
    /// Dernier placement (case, reconstruction de son sol, stade et fortification d'une ville) :
    /// replacée quand il change.
    placed: Option<(Hex, u32, u8)>,
    /// Déplacée par une animation (`feedback`) : replacée sur sa case quand elle se termine.
    animated: bool,
}

/// Sol drapé sur le relief (terre battue d'une ville, d'un village ou d'un camp) : contour (repère
/// 2D), refait quand le site est replacé.
#[derive(Component)]
struct Patch3d {
    outline: Vec<Vec2>,
}

/// Pose d'un site, calculée par `settlements`.
enum SitePose {
    Town(crate::settlements::Layout),
    Camp(crate::settlements::CampLayout),
}

/// Ville, village ou camp posé par `settlements` : la pose est gardée et ne change qu'avec le stade
/// ou la fortification (les bâtiments ne bougent pas quand le sol alentour est refait).
#[derive(Component, Default)]
struct SiteModel {
    pose: Option<SitePose>,
    code: u8,
}

/// Maillage unique du bâti d'un site.
#[derive(Component)]
struct SiteBody;

/// Bouffée de fumée d'un camp : elle s'élève en boucle, plus épaisse avant la sortie d'un guerrier.
#[derive(Component)]
struct CampSmoke {
    source: Entity,
    index: usize,
}

/// Terrain d'une case vu par `settlements`, relatif au centre de la case.
struct ReliefSite<'a> {
    relief: &'a Relief,
    center: Vec2,
    hex: Hex,
    shoreline: crate::relief::Shoreline,
    /// Pont construit sur une rivière qui borde la case (ville : porte et tête de pont).
    built: Option<crate::settlements::Bridge>,
}

/// Rivage dessiné de la mer et des lacs autour d'un point : celui que le joueur voit.
fn shoreline_near(map: &crate::map::WorldMap, relief: &Relief, center: Vec2) -> crate::relief::Shoreline {
    let mut shoreline = crate::relief::Shoreline::default();
    let water = |h: Hex| relief.water_hexes.contains(&relief.wrap.hex(h));
    for h in Hex::from_world(center.x, center.y).range(3).filter(|&h| water(h)) {
        let (corners, edges) = crate::waterways::coast_warp(map, relief.wrap.hex(h));
        let shore = (0..6).filter(|&d| !water(h + crate::hex::DIRECTIONS[d])).fold(0u8, |bits, d| bits | 1 << d);
        let (x, y) = h.to_world();
        shoreline.add(Vec2::new(x, y), &corners, &edges, shore);
    }
    shoreline
}

impl crate::settlements::Site for ReliefSite<'_> {
    fn height(&self, p: Vec2) -> f32 {
        let q = self.center + p;
        self.relief.ground(q.x, q.y)
    }

    fn water_gap(&self, p: Vec2) -> f32 {
        let q = self.center + p;
        let river = self.relief.channels.get(&self.hex).map_or(f32::MAX, |near| near.iter().map(|c| c.water_gap(q)).fold(f32::MAX, f32::min));
        river.min(self.shoreline.gap(q))
    }

    fn river_near(&self, p: Vec2) -> Option<crate::settlements::RiverAxis> {
        let q = self.center + p;
        let near = self.relief.channels.get(&self.hex)?;
        let (point, dir, half_width) = near.iter().filter_map(|c| c.span_near(q)).min_by(|a, b| a.0.distance(q).total_cmp(&b.0.distance(q)))?;
        (point.distance(q) <= half_width + 2.0).then_some(crate::settlements::RiverAxis { point: point - self.center, dir, half_width })
    }

    fn built_bridge(&self) -> Option<crate::settlements::Bridge> {
        self.built
    }
}

/// Écart (unités du monde) entre le milieu d'un pont et la barque d'une unité dans l'eau de la même case.
const BRIDGE_WATER_GAP: f32 = 9.0;

/// Distance à laquelle on cherche la terre ferme de l'autre rive : rivière, fleuve.
const BRIDGE_BANK_RIVER: f32 = 14.0;
const BRIDGE_BANK_FLEUVE: f32 = 40.0;
/// Rayon dégagé de tout arbre et rocher autour d'un pont (le long du tablier et à ses bouts).
const BRIDGE_CLEARING: f32 = 4.5;
/// Un bout de pont repose sur un sol au moins à cette hauteur au-dessus de l'eau ; on le
/// prolonge vers la terre (de `BRIDGE_EXTEND_STEP` à la fois, jusqu'à `BRIDGE_EXTEND_MAX`) tant
/// que la berge est plus basse (fleuve dont l'eau monte sur une rive en pente douce).
const BRIDGE_ABOVE_WATER: f32 = 0.8;
const BRIDGE_EXTEND_STEP: f32 = 0.5;
const BRIDGE_EXTEND_MAX: f32 = 40.0;
/// Haut d'une berge raide : on la remonte (jusqu'à `BRIDGE_RIM_SEARCH`) tant que le sol monte de plus
/// de `BRIDGE_RIM_SLOPE` par unité ; on y pose le bout si elle s'élève de plus de `BRIDGE_RIM_RISE`.
const BRIDGE_RIM_SEARCH: f32 = 14.0;
const BRIDGE_RIM_SLOPE: f32 = 0.35;
const BRIDGE_RIM_RISE: f32 = 3.0;

/// Pose les deux bouts d'un pont sur une berge sèche : chacun est repoussé vers sa rive tant que
/// le sol y est sous la surface de l'eau (plus `BRIDGE_ABOVE_WATER`).
fn anchor_bridge(relief: &Relief, hex: Hex, origin: Vec2, mut bridge: crate::settlements::Bridge) -> crate::settlements::Bridge {
    let level = water_near(relief, hex, origin + (bridge.near + bridge.far) / 2.0);
    let out = (bridge.far - bridge.near).normalize_or_zero();
    let ground = |p: Vec2| relief.ground(origin.x + p.x, origin.y + p.y);
    for (end, dir) in [(&mut bridge.near, -out), (&mut bridge.far, out)] {
        let mut k = 0.0;
        while k < BRIDGE_EXTEND_MAX && ground(*end) < level + BRIDGE_ABOVE_WATER {
            *end += dir * BRIDGE_EXTEND_STEP;
            k += BRIDGE_EXTEND_STEP;
        }
        // Berge raide (gorge) : le bout monte jusqu'en haut, là où la pente s'adoucit, pour franchir la
        // gorge d'une seule portée plutôt que de rester au pied des parois
        let (start, mut rim, mut k) = (ground(*end), *end, 0.0);
        while k < BRIDGE_RIM_SEARCH {
            let next = rim + dir * BRIDGE_EXTEND_STEP;
            if ground(next) - ground(rim) < BRIDGE_RIM_SLOPE * BRIDGE_EXTEND_STEP {
                break;
            }
            rim = next;
            k += BRIDGE_EXTEND_STEP;
        }
        if ground(rim) - start > BRIDGE_RIM_RISE {
            *end = rim;
        }
    }
    bridge
}

/// Pose d'un pont construit en 3D : centre de la case qui sert d'origine, et pont relatif à ce
/// centre. Le pont est posé sur la terre ferme de chaque rive (`settlements::span_bridge`), selon
/// les lits creusés ; à défaut (lits pas encore calculés), un tablier droit par-dessus l'eau.
fn bridge_pose(map: &crate::map::WorldMap, relief: &Relief, at: crate::map::BridgeAt) -> (Vec2, crate::settlements::Bridge) {
    let wrap = map.wrap();
    let world = |h: Hex| {
        let (x, y) = h.to_world();
        Vec2::new(x, y)
    };
    match at {
        crate::map::BridgeAt::Edge { hex, dir } => {
            let c = world(hex);
            let other = world(wrap.nearest(hex, wrap.neighbor(hex, usize::from(dir))));
            let site = ReliefSite { relief, center: c, hex, shoreline: shoreline_near(map, relief, c), built: None };
            let toward = (other - c) / 2.0;
            let bridge = crate::settlements::span_bridge(&site, toward, None, BRIDGE_BANK_RIVER)
                .unwrap_or(crate::settlements::Bridge { near: toward * 0.7, far: toward * 1.3, half_width: crate::settlements::BRIDGE_HALF_WIDTH, head: false });
            (c, anchor_bridge(relief, hex, c, bridge))
        }
        crate::map::BridgeAt::Fleuve { hex, axis } => {
            let c = world(hex);
            let a = world(wrap.nearest(hex, wrap.neighbor(hex, usize::from(axis)))) - c;
            let site = ReliefSite { relief, center: c, hex, shoreline: shoreline_near(map, relief, c), built: None };
            // (perpendiculaire au courant, comme sur une rivière ; les rives reliées dans les règles
            // sont celles de ses deux bouts)
            let mut bridge = crate::settlements::span_bridge(&site, Vec2::ZERO, None, BRIDGE_BANK_FLEUVE)
                .unwrap_or(crate::settlements::Bridge { near: a * 0.55, far: -a * 0.55, half_width: crate::settlements::BRIDGE_HALF_WIDTH, head: false });
            // (le bout `near` du côté de la rive `axis`)
            if bridge.near.dot(a) < bridge.far.dot(a) {
                std::mem::swap(&mut bridge.near, &mut bridge.far);
            }
            (c, anchor_bridge(relief, hex, c, bridge))
        }
    }
}

/// Pont construit sur une rivière qui borde une ville, vu depuis son centre (le premier dans
/// l'ordre des directions) : la pose de la ville le dessine, avec sa porte et sa tête de pont.
fn city_bridge(map: &crate::map::WorldMap, relief: &Relief, hex: Hex) -> Option<(usize, crate::settlements::Bridge)> {
    let dir = (0..6).find(|&d| map.bridge_between(hex, map.wrap().neighbor(hex, d)))?;
    let at = map.bridge_site(hex, dir)?;
    let (_, bridge) = bridge_pose(map, relief, at);
    // (pose rapportée au centre de la ville, si le pont est rangé sous la case voisine)
    let stored = match at {
        crate::map::BridgeAt::Edge { hex, .. } | crate::map::BridgeAt::Fleuve { hex, .. } => hex,
    };
    let ((sx, sy), (x, y)) = (map.wrap().nearest(hex, stored).to_world(), hex.to_world());
    let shift = Vec2::new(sx - x, sy - y);
    let (near, far) = (bridge.near + shift, bridge.far + shift);
    let (near, far) = if near.length() <= far.length() { (near, far) } else { (far, near) };
    Some((dir, crate::settlements::Bridge { near, far, ..bridge }))
}

/// Pont construit, dessiné à part (ceux des villes sont dans leur pose).
#[derive(Component)]
struct Bridge3d;

/// Ponts construits en 3D : entité et clé de pose de chacun (génération du relief alentour, ville
/// au bout, rives explorées), et tracé de tous (y compris ceux des villes) pour dégager les arbres.
#[derive(Resource, Default)]
pub struct Bridges3d {
    drawn: HashMap<crate::map::BridgeAt, (Option<Entity>, (u32, bool))>,
    /// Bouts de chaque pont (repère 2D du monde).
    spans: Vec<(Vec2, Vec2)>,
    /// Bouts du tablier des ponts de fleuve, tels qu'ils sont dessinés (repère 2D du monde).
    decks: HashMap<Hex, (Vec2, Vec2)>,
}

impl Bridges3d {
    /// Tablier dessiné du pont de la case de fleuve `hex` (repère 2D du monde).
    pub fn deck(&self, hex: Hex) -> Option<(Vec2, Vec2)> {
        self.decks.get(&hex).copied()
    }
}

/// Dessine les ponts construits (ou posés à la console) et les refait quand le relief alentour
/// change. Un pont n'est montré que si ses deux rives sont explorées.
#[allow(clippy::too_many_arguments)]
fn sync_bridges_3d(
    mut commands: Commands,
    map: Res<GameMap>,
    relief: Option<Res<Relief>>,
    fog: Res<ShownFog>,
    assets: Option<Res<Assets3d>>,
    cities: Query<(&HexPos, &City)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<Bridges3d>,
) {
    let (Some(relief), Some(a)) = (relief, assets) else { return };
    let wrap = map.0.wrap();
    let city_hexes: HashSet<Hex> = cities.iter().map(|(p, _)| map.0.canonical(p.0)).collect();
    // Ville qui a construit chaque pont : sa case, et si elle est fortifiée (tour à l'autre bout)
    let owners: HashMap<crate::map::BridgeAt, (Hex, bool)> =
        cities.iter().flat_map(|(p, c)| c.state.bridges.iter().map(move |&at| (at, (p.0, c.state.fortified)))).collect();
    let wanted: HashSet<crate::map::BridgeAt> = map.0.bridges().collect();
    let gone: Vec<crate::map::BridgeAt> = state.drawn.keys().filter(|k| !wanted.contains(*k)).copied().collect();
    let mut changed = !gone.is_empty();
    for k in gone {
        if let Some((Some(e), _)) = state.drawn.remove(&k) {
            commands.entity(e).despawn();
        }
    }
    for at in wanted {
        let (e0, e1) = at.ends(wrap);
        let mid = match at {
            crate::map::BridgeAt::Edge { hex, .. } | crate::map::BridgeAt::Fleuve { hex, .. } => hex,
        };
        let generation = [e0, e1, mid].iter().map(|h| relief.generation.get(h).copied().unwrap_or(0)).sum::<u32>();
        let seen = fog.explored.contains(&e0) && fog.explored.contains(&e1) && fog.explored.contains(&mid);
        let in_city = matches!(at, crate::map::BridgeAt::Edge { .. }) && (city_hexes.contains(&e0) || city_hexes.contains(&e1));
        let owner = owners.get(&at).copied();
        let key = (generation + u32::from(in_city) * 1_000_000 + u32::from(owner.is_some_and(|o| o.1)) * 2_000_000, seen);
        if state.drawn.get(&at).is_some_and(|d| d.1 == key) {
            continue;
        }
        changed = true;
        if let Some((Some(e), _)) = state.drawn.remove(&at) {
            commands.entity(e).despawn();
        }
        // (le pont d'une ville est dessiné par sa pose)
        let entity = (seen && !in_city).then(|| {
            let (origin, mut bridge) = bridge_pose(&map.0, &relief, at);
            // Tour de tête de pont : à l'autre bout, vu de la ville fortifiée qui l'a construit
            if let Some((city, true)) = owner {
                let (cx, cy) = wrap.nearest(Hex::from_world(origin.x, origin.y), city).to_world();
                let c = Vec2::new(cx, cy) - origin;
                if bridge.far.distance(c) < bridge.near.distance(c) {
                    std::mem::swap(&mut bridge.near, &mut bridge.far);
                }
                bridge.head = true;
            }
            let base = relief.ground(origin.x, origin.y);
            let ground = |p: Vec2| relief.ground(origin.x + p.x, origin.y + p.y) - base;
            let stone = map.0.bridge_stone(at);
            let family = owner.and_then(|(h, _)| map.0.get(h)).map_or(crate::settlements::Family::Prairie, crate::settlements::Family::of);
            let mesh = meshes.add(to_mesh(crate::settlement_mesh::bake_bridge(&bridge, stone, family, &ground)));
            commands.spawn((Bridge3d, Mesh3d(mesh), MeshMaterial3d(a.site_body.clone()), Transform::from_xyz(origin.x, base, -origin.y), layer())).id()
        });
        state.drawn.insert(at, (entity, key));
    }
    if changed {
        let spans = state
            .drawn
            .keys()
            .map(|&at| {
                let (o, b) = bridge_pose(&map.0, &relief, at);
                (o + b.near, o + b.far)
            })
            .collect();
        state.spans = spans;
        let decks = state
            .drawn
            .keys()
            .filter_map(|&at| match at {
                crate::map::BridgeAt::Fleuve { hex, .. } => {
                    let (o, b) = bridge_pose(&map.0, &relief, at);
                    Some((hex, (o + b.near, o + b.far)))
                }
                crate::map::BridgeAt::Edge { .. } => None,
            })
            .collect();
        state.decks = decks;
    }
}

/// Niveau de l'eau près d'un point : celle d'un cours d'eau, sinon d'une mer ou d'un lac voisin.
fn water_near(relief: &Relief, hex: Hex, p: Vec2) -> f32 {
    if let Some(near) = relief.channels.get(&hex)
        && let Some(level) = crate::relief::water_surface(near, p)
    {
        return level;
    }
    Hex::from_world(p.x, p.y)
        .range(2)
        // (l'eau la plus proche : près d'un lac, le niveau de la mer voisine serait sous sa surface)
        .filter_map(|h| {
            let wrapped = relief.wrap.hex(h);
            let (cx, cy) = h.to_world();
            relief.water_hexes.contains(&wrapped).then(|| relief.tops.get(&wrapped).map(|t| (p.distance(Vec2::new(cx, cy)), t.center))).flatten()
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or_else(|| relief.ground(p.x, p.y) - 2.0, |w| w.1)
}

/// Contour irrégulier d'un sol de terre battue.
fn earth_outline(radius: f32) -> Vec<Vec2> {
    (0..48)
        .map(|i| {
            let a = i as f32 / 48.0 * std::f32::consts::TAU;
            Vec2::from_angle(a) * radius * (1.0 + 0.13 * (3.0 * a + 1.0).sin() + 0.07 * (7.0 * a).sin() + 0.05 * (13.0 * a + 2.0).sin())
        })
        .collect()
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

/// Hauteur d'une ville pour la bannière de son nom.
pub const CITY_HEIGHT: f32 = 12.0;

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct Gizmos3d;

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
/// Avancée de l'icône vers la caméra (part de la distance qui les sépare) : bien plus que
/// l'étendue du groupe de figurines agrandies (~35 de haut, anneau de 25 de rayon), et plus que
/// l'avance des lignes 3D (`depth_bias` des gizmos), qui ne la barrent donc plus.
const FAR_ICON_PULL: f32 = 0.25;
/// Taille de l'icône (fois le carré de 26) à la distance d'apparition.
const FAR_ICON_SCALE: f32 = 0.9;

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
    /// Villes, villages et camps : matière blanche (couleur portée par les sommets) et sol de terre
    /// battue et chemins, transparents sur leur bord ; fumée des camps.
    site_body: Handle<StandardMaterial>,
    site_ground: Handle<StandardMaterial>,
    smoke_mesh: Handle<Mesh>,
    smoke: Handle<StandardMaterial>,
    icons: HashMap<&'static str, Handle<StandardMaterial>>,
}

pub struct View3dPlugin;

/// Cases du relief 3D encore à construire ou à recolorer (0 : relief à jour ; les captures de la
/// console l'attendent).
#[derive(Resource, Default)]
pub struct ReliefPending(pub usize);

impl Plugin for View3dPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewMode>()
            .init_resource::<ReliefPending>()
            .init_gizmo_group::<Gizmos3d>()
            .insert_resource(GlobalAmbientLight { brightness: 420.0, ..default() })
            .init_resource::<Bridges3d>()
            .add_systems(Startup, setup_3d)
            .add_systems(
                Update,
                (
                    toggle_view_key,
                    apply_view_mode,
                    camera_3d_controls,
                    update_relief_3d,
                    sync_bridges_3d,
                    draw_bridge_spots_3d,
                    update_decor_3d,
                    spawn_mirrors,
                    sync_mirrors,
                    animate_mirrors,
                    animate_camp_smoke,
                    far_icons,
                    scale_units,
                    draw_overlays_3d,
                    draw_wind_3d,
                    draw_currents_3d,
                    draw_waves_3d,
                    draw_tide_lines_3d,
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
/// Distance sur laquelle l'eau d'une embouchure prend la couleur de la mer ou du lac (rivière, fleuve).
pub const MOUTH_BLEND: f32 = 14.0;
pub const MOUTH_BLEND_FLEUVE: f32 = 30.0;
/// Dénivelé minimal d'une chute d'eau.
const CASCADE_MIN_DROP: f32 = 4.0;
/// Écume d'une chute : pente de la surface (hauteur par unité) où elle commence et où elle est
/// entière, et pas de mesure de cette pente.
const CASCADE_SLOPE: (f32, f32) = (0.7, 1.3);
const CASCADE_SLOPE_STEP: f32 = 1.5;
/// Part de l'écume dans la couleur d'une chute d'eau (le reste : la couleur de l'eau).
const CASCADE_FOAM: f32 = 0.55;
/// Redécoupage des triangles d'eau là où la surface se courbe : écart toléré entre la surface et
/// la ligne droite au milieu d'une arête, profondeur et arête la plus courte.
const WATER_SPLIT_TOLERANCE: f32 = 0.12;
const WATER_SPLIT_DEPTH: usize = 3;
const WATER_SPLIT_MIN: f32 = 0.5;
/// Surélévation de l'eau d'un affluent qui entre dans un fleuve, et des éléments des rapides (eau
/// vive, eau lisse, écume, rochers : dans cet ordre, du dessous au dessus).
const INFLOW_LIFT: f32 = 0.04;
// (au moins la tolérance de la surface de l'eau, `WATER_SPLIT_TOLERANCE` : sinon ces couches passent
// par endroits sous de grands triangles d'eau, et l'eau vive se lit en bandes et en éclats)
const LIGHT_LIFT: f32 = 0.14;
const SHADE_LIFT: f32 = 0.15;
const FOAM_LIFT: f32 = 0.18;
const ROCK_LIFT: f32 = 0.25;

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
        bevy::ui::picking_backend::UiPickingCamera,
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
    let mut mat = |color: Color, roughness: f32| {
        materials.add(StandardMaterial { base_color: color, perceptual_roughness: roughness, ..default() })
    };
    // Sol : blanc (vu) ou assombri (hors de vue), multiplié par la couleur des sommets
    let ground = [mat(Color::WHITE, 0.9), mat(Color::WHITE, 0.25)];
    let dim = Color::srgb(0.45, 0.45, 0.5);
    let ground_dim = [mat(dim, 0.9), mat(dim, 0.25)];
    let player = mat(Color::srgb(0.20, 0.45, 0.95), 0.6);
    let hostile = mat(Color::srgb(0.78, 0.17, 0.15), 0.6);
    let site_body = mat(Color::WHITE, 0.9);
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
    let kit = DecorKit {
        trunk_mesh,
        crown_mesh,
        conifer_mesh,
        canopy_mesh,
        shrub_mesh,
        reed_mesh,
        puddle_mesh,
        rock_mesh,
        trunk,
        broadleaf,
        jungle_leaf,
        conifer,
        acacia,
        shrub,
        reed,
        puddle,
        rock: rock.clone(),
    };

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
    let mut relief = Relief { tops: HashMap::new(), tiles: HashMap::new(), decor: HashMap::new(), water: HashMap::new(), channels: HashMap::new(), axis_cache: HashMap::new(), explored: crate::cases::CaseSet::new(), hatches: HashMap::new(), bumps: BumpMap::new().with_wrap(wrap), generation: HashMap::new(), shape: 0, wrap, water_seen: HashSet::new(), water_colors: map.0.hexes().filter_map(|h| map.0.get(h).filter(|t| t.is_water()).map(|t| (h, terrain_color(t)))).collect(), water_hexes: map.0.hexes().filter(|&h| map.0.get(h).is_some_and(|t| t.is_water())).collect() };
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

        // Décor : arbres en forêt, rochers en relief accidenté
        let ids = spawn_decor(&mut commands, &kit, &map.0, &all_bumps, hex);
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
            // Pas de rocher dans le lit d'un cours d'eau qui descend la falaise
            if crate::waterways::in_water(&map.0, edge.high, edge.dir, p, 3.2 * size) {
                continue;
            }
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
    commands.insert_resource(kit);

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
        site_body,
        site_ground: materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, alpha_mode: AlphaMode::Blend, ..default() }),
        smoke_mesh: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        smoke: materials.add(StandardMaterial { base_color: Color::srgba(0.17, 0.15, 0.14, 0.34), unlit: true, alpha_mode: AlphaMode::Blend, ..default() }),
        icons,
    });
}

/// Maillages et matériaux du décor des cases (végétation, rochers), gardés pour refaire le décor
/// d'une case modifiée (console de debug).
#[derive(Resource)]
struct DecorKit {
    trunk_mesh: Handle<Mesh>,
    crown_mesh: Handle<Mesh>,
    conifer_mesh: Handle<Mesh>,
    canopy_mesh: Handle<Mesh>,
    shrub_mesh: Handle<Mesh>,
    reed_mesh: Handle<Mesh>,
    puddle_mesh: Handle<Mesh>,
    rock_mesh: Handle<Mesh>,
    trunk: Handle<StandardMaterial>,
    broadleaf: Handle<StandardMaterial>,
    jungle_leaf: Handle<StandardMaterial>,
    conifer: Handle<StandardMaterial>,
    acacia: Handle<StandardMaterial>,
    shrub: Handle<StandardMaterial>,
    reed: Handle<StandardMaterial>,
    puddle: Handle<StandardMaterial>,
    rock: Handle<StandardMaterial>,
}

/// Décor d'une case : arbres selon le biome, rochers en relief accidenté (`bumps` : relief interne
/// des cases, pour la limite des arbres). Créé caché, posé sur le sol quand la case est construite.
fn spawn_decor(commands: &mut Commands, kit: &DecorKit, map: &crate::map::WorldMap, bumps: &BumpMap, hex: Hex) -> Vec<Entity> {
    // Décor : arbres en forêt, bosses en colline, pic en montagne (hauteurs fixées avec le relief)
    let t = map.get(hex).unwrap();
    let polar = |deg: f32, r: f32| Vec2::from_angle(deg.to_radians()) * r;
    let mut pieces: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Vec2, f32, Vec3)> = Vec::new();
    // Relief interne (collines, montagnes) : modelé dans le sol de la case (voir `Bumps`) ;
    // quelques rochers sur un relief accidenté
    let relief_kind = map.relief(hex);
    if relief_kind == CellRelief::Rugged {
        for (a, r) in [(30.0_f32, 6.0), (150.0, 20.0), (270.0, 18.0)] {
            pieces.push((kit.rock_mesh.clone(), kit.rock.clone(), polar(a, r), 1.5, Vec3::new(1.4, 1.0, 1.1)));
        }
    }
    // Végétation selon le biome (pas dans le lit d'un fleuve, ni sur un pic). Les plantes
    // restent près du bord : le centre est libre pour le jeton d'une unité. En montagne, les
    // arbres s'arrêtent à la limite des arbres (sous la limite des neiges).
    if !map.is_fleuve(hex) && relief_kind < CellRelief::Peak {
        let ring6 = [30.0_f32, 90.0, 150.0, 210.0, 270.0, 330.0];
        let (cx, cy) = hex.to_world();
        let below_treeline = |at: Vec2| {
            let (up, snow) = lift_at(Vec2::new(cx, cy) + at, bumps);
            up < (snow * 0.6).min(14.0)
        };
        // Emplacements (position, hausse) sous la limite des arbres ; seconde couronne (entre
        // les premiers) pour les forêts denses
        // En montagne : étage montagnard, arbres clairsemés (un emplacement sur deux, pas de
        // seconde couronne), conifères à la place des feuillus
        let montane = relief_kind == CellRelief::Mountain;
        let spots: Vec<(Vec2, f32)> = ring6.iter().step_by(if montane { 2 } else { 1 }).map(|&a| (polar(a, 21.0), 0.0)).filter(|&(at, _)| below_treeline(at)).collect();
        let inner: Vec<Vec2> = if montane { Vec::new() } else { ring6.iter().map(|&a| polar(a + 30.0, 24.5)).filter(|&at| below_treeline(at)).collect() };
        // Feuillu : tronc et houppier rond
        let broad = |at: Vec2, up: f32, leaf: &Handle<StandardMaterial>, size: f32, pieces: &mut Vec<_>| {
            pieces.push((kit.trunk_mesh.clone(), kit.trunk.clone(), at, up + 4.0 * size, Vec3::new(size, size, size)));
            pieces.push((kit.crown_mesh.clone(), leaf.clone(), at, up + 10.5 * size, Vec3::new(size, 0.85 * size, size)));
        };
        match t {
            Terrain::Forest if montane => {
                for &(at, up) in &spots {
                    pieces.push((kit.conifer_mesh.clone(), kit.conifer.clone(), at, up + 7.0, Vec3::splat(0.8)));
                }
            }
            Terrain::Forest => {
                for &(at, up) in &spots {
                    broad(at, up, &kit.broadleaf, 1.0, &mut pieces);
                }
            }
            Terrain::Jungle => {
                // Dense : grands arbres sur le pourtour et plus petits entre eux
                for &(at, up) in &spots {
                    broad(at, up, &kit.jungle_leaf, 1.2, &mut pieces);
                }
                for &at in &inner {
                    broad(at, 0.0, &kit.jungle_leaf, 0.8, &mut pieces);
                }
            }
            Terrain::Taiga => {
                for &(at, up) in &spots {
                    pieces.push((kit.conifer_mesh.clone(), kit.conifer.clone(), at, up + 9.0, Vec3::ONE));
                }
                for &at in &inner {
                    pieces.push((kit.conifer_mesh.clone(), kit.conifer.clone(), at, 7.0, Vec3::splat(0.8)));
                }
            }
            Terrain::WoodedSavanna | Terrain::Savanna => {
                // Acacias : tronc fin et cime plate, épars (un seul en savane herbeuse)
                let picks: &[usize] = if t == Terrain::Savanna { &[1] } else { &[0, 2, 4] };
                for &k in picks.iter().filter(|_| !spots.is_empty()) {
                    let (at, up) = spots[k % spots.len()];
                    pieces.push((kit.trunk_mesh.clone(), kit.trunk.clone(), at, up + 4.0, Vec3::new(0.7, 1.2, 0.7)));
                    pieces.push((kit.canopy_mesh.clone(), kit.acacia.clone(), at, up + 9.5, Vec3::ONE));
                }
            }
            Terrain::Mediterranean => {
                // Broussailles : buissons bas, un arbre (olivier) de temps en temps
                for (k, &(at, up)) in spots.iter().enumerate() {
                    if k % 3 == 0 {
                        broad(at, up, &kit.shrub, 0.75, &mut pieces);
                    } else {
                        pieces.push((kit.shrub_mesh.clone(), kit.shrub.clone(), at, up + 2.0, Vec3::new(1.5, 0.9, 1.5)));
                    }
                }
            }
            Terrain::Marsh => {
                // Flaques et touffes de roseaux
                for a in [60.0_f32, 240.0] {
                    pieces.push((kit.puddle_mesh.clone(), kit.puddle.clone(), polar(a, 18.0), 0.2, Vec3::ONE));
                }
                for a in [0.0_f32, 120.0, 180.0, 300.0] {
                    for (dx, dy) in [(0.0, 0.0), (1.6, 0.8), (-1.2, 1.4)] {
                        pieces.push((kit.reed_mesh.clone(), kit.reed.clone(), polar(a, 20.0) + Vec2::new(dx, dy), 3.0, Vec3::ONE));
                    }
                }
            }
            _ => {}
        }
    }
    // Îles de terre d'un fleuve : des arbres du milieu (les mêmes qu'alentour, plus petits ; des
    // feuillus de bord de l'eau dans les plaines et les steppes, des buissons au désert)
    if map.is_fleuve(hex) {
        let (cx, cy) = hex.to_world();
        for (p, size) in crate::waterways::isle_trees(&hex_water(map, hex), hex) {
            let at = p - Vec2::new(cx, cy);
            let broad = |leaf: &Handle<StandardMaterial>, size: f32, pieces: &mut Vec<_>| {
                pieces.push((kit.trunk_mesh.clone(), kit.trunk.clone(), at, 4.0 * size, Vec3::splat(size)));
                pieces.push((kit.crown_mesh.clone(), leaf.clone(), at, 10.5 * size, Vec3::new(size, 0.85 * size, size)));
            };
            match t {
                Terrain::Jungle => broad(&kit.jungle_leaf, size * 1.1, &mut pieces),
                Terrain::Taiga => pieces.push((kit.conifer_mesh.clone(), kit.conifer.clone(), at, 9.0 * size, Vec3::splat(size))),
                Terrain::Savanna | Terrain::WoodedSavanna => {
                    pieces.push((kit.trunk_mesh.clone(), kit.trunk.clone(), at, 4.0 * size, Vec3::new(0.7 * size, 1.2 * size, 0.7 * size)));
                    pieces.push((kit.canopy_mesh.clone(), kit.acacia.clone(), at, 9.5 * size, Vec3::splat(size)));
                }
                Terrain::Desert | Terrain::Mediterranean => pieces.push((kit.shrub_mesh.clone(), kit.shrub.clone(), at, 2.0 * size, Vec3::new(1.5 * size, 0.9 * size, 1.5 * size))),
                Terrain::Tundra | Terrain::Snow => {}
                _ => broad(&kit.broadleaf, size, &mut pieces),
            }
        }
    }
    pieces
        .into_iter()
        .map(|(mesh, material, offset, lift, scale)| {
            commands
                .spawn((
                    Decor3d { hex, offset, lift, requires: None, vegetation: true },
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_scale(scale),
                    Visibility::Hidden,
                    layer(),
                ))
                .id()
        })
        .collect()
}

fn toggle_view_key(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<ViewMode>, ecran: Res<crate::ecran_ville::EcranVille>) {
    // (pas sous l'écran de la ville, qui couvre la carte)
    if keys.just_pressed(KeyCode::KeyV) && !ecran.ouvert() {
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
                match (!map.0.cliff_with_sea(a, b), wa || wb) {
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
    (mode, time, cam): (Res<ViewMode>, Res<Time<Real>>, Res<Cam3d>),
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
    (mut edited, kit, mut pending, geo): (MessageReader<crate::console::MapEdited>, Option<Res<DecorKit>>, ResMut<ReliefPending>, Res<crate::render::GeoColors>),
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
            .difference_symetrique(&queue.explored)
            .chain(sites.symmetric_difference(&queue.sites).copied())
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
    // Cases modifiées (console de debug) et leurs voisines : eau, décor et relief refaits
    let edited: HashSet<Hex> = edited.read().flat_map(|e| e.0.iter().copied()).collect();
    if !edited.is_empty() {
        let all_bumps: BumpMap = map.0.hexes().filter_map(|h| bumps_for(&map.0, h).map(|b| (h, b))).collect::<BumpMap>().with_wrap(map.0.wrap());
        for &h in &edited {
            match map.0.get(h).filter(|t| t.is_water()) {
                Some(t) => {
                    relief.water_hexes.insert(h);
                    relief.water_colors.insert(h, terrain_color(t));
                }
                None => {
                    relief.water_hexes.remove(&h);
                    relief.water_colors.remove(&h);
                }
            }
            let shape = hex_water(&map.0, h);
            match (shape.is_empty(), relief.water.remove(&h)) {
                (true, Some((_, e))) => commands.entity(e).despawn(),
                (true, None) => {}
                (false, Some((_, e))) => {
                    relief.water.insert(h, (shape, e));
                }
                (false, None) => {
                    let e = commands.spawn((Mesh3d(meshes.add(to_mesh(MeshData::default()))), MeshMaterial3d(assets.fresh_water[0].clone()), self::layer())).id();
                    relief.water.insert(h, (shape, e));
                }
            }
            // Décor de la case refait (les rochers au pied d'une falaise restent)
            if let Some(kit) = &kit {
                let mut kept = Vec::new();
                for e in relief.decor.remove(&h).unwrap_or_default() {
                    match decor.get(e) {
                        Ok((d, _)) if d.requires.is_some() => kept.push(e),
                        _ => commands.entity(e).despawn(),
                    }
                }
                kept.extend(spawn_decor(&mut commands, kit, &map.0, &all_bumps, h));
                relief.decor.insert(h, kept);
            }
            relief.axis_cache.remove(&h);
            queue.geometry.insert(h);
            queue.levels.insert(h);
        }
        relief.water_seen = relief.water_hexes.iter().copied().filter(|h| fog.explored.contains(h)).collect();
        relief.bumps = explored_bumps(&map.0, &fog, &sites);
        relief.shape += 1;
        queue.order_focus = None;
        pending.0 = queue.geometry.len() + queue.color.len();
        // (entités créées ci-dessus : la construction reprend à l'image suivante)
        return;
    }
    // Changement de carte (normale / relief / climat) : seules les couleurs changent, le relief déjà
    // calculé est réutilisé. La vision peut déborder du bord de la carte : on ne garde que la carte.
    // (carte géographique : aussi quand sa famille ou son niveau changent)
    if layer.is_changed() || (geo.is_changed() && *layer == MapLayer::Geography) {
        queue.color.extend(fog.explored.iter().filter(|h| map.0.contains(*h)));
        // (beaucoup de cases d'un coup : l'ordre est refait en une fois)
        queue.order_focus = None;
    }

    // Matériaux (case vue, explorée ou inconnue) : peu coûteux, mis à jour aussitôt
    if fog.is_changed() {
        relief.water_seen = relief.water_hexes.iter().copied().filter(|h| fog.explored.contains(h)).collect();
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
        pending.0 = 0;
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
        (true, shown) if shown.shows_ice() && map.0.is_ice(hex) => {
            let (r, g, b) = crate::render::ICE_COLOR;
            let c = Color::srgb(r, g, b).to_linear();
            Vec3::new(c.red, c.green, c.blue)
        }
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
                MapLayer::Geography => geo.0.shade.get(map.0.cell(map.0.canonical(hex))).map_or(crate::render::GEO_LAND, |&s| crate::render::geo_color(s)),
                MapLayer::Currents => currents_color(&map.0, hex),
                MapLayer::Tides => crate::render::tides_color(&map.0, hex),
                MapLayer::Waves => crate::render::waves_color(&map.0, hex),
                MapLayer::Soils => crate::render::soils_color(&map.0, hex),
                _ => hydro_color(&map.0, hex, thematic),
            };
            let c = Color::srgb(r, g, b).to_linear();
            Vec3::new(c.red, c.green, c.blue)
        }
    };
    // Le fondu d'une embouchure vise la couleur affichée de l'eau (carte, banquise), pas toujours la
    // mer de la carte normale
    if layer.is_changed() || fog.is_changed() {
        let colors: Vec<(Hex, Vec3)> = relief.water_hexes.iter().filter(|h| fog.explored.contains(h)).map(|&h| (h, hex_color(h))).collect();
        relief.water_colors.extend(colors);
    }
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
        // Hauteurs d'eau des lits des cases du paquet et de leurs voisines, calculées en parallèle
        // (beaucoup de lits par case : rivières courbes, affluents, îles ; en série, la
        // préparation d'un paquet dépassait le temps d'une image)
        let missing: HashSet<Hex> = processed[processed.len() - count..]
            .iter()
            .flat_map(|&(h, _)| h.range(1))
            .map(|seen| relief.wrap.hex(seen))
            .filter(|h| relief.water.contains_key(h) && !relief.axis_cache.contains_key(h))
            .collect();
        let computed: Vec<(Hex, Vec<Channel>)> = {
            let r = &*relief;
            missing.into_iter().collect::<Vec<Hex>>().into_par_iter().map(|h| (h, r.water[&h].0.channels.iter().map(|a| r.channel(a)).collect())).collect()
        };
        relief.axis_cache.extend(computed);
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
            let mut top = relief.tops[&hex];
            // Pas de plage de sable dans le grand froid ni contre la banquise
            let cold = matches!(map.0.get(hex), Some(Terrain::Snow | Terrain::Tundra));
            top.beach = !cold && !map.0.neighbors(hex).iter().any(|&n| map.0.is_ice(n));
            (top.warp_corners, top.warp_edges) = crate::waterways::coast_warp(&map.0, hex);
            jobs.push((hex, top, color, sides, near, water(hex)));
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
            let fade = map.0.is_fleuve(hex).then(|| crate::waterways::fleuve_fade(&map.0, hex));
            (*e, water_mesh(fade, relief, &near, shape, shape.visible(hex, explored), explored))
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
    pending.0 = queue.geometry.len() + queue.color.len();
}

/// Moyenne glissante d'une durée mesurée à chaque image.
fn smooth(average: f32, sample: f32) -> f32 {
    if average == 0.0 { sample } else { average + (sample - average) * 0.2 }
}

/// Maillage des cours d'eau d'une case : surface plate en travers du lit, un peu sous le niveau
/// du sol (prise sur l'axe du cours d'eau, lissée le long du cours), couleur selon le type.
fn water_mesh<'a>(
    fade: Option<(Vec<Hex>, Option<f32>)>,
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
    // Mer ou lac le plus proche d'un point parmi ceux d'alentour (`waters_around`, cherchés une
    // fois par triangle) : distance (approchée) et couleur
    let nearest_water = |waters: &[(Vec2, Vec3)], p: Vec2| waters.iter().map(|&(c, color)| (hex_gap(p, c), color)).min_by(|a, b| a.0.total_cmp(&b.0));
    let tint = |waters: &[(Vec2, Vec3)], p: Vec2, tone: crate::waterways::Tone, kind: WaterKind, blend: f32| {
        // Teinte du cours d'eau (débit, limon), qui se fond dans la mer ou le lac où il se jette
        // (sur une plus longue distance pour un fleuve : pas de coupure de couleur à l'embouchure)
        let water = tone.apply_3d(fresh);
        let own = kind.rapids_color(tone, water).unwrap_or(water);
        // (vers la couleur du bord de la case d'eau, assombri : pas de ligne droite à l'arête)
        let c = match nearest_water(waters, p) {
            Some((d, water)) => own.lerp(water * crate::relief::EDGE_SHADE, 1.0 - smoothstep01(d / blend)),
            None => own,
        };
        [c.x, c.y, c.z, 1.0]
    };
    // L'eau s'arrête net au bord de la zone explorée (une rivière d'arête se voit dès qu'une
    // rive est explorée : seule sa moitié côté exploré est dessinée)
    let pieces = tris.flat_map(|t| crate::waterways::clip_to_explored(t.pts, &explored).into_iter().map(move |pts| (t, pts)));
    // (lits rangés par mailles autour de l'eau de la case : voir `ChannelGrid`)
    let (lo, hi) = shape.tris.iter().flat_map(|t| t.pts).fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
    let grid = crate::relief::ChannelGrid::new(near, (lo + hi) / 2.0, ((hi - lo).max_element() / 2.0 + 2.0).max(1.0));
    for (t, [a, b, c]) in pieces {
        // Sens trigonométrique vu du dessus (repère 3D : z = −y)
        let ccw = (b - a).perp_dot(c - a) > 0.0;
        let pts = if ccw { [a, b, c] } else { [a, c, b] };
        // Même surface que celle qui a servi à tailler le lit (lits de la case et des voisines) ;
        // hors de portée des lits, la hauteur du tronçon (calcul coûteux : seulement dans ce cas)
        // (hauteurs déjà calculées dans ce triangle : chaque point en sert plusieurs fois)
        let mut known: Vec<(Vec2, f32)> = Vec::new();
        let mut level = |p: Vec2| {
            if let Some(&(_, y)) = known.iter().find(|(q, _)| *q == p) {
                return y;
            }
            let y = water_surface(grid.near(p), p).unwrap_or_else(|| channels.entry(t.channel).or_insert_with(|| relief.channel(&shape.channels[t.channel])).levels[0]);
            known.push((p, y));
            y
        };
        // Un fleuve ne se fond que dans l'eau où il s'ouvre : il ne bleuit pas en longeant une mer
        let mut waters = relief.waters_around((a + b + c) / 3.0);
        let fleuve_bed = shape.channels[t.channel].kind != WaterKind::River || shape.channels[t.channel].inflow.is_some();
        if let Some((open, _)) = fade.as_ref().filter(|_| fleuve_bed) {
            waters.retain(|&(w, _)| open.iter().any(|h| w.distance(Vec2::from(h.to_world())) < 1.0));
        }
        // Même fondu pour tout ce qui est posé sur un fleuve (rapides, panache d'un affluent) que
        // pour le fleuve : sinon ils gardent leur couleur sur une eau déjà bleue (dalles claires)
        let bed = &shape.channels[t.channel];
        let blend = match fade.as_ref().and_then(|f| f.1) {
            Some(inlet) if fleuve_bed => inlet,
            _ if bed.kind == WaterKind::Fleuve || bed.inflow.is_some() => MOUTH_BLEND_FLEUVE,
            _ => MOUTH_BLEND,
        };
        // Affluent dans un fleuve : un rien au-dessus de l'eau du fleuve (sa couleur s'y fond
        // sans que les deux surfaces se chevauchent par endroits) ; éléments des rapides : posés
        // sur l'eau
        let lift = match t.kind {
            WaterKind::Light => LIGHT_LIFT,
            WaterKind::Shade => SHADE_LIFT,
            WaterKind::Foam => FOAM_LIFT,
            WaterKind::Rock => ROCK_LIFT,
            _ => 0.0,
        } + if shape.channels[t.channel].inflow.is_some() { INFLOW_LIFT } else { 0.0 };
        // Chute d'eau : l'eau descend très fort le long du cours (elle franchit une falaise) →
        // écume (décidée sur le triangle d'origine)
        let ys = pts.map(&mut level);
        let hi = (0..3).max_by(|&i, &j| ys[i].total_cmp(&ys[j])).unwrap();
        let lo = (0..3).min_by(|&i, &j| ys[i].total_cmp(&ys[j])).unwrap();
        let rise = ys[hi] - ys[lo];
        let foam = rise > CASCADE_MIN_DROP * 0.5;
        // Là où la surface se courbe (bord d'une chute : plate puis plongeante), le triangle est
        // redécoupé : tendu en ligne droite, il passait sous la surface, donc sous le sol taillé
        // pour elle (zone sans eau). Décision arête par arête (milieu de l'arête comparé à la
        // surface), la même pour les deux triangles qui la partagent : pas de fente entre eux
        let mut stack = vec![(pts, 0usize)];
        while let Some(([a, b, c], depth)) = stack.pop() {
            let mut split = |p: Vec2, q: Vec2| {
                if depth >= WATER_SPLIT_DEPTH || p.distance(q) < WATER_SPLIT_MIN {
                    return None;
                }
                let m = (p + q) / 2.0;
                ((level(m) - (level(p) + level(q)) / 2.0).abs() > WATER_SPLIT_TOLERANCE).then_some(m)
            };
            let children: Vec<[Vec2; 3]> = match (split(a, b), split(b, c), split(c, a)) {
                (None, None, None) => {
                    for p in [a, b, c] {
                        data.positions.push([p.x, level(p) + lift, -p.y]);
                        data.normals.push([0.0, 1.0, 0.0]);
                        // (écume mêlée à l'eau : un blanc plat se lisait comme une zone sans eau)
                        let water = tint(&waters, p, t.tone_at(p), t.kind, blend);
                        let white = linear(Color::srgb(0.86, 0.93, 0.98));
                        // Écume selon la pente de la surface à ce sommet (pas triangle par triangle :
                        // des éclats), et aucune en arrivant à la mer ou au lac
                        let froth = if foam {
                            let e = CASCADE_SLOPE_STEP;
                            let (dx, dy) = (level(p + Vec2::X * e) - level(p - Vec2::X * e), level(p + Vec2::Y * e) - level(p - Vec2::Y * e));
                            let steep = smoothstep01((dx.hypot(dy) / (2.0 * e) - CASCADE_SLOPE.0) / (CASCADE_SLOPE.1 - CASCADE_SLOPE.0));
                            steep * nearest_water(&waters, p).map_or(1.0, |(d, _)| smoothstep01(d / MOUTH_BLEND))
                        } else {
                            0.0
                        };
                        data.colors.push(if froth > 0.0 { std::array::from_fn(|k| water[k] + (white[k] - water[k]) * CASCADE_FOAM * froth) } else { water });
                        data.indices.push(data.indices.len() as u32);
                    }
                    continue;
                }
                (Some(ab), None, None) => vec![[a, ab, c], [ab, b, c]],
                (None, Some(bc), None) => vec![[a, b, bc], [a, bc, c]],
                (None, None, Some(ca)) => vec![[a, b, ca], [ca, b, c]],
                (Some(ab), Some(bc), None) => vec![[ab, b, bc], [a, ab, bc], [a, bc, c]],
                (None, Some(bc), Some(ca)) => vec![[ca, bc, c], [a, b, bc], [a, bc, ca]],
                (Some(ab), None, Some(ca)) => vec![[a, ab, ca], [ab, b, c], [ab, c, ca]],
                (Some(ab), Some(bc), Some(ca)) => vec![[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]],
            };
            stack.extend(children.into_iter().map(|tri| (tri, depth + 1)));
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
    explored: crate::cases::CaseSet,
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
const RELIEF_BUDGET_MS: f32 = 10.0;
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

/// Intervalle minimal (s) entre deux recalculs des tracés posés sur le sol tant que le relief se
/// construit (la forme du sol change alors à presque chaque image).
const GROUND_LINES_REFRESH_S: f32 = 0.5;

/// Faut-il reposer sur le sol des tracés gardés d'une image à l'autre ? Sans tracés ou dès que le
/// relief est terminé : aussitôt. Pendant sa construction : pas plus d'une fois par intervalle.
/// `last` (instant du dernier recalcul, s) est mis à jour quand la réponse est oui.
pub fn ground_lines_stale(cached: Option<u32>, shape: u32, pending: usize, now: f32, last: &mut f32) -> bool {
    let stale = match cached {
        None => true,
        Some(s) if s == shape => false,
        Some(_) => pending == 0 || now - *last >= GROUND_LINES_REFRESH_S,
    };
    if stale {
        *last = now;
    }
    stale
}

/// Décor visible sur les cases explorées, sauf dans la clairière d'une ville, d'un village ou d'un
/// camp (il réapparaît quand le village ou le camp disparaît).
#[allow(clippy::too_many_arguments)]
fn update_decor_3d(
    fog: Res<ShownFog>,
    cities: Query<(&HexPos, &City)>,
    villages: Query<&HexPos, With<Village>>,
    camps: Query<(&HexPos, &Camp)>,
    added: Query<(), Or<(Added<City>, Added<Village>, Added<Camp>)>>,
    mut removed_villages: RemovedComponents<Village>,
    mut removed_camps: RemovedComponents<Camp>,
    mut decor: Query<(&Decor3d, &mut Visibility)>,
    layer: Res<MapLayer>,
    mut shown_codes: Local<HashMap<Hex, u8>>,
    map: Res<GameMap>,
    bridges: Res<Bridges3d>,
) {
    let removed = removed_villages.read().count() + removed_camps.read().count() > 0;
    // Clairière de chaque ville : elle grandit avec le stade et va jusqu'à la muraille
    let mut clearing: HashMap<Hex, f32> = HashMap::new();
    let mut city_changed = false;
    for (pos, city) in &cities {
        let stage = crate::settlements::stage_of_population(city.state.palier());
        let code = stage as u8 * 2 + u8::from(city.state.fortified);
        if shown_codes.insert(pos.0, code) != Some(code) {
            city_changed = true;
        }
        clearing.insert(pos.0, if city.state.fortified { [26.0, 28.0, 99.0][stage] } else { [20.5, 24.0, 99.0][stage] });
    }
    if !fog.is_changed() && added.is_empty() && !removed && !layer.is_changed() && !city_changed && !bridges.is_changed() {
        return;
    }
    for pos in &villages {
        clearing.insert(pos.0, 23.0);
    }
    // (en marais, le site a ses propres mares et roseaux : ils remplacent les flaques du décor)
    for pos in cities.iter().map(|(p, _)| p).chain(&villages) {
        if map.0.get(map.0.canonical(pos.0)) == Some(Terrain::Marsh) {
            clearing.insert(pos.0, 99.0);
        }
    }
    for (pos, camp) in &camps {
        // (un fort dégage tout le terrain ; un camp reste dans les arbres)
        clearing.insert(pos.0, if camp.kind == crate::settlements::CampKind::Fort { 99.0 } else { 22.5 });
    }
    let topo = layer.thematic();
    // Abords des ponts dégagés : ni arbre ni rocher sur le tablier ou à ses bouts (distance au plus
    // court autour de la planète)
    let world = map.0.wrap().world_width();
    let near_bridge = |p: Vec2| {
        bridges.spans.iter().any(|&(a, b)| {
            let mut q = p;
            if world > 0.0 {
                q.x = a.x + (q.x - a.x + world / 2.0).rem_euclid(world) - world / 2.0;
            }
            let ab = b - a;
            let t = ((q - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            q.distance(a + ab * t) < BRIDGE_CLEARING
        })
    };
    for (d, mut vis) in &mut decor {
        let (cx, cy) = d.hex.to_world();
        let shown = fog.explored.contains(&d.hex)
            && d.requires.is_none_or(|r| fog.explored.contains(&r))
            && !(topo && d.vegetation)
            && clearing.get(&d.hex).is_none_or(|&r| d.offset.length() > r)
            && (bridges.spans.is_empty() || !near_bridge(Vec2::new(cx, cy) + d.offset));
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
            .spawn((Mirror3d { source, is_unit: true, placed: None, animated: false }, Transform::default(), Visibility::Hidden, layer()))
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
    for source in cities.iter().chain(&villages) {
        spawn_settlement_model(&mut commands, &a, &mut meshes, source, false);
    }
    for source in &camps {
        spawn_settlement_model(&mut commands, &a, &mut meshes, source, true);
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

/// Sol drapé sur le relief (maillage propre au site, refait quand il est replacé).
fn patch(meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, outline: Vec<Vec2>) -> impl Bundle {
    (Patch3d { outline }, Mesh3d(meshes.add(to_mesh(MeshData::default()))), MeshMaterial3d(material.clone()), Transform::default(), layer())
}

/// Ville, village ou camp 3D : un maillage pour le bâti, un pour le sol et les chemins. Leur contenu
/// est calculé dans `sync_mirrors`, d'après la case, le stade et la fortification. Un camp a sa fumée.
fn spawn_settlement_model(commands: &mut Commands, a: &Assets3d, meshes: &mut Assets<Mesh>, source: Entity, camp: bool) {
    commands
        .spawn((Mirror3d { source, is_unit: false, placed: None, animated: false }, SiteModel::default(), Transform::default(), Visibility::Hidden, layer()))
        .with_children(|p| {
            p.spawn(patch(meshes, &a.site_ground, earth_outline(12.0)));
            p.spawn((SiteBody, Mesh3d(meshes.add(to_mesh(MeshData::default()))), MeshMaterial3d(a.site_body.clone()), Transform::default(), layer()));
            if camp {
                for index in 0..CAMP_SMOKE_PUFFS {
                    p.spawn((CampSmoke { source, index }, Mesh3d(a.smoke_mesh.clone()), MeshMaterial3d(a.smoke.clone()), Transform::default(), Visibility::Hidden, layer()));
                }
            }
        });
}

/// Bouffées de fumée d'un camp, dont une partie seulement hors de la période d'alerte.
const CAMP_SMOKE_PUFFS: usize = 6;

/// Fumée des camps : elle monte et s'élargit en boucle ; plus dense et plus large dans les derniers
/// tours avant la sortie d'un guerrier.
fn animate_camp_smoke(time: Res<Time>, camps: Query<&Camp>, mut puffs: Query<(&CampSmoke, &mut Transform, &mut Visibility)>) {
    let now = time.elapsed_secs();
    for (puff, mut t, mut vis) in &mut puffs {
        let Ok(camp) = camps.get(puff.source) else { continue };
        let alert = camp.cooldown <= crate::hostile::CAMP_WARNING_TURNS;
        let count = if alert { CAMP_SMOKE_PUFFS } else { 3 };
        let wanted = if puff.index < count { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
        let phase = (now * 0.11 + puff.index as f32 / count as f32).fract();
        let size = (0.9 + 2.2 * phase) * if alert { 1.55 } else { 1.0 };
        t.translation = Vec3::new(0.7 * (phase * 7.0 + puff.index as f32).sin(), 3.0 + phase * if alert { 19.0 } else { 13.0 }, 0.0);
        // (elle se dissipe en haut)
        t.scale = Vec3::splat(size * (1.0 - phase * phase * phase));
    }
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
fn patch_mesh(relief: &Relief, center: Vec2, origin: Vec3, outline: &[Vec2], tint: Vec3) -> MeshData {
    const RINGS: usize = 5;
    let mut data = MeshData::default();
    // (le bord se fond dans le terrain)
    let color = move |ring: usize| [tint.x, tint.y, tint.z, [0.55, 0.55, 0.55, 0.55, 0.38, 0.0][ring]];
    let push = |p: Vec2, ring: usize, data: &mut MeshData| {
        let q = relief.on_ground(center.x + p.x, center.y + p.y, 0.35) - origin;
        data.positions.push(q.to_array());
        data.normals.push([0.0, 1.0, 0.0]);
        data.colors.push(color(ring));
    };
    push(Vec2::ZERO, 0, &mut data);
    let n = outline.len() as u32;
    for r in 1..=RINGS {
        let t = r as f32 / RINGS as f32;
        for &o in outline {
            push(o * t, r, &mut data);
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
    sources: Query<(&HexPos, Has<Hostile>, Has<Village>, Has<Camp>, Has<crate::feedback::Motion>, Option<&Unit>)>,
    retired: Query<(), With<crate::feedback::Retired>>,
    bridges: Res<Bridges3d>,
    cities: Query<&City>,
    camp_states: Query<&Camp>,
    mut mirrors: Query<(Entity, &mut Mirror3d, &mut Transform, &mut Visibility, Option<&Children>, Option<&mut SiteModel>)>,
    mut socles: Query<(&mut Transform, &mut Visibility), (With<Socle3d>, Without<Mirror3d>)>,
    mut boats: Query<(&mut Transform, &mut Visibility), (With<Boat3d>, Without<Socle3d>, Without<Mirror3d>)>,
    mut members: Query<(&mut Member, &mut Transform), (Without<Boat3d>, Without<Socle3d>, Without<Mirror3d>)>,
    mut patches: Query<(&mut Patch3d, &Mesh3d)>,
    bodies: Query<&Mesh3d, With<SiteBody>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(relief) = relief else { return };
    for (entity, mut mirror, mut transform, mut vis, children, mut model) in &mut mirrors {
        let Ok((pos, hostile, village, camp, moving, unit)) = sources.get(mirror.source) else {
            // (une unité dispersée reste montrée le temps de son effacement)
            if !retired.contains(mirror.source) {
                commands.entity(entity).despawn();
            }
            continue;
        };
        let hex = map.0.canonical(pos.0);
        // Une ville ou un village se pose quand sa case est explorée : avant, son terrain n'est pas connu
        if model.is_some() && !fog.explored.contains(&pos.0) {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        // Stade et fortification d'une ville : ils changent sa pose
        let city = cities.get(mirror.source).ok();
        // (et le pont construit sur une rivière qui la borde : porte et tête de pont)
        let bridge_dir = city.and_then(|_| (0..6).find(|&d| map.0.bridge_between(hex, map.0.wrap().neighbor(hex, d))));
        let code = city.map_or(0, |c| crate::settlements::stage_of_population(c.state.palier()) as u8 * 2 + u8::from(c.state.fortified)) + bridge_dir.map_or(0, |d| 8 * (d as u8 + 1));
        // Unité sur une case de fleuve avec pont : sur le tablier (1) ou dans l'eau, à côté (2)
        let deck = bridges.deck(hex).filter(|_| mirror.is_unit);
        let code = match (deck, unit) {
            (Some(_), Some(u)) => code + if u.0.on_bridge { 1 } else { 2 },
            _ => code,
        };
        let key = (hex, relief.generation.get(&hex).copied().unwrap_or(0), code);
        if mirror.placed != Some(key) {
            mirror.placed = Some(key);
            let (cx, cy) = pos.0.to_world();
            let mut c = Vec2::new(cx, cy);
            // (tablier et place dans l'eau, repère de la case, la case pouvant être une copie au-delà du raccord)
            let shift = c - Vec2::from(hex.to_world());
            let on_deck = deck.map(|(a, b)| (a + shift, b + shift)).filter(|_| code % 8 == 1);
            if let Some((a, b)) = deck.map(|(a, b)| (a + shift, b + shift)) {
                let mid = (a + b) / 2.0;
                c = if on_deck.is_some() { mid } else { mid + (b - a).normalize_or_zero().perp() * BRIDGE_WATER_GAP };
            }
            let (cx, cy) = (c.x, c.y);
            let center = relief.ground(cx, cy);
            let children: Vec<Entity> = children.map(|c| c.iter().collect()).unwrap_or_default();
            if mirror.is_unit {
                // Sur un fleuve : dans une barque, à la surface de l'eau, dans le sens du courant
                let river = relief.channels.get(&hex).filter(|_| map.0.is_fleuve(hex));
                let afloat = river.and_then(|near| {
                    let water = crate::relief::water_surface(near, c)?;
                    let axis = near.iter().filter(|ch| !ch.is_disc() && ch.isle.is_none()).min_by(|a, b| c.distance(a.nearest(c)).total_cmp(&c.distance(b.nearest(c))))?;
                    let d = axis.b - axis.a;
                    Some((water, Quat::from_rotation_y(d.x.atan2(-d.y))))
                });
                // Groupe : chaque figurine a ses pieds au plus haut du sol sous elle (jamais
                // enfoncée), ou sur le pont de sa barque (le groupe resserré) ; anneau au sol
                // incliné comme le sol
                // Sur le tablier d'un pont : au milieu, à sa hauteur (bouts sur les rives, bombé de 1,3)
                let deck_top = on_deck.map(|(a, b)| (relief.ground(a.x, a.y) + relief.ground(b.x, b.y)) / 2.0 + 0.15 + 1.3);
                let afloat = afloat.filter(|_| deck_top.is_none());
                let root = match (afloat, deck_top) {
                    (_, Some(top)) => top,
                    (Some((water, _)), None) => water + BOAT_DECK,
                    (None, None) => footprint(&relief, c, FIGURE_REACH).1,
                };
                let deck_turn = on_deck.map(|(a, b)| {
                    let d = b - a;
                    Quat::from_rotation_y(d.x.atan2(-d.y))
                });
                transform.translation = Vec3::new(cx, root, -cy);
                let ring = (center - root + 0.3, Quat::from_rotation_arc(Vec3::Y, ground_normal(&relief, c, SOCLE_RADIUS)));
                for &child in &children {
                    if let Ok((mut t, mut v)) = socles.get_mut(child) {
                        // (pas d'anneau sur l'eau ni sur un pont)
                        *v = if afloat.is_some() || deck_top.is_some() { Visibility::Hidden } else { Visibility::Inherited };
                        (t.translation.y, t.rotation) = ring;
                    } else if let Ok((mut t, mut v)) = boats.get_mut(child) {
                        *v = if afloat.is_some() { Visibility::Inherited } else { Visibility::Hidden };
                        if let Some((water, turn)) = afloat {
                            t.translation.y = water + BOAT_HULL_LIFT - root;
                            t.rotation = turn;
                        }
                    } else if let Ok((mut m, mut t)) = members.get_mut(child) {
                        m.base = match (afloat, deck_turn) {
                            // (en file sur le tablier)
                            (_, Some(turn)) => turn * local3(Vec2::new(m.at.x * 0.3, m.at.y * 1.2)),
                            (Some((_, turn)), None) => turn * local3(m.at * BOAT_SQUEEZE),
                            (None, None) => local3(m.at) + Vec3::Y * (footprint(&relief, c + m.at, FIGURE_REACH).1 - root),
                        };
                        let k = t.scale.x;
                        t.translation = Vec3::new(m.base.x * k, m.base.y, m.base.z * k);
                    }
                }
            } else {
                transform.translation = Vec3::new(cx, center, -cy);
                // Pose gardée tant que le stade et la fortification ne changent pas
                if let Some(model) = model.as_mut() {
                    if model.pose.is_none() || model.code != code {
                        let built = city.and_then(|_| city_bridge(&map.0, &relief, hex)).map(|b| b.1);
                        let site = ReliefSite { relief: &relief, center: c, hex, shoreline: shoreline_near(&map.0, &relief, c), built };
                        let family = map.0.get(hex).map_or(crate::settlements::Family::Prairie, crate::settlements::Family::of);
                        let seed = (hex.q as i64 as u64).wrapping_mul(73_856_093) ^ (hex.r as i64 as u64).wrapping_mul(19_349_663);
                        model.pose = Some(if let Ok(state) = camp_states.get(mirror.source) {
                            SitePose::Camp(crate::settlements::camp_layout(&site, family, state.kind, seed))
                        } else {
                            SitePose::Town(match city {
                                Some(city) => crate::settlements::city_layout(&site, family, crate::settlements::stage_of_population(city.state.palier()), city.state.fortified, seed),
                                // (été frais : un village d'éleveurs plutôt qu'un village de culture)
                                None => crate::settlements::village_layout(&site, map.0.warmest_season(hex).map_or(family, |t| crate::settlements::village_family(family, t)), seed),
                            })
                        });
                        model.code = code;
                    }
                    if let Some(pose) = &model.pose {
                        let ground = |p: Vec2| relief.ground(cx + p.x, cy + p.y) - center;
                        let tint = |color: Color| Vec3::from_array(color.to_linear().to_vec3().to_array());
                        // (bâti, puis ce qui se pose sur le sol : terre battue et chemins)
                        let ponds = if let SitePose::Town(layout) = pose { layout.ponds.as_slice() } else { &[][..] };
                        let (body, paths, earth_radius, earth, trail) = match pose {
                            SitePose::Town(layout) => {
                                let water = layout.pier.map_or(0.0, |p| water_near(&relief, hex, c + p.to) - center);
                                let (earth, trail) = crate::settlement_mesh::earth_color(layout.family);
                                (crate::settlement_mesh::bake(layout, &ground, water, tint(Color::srgb(0.20, 0.45, 0.95))), layout.paths.as_slice(), layout.ground_radius, earth, trail)
                            }
                            SitePose::Camp(camp) => {
                                let earth = crate::settlement_mesh::camp_earth();
                                (crate::settlement_mesh::bake_camp(camp, &ground), &[][..], camp.ground_radius, earth, earth)
                            }
                        };
                        let mut body = Some(body);
                        for &child in &children {
                            if let Ok(mesh) = bodies.get(child)
                                && let Some(mut m) = meshes.get_mut(&mesh.0)
                                && let Some(body) = body.take()
                            {
                                *m = to_mesh(body);
                            } else if let Ok((mut p, mesh)) = patches.get_mut(child)
                                && let Some(mut m) = meshes.get_mut(&mesh.0)
                            {
                                p.outline = earth_outline(earth_radius);
                                let mut data = patch_mesh(&relief, c, Vec3::new(cx, center, -cy), &p.outline, earth);
                                // (dans l'ordre : la terre, puis les mares, puis les chemins)
                                for extra in [crate::settlement_mesh::bake_ponds(ponds, &ground, earth), crate::settlement_mesh::bake_paths(paths, &ground, trail)] {
                                    let base = data.positions.len() as u32;
                                    data.positions.extend(extra.positions);
                                    data.normals.extend(extra.normals);
                                    data.colors.extend(extra.colors);
                                    data.indices.extend(extra.indices.iter().map(|i| i + base));
                                }
                                *m = to_mesh(data);
                            }
                        }
                    }
                }
            }
        }
        let visible = if hostile {
            moving || fog.visible.contains(&pos.0)
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

/// Figurines en déplacement mis en scène (`feedback`) : posées sur le sol au point de leur
/// animation ; replacées exactement sur leur case quand elle se termine.
fn animate_mirrors(
    time: Res<Time>,
    relief: Option<Res<Relief>>,
    motions: Query<&crate::feedback::Motion>,
    mut mirrors: Query<(&mut Mirror3d, &mut Transform)>,
) {
    let Some(relief) = relief else { return };
    let now = time.elapsed_secs_f64();
    for (mut mirror, mut transform) in &mut mirrors {
        if !mirror.is_unit {
            continue;
        }
        match motions.get(mirror.source).ok().and_then(|m| m.sample(now)) {
            Some(pose) => {
                mirror.animated = true;
                transform.translation = relief.on_ground(pose.at.x, pose.at.y, 0.0);
                transform.scale = Vec3::splat(pose.scale);
            }
            None if mirror.animated => {
                mirror.animated = false;
                mirror.placed = None;
                transform.scale = Vec3::ONE;
            }
            None => {}
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
/// suffit). Chacune est avancée vers l'œil de la caméra le long du rayon qui la vise : elle reste
/// à la même place à l'écran (taille corrigée d'autant) mais passe devant ses figurines, qui ne
/// la traversent plus.
fn far_icons(
    mode: Res<ViewMode>,
    cam: Res<Cam3d>,
    map: Res<GameMap>,
    mirrors: Query<&Transform, (With<Mirror3d>, Without<FarIcon>)>,
    mut icons: Query<(&ChildOf, &mut Transform, &mut Visibility), With<FarIcon>>,
) {
    if !mode.is_3d {
        return;
    }
    let far = cam.distance > FAR_ICON_DISTANCE;
    let eye = camera_transform(&cam);
    let world = map.0.wrap().world_width();
    // (taille à l'écran à peu près constante : agrandie avec la distance)
    let size = cam.distance / FAR_ICON_DISTANCE * FAR_ICON_SCALE;
    for (parent, mut t, mut vis) in &mut icons {
        let wanted = if far { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
        if !far {
            continue;
        }
        let Ok(mirror) = mirrors.get(parent.parent()) else { continue };
        // Rayon de l'œil vers la place de l'icône, pris par le plus court autour de la planète
        // (une unité montrée par une caméra fantôme est vue à un tour près)
        let lift = Vec3::Y * FAR_ICON_LIFT * size;
        let mut ray = mirror.translation + lift - eye.translation;
        if world > 0.0 {
            ray.x -= (ray.x / world).round() * world;
        }
        let translation = lift - ray * FAR_ICON_PULL;
        let scale = Vec3::splat(size * (1.0 - FAR_ICON_PULL));
        if t.rotation != eye.rotation || t.scale != scale || t.translation != translation {
            t.rotation = eye.rotation;
            t.scale = scale;
            t.translation = translation;
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
    groupe: Res<crate::groupes::Groupe>,
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
    // Autres unités du groupe : anneau plus pâle
    for &e in groupe.0.iter().filter(|&&e| e != selected) {
        if let Ok(p) = positions.get(e) {
            ground_circle(&mut gizmos, p.0, 2.0, HEX_SIZE * 0.78, Color::srgba(1.0, 0.9, 0.2, 0.55));
        }
    }
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

/// Flèches posées sur le sol, gardées d'une image à l'autre (forme du sol `shape`, voir
/// `ground_lines_stale`) : départ, arrivée, couleur.
#[derive(Default)]
struct ArrowsCache {
    shape: Option<u32>,
    refreshed: f32,
    arrows: Vec<(Vec3, Vec3, Color)>,
}

/// Carte du climat en 3D : flèches des vents dominants, posées au-dessus du relief.
fn draw_wind_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    (pending, time): (Res<ReliefPending>, Res<Time>),
    mut cache: Local<ArrowsCache>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    // (exploration ou carte changées pendant qu'une autre carte est affichée ; `regenerer` refait
    // les vents sans toucher forcément au sol)
    if fog.is_changed() || map.is_changed() {
        cache.shape = None;
    }
    let Some(relief) = relief else { return };
    if !mode.is_3d || *layer != MapLayer::Climate {
        return;
    }
    let cache = &mut *cache;
    if ground_lines_stale(cache.shape, relief.shape, pending.0, time.elapsed_secs(), &mut cache.refreshed) {
        let lift = |p: Vec2| relief.on_ground(p.x, p.y, 6.0);
        let arrows = fog.explored.iter().filter(|&h| wind_arrow_site(h)).filter_map(|hex| wind_arrow(&map.0, hex));
        cache.arrows = arrows.map(|(from, to)| (lift(from), lift(to), Color::srgba(1.0, 1.0, 1.0, 0.85))).collect();
        cache.shape = Some(relief.shape);
    }
    for &(from, to, color) in &cache.arrows {
        gizmos.arrow(from, to, color).with_tip_length(8.0);
    }
}

/// Carte des courants marins en 3D : flèches des courants, posées sur l'eau.
fn draw_currents_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    (pending, time): (Res<ReliefPending>, Res<Time>),
    mut cache: Local<ArrowsCache>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    // (comme pour les vents : `regenerer` refait les courants sans toucher forcément au sol)
    if fog.is_changed() || map.is_changed() {
        cache.shape = None;
    }
    let Some(relief) = relief else { return };
    if !mode.is_3d || *layer != MapLayer::Currents {
        return;
    }
    let cache = &mut *cache;
    if ground_lines_stale(cache.shape, relief.shape, pending.0, time.elapsed_secs(), &mut cache.refreshed) {
        let lift = |p: Vec2| relief.on_ground(p.x, p.y, 2.0);
        let arrows = fog.explored.iter().filter(|&h| wind_arrow_site(h)).filter_map(|hex| current_arrow(&map.0, hex));
        cache.arrows = arrows.map(|(from, to, color)| (lift(from), lift(to), color)).collect();
        cache.shape = Some(relief.shape);
    }
    for &(from, to, color) in &cache.arrows {
        gizmos.arrow(from, to, color).with_tip_length(7.0);
    }
}

/// Carte de la houle en 3D : flèches du sens de la houle, posées sur l'eau.
fn draw_waves_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    (pending, time): (Res<ReliefPending>, Res<Time>),
    mut cache: Local<ArrowsCache>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    // (comme pour les courants : `regenerer` refait la houle sans toucher forcément au sol)
    if fog.is_changed() || map.is_changed() {
        cache.shape = None;
    }
    let Some(relief) = relief else { return };
    if !mode.is_3d || *layer != MapLayer::Waves {
        return;
    }
    let cache = &mut *cache;
    if ground_lines_stale(cache.shape, relief.shape, pending.0, time.elapsed_secs(), &mut cache.refreshed) {
        let lift = |p: Vec2| relief.on_ground(p.x, p.y, 2.0);
        let arrows = fog.explored.iter().filter(|&h| crate::render::wind_arrow_site(h)).filter_map(|hex| crate::render::wave_arrow(&map.0, hex));
        cache.arrows = arrows.map(|(from, to, color)| (lift(from), lift(to), color)).collect();
        cache.shape = Some(relief.shape);
    }
    for &(from, to, color) in &cache.arrows {
        gizmos.arrow(from, to, color).with_tip_length(7.0);
    }
}

/// Lignes d'heure de pleine mer posées sur la mer, gardées d'une image à l'autre (refaites quand
/// l'exploration ou la forme du sol changent, voir `ground_lines_stale`).
#[derive(Default)]
struct TideLines3d {
    shape: Option<u32>,
    lines: Vec<(Color, Vec3, Vec3)>,
    amphidromes: Vec<Vec3>,
    arrows: Vec<(Vec3, Vec3)>,
}

/// Carte des marées : lignes d'égale heure de pleine mer et points amphidromiques (en 3D).
fn draw_tide_lines_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    shown: Res<crate::render::TideLines>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    (pending, time): (Res<ReliefPending>, Res<Time>),
    mut cache: Local<TideLines3d>,
    mut refreshed: Local<f32>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    if fog.is_changed() {
        cache.shape = None;
    }
    let Some(relief) = relief else { return };
    if !mode.is_3d || *layer != MapLayer::Tides || !(shown.lines || shown.arrows) {
        return;
    }
    if ground_lines_stale(cache.shape, relief.shape, pending.0, time.elapsed_secs(), &mut refreshed) {
        let (lines, amphidromes) = crate::render::tide_lines(&map.0, &fog);
        let lift = |p: Vec2| relief.on_ground(p.x, p.y, 1.5);
        *cache = TideLines3d {
            shape: Some(relief.shape),
            lines: lines.iter().map(|&(hour, a, b)| (crate::render::tide_hour_color(hour), lift(a), lift(b))).collect(),
            amphidromes: amphidromes.iter().map(|&p| lift(p)).collect(),
            arrows: crate::render::tide_arrows(&map.0, &fog).iter().map(|&(a, b)| (lift(a) + Vec3::Y, lift(b) + Vec3::Y)).collect(),
        };
    }
    if shown.lines {
        for &(color, a, b) in &cache.lines {
            gizmos.line(a, b, color);
        }
        for &p in &cache.amphidromes {
            gizmos.circle(Isometry3d::new(p, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 8.0, Color::WHITE);
            gizmos.circle(Isometry3d::new(p, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 3.5, Color::BLACK);
        }
    }
    if shown.arrows {
        for &(a, b) in &cache.arrows {
            gizmos.arrow(a, b, crate::render::TIDE_ARROW).with_tip_length(7.0);
        }
    }
}

/// Case sous le curseur en vue 3D : on avance le long du rayon de la caméra, du haut du
/// relief vers le sol, jusqu'à passer sous le dessus de la case survolée (falaises comprises).
///
/// Rend aussi le point du sol touché (repère 2D du monde).
pub fn hovered_ground_3d(ray: Ray3d, relief: &Relief, map: &GameMap) -> Option<(Hex, Vec2)> {
    let top = meters_to_height(4_800.0);
    let ground = Vec3::Y;
    let start = ray.intersect_plane(Vec3::new(0.0, top, 0.0), InfinitePlane3d::new(ground))?;
    let end = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(ground))?;
    let steps = ((end - start) / 2.0).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let p = ray.get_point(start + (end - start) * i as f32 / steps as f32);
        let h = map.0.canonical(Hex::from_world(p.x, -p.z));
        if map.0.contains(h) && p.y <= relief.ground(p.x, -p.z) {
            return Some((h, Vec2::new(p.x, -p.z)));
        }
    }
    let p = ray.get_point(end);
    Some((map.0.canonical(Hex::from_world(p.x, -p.z)), Vec2::new(p.x, -p.z))).filter(|(h, _)| map.0.contains(*h))
}

/// Emplacements de pont surlignés en 3D, en mode choix (voir `bridge_build`) : un trait posé au
/// ras du sol d'une rive à l'autre, celui du curseur plus épais et blanc.
fn draw_bridge_spots_3d(
    placement: Res<crate::bridge_build::BridgePlacement>,
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    mut gizmos: Gizmos<Gizmos3d>,
) {
    let Some(relief) = relief else { return };
    if !placement.active() || !mode.is_3d {
        return;
    }
    for &at in &placement.spots {
        let (e0, e1) = at.ends(map.0.wrap());
        if !(fog.explored.contains(&e0) || fog.explored.contains(&e1)) {
            continue;
        }
        let (a, b) = crate::render::bridge_ends_2d(&map.0, at);
        let hovered = placement.hovered == Some(at);
        let color = if hovered { Color::srgb(1.0, 1.0, 1.0) } else { Color::srgba(1.0, 0.9, 0.45, 0.9) };
        let side = (b - a).normalize_or_zero().perp() * if hovered { 1.6 } else { 0.8 };
        for k in [-1.0f32, 0.0, 1.0] {
            let n = 8;
            let pts: Vec<Vec3> = (0..=n)
                .map(|i| {
                    let p = a.lerp(b, i as f32 / n as f32) + side * k;
                    Vec3::new(p.x, relief.ground(p.x, p.y).max(water_near(&relief, map.0.canonical(Hex::from_world(p.x, p.y)), p)) + 1.2, -p.y)
                })
                .collect();
            gizmos.linestrip(pts, color);
        }
    }
}

/// Copie, de l'autre côté du raccord d'une planète, du sol ou de l'eau d'une case du bord
/// (même maillage, même matériau, décalée d'un tour). Sans elle, la vue « fantôme » du bord
/// opposé (voir `seam.rs`) et la vue principale se touchent pile sur le raccord, et le lissage
/// des bords (MSAA) y laisse un fin liseré sombre en pointillés ; avec elle, la vue fantôme
/// déborde d'une colonne sous la vue principale. Seules les caméras fantômes la voient : dessinée
/// par la caméra principale, elle recouvrait ce que le fantôme avait peint sur cette colonne
/// (unités, bâtiments, arbres, eau).
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
        let ground = relief.tiles.iter().map(|(h, &e)| (h, e, RenderLayers::layer(LAYER_SEAM_COPY)));
        // Une rivière d'arête n'est dessinée que par une de ses deux cases, et déborde sur l'autre :
        // son eau reste vue des deux côtés. Pas celle d'un fleuve ou d'un lac, qui resterait
        // peinte sur les barques du fantôme.
        let water = relief.water.iter().map(|(h, (_, e))| (h, *e, if map.0.is_fleuve(*h) || map.0.get(*h).is_some_and(|t| t.is_water()) { RenderLayers::layer(LAYER_SEAM_COPY) } else { layer() }));
        for (hex, source, layers) in ground.chain(water) {
            let Some(shift) = edge(hex) else { continue };
            let Ok((mesh, material, _)) = sources.get(source) else { continue };
            commands.spawn((
                SeamCopy { source },
                mesh.clone(),
                material.clone(),
                Transform::from_xyz(shift, 0.0, 0.0),
                layers,
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
/// l'autre et recalculés seulement quand l'exploration ou la forme du sol changent (voir
/// `ground_lines_stale`).
fn draw_plates_3d(
    mode: Res<ViewMode>,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    relief: Option<Res<Relief>>,
    (pending, time): (Res<ReliefPending>, Res<Time>),
    mut cache: Local<PlatesCache>,
    mut refreshed: Local<f32>,
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
    if ground_lines_stale(cache.shape, relief.shape, pending.0, time.elapsed_secs(), &mut refreshed) {
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
    let arrows = fog.explored.iter().filter(|&h| wind_arrow_site(h)).filter_map(|hex| plate_arrow(m, hex)).map(|(from, to)| (lift(from, 6.0), lift(to, 6.0))).collect();
    for &h in m.plates().hotspots.iter().filter(|h| fog.explored.contains(h)) {
        let (x, y) = h.to_world();
        for radius in [0.45, 0.2] {
            let points = (0..=24).map(|k| lift(Vec2::new(x, y) + Vec2::from_angle(k as f32 / 24.0 * std::f32::consts::TAU) * HEX_SIZE * radius, 3.0)).collect();
            lines.push((points, crate::render::HOTSPOT));
        }
    }
    PlatesCache { shape: Some(relief.shape), lines, arrows }
}

#[cfg(test)]
mod ground_lines_tests {
    use super::*;

    #[test]
    fn traces_recalcules_au_plus_une_fois_par_intervalle_pendant_la_construction() {
        let mut last = 0.0;
        // Pas encore de tracés : aussitôt, même en pleine construction
        assert!(ground_lines_stale(None, 5, 100, 1.0, &mut last));
        assert_eq!(last, 1.0);
        // Même forme du sol : rien à faire
        assert!(!ground_lines_stale(Some(5), 5, 0, 9.0, &mut last));
        // Le sol change pendant la construction : on attend la fin de l'intervalle
        assert!(!ground_lines_stale(Some(5), 6, 100, 1.2, &mut last));
        assert_eq!(last, 1.0);
        assert!(ground_lines_stale(Some(5), 7, 100, 1.0 + GROUND_LINES_REFRESH_S, &mut last));
        // Relief terminé : le dernier recalcul part sans attendre, les tracés finissent à jour
        last = 2.0;
        assert!(ground_lines_stale(Some(7), 8, 0, 2.1, &mut last));
    }
}
