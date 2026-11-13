//! Échanges de ressources entre les villes d'un même peuple (étape 5 ; décisions de Benjamin du
//! 13/11/2026, `docs/conception/conception-ressources.md`, « Échanges »).
//!
//! Logique pure, à partir de la demande : une ville qui manque d'une ressource (moins de
//! `TOURS_VISES` tours de sa demande en stock) la reçoit d'une ville reliée qui en a au-delà de sa
//! réserve (`TOURS_RESERVE` tours). La ville qui donne tente d'obtenir en échange, une unité pour
//! une, ce qui lui manque et que l'autre a en trop ; sinon, elle donne. Les villes sont reliées à
//! `PORTEE` cases au plus (`PORTEE_EAU` si les deux sont au bord de l'eau), de proche en proche :
//! tout un réseau partage. La ville qui manque le plus est servie d'abord, par la ville la plus
//! proche. Pas de délai ni de perte (un tour dure un mois).

use crate::gisements::{N_RESSOURCES, Ressource};
use crate::hex::Hex;

/// Portée d'une liaison entre deux villes (cases), et le long de l'eau.
pub const PORTEE: i32 = 6;
pub const PORTEE_EAU: i32 = 9;
/// Une ville demande une ressource tant qu'elle en a moins que ce nombre de tours de sa demande.
pub const TOURS_VISES: f64 = 2.0;
/// Réserve qu'une ville garde (tours de sa demande) ; elle n'échange que ce qui dépasse.
pub const TOURS_RESERVE: f64 = 6.0;
/// Au-delà de ce nombre de tours de sa demande, une ville perd ce qu'elle produit en trop.
pub const TOURS_PLAFOND: f64 = 12.0;
/// Stock au plus d'une ressource que la ville ne consomme pas encore (bois d'œuvre…).
pub const PLAFOND_SANS_DEMANDE: f64 = 20.0;

/// Ce que l'échange sait d'une ville.
#[derive(Clone, Debug)]
pub struct VilleEchange {
    pub centre: Hex,
    /// Au bord de l'eau (fleuve, rivière, lac ou mer) : portée plus longue.
    pub eau: bool,
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

/// Une ville au bord de l'eau (fleuve, rivière, lac ou mer, sur sa case ou une voisine) : sa
/// portée d'échange est plus longue.
pub fn au_bord_de_l_eau(map: &crate::map::WorldMap, centre: Hex, site: &crate::rules::Site) -> bool {
    site.eau_courante || site.lac || std::iter::once(centre).chain(map.neighbors(centre)).any(|h| map.get(map.canonical(h)).is_some_and(|t| t.is_water()))
}

/// Un transfert du tour : `qte` de `ressource` de la ville `de` à la ville `vers`, et la
/// contrepartie reçue par `de`, s'il y en a une (sinon, un don).
#[derive(Clone, Debug, PartialEq)]
pub struct Transfert {
    pub de: usize,
    pub vers: usize,
    pub ressource: Ressource,
    pub qte: f64,
    pub contre: Option<(Ressource, f64)>,
}

/// Réseau de chaque ville : les villes reliées de proche en proche ont le même numéro.
pub fn reseaux(villes: &[VilleEchange], distance: impl Fn(Hex, Hex) -> i32) -> Vec<usize> {
    let n = villes.len();
    let mut reseau: Vec<usize> = (0..n).collect();
    fn racine(r: &mut [usize], mut i: usize) -> usize {
        while r[i] != i {
            r[i] = r[r[i]];
            i = r[i];
        }
        i
    }
    for a in 0..n {
        for b in a + 1..n {
            let portee = if villes[a].eau && villes[b].eau { PORTEE_EAU } else { PORTEE };
            if distance(villes[a].centre, villes[b].centre) <= portee {
                let (ra, rb) = (racine(&mut reseau, a), racine(&mut reseau, b));
                reseau[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    (0..n).map(|i| racine(&mut reseau, i)).collect()
}

/// Les échanges du tour : modifie les stocks et rend les transferts.
pub fn echanger(villes: &mut [VilleEchange], reseau: &[usize], distance: impl Fn(Hex, Hex) -> i32) -> Vec<Transfert> {
    let mut transferts = Vec::new();
    for r in 0..N_RESSOURCES {
        // La ville qui manque le plus (en tours de sa demande) d'abord
        let mut demandeuses: Vec<usize> = (0..villes.len()).filter(|&v| villes[v].manque(r) > 1e-9).collect();
        demandeuses.sort_by(|&a, &b| {
            let tours = |v: usize| villes[v].stock[r] / villes[v].demande[r].max(1e-9);
            tours(a).total_cmp(&tours(b)).then(a.cmp(&b))
        });
        for vers in demandeuses {
            let mut donneuses: Vec<usize> = (0..villes.len()).filter(|&d| d != vers && reseau[d] == reseau[vers] && villes[d].surplus(r) > 1e-9).collect();
            donneuses.sort_by_key(|&d| (distance(villes[d].centre, villes[vers].centre), d));
            for de in donneuses {
                let q = villes[vers].manque(r).min(villes[de].surplus(r));
                if q <= 1e-9 {
                    continue;
                }
                villes[de].stock[r] -= q;
                villes[vers].stock[r] += q;
                // Contrepartie : ce qui manque à la donneuse et que la demandeuse a en trop
                let contre = (0..N_RESSOURCES).filter(|&s| s != r).find_map(|s| {
                    let c = q.min(villes[de].manque(s)).min(villes[vers].surplus(s));
                    (c > 1e-9).then_some((s, c))
                });
                if let Some((s, c)) = contre {
                    villes[vers].stock[s] -= c;
                    villes[de].stock[s] += c;
                }
                transferts.push(Transfert { de, vers, ressource: Ressource::TOUTES[r], qte: q, contre: contre.map(|(s, c)| (Ressource::TOUTES[s], c)) });
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

    fn ville(col: i32, eau: bool) -> VilleEchange {
        VilleEchange { centre: Hex::from_offset(col, 0), eau, stock: [0.0; N_RESSOURCES], demande: [0.0; N_RESSOURCES] }
    }

    fn dist(a: Hex, b: Hex) -> i32 {
        a.distance(b)
    }

    #[test]
    fn la_demande_trouve_le_surplus_par_troc_ou_par_don() {
        let (sel, ocre) = (Ressource::Sel.index(), Ressource::Ocre.index());
        // A (0) a du sel en trop et manque d'ocre ; B (5) manque de sel et a de l'ocre en trop ;
        // C (11) manque de sel, n'a rien à offrir ; D (30) est trop loin
        let mut v = vec![ville(0, false), ville(5, false), ville(11, false), ville(30, false)];
        v[0].demande[sel] = 0.1;
        v[0].stock[sel] = 5.0;
        v[0].demande[ocre] = 0.2;
        v[1].demande[sel] = 0.5;
        v[1].stock[ocre] = 3.0;
        v[2].demande[sel] = 0.3;
        v[3].demande[sel] = 1.0;
        let reseau = reseaux(&v, dist);
        assert_eq!(reseau[0], reseau[1]);
        assert_eq!(reseau[1], reseau[2], "de proche en proche : C est reliée à A par B");
        assert_ne!(reseau[0], reseau[3]);
        let t = echanger(&mut v, &reseau, dist);
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
        // Au bord de l'eau, la portée est plus longue
        let w = vec![ville(0, true), ville(8, true)];
        let r2 = reseaux(&w, dist);
        assert_eq!(r2[0], r2[1]);
        assert_eq!(plafond(0.5), 6.0);
        assert_eq!(plafond(0.0), PLAFOND_SANS_DEMANDE);
    }
}
