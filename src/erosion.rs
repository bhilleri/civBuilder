//! Érosion (logique pure) : l'eau creuse le relief tectonique avant qu'on y fasse couler les
//! vrais cours d'eau.
//!
//! À chaque passe :
//! 1. les cuvettes sont remplies jusqu'à leur seuil (Priority-Flood depuis la mer) : chaque case
//!    de terre a une case aval, et l'eau d'une cuvette ressort par son seuil ;
//! 2. on cumule l'aire drainée par chaque case (sa surface plus celle de tout son amont) ;
//! 3. chaque case descend vers sa case aval selon la loi « puissance du courant » : d'autant plus
//!    que l'aire drainée (le débit) et la pente sont grandes. Le calcul est implicite (de l'aval
//!    vers l'amont : une case ne descend jamais sous sa case aval), donc stable même pour les
//!    grands fleuves ; les seuils des cuvettes, traversés par toute l'eau de la cuvette, s'usent ;
//! 4. les fonds de cuvettes reçoivent des sédiments (ils remontent vers le niveau de leur seuil) ;
//! 5. une légère diffusion adoucit les pentes.
//!
//! Effets : vallées le long des grands cours d'eau, profils réguliers (moins de ruptures de pente,
//! donc de cascades), cuvettes vidées ou comblées (moins de lacs), crêtes découpées par les
//! vallées et cols aux points bas des crêtes. Les montagnes, peu drainées, gardent leur hauteur.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Réglages de l'érosion.
#[derive(Clone, Copy, Debug)]
pub struct ErosionParams {
    pub passes: usize,
    /// Coefficient de la loi puissance du courant (par passe) et exposant de l'aire drainée.
    pub stream: f64,
    pub area_exponent: f64,
    /// Part de la profondeur d'une cuvette comblée par passe (sédiments).
    pub infill: f64,
    /// Diffusion des pentes (par passe, 0 à 0,25).
    pub diffusion: f64,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self { passes: 10, stream: 0.02, area_exponent: 0.5, infill: 0.08, diffusion: 0.04 }
    }
}

/// Bilan d'une érosion (diagnostics, tests).
#[derive(Clone, Copy, Debug, Default)]
pub struct ErosionReport {
    /// Cases de terre dans une cuvette (sous le niveau de leur seuil), avant et après.
    pub pit_cells_before: usize,
    pub pit_cells_after: usize,
    /// Abaissement moyen et maximal des terres.
    pub mean_lowering: f64,
    pub max_lowering: f64,
}

/// Érode les altitudes `elevation` (la mer en dessous de 0 n'est pas touchée ; une case de terre
/// reste au-dessus de la mer). `neighbors[i]` : voisines de la case i et distance (en tailles de
/// case) ; `area[i]` : surface relative de la case.
pub fn erode(elevation: &mut [f64], neighbors: &[Vec<(usize, f32)>], area: &[f32], params: ErosionParams) -> ErosionReport {
    let n = elevation.len();
    let original = elevation.to_vec();
    let is_land: Vec<bool> = elevation.iter().map(|&e| e >= 0.0).collect();
    let mut report = ErosionReport::default();
    for pass in 0..params.passes {
        let flow = route(elevation, neighbors, &is_land);
        let pits = (0..n).filter(|&i| is_land[i] && flow.filled[i] > elevation[i] + 1e-6).count();
        if pass == 0 {
            report.pit_cells_before = pits;
        }
        // Aire drainée : de l'amont vers l'aval (ordre inverse du remplissage)
        let mut drained: Vec<f64> = area.iter().map(|&a| f64::from(a)).collect();
        for &i in flow.order.iter().rev() {
            if let Some(r) = flow.receiver[i] {
                drained[r] += drained[i];
            }
        }
        // Creusement implicite, de l'aval vers l'amont, sur la surface remplie (une cuvette est
        // traversée à niveau, son seuil s'use)
        let mut surface = flow.filled.clone();
        for &i in &flow.order {
            let Some(r) = flow.receiver[i] else { continue };
            if !is_land[i] {
                continue;
            }
            let dist = neighbors[i].iter().find(|&&(j, _)| j == r).map_or(1.0, |&(_, d)| f64::from(d).max(0.05));
            let f = params.stream * drained[i].powf(params.area_exponent) / dist;
            let lowered = (surface[i] + f * surface[r]) / (1.0 + f);
            if lowered < surface[i] {
                surface[i] = lowered.max(surface[r]);
            }
        }
        // Report sur le relief : une case hors cuvette prend la surface creusée ; un fond de
        // cuvette se comble en partie (sédiments), sans dépasser le nouveau niveau du seuil
        for i in 0..n {
            if !is_land[i] {
                continue;
            }
            let pit = flow.filled[i] > elevation[i] + 1e-6;
            elevation[i] = if pit {
                let top = surface[i];
                elevation[i] + (top - elevation[i]).max(0.0) * params.infill
            } else {
                surface[i]
            };
        }
        // Diffusion des pentes (terres seulement)
        if params.diffusion > 0.0 {
            let before = elevation.to_vec();
            for i in 0..n {
                if !is_land[i] || neighbors[i].is_empty() {
                    continue;
                }
                let mean = neighbors[i].iter().map(|&(j, _)| before[j].max(0.0)).sum::<f64>() / neighbors[i].len() as f64;
                elevation[i] = before[i] + params.diffusion * (mean - before[i]);
            }
        }
        // Une case de terre reste de la terre
        for i in 0..n {
            if is_land[i] {
                elevation[i] = elevation[i].max(1e-3);
            }
        }
    }
    let flow = route(elevation, neighbors, &is_land);
    report.pit_cells_after = (0..n).filter(|&i| is_land[i] && flow.filled[i] > elevation[i] + 1e-6).count();
    let lowering: Vec<f64> = (0..n).filter(|&i| is_land[i]).map(|i| original[i] - elevation[i]).collect();
    report.mean_lowering = lowering.iter().sum::<f64>() / lowering.len().max(1) as f64;
    report.max_lowering = lowering.iter().copied().fold(0.0, f64::max);
    report
}

/// Écoulement sur la surface remplie : niveau de remplissage, case aval et ordre de traitement
/// (de la mer vers l'amont).
struct Flow {
    filled: Vec<f64>,
    receiver: Vec<Option<usize>>,
    order: Vec<usize>,
}

/// Priority-Flood depuis la mer : chaque case de terre est atteinte par sa case aval, à une
/// hauteur au moins égale à celle-ci (pente minimale imposée dans les cuvettes et les plats).
fn route(elevation: &[f64], neighbors: &[Vec<(usize, f32)>], is_land: &[bool]) -> Flow {
    const EPSILON: f64 = 1e-5;
    let n = elevation.len();
    let key = |h: f64| (h * 1e9) as i64;
    let mut filled = elevation.to_vec();
    let mut receiver = vec![None; n];
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut queue: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
    for i in 0..n {
        if !is_land[i] {
            done[i] = true;
            queue.push(Reverse((key(filled[i]), i)));
        }
    }
    // Pas de mer (tests) : l'eau sort par la case la plus basse
    if queue.is_empty()
        && let Some(i) = (0..n).min_by(|&a, &b| elevation[a].total_cmp(&elevation[b]))
    {
        done[i] = true;
        queue.push(Reverse((key(filled[i]), i)));
    }
    while let Some(Reverse((_, i))) = queue.pop() {
        order.push(i);
        for &(j, _) in &neighbors[i] {
            if done[j] {
                continue;
            }
            done[j] = true;
            receiver[j] = Some(i);
            filled[j] = filled[j].max(filled[i] + EPSILON);
            queue.push(Reverse((key(filled[j]), j)));
        }
    }
    Flow { filled, receiver, order }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grille rectangulaire w × h (4 voisines), distance 1.
    fn grid(w: usize, h: usize) -> Vec<Vec<(usize, f32)>> {
        (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let mut v = Vec::new();
                if x > 0 {
                    v.push((i - 1, 1.0));
                }
                if x + 1 < w {
                    v.push((i + 1, 1.0));
                }
                if y > 0 {
                    v.push((i - w, 1.0));
                }
                if y + 1 < h {
                    v.push((i + w, 1.0));
                }
                v
            })
            .collect()
    }

    #[test]
    fn une_cuvette_se_vide_ou_se_comble() {
        // Pente vers la mer (colonne 0) avec une cuvette au milieu
        let (w, h) = (20, 5);
        let mut e: Vec<f64> = (0..w * h).map(|i| if i % w == 0 { -0.5 } else { (i % w) as f64 * 0.05 }).collect();
        for y in 1..4 {
            e[y * w + 10] = 0.1;
        }
        let neighbors = grid(w, h);
        let area = vec![1.0; w * h];
        // Profondeur de la cuvette : seuil (la plus basse des cases qui l'entourent) moins le fond
        let depth = |e: &[f64]| [9usize, 11].iter().flat_map(|&x| (1..4).map(move |y| y * w + x)).chain([10, 4 * w + 10]).map(|i| e[i]).fold(f64::MAX, f64::min) - e[2 * w + 10];
        let before = depth(&e);
        let r = erode(&mut e, &neighbors, &area, ErosionParams::default());
        assert!(r.pit_cells_before >= 3);
        assert!(r.pit_cells_after <= r.pit_cells_before, "{r:?}");
        assert!(depth(&e) < before * 0.7, "cuvette moins profonde : {} contre {before}", depth(&e));
        // La mer n'a pas bougé ; la terre reste de la terre
        assert!((0..h).all(|y| e[y * w] == -0.5));
        assert!((0..w * h).filter(|&i| i % w != 0).all(|i| e[i] > 0.0));
    }

    #[test]
    fn les_grands_cours_d_eau_creusent_plus() {
        // Plateau incliné : la ligne du milieu draine tout (vallée) ; elle s'abaisse plus que les
        // bords, qui drainent peu
        let (w, h) = (24, 9);
        let mut e: Vec<f64> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                if x == 0 { -0.5 } else { 0.2 + x as f64 * 0.03 + (y as f64 - 4.0).abs() * 0.08 }
            })
            .collect();
        let before = e.clone();
        let neighbors = grid(w, h);
        erode(&mut e, &neighbors, &vec![1.0; w * h], ErosionParams::default());
        let lowered = |x: usize, y: usize| before[y * w + x] - e[y * w + x];
        assert!(lowered(6, 4) > lowered(6, 0) * 1.5, "vallée : {} contre {}", lowered(6, 4), lowered(6, 0));
        // Profil le long de la vallée : toujours descendant vers la mer
        for x in 2..w {
            assert!(e[4 * w + x] >= e[4 * w + x - 1] - 1e-9, "x = {x}");
        }
    }
}
