//! Villages et faction hostile (côté ECS).
//!
//! - Les villages déclenchent un événement à la première visite d'une unité du joueur.
//! - Les camps hostiles et leurs guerriers forment une seule faction, hostile envers tout le monde.
//!   Les guerriers attaquent les unités du joueur à leur portée ; un camp vide est pris en y entrant.

use crate::battle::{self, Side};
use crate::combat::{self, Fate};
use crate::game::{City, GameLog, GameMap, GameState, HexPos, Selection, Territory, Unit, claim_territory, spawn_unit};
use crate::hex::Hex;
use crate::map::MoveCosts;
use crate::rules::{self, UnitKind};
use crate::sites::{self, VillageEvent};
use bevy::prelude::*;
use rand::Rng;
use rand::seq::IndexedRandom;
use std::collections::HashSet;

/// Nombre de tours entre deux recrutements d'un camp.
pub const CAMP_SPAWN_INTERVAL: u32 = 6;
/// Nombre maximal de guerriers rattachés à un même camp.
pub const CAMP_MAX_UNITS: usize = 2;
/// Rayon d'errance des guerriers hostiles autour de leur base.
pub const WANDER_RADIUS: i32 = 3;
/// Distance à laquelle un guerrier hostile repère et va attaquer une unité du joueur.
pub const HUNT_RANGE: i32 = 2;
/// En dessous de ces PV, un guerrier hostile sans cible favorable reste sur place pour se soigner.
pub const WOUNDED_HP: i32 = 60;
/// Or obtenu en dispersant un camp.
pub const CAMP_CAPTURE_GOLD: i32 = 25;

#[derive(Component)]
pub struct Village;

#[derive(Component)]
pub struct Camp {
    pub cooldown: u32,
}

/// Marqueur des unités de la faction hostile (ni contrôlables, ni source de vision pour le joueur).
#[derive(Component)]
pub struct Hostile;

/// Base d'un guerrier hostile (camp ou ancien village) autour de laquelle il erre.
#[derive(Component)]
pub struct HostileHome(pub Hex);

/// Guerriers hostiles visibles par le joueur : cases infranchissables (on les attaque au lieu d'y entrer).
#[derive(Resource, Default)]
pub struct Blocked(pub HashSet<Hex>);

/// Occupation réelle des cases (une unité par case) : unités du joueur et toutes les unités.
#[derive(Resource, Default)]
pub struct Occupancy {
    pub own: HashSet<Hex>,
    pub all: HashSet<Hex>,
}

/// Zones de contrôle des guerriers hostiles visibles (cases adjacentes à l'infanterie ennemie).
#[derive(Resource, Default)]
pub struct EnemyZoc(pub HashSet<Hex>);

/// Zone dangereuse pour l'exploration automatique : cases à 2 cases ou moins d'un hostile
/// visible ou d'un camp connu (portée de chasse des hostiles).
#[derive(Resource, Default)]
pub struct Danger(pub HashSet<Hex>);

/// Villages déjà découverts par le joueur (cibles de l'exploration automatique).
#[derive(Resource, Default)]
pub struct KnownVillages(pub HashSet<Hex>);

/// Demande de tour de la faction hostile (posée par la fin de tour du joueur).
#[derive(Resource, Default)]
pub struct HostileTurnPending(pub bool);

/// Graine de la partie (pour un placement des sites reproductible).
#[derive(Resource)]
pub struct WorldSeed(pub u64);

pub struct HostilePlugin;

impl Plugin for HostilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Blocked>()
            .init_resource::<Occupancy>()
            .init_resource::<EnemyZoc>()
            .init_resource::<Danger>()
            .init_resource::<KnownVillages>()
            .init_resource::<HostileTurnPending>()
            .add_systems(Startup, spawn_sites);
    }
}

/// Crée un guerrier hostile rattaché à `home`.
fn spawn_hostile(commands: &mut Commands, at: Hex, home: Hex) {
    let entity = spawn_unit(commands, UnitKind::Militia, at);
    commands.entity(entity).insert((Hostile, HostileHome(home)));
}

fn spawn_sites(mut commands: Commands, map: Res<GameMap>, seed: Res<WorldSeed>) {
    let start = map.0.find_start().expect("aucun emplacement de départ");
    let placed = sites::place_sites(&map.0, start, seed.0);
    for &v in &placed.villages {
        commands.spawn((Village, HexPos(v)));
    }
    for &c in &placed.camps {
        commands.spawn((Camp { cooldown: CAMP_SPAWN_INTERVAL }, HexPos(c)));
        // Un premier guerrier sur une case voisine
        if let Some(&at) = c.neighbors().iter().find(|&&h| map.0.move_cost(h).is_some()) {
            spawn_hostile(&mut commands, at, c);
        }
    }
    info!("{} villages et {} camps hostiles placés", placed.villages.len(), placed.camps.len());
}

/// Visites des unités du joueur (y compris les cases traversées) : villages et camps vides.
pub fn visit_sites(
    mut commands: Commands,
    map: Res<GameMap>,
    mut state: ResMut<GameState>,
    mut territory: ResMut<Territory>,
    mut log: ResMut<GameLog>,
    movers: Query<(&Unit, &HexPos), (Without<Hostile>, Changed<HexPos>)>,
    villages: Query<(Entity, &HexPos), With<Village>>,
    camps: Query<(Entity, &HexPos), With<Camp>>,
    mut cities: Query<(Entity, &HexPos, &mut City)>,
    others: Query<&HexPos, Or<(With<Unit>, With<Camp>, With<Village>)>>,
) {
    if movers.is_empty() {
        return;
    }
    let touched: HashSet<Hex> = movers
        .iter()
        .flat_map(|(u, p)| u.0.last_steps.iter().copied().chain([p.0]))
        .collect();
    let mut rng = rand::rng();
    let mut taken: HashSet<Hex> = others.iter().map(|p| p.0).chain(cities.iter().map(|(_, p, _)| p.0)).collect();

    for (camp, pos) in &camps {
        if touched.contains(&pos.0) {
            commands.entity(camp).despawn();
            state.gold += CAMP_CAPTURE_GOLD;
            log.push(format!("Camp hostile dispersé : +{CAMP_CAPTURE_GOLD} or."));
        }
    }

    for (village, pos) in &villages {
        if !touched.contains(&pos.0) {
            continue;
        }
        commands.entity(village).despawn();
        let city_list: Vec<(Entity, Hex)> = cities.iter().map(|(e, p, _)| (e, p.0)).collect();
        match sites::roll_village_event(&mut rng, !city_list.is_empty()) {
            VillageEvent::JoinCity => {
                let target = sites::nearest(pos.0, &city_list).expect("au moins une ville");
                let (entity, center, mut city) = cities.get_mut(target).unwrap();
                city.state.population += 1;
                let radius = rules::territory_radius(city.state.population);
                claim_territory(&mut territory.0, entity, center.0, radius, &map.0);
                log.push(format!("Village : les habitants rejoignent {} (+1 habitant).", city.state.name));
            }
            VillageEvent::Trade { gold } => {
                state.gold += gold;
                log.push(format!("Village : échanges commerciaux, +{gold} or."));
            }
            VillageEvent::Hostile { warriors } => {
                let mut free: Vec<Hex> = pos
                    .0
                    .range(2)
                    .filter(|h| map.0.move_cost(*h).is_some() && !taken.contains(h) && !touched.contains(h))
                    .collect();
                free.sort();
                let mut spawned = 0;
                for _ in 0..warriors {
                    let Some(&at) = free.choose(&mut rng) else { break };
                    free.retain(|h| *h != at);
                    taken.insert(at);
                    spawn_hostile(&mut commands, at, pos.0);
                    spawned += 1;
                }
                log.push(format!("Village hostile : {spawned} milice(s) hostile(s) surgissent !"));
            }
        }
    }
}

/// Tour de la faction hostile : soins, recrutement dans les camps, puis pour chaque guerrier :
/// attaque d'une unité adjacente, sinon approche d'une unité proche, sinon errance.
pub fn hostile_turn(
    mut commands: Commands,
    mut pending: ResMut<HostileTurnPending>,
    map: Res<GameMap>,
    mut log: ResMut<GameLog>,
    mut selection: ResMut<Selection>,
    mut camps: Query<(&HexPos, &mut Camp)>,
    mut hostiles: Query<(Entity, &mut Unit, &mut HexPos, &HostileHome), (With<Hostile>, Without<Camp>)>,
    mut players: Query<(Entity, &mut Unit, &HexPos), (Without<Hostile>, Without<Camp>)>,
    fixed: Query<&HexPos, (Or<(With<City>, With<Village>)>, Without<Hostile>, Without<Unit>)>,
) {
    if !std::mem::take(&mut pending.0) {
        return;
    }
    let map = &map.0;
    let mut rng = rand::rng();
    let cities_and_villages: HashSet<Hex> = fixed.iter().map(|p| p.0).collect();
    let mut taken: HashSet<Hex> = cities_and_villages
        .iter()
        .copied()
        .chain(camps.iter().map(|(p, _)| p.0))
        .chain(hostiles.iter().map(|(_, _, p, _)| p.0))
        .chain(players.iter().map(|(_, _, p)| p.0))
        .collect();

    // Soins et nouveau tour
    for (_, mut unit, _, _) in &mut hostiles {
        unit.0.start_turn(false);
    }

    // Recrutement
    for (camp_pos, mut camp) in &mut camps {
        camp.cooldown = camp.cooldown.saturating_sub(1);
        if camp.cooldown > 0 {
            continue;
        }
        camp.cooldown = CAMP_SPAWN_INTERVAL;
        let garrison = hostiles.iter().filter(|(_, _, _, home)| home.0 == camp_pos.0).count();
        if garrison >= CAMP_MAX_UNITS {
            continue;
        }
        let free: Vec<Hex> = camp_pos
            .0
            .neighbors()
            .into_iter()
            .filter(|h| map.move_cost(*h).is_some() && !taken.contains(h))
            .collect();
        if let Some(&at) = free.choose(&mut rng) {
            taken.insert(at);
            spawn_hostile(&mut commands, at, camp_pos.0);
        }
    }

    // Actions des guerriers, l'un après l'autre
    let mut order: Vec<Entity> = hostiles.iter().map(|(e, ..)| e).collect();
    order.sort();
    for entity in order {
        let Ok((_, me, pos, home)) = hostiles.get(entity) else { continue };
        let (mut here, home) = (pos.0, home.0);
        let me = me.0.clone();
        let hostile_positions: Vec<Hex> = hostiles.iter().filter(|(e, ..)| *e != entity).map(|(_, _, p, _)| p.0).collect();

        // Proie : unité du joueur proche (et pas trop loin de la base) contre laquelle le combat est
        // favorable — pas de mort certaine, et au moins autant de dégâts infligés que subis
        // (ou une chance de disperser la cible). Terrain et flanquement par les autres hostiles
        // sont pris en compte. Parmi les proies, la plus proche puis la plus faible.
        let prey = players
            .iter()
            .filter(|(_, u, p)| u.0.hp > 0 && here.distance(p.0) <= HUNT_RANGE && p.0.distance(home) <= WANDER_RADIUS + 1)
            .filter(|(_, u, p)| {
                let e = battle::engagement(
                    map,
                    (&me, here),
                    (&u.0, p.0),
                    hostile_positions.iter().copied(),
                    here.distance(p.0) > 1,
                );
                let preview = combat::preview(&e);
                preview.attacker_fate != Fate::Dies
                    && (preview.to_defender.expected >= preview.to_attacker.expected
                        || preview.defender_fate != Fate::Survives)
            })
            .min_by_key(|(e, u, p)| (here.distance(p.0), u.0.hp, *e))
            .map(|(e, _, p)| (e, p.0));

        let Some((prey, prey_pos)) = prey else {
            // Blessé : reste sur place pour se soigner
            if me.hp < WOUNDED_HP {
                continue;
            }
            // Errance : un pas aléatoire (60 % de chances) en restant près de la base
            if rng.random_bool(0.6) {
                let options: Vec<Hex> = here
                    .neighbors()
                    .into_iter()
                    .filter(|h| h.distance(home) <= WANDER_RADIUS && map.step_cost(here, *h).is_some() && !taken.contains(h))
                    .collect();
                if let Some(&next) = options.choose(&mut rng) {
                    let (_, mut unit, mut pos, _) = hostiles.get_mut(entity).unwrap();
                    taken.remove(&pos.0);
                    taken.insert(next);
                    pos.0 = next;
                    unit.0.moved_this_turn = true;
                }
            }
            continue;
        };

        // Approche : un pas vers la proie si elle n'est pas adjacente
        if here.distance(prey_pos) > 1 {
            let step = here
                .neighbors()
                .into_iter()
                .filter(|h| {
                    h.distance(prey_pos) == 1
                        && map.step_cost(here, *h).is_some()
                        && rules::in_attack_position(map, me.kind, *h, prey_pos)
                        && !taken.contains(h)
                })
                .min_by_key(|h| (map.step_cost(here, *h).unwrap_or(9), *h));
            let Some(step) = step else { continue };
            let (_, mut unit, mut pos, _) = hostiles.get_mut(entity).unwrap();
            let cost = map.step_cost(here, step).unwrap_or(1);
            unit.0.moves_left = unit.0.moves_left.saturating_sub(cost);
            unit.0.moved_this_turn = true;
            taken.remove(&pos.0);
            taken.insert(step);
            pos.0 = step;
            here = step;
            if unit.0.moves_left == 0 {
                continue;
            }
        }

        // Attaque (impossible à travers une falaise)
        if !rules::in_attack_position(map, me.kind, here, prey_pos) {
            continue;
        }
        let hostile_positions: Vec<Hex> = hostiles.iter().filter(|(e, ..)| *e != entity).map(|(_, _, p, _)| p.0).collect();
        let Ok((_, mut attacker, _, _)) = hostiles.get_mut(entity) else { continue };
        let Ok((_, mut defender, _)) = players.get_mut(prey) else { continue };
        let e = battle::engagement(map, (&attacker.0, here), (&defender.0, prey_pos), hostile_positions, false);
        let result = battle::fight(
            &mut commands,
            &mut log,
            Side { entity, unit: &mut attacker.0, hostile: true },
            Side { entity: prey, unit: &mut defender.0, hostile: false },
            &e,
        );
        if result.defender_died {
            taken.remove(&prey_pos);
            if selection.0 == Some(prey) {
                selection.0 = None;
            }
            // Le vainqueur avance sur la case libérée (sauf ville ou village)
            if result.attacker_advances() && !cities_and_villages.contains(&prey_pos) {
                let (_, _, mut pos, _) = hostiles.get_mut(entity).unwrap();
                taken.remove(&pos.0);
                taken.insert(prey_pos);
                pos.0 = prey_pos;
            }
        }
        if result.attacker_died {
            taken.remove(&here);
        }
    }
}

/// Ce que le joueur sait de la carte : guerriers hostiles visibles, occupation des cases,
/// villages découverts. Mis à jour à chaque image.
pub fn update_knowledge(
    fog: Res<crate::game::Fog>,
    hostiles: Query<(&Unit, &HexPos), With<Hostile>>,
    own: Query<&HexPos, (With<Unit>, Without<Hostile>)>,
    villages: Query<&HexPos, With<Village>>,
    camps: Query<&HexPos, With<Camp>>,
    mut blocked: ResMut<Blocked>,
    mut enemy_zoc: ResMut<EnemyZoc>,
    mut danger: ResMut<Danger>,
    mut occupancy: ResMut<Occupancy>,
    mut known_villages: ResMut<KnownVillages>,
) {
    let known: HashSet<Hex> = villages.iter().map(|p| p.0).filter(|h| fog.explored.contains(h)).collect();
    if known_villages.0 != known {
        known_villages.0 = known;
    }
    let visible: HashSet<Hex> = hostiles.iter().map(|(_, p)| p.0).filter(|h| fog.visible.contains(h)).collect();
    if blocked.0 != visible {
        blocked.0 = visible.clone();
    }
    let zoc = rules::zoc_tiles(
        hostiles.iter().filter(|(_, p)| fog.visible.contains(&p.0)).map(|(u, p)| (u.0.kind, p.0)),
    );
    if enemy_zoc.0 != zoc {
        enemy_zoc.0 = zoc;
    }
    let threats: HashSet<Hex> = visible
        .iter()
        .copied()
        .chain(camps.iter().map(|p| p.0).filter(|h| fog.explored.contains(h)))
        .flat_map(|h| h.range(HUNT_RANGE))
        .collect();
    if danger.0 != threats {
        danger.0 = threats;
    }
    let own_set: HashSet<Hex> = own.iter().map(|p| p.0).collect();
    let all: HashSet<Hex> = own_set.iter().copied().chain(hostiles.iter().map(|(_, p)| p.0)).collect();
    if occupancy.own != own_set || occupancy.all != all {
        occupancy.own = own_set;
        occupancy.all = all;
    }
}
