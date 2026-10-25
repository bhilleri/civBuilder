//! Recherche de chemin A* sur la grille hexagonale (logique pure).

use crate::hex::Hex;
use crate::map::MoveCosts;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Chemin le moins coûteux de `start` à `goal` (start exclu, goal inclus).
/// `None` si la destination est inaccessible.
pub fn find_path(map: &impl MoveCosts, start: Hex, goal: Hex) -> Option<Vec<Hex>> {
    if start == goal {
        return Some(Vec::new());
    }
    map.move_cost(goal)?;

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<Hex, Hex> = HashMap::new();
    let mut cost: HashMap<Hex, u32> = HashMap::from([(start, 0)]);
    let wrap = map.wrap();
    open.push(Reverse((wrap.distance(start, goal) as u32, start)));

    while let Some(Reverse((_, current))) = open.pop() {
        if current == goal {
            let mut path = vec![goal];
            let mut node = goal;
            while let Some(&prev) = came_from.get(&node) {
                if prev == start {
                    break;
                }
                path.push(prev);
                node = prev;
            }
            path.reverse();
            return Some(path);
        }
        let current_cost = cost[&current];
        for next in map.wrap().neighbors(current) {
            let Some(step) = map.step_cost(current, next) else { continue };
            let new_cost = current_cost + step;
            if cost.get(&next).is_none_or(|&c| new_cost < c) {
                cost.insert(next, new_cost);
                came_from.insert(next, current);
                open.push(Reverse((new_cost + wrap.distance(next, goal) as u32, next)));
            }
        }
    }
    None
}

/// Chemin le plus rapide pour une unité, en tours puis en PM restants (et non en coût total) :
/// on peut entrer sur une case tant qu'il reste au moins 1 PM, quel que soit son coût. Une
/// traversée chère (rivière, fleuve) faite d'un coup peut donc arriver plus tôt qu'un détour
/// moins coûteux en tout. Même règle que `reachable` et `rules::path_turns`.
pub fn find_path_turns(map: &impl MoveCosts, start: Hex, goal: Hex, moves_left: u32, max_moves: u32) -> Option<Vec<Hex>> {
    if start == goal {
        return Some(Vec::new());
    }
    map.move_cost(goal)?;
    let max_moves = max_moves.max(1);
    // Clé : (tours commencés, PM dépensés dans le tour), à minimiser ; une unité sans PM
    // restant repart au tour suivant avec tous ses PM
    let key = |turn: u32, left: u32| (u64::from(turn) << 32) | u64::from(u32::MAX - left);
    let mut best: HashMap<Hex, (u64, u32, u32)> = HashMap::from([(start, (key(0, moves_left), 0, moves_left))]);
    let mut came_from: HashMap<Hex, Hex> = HashMap::new();
    let mut open = BinaryHeap::from([Reverse((key(0, moves_left), start))]);
    while let Some(Reverse((k, current))) = open.pop() {
        let (bk, turn, left) = best[&current];
        if k > bk {
            continue;
        }
        if current == goal {
            let mut path = vec![goal];
            let mut node = goal;
            while let Some(&prev) = came_from.get(&node) {
                if prev == start {
                    break;
                }
                path.push(prev);
                node = prev;
            }
            path.reverse();
            return Some(path);
        }
        let (turn, left) = if left == 0 { (turn + 1, max_moves) } else { (turn, left) };
        for next in map.wrap().neighbors(current) {
            let Some(step) = map.step_cost(current, next) else { continue };
            let after = left.saturating_sub(step);
            let nk = key(turn, after);
            if best.get(&next).is_none_or(|&(b, _, _)| nk < b) {
                best.insert(next, (nk, turn, after));
                came_from.insert(next, current);
                open.push(Reverse((nk, next)));
            }
        }
    }
    None
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
        let fastest = find_path_turns(&map, a, b, 1, 2).unwrap();
        assert_eq!(fastest, vec![b]);
        assert_eq!(path_turns(&map, a, &fastest, 1, 2), vec![1]);
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
                    let this_turn = find_path_turns(&map, start, target, moves, moves)
                        .is_some_and(|p| path_turns(&map, start, &p, moves, moves).last() == Some(&1));
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
        let mut explored: HashSet<Hex> = a.range(2).collect();

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
        let mut explored: HashSet<Hex> = a.range(1).collect();
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
        let explored = HashSet::from([Hex::from_offset(1, 1)]);
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
        // Plateau de niveau 4 à droite de la colonne 5, falaise sauf une rampe (niveaux 2 → 3 → 4)
        let mut map = WorldMap::filled(12, 9, Terrain::Plains);
        for h in map.hexes().collect::<Vec<_>>() {
            let (col, _) = h.to_offset();
            if col >= 5 {
                map.set_level(h, 4);
            }
        }
        let ramp = [Hex::from_offset(3, 7), Hex::from_offset(4, 7)];
        map.set_level(ramp[0], 2);
        map.set_level(ramp[1], 3);
        let (start, goal) = (Hex::from_offset(2, 1), Hex::from_offset(7, 1));
        let path = find_path(&map, start, goal).expect("la rampe permet de monter");
        assert!(path.contains(&ramp[1]), "le trajet passe par la rampe : {path:?}");
        // Sans rampe : inaccessible
        map.set_level(ramp[1], 1);
        map.set_level(ramp[0], 1);
        assert!(find_path(&map, start, goal).is_none());
    }
}
