//! Options de la carte (bouton « Options de carte » ou touche O) : étiquettes affichées,
//! comportement de la surbrillance des cours d'eau et contenu de l'infobulle pour chaque carte.

use crate::highlight::HighlightMode;
use crate::labels::LabelFilter;
use crate::render::MapLayer;
use crate::tooltip::{Info, TooltipConfig};
use bevy::prelude::*;

/// Bouton qui ouvre le panneau (placé avec les autres boutons, en bas à droite).
#[derive(Component)]
pub struct OptionsButton;
#[derive(Component)]
struct OptionsPanel;
#[derive(Component)]
struct CloseButton;
/// Bascule d'une famille d'étiquettes.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum LabelToggle {
    Rivers,
    Lakes,
    Cities,
}
/// Choix du comportement de la surbrillance.
#[derive(Component, Clone, Copy)]
struct ModeChoice(HighlightMode);
/// Case du tableau de l'infobulle : rubrique affichée ou non sur une carte.
#[derive(Component, Clone, Copy)]
struct TooltipCell(MapLayer, Info);
/// Pastille d'une case du tableau.
#[derive(Component)]
struct CellDot;

const PANEL_BG: Color = Color::srgb(0.05, 0.06, 0.09);
const IDLE: Color = Color::srgba(0.16, 0.18, 0.24, 0.95);
const HOVER: Color = Color::srgb(0.24, 0.28, 0.38);
const ACTIVE: Color = Color::srgb(0.18, 0.40, 0.72);
const DOT_ON: Color = Color::srgb(0.30, 0.62, 1.0);
const DOT_OFF: Color = Color::srgb(0.22, 0.24, 0.30);
const TEXT: Color = Color::srgb(0.92, 0.92, 0.95);
const MUTED: Color = Color::srgb(0.66, 0.68, 0.74);
const LABEL_WIDTH: f32 = 230.0;
const COLUMN_WIDTH: f32 = 64.0;

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TooltipConfig>()
            .add_systems(Startup, setup_panel)
            .add_systems(Update, (open_close, label_toggles, mode_choices, tooltip_cells));
    }
}

fn text(content: &str, size: f32, color: Color) -> impl Bundle {
    (Text::new(content), TextFont::from_font_size(size), TextColor(color))
}

fn button(width: Option<f32>) -> impl Bundle {
    (
        Button,
        Node {
            width: width.map_or(Val::Auto, Val::Px),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(IDLE),
    )
}

fn section(title: &str) -> impl Bundle {
    (Node { margin: UiRect::top(Val::Px(8.0)), ..default() }, children![text(title, 15.0, TEXT)])
}

fn row() -> Node {
    Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }
}

fn setup_panel(mut commands: Commands) {
    let panel = commands
        .spawn((
            OptionsPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(52.0),
                right: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(12.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            // Le panneau bloque le survol de la carte
            Interaction::default(),
            GlobalZIndex(10),
        ))
        .id();
    commands.entity(panel).with_children(|p| {
        p.spawn((
            Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(20.0), ..default() },
            children![text("Options de carte", 17.0, TEXT), (CloseButton, button(None), children![text("Fermer (O)", 14.0, TEXT)])],
        ));

        p.spawn(section("Étiquettes (N : tout masquer / tout afficher)"));
        p.spawn(row()).with_children(|r| {
            for (toggle, label) in [(LabelToggle::Rivers, "Cours d'eau"), (LabelToggle::Lakes, "Lacs et mers"), (LabelToggle::Cities, "Villes")] {
                r.spawn((toggle, button(None), children![text(label, 14.0, TEXT)]));
            }
        });

        p.spawn(section("Surbrillance des cours d'eau et des lacs"));
        p.spawn(row()).with_children(|r| {
            for (mode, label) in [
                (HighlightMode::OnFeature, "Au survol du cours d'eau"),
                (HighlightMode::OnLabel, "Au survol de l'étiquette"),
                (HighlightMode::Off, "Aucune"),
            ] {
                r.spawn((ModeChoice(mode), button(None), children![text(label, 14.0, TEXT)]));
            }
        });

        p.spawn(section("Infobulle : rubriques affichées sur chaque carte"));
        p.spawn(row()).with_children(|r| {
            r.spawn(Node { width: Val::Px(LABEL_WIDTH), ..default() });
            for layer in MapLayer::ALL {
                let name = match layer {
                    MapLayer::Normal => "Normale",
                    MapLayer::Altitude => "Altitude",
                    MapLayer::Relief => "Relief",
                    MapLayer::Climate => "Climat",
                    MapLayer::Basins => "Bassins",
                    MapLayer::Flow => "Débits",
                    MapLayer::Network => "Réseau",
                };
                r.spawn((Node { width: Val::Px(COLUMN_WIDTH), justify_content: JustifyContent::Center, ..default() }, children![text(name, 13.0, MUTED)]));
            }
        });
        for info in Info::ALL {
            p.spawn(row()).with_children(|r| {
                r.spawn((Node { width: Val::Px(LABEL_WIDTH), ..default() }, children![text(info.label(), 13.0, TEXT)]));
                for layer in MapLayer::ALL {
                    r.spawn((
                        TooltipCell(layer, info),
                        Button,
                        Node {
                            width: Val::Px(COLUMN_WIDTH),
                            height: Val::Px(22.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        children![(
                            CellDot,
                            Node { width: Val::Px(14.0), height: Val::Px(14.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
                            BackgroundColor(DOT_OFF),
                        )],
                    ));
                }
            });
        }
        p.spawn(text("Cliquer sur une case pour afficher ou masquer la rubrique sur cette carte.", 12.0, MUTED));
    });
}

/// Ouverture et fermeture : bouton, touche O, bouton « Fermer ».
fn open_close(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: Single<&mut Node, With<OptionsPanel>>,
    mut open_button: Query<(Ref<Interaction>, &mut BackgroundColor), (With<OptionsButton>, Without<CloseButton>)>,
    mut close_button: Query<(Ref<Interaction>, &mut BackgroundColor), (With<CloseButton>, Without<OptionsButton>)>,
) {
    let mut open = panel.display == Display::Flex;
    if keys.just_pressed(KeyCode::KeyO) {
        open = !open;
    }
    for (interaction, mut bg) in &mut open_button {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            open = !open;
        }
        let c = if *interaction == Interaction::None { IDLE } else { HOVER };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (interaction, mut bg) in &mut close_button {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            open = false;
        }
        let c = if *interaction == Interaction::None { IDLE } else { HOVER };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    let display = if open { Display::Flex } else { Display::None };
    if panel.display != display {
        panel.display = display;
    }
}

fn label_toggles(
    keys: Res<ButtonInput<KeyCode>>,
    mut filter: ResMut<LabelFilter>,
    mut buttons: Query<(Ref<Interaction>, &LabelToggle, &mut BackgroundColor)>,
) {
    if keys.just_pressed(KeyCode::KeyN) {
        let any = filter.rivers || filter.lakes || filter.cities;
        *filter = LabelFilter { rivers: !any, lakes: !any, cities: !any };
    }
    for (interaction, toggle, mut bg) in &mut buttons {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            match toggle {
                LabelToggle::Rivers => filter.rivers = !filter.rivers,
                LabelToggle::Lakes => filter.lakes = !filter.lakes,
                LabelToggle::Cities => filter.cities = !filter.cities,
            }
        }
        let on = match toggle {
            LabelToggle::Rivers => filter.rivers,
            LabelToggle::Lakes => filter.lakes,
            LabelToggle::Cities => filter.cities,
        };
        let c = if *interaction == Interaction::Hovered { HOVER } else if on { ACTIVE } else { IDLE };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

fn mode_choices(mut mode: ResMut<HighlightMode>, mut buttons: Query<(Ref<Interaction>, &ModeChoice, &mut BackgroundColor)>) {
    for (interaction, choice, mut bg) in &mut buttons {
        if interaction.is_changed() && *interaction == Interaction::Pressed && *mode != choice.0 {
            *mode = choice.0;
        }
        let c = if *interaction == Interaction::Hovered { HOVER } else if *mode == choice.0 { ACTIVE } else { IDLE };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

fn tooltip_cells(
    mut config: ResMut<TooltipConfig>,
    layer: Res<MapLayer>,
    mut cells: Query<(Ref<Interaction>, &TooltipCell, &mut BackgroundColor, &Children)>,
    mut dots: Query<&mut BackgroundColor, (With<CellDot>, Without<TooltipCell>)>,
) {
    for (interaction, cell, mut bg, children) in &mut cells {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            config.toggle(cell.0, cell.1);
        }
        // Colonne de la carte affichée légèrement éclairée
        let c = if *interaction == Interaction::Hovered {
            HOVER
        } else if *layer == cell.0 {
            Color::srgb(0.10, 0.12, 0.17)
        } else {
            Color::NONE
        };
        if bg.0 != c {
            bg.0 = c;
        }
        let dot = if config.shows(cell.0, cell.1) { DOT_ON } else { DOT_OFF };
        for &child in children {
            if let Ok(mut d) = dots.get_mut(child)
                && d.0 != dot
            {
                d.0 = dot;
            }
        }
    }
}
