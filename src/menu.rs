//! Menu du jeu (touche Échap), en pages courtes pour qu'il reste lisible à mesure que les
//! options s'ajoutent :
//! - page principale : reprendre, paramètres, nouveau monde, quitter ;
//! - **Paramètres** : vitesse des animations, mode debug (enregistrés aussitôt, voir `settings.rs`) ;
//! - **Nouveau monde** : graine du monde (regénérer le même monde ou un nouveau).
//!
//! Échap sur une sous-page revient à la page principale ; sur la page principale, il ferme le menu.
//!
//! Le mode debug révèle toute la carte à l'affichage (brouillard de guerre levé). Il ne change
//! pas ce que le joueur sait : en le désactivant, on revoit uniquement ce qui a été exploré.

use crate::game::{DebugMode, GameLog};
use crate::hostile::WorldSeed;
use crate::settings::{AnimationSpeed, Settings};
use crate::wiki::WikiState;
use bevy::input::keyboard::Key;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, EditableTextFilter, TextCursorStyle, TextEdit};

/// Page du menu affichée.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuPage {
    #[default]
    Main,
    Settings,
    NewWorld,
}

/// Menu ouvert ou fermé, et page affichée.
#[derive(Resource, Default)]
pub struct GameMenu {
    pub open: bool,
    pub page: MenuPage,
}

#[derive(Component)]
struct MenuBackdrop;
/// Contenu d'une page du menu.
#[derive(Component)]
struct PageNode(MenuPage);
#[derive(Component)]
enum MenuButton {
    Resume,
    /// Ouvre une sous-page.
    Open(MenuPage),
    /// Revient à la page principale.
    Back,
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
/// Case du sélecteur de vitesse des animations.
#[derive(Component)]
struct SpeedChoice(AnimationSpeed);
/// Ligne d'aide sous le sélecteur de vitesse.
#[derive(Component)]
struct SpeedHint;

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
const CHOICE_ON: Color = Color::srgb(0.22, 0.42, 0.78);
const DEBUG_ON: Color = Color::srgb(0.70, 0.35, 0.10);
const QUIT_BG: Color = Color::srgb(0.45, 0.14, 0.14);
const QUIT_HOVER: Color = Color::srgb(0.60, 0.18, 0.18);
const HINT: Color = Color::srgb(0.65, 0.67, 0.72);
const WARNING: Color = Color::srgb(1.0, 0.70, 0.40);
/// Largeur des boutons et du contenu des pages.
const WIDTH: f32 = 300.0;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameMenu>()
            .insert_resource(crate::settings::load())
            .add_systems(Startup, setup_menu)
            // Avant le wiki : Échap ferme d'abord le wiki s'il est ouvert, sans ouvrir le menu
            .add_systems(Update, (toggle_menu.before(crate::wiki::WikiSet), seed_field, menu_buttons, speed_choices, save_settings, show_menu, update_fps).chain());
    }
}

fn label(s: &str, size: f32) -> impl Bundle + use<> {
    (Text::new(s.to_string()), TextFont::from_font_size(size), TextColor(Color::srgb(0.95, 0.95, 0.97)))
}

/// Texte d'aide (petit, gris).
fn hint(s: &str) -> impl Bundle + use<> {
    (Text::new(s.to_string()), TextFont::from_font_size(13.0), TextColor(HINT), Node { max_width: Val::Px(WIDTH), ..default() })
}

/// Titre d'une page.
fn title(s: &str) -> impl Bundle + use<> {
    (label(s, 24.0), Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() })
}

/// Titre d'une rubrique dans une page (aligné à gauche, avec un filet).
fn section(s: &str) -> impl Bundle + use<> {
    (
        Node {
            width: Val::Px(WIDTH),
            margin: UiRect::top(Val::Px(6.0)),
            padding: UiRect::bottom(Val::Px(3.0)),
            border: UiRect::bottom(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.25, 0.27, 0.33)),
        children![(Text::new(s.to_uppercase()), TextFont::from_font_size(12.0), TextColor(Color::srgb(0.60, 0.70, 0.88)))],
    )
}

fn spacer() -> impl Bundle + use<> {
    Node { height: Val::Px(8.0), ..default() }
}

fn button(kind: MenuButton, text: &str, bg: Color) -> impl Bundle + use<> {
    let is_debug = matches!(kind, MenuButton::Debug);
    (
        kind,
        Button,
        Node {
            width: Val::Px(WIDTH),
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

/// Case du sélecteur de vitesse.
fn speed_choice(speed: AnimationSpeed) -> impl Bundle + use<> {
    (
        SpeedChoice(speed),
        Button,
        Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(2.0), Val::Px(7.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(BUTTON_BG),
        children![label(speed.label(), 13.0)],
    )
}

/// Page du menu (cachée tant qu'elle n'est pas choisie).
fn page(kind: MenuPage) -> impl Bundle + use<> {
    (
        PageNode(kind),
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            display: if kind == MenuPage::Main { Display::Flex } else { Display::None },
            ..default()
        },
    )
}

/// Marqueur posé seulement sur le texte du bouton debug.
#[derive(Component)]
struct DebugLabelIf(bool);

fn setup_menu(mut commands: Commands) {
    let main = (
        page(MenuPage::Main),
        children![
            title("Menu"),
            button(MenuButton::Resume, "Reprendre (Échap)", BUTTON_BG),
            button(MenuButton::Open(MenuPage::Settings), "Paramètres", BUTTON_BG),
            button(MenuButton::Open(MenuPage::NewWorld), "Nouveau monde…", BUTTON_BG),
            spacer(),
            button(MenuButton::Quit, "Quitter le jeu", QUIT_BG),
        ],
    );
    let settings = (
        page(MenuPage::Settings),
        children![
            title("Paramètres"),
            section("Animations sur la carte"),
            (
                Node { width: Val::Px(WIDTH), column_gap: Val::Px(2.0), border_radius: BorderRadius::all(Val::Px(6.0)), overflow: Overflow::clip(), ..default() },
                Children::spawn(SpawnIter(AnimationSpeed::ALL.into_iter().map(speed_choice))),
            ),
            (SpeedHint, hint("")),
            section("Développement"),
            button(MenuButton::Debug, "Mode debug : désactivé", BUTTON_BG),
            hint("Révèle toute la carte (brouillard de guerre levé) et affiche les images par seconde."),
            spacer(),
            button(MenuButton::Back, "Retour (Échap)", BUTTON_BG),
            hint("Les paramètres sont enregistrés aussitôt et gardés d'une partie à l'autre."),
        ],
    );
    let new_world = (
        page(MenuPage::NewWorld),
        children![
            title("Nouveau monde"),
            (Text::new("La partie en cours sera perdue."), TextFont::from_font_size(14.0), TextColor(WARNING)),
            section("Graine du monde"),
            (
                Node { width: Val::Px(WIDTH), column_gap: Val::Px(6.0), ..default() },
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
            hint("Même graine, même monde. Tapez ou collez une graine (chiffres), puis Entrée ou le bouton. Ctrl+C / Ctrl+V : copier, coller."),
            button(MenuButton::Regenerate, "Regénérer avec cette graine", BUTTON_BG),
            button(MenuButton::NewWorld, "Graine au hasard", BUTTON_BG),
            spacer(),
            button(MenuButton::Back, "Retour (Échap)", BUTTON_BG),
        ],
    );
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
                padding: UiRect::all(Val::Px(24.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            children![main, settings, new_world],
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

/// Échap ouvre ou ferme le menu (sauf si le wiki est ouvert : Échap le ferme d'abord) ; sur
/// une sous-page, il revient à la page principale.
fn toggle_menu(keys: Res<ButtonInput<KeyCode>>, wiki: Res<WikiState>, mut menu: ResMut<GameMenu>) {
    if keys.just_pressed(KeyCode::Escape) && !wiki.open {
        if menu.open && menu.page != MenuPage::Main {
            menu.page = MenuPage::Main;
        } else {
            menu.open = !menu.open;
            menu.page = MenuPage::Main;
        }
    }
}

/// Champ de la graine : à l'ouverture de la page « Nouveau monde », il montre la graine du
/// monde actuel, toute sélectionnée (on peut la copier aussitôt, ou taper par-dessus) ; Entrée
/// regénère.
fn seed_field(
    menu: Res<GameMenu>,
    seed: Res<WorldSeed>,
    keys: Res<ButtonInput<Key>>,
    mut focus: ResMut<InputFocus>,
    mut was_shown: Local<bool>,
    mut field: Single<(Entity, &mut EditableText), With<SeedField>>,
    mut log: ResMut<GameLog>,
    mut exit: MessageWriter<AppExit>,
) {
    let (entity, ref mut editable) = *field;
    let shown = menu.open && menu.page == MenuPage::NewWorld;
    if shown && !*was_shown {
        editable.editor_mut().set_text(&seed.0.to_string());
        editable.queue_edit(TextEdit::SelectAll);
        focus.set(entity, FocusCause::Navigated);
    }
    if !shown && *was_shown && focus.get() == Some(entity) {
        focus.clear();
    }
    *was_shown = shown;
    if shown && focus.get() == Some(entity) && keys.just_pressed(Key::Enter) {
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
                MenuButton::Open(page) => menu.page = *page,
                MenuButton::Back => menu.page = MenuPage::Main,
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

/// Sélecteur de vitesse des animations : la case choisie est en bleu, la ligne d'aide dit ce
/// qu'elle fait.
fn speed_choices(
    mut settings: ResMut<Settings>,
    mut choices: Query<(Ref<Interaction>, &SpeedChoice, &mut BackgroundColor)>,
    mut hint: Single<&mut Text, With<SpeedHint>>,
) {
    for (interaction, choice, _) in &choices {
        if interaction.is_changed() && *interaction == Interaction::Pressed && settings.animation != choice.0 {
            settings.animation = choice.0;
        }
    }
    for (interaction, choice, mut bg) in &mut choices {
        let color = if settings.animation == choice.0 {
            CHOICE_ON
        } else if *interaction != Interaction::None {
            BUTTON_HOVER
        } else {
            BUTTON_BG
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
    let text = settings.animation.hint();
    if hint.0 != text {
        hint.0 = text.to_string();
    }
}

/// Enregistre les paramètres dès qu'ils changent.
fn save_settings(settings: Res<Settings>, mut log: ResMut<GameLog>) {
    if settings.is_changed()
        && !settings.is_added()
        && let Err(e) = crate::settings::save(&settings)
    {
        log.push(format!("Impossible d'enregistrer les paramètres : {e}"));
    }
}

fn show_menu(
    menu: Res<GameMenu>,
    debug: Res<DebugMode>,
    mut backdrop: Single<&mut Node, (With<MenuBackdrop>, Without<DebugBanner>, Without<PageNode>)>,
    mut banner: Single<&mut Node, (With<DebugBanner>, Without<MenuBackdrop>, Without<PageNode>)>,
    mut pages: Query<(&mut Node, &PageNode), (Without<MenuBackdrop>, Without<DebugBanner>)>,
    mut labels: Query<(&mut Text, &DebugLabelIf)>,
) {
    let display = |on: bool| if on { Display::Flex } else { Display::None };
    if backdrop.display != display(menu.open) {
        backdrop.display = display(menu.open);
    }
    for (mut node, page) in &mut pages {
        if node.display != display(page.0 == menu.page) {
            node.display = display(page.0 == menu.page);
        }
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
