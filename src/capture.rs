//! Mode capture, pour le développement : avec `CIVBUILDER_CAPTURE=<dossier>`, le jeu prépare une
//! scène (ville sur une colline, ville au départ, les douze types d'unités près d'une montagne,
//! trois unités en barque sur un fleuve, carte révélée), passe en 3D, vise plusieurs cibles à plusieurs distances (ville, unités,
//! camp, village, vue lointaine, carte des courants marins), enregistre chaque vue en PNG dans le dossier, puis se ferme.
//! Sert à vérifier le rendu sans jouer (`CIVBUILDER_SEED` fixe la graine).
//! Avec en plus `CIVBUILDER_CAPTURE_EXPLORE=1`, la carte n'est pas révélée : un éclaireur longe un
//! fleuve case par case (exploration progressive, relief 3D construit au fil des pas), puis des
//! vues sont prises le long de son trajet (bords des cours d'eau découverts en marchant).

use crate::game::{DebugMode, GameMap, GameState, HexPos, ShownFog, Territory, Unit, found_city, spawn_unit};
use crate::hex::Hex;
use crate::hostile::{Camp, Village};
use crate::map::Relief;
use crate::render::MapLayer;
use crate::units::UnitKind;
use crate::view3d::{Cam3d, ViewMode, stale_channels, tile_height, to_3d};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::path::PathBuf;

/// Images laissées au relief 3D pour se construire autour d'une nouvelle cible, puis à la capture
/// pour s'enregistrer.
const SETTLE_FRAMES: u32 = 150;
const SAVE_FRAMES: u32 = 20;
/// Images entre deux pas de l'éclaireur (mode exploration) et longueur maximale de son trajet.
const STEP_FRAMES: u32 = 6;
const WALK_LENGTH: usize = 30;

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        let Some(dir) = std::env::var_os("CIVBUILDER_CAPTURE").map(PathBuf::from) else { return };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            error!("Mode capture : dossier {} impossible à créer ({e})", dir.display());
            return;
        }
        info!("Mode capture : images dans {}", dir.display());
        let explore = std::env::var_os("CIVBUILDER_CAPTURE_EXPLORE").is_some();
        app.insert_resource(Capture { dir, frame: 0, shots: Vec::new(), next: 0, wait: 0, shooting: false, explore, walk: Vec::new(), walker: None })
            .add_systems(Update, run_capture);
    }
}

/// Une vue à enregistrer : nom du fichier, case visée, distance et orientation de la caméra,
/// carte affichée.
struct Shot {
    name: String,
    target: Hex,
    distance: f32,
    yaw: f32,
    layer: MapLayer,
}

#[derive(Resource)]
struct Capture {
    dir: PathBuf,
    frame: u32,
    shots: Vec<Shot>,
    /// Vues déjà placées, images à attendre, une vue placée attend sa capture.
    next: usize,
    wait: u32,
    shooting: bool,
    /// Mode exploration : trajet restant de l'éclaireur (de la fin vers le début) et éclaireur.
    explore: bool,
    walk: Vec<Hex>,
    walker: Option<Entity>,
}

/// Déroulé : mise en scène, puis pour chaque vue, caméra placée, attente, capture.
#[allow(clippy::too_many_arguments)]
fn run_capture(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    map: Res<GameMap>,
    mut mode: ResMut<ViewMode>,
    mut cam: ResMut<Cam3d>,
    mut layer: ResMut<MapLayer>,
    mut debug: ResMut<DebugMode>,
    mut state: ResMut<GameState>,
    mut territory: ResMut<Territory>,
    units: Query<(Entity, &Unit, &HexPos)>,
    camps: Query<&HexPos, With<Camp>>,
    villages: Query<&HexPos, With<Village>>,
    mut exit: MessageWriter<AppExit>,
    relief: Option<ResMut<crate::view3d::Relief>>,
    fog: Res<ShownFog>,
) {
    capture.frame += 1;
    let m = &map.0;
    match capture.frame {
        // Mode exploration : éclaireur au bord d'un fleuve, carte non révélée
        3 if capture.explore => {
            let Some((_, _, start)) = units.iter().next() else { return };
            let start = start.0;
            let passable = |h: Hex| m.contains(h) && m.get(h).is_some_and(|t| t.move_cost().is_some());
            let on_river = |h: Hex| m.is_fleuve(h) && passable(h);
            let mut by_distance: Vec<Hex> = m.hexes().collect();
            by_distance.sort_by_key(|&h| m.distance(h, start));
            // Le plus long trajet le long d'un fleuve parmi les fleuves proches du départ
            let walk_from = |first: Hex| {
                let mut path = vec![first];
                while path.len() < WALK_LENGTH {
                    let last = *path.last().unwrap();
                    let Some(next) = m.neighbors(last).into_iter().map(|n| m.canonical(n)).find(|&n| on_river(n) && !path.contains(&n)) else { break };
                    path.push(next);
                }
                path
            };
            let Some(path) = by_distance.iter().copied().filter(|&h| on_river(h)).take(40).map(walk_from).max_by_key(|p| p.len()) else {
                error!("Mode capture : aucun fleuve");
                exit.write(AppExit::Success);
                return;
            };
            info!("Mode capture (exploration) : éclaireur sur {} cases de fleuve depuis {:?}", path.len(), path[0]);
            capture.walker = Some(spawn_unit(&mut commands, UnitKind::Scout, path[0]));
            mode.is_3d = true;
            cam.target = to_3d(path[0], tile_height(m, path[0]));
            cam.distance = 260.0;
            let mut shots = Vec::new();
            for (k, &h) in path.iter().enumerate().skip(2).step_by(4) {
                shots.push(Shot { name: format!("exploration_{k:02}"), target: h, distance: 220.0, yaw: 0.4 * k as f32, layer: MapLayer::Normal });
            }
            if let Some(&h) = path.last() {
                shots.push(Shot { name: "exploration_ensemble".into(), target: path[path.len() / 2], distance: 700.0, yaw: 0.0, layer: MapLayer::Normal });
                shots.push(Shot { name: "exploration_fin".into(), target: h, distance: 220.0, yaw: 2.0, layer: MapLayer::Normal });
            }
            capture.shots = shots;
            capture.walk = path.into_iter().rev().collect();
            capture.walk.pop();
            capture.wait = SETTLE_FRAMES;
        }
        // Mise en scène (une fois les unités de départ créées)
        3 => {
            let Some((settler, _, start)) = units.iter().find(|(_, u, _)| u.0.kind == UnitKind::Settler) else { return };
            let start = start.0;
            let passable = |h: Hex| m.contains(h) && m.get(h).is_some_and(|t| t.move_cost().is_some()) && m.relief(h) != Relief::Peak;
            // Cases de la carte, de la plus proche du départ à la plus lointaine
            let mut by_distance: Vec<Hex> = m.hexes().collect();
            by_distance.sort_by_key(|&h| m.distance(h, start));
            let land_around = |h: Hex, radius: i32| m.range(h, radius).filter(|&n| passable(n)).count();
            let mut cities_at = Vec::new();
            if found_city(&mut commands, settler, start, m, &[], &mut state, &mut territory).is_ok() {
                cities_at.push(start);
            }
            // Une ville sur un relief accidenté (sinon une colline), entourée de terre
            let hill = [Relief::Rugged, Relief::Hills].into_iter().find_map(|relief| {
                by_distance.iter().copied().find(|&h| m.relief(h) == relief && land_around(h, 1) == 7 && crate::rules::can_found_city(m, h, &cities_at).is_ok())
            });
            if let Some(h) = hill {
                let s = spawn_unit(&mut commands, UnitKind::Settler, h);
                if found_city(&mut commands, s, h, m, &cities_at, &mut state, &mut territory).is_ok() {
                    cities_at.push(h);
                }
            }
            // Les douze types d'unités autour d'une montagne
            let taken: Vec<Hex> = units.iter().map(|(_, _, p)| p.0).chain(cities_at.iter().copied()).collect();
            let mountain = by_distance.iter().copied().find(|&h| m.relief(h) == Relief::Mountain && passable(h) && land_around(h, 2) >= 16).unwrap_or(start);
            let spots: Vec<Hex> = m.range(mountain, 2).filter(|&h| passable(h) && !taken.contains(&m.canonical(h))).collect();
            for (kind, &h) in UnitKind::ALL.iter().zip(&spots) {
                spawn_unit(&mut commands, *kind, m.canonical(h));
            }
            // Trois unités en barque sur un fleuve (cases de fleuve voisines)
            let used: Vec<Hex> = taken.iter().copied().chain(spots.iter().map(|&h| m.canonical(h))).collect();
            let on_river = |h: Hex| m.is_fleuve(h) && passable(h) && !used.contains(&h);
            let river = by_distance.iter().copied().find(|&h| on_river(h) && m.neighbors(h).iter().filter(|&&n| on_river(m.canonical(n))).count() >= 2);
            if let Some(h) = river {
                let mut boats = vec![h];
                boats.extend(m.neighbors(h).into_iter().map(|n| m.canonical(n)).filter(|&n| on_river(n)).take(2));
                for (kind, &b) in [UnitKind::Settler, UnitKind::HeavyCavalry, UnitKind::Spearman].iter().zip(&boats) {
                    spawn_unit(&mut commands, *kind, b);
                }
            }
            debug.0 = true;
            mode.is_3d = true;
            // Vues
            let mut shots = Vec::new();
            let mut add_on = |name: &str, target: Hex, distance: f32, yaw: f32, layer: MapLayer| shots.push(Shot { name: name.into(), target, distance, yaw, layer });
            let mut add = |name: &str, target: Hex, distance: f32, yaw: f32| add_on(name, target, distance, yaw, MapLayer::Normal);
            if let Some(h) = hill {
                add("ville_colline", h, 260.0, 0.0);
                add("ville_colline_cote", h, 260.0, 2.1);
            }
            add("ville_depart", start, 260.0, 0.6);
            add("unites_montagne", mountain, 230.0, 0.0);
            add("unites_montagne_cote", mountain, 230.0, 2.6);
            add("unites_moyen", mountain, 900.0, 0.0);
            // (juste après l'apparition des icônes : figurines encore grandes, sous l'icône)
            add("unites_seuil_icones", mountain, 1_150.0, 0.0);
            add("unites_loin", mountain, 1_500.0, 0.0);
            add("ville_loin", start, 1_150.0, 0.6);
            if let Some(h) = river {
                add("fleuve", h, 200.0, 0.3);
            }
            let nearest = |list: Vec<Hex>| list.into_iter().min_by_key(|&h| m.distance(h, start));
            if let Some(h) = nearest(camps.iter().map(|p| p.0).collect()) {
                add("camp", h, 220.0, 0.0);
            }
            if let Some(h) = nearest(villages.iter().map(|p| p.0).collect()) {
                add("village", h, 220.0, 0.0);
            }
            // Carte des courants marins : sur le courant le plus rapide près du départ
            let fastest = by_distance.iter().copied().take(4_000).filter(|&h| m.current(h).is_some()).max_by(|&a, &b| {
                let speed = |h: Hex| m.current(h).map_or(0.0, |(u, v)| u.hypot(v));
                speed(a).total_cmp(&speed(b))
            });
            if let Some(h) = fastest {
                add_on("courants", h, 700.0, 0.0, MapLayer::Currents);
                add_on("courants_loin", h, 2_200.0, 0.0, MapLayer::Currents);
            }
            info!("Mode capture : {} vues, {} villes fondées", shots.len(), cities_at.len());
            capture.shots = shots;
            capture.wait = SETTLE_FRAMES;
        }
        f if f > 3 => {
            if capture.wait > 0 {
                capture.wait -= 1;
                return;
            }
            // Mode exploration : un pas de l'éclaireur, la caméra le suit
            if let Some(walker) = capture.walker
                && let Some(h) = capture.walk.pop()
            {
                commands.entity(walker).insert(HexPos(h));
                cam.target = to_3d(h, tile_height(m, h));
                capture.wait = if capture.walk.is_empty() { SETTLE_FRAMES } else { STEP_FRAMES };
                return;
            }
            // Vue en place depuis assez longtemps : capture (la caméra ne bouge pas avant qu'elle
            // soit enregistrée)
            if capture.shooting {
                capture.shooting = false;
                let n = capture.next;
                let path = capture.dir.join(format!("{n:02}_{}.png", capture.shots[n - 1].name));
                commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
                capture.wait = SAVE_FRAMES;
                return;
            }
            let Some(shot) = capture.shots.get(capture.next) else {
                // Lits creusés restés en retard sur le relief (exploration progressive)
                if let Some(mut relief) = relief {
                    let (stale, worst) = stale_channels(&mut relief, &map, &fog);
                    info!("Mode capture : {} cases aux lits périmés (écart d'eau max {worst:.2}) {:?}", stale.len(), &stale[..stale.len().min(8)]);
                }
                info!("Mode capture : terminé");
                exit.write(AppExit::Success);
                return;
            };
            cam.target = to_3d(shot.target, tile_height(m, shot.target));
            cam.distance = shot.distance;
            cam.yaw = shot.yaw;
            if *layer != shot.layer {
                *layer = shot.layer;
            }
            capture.next += 1;
            capture.shooting = true;
            capture.wait = SETTLE_FRAMES;
        }
        _ => {}
    }
}
