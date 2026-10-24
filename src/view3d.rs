//! Vue 3D optionnelle (touche V ou bouton) : même partie, rendue en relief.
//!
//! La vue 2D reste la référence. La 3D est un second « monde visuel » sur la couche de rendu 1,
//! vu par une caméra perspective inclinée ; il reflète l'état du jeu (cases, brouillard, unités,
//! villes, villages, camps, sélection, trajets) sans rien changer à la logique.
//! Repère 3D : x = x 2D, y = hauteur, z = −y 2D (le nord est au fond de l'écran).

use crate::game::{City, GameMap, ShownFog, HexPos, Selection, Territory, Unit};
use crate::hex::{DIRECTIONS, HEX_SIZE, Hex};
use crate::hostile::{Camp, Hostile, Village};
use crate::input::{HoverPath, OrderPath};
use crate::map::{MAX_LEVEL, Terrain};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use crate::render::{MapLayer, ReachArea, climate_color, hydro_color, topo_color, wind_arrow, wind_arrow_site};
use crate::units::UnitKind;
use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use crate::relief::{CHANNEL_LEVELS, Channel, HexTop, Link, MeshData, build_hex, carve, compute_top, mountain_mesh, water_surface};
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
}

impl Relief {
    /// Hauteur du sol au point 2D (x, y) du monde (lits des cours d'eau creusés).
    pub fn ground(&self, x: f32, y: f32) -> f32 {
        let hex = Hex::from_world(x, y);
        let terrain = self.surface(x, y);
        self.channels.get(&hex).map_or(terrain, |c| carve(c, Vec2::new(x, y), terrain).0)
    }

    /// Hauteur du sol sans les lits des cours d'eau.
    fn surface(&self, x: f32, y: f32) -> f32 {
        let hex = Hex::from_world(x, y);
        let (cx, cy) = hex.to_world();
        self.tops.get(&hex).map_or(unexplored_height(), |t| t.sample(Vec2::new(x - cx, y - cy)))
    }

    /// Mer ou lac exploré le plus proche d'un point : distance (approchée) et couleur.
    fn nearest_water(&self, p: Vec2) -> Option<(f32, Vec3)> {
        Hex::from_world(p.x, p.y)
            .range(2)
            .filter(|h| self.water_seen.contains(h))
            .filter_map(|h| {
                let (cx, cy) = h.to_world();
                let d = (p.distance(Vec2::new(cx, cy)) - HEX_SIZE * 0.866).max(0.0);
                self.water_colors.get(&h).map(|&c| (d, c))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// Lits visibles qui touchent une case : les siens et ceux de ses voisines (une arête est
    /// ainsi taillée pareil des deux côtés).
    fn channels_near(&mut self, hex: Hex, explored: impl Fn(Hex) -> bool + Copy) -> Vec<Channel> {
        let mut out: Vec<Channel> = Vec::new();
        let (cx, cy) = hex.to_world();
        let c = Vec2::new(cx, cy);
        for h in hex.range(1) {
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
                    let h = Hex::from_world(q.x, q.y);
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
            .filter(|h| self.water_seen.contains(h))
            .filter_map(|h| {
                let (cx, cy) = h.to_world();
                let d = (p.distance(Vec2::new(cx, cy)) - HEX_SIZE * 0.866).max(0.0);
                self.tops.get(&h).map(|t| (d, t.center))
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

/// Représentation 3D d'une entité du jeu (unité, ville, village, camp).
#[derive(Component)]
struct Mirror3d {
    source: Entity,
    lift: f32,
    /// Jeton d'unité : posé sur le toit de la ville s'il y en a une sur la case.
    is_unit: bool,
}

/// Hauteur du donjon d'une ville (une unité en garnison est posée dessus).
pub const CITY_HEIGHT: f32 = 12.0;

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct Gizmos3d;

/// Pièces d'une ville 3D : socle, remparts, tours d'angle, maisons et donjon central.
struct CityKit {
    base: Handle<Mesh>,
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
    ground: Handle<Mesh>,
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

/// Dimensions d'une ville 3D (repère local : origine sur le dessus de la case).
const CITY_BASE: f32 = 3.0;
const CITY_WALL_RADIUS: f32 = 26.0;
const CITY_WALL_HEIGHT: f32 = 7.0;
const CITY_TOWER_HEIGHT: f32 = 12.0;
const CITY_KEEP_ROOF: f32 = 1.5;

#[derive(Resource)]
struct Assets3d {
    /// Sol (la couleur du terrain est portée par les sommets) : [terre, eau], vu / hors de vue.
    ground: [Handle<StandardMaterial>; 2],
    ground_dim: [Handle<StandardMaterial>; 2],
    unexplored: Handle<StandardMaterial>,
    /// Eau douce (couleur portée par les sommets) : vue, hors de vue.
    fresh_water: [Handle<StandardMaterial>; 2],
    token: Handle<Mesh>,
    icon_quad: Handle<Mesh>,
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
                    draw_overlays_3d,
                    draw_wind_3d,
                )
                    .chain(),
            );
    }
}

/// Niveau de la mer en 3D.
const SEA_LEVEL: f32 = 8.0;
/// Hauteur d'un niveau d'altitude en 3D.
const LEVEL_STEP: f32 = 8.0;
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

/// Hauteur du dessus d'une case : eau plus ou moins profonde, terre selon son niveau
/// (les collines et les montagnes ont leur propre décor par-dessus).
pub fn tile_height(t: Terrain, level: u8) -> f32 {
    match t {
        // Même niveau de mer pour la côte et le large : seule la couleur indique la profondeur
        Terrain::Ocean | Terrain::Coast => SEA_LEVEL,
        // Lac : un peu sous les terres de son niveau, qui descendent jusqu'à lui
        Terrain::Lake => SEA_LEVEL + f32::from(level) * LEVEL_STEP - LAKE_DEPTH,
        _ => SEA_LEVEL + f32::from(level) * LEVEL_STEP,
    }
}

/// Hauteur d'une case inexplorée (plate, au niveau du littoral).
fn unexplored_height() -> f32 {
    tile_height(Terrain::Plains, 1)
}

/// Hauteur affichée d'une case : relief connu, ou plat si elle est inexplorée.
pub fn shown_height(map: &GameMap, fog: &ShownFog, hex: Hex) -> f32 {
    match (map.0.get(hex), map.0.level(hex)) {
        (Some(t), Some(level)) if fog.explored.contains(&hex) => tile_height(t, level),
        _ => unexplored_height(),
    }
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
    let tree = mat(Color::srgb(0.10, 0.30, 0.12), 0.9);
    let rock = mat(Color::srgb(0.50, 0.48, 0.47), 0.95);
    let boulder = mat(Color::srgb(0.46, 0.42, 0.38), 1.0);
    // Montagnes : couleurs portées par les sommets du maillage
    let mountain = mat(Color::WHITE, 0.95);
    // Bosses des collines : couleur du terrain de base, un peu plus sombre
    let hill: HashMap<Terrain, Handle<StandardMaterial>> = Terrain::ALL
        .into_iter()
        .map(|t| {
            let (r, g, b) = t.color();
            let k = TERRAIN_TINT * 0.88;
            (t, mat(Color::srgb(r * k, g * k, b * k), 0.95))
        })
        .collect();

    let tree_mesh = meshes.add(Cone { radius: 5.5, height: 17.0 });
    // Bosses des collines : demi-sphères aplaties
    let mound_mesh = meshes.add(Sphere::new(10.0).mesh().uv(16, 8));

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
    let mut relief = Relief { tops: HashMap::new(), tiles: HashMap::new(), decor: HashMap::new(), water: HashMap::new(), channels: HashMap::new(), axis_cache: HashMap::new(), explored: HashSet::new(), hatches: HashMap::new(), water_seen: HashSet::new(), water_colors: map.0.hexes().filter_map(|h| map.0.get(h).filter(|t| t.is_water()).map(|t| (h, terrain_color(t)))).collect(), water_hexes: map.0.hexes().filter(|&h| map.0.get(h).is_some_and(|t| t.is_water())).collect() };
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
        let mesh = build_hex(hex, &relief.tops[&hex], Vec3::ONE, None, &[], false);
        let tile = commands.spawn((Tile3d, Mesh3d(meshes.add(to_mesh(mesh))), MeshMaterial3d(unexplored.clone()), layer())).id();
        relief.tiles.insert(hex, tile);

        // Décor : arbres en forêt, bosses en colline, pic en montagne (hauteurs fixées avec le relief)
        let t = map.0.get(hex).unwrap();
        let polar = |deg: f32, r: f32| Vec2::from_angle(deg.to_radians()) * r;
        let mut pieces: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Vec2, f32, Vec3)> = Vec::new();
        // Collines (relief interne à la case) : trois bosses de la couleur du terrain
        const MOUNDS: [(f32, f32, f32); 3] = [(90.0, 12.0, 1.0), (210.0, 13.0, 0.85), (330.0, 12.5, 0.75)];
        let hills = map.0.hills(hex);
        if hills {
            for (a, r, k) in MOUNDS {
                pieces.push((mound_mesh.clone(), hill[&t].clone(), polar(a, r), -1.0, Vec3::new(k, 0.75 * k, k)));
            }
        }
        // (pas d'arbres dans le lit d'un fleuve)
        if t == Terrain::Forest && !map.0.is_fleuve(hex) {
            if hills {
                // Forêt vallonnée : un arbre au sommet de chaque bosse, trois entre elles
                for (a, r, k) in MOUNDS {
                    pieces.push((tree_mesh.clone(), tree.clone(), polar(a, r), 8.0 + 7.0 * k, Vec3::ONE));
                }
                for a in [30.0_f32, 150.0, 270.0] {
                    pieces.push((tree_mesh.clone(), tree.clone(), polar(a, 21.0), 8.0, Vec3::ONE));
                }
            } else {
                // Arbres en couronne près du bord : le centre reste libre pour le jeton d'une unité
                for a in [30.0_f32, 90.0, 150.0, 210.0, 270.0, 330.0] {
                    pieces.push((tree_mesh.clone(), tree.clone(), polar(a, 21.0), 8.0, Vec3::ONE));
                }
            }
        }
        pieces.extend(match t {
            // Montagne en relief, différente sur chaque case (sommets, arêtes, neige)
            Terrain::Mountains => {
                let seed = (hex.q * 7919 + hex.r * 104_729).unsigned_abs();
                let ridges = std::array::from_fn(|d| map.0.get(hex + DIRECTIONS[d]) == Some(Terrain::Mountains));
                vec![(meshes.add(to_mesh(mountain_mesh(seed, ridges))), mountain.clone(), Vec2::ZERO, 0.0, Vec3::ONE)]
            }
            _ => Vec::new(),
        });
        let mut ids = Vec::new();
        for (mesh, material, offset, lift, scale) in pieces {
            let vegetation = mesh == tree_mesh || mesh == mound_mesh;
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
        let low = edge.low();
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
                            hex: low,
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
        relief.decor.entry(low).or_default().extend(ids);
    }
    commands.insert_resource(relief);

    commands.insert_resource(Assets3d {
        ground,
        ground_dim,
        unexplored,
        fresh_water,
        token: meshes.add(Cylinder::new(16.0, 6.0)),
        icon_quad: meshes.add(Rectangle::new(26.0, 26.0)),
        player,
        hostile,
        city: CityKit {
            base: meshes.add(Extrusion::new(RegularPolygon::new(HEX_SIZE * 0.9, 6), CITY_BASE)),
            wall: meshes.add(Cuboid::new(CITY_WALL_RADIUS, CITY_WALL_HEIGHT, 3.0)),
            tower: meshes.add(Cylinder::new(4.0, CITY_TOWER_HEIGHT)),
            tower_roof: meshes.add(Cone { radius: 5.2, height: 6.0 }),
            house: meshes.add(Cuboid::new(5.0, 5.0, 5.0)),
            house_roof: meshes.add(Cone { radius: 4.4, height: 4.0 }.mesh().resolution(4)),
            keep: meshes.add(Cylinder::new(10.0, CITY_HEIGHT - CITY_BASE - CITY_KEEP_ROOF)),
            keep_roof: meshes.add(Cylinder::new(11.5, CITY_KEEP_ROOF)),
            stone: city_stone,
            paving: city_paving,
            plaster: city_plaster,
            tiles: city_tiles,
        },
        sites: SiteKit {
            ground: meshes.add(Cylinder::new(25.0, 1.5)),
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
    mut cam2d_q: Query<(Entity, &mut Camera, &mut Transform), (With<Camera2d>, Without<Camera3d>)>,
    mut cam3d_q: Query<(Entity, &mut Camera, &mut Transform), (With<Camera3d>, Without<Camera2d>)>,
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
    mut camera: Query<&mut Transform, With<Camera3d>>,
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
/// rocheux sinon (falaise, falaise côtière) ; bord de l'inexploré ou terrasse d'une ville ou
/// d'un camp → paroi.
fn link(map: &GameMap, fog: &ShownFog, terraces: &HashSet<Hex>, a: Hex, b: Hex) -> Link {
    let (Some(ta), Some(tb)) = (map.0.get(a), map.0.get(b)) else { return Link::None };
    match (fog.explored.contains(&a), fog.explored.contains(&b)) {
        (false, false) => Link::Gentle,
        (true, true) if !terraces.contains(&a) && !terraces.contains(&b) => match (ta.is_water(), tb.is_water()) {
            (true, true) => Link::Gentle,
            // Terre–terre comme terre–mer (la mer au niveau 0) : plage ou pente si l'écart le
            // permet, falaise sinon (la mer reste plate, voir `compute_top`)
            (wa, wb) => {
                let (la, lb) = (map.0.level(a).unwrap_or(0), map.0.level(b).unwrap_or(0));
                match (crate::map::slope_cost(la, lb).is_some(), wa || wb) {
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
/// limité à chaque image en commençant par les plus proches de la caméra. Rien n'est recalculé
/// tant que la vue 3D n'est pas affichée.
fn update_relief_3d(
    fog: Res<ShownFog>,
    map: Res<GameMap>,
    mode: Res<ViewMode>,
    cam: Res<Cam3d>,
    assets: Option<Res<Assets3d>>,
    relief: Option<ResMut<Relief>>,
    structures: Query<&HexPos, Or<(With<City>, With<Camp>)>>,
    mut known: Local<ReliefQueue>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut tiles: Query<(&Mesh3d, &mut MeshMaterial3d<StandardMaterial>), With<Tile3d>>,
    mut decor: Query<(&Decor3d, &mut Transform)>,
    mut water_materials: Query<&mut MeshMaterial3d<StandardMaterial>, Without<Tile3d>>,
    water_meshes: Query<&Mesh3d, Without<Tile3d>>,
    layer: Res<MapLayer>,
    mut commands: Commands,
) {
    let (Some(assets), Some(mut relief)) = (assets, relief) else { return };
    let queue = &mut *known;
    let terraces: HashSet<Hex> = structures.iter().map(|p| p.0).collect();
    if fog.is_changed() || terraces != queue.terraces {
        let changed: Vec<Hex> = fog
            .explored
            // Symétrique : en quittant le mode debug, des cases redeviennent inconnues
            .symmetric_difference(&queue.explored)
            .chain(terraces.symmetric_difference(&queue.terraces))
            .copied()
            .collect();
        queue.geometry.extend(changed.iter().flat_map(|h| h.range(2)).filter(|h| map.0.contains(*h)));
        queue.explored = fog.explored.clone();
        queue.terraces = terraces.clone();
    }
    // Changement de carte (normale / relief / climat) : seules les couleurs changent, le relief déjà
    // calculé est réutilisé. La vision peut déborder du bord de la carte : on ne garde que la carte.
    if layer.is_changed() {
        queue.color.extend(fog.explored.iter().copied().filter(|h| map.0.contains(*h)));
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
            let lit = fog.visible.contains(hex) || hex.neighbors().iter().any(|n| fog.visible.contains(n));
            let wanted = &assets.fresh_water[usize::from(!lit)];
            if let Ok(mut material) = water_materials.get_mut(*e)
                && material.0 != *wanted
            {
                material.0 = wanted.clone();
            }
        }
    }
    if !mode.is_3d || (queue.geometry.is_empty() && queue.color.is_empty()) {
        return;
    }

    // Les cases les plus proches du centre de la vue d'abord
    let focus = Hex::from_world(cam.target.x, -cam.target.z);
    let mut batch: Vec<Hex> = queue.geometry.iter().chain(queue.color.difference(&queue.geometry)).copied().collect();
    let priority = |h: &Hex| h.distance(focus);
    if batch.len() > RELIEF_BATCH {
        batch.select_nth_unstable_by_key(RELIEF_BATCH, priority);
        batch.truncate(RELIEF_BATCH);
    }
    batch.sort_unstable_by_key(priority);

    let height = |h: Hex| shown_height(&map, &fog, h);
    let link = |a: Hex, b: Hex| link(&map, &fog, &terraces, a, b);
    let water = |h: Hex| fog.explored.contains(&h) && map.0.get(h).is_some_and(|t| t.is_water());
    // Couleur du sol d'une case selon la carte affichée
    let hex_color = |hex: Hex| match (fog.explored.contains(&hex), *layer) {
        (false, _) => Vec3::ONE,
        // Lit d'un fleuve en forêt : prairie de berge (pas d'arbres dans le lit)
        (true, MapLayer::Normal) if map.0.is_fleuve(hex) && map.0.get(hex) == Some(Terrain::Forest) => {
            terrain_color(Terrain::Grassland)
        }
        (true, MapLayer::Normal) => terrain_color(map.0.get(hex).unwrap()),
        (true, thematic) => {
            let t = map.0.get(hex).unwrap();
            let (r, g, b) = match thematic {
                MapLayer::Relief => topo_color(t, map.0.level(hex).unwrap_or(1)),
                MapLayer::Climate => climate_color(t, map.0.rain(hex).unwrap_or(500.0)),
                _ => hydro_color(&map.0, hex, thematic),
            };
            let c = Color::srgb(r, g, b).to_linear();
            Vec3::new(c.red, c.green, c.blue)
        }
    };
    let started = std::time::Instant::now();
    let mut reshaped = Vec::new();
    let mut processed = Vec::new();
    let mut pending = batch.into_iter().peekable();
    // Par paquets : préparation (relief, lits) dans l'ordre, puis maillages construits en
    // parallèle sur tous les cœurs, jusqu'à épuisement du temps alloué à l'image
    while pending.peek().is_some() && started.elapsed().as_secs_f32() * 1000.0 < RELIEF_BUDGET_MS {
        let chunk: Vec<Hex> = pending.by_ref().take(RELIEF_CHUNK).collect();
        let count = chunk.len();
        let mut jobs = Vec::with_capacity(count);
        for hex in chunk {
            let reshape = queue.geometry.remove(&hex);
            queue.color.remove(&hex);
            if reshape || !relief.tops.contains_key(&hex) {
                relief.tops.insert(hex, compute_top(hex, height, link, water));
                reshaped.push(hex);
                // Les hauteurs d'eau des lits proches dépendent de ce relief
                for h in hex.range(2) {
                    relief.axis_cache.remove(&h);
                }
            }
            processed.push(hex);
        }
        for &hex in &processed[processed.len() - count..] {
            // Lits qui touchent la case (la surface d'un lac ou de la mer reste plate, une case
            // inexplorée aussi)
            let near = if water(hex) || !fog.explored.contains(&hex) { Vec::new() } else { relief.channels_near(hex, |h| fog.explored.contains(&h)) };
            if near.is_empty() {
                relief.channels.remove(&hex);
            } else {
                relief.channels.insert(hex, near.clone());
            }
            let color = hex_color(hex);
            // Case de fleuve : ses berges prennent la couleur des cases voisines (de terre)
            let sides = (map.0.is_fleuve(hex) && fog.explored.contains(&hex)).then(|| {
                std::array::from_fn(|d| {
                    let n = hex + DIRECTIONS[d];
                    let land = map.0.get(n).is_some_and(|t| !t.is_water()) && !map.0.is_fleuve(n);
                    if land && fog.explored.contains(&n) { hex_color(n) } else { color }
                })
            });
            jobs.push((hex, relief.tops[&hex], color, sides, near, water(hex)));
        }
        let built: Vec<(Hex, MeshData)> =
            jobs.into_par_iter().map(|(hex, top, color, sides, near, skirt)| (hex, build_hex(hex, &top, color, sides.as_ref(), &near, skirt))).collect();
        for (hex, data) in built {
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
    }
    // Le relief d'une case change la hauteur de l'eau des lits voisins : les cases proches qui
    // ont des cours d'eau sont reconstruites (couleurs et lits seulement)
    let done: HashSet<Hex> = processed.iter().copied().collect();
    for hex in &reshaped {
        for h in hex.range(3) {
            if !done.contains(&h)
                && !queue.geometry.contains(&h)
                && relief.channels.contains_key(&h)
                && map.0.contains(h)
            {
                queue.color.insert(h);
                queue.levels.insert(h);
            }
        }
    }
    // Surfaces d'eau des cases reconstruites et de leurs voisines (une rivière d'arête est
    // dessinée par une seule des deux cases)
    // (un simple changement de carte ne change pas l'eau : rien à refaire)
    let mut redo: HashSet<Hex> = HashSet::new();
    let reshaped: HashSet<Hex> = reshaped.into_iter().collect();
    for hex in processed {
        if !queue.levels.remove(&hex) && !reshaped.contains(&hex) {
            continue;
        }
        redo.extend(hex.range(1).filter(|h| relief.water.contains_key(h)));
    }
    let explored = |h: Hex| fog.explored.contains(&h);
    for hex in redo {
        let (shape, e) = &relief.water[&hex];
        // Case inexplorée qui porte une rivière d'arête visible (sa rive voisine est explorée) :
        // elle n'a pas de lits à elle, on prend ceux des voisines, qui ont taillé le lit (sinon
        // l'eau, à une hauteur fixe, passait sous la berge)
        let near: Vec<Channel> = match relief.channels.get(&hex) {
            Some(c) => c.clone(),
            None => {
                let mut v: Vec<Channel> = Vec::new();
                for ch in hex.range(1).filter_map(|h| relief.channels.get(&h)).flatten() {
                    if !v.contains(ch) {
                        v.push(*ch);
                    }
                }
                v
            }
        };
        let data = water_mesh(&relief, &near, shape, shape.visible(hex, explored), explored);
        if let Ok(mesh) = water_meshes.get(*e)
            && let Some(mut m) = meshes.get_mut(&mesh.0)
        {
            *m = to_mesh(data);
        }
    }
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
        // Même surface que celle qui a servi à tailler le lit (lits de la case et des voisines)
        let ch = channels.entry(t.channel).or_insert_with(|| relief.channel(&shape.channels[t.channel]));
        let ys = pts.map(|p| water_surface(near, p).unwrap_or_else(|| ch.levels[0]));
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
    terraces: HashSet<Hex>,
    /// Relief à recalculer (et couleurs).
    geometry: HashSet<Hex>,
    /// Couleurs seulement (changement de carte).
    color: HashSet<Hex>,
    /// Hauteurs d'eau des lits à revoir (relief voisin changé) : surfaces d'eau à refaire aussi.
    levels: HashSet<Hex>,
}

/// Cases du relief 3D examinées par image, et temps de calcul maximal par image (ms).
const RELIEF_BATCH: usize = 4_000;
const RELIEF_BUDGET_MS: f32 = 6.0;
/// Cases préparées puis construites ensemble (en parallèle).
const RELIEF_CHUNK: usize = 12;

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
    let occupied: std::collections::HashSet<Hex> = structures.iter().map(|p| p.0).collect();
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
    units: Query<(Entity, &Unit, Has<Hostile>), Added<Unit>>,
    cities: Query<Entity, Added<City>>,
    villages: Query<Entity, Added<Village>>,
    camps: Query<Entity, Added<Camp>>,
) {
    let Some(a) = assets else { return };
    let upright = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let icon = |commands: &mut Commands, parent: Entity, path: &'static str, y: f32| {
        commands.entity(parent).with_children(|p| {
            p.spawn((
                Mesh3d(a.icon_quad.clone()),
                MeshMaterial3d(a.icons[path].clone()),
                Transform::from_xyz(0.0, y, 0.0).with_rotation(upright),
                layer(),
            ));
        });
    };
    for (source, unit, hostile) in &units {
        let material = if hostile { &a.hostile } else { &a.player };
        let e = commands
            .spawn((
                Mirror3d { source, lift: 3.0, is_unit: true },
                Mesh3d(a.token.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Visibility::Hidden,
                layer(),
            ))
            .id();
        icon(&mut commands, e, unit.0.kind.icon(), 3.2);
    }
    for source in &cities {
        spawn_city_model(&mut commands, &a, source);
    }
    for source in &villages {
        spawn_site_model(&mut commands, &a, source, false);
    }
    for source in &camps {
        spawn_site_model(&mut commands, &a, source, true);
    }
}

/// Village : huttes rondes au toit de chaume autour d'un feu.
/// Camp hostile : palissade de pieux, trois tentes rouges, un feu et un étendard.
fn spawn_site_model(commands: &mut Commands, a: &Assets3d, source: Entity, is_camp: bool) {
    let k = &a.sites;
    let dir = |deg: f32| {
        let (sin, cos) = deg.to_radians().sin_cos();
        Vec3::new(cos, 0.0, -sin)
    };
    let part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, transform: Transform| {
        (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform, layer())
    };
    // Le village est posé directement sur le terrain de la case ; le camp a son sol de terre battue
    let floor = if is_camp { 1.5 } else { 0.0 };
    commands
        .spawn((Mirror3d { source, lift: 0.0, is_unit: false }, Transform::default(), Visibility::Hidden, layer()))
        .with_children(|p| {
            if is_camp {
                p.spawn(part(&k.ground, &k.camp_ground, Transform::from_xyz(0.0, floor / 2.0, 0.0)));
                for i in 0..18 {
                    let at = dir(20.0 * i as f32) * 22.5;
                    p.spawn(part(&k.stake, &k.wood, Transform::from_translation(at + Vec3::Y * (floor + 4.0))));
                    p.spawn(part(&k.stake_tip, &k.wood, Transform::from_translation(at + Vec3::Y * (floor + 9.25))));
                }
                for i in 0..3 {
                    let angle = 90.0 + 120.0 * i as f32;
                    let at = dir(angle) * 11.5;
                    let turn = Quat::from_rotation_y(angle.to_radians());
                    p.spawn(part(&k.tent, &k.tent_red, Transform::from_translation(at + Vec3::Y * (floor + 5.0)).with_rotation(turn)));
                }
                // Étendard planté près du feu
                let pole = dir(30.0) * 6.0;
                p.spawn(part(&k.pole, &k.wood, Transform::from_translation(pole + Vec3::Y * (floor + 10.0))));
                p.spawn(part(&k.flag, &k.tent_red, Transform::from_translation(pole + Vec3::new(4.0, floor + 17.0, 0.0))));
            } else {
                for i in 0..5 {
                    let at = dir(90.0 + 72.0 * i as f32) * 14.0;
                    p.spawn(part(&k.hut, &k.mud, Transform::from_translation(at + Vec3::Y * (floor + 2.5))));
                    p.spawn(part(&k.hut_roof, &k.straw, Transform::from_translation(at + Vec3::Y * (floor + 5.0 + 3.75))));
                }
            }
            p.spawn(part(&k.fire, &k.flame, Transform::from_xyz(0.0, floor + 2.5, 0.0)));
            for i in 0..6 {
                let at = dir(60.0 * i as f32) * 3.4;
                p.spawn(part(&k.fire_stone, &k.stone, Transform::from_translation(at + Vec3::Y * (floor + 0.6))));
            }
        });
}

/// Ville 3D : socle pavé, remparts hexagonaux avec tours d'angle aux toits bleus,
/// maisons entre les remparts et le donjon central, sur lequel se tient la garnison.
fn spawn_city_model(commands: &mut Commands, a: &Assets3d, source: Entity) {
    let k = &a.city;
    let upright = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    // Direction 3D d'un angle de la vue 2D (x, −y)
    let dir = |deg: f32| {
        let (sin, cos) = deg.to_radians().sin_cos();
        Vec3::new(cos, 0.0, -sin)
    };
    let part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, transform: Transform| {
        (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform, layer())
    };
    commands
        .spawn((Mirror3d { source, lift: 0.0, is_unit: false }, Transform::default(), Visibility::Hidden, layer()))
        .with_children(|p| {
            p.spawn(part(&k.base, &k.paving, Transform::from_xyz(0.0, CITY_BASE / 2.0, 0.0).with_rotation(upright)));
            for i in 0..6 {
                // Remparts au milieu des côtés, tours aux sommets (comme les cases)
                let side = 60.0 * i as f32;
                let wall_at = dir(side) * CITY_WALL_RADIUS * 0.866 + Vec3::Y * (CITY_BASE + CITY_WALL_HEIGHT / 2.0);
                p.spawn(part(
                    &k.wall,
                    &k.stone,
                    Transform::from_translation(wall_at).with_rotation(Quat::from_rotation_y((side + 90.0).to_radians())),
                ));
                let corner = dir(side + 30.0) * CITY_WALL_RADIUS;
                p.spawn(part(
                    &k.tower,
                    &k.stone,
                    Transform::from_translation(corner + Vec3::Y * (CITY_BASE + CITY_TOWER_HEIGHT / 2.0)),
                ));
                p.spawn(part(
                    &k.tower_roof,
                    &a.player,
                    Transform::from_translation(corner + Vec3::Y * (CITY_BASE + CITY_TOWER_HEIGHT + 3.0)),
                ));
                let house = dir(side) * 16.5;
                let turn = Quat::from_rotation_y((side + 45.0).to_radians());
                p.spawn(part(&k.house, &k.plaster, Transform::from_translation(house + Vec3::Y * (CITY_BASE + 2.5)).with_rotation(turn)));
                p.spawn(part(
                    &k.house_roof,
                    &k.tiles,
                    Transform::from_translation(house + Vec3::Y * (CITY_BASE + 7.0)).with_rotation(turn),
                ));
            }
            let keep_height = CITY_HEIGHT - CITY_BASE - CITY_KEEP_ROOF;
            p.spawn(part(&k.keep, &k.stone, Transform::from_xyz(0.0, CITY_BASE + keep_height / 2.0, 0.0)));
            p.spawn(part(&k.keep_roof, &a.player, Transform::from_xyz(0.0, CITY_HEIGHT - CITY_KEEP_ROOF / 2.0, 0.0)));
        });
}

/// Position et visibilité des représentations 3D ; suppression quand l'entité du jeu disparaît.
fn sync_mirrors(
    mut commands: Commands,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    sources: Query<(&HexPos, Has<Hostile>, Has<Village>, Has<Camp>)>,
    cities: Query<&HexPos, With<City>>,
    mut mirrors: Query<(Entity, &Mirror3d, &mut Transform, &mut Visibility)>,
) {
    let city_tiles: std::collections::HashSet<Hex> = cities.iter().map(|p| p.0).collect();
    for (entity, mirror, mut transform, mut vis) in &mut mirrors {
        let Ok((pos, hostile, village, camp)) = sources.get(mirror.source) else {
            commands.entity(entity).despawn();
            continue;
        };
        let on_roof = if mirror.is_unit && city_tiles.contains(&pos.0) { CITY_HEIGHT } else { 0.0 };
        let wanted = to_3d(pos.0, shown_height(&map, &fog, pos.0) + mirror.lift + on_roof);
        if transform.translation != wanted {
            transform.translation = wanted;
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

/// Lignes 3D : frontières, zone accessible, cibles attaquables, sélection, trajets.
fn draw_overlays_3d(
    mode: Res<ViewMode>,
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
            if territory.0.get(&(hex + *d)) != Some(&owner) {
                edge(&mut gizmos, hex, dir, Color::srgb(0.45, 0.70, 1.0));
            }
        }
    }
    for &hex in &area.cells {
        for (dir, d) in DIRECTIONS.iter().enumerate() {
            if !area.cells.contains(&(hex + *d)) {
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
        let centers: Vec<Vec2> = std::iter::once(pos.0)
            .chain(path.iter().copied())
            .map(|h| {
                let (x, y) = h.to_world();
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

/// Case sous le curseur en vue 3D : on avance le long du rayon de la caméra, du haut du
/// relief vers le sol, jusqu'à passer sous le dessus de la case survolée (falaises comprises).
pub fn hovered_hex_3d(ray: Ray3d, relief: &Relief, map: &GameMap) -> Option<Hex> {
    let top = tile_height(Terrain::Plains, MAX_LEVEL);
    let ground = Vec3::Y;
    let start = ray.intersect_plane(Vec3::new(0.0, top, 0.0), InfinitePlane3d::new(ground))?;
    let end = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(ground))?;
    let steps = ((end - start) / 2.0).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let p = ray.get_point(start + (end - start) * i as f32 / steps as f32);
        let h = Hex::from_world(p.x, -p.z);
        if map.0.contains(h) && p.y <= relief.ground(p.x, -p.z) {
            return Some(h);
        }
    }
    let p = ray.get_point(end);
    Some(Hex::from_world(p.x, -p.z)).filter(|h| map.0.contains(*h))
}
