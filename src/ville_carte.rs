//! Écran de la ville posé sur la carte du monde (maquette v7, `conception-interface.md`) : ce que
//! la carte montre de la ville ouverte, en 2D comme en 3D. Couche colorée des cases du territoire,
//! reste du monde assombri, contours posés sur le relief (ils suivent les vraies cases, même en
//! 3D), étiquettes de l'interface placées sur les cases à chaque image, cadrage de la caméra dans la
//! partie de la carte que les panneaux laissent libre. L'écran de la ville (`ecran_ville.rs`) dit
//! quoi montrer dans `CoucheVille` ; ce module le dessine.

use crate::game::GameMap;
use crate::hex::{HEX_SIZE, Hex};
use crate::view3d::{Cam3d, Gizmos3d, Relief, ViewMode};
use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::mesh::PrimitiveTopology;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Rayon (en cases) autour du centre de la ville où le monde est assombri ; l'ombre s'efface sur
/// les `FONDU` dernières couronnes (pas de bord net quand on recule la caméra).
const RAYON_SOMBRE: i32 = 26;
const FONDU: i32 = 8;
/// Opacité de l'ombre posée sur le reste du monde (« assombrir légèrement », Benjamin).
const OMBRE: f32 = 0.42;
/// Hauteur des couches 3D au-dessus du sol, et subdivision d'un secteur d'hexagone (le maillage
/// épouse le relief sans le traverser).
const LEVEE_3D: f32 = 0.9;
const SUBDIVISION: usize = 4;
/// Plan 2D des couches : au-dessus du terrain, sous les villes et les unités.
const Z_COUCHE: f32 = 1.5;

/// Contour d'une case dessiné sur la carte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contour {
    pub hex: Hex,
    pub couleur: Color,
    /// Trait en pointillé (case à acheter, chantier en file).
    pub pointille: bool,
    /// Retrait vers le centre (0 : sur le bord ; 0,1 : un anneau intérieur, sous un autre contour).
    pub retrait: f32,
}

/// Flèche d'un échange entre deux villes (de centre à centre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fleche {
    pub de: Hex,
    pub vers: Hex,
    pub couleur: Color,
    /// Épaisseur (en traits côte à côte) : la quantité échangée.
    pub epaisseur: f32,
    /// Ville non reliée : trait en pointillé, sans pointe.
    pub pointille: bool,
}

/// Ce que la carte du monde montre de la ville ouverte (écrit par l'écran de la ville, seulement
/// quand cela change).
#[derive(Resource, Default, PartialEq)]
pub struct CoucheVille {
    /// L'écran de la ville est ouvert : couches, ombre et contours dessinés.
    pub ouverte: bool,
    pub centre: Hex,
    /// Teinte (avec son opacité) des cases montrées.
    pub teintes: Vec<(Hex, Color)>,
    /// Cases laissées claires (territoire, cases à acheter) ; les autres sont assombries.
    pub claires: HashSet<Hex>,
    /// Contours des cases (survol exclu : il change à chaque image, voir `SurvolVille`).
    pub contours: Vec<Contour>,
    /// Échanges du tour avec les autres villes.
    pub fleches: Vec<Fleche>,
}

/// Case survolée sur la carte pendant que l'écran est ouvert (contour blanc), et case de la file
/// ou de la liste survolée dans les panneaux.
#[derive(Resource, Default)]
pub struct SurvolVille(pub Option<Hex>);

/// Partie de la fenêtre (pixels logiques, y vers le bas) que les panneaux laissent voir : la ville
/// y est cadrée.
#[derive(Resource, Clone, Copy)]
pub struct ZoneLibre(pub Rect);

impl Default for ZoneLibre {
    fn default() -> Self {
        ZoneLibre(Rect::new(0.0, 0.0, 1400.0, 860.0))
    }
}

/// Demande de cadrage : les cases à montrer dans la zone libre (le vol de la caméra suit).
#[derive(Message)]
pub struct CadrerVille(pub Vec<Hex>);

/// Rayon d'une case à l'écran (pixels logiques) au centre de la ville : les étiquettes sont
/// taillées d'après lui (l'écran se refait quand il change nettement).
#[derive(Resource, Default)]
pub struct TailleCase(pub f32);

/// Étiquette de l'interface posée sur une case (centrée sur elle, à chaque image).
#[derive(Component)]
pub struct SurCase {
    /// Case, prise du côté du raccord le plus proche du centre de la ville.
    pub hex: Hex,
}

/// Vol de la caméra vers le cadrage voulu.
#[derive(Resource, Default)]
struct Vol(Option<But>);

#[derive(Clone, Copy)]
enum But {
    Plan { centre: Vec2, echelle: f32, temps: f32 },
    Relief { cible: Vec3, distance: f32, temps: f32 },
}

/// Maillages des couches (2D et 3D).
#[derive(Component)]
struct Couche2d;
#[derive(Component)]
struct Couche3d;

pub struct VilleCartePlugin;

impl Plugin for VilleCartePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CoucheVille>()
            .init_resource::<SurvolVille>()
            .init_resource::<ZoneLibre>()
            .init_resource::<TailleCase>()
            .init_resource::<Vol>()
            .add_message::<CadrerVille>()
            .add_systems(Startup, creer_couches)
            .add_systems(Update, (cadrer, voler, maillages, contours, contours_3d, mesurer_case).chain())
            .add_systems(PostUpdate, placer_etiquettes.after(bevy::camera::CameraUpdateSystems).before(bevy::ui::UiSystems::Prepare));
    }
}

fn creer_couches(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>, mut materiaux: ResMut<Assets<StandardMaterial>>) {
    let plan = materials.add(ColorMaterial { alpha_mode: bevy::sprite_render::AlphaMode2d::Blend, ..ColorMaterial::from_color(Color::WHITE) });
    commands.spawn((Couche2d, Mesh2d(meshes.add(vide())), MeshMaterial2d(plan), Transform::from_xyz(0.0, 0.0, Z_COUCHE), Visibility::Hidden));
    // Non éclairé : une teinte posée sur le sol, pas une surface de plus
    let relief = materiaux.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, depth_bias: 2.0, ..default() });
    commands.spawn((Couche3d, Mesh3d(meshes.add(vide())), MeshMaterial3d(relief), Transform::default(), Visibility::Hidden, crate::view3d::couche_3d()));
}

/// Maillage vide (un triangle dégénéré : un maillage sans sommet est refusé).
fn vide() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3])
}

/// Sommets d'un hexagone (monde 2D), dans l'ordre des directions.
fn sommets(h: Hex) -> [Vec2; 6] {
    std::array::from_fn(|d| {
        let (a, _) = h.edge_corners(d);
        Vec2::new(a.0, a.1)
    })
}

/// Teinte de chaque case dessinée : couches de l'écran, puis ombre du reste du monde.
fn cases_teintees(map: &crate::map::WorldMap, couche: &CoucheVille) -> Vec<(Hex, [f32; 4])> {
    let wrap = map.wrap();
    let mut v: Vec<(Hex, [f32; 4])> = couche.teintes.iter().map(|(h, c)| (wrap.nearest(couche.centre, *h), c.to_linear().to_f32_array())).collect();
    for h in couche.centre.range(RAYON_SOMBRE) {
        let k = map.canonical(h);
        if !map.contains(k) || couche.claires.contains(&k) {
            continue;
        }
        let d = couche.centre.distance(h);
        let fondu = ((RAYON_SOMBRE - d) as f32 / FONDU as f32).clamp(0.0, 1.0);
        v.push((h, [0.0, 0.0, 0.0, OMBRE * fondu]));
    }
    v
}

/// Refait les maillages quand ce que montre la ville change, quand la vue bascule, ou quand le
/// sol 3D change de forme.
fn maillages(
    couche: Res<CoucheVille>,
    map: Res<GameMap>,
    mode: Res<ViewMode>,
    relief: Option<Res<Relief>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plan: Single<(&Mesh2d, &mut Visibility), (With<Couche2d>, Without<Couche3d>)>,
    mut sol: Single<(&Mesh3d, &mut Visibility), (With<Couche3d>, Without<Couche2d>)>,
    mut forme: Local<Option<u32>>,
) {
    let forme_sol = relief.as_ref().map(|r| r.shape);
    let refaire = couche.is_changed() || mode.is_changed() || (mode.is_3d && *forme != forme_sol);
    if !refaire {
        return;
    }
    *forme = forme_sol;
    let (vis2, vis3) = match (couche.ouverte, mode.is_3d) {
        (false, _) => (Visibility::Hidden, Visibility::Hidden),
        (true, false) => (Visibility::Inherited, Visibility::Hidden),
        (true, true) => (Visibility::Hidden, Visibility::Inherited),
    };
    if *plan.1 != vis2 {
        *plan.1 = vis2;
    }
    if *sol.1 != vis3 {
        *sol.1 = vis3;
    }
    if !couche.ouverte {
        return;
    }
    let cases = cases_teintees(&map.0, &couche);
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut couleurs: Vec<[f32; 4]> = Vec::new();
    if !mode.is_3d {
        for (h, c) in &cases {
            let (x, y) = h.to_world();
            let centre = Vec2::new(x, y);
            let s = sommets(*h);
            for d in 0..6 {
                for p in [centre, s[d], s[(d + 1) % 6]] {
                    positions.push([p.x, p.y, 0.0]);
                    couleurs.push(*c);
                }
            }
        }
        let n = positions.len();
        if let Some(mut m) = meshes.get_mut(&plan.0.0) {
            *m = if n == 0 { vide() } else { maillage(positions, vec![[0.0, 0.0, 1.0]; n], couleurs) };
        }
        return;
    }
    let Some(relief) = relief else { return };
    // Secteurs subdivisés : chaque point prend la hauteur du sol (les points d'une arête sont les
    // mêmes des deux côtés, sans fente entre deux cases)
    let k = SUBDIVISION;
    for (h, c) in &cases {
        let (x, y) = h.to_world();
        let centre = Vec2::new(x, y);
        let s = sommets(*h);
        let sol = |p: Vec2| relief.on_ground(p.x, p.y, LEVEE_3D);
        for d in 0..6 {
            let (u, v) = (s[d] - centre, s[(d + 1) % 6] - centre);
            let point = |a: usize, b: usize| sol(centre + u * (a as f32 / k as f32) + v * (b as f32 / k as f32));
            for a in 0..k {
                for b in 0..(k - a) {
                    for p in [point(a, b), point(a + 1, b), point(a, b + 1)] {
                        positions.push(p.to_array());
                        couleurs.push(*c);
                    }
                    if a + b + 1 < k {
                        for p in [point(a + 1, b), point(a + 1, b + 1), point(a, b + 1)] {
                            positions.push(p.to_array());
                            couleurs.push(*c);
                        }
                    }
                }
            }
        }
    }
    let n = positions.len();
    if let Some(mut m) = meshes.get_mut(&sol.0.0) {
        *m = if n == 0 { vide() } else { maillage(positions, vec![[0.0, 1.0, 0.0]; n], couleurs) };
    }
}

fn maillage(positions: Vec<[f32; 3]>, normales: Vec<[f32; 3]>, couleurs: Vec<[f32; 4]>) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normales)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, couleurs)
}

/// Segments d'un contour (monde 2D) : les six arêtes, en retrait vers le centre, coupées en tirets
/// pour un pointillé.
fn traits(c: &Contour, centre: Hex, wrap: crate::hex::Wrap) -> Vec<(Vec2, Vec2)> {
    let h = wrap.nearest(centre, c.hex);
    let (x, y) = h.to_world();
    let m = Vec2::new(x, y);
    let s = sommets(h).map(|p| p.lerp(m, c.retrait));
    let mut v = Vec::new();
    for d in 0..6 {
        let (a, b) = (s[d], s[(d + 1) % 6]);
        if c.pointille {
            for i in 0..5 {
                let (t0, t1) = (i as f32 / 5.0, (i as f32 + 0.55) / 5.0);
                v.push((a.lerp(b, t0), a.lerp(b, t1)));
            }
        } else {
            v.push((a, b));
        }
    }
    v
}

/// Segments d'une flèche (monde 2D) : traits côte à côte selon l'épaisseur, pointe à l'arrivée
/// (arrêtée au bord de la case), tirets pour une ville non reliée.
fn segments_fleche(f: &Fleche, centre: Hex, wrap: crate::hex::Wrap) -> Vec<(Vec2, Vec2)> {
    let a = Vec2::from(wrap.nearest(centre, f.de).to_world());
    let b = Vec2::from(wrap.nearest(a_hex(a), f.vers).to_world());
    let dir = (b - a).normalize_or_zero();
    let (a, b) = (a + dir * HEX_SIZE * 0.9, b - dir * HEX_SIZE * 0.9);
    let cote = dir.perp();
    let mut v = Vec::new();
    if f.pointille {
        let n = ((b - a).length() / 14.0).max(1.0) as usize;
        for i in (0..n).step_by(2) {
            v.push((a.lerp(b, i as f32 / n as f32), a.lerp(b, (i + 1) as f32 / n as f32)));
        }
        return v;
    }
    let traits = f.epaisseur.round().max(1.0) as i32;
    for k in 0..traits {
        let d = cote * (k as f32 - (traits - 1) as f32 / 2.0) * 1.6;
        v.push((a + d, b + d));
    }
    let pointe = HEX_SIZE * 0.45;
    v.push((b, b - dir * pointe + cote * pointe * 0.6));
    v.push((b, b - dir * pointe - cote * pointe * 0.6));
    v
}

fn a_hex(p: Vec2) -> Hex {
    Hex::from_world(p.x, p.y)
}

/// Contours en 2D.
fn contours(couche: Res<CoucheVille>, survol: Res<SurvolVille>, mode: Res<ViewMode>, map: Res<GameMap>, mut gizmos: Gizmos) {
    if !couche.ouverte || mode.is_3d {
        return;
    }
    let wrap = map.0.wrap();
    let blanc = survol.0.map(|hex| Contour { hex, couleur: Color::srgba(1.0, 1.0, 1.0, 0.8), pointille: false, retrait: 0.04 });
    for c in couche.contours.iter().chain(blanc.as_ref()) {
        for (a, b) in traits(c, couche.centre, wrap) {
            gizmos.line_2d(a, b, c.couleur);
        }
    }
    for f in &couche.fleches {
        for (a, b) in segments_fleche(f, couche.centre, wrap) {
            gizmos.line_2d(a, b, f.couleur);
        }
    }
}

/// Contours en 3D : chaque trait échantillonné sur le sol (il suit le relief et les vraies cases).
fn contours_3d(couche: Res<CoucheVille>, survol: Res<SurvolVille>, mode: Res<ViewMode>, map: Res<GameMap>, relief: Option<Res<Relief>>, mut gizmos: Gizmos<Gizmos3d>) {
    let Some(relief) = relief else { return };
    if !couche.ouverte || !mode.is_3d {
        return;
    }
    let wrap = map.0.wrap();
    let blanc = survol.0.map(|hex| Contour { hex, couleur: Color::srgba(1.0, 1.0, 1.0, 0.8), pointille: false, retrait: 0.04 });
    for c in couche.contours.iter().chain(blanc.as_ref()) {
        for (a, b) in traits(c, couche.centre, wrap) {
            let n = if c.pointille { 2 } else { 6 };
            gizmos.linestrip((0..=n).map(|k| a.lerp(b, k as f32 / n as f32)).map(|p| relief.on_ground(p.x, p.y, LEVEE_3D + 0.4)), c.couleur);
        }
    }
    for f in &couche.fleches {
        for (a, b) in segments_fleche(f, couche.centre, wrap) {
            let n = ((b - a).length() / 8.0).ceil().max(1.0) as usize;
            gizmos.linestrip((0..=n).map(|k| a.lerp(b, k as f32 / n as f32)).map(|p| relief.on_ground(p.x, p.y, LEVEE_3D + 3.0)), f.couleur);
        }
    }
}

/// Point du monde (repère de la caméra active) au centre d'une case.
fn point_monde(h: Hex, relief: Option<&Relief>, is_3d: bool) -> Vec3 {
    let (x, y) = h.to_world();
    match relief.filter(|_| is_3d) {
        Some(r) => r.on_ground(x, y, 0.0),
        None => Vec3::new(x, y, 0.0),
    }
}

/// Pose chaque étiquette sur sa case (centrée), cachée si la case est derrière la caméra.
#[allow(clippy::type_complexity)]
fn placer_etiquettes(
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    relief: Option<Res<Relief>>,
    cameras: Query<(&Camera, &GlobalTransform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut etiquettes: Query<(&SurCase, &mut Node, &ComputedNode, &mut Visibility)>,
) {
    let Some((camera, global, _)) = cameras.iter().find(|(c, _, is_3d)| c.is_active && *is_3d == mode.is_3d) else { return };
    let cam_x = global.translation().x;
    for (s, mut node, calcule, mut vis) in &mut etiquettes {
        let p = crate::labels::near_camera(&map, point_monde(s.hex, relief.as_deref(), mode.is_3d), cam_x);
        let ecran = camera.world_to_viewport(global, p).ok();
        let voulu = if ecran.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != voulu {
            *vis = voulu;
        }
        let Some(e) = ecran else { continue };
        let taille = calcule.size() * calcule.inverse_scale_factor();
        let (l, t) = (Val::Px((e.x - taille.x / 2.0).round()), Val::Px((e.y - taille.y / 2.0).round()));
        if node.left != l || node.top != t {
            node.left = l;
            node.top = t;
        }
    }
}

/// Rayon à l'écran de la case du centre de la ville (pour tailler les étiquettes).
fn mesurer_case(
    couche: Res<CoucheVille>,
    mode: Res<ViewMode>,
    relief: Option<Res<Relief>>,
    cameras: Query<(&Camera, &GlobalTransform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut taille: ResMut<TailleCase>,
) {
    if !couche.ouverte {
        return;
    }
    let Some((camera, global, _)) = cameras.iter().find(|(c, _, is_3d)| c.is_active && *is_3d == mode.is_3d) else { return };
    let p = point_monde(couche.centre, relief.as_deref(), mode.is_3d);
    let (Ok(a), Ok(b)) = (camera.world_to_viewport(global, p), camera.world_to_viewport(global, p + global.right() * HEX_SIZE)) else { return };
    let r = a.distance(b);
    // (seulement un changement net : l'écran se refait quand l'échelle change de plus de 12 %)
    if taille.0 <= 0.0 || (r / taille.0 - 1.0).abs() > 0.12 {
        taille.0 = r;
    }
}

/// Calcule le cadrage demandé : les cases tiennent dans la zone libre (2D : échelle et centre ;
/// 3D : point visé et distance, ajustés en projetant).
#[allow(clippy::too_many_arguments)]
fn cadrer(
    mut demandes: MessageReader<CadrerVille>,
    zone: Res<ZoneLibre>,
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    relief: Option<Res<Relief>>,
    cam3d: Res<Cam3d>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera3d: Query<&Camera, (With<Camera3d>, Without<crate::seam::GhostCamera>)>,
    camera2d: Single<&Transform, (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
    mut vol: ResMut<Vol>,
) {
    let Some(CadrerVille(cases)) = demandes.read().last() else { return };
    let Some(&premiere) = cases.first() else { return };
    let wrap = map.0.wrap();
    let points: Vec<Vec2> = cases.iter().map(|&h| Vec2::from(wrap.nearest(premiere, h).to_world())).collect();
    let (min, max) = points.iter().fold((Vec2::MAX, Vec2::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
    let (min, max) = (min - Vec2::splat(HEX_SIZE), max + Vec2::splat(HEX_SIZE));
    let centre = (min + max) / 2.0;
    let z = zone.0;
    let ecran = Vec2::new(window.width(), window.height());
    let monde = wrap.world_width();
    let pres = |x: f32, de: f32| if monde > 0.0 { x + ((de - x) / monde).round() * monde } else { x };
    if !mode.is_3d {
        let echelle = ((max.x - min.x) / (z.width() * 0.88)).max((max.y - min.y) / (z.height() * 0.88)).clamp(0.5, 8.0);
        // Le centre des cases tombe au centre de la zone libre (y du monde vers le haut)
        let d = (z.center() - ecran / 2.0) * echelle;
        let c = Vec2::new(centre.x - d.x, centre.y + d.y);
        vol.0 = Some(But::Plan { centre: Vec2::new(pres(c.x, camera2d.translation.x), c.y), echelle, temps: 0.0 });
        return;
    }
    let Some(relief) = relief else { return };
    let sol = |p: Vec2| relief.on_ground(p.x, p.y, 0.0);
    let hauteur = sol(centre).y;
    let coins: Vec<Vec3> = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)].into_iter().map(|p| Vec3::new(p.x, hauteur, -p.y)).collect();
    let mut cam = Cam3d { target: Vec3::new(centre.x, hauteur, -centre.y), distance: cam3d.distance, yaw: cam3d.yaw };
    // Première distance : comme le vol vers un lieu, rapportée à la part de l'écran que laisse la zone
    let part = (z.width() / ecran.x).min(z.height() / ecran.y).max(0.2);
    cam.distance = (((max.y - min.y) / 2.0).max((max.x - min.x) / 2.0 * ecran.y / ecran.x) * 2.4 / part + 150.0).clamp(150.0, 2_800.0);
    if let Ok(camera) = camera3d.single() {
        // Ajustements : déplacer le point visé pour que le centre tombe au centre de la zone, et
        // la distance pour que les cases y tiennent (perspective : on mesure en projetant)
        for _ in 0..4 {
            let t = GlobalTransform::from(crate::view3d::camera_transform_de(&cam));
            let Ok(rayon) = camera.viewport_to_world(&t, z.center()) else { break };
            let Some(k) = rayon.intersect_plane(Vec3::Y * hauteur, InfinitePlane3d::new(Vec3::Y)) else { break };
            cam.target += Vec3::new(centre.x, hauteur, -centre.y) - rayon.get_point(k);
            let t = GlobalTransform::from(crate::view3d::camera_transform_de(&cam));
            let proj: Option<Vec<Vec2>> = coins.iter().map(|p| camera.world_to_viewport(&t, *p).ok()).collect();
            let Some(proj) = proj else { break };
            let (a, b) = proj.iter().fold((Vec2::MAX, Vec2::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
            let besoin = ((b.x - a.x) / (z.width() * 0.85)).max((b.y - a.y) / (z.height() * 0.85));
            if besoin > 0.0 {
                cam.distance = (cam.distance * besoin.powf(0.8)).clamp(330.0, 2_800.0);
            }
        }
    }
    vol.0 = Some(But::Relief { cible: Vec3::new(pres(cam.target.x, cam3d.target.x), cam.target.y, cam.target.z), distance: cam.distance, temps: 0.0 });
}

/// Vol en douceur vers le cadrage ; une action du joueur sur la caméra l'interrompt.
#[allow(clippy::too_many_arguments)]
fn voler(
    mut vol: ResMut<Vol>,
    mode: Res<ViewMode>,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    over_ui: Res<crate::input::PointerOverUi>,
    mut cam3d: ResMut<Cam3d>,
    camera2d: Single<(&mut Transform, &mut Projection), (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
) {
    let Some(but) = vol.0.as_mut() else { return };
    let bouge = [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD, KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight];
    if keys.any_pressed(bouge) || mouse.pressed(MouseButton::Middle) || (scroll.delta.y != 0.0 && !over_ui.0) {
        vol.0 = None;
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let k = 1.0 - (-dt * 8.0).exp();
    let arrive = match (but, mode.is_3d) {
        (But::Plan { centre, echelle, temps }, false) => {
            *temps += dt;
            let (mut t, mut p) = camera2d.into_inner();
            let Projection::Orthographic(o) = p.as_mut() else { return };
            let at = t.translation.truncate().lerp(*centre, k);
            let s = (o.scale.ln() + (echelle.ln() - o.scale.ln()) * k).exp();
            t.translation.x = at.x;
            t.translation.y = at.y;
            o.scale = s;
            *temps > 2.0 || (at.distance(*centre) < 0.5 && (s / *echelle - 1.0).abs() < 0.005)
        }
        (But::Relief { cible, distance, temps }, true) => {
            *temps += dt;
            let c = cam3d.target.lerp(*cible, k);
            let d = cam3d.distance + (*distance - cam3d.distance) * k;
            cam3d.target = c;
            cam3d.distance = d;
            *temps > 2.0 || (c.distance(*cible) < 0.5 && (d - *distance).abs() < 0.5)
        }
        // (la vue a basculé pendant le vol : le cadrage est à refaire)
        _ => true,
    };
    if arrive {
        vol.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_reste_du_monde_est_assombri_mais_pas_la_ville() {
        let map = crate::map::WorldMap::filled(60, 60, crate::map::Terrain::Grassland);
        let centre = Hex::from_offset(30, 30);
        let claires: HashSet<Hex> = map.range(centre, 1).filter(|h| map.contains(*h)).collect();
        let couche = CoucheVille { ouverte: true, centre, teintes: vec![(centre, Color::WHITE)], claires: claires.clone(), contours: Vec::new(), fleches: Vec::new() };
        let cases = cases_teintees(&map, &couche);
        // la ville : sa teinte seulement, sans ombre
        for h in &claires {
            assert!(cases.iter().filter(|(c, k)| c == h && k[3] > 0.0 && k[0] == 0.0).count() == 0, "{h:?} assombrie");
        }
        // une voisine hors de la ville : ombre pleine ; loin : effacée
        let pres = cases.iter().find(|(h, _)| centre.distance(*h) == 3).expect("case assombrie");
        assert!((pres.1[3] - OMBRE).abs() < 1e-6);
        assert!(cases.iter().filter(|(h, _)| centre.distance(*h) == RAYON_SOMBRE).all(|(_, k)| k[3] == 0.0));
    }

    #[test]
    fn un_contour_en_pointille_est_fait_de_tirets() {
        let h = Hex::new(3, 2);
        let plein = Contour { hex: h, couleur: Color::WHITE, pointille: false, retrait: 0.0 };
        let tirets = Contour { pointille: true, ..plein };
        assert_eq!(traits(&plein, h, crate::hex::Wrap::NONE).len(), 6);
        assert_eq!(traits(&tirets, h, crate::hex::Wrap::NONE).len(), 30);
        // les traits pleins relient les sommets de la case
        let (x, y) = h.to_world();
        for (a, _) in traits(&plein, h, crate::hex::Wrap::NONE) {
            assert!((a.distance(Vec2::new(x, y)) - HEX_SIZE).abs() < 1e-3);
        }
    }
}
