//! Travailleurs sur les cases d'une ville : aménagements, capacités, rendements décroissants et
//! répartition par l'intendant (logique pure, testée). Voir `docs/conception/conception-villes.md`,
//! « Étape 4 de la feuille de route ».
//!
//! - La moitié des habitants travaille (`PART_ACTIVE`).
//! - Chaque case a une capacité en travailleurs, selon son aménagement ; son rendement croît avec
//!   les travailleurs, de moins en moins vite : maximum × (travailleurs / capacité)^0,7.
//! - L'intendant place les travailleurs par blocs là où le gain est le plus fort ; le poids d'une
//!   ressource grandit avec son manque, et avec la priorité donnée par le joueur.

use crate::hex::Hex;
use crate::map::{Relief, Terrain, WorldMap, Yields};

/// Part des habitants qui travaille.
pub const PART_ACTIVE: f64 = 0.5;
/// Travailleurs d'une case sans aménagement (cueillette, chasse).
pub const CAPACITE_SANS_AMENAGEMENT: u32 = 500;
/// Exposant des rendements décroissants (la moitié de la capacité donne ≈ 62 % du maximum).
pub const EXPOSANT: f64 = 0.7;
/// Plus petit bloc de travailleurs placé d'un coup.
pub const BLOC: u32 = 100;
/// Pluie annuelle (mm) au-dessous de laquelle un champ demande de l'eau douce voisine.
pub const PLUIE_CHAMP_MM: f32 = 400.0;
/// Surcoût d'un champ ou d'un pâturage pris sur la forêt (défrichement).
pub const COUT_DEFRICHEMENT: i32 = 10;
/// Pénalité par case d'éloignement du centre (au-delà de la première couronne).
const PENALITE_DISTANCE: f64 = 0.05;

/// Aménagement rural d'une case, choisi par le joueur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Amenagement {
    Champ,
    Paturage,
    CoupeDeBois,
}

impl Amenagement {
    pub const ALL: [Amenagement; 3] = [Amenagement::Champ, Amenagement::Paturage, Amenagement::CoupeDeBois];

    /// Nom affiché.
    pub fn nom(self) -> &'static str {
        match self {
            Amenagement::Champ => "champ",
            Amenagement::Paturage => "pâturage",
            Amenagement::CoupeDeBois => "coupe de bois",
        }
    }

    /// Nom court pour la console (sans accent ni espace).
    pub fn code(self) -> &'static str {
        match self {
            Amenagement::Champ => "champ",
            Amenagement::Paturage => "paturage",
            Amenagement::CoupeDeBois => "bois",
        }
    }

    /// Travailleurs au plus sur la case aménagée.
    pub fn capacite(self) -> u32 {
        match self {
            Amenagement::Champ => 1_500,
            Amenagement::Paturage | Amenagement::CoupeDeBois => 1_000,
        }
    }

    /// Coût en production sur une case sans forêt.
    pub fn cout_de_base(self) -> i32 {
        match self {
            Amenagement::Champ => 20,
            Amenagement::Paturage | Amenagement::CoupeDeBois => 15,
        }
    }

    /// Coût en production sur cette case (défrichement compris).
    pub fn cout(self, map: &WorldMap, h: Hex) -> i32 {
        self.cout_de_base() + if self != Amenagement::CoupeDeBois && defriche(map, h) { COUT_DEFRICHEMENT } else { 0 }
    }

    /// L'aménagement est-il possible sur la case ? Sinon, la raison.
    pub fn possible(self, map: &WorldMap, h: Hex) -> Result<(), &'static str> {
        let Some(t) = map.get(h) else { return Err("hors de la carte") };
        if t.is_water() || map.is_fleuve(h) || map.is_ice(h) {
            return Err("pas sur l'eau ni sur la glace");
        }
        let relief = map.relief(h);
        if relief >= Relief::Mountain {
            return Err("pas en montagne");
        }
        match self {
            Amenagement::Champ => {
                if relief > Relief::Hills {
                    return Err("relief trop accidenté pour un champ");
                }
                if matches!(t, Terrain::Desert | Terrain::Snow | Terrain::Tundra | Terrain::Marsh) {
                    return Err("sol impropre aux cultures");
                }
                if !map.fresh_water(h) && map.rain(h).is_some_and(|r| r < PLUIE_CHAMP_MM) {
                    return Err("trop sec : il faut de la pluie ou une rivière voisine");
                }
                Ok(())
            }
            Amenagement::Paturage => {
                if matches!(t, Terrain::Desert | Terrain::Snow | Terrain::Marsh) {
                    return Err("pas d'herbe pour les bêtes");
                }
                Ok(())
            }
            Amenagement::CoupeDeBois => {
                if t.is_wooded() || t == Terrain::WoodedSavanna {
                    Ok(())
                } else {
                    Err("pas de forêt")
                }
            }
        }
    }

    /// Aménagements possibles sur la case.
    pub fn possibles(map: &WorldMap, h: Hex) -> Vec<Amenagement> {
        Amenagement::ALL.into_iter().filter(|a| a.possible(map, h).is_ok()).collect()
    }

    /// Code de la console → aménagement.
    pub fn depuis_code(code: &str) -> Option<Amenagement> {
        Amenagement::ALL.into_iter().find(|a| a.code() == code)
    }
}

/// La case est-elle boisée (un champ ou un pâturage y demande un défrichement) ?
fn defriche(map: &WorldMap, h: Hex) -> bool {
    map.get(h).is_some_and(|t| t.is_wooded() || t == Terrain::WoodedSavanna)
}

/// Rendements d'une ville ou d'une case, en fractions (les travailleurs en donnent des parts).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rendements {
    pub nourriture: f64,
    pub production: f64,
    #[serde(alias = "economie")]
    pub deniers: f64,
}

impl Rendements {
    pub const fn new(nourriture: f64, production: f64, deniers: f64) -> Self {
        Rendements { nourriture, production, deniers }
    }

    /// Les trois valeurs dans l'ordre nourriture, production, deniers.
    pub fn valeurs(self) -> [f64; 3] {
        [self.nourriture, self.production, self.deniers]
    }

    pub fn fois(self, k: f64) -> Rendements {
        Rendements::new(self.nourriture * k, self.production * k, self.deniers * k)
    }
}

impl From<Yields> for Rendements {
    fn from(y: Yields) -> Self {
        Rendements::new(y.food as f64, y.production as f64, y.deniers as f64)
    }
}

impl std::ops::Add for Rendements {
    type Output = Rendements;
    fn add(self, o: Rendements) -> Rendements {
        Rendements::new(self.nourriture + o.nourriture, self.production + o.production, self.deniers + o.deniers)
    }
}

impl std::ops::AddAssign for Rendements {
    fn add_assign(&mut self, o: Rendements) {
        *self = *self + o;
    }
}

impl std::ops::Sub for Rendements {
    type Output = Rendements;
    fn sub(self, o: Rendements) -> Rendements {
        Rendements::new(self.nourriture - o.nourriture, self.production - o.production, self.deniers - o.deniers)
    }
}

/// Capacité de la case en travailleurs.
pub fn capacite(amenagement: Option<Amenagement>) -> u32 {
    amenagement.map_or(CAPACITE_SANS_AMENAGEMENT, Amenagement::capacite)
}

/// Rendement de la case à pleine capacité.
pub fn rendement_max(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> Rendements {
    let mut y = map.yields(h).unwrap_or_default();
    // (champ ou pâturage pris sur la forêt : le bois est perdu, la terre devient une prairie)
    if matches!(amenagement, Some(Amenagement::Champ | Amenagement::Paturage)) && defriche(map, h) {
        y = Yields::new(y.food + 1, (y.production - 2).max(0), y.deniers);
    }
    let r = Rendements::from(y);
    match amenagement {
        None => r,
        Some(Amenagement::Champ) => Rendements::new(r.nourriture * 2.0, r.production, r.deniers),
        Some(Amenagement::Paturage) => Rendements::new(r.nourriture * 1.5, r.production + 1.0, r.deniers),
        Some(Amenagement::CoupeDeBois) => Rendements::new(r.nourriture, r.production * 2.0, r.deniers),
    }
}

/// Rendement de `travailleurs` sur une case (rendements décroissants jusqu'à la capacité).
pub fn rendement(max: Rendements, travailleurs: u32, capacite: u32) -> Rendements {
    if travailleurs == 0 || capacite == 0 {
        return Rendements::default();
    }
    let part = (travailleurs as f64 / capacite as f64).min(1.0);
    max.fois(part.powf(EXPOSANT))
}

/// Consigne du joueur pour une ressource.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Priorite {
    #[default]
    Normale,
    Prioritaire,
    Maximale,
}

impl Priorite {
    pub const ALL: [Priorite; 3] = [Priorite::Normale, Priorite::Prioritaire, Priorite::Maximale];

    fn facteur(self) -> f64 {
        match self {
            Priorite::Normale => 1.0,
            Priorite::Prioritaire => 2.0,
            Priorite::Maximale => 4.0,
        }
    }

    /// Nom affiché (et code de la console).
    pub fn nom(self) -> &'static str {
        match self {
            Priorite::Normale => "normale",
            Priorite::Prioritaire => "prioritaire",
            Priorite::Maximale => "maximale",
        }
    }
}

/// Noms des trois ressources dans l'ordre des priorités.
pub const RESSOURCES: [&str; 3] = ["nourriture", "production", "deniers"];

/// Consignes du joueur à l'intendant d'une ville.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Consignes {
    /// Priorité de la nourriture, de la production et des deniers.
    pub priorites: [Priorite; 3],
    /// Cases forcées et leur effectif (en travailleurs).
    pub forcees: Vec<(Hex, u32)>,
}

/// Case que la ville peut exploiter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaseDispo {
    pub hex: Hex,
    pub max: Rendements,
    pub capacite: u32,
    /// Distance au centre de la ville (en cases).
    pub distance: i32,
}

/// Travailleurs placés par l'intendant.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Repartition {
    /// Cases exploitées et leurs travailleurs (dans l'ordre des cases données).
    pub cases: Vec<(Hex, u32)>,
    pub actifs: u32,
    /// Actifs sans case (toutes les capacités sont pleines).
    pub chomeurs: u32,
    /// Rendement total de la ville (centre compris).
    pub rendements: Rendements,
}

impl Repartition {
    /// Travailleurs sur la case.
    pub fn sur(&self, h: Hex) -> u32 {
        self.cases.iter().find(|(c, _)| *c == h).map_or(0, |(_, n)| *n)
    }
}

/// Actifs d'une ville de `habitants` habitants.
pub fn actifs(habitants: u64) -> u32 {
    (habitants as f64 * PART_ACTIVE).round() as u32
}

/// Poids des trois ressources selon le manque de nourriture (couverture du besoin) et les priorités.
fn poids(couverture: f64, consignes: &Consignes) -> [f64; 3] {
    // (tant que la ville ne se nourrit pas, la nourriture pèse lourd ; puis pour croître ; puis peu)
    let nourriture = if couverture < 1.0 {
        4.0
    } else if couverture < 1.0 + crate::rules::SURPLUS_POUR_CROITRE {
        2.0
    } else {
        0.3
    };
    let base = [nourriture, 1.0, 0.6];
    std::array::from_fn(|i| base[i] * consignes.priorites[i].facteur())
}

/// Répartit les `actifs` sur les cases : d'abord les cases forcées, puis par blocs là où le gain
/// pondéré est le plus fort. `centre` : rendement du centre de la ville (sans travailleurs).
pub fn repartir(cases: &[CaseDispo], actifs: u32, besoin_nourriture: f64, centre: Rendements, consignes: &Consignes) -> Repartition {
    let mut n = vec![0u32; cases.len()];
    let mut libres = actifs;
    let mut forcee = vec![false; cases.len()];
    for &(h, k) in &consignes.forcees {
        if let Some(i) = cases.iter().position(|c| c.hex == h) {
            let k = k.min(cases[i].capacite).min(libres);
            n[i] = k;
            libres -= k;
            forcee[i] = true;
        }
    }
    let total = |n: &[u32]| cases.iter().zip(n).fold(centre, |acc, (c, &k)| acc + rendement(c.max, k, c.capacite));
    let mut courant = total(&n);
    // (blocs plus gros dans les grandes villes : le calcul reste léger)
    let bloc = BLOC.max(actifs / 200 / BLOC * BLOC);
    while libres > 0 {
        let b = bloc.min(libres);
        let w = poids(courant.nourriture / besoin_nourriture.max(1e-6), consignes);
        let mut meilleur: Option<(usize, f64)> = None;
        for (i, c) in cases.iter().enumerate() {
            if forcee[i] || n[i] >= c.capacite {
                continue;
            }
            let k = (n[i] + b).min(c.capacite);
            let gain = rendement(c.max, k, c.capacite) - rendement(c.max, n[i], c.capacite);
            let eloignement = (1.0 - PENALITE_DISTANCE * (c.distance - 1).max(0) as f64).max(0.5);
            // (gain par travailleur : un bloc tronqué par la capacité n'est pas avantagé)
            let score = gain.valeurs().iter().zip(w).map(|(g, w)| g * w).sum::<f64>() * eloignement / (k - n[i]) as f64;
            if score > 1e-12 && meilleur.is_none_or(|(_, s)| score > s) {
                meilleur = Some((i, score));
            }
        }
        let Some((i, _)) = meilleur else { break };
        let k = (n[i] + b).min(cases[i].capacite);
        courant = courant - rendement(cases[i].max, n[i], cases[i].capacite) + rendement(cases[i].max, k, cases[i].capacite);
        libres -= k - n[i];
        n[i] = k;
    }
    Repartition {
        cases: cases.iter().zip(&n).filter(|(_, k)| **k > 0).map(|(c, k)| (c.hex, *k)).collect(),
        actifs,
        chomeurs: libres,
        rendements: total(&n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(col: i32, max: Rendements, capacite: u32) -> CaseDispo {
        CaseDispo { hex: Hex::from_offset(col, 0), max, capacite, distance: 1 }
    }

    #[test]
    fn rendements_decroissants() {
        let max = Rendements::new(2.0, 0.0, 0.0);
        assert_eq!(rendement(max, 0, 500), Rendements::default());
        assert_eq!(rendement(max, 500, 500), max);
        assert_eq!(rendement(max, 900, 500), max, "pas au-delà de la capacité");
        let moitie = rendement(max, 250, 500).nourriture;
        assert!((moitie - 2.0 * 0.5f64.powf(0.7)).abs() < 1e-9 && moitie > 1.2 && moitie < 1.3, "{moitie}");
    }

    #[test]
    fn la_nourriture_d_abord_puis_la_production() {
        let pre = case(1, Rendements::new(2.0, 0.0, 0.0), 500);
        let bois = case(2, Rendements::new(1.0, 2.0, 0.0), 500);
        // Ville affamée : surtout au pré (la forêt nourrit aussi un peu : rendements décroissants)
        let r = repartir(&[pre, bois], 300, 3.0, Rendements::default(), &Consignes::default());
        assert!(r.sur(pre.hex) > r.sur(bois.hex), "{r:?}");
        let r = repartir(&[pre, bois], 100, 3.0, Rendements::default(), &Consignes::default());
        assert_eq!(r.sur(pre.hex), 100, "le premier bloc au pré");
        // Nourriture largement couverte par le centre : le bois passe devant
        let r = repartir(&[pre, bois], 300, 1.0, Rendements::new(5.0, 0.0, 0.0), &Consignes::default());
        assert_eq!(r.sur(bois.hex), 300);
        // Tous placés, sans chômeurs, tant qu'il y a de la place
        let r = repartir(&[pre, bois], 1000, 3.0, Rendements::default(), &Consignes::default());
        assert_eq!((r.sur(pre.hex), r.sur(bois.hex), r.chomeurs), (500, 500, 0));
    }

    #[test]
    fn chomeurs_au_dela_des_capacites() {
        let pre = case(1, Rendements::new(2.0, 0.0, 0.0), 500);
        let r = repartir(&[pre], 1234, 3.0, Rendements::default(), &Consignes::default());
        assert_eq!((r.sur(pre.hex), r.chomeurs, r.actifs), (500, 734, 1234));
        assert_eq!(r.rendements.nourriture, 2.0);
    }

    #[test]
    fn consignes_du_joueur() {
        let pre = case(1, Rendements::new(2.0, 0.0, 0.0), 500);
        let bois = case(2, Rendements::new(0.0, 2.0, 0.0), 500);
        // Priorité maximale à la production : le bois passe devant une ville qui veut croître
        let mut consignes = Consignes::default();
        consignes.priorites[1] = Priorite::Maximale;
        let r = repartir(&[pre, bois], 300, 1.0, Rendements::new(1.2, 0.0, 0.0), &consignes);
        assert_eq!(r.sur(bois.hex), 300);
        // Case forcée : son effectif d'abord, et l'intendant n'y touche plus
        let consignes = Consignes { forcees: vec![(bois.hex, 200)], ..Default::default() };
        let r = repartir(&[pre, bois], 600, 3.0, Rendements::default(), &consignes);
        assert_eq!((r.sur(bois.hex), r.sur(pre.hex), r.chomeurs), (200, 400, 0));
    }

    #[test]
    fn amenagements_selon_la_case() {
        let mut map = WorldMap::filled(6, 6, Terrain::Grassland);
        let h = Hex::from_offset(2, 2);
        assert_eq!(Amenagement::possibles(&map, h), vec![Amenagement::Champ, Amenagement::Paturage]);
        let champ = rendement_max(&map, h, Some(Amenagement::Champ));
        assert_eq!(champ.nourriture, 2.0 * rendement_max(&map, h, None).nourriture);
        map.set(h, Terrain::Forest);
        assert_eq!(Amenagement::possibles(&map, h), Amenagement::ALL.to_vec(), "forêt : bois, ou défrichement");
        assert_eq!(Amenagement::Champ.cout(&map, h), 30);
        assert_eq!(Amenagement::CoupeDeBois.cout(&map, h), 15);
        assert_eq!(rendement_max(&map, h, Some(Amenagement::CoupeDeBois)).production, 4.0);
        assert_eq!(rendement_max(&map, h, Some(Amenagement::Champ)), Rendements::new(4.0, 0.0, 0.0), "défriché : une prairie");
        map.set(h, Terrain::Desert);
        assert!(Amenagement::possibles(&map, h).is_empty());
        map.set(h, Terrain::Grassland);
        map.set_relief(h, Relief::Rugged);
        assert_eq!(Amenagement::possibles(&map, h), vec![Amenagement::Paturage]);
    }
}
