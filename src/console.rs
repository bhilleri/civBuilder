//! Console de debug (touche ², seulement en mode debug) et scripts de commandes.
//!
//! - **Console** : une ligne de saisie sous le bandeau du mode debug, fermée par défaut. Tab
//!   complète (commande, puis valeurs : terrains, reliefs, cartes, critères), ↑ ↓ rappellent les
//!   commandes précédentes, la ligne du bas rappelle la forme de la commande tapée, Échap ou ²
//!   referme. Langage et aide : voir `debug_cmd.rs` (commande `aide`).
//! - **Script** : `CIVBUILDER_SCRIPT=<fichier>` exécute les commandes du fichier, une par image
//!   (lignes vides et `#` ignorés), en mode debug. Chaque commande et sa réponse sont écrites
//!   dans `CIVBUILDER_SCRIPT_OUT` (sinon `<script>.reponses.txt` à côté du script) et dans la
//!   console du terminal. Une capture attend que le relief 3D soit construit. Pensé pour les
//!   agents IA : mise en scène d'un cas, captures, lecture des réponses, `quitter`.
//!
//! Après une modification de la carte, `MapEdited` porte les cases dont le rendu est à refaire
//! (cases modifiées et voisines) : le rendu 2D (`render.rs`) et 3D (`view3d.rs`) les reconstruit.

use crate::debug_cmd::{self, Effect, Reply, Session, UnitChange};
use crate::rules::MAX_HP;
use crate::feedback::TurnReplay;
use crate::game::{City, DebugMode, EndTurnRequested, GameMap, GameState, HexPos, Selection, ShownFog, Territory, Unit, found_city, spawn_unit};
use crate::hostile::{Hostile, HostileHome};
use crate::hex::Hex;
use crate::hostile::WorldSeed;
use crate::input::HoveredHex;
use crate::render::MapLayer;
use crate::view3d::{Cam3d, ReliefPending, ViewMode, tile_height, to_3d};
use bevy::input::InputSystems;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::text::{EditableText, TextCursorStyle, TextEdit};
use bevy::ui_widgets::TextInput;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;

/// Cases dont les données ont changé (modifiées et voisines) : rendu 2D et 3D à refaire.
#[derive(Message, Clone, Debug)]
pub struct MapEdited(pub Vec<Hex>);

/// Noms des lieux refaits (`regenerer`) : étiquettes des cours d'eau et index de la recherche à
/// refaire.
#[derive(Message, Clone, Copy, Debug)]
pub struct NamesChanged;

/// Lignes de réponse affichées (les plus récentes) et gardées.
const SHOWN_LINES: usize = 16;
const KEPT_LINES: usize = 300;
/// Portée du rendu refait autour d'une case modifiée (relief 3D partagé avec les voisines).
const RENDER_REACH: i32 = 2;
/// Images avant la première commande d'un script (jeu prêt), et attente maximale du relief 3D
/// avant une capture.
const SCRIPT_START_FRAMES: u32 = 10;
const CAPTURE_MAX_WAIT: u32 = 900;
/// Images entre la fin du relief et la capture, puis entre la capture et la commande suivante
/// (fichier écrit).
const CAPTURE_SETTLE: u32 = 10;
const CAPTURE_SAVE: u32 = 20;

const PANEL_BG: Color = Color::srgba(0.04, 0.05, 0.08, 0.93);
const FIELD_BG: Color = Color::srgb(0.08, 0.09, 0.13);
const TEXT: Color = Color::srgb(0.90, 0.91, 0.94);
const MUTED: Color = Color::srgb(0.55, 0.58, 0.66);
const OK_COLOR: Color = Color::srgb(0.55, 0.85, 0.55);
const ERROR_COLOR: Color = Color::srgb(1.0, 0.50, 0.42);
const COMMAND_COLOR: Color = Color::srgb(0.98, 0.82, 0.50);

/// Script en cours : commandes restantes, dossier (captures), fichier des réponses, attente.
struct Script {
    lines: VecDeque<String>,
    dir: PathBuf,
    out: PathBuf,
    wait: u32,
}

/// Travelling en cours : départ et arrivée (point visé en 3D, centre en 2D, distance de la
/// caméra 3D), images faites et à faire, durées des images (ms).
struct Travel {
    from: (Vec3, Vec2, f32),
    to: (Vec3, Vec2, f32),
    done: u32,
    frames: u32,
    times: Vec<f32>,
}

/// Capture demandée : fichier, images déjà attendues, capture faite (attente de l'écriture).
struct PendingCapture {
    path: PathBuf,
    waited: u32,
    settled: u32,
    shot: bool,
}

#[derive(Resource, Default)]
pub struct Console {
    pub open: bool,
    session: Session,
    output: VecDeque<String>,
    history: Vec<String>,
    /// Position dans l'historique pendant qu'on le parcourt (↑ ↓).
    browsing: Option<usize>,
    candidates: Vec<String>,
    dirty: bool,
    script: Option<Script>,
    capture: Option<PendingCapture>,
    travel: Option<Travel>,
    /// Tours à jouer (`tour n`) et tour attendu pour celui en cours.
    turns: Option<(u32, Option<u32>)>,
    /// Modification à l'image précédente : le brouillard affiché est marqué changé une seconde
    /// fois (décor et motifs recréés visibles à leur tour).
    touch_fog: bool,
}

impl Console {
    fn push(&mut self, line: String) {
        self.output.push_back(line);
        while self.output.len() > KEPT_LINES {
            self.output.pop_front();
        }
        self.dirty = true;
    }
}

#[derive(Component)]
struct ConsoleRoot;
#[derive(Component)]
struct ConsoleOutput;
#[derive(Component)]
struct ConsoleInput;
#[derive(Component)]
struct ConsoleCandidates;
#[derive(Component)]
struct ConsoleHint;

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        let mut console = Console::default();
        if let Some(path) = std::env::var_os("CIVBUILDER_SCRIPT").map(PathBuf::from) {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    let out = std::env::var_os("CIVBUILDER_SCRIPT_OUT").map(PathBuf::from).unwrap_or_else(|| {
                        let stem = path.file_stem().map_or("script".into(), |s| s.to_string_lossy().into_owned());
                        path.with_file_name(format!("{stem}.reponses.txt"))
                    });
                    // (fichier des réponses vidé au départ)
                    if let Err(e) = std::fs::write(&out, "") {
                        error!("Script : fichier des réponses {} impossible à écrire ({e})", out.display());
                    }
                    info!("Script {} : {} ligne(s), réponses dans {}", path.display(), text.lines().count(), out.display());
                    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
                    console.script = Some(Script { lines: text.lines().map(String::from).collect(), dir, out, wait: SCRIPT_START_FRAMES });
                }
                Err(e) => error!("Script {} illisible ({e})", path.display()),
            }
        }
        app.insert_resource(console)
            .add_message::<MapEdited>()
            .add_message::<NamesChanged>()
            .add_systems(Startup, setup_console)
            .add_systems(PreUpdate, console_keys.after(InputSystems).after(crate::search::capture_keyboard))
            .add_systems(Update, (run_script, run_travel, run_capture, run_turns, touch_fog, refresh_console).chain());
    }
}

fn text(s: &str, size: f32, color: Color) -> impl Bundle + use<> {
    (Text::new(s.to_string()), TextFont::from_font_size(size), TextColor(color))
}

fn setup_console(mut commands: Commands) {
    commands.spawn((
        ConsoleRoot,
        Node {
            position_type: PositionType::Absolute,
            // Sous le bandeau du mode debug
            top: Val::Px(84.0),
            left: Val::Percent(50.0),
            width: Val::Px(780.0),
            max_width: Val::Percent(96.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(8.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            display: Display::None,
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
        BackgroundColor(PANEL_BG),
        // Bloque les clics vers la carte
        Interaction::default(),
        GlobalZIndex(70),
        children![
            (ConsoleOutput, Node { flex_direction: FlexDirection::Column, ..default() }),
            (ConsoleCandidates, text("", 12.5, MUTED)),
            (
                Node { align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                BackgroundColor(FIELD_BG),
                children![
                    text(">", 15.0, COMMAND_COLOR),
                    (
                        ConsoleInput,
                        TextInput,
                        EditableText { max_characters: Some(200), ..default() },
                        TextFont::from_font_size(15.0),
                        TextColor(TEXT),
                        TextLayout::no_wrap(),
                        TextCursorStyle { color: Color::WHITE, selection_color: Color::srgb(0.20, 0.40, 0.75), ..default() },
                        Node { flex_grow: 1.0, ..default() },
                    ),
                ],
            ),
            (ConsoleHint, text("", 12.0, MUTED)),
        ],
    ));
}

/// Remplace le texte du champ.
fn set_field(editable: &mut EditableText, value: &str) {
    editable.queue_edit(TextEdit::SelectAll);
    editable.queue_edit(TextEdit::Backspace);
    if !value.is_empty() {
        editable.queue_edit(TextEdit::Insert(value.into()));
    }
}

/// Touches de la console : ² ouvre et ferme, Échap ferme, Tab complète, ↑ ↓ historique, Entrée
/// exécute. Passe après la recherche, qui coupe les raccourcis du jeu pendant la saisie.
#[allow(clippy::too_many_arguments)]
fn console_keys(
    mut raw: MessageReader<KeyboardInput>,
    logical: Res<ButtonInput<Key>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    debug: Res<DebugMode>,
    menu: Res<crate::menu::GameMenu>,
    mut console: ResMut<Console>,
    mut focus: ResMut<InputFocus>,
    mut input: Single<(Entity, &mut EditableText), With<ConsoleInput>>,
    mut run: Local<Vec<String>>,
    mut ctx: ConsoleContext,
) {
    let toggle = raw.read().any(|k| k.key_code == KeyCode::Backquote && k.state.is_pressed() && !k.repeat);
    let (entity, ref mut editable) = *input;
    let value: String = editable.value().to_string();
    // (le ² tapé pour ouvrir ou fermer arrive aussi dans le champ : retiré)
    if value.contains(['²', '`']) {
        set_field(editable, &value.replace(['²', '`'], ""));
    }
    if (toggle && console.open) || (console.open && (!debug.0 || menu.open)) || (console.open && logical.just_pressed(Key::Escape)) {
        console.open = false;
        keys.reset(KeyCode::Escape);
        if focus.get() == Some(entity) {
            focus.clear();
        }
        return;
    }
    if toggle && debug.0 && !menu.open {
        console.open = true;
        console.dirty = true;
        if console.output.is_empty() {
            console.push("Console de debug — « aide » pour la liste des commandes, Tab pour compléter.".into());
        }
    }
    if !console.open {
        return;
    }
    if focus.get() != Some(entity) {
        focus.set(entity, FocusCause::Navigated);
    }
    let value = value.replace(['²', '`'], "");
    if logical.just_pressed(Key::Tab) {
        let c = debug_cmd::complete(&value);
        if c.line != value {
            set_field(editable, &c.line);
        }
        console.candidates = c.candidates;
        console.dirty = true;
    }
    let browse = |console: &mut Console, up: bool| -> Option<String> {
        let n = console.history.len();
        if n == 0 {
            return None;
        }
        let at = match (console.browsing, up) {
            (None, true) => n - 1,
            (None, false) => return None,
            (Some(i), true) => i.saturating_sub(1),
            (Some(i), false) if i + 1 < n => i + 1,
            (Some(_), false) => {
                console.browsing = None;
                return Some(String::new());
            }
        };
        console.browsing = Some(at);
        Some(console.history[at].clone())
    };
    for (key, up) in [(Key::ArrowUp, true), (Key::ArrowDown, false)] {
        if logical.just_pressed(key)
            && let Some(line) = browse(&mut console, up)
        {
            set_field(editable, &line);
            editable.queue_edit(TextEdit::TextEnd(false));
        }
    }
    if logical.just_pressed(Key::Enter) && !value.trim().is_empty() {
        set_field(editable, "");
        console.browsing = None;
        console.candidates.clear();
        if console.history.last() != Some(&value) {
            console.history.push(value.clone());
        }
        run.push(value);
    }
    for line in run.drain(..) {
        execute(&mut console, &mut ctx, &line);
    }
}

/// Ressources touchées par les commandes.
#[derive(bevy::ecs::system::SystemParam)]
struct ConsoleContext<'w, 's> {
    map: ResMut<'w, GameMap>,
    shown: ResMut<'w, ShownFog>,
    hovered: Res<'w, HoveredHex>,
    mode: ResMut<'w, ViewMode>,
    cam: ResMut<'w, Cam3d>,
    layer: ResMut<'w, MapLayer>,
    geo: ResMut<'w, crate::render::GeoView>,
    tide_lines: ResMut<'w, crate::render::TideLines>,
    seed: Option<Res<'w, WorldSeed>>,
    edited: MessageWriter<'w, MapEdited>,
    renamed: MessageWriter<'w, NamesChanged>,
    exit: MessageWriter<'w, AppExit>,
    camera_2d: Query<'w, 's, &'static mut Transform, (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
    commands: Commands<'w, 's>,
    game: (ResMut<'w, GameState>, ResMut<'w, Territory>, ResMut<'w, Selection>),
    cities: Query<'w, 's, &'static HexPos, With<City>>,
    units: Query<'w, 's, (Entity, &'static mut Unit, &'static HexPos, Has<Hostile>)>,
    /// Cours d'eau ou lac survolés : leurs numéros changent quand l'eau est refaite.
    pointed: (ResMut<'w, crate::highlight::HoveredLabel>, ResMut<'w, crate::highlight::PointedFeature>),
}

/// Exécute une ligne (console ou script) et applique ses effets ; la réponse s'affiche.
fn execute(console: &mut Console, ctx: &mut ConsoleContext, line: &str) -> Reply {
    let focus = if ctx.mode.is_3d {
        Hex::from_world(ctx.cam.target.x, -ctx.cam.target.z)
    } else {
        ctx.camera_2d.single().map_or(Hex::default(), |t| Hex::from_world(t.translation.x, t.translation.y))
    };
    let focus = ctx.map.0.canonical(focus);
    let reply = console.session.run(&mut ctx.map.0, line, ctx.hovered.0, focus);
    if !line.trim().is_empty() && !line.trim_start().starts_with('#') {
        console.push(format!("> {line}"));
    }
    if reply.ok && debug_cmd::token(line.split_whitespace().next().unwrap_or("")) == "regenerer" {
        ctx.renamed.write(NamesChanged);
        ctx.pointed.0.0 = None;
        ctx.pointed.1.0 = None;
    }
    if !reply.changed.is_empty() {
        ctx.edited.write(MapEdited(debug_cmd::render_area(&ctx.map.0, &reply.changed, RENDER_REACH)));
        ctx.shown.set_changed();
        console.touch_fog = true;
    }
    let mut lines = reply.lines.clone();
    let mut ok = reply.ok;
    let mut refuse = |lines: &mut Vec<String>, message: String| {
        *lines = vec![format!("ERREUR {message}")];
        ok = false;
    };
    match &reply.effect {
        Some(Effect::Unit(kind, at, hostile)) => {
            if ctx.units.iter().any(|(_, _, p, _)| p.0 == *at) {
                refuse(&mut lines, format!("{} : une unité occupe déjà la case", debug_cmd::coords(&ctx.map.0, *at)));
            } else {
                let e = spawn_unit(&mut ctx.commands, *kind, *at);
                if *hostile {
                    ctx.commands.entity(e).insert((Hostile, HostileHome(*at)));
                }
            }
        }
        Some(Effect::City(at)) => {
            let existing: Vec<Hex> = ctx.cities.iter().map(|p| p.0).collect();
            let settler = ctx.commands.spawn_empty().id();
            let (state, territory, _) = &mut ctx.game;
            match found_city(&mut ctx.commands, settler, *at, &ctx.map.0, &existing, state, territory) {
                Ok((_, name)) => lines = vec![format!("OK ville {name} fondée en {}", debug_cmd::coords(&ctx.map.0, *at))],
                Err(e) => {
                    ctx.commands.entity(settler).despawn();
                    refuse(&mut lines, format!("ville impossible en {} : {e}", debug_cmd::coords(&ctx.map.0, *at)));
                }
            }
        }
        Some(Effect::Remove(at)) => {
            let gone: Vec<Entity> = ctx.units.iter().filter(|(_, _, p, _)| p.0 == *at).map(|(e, ..)| e).collect();
            let selection = &mut ctx.game.2;
            for &e in &gone {
                if selection.0 == Some(e) {
                    selection.0 = None;
                }
                ctx.commands.entity(e).despawn();
            }
            lines = vec![format!("OK {} unité(s) retirée(s) de {}", gone.len(), debug_cmd::coords(&ctx.map.0, *at))];
        }
        Some(Effect::UnitSet(at, change)) => {
            let place = debug_cmd::coords(&ctx.map.0, *at);
            match ctx.units.iter_mut().find(|(_, _, p, _)| p.0 == *at) {
                None => refuse(&mut lines, format!("aucune unité en {place}")),
                Some((e, mut unit, _, hostile)) => {
                    let u = &mut unit.0;
                    let max = u.kind.max_moves();
                    match *change {
                        UnitChange::Hp(hp) => u.hp = hp,
                        UnitChange::Moves(m) => {
                            u.moves_left = m.min(max);
                            u.immobilized &= u.moves_left == 0;
                        }
                        UnitChange::Debt(d) => u.move_debt = d,
                        UnitChange::Heal => {
                            u.hp = MAX_HP;
                            u.moves_left = max;
                            u.move_debt = 0;
                            u.immobilized = false;
                        }
                    }
                    if u.hp <= 0 {
                        // (dispersée : retirée de la carte, comme après un combat perdu)
                        lines = vec![format!("OK {} en {place} : dispersée (pv=0), retirée de la carte", debug_cmd::token(u.kind.name()))];
                        if ctx.game.2.0 == Some(e) {
                            ctx.game.2.0 = None;
                        }
                        ctx.commands.entity(e).despawn();
                    } else {
                        lines = vec![format!("OK {}", describe_unit(u, hostile, &place))];
                    }
                }
            }
        }
        Some(Effect::UnitsAt(at)) => {
            let place = debug_cmd::coords(&ctx.map.0, *at);
            let here: Vec<String> = ctx.units.iter().filter(|(_, _, p, _)| p.0 == *at).map(|(_, u, _, hostile)| format!("unite={}", describe_unit(&u.0, hostile, &place))).collect();
            lines.push(if here.is_empty() { "unite=aucune".into() } else { here.join(" ; ") });
        }
        Some(Effect::Turns(n)) => {
            console.turns = Some((*n, None));
        }
        Some(Effect::Goto(h)) => {
            ctx.cam.target = to_3d(*h, tile_height(&ctx.map.0, *h));
            if let Ok(mut t) = ctx.camera_2d.single_mut() {
                (t.translation.x, t.translation.y) = h.to_world();
            }
        }
        Some(Effect::View(is_3d)) => {
            if ctx.mode.is_3d != *is_3d {
                ctx.mode.is_3d = *is_3d;
            }
        }
        Some(Effect::Camera { distance, yaw_deg }) => {
            ctx.cam.distance = *distance;
            if let Some(y) = yaw_deg {
                ctx.cam.yaw = y.to_radians();
            }
        }
        Some(Effect::Layer(l)) => {
            if *ctx.layer != *l {
                *ctx.layer = *l;
            }
        }
        Some(Effect::TideLines(on)) => {
            if ctx.tide_lines.lines != *on {
                ctx.tide_lines.lines = *on;
            }
        }
        Some(Effect::TideArrows(on)) => {
            if ctx.tide_lines.arrows != *on {
                ctx.tide_lines.arrows = *on;
            }
        }
        Some(Effect::Geo(family)) => {
            if *ctx.layer != MapLayer::Geography {
                *ctx.layer = MapLayer::Geography;
            }
            if ctx.geo.family != *family {
                ctx.geo.family = *family;
            }
        }
        Some(Effect::Capture(name)) => {
            let mut path = PathBuf::from(name);
            if path.extension().is_none() {
                path.set_extension("png");
            }
            if path.is_relative() {
                let dir = console.script.as_ref().map_or_else(|| std::env::temp_dir().join("civbuilder"), |s| s.dir.clone());
                path = dir.join(path);
            }
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            lines = vec![format!("OK capture : {}", path.display())];
            console.capture = Some(PendingCapture { path, waited: 0, settled: 0, shot: false });
        }
        Some(Effect::Travel { to, distance, frames }) => {
            let center = ctx.camera_2d.single().map_or(Vec2::ZERO, |t| t.translation.truncate());
            let (x, y) = to.to_world();
            let from = (ctx.cam.target, center, ctx.cam.distance);
            let to = (to_3d(*to, tile_height(&ctx.map.0, *to)), Vec2::new(x, y), *distance);
            console.travel = Some(Travel { from, to, done: 0, frames: *frames, times: Vec::new() });
        }
        Some(Effect::Wait(n)) => {
            if let Some(s) = console.script.as_mut() {
                s.wait = *n;
            }
        }
        Some(Effect::SaveJournal(name)) => {
            let (w, h) = (ctx.map.0.width, ctx.map.0.height);
            let seed = ctx.seed.as_ref().map_or("?".to_string(), |s| s.0.to_string());
            let mut out = format!("# CivBuilder — graine {seed}, carte {w}x{h}\n# CIVBUILDER_SEED={seed} CIVBUILDER_MAP={w}x{h} CIVBUILDER_SCRIPT=<ce fichier> cargo run\n");
            for l in &console.session.journal {
                out += l;
                out.push('\n');
            }
            lines = match std::fs::write(name, out) {
                Ok(()) => vec![format!("OK journal écrit : {name} ({} commande(s))", console.session.journal.len())],
                Err(e) => vec![format!("ERREUR journal impossible à écrire : {name} ({e})")],
            };
        }
        Some(Effect::Clear) => console.output.clear(),
        Some(Effect::Quit) => {
            ctx.exit.write(AppExit::Success);
        }
        None => {}
    }
    for l in &lines {
        console.push(l.clone());
    }
    // Réponses dans le terminal (et dans le fichier du script)
    info!("console> {line}\n{}", lines.join("\n"));
    if let Some(s) = &console.script {
        let mut text = format!("> {line}\n");
        for l in &lines {
            text += l;
            text.push('\n');
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(&s.out) {
            let _ = f.write_all(text.as_bytes());
        }
    }
    Reply { lines, ok, ..reply }
}

/// État d'une unité en une ligne : « milice joueur en 90 45 pv=40/100 pm=1/2 dette=0 ».
fn describe_unit(u: &crate::rules::UnitState, hostile: bool, place: &str) -> String {
    format!(
        "{} {} en {place} pv={}/{MAX_HP} pm={}/{} dette={}{}",
        debug_cmd::token(u.kind.name()),
        if hostile { "hostile" } else { "joueur" },
        u.hp,
        u.moves_left,
        u.kind.max_moves(),
        u.move_debt,
        if u.immobilized { " immobilisee" } else { "" }
    )
}

/// Script : une commande par image, sauf pendant une attente ou une capture.
fn run_script(mut console: ResMut<Console>, mut debug: ResMut<DebugMode>, mut ctx: ConsoleContext) {
    let capturing = console.capture.is_some();
    let playing = console.turns.is_some();
    let travelling = console.travel.is_some();
    let Some(script) = console.script.as_mut() else { return };
    if !debug.0 {
        debug.0 = true;
    }
    if script.wait > 0 {
        script.wait -= 1;
        return;
    }
    if capturing || playing || travelling {
        return;
    }
    // (lignes vides et commentaires passés d'un coup)
    let line = loop {
        match script.lines.pop_front() {
            Some(l) if l.trim().is_empty() || l.trim_start().starts_with('#') => continue,
            Some(l) => break Some(l),
            None => break None,
        }
    };
    match line {
        Some(line) => {
            execute(&mut console, &mut ctx, &line);
        }
        None => {
            info!("Script terminé (réponses dans {})", script.out.display());
            let out = script.out.clone();
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&out) {
                let _ = f.write_all(b"# fin du script\n");
            }
            console.script = None;
        }
    }
}

/// Travelling (`travelling`) : la vue avance d'un pas par image (départ et arrivée en douceur), le
/// zoom suit (en 2D, l'échelle équivalente à la distance de la caméra 3D) ; à la fin, les durées
/// des images (la première, celle de la commande, n'est pas comptée). La souris est tenue au
/// milieu de la vue (souris pilotée) : la case survolée change en chemin, comme quand on se
/// déplace sur la carte (infobulle, surbrillance).
fn run_travel(mut console: ResMut<Console>, time: Res<Time>, mut cam: ResMut<Cam3d>, mode: Res<ViewMode>, mut pilot: ResMut<crate::input::Pilot>, mut camera_2d: Query<(&mut Transform, &mut Projection), (With<Camera2d>, Without<crate::seam::GhostCamera>)>) {
    let Some(travel) = console.travel.as_mut() else { return };
    if travel.done > 0 {
        travel.times.push(time.delta_secs() * 1000.0);
    }
    travel.done += 1;
    let t = (travel.done as f32 / travel.frames as f32).min(1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let ((a3, a2, ad), (b3, b2, bd)) = (travel.from, travel.to);
    cam.target = a3.lerp(b3, t);
    // (zoom en échelle logarithmique : la vitesse apparente reste la même)
    cam.distance = (ad.ln() + (bd.ln() - ad.ln()) * t).exp();
    let p = a2.lerp(b2, t);
    pilot.active = true;
    pilot.cursor = Some(if mode.is_3d { Hex::from_world(cam.target.x, -cam.target.z) } else { Hex::from_world(p.x, p.y) });
    if let Ok((mut transform, mut projection)) = camera_2d.single_mut() {
        (transform.translation.x, transform.translation.y) = (p.x, p.y);
        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.scale = (cam.distance / 700.0).clamp(0.3, 8.0);
        }
    }
    if travel.done <= travel.frames {
        return;
    }
    let times = std::mem::take(&mut travel.times);
    console.travel = None;
    (pilot.active, pilot.cursor) = (false, None);
    let n = times.len().max(1) as f32;
    let mean = times.iter().sum::<f32>() / n;
    let worst = times.iter().copied().fold(0.0f32, f32::max);
    let jolts = |ms: f32| times.iter().filter(|&&x| x > ms).count();
    let line = format!("OK travelling fini : {} images, moyenne {mean:.1} ms, pire {worst:.0} ms, à-coups {} au-delà de 33 ms, {} au-delà de 50 ms, {} au-delà de 100 ms", times.len(), jolts(33.0), jolts(50.0), jolts(100.0));
    info!("console> {line}");
    if let Some(s) = &console.script
        && let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&s.out)
    {
        let _ = writeln!(f, "{line}");
    }
    console.push(line);
}

/// `tour n` : fin de tour demandée, puis attente du tour suivant et de la fin de ses animations,
/// autant de fois que demandé.
fn run_turns(mut console: ResMut<Console>, state: Res<GameState>, replay: Res<TurnReplay>, mut end_turn: ResMut<EndTurnRequested>) {
    let Some((left, waiting)) = console.turns else { return };
    match waiting {
        Some(target) if state.turn >= target && !replay.playing && !end_turn.0 => {
            let left = left - 1;
            console.turns = (left > 0).then_some((left, None));
            if left == 0 {
                let line = format!("OK tour {} : à vous de jouer", state.turn);
                info!("console> {line}");
                if let Some(s) = &console.script
                    && let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&s.out)
                {
                    let _ = writeln!(f, "{line}");
                }
                console.push(line);
            }
        }
        Some(_) => {}
        None if !replay.playing && !end_turn.0 => {
            end_turn.0 = true;
            console.turns = Some((left, Some(state.turn + 1)));
        }
        None => {}
    }
}

/// Capture : console cachée, relief 3D construit (ou attente maximale), puis écran enregistré.
fn run_capture(mut commands: Commands, mut console: ResMut<Console>, pending: Res<ReliefPending>, mode: Res<ViewMode>) {
    let Some(c) = console.capture.as_mut() else { return };
    if c.shot {
        c.waited += 1;
        if c.waited >= CAPTURE_SAVE {
            console.capture = None;
            console.dirty = true;
        }
        return;
    }
    c.waited += 1;
    // (en 2D, le relief 3D se prépare en arrière-plan : il n'y a pas à l'attendre)
    if mode.is_3d && pending.0 > 0 && c.waited < CAPTURE_MAX_WAIT {
        c.settled = 0;
        return;
    }
    c.settled += 1;
    if c.settled < CAPTURE_SETTLE {
        return;
    }
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(c.path.clone()));
    c.shot = true;
    c.waited = 0;
}

/// Image qui suit une modification : brouillard affiché marqué changé de nouveau (le décor et les
/// motifs de relief recréés à l'image précédente prennent leur visibilité).
fn touch_fog(mut console: ResMut<Console>, mut shown: ResMut<ShownFog>) {
    if console.touch_fog {
        console.touch_fog = false;
        shown.set_changed();
    }
}

/// Affichage : panneau ouvert ou fermé (caché pendant une capture), dernières réponses,
/// propositions de la complétion, aide de la saisie en cours.
fn refresh_console(
    mut commands: Commands,
    mut console: ResMut<Console>,
    mut root: Single<&mut Node, With<ConsoleRoot>>,
    output: Single<Entity, With<ConsoleOutput>>,
    input: Single<&EditableText, With<ConsoleInput>>,
    mut candidates: Single<&mut Text, (With<ConsoleCandidates>, Without<ConsoleHint>)>,
    mut hint: Single<&mut Text, (With<ConsoleHint>, Without<ConsoleCandidates>)>,
    mut last_value: Local<String>,
) {
    let display = if console.open && console.capture.is_none() { Display::Flex } else { Display::None };
    if root.display != display {
        root.display = display;
    }
    if !console.open {
        return;
    }
    let value = input.value().to_string();
    if value != *last_value {
        // (les propositions valent pour le mot en cours : effacées quand on continue de taper)
        if !last_value.is_empty() || value.is_empty() {
            console.candidates.clear();
        }
        *last_value = value.clone();
        let h = debug_cmd::hint(&value);
        if hint.0 != h {
            hint.0 = h;
        }
        console.dirty = true;
    } else if hint.0.is_empty() {
        hint.0 = debug_cmd::hint(&value);
    }
    if !console.dirty {
        return;
    }
    console.dirty = false;
    let c = if console.candidates.is_empty() { String::new() } else { format!("Tab : {}", console.candidates.join("  ")) };
    if candidates.0 != c {
        candidates.0 = c;
    }
    commands.entity(*output).despawn_related::<Children>();
    let from = console.output.len().saturating_sub(SHOWN_LINES);
    let lines: Vec<String> = console.output.iter().skip(from).cloned().collect();
    commands.entity(*output).with_children(|o| {
        for l in lines {
            let color = if l.starts_with("> ") {
                COMMAND_COLOR
            } else if l.starts_with("OK") {
                OK_COLOR
            } else if l.starts_with("ERREUR") {
                ERROR_COLOR
            } else {
                TEXT
            };
            o.spawn(text(&l, 13.0, color));
        }
    });
}
