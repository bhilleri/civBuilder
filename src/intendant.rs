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
/// Travailleurs d'une case sans aménagement (cueillette, chasse) : peu de places, mais chacun y
/// rapporte plus que dans un champ (Bowles 2011 ; décision de Benjamin du 13/11/2026).
pub const CAPACITE_SANS_AMENAGEMENT: u32 = 300;
/// Nourriture d'un champ plein, rapportée à celle que la case tire de son terrain : 3 000
/// travailleurs à 1,25 × le minimum vital contre 300 à 1,67 × pour la cueillette (une case
/// cultivée porte dix fois plus d'habitants, chacun y rapporte un quart de moins).
pub const FACTEUR_CHAMP: f64 = 7.5;
/// Exposant des rendements décroissants de la cueillette et de la chasse : les ressources sauvages
/// s'épuisent vite (la moitié de la capacité donne ≈ 62 % du maximum).
pub const EXPOSANT: f64 = 0.7;
/// Exposant des aménagements : chaque travailleur de plus étend le travail (une parcelle, des arbres
/// de plus), le rendement baisse lentement (la moitié de la capacité donne ≈ 55 % du maximum ; décision
/// de Benjamin du 13/11/2026).
pub const EXPOSANT_AMENAGEMENT: f64 = 0.85;
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
    /// Champ de légumineuses (lentilles, pois, fèves) : des protéines sans élevage, au rendement
    /// réduit (80 % d'un champ de céréales).
    Legumineuses,
    /// Maraîchage (ex-jardin) : fruits et légumes, peu de places (les sauvegardes d'avant disent « Jardin »).
    #[serde(alias = "Jardin")]
    Maraichage,
    /// Exploitation d'un gisement connu (saline, minière, carrière, coupe de bois d'œuvre…) : une
    /// unité de sa ressource par tour à pleine capacité, un peu de production (décision D2).
    Exploitation,
}

impl Amenagement {
    pub const ALL: [Amenagement; 6] = [Amenagement::Champ, Amenagement::Legumineuses, Amenagement::Paturage, Amenagement::Maraichage, Amenagement::CoupeDeBois, Amenagement::Exploitation];

    /// Nom affiché.
    pub fn nom(self) -> &'static str {
        match self {
            Amenagement::Champ => "champ",
            Amenagement::Paturage => "pâturage",
            Amenagement::CoupeDeBois => "coupe de bois",
            Amenagement::Legumineuses => "champ de légumineuses",
            Amenagement::Maraichage => "maraîchage",
            Amenagement::Exploitation => "exploitation",
        }
    }

    /// Nom court pour la console (sans accent ni espace).
    pub fn code(self) -> &'static str {
        match self {
            Amenagement::Champ => "champ",
            Amenagement::Paturage => "paturage",
            Amenagement::CoupeDeBois => "bois",
            Amenagement::Legumineuses => "legumineuses",
            Amenagement::Maraichage => "maraichage",
            Amenagement::Exploitation => "exploitation",
        }
    }

    /// Travailleurs au plus sur la case aménagée.
    pub fn capacite(self) -> u32 {
        match self {
            Amenagement::Champ | Amenagement::Legumineuses => 3_000,
            Amenagement::Paturage => 1_200,
            Amenagement::CoupeDeBois => 600,
            Amenagement::Maraichage => 600,
            Amenagement::Exploitation => 500,
        }
    }

    /// Coût en production sur une case sans forêt.
    pub fn cout_de_base(self) -> i32 {
        match self {
            Amenagement::Champ | Amenagement::Legumineuses => 20,
            Amenagement::Paturage | Amenagement::CoupeDeBois => 15,
            Amenagement::Maraichage => 10,
            Amenagement::Exploitation => 20,
        }
    }

    /// Coût en production sur cette case (défrichement compris).
    pub fn cout(self, map: &WorldMap, h: Hex) -> i32 {
        // (une coupe ou une exploitation ne défriche pas : la forêt reste)
        self.cout_de_base() + if !matches!(self, Amenagement::CoupeDeBois | Amenagement::Exploitation) && defriche(map, h) { COUT_DEFRICHEMENT } else { 0 }
    }

    /// L'aménagement est-il possible sur la case ? Sinon, la raison.
    pub fn possible(self, map: &WorldMap, h: Hex) -> Result<(), &'static str> {
        let Some(t) = map.get(h) else { return Err("hors de la carte") };
        if t.is_water() || map.is_fleuve(h) || map.is_ice(h) {
            return Err("pas sur l'eau ni sur la glace");
        }
        let relief = map.relief(h);
        // (une mine ou une carrière peut être en montagne, pas sur un pic)
        if self == Amenagement::Exploitation {
            return if relief == Relief::Peak {
                Err("pas sur un pic")
            } else if map.gisement(h).is_none() {
                Err("aucun gisement")
            } else {
                Ok(())
            };
        }
        if relief >= Relief::Mountain {
            return Err("pas en montagne");
        }
        match self {
            Amenagement::Champ | Amenagement::Legumineuses | Amenagement::Maraichage => {
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
            Amenagement::Exploitation => Ok(()),
        }
    }

    /// Aménagements possibles sur la case.
    pub fn possibles(map: &WorldMap, h: Hex) -> Vec<Amenagement> {
        Amenagement::ALL.into_iter().filter(|a| a.possible(map, h).is_ok()).collect()
    }

    /// Code de la console → aménagement.
    pub fn depuis_code(code: &str) -> Option<Amenagement> {
        // (« jardin » : l'ancien nom du maraîchage)
        let code = if code == "jardin" { "maraichage" } else { code };
        Amenagement::ALL.into_iter().find(|a| a.code() == code)
    }
}

/// La case est-elle boisée (un champ ou un pâturage y demande un défrichement) ?
fn defriche(map: &WorldMap, h: Hex) -> bool {
    map.get(h).is_some_and(|t| t.is_wooded() || t == Terrain::WoodedSavanna)
}

/// Production d'une exploitation pleine, en plus de celle de la case.
pub const PRODUCTION_EXPLOITATION: f64 = 1.0;

/// Unités de ressource que tirent `travailleurs` d'une exploitation (rendements décroissants
/// comme les autres cases, `UNITES_PAR_TOUR` à pleine capacité).
pub fn unites_extraites(travailleurs: u32, capacite: u32) -> f64 {
    crate::gisements::UNITES_PAR_TOUR * part(travailleurs, capacite, EXPOSANT_AMENAGEMENT)
}

/// Rendement d'un champ de légumineuses, rapporté à un champ de céréales (80 % : ses travailleurs
/// et leurs familles en vivent tout juste ; décision de Benjamin du 13/11/2026).
pub const RENDEMENT_LEGUMINEUSES: f64 = 0.8;
/// Fruits et légumes d'un maraîchage plein, rapportés à la nourriture de la case (600 places : 0,003
/// par maraîcher, moins que la cueillette, et autant qu'elle avec 300 travailleurs).
pub const RENDEMENT_MARAICHAGE: f64 = 1.8;
/// Nourriture d'un pâturage plein, rapportée à la case.
pub const FACTEUR_PATURAGE: f64 = 3.0;
/// Travailleurs au-delà desquels la production d'un pâturage n'augmente plus.
pub const CAPACITE_PRODUCTION_PATURAGE: u32 = 800;
/// Production d'une coupe de bois pleine, rapportée à la forêt (600 bûcherons : 0,0083 chacun contre
/// 0,0067 pour un cueilleur ; une coupe ne prend qu'une partie de la forêt).
pub const FACTEUR_COUPE: f64 = 2.5;

/// Types de nourriture (livraison 5c) : l'équilibre entre eux relève la limite de la ville.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeNourriture {
    Cereales,
    Proteines,
    FruitsLegumes,
}

impl TypeNourriture {
    pub const TOUS: [TypeNourriture; 3] = [TypeNourriture::Cereales, TypeNourriture::Proteines, TypeNourriture::FruitsLegumes];

    pub const fn nom(self) -> &'static str {
        match self {
            TypeNourriture::Cereales => "céréales et féculents",
            TypeNourriture::Proteines => "protéines",
            TypeNourriture::FruitsLegumes => "fruits et légumes",
        }
    }
}

/// Part de chaque type dans la nourriture d'une case, dans l'ordre de `TypeNourriture::TOUS`.
pub type Parts = [f64; 3];

/// Le centre de la ville : ses champs, ses bêtes et ses jardins proches.
pub const PARTS_CENTRE: Parts = [0.5, 0.25, 0.25];

/// Part de chaque type dans la nourriture de la case : selon l'aménagement, sinon selon le biome
/// (cueillette, chasse ; l'eau donne du poisson).
pub fn parts_de_la_case(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> Parts {
    // Une case aménagée : la nourriture de son travail, et celle de la subsistance (la nature), au prorata
    let Some(a) = amenagement.filter(|a| *a != Amenagement::CoupeDeBois) else { return parts_propres(map, h, amenagement) };
    let c = courbe(map, h, Some(a));
    let (t, s) = (c.travail.nourriture, c.subsistance.nourriture);
    if t + s <= 0.0 {
        return parts_propres(map, h, Some(a));
    }
    let (pa, pn) = (parts_propres(map, h, Some(a)), parts_propres(map, h, None));
    std::array::from_fn(|i| (pa[i] * t + pn[i] * s) / (t + s))
}

/// Part de chaque type dans la nourriture du travail de la case (ou de sa cueillette).
fn parts_propres(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> Parts {
    match amenagement {
        Some(Amenagement::Champ) => [1.0, 0.0, 0.0],
        Some(Amenagement::Legumineuses | Amenagement::Paturage) => [0.0, 1.0, 0.0],
        Some(Amenagement::Maraichage) => [0.0, 0.0, 1.0],
        // (pas de nourriture)
        Some(Amenagement::Exploitation) => [0.0, 0.0, 0.0],
        Some(Amenagement::CoupeDeBois) | None => {
            if map.is_fleuve(h) {
                return [0.0, 1.0, 0.0];
            }
            match map.get(h) {
                Some(t) if t.is_water() => [0.0, 1.0, 0.0],
                Some(Terrain::Forest | Terrain::Taiga | Terrain::Jungle | Terrain::WoodedSavanna) => [0.0, 0.5, 0.5],
                Some(Terrain::Mediterranean) => [0.2, 0.3, 0.5],
                Some(Terrain::Grassland | Terrain::Plains | Terrain::Savanna | Terrain::Steppe) => [0.4, 0.6, 0.0],
                Some(Terrain::Marsh) => [0.3, 0.7, 0.0],
                Some(Terrain::Oasis) => [0.0, 0.5, 0.5],
                _ => [0.0, 1.0, 0.0],
            }
        }
    }
}

/// Sortes de nourriture (livraison 5d) : leur diversité au sein d'un type donne du bien-être. Une
/// case n'en présente qu'une par type, la plus adaptée (décision de Benjamin). Les sortes de l'âge
/// du bronze (seigle et avoine, fruits du verger, vigne et olivier) et le maïs (un seul foyer
/// d'origine) viendront avec les ères et le commerce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sorte {
    Ble,
    Orge,
    MilletSorgho,
    Riz,
    Tubercules,
    Chataignes,
    Legumineuses,
    Boeufs,
    MoutonsChevres,
    Porcs,
    BetesExtremes,
    Gibier,
    Poisson,
    Baies,
    Legumes,
    FruitsTropicaux,
    Dattes,
}

/// Nombre de sortes.
pub const N_SORTES: usize = 17;
/// Nourriture de la ville par sorte, dans l'ordre de `Sorte::TOUTES`.
pub type ParSorte = [f64; N_SORTES];

impl Sorte {
    pub const TOUTES: [Sorte; N_SORTES] = [
        Sorte::Ble,
        Sorte::Orge,
        Sorte::MilletSorgho,
        Sorte::Riz,
        Sorte::Tubercules,
        Sorte::Chataignes,
        Sorte::Legumineuses,
        Sorte::Boeufs,
        Sorte::MoutonsChevres,
        Sorte::Porcs,
        Sorte::BetesExtremes,
        Sorte::Gibier,
        Sorte::Poisson,
        Sorte::Baies,
        Sorte::Legumes,
        Sorte::FruitsTropicaux,
        Sorte::Dattes,
    ];

    pub const fn nom(self) -> &'static str {
        match self {
            Sorte::Ble => "blé",
            Sorte::Orge => "orge",
            Sorte::MilletSorgho => "millet et sorgho",
            Sorte::Riz => "riz",
            Sorte::Tubercules => "tubercules",
            Sorte::Chataignes => "châtaignes et glands",
            Sorte::Legumineuses => "légumineuses",
            Sorte::Boeufs => "bœufs",
            Sorte::MoutonsChevres => "moutons et chèvres",
            Sorte::Porcs => "porcs",
            Sorte::BetesExtremes => "rennes, lamas et chameaux",
            Sorte::Gibier => "gibier",
            Sorte::Poisson => "poisson",
            Sorte::Baies => "baies et champignons",
            Sorte::Legumes => "légumes",
            Sorte::FruitsTropicaux => "fruits tropicaux",
            Sorte::Dattes => "dattes",
        }
    }

    /// Perte par tour de la sorte gardée au grenier (décision G4) : grains, légumineuses,
    /// châtaignes et dattes 2 % ; tubercules, fruits et légumes 10 % ; viande et poisson 20 % (5 %
    /// salés, quand le sel existera).
    pub const fn perte(self) -> f64 {
        match self {
            Sorte::Ble | Sorte::Orge | Sorte::MilletSorgho | Sorte::Riz | Sorte::Chataignes | Sorte::Legumineuses | Sorte::Dattes => 0.02,
            Sorte::Tubercules | Sorte::Baies | Sorte::Legumes | Sorte::FruitsTropicaux => 0.10,
            Sorte::Boeufs | Sorte::MoutonsChevres | Sorte::Porcs | Sorte::BetesExtremes | Sorte::Gibier | Sorte::Poisson => 0.20,
        }
    }

    /// Type de la sorte (indice dans `TypeNourriture::TOUS`).
    pub const fn type_(self) -> usize {
        match self {
            Sorte::Ble | Sorte::Orge | Sorte::MilletSorgho | Sorte::Riz | Sorte::Tubercules | Sorte::Chataignes => 0,
            Sorte::Legumineuses | Sorte::Boeufs | Sorte::MoutonsChevres | Sorte::Porcs | Sorte::BetesExtremes | Sorte::Gibier | Sorte::Poisson => 1,
            Sorte::Baies | Sorte::Legumes | Sorte::FruitsTropicaux | Sorte::Dattes => 2,
        }
    }

    pub fn index(self) -> usize {
        Sorte::TOUTES.iter().position(|&s| s == self).unwrap_or(0)
    }
}

/// Sorte de chaque type que la case présente (la plus adaptée à son climat et à son biome), selon
/// son aménagement.
pub fn sortes_de_la_case(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> [Option<Sorte>; 3] {
    // Une case aménagée : les sortes de son travail, et pour les autres types celles de la nature (la subsistance)
    let propres = sortes_propres(map, h, amenagement);
    match amenagement {
        Some(_) => {
            let nature = sortes_propres(map, h, None);
            let parts = parts_de_la_case(map, h, amenagement);
            std::array::from_fn(|i| propres[i].or(nature[i].filter(|_| parts[i] > 0.0)))
        }
        None => propres,
    }
}

/// Sortes du travail de la case (ou de sa cueillette).
fn sortes_propres(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> [Option<Sorte>; 3] {
    let t = map.get(h);
    let temperature = map.temperature(h).unwrap_or(12.0);
    let pluie = map.rain(h).unwrap_or(700.0);
    let cereale = || match t {
        Some(Terrain::Jungle | Terrain::WoodedSavanna) => Sorte::Tubercules,
        // Désert irrigué : blé et orge (Égypte), millet et sorgho au plus chaud (Soudan)
        Some(Terrain::Riverbank | Terrain::Oasis) if temperature <= 24.0 => Sorte::Ble,
        Some(Terrain::Riverbank | Terrain::Oasis) => Sorte::MilletSorgho,
        _ if temperature > 20.0 && pluie > 1_000.0 => Sorte::Riz,
        Some(Terrain::Savanna | Terrain::Steppe) => Sorte::MilletSorgho,
        _ if pluie < 600.0 && temperature > 15.0 => Sorte::MilletSorgho,
        Some(Terrain::Mediterranean) => Sorte::Ble,
        _ if (8.0..=20.0).contains(&temperature) && (400.0..=1_000.0).contains(&pluie) => Sorte::Ble,
        _ => Sorte::Orge,
    };
    let betail = || match t {
        Some(Terrain::Taiga | Terrain::Tundra) => Sorte::BetesExtremes,
        _ if map.elevation(h).unwrap_or(0.0) > 2_500.0 => Sorte::BetesExtremes,
        Some(Terrain::Desert | Terrain::Oasis) => Sorte::BetesExtremes,
        Some(Terrain::Forest) => Sorte::Porcs,
        Some(Terrain::Steppe | Terrain::Mediterranean) => Sorte::MoutonsChevres,
        _ if map.relief(h) >= Relief::Hills => Sorte::MoutonsChevres,
        _ => Sorte::Boeufs,
    };
    let jardin = || if temperature > 20.0 && pluie < 400.0 { Sorte::Dattes } else { Sorte::Legumes };
    match amenagement {
        Some(Amenagement::Champ) => [Some(cereale()), None, None],
        Some(Amenagement::Legumineuses) => [None, Some(Sorte::Legumineuses), None],
        Some(Amenagement::Paturage) => [None, Some(betail()), None],
        Some(Amenagement::Maraichage) => [None, None, Some(jardin())],
        Some(Amenagement::Exploitation) => [None, None, None],
        Some(Amenagement::CoupeDeBois) | None => {
            if map.is_fleuve(h) || t.is_some_and(|t| t.is_water()) {
                return [None, Some(Sorte::Poisson), None];
            }
            match t {
                Some(Terrain::Forest | Terrain::Taiga) => [None, Some(Sorte::Gibier), Some(Sorte::Baies)],
                Some(Terrain::Jungle | Terrain::WoodedSavanna) => [None, Some(Sorte::Gibier), Some(Sorte::FruitsTropicaux)],
                Some(Terrain::Mediterranean) => [Some(Sorte::Chataignes), Some(Sorte::Gibier), Some(Sorte::Baies)],
                Some(Terrain::Savanna) => [Some(Sorte::MilletSorgho), Some(Sorte::Gibier), None],
                Some(Terrain::Grassland | Terrain::Plains | Terrain::Steppe) => [Some(Sorte::Orge), Some(Sorte::Gibier), None],
                Some(Terrain::Marsh) => [Some(Sorte::Riz), Some(Sorte::Poisson), None],
                Some(Terrain::Riverbank) => [None, Some(Sorte::Poisson), None],
                Some(Terrain::Oasis) => [None, Some(Sorte::Gibier), Some(Sorte::Dattes)],
                _ => [None, Some(Sorte::Gibier), None],
            }
        }
    }
}

/// Sortes du centre de la ville : ses champs, ses bêtes et ses jardins (selon sa case).
pub fn sortes_du_centre(map: &WorldMap, h: Hex) -> [Option<Sorte>; 3] {
    let c = sortes_de_la_case(map, h, Some(Amenagement::Champ))[0];
    let b = sortes_de_la_case(map, h, Some(Amenagement::Paturage))[1];
    let j = sortes_de_la_case(map, h, Some(Amenagement::Maraichage))[2];
    [c, b, j]
}

/// Une sorte compte dans la diversité si elle couvre cette part de ce que la ville mange.
pub const SEUIL_SORTE: f64 = 0.05;
/// Bien-être par sorte au-delà de la première de chaque type, et au plus.
pub const BIEN_ETRE_PAR_SORTE: f64 = 2.0;
pub const BIEN_ETRE_DIVERSITE_MAX: f64 = 8.0;
/// Bonus de poids de l'intendant pour une sorte qui ne compte pas encore (après l'équilibre).
const POIDS_DIVERSITE: f64 = 1.0;

/// Sortes en plus (au-delà de la première de chaque type) qui comptent : la diversité des repas.
pub fn sortes_en_plus(par_sorte: &ParSorte, besoin: f64) -> u8 {
    let consommation = besoin.min(par_sorte.iter().sum::<f64>());
    let mut par_type = [0u8; 3];
    for (s, &n) in Sorte::TOUTES.iter().zip(par_sorte) {
        if n > 1e-9 && n >= SEUIL_SORTE * consommation - 1e-9 {
            par_type[s.type_()] += 1;
        }
    }
    par_type.iter().map(|&k| k.saturating_sub(1)).sum()
}

/// Un type compte dans l'équilibre s'il couvre cette part de ce que la ville mange (sa
/// consommation : le besoin, ou toute la nourriture en famine ; décision de Benjamin).
pub const SEUIL_TYPE: f64 = 0.15;
/// Bonus de poids de l'intendant pour la nourriture d'un type qui manque (après avoir nourri).
const POIDS_EQUILIBRE: f64 = 3.0;

/// Nombre de types qui comptent (0 à 3) pour une nourriture par type et un besoin donnés.
pub fn types_comptes(par_type: Parts, besoin: f64) -> u8 {
    let consommation = besoin.min(par_type.iter().sum::<f64>());
    par_type.iter().filter(|&&n| n > 1e-9 && n >= SEUIL_TYPE * consommation - 1e-9).count() as u8
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

    /// Somme des trois rendements (pour classer des cases).
    pub fn total(self) -> f64 {
        self.nourriture + self.production + self.deniers
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

/// Une oasis est petite : capacité de ses aménagements rapportée à celle d'une autre case.
pub const CAPACITE_OASIS: f64 = 0.5;

/// Capacité de la case en travailleurs (la moitié dans une oasis).
pub fn capacite(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> u32 {
    let c = amenagement.map_or(CAPACITE_SANS_AMENAGEMENT, Amenagement::capacite);
    if map.get(h) == Some(Terrain::Oasis) { (f64::from(c) * CAPACITE_OASIS).round() as u32 } else { c }
}

/// Taux de conversion de la subsistance : sur une case aménagée, ceux qui y vivent tirent aussi de
/// la nature (petite chasse, cueillette d'appoint, bois de chauffe) 75 % de ce que donne la cueillette
/// du terrain, atteint à 500 personnes (décision de Benjamin du 13/11/2026 ; les technologies le
/// relèveront, jusqu'à dépasser 100 %).
pub const TAUX_SUBSISTANCE: f64 = 0.75;
/// Personnes à partir desquelles la subsistance d'une case ne monte plus (les ressources communes se partagent).
pub const PLACES_SUBSISTANCE: u32 = 500;

/// Ce que donne une case selon ses travailleurs (règle du 13/11/2026). Sans aménagement : la
/// cueillette, le travail des chasseurs-cueilleurs (300 places, exposant 0,7). Avec un aménagement :
/// son travail (par grandeur, un rendement plein et sa capacité, exposant 0,85) plus la subsistance de
/// ceux qui y vivent (75 % de la cueillette du terrain, atteinte à 500 personnes, exposant 0,7). Le
/// travail est réglé pour que l'aménagement plein garde le total décidé (champ de plaine : 7,5).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Courbe {
    /// Cueillette (case sans aménagement) : rendement plein et places.
    pub cueillette: Rendements,
    pub places_cueillette: u32,
    /// Travail de l'aménagement : rendement plein de chaque grandeur et ses places.
    pub travail: Rendements,
    pub places: [u32; 3],
    /// Subsistance pleine (atteinte à `places_subsistance` personnes).
    pub subsistance: Rendements,
    pub places_subsistance: u32,
    /// Places de la case.
    pub capacite: u32,
}

/// Part du maximum que donnent `t` travailleurs sur `capacite` places.
fn part(t: u32, capacite: u32, exposant: f64) -> f64 {
    if t == 0 || capacite == 0 { 0.0 } else { (t as f64 / capacite as f64).min(1.0).powf(exposant) }
}

impl Courbe {
    /// Une courbe simple : un rendement plein sur `places` (la cueillette).
    pub fn simple(max: Rendements, places: u32) -> Self {
        Courbe { cueillette: max, places_cueillette: places, capacite: places, ..Default::default() }
    }

    /// Rendement avec `t` travailleurs : cueillette, ou travail plus subsistance.
    pub fn rendement(&self, t: u32) -> Rendements {
        let t = t.min(self.capacite);
        let a = |v: f64, i: usize| v * part(t, self.places[i], EXPOSANT_AMENAGEMENT);
        self.cueillette.fois(part(t, self.places_cueillette, EXPOSANT))
            + Rendements::new(a(self.travail.nourriture, 0), a(self.travail.production, 1), a(self.travail.deniers, 2))
            + self.subsistance_de(t)
    }

    /// La part de la subsistance avec `t` travailleurs.
    pub fn subsistance_de(&self, t: u32) -> Rendements {
        self.subsistance.fois(part(t.min(self.capacite), self.places_subsistance, EXPOSANT))
    }

    /// Places de la case.
    pub fn capacite(&self) -> u32 {
        self.capacite
    }

    /// Rendement à pleine capacité.
    pub fn max(&self) -> Rendements {
        self.rendement(self.capacite)
    }

    /// La courbe, multipliée (les outils des cases aménagées).
    pub fn fois(&self, k: f64) -> Self {
        Courbe { cueillette: self.cueillette.fois(k), travail: self.travail.fois(k), subsistance: self.subsistance.fois(k), ..*self }
    }
}

/// Courbe d'une case : ce qu'elle donne selon ses travailleurs, avec son aménagement.
pub fn courbe(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> Courbe {
    let mut y = map.yields(h).unwrap_or_default();
    // (champ, pâturage ou maraîchage pris sur la forêt : le bois est perdu, la terre devient une prairie)
    if matches!(amenagement, Some(Amenagement::Champ | Amenagement::Paturage | Amenagement::Legumineuses | Amenagement::Maraichage)) && defriche(map, h) {
        y = Yields::new(y.food + 1, (y.production - 2).max(0), y.deniers);
    }
    let r = Rendements::from(y);
    // Fertilité du sol (5b) : sur la nourriture que le champ tire du terrain, pas sur l'eau douce ;
    // un tiers de l'écart pour les pâturages ; ni la cueillette ni le bois
    let eau = if !map.is_fleuve(h) && map.get(h).is_some_and(|t| !t.is_water()) && map.fresh_water(h) { f64::from(crate::map::FRESH_WATER_FOOD) } else { 0.0 };
    let terre = (r.nourriture - eau).max(0.0);
    let sol = map.sol(h);
    let fertile = |m: f64| terre * m + eau.min(r.nourriture);
    let champ = fertile(sol.map_or(1.0, |s| s.fertilite())) * FACTEUR_CHAMP;
    // (une oasis est petite : la moitié des places)
    let k = if map.get(h) == Some(Terrain::Oasis) { CAPACITE_OASIS } else { 1.0 };
    let pl = |p: u32| (f64::from(p) * k).round() as u32;
    let Some(a) = amenagement else { return Courbe::simple(r, pl(CAPACITE_SANS_AMENAGEMENT)) };
    // Totaux pleins décidés (nourriture, production) et places de chaque grandeur ; le travail est ce
    // qui reste une fois la subsistance comptée
    let subsistance = r.fois(TAUX_SUBSISTANCE);
    let (total, places) = match a {
        Amenagement::Champ => (Rendements::new(champ, 0.0, 0.0), [3_000, 0, 0]),
        Amenagement::Legumineuses => (Rendements::new(champ * RENDEMENT_LEGUMINEUSES, 0.0, 0.0), [3_000, 0, 0]),
        Amenagement::Maraichage => (Rendements::new(fertile(sol.map_or(1.0, |s| s.fertilite())) * RENDEMENT_MARAICHAGE, 0.0, 0.0), [600, 0, 0]),
        Amenagement::Paturage => (Rendements::new(fertile(sol.map_or(1.0, |s| s.fertilite_paturage())) * FACTEUR_PATURAGE, r.production + 1.0, 0.0), [1_200, CAPACITE_PRODUCTION_PATURAGE, 0]),
        Amenagement::CoupeDeBois => (Rendements::new(0.0, r.production * FACTEUR_COUPE, 0.0), [0, 600, 0]),
        // (la case sert à l'extraction : un peu de production en plus)
        Amenagement::Exploitation => (Rendements::new(0.0, r.production + PRODUCTION_EXPLOITATION, 0.0), [0, 500, 0]),
    };
    let reste = |t: f64, s: f64, places: u32| if places > 0 { (t - s).max(0.0) } else { 0.0 };
    let travail = Rendements::new(reste(total.nourriture, subsistance.nourriture, places[0]), reste(total.production, subsistance.production, places[1]), 0.0);
    Courbe { cueillette: Rendements::default(), places_cueillette: 0, travail, places: places.map(pl), subsistance, places_subsistance: pl(PLACES_SUBSISTANCE), capacite: pl(a.capacite()) }
}

/// Rendement de la case à pleine capacité.
pub fn rendement_max(map: &WorldMap, h: Hex, amenagement: Option<Amenagement>) -> Rendements {
    courbe(map, h, amenagement).max()
}

impl CaseDispo {
    /// La case avec un autre aménagement : sa courbe, son maximum, ses places.
    pub fn amenager(&mut self, map: &WorldMap, a: Option<Amenagement>) {
        self.courbe = courbe(map, self.hex, a);
        self.max = self.courbe.max();
        self.capacite = self.courbe.capacite();
        self.amenagee = a.is_some();
    }
}

/// Rendement de `travailleurs` sur une case simple (une seule courbe, celle de la nature).
#[cfg(test)]
fn rendement(max: Rendements, travailleurs: u32, capacite: u32) -> Rendements {
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

/// Objectif de nourriture de l'intendant : la couverture du besoin qu'il vise d'abord, avant de
/// répartir le reste des travailleurs selon les priorités (décision de Benjamin, maquette v19).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObjectifNourriture {
    /// 1 × le besoin : la croissance s'arrête, le reste va à la production et aux deniers.
    NourrirSeulement,
    /// 1,25 × le besoin : croissance modérée.
    Croitre,
    /// 1,5 × le besoin : croissance pleine.
    #[default]
    CroissanceMaximale,
    /// 2 × le besoin : un excédent (au grenier ; à vendre quand le commerce viendra).
    Excedent,
}

impl ObjectifNourriture {
    pub const ALL: [ObjectifNourriture; 4] = [ObjectifNourriture::NourrirSeulement, ObjectifNourriture::Croitre, ObjectifNourriture::CroissanceMaximale, ObjectifNourriture::Excedent];

    /// Couverture du besoin visée.
    pub fn cible(self) -> f64 {
        match self {
            ObjectifNourriture::NourrirSeulement => 1.0,
            ObjectifNourriture::Croitre => 1.25,
            ObjectifNourriture::CroissanceMaximale => 1.0 + crate::rules::SURPLUS_POUR_CROITRE,
            ObjectifNourriture::Excedent => 2.0,
        }
    }

    /// Nom affiché.
    pub fn nom(self) -> &'static str {
        match self {
            ObjectifNourriture::NourrirSeulement => "Nourrir seulement",
            ObjectifNourriture::Croitre => "Croître",
            ObjectifNourriture::CroissanceMaximale => "Croissance maximale",
            ObjectifNourriture::Excedent => "Excédent à vendre",
        }
    }

    /// Code de la console.
    pub fn code(self) -> &'static str {
        match self {
            ObjectifNourriture::NourrirSeulement => "nourrir",
            ObjectifNourriture::Croitre => "croitre",
            ObjectifNourriture::CroissanceMaximale => "maximale",
            ObjectifNourriture::Excedent => "excedent",
        }
    }
}

/// Consignes du joueur à l'intendant d'une ville.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Consignes {
    /// Objectif de nourriture (la nourriture n'a pas de priorité : c'est son objectif qui la règle).
    #[serde(default)]
    pub objectif: ObjectifNourriture,
    /// Priorité de la production et des deniers (la première case, celle de la nourriture, ne sert pas).
    pub priorites: [Priorite; 3],
    /// Cases forcées et leur effectif (en travailleurs).
    pub forcees: Vec<(Hex, u32)>,
}

/// Case que la ville peut exploiter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaseDispo {
    pub hex: Hex,
    /// Rendement à pleine capacité.
    pub max: Rendements,
    /// Ce qu'elle donne selon ses travailleurs.
    pub courbe: Courbe,
    /// Part de chaque type dans sa nourriture.
    pub parts: Parts,
    /// Sorte de chaque type.
    pub sortes: [Option<Sorte>; 3],
    pub capacite: u32,
    /// Distance au centre de la ville (en cases).
    pub distance: i32,
    /// Case aménagée (les outils y améliorent le rendement, 5h).
    pub amenagee: bool,
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
    /// Nourriture de la ville par type (centre compris).
    pub par_type: Parts,
    /// Nourriture de la ville par sorte (centre compris).
    pub par_sorte: ParSorte,
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

/// Poids des trois ressources selon le manque de nourriture (couverture du besoin), l'objectif de
/// nourriture et les priorités de la production et des deniers.
fn poids(couverture: f64, consignes: &Consignes) -> [f64; 3] {
    // (tant que la ville ne se nourrit pas, la nourriture pèse lourd ; puis jusqu'à l'objectif ; puis peu)
    let nourriture = if couverture < 1.0 {
        4.0
    } else if couverture < consignes.objectif.cible() - 1e-9 {
        2.0
    } else {
        0.3
    };
    [nourriture, consignes.priorites[1].facteur(), 0.6 * consignes.priorites[2].facteur()]
}

/// Répartit les `actifs` sur les cases : d'abord les cases forcées, puis par blocs là où le gain
/// pondéré est le plus fort. `centre` : rendement du centre de la ville (sans travailleurs).
#[cfg(test)]
pub fn repartir(cases: &[CaseDispo], actifs: u32, besoin_nourriture: f64, centre: Rendements, consignes: &Consignes) -> Repartition {
    repartir_avec(cases, actifs, besoin_nourriture, centre, [None; 3], consignes)
}

/// `repartir`, avec les sortes du centre de la ville (diversité des repas).
pub fn repartir_avec(cases: &[CaseDispo], actifs: u32, besoin_nourriture: f64, centre: Rendements, sortes_centre: [Option<Sorte>; 3], consignes: &Consignes) -> Repartition {
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
    let total = |n: &[u32]| cases.iter().zip(n).fold(centre, |acc, (c, &k)| acc + c.courbe.rendement(k));
    let par_type = |n: &[u32]| {
        let mut t = PARTS_CENTRE.map(|p| p * centre.nourriture);
        for (c, &k) in cases.iter().zip(n) {
            let f = c.courbe.rendement(k).nourriture;
            for (x, p) in t.iter_mut().zip(c.parts) {
                *x += f * p;
            }
        }
        t
    };
    let par_sorte = |n: &[u32]| {
        let mut s = [0.0; N_SORTES];
        let mut ajoute = |sortes: [Option<Sorte>; 3], parts: Parts, f: f64| {
            for t in 0..3 {
                if let Some(x) = sortes[t] {
                    s[x.index()] += f * parts[t];
                }
            }
        };
        ajoute(sortes_centre, PARTS_CENTRE, centre.nourriture);
        for (c, &k) in cases.iter().zip(n) {
            ajoute(c.sortes, c.parts, c.courbe.rendement(k).nourriture);
        }
        s
    };
    let mut courant = total(&n);
    let mut types = par_type(&n);
    let mut sortes = par_sorte(&n);
    // (blocs plus gros dans les grandes villes : le calcul reste léger)
    let bloc = BLOC.max(actifs / 200 / BLOC * BLOC);
    while libres > 0 {
        let b = bloc.min(libres);
        let couverture = courant.nourriture / besoin_nourriture.max(1e-6);
        let w = poids(couverture, consignes);
        // Équilibre (après avoir nourri) : la nourriture d'un type sous le seuil pèse en plus
        let manque: Parts = std::array::from_fn(|t| if couverture >= 1.0 && types[t] < SEUIL_TYPE * besoin_nourriture { POIDS_EQUILIBRE } else { 0.0 });
        // Diversité (après l'équilibre) : une sorte qui ne compte pas encore pèse un peu en plus
        let equilibre_fait = manque.iter().all(|&m| m == 0.0);
        let mut meilleur: Option<(usize, f64)> = None;
        for (i, c) in cases.iter().enumerate() {
            if forcee[i] || n[i] >= c.capacite {
                continue;
            }
            let k = (n[i] + b).min(c.capacite);
            let gain = c.courbe.rendement(k) - c.courbe.rendement(n[i]);
            let eloignement = (1.0 - PENALITE_DISTANCE * (c.distance - 1).max(0) as f64).max(0.5);
            // (gain par travailleur : un bloc tronqué par la capacité n'est pas avantagé)
            let mut equilibre: f64 = c.parts.iter().zip(manque).map(|(p, m)| gain.nourriture * p * m).sum();
            if couverture >= 1.0 && equilibre_fait {
                for t in 0..3 {
                    if let Some(x) = c.sortes[t]
                        && sortes[x.index()] < SEUIL_SORTE * besoin_nourriture
                    {
                        equilibre += gain.nourriture * c.parts[t] * POIDS_DIVERSITE;
                    }
                }
            }
            let score = (gain.valeurs().iter().zip(w).map(|(g, w)| g * w).sum::<f64>() + equilibre) * eloignement / (k - n[i]) as f64;
            if score > 1e-12 && meilleur.is_none_or(|(_, s)| score > s) {
                meilleur = Some((i, score));
            }
        }
        let Some((i, _)) = meilleur else { break };
        let k = (n[i] + b).min(cases[i].capacite);
        let avant = cases[i].courbe.rendement(n[i]);
        let apres = cases[i].courbe.rendement(k);
        courant = courant - avant + apres;
        for (x, p) in types.iter_mut().zip(cases[i].parts) {
            *x += (apres.nourriture - avant.nourriture) * p;
        }
        for t in 0..3 {
            if let Some(x) = cases[i].sortes[t] {
                sortes[x.index()] += (apres.nourriture - avant.nourriture) * cases[i].parts[t];
            }
        }
        libres -= k - n[i];
        n[i] = k;
    }
    Repartition {
        cases: cases.iter().zip(&n).filter(|(_, k)| **k > 0).map(|(c, k)| (c.hex, *k)).collect(),
        actifs,
        chomeurs: libres,
        rendements: total(&n),
        par_type: par_type(&n),
        par_sorte: par_sorte(&n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_objectif_de_nourriture_regle_la_part_des_champs() {
        // Un pré (nourriture) et un bois (production), assez de travailleurs pour choisir
        let pre = case(1, Rendements::new(4.0, 0.0, 0.0), 1000);
        let bois = case(2, Rendements::new(0.5, 3.0, 0.0), 1000);
        let avec = |o| {
            let c = Consignes { objectif: o, ..Consignes::default() };
            repartir(&[pre, bois], 1000, 2.0, Rendements::default(), &c).rendements
        };
        let seulement = avec(ObjectifNourriture::NourrirSeulement);
        let maximale = avec(ObjectifNourriture::CroissanceMaximale);
        let excedent = avec(ObjectifNourriture::Excedent);
        assert!(seulement.nourriture < maximale.nourriture && maximale.nourriture < excedent.nourriture, "{seulement:?} {maximale:?} {excedent:?}");
        assert!(seulement.production > excedent.production);
        // Nourrir seulement : au moins le besoin
        assert!(seulement.nourriture >= 2.0);
    }

    fn case(col: i32, max: Rendements, capacite: u32) -> CaseDispo {
        CaseDispo { hex: Hex::from_offset(col, 0), max, courbe: Courbe::simple(max, capacite), parts: [1.0, 0.0, 0.0], sortes: [None; 3], capacite, distance: 1, amenagee: false }
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
    fn l_equilibre_compte_sur_la_consommation() {
        // Besoin 10 : un type compte à partir de 1,5 (15 % de ce qui est mangé)
        assert_eq!(types_comptes([8.5, 1.5, 0.0], 10.0), 2);
        assert_eq!(types_comptes([8.5, 1.4, 0.1], 10.0), 1);
        // Un excédent de céréales ne fait pas perdre les protéines (seuil sur le besoin, pas la production)
        assert_eq!(types_comptes([40.0, 1.5, 1.5], 10.0), 3);
        // En famine, le seuil porte sur ce qui est mangé
        assert_eq!(types_comptes([3.0, 0.7, 0.7], 10.0), 3);
        assert_eq!(types_comptes([3.0, 0.5, 0.5], 10.0), 1, "12,5 % de ce qui est mangé : sous le seuil");
    }

    #[test]
    fn la_diversite_compte_les_sortes_en_plus_de_chaque_type() {
        let mut p = [0.0; N_SORTES];
        // Besoin 10 : une sorte compte à partir de 0,5
        p[Sorte::Ble.index()] = 6.0;
        p[Sorte::Orge.index()] = 0.6;
        p[Sorte::Boeufs.index()] = 2.0;
        p[Sorte::Gibier.index()] = 0.4;
        p[Sorte::Baies.index()] = 1.0;
        assert_eq!(sortes_en_plus(&p, 10.0), 1, "orge en plus du blé ; le gibier (4 %) ne compte pas");
        p[Sorte::Gibier.index()] = 0.5;
        assert_eq!(sortes_en_plus(&p, 10.0), 2);
        // Les sortes par case : céréale selon le climat, bétail selon le biome, l'eau donne du poisson
        let mut map = WorldMap::filled(6, 6, Terrain::Steppe);
        let h = Hex::from_offset(2, 2);
        assert_eq!(sortes_de_la_case(&map, h, Some(Amenagement::Paturage))[1], Some(Sorte::MoutonsChevres));
        assert_eq!(sortes_de_la_case(&map, h, Some(Amenagement::Champ))[0], Some(Sorte::MilletSorgho));
        assert_eq!(sortes_de_la_case(&map, h, Some(Amenagement::Legumineuses))[1], Some(Sorte::Legumineuses));
        map.set(h, Terrain::Taiga);
        assert_eq!(sortes_de_la_case(&map, h, None), [None, Some(Sorte::Gibier), Some(Sorte::Baies)]);
        assert_eq!(sortes_de_la_case(&map, h, Some(Amenagement::Paturage))[1], Some(Sorte::BetesExtremes));
        map.set(h, Terrain::Lake);
        assert_eq!(sortes_de_la_case(&map, h, None), [None, Some(Sorte::Poisson), None]);
        // Chaque sorte a son type
        assert!(Sorte::TOUTES.iter().all(|s| s.type_() < 3) && Sorte::TOUTES.len() == N_SORTES);
    }

    #[test]
    fn l_intendant_cherche_l_equilibre_apres_avoir_nourri() {
        // Un champ (céréales, plus de nourriture) et un pâturage (protéines, moins)
        let champ = CaseDispo { parts: [1.0, 0.0, 0.0], ..case(1, Rendements::new(6.0, 0.0, 0.0), 1500) };
        let pre = CaseDispo { parts: [0.0, 1.0, 0.0], ..case(2, Rendements::new(3.0, 1.0, 0.0), 1000) };
        let r = repartir(&[champ, pre], 1500, 2.0, Rendements::default(), &Consignes::default());
        assert!(r.rendements.nourriture >= 2.0, "nourrir d'abord");
        assert!(r.par_type[1] >= SEUIL_TYPE * 2.0, "puis des protéines : {:?}", r.par_type);
        assert!(r.sur(pre.hex) > 0);
        // Puis la diversité : une seconde sorte de céréales attire des travailleurs
        let ble = CaseDispo { sortes: [Some(Sorte::Ble), None, None], ..case(3, Rendements::new(4.0, 0.0, 0.0), 1500) };
        let orge = CaseDispo { sortes: [Some(Sorte::Orge), None, None], ..case(4, Rendements::new(2.0, 0.0, 0.0), 1500) };
        let r = repartir_avec(&[ble, orge], 1500, 2.0, Rendements::new(2.0, 0.0, 0.0), [None, Some(Sorte::Boeufs), Some(Sorte::Legumes)], &Consignes::default());
        assert!(r.par_sorte[Sorte::Orge.index()] >= SEUIL_SORTE * 2.0, "{:?}", r.par_sorte);
    }

    #[test]
    fn cultures_du_desert_irrigue() {
        let mut map = WorldMap::filled(8, 8, Terrain::Desert);
        let (d, r, o) = (Hex::from_offset(2, 2), Hex::from_offset(4, 4), Hex::from_offset(5, 2));
        map.set(r, Terrain::Riverbank);
        map.set(o, Terrain::Oasis);
        // Le désert : rien ; les rives vertes et l'oasis : champs, légumineuses, pâturages, jardins
        assert!(Amenagement::possibles(&map, d).is_empty());
        let tout = vec![Amenagement::Champ, Amenagement::Legumineuses, Amenagement::Paturage, Amenagement::Maraichage];
        assert_eq!(Amenagement::possibles(&map, o), tout);
        map.set_fleuve(Hex::from_offset(4, 5));
        assert_eq!(Amenagement::possibles(&map, r), tout);
        // Une oasis est petite : la moitié des travailleurs
        assert_eq!(capacite(&map, o, Some(Amenagement::Champ)), capacite(&map, r, Some(Amenagement::Champ)) / 2);
        // Sortes : blé des rives (Égypte), bœufs au bord du fleuve, chameaux à l'oasis, dattes
        assert_eq!(sortes_de_la_case(&map, r, Some(Amenagement::Champ))[0], Some(Sorte::Ble));
        assert_eq!(sortes_de_la_case(&map, r, Some(Amenagement::Paturage))[1], Some(Sorte::Boeufs));
        assert_eq!(sortes_de_la_case(&map, o, Some(Amenagement::Paturage))[1], Some(Sorte::BetesExtremes));
        assert_eq!(sortes_de_la_case(&map, o, None)[2], Some(Sorte::Dattes));
    }

    #[test]
    fn amenagements_selon_la_case() {
        let mut map = WorldMap::filled(6, 6, Terrain::Grassland);
        let h = Hex::from_offset(2, 2);
        assert_eq!(Amenagement::possibles(&map, h), vec![Amenagement::Champ, Amenagement::Legumineuses, Amenagement::Paturage, Amenagement::Maraichage]);
        let champ = rendement_max(&map, h, Some(Amenagement::Champ));
        assert_eq!(champ.nourriture, FACTEUR_CHAMP * rendement_max(&map, h, None).nourriture);
        // Légumineuses : 80 % d'un champ ; maraîchage : 1,8 × la case, 600 places
        assert!((rendement_max(&map, h, Some(Amenagement::Legumineuses)).nourriture - 0.8 * champ.nourriture).abs() < 1e-9);
        assert!((rendement_max(&map, h, Some(Amenagement::Maraichage)).nourriture - RENDEMENT_MARAICHAGE * rendement_max(&map, h, None).nourriture).abs() < 1e-9);
        assert_eq!(capacite(&map, h, Some(Amenagement::Maraichage)), 600);
        // La cueillette rapporte plus par travailleur ; les cultures portent plus de monde, et
        // chacun de leurs travailleurs nourrit au moins sa famille (le minimum vital)
        let par_travailleur = |a: Option<Amenagement>| rendement_max(&map, h, a).nourriture / f64::from(capacite(&map, h, a));
        let cueillette = par_travailleur(None);
        let minimum_vital = 1.0 / (crate::rules::HABITANTS_PAR_NOURRITURE * PART_ACTIVE);
        for a in [Amenagement::Champ, Amenagement::Legumineuses, Amenagement::Paturage, Amenagement::Maraichage] {
            assert!(par_travailleur(Some(a)) < cueillette, "{a:?}");
            assert!(par_travailleur(Some(a)) >= minimum_vital - 1e-12, "{a:?}");
            assert!(capacite(&map, h, Some(a)) > capacite(&map, h, None));
        }
        // Types : champ → céréales, légumineuses et pâturage → protéines, maraîchage → fruits et légumes ;
        // la subsistance y ajoute ceux de la nature (graines sauvages et gibier sur une prairie)
        assert_eq!(parts_propres(&map, h, Some(Amenagement::Champ)), [1.0, 0.0, 0.0]);
        assert_eq!(parts_propres(&map, h, Some(Amenagement::Legumineuses)), [0.0, 1.0, 0.0]);
        assert_eq!(parts_propres(&map, h, Some(Amenagement::Maraichage)), [0.0, 0.0, 1.0]);
        let champ_types = parts_de_la_case(&map, h, Some(Amenagement::Champ));
        assert!(champ_types[0] > 0.9 && champ_types[1] > 0.0 && (champ_types.iter().sum::<f64>() - 1.0).abs() < 1e-9, "{champ_types:?}");
        assert_eq!(sortes_de_la_case(&map, h, Some(Amenagement::Champ))[1], Some(Sorte::Gibier), "le gibier de la subsistance");
        assert_eq!(parts_de_la_case(&map, h, None), [0.4, 0.6, 0.0], "prairie : graines sauvages et gibier");
        map.set(h, Terrain::Forest);
        let sans_gisement: Vec<Amenagement> = Amenagement::ALL.into_iter().filter(|&a| a != Amenagement::Exploitation).collect();
        assert_eq!(Amenagement::possibles(&map, h), sans_gisement, "forêt : bois, ou défrichement (pas d'exploitation sans gisement)");
        assert_eq!(Amenagement::Champ.cout(&map, h), 30);
        assert_eq!(Amenagement::CoupeDeBois.cout(&map, h), 15);
        // Coupe de bois : 5 de production (son travail et la subsistance), la nourriture de la subsistance
        let bois = courbe(&map, h, Some(Amenagement::CoupeDeBois));
        let foret = courbe(&map, h, None);
        assert!((bois.max().production - 2.0 * FACTEUR_COUPE).abs() < 1e-9);
        assert!((bois.max().nourriture - TAUX_SUBSISTANCE).abs() < 1e-9);
        // Un bûcheron rapporte plus de production qu'un cueilleur ; avec les 300 travailleurs de la forêt, la coupe donne déjà plus
        assert!(bois.max().production / 600.0 > foret.max().production / 300.0);
        assert!(bois.rendement(300).production > foret.rendement(300).production + 0.5);
        // Défriché : une prairie (sa subsistance aussi)
        let defriche = rendement_max(&map, h, Some(Amenagement::Champ));
        assert!((defriche.nourriture - 2.0 * FACTEUR_CHAMP).abs() < 1e-9 && defriche.production == 0.0, "{defriche:?}");
        map.set(h, Terrain::Desert);
        assert!(Amenagement::possibles(&map, h).is_empty());
        map.set(h, Terrain::Grassland);
        map.set_relief(h, Relief::Rugged);
        assert_eq!(Amenagement::possibles(&map, h), vec![Amenagement::Paturage]);
    }

    #[test]
    fn la_subsistance_rapporte_moins_que_la_cueillette() {
        // À nombre égal de personnes, la subsistance d'une case aménagée rapporte moins que la cueillette
        for terrain in [Terrain::Plains, Terrain::Grassland, Terrain::Forest, Terrain::Savanna] {
            let map = WorldMap::filled(6, 6, terrain);
            let h = Hex::from_offset(2, 2);
            let nature = courbe(&map, h, None);
            for a in Amenagement::possibles(&map, h).into_iter().filter(|&a| !defriche(&map, h) || a == Amenagement::CoupeDeBois) {
                let c = courbe(&map, h, Some(a));
                for t in (50..=3_000).step_by(50) {
                    let (s, n) = (c.subsistance_de(t), nature.rendement(t));
                    assert!(s.nourriture < n.nourriture + 1e-9 && s.production < n.production + 1e-9, "{terrain:?} {a:?} {t} : {s:?} ≥ {n:?}");
                }
            }
        }
    }

    #[test]
    fn un_amenagement_plein_garde_son_total() {
        // Le travail est réglé pour que l'aménagement plein donne le total décidé (subsistance comprise)
        let map = WorldMap::filled(6, 6, Terrain::Plains);
        let h = Hex::from_offset(2, 2);
        let plein = |a| rendement_max(&map, h, Some(a));
        assert!((plein(Amenagement::Champ).nourriture - 7.5).abs() < 1e-9);
        assert!((plein(Amenagement::Legumineuses).nourriture - 6.0).abs() < 1e-9);
        assert!((plein(Amenagement::Maraichage).nourriture - 1.8).abs() < 1e-9);
        assert!((plein(Amenagement::Paturage).nourriture - 3.0).abs() < 1e-9);
        assert!((plein(Amenagement::Paturage).production - 2.0).abs() < 1e-9);
        // La grandeur qu'il ne produit pas : la subsistance seule (75 % de la plaine)
        assert!((plein(Amenagement::Champ).production - TAUX_SUBSISTANCE).abs() < 1e-9);
        // Avec peu de travailleurs, le champ n'est jamais nul : ceux qui y vivent ont leur subsistance
        let champ = courbe(&map, h, Some(Amenagement::Champ));
        assert!(champ.rendement(300).nourriture > courbe(&map, h, None).rendement(300).nourriture);
    }

    #[test]
    fn la_prairie_reste_plus_riche_que_la_plaine() {
        // (sa subsistance est plus forte : décision de Benjamin)
        let (plaine, prairie) = (WorldMap::filled(6, 6, Terrain::Plains), WorldMap::filled(6, 6, Terrain::Grassland));
        let h = Hex::from_offset(2, 2);
        let (a, b) = (courbe(&plaine, h, Some(Amenagement::Champ)), courbe(&prairie, h, Some(Amenagement::Champ)));
        for t in (100..=3_000).step_by(100) {
            assert!(b.rendement(t).nourriture > a.rendement(t).nourriture, "{t}");
        }
    }

    #[test]
    fn l_intendant_remplit_une_coupe_de_bois() {
        // Une forêt avec sa coupe et une plaine sans aménagement : 900 actifs, la production prioritaire
        let map = WorldMap::filled(6, 6, Terrain::Forest);
        let h = Hex::from_offset(2, 2);
        let bois = courbe(&map, h, Some(Amenagement::CoupeDeBois));
        let coupe = CaseDispo { hex: h, max: bois.max(), courbe: bois, parts: [0.4, 0.6, 0.0], sortes: [None; 3], capacite: bois.capacite(), distance: 1, amenagee: true };
        let plaine = CaseDispo { hex: Hex::from_offset(3, 2), courbe: Courbe::simple(Rendements::new(1.0, 1.0, 0.0), 300), max: Rendements::new(1.0, 1.0, 0.0), parts: [0.5, 0.5, 0.0], sortes: [None; 3], capacite: 300, distance: 1, amenagee: false };
        let mut cons = Consignes::default();
        cons.priorites[1] = Priorite::Maximale;
        let r = repartir(&[coupe, plaine], 900, 0.5, Rendements::new(2.0, 2.0, 1.0), &cons);
        let sur_coupe = r.cases.iter().find(|(c, _)| *c == h).map_or(0, |(_, n)| *n);
        assert!(sur_coupe > 300, "la coupe dépasse le socle de la cueillette : {sur_coupe}");
    }
}
