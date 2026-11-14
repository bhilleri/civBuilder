//! Routes construites par les villes (voir `docs/conception/conception-routes.md`).
//!
//! Logique pure : coût de construction d'une case selon son biome et son relief, tracé le plus
//! rapide une fois construit (par des points de passage), chantier construit tronçon par tronçon
//! au fur et à mesure de la production. Un tronçon relie deux cases voisines ; il passe une
//! rivière par une passerelle (comprise) et un fleuve par un pont, que la route construit.
//! Effet sur le déplacement : `WorldMap::route_step` (unités) et `echanges::Porteur` (porteurs).

use crate::hex::Hex;
use crate::map::{BridgeAt, Relief, Terrain, WorldMap};

/// Niveau d'un chemin (le premier, ouvert dès le départ ; décision de Benjamin, 14/11/2026).
pub const NIVEAU_CHEMIN: u8 = 1;
/// Coût de base d'une case de chemin (production), en plaine plate.
pub const COUT_CASE: f64 = 4.0;
/// Améliorer une route coûte cette part de sa construction (le tracé est déjà là).
pub const PART_AMELIORATION: f64 = 0.5;

/// Facteur de coût d'un biome : prairie et plaine 1, broussailles et toundra 1,5, forêt 2, jungle,
/// marais et neige 3 (décision de Benjamin, 14/11/2026).
pub fn facteur_biome(t: Terrain) -> f64 {
    match t {
        Terrain::Grassland | Terrain::Plains | Terrain::Desert | Terrain::Savanna | Terrain::Steppe | Terrain::Riverbank | Terrain::Oasis => 1.0,
        Terrain::Mediterranean | Terrain::WoodedSavanna | Terrain::Tundra => 1.5,
        Terrain::Forest | Terrain::Taiga => 2.0,
        Terrain::Jungle | Terrain::Marsh | Terrain::Snow => 3.0,
        Terrain::Ocean | Terrain::Coast | Terrain::Lake => 0.0,
    }
}

/// Altitude (m) au-dessus de laquelle une route de montagne coûte davantage.
pub const HAUTE_MONTAGNE_M: f32 = 2_500.0;

/// Facteur de coût du relief : plat 1, collines 1,5, relief accidenté 2,5, montagne 4, 5 au-dessus
/// de `HAUTE_MONTAGNE_M`.
pub fn facteur_relief(relief: Relief, altitude_m: f32) -> f64 {
    match relief {
        Relief::Flat => 1.0,
        Relief::Hills => 1.5,
        Relief::Rugged => 2.5,
        Relief::Mountain | Relief::Peak if altitude_m > HAUTE_MONTAGNE_M => 5.0,
        Relief::Mountain | Relief::Peak => 4.0,
    }
}

/// Coût d'une case de chemin (production) : `COUT_CASE` × biome × relief ; rien pour un fleuve
/// (c'est son pont qui coûte).
pub fn cout_case(map: &WorldMap, h: Hex) -> f64 {
    if map.is_fleuve(h) {
        return 0.0;
    }
    let Some(t) = map.get(h) else { return 0.0 };
    COUT_CASE * facteur_biome(t) * facteur_relief(map.relief(h), map.elevation(h).unwrap_or(0.0))
}

/// Pont qu'il faut construire pour entrer par la route dans la case de fleuve `vers` depuis `de`
/// (aucun : pas un fleuve, ou il a déjà son pont).
pub fn pont_requis(map: &WorldMap, de: Hex, vers: Hex) -> Option<BridgeAt> {
    if !map.is_fleuve(vers) || map.fleuve_bridge(vers).is_some() {
        return None;
    }
    map.bridge_site(vers, map.wrap().direction(map.canonical(vers), map.canonical(de))?)
}

/// Coût de construction du tronçon `de` → `vers` au niveau `niveau` (production) : la moitié de
/// chaque case, le pont d'un fleuve compris ; rien s'il a déjà ce niveau, `PART_AMELIORATION` pour
/// améliorer une route plus basse.
pub fn cout_troncon(map: &WorldMap, de: Hex, vers: Hex, niveau: u8) -> f64 {
    let deja = map.route(de, vers);
    if deja >= niveau {
        return 0.0;
    }
    let base = (cout_case(map, de) + cout_case(map, vers)) / 2.0;
    let pont = pont_requis(map, de, vers).map_or(0.0, |at| crate::rules::bridge_cost(map, at) as f64);
    let part = if deja > 0 { PART_AMELIORATION } else { 1.0 };
    base * part + pont
}

/// Case de terre où une route peut passer (ni eau, ni glace, ni pic, ni fleuve : le fleuve se
/// franchit d'une rive à l'autre).
fn terre_ferme(map: &WorldMap, h: Hex) -> bool {
    map.contains(h) && !map.is_fleuve(h) && !map.is_ice(h) && map.get(h).is_some_and(|t| !t.is_water()) && map.relief(h) != Relief::Peak
}

/// Pas possibles d'une route depuis la case de terre `a` : vers une voisine de terre ferme
/// (sans falaise), ou par-dessus une case de fleuve jusqu'à la rive opposée (son pont, à construire
/// ou déjà là dans cet axe). Chaque pas : les cases traversées (une ou deux) et son temps une fois
/// construit (PM des unités).
fn pas_de_route(map: &WorldMap, a: Hex, connu: &impl Fn(Hex) -> bool) -> Vec<(Vec<Hex>, u32)> {
    let wrap = map.wrap();
    let mut out = Vec::new();
    for d in 0..6 {
        let n = map.canonical(wrap.neighbor(a, d));
        if !map.contains(n) || !connu(n) {
            continue;
        }
        if map.is_fleuve(n) {
            let b = map.canonical(wrap.neighbor(n, d));
            let axe_ok = map.fleuve_bridge(n).is_none_or(|axis| axis == d % 3);
            if axe_ok && terre_ferme(map, b) && connu(b) && map.bridge_site(n, d).is_some()
                && let Some(s) = map.route_step(n, b)
            {
                out.push((vec![n, b], crate::map::BRIDGE_MOVE_COST + s.terrain));
            }
            continue;
        }
        if terre_ferme(map, n) && !map.cliff_between(a, n)
            && let Some(s) = map.route_step(a, n)
        {
            out.push((vec![n], s.terrain));
        }
    }
    out
}

/// Tracé le plus rapide une fois construit de `a` à `b` (cases, `a` comprise), à temps égal le
/// moins cher à construire au niveau `niveau` ; seulement par des cases connues (`connu`).
pub fn tracer_troncon(map: &WorldMap, a: Hex, b: Hex, niveau: u8, connu: &impl Fn(Hex) -> bool) -> Option<Vec<Hex>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let (a, b) = (map.canonical(a), map.canonical(b));
    if !terre_ferme(map, a) || !terre_ferme(map, b) {
        return None;
    }
    if a == b {
        return Some(vec![a]);
    }
    let n = (map.width * map.height) as usize;
    let mut meilleur = vec![u64::MAX; n];
    let mut venu: Vec<Option<(usize, Option<Hex>)>> = vec![None; n];
    let mut tas = BinaryHeap::new();
    meilleur[map.cell(a)] = 0;
    tas.push(Reverse((0u64, map.cell(a))));
    let cible = map.cell(b);
    while let Some(Reverse((c, i))) = tas.pop() {
        if c > meilleur[i] {
            continue;
        }
        if i == cible {
            break;
        }
        let h = map.hex_at(i);
        for (cases, temps) in pas_de_route(map, h, connu) {
            let mut prix = 0.0;
            let mut de = h;
            for &x in &cases {
                prix += cout_troncon(map, de, x, niveau);
                de = x;
            }
            // (le temps d'abord, puis le prix au dixième)
            let poids = c + u64::from(temps) * 1_000_000 + (prix * 10.0).round() as u64;
            let j = map.cell(*cases.last().unwrap());
            if poids < meilleur[j] {
                meilleur[j] = poids;
                venu[j] = Some((i, (cases.len() == 2).then_some(cases[0])));
                tas.push(Reverse((poids, j)));
            }
        }
    }
    venu[cible]?;
    let mut chemin = vec![b];
    let mut j = cible;
    while let Some((i, fleuve)) = venu[j] {
        chemin.extend(fleuve);
        chemin.push(map.hex_at(i));
        j = i;
    }
    chemin.reverse();
    Some(chemin)
}

/// Tracé par des points de passage (le départ, puis chacun dans l'ordre).
pub fn tracer(map: &WorldMap, points: &[Hex], niveau: u8, connu: &impl Fn(Hex) -> bool) -> Option<Vec<Hex>> {
    let mut trace: Vec<Hex> = vec![map.canonical(*points.first()?)];
    for w in points.windows(2) {
        let t = tracer_troncon(map, w[0], w[1], niveau, connu)?;
        trace.extend(t.into_iter().skip(1));
    }
    Some(trace)
}

/// Coût total de construction d'un tracé.
pub fn cout_trace(map: &WorldMap, trace: &[Hex], niveau: u8) -> f64 {
    trace.windows(2).map(|w| cout_troncon(map, w[0], w[1], niveau)).sum()
}

/// Temps d'un porteur le long d'un tracé (tours), tel qu'est la carte (`None` : un pas impossible).
pub fn tours_porteur(map: &WorldMap, trace: &[Hex]) -> Option<f64> {
    let porteur = crate::echanges::Porteur(map);
    let mut demi = 0u32;
    for w in trace.windows(2) {
        demi += crate::map::MoveCosts::step_cost(&porteur, w[0], w[1])?;
    }
    Some(demi as f64 / 2.0 / crate::echanges::PM_PORTEUR as f64)
}

/// Temps d'un porteur le long d'un tracé une fois la route construite (tours) : 0,5 point au moins
/// par case, le pont d'un fleuve 1 point.
pub fn tours_porteur_avec_route(map: &WorldMap, trace: &[Hex]) -> Option<f64> {
    let mut demi = 0u32;
    for w in trace.windows(2) {
        demi += if map.is_fleuve(w[1]) { 2 } else { map.route_step(w[0], w[1])?.terrain.max(1) };
    }
    Some(demi as f64 / 2.0 / crate::echanges::PM_PORTEUR as f64)
}

/// Pose une route sur tout un tracé, ponts des fleuves compris (console, tests, aperçu).
pub fn poser(map: &mut WorldMap, trace: &[Hex], niveau: u8) {
    for w in trace.windows(2) {
        let t = Troncon { de: w[0], vers: w[1], pont: pont_requis(map, w[0], w[1]) };
        t.poser(map, niveau);
    }
}

/// Un tronçon achevé : ses deux cases, et le pont de fleuve qu'il a fallu bâtir.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Troncon {
    pub de: Hex,
    pub vers: Hex,
    pub pont: Option<BridgeAt>,
}

impl Troncon {
    /// Pose ce tronçon sur la carte (route de ce niveau, et son pont).
    pub fn poser(self, map: &mut WorldMap, niveau: u8) {
        if let Some(at) = self.pont {
            map.set_bridge(at, true);
        }
        if let Some(at) = map.route_at(self.de, self.vers)
            && map.route(self.de, self.vers) < niveau
        {
            map.set_route(at, niveau);
        }
    }
}

/// Chantier de route d'une ville : son tracé, construit tronçon par tronçon dans l'ordre.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Chantier {
    /// Numéro du chantier dans la ville (la commande `Production::Route` le désigne).
    pub id: u32,
    pub trace: Vec<Hex>,
    pub niveau: u8,
    /// Tronçons déjà faits (depuis le début du tracé).
    pub faits: usize,
    /// Coût total annoncé quand il a été lancé.
    pub total: i32,
}

impl Chantier {
    /// Tronçons du tracé.
    pub fn troncons(&self) -> usize {
        self.trace.len().saturating_sub(1)
    }

    /// Fini : tous les tronçons sont faits.
    pub fn fini(&self) -> bool {
        self.faits >= self.troncons()
    }

    /// Ce qu'il reste à payer, selon la carte d'aujourd'hui (les tronçons qu'une autre route a
    /// faits entre-temps ne coûtent plus rien).
    pub fn restant(&self, map: &WorldMap) -> f64 {
        self.trace[self.faits.min(self.trace.len())..].windows(2).map(|w| cout_troncon(map, w[0], w[1], self.niveau)).sum()
    }

    /// Avance le chantier avec la production investie : fait les tronçons qu'elle paie, dans
    /// l'ordre (le reste attend le tour suivant), et les rend, à poser sur la carte.
    pub fn avancer(&mut self, map: &WorldMap, investi: &mut f64) -> Vec<Troncon> {
        let mut faits = Vec::new();
        while !self.fini() {
            let (de, vers) = (self.trace[self.faits], self.trace[self.faits + 1]);
            let cout = cout_troncon(map, de, vers, self.niveau);
            if cout > *investi + 1e-9 {
                break;
            }
            *investi = (*investi - cout).max(0.0);
            self.faits += 1;
            faits.push(Troncon { de, vers, pont: pont_requis(map, de, vers) });
        }
        faits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MoveCosts;

    fn ligne(m: &WorldMap, col0: i32, col1: i32, row: i32) -> Vec<Hex> {
        (col0..=col1).map(|c| Hex::from_offset(c, row)).filter(|h| m.contains(*h)).collect()
    }

    #[test]
    fn une_route_rend_la_foret_et_le_marais_comme_la_plaine() {
        let mut m = WorldMap::filled(12, 8, Terrain::Forest);
        let (a, b) = (Hex::from_offset(3, 3), Hex::from_offset(4, 3));
        assert_eq!(m.step_cost(a, b), Some(4));
        let porteur = crate::echanges::Porteur(&m);
        assert_eq!(porteur.step_cost(a, b), Some(4), "2 points de porteur, en demi-points");
        m.set_route(m.route_at(a, b).unwrap(), NIVEAU_CHEMIN);
        assert_eq!(m.step_cost(a, b), Some(1));
        assert_eq!(m.step_cost(b, a), Some(1), "dans les deux sens");
        let porteur = crate::echanges::Porteur(&m);
        assert_eq!(porteur.step_cost(a, b), Some(1), "0,5 point de porteur");
        // Le marais aussi ; la plaine reste à 1 PM pour les unités, 0,5 pour les porteurs
        m.set(b, Terrain::Marsh);
        assert_eq!(m.step_cost(a, b), Some(1));
        m.set(b, Terrain::Grassland);
        assert_eq!(m.step_cost(a, b), Some(1));
    }

    #[test]
    fn la_montagne_reste_chere_sur_la_route() {
        let mut m = WorldMap::filled(12, 8, Terrain::Grassland);
        let (a, b) = (Hex::from_offset(3, 3), Hex::from_offset(4, 3));
        m.set_relief(b, Relief::Mountain);
        m.set_elevation(a, 1_800.0);
        m.set_elevation(b, 2_100.0);
        let sans = m.step_cost(a, b).unwrap();
        m.set_route(m.route_at(a, b).unwrap(), NIVEAU_CHEMIN);
        let avec = m.step_cost(a, b).unwrap();
        assert_eq!(avec, sans / 2, "la moitié du coût de la montagne");
        assert!(avec >= 4, "au moins un tour d'unité : {avec}");
        // Collines : 2 ; relief accidenté : 3 ; une forte montée compte la moitié
        m.set_relief(b, Relief::Hills);
        m.set_elevation(b, 1_800.0);
        assert_eq!(m.step_cost(a, b), Some(crate::map::ROUTE_HILLS_PM));
        m.set_relief(b, Relief::Rugged);
        assert_eq!(m.step_cost(a, b), Some(3));
        m.set_elevation(b, 2_800.0);
        assert_eq!(m.step_cost(a, b), Some(3 + 2), "montée de plus de 900 m : +4 sans route, +2 avec");
    }

    #[test]
    fn le_cout_suit_le_biome_et_le_relief() {
        let mut m = WorldMap::filled(12, 8, Terrain::Grassland);
        let h = Hex::from_offset(3, 3);
        assert_eq!(cout_case(&m, h), 4.0);
        m.set(h, Terrain::Forest);
        m.set_relief(h, Relief::Hills);
        assert_eq!(cout_case(&m, h), 12.0, "forêt en colline : 4 × 2 × 1,5");
        m.set(h, Terrain::Marsh);
        m.set_relief(h, Relief::Flat);
        assert_eq!(cout_case(&m, h), 12.0);
        m.set(h, Terrain::Grassland);
        m.set_relief(h, Relief::Mountain);
        m.set_elevation(h, 2_000.0);
        assert_eq!(cout_case(&m, h), 16.0);
        m.set_elevation(h, 2_700.0);
        assert_eq!(cout_case(&m, h), 20.0);
        // Un tronçon : la moitié de chaque case ; déjà fait, rien ; amélioration, la moitié
        let (a, b) = (Hex::from_offset(5, 3), Hex::from_offset(6, 3));
        assert_eq!(cout_troncon(&m, a, b, 1), 4.0);
        m.set_route(m.route_at(a, b).unwrap(), 1);
        assert_eq!(cout_troncon(&m, a, b, 1), 0.0);
        assert_eq!(cout_troncon(&m, a, b, 2), 2.0);
    }

    #[test]
    fn le_trace_evite_le_marais_a_temps_egal_et_franchit_le_fleuve_par_un_pont() {
        let mut m = WorldMap::filled(16, 9, Terrain::Grassland);
        let tout = |_: Hex| true;
        let (a, b) = (Hex::from_offset(2, 4), Hex::from_offset(10, 4));
        // Un marais sur le tracé : à temps égal (autant de cases), le nouveau tracé l'évite
        let (c, d) = (Hex::from_offset(2, 1), Hex::from_offset(9, 7));
        let t0 = tracer(&m, &[c, d], 1, &tout).unwrap();
        assert_eq!((t0[0], *t0.last().unwrap()), (c, d));
        let milieu = t0[t0.len() / 2];
        m.set(milieu, Terrain::Marsh);
        let t = tracer(&m, &[c, d], 1, &tout).unwrap();
        assert_eq!(t.len(), t0.len());
        assert!(!t.contains(&milieu) && cout_trace(&m, &t, 1) < cout_trace(&m, &t0, 1));
        // Un fleuve en travers : le tracé le passe tout droit, et le pont est compté
        let mut m = WorldMap::filled(16, 9, Terrain::Grassland);
        for r in 0..9 {
            m.set_fleuve(Hex::from_offset(6, r));
        }
        let t = tracer(&m, &[a, b], 1, &tout).unwrap();
        let f = t.iter().position(|h| m.is_fleuve(*h)).expect("passe le fleuve");
        assert!(pont_requis(&m, t[f - 1], t[f]).is_some());
        assert!(cout_trace(&m, &t, 1) >= crate::rules::BRIDGE_COST.0 as f64);
        poser(&mut m, &t, 1);
        assert!(m.fleuve_bridge(t[f]).is_some(), "la route a bâti le pont");
        assert_eq!(cout_trace(&m, &t, 1), 0.0);
        // Seulement par les cases connues
        let connu = |h: Hex| h.to_offset().1 == 4 || h.to_offset().0 != 6;
        assert!(tracer(&m, &[a, b], 1, &connu).is_some());
        let rien = |h: Hex| h.to_offset().0 != 6;
        let m2 = WorldMap::filled(16, 9, Terrain::Grassland);
        assert!(tracer(&m2, &[a, b], 1, &rien).is_none());
    }

    #[test]
    fn le_point_de_passage_detourne_le_trace() {
        let m = WorldMap::filled(16, 12, Terrain::Grassland);
        let tout = |_: Hex| true;
        let (a, b, p) = (Hex::from_offset(2, 5), Hex::from_offset(12, 5), Hex::from_offset(7, 9));
        let t = tracer(&m, &[a, p, b], 1, &tout).unwrap();
        assert!(t.contains(&p));
        assert!(t.windows(2).all(|w| m.wrap().distance(w[0], w[1]) == 1), "des cases voisines");
        assert!(t.len() > tracer(&m, &[a, b], 1, &tout).unwrap().len());
    }

    #[test]
    fn le_chantier_avance_avec_la_production_et_saute_ce_qui_est_fait() {
        let mut m = WorldMap::filled(16, 9, Terrain::Forest);
        let trace = ligne(&m, 2, 8, 4);
        let mut ch = Chantier { id: 1, total: cout_trace(&m, &trace, 1).round() as i32, trace: trace.clone(), niveau: 1, faits: 0 };
        assert_eq!(ch.total, 6 * 8);
        // Une autre route a déjà fait le 3e tronçon
        m.set_route(m.route_at(trace[2], trace[3]).unwrap(), 1);
        assert_eq!(ch.restant(&m), 40.0);
        let mut investi = 20.0;
        let faits = ch.avancer(&m, &mut investi);
        assert_eq!(faits.len(), 3, "deux tronçons payés et celui déjà fait");
        assert!((investi - 4.0).abs() < 1e-9);
        for t in faits {
            t.poser(&mut m, 1);
        }
        assert_eq!(ch.restant(&m), 24.0);
        let mut investi = 100.0;
        ch.avancer(&m, &mut investi);
        assert!(ch.fini() && (investi - 76.0).abs() < 1e-9);
    }

    #[test]
    fn la_file_garde_le_chantier_tant_qu_il_n_est_pas_fini() {
        let mut c = crate::rules::CityState::new("Chantier".into());
        let id = c.lancer_route(vec![Hex::from_offset(1, 1), Hex::from_offset(2, 1)], NIVEAU_CHEMIN, 4);
        assert_eq!(c.chantier(id).map(|ch| ch.troncons()), Some(1));
        // (la production dépasse le coût : c'est le jeu qui pose les tronçons, pas la file)
        let r = c.process_turn(crate::intendant::Rendements::new(3.0, 20.0, 0.0));
        assert_eq!(r.built, None);
        assert!(matches!(c.file[0].objet, crate::rules::Production::Route { id: i, .. } if i == id) && c.file[0].investi >= 20.0);
        assert_eq!(c.lancer_route(vec![Hex::from_offset(1, 1)], NIVEAU_CHEMIN, 0), id + 1, "un numéro par chantier");
    }

    #[test]
    fn le_porteur_va_deux_fois_plus_vite_en_plaine() {
        let mut m = WorldMap::filled(20, 9, Terrain::Grassland);
        let trace = ligne(&m, 2, 10, 4);
        let avant = tours_porteur(&m, &trace).unwrap();
        let apres = tours_porteur_avec_route(&m, &trace).unwrap();
        assert!((avant - 2.0).abs() < 1e-9 && (apres - 1.0).abs() < 1e-9, "{avant} {apres}");
        poser(&mut m, &trace, 1);
        let t = crate::echanges::trajet_a_pied(&m, trace[0], *trace.last().unwrap()).unwrap();
        assert!((t - 1.0).abs() < 1e-9, "{t}");
    }
}
