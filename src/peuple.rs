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

/// Trésor de chaque peuple, en Économie.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tresors(Vec<i32>);

impl Tresors {
    /// Trésor d'un peuple (0 s'il n'a encore rien reçu).
    pub fn get(&self, peuple: Peuple) -> i32 {
        self.0.get(peuple.index()).copied().unwrap_or(0)
    }

    /// Trésor d'un peuple, à modifier (la table s'agrandit au besoin).
    pub fn get_mut(&mut self, peuple: Peuple) -> &mut i32 {
        let i = peuple.index();
        if self.0.len() <= i {
            self.0.resize(i + 1, 0);
        }
        &mut self.0[i]
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
    }
}
