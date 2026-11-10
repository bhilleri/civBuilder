//! Recherche de chemin A* sur la grille hexagonale (logique pure).

use crate::cases::Grille;
use crate::hex::Hex;
use crate::map::{MoveCosts, turn_moves};
use std::cmp::Reverse;
use bevy::platform::collections::{HashMap, HashSet};
use std::collections::BinaryHeap;

/// Chemin le moins coûteux de `start` à `goal` (start exclu, goal inclus).
/// `None` si la destination est inaccessible.
pub fn find_path(map: &impl MoveCosts, start: Hex, goal: Hex) -> Option<Vec<Hex>> {
    if start == goal {
        return Some(Vec::new());
    }
    map.move_cost(goal)?;

    let mut open = BinaryHeap::new();
    // (tableaux plats plutôt que tables de hachage : `cases::Grille`)
    let mut came_from: Grille<Option<Hex>> = Grille::new(None);
    let mut cost: Grille<u32> = Grille::new(u32::MAX);
    cost.set(start, 0);
    let wrap = map.wrap();
    open.push(Reverse((wrap.distance(start, goal) as u32, start)));

    while let Some(Reverse((_, current))) = open.pop() {
        if current == goal {
            return Some(rebuild(&came_from, start, goal));
        }
        let current_cost = cost.get(current);
        for next in map.wrap().neighbors(current) {
            let Some(step) = map.step_cost(current, next) else { continue };
            let new_cost = current_cost + step;
            if new_cost < cost.get(next) {
                cost.set(next, new_cost);
                came_from.set(next, Some(current));
                open.push(Reverse((new_cost + wrap.distance(next, goal) as u32, next)));
            }
        }
    }
    None
}

/// Chemin le plus rapide pour une unité, en tours puis en PM restants (et non en coût total) :
/// on peut entrer sur une case tant qu'il reste au moins 1 PM, quel que soit son coût. Une
/// traversée chère (rivière, fleuve) faite d'un coup peut donc arriver plus tôt qu'un détour
/// moins coûteux en tout. Le terrain plus cher que les PM restants laisse une dette (`debt` au
/// départ) qui retarde le départ des tours suivants. Même règle que `reachable` et `rules::path_turns`.
pub fn find_path_turns(map: &impl MoveCosts, start: Hex, goal: Hex, moves_left: u32, debt: u32, max_moves: u32) -> Option<Vec<Hex>> {
    chemin_en_tours(map, start, goal, moves_left, debt, max_moves, true).map(|(path, _)| path)
}

/// Chemin le plus rapide et sa clé d'arrivée (tours, puis PM dépensés) ; `estimer` : avec la
/// borne A* (sans elle, Dijkstra : la référence des tests).
fn chemin_en_tours(map: &impl MoveCosts, start: Hex, goal: Hex, moves_left: u32, debt: u32, max_moves: u32, estimer: bool) -> Option<(Vec<Hex>, u64)> {
    if start == goal {
        return Some((Vec::new(), 0));
    }
    map.move_cost(goal)?;
    let max_moves = max_moves.max(1);
    // Clé : (tours commencés, PM dépensés dans le tour), à minimiser
    let key = |turn: u32, left: u32| (u64::from(turn) << 32) | u64::from(u32::MAX - left);
    // Une unité sans PM restant repart au premier tour où la dette lui en laisse : ce départ
    // résume tout son état (la dette est soldée), et sert de clé aux cases de passage
    let resume = |mut turn: u32, mut left: u32, mut debt: u32| {
        while left == 0 {
            turn += 1;
            (left, debt) = turn_moves(max_moves, debt);
        }
        (turn, left)
    };
    let (turn0, left0) = resume(0, moves_left, debt);
    // Borne basse de la clé d'arrivée depuis (tour, PM) à `d` cases du but (A*) : chaque pas coûte
    // au moins 1 PM, et un tour en donne au plus `max_moves`. Elle ne surestime jamais : le chemin
    // trouvé reste le plus rapide, en explorant bien moins de cases que sans estimation.
    let wrap = map.wrap();
    let borne = |turn: u32, left: u32, d: u32| {
        if !estimer {
            key(turn, left)
        } else if d <= left {
            key(turn, left - d)
        } else {
            let reste = d - left;
            let tours = reste.div_ceil(max_moves);
            let dernier = reste - (tours - 1) * max_moves;
            key(turn + tours, max_moves - dernier)
        }
    };
    let mut best: Grille<(u64, u32, u32)> = Grille::new((u64::MAX, 0, 0));
    best.set(start, (key(turn0, left0), turn0, left0));
    let mut came_from: Grille<Option<Hex>> = Grille::new(None);
    let mut open = BinaryHeap::from([Reverse((borne(turn0, left0, wrap.distance(start, goal) as u32), key(turn0, left0), start))]);
    while let Some(Reverse((_, k, current))) = open.pop() {
        let (bk, turn, left) = best.get(current);
        if k > bk {
            continue;
        }
        if current == goal {
            return Some((rebuild(&came_from, start, goal), k));
        }
        for next in map.wrap().neighbors(current) {
            let Some(step) = map.step(current, next) else { continue };
            let (after, owed) = step.spend(left);
            // La destination compte à l'arrivée ; une case de passage, au départ suivant
            let (nt, nl) = if next == goal { (turn, after) } else { resume(turn, after, owed) };
            let nk = key(nt, nl);
            if nk < best.get(next).0 {
                best.set(next, (nk, nt, nl));
                came_from.set(next, Some(current));
                open.push(Reverse((borne(nt, nl, wrap.distance(next, goal) as u32), nk, next)));
            }
        }
    }
    None
}

/// Chemin de `start` (exclu) à `goal` (inclus), en remontant les prédécesseurs.
fn rebuild(came_from: &Grille<Option<Hex>>, start: Hex, goal: Hex) -> Vec<Hex> {
    let mut path = vec![goal];
    let mut node = goal;
    while let Some(prev) = came_from.get(node) {
        if prev == start {
            break;
        }
        path.push(prev);
        node = prev;
    }
    path.reverse();
    path
}

/// Cases atteignables ce tour-ci avec `moves` points de mouvement (départ exclu).
///
/// Même règle que le déplacement : on peut entrer sur une case tant qu'il reste
/// au moins 1 PM, quel que soit son coût. Dijkstra sur le coût cumulé : une case est
/// atteignable si l'on peut arriver sur l'une de ses voisines avec un coût < `moves`.
pub fn reachable(map: &impl MoveCosts, start: Hex, moves: u32) -> HashSet<Hex> {
    let mut best: HashMap<Hex, u32> = HashMap::from([(start, 0)]);
    let mut open = BinaryHeap::from([Reverse((0u32, start))]);
    while let Some(Reverse((spent, current))) = open.pop() {
        if spent > best[&current] || spent >= moves {
            continue; // entrée obsolète, ou plus de PM pour repartir de cette case
        }
        for next in map.wrap().neighbors(current) {
            let Some(cost) = map.step_cost(current, next) else { continue };
            let total = spent + cost;
            if best.get(&next).is_none_or(|&b| total < b) {
                best.insert(next, total);
                open.push(Reverse((total, next)));
            }
        }
    }
    best.remove(&start);
    best.into_keys().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{KnownMap, Relief, Terrain, WorldMap};

    #[test]
    fn ligne_droite() {
        let map = WorldMap::filled(10, 10, Terrain::Plains);
        let a = Hex::from_offset(1, 1);
        let b = Hex::from_offset(6, 1);
        let path = find_path(&map, a, b).unwrap();
        assert_eq!(path.len(), 5);
        assert_eq!(*path.last().unwrap(), b);
    }

    #[test]
    fn contourne_les_montagnes() {
        let mut map = WorldMap::filled(10, 10, Terrain::Plains);
        // mur de montagnes vertical en colonne 4, sauf la ligne 8
        for row in 0..8 {
            map.set_relief(Hex::from_offset(4, row), Relief::Peak);
        }
        let a = Hex::from_offset(2, 2);
        let b = Hex::from_offset(6, 2);
        let path = find_path(&map, a, b).unwrap();
        assert!(path.iter().all(|&h| map.relief(h) != Relief::Peak));
        assert!(path.len() > 4);
    }

    #[test]
    fn prefere_terrain_rapide() {
        let mut map = WorldMap::filled(10, 10, Terrain::Plains);
        let a = Hex::from_offset(1, 4);
        let b = Hex::from_offset(5, 4);
        // forêts sur la ligne directe
        for col in 2..5 {
            map.set(Hex::from_offset(col, 4), Terrain::Forest);
        }
        let path = find_path(&map, a, b).unwrap();
        let cost: u32 = path.iter().map(|&h| map.move_cost(h).unwrap()).sum();
        assert!(cost <= 6, "coût {cost}");
    }

    #[test]
    fn portee_en_plaine() {
        let map = WorldMap::filled(20, 20, Terrain::Plains);
        let c = Hex::from_offset(10, 10);
        // 2 PM en plaine : anneaux 1 et 2 = 6 + 12 cases
        assert_eq!(reachable(&map, c, 2).len(), 18);
        assert!(reachable(&map, c, 0).is_empty());
    }

    #[test]
    fn portee_avec_forets() {
        let mut map = WorldMap::filled(20, 20, Terrain::Forest);
        let c = Hex::from_offset(10, 10);
        // Forêt partout (2 PM) avec 2 PM : on entre dans une forêt et on s'arrête
        assert_eq!(reachable(&map, c, 2).len(), 6);
        // Avec 3 PM : forêt (reste 1) puis une seconde forêt
        assert_eq!(reachable(&map, c, 3).len(), 18);
        map.set_relief(c.neighbors()[0], Relief::Peak);
        assert!(!reachable(&map, c, 3).contains(&c.neighbors()[0]));
    }

    /// Traverser une rivière d'un coup (on entre avec 1 PM) arrive plus tôt que le détour,
    /// pourtant moins coûteux au total.
    #[test]
    fn traversee_de_riviere_plus_rapide_que_le_detour() {
        use crate::rules::path_turns;
        let mut map = WorldMap::filled(10, 10, Terrain::Plains);
        let a = Hex::from_offset(4, 4);
        let b = a + crate::hex::DIRECTIONS[0];
        map.set_river(a, 0, 150.0);
        let cheapest = find_path(&map, a, b).unwrap();
        assert_eq!(cheapest.len(), 2, "le moins coûteux contourne l'arête");
        let fastest = find_path_turns(&map, a, b, 1, 0, 2).unwrap();
        assert_eq!(fastest, vec![b]);
        assert_eq!(path_turns(&map, a, &fastest, 1, 0, 2), vec![1]);
    }

    /// La surbrillance doit concorder avec le nombre de tours affiché sur le trajet.
    #[test]
    fn portee_concorde_avec_les_trajets() {
        use crate::rules::path_turns;
        for seed in 0..5 {
            let map = WorldMap::generate(40, 30, seed);
            let start = map.find_start().unwrap();
            for moves in 1..=3 {
                let reach = reachable(&map, start, moves);
                for target in start.range(5).filter(|&h| h != start) {
                    let this_turn = find_path_turns(&map, start, target, moves, 0, moves)
                        .is_some_and(|p| path_turns(&map, start, &p, moves, 0, moves).last() == Some(&1));
                    assert_eq!(reach.contains(&target), this_turn, "graine {seed}, {moves} PM, {target:?}");
                }
            }
        }
    }

    /// Un lac inexploré est supposé franchissable : le trajet passe tout droit.
    /// Une fois exploré, le trajet le contourne.
    #[test]
    fn brouillard_suppose_franchissable() {
        let mut map = WorldMap::filled(12, 12, Terrain::Plains);
        for row in 0..10 {
            map.set(Hex::from_offset(5, row), Terrain::Ocean);
        }
        let (a, b) = (Hex::from_offset(2, 3), Hex::from_offset(8, 3));
        let mut explored: crate::cases::CaseSet = a.range(2).collect();

        let blind = find_path(&KnownMap::new(&map, &explored), a, b).unwrap();
        assert_eq!(blind.len(), a.distance(b) as usize, "ligne droite à travers l'inconnu");

        explored.extend(map.hexes());
        let informed = find_path(&KnownMap::new(&map, &explored), a, b).unwrap();
        assert!(informed.iter().all(|&h| !map.get(h).unwrap().is_water()));
        assert!(informed.len() > blind.len());
    }

    /// À distance égale, le terrain connu est préféré à l'inconnu.
    #[test]
    fn prefere_le_connu() {
        let map = WorldMap::filled(12, 12, Terrain::Plains);
        let (a, b) = (Hex::from_offset(2, 5), Hex::from_offset(8, 5));
        // Bande connue légèrement au nord du trajet direct
        let mut explored: crate::cases::CaseSet = a.range(1).collect();
        for col in 2..=9 {
            explored.insert(Hex::from_offset(col, 4));
            explored.insert(Hex::from_offset(col, 3));
        }
        explored.extend(b.range(1));
        let known = KnownMap::new(&map, &explored);
        let path = find_path(&known, a, b).unwrap();
        let unknown_steps = path.iter().filter(|h| !explored.contains(h)).count();
        assert_eq!(unknown_steps, 0, "le trajet doit emprunter la bande connue : {path:?}");
    }

    #[test]
    fn destination_inconnue_acceptee() {
        let map = WorldMap::filled(10, 10, Terrain::Ocean);
        let explored: crate::cases::CaseSet = [Hex::from_offset(1, 1)].into_iter().collect();
        let known = KnownMap::new(&map, &explored);
        // Tout est de l'eau en réalité, mais rien n'est connu hormis le départ
        assert!(find_path(&known, Hex::from_offset(1, 1), Hex::from_offset(5, 5)).is_some());
        assert!(find_path(&map, Hex::from_offset(1, 1), Hex::from_offset(5, 5)).is_none());
    }

    #[test]
    fn inaccessible() {
        let map = WorldMap::filled(10, 10, Terrain::Ocean);
        assert!(find_path(&map, Hex::from_offset(1, 1), Hex::from_offset(3, 3)).is_none());
    }

    #[test]
    fn falaise_contournee_par_une_rampe() {
        // Plateau à 1 300 m à droite de la colonne 5 (plaine à 150 m), falaise sauf une rampe
        // (450 → 850 → 1 300 m)
        let mut map = WorldMap::filled(12, 9, Terrain::Plains);
        for h in map.hexes().collect::<Vec<_>>() {
            let (col, _) = h.to_offset();
            if col >= 5 {
                map.set_elevation(h, 1_300.0);
            }
        }
        let ramp = [Hex::from_offset(3, 7), Hex::from_offset(4, 7)];
        map.set_elevation(ramp[0], 450.0);
        map.set_elevation(ramp[1], 850.0);
        let (start, goal) = (Hex::from_offset(2, 1), Hex::from_offset(7, 1));
        let path = find_path(&map, start, goal).expect("la rampe permet de monter");
        assert!(path.contains(&ramp[1]), "le trajet passe par la rampe : {path:?}");
        // Sans rampe : inaccessible
        map.set_elevation(ramp[1], 150.0);
        map.set_elevation(ramp[0], 150.0);
        assert!(find_path(&map, start, goal).is_none());
    }

    #[test]
    fn chemin_en_tours_aussi_rapide_qu_une_recherche_sans_estimation() {
        // Sur un relief varié, l'estimation A* donne la même arrivée (tour et PM) que la recherche
        // complète sans estimation : elle ne fait rien perdre
        let mut map = WorldMap::filled(40, 30, Terrain::Plains);
        for i in 0..40 {
            for j in 0..30 {
                let h = Hex::from_offset(i, j);
                match (i * 7 + j * 13) % 11 {
                    0 | 1 => map.set(h, Terrain::Forest),
                    2 => map.set_relief(h, Relief::Peak),
                    3 => map.set_relief(h, Relief::Hills),
                    _ => {}
                }
            }
        }
        let paires = (0..30).map(|k: i32| ((k * 7 % 40, k * 11 % 30), ((k * 13 + 17) % 40, (k * 5 + 9) % 30)));
        for (k, (a, b)) in paires.enumerate() {
            let (a, b) = (Hex::from_offset(a.0, a.1), Hex::from_offset(b.0, b.1));
            if map.relief(a) == Relief::Peak || map.relief(b) == Relief::Peak || a == b {
                continue;
            }
            for (max, left, debt) in [(2, 2, 0), (4, 4, 0), (4, 1, 0), (3, 2, 2)] {
                let (_, avec) = chemin_en_tours(&map, a, b, left, debt, max, true).expect("chemin");
                let (_, sans) = chemin_en_tours(&map, a, b, left, debt, max, false).expect("chemin");
                assert_eq!(avec, sans, "cas {k}, {max} PM ({left} restants, dette {debt}) : même arrivée");
            }
        }
    }

    /// Durée du calcul de chemin sur une carte de la taille réelle (192 × 111), avec des murs de
    /// montagnes : `cargo test --no-default-features bilan_chemins -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bilan_chemins() {
        let mut map = WorldMap::filled(192, 111, Terrain::Plains);
        // Murs de montagnes percés d'un passage, pour obliger à des détours
        for col in (20..190).step_by(20) {
            for row in 0..111 {
                if row % 50 != 7 {
                    map.set_relief(Hex::from_offset(col, row), Relief::Peak);
                }
            }
        }
        let paires: Vec<(Hex, Hex)> = (0..60).map(|i| (Hex::from_offset(2 + i % 10, 5 + i), Hex::from_offset(185 - i % 7, 100 - i))).collect();
        let t = std::time::Instant::now();
        let n = paires.iter().filter(|(a, b)| find_path(&map, *a, *b).is_some()).count();
        let a_star = t.elapsed();
        let t = std::time::Instant::now();
        let m = paires.iter().filter(|(a, b)| find_path_turns(&map, *a, *b, 4, 0, 4).is_some()).count();
        let en_tours = t.elapsed();
        let t = std::time::Instant::now();
        let r: usize = paires.iter().map(|(a, _)| reachable(&map, *a, 40).len()).sum();
        let portee = t.elapsed();
        println!("find_path : {n} chemins en {:.1} ms ({:.2} ms chacun)", a_star.as_secs_f64() * 1e3, a_star.as_secs_f64() * 1e3 / 60.0);
        println!("find_path_turns : {m} chemins en {:.1} ms ({:.2} ms chacun)", en_tours.as_secs_f64() * 1e3, en_tours.as_secs_f64() * 1e3 / 60.0);
        println!("reachable (40 PM) : {r} cases en {:.1} ms", portee.as_secs_f64() * 1e3);
        assert!(n > 0 && m > 0);
    }
}
