//! Gisements du lot néolithique (étape 5, livraison 5f ; `docs/conception/conception-ressources.md`).
//!
//! Logique pure : un gisement se déduit de la roche, du sol, du biome, du relief, du climat et des
//! rivages de la case, plus un tirage par case et par ressource fixé par la graine (même graine,
//! mêmes gisements). Une case porte au plus un gisement. Les affleurements (sel, silex,
//! obsidienne, ocre) et les ressources vivantes (plantes médicinales, fibres et peaux, bois d'œuvre) se voient
//! dès que la case est explorée ; la pierre dure et les pierres et parures se découvrent par la
//! prospection. Ces couches ne changent ni le terrain, ni le relief : mondes et sauvegardes
//! identiques.

use crate::geologie::{Roche, Sol};
use crate::map::{Relief, Terrain};

/// Ressource d'un gisement (lot néolithique, décision D1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Ressource {
    /// Sel : marais salants des côtes chaudes et sèches, rives des lacs salés, sel gemme.
    Sel,
    /// Silex : rognons de la craie et des calcaires.
    Silex,
    /// Obsidienne : verre volcanique des arcs et des volcans.
    Obsidienne,
    /// Pierre dure (dolérite, jadéite, basalte) : haches polies, meules, mégalithes.
    PierreDure,
    /// Ocre : terres rouges et jaunes des grès, des argiles et des latérites.
    Ocre,
    /// Plantes médicinales (thym, sauge, valériane, écorce de saule…) des garrigues et des forêts
    /// de pente (nommées « simples » autrefois).
    Plantes,
    /// Pierres et parures : pierres précieuses des vieux massifs, ambre des côtes froides.
    Parures,
    /// Fibres et peaux : lin des prairies, fourrures et peaux du nord.
    Fibres,
    /// Bois d'œuvre : vieilles forêts de grands arbres.
    BoisOeuvre,
}

/// Nombre de ressources.
pub const N_RESSOURCES: usize = 9;

impl Ressource {
    pub const TOUTES: [Ressource; N_RESSOURCES] =
        [Ressource::Sel, Ressource::Silex, Ressource::Obsidienne, Ressource::PierreDure, Ressource::Ocre, Ressource::Plantes, Ressource::Parures, Ressource::Fibres, Ressource::BoisOeuvre];

    pub const fn nom(self) -> &'static str {
        match self {
            Ressource::Sel => "sel",
            Ressource::Silex => "silex",
            Ressource::Obsidienne => "obsidienne",
            Ressource::PierreDure => "pierre dure",
            Ressource::Ocre => "ocre",
            Ressource::Plantes => "plantes médicinales",
            Ressource::Parures => "pierres et parures",
            Ressource::Fibres => "fibres et peaux",
            Ressource::BoisOeuvre => "bois d'œuvre",
        }
    }

    /// Abréviation pour l'icône de la carte des ressources.
    pub const fn sigle(self) -> &'static str {
        match self {
            Ressource::Sel => "Se",
            Ressource::Silex => "Si",
            Ressource::Obsidienne => "Ob",
            Ressource::PierreDure => "Pd",
            Ressource::Ocre => "Oc",
            Ressource::Plantes => "Pm",
            Ressource::Parures => "Pp",
            Ressource::Fibres => "Fp",
            Ressource::BoisOeuvre => "Bo",
        }
    }

    /// Couleur de l'icône (sRGB).
    pub const fn couleur(self) -> (f32, f32, f32) {
        match self {
            Ressource::Sel => (0.93, 0.93, 0.96),
            Ressource::Silex => (0.62, 0.64, 0.70),
            Ressource::Obsidienne => (0.20, 0.18, 0.26),
            Ressource::PierreDure => (0.42, 0.50, 0.46),
            Ressource::Ocre => (0.80, 0.42, 0.18),
            Ressource::Plantes => (0.52, 0.72, 0.36),
            Ressource::Parures => (0.36, 0.58, 0.90),
            Ressource::Fibres => (0.86, 0.76, 0.52),
            Ressource::BoisOeuvre => (0.46, 0.30, 0.16),
        }
    }

    /// L'exploitation, selon la ressource.
    pub const fn exploitation(self) -> &'static str {
        match self {
            Ressource::Sel => "saline",
            Ressource::Silex | Ressource::Obsidienne | Ressource::Ocre => "minière",
            Ressource::PierreDure | Ressource::Parures => "carrière",
            Ressource::Plantes => "cueillette des plantes",
            Ressource::Fibres => "champ de lin, chasse aux peaux",
            Ressource::BoisOeuvre => "coupe de bois d'œuvre",
        }
    }

    /// Visible dès que la case est explorée (affleurement, ressource vivante) ; sinon, découverte
    /// par la prospection (décision D3).
    pub const fn affleure(self) -> bool {
        !matches!(self, Ressource::PierreDure | Ressource::Parures)
    }

    /// Usage prévu (les livraisons suivantes : besoins de base, luxe, outils, matériaux).
    pub const fn usage(self) -> &'static str {
        match self {
            Ressource::Sel => "besoin de base : salubrité, conservation de la viande et du poisson",
            Ressource::Silex => "outils de pierre (niveau 1)",
            Ressource::Obsidienne => "outils de pierre (niveau 1) et luxe fin",
            Ressource::PierreDure => "outils de pierre (niveau 1) ; meules, mégalithes plus tard",
            Ressource::Ocre => "luxe commun (parure, peinture)",
            Ressource::Plantes => "santé (salubrité)",
            Ressource::Parures => "luxe rare",
            Ressource::Fibres => "besoin de base : vêtements",
            Ressource::BoisOeuvre => "matériau : grands bâtiments, navires",
        }
    }

    pub fn index(self) -> usize {
        Ressource::TOUTES.iter().position(|&r| r == self).unwrap_or(0)
    }

    /// Besoin de base que la ressource couvre (sel, vêtements, santé) : sa demande suit les
    /// habitants (décision B2).
    pub const fn besoin_de_base(self) -> bool {
        matches!(self, Ressource::Sel | Ressource::Fibres | Ressource::Plantes)
    }

    /// Niveau d'outil de la ressource (1 pierre, 2 cuivre, 3 bronze, 4 fer ; décision du 12/11) :
    /// seul le niveau 1 existe au Néolithique.
    pub const fn niveau_outil(self) -> Option<u8> {
        match self {
            Ressource::Silex | Ressource::PierreDure | Ressource::Obsidienne => Some(1),
            _ => None,
        }
    }

    /// Rang de luxe : bien-être d'une sorte couverte (commun +2, fin +4, rare +6 ; décision B3).
    pub const fn luxe(self) -> Option<f64> {
        match self {
            Ressource::Ocre => Some(2.0),
            Ressource::Obsidienne => Some(4.0),
            Ressource::Parures => Some(6.0),
            _ => None,
        }
    }
}

/// Ce que la carte sait d'une case de terre, pour y placer un gisement.
#[derive(Clone, Copy, Debug)]
pub struct LieuGisement {
    pub roche: Roche,
    pub sol: Sol,
    pub terrain: Terrain,
    pub relief: Relief,
    /// Température moyenne annuelle (°C) et précipitations annuelles (mm).
    pub temperature: f32,
    pub pluie: f32,
    /// Au bord de la mer.
    pub cote: bool,
    /// Au bord d'un lac salé ou d'une mer intérieure.
    pub rive_salee: bool,
}

/// Chance qu'une case qui convient porte la ressource (décision D4 : 1 à 3 % des terres environ
/// selon la ressource, à caler par le bilan `bilan_gisements`).
pub fn chance(r: Ressource, l: &LieuGisement) -> f32 {
    let haut = l.relief >= Relief::Hills;
    let montagne = l.relief >= Relief::Rugged;
    let sec_et_chaud = l.pluie < 500.0 && l.temperature > 15.0;
    match r {
        Ressource::Sel => {
            if l.rive_salee {
                0.35
            } else if l.cote && (sec_et_chaud || l.terrain.is_arid()) {
                0.22
            } else if matches!(l.roche, Roche::Calcaire | Roche::Argile | Roche::Gres) && l.pluie < 400.0 && !haut {
                0.04
            } else {
                0.0
            }
        }
        Ressource::Silex => if l.roche == Roche::Calcaire && !montagne { 0.18 } else { 0.0 },
        Ressource::Obsidienne => match l.roche {
            Roche::Volcanique if haut => 0.12,
            Roche::Basalte if haut => 0.05,
            _ => 0.0,
        },
        Ressource::PierreDure => if matches!(l.roche, Roche::Granite | Roche::Basalte) && haut { 0.08 } else { 0.0 },
        Ressource::Ocre => if (matches!(l.roche, Roche::Gres | Roche::Argile) && l.temperature > 12.0) || l.sol == Sol::LessiveTropical { 0.05 } else { 0.0 },
        Ressource::Plantes => match l.terrain {
            Terrain::Mediterranean => 0.08,
            Terrain::Forest | Terrain::Jungle if haut => 0.04,
            _ => 0.0,
        },
        Ressource::Parures => {
            if matches!(l.roche, Roche::Granite | Roche::Volcanique) && montagne {
                0.03
            } else if l.cote && l.temperature < 8.0 && matches!(l.terrain, Terrain::Forest | Terrain::Taiga) {
                0.08
            } else {
                0.0
            }
        }
        Ressource::Fibres => match l.terrain {
            Terrain::Grassland | Terrain::Plains if (500.0..=1_200.0).contains(&l.pluie) => 0.07,
            Terrain::Taiga | Terrain::Tundra => 0.05,
            _ => 0.0,
        },
        Ressource::BoisOeuvre => if matches!(l.terrain, Terrain::Forest | Terrain::Taiga | Terrain::Jungle) && !montagne { 0.04 } else { 0.0 },
    }
}

/// Ordre des tirages : les plus rares d'abord (une case porte au plus un gisement).
pub const ORDRE: [Ressource; N_RESSOURCES] =
    [Ressource::Parures, Ressource::Obsidienne, Ressource::Sel, Ressource::PierreDure, Ressource::Ocre, Ressource::Silex, Ressource::Plantes, Ressource::BoisOeuvre, Ressource::Fibres];

/// Gisement d'une case : la première ressource (dans `ORDRE`) dont le tirage `tirage(r)` (0 à 1,
/// propre à la case et à la ressource) passe sous sa chance.
pub fn gisement(l: &LieuGisement, tirage: impl Fn(Ressource) -> f32) -> Option<Ressource> {
    ORDRE.into_iter().find(|&r| {
        let c = chance(r, l);
        c > 0.0 && tirage(r) < c
    })
}

/// Unités de ressource par tour d'une exploitation pleine (décision D2).
pub const UNITES_PAR_TOUR: f64 = 1.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn lieu() -> LieuGisement {
        LieuGisement { roche: Roche::Argile, sol: Sol::Brun, terrain: Terrain::Grassland, relief: Relief::Flat, temperature: 12.0, pluie: 700.0, cote: false, rive_salee: false }
    }

    #[test]
    fn les_gisements_suivent_la_roche_le_climat_et_les_rivages() {
        let toujours = |_| 0.0;
        let jamais = |_| 1.0;
        // Craie : silex ; arc volcanique en collines : obsidienne ; côte aride : sel
        assert_eq!(gisement(&LieuGisement { roche: Roche::Calcaire, ..lieu() }, toujours), Some(Ressource::Silex));
        assert_eq!(gisement(&LieuGisement { roche: Roche::Volcanique, relief: Relief::Hills, ..lieu() }, toujours), Some(Ressource::Obsidienne));
        assert_eq!(gisement(&LieuGisement { cote: true, terrain: Terrain::Desert, pluie: 100.0, temperature: 25.0, roche: Roche::Gres, ..lieu() }, toujours), Some(Ressource::Sel));
        assert_eq!(gisement(&LieuGisement { rive_salee: true, ..lieu() }, toujours), Some(Ressource::Sel));
        // Prairie arrosée : lin ; forêt : bois d'œuvre ; garrigue : plantes médicinales
        assert_eq!(gisement(&lieu(), toujours), Some(Ressource::Fibres));
        assert_eq!(gisement(&LieuGisement { terrain: Terrain::Forest, ..lieu() }, toujours), Some(Ressource::BoisOeuvre));
        assert_eq!(gisement(&LieuGisement { terrain: Terrain::Mediterranean, pluie: 450.0, ..lieu() }, toujours), Some(Ressource::Plantes));
        // Vieux massif : les parures passent avant la pierre dure
        assert_eq!(gisement(&LieuGisement { roche: Roche::Granite, relief: Relief::Rugged, ..lieu() }, toujours), Some(Ressource::Parures));
        assert_eq!(gisement(&LieuGisement { roche: Roche::Granite, relief: Relief::Rugged, ..lieu() }, |r| if r == Ressource::Parures { 1.0 } else { 0.0 }), Some(Ressource::PierreDure));
        // Le tirage décide
        assert_eq!(gisement(&LieuGisement { roche: Roche::Calcaire, ..lieu() }, jamais), None);
        // Découverte : la pierre dure et les parures se prospectent, le reste affleure
        assert!(!Ressource::PierreDure.affleure() && !Ressource::Parures.affleure() && Ressource::Sel.affleure());
        assert!(Ressource::TOUTES.iter().enumerate().all(|(k, r)| r.index() == k));
    }
}
