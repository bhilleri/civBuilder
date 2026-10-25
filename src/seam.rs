//! Planète qui fait le tour d'est en ouest : défilement sans fin des vues 2D et 3D.
//!
//! La carte n'est dessinée qu'une fois, entre x = 0 et x = un tour. Quand la vue déborde d'un
//! bord, une caméra « fantôme » décalée d'un tour dessine l'autre bord à sa place, sous la
//! caméra principale (qui ne repeint alors pas le fond) : le raccord est invisible. La caméra
//! principale reste dans le tour : passé un bord, elle est ramenée d'un tour en arrière, ce qui
//! ne change rien à l'image.

use crate::game::GameMap;
use crate::view3d::Cam3d;
use bevy::camera::ClearColorConfig;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Caméra fantôme : copie de `main` décalée de `side` tours (−1 ou +1).
#[derive(Component)]
pub struct GhostCamera {
    main: Entity,
    side: f32,
}

pub struct SeamPlugin;

impl Plugin for SeamPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_ghosts).add_systems(
            PostUpdate,
            // (avant le calcul des projections : un fantôme qui aurait une image de retard sur le zoom
            // ferait sauter le raccord)
            (wrap_cameras, follow_ghosts)
                .chain()
                .before(bevy::transform::TransformSystems::Propagate)
                .before(bevy::camera::CameraUpdateSystems),
        );
    }
}

/// Une paire de fantômes (un tour à l'ouest, un tour à l'est) pour chaque caméra du monde.
fn spawn_ghosts(
    mut commands: Commands,
    map: Res<GameMap>,
    mains: Query<(Entity, &Camera, Has<Camera3d>, Option<&RenderLayers>), (Without<GhostCamera>, Or<(With<Camera2d>, With<Camera3d>)>)>,
    ghosts: Query<&GhostCamera>,
) {
    if !map.0.wrap().wraps() {
        return;
    }
    for (main, camera, is_3d, layers) in &mains {
        if ghosts.iter().any(|g| g.main == main) {
            continue;
        }
        for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            // Sous la caméra principale (ordre plus petit), jamais celle de l'interface
            let ghost = Camera { is_active: false, order: camera.order - 10 - k as isize, ..default() };
            let mut e = commands.spawn((GhostCamera { main, side }, ghost, Transform::default()));
            if is_3d {
                e.insert(Camera3d::default());
            } else {
                e.insert(Camera2d);
            }
            if let Some(layers) = layers {
                e.insert(layers.clone());
            }
        }
    }
}

/// Ramène les caméras principales dans le tour (même image, un tour plus loin).
fn wrap_cameras(
    map: Res<GameMap>,
    mut cam3d: ResMut<Cam3d>,
    mut cams2d: Query<&mut Transform, (With<Camera2d>, Without<GhostCamera>, Without<Camera3d>)>,
    mut cams3d: Query<&mut Transform, (With<Camera3d>, Without<GhostCamera>, Without<Camera2d>)>,
) {
    let world = map.0.wrap().world_width();
    if world <= 0.0 {
        return;
    }
    let turns = |x: f32| (x / world).floor();
    for mut t in &mut cams2d {
        let k = turns(t.translation.x);
        if k != 0.0 {
            t.translation.x -= k * world;
        }
    }
    let k = turns(cam3d.target.x);
    if k != 0.0 {
        // (sans marquer `Cam3d` changée : la caméra est déplacée ici, du même tour)
        cam3d.bypass_change_detection().target.x -= k * world;
        for mut t in &mut cams3d {
            t.translation.x -= k * world;
        }
    }
}

/// Les fantômes suivent leur caméra, décalés d'un tour, et ne s'allument que si la vue déborde
/// de leur côté. Seule la première caméra dessinée (la plus petite) repeint le fond.
fn follow_ghosts(
    map: Res<GameMap>,
    cam3d: Res<Cam3d>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut mains: Query<(Entity, &mut Camera, &Transform, &Projection), Without<GhostCamera>>,
    mut ghosts: Query<(&GhostCamera, &mut Camera, &mut Transform, &mut Projection)>,
) {
    let world = map.0.wrap().world_width();
    if world <= 0.0 {
        return;
    }
    let half_width = window.map_or(800.0, |w| w.width() / 2.0);
    // Fantômes allumés de chaque caméra principale : (ordre, entité du fantôme)
    let mut lit: Vec<(Entity, isize)> = Vec::new();
    for (ghost, mut camera, mut transform, mut projection) in &mut ghosts {
        let Ok((_, main_camera, main_transform, main_projection)) = mains.get(ghost.main) else { continue };
        // Étendue visible au sol de part et d'autre du centre de la vue (large en 3D : la vue
        // inclinée voit loin vers le haut de l'écran, et elle peut tourner)
        let (center, reach) = match main_projection {
            Projection::Orthographic(o) => (main_transform.translation.x, half_width * o.scale + 64.0),
            _ => (cam3d.target.x, cam3d.distance * 3.0 + 400.0),
        };
        let active = main_camera.is_active && if ghost.side < 0.0 { center + reach > world } else { center - reach < 0.0 };
        if camera.is_active != active {
            camera.is_active = active;
        }
        // Le fantôme « ouest » (side −1) montre l'ouest de la carte au-delà du bord est : placé un
        // tour plus à l'ouest que la caméra principale, il voit l'ouest de la carte là où elle ne
        // voit plus rien ; et inversement
        let mut t = *main_transform;
        t.translation.x += ghost.side * world;
        if *transform != t {
            *transform = t;
        }
        *projection = main_projection.clone();
        if active {
            lit.push((ghost.main, camera.order));
        }
    }
    let first_of = |main: Entity| lit.iter().filter(|(m, _)| *m == main).map(|(_, o)| *o).min();
    for (ghost, mut camera, _, _) in &mut ghosts {
        let clears = camera.is_active && first_of(ghost.main) == Some(camera.order);
        set_clear(&mut camera, clears);
    }
    for (main, mut camera, _, _) in &mut mains {
        let clears = first_of(main).is_none();
        set_clear(&mut camera, clears);
    }
}

fn set_clear(camera: &mut Camera, clears: bool) {
    let is_none = matches!(camera.clear_color, ClearColorConfig::None);
    if clears == is_none {
        camera.clear_color = if clears { ClearColorConfig::Default } else { ClearColorConfig::None };
    }
}
