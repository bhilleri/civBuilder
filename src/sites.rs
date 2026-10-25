//! Villages et camps hostiles : placement à la génération et événements de visite (logique pure).

use crate::hex::Hex;
use crate::map::{MoveCosts, WorldMap};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Une case terrestre sur `LAND_PER_VILLAGE` porte un village (environ).
pub const LAND_PER_VILLAGE: usize = 45;
/// Une case terrestre sur `LAND_PER_CAMP` porte un camp hostile (environ).
pub const LAND_PER_CAMP: usize = 160;

/// Distances minimales (en cases).
pub const VILLAGE_MIN_FROM_START: i32 = 3;
pub const CAMP_MIN_FROM_START: i32 = 10;
const MIN_BETWEEN_SITES: i32 = 4;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sites {
    pub villages: Vec<Hex>,
    pub camps: Vec<Hex>,
}

/// Place villages et camps sur la terre ferme franchissable, de façon déterministe pour une graine.
pub fn place_sites(map: &WorldMap, start: Hex, seed: u64) -> Sites {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut land: Vec<Hex> = map.hexes().filter(|&h| map.move_cost(h).is_some() && !map.is_fleuve(h)).collect();
    land.shuffle(&mut rng);

    let camp_target = (land.len() / LAND_PER_CAMP).max(2);
    let village_target = (land.len() / LAND_PER_VILLAGE).max(4);
    let mut sites = Sites::default();
    let far_enough = |h: Hex, sites: &Sites| {
        sites.villages.iter().chain(&sites.camps).all(|&s| map.distance(s, h) >= MIN_BETWEEN_SITES)
    };

    // Les camps d'abord (contrainte la plus forte : loin du départ)
    for &h in &land {
        if sites.camps.len() >= camp_target {
            break;
        }
        if map.distance(h, start) >= CAMP_MIN_FROM_START && far_enough(h, &sites) {
            sites.camps.push(h);
        }
    }
    for &h in &land {
        if sites.villages.len() >= village_target {
            break;
        }
        if map.distance(h, start) >= VILLAGE_MIN_FROM_START && far_enough(h, &sites) {
            sites.villages.push(h);
        }
    }
    sites
}

/// Ce qui se passe quand une unité entre dans un village.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VillageEvent {
    /// Les habitants rejoignent la ville la plus proche (+1 habitant).
    JoinCity,
    /// Échanges commerciaux : de l'or.
    Trade { gold: i32 },
    /// Le village est hostile : des guerriers hostiles surgissent.
    Hostile { warriors: u32 },
}

/// Probabilités (en %) : rejoindre 35, commerce 45, hostile 20.
/// Sans ville, « rejoindre » est impossible et son poids passe au commerce.
pub fn roll_village_event(rng: &mut impl Rng, has_city: bool) -> VillageEvent {
    let roll = rng.random_range(0..100);
    let join = if has_city { 35 } else { 0 };
    if roll < join {
        VillageEvent::JoinCity
    } else if roll < 80 {
        VillageEvent::Trade { gold: rng.random_range(15..=35) }
    } else {
        VillageEvent::Hostile { warriors: rng.random_range(1..=2) }
    }
}

/// Ville la plus proche d'un point (départage par ordre de la liste).
pub fn nearest<T: Copy>(map: &WorldMap, from: Hex, items: &[(T, Hex)]) -> Option<T> {
    items.iter().min_by_key(|&&(_, h)| map.distance(h, from)).map(|(t, _)| *t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_respecte_les_contraintes() {
        for seed in 0..5 {
            let map = WorldMap::generate(64, 40, seed as u32);
            let start = map.find_start().unwrap();
            let sites = place_sites(&map, start, seed);
            assert!(sites.villages.len() >= 4, "graine {seed} : {} villages", sites.villages.len());
            assert!(!sites.camps.is_empty());
            let all: Vec<Hex> = sites.villages.iter().chain(&sites.camps).copied().collect();
            for (i, a) in all.iter().enumerate() {
                assert!(map.move_cost(*a).is_some(), "site sur terrain franchissable");
                for b in &all[i + 1..] {
                    assert!(a.distance(*b) >= MIN_BETWEEN_SITES);
                }
            }
            assert!(sites.villages.iter().all(|v| v.distance(start) >= VILLAGE_MIN_FROM_START));
            assert!(sites.camps.iter().all(|c| c.distance(start) >= CAMP_MIN_FROM_START));
        }
    }

    #[test]
    fn placement_deterministe() {
        let map = WorldMap::generate(64, 40, 7);
        let start = map.find_start().unwrap();
        assert_eq!(place_sites(&map, start, 7), place_sites(&map, start, 7));
    }

    #[test]
    fn distribution_des_evenements() {
        let mut rng = StdRng::seed_from_u64(1);
        let (mut join, mut trade, mut hostile) = (0, 0, 0);
        for _ in 0..10_000 {
            match roll_village_event(&mut rng, true) {
                VillageEvent::JoinCity => join += 1,
                VillageEvent::Trade { gold } => {
                    assert!((15..=35).contains(&gold));
                    trade += 1
                }
                VillageEvent::Hostile { warriors } => {
                    assert!((1..=2).contains(&warriors));
                    hostile += 1
                }
            }
        }
        // ~35 % / 45 % / 20 %, avec une marge
        assert!((3_200..3_800).contains(&join), "rejoindre {join}");
        assert!((4_200..4_800).contains(&trade), "commerce {trade}");
        assert!((1_700..2_300).contains(&hostile), "hostile {hostile}");
    }

    #[test]
    fn sans_ville_pas_de_ralliement() {
        let mut rng = StdRng::seed_from_u64(2);
        for _ in 0..2_000 {
            assert_ne!(roll_village_event(&mut rng, false), VillageEvent::JoinCity);
        }
    }

    #[test]
    fn ville_la_plus_proche() {
        let mut map = WorldMap::filled(40, 4, crate::map::Terrain::Plains);
        let cities = [("A", Hex::new(0, 0)), ("B", Hex::new(10, 0))];
        assert_eq!(nearest(&map, Hex::new(7, 0), &cities), Some("B"));
        assert_eq!(nearest::<&str>(&map, Hex::new(7, 0), &[]), None);
        // Sur une planète, la plus proche peut l'être par l'autre côté
        let far = [("A", Hex::from_offset(10, 0)), ("B", Hex::from_offset(37, 0))];
        assert_eq!(nearest(&map, Hex::from_offset(1, 0), &far), Some("A"));
        map.set_wraps(true);
        assert_eq!(nearest(&map, Hex::from_offset(1, 0), &far), Some("B"));
    }
}
