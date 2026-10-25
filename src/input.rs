//! Caméra et commandes du joueur (souris + clavier).

use crate::game::{
    City, EndTurnRequested, Fog, GameLog, GameMap, GameState, HexPos, Selection, Territory, Unit, explore_unit, found_city,
    move_unit, movement_blocked, reserved_by_others,
};
use crate::hex::Hex;
use crate::explore::ExploreWorld;
use crate::battle::{self, Side};
use crate::combat::{self, Preview};
use crate::hostile::{Blocked, Danger, EnemyZoc, Hostile, KnownVillages, Occupancy};
use crate::map::{KnownMap, MoveCosts};
use crate::pathfinding::find_path_turns;
use crate::rules::{self, UnitKind};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Case actuellement sous le curseur (si sur la carte et hors interface).
#[derive(Resource, Default)]
pub struct HoveredHex(pub Option<Hex>);

/// Chemin prévisualisé vers la case survolée pour l'unité sélectionnée,
/// avec pour chaque case le tour d'arrivée (1 = ce tour-ci).
#[derive(Resource, Default)]
pub struct HoverPath {
    pub path: Vec<Hex>,
    pub turns: Vec<u32>,
}

/// Ordre de déplacement en cours de l'unité sélectionnée, avec le tour d'arrivée sur chaque case.
#[derive(Resource, Default)]
pub struct OrderPath {
    pub path: Vec<Hex>,
    pub turns: Vec<u32>,
}

/// Prévision d'attaque quand le curseur survole un ennemi visible avec une unité sélectionnée.
#[derive(Resource, Default)]
pub struct AttackPreview(pub Option<AttackInfo>);

#[derive(Clone, Debug, PartialEq)]
pub struct AttackInfo {
    pub attacker: crate::units::UnitKind,
    pub defender: crate::units::UnitKind,
    pub preview: Preview,
    /// L'attaque est possible ce tour-ci (PM suffisants, case d'approche libre).
    pub reachable: bool,
}

/// Vrai quand le curseur est au-dessus d'un élément d'interface.
#[derive(Resource, Default)]
pub struct PointerOverUi(pub bool);

/// Systèmes de commandes du joueur.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputSet;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredHex>()
            .init_resource::<HoverPath>()
            .init_resource::<OrderPath>()
            .init_resource::<AttackPreview>()
            .init_resource::<PointerOverUi>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                (
                    camera_controls,
                    track_pointer_over_ui,
                    update_hovered_hex,
                    update_hover_path,
                    // Commandes de jeu suspendues pendant la lecture du wiki ou le menu Échap
                    mouse_commands.run_if(no_overlay),
                    keyboard_commands.run_if(no_overlay),
                )
                    .chain()
                    .in_set(InputSet)
                    .before(crate::game::EndTurnSet),
            );
    }
}

fn no_overlay(wiki: Res<crate::wiki::WikiState>, menu: Res<crate::menu::GameMenu>) -> bool {
    !wiki.open && !menu.open
}

fn spawn_camera(mut commands: Commands, map: Res<GameMap>) {
    let start = map.0.find_start().unwrap_or_default();
    let (x, y) = start.to_world();
    // L'interface s'affiche sur la caméra 2D au démarrage (la caméra 3D, inactive, a un ordre plus élevé)
    commands.spawn((Camera2d, IsDefaultUiCamera, Transform::from_xyz(x, y, 100.0)));
}

fn camera_controls(
    mode: Res<crate::view3d::ViewMode>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    over_ui: Res<PointerOverUi>,
    time: Res<Time>,
    camera: Single<(&mut Transform, &mut Projection), (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
) {
    if mode.is_3d {
        return;
    }
    let (mut transform, mut projection) = camera.into_inner();
    let Projection::Orthographic(ortho) = projection.as_mut() else { return };

    // Zoom à la molette
    if scroll.delta.y != 0.0 && !over_ui.0 {
        let step = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * 0.1,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.002,
        };
        ortho.scale = (ortho.scale * (1.0 - step)).clamp(0.3, 8.0);
    }

    // Déplacement clavier (ZQSD / WASD / flèches)
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.x += 1.0;
    }
    let speed = 700.0 * ortho.scale;
    transform.translation += (dir.normalize_or_zero() * speed * time.delta_secs()).extend(0.0);

    // Glisser avec le bouton du milieu
    if mouse.pressed(MouseButton::Middle) {
        transform.translation.x -= motion.delta.x * ortho.scale;
        transform.translation.y += motion.delta.y * ortho.scale;
    }
}

fn track_pointer_over_ui(mut over: ResMut<PointerOverUi>, nodes: Query<&Interaction>) {
    let value = nodes.iter().any(|i| *i != Interaction::None);
    if over.0 != value {
        over.0 = value;
    }
}

fn update_hovered_hex(
    mut hovered: ResMut<HoveredHex>,
    window: Single<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    map: Res<GameMap>,
    relief: Option<Res<crate::view3d::Relief>>,
    over_ui: Res<PointerOverUi>,
) {
    // Caméra active : 2D (projection directe) ou 3D (rayon intersecté avec le relief)
    let Some((camera, cam_transform, is_3d)) = cameras.iter().find(|(c, ..)| c.is_active) else { return };
    let hex = window.cursor_position().filter(|_| !over_ui.0).and_then(|p| {
        if is_3d {
            let ray = camera.viewport_to_world(cam_transform, p).ok()?;
            crate::view3d::hovered_hex_3d(ray, relief.as_deref()?, &map)
        } else {
            let w = camera.viewport_to_world_2d(cam_transform, p).ok()?;
            Some(map.0.canonical(Hex::from_world(w.x, w.y))).filter(|&h| map.0.contains(h))
        }
    });
    if hovered.0 != hex {
        hovered.0 = hex;
    }
}

/// Met à jour l'aperçu sous le curseur et l'ordre en cours de l'unité sélectionnée.
/// Les deux sont calculés avec la carte connue du joueur (brouillard = prairie).
fn update_hover_path(
    hovered: Res<HoveredHex>,
    selection: Res<Selection>,
    map: Res<GameMap>,
    fog: Res<Fog>,
    blocked: Res<Blocked>,
    occupancy: Res<Occupancy>,
    enemy_zoc: Res<EnemyZoc>,
    units: Query<(Ref<Unit>, Ref<HexPos>), Without<Hostile>>,
    hostiles: Query<(&Unit, &HexPos), With<Hostile>>,
    mut hover: ResMut<HoverPath>,
    mut order: ResMut<OrderPath>,
    mut attack: ResMut<AttackPreview>,
) {
    let selected = selection.0.and_then(|e| units.get(e).ok());
    // Recalcul si le curseur, la sélection, l'unité sélectionnée (position, PM, ordre) ou la carte connue ont changé
    let unit_changed = selected.as_ref().is_some_and(|(u, p)| u.is_changed() || p.is_changed());
    if !hovered.is_changed()
        && !selection.is_changed()
        && !unit_changed
        && !fog.is_changed()
        && !blocked.is_changed()
        && !occupancy.is_changed()
        && !enemy_zoc.is_changed()
    {
        return;
    }
    let blocked_now = movement_blocked(&blocked.0, &occupancy.own);
    let known = KnownMap::new(&map.0, &fog.explored).blocking(&blocked_now);

    // Survol d'un ennemi visible : trajet d'approche et prévision du combat
    let foe = hovered.0.filter(|h| blocked.0.contains(h)).and_then(|h| hostiles.iter().find(|(_, p)| p.0 == h));
    let mut attack_info = None;
    let (hover_path, hover_turns) = match (hovered.0, &selected, foe) {
        (Some(target), Some((unit, pos)), Some((foe_unit, _))) if unit.0.kind.can_attack() => {
            let approach = rules::attack_approach(&known, &map.0, &unit.0, pos.0, target, &enemy_zoc.0);
            // Prévision depuis la case d'où l'attaque partira
            let from = approach.as_ref().and_then(|p| p.last().copied()).unwrap_or(pos.0);
            let will_move = approach.as_ref().is_some_and(|p| !p.is_empty());
            let allies = occupancy.own.iter().copied().filter(|h| *h != pos.0);
            let e = battle::engagement(&map.0, (&unit.0, from), (&foe_unit.0, target), allies, will_move);
            attack_info = Some(AttackInfo {
                attacker: unit.0.kind,
                defender: foe_unit.0.kind,
                preview: combat::preview(&e),
                reachable: approach.is_some(),
            });
            let path = approach.unwrap_or_default();
            let turns = vec![1; path.len()];
            (path, turns)
        }
        (Some(target), Some((unit, pos)), None) => match find_path_turns(&known, pos.0, target, unit.0.moves_left, unit.0.kind.max_moves()) {
            Some(path) => {
                let turns = rules::path_turns(&known, pos.0, &path, unit.0.moves_left, unit.0.kind.max_moves());
                (path, turns)
            }
            None => (Vec::new(), Vec::new()),
        },
        _ => (Vec::new(), Vec::new()),
    };
    if hover.path != hover_path || hover.turns != hover_turns {
        *hover = HoverPath { path: hover_path, turns: hover_turns };
    }
    if attack.0 != attack_info {
        attack.0 = attack_info;
    }

    let (order_path, order_turns) = match &selected {
        Some((unit, pos)) if !unit.0.path.is_empty() => {
            let turns = rules::path_turns(&known, pos.0, &unit.0.path, unit.0.moves_left, unit.0.kind.max_moves());
            (unit.0.path.clone(), turns)
        }
        _ => (Vec::new(), Vec::new()),
    };
    if order.path != order_path || order.turns != order_turns {
        *order = OrderPath { path: order_path, turns: order_turns };
    }
}

fn mouse_commands(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredHex>,
    map: Res<GameMap>,
    mut fog: ResMut<Fog>,
    blocked: Res<Blocked>,
    occupancy: Res<Occupancy>,
    enemy_zoc: Res<EnemyZoc>,
    mut selection: ResMut<Selection>,
    mut log: ResMut<GameLog>,
    mut units: Query<(Entity, &mut Unit, &mut HexPos), (Without<City>, Without<Hostile>)>,
    mut hostiles: Query<(Entity, &mut Unit, &HexPos), With<Hostile>>,
    cities: Query<(Entity, &HexPos), With<City>>,
) {
    let Some(target) = hovered.0 else { return };

    if mouse.just_pressed(MouseButton::Left) {
        // Clic répété sur une case : on passe de l'unité à la ville (et inversement).
        let mut candidates: Vec<Entity> = units.iter().filter(|(_, _, p)| p.0 == target).map(|(e, _, _)| e).collect();
        candidates.extend(cities.iter().filter(|(_, p)| p.0 == target).map(|(e, _)| e));
        selection.0 = match selection.0.and_then(|s| candidates.iter().position(|&c| c == s)) {
            Some(i) => candidates.get((i + 1) % candidates.len()).copied(),
            None => candidates.first().copied(),
        };
    }

    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(selected) = selection.0 else { return };
    let Ok((_, unit, pos)) = units.get(selected) else { return };
    let (kind, from, moves) = (unit.0.kind, pos.0, unit.0.moves_left);
    // État de l'unité avant l'ordre (l'accès mutable est repris plus bas)
    let snapshot = unit.0.clone();
    let blocked_now = movement_blocked(&blocked.0, &occupancy.own);
    let mut occupied = occupancy.all.clone();

    // 1) Ennemi visible : approche éventuelle puis attaque (mêlée) ou tir
    if blocked.0.contains(&target) {
        let Some(foe) = hostiles.iter().find(|(_, _, p)| p.0 == target).map(|(e, _, _)| e) else { return };
        if !kind.can_attack() {
            log.push(format!("{} : cette unité ne peut pas attaquer.", kind.name()));
            return;
        }
        let known = KnownMap::new(&map.0, &fog.explored).blocking(&blocked_now);
        let Some(approach) = rules::attack_approach(&known, &map.0, &snapshot, from, target, &enemy_zoc.0) else {
            let reason = if snapshot.has_attacked {
                "a déjà attaqué ce tour-ci"
            } else if kind == UnitKind::ExpertArcher && snapshot.moved_this_turn {
                "ne peut pas tirer après s'être déplacé"
            } else {
                "cible hors de portée ce tour-ci"
            };
            log.push(format!("{} : {reason}.", kind.name()));
            return;
        };
        let (_, mut unit, mut pos) = units.get_mut(selected).unwrap();
        if !approach.is_empty() {
            unit.0.set_order(pos.0, approach, &known);
            move_unit(&mut unit.0, &mut pos, &map.0, &mut fog, &mut log, &blocked_now, &mut occupied, &enemy_zoc.0);
            if !rules::in_attack_position(&map.0, kind, pos.0, target) || unit.0.moves_left == 0 {
                log.push(format!("{} : approche interrompue.", kind.name()));
                return;
            }
        }
        let here = pos.0;
        let allies: Vec<Hex> = occupancy.own.iter().copied().filter(|h| *h != from).collect();
        let Ok((_, mut foe_unit, _)) = hostiles.get_mut(foe) else { return };
        let e = battle::engagement(&map.0, (&unit.0, here), (&foe_unit.0, target), allies, false);
        let result = battle::fight(
            &mut commands,
            &mut log,
            Side { entity: selected, unit: &mut unit.0, hostile: false },
            Side { entity: foe, unit: &mut foe_unit.0, hostile: true },
            &e,
        );
        if result.attacker_died {
            selection.0 = None;
        } else if result.attacker_advances() {
            // Le vainqueur d'un corps à corps avance sur la case libérée
            pos.0 = target;
            unit.0.last_steps = vec![target];
        }
        return;
    }

    if snapshot.zoc_stopped {
        log.push(format!("{} : arrêté(e) par une zone de contrôle ennemie jusqu'au prochain tour.", kind.name()));
        return;
    }

    // 2) Une de nos unités : échange de place si elle est adjacente
    if let Some(other) = units.iter().find(|(e, _, p)| *e != selected && p.0 == target).map(|(e, _, _)| e) {
        // Échange possible seulement si chacune peut aller sur la case de l'autre (pas de falaise)
        let cost = map.0.step_cost(from, target);
        if map.0.distance(from, target) != 1 || moves == 0 || cost.is_none() || map.0.step_cost(target, from).is_none() {
            log.push("Case occupée par une de vos unités (échange possible avec une unité adjacente).");
            return;
        }
        let [(_, mut a, mut pa), (_, _, mut pb)] = units.get_many_mut([selected, other]).unwrap();
        pa.0 = target;
        pb.0 = from;
        a.0.clear_order();
        a.0.moves_left = a.0.moves_left.saturating_sub(cost.unwrap_or(1));
        a.0.moved_this_turn = true;
        a.0.last_steps = vec![target];
        return;
    }

    // 3) Déplacement (en contournant nos autres unités)
    let (_, mut unit, mut pos) = units.get_mut(selected).unwrap();
    let known = KnownMap::new(&map.0, &fog.explored).blocking(&blocked_now);
    match find_path_turns(&known, from, target, unit.0.moves_left, unit.0.kind.max_moves()) {
        Some(path) => {
            unit.0.set_order(pos.0, path, &known);
            move_unit(&mut unit.0, &mut pos, &map.0, &mut fog, &mut log, &blocked_now, &mut occupied, &enemy_zoc.0);
        }
        None => log.push("Destination inaccessible."),
    }
}

fn keyboard_commands(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    map: Res<GameMap>,
    mut state: ResMut<GameState>,
    mut territory: ResMut<Territory>,
    mut selection: ResMut<Selection>,
    mut log: ResMut<GameLog>,
    mut end_turn: ResMut<EndTurnRequested>,
    mut fog: ResMut<Fog>,
    knowledge: (Res<Blocked>, Res<Occupancy>, Res<Danger>, Res<KnownVillages>),
    mut units: Query<(Entity, &mut Unit, &mut HexPos), (Without<City>, Without<Hostile>)>,
    mut cities: Query<(&mut City, &HexPos)>,
    camera: Single<&mut Transform, (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
    mut cam3d: ResMut<crate::view3d::Cam3d>,
) {
    // Ce que le joueur sait de la carte (regroupé : un système Bevy accepte 16 paramètres au plus)
    let (blocked, occupancy, danger, villages) = knowledge;
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        end_turn.0 = true;
    }

    // Tab : unité suivante en attente d'ordres (et centrage de la caméra)
    if keys.just_pressed(KeyCode::Tab) {
        let mut waiting: Vec<(Entity, Hex)> =
            units.iter().filter(|(_, u, _)| u.0.needs_orders()).map(|(e, _, p)| (e, p.0)).collect();
        waiting.sort_by_key(|(e, _)| *e);
        let next = match selection.0.and_then(|s| waiting.iter().position(|(e, _)| *e == s)) {
            Some(i) => waiting.get((i + 1) % waiting.len()),
            None => waiting.first(),
        };
        if let Some(&(e, hex)) = next {
            selection.0 = Some(e);
            let (x, y) = hex.to_world();
            let mut cam = camera.into_inner();
            cam.translation.x = x;
            cam.translation.y = y;
            cam3d.target = crate::view3d::to_3d(hex, 0.0);
        } else {
            log.push("Aucune unité n'attend d'ordres. Entrée pour finir le tour.");
        }
    }

    let Some(selected) = selection.0 else { return };

    // --- Commandes d'unité
    let reserved = reserved_by_others(&map.0, units.iter().map(|(e, u, _)| (e, &u.0)), selected);
    if let Ok((entity, mut unit, mut pos)) = units.get_mut(selected) {
        if keys.just_pressed(KeyCode::Space) {
            unit.0.sleeping = true;
            unit.0.auto_explore = false;
            unit.0.clear_order();
        }
        if keys.just_pressed(KeyCode::KeyE) {
            let name = unit.0.kind.name();
            if !unit.0.kind.can_auto_explore() {
                log.push(format!("{name} : exploration automatique réservée aux unités militaires et éclaireurs."));
            } else if unit.0.auto_explore {
                unit.0.auto_explore = false;
                unit.0.clear_order();
                log.push(format!("{name} : exploration automatique arrêtée."));
            } else {
                unit.0.clear_order();
                unit.0.sleeping = false;
                unit.0.auto_explore = true;
                log.push(format!("{name} : exploration automatique activée."));
                let blocked_now = movement_blocked(&blocked.0, &occupancy.own);
                let mut occupied = occupancy.all.clone();
                let threats: std::collections::HashSet<Hex> = occupancy.all.difference(&occupancy.own).copied().collect();
                let world = ExploreWorld::new(&map.0)
                    .reserved(&reserved)
                    .blocked(&blocked_now)
                    .villages(&villages.0)
                    .danger(&danger.0)
                .threats(&threats);
                explore_unit(&mut unit.0, &mut pos, &world, &mut occupied, &mut fog, &mut log);
            }
        }
        if keys.just_pressed(KeyCode::KeyB) {
            if unit.0.kind != UnitKind::Settler {
                log.push("Seul un colon peut fonder une ville.");
            } else {
                let at = pos.0;
                let existing: Vec<Hex> = cities.iter().map(|(_, p)| p.0).collect();
                match found_city(&mut commands, entity, at, &map.0, &existing, &mut state, &mut territory) {
                    Ok(city) => {
                        log.push(format!("Nouvelle ville fondée (tour {}).", state.turn));
                        selection.0 = Some(city);
                    }
                    Err(reason) => log.push(format!("Impossible de fonder ici : {reason}.")),
                }
            }
        }
        return;
    }

    // --- Commandes de ville : choix de la production
    if let Ok((mut city, _)) = cities.get_mut(selected) {
        let choices = [
            (KeyCode::Digit1, KeyCode::Numpad1),
            (KeyCode::Digit2, KeyCode::Numpad2),
            (KeyCode::Digit3, KeyCode::Numpad3),
            (KeyCode::Digit4, KeyCode::Numpad4),
            (KeyCode::Digit5, KeyCode::Numpad5),
            (KeyCode::Digit6, KeyCode::Numpad6),
            (KeyCode::Digit7, KeyCode::Numpad7),
            (KeyCode::Digit8, KeyCode::Numpad8),
            (KeyCode::Digit9, KeyCode::Numpad9),
        ];
        for (i, (k1, k2)) in choices.iter().enumerate() {
            if keys.just_pressed(*k1) || keys.just_pressed(*k2) {
                let kind = UnitKind::ALL[i];
                if city.state.producing != Some(kind) {
                    city.state.producing = Some(kind);
                    log.push(format!("{} produit désormais : {}.", city.state.name, kind.name()));
                }
            }
        }
    }
}
