//! Construction des ponts par les villes (voir `docs/conception/conception-ponts.md`, étape 2).
//!
//! Le bouton « Pont… » du panneau de production met la carte en mode choix : les emplacements de
//! pont du territoire de la ville sont surlignés (arêtes de rivière dont une case au moins est au
//! territoire, cases de fleuve), celui sous le curseur l'est davantage. Un clic gauche le choisit :
//! la ville produit ce pont, au coût fixé par le débit (`rules::bridge_cost`). Clic droit, nouveau
//! clic sur le bouton ou autre sélection : on annule. Le pont est posé à la fin du tour où la
//! production est terminée (`game::end_turn`).

use crate::feedback::TurnReplay;
use crate::game::{City, GameLog, GameMap, HexPos, LogText, Selection, ShownFog, Territory};
use crate::hex::{HEX_SIZE, Hex};
use crate::map::{BridgeAt, WorldMap};
use crate::pointer::UiPointer;
use crate::rules::{self, Production};
use bevy::prelude::*;

/// Ville qui choisit l'emplacement d'un pont (mode choix de la carte), et emplacement sous le curseur.
#[derive(Resource, Default)]
pub struct BridgePlacement {
    pub city: Option<Entity>,
    pub hovered: Option<BridgeAt>,
    /// Emplacements possibles de la ville (calculés à l'entrée dans le mode choix).
    pub spots: Vec<BridgeAt>,
}

impl BridgePlacement {
    pub fn active(&self) -> bool {
        self.city.is_some()
    }
}

/// Bouton « Pont… » du panneau de production.
#[derive(Component)]
pub struct BridgeButton;

/// Hors du mode choix d'un pont (les clics de la carte reprennent leur rôle habituel).
pub fn not_placing(placement: Res<BridgePlacement>) -> bool {
    !placement.active()
}

pub struct BridgeBuildPlugin;

impl Plugin for BridgeBuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BridgePlacement>().add_systems(
            Update,
            (bridge_button, update_bridge_button, (hover_spot, place_or_cancel).chain().after(crate::input::InputSet), draw_spots_2d),
        );
    }
}

/// Emplacements de pont d'une ville : son territoire, sans les ponts déjà construits ni ceux
/// qu'une autre ville produit.
pub fn city_spots(map: &WorldMap, territory: &Territory, city: Entity, others: impl Iterator<Item = BridgeAt>) -> Vec<BridgeAt> {
    let owned = |h: Hex| territory.0.get(&h) == Some(&city);
    let mut spots = rules::bridge_spots(map, owned, territory.0.iter().filter(|(_, e)| **e == city).map(|(h, _)| *h));
    let taken: Vec<BridgeAt> = others.collect();
    spots.retain(|s| !taken.contains(s));
    spots
}

/// Emplacement sous le curseur : sur un fleuve, la traversée dont la rive est la plus proche du
/// curseur ; sur la terre, l'arête de rivière la plus proche, si le curseur est près du bord de la
/// case. `offset` : position du curseur par rapport au centre de `hex`.
pub fn spot_under(map: &WorldMap, hex: Hex, offset: Vec2, spots: &[BridgeAt]) -> Option<BridgeAt> {
    let hex = map.canonical(hex);
    let angle = offset.y.atan2(offset.x).to_degrees().rem_euclid(360.0);
    let gap = |d: usize| {
        let a = (d as f32 * 60.0 - angle).rem_euclid(360.0);
        a.min(360.0 - a)
    };
    let own: Vec<(usize, BridgeAt)> = spots
        .iter()
        .filter_map(|&at| match at {
            BridgeAt::Fleuve { hex: h, axis } if h == hex => {
                let a = usize::from(axis);
                Some((if gap(a) <= gap(a + 3) { a } else { a + 3 }, at))
            }
            BridgeAt::Edge { hex: h, dir } if h == hex => Some((usize::from(dir), at)),
            BridgeAt::Edge { hex: h, dir } if map.canonical(map.wrap().neighbor(h, usize::from(dir))) == hex => Some(((usize::from(dir) + 3) % 6, at)),
            _ => None,
        })
        .collect();
    if !map.is_fleuve(hex) && offset.length() < HEX_SIZE * 0.35 {
        return None;
    }
    own.into_iter().min_by(|a, b| gap(a.0).total_cmp(&gap(b.0))).map(|(_, at)| at)
}

/// Clic sur le bouton « Pont… » : entre dans le mode choix (ou en sort).
fn bridge_button(
    buttons: Query<&UiPointer, (Changed<UiPointer>, With<BridgeButton>)>,
    selection: Res<Selection>,
    replay: Res<TurnReplay>,
    map: Res<GameMap>,
    territory: Res<Territory>,
    cities: Query<(Entity, &City)>,
    mut placement: ResMut<BridgePlacement>,
) {
    if !buttons.iter().any(|i| *i == UiPointer::Pressed) || replay.playing {
        return;
    }
    if placement.active() {
        *placement = BridgePlacement::default();
        return;
    }
    let Some(city) = selection.0.filter(|e| cities.contains(*e)) else { return };
    let others = cities.iter().filter(|(e, _)| *e != city).filter_map(|(_, c)| match c.state.producing {
        Some(Production::Bridge { at, .. }) => Some(at),
        _ => None,
    });
    let spots = city_spots(&map.0, &territory, city, others);
    if !spots.is_empty() {
        *placement = BridgePlacement { city: Some(city), hovered: None, spots };
    }
}

/// Libellé et visibilité du bouton « Pont… » : caché sans emplacement (ni pont en production).
#[allow(clippy::type_complexity)]
fn update_bridge_button(
    selection: Res<Selection>,
    map: Res<GameMap>,
    territory: Res<Territory>,
    placement: Res<BridgePlacement>,
    cities: Query<(Entity, &City, &HexPos)>,
    mut buttons: Query<(&UiPointer, &mut Node, &mut BackgroundColor, &Children), With<BridgeButton>>,
    mut texts: Query<&mut Text>,
) {
    let Some((city, state, pos)) = selection.0.and_then(|e| cities.get(e).ok()) else { return };
    let others = cities.iter().filter(|(e, _, _)| *e != city).filter_map(|(_, c, _)| match c.state.producing {
        Some(Production::Bridge { at, .. }) => Some(at),
        _ => None,
    });
    let count = if placement.active() { placement.spots.len() } else { city_spots(&map.0, &territory, city, others).len() };
    let production = rules::city_yields(&map.0, pos.0, &state.worked).production.max(1);
    let (label, active) = match state.state.producing {
        Some(item @ Production::Bridge { .. }) => {
            let turns = ((item.cost() - state.state.production_stock).max(0) as f32 / production as f32).ceil() as i32;
            (format!("     {}  —  {} ({} tours)  ·  autre emplacement : clic", item.name(), item.cost(), turns), true)
        }
        _ if placement.active() => ("     Pont : cliquez un emplacement surligné (clic droit : annuler)".to_string(), true),
        _ => (format!("     Pont…  —  choisir l'emplacement ({count} possible{})", if count > 1 { "s" } else { "" }), false),
    };
    let shown = count > 0 || matches!(state.state.producing, Some(Production::Bridge { .. }));
    for (interaction, mut node, mut bg, children) in &mut buttons {
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let color = if active {
            crate::ui::PRODUCE_ACTIVE
        } else if *interaction == UiPointer::Hovered {
            crate::ui::PRODUCE_HOVER
        } else {
            crate::ui::PRODUCE_BG
        };
        if bg.0 != color {
            bg.0 = color;
        }
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok())
            && t.0 != label
        {
            t.0 = label.clone();
        }
    }
}

/// Emplacement sous le curseur, en mode choix.
fn hover_spot(hovered: Res<crate::input::HoveredHex>, point: Res<crate::input::HoveredPoint>, map: Res<GameMap>, mut placement: ResMut<BridgePlacement>) {
    if !placement.active() {
        return;
    }
    let spot = hovered.0.zip(point.0).and_then(|(h, p)| {
        let (cx, cy) = map.0.wrap().nearest(Hex::from_world(p.x, p.y), h).to_world();
        spot_under(&map.0, h, p - Vec2::new(cx, cy), &placement.spots)
    });
    if placement.hovered != spot {
        placement.hovered = spot;
    }
}

/// Clic gauche : la ville produit le pont surligné ; clic droit, ou ville désélectionnée : annulé.
fn place_or_cancel(
    mouse: Res<ButtonInput<MouseButton>>,
    selection: Res<Selection>,
    map: Res<GameMap>,
    over_ui: Res<crate::input::PointerOverUi>,
    mut placement: ResMut<BridgePlacement>,
    mut cities: Query<(&mut City, &HexPos)>,
    mut log: ResMut<GameLog>,
    mut pilot: ResMut<crate::input::Pilot>,
) {
    let Some(city_entity) = placement.city else { return };
    // (clic de la souris, ou du mode joueur pilotable)
    let click = pilot.click.take();
    let pressed = |b: MouseButton| mouse.just_pressed(b) || click == Some(b);
    if selection.0 != Some(city_entity) || pressed(MouseButton::Right) {
        *placement = BridgePlacement::default();
        return;
    }
    if !pressed(MouseButton::Left) || over_ui.0 {
        return;
    }
    let Some(at) = placement.hovered else { return };
    if let Ok((mut city, pos)) = cities.get_mut(city_entity) {
        let item = Production::Bridge { at, cost: rules::bridge_cost(&map.0, at) };
        city.state.producing = Some(item);
        log.push(LogText::new().city(&city.state.name, city_entity, pos.0).text(format!(" produit désormais : {} ({}).", item.name(), item.cost())));
    }
    *placement = BridgePlacement::default();
}

/// Emplacements surlignés en 2D (planche jaune pâle, celle du curseur en blanc).
fn draw_spots_2d(placement: Res<BridgePlacement>, map: Res<GameMap>, fog: Res<ShownFog>, mode: Res<crate::view3d::ViewMode>, mut gizmos: Gizmos) {
    if !placement.active() || mode.is_3d {
        return;
    }
    for &at in &placement.spots {
        let (e0, e1) = at.ends(map.0.wrap());
        if !(fog.explored.contains(&e0) || fog.explored.contains(&e1)) {
            continue;
        }
        let (a, b) = crate::render::bridge_ends_2d(&map.0, at);
        let hovered = placement.hovered == Some(at);
        let color = if hovered { Color::srgb(1.0, 1.0, 1.0) } else { Color::srgba(1.0, 0.9, 0.45, 0.85) };
        let side = (b - a).normalize_or_zero().perp() * if hovered { 3.0 } else { 1.5 };
        for k in [-1.0f32, 0.0, 1.0] {
            gizmos.line_2d(a + side * k, b + side * k, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;

    #[test]
    fn emplacement_sous_le_curseur() {
        let mut m = WorldMap::filled(10, 10, Terrain::Grassland);
        let a = Hex::from_offset(4, 4);
        m.set_river(a, 0, 300.0);
        m.set_river(a, 3, 300.0);
        let f = Hex::from_offset(7, 6);
        m.set_fleuve(f);
        let spots = rules::bridge_spots(&m, |_| true, m.hexes().collect::<Vec<_>>());
        let east = m.bridge_site(a, 0).unwrap();
        let west = m.bridge_site(a, 3).unwrap();
        assert!(spots.contains(&east) && spots.contains(&west));
        assert_eq!(spot_under(&m, a, Vec2::new(20.0, 2.0), &spots), Some(east));
        assert_eq!(spot_under(&m, a, Vec2::new(-20.0, -3.0), &spots), Some(west));
        assert_eq!(spot_under(&m, a, Vec2::new(2.0, 1.0), &spots), None, "au centre de la case : rien");
        // De l'autre côté de l'arête, le même pont
        assert_eq!(spot_under(&m, a + crate::hex::DIRECTIONS[0], Vec2::new(-20.0, 0.0), &spots), Some(east));
        // Fleuve : la traversée dont la rive est vers le curseur
        let toward = |d: usize| Vec2::from_angle((d as f32 * 60.0).to_radians()) * 10.0;
        for d in 0..6 {
            if let Some(at) = spot_under(&m, f, toward(d), &spots) {
                assert_eq!(at, BridgeAt::Fleuve { hex: f, axis: (d % 3) as u8 });
            }
        }
        // Un pont construit n'est plus un emplacement ; le coût suit le débit
        m.set_bridge(east, true);
        assert!(!rules::bridge_spots(&m, |_| true, m.hexes().collect::<Vec<_>>()).contains(&east));
        let small = rules::bridge_cost(&m, east);
        assert!((rules::BRIDGE_COST.0..=rules::BRIDGE_COST.1).contains(&small));
        assert!((rules::BRIDGE_COST.0..=rules::BRIDGE_COST.1).contains(&rules::bridge_cost(&m, BridgeAt::Fleuve { hex: f, axis: 1 })));
    }
}
