//! Terrains, carte du monde et génération procédurale (logique pure).

use crate::hex::{Hex, Wrap};
use crate::climate::{self, Climate, Region};
use crate::basins::{self, Basin, Basins, NamedRiver};
use crate::hydro::{self, Hydrology};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use std::collections::HashSet;

/// Source des coûts de déplacement utilisée par le pathfinding.
pub trait MoveCosts {
    /// Coût du terrain de `hex` ; `None` = infranchissable ou hors carte.
    fn move_cost(&self, hex: Hex) -> Option<u32>;

    /// Coût d'un pas de `from` vers sa voisine `to` : terrain d'arrivée + pente.
    /// `None` = infranchissable (terrain, falaise…).
    fn step_cost(&self, from: Hex, to: Hex) -> Option<u32> {
        let _ = from;
        self.move_cost(to)
    }

    /// Tour d'est en ouest de la carte (voisines, distances).
    fn wrap(&self) -> Wrap {
        Wrap::NONE
    }
}

/// Altitude : 0 = eau, 1 (littoral) à 6 (massif, où se trouvent les sommets).
pub const MAX_LEVEL: u8 = 6;

/// Falaise entre deux cases de terre voisines : écart d'au moins 3 classes d'altitude
/// (≈ 1 000 m et plus), infranchissable dans les deux sens (et dessinée comme telle en 3D).
pub fn is_cliff(from_level: u8, to_level: u8) -> bool {
    (i32::from(to_level) - i32::from(from_level)).abs() >= 3
}

/// Surcoût d'un pas selon la dénivelée réelle (m) : montée de moins de `CLIMB_GENTLE_M`
/// gratuite, jusqu'à `CLIMB_STEEP_M` +1 PM, au-delà +2 PM ; la descente est gratuite. (À 40 km
/// entre deux centres de cases, c'est surtout le relief interne à la case qui gêne : voir `Relief`.)
pub fn climb_cost(from_m: f32, to_m: f32) -> u32 {
    let up = to_m - from_m;
    if up < CLIMB_GENTLE_M {
        0
    } else if up < CLIMB_STEEP_M {
        1
    } else {
        2
    }
}
pub const CLIMB_GENTLE_M: f32 = 400.0;
pub const CLIMB_STEEP_M: f32 = 900.0;

/// Relief interne à une case, du plus doux au plus escarpé. Il s'ajoute au terrain (biome) :
/// une forêt peut être vallonnée, une toundra montagneuse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relief {
    #[default]
    Flat,
    /// Collines.
    Hills,
    /// Relief accidenté : gorges, crêtes, versants raides.
    Rugged,
    /// Montagne : franchissable, à 3 PM.
    Mountain,
    /// Pic : sommet infranchissable.
    Peak,
}

impl Relief {
    pub const ALL: [Relief; 5] = [Relief::Flat, Relief::Hills, Relief::Rugged, Relief::Mountain, Relief::Peak];

    pub fn name(self) -> &'static str {
        match self {
            Relief::Flat => "plat",
            Relief::Hills => "vallonné",
            Relief::Rugged => "accidenté",
            Relief::Mountain => "montagne",
            Relief::Peak => "pic",
        }
    }

    /// Couleur sur la carte du relief (sRGB).
    pub fn color(self) -> (f32, f32, f32) {
        match self {
            Relief::Flat => (0.86, 0.88, 0.76),
            Relief::Hills => (0.80, 0.75, 0.47),
            Relief::Rugged => (0.71, 0.52, 0.31),
            Relief::Mountain => (0.47, 0.35, 0.29),
            Relief::Peak => (0.96, 0.96, 0.98),
        }
    }

    /// Surcoût de déplacement (PM) ajouté au terrain, sous le plafond `MOUNTAIN_MOVE_COST`.
    pub fn move_extra(self) -> u32 {
        match self {
            Relief::Flat => 0,
            Relief::Hills => HILL_MOVE_COST,
            Relief::Rugged => RUGGED_MOVE_COST,
            Relief::Mountain | Relief::Peak => 0,
        }
    }

    /// Bonus de défense (combat) : collines et relief accidenté une fois, montagne deux fois.
    pub fn defense_steps(self) -> i32 {
        match self {
            Relief::Flat | Relief::Peak => 0,
            Relief::Hills | Relief::Rugged => 1,
            Relief::Mountain => 2,
        }
    }

    /// Empêche la charge de cavalerie.
    pub fn blocks_charge(self) -> bool {
        self != Relief::Flat
    }
}

/// Changement de niveau entre deux cases de terre voisines (hors montagnes) :
/// `high` est la case haute, `dir` la direction de la case basse, `diff` l'écart (1 ou plus).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelEdge {
    pub high: Hex,
    pub dir: usize,
    pub diff: u8,
}

impl LevelEdge {
    pub fn low(&self) -> Hex {
        self.high + crate::hex::DIRECTIONS[self.dir]
    }
    /// Falaise : infranchissable.
    pub fn is_cliff(&self) -> bool {
        self.diff >= 3
    }
}

/// Vérité terrain : utilisée pour le déplacement effectif et dans les tests.
impl MoveCosts for WorldMap {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        if self.is_fleuve(hex) {
            return Some(FLEUVE_MOVE_COST);
        }
        let base = self.get(hex)?.move_cost()?;
        match self.relief(hex) {
            Relief::Peak => None,
            Relief::Mountain => Some(MOUNTAIN_MOVE_COST),
            r => Some((base + r.move_extra()).min(MOUNTAIN_MOVE_COST.max(base))),
        }
    }

    fn step_cost(&self, from: Hex, to: Hex) -> Option<u32> {
        let terrain = self.move_cost(to)?;
        if self.cliff_between(from, to) {
            return None;
        }
        let river = if self.river_between(from, to) { RIVER_CROSSING_COST } else { 0 };
        Some(terrain + river + climb_cost(self.elevation(from)?, self.elevation(to)?))
    }

    fn wrap(&self) -> Wrap {
        self.wrap
    }
}

/// Carte telle que le joueur la connaît : une case non explorée est supposée
/// franchissable, pour ne rien révéler du brouillard, mais au prix fort (2 PM) :
/// à distance égale, les trajets préfèrent le terrain connu à l'inconnu.
pub struct KnownMap<'a> {
    pub map: &'a WorldMap,
    pub explored: &'a HashSet<Hex>,
    /// Cases occupées par l'ennemi (connues du joueur) : infranchissables tant qu'il n'y a pas de combat.
    pub blocked: Option<&'a HashSet<Hex>>,
}

impl<'a> KnownMap<'a> {
    pub fn new(map: &'a WorldMap, explored: &'a HashSet<Hex>) -> Self {
        Self { map, explored, blocked: None }
    }

    pub fn blocking(mut self, blocked: &'a HashSet<Hex>) -> Self {
        self.blocked = Some(blocked);
        self
    }
}

pub const UNKNOWN_MOVE_COST: u32 = 2;

impl MoveCosts for KnownMap<'_> {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        if !self.map.contains(hex) || self.blocked.is_some_and(|b| b.contains(&hex)) {
            None
        } else if self.explored.contains(&hex) {
            self.map.move_cost(hex)
        } else {
            Some(UNKNOWN_MOVE_COST)
        }
    }

    /// La pente n'est connue qu'entre deux cases explorées.
    fn step_cost(&self, from: Hex, to: Hex) -> Option<u32> {
        let cost = self.move_cost(to)?;
        if self.explored.contains(&from) && self.explored.contains(&to) {
            self.map.step_cost(from, to)
        } else {
            Some(cost)
        }
    }

    fn wrap(&self) -> Wrap {
        self.map.wrap
    }
}

/// Répartition des terres émergées par classe d'altitude (bornes cumulées des classes 1 à 5 ; le
/// reste est en classe 6).
const LEVEL_BANDS: [f64; 5] = [0.16, 0.36, 0.56, 0.74, 0.87];
/// Percentile des terres au-delà duquel commence le pied des sommets (`SUMMIT_FOOT_M`).
const MOUNTAIN_PERCENTILE: f64 = 0.96;
/// Parts des terres par relief (les plus escarpées d'abord) : pics, montagnes, relief accidenté,
/// collines ; le reste est plat. Une montagne demande au moins `MOUNTAIN_MIN_M`, un pic
/// `PEAK_MIN_M` et d'être un sommet local (jamais deux pics côte à côte).
const PEAK_SHARE: f64 = 0.01;
const MOUNTAIN_SHARE: f64 = 0.05;
const RUGGED_SHARE: f64 = 0.08;
const HILL_SHARE: f64 = 0.15;
const MOUNTAIN_MIN_M: f32 = 1_500.0;
const PEAK_MIN_M: f32 = 2_400.0;
/// Un pic peut avoir une voisine un peu plus haute que lui (crête), de cet écart au plus (m).
const PEAK_TOLERANCE_M: f32 = 200.0;
/// Montagne : 3 PM (plafond aussi pour les autres reliefs) ; relief accidenté +2 PM.
pub const MOUNTAIN_MOVE_COST: u32 = 3;
pub const RUGGED_MOVE_COST: u32 = 2;
/// Rendements d'une montagne (pâturages d'altitude, pierre).
pub const MOUNTAIN_YIELDS: Yields = Yields::new(0, 1, 0);
/// Collines : +1 de production et +1 PM par rapport au terrain de base.
pub const HILL_PRODUCTION: i32 = 1;
/// Températures moyennes annuelles (°C) sous lesquelles la terre est enneigée, ou en toundra.
pub const SNOW_BELOW: f32 = -15.0;
pub const TUNDRA_BELOW: f32 = -5.0;
/// Indice d'aridité (précipitations / évaporation potentielle), seuils du PNUE : sous 0,2 la
/// terre est aride (désert), sous 0,5 semi-aride (plaine, steppe ou savane), sous 0,65
/// subhumide sèche ; au-delà, humide. La forêt demande un sol bien arrosé.
pub const ARID_BELOW: f32 = 0.2;
pub const SEMI_ARID_BELOW: f32 = 0.5;
pub const SUBHUMID_BELOW: f32 = 0.65;
pub const FOREST_ABOVE: f32 = 0.75;
/// Un désert de sable demande un climat qui ne gèle pas en moyenne (°C).
pub const DESERT_ABOVE: f32 = 0.0;
/// Saisons (températures moyennes de la saison, °C) : taïga ou steppe froide si la plus froide
/// passe sous `COLD_WINTER` ; climat tropical (jungle, savanes) si la plus froide reste
/// au-dessus de `TROPICAL_WINTER`. (La toundra reste définie par la moyenne annuelle : les
/// écarts entre saisons de la simulation sont plus faibles que sur Terre.)
pub const COLD_WINTER: f32 = -3.0;
pub const TROPICAL_WINTER: f32 = 18.0;
/// Jungle : climat tropical humide avec au plus `JUNGLE_DRY_SEASONS` saison sèche, ou très
/// arrosé (indice d'aridité d'au moins `JUNGLE_WET`) malgré une saison sèche plus longue. La
/// ceinture de pluies équatoriale de la simulation se déplace beaucoup avec les saisons : une
/// stricte absence de saison sèche serait presque introuvable.
pub const JUNGLE_DRY_SEASONS: u8 = 1;
pub const JUNGLE_WET: f32 = 2.0;
/// Taïga : indice d'aridité minimal (l'évaporation est faible dans le froid).
pub const TAIGA_ABOVE: f32 = 0.6;
/// Savane ou broussailles plutôt que plaine dans le semi-aride, au-dessus de cet indice d'aridité.
pub const SEMI_ARID_WOODLAND: f32 = 0.35;
/// Marais : niveau d'altitude maximal et indice d'aridité minimal.
const MARSH_MAX_LEVEL: u8 = 2;
const MARSH_ARIDITY: f32 = 1.0;

/// Climat d'une case vu par le choix du biome.
#[derive(Clone, Copy, Debug)]
pub struct BiomeClimate {
    /// Température moyenne annuelle (°C).
    pub temperature: f32,
    /// Indice d'aridité (pluie / évaporation potentielle).
    pub aridity: f32,
    /// Température de la saison la plus froide (°C).
    pub coldest: f32,
    /// Nombre de saisons sèches (0 à 4).
    pub dry_seasons: u8,
    /// Été sec et hiver pluvieux.
    pub dry_summer: bool,
}

/// Biome d'une case de terre (hors montagnes) selon son climat et ses saisons.
pub fn biome(c: BiomeClimate) -> Terrain {
    let tropical = c.coldest >= TROPICAL_WINTER;
    if c.temperature < SNOW_BELOW {
        Terrain::Snow
    } else if c.temperature < TUNDRA_BELOW {
        Terrain::Tundra
    } else if c.aridity < ARID_BELOW && c.temperature >= DESERT_ABOVE {
        Terrain::Desert
    } else if c.aridity < SEMI_ARID_BELOW {
        if c.coldest < 0.0 {
            Terrain::Steppe
        } else if tropical && c.aridity >= SEMI_ARID_WOODLAND {
            Terrain::Savanna
        } else if c.dry_summer && c.aridity >= SEMI_ARID_WOODLAND {
            Terrain::Mediterranean
        } else {
            Terrain::Plains
        }
    } else if c.coldest < COLD_WINTER {
        if c.aridity >= TAIGA_ABOVE { Terrain::Taiga } else { Terrain::Steppe }
    } else if tropical {
        if c.aridity < FOREST_ABOVE {
            Terrain::Savanna
        } else if c.dry_seasons <= JUNGLE_DRY_SEASONS || c.aridity >= JUNGLE_WET {
            Terrain::Jungle
        } else {
            Terrain::WoodedSavanna
        }
    } else if c.dry_summer && c.aridity < FOREST_ABOVE {
        // Été sec : végétation basse et clairsemée ; bien arrosé sur l'année, c'est une forêt
        Terrain::Mediterranean
    } else if c.aridity < FOREST_ABOVE {
        Terrain::Grassland
    } else {
        Terrain::Forest
    }
}
pub const HILL_MOVE_COST: u32 = 1;
/// Altitudes (m) des limites entre niveaux voisins, du pied des sommets et des plus hauts pics.
const LEVEL_BOUNDS_M: [f32; 5] = [300.0, 650.0, 1_075.0, 1_550.0, 2_100.0];
const SUMMIT_FOOT_M: f32 = 3_000.0;
const PEAK_M: f32 = 4_800.0;

/// Niveau d'altitude (1 à `MAX_LEVEL`) d'une altitude en mètres.
pub fn level_of_meters(m: f32) -> u8 {
    1 + LEVEL_BOUNDS_M.iter().filter(|&&b| m > b).count() as u8
}
/// Traverser une rivière (arête) : +2 PM, ce qui arrête la plupart des unités sur l'autre rive.
pub const RIVER_CROSSING_COST: u32 = 2;
/// Entrer dans un fleuve (à gué ou en barque) : 3 PM, l'unité s'y arrête.
pub const FLEUVE_MOVE_COST: u32 = 3;
/// Rendements d'une case de fleuve (pêche, transport et commerce).
pub const FLEUVE_YIELDS: Yields = Yields::new(2, 0, 1);
/// Nourriture en plus près de l'eau douce (rivière, fleuve, lac).
pub const FRESH_WATER_FOOD: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Terrain {
    Ocean,
    Coast,
    Grassland,
    Plains,
    Desert,
    Tundra,
    Snow,
    Forest,
    /// Lac d'eau douce (au moins une demi-case), éventuellement en altitude.
    Lake,
    /// Forêt tropicale humide : chaud toute l'année, sans saison sèche.
    Jungle,
    /// Savane herbeuse : chaud, une ou plusieurs saisons sèches.
    Savanna,
    /// Savane arborée (forêt claire, forêt de mousson) : chaud et bien arrosé, avec une saison
    /// sèche.
    WoodedSavanna,
    /// Broussailles (maquis, garrigue) : été sec, hiver doux et pluvieux.
    Mediterranean,
    /// Taïga (forêt boréale) : hiver rigoureux, été doux.
    Taiga,
    /// Steppe : semi-aride à hiver froid.
    Steppe,
    /// Marais : terre basse et plate gorgée d'eau, au bord d'un fleuve ou d'un lac.
    Marsh,
}

/// Rendement d'une case : nourriture, production, or.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Yields {
    pub food: i32,
    pub production: i32,
    pub gold: i32,
}

impl Yields {
    pub const fn new(food: i32, production: i32, gold: i32) -> Self {
        Self { food, production, gold }
    }
}

impl std::ops::AddAssign for Yields {
    fn add_assign(&mut self, o: Self) {
        self.food += o.food;
        self.production += o.production;
        self.gold += o.gold;
    }
}

impl Terrain {
    pub fn name(self) -> &'static str {
        match self {
            Terrain::Ocean => "Océan",
            Terrain::Coast => "Côte",
            Terrain::Grassland => "Prairie",
            Terrain::Plains => "Plaine",
            Terrain::Desert => "Désert",
            Terrain::Tundra => "Toundra",
            Terrain::Snow => "Neige",
            Terrain::Forest => "Forêt tempérée",
            Terrain::Lake => "Lac",
            Terrain::Jungle => "Jungle",
            Terrain::Savanna => "Savane",
            Terrain::WoodedSavanna => "Savane arborée",
            Terrain::Mediterranean => "Broussailles",
            Terrain::Taiga => "Taïga",
            Terrain::Steppe => "Steppe",
            Terrain::Marsh => "Marais",
        }
    }

    pub fn yields(self) -> Yields {
        match self {
            Terrain::Ocean => Yields::new(1, 0, 1),
            Terrain::Coast => Yields::new(2, 0, 1),
            Terrain::Grassland => Yields::new(2, 0, 0),
            Terrain::Plains => Yields::new(1, 1, 0),
            Terrain::Desert => Yields::new(0, 0, 0),
            Terrain::Tundra => Yields::new(1, 0, 0),
            Terrain::Snow => Yields::new(0, 0, 0),
            Terrain::Forest => Yields::new(1, 2, 0),
            Terrain::Lake => Yields::new(2, 0, 1),
            Terrain::Jungle => Yields::new(1, 1, 0),
            Terrain::Savanna => Yields::new(2, 0, 0),
            Terrain::WoodedSavanna => Yields::new(1, 1, 0),
            Terrain::Mediterranean => Yields::new(1, 1, 1),
            Terrain::Taiga => Yields::new(1, 1, 0),
            Terrain::Steppe => Yields::new(1, 1, 0),
            Terrain::Marsh => Yields::new(1, 0, 0),
        }
    }

    /// Coût de déplacement pour une unité terrestre ; `None` = infranchissable.
    pub fn move_cost(self) -> Option<u32> {
        match self {
            Terrain::Ocean | Terrain::Coast | Terrain::Lake => None,
            Terrain::Forest | Terrain::Jungle | Terrain::Taiga => Some(2),
            Terrain::Marsh => Some(3),
            _ => Some(1),
        }
    }

    /// Couvert boisé dense (forêt tempérée, jungle, taïga) : défense, pas de charge, tir gêné.
    pub fn is_wooded(self) -> bool {
        matches!(self, Terrain::Forest | Terrain::Jungle | Terrain::Taiga)
    }

    pub fn is_water(self) -> bool {
        matches!(self, Terrain::Ocean | Terrain::Coast | Terrain::Lake)
    }

    /// Mer (océan ou côte), au niveau 0.
    pub fn is_sea(self) -> bool {
        matches!(self, Terrain::Ocean | Terrain::Coast)
    }

    /// Couleur sRGB d'affichage.
    pub fn color(self) -> (f32, f32, f32) {
        match self {
            Terrain::Ocean => (0.10, 0.22, 0.45),
            Terrain::Coast => (0.20, 0.42, 0.65),
            Terrain::Grassland => (0.36, 0.62, 0.27),
            Terrain::Plains => (0.66, 0.66, 0.36),
            Terrain::Desert => (0.88, 0.80, 0.52),
            Terrain::Tundra => (0.55, 0.58, 0.50),
            Terrain::Snow => (0.92, 0.94, 0.97),
            Terrain::Forest => (0.15, 0.42, 0.18),
            Terrain::Lake => (0.33, 0.62, 0.82),
            Terrain::Jungle => (0.05, 0.33, 0.13),
            Terrain::Savanna => (0.78, 0.70, 0.36),
            Terrain::WoodedSavanna => (0.60, 0.62, 0.30),
            Terrain::Mediterranean => (0.56, 0.58, 0.38),
            Terrain::Taiga => (0.13, 0.31, 0.27),
            Terrain::Steppe => (0.72, 0.68, 0.50),
            Terrain::Marsh => (0.38, 0.53, 0.47),
        }
    }

    pub const ALL: [Terrain; 16] = [
        Terrain::Ocean,
        Terrain::Coast,
        Terrain::Grassland,
        Terrain::Plains,
        Terrain::Desert,
        Terrain::Tundra,
        Terrain::Snow,
        Terrain::Forest,
        Terrain::Lake,
        Terrain::Jungle,
        Terrain::Savanna,
        Terrain::WoodedSavanna,
        Terrain::Mediterranean,
        Terrain::Taiga,
        Terrain::Steppe,
        Terrain::Marsh,
    ];

    /// Nom au féminin ou au masculin pour l'accord de « vallonné ».
    fn is_feminine(self) -> bool {
        !matches!(self, Terrain::Desert | Terrain::Lake | Terrain::Marsh)
    }
}

/// Une case de fleuve qui touche au moins autant de cases de lac est intégrée au lac.
pub const FLEUVE_IN_LAKE: usize = 3;
/// Altitude (m) d'une ancienne mare de mer devenue cuvette : assez creuse pour qu'un lac
/// l'occupe toute dès que l'eau y arrive en quantité.
const POND_DEPTH_M: f32 = -60.0;
/// Passes climat → cours d'eau : à partir de la deuxième, le climat tient compte des lacs.
pub const CLIMATE_PASSES: usize = 3;

/// Carte rectangulaire stockée en coordonnées offset (odd-r).
#[derive(Clone, Debug)]
pub struct WorldMap {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Terrain>,
    /// Altitude de chaque case (0 = eau, 1 à `MAX_LEVEL` sur terre).
    levels: Vec<u8>,
    /// Relief interne à chaque case de terre (plat à pic).
    relief: Vec<Relief>,
    /// Altitude du sol (m), continue (les niveaux en sont un découpage).
    elevation: Vec<f32>,
    /// Altitude de la surface de chaque lac (m), la même pour toutes ses cases.
    lake_level_m: Vec<f32>,
    /// Climat simulé à la génération (vide pour une carte remplie à la main).
    climate: Climate,
    /// Cours d'eau et lacs (vides pour une carte remplie à la main).
    hydro: Hydrology,
    /// Échelle et latitude de la carte.
    region: Region,
    /// Bassins versants et fleuves nommés.
    basins: Basins,
    /// Tour d'est en ouest (planète) ou bords.
    wrap: Wrap,
    /// Coût d'un pas de chaque case vers chacune de ses voisines (`u8::MAX` : infranchissable),
    /// calculé à la première demande (le terrain ne change pas en cours de partie).
    step_cache: std::sync::OnceLock<Vec<[u8; 6]>>,
}

impl WorldMap {
    pub fn filled(width: i32, height: i32, terrain: Terrain) -> Self {
        let level = if terrain.is_water() { 0 } else { 1 };
        let n = (width * height) as usize;
        Self {
            width,
            height,
            tiles: vec![terrain; n],
            levels: vec![level; n],
            relief: vec![Relief::Flat; n],
            elevation: vec![if terrain.is_water() { 0.0 } else { climate::LEVEL_ALTITUDE[0] }; n],
            lake_level_m: vec![climate::LEVEL_ALTITUDE[0]; n],
            climate: Climate::default(),
            hydro: Hydrology::empty(n),
            region: Region::for_seed(0),
            basins: Basins::empty(n),
            wrap: Wrap::NONE,
            step_cache: std::sync::OnceLock::new(),
        }
    }

    fn index(&self, hex: Hex) -> Option<usize> {
        let (col, row) = self.wrap.hex(hex).to_offset();
        (col >= 0 && col < self.width && row >= 0 && row < self.height)
            .then(|| (row * self.width + col) as usize)
    }

    /// Tour d'est en ouest de la carte.
    pub fn wrap(&self) -> Wrap {
        self.wrap
    }

    /// Fait boucler (ou non) la carte d'est en ouest (tests, éditeur).
    #[cfg(test)]
    pub fn set_wraps(&mut self, wraps: bool) {
        self.wrap = Wrap::new(self.width, wraps);
        self.step_cache = std::sync::OnceLock::new();
    }

    /// La case ramenée dans la carte (d'est en ouest si la carte boucle).
    pub fn canonical(&self, hex: Hex) -> Hex {
        self.wrap.hex(hex)
    }

    /// Les 6 voisines (ramenées dans la carte ; elles peuvent être hors des bords nord et sud,
    /// ou est et ouest d'une carte qui ne boucle pas).
    pub fn neighbors(&self, hex: Hex) -> [Hex; 6] {
        self.wrap.neighbors(hex)
    }

    /// Distance en cases, par le plus court côté.
    pub fn distance(&self, a: Hex, b: Hex) -> i32 {
        self.wrap.distance(a, b)
    }

    /// Cases à distance ≤ `radius` de `center`, ramenées dans la carte (dedans ou non).
    pub fn range(&self, center: Hex, radius: i32) -> impl Iterator<Item = Hex> + use<> {
        self.wrap.range(center, radius)
    }

    pub fn contains(&self, hex: Hex) -> bool {
        self.index(hex).is_some()
    }

    pub fn get(&self, hex: Hex) -> Option<Terrain> {
        self.index(hex).map(|i| self.tiles[i])
    }

    /// Change le terrain ; la mer est ramenée au niveau 0, la terre (et les lacs) au niveau 1 au moins.
    pub fn set(&mut self, hex: Hex, terrain: Terrain) {
        self.step_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex) {
            self.tiles[i] = terrain;
            self.levels[i] = if terrain.is_sea() { 0 } else { self.levels[i].max(1) };
            if terrain.is_water() {
                self.relief[i] = Relief::Flat;
            }
            if terrain.is_water() {
                self.hydro.fleuve[i] = false;
                self.hydro.small_lake[i] = false;
            }
        }
    }

    /// Relief de la case (plat pour l'eau et hors carte).
    pub fn relief(&self, hex: Hex) -> Relief {
        self.index(hex).map_or(Relief::Flat, |i| self.relief[i])
    }

    /// Change le relief (seulement sur la terre ; un fleuve reste plat) (tests, éditeur).
    #[cfg(test)]
    pub fn set_relief(&mut self, hex: Hex, relief: Relief) {
        self.step_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex) {
            self.relief[i] = if self.tiles[i].is_water() || self.hydro.fleuve[i] { Relief::Flat } else { relief };
        }
    }

    /// La case a des collines.
    #[cfg(test)]
    pub fn hills(&self, hex: Hex) -> bool {
        self.relief(hex) == Relief::Hills
    }

    /// Ajoute ou retire les collines (tests, éditeur).
    #[cfg(test)]
    pub fn set_hills(&mut self, hex: Hex, hills: bool) {
        self.set_relief(hex, if hills { Relief::Hills } else { Relief::Flat });
    }

    /// Rendements d'une case : terrain de base, plus la production des collines et la
    /// nourriture de l'eau douce (rivière, fleuve ou lac tout proche) ; un fleuve a ses
    /// propres rendements (pêche, commerce).
    pub fn yields(&self, hex: Hex) -> Option<Yields> {
        if self.is_fleuve(hex) {
            return Some(FLEUVE_YIELDS);
        }
        let t = self.get(hex)?;
        let relief = self.relief(hex);
        let mut y = match relief {
            Relief::Peak => return Some(Yields::default()),
            Relief::Mountain => return Some(MOUNTAIN_YIELDS),
            _ => t.yields(),
        };
        match relief {
            Relief::Hills => y.production += HILL_PRODUCTION,
            Relief::Rugged => {
                y.production += HILL_PRODUCTION;
                y.food = (y.food - 1).max(0);
            }
            _ => {}
        }
        if !t.is_water() && self.fresh_water(hex) {
            // (pas sur une montagne ni un pic : sortis plus haut)
            y.food += FRESH_WATER_FOOD;
        }
        Some(y)
    }

    /// Nom complet d'une case : « Prairie vallonnée », « Désert vallonné », « Forêt »,
    /// « Fleuve (rives de prairie) », « Plaine, petit lac »…
    pub fn describe(&self, hex: Hex) -> String {
        let Some(t) = self.get(hex) else { return String::new() };
        if self.is_fleuve(hex) {
            return format!("Fleuve (rives : {})", t.name().to_lowercase());
        }
        let name = t.name();
        // Accord de l'adjectif : « vallonnée », « accidentés »…
        let agree = |adj: &str| {
            let fem = if t.is_feminine() || t == Terrain::Mediterranean { "e" } else { "" };
            let plural = if t == Terrain::Mediterranean { "s" } else { "" };
            format!("{name} {adj}{fem}{plural}")
        };
        let base = match self.relief(hex) {
            Relief::Flat => name.to_string(),
            Relief::Hills => agree("vallonné"),
            Relief::Rugged => agree("accidenté"),
            Relief::Mountain => format!("Montagne ({})", name.to_lowercase()),
            Relief::Peak => format!("Pic ({})", name.to_lowercase()),
        };
        if self.small_lake(hex) { format!("{base}, petit lac") } else { base }
    }

    // ------------------------------------------------------------ Cours d'eau

    /// Altitude du sol (m) ; 0 pour la mer.
    pub fn elevation(&self, hex: Hex) -> Option<f32> {
        self.index(hex).map(|i| if self.tiles[i].is_sea() { 0.0 } else { self.elevation[i].max(0.0) })
    }

    /// Altitude (m) de la surface d'une case d'eau (0 pour la mer) ou du sol d'une case de terre.
    pub fn surface_elevation(&self, hex: Hex) -> Option<f32> {
        let i = self.index(hex)?;
        Some(match self.tiles[i] {
            t if t.is_sea() => 0.0,
            Terrain::Lake => self.lake_level_m[i],
            _ => self.elevation[i].max(0.0),
        })
    }

    /// Change l'altitude (m) d'une case de terre, et sa classe d'altitude (tests, éditeur).
    #[cfg(test)]
    pub fn set_elevation(&mut self, hex: Hex, meters: f32) {
        self.step_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex)
            && !self.tiles[i].is_water()
        {
            self.elevation[i] = meters;
            self.levels[i] = level_of_meters(meters);
        }
    }

    /// Falaise entre deux cases voisines (écart de classes d'altitude).
    pub fn cliff_between(&self, a: Hex, b: Hex) -> bool {
        match (self.level(a), self.level(b)) {
            (Some(la), Some(lb)) if la > 0 && lb > 0 => is_cliff(la, lb),
            _ => false,
        }
    }

    /// Le cours d'eau occupe la case (fleuve).
    pub fn is_fleuve(&self, hex: Hex) -> bool {
        self.index(hex).is_some_and(|i| self.hydro.fleuve[i])
    }

    /// Petit lac dans la case.
    pub fn small_lake(&self, hex: Hex) -> bool {
        self.index(hex).is_some_and(|i| self.hydro.small_lake[i])
    }

    /// Débit de la rivière sur l'arête `dir` de la case (m³/s, 0 sans rivière).
    pub fn river_flow(&self, hex: Hex, dir: usize) -> f32 {
        self.index(hex).map_or(0.0, |i| self.hydro.edge_flow[i][dir % 6])
    }

    /// Une rivière coule sur l'arête entre deux cases voisines.
    pub fn river_between(&self, a: Hex, b: Hex) -> bool {
        self.wrap.direction(a, b).is_some_and(|dir| self.river_flow(a, dir) > 0.0)
    }

    /// Débit sortant de la case (m³/s).
    pub fn discharge(&self, hex: Hex) -> f32 {
        self.index(hex).map_or(0.0, |i| self.hydro.discharge[i])
    }

    /// Direction de la case aval.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn downstream(&self, hex: Hex) -> Option<usize> {
        self.index(hex).and_then(|i| self.hydro.downstream[i]).map(usize::from)
    }

    /// Coûts d'un pas de chaque case (indice) vers ses six voisines (`u8::MAX` : infranchissable,
    /// hors de la carte compris). Mis en cache : la recherche de chemin les consulte sans cesse.
    pub fn step_costs(&self) -> &[[u8; 6]] {
        self.step_cache.get_or_init(|| {
            (0..self.tiles.len())
                .map(|i| {
                    let h = self.hex_at(i);
                    std::array::from_fn(|d| {
                        let to = self.wrap.neighbor(h, d);
                        if !self.contains(to) {
                            return u8::MAX;
                        }
                        self.step_cost(h, to).map_or(u8::MAX, |c| c.min(254) as u8)
                    })
                })
                .collect()
        })
    }

    /// Indice d'une case dans les tableaux de la carte (la case doit être sur la carte).
    pub fn cell(&self, hex: Hex) -> usize {
        self.index(hex).expect("case hors de la carte")
    }

    /// Bassins versants et fleuves nommés.
    pub fn basins(&self) -> &Basins {
        &self.basins
    }

    /// Bassin versant d'une case de terre ou de lac.
    pub fn basin(&self, hex: Hex) -> Option<&Basin> {
        self.index(hex).and_then(|i| self.basins.basin(i))
    }

    /// Fleuve nommé qui occupe la case.
    pub fn named_river(&self, hex: Hex) -> Option<&NamedRiver> {
        self.index(hex).and_then(|i| self.basins.river(i))
    }

    /// Case d'indice donné.
    pub fn hex_at(&self, cell: usize) -> Hex {
        Hex::from_offset(cell as i32 % self.width, cell as i32 / self.width)
    }

    /// Directions où le fleuve de la case touche l'arête (bits 0 à 5).
    pub fn fleuve_links(&self, hex: Hex) -> u8 {
        self.index(hex).map_or(0, |i| self.hydro.fleuve_links[i])
    }

    /// Bilan de l'eau de la génération.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn hydrology(&self) -> &Hydrology {
        &self.hydro
    }

    /// Eau douce à portée : rivière sur une arête, petit lac, fleuve ou lac voisin.
    pub fn fresh_water(&self, hex: Hex) -> bool {
        self.small_lake(hex)
            || (0..6).any(|d| self.river_flow(hex, d) > 0.0)
            || self.neighbors(hex).iter().any(|&n| self.is_fleuve(n) || self.get(n) == Some(Terrain::Lake))
    }

    /// Fait d'une case un fleuve (tests, éditeur).
    #[cfg(test)]
    pub fn set_fleuve(&mut self, hex: Hex) {
        self.step_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex) {
            self.hydro.fleuve[i] = true;
            self.relief[i] = Relief::Flat;
        }
    }

    /// Fixe les arêtes par où passe un fleuve (tests).
    #[cfg(test)]
    pub fn set_fleuve_links(&mut self, hex: Hex, links: u8) {
        self.set_fleuve(hex);
        if let Some(i) = self.index(hex) {
            self.hydro.fleuve_links[i] = links;
        }
    }

    /// Pose une rivière sur l'arête entre une case et sa voisine `dir` (tests, éditeur).
    #[cfg(test)]
    pub fn set_river(&mut self, hex: Hex, dir: usize, flow: f32) {
        self.step_cache = std::sync::OnceLock::new();
        let other = self.wrap.neighbor(hex, dir);
        if let (Some(i), Some(j)) = (self.index(hex), self.index(other)) {
            self.hydro.edge_flow[i][dir] = flow;
            self.hydro.edge_flow[j][(dir + 3) % 6] = flow;
        }
    }

    /// Précipitations annuelles (mm).
    pub fn rain(&self, hex: Hex) -> Option<f32> {
        self.index(hex).and_then(|i| self.climate.rain.get(i).copied())
    }

    /// Température moyenne annuelle (°C).
    pub fn temperature(&self, hex: Hex) -> Option<f32> {
        self.index(hex).and_then(|i| self.climate.temperature.get(i).copied())
    }

    /// Indice d'aridité : précipitations / évaporation potentielle.
    pub fn aridity(&self, hex: Hex) -> Option<f32> {
        self.index(hex).and_then(|i| self.climate.aridity(i))
    }

    /// Climat complet de la génération (pour les rivières à venir, les diagnostics…).
    #[allow(dead_code)]
    pub fn climate(&self) -> &Climate {
        &self.climate
    }

    /// Vent moyen (x vers l'est, y vers le nord).
    pub fn wind(&self, hex: Hex) -> Option<(f32, f32)> {
        self.index(hex).and_then(|i| self.climate.wind.get(i).copied())
    }

    /// Climat en mots, avec température et pluie : « tempéré humide, 11 °C, 850 mm/an ».
    /// Climat d'une case de terre en deux mots : « tempéré humide », « chaud aride »…
    pub fn climate_zone(&self, hex: Hex) -> Option<String> {
        let name = self.climate_name(hex)?;
        name.split(',').next().map(str::to_string)
    }

    pub fn climate_name(&self, hex: Hex) -> Option<String> {
        let (t, r, ai) = (self.temperature(hex)?, self.rain(hex)?, self.aridity(hex)?);
        if self.get(hex)?.is_water() {
            return None;
        }
        let zone = if t >= 22.0 {
            "tropical"
        } else if t >= 14.0 {
            "chaud"
        } else if t >= 2.0 {
            "tempéré"
        } else {
            "froid"
        };
        let wet = if ai < ARID_BELOW {
            "aride"
        } else if ai < SEMI_ARID_BELOW {
            "semi-aride"
        } else if ai < SUBHUMID_BELOW {
            "subhumide"
        } else {
            "humide"
        };
        let lat = self.latitude(hex);
        let hemisphere = if lat >= 0.0 { "N" } else { "S" };
        // Saisons : été sec (méditerranéen) ou nombre de saisons sèches
        let i = self.index(hex)?;
        let dry = self.climate.dry_seasons(i).map_or(0, |d| d.iter().filter(|&&x| x).count());
        let seasons = match (self.climate.dry_summer(i).unwrap_or(false), dry) {
            (true, _) => " à été sec".to_string(),
            // (sec toute l'année : déjà dit par « aride » ou « semi-aride »)
            (false, 0 | 4) => String::new(),
            (false, 1) => " à une saison sèche".to_string(),
            (false, k) => format!(" à {k} saisons sèches"),
        };
        let wet = format!("{wet}{seasons}");
        Some(format!("{zone} {wet}, {t:.0} °C, {:.0} mm/an, {:.0}°{hemisphere}", (r / 10.0).round() * 10.0, lat.abs()))
    }

    /// Altitude d'une case (0 = eau).
    pub fn level(&self, hex: Hex) -> Option<u8> {
        self.index(hex).map(|i| self.levels[i])
    }

    /// Tous les changements de niveau entre cases de terre voisines, chacun une seule fois.
    pub fn level_edges(&self) -> Vec<LevelEdge> {
        let land = |h: Hex| self.get(h).filter(|t| !t.is_water() && self.relief(h) < Relief::Mountain).and(self.level(h));
        let mut edges = Vec::new();
        for high in self.hexes() {
            let Some(hl) = land(high) else { continue };
            for (dir, d) in crate::hex::DIRECTIONS.iter().enumerate() {
                if let Some(ll) = land(high + *d)
                    && hl > ll
                {
                    edges.push(LevelEdge { high, dir, diff: hl - ll });
                }
            }
        }
        edges
    }

    /// Change la classe d'altitude d'une case de terre (bornée entre 1 et `MAX_LEVEL`), et son
    /// altitude (m) en conséquence (altitude moyenne de la classe) (tests, éditeur).
    #[cfg(test)]
    pub fn set_level(&mut self, hex: Hex, level: u8) {
        self.step_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex)
            && !self.tiles[i].is_water()
        {
            self.levels[i] = level.clamp(1, MAX_LEVEL);
            self.elevation[i] = climate::LEVEL_ALTITUDE[(self.levels[i] - 1) as usize];
        }
    }

    pub fn hexes(&self) -> impl Iterator<Item = Hex> + '_ {
        (0..self.height).flat_map(move |row| (0..self.width).map(move |col| Hex::from_offset(col, row)))
    }

    /// Génère un monde : continents (bruit fractal + atténuation sur les bords), altitude en
    /// 6 niveaux de la côte aux massifs, relief accidenté (collines), climat selon la latitude,
    /// l'altitude et l'humidité.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn generate(width: i32, height: i32, seed: u32) -> Self {
        Self::generate_in(width, height, seed, Region::for_seed(seed))
    }

    /// Latitude d'une case (degrés, positive au nord).
    pub fn latitude(&self, hex: Hex) -> f32 {
        self.region.latitude_deg(hex.to_offset().1, self.height)
    }

    /// Génère un monde dans une région donnée (taille des cases, latitude).
    pub fn generate_in(width: i32, height: i32, seed: u32, region: Region) -> Self {
        let elevation_noise: Fbm<Perlin> = Fbm::new(seed).set_octaves(5).set_frequency(1.0);
        let moisture_noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(7919)).set_octaves(4).set_frequency(1.0);
        let rough_noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(104_729)).set_octaves(3).set_frequency(1.0);
        let mut map = Self::filled(width, height, Terrain::Ocean);
        map.wrap = Wrap::new(width, region.wraps);
        let wrap = map.wrap;
        let scale = 4.0 / width as f64;
        // Bruit lu sur un cylindre quand la carte boucle : pas de couture entre les bords est et
        // ouest (même échelle qu'à plat : le tour fait `width` colonnes)
        let radius = width as f64 * scale / std::f64::consts::TAU;
        let sample = |noise: &Fbm<Perlin>, h: Hex, zoom: f64| {
            let (col, row) = h.to_offset();
            if wrap.wraps() {
                let a = col as f64 / width as f64 * std::f64::consts::TAU;
                noise.get([a.cos() * radius * zoom, a.sin() * radius * zoom, row as f64 * scale * zoom])
            } else {
                noise.get([col as f64 * scale * zoom, row as f64 * scale * zoom])
            }
        };
        // 1) Élévation brute
        let elevations: Vec<(Hex, f64)> = map
            .hexes()
            .map(|h| {
                let (col, row) = h.to_offset();
                let mut e = sample(&elevation_noise, h, 1.0);
                // Atténuation vers les bords pour obtenir des continents entourés d'océan
                let nx = col as f64 / (width - 1) as f64 * 2.0 - 1.0;
                let ny = row as f64 / (height - 1) as f64 * 2.0 - 1.0;
                // (une planète n'a de bords qu'aux pôles)
                let edge = if wrap.wraps() { ny.abs() } else { nx.abs().max(ny.abs()) };
                e -= (edge.powi(3)) * 0.9;
                (h, e)
            })
            .collect();
        // 2) Niveau de la mer par percentile (~58 % d'eau), puis niveaux de la terre par
        //    percentile des terres émergées ; les sommets sont le haut du niveau 6.
        let mut sorted: Vec<f64> = elevations.iter().map(|(_, e)| *e).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let sea = sorted[((sorted.len() - 1) as f64 * 0.58) as usize];
        let land_sorted: Vec<f64> = sorted.iter().copied().filter(|&e| e >= sea).collect();
        // Mares de mer : petites étendues de mer (moins de `MIN_INLAND_SEA_CELLS` cases) enfermées
        // dans les terres. Une case de mer isolée qui avalerait de grands fleuves n'a pas de sens :
        // elles deviennent des cuvettes de terre, que l'hydrologie remplit (lac qui déborde vers
        // la vraie mer, ou cuvette sèche en climat aride)
        let ponds: std::collections::HashSet<Hex> = {
            let wet = |h: Hex| map.index(h).is_some_and(|i| elevations[i].1 < sea);
            let mut seen = std::collections::HashSet::new();
            let mut ponds = std::collections::HashSet::new();
            for &(start, e) in &elevations {
                if e >= sea || seen.contains(&start) {
                    continue;
                }
                let mut group = vec![start];
                seen.insert(start);
                let mut k = 0;
                let mut border = false;
                while k < group.len() && group.len() < crate::basins::MIN_INLAND_SEA_CELLS {
                    let h = group[k];
                    let (c, r) = h.to_offset();
                    border |= r == 0 || r == height - 1 || (!wrap.wraps() && (c == 0 || c == width - 1));
                    for nb in wrap.neighbors(h) {
                        if wet(nb) && seen.insert(nb) {
                            group.push(nb);
                        }
                    }
                    k += 1;
                }
                // Groupe exploré en entier (pas de frontière d'exploration en attente) et petit
                if !border && k == group.len() && group.len() < crate::basins::MIN_INLAND_SEA_CELLS {
                    ponds.extend(group);
                } else {
                    // Le reste du groupe (grand) : on finit de le marquer
                    while k < group.len() {
                        for nb in wrap.neighbors(group[k]) {
                            if wet(nb) && seen.insert(nb) {
                                group.push(nb);
                            }
                        }
                        k += 1;
                    }
                }
            }
            ponds
        };
        let is_sea = |h: Hex, e: f64| e < sea && !ponds.contains(&h);
        let land_pct = |p: f64| land_sorted[((land_sorted.len() - 1) as f64 * p) as usize];
        // Bornes hautes des niveaux 1 à 5 ; au-delà : niveau 6
        let bands: Vec<f64> = LEVEL_BANDS.iter().map(|&p| land_pct(p)).collect();
        let summits = land_pct(MOUNTAIN_PERCENTILE);
        let level_of = |e: f64| 1 + bands.iter().filter(|&&b| e > b).count() as u8;
        // Altitude continue (m) : interpolation entre les bornes des niveaux (limites entre les
        // altitudes moyennes de deux niveaux voisins), jusqu'aux sommets
        let top = land_sorted.last().copied().unwrap_or(sea + 1.0);
        let mut knots: Vec<(f64, f32)> = vec![(sea, 0.0)];
        knots.extend(bands.iter().zip(LEVEL_BOUNDS_M).map(|(&b, m)| (b, m)));
        knots.push((summits, SUMMIT_FOOT_M));
        knots.push((top.max(summits + 1e-6), PEAK_M));
        let meters = |e: f64| {
            let k = knots.windows(2).find(|w| e <= w[1].0).unwrap_or(&knots[knots.len() - 2..]);
            let ((e0, m0), (e1, m1)) = (k[0], k[1]);
            let t = if e1 > e0 { ((e - e0) / (e1 - e0)).clamp(0.0, 1.0) as f32 } else { 1.0 };
            m0 + (m1 - m0) * t
        };
        let land_levels: std::collections::HashMap<Hex, u8> =
            elevations.iter().filter(|&&(h, e)| !is_sea(h, e)).map(|&(h, e)| (h, if ponds.contains(&h) { 1 } else { level_of(e) })).collect();
        // 3) Altitude (m) et classe d'altitude de chaque case de terre
        let mut surface = vec![climate::Surface::Water; (width * height) as usize];
        for &(h, e) in &elevations {
            if is_sea(h, e) {
                continue; // reste océan
            }
            let i = map.index(h).unwrap();
            map.elevation[i] = if ponds.contains(&h) { POND_DEPTH_M } else { meters(e) };
            surface[i] = climate::Surface::Land(map.elevation[i].max(0.0));
            map.tiles[i] = Terrain::Plains;
            map.levels[i] = land_levels[&h];
        }
        // Relief de chaque case : note d'escarpement = dénivelée avec les voisines (rangée parmi
        // les terres), bruit propre et altitude ; les cases les mieux notées sont montagnes, puis
        // relief accidenté, puis collines. Les pics sont les plus hauts sommets locaux des montagnes.
        let land_cells: Vec<usize> = (0..map.tiles.len()).filter(|&i| !map.tiles[i].is_water()).collect();
        let hex_of = |i: usize| Hex::from_offset(i as i32 % width, i as i32 / width);
        let local: Vec<f32> = land_cells
            .iter()
            .map(|&i| {
                wrap.neighbors(hex_of(i))
                    .iter()
                    .filter_map(|&nb| map.index(nb))
                    .filter(|&j| !map.tiles[j].is_water())
                    .map(|j| (map.elevation[i] - map.elevation[j]).abs())
                    .fold(0.0, f32::max)
            })
            .collect();
        let rank = |values: &[f32]| -> Vec<f32> {
            let mut order: Vec<usize> = (0..values.len()).collect();
            order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
            let mut r = vec![0.0; values.len()];
            for (k, &o) in order.iter().enumerate() {
                r[o] = k as f32 / values.len().max(1) as f32;
            }
            r
        };
        let rough: Vec<f32> = land_cells.iter().map(|&i| sample(&rough_noise, hex_of(i), 2.5) as f32).collect();
        let (local_rank, rough_rank) = (rank(&local), rank(&rough));
        let score: Vec<f32> = (0..land_cells.len())
            .map(|k| 0.45 * local_rank[k] + 0.3 * rough_rank[k] + 0.35 * (map.elevation[land_cells[k]] / PEAK_M).clamp(0.0, 1.0))
            .collect();
        let mut by_score: Vec<usize> = (0..land_cells.len()).filter(|&k| !ponds.contains(&hex_of(land_cells[k]))).collect();
        by_score.sort_by(|&a, &b| score[b].total_cmp(&score[a]));
        let count = |share: f64| (land_cells.len() as f64 * share).round() as usize;
        let (mountains, rugged, hills) = (count(MOUNTAIN_SHARE), count(RUGGED_SHARE), count(HILL_SHARE));
        for (rank, &k) in by_score.iter().enumerate() {
            let i = land_cells[k];
            map.relief[i] = if rank < mountains {
                if map.elevation[i] >= MOUNTAIN_MIN_M { Relief::Mountain } else { Relief::Rugged }
            } else if rank < mountains + rugged {
                Relief::Rugged
            } else if rank < mountains + rugged + hills {
                Relief::Hills
            } else {
                Relief::Flat
            };
        }
        // Pics : sommets locaux des montagnes, du plus haut au plus bas, jamais côte à côte
        let mut summits_local: Vec<usize> = land_cells
            .iter()
            .copied()
            .filter(|&i| map.relief[i] == Relief::Mountain && map.elevation[i] >= PEAK_MIN_M)
            .filter(|&i| {
                wrap.neighbors(hex_of(i)).iter().filter_map(|&nb| map.index(nb)).all(|j| map.elevation[j] <= map.elevation[i] + PEAK_TOLERANCE_M)
            })
            .collect();
        summits_local.sort_by(|&a, &b| map.elevation[b].total_cmp(&map.elevation[a]));
        let mut peaks = 0;
        for i in summits_local {
            if peaks >= count(PEAK_SHARE) {
                break;
            }
            if wrap.neighbors(hex_of(i)).iter().filter_map(|&nb| map.index(nb)).any(|j| map.relief[j] == Relief::Peak) {
                continue;
            }
            map.relief[i] = Relief::Peak;
            peaks += 1;
        }
        let relief_base = map.relief.clone();
        map.region = region;
        // 4) à 6) Climat, terrain, cours d'eau et lacs, en `CLIMATE_PASSES` passes : à partir de la
        //    deuxième, le climat voit les lacs de la passe précédente (ils évaporent et humidifient
        //    l'air), ce qui change la pluie, le ruissellement, donc les lacs eux-mêmes
        for pass in 0..CLIMATE_PASSES {
            let mut surface = surface.clone();
            if pass > 0 {
                for (i, s) in surface.iter_mut().enumerate() {
                    if map.tiles[i] == Terrain::Lake {
                        *s = climate::Surface::Lake(climate::altitude(map.levels[i], false));
                    }
                }
                // Terrain d'avant les lacs (classes d'altitude, relief) : la passe le recalcule
                for &(h, e) in &elevations {
                    if is_sea(h, e) {
                        continue;
                    }
                    let i = map.index(h).unwrap();
                    map.tiles[i] = Terrain::Plains;
                    map.levels[i] = land_levels[&h];
                }
                map.relief.clone_from(&relief_base);
                map.step_cache = std::sync::OnceLock::new();
            }
            map.climate = climate::simulate(width, height, &surface, seed, &region);
            // 4) Climat : pression, vents, humidité et pluie (voir climate.rs), puis terrain selon
            //    la température et les précipitations, avec un peu de bruit pour casser les contours.
            for &(h, e) in &elevations {
                if is_sea(h, e) {
                    continue;
                }
                let i = map.index(h).unwrap();
                let temperature = map.climate.temperature[i];
                // Petite variation locale (sols, exposition) pour casser les contours
                let jitter = 1.0 + sample(&moisture_noise, h, 3.0) as f32 * 0.25;
                let aridity = map.climate.aridity(i).unwrap_or(1.0) * jitter;
                let (coldest, _) = map.climate.coldest_warmest(i).unwrap_or((temperature, temperature));
                let dry = map.climate.dry_seasons(i).unwrap_or([false; 4]);
                let t = biome(BiomeClimate {
                    temperature,
                    aridity,
                    coldest,
                    dry_seasons: dry.iter().filter(|&&d| d).count() as u8,
                    dry_summer: map.climate.dry_summer(i).unwrap_or(false),
                });
                map.set(h, t);
            }
            // 5) Les eaux adjacentes à la terre deviennent des côtes
            let coasts: Vec<Hex> = map
                .hexes()
                .filter(|&h| map.get(h) == Some(Terrain::Ocean))
                .filter(|&h| map.neighbors(h).iter().any(|&n| map.get(n).is_some_and(|t| !t.is_water())))
                .collect();
            for h in coasts {
                map.set(h, Terrain::Coast);
            }
            // 6) Cours d'eau et lacs (voir hydro.rs)
            map.add_hydrology();
            map.add_marshes();
        }
        map
    }

    /// Écoulement de l'eau sur la carte générée : lacs (cases d'eau), petits lacs, fleuves,
    /// rivières sur les arêtes.
    fn add_hydrology(&mut self) {
        let n = self.tiles.len();
        if self.climate.runoff.len() != n {
            return;
        }
        let sea: Vec<bool> = self.tiles.iter().map(|t| t.is_sea()).collect();
        let aridity: Vec<f32> = (0..n).map(|i| self.climate.aridity(i).unwrap_or(1.0)).collect();
        let hydro = hydro::simulate(&hydro::HydroInput {
            width: self.width,
            height: self.height,
            cell_km: self.region.cell_km,
            elevation: &self.elevation,
            sea: &sea,
            runoff: &self.climate.runoff,
            rain: &self.climate.rain,
            pet: &self.climate.pet,
            aridity: &aridity,
            wraps: self.wrap.wraps(),
        });
        let mut hydro = hydro;
        // Un fleuve presque entouré par un lac (au moins `FLEUVE_IN_LAKE` voisines de lac) en fait
        // partie : il traverse le lac au lieu de le découper (de proche en proche)
        loop {
            let swallowed: Vec<usize> = (0..n)
                .filter(|&i| hydro.fleuve[i])
                .filter(|&i| {
                    let hex = Hex::from_offset(i as i32 % self.width, i as i32 / self.width);
                    self.neighbors(hex).iter().filter(|&&nb| self.index(nb).is_some_and(|j| hydro.lake[j])).count() >= FLEUVE_IN_LAKE
                })
                .collect();
            if swallowed.is_empty() {
                break;
            }
            for i in swallowed {
                // Surface du lac voisin (le niveau du lac n'en est pas changé)
                let hex = Hex::from_offset(i as i32 % self.width, i as i32 / self.width);
                hydro.lake_surface[i] = self
                    .neighbors(hex)
                    .iter()
                    .filter_map(|&nb| self.index(nb))
                    .filter(|&j| hydro.lake[j])
                    .map(|j| hydro.lake_surface[j])
                    .fold(f32::MIN, f32::max);
                hydro.fleuve[i] = false;
                hydro.fleuve_links[i] = 0;
                hydro.lake[i] = true;
                hydro.small_lake[i] = false;
            }
        }
        let lakes: Vec<usize> = (0..n).filter(|&i| hydro.lake[i]).collect();
        for &i in &lakes {
            self.tiles[i] = Terrain::Lake;
            self.relief[i] = Relief::Flat;
        }
        // Niveau d'un lac : celui de sa surface, sans dépasser ses rives ; le même pour toutes les
        // cases d'eau qui se touchent (une seule surface plane)
        let hex_of = |i: usize| Hex::from_offset(i as i32 % self.width, i as i32 / self.width);
        let wrap = self.wrap;
        let mut done = vec![false; n];
        for &start in &lakes {
            if done[start] {
                continue;
            }
            let mut group = vec![start];
            done[start] = true;
            let mut k = 0;
            while k < group.len() {
                let i = group[k];
                k += 1;
                for nb in wrap.neighbors(hex_of(i)) {
                    if let Some(j) = self.index(nb)
                        && self.tiles[j] == Terrain::Lake
                        && !done[j]
                    {
                        done[j] = true;
                        group.push(j);
                    }
                }
            }
            let level = group
                .iter()
                .map(|&i| {
                    let shore = wrap
                        .neighbors(hex_of(i))
                        .iter()
                        .filter_map(|&nb| self.index(nb))
                        .filter(|&j| !self.tiles[j].is_water())
                        .map(|j| self.levels[j])
                        .min()
                        .unwrap_or(MAX_LEVEL);
                    level_of_meters(hydro.lake_surface[i]).min(shore)
                })
                .min()
                .unwrap_or(1)
                .max(1);
            // Surface en mètres (vue 3D) : la plus basse du groupe, sous ses rives
            let surface = group
                .iter()
                .map(|&i| {
                    let shore = wrap
                        .neighbors(hex_of(i))
                        .iter()
                        .filter_map(|&nb| self.index(nb))
                        .filter(|&j| !self.tiles[j].is_water())
                        .map(|j| self.elevation[j].max(0.0))
                        .fold(f32::MAX, f32::min);
                    hydro.lake_surface[i].min(shore)
                })
                .fold(f32::MAX, f32::min)
                .max(0.0);
            for &i in &group {
                self.levels[i] = level;
                self.lake_level_m[i] = surface;
            }
        }
        for i in 0..n {
            if hydro.fleuve[i] {
                // Un fleuve coule dans une vallée : sa case est plate
                self.relief[i] = Relief::Flat;
            }
        }
        self.hydro = hydro;
        self.add_basins();
    }

    /// Marais (après l'hydrologie) : case de terre basse et plate (aucune voisine plus basse,
    /// pas de collines), au bord d'un fleuve ou d'un lac (ou avec un petit lac), dans un climat
    /// humide qui ne gèle pas toute l'année.
    fn add_marshes(&mut self) {
        let marsh: Vec<usize> = (0..self.tiles.len())
            .filter(|&i| {
                let hex = Hex::from_offset(i as i32 % self.width, i as i32 / self.width);
                let t = self.tiles[i];
                if t.is_water() || matches!(t, Terrain::Snow | Terrain::Tundra | Terrain::Desert) {
                    return false;
                }
                if self.relief[i] != Relief::Flat || self.is_fleuve(hex) || self.levels[i] > MARSH_MAX_LEVEL {
                    return false;
                }
                let wet = self.climate.aridity(i).is_some_and(|a| a >= MARSH_ARIDITY) && self.climate.temperature.get(i).is_some_and(|&t| t > 0.0);
                if !wet {
                    return false;
                }
                let lower = self.neighbors(hex).iter().any(|&n| {
                    self.index(n).is_some_and(|j| !self.tiles[j].is_water() && self.levels[j] < self.levels[i])
                });
                let water = self.small_lake(hex) || self.neighbors(hex).iter().any(|&n| self.is_fleuve(n) || self.get(n) == Some(Terrain::Lake));
                !lower && water
            })
            .collect();
        for i in marsh {
            self.tiles[i] = Terrain::Marsh;
        }
        self.step_cache = std::sync::OnceLock::new();
    }

    /// Bassins versants et noms des fleuves (après l'hydrologie).
    fn add_basins(&mut self) {
        let sea: Vec<bool> = self.tiles.iter().map(|t| t.is_sea()).collect();
        let (region, height) = (self.region, self.height);
        let latitude = move |row: i32| region.latitude_deg(row, height);
        self.basins = basins::compute(&basins::BasinInput {
            width: self.width,
            height: self.height,
            downstream: &self.hydro.downstream,
            discharge: &self.hydro.discharge,
            fleuve: &self.hydro.fleuve,
            sea: &sea,
            lake: &self.hydro.lake,
            segments: &self.hydro.river_segments,
            corner_outlet: &self.hydro.corner_outlet,
            corner_cells: &self.hydro.corner_cells,
            latitude: &latitude,
            wrap: self.wrap,
        });
    }

    /// Note d'un emplacement de départ : somme des rendements autour, terrain constructible.
    pub fn start_score(&self, hex: Hex) -> i32 {
        match self.get(hex) {
            Some(Terrain::Grassland | Terrain::Plains | Terrain::Savanna | Terrain::Steppe | Terrain::Mediterranean) if !self.is_fleuve(hex) => {}
            _ => return i32::MIN,
        }
        self.range(hex, 2)
            .filter_map(|h| self.yields(h))
            .map(|y| y.food * 3 + y.production * 2 + y.gold)
            .sum()
    }

    /// Meilleur emplacement de départ, en privilégiant le centre de la carte.
    pub fn find_start(&self) -> Option<Hex> {
        let center = Hex::from_offset(self.width / 2, self.height / 2);
        self.hexes()
            .filter(|&h| self.start_score(h) > i32::MIN)
            .max_by_key(|&h| self.start_score(h) * 4 - self.distance(h, center) * 3)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn biomes_selon_les_saisons() {
        let c = |temperature: f32, aridity: f32, coldest: f32, dry_seasons: u8, dry_summer: bool| {
            biome(BiomeClimate { temperature, aridity, coldest, dry_seasons, dry_summer })
        };
        assert_eq!(c(-20.0, 1.0, -30.0, 0, false), Terrain::Snow);
        assert_eq!(c(-8.0, 1.0, -20.0, 0, false), Terrain::Tundra);
        assert_eq!(c(2.0, 1.0, -10.0, 0, false), Terrain::Taiga, "hiver rigoureux, humide");
        assert_eq!(c(2.0, 0.55, -10.0, 0, false), Terrain::Steppe, "hiver rigoureux, assez sec");
        assert_eq!(c(5.0, 0.4, -5.0, 2, false), Terrain::Steppe, "semi-aride à hiver froid");
        assert_eq!(c(25.0, 0.1, 22.0, 4, false), Terrain::Desert);
        assert_eq!(c(26.0, 1.2, 24.0, 0, false), Terrain::Jungle);
        assert_eq!(c(26.0, 1.2, 24.0, 1, false), Terrain::Jungle, "une courte saison sèche");
        assert_eq!(c(26.0, 2.5, 24.0, 3, false), Terrain::Jungle, "très arrosé");
        assert_eq!(c(26.0, 1.2, 24.0, 2, false), Terrain::WoodedSavanna);
        assert_eq!(c(26.0, 0.6, 24.0, 2, false), Terrain::Savanna);
        assert_eq!(c(26.0, 0.4, 24.0, 3, false), Terrain::Savanna, "savane sèche");
        assert_eq!(c(15.0, 0.6, 8.0, 1, true), Terrain::Mediterranean);
        assert_eq!(c(15.0, 0.9, 8.0, 1, true), Terrain::Forest, "été sec mais bien arrosé : forêt");
        assert_eq!(c(15.0, 0.4, 8.0, 2, true), Terrain::Mediterranean, "broussailles semi-arides");
        assert_eq!(c(15.0, 0.4, 8.0, 2, false), Terrain::Plains);
        assert_eq!(c(10.0, 0.6, 2.0, 0, false), Terrain::Grassland);
        assert_eq!(c(10.0, 1.0, 2.0, 0, false), Terrain::Forest);
    }

    #[test]
    fn pas_de_mare_de_mer() {
        // Aucune étendue de mer de moins de `MIN_INLAND_SEA_CELLS` cases enfermée dans les terres
        for seed in [2, 777] {
            let m = WorldMap::generate_in(128, 74, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 74));
            let mut seen = std::collections::HashSet::new();
            for start in m.hexes().filter(|&h| m.get(h).is_some_and(|t| t.is_sea())) {
                if !seen.insert(start) {
                    continue;
                }
                let mut group = vec![start];
                let mut k = 0;
                while k < group.len() {
                    for nb in m.neighbors(group[k]) {
                        if m.get(nb).is_some_and(|t| t.is_sea()) && seen.insert(nb) {
                            group.push(nb);
                        }
                    }
                    k += 1;
                }
                let border = group.iter().any(|h| {
                    let (c, r) = h.to_offset();
                    let _ = c;
                    r == 0 || r == m.height - 1
                });
                assert!(border || group.len() >= crate::basins::MIN_INLAND_SEA_CELLS, "graine {seed} : {:?}", group[0].to_offset());
            }
        }
    }
    use super::*;

    #[test]
    fn generation_deterministe_et_variee() {
        let a = WorldMap::generate(60, 40, 42);
        let b = WorldMap::generate(60, 40, 42);
        assert!(a.hexes().all(|h| a.get(h) == b.get(h)));

        let land = a.hexes().filter(|&h| !a.get(h).unwrap().is_water()).count();
        let total = (60 * 40) as usize;
        assert!(land > total / 4 && land < total * 3 / 5, "terres : {land}/{total}");
    }

    #[test]
    fn depart_constructible() {
        for seed in 0..10 {
            let map = WorldMap::generate(60, 40, seed);
            let start = map.find_start().expect("un départ doit exister");
            assert!(matches!(map.get(start), Some(Terrain::Grassland | Terrain::Plains | Terrain::Savanna | Terrain::Steppe | Terrain::Mediterranean)));
        }
    }

    #[test]
    fn hors_carte() {
        let map = WorldMap::filled(10, 10, Terrain::Plains);
        assert!(map.get(Hex::from_offset(-1, 0)).is_none());
        assert!(map.get(Hex::from_offset(9, 9)).is_some());
        assert_eq!(map.hexes().count(), 100);
    }

    #[test]
    fn planete_sans_bord_est_ouest() {
        let mut map = WorldMap::filled(20, 8, Terrain::Plains);
        map.set_wraps(true);
        // Au-delà du bord est : la colonne 0 ; au-delà des pôles : rien
        assert_eq!(map.get(Hex::from_offset(20, 3)), Some(Terrain::Plains));
        assert!(map.get(Hex::from_offset(3, -1)).is_none());
        map.set(Hex::from_offset(0, 3), Terrain::Desert);
        assert_eq!(map.get(Hex::from_offset(20, 3)), Some(Terrain::Desert));
        // Chaque case a ses 6 voisines franchissables, même au bord est
        let costs = map.step_costs();
        let east = Hex::from_offset(19, 4);
        assert!(costs[map.cell(east)].iter().all(|&c| c != u8::MAX));
        // Le plus court chemin passe par le raccord
        let (a, b) = (Hex::from_offset(1, 4), Hex::from_offset(18, 4));
        let path = crate::pathfinding::find_path(&map, a, b).unwrap();
        assert_eq!(path.len(), 3, "{path:?}");
        assert!(path.iter().all(|&h| map.contains(h) && map.canonical(h) == h));
        assert_eq!(map.distance(a, b), 3);
        // Une rivière posée sur une arête du raccord se voit des deux côtés
        map.set_river(east, 0, 50.0);
        let west = map.neighbors(east)[0];
        assert_eq!(west.to_offset().0, 0);
        assert!(map.river_between(east, west) && map.river_between(west, east));
    }

    #[test]
    fn planete_generee_sans_couture() {
        let m = WorldMap::generate_in(128, 74, 5, crate::climate::Region::planet(crate::climate::CELL_KM, 74));
        assert!(m.wrap().wraps());
        // De part et d'autre du raccord, l'altitude varie comme entre deux colonnes voisines
        let gap = |c0: i32, c1: i32| -> f32 {
            (5..69)
                .map(|r| {
                    let (a, b) = (Hex::from_offset(c0, r), Hex::from_offset(c1, r));
                    (m.elevation(a).unwrap().max(0.0) - m.elevation(b).unwrap().max(0.0)).abs()
                })
                .sum::<f32>()
                / 64.0
        };
        let seam = gap(127, 0);
        let inside = (1..127).map(|c| gap(c - 1, c)).sum::<f32>() / 126.0;
        assert!(seam < inside * 3.0, "couture : {seam} m contre {inside} m en moyenne");
        // Des terres touchent le raccord (plus d'océan forcé à l'est et à l'ouest)
        assert!((0..74).any(|r| !m.get(Hex::from_offset(0, r)).unwrap().is_water() || !m.get(Hex::from_offset(127, r)).unwrap().is_water()));
    }

    #[test]
    fn altitude_et_falaises() {
        for seed in 0..5 {
            let map = WorldMap::generate(64, 40, seed);
            let mut used = [false; MAX_LEVEL as usize + 1];
            let (mut edges, mut cliffs) = (0, 0);
            for h in map.hexes() {
                let (t, l) = (map.get(h).unwrap(), map.level(h).unwrap());
                assert_eq!(t.is_sea(), l == 0, "mer au niveau 0, terre et lacs au-dessus");
                used[l as usize] = true;
                for n in h.neighbors() {
                    if let Some(m) = map.level(n).filter(|&m| m > 0 && l > 0) {
                        edges += 1;
                        cliffs += usize::from(is_cliff(l, m));
                    }
                }
            }
            assert!(used.iter().all(|&u| u), "graine {seed} : tous les niveaux sont présents");
            assert!(cliffs * 100 < edges * 5, "graine {seed} : falaises rares ({cliffs}/{edges})");
            // Presque tout le continent de départ reste accessible malgré les falaises
            let start = map.find_start().unwrap();
            let mut land = HashSet::from([start]);
            let mut stack = vec![start];
            while let Some(c) = stack.pop() {
                for n in c.neighbors() {
                    if map.move_cost(n).is_some() && land.insert(n) {
                        stack.push(n);
                    }
                }
            }
            let reach = crate::pathfinding::reachable(&map, start, u32::MAX).len() + 1;
            assert!(reach * 100 >= land.len() * 95, "graine {seed} : {reach}/{}", land.len());
        }
    }

    #[test]
    fn pente() {
        assert_eq!(climb_cost(450.0, 800.0), 0, "pente douce");
        assert_eq!(climb_cost(800.0, 450.0), 0);
        assert_eq!(climb_cost(450.0, 1_100.0), 1, "montée raide");
        assert_eq!(climb_cost(450.0, 1_500.0), 2, "très raide");
        assert_eq!(climb_cost(1_500.0, 450.0), 0, "descente gratuite");
        assert!(is_cliff(1, 4), "falaise");
        assert!(is_cliff(5, 2), "falaise dans les deux sens");
        assert!(!is_cliff(2, 4));
        let mut map = WorldMap::filled(5, 5, Terrain::Plains);
        let (a, b) = (Hex::from_offset(1, 1), Hex::from_offset(2, 1));
        map.set_level(b, 3);
        assert_eq!(map.step_cost(a, b), Some(2), "1 PM + 1 de montée");
        assert_eq!(map.step_cost(b, a), Some(1));
        map.set_level(b, 4);
        assert_eq!(map.step_cost(a, b), None);
        // En mètres : 150 → 500 m gratuit, 150 → 1 000 m +1 (même classe d'écart < 3)
        map.set_elevation(b, 500.0);
        assert_eq!(map.step_cost(a, b), Some(1));
        map.set_elevation(b, 1_000.0);
        assert_eq!(map.step_cost(a, b), Some(2));
    }

    #[test]
    fn collines_sur_tout_terrain() {
        let map = WorldMap::generate(64, 40, 3);
        let land: Vec<Hex> = map.hexes().filter(|&h| map.move_cost(h).is_some()).collect();
        let hills = land.iter().filter(|&&h| map.hills(h)).count();
        assert!(hills * 100 > land.len() * 8 && hills * 100 < land.len() * 25, "{hills}/{}", land.len());
        assert!(land.iter().any(|&h| map.hills(h) && map.get(h).is_some_and(|t| t.is_wooded())), "forêts vallonnées");
        assert!(map.hexes().filter(|&h| map.hills(h)).all(|h| map.get(h).unwrap().move_cost().is_some()));
        // Rendements et coûts
        let mut m = WorldMap::filled(4, 4, Terrain::Forest);
        let h = Hex::from_offset(1, 1);
        m.set_hills(h, true);
        assert_eq!(m.yields(h), Some(Yields::new(1, 3, 0)));
        assert_eq!(m.move_cost(h), Some(3));
        assert_eq!(m.describe(h), "Forêt tempérée vallonnée");
        m.set_relief(h, Relief::Mountain);
        assert_eq!(m.move_cost(h), Some(MOUNTAIN_MOVE_COST), "montagne franchissable");
        assert_eq!(m.yields(h), Some(MOUNTAIN_YIELDS));
        assert_eq!(m.describe(h), "Montagne (forêt tempérée)");
        m.set_relief(h, Relief::Rugged);
        assert_eq!(m.move_cost(h), Some(MOUNTAIN_MOVE_COST), "forêt accidentée : plafond");
        assert_eq!(m.yields(h), Some(Yields::new(0, 3, 0)));
        assert_eq!(m.describe(h), "Forêt tempérée accidentée");
        m.set_relief(h, Relief::Peak);
        assert_eq!(m.move_cost(h), None, "pic infranchissable");
        // Répartition des reliefs générés : peu de pics, jamais deux côte à côte
        let peaks: Vec<Hex> = map.hexes().filter(|&h| map.relief(h) == Relief::Peak).collect();
        let land_all = map.hexes().filter(|&h| map.get(h).is_some_and(|t| !t.is_water())).count();
        assert!(peaks.len() * 100 <= land_all * 2, "{} pics / {land_all}", peaks.len());
        assert!(peaks.iter().all(|p| p.neighbors().iter().all(|n| map.relief(*n) != Relief::Peak)));
        assert!(map.hexes().any(|h| map.relief(h) == Relief::Mountain), "des montagnes franchissables");
    }
}

#[cfg(test)]
mod apercu {
    #[test]
    fn rivieres_et_fleuves_en_jeu() {
        use super::*;
        let mut m = WorldMap::filled(10, 10, Terrain::Grassland);
        let a = Hex::from_offset(4, 4);
        let b = a + crate::hex::DIRECTIONS[0];
        assert_eq!(m.step_cost(a, b), Some(1));
        assert_eq!(m.yields(a), Some(Yields::new(2, 0, 0)));
        // Rivière sur l'arête : traversée plus chère dans les deux sens, eau douce des deux côtés
        m.set_river(a, 0, 150.0);
        assert!(m.river_between(a, b) && m.river_between(b, a));
        assert_eq!(m.step_cost(a, b), Some(1 + RIVER_CROSSING_COST));
        assert_eq!(m.step_cost(b, a), Some(1 + RIVER_CROSSING_COST));
        assert_eq!(m.yields(b), Some(Yields::new(2 + FRESH_WATER_FOOD, 0, 0)));
        // Longer la rivière ne la traverse pas
        let c = a + crate::hex::DIRECTIONS[1];
        assert_eq!(m.step_cost(a, c), Some(1));
        // Fleuve : on y entre en s'arrêtant, rendements propres, eau douce pour les rives
        let f = Hex::from_offset(7, 7);
        m.set_hills(f, true);
        m.set_fleuve(f);
        assert!(!m.hills(f));
        assert_eq!(m.move_cost(f), Some(FLEUVE_MOVE_COST));
        assert_eq!(m.yields(f), Some(FLEUVE_YIELDS));
        assert!(m.fresh_water(f + crate::hex::DIRECTIONS[3]));
        assert!(crate::rules::can_found_city(&m, f, &[]).is_err());
        assert!(crate::rules::can_found_city(&m, f + crate::hex::DIRECTIONS[3], &[]).is_ok());
    }

    #[test]
    fn lacs_en_altitude() {
        use super::*;
        let mut m = WorldMap::filled(6, 6, Terrain::Plains);
        let h = Hex::from_offset(2, 2);
        m.set_level(h, 4);
        m.set(h, Terrain::Lake);
        assert_eq!(m.level(h), Some(4), "un lac garde son altitude");
        assert!(m.get(h).unwrap().is_water() && !m.get(h).unwrap().is_sea());
        m.set(h, Terrain::Ocean);
        assert_eq!(m.level(h), Some(0));
    }

    #[test]
    fn hydrologie_de_la_carte_generee() {
        use super::*;
        let m = WorldMap::generate(64, 40, 7);
        let hy = m.hydrology();
        let balance = hy.to_sea + hy.lake_evaporation + hy.bed_losses;
        assert!((balance - hy.total_runoff).abs() <= hy.total_runoff * 1e-3 + 1.0);
        for h in m.hexes() {
            // Une rivière d'arête est entre deux cases de terre ; un fleuve n'est jamais un lac
            for d in 0..6 {
                if m.river_flow(h, d) > 0.0 {
                    let o = h + crate::hex::DIRECTIONS[d];
                    assert!(!m.get(h).unwrap().is_water() && !m.get(o).unwrap().is_water());
                    assert!(!m.is_fleuve(h) && !m.is_fleuve(o));
                }
            }
            if m.is_fleuve(h) {
                assert!(!m.get(h).unwrap().is_water());
            }
            if m.get(h) == Some(Terrain::Lake) {
                assert!(m.level(h).unwrap() >= 1);
                // Deux cases de lac voisines : même niveau (une seule surface)
                for nb in h.neighbors() {
                    if m.get(nb) == Some(Terrain::Lake) {
                        assert_eq!(m.level(nb), m.level(h));
                    }
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn carte_ascii() {
        use super::*;
        let seed: u32 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(777);
        let m = WorldMap::generate(64, 40, seed);
        let mut counts = std::collections::BTreeMap::new();
        for row in 0..m.height {
            let mut line = String::new();
            let mut rl = String::new();
            for col in 0..m.width {
                let h = Hex::from_offset(col, row);
                let t = m.get(h).unwrap();
                *counts.entry(t.name()).or_insert(0) += 1;
                line.push(match t {
                    Terrain::Ocean => ' ', Terrain::Coast => '.', Terrain::Grassland => 'p', Terrain::Plains => '-',
                    Terrain::Desert => 'D', Terrain::Tundra => 't', Terrain::Snow => '*', Terrain::Forest => 'F',
                    Terrain::Lake => 'o', Terrain::Jungle => 'J', Terrain::Savanna => 's', Terrain::WoodedSavanna => 'w', Terrain::Mediterranean => 'm', Terrain::Taiga => 'T', Terrain::Steppe => '_', Terrain::Marsh => '~',
                });
                let w = m.wind(h).unwrap();
                rl.push(if t.is_water() {
                    let a = w.1.atan2(w.0).to_degrees();
                    ['→', '↗', '↑', '↖', '←', '↙', '↓', '↘'][(((a + 360.0 + 22.5) % 360.0) / 45.0) as usize]
                } else {
                    // Pluie : 0 (< 100 mm) à 9 (≥ 2 000 mm), par paliers
                    let mm = m.rain(h).unwrap();
                    let k = [100.0, 200.0, 300.0, 450.0, 600.0, 800.0, 1_000.0, 1_400.0, 2_000.0].iter().filter(|&&s| mm >= s).count();
                    char::from_digit(k as u32, 10).unwrap()
                });
            }
            println!("{line}  {rl}");
        }
        println!("{counts:?}");
    }
}

#[cfg(test)]
mod bilan {
    #[test]
    #[ignore]
    fn bilan_eau() {
        use super::*;
        let seed: u32 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(777);
        let m = WorldMap::generate(64, 40, seed);
        let c = m.climate();
        let (mut land, mut sea) = (Vec::new(), Vec::new());
        for (i, h) in m.hexes().enumerate() {
            if m.get(h).unwrap().is_water() { sea.push(i) } else { land.push(i) }
        }
        let avg = |f: &Vec<f32>, set: &Vec<usize>| set.iter().map(|&i| f[i]).sum::<f32>() / set.len() as f32;
        println!("TERRE pluie {:.0} évap {:.0} ETP {:.0} ruiss {:.0} HR {:.0}% col {:.1} T {:.1}", avg(&c.rain, &land), avg(&c.evaporation, &land), avg(&c.pet, &land), avg(&c.runoff, &land), 100.0 * avg(&c.humidity, &land), avg(&c.column, &land), avg(&c.temperature, &land));
        println!("MER   pluie {:.0} évap {:.0} HR {:.0}% col {:.1} T {:.1}", avg(&c.rain, &sea), avg(&c.evaporation, &sea), 100.0 * avg(&c.humidity, &sea), avg(&c.column, &sea), avg(&c.temperature, &sea));
        if let Some(row) = std::env::var("ROW").ok().and_then(|v| v.parse::<i32>().ok()) {
            let cell = |f: &Vec<f32>, k: f32| (0..64).map(|col| format!("{:>4.0}", k * f[(row * 64 + col) as usize])).collect::<Vec<_>>().join("");
            let terr: String = (0..64).map(|col| format!("{:>4}", if m.get(Hex::from_offset(col, row)).unwrap().is_water() { "~" } else { "#" })).collect();
            let lvl: String = (0..64).map(|col| format!("{:>4}", m.level(Hex::from_offset(col, row)).unwrap())).collect();
            println!("terr {terr}\nniv  {lvl}\npluie{}\nHR   {}\ncol  {}\névap {}\nvent {}", cell(&c.rain, 1.0), cell(&c.humidity, 100.0), cell(&c.column, 1.0), cell(&c.evaporation, 1.0), (0..64).map(|col| format!("{:>4.1}", c.wind[(row * 64 + col) as usize].0)).collect::<Vec<_>>().join(""));
        }
        // Par bande de latitude (lignes de 4)
        for band in 0..10 {
            let rows = band * 4..band * 4 + 4;
            let set: Vec<usize> = land.iter().copied().filter(|&i| rows.contains(&(i as i32 / 64))).collect();
            let sset: Vec<usize> = sea.iter().copied().filter(|&i| rows.contains(&(i as i32 / 64))).collect();
            if set.is_empty() { continue; }
            println!("lignes {:>2}-{:>2}  terre pluie {:>5.0} HR {:>3.0}% | mer pluie {:>5.0} évap {:>5.0} HR {:>3.0}% col {:>4.1}", rows.start, rows.end - 1, avg(&c.rain, &set), 100.0 * avg(&c.humidity, &set), avg(&c.rain, &sset), avg(&c.evaporation, &sset), 100.0 * avg(&c.humidity, &sset), avg(&c.column, &sset));
        }
    }
}

#[cfg(test)]
mod planete {
    #[test]
    #[ignore]
    fn generation_planete() {
        use super::*;
        let seed: u32 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(777);
        let (w, h) = (
            std::env::var("W").ok().and_then(|s| s.parse().ok()).unwrap_or(192),
            std::env::var("H").ok().and_then(|s| s.parse().ok()).unwrap_or(111),
        );
        let t = std::time::Instant::now();
        let m = WorldMap::generate_in(w, h, seed, Region::planet(crate::climate::CELL_KM, h));
        println!("génération {w} × {h} : {:.1} s", t.elapsed().as_secs_f32());
        let mut counts = std::collections::BTreeMap::new();
        for hex in m.hexes() {
            *counts.entry(m.get(hex).unwrap().name()).or_insert(0) += 1;
        }
        println!("{counts:?}");
        // Cours d'eau
        let hy = m.hydrology();
        let land = m.hexes().filter(|&x| !m.get(x).unwrap().is_water()).count();
        let edges: usize = hy.edge_flow.iter().map(|e| e.iter().filter(|&&f| f > 0.0).count()).sum::<usize>() / 2;
        let fleuves = hy.fleuve.iter().filter(|&&f| f).count();
        let lakes = hy.lake.iter().filter(|&&f| f).count();
        let small = hy.small_lake.iter().filter(|&&f| f).count();
        println!(
            "eau : ruissellement {:.0} m³/s → mer {:.0}, lacs {:.0}, lits {:.0} ; {land} cases de terre",
            hy.total_runoff, hy.to_sea, hy.lake_evaporation, hy.bed_losses
        );
        println!("rivières : {edges} arêtes ; fleuves : {fleuves} cases ; lacs : {lakes} cases ; petits lacs : {small}");
        let mut mouths: Vec<(f32, Hex)> = m
            .hexes()
            .filter(|&x| !m.get(x).unwrap().is_water())
            .filter(|&x| m.downstream(x).is_some_and(|d| m.get(x + crate::hex::DIRECTIONS[d]).is_some_and(|t| t.is_sea())))
            .map(|x| (m.discharge(x), x))
            .collect();
        mouths.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("plus grands débits à l'embouchure (m³/s) : {:?}", mouths.iter().take(15).map(|(q, _)| q.round()).collect::<Vec<_>>());
        for t in [20.0f32, 40.0, 60.0, 100.0, 150.0, 250.0] {
            let e: usize = hy.edge_flow.iter().map(|e| e.iter().filter(|&&f| f >= t).count()).sum::<usize>() / 2;
            println!("  arêtes ≥ {t} m³/s : {e}");
        }
        for t in [600.0f32, 1000.0, 1500.0, 2000.0, 3000.0] {
            let c = m.hexes().filter(|&x| !m.get(x).unwrap().is_water() && m.discharge(x) >= t).count();
            println!("  cases ≥ {t} m³/s : {c}");
        }
        // Lacs : tailles (composantes connexes)
        let mut seen = std::collections::HashSet::new();
        let mut sizes = Vec::new();
        for x in m.hexes() {
            if m.get(x) != Some(Terrain::Lake) || !seen.insert(x) { continue; }
            let mut stack = vec![x];
            let mut k = 0;
            while let Some(c) = stack.pop() {
                k += 1;
                for nb in m.neighbors(c) {
                    if m.get(nb) == Some(Terrain::Lake) && seen.insert(nb) { stack.push(nb); }
                }
            }
            sizes.push(k);
        }
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        println!("lacs : {} ; tailles {:?}", sizes.len(), &sizes[..sizes.len().min(30)]);
        if std::env::var("ASCII").is_ok() {
            for row in (0..h).step_by(2) {
                let line: String = (0..w)
                    .step_by(2)
                    .map(|col| {
                        let hx = Hex::from_offset(col, row);
                        match m.get(hx).unwrap() {
                            Terrain::Ocean => ' ',
                            Terrain::Coast => '.',
                            Terrain::Grassland => 'p',
                            Terrain::Plains => '-',
                            Terrain::Desert => 'D',
                            Terrain::Tundra => 't',
                            Terrain::Snow => '*',
                            Terrain::Forest => 'F',
                            Terrain::Lake => 'o',
                            Terrain::Jungle => 'J', Terrain::Savanna => 's', Terrain::WoodedSavanna => 'w', Terrain::Mediterranean => 'm', Terrain::Taiga => 'T', Terrain::Steppe => '_', Terrain::Marsh => '~',
                        }
                    })
                    .collect();
                println!("{:>4.0}° {line}", m.latitude(Hex::from_offset(0, row)));
            }
        }
    }
}

