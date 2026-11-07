//! Érosion (logique pure) : l'eau creuse le relief tectonique avant qu'on y fasse couler les
//! vrais cours d'eau, puis les vrais cours d'eau le creusent après le climat (`incise`).
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

/// Relief des vallées, avant la première érosion : une longue crête aux versants lisses ne rassemble
/// pas l'eau, et chaque rangée de cases garde sa rivière (un peigne). Le bruit (`noise`, de −1 à 1,
/// à l'échelle de quelques cases) y creuse des vallées et relève d'autant les crêtes voisines :
/// l'altitude moyenne ne change pas, et l'érosion approfondira ces vallées. Altitude relative `e`
/// (0 : niveau de la mer, 0,38 : 1 200 m environ) à partir de laquelle ça commence et où c'est
/// complet ; part de l'altitude déplacée au plus.
const VALLEY_FROM: f64 = 0.14;
const VALLEY_FULL: f64 = 0.38;
pub const VALLEY_DEPTH: f64 = 0.35;
/// Échelle du bruit des vallées (12 : une alternance vallée-crête tous les 4 cases environ).
pub const VALLEY_ZOOM: f64 = 12.0;

/// Altitude relative après le relief des vallées. La mer et les basses terres ne changent pas ; une
/// case de terre reste de la terre.
pub fn with_valleys(e: f64, noise: f64) -> f64 {
    if e <= VALLEY_FROM {
        return e;
    }
    let t = ((e - VALLEY_FROM) / (VALLEY_FULL - VALLEY_FROM)).clamp(0.0, 1.0);
    let highland = t * t * (3.0 - 2.0 * t);
    let valley = (-noise * 1.5).clamp(-1.0, 1.0);
    e * (1.0 + VALLEY_DEPTH * highland * valley)
}

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
/// case) ; `area[i]` : surface relative de la case ; `outlet[i]` : case de terre par où l'eau
/// quitte la carte (bord d'une planète, aux pôles : la terre continue au-delà).
pub fn erode(elevation: &mut [f64], neighbors: &[Vec<(usize, f32)>], area: &[f32], outlet: &[bool], params: ErosionParams) -> ErosionReport {
    let n = elevation.len();
    let original = elevation.to_vec();
    let is_land: Vec<bool> = elevation.iter().map(|&e| e >= 0.0).collect();
    let mut report = ErosionReport::default();
    for pass in 0..params.passes {
        let flow = route(elevation, neighbors, &is_land, outlet);
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
    let flow = route(elevation, neighbors, &is_land, outlet);
    report.pit_cells_after = (0..n).filter(|&i| is_land[i] && flow.filled[i] > elevation[i] + 1e-6).count();
    let lowering: Vec<f64> = (0..n).filter(|&i| is_land[i]).map(|i| original[i] - elevation[i]).collect();
    report.mean_lowering = lowering.iter().sum::<f64>() / lowering.len().max(1) as f64;
    report.max_lowering = lowering.iter().copied().fold(0.0, f64::max);
    report
}

/// Réglages du creusement par les vrais cours d'eau, après le climat.
#[derive(Clone, Copy, Debug)]
pub struct IncisionParams {
    pub passes: usize,
    /// Coefficient par passe, pour un débit de `reference_m3s`, et exposant du débit.
    pub stream: f32,
    pub reference_m3s: f32,
    pub exponent: f32,
    /// Débit (m³/s) sous lequel une case ne creuse pas.
    pub min_m3s: f32,
    /// Altitude (m) minimale d'une case de terre.
    pub floor_m: f32,
}

impl Default for IncisionParams {
    fn default() -> Self {
        // (référence : le débit d'une case sous 1 000 mm/an, qui vaut une case de surface drainée)
        Self { passes: 10, stream: 0.02, reference_m3s: 44.0, exponent: 0.5, min_m3s: 100.0, floor_m: 1.0 }
    }
}

/// Renfort du creusement à l'embouchure, selon le bilan de sédiments : là où la houle et la marée
/// emportent plus que le fleuve n'apporte, rien ne se dépose et la vallée reste creusée (au plus
/// `MOUTH_BOOST_MAX` fois). `MOUTH_REACH` : part du renfort sur l'embouchure et les cases d'amont ;
/// la houle attaque la côte et la marée remonte le bas du fleuve, pas plus loin.
const MOUTH_BOOST_MAX: f32 = 4.0;
pub const MOUTH_REACH: [f32; 3] = [1.0, 0.6, 0.3];

/// Coefficient du creusement à une embouchure, d'après les flux de sédiments (kg/s) du fleuve, de
/// la houle et de la marée, à la part `reach` de la portée.
pub fn mouth_boost(river: f32, wave: f32, tide: f32, reach: f32) -> f32 {
    let full = ((wave + tide) / river.max(1.0)).sqrt().clamp(1.0, MOUTH_BOOST_MAX);
    1.0 + (full - 1.0) * reach
}

/// Creuse les cases selon leur débit réel : chacune descend vers sa case aval (`receiver`), avec la
/// même loi que `erode`, mais le débit remplace la surface drainée : un petit bassin très arrosé
/// creuse autant qu'il porte d'eau, une montagne sèche ne bouge pas. `elevation` : altitude (m) de
/// la surface, 0 en mer ; `fixed` : cases qui ne changent pas (mer, lacs et leurs sorties) ; `boost` :
/// coefficient du creusement de chaque case (`mouth_boost`). Une case ne descend jamais sous sa case
/// aval. Rend les nouvelles altitudes.
pub fn incise(elevation: &[f32], receiver: &[Option<usize>], discharge: &[f32], fixed: &[bool], boost: &[f32], params: IncisionParams) -> Vec<f32> {
    let n = elevation.len();
    // Ordre de l'aval vers l'amont (une boucle éventuelle est simplement laissée de côté)
    let mut upstream = vec![0u32; n];
    for r in receiver.iter().flatten() {
        upstream[*r] += 1;
    }
    let mut order: Vec<usize> = (0..n).filter(|&i| upstream[i] == 0).collect();
    let mut k = 0;
    while k < order.len() {
        if let Some(r) = receiver[order[k]] {
            upstream[r] -= 1;
            if upstream[r] == 0 {
                order.push(r);
            }
        }
        k += 1;
    }
    order.reverse();
    let mut z = elevation.to_vec();
    for _ in 0..params.passes {
        for &i in &order {
            let Some(r) = receiver[i] else { continue };
            if fixed[i] || discharge[i] < params.min_m3s || z[i] <= z[r] {
                continue;
            }
            let f = boost[i] * params.stream * (discharge[i] / params.reference_m3s).powf(params.exponent);
            z[i] = ((z[i] + f * z[r]) / (1.0 + f)).max(z[r]).max(params.floor_m);
        }
    }
    z
}

/// Écoulement sur la surface remplie : niveau de remplissage, case aval et ordre de traitement
/// (de la mer vers l'amont).
struct Flow {
    filled: Vec<f64>,
    receiver: Vec<Option<usize>>,
    order: Vec<usize>,
}

/// Priority-Flood depuis la mer et le bord de la carte : chaque case de terre est atteinte par sa
/// case aval, à une
/// hauteur au moins égale à celle-ci (pente minimale imposée dans les cuvettes et les plats).
fn route(elevation: &[f64], neighbors: &[Vec<(usize, f32)>], is_land: &[bool], outlet: &[bool]) -> Flow {
    const EPSILON: f64 = 1e-5;
    let n = elevation.len();
    let key = |h: f64| (h * 1e9) as i64;
    let mut filled = elevation.to_vec();
    let mut receiver = vec![None; n];
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut queue: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
    // Exutoires : la mer, et le bord de la carte d'une planète
    for i in 0..n {
        if !is_land[i] || outlet.get(i).copied().unwrap_or(false) {
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
        let r = erode(&mut e, &neighbors, &area, &[], ErosionParams::default());
        assert!(r.pit_cells_before >= 3);
        assert!(r.pit_cells_after <= r.pit_cells_before, "{r:?}");
        assert!(depth(&e) < before * 0.7, "cuvette moins profonde : {} contre {before}", depth(&e));
        // La mer n'a pas bougé ; la terre reste de la terre
        assert!((0..h).all(|y| e[y * w] == -0.5));
        assert!((0..w * h).filter(|&i| i % w != 0).all(|i| e[i] > 0.0));
    }

    #[test]
    fn le_debit_reel_creuse_et_le_profil_reste_descendant() {
        // Ligne amont → mer : cases 0 à 3, la 4 est la mer
        let e = vec![2_000.0, 1_500.0, 1_200.0, 1_000.0, 0.0];
        let receiver = vec![Some(1), Some(2), Some(3), Some(4), None];
        let fixed = vec![false, false, false, false, true];
        let z = incise(&e, &receiver, &[3_000.0; 5], &fixed, &[1.0; 5], IncisionParams::default());
        assert!(z[3] < 300.0, "l'embouchure d'un fort débit descend : {}", z[3]);
        assert_eq!(z[4], 0.0, "la mer ne bouge pas");
        assert!(z[0] >= z[1] && z[1] >= z[2] && z[2] >= z[3] && z[3] >= 1.0, "{z:?}");
        // Sans débit, rien ne bouge ; un débit plus fort creuse plus
        assert_eq!(incise(&e, &receiver, &[10.0; 5], &fixed, &[1.0; 5], IncisionParams::default()), e);
        let faible = incise(&e, &receiver, &[200.0; 5], &fixed, &[1.0; 5], IncisionParams::default());
        assert!(z[3] < faible[3] && faible[3] < e[3]);
        // Le renfort de l'embouchure creuse davantage
        let renfort = incise(&e, &receiver, &[3_000.0; 5], &fixed, &[1.0, 1.0, 1.0, 4.0, 1.0], IncisionParams::default());
        assert!(renfort[3] < z[3], "{} contre {}", renfort[3], z[3]);
    }

    #[test]
    fn la_mer_renforce_le_creusement_si_le_fleuve_apporte_peu() {
        // Le fleuve l'emporte : pas de renfort ; la houle l'emporte : renfort, borné, qui s'éteint vers l'amont
        assert_eq!(mouth_boost(1_000.0, 100.0, 50.0, 1.0), 1.0);
        assert!((mouth_boost(100.0, 300.0, 100.0, 1.0) - 2.0).abs() < 1e-5);
        assert_eq!(mouth_boost(10.0, 10_000.0, 0.0, 1.0), MOUTH_BOOST_MAX);
        assert!(mouth_boost(100.0, 300.0, 100.0, MOUTH_REACH[2]) < mouth_boost(100.0, 300.0, 100.0, MOUTH_REACH[1]));
    }

    #[test]
    fn les_vallees_ne_touchent_que_les_terres_hautes() {
        // Mer et basses terres : rien ne change
        assert_eq!(with_valleys(-0.3, 1.0), -0.3);
        assert_eq!(with_valleys(0.1, 1.0), 0.1);
        // En montagne : une vallée d'un côté du bruit, une crête autant relevée de l'autre, rien au milieu
        let (valley, ridge) = (with_valleys(0.8, 1.0), with_valleys(0.8, -1.0));
        assert!(valley < 0.8 && ridge > 0.8);
        assert!((valley + ridge - 1.6).abs() < 1e-9, "altitude moyenne gardée");
        assert_eq!(with_valleys(0.8, 0.0), 0.8);
        // Une terre reste une terre
        for e in [0.15, 0.2, 0.4, 1.0] {
            assert!(with_valleys(e, 1.0) > 0.0, "{e}");
        }
    }

    #[test]
    fn une_case_figee_garde_son_altitude() {
        let e = vec![800.0, 500.0, 0.0];
        let z = incise(&e, &[Some(1), Some(2), None], &[5_000.0; 3], &[false, true, true], &[1.0; 3], IncisionParams::default());
        assert_eq!(z[1], 500.0, "le lac ne bouge pas");
        assert!(z[0] < 800.0 && z[0] >= 500.0, "la case amont ne descend pas sous le lac : {}", z[0]);
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
        erode(&mut e, &neighbors, &vec![1.0; w * h], &[], ErosionParams::default());
        let lowered = |x: usize, y: usize| before[y * w + x] - e[y * w + x];
        assert!(lowered(6, 4) > lowered(6, 0) * 1.5, "vallée : {} contre {}", lowered(6, 4), lowered(6, 0));
        // Profil le long de la vallée : toujours descendant vers la mer
        for x in 2..w {
            assert!(e[4 * w + x] >= e[4 * w + x - 1] - 1e-9, "x = {x}");
        }
    }
}
