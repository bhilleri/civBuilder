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

/// Limite de l'habitat : pas de site si la saison la plus chaude reste sous cette température
/// (°C). L'habitat de montagne suit la chaleur de l'été, pas l'altitude : un haut plateau tropical
/// reste habité, une montagne froide non (voir `conception/conception-structures.md`).
pub const NO_SITE_BELOW: f32 = 5.0;
/// Sous cette température de la saison la plus chaude (°C), les villages sont des éleveurs.
pub const HERDERS_BELOW: f32 = 10.0;

/// Le climat de la case permet d'y vivre à l'année (les cases sans climat simulé : oui).
fn habitable(map: &WorldMap, h: Hex) -> bool {
    map.warmest_season(h).is_none_or(|t| t >= NO_SITE_BELOW)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sites {
    pub villages: Vec<Hex>,
    pub camps: Vec<Hex>,
}

/// Place villages et camps sur la terre ferme franchissable, de façon déterministe pour une graine.
#[cfg(test)]
pub fn place_sites(map: &WorldMap, start: Hex, seed: u64) -> Sites {
    place_sites_avec(map, start, seed, LAND_PER_CAMP)
}

/// Comme `place_sites`, avec une densité de camps choisie (cases de terre par camp, réglage
/// « Barbares » de la configuration de partie).
pub fn place_sites_avec(map: &WorldMap, start: Hex, seed: u64, terres_par_camp: usize) -> Sites {
    let mut rng = StdRng::seed_from_u64(seed);
    // (la banquise est franchissable, mais aucune structure n'y prend place)
    let mut land: Vec<Hex> = map.hexes().filter(|&h| map.move_cost(h).is_some() && !map.is_fleuve(h) && !map.is_ice(h) && habitable(map, h)).collect();
    land.shuffle(&mut rng);

    let camp_target = (land.len() / terres_par_camp.max(1)).max(2);
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
    fn aucun_site_sur_la_banquise() {
        let mut map = WorldMap::filled(30, 30, crate::map::Terrain::Ocean);
        for h in map.hexes().collect::<Vec<_>>() {
            let (col, _) = h.to_offset();
            if col < 15 {
                map.set(h, crate::map::Terrain::Plains);
            } else {
                map.freeze(h);
            }
        }
        let sites = place_sites(&map, Hex::from_offset(7, 15), 42);
        for &at in sites.villages.iter().chain(&sites.camps) {
            assert!(!map.is_ice(at), "site sur la banquise : {at:?}");
        }
    }

    #[test]
    fn placement_deterministe() {
        let map = WorldMap::generate(64, 40, 7);
        let start = map.find_start().unwrap();
        assert_eq!(place_sites(&map, start, 7), place_sites(&map, start, 7));
    }

    #[test]
    fn aucun_site_sans_ete() {
        for seed in 0..5 {
            let map = WorldMap::generate(64, 40, seed as u32);
            let sites = place_sites(&map, map.find_start().unwrap(), seed);
            for &h in sites.villages.iter().chain(&sites.camps) {
                let warmest = map.warmest_season(h).unwrap();
                assert!(warmest >= NO_SITE_BELOW, "graine {seed} : site à {warmest:.1} °C en été");
            }
        }
    }

    #[test]
    fn un_ete_frais_donne_des_eleveurs() {
        use crate::settlements::{Family, village_family};
        assert_eq!(village_family(Family::Forest, 7.0), Family::Steppe);
        assert_eq!(village_family(Family::Forest, 14.0), Family::Forest);
        assert_eq!(village_family(Family::Cold, 7.0), Family::Cold);
    }

    /// Effet de la limite d'habitat : `cargo test bilan_sites_climat -- --ignored --nocapture`
    /// (`SEEDS=777,2`) : terres écartées, sites, part des villages d'éleveurs, altitudes.
    #[test]
    #[ignore]
    fn bilan_sites_climat() {
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![777, 2, 4242]);
        for seed in seeds {
            let map = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let land: Vec<Hex> = map.hexes().filter(|&h| map.move_cost(h).is_some() && !map.is_fleuve(h) && !map.is_ice(h)).collect();
            let cold = land.iter().filter(|&&h| !habitable(&map, h)).count();
            let sites = place_sites(&map, map.find_start().unwrap(), seed as u64);
            let herders = sites.villages.iter().filter(|&&h| map.warmest_season(h).is_some_and(|t| t < HERDERS_BELOW)).count();
            let high = sites.villages.iter().chain(&sites.camps).map(|&h| map.elevation(h).unwrap_or(0.0)).fold(0.0f32, f32::max);
            let mountain = sites.villages.iter().chain(&sites.camps).filter(|&&h| map.elevation(h).unwrap_or(0.0) >= 2_000.0).count();
            println!("graine {seed} : terres {}, écartées {cold} ({:.0} %), villages {} (éleveurs {herders}), camps {}, sites à 2 000 m ou plus {mountain}, plus haut {high:.0} m", land.len(), 100.0 * cold as f32 / land.len().max(1) as f32, sites.villages.len(), sites.camps.len());
            if std::env::var("SITES_LIST").is_ok() {
                let line = |h: Hex| format!("{:?} {:.0} m, été {:.1} °C", h.to_offset(), map.elevation(h).unwrap_or(0.0), map.warmest_season(h).unwrap_or(0.0));
                sites.villages.iter().filter(|&&h| map.elevation(h).unwrap_or(0.0) >= 2_000.0 || map.warmest_season(h).is_some_and(|t| t < HERDERS_BELOW)).for_each(|&h| println!("  village {}", line(h)));
                sites.camps.iter().filter(|&&h| map.elevation(h).unwrap_or(0.0) >= 2_000.0).for_each(|&h| println!("  camp {}", line(h)));
            }
        }
    }

    /// Effet du plafond des pluies : `cargo test bilan_plafond_pluie -- --ignored --nocapture`
    /// (`SEEDS=777,2`) : pluie la plus forte, cases très arrosées, déserts, pluie moyenne des terres.
    #[test]
    #[ignore]
    fn bilan_plafond_pluie() {
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![777, 2, 4242]);
        for seed in seeds {
            let map = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let land: Vec<Hex> = map.hexes().filter(|&h| map.get(h).is_some_and(|t| !t.is_water())).collect();
            let rain: Vec<f32> = land.iter().filter_map(|&h| map.rain(h)).collect();
            let max = rain.iter().copied().fold(0.0f32, f32::max);
            let deserts = land.iter().filter(|&&h| map.get(h) == Some(crate::map::Terrain::Desert)).count();
            println!("graine {seed} : terres {}, pluie max {max:.0} mm, plus de 12 000 mm : {}, plus de 8 000 mm : {}, déserts {deserts}, pluie moyenne {:.0} mm, arêtes de rivière {}", land.len(), rain.iter().filter(|&&r| r > 12_000.0).count(), rain.iter().filter(|&&r| r > 8_000.0).count(), rain.iter().sum::<f32>() / rain.len().max(1) as f32, land.iter().map(|&h| (0..6).filter(|&d| map.river_flow(h, d) > 0.0).count()).sum::<usize>() / 2);
            let mouths: Vec<f32> = land.iter().filter(|&&h| map.mouth(h).is_some()).map(|&h| map.elevation(h).unwrap_or(0.0)).collect();
            let forests_high = land.iter().filter(|&&h| map.get(h) == Some(crate::map::Terrain::Forest) && map.warmest_season(h).is_some_and(|t| t < HERDERS_BELOW)).count();
            println!("  embouchures {}, à 40 m ou plus {}, à 500 m ou plus {} ; forêts tempérées sous 10 °C en été : {forests_high}", mouths.len(), mouths.iter().filter(|&&a| a >= 40.0).count(), mouths.iter().filter(|&&a| a >= 500.0).count());
        }
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
