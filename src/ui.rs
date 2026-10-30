//! Interface : barre du haut, panneau de sélection, infos de case, journal, bouton de fin de tour.

use crate::game::{City, EndTurnRequested, GameLog, GameMap, GameState, Importance, Selection, Unit};
use crate::input::{HoverPath, HoveredHex, OrderPath};
use crate::rules::{self, UnitKind};
use crate::render::{
    BASIN_MUTED, CLIMATE_COAST, CLIMATE_OCEAN, HYDRO_LAKE, HYDRO_OCEAN, MapLayer, TOPO_COAST, TOPO_OCEAN,
    basin_color, flow_color, rain_color,
};
use bevy::prelude::*;

#[derive(Component)]
struct TopBarText;
#[derive(Component)]
struct SelectionText;
#[derive(Component)]
struct HoverText;
/// Journal (en haut à droite) : une ligne par message.
#[derive(Component)]
struct LogList;
/// Ligne du journal : message, instant d'apparition, importance.
#[derive(Component)]
struct LogRow {
    id: u64,
    born: f64,
    importance: Importance,
}
/// Liseré coloré et texte d'une ligne du journal.
#[derive(Component)]
struct LogAccent;
#[derive(Component)]
struct LogLine;
#[derive(Component)]
struct EndTurnButton;
/// Bouton de bascule 2D / 3D.
#[derive(Component)]
struct ViewToggleButton;
/// Bouton « Carte » : ouvre le choix de la carte affichée.
#[derive(Component)]
struct MapLayerButton;
/// Menu de choix de la carte (ouvert par le bouton).
#[derive(Component)]
struct MapLayerMenu;
/// Option du menu de carte.
#[derive(Component)]
struct MapLayerOption(MapLayer);
/// Légende d'une carte (affichée avec elle).
#[derive(Component)]
struct Legend(MapLayer);
/// Lignes de la légende des bassins : les grands bassins découverts.
#[derive(Component)]
struct BasinLegendRows;
/// Panneau de production (visible quand une ville est sélectionnée).
#[derive(Component)]
struct ProductionPanel;
/// Bouton « produire telle unité ».
#[derive(Component)]
struct ProduceButton(UnitKind);

const PANEL_BG: Color = Color::srgba(0.05, 0.06, 0.09, 0.88);
const BUTTON_BG: Color = Color::srgb(0.16, 0.35, 0.70);
const BUTTON_HOVER: Color = Color::srgb(0.22, 0.45, 0.85);
const PRODUCE_BG: Color = Color::srgba(0.16, 0.18, 0.24, 0.95);
const PRODUCE_HOVER: Color = Color::srgb(0.24, 0.28, 0.38);
const PRODUCE_ACTIVE: Color = Color::srgb(0.18, 0.40, 0.72);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    end_turn_button,
                    view_toggle_button,
                    map_layer_controls,
                    wiki_button_hover,
                    production_buttons,
                    update_production_panel,
                    update_top_bar,
                    update_selection_panel,
                    update_hover_text,
                    update_log,
                    animate_log.after(update_log),
                    update_basin_legend,
                ),
            );
    }
}

fn panel(node: Node) -> impl Bundle {
    let node = Node { border_radius: BorderRadius::all(Val::Px(6.0)), ..node };
    (node, BackgroundColor(PANEL_BG), Interaction::default())
}

fn text(content: &str, size: f32) -> impl Bundle {
    (Text::new(content), TextFont::from_font_size(size), TextColor(Color::srgb(0.92, 0.92, 0.95)))
}

fn setup_ui(mut commands: Commands) {
    // Barre du haut
    commands.spawn((
        panel(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        }),
        children![(TopBarText, text("", 18.0))],
    ));

    // Journal (en haut à droite) : les lignes sont ajoutées par `update_log`
    commands.spawn((
        LogList,
        panel(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            width: Val::Px(440.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::all(Val::Px(5.0)),
            ..default()
        }),
    ));

    // Panneau de production (à gauche, sous la barre du haut), masqué hors sélection de ville
    commands
        .spawn((
            ProductionPanel,
            panel(Node {
                position_type: PositionType::Absolute,
                top: Val::Px(52.0),
                left: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(8.0)),
                display: Display::None,
                ..default()
            }),
        ))
        .with_children(|p| {
            p.spawn(text("Production (clic ou touches 1 à 9)", 14.0));
            for kind in UnitKind::ALL {
                p.spawn((
                    ProduceButton(kind),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(PRODUCE_BG),
                    children![text(kind.name(), 14.0)],
                ));
            }
        });

    // Panneau de sélection (en bas à gauche)
    commands.spawn((
        panel(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(8.0),
            min_width: Val::Px(320.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        }),
        children![(SelectionText, text("", 16.0))],
    ));

    // Infos de case + bouton de fin de tour (en bas à droite)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(8.0),
            ..default()
        },
        children![
            (
                panel(Node { padding: UiRect::all(Val::Px(10.0)), ..default() }),
                children![(HoverText, text("", 14.0))],
            ),
            topo_legend(),
            relief_legend(),
            climate_legend(),
            basin_legend(),
            flow_legend(),
            network_legend(),
            plates_legend(),
            currents_legend(),
            resources_legend(),
            (
                MapLayerMenu,
                panel(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    display: Display::None,
                    ..default()
                }),
                children![
                    map_option(MapLayer::Normal, "Carte normale"),
                    map_option(MapLayer::Altitude, "Carte de l'altitude"),
                    map_option(MapLayer::Relief, "Carte du relief"),
                    map_option(MapLayer::Climate, "Carte du climat"),
                    map_option(MapLayer::Basins, "Carte des bassins versants"),
                    map_option(MapLayer::Flow, "Carte des débits"),
                    map_option(MapLayer::Network, "Carte du réseau hydrographique"),
                    map_option(MapLayer::Plates, "Carte des plaques tectoniques"),
                    map_option(MapLayer::Currents, "Carte des courants marins"),
                    map_option(MapLayer::Resources, "Carte des ressources"),
                ],
            ),
            (
                crate::wiki::WikiButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(PRODUCE_BG),
                children![text("Wiki (F1)", 16.0)],
            ),
            (
                crate::search::SearchButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(PRODUCE_BG),
                children![text("Rechercher (Ctrl+F)", 16.0)],
            ),
            (
                MapLayerButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(PRODUCE_BG),
                children![text("Carte : normale (T)", 16.0)],
            ),
            (
                crate::options::OptionsButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(PRODUCE_BG),
                children![text("Options de carte (O)", 16.0)],
            ),
            (
                ViewToggleButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(PRODUCE_BG),
                children![text("Vue 3D (V)", 16.0)],
            ),
            (
                EndTurnButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(12.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(BUTTON_BG),
                children![text("Fin du tour (Entrée)", 18.0)],
            ),
        ],
    ));
}

/// Survol du bouton Wiki.
fn wiki_button_hover(mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<crate::wiki::WikiButton>)>) {
    for (interaction, mut bg) in &mut buttons {
        bg.0 = if *interaction == Interaction::None { PRODUCE_BG } else { PRODUCE_HOVER };
    }
}

/// Une option du menu de carte.
fn map_option(layer: MapLayer, label: &str) -> impl Bundle {
    (
        MapLayerOption(layer),
        Button,
        Node {
            padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(PRODUCE_BG),
        children![text(label, 15.0)],
    )
}

/// Une ligne de légende : pastille de couleur et libellé.
fn legend_row((r, g, b): (f32, f32, f32), label: String) -> impl Bundle {
    (
        Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() },
        children![
            (
                Node { width: Val::Px(14.0), height: Val::Px(14.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
                BackgroundColor(Color::srgb(r, g, b)),
            ),
            (Text::new(label), TextFont::from_font_size(13.0), TextColor(Color::srgb(0.92, 0.92, 0.95))),
        ],
    )
}

/// Légende de la carte du climat : précipitations et vents.
fn climate_legend() -> impl Bundle {
    (
        Legend(MapLayer::Climate),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        }),
        Children::spawn((
            Spawn(text("Précipitations (par an)", 14.0)),
            Spawn(legend_row(rain_color(2_000.0), "2 000 mm et plus".into())),
            Spawn(legend_row(rain_color(1_000.0), "1 000 mm".into())),
            Spawn(legend_row(rain_color(500.0), "500 mm".into())),
            Spawn(legend_row(rain_color(250.0), "250 mm".into())),
            Spawn(legend_row(rain_color(0.0), "Presque rien".into())),
            Spawn(legend_row(CLIMATE_COAST, "Eaux peu profondes".into())),
            Spawn(legend_row(CLIMATE_OCEAN, "Haute mer".into())),
            Spawn(text("Flèches : vents dominants", 13.0)),
        )),
    )
}

/// Légende de la carte de l'altitude : teintes par altitude (m).
fn topo_legend() -> impl Bundle {
    use crate::render::altitude_color;
    (
        Legend(MapLayer::Altitude),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        }),
        Children::spawn((
            Spawn(text("Altitude", 14.0)),
            Spawn(legend_row(altitude_color(4_500.0), "4 500 m et plus".into())),
            Spawn(legend_row(altitude_color(3_400.0), "3 400 m".into())),
            Spawn(legend_row(altitude_color(2_400.0), "2 400 m".into())),
            Spawn(legend_row(altitude_color(1_500.0), "1 500 m".into())),
            Spawn(legend_row(altitude_color(800.0), "800 m".into())),
            Spawn(legend_row(altitude_color(300.0), "300 m".into())),
            Spawn(legend_row(altitude_color(0.0), "niveau de la mer".into())),
            Spawn(legend_row(TOPO_COAST, "Eaux peu profondes (moins de 200 m), lac".into())),
            Spawn(legend_row(TOPO_OCEAN, "Haute mer (200 à 3 000 m)".into())),
            Spawn(legend_row(crate::render::TOPO_ABYSS, "Abysses (plus de 3 000 m)".into())),
        )),
    )
}

/// Légende de la carte du relief : plat, vallonné, accidenté, montagne, pic.
fn relief_legend() -> impl Bundle {
    use crate::map::Relief;
    let label = |r: Relief| match r {
        Relief::Flat => "Plat".to_string(),
        Relief::Hills => "Vallonné (collines)".to_string(),
        Relief::Rugged => "Accidenté".to_string(),
        Relief::Mountain => "Montagne (franchissable)".to_string(),
        Relief::Peak => "Pic (infranchissable)".to_string(),
    };
    (
        Legend(MapLayer::Relief),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        }),
        Children::spawn((
            Spawn(text("Relief", 14.0)),
            Spawn(legend_row(Relief::Peak.color(), label(Relief::Peak))),
            Spawn(legend_row(Relief::Mountain.color(), label(Relief::Mountain))),
            Spawn(legend_row(Relief::Rugged.color(), label(Relief::Rugged))),
            Spawn(legend_row(Relief::Hills.color(), label(Relief::Hills))),
            Spawn(legend_row(Relief::Flat.color(), label(Relief::Flat))),
            Spawn(legend_row(TOPO_COAST, "Eaux peu profondes, lac".into())),
            Spawn(legend_row(TOPO_OCEAN, "Haute mer".into())),
        )),
    )
}

/// Légende de la carte des bassins versants : les grands bassins découverts (mis à jour avec
/// l'exploration), puis les petits bassins et l'eau.
fn basin_legend() -> impl Bundle {
    (
        Legend(MapLayer::Basins),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            max_width: Val::Px(320.0),
            ..default()
        }),
        Children::spawn((
            Spawn(text("Bassins versants", 14.0)),
            Spawn((BasinLegendRows, Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })),
            Spawn(legend_row(BASIN_MUTED[1], "Petits bassins (côtiers ou fermés)".into())),
            Spawn(legend_row(HYDRO_LAKE, "Lac".into())),
            Spawn(legend_row(HYDRO_OCEAN, "Mer".into())),
            Spawn(text("Une couleur : toute l'eau qui y tombe rejoint la même sortie", 12.0)),
            Spawn(text("Survol d'un nom : le bassin ressort ; clic : la carte y va", 12.0)),
        )),
    )
}

/// Nombre de grands bassins listés dans la légende.
const BASIN_LEGEND_ROWS: usize = 10;

/// Grands bassins découverts dans la légende : nom du fleuve principal et étendue.
fn update_basin_legend(
    mut commands: Commands,
    layer: Res<MapLayer>,
    map: Res<GameMap>,
    fog: Res<crate::game::ShownFog>,
    rows: Single<(Entity, Option<&Children>), With<BasinLegendRows>>,
) {
    if *layer != MapLayer::Basins || !(layer.is_changed() || fog.is_changed()) {
        return;
    }
    let basins = map.0.basins();
    let mut found: Vec<usize> = fog
        .explored
        .iter()
        .filter_map(|&h| map.0.contains(h).then(|| basins.basin_of[map.0.cell(h)]).flatten())
        .map(|b| b as usize)
        .filter(|&b| basins.basins[b].major())
        .collect();
    found.sort_unstable();
    found.dedup();
    found.sort_by_key(|&b| std::cmp::Reverse(basins.basins[b].cells));
    let (entity, children) = *rows;
    for &child in children.into_iter().flatten() {
        commands.entity(child).despawn();
    }
    if found.is_empty() {
        commands.entity(entity).with_child(text("Aucun grand bassin découvert", 12.0));
    }
    for &b in found.iter().take(BASIN_LEGEND_ROWS) {
        let basin = &basins.basins[b];
        let name = basins.basin_name(basin).unwrap_or("?");
        commands.entity(entity).with_child(basin_row(b, basin_color(basin.color), format!("{name} ({} cases)", basin.cells)));
    }
    if found.len() > BASIN_LEGEND_ROWS {
        commands.entity(entity).with_child(text(&format!("… et {} autres", found.len() - BASIN_LEGEND_ROWS), 12.0));
    }
}

/// Ligne d'un grand bassin dans la légende : son survol fait ressortir le bassin sur la carte,
/// son clic y amène la caméra.
fn basin_row(basin: usize, (r, g, b): (f32, f32, f32), label: String) -> impl Bundle {
    use crate::search::{HoverColors, MapTarget, TargetButton};
    (
        TargetButton::new(MapTarget::Basin(basin)),
        Button,
        HoverColors { normal: Color::NONE, hover: PRODUCE_HOVER },
        BackgroundColor(Color::NONE),
        Node {
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
            margin: UiRect::horizontal(Val::Px(-4.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        children![
            (
                Node { width: Val::Px(14.0), height: Val::Px(14.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
                BackgroundColor(Color::srgb(r, g, b)),
            ),
            (Text::new(label), TextFont::from_font_size(13.0), TextColor(Color::srgb(0.92, 0.92, 0.95))),
        ],
    )
}

/// Légende de la carte des débits.
fn flow_legend() -> impl Bundle {
    use crate::hydro::{FLEUVE_M3S, RIVER_M3S};
    (
        Legend(MapLayer::Flow),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        }),
        Children::spawn((
            Spawn(text("Débit qui traverse la case", 14.0)),
            Spawn(legend_row(flow_color(50_000.0), "50 000 m³/s et plus".into())),
            Spawn(legend_row(flow_color(10_000.0), "10 000 m³/s".into())),
            Spawn(legend_row(flow_color(FLEUVE_M3S), format!("{} m³/s : fleuve", round_flow(FLEUVE_M3S)))),
            Spawn(legend_row(flow_color(1_000.0), "1 000 m³/s".into())),
            Spawn(legend_row(flow_color(RIVER_M3S), format!("{} m³/s : rivière", round_flow(RIVER_M3S)))),
            Spawn(legend_row(flow_color(10.0), "10 m³/s et moins".into())),
            Spawn(legend_row(HYDRO_LAKE, "Lac".into())),
            Spawn(legend_row(HYDRO_OCEAN, "Mer".into())),
            Spawn(text("Repères : Charente 100, Rhône 1 700, Congo 41 000", 12.0)),
        )),
    )
}

/// Légende de la carte du réseau hydrographique.
fn network_legend() -> impl Bundle {
    use crate::highlight::LEVEL_COLORS;
    use crate::render::{NETWORK_LAKE, NETWORK_OCEAN};
    (
        Legend(MapLayer::Network),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            max_width: Val::Px(320.0),
            ..default()
        }),
        Children::spawn((
            Spawn(text("Rang par rapport au déversoir", 14.0)),
            Spawn(legend_row(LEVEL_COLORS[0], "Se jette dans la mer (ou un lac salé, une mer intérieure)".into())),
            Spawn(legend_row(LEVEL_COLORS[1], "Affluent".into())),
            Spawn(legend_row(LEVEL_COLORS[2], "Affluent d'un affluent".into())),
            Spawn(legend_row(LEVEL_COLORS[3], "3e rang".into())),
            Spawn(legend_row(LEVEL_COLORS[4], "4e rang et au-delà".into())),
            Spawn(legend_row(NETWORK_LAKE, "Lac (rang du cours qui en sort)".into())),
            Spawn(legend_row(NETWORK_OCEAN, "Mer".into())),
            Spawn(text("Traits épais : fleuves ; fins : rivières ; chevrons : sens du courant", 12.0)),
        )),
    )
}

/// Légende de la carte des courants marins : température de la mer, couleur des flèches.
fn currents_legend() -> impl Bundle {
    use crate::render::{COLD_CURRENT, MILD_CURRENT, WARM_CURRENT, sea_temperature_color};
    let swatch = |c: Color| {
        let s = c.to_srgba();
        (s.red, s.green, s.blue)
    };
    (
        Legend(MapLayer::Currents),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            max_width: Val::Px(330.0),
            ..default()
        }),
        Children::spawn((
            Spawn(text("Température de la mer", 14.0)),
            Spawn(legend_row(sea_temperature_color(28.0), "28 °C".into())),
            Spawn(legend_row(sea_temperature_color(20.0), "20 °C".into())),
            Spawn(legend_row(sea_temperature_color(12.0), "12 °C".into())),
            Spawn(legend_row(sea_temperature_color(4.0), "4 °C".into())),
            Spawn(legend_row(sea_temperature_color(-2.0), "−2 °C".into())),
            Spawn(text("Flèches : sens et vitesse des courants", 13.0)),
            Spawn(legend_row(swatch(WARM_CURRENT), "courant chaud (eau plus chaude que la latitude)".into())),
            Spawn(legend_row(swatch(COLD_CURRENT), "courant froid".into())),
            Spawn(legend_row(swatch(MILD_CURRENT), "eau à la température de la latitude".into())),
            Spawn(text("Les courants froids rafraîchissent et assèchent les côtes (déserts côtiers, brouillards) ; les courants chauds les adoucissent et les arrosent. Leur rencontre enrichit la pêche.", 12.0)),
        )),
    )
}

/// Légende de la carte des ressources.
fn resources_legend() -> impl Bundle {
    (
        Legend(MapLayer::Resources),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            max_width: Val::Px(320.0),
            ..default()
        }),
        Children::spawn((
            Spawn(text("Ressources", 14.0)),
            Spawn((
                Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() },
                children![
                    (crate::labels::FishIcon, Node { width: Val::Px(18.0), height: Val::Px(18.0), ..default() }),
                    (Text::new(format!("Banc de poissons : +{} nourriture", crate::map::RICH_WATERS_FOOD)), TextFont::from_font_size(13.0), TextColor(Color::srgb(0.92, 0.92, 0.95))),
                ],
            )),
            Spawn(text("Survol de l'icône ou du banc : sa zone ; clic : la carte y va", 12.0)),
        )),
    )
}

/// Légende de la carte des plaques tectoniques.
fn plates_legend() -> impl Bundle {
    use crate::plates::Boundary;
    use crate::render::{HOTSPOT, PLATE_CONTINENTAL, PLATE_OCEANIC};
    let swatch = |c: Color| {
        let s = c.to_srgba();
        (s.red, s.green, s.blue)
    };
    (
        Legend(MapLayer::Plates),
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            max_width: Val::Px(330.0),
            ..default()
        }),
        Children::spawn((
            Spawn(text("Plaques tectoniques", 14.0)),
            Spawn(legend_row(PLATE_CONTINENTAL[0], "Plaque continentale (teintes chaudes)".into())),
            Spawn(legend_row(PLATE_OCEANIC[0], "Plaque océanique (teintes froides)".into())),
            Spawn(text("Frontières : les plaques…", 13.0)),
            Spawn(legend_row(Boundary::Collision.color(), "se percutent : collision de continents".into())),
            Spawn(legend_row(Boundary::Subduction.color(), "se rapprochent : subduction (océan sous continent)".into())),
            Spawn(legend_row(Boundary::IslandArc.color(), "se rapprochent : arc d'îles (deux océans)".into())),
            Spawn(legend_row(Boundary::Rift.color(), "s'écartent : rift continental".into())),
            Spawn(legend_row(Boundary::Ridge.color(), "s'écartent : dorsale".into())),
            Spawn(legend_row(Boundary::Transform.color(), "glissent : faille transformante".into())),
            Spawn(legend_row(swatch(HOTSPOT), "Point chaud (volcans loin des frontières)".into())),
            Spawn(text("Flèches : sens et vitesse du mouvement (1 à 9 cm/an). Les continents sont sur les plaques continentales ; les montagnes naissent là où les plaques se rapprochent.", 12.0)),
        )),
    )
}

/// Choix de la carte : le bouton ouvre ou ferme le menu, une option change la carte (T : bascule
/// directe) ; chaque légende n'apparaît qu'avec sa carte.
fn map_layer_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut layer: ResMut<MapLayer>,
    mut button: Query<(Ref<Interaction>, &mut BackgroundColor, &Children), (With<MapLayerButton>, Without<MapLayerOption>)>,
    mut options: Query<(Ref<Interaction>, &MapLayerOption, &mut BackgroundColor), Without<MapLayerButton>>,
    mut menu: Single<&mut Node, (With<MapLayerMenu>, Without<Legend>)>,
    mut legends: Query<(&Legend, &mut Node), Without<MapLayerMenu>>,
    mut texts: Query<&mut Text>,
) {
    if keys.just_pressed(KeyCode::KeyT) {
        *layer = layer.next();
    }
    let mut open = menu.display == Display::Flex;
    for (interaction, mut bg, children) in &mut button {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            open = !open;
        }
        let color = if *interaction == Interaction::None { PRODUCE_BG } else { PRODUCE_HOVER };
        if bg.0 != color {
            bg.0 = color;
        }
        let label = format!("Carte : {} (T)", layer.short_name());
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok())
            && t.0 != label
        {
            t.0 = label;
        }
    }
    for (interaction, option, mut bg) in &mut options {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            if *layer != option.0 {
                *layer = option.0;
            }
            open = false;
        }
        let color = if *layer == option.0 {
            PRODUCE_ACTIVE
        } else if *interaction == Interaction::Hovered {
            PRODUCE_HOVER
        } else {
            PRODUCE_BG
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
    let menu_display = if open { Display::Flex } else { Display::None };
    if menu.display != menu_display {
        menu.display = menu_display;
    }
    for (legend, mut node) in &mut legends {
        let display = if *layer == legend.0 { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

fn end_turn_button(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<EndTurnButton>)>,
    mut end_turn: ResMut<EndTurnRequested>,
) {
    for (interaction, mut bg) in &mut buttons {
        match interaction {
            Interaction::Pressed => end_turn.0 = true,
            Interaction::Hovered => bg.0 = BUTTON_HOVER,
            Interaction::None => bg.0 = BUTTON_BG,
        }
    }
}

/// Clic sur un bouton de production : change la production de la ville sélectionnée.
fn production_buttons(
    buttons: Query<(&Interaction, &ProduceButton), Changed<Interaction>>,
    selection: Res<Selection>,
    mut cities: Query<&mut City>,
    mut log: ResMut<GameLog>,
) {
    let Some(mut city) = selection.0.and_then(|e| cities.get_mut(e).ok()) else { return };
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed && city.state.producing != Some(button.0) {
            city.state.producing = Some(button.0);
            log.push(format!("{} produit désormais : {}.", city.state.name, button.0.name()));
        }
    }
}

/// Affiche le panneau de production pour la ville sélectionnée : coût, tours, statistiques.
fn update_production_panel(
    selection: Res<Selection>,
    map: Res<GameMap>,
    cities: Query<(&City, &crate::game::HexPos)>,
    mut panel: Single<&mut Node, With<ProductionPanel>>,
    mut buttons: Query<(&ProduceButton, &Interaction, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let city = selection.0.and_then(|e| cities.get(e).ok());
    let display = if city.is_some() { Display::Flex } else { Display::None };
    if panel.display != display {
        panel.display = display;
    }
    let Some((city, pos)) = city else { return };
    let production = rules::city_yields(&map.0, pos.0, &city.worked).production.max(1);
    for (i, (button, interaction, mut bg, children)) in buttons.iter_mut().enumerate() {
        let kind = button.0;
        let st = kind.stats();
        let remaining = (kind.cost() - if city.state.producing == Some(kind) { city.state.production_stock } else { 0 }).max(0);
        let turns = (remaining as f32 / production as f32).ceil() as i32;
        let combat = if st.ranged_attack > 0 {
            format!(
                "  ·  tir {} (portée {}) / déf {}  ·  {} PM",
                st.ranged_attack, st.range, st.melee_defense, st.moves
            )
        } else if kind.is_military() {
            format!("  ·  att {} / déf {}  ·  {} PM", st.melee_attack, st.melee_defense, st.moves)
        } else {
            format!("  ·  {} PM", st.moves)
        };
        let key = if i < 9 { format!("[{}] ", i + 1) } else { "     ".into() };
        let label = format!("{key}{}  —  {} ({} tours){combat}", kind.name(), kind.cost(), turns);
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok())
            && t.0 != label
        {
            t.0 = label;
        }
        let color = if city.state.producing == Some(kind) {
            PRODUCE_ACTIVE
        } else if *interaction == Interaction::Hovered {
            PRODUCE_HOVER
        } else {
            PRODUCE_BG
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

/// Bouton de vue : un clic bascule entre 2D et 3D ; le libellé indique la vue proposée.
fn view_toggle_button(
    mut buttons: Query<(Ref<Interaction>, &mut BackgroundColor, &Children), With<ViewToggleButton>>,
    mut mode: ResMut<crate::view3d::ViewMode>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, mut bg, children) in &mut buttons {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            mode.is_3d = !mode.is_3d;
        }
        let color = if *interaction == Interaction::None { PRODUCE_BG } else { PRODUCE_HOVER };
        if bg.0 != color {
            bg.0 = color;
        }
        let label = if mode.is_3d { "Vue 2D (V)" } else { "Vue 3D (V)" };
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok())
            && t.0 != label
        {
            t.0 = label.to_string();
        }
    }
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        text.0 = value;
    }
}

fn update_top_bar(
    state: Res<GameState>,
    map: Res<GameMap>,
    cities: Query<(&City, &crate::game::HexPos)>,
    units: Query<&Unit, Without<crate::hostile::Hostile>>,
    mut text: Single<&mut Text, With<TopBarText>>,
) {
    let gold_income: i32 =
        cities.iter().map(|(c, p)| rules::city_yields(&map.0, p.0, &c.worked).gold).sum();
    let waiting = units.iter().filter(|u| u.0.needs_orders()).count();
    set_text(
        &mut text,
        format!(
            "Tour {}   |   Or : {} (+{})   |   Villes : {}   |   Unités : {} ({} en attente)",
            state.turn,
            state.gold,
            gold_income,
            cities.iter().count(),
            units.iter().count(),
            waiting
        ),
    );
}

fn update_selection_panel(
    selection: Res<Selection>,
    order: Res<OrderPath>,
    map: Res<GameMap>,
    units: Query<&Unit>,
    cities: Query<(&City, &crate::game::HexPos)>,
    mut text: Single<&mut Text, With<SelectionText>>,
) {
    let content = match selection.0 {
        Some(e) if units.contains(e) => {
            let u = &units.get(e).unwrap().0;
            let st = u.kind.stats();
            let mut s = format!("{}\nPV : {}/{}   ·   Mouvements : {}/{}", u.kind.name(), u.hp, rules::MAX_HP, u.moves_left, st.moves);
            if u.kind.is_military() || u.kind == UnitKind::Scout {
                s += &format!(
                    "\nMêlée : attaque {} · défense {}   ·   Déf. à distance {}",
                    st.melee_attack, st.melee_defense, st.ranged_defense
                );
            }
            if st.ranged_attack > 0 {
                s += &format!("\nTir : {} (portée {})", st.ranged_attack, st.range);
            }
            if let Some(trait_text) = unit_traits(u.kind) {
                s += &format!("\n{trait_text}");
            }
            if u.kind.can_charge() {
                s += if u.moved_this_turn && u.moves_left > 0 {
                    "\nCharge prête : +8 en attaque ce tour-ci"
                } else {
                    "\nCharge : +8 en attaque après un déplacement"
                };
            }
            if u.has_attacked {
                s += "\nA déjà attaqué ce tour-ci";
            }
            if u.zoc_stopped {
                s += "\nArrêté(e) par une zone de contrôle : plus de déplacement ce tour-ci";
            }
            s += "\n\nClic droit : se déplacer / attaquer un ennemi\nEspace : passer   ·   Tab : unité suivante";
            if u.auto_explore {
                s += "\nExploration automatique en cours (E pour arrêter)";
            } else if let Some(&turns) = order.turns.last() {
                s += &format!("\nEn route : {} case(s) restante(s), arrivée en {} tour(s)", order.path.len(), turns);
            }
            if u.kind.can_auto_explore() && !u.auto_explore {
                s += "\nE : exploration automatique";
            }
            if u.kind == UnitKind::Settler {
                s += "\nB : fonder une ville";
            }
            s
        }
        Some(e) if cities.contains(e) => {
            let (city, pos) = cities.get(e).unwrap();
            let c = &city.state;
            let y = rules::city_yields(&map.0, pos.0, &city.worked);
            let surplus = c.food_surplus(y);
            let growth = rules::growth_threshold(c.population);
            let turns_to_grow = if surplus > 0 {
                format!("{} tours", ((growth - c.food_stock) as f32 / surplus as f32).ceil() as i32)
            } else {
                "—".into()
            };
            let production = match c.producing {
                Some(k) => {
                    let turns = if y.production > 0 {
                        format!("{} tours", ((k.cost() - c.production_stock).max(0) as f32 / y.production as f32).ceil() as i32)
                    } else {
                        "∞".into()
                    };
                    format!("{} {}/{} ({})", k.name(), c.production_stock, k.cost(), turns)
                }
                None => "rien".into(),
            };
            format!(
                "{} — population {}\nNourriture : {}/{} ({:+}/tour, croissance : {})\nProduction : +{}/tour   Or : +{}/tour\nEn cours : {}\n\n{}",
                c.name,
                c.population,
                c.food_stock,
                growth,
                surplus,
                turns_to_grow,
                y.production,
                y.gold,
                production,
                "Choisissez la production dans le panneau de gauche."
            )
        }
        _ => "Aucune sélection\n\nClic gauche : sélectionner\nZQSD / flèches : caméra\nMolette : zoom   Clic milieu : glisser\nTab : unité suivante".into(),
    };
    set_text(&mut text, content);
}

/// Particularités d'un type d'unité, en une ligne.
fn unit_traits(kind: UnitKind) -> Option<&'static str> {
    match kind {
        UnitKind::Militia => Some("Zone de contrôle"),
        UnitKind::Spearman => Some("+10 contre la cavalerie · zone de contrôle"),
        UnitKind::Pikeman => Some("+20 contre la cavalerie · zone de contrôle"),
        UnitKind::LightCavalry => Some("+10 contre les archers et la milice"),
        UnitKind::HeavyCavalry => Some("Redoutable, sauf face aux lances"),
        UnitKind::ShieldWarrior => Some("Défenseur solide, résiste aux flèches · zone de contrôle"),
        UnitKind::HeavyWarrior => Some("Attaquant de choc · zone de contrôle"),
        UnitKind::ArcherSwordsman => Some("Tir sans riposte ; portée −1 vers une colline ou une forêt, +20 % en surplomb"),
        UnitKind::ExpertArcher => Some("Tir puissant, mais pas après s'être déplacé ; +20 % en surplomb"),
        UnitKind::HorseArcher => Some("Tir puis repli : garde ses PM après avoir tiré"),
        _ => None,
    }
}

fn fate_text(fate: crate::combat::Fate) -> &'static str {
    match fate {
        crate::combat::Fate::Dies => " — dispersé(e) à coup sûr",
        crate::combat::Fate::MayDie => " — peut être dispersé(e)",
        crate::combat::Fate::Survives => "",
    }
}

fn update_hover_text(
    hovered: Res<HoveredHex>,
    map: Res<GameMap>,
    fog: Res<crate::game::ShownFog>,
    hover_path: Res<HoverPath>,
    villages: Query<&crate::game::HexPos, With<crate::hostile::Village>>,
    camps: Query<&crate::game::HexPos, With<crate::hostile::Camp>>,
    hostiles: Query<(&Unit, &crate::game::HexPos), With<crate::hostile::Hostile>>,
    attack: Res<crate::input::AttackPreview>,
    layer: Res<MapLayer>,
    feature: Res<crate::highlight::HoveredFeature>,
    config: Res<crate::tooltip::TooltipConfig>,
    mut text: Single<&mut Text, With<HoverText>>,
) {
    use crate::tooltip::Info;
    let shows = |info: Info| config.shows(*layer, info);
    let mut content = match hovered.0 {
        Some(h) if fog.explored.contains(&h) => {
            use crate::map::MoveCosts;
            let m = &map.0;
            // Terrain et climat
            let mut place = Vec::new();
            let land_here = m.get(h).is_some_and(|t| !t.is_water());
            let altitude = if land_here {
                shows(Info::Altitude).then(|| format!("{} m", round_meters(m.elevation(h).unwrap_or(0.0))))
            } else {
                // Mer : profondeur du fond (et fosse, au-delà de 6 000 m)
                m.depth(h).map(|d| {
                    let trench = if d >= crate::map::TRENCH_DEPTH_M { ", fosse" } else { "" };
                    format!("{} m de fond{trench}", round_meters(d))
                })
            };
            // Lac ou mer intérieure nommés : leur nom et leur nature plutôt que « Lac » ou « Océan »
            // Étendues marines de la case : « Côte — Baie des Phoques, Mer de Vaskar »
            let seas_here = if m.contains(h) { m.seas().chain(m.cell(h)) } else { Vec::new() };
            let sea_note = if seas_here.is_empty() {
                String::new()
            } else {
                format!(" — {}", seas_here.iter().map(|&z| m.seas().zones[z].name.as_str()).collect::<Vec<_>>().join(", "))
            };
            let terrain = match m.contains(h).then(|| m.basins().lake(m.cell(h))).flatten() {
                Some(body) => match body.etymology {
                    Some(e) => format!("{} ({} ; {})", body.name, body.kind.short(), e.describe()),
                    None => format!("{} ({})", body.name, body.kind.short()),
                },
                None => format!("{}{sea_note}", m.describe(h)),
            };
            match (shows(Info::Terrain), altitude) {
                (true, Some(a)) => place.push(format!("{terrain} ({a})")),
                (true, None) => place.push(terrain),
                (false, Some(a)) => place.push(format!("Altitude : {a}")),
                (false, None) => {}
            }
            let land = m.get(h).is_some_and(|t| !t.is_water());
            if land {
                if shows(Info::Climate)
                    && let Some(zone) = m.climate_zone(h)
                {
                    place.push(format!("climat {zone}"));
                }
                if shows(Info::Temperature)
                    && let Some(t) = m.temperature(h)
                {
                    place.push(format!("{t:.0} °C"));
                }
                if shows(Info::Rain)
                    && let Some(r) = m.rain(h)
                {
                    place.push(format!("{:.0} mm/an", (r / 10.0).round() * 10.0));
                }
            }
            if shows(Info::Latitude) {
                let lat = m.latitude(h);
                place.push(format!("{:.0}°{}", lat.abs(), if lat >= 0.0 { "N" } else { "S" }));
            }
            let mut parts = Vec::new();
            if !place.is_empty() {
                parts.push(place.join(", "));
            }
            // Banc de poissons : son nom (et, sur la carte des ressources, d'où vient sa richesse)
            let bank = m.bank(h).map(|b| &m.banks()[b]);
            if let Some(bank) = bank
                && shows(Info::Fishing)
            {
                use crate::map::marine;
                let mut why = Vec::new();
                if bank.origins & marine::UPWELLING != 0 {
                    why.push("remontée d'eaux froides");
                }
                if bank.origins & marine::SEAMOUNT != 0 {
                    why.push("mont sous-marin");
                }
                if bank.origins & marine::ISLAND != 0 {
                    why.push("eaux d'île volcanique");
                }
                if bank.origins & marine::FRONT != 0 {
                    why.push("rencontre d'un courant chaud et d'un courant froid");
                }
                parts.push(format!(
                    "{} : banc de poissons ({}), {} cases, +{} nourriture",
                    bank.name,
                    why.join(", "),
                    bank.cells.len(),
                    crate::map::RICH_WATERS_FOOD
                ));
            }
            if shows(Info::Yields) {
                let y = m.yields(h).unwrap_or_default();
                let fish = match bank {
                    Some(b) if !shows(Info::Fishing) => format!(" (dont +{} : {})", crate::map::RICH_WATERS_FOOD, b.name),
                    _ => String::new(),
                };
                parts.push(format!("nourriture {}{fish}, production {}, or {}", y.food, y.production, y.gold));
            }
            if shows(Info::Moves) {
                parts.push(m.move_cost(h).map(|c| format!("{c} PM")).unwrap_or_else(|| "infranchissable".into()));
            }
            let mut line = parts.join(" — ");
            if shows(Info::Waterways) {
                line += &water_note(m, h);
            }
            if shows(Info::Lake)
                && let Some(lake) = m.contains(h).then(|| m.basins().lake(m.cell(h))).flatten()
            {
                line += &format!("\n{} : {}, {} cases", lake.name, lake.kind.describe(), lake.cells.len());
            }
            if shows(Info::Discharge) && m.discharge(h) > 0.0 {
                line += &format!("\nDébit qui traverse la case : ≈ {} m³/s", round_flow(m.discharge(h)));
            }
            if shows(Info::Basin) {
                line += &basin_note(m, h);
            }
            if shows(Info::Network) {
                line += &network_note(m, &fog, feature.0);
            }
            if shows(Info::Plate) {
                line += &plate_note(m, h);
            }
            line.trim_start_matches('\n').to_string()
        }
        Some(_) => "Inexploré (terrain inconnu, supposé franchissable : 2 PM)".into(),
        // Survol d'une étiquette : l'élément nommé et son réseau
        None if feature.0.is_some() => network_note(&map.0, &fog, feature.0).trim_start_matches('\n').to_string(),
        None => "—".into(),
    };
    if let Some(h) = hovered.0 {
        if fog.explored.contains(&h) && villages.iter().any(|p| p.0 == h) {
            content += "\nVillage : une visite peut rapporter des habitants, de l'or… ou des ennuis";
        }
        if fog.explored.contains(&h) && camps.iter().any(|p| p.0 == h) {
            content += &format!(
                "\nCamp hostile : entrez-y avec une unité pour le disperser (+{} or)",
                crate::hostile::CAMP_CAPTURE_GOLD
            );
        }
        if fog.visible.contains(&h)
            && let Some((u, _)) = hostiles.iter().find(|(_, p)| p.0 == h)
        {
            content += &format!("\n{} hostile — PV {}/{}", u.0.kind.name(), u.0.hp, rules::MAX_HP);
        }
    }
    if let Some(info) = &attack.0 {
        let p = &info.preview;
        let mods = |m: &Vec<(String, i32)>| {
            m.iter().map(|(label, v)| format!(" {label} {v:+}")).collect::<String>()
        };
        let ranged = p.kind == crate::combat::AttackKind::Ranged;
        content += &format!(
            "\n\n{} : {} {}{}  contre  {} {}{}",
            if ranged { "Tir" } else { "Attaque" },
            info.attacker.name(),
            p.attack.value,
            mods(&p.attack.modifiers),
            info.defender.name(),
            p.defense.value,
            mods(&p.defense.modifiers),
        );
        content += &format!(
            "\nDégâts infligés : {} ({} à {}){}",
            p.to_defender.expected,
            p.to_defender.min,
            p.to_defender.max,
            fate_text(p.defender_fate)
        );
        if ranged {
            content += "\nDégâts subis : aucun (pas de riposte à un tir)";
        } else {
            content += &format!(
                "\nDégâts subis : {} ({} à {}){}",
                p.to_attacker.expected,
                p.to_attacker.min,
                p.to_attacker.max,
                fate_text(p.attacker_fate)
            );
        }
        if !info.reachable {
            content += "\nImpossible ce tour-ci (hors de portée, PM insuffisants, déjà attaqué ou accès bloqué)";
        }
    }
    if attack.0.is_none()
        && let Some(&turns) = hover_path.turns.last()
    {
        content += &format!("\nTrajet : {} case(s), arrivée en {} tour(s)", hover_path.path.len(), turns);
    }
    set_text(&mut text, content);
}

/// Mise en évidence d'un message selon son importance : couleur du liseré, durée et force du
/// flash d'arrivée, nombre de pulsations, teinte gardée ensuite, couleur du texte.
struct LogStyle {
    accent: Color,
    flash_secs: f64,
    pulses: f32,
    peak: f32,
    resting: f32,
    text: Color,
}

fn log_style(importance: Importance) -> LogStyle {
    match importance {
        Importance::Routine => LogStyle { accent: Color::srgb(0.45, 0.47, 0.52), flash_secs: 0.0, pulses: 1.0, peak: 0.0, resting: 0.0, text: Color::srgb(0.72, 0.73, 0.77) },
        Importance::Info => LogStyle { accent: Color::srgb(0.35, 0.62, 1.0), flash_secs: 0.9, pulses: 1.0, peak: 0.22, resting: 0.0, text: Color::srgb(0.92, 0.92, 0.95) },
        Importance::Success => LogStyle { accent: Color::srgb(0.98, 0.78, 0.28), flash_secs: 1.3, pulses: 1.0, peak: 0.32, resting: 0.05, text: Color::srgb(0.98, 0.95, 0.86) },
        Importance::Warning => LogStyle { accent: Color::srgb(1.0, 0.56, 0.16), flash_secs: 1.8, pulses: 2.0, peak: 0.40, resting: 0.08, text: Color::srgb(1.0, 0.93, 0.85) },
        Importance::Critical => LogStyle { accent: Color::srgb(0.96, 0.24, 0.20), flash_secs: 2.6, pulses: 3.0, peak: 0.55, resting: 0.14, text: Color::srgb(1.0, 0.88, 0.85) },
    }
}

/// Lignes du journal : retire celles qui en sont sorties, ajoute les nouvelles en bas.
fn update_log(
    mut commands: Commands,
    time: Res<Time>,
    log: Res<GameLog>,
    list: Single<Entity, With<LogList>>,
    rows: Query<(Entity, &LogRow)>,
) {
    if !log.is_changed() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let entries = log.entries();
    for (entity, row) in &rows {
        if !entries.iter().any(|e| e.id == row.id) {
            commands.entity(entity).despawn();
        }
    }
    let shown: Vec<u64> = rows.iter().map(|(_, r)| r.id).collect();
    for entry in entries.iter().filter(|e| !shown.contains(&e.id)) {
        let style = log_style(entry.importance);
        let size = if entry.importance == Importance::Critical { 15.0 } else { 14.0 };
        commands.entity(*list).with_child((
            LogRow { id: entry.id, born: now, importance: entry.importance },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(7.0),
                padding: UiRect::new(Val::Px(0.0), Val::Px(8.0), Val::Px(3.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Outline::new(Val::Px(1.0), Val::ZERO, Color::NONE),
            UiTransform::default(),
            children![
                (LogAccent, Node { width: Val::Px(3.0), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(style.accent)),
                (LogLine, Node { flex_grow: 1.0, flex_shrink: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), ..default() }, Text::new(entry.text.clone()), TextFont::from_font_size(size), TextColor(style.text)),
            ],
        ));
    }
}

/// Animation des lignes du journal : glissement d'entrée, flash (et pulsations pour les messages
/// importants) qui s'éteint, teinte gardée pour les alertes, messages anciens estompés.
fn animate_log(
    time: Res<Time>,
    log: Res<GameLog>,
    mut rows: Query<(&LogRow, &Children, &mut BackgroundColor, &mut Outline, &mut UiTransform), Without<LogAccent>>,
    mut texts: Query<&mut TextColor, With<LogLine>>,
) {
    let now = time.elapsed_secs_f64();
    let newest = log.entries().last().map_or(0, |e| e.id);
    for (row, children, mut bg, mut outline, mut ui) in &mut rows {
        let style = log_style(row.importance);
        let age = now - row.born;
        // Glissement depuis la droite et léger grossissement à l'arrivée (plus marqué si grave)
        let enter = (age / 0.3).clamp(0.0, 1.0) as f32;
        let ease = 1.0 - (1.0 - enter) * (1.0 - enter);
        let grow = if row.importance >= Importance::Warning { 0.05 } else { 0.0 };
        let translation = Val2::px(28.0 * (1.0 - ease), 0.0);
        let scale = Vec2::splat(1.0 + grow * (1.0 - ease));
        if ui.translation != translation || ui.scale != scale {
            ui.translation = translation;
            ui.scale = scale;
        }
        // Flash qui s'éteint, en pulsations pour les alertes
        let flash = if age < style.flash_secs {
            let u = (age / style.flash_secs) as f32;
            let pulse = if style.pulses > 1.0 { 0.55 + 0.45 * (u * style.pulses * std::f32::consts::TAU).cos() } else { 1.0 };
            style.peak * (1.0 - u) * pulse
        } else {
            0.0
        };
        let tint = style.accent.with_alpha(style.resting.max(flash));
        if bg.0 != tint {
            bg.0 = tint;
        }
        let ring = if row.importance >= Importance::Warning { style.accent.with_alpha(flash * 1.6) } else { Color::NONE };
        if outline.color != ring {
            outline.color = ring;
        }
        // Anciens messages estompés (les alertes restent lisibles)
        let rank = newest.saturating_sub(row.id) as f32;
        let floor = if row.importance >= Importance::Warning { 0.8 } else { 0.55 };
        let alpha = (1.0 - 0.09 * rank).max(floor) * ease.max(0.15);
        for child in children.iter() {
            if let Ok(mut color) = texts.get_mut(child) {
                let wanted = style.text.with_alpha(alpha);
                if color.0 != wanted {
                    color.0 = wanted;
                }
            }
        }
    }
}

/// Cours d'eau d'une case pour le survol : fleuve, rivières sur ses arêtes, eau douce.
fn water_note(map: &crate::map::WorldMap, h: crate::hex::Hex) -> String {
    use crate::map::{FLEUVE_MOVE_COST, FRESH_WATER_FOOD, RIVER_CROSSING_COST};
    const SIDES: [&str; 6] = ["à l'est", "au nord-est", "au nord-ouest", "à l'ouest", "au sud-ouest", "au sud-est"];
    let flow = |q: f32| format!("≈ {} m³/s", round_flow(q));
    let mut note = String::new();
    if map.is_fleuve(h) {
        let name = map
            .named_river(h)
            .map(|r| match r.etymology {
                Some(e) => format!(" {} ({})", r.name, e.describe()),
                None => format!(" {}", r.name),
            })
            .unwrap_or_default();
        note += &format!(
            "\nFleuve{name} ({}) : une unité à pied y entre à gué ou en barque ({FLEUVE_MOVE_COST} PM) ; pas de ville dans son lit",
            flow(map.discharge(h))
        );
    }
    let rivers: Vec<(usize, f32)> = (0..6).map(|d| (d, map.river_flow(h, d))).filter(|(_, q)| *q > 0.0).collect();
    if !rivers.is_empty() {
        let sides: Vec<String> = rivers
            .iter()
            .map(|&(d, q)| match map.contains(h).then(|| map.basins().river_on_edge(map.cell(h), d)).flatten() {
                Some(r) => format!("{} ({}, {})", SIDES[d], r.name, flow(q)),
                None => format!("{} ({})", SIDES[d], flow(q)),
            })
            .collect();
        note += &format!("\nRivière {} : la traverser coûte +{RIVER_CROSSING_COST} PM", sides.join(", "));
    }
    if map.small_lake(h) {
        note += "\nPetit lac";
    }
    let t = map.get(h);
    if !map.is_fleuve(h) && t.is_some_and(|t| !t.is_water()) && map.relief(h) < crate::map::Relief::Mountain && map.fresh_water(h) {
        note += &format!(" — eau douce : +{FRESH_WATER_FOOD} nourriture");
    }
    note
}

/// Élément surligné (cours d'eau ou lac) : où il se jette, ses affluents directs et le nombre
/// d'affluents plus lointains (connus seulement).
fn network_note(map: &crate::map::WorldMap, fog: &crate::game::ShownFog, feature: Option<crate::highlight::Feature>) -> String {
    use crate::highlight::{Feature, known, network};
    let Some(main) = feature else { return String::new() };
    let basins = map.basins();
    let net = network(basins, main);
    let mut note = format!("\nEn rouge : {}", main.name(basins));
    if let Some(r) = net.receiver.filter(|&r| known(map, fog, r)) {
        let verb = if matches!(main, Feature::Lake(_)) { "se déverse dans" } else { "se jette dans" };
        note += &format!(" — {verb} : {} (en bleu)", r.name(basins));
    }
    let direct: Vec<&str> = net.direct.iter().filter(|&&f| known(map, fog, f)).map(|f| f.name(basins)).collect();
    if !direct.is_empty() {
        note += &format!(" — affluents (en orange) : {}", direct.join(", "));
    }
    let after: Vec<&str> = net.downstream.iter().filter(|&&f| known(map, fog, f)).map(|f| f.name(basins)).collect();
    if !after.is_empty() {
        note += &format!(" — puis : {} (en violet)", after.join(" → "));
    }
    let deeper = net.deeper.iter().filter(|&&f| known(map, fog, f)).count();
    if deeper > 0 {
        note += &format!(" — et {deeper} plus en amont (en jaune)");
    }
    note
}

/// Bassin versant d'une case pour le survol.
/// Infobulle : plaque de la case, son mouvement, la frontière qui la touche, un point chaud.
fn plate_note(map: &crate::map::WorldMap, h: crate::hex::Hex) -> String {
    let (Some(plate), Some(v)) = (map.plate(h), map.plate_velocity(h)) else { return String::new() };
    let heading = ["l'est", "le nord-est", "le nord", "le nord-ouest", "l'ouest", "le sud-ouest", "le sud", "le sud-est"]
        [((v.y.atan2(v.x).to_degrees() + 22.5).rem_euclid(360.0) / 45.0) as usize % 8];
    let name = &plate.name;
    let mut note = format!(
        "\n{}{} ({}), {:.0} cm/an vers {heading}",
        name[..1].to_uppercase(),
        &name[1..],
        plate.kind.name(),
        v.length()
    );
    // (une plaque peut porter continent et océan : la croûte de la case)
    if let Some(crust) = map.contains(h).then(|| map.plates().crust.get(map.cell(h))).flatten() {
        note += &format!(" ; croûte {}", crust.name());
    }
    if let Some(b) = map.plate_boundary(h) {
        note += &format!(" ; frontière : {}", b.name());
    }
    if map.plates().hotspots.contains(&h) {
        note += " ; point chaud";
    }
    note
}

fn basin_note(map: &crate::map::WorldMap, h: crate::hex::Hex) -> String {
    let Some(basin) = map.basin(h) else { return String::new() };
    let basins = map.basins();
    let kind = match (basin.lake, basin.river) {
        (Some(l), _) => {
            let lake = &basins.lakes[l];
            format!("Bassin fermé : l'eau finit dans {} ({})", lake.name, lake.kind.short())
        }
        (None, Some(r)) => format!("Bassin versant du fleuve {}", basins.rivers[r].name),
        _ if basin.closed => "Bassin fermé (l'eau finit dans un lac sans issue)".to_string(),
        _ => "Petit bassin côtier".to_string(),
    };
    let mut note = format!("\n{kind} : {} cases, ≈ {} m³/s à sa sortie", basin.cells, round_flow(basin.outflow));
    if let Some(other) = map.contains(h).then(|| basins.basin_also[map.cell(h)]).flatten() {
        let other = &basins.basins[other as usize];
        let name = basins.basin_name(other).map_or_else(|| "un petit bassin voisin".to_string(), |n| format!("le bassin : {n}"));
        note += &format!(" ; dessert aussi {name} (hachures)");
    }
    note
}

/// Altitude arrondie lisiblement : 150, 1 850 (à 10 m près, espace des milliers).
fn round_meters(m: f32) -> String {
    let v = ((m / 10.0).round() * 10.0) as i64;
    if v >= 1_000 { format!("{} {:03}", v / 1_000, v % 1_000) } else { v.to_string() }
}

/// Débit arrondi lisiblement : 140, 1 200, 31 000.
fn round_flow(q: f32) -> String {
    let step = if q < 1_000.0 { 10.0 } else if q < 10_000.0 { 100.0 } else { 1_000.0 };
    let v = ((q / step).round() * step) as i64;
    let s = v.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}
