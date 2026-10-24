//! Combat (logique pure) : corps à corps et tir à distance.
//!
//! Dégâts ≈ 30 × e^(écart de force ÷ 25) × aléa (0,8 à 1,2), sur 100 PV.
//! En mêlée, le défenseur riposte ; un tir ne reçoit pas de riposte.
//! Modificateurs : bonus de type (anti-cavalerie…), charge, flanquement, terrain, surplomb, blessures.

use crate::map::{Relief, Terrain};
use crate::rules::{MAX_HP, UnitState};
use crate::units::{UnitClass, UnitKind};
use rand::Rng;

pub const BASE_DAMAGE: f32 = 30.0;
pub const STRENGTH_SCALE: f32 = 25.0;
pub const CHARGE_BONUS: i32 = 8;
/// Bonus d'attaque en mêlée par allié adjacent à la cible (hors attaquant).
pub const FLANK_BONUS: i32 = 2;
/// Bonus de défense en forêt et dans des collines (cumulables : forêt vallonnée).
pub const TERRAIN_DEFENSE: i32 = 3;
/// Bonus de défense en mêlée quand le défenseur est à une altitude supérieure à l'attaquant.
pub const HIGHER_GROUND_DEFENSE: i32 = 2;
/// Bonus de défense en mêlée contre un attaquant qui franchit une rivière ou sort d'un fleuve.
pub const RIVER_DEFENSE: i32 = 3;
/// Bonus de force de tir quand l'archer est plus haut que sa cible (en %).
pub const HIGH_GROUND_PERCENT: i32 = 20;
pub const RANDOM_MIN: f32 = 0.8;
pub const RANDOM_MAX: f32 = 1.2;

/// Un combattant : type, PV, terrain et altitude de sa case.
#[derive(Clone, Copy, Debug)]
pub struct Fighter {
    pub kind: UnitKind,
    pub hp: i32,
    pub terrain: Terrain,
    /// Altitude de la case (m).
    pub altitude: i32,
    /// Relief de la case (collines, montagne…).
    pub relief: Relief,
}

/// Écart d'altitude (m) à partir duquel une case est « en hauteur » par rapport à une autre.
pub const HIGHER_GROUND_M: i32 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackKind {
    Melee,
    Ranged,
}

/// Situation tactique d'une attaque.
#[derive(Clone, Copy, Debug)]
pub struct Engagement {
    pub attacker: Fighter,
    pub defender: Fighter,
    pub kind: AttackKind,
    /// L'attaquant s'est déplacé ce tour-ci (charge de cavalerie).
    pub attacker_moved: bool,
    /// Alliés de l'attaquant adjacents à la cible, hors attaquant (flanquement, mêlée seulement).
    pub flanking_allies: u32,
    /// L'attaquant franchit une rivière ou sort d'un fleuve pour frapper (mêlée).
    pub across_river: bool,
}

impl Engagement {
    pub fn melee(attacker: Fighter, defender: Fighter, attacker_moved: bool) -> Self {
        Self { attacker, defender, kind: AttackKind::Melee, attacker_moved, flanking_allies: 0, across_river: false }
    }
    pub fn ranged(attacker: Fighter, defender: Fighter) -> Self {
        Self { attacker, defender, kind: AttackKind::Ranged, attacker_moved: false, flanking_allies: 0, across_river: false }
    }
    pub fn flanked_by(self, allies: u32) -> Self {
        Self { flanking_allies: allies, ..self }
    }
    pub fn across_river(self, across: bool) -> Self {
        Self { across_river: across, ..self }
    }
}

/// Mode d'attaque naturel d'un type d'unité : tir pour les unités qui en sont capables.
pub fn attack_kind(kind: UnitKind) -> AttackKind {
    if kind.stats().ranged_attack > 0 { AttackKind::Ranged } else { AttackKind::Melee }
}

/// Portée effective d'un tir : −1 si l'archer est à une altitude plus basse que la cible ou si
/// la cible est en forêt ; jamais moins de 1 (un tir adjacent reste possible). 0 = pas de tir.
/// (Les collines sont un relief interne à la case : elles ne comptent pas dans la hauteur.)
pub fn effective_range(kind: UnitKind, from: (i32, Terrain), target: (i32, Terrain)) -> i32 {
    let base = kind.stats().range;
    if base == 0 {
        return 0;
    }
    let malus = target.0 - from.0 >= HIGHER_GROUND_M || target.1.is_wooded();
    (base - i32::from(malus)).max(1)
}

/// L'unité peut-elle attaquer maintenant ? (une attaque par tour, il faut des PM ;
/// l'archer expert ne tire pas après s'être déplacé)
pub fn can_attack_now(unit: &UnitState) -> bool {
    unit.kind.can_attack()
        && !unit.has_attacked
        && unit.moves_left > 0
        && !(unit.kind == UnitKind::ExpertArcher && unit.moved_this_turn)
}

/// Bonus de `me` face à `other` en mêlée (s'applique en attaque comme en défense).
pub fn bonus_vs(me: UnitKind, other: UnitKind) -> Option<(i32, &'static str)> {
    match (me, other.class()) {
        (UnitKind::Spearman, UnitClass::Cavalry) => Some((10, "anti-cavalerie")),
        (UnitKind::Pikeman, UnitClass::Cavalry) => Some((20, "anti-cavalerie")),
        (UnitKind::LightCavalry, UnitClass::Ranged) => Some((10, "contre archers")),
        (UnitKind::LightCavalry, _) if other == UnitKind::Militia || other == UnitKind::HorseArcher => {
            Some((10, if other == UnitKind::Militia { "contre milice" } else { "contre archers" }))
        }
        _ => None,
    }
}

/// La charge s'applique si l'attaquant (cavalerie de mêlée) a parcouru au moins une case
/// ce tour-ci et que la cible n'est ni en forêt ni dans des collines.
pub fn charge_applies(attacker: UnitKind, moved_this_turn: bool, target_terrain: Terrain, target_relief: Relief) -> bool {
    attacker.can_charge() && moved_this_turn && !target_terrain.is_wooded() && target_terrain != Terrain::Marsh && !target_relief.blocks_charge()
}

/// Malus de blessure : −1 de force par tranche de 10 PV perdus.
pub fn wound_penalty(hp: i32) -> i32 {
    (MAX_HP - hp.clamp(0, MAX_HP)) / 10
}

/// Force effective d'un camp et détail des modificateurs (pour l'affichage).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strength {
    pub value: i32,
    pub modifiers: Vec<(String, i32)>,
}

impl Strength {
    fn build(base: i32, mut modifiers: Vec<(String, i32)>, hp: i32) -> Self {
        let wound = wound_penalty(hp);
        if wound > 0 {
            modifiers.push(("blessures".to_string(), -wound));
        }
        let value = (base + modifiers.iter().map(|(_, v)| v).sum::<i32>()).max(1);
        Strength { value, modifiers }
    }
}

/// Dégâts infligés pour un écart de force donné et un facteur aléatoire.
pub fn damage(strength_diff: i32, factor: f32) -> i32 {
    let raw = BASE_DAMAGE * (strength_diff as f32 / STRENGTH_SCALE).exp() * factor;
    (raw.round() as i32).clamp(1, MAX_HP)
}

/// Fourchette de dégâts (minimum, attendu, maximum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageRange {
    pub min: i32,
    pub expected: i32,
    pub max: i32,
}

impl DamageRange {
    const NONE: DamageRange = DamageRange { min: 0, expected: 0, max: 0 };

    fn for_diff(diff: i32) -> Self {
        Self { min: damage(diff, RANDOM_MIN), expected: damage(diff, 1.0), max: damage(diff, RANDOM_MAX) }
    }
}

/// Probabilité qu'une unité soit dispersée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    Survives,
    MayDie,
    Dies,
}

fn fate(hp: i32, dmg: DamageRange) -> Fate {
    if dmg.max == 0 || dmg.max < hp {
        Fate::Survives
    } else if dmg.min >= hp {
        Fate::Dies
    } else {
        Fate::MayDie
    }
}

/// Prévision d'un combat, affichée avant d'attaquer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub kind: AttackKind,
    pub attack: Strength,
    pub defense: Strength,
    pub charge: bool,
    pub to_defender: DamageRange,
    pub to_attacker: DamageRange,
    pub defender_fate: Fate,
    pub attacker_fate: Fate,
}

pub fn preview(e: &Engagement) -> Preview {
    let (a, d) = (&e.attacker, &e.defender);
    let (sa, sd) = (a.kind.stats(), d.kind.stats());
    let mut def_mods = Vec::new();
    if d.terrain.is_wooded() {
        def_mods.push((d.terrain.name().to_lowercase(), TERRAIN_DEFENSE));
    }
    if d.relief.defense_steps() > 0 {
        let label = if d.relief == Relief::Hills { "collines".to_string() } else { format!("relief {}", d.relief.name()) };
        def_mods.push((label, TERRAIN_DEFENSE * d.relief.defense_steps()));
    }

    let (attack, defense, charge) = match e.kind {
        AttackKind::Melee => {
            let mut att_mods = Vec::new();
            if let Some((b, label)) = bonus_vs(a.kind, d.kind) {
                att_mods.push((label.to_string(), b));
            }
            let charge = charge_applies(a.kind, e.attacker_moved, d.terrain, d.relief);
            if charge {
                att_mods.push(("charge".to_string(), CHARGE_BONUS));
            }
            if e.flanking_allies > 0 {
                att_mods.push(("flanquement".to_string(), FLANK_BONUS * e.flanking_allies as i32));
            }
            if let Some((b, label)) = bonus_vs(d.kind, a.kind) {
                def_mods.push((label.to_string(), b));
            }
            if d.altitude - a.altitude >= HIGHER_GROUND_M {
                def_mods.push(("en hauteur".to_string(), HIGHER_GROUND_DEFENSE));
            }
            if e.across_river {
                def_mods.push(("rivière à franchir".to_string(), RIVER_DEFENSE));
            }
            (
                Strength::build(sa.melee_attack, att_mods, a.hp),
                Strength::build(sd.melee_defense, def_mods, d.hp),
                charge,
            )
        }
        AttackKind::Ranged => {
            let mut att_mods = Vec::new();
            if a.altitude - d.altitude >= HIGHER_GROUND_M {
                att_mods.push(("surplomb".to_string(), (sa.ranged_attack * HIGH_GROUND_PERCENT + 50) / 100));
            }
            (
                Strength::build(sa.ranged_attack, att_mods, a.hp),
                Strength::build(sd.ranged_defense, def_mods, d.hp),
                false,
            )
        }
    };
    let to_defender = DamageRange::for_diff(attack.value - defense.value);
    let to_attacker = match e.kind {
        AttackKind::Melee => DamageRange::for_diff(defense.value - attack.value),
        AttackKind::Ranged => DamageRange::NONE,
    };
    Preview {
        kind: e.kind,
        defender_fate: fate(d.hp, to_defender),
        attacker_fate: fate(a.hp, to_attacker),
        attack,
        defense,
        charge,
        to_defender,
        to_attacker,
    }
}

/// Résultat d'un combat : PV restants de chaque camp (0 = dispersé).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub kind: AttackKind,
    pub damage_to_defender: i32,
    pub damage_to_attacker: i32,
    pub attacker_hp: i32,
    pub defender_hp: i32,
    pub charge: bool,
}

pub fn resolve(e: &Engagement, rng: &mut impl Rng) -> Outcome {
    let p = preview(e);
    let diff = p.attack.value - p.defense.value;
    let to_def = damage(diff, rng.random_range(RANDOM_MIN..=RANDOM_MAX));
    let to_att = match e.kind {
        AttackKind::Melee => damage(-diff, rng.random_range(RANDOM_MIN..=RANDOM_MAX)),
        AttackKind::Ranged => 0,
    };
    Outcome {
        kind: e.kind,
        damage_to_defender: to_def,
        damage_to_attacker: to_att,
        attacker_hp: (e.attacker.hp - to_att).max(0),
        defender_hp: (e.defender.hp - to_def).max(0),
        charge: p.charge,
    }
}

/// Applique un combat aux deux unités : PV, indicateurs, PM de l'attaquant (l'archer monté
/// garde ses PM moins 1 pour se replier après un tir ; les autres terminent leur tour).
/// L'avance sur la case libérée et la dispersion (PV à 0) sont gérées par l'appelant.
pub fn apply(attacker: &mut UnitState, defender: &mut UnitState, e: &Engagement, rng: &mut impl Rng) -> Outcome {
    let outcome = resolve(e, rng);
    attacker.hp = outcome.attacker_hp;
    defender.hp = outcome.defender_hp;
    attacker.moves_left = if e.kind == AttackKind::Ranged && attacker.kind == UnitKind::HorseArcher {
        attacker.moves_left.saturating_sub(1)
    } else {
        0
    };
    attacker.has_attacked = true;
    attacker.fought_this_turn = true;
    defender.fought_this_turn = true;
    attacker.clear_order();
    attacker.auto_explore = false;
    defender.clear_order();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use UnitKind::*;

    fn f(kind: UnitKind) -> Fighter {
        Fighter { kind, hp: MAX_HP, terrain: Terrain::Plains, altitude: 150, relief: Relief::Flat }
    }

    fn on(kind: UnitKind, terrain: Terrain) -> Fighter {
        Fighter { kind, hp: MAX_HP, terrain, altitude: 150, relief: Relief::Flat }
    }

    fn on_hills(kind: UnitKind, terrain: Terrain) -> Fighter {
        Fighter { relief: Relief::Hills, ..on(kind, terrain) }
    }

    /// Prévision d'un corps à corps simple.
    fn pm(a: &Fighter, d: &Fighter, moved: bool) -> Preview {
        preview(&Engagement::melee(*a, *d, moved))
    }

    #[test]
    fn riviere_a_franchir() {
        let plain = pm(&f(Militia), &f(Militia), false);
        let wet = preview(&Engagement::melee(f(Militia), f(Militia), false).across_river(true));
        assert_eq!(wet.defense.value, plain.defense.value + RIVER_DEFENSE);
        // Le tir ignore la rivière
        let shot = preview(&Engagement::ranged(f(ExpertArcher), f(Militia)).across_river(true));
        let flat = preview(&Engagement::ranged(f(ExpertArcher), f(Militia)));
        assert_eq!(shot.defense.value, flat.defense.value);
    }

    #[test]
    fn forces_egales_trente_degats() {
        let p = pm(&f(Militia), &f(Militia), false);
        assert_eq!(p.to_defender.expected, 30);
        assert_eq!(p.to_attacker.expected, 30);
        assert_eq!((p.to_defender.min, p.to_defender.max), (24, 36));
    }

    #[test]
    fn dix_points_d_avance() {
        // Arme lourde (20) contre milice (8) : +12
        let p = pm(&f(HeavyWarrior), &f(Militia), false);
        assert!(p.to_defender.expected > 45, "{:?}", p.to_defender);
        assert!(p.to_attacker.expected < 20, "{:?}", p.to_attacker);
    }

    #[test]
    fn charge_conditions() {
        assert!(charge_applies(LightCavalry, true, Terrain::Plains, Relief::Flat));
        assert!(!charge_applies(LightCavalry, false, Terrain::Plains, Relief::Flat), "sans déplacement");
        assert!(!charge_applies(HeavyCavalry, true, Terrain::Forest, Relief::Flat), "cible en forêt");
        assert!(!charge_applies(HeavyCavalry, true, Terrain::Grassland, Relief::Hills), "cible dans des collines");
        assert!(!charge_applies(HorseArcher, true, Terrain::Plains, Relief::Flat), "pas pour l'archer monté");
        assert!(!charge_applies(HeavyWarrior, true, Terrain::Plains, Relief::Flat), "pas pour l'infanterie");

        let still = pm(&f(HeavyCavalry), &f(ShieldWarrior), false);
        let charging = pm(&f(HeavyCavalry), &f(ShieldWarrior), true);
        assert_eq!(charging.attack.value, still.attack.value + CHARGE_BONUS);
        assert!(charging.to_defender.expected as f32 > still.to_defender.expected as f32 * 1.35);
    }

    #[test]
    fn les_lances_arretent_la_cavalerie() {
        // Cavalerie lourde en charge (24 + 8) contre piquier (13 + 20)
        let p = pm(&f(HeavyCavalry), &f(Pikeman), true);
        assert!(p.defense.value >= p.attack.value, "{} vs {}", p.attack.value, p.defense.value);
        // … alors qu'elle écrase un guerrier arme lourde
        let p = pm(&f(HeavyCavalry), &f(HeavyWarrior), true);
        assert!(p.attack.value - p.defense.value >= 20);
    }

    #[test]
    fn cavalerie_legere_devastatrice_contre_archers_et_milice() {
        for target in [ExpertArcher, ArcherSwordsman, Militia] {
            let p = pm(&f(LightCavalry), &f(target), true);
            assert!(p.to_defender.expected >= 60, "{target:?} : {:?}", p.to_defender);
        }
    }

    #[test]
    fn les_blessures_affaiblissent() {
        let healthy = pm(&f(Militia), &f(Militia), false);
        let wounded = pm(&Fighter { hp: 40, ..f(Militia) }, &f(Militia), false);
        assert_eq!(wounded.attack.value, healthy.attack.value - 6);
        assert!(wounded.to_defender.expected < healthy.to_defender.expected);
    }

    #[test]
    fn issue_annoncee() {
        let p = pm(&f(HeavyCavalry), &Fighter { hp: 10, ..f(Militia) }, true);
        assert_eq!(p.defender_fate, Fate::Dies);
        let p = pm(&f(Militia), &f(ShieldWarrior), false);
        assert_eq!(p.defender_fate, Fate::Survives);
    }

    #[test]
    fn melee_applique_le_resultat() {
        let mut rng = StdRng::seed_from_u64(9);
        let mut att = UnitState::new(LightCavalry);
        att.moved_this_turn = true;
        let mut def = UnitState::new(Militia);
        let e = Engagement::melee(f(LightCavalry), f(Militia), true);
        let o = apply(&mut att, &mut def, &e, &mut rng);
        assert!(o.charge);
        assert_eq!(att.moves_left, 0);
        assert!(att.fought_this_turn && def.fought_this_turn);
        assert_eq!(def.hp, (MAX_HP - o.damage_to_defender).max(0));
        assert_eq!(att.hp, MAX_HP - o.damage_to_attacker);
    }

    #[test]
    fn terrain_et_flanquement() {
        let plain = pm(&f(Militia), &f(Militia), false);
        let forest = pm(&f(Militia), &on(Militia, Terrain::Forest), false);
        let hills = pm(&f(Militia), &on_hills(Militia, Terrain::Plains), false);
        let both = pm(&f(Militia), &on_hills(Militia, Terrain::Forest), false);
        assert_eq!(forest.defense.value, plain.defense.value + TERRAIN_DEFENSE);
        assert_eq!(hills.defense.value, plain.defense.value + TERRAIN_DEFENSE);
        assert_eq!(both.defense.value, plain.defense.value + 2 * TERRAIN_DEFENSE, "forêt vallonnée : cumul");
        let mountain = pm(&f(Militia), &Fighter { relief: Relief::Mountain, ..f(Militia) }, false);
        assert_eq!(mountain.defense.value, plain.defense.value + 2 * TERRAIN_DEFENSE, "montagne : double");
        let flanked = preview(&Engagement::melee(f(Militia), f(Militia), false).flanked_by(2));
        assert_eq!(flanked.attack.value, plain.attack.value + 2 * FLANK_BONUS);
        // Pas de flanquement pour un tir
        let shot = preview(&Engagement::ranged(f(ExpertArcher), f(Militia)).flanked_by(3));
        assert!(shot.attack.modifiers.iter().all(|(l, _)| l != "flanquement"));
    }

    #[test]
    fn tir_sans_riposte() {
        let p = preview(&Engagement::ranged(f(ExpertArcher), f(Militia)));
        assert_eq!(p.kind, AttackKind::Ranged);
        assert_eq!(p.attack.value, 20);
        assert_eq!(p.defense.value, 6, "défense à distance de la milice");
        assert_eq!(p.to_attacker, DamageRange::NONE);
        assert_eq!(p.attacker_fate, Fate::Survives);
        // Le guerrier bouclier encaisse bien mieux les flèches (~40 % de dégâts en moins)
        let shield = preview(&Engagement::ranged(f(ExpertArcher), f(ShieldWarrior)));
        assert!((shield.to_defender.expected as f32) < p.to_defender.expected as f32 * 0.7);
    }

    #[test]
    fn hauteur_et_portee() {
        use Terrain::*;
        // Archer expert (portée 2)
        let (p, fo) = ((450, Plains), (450, Forest));
        assert_eq!(effective_range(ExpertArcher, p, p), 2);
        assert_eq!(effective_range(ExpertArcher, p, (850, Plains)), 1, "archer plus bas");
        assert_eq!(effective_range(ExpertArcher, p, (600, Plains)), 2, "écart trop faible");
        assert_eq!(effective_range(ExpertArcher, (850, Plains), p), 2);
        assert_eq!(effective_range(ExpertArcher, p, fo), 1, "cible en forêt");
        // Archer monté (portée 1) : jamais moins de 1
        assert_eq!(effective_range(HorseArcher, p, (850, Plains)), 1);
        assert_eq!(effective_range(Militia, p, p), 0, "pas de tir");
        // +20 % en surplomb
        let flat = preview(&Engagement::ranged(f(ExpertArcher), f(Militia)));
        let hills = preview(&Engagement::ranged(on_hills(ExpertArcher, Plains), f(Militia)));
        assert_eq!(hills.attack.value, flat.attack.value, "les collines ne donnent pas de surplomb");
        let above = preview(&Engagement::ranged(Fighter { altitude: 450, ..f(ExpertArcher) }, f(Militia)));
        assert_eq!(above.attack.value, flat.attack.value + 4, "plus haut de 300 m");
    }

    #[test]
    fn defense_en_hauteur() {
        let flat = pm(&f(Militia), &f(Militia), false);
        let high = pm(&f(Militia), &Fighter { altitude: 450, ..f(Militia) }, false);
        assert_eq!(high.defense.value, flat.defense.value + HIGHER_GROUND_DEFENSE);
        let low = pm(&f(Militia), &Fighter { altitude: 0, ..f(Militia) }, false);
        assert_eq!(low.defense.value, flat.defense.value, "pas de malus en contrebas");
    }

    #[test]
    fn archer_monte_garde_des_pm() {
        let mut rng = StdRng::seed_from_u64(5);
        let mut att = UnitState::new(HorseArcher);
        let mut def = UnitState::new(Militia);
        apply(&mut att, &mut def, &Engagement::ranged(f(HorseArcher), f(Militia)), &mut rng);
        assert_eq!(att.moves_left, 3);
        assert_eq!(att.hp, MAX_HP);
        assert!(!can_attack_now(&att), "une seule attaque par tour");
        let mut expert = UnitState::new(ExpertArcher);
        expert.moved_this_turn = true;
        assert!(!can_attack_now(&expert), "l'archer expert ne tire pas après s'être déplacé");
    }

    #[test]
    fn resolution_dans_la_fourchette() {
        let mut rng = StdRng::seed_from_u64(3);
        let (a, d) = (f(Spearman), f(LightCavalry));
        let p = pm(&a, &d, false);
        for _ in 0..1_000 {
            let o = resolve(&Engagement::melee(a, d, false), &mut rng);
            assert!((p.to_defender.min..=p.to_defender.max).contains(&o.damage_to_defender));
            assert!((p.to_attacker.min..=p.to_attacker.max).contains(&o.damage_to_attacker));
            assert_eq!(o.defender_hp, (MAX_HP - o.damage_to_defender).max(0));
        }
    }
}
