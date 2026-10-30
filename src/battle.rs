//! Combat côté ECS : prépare l'engagement (terrain, flanquement, déplacement), applique le
//! résultat aux deux unités, l'inscrit au journal et disperse les vaincus.

use crate::combat::{self, AttackKind, Engagement, Fighter};
use crate::feedback::{Actor, Cue, Cues, Retired, Strike};
use crate::game::{GameLog, HexPos, Importance, LogText, Unit};
use crate::search::MapTarget;
use crate::hostile::{Hostile, HostileHome};
use crate::hex::Hex;
use crate::map::{Terrain, WorldMap};
use crate::rules::{self, UnitState};
use crate::units::UnitKind;
use bevy::prelude::*;

/// Une unité engagée dans un combat.
pub struct Side<'a> {
    pub entity: Entity,
    /// Case de l'unité au moment du combat.
    pub at: Hex,
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

/// Retire du jeu une unité dispersée ; elle reste montrée le temps de son effacement
/// (voir `feedback`), puis disparaît.
fn retire(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<(Unit, HexPos, Hostile, HostileHome)>().insert(Retired);
}

/// Résout un combat, l'inscrit au journal, le fait mettre en scène et disperse les unités à 0 PV.
pub fn fight(commands: &mut Commands, log: &mut GameLog, cues: &mut Cues, attacker: Side, defender: Side, e: &Engagement) -> FightResult {
    let (att_name, def_name) = (label(&attacker), label(&defender));
    // Les noms des unités renvoient vers elles (ou vers leur case si elles sont dispersées)
    let (att_link, def_link) = ((MapTarget::Unit(attacker.entity), attacker.at), (MapTarget::Unit(defender.entity), defender.at));
    let outcome = combat::apply(attacker.unit, defender.unit, e, &mut rand::rng());
    let msg = LogText::new().link(capitalize(&att_name), att_link.0, att_link.1);
    let mut msg = match outcome.kind {
        AttackKind::Ranged => msg.text(" tire sur ").link(&def_name, def_link.0, def_link.1).text(format!(" : −{} PV.", outcome.damage_to_defender)),
        AttackKind::Melee => msg
            .text(if outcome.charge { " (charge) attaque " } else { " attaque " })
            .link(&def_name, def_link.0, def_link.1)
            .text(format!(" : −{} PV / −{} PV.", outcome.damage_to_defender, outcome.damage_to_attacker)),
    };
    let result = FightResult {
        kind: outcome.kind,
        attacker_died: outcome.attacker_hp == 0,
        defender_died: outcome.defender_hp == 0,
    };
    cues.0.push(Cue::Strike(Strike {
        attacker: attacker.entity,
        defender: defender.entity,
        from: attacker.at,
        target: defender.at,
        ranged: outcome.kind == AttackKind::Ranged,
        actor: if attacker.hostile { Actor::Hostile } else { Actor::Player },
        // (PV réellement perdus : un coup ne retire pas plus que ce qui reste)
        to_defender: e.defender.hp - outcome.defender_hp,
        to_attacker: e.attacker.hp - outcome.attacker_hp,
        attacker_hp: outcome.attacker_hp,
        defender_hp: outcome.defender_hp,
        defender_is_player: !defender.hostile,
    }));
    if result.defender_died {
        msg = msg.text(" ").link(capitalize(&def_name), def_link.0, def_link.1).text(" est dispersé(e) !");
        retire(commands, defender.entity);
    }
    if result.attacker_died {
        msg = msg.text(" ").link(capitalize(&att_name), att_link.0, att_link.1).text(" est dispersé(e) !");
        retire(commands, attacker.entity);
    }
    // Perte d'une unité du joueur : critique ; ennemi dispersé : succès ; sinon combat (alerte)
    let lost = (result.defender_died && !defender.hostile) || (result.attacker_died && !attacker.hostile);
    let won = (result.defender_died && defender.hostile) || (result.attacker_died && attacker.hostile);
    let importance = if lost {
        Importance::Critical
    } else if won {
        Importance::Success
    } else {
        Importance::Warning
    };
    log.notify(importance, msg);
    result
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
