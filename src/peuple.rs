//! Peuples de la partie (logique pure).
//!
//! Chaque unité et chaque ville appartient à un peuple. Aujourd'hui il y en a deux : le joueur et
//! la faction hostile ; les peuples rivaux (`docs/conception/conception-ia.md`) s'y ajouteront.
//! Le trésor se compte par peuple (`GameState::tresor_mut`).

use bevy::prelude::Component;

/// Peuple propriétaire d'une unité ou d'une ville (un numéro stable, qui sert aussi au hasard,
/// voir `hasard.rs`). Par défaut, le joueur : toute unité ou ville créée sans peuple est à lui.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Peuple(pub u32);

impl Peuple {
    /// Le peuple du joueur.
    pub const JOUEUR: Peuple = Peuple(0);
    /// La faction hostile (camps barbares).
    pub const HOSTILE: Peuple = Peuple(1);

    /// Rang du peuple dans les tables par peuple (trésors…).
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Trésor de chaque peuple, en deniers.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tresors {
    entiers: Vec<i32>,
    /// Fraction de deniers pas encore versée (les villes en produisent des parts).
    #[serde(default)]
    fractions: Vec<f64>,
}

impl Tresors {
    /// Verse des deniers fractionnaires : la part entière va au trésor, le reste attend.
    pub fn ajouter(&mut self, peuple: Peuple, deniers: f64) {
        let i = peuple.index();
        if self.fractions.len() <= i {
            self.fractions.resize(i + 1, 0.0);
        }
        let total = self.fractions[i] + deniers;
        let entier = total.floor();
        self.fractions[i] = total - entier;
        *self.get_mut(peuple) += entier as i32;
    }

    /// Trésor d'un peuple (0 s'il n'a encore rien reçu).
    pub fn get(&self, peuple: Peuple) -> i32 {
        self.entiers.get(peuple.index()).copied().unwrap_or(0)
    }

    /// Trésor d'un peuple, à modifier (la table s'agrandit au besoin).
    pub fn get_mut(&mut self, peuple: Peuple) -> &mut i32 {
        let i = peuple.index();
        if self.entiers.len() <= i {
            self.entiers.resize(i + 1, 0);
        }
        &mut self.entiers[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tresors_separes_par_peuple() {
        let mut t = Tresors::default();
        assert_eq!(t.get(Peuple::HOSTILE), 0);
        *t.get_mut(Peuple::JOUEUR) += 10;
        *t.get_mut(Peuple::HOSTILE) += 3;
        *t.get_mut(Peuple::JOUEUR) -= 4;
        assert_eq!(t.get(Peuple::JOUEUR), 6);
        assert_eq!(t.get(Peuple::HOSTILE), 3);
        assert_eq!(t.get(Peuple(7)), 0);
        // Deniers fractionnaires : la part entière versée, le reste gardé
        t.ajouter(Peuple::JOUEUR, 1.75);
        t.ajouter(Peuple::JOUEUR, 0.5);
        assert_eq!(t.get(Peuple::JOUEUR), 8);
    }
}
