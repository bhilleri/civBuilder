//! Groupes d'unités : sélection multiple, ordre de groupe, groupes rappelés au clavier
//! (`docs/conception/conception-combat.md`, « Campagnes : déplacements de groupe »).
//!
//! - **Maj + clic** sur une de vos unités l'ajoute au groupe sélectionné (ou l'en retire) ; un
//!   clic simple sur la carte revient à une seule unité.
//! - **Clic droit** avec un groupe : l'unité principale va sur la case visée, les autres sur les
//!   cases libres les plus proches autour d'elle (chacune vers la plus proche d'elle-même).
//! - **Ctrl + chiffre** range le groupe sous ce chiffre ; le **chiffre** seul le rappelle (sauf
//!   quand une ville est choisie : les chiffres y choisissent la production) ; deux appuis rapides
//!   centrent la vue dessus.
//!
//! Plus tard : rectangle de sélection, chef d'armée, allure « en ordre ».

use crate::game::{City, Fog, GameLog, GameMap, HexPos, LogText, Selection, Unit, move_unit};
use crate::hex::Hex;
use crate::hostile::{Blocked, EnemyZoc, Hostile, Occupancy};
use crate::feedback::{Actor, Cues, TurnReplay};
use crate::input::{HoveredHex, Pilot};
use crate::hex::Wrap;
use crate::map::{KnownMap, MoveCosts};
use crate::pathfinding::find_path_turns;
use crate::rules::ForUnit;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;

/// Unités sélectionnées ensemble (l'unité principale est celle de `Selection`).
#[derive(Resource, Default, Debug)]
pub struct Groupe(pub Vec<Entity>);

/// Groupes rangés sous les chiffres 0 à 9 (Ctrl + chiffre), et le dernier rappel (double appui).
#[derive(Resource, Default)]
pub struct GroupesClavier {
    pub groupes: [Vec<Entity>; 10],
    dernier_rappel: Option<(usize, f64)>,
}

/// Délai entre deux appuis pour centrer la vue sur le groupe (s).
const DOUBLE_APPUI_S: f64 = 0.4;
/// Rayon (en cases) où chercher les cases d'arrivée autour de la cible.
const RAYON_ARRIVEE: i32 = 3;

/// Cases d'arrivée d'un groupe autour de `cible` : chaque unité (sauf la principale, qui va sur
/// la cible) reçoit une case libre à `RAYON_ARRIVEE` cases au plus de la cible, la plus proche
/// d'elle-même (les unités les plus proches de la cible choisissent d'abord).
pub fn repartir_arrivees(wrap: Wrap, cible: Hex, unites: &[(Entity, Hex)], libre: impl Fn(Hex) -> bool) -> Vec<(Entity, Hex)> {
    let mut candidates: Vec<Hex> = wrap.range(cible, RAYON_ARRIVEE).filter(|&h| h != cible && libre(h)).collect();
    candidates.sort_by_key(|&h| (wrap.distance(h, cible), h));
    let mut ordre: Vec<&(Entity, Hex)> = unites.iter().collect();
    ordre.sort_by_key(|(e, h)| (wrap.distance(*h, cible), *e));
    let mut resultat = Vec::new();
    for &&(e, depuis) in &ordre {
        let Some(i) = (0..candidates.len()).min_by_key(|&i| (wrap.distance(candidates[i], cible), wrap.distance(candidates[i], depuis))) else { break };
        resultat.push((e, candidates.remove(i)));
    }
    resultat
}

pub struct GroupesPlugin;

impl Plugin for GroupesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Groupe>()
            .init_resource::<GroupesClavier>();
        // (les systèmes sont rangés dans la chaîne des commandes de `input.rs`)
    }
}

/// Le bouton de la souris vient-il d'être pressé (souris ou pilote) ?
fn presse(mouse: &ButtonInput<MouseButton>, pilot: &Pilot, b: MouseButton) -> bool {
    mouse.just_pressed(b) || pilot.click == Some(b)
}

/// Maj + clic : ajoute ou retire une unité du groupe ; clic simple sur la carte : plus de groupe.
pub fn selection_de_groupe(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    pilot: Res<Pilot>,
    hovered: Res<HoveredHex>,
    selection: Res<Selection>,
    mut groupe: ResMut<Groupe>,
    mut log: ResMut<GameLog>,
    units: Query<(Entity, &HexPos), (With<Unit>, Without<Hostile>)>,
) {
    if !presse(&mouse, &pilot, MouseButton::Left) {
        return;
    }
    let Some(case) = hovered.0 else { return };
    let maj = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !maj {
        groupe.0.clear();
        return;
    }
    let Some((cliquee, _)) = units.iter().find(|(_, p)| p.0 == case) else { return };
    // L'unité principale d'avant le clic entre dans le groupe
    if groupe.0.is_empty()
        && let Some(e) = selection.0.filter(|e| units.contains(*e))
    {
        groupe.0.push(e);
    }
    match groupe.0.iter().position(|&e| e == cliquee) {
        Some(i) => {
            groupe.0.remove(i);
        }
        None => groupe.0.push(cliquee),
    }
    log.push(format!("Groupe : {} unité(s) sélectionnée(s).", groupe.0.len()));
}

/// Clic droit avec un groupe : les autres unités vont autour de la case visée.
#[allow(clippy::too_many_arguments)]
pub fn ordre_de_groupe(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    pilot: Res<Pilot>,
    hovered: Res<HoveredHex>,
    selection: Res<Selection>,
    replay: Res<TurnReplay>,
    mut groupe: ResMut<Groupe>,
    map: Res<GameMap>,
    (mut fog, mut log, mut cues): (ResMut<Fog>, ResMut<GameLog>, ResMut<Cues>),
    (blocked, mut occupancy, zoc): (Res<Blocked>, ResMut<Occupancy>, Res<EnemyZoc>),
    mut units: Query<(Entity, &mut Unit, &mut HexPos), (Without<Hostile>, Without<City>)>,
) {
    if !presse(&mouse, &pilot, MouseButton::Right) || replay.playing || keys.pressed(KeyCode::ShiftLeft) {
        return;
    }
    let (Some(cible), Some(principale)) = (hovered.0, selection.0) else { return };
    groupe.0.retain(|e| units.contains(*e));
    if groupe.0.len() < 2 || !groupe.0.contains(&principale) {
        return;
    }
    // Un ennemi visible sur la cible : seule l'unité principale attaque
    if occupancy.all.contains(&cible) && !occupancy.own.contains(&cible) && fog.visible.contains(&cible) {
        return;
    }
    let autres: Vec<(Entity, Hex)> = groupe.0.iter().filter(|&&e| e != principale).filter_map(|&e| units.get(e).ok().map(|(_, _, p)| (e, p.0))).collect();
    let membres: HashSet<Hex> = autres.iter().map(|(_, h)| *h).collect();
    let m = &map.0;
    let libre = |h: Hex| fog.explored.contains(&h) && m.move_cost(h).is_some() && (!occupancy.all.contains(&h) || membres.contains(&h)) && !blocked.0.contains(&h);
    let arrivees = repartir_arrivees(m.wrap(), cible, &autres, libre);
    let mut parties = 0;
    for (e, arrivee) in arrivees {
        let Ok((entity, mut unit, mut pos)) = units.get_mut(e) else { continue };
        {
            let known = KnownMap::new(m, &fog.explored).blocking(&blocked.0);
            let costs = ForUnit::new(&known, unit.0.kind);
            let Some(path) = find_path_turns(&costs, pos.0, arrivee, unit.0.moves_left, unit.0.move_debt, unit.0.kind.max_moves()) else { continue };
            unit.0.sleeping = false;
            unit.0.auto_explore = false;
            unit.0.set_order(pos.0, path, &costs);
        }
        let depart = pos.0;
        // (l'occupation est tenue à jour : l'unité principale part après, dans `mouse_commands`)
        let avant = crate::map::slot(pos.0, unit.0.on_bridge);
        let mut occupied = std::mem::take(&mut occupancy.all);
        move_unit(entity, &mut unit.0, &mut pos, m, &mut fog, &mut log, &blocked.0, &mut occupied, &zoc.0, &occupancy.own);
        occupancy.all = occupied;
        let apres = crate::map::slot(pos.0, unit.0.on_bridge);
        occupancy.own.remove(&avant);
        occupancy.own.insert(apres);
        if pos.0 != depart {
            cues.walk(entity, depart, &unit.0.last_steps, Actor::Player);
        }
        parties += 1;
    }
    if parties > 0 {
        log.push(LogText::new().text(format!("Groupe : {parties} unité(s) en route autour de la cible.")));
    }
}

/// Après le clic : l'unité principale reste dans le groupe (Maj + clic qui en retire une) ; un
/// groupe d'une seule unité n'en est plus un.
pub fn corriger_principale(mut selection: ResMut<Selection>, mut groupe: ResMut<Groupe>) {
    if groupe.0.len() < 2 {
        if !groupe.0.is_empty() {
            selection.0 = groupe.0.first().copied();
            groupe.0.clear();
        }
        return;
    }
    if selection.0.is_none_or(|e| !groupe.0.contains(&e)) {
        selection.0 = groupe.0.first().copied();
    }
}

/// Ctrl + chiffre : range le groupe ; chiffre : le rappelle ; deux appuis : centre la vue.
#[allow(clippy::too_many_arguments)]
pub fn groupes_au_clavier(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut selection: ResMut<Selection>,
    mut groupe: ResMut<Groupe>,
    mut groupes: ResMut<GroupesClavier>,
    mut log: ResMut<GameLog>,
    units: Query<(Entity, &HexPos), (With<Unit>, Without<Hostile>)>,
    cities: Query<(), With<City>>,
    camera: Single<&mut Transform, (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
    mut cam3d: ResMut<crate::view3d::Cam3d>,
) {
    const CHIFFRES: [(KeyCode, KeyCode); 10] = [
        (KeyCode::Digit0, KeyCode::Numpad0),
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
    let Some(n) = CHIFFRES.iter().position(|(a, b)| keys.just_pressed(*a) || keys.just_pressed(*b)) else { return };
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl {
        let membres: Vec<Entity> = if groupe.0.is_empty() { selection.0.filter(|e| units.contains(*e)).into_iter().collect() } else { groupe.0.clone() };
        if membres.is_empty() {
            return;
        }
        log.push(format!("Groupe {n} : {} unité(s) (touche {n} pour le rappeler).", membres.len()));
        groupes.groupes[n] = membres;
        return;
    }
    // (une ville choisie : les chiffres choisissent sa production)
    if selection.0.is_some_and(|e| cities.contains(e)) {
        return;
    }
    let vivants: Vec<Entity> = groupes.groupes[n].iter().copied().filter(|e| units.contains(*e)).collect();
    groupes.groupes[n] = vivants.clone();
    let Some(&premiere) = vivants.first() else { return };
    let maintenant = time.elapsed_secs_f64();
    let double = groupes.dernier_rappel.is_some_and(|(m, t)| m == n && maintenant - t < DOUBLE_APPUI_S);
    groupes.dernier_rappel = Some((n, maintenant));
    selection.0 = Some(premiere);
    groupe.0 = if vivants.len() > 1 { vivants.clone() } else { Vec::new() };
    if double && let Ok((_, p)) = units.get(premiere) {
        let (x, y) = p.0.to_world();
        let mut cam = camera.into_inner();
        cam.translation.x = x;
        cam.translation.y = y;
        cam3d.target = crate::view3d::to_3d(p.0, 0.0);
    } else {
        log.push(format!("Groupe {n} rappelé : {} unité(s).", vivants.len()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrivees_reparties_autour_de_la_cible() {
        let wrap = Wrap::new(40, true);
        let cible = Hex::from_offset(10, 10);
        let unites: Vec<(Entity, Hex)> = (0..5).map(|i| (Entity::from_raw_u32(i + 1).unwrap(), Hex::from_offset(2, 8 + i as i32))).collect();
        let occupee = cible.neighbors()[0];
        let arrivees = repartir_arrivees(wrap, cible, &unites, |h| h != occupee);
        assert_eq!(arrivees.len(), 5);
        let cases: HashSet<Hex> = arrivees.iter().map(|(_, h)| *h).collect();
        assert_eq!(cases.len(), 5, "une case chacune");
        assert!(!cases.contains(&cible) && !cases.contains(&occupee));
        assert!(arrivees.iter().all(|(_, h)| wrap.distance(*h, cible) == 1), "au plus près de la cible : {arrivees:?}");
        // Plus de place que d'unités : personne sans case ; pas de place : personne ne bouge
        assert!(repartir_arrivees(wrap, cible, &unites, |_| false).is_empty());
    }
}
