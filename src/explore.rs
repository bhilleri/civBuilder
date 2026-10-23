//! Exploration automatique (logique pure).
//!
//! Principe « frontière » : l'unité ne marche que sur du terrain connu et franchissable.
//! Chaque case candidate reçoit un score = cases inconnues qu'elle révélerait / (coût d'accès + 2).
//! Le +2 évite de surévaluer les cases toutes proches qui ne révèlent presque rien :
//! l'unité accepte un petit détour si cela découvre nettement plus de terrain.
//! La cible est réévaluée à chaque pas, ce qui fait suivre à l'unité la direction la plus riche.
//!
//! Coopération : chaque explorateur mémorise sa cible ; les cases qu'il y révélera sont
//! « réservées » et ne comptent pas dans le score des autres, qui partent donc ailleurs.
//! S'il ne reste que des zones réservées, l'unité y va quand même plutôt que de s'arrêter.
//!
//! Villages : un village connu vaut `VILLAGE_VALUE` cases découvertes ; les explorateurs font
//! donc un détour pour les visiter (et un village visé par un explorateur est réservé).

use crate::hex::Hex;
use crate::map::{MoveCosts, WorldMap};
use crate::rules::UnitState;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::LazyLock;

/// Pondération du coût dans le score (voir l'en-tête du module).
const COST_OFFSET: f32 = 2.0;
/// Coût supplémentaire pour entrer dans la zone dangereuse.
pub const DANGER_PENALTY: u32 = 6;
/// Rayon de la zone dangereuse autour d'un ennemi aperçu (sa portée de chasse).
pub const THREAT_RADIUS: i32 = 2;
/// Valeur d'un village connu, en « cases découvertes » équivalentes.
pub const VILLAGE_VALUE: usize = 25;

static EMPTY: LazyLock<HashSet<Hex>> = LazyLock::new(HashSet::new);

/// Ce que l'explorateur sait du monde, hors brouillard (passé à part car il évolue en route).
#[derive(Clone, Copy)]
pub struct ExploreWorld<'a> {
    pub map: &'a WorldMap,
    /// Cases que d'autres explorateurs vont révéler (voir `reserved_by`).
    pub reserved: &'a HashSet<Hex>,
    /// Cases infranchissables (ennemis connus).
    pub blocked: &'a HashSet<Hex>,
    /// Villages connus, à visiter.
    pub villages: &'a HashSet<Hex>,
    /// Cases réellement occupées (une unité par case) : vérifiées à chaque pas.
    pub occupied: &'a HashSet<Hex>,
    /// Zone dangereuse (abords des ennemis et des camps connus) : l'explorateur l'évite autant que
    /// possible (coût supplémentaire), sans s'y interdire de passage pour pouvoir en sortir.
    pub danger: &'a HashSet<Hex>,
    /// Positions réelles des ennemis : ceux que l'explorateur aperçoit en route s'ajoutent
    /// aussitôt à la zone dangereuse (portée de chasse `THREAT_RADIUS`).
    pub threats: &'a HashSet<Hex>,
}

impl<'a> ExploreWorld<'a> {
    pub fn new(map: &'a WorldMap) -> Self {
        Self { map, reserved: &EMPTY, blocked: &EMPTY, villages: &EMPTY, occupied: &EMPTY, danger: &EMPTY, threats: &EMPTY }
    }
    pub fn reserved(self, reserved: &'a HashSet<Hex>) -> Self {
        Self { reserved, ..self }
    }
    pub fn blocked(self, blocked: &'a HashSet<Hex>) -> Self {
        Self { blocked, ..self }
    }
    pub fn villages(self, villages: &'a HashSet<Hex>) -> Self {
        Self { villages, ..self }
    }
    pub fn occupied(self, occupied: &'a HashSet<Hex>) -> Self {
        Self { occupied, ..self }
    }
    pub fn danger(self, danger: &'a HashSet<Hex>) -> Self {
        Self { danger, ..self }
    }
    pub fn threats(self, threats: &'a HashSet<Hex>) -> Self {
        Self { threats, ..self }
    }
}

/// Cases inconnues (et sur la carte) qu'une unité verrait depuis `at`, hors cases réservées.
pub fn reveal_gain(map: &WorldMap, explored: &HashSet<Hex>, reserved: &HashSet<Hex>, at: Hex, sight: i32) -> usize {
    at.range(sight)
        .filter(|h| map.contains(*h) && !explored.contains(h) && !reserved.contains(h))
        .count()
}

/// Cases que les autres explorateurs vont révéler : (cible, vision) de chacun.
pub fn reserved_by(others: impl IntoIterator<Item = (Hex, i32)>) -> HashSet<Hex> {
    others.into_iter().flat_map(|(target, sight)| target.range(sight)).collect()
}

/// Meilleure cible d'exploration et chemin pour l'atteindre (départ exclu),
/// ou `None` s'il ne reste rien d'accessible à découvrir ni à visiter.
pub fn choose_target(world: &ExploreWorld, explored: &HashSet<Hex>, start: Hex, sight: i32) -> Option<(Hex, Vec<Hex>)> {
    let free = world.reserved(&EMPTY);
    // Par ordre de préférence : cible sûre et non réservée, cible sûre réservée par un autre
    // explorateur, puis — s'il ne reste rien d'autre — cible dans la zone dangereuse.
    search_target(world, explored, start, sight, true)
        .or_else(|| search_target(&free, explored, start, sight, true))
        .or_else(|| search_target(world, explored, start, sight, false))
        .or_else(|| search_target(&free, explored, start, sight, false))
}

fn search_target(
    world: &ExploreWorld,
    explored: &HashSet<Hex>,
    start: Hex,
    sight: i32,
    safe_targets_only: bool,
) -> Option<(Hex, Vec<Hex>)> {
    let ExploreWorld { map, reserved, blocked, villages, danger, .. } = *world;
    // Dijkstra limité au terrain connu et franchissable
    let mut cost: HashMap<Hex, u32> = HashMap::from([(start, 0)]);
    let mut came_from: HashMap<Hex, Hex> = HashMap::new();
    let mut open = BinaryHeap::from([Reverse((0u32, start))]);
    let mut best: Option<(f32, u32, Hex)> = None;

    while let Some(Reverse((c, current))) = open.pop() {
        if c > cost[&current] {
            continue;
        }
        if current != start && !(safe_targets_only && danger.contains(&current)) {
            let village = villages.contains(&current) && !reserved.contains(&current);
            let gain = reveal_gain(map, explored, reserved, current, sight) + if village { VILLAGE_VALUE } else { 0 };
            if gain > 0 {
                let score = gain as f32 / (c as f32 + COST_OFFSET);
                // Départage déterministe : meilleur score, puis moindre coût, puis coordonnées
                let better = match best {
                    None => true,
                    Some((s, bc, bh)) => score > s + 1e-6 || ((score - s).abs() <= 1e-6 && (c, current) < (bc, bh)),
                };
                if better {
                    best = Some((score, c, current));
                }
            }
        }
        for next in current.neighbors() {
            // Terrain connu seulement ; les abords des ennemis coûtent cher
            if !explored.contains(&next) || blocked.contains(&next) {
                continue;
            }
            let Some(step) = map.step_cost(current, next) else { continue };
            let step = step + if danger.contains(&next) { DANGER_PENALTY } else { 0 };
            let total = c + step;
            if cost.get(&next).is_none_or(|&b| total < b) {
                cost.insert(next, total);
                came_from.insert(next, current);
                open.push(Reverse((total, next)));
            }
        }
    }

    let (_, _, target) = best?;
    let mut path = vec![target];
    let mut node = target;
    while let Some(&prev) = came_from.get(&node) {
        if prev == start {
            break;
        }
        path.push(prev);
        node = prev;
    }
    path.reverse();
    Some((target, path))
}

/// Issue d'un tour d'exploration automatique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExploreOutcome {
    /// L'unité a exploré (ou attend le tour suivant faute de PM).
    Exploring,
    /// Plus rien d'accessible à découvrir : l'exploration s'arrête.
    Finished,
}

/// Dépense les PM de l'unité à explorer, pas à pas, en réévaluant la cible après chaque pas.
/// `unit.path` reçoit le chemin restant vers la cible courante (pour l'affichage) et
/// `unit.explore_target` la cible, que les autres explorateurs éviteront.
pub fn explore_step(unit: &mut UnitState, pos: &mut Hex, world: &ExploreWorld, explored: &mut HashSet<Hex>) -> ExploreOutcome {
    let map = world.map;
    let sight = unit.kind.sight();
    unit.last_steps.clear();
    explored.extend(pos.range(sight));
    // Cases occupées découvertes en route (une unité par case), ajoutées aux cases bloquées ;
    // ennemis aperçus en route, ajoutés à la zone dangereuse.
    let mut blocked = world.blocked.clone();
    let mut danger = world.danger.clone();
    loop {
        for &t in world.threats {
            if t.distance(*pos) <= sight {
                danger.extend(t.range(THREAT_RADIUS));
            }
        }
        let current = ExploreWorld { blocked: &blocked, danger: &danger, ..*world };
        let Some((target, path)) = choose_target(&current, explored, *pos, sight) else {
            unit.path.clear();
            unit.explore_target = None;
            return ExploreOutcome::Finished;
        };
        unit.explore_target = Some(target);
        unit.path = path;
        if unit.moves_left == 0 || unit.zoc_stopped {
            return ExploreOutcome::Exploring;
        }
        let next = unit.path[0];
        if world.occupied.contains(&next) {
            blocked.insert(next);
            continue;
        }
        unit.path.remove(0);
        let cost = map.step_cost(*pos, next).expect("le chemin d'exploration n'emprunte que du terrain franchissable");
        unit.moves_left = unit.moves_left.saturating_sub(cost);
        *pos = next;
        unit.last_steps.push(next);
        unit.moved_this_turn = true;
        explored.extend(pos.range(sight));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;
    use crate::rules::UnitKind;

    #[test]
    fn rien_a_decouvrir() {
        let map = WorldMap::filled(10, 10, Terrain::Plains);
        let explored: HashSet<Hex> = map.hexes().collect();
        assert!(choose_target(&ExploreWorld::new(&map), &explored, Hex::from_offset(5, 5), 2).is_none());
    }

    #[test]
    fn va_vers_la_frontiere_la_plus_proche() {
        let map = WorldMap::filled(30, 10, Terrain::Plains);
        let start = Hex::from_offset(10, 5);
        // Tout est connu sauf deux zones : une proche à l'est, une lointaine à l'ouest
        let mut explored: HashSet<Hex> = map.hexes().collect();
        let near = Hex::from_offset(17, 5);
        let far = Hex::from_offset(1, 5);
        for h in near.range(1).chain(far.range(1)) {
            explored.remove(&h);
        }
        let (target, path) = choose_target(&ExploreWorld::new(&map), &explored, start, 2).unwrap();
        assert!(target.distance(near) <= 3, "cible {target:?}");
        assert!(path.iter().all(|h| explored.contains(h)), "ne marche que sur du connu");
    }

    #[test]
    fn prefere_un_gain_plus_grand_a_cout_egal() {
        let map = WorldMap::filled(30, 12, Terrain::Plains);
        let start = Hex::from_offset(15, 6);
        let mut explored: HashSet<Hex> = map.hexes().collect();
        // Est : une seule case inconnue. Ouest, même distance : une grande zone inconnue.
        explored.remove(&Hex::from_offset(21, 6));
        for h in Hex::from_offset(9, 6).range(2) {
            explored.remove(&h);
        }
        let (target, _) = choose_target(&ExploreWorld::new(&map), &explored, start, 2).unwrap();
        assert!(target.to_offset().0 < 15, "devrait partir vers l'ouest, cible {target:?}");
    }

    #[test]
    fn ignore_l_inconnu_inaccessible() {
        // Île de départ entourée d'eau : l'inconnu au-delà de la mer ne doit pas être visé
        let mut map = WorldMap::filled(30, 30, Terrain::Ocean);
        let center = Hex::from_offset(15, 15);
        for h in center.range(2) {
            map.set(h, Terrain::Plains);
        }
        let explored: HashSet<Hex> = center.range(5).collect();
        let mut unit = UnitState::new(UnitKind::Scout);
        let mut pos = center;
        let mut explored = explored;
        let mut outcome = ExploreOutcome::Exploring;
        for _ in 0..20 {
            outcome = explore_step(&mut unit, &mut pos, &ExploreWorld::new(&map), &mut explored);
            if outcome == ExploreOutcome::Finished {
                break;
            }
            unit.moves_left = unit.kind.max_moves();
        }
        assert_eq!(outcome, ExploreOutcome::Finished);
        assert!(!map.get(pos).unwrap().is_water());
    }

    /// Deux explorateurs au même endroit, deux zones inconnues de part et d'autre :
    /// le second évite la zone visée par le premier.
    #[test]
    fn deux_explorateurs_se_repartissent() {
        let map = WorldMap::filled(40, 12, Terrain::Plains);
        let start = Hex::from_offset(20, 6);
        let mut explored: HashSet<Hex> = map.hexes().collect();
        for h in Hex::from_offset(30, 6).range(2).chain(Hex::from_offset(10, 6).range(2)) {
            explored.remove(&h);
        }
        let (first, _) = choose_target(&ExploreWorld::new(&map), &explored, start, 3).unwrap();
        let reserved = reserved_by([(first, 3)]);
        let (second, _) = choose_target(&ExploreWorld::new(&map).reserved(&reserved), &explored, start, 3).unwrap();
        let east = |h: Hex| h.to_offset().0 > 20;
        assert_ne!(east(first), east(second), "premier {first:?}, second {second:?}");
    }

    /// S'il ne reste qu'une zone, déjà visée par un autre, on y va quand même.
    #[test]
    fn zone_reservee_en_dernier_recours() {
        let map = WorldMap::filled(30, 12, Terrain::Plains);
        let start = Hex::from_offset(10, 6);
        let mut explored: HashSet<Hex> = map.hexes().collect();
        let zone = Hex::from_offset(20, 6);
        for h in zone.range(1) {
            explored.remove(&h);
        }
        let reserved = reserved_by([(Hex::from_offset(18, 6), 3)]);
        assert!(choose_target(&ExploreWorld::new(&map).reserved(&reserved), &explored, start, 2).is_some());
    }

    /// Un village connu, un peu plus loin qu'une frontière proche, attire l'explorateur.
    #[test]
    fn detour_pour_un_village() {
        let map = WorldMap::filled(40, 12, Terrain::Plains);
        let start = Hex::from_offset(20, 6);
        let mut explored: HashSet<Hex> = map.hexes().collect();
        // Petite frontière à 4 cases à l'est ; village connu à 7 cases à l'ouest
        explored.remove(&Hex::from_offset(24, 6));
        let village = Hex::from_offset(13, 6);
        let villages = HashSet::from([village]);
        let (target, _) = choose_target(&ExploreWorld::new(&map).villages(&villages), &explored, start, 2).unwrap();
        assert_eq!(target, village);
        // Sans le village, il irait vers la frontière
        let (target, _) = choose_target(&ExploreWorld::new(&map), &explored, start, 2).unwrap();
        assert!(target.to_offset().0 > 20);
    }

    /// Un village visé par un autre explorateur ne l'intéresse plus.
    #[test]
    fn village_reserve_par_un_autre() {
        let map = WorldMap::filled(40, 12, Terrain::Plains);
        let start = Hex::from_offset(20, 6);
        let mut explored: HashSet<Hex> = map.hexes().collect();
        explored.remove(&Hex::from_offset(26, 6));
        let village = Hex::from_offset(14, 6);
        let villages = HashSet::from([village]);
        let reserved = reserved_by([(village, 2)]);
        let world = ExploreWorld::new(&map).villages(&villages).reserved(&reserved);
        let (target, _) = choose_target(&world, &explored, start, 2).unwrap();
        assert_ne!(target, village);
    }

    /// L'explorateur contourne les abords d'un ennemi connu.
    #[test]
    fn evite_la_zone_dangereuse() {
        let map = WorldMap::filled(30, 12, Terrain::Plains);
        let start = Hex::from_offset(5, 6);
        let mut explored: HashSet<Hex> = map.hexes().collect();
        for h in Hex::from_offset(20, 6).range(2) {
            explored.remove(&h);
        }
        let enemy = Hex::from_offset(12, 6);
        let danger: HashSet<Hex> = enemy.range(2).collect();
        let blocked = HashSet::from([enemy]);
        let world = ExploreWorld::new(&map).blocked(&blocked).danger(&danger);
        let (_, path) = choose_target(&world, &explored, start, 2).unwrap();
        assert!(path.iter().all(|h| !danger.contains(h)), "{path:?}");
    }

    /// Un ennemi aperçu en cours de route est aussitôt évité.
    #[test]
    fn menace_decouverte_en_route() {
        let map = WorldMap::filled(30, 12, Terrain::Plains);
        let start = Hex::from_offset(4, 6);
        let mut explored: HashSet<Hex> = map.hexes().filter(|h| h.to_offset().0 <= 10).collect();
        // Ennemi juste au-delà de la vision de départ, sur la route directe vers l'inconnu
        let enemy = Hex::from_offset(8, 6);
        let threats = HashSet::from([enemy]);
        let occupied = HashSet::from([enemy]);
        let world = ExploreWorld::new(&map).threats(&threats).occupied(&occupied);
        let mut unit = UnitState::new(UnitKind::LightCavalry); // 4 PM, vision 3
        let mut pos = start;
        explore_step(&mut unit, &mut pos, &world, &mut explored);
        assert!(pos.distance(enemy) > THREAT_RADIUS, "arrêt à {:?}, trop près de l'ennemi", pos.to_offset());
    }

    /// Un explorateur déjà dans la zone dangereuse peut en sortir (pas de blocage).
    #[test]
    fn sortir_de_la_zone_dangereuse() {
        let map = WorldMap::filled(30, 12, Terrain::Plains);
        let start = Hex::from_offset(12, 6);
        let mut explored: HashSet<Hex> = map.hexes().collect();
        for h in Hex::from_offset(25, 6).range(2) {
            explored.remove(&h);
        }
        let danger: HashSet<Hex> = start.range(3).collect();
        let world = ExploreWorld::new(&map).danger(&danger);
        assert!(choose_target(&world, &explored, start, 2).is_some());
    }

    /// Sur un continent, l'exploration finit par révéler toute la terre accessible.
    #[test]
    fn explore_tout_le_continent() {
        for seed in 0..3 {
            let map = WorldMap::generate(40, 30, seed);
            let start = map.find_start().unwrap();
            let mut unit = UnitState::new(UnitKind::Scout);
            let mut pos = start;
            let mut explored = HashSet::new();
            let mut turns = 0;
            while explore_step(&mut unit, &mut pos, &ExploreWorld::new(&map), &mut explored) == ExploreOutcome::Exploring {
                assert!(map.get(pos).unwrap().move_cost().is_some());
                unit.moves_left = unit.kind.max_moves();
                turns += 1;
                assert!(turns < 500, "l'exploration doit se terminer");
            }
            // Toute case terrestre reliée au départ par voie de terre est désormais connue
            let mut seen = HashSet::from([start]);
            let mut stack = vec![start];
            while let Some(h) = stack.pop() {
                for n in h.neighbors() {
                    if map.move_cost(n).is_some() && seen.insert(n) {
                        stack.push(n);
                    }
                }
            }
            let missing = seen.iter().filter(|h| !explored.contains(h)).count();
            assert_eq!(missing, 0, "graine {seed} : {missing} cases accessibles non explorées");
        }
    }
}
