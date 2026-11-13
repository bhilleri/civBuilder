//! Règles du jeu indépendantes du moteur : unités, villes, croissance, production.

use crate::hex::{Hex, Wrap};
use crate::map::{KnownMap, MoveCosts, StepCost, WorldMap, Yields, turn_moves};
use crate::combat::{self, AttackKind};
use crate::pathfinding::{find_path, find_path_turns};
use crate::intendant::{self, Amenagement, CaseDispo, Consignes, Rendements, Repartition};
use bevy::ecs::entity::Entity;
use bevy::platform::collections::{HashMap, HashSet};

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

// ---------------------------------------------------------------- Territoire par la Culture (4d)

/// Culture produite par toute ville, par tour (+2 avec un sanctuaire, quand il existera).
pub const CULTURE_PAR_TOUR: f64 = 1.0;
/// Projet « Développement de l'art et de la culture » : culture par point de production (chiffre
/// proposé, à régler en jouant).
pub const CULTURE_PAR_PRODUCTION: f64 = 0.2;

/// Stock de culture qui fait gagner une case quand la ville en a déjà gagné `gagnees` :
/// 10 + 6 × `gagnees` (conception des villes, 4d).
pub fn seuil_culture(gagnees: u32) -> f64 {
    10.0 + 6.0 * gagnees as f64
}

/// Prix d'achat d'une case, en deniers : 20 + 10 × sa distance au centre.
pub fn prix_case(distance: i32) -> i32 {
    20 + 10 * distance
}

/// Cases que la ville peut gagner : voisines de son territoire, à personne, dans la carte.
pub fn cases_gagnables(map: &WorldMap, ville: Entity, territoire: &HashMap<Hex, Entity>) -> Vec<Hex> {
    let mut v: Vec<Hex> = territoire
        .iter()
        .filter(|(_, e)| **e == ville)
        .flat_map(|(h, _)| map.neighbors(*h))
        .filter(|h| map.contains(*h) && !territoire.contains_key(h))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Case que la culture fait gagner : la meilleure voisine (ce qu'elle rapporterait aménagée au
/// mieux), la plus proche du centre à valeur égale ; les atouts (ressources) d'abord, quand ils
/// existeront.
pub fn case_par_la_culture(map: &WorldMap, centre: Hex, ville: Entity, territoire: &HashMap<Hex, Entity>) -> Option<Hex> {
    let valeur = |h: Hex| {
        std::iter::once(None)
            .chain(Amenagement::possibles(map, h).into_iter().map(Some))
            .map(|a| intendant::rendement_max(map, h, a).total())
            .fold(0.0, f64::max)
    };
    cases_gagnables(map, ville, territoire)
        .into_iter()
        .map(|h| (h, valeur(h), map.distance(centre, h)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.2.cmp(&a.2)).then(b.0.cmp(&a.0)))
        .map(|(h, _, _)| h)
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

/// Nombre à une décimale, à la française (« 2,5 »).
pub fn decimal(x: f64) -> String {
    format!("{x:.1}").replace('.', ",")
}

/// Tours pour finir `cout` avec `stock` déjà accumulé à `par_tour` par tour (`None` : jamais).
pub fn tours_restants(cout: i32, stock: f64, par_tour: f64) -> Option<i32> {
    let reste = (cout as f64 - stock).max(0.0);
    if reste == 0.0 {
        Some(0)
    } else if par_tour > 1e-9 {
        Some((reste / par_tour).ceil() as i32)
    } else {
        None
    }
}

/// Variation des habitants en un tour selon la nourriture produite : croissance naturelle quand
/// la nourriture dépasse le besoin (pleine à 50 % de surplus), famine quand elle manque.
#[cfg(test)]
pub fn variation_des_habitants(habitants: u64, nourriture: f64) -> i64 {
    let besoin = (habitants as f64 / HABITANTS_PAR_NOURRITURE).max(0.001);
    let couverture = nourriture.max(0.0) / besoin;
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
    Yields::new(y.food.max(2), y.production.max(2), y.deniers.max(1))
}

/// Cases exploitables par une ville (son territoire, hors centre et hors cases d'autres villes),
/// avec le rendement maximal et la capacité que leur donne leur aménagement.
pub fn cases_de_la_ville(map: &WorldMap, center: Hex, territory: &[Hex], taken: &HashSet<Hex>, amenagements: &std::collections::BTreeMap<Hex, Amenagement>) -> Vec<CaseDispo> {
    let mut cases: Vec<CaseDispo> = territory
        .iter()
        .copied()
        .filter(|&h| h != center && !taken.contains(&h) && map.contains(h))
        .map(|h| {
            let a = amenagements.get(&h).copied();
            CaseDispo { hex: h, max: intendant::rendement_max(map, h, a), parts: intendant::parts_de_la_case(map, h, a), sortes: intendant::sortes_de_la_case(map, h, a), capacite: intendant::capacite(map, h, a), distance: map.distance(center, h) }
        })
        .collect();
    cases.sort_by_key(|c| c.hex);
    cases
}

// ---------------------------------------------------------------- Bien-être et salubrité (4c)

// ---------------------------------------------------------------- Nourriture par type (5c)

/// Limite de la ville selon les types de nourriture qui comptent (1 point de nourriture nourrit
/// 1 000, 1 100 ou 1 200 habitants) : × 1,10 avec deux types, × 1,20 avec trois.
pub fn facteur_equilibre(types: u8) -> f64 {
    1.0 + 0.1 * f64::from(types.clamp(1, 3) - 1)
}

/// Famines adoucies par l'équilibre : pertes × 0,75 avec deux types, × 0,5 avec trois.
pub fn facteur_famine(types: u8) -> f64 {
    match types {
        0 | 1 => 1.0,
        2 => 0.75,
        _ => 0.5,
    }
}

/// Zone d'équilibre au-dessus du besoin (3 %) : pas de croissance, pour que la ville se pose à sa
/// limite sans osciller entre croissance et famine.
pub const ZONE_EQUILIBRE: f64 = 0.03;

/// Bien-être neutre (et de départ) : la croissance naturelle n'y change pas.
pub const BIEN_ETRE_NEUTRE: f64 = 60.0;
/// Bien-être visé en moins pendant une famine.
pub const BIEN_ETRE_FAMINE: f64 = 15.0;
/// Bien-être visé en moins par point de % de chômeurs parmi les actifs (1 par tranche de 2 %), au plus.
pub const BIEN_ETRE_PAR_CHOMAGE: f64 = 0.5;
pub const BIEN_ETRE_CHOMAGE_MAX: f64 = 20.0;
/// Grands jeux : bien-être visé en plus par point de production (1 pour 5), au plus.
pub const BIEN_ETRE_PAR_PRODUCTION_JEUX: f64 = 0.2;
pub const BIEN_ETRE_JEUX_MAX: f64 = 15.0;
/// Part de l'écart au bien-être visé comblée chaque tour (il suit avec retard).
pub const BIEN_ETRE_INERTIE: f64 = 0.2;
/// Salubrité d'une ville sans rien de particulier.
pub const SALUBRITE_BASE: f64 = 50.0;
/// Eau courante voisine (rivière, fleuve) : boire, laver, évacuer.
pub const SALUBRITE_EAU_COURANTE: f64 = 15.0;
/// Lac voisin : de l'eau, mais qui se renouvelle mal (parasites).
pub const SALUBRITE_LAC: f64 = 5.0;
/// Marais voisin : les fièvres, de tout leur poids là où l'été est chaud.
pub const SALUBRITE_MARAIS: f64 = 15.0;
/// Été (saison la plus chaude) sous lequel un marais ne donne pas de fièvres, et au-dessus duquel
/// il pèse de tout son poids (°C).
pub const MARAIS_ETE_FROID: f64 = 15.0;
pub const MARAIS_ETE_CHAUD: f64 = 20.0;
/// Pâturages exploités (maladies venues du bétail) : par pâturage, au plus.
pub const SALUBRITE_PAR_PATURAGE: f64 = 1.0;
pub const SALUBRITE_PATURAGES_MAX: f64 = 10.0;
/// Taille : salubrité en moins par millier d'habitants au-delà de `SALUBRITE_TAILLE_DEBUT` (le
/// double sans eau courante), au plus.
pub const SALUBRITE_TAILLE_DEBUT: f64 = 5_000.0;
pub const SALUBRITE_PAR_MILLIER: f64 = 1.0;
pub const SALUBRITE_TAILLE_MAX: f64 = 40.0;
/// Puits de la ville.
pub const SALUBRITE_PUITS: f64 = 10.0;
/// Salubrité sous laquelle la ville perd des habitants (maladies), jusqu'à `PERTE_INSALUBRITE_MAX`
/// par tour à 0 ; elle n'est montrée que sous ce seuil.
pub const SALUBRITE_SEUIL: f64 = 40.0;
pub const PERTE_INSALUBRITE_MAX: f64 = 0.01;
/// Coût du puits (production).
pub const PUITS_COUT: i32 = 15;
/// Grenier (décisions G1 à G4) : coût (production) et capacité, en tours de besoin de la ville.
pub const GRENIER_COUT: i32 = 30;
pub const GRENIER_TOURS: f64 = 5.0;

/// Ce que le lieu de la ville donne à sa salubrité (calculé depuis la carte, gardé dans la ville
/// pour ses tours et sa prévision).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Site {
    /// Rivière ou fleuve sur la case du centre ou une voisine.
    pub eau_courante: bool,
    /// Lac (ou petit lac) sur le centre ou une voisine.
    pub lac: bool,
    /// Marais voisin, pondéré par la chaleur de l'été (0 à 1).
    pub marais: f64,
    /// Pâturages exploités dans le territoire.
    pub paturages: u32,
}

/// Site de la ville au centre `centre` : eau courante, lac, marais (selon l'été), pâturages du territoire.
pub fn site_de_la_ville(map: &WorldMap, centre: Hex, territoire: &[Hex], amenagements: &std::collections::BTreeMap<Hex, Amenagement>) -> Site {
    let autour: Vec<Hex> = std::iter::once(centre).chain(map.neighbors(centre)).map(|h| map.canonical(h)).filter(|h| map.contains(*h)).collect();
    // (la source d'une oasis voisine compte comme une eau courante)
    let eau_courante = autour.iter().any(|&h| map.is_fleuve(h) || map.get(h) == Some(crate::map::Terrain::Oasis)) || (0..6).any(|d| map.river_flow(centre, d) > 0.0);
    let lac = autour.iter().any(|&h| map.get(h) == Some(crate::map::Terrain::Lake) || map.small_lake(h));
    let marais = autour
        .iter()
        .filter(|&&h| map.get(h) == Some(crate::map::Terrain::Marsh))
        .map(|&h| {
            let ete = map.warmest_season(h).unwrap_or(0.0) as f64;
            ((ete - MARAIS_ETE_FROID) / (MARAIS_ETE_CHAUD - MARAIS_ETE_FROID)).clamp(0.0, 1.0)
        })
        .fold(0.0, f64::max);
    let paturages = territoire.iter().filter(|h| amenagements.get(h) == Some(&Amenagement::Paturage)).count() as u32;
    Site { eau_courante, lac, marais, paturages }
}

/// Humeur de la ville, dite en mots (le bien-être en chiffres reste dans la fiche).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Humeur {
    Mecontente,
    Satisfaite,
    Prospere,
}

impl Humeur {
    /// Humeur pour un bien-être : mécontente sous 40, prospère au-dessus de 70.
    pub fn de(bien_etre: f64) -> Humeur {
        if bien_etre < 40.0 {
            Humeur::Mecontente
        } else if bien_etre > 70.0 {
            Humeur::Prospere
        } else {
            Humeur::Satisfaite
        }
    }

    pub const fn nom(self) -> &'static str {
        match self {
            Humeur::Mecontente => "mécontente",
            Humeur::Satisfaite => "satisfaite",
            Humeur::Prospere => "prospère",
        }
    }
}

/// Croissance d'une ville en un tour, par cause (parts des habitants ; la somme fait la variation).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Croissance {
    /// Croissance naturelle (surplus de nourriture), bien-être compris.
    pub naturelle: f64,
    /// Facteur du bien-être sur la croissance naturelle (1 à 60).
    pub facteur_bien_etre: f64,
    /// Projet « Peupler la ville ».
    pub peupler: f64,
    /// Maladies d'une salubrité sous 50 (négatif).
    pub insalubrite: f64,
    /// Famine (négatif).
    pub famine: f64,
}

impl Croissance {
    pub fn total(&self) -> f64 {
        self.naturelle + self.peupler + self.insalubrite + self.famine
    }
}

/// Travailleurs de la ville placés par l'intendant, et son rendement total (centre compris).
pub fn repartition_ville(map: &WorldMap, center: Hex, city: &CityState, cases: &[CaseDispo]) -> Repartition {
    intendant::repartir_avec(cases, intendant::actifs(city.habitants), city.besoin_nourriture(), Rendements::from(center_yields(map, center)), intendant::sortes_du_centre(map, center), &city.consignes)
}

/// Un tour de la prévision d'une ville.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pas {
    /// Habitants à la fin du tour.
    pub habitants: u64,
    /// Rendements du tour (répartition de l'intendant avec les habitants du début du tour).
    pub rendements: Rendements,
    /// Variation des habitants pendant le tour.
    pub variation: i64,
    /// Actifs sans case pendant le tour.
    pub chomeurs: u32,
    /// Bien-être à la fin du tour.
    pub bien_etre: f64,
}

/// Croissance (part des habitants par tour) sous laquelle la ville est jugée arrêtée.
pub const CROISSANCE_ARRETEE: f64 = 0.001;

/// Prévision de la ville sur `tours` tours : à chaque tour, la répartition de l'intendant refaite
/// avec les habitants du moment, puis la croissance (mêmes cases, consignes et projet ; la file de
/// construction n'avance pas). Une seule prévision sert à tout l'écran de la ville (décision B4).
pub fn prevoir(map: &WorldMap, center: Hex, city: &CityState, cases: &[CaseDispo], tours: usize) -> Vec<Pas> {
    let mut st = city.allege();
    let mut cases = cases.to_vec();
    (0..tours)
        .map(|_| {
            let r = repartition_ville(map, center, &st, &cases);
            let v = st.variation(r.rendements);
            // Le tour comme en jeu (croissance, puis la file avance : décision C1) ; un aménagement
            // achevé change sa case pour les tours suivants
            let rapport = st.passer_le_tour(r.rendements, r.chomeurs, r.par_type, &r.par_sorte);
            if let Some(Production::Amenagement { hex, kind, .. }) = rapport.built
                && let Some(c) = cases.iter_mut().find(|c| c.hex == hex)
            {
                c.max = intendant::rendement_max(map, hex, Some(kind));
                c.capacite = intendant::capacite(map, hex, Some(kind));
            }
            Pas { habitants: st.habitants, rendements: r.rendements, variation: v, chomeurs: r.chomeurs, bien_etre: st.bien_etre }
        })
        .collect()
}

/// Premier tour de la prévision (1 = le prochain) où la croissance s'arrête (moins de 0,1 % par
/// tour, ou baisse), s'il y en a un.
pub fn arret_de_croissance(prevision: &[Pas]) -> Option<usize> {
    prevision.iter().position(|p| (p.variation as f64) < CROISSANCE_ARRETEE * (p.habitants as i64 - p.variation).max(1) as f64).map(|i| i + 1)
}

/// Premier tour de la prévision (1 = le prochain) où la ville atteint `habitants`.
pub fn tours_jusqu_a(prevision: &[Pas], habitants: u64) -> Option<usize> {
    prevision.iter().position(|p| p.habitants >= habitants).map(|i| i + 1)
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

/// Ce que construit une ville : une unité, un pont à un emplacement choisi, ou un aménagement
/// sur une case (coûts fixés au choix).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Production {
    Unit(UnitKind),
    Bridge { at: crate::map::BridgeAt, cost: i32 },
    Amenagement { hex: Hex, kind: Amenagement, cost: i32 },
    /// Puits de la ville (salubrité ; plus tard, de l'eau) : un par ville, sur le centre.
    Puits,
    /// Grenier de la ville (stock de nourriture contre les famines) : un par ville.
    Grenier,
}

impl Production {
    pub fn cost(self) -> i32 {
        match self {
            Production::Unit(kind) => kind.cost(),
            Production::Bridge { cost, .. } | Production::Amenagement { cost, .. } => cost,
            Production::Puits => PUITS_COUT,
            Production::Grenier => GRENIER_COUT,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Production::Unit(kind) => kind.name(),
            Production::Bridge { at: crate::map::BridgeAt::Edge { .. }, .. } => "Pont sur la rivière",
            Production::Bridge { at: crate::map::BridgeAt::Fleuve { .. }, .. } => "Pont sur le fleuve",
            Production::Amenagement { kind, .. } => kind.nom(),
            Production::Puits => "Puits",
            Production::Grenier => "Grenier",
        }
    }
}

/// Projet de ville : sans fin, il reçoit la production quand la file de construction est vide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Projet {
    /// La production s'ajoute au surplus de nourriture pour la croissance.
    Peupler,
    /// Bien-être (livraison 4c).
    GrandsJeux,
    /// Recherche du pays (tâche 34).
    Recherche,
    /// Culture, qui étend le territoire (livraison 4d).
    Culture,
}

impl Projet {
    pub const TOUS: [Projet; 4] = [Projet::Peupler, Projet::GrandsJeux, Projet::Recherche, Projet::Culture];

    pub const fn nom(self) -> &'static str {
        match self {
            Projet::Peupler => "Peupler la ville",
            Projet::GrandsJeux => "Grands jeux",
            Projet::Recherche => "Projet de recherche",
            Projet::Culture => "Développement de l'art et de la culture",
        }
    }

    /// Nom court (console).
    pub const fn code(self) -> &'static str {
        match self {
            Projet::Peupler => "peupler",
            Projet::GrandsJeux => "jeux",
            Projet::Recherche => "recherche",
            Projet::Culture => "culture",
        }
    }

    /// Ce que fait le projet, en une ligne.
    pub const fn effet(self) -> &'static str {
        match self {
            Projet::Peupler => "la production augmente la croissance (jusqu'à +1,5 % par tour, si la ville n'a pas faim)",
            Projet::GrandsJeux => "le bien-être de la ville monte (1 point pour 5 de production, jusqu'à +15)",
            Projet::Recherche => "recherche du pays",
            Projet::Culture => "la production donne de la culture (1 pour 5), qui étend le territoire",
        }
    }

    /// Le système du projet est-il dans le jeu ?
    pub const fn disponible(self) -> bool {
        matches!(self, Projet::Peupler | Projet::GrandsJeux | Projet::Culture)
    }
}

/// Projet « Peupler la ville » : croissance en plus par point de production rapporté au besoin de
/// nourriture (Benjamin : le projet augmente la croissance, pas la nourriture ; chiffres à régler).
pub const BONUS_PEUPLER_PAR_PRODUCTION: f64 = 0.001;
/// Croissance en plus au plus, par tour (la croissance naturelle double au plus).
pub const BONUS_PEUPLER_MAX: f64 = CROISSANCE_NATURELLE;
/// Le joueur est prévenu tous les `RAPPEL_PROJET` tours passés en projet.
pub const RAPPEL_PROJET: u32 = 10;

/// Élément de la file de construction, avec la production déjà investie.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Commande {
    pub objet: Production,
    pub investi: f64,
}

/// État économique d'une ville.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CityState {
    pub name: String,
    /// Habitants de toute l'aire de la ville (ville et campagnes).
    pub habitants: u64,
    /// File de construction (la tête reçoit la production).
    pub file: Vec<Commande>,
    /// Projet suivi quand la file est vide.
    pub projet: Option<Projet>,
    /// Tours d'affilée passés en projet (pour le rappel).
    pub tours_en_projet: u32,
    /// Ponts construits par la ville (une tour garde leur autre bout si elle est fortifiée).
    pub bridges: Vec<crate::map::BridgeAt>,
    /// Présence d'une fortification : la muraille n'existe que si elle est vraie. La règle de jeu
    /// reste à concevoir ; pour l'instant, seule la console de debug la pose.
    pub fortified: bool,
    /// Consignes du joueur à l'intendant (priorités, cases forcées).
    #[serde(default)]
    pub consignes: Consignes,
    /// Rendements et habitants des derniers tours (`HISTORIQUE_TOURS` au plus, le plus récent à la
    /// fin) : graphiques de l'écran de la ville.
    #[serde(default)]
    pub historique: std::collections::VecDeque<Releve>,
    /// Histoire de la ville : ses moments marquants, dans l'ordre.
    #[serde(default)]
    pub chronique: Vec<Evenement>,
    /// La ville était-elle en famine au tour précédent (début et fin de famine dans la chronique) ?
    #[serde(default)]
    pub en_famine: bool,
    /// Stock de culture (une case de plus au seuil, 4d).
    #[serde(default)]
    pub culture: f64,
    /// Cases gagnées par la culture ou achetées (le seuil suivant en dépend).
    #[serde(default)]
    pub cases_gagnees: u32,
    /// Bien-être (0 à 100, 60 au départ) : il suit le bien-être visé avec retard (4c).
    #[serde(default = "bien_etre_de_depart")]
    pub bien_etre: f64,
    /// Ce que le lieu donne à la salubrité (mis à jour depuis la carte).
    #[serde(default)]
    pub site: Site,
    /// La ville a son puits.
    #[serde(default)]
    pub puits: bool,
    /// La salubrité était-elle sous le seuil au tour précédent (début et fin dans la chronique) ?
    #[serde(default)]
    pub insalubre: bool,
    /// Types de nourriture qui ont compté au dernier tour (0 à 3) : la limite et les famines en
    /// dépendent (5c).
    #[serde(default)]
    pub types_nourriture: u8,
    /// Sortes en plus (au-delà de la première de chaque type) au dernier tour : la diversité des
    /// repas, qui donne du bien-être (5d).
    #[serde(default)]
    pub sortes_en_plus: u8,
    /// La ville a son grenier.
    #[serde(default)]
    pub grenier: bool,
    /// Nourriture gardée au grenier, par sorte (ordre de `Sorte::TOUTES` ; vide sans grenier).
    #[serde(default)]
    pub stock: Vec<f64>,
}

fn bien_etre_de_depart() -> f64 {
    BIEN_ETRE_NEUTRE
}

/// Genre d'un moment de l'histoire d'une ville.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GenreEvenement {
    Fondation,
    Population,
    Construction,
    Famine,
    Bataille,
    Projet,
    /// Case gagnée par la culture ou achetée.
    Territoire,
}

impl GenreEvenement {
    pub const TOUS: [GenreEvenement; 7] = [GenreEvenement::Fondation, GenreEvenement::Population, GenreEvenement::Construction, GenreEvenement::Territoire, GenreEvenement::Famine, GenreEvenement::Bataille, GenreEvenement::Projet];

    /// Nom du filtre.
    pub const fn nom(self) -> &'static str {
        match self {
            GenreEvenement::Fondation => "Fondation",
            GenreEvenement::Population => "Population",
            GenreEvenement::Construction => "Constructions",
            GenreEvenement::Famine => "Famines",
            GenreEvenement::Bataille => "Batailles",
            GenreEvenement::Projet => "Projets",
            GenreEvenement::Territoire => "Territoire",
        }
    }
}

/// Moment marquant de l'histoire d'une ville (le lieu : une case, s'il en a une).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Evenement {
    pub tour: u32,
    pub genre: GenreEvenement,
    pub texte: String,
    pub lieu: Option<Hex>,
}

/// Tours gardés dans l'historique d'une ville.
pub const HISTORIQUE_TOURS: usize = 40;

/// Relevé d'un tour de la ville : ce qu'elle a produit, et ses habitants à la fin du tour.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Releve {
    pub nourriture: f64,
    pub production: f64,
    pub deniers: f64,
    pub habitants: u64,
}

/// Ce qui s'est passé dans une ville pendant la fin de tour.
#[derive(Debug, Default, PartialEq)]
pub struct CityTurnReport {
    pub grew: bool,
    /// La nourriture n'a pas couvert le besoin (la population baisse).
    pub starved: bool,
    /// Rien à produire (file vide, pas de projet) : production perdue.
    pub inactive: bool,
    /// Rappel : la ville est en projet depuis `tours_en_projet` tours.
    pub rappel_projet: bool,
    pub built: Option<Production>,
    pub deniers: f64,
    /// Le colon en tête de file vient d'être payé mais la ville n'a pas assez d'habitants : la file
    /// est bloquée (dit une seule fois, au tour où il est payé).
    pub colon_attend: bool,
    /// Nourriture puisée au grenier ce tour-ci.
    pub puise: f64,
}

impl CityState {
    pub fn new(name: String) -> Self {
        Self { name, habitants: HABITANTS_COLON, file: Vec::new(), projet: None, tours_en_projet: 0, bridges: Vec::new(), fortified: false, consignes: Consignes::default(), historique: Default::default(), chronique: Vec::new(), en_famine: false, culture: 0.0, cases_gagnees: 0, bien_etre: BIEN_ETRE_NEUTRE, site: Site::default(), puits: false, insalubre: false, types_nourriture: 0, sortes_en_plus: 0, grenier: false, stock: Vec::new() }
    }

    /// Consommation : 2 nourriture par habitant.
    /// Palier de la ville (ce que voit le joueur et ce que lisent les règles de taille).
    pub fn palier(&self) -> i32 {
        palier_de(self.habitants)
    }

    /// Nourriture nécessaire à la population de l'aire ce tour-ci : une nourriture équilibrée
    /// nourrit plus de monde (`facteur_equilibre`).
    pub fn besoin_nourriture(&self) -> f64 {
        self.habitants as f64 / HABITANTS_PAR_NOURRITURE / facteur_equilibre(self.types_nourriture)
    }

    /// Inscrit un moment dans l'histoire de la ville.
    pub fn noter(&mut self, tour: u32, genre: GenreEvenement, texte: impl Into<String>, lieu: Option<Hex>) {
        self.chronique.push(Evenement { tour, genre, texte: texte.into(), lieu });
    }

    /// Projet qui reçoit la production ce tour-ci (la file est vide).
    pub fn projet_actif(&self) -> Option<Projet> {
        self.projet.filter(|_| self.file.is_empty())
    }

    /// Croissance en plus donnée par le projet « Peupler la ville » (part des habitants par tour) :
    /// seulement s'il est suivi et si la ville n'a pas faim.
    pub fn bonus_peupler(&self, yields: Rendements) -> f64 {
        let besoin = self.besoin_nourriture().max(1e-6);
        if self.projet_actif() == Some(Projet::Peupler) && yields.nourriture >= besoin {
            (BONUS_PEUPLER_PAR_PRODUCTION * yields.production / besoin).min(BONUS_PEUPLER_MAX)
        } else {
            0.0
        }
    }

    /// Croissance au prochain tour avec ces rendements, par cause : nourriture (et bien-être),
    /// projet, salubrité, famine.
    pub fn croissance(&self, yields: Rendements) -> Croissance {
        let besoin = self.besoin_nourriture().max(0.001);
        let couverture = self.nourriture_avec_grenier(yields.nourriture).max(0.0) / besoin;
        let facteur_bien_etre = (1.0 + (self.bien_etre - BIEN_ETRE_NEUTRE) / 100.0).max(0.0);
        let s = self.salubrite();
        let insalubrite = if s < SALUBRITE_SEUIL { -PERTE_INSALUBRITE_MAX * (SALUBRITE_SEUIL - s) / SALUBRITE_SEUIL } else { 0.0 };
        if couverture >= 1.0 {
            let naturelle = CROISSANCE_NATURELLE * ((couverture - 1.0 - ZONE_EQUILIBRE) / (SURPLUS_POUR_CROITRE - ZONE_EQUILIBRE)).clamp(0.0, 1.0) * facteur_bien_etre;
            Croissance { naturelle, facteur_bien_etre, peupler: self.bonus_peupler(yields), insalubrite, famine: 0.0 }
        } else {
            let famine = -(FAMINE_MIN + (FAMINE_MAX - FAMINE_MIN) * (1.0 - couverture)) * facteur_famine(self.types_nourriture);
            Croissance { naturelle: 0.0, facteur_bien_etre, peupler: 0.0, insalubrite, famine }
        }
    }

    /// Nourriture gardée au grenier (toutes sortes).
    pub fn stock_total(&self) -> f64 {
        self.stock.iter().sum()
    }

    /// Capacité du grenier : `GRENIER_TOURS` tours de besoin (0 sans grenier).
    pub fn capacite_grenier(&self) -> f64 {
        if self.grenier { GRENIER_TOURS * self.besoin_nourriture() } else { 0.0 }
    }

    /// Ce que la ville tirera du grenier avec `nourriture` ce tour-ci : le manque, tant qu'il y en a.
    pub fn tire_du_grenier(&self, nourriture: f64) -> f64 {
        (self.besoin_nourriture() - nourriture).max(0.0).min(self.stock_total())
    }

    /// Nourriture du tour, grenier compris (il comble le manque).
    pub fn nourriture_avec_grenier(&self, nourriture: f64) -> f64 {
        nourriture + self.tire_du_grenier(nourriture)
    }

    /// Ce qui peut aller au grenier avec `nourriture` (décision G2) : l'excédent que la croissance
    /// n'utilise pas, dans la zone d'équilibre (jusqu'à 3 % au-dessus du besoin) et au-delà de
    /// `1 + SURPLUS_POUR_CROITRE` fois le besoin.
    pub fn excedent_pour_grenier(&self, nourriture: f64) -> f64 {
        let besoin = self.besoin_nourriture();
        (nourriture - besoin).clamp(0.0, ZONE_EQUILIBRE * besoin) + (nourriture - (1.0 + SURPLUS_POUR_CROITRE) * besoin).max(0.0)
    }

    /// Le grenier au tour : on y puise le manque (les sortes qui se gardent le moins d'abord),
    /// les pertes de chaque sorte, puis l'excédent y entre (celles qui se gardent le mieux
    /// d'abord, chacune au plus ce que la ville en a produit), jusqu'à la capacité. Rend ce qui a
    /// été puisé.
    fn tour_du_grenier(&mut self, nourriture: f64, par_sorte: &intendant::ParSorte) -> f64 {
        if !self.grenier {
            return 0.0;
        }
        self.stock.resize(intendant::N_SORTES, 0.0);
        let mut ordre: Vec<usize> = (0..intendant::N_SORTES).collect();
        ordre.sort_by(|&a, &b| intendant::Sorte::TOUTES[a].perte().total_cmp(&intendant::Sorte::TOUTES[b].perte()));
        let puise = self.tire_du_grenier(nourriture);
        let mut reste = puise;
        for &s in ordre.iter().rev() {
            let pris = reste.min(self.stock[s]);
            self.stock[s] -= pris;
            reste -= pris;
        }
        for (s, q) in self.stock.iter_mut().enumerate() {
            *q *= 1.0 - intendant::Sorte::TOUTES[s].perte();
        }
        let mut entre = self.excedent_pour_grenier(nourriture).min((self.capacite_grenier() - self.stock_total()).max(0.0));
        for &s in &ordre {
            let q = entre.min(par_sorte[s]);
            self.stock[s] += q;
            entre -= q;
        }
        puise
    }

    /// Variation des habitants au prochain tour avec ces rendements (projet, bien-être et salubrité compris).
    pub fn variation(&self, yields: Rendements) -> i64 {
        (self.habitants as f64 * self.croissance(yields).total()).round() as i64
    }

    /// Ce qui fait la salubrité, en points (le lieu, le puits, la taille), sans la base.
    pub fn termes_salubrite(&self) -> Vec<(&'static str, f64)> {
        let s = &self.site;
        let taille_k = if s.eau_courante { 1.0 } else { 2.0 };
        let taille = ((self.habitants as f64 - SALUBRITE_TAILLE_DEBUT).max(0.0) / 1_000.0 * SALUBRITE_PAR_MILLIER * taille_k).min(SALUBRITE_TAILLE_MAX);
        let termes = [
            ("eau courante voisine", if s.eau_courante { SALUBRITE_EAU_COURANTE } else { 0.0 }),
            ("lac voisin", if s.lac { SALUBRITE_LAC } else { 0.0 }),
            ("marais voisin (fièvres)", -SALUBRITE_MARAIS * s.marais),
            ("bétail des pâturages", -(SALUBRITE_PAR_PATURAGE * s.paturages as f64).min(SALUBRITE_PATURAGES_MAX)),
            ("puits", if self.puits { SALUBRITE_PUITS } else { 0.0 }),
            (if s.eau_courante { "taille de la ville" } else { "taille de la ville, sans eau courante" }, -taille),
        ];
        termes.into_iter().filter(|(_, v)| v.abs() >= 0.05).collect()
    }

    /// Salubrité (0 à 100) : le lieu, le puits et la taille de la ville.
    pub fn salubrite(&self) -> f64 {
        (SALUBRITE_BASE + self.termes_salubrite().iter().map(|(_, v)| v).sum::<f64>()).clamp(0.0, 100.0)
    }

    /// Ce qui fait le bien-être visé ce tour, en points (sans la base) : famine, chômage, Grands
    /// jeux (le luxe viendra avec les ressources).
    pub fn termes_bien_etre(&self, yields: Rendements, chomeurs: u32) -> Vec<(&'static str, f64)> {
        let famine = if self.nourriture_avec_grenier(yields.nourriture) < self.besoin_nourriture() - 1e-9 { -BIEN_ETRE_FAMINE } else { 0.0 };
        let part = chomeurs as f64 / intendant::actifs(self.habitants).max(1) as f64;
        let chomage = -(part * 100.0 * BIEN_ETRE_PAR_CHOMAGE).min(BIEN_ETRE_CHOMAGE_MAX);
        let jeux = if self.projet_actif() == Some(Projet::GrandsJeux) { (BIEN_ETRE_PAR_PRODUCTION_JEUX * yields.production.max(0.0)).min(BIEN_ETRE_JEUX_MAX) } else { 0.0 };
        let diversite = (intendant::BIEN_ETRE_PAR_SORTE * f64::from(self.sortes_en_plus)).min(intendant::BIEN_ETRE_DIVERSITE_MAX);
        [("famine", famine), ("chômage", chomage), ("Grands jeux", jeux), ("diversité des repas", diversite)].into_iter().filter(|(_, v)| v.abs() >= 0.05).collect()
    }

    /// Bien-être visé ce tour (0 à 100) ; le bien-être de la ville s'en approche d'un cinquième de l'écart.
    pub fn bien_etre_vise(&self, yields: Rendements, chomeurs: u32) -> f64 {
        (BIEN_ETRE_NEUTRE + self.termes_bien_etre(yields, chomeurs).iter().map(|(_, v)| v).sum::<f64>()).clamp(0.0, 100.0)
    }

    /// Humeur de la ville.
    pub fn humeur(&self) -> Humeur {
        Humeur::de(self.bien_etre)
    }

    /// Ajoute un élément à la fin de la file (refusé s'il y est déjà : même pont, même case).
    pub fn mettre_en_file(&mut self, objet: Production) -> bool {
        let deja = |c: &Commande| match (c.objet, objet) {
            (Production::Bridge { at: a, .. }, Production::Bridge { at: b, .. }) => a == b,
            (Production::Amenagement { hex: a, .. }, Production::Amenagement { hex: b, .. }) => a == b,
            _ => false,
        };
        let unique = |fait: bool| fait || self.file.iter().any(|c| c.objet == objet);
        if self.file.iter().any(deja) || objet == Production::Puits && unique(self.puits) || objet == Production::Grenier && unique(self.grenier) {
            return false;
        }
        self.file.push(Commande { objet, investi: 0.0 });
        true
    }

    /// Tours pour finir l'élément `i` de la file (ceux d'avant d'abord) au rythme `par_tour`.
    pub fn tours_pour(&self, i: usize, par_tour: f64) -> Option<i32> {
        let reste: f64 = self.file.iter().take(i + 1).map(|c| (c.objet.cost() as f64 - c.investi).max(0.0)).sum();
        tours_restants(reste.ceil() as i32, 0.0, par_tour)
    }

    /// Copie pour une simulation : sans nom, historique ni chronique (plus légère).
    pub fn allege(&self) -> CityState {
        CityState {
            name: String::new(),
            habitants: self.habitants,
            file: self.file.clone(),
            projet: self.projet,
            tours_en_projet: self.tours_en_projet,
            bridges: Vec::new(),
            fortified: self.fortified,
            consignes: self.consignes.clone(),
            historique: Default::default(),
            chronique: Vec::new(),
            en_famine: self.en_famine,
            culture: self.culture,
            cases_gagnees: self.cases_gagnees,
            bien_etre: self.bien_etre,
            site: self.site,
            puits: self.puits,
            insalubre: self.insalubre,
            types_nourriture: self.types_nourriture,
            sortes_en_plus: self.sortes_en_plus,
            grenier: self.grenier,
            stock: self.stock.clone(),
        }
    }

    /// Culture produite ce tour : la ville, plus le projet « Développement de l'art et de la culture ».
    pub fn culture_du_tour(&self, yields: Rendements) -> f64 {
        CULTURE_PAR_TOUR + if self.projet_actif() == Some(Projet::Culture) { CULTURE_PAR_PRODUCTION * yields.production.max(0.0) } else { 0.0 }
    }

    /// Tours avant la prochaine case gagnée par la culture, au rythme `par_tour`.
    pub fn tours_avant_case(&self, par_tour: f64) -> Option<u32> {
        let reste = seuil_culture(self.cases_gagnees) - self.culture;
        (par_tour > 0.0).then(|| (reste / par_tour).ceil().max(0.0) as u32)
    }

    /// Habitants qu'il faut pour former un colon (il en emporte la moitié).
    pub const HABITANTS_POUR_COLON: u64 = 2 * HABITANTS_COLON;

    /// Un colon en tête de file attend des habitants : rien d'autre n'avance.
    pub fn colon_bloque(&self) -> bool {
        self.file.first().is_some_and(|k| k.objet == Production::Unit(UnitKind::Settler)) && self.habitants < Self::HABITANTS_POUR_COLON
    }

    /// Un tour de la ville sans chômeurs (tests).
    #[cfg(test)]
    pub fn process_turn(&mut self, yields: Rendements) -> CityTurnReport {
        self.passer_le_tour(yields, 0, [yields.nourriture, 0.0, 0.0], &[0.0; intendant::N_SORTES])
    }

    /// Un tour de la ville : croissance, bien-être, culture, production ; `par_type` : sa
    /// nourriture par type (l'équilibre du tour suivant).
    pub fn passer_le_tour(&mut self, yields: Rendements, chomeurs: u32, par_type: intendant::Parts, par_sorte: &intendant::ParSorte) -> CityTurnReport {
        let mut report = CityTurnReport { deniers: yields.deniers, ..Default::default() };

        // Croissance (ou famine, maladies) en habitants ; « a grandi » : nouveau palier atteint
        let palier = self.palier();
        let variation = self.variation(yields);
        let vise = self.bien_etre_vise(yields, chomeurs);
        report.starved = self.nourriture_avec_grenier(yields.nourriture) < self.besoin_nourriture() - 1e-9;
        // Grenier : avec le besoin du début du tour (avant le changement d'habitants)
        report.puise = self.tour_du_grenier(yields.nourriture, par_sorte);
        if variation < 0 {
            self.habitants = self.habitants.saturating_sub(variation.unsigned_abs()).max(HABITANTS_PLANCHER.min(self.habitants));
        } else {
            self.habitants += variation as u64;
        }
        report.grew = self.palier() > palier;
        self.historique.push_back(Releve { nourriture: yields.nourriture, production: yields.production, deniers: yields.deniers, habitants: self.habitants });
        while self.historique.len() > HISTORIQUE_TOURS {
            self.historique.pop_front();
        }

        // Bien-être : il s'approche du bien-être visé au début du tour
        self.bien_etre += (vise - self.bien_etre) * BIEN_ETRE_INERTIE;
        // Types de nourriture qui comptent, sur ce que la ville a mangé ce tour-ci
        self.types_nourriture = intendant::types_comptes(par_type, self.besoin_nourriture());
        self.sortes_en_plus = intendant::sortes_en_plus(par_sorte, self.besoin_nourriture());

        // Culture : la ville, plus le projet (les cases gagnées sont choisies par le jeu, qui voit la carte)
        self.culture += self.culture_du_tour(yields);

        // Production : la tête de la file, sinon le projet, sinon perdue
        if let Some(tete) = self.file.first_mut() {
            self.tours_en_projet = 0;
            tete.investi += yields.production;
            let item = tete.objet;
            let settler = item == Production::Unit(UnitKind::Settler);
            let pop_ok = !settler || self.habitants >= Self::HABITANTS_POUR_COLON;
            let paye = tete.investi >= item.cost() as f64;
            report.colon_attend = settler && paye && !pop_ok && tete.investi - yields.production < item.cost() as f64;
            if paye && pop_ok {
                let surplus = tete.investi - item.cost() as f64;
                self.file.remove(0);
                if let Some(suivant) = self.file.first_mut() {
                    suivant.investi += surplus;
                }
                if item == Production::Puits {
                    self.puits = true;
                }
                if item == Production::Grenier {
                    self.grenier = true;
                }
                if settler {
                    self.habitants -= HABITANTS_COLON;
                    // (le solde du tour compte le départ)
                    if let Some(r) = self.historique.back_mut() {
                        r.habitants = self.habitants;
                    }
                }
                report.built = Some(item);
            }
        } else if self.projet.is_some() {
            self.tours_en_projet += 1;
            report.rappel_projet = self.tours_en_projet.is_multiple_of(RAPPEL_PROJET);
        } else {
            report.inactive = true;
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Relief, Terrain};

    #[test]
    fn le_grenier_garde_l_excedent_et_nourrit_les_famines() {
        use crate::intendant::{N_SORTES, Sorte};
        let mut c = CityState::new("Grenier".into());
        c.habitants = 10_000;
        c.types_nourriture = 1;
        // Un seul grenier, en file puis bâti
        assert!(c.mettre_en_file(Production::Grenier));
        assert!(!c.mettre_en_file(Production::Grenier));
        c.file.clear();
        c.grenier = true;
        let besoin = c.besoin_nourriture();
        assert!((c.capacite_grenier() - GRENIER_TOURS * besoin).abs() < 1e-9);
        // Excédent : la zone d'équilibre (3 %) et ce qui dépasse 1,5 fois le besoin ; rien entre les deux
        assert!(c.excedent_pour_grenier(0.9 * besoin) == 0.0);
        assert!((c.excedent_pour_grenier(1.02 * besoin) - 0.02 * besoin).abs() < 1e-9);
        assert!((c.excedent_pour_grenier(1.3 * besoin) - 0.03 * besoin).abs() < 1e-9);
        assert!((c.excedent_pour_grenier(1.7 * besoin) - 0.23 * besoin).abs() < 1e-9);
        // Il garde d'abord ce qui se garde : blé plutôt que poisson
        let mut par_sorte = [0.0; N_SORTES];
        par_sorte[Sorte::Poisson.index()] = 0.9 * besoin;
        par_sorte[Sorte::Ble.index()] = 0.12 * besoin;
        let puise = c.tour_du_grenier(1.02 * besoin, &par_sorte);
        assert_eq!(puise, 0.0);
        assert!((c.stock[Sorte::Ble.index()] - 0.02 * besoin).abs() < 1e-9 && c.stock[Sorte::Poisson.index()] == 0.0, "{:?}", c.stock);
        // Pertes : 2 % pour le blé, 20 % pour le poisson
        c.stock[Sorte::Poisson.index()] = 1.0;
        c.tour_du_grenier(besoin, &par_sorte);
        assert!((c.stock[Sorte::Poisson.index()] - 0.8).abs() < 1e-9);
        assert!((c.stock[Sorte::Ble.index()] - 0.02 * besoin * 0.98).abs() < 1e-9);
        // Famine : il comble le manque, le poisson d'abord ; la ville n'est pas affamée
        c.stock[Sorte::Ble.index()] = besoin;
        let y = Rendements::new(0.6 * besoin, 1.0, 0.0);
        assert!((c.nourriture_avec_grenier(y.nourriture) - besoin).abs() < 1e-9);
        assert!(c.croissance(y).famine == 0.0);
        let r = c.passer_le_tour(y, 0, [y.nourriture, 0.0, 0.0], &[0.0; N_SORTES]);
        assert!(!r.starved && (r.puise - 0.4 * besoin).abs() < 1e-9);
        assert_eq!(c.stock[Sorte::Poisson.index()], 0.0, "le poisson d'abord");
        assert!((c.stock[Sorte::Ble.index()] - (besoin - (0.4 * besoin - 0.8)) * 0.98).abs() < 1e-6);
        // Grenier vide : la famine revient
        c.stock.iter_mut().for_each(|q| *q = 0.0);
        assert!(c.croissance(y).famine < 0.0);
    }

    #[test]
    fn file_vide_projets_et_rappel() {
        let y = Rendements::new(1.1, 4.0, 1.0);
        // Rien à produire : la production est perdue et c'est signalé
        let mut city = CityState::new("Essai".into());
        assert!(city.process_turn(y).inactive);
        // Peupler la ville : la croissance augmente, même quand la nourriture est déjà au plafond
        let mut sans = CityState::new("Sans".into());
        sans.mettre_en_file(Production::Unit(UnitKind::Settler));
        let mut avec = CityState::new("Avec".into());
        avec.projet = Some(Projet::Peupler);
        assert!(avec.variation(y) > sans.variation(y));
        let rassasie = Rendements::new(3.0, 4.0, 1.0);
        assert_eq!(sans.variation(rassasie), 15, "plafond de la croissance naturelle");
        assert_eq!(avec.variation(rassasie), 19, "+0,1 % × 4 production / 1 de besoin");
        let riche = Rendements::new(3.0, 40.0, 1.0);
        assert_eq!(avec.variation(riche), 30, "au plus le double de la croissance naturelle");
        // jamais pour le besoin : en famine, le projet n'aide pas
        let faim = Rendements::new(0.5, 4.0, 1.0);
        assert_eq!(avec.variation(faim), sans.variation(faim));
        // Rappel tous les 10 tours
        let rappels: Vec<usize> = (1..=25).filter(|_| avec.process_turn(y).rappel_projet).collect();
        assert_eq!(rappels, vec![10, 20]);
        // Une construction reprend la main : le compteur repart
        avec.mettre_en_file(Production::Unit(UnitKind::Militia));
        avec.process_turn(y);
        assert_eq!(avec.tours_en_projet, 0);
    }

    #[test]
    fn l_equilibre_releve_la_limite_et_adoucit_les_famines() {
        assert_eq!((facteur_equilibre(0), facteur_equilibre(1), facteur_equilibre(2), facteur_equilibre(3)), (1.0, 1.0, 1.1, 1.2));
        let mut c = CityState::new("Essai".into());
        c.habitants = 12_000;
        // Un seul type : besoin 12 ; trois types : 10 (1 point nourrit 1 200 habitants)
        assert_eq!(c.besoin_nourriture(), 12.0);
        c.types_nourriture = 3;
        assert!((c.besoin_nourriture() - 10.0).abs() < 1e-9);
        // Famine adoucie de moitié avec trois types
        let faim = Rendements::new(5.0, 1.0, 0.0);
        let avec = c.croissance(faim).famine;
        c.types_nourriture = 1;
        let sans = c.croissance(faim).famine;
        assert!(avec < 0.0 && sans < avec, "{avec} {sans}");
        // Zone d'équilibre : pas de croissance juste au-dessus du besoin (pas d'oscillation)
        assert_eq!(c.croissance(Rendements::new(12.3, 0.0, 0.0)).naturelle, 0.0);
        assert!(c.croissance(Rendements::new(13.0, 0.0, 0.0)).naturelle > 0.0);
        // Le tour compte les types sur ce que la ville a mangé
        c.passer_le_tour(Rendements::new(13.0, 1.0, 0.0), 0, [9.0, 2.0, 2.0], &[0.0; intendant::N_SORTES]);
        assert_eq!(c.types_nourriture, 3);
    }

    #[test]
    fn le_bien_etre_suit_sa_visee_et_module_la_croissance() {
        let rassasie = Rendements::new(3.0, 40.0, 1.0);
        let mut c = CityState::new("Essai".into());
        // Neutre à 60 : la croissance naturelle ne change pas
        assert_eq!((c.bien_etre, c.variation(rassasie)), (60.0, 15));
        assert_eq!(c.humeur(), Humeur::Satisfaite);
        // Famine : visée 45, le bien-être en prend un cinquième par tour
        let faim = Rendements::new(0.5, 4.0, 1.0);
        assert_eq!(c.bien_etre_vise(faim, 0), 45.0);
        assert!(c.process_turn(faim).starved);
        assert!((c.bien_etre - 57.0).abs() < 1e-9);
        // Chômage : −1 par tranche de 2 % des actifs (20 % → −10), au plus −20
        let c = CityState::new("Essai".into());
        assert_eq!(c.bien_etre_vise(rassasie, 100), 50.0);
        assert_eq!(c.bien_etre_vise(rassasie, 500), 40.0);
        // Grands jeux : 1 point pour 5 de production, au plus +15
        let mut jeux = CityState::new("Jeux".into());
        jeux.projet = Some(Projet::GrandsJeux);
        assert!(Projet::GrandsJeux.disponible());
        assert_eq!(jeux.bien_etre_vise(Rendements::new(3.0, 20.0, 1.0), 0), 64.0);
        assert_eq!(jeux.bien_etre_vise(rassasie, 0), 68.0);
        assert_eq!(jeux.bien_etre_vise(Rendements::new(3.0, 100.0, 1.0), 0), 75.0);
        // Effet : × (1 + (bien-être − 60) / 100) sur la croissance naturelle
        let mut heureuse = CityState::new("Heureuse".into());
        heureuse.bien_etre = 80.0;
        assert_eq!(heureuse.variation(rassasie), 18);
        assert_eq!(heureuse.humeur(), Humeur::Prospere);
        heureuse.bien_etre = 20.0;
        assert_eq!((heureuse.variation(rassasie), heureuse.humeur()), (9, Humeur::Mecontente));
    }

    #[test]
    fn la_salubrite_depend_du_lieu_de_la_taille_et_du_puits() {
        let rassasie = Rendements::new(3.0, 4.0, 1.0);
        let mut c = CityState::new("Essai".into());
        assert_eq!(c.salubrite(), 50.0);
        // Marais en été chaud : 35, sous le seuil de 40 : les maladies emportent 0,125 % par tour
        c.site.marais = 1.0;
        assert_eq!(c.salubrite(), 35.0);
        assert_eq!(c.variation(rassasie), 14);
        let r = c.process_turn(rassasie);
        assert!(!r.starved, "des maladies, pas une famine");
        // Le puits : +10, au-dessus du seuil
        assert!(c.mettre_en_file(Production::Puits));
        assert!(!c.mettre_en_file(Production::Puits), "un seul puits en file");
        let built: Vec<Production> = (0..10).filter_map(|_| c.process_turn(Rendements::new(3.0, 5.0, 1.0)).built).collect();
        assert_eq!(built, vec![Production::Puits]);
        assert!(c.puits && !c.mettre_en_file(Production::Puits), "un puits par ville");
        assert_eq!(c.salubrite(), 45.0);
        // Taille : −1 par millier au-delà de 5 000, le double sans eau courante
        let mut grande = CityState::new("Grande".into());
        grande.habitants = 15_000;
        assert_eq!(grande.salubrite(), 30.0);
        grande.site.eau_courante = true;
        assert_eq!(grande.salubrite(), 55.0);
        grande.site.lac = true;
        grande.site.paturages = 14;
        assert_eq!(grande.salubrite(), 50.0, "+5 de lac, −10 de bétail au plus");
    }

    #[test]
    fn le_site_voit_le_lac_et_les_paturages() {
        let mut map = WorldMap::filled(12, 12, Terrain::Grassland);
        let centre = Hex::from_offset(6, 6);
        let voisine = map.neighbors(centre)[0];
        let loin = Hex::from_offset(6, 9);
        map.set(loin, Terrain::Lake);
        let territoire: Vec<Hex> = centre.range(1).collect();
        let mut amenagements = std::collections::BTreeMap::new();
        amenagements.insert(voisine, Amenagement::Paturage);
        amenagements.insert(loin, Amenagement::Paturage);
        let site = site_de_la_ville(&map, centre, &territoire, &amenagements);
        assert_eq!(site, Site { eau_courante: false, lac: false, marais: 0.0, paturages: 1 });
        map.set(voisine, Terrain::Lake);
        assert!(site_de_la_ville(&map, centre, &territoire, &amenagements).lac);
    }

    #[test]
    fn la_file_refuse_un_doublon_de_case() {
        let mut city = CityState::new("Essai".into());
        let h = Hex::new(3, 4);
        assert!(city.mettre_en_file(Production::Amenagement { hex: h, kind: Amenagement::Champ, cost: 20 }));
        assert!(!city.mettre_en_file(Production::Amenagement { hex: h, kind: Amenagement::Paturage, cost: 15 }));
        assert!(city.mettre_en_file(Production::Unit(UnitKind::Militia)));
        assert!(city.mettre_en_file(Production::Unit(UnitKind::Militia)));
        assert_eq!(city.tours_pour(2, 5.0), Some(10));
    }

    #[test]
    fn l_historique_garde_les_quarante_derniers_tours() {
        let mut c = CityState::new("Essai".into());
        for i in 0..45 {
            c.process_turn(Rendements::new(2.0, f64::from(i), 1.0));
        }
        assert_eq!(c.historique.len(), HISTORIQUE_TOURS);
        assert_eq!(c.historique.back().map(|r| r.production), Some(44.0));
        assert_eq!(c.historique.front().map(|r| r.production), Some(5.0));
        assert_eq!(c.historique.back().map(|r| r.habitants), Some(c.habitants));
    }

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

    /// Bilan du grenier (à lancer à la main, `SEEDS=777,2`) : des villes de 19 cases (champs et
    /// pâturages, un sur trois), grenier dès la fondation ; habitants et stock (en tours de besoin)
    /// au fil des tours, avec l'objectif de nourriture par défaut ou « Excédent ».
    #[test]
    #[ignore]
    fn bilan_grenier() {
        use crate::intendant::Amenagement;
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or(vec![777, 2]);
        for seed in seeds {
            let map = WorldMap::generate(192, 111, seed);
            let centres: Vec<Hex> = map
                .hexes()
                .filter(|&h| matches!(map.get(h), Some(Terrain::Grassland | Terrain::Plains | Terrain::Savanna)) && map.relief(h) == Relief::Flat)
                .step_by(97)
                .take(8)
                .collect();
            for objectif in [intendant::ObjectifNourriture::CroissanceMaximale, intendant::ObjectifNourriture::Excedent] {
                let mut lignes = Vec::new();
                for &centre in &centres {
                    let territoire: Vec<Hex> = map.range(centre, 2).map(|h| map.canonical(h)).filter(|&h| map.contains(h)).collect();
                    let mut amen = std::collections::BTreeMap::new();
                    for (k, &h) in territoire.iter().enumerate().filter(|(_, h)| **h != centre) {
                        let a = if k % 3 == 0 { Amenagement::Paturage } else { Amenagement::Champ };
                        if a.possible(&map, h).is_ok() {
                            amen.insert(h, a);
                        }
                    }
                    let cases = cases_de_la_ville(&map, centre, &territoire, &HashSet::new(), &amen);
                    let mut st = CityState::new("Bilan".into());
                    st.grenier = true;
                    st.projet = Some(Projet::Peupler);
                    st.consignes.objectif = objectif;
                    let mut releves = Vec::new();
                    for t in 1..=200 {
                        let r = repartition_ville(&map, centre, &st, &cases);
                        st.passer_le_tour(r.rendements, r.chomeurs, r.par_type, &r.par_sorte);
                        if [25, 50, 100, 200].contains(&t) {
                            releves.push(format!("t{t} {} hab. {:.1}", milliers(st.habitants), st.stock_total() / st.besoin_nourriture()));
                        }
                    }
                    lignes.push(releves.join(" · "));
                }
                println!("graine {seed}, objectif {objectif:?} :");
                for l in lignes {
                    println!("  {l}");
                }
            }
        }
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
        assert_eq!(variation_des_habitants(1_000, 2.0), 15);
        assert_eq!(variation_des_habitants(10_000, 15.0), 150);
        // Juste le besoin : pas de croissance ; un surplus de 25 % : la moitié
        assert_eq!(variation_des_habitants(10_000, 10.0), 0);
        assert_eq!(variation_des_habitants(20_000, 25.0), 150);
        // Famine : 2 % pour un léger manque, 5 % sans rien
        assert_eq!(variation_des_habitants(10_000, 0.0), -500);
        assert!((-260..=-200).contains(&variation_des_habitants(10_000, 9.0)));
        let mut city = CityState::new("Test".into());
        assert_eq!(city.habitants, HABITANTS_COLON);
        let y = Rendements::from(Yields::new(6, 5, 2));
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
        // File vide : rien n'est produit
        assert!(built.is_empty());
        // 50 de production sur 10 tours, trois milices en file (15 chacune) : le surplus passe à la suivante
        let mut city = CityState::new("Test".into());
        for _ in 0..3 {
            city.mettre_en_file(Production::Unit(UnitKind::Militia));
        }
        let built: Vec<Production> = (0..10).filter_map(|_| city.process_turn(y).built).collect();
        assert_eq!(built, vec![Production::Unit(UnitKind::Militia); 3]);
        assert!(city.file.is_empty());
        // Une famine ne descend pas sous le plancher
        let mut affame = CityState::new("Faim".into());
        for _ in 0..100 {
            affame.process_turn(Rendements::from(Yields::new(0, 0, 0)));
        }
        assert_eq!(affame.habitants, HABITANTS_PLANCHER);
    }

    #[test]
    fn colon_exige_deux_mille_habitants() {
        let mut city = CityState::new("Test".into());
        city.mettre_en_file(Production::Unit(UnitKind::Settler));
        city.file[0].investi = 100.0;
        assert!(city.colon_bloque());
        let r = city.process_turn(Rendements::from(Yields::new(1, 0, 0)));
        assert_eq!(r.built, None);
        assert!(!r.colon_attend, "payé depuis longtemps : déjà dit");
        city.file[0].investi = Production::Unit(UnitKind::Settler).cost() as f64 - 1.0;
        assert!(city.process_turn(Rendements::from(Yields::new(1, 3, 0))).colon_attend, "payé ce tour-ci");
        city.habitants = 2 * HABITANTS_COLON;
        assert_eq!(city.process_turn(Rendements::from(Yields::new(2, 0, 0))).built, Some(Production::Unit(UnitKind::Settler)));
        assert_eq!(city.habitants, HABITANTS_COLON);
        assert_eq!(city.historique.back().unwrap().habitants, HABITANTS_COLON, "le départ compte dans le solde");
    }

    #[test]
    fn choix_des_cases() {
        let mut map = WorldMap::filled(10, 10, Terrain::Desert);
        let center = Hex::from_offset(5, 5);
        let best = center.neighbors()[2];
        map.set(best, Terrain::Grassland);
        let territory: Vec<Hex> = center.range(1).collect();
        let city = CityState::new("Essai".into());
        let aucun = std::collections::BTreeMap::new();
        let cases = cases_de_la_ville(&map, center, &territory, &HashSet::new(), &aucun);
        assert_eq!(cases.len(), 6, "hors centre");
        let r = repartition_ville(&map, center, &city, &cases);
        assert_eq!(r.cases, vec![(best, 500)], "1 000 habitants : 500 actifs, à la prairie");
        let taken = HashSet::from([best]);
        let cases = cases_de_la_ville(&map, center, &territory, &taken, &aucun);
        assert!(cases.iter().all(|c| c.hex != best));
        // Un champ double la nourriture et la capacité passe à 1 500
        let champ = std::collections::BTreeMap::from([(best, Amenagement::Champ)]);
        let cases = cases_de_la_ville(&map, center, &territory, &HashSet::new(), &champ);
        let c = cases.iter().find(|c| c.hex == best).unwrap();
        assert_eq!((c.capacite, c.max.nourriture), (1_500, 4.0));
    }

    #[test]
    fn la_prevision_refait_la_repartition_a_chaque_tour() {
        let mut map = WorldMap::filled(10, 10, Terrain::Grassland);
        let center = Hex::from_offset(5, 5);
        let territory: Vec<Hex> = center.range(1).collect();
        let city = CityState::new("Essai".into());
        let cases = cases_de_la_ville(&map, center, &territory, &HashSet::new(), &Default::default());
        let p = prevoir(&map, center, &city, &cases, 600);
        // Le premier tour est celui de la répartition d'aujourd'hui
        assert_eq!(p[0].rendements, repartition_ville(&map, center, &city, &cases).rendements);
        // Les cases se remplissent : la croissance finit par s'arrêter, puis plus rien ne bouge
        let arret = arret_de_croissance(&p).expect("arrêt de la croissance");
        assert!(p[arret - 1].habitants > city.habitants);
        assert!(p.last().unwrap().habitants <= p[arret - 1].habitants * 11 / 10);
        assert!(p.last().unwrap().chomeurs > 0, "toutes les cases pleines");
        assert_eq!(tours_jusqu_a(&p, city.habitants), Some(1));
        assert_eq!(tours_jusqu_a(&p, u64::MAX), None);
        // Sans nourriture hors du centre : la population baisse dès le premier tour
        for h in center.range(1) {
            map.set(h, Terrain::Desert);
        }
        let cases = cases_de_la_ville(&map, center, &territory, &HashSet::new(), &Default::default());
        let mut grosse = city.clone();
        grosse.habitants = 20_000;
        let p = prevoir(&map, center, &grosse, &cases, 200);
        assert_eq!(arret_de_croissance(&p), Some(1));
        // (le centre donne les trois types : repas équilibrés, 1 point nourrit 1 200 habitants)
        assert!((2_300..=2_400).contains(&p.last().unwrap().habitants), "le centre nourrit 2 400 habitants : {}", p.last().unwrap().habitants);
    }

    #[test]
    fn la_culture_fait_gagner_la_meilleure_case_voisine() {
        let mut map = WorldMap::filled(12, 12, Terrain::Desert);
        let centre = Hex::from_offset(6, 6);
        let ville = Entity::from_raw_u32(1).unwrap();
        let mut territoire: HashMap<Hex, Entity> = centre.range(1).map(|h| (h, ville)).collect();
        // Seuils : 10, 16, 22…
        assert_eq!((seuil_culture(0), seuil_culture(1), seuil_culture(2)), (10.0, 16.0, 22.0));
        assert_eq!(prix_case(2), 40);
        // 12 cases voisines, toutes à personne ; une prairie parmi les déserts : c'est elle
        assert_eq!(cases_gagnables(&map, ville, &territoire).len(), 12);
        let prairie = *centre.range(2).filter(|h| map.distance(centre, *h) == 2).collect::<Vec<_>>().last().unwrap();
        map.set(prairie, Terrain::Grassland);
        assert_eq!(case_par_la_culture(&map, centre, ville, &territoire), Some(prairie));
        // Une case d'une autre ville n'est pas à prendre
        territoire.insert(prairie, Entity::from_raw_u32(2).unwrap());
        assert_ne!(case_par_la_culture(&map, centre, ville, &territoire), Some(prairie));
        // Culture du tour : 1, plus 1 pour 5 de production avec le projet (file vide)
        let mut city = CityState::new("Essai".into());
        let y = Rendements::new(3.0, 10.0, 0.0);
        assert_eq!(city.culture_du_tour(y), 1.0);
        city.projet = Some(Projet::Culture);
        assert!((city.culture_du_tour(y) - 3.0).abs() < 1e-9);
        assert_eq!(city.tours_avant_case(3.0), Some(4));
        city.process_turn(y);
        assert!((city.culture - 3.0).abs() < 1e-9);
    }

    #[test]
    fn la_prevision_fait_avancer_la_file() {
        let map = WorldMap::filled(10, 10, Terrain::Grassland);
        let center = Hex::from_offset(5, 5);
        let territory: Vec<Hex> = center.range(1).collect();
        let cases = cases_de_la_ville(&map, center, &territory, &HashSet::new(), &Default::default());
        let mut city = CityState::new("Essai".into());
        city.habitants = 4_000;
        let sans = prevoir(&map, center, &city, &cases, 60);
        // Un champ en file : achevé, il donne plus de nourriture pour les tours suivants
        let h = territory.iter().copied().find(|h| *h != center).unwrap();
        city.mettre_en_file(Production::Amenagement { hex: h, kind: Amenagement::Champ, cost: 20 });
        let avec = prevoir(&map, center, &city, &cases, 60);
        assert!(avec.last().unwrap().habitants > sans.last().unwrap().habitants, "{} ≤ {}", avec.last().unwrap().habitants, sans.last().unwrap().habitants);
        // Un colon en file : 1 000 habitants partent quand il est formé
        let mut colon = CityState::new("Colon".into());
        colon.habitants = 3_000;
        colon.mettre_en_file(Production::Unit(UnitKind::Settler));
        colon.file[0].investi = Production::Unit(UnitKind::Settler).cost() as f64;
        let p = prevoir(&map, center, &colon, &cases, 2);
        assert!(p[0].habitants < 3_000 - HABITANTS_COLON + 100, "{}", p[0].habitants);
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
