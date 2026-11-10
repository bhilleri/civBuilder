//! Hasard de la partie, tiré de la graine (logique pure).
//!
//! La génération du monde est déjà déterministe ; ce module fait de même pour le hasard pendant la
//! partie (combats, visites de villages, faction hostile…). Chaque tirage part de la graine, du
//! flux (le système qui tire), du peuple concerné et du numéro du tirage dans ce flux : la même
//! partie, jouée avec les mêmes ordres, donne les mêmes résultats. C'est ce qui permettra de
//! recharger une sauvegarde sans changer la suite, de rejouer un bug et de comparer des parties
//! d'IA (`docs/conception/conception-ia.md`, « Déterminisme »).

use crate::peuple::Peuple;
use bevy::prelude::Resource;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::collections::BTreeMap;

/// Système qui tire au hasard : chacun a sa propre suite de tirages, pour qu'un tirage de plus
/// dans l'un ne décale pas les autres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Flux {
    /// Dégâts des combats.
    Combat,
    /// Visites des villages et des camps (événements, embuscades).
    Sites,
    /// Décisions de la faction hostile (recrutement, errance).
    Hostile,
}

impl Flux {
    /// Valeur fixe du flux dans le calcul de la graine (ne pas changer : les parties enregistrées en dépendent).
    fn code(self) -> u64 {
        match self {
            Flux::Combat => 1,
            Flux::Sites => 2,
            Flux::Hostile => 3,
        }
    }
}

/// Mélange 64 bits (SplitMix64) : deux entrées voisines donnent des sorties sans rapport.
fn melanger(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Générateur d'un tirage précis : même graine, flux, peuple et numéro, même générateur.
pub fn generateur(graine: u32, flux: Flux, peuple: Peuple, numero: u64) -> StdRng {
    let mut x = melanger(graine as u64);
    x = melanger(x ^ flux.code());
    x = melanger(x ^ peuple.0 as u64);
    x = melanger(x ^ numero);
    StdRng::seed_from_u64(x)
}

/// Hasard de la partie : la graine et le nombre de tirages déjà faits dans chaque flux, pour
/// chaque peuple (c'est cet état qu'une sauvegarde devra garder).
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct Hasard {
    graine: u32,
    tirages: BTreeMap<(Flux, Peuple), u64>,
}

impl Hasard {
    /// Hasard d'une nouvelle partie.
    pub fn new(graine: u32) -> Self {
        Self { graine, tirages: BTreeMap::new() }
    }

    /// Générateur du tirage suivant dans ce flux, pour ce peuple.
    pub fn tirage(&mut self, flux: Flux, peuple: Peuple) -> StdRng {
        let numero = self.tirages.entry((flux, peuple)).or_insert(0);
        let rng = generateur(self.graine, flux, peuple, *numero);
        *numero += 1;
        rng
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    fn suite(h: &mut Hasard, flux: Flux, peuple: Peuple) -> Vec<u32> {
        (0..5).map(|_| h.tirage(flux, peuple).random()).collect()
    }

    #[test]
    fn meme_graine_memes_tirages() {
        let (mut a, mut b) = (Hasard::new(777), Hasard::new(777));
        assert_eq!(suite(&mut a, Flux::Combat, Peuple::JOUEUR), suite(&mut b, Flux::Combat, Peuple::JOUEUR));
        assert_eq!(a, b);
    }

    #[test]
    fn graines_differentes_tirages_differents() {
        let (mut a, mut b) = (Hasard::new(777), Hasard::new(778));
        assert_ne!(suite(&mut a, Flux::Combat, Peuple::JOUEUR), suite(&mut b, Flux::Combat, Peuple::JOUEUR));
    }

    #[test]
    fn les_flux_ne_se_decalent_pas() {
        // Un tirage de plus dans un flux ne change pas la suite d'un autre
        let (mut a, mut b) = (Hasard::new(5), Hasard::new(5));
        a.tirage(Flux::Sites, Peuple::JOUEUR);
        a.tirage(Flux::Hostile, Peuple::HOSTILE);
        assert_eq!(suite(&mut a, Flux::Combat, Peuple::JOUEUR), suite(&mut b, Flux::Combat, Peuple::JOUEUR));
    }

    #[test]
    fn flux_et_peuples_distincts() {
        let mut h = Hasard::new(2);
        let combat = suite(&mut h, Flux::Combat, Peuple::JOUEUR);
        let sites = suite(&mut h, Flux::Sites, Peuple::JOUEUR);
        let hostile = suite(&mut h, Flux::Combat, Peuple::HOSTILE);
        assert_ne!(combat, sites);
        assert_ne!(combat, hostile);
    }

    #[test]
    fn tirages_successifs_differents() {
        let mut h = Hasard::new(9);
        let s = suite(&mut h, Flux::Hostile, Peuple::HOSTILE);
        assert!(s.windows(2).all(|w| w[0] != w[1]));
    }
}
