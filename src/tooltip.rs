//! Contenu de l'infobulle de survol : rubriques affichées, réglables pour chaque carte (le
//! débit sur la carte des débits, le bassin sur celle des bassins versants…). Par défaut, les
//! informations utiles au jeu : terrain, climat, rendements et coût de déplacement.

use crate::render::MapLayer;
use bevy::prelude::*;

/// Rubrique de l'infobulle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Info {
    Terrain,
    Altitude,
    Climate,
    Temperature,
    Rain,
    Latitude,
    Yields,
    Moves,
    Waterways,
    Lake,
    Discharge,
    Basin,
    Network,
    Plate,
}

impl Info {
    pub const ALL: [Info; 14] = [
        Info::Terrain,
        Info::Altitude,
        Info::Climate,
        Info::Temperature,
        Info::Rain,
        Info::Latitude,
        Info::Yields,
        Info::Moves,
        Info::Waterways,
        Info::Lake,
        Info::Discharge,
        Info::Basin,
        Info::Network,
        Info::Plate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Info::Terrain => "Terrain",
            Info::Altitude => "Altitude (m) et relief",
            Info::Climate => "Climat",
            Info::Temperature => "Température",
            Info::Rain => "Précipitations",
            Info::Latitude => "Latitude",
            Info::Yields => "Rendements",
            Info::Moves => "Coût de déplacement",
            Info::Waterways => "Cours d'eau (traversée, eau douce)",
            Info::Lake => "Lac ou mer intérieure",
            Info::Discharge => "Débit de la case",
            Info::Basin => "Bassin versant",
            Info::Network => "Affluents et embouchure",
            Info::Plate => "Plaque tectonique",
        }
    }

    fn index(self) -> usize {
        Info::ALL.iter().position(|&i| i == self).unwrap()
    }

    /// Affichée par défaut sur cette carte.
    fn default_on(self, layer: MapLayer) -> bool {
        match self {
            Info::Terrain | Info::Climate | Info::Yields | Info::Moves => true,
            Info::Altitude => matches!(layer, MapLayer::Altitude | MapLayer::Relief),
            Info::Temperature | Info::Rain | Info::Latitude => layer == MapLayer::Climate,
            Info::Waterways | Info::Lake | Info::Network => layer.hydrologic(),
            Info::Discharge => layer == MapLayer::Flow,
            Info::Basin => layer == MapLayer::Basins,
            Info::Plate => layer == MapLayer::Plates,
        }
    }
}

fn layer_index(layer: MapLayer) -> usize {
    MapLayer::ALL.iter().position(|&l| l == layer).unwrap()
}

/// Rubriques affichées, pour chaque carte.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct TooltipConfig {
    shown: [[bool; Info::ALL.len()]; MapLayer::ALL.len()],
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self { shown: MapLayer::ALL.map(|layer| Info::ALL.map(|info| info.default_on(layer))) }
    }
}

impl TooltipConfig {
    pub fn shows(&self, layer: MapLayer, info: Info) -> bool {
        self.shown[layer_index(layer)][info.index()]
    }

    pub fn toggle(&mut self, layer: MapLayer, info: Info) {
        let cell = &mut self.shown[layer_index(layer)][info.index()];
        *cell = !*cell;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_defaut_l_essentiel_et_selon_la_carte() {
        let c = TooltipConfig::default();
        for layer in MapLayer::ALL {
            for info in [Info::Terrain, Info::Climate, Info::Yields, Info::Moves] {
                assert!(c.shows(layer, info));
            }
        }
        assert!(!c.shows(MapLayer::Normal, Info::Discharge));
        assert!(c.shows(MapLayer::Flow, Info::Discharge));
        assert!(c.shows(MapLayer::Basins, Info::Basin));
        assert!(c.shows(MapLayer::Climate, Info::Temperature));
        assert!(!c.shows(MapLayer::Normal, Info::Temperature));
        let mut c = c;
        c.toggle(MapLayer::Normal, Info::Temperature);
        assert!(c.shows(MapLayer::Normal, Info::Temperature));
        assert!(!c.shows(MapLayer::Relief, Info::Temperature));
    }
}
