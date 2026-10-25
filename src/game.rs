//! État du jeu côté ECS : composants, ressources, initialisation et fin de tour.

use crate::explore::{self, ExploreOutcome, ExploreWorld};
use crate::hex::Hex;
use crate::hostile::{self, Blocked, Danger, EnemyZoc, Hostile, HostileTurnPending, KnownVillages, Occupancy, WorldSeed};
use crate::map::WorldMap;
use crate::rules::{self, CityState, MoveOutcome, UnitKind, UnitState};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub const MAP_WIDTH: i32 = 192;
pub const MAP_HEIGHT: i32 = 111;

const CITY_NAMES: &[&str] = &[
    "Paris", "Lyon", "Marseille", "Bordeaux", "Toulouse", "Nantes", "Lille", "Strasbourg",
    "Rennes", "Montpellier", "Nice", "Dijon", "Reims", "Rouen", "Tours", "Limoges",
];

// ---------------------------------------------------------------- Composants

/// Position d'une entité sur la grille.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HexPos(pub Hex);

#[derive(Component, Debug)]
pub struct Unit(pub UnitState);

#[derive(Component, Debug)]
pub struct City {
    pub state: CityState,
    pub worked: Vec<Hex>,
}

// ---------------------------------------------------------------- Ressources

#[derive(Resource)]
pub struct GameMap(pub WorldMap);

#[derive(Resource, Default)]
pub struct GameState {
    pub turn: u32,
    pub gold: i32,
    pub founded_cities: usize,
}

/// Brouillard de guerre : cases déjà explorées et cases vues ce tour-ci.
#[derive(Resource, Default)]
pub struct Fog {
    pub explored: HashSet<Hex>,
    pub visible: HashSet<Hex>,
}

/// Brouillard tel qu'il est affiché : celui du joueur, ou la carte entière en mode debug.
/// La logique du jeu (chemins, exploration, IA) lit toujours `Fog`, la connaissance réelle du
/// joueur ; seul l'affichage lit `ShownFog`.
#[derive(Resource, Default)]
pub struct ShownFog {
    pub explored: HashSet<Hex>,
    pub visible: HashSet<Hex>,
}

/// Mode debug : révèle toute la carte à l'affichage (menu Échap).
#[derive(Resource, Default)]
pub struct DebugMode(pub bool);

/// Propriétaire de chaque case revendiquée.
#[derive(Resource, Default)]
pub struct Territory(pub HashMap<Hex, Entity>);

#[derive(Resource, Default)]
pub struct Selection(pub Option<Entity>);

/// Journal des événements, affiché dans l'interface.
#[derive(Resource, Default)]
pub struct GameLog(pub Vec<String>);

impl GameLog {
    pub fn push(&mut self, msg: impl Into<String>) {
        self.0.push(msg.into());
        if self.0.len() > 6 {
            self.0.remove(0);
        }
    }
}

/// Demande de fin de tour (posée par le clavier ou le bouton).
#[derive(Resource, Default)]
pub struct EndTurnRequested(pub bool);

/// Systèmes de résolution du tour (exécutés après les commandes du joueur).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EndTurnSet;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        // Graine imposée par la variable d'environnement CIVBUILDER_SEED (pour rejouer une carte)
        let seed = std::env::var("CIVBUILDER_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or_else(rand::random::<u32>);
        // Une planète entière, d'un pôle à l'autre, avec des cases de 40 km
        // (taille imposée par CIVBUILDER_MAP, par exemple « 128x74 »)
        let (width, height) = std::env::var("CIVBUILDER_MAP")
            .ok()
            .and_then(|s| {
                let (w, h) = s.split_once(['x', 'X'])?;
                Some((w.trim().parse::<i32>().ok()?, h.trim().parse::<i32>().ok()?))
            })
            .filter(|&(w, h)| w >= 16 && h >= 10)
            .unwrap_or((MAP_WIDTH, MAP_HEIGHT));
        let region = crate::climate::Region::planet(crate::climate::CELL_KM, height);
        let started = std::time::Instant::now();
        let map = WorldMap::generate_in(width, height, seed, region);
        info!(
            "Carte générée en {:.1} s (graine {seed}, {width} × {height} cases de {} km)",
            started.elapsed().as_secs_f32(),
            region.cell_km
        );
        app.insert_resource(GameMap(map))
            .insert_resource(WorldSeed(seed as u64))
            .insert_resource(GameState { turn: 1, gold: 10, founded_cities: 0 })
            .init_resource::<Fog>()
            .init_resource::<ShownFog>()
            .init_resource::<DebugMode>()
            .init_resource::<Territory>()
            .init_resource::<Selection>()
            .init_resource::<GameLog>()
            .init_resource::<EndTurnRequested>()
            .add_systems(Startup, spawn_starting_units)
            .add_systems(
                Update,
                (
                    end_turn,
                    hostile::hostile_turn,
                    hostile::visit_sites,
                    refresh_worked_tiles,
                    update_fog,
                    hostile::update_knowledge,
                )
                    .chain()
                    .in_set(EndTurnSet),
            )
            .add_systems(Update, sync_shown_fog.after(EndTurnSet));
    }
}

/// Crée une unité à une position donnée (les visuels sont ajoutés par le module de rendu).
pub fn spawn_unit(commands: &mut Commands, kind: UnitKind, at: Hex) -> Entity {
    commands.spawn((Unit(UnitState::new(kind)), HexPos(at))).id()
}

fn spawn_starting_units(
    mut commands: Commands,
    map: Res<GameMap>,
    mut selection: ResMut<Selection>,
    mut log: ResMut<GameLog>,
) {
    let start = map.0.find_start().expect("aucun emplacement de départ");
    let settler = spawn_unit(&mut commands, UnitKind::Settler, start);
    // Milice et éclaireur sur des cases voisines praticables
    let mut free = map.0.neighbors(start).into_iter().filter(|&h| map.0.get(h).and_then(|t| t.move_cost()).is_some());
    spawn_unit(&mut commands, UnitKind::Militia, free.next().unwrap_or(start));
    spawn_unit(&mut commands, UnitKind::Scout, free.next().unwrap_or(start));
    selection.0 = Some(settler);
    log.push("Bienvenue ! Fondez votre capitale avec le colon (touche B).");
}

/// Fonde une ville avec le colon `settler`. Retourne un message d'erreur si impossible.
pub fn found_city(
    commands: &mut Commands,
    settler: Entity,
    at: Hex,
    map: &WorldMap,
    existing: &[Hex],
    state: &mut GameState,
    territory: &mut Territory,
) -> Result<Entity, &'static str> {
    rules::can_found_city(map, at, existing)?;
    let name = CITY_NAMES
        .get(state.founded_cities)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Ville {}", state.founded_cities + 1));
    state.founded_cities += 1;
    commands.entity(settler).despawn();
    let city = commands
        .spawn((City { state: CityState::new(name), worked: Vec::new() }, HexPos(at)))
        .id();
    claim_territory(&mut territory.0, city, at, 1, map);
    Ok(city)
}

pub(crate) fn claim_territory(territory: &mut HashMap<Hex, Entity>, city: Entity, center: Hex, radius: i32, map: &WorldMap) {
    for h in map.range(center, radius).filter(|&h| map.contains(h)) {
        territory.entry(h).or_insert(city);
    }
}

/// Fait avancer une unité le long de son ordre en découvrant le terrain
/// (le trajet est recalculé à chaque pas avec la connaissance à jour).
/// `blocked` : cases connues infranchissables (ennemis visibles, unités alliées) ;
/// `occupied` : occupation réelle de toutes les cases, tenue à jour après le déplacement ;
/// `zoc` : zones de contrôle ennemies (arrêt en y entrant).
pub fn move_unit(
    unit: &mut UnitState,
    pos: &mut Mut<HexPos>,
    map: &WorldMap,
    fog: &mut Fog,
    log: &mut GameLog,
    blocked: &HashSet<Hex>,
    occupied: &mut HashSet<Hex>,
    zoc: &HashSet<Hex>,
) {
    if unit.path.is_empty() {
        return;
    }
    let mut hex = pos.0;
    match unit.advance_exploring(&mut hex, map, &mut fog.explored, blocked, occupied, zoc) {
        MoveOutcome::Moved => {}
        MoveOutcome::Unreachable => {
            log.push(format!("{} : destination inaccessible, ordre annulé.", unit.kind.name()));
        }
        MoveOutcome::TooLong => {
            log.push(format!("{} : trajet bien plus long que prévu, ordre suspendu.", unit.kind.name()));
        }
    }
    relocate(pos, hex, occupied);
}

/// Déplace une entité et met à jour l'occupation des cases.
pub fn relocate(pos: &mut Mut<HexPos>, to: Hex, occupied: &mut HashSet<Hex>) {
    if pos.0 != to {
        occupied.remove(&pos.0);
        occupied.insert(to);
        pos.0 = to;
    }
}

/// Union des cases bloquées par l'ennemi visible et par les unités alliées.
pub fn movement_blocked(hostile: &HashSet<Hex>, own: &HashSet<Hex>) -> HashSet<Hex> {
    hostile.union(own).copied().collect()
}

/// Case libre pour faire apparaître une unité : `at` si libre, sinon une voisine franchissable.
pub fn free_tile_near(map: &WorldMap, at: Hex, occupied: &HashSet<Hex>) -> Option<Hex> {
    std::iter::once(at)
        .chain(map.neighbors(at))
        .find(|h| !occupied.contains(h) && map.get(*h).and_then(|t| t.move_cost()).is_some())
}

/// Cases réservées par les explorateurs autres que `except` (cibles déjà choisies).
pub fn reserved_by_others<'a>(map: &WorldMap, units: impl IntoIterator<Item = (Entity, &'a UnitState)>, except: Entity) -> HashSet<Hex> {
    explore::reserved_by(
        map.wrap(),
        units
            .into_iter()
            .filter(|(e, u)| *e != except && u.auto_explore)
            .filter_map(|(_, u)| u.explore_target.map(|t| (t, u.kind.sight()))),
    )
}

/// Un tour d'exploration automatique ; l'arrête quand il n'y a plus rien à découvrir.
pub fn explore_unit(
    unit: &mut UnitState,
    pos: &mut Mut<HexPos>,
    world: &ExploreWorld,
    occupied: &mut HashSet<Hex>,
    fog: &mut Fog,
    log: &mut GameLog,
) {
    let mut hex = pos.0;
    let outcome = explore::explore_step(unit, &mut hex, &world.occupied(occupied), &mut fog.explored);
    if outcome == ExploreOutcome::Finished {
        unit.auto_explore = false;
        log.push(format!("{} : plus rien d'accessible à explorer.", unit.kind.name()));
    }
    relocate(pos, hex, occupied);
}

/// Recalcule les cases exploitées de toutes les villes (évite les doublons entre villes).
pub fn assign_worked_tiles(map: &WorldMap, territory: &Territory, cities: &mut [(Entity, Hex, Mut<City>)]) {
    let mut taken: HashSet<Hex> = HashSet::new();
    for (entity, center, city) in cities.iter_mut() {
        let owned: Vec<Hex> = territory.0.iter().filter(|(_, e)| *e == entity).map(|(h, _)| *h).collect();
        let worked = rules::choose_worked_tiles(map, *center, city.state.population, &owned, &taken);
        taken.extend(worked.iter().copied());
        city.worked = worked;
    }
}

fn end_turn(
    mut commands: Commands,
    mut requested: ResMut<EndTurnRequested>,
    map: Res<GameMap>,
    mut state: ResMut<GameState>,
    mut territory: ResMut<Territory>,
    mut log: ResMut<GameLog>,
    mut fog: ResMut<Fog>,
    blocked: Res<Blocked>,
    occupancy: Res<Occupancy>,
    enemy_zoc: Res<EnemyZoc>,
    danger: Res<Danger>,
    villages: Res<KnownVillages>,
    mut hostile_turn: ResMut<HostileTurnPending>,
    mut cities: Query<(Entity, &HexPos, &mut City)>,
    mut units: Query<(Entity, &mut Unit, &mut HexPos), (Without<City>, Without<Hostile>)>,
) {
    if !std::mem::take(&mut requested.0) {
        return;
    }
    let map = &map.0;

    // 1) Économie des villes
    let mut list: Vec<(Entity, Hex, Mut<City>)> = cities.iter_mut().map(|(e, p, c)| (e, p.0, c)).collect();
    list.sort_by_key(|(e, _, _)| *e);
    assign_worked_tiles(map, &territory, &mut list);
    let mut new_units = Vec::new();
    for (entity, center, city) in list.iter_mut() {
        let yields = rules::city_yields(map, *center, &city.worked);
        let report = city.state.process_turn(yields);
        state.gold += report.gold;
        let name = city.state.name.clone();
        if report.grew {
            log.push(format!("{name} gagne un habitant."));
            claim_territory(&mut territory.0, *entity, *center, rules::territory_radius(city.state.population), map);
        }
        if report.starved {
            log.push(format!("{name} souffre de famine !"));
        }
        if let Some(kind) = report.built {
            new_units.push((*entity, kind, *center, name));
        }
    }
    drop(list);

    // 2) Unités : soins, PM restaurés, puis poursuite des ordres de déplacement.
    // Les unités jouent l'une après l'autre (une unité par case) ; les explorateurs évitent
    // les cibles déjà choisies par les autres.
    let mut occupied = occupancy.all.clone();
    let mut own = occupancy.own.clone();
    let threats: HashSet<Hex> = occupancy.all.difference(&occupancy.own).copied().collect();
    let mut order: Vec<Entity> = units.iter().map(|(e, _, _)| e).collect();
    order.sort();
    for entity in order {
        let reserved = reserved_by_others(map, units.iter().map(|(e, u, _)| (e, &u.0)), entity);
        let Ok((_, mut unit, mut pos)) = units.get_mut(entity) else { continue };
        let friendly = territory.0.contains_key(&pos.0);
        unit.0.start_turn(friendly);
        let before = pos.0;
        let blocked_now = movement_blocked(&blocked.0, &own);
        if unit.0.auto_explore {
            let world = ExploreWorld::new(map)
                .reserved(&reserved)
                .blocked(&blocked_now)
                .villages(&villages.0)
                .danger(&danger.0)
                .threats(&threats);
            explore_unit(&mut unit.0, &mut pos, &world, &mut occupied, &mut fog, &mut log);
        } else {
            move_unit(&mut unit.0, &mut pos, map, &mut fog, &mut log, &blocked_now, &mut occupied, &enemy_zoc.0);
        }
        if pos.0 != before {
            own.remove(&before);
            own.insert(pos.0);
        }
    }

    // 3) Unités produites : sur la ville, ou une case voisine libre ; sinon la production attend
    for (city_entity, kind, center, name) in new_units {
        match free_tile_near(map, center, &occupied) {
            Some(at) => {
                occupied.insert(at);
                spawn_unit(&mut commands, kind, at);
                log.push(format!("{name} a produit : {}.", kind.name()));
            }
            None => {
                if let Ok((_, _, mut city)) = cities.get_mut(city_entity) {
                    city.state.production_stock += kind.cost();
                    if kind == UnitKind::Settler {
                        city.state.population += 1;
                    }
                }
                log.push(format!("{name} : pas de place pour {}, la production attend.", kind.name()));
            }
        }
    }

    // 4) Puis la faction hostile joue (système suivant)
    hostile_turn.0 = true;
    state.turn += 1;
}

/// Maintient à jour les cases exploitées (nouvelle ville, territoire étendu, nouveau tour).
fn refresh_worked_tiles(
    map: Res<GameMap>,
    state: Res<GameState>,
    territory: Res<Territory>,
    mut cities: Query<(Entity, &HexPos, &mut City)>,
) {
    // Parcourir des `Mut` sans les déréférencer en écriture ne les marque pas « modifiés ».
    let any_added = cities.iter_mut().any(|(_, _, c)| c.is_added());
    if !any_added && !territory.is_changed() && !state.is_changed() {
        return;
    }
    let mut list: Vec<(Entity, Hex, Mut<City>)> = cities.iter_mut().map(|(e, p, c)| (e, p.0, c)).collect();
    list.sort_by_key(|(e, _, _)| *e);
    assign_worked_tiles(&map.0, &territory, &mut list);
}

/// Recalcule les cases visibles à partir des unités et des villes du joueur.
/// Brouillard affiché : copie de celui du joueur, ou toute la carte en mode debug. En quittant le
/// mode debug, on revient exactement à ce que le joueur a exploré.
fn sync_shown_fog(fog: Res<Fog>, debug: Res<DebugMode>, map: Res<GameMap>, mut shown: ResMut<ShownFog>) {
    if !fog.is_changed() && !debug.is_changed() {
        return;
    }
    // En mode debug, toute la carte est déjà affichée : un changement du brouillard du joueur ne
    // change rien à l'écran (sans ce garde-fou, chaque tour redessinait toute la carte)
    if debug.0 && !debug.is_changed() && shown.explored.len() == map.0.hexes().count() {
        return;
    }
    if debug.0 {
        let all: HashSet<Hex> = map.0.hexes().collect();
        shown.visible = all.clone();
        shown.explored = all;
    } else {
        shown.explored = fog.explored.clone();
        shown.visible = fog.visible.clone();
    }
}

fn update_fog(
    map: Res<GameMap>,
    mut fog: ResMut<Fog>,
    units: Query<(&Unit, &HexPos), Without<Hostile>>,
    cities: Query<&HexPos, With<City>>,
    changed: Query<(), Or<(Changed<HexPos>, Added<HexPos>)>>,
    mut removed: RemovedComponents<HexPos>,
) {
    if changed.is_empty() && removed.read().count() == 0 {
        return;
    }
    let mut visible = HashSet::new();
    for (unit, pos) in &units {
        visible.extend(map.0.range(pos.0, unit.0.kind.sight()));
    }
    for pos in &cities {
        visible.extend(map.0.range(pos.0, 3));
    }
    fog.explored.extend(visible.iter().copied());
    fog.visible = visible;
}
