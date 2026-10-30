//! Règles du jeu indépendantes du moteur : unités, villes, croissance, production.

use crate::hex::{Hex, Wrap};
use crate::map::{KnownMap, MoveCosts, StepCost, WorldMap, Yields, turn_moves};
use crate::combat::{self, AttackKind};
use crate::pathfinding::{find_path, find_path_turns};
use bevy::platform::collections::HashSet;

// ---------------------------------------------------------------- Unités

pub use crate::units::UnitKind;

/// Points de vie d'une unité en pleine santé.
pub const MAX_HP: i32 = 100;

/// État d'une unité (sans sa position, stockée à part).
#[derive(Clone, Debug)]
pub struct UnitState {
    pub kind: UnitKind,
    pub moves_left: u32,
    /// PM dus pour un terrain plus cher que les PM qu'il restait (montagne…), retirés des tours suivants.
    pub move_debt: u32,
    /// La dette a pris tous les PM de ce tour-ci : l'unité ne peut pas bouger.
    pub immobilized: bool,
    /// Chemin restant à parcourir (ordre de déplacement sur plusieurs tours).
    pub path: Vec<Hex>,
    /// L'unité a « passé son tour » (ou est fortifiée) : ignorée par « unité suivante ».
    pub sleeping: bool,
    /// Coût maximal toléré pour l'ordre en cours (garde-fou anti-détour), `None` = sans limite.
    pub order_budget: Option<u32>,
    /// PM déjà dépensés depuis que l'ordre en cours a été donné.
    pub order_spent: u32,
    /// Exploration automatique en cours (voir le module `explore`).
    pub auto_explore: bool,
    /// Case visée par l'exploration automatique (réservée vis-à-vis des autres explorateurs).
    pub explore_target: Option<Hex>,
    /// Cases traversées lors du dernier déplacement (pour déclencher les villages en chemin).
    pub last_steps: Vec<Hex>,
    /// Points de vie (0 = unité dispersée).
    pub hp: i32,
    /// A parcouru au moins une case ce tour-ci (charge de cavalerie, soins).
    pub moved_this_turn: bool,
    /// A combattu ce tour-ci, en attaque ou en défense (soins).
    pub fought_this_turn: bool,
    /// A déjà attaqué ce tour-ci (une attaque par tour).
    pub has_attacked: bool,
    /// Entrée dans une zone de contrôle ennemie : plus de déplacement ce tour-ci (l'attaque reste possible).
    pub zoc_stopped: bool,
}

/// Marge tolérée sur le coût estimé d'un ordre avant de le suspendre : +50 %.
pub const DETOUR_TOLERANCE_PERCENT: u32 = 50;

/// Issue d'un déplacement le long d'un ordre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveOutcome {
    /// Déplacement normal (ordre en cours ou terminé).
    Moved,
    /// La destination s'est révélée inaccessible : ordre annulé.
    Unreachable,
    /// Le trajet recalculé dépasse nettement l'estimation initiale : ordre suspendu.
    TooLong,
}

impl UnitState {
    pub fn new(kind: UnitKind) -> Self {
        Self {
            kind,
            moves_left: kind.max_moves(),
            move_debt: 0,
            immobilized: false,
            path: Vec::new(),
            sleeping: false,
            order_budget: None,
            order_spent: 0,
            auto_explore: false,
            explore_target: None,
            last_steps: Vec::new(),
            hp: MAX_HP,
            moved_this_turn: false,
            fought_this_turn: false,
            has_attacked: false,
            zoc_stopped: false,
        }
    }

    /// Donne un ordre de déplacement. Son coût est estimé avec la carte connue
    /// et sert de référence au garde-fou anti-détour.
    pub fn set_order(&mut self, from: Hex, path: Vec<Hex>, known: &impl MoveCosts) {
        let estimate = path_cost(known, from, &path);
        self.order_budget = Some(estimate + (estimate * DETOUR_TOLERANCE_PERCENT).div_ceil(100));
        self.order_spent = 0;
        self.path = path;
        self.sleeping = false;
        self.auto_explore = false;
        self.explore_target = None;
    }

    /// Début de tour : soins si l'unité ne s'est ni déplacée ni battue au tour précédent
    /// (+10 PV, +20 en territoire ami), puis PM restaurés (moins la dette) et indicateurs remis à zéro.
    pub fn start_turn(&mut self, in_friendly_territory: bool) {
        if !self.moved_this_turn && !self.fought_this_turn && self.hp < MAX_HP {
            let heal = if in_friendly_territory { 20 } else { 10 };
            self.hp = (self.hp + heal).min(MAX_HP);
        }
        (self.moves_left, self.move_debt) = turn_moves(self.kind.max_moves(), self.move_debt);
        self.immobilized = self.moves_left == 0;
        self.moved_this_turn = false;
        self.fought_this_turn = false;
        self.has_attacked = false;
        self.zoc_stopped = false;
    }

    pub fn clear_order(&mut self) {
        self.path.clear();
        self.explore_target = None;
        self.order_budget = None;
        self.order_spent = 0;
    }

    /// Paie un pas : PM restants, et dette pour le terrain qui dépasse les PM qu'il restait.
    pub fn pay(&mut self, step: StepCost) {
        let (left, debt) = step.spend(self.moves_left);
        self.moves_left = left;
        self.move_debt += debt;
    }

    /// Tours sans pouvoir bouger qui restent à cause de la dette (celui-ci compris s'il l'est).
    pub fn stuck_turns(&self) -> u32 {
        u32::from(self.immobilized) + self.move_debt / self.kind.max_moves().max(1)
    }

    pub fn needs_orders(&self) -> bool {
        self.moves_left > 0 && self.path.is_empty() && !self.sleeping && !self.auto_explore
    }

    /// Avance le long de l'ordre tant qu'il reste des points de mouvement.
    /// On peut toujours entrer sur une case s'il reste au moins 1 PM ; le terrain qui coûte plus
    /// que les PM restants laisse une dette, payée sur les tours suivants (voir [`StepCost`]).
    ///
    /// Découverte du terrain : à chaque pas, l'unité révèle les cases
    /// autour d'elle (`explored`) et recalcule son trajet vers la destination avec ce qu'elle
    /// sait désormais. Retourne `true` si la destination s'avère inaccessible (ordre annulé).
    pub fn advance_exploring(
        &mut self,
        pos: &mut Hex,
        map: &WorldMap,
        explored: &mut HashSet<Hex>,
        blocked: &HashSet<Hex>,
        occupied: &HashSet<Hex>,
        zoc: &HashSet<Hex>,
    ) -> MoveOutcome {
        let sight = self.kind.sight();
        self.last_steps.clear();
        explored.extend(map.range(*pos, sight));
        // Cases bloquées connues + cases occupées découvertes en route (une unité par case)
        let mut blocked = blocked.clone();
        while self.moves_left > 0 && !self.zoc_stopped {
            let Some(&destination) = self.path.last() else { break };
            let known = ForUnit::new(KnownMap::new(map, explored).blocking(&blocked), self.kind);
            let Some(path) = find_path_turns(&known, *pos, destination, self.moves_left, self.move_debt, self.kind.max_moves()) else {
                self.clear_order();
                return MoveOutcome::Unreachable;
            };
            // Garde-fou : le trajet total (déjà parcouru + reste estimé) ne doit pas exploser
            if let Some(budget) = self.order_budget
                && self.order_spent + path_cost(&known, *pos, &path) > budget
            {
                self.clear_order();
                return MoveOutcome::TooLong;
            }
            self.path = path;
            let next = self.path[0];
            // Case suivante occupée (elle est voisine, donc visible) : on recalcule en l'évitant.
            if occupied.contains(&next) {
                blocked.insert(next);
                continue;
            }
            self.path.remove(0);
            // La case suivante est voisine, donc déjà révélée (vision ≥ 1) : son coût est le vrai.
            let Some(step) = ForUnit::new(map, self.kind).step(*pos, next) else {
                self.clear_order();
                return MoveOutcome::Unreachable;
            };
            self.pay(step);
            self.order_spent += step.total();
            *pos = next;
            self.last_steps.push(next);
            self.moved_this_turn = true;
            explored.extend(map.range(*pos, sight));
            // Zone de contrôle ennemie : l'unité s'arrête (l'ordre reprendra au tour suivant)
            if zoc.contains(&next) {
                self.zoc_stopped = true;
            }
        }
        if self.path.is_empty() {
            self.clear_order();
        }
        MoveOutcome::Moved
    }
}

/// Cases adjacentes aux ennemis qui exercent une zone de contrôle.
pub fn zoc_tiles(wrap: Wrap, enemies: impl IntoIterator<Item = (UnitKind, Hex)>) -> HashSet<Hex> {
    enemies.into_iter().filter(|(k, _)| k.exerts_zoc()).flat_map(|(_, h)| wrap.neighbors(h)).collect()
}

/// Coûts de déplacement qui interdisent de traverser une zone de contrôle
/// (on peut seulement y terminer son trajet, sur `goal`).
struct AvoidZoc<'a, M: MoveCosts> {
    inner: &'a M,
    zoc: &'a HashSet<Hex>,
    goal: Hex,
}

impl<M: MoveCosts> MoveCosts for AvoidZoc<'_, M> {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        if hex != self.goal && self.zoc.contains(&hex) { None } else { self.inner.move_cost(hex) }
    }

    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        self.move_cost(to)?;
        self.inner.step(from, to)
    }

    fn wrap(&self) -> Wrap {
        self.inner.wrap()
    }
}

/// Coûts de déplacement vus par un type d'unité : le pas en montagne suit [`UnitKind::mountain_cost`].
pub struct ForUnit<M: MoveCosts> {
    inner: M,
    kind: UnitKind,
}

impl<M: MoveCosts> ForUnit<M> {
    pub fn new(inner: M, kind: UnitKind) -> Self {
        Self { inner, kind }
    }
}

impl<M: MoveCosts> MoveCosts for ForUnit<M> {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        self.inner.move_cost(hex)
    }

    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        let mut step = self.inner.step(from, to)?;
        if step.mountain {
            step.terrain = self.kind.mountain_cost(step.terrain);
        }
        Some(step)
    }

    fn wrap(&self) -> Wrap {
        self.inner.wrap()
    }
}

/// L'unité peut-elle attaquer `target` depuis `at` ? (adjacente en mêlée, à portée effective pour un tir)
/// Pas de corps à corps à travers une falaise.
pub fn in_attack_position(world: &WorldMap, kind: UnitKind, at: Hex, target: Hex) -> bool {
    let spot = |h: Hex| (world.elevation(h).unwrap_or(0.0).round() as i32, world.get(h).unwrap_or(crate::map::Terrain::Plains));
    match combat::attack_kind(kind) {
        AttackKind::Melee => world.distance(at, target) == 1 && !world.cliff_between(at, target),
        AttackKind::Ranged => at != target && world.distance(at, target) <= combat::effective_range(kind, spot(at), spot(target)),
    }
}

/// Trajet pour attaquer `target` ce tour-ci : chemin (éventuellement vide) jusqu'à une case
/// d'où l'attaque est possible (adjacente en mêlée, à portée pour un tir), en gardant au moins
/// 1 PM et sans traverser de zone de contrôle. L'archer expert ne se déplace pas pour tirer.
/// `None` si l'attaque est impossible ce tour-ci.
pub fn attack_approach(
    known: &impl MoveCosts,
    world: &WorldMap,
    unit: &UnitState,
    from: Hex,
    target: Hex,
    zoc: &HashSet<Hex>,
) -> Option<Vec<Hex>> {
    if !combat::can_attack_now(unit) {
        return None;
    }
    if in_attack_position(world, unit.kind, from, target) {
        return Some(Vec::new());
    }
    if unit.kind == UnitKind::ExpertArcher || unit.zoc_stopped {
        return None;
    }
    let reach = unit.kind.stats().range.max(1);
    let known = &ForUnit::new(known, unit.kind);
    world
        .range(target, reach)
        .filter(|&h| h != target && known.move_cost(h).is_some() && in_attack_position(world, unit.kind, h, target))
        .filter_map(|h| {
            let path = find_path(&AvoidZoc { inner: known, zoc, goal: h }, from, h)?;
            let mut moves = unit.moves_left;
            let mut at = from;
            for &step in &path {
                if moves == 0 {
                    return None;
                }
                moves = moves.saturating_sub(known.step_cost(at, step)?);
                at = step;
            }
            (moves > 0).then(|| (path_cost(known, from, &path), h, path))
        })
        .min_by_key(|(cost, h, _)| (*cost, *h))
        .map(|(_, _, path)| path)
}

/// Coût total d'un chemin partant de `from` (pas infranchissables comptés 1, ils sont exclus en amont).
pub fn path_cost(map: &impl MoveCosts, from: Hex, path: &[Hex]) -> u32 {
    steps(from, path).map(|(a, b)| map.step_cost(a, b).unwrap_or(1)).sum()
}

/// Pas successifs (case de départ, case d'arrivée) d'un chemin partant de `from`.
fn steps(from: Hex, path: &[Hex]) -> impl Iterator<Item = (Hex, Hex)> + '_ {
    std::iter::once(from).chain(path.iter().copied()).zip(path.iter().copied())
}

/// Pour chaque case d'un chemin, numéro du tour où l'unité l'atteint
/// (1 = ce tour-ci). Même règle que [`UnitState::advance_exploring`] : on peut entrer
/// sur une case tant qu'il reste au moins 1 PM ; sinon on attend le tour suivant, ou plusieurs
/// tant que la dette (`debt`, puis celle laissée par un terrain trop cher) prend tous les PM.
pub fn path_turns(map: &impl MoveCosts, from: Hex, path: &[Hex], moves_left: u32, debt: u32, max_moves: u32) -> Vec<u32> {
    let max_moves = max_moves.max(1);
    let mut turn = 1;
    let (mut moves, mut debt) = (moves_left, debt);
    steps(from, path)
        .map(|(a, b)| {
            while moves == 0 {
                turn += 1;
                (moves, debt) = turn_moves(max_moves, debt);
            }
            let step = map.step(a, b).unwrap_or(StepCost::terrain(1));
            let (left, owed) = step.spend(moves);
            moves = left;
            debt += owed;
            turn
        })
        .collect()
}

// ---------------------------------------------------------------- Villes

pub const MIN_CITY_DISTANCE: i32 = 4;

/// Rayon du territoire d'une ville selon sa population.
pub fn territory_radius(population: i32) -> i32 {
    if population >= 3 { 2 } else { 1 }
}

/// Nourriture à accumuler pour passer à la population suivante.
pub fn growth_threshold(population: i32) -> i32 {
    15 + 6 * (population - 1)
}

pub fn can_found_city(map: &WorldMap, at: Hex, cities: &[Hex]) -> Result<(), &'static str> {
    match map.get(at) {
        None => return Err("hors de la carte"),
        Some(t) if t.move_cost().is_none() => return Err("terrain inconstructible"),
        _ => {}
    }
    if map.is_fleuve(at) {
        return Err("pas de ville dans le lit d'un fleuve : choisissez une rive");
    }
    if map.relief(at) >= crate::map::Relief::Mountain {
        return Err("pas de ville en montagne");
    }
    if cities.iter().any(|&c| map.distance(c, at) < MIN_CITY_DISTANCE) {
        return Err("trop près d'une autre ville");
    }
    Ok(())
}

/// Rendement de la case centrale : au moins 2 nourriture, 2 production, 1 or.
pub fn center_yields(map: &WorldMap, center: Hex) -> Yields {
    let y = map.yields(center).unwrap_or_default();
    Yields::new(y.food.max(2), y.production.max(2), y.gold.max(1))
}

fn tile_score(y: Yields) -> i32 {
    y.food * 4 + y.production * 3 + y.gold * 2
}

/// Choisit les `population` meilleures cases exploitables du territoire
/// (hors centre et hors cases déjà prises par d'autres villes).
pub fn choose_worked_tiles(
    map: &WorldMap,
    center: Hex,
    population: i32,
    territory: &[Hex],
    taken: &HashSet<Hex>,
) -> Vec<Hex> {
    let mut candidates: Vec<Hex> = territory
        .iter()
        .copied()
        .filter(|&h| h != center && !taken.contains(&h) && map.contains(h))
        .collect();
    candidates.sort_by_key(|&h| (-tile_score(map.yields(h).unwrap_or_default()), h));
    candidates.truncate(population.max(0) as usize);
    candidates
}

/// Rendement total d'une ville (centre + cases exploitées).
pub fn city_yields(map: &WorldMap, center: Hex, worked: &[Hex]) -> Yields {
    let mut total = center_yields(map, center);
    for &h in worked {
        if let Some(y) = map.yields(h) {
            total += y;
        }
    }
    total
}

/// État économique d'une ville.
#[derive(Clone, Debug)]
pub struct CityState {
    pub name: String,
    pub population: i32,
    pub food_stock: i32,
    pub production_stock: i32,
    pub producing: Option<UnitKind>,
}

/// Ce qui s'est passé dans une ville pendant la fin de tour.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CityTurnReport {
    pub grew: bool,
    pub starved: bool,
    pub built: Option<UnitKind>,
    pub gold: i32,
}

impl CityState {
    pub fn new(name: String) -> Self {
        Self { name, population: 1, food_stock: 0, production_stock: 0, producing: Some(UnitKind::Militia) }
    }

    /// Consommation : 2 nourriture par habitant.
    pub fn food_surplus(&self, yields: Yields) -> i32 {
        yields.food - 2 * self.population
    }

    pub fn process_turn(&mut self, yields: Yields) -> CityTurnReport {
        let mut report = CityTurnReport { gold: yields.gold, ..Default::default() };

        // Croissance
        self.food_stock += self.food_surplus(yields);
        if self.food_stock >= growth_threshold(self.population) {
            self.food_stock -= growth_threshold(self.population);
            self.population += 1;
            report.grew = true;
        } else if self.food_stock < 0 {
            if self.population > 1 {
                self.population -= 1;
                report.starved = true;
            }
            self.food_stock = 0;
        }

        // Production
        if let Some(kind) = self.producing {
            self.production_stock += yields.production;
            let pop_ok = kind != UnitKind::Settler || self.population >= 2;
            if self.production_stock >= kind.cost() && pop_ok {
                self.production_stock -= kind.cost();
                if kind == UnitKind::Settler {
                    self.population -= 1;
                }
                report.built = Some(kind);
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Relief, Terrain};

    /// Couloir horizontal sur la ligne `row`, entouré de montagnes : le trajet est imposé.
    fn corridor(width: i32, row: i32) -> WorldMap {
        let mut map = WorldMap::filled(width, row + 3, Terrain::Plains);
        for h in map.hexes().collect::<Vec<_>>() {
            if h.to_offset().1 != row {
                map.set_relief(h, Relief::Peak);
            }
        }
        map
    }

    fn all_known(map: &WorldMap) -> HashSet<Hex> {
        map.hexes().collect()
    }

    #[test]
    fn deplacement_consomme_les_pm() {
        let mut map = corridor(10, 2);
        map.set(Hex::from_offset(3, 2), Terrain::Forest);
        let mut explored = all_known(&map);
        let mut u = UnitState::new(UnitKind::Militia); // 2 PM
        let mut pos = Hex::from_offset(1, 2);
        u.path = vec![Hex::from_offset(2, 2), Hex::from_offset(3, 2), Hex::from_offset(4, 2)];
        // plaine (1) puis forêt (2) : la forêt est accessible avec 1 PM restant
        assert_eq!(u.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new()), MoveOutcome::Moved);
        assert_eq!(pos, Hex::from_offset(3, 2));
        assert_eq!(u.moves_left, 0);
        assert_eq!(u.path, vec![Hex::from_offset(4, 2)]);
    }

    #[test]
    fn tours_sur_un_chemin() {
        let mut map = WorldMap::filled(12, 12, Terrain::Plains);
        let path: Vec<Hex> = (2..8).map(|c| Hex::from_offset(c, 2)).collect();
        let from = Hex::from_offset(1, 2);
        // 6 cases de plaine à 2 PM/tour : 2 cases par tour
        assert_eq!(path_turns(&map, from, &path, 2, 0, 2), vec![1, 1, 2, 2, 3, 3]);
        // unité ayant déjà bougé (0 PM) : départ au tour suivant
        assert_eq!(path_turns(&map, from, &path, 0, 0, 2), vec![2, 2, 3, 3, 4, 4]);
        // une forêt avec 1 PM restant reste accessible, mais le PM qui manque est dû au tour suivant
        map.set(path[1], Terrain::Forest);
        assert_eq!(path_turns(&map, from, &path, 2, 0, 2), vec![1, 1, 2, 3, 3, 4]);
        map.set(path[0], Terrain::Forest);
        assert_eq!(path_turns(&map, from, &path, 2, 0, 2), vec![1, 2, 3, 3, 4, 4]);
        // dette de départ : 3 PM dus, à 2 PM par tour → immobile au tour 2, 1 PM au tour 3
        let plain = WorldMap::filled(12, 12, Terrain::Plains);
        assert_eq!(path_turns(&plain, from, &path, 0, 3, 2), vec![3, 4, 4, 5, 5, 6]);
    }

    /// Montagne : le coût dépasse les PM, la dette immobilise l'unité les tours suivants.
    #[test]
    fn dette_en_montagne() {
        use crate::map::{MOUNTAIN_MOVE_MIN, StepCost};
        let mut map = WorldMap::filled(8, 8, Terrain::Plains);
        let (a, b) = (Hex::from_offset(2, 2), Hex::from_offset(3, 2));
        map.set_relief(b, Relief::Mountain);
        let mut unit = UnitState::new(UnitKind::Militia);
        unit.pay(ForUnit::new(&map, unit.kind).step(a, b).unwrap());
        assert_eq!((unit.moves_left, unit.move_debt), (0, MOUNTAIN_MOVE_MIN - 2));
        assert_eq!(unit.stuck_turns(), 1, "4 PM à 2 PM par tour : un tour immobilisé");
        unit.start_turn(false);
        assert!(unit.immobilized && unit.moves_left == 0 && unit.stuck_turns() == 1);
        unit.start_turn(false);
        assert!(!unit.immobilized && unit.moves_left == 2 && unit.stuck_turns() == 0);
        // Rivière et fleuve : arrêt sans dette
        let river = StepCost { terrain: 1, water: 2, mountain: false };
        assert_eq!(river.spend(1), (0, 0));
        // Cavalerie : double ; éclaireur : deux tiers
        let cav = ForUnit::new(&map, UnitKind::LightCavalry).step(a, b).unwrap();
        let scout = ForUnit::new(&map, UnitKind::Scout).step(a, b).unwrap();
        assert_eq!((cav.terrain, scout.terrain), (2 * MOUNTAIN_MOVE_MIN, 3));
    }

    /// Un massif large de 3 cases : la ligne droite (≈ 2 tours par montagne) perd face au
    /// contournement par la plaine, qui reste possible.
    #[test]
    fn contourne_un_massif() {
        let mut map = WorldMap::filled(20, 14, Terrain::Plains);
        for c in 8..11 {
            for r in 3..11 {
                map.set_relief(Hex::from_offset(c, r), Relief::Mountain);
            }
        }
        let (from, to) = (Hex::from_offset(5, 6), Hex::from_offset(13, 6));
        let path = find_path_turns(&map, from, to, 2, 0, 2).unwrap();
        assert!(path.iter().all(|&h| map.relief(h) != Relief::Mountain), "contournement : {path:?}");
        // Sans détour possible (massif sur toute la hauteur), on traverse quand même
        for r in 0..14 {
            for c in 8..11 {
                map.set_relief(Hex::from_offset(c, r), Relief::Mountain);
            }
        }
        let path = find_path_turns(&map, from, to, 2, 0, 2).unwrap();
        assert!(path.iter().any(|&h| map.relief(h) == Relief::Mountain));
    }

    /// Bilan (à lancer à la main) : montagnes par coût, et part des trajets de 6 à 12 cases
    /// partant d'une montagne qui la quittent par le plus court en tours plutôt qu'à travers le massif.
    #[test]
    #[ignore]
    fn bilan_montagnes() {
        use crate::map::{MOUNTAIN_MOVE_MAX, MOUNTAIN_MOVE_MIN};
        for seed in [777, 2, 5] {
            let map = WorldMap::generate(192, 111, seed);
            let mountains: Vec<Hex> = map.hexes().filter(|&h| map.relief(h) == Relief::Mountain).collect();
            let by_cost: Vec<usize> = (MOUNTAIN_MOVE_MIN..=MOUNTAIN_MOVE_MAX)
                .map(|c| mountains.iter().filter(|&&h| map.move_cost(h) == Some(c)).count())
                .collect();
            // Traversées : d'une case plate à 4 cases d'une montagne vers la case opposée
            let (mut crossings, mut through) = (0, 0);
            for &m in mountains.iter().step_by(7) {
                for d in 0..3 {
                    let dir = crate::hex::DIRECTIONS[d];
                    let far = Hex { q: dir.q * 4, r: dir.r * 4 };
                    let (a, b) = (map.canonical(m + far), map.canonical(m - far));
                    if map.relief(a) >= Relief::Mountain || map.relief(b) >= Relief::Mountain || map.move_cost(a).is_none() || map.move_cost(b).is_none() {
                        continue;
                    }
                    let Some(path) = find_path_turns(&ForUnit::new(&map, UnitKind::Militia), a, b, 2, 0, 2) else { continue };
                    crossings += 1;
                    through += usize::from(path.iter().filter(|&&h| map.relief(h) == Relief::Mountain).count() >= 2);
                }
            }
            println!(
                "graine {seed} : {} montagnes, par coût {MOUNTAIN_MOVE_MIN}..{MOUNTAIN_MOVE_MAX} PM : {by_cost:?} ; \
                 traversées de massif (milice) : {through}/{crossings} passent par au moins 2 montagnes",
                mountains.len()
            );
        }
    }

    #[test]
    fn montagne_selon_l_altitude() {
        use crate::map::{MOUNTAIN_MOVE_MAX, MOUNTAIN_MOVE_MIN, mountain_move_cost};
        assert_eq!(mountain_move_cost(1_500.0), MOUNTAIN_MOVE_MIN);
        assert_eq!(mountain_move_cost(1_999.0), MOUNTAIN_MOVE_MIN);
        assert_eq!(mountain_move_cost(2_600.0), MOUNTAIN_MOVE_MIN + 2);
        assert_eq!(mountain_move_cost(6_000.0), MOUNTAIN_MOVE_MAX);
    }

    /// Le calcul prévisionnel doit concorder avec le déplacement réel tour par tour.
    #[test]
    fn prevision_concorde_avec_le_deplacement() {
        let mut map = corridor(14, 3);
        map.set_hills(Hex::from_offset(4, 3), true);
        map.set(Hex::from_offset(6, 3), Terrain::Forest);
        // Une montagne : la dette immobilise l'unité, la prévision doit le savoir
        map.set_relief(Hex::from_offset(8, 3), Relief::Mountain);
        let path: Vec<Hex> = (2..11).map(|c| Hex::from_offset(c, 3)).collect();
        let expected = path_turns(&map, Hex::from_offset(1, 3), &path, 1, 0, 2);
        let mut unit = UnitState::new(UnitKind::Militia);
        unit.moves_left = 1;
        unit.path = path.clone();
        let mut pos = Hex::from_offset(1, 3);
        let mut explored = all_known(&map);
        let mut reached = Vec::new();
        for turn in 1..20 {
            let before = unit.path.len();
            unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new());
            reached.extend(std::iter::repeat_n(turn, before - unit.path.len()));
            unit.start_turn(false);
        }
        assert_eq!(reached, expected);
    }

    /// Lac vertical sur la colonne 7, des lignes `rows`, sur une carte de plaines.
    fn lake_map(rows: std::ops::Range<i32>) -> WorldMap {
        let mut map = WorldMap::filled(16, 24, Terrain::Plains);
        for row in rows {
            map.set(Hex::from_offset(7, row), Terrain::Ocean);
        }
        map
    }

    /// Joue l'ordre tour après tour ; retourne la position finale et la dernière issue.
    fn play_order(map: &WorldMap, start: Hex, dest: Hex) -> (Hex, MoveOutcome) {
        let mut explored: HashSet<Hex> = start.range(2).collect();
        let mut unit = UnitState::new(UnitKind::Militia);
        let known = KnownMap::new(map, &explored);
        let path = find_path(&known, start, dest).unwrap();
        assert!(path.iter().any(|&h| map.get(h) == Some(Terrain::Ocean)), "le trajet initial traverse le lac");
        unit.set_order(start, path, &known);
        let mut pos = start;
        let mut outcome = MoveOutcome::Moved;
        for _ in 0..30 {
            outcome = unit.advance_exploring(&mut pos, map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new());
            assert!(!map.get(pos).unwrap().is_water());
            if outcome != MoveOutcome::Moved || unit.path.is_empty() {
                break;
            }
            unit.moves_left = unit.kind.max_moves();
        }
        (pos, outcome)
    }

    /// Petit lac inconnu sur le trajet : l'unité le découvre et le contourne.
    #[test]
    fn decouverte_et_contournement() {
        let map = lake_map(1..8);
        let (start, dest) = (Hex::from_offset(2, 4), Hex::from_offset(12, 4));
        assert_eq!(play_order(&map, start, dest), (dest, MoveOutcome::Moved));
    }

    /// Grand lac : le contourner allonge le trajet de plus de 50 % → ordre suspendu.
    #[test]
    fn grand_lac_ordre_suspendu() {
        let map = lake_map(0..16);
        let (start, dest) = (Hex::from_offset(2, 4), Hex::from_offset(12, 4));
        let (pos, outcome) = play_order(&map, start, dest);
        assert_eq!(outcome, MoveOutcome::TooLong);
        assert_ne!(pos, dest);
    }

    /// Destination qui se révèle être de l'eau : l'ordre est annulé.
    #[test]
    fn destination_revelee_inaccessible() {
        let mut map = WorldMap::filled(16, 16, Terrain::Plains);
        let dest = Hex::from_offset(9, 4);
        map.set(dest, Terrain::Ocean);
        let start = Hex::from_offset(2, 4);
        let mut unit = UnitState::new(UnitKind::Militia);
        let mut pos = start;
        let mut explored = HashSet::new();
        explored.extend(start.range(2));
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, dest).unwrap(), &known);
        let mut outcomes = Vec::new();
        for _ in 0..10 {
            outcomes.push(unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new()));
            unit.moves_left = unit.kind.max_moves();
        }
        assert!(outcomes.contains(&MoveOutcome::Unreachable));
        assert!(unit.path.is_empty());
        assert!(pos.distance(dest) <= 2, "l'unité s'arrête dès qu'elle voit l'eau");
    }

    /// Cas signalé en jeu : destination au centre d'un massif montagneux fermé.
    /// Sans garde-fou, l'éclaireur fait le tour du massif en cherchant une brèche.
    #[test]
    fn massif_ferme_ordre_suspendu() {
        let mut map = WorldMap::filled(40, 30, Terrain::Plains);
        let center = Hex::from_offset(22, 15);
        for h in center.range(5).filter(|h| h.distance(center) >= 3) {
            map.set_relief(h, Relief::Peak);
        }
        let start = Hex::from_offset(8, 12);
        let mut pos = start;
        let mut explored: HashSet<Hex> = start.range(3).collect();
        let mut unit = UnitState::new(UnitKind::Scout);
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, center).unwrap(), &known);
        let budget = unit.order_budget.unwrap();

        let mut outcome = MoveOutcome::Moved;
        let mut turns = 0;
        while outcome == MoveOutcome::Moved && !unit.path.is_empty() && turns < 50 {
            outcome = unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new());
            unit.moves_left = unit.kind.max_moves();
            turns += 1;
        }
        assert_ne!(outcome, MoveOutcome::Moved, "l'ordre doit s'arrêter");
        assert!(unit.path.is_empty());
        assert!(unit.order_spent <= budget, "dépensé {} > budget {budget}", unit.order_spent);
    }

    /// Une unité alliée sur le trajet est contournée ; sur la destination, l'ordre est annulé.
    #[test]
    fn une_unite_par_case() {
        let map = WorldMap::filled(12, 12, Terrain::Plains);
        let start = Hex::from_offset(2, 5);
        let dest = Hex::from_offset(5, 5); // 3 cases ; le détour en fait 4 = les PM de la cavalerie légère
        let mut explored: HashSet<Hex> = map.hexes().collect();
        let known = KnownMap::new(&map, &explored);
        let path = find_path(&known, start, dest).unwrap();
        let obstacle = path[1];
        let occupied = HashSet::from([obstacle]);

        let mut unit = UnitState::new(UnitKind::LightCavalry); // 4 PM
        unit.set_order(start, path, &known);
        let mut pos = start;
        let outcome = unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &occupied, &HashSet::new());
        assert_eq!(outcome, MoveOutcome::Moved);
        assert_eq!(pos, dest);
        assert!(!unit.last_steps.contains(&obstacle));

        let mut unit = UnitState::new(UnitKind::LightCavalry);
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, dest).unwrap(), &known);
        let mut pos = start;
        let occupied = HashSet::from([dest]);
        let outcome = unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &occupied, &HashSet::new());
        assert_eq!(outcome, MoveOutcome::Unreachable);
        assert_ne!(pos, dest);
    }

    #[test]
    fn soins_et_debut_de_tour() {
        let mut u = UnitState::new(UnitKind::Militia);
        u.hp = 50;
        u.moves_left = 0;
        u.moved_this_turn = true;
        u.start_turn(false);
        assert_eq!(u.hp, 50, "pas de soin après un déplacement");
        assert_eq!(u.moves_left, 2);
        u.start_turn(false);
        assert_eq!(u.hp, 60);
        u.start_turn(true);
        assert_eq!(u.hp, 80, "soin renforcé en territoire ami");
        u.hp = 95;
        u.start_turn(true);
        assert_eq!(u.hp, MAX_HP);
    }

    #[test]
    fn approche_pour_attaquer() {
        let mut map = WorldMap::filled(12, 12, Terrain::Plains);
        let none = HashSet::new();
        let from = Hex::from_offset(2, 5);
        let target = Hex::from_offset(5, 5);
        // 4 PM (cavalerie légère) : 2 cases d'approche en plaine, il reste des PM pour attaquer
        let cav = UnitState::new(UnitKind::LightCavalry);
        let path = attack_approach(&map, &map, &cav, from, target, &none).unwrap();
        assert_eq!(path.last().unwrap().distance(target), 1);
        // 2 PM (milice) : arrivée à côté avec 0 PM → impossible ce tour-ci
        let militia = UnitState::new(UnitKind::Militia);
        assert!(attack_approach(&map, &map, &militia, from, target, &none).is_none());
        // Déjà adjacent : pas de déplacement
        assert_eq!(attack_approach(&map, &map, &militia, target.neighbors()[0], target, &none), Some(Vec::new()));
        // Cible entourée de montagnes : inaccessible
        for n in target.neighbors() {
            map.set_relief(n, Relief::Peak);
        }
        assert!(attack_approach(&map, &map, &cav, from, target, &none).is_none());
    }

    #[test]
    fn approche_pour_tirer() {
        let map = WorldMap::filled(14, 12, Terrain::Plains);
        let none = HashSet::new();
        let target = Hex::from_offset(8, 5);
        // Archer-épéiste à 2 cases : tire sans bouger
        let archer = UnitState::new(UnitKind::ArcherSwordsman);
        assert_eq!(attack_approach(&map, &map, &archer, Hex::from_offset(6, 5), target, &none), Some(Vec::new()));
        // À 3 cases : avance d'une case puis tire
        let path = attack_approach(&map, &map, &archer, Hex::from_offset(5, 5), target, &none).unwrap();
        assert_eq!(path.len(), 1);
        assert!(path[0].distance(target) <= 2);
        // L'archer expert ne se déplace pas pour tirer
        let expert = UnitState::new(UnitKind::ExpertArcher);
        assert!(attack_approach(&map, &map, &expert, Hex::from_offset(5, 5), target, &none).is_none());
        // Cible en forêt : portée réduite à 1
        let mut forest = map.clone();
        forest.set(target, Terrain::Forest);
        assert_ne!(attack_approach(&forest, &forest, &archer, Hex::from_offset(6, 5), target, &none), Some(Vec::new()));
    }

    #[test]
    fn zone_de_controle() {
        let map = WorldMap::filled(14, 12, Terrain::Plains);
        let enemy = Hex::from_offset(7, 5);
        let zoc = zoc_tiles(Wrap::NONE, [(UnitKind::Militia, enemy)]);
        assert_eq!(zoc.len(), 6);
        assert!(zoc_tiles(Wrap::NONE, [(UnitKind::ExpertArcher, enemy)]).is_empty(), "les archers n'en exercent pas");

        // Un déplacement qui longe l'ennemi s'arrête en entrant dans sa zone de contrôle
        let mut explored: HashSet<Hex> = map.hexes().collect();
        let mut unit = UnitState::new(UnitKind::LightCavalry); // 4 PM
        let start = Hex::from_offset(4, 4);
        let dest = Hex::from_offset(10, 4);
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, dest).unwrap(), &known);
        let mut pos = start;
        let blocked = HashSet::from([enemy]);
        unit.advance_exploring(&mut pos, &map, &mut explored, &blocked, &blocked, &zoc);
        assert!(zoc.contains(&pos), "arrêt dans la zone de contrôle");
        assert!(unit.zoc_stopped);
        assert!(unit.moves_left > 0, "il reste des PM pour attaquer");
        assert!(!unit.path.is_empty(), "l'ordre reprendra au tour suivant");

        // L'approche pour attaquer ne traverse pas la zone de contrôle d'un autre ennemi
        let target = Hex::from_offset(10, 5);
        let cav = UnitState::new(UnitKind::LightCavalry);
        let path = attack_approach(&map, &map, &cav, Hex::from_offset(5, 5), target, &zoc);
        if let Some(path) = path {
            assert!(path.iter().rev().skip(1).all(|h| !zoc.contains(h)));
        }
    }

    #[test]
    fn budget_de_detour() {
        let map = WorldMap::filled(10, 10, Terrain::Plains);
        let mut unit = UnitState::new(UnitKind::Militia);
        let path: Vec<Hex> = (1..5).map(|c| Hex::from_offset(c, 1)).collect();
        unit.set_order(Hex::from_offset(0, 1), path, &map); // coût 4 → budget 4 + 2 = 6
        assert_eq!(unit.order_budget, Some(6));
        unit.clear_order();
        assert_eq!(unit.order_budget, None);
    }

    #[test]
    fn fondation_distance_minimale() {
        let map = WorldMap::filled(20, 20, Terrain::Plains);
        let a = Hex::from_offset(5, 5);
        assert!(can_found_city(&map, a, &[]).is_ok());
        assert!(can_found_city(&map, Hex::from_offset(7, 5), &[a]).is_err());
        assert!(can_found_city(&map, Hex::from_offset(10, 5), &[a]).is_ok());
    }

    #[test]
    fn croissance_et_production() {
        let mut city = CityState::new("Test".into());
        // 6 nourriture => surplus 4 ; 5 production
        let y = Yields::new(6, 5, 2);
        let mut grew_at = None;
        let mut built = Vec::new();
        for turn in 1..=10 {
            let r = city.process_turn(y);
            if r.grew && grew_at.is_none() {
                grew_at = Some(turn);
            }
            built.extend(r.built);
        }
        assert_eq!(grew_at, Some(4)); // 4 tours × 4 = 16 >= 15
        // 50 de production sur 10 tours, milice à 15 : une unité tous les 3 tours
        assert_eq!(built, vec![UnitKind::Militia; 3]);
    }

    #[test]
    fn colon_exige_deux_habitants() {
        let mut city = CityState::new("Test".into());
        city.producing = Some(UnitKind::Settler);
        city.production_stock = 100;
        assert_eq!(city.process_turn(Yields::new(2, 0, 0)).built, None);
        city.population = 2;
        assert_eq!(city.process_turn(Yields::new(4, 0, 0)).built, Some(UnitKind::Settler));
        assert_eq!(city.population, 1);
    }

    #[test]
    fn choix_des_cases() {
        let mut map = WorldMap::filled(10, 10, Terrain::Desert);
        let center = Hex::from_offset(5, 5);
        let best = center.neighbors()[2];
        map.set(best, Terrain::Grassland);
        let territory: Vec<Hex> = center.range(1).collect();
        let worked = choose_worked_tiles(&map, center, 1, &territory, &HashSet::new());
        assert_eq!(worked, vec![best]);
        let taken = HashSet::from([best]);
        let worked = choose_worked_tiles(&map, center, 1, &territory, &taken);
        assert_ne!(worked, vec![best]);
    }
}

#[cfg(test)]
mod experience_brouillard {
    use super::*;
    use crate::map::{Relief, Terrain};

    /// Massif montagneux en anneau (rayon 3 à 5) autour d'une cuvette ; `gap` ouvre une brèche.
    fn massif(gap: bool) -> (WorldMap, Hex) {
        let mut map = WorldMap::filled(40, 30, Terrain::Plains);
        let center = Hex::from_offset(22, 15);
        for h in center.range(5) {
            let d = h.distance(center);
            if (3..=5).contains(&d) {
                map.set_relief(h, Relief::Peak);
            }
        }
        if gap {
            // brèche côté est (opposé à l'éclaireur)
            for d in 3..=5 {
                map.set(center + Hex::new(d, 0), Terrain::Plains);
            }
        }
        (map, center)
    }

    fn run(cost_unknown: u32, gap: bool) -> (usize, bool, Hex) {
        // On simule un coût d'inconnu différent via un wrapper
        struct K<'a> { map: &'a WorldMap, explored: &'a HashSet<Hex>, c: u32 }
        impl MoveCosts for K<'_> {
            fn move_cost(&self, h: Hex) -> Option<u32> {
                if !self.map.contains(h) { None } else if self.explored.contains(&h) { self.map.move_cost(h) } else { Some(self.c) }
            }
        }
        let (map, center) = massif(gap);
        let start = Hex::from_offset(8, 10);
        let mut pos = start;
        let mut explored: HashSet<Hex> = start.range(3).collect();
        let mut steps = 0;
        let mut path = find_path(&K { map: &map, explored: &explored, c: cost_unknown }, pos, center).unwrap();
        for _ in 0..200 {
            if path.is_empty() || pos == center { break; }
            match find_path(&K { map: &map, explored: &explored, c: cost_unknown }, pos, center) {
                None => return (steps, true, pos),
                Some(p) => path = p,
            }
            pos = path.remove(0);
            steps += 1;
            explored.extend(pos.range(3));
        }
        (steps, false, pos)
    }

    #[test]
    fn comparer() {
        for gap in [false, true] {
            for c in [1, 2, 3] {
                let (steps, blocked, pos) = run(c, gap);
                println!("brèche={gap} coût_inconnu={c} : {steps} pas, bloqué={blocked}, arrivé={}", !blocked && steps < 200 && pos == massif(gap).1);
            }
        }
    }
}
