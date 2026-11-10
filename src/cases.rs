//! Ensemble de cases rangé en bits (logique pure).
//!
//! Remplace `HashSet<Hex>` pour le brouillard : un bit par case, ligne par ligne, soit environ
//! 3 Ko pour une carte de 192 × 111, au lieu de plusieurs centaines de Ko pour un ensemble haché
//! bien rempli. Avec un brouillard par peuple (`docs/conception/conception-ia.md`), 12 peuples
//! tiennent dans quelques dizaines de Ko. Les cases se parcourent dans un ordre fixe (ligne puis
//! colonne), ce qui garde le jeu déterministe.
//!
//! Les cases d'une carte sont ramenées dans la carte (`canonical`) : colonne et ligne positives.
//! Une case hors de ce cadre (coordonnées négatives, dans des tests ou avant `canonical`) est
//! gardée à part, sans erreur.

use crate::hex::Hex;
use std::collections::BTreeSet;

/// Ensemble de cases : un bit par case de la carte.
#[derive(Clone, Debug, Default)]
pub struct CaseSet {
    /// Bits de chaque ligne (indice de la ligne), 64 colonnes par mot.
    lignes: Vec<Vec<u64>>,
    /// Cases de coordonnées négatives (rares).
    hors: BTreeSet<Hex>,
    /// Nombre de cases.
    nombre: usize,
}

/// Ligne, mot et bit d'une case, si ses coordonnées sont positives.
fn place(h: Hex) -> Option<(usize, usize, u64)> {
    let (col, row) = h.to_offset();
    (col >= 0 && row >= 0).then(|| (row as usize, col as usize / 64, 1u64 << (col as usize % 64)))
}

impl CaseSet {
    /// Ensemble vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// La case est-elle dans l'ensemble ?
    pub fn contains(&self, h: &Hex) -> bool {
        match place(*h) {
            Some((r, w, bit)) => self.lignes.get(r).and_then(|l| l.get(w)).is_some_and(|m| m & bit != 0),
            None => self.hors.contains(h),
        }
    }

    /// Ajoute une case ; `true` si elle n'y était pas.
    pub fn insert(&mut self, h: Hex) -> bool {
        let nouvelle = match place(h) {
            Some((r, w, bit)) => {
                if self.lignes.len() <= r {
                    self.lignes.resize(r + 1, Vec::new());
                }
                let ligne = &mut self.lignes[r];
                if ligne.len() <= w {
                    ligne.resize(w + 1, 0);
                }
                let avant = ligne[w];
                ligne[w] |= bit;
                avant & bit == 0
            }
            None => self.hors.insert(h),
        };
        if nouvelle {
            self.nombre += 1;
        }
        nouvelle
    }

    /// Retire une case ; `true` si elle y était.
    #[allow(dead_code, reason = "pas encore utilisé hors des tests (brouillards par peuple)")]
    pub fn remove(&mut self, h: &Hex) -> bool {
        let presente = match place(*h) {
            Some((r, w, bit)) => match self.lignes.get_mut(r).and_then(|l| l.get_mut(w)) {
                Some(m) if *m & bit != 0 => {
                    *m &= !bit;
                    true
                }
                _ => false,
            },
            None => self.hors.remove(h),
        };
        if presente {
            self.nombre -= 1;
        }
        presente
    }

    /// Nombre de cases.
    pub fn len(&self) -> usize {
        self.nombre
    }

    /// L'ensemble est-il vide ?
    #[allow(dead_code, reason = "pas encore utilisé hors des tests")]
    pub fn is_empty(&self) -> bool {
        self.nombre == 0
    }

    /// Cases de l'ensemble, dans un ordre fixe : les cases hors cadre, puis ligne par ligne.
    pub fn iter(&self) -> impl Iterator<Item = Hex> + '_ {
        let dans_le_cadre = self.lignes.iter().enumerate().flat_map(|(r, ligne)| {
            ligne.iter().enumerate().flat_map(move |(w, &mot)| {
                (0..64).filter(move |b| mot & (1u64 << b) != 0).map(move |b| Hex::from_offset((w * 64 + b) as i32, r as i32))
            })
        });
        self.hors.iter().copied().chain(dans_le_cadre)
    }
}

impl CaseSet {
    /// Cases présentes dans un seul des deux ensembles.
    pub fn difference_symetrique<'a>(&'a self, autre: &'a CaseSet) -> impl Iterator<Item = Hex> + 'a {
        self.iter().filter(|h| !autre.contains(h)).chain(autre.iter().filter(|h| !self.contains(h)))
    }
}

impl Extend<Hex> for CaseSet {
    fn extend<I: IntoIterator<Item = Hex>>(&mut self, cases: I) {
        for h in cases {
            self.insert(h);
        }
    }
}

impl FromIterator<Hex> for CaseSet {
    fn from_iter<I: IntoIterator<Item = Hex>>(cases: I) -> Self {
        let mut set = Self::new();
        set.extend(cases);
        set
    }
}

impl PartialEq for CaseSet {
    fn eq(&self, other: &Self) -> bool {
        self.nombre == other.nombre && self.iter().all(|h| other.contains(&h))
    }
}

impl Eq for CaseSet {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ajouter_chercher_retirer() {
        let mut s = CaseSet::new();
        let a = Hex::from_offset(3, 2);
        let b = Hex::from_offset(130, 70);
        assert!(!s.contains(&a));
        assert!(s.insert(a));
        assert!(!s.insert(a));
        assert!(s.insert(b));
        assert_eq!(s.len(), 2);
        assert!(s.contains(&a) && s.contains(&b));
        assert!(!s.contains(&Hex::from_offset(4, 2)));
        assert!(s.remove(&a));
        assert!(!s.remove(&a));
        assert_eq!(s.len(), 1);
        assert!(!s.contains(&a));
        assert!(s.remove(&b));
        assert!(s.is_empty());
    }

    #[test]
    fn comme_un_ensemble_hache() {
        // Mêmes réponses qu'un HashSet sur un nuage de cases, y compris hors cadre
        let cases: Vec<Hex> = Hex::from_offset(5, 5).range(8).chain(Hex::from_offset(150, 100).range(3)).collect();
        let s: CaseSet = cases.iter().copied().collect();
        let h: HashSet<Hex> = cases.iter().copied().collect();
        assert_eq!(s.len(), h.len());
        for c in Hex::from_offset(5, 5).range(12) {
            assert_eq!(s.contains(&c), h.contains(&c), "{c:?}");
        }
        let parcours: HashSet<Hex> = s.iter().collect();
        assert_eq!(parcours, h);
    }

    #[test]
    fn ordre_de_parcours_fixe_et_egalite() {
        let mut a = CaseSet::new();
        let mut b = CaseSet::new();
        for c in [Hex::from_offset(9, 1), Hex::from_offset(2, 0), Hex::from_offset(70, 1)] {
            a.insert(c);
        }
        for c in [Hex::from_offset(70, 1), Hex::from_offset(9, 1), Hex::from_offset(2, 0)] {
            b.insert(c);
        }
        let ordre: Vec<(i32, i32)> = a.iter().map(|h| h.to_offset()).collect();
        assert_eq!(ordre, vec![(2, 0), (9, 1), (70, 1)]);
        assert_eq!(a, b);
        b.remove(&Hex::from_offset(2, 0));
        assert_ne!(a, b);
    }
}
