//! Menu du jeu (touche Échap) : reprendre, mode debug, graine du monde (regénérer le même monde
//! ou un nouveau), quitter.
//!
//! Le mode debug révèle toute la carte à l'affichage (brouillard de guerre levé). Il ne change
//! pas ce que le joueur sait : en le désactivant, on revoit uniquement ce qui a été exploré.

use crate::game::{DebugMode, GameLog};
use crate::hostile::WorldSeed;
use crate::wiki::WikiState;
use bevy::input::keyboard::Key;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, EditableTextFilter, TextCursorStyle, TextEdit};

/// Menu ouvert ou fermé.
#[derive(Resource, Default)]
pub struct GameMenu {
    pub open: bool,
}

#[derive(Component)]
struct MenuBackdrop;
#[derive(Component)]
enum MenuButton {
    Resume,
    Debug,
    /// Regénérer le monde avec la graine saisie.
    Regenerate,
    /// Nouveau monde, graine au hasard.
    NewWorld,
    /// Copier la graine saisie dans le presse-papiers.
    CopySeed,
    /// Coller le presse-papiers dans le champ (remplace la graine).
    PasteSeed,
    Quit,
}

/// Champ de saisie de la graine (chiffres seulement ; Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+A).
#[derive(Component)]
struct SeedField;
/// Bandeau rappelant que le mode debug est actif.
#[derive(Component)]
struct DebugBanner;
/// Compteur d'images par seconde (mode debug).
#[derive(Component)]
struct FpsText;

const PANEL_BG: Color = Color::srgb(0.05, 0.06, 0.09);
const BUTTON_BG: Color = Color::srgb(0.15, 0.17, 0.22);
const BUTTON_HOVER: Color = Color::srgb(0.24, 0.27, 0.34);
const DEBUG_ON: Color = Color::srgb(0.70, 0.35, 0.10);
const QUIT_BG: Color = Color::srgb(0.45, 0.14, 0.14);
const QUIT_HOVER: Color = Color::srgb(0.60, 0.18, 0.18);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameMenu>()
            .add_systems(Startup, setup_menu)
            // Avant le wiki : Échap ferme d'abord le wiki s'il est ouvert, sans ouvrir le menu
            .add_systems(Update, (toggle_menu.before(crate::wiki::WikiSet), seed_field, menu_buttons, show_menu, update_fps).chain());
    }
}

fn label(s: &str, size: f32) -> impl Bundle + use<> {
    (Text::new(s.to_string()), TextFont::from_font_size(size), TextColor(Color::srgb(0.95, 0.95, 0.97)))
}

fn button(kind: MenuButton, text: &str, bg: Color) -> impl Bundle + use<> {
    let is_debug = matches!(kind, MenuButton::Debug);
    (
        kind,
        Button,
        Node {
            width: Val::Px(280.0),
            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(bg),
        Children::spawn(Spawn((label(text, 17.0), DebugLabelIf(is_debug)))),
    )
}

/// Petit bouton à côté du champ de la graine.
fn small_button(kind: MenuButton, text: &str) -> impl Bundle + use<> {
    (
        kind,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(7.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(BUTTON_BG),
        children![label(text, 14.0)],
    )
}

/// Marqueur posé seulement sur le texte du bouton debug.
#[derive(Component)]
struct DebugLabelIf(bool);

fn setup_menu(mut commands: Commands) {
    commands.spawn((
        MenuBackdrop,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            display: Display::None,
            ..default()
        },
        // Fond assombri qui capte les clics : la carte et les boutons dessous sont inactifs
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Interaction::default(),
        GlobalZIndex(200),
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(24.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            children![
                (label("Menu", 24.0), Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() }),
                button(MenuButton::Resume, "Reprendre (Échap)", BUTTON_BG),
                button(MenuButton::Debug, "Mode debug : désactivé", BUTTON_BG),
                (
                    Text::new("Révèle toute la carte (brouillard de guerre levé)."),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.65, 0.67, 0.72)),
                ),
                (Node { height: Val::Px(8.0), ..default() }),
                (label("Graine du monde", 16.0)),
                (
                    Node { width: Val::Px(280.0), column_gap: Val::Px(6.0), ..default() },
                    children![
                        (
                            Node {
                                flex_grow: 1.0,
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.02, 0.02, 0.04)),
                            BorderColor::all(Color::srgb(0.35, 0.38, 0.46)),
                            children![(
                                SeedField,
                                EditableText { max_characters: Some(10), ..default() },
                                EditableTextFilter::new(|c| c.is_ascii_digit()),
                                TextFont::from_font_size(18.0),
                                TextColor(Color::srgb(0.95, 0.95, 0.97)),
                                TextLayout::no_wrap(),
                                TextCursorStyle { color: Color::WHITE, selection_color: Color::srgb(0.20, 0.40, 0.75), ..default() },
                                Node { width: Val::Percent(100.0), ..default() },
                            )],
                        ),
                        small_button(MenuButton::CopySeed, "Copier"),
                        small_button(MenuButton::PasteSeed, "Coller"),
                    ],
                ),
                (
                    Text::new("Tapez ou collez une graine (chiffres), puis Entrée ou le bouton : même graine, même monde. Ctrl+C / Ctrl+V : copier, coller."),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.65, 0.67, 0.72)),
                    Node { max_width: Val::Px(300.0), ..default() },
                ),
                button(MenuButton::Regenerate, "Regénérer avec cette graine", BUTTON_BG),
                button(MenuButton::NewWorld, "Nouveau monde (graine au hasard)", BUTTON_BG),
                (
                    Text::new("La partie en cours sera perdue."),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(0.65, 0.67, 0.72)),
                ),
                (Node { height: Val::Px(8.0), ..default() }),
                button(MenuButton::Quit, "Quitter le jeu", QUIT_BG),
            ],
        )],
    ));
    commands.spawn((
        DebugBanner,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(52.0),
            left: Val::Percent(50.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            display: Display::None,
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
        BackgroundColor(DEBUG_ON),
        GlobalZIndex(50),
        children![
            label("MODE DEBUG — carte révélée", 14.0),
            (FpsText, label("", 14.0), Node { margin: UiRect::left(Val::Px(12.0)), ..default() }),
        ],
    ));
}

/// Échap ouvre ou ferme le menu (sauf si le wiki est ouvert : Échap le ferme d'abord).
fn toggle_menu(keys: Res<ButtonInput<KeyCode>>, wiki: Res<WikiState>, mut menu: ResMut<GameMenu>) {
    if keys.just_pressed(KeyCode::Escape) && !wiki.open {
        menu.open = !menu.open;
    }
}

/// Champ de la graine : à l'ouverture du menu, il montre la graine du monde actuel, toute
/// sélectionnée (on peut la copier aussitôt, ou taper par-dessus) ; Entrée regénère.
fn seed_field(
    menu: Res<GameMenu>,
    seed: Res<WorldSeed>,
    keys: Res<ButtonInput<Key>>,
    mut focus: ResMut<InputFocus>,
    mut was_open: Local<bool>,
    mut field: Single<(Entity, &mut EditableText), With<SeedField>>,
    mut log: ResMut<GameLog>,
    mut exit: MessageWriter<AppExit>,
) {
    let (entity, ref mut editable) = *field;
    if menu.open && !*was_open {
        editable.editor_mut().set_text(&seed.0.to_string());
        editable.queue_edit(TextEdit::SelectAll);
        focus.set(entity, FocusCause::Navigated);
    }
    if !menu.open && *was_open && focus.get() == Some(entity) {
        focus.clear();
    }
    *was_open = menu.open;
    if menu.open && focus.get() == Some(entity) && keys.just_pressed(Key::Enter) {
        regenerate_with(&editable.value().to_string(), &mut log, &mut exit);
    }
}

/// Regénère avec la graine écrite (vérifiée).
fn regenerate_with(text: &str, log: &mut GameLog, exit: &mut MessageWriter<AppExit>) {
    match text.parse::<u32>() {
        Ok(seed) => regenerate(Some(seed), log, exit),
        Err(_) => log.push("Graine invalide : entre 0 et 4 294 967 295."),
    }
}

/// Relance le jeu sur un nouveau monde : avec la graine donnée, ou au hasard. Le jeu repart de
/// zéro (nouveau processus, mêmes réglages), la fenêtre actuelle se ferme.
fn regenerate(seed: Option<u32>, log: &mut GameLog, exit: &mut MessageWriter<AppExit>) {
    let launched = std::env::current_exe().and_then(|exe| {
        let mut command = std::process::Command::new(exe);
        command.args(std::env::args_os().skip(1));
        match seed {
            Some(s) => command.env("CIVBUILDER_SEED", s.to_string()),
            None => command.env_remove("CIVBUILDER_SEED"),
        };
        command.spawn()
    });
    match launched {
        Ok(_) => {
            exit.write(AppExit::Success);
        }
        Err(e) => log.push(format!("Impossible de relancer le jeu : {e}")),
    }
}

fn menu_buttons(
    mut menu: ResMut<GameMenu>,
    mut debug: ResMut<DebugMode>,
    mut field: Single<(Entity, &mut EditableText), With<SeedField>>,
    mut focus: ResMut<InputFocus>,
    mut clipboard: ResMut<bevy::clipboard::Clipboard>,
    mut log: ResMut<GameLog>,
    mut buttons: Query<(Ref<Interaction>, &MenuButton, &mut BackgroundColor)>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, kind, mut bg) in &mut buttons {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            match kind {
                MenuButton::Resume => menu.open = false,
                MenuButton::Debug => debug.0 = !debug.0,
                MenuButton::Regenerate => regenerate_with(&field.1.value().to_string(), &mut log, &mut exit),
                MenuButton::CopySeed => {
                    let value = field.1.value().to_string();
                    match clipboard.set_text(value.clone()) {
                        Ok(()) => log.push(format!("Graine {value} copiée dans le presse-papiers.")),
                        Err(e) => log.push(format!("Impossible de copier la graine : {e:?}")),
                    }
                    focus.set(field.0, FocusCause::Navigated);
                }
                MenuButton::PasteSeed => {
                    // Remplace tout le champ (les caractères autres que des chiffres sont ignorés)
                    field.1.queue_edit(TextEdit::SelectAll);
                    field.1.queue_edit(TextEdit::Paste);
                    focus.set(field.0, FocusCause::Navigated);
                }
                MenuButton::NewWorld => regenerate(None, &mut log, &mut exit),
                MenuButton::Quit => {
                    exit.write(AppExit::Success);
                }
            }
        }
        let hovered = *interaction != Interaction::None;
        let color = match kind {
            MenuButton::Quit if hovered => QUIT_HOVER,
            MenuButton::Quit => QUIT_BG,
            MenuButton::Debug if debug.0 => DEBUG_ON,
            _ if hovered => BUTTON_HOVER,
            _ => BUTTON_BG,
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

fn show_menu(
    menu: Res<GameMenu>,
    debug: Res<DebugMode>,
    mut backdrop: Single<&mut Node, (With<MenuBackdrop>, Without<DebugBanner>)>,
    mut banner: Single<&mut Node, (With<DebugBanner>, Without<MenuBackdrop>)>,
    mut labels: Query<(&mut Text, &DebugLabelIf)>,
) {
    let display = |on: bool| if on { Display::Flex } else { Display::None };
    if backdrop.display != display(menu.open) {
        backdrop.display = display(menu.open);
    }
    if banner.display != display(debug.0) {
        banner.display = display(debug.0);
    }
    if debug.is_changed() {
        let text = if debug.0 { "Mode debug : activé" } else { "Mode debug : désactivé" };
        for (mut t, is_debug) in &mut labels {
            if is_debug.0 {
                t.0 = text.to_string();
            }
        }
    }
}

/// Images par seconde, lissées sur environ une demi-seconde, et pire image de la dernière seconde
/// (utile pour repérer les à-coups).
fn update_fps(
    time: Res<Time<Real>>,
    debug: Res<DebugMode>,
    mut text: Single<&mut Text, With<FpsText>>,
    mut state: Local<(f32, f32, f32, f32)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let (avg, worst, shown_worst, since) = &mut *state;
    *avg = if *avg == 0.0 { dt } else { *avg + (dt - *avg) * (dt / 0.5).min(1.0) };
    *worst = worst.max(dt);
    *since += dt;
    if *since >= 1.0 {
        *shown_worst = *worst;
        *worst = 0.0;
        *since = 0.0;
    }
    if !debug.0 {
        return;
    }
    let value = format!("{:.0} FPS · {:.1} ms · pire {:.0} ms", 1.0 / *avg, *avg * 1000.0, shown_worst.max(*worst) * 1000.0);
    if text.0 != value {
        text.0 = value;
    }
}
