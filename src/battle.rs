//! Combat côté ECS : prépare l'engagement (terrain, flanquement, déplacement), applique le
//! résultat aux deux unités, l'inscrit au journal et disperse les vaincus.

use crate::combat::{self, AttackKind, Engagement, Fighter};
use crate::game::GameLog;
use crate::hex::Hex;
use crate::map::{Terrain, WorldMap};
use crate::rules::{self, UnitState};
use crate::units::UnitKind;
use bevy::prelude::*;

/// Une unité engagée dans un combat.
pub struct Side<'a> {
    pub entity: Entity,
    pub unit: &'a mut UnitState,
    pub hostile: bool,
}

/// Engagement entre deux unités : mode d'attaque naturel de l'attaquant (tir s'il en est capable),
/// terrain des deux cases, flanquement par les alliés de l'attaquant adjacents à la cible.
/// `will_move` : l'attaquant va se déplacer avant d'attaquer (charge).
pub fn engagement(
    map: &WorldMap,
    attacker: (&UnitState, Hex),
    defender: (&UnitState, Hex),
    attacker_allies: impl IntoIterator<Item = Hex>,
    will_move: bool,
) -> Engagement {
    let terrain = |h: Hex| map.get(h).unwrap_or(Terrain::Plains);
    let altitude = |h: Hex| map.elevation(h).unwrap_or(0.0).round() as i32;
    let fighter = |u: &UnitState, h: Hex| Fighter {
        kind: u.kind,
        hp: u.hp,
        terrain: terrain(h),
        altitude: altitude(h),
        relief: map.relief(h),
    };
    let (a, d) = (fighter(attacker.0, attacker.1), fighter(defender.0, defender.1));
    match combat::attack_kind(a.kind) {
        AttackKind::Ranged => Engagement::ranged(a, d),
        AttackKind::Melee => {
            let flankers = attacker_allies
                .into_iter()
                // Un allié séparé de la cible par une falaise ne la flanque pas
                .filter(|h| *h != attacker.1 && rules::in_attack_position(map, UnitKind::Militia, *h, defender.1))
                .count() as u32;
            let across = map.river_between(attacker.1, defender.1) || map.is_fleuve(attacker.1);
            Engagement::melee(a, d, attacker.0.moved_this_turn || will_move).flanked_by(flankers).across_river(across)
        }
    }
}

fn label(side: &Side) -> String {
    if side.hostile {
        format!("{} hostile", side.unit.kind.name())
    } else {
        format!("votre {}", side.unit.kind.name())
    }
}

/// Résultat vu par l'appelant (qui gère l'avance sur la case libérée et la sélection).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FightResult {
    pub kind: AttackKind,
    pub attacker_died: bool,
    pub defender_died: bool,
}

impl FightResult {
    /// Le vainqueur d'un corps à corps avance sur la case libérée (pas après un tir).
    pub fn attacker_advances(&self) -> bool {
        self.kind == AttackKind::Melee && self.defender_died && !self.attacker_died
    }
}

/// Résout un combat, l'inscrit au journal et disperse les unités à 0 PV.
pub fn fight(commands: &mut Commands, log: &mut GameLog, attacker: Side, defender: Side, e: &Engagement) -> FightResult {
    let (att_name, def_name) = (label(&attacker), label(&defender));
    let outcome = combat::apply(attacker.unit, defender.unit, e, &mut rand::rng());
    let mut msg = match outcome.kind {
        AttackKind::Ranged => format!(
            "{} tire sur {} : −{} PV.",
            capitalize(&att_name),
            def_name,
            outcome.damage_to_defender
        ),
        AttackKind::Melee => format!(
            "{}{} attaque {} : −{} PV / −{} PV.",
            capitalize(&att_name),
            if outcome.charge { " (charge)" } else { "" },
            def_name,
            outcome.damage_to_defender,
            outcome.damage_to_attacker
        ),
    };
    let result = FightResult {
        kind: outcome.kind,
        attacker_died: outcome.attacker_hp == 0,
        defender_died: outcome.defender_hp == 0,
    };
    if result.defender_died {
        msg += &format!(" {} est dispersé(e) !", capitalize(&def_name));
        commands.entity(defender.entity).despawn();
    }
    if result.attacker_died {
        msg += &format!(" {} est dispersé(e) !", capitalize(&att_name));
        commands.entity(attacker.entity).despawn();
    }
    log.push(msg);
    result
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
