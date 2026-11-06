//! Houle : bilan d'énergie simplifié, résolu à la génération (logique pure, testée).
//!
//! Le principe est celui des modèles spectraux (WAM, WaveWatch III), très réduit : chaque case de
//! mer porte l'énergie des vagues selon 12 directions et 3 périodes (8, 12 et 16 s), et cette
//! énergie change à chaque pas de temps sous trois effets :
//! - **propagation** : elle passe aux cases voisines à la vitesse de groupe (`g T / 4π`), en amont
//!   décentré, et les côtes et la banquise l'absorbent ;
//! - **vent** : elle croît vers la mer saturée (Pierson-Moskowitz), à un rythme réglé pour retrouver
//!   la loi de croissance avec le fetch (JONSWAP) ;
//! - **pertes** : dissipation lente de la houle au large (Ardhuin et al., 2009), frottement sur
//!   le fond du plateau (JONSWAP).
//!
//! Le vent est celui des régimes du climat (`climate::RegimeWind`), renforcé sur le rail des
//! dépressions : le climat n'a pas de vraies tempêtes (voir `sea_wind`). Aux côtes, un dernier
//! calcul (`shore_of`) donne le déferlement, l'énergie reçue et la dérive littorale.
//! Voir `docs/conception/conception-fonds-marins.md`.

use crate::climate::{RegimeWind, Region};
use crate::hex::Hex;
use rayon::prelude::*;
use std::f32::consts::{PI, TAU};

const G: f32 = 9.81;
/// Directions et périodes du spectre.
const DIRS: usize = 12;
const PERIODS: usize = 3;
const BINS: usize = DIRS * PERIODS;
/// Périodes des classes (s).
const PERIOD_S: [f32; PERIODS] = [8.0, 12.0, 16.0];
/// Distance (m) sur laquelle la houle de chaque classe perd un facteur e de son énergie : la houle
/// cambrée s'use vite, la longue traverse les océans (Ardhuin et al. : de 2 700 à plus de 20 000 km).
const SWELL_DECAY_M: [f32; PERIODS] = [3.0e6, 8.0e6, 2.0e7];
/// Coefficient de la mer saturée, `Hs = 0,21 U² / g` (Pierson-Moskowitz).
const SATURATED_HS: f32 = 0.21;
/// Pulsation du pic de la mer saturée, `ω = 0,877 g / U`.
const SATURATED_PEAK: f32 = 0.877;
/// Largeur (s) de la répartition de la mer du vent entre les classes de période.
const PERIOD_SPREAD_S: f32 = 4.0;
/// Temps de croissance (s par m/s de vent) : `Hs ≈ 0,0016 √(g F) U / g` au début du fetch (JONSWAP).
/// Avec `E(x) = Esat (1 − exp(−x / (Cg τ)))` et `Cg ≈ 0,57 U` au pic : `τ ≈ 3 084 U`.
const GROWTH_S_PER_MS: f32 = 3_084.0;
/// Frottement sur le fond (JONSWAP, m²/s³).
const BOTTOM_FRICTION: f32 = 0.038;
/// Au-delà, le fond ne se sent plus (m).
const BOTTOM_FELT_M: f32 = 250.0;
/// Hauteur la plus forte selon la profondeur (déferlement, part de la profondeur).
const DEPTH_LIMIT: f32 = 0.6;
/// Durée d'un régime et mise en route (jours) ; un relevé tous les `SAMPLE_EVERY` pas.
const REGIME_DAYS: f32 = 7.0;
const SPIN_UP_DAYS: f32 = 20.0;
const SAMPLE_EVERY: usize = 6;
/// Part de la vitesse la plus grande permise par pas (stabilité du schéma).
const CFL: f32 = 0.9;
/// Vent de surface du climat → vent sur la mer (le climat le cale à 8 m/s sur 90 % des cases).
const SEA_WIND_FACTOR: f32 = 1.2;
/// Renfort sur le rail des dépressions, et en plus l'hiver.
const STORM_BOOST: f32 = 1.0;
const WINTER_BOOST: f32 = 0.5;
/// Masse volumique de l'eau de mer (kg/m³).
const WATER_DENSITY: f32 = 1_025.0;
/// Déferlement : hauteur / profondeur (McCowan).
const BREAKER_RATIO: f32 = 0.78;
/// Part d'énergie autour de la seconde direction pour qu'on la retienne.
const SECOND_MIN_SHARE: f32 = 0.25;

/// Houle de chaque case (même indexation que la carte), en moyenne sur l'année.
#[derive(Clone, Debug, Default)]
pub struct Waves {
    /// Hauteur significative moyenne (m) ; 0 hors de la mer et sous la banquise.
    pub height: Vec<f32>,
    /// Plus forte hauteur significative de l'année (m).
    pub storm: Vec<f32>,
    /// Période moyenne, pondérée par l'énergie (s).
    pub period: Vec<f32>,
    /// Direction dominante, vers où va la houle (radians, 0 vers l'est, sens direct).
    pub direction: Vec<f32>,
    /// Part de l'énergie autour de la direction dominante (± 45°) : 0,25 sans direction nette.
    pub focus: Vec<f32>,
    /// Seconde houle, à au moins 90° de la première : direction, et part de l'énergie (0 sans elle).
    pub second_direction: Vec<f32>,
    pub second_share: Vec<f32>,
    /// Part de l'énergie qui ne vient pas du vent local (houle venue d'ailleurs).
    pub swell_share: Vec<f32>,
    /// Hauteur au déferlement sur la côte (m), 0 pour une case sans côte.
    pub shore_height: Vec<f32>,
    /// Énergie reçue par la côte (kW par mètre de rivage).
    pub shore_power: Vec<f32>,
    /// Dérive littorale : puissance le long du rivage (kW/m), avec son sens (x vers l'est, y vers le nord).
    pub shore_drift: Vec<(f32, f32)>,
    /// Hauteur (m) et période (s) au large de la houle qui arrive vers le rivage (pour le type d'embouchure).
    pub shore_deep_height: Vec<f32>,
    pub shore_period: Vec<f32>,
}

/// Part de l'énergie autour de la direction dominante à partir de laquelle on la dit nette.
pub const CLEAR_DIRECTION: f32 = 0.4;

/// D'où vient une houle qui va dans la direction `angle` (radians, 0 vers l'est, sens direct).
pub fn coming_from(angle: f32) -> &'static str {
    const NAMES: [&str; 8] = ["de l'est", "du nord-est", "du nord", "du nord-ouest", "de l'ouest", "du sud-ouest", "du sud", "du sud-est"];
    let sector = ((angle + PI).rem_euclid(TAU) / (PI / 4.0)).round() as usize % 8;
    NAMES[sector]
}

/// Houle d'un monde : `sea` les cases de mer, `ice` la banquise (pas de houle), `depth` (m) par case,
/// `regimes` le vent de chaque régime du climat, dans l'ordre de l'année.
pub fn simulate(width: i32, height: i32, sea: &[bool], ice: &[bool], depth: &[f32], regimes: &[RegimeWind], region: &Region) -> Waves {
    let n = (width * height) as usize;
    let lat: Vec<f32> = (0..n).map(|i| region.latitude_deg(i as i32 / width, height) / 90.0).collect();
    let winds: Vec<Vec<(f32, f32)>> = regimes
        .iter()
        .map(|r| {
            (0..n)
                .map(|i| {
                    let boost = sea_wind(lat[i], r.sun_lat);
                    (r.surface[i].0 * boost, r.surface[i].1 * boost)
                })
                .collect()
        })
        .collect();
    run(&Basin { width, height, wraps: region.wraps, cell_km: region.cell_km, sea, ice, depth }, &winds)
}

/// Rapport entre le vent de la mer et le vent de surface du climat, selon la latitude (−1 à 1) et
/// la position du soleil : le rail des dépressions, vers 38-61°, plus fort l'hiver.
fn sea_wind(lat: f32, sun_lat: f32) -> f32 {
    let rail = crate::climate::bump((lat - sun_lat * 0.5).abs(), 0.42, 0.68, 0.12);
    let winter = (-sun_lat * lat.signum() / 0.26).clamp(0.0, 1.0);
    SEA_WIND_FACTOR * (1.0 + STORM_BOOST * rail * (1.0 + WINTER_BOOST * winter))
}

/// Une mer et son fond.
struct Basin<'a> {
    width: i32,
    height: i32,
    wraps: bool,
    cell_km: f32,
    sea: &'a [bool],
    ice: &'a [bool],
    depth: &'a [f32],
}

/// Spectre : ce que la propagation tire de chaque direction.
struct Spectrum {
    /// Voisines d'où vient l'énergie de chaque direction, et leurs poids.
    upstream: [(usize, usize, f32, f32); DIRS],
    cos: [f32; DIRS],
    sin: [f32; DIRS],
    /// Part d'une case franchie par pas, par classe.
    courant: [f32; PERIODS],
    /// Perte par pas de la houle au large, par classe.
    keep: [f32; PERIODS],
    dt: f32,
}

impl Spectrum {
    fn new(cell_km: f32) -> Self {
        let dx = cell_km * 1_000.0;
        let cg = PERIOD_S.map(group_speed);
        // Le chemin d'une direction se répartit sur deux voisines : poids total au plus 2/√3
        let fastest = cg.iter().cloned().fold(0.0, f32::max);
        let dt = CFL * dx / (fastest * 2.0 / 3f32.sqrt());
        let angle = |d: usize| TAU * d as f32 / DIRS as f32;
        let sextant = PI / 3.0;
        let upstream = std::array::from_fn(|d| {
            let th = angle(d);
            let a = (th / sextant + 1e-4).floor() as usize % 6;
            let rest = (th - a as f32 * sextant).max(0.0);
            let s = sextant.sin();
            // Les directions de voisines (`hex::DIRECTIONS`) tournent de 60° en 60° depuis l'est
            ((a + 3) % 6, (a + 4) % 6, (sextant - rest).sin() / s, rest.sin() / s)
        });
        let mut keep = [0.0; PERIODS];
        for k in 0..PERIODS {
            keep[k] = 1.0 - dt * cg[k] / SWELL_DECAY_M[k];
        }
        Spectrum { upstream, cos: std::array::from_fn(|d| angle(d).cos()), sin: std::array::from_fn(|d| angle(d).sin()), courant: cg.map(|c| c * dt / dx), keep, dt }
    }
}

/// Vitesse de groupe en eau profonde (m/s).
fn group_speed(period: f32) -> f32 {
    G * period / (4.0 * PI)
}

/// Nombre d'onde pour une période et une profondeur (relation de dispersion, méthode de Newton).
fn wavenumber(period: f32, depth: f32) -> f32 {
    let omega = TAU / period;
    let mut k = omega * omega / G;
    for _ in 0..8 {
        let t = (k * depth).tanh();
        let f = G * k * t - omega * omega;
        let df = G * t + G * k * depth * (1.0 - t * t);
        k = (k - f / df.max(1e-9)).max(1e-6);
    }
    k
}

/// Perte par pas due au frottement sur le fond, par classe.
fn bottom_keep(depth: f32, dt: f32) -> [f32; PERIODS] {
    PERIOD_S.map(|t| {
        if depth >= BOTTOM_FELT_M {
            return 1.0;
        }
        let d = depth.max(5.0);
        let (omega, k) = (TAU / t, wavenumber(t, d));
        let rate = BOTTOM_FRICTION * omega * omega / (G * G * (k * d).sinh().powi(2).max(1e-4));
        (-rate * dt).exp()
    })
}

/// Cumuls d'une case : spectre, somme et maximum des hauteurs, somme de la houle venue d'ailleurs.
const TOTAL_HS: usize = BINS;
const MAX_HS: usize = BINS + 1;
const SWELL: usize = BINS + 2;
const SUMS: usize = BINS + 3;

/// Résout le bilan d'énergie sur les vents `winds` (par régime, par case, en m/s).
fn run(b: &Basin, winds: &[Vec<(f32, f32)>]) -> Waves {
    let n = (b.width * b.height) as usize;
    let mut out = Waves {
        height: vec![0.0; n],
        storm: vec![0.0; n],
        period: vec![0.0; n],
        direction: vec![0.0; n],
        focus: vec![0.0; n],
        second_direction: vec![0.0; n],
        second_share: vec![0.0; n],
        swell_share: vec![0.0; n],
        shore_height: vec![0.0; n],
        shore_power: vec![0.0; n],
        shore_drift: vec![(0.0, 0.0); n],
        shore_deep_height: vec![0.0; n],
        shore_period: vec![0.0; n],
    };
    let open = |i: usize| b.sea[i] && !b.ice[i];
    let cells: Vec<usize> = (0..n).filter(|&i| open(i)).collect();
    if cells.is_empty() || winds.is_empty() {
        return out;
    }
    let mut slot = vec![u32::MAX; n];
    for (s, &i) in cells.iter().enumerate() {
        slot[i] = s as u32;
    }
    // Voisines de chaque case de mer (directions de `Hex::neighbors`), sans la banquise ni la terre
    let neighbor_cell = |i: usize, k: usize| -> Option<usize> {
        let h = Hex::from_offset(i as i32 % b.width, i as i32 / b.width);
        let (c, r) = h.neighbors()[k].to_offset();
        let c = if b.wraps { c.rem_euclid(b.width) } else { c };
        (c >= 0 && c < b.width && r >= 0 && r < b.height).then_some((r * b.width + c) as usize)
    };
    let up: Vec<[u32; 6]> = cells.iter().map(|&i| std::array::from_fn(|k| neighbor_cell(i, k).map_or(u32::MAX, |j| slot[j]))).collect();
    let spectrum = Spectrum::new(b.cell_km);
    let keep_bottom: Vec<[f32; PERIODS]> = cells.iter().map(|&i| bottom_keep(b.depth[i], spectrum.dt)).collect();

    let steps_of = |days: f32| (days * 86_400.0 / spectrum.dt).ceil() as usize;
    let per_regime = steps_of(REGIME_DAYS).max(1);
    let spin_up = steps_of(SPIN_UP_DAYS);
    let total = spin_up + per_regime * winds.len();
    let mut energy = vec![0.0f32; cells.len() * BINS];
    let mut next = vec![0.0f32; cells.len() * BINS];
    let mut sums = vec![0.0f32; cells.len() * SUMS];
    let mut samples = 0.0f32;
    for step in 0..total {
        // Mise en route sur le dernier régime de l'année : l'année se referme
        let regime = if step < spin_up { winds.len() - 1 } else { (step - spin_up) / per_regime };
        let wind = &winds[regime];
        let sampling = step >= spin_up && (step - spin_up) % SAMPLE_EVERY == 0;
        if sampling {
            samples += 1.0;
        }
        let e = &energy;
        next.par_chunks_mut(BINS).zip(sums.par_chunks_mut(SUMS)).enumerate().for_each(|(s, (cell, sum))| {
            propagate(s, cell, e, &up[s], &spectrum);
            let swell = apply_sources(cell, wind[cells[s]], &spectrum, &keep_bottom[s], sampling);
            if sampling {
                let mut all = 0.0;
                for (acc, v) in sum[..BINS].iter_mut().zip(cell.iter()) {
                    *acc += v;
                    all += v;
                }
                let hs = 4.0 * all.sqrt();
                sum[TOTAL_HS] += hs;
                sum[MAX_HS] = sum[MAX_HS].max(hs);
                sum[SWELL] += swell;
            }
        });
        std::mem::swap(&mut energy, &mut next);
    }

    // Résultats : moyennes de l'année, directions, côtes
    let results: Vec<CellResult> = cells
        .par_iter()
        .enumerate()
        .map(|(s, &i)| {
            let sum = &sums[s * SUMS..(s + 1) * SUMS];
            let mean: [f32; BINS] = std::array::from_fn(|b| sum[b] / samples.max(1.0));
            let land = std::array::from_fn(|k| neighbor_cell(i, k).is_some_and(|j| !b.sea[j]));
            CellResult::of(&mean, sum, samples.max(1.0), b.depth[i], &land, &spectrum)
        })
        .collect();
    for (&i, r) in cells.iter().zip(&results) {
        out.height[i] = r.height;
        out.storm[i] = r.storm;
        out.period[i] = r.period;
        out.direction[i] = r.direction;
        out.focus[i] = r.focus;
        out.second_direction[i] = r.second_direction;
        out.second_share[i] = r.second_share;
        out.swell_share[i] = r.swell_share;
        out.shore_height[i] = r.shore.height;
        out.shore_power[i] = r.shore.power;
        out.shore_drift[i] = r.shore.drift;
        out.shore_deep_height[i] = r.shore.deep_height;
        out.shore_period[i] = r.shore.period;
    }
    out
}

/// Propagation (amont décentré) : l'énergie d'une direction vient des deux voisines d'où elle arrive.
#[inline]
fn propagate(s: usize, cell: &mut [f32], energy: &[f32], up: &[u32; 6], spectrum: &Spectrum) {
    for d in 0..DIRS {
        let (ka, kb, wa, wb) = spectrum.upstream[d];
        let (ua, ub) = (up[ka], up[kb]);
        for k in 0..PERIODS {
            let bin = d * PERIODS + k;
            let c = spectrum.courant[k];
            let from = |u: u32| if u == u32::MAX { 0.0 } else { energy[u as usize * BINS + bin] };
            cell[bin] = energy[s * BINS + bin] * (1.0 - c * (wa + wb)) + c * (wa * from(ua) + wb * from(ub));
        }
    }
}

/// Vent et pertes d'une case ; rend l'énergie qui dépasse ce que le vent local entretient (si
/// `sampling`).
#[inline]
fn apply_sources(cell: &mut [f32], wind: (f32, f32), spectrum: &Spectrum, keep_bottom: &[f32; PERIODS], sampling: bool) -> f32 {
    let u = wind.0.hypot(wind.1).max(0.1);
    let (wx, wy) = (wind.0 / u, wind.1 / u);
    let saturated = (SATURATED_HS * u * u / G).powi(2) / 16.0;
    let peak = TAU * u / (SATURATED_PEAK * G);
    let raw = PERIOD_S.map(|t| (-((t - peak) / PERIOD_SPREAD_S).powi(2)).exp());
    let total: f32 = raw.iter().sum::<f32>().max(1e-6);
    let growth = (spectrum.dt / (GROWTH_S_PER_MS * u.max(1.0))).min(1.0);
    let spread_norm = 4.0 / DIRS as f32;
    let mut swell = 0.0;
    for d in 0..DIRS {
        let cosd = spectrum.cos[d] * wx + spectrum.sin[d] * wy;
        let spread = if cosd > 0.0 { cosd * cosd * spread_norm } else { 0.0 };
        for k in 0..PERIODS {
            let bin = d * PERIODS + k;
            let target = saturated * raw[k] / total * spread;
            let mut v = cell[bin];
            if v < target {
                v += (target - v) * growth;
            } else if sampling {
                swell += v - target;
            }
            v *= spectrum.keep[k] * keep_bottom[k];
            // (pas de nombres dénormalisés : ils ralentissent beaucoup le calcul)
            cell[bin] = if v < 1e-10 { 0.0 } else { v };
        }
    }
    swell
}

/// Côte d'une case de mer.
#[derive(Clone, Copy, Default)]
struct Shore {
    height: f32,
    power: f32,
    drift: (f32, f32),
    deep_height: f32,
    period: f32,
}

/// Résultats d'une case.
struct CellResult {
    height: f32,
    storm: f32,
    period: f32,
    direction: f32,
    focus: f32,
    second_direction: f32,
    second_share: f32,
    swell_share: f32,
    shore: Shore,
}

impl CellResult {
    fn of(mean: &[f32; BINS], sum: &[f32], samples: f32, depth: f32, land: &[bool; 6], spectrum: &Spectrum) -> Self {
        let limit = (depth * DEPTH_LIMIT).max(0.5);
        let by_dir: [f32; DIRS] = std::array::from_fn(|d| mean[d * PERIODS..(d + 1) * PERIODS].iter().sum());
        let all: f32 = by_dir.iter().sum();
        let mut weighted = 0.0;
        for d in 0..DIRS {
            for k in 0..PERIODS {
                weighted += mean[d * PERIODS + k] * PERIOD_S[k];
            }
        }
        let around = |d: usize| by_dir[(d + DIRS - 1) % DIRS] + by_dir[d] + by_dir[(d + 1) % DIRS];
        let main = (0..DIRS).max_by(|&a, &b| around(a).total_cmp(&around(b))).unwrap_or(0);
        let angle_near = |d: usize| {
            let (mut x, mut y) = (0.0, 0.0);
            for o in [DIRS - 1, 0, 1] {
                let e = (d + o) % DIRS;
                x += by_dir[e] * spectrum.cos[e];
                y += by_dir[e] * spectrum.sin[e];
            }
            y.atan2(x)
        };
        // Seconde houle : la plus forte à au moins 90° de la première
        let apart = |d: usize| {
            let gap = (d + DIRS - main) % DIRS;
            gap.min(DIRS - gap) >= 3
        };
        let second = (0..DIRS).filter(|&d| apart(d)).max_by(|&a, &b| around(a).total_cmp(&around(b))).unwrap_or(main);
        let safe = all.max(1e-9);
        let second_share = around(second) / safe;
        CellResult {
            height: (sum[TOTAL_HS] / samples).min(limit),
            storm: sum[MAX_HS].min(limit),
            period: if all > 0.0 { weighted / all } else { 0.0 },
            direction: angle_near(main),
            focus: if all > 0.0 { around(main) / safe } else { 0.0 },
            second_direction: angle_near(second),
            second_share: if second_share >= SECOND_MIN_SHARE { second_share } else { 0.0 },
            swell_share: (sum[SWELL] / samples / safe).min(1.0),
            shore: shore_of(mean, land, spectrum),
        }
    }
}

/// Côte d'une case de mer d'après son spectre moyen : `land` dit quelles voisines sont des terres.
/// L'énergie qui arrive vers le rivage déferle (Komar et Gaughan, 1973) ; son angle se resserre
/// en eau peu profonde (loi de Snell) ; ce qui reste de biais pousse le sable le long du rivage.
fn shore_of(mean: &[f32; BINS], land: &[bool; 6], spectrum: &Spectrum) -> Shore {
    // Vers la terre : moyenne des directions des voisines de terre (`Hex::neighbors` : 60° par 60°)
    let (mut nx, mut ny) = (0.0f32, 0.0f32);
    let mut any = false;
    for (k, _) in land.iter().enumerate().filter(|(_, is_land)| **is_land) {
        let a = PI / 3.0 * k as f32;
        nx += a.cos();
        ny += a.sin();
        any = true;
    }
    let len = nx.hypot(ny);
    if !any || len < 0.1 {
        return Shore::default();
    }
    let (nx, ny) = (nx / len, ny / len);
    let (tx, ty) = (-ny, nx);
    // Hauteur et période de l'énergie qui arrive
    let (mut arriving, mut weighted) = (0.0f32, 0.0f32);
    for d in 0..DIRS {
        let cos_in = spectrum.cos[d] * nx + spectrum.sin[d] * ny;
        if cos_in <= 0.0 {
            continue;
        }
        for k in 0..PERIODS {
            let e = mean[d * PERIODS + k] * cos_in;
            arriving += e;
            weighted += e * PERIOD_S[k];
        }
    }
    if arriving <= 0.0 {
        return Shore::default();
    }
    let period = weighted / arriving;
    let deep_height_sq = 16.0 * arriving;
    let height = 0.39 * G.powf(0.2) * (period * deep_height_sq).powf(0.4);
    let breaker_depth = height / BREAKER_RATIO;
    let celerity_ratio = (G * breaker_depth).sqrt() / (G * period / TAU);
    let (mut power, mut along) = (0.0f32, 0.0f32);
    for d in 0..DIRS {
        let cos_in = spectrum.cos[d] * nx + spectrum.sin[d] * ny;
        if cos_in <= 0.0 {
            continue;
        }
        let sin_in = spectrum.cos[d] * tx + spectrum.sin[d] * ty;
        // (Snell : l'angle au déferlement ; le flux d'énergie vers le rivage se conserve)
        let sin_b = (sin_in * celerity_ratio).clamp(-1.0, 1.0);
        let cos_b = (1.0 - sin_b * sin_b).sqrt();
        for k in 0..PERIODS {
            let flux = WATER_DENSITY * G * mean[d * PERIODS + k] * group_speed(PERIOD_S[k]) * cos_in;
            power += flux;
            along += flux * sin_b / cos_b.max(0.1);
        }
    }
    Shore { height, power: power / 1_000.0, drift: (tx * along / 1_000.0, ty * along / 1_000.0), deep_height: deep_height_sq.sqrt(), period }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EAST: (f32, f32) = (1.0, 0.0);

    /// Toute une mer profonde, avec des terres là où `land` le dit.
    fn basin_run(w: i32, h: i32, wraps: bool, land: impl Fn(i32, i32) -> bool, wind: impl Fn(i32, i32) -> (f32, f32), depth: f32, regimes: usize) -> Waves {
        let n = (w * h) as usize;
        let sea: Vec<bool> = (0..n).map(|i| !land(i as i32 % w, i as i32 / w)).collect();
        let ice = vec![false; n];
        let d = vec![depth; n];
        let field: Vec<(f32, f32)> = (0..n).map(|i| wind(i as i32 % w, i as i32 / w)).collect();
        run(&Basin { width: w, height: h, wraps, cell_km: 40.0, sea: &sea, ice: &ice, depth: &d }, &vec![field; regimes])
    }

    fn at(w: i32, col: i32, row: i32) -> usize {
        (row * w + col) as usize
    }

    #[test]
    fn mer_saturee_sous_un_vent_uniforme() {
        // Mer qui fait le tour de la planète, vent de 10 m/s : Hs = 0,21 U²/g = 2,14 m
        let u = 10.0;
        let (w, h) = (40, 20);
        let waves = basin_run(w, h, true, |_, _| false, |_, _| (u, 0.0), 4_000.0, 1);
        // (le haut et le bas de la carte sont des bords : on regarde le milieu)
        let hs = waves.height[at(w, 20, 10)];
        assert!((hs - 2.14).abs() < 0.45, "Hs {hs}");
    }

    #[test]
    fn hauteur_croit_avec_le_fetch_comme_la_loi_jonswap() {
        // Vent de 10 m/s vers l'est, depuis un rivage à l'ouest
        let (w, h, u) = (90, 40, 10.0);
        let waves = basin_run(w, h, false, |c, _| c < 2, |_, _| (u, 0.0), 4_000.0, 2);
        let row = 20;
        let hs = |c: i32| waves.height[at(w, c + row / 2, row)];
        let jonswap = |km: f32| 0.0016 * (G * km * 1_000.0).sqrt() * u / G;
        // Début du fetch : loi en racine, à 45 % près (les cases font 40 km)
        for (cells, km) in [(3, 120.0), (5, 200.0)] {
            let (got, want) = (hs(2 + cells), jonswap(km));
            assert!((got - want).abs() < want * 0.45, "à {km} km : {got} au lieu de {want}");
        }
        assert!(hs(6) < hs(12) && hs(12) < hs(30), "{} {} {}", hs(6), hs(12), hs(30));
        // Loin du rivage : la mer saturée
        assert!((hs(70) - 2.14).abs() < 0.5, "Hs au large {}", hs(70));
    }

    #[test]
    fn la_houle_voyage_et_s_attenue_sous_un_ciel_calme() {
        // Tempête de 20 m/s à l'ouest de la colonne 15, calme ailleurs
        let (w, h) = (120, 40);
        let waves = basin_run(w, h, true, |_, _| false, |c, _| if c < 15 { (20.0, 0.0) } else { (0.0, 0.0) }, 4_000.0, 4);
        let row = 20;
        let (near, far) = (at(w, 30, row), at(w, 80, row));
        assert!(waves.height[near] > 0.5, "près {}", waves.height[near]);
        assert!(waves.height[far] > 0.1, "loin {}", waves.height[far]);
        assert!(waves.height[far] < waves.height[near], "{} {}", waves.height[near], waves.height[far]);
        // Elle va vers l'est, c'est de la houle venue d'ailleurs, et les longues périodes
        // gagnent avec la distance
        assert!(waves.direction[far].cos() > 0.8, "direction {}", waves.direction[far]);
        assert!(waves.swell_share[far] > 0.9, "part {}", waves.swell_share[far]);
        assert!(waves.period[far] >= waves.period[near] - 0.5, "{} {}", waves.period[near], waves.period[far]);
    }

    #[test]
    fn ombre_derriere_une_ile() {
        // Vent à l'ouest seulement ; une île barre la route d'une partie de la houle
        let (w, h) = (100, 50);
        let island = |c: i32, r: i32| (35..40).contains(&(c + r / 2)) && (20..30).contains(&r);
        let waves = basin_run(w, h, true, island, |c, _| if c < 20 { (18.0, 0.0) } else { (0.0, 0.0) }, 4_000.0, 3);
        let behind = waves.height[at(w, 44 - 25 / 2, 25)];
        let beside = waves.height[at(w, 44 - 40 / 2, 40)];
        assert!(behind < beside * 0.8, "derrière {behind}, à côté {beside}");
    }

    #[test]
    fn mer_fermee_plus_calme_que_le_large() {
        let (w, h) = (60, 40);
        let lake = |c: i32, r: i32| {
            let x = c + r / 2;
            !((20..26).contains(&x) && (15..21).contains(&r))
        };
        let wind = |_: i32, _: i32| (15.0, 0.0);
        let closed = basin_run(w, h, true, lake, wind, 4_000.0, 1);
        let open = basin_run(w, h, true, |_, _| false, wind, 4_000.0, 1);
        let c = at(w, 25 - 18 / 2, 18);
        assert!(closed.height[c] > 0.0 && closed.height[c] < open.height[c] * 0.7, "{} {}", closed.height[c], open.height[c]);
    }

    #[test]
    fn le_fond_du_plateau_use_la_houle_longue() {
        let (w, h) = (40, 20);
        let wind = |_: i32, _: i32| (15.0, 0.0);
        let deep = basin_run(w, h, true, |_, _| false, wind, 4_000.0, 1);
        let shelf = basin_run(w, h, true, |_, _| false, wind, 20.0, 1);
        let c = at(w, 20, 10);
        assert!(shelf.height[c] < deep.height[c] * 0.9, "{} {}", shelf.height[c], deep.height[c]);
    }

    #[test]
    fn la_banquise_arrete_la_houle() {
        let (w, h) = (60, 30);
        let n = (w * h) as usize;
        let ice: Vec<bool> = (0..n).map(|i| (25..30).contains(&(i as i32 % w))).collect();
        let (sea, d) = (vec![true; n], vec![4_000.0; n]);
        let field: Vec<(f32, f32)> = (0..n).map(|i| if ((i as i32) % w) < 20 { (18.0, 0.0) } else { (0.0, 0.0) }).collect();
        let waves = run(&Basin { width: w, height: h, wraps: false, cell_km: 40.0, sea: &sea, ice: &ice, depth: &d }, &vec![field; 2]);
        assert_eq!(waves.height[at(w, 27, 15)], 0.0);
        assert!(waves.height[at(w, 35, 15)] < 0.05, "derrière {}", waves.height[at(w, 35, 15)]);
    }

    #[test]
    fn cote_energie_et_derive_littorale() {
        // Rivage nord-sud à l'est (colonnes 50 et plus)
        let (w, h) = (60, 40);
        let land = |c: i32, r: i32| c + r / 2 >= 50 + 20 / 2;
        let row = 20;
        let col = 50 + 20 / 2 - row / 2 - 1;
        let c = at(w, col, row);
        assert!(!land(col, row) && land(col + 1, row));
        // Vent droit sur le rivage : de l'énergie, presque pas de dérive
        let straight = basin_run(w, h, false, land, |_, _| (12.0, 0.0), 500.0, 1);
        assert!(straight.shore_power[c] > 1.0, "énergie {}", straight.shore_power[c]);
        assert!(straight.shore_height[c] > 0.5, "déferlement {}", straight.shore_height[c]);
        assert!(straight.shore_drift[c].1.abs() < straight.shore_power[c] * 0.2, "dérive {:?}", straight.shore_drift[c]);
        // Vent de biais, vers le nord-est : la dérive part vers le nord
        let (a, b) = (12.0 * 0.707, 12.0 * 0.707);
        let slanted = basin_run(w, h, false, land, |_, _| (a, b), 500.0, 1);
        assert!(slanted.shore_drift[c].1 > 0.1, "dérive {:?}", slanted.shore_drift[c]);
        // Pas de côte : rien
        let open = at(w, 10, 20);
        assert_eq!(straight.shore_power[open], 0.0);
    }

    #[test]
    fn deux_houles_croisees_gardent_chacune_leur_direction() {
        // Un régime vers l'est, un régime vers le nord : deux houles à 90°
        let (w, h) = (40, 30);
        let n = (w * h) as usize;
        let (sea, ice, d) = (vec![true; n], vec![false; n], vec![4_000.0; n]);
        let winds = vec![vec![EAST; n].iter().map(|v| (v.0 * 14.0, v.1)).collect::<Vec<_>>(), vec![(0.0, 13.0); n]];
        let waves = run(&Basin { width: w, height: h, wraps: true, cell_km: 40.0, sea: &sea, ice: &ice, depth: &d }, &winds);
        let c = at(w, 20, 15);
        assert!(waves.second_share[c] > 0.0, "seconde {}", waves.second_share[c]);
        let (a, b) = (waves.direction[c], waves.second_direction[c]);
        let gap = (a - b).sin().abs();
        assert!(gap > 0.8, "directions {a} {b}");
    }

    #[test]
    fn meme_vent_meme_houle() {
        let wind = |c: i32, r: i32| (((c as f32) * 0.3).sin() * 12.0, ((r as f32) * 0.2).cos() * 9.0);
        let a = basin_run(50, 30, true, |c, r| c == 10 && r < 20, wind, 300.0, 2);
        let b = basin_run(50, 30, true, |c, r| c == 10 && r < 20, wind, 300.0, 2);
        assert_eq!(a.height, b.height);
        assert_eq!(a.direction, b.direction);
    }

    #[test]
    fn nom_de_l_origine_de_la_houle() {
        // Une houle qui va vers l'est vient de l'ouest ; vers le sud-ouest, du nord-est
        assert_eq!(coming_from(0.0), "de l'ouest");
        assert_eq!(coming_from(PI / 2.0), "du sud");
        assert_eq!(coming_from(-3.0 * PI / 4.0), "du nord-est");
    }

    #[test]
    fn rail_des_depressions() {
        // Plus fort vers 50° que sous les tropiques, et l'hiver que l'été
        assert!(sea_wind(0.55, 0.0) > sea_wind(0.1, 0.0) * 1.3);
        assert!(sea_wind(-0.55, 0.26) > sea_wind(-0.55, -0.26));
    }
}
