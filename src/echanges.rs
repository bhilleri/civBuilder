//! Échanges de ressources entre les villes d'un même peuple (étape 5 ; décisions de Benjamin du
//! 13/11/2026, `docs/conception/conception-ressources.md`, « Échanges »).
//!
//! Logique pure, à partir de la demande : une ville qui manque d'une ressource (moins de
//! `TOURS_VISES` tours de sa demande en stock) la reçoit d'une ville reliée qui en a au-delà de sa
//! réserve (`TOURS_RESERVE` tours). La ville qui donne tente d'obtenir en échange, une unité pour
//! une, ce qui lui manque et que l'autre a en trop ; sinon, elle donne. La distance est un **temps
//! de trajet** (décision de Benjamin, 13/11/2026) : celui d'un porteur à pied (`PM_PORTEUR` points
//! de mouvement par tour, chemin le plus rapide avec des coûts de sentier, `Porteur`) ; deux villes sont
//! reliées à `TOURS_TRAJET_MAX` tours au plus, et de proche en proche : tout un réseau partage (le
//! trajet entre deux villes du réseau est la somme des étapes). La ville qui manque le plus est
//! servie d'abord, par la ville la plus proche. L'échange arrive dans le tour, mais perd une part
//! par tour de trajet selon la ressource (`perte_par_tour`). Pas encore de bateaux.

use crate::gisements::{N_RESSOURCES, Ressource};
use crate::hex::Hex;

/// Points de mouvement d'un porteur à pied par tour (ceux d'un colon).
pub const PM_PORTEUR: u32 = 4;
/// Trajet au plus d'une liaison entre deux villes (tours).
pub const TOURS_TRAJET_MAX: f64 = 3.0;
/// Distance au-delà de laquelle aucun trajet ne tient en `TOURS_TRAJET_MAX` tours (cases ; une
/// case coûte au moins 1 point).
pub const CASES_MAX: i32 = (TOURS_TRAJET_MAX as i32) * PM_PORTEUR as i32;
/// Une ville demande une ressource tant qu'elle en a moins que ce nombre de tours de sa demande.
pub const TOURS_VISES: f64 = 2.0;
/// Réserve qu'une ville garde (tours de sa demande) ; elle n'échange que ce qui dépasse.
pub const TOURS_RESERVE: f64 = 6.0;
/// Au-delà de ce nombre de tours de sa demande, une ville perd ce qu'elle produit en trop.
pub const TOURS_PLAFOND: f64 = 12.0;
/// Stock au plus d'une ressource que la ville ne consomme pas encore (bois d'œuvre…).
pub const PLAFOND_SANS_DEMANDE: f64 = 20.0;

/// Part d'une ressource perdue par tour de trajet (mangée, cassée, gâtée en route) : rien pour
/// la pierre et le sel, 5 % pour les fibres et le bois, 10 % pour les plantes médicinales
/// (décision de Benjamin, 13/11/2026).
pub fn perte_par_tour(r: Ressource) -> f64 {
    match r {
        Ressource::Sel | Ressource::Silex | Ressource::Obsidienne | Ressource::PierreDure | Ressource::Ocre | Ressource::Parures => 0.0,
        Ressource::Fibres | Ressource::BoisOeuvre => 0.05,
        Ressource::Plantes => 0.10,
    }
}

/// Part d'une ressource qui arrive après `tours` tours de trajet.
pub fn part_arrivee(r: Ressource, tours: f64) -> f64 {
    (1.0 - perte_par_tour(r) * tours).clamp(0.1, 1.0)
}

/// Trajet d'un porteur à pied entre deux centres (tours), s'il tient en `TOURS_TRAJET_MAX` tours.
pub fn tours_de_trajet(map: &impl crate::map::MoveCosts, a: Hex, b: Hex) -> Option<f64> {
    if map.wrap().distance(a, b) > CASES_MAX {
        return None;
    }
    trajet_a_pied(map, a, b).filter(|t| *t <= TOURS_TRAJET_MAX + 1e-9)
}

/// Trajet d'un porteur à pied entre deux cases (tours), quel qu'il soit (aucun : pas de chemin).
pub fn trajet_a_pied(map: &impl crate::map::MoveCosts, a: Hex, b: Hex) -> Option<f64> {
    let porteur = Porteur(map);
    let chemin = crate::pathfinding::find_path(&porteur, a, b)?;
    Some(crate::rules::path_cost(&porteur, a, &chemin) as f64 / DEMI_POINTS as f64 / PM_PORTEUR as f64)
}

/// Les coûts d'un pas comptent en demi-points pour le porteur.
const DEMI_POINTS: u32 = 2;

/// Coûts d'un porteur à pied (décision de Benjamin, 14/11/2026 : des sentiers, pas les coûts de
/// l'armée) : le terrain et le relief comptent la moitié de ceux des unités, 1 point au moins
/// (plaine 1, forêt 2, marais 3, colline 1,5, montagne 4 à 8) ; une rivière se passe pour 1 point
/// de plus, un fleuve pour 2 (gué, bac). En demi-points, pour rester entier.
pub struct Porteur<M>(pub M);

impl<M: crate::map::MoveCosts> crate::map::MoveCosts for Porteur<M> {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        self.0.move_cost(hex).map(|c| c.max(DEMI_POINTS))
    }

    fn step(&self, from: Hex, to: Hex) -> Option<crate::map::StepCost> {
        let s = self.0.step(from, to)?;
        let eau = match s.water {
            0 => 0,
            1..=4 => DEMI_POINTS,
            _ => 2 * DEMI_POINTS,
        };
        // (un fleuve n'a que de l'eau : pas de terrain en plus)
        let terrain = if s.terrain == 0 && eau > 0 { 0 } else { s.terrain.max(DEMI_POINTS) };
        Some(crate::map::StepCost { terrain, water: eau, ..s })
    }

    fn wrap(&self) -> crate::hex::Wrap {
        self.0.wrap()
    }
}

/// Trajets entre les villes d'un réseau, de proche en proche (la somme des étapes la plus courte) ;
/// `direct(a, b)` : la liaison directe, si elle existe. Aucun : pas reliées.
#[allow(clippy::needless_range_loop)]
pub fn liaisons(n: usize, direct: impl Fn(usize, usize) -> Option<f64>) -> Vec<Vec<Option<f64>>> {
    let mut t = vec![vec![None; n]; n];
    for a in 0..n {
        t[a][a] = Some(0.0);
        for b in a + 1..n {
            let d = direct(a, b);
            t[a][b] = d;
            t[b][a] = d;
        }
    }
    for k in 0..n {
        for a in 0..n {
            for b in 0..n {
                if let (Some(x), Some(y)) = (t[a][k], t[k][b])
                    && t[a][b].is_none_or(|d| x + y < d - 1e-12)
                {
                    t[a][b] = Some(x + y);
                }
            }
        }
    }
    t
}

/// Ce que l'échange sait d'une ville.
#[derive(Clone, Debug)]
pub struct VilleEchange {
    pub centre: Hex,
    pub stock: [f64; N_RESSOURCES],
    /// Demande par tour de chaque ressource (les outils : sur le silex et la pierre dure).
    pub demande: [f64; N_RESSOURCES],
}

impl VilleEchange {
    /// Ce qui lui manque pour avoir `TOURS_VISES` tours de sa demande.
    pub fn manque(&self, r: usize) -> f64 {
        (TOURS_VISES * self.demande[r] - self.stock[r]).max(0.0)
    }

    /// Ce qu'elle a au-delà de sa réserve.
    pub fn surplus(&self, r: usize) -> f64 {
        (self.stock[r] - TOURS_RESERVE * self.demande[r]).max(0.0)
    }
}

/// Un transfert du tour : `qte` de `ressource` envoyée de la ville `de` à la ville `vers` (dont
/// `perdu` en route, après `tours` tours de trajet), et la contrepartie envoyée par `vers`, s'il y en
/// a une (sinon, un don).
#[derive(Clone, Debug, PartialEq)]
pub struct Transfert {
    pub de: usize,
    pub vers: usize,
    pub ressource: Ressource,
    pub qte: f64,
    pub perdu: f64,
    pub tours: f64,
    pub contre: Option<(Ressource, f64)>,
}

/// Les échanges du tour, entre les villes reliées (`trajets`, de `liaisons`) : modifie les stocks
/// et rend les transferts.
pub fn echanger(villes: &mut [VilleEchange], trajets: &[Vec<Option<f64>>]) -> Vec<Transfert> {
    let mut transferts = Vec::new();
    for r in 0..N_RESSOURCES {
        // La ville qui manque le plus (en tours de sa demande) d'abord
        let mut demandeuses: Vec<usize> = (0..villes.len()).filter(|&v| villes[v].manque(r) > 1e-9).collect();
        demandeuses.sort_by(|&a, &b| {
            let tours = |v: usize| villes[v].stock[r] / villes[v].demande[r].max(1e-9);
            tours(a).total_cmp(&tours(b)).then(a.cmp(&b))
        });
        for vers in demandeuses {
            let mut donneuses: Vec<(usize, f64)> = (0..villes.len()).filter(|&d| d != vers && villes[d].surplus(r) > 1e-9).filter_map(|d| trajets[d][vers].map(|t| (d, t))).collect();
            donneuses.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            for (de, tours) in donneuses {
                // (on envoie de quoi combler le manque une fois la perte du trajet faite)
                let garde = part_arrivee(Ressource::TOUTES[r], tours);
                let q = (villes[vers].manque(r) / garde).min(villes[de].surplus(r));
                if q <= 1e-9 {
                    continue;
                }
                villes[de].stock[r] -= q;
                villes[vers].stock[r] += q * garde;
                // Contrepartie (une unité envoyée pour une) : ce qui manque à la donneuse et que la
                // demandeuse a en trop ; elle perd aussi en route
                let contre = (0..N_RESSOURCES).filter(|&s| s != r).find_map(|s| {
                    let g = part_arrivee(Ressource::TOUTES[s], tours);
                    let c = q.min(villes[de].manque(s) / g).min(villes[vers].surplus(s));
                    (c > 1e-9).then_some((s, c, g))
                });
                if let Some((s, c, g)) = contre {
                    villes[vers].stock[s] -= c;
                    villes[de].stock[s] += c * g;
                }
                transferts.push(Transfert { de, vers, ressource: Ressource::TOUTES[r], qte: q, perdu: q * (1.0 - garde), tours, contre: contre.map(|(s, c, _)| (Ressource::TOUTES[s], c)) });
                if villes[vers].manque(r) <= 1e-9 {
                    break;
                }
            }
        }
    }
    transferts
}

/// Plafond du stock d'une ressource : `TOURS_PLAFOND` tours de la demande, ou
/// `PLAFOND_SANS_DEMANDE` pour une ressource que la ville ne consomme pas.
pub fn plafond(demande: f64) -> f64 {
    if demande > 0.0 { (TOURS_PLAFOND * demande).max(1.0) } else { PLAFOND_SANS_DEMANDE }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ville(col: i32) -> VilleEchange {
        VilleEchange { centre: Hex::from_offset(col, 0), stock: [0.0; N_RESSOURCES], demande: [0.0; N_RESSOURCES] }
    }

    /// Trajets en plaine (1 point par case) : reliées à 12 cases au plus.
    fn trajets(v: &[VilleEchange]) -> Vec<Vec<Option<f64>>> {
        liaisons(v.len(), |a, b| {
            let t = v[a].centre.distance(v[b].centre) as f64 / PM_PORTEUR as f64;
            (t <= TOURS_TRAJET_MAX).then_some(t)
        })
    }

    #[test]
    fn la_demande_trouve_le_surplus_par_troc_ou_par_don() {
        let (sel, ocre) = (Ressource::Sel.index(), Ressource::Ocre.index());
        // A (0) a du sel en trop et manque d'ocre ; B (5) manque de sel et a de l'ocre en trop ;
        // C (11) manque de sel, n'a rien à offrir ; D (30) est trop loin
        let mut v = vec![ville(0), ville(5), ville(11), ville(30)];
        v[0].demande[sel] = 0.1;
        v[0].stock[sel] = 5.0;
        v[0].demande[ocre] = 0.2;
        v[1].demande[sel] = 0.5;
        v[1].stock[ocre] = 3.0;
        v[2].demande[sel] = 0.3;
        v[3].demande[sel] = 1.0;
        let tr = trajets(&v);
        assert_eq!(tr[0][1], Some(1.25));
        assert_eq!(tr[0][2], Some(2.75), "de proche en proche : C est reliée à A par B (5 + 6 cases)");
        assert_eq!(tr[0][3], None);
        let t = echanger(&mut v, &tr);
        // B (0 tour de sel) et C (0) : B d'abord (indice), chacune reçoit 2 tours de sa demande
        assert!((v[1].stock[sel] - 1.0).abs() < 1e-9 && (v[2].stock[sel] - 0.6).abs() < 1e-9);
        // B paie en ocre (A en manquait : 2 tours de 0,2 = 0,4) ; C reçoit en don
        let tb = t.iter().find(|x| x.vers == 1).unwrap();
        assert_eq!(tb.contre.map(|(r, _)| r), Some(Ressource::Ocre));
        assert!((v[0].stock[ocre] - 0.4).abs() < 1e-9);
        assert!(t.iter().find(|x| x.vers == 2).unwrap().contre.is_none(), "rien à offrir : un don");
        // A garde sa réserve (6 tours de 0,1) ; D, trop loin, ne reçoit rien
        assert!(v[0].stock[sel] >= TOURS_RESERVE * 0.1 - 1e-9);
        assert_eq!(v[3].stock[sel], 0.0);
        assert!(t.iter().all(|x| x.perdu == 0.0), "le sel et l'ocre ne se perdent pas en route");
        assert_eq!(plafond(0.5), 6.0);
        assert_eq!(plafond(0.0), PLAFOND_SANS_DEMANDE);
    }

    /// Bande de cases sur une ligne : le coût de chaque colonne (forêt 4…), une rivière à franchir
    /// avant la colonne `riviere`.
    struct Bande {
        couts: Vec<u32>,
        riviere: i32,
    }

    impl crate::map::MoveCosts for Bande {
        fn move_cost(&self, hex: Hex) -> Option<u32> {
            let (c, l) = hex.to_offset();
            (l == 0 && c >= 0).then(|| self.couts.get(c as usize).copied()).flatten()
        }

        fn step(&self, from: Hex, to: Hex) -> Option<crate::map::StepCost> {
            let terrain = self.move_cost(to)?;
            let water = if to.to_offset().0 == self.riviere && from.to_offset().0 == self.riviere - 1 { crate::map::RIVER_CROSSING_COST } else { 0 };
            Some(crate::map::StepCost { terrain, water, ..Default::default() })
        }
    }

    #[test]
    fn le_porteur_suit_les_sentiers() {
        // Quatre cases de forêt (4 PM chacune pour une unité) puis une plaine, une rivière avant la
        // dernière : 4 × 2 + 1 + 1 = 10 points, 2,5 tours (une unité y mettrait 4 × 4 + 1 + 4 = 21 PM)
        let bande = Bande { couts: vec![1, 4, 4, 4, 4, 1], riviere: 5 };
        let (a, b) = (Hex::from_offset(0, 0), Hex::from_offset(5, 0));
        assert_eq!(trajet_a_pied(&bande, a, b), Some(2.5));
        assert_eq!(tours_de_trajet(&bande, a, b), Some(2.5), "3 tours au plus : reliées");
        // Six cases de marais (3 points chacune) : 4,5 tours, pas reliées
        let marais = Bande { couts: vec![1, 6, 6, 6, 6, 6, 6], riviere: 99 };
        assert_eq!(tours_de_trajet(&marais, a, Hex::from_offset(6, 0)), None);
        // En plaine : 5 cases, 1,25 tour
        let plaine = Bande { couts: vec![1; 6], riviere: 99 };
        assert_eq!(tours_de_trajet(&plaine, a, b), Some(1.25));
    }

    #[test]
    fn les_plantes_se_perdent_en_route() {
        let p = Ressource::Plantes.index();
        // B, à 6 cases (1,5 tour), manque de plantes : A envoie de quoi combler, 15 % se perd
        let mut v = vec![ville(0), ville(6)];
        v[0].stock[p] = 10.0;
        v[1].demande[p] = 1.0;
        let tr = trajets(&v);
        let t = echanger(&mut v, &tr);
        assert!((v[1].stock[p] - 2.0).abs() < 1e-9, "B reçoit ses 2 tours de demande");
        assert!((t[0].qte - 2.0 / 0.85).abs() < 1e-9 && (t[0].perdu - t[0].qte * 0.15).abs() < 1e-9);
        assert!((v[0].stock[p] - (10.0 - 2.0 / 0.85)).abs() < 1e-9);
        assert_eq!(perte_par_tour(Ressource::Sel), 0.0);
        assert_eq!(part_arrivee(Ressource::BoisOeuvre, 2.0), 0.9);
    }
}
