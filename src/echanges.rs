//! Échanges de ressources entre les villes d'un même peuple (étape 5 ; décisions de Benjamin des
//! 13 et 14/11/2026, `docs/conception/conception-ressources.md`, « Échanges » et « Transport »).
//!
//! Logique pure, à partir de la demande : une ville qui manque d'une ressource (moins de
//! `TOURS_VISES` tours de sa demande en stock) la reçoit d'une ville reliée qui en a au-delà de sa
//! réserve (`TOURS_RESERVE` tours). La ville qui donne tente d'obtenir en échange, une unité pour
//! une, ce qui lui manque et que l'autre a en trop ; sinon, elle donne.
//!
//! Le réseau : deux villes sont reliées par une **étape** de `TOURS_TRAJET_MAX` tours au plus, à
//! pied (`Porteur` : coûts de sentier) ou en pirogue (`EnPirogue` : fleuves, lacs, côte ; on charge
//! et décharge seulement dans les villes, 0,5 point chaque fois) ; au-delà, de ville en ville (les
//! villes du milieu voient passer le transit). Chaque étape porte au plus `Moyen::capacite` unités
//! par tour. Le transport se paie **en nature** (`Moyen::cout_par_tour` : vivres et salaire des
//! porteurs ou des bateliers), et les denrées fragiles se perdent en route (`perte_par_tour`).
//! L'échange ne se fait que s'il reste **rentable** : ce qui arrive doit valoir plus chez celle qui
//! reçoit que ce qui part chez celle qui donne (`valeur`, selon le stock : l'offre et la demande, en
//! attendant les prix). La demande la plus urgente passe d'abord, toutes ressources confondues.

use crate::gisements::{N_RESSOURCES, Ressource};
use crate::hex::{Hex, Wrap};
use crate::map::{MoveCosts, StepCost};

/// Points de mouvement d'un porteur à pied (ou d'une pirogue) par tour.
pub const PM_PORTEUR: u32 = 4;
/// Étape au plus entre deux villes (tours).
pub const TOURS_TRAJET_MAX: f64 = 3.0;
/// Distance au-delà de laquelle aucune étape à pied ne tient en `TOURS_TRAJET_MAX` tours (cases ;
/// une case coûte au moins 1 point).
pub const CASES_MAX: i32 = (TOURS_TRAJET_MAX as i32) * PM_PORTEUR as i32;
/// Même borne en pirogue (une case d'eau coûte au moins 0,5 point).
pub const CASES_MAX_PIROGUE: i32 = 2 * CASES_MAX;
/// Une ville demande une ressource tant qu'elle en a moins que ce nombre de tours de sa demande.
pub const TOURS_VISES: f64 = 2.0;
/// Réserve qu'une ville garde (tours de sa demande) ; elle n'échange que ce qui dépasse.
pub const TOURS_RESERVE: f64 = 6.0;
/// Au-delà de ce nombre de tours de sa demande, une ville perd ce qu'elle produit en trop.
pub const TOURS_PLAFOND: f64 = 12.0;
/// Stock au plus d'une ressource que la ville ne consomme pas encore (bois d'œuvre…).
pub const PLAFOND_SANS_DEMANDE: f64 = 20.0;
/// Valeur d'une unité d'une ressource selon le stock de la ville, en tours de sa demande : manque
/// total, but, réserve, plafond (au-delà, le surplus serait perdu).
pub const VALEURS: [(f64, f64); 4] = [(0.0, 2.0), (TOURS_VISES, 1.0), (TOURS_RESERVE, 0.5), (TOURS_PLAFOND, 0.1)];

/// Part d'une ressource perdue par tour de trajet (cassée, gâtée en route) : rien pour la pierre et
/// le sel, 5 % pour les fibres et le bois, 10 % pour les plantes médicinales (décision de
/// Benjamin, 13/11/2026).
pub fn perte_par_tour(r: Ressource) -> f64 {
    match r {
        Ressource::Sel | Ressource::Silex | Ressource::Obsidienne | Ressource::PierreDure | Ressource::Ocre | Ressource::Parures => 0.0,
        Ressource::Fibres | Ressource::BoisOeuvre => 0.05,
        Ressource::Plantes => 0.10,
    }
}

/// Valeur d'une unité d'une ressource pour une ville, selon son stock et sa demande par tour
/// (`VALEURS`, entre les points en ligne droite) ; une ressource qu'elle ne consomme pas vaut le
/// plancher.
pub fn valeur(stock: f64, demande: f64) -> f64 {
    let plancher = VALEURS[VALEURS.len() - 1].1;
    if demande <= 0.0 {
        return plancher;
    }
    let t = (stock / demande).max(0.0);
    for w in VALEURS.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            return v0 + (v1 - v0) * (t - t0) / (t1 - t0);
        }
    }
    plancher
}

/// Moyen de transport d'une étape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moyen {
    Pied,
    Pirogue,
}

impl Moyen {
    /// Charge portée au plus par tour sur une étape (unités ; décision de Benjamin, 14/11/2026).
    pub fn capacite(self) -> f64 {
        match self {
            Moyen::Pied => 4.0,
            Moyen::Pirogue => 12.0,
        }
    }

    /// Part de la charge payée en nature par tour de trajet (vivres et salaire des porteurs ou des
    /// bateliers ; la pirogue porte plus pour autant de bras).
    pub fn cout_par_tour(self) -> f64 {
        match self {
            Moyen::Pied => 0.05,
            Moyen::Pirogue => 0.02,
        }
    }
}

/// Une étape : la liaison directe entre deux villes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Etape {
    pub tours: f64,
    pub moyen: Moyen,
}

impl Etape {
    /// Part de la charge payée pour le transport.
    pub fn cout(&self) -> f64 {
        self.moyen.cout_par_tour() * self.tours
    }
}

/// Trajet d'un porteur à pied entre deux centres (tours), s'il tient en `TOURS_TRAJET_MAX` tours.
pub fn tours_de_trajet(map: &impl MoveCosts, a: Hex, b: Hex) -> Option<f64> {
    if map.wrap().distance(a, b) > CASES_MAX {
        return None;
    }
    trajet_a_pied(map, a, b).filter(|t| *t <= TOURS_TRAJET_MAX + 1e-9)
}

/// Trajet d'un porteur à pied entre deux cases (tours), quel qu'il soit (aucun : pas de chemin).
pub fn trajet_a_pied(map: &impl MoveCosts, a: Hex, b: Hex) -> Option<f64> {
    let porteur = Porteur(map);
    let chemin = crate::pathfinding::find_path(&porteur, a, b)?;
    Some(crate::rules::path_cost(&porteur, a, &chemin) as f64 / DEMI_POINTS as f64 / PM_PORTEUR as f64)
}

/// L'étape la moins chère entre deux centres (à pied ou en pirogue ; à coût égal, la plus rapide).
pub fn etape(map: &crate::map::WorldMap, a: Hex, b: Hex) -> Option<Etape> {
    let pied = tours_de_trajet(map, a, b).map(|tours| Etape { tours, moyen: Moyen::Pied });
    let eau = (map.wrap().distance(a, b) <= CASES_MAX_PIROGUE).then(|| trajet_en_pirogue(map, a, b)).flatten().filter(|t| *t <= TOURS_TRAJET_MAX + 1e-9).map(|tours| Etape { tours, moyen: Moyen::Pirogue });
    match (pied, eau) {
        (Some(p), Some(e)) => Some(if e.cout() < p.cout() - 1e-12 || ((e.cout() - p.cout()).abs() <= 1e-12 && e.tours < p.tours) { e } else { p }),
        (p, e) => p.or(e),
    }
}

/// Les coûts d'un pas comptent en demi-points pour le porteur.
const DEMI_POINTS: u32 = 2;
/// Et en quarts de point pour la pirogue.
const QUARTS: u32 = 4;
/// Charger ou décharger (quarts de point : 0,5 point ; décision de Benjamin, 14/11/2026).
const TRANSBORDEMENT: u32 = 2;

/// Coûts d'un porteur à pied (décision de Benjamin, 14/11/2026 : des sentiers, pas les coûts de
/// l'armée) : le terrain et le relief comptent la moitié de ceux des unités, 1 point au moins
/// (plaine 1, forêt 2, marais 3, colline 1,5, montagne 4 à 8) ; une rivière se passe pour 1 point
/// de plus, un fleuve pour 2 (gué, bac). En demi-points, pour rester entier.
pub struct Porteur<M>(pub M);

impl<M: MoveCosts> MoveCosts for Porteur<M> {
    fn move_cost(&self, hex: Hex) -> Option<u32> {
        self.0.move_cost(hex).map(|c| c.max(DEMI_POINTS))
    }

    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        let s = self.0.step(from, to)?;
        let eau = match s.water {
            0 => 0,
            1..=4 => DEMI_POINTS,
            _ => 2 * DEMI_POINTS,
        };
        // (un fleuve n'a que de l'eau : pas de terrain en plus)
        let terrain = if s.terrain == 0 && eau > 0 { 0 } else { s.terrain.max(DEMI_POINTS) };
        Some(StepCost { terrain, water: eau, ..s })
    }

    fn wrap(&self) -> Wrap {
        self.0.wrap()
    }
}

/// Les eaux qu'une pirogue peut suivre : le coût d'entrée dans une case d'eau depuis sa voisine,
/// en quarts de point (aucun : pas navigable en pirogue).
pub trait Eaux {
    fn eaux_wrap(&self) -> Wrap;
    fn entree(&self, de: Hex, vers: Hex) -> Option<u32>;
}

/// Eaux de la carte : descendre un fleuve 0,5 point par case, le remonter 1 ; un lac 0,5 ; la côte
/// 0,75 (cabotage) ; ni la haute mer (avant la Voile) ni les rivières (sur les bords des cases).
impl Eaux for crate::map::WorldMap {
    fn eaux_wrap(&self) -> Wrap {
        self.wrap()
    }

    fn entree(&self, de: Hex, vers: Hex) -> Option<u32> {
        let (de, vers) = (self.canonical(de), self.canonical(vers));
        if self.is_fleuve(vers) {
            let descend = self.is_fleuve(de) && self.downstream(de).is_some_and(|d| self.canonical(self.wrap().neighbor(de, d)) == vers);
            return Some(if descend { 2 } else { 4 });
        }
        match self.get(vers)? {
            crate::map::Terrain::Lake => Some(2),
            crate::map::Terrain::Coast => Some(3),
            _ => None,
        }
    }
}

/// Trajet d'une pirogue d'un centre à l'autre (tours) : on charge au départ, on suit l'eau, on
/// décharge à l'arrivée.
pub fn trajet_en_pirogue(eaux: &impl Eaux, a: Hex, b: Hex) -> Option<f64> {
    let p = EnPirogue { eaux, a, b };
    let chemin = crate::pathfinding::find_path(&p, a, b)?;
    Some(crate::rules::path_cost(&p, a, &chemin) as f64 / QUARTS as f64 / PM_PORTEUR as f64)
}

/// La pirogue entre deux centres, vue comme des coûts de déplacement (en quarts de point).
struct EnPirogue<'a, E> {
    eaux: &'a E,
    a: Hex,
    b: Hex,
}

impl<E: Eaux> MoveCosts for EnPirogue<'_, E> {
    fn move_cost(&self, _: Hex) -> Option<u32> {
        Some(1)
    }

    fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
        if to == self.a {
            return None;
        }
        if to == self.b {
            // (on décharge depuis l'eau, jamais à pied)
            return (from != self.a).then_some(StepCost::terrain(TRANSBORDEMENT));
        }
        let c = self.eaux.entree(from, to)?;
        Some(StepCost::terrain(c + if from == self.a { TRANSBORDEMENT } else { 0 }))
    }

    fn wrap(&self) -> Wrap {
        self.eaux.eaux_wrap()
    }
}

/// Liaison entre deux villes du réseau : les villes traversées (`route`, de la première à la
/// dernière), le trajet (dont en pirogue), la part payée pour le transport.
#[derive(Clone, Debug, PartialEq)]
pub struct Liaison {
    pub route: Vec<usize>,
    pub tours: f64,
    pub tours_pirogue: f64,
    pub cout: f64,
}

/// Le réseau des villes : les étapes directes et la liaison la moins chère entre chaque paire.
#[derive(Clone, Debug, Default)]
pub struct Reseau {
    pub etapes: Vec<Vec<Option<Etape>>>,
    pub liaisons: Vec<Vec<Option<Liaison>>>,
}

impl Reseau {
    /// Les `n` villes et leurs étapes (`etape(a, b)` pour `a < b`) ; de ville en ville, la liaison
    /// la moins chère (à coût égal, la plus rapide).
    #[allow(clippy::needless_range_loop)]
    pub fn new(n: usize, etape: impl Fn(usize, usize) -> Option<Etape>) -> Self {
        let mut etapes = vec![vec![None; n]; n];
        for a in 0..n {
            for b in a + 1..n {
                let e = etape(a, b);
                etapes[a][b] = e;
                etapes[b][a] = e;
            }
        }
        // (coût, tours) et la ville suivante sur la route
        let mut m: Vec<Vec<Option<(f64, f64)>>> = vec![vec![None; n]; n];
        let mut suivante = vec![vec![usize::MAX; n]; n];
        for a in 0..n {
            m[a][a] = Some((0.0, 0.0));
            suivante[a][a] = a;
            for b in 0..n {
                if let Some(e) = etapes[a][b] {
                    m[a][b] = Some((e.cout(), e.tours));
                    suivante[a][b] = b;
                }
            }
        }
        for k in 0..n {
            for a in 0..n {
                for b in 0..n {
                    if let (Some(x), Some(y)) = (m[a][k], m[k][b]) {
                        let par_k = (x.0 + y.0, x.1 + y.1);
                        let mieux = m[a][b].is_none_or(|d| par_k.0 < d.0 - 1e-12 || ((par_k.0 - d.0).abs() <= 1e-12 && par_k.1 < d.1 - 1e-12));
                        if mieux {
                            m[a][b] = Some(par_k);
                            suivante[a][b] = suivante[a][k];
                        }
                    }
                }
            }
        }
        let mut liaisons = vec![vec![None; n]; n];
        for a in 0..n {
            for b in 0..n {
                if a == b || m[a][b].is_none() {
                    continue;
                }
                let mut route = vec![a];
                let mut x = a;
                while x != b {
                    x = suivante[x][b];
                    route.push(x);
                }
                let (mut tours, mut tours_pirogue, mut cout) = (0.0, 0.0, 0.0);
                for w in route.windows(2) {
                    let e = etapes[w[0]][w[1]].expect("étape de la route");
                    tours += e.tours;
                    cout += e.cout();
                    if e.moyen == Moyen::Pirogue {
                        tours_pirogue += e.tours;
                    }
                }
                liaisons[a][b] = Some(Liaison { route, tours, tours_pirogue, cout });
            }
        }
        Reseau { etapes, liaisons }
    }

    /// Part d'une ressource qui arrive au bout d'une liaison (le transport payé, la perte en route).
    pub fn part_arrivee(l: &Liaison, r: Ressource) -> f64 {
        (1.0 - l.cout - perte_par_tour(r) * l.tours).max(0.0)
    }
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

    /// Valeur d'une unité de la ressource pour elle.
    pub fn valeur(&self, r: usize) -> f64 {
        valeur(self.stock[r], self.demande[r])
    }
}

/// Un transfert du tour : `qte` de `ressource` envoyée de la ville `de` à la ville `vers` par la
/// `route` (villes traversées comprises), dont `paye` pour le transport et `perdu` en route, après
/// `tours` tours de trajet (dont `tours_pirogue` en pirogue) ; la contrepartie envoyée par `vers`,
/// s'il y en a une (sinon, un don).
#[derive(Clone, Debug, PartialEq)]
pub struct Transfert {
    pub de: usize,
    pub vers: usize,
    pub ressource: Ressource,
    pub qte: f64,
    pub paye: f64,
    pub perdu: f64,
    pub tours: f64,
    pub tours_pirogue: f64,
    pub route: Vec<usize>,
    pub contre: Option<(Ressource, f64)>,
}

/// Les échanges du tour entre les villes du réseau : modifie les stocks et rend les transferts.
pub fn echanger(villes: &mut [VilleEchange], reseau: &Reseau) -> Vec<Transfert> {
    let n = villes.len();
    // Charge restante de chaque étape ce tour (les deux sens ensemble)
    let mut restant: Vec<Vec<f64>> = (0..n).map(|a| (0..n).map(|b| reseau.etapes[a][b].map_or(0.0, |e| e.moyen.capacite())).collect()).collect();
    let capacite = |restant: &Vec<Vec<f64>>, route: &[usize]| route.windows(2).map(|w| restant[w[0]][w[1]]).fold(f64::INFINITY, f64::min);
    let utiliser = |restant: &mut Vec<Vec<f64>>, route: &[usize], q: f64| {
        for w in route.windows(2) {
            restant[w[0]][w[1]] -= q;
            restant[w[1]][w[0]] -= q;
        }
    };
    // Les demandes, la plus urgente d'abord (en tours de sa demande), toutes ressources confondues
    let mut demandes: Vec<(usize, usize)> = (0..n).flat_map(|v| (0..N_RESSOURCES).map(move |r| (v, r))).filter(|&(v, r)| villes[v].manque(r) > 1e-9).collect();
    demandes.sort_by(|&(va, ra), &(vb, rb)| {
        let tours = |v: usize, r: usize| villes[v].stock[r] / villes[v].demande[r].max(1e-9);
        tours(va, ra).total_cmp(&tours(vb, rb)).then(va.cmp(&vb)).then(ra.cmp(&rb))
    });
    let mut transferts = Vec::new();
    for (vers, r) in demandes {
        let res = Ressource::TOUTES[r];
        // Donneuses : la part qui arrive la plus grande d'abord
        let mut donneuses: Vec<(usize, &Liaison, f64)> = (0..n)
            .filter(|&d| d != vers && villes[d].surplus(r) > 1e-9)
            .filter_map(|d| reseau.liaisons[d][vers].as_ref().map(|l| (d, l, Reseau::part_arrivee(l, res))))
            .collect();
        donneuses.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        for (de, l, garde) in donneuses {
            let manque = villes[vers].manque(r);
            if manque <= 1e-9 {
                break;
            }
            // Rentable : ce qui arrive vaut plus chez la demandeuse que ce qui part chez la donneuse
            if garde * villes[vers].valeur(r) <= villes[de].valeur(r) {
                continue;
            }
            let q = (manque / garde).min(villes[de].surplus(r)).min(capacite(&restant, &l.route));
            if q <= 1e-9 {
                continue;
            }
            villes[de].stock[r] -= q;
            villes[vers].stock[r] += q * garde;
            utiliser(&mut restant, &l.route, q);
            // Contrepartie (une unité envoyée pour une) : ce qui manque à la donneuse et que la
            // demandeuse a en trop, si elle aussi reste rentable ; elle paie aussi le transport
            let contre = (0..N_RESSOURCES).filter(|&s| s != r).find_map(|s| {
                let g = Reseau::part_arrivee(l, Ressource::TOUTES[s]);
                if g <= 0.0 || g * villes[de].valeur(s) <= villes[vers].valeur(s) {
                    return None;
                }
                let c = q.min(villes[de].manque(s) / g).min(villes[vers].surplus(s)).min(capacite(&restant, &l.route));
                (c > 1e-9).then_some((s, c, g))
            });
            if let Some((s, c, g)) = contre {
                villes[vers].stock[s] -= c;
                villes[de].stock[s] += c * g;
                utiliser(&mut restant, &l.route, c);
            }
            transferts.push(Transfert {
                de,
                vers,
                ressource: res,
                qte: q,
                paye: q * l.cout,
                perdu: q * (1.0 - l.cout - garde),
                tours: l.tours,
                tours_pirogue: l.tours_pirogue,
                route: l.route.clone(),
                contre: contre.map(|(s, c, _)| (Ressource::TOUTES[s], c)),
            });
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

    /// Réseau en plaine, à pied (1 point par case) : étapes de 12 cases au plus.
    fn a_pied(v: &[VilleEchange]) -> Reseau {
        Reseau::new(v.len(), |a, b| {
            let t = v[a].centre.distance(v[b].centre) as f64 / PM_PORTEUR as f64;
            (t <= TOURS_TRAJET_MAX).then_some(Etape { tours: t, moyen: Moyen::Pied })
        })
    }

    fn proche(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
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
        let reseau = a_pied(&v);
        assert!(reseau.liaisons[0][3].is_none());
        let t = echanger(&mut v, &reseau);
        // Chacune reçoit ses 2 tours de demande (le transport payé en route)
        assert!(proche(v[1].stock[sel], 1.0) && proche(v[2].stock[sel], 0.6) && proche(v[0].stock[ocre], 0.4));
        // A demande l'ocre (toutes ressources confondues, A d'abord) et paie en sel ; C reçoit en don
        let ta = t.iter().find(|x| x.vers == 0).unwrap();
        assert_eq!((ta.ressource, ta.contre.map(|(r, _)| r)), (Ressource::Ocre, Some(Ressource::Sel)));
        assert!(t.iter().find(|x| x.vers == 2).unwrap().contre.is_none(), "rien à offrir : un don");
        // Le transport se paie : 5 % par tour à pied (1,25 tour entre A et B)
        assert!(proche(ta.paye, ta.qte * 0.0625) && ta.perdu.abs() < 1e-12, "le sel et l'ocre ne se perdent pas, mais le transport se paie");
        // A garde sa réserve (6 tours de 0,1) ; D, trop loin, ne reçoit rien
        assert!(v[0].stock[sel] >= TOURS_RESERVE * 0.1 - 1e-9);
        assert_eq!(v[3].stock[sel], 0.0);
        assert_eq!(plafond(0.5), 6.0);
        assert_eq!(plafond(0.0), PLAFOND_SANS_DEMANDE);
    }

    #[test]
    fn les_plantes_se_perdent_en_route() {
        let p = Ressource::Plantes.index();
        // B, à 6 cases (1,5 tour), manque de plantes : A envoie de quoi combler ; 7,5 % paient le
        // transport, 15 % se gâtent
        let mut v = vec![ville(0), ville(6)];
        v[0].stock[p] = 10.0;
        v[1].demande[p] = 1.0;
        let reseau = a_pied(&v);
        let t = echanger(&mut v, &reseau);
        assert!(proche(v[1].stock[p], 2.0), "B reçoit ses 2 tours de demande");
        let q = 2.0 / 0.775;
        assert!(proche(t[0].qte, q) && proche(t[0].paye, q * 0.075) && proche(t[0].perdu, q * 0.15));
        assert_eq!(perte_par_tour(Ressource::Sel), 0.0);
    }

    #[test]
    fn la_valeur_suit_le_stock() {
        assert!(proche(valeur(0.0, 1.0), 2.0) && proche(valeur(2.0, 1.0), 1.0) && proche(valeur(6.0, 1.0), 0.5));
        assert!(proche(valeur(4.0, 1.0), 0.75) && proche(valeur(12.0, 1.0), 0.1) && proche(valeur(50.0, 1.0), 0.1));
        assert!(proche(valeur(3.0, 0.0), 0.1), "une ressource que la ville ne consomme pas vaut le plancher");
    }

    #[test]
    fn un_echange_peu_rentable_ne_se_fait_pas() {
        let p = Ressource::Plantes.index();
        // B a 1,8 tour de plantes (valeur 1,1) ; A en a 6,5 tours (valeur ≈ 0,47) : à 4 tours de
        // trajet par un relais, 40 % arrivent (20 % de transport, 40 % gâtés) : 0,44 < 0,47, ça ne
        // vaut pas la peine
        let mut v = vec![ville(0), ville(8), ville(16)];
        v[0].demande[p] = 1.0;
        v[0].stock[p] = 6.5;
        v[2].demande[p] = 1.0;
        v[2].stock[p] = 1.8;
        let reseau = a_pied(&v);
        assert!(proche(Reseau::part_arrivee(reseau.liaisons[0][2].as_ref().unwrap(), Ressource::Plantes), 0.4));
        assert!(echanger(&mut v.clone(), &reseau).is_empty());
        // Au plafond, le surplus serait perdu sur place : il part
        v[0].stock[p] = 12.0;
        assert!(!echanger(&mut v, &reseau).is_empty());
    }

    #[test]
    fn une_etape_porte_ce_que_permet_son_moyen() {
        let sel = Ressource::Sel.index();
        // B manque de 10 unités de sel : à pied, 4 par tour au plus
        let mut v = vec![ville(0), ville(4)];
        v[0].stock[sel] = 50.0;
        v[1].demande[sel] = 5.0;
        let reseau = a_pied(&v);
        let t = echanger(&mut v, &reseau);
        assert!(proche(t.iter().map(|x| x.qte).sum::<f64>(), 4.0));
        // En pirogue, 12
        let mut w = vec![ville(0), ville(4)];
        w[0].stock[sel] = 50.0;
        w[1].demande[sel] = 5.0;
        let pirogue = Reseau::new(2, |_, _| Some(Etape { tours: 0.5, moyen: Moyen::Pirogue }));
        let t = echanger(&mut w, &pirogue);
        assert!(proche(t[0].qte, 10.0 / 0.99), "toute la demande passe ({})", t[0].qte);
    }

    #[test]
    fn le_transit_passe_par_la_ville_du_milieu() {
        let sel = Ressource::Sel.index();
        // A (0) et C (20) ne sont pas reliées directement (5 tours) : B (10) sert de relais
        let mut v = vec![ville(0), ville(10), ville(20)];
        v[0].stock[sel] = 10.0;
        v[2].demande[sel] = 0.5;
        let reseau = a_pied(&v);
        let l = reseau.liaisons[0][2].as_ref().unwrap();
        assert_eq!(l.route, vec![0, 1, 2]);
        assert!(proche(l.tours, 5.0) && proche(l.cout, 0.25));
        let t = echanger(&mut v, &reseau);
        assert_eq!(t[0].route, vec![0, 1, 2]);
        assert!(proche(v[2].stock[sel], 1.0));
    }

    /// Une ligne de cases : à pied, le coût de chaque colonne (forêt 4…) et une rivière à franchir
    /// avant la colonne `riviere` ; en pirogue, les colonnes d'eau (coût en quarts de point).
    struct Bande {
        couts: Vec<u32>,
        riviere: i32,
        eau: Vec<Option<u32>>,
    }

    impl MoveCosts for Bande {
        fn move_cost(&self, hex: Hex) -> Option<u32> {
            let (c, l) = hex.to_offset();
            (l == 0 && c >= 0).then(|| self.couts.get(c as usize).copied()).flatten()
        }

        fn step(&self, from: Hex, to: Hex) -> Option<StepCost> {
            let terrain = self.move_cost(to)?;
            let water = if to.to_offset().0 == self.riviere && from.to_offset().0 == self.riviere - 1 { crate::map::RIVER_CROSSING_COST } else { 0 };
            Some(StepCost { terrain, water, ..Default::default() })
        }
    }

    impl Eaux for Bande {
        fn eaux_wrap(&self) -> Wrap {
            Wrap::NONE
        }

        fn entree(&self, _: Hex, vers: Hex) -> Option<u32> {
            let (c, l) = vers.to_offset();
            (l == 0 && c >= 0).then(|| self.eau.get(c as usize).copied().flatten()).flatten()
        }
    }

    #[test]
    fn le_porteur_suit_les_sentiers() {
        // Quatre cases de forêt (4 PM chacune pour une unité) puis une plaine, une rivière avant la
        // dernière : 4 × 2 + 1 + 1 = 10 points, 2,5 tours (une unité y mettrait 4 × 4 + 1 + 4 = 21 PM)
        let bande = Bande { couts: vec![1, 4, 4, 4, 4, 1], riviere: 5, eau: Vec::new() };
        let (a, b) = (Hex::from_offset(0, 0), Hex::from_offset(5, 0));
        assert_eq!(trajet_a_pied(&bande, a, b), Some(2.5));
        assert_eq!(tours_de_trajet(&bande, a, b), Some(2.5), "3 tours au plus : reliées");
        // Six cases de marais (3 points chacune) : 4,5 tours, pas reliées
        let marais = Bande { couts: vec![1, 6, 6, 6, 6, 6, 6], riviere: 99, eau: Vec::new() };
        assert_eq!(tours_de_trajet(&marais, a, Hex::from_offset(6, 0)), None);
    }

    #[test]
    fn la_pirogue_descend_vite_et_charge_en_ville() {
        // Deux villes (colonnes 0 et 9) et un fleuve de 8 cases entre elles, descendu (0,5 point par
        // case) : 0,5 pour charger + 8 × 0,5 + 0,5 pour décharger = 5 points, 1,25 tour
        let mut eau = vec![None];
        eau.extend(std::iter::repeat_n(Some(2), 8));
        eau.push(None);
        let fleuve = Bande { couts: vec![1; 10], riviere: 99, eau };
        let (a, b) = (Hex::from_offset(0, 0), Hex::from_offset(9, 0));
        assert_eq!(trajet_en_pirogue(&fleuve, a, b), Some(1.25));
        // Sans eau entre les deux, pas de pirogue
        let sec = Bande { couts: vec![1; 10], riviere: 99, eau: vec![None; 10] };
        assert_eq!(trajet_en_pirogue(&sec, a, b), None);
        // Le transport coûte moins en pirogue : 2,5 % pour ce trajet, contre 11,25 % à pied (2,25 tours)
        let (p, e) = (Etape { tours: 2.25, moyen: Moyen::Pied }, Etape { tours: 1.25, moyen: Moyen::Pirogue });
        assert!(proche(p.cout(), 0.1125) && proche(e.cout(), 0.025));
    }

    /// Bilan du réseau des échanges sur des cartes générées (graines `SEEDS`, 777 et 2 par
    /// défaut) : par grappe de villes (à 4 cases au moins les unes des autres, dans un rayon de 15
    /// cases), étapes à pied et en pirogue, villes reliées, trajets, part qui arrive, et ce que la
    /// rentabilité laisse passer (perte acceptée : 45 % pour un léger manque, 70 % pour un manque, 95 %
    /// pour un manque total contre un surplus perdu).
    #[test]
    #[ignore]
    fn bilan_echanges() {
        use crate::map::{Terrain, WorldMap};
        let seeds: Vec<u32> = std::env::var("SEEDS").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or(vec![777, 2]);
        for seed in seeds {
            let map = WorldMap::generate(192, 111, seed);
            let terres: Vec<Hex> = map.hexes().filter(|&h| map.get(h).is_some_and(|t| !t.is_water() && t != Terrain::Snow) && map.move_cost(h).is_some()).collect();
            let (mut etapes_pied, mut etapes_eau, mut paires, mut reliees, mut avec_pirogue) = (0, 0, 0, 0, 0);
            let mut tours: Vec<f64> = Vec::new();
            let (mut sel, mut plantes) = ([0usize; 3], [0usize; 3]);
            let debut = std::time::Instant::now();
            for g in 0..6 {
                let germe = terres[(g * 7919 + 13) % terres.len()];
                let mut centres: Vec<Hex> = Vec::new();
                for &h in terres.iter().filter(|&&h| map.distance(germe, h) <= 15) {
                    if centres.iter().all(|&c| map.distance(c, h) >= 4) && centres.len() < 10 {
                        centres.push(h);
                    }
                }
                let reseau = Reseau::new(centres.len(), |a, b| etape(&map, centres[a], centres[b]));
                for a in 0..centres.len() {
                    for b in a + 1..centres.len() {
                        match reseau.etapes[a][b].map(|e| e.moyen) {
                            Some(Moyen::Pied) => etapes_pied += 1,
                            Some(Moyen::Pirogue) => etapes_eau += 1,
                            None => {}
                        }
                        paires += 1;
                        if let Some(l) = &reseau.liaisons[a][b] {
                            reliees += 1;
                            tours.push(l.tours);
                            if l.tours_pirogue > 0.0 {
                                avec_pirogue += 1;
                            }
                            for (k, seuil) in [0.45, 0.70, 0.95].into_iter().enumerate() {
                                if Reseau::part_arrivee(l, Ressource::Sel) >= 1.0 - seuil {
                                    sel[k] += 1;
                                }
                                if Reseau::part_arrivee(l, Ressource::Plantes) >= 1.0 - seuil {
                                    plantes[k] += 1;
                                }
                            }
                        }
                    }
                }
            }
            tours.sort_by(f64::total_cmp);
            let quantile = |q: f64| tours.get(((tours.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
            println!("graine {seed} : 6 grappes, {paires} paires de villes ({:.1} s)", debut.elapsed().as_secs_f64());
            println!("  étapes directes : {etapes_pied} à pied, {etapes_eau} en pirogue");
            println!("  reliées : {reliees} ({:.0} %), dont {avec_pirogue} avec de la pirogue ; trajet médian {:.1} tours, 90 % sous {:.1}, au plus {:.1}", 100.0 * reliees as f64 / paires.max(1) as f64, quantile(0.5), quantile(0.9), quantile(1.0));
            println!("  rentables (perte acceptée 45 / 70 / 95 %) : sel {} / {} / {} ; plantes {} / {} / {} paires reliées", sel[0], sel[1], sel[2], plantes[0], plantes[1], plantes[2]);
        }
    }
}
