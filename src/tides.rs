//! Marées, calculées à la génération du monde.
//!
//! **Modèle** : les équations de l'eau peu profonde linéarisées (équations de marée de Laplace),
//! résolues pas à pas sur les cases de mer. La surface monte et descend (`η`), l'eau court d'une
//! case à l'autre ; une onde y va à `√(g H)` (environ 200 m/s par 4 000 m de fond, 20 m/s sur le
//! plateau continental). Vitesses portées par les arêtes entre deux cases de mer (grille décalée
//! : pas de vitesse à travers une côte), hauteurs au centre des cases.
//!
//! - **Moteur** : la marée d'équilibre lunaire semi-diurne (onde M2, période 12 h 25 min), qui fait
//!   le tour de la planète en un jour lunaire : `η_eq = K cos²φ cos(ωt + 2λ)`. Le Soleil ajoute
//!   l'onde S2 (46 % de M2), avec la même réponse des bassins : aux vives-eaux, les deux
//!   s'additionnent (marnage de vives-eaux = 2 × 1,46 × l'amplitude de M2).
//! - **Force de Coriolis** selon la latitude : l'onde tourne autour de points où la marée est nulle
//!   (points amphidromiques), dans le sens inverse des aiguilles d'une montre au nord.
//! - **Frottement** sur le fond, d'autant plus fort que l'eau est peu profonde (frottement
//!   quadratique linéarisé), plus une perte lente partout (en pleine mer, l'onde perd de son
//!   énergie en ondes internes) : la marée se dissipe surtout sur les plateaux.
//!
//! En sortent les grands traits des marées terrestres : faibles en plein océan (moins d'un mètre),
//! amplifiées sur les plateaux continentaux (l'onde ralentit et grandit) et dans les bassins qui
//! résonnent, presque nulles dans les mers fermées reliées à l'océan par un détroit (Méditerranée,
//! Baltique). Le calcul démarre au repos et tourne une dizaine de marées avant la mesure (le
//! régime établi) ; l'amplitude, l'heure de la pleine mer et le courant de marée le plus fort sont
//! tirés de la dernière marée.
//!
//! **Limite** : avec des cases de 40 km, un golfe étroit tient en une ou deux cases ; les
//! résonances locales (baie de Fundy, baie du Mont-Saint-Michel) n'y sont pas résolues.
//!
//! Logique pure (aucun rendu), déterministe.

use crate::climate::Region;
use crate::hex::{COLUMN_WIDTH, DIRECTIONS, Hex};
use bevy::math::Vec2;
use rayon::prelude::*;

/// Pesanteur (m/s²).
const G: f64 = 9.81;
/// Rotation de la planète (rad/s), comme la Terre.
const OMEGA: f64 = 7.292e-5;
/// Onde lunaire semi-diurne M2 : pulsation (rad/s) et amplitude de la marée d'équilibre à
/// l'équateur (m), réduite par la déformation de la Terre solide (facteur 0,69).
const M2_OMEGA: f64 = 1.405_19e-4;
const M2_EQUILIBRIUM_M: f64 = 0.242 * 0.69;
/// Rapport de l'onde solaire S2 à M2 (vives-eaux : les deux s'additionnent).
pub const S2_RATIO: f32 = 0.46;
/// Frottement sur le fond (m/s, coefficient du frottement linéarisé : environ 8/3π × 2,5·10⁻³ ×
/// 1 m/s), et perte lente partout (1/s : énergie cédée aux ondes internes, en quelques jours).
const BOTTOM_FRICTION: f64 = 2.1e-3;
const BACKGROUND_DAMPING: f64 = 1.0 / (2.0 * 86_400.0);
/// Profondeur au plus prise en compte (m) : les fosses ne changent rien à la marée et
/// raccourciraient le pas de temps.
const MAX_DEPTH_M: f64 = 6_000.0;
/// Profondeur au moins (m) : les cases côtières très peu profondes restent stables.
const MIN_DEPTH_M: f64 = 10.0;
/// Marées de mise en route (la marée perd son énergie en un à deux jours : le régime est établi
/// en quelques jours), puis marée de mesure ; marge sur le pas de temps le plus long stable.
const SPIN_UP_TIDES: usize = 10;
const CFL: f64 = 0.45;

/// Marée de chaque case de mer (valeurs nulles sur terre et dans les lacs).
#[derive(Clone, Debug, Default)]
pub struct Tides {
    /// Amplitude de l'onde M2 (m) : la moitié de l'écart entre pleine et basse mer moyennes.
    pub amplitude: Vec<f32>,
    /// Heure de la pleine mer après le passage de la Lune au méridien zéro (h, 0 à 12,42).
    pub high_water_h: Vec<f32>,
    /// Vitesse du courant de marée au plus fort (m/s), aux vives-eaux.
    pub current: Vec<f32>,
}

impl Tides {
    /// Marnage de vives-eaux (m) : écart entre pleine et basse mer quand la Lune et le Soleil
    /// s'ajoutent.
    pub fn spring_range(&self, i: usize) -> f32 {
        self.amplitude.get(i).map_or(0.0, |a| 2.0 * a * (1.0 + S2_RATIO))
    }
}

/// Période de l'onde M2 (h).
pub const M2_PERIOD_H: f32 = 12.42;

/// Lignes d'égale heure de pleine mer (lignes cotidales), toutes les heures : morceaux de ligne
/// (heure, extrémités en coordonnées monde 2D) et points amphidromiques (où la marée est nulle,
/// et autour desquels l'heure de la pleine mer fait le tour du cycle). Calculées sur les
/// triangles formés par les centres de trois cases de mer voisines dont `keep` dit oui (cases
/// explorées) : l'heure y varie linéairement, ramenée à moins d'une demi-période de celle du
/// premier sommet (elle est cyclique) ; un triangle dont les écarts d'heure font le tour du cycle
/// entoure un point amphidromique (pas de ligne dedans). La carte qui fait le tour de la planète :
/// les triangles du raccord sont pris des deux côtés (positions non ramenées dans la carte).
pub fn cotidal_lines(width: i32, height: i32, wraps: bool, high_water_h: &[f32], sea: &[bool], keep: impl Fn(usize) -> bool) -> (Vec<(u8, Vec2, Vec2)>, Vec<Vec2>) {
    let index = |h: Hex| {
        let (c, r) = h.to_offset();
        let c = if wraps { c.rem_euclid(width) } else { c };
        (c >= 0 && c < width && r >= 0 && r < height).then_some((r * width + c) as usize)
    };
    let wrap_diff = |d: f32| (d + M2_PERIOD_H / 2.0).rem_euclid(M2_PERIOD_H) - M2_PERIOD_H / 2.0;
    let (mut lines, mut amphidromes) = (Vec::new(), Vec::new());
    for row in 0..height {
        for col in 0..width {
            let h = Hex::from_offset(col, row);
            // Deux coins par case (à −30° et 30°) : chaque coin de la grille une seule fois
            for j in 0..2 {
                let cells = [h, h + DIRECTIONS[(j + 5) % 6], h + DIRECTIONS[j]];
                let Some(ids) = cells.iter().map(|&c| index(c)).collect::<Option<Vec<usize>>>() else { continue };
                if !ids.iter().all(|&i| sea[i] && keep(i)) {
                    continue;
                }
                let pts = cells.map(|c| {
                    let (x, y) = c.to_world();
                    Vec2::new(x, y)
                });
                let t = [high_water_h[ids[0]], high_water_h[ids[1]], high_water_h[ids[2]]];
                let winding = wrap_diff(t[1] - t[0]) + wrap_diff(t[2] - t[1]) + wrap_diff(t[0] - t[2]);
                if winding.abs() > M2_PERIOD_H / 2.0 {
                    amphidromes.push((pts[0] + pts[1] + pts[2]) / 3.0);
                    continue;
                }
                let v = [t[0], t[0] + wrap_diff(t[1] - t[0]), t[0] + wrap_diff(t[2] - t[0])];
                let (lo, hi) = (v[0].min(v[1]).min(v[2]), v[0].max(v[1]).max(v[2]));
                for hour in 0..=12u8 {
                    for turn in [-1.0f32, 0.0, 1.0] {
                        let level = hour as f32 + turn * M2_PERIOD_H;
                        if level <= lo || level >= hi {
                            continue;
                        }
                        let cut: Vec<Vec2> = [(0, 1), (1, 2), (2, 0)]
                            .iter()
                            .filter(|&&(a, b)| (v[a] - level) * (v[b] - level) < 0.0)
                            .map(|&(a, b)| pts[a].lerp(pts[b], (level - v[a]) / (v[b] - v[a])))
                            .collect();
                        if let [a, b] = cut[..] {
                            lines.push((hour, a, b));
                        }
                    }
                }
            }
        }
    }
    (lines, amphidromes)
}

/// Sens dans lequel la marée avance (de la pleine mer la plus tôt vers la plus tard), sur une case
/// de mer sur trois dans chaque sens (`keep` : cases explorées) : départ et arrivée de la flèche
/// (coordonnées monde 2D, longueur fixe `length`). Pas de flèche là où la marée est presque nulle
/// (près d'un point amphidromique, l'heure n'y veut plus rien dire) ni là où la pleine mer a lieu
/// presque à la même heure sur les cases voisines.
pub fn tide_arrows(width: i32, height: i32, wraps: bool, high_water_h: &[f32], amplitude: &[f32], sea: &[bool], length: f32, keep: impl Fn(usize) -> bool) -> Vec<(Vec2, Vec2)> {
    let index = |h: Hex| {
        let (c, r) = h.to_offset();
        let c = if wraps { c.rem_euclid(width) } else { c };
        (c >= 0 && c < width && r >= 0 && r < height).then_some((r * width + c) as usize)
    };
    let wrap_diff = |d: f32| (d + M2_PERIOD_H / 2.0).rem_euclid(M2_PERIOD_H) - M2_PERIOD_H / 2.0;
    let mut arrows = Vec::new();
    for row in (1..height - 1).step_by(3) {
        // (en quinconce d'une rangée à l'autre)
        for col in ((row / 3) % 2 * 2..width).step_by(3) {
            let i = (row * width + col) as usize;
            if !sea[i] || !keep(i) || amplitude[i] < ARROW_MIN_AMPLITUDE_M {
                continue;
            }
            let h = Hex::from_offset(col, row);
            let (x, y) = h.to_world();
            let here = Vec2::new(x, y);
            // Gradient de l'heure de la pleine mer (h par unité monde), sur les voisines de mer
            let mut g = Vec2::ZERO;
            for nb in h.neighbors() {
                let Some(j) = index(nb) else { continue };
                if !sea[j] {
                    continue;
                }
                let (nx, ny) = nb.to_world();
                let e = Vec2::new(nx, ny) - here;
                g += e / e.length_squared() * wrap_diff(high_water_h[j] - high_water_h[i]);
            }
            // (une mer qui monte d'un bloc, toute à la même heure : pas de sens à montrer)
            if g.length() * COLUMN_WIDTH < ARROW_MIN_SHIFT_H {
                continue;
            }
            let d = g.normalize() * length / 2.0;
            arrows.push((here - d, here + d));
        }
    }
    arrows
}

/// Amplitude (m) au-dessous de laquelle le sens de la marée n'est pas montré, et écart d'heure de
/// la pleine mer d'une case à la suivante (h) au-dessous duquel la marée n'avance pas assez pour
/// avoir un sens (elle monte d'un bloc).
const ARROW_MIN_AMPLITUDE_M: f32 = 0.05;
const ARROW_MIN_SHIFT_H: f32 = 0.08;

/// Arête entre deux cases de mer : de `a` vers `b`, direction (unité), profondeur.
struct Edge {
    a: usize,
    b: usize,
    n: (f64, f64),
    depth: f64,
}

/// Marées des cases de mer (`sea`), de profondeur `depth` (m).
pub fn simulate(width: i32, height: i32, sea: &[bool], depth: &[f32], region: &Region) -> Tides {
    let n = (width * height) as usize;
    let d = region.cell_km as f64 * 1_000.0;
    let step = (COLUMN_WIDTH as f64) * 1.0;
    let mut edges: Vec<Edge> = Vec::new();
    let mut lon = vec![0.0f64; n];
    let mut coriolis = vec![0.0f64; n];
    let mut weight = vec![0.0f64; n];
    for row in 0..height {
        for col in 0..width {
            let i = (row * width + col) as usize;
            let h = Hex::from_offset(col, row);
            let (x, y) = h.to_world();
            lon[i] = (x as f64 / (width as f64 * step)) * std::f64::consts::TAU;
            coriolis[i] = 2.0 * OMEGA * (region.latitude_deg(row, height) as f64).to_radians().sin();
            weight[i] = (region.latitude_deg(row, height) as f64).to_radians().cos().powi(2);
            if !sea[i] {
                continue;
            }
            for nb in h.neighbors() {
                let (c, r) = nb.to_offset();
                let c = if region.wraps { c.rem_euclid(width) } else { c };
                if c < 0 || c >= width || r < 0 || r >= height {
                    continue;
                }
                let j = (r * width + c) as usize;
                // (chaque arête une seule fois, de la plus petite case vers la plus grande)
                if j <= i || !sea[j] {
                    continue;
                }
                let (nx, ny) = nb.to_world();
                let (ex, ey) = ((nx - x) as f64, (ny - y) as f64);
                let len = ex.hypot(ey);
                let depth = ((depth[i] as f64 + depth[j] as f64) / 2.0).clamp(MIN_DEPTH_M, MAX_DEPTH_M);
                edges.push(Edge { a: i, b: j, n: (ex / len, ey / len), depth });
            }
        }
    }
    if edges.is_empty() {
        return Tides { amplitude: vec![0.0; n], high_water_h: vec![0.0; n], current: vec![0.0; n] };
    }
    // Arêtes de chaque case (indice, signe : +1 si l'eau qui y passe dans son sens sort de la case)
    let mut around: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for (k, e) in edges.iter().enumerate() {
        around[e.a].push((k, 1.0));
        around[e.b].push((k, -1.0));
    }
    // Pas de temps : l'onde la plus rapide ne franchit qu'une part d'une case par pas
    let fastest = edges.iter().map(|e| (G * e.depth).sqrt()).fold(0.0, f64::max);
    let period = std::f64::consts::TAU / M2_OMEGA;
    let per_tide = (period / (CFL * d / fastest)).ceil() as usize;
    let dt = period / per_tide as f64;
    // Rapport longueur d'une arête / surface d'une case (hexagones de côté d/√3)
    let flux_factor = 2.0 / (3.0 * d);
    let damping: Vec<f64> = edges.iter().map(|e| BOTTOM_FRICTION / e.depth + BACKGROUND_DAMPING).collect();

    // Marée d'équilibre : K cos²φ (cos ωt cos 2λ − sin ωt sin 2λ)
    let (eq_cos, eq_sin): (Vec<f64>, Vec<f64>) = (0..n).map(|i| (M2_EQUILIBRIUM_M * weight[i] * (2.0 * lon[i]).cos(), M2_EQUILIBRIUM_M * weight[i] * (2.0 * lon[i]).sin())).unzip();
    let f_edge: Vec<f64> = edges.iter().map(|e| (coriolis[e.a] + coriolis[e.b]) / 2.0).collect();
    let cells: Vec<usize> = (0..n).filter(|&i| !around[i].is_empty()).collect();
    let mut eta = vec![0.0f64; n];
    let mut u = vec![0.0f64; edges.len()];
    let mut cell_u = vec![(0.0f64, 0.0f64); n];
    let (mut cos_sum, mut sin_sum) = (vec![0.0f64; n], vec![0.0f64; n]);
    let mut current = vec![0.0f64; n];
    let total = per_tide * (SPIN_UP_TIDES + 1);
    for s in 0..total {
        let t = s as f64 * dt;
        let (c, sn) = ((M2_OMEGA * t).cos(), (M2_OMEGA * t).sin());
        let forcing = |i: usize| eq_cos[i] * c - eq_sin[i] * sn;
        // Vitesse de chaque case, reconstituée de ses arêtes (pour la force de Coriolis) : la
        // vitesse d'une arête porte sur sa direction, quelle que soit la case vue (une case
        // entourée de mer : six arêtes, Σ n nᵀ = 3 I)
        let recon: Vec<(f64, f64)> = cells
            .par_iter()
            .map(|&i| {
                let (mut vx, mut vy) = (0.0, 0.0);
                for &(k, _) in &around[i] {
                    vx += u[k] * edges[k].n.0;
                    vy += u[k] * edges[k].n.1;
                }
                (vx / 3.0, vy / 3.0)
            })
            .collect();
        for (&i, v) in cells.iter().zip(recon) {
            cell_u[i] = v;
        }
        // Vitesses : pente de la surface (moins celle de la marée d'équilibre), Coriolis, frottement
        u.par_iter_mut().enumerate().for_each(|(k, uk)| {
            let e = &edges[k];
            let slope = ((eta[e.b] - forcing(e.b)) - (eta[e.a] - forcing(e.a))) / d;
            let (ux, uy) = ((cell_u[e.a].0 + cell_u[e.b].0) / 2.0, (cell_u[e.a].1 + cell_u[e.b].1) / 2.0);
            // (composante tangente : n tourné d'un quart de tour vers la gauche)
            let tangent = -ux * e.n.1 + uy * e.n.0;
            *uk = (*uk + dt * (-G * slope + f_edge[k] * tangent)) / (1.0 + dt * damping[k]);
        });
        // Hauteurs : l'eau qui entre moins celle qui sort
        let change: Vec<f64> = cells.par_iter().map(|&i| around[i].iter().map(|&(k, sign)| edges[k].depth * u[k] * sign).sum::<f64>()).collect();
        for (&i, out) in cells.iter().zip(change) {
            eta[i] -= dt * flux_factor * out;
        }
        // Mesure sur la dernière marée
        if s >= per_tide * SPIN_UP_TIDES {
            let phase = M2_OMEGA * (t + dt);
            for &i in &cells {
                cos_sum[i] += eta[i] * phase.cos();
                sin_sum[i] += eta[i] * phase.sin();
                current[i] = current[i].max(cell_u[i].0.hypot(cell_u[i].1));
            }
        }
    }
    let samples = per_tide as f64;
    let amplitude: Vec<f32> = (0..n).map(|i| if sea[i] { (2.0 * cos_sum[i].hypot(sin_sum[i]) / samples) as f32 } else { 0.0 }).collect();
    let high_water_h: Vec<f32> = (0..n)
        .map(|i| {
            if !sea[i] {
                return 0.0;
            }
            // η ≈ A cos(ωt − φ) : pleine mer quand ωt = φ
            let phi = sin_sum[i].atan2(cos_sum[i]).rem_euclid(std::f64::consts::TAU);
            (phi / M2_OMEGA / 3_600.0) as f32
        })
        .collect();
    let current: Vec<f32> = (0..n).map(|i| if sea[i] { (current[i] * (1.0 + S2_RATIO as f64)) as f32 } else { 0.0 }).collect();
    Tides { amplitude, high_water_h, current }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Carte d'essai : `land(col, row)` dit les terres, `depth(col, row)` la profondeur des mers.
    fn world(width: i32, height: i32, land: impl Fn(i32, i32) -> bool, depth: impl Fn(i32, i32) -> f32) -> (Vec<bool>, Vec<f32>, Region) {
        let mut sea = Vec::new();
        let mut d = Vec::new();
        for row in 0..height {
            for col in 0..width {
                sea.push(!land(col, row));
                d.push(depth(col, row));
            }
        }
        (sea, d, Region { cell_km: 40.0, center_lat: 0.0, degrees_per_row: 1.6, wraps: true })
    }

    #[test]
    fn marees_d_un_ocean_ordre_de_grandeur() {
        // Océan de 4 000 m, large de 7 000 km, bordé d'un continent ; pôles en bordure
        let (w, h) = (192, 70);
        let (sea, depth, region) = world(w, h, |c, r| c < 15 || r == 0 || r == h - 1, |_, _| 4_000.0);
        let tides = simulate(w, h, &sea, &depth, &region);
        let ocean: Vec<f32> = (0..sea.len()).filter(|&i| sea[i]).map(|i| tides.spring_range(i)).collect();
        let mean = ocean.iter().sum::<f32>() / ocean.len() as f32;
        let max = ocean.iter().copied().fold(0.0, f32::max);
        assert!(ocean.iter().all(|a| a.is_finite()));
        // (sur Terre : un mètre environ en plein océan, quelques mètres au plus)
        assert!(mean > 0.3 && mean < 2.0 && max < 6.0, "moyenne {mean} m, au plus {max} m");
        // Des points amphidromiques : des cases presque sans marée
        assert!(ocean.iter().any(|&a| a < 0.2 * mean), "pas de point amphidromique");
    }

    #[test]
    fn mer_fermee_presque_sans_maree() {
        // Grand océan ; une mer (24 × 18 cases, 3 000 m) fermée par une terre, reliée par un
        // détroit d'une case de large et de 80 m de fond (comme Gibraltar, le détroit ne laisse
        // pas passer assez d'eau pour que la mer suive l'océan)
        let (w, h) = (192, 70);
        let inner = |c: i32, r: i32| (100..124).contains(&c) && (26..44).contains(&r);
        let strait = |c: i32, r: i32| (97..100).contains(&c) && r == 35;
        let wall = |c: i32, r: i32| (97..127).contains(&c) && (23..47).contains(&r) && !inner(c, r) && !strait(c, r);
        let (sea, depth, region) = world(w, h, |c, r| c < 15 || wall(c, r) || r == 0 || r == h - 1, |c, r| if strait(c, r) { 80.0 } else { 3_000.0 });
        let tides = simulate(w, h, &sea, &depth, &region);
        let mean = |f: &dyn Fn(i32, i32) -> bool| {
            let cells: Vec<usize> = (0..sea.len()).filter(|&i| sea[i] && f(i as i32 % w, i as i32 / w)).collect();
            cells.iter().map(|&i| tides.amplitude[i]).sum::<f32>() / cells.len() as f32
        };
        let (open, closed) = (mean(&|c, r| !(90..135).contains(&c) || !(15..55).contains(&r)), mean(&inner));
        assert!(closed < 0.4 * open, "mer fermée {closed} m, océan {open} m");
    }

    #[test]
    fn plateau_continental_amplifie_la_maree() {
        // Même océan ; la côte est bordée d'un plateau de 60 m sur six cases, la côte ouest tombe
        // à pic
        let (w, h) = (120, 60);
        let (sea, depth, region) = world(w, h, |c, r| !(10..=100).contains(&c) || r == 0 || r == h - 1, |c, _| if c > 94 { 60.0 } else { 4_000.0 });
        let tides = simulate(w, h, &sea, &depth, &region);
        let coast = |c: i32| (5..55).map(|r| tides.amplitude[(r * w + c) as usize]).sum::<f32>() / 50.0;
        assert!(coast(100) > 1.5 * coast(10), "plateau {} m, côte à pic {} m", coast(100), coast(10));
        assert!(tides.current[(30 * w + 97) as usize] > tides.current[(30 * w + 50) as usize], "courant de marée plus fort sur le plateau");
    }

    #[test]
    fn lignes_d_heure_autour_d_un_point_amphidromique() {
        // Heure de la pleine mer qui tourne autour d'un point (angle) : les lignes en partent comme
        // des rayons, un seul point amphidromique au centre
        let (w, h) = (30, 30);
        let center = Hex::from_offset(15, 15).to_world();
        let center = Vec2::new(center.0, center.1) + Vec2::new(7.0, 5.0);
        let mut hours = Vec::new();
        for r in 0..h {
            for c in 0..w {
                let (x, y) = Hex::from_offset(c, r).to_world();
                let a = (Vec2::new(x, y) - center).to_angle().rem_euclid(std::f32::consts::TAU);
                hours.push(a / std::f32::consts::TAU * M2_PERIOD_H);
            }
        }
        let sea = vec![true; (w * h) as usize];
        let (lines, amphidromes) = cotidal_lines(w, h, false, &hours, &sea, |_| true);
        assert_eq!(amphidromes.len(), 1, "{amphidromes:?}");
        assert!(amphidromes[0].distance(center) < 40.0);
        // Chaque heure a sa ligne, dans la bonne direction depuis le centre
        for hour in 0..12u8 {
            let mine: Vec<&(u8, Vec2, Vec2)> = lines.iter().filter(|l| l.0 == hour).collect();
            assert!(mine.len() >= 5, "heure {hour} : {} morceaux", mine.len());
            let expected = hour as f32 / M2_PERIOD_H * std::f32::consts::TAU;
            for l in mine {
                let a = (l.1 - center).to_angle().rem_euclid(std::f32::consts::TAU);
                let gap = (a - expected).abs().min(std::f32::consts::TAU - (a - expected).abs());
                assert!(gap < 0.35 || l.1.distance(center) < 60.0, "heure {hour} à {a} rad au lieu de {expected}");
            }
        }
        // Rien sur la terre ni hors de l'exploré
        let (none, _) = cotidal_lines(w, h, false, &hours, &sea, |_| false);
        assert!(none.is_empty());
        // Flèches : la marée tourne autour du point (dans le sens où l'heure augmente)
        let amplitude = vec![1.0; (w * h) as usize];
        let arrows = tide_arrows(w, h, false, &hours, &amplitude, &sea, 20.0, |_| true);
        assert!(arrows.len() >= 20);
        for (a, b) in &arrows {
            let (mid, dir) = ((*a + *b) / 2.0, *b - *a);
            if mid.distance(center) < 80.0 {
                continue;
            }
            // (heure croissante avec l'angle : sens inverse des aiguilles d'une montre)
            assert!((mid - center).perp().dot(dir) > 0.0, "flèche à contresens en {mid}");
        }
    }

    #[test]
    fn meme_monde_meme_maree() {
        let (sea, depth, region) = world(60, 40, |c, r| c < 5 || r == 0 || r == 39, |c, _| 500.0 + 40.0 * c as f32);
        let (a, b) = (simulate(60, 40, &sea, &depth, &region), simulate(60, 40, &sea, &depth, &region));
        assert_eq!(a.amplitude, b.amplitude);
        assert!(a.amplitude.iter().zip(&sea).all(|(&x, &s)| s || x == 0.0));
    }
}
