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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// Sur le pont d'une case de fleuve (entrée par un de ses bouts) plutôt que dans l'eau.
    pub on_bridge: bool,
    /// Destination d'un ordre sur une case de fleuve avec pont : sur le pont (`Some(true)`) ou dans
    /// l'eau (`Some(false)`) ; `None` : selon le côté d'où l'on arrive.
    pub order_layer: Option<bool>,
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
            on_bridge: false,
            order_layer: None,
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
        self.order_layer = None;
        self.explore_target = None;
        self.order_budget = None;
        self.order_spent = 0;
    }

    /// Paie un pas : PM restants, et dette pour le terrain qui dépasse les PM qu'il restait.
    pub fn pay(&mut self, step: StepCost) {
        self.on_bridge = step.onto_bridge;
        let (left, debt) = step.spend(self.moves_left);
        self.moves_left = left;
        self.move_debt += debt;
    }

    /// Tours avant de pouvoir repartir quand la dette immobilise l'unité au moins un tour entier
    /// (0 sinon) : compte à rebours qui baisse de 1 à chaque tour, celui de l'entrée en montagne compris.
    pub fn stuck_turns(&self) -> u32 {
        let max = self.kind.max_moves().max(1);
        if self.immobilized || self.move_debt >= max { 1 + self.move_debt / max } else { 0 }
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
        explored: &mut crate::cases::CaseSet,
        blocked: &HashSet<Hex>,
        occupied: &HashSet<Hex>,
        zoc: &HashSet<Hex>,
        allies: &HashSet<Hex>,
    ) -> MoveOutcome {
        let sight = self.kind.sight();
        self.last_steps.clear();
        // Avant d'entrer sur la case d'un allié : l'état à retrouver si l'unité devait s'y arrêter
        // (une unité par case : on ne fait que traverser les alliés, dans le même tour)
        let mut avant_allie: Option<(UnitState, Hex, usize)> = None;
        explored.extend(map.range(*pos, sight));
        // Cases bloquées connues + cases occupées découvertes en route (une unité par case)
        let mut blocked = blocked.clone();
        while self.moves_left > 0 && !self.zoc_stopped {
            let Some(&destination) = self.path.last() else { break };
            let known = ForUnit::new(KnownMap::new(map, explored).blocking(&blocked), self.kind);
            // Pont d'un fleuve : on n'en sort que par ses deux bouts, et on n'y monte que par eux ;
            // l'étape intermédiaire est le bout le plus proche du but
            let ends = |h: Hex| map.fleuve_bridge(h).map(|a| [map.wrap().neighbor(h, a), map.wrap().neighbor(h, a + 3)]);
            let on_bridge_here = self.on_bridge && ends(*pos).is_some();
            let goal = match (ends(*pos), ends(destination)) {
                (Some(e), _) if on_bridge_here && !e.contains(&destination) && destination != *pos => {
                    *e.iter().min_by_key(|&&x| map.distance(x, destination)).unwrap()
                }
                (_, Some(e)) if self.order_layer == Some(true) && !e.contains(pos) && *pos != destination => {
                    *e.iter().min_by_key(|&&x| map.distance(x, *pos)).unwrap()
                }
                _ => destination,
            };
            let Some(mut path) = find_path_turns(&known, *pos, goal, self.moves_left, self.move_debt, self.kind.max_moves()) else {
                self.clear_order();
                return MoveOutcome::Unreachable;
            };
            if goal != destination {
                path.push(destination);
            }
            // Garde-fou : le trajet total (déjà parcouru + reste estimé) ne doit pas exploser
            if let Some(budget) = self.order_budget
                && self.order_spent + path_cost(&known, *pos, &path) > budget
            {
                self.clear_order();
                return MoveOutcome::TooLong;
            }
            self.path = path;
            let next = self.path[0];
            // Sur le pont ou dans l'eau : par un bout du pont, sur le pont, sauf si l'ordre vise l'eau
            let last = self.path.len() == 1;
            let onto = map.bridge_end(next, *pos) && !(last && self.order_layer == Some(false));
            // La case suivante est voisine, donc déjà révélée (vision ≥ 1) : son coût est le vrai.
            let Some(mut step) = ForUnit::new(map, self.kind).step(*pos, next) else {
                self.clear_order();
                return MoveOutcome::Unreachable;
            };
            if step.onto_bridge && !onto {
                // (descendre dans l'eau depuis le bout du pont : entrée dans le fleuve)
                step = map.enter_cost(next).unwrap_or_default();
            }
            // Case suivante occupée (elle est voisine, donc visible). Un allié se traverse s'il reste
            // des PM pour repartir vers une case libre, hors zone de contrôle ; sinon on recalcule
            // le trajet en évitant la case.
            let place = crate::map::slot(next, onto);
            if occupied.contains(&place) {
                let traversable = allies.contains(&place)
                    && step.spend(self.moves_left).0 > 0
                    && !zoc.contains(&next)
                    && self.path.get(1).is_some_and(|&apres| !occupied.contains(&crate::map::slot(apres, false)) && !occupied.contains(&crate::map::slot(apres, true)));
                if !traversable {
                    blocked.insert(next);
                    continue;
                }
                if avant_allie.is_none() {
                    avant_allie = Some((self.clone(), *pos, self.last_steps.len()));
                }
            } else {
                avant_allie = None;
            }
            self.path.remove(0);
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
        // Arrêté sur la case d'un allié (rien n'a permis d'en repartir) : retour avant d'y entrer
        if let Some((etat, case, _)) = avant_allie
            && (allies.contains(&crate::map::slot(*pos, self.on_bridge)) || occupied.contains(&crate::map::slot(*pos, self.on_bridge)))
        {
            *self = etat;
            *pos = case;
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

// ---------------------------------------------------------------- Population de l'aire

/// Habitants emportés par un colon ; une ville en forme un à partir du double.
pub const HABITANTS_COLON: u64 = 1_000;
/// Habitants nourris par un point de nourriture.
pub const HABITANTS_PAR_NOURRITURE: f64 = 1_000.0;
/// Croissance naturelle par tour d'une aire bien nourrie (1,5 %).
pub const CROISSANCE_NATURELLE: f64 = 0.015;
/// Surplus de nourriture (part du besoin) qui donne toute la croissance naturelle.
pub const SURPLUS_POUR_CROITRE: f64 = 0.5;
/// Perte par tour en famine : de 2 % (léger manque) à 5 % (plus rien à manger).
pub const FAMINE_MIN: f64 = 0.02;
pub const FAMINE_MAX: f64 = 0.05;
/// Population en dessous de laquelle une famine ne fait plus baisser une ville.
pub const HABITANTS_PLANCHER: u64 = 500;
/// Palier le plus haut de la table.
pub const PALIER_MAX: i32 = 30;
/// Paliers repères et habitants de toute l'aire (ville et campagnes), validés le 11/11/2026
/// (`docs/conception/conception-echelle-et-but.md`) ; les autres paliers s'en déduisent.
const REPERES: [(i32, f64); 8] = [(1, 1_000.0), (3, 3_000.0), (5, 10_000.0), (10, 50_000.0), (15, 150_000.0), (20, 400_000.0), (25, 1_000_000.0), (30, 2_500_000.0)];

/// Habitants de l'aire au début d'un palier (progression géométrique entre les repères).
pub fn habitants_du_palier(palier: i32) -> u64 {
    let p = palier.clamp(1, PALIER_MAX);
    let (mut a, mut b) = (REPERES[0], REPERES[1]);
    for w in REPERES.windows(2) {
        if p >= w[0].0 && p <= w[1].0 {
            (a, b) = (w[0], w[1]);
            break;
        }
    }
    let t = f64::from(p - a.0) / f64::from(b.0 - a.0);
    let h = a.1 * (b.1 / a.1).powf(t);
    // Arrondi à la centaine (au millier au-delà de 100 000)
    let pas = if h >= 100_000.0 { 1_000.0 } else { 100.0 };
    ((h / pas).round() * pas) as u64
}

/// Palier d'une aire d'après ses habitants (1 au moins).
pub fn palier_de(habitants: u64) -> i32 {
    (1..=PALIER_MAX).rev().find(|&p| habitants >= habitants_du_palier(p)).unwrap_or(1)
}

/// Nombre écrit à la française, avec une espace fine entre les milliers (« 12 400 »).
pub fn milliers(n: u64) -> String {
    let chiffres = n.to_string();
    let mut out = String::new();
    for (i, c) in chiffres.chars().enumerate() {
        if i > 0 && (chiffres.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// Variation des habitants en un tour selon la nourriture produite : croissance naturelle quand
/// la nourriture dépasse le besoin (pleine à 50 % de surplus), famine quand elle manque.
pub fn variation_des_habitants(habitants: u64, nourriture: i32) -> i64 {
    let besoin = (habitants as f64 / HABITANTS_PAR_NOURRITURE).max(0.001);
    let couverture = f64::from(nourriture.max(0)) / besoin;
    let taux = if couverture >= 1.0 {
        CROISSANCE_NATURELLE * ((couverture - 1.0) / SURPLUS_POUR_CROITRE).min(1.0)
    } else {
        -(FAMINE_MIN + (FAMINE_MAX - FAMINE_MIN) * (1.0 - couverture))
    };
    (habitants as f64 * taux).round() as i64
}

pub fn can_found_city(map: &WorldMap, at: Hex, cities: &[Hex]) -> Result<(), &'static str> {
    if map.is_ice(at) {
        return Err("pas de ville sur la banquise");
    }
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
    Yields::new(y.food.max(2), y.production.max(2), y.economie.max(1))
}

fn tile_score(y: Yields) -> i32 {
    y.food * 4 + y.production * 3 + y.economie * 2
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

/// Coût d'un pont (production) : de `BRIDGE_COST.0` sur la plus petite rivière (`RIVER_M3S`) à
/// `BRIDGE_COST.1` sur les plus grands fleuves (`BRIDGE_COST_TOP_M3S`, l'Amazone), avec le
/// logarithme du débit (choix de Benjamin, 08/11/2026), pour un pont de bois ; `BRIDGE_STONE_COST`
/// fois plus pour un pont de pierre (09/11/2026). Arrondi à 5.
pub const BRIDGE_COST: (i32, i32) = (20, 120);
pub const BRIDGE_STONE_COST: f32 = 1.5;
pub const BRIDGE_COST_TOP_M3S: f32 = 100_000.0;

/// Coût de production d'un pont, selon le débit du cours d'eau qu'il franchit.
pub fn bridge_cost(map: &WorldMap, at: crate::map::BridgeAt) -> i32 {
    let q = match at {
        crate::map::BridgeAt::Edge { hex, dir } => map.river_flow(hex, usize::from(dir)),
        crate::map::BridgeAt::Fleuve { hex, .. } => map.discharge(hex),
    };
    let lo = crate::hydro::RIVER_M3S;
    let share = ((q.max(lo) / lo).ln() / (BRIDGE_COST_TOP_M3S / lo).ln()).clamp(0.0, 1.0);
    let cost = BRIDGE_COST.0 as f32 + (BRIDGE_COST.1 - BRIDGE_COST.0) as f32 * share;
    let cost = if map.bridge_stone(at) { cost * BRIDGE_STONE_COST } else { cost };
    ((cost / 5.0).round() * 5.0) as i32
}

/// Emplacements de pont d'un territoire (`owned` : la case est au territoire) : toutes les arêtes
/// de rivière dont une case au moins en est, toutes ses cases de fleuve (d'une rive à l'autre ;
/// chaque traversée possible), sauf là où un pont existe déjà.
pub fn bridge_spots(map: &WorldMap, owned: impl Fn(Hex) -> bool, cells: impl IntoIterator<Item = Hex>) -> Vec<crate::map::BridgeAt> {
    let mut spots: Vec<crate::map::BridgeAt> = Vec::new();
    for h in cells {
        let h = map.canonical(h);
        if !owned(h) {
            continue;
        }
        let dirs = if map.is_fleuve(h) { 0..3 } else { 0..6 };
        if map.is_fleuve(h) && map.fleuve_bridge(h).is_some() {
            continue;
        }
        for d in dirs {
            if let Some(at) = map.bridge_site(h, d)
                && !map.bridges().any(|b| b == at)
                && !spots.contains(&at)
            {
                spots.push(at);
            }
        }
    }
    spots
}

/// Ce que produit une ville : une unité, ou un pont à un emplacement choisi (au coût fixé au choix).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Production {
    Unit(UnitKind),
    Bridge { at: crate::map::BridgeAt, cost: i32 },
}

impl Production {
    pub fn cost(self) -> i32 {
        match self {
            Production::Unit(kind) => kind.cost(),
            Production::Bridge { cost, .. } => cost,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Production::Unit(kind) => kind.name(),
            Production::Bridge { at: crate::map::BridgeAt::Edge { .. }, .. } => "Pont sur la rivière",
            Production::Bridge { at: crate::map::BridgeAt::Fleuve { .. }, .. } => "Pont sur le fleuve",
        }
    }

}

/// État économique d'une ville.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CityState {
    pub name: String,
    /// Habitants de toute l'aire de la ville (ville et campagnes).
    pub habitants: u64,
    pub production_stock: i32,
    pub producing: Option<Production>,
    /// Ponts construits par la ville (une tour garde leur autre bout si elle est fortifiée).
    pub bridges: Vec<crate::map::BridgeAt>,
    /// Présence d'une fortification : la muraille n'existe que si elle est vraie. La règle de jeu
    /// reste à concevoir ; pour l'instant, seule la console de debug la pose.
    pub fortified: bool,
}

/// Ce qui s'est passé dans une ville pendant la fin de tour.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CityTurnReport {
    pub grew: bool,
    pub starved: bool,
    pub built: Option<Production>,
    pub economie: i32,
}

impl CityState {
    pub fn new(name: String) -> Self {
        Self { name, habitants: HABITANTS_COLON, production_stock: 0, producing: Some(Production::Unit(UnitKind::Militia)), bridges: Vec::new(), fortified: false }
    }

    /// Consommation : 2 nourriture par habitant.
    /// Palier de la ville (ce que voit le joueur et ce que lisent les règles de taille).
    pub fn palier(&self) -> i32 {
        palier_de(self.habitants)
    }

    /// Nourriture nécessaire à la population de l'aire ce tour-ci.
    pub fn besoin_nourriture(&self) -> f64 {
        self.habitants as f64 / HABITANTS_PAR_NOURRITURE
    }

    pub fn process_turn(&mut self, yields: Yields) -> CityTurnReport {
        let mut report = CityTurnReport { economie: yields.economie, ..Default::default() };

        // Croissance (ou famine) en habitants ; « a grandi » : nouveau palier atteint
        let palier = self.palier();
        let variation = variation_des_habitants(self.habitants, yields.food);
        if variation < 0 {
            report.starved = true;
            self.habitants = self.habitants.saturating_sub(variation.unsigned_abs()).max(HABITANTS_PLANCHER.min(self.habitants));
        } else {
            self.habitants += variation as u64;
        }
        report.grew = self.palier() > palier;

        // Production
        if let Some(item) = self.producing {
            self.production_stock += yields.production;
            let settler = item == Production::Unit(UnitKind::Settler);
            let pop_ok = !settler || self.habitants >= 2 * HABITANTS_COLON;
            if self.production_stock >= item.cost() && pop_ok {
                self.production_stock -= item.cost();
                if settler {
                    self.habitants -= HABITANTS_COLON;
                }
                report.built = Some(item);
                // (un pont n'est construit qu'une fois : la ville revient à la milice)
                if matches!(item, Production::Bridge { .. }) {
                    self.producing = Some(Production::Unit(UnitKind::Militia));
                }
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

    fn all_known(map: &WorldMap) -> crate::cases::CaseSet {
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
        assert_eq!(u.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new(), &HashSet::new()), MoveOutcome::Moved);
        assert_eq!(pos, Hex::from_offset(3, 2));
        assert_eq!(u.moves_left, 0);
        assert_eq!(u.path, vec![Hex::from_offset(4, 2)]);
    }

    #[test]
    fn tours_sur_un_chemin() {
        let mut map = WorldMap::filled(12, 12, Terrain::Plains);
        let path: Vec<Hex> = (2..8).map(|c| Hex::from_offset(c, 2)).collect();
        let from = Hex::from_offset(1, 2);
        // 6 cases de plaine à 4 PM/tour : 4 cases par tour
        assert_eq!(path_turns(&map, from, &path, 4, 0, 4), vec![1, 1, 1, 1, 2, 2]);
        // unité ayant déjà bougé (0 PM) : départ au tour suivant
        assert_eq!(path_turns(&map, from, &path, 0, 0, 4), vec![2, 2, 2, 2, 3, 3]);
        // une forêt (4 PM) avec 3 PM restants reste accessible, mais le PM qui manque est dû au tour suivant
        map.set(path[1], Terrain::Forest);
        assert_eq!(path_turns(&map, from, &path, 4, 0, 4), vec![1, 1, 2, 2, 2, 3]);
        map.set(path[0], Terrain::Forest);
        assert_eq!(path_turns(&map, from, &path, 4, 0, 4), vec![1, 2, 3, 3, 3, 3]);
        // dette de départ : 6 PM dus, à 4 PM par tour → immobile au tour 2, 2 PM au tour 3
        let plain = WorldMap::filled(12, 12, Terrain::Plains);
        assert_eq!(path_turns(&plain, from, &path, 0, 6, 4), vec![3, 3, 4, 4, 4, 4]);
    }

    /// Fleuve avec pont : deux niveaux, le pont (par ses bouts) ou l'eau, au choix de l'ordre.
    #[test]
    fn pont_ou_eau_sur_un_fleuve() {
        use crate::map::{BridgeAt, FLEUVE_MOVE_COST, slot};
        let mut map = WorldMap::filled(10, 10, Terrain::Plains);
        let f = Hex::from_offset(5, 5);
        map.set_fleuve(f);
        map.set_bridge(BridgeAt::Fleuve { hex: f, axis: 0 }, true);
        let (west, east) = (f + crate::hex::DIRECTIONS[3], f + crate::hex::DIRECTIONS[0]);
        let side = f + crate::hex::DIRECTIONS[1];
        let mut explored: crate::cases::CaseSet = map.hexes().collect();
        let go = |unit: &mut UnitState, from: &mut Hex, to: Hex, layer: Option<bool>, occupied: &HashSet<Hex>, explored: &mut crate::cases::CaseSet| {
            unit.moves_left = unit.kind.max_moves();
            unit.set_order(*from, vec![to], &map);
            unit.order_layer = layer;
            unit.advance_exploring(from, &map, explored, &HashSet::new(), occupied, &HashSet::new(), &HashSet::new())
        };
        // Par un bout : sur le pont, pour 1 PM ; l'unité peut continuer
        let mut unit = UnitState::new(UnitKind::Militia);
        let mut at = west;
        go(&mut unit, &mut at, f, None, &HashSet::new(), &mut explored);
        assert!(at == f && unit.on_bridge && unit.moves_left == unit.kind.max_moves() - 1);
        // Depuis le pont, vers une case qui n'est pas un bout : on passe par un bout
        go(&mut unit, &mut at, side, None, &HashSet::new(), &mut explored);
        assert!(unit.last_steps.first().is_some_and(|h| *h == east || *h == west), "{:?}", unit.last_steps);
        assert!(!unit.on_bridge);
        // Par un bout, mais l'ordre vise l'eau : dans le fleuve (6 PM, arrêt)
        let mut unit = UnitState::new(UnitKind::Militia);
        let mut at = west;
        go(&mut unit, &mut at, f, Some(false), &HashSet::new(), &mut explored);
        assert!(at == f && !unit.on_bridge && unit.moves_left == 0);
        assert_eq!(FLEUVE_MOVE_COST, 6);
        // Le pont occupé : l'unité peut aller dans l'eau dessous (avec 4 PM, sans consigne, elle
        // viserait le pont par un bout, moins cher ; l'ordre vise donc l'eau)
        let taken = HashSet::from([slot(f, true)]);
        let mut unit = UnitState::new(UnitKind::Militia);
        let mut at = side;
        go(&mut unit, &mut at, f, Some(false), &taken, &mut explored);
        assert!(at == f && !unit.on_bridge, "dans l'eau, sous le pont occupé");
        // Viser le pont depuis le côté : on monte par un bout
        let mut unit = UnitState::new(UnitKind::Scout);
        let mut at = side;
        for _ in 0..3 {
            if at == f {
                break;
            }
            go(&mut unit, &mut at, f, Some(true), &HashSet::new(), &mut explored);
        }
        assert!(at == f && unit.on_bridge, "sur le pont, arrivé par un bout");
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
        assert_eq!((unit.moves_left, unit.move_debt), (0, MOUNTAIN_MOVE_MIN - 4));
        assert_eq!(unit.stuck_turns(), 2, "8 PM à 4 PM par tour : un tour immobilisé, repart dans 2 tours");
        unit.start_turn(false);
        assert!(unit.immobilized && unit.moves_left == 0 && unit.stuck_turns() == 1);
        unit.start_turn(false);
        assert!(!unit.immobilized && unit.moves_left == 4 && unit.stuck_turns() == 0);
        // Rivière et fleuve : arrêt sans dette
        let river = StepCost { terrain: 1, water: 2, ..StepCost::default() };
        assert_eq!(river.spend(1), (0, 0));
        // Cavalerie : double ; éclaireur : deux tiers
        let cav = ForUnit::new(&map, UnitKind::LightCavalry).step(a, b).unwrap();
        let scout = ForUnit::new(&map, UnitKind::Scout).step(a, b).unwrap();
        assert_eq!((cav.terrain, scout.terrain), (2 * MOUNTAIN_MOVE_MIN, 6));
    }

    /// La pastille d'unité immobilisée baisse de 1 à chaque tour jusqu'au départ, quelle que soit la dette.
    #[test]
    fn compte_a_rebours_immobilisation() {
        for debt in 0..12 {
            let mut unit = UnitState::new(UnitKind::Militia);
            (unit.moves_left, unit.move_debt) = (0, debt);
            let mut counts = vec![unit.stuck_turns()];
            while unit.moves_left == 0 || counts.len() == 1 {
                unit.start_turn(false);
                counts.push(unit.stuck_turns());
            }
            // Tours de jeu jusqu'à celui où l'unité a de nouveau des PM
            let wait = counts.len() as u32 - 1;
            let expected: Vec<u32> = if wait >= 2 { (0..=wait).rev().collect() } else { vec![0; counts.len()] };
            assert_eq!(counts, expected, "dette de {debt} PM");
        }
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
        assert_eq!(mountain_move_cost(2_600.0), MOUNTAIN_MOVE_MIN + 4);
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
        let expected = path_turns(&map, Hex::from_offset(1, 3), &path, 1, 0, UnitKind::Militia.max_moves());
        let mut unit = UnitState::new(UnitKind::Militia);
        unit.moves_left = 1;
        unit.path = path.clone();
        let mut pos = Hex::from_offset(1, 3);
        let mut explored = all_known(&map);
        let mut reached = Vec::new();
        for turn in 1..20 {
            let before = unit.path.len();
            unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new(), &HashSet::new());
            reached.extend(std::iter::repeat_n(turn, before - unit.path.len()));
            unit.start_turn(false);
        }
        assert_eq!(reached, expected);
    }

    /// Les alliés se traversent dans le même tour, jamais pour s'y arrêter.
    #[test]
    fn traverser_un_allie() {
        let map = corridor(12, 3);
        let allie = Hex::from_offset(3, 3);
        let allies = HashSet::from([crate::map::slot(allie, false)]);
        let aller = |moves: u32| {
            let mut unit = UnitState::new(UnitKind::Militia);
            unit.moves_left = moves;
            let mut pos = Hex::from_offset(1, 3);
            unit.set_order(pos, (2..9).map(|c| Hex::from_offset(c, 3)).collect(), &map);
            let mut explored = all_known(&map);
            unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &allies, &HashSet::new(), &allies);
            pos
        };
        // 4 PM : on passe l'allié et on continue au-delà
        assert_eq!(aller(4), Hex::from_offset(5, 3));
        // 2 PM : on arriverait sur l'allié sans PM pour en repartir : arrêt avant lui
        assert_eq!(aller(2), Hex::from_offset(2, 3));
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
        let mut explored: crate::cases::CaseSet = start.range(2).collect();
        let mut unit = UnitState::new(UnitKind::Militia);
        let known = KnownMap::new(map, &explored);
        let path = find_path(&known, start, dest).unwrap();
        assert!(path.iter().any(|&h| map.get(h) == Some(Terrain::Ocean)), "le trajet initial traverse le lac");
        unit.set_order(start, path, &known);
        let mut pos = start;
        let mut outcome = MoveOutcome::Moved;
        for _ in 0..30 {
            outcome = unit.advance_exploring(&mut pos, map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new(), &HashSet::new());
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
        let mut explored = crate::cases::CaseSet::new();
        explored.extend(start.range(2));
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, dest).unwrap(), &known);
        let mut outcomes = Vec::new();
        for _ in 0..10 {
            outcomes.push(unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new(), &HashSet::new()));
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
        let mut explored: crate::cases::CaseSet = start.range(3).collect();
        let mut unit = UnitState::new(UnitKind::Scout);
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, center).unwrap(), &known);
        let budget = unit.order_budget.unwrap();

        let mut outcome = MoveOutcome::Moved;
        let mut turns = 0;
        while outcome == MoveOutcome::Moved && !unit.path.is_empty() && turns < 50 {
            outcome = unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &HashSet::new(), &HashSet::new(), &HashSet::new());
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
        let mut explored: crate::cases::CaseSet = map.hexes().collect();
        let known = KnownMap::new(&map, &explored);
        let path = find_path(&known, start, dest).unwrap();
        let obstacle = path[1];
        let occupied = HashSet::from([obstacle]);

        let mut unit = UnitState::new(UnitKind::LightCavalry); // 4 PM
        unit.set_order(start, path, &known);
        let mut pos = start;
        let outcome = unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &occupied, &HashSet::new(), &HashSet::new());
        assert_eq!(outcome, MoveOutcome::Moved);
        assert_eq!(pos, dest);
        assert!(!unit.last_steps.contains(&obstacle));

        let mut unit = UnitState::new(UnitKind::LightCavalry);
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, dest).unwrap(), &known);
        let mut pos = start;
        let occupied = HashSet::from([dest]);
        let outcome = unit.advance_exploring(&mut pos, &map, &mut explored, &HashSet::new(), &occupied, &HashSet::new(), &HashSet::new());
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
        assert_eq!(u.moves_left, 4);
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
        let target = Hex::from_offset(7, 5);
        // 8 PM (cavalerie légère) : 4 cases d'approche en plaine, il reste des PM pour attaquer
        let cav = UnitState::new(UnitKind::LightCavalry);
        let path = attack_approach(&map, &map, &cav, from, target, &none).unwrap();
        assert_eq!(path.last().unwrap().distance(target), 1);
        // 4 PM (milice) : arrivée à côté avec 0 PM → impossible ce tour-ci
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
        let mut explored: crate::cases::CaseSet = map.hexes().collect();
        let mut unit = UnitState::new(UnitKind::LightCavalry); // 4 PM
        let start = Hex::from_offset(4, 4);
        let dest = Hex::from_offset(10, 4);
        let known = KnownMap::new(&map, &explored);
        unit.set_order(start, find_path(&known, start, dest).unwrap(), &known);
        let mut pos = start;
        let blocked = HashSet::from([enemy]);
        unit.advance_exploring(&mut pos, &map, &mut explored, &blocked, &blocked, &zoc, &HashSet::new());
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
        // Banquise : on y marche, mais pas de ville
        let mut frozen = WorldMap::filled(8, 8, Terrain::Ocean);
        let ice = Hex::from_offset(3, 3);
        frozen.freeze(ice);
        assert_eq!(can_found_city(&frozen, ice, &[]), Err("pas de ville sur la banquise"));
        assert!(can_found_city(&map, Hex::from_offset(7, 5), &[a]).is_err());
        assert!(can_found_city(&map, Hex::from_offset(10, 5), &[a]).is_ok());
    }

    #[test]
    fn paliers_de_la_table() {
        // Les repères validés
        for (p, h) in [(1, 1_000), (3, 3_000), (5, 10_000), (10, 50_000), (15, 150_000), (20, 400_000), (25, 1_000_000), (30, 2_500_000)] {
            assert_eq!(habitants_du_palier(p), h, "palier {p}");
            assert_eq!(palier_de(h), p);
        }
        // Croissants, et le palier d'une valeur intermédiaire est celui d'en dessous
        for p in 1..PALIER_MAX {
            assert!(habitants_du_palier(p + 1) > habitants_du_palier(p), "palier {p}");
        }
        assert_eq!(palier_de(600), 1);
        assert_eq!(palier_de(49_999), 9);
        assert_eq!(palier_de(10_000_000), PALIER_MAX);
    }

    #[test]
    fn croissance_famine_et_production() {
        // 1 000 habitants mangent 1 : avec 1,5 de nourriture (50 % de surplus), pleine croissance
        assert_eq!(variation_des_habitants(1_000, 2), 15);
        assert_eq!(variation_des_habitants(10_000, 15), 150);
        // Juste le besoin : pas de croissance ; un surplus de 25 % : la moitié
        assert_eq!(variation_des_habitants(10_000, 10), 0);
        assert_eq!(variation_des_habitants(20_000, 25), 150);
        // Famine : 2 % pour un léger manque, 5 % sans rien
        assert_eq!(variation_des_habitants(10_000, 0), -500);
        assert!((-260..=-200).contains(&variation_des_habitants(10_000, 9)));
        let mut city = CityState::new("Test".into());
        assert_eq!(city.habitants, HABITANTS_COLON);
        let y = Yields::new(6, 5, 2);
        let mut built = Vec::new();
        let mut grew = 0;
        for _ in 0..10 {
            let r = city.process_turn(y);
            grew += usize::from(r.grew);
            built.extend(r.built);
        }
        // 1,5 % par tour : 1 000 → ≈ 1 160 en 10 tours (palier 1 encore)
        assert!((1_150..=1_170).contains(&city.habitants), "{}", city.habitants);
        assert_eq!(grew, 0);
        // 50 de production sur 10 tours, milice à 15 : une unité tous les 3 tours
        assert_eq!(built, vec![Production::Unit(UnitKind::Militia); 3]);
        // Une famine ne descend pas sous le plancher
        let mut affame = CityState::new("Faim".into());
        for _ in 0..100 {
            affame.process_turn(Yields::new(0, 0, 0));
        }
        assert_eq!(affame.habitants, HABITANTS_PLANCHER);
    }

    #[test]
    fn colon_exige_deux_mille_habitants() {
        let mut city = CityState::new("Test".into());
        city.producing = Some(Production::Unit(UnitKind::Settler));
        city.production_stock = 100;
        assert_eq!(city.process_turn(Yields::new(1, 0, 0)).built, None);
        city.habitants = 2 * HABITANTS_COLON;
        assert_eq!(city.process_turn(Yields::new(2, 0, 0)).built, Some(Production::Unit(UnitKind::Settler)));
        assert_eq!(city.habitants, HABITANTS_COLON);
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
        struct K<'a> { map: &'a WorldMap, explored: &'a crate::cases::CaseSet, c: u32 }
        impl MoveCosts for K<'_> {
            fn move_cost(&self, h: Hex) -> Option<u32> {
                if !self.map.contains(h) { None } else if self.explored.contains(&h) { self.map.move_cost(h) } else { Some(self.c) }
            }
        }
        let (map, center) = massif(gap);
        let start = Hex::from_offset(8, 10);
        let mut pos = start;
        let mut explored: crate::cases::CaseSet = start.range(3).collect();
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
