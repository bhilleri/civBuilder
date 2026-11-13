//! Terrains, carte du monde et génération procédurale (logique pure).

use crate::hex::{Hex, Wrap};
use crate::plates::{self, Plate, Plates};
use crate::climate::{self, Climate, Region};
use crate::currents;
use crate::basins::{self, Basin, Basins, NamedRiver};
use crate::hydro::{self, Hydrology};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use bevy::platform::collections::HashSet;

/// Coût d'un pas vers une case voisine, en deux parts. Le terrain (relief et montée compris) se
/// paie en entier : ce qui dépasse les PM restants est une dette, retirée des PM des tours suivants
/// (une montagne immobilise plusieurs tours). L'eau (traversée d'une rivière, entrée dans un fleuve)
/// arrête seulement l'unité, sans dette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepCost {
    pub terrain: u32,
    pub water: u32,
    /// Case d'arrivée en montagne (coût propre à chaque type d'unité).
    pub mountain: bool,
    /// Le pas mène sur le pont d'une case de fleuve (par un de ses bouts).
    pub onto_bridge: bool,
}

impl StepCost {
    /// Pas sans eau ni montagne.
    pub fn terrain(terrain: u32) -> Self {
        Self { terrain, ..Self::default() }
    }

    /// Coût total du pas (PM).
    pub fn total(self) -> u32 {
        self.terrain + self.water
    }

    /// PM restants et dette après ce pas, fait avec `left` PM.
    pub fn spend(self, left: u32) -> (u32, u32) {
        (left.saturating_sub(self.total()), self.terrain.saturating_sub(left))
    }
}

/// PM d'un nouveau tour et dette restante : la dette est payée d'abord, jusqu'à `max_moves`.
/// Aucun PM : l'unité est immobilisée ce tour-ci.
pub fn turn_moves(max_moves: u32, debt: u32) -> (u32, u32) {
    let paid = debt.min(max_moves);
    (max_moves - paid, debt - paid)
}

/// Source des coûts de déplacement utilisée par le pathfinding.
pub trait MoveCosts {
    /// Coût du terrain de `hex` ; `None` = infranchissable ou hors carte.
    fn move_cost(&self, hex: Hex) -> Option<u32>;

    /// Coût détaillé d'un pas de `from` vers sa voisine `to` : terrain d'arrivée, pente, eau.
    /// `None` = infranchissable (terrain, falaise…).
    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        let _ = from;
        self.move_cost(to).map(StepCost::terrain)
    }

    /// Coût total d'un pas de `from` vers sa voisine `to` (voir [`MoveCosts::step`]).
    fn step_cost(&self, from: Hex, to: Hex) -> Option<u32> {
        self.step(from, to).map(StepCost::total)
    }

    /// Tour d'est en ouest de la carte (voisines, distances).
    fn wrap(&self) -> Wrap {
        Wrap::NONE
    }
}

impl<M: MoveCosts + ?Sized> MoveCosts for &M {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        (**self).move_cost(hex)
    }

    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        (**self).step(from, to)
    }

    fn wrap(&self) -> Wrap {
        (**self).wrap()
    }
}

/// Niveau d'altitude : 0 = eau, 1 (littoral) à 9 (massif, où se trouvent les sommets), déduit de
/// l'altitude en mètres (`level_of_meters`).
pub const MAX_LEVEL: u8 = 9;

/// Falaise : dénivelée minimale (m) entre deux cases voisines, la case basse étant sous
/// `MOUNTAIN_MIN_M` (entre deux cases de montagne, c'est le coût de la montagne qui freine).
pub const CLIFF_M: f32 = 1_000.0;

/// Falaise entre deux cases voisines d'altitudes `from_m` et `to_m` (m ; mer à 0) :
/// infranchissable dans les deux sens (et dessinée comme telle en 3D).
pub fn is_cliff(from_m: f32, to_m: f32) -> bool {
    (to_m - from_m).abs() >= CLIFF_M && from_m.min(to_m) < MOUNTAIN_MIN_M
}

/// Surcoût d'un pas selon la dénivelée réelle (m) : montée de moins de `CLIMB_GENTLE_M`
/// gratuite, jusqu'à `CLIMB_STEEP_M` +2 PM, au-delà +4 PM ; la descente est gratuite (coûts du
/// relief doublés avec les PM, 11/11/2026). (À 40 km
/// entre deux centres de cases, c'est surtout le relief interne à la case qui gêne : voir `Relief`.)
pub fn climb_cost(from_m: f32, to_m: f32) -> u32 {
    let up = to_m - from_m;
    if up < CLIMB_GENTLE_M {
        0
    } else if up < CLIMB_STEEP_M {
        2
    } else {
        4
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
    /// Montagne : franchissable, de 4 à 8 PM selon l'altitude.
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

    /// Surcoût de déplacement (PM) ajouté au terrain, sous le plafond `RELIEF_MOVE_CAP`.
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
/// `high` est la case haute, `dir` la direction de la case basse, `diff` l'écart de niveaux (1 ou
/// plus), `cliff` : falaise (`is_cliff`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelEdge {
    pub high: Hex,
    pub dir: usize,
    pub diff: u8,
    pub cliff: bool,
}

impl LevelEdge {
    pub fn low(&self) -> Hex {
        self.high + crate::hex::DIRECTIONS[self.dir]
    }
    /// Falaise : infranchissable.
    pub fn is_cliff(&self) -> bool {
        self.cliff
    }
}

/// Vérité terrain : utilisée pour le déplacement effectif et dans les tests.
impl MoveCosts for WorldMap {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        self.enter_cost(hex).map(StepCost::total)
    }

    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        let mut step = self.enter_cost(to)?;
        // (une côte escarpée barre aussi le passage entre la terre et la banquise)
        if self.cliff_between(from, to) || ((self.is_ice(from) || self.is_ice(to)) && self.cliff_with_sea(from, to)) {
            return None;
        }
        // Pont d'un fleuve, par un de ses bouts : 1 PM, sans arrêt ; pont d'une rivière : sans surcoût
        if self.bridge_end(to, from) {
            step = StepCost { terrain: BRIDGE_MOVE_COST, onto_bridge: true, ..StepCost::default() };
        } else if self.river_between(from, to) && !self.bridge_between(from, to) {
            step.water += RIVER_CROSSING_COST;
        }
        step.terrain += climb_cost(self.elevation(from)?, self.elevation(to)?);
        Some(step)
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
    pub explored: &'a crate::cases::CaseSet,
    /// Cases occupées par l'ennemi (connues du joueur) : infranchissables tant qu'il n'y a pas de combat.
    pub blocked: Option<&'a HashSet<Hex>>,
}

impl<'a> KnownMap<'a> {
    pub fn new(map: &'a WorldMap, explored: &'a crate::cases::CaseSet) -> Self {
        Self { map, explored, blocked: None }
    }

    pub fn blocking(mut self, blocked: &'a HashSet<Hex>) -> Self {
        self.blocked = Some(blocked);
        self
    }
}

pub const UNKNOWN_MOVE_COST: u32 = 4;

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

    /// La pente et les rivières ne sont connues qu'entre deux cases explorées.
    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        let cost = self.move_cost(to)?;
        match (self.explored.contains(&from), self.explored.contains(&to)) {
            (true, true) => self.map.step(from, to),
            (false, true) => self.map.enter_cost(to),
            _ => Some(StepCost::terrain(cost)),
        }
    }

    fn wrap(&self) -> Wrap {
        self.map.wrap
    }
}

/// Altitude (m) des terres d'une planète selon l'altitude relative issue des plaques et de
/// l'érosion (0 au rivage, ~1,5 aux plus hauts sommets) : échelle fixe, interpolée entre ces
/// points (prolongée au-delà du dernier). Calée pour qu'en moyenne la répartition des altitudes
/// ressemble à celle de la Terre (~30 % des terres sous 300 m, ~70 % sous 1 000 m) ; chaque monde
/// garde la sienne (un continent de plaines, un autre de hauts plateaux).
const TECTONIC_METERS: [(f64, f32); 8] =
    [(0.0, 0.0), (0.04, 150.0), (0.14, 350.0), (0.25, 650.0), (0.38, 1_200.0), (0.55, 2_100.0), (0.95, 3_000.0), (1.5, 4_800.0)];
/// Carte régionale (sans plaques, tests) : altitude (m) selon le rang parmi les terres émergées
/// (part des terres plus basses), même répartition moyenne que sur une planète.
const REGION_METERS: [(f64, f32); 7] =
    [(0.0, 0.0), (0.16, 150.0), (0.36, 350.0), (0.56, 650.0), (0.74, 1_200.0), (0.87, 2_100.0), (0.96, 3_000.0)];
/// Parts des terres par relief (les plus escarpées d'abord) : pics, montagnes, relief accidenté,
/// collines ; le reste est plat. Une montagne demande au moins `MOUNTAIN_MIN_M`, un pic
/// `PEAK_MIN_M` et d'être un sommet local (jamais deux pics côte à côte).
const PEAK_SHARE: f64 = 0.01;
const MOUNTAIN_SHARE: f64 = 0.05;
const RUGGED_SHARE: f64 = 0.08;
const HILL_SHARE: f64 = 0.15;
pub const MOUNTAIN_MIN_M: f32 = 1_500.0;
pub const PEAK_MIN_M: f32 = 2_400.0;
/// Un pic peut avoir une voisine un peu plus haute que lui (crête), de cet écart au plus (m).
const PEAK_TOLERANCE_M: f32 = 200.0;
/// Montagne : `MOUNTAIN_MOVE_MIN` PM jusqu'à `MOUNTAIN_MIN_M` + `MOUNTAIN_STEP_M`, puis +2 PM par
/// tranche de `MOUNTAIN_STEP_M`, jusqu'à `MOUNTAIN_MOVE_MAX` PM : les cols et les vallées, plus bas
/// que les crêtes, sont les passages les plus rapides. (Coûts du relief doublés avec les PM des
/// unités, 11/11/2026 : on avance deux fois plus loin en plaine, le relief reste une barrière.)
pub const MOUNTAIN_MOVE_MIN: u32 = 8;
pub const MOUNTAIN_MOVE_MAX: u32 = 16;
pub const MOUNTAIN_STEP_M: f32 = 500.0;
/// Plafond des autres reliefs (terrain + collines ou relief accidenté) ; relief accidenté +4 PM.
pub const RELIEF_MOVE_CAP: u32 = 6;
pub const RUGGED_MOVE_COST: u32 = 4;

/// Coût d'une case de montagne selon son altitude (m).
pub fn mountain_move_cost(elevation_m: f32) -> u32 {
    let steps = ((elevation_m - MOUNTAIN_MIN_M) / MOUNTAIN_STEP_M).floor().max(0.0) as u32;
    (MOUNTAIN_MOVE_MIN + 2 * steps).min(MOUNTAIN_MOVE_MAX)
}
/// Rendements d'une montagne (pâturages d'altitude, pierre).
pub const MOUNTAIN_YIELDS: Yields = Yields::new(0, 1, 0);
/// Collines : +1 de production et +2 PM par rapport au terrain de base.
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
/// Marais : niveau d'altitude maximal (3 : sous 500 m) et indice d'aridité minimal.
pub const MARSH_MAX_LEVEL: u8 = 3;
pub const MARSH_ARIDITY: f32 = 1.0;
/// Limite des arbres (choix de Benjamin, 08/11/2026) : pas de forêt tempérée quand la saison la
/// plus chaude reste sous ces températures (°C) ; c'est une prairie d'altitude (páramo, puna,
/// alpages). Repère : la limite des arbres et la toundra de Köppen, vers 10 °C au mois le plus
/// chaud. Ne vise que les forêts tempérées : sous ces latitudes, celles des hauts plateaux
/// tropicaux, presque sans saisons. La taïga n'est pas concernée (les étés de la simulation sont
/// trop frais aux hautes latitudes : la plupart de ses cases resteraient sous 10 °C).
pub const TREE_LINE_SUMMER: f32 = 10.0;

/// Climat d'une case vu par le choix du biome.
#[derive(Clone, Copy, Debug)]
pub struct BiomeClimate {
    /// Température moyenne annuelle (°C).
    pub temperature: f32,
    /// Indice d'aridité (pluie / évaporation potentielle).
    pub aridity: f32,
    /// Température de la saison la plus froide (°C).
    pub coldest: f32,
    /// Température de la saison la plus chaude (°C).
    pub warmest: f32,
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
    } else if c.aridity < FOREST_ABOVE || c.warmest < TREE_LINE_SUMMER {
        // (au-dessus de la limite des arbres : prairie d'altitude)
        Terrain::Grassland
    } else {
        Terrain::Forest
    }
}
pub const HILL_MOVE_COST: u32 = 2;
/// Altitudes (m) des limites entre niveaux voisins : 9 niveaux, serrés en basse altitude
/// (choix de Benjamin, 01/11/2026), et altitude des plus hauts pics (repère du relief).
pub const LEVEL_BOUNDS_M: [f32; 8] = [100.0, 250.0, 500.0, 750.0, 1_000.0, 1_300.0, 1_700.0, 2_300.0];
const PEAK_M: f32 = 4_800.0;

/// Altitude (m) au milieu de chaque niveau (cartes faites à la main : tests).
#[cfg(test)]
const LEVEL_ALTITUDE_M: [f32; MAX_LEVEL as usize] = [50.0, 175.0, 375.0, 625.0, 875.0, 1_150.0, 1_500.0, 2_000.0, 2_700.0];

/// Interpolation linéaire par morceaux entre des points (x croissants), prolongée au-delà des
/// extrémités par la pente du premier ou du dernier segment.
fn piecewise(knots: &[(f64, f32)], x: f64) -> f32 {
    let k = knots.windows(2).find(|w| x <= w[1].0).unwrap_or(&knots[knots.len() - 2..]);
    let ((x0, y0), (x1, y1)) = (k[0], k[1]);
    if x1 > x0 { y0 + (y1 - y0) * ((x - x0) / (x1 - x0)) as f32 } else { y1 }
}

/// Niveau d'altitude (1 à `MAX_LEVEL`) d'une altitude en mètres.
pub fn level_of_meters(m: f32) -> u8 {
    1 + LEVEL_BOUNDS_M.iter().filter(|&&b| m > b).count() as u8
}
/// Traverser une rivière (arête) : +4 PM, ce qui arrête la plupart des unités sur l'autre rive.
pub const RIVER_CROSSING_COST: u32 = 4;
/// Entrer dans un fleuve (à gué ou en barque) : 6 PM, l'unité s'y arrête.
pub const FLEUVE_MOVE_COST: u32 = 6;
/// Monter sur le pont d'un fleuve par un de ses bouts : 1 PM, sans arrêt (choix de Benjamin, 08/11/2026).
pub const BRIDGE_MOVE_COST: u32 = 1;

/// Place d'une unité dans les ensembles d'occupation (`HashSet<Hex>`) : la case elle-même, ou, sur
/// le pont d'une case de fleuve, une clé à part (la case décalée de `BRIDGE_SLOT_ROWS` lignes, hors
/// de toute carte). Une case de fleuve avec pont a ainsi deux places : le pont et l'eau
/// (choix de Benjamin, 09/11/2026).
pub fn slot(hex: Hex, on_bridge: bool) -> Hex {
    if on_bridge { Hex { q: hex.q, r: hex.r + BRIDGE_SLOT_ROWS } } else { hex }
}
const BRIDGE_SLOT_ROWS: i32 = 1 << 20;

/// Pont construit (voir `conception/conception-ponts.md`) : sur l'arête d'une rivière, ou en
/// travers d'une case de fleuve, d'une rive à la rive opposée. Toujours rangé sous une seule
/// écriture (`WorldMap::bridge_site`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BridgeAt {
    /// Arête `dir` de la case `hex` (rivière d'arête).
    Edge { hex: Hex, dir: u8 },
    /// Case de fleuve `hex`, de sa voisine `axis` à sa voisine `axis + 3` (`axis` de 0 à 2).
    Fleuve { hex: Hex, axis: u8 },
}

impl BridgeAt {
    /// Les deux cases que le pont relie (les deux rives).
    pub fn ends(self, wrap: Wrap) -> (Hex, Hex) {
        match self {
            BridgeAt::Edge { hex, dir } => (hex, wrap.neighbor(hex, usize::from(dir))),
            BridgeAt::Fleuve { hex, axis } => (wrap.neighbor(hex, usize::from(axis)), wrap.neighbor(hex, usize::from(axis) + 3)),
        }
    }
}
/// Rendements d'une case de fleuve (pêche, transport et commerce).
pub const FLEUVE_YIELDS: Yields = Yields::new(2, 0, 1);
/// Un lieu nommé (barre de lieu) : indice de sa zone dans sa famille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// `Seas::zones`
    Sea(usize),
    /// `Basins::lakes`
    Lake(usize),
    /// `Lands::zones`
    Land(usize),
    /// `Regions::zones`
    Region(usize),
    /// `Reliefs::zones`
    Relief(usize),
    /// `Basins::basins`
    Basin(usize),
}

/// Zone allongée (nom courbé le long de son axe) : longueur de l'axe au carré rapportée au nombre
/// de cases.
pub const SPINE_ELONGATION: f32 = 2.5;
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
    pub deniers: i32,
}

impl Yields {
    pub const fn new(food: i32, production: i32, deniers: i32) -> Self {
        Self { food, production, deniers }
    }
}

impl std::ops::AddAssign for Yields {
    fn add_assign(&mut self, o: Self) {
        self.food += o.food;
        self.production += o.production;
        self.deniers += o.deniers;
    }
}

/// Terre gagnée sur la mer par un delta : altitude (m) à la côte, et un peu plus près du fleuve.
const DELTA_LAND_M: [f32; 2] = [6.0, 3.0];
/// Un delta avance sur le plateau (fond inférieur à `SHELF_DEPTH_M`), de une case, deux pour un
/// fleuve qui apporte au moins ce flux de sédiments (kg/s) et domine (delta digité) ; un delta arqué
/// s'étale aussi sur les deux cases de mer voisines de son embouchure.
const DELTA_TWO_CELLS_KG_S: f32 = 800.0;
/// Il ne ferme ni détroit ni baie : au plus ce nombre de voisines de terre pour une nouvelle case,
/// sans compter celle d'où il vient.
const DELTA_MAX_LAND_AROUND: usize = 2;

/// Marcher sur la banquise : 2 PM, comme une forêt.
pub const ICE_MOVE_COST: u32 = 4;
/// Banquise : température de la mer du jeu (°C) en dessous de laquelle elle est gelée toute
/// l'année. Cette température suit celle de l'air vers les pôles (voir
/// `climate::sea_level_temperature` : −10 °C à 70°, −18 °C à 80°) : le seuil met la limite vers 76° en moyenne.
pub const ICE_SEA_C: f32 = -15.0;
/// Écart de ce seuil (°C, multiplié par un bruit de ± 0,7 environ) et taille des découpures du bord
/// (cases) : la température varie trop peu le long d'une latitude pour dessiner un bord naturel.
const ICE_EDGE_NOISE_C: f32 = 7.0;
const ICE_EDGE_CELLS: f64 = 14.0;

/// Écart du seuil de la banquise par case (°C) : bruit de la graine, raccordé d'est en ouest sur une
/// carte qui boucle.
fn ice_edge_noise(width: i32, height: i32, seed: u32, wraps: bool) -> Vec<f32> {
    let noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(424_243)).set_octaves(3).set_frequency(1.0);
    let f = 1.0 / ICE_EDGE_CELLS;
    let radius = width as f64 / std::f64::consts::TAU;
    (0..width * height)
        .map(|i| {
            let (col, row) = ((i % width) as f64, (i / width) as f64);
            let value = if wraps {
                let angle = col / width as f64 * std::f64::consts::TAU;
                noise.get([angle.cos() * radius * f, angle.sin() * radius * f, row * f])
            } else {
                noise.get([col * f, row * f])
            };
            value as f32 * ICE_EDGE_NOISE_C
        })
        .collect()
}

impl Terrain {
    pub fn name(self) -> &'static str {
        match self {
            Terrain::Ocean => "Haute mer",
            Terrain::Coast => "Eaux peu profondes",
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
            Terrain::Forest | Terrain::Jungle | Terrain::Taiga => Some(4),
            Terrain::Marsh => Some(6),
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

/// Fond marin : part des cases de mer (des moins profondes aux plus profondes) et profondeur (m)
/// atteinte à cette part. Plateau continental jusqu'à 200 m (≈ 14 % des mers : un peu plus que sur Terre, à cause des cases de 40 km), talus jusqu'à
/// 3 000 m, plaines abyssales jusqu'à 6 000 m ; les fosses (au-delà) suivent les subductions.
const DEPTH_KNOTS: [(f32, f32); 4] = [(0.0, 10.0), (0.14, SHELF_DEPTH_M), (0.34, ABYSS_DEPTH_M), (1.0, 5_800.0)];
/// Eaux poissonneuses : nourriture en plus.
pub const RICH_WATERS_FOOD: i32 = 1;
/// Remontée d'eaux : latitude maximale (degrés) et force minimale du transport vers le large
/// (vent de 0 à 1).
const UPWELLING_MAX_LATITUDE: f32 = 45.0;
const UPWELLING_MIN: f32 = 0.25;
/// Rencontre de courants : eau plus chaude et eau plus froide d'au moins tant (°C) que celle de
/// la latitude, à au plus `FRONT_RADIUS` cases l'une de l'autre, à au plus `FRONT_COAST_CELLS`
/// cases d'une côte (ou sur le plateau).
const FRONT_ANOMALY: f32 = 1.5;
const FRONT_RADIUS: i32 = 2;
const FRONT_COAST_CELLS: i32 = 3;
/// Mont sous-marin : sommet à au plus cette profondeur, au moins tant au-dessus du fond alentour.
const SEAMOUNT_MAX_DEPTH_M: f32 = 2_000.0;
const SEAMOUNT_RISE_M: f32 = 900.0;
/// … et à au moins autant de cases du plateau continental et des terres.
const SEAMOUNT_FROM_SHELF: i32 = 2;
/// Île volcanique isolée : au plus autant de cases.
const ISLAND_MAX_CELLS: usize = 30;
/// Fond marin : écart de rang par case d'éloignement de la côte (le plateau longe les côtes).
const SHELF_DISTANCE_WEIGHT: f64 = 0.15;
/// Limite du plateau continental (eaux peu profondes) : pêche riche.
pub const SHELF_DEPTH_M: f32 = 200.0;
/// Début des abysses et des fosses (m).
pub const ABYSS_DEPTH_M: f32 = 3_000.0;
pub const TRENCH_DEPTH_M: f32 = 6_000.0;

/// Banc de poissons : eaux poissonneuses d'un seul tenant.
#[derive(Clone, Debug)]
pub struct FishingBank {
    pub cells: Vec<usize>,
    /// Origines de la richesse (voir `marine`).
    pub origins: u8,
    pub name: String,
}

/// Eaux poissonneuses (en plus du plateau continental) : chacune donne +1 nourriture (une seule
/// fois par case).
pub mod marine {
    /// Remontée d'eaux froides : le vent pousse l'eau de surface vers le large (côtes ouest des
    /// continents sous les alizés), l'eau profonde riche remonte.
    pub const UPWELLING: u8 = 1;
    /// Mont sous-marin : un volcan qui monte vers la surface (chaînes des points chauds, arcs
    /// d'îles, abords des dorsales) ; courants et remous y concentrent le plancton et les thons.
    pub const SEAMOUNT: u8 = 2;
    /// Eaux d'une île volcanique isolée en haute mer (« effet de masse d'île »).
    pub const ISLAND: u8 = 4;
    /// Rencontre d'un courant chaud et d'un courant froid : les eaux se mélangent, le plancton
    /// prolifère (bancs de Terre-Neuve, du Japon).
    pub const FRONT: u8 = 8;
}
/// Passes climat → cours d'eau : à partir de la deuxième, le climat tient compte des lacs.
pub const CLIMATE_PASSES: usize = 3;

/// Carte rectangulaire stockée en coordonnées offset (odd-r).
/// Règle du relief de la génération : note d'escarpement d'une case (dénivelée avec les voisines
/// et bruit propre, classés parmi les terres, et altitude) et notes des dernières cases de chaque
/// relief ; un pic est un sommet local de montagne au moins aussi haut que le plus bas des pics.
#[derive(Clone, Debug)]
struct ReliefRule {
    /// Dénivelées et bruits des terres à la génération, triés (rang d'une valeur nouvelle).
    local_sorted: Vec<f32>,
    rough_sorted: Vec<f32>,
    /// Bruit propre de chaque case (toutes, eau comprise).
    rough: Vec<f32>,
    /// Par case de terre à la génération : dénivelée et rangs (dénivelée, bruit) alors calculés,
    /// repris tels quels tant que la dénivelée n'a pas changé (ex aequo départagés comme alors).
    generated: Vec<Option<(f32, f32, f32)>>,
    /// Note minimale d'une montagne, d'un relief accidenté, de collines.
    thresholds: [f32; 3],
    /// Altitude du plus bas des pics (m).
    peak_floor_m: f32,
}

/// État de la carte d'avant le climat (étape 3 de la génération), d'où repartent le climat, les
/// biomes et l'eau : classes d'altitude et relief des terres (avant les lacs et les fleuves),
/// variation locale de l'aridité de chaque case (sols, exposition). Les modifications de la
/// console de debug y sont reportées.
#[derive(Clone, Debug)]
struct ClimateBase {
    levels: Vec<u8>,
    relief: Vec<Relief>,
    jitter: Vec<f32>,
}

/// Rang (0 à 1) d'une valeur parmi des valeurs triées.
fn rank_in(sorted: &[f32], v: f32) -> f32 {
    sorted.partition_point(|&x| x < v) as f32 / sorted.len().max(1) as f32
}

/// Note d'escarpement : dénivelée et bruit propre (rangs parmi les terres) et altitude.
fn relief_score(local_rank: f32, rough_rank: f32, elevation_m: f32) -> f32 {
    0.45 * local_rank + 0.3 * rough_rank + 0.35 * (elevation_m / PEAK_M).clamp(0.0, 1.0)
}

/// Limon apporté par l'eau qui ruisselle sur une case (0 à 1, indice relatif) : fort sur les
/// sols nus ou peu couverts (désert, steppe, savane), faible sous la forêt boréale, la toundra
/// et la neige ; les hautes terres rocheuses donnent une eau claire.
pub fn sediment_yield(terrain: Terrain, elevation_m: f32) -> f32 {
    let soil = match terrain {
        Terrain::Desert => 1.0,
        Terrain::Steppe => 0.85,
        Terrain::Savanna => 0.8,
        Terrain::Mediterranean | Terrain::Plains => 0.6,
        Terrain::WoodedSavanna => 0.55,
        Terrain::Jungle => 0.5,
        Terrain::Grassland => 0.45,
        Terrain::Marsh => 0.35,
        Terrain::Forest => 0.25,
        Terrain::Taiga | Terrain::Tundra => 0.1,
        Terrain::Snow | Terrain::Ocean | Terrain::Coast | Terrain::Lake => 0.0,
    };
    let rock = ((elevation_m - 600.0) / 1_400.0).clamp(0.0, 1.0);
    soil * (1.0 - 0.7 * rock)
}

/// Part de limon qui ressort d'un lac (le reste s'y dépose : l'eau qui en sort est claire).
const LAKE_MURK_KEPT: f32 = 0.25;

/// Part du limon d'une case dans la turbidité de l'eau qui la traverse (le reste vient de
/// l'amont) : un cours d'eau prend la couleur des terres qu'il traverse en trois cases environ.
const MURK_LOCAL: f32 = 0.35;

/// Turbidité de l'eau sortant de chaque case : celle de l'amont (affluents mêlés selon leur
/// débit) tirée vers le limon de la case (`yields`), de sorte qu'un cours d'eau prend peu à peu
/// la couleur des terres qu'il traverse ; un lac en garde la plus grande partie. `down` : case aval.
pub fn murk_of_cells(down: &[Option<usize>], discharge: &[f32], yields: &[f32], lake: &[bool]) -> Vec<f32> {
    let n = down.len();
    let mut uphill = vec![0u32; n];
    for j in down.iter().flatten() {
        uphill[*j] += 1;
    }
    let is_lake = |k: usize| lake.get(k).copied().unwrap_or(false);
    // Des sources vers l'aval : une case est traitée quand tout son amont l'a été
    let mut ready: Vec<usize> = (0..n).filter(|&i| uphill[i] == 0).collect();
    // Débit reçu de l'amont, et limon qu'il porte (pondéré par le débit)
    let (mut inflow, mut load) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut murk = yields.to_vec();
    while let Some(i) = ready.pop() {
        let upstream = if inflow[i] > 0.0 { load[i] / inflow[i] } else { yields[i] };
        murk[i] = upstream + (yields[i] - upstream) * MURK_LOCAL;
        // Le limon se dépose dans le lac : l'eau en sort claire (une fois, à son exutoire)
        if is_lake(i) && down[i].is_none_or(|j| !is_lake(j)) {
            murk[i] *= LAKE_MURK_KEPT;
        }
        if let Some(j) = down[i] {
            // (au moins un peu de poids : une case sèche transmet quand même sa couleur)
            let q = discharge.get(i).copied().unwrap_or(0.0).max(1e-3);
            inflow[j] += q;
            load[j] += q * murk[i];
            uphill[j] -= 1;
            if uphill[j] == 0 {
                ready.push(j);
            }
        }
    }
    murk
}

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
    /// Courants marins et température de la mer (vides pour une carte remplie à la main).
    currents: crate::currents::Currents,
    /// Marées des cases de mer (voir `tides.rs`).
    tides: crate::tides::Tides,
    /// Houle des cases de mer (voir `waves.rs`).
    waves: crate::waves::Waves,
    /// Banquise : cases de mer gelées toute l'année (voir `is_ice`).
    ice: Vec<bool>,
    /// Embouchures de fleuve (case, forme), triées par case (voir `mouths.rs`).
    mouths: Vec<(usize, crate::mouths::Mouth)>,
    /// Cases de mer devenues terre devant un delta, avec la forme de ce delta (une seule fois :
    /// refaire le climat ne les fait pas grandir encore).
    delta_cells: Vec<(usize, crate::mouths::MouthKind)>,
    /// Écoulements imposés à travers cette terre (case, case réceptrice).
    delta_routes: Vec<(usize, usize)>,
    /// Cours d'eau et lacs (vides pour une carte remplie à la main).
    hydro: Hydrology,
    /// Échelle et latitude de la carte.
    region: Region,
    /// Bassins versants et fleuves nommés.
    basins: Basins,
    /// Tour d'est en ouest (planète) ou bords.
    wrap: Wrap,
    /// Plaques tectoniques (vides pour une carte remplie à la main).
    plates: Plates,
    /// Langue parlée sur chaque case (noms des lieux ; vide pour une carte remplie à la main).
    languages: Vec<crate::toponymy::Language>,
    /// Étendues marines nommées : océans, mers, golfes, baies, détroits.
    seas: crate::seas::Seas,
    /// Terres nommées : continents, îles, îlots, archipels, péninsules, isthmes.
    lands: crate::lands::Lands,
    /// Relief nommé : chaînes de montagnes, massifs, cols, plateaux, collines.
    reliefs: crate::mountains::Reliefs,
    /// Régions naturelles : forêts, déserts, steppes, marais… et leurs sous-régions.
    regions: crate::regions::Regions,
    /// Profondeur du fond de chaque case de mer (m, positive ; 0 sur terre).
    depth: Vec<f32>,
    /// Eaux poissonneuses de chaque case de mer (voir `marine`).
    marine: Vec<u8>,
    /// Bancs de poissons : eaux poissonneuses d'un seul tenant, nommées.
    banks: Vec<FishingBank>,
    bank_of: Vec<Option<u16>>,
    /// Règle du relief de la génération, pour recalculer le relief d'une case modifiée (vide pour
    /// une carte remplie à la main).
    relief_rule: Option<ReliefRule>,
    /// Les cours d'eau ont déjà creusé le relief (`incise_rivers`).
    rivers_incised: bool,
    /// Graine de la génération, et état d'avant le climat (pour refaire climat et eau).
    seed: u32,
    climate_base: Option<ClimateBase>,
    /// Coût d'un pas de chaque case vers chacune de ses voisines (`u8::MAX` : infranchissable),
    /// calculé à la première demande (le terrain ne change pas en cours de partie).
    step_cache: std::sync::OnceLock<Vec<[u8; 6]>>,
    /// Turbidité de l'eau sortant de chaque case (voir `water_murk`), calculée à la première demande.
    murk_cache: std::sync::OnceLock<Vec<f32>>,
    /// Roche et sol de chaque case de terre (géologie de surface, calculée à la demande depuis la
    /// carte : `geologie.rs` ; rien sur une carte sans climat), vidé avec le cache de turbidité.
    geologie_cache: std::sync::OnceLock<Vec<Option<(crate::geologie::Roche, crate::geologie::Sol)>>>,
    /// Coin aval de la rivière de chaque arête (numéroté dans la case qui porte le tronçon, 255 sans
    /// tronçon), calculé à la première demande.
    river_down_cache: std::sync::OnceLock<Vec<[u8; 6]>>,
    /// Ponts construits (ou posés à la console).
    bridges: HashSet<BridgeAt>,
}

impl WorldMap {
    pub fn filled(width: i32, height: i32, terrain: Terrain) -> Self {
        // (une terre à 150 m, comme les plaines côtières)
        let level = if terrain.is_water() { 0 } else { level_of_meters(150.0) };
        let n = (width * height) as usize;
        Self {
            width,
            height,
            tiles: vec![terrain; n],
            levels: vec![level; n],
            relief: vec![Relief::Flat; n],
            elevation: vec![if terrain.is_water() { 0.0 } else { 150.0 }; n],
            lake_level_m: vec![150.0; n],
            climate: Climate::default(),
            currents: crate::currents::Currents::default(),
            tides: crate::tides::Tides::default(),
            waves: crate::waves::Waves::default(),
            ice: Vec::new(),
            mouths: Vec::new(),
            delta_cells: Vec::new(),
            delta_routes: Vec::new(),
            hydro: Hydrology::empty(n),
            region: Region::for_seed(0),
            basins: Basins::empty(n),
            wrap: Wrap::NONE,
            plates: Plates::default(),
            languages: Vec::new(),
            seas: crate::seas::Seas::default(),
            lands: crate::lands::Lands::default(),
            reliefs: crate::mountains::Reliefs::default(),
            regions: crate::regions::Regions::default(),
            depth: vec![0.0; n],
            marine: vec![0; n],
            banks: Vec::new(),
            bank_of: vec![None; n],
            relief_rule: None,
            rivers_incised: false,
            seed: 0,
            climate_base: None,
            step_cache: std::sync::OnceLock::new(),
            murk_cache: std::sync::OnceLock::new(),
            geologie_cache: std::sync::OnceLock::new(),
            river_down_cache: std::sync::OnceLock::new(),
            bridges: HashSet::new(),
        }
    }

    /// Emplacement de pont sur l'arête `dir` de la case `hex`, rangé sous son unique écriture :
    /// en travers de la case si c'est un fleuve (de la voisine `dir` à l'opposée, deux terres
    /// fermes), sinon sur la rivière de cette arête (deux cases de terre). `None` : pas de pont
    /// possible ici.
    pub fn bridge_site(&self, hex: Hex, dir: usize) -> Option<BridgeAt> {
        let hex = self.canonical(hex);
        let dir = dir % 6;
        let firm = |h: Hex| self.contains(h) && !self.is_fleuve(h) && self.get(h).is_some_and(|t| !t.is_water()) && !self.is_ice(h);
        if self.is_fleuve(hex) {
            let axis = dir % 3;
            let ok = firm(self.wrap.neighbor(hex, axis)) && firm(self.wrap.neighbor(hex, axis + 3));
            return ok.then_some(BridgeAt::Fleuve { hex, axis: axis as u8 });
        }
        let other = self.wrap.neighbor(hex, dir);
        if !(firm(hex) && firm(other) && self.river_flow(hex, dir) > 0.0) {
            return None;
        }
        // (l'écriture de la case de plus petit indice)
        Some(if self.cell(hex) <= self.cell(other) { BridgeAt::Edge { hex, dir: dir as u8 } } else { BridgeAt::Edge { hex: other, dir: ((dir + 3) % 6) as u8 } })
    }

    /// Pose (`true`) ou retire un pont ; rend vrai si quelque chose a changé. Une case de fleuve n'a
    /// qu'un pont : en poser un autre remplace le premier.
    pub fn set_bridge(&mut self, at: BridgeAt, built: bool) -> bool {
        if built && let BridgeAt::Fleuve { hex, axis } = at {
            self.bridges.retain(|b| !matches!(*b, BridgeAt::Fleuve { hex: h, axis: a } if h == hex && a != axis));
        }
        let changed = if built { self.bridges.insert(at) } else { self.bridges.remove(&at) };
        if changed {
            self.step_cache = std::sync::OnceLock::new();
        }
        changed
    }

    /// Ponts construits.
    pub fn bridges(&self) -> impl Iterator<Item = BridgeAt> + '_ {
        self.bridges.iter().copied()
    }

    /// Un pont franchit la rivière entre deux cases voisines.
    pub fn bridge_between(&self, a: Hex, b: Hex) -> bool {
        self.wrap.direction(a, b).and_then(|d| self.bridge_site(a, d)).is_some_and(|at| matches!(at, BridgeAt::Edge { .. }) && self.bridges.contains(&at))
    }

    /// Axe du pont d'une case de fleuve (0 à 2 : de la voisine `axis` à `axis + 3`), s'il y en a un.
    pub fn fleuve_bridge(&self, hex: Hex) -> Option<usize> {
        let hex = self.canonical(hex);
        (0..3).find(|&axis| self.bridges.contains(&BridgeAt::Fleuve { hex, axis: axis as u8 }))
    }

    /// Pont de pierre (sinon de bois), selon le lieu (choix de Benjamin, 09/11/2026) : en bois si l'une
    /// de ses rives est boisée, ou de famille à muraille de bois (forêt, taïga, jungle, savane,
    /// marais, steppe), comme les murailles des villes ; en pierre sinon. La pierre sera débloquée
    /// partout plus tard.
    pub fn bridge_stone(&self, at: BridgeAt) -> bool {
        let (a, b) = at.ends(self.wrap);
        ![a, b].iter().any(|&h| self.get(h).is_some_and(|t| crate::settlements::Family::of(t).wooden_wall()))
    }

    /// `from` est à un bout du pont de la case de fleuve `hex` (on y monte, on en descend).
    pub fn bridge_end(&self, hex: Hex, from: Hex) -> bool {
        match (self.fleuve_bridge(hex), self.wrap.direction(hex, from)) {
            (Some(axis), Some(d)) => d % 3 == axis,
            _ => false,
        }
    }

    fn index(&self, hex: Hex) -> Option<usize> {
        let (col, row) = self.wrap.hex(hex).to_offset();
        (col >= 0 && col < self.width && row >= 0 && row < self.height)
            .then(|| (row * self.width + col) as usize)
    }

    /// Noms déjà donnés aux lieux de la carte (cours d'eau, lacs, plaques).
    pub fn names_taken(&self) -> Vec<String> {
        self.basins
            .rivers
            .iter()
            .map(|r| r.name.clone())
            .chain(self.basins.lakes.iter().map(|l| l.name.clone()))
            .chain(self.plates.plates.iter().map(|p| p.name.clone()))
            .chain(self.seas.zones.iter().map(|z| z.name.clone()))
            .chain(self.lands.zones.iter().map(|z| z.name.clone()))
            .chain(self.reliefs.zones.iter().map(|z| z.name.clone()))
            .chain(self.regions.zones.iter().map(|z| z.name.clone()))
            .chain(self.banks.iter().map(|b| b.name.clone()))
            .collect()
    }

    /// Ce qu'il faut savoir d'une case pour nommer un lieu qui s'y trouve.
    pub fn cell_info(&self, hex: Hex) -> Option<crate::toponymy::CellInfo> {
        let i = self.index(hex)?;
        Some(crate::toponymy::CellInfo {
            language: self.languages.get(i).copied().unwrap_or(crate::toponymy::Language::Latine),
            temperature: self.climate.temperature.get(i).copied().unwrap_or(15.0),
            aridity: self.climate.aridity(i).unwrap_or(1.0),
            elevation: self.elevation[i],
            terrain: self.tiles[i],
        })
    }

    /// Langues des régions, puis noms des cours d'eau, des lacs et des plaques.
    fn add_names(&mut self, seed: u32) {
        use crate::toponymy::{self, CellInfo, Namer};
        let n = self.tiles.len();
        // Positions sur le cylindre de la carte (rayon 1) : distances de la carte, raccord compris
        let world = self.width as f32;
        let points: Vec<bevy::math::Vec3> =
            (0..n).map(|i| plates::cylinder(plates::map_point(self.hex_at(i), 1.0), world) / (world / std::f32::consts::TAU)).collect();
        let land: Vec<bool> = self.tiles.iter().map(|t| !t.is_water()).collect();
        let climate: Vec<(f32, f32)> =
            (0..n).map(|i| (self.climate.temperature.get(i).copied().unwrap_or(15.0), self.climate.aridity(i).unwrap_or(1.0))).collect();
        self.languages = toponymy::languages(&points, &land, &climate, seed);
        let cells: Vec<CellInfo> = (0..n)
            .map(|i| {
                let h = self.hex_at(i);
                let mut terrain = self.tiles[i];
                // Un lac est caractérisé par ses rives (forêt, désert…)
                if terrain == Terrain::Lake {
                    let shore: Vec<Terrain> = self.neighbors(h).iter().filter_map(|&nb| self.get(nb)).filter(|t| !t.is_water()).collect();
                    if let Some(t) = shore.iter().max_by_key(|t| shore.iter().filter(|u| u == t).count()) {
                        terrain = *t;
                    }
                }
                CellInfo { language: self.languages[i], temperature: climate[i].0, aridity: climate[i].1, elevation: self.elevation[i], terrain }
            })
            .collect();
        let mut namer = Namer::new(seed);
        toponymy::name_waters(&mut self.basins, &cells, &mut namer);
        for p in 0..self.plates.plates.len() {
            let language = self.index(self.plates.plates[p].seed).map_or(toponymy::Language::Latine, |i| self.languages[i]);
            self.plates.plates[p].name = toponymy::plate_name(language, &mut namer);
        }
        // Étendues marines (hors lacs et mers intérieures, déjà nommés), nommées d'après leurs côtes
        let water: Vec<bool> = (0..n).map(|i| self.tiles[i].is_sea() && self.basins.lake_of[i].is_none()).collect();
        let shelf: Vec<bool> = (0..n).map(|i| water[i] && self.depth.get(i).is_some_and(|&d| d < SHELF_DEPTH_M)).collect();
        // Océans polaires : seulement si la carte va jusqu'au pôle (le bord y est le pôle)
        let (north, south) = (self.region.latitude_deg(0, self.height) >= 89.0, self.region.latitude_deg(self.height - 1, self.height) <= -89.0);
        let polar: Vec<f32> = (0..n)
            .map(|i| {
                let lat = self.region.latitude_deg(i as i32 / self.width, self.height);
                match (lat >= 0.0 && north) || (lat < 0.0 && south) {
                    true => lat.abs() - crate::seas::POLAR_OCEAN_LATITUDE,
                    false => f32::NEG_INFINITY,
                }
            })
            .collect();
        self.seas = crate::seas::compute(self.width, self.height, self.wrap, &water, &shelf, &polar);
        // Embouchure de chaque fleuve qui se jette dans la mer : la case de mer voisine
        let mut mouths: Vec<(usize, usize)> = Vec::new();
        for (r, river) in self.basins.rivers.iter().enumerate() {
            if !river.fleuve || river.flows_into.is_some() || river.into_lake.is_some() || river.etymology.is_none() {
                continue;
            }
            let first = self.hex_at(river.first_cell());
            if let Some(sea) = self.neighbors(first).iter().filter_map(|&nb| self.index(nb)).find(|&j| water[j]) {
                mouths.push((sea, r));
            }
        }
        for z in 0..self.seas.zones.len() {
            let zone = &self.seas.zones[z];
            let mut coast: Vec<usize> = zone.cells.iter().flat_map(|&c| self.neighbors(self.hex_at(c))).filter_map(|nb| self.index(nb)).filter(|&j| !water[j]).collect();
            coast.sort_unstable();
            coast.dedup();
            let coast: Vec<&CellInfo> = coast.iter().map(|&j| &cells[j]).collect();
            let latitude = zone.cells.iter().map(|&c| self.latitude(self.hex_at(c))).sum::<f32>() / zone.cells.len() as f32;
            let members: bevy::platform::collections::HashSet<usize> = zone.cells.iter().copied().collect();
            let river = mouths
                .iter()
                .filter(|(sea, _)| members.contains(sea))
                .map(|&(_, r)| &self.basins.rivers[r])
                .max_by(|a, b| a.discharge.total_cmp(&b.discharge))
                .map(|r| (r.name.as_str(), r.feminine));
            let (name, feminine, etymology) = toponymy::sea_name(zone.kind, &coast, latitude, z == 0, river, &mut namer);
            let zone = &mut self.seas.zones[z];
            zone.name = name;
            zone.feminine = feminine;
            zone.etymology = etymology;
        }
        // Terres : continents, îles, archipels, péninsules, isthmes (un lac compte comme de la terre)
        let land: Vec<bool> = water.iter().map(|w| !w).collect();
        self.lands = crate::lands::compute(self.width, self.height, self.wrap, &land);
        // Les archipels d'abord : leurs îles prennent leur langue
        let mut order: Vec<usize> = (0..self.lands.zones.len()).collect();
        order.sort_by_key(|&z| (self.lands.zones[z].kind != crate::lands::LandKind::Archipelago, z));
        for z in order {
            let zone = &self.lands.zones[z];
            let info: Vec<&CellInfo> = zone.cells.iter().map(|&c| &cells[c]).collect();
            let latitude = zone.cells.iter().map(|&c| self.latitude(self.hex_at(c))).sum::<f32>() / zone.cells.len() as f32;
            let language = zone.parent.filter(|&p| self.lands.zones[p].kind == crate::lands::LandKind::Archipelago).and_then(|p| self.lands.zones[p].etymology).map(|e| e.language);
            let (name, feminine, etymology) = toponymy::land_name(zone.kind, &info, latitude, language, &mut namer);
            let zone = &mut self.lands.zones[z];
            zone.name = name;
            zone.feminine = feminine;
            zone.etymology = etymology;
        }
        // Relief : chaînes, massifs, cols (dans cet ordre : un col peut reprendre le nom de sa
        // chaîne), plateaux, collines
        let solid: Vec<bool> = self.tiles.iter().map(|t| !t.is_water()).collect();
        self.reliefs = crate::mountains::compute(self.width, self.height, self.wrap, &solid, &self.relief, &self.elevation);
        let mut words: Vec<Option<(String, toponymy::Etymology)>> = vec![None; self.reliefs.zones.len()];
        for z in 0..self.reliefs.zones.len() {
            let zone = &self.reliefs.zones[z];
            // (un col : les pentes autour de lui disent le pays ; sinon les cases de la zone)
            let around: Vec<usize> = if zone.kind == crate::mountains::ReliefKind::Pass {
                self.range(self.hex_at(zone.cells[0]), 2).filter_map(|h| self.index(h)).collect()
            } else {
                zone.cells.clone()
            };
            let info: Vec<&CellInfo> = around.iter().map(|&c| &cells[c]).collect();
            let latitude = zone.cells.iter().map(|&c| self.latitude(self.hex_at(c))).sum::<f32>() / zone.cells.len() as f32;
            // Chaîne d'un col (par son massif)
            let mut top = zone.parent;
            while let Some(p) = top.and_then(|p| self.reliefs.zones[p].parent) {
                top = Some(p);
            }
            let root = top.and_then(|p| words[p].as_ref()).map(|(w, e)| (w.as_str(), *e));
            let (name, feminine, etymology, word) = toponymy::relief_name(zone.kind, &info, latitude, root, &mut namer);
            words[z] = word.zip(etymology);
            let zone = &mut self.reliefs.zones[z];
            zone.name = name;
            zone.feminine = feminine;
            zone.etymology = etymology;
        }
        // Régions naturelles, puis leurs sous-régions (« Forêt Noire du Nord »)
        self.regions = crate::regions::compute(self.width, self.height, self.wrap, &self.tiles);
        for z in 0..self.regions.zones.len() {
            let zone = &self.regions.zones[z];
            let named = match (zone.parent, zone.side) {
                (Some(p), Some(side)) => {
                    let parent = &self.regions.zones[p];
                    let full = format!("{} {side}", parent.name);
                    namer.claim(&full).then_some((full, parent.feminine, parent.etymology))
                }
                _ => None,
            };
            let (name, feminine, etymology) = named.unwrap_or_else(|| {
                let info: Vec<&CellInfo> = zone.cells.iter().map(|&c| &cells[c]).collect();
                let latitude = zone.cells.iter().map(|&c| self.latitude(self.hex_at(c))).sum::<f32>() / zone.cells.len() as f32;
                toponymy::region_name(zone.biome, &info, latitude, &mut namer)
            });
            let zone = &mut self.regions.zones[z];
            zone.name = name;
            zone.feminine = feminine;
            zone.etymology = etymology;
        }
    }

    /// Profondeur du fond (m) d'une case de mer.
    pub fn depth(&self, hex: Hex) -> Option<f32> {
        self.index(hex).filter(|&i| self.tiles[i].is_sea()).map(|i| self.depth[i])
    }

    /// Eaux poissonneuses d'une case (voir `marine`).
    pub fn marine(&self, hex: Hex) -> u8 {
        self.index(hex).map_or(0, |i| self.marine[i])
    }

    /// Eaux poissonneuses : remontées d'eaux froides le long des côtes où le vent pousse l'eau de
    /// surface vers le large, monts sous-marins (volcans qui montent vers la surface), eaux des
    /// îles volcaniques isolées en haute mer.
    fn add_marine_life(&mut self, seed: u32) {
        let n = self.tiles.len();
        let open_sea = |i: usize| self.tiles[i].is_sea() && self.basins.lake_of[i].is_none();
        let pos = |i: usize| {
            let (x, y) = self.hex_at(i).to_world();
            bevy::math::Vec2::new(x, y)
        };
        let mut marine = vec![0u8; n];
        // Remontées d'eaux : transport d'Ekman (90° à droite du vent au nord, à gauche au sud)
        // dirigé vers le large
        if self.climate.wind.len() == n {
            for i in (0..n).filter(|&i| open_sea(i)) {
                let h = self.hex_at(i);
                let latitude = self.latitude(h);
                if latitude.abs() < 3.0 || latitude.abs() > UPWELLING_MAX_LATITUDE {
                    continue;
                }
                let me = pos(i);
                let mut offshore = bevy::math::Vec2::ZERO;
                for land in self.range(h, 2).filter_map(|nb| self.index(nb)).filter(|&j| !self.tiles[j].is_water()) {
                    let mut d = me - pos(land);
                    let world = self.wrap.world_width();
                    if world > 0.0 {
                        d.x -= (d.x / world).round() * world;
                    }
                    offshore += d.normalize_or_zero();
                }
                let Some(offshore) = offshore.try_normalize() else { continue };
                let (wx, wy) = self.climate.wind[i];
                let ekman = if latitude >= 0.0 { bevy::math::Vec2::new(wy, -wx) } else { bevy::math::Vec2::new(-wy, wx) };
                if ekman.dot(offshore) >= UPWELLING_MIN {
                    marine[i] |= marine::UPWELLING;
                }
            }
        }
        // Rencontres de courants : eau nettement plus chaude et eau nettement plus froide que
        // celle de la latitude à quelques cases l'une de l'autre, sur le plateau ou près des côtes
        if self.currents.anomaly.len() == n {
            let anomaly = &self.currents.anomaly;
            for i in (0..n).filter(|&i| open_sea(i)) {
                let h = self.hex_at(i);
                let near_coast = self.depth[i] < SHELF_DEPTH_M
                    || self.range(h, FRONT_COAST_CELLS).filter_map(|nb| self.index(nb)).any(|j| !self.tiles[j].is_water());
                if !near_coast {
                    continue;
                }
                let around: Vec<f32> = self.range(h, FRONT_RADIUS).filter_map(|nb| self.index(nb)).filter(|&j| self.tiles[j].is_sea()).map(|j| anomaly[j]).collect();
                let (cold, warm) = around.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &a| (lo.min(a), hi.max(a)));
                if cold <= -FRONT_ANOMALY && warm >= FRONT_ANOMALY {
                    marine[i] |= marine::FRONT;
                }
            }
        }
        // Monts sous-marins : fond de haute mer qui remonte nettement au-dessus du fond alentour
        for i in (0..n).filter(|&i| open_sea(i) && self.depth[i] >= SHELF_DEPTH_M && self.depth[i] <= SEAMOUNT_MAX_DEPTH_M) {
            let h = self.hex_at(i);
            // (en pleine mer : le rebord du plateau n'est pas un mont)
            if self.range(h, SEAMOUNT_FROM_SHELF).filter_map(|nb| self.index(nb)).any(|j| self.depth[j] < SHELF_DEPTH_M || !self.tiles[j].is_sea()) {
                continue;
            }
            let ring: Vec<f32> = self.range(h, 2).filter(|&nb| self.distance(h, nb) == 2).filter_map(|nb| self.index(nb)).filter(|&j| open_sea(j)).map(|j| self.depth[j]).collect();
            if ring.len() >= 8 && ring.iter().sum::<f32>() / ring.len() as f32 - self.depth[i] >= SEAMOUNT_RISE_M {
                marine[i] |= marine::SEAMOUNT;
            }
        }
        // Îles volcaniques isolées : petite terre sur une plaque océanique ; ses eaux de haute mer
        let mut seen = vec![false; n];
        for start in 0..n {
            if seen[start] || self.tiles[start].is_water() {
                continue;
            }
            let mut island = vec![start];
            seen[start] = true;
            let mut k = 0;
            while k < island.len() && island.len() <= ISLAND_MAX_CELLS {
                for nb in self.neighbors(self.hex_at(island[k])) {
                    if let Some(j) = self.index(nb)
                        && !seen[j]
                        && !self.tiles[j].is_water()
                    {
                        seen[j] = true;
                        island.push(j);
                    }
                }
                k += 1;
            }
            let oceanic = island.iter().any(|&c| self.plates.crust.get(c) == Some(&plates::PlateKind::Oceanic));
            if island.len() > ISLAND_MAX_CELLS || !oceanic {
                // (grande terre : finir de la marquer)
                while k < island.len() {
                    for nb in self.neighbors(self.hex_at(island[k])) {
                        if let Some(j) = self.index(nb)
                            && !seen[j]
                            && !self.tiles[j].is_water()
                        {
                            seen[j] = true;
                            island.push(j);
                        }
                    }
                    k += 1;
                }
                continue;
            }
            for &c in &island {
                for nb in self.neighbors(self.hex_at(c)) {
                    if let Some(j) = self.index(nb)
                        && open_sea(j)
                    {
                        marine[j] |= marine::ISLAND;
                    }
                }
            }
        }
        self.marine = marine;
        // Bancs de poissons : eaux poissonneuses voisines, d'un seul tenant, nommées dans la langue
        // du lieu
        let mut namer = crate::toponymy::Namer::with_taken(seed ^ 0xF15B, self.names_taken());
        let mut bank_of: Vec<Option<u16>> = vec![None; n];
        let mut banks = Vec::new();
        for start in 0..n {
            if self.marine[start] == 0 || bank_of[start].is_some() {
                continue;
            }
            let id = banks.len() as u16;
            let mut cells = vec![start];
            bank_of[start] = Some(id);
            let mut k = 0;
            while k < cells.len() {
                for nb in self.neighbors(self.hex_at(cells[k])) {
                    if let Some(j) = self.index(nb)
                        && self.marine[j] != 0
                        && bank_of[j].is_none()
                    {
                        bank_of[j] = Some(id);
                        cells.push(j);
                    }
                }
                k += 1;
            }
            cells.sort_unstable();
            let origins = cells.iter().fold(0u8, |acc, &c| acc | self.marine[c]);
            // (langue et eau du lieu : celles de la case du milieu de la liste)
            let info = self.cell_info(self.hex_at(cells[cells.len() / 2])).expect("banc sur la carte");
            let name = crate::toponymy::bank_name(&info, origins, &mut namer);
            banks.push(FishingBank { cells, origins, name });
        }
        self.banks = banks;
        self.bank_of = bank_of;
    }

    /// Bancs de poissons.
    pub fn banks(&self) -> &[FishingBank] {
        &self.banks
    }

    /// Banc de poissons d'une case.
    pub fn bank(&self, hex: Hex) -> Option<usize> {
        self.index(hex).and_then(|i| self.bank_of[i]).map(usize::from)
    }

    /// Étendues marines nommées.
    pub fn seas(&self) -> &crate::seas::Seas {
        &self.seas
    }

    /// Terres nommées.
    pub fn lands(&self) -> &crate::lands::Lands {
        &self.lands
    }

    /// Relief nommé.
    pub fn reliefs(&self) -> &crate::mountains::Reliefs {
        &self.reliefs
    }

    /// Régions naturelles.
    pub fn regions(&self) -> &crate::regions::Regions {
        &self.regions
    }

    /// Axe d'une zone allongée (chaîne, golfe…), pour y courber son nom : le plus court chemin dans
    /// la zone entre ses deux cases les plus éloignées l'une de l'autre (par la zone). Cases
    /// d'un seul tenant d'un bout à l'autre, raccord est-ouest compris (non ramenées dans la
    /// carte). Aucun si la zone est ramassée : son axe doit être long par rapport à sa surface
    /// (longueur² ≥ `SPINE_ELONGATION` × cases ; un disque donne ~1,3), d'au moins 5 cases.
    pub fn zone_spine(&self, cells: &[usize]) -> Vec<Hex> {
        let Some(&first) = cells.first() else { return Vec::new() };
        let members: std::collections::HashSet<usize> = cells.iter().copied().collect();
        // Parcours en largeur dans la zone : case la plus éloignée et chemin pour y aller
        let walk = |from: usize| -> (usize, std::collections::HashMap<usize, usize>) {
            let mut parent: std::collections::HashMap<usize, usize> = std::collections::HashMap::from([(from, from)]);
            let mut queue = std::collections::VecDeque::from([from]);
            let mut last = from;
            while let Some(c) = queue.pop_front() {
                last = c;
                for nb in self.neighbors(self.hex_at(c)) {
                    if let Some(j) = self.index(nb)
                        && members.contains(&j)
                        && !parent.contains_key(&j)
                    {
                        parent.insert(j, c);
                        queue.push_back(j);
                    }
                }
            }
            (last, parent)
        };
        let (a, _) = walk(first);
        let (b, parent) = walk(a);
        let mut path = vec![b];
        while let Some(&p) = parent.get(path.last().unwrap()) {
            if p == *path.last().unwrap() {
                break;
            }
            path.push(p);
        }
        if path.len() < 5 || ((path.len() * path.len()) as f32) < SPINE_ELONGATION * cells.len() as f32 {
            return Vec::new();
        }
        // D'un seul tenant : chaque case au plus près de la précédente (raccord est-ouest)
        let mut hexes: Vec<Hex> = vec![self.hex_at(path[0])];
        for &c in &path[1..] {
            let prev = *hexes.last().unwrap();
            hexes.push(self.wrap.nearest(prev, self.hex_at(c)));
        }
        hexes
    }

    /// Lieux nommés d'une case, du plus vaste au plus précis (barre de lieu) : en mer, océan, mer,
    /// golfe, baie, détroit ; sur un lac, le lac ; sur terre, archipel, continent ou île,
    /// péninsule, isthme, région naturelle et sous-région, chaîne, massif, col, plateau, collines ;
    /// puis le grand bassin versant. Chacun avec son nom, sans doublon de nom.
    pub fn places(&self, hex: Hex) -> Vec<(Place, String)> {
        let Some(c) = self.index(hex) else { return Vec::new() };
        let mut places: Vec<(Place, String)> = Vec::new();
        let mut push = |place: Place, name: &str| {
            if !name.is_empty() && !places.iter().any(|(_, n)| n == name) {
                places.push((place, name.to_string()));
            }
        };
        // (chaque famille donne ses zones de la plus précise à la plus vaste : on les retourne)
        for z in self.seas.chain(c).into_iter().rev() {
            push(Place::Sea(z), &self.seas.zones[z].name);
        }
        if let Some(l) = self.basins.lake_of.get(c).copied().flatten() {
            push(Place::Lake(usize::from(l)), &self.basins.lakes[usize::from(l)].name);
        }
        for z in self.lands.chain(c).into_iter().rev() {
            push(Place::Land(z), &self.lands.zones[z].name);
        }
        for z in self.regions.chain(c).into_iter().rev() {
            push(Place::Region(z), &self.regions.zones[z].name);
        }
        for z in self.reliefs.chain(c).into_iter().rev() {
            push(Place::Relief(z), &self.reliefs.zones[z].name);
        }
        if let Some(b) = self.basins.basin_of.get(c).copied().flatten()
            && let Some(basin) = self.basins.basins.get(b as usize).filter(|b| b.major())
            && let Some(title) = self.basins.basin_title(basin)
        {
            push(Place::Basin(b as usize), &title);
        }
        places
    }

    /// Plaques tectoniques.
    pub fn plates(&self) -> &Plates {
        &self.plates
    }

    /// Plaque d'une case.
    pub fn plate(&self, hex: Hex) -> Option<&Plate> {
        self.index(hex).and_then(|i| self.plates.plate(i))
    }

    /// Vitesse de la plaque à la case (cm/an ; x vers l'est, y vers le nord).
    pub fn plate_velocity(&self, hex: Hex) -> Option<bevy::math::Vec2> {
        self.index(hex).and_then(|i| self.plates.velocity.get(i).copied())
    }

    /// Frontière de plaques la plus active qui touche la case.
    pub fn plate_boundary(&self, hex: Hex) -> Option<plates::Boundary> {
        self.index(hex).and_then(|i| self.plates.boundary.get(i).copied().flatten())
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
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
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
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex) {
            self.tiles[i] = terrain;
            self.levels[i] = if terrain.is_sea() { 0 } else { self.levels[i].max(1) };
            if terrain.is_water() {
                self.relief[i] = Relief::Flat;
            }
            // (une terre devenue mer, ou l'inverse, le reste si l'on refait le climat)
            if let Some(b) = self.climate_base.as_mut() {
                b.levels[i] = if terrain.is_sea() { 0 } else { b.levels[i].max(1) };
                if terrain.is_sea() {
                    b.relief[i] = Relief::Flat;
                }
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

    /// Change le relief (seulement sur la terre ; un fleuve reste plat) (tests, console de debug).
    pub fn set_relief(&mut self, hex: Hex, relief: Relief) {
        self.step_cache = std::sync::OnceLock::new();
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex) {
            self.relief[i] = if self.tiles[i].is_water() || self.hydro.fleuve[i] { Relief::Flat } else { relief };
            // (relief voulu gardé pour la terre, même sous un fleuve qui l'aplanit aujourd'hui)
            if let Some(b) = self.climate_base.as_mut()
                && !self.tiles[i].is_sea()
            {
                b.relief[i] = relief;
            }
        }
    }

    /// Relief que la génération donnerait à une case de terre avec les altitudes actuelles
    /// (note d'escarpement comparée aux seuils de la carte ; un fleuve est plat) ; les pics à
    /// part (`could_be_peak`). `None` : eau, ou carte sans règle du relief.
    pub fn natural_relief(&self, hex: Hex) -> Option<Relief> {
        let rule = self.relief_rule.as_ref()?;
        let i = self.index(hex)?;
        if self.tiles[i].is_water() {
            return None;
        }
        if self.hydro.fleuve[i] {
            return Some(Relief::Flat);
        }
        let local = self
            .wrap
            .neighbors(hex)
            .iter()
            .filter_map(|&nb| self.index(nb))
            // (les lacs viennent après le relief : leur fond compte comme une terre, comme alors)
            .filter(|&j| !self.tiles[j].is_sea())
            .map(|j| (self.elevation[i] - self.elevation[j]).abs())
            .fold(0.0, f32::max);
        let (local_rank, rough_rank) = match rule.generated[i] {
            Some((before, l, r)) if before == local => (l, r),
            Some((_, _, r)) => (rank_in(&rule.local_sorted, local), r),
            None => (rank_in(&rule.local_sorted, local), rank_in(&rule.rough_sorted, rule.rough[i])),
        };
        let score = relief_score(local_rank, rough_rank, self.elevation[i]);
        Some(if score >= rule.thresholds[0] {
            if self.elevation[i] >= MOUNTAIN_MIN_M { Relief::Mountain } else { Relief::Rugged }
        } else if score >= rule.thresholds[1] {
            Relief::Rugged
        } else if score >= rule.thresholds[2] {
            Relief::Hills
        } else {
            Relief::Flat
        })
    }

    /// Une montagne pourrait être un pic : assez haute (au moins `PEAK_MIN_M` et le plus bas des
    /// pics de la génération) et sommet local (aucune voisine plus haute de `PEAK_TOLERANCE_M`).
    /// La règle « jamais deux pics côte à côte » est à vérifier à part.
    pub fn could_be_peak(&self, hex: Hex) -> bool {
        self.relief(hex) == Relief::Mountain && self.could_be_peak_ignoring_relief(hex)
    }

    /// Hauteur et position d'un pic (`could_be_peak` sans regarder le relief actuel).
    pub fn could_be_peak_ignoring_relief(&self, hex: Hex) -> bool {
        let (Some(rule), Some(i)) = (self.relief_rule.as_ref(), self.index(hex)) else { return false };
        let e = self.elevation[i];
        e >= PEAK_MIN_M
            && e >= rule.peak_floor_m
            && self.wrap.neighbors(hex).iter().filter_map(|&nb| self.index(nb)).all(|j| self.elevation[j] <= e + PEAK_TOLERANCE_M)
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
        if t.is_sea() && self.marine(hex) != 0 {
            y.food += RICH_WATERS_FOOD;
        }
        Some(y)
    }

    /// Nom complet d'une case : « Prairie vallonnée », « Désert vallonné », « Forêt »,
    /// « Fleuve (rives de prairie) », « Plaine, petit lac »…
    pub fn describe(&self, hex: Hex) -> String {
        let Some(t) = self.get(hex) else { return String::new() };
        if self.is_ice(hex) {
            return "Banquise".to_string();
        }
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

    /// Change l'altitude (m) d'une case de terre, et sa classe d'altitude (tests, console de debug).
    pub fn set_elevation(&mut self, hex: Hex, meters: f32) {
        self.step_cache = std::sync::OnceLock::new();
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex)
            && !self.tiles[i].is_water()
        {
            self.elevation[i] = meters;
            self.levels[i] = level_of_meters(meters);
            if let Some(b) = self.climate_base.as_mut() {
                b.levels[i] = self.levels[i];
            }
        }
    }

    /// Change l'altitude (m) de la surface d'un lac sur une case (console de debug).
    pub fn set_lake_level(&mut self, hex: Hex, meters: f32) {
        if let Some(i) = self.index(hex) {
            self.lake_level_m[i] = meters;
        }
    }

    /// Falaise entre deux cases voisines de terre ou de lac (dénivelée, voir `is_cliff`).
    pub fn cliff_between(&self, a: Hex, b: Hex) -> bool {
        let land = |h: Hex| self.get(h).filter(|t| !t.is_sea()).and(self.surface_elevation(h));
        matches!((land(a), land(b)), (Some(ha), Some(hb)) if is_cliff(ha, hb))
    }

    /// Falaise entre deux cases voisines, la mer comptant à 0 m (côte escarpée, pour le dessin).
    pub fn cliff_with_sea(&self, a: Hex, b: Hex) -> bool {
        matches!((self.surface_elevation(a), self.surface_elevation(b)), (Some(ha), Some(hb)) if is_cliff(ha, hb))
    }

    /// Coût pour entrer dans la case, sans la pente ni la rivière de l'arête franchie
    /// (fleuve : de l'eau ; montagne : selon l'altitude). `None` = infranchissable.
    pub fn enter_cost(&self, hex: Hex) -> Option<StepCost> {
        if self.is_fleuve(hex) {
            return Some(StepCost { water: FLEUVE_MOVE_COST, ..StepCost::default() });
        }
        if self.is_ice(hex) {
            return Some(StepCost::terrain(ICE_MOVE_COST));
        }
        let base = self.get(hex)?.move_cost()?;
        match self.relief(hex) {
            Relief::Peak => None,
            Relief::Mountain => {
                Some(StepCost { terrain: mountain_move_cost(self.elevation(hex)?), mountain: true, ..StepCost::default() })
            }
            r => Some(StepCost::terrain((base + r.move_extra()).min(RELIEF_MOVE_CAP.max(base)))),
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

    /// Graine de la génération (0 pour une carte remplie à la main).
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Turbidité de l'eau qui sort de la case (0 : claire, 1 : très chargée en limon) : moyenne
    /// des apports de son bassin pondérée par leur débit (voir `murk_of_cells`).
    pub fn water_murk(&self, hex: Hex) -> f32 {
        let all = self.murk_cache.get_or_init(|| {
            let n = self.tiles.len();
            let down = (0..n)
                .map(|i| self.hydro.downstream[i].and_then(|d| self.index(self.wrap.neighbor(self.hex_at(i), d as usize))))
                .collect::<Vec<_>>();
            let yields = (0..n).map(|i| sediment_yield(self.tiles[i], self.elevation[i])).collect::<Vec<_>>();
            murk_of_cells(&down, &self.hydro.discharge, &yields, &self.hydro.lake)
        });
        self.index(hex).map_or(0.0, |i| all[i])
    }

    /// Coin vers lequel coule la rivière de l'arête `dir` de la case : la case dont il porte le
    /// numéro (`hex` ou sa voisine d'en face, non ramenée dans la carte) et ce numéro (0 à 5 ; le
    /// coin j est commun à la case et à ses voisines des directions j−1 et j). `None` sans tronçon
    /// orienté (rivière posée à la main).
    pub fn river_down_corner(&self, hex: Hex, dir: usize) -> Option<(Hex, usize)> {
        let all = self.river_down_cache.get_or_init(|| {
            let mut all = vec![[u8::MAX; 6]; self.tiles.len()];
            for s in &self.hydro.river_segments {
                if let Some(row) = all.get_mut(s.cell as usize) {
                    row[s.dir as usize % 6] = s.down_corner;
                }
            }
            all
        });
        let dir = dir % 6;
        if let Some(j) = self.index(hex).map(|i| all[i][dir]).filter(|&j| j != u8::MAX) {
            return Some((hex, j as usize));
        }
        let other = hex + crate::hex::DIRECTIONS[dir];
        self.index(other).map(|i| all[i][(dir + 3) % 6]).filter(|&j| j != u8::MAX).map(|j| (other, j as usize))
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
    /// Roche de surface d'une case de terre (`geologie.rs`).
    pub fn roche(&self, hex: Hex) -> Option<crate::geologie::Roche> {
        self.index(hex).and_then(|i| self.geologie().get(i).copied().flatten()).map(|g| g.0)
    }

    /// Classe de sol d'une case de terre (fertilité des champs).
    pub fn sol(&self, hex: Hex) -> Option<crate::geologie::Sol> {
        self.index(hex).and_then(|i| self.geologie().get(i).copied().flatten()).map(|g| g.1)
    }

    /// Le calcaire de la case est karstique (grottes, gouffres).
    pub fn karstique(&self, hex: Hex) -> bool {
        self.roche(hex).is_some_and(|r| crate::geologie::karstique(r, self.rain(hex).unwrap_or(0.0)))
    }

    /// Roche et sol de toutes les cases (calculés une fois, puis gardés jusqu'à une modification).
    fn geologie(&self) -> &[Option<(crate::geologie::Roche, crate::geologie::Sol)>] {
        self.geologie_cache.get_or_init(|| {
            let n = self.tiles.len();
            // (carte remplie à la main, sans climat : pas de géologie, champs sans fertilité)
            if self.climate.temperature.len() != n {
                return vec![None; n];
            }
            // Bruit lent (une région de quelques centaines de km) : départage calcaire et argile
            let noise: Fbm<Perlin> = Fbm::new(self.seed.wrapping_add(707_070)).set_octaves(2).set_frequency(1.0);
            let f = 1.0 / 12.0;
            let radius = self.width as f64 / std::f64::consts::TAU;
            let wraps = self.wrap.wraps();
            let bruit = |h: Hex| {
                let (col, row) = h.to_offset();
                let (col, row) = (col as f64, row as f64);
                let v = if wraps {
                    let a = col / self.width as f64 * std::f64::consts::TAU;
                    noise.get([a.cos() * radius * f, a.sin() * radius * f, row * f])
                } else {
                    noise.get([col * f, row * f])
                };
                (v as f32 * 1.6).clamp(-1.0, 1.0)
            };
            let plaques = !self.plates.edges.is_empty();
            let cell_km = self.region.cell_km;
            // Taille de la terre d'un seul tenant de chaque case (les îles volcaniques sont petites)
            let mut terre_de = vec![usize::MAX; n];
            let mut tailles = Vec::new();
            for depart in 0..n {
                if terre_de[depart] != usize::MAX || self.tiles[depart].is_water() {
                    continue;
                }
                let id = tailles.len();
                let mut pile = vec![depart];
                terre_de[depart] = id;
                let mut taille = 0usize;
                while let Some(i) = pile.pop() {
                    taille += 1;
                    for v in self.neighbors(self.hex_at(i)) {
                        if let Some(j) = self.index(self.canonical(v))
                            && terre_de[j] == usize::MAX
                            && !self.tiles[j].is_water()
                        {
                            terre_de[j] = id;
                            pile.push(j);
                        }
                    }
                }
                tailles.push(taille);
            }
            (0..n)
                .map(|i| {
                    let t = self.tiles[i];
                    if t.is_water() {
                        return None;
                    }
                    let h = self.hex_at(i);
                    let frontiere = plaques.then(|| {
                        let e = &self.plates.edges[self.plates.nearest_edge[i] as usize];
                        (e.kind, self.plates.boundary_km[i], self.plates.plate_of[i] != e.under)
                    });
                    let point_chaud_km = self.plates.hotspots.iter().map(|&p| self.wrap.distance(h, p) as f32 * cell_km).fold(f32::MAX, f32::min);
                    let grande_riviere = (0..6).any(|d| self.river_flow(h, d) > 300.0) || self.neighbors(h).iter().any(|&v| self.is_fleuve(v));
                    let lieu = crate::geologie::Lieu {
                        ile_oceanique: plaques && !self.plates.continental(i) && tailles.get(terre_de[i]).is_some_and(|&t| t < crate::geologie::ILE_CASES),
                        frontiere,
                        point_chaud_km,
                        altitude: self.elevation[i],
                        relief: self.relief[i],
                        temperature: self.climate.temperature.get(i).copied().unwrap_or(12.0),
                        pluie: self.rain(h).unwrap_or(700.0),
                        saisons_seches: self.climate.dry_seasons(i).map_or(0, |s| s.iter().filter(|&&d| d).count() as u8),
                        terrain: t,
                        fleuve_ou_delta: self.hydro.fleuve.get(i).copied().unwrap_or(false) || self.delta_cells.iter().any(|c| c.0 == i),
                        grande_riviere,
                        bruit: bruit(h),
                    };
                    let r = crate::geologie::roche(&lieu);
                    Some((r, crate::geologie::sol(r, &lieu)))
                })
                .collect()
        })
    }

    pub fn fresh_water(&self, hex: Hex) -> bool {
        self.small_lake(hex)
            || (0..6).any(|d| self.river_flow(hex, d) > 0.0)
            || self.neighbors(hex).iter().any(|&n| self.is_fleuve(n) || self.get(n) == Some(Terrain::Lake))
    }

    /// Fait couler un fleuve de `hex` vers sa voisine `dir` (débit en m³/s) : les cases de terre
    /// deviennent fleuve (plates), liées par leur arête, l'aval de `hex` est la voisine (console
    /// de debug ; bassins et noms ne suivent pas).
    pub fn link_fleuve(&mut self, hex: Hex, dir: usize, flow: f32) {
        self.step_cache = std::sync::OnceLock::new();
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
        let to = self.wrap.neighbor(hex, dir);
        let (Some(i), Some(j)) = (self.index(hex), self.index(to)) else { return };
        for (k, d) in [(i, dir), (j, (dir + 3) % 6)] {
            if self.tiles[k].is_water() {
                continue;
            }
            self.hydro.fleuve[k] = true;
            self.hydro.fleuve_links[k] |= 1 << d;
            self.hydro.discharge[k] = self.hydro.discharge[k].max(flow);
            self.relief[k] = Relief::Flat;
        }
        if !self.tiles[i].is_water() {
            self.hydro.downstream[i] = Some(dir as u8);
        }
    }

    /// Fait d'une case un fleuve (tests, éditeur).
    #[cfg(test)]
    pub fn set_fleuve(&mut self, hex: Hex) {
        self.step_cache = std::sync::OnceLock::new();
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
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

    /// Change le débit sortant d'une case (tests).
    #[cfg(test)]
    pub fn set_discharge(&mut self, hex: Hex, flow: f32) {
        if let Some(i) = self.index(hex) {
            self.hydro.discharge[i] = flow;
        }
    }

    /// Oriente la rivière de l'arête `dir` vers le coin `down_corner` de la case (tests).
    #[cfg(test)]
    pub fn orient_river(&mut self, hex: Hex, dir: usize, down_corner: usize) {
        self.river_down_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex) {
            let flow = self.hydro.edge_flow[i][dir];
            self.hydro.river_segments.push(crate::hydro::RiverSegment { cell: i as u32, dir: dir as u8, up: 0, down: 0, down_corner: down_corner as u8, flow });
        }
    }

    /// Pose une rivière sur l'arête entre une case et sa voisine `dir` (tests, éditeur).
    #[cfg(test)]
    pub fn set_river(&mut self, hex: Hex, dir: usize, flow: f32) {
        self.step_cache = std::sync::OnceLock::new();
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
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

    /// Température de la saison la plus chaude (°C).
    pub fn warmest_season(&self, hex: Hex) -> Option<f32> {
        self.index(hex).and_then(|i| self.climate.coldest_warmest(i)).map(|t| t.1)
    }

    /// Température moyenne annuelle (°C).
    pub fn temperature(&self, hex: Hex) -> Option<f32> {
        self.index(hex).and_then(|i| self.climate.temperature.get(i).copied())
    }

    /// Change la température moyenne annuelle (°C) d'une case simulée, saisons, minimales et
    /// maximales décalées d'autant (console de debug ; l'évaporation potentielle ne suit pas).
    pub fn set_temperature(&mut self, hex: Hex, celsius: f32) {
        let Some(i) = self.index(hex) else { return };
        let c = &mut self.climate;
        if let Some(t) = c.temperature.get_mut(i)
        {
            let delta = celsius - *t;
            *t = celsius;
            for v in [c.temp_min.get_mut(i), c.temp_max.get_mut(i)].into_iter().flatten() {
                *v += delta;
            }
            for s in c.season_temp.get_mut(i).into_iter().flatten() {
                *s += delta;
            }
        }
    }

    /// Change les précipitations annuelles (mm) d'une case simulée, celles de chaque saison
    /// dans la même proportion (console de debug).
    pub fn set_rain(&mut self, hex: Hex, mm: f32) {
        let Some(i) = self.index(hex) else { return };
        let c = &mut self.climate;
        if let Some(r) = c.rain.get_mut(i) {
            let old = *r;
            *r = mm;
            // (sans pluie avant, la pluie nouvelle tombe également en toute saison)
            for s in c.season_rain.get_mut(i).into_iter().flatten() {
                *s = if old > 0.0 { *s * mm / old } else { mm };
            }
        }
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

    /// Courant de surface d'une case de mer (m/s, x vers l'est, y vers le nord).
    pub fn current(&self, hex: Hex) -> Option<(f32, f32)> {
        self.index(hex).filter(|&i| self.tiles[i].is_sea()).and_then(|i| self.currents.velocity.get(i).copied())
    }

    /// Température de surface d'une case de mer (°C) et son écart à celle de la latitude.
    pub fn sea_temperature(&self, hex: Hex) -> Option<(f32, f32)> {
        self.index(hex)
            .filter(|&i| self.tiles[i].is_sea())
            .and_then(|i| Some((*self.currents.temperature.get(i)?, *self.currents.anomaly.get(i)?)))
    }

    /// Courants marins de toute la carte.
    /// Marées de toutes les cases (amplitude, heure de la pleine mer, courant).
    pub fn tides(&self) -> &crate::tides::Tides {
        &self.tides
    }

    /// Houle de toutes les cases (hauteur, période, direction, côtes).
    pub fn waves(&self) -> &crate::waves::Waves {
        &self.waves
    }

    /// Banquise : mer gelée toute l'année. Les unités y marchent (`enter_cost`), mais aucune ville ni
    /// structure n'y prend place.
    pub fn is_ice(&self, hex: Hex) -> bool {
        self.index(hex).is_some_and(|i| self.tiles[i].is_sea() && self.ice.get(i).copied().unwrap_or(false))
    }

    /// Embouchure de fleuve de la case (fleuve qui se jette dans la mer), avec sa forme.
    pub fn mouth(&self, hex: Hex) -> Option<&crate::mouths::Mouth> {
        let i = self.index(hex)?;
        self.mouths.binary_search_by_key(&i, |m| m.0).ok().map(|k| &self.mouths[k].1)
    }

    /// Toutes les embouchures de fleuve : (case, forme) (bilan).
    #[cfg(test)]
    pub fn mouths(&self) -> &[(usize, crate::mouths::Mouth)] {
        &self.mouths
    }

    /// Forme de chaque embouchure de fleuve, d'après le fleuve, la mer devant et le relief autour.
    fn classify_mouths(&self) -> Vec<(usize, crate::mouths::Mouth)> {
        let mut out = Vec::new();
        for i in 0..self.tiles.len() {
            let h = self.hex_at(i);
            if !self.is_fleuve(h) {
                continue;
            }
            let Some(front) = self.downstream(h).map(|d| h + crate::hex::DIRECTIONS[d]) else { continue };
            let Some(f) = self.index(front).filter(|&f| self.tiles[f].is_sea()) else { continue };
            // Fond trois cases plus loin, tant que la mer continue dans la même direction
            let step = front - h;
            let far = (1..=3).map(|k| (0..k).fold(front, |p, _| p + step)).take_while(|&p| self.depth(p).is_some()).last().and_then(|p| self.depth(p));
            let relief = self.range(h, 2).filter(|&p| self.get(p).is_some_and(|t| !t.is_water())).filter_map(|p| self.elevation(p)).fold(0.0, f32::max);
            let w = &self.waves;
            let (wave_height, wave_period) = if f < w.height.len() { (w.shore_deep_height[f], w.shore_period[f]) } else { (0.0, 8.0) };
            let input = crate::mouths::MouthInput {
                discharge: self.discharge(h),
                murk: self.water_murk(h),
                tidal_range: self.tides.spring_range(f),
                wave_height,
                wave_period,
                front_depth: self.depth[f],
                far_depth: far.unwrap_or(self.depth[f]),
                relief,
                temperature: self.temperature(h).unwrap_or(15.0),
            };
            let mut mouth = crate::mouths::classify(&input);
            // (la terre gagnée ne change pas la nature de son delta : le fond est différent devant)
            if let Some(&(_, kind)) = self.delta_cells.iter().find(|c| c.0 == i) {
                mouth.kind = kind;
            }
            out.push((i, mouth));
        }
        out
    }

    /// Fixe la direction aval d'une case (tests).
    #[cfg(test)]
    pub fn set_downstream(&mut self, hex: Hex, dir: usize) {
        let i = self.cell(hex);
        self.hydro.downstream[i] = Some(dir as u8);
    }

    /// Donne une forme à l'embouchure d'une case (tests).
    #[cfg(test)]
    pub fn set_mouth(&mut self, hex: Hex, kind: crate::mouths::MouthKind) {
        let i = self.cell(hex);
        self.mouths.retain(|m| m.0 != i);
        self.mouths.push((i, crate::mouths::Mouth { kind, river: 0.0, wave: 0.0, tide: 0.0 }));
        self.mouths.sort_by_key(|m| m.0);
    }

    /// Gèle une case de mer (tests).
    #[cfg(test)]
    pub fn freeze(&mut self, hex: Hex) {
        let (n, i) = (self.tiles.len(), self.cell(hex));
        self.ice.resize(n, false);
        self.ice[i] = true;
        self.step_cache = std::sync::OnceLock::new();
    }

    /// Case de mer qui a sa houle calculée (toutes les mers d'un monde généré), et son indice.
    pub fn wave_cell(&self, hex: Hex) -> Option<usize> {
        self.index(hex).filter(|&i| self.tiles[i].is_sea() && i < self.waves.height.len())
    }

    /// Marnage de vives-eaux d'une case de mer (m).
    pub fn tidal_range(&self, hex: Hex) -> Option<f32> {
        self.index(hex).filter(|&i| self.tiles[i].is_sea()).map(|i| self.tides.spring_range(i))
    }

    /// Courant de marée au plus fort d'une case de mer (m/s, vives-eaux).
    pub fn tidal_current(&self, hex: Hex) -> Option<f32> {
        self.index(hex).filter(|&i| self.tiles[i].is_sea()).and_then(|i| self.tides.current.get(i).copied())
    }

    pub fn currents(&self) -> &crate::currents::Currents {
        &self.currents
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
                    let cliff = self.cliff_between(high, high + *d);
                    edges.push(LevelEdge { high, dir, diff: hl - ll, cliff });
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
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
        if let Some(i) = self.index(hex)
            && !self.tiles[i].is_water()
        {
            self.levels[i] = level.clamp(1, MAX_LEVEL);
            self.elevation[i] = LEVEL_ALTITUDE_M[(self.levels[i] - 1) as usize];
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
        // 0) Plaques tectoniques (décrites seulement, pour l'instant : carte « Plaques »)
        map.plates = plates::generate(width, height, wrap, seed, region.cell_km);
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
        // 1) Élévation brute : sur une planète, issue des plaques (continents, chaînes, fosses, arcs
        //    d'îles, points chauds ; la mer sous 0) ; sur une carte régionale, bruit fractal atténué
        //    vers les bords
        let tectonic = wrap.wraps();
        let tectonic_e = if tectonic {
            let mut e = map.plates.elevation(seed, region.cell_km);
            // Des vallées dans les terres hautes, pour que l'eau y converge (voir erosion.rs)
            let valley_noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(271_828)).set_octaves(2).set_frequency(1.0);
            for (i, v) in e.iter_mut().enumerate() {
                *v = crate::erosion::with_valleys(*v, sample(&valley_noise, map.hex_at(i), crate::erosion::VALLEY_ZOOM));
            }
            // L'eau creuse le relief : vallées, cuvettes vidées ou comblées, cols (voir erosion.rs)
            let neighbors = map.plates.neighbor_graph(region.cell_km);
            // (l'eau qui atteint le bord, aux pôles, s'en va : la terre continue au-delà)
            let outlet: Vec<bool> = (0..e.len()).map(|i| matches!(i as i32 / width, r if r == 0 || r == height - 1)).collect();
            crate::erosion::erode(&mut e, &neighbors, &map.plates.cell_area(), &outlet, crate::erosion::ErosionParams::default());
            // Au moins un passage maritime fait le tour de la planète (sinon, une chaîne d'îles
            // ou une terre qui le ferme est rabotée près d'un pôle)
            plates::open_sea_passage(&mut e, width);
            e
        } else {
            Vec::new()
        };
        let elevations: Vec<(Hex, f64)> = map
            .hexes()
            .enumerate()
            .map(|(i, h)| {
                if tectonic {
                    return (h, tectonic_e[i]);
                }
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
        // 2) Niveau de la mer : 0 sur une planète, percentile (~58 % d'eau) sur une carte régionale
        let mut sorted: Vec<f64> = elevations.iter().map(|(_, e)| *e).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let sea = if tectonic { 0.0 } else { sorted[((sorted.len() - 1) as f64 * 0.58) as usize] };
        let land_sorted: Vec<f64> = sorted.iter().copied().filter(|&e| e >= sea).collect();
        // Mares de mer : petites étendues de mer (moins de `MIN_INLAND_SEA_CELLS` cases) enfermées
        // dans les terres. Une case de mer isolée qui avalerait de grands fleuves n'a pas de sens :
        // elles deviennent des cuvettes de terre, que l'hydrologie remplit (lac qui déborde vers
        // la vraie mer, ou cuvette sèche en climat aride)
        let ponds: bevy::platform::collections::HashSet<Hex> = {
            let wet = |h: Hex| map.index(h).is_some_and(|i| elevations[i].1 < sea);
            let mut seen = bevy::platform::collections::HashSet::new();
            let mut ponds = bevy::platform::collections::HashSet::new();
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
        // Altitude en mètres : échelle fixe sur une planète (`TECTONIC_METERS`) ; sur une carte
        // régionale, selon le rang parmi les terres émergées (`REGION_METERS`, jusqu'aux sommets)
        let top = land_sorted.last().copied().unwrap_or(sea + 1.0);
        let region_knots: Vec<(f64, f32)> = REGION_METERS
            .iter()
            .map(|&(p, m)| (land_sorted[((land_sorted.len().max(1) - 1) as f64 * p) as usize], m))
            .chain([(top + 1e-6, PEAK_M)])
            .collect();
        let meters = |e: f64| if tectonic { piecewise(&TECTONIC_METERS, e) } else { piecewise(&region_knots, e).min(PEAK_M) }.max(0.0);
        // 3) Altitude (m) de chaque case de terre, puis son niveau (`level_of_meters`)
        for &(h, e) in &elevations {
            if is_sea(h, e) {
                continue; // reste océan
            }
            let i = map.index(h).unwrap();
            map.elevation[i] = if ponds.contains(&h) { POND_DEPTH_M } else { meters(e) };
            map.tiles[i] = Terrain::Plains;
            map.levels[i] = level_of_meters(map.elevation[i]);
        }
        // Fond marin : les cases de mer, rangées de la plus haute à la plus basse, prennent la
        // profondeur de leur rang (plateau continental là où le socle est haut : continents
        // submergés ; fosses au pied des subductions)
        {
            let mut sea_cells: Vec<(usize, f64)> = elevations.iter().filter(|&&(h, e)| is_sea(h, e)).map(|&(h, e)| (map.index(h).unwrap(), e)).collect();
            // Distance à la terre (cases) : le plateau longe les côtes, sauf là où le socle plonge
            // (subductions)
            let n = map.tiles.len();
            let mut dist = vec![u16::MAX; n];
            let mut queue = std::collections::VecDeque::new();
            for &(h, e) in &elevations {
                if !is_sea(h, e) {
                    let i = map.index(h).unwrap();
                    dist[i] = 0;
                    queue.push_back(i);
                }
            }
            while let Some(i) = queue.pop_front() {
                for nb in map.neighbors(map.hex_at(i)) {
                    if let Some(j) = map.index(nb)
                        && dist[j] == u16::MAX
                    {
                        dist[j] = dist[i] + 1;
                        queue.push_back(j);
                    }
                }
            }
            // Rang du socle parmi les mers (0 le plus bas, 1 le plus haut), moins un écart par case
            // d'éloignement de la côte
            sea_cells.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            let last = sea_cells.len().saturating_sub(1).max(1) as f64;
            // (plus un bruit doux : un plateau plus ou moins large selon les côtes)
            let shelf_noise: Fbm<Perlin> = Fbm::new(seed.wrapping_add(31_337)).set_octaves(3).set_frequency(1.0);
            for (rank, cell) in sea_cells.iter_mut().enumerate() {
                let away = f64::from(dist[cell.0].min(100).saturating_sub(1));
                let wobble = sample(&shelf_noise, map.hex_at(cell.0), 3.0);
                cell.1 = rank as f64 / last - SHELF_DISTANCE_WEIGHT * away * (1.0 + 0.6 * wobble);
            }
            sea_cells.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            let last = sea_cells.len().saturating_sub(1).max(1) as f32;
            for (rank, &(i, _)) in sea_cells.iter().enumerate() {
                let share = rank as f32 / last;
                let k = DEPTH_KNOTS.windows(2).find(|w| share <= w[1].0).unwrap_or(&DEPTH_KNOTS[2..]);
                let t = ((share - k[0].0) / (k[1].0 - k[0].0)).clamp(0.0, 1.0);
                map.depth[i] = k[0].1 + (k[1].1 - k[0].1) * t;
            }
            // Fosses : le long des subductions et des arcs d'îles, côté plaque océanique qui plonge
            for &(i, _) in &sea_cells {
                let boundary = map.plates.boundary.get(i).copied().flatten();
                let oceanic = map.plates.crust.get(i) == Some(&plates::PlateKind::Oceanic);
                if oceanic && dist[i] >= 2 && matches!(boundary, Some(plates::Boundary::Subduction | plates::Boundary::IslandArc)) {
                    let wobble = sample(&shelf_noise, map.hex_at(i), 6.0) as f32;
                    map.depth[i] = map.depth[i].max(TRENCH_DEPTH_M + 1_500.0 + 1_500.0 * wobble);
                }
            }
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
        let score: Vec<f32> = (0..land_cells.len()).map(|k| relief_score(local_rank[k], rough_rank[k], map.elevation[land_cells[k]])).collect();
        let mut by_score: Vec<usize> = (0..land_cells.len()).filter(|&k| !ponds.contains(&hex_of(land_cells[k]))).collect();
        by_score.sort_by(|&a, &b| score[b].total_cmp(&score[a]));
        let count = |share: f64| (land_cells.len() as f64 * share).round() as usize;
        let (mountains, rugged, hills) = (count(MOUNTAIN_SHARE), count(RUGGED_SHARE), count(HILL_SHARE));
        // Règle gardée pour la console de debug : note de la dernière case de chaque relief
        let last_score = |n: usize| by_score.get(n.saturating_sub(1)).map_or(f32::MAX, |&k| score[k]);
        let sorted = |v: &[f32]| {
            let mut v = v.to_vec();
            v.sort_by(f32::total_cmp);
            v
        };
        let mut rule = ReliefRule {
            local_sorted: sorted(&local),
            rough_sorted: sorted(&rough),
            rough: (0..map.tiles.len()).map(|i| sample(&rough_noise, hex_of(i), 2.5) as f32).collect(),
            generated: {
                let mut g = vec![None; map.tiles.len()];
                for (k, &i) in land_cells.iter().enumerate() {
                    g[i] = Some((local[k], local_rank[k], rough_rank[k]));
                }
                g
            },
            thresholds: [last_score(mountains), last_score(mountains + rugged), last_score(mountains + rugged + hills)],
            peak_floor_m: f32::MAX,
        };
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
            rule.peak_floor_m = rule.peak_floor_m.min(map.elevation[i]);
            peaks += 1;
        }
        map.relief_rule = Some(rule);
        map.region = region;
        map.seed = seed;
        map.climate_base = Some(ClimateBase {
            levels: map.levels.clone(),
            relief: map.relief.clone(),
            // Petite variation locale (sols, exposition) pour casser les contours des biomes
            jitter: (0..map.tiles.len()).map(|i| 1.0 + sample(&moisture_noise, map.hex_at(i), 3.0) as f32 * 0.25).collect(),
        });
        map.simulate_climate_and_water();
        map
    }

    /// Étapes 4 à 8 de la génération, depuis les altitudes, les terres et la mer actuelles :
    /// climat (avec les courants marins), biomes, eaux peu profondes, cours d'eau et lacs, marais,
    /// noms, eaux poissonneuses, en `CLIMATE_PASSES` passes (à partir de la deuxième, le climat
    /// voit les lacs de la passe précédente : ils évaporent et humidifient l'air, ce qui change la
    /// pluie, le ruissellement, donc les lacs eux-mêmes).
    fn simulate_climate_and_water(&mut self) {
        if self.climate_base.is_none() {
            return;
        }
        // Les vrais cours d'eau creusent le relief, une seule fois : une passe de climat donne leur
        // débit, puis tout le climat est refait sur le relief creusé (la pluie suit les vallées, et
        // « regenerer climat » redonne la même carte)
        if !self.rivers_incised && self.region.wraps {
            self.rivers_incised = true;
            self.climate_passes(1);
            self.sea_and_mouths();
            self.incise_rivers();
        }
        self.climate_passes(CLIMATE_PASSES);
        let regimes = self.sea_and_mouths();
        let (width, height, seed, region) = (self.width, self.height, self.seed, self.region);
        let n = self.tiles.len();
        // 13) Les deltas gagnent sur la mer : l'eau, les noms et la houle sont refaits ; les marées
        //     ne changent guère pour si peu de cases
        if self.delta_cells.is_empty() && self.grow_deltas() {
            self.regenerate_water();
            self.add_marine_life(seed);
            let sea: Vec<bool> = self.tiles.iter().map(|t| t.is_sea()).collect();
            self.ice = (0..n).map(|i| self.ice[i] && sea[i]).collect();
            self.waves = crate::waves::simulate(width, height, &sea, &self.ice, &self.depth, &regimes, &region);
            self.mouths = self.classify_mouths();
        }
    }

    /// Climat, biomes, eaux peu profondes, cours d'eau et lacs, marais, noms, eaux poissonneuses, en
    /// `passes` passes (voir `simulate_climate_and_water`).
    fn climate_passes(&mut self, passes: usize) {
        let Some(base) = self.climate_base.clone() else { return };
        let (width, height, seed, region) = (self.width, self.height, self.seed, self.region);
        let n = self.tiles.len();
        // Terres (lacs et marais compris) et mer
        let land: Vec<bool> = self.tiles.iter().map(|t| !t.is_sea()).collect();
        let surface: Vec<climate::Surface> =
            (0..n).map(|i| if land[i] { climate::Surface::Land(self.elevation[i].max(0.0)) } else { climate::Surface::Water }).collect();
        for pass in 0..passes {
            let mut surface = surface.clone();
            if pass > 0 {
                for (i, s) in surface.iter_mut().enumerate() {
                    if self.tiles[i] == Terrain::Lake {
                        *s = climate::Surface::Lake(self.lake_level_m[i]);
                    }
                }
            }
            // Terrain d'avant les lacs (classes d'altitude, relief) : la passe le recalcule
            for i in (0..n).filter(|&i| land[i]) {
                self.tiles[i] = Terrain::Plains;
                self.levels[i] = base.levels[i];
                self.relief[i] = base.relief[i];
            }
            self.step_cache = std::sync::OnceLock::new();
            self.murk_cache = std::sync::OnceLock::new();
            self.geologie_cache = std::sync::OnceLock::new();
            self.river_down_cache = std::sync::OnceLock::new();
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
            // Courants marins poussés par le vent (vents théoriques à la première passe, puis ceux
            // du climat de la passe précédente) : température de la mer donnée au climat
            let wind = if pass == 0 { currents::theoretical_wind(width, height, &region) } else { self.climate.wind.clone() };
            self.currents = currents::simulate(width, height, &surface, &wind, &region);
            self.climate = climate::simulate(width, height, &surface, seed, &region, &self.currents.anomaly);
            // 4) Climat : pression, vents, humidité et pluie (voir climate.rs), puis terrain selon
            //    la température et les précipitations, avec un peu de bruit pour casser les contours.
            for i in (0..n).filter(|&i| land[i]) {
                let t = self.climate_biome(i, base.jitter[i]);
                self.set(self.hex_at(i), t);
            }
            // 5) Les eaux du plateau continental (moins de 200 m) sont des eaux peu profondes
            let coasts: Vec<Hex> = self
                .hexes()
                .filter(|&h| self.get(h) == Some(Terrain::Ocean))
                .filter(|&h| self.index(h).is_some_and(|i| self.depth[i] < SHELF_DEPTH_M))
                .collect();
            for h in coasts {
                self.set(h, Terrain::Coast);
            }
            // 6) Cours d'eau et lacs (voir hydro.rs)
            self.add_hydrology();
            self.add_marshes();
            // 7) Langues des régions et noms des lieux (voir toponymy.rs)
            self.add_names(seed);
            // 8) Eaux poissonneuses (remontées d'eaux, monts sous-marins, îles volcaniques)
            self.add_marine_life(seed);
        }
    }

    /// Marées, banquise, houle et forme des embouchures, sur les mers et le climat actuels. Rend les
    /// régimes de vent du climat (qui ne sert plus ensuite).
    fn sea_and_mouths(&mut self) -> Vec<crate::climate::RegimeWind> {
        let (width, height, seed, region) = (self.width, self.height, self.seed, self.region);
        let n = self.tiles.len();
        // 9) Marées (voir tides.rs), sur les mers définitives
        let sea: Vec<bool> = self.tiles.iter().map(|t| t.is_sea()).collect();
        self.tides = crate::tides::simulate(width, height, &sea, &self.depth, &region);
        // 10) Banquise, selon la température de la mer, avec un bord découpé
        let edge = ice_edge_noise(width, height, seed, region.wraps);
        self.ice = (0..n).map(|i| sea[i] && self.currents.temperature[i] <= ICE_SEA_C + edge[i]).collect();
        self.step_cache = std::sync::OnceLock::new();
        // 11) Houle (voir waves.rs), sous le vent des régimes du climat, qui ne sert plus ensuite
        let regimes = std::mem::take(&mut self.climate.regime_wind);
        self.waves = crate::waves::simulate(width, height, &sea, &self.ice, &self.depth, &regimes, &region);
        // 12) Forme des embouchures de fleuve (voir mouths.rs)
        self.mouths = self.classify_mouths();
        regimes
    }

    /// Érosion par l'eau réelle, après le climat (voir `erosion::incise`) : les cases parcourues par
    /// un fort débit descendent vers leur case aval ; classe d'altitude et relief suivent. Le climat
    /// et l'eau sont à refaire ensuite.
    fn incise_rivers(&mut self) {
        let Some(mut base) = self.climate_base.take() else { return };
        let n = self.tiles.len();
        let surface: Vec<f32> = (0..n).map(|i| match self.tiles[i] {
            t if t.is_sea() => 0.0,
            Terrain::Lake => self.lake_level_m[i],
            _ => self.elevation[i].max(0.0),
        }).collect();
        let receiver: Vec<Option<usize>> = (0..n).map(|i| {
            let h = self.hex_at(i);
            self.downstream(h).and_then(|d| self.index(h + crate::hex::DIRECTIONS[d]))
        }).collect();
        let discharge: Vec<f32> = (0..n).map(|i| self.discharge(self.hex_at(i))).collect();
        // Ni la mer et les lacs, ni la sortie d'un lac : l'abaisser le viderait
        let mut fixed: Vec<bool> = self.tiles.iter().map(|t| t.is_water()).collect();
        for (i, r) in receiver.iter().enumerate() {
            if self.tiles[i] == Terrain::Lake && let Some(r) = *r {
                fixed[r] = true;
            }
        }
        // Embouchures : la houle et la marée renforcent le creusement sur les dernières cases, en
        // remontant le fleuve par son plus fort affluent
        let mut boost = vec![1.0f32; n];
        for &(mouth, m) in &self.mouths {
            let mut at = mouth;
            for reach in crate::erosion::MOUTH_REACH {
                boost[at] = boost[at].max(crate::erosion::mouth_boost(m.river, m.wave, m.tide, reach));
                let up = self.neighbors(self.hex_at(at)).into_iter().filter_map(|nb| self.index(nb)).filter(|&j| receiver[j] == Some(at)).max_by(|&a, &b| discharge[a].total_cmp(&discharge[b]));
                let Some(up) = up else { break };
                at = up;
            }
        }
        let lowered = crate::erosion::incise(&surface, &receiver, &discharge, &fixed, &boost, crate::erosion::IncisionParams::default());
        let changed: Vec<usize> = (0..n).filter(|&i| !fixed[i] && lowered[i] < surface[i]).collect();
        for &i in &changed {
            self.elevation[i] = lowered[i];
            self.levels[i] = level_of_meters(lowered[i]);
            base.levels[i] = self.levels[i];
        }
        // Relief des cases abaissées et de leurs voisines (dénivelée), comme à la génération : sans
        // les fleuves, que l'eau refaite aplanira de nouveau ; un pic reste un pic s'il en remplit
        // encore les conditions
        let mut touched: Vec<usize> = changed.iter().flat_map(|&i| {
            std::iter::once(i).chain(self.neighbors(self.hex_at(i)).into_iter().filter_map(|nb| self.index(nb)).collect::<Vec<_>>())
        }).collect();
        touched.sort_unstable();
        touched.dedup();
        let fleuves = std::mem::replace(&mut self.hydro.fleuve, vec![false; n]);
        for i in touched {
            let h = self.hex_at(i);
            if let Some(r) = self.natural_relief(h)
                && !(base.relief[i] == Relief::Peak && r == Relief::Mountain && self.could_be_peak_ignoring_relief(h))
            {
                base.relief[i] = r;
            }
        }
        self.hydro.fleuve = fleuves;
        self.step_cache = std::sync::OnceLock::new();
        self.climate_base = Some(base);
    }

    /// Fait avancer les deltas : les cases de mer peu profonde devant leur embouchure deviennent
    /// de la terre basse. `true` si au moins une a changé.
    fn grow_deltas(&mut self) -> bool {
        use crate::mouths::MouthKind;
        let mut grown: Vec<(usize, Hex, f32, MouthKind)> = Vec::new();
        let mut routes: Vec<(usize, usize)> = Vec::new();
        for &(i, m) in &self.mouths {
            if !matches!(m.kind, MouthKind::BirdFoot | MouthKind::Arcuate | MouthKind::TidalDelta) {
                continue;
            }
            let h = self.hex_at(i);
            let Some(d) = self.downstream(h) else { continue };
            let cells = if m.kind == MouthKind::BirdFoot && m.river >= DELTA_TWO_CELLS_KG_S { 2 } else { 1 };
            let (mut from, mut p) = (h, h + crate::hex::DIRECTIONS[d]);
            for k in 0..cells {
                let Some(c) = self.index(p) else { break };
                let taken = |g: &[(usize, Hex, f32, MouthKind)], j: usize| g.iter().any(|x| x.0 == j);
                let around = self.neighbors(p).iter().filter(|&&nb| self.canonical(nb) != self.canonical(from)).filter(|&&nb| self.index(nb).is_some_and(|j| !self.tiles[j].is_sea() || taken(&grown, j))).count();
                if !self.tiles[c].is_sea() || self.ice[c] || self.depth[c] >= SHELF_DEPTH_M || taken(&grown, c) || around > DELTA_MAX_LAND_AROUND {
                    break;
                }
                // (plus bas que la case d'où il vient : l'eau descend)
                let ground = self.elevation[self.cell(from)].max(1.0) * 0.5;
                let ground = grown.iter().find(|g| g.1 == from).map_or(ground, |g| g.2 * 0.5);
                grown.push((c, p, DELTA_LAND_M[k.min(1)].min(ground), m.kind));
                routes.push((self.cell(from), c));
                from = p;
                p = p + crate::hex::DIRECTIONS[d];
            }
            // Delta arqué : la houle étale ses sables de part et d'autre, un lobe bombé sur la côte
            if m.kind == MouthKind::Arcuate && grown.iter().any(|g| g.1 == h + crate::hex::DIRECTIONS[d]) {
                for side in [(d + 1) % 6, (d + 5) % 6] {
                    let p = h + crate::hex::DIRECTIONS[side];
                    let Some(c) = self.index(p) else { continue };
                    let already = grown.iter().any(|g| g.0 == c);
                    if self.tiles[c].is_sea() && !self.ice[c] && self.depth[c] < SHELF_DEPTH_M && !already {
                        grown.push((c, p, DELTA_LAND_M[0].min(self.elevation[i].max(1.0) * 0.5), m.kind));
                    }
                }
            }
        }
        for &(c, p, meters, kind) in &grown {
            // (le biome de son propre climat : `verifier` et `regenerer climat` le retrouvent)
            let terrain = self.climate_biome(c, self.climate_base.as_ref().map_or(1.0, |b| b.jitter[c]));
            self.set(p, terrain);
            self.elevation[c] = meters;
            self.delta_cells.push((c, kind));
        }
        self.delta_routes = routes;
        !grown.is_empty()
    }

    /// Cases gagnées sur la mer par les deltas.
    #[cfg(test)]
    pub fn delta_cells(&self) -> &[(usize, crate::mouths::MouthKind)] {
        &self.delta_cells
    }

    /// Biome d'une case de terre selon son climat, l'aridité variée de `jitter` (sols, exposition).
    fn climate_biome(&self, i: usize, jitter: f32) -> Terrain {
        let temperature = self.climate.temperature[i];
        let aridity = self.climate.aridity(i).unwrap_or(1.0) * jitter;
        let (coldest, warmest) = self.climate.coldest_warmest(i).unwrap_or((temperature, temperature));
        let dry = self.climate.dry_seasons(i).unwrap_or([false; 4]);
        biome(BiomeClimate {
            temperature,
            aridity,
            coldest,
            warmest,
            dry_seasons: dry.iter().filter(|&&d| d).count() as u8,
            dry_summer: self.climate.dry_summer(i).unwrap_or(false),
        })
    }

    /// Variation locale de l'aridité de la case à la génération (1 : aucune ; `None` : carte
    /// remplie à la main).
    pub fn aridity_jitter(&self, hex: Hex) -> Option<f32> {
        let i = self.index(hex)?;
        self.climate_base.as_ref().map(|b| b.jitter[i])
    }

    /// Refait le climat, les biomes, l'eau, les noms et les eaux poissonneuses comme à la
    /// génération, depuis les altitudes, les terres et la mer actuelles (console de debug).
    /// `false` : carte sans génération (remplie à la main).
    pub fn regenerate_climate(&mut self) -> bool {
        if self.climate_base.is_none() {
            return false;
        }
        self.simulate_climate_and_water();
        true
    }

    /// Refait les cours d'eau, les lacs, les marais et les noms avec le climat actuel (console de
    /// debug) : lacs et marais redeviennent la terre que donne leur climat, les reliefs aplanis
    /// par un fleuve reviennent, puis l'eau coule de nouveau. `false` : carte sans génération.
    pub fn regenerate_water(&mut self) -> bool {
        let Some(base) = self.climate_base.clone() else { return false };
        if self.climate.runoff.len() != self.tiles.len() {
            return false;
        }
        for i in 0..self.tiles.len() {
            let t = self.tiles[i];
            if t.is_sea() {
                continue;
            }
            if matches!(t, Terrain::Lake | Terrain::Marsh) {
                self.tiles[i] = self.climate_biome(i, base.jitter[i]);
            }
            self.levels[i] = base.levels[i];
            self.relief[i] = base.relief[i];
        }
        self.add_hydrology();
        self.add_marshes();
        self.add_names(self.seed);
        self.step_cache = std::sync::OnceLock::new();
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
        self.mouths = self.classify_mouths();
        true
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
            routes: &self.delta_routes,
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
        self.murk_cache = std::sync::OnceLock::new();
        self.geologie_cache = std::sync::OnceLock::new();
        self.river_down_cache = std::sync::OnceLock::new();
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
            .map(|y| y.food * 3 + y.production * 2 + y.deniers)
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
    fn turbidite_selon_les_terres_traversees_et_eclaircie_par_les_lacs() {
        let desert = sediment_yield(Terrain::Desert, 200.0);
        let taiga = sediment_yield(Terrain::Taiga, 200.0);
        // Affluents 0 (désert, 10 m³/s) et 1 (taïga, 90 m³/s) mêlés en 2 (taïga)
        let down = [Some(2), Some(2), Some(3), Some(4), Some(5), None];
        let discharge = [10.0, 90.0, 100.0, 100.0, 100.0, 100.0];
        let yields = [desert, taiga, taiga, desert, desert, desert];
        let m = murk_of_cells(&down, &discharge, &yields, &[false; 6]);
        let mixed = (10.0 * desert + 90.0 * taiga) / 100.0;
        assert!((m[2] - (mixed + (taiga - mixed) * MURK_LOCAL)).abs() < 1e-5, "{} {mixed}", m[2]);
        // Dans le désert, l'eau se charge peu à peu
        assert!(m[2] < m[3] && m[3] < m[4] && m[4] < m[5] && m[5] < desert);
        // Lac de deux cases (3, 4) : éclaircie une seule fois, à sa sortie
        let lake = [false, false, false, true, true, false];
        let l = murk_of_cells(&down, &discharge, &yields, &lake);
        assert!((l[3] - m[3]).abs() < 1e-5 && l[4] < m[4] * 0.5);
        // Les hautes terres rocheuses donnent une eau plus claire
        assert!(sediment_yield(Terrain::Steppe, 2_500.0) < sediment_yield(Terrain::Steppe, 200.0));
    }

    #[test]
    fn biomes_selon_les_saisons() {
        let c = |temperature: f32, aridity: f32, coldest: f32, dry_seasons: u8, dry_summer: bool| {
            // (été : 8 °C au-dessus de l'hiver, au moins la moyenne annuelle)
            let warmest = (coldest + 8.0).max(temperature);
            biome(BiomeClimate { temperature, aridity, coldest, warmest, dry_seasons, dry_summer })
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
        let alpine = BiomeClimate { temperature: 5.0, aridity: 1.2, coldest: 1.0, warmest: 8.0, dry_seasons: 0, dry_summer: false };
        assert_eq!(biome(alpine), Terrain::Grassland, "au-dessus de la limite des arbres : prairie d'altitude");
        assert_eq!(biome(BiomeClimate { warmest: 11.0, ..alpine }), Terrain::Forest, "sous la limite des arbres");
        assert_eq!(biome(BiomeClimate { coldest: -8.0, warmest: 8.0, ..alpine }), Terrain::Taiga, "la taïga n'est pas concernée");
    }

    #[test]
    fn pas_de_mare_de_mer() {
        // Aucune étendue de mer de moins de `MIN_INLAND_SEA_CELLS` cases enfermée dans les terres
        for seed in [2, 777] {
            let m = WorldMap::generate_in(128, 74, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 74));
            let mut seen = bevy::platform::collections::HashSet::new();
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
        // Des terres peuvent toucher le raccord (plus d'océan forcé à l'est et à l'ouest)
        let land_at_seam = |m: &WorldMap| (0..74).any(|r| !m.get(Hex::from_offset(0, r)).unwrap().is_water() || !m.get(Hex::from_offset(127, r)).unwrap().is_water());
        assert!(land_at_seam(&m) || [6, 7].iter().any(|&s| land_at_seam(&WorldMap::generate_in(128, 74, s, crate::climate::Region::planet(crate::climate::CELL_KM, 74)))));
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
                    if map.level(n).is_some_and(|m| m > 0 && l > 0) {
                        edges += 1;
                        cliffs += usize::from(map.cliff_between(h, n));
                    }
                }
            }
            assert!(used.iter().all(|&u| u), "graine {seed} : tous les niveaux sont présents");
            // (cartes régionales de bruit, plus heurtées que les planètes, où les falaises font ~1 %)
            assert!(cliffs * 100 < edges * 8, "graine {seed} : falaises rares ({cliffs}/{edges})");
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
        assert_eq!(climb_cost(450.0, 1_100.0), 2, "montée raide");
        assert_eq!(climb_cost(450.0, 1_500.0), 4, "très raide");
        assert_eq!(climb_cost(1_500.0, 450.0), 0, "descente gratuite");
        assert!(is_cliff(150.0, 1_300.0), "falaise");
        assert!(is_cliff(1_300.0, 150.0), "falaise dans les deux sens");
        assert!(!is_cliff(450.0, 1_300.0));
        assert!(is_cliff(0.0, 1_000.0), "côte escarpée");
        assert!(!is_cliff(1_600.0, 3_200.0), "entre deux montagnes : pas de falaise");
        let mut map = WorldMap::filled(5, 5, Terrain::Plains);
        let (a, b) = (Hex::from_offset(1, 1), Hex::from_offset(2, 1));
        map.set_elevation(b, 850.0);
        assert_eq!(map.step_cost(a, b), Some(3), "1 PM + 2 de montée");
        assert_eq!(map.step_cost(b, a), Some(1));
        map.set_elevation(b, 1_300.0);
        assert_eq!(map.step_cost(a, b), None);
        // 150 → 500 m gratuit, 150 → 1 000 m +2 (dénivelée sous la falaise)
        map.set_elevation(b, 500.0);
        assert_eq!(map.step_cost(a, b), Some(1));
        map.set_elevation(b, 1_000.0);
        assert_eq!(map.step_cost(a, b), Some(3));
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
        assert_eq!(m.move_cost(h), Some(RELIEF_MOVE_CAP), "forêt (4) + collines (2)");
        assert_eq!(m.describe(h), "Forêt tempérée vallonnée");
        m.set_relief(h, Relief::Mountain);
        assert_eq!(m.move_cost(h), Some(MOUNTAIN_MOVE_MIN), "montagne franchissable");
        assert_eq!(m.yields(h), Some(MOUNTAIN_YIELDS));
        assert_eq!(m.describe(h), "Montagne (forêt tempérée)");
        m.set_relief(h, Relief::Rugged);
        assert_eq!(m.move_cost(h), Some(RELIEF_MOVE_CAP), "forêt accidentée : plafond");
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
    fn ponts_sur_riviere_et_fleuve() {
        use super::*;
        let mut m = WorldMap::filled(10, 10, Terrain::Grassland);
        let a = Hex::from_offset(4, 4);
        let b = a + crate::hex::DIRECTIONS[0];
        assert_eq!(m.bridge_site(a, 0), None, "pas de pont sans cours d'eau");
        m.set_river(a, 0, 150.0);
        // Une seule écriture, quel que soit le côté
        let at = m.bridge_site(a, 0).unwrap();
        assert_eq!(m.bridge_site(b, 3), Some(at));
        assert!(m.set_bridge(at, true) && !m.set_bridge(at, true));
        assert!(m.bridge_between(a, b) && m.bridge_between(b, a));
        assert_eq!(m.step_cost(a, b), Some(1), "rivière franchie par le pont sans surcoût");
        assert_eq!(m.step_cost(b, a), Some(1));
        // Fleuve : pont d'une rive à l'opposée ; on y monte par un bout pour 1 PM, sans arrêt
        let f = Hex::from_offset(7, 6);
        m.set_fleuve(f);
        let (e0, e3) = (f + crate::hex::DIRECTIONS[1], f + crate::hex::DIRECTIONS[4]);
        let side = f + crate::hex::DIRECTIONS[0];
        let at = m.bridge_site(f, 4).unwrap();
        assert_eq!(at, BridgeAt::Fleuve { hex: f, axis: 1 });
        m.set_bridge(m.bridge_site(f, 0).unwrap(), true);
        m.set_bridge(at, true);
        assert_eq!(m.bridges().count(), 2, "un pont sur la rivière, un seul sur le fleuve");
        assert_eq!(m.fleuve_bridge(f), Some(1));
        assert!(m.bridge_end(f, e0) && m.bridge_end(f, e3) && !m.bridge_end(f, side));
        let onto = m.step(e0, f).unwrap();
        assert_eq!((onto.total(), onto.water, onto.onto_bridge), (BRIDGE_MOVE_COST, 0, true));
        assert_eq!(onto.spend(2), (1, 0), "l'unité peut continuer après le pont");
        assert_eq!(m.step(side, f).unwrap(), StepCost { water: FLEUVE_MOVE_COST, ..StepCost::default() }, "par le côté : dans l'eau");
        assert_eq!(m.step_cost(f, e3), Some(1), "descendre du pont : le terrain d'arrivée");
        // Un fleuve bordé d'eau n'a pas de pont
        m.set(side, Terrain::Ocean);
        assert_eq!(m.bridge_site(f, 0), None);
        m.set_bridge(at, false);
        assert_eq!(m.fleuve_bridge(f), None);
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

    /// Bilan des forêts froides (essai) : forêts tempérées et taïga dont la saison la plus chaude reste sous 10 °C.
    #[test]
    #[ignore]
    fn bilan_forets_froides() {
        use super::*;
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![777, 2, 4242]);
        for seed in seeds {
            let m = WorldMap::generate_in(192, 111, seed, Region::planet(crate::climate::CELL_KM, 111));
            let c = m.climate();
            for kind in [Terrain::Forest, Terrain::Taiga, Terrain::Grassland, Terrain::Steppe, Terrain::Tundra] {
                let cells: Vec<(usize, Hex)> = m.hexes().enumerate().filter(|&(_, h)| m.get(h) == Some(kind)).collect();
                let warm = |i: usize| c.coldest_warmest(i).map_or(99.0, |w| w.1);
                let mut hist = [0usize; 8];
                let (mut high, mut lat_sum, mut cold_sum, mut t_sum, mut k) = (0usize, 0.0f32, 0.0f32, 0.0f32, 0usize);
                for &(i, h) in &cells {
                    let w = warm(i);
                    let b = ((w - 4.0) / 2.0).floor().clamp(0.0, 7.0) as usize;
                    hist[b] += 1;
                    if w < 10.0 {
                        k += 1;
                        high += usize::from(m.elevation(h).unwrap_or(0.0) >= 1_500.0);
                        lat_sum += m.latitude(h).abs();
                        cold_sum += c.coldest_warmest(i).unwrap().0;
                        t_sum += c.temperature[i];
                    }
                }
                let kf = k.max(1) as f32;
                println!("graine {seed} {:<14} {:>5} cases ; été <10 °C : {:>4} (dont ≥ 1 500 m {:>4}) lat. moy. {:>4.0}° hiver {:>5.1} °C année {:>5.1} °C | été par 2 °C depuis <6 : {:?}",
                    kind.name(), cells.len(), k, high, lat_sum / kf, cold_sum / kf, t_sum / kf, hist);
            }
        }
    }

#[cfg(test)]
mod bilan {
    /// Bilan de l'aridité par bande de latitude (à lancer à la main) : pluie, ETP, aridité et
    /// biomes des terres, sur les graines 777 / 2 / 4242 (`SEEDS=a,b` pour d'autres) ; côtes est et
    /// ouest (désert, pluie, vents et humidité de l'été et de l'hiver boréaux, mer au large) ; bilan
    /// d'eau de la planète (évaporation et pluie de la mer et des terres, eau de l'air, séjour).
    #[test]
    #[ignore]
    fn bilan_aridite() {
        use super::*;
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![777, 2, 4242]);
        for seed in seeds {
            let m = WorldMap::generate_in(192, 111, seed, Region::planet(crate::climate::CELL_KM, 111));
            let c = m.climate();
            let land: Vec<(usize, Hex)> = m.hexes().enumerate().filter(|&(_, h)| { let t = m.get(h).unwrap(); !t.is_water() }).collect();
            let n = land.len() as f32;
            let count = |t: Terrain| land.iter().filter(|&&(_, h)| m.get(h) == Some(t)).count();
            println!("\n=== graine {seed} : {} cases de terre ; pluie {:.0} mm, ETP {:.0} mm, P/ETP médian {:.2}", land.len(),
                land.iter().map(|&(i, _)| c.rain[i]).sum::<f32>() / n, land.iter().map(|&(i, _)| c.pet[i]).sum::<f32>() / n,
                { let mut a: Vec<f32> = land.iter().map(|&(i, _)| c.aridity(i).unwrap()).collect(); a.sort_by(f32::total_cmp); a[a.len() / 2] });
            println!("désert {:.1} %, jungle {:.1} %, savane arborée {:.1} %, savane {:.1} %, forêt {:.1} %, plaine {:.1} %",
                100.0 * count(Terrain::Desert) as f32 / n, 100.0 * count(Terrain::Jungle) as f32 / n, 100.0 * count(Terrain::WoodedSavanna) as f32 / n,
                100.0 * count(Terrain::Savanna) as f32 / n, 100.0 * count(Terrain::Forest) as f32 / n, 100.0 * count(Terrain::Plains) as f32 / n);
            // Bilan d'eau de la planète : mer et terres, eau de l'air et séjour de l'eau dans l'air
            {
                let all: Vec<usize> = (0..m.tiles.len()).collect();
                let sea: Vec<usize> = all.iter().copied().filter(|&i| m.tiles[i].is_sea()).collect();
                let lnd: Vec<usize> = land.iter().map(|&(i, _)| i).collect();
                // Moyennes par case : les cases du climat ont toutes la même surface (pas de pondération
                // par la latitude, qui ferait croire à une fuite d'eau)
                let avg = |f: &Vec<f32>, set: &Vec<usize>| set.iter().map(|&i| f[i]).sum::<f32>() / set.len().max(1) as f32;
                let evap_all = avg(&c.evaporation, &all);
                println!("bilan : mer évap {:.0} pluie {:.0} mm, terres pluie {:.0} évap {:.0} mm, planète évap {:.0} pluie {:.0} mm, eau de l'air {:.1} mm, séjour {:.1} j",
                    avg(&c.evaporation, &sea), avg(&c.rain, &sea), avg(&c.rain, &lnd), avg(&c.evaporation, &lnd), evap_all, avg(&c.rain, &all), avg(&c.column, &all), avg(&c.column, &all) / (evap_all / 365.0));
                let bands: Vec<String> = (-9..9).map(|b| { let set: Vec<usize> = all.iter().copied().filter(|&i| { let l = m.latitude(m.hex_at(i)); l >= b as f32 * 10.0 && l < b as f32 * 10.0 + 10.0 }).collect(); format!("{}:{:.0}/{:.0}", b * 10, avg(&c.rain, &set), avg(&c.evaporation, &set)) }).collect();
                println!("pluie/évap par bande (toutes cases) : {}", bands.join(" "));
            }
            println!(" lat  terre   pluie   ETP  P/ETP  T  froide  sèches | désert jungle s.arb savane plaine forêt  | tropic. <0,75 sais.");
            for band in 0..9 {
                let (lo, hi) = (band as f32 * 10.0, band as f32 * 10.0 + 10.0);
                let set: Vec<(usize, Hex)> = land.iter().copied().filter(|&(_, h)| (lo..hi).contains(&m.latitude(h).abs())).collect();
                if set.is_empty() { continue; }
                let k = set.len() as f32;
                let avg = |f: &dyn Fn(usize) -> f32| set.iter().map(|&(i, _)| f(i)).sum::<f32>() / k;
                let cnt = |t: Terrain| set.iter().filter(|&&(_, h)| m.get(h) == Some(t)).count();
                let tropical: Vec<usize> = set.iter().map(|&(i, _)| i).filter(|&i| c.coldest_warmest(i).unwrap().0 >= TROPICAL_WINTER).collect();
                let dry_ar = tropical.iter().filter(|&&i| c.aridity(i).unwrap() < FOREST_ABOVE).count();
                let dry_s = tropical.iter().filter(|&&i| c.aridity(i).unwrap() >= FOREST_ABOVE && c.dry_seasons(i).unwrap().iter().filter(|&&d| d).count() > 1).count();
                println!("{:>2}-{:<2} {:>5} {:>7.0} {:>5.0} {:>5.2} {:>5.1} {:>5.1} {:>5.2}  | {:>5} {:>5} {:>5} {:>5} {:>5} {:>5}  | {:>5} {:>5} {:>5}",
                    lo, hi, set.len(), avg(&|i| c.rain[i]), avg(&|i| c.pet[i]), avg(&|i| c.aridity(i).unwrap()), avg(&|i| c.temperature[i]),
                    avg(&|i| c.coldest_warmest(i).unwrap().0), avg(&|i| c.dry_seasons(i).unwrap().iter().filter(|&&d| d).count() as f32),
                    cnt(Terrain::Desert), cnt(Terrain::Jungle), cnt(Terrain::WoodedSavanna), cnt(Terrain::Savanna), cnt(Terrain::Plains), cnt(Terrain::Forest),
                    tropical.len(), dry_ar, dry_s);
            }
            // Terres équatoriales (0-10°) selon l'altitude : part tropicale (saison froide ≥ 18 °C) et jungle
            let eq: Vec<usize> = land.iter().filter(|&&(_, h)| m.latitude(h).abs() < 10.0).map(|&(i, _)| i).collect();
            let alt: Vec<String> = [(0.0, 300.0), (300.0, 800.0), (800.0, 1_500.0), (1_500.0, 1e9)].iter().map(|&(a, b)| {
                let s: Vec<usize> = eq.iter().copied().filter(|&i| (a..b).contains(&m.elevation[i].max(0.0))).collect();
                let tr = s.iter().filter(|&&i| c.coldest_warmest(i).unwrap().0 >= TROPICAL_WINTER).count();
                let j = s.iter().filter(|&&i| m.tiles[i] == Terrain::Jungle).count();
                let f = s.iter().filter(|&&i| m.tiles[i] == Terrain::Forest).count();
                format!("{a:.0}-{b:.0} m : {} cases, {tr} tropicales, {j} jungle, {f} forêt tempérée", s.len())
            }).collect();
            println!("0-10° : {}", alt.join(" | "));
            for band in [-3i32, -2, -1, 0, 1, 2] {
                let (lo, hi) = (band as f32 * 10.0, band as f32 * 10.0 + 10.0);
                let (mut l, mut sea) = (Vec::new(), Vec::new());
                for (i, h) in m.hexes().enumerate() {
                    if !(lo..hi).contains(&m.latitude(h)) { continue; }
                    if m.get(h).unwrap().is_sea() { sea.push(i) } else if !m.get(h).unwrap().is_water() { l.push(i) }
                }
                let avg = |f: &Vec<f32>, set: &Vec<usize>, k: f32| k * set.iter().map(|&i| f[i]).sum::<f32>() / set.len().max(1) as f32;
                let wind = |set: &Vec<usize>| { let (x, y) = set.iter().fold((0.0, 0.0), |a, &i| (a.0 + c.wind[i].0, a.1 + c.wind[i].1)); (x / set.len().max(1) as f32, y / set.len().max(1) as f32) };
                println!("{lo:>3}..{hi:<3} mer : HR {:>3.0} % col {:>4.1} évap {:>5.0} pluie {:>5.0} vent ({:>5.2}, {:>5.2}) | terre : HR {:>3.0} % col {:>4.1} évap {:>5.0} pluie {:>5.0} ruiss. {:>4.0} vent ({:>5.2}, {:>5.2})",
                    avg(&c.humidity, &sea, 100.0), avg(&c.column, &sea, 1.0), avg(&c.evaporation, &sea, 1.0), avg(&c.rain, &sea, 1.0), wind(&sea).0, wind(&sea).1,
                    avg(&c.humidity, &l, 100.0), avg(&c.column, &l, 1.0), avg(&c.evaporation, &l, 1.0), avg(&c.rain, &l, 1.0), avg(&c.runoff, &l, 1.0), wind(&l).0, wind(&l).1);
            }
            let share = |a: f32| 100.0 * land.iter().filter(|&&(i, _)| m.elevation[i].max(0.0) < a).count() as f32 / n;
            println!("terres sous 300 m : {:.0} %, sous 500 m : {:.0} %, sous 1 000 m : {:.0} %, sous 2 000 m : {:.0} %", share(300.0), share(500.0), share(1_000.0), share(2_000.0));
            // Distance à la mer (cases), par parcours en largeur depuis les côtes
            let mut dist = vec![u32::MAX; m.tiles.len()];
            let mut queue = std::collections::VecDeque::new();
            for (i, h) in m.hexes().enumerate() {
                if m.get(h).unwrap().is_sea() { dist[i] = 0; queue.push_back(h); }
            }
            while let Some(h) = queue.pop_front() {
                let d = dist[m.index(h).unwrap()];
                for nb in m.neighbors(h) {
                    let Some(j) = m.index(nb) else { continue };
                    if dist[j] == u32::MAX { dist[j] = d + 1; queue.push_back(nb); }
                }
            }
            let med = |mut v: Vec<f32>| { if v.is_empty() { return f32::NAN; } v.sort_by(f32::total_cmp); v[v.len() / 2] };
            println!(" lat   alt.méd  pluie méd  ETP méd  P/ETP méd | pluie méd par saison (éq., été N, éq., hiver N) | désert selon la distance à la mer 1-2 / 3-5 / 6-10 / >10");
            for band in [-3i32, -2, -1, 0, 1, 2] {
                let (lo, hi) = (band as f32 * 10.0, band as f32 * 10.0 + 10.0);
                let set: Vec<usize> = land.iter().filter(|&&(_, h)| (lo..hi).contains(&m.latitude(h))).map(|&(i, _)| i).collect();
                if set.is_empty() { continue; }
                let season: Vec<String> = (0..4).map(|s| format!("{:>5.0}", med(set.iter().map(|&i| c.season_rain[i][s]).collect()))).collect();
                let by_dist: Vec<String> = [(1, 2), (3, 5), (6, 10), (11, u32::MAX)].iter().map(|&(a, b)| {
                    let s: Vec<usize> = set.iter().copied().filter(|&i| (a..=b).contains(&dist[i])).collect();
                    let d = s.iter().filter(|&&i| m.tiles[i] == Terrain::Desert).count();
                    format!("{d}/{}", s.len())
                }).collect();
                println!("{:>3}..{:<3} {:>6.0} {:>9.0} {:>8.0} {:>9.2}  | {} | {}", lo, hi,
                    med(set.iter().map(|&i| m.elevation[i].max(0.0)).collect()), med(set.iter().map(|&i| c.rain[i]).collect()),
                    med(set.iter().map(|&i| c.pet[i]).collect()), med(set.iter().map(|&i| c.aridity(i).unwrap()).collect()), season.join(""), by_dist.join("  "));
            }
            // Côtes est et ouest (terres à 3 cases au plus de la mer) : la mer la plus proche sur la
            // ligne, à l'est ou à l'ouest (6 cases au plus)
            // Climat d'une saison seule (été boréal, hiver boréal) : vents et pluie
            let surface: Vec<crate::climate::Surface> = (0..m.tiles.len()).map(|i| if m.tiles[i].is_sea() { crate::climate::Surface::Water } else if m.tiles[i] == Terrain::Lake { crate::climate::Surface::Lake(m.lake_level_m[i]) } else { crate::climate::Surface::Land(m.elevation[i].max(0.0)) }).collect();
            let seasons: Vec<crate::climate::Climate> = [0.26f32, -0.26].iter().map(|&sun| crate::climate::simulate_periods(m.width, m.height, &surface, m.seed, &m.region, &m.currents.anomaly, &[crate::climate::Period { sun_lat: sun }])).collect();
            println!(" lat   côte est : désert / cases, pluie méd, P/ETP méd | côte ouest : idem");
            let sea_at = |h: Hex, step: i32| (1..=6).find(|&k| { let (col, row) = h.to_offset(); m.get(m.canonical(Hex::from_offset(col + step * k, row))).is_some_and(|t| t.is_sea()) });
            for band in [-4i32, -3, -2, -1, 0, 1, 2, 3] {
                let (lo, hi) = (band as f32 * 10.0, band as f32 * 10.0 + 10.0);
                let (mut east, mut west) = (Vec::new(), Vec::new());
                for &(i, h) in &land {
                    if !(lo..hi).contains(&m.latitude(h)) || !(1..=3).contains(&dist[i]) { continue; }
                    match (sea_at(h, 1), sea_at(h, -1)) {
                        (Some(e), Some(w)) if e < w => east.push(i),
                        (Some(_), None) => east.push(i),
                        (Some(e), Some(w)) if w < e => west.push(i),
                        (None, Some(_)) => west.push(i),
                        _ => {}
                    }
                }
                let show = |v: &Vec<usize>| {
                    let k = v.len().max(1) as f32;
                    let mean = |f: &dyn Fn(usize) -> f32| v.iter().map(|&i| f(i)).sum::<f32>() / k;
                    format!("{:>4}/{:<4} {:>6.0} {:>5.2} HR {:>2.0} vent ({:>5.2},{:>5.2}) saisons {:>4.0} {:>4.0} {:>4.0} {:>4.0}", v.iter().filter(|&&i| m.tiles[i] == Terrain::Desert).count(), v.len(),
                        med(v.iter().map(|&i| c.rain[i]).collect()), med(v.iter().map(|&i| c.aridity(i).unwrap()).collect()),
                        mean(&|i| 100.0 * c.humidity[i]), mean(&|i| c.wind[i].0), mean(&|i| c.wind[i].1),
                        mean(&|i| c.season_rain[i][0]), mean(&|i| c.season_rain[i][1]), mean(&|i| c.season_rain[i][2]), mean(&|i| c.season_rain[i][3]))
                };
                println!("{lo:>3}..{hi:<3} {} | {}", show(&east), show(&west));
                let sw = |v: &Vec<usize>| seasons.iter().map(|cl| { let k = v.len().max(1) as f32; format!("vent ({:>5.2},{:>5.2}) pluie {:>5.0} HR {:>2.0}", v.iter().map(|&i| cl.wind[i].0).sum::<f32>() / k, v.iter().map(|&i| cl.wind[i].1).sum::<f32>() / k, v.iter().map(|&i| cl.rain[i]).sum::<f32>() / k, v.iter().map(|&i| 100.0 * cl.humidity[i]).sum::<f32>() / k) }).collect::<Vec<_>>().join(" / ");
                println!("          est été/hiver N : {}   ouest : {}", sw(&east), sw(&west));
                let sst = |v: &Vec<usize>, step: i32| { let k = v.len().max(1) as f32; v.iter().map(|&i| { let h = m.hex_at(i); let (col, row) = h.to_offset(); (1..=6).filter_map(|d| m.index(m.canonical(Hex::from_offset(col + step * d, row)))).find(|&j| m.tiles[j].is_sea()).map_or(0.0, |j| m.currents.anomaly[j]) }).sum::<f32>() / k };
                println!("          mer au large : est {:+.1} °C, ouest {:+.1} °C", sst(&east, 1), sst(&west, -1));
            }
        }
    }

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
        let mut seen = bevy::platform::collections::HashSet::new();
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


#[cfg(test)]
mod tectonique {
    use super::*;

    /// Bilan de la géologie et des sols (à lancer à la main, `SEEDS=777,2`) : part de chaque roche
    /// et de chaque sol sur les terres, karst, et limite de nourriture d'un territoire de 19 cases
    /// tout en champs, avec et sans la fertilité (les vallées alluviales ne doivent pas devenir des
    /// métropoles dès le Néolithique).
    #[test]
    #[ignore]
    fn bilan_geologie() {
        use crate::geologie::{Roche, Sol};
        use crate::intendant::{rendement_max, Amenagement};
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or(vec![777, 2]);
        for seed in seeds {
            let m = WorldMap::generate(192, 111, seed);
            let terres: Vec<Hex> = m.hexes().filter(|&h| m.roche(h).is_some()).collect();
            let n = terres.len() as f32;
            let part = |k: usize| format!("{:.1} %", 100.0 * k as f32 / n);
            println!("graine {seed} : {} cases de terre", terres.len());
            for r in Roche::TOUTES {
                println!("  {:<26} {}", r.nom(), part(terres.iter().filter(|&&h| m.roche(h) == Some(r)).count()));
            }
            println!("  karst : {}", part(terres.iter().filter(|&&h| m.karstique(h)).count()));
            // Causes du basalte et du volcanisme (croûte océanique, rifts et dorsales, points chauds)
            let idx = |h: Hex| m.index(h).unwrap();
            let oceanique = terres.iter().filter(|&&h| !m.plates.continental(idx(h))).count();
            let pres = |kinds: &[crate::plates::Boundary], km: f32| terres.iter().filter(|&&h| { let i = idx(h); m.plates.boundary_km[i] < km && kinds.contains(&m.plates.edges[m.plates.nearest_edge[i] as usize].kind) }).count();
            let chaud = terres.iter().filter(|&&h| m.plates.hotspots.iter().any(|&p| m.wrap.distance(h, p) as f32 * m.region.cell_km < crate::geologie::POINT_CHAUD_KM)).count();
            println!("  (croûte océanique {}, près d'un rift {}, près d'un point chaud {})", part(oceanique), part(pres(&[crate::plates::Boundary::Rift], crate::geologie::RIFT_KM)), part(chaud));
            for s in Sol::TOUS {
                println!("  {:<26} {}", s.nom(), part(terres.iter().filter(|&&h| m.sol(h) == Some(s)).count()));
            }
            // Limite d'un territoire de 19 cases tout en champs (là où un champ est possible)
            let champ = |h: Hex| Amenagement::possibles(&m, h).contains(&Amenagement::Champ);
            let mut avec: Vec<f64> = Vec::new();
            let mut sans: Vec<f64> = Vec::new();
            for &c in terres.iter().filter(|&&h| champ(h)) {
                let cases: Vec<Hex> = m.range(c, 2).map(|h| m.canonical(h)).filter(|&h| m.contains(h)).collect();
                avec.push(cases.iter().map(|&h| if champ(h) { rendement_max(&m, h, Some(Amenagement::Champ)).nourriture } else { rendement_max(&m, h, None).nourriture }).sum());
                // (sans la fertilité : le champ divisé par le multiplicateur de son sol)
                sans.push(cases.iter().map(|&h| if champ(h) { rendement_max(&m, h, Some(Amenagement::Champ)).nourriture / m.sol(h).map_or(1.0, |s| s.fertilite()) } else { rendement_max(&m, h, None).nourriture }).sum());
            }
            let quantile = |v: &mut Vec<f64>, q: f64| {
                v.sort_by(f64::total_cmp);
                v.get(((v.len() as f64 - 1.0) * q) as usize).copied().unwrap_or(0.0)
            };
            println!(
                "  limite (milliers d'habitants, 19 cases en champs) : médiane {:.0} → {:.0}, 95 % {:.0} → {:.0}, maximum {:.0} → {:.0}",
                quantile(&mut sans, 0.5), quantile(&mut avec, 0.5), quantile(&mut sans, 0.95), quantile(&mut avec, 0.95), quantile(&mut sans, 1.0), quantile(&mut avec, 1.0)
            );
        }
    }

    /// Bilan du relief tectonique (à lancer à la main) : part de terres, reliefs, altitudes.
    #[test]
    #[ignore]
    fn bilan_tectonique() {
        for seed in [777u32, 2, 5] {
            let t = std::time::Instant::now();
            let wrap = Wrap::new(192, true);
            let region = crate::climate::Region::planet(crate::climate::CELL_KM, 111);
            let p = crate::plates::generate(192, 111, wrap, seed, region.cell_km);
            let t1 = t.elapsed().as_secs_f32();
            let _ = p.elevation(seed, region.cell_km);
            println!("plaques {t1:.2} s, altitude {:.2} s", t.elapsed().as_secs_f32() - t1);
            let t = std::time::Instant::now();
            let m = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let n = m.hexes().count() as f32;
            let land: Vec<Hex> = m.hexes().filter(|&h| !m.get(h).unwrap().is_water()).collect();
            let area = |h: Hex| m.latitude(h).to_radians().cos().max(0.02);
            let total: f32 = m.hexes().map(area).sum();
            let land_area: f32 = land.iter().map(|&h| area(h)).sum();
            let count = |r: Relief| land.iter().filter(|&&h| m.relief(h) == r).count();
            let lake = |h: Hex| m.get(h) == Some(Terrain::Lake);
            // Ruptures de pente le long des cours d'eau (cascades probables) : plus de 300 m de
            // dénivelée entre une case et sa case aval
            let drop_over = |h: Hex, limit: f32| {
                m.downstream(h).is_some_and(|d| {
                    let down = m.wrap().neighbor(h, d);
                    m.elevation(h).unwrap_or(0.0) - m.elevation(down).unwrap_or(0.0).max(0.0) > limit && !m.get(down).unwrap().is_water()
                })
            };
            let river_steps = m.hexes().filter(|&h| !m.is_fleuve(h) && (0..6).any(|d| m.river_flow(h, d) > 0.0) && drop_over(h, 300.0)).count();
            let fleuve_steps = m.hexes().filter(|&h| m.is_fleuve(h) && drop_over(h, 150.0)).count();
            let steps = format!("rivières {river_steps}, fleuves {fleuve_steps} / {}", m.hexes().filter(|&h| m.is_fleuve(h)).count());
            // (lacs des rifts : à moins de 120 km d'un rift)
            let near_rift = |h: Hex| {
                m.index(h).is_some_and(|i| m.plates.boundary_km[i] < 120.0 && m.plates.edges.get(m.plates.nearest_edge[i] as usize).is_some_and(|e| e.kind == crate::plates::Boundary::Rift))
            };
            let lakes = format!(
                "{} (au-delà de 55° : {} ; près d'un rift : {})",
                m.hexes().filter(|&h| lake(h)).count(),
                m.hexes().filter(|&h| lake(h) && m.latitude(h).abs() > 55.0).count(),
                m.hexes().filter(|&h| lake(h) && near_rift(h)).count()
            );
            let cont_land = land.iter().filter(|&&h| m.index(h).is_some_and(|i| m.plates.continental(i))).count();
            // Passages maritimes qui font le tour, près de chaque pôle (26 lignes) et en tout
            let passage = |rows: std::ops::Range<i32>| {
                let sea: Vec<bool> = m.hexes().map(|h| rows.contains(&h.to_offset().1) && m.get(h).is_some_and(|t| t.is_sea())).collect();
                if crate::plates::has_sea_passage(&sea, 192) { "oui" } else { "non" }
            };
            let cont = m.hexes().filter(|&h| m.index(h).is_some_and(|i| m.plates.continental(i))).count();
            println!("graine {seed} : croûte continentale {:.0} % des cases", cont as f32 / n * 100.0);
            let polar_land = m.hexes().filter(|&h| m.latitude(h).abs() > 70.0 && !m.get(h).unwrap().is_water()).count();
            println!(
                "graine {seed} : passage nord {}, sud {}, en tout {} ; terres au-delà de 70° : {polar_land}",
                passage(0..26),
                passage(85..111),
                passage(0..111)
            );
            println!(
                "graine {seed} ({:.1} s) : ruptures {steps} ; terres {:.0} % des cases, {:.0} % de la surface ; sur croûte continentale {:.0} % ; montagnes {} pics {} accidenté {} ; lacs {} ; départ {:?}",
                t.elapsed().as_secs_f32(),
                land.len() as f32 / n * 100.0,
                land_area / total * 100.0,
                cont_land as f32 / land.len().max(1) as f32 * 100.0,
                count(Relief::Mountain),
                count(Relief::Peak),
                count(Relief::Rugged),
                lakes,
                m.find_start().map(|h| h.to_offset())
            );
        }
    }
}

#[cfg(test)]
mod fond_marin {
    use super::*;

    #[test]
    fn un_delta_gagne_sur_la_mer_et_le_fleuve_le_traverse() {
        let mut m = WorldMap::generate_in(128, 74, 5, crate::climate::Region::planet(crate::climate::CELL_KM, 74));
        let grown = m.delta_cells().to_vec();
        assert!(!grown.is_empty(), "au moins un delta gagne sur la mer");
        for &(c, _) in &grown {
            let h = m.hex_at(c);
            assert!(!m.get(h).unwrap().is_sea(), "case gagnée : de la terre");
            assert!(m.elevation(h).unwrap() > 0.0 && m.elevation(h).unwrap() <= DELTA_LAND_M[0]);
        }
        // Le fleuve traverse la terre gagnée : chaque écoulement imposé est bien suivi
        for &(from, to) in &m.delta_routes {
            let h = m.hex_at(from);
            let down = m.downstream(h).map(|d| m.cell(h + crate::hex::DIRECTIONS[d]));
            assert_eq!(down, Some(to), "écoulement imposé depuis {:?}", h.to_offset());
        }
        // Aucune incohérence, et refaire le climat ne fait pas grandir les deltas
        assert!(crate::debug_cmd::verify(&m, None).is_empty());
        let before = m.tiles.iter().filter(|t| t.is_sea()).count();
        m.regenerate_climate();
        assert_eq!(m.delta_cells().len(), grown.len());
        assert_eq!(m.tiles.iter().filter(|t| t.is_sea()).count(), before);
    }

    #[test]
    fn on_marche_sur_la_banquise_mais_pas_sur_la_mer() {
        let mut m = WorldMap::filled(8, 8, Terrain::Ocean);
        let (land, ice, sea) = (Hex::from_offset(3, 3), Hex::from_offset(4, 3), Hex::from_offset(5, 3));
        m.set(land, Terrain::Plains);
        m.freeze(ice);
        assert_eq!(m.move_cost(ice), Some(ICE_MOVE_COST));
        assert_eq!(m.move_cost(sea), None, "la mer libre reste infranchissable");
        assert_eq!(m.step(land, ice).map(StepCost::total), Some(ICE_MOVE_COST));
        assert_eq!(m.step(ice, sea), None);
        // Une côte escarpée barre le passage, dans les deux sens
        m.set_elevation(land, 1_200.0);
        assert!(m.cliff_with_sea(land, ice));
        assert_eq!(m.step(land, ice), None);
        assert_eq!(m.step(ice, land), None);
    }

    #[test]
    fn bord_de_la_banquise_varie_et_se_raccorde() {
        let (w, h) = (192, 111);
        let a = ice_edge_noise(w, h, 777, true);
        assert_eq!(a, ice_edge_noise(w, h, 777, true), "même graine, même bord");
        assert_ne!(a, ice_edge_noise(w, h, 2, true));
        // Il varie vraiment, d'environ le seuil de température de plusieurs rangées de latitude
        let (lo, hi) = a.iter().fold((f32::MAX, f32::MIN), |(l, u), &v| (l.min(v), u.max(v)));
        assert!(hi - lo > 5.0, "de {lo} à {hi}");
        // Pas de coupure entre le dernier et le premier bord de la carte
        for row in 0..h {
            let (east, west) = (a[(row * w + w - 1) as usize], a[(row * w) as usize]);
            assert!((east - west).abs() < 2.0, "rangée {row} : {east} et {west}");
        }
    }

    #[test]
    fn la_banquise_ne_couvre_que_la_mer() {
        let mut m = WorldMap::filled(6, 6, Terrain::Ocean);
        m.ice = vec![true; 36];
        let h = Hex::from_offset(2, 2);
        assert!(m.is_ice(h));
        // Une case devenue terre (console de debug) n'est plus de la banquise
        m.set(h, Terrain::Plains);
        assert!(!m.is_ice(h));
        // Monde sans banquise calculée : aucune
        assert!(!WorldMap::filled(6, 6, Terrain::Ocean).is_ice(h));
    }

    /// Bilan des embouchures de fleuve : formes, flux de sédiments et marnage, par graine :
    /// `cargo test bilan_embouchures -- --ignored --nocapture` (`SEEDS=777,2` ; `MOUTHS_LIST=1` : chacune).
    #[test]
    #[ignore]
    fn bilan_embouchures() {
        use crate::mouths::MouthKind;
        let seeds: Vec<u32> = std::env::var("SEEDS").map_or(vec![777, 2, 5, 4242], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
        let mut totals = vec![0usize; MouthKind::ALL.len()];
        for seed in seeds {
            let t = std::time::Instant::now();
            let m = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            println!("génération {:.1} s", t.elapsed().as_secs_f32());
            let med = |f: &dyn Fn(&crate::mouths::Mouth) -> f32| {
                let mut v: Vec<f32> = m.mouths().iter().map(|(_, q)| f(q)).collect();
                v.sort_by(f32::total_cmp);
                v.get(v.len() / 2).copied().unwrap_or(0.0)
            };
            let dominant = |pick: &dyn Fn(&crate::mouths::Mouth) -> bool| m.mouths().iter().filter(|(_, q)| pick(q)).count();
            print!("graine {seed} : {} embouchures ;", m.mouths().len());
            for (k, kind) in MouthKind::ALL.iter().enumerate() {
                let n = m.mouths().iter().filter(|(_, q)| q.kind == *kind).count();
                totals[k] += n;
                print!(" {} {n},", kind.name());
            }
            let on_delta = m.mouths().iter().filter(|(i, _)| m.delta_cells().iter().any(|c| c.0 == *i)).count();
            println!("\n  cases gagnées sur la mer {}, embouchures sur une de ces cases {on_delta}", m.delta_cells().len());
            println!(
                "  flux médians (kg/s) : fleuve {:.0}, houle {:.0}, marée {:.0} ; plus fort : fleuve {}, houle {}, marée {}",
                med(&|q| q.river), med(&|q| q.wave), med(&|q| q.tide),
                dominant(&|q| q.river >= q.wave && q.river >= q.tide), dominant(&|q| q.wave > q.river && q.wave >= q.tide), dominant(&|q| q.tide > q.river && q.tide > q.wave),
            );
            if std::env::var("MOUTHS_LIST").is_ok() {
                for (i, q) in m.mouths() {
                    println!("   {:?} {} : débit {:.0}, flux {:.0} / {:.0} / {:.0}", m.hex_at(*i).to_offset(), q.kind.name(), m.discharge(m.hex_at(*i)), q.river, q.wave, q.tide);
                }
            }
        }
        println!("total : {:?}", MouthKind::ALL.iter().map(|k| k.name()).zip(&totals).collect::<Vec<_>>());
    }

    /// Bilan de la houle : hauteur par bande de latitude, part de houle venue d'ailleurs, période,
    /// côtes, temps de calcul : `cargo test bilan_houle -- --ignored --nocapture` (`SEEDS=777,2`).
    #[test]
    #[ignore]
    fn bilan_houle() {
        let seeds: Vec<u32> = std::env::var("SEEDS").map_or(vec![777, 2, 5], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
        for seed in seeds {
            let t = std::time::Instant::now();
            let m = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let total = t.elapsed().as_secs_f32();
            let w = m.waves();
            let sea: Vec<usize> = (0..m.tiles.len()).filter(|&i| m.tiles[i].is_sea() && !m.ice[i]).collect();
            let ice: Vec<usize> = (0..m.tiles.len()).filter(|&i| m.ice[i]).collect();
            let ice_lat = |north: bool| ice.iter().map(|&i| m.latitude(m.hex_at(i))).filter(|&l| (l > 0.0) == north).map(f32::abs).fold(f32::MAX, f32::min);
            let in_rows = |lo: f32| {
                let rows: Vec<usize> = (0..m.tiles.len()).filter(|&i| m.tiles[i].is_sea() && m.latitude(m.hex_at(i)).abs() >= lo).collect();
                rows.iter().filter(|&&i| m.ice[i]).count() as f32 / rows.len().max(1) as f32 * 100.0
            };
            println!(
                "  banquise : {} cases ({:.0} % des mers), jusqu'à {:.0}°N et {:.0}°S ; part gelée au-delà de 60° {:.0} %, de 70° {:.0} %, de 80° {:.0} % ; {} cases gelées dans un banc de poissons",
                ice.len(), ice.len() as f32 / (ice.len() + sea.len()).max(1) as f32 * 100.0, ice_lat(true), ice_lat(false), in_rows(60.0), in_rows(70.0), in_rows(80.0), ice.iter().filter(|&&i| m.bank(m.hex_at(i)).is_some()).count(),
            );
            let lat = |i: usize| m.latitude(m.hex_at(i)).abs();
            let pct = |mut v: Vec<f32>, p: f32| {
                v.sort_by(f32::total_cmp);
                if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f32 * p) as usize] }
            };
            let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
            let heights: Vec<f32> = sea.iter().map(|&i| w.height[i]).collect();
            println!(
                "graine {seed} : génération {total:.1} s ; {} cases de mer, {} sous la banquise ; hauteur (m) médiane {:.2}, 10 % {:.2}, 90 % {:.2}, max {:.1} ; plus forte de l'année médiane {:.1}, max {:.1}",
                sea.len(), ice.len(), pct(heights.clone(), 0.5), pct(heights.clone(), 0.1), pct(heights.clone(), 0.9), pct(heights, 1.0),
                pct(sea.iter().map(|&i| w.storm[i]).collect(), 0.5), pct(sea.iter().map(|&i| w.storm[i]).collect(), 1.0),
            );
            for (lo, hi) in [(0.0, 15.0), (15.0, 30.0), (30.0, 45.0), (45.0, 60.0), (60.0, 90.0)] {
                let band: Vec<usize> = sea.iter().copied().filter(|&i| lat(i) >= lo && lat(i) < hi).collect();
                let of = |f: &dyn Fn(usize) -> f32| mean(&band.iter().map(|&i| f(i)).collect::<Vec<f32>>());
                println!(
                    "  {lo:>2.0}-{hi:<2.0}° : {:>5} cases, hauteur {:.2} m, tempête {:.1} m, période {:.1} s, houle venue d'ailleurs {:.0} %, direction nette {:.0} %",
                    band.len(), of(&|i| w.height[i]), of(&|i| w.storm[i]), of(&|i| w.period[i]), of(&|i| w.swell_share[i]) * 100.0, of(&|i| w.focus[i]) * 100.0,
                );
            }
            let shore: Vec<usize> = sea.iter().copied().filter(|&i| w.shore_power[i] > 0.0).collect();
            let of = |f: &dyn Fn(usize) -> f32| shore.iter().map(|&i| f(i)).collect::<Vec<f32>>();
            let classes = [0.6f32, 1.5].map(|h| shore.iter().filter(|&&i| w.shore_height[i] < h).count());
            println!(
                "  côtes : {} cases ; déferlement (m) médiane {:.2}, 90 % {:.2}, max {:.1} ; énergie (kW/m) médiane {:.1}, 90 % {:.1}, max {:.0} ; dérive (kW/m) médiane {:.1} ; moins de 0,6 m : {}, moins de 1,5 m : {}",
                shore.len(), pct(of(&|i| w.shore_height[i]), 0.5), pct(of(&|i| w.shore_height[i]), 0.9), pct(of(&|i| w.shore_height[i]), 1.0),
                pct(of(&|i| w.shore_power[i]), 0.5), pct(of(&|i| w.shore_power[i]), 0.9), pct(of(&|i| w.shore_power[i]), 1.0),
                pct(of(&|i| w.shore_drift[i].0.hypot(w.shore_drift[i].1)), 0.5), classes[0], classes[1],
            );
        }
    }

    /// Bilan des courants marins : vitesses, écarts de température de la mer, courant
    /// circumpolaire, rencontres de courants. `CURRENTS_DUMP=<dossier>` écrit pour chaque case
    /// « terre/mer, température, écart, u, v » (une ligne par case) :
    /// `cargo test bilan_courants -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bilan_marees() {
        let seeds: Vec<u32> = std::env::var("SEEDS").map_or(vec![777, 2, 5], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
        for seed in seeds {
            let m = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let sea: Vec<usize> = (0..m.tiles.len()).filter(|&i| m.tiles[i].is_sea()).collect();
            let t = std::time::Instant::now();
            let land: Vec<bool> = m.tiles.iter().map(|t| t.is_sea()).collect();
            let _ = crate::tides::simulate(m.width, m.height, &land, &m.depth, &crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let took = t.elapsed().as_secs_f32();
            let tides = m.tides();
            let coast = |i: usize| m.neighbors(m.hex_at(i)).iter().any(|&n| m.get(n).is_some_and(|t| !t.is_water()));
            let pct = |mut v: Vec<f32>, p: f32| {
                v.sort_by(f32::total_cmp);
                if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f32 * p) as usize] }
            };
            let open: Vec<f32> = sea.iter().copied().filter(|&i| m.depth[i] > 3_000.0).map(|i| tides.spring_range(i)).collect();
            let shelf: Vec<f32> = sea.iter().copied().filter(|&i| m.depth[i] < SHELF_DEPTH_M).map(|i| tides.spring_range(i)).collect();
            let shore: Vec<f32> = sea.iter().copied().filter(|&i| coast(i)).map(|i| tides.spring_range(i)).collect();
            let current: Vec<f32> = sea.iter().map(|&i| tides.current[i]).collect();
            let still = sea.iter().filter(|&&i| tides.amplitude[i] < 0.05).count();
            println!(
                "graine {seed} : marnage de vives-eaux (m) en pleine mer (> 3 000 m) médiane {:.2}, 90 % {:.2} ; plateau médiane {:.2}, 90 % {:.2}, max {:.1} ; côtes médiane {:.2}, 10 % {:.2}, 90 % {:.2}, 99 % {:.1}, max {:.1} ; courant (m/s) médiane {:.2}, 99 % {:.2}, max {:.2} ; cases presque sans marée {still} ; calcul {took:.1} s",
                pct(open.clone(), 0.5), pct(open, 0.9), pct(shelf.clone(), 0.5), pct(shelf.clone(), 0.9), pct(shelf, 1.0),
                pct(shore.clone(), 0.5), pct(shore.clone(), 0.1), pct(shore.clone(), 0.9), pct(shore.clone(), 0.99), pct(shore, 1.0),
                pct(current.clone(), 0.5), pct(current.clone(), 0.99), pct(current, 1.0),
            );
            // Les plus forts marnages côtiers : où sont-ils ?
            let mut best: Vec<usize> = sea.iter().copied().filter(|&i| coast(i)).collect();
            best.sort_by(|&a, &b| tides.spring_range(b).total_cmp(&tides.spring_range(a)));
            for &i in best.iter().take(5) {
                println!("   {:?} marnage {:.1} m, fond {:.0} m, courant {:.2} m/s", m.hex_at(i).to_offset(), tides.spring_range(i), m.depth[i], tides.current[i]);
            }
        }
    }

    #[test]
    #[ignore]
    fn bilan_courants() {
        for seed in [777, 2, 5] {
            let m = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let c = m.currents();
            let sea: Vec<usize> = (0..m.tiles.len()).filter(|&i| m.tiles[i].is_sea()).collect();
            let mut speed: Vec<f32> = sea.iter().map(|&i| c.speed(i)).collect();
            speed.sort_by(f32::total_cmp);
            let pct = |v: &[f32], p: f32| v[((v.len() - 1) as f32 * p) as usize];
            let mut anomaly: Vec<f32> = sea.iter().map(|&i| c.anomaly[i]).collect();
            anomaly.sort_by(f32::total_cmp);
            let front = sea.iter().filter(|&&i| m.marine[i] & marine::FRONT != 0).count();
            let upwelling: Vec<f32> = sea.iter().filter(|&&i| m.marine[i] & marine::UPWELLING != 0).map(|&i| c.anomaly[i]).collect();
            let upwelling = upwelling.iter().sum::<f32>() / upwelling.len().max(1) as f32;
            let coast_anomaly = |west: bool, band: std::ops::Range<f32>| {
                // Côtes ouest (terre à l'est) et est des continents, dans une bande de latitudes
                let v: Vec<f32> = sea
                    .iter()
                    .copied()
                    .filter(|&i| band.contains(&m.latitude(m.hex_at(i)).abs()))
                    .filter(|&i| {
                        let h = m.hex_at(i);
                        m.get(m.wrap.neighbor(h, if west { 0 } else { 3 })).is_some_and(|t| !t.is_water())
                    })
                    .map(|i| c.anomaly[i])
                    .collect();
                v.iter().sum::<f32>() / v.len().max(1) as f32
            };
            println!(
                "graine {seed} : vitesse (m/s) médiane {:.2}, 90 % {:.2}, 99 % {:.2}, max {:.2} ; écart (°C) 1 % {:.1}, 10 % {:.1}, 90 % {:.1}, 99 % {:.1} ; côtes ouest 15-35° {:.1}, côtes est {:.1}, côtes ouest 45-65° {:.1}, côtes est {:.1} ; remontées d'eaux {upwelling:.1} ; circumpolaire {:?} ; rencontres {front}",
                pct(&speed, 0.5),
                pct(&speed, 0.9),
                pct(&speed, 0.99),
                speed[speed.len() - 1],
                pct(&anomaly, 0.01),
                pct(&anomaly, 0.1),
                pct(&anomaly, 0.9),
                pct(&anomaly, 0.99),
                coast_anomaly(true, 15.0..35.0),
                coast_anomaly(false, 15.0..35.0),
                coast_anomaly(true, 45.0..65.0),
                coast_anomaly(false, 45.0..65.0),
                c.circumpolar,
            );
            // Vent d'ouest moyen sur la mer par bande de latitude (la jonction des tourbillons suit le
            // maximum des vents d'ouest)
            let bands: Vec<String> = (0..9)
                .map(|b| {
                    let (lo, hi) = (b as f32 * 10.0, b as f32 * 10.0 + 10.0);
                    let v: Vec<f32> = sea.iter().copied().filter(|&i| (lo..hi).contains(&m.latitude(m.hex_at(i)).abs())).map(|i| m.climate.wind[i].0).collect();
                    format!("{lo:.0}-{hi:.0}° {:.2}", v.iter().sum::<f32>() / v.len().max(1) as f32)
                })
                .collect();
            println!("  vent d'ouest sur la mer : {}", bands.join(", "));
            if let Ok(dir) = std::env::var("CURRENTS_DUMP") {
                let out: String = (0..m.tiles.len())
                    .map(|i| {
                        let (u, v) = c.velocity[i];
                        format!("{} {:.2} {:.2} {:.3} {:.3}
", u8::from(m.tiles[i].is_sea()), c.temperature[i], c.anomaly[i], u, v)
                    })
                    .collect();
                std::fs::write(format!("{dir}/currents_{seed}.txt"), out).unwrap();
            }
        }
    }

    /// Bilan du fond marin et des eaux poissonneuses : `cargo test bilan_fond_marin -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bilan_fond_marin() {
        for seed in [777, 4242, 31] {
            let m = WorldMap::generate_in(192, 111, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
            let sea: Vec<Hex> = m.hexes().filter(|&h| m.get(h).is_some_and(|t| t.is_sea())).collect();
            let count = |f: &dyn Fn(f32) -> bool| sea.iter().filter(|&&h| f(m.depth(h).unwrap())).count();
            let flag = |b: u8| sea.iter().filter(|&&h| m.marine(h) & b != 0).count();
            let near_land = sea.iter().filter(|&&h| m.neighbors(h).iter().any(|&n| m.get(n).is_some_and(|t| !t.is_water()))).count();
            let deep_shore = sea
                .iter()
                .filter(|&&h| m.get(h) == Some(Terrain::Ocean) && m.neighbors(h).iter().any(|&n| m.get(n).is_some_and(|t| !t.is_water())))
                .count();
            if let Ok(dir) = std::env::var("DEPTH_DUMP") {
                let out: String = m
                    .hexes()
                    .map(|h| {
                        let class = match m.depth(h) {
                            None => 0,
                            Some(d) if d < SHELF_DEPTH_M => 1,
                            Some(d) if d < ABYSS_DEPTH_M => 2,
                            Some(d) if d < TRENCH_DEPTH_M => 3,
                            _ => 4,
                        };
                        format!("{class} {}\n", m.marine(h))
                    })
                    .collect();
                std::fs::write(format!("{dir}/depth_{seed}.txt"), out).unwrap();
            }
            println!(
                "graine {seed} : mer {} ; plateau {} ; talus {} ; abysses {} ; fosses {} ; cases au bord des terres {near_land} dont haute mer {deep_shore} ; remontées {} ; monts {} ; îles {}",
                sea.len(),
                count(&|d| d < SHELF_DEPTH_M),
                count(&|d| (SHELF_DEPTH_M..ABYSS_DEPTH_M).contains(&d)),
                count(&|d| (ABYSS_DEPTH_M..TRENCH_DEPTH_M).contains(&d)),
                count(&|d| d >= TRENCH_DEPTH_M),
                flag(marine::UPWELLING),
                flag(marine::SEAMOUNT),
                flag(marine::ISLAND),
            );
        }
    }
}

#[cfg(test)]
mod lieux {
    use super::*;

    #[test]
    fn chaine_des_lieux_d_une_case() {
        let m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        // Une case de terre d'un continent : il ouvre la chaîne, sa région naturelle suit
        let land = m.hexes().find(|&h| {
            let c = m.cell(h);
            m.lands().mass_of[c].is_some_and(|z| m.lands().zones[z as usize].kind == crate::lands::LandKind::Continent) && m.lands().group_of[c].is_none() && m.regions().region_of[c].is_some()
        });
        let land = land.expect("une case de continent dans une région naturelle");
        let places = m.places(land);
        let c = m.cell(land);
        let continent = m.lands().mass_of[c].unwrap() as usize;
        assert_eq!(places[0], (Place::Land(continent), m.lands().zones[continent].name.clone()), "{places:?}");
        let region = m.regions().region_of[c].unwrap() as usize;
        assert!(places.contains(&(Place::Region(region), m.regions().zones[region].name.clone())), "{places:?}");
        // Une case de haute mer : son océan d'abord
        let sea = m.hexes().find(|&h| m.get(h) == Some(Terrain::Ocean) && m.seas().basin_of[m.cell(h)].is_some()).expect("haute mer");
        let ocean = m.seas().basin_of[m.cell(sea)].unwrap() as usize;
        assert_eq!(m.places(sea)[0], (Place::Sea(ocean), m.seas().zones[ocean].name.clone()));
        // Pas de doublon ; rien au-delà d'un pôle
        for h in m.hexes() {
            let places = m.places(h);
            let unique: std::collections::HashSet<&String> = places.iter().map(|(_, n)| n).collect();
            assert_eq!(unique.len(), places.len(), "{places:?}");
        }
        assert!(m.places(Hex::from_offset(3, -5)).is_empty());
    }

    #[test]
    fn axe_d_une_zone_allongee() {
        let m = WorldMap::filled(40, 20, Terrain::Grassland);
        // Une bande de 20 × 2 cases : son axe va d'un bout à l'autre ; un carré de 6 × 6 n'en a pas
        let band: Vec<usize> = (0..20).flat_map(|x| [(5 * 40 + 10 + x) as usize, (6 * 40 + 10 + x) as usize]).collect();
        let spine = m.zone_spine(&band);
        assert!(spine.len() >= 19, "{}", spine.len());
        for w in spine.windows(2) {
            assert_eq!(w[0].distance(w[1]), 1, "d'un seul tenant");
        }
        let square: Vec<usize> = (0..6).flat_map(|y| (0..6).map(move |x| ((y + 2) * 40 + x + 2) as usize)).collect();
        assert!(m.zone_spine(&square).is_empty());
    }

    /// Effet de l'érosion après le climat : `cargo test bilan_incision -- --ignored --nocapture`
    /// (`SEEDS=777,2`) : embouchures hautes, hautes terres, falaises, lacs, chutes des fleuves.
    #[test]
    #[ignore]
    fn bilan_incision() {
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![777, 2, 4242]);
        for seed in seeds {
            let m = WorldMap::generate_in(192, 111, seed, Region::planet(crate::climate::CELL_KM, 111));
            let mouths: Vec<f32> = m.mouths().iter().filter(|(i, _)| !m.delta_cells.iter().any(|c| c.0 == *i)).map(|(i, _)| m.elevation[*i]).collect();
            let land: Vec<usize> = (0..m.tiles.len()).filter(|&i| !m.tiles[i].is_water()).collect();
            let down = |i: usize| { let h = m.hex_at(i); m.downstream(h).and_then(|d| m.index(h + crate::hex::DIRECTIONS[d])) };
            let cliffs = land.iter().map(|&i| m.neighbors(m.hex_at(i)).iter().filter(|&&nb| m.cliff_between(m.hex_at(i), nb)).count()).sum::<usize>() / 2;
            let falls = land.iter().filter(|&&i| m.is_fleuve(m.hex_at(i)) && down(i).is_some_and(|j| m.elevation[i] - m.elevation[j] > 150.0)).count();
            let high = land.iter().filter(|&&i| m.elevation[i] >= 1_500.0).count();
            let mean = land.iter().map(|&i| f64::from(m.elevation[i])).sum::<f64>() / land.len().max(1) as f64;
            let lakes = m.tiles.iter().filter(|&&t| t == Terrain::Lake).count();
            let mountains = land.iter().filter(|&&i| matches!(m.relief[i], Relief::Mountain | Relief::Peak)).count();
            let highest = mouths.iter().copied().fold(0.0f32, f32::max);
            // Peigne : part des hautes terres touchées par une rivière, confluences pour 100 arêtes
            let high_cells: Vec<usize> = land.iter().copied().filter(|&i| m.elevation[i] >= 1_000.0).collect();
            let touched = high_cells.iter().filter(|&&i| (0..6).any(|d| m.river_flow(m.hex_at(i), d) > 0.0)).count();
            let mut arrivals: bevy::platform::collections::HashMap<u32, u32> = bevy::platform::collections::HashMap::default();
            for s in &m.hydro.river_segments {
                *arrivals.entry(s.down).or_insert(0) += 1;
            }
            let confluences = arrivals.values().filter(|&&k| k >= 2).count();
            println!("  peigne : hautes terres touchées {:.0} %, confluences pour 100 arêtes {:.1}, arêtes {}", 100.0 * touched as f32 / high_cells.len().max(1) as f32, 100.0 * confluences as f32 / m.hydro.river_segments.len().max(1) as f32, m.hydro.river_segments.len());
            if std::env::var("MOUTHS_LIST").is_ok() { for (i, q) in m.mouths().iter().filter(|(i, _)| !m.delta_cells.iter().any(|c| c.0 == *i)) { println!("  {:?} {:?} {:.0} m, marnage {:.1} m, fleuve {:.0} houle {:.0} maree {:.0} kg/s", m.hex_at(*i).to_offset(), q.kind, m.elevation[*i], m.tides.spring_range(m.downstream(m.hex_at(*i)).and_then(|d| m.index(m.hex_at(*i) + crate::hex::DIRECTIONS[d])).unwrap_or(*i)), q.river, q.wave, q.tide); } }
            println!("graine {seed} : embouchures {}, à 40 m ou plus {}, à 200 m ou plus {}, à 500 m ou plus {}, la plus haute {highest:.0} m ; moyenne {mean:.0} m, à 1 500 m ou plus {high}, montagnes {mountains}, falaises {cliffs}, lacs {lakes}, chutes de fleuve {falls}", mouths.len(), mouths.iter().filter(|&&a| a >= 40.0).count(), mouths.iter().filter(|&&a| a >= 200.0).count(), mouths.iter().filter(|&&a| a >= 500.0).count());
        }
    }

    /// Fins de rivière : `cargo test bilan_fins_de_riviere -- --ignored --nocapture` (`SEEDS=777,2` ;
    /// `FINS_LIST=1` : chacune) : rivières qui s'arrêtent sans rejoindre aucune eau, à plein débit
    /// ou parce qu'elles se tarissent (le débit de sortie passe sous le seuil).
    #[test]
    #[ignore]
    fn bilan_fins_de_riviere() {
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![777, 2, 4242]);
        let list = std::env::var("FINS_LIST").is_ok();
        for seed in seeds {
            let m = WorldMap::generate_in(192, 111, seed, Region::planet(crate::climate::CELL_KM, 111));
            let segs = &m.hydro.river_segments;
            let leaving: bevy::platform::collections::HashSet<u32> = segs.iter().map(|s| s.up).collect();
            let mut ends: bevy::platform::collections::HashMap<u32, f32> = bevy::platform::collections::HashMap::default();
            for s in segs {
                if !leaving.contains(&s.down) {
                    let cells = m.hydro.corner_cells[s.down as usize];
                    let wet = cells.iter().any(|&c| c == u32::MAX || { let c = c as usize; m.tiles[c].is_water() || m.hydro.fleuve[c] || m.hydro.small_lake[c] });
                    if !wet {
                        let e = ends.entry(s.down).or_insert(0.0);
                        *e += s.flow;
                    }
                }
            }
            let at = |min: f32| ends.values().filter(|&&q| q >= min).count();
            println!("graine {seed} : rivières {}, fins sans eau {}, à 200 m³/s ou plus {}, à 500 ou plus {}, à 1 000 ou plus {}", segs.len(), ends.len(), at(200.0), at(500.0), at(1_000.0));
            if list {
                for (c, q) in &ends {
                    let cells = m.hydro.corner_cells[*c as usize].map(|i| m.hex_at(i as usize).to_offset());
                    println!("  {q:.0} m³/s au coin {cells:?}");
                }
            }
        }
    }

    /// Pluie comparée à la Terre : `cargo test bilan_pluie_comparatif -- --ignored --nocapture`
    /// (`SEEDS=777,2` ; 8 graines par défaut). Médianes par bande de latitude et par situation (côte
    /// ouest ou est à 2 cases au plus de la mer, intérieur à 8 cases ou plus, hautes terres à 1 500 m
    /// ou plus), parts des biomes, puis des indicateurs de cohérence (voir
    /// `conception/conception-climat.md`, « Comparatif avec la Terre »).
    #[test]
    #[ignore]
    #[allow(clippy::needless_range_loop)]
    fn bilan_pluie_comparatif() {
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![777, 4242, 8, 31337, 90210, 2, 1234, 55555]);
        let bands = [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 91.0];
        let mut groups: std::collections::BTreeMap<(usize, &str), Vec<f32>> = Default::default();
        let mut biomes: std::collections::BTreeMap<(usize, String), usize> = Default::default();
        let (mut weight, mut rain_sum, mut runoff_sum, mut land_cells, mut deserts) = (0.0f64, 0.0f64, 0.0f64, 0usize, 0usize);
        let started = std::time::Instant::now();
        let mut sources: std::collections::BTreeMap<(usize, &str), [f64; 11]> = Default::default();
        for &seed in &seeds {
            let m = WorldMap::generate_in(192, 111, seed, Region::planet(crate::climate::CELL_KM, 111));
            let n = m.tiles.len();
            let w = m.width;
            let mut dist = vec![u16::MAX; n];
            let mut q = std::collections::VecDeque::new();
            for i in 0..n {
                if m.tiles[i].is_sea() {
                    dist[i] = 0;
                    q.push_back(i);
                }
            }
            while let Some(i) = q.pop_front() {
                for nb in m.neighbors(m.hex_at(i)) {
                    if let Some(j) = m.index(nb)
                        && dist[j] == u16::MAX
                    {
                        dist[j] = dist[i] + 1;
                        q.push_back(j);
                    }
                }
            }
            let river_cells = (0..n).filter(|&i| !m.tiles[i].is_sea() && (0..6).any(|d| m.river_flow(m.hex_at(i), d) > 0.0)).count();
            let high_river = (0..n).filter(|&i| !m.tiles[i].is_sea() && m.elevation[i] >= 1_500.0 && (0..6).any(|d| m.river_flow(m.hex_at(i), d) > 0.0)).count();
            let desert = (0..n).filter(|&i| m.tiles[i] == Terrain::Desert).count();
            println!("RIVIERES graine {seed} : {river_cells} cases de terre touchent une rivière, dont {high_river} à 1 500 m ou plus ; désert {desert} cases");
            for i in 0..n {
                if m.tiles[i].is_sea() {
                    continue;
                }
                let (col, row) = m.hex_at(i).to_offset();
                let lat = m.latitude(m.hex_at(i));
                let b = bands.windows(2).position(|x| lat.abs() >= x[0] && lat.abs() < x[1]).unwrap_or(7);
                let rain = m.climate.rain[i];
                let area = f64::from(lat.to_radians().cos().max(0.0));
                weight += area;
                rain_sum += area * f64::from(rain);
                runoff_sum += area * f64::from(m.climate.runoff[i]);
                land_cells += 1;
                deserts += usize::from(m.tiles[i] == Terrain::Desert);
                let toward = |step: i32| (1..w).find(|&k| m.tiles[(row * w + (col + step * k).rem_euclid(w)) as usize].is_sea()).unwrap_or(w);
                let (west, east) = (toward(-1), toward(1));
                let srcs = m.climate.sources.get(i).copied().unwrap_or_default();
                let mut push = |c: &'static str| {
                    groups.entry((b, c)).or_default().push(rain);
                    let e = sources.entry((b, c)).or_insert([0.0; 11]);
                    for k in 0..6 {
                        e[k] += f64::from(srcs[k]);
                    }
                    e[6] += f64::from(rain);
                    e[7] += 1.0;
                    let sr = m.climate.season_rain[i];
                    let (summer, winter) = if lat >= 0.0 { (sr[1], sr[3]) } else { (sr[3], sr[1]) };
                    e[8] += f64::from(summer);
                    e[9] += f64::from(winter);
                    e[10] += f64::from(sr[0] + sr[2]) / 2.0;
                };
                push("toutes");
                if dist[i] <= 2 && west < east {
                    push("cote-ouest");
                }
                if dist[i] <= 2 && east < west {
                    push("cote-est");
                }
                if dist[i] >= 8 {
                    push("interieur");
                }
                if m.elevation[i] >= 1_500.0 {
                    push("haut");
                }
                if m.elevation[i] < 500.0 {
                    push("bas");
                }
                if dist[i] >= 8 && m.elevation[i] < 500.0 {
                    push("int-bas");
                }
                // Versants (agent B) : altitude 2 cases sous le vent moins 2 cases au vent (vent annuel)
                {
                    const U: [(f32, f32); 6] = [(1.0, 0.0), (0.5, 0.866), (-0.5, 0.866), (-1.0, 0.0), (-0.5, -0.866), (0.5, -0.866)];
                    let (wx, wy) = m.climate.wind[i];
                    if wx.hypot(wy) > 0.05 {
                        let down = (0..6).max_by(|&a, &b| (U[a].0 * wx + U[a].1 * wy).total_cmp(&(U[b].0 * wx + U[b].1 * wy))).unwrap();
                        let up = (down + 3) % 6;
                        let walk = |d: usize| {
                            let h = m.neighbors(m.neighbors(m.hex_at(i))[d])[d];
                            m.index(h).map(|j| if m.tiles[j].is_sea() { 0.0 } else { m.elevation[j] })
                        };
                        if let (Some(zd), Some(zu)) = (walk(down), walk(up)) {
                            let rise = zd - zu;
                            if m.elevation[i] >= 500.0 && rise > 600.0 {
                                push("au-vent");
                            }
                            if m.elevation[i] >= 500.0 && rise < -600.0 {
                                push("sous-vent");
                            }
                        }
                    }
                }
                *biomes.entry((b, format!("{:?}", m.tiles[i]))).or_default() += 1;
            }
        }
        for ((b, c), v) in groups.iter_mut() {
            v.sort_by(f32::total_cmp);
            let p = |f: f32| v[((v.len() - 1) as f32 * f) as usize];
            println!("B {:>2}-{:<2} {c:<11} n {:>5} p10 {:>5.0} med {:>5.0} p90 {:>6.0}", bands[*b] as i32, bands[b + 1].min(90.0) as i32, v.len(), p(0.1), p(0.5), p(0.9));
        }
        for ((b, c), e) in &sources {
            if ["bas", "haut", "au-vent", "sous-vent", "int-bas", "cote-ouest"].contains(c) {
                let k = e[7].max(1.0);
                println!("S {:>2} {c:<11} pluie {:>5.0} (été {:>5.0} hiver {:>5.0} équinoxe {:>5.0}) | relief {:>5.0} bruine {:>4.0} profonde {:>5.0} basse {:>4.0} haute {:>4.0} sat-haute {:>4.0}", bands[*b] as i32, e[6] / k, e[8] / k, e[9] / k, e[10] / k, e[0] / k, e[1] / k, e[2] / k, e[3] / k, e[4] / k, e[5] / k);
            }
        }
        for b in 0..8 {
            let total: usize = biomes.iter().filter(|((x, _), _)| *x == b).map(|(_, k)| k).sum();
            let mut list: Vec<(String, usize)> = biomes.iter().filter(|((x, _), _)| *x == b).map(|((_, t), k)| (t.clone(), *k)).collect();
            list.sort_by_key(|x| std::cmp::Reverse(x.1));
            println!("T {}-{} : {}", bands[b] as i32, bands[b + 1].min(90.0) as i32, list.iter().take(5).map(|(t, k)| format!("{t} {:.0} %", 100.0 * *k as f32 / total.max(1) as f32)).collect::<Vec<_>>().join(", "));
        }
        let median = |b: usize, c: &str| {
            groups.get(&(b, c)).filter(|v| v.len() >= 30).map(|v| {
                let mut v = v.clone();
                v.sort_by(f32::total_cmp);
                v[v.len() / 2]
            })
        };
        // Indicateurs de cohérence (médianes ; repères de la Terre dans la doc du climat)
        let ratio = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(a), Some(b)) if b > 0.0 => format!("{:.2}", a / b),
            _ => "-".into(),
        };
        let mm = |v: Option<f32>| v.map_or("-".into(), |x| format!("{x:.0}"));
        println!(
            "INDICATEURS graines {} ; moyenne des terres {:.0} mm (Terre ≈ 850) ; ruissellement {:.0} mm (Terre 290-460) ; déserts {:.1} % des cases de terre",
            seeds.len(),
            rain_sum / weight,
            runoff_sum / weight,
            100.0 * deserts as f32 / land_cells.max(1) as f32
        );
        println!(
            "INDICATEURS équateur 0-10 {} mm (Terre 1 800-2 300) ; intérieur 20-30 {} (Terre < 250) ; côte ouest 20-30 {} (Terre < 200)",
            mm(median(0, "toutes")),
            mm(median(2, "interieur")),
            mm(median(2, "cote-ouest"))
        );
        println!(
            "INDICATEURS côte est / côte ouest 20-30 {} et 30-40 {} (Terre > 1,5) ; intérieur / côte ouest 40-50 {} et 50-60 {} (Terre < 0,7)",
            ratio(median(2, "cote-est"), median(2, "cote-ouest")),
            ratio(median(3, "cote-est"), median(3, "cote-ouest")),
            ratio(median(4, "interieur"), median(4, "cote-ouest")),
            ratio(median(5, "interieur"), median(5, "cote-ouest"))
        );
        println!(
            "INDICATEURS 60-70 {} mm (Terre 300-500) ; hautes terres / basses 30-40 {} et 40-50 {} (Terre ≈ 1,3-2) ; génération {:.1} s par graine",
            mm(median(6, "toutes")),
            ratio(median(3, "haut"), median(3, "bas")),
            ratio(median(4, "haut"), median(4, "bas")),
            started.elapsed().as_secs_f32() / seeds.len().max(1) as f32
        );
    }
}
