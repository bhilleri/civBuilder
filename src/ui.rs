//! Interface : barre du haut, panneau de sélection, infos de case, journal, bouton de fin de tour.

use crate::game::{City, EndTurnRequested, GameLog, GameMap, GameState, Selection, Unit};
use crate::input::{HoverPath, HoveredHex, OrderPath};
use crate::rules::{self, UnitKind};
use crate::render::{CLIMATE_COAST, CLIMATE_OCEAN, MapLayer, TOPO_COAST, TOPO_LEVELS, TOPO_OCEAN, TOPO_SUMMIT, rain_color};
use bevy::prelude::*;

#[derive(Component)]
struct TopBarText;
#[derive(Component)]
struct SelectionText;
#[derive(Component)]
struct HoverText;
#[derive(Component)]
struct LogText;
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
/// Légende de la carte topographique.
#[derive(Component)]
struct TopoLegend;

/// Légende de la carte du climat.
#[derive(Component)]
struct ClimateLegend;
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

    // Journal (en haut à droite)
    commands.spawn((
        panel(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            max_width: Val::Px(420.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        }),
        children![(LogText, text("", 14.0))],
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
            climate_legend(),
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
                    map_option(MapLayer::Relief, "Carte du relief"),
                    map_option(MapLayer::Climate, "Carte du climat"),
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
        ClimateLegend,
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
            Spawn(legend_row(CLIMATE_COAST, "Eau côtière".into())),
            Spawn(legend_row(CLIMATE_OCEAN, "Eau profonde".into())),
            Spawn(text("Flèches : vents dominants", 13.0)),
        )),
    )
}

/// Légende de la carte du relief : une pastille de couleur par niveau.
fn topo_legend() -> impl Bundle {
    let row = |(r, g, b): (f32, f32, f32), label: String| {
        (
            Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() },
            children![
                (
                    Node {
                        width: Val::Px(14.0),
                        height: Val::Px(14.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(r, g, b)),
                ),
                (Text::new(label), TextFont::from_font_size(13.0), TextColor(Color::srgb(0.92, 0.92, 0.95))),
            ],
        )
    };
    let levels: Vec<String> = (1..=crate::map::MAX_LEVEL).map(|l| format!("{l} — {}", crate::map::level_name(l))).collect();
    (
        TopoLegend,
        panel(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        }),
        Children::spawn((
            Spawn(text("Relief", 14.0)),
            Spawn(row(TOPO_SUMMIT, "Sommets".into())),
            Spawn(row(TOPO_LEVELS[5], levels[5].clone())),
            Spawn(row(TOPO_LEVELS[4], levels[4].clone())),
            Spawn(row(TOPO_LEVELS[3], levels[3].clone())),
            Spawn(row(TOPO_LEVELS[2], levels[2].clone())),
            Spawn(row(TOPO_LEVELS[1], levels[1].clone())),
            Spawn(row(TOPO_LEVELS[0], levels[0].clone())),
            Spawn(row(TOPO_COAST, "Eau côtière".into())),
            Spawn(row(TOPO_OCEAN, "Eau profonde".into())),
        )),
    )
}

/// Choix de la carte : le bouton ouvre ou ferme le menu, une option change la carte (T : bascule
/// directe) ; la légende du relief n'apparaît qu'avec la carte du relief.
fn map_layer_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut layer: ResMut<MapLayer>,
    mut button: Query<(Ref<Interaction>, &mut BackgroundColor, &Children), (With<MapLayerButton>, Without<MapLayerOption>)>,
    mut options: Query<(Ref<Interaction>, &MapLayerOption, &mut BackgroundColor), Without<MapLayerButton>>,
    mut menu: Single<&mut Node, (With<MapLayerMenu>, Without<TopoLegend>, Without<ClimateLegend>)>,
    mut legend: Single<&mut Node, (With<TopoLegend>, Without<MapLayerMenu>, Without<ClimateLegend>)>,
    mut climate_legend: Single<&mut Node, (With<ClimateLegend>, Without<MapLayerMenu>, Without<TopoLegend>)>,
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
        let label = match *layer {
            MapLayer::Normal => "Carte : normale (T)",
            MapLayer::Relief => "Carte : relief (T)",
            MapLayer::Climate => "Carte : climat (T)",
        };
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok())
            && t.0 != label
        {
            t.0 = label.to_string();
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
    let legend_display = if *layer == MapLayer::Relief { Display::Flex } else { Display::None };
    if legend.display != legend_display {
        legend.display = legend_display;
    }
    let climate_display = if *layer == MapLayer::Climate { Display::Flex } else { Display::None };
    if climate_legend.display != climate_display {
        climate_legend.display = climate_display;
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
    mut text: Single<&mut Text, With<HoverText>>,
) {
    let mut content = match hovered.0 {
        Some(h) if fog.explored.contains(&h) => {
            use crate::map::MoveCosts;
            let y = map.0.yields(h).unwrap_or_default();
            let moves = map.0.move_cost(h).map(|c| format!("{c} PM")).unwrap_or_else(|| "infranchissable".into());
            let altitude = match map.0.level(h).unwrap_or(0) {
                0 => String::new(),
                l => format!(" (niveau {l}, {})", crate::map::level_name(l)),
            };
            let name = map.0.describe(h);
            let climate = map.0.climate_name(h).map(|c| format!(", climat {c}")).unwrap_or_default();
            let mut line =
                format!("{name}{altitude}{climate} — nourriture {}, production {}, or {} — {}", y.food, y.production, y.gold, moves);
            line += &water_note(&map.0, h);
            line
        }
        Some(_) => "Inexploré (terrain inconnu, supposé franchissable : 2 PM)".into(),
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

fn update_log(log: Res<GameLog>, mut text: Single<&mut Text, With<LogText>>) {
    if log.is_changed() {
        set_text(&mut text, log.0.join("\n"));
    }
}

/// Cours d'eau d'une case pour le survol : fleuve, rivières sur ses arêtes, eau douce.
fn water_note(map: &crate::map::WorldMap, h: crate::hex::Hex) -> String {
    use crate::map::{FLEUVE_MOVE_COST, FRESH_WATER_FOOD, RIVER_CROSSING_COST, Terrain};
    const SIDES: [&str; 6] = ["à l'est", "au nord-est", "au nord-ouest", "à l'ouest", "au sud-ouest", "au sud-est"];
    let flow = |q: f32| format!("≈ {} m³/s", round_flow(q));
    let mut note = String::new();
    if map.is_fleuve(h) {
        note += &format!(
            "\nFleuve ({}) : une unité à pied y entre à gué ou en barque ({FLEUVE_MOVE_COST} PM) ; pas de ville dans son lit",
            flow(map.discharge(h))
        );
    }
    let rivers: Vec<(usize, f32)> = (0..6).map(|d| (d, map.river_flow(h, d))).filter(|(_, q)| *q > 0.0).collect();
    if !rivers.is_empty() {
        let sides: Vec<String> = rivers.iter().map(|&(d, q)| format!("{} ({})", SIDES[d], flow(q))).collect();
        note += &format!("\nRivière {} : la traverser coûte +{RIVER_CROSSING_COST} PM", sides.join(", "));
    }
    if map.small_lake(h) {
        note += "\nPetit lac";
    }
    let t = map.get(h);
    if !map.is_fleuve(h) && t.is_some_and(|t| !t.is_water() && t != Terrain::Mountains) && map.fresh_water(h) {
        note += &format!(" — eau douce : +{FRESH_WATER_FOOD} nourriture");
    }
    note
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
