//! Simulation climatique de la génération du monde : pression, vents, humidité, pluie, sols.
//!
//! La carte va d'un pôle (première ligne) à l'autre (dernière), l'équateur au milieu.
//! Les grandeurs sont physiques (°C, g/kg, mm, m/s) pour que les règles restent vérifiables.
//!
//! **Pression et vents.** Ceintures alternées de part et d'autre de l'équateur (basse pression
//! équatoriale, anticyclones vers 30°, dépressions vers 60°, hautes pressions polaires), effet
//! thermique des continents, dépressions et anticyclones mobiles (un régime météo différent
//! toutes les quelques semaines simulées). Le vent de surface va des hautes vers les basses
//! pressions, dévié par la rotation de la planète : alizés, vents d'ouest et vents d'est polaires
//! en découlent. En altitude, les vents sont surtout d'ouest, avec un retour vers les pôles
//! au-dessus des alizés.
//!
//! **Deux couches d'air.** La couche basse (2 km au-dessus du sol) suit le relief, a la
//! température du sol et porte l'essentiel de la vapeur ; la couche haute (autour de 4 km) est
//! plus froide (6,5 °C par km). L'air bas monte quand les vents convergent, quand le relief le
//! soulève ou par convection (tropiques chauds et humides, sauf sous un anticyclone) : il se
//! refroidit, sature, et l'excès devient de l'eau nuageuse qui tombe en pluie (une partie
//! s'évapore en tombant dans un air sec). Là où l'air descend (anticyclones, derrière les
//! massifs), l'air sec d'altitude dessèche la couche basse.
//!
//! **Sols.** Chaque case de terre a une réserve d'eau (modèle du « seau ») : la pluie la remplit,
//! l'évaporation la vide (d'autant moins que le sol est sec), le trop-plein ruisselle (futures
//! rivières). L'évaporation potentielle suit la formule de Hargreaves (rayonnement solaire,
//! température, écart jour-nuit), bornée par le transfert vers l'air (vent, air plus ou moins
//! proche de la saturation). La mer s'évapore selon sa température, modifiée par les courants.
//! La rosée se forme la nuit quand l'air sature à la température minimale.
//!
//! La simulation est organisée par **périodes** (une seule pour l'instant : l'année moyenne) afin
//! d'accueillir plus tard les saisons. Ce module est de la logique pure (aucun rendu).

use crate::hex::Hex;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use rayon::prelude::*;

// --- Échelle et temps
/// Longueur d'un degré de latitude (km).
const KM_PER_DEGREE: f32 = 111.2;
/// Pas de temps le plus long (une demi-journée) ; il est réduit sur les petites cases pour que
/// l'air ne saute jamais une case pendant un pas.
const MAX_DT_S: f32 = 43_200.0;
/// Durées simulées (jours) : mise en place de l'air et des sols (première période), transition
/// entre deux périodes, puis régimes météo successifs dans chaque période.
const SPIN_UP_DAYS: f32 = 30.0;
const TRANSITION_DAYS: f32 = 6.0;
const REGIMES: u32 = 4;
const REGIME_DAYS: f32 = 7.0;
/// Pendant la mise en place, les sols réagissent plus vite (réserve réduite d'autant) pour
/// atteindre leur équilibre sans simuler des années.
const SOIL_SPIN_UP_SPEED: f32 = 8.0;
/// Taille typique des dépressions et anticyclones (km).
const WEATHER_SYSTEM_KM: f32 = 2_500.0;
/// Rayon de l'effet thermique des continents sur la pression (km : les dépressions thermiques
/// ont la taille d'un continent) et rayon de l'influence de la mer sur l'écart jour-nuit (km).
const CONTINENT_RADIUS_KM: f32 = 800.0;
const MARITIME_RADIUS_KM: f32 = 250.0;
/// Déviation du vent de surface par rapport à la pente de pression (degrés) : il franchit les
/// isobares sous un angle de 10 à 20° sur mer, 25 à 45° sur terre (frottement).
const CROSS_ISOBAR_DEFLECTION: f32 = 84.0;
/// Déplacement des ceintures de pression avec le soleil (part de sa latitude) : la ceinture de
/// pluies équatoriale va de ~5° (océans) à ~15° (continents) au fil de l'année.
const SEASONAL_SHIFT: f32 = 0.45;
/// Vent d'est observé près de l'équateur, là où les alizés des deux hémisphères se rejoignent (m/s).
const EQUATORIAL_EASTERLY: f32 = 4.0;

// --- Atmosphère
/// Vitesse d'un vent de surface « fort » (intensité normalisée 1), en m/s.
const SURFACE_WIND: f32 = 8.0;
/// Vent moyen de la couche basse rapporté au vent près du sol : au-dessus de la couche de
/// frottement (vers 1 à 1,5 km), le vent est 1,5 à 2 fois plus fort, et c'est lui qui porte
/// l'essentiel de la vapeur.
const LOW_LAYER_WIND: f32 = 1.7;
/// Vitesse de référence des vents d'altitude qui portent l'humidité, en m/s.
const UPPER_WIND: f32 = 12.0;
/// Épaisseur de la couche basse (m).
const LOW_DEPTH: f32 = 2_000.0;
/// Altitude du milieu de la couche haute (m) et épaisseur.
const UPPER_Z: f32 = 4_000.0;
const UPPER_DEPTH: f32 = 4_000.0;
/// Décroissance de la température avec l'altitude (°C par m).
const LAPSE: f32 = 0.0065;
/// L'humidité diminue avec la hauteur dans la couche basse : près du sol, l'air est environ
/// 25 % plus humide que la moyenne de la couche.
const NEAR_SURFACE_HUMIDITY: f32 = 1.25;
/// Coefficient d'échange de vapeur avec la surface (formule « bulk ») et densité de l'air.
const BULK_CE: f32 = 1.2e-3;
const AIR_DENSITY: f32 = 1.2;
/// Vitesse verticale typique des grands mouvements d'air (ascendance de la zone équatoriale,
/// subsidence des anticyclones), quelques mm/s à l'échelle de centaines de km.
const LARGE_SCALE_W: f32 = 0.003;
/// Soulèvement moyen par les fronts des perturbations des latitudes moyennes (m/s) : elles
/// défilent tous les quelques jours, trop vite pour les régimes mensuels de la simulation.
const STORM_W: f32 = 0.004;
/// Vitesse verticale de la convection pleinement développée (m/s).
const CONVECTION_W: f32 = 0.004;
/// Effet de lac : l'air qui passe sur un lac s'y charge d'humidité et s'élève au-dessus du lac et
/// sur la rive sous le vent (brise de lac, air réchauffé par l'eau). Vitesse verticale ajoutée sur
/// le lac et sur la rive juste sous le vent (m/s), décroissant jusqu'à `LAKE_EFFECT_KM`. Réglage
/// simplifié, faute de saisons : étouffé comme la convection sous le couvercle des anticyclones.
const LAKE_EFFECT_W: f32 = 0.003;
const LAKE_EFFECT_KM: f32 = 130.0;
/// Saison sèche : pluie de la saison inférieure à cette part de son évaporation potentielle.
pub const DRY_SEASON_RATIO: f32 = 0.3;
/// Été sec (méditerranéen) : pluie de la saison chaude inférieure à cette part de celle de la
/// saison froide.
pub const SUMMER_DRY_SHARE: f32 = 0.4;
/// Part de l'eau condensée dans l'air soulevé qui précipite, dans un air sec puis dans un air
/// saturé : le reste s'évapore en altitude (enclumes des orages). Un air humide perd moins de
/// gouttes en route, d'où un meilleur rendement.
const PRECIPITATION_EFFICIENCY: (f32, f32) = (0.3, 0.7);
/// Temps de chute des gouttes une fois formées (s) : le vent les pousse pendant ce temps, d'où
/// une partie de la pluie qui passe la crête d'un relief.
const FALLOUT_TIME_S: f32 = 2_000.0;
/// Échelle sur laquelle l'air « voit » le relief (km).
const AIRFLOW_SMOOTHING_KM: f32 = 40.0;
/// Les flocons tombent plus lentement (~1 m/s) : le vent les porte plus loin, au-delà des crêtes.
const SNOWFALL_TIME_S: f32 = 5_000.0;
/// Brouillard et nuages bas (air bas sursaturé sans être soulevé) : ils bruinent à peine
/// (temps de chute de quelques jours) et s'évaporent dès que l'air se réchauffe.
const DRIZZLE_TIME_S: f32 = 3.0 * 86_400.0;
/// Temps d'évaporation d'un nuage dans un air d'altitude non saturé (s).
const CLOUD_EVAPORATION_S: f32 = 10_800.0;
/// Brassage turbulent entre les deux couches (part de l'écart d'humidité relative par jour).
const VERTICAL_MIXING_PER_DAY: f32 = 0.06;
/// Mélange horizontal par les tourbillons (m²/s) : faible en général, fort sur le rail des
/// dépressions des latitudes moyennes.
const EDDY_DIFFUSION: (f32, f32) = (3.2e5, 1.3e6);
/// Part maximale de la pluie qui s'évapore en tombant dans un air très sec.
const RAIN_REEVAPORATION: f32 = 0.5;

// --- Sols
/// Réserve d'eau utile d'un sol (mm) : 150 mm dans le modèle de Manabe, de 100 à plus de 250 mm
/// dans les sols réels (davantage sous forêt, aux racines profondes). Au-dessus de 75 %, le sol
/// s'évapore librement.
const SOIL_CAPACITY: f32 = 250.0;
const SOIL_FREE: f32 = 0.75;
/// Rosée maximale (mm par jour).
const DEW_MAX_PER_DAY: f32 = 0.3;

/// Altitude moyenne (m) des niveaux de terre 1 à 6, et des sommets.
pub const LEVEL_ALTITUDE: [f32; 6] = [150.0, 450.0, 850.0, 1_300.0, 1_800.0, 2_400.0];
pub const SUMMIT_ALTITUDE: f32 = 3_800.0;

/// Altitude (m) d'une case de terre selon son niveau.
pub fn altitude(level: u8, mountain: bool) -> f32 {
    if mountain {
        SUMMIT_ALTITUDE
    } else {
        LEVEL_ALTITUDE[(level.clamp(1, 6) - 1) as usize]
    }
}

/// Directions unitaires (x vers l'est, y vers le nord) des 6 voisins, dans l'ordre de `hex::DIRECTIONS`.
const UNIT: [(f32, f32); 6] = [
    (1.0, 0.0),
    (0.5, 0.866_025_4),
    (-0.5, 0.866_025_4),
    (-1.0, 0.0),
    (-0.5, -0.866_025_4),
    (0.5, -0.866_025_4),
];

/// Ce que la simulation sait de chaque case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    Water,
    /// Terre, avec son altitude en mètres.
    Land(f32),
    /// Lac (eau libre à l'intérieur des terres), avec l'altitude de sa surface en mètres : il
    /// évapore comme la mer et humidifie l'air, mais compte comme de la terre pour les courants
    /// marins, la continentalité et le relief vu par l'air.
    Lake(f32),
}

/// Une période simulée. `sun_lat` : latitude (signée, -1 à 1) où le soleil est au zénith ;
/// 0 pour l'année moyenne. Les saisons viendront en enchaînant plusieurs périodes.
#[derive(Clone, Copy, Debug)]
pub struct Period {
    pub sun_lat: f32,
}

impl Period {
    /// Les quatre moments de l'année : équinoxe, solstice (soleil au tropique nord, 23,4°),
    /// équinoxe, solstice (tropique sud). Leur moyenne donne le climat annuel ; elle déplace
    /// la ceinture de pluies équatoriale, qui arrose les savanes une partie de l'année.
    pub const YEAR: [Period; 4] = [
        Period { sun_lat: 0.0 },
        Period { sun_lat: 0.26 },
        Period { sun_lat: 0.0 },
        Period { sun_lat: -0.26 },
    ];
}

/// Résultat de la simulation, une valeur par case (même indexation que la carte). Certaines
/// grandeurs (minimales, maximales, ruissellement…) servent aux diagnostics et aux rivières à venir.
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct Climate {
    /// Température moyenne au sol (°C).
    pub temperature: Vec<f32>,
    /// Moyennes des minimales et maximales journalières (°C).
    pub temp_min: Vec<f32>,
    pub temp_max: Vec<f32>,
    /// Précipitations (mm par an).
    pub rain: Vec<f32>,
    /// Évaporation potentielle (mm par an).
    pub pet: Vec<f32>,
    /// Évaporation réelle (mm par an).
    pub evaporation: Vec<f32>,
    /// Ruissellement : trop-plein des sols (mm par an), qui alimentera les rivières.
    pub runoff: Vec<f32>,
    /// Remplissage moyen de la réserve du sol (0 à 1).
    pub soil: Vec<f32>,
    /// Vent de surface moyen (x vers l'est, y vers le nord), d'intensité 0 à 1 environ.
    pub wind: Vec<(f32, f32)>,
    /// Humidité relative moyenne de l'air près du sol (0 à 1).
    pub humidity: Vec<f32>,
    /// Eau précipitable moyenne de la colonne d'air (mm).
    pub column: Vec<f32>,
    /// Par saison (les 4 moments de `Period::YEAR` : équinoxe, été boréal, équinoxe, hiver
    /// boréal) : température moyenne (°C), précipitations et évaporation potentielle au rythme
    /// de la saison (mm par an). Avec un autre nombre de périodes, les 4 valeurs sont celles de
    /// l'année.
    pub season_temp: Vec<[f32; 4]>,
    pub season_rain: Vec<[f32; 4]>,
    pub season_pet: Vec<[f32; 4]>,
}

impl Climate {
    /// Indice d'aridité (précipitations / évaporation potentielle).
    pub fn aridity(&self, i: usize) -> Option<f32> {
        Some(self.rain.get(i)? / self.pet.get(i)?.max(1.0))
    }

    /// Températures de la saison la plus froide et de la plus chaude (°C).
    pub fn coldest_warmest(&self, i: usize) -> Option<(f32, f32)> {
        let t = self.season_temp.get(i)?;
        Some((t.iter().copied().fold(f32::MAX, f32::min), t.iter().copied().fold(f32::MIN, f32::max)))
    }

    /// Saisons sèches : précipitations de la saison inférieures à `DRY_SEASON_RATIO` fois son
    /// évaporation potentielle.
    pub fn dry_seasons(&self, i: usize) -> Option<[bool; 4]> {
        let (r, p) = (self.season_rain.get(i)?, self.season_pet.get(i)?);
        Some(std::array::from_fn(|s| r[s] < DRY_SEASON_RATIO * p[s].max(1.0)))
    }

    /// Été sec et hiver pluvieux (climat méditerranéen) : la saison la plus chaude est sèche et
    /// reçoit moins de `SUMMER_DRY_SHARE` fois la pluie de la saison la plus froide.
    pub fn dry_summer(&self, i: usize) -> Option<bool> {
        let (t, r) = (self.season_temp.get(i)?, self.season_rain.get(i)?);
        let warm = (0..4).max_by(|&a, &b| t[a].total_cmp(&t[b]))?;
        let cold = (0..4).min_by(|&a, &b| t[a].total_cmp(&t[b]))?;
        Some(warm != cold && self.dry_seasons(i)?[warm] && r[warm] < SUMMER_DRY_SHARE * r[cold])
    }
}

/// Échelle et position de la carte sur sa planète : taille d'une case et latitude du centre.
/// Les lignes vont du nord (première) au sud (dernière).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    /// Distance entre les centres de deux cases voisines (km).
    pub cell_km: f32,
    /// Latitude du centre de la carte (degrés, positive au nord).
    pub center_lat: f32,
    /// Écart de latitude entre deux lignes (degrés).
    pub degrees_per_row: f32,
    /// La carte fait le tour de la planète : l'est rejoint l'ouest.
    pub wraps: bool,
}

/// Taille d'une case du jeu (km) : une ville tient sur une case, son territoire sur quelques-unes.
pub const CELL_KM: f32 = 40.0;
/// Latitudes possibles du centre de la carte (degrés nord).
pub const LATITUDE_RANGE: (f32, f32) = (25.0, 55.0);

impl Region {
    /// Région de jeu : cases de `CELL_KM`, latitude tirée de la graine.
    pub fn for_seed(seed: u32) -> Self {
        let mut x = seed.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        let t = (x % 10_000) as f32 / 10_000.0;
        Region::local(CELL_KM, LATITUDE_RANGE.0 + (LATITUDE_RANGE.1 - LATITUDE_RANGE.0) * t)
    }

    /// Une région d'une planète de la taille de la Terre, centrée sur une latitude.
    pub fn local(cell_km: f32, center_lat: f32) -> Self {
        Region { cell_km, center_lat, degrees_per_row: cell_km * 0.866_025_4 / KM_PER_DEGREE, wraps: false }
    }

    /// Toute une planète, d'un pôle à l'autre sur `height` lignes, avec des cases de `cell_km` :
    /// sa taille découle de celle de la carte (plus petite que la Terre). Elle fait le tour complet.
    pub fn planet(cell_km: f32, height: i32) -> Self {
        Region { cell_km, center_lat: 0.0, degrees_per_row: 180.0 / (height - 1).max(1) as f32, wraps: true }
    }

    /// Latitude d'une ligne (degrés).
    pub fn latitude_deg(&self, row: i32, height: i32) -> f32 {
        (self.center_lat + ((height - 1) as f32 / 2.0 - row as f32) * self.degrees_per_row).clamp(-90.0, 90.0)
    }
}

/// Latitude signée d'une ligne : +1 au pôle nord, 0 à l'équateur, -1 au pôle sud.
pub fn latitude(region: &Region, row: i32, height: i32) -> f32 {
    region.latitude_deg(row, height) / 90.0
}

/// Température moyenne annuelle au niveau de la mer selon la latitude (signée, -1 à 1) en °C,
/// moyennes zonales terrestres arrondies.
pub fn sea_level_temperature(lat: f32) -> f32 {
    const TABLE: [(f32, f32); 10] = [
        (0.0, 27.0),
        (10.0, 26.5),
        (20.0, 24.5),
        (30.0, 20.0),
        (40.0, 13.5),
        (50.0, 6.0),
        (60.0, -1.5),
        (70.0, -10.0),
        (80.0, -18.0),
        (90.0, -24.0),
    ];
    let deg = (lat.abs() * 90.0).min(90.0);
    let k = TABLE.iter().position(|e| e.0 >= deg).unwrap_or(TABLE.len() - 1).max(1);
    let ((a, ta), (b, tb)) = (TABLE[k - 1], TABLE[k]);
    ta + (tb - ta) * (deg - a) / (b - a)
}

/// Rayonnement solaire moyen au sommet de l'atmosphère (W/m²) selon la latitude.
fn insolation(lat: f32) -> f32 {
    let s = (lat * std::f32::consts::FRAC_PI_2).sin();
    340.25 * (1.0 - 0.482 * (3.0 * s * s - 1.0) / 2.0)
}

/// Évaporation potentielle de Hargreaves (mm/jour).
fn hargreaves(lat: f32, t_mean: f32, diurnal_range: f32) -> f32 {
    let ra = insolation(lat) * 0.0864 / 2.45; // rayonnement en mm d'eau évaporable par jour
    (0.0023 * ra * (t_mean + 17.8).max(0.0) * diurnal_range.max(0.0).sqrt()).max(0.0)
}

/// Pression (hPa) à une altitude (m).
fn pressure(z: f32) -> f32 {
    1013.25 * (-z / 8_000.0).exp()
}

/// Humidité spécifique de saturation (g/kg) à une température (°C) et une altitude (m).
fn saturation(t: f32, z: f32) -> f32 {
    let es = 6.112 * (17.67 * t / (t + 243.5)).exp();
    622.0 * es / (pressure(z) - 0.378 * es).max(1.0)
}

/// Eau (mm) contenue par g/kg de vapeur dans une couche d'épaisseur `depth` centrée à `z`.
fn layer_mass(z: f32, depth: f32) -> f32 {
    AIR_DENSITY * (-z / 8_500.0).exp() * depth / 1_000.0
}

/// Pression des grandes ceintures : basse à l'équateur et vers 60°, haute vers 30° et aux pôles.
fn band_pressure(lat: f32) -> f32 {
    -(3.0 * std::f32::consts::PI * lat.abs()).cos()
}

/// Bosse douce valant 1 entre `a` et `b`, 0 au-delà de la marge `m`.
fn bump(x: f32, a: f32, b: f32, m: f32) -> f32 {
    let rise = ((x - (a - m)) / m).clamp(0.0, 1.0);
    let fall = (((b + m) - x) / m).clamp(0.0, 1.0);
    rise.min(fall)
}

/// Grille de travail : voisins précalculés, échelle et pas de temps.
struct Grid {
    /// Pour chaque case, ses voisins dans la carte (direction, indice).
    neighbors: Vec<Vec<(usize, usize)>>,
    hexes: Vec<Hex>,
    width: i32,
    height: i32,
    wraps: bool,
    lat: Vec<f32>,
    /// Position (km).
    pos: Vec<(f32, f32)>,
    /// Distance entre deux centres voisins (m).
    dx: f32,
    cell_km: f32,
    /// Pas de temps (s et jours).
    dt: f32,
    dt_days: f32,
}

impl Grid {
    fn new(width: i32, height: i32, region: &Region) -> Self {
        let n = (width * height) as usize;
        let wraps = region.wraps;
        let index = |h: Hex| {
            let (c, r) = h.to_offset();
            let c = if wraps { c.rem_euclid(width) } else { c };
            (c >= 0 && c < width && r >= 0 && r < height).then(|| (r * width + c) as usize)
        };
        let mut neighbors = Vec::with_capacity(n);
        let mut lat = Vec::with_capacity(n);
        let mut pos = Vec::with_capacity(n);
        let mut hexes = Vec::with_capacity(n);
        let step = crate::hex::HEX_SIZE * 3f32.sqrt();
        for row in 0..height {
            for col in 0..width {
                let h = Hex::from_offset(col, row);
                neighbors.push(
                    h.neighbors().iter().enumerate().filter_map(|(d, &nb)| index(nb).map(|i| (d, i))).collect(),
                );
                hexes.push(h);
                lat.push(latitude(region, row, height));
                let (x, y) = h.to_world();
                pos.push((x / step * region.cell_km, y / step * region.cell_km));
            }
        }
        let dx = region.cell_km * 1_000.0;
        // L'air de la couche basse ne doit pas franchir plus d'une case par pas
        let dt = MAX_DT_S.min(0.9 * dx / (SURFACE_WIND * LOW_LAYER_WIND));
        Grid { neighbors, hexes, width, height, wraps, lat, pos, dx, cell_km: region.cell_km, dt, dt_days: dt / 86_400.0 }
    }

    /// Indice d'une case de la carte (en faisant le tour si la carte boucle).
    fn index(&self, h: Hex) -> Option<usize> {
        let (c, r) = h.to_offset();
        let c = if self.wraps { c.rem_euclid(self.width) } else { c };
        (c >= 0 && c < self.width && r >= 0 && r < self.height).then(|| (r * self.width + c) as usize)
    }

    /// Valeur d'un bruit à la position d'une case ; sur une planète, le bruit est pris sur un
    /// cylindre pour se raccorder entre l'est et l'ouest.
    fn noise(&self, noise: &Fbm<Perlin>, i: usize, wavelength_km: f32) -> f32 {
        let (x, y) = self.pos[i];
        let f = 1.0 / wavelength_km as f64;
        if self.wraps {
            let circumference = self.width as f64 * self.cell_km as f64;
            let radius = circumference / std::f64::consts::TAU;
            let angle = x as f64 / circumference * std::f64::consts::TAU;
            noise.get([radius * angle.cos() * f, radius * angle.sin() * f, y as f64 * f]) as f32
        } else {
            noise.get([x as f64 * f, y as f64 * f]) as f32
        }
    }

    /// Nombre de cases correspondant à une distance (au moins `min`).
    fn cells(&self, km: f32, min: i32) -> i32 {
        ((km / self.cell_km).round() as i32).max(min)
    }

    /// Gradient d'un champ scalaire, par pas de voisinage.
    fn gradient(&self, f: &[f32], i: usize) -> (f32, f32) {
        let (mut gx, mut gy) = (0.0, 0.0);
        for &(d, j) in &self.neighbors[i] {
            let diff = f[j] - f[i];
            gx += diff * UNIT[d].0;
            gy += diff * UNIT[d].1;
        }
        (gx / 3.0, gy / 3.0)
    }

    /// Transport pendant un pas (vent en m/s) : voisins sous le vent avec la part de l'air envoyée
    /// à chacun, part totale qui quitte la case (y compris hors de la carte), et part de l'air qui
    /// entre depuis l'extérieur de la carte.
    fn outflow(&self, wind: &[(f32, f32)], i: usize) -> Flow {
        let (wx, wy) = wind[i];
        let speed = wx.hypot(wy);
        if speed < 1e-3 {
            return Flow::default();
        }
        let dot = |d: usize| (wx * UNIT[d].0 + wy * UNIT[d].1) / speed;
        let weight = |d: usize| dot(d).max(0.0).powi(2);
        let total: f32 = (0..6).map(weight).sum::<f32>().max(1e-6);
        let share = (speed * self.dt / self.dx).min(0.9);
        let to = self.neighbors[i]
            .iter()
            .map(|&(d, j)| (j, share * weight(d) / total))
            .filter(|&(_, f)| f > 0.0)
            .collect();
        // Au vent d'un bord de carte : l'air arrive de l'extérieur
        let inside = |d: usize| self.neighbors[i].iter().any(|&(e, _)| e == d);
        let upwind_outside: f32 = (0..6).filter(|&d| !inside(d)).map(|d| (-dot(d)).max(0.0).powi(2)).sum();
        Flow { to, leaving: share, entering: (share * upwind_outside / total).min(0.9) }
    }

    /// Coefficient du mélange horizontal par les tourbillons (fort sur le rail des dépressions).
    fn mixing(&self, i: usize) -> f32 {
        let diffusion = EDDY_DIFFUSION.0 + (EDDY_DIFFUSION.1 - EDDY_DIFFUSION.0) * bump(self.lat[i].abs(), 0.42, 0.68, 0.12);
        (diffusion * self.dt / (self.dx * self.dx)).min(0.45)
    }
}

/// Liste, pour chaque case, des cases qui lui envoient une part de leur contenu (format compact,
/// pour calculer chaque case indépendamment, en parallèle).
#[derive(Default)]
struct Gather {
    start: Vec<u32>,
    source: Vec<u32>,
    share: Vec<f32>,
}

impl Gather {
    /// À partir d'envois « case → voisines » (`sends[j]` : liste de (destination, part)).
    fn from_sends(n: usize, sends: impl Fn(usize) -> Vec<(usize, f32)>) -> Self {
        let mut incoming: Vec<Vec<(u32, f32)>> = vec![Vec::new(); n];
        for j in 0..n {
            for (i, f) in sends(j) {
                incoming[i].push((j as u32, f));
            }
        }
        let mut g = Gather { start: Vec::with_capacity(n + 1), ..default_gather() };
        g.start.push(0);
        for list in incoming {
            for (j, f) in list {
                g.source.push(j);
                g.share.push(f);
            }
            g.start.push(g.source.len() as u32);
        }
        g
    }

    /// Somme de ce que la case `i` reçoit.
    fn received(&self, i: usize, field: &[f32]) -> f32 {
        let (a, b) = (self.start[i] as usize, self.start[i + 1] as usize);
        (a..b).map(|k| field[self.source[k] as usize] * self.share[k]).sum()
    }
}

fn default_gather() -> Gather {
    Gather { start: Vec::new(), source: Vec::new(), share: Vec::new() }
}

/// Transport d'une quantité par le vent : chaque case garde une part, reçoit de ses voisines au
/// vent et, au bord de la carte, de l'air extérieur (`outside`).
fn advect(field: &mut Vec<f32>, flows: &Flows, outside: Option<&[f32]>, next: &mut Vec<f32>) {
    let old: &[f32] = field;
    next.par_iter_mut().enumerate().for_each(|(i, v)| {
        let from_outside = outside.map_or(0.0, |o| o[i] * flows.entering[i]);
        *v = old[i] * (1.0 - flows.leaving[i]) + from_outside + flows.gather.received(i, old);
    });
    std::mem::swap(field, next);
}

/// Mélange horizontal (`k` : coefficient de chaque case).
fn mix(field: &mut Vec<f32>, grid: &Grid, k: &[f32], next: &mut Vec<f32>) {
    let old: &[f32] = field;
    next.par_iter_mut().enumerate().for_each(|(i, v)| {
        let nb = &grid.neighbors[i];
        let mean = nb.iter().map(|&(_, j)| old[j]).sum::<f32>() / nb.len().max(1) as f32;
        *v = old[i] + k[i] * (mean - old[i]);
    });
    std::mem::swap(field, next);
}

/// Transport d'une couche pendant un pas, sous forme « collecte ».
struct Flows {
    gather: Gather,
    leaving: Vec<f32>,
    entering: Vec<f32>,
}

impl Flows {
    fn new(flows: &[Flow]) -> Self {
        Flows {
            gather: Gather::from_sends(flows.len(), |j| flows[j].to.clone()),
            leaving: flows.iter().map(|f| f.leaving).collect(),
            entering: flows.iter().map(|f| f.entering).collect(),
        }
    }
}

/// Transport d'une case pendant un pas.
#[derive(Default)]
struct Flow {
    to: Vec<(usize, f32)>,
    leaving: f32,
    entering: f32,
}

/// Vents d'un régime météo : surface et altitude (m/s), et mouvements verticaux (m/s).
struct Regime {
    surface: Vec<(f32, f32)>,
    /// Vitesse du vent près du sol (m/s, au moins 2).
    speed: Vec<f32>,
    surface_out: Flows,
    upper_out: Flows,
    /// Redistribution de l'air bas chassé par la subsidence vers les cases sous le vent.
    pushed: Gather,
    /// La case garde l'air chassé (aucun voisin sous le vent).
    keeps_pushed: Vec<bool>,
    /// Vitesse verticale due à la convergence (> 0 : l'air monte) ou à la divergence (< 0).
    w_large: Vec<f32>,
    /// Vitesse verticale imposée par le relief (air poussé contre une pente).
    w_relief: Vec<f32>,
    /// Descente de fond (anticyclones subtropicaux), qui forme un couvercle d'inversion (m/s).
    w_subsidence: Vec<f32>,
}

fn regime(grid: &Grid, period: Period, seed: u32, k: u32, land_heat: &[f32], ground: &[f32]) -> Regime {
    let n = grid.lat.len();
    let weather: Fbm<Perlin> =
        Fbm::new(seed.wrapping_add(31_337).wrapping_add(k * 7_919)).set_octaves(2).set_frequency(1.0);
    let systems: Vec<f32> = (0..n).map(|i| grid.noise(&weather, i, WEATHER_SYSTEM_KM)).collect();
    // Ceintures (décalées vers le soleil), continents, dépressions et anticyclones mobiles
    let pressure: Vec<f32> = (0..n)
        .map(|i| band_pressure(grid.lat[i] - period.sun_lat * SEASONAL_SHIFT) + land_heat[i] + systems[i] * 1.1)
        .collect();
    let deflect = |lat: f32, max_deg: f32| -lat.signum() * max_deg.to_radians() * (lat.abs() / 0.12).min(1.0);
    let rotate = |(x, y): (f32, f32), a: f32| {
        let (s, c) = a.sin_cos();
        (x * c - y * s, x * s + y * c)
    };
    // Surface : du haut vers le bas de la pression, dévié d'environ 60° (frottement)
    let mut surface: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let (gx, gy) = grid.gradient(&pressure, i);
            rotate((-gx, -gy), deflect(grid.lat[i], CROSS_ISOBAR_DEFLECTION))
        })
        .collect();
    let mut speeds: Vec<f32> = surface.iter().map(|w| w.0.hypot(w.1)).collect();
    speeds.sort_by(|a, b| a.total_cmp(b));
    let strong = speeds[(n - 1) * 9 / 10].max(1e-6);
    let thermal_equator = period.sun_lat * SEASONAL_SHIFT;
    for (i, w) in surface.iter_mut().enumerate() {
        let easterly = EQUATORIAL_EASTERLY * bump((grid.lat[i] - thermal_equator).abs(), 0.0, 0.12, 0.1);
        *w = (w.0 / strong * SURFACE_WIND - easterly, w.1 / strong * SURFACE_WIND);
    }
    // Altitude : vents d'ouest (maximum vers 35-40°), retour vers les pôles au-dessus des
    // alizés, et perturbations qui tournent autour des dépressions et anticyclones (90°)
    let upper: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let lat = grid.lat[i];
            let westerly = (std::f32::consts::PI * (lat.abs() / 0.8).min(1.0)).sin();
            let poleward = lat.signum() * 0.35 * bump(lat.abs(), 0.05, 0.3, 0.05);
            // Gradient du champ des perturbations rapporté à 1 000 km
            let (gx, gy) = grid.gradient(&systems, i);
            let per_1000_km = 1_000.0 / grid.cell_km * 1.5;
            let (ex, ey) = rotate((-gx * per_1000_km, -gy * per_1000_km), deflect(lat, 90.0));
            ((westerly + ex) * UPPER_WIND, (poleward + ey) * UPPER_WIND)
        })
        .collect();
    let w_relief: Vec<f32> = (0..n)
        .map(|i| {
            let (gx, gy) = grid.gradient(ground, i);
            let (wx, wy) = surface[i];
            ((gx * wx + gy * wy) / grid.dx).max(0.0)
        })
        .collect();
    let low_layer: Vec<(f32, f32)> = surface.iter().map(|w| (w.0 * LOW_LAYER_WIND, w.1 * LOW_LAYER_WIND)).collect();
    let surface_out: Vec<Flow> = (0..n).map(|i| grid.outflow(&low_layer, i)).collect();
    let upper_out: Vec<Flow> = (0..n).map(|i| grid.outflow(&upper, i)).collect();
    // Vitesse verticale cohérente avec le transport : l'air de la couche basse qui converge sur
    // une case est l'air qui monte, celui qui diverge est remplacé par de l'air qui descend
    let mut converging = vec![0.0f32; n];
    for (i, flow) in surface_out.iter().enumerate() {
        converging[i] += flow.entering - flow.leaving;
        for &(j, f) in &flow.to {
            converging[j] += f;
        }
    }
    // S'y ajoute la circulation de fond imposée par la latitude (cellules de Hadley et de
    // Ferrel) : l'air descend sous les anticyclones subtropicaux et monte à l'équateur et vers
    // 60°. Sur une carte régionale, elle ne peut pas naître de la convergence des vents de la carte.
    let shift = period.sun_lat * SEASONAL_SHIFT;
    let background: Vec<f32> = (0..n).map(|i| -LARGE_SCALE_W * band_pressure(grid.lat[i] - shift)).collect();
    let w_large: Vec<f32> = (0..n).map(|i| background[i] + converging[i] * LOW_DEPTH / grid.dt).collect();
    let w_subsidence: Vec<f32> = background.iter().map(|&w| (-w).max(0.0)).collect();
    let pushed = Gather::from_sends(n, |j| {
        let total: f32 = surface_out[j].to.iter().map(|&(_, f)| f).sum();
        if total > 0.0 { surface_out[j].to.iter().map(|&(i, f)| (i, f / total)).collect() } else { Vec::new() }
    });
    let keeps_pushed = surface_out.iter().map(|f| f.to.is_empty()).collect();
    let speed = surface.iter().map(|w| w.0.hypot(w.1).max(2.0)).collect();
    Regime {
        surface,
        speed,
        surface_out: Flows::new(&surface_out),
        upper_out: Flows::new(&upper_out),
        pushed,
        keeps_pushed,
        w_large,
        w_relief,
        w_subsidence,
    }
}

/// Simule le climat d'une carte de `width` × `height` cases (indexation offset, ligne par ligne)
/// sur une année (quatre périodes). `sea_anomaly` : écart de la température de la mer à celle de
/// sa latitude dû aux courants (°C, voir `currents.rs`), vide pour une mer sans courants.
pub fn simulate(width: i32, height: i32, surface: &[Surface], seed: u32, region: &Region, sea_anomaly: &[f32]) -> Climate {
    simulate_periods(width, height, surface, seed, region, sea_anomaly, &Period::YEAR)
}

/// Simule le climat en enchaînant des périodes ; les sols gardent leur eau d'une période à l'autre.
pub fn simulate_periods(
    width: i32,
    height: i32,
    surface: &[Surface],
    seed: u32,
    region: &Region,
    sea_anomaly: &[f32],
    periods: &[Period],
) -> Climate {
    let grid = Grid::new(width, height, region);
    let (dt, dt_days) = (grid.dt, grid.dt_days);
    let steps = |days: f32| (days / dt_days).ceil() as usize;
    let n = grid.lat.len();
    assert_eq!(surface.len(), n);
    let is_land = |i: usize| matches!(surface[i], Surface::Land(_) | Surface::Lake(_));
    // Sol (réservoir d'eau, ruissellement) : la terre ; la mer et les lacs évaporent librement
    let has_soil = |i: usize| matches!(surface[i], Surface::Land(_));
    // Altitude réelle du sol, et relief « vu par l'air » : l'écoulement épouse un relief lissé sur
    // quelques dizaines de km et contourne en partie les sommets isolés
    let elevation: Vec<f32> = surface
        .iter()
        .map(|s| match s {
            Surface::Water => 0.0,
            Surface::Land(z) | Surface::Lake(z) => *z,
        })
        .collect();
    let smoothing = grid.cells(AIRFLOW_SMOOTHING_KM, 0);
    let ground: Vec<f32> = (0..n)
        .map(|i| {
            if !is_land(i) || smoothing == 0 {
                return elevation[i];
            }
            let around: Vec<f32> = grid.hexes[i].range(smoothing).filter_map(|h| grid.index(h)).map(|j| elevation[j]).collect();
            (around.iter().sum::<f32>() + elevation[i]) / (around.len() + 1) as f32
        })
        .collect();

    // Courants marins (température de la mer) et influence maritime des côtes
    let sea_anomaly: Vec<f32> = (0..n).map(|i| if is_land(i) { 0.0 } else { sea_anomaly.get(i).copied().unwrap_or(0.0) }).collect();
    let coast_anomaly: Vec<f32> = (0..n)
        .map(|i| {
            if !is_land(i) {
                return sea_anomaly[i];
            }
            let near: Vec<f32> =
                grid.neighbors[i].iter().filter(|&&(_, j)| !is_land(j)).map(|&(_, j)| sea_anomaly[j]).collect();
            if near.is_empty() { 0.0 } else { 0.7 * near.iter().sum::<f32>() / near.len() as f32 }
        })
        .collect();
    // Part de terre alentour : continentalité (effet sur la pression, et sur l'écart jour-nuit)
    let share_within = |km: f32| -> Vec<f32> {
        let radius = grid.cells(km, 2);
        (0..n)
            .map(|i| {
                let (mut land, mut all) = (0usize, 0usize);
                for j in grid.hexes[i].range(radius).filter_map(|h| grid.index(h)) {
                    all += 1;
                    land += usize::from(is_land(j));
                }
                land as f32 / all.max(1) as f32
            })
            .collect()
    };
    let continental = share_within(CONTINENT_RADIUS_KM);
    let land_share = share_within(MARITIME_RADIUS_KM);

    let mixing: Vec<f32> = (0..n).map(|i| grid.mixing(i)).collect();
    // Effet de lac : pour chaque case proche d'un lac, distance et direction depuis le lac le plus
    // proche
    let lake_effect: Vec<(f32, f32, f32)> = {
        let reach = grid.cells(LAKE_EFFECT_KM, 1);
        let mut best: Vec<Option<(i32, usize)>> = vec![None; n];
        for k in (0..n).filter(|&k| matches!(surface[k], Surface::Lake(_))) {
            for h in grid.hexes[k].range(reach) {
                if let Some(j) = grid.index(h) {
                    let d = grid.hexes[k].distance(h);
                    if best[j].is_none_or(|(bd, _)| d < bd) {
                        best[j] = Some((d, k));
                    }
                }
            }
        }
        (0..n)
            .map(|i| match best[i] {
                None => (0.0, 0.0, 0.0),
                Some(_) if matches!(surface[i], Surface::Lake(_)) => (0.5, 0.0, 0.0),
                Some(_) if !is_land(i) => (0.0, 0.0, 0.0),
                Some((d, k)) => {
                    let ((x0, y0), (x1, y1)) = (grid.hexes[k].to_world(), grid.hexes[i].to_world());
                    let (dx, dy) = (x1 - x0, y1 - y0);
                    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                    let f = 1.0 - (d - 1) as f32 / reach.max(1) as f32;
                    (f.clamp(0.0, 1.0), dx / len, dy / len)
                }
            })
            .collect()
    };
    // État de chaque case, conservé d'une période à l'autre
    let mut cells: Vec<CellState> = (0..n)
        .map(|i| CellState {
            soil: if has_soil(i) { SOIL_CAPACITY / SOIL_SPIN_UP_SPEED * 0.5 } else { 0.0 },
            rh_mean: 0.6,
            ..CellState::default()
        })
        .collect();
    let mut sums: Vec<Sums> = vec![Sums::default(); n];
    let mut samples = 0.0f32;
    // Vapeur des deux couches, eau nuageuse et brouillard (mm), transportés par le vent
    let mut low: Vec<f32> = Vec::new();
    let mut up: Vec<f32> = Vec::new();
    let mut cloud = vec![0.0f32; n];
    let mut fog = vec![0.0f32; n];
    let mut scratch = vec![0.0f32; n];
    let mut previous_qs: (Vec<f32>, Vec<f32>) = (Vec::new(), Vec::new());

    // Sommes au début de chaque période : les valeurs par saison sont les écarts
    let mut season_sums: Vec<Vec<(f32, f32, f32)>> = Vec::with_capacity(periods.len());
    let mut season_samples: Vec<f32> = Vec::with_capacity(periods.len());
    for (p, &period) in periods.iter().enumerate() {
        let before: Vec<(f32, f32, f32)> = sums.iter().map(|s| (s.temperature, s.rain, s.pet)).collect();
        let samples_before = samples;
        // Constantes de la période pour chaque case
        let consts: Vec<CellConst> = (0..n)
            .map(|i| {
                let t0 = sea_level_temperature(grid.lat[i] - period.sun_lat * 0.4);
                let t_surface = t0 - LAPSE * ground[i] + coast_anomaly[i];
                let z_upper = UPPER_Z.max(ground[i] + LOW_DEPTH + 1_000.0);
                let t_upper = t0 - LAPSE * z_upper;
                let m_low = layer_mass(ground[i] + LOW_DEPTH / 2.0, LOW_DEPTH);
                let m_up = layer_mass(z_upper, UPPER_DEPTH);
                // Température au sol réel (sommet plus froid que le relief lissé)
                let t_ground = t_surface - LAPSE * (elevation[i] - ground[i]);
                CellConst {
                    land: has_soil(i),
                    lake_effect: lake_effect[i],
                    t_surface,
                    t_ground,
                    m_low,
                    m_up,
                    cap_low: saturation(t_surface, ground[i]) * m_low,
                    cap_up: saturation(t_upper, z_upper) * m_up,
                    qs_low: saturation(t_surface, ground[i]),
                    // Sommet de la couche basse (montée limitée par un couvercle d'inversion)
                    qs_low_top: saturation(t_surface - LAPSE * LOW_DEPTH, ground[i] + LOW_DEPTH),
                    // Sommet de la couche haute : l'air soulevé par les perturbations y monte et y sature
                    qs_top: saturation(t_upper - LAPSE * UPPER_DEPTH / 2.0, z_upper + UPPER_DEPTH / 2.0),
                    // Neige sur les reliefs froids
                    snow: t_upper + LAPSE * (z_upper - ground[i] - LOW_DEPTH) < 0.0 && t_surface < 2.0,
                    // Évaporation potentielle de Hargreaves, sans le facteur de l'écart jour-nuit
                    pet_base: hargreaves(grid.lat[i] - period.sun_lat, t_ground, 1.0),
                    storms: STORM_W * bump((grid.lat[i] - period.sun_lat * 0.5).abs(), 0.42, 0.68, 0.12),
                    land_share: land_share[i],
                    ground: ground[i],
                }
            })
            .collect();
        // Effet thermique des continents sur la pression : terre chaude = basse pression
        // (température ramenée au niveau de la mer : un massif froid n'est pas un anticyclone)
        let land_heat: Vec<f32> =
            (0..n).map(|i| -0.035 * (consts[i].t_surface + LAPSE * ground[i] - 12.0) * continental[i]).collect();
        // Air qui entre par les bords : air marin (la carte est bordée d'océan)
        let outside_low: Vec<f32> = consts.iter().map(|c| 0.75 * c.cap_low).collect();
        let outside_up: Vec<f32> = consts.iter().map(|c| 0.35 * c.cap_up).collect();

        if p == 0 {
            low = (0..n).map(|i| consts[i].cap_low * if is_land(i) { 0.4 } else { 0.75 }).collect();
            up = consts.iter().map(|c| c.cap_up * 0.3).collect();
        } else {
            // Le passage d'une saison à l'autre prend des semaines, pendant lesquelles l'air se
            // renouvelle : on garde l'humidité relative plutôt que de condenser d'un coup l'eau
            // d'un air devenu trop chargé pour sa nouvelle température
            for i in 0..n {
                low[i] = low[i].min(low[i] * consts[i].cap_low / previous_qs.0[i].max(1e-6));
                up[i] = up[i].min(up[i] * consts[i].cap_up / previous_qs.1[i].max(1e-6));
            }
        }
        previous_qs = (consts.iter().map(|c| c.cap_low).collect(), consts.iter().map(|c| c.cap_up).collect());

        let regimes: Vec<Regime> = (0..REGIMES)
            .into_par_iter()
            .map(|k| regime(&grid, period, seed.wrapping_add(p as u32 * 101), k, &land_heat, &ground))
            .collect();
        let warm_up = steps(if p == 0 { SPIN_UP_DAYS } else { TRANSITION_DAYS });
        let per_regime = steps(REGIME_DAYS);
        let total = warm_up + REGIMES as usize * per_regime;
        let rates = Rates {
            dt,
            dt_days,
            fallout: 1.0 - (-dt / FALLOUT_TIME_S).exp(),
            snowfall: 1.0 - (-dt / SNOWFALL_TIME_S).exp(),
            cloud_evaporation: 1.0 - (-dt / CLOUD_EVAPORATION_S).exp(),
            drizzle: 1.0 - (-dt / DRIZZLE_TIME_S).exp(),
            rh_memory: (dt_days / 25.0).min(1.0),
        };
        // Le point de rosée nocturne change lentement : recalculé toutes les quelques heures
        let dew_every = steps(0.25).max(1);
        for step in 0..total {
            let regime_index = if step < warm_up { (step / per_regime.max(1)) % REGIMES as usize } else { (step - warm_up) / per_regime };
            let r = &regimes[regime_index.min(REGIMES as usize - 1)];
            let record = step >= warm_up;
            // Sols plus réactifs pendant la mise en place, puis vraie réserve (même remplissage)
            let soil_capacity = if p == 0 && !record { SOIL_CAPACITY / SOIL_SPIN_UP_SPEED } else { SOIL_CAPACITY };
            if p == 0 && step == warm_up {
                cells.par_iter_mut().for_each(|c| c.soil *= SOIL_SPIN_UP_SPEED);
            }
            let refresh_dew = step % dew_every == 0;

            // 1) Transport horizontal et mélange
            advect(&mut low, &r.surface_out, Some(&outside_low), &mut scratch);
            advect(&mut up, &r.upper_out, Some(&outside_up), &mut scratch);
            advect(&mut cloud, &r.upper_out, None, &mut scratch);
            advect(&mut fog, &r.surface_out, None, &mut scratch);
            mix(&mut low, &grid, &mixing, &mut scratch);
            mix(&mut up, &grid, &mixing, &mut scratch);

            // 2) à 4) Processus verticaux et surface, chaque case indépendamment
            (low.par_iter_mut(), up.par_iter_mut(), cloud.par_iter_mut(), fog.par_iter_mut(), cells.par_iter_mut(), sums.par_iter_mut())
                .into_par_iter()
                .enumerate()
                .for_each(|(i, (low, up, cloud, fog, cell, sum))| {
                    let mut air = Air { low: *low, up: *up, cloud: *cloud, fog: *fog };
                    column_step(&mut air, cell, &consts[i], r, i, &rates, soil_capacity, refresh_dew, record.then_some(sum));
                    (*low, *up, *cloud, *fog) = (air.low, air.up, air.cloud, air.fog);
                });

            // L'air bas chassé par la subsidence s'écoule vers les cases sous le vent
            scratch.par_iter_mut().zip(cells.par_iter()).for_each(|(v, c)| *v = c.pushed);
            let pushed: &[f32] = &scratch;
            low.par_iter_mut().enumerate().for_each(|(i, v)| {
                *v += r.pushed.received(i, pushed) + if r.keeps_pushed[i] { pushed[i] } else { 0.0 };
            });
            cells.par_iter_mut().for_each(|c| c.pushed = 0.0);
            if record {
                samples += 1.0;
            }
        }
        season_sums.push(sums.iter().zip(&before).map(|(s, b)| (s.temperature - b.0, s.rain - b.1, s.pet - b.2)).collect());
        season_samples.push(samples - samples_before);
    }
    // Moyennes ; quantités d'eau ramenées à l'année
    let per_year = 365.0 / (samples * dt_days);
    let mean = |f: fn(&Sums) -> f32| sums.iter().map(|s| f(s) / samples).collect::<Vec<f32>>();
    let yearly = |f: fn(&Sums) -> f32| sums.iter().map(|s| f(s) * per_year).collect::<Vec<f32>>();
    // Valeurs par saison (champ 0 : température moyenne, 1 : pluie, 2 : ETP, au rythme annuel)
    let seasonal = |field: usize, annual: &dyn Fn(&Sums) -> f32| -> Vec<[f32; 4]> {
        (0..sums.len())
            .map(|i| {
                if season_sums.len() != 4 {
                    return [annual(&sums[i]); 4];
                }
                std::array::from_fn(|p| {
                    let (t, r, e) = season_sums[p][i];
                    let k = season_samples[p].max(1.0);
                    match field {
                        0 => t / k,
                        1 => r * 365.0 / (k * dt_days),
                        _ => e * 365.0 / (k * dt_days),
                    }
                })
            })
            .collect()
    };
    Climate {
        temperature: mean(|s| s.temperature),
        temp_min: mean(|s| s.temp_min),
        temp_max: mean(|s| s.temp_max),
        rain: yearly(|s| s.rain),
        pet: yearly(|s| s.pet),
        evaporation: yearly(|s| s.evaporation),
        runoff: yearly(|s| s.runoff),
        soil: mean(|s| s.soil),
        wind: sums.iter().map(|s| (s.wind.0 / samples, s.wind.1 / samples)).collect(),
        humidity: mean(|s| s.humidity),
        column: mean(|s| s.column),
        season_temp: seasonal(0, &|s: &Sums| s.temperature / samples),
        season_rain: seasonal(1, &|s: &Sums| s.rain * per_year),
        season_pet: seasonal(2, &|s: &Sums| s.pet * per_year),
    }
}

/// Grandeurs fixes d'une case pendant une période.
#[derive(Clone, Copy, Debug, Default)]
struct CellConst {
    land: bool,
    /// Effet de lac : intensité (0 à 1) et direction (unitaire) du lac le plus proche vers la case ;
    /// direction nulle sur le lac lui-même (effet quel que soit le vent).
    lake_effect: (f32, f32, f32),
    t_surface: f32,
    t_ground: f32,
    m_low: f32,
    m_up: f32,
    /// Eau que peuvent contenir les couches à saturation (mm).
    cap_low: f32,
    cap_up: f32,
    qs_low: f32,
    qs_low_top: f32,
    qs_top: f32,
    snow: bool,
    pet_base: f32,
    storms: f32,
    land_share: f32,
    ground: f32,
}

/// État propre à chaque case (sol et mémoires).
#[derive(Clone, Copy, Debug, Default)]
struct CellState {
    soil: f32,
    rh_mean: f32,
    /// Saturation de l'air à la température minimale (g/kg), recalculée de temps en temps.
    qs_night: f32,
    /// Air bas chassé par la subsidence pendant ce pas (mm).
    pushed: f32,
}

/// Eau de l'air au-dessus d'une case (mm).
struct Air {
    low: f32,
    up: f32,
    cloud: f32,
    fog: f32,
}

/// Cumuls pour les moyennes.
#[derive(Clone, Copy, Debug, Default)]
struct Sums {
    temperature: f32,
    temp_min: f32,
    temp_max: f32,
    rain: f32,
    pet: f32,
    evaporation: f32,
    runoff: f32,
    soil: f32,
    humidity: f32,
    column: f32,
    wind: (f32, f32),
}

/// Taux par pas de temps.
struct Rates {
    dt: f32,
    dt_days: f32,
    fallout: f32,
    snowfall: f32,
    cloud_evaporation: f32,
    drizzle: f32,
    rh_memory: f32,
}

/// Processus verticaux et échanges avec la surface d'une colonne d'air pendant un pas.
#[allow(clippy::too_many_arguments)]
fn column_step(
    air: &mut Air,
    cell: &mut CellState,
    c: &CellConst,
    r: &Regime,
    i: usize,
    rates: &Rates,
    soil_capacity: f32,
    refresh_dew: bool,
    sum: Option<&mut Sums>,
) {
    let (dt, dt_days) = (rates.dt, rates.dt_days);
    let efficiency_at = |rh: f32| PRECIPITATION_EFFICIENCY.0 + (PRECIPITATION_EFFICIENCY.1 - PRECIPITATION_EFFICIENCY.0) * rh;
    let rh = (air.low / c.cap_low).clamp(0.0, 1.5);
    cell.rh_mean += rates.rh_memory * (rh.min(1.0) - cell.rh_mean);

    // 2) Mouvements verticaux
    let rising_large = r.w_large[i].max(0.0);
    let sinking = (-r.w_large[i]).max(0.0);
    // Convection : air bas chaud et humide
    let unstable = ((rh - 0.6) / 0.3).clamp(0.0, 1.0) * ((c.t_surface - 15.0) / 10.0).clamp(0.0, 1.0);
    // Couvercle d'inversion sous les anticyclones : la convection est étouffée et l'air soulevé ne
    // dépasse guère la couche basse
    let lid = 1.0 - 0.9 * (r.w_subsidence[i] / 0.002).clamp(0.0, 1.0);
    // Le relief ne soulève l'air que de la hauteur de la pente : son effet passe par la couche
    // basse, qui suit le sol et se refroidit avec l'altitude (voir plus bas).
    // Montée profonde (fronts, convection, ascendance hors couvercle) et montée peu profonde
    // (convergence sous le couvercle d'un anticyclone : cumulus bas)
    // Effet de lac : sur le lac, et sur la rive vers laquelle souffle le vent
    let lake_w = match c.lake_effect {
        (f, _, _) if f <= 0.0 => 0.0,
        (f, 0.0, 0.0) => LAKE_EFFECT_W * f,
        (f, dx, dy) => {
            let (wx, wy) = r.surface[i];
            let speed = (wx * wx + wy * wy).sqrt().max(1e-6);
            LAKE_EFFECT_W * f * ((dx * wx + dy * wy) / speed).max(0.0)
        }
    };
    let w_deep = rising_large * lid + CONVECTION_W * unstable * lid + lake_w * lid + c.storms;
    let w_shallow = rising_large * (1.0 - lid);
    let w_up = w_deep + w_shallow;
    let lifted = (w_deep * dt / LOW_DEPTH).min(0.3);
    let lifted_shallow = (w_shallow * dt / LOW_DEPTH).min(0.3);
    let efficiency = efficiency_at(rh.min(1.0));
    if lifted_shallow > 0.0 {
        // Sous le couvercle, l'air ne monte que jusqu'au sommet de la couche basse et y reste :
        // cumulus bas, qui pleuvent peu
        let parcel = air.low * lifted_shallow;
        let capacity = c.qs_low_top * c.m_low * lifted_shallow;
        let condensed = (parcel - capacity).max(0.0) * PRECIPITATION_EFFICIENCY.0;
        air.low -= condensed;
        air.cloud += condensed;
    }
    if lifted > 0.0 {
        // L'air soulevé monte jusqu'au sommet de la couche haute, s'y refroidit et condense ; une
        // partie seulement précipite, le reste s'évapore en altitude
        let parcel = air.low * lifted;
        let capacity = c.qs_top * c.m_low * lifted;
        let condensed = (parcel - capacity).max(0.0) * efficiency;
        air.low -= parcel;
        air.up += parcel - condensed;
        air.cloud += condensed;
        // Le brouillard soulevé avec l'air devient nuage de pluie
        let lifted_fog = air.fog * lifted;
        air.fog -= lifted_fog;
        air.cloud += lifted_fog;
        // L'air de la couche haute est soulevé lui aussi (fronts, convection)
        let rh_up = (air.up / c.cap_up).min(1.0);
        let lifted_up = (w_up * dt / UPPER_DEPTH).min(0.3);
        let parcel_up = air.up * lifted_up;
        let capacity_up = c.qs_top * c.m_up * lifted_up;
        let condensed_up = (parcel_up - capacity_up).max(0.0) * efficiency_at(rh_up);
        air.up -= condensed_up;
        air.cloud += condensed_up;
    }
    // Près du sol, la subsidence s'atténue : l'air descendant n'entre que par le haut de la couche
    // basse (entraînement), d'où la moitié de la vitesse d'altitude
    let sunk = (0.5 * sinking * dt / LOW_DEPTH).min(0.3);
    if sunk > 0.0 {
        // L'air d'altitude descend et remplace une part de l'air bas, qui part sous le vent
        let arriving = air.up / c.m_up * c.m_low * sunk;
        cell.pushed += air.low * sunk;
        air.low += arriving - air.low * sunk;
        air.up -= arriving.min(air.up);
    }
    // Saturation. Sur une pente au vent, l'air qui monte en suivant le sol se refroidit : le nuage
    // qui s'y forme est un nuage de relief, qui pleut (une partie passe la crête avec l'air) ;
    // ailleurs, c'est du brouillard
    let excess_low = (air.low - c.cap_low).max(0.0);
    air.low -= excess_low;
    if r.w_relief[i] > 0.0 {
        air.cloud += excess_low * efficiency;
        air.up += excess_low * (1.0 - efficiency);
    } else {
        air.fog += excess_low;
    }
    // En altitude : sous un anticyclone, ce sont surtout des nuages élevés (cirrus) qui pleuvent à
    // peine ; ils restent dans la couche haute et partent avec le vent
    let excess_up = (air.up - c.cap_up).max(0.0) * (0.3 + 0.7 * lid);
    air.up -= excess_up;
    air.cloud += excess_up;
    // Le brouillard s'évapore dans un air bas non saturé, sinon il bruine
    let cleared = air.fog.min(rates.cloud_evaporation * (c.cap_low - air.low).max(0.0));
    air.fog -= cleared;
    air.low += cleared;
    let drizzled = air.fog * rates.drizzle;
    air.fog -= drizzled;
    // Brassage vertical : rapproche les humidités relatives des deux couches
    let moved = VERTICAL_MIXING_PER_DAY * dt_days * (air.low / c.cap_low - air.up / c.cap_up) * c.cap_up.min(c.cap_low);
    air.low -= moved;
    air.up += moved;

    // 3) Pluie (ou neige, qui tombe plus lentement) ; une partie s'évapore en tombant dans un air sec
    let fall_rate = if c.snow { rates.snowfall } else { rates.fallout };
    let from_cloud = air.cloud * fall_rate;
    air.cloud -= from_cloud;
    let falling = from_cloud + drizzled;
    // Le nuage restant (non précipité) s'évapore dans un air d'altitude non saturé
    let vanished = air.cloud.min(rates.cloud_evaporation * (c.cap_up - air.up).max(0.0));
    air.cloud -= vanished;
    air.up += vanished;
    let rh = (air.low / c.cap_low).clamp(0.0, 1.0);
    let reevaporated = falling * RAIN_REEVAPORATION * (1.0 - rh);
    air.low += reevaporated;
    let rain = falling - reevaporated;

    // 4) Surface
    let q_low = air.low / c.m_low;
    let q_surface = (q_low * NEAR_SURFACE_HUMIDITY).min(c.qs_low);
    let deficit = (c.qs_low - q_surface).max(0.0) / 1_000.0; // kg/kg
    let bulk = AIR_DENSITY * BULK_CE * r.speed[i] * deficit * 86_400.0; // mm/jour
    let range = if c.land { 4.0 + 2.0 * c.land_share + 14.0 * (1.0 - cell.rh_mean) * c.land_share } else { 2.0 };
    let (evap, runoff, dew, pet);
    if c.land {
        pet = (c.pet_base * range.sqrt()).min(bulk.max(0.0) + 0.5);
        // Rosée : l'air sature la nuit, à la température minimale
        if refresh_dew || cell.qs_night == 0.0 {
            cell.qs_night = saturation(c.t_surface - range / 2.0, c.ground);
        }
        dew = ((DEW_MAX_PER_DAY * dt_days).min(0.6 * dt_days * (q_low - cell.qs_night) * c.m_low)).max(0.0);
        air.low -= dew;
        cell.soil += rain + dew;
        let beta = (cell.soil / (soil_capacity * SOIL_FREE)).min(1.0);
        evap = (beta * pet * dt_days).min(cell.soil);
        cell.soil -= evap;
        runoff = (cell.soil - soil_capacity).max(0.0);
        cell.soil -= runoff;
    } else {
        pet = bulk;
        evap = bulk * dt_days;
        runoff = 0.0;
        dew = 0.0;
    }
    air.low += evap;

    if let Some(s) = sum {
        s.temperature += c.t_ground;
        s.temp_min += c.t_ground - range / 2.0;
        s.temp_max += c.t_ground + range / 2.0;
        s.rain += rain + dew;
        s.pet += pet * dt_days;
        s.evaporation += evap;
        s.runoff += runoff;
        s.soil += cell.soil / SOIL_CAPACITY;
        s.humidity += (air.low * NEAR_SURFACE_HUMIDITY / c.cap_low).min(1.0);
        s.column += air.low + air.up + air.cloud + air.fog;
        s.wind.0 += r.surface[i].0 / SURFACE_WIND;
        s.wind.1 += r.surface[i].1 / SURFACE_WIND;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 64;
    const H: i32 = 40;
    /// Colonnes et lignes du continent de test (40 × 32 cases, soit ~1 600 × 1 100 km), entouré de mer.
    const LAND: (i32, i32) = (12, 52);
    const ROWS: (i32, i32) = (4, 36);

    fn region(lat: f32) -> Region {
        Region::local(CELL_KM, lat)
    }

    /// Mer de la Terre autour du continent d'essai (écart à la latitude, °C), comme le donneraient
    /// les courants d'un grand océan (`currents.rs`) : courant froid le long de la côte ouest vers
    /// 15-40° (Canaries), courant chaud le long de la côte est (Gulf Stream), dérive chaude devant la
    /// côte ouest au-delà de 45° (Atlantique nord devant l'Europe).
    fn earth_sea(lat: f32) -> Vec<f32> {
        let l = lat.abs();
        let (west, east) = if l >= 45.0 {
            (4.0, -1.0)
        } else if l >= 15.0 {
            (-3.0, 2.0)
        } else {
            (0.0, 0.0)
        };
        (0..W * H).map(|i| if i % W < LAND.0 { west } else if i % W >= LAND.1 { east } else { 0.0 }).collect()
    }

    /// Continent rectangulaire ; option : un massif nord-sud (sommets au centre, contreforts).
    fn continent(range_at: Option<i32>) -> Vec<Surface> {
        let mut s = Vec::new();
        for row in 0..H {
            for col in 0..W {
                let land = (LAND.0..LAND.1).contains(&col) && (ROWS.0..ROWS.1).contains(&row);
                s.push(if !land {
                    Surface::Water
                } else {
                    let d = range_at.map_or(99, |m| (col - m).abs());
                    Surface::Land(match d {
                        0 => SUMMIT_ALTITUDE,
                        1 => altitude(6, false),
                        2 => altitude(5, false),
                        3 => altitude(3, false),
                        4 => altitude(2, false),
                        _ => altitude(1, false),
                    })
                });
            }
        }
        s
    }

    /// Moyenne d'un champ sur les lignes centrales du continent, pour une colonne.
    fn column_mean(field: &[f32], col: i32) -> f32 {
        let rows = (ROWS.0 + 6)..(ROWS.1 - 6);
        let n = rows.len() as f32;
        rows.map(|row| field[(row * W + col) as usize]).sum::<f32>() / n
    }

    /// Profil ouest → est des précipitations (mm/an) sur le continent.
    fn profile(c: &Climate) -> Vec<f32> {
        (LAND.0..LAND.1).map(|col| column_mean(&c.rain, col)).collect()
    }

    fn land_mean(c: &Climate, s: &[Surface]) -> f32 {
        let v: Vec<f32> = (0..s.len()).filter(|&i| matches!(s[i], Surface::Land(_))).map(|i| c.rain[i]).collect();
        v.iter().sum::<f32>() / v.len() as f32
    }

    #[test]
    fn region_et_latitudes() {
        let r = region(45.0);
        // 40 km par case : ~12° du nord au sud de la carte, centrés sur 45°
        assert!((r.latitude_deg(0, H) - 51.1).abs() < 0.5, "{}", r.latitude_deg(0, H));
        assert!((r.latitude_deg(H - 1, H) - 38.9).abs() < 0.5);
        let p = Region::planet(CELL_KM, H);
        assert!((p.latitude_deg(0, H) - 90.0).abs() < 0.1 && (p.latitude_deg(H - 1, H) + 90.0).abs() < 0.1);
        // La graine choisit une latitude dans la plage prévue
        for seed in 0..50 {
            let lat = Region::for_seed(seed).center_lat;
            assert!((LATITUDE_RANGE.0..=LATITUDE_RANGE.1).contains(&lat));
        }
    }

    #[test]
    fn vents_dominants() {
        let s = continent(None);
        let mean_x = |c: &Climate| c.wind.iter().map(|w| w.0).sum::<f32>() / c.wind.len() as f32;
        // Vents d'ouest aux latitudes moyennes, alizés (vers l'ouest) dans les tropiques
        assert!(mean_x(&simulate(W, H, &s, 5, &region(47.0), &[])) > 0.0, "vents d'ouest");
        assert!(mean_x(&simulate(W, H, &s, 5, &region(15.0), &[])) < 0.0, "alizés");
    }

    #[test]
    fn saturation_clausius_clapeyron() {
        // ~7 % de plus par degré, et ~4 fois plus à 30 °C qu'à 10 °C
        let r = saturation(21.0, 0.0) / saturation(20.0, 0.0);
        assert!((1.05..1.08).contains(&r), "{r}");
        let k = saturation(30.0, 0.0) / saturation(10.0, 0.0);
        assert!((3.0..4.2).contains(&k), "{k}");
    }

    #[test]
    fn evaporation_potentielle_realiste() {
        // Ordres de grandeur annuels : équateur humide ~1 500 mm, désert subtropical ~1 800 mm,
        // latitudes moyennes ~600 mm
        let year = |lat: f32, t: f32, range: f32| hargreaves(lat, t, range) * 365.0;
        assert!((1_200.0..1_900.0).contains(&year(0.0, 27.0, 8.0)));
        assert!((1_500.0..2_300.0).contains(&year(0.28, 24.0, 16.0)));
        assert!((400.0..800.0).contains(&year(0.55, 6.0, 9.0)));
    }

    #[test]
    fn climats_selon_la_latitude() {
        // Même continent : sec sous les anticyclones subtropicaux, plus arrosé aux latitudes
        // moyennes et près de l'équateur
        let s = continent(None);
        let subtropical = land_mean(&simulate(W, H, &s, 9, &region(35.0), &earth_sea(35.0)), &s);
        let temperate = land_mean(&simulate(W, H, &s, 9, &region(48.0), &earth_sea(48.0)), &s);
        let equatorial = land_mean(&simulate(W, H, &s, 9, &region(8.0), &earth_sea(8.0)), &s);
        assert!(temperate > subtropical * 1.15, "tempéré {temperate} / subtropical {subtropical}");
        assert!(equatorial > temperate * 3.0, "équatorial {equatorial} / tempéré {temperate}");
        // Ordres de grandeur : Europe de l'Ouest, Méditerranée, Bornéo
        assert!((350.0..900.0).contains(&temperate), "tempéré {temperate}");
        assert!((200.0..600.0).contains(&subtropical), "subtropical {subtropical}");
        assert!((2_000.0..6_000.0).contains(&equatorial), "équatorial {equatorial}");
    }

    #[test]
    fn pluie_penetre_dans_les_terres() {
        // Vents d'ouest : la pluie diminue progressivement en s'éloignant de la côte ouest
        let c = simulate(W, H, &continent(None), 11, &region(48.0), &earth_sea(48.0));
        let p = profile(&c);
        // Côte exposée très arrosée, puis une pluie qui ne baisse que lentement dans les terres
        assert!(p[0] > 800.0, "côte ouest {p:?}");
        assert!(p[4] > 300.0, "intérieur {p:?}");
        assert!(p[24] > p[4] * 0.7, "1 000 km plus loin : {p:?}");
    }

    #[test]
    fn relief_et_ombre_pluviometrique() {
        let m = (LAND.0 + LAND.1) / 2;
        let c = simulate(W, H, &continent(Some(m)), 11, &region(48.0), &earth_sea(48.0));
        let p = profile(&c);
        let at = |col: i32| p[(col - LAND.0) as usize];
        // Plus de pluie sur le versant au vent qu'en plaine en amont ; maximum avant le sommet
        let max_col = (m - 8..=m + 2).max_by(|&a, &b| at(a).total_cmp(&at(b))).unwrap();
        assert!(max_col < m, "le maximum doit être sur la pente au vent : {p:?}");
        assert!(at(max_col) > at(m - 8) * 1.3, "versant {p:?}");
        // Creux sous le vent (une partie de la neige déborde juste derrière la crête), puis
        // remontée plus loin
        let lee = (m + 2..=m + 8).map(at).fold(f32::MAX, f32::min);
        assert!(lee < at(max_col) * 0.1, "ombre {p:?}");
        assert!(at(m + 16) > lee * 1.3, "remontée {p:?}");
    }

    #[test]
    fn planete_bouclee() {
        // Petite planète océanique avec un continent : le climat se raccorde entre l'est et l'ouest
        let (w, h) = (96, 56);
        let s: Vec<Surface> = (0..h)
            .flat_map(|row| {
                (0..w).map(move |col| {
                    if (30..60).contains(&col) && (14..42).contains(&row) { Surface::Land(altitude(1, false)) } else { Surface::Water }
                })
            })
            .collect();
        let c = simulate(w, h, &s, 3, &Region::planet(CELL_KM, h), &[]);
        let at = |row: i32, col: i32| (row * w + col) as usize;
        // Raccord : de part et d'autre du bord, la pluie et le vent sont continus
        for row in (4..h - 4).step_by(6) {
            let (a, b) = (c.rain[at(row, 0)], c.rain[at(row, w - 1)]);
            let (inner_a, inner_b) = (c.rain[at(row, 1)], c.rain[at(row, 2)]);
            let typical = (a - inner_a).abs().max((inner_a - inner_b).abs()).max(50.0);
            assert!((a - b).abs() < 3.0 * typical, "ligne {row} : {a} / {b}");
        }
        // Vents d'ouest vers 45°, alizés vers 15°, dans les deux hémisphères
        let row_of = |lat: f32| ((1.0 - lat / 90.0) / 2.0 * (h - 1) as f32).round() as i32;
        let mean_x = |row: i32| (0..w).map(|col| c.wind[at(row, col)].0).sum::<f32>() / w as f32;
        assert!(mean_x(row_of(45.0)) > 0.0 && mean_x(row_of(-45.0)) > 0.0, "vents d'ouest");
        assert!(mean_x(row_of(15.0)) < 0.0 && mean_x(row_of(-15.0)) < 0.0, "alizés");
        // Sur l'océan : très arrosé à l'équateur, sec sous les anticyclones subtropicaux
        let ocean_rain = |lat: f32| {
            let row = row_of(lat);
            let v: Vec<f32> = (0..w).filter(|&col| !(30..60).contains(&col)).map(|col| c.rain[at(row, col)]).collect();
            v.iter().sum::<f32>() / v.len() as f32
        };
        let (equator, subtropics) = (ocean_rain(0.0), (ocean_rain(27.0) + ocean_rain(-27.0)) / 2.0);
        assert!(equator > subtropics * 2.0, "équateur {equator} / subtropiques {subtropics}");
    }

    #[test]
    #[ignore]
    fn profils() {
        let m = (LAND.0 + LAND.1) / 2;
        for (name, ridge) in [("plaine", None), ("massif", Some(m))].into_iter().take(if std::env::var("PLAIN").is_ok() { 1 } else { 2 }) {
            let s = continent(ridge);
            for lat in std::env::var("LATS").ok().map(|v| v.split(',').map(|x| x.parse().unwrap()).collect::<Vec<f32>>()).unwrap_or(vec![48.0, 35.0, 27.0, 8.0]) {
                let t = std::time::Instant::now();
                let c = simulate(W, H, &s, 11, &region(lat), &earth_sea(lat));
                let show = |f: &[f32], k: f32| -> String {
                    (LAND.0..LAND.1).map(|col| format!("{:.0}", k * column_mean(f, col))).collect::<Vec<_>>().join(" ")
                };
                if std::env::var("GRID").is_ok() {
                    for row in 0..H {
                        let line: String = (0..W)
                            .map(|col| {
                                let i = (row * W + col) as usize;
                                if !matches!(s[i], Surface::Land(_)) {
                                    return '.';
                                }
                                let k = [100.0, 200.0, 300.0, 450.0, 600.0, 800.0, 1_000.0, 1_400.0, 2_000.0]
                                    .iter()
                                    .filter(|&&t| c.rain[i] >= t)
                                    .count();
                                char::from_digit(k as u32, 10).unwrap()
                            })
                            .collect();
                        let wind: String = (0..W)
                            .step_by(4)
                            .map(|col| {
                                let (x, y) = c.wind[(row * W + col) as usize];
                                let a = y.atan2(x).to_degrees();
                                ['→', '↗', '↑', '↖', '←', '↙', '↓', '↘'][(((a + 360.0 + 22.5) % 360.0) / 45.0) as usize]
                            })
                            .collect();
                        println!("{line}  {wind}");
                    }
                }
                println!(
                    "{name} {lat}° ({:.1} s, moyenne {:.0} mm)\n  pluie {}\n  evap  {}\n  HR%   {}\n  sol%  {}",
                    t.elapsed().as_secs_f32(),
                    land_mean(&c, &s),
                    show(&c.rain, 1.0),
                    show(&c.evaporation, 1.0),
                    show(&c.humidity, 100.0),
                    show(&c.soil, 100.0)
                );
            }
        }
    }
}
