//! Période nomade (logique pure, `docs/conception/conception-nomade.md`).
//!
//! Chaque peuple commence nomade : une **bande** de chasseurs-cueilleurs erre, se nourrit de la
//! cueillette des cases autour d'elle (qui s'épuisent si elle reste), et observe les plantes et les
//! bêtes sauvages domesticables. L'**Agriculture** se découvre ainsi ; elle est requise pour fonder
//! la première ville. Décisions de Benjamin (14/11/2026) pour que les premiers et les derniers
//! peuples à fonder ne soient séparés que de quelques tours :
//! - **départs équitables** : chaque peuple part à portée de céréales et de bêtes sauvages ;
//! - **fenêtre étroite** : la découverte prend 18 à 22 tours en errant bien, 24 à 29 sur place ;
//! - **rattrapage** : un peuple qui voit des villes d'agriculteurs apprend plus vite (diffusion),
//!   et une bande qui a erré plus longtemps a grandi : sa ville part plus peuplée.

use crate::hex::Hex;
use crate::intendant::{self, Amenagement, Sorte};
use crate::map::WorldMap;
use crate::peuple::Peuple;
use std::collections::BTreeMap;

/// Membres d'une bande au départ (autant qu'un colon : la ville fondée reprend ses membres).
pub const MEMBRES_BANDE: u64 = crate::rules::HABITANTS_COLON;
/// Membres en dessous desquels la faim ne fait plus diminuer une bande.
pub const PLANCHER_BANDE: u64 = 300;
/// Points de l'Agriculture à réunir.
pub const AGRICULTURE: f64 = 100.0;
/// Apport de chaque tour : ce que la bande apprend d'elle-même (sa Recherche).
pub const APPORT_DE_BASE: f64 = 1.2;
/// Apport de chaque tour par sorte domesticable sur la case de la bande ou une voisine (4 au plus).
pub const APPORT_PAR_SORTE_PROCHE: f64 = 0.4;
pub const SORTES_PROCHES_MAX: u32 = 4;
/// Apport, une fois, de chaque sorte domesticable vue pour la première fois.
pub const BONUS_SORTE_NOUVELLE: f64 = 5.0;
/// Diffusion : apport de chaque tour par ville d'un autre peuple (qui connaît l'agriculture) à
/// portée, et son plafond.
pub const DIFFUSION_PAR_VILLE: f64 = 1.5;
pub const DIFFUSION_MAX: f64 = 3.0;
pub const PORTEE_DIFFUSION: i32 = 8;
/// Rayon où la bande observe les plantes et les bêtes (sa vue).
pub const RAYON_OBSERVATION: i32 = 2;
/// Épuisement : part de ce qui reste que la bande prélève à chaque tour passé sur une case (ou à
/// côté), et part de l'épuisement qui repousse à chaque tour (les trois quarts en un an) : au bout
/// de quatre à cinq tours au même endroit, la bande a faim.
pub const PRELEVEMENT: f32 = 0.4;
pub const REPOUSSE: f32 = 0.1;
/// Départ équitable : sortes domesticables (une céréale et une bête au moins) dans ce rayon.
pub const RAYON_DEPART: i32 = 5;
pub const SORTES_DEPART: u32 = 3;

/// Sorte de bétail (et non de plante).
pub const fn betail(s: Sorte) -> bool {
    matches!(s, Sorte::Boeufs | Sorte::MoutonsChevres | Sorte::Porcs | Sorte::BetesExtremes)
}

/// Bit de la sorte dans un ensemble de sortes.
fn bit(s: Sorte) -> u32 {
    1 << s.index()
}

/// Sortes d'un ensemble, dans l'ordre de `Sorte::TOUTES`.
pub fn sortes_de(ensemble: u32) -> Vec<Sorte> {
    Sorte::TOUTES.into_iter().filter(|&s| ensemble & bit(s) != 0).collect()
}

/// Ancêtres sauvages domesticables de la case : la céréale qu'y donnerait un champ et les
/// légumineuses (climat tempéré à subtropical), sur les terres ouvertes (les tubercules dans la
/// jungle et la savane arborée) ; le bétail d'un pâturage (les porcs en forêt, les rennes dans la
/// taïga).
pub fn sortes_sauvages(map: &WorldMap, h: Hex) -> u32 {
    use crate::map::Terrain;
    let mut e = 0;
    let boise = matches!(map.get(h), Some(Terrain::Forest | Terrain::Taiga));
    if !boise && Amenagement::Champ.possible(map, h).is_ok() {
        if let Some(s) = intendant::sortes_de_la_case(map, h, Some(Amenagement::Champ))[0] {
            e |= bit(s);
        }
        if map.temperature(h).is_some_and(|t| (8.0..=24.0).contains(&t)) {
            e |= bit(Sorte::Legumineuses);
        }
    }
    if Amenagement::Paturage.possible(map, h).is_ok()
        && let Some(s) = intendant::sortes_de_la_case(map, h, Some(Amenagement::Paturage))[1]
    {
        e |= bit(s);
    }
    e
}

/// Sortes domesticables des cases à `rayon` de `h` (au plus).
pub fn sortes_autour(map: &WorldMap, h: Hex, rayon: i32) -> u32 {
    map.range(h, rayon).fold(0, |e, c| e | sortes_sauvages(map, c))
}

/// Départ équitable : assez de sortes domesticables à portée, dont une céréale et une bête.
pub fn depart_equitable(map: &WorldMap, h: Hex) -> bool {
    let e = sortes_autour(map, h, RAYON_DEPART);
    let s = sortes_de(e);
    e.count_ones() >= SORTES_DEPART && s.iter().any(|&x| betail(x)) && s.iter().any(|&x| !betail(x))
}

/// Ce qu'un peuple sait de l'Agriculture.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Savoir {
    /// Points réunis (sur `AGRICULTURE`).
    pub agriculture: f64,
    /// L'Agriculture est connue : la bande peut fonder une ville.
    pub connue: bool,
    /// Sortes domesticables déjà observées (bits de `Sorte::index`).
    pub observees: u32,
}

/// Apport d'un tour à l'Agriculture, terme à terme.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Apport {
    pub base: f64,
    /// Sortes domesticables tout près (case de la bande et voisines).
    pub proches: f64,
    /// Sortes vues pour la première fois ce tour.
    pub nouvelles: Vec<Sorte>,
    pub decouverte: f64,
    /// Villes d'agriculteurs à portée.
    pub diffusion: f64,
}

impl Apport {
    pub fn total(&self) -> f64 {
        self.base + self.proches + self.decouverte + self.diffusion
    }
}

impl Savoir {
    /// Apport d'un tour : `vues` (sortes dans le rayon d'observation), `proches` (sur la case et ses
    /// voisines), `villes_agricoles` (villes d'autres peuples à portée).
    pub fn apport(&self, vues: u32, proches: u32, villes_agricoles: usize) -> Apport {
        let nouvelles = sortes_de(vues & !self.observees);
        Apport {
            base: APPORT_DE_BASE,
            proches: APPORT_PAR_SORTE_PROCHE * f64::from(proches.count_ones().min(SORTES_PROCHES_MAX)),
            decouverte: BONUS_SORTE_NOUVELLE * nouvelles.len() as f64,
            nouvelles,
            diffusion: (DIFFUSION_PAR_VILLE * villes_agricoles as f64).min(DIFFUSION_MAX),
        }
    }

    /// Ajoute l'apport du tour ; rend vrai si l'Agriculture vient d'être découverte.
    pub fn avancer(&mut self, apport: &Apport, vues: u32) -> bool {
        self.observees |= vues;
        if self.connue {
            return false;
        }
        self.agriculture = (self.agriculture + apport.total()).min(AGRICULTURE);
        if self.agriculture >= AGRICULTURE - 1e-9 {
            self.connue = true;
            return true;
        }
        false
    }

    /// Tours qu'il faudrait encore au même rythme.
    pub fn tours_restants(&self, par_tour: f64) -> Option<u32> {
        if self.connue {
            Some(0)
        } else if par_tour > 1e-9 {
            Some(((AGRICULTURE - self.agriculture) / par_tour).ceil() as u32)
        } else {
            None
        }
    }
}

/// Période nomade de la partie : le savoir de chaque peuple et l'épuisement des cases cueillies.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Nomadisme {
    /// Savoir de chaque peuple (par `Peuple::index`).
    pub peuples: Vec<Savoir>,
    /// Épuisement des cases (0 : intactes, 1 : plus rien à cueillir) ; seules les cases touchées.
    pub epuisement: BTreeMap<Hex, f32>,
}

impl Nomadisme {
    /// Savoir d'un peuple.
    pub fn savoir(&self, p: Peuple) -> Savoir {
        self.peuples.get(p.index()).cloned().unwrap_or_default()
    }

    /// Savoir d'un peuple, à modifier.
    pub fn savoir_mut(&mut self, p: Peuple) -> &mut Savoir {
        if self.peuples.len() <= p.index() {
            self.peuples.resize(p.index() + 1, Savoir::default());
        }
        &mut self.peuples[p.index()]
    }

    /// Le peuple connaît l'Agriculture.
    pub fn agriculture_connue(&self, p: Peuple) -> bool {
        self.peuples.get(p.index()).is_some_and(|s| s.connue)
    }

    /// Épuisement d'une case.
    pub fn epuisement(&self, h: Hex) -> f32 {
        self.epuisement.get(&h).copied().unwrap_or(0.0)
    }

    /// Un tour d'épuisement : les cases cueillies (`cueillies`) perdent `PRELEVEMENT` de ce qui leur
    /// reste ; les autres repoussent.
    pub fn epuiser(&mut self, cueillies: &[Hex]) {
        for e in self.epuisement.values_mut() {
            *e *= 1.0 - REPOUSSE;
        }
        for &h in cueillies {
            let e = self.epuisement.entry(h).or_insert(0.0);
            *e += PRELEVEMENT * (1.0 - *e);
        }
        self.epuisement.retain(|_, e| *e >= 0.01);
    }
}

/// Cases où cueille une bande : la sienne et ses voisines, sur terre ou en eau qui nourrit.
pub fn cases_cueillies(map: &WorldMap, h: Hex) -> Vec<Hex> {
    map.range(h, 1).filter(|&c| !map.is_ice(c) && map.yields(c).is_some_and(|y| y.food > 0)).collect()
}

/// Nourriture d'une bande de `membres` en `h` : la cueillette de ses cases (actifs répartis à parts
/// égales), diminuée de leur épuisement.
pub fn nourriture_bande(map: &WorldMap, h: Hex, membres: u64, nomadisme: &Nomadisme) -> f64 {
    let cases = cases_cueillies(map, h);
    if cases.is_empty() {
        return 0.0;
    }
    let par_case = intendant::actifs(membres) / cases.len() as u32;
    cases
        .iter()
        .map(|&c| intendant::courbe(map, c, None).rendement(par_case).nourriture * f64::from(1.0 - nomadisme.epuisement(c)))
        .sum()
}

/// Variation des membres d'une bande en un tour : comme une ville (croissance naturelle avec un
/// surplus, famine quand la nourriture manque), sans descendre sous `PLANCHER_BANDE` par la faim.
pub fn variation_bande(membres: u64, nourriture: f64) -> i64 {
    use crate::rules::{CROISSANCE_NATURELLE, FAMINE_MAX, FAMINE_MIN, HABITANTS_PAR_NOURRITURE, SURPLUS_POUR_CROITRE};
    let besoin = (membres as f64 / HABITANTS_PAR_NOURRITURE).max(0.001);
    let couverture = nourriture.max(0.0) / besoin;
    let taux = if couverture >= 1.0 {
        CROISSANCE_NATURELLE * ((couverture - 1.0) / SURPLUS_POUR_CROITRE).min(1.0)
    } else {
        -(FAMINE_MIN + (FAMINE_MAX - FAMINE_MIN) * (1.0 - couverture))
    };
    let v = (membres as f64 * taux).round() as i64;
    if v < 0 { v.max(-(membres.saturating_sub(PLANCHER_BANDE) as i64)) } else { v }
}

/// Fiche d'une bande en `at` (panneau de la sélection) : membres, nourriture, Agriculture.
pub fn fiche(map: &WorldMap, n: &Nomadisme, peuple: Peuple, at: Hex, membres: u64) -> String {
    use crate::rules::{decimal, milliers, HABITANTS_PAR_NOURRITURE};
    let nourriture = nourriture_bande(map, at, membres, n);
    let besoin = membres as f64 / HABITANTS_PAR_NOURRITURE;
    let v = variation_bande(membres, nourriture);
    let variation = if v >= 0 { format!("+{} personnes par tour", milliers(v as u64)) } else { format!("famine : −{} par tour", milliers(v.unsigned_abs())) };
    let cases = cases_cueillies(map, at);
    let epuisement = if cases.is_empty() { 0.0 } else { cases.iter().map(|&c| n.epuisement(c)).sum::<f32>() / cases.len() as f32 };
    let mut s = format!(
        "{} personnes
Cueillette : {} de nourriture pour un besoin de {} ({variation})",
        milliers(membres),
        decimal(nourriture),
        decimal(besoin)
    );
    if epuisement >= 0.05 {
        s += &format!("
Cases autour épuisées à {:.0} % : les trois quarts repoussent en un an", epuisement * 100.0);
    }
    let savoir = n.savoir(peuple);
    if savoir.connue {
        s += "
Agriculture connue : B pour fonder la ville ici (elle reprend les membres de la bande)";
    } else {
        let vues = sortes_autour(map, at, RAYON_OBSERVATION);
        let proches = sortes_autour(map, at, 1);
        let a = savoir.apport(vues, proches, 0);
        let tours = savoir.tours_restants(a.base + a.proches).map_or("—".to_string(), |t| format!("≈ {t} tours"));
        s += &format!("
Agriculture : {:.0} / {:.0} (+{} par tour ici, {tours})", savoir.agriculture, AGRICULTURE, decimal(a.base + a.proches));
        let observees: Vec<&str> = sortes_de(savoir.observees).iter().map(|s| s.nom()).collect();
        if !observees.is_empty() {
            s += &format!("
Observées : {}", observees.join(", "));
        }
        let ici: Vec<&str> = sortes_de(proches).iter().map(|s| s.nom()).collect();
        s += &if ici.is_empty() { "
Rien de domesticable tout près : cherchez céréales et bêtes sauvages".to_string() } else { format!("
Tout près : {}", ici.join(", ")) };
        let nouvelles: Vec<&str> = sortes_de(vues & !savoir.observees).iter().map(|s| s.nom()).collect();
        if !nouvelles.is_empty() {
            s += &format!("
À observer au prochain tour : {} (+{:.0} chacune)", nouvelles.join(", "), BONUS_SORTE_NOUVELLE);
        }
        s += "
B : fonder la ville, une fois l'Agriculture découverte";
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_decouverte_prend_vingt_a_trente_tours_selon_la_region() {
        // Région riche : sept sortes à découvrir, quatre tout près ; région pauvre (la moins bien
        // dotée des départs équitables, immobile) : quatre, deux
        let tours = |nouvelles: u32, proches: u32, toutes: u32| {
            let mut s = Savoir::default();
            let vues = (1u32 << nouvelles) - 1;
            let pres = (1u32 << proches) - 1;
            let mut t = 0;
            while !s.connue {
                let a = s.apport(vues, if t % 2 == 0 { pres } else { (1u32 << toutes) - 1 }, 0);
                s.avancer(&a, vues);
                t += 1;
            }
            t
        };
        let riche = tours(7, 4, 4);
        let pauvre = tours(4, 2, 2);
        assert!((22..=25).contains(&riche), "riche : {riche}");
        assert!((38..=42).contains(&pauvre), "pauvre : {pauvre}");
    }

    #[test]
    fn une_sorte_ne_compte_qu_une_fois_et_la_diffusion_aide_les_retardataires() {
        let mut s = Savoir::default();
        let vues = bit(Sorte::Ble) | bit(Sorte::Boeufs);
        let a = s.apport(vues, 0, 0);
        assert_eq!(a.nouvelles, vec![Sorte::Ble, Sorte::Boeufs]);
        s.avancer(&a, vues);
        assert!(s.apport(vues, 0, 0).nouvelles.is_empty(), "déjà observées");
        assert_eq!(s.apport(0, 0, 1).diffusion, DIFFUSION_PAR_VILLE);
        assert_eq!(s.apport(0, 0, 5).diffusion, DIFFUSION_MAX, "plafond");
    }

    #[test]
    fn les_cases_cueillies_s_epuisent_puis_repoussent() {
        let mut n = Nomadisme::default();
        let h = Hex::new(3, 3);
        for _ in 0..3 {
            n.epuiser(&[h]);
        }
        let e = n.epuisement(h);
        assert!((0.7..0.8).contains(&e), "trois tours : {e}");
        for _ in 0..12 {
            n.epuiser(&[]);
        }
        assert!(n.epuisement(h) < 0.25, "un an après : {}", n.epuisement(h));
    }

    #[test]
    fn une_bande_affamee_ne_descend_pas_sous_le_plancher() {
        assert!(variation_bande(1_000, 2.0) > 0, "bien nourrie : elle grandit");
        assert!(variation_bande(1_000, 0.5) < 0);
        assert_eq!(variation_bande(PLANCHER_BANDE + 2, 0.0), -2);
    }

    #[test]
    fn le_depart_est_a_portee_de_cereales_et_de_betes() {
        let map = WorldMap::generate(96, 56, 777);
        let start = map.find_start().unwrap();
        assert!(depart_equitable(&map, start));
        let n = sortes_autour(&map, start, RAYON_DEPART);
        assert!(n.count_ones() >= SORTES_DEPART, "{:?}", sortes_de(n));
        // La bande y trouve de quoi vivre
        assert!(nourriture_bande(&map, start, MEMBRES_BANDE, &Nomadisme::default()) >= 1.0);
    }

    /// Bilan de la période nomade (graines `SEEDS`, 777, 2, 5, 4242 par défaut) : sortes autour du
    /// départ, nourriture, et tours avant l'Agriculture pour une bande qui reste sur place ou qui erre
    /// vers ce qu'elle n'a pas vu (deux cases par tour au plus) ; puis la même chose depuis des
    /// départs quelconques (non équitables), pour comparer.
    #[test]
    #[ignore]
    fn bilan_nomade() {
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map_or(vec![777, 2, 5, 4242], |s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
        let simuler = |map: &WorldMap, depart: Hex, errer: bool| -> (u32, u64) {
            let mut n = Nomadisme::default();
            let mut at = depart;
            let mut membres = MEMBRES_BANDE;
            for t in 1..=80 {
                if errer {
                    let savoir = n.savoir(Peuple::JOUEUR);
                    let valeur = |c: Hex| {
                        let nouvelles = (sortes_autour(map, c, RAYON_OBSERVATION) & !savoir.observees).count_ones() as f64;
                        let proches = sortes_autour(map, c, 1).count_ones().min(SORTES_PROCHES_MAX) as f64;
                        nouvelles * 10.0 + proches + nourriture_bande(map, c, membres, &n).min(3.0)
                    };
                    if let Some(c) = map.range(at, 2).filter(|&c| map.get(c).is_some_and(|t| t.move_cost().is_some()) && !map.is_fleuve(c)).max_by(|a, b| valeur(*a).total_cmp(&valeur(*b))) {
                        at = c;
                    }
                }
                let nourriture = nourriture_bande(map, at, membres, &n);
                membres = membres.saturating_add_signed(variation_bande(membres, nourriture));
                let vues = sortes_autour(map, at, RAYON_OBSERVATION);
                let proches = sortes_autour(map, at, 1);
                let savoir = n.savoir_mut(Peuple::JOUEUR);
                let a = savoir.apport(vues, proches, 0);
                let fini = savoir.avancer(&a, vues);
                n.epuiser(&cases_cueillies(map, at));
                if fini {
                    return (t, membres);
                }
            }
            (99, membres)
        };
        for seed in seeds {
            // (la planète du jeu : 192 × 111 cases de 40 km)
            let map = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let start = map.find_start().unwrap();
            let autour = sortes_autour(&map, start, RAYON_DEPART);
            let noms: Vec<&str> = sortes_de(autour).iter().map(|s| s.nom()).collect();
            let (sur_place, m1) = simuler(&map, start, false);
            let (errant, m2) = simuler(&map, start, true);
            println!(
                "graine {seed} : départ {:?} {} ; {} sortes à {RAYON_DEPART} cases ({}) ; cueillette {:.1} ; Agriculture : {sur_place} tours sur place ({m1} membres), {errant} en errant ({m2})",
                start.to_offset(),
                map.describe(start),
                autour.count_ones(),
                noms.join(", "),
                nourriture_bande(&map, start, MEMBRES_BANDE, &Nomadisme::default())
            );
            // Départs quelconques : une case de terre sur 400, en errant
            let mut tours: Vec<u32> = map
                .hexes()
                .step_by(400)
                .filter(|&h| map.get(h).is_some_and(|t| !t.is_water() && t.move_cost().is_some()) && !map.is_fleuve(h) && !map.is_ice(h))
                .map(|h| simuler(&map, h, true).0)
                .collect();
            tours.sort();
            if !tours.is_empty() {
                let q = |p: f64| tours[((tours.len() - 1) as f64 * p).round() as usize];
                println!("  départs quelconques ({}) en errant : min {}, quart {}, médiane {}, trois quarts {}, max {}", tours.len(), tours[0], q(0.25), q(0.5), q(0.75), tours[tours.len() - 1]);
            }
        }
    }
}
