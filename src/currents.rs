//! Courants marins de surface et température de la mer, calculés à la génération du monde.
//!
//! **Circulation poussée par le vent** (modèle de Stommel, 1948) : le vent frotte la surface ; sa
//! rotation (rotationnel de la tension du vent) fait tourner l'eau, la variation de la force de
//! Coriolis avec la latitude (effet β) et le frottement sur le fond l'équilibrent. On résout
//! `r ∇²ψ + β ∂ψ/∂x = rot τ / (ρ H)` pour la fonction de courant ψ de la couche mélangée par le
//! vent ; les grands continents sont des bords où l'eau ne passe pas (les petites îles la laissent
//! passer). En sortent les tourbillons subtropicaux et subpolaires, les courants équatoriaux et le
//! **renforcement à l'ouest des océans** (courants de bord ouest étroits et rapides : Gulf Stream,
//! Kuroshio), avec des retours lents vers l'équateur à l'est. Près de l'équateur, où la force de
//! Coriolis s'annule, l'eau suit en plus directement le vent (courants équatoriaux vers l'ouest).
//!
//! **Circulation de retournement** (simplifiée) : aux latitudes moyennes, l'eau de surface dérive
//! en plus vers le pôle, où elle refroidit et plonge. Là où les courants des tourbillons sont lents
//! (côté est des océans), elle porte l'eau chaude jusqu'aux côtes ouest des continents des hautes
//! latitudes (dérive nord-atlantique devant l'Europe) ; les courants froids rapides qui longent les
//! côtes est des continents (Labrador) restent froids.
//!
//! **Courant circumpolaire** : si la mer fait le tour d'un pôle (passage maritime), la région
//! polaire est une « île » dont la fonction de courant est fixée par l'équilibre entre le vent
//! d'ouest et le frottement le long du passage.
//!
//! **Température de la mer** : chaque case tend vers la température de sa latitude (refroidie par
//! les remontées d'eaux froides), et le courant transporte cette chaleur vers l'aval : eau chaude
//! vers les pôles le long des côtes est des continents, eau froide vers l'équateur le long de
//! leurs côtes ouest, eau douce aux hautes latitudes. L'écart à la température de la latitude
//! (moyenne de l'air au niveau de la mer, continents compris) est transmis au climat : comme sur
//! Terre, la mer des hautes latitudes est plus chaude que cette moyenne.
//!
//! Logique pure (aucun rendu), déterministe.

use crate::climate::{Region, Surface};
use crate::hex::{HEX_SIZE, Hex};

/// Vent « fort » du climat (intensité normalisée 1), en m/s.
const WIND_SPEED_MS: f64 = 8.0;
/// Tension du vent : densité de l'air (kg/m³) et coefficient de frottement sur la mer.
const AIR_DENSITY: f64 = 1.2;
const DRAG: f64 = 1.3e-3;
/// Densité de l'eau de mer (kg/m³) et épaisseur de la couche mise en mouvement par le vent (m).
const WATER_DENSITY: f64 = 1_025.0;
const LAYER_DEPTH_M: f64 = 1_000.0;
/// Rotation de la planète (rad/s), comme la Terre.
const OMEGA: f64 = 7.292e-5;
/// Largeur des courants de bord ouest (couche de Stommel r/β, en cases à 30° de latitude) : elle
/// fixe le frottement `r`. Au moins une demi-case pour que la résolution reste stable.
const BOUNDARY_CELLS: f64 = 1.5;
/// Une terre d'au moins autant de cases (lacs compris) arrête l'eau ; une plus petite île la laisse
/// passer.
pub const BLOCKING_MIN_CELLS: usize = 100;
/// Vitesse de surface : vitesse moyenne de la couche × ce facteur (le courant est concentré près
/// de la surface).
const SURFACE_FACTOR: f32 = 50.0;
/// Dérive directe sous le vent près de l'équateur (m/s pour un vent d'intensité 1), jusqu'à cette
/// latitude (degrés).
const EQUATORIAL_DRIFT: f32 = 0.3;
const EQUATORIAL_DRIFT_LAT: f32 = 12.0;
/// Circulation de retournement : dérive de surface vers le pôle (m/s) aux latitudes moyennes (pleine
/// entre ces latitudes, nulle à `OVERTURNING_MARGIN` degrés au-delà) ; l'eau plonge aux hautes
/// latitudes. Elle porte l'eau chaude jusqu'aux côtes ouest des continents (dérive nord-atlantique).
const OVERTURNING: f32 = 0.12;
const OVERTURNING_LATITUDES: (f32, f32) = (42.0, 58.0);
const OVERTURNING_MARGIN: f32 = 8.0;
/// Temps pour que l'eau prenne la température de sa latitude (jours) : plus il est long, plus les
/// courants portent loin le chaud et le froid.
const RELAXATION_DAYS: f32 = 60.0;
/// Remontée d'eaux : latitudes (degrés) et refroidissement de l'eau (°C) pour un transport vers le
/// large d'intensité 1 : l'eau froide remonte sur place (la surface y est sans cesse renouvelée),
/// puis le courant l'emporte (refroidissement de l'eau qu'il transporte).
const UPWELLING_LATITUDES: (f32, f32) = (3.0, 45.0);
const UPWELLING_COOLING: f32 = 6.0;
/// Itérations des résolutions (arrêt avant si la solution ne bouge plus).
const MAX_SWEEPS: usize = 6_000;
const SOR: f64 = 1.7;

/// Courants et température de surface de chaque case (valeurs nulles sur terre).
#[derive(Clone, Debug, Default)]
pub struct Currents {
    /// Vitesse du courant de surface (m/s, x vers l'est, y vers le nord).
    pub velocity: Vec<(f32, f32)>,
    /// Température de surface de la mer (°C).
    pub temperature: Vec<f32>,
    /// Écart à la température de la latitude (°C) : négatif sur les courants froids.
    pub anomaly: Vec<f32>,
    /// Courant circumpolaire autour du pôle nord et du pôle sud (passage maritime).
    pub circumpolar: (bool, bool),
}

impl Currents {
    /// Vitesse (m/s) d'une case, si elle en a une.
    pub fn speed(&self, i: usize) -> f32 {
        self.velocity.get(i).map_or(0.0, |v| v.0.hypot(v.1))
    }
}

/// Vents théoriques par latitude (intensité normalisée, x vers l'est) : alizés d'est, vents
/// d'ouest des latitudes moyennes (au plus fort vers 45°), vents d'est polaires, comme les vents
/// moyens que le climat simule sur la mer. Servent avant que le climat ait simulé les siens.
pub fn theoretical_wind(width: i32, height: i32, region: &Region) -> Vec<(f32, f32)> {
    const EASTWARD: [(f32, f32); 7] = [(0.0, -0.7), (15.0, -0.95), (30.0, 0.0), (45.0, 0.65), (60.0, 0.0), (75.0, -0.65), (90.0, -0.3)];
    (0..height)
        .flat_map(|row| {
            let lat = region.latitude_deg(row, height).abs();
            let k = EASTWARD.iter().position(|e| e.0 >= lat).unwrap_or(EASTWARD.len() - 1).max(1);
            let ((a, ua), (b, ub)) = (EASTWARD[k - 1], EASTWARD[k]);
            std::iter::repeat_n((ua + (ub - ua) * (lat - a) / (b - a), 0.0), width as usize)
        })
        .collect()
}

/// Grille : voisins (indice et vecteur unité vers eux) et latitude de chaque case.
struct Grid {
    width: i32,
    height: i32,
    wraps: bool,
    neighbors: Vec<Vec<(usize, (f64, f64))>>,
    lat: Vec<f32>,
    /// Distance entre deux centres voisins (m).
    d: f64,
}

impl Grid {
    fn new(width: i32, height: i32, region: &Region) -> Self {
        let step = (HEX_SIZE * 3f32.sqrt()) as f64;
        let mut neighbors = Vec::with_capacity((width * height) as usize);
        let mut lat = Vec::with_capacity((width * height) as usize);
        for row in 0..height {
            for col in 0..width {
                let h = Hex::from_offset(col, row);
                let (x, y) = h.to_world();
                neighbors.push(
                    h.neighbors()
                        .iter()
                        .filter_map(|&nb| {
                            let (c, r) = nb.to_offset();
                            let c = if region.wraps { c.rem_euclid(width) } else { c };
                            let (nx, ny) = nb.to_world();
                            let e = (((nx - x) as f64) / step, ((ny - y) as f64) / step);
                            (c >= 0 && c < width && r >= 0 && r < height).then(|| ((r * width + c) as usize, e))
                        })
                        .collect(),
                );
                lat.push(region.latitude_deg(row, height));
            }
        }
        Grid { width, height, wraps: region.wraps, neighbors, lat, d: region.cell_km as f64 * 1_000.0 }
    }

    /// Case au bord de la carte : pôles (première et dernière lignes) et, sans tour complet,
    /// premières et dernières colonnes.
    fn edge(&self, i: usize) -> bool {
        let (row, col) = (i as i32 / self.width, i as i32 % self.width);
        row == 0 || row == self.height - 1 || (!self.wraps && (col == 0 || col == self.width - 1))
    }

    /// Gradient d'un champ (par mètre) : (1 / 3d) Σ (f_j − f_i) e_j sur les six voisins.
    fn gradient(&self, f: &[f64], i: usize) -> (f64, f64) {
        let (mut gx, mut gy) = (0.0, 0.0);
        for &(j, (ex, ey)) in &self.neighbors[i] {
            gx += (f[j] - f[i]) * ex;
            gy += (f[j] - f[i]) * ey;
        }
        (gx / (3.0 * self.d), gy / (3.0 * self.d))
    }
}

/// Courants de surface et température de la mer. `wind` : vent de surface de chaque case
/// (intensité normalisée, comme `Climate::wind`) ; les lacs comptent comme de la terre.
pub fn simulate(width: i32, height: i32, surface: &[Surface], wind: &[(f32, f32)], region: &Region) -> Currents {
    let n = (width * height) as usize;
    let grid = Grid::new(width, height, region);
    let sea: Vec<bool> = surface.iter().map(|s| matches!(s, Surface::Water)).collect();

    // Bords : pôles, grandes terres ; chaque ensemble de bords d'un seul tenant a sa valeur de ψ
    let land_size = component_sizes(&grid, &|i| !sea[i]);
    let boundary: Vec<bool> = (0..n).map(|i| grid.edge(i) || (!sea[i] && land_size[i] >= BLOCKING_MIN_CELLS)).collect();
    let group = components(&grid, &|i| boundary[i]);
    let (north, south) = (group[0], group[n - 1]);

    // Tension du vent (N/m²), β et frottement
    let stress: Vec<(f64, f64)> = wind
        .iter()
        .map(|&(x, y)| {
            let (u, v) = (x as f64 * WIND_SPEED_MS, y as f64 * WIND_SPEED_MS);
            let k = AIR_DENSITY * DRAG * u.hypot(v);
            (k * u, k * v)
        })
        .collect();
    let radius = grid.d / (region.degrees_per_row as f64).to_radians() * 0.866_025_4;
    let beta: Vec<f64> = grid.lat.iter().map(|&l| 2.0 * OMEGA * (l as f64).to_radians().cos() / radius).collect();
    let r = 2.0 * OMEGA * 30f64.to_radians().cos() / radius * BOUNDARY_CELLS * grid.d;
    let tx: Vec<f64> = stress.iter().map(|s| s.0).collect();
    let ty: Vec<f64> = stress.iter().map(|s| s.1).collect();
    let curl: Vec<f64> = (0..n).map(|i| grid.gradient(&ty, i).0 - grid.gradient(&tx, i).1).collect();

    let unknown: Vec<bool> = boundary.iter().map(|b| !b).collect();
    let forcing: Vec<f64> = curl.iter().map(|c| c / (WATER_DENSITY * LAYER_DEPTH_M)).collect();
    let mut psi = solve_stommel(&grid, &unknown, &beta, r, &forcing, vec![0.0; n]);

    // Courant circumpolaire : la région polaire, séparée des continents par la mer, prend la
    // valeur de ψ qui équilibre vent d'ouest et frottement le long du passage
    let mut circumpolar = (false, false);
    if north != south {
        for (pole, flag) in [(north, &mut circumpolar.0), (south, &mut circumpolar.1)] {
            let start: Vec<f64> = (0..n).map(|i| if boundary[i] && group[i] == pole { 1.0 } else { 0.0 }).collect();
            let unit = solve_stommel(&grid, &unknown, &beta, r, &vec![0.0; n], start);
            // (l'anneau doit faire le tour de la planète, du côté de ce pôle : sinon l'« île » est un
            // continent rattaché au pôle, et l'eau tourne autour de l'autre pôle)
            let ring: Vec<usize> = (0..n).filter(|&i| unknown[i] && sea[i] && unit[i] > 0.2 && unit[i] < 0.8).collect();
            let sign = if pole == north { 1.0 } else { -1.0 };
            let columns: std::collections::HashSet<i32> = ring.iter().filter(|&&i| grid.lat[i] * sign > 0.0).map(|&i| i as i32 % width).collect();
            if !grid.wraps || columns.len() < width as usize {
                continue;
            }
            let eastward = |f: &[f64], i: usize| -grid.gradient(f, i).1;
            let mean = |f: &dyn Fn(usize) -> f64| ring.iter().map(|&i| f(i)).sum::<f64>() / ring.len() as f64;
            let target = mean(&|i| tx[i]) / (WATER_DENSITY * LAYER_DEPTH_M * r);
            let (now, per_unit) = (mean(&|i| eastward(&psi, i)), mean(&|i| eastward(&unit, i)));
            if per_unit.abs() < 1e-12 {
                continue;
            }
            let k = (target - now) / per_unit;
            for i in 0..n {
                psi[i] += k * unit[i];
            }
            *flag = true;
        }
    }

    // Vitesse de surface : u = −∂ψ/∂y, v = ∂ψ/∂x, plus la dérive équatoriale sous le vent
    let velocity: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            if !sea[i] || boundary[i] {
                return (0.0, 0.0);
            }
            let (gx, gy) = grid.gradient(&psi, i);
            let lat = grid.lat[i];
            let drift = EQUATORIAL_DRIFT * (1.0 - lat.abs() / EQUATORIAL_DRIFT_LAT).max(0.0);
            let (a, b) = OVERTURNING_LATITUDES;
            let rise = ((lat.abs() - (a - OVERTURNING_MARGIN)) / OVERTURNING_MARGIN).clamp(0.0, 1.0);
            let fall = (((b + OVERTURNING_MARGIN) - lat.abs()) / OVERTURNING_MARGIN).clamp(0.0, 1.0);
            let poleward = OVERTURNING * rise.min(fall) * lat.signum();
            (-gy as f32 * SURFACE_FACTOR + drift * wind[i].0, gx as f32 * SURFACE_FACTOR + drift * wind[i].1 + poleward)
        })
        .collect();

    // Température : celle de la latitude (moins les remontées d'eaux), transportée par le courant
    let upwelling = upwelling(&grid, &sea, surface, wind);
    let latitude_t: Vec<f32> = grid.lat.iter().map(|&l| crate::climate::sea_level_temperature(l / 90.0)).collect();
    let target: Vec<f32> = (0..n).map(|i| latitude_t[i] - UPWELLING_COOLING * upwelling[i]).collect();
    let transported = transport_heat(&grid, &sea, &velocity, &target);
    // La température de la latitude est une moyenne terrestre qui tient déjà compte des courants :
    // une latitude où la mer serait en moyenne plus froide (les tropiques, qui exportent leur
    // chaleur) est ramenée à cette moyenne ; aux hautes latitudes, la mer reste plus chaude que la
    // moyenne de l'air, refroidie par les continents
    let mut deficit = vec![0.0f32; height as usize];
    for (row, d) in deficit.iter_mut().enumerate() {
        let cells: Vec<usize> = (0..width as usize).map(|col| row * width as usize + col).filter(|&i| sea[i]).collect();
        *d = (cells.iter().map(|&i| transported[i] - latitude_t[i]).sum::<f32>() / cells.len().max(1) as f32).min(0.0);
    }
    let anomaly: Vec<f32> = (0..n)
        .map(|i| if sea[i] { transported[i] - latitude_t[i] - deficit[i / width as usize] - UPWELLING_COOLING * upwelling[i] } else { 0.0 })
        .collect();
    let temperature: Vec<f32> = (0..n).map(|i| if sea[i] { latitude_t[i] + anomaly[i] } else { 0.0 }).collect();
    Currents { velocity, temperature, anomaly, circumpolar }
}

/// Résout `r ∇²ψ + β ∂ψ/∂x = forcing` sur les cases inconnues (les autres gardent la valeur de
/// `psi`), par surrelaxation. Discrétisation sur les six voisins : Σ c_j (ψ_j − ψ_i) = forcing,
/// c_j = 2r / 3d² + β e_x / 3d (tous positifs : la couche de Stommel fait au moins une demi-case).
fn solve_stommel(grid: &Grid, unknown: &[bool], beta: &[f64], r: f64, forcing: &[f64], mut psi: Vec<f64>) -> Vec<f64> {
    let d = grid.d;
    let cells: Vec<usize> = (0..psi.len()).filter(|&i| unknown[i]).collect();
    // Coefficients rapportés à 2r / 3d² (ψ en m²/s)
    let coef: Vec<Vec<(usize, f64)>> = cells
        .iter()
        .map(|&i| grid.neighbors[i].iter().map(|&(j, (ex, _))| (j, 1.0 + beta[i] * ex * d / (2.0 * r))).collect())
        .collect();
    let rhs: Vec<f64> = cells.iter().map(|&i| forcing[i] * 3.0 * d * d / (2.0 * r)).collect();
    for _ in 0..MAX_SWEEPS {
        let (mut change, mut size) = (0.0f64, 1e-30f64);
        for (k, &i) in cells.iter().enumerate() {
            let (mut sum, mut weight) = (0.0, 0.0);
            for &(j, c) in &coef[k] {
                sum += c * psi[j];
                weight += c;
            }
            let new = (sum - rhs[k]) / weight;
            let step = SOR * (new - psi[i]);
            psi[i] += step;
            change = change.max(step.abs());
            size = size.max(psi[i].abs());
        }
        if change < 1e-8 * size {
            break;
        }
    }
    psi
}

/// Remontées d'eaux froides (0 à 1) : le transport d'Ekman (90° à droite du vent au nord, à
/// gauche au sud) pousse l'eau de surface vers le large, à au plus deux cases d'une côte.
fn upwelling(grid: &Grid, sea: &[bool], surface: &[Surface], wind: &[(f32, f32)]) -> Vec<f32> {
    let n = sea.len();
    (0..n)
        .map(|i| {
            let lat = grid.lat[i];
            if !sea[i] || lat.abs() < UPWELLING_LATITUDES.0 || lat.abs() > UPWELLING_LATITUDES.1 {
                return 0.0;
            }
            // Direction du large : depuis les terres à une ou deux cases
            let (mut ox, mut oy) = (0.0f64, 0.0f64);
            for &(j, (ex, ey)) in &grid.neighbors[i] {
                for &(k, (fx, fy)) in &grid.neighbors[j] {
                    if k != i && matches!(surface[k], Surface::Land(_)) {
                        let (dx, dy) = (ex + fx, ey + fy);
                        let len = dx.hypot(dy);
                        ox -= dx / len;
                        oy -= dy / len;
                    }
                }
                if matches!(surface[j], Surface::Land(_)) {
                    ox -= ex * 2.0;
                    oy -= ey * 2.0;
                }
            }
            let len = ox.hypot(oy);
            if len < 1e-6 {
                return 0.0;
            }
            let (wx, wy) = wind[i];
            let ekman = if lat >= 0.0 { (wy, -wx) } else { (-wy, wx) };
            ((ekman.0 as f64 * ox + ekman.1 as f64 * oy) / len).clamp(0.0, 1.0) as f32
        })
        .collect()
}

/// Température de l'eau en régime établi : `u·∇T = (cible − T) / τ` (transport amont, résolu
/// par balayages alternés).
fn transport_heat(grid: &Grid, sea: &[bool], velocity: &[(f32, f32)], target: &[f32]) -> Vec<f32> {
    let n = sea.len();
    let relax = 1.0 / (RELAXATION_DAYS as f64 * 86_400.0);
    let cells: Vec<usize> = (0..n).filter(|&i| sea[i]).collect();
    // Apports de chaque voisine d'amont : (2 / 3d) max(0, −u·e)
    let inflow: Vec<Vec<(usize, f64)>> = cells
        .iter()
        .map(|&i| {
            let (u, v) = (velocity[i].0 as f64, velocity[i].1 as f64);
            grid.neighbors[i]
                .iter()
                .filter(|&&(j, _)| sea[j])
                .filter_map(|&(j, (ex, ey))| {
                    let a = (-(u * ex + v * ey)).max(0.0) * 2.0 / (3.0 * grid.d);
                    (a > 0.0).then_some((j, a))
                })
                .collect()
        })
        .collect();
    let mut t: Vec<f64> = target.iter().map(|&x| x as f64).collect();
    for sweep in 0..MAX_SWEEPS {
        let mut change = 0.0f64;
        let mut update = |k: usize| {
            let i = cells[k];
            let (mut sum, mut weight) = (target[i] as f64 * relax, relax);
            for &(j, a) in &inflow[k] {
                sum += a * t[j];
                weight += a;
            }
            let new = sum / weight;
            change = change.max((new - t[i]).abs());
            t[i] = new;
        };
        if sweep % 2 == 0 {
            (0..cells.len()).for_each(&mut update);
        } else {
            (0..cells.len()).rev().for_each(&mut update);
        }
        if change < 1e-3 {
            break;
        }
    }
    (0..n).map(|i| if sea[i] { t[i] as f32 } else { 0.0 }).collect()
}

/// Taille de l'ensemble d'un seul tenant (cases où `inside` est vrai) de chaque case.
fn component_sizes(grid: &Grid, inside: &dyn Fn(usize) -> bool) -> Vec<usize> {
    let group = components(grid, inside);
    let mut sizes = vec![0usize; group.len()];
    for &g in group.iter().filter(|&&g| g != usize::MAX) {
        sizes[g] += 1;
    }
    group.iter().map(|&g| if g == usize::MAX { 0 } else { sizes[g] }).collect()
}

/// Ensembles d'un seul tenant : numéro (indice de la première case trouvée) de chaque case où
/// `inside` est vrai, `usize::MAX` ailleurs.
fn components(grid: &Grid, inside: &dyn Fn(usize) -> bool) -> Vec<usize> {
    let n = grid.neighbors.len();
    let mut group = vec![usize::MAX; n];
    for start in 0..n {
        if group[start] != usize::MAX || !inside(start) {
            continue;
        }
        group[start] = start;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            for &(j, _) in &grid.neighbors[i] {
                if group[j] == usize::MAX && inside(j) {
                    group[j] = start;
                    stack.push(j);
                }
            }
        }
    }
    group
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 192;
    const H: i32 = 111;
    /// Colonnes de l'océan d'essai, entre deux continents.
    const OCEAN: (i32, i32) = (32, 140);

    /// Planète d'essai à la taille du jeu : un océan entre deux continents qui vont d'un pôle à
    /// l'autre, ou du pôle nord jusqu'à `passage` degrés de latitude sud, laissant la mer faire le
    /// tour du pôle sud.
    fn basin(passage: Option<f32>) -> (Vec<Surface>, Region) {
        let region = Region::planet(40.0, H);
        let surface = (0..H)
            .flat_map(|row| {
                let lat = region.latitude_deg(row, H);
                (0..W).map(move |col| {
                    let land = !(OCEAN.0..OCEAN.1).contains(&col) && row >= 1 && passage.is_none_or(|p| lat > -p);
                    if land { Surface::Land(200.0) } else { Surface::Water }
                })
            })
            .collect();
        (surface, region)
    }

    /// Courants de la planète d'essai sous les vents théoriques.
    fn currents(passage: Option<f32>) -> Currents {
        let (s, region) = basin(passage);
        simulate(W, H, &s, &theoretical_wind(W, H, &region), &region)
    }

    /// Moyenne d'une grandeur sur les cases de mer d'une bande de latitudes et de colonnes.
    fn mean_over(c: &Currents, lats: (f32, f32), cols: std::ops::Range<i32>, f: impl Fn(&Currents, usize) -> f32) -> f32 {
        let region = Region::planet(40.0, H);
        let rows: Vec<i32> = (0..H).filter(|&r| (lats.0..lats.1).contains(&region.latitude_deg(r, H))).collect();
        let cells: Vec<usize> = rows.iter().flat_map(|&r| cols.clone().map(move |c| (r * W + c) as usize)).collect();
        cells.iter().map(|&i| f(c, i)).sum::<f32>() / cells.len() as f32
    }

    #[test]
    fn tourbillon_subtropical_et_bord_ouest() {
        let c = currents(None);
        // Vers 25-35° : courant vers le pôle le long du bord ouest de l'océan, lent vers
        // l'équateur au milieu, dans les deux hémisphères
        let west = mean_over(&c, (25.0, 35.0), OCEAN.0..OCEAN.0 + 3, |c, i| c.velocity[i].1);
        let middle = mean_over(&c, (25.0, 35.0), 70..100, |c, i| c.velocity[i].1);
        assert!(west > 0.2, "bord ouest {west}");
        assert!(middle < 0.0 && middle.abs() < west / 4.0, "intérieur {middle} / bord ouest {west}");
        let west_s = mean_over(&c, (-35.0, -25.0), OCEAN.0..OCEAN.0 + 3, |c, i| c.velocity[i].1);
        assert!(west_s < -0.2, "bord ouest sud {west_s}");
    }

    #[test]
    fn eaux_froides_a_l_est_chaudes_a_l_ouest() {
        let c = currents(None);
        // Vers 20-35° : eau plus chaude le long du bord ouest de l'océan que le long du bord est
        let west = mean_over(&c, (20.0, 35.0), OCEAN.0..OCEAN.0 + 2, |c, i| c.anomaly[i]);
        let east = mean_over(&c, (20.0, 35.0), OCEAN.1 - 2..OCEAN.1, |c, i| c.anomaly[i]);
        assert!(west > east + 2.0, "ouest {west} / est {east}");
    }

    #[test]
    fn derive_chaude_des_hautes_latitudes() {
        let c = currents(None);
        // Vers 50-60° nord : eau douce le long de la côte ouest du continent de droite (côté est de
        // l'océan, comme l'Europe), plus froide le long de l'autre côte (comme le Labrador)
        let europe = mean_over(&c, (50.0, 60.0), OCEAN.1 - 2..OCEAN.1, |c, i| c.anomaly[i]);
        let labrador = mean_over(&c, (50.0, 60.0), OCEAN.0..OCEAN.0 + 2, |c, i| c.anomaly[i]);
        assert!(europe > 2.0 && europe > labrador + 2.0, "côte est de l'océan {europe} / côte ouest {labrador}");
    }

    #[test]
    fn courant_circumpolaire_par_le_passage() {
        let c = currents(Some(50.0));
        assert_eq!(c.circumpolar, (false, true));
        // Dans le passage, le courant fait le tour du pôle dans le sens du vent moyen
        let (_, region) = basin(None);
        let wind = theoretical_wind(W, H, &region);
        let flow = mean_over(&c, (-89.0, -50.0), 0..W, |c, i| c.velocity[i].0);
        let push = mean_over(&c, (-89.0, -50.0), 0..W, |_, i| wind[i].0);
        assert!(flow.abs() > 0.03 && flow * push > 0.0, "courant circumpolaire {flow}, vent {push}");
        assert_eq!(currents(None).circumpolar, (false, false));
    }
}
