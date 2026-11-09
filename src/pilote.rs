//! Mode joueur pilotable, pour les agents de test : avec `CIVBUILDER_PILOTE=<fichier>`, le jeu suit
//! le fichier d'ordres pendant la partie (une ligne ajoutée = un ordre), sans mode debug : le
//! brouillard reste actif, et un agent peut réagir à ce qu'il voit. Réponses dans
//! `<fichier>.reponses.txt` (ou `CIVBUILDER_SCRIPT_OUT`). Voir `scripts/pilote.sh`.
//!
//! Les ordres passent par les vraies commandes du joueur : souris pilotée (`input::Pilot`) pour la
//! carte, messages de clavier et de souris injectés avant la lecture des entrées pour le reste
//! (la vraie souris n'est pas déplacée : plusieurs parties peuvent tourner en même temps). Les
//! réponses ne donnent que ce que le joueur peut savoir : cases explorées, ennemis en vue, texte de
//! l'infobulle. Les commandes de console qui modifient la carte ou la révèlent sont refusées.

use crate::debug_cmd::{coords, token};
use crate::feedback::TurnReplay;
use crate::game::{City, EndTurnRequested, Fog, GameLog, GameMap, GameState, HexPos, Selection, Unit};
use crate::hex::Hex;
use crate::hostile::{Camp, Hostile, SiteName, Village};
use crate::input::{HoverPath, Pilot};
use crate::map::{MoveCosts, WorldMap};
use crate::pointer::UiPointer;
use crate::render::MapLayer;
use crate::rules::MAX_HP;
use crate::ui::HoverText;
use crate::view3d::ViewMode;
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseButtonInput;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::{CursorMoved, PrimaryWindow, WindowEvent};
use std::collections::VecDeque;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::PathBuf;

/// Commandes de la console permises en mode joueur : la caméra, les cartes, les captures et la fin
/// de tour, que le joueur a aussi. Rien qui modifie la carte ou révèle ce qui est inexploré.
const CONSOLE_COMMANDS: [&str; 11] = ["aller", "vue", "camera", "carte", "geographie", "lignes-maree", "fleches-maree", "capture", "attendre", "tour", "quitter"];

/// Rayon maximal de `alentours` (réponse lisible).
const MAX_RADIUS: i32 = 6;
/// Images d'attente au plus pour la fin d'un passage de tour (ensuite, réponse quand même).
const MAX_FRAMES: u32 = 6000;

const HELP: &str = "Ordres du joueur :\n\
etat : tour, or, unités, villes, ennemis en vue, villages et camps connus, derniers messages\n\
alentours [col lig] [rayon] : cases explorées autour d'une case (par défaut la sélection, rayon 2, 6 au plus)\n\
survol col lig : place la souris sur la case et rend le texte de l'infobulle et le trajet prévu\n\
clic col lig [côté] : clic gauche sur la case (sélection) ; avec un côté (e, ne, no, o, so, se), près de ce bord (arête d'une rivière, emplacement d'un pont)\n\
clic-droit col lig : clic droit sur la case (ordre : déplacer, attaquer)\n\
touche nom : appui sur une touche (a à z, 0 à 9, entree, tab, espace, echap, haut, bas, gauche, droite, f1 à f12)\n\
interface : boutons visibles de l'interface, avec leur numéro et leur position\n\
bouton numéro|texte : clic sur un bouton de l'interface\n\
clic-ecran x y : clic gauche sur un point de l'écran, en pixels\n\
zoom échelle : zoom de la vue 2D (0,3 à 8 ; petit = de près)\n\
Console : aller, vue, camera, carte, geographie, lignes-maree, fleches-maree, capture, attendre, tour, quitter";

/// Nom d'une touche → code physique et touche logique.
fn key_of(name: &str) -> Option<(KeyCode, Key)> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI,
        KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR,
        KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    ];
    const FUNCTIONS: [(KeyCode, Key); 12] = [
        (KeyCode::F1, Key::F1), (KeyCode::F2, Key::F2), (KeyCode::F3, Key::F3), (KeyCode::F4, Key::F4), (KeyCode::F5, Key::F5), (KeyCode::F6, Key::F6),
        (KeyCode::F7, Key::F7), (KeyCode::F8, Key::F8), (KeyCode::F9, Key::F9), (KeyCode::F10, Key::F10), (KeyCode::F11, Key::F11), (KeyCode::F12, Key::F12),
    ];
    let name = token(name);
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        let code = match c {
            'a'..='z' => LETTERS[c as usize - 'a' as usize],
            '0'..='9' => DIGITS[c as usize - '0' as usize],
            _ => return None,
        };
        return Some((code, Key::Character(c.to_string().as_str().into())));
    }
    match name.as_str() {
        "entree" => Some((KeyCode::Enter, Key::Enter)),
        "tab" => Some((KeyCode::Tab, Key::Tab)),
        "espace" => Some((KeyCode::Space, Key::Space)),
        "echap" => Some((KeyCode::Escape, Key::Escape)),
        "haut" => Some((KeyCode::ArrowUp, Key::ArrowUp)),
        "bas" => Some((KeyCode::ArrowDown, Key::ArrowDown)),
        "gauche" => Some((KeyCode::ArrowLeft, Key::ArrowLeft)),
        "droite" => Some((KeyCode::ArrowRight, Key::ArrowRight)),
        f => f.strip_prefix('f').and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=12).contains(n)).map(|n| FUNCTIONS[n - 1].clone()),
    }
}

/// Ordre du joueur décodé.
#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    State,
    Around(Option<Hex>, i32),
    Hover(Hex),
    /// Case, bouton, et côté de la case visé (son bord ; sinon son centre).
    Click(Hex, MouseButton, Option<usize>),
    Key(KeyCode, Key),
    Buttons,
    /// Numéro ou texte du bouton.
    Button(String),
    ScreenClick(Vec2),
    Zoom(f32),
}

/// Destination d'une ligne du fichier d'ordres.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// Commande de la console (exécutée par `console::execute`).
    Console,
    /// Réponse immédiate, sans effet sur la partie (repère, aide, refus).
    Answer(Vec<String>),
    Game(Order),
}

/// Case désignée par « colonne ligne » : le bord est-ouest boucle, les pôles sont les bords.
fn parse_cell(map: &WorldMap, col: &str, row: &str) -> Result<Hex, String> {
    let (Ok(c), Ok(r)) = (col.parse::<i32>(), row.parse::<i32>()) else {
        return Err(format!("coordonnées illisibles : {col} {row}"));
    };
    let hex = map.canonical(Hex::from_offset(c, r));
    if map.contains(hex) { Ok(hex) } else { Err(format!("case hors de la carte : {col} {row}")) }
}

/// Décode une ligne du fichier d'ordres.
pub fn route(map: &WorldMap, line: &str) -> Route {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some(&first) = words.first() else { return Route::Answer(Vec::new()) };
    if first.starts_with('#') {
        return Route::Answer(Vec::new());
    }
    let refuse = |message: String| Route::Answer(vec![format!("ERREUR {message}")]);
    let head = token(first);
    let cell = |usage: &str| match words[1..] {
        [c, r] => parse_cell(map, c, r),
        _ => Err(format!("usage : {usage}")),
    };
    let order = match head.as_str() {
        "repere" => return Route::Answer(vec![format!("# repere {}", words[1..].join(" "))]),
        "aide" => return Route::Answer(HELP.lines().map(String::from).collect()),
        "etat" => Ok(Order::State),
        "alentours" => match words[1..] {
            [] => Ok(Order::Around(None, 2)),
            [radius] => radius.parse().map(|n| Order::Around(None, n)).map_err(|_| format!("rayon illisible : {radius}")),
            [c, r] => parse_cell(map, c, r).map(|h| Order::Around(Some(h), 2)),
            [c, r, radius] => parse_cell(map, c, r).and_then(|h| radius.parse().map(|n| Order::Around(Some(h), n)).map_err(|_| format!("rayon illisible : {radius}"))),
            _ => Err("usage : alentours [col lig] [rayon]".to_string()),
        },
        "survol" => cell("survol col lig").map(Order::Hover),
        "clic" | "clic-droit" => {
            let button = if head == "clic" { MouseButton::Left } else { MouseButton::Right };
            let side = |w: &str| ["e", "ne", "no", "o", "so", "se"].iter().position(|d| *d == token(w)).ok_or(format!("côté illisible : {w} (e, ne, no, o, so, se)"));
            match words[1..] {
                [c, r] => parse_cell(map, c, r).map(|h| Order::Click(h, button, None)),
                [c, r, d] => parse_cell(map, c, r).and_then(|h| side(d).map(|d| Order::Click(h, button, Some(d)))),
                _ => Err(format!("usage : {head} col lig [e|ne|no|o|so|se]")),
            }
        }
        "touche" => match words[1..] {
            [name] => key_of(name).map(|(code, key)| Order::Key(code, key)).ok_or(format!("touche inconnue : {name} (aide)")),
            _ => Err("usage : touche nom".to_string()),
        },
        "interface" => Ok(Order::Buttons),
        "bouton" if words.len() > 1 => Ok(Order::Button(words[1..].join(" "))),
        "bouton" => Err("usage : bouton numéro|texte".to_string()),
        "clic-ecran" => match words[1..] {
            [x, y] => match (x.parse(), y.parse()) {
                (Ok(x), Ok(y)) => Ok(Order::ScreenClick(Vec2::new(x, y))),
                _ => Err(format!("position illisible : {x} {y}")),
            },
            _ => Err("usage : clic-ecran x y".to_string()),
        },
        "zoom" => match words[1..] {
            [scale] => scale.replace(',', ".").parse::<f32>().map(|s| Order::Zoom(s.clamp(0.3, 8.0))).map_err(|_| format!("échelle illisible : {scale}")),
            _ => Err("usage : zoom échelle".to_string()),
        },
        name if CONSOLE_COMMANDS.contains(&name) => return Route::Console,
        // (préparer une scène de test : toutes les commandes de la console, avec `CIVBUILDER_PILOTE_DEBUG`)
        _ if std::env::var_os("CIVBUILDER_PILOTE_DEBUG").is_some() => return Route::Console,
        _ => Err(format!("« {first} » n'existe pas en mode joueur (aide : liste des ordres)")),
    };
    match order {
        Ok(order) => Route::Game(order),
        Err(message) => refuse(message),
    }
}

/// Fichier d'ordres suivi pendant la partie : seules les lignes complètes sont lues.
pub struct LiveFile {
    path: PathBuf,
    offset: u64,
}

impl LiveFile {
    /// Suit ce fichier depuis son début (il peut ne pas encore exister).
    pub fn new(path: PathBuf) -> Self {
        Self { path, offset: 0 }
    }

    /// Lignes ajoutées depuis le dernier appel.
    pub fn poll(&mut self) -> Vec<String> {
        let Ok(meta) = std::fs::metadata(&self.path) else { return Vec::new() };
        if meta.len() <= self.offset {
            return Vec::new();
        }
        let Ok(mut file) = std::fs::File::open(&self.path) else { return Vec::new() };
        let mut bytes = Vec::new();
        if file.seek(SeekFrom::Start(self.offset)).is_err() || file.read_to_end(&mut bytes).is_err() {
            return Vec::new();
        }
        let Some(end) = bytes.iter().rposition(|&b| b == b'\n') else { return Vec::new() };
        self.offset += end as u64 + 1;
        String::from_utf8_lossy(&bytes[..end]).lines().map(|l| l.trim_end_matches('\r').to_string()).collect()
    }
}

/// Entrée à injecter avant la lecture des entrées de l'image.
#[derive(Clone, Debug)]
enum Input {
    KeyDown(KeyCode, Key),
    KeyUp(KeyCode, Key),
    Move(Vec2),
    Press,
    Release,
}

/// Ordre en cours : sa ligne, son décodage et les images écoulées.
struct Running {
    line: String,
    order: Order,
    frame: u32,
    /// Position du clic d'interface, une fois le bouton trouvé.
    target: Option<Vec2>,
}

/// État du mode joueur.
#[derive(Resource, Default)]
pub struct PlayerOrders {
    active: bool,
    out: PathBuf,
    current: Option<Running>,
    inputs: VecDeque<Input>,
    /// Numéro du dernier message du journal déjà donné.
    log_seen: Option<u64>,
    last_cursor: Vec2,
}

impl PlayerOrders {
    /// Mode joueur actif, réponses dans ce fichier.
    pub fn activate(out: PathBuf) -> Self {
        Self { active: true, out, ..default() }
    }

    /// Un ordre se joue encore : la ligne suivante attend.
    pub fn busy(&self) -> bool {
        self.current.is_some()
    }

    /// Commence un ordre.
    pub fn start(&mut self, line: &str, order: Order) {
        self.current = Some(Running { line: line.to_string(), order, frame: 0, target: None });
    }

    /// Écrit une réponse immédiate.
    pub fn answer(&self, line: &str, lines: &[String]) {
        if !lines.is_empty() {
            write_reply(&self.out, line, lines);
        }
    }
}

fn write_reply(out: &PathBuf, line: &str, lines: &[String]) {
    let mut text = if line.is_empty() || line.starts_with("repere") { String::new() } else { format!("> {line}\n") };
    for l in lines {
        text += l;
        text.push('\n');
    }
    info!("pilote> {line}\n{}", lines.join("\n"));
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(out) {
        let _ = f.write_all(text.as_bytes());
    }
}

/// Injecte les entrées de l'ordre en cours, avant que Bevy lise celles de l'image.
pub fn inject_inputs(
    mut orders: ResMut<PlayerOrders>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut keyboard: MessageWriter<KeyboardInput>,
    mut mouse: MessageWriter<MouseButtonInput>,
    mut events: MessageWriter<WindowEvent>,
) {
    let window = *window;
    while let Some(input) = orders.inputs.pop_front() {
        let button = |state| MouseButtonInput { button: MouseButton::Left, state, window };
        match input {
            Input::KeyDown(key_code, logical_key) => {
                let text = if let Key::Character(c) = &logical_key { Some(c.clone()) } else { None };
                keyboard.write(KeyboardInput { key_code, logical_key, state: ButtonState::Pressed, text, repeat: false, window });
            }
            Input::KeyUp(key_code, logical_key) => {
                keyboard.write(KeyboardInput { key_code, logical_key, state: ButtonState::Released, text: None, repeat: false, window });
            }
            Input::Move(position) => {
                events.write(WindowEvent::CursorMoved(CursorMoved { window, position, delta: None }));
            }
            Input::Press => {
                mouse.write(button(ButtonState::Pressed));
                events.write(WindowEvent::MouseButtonInput(button(ButtonState::Pressed)));
            }
            Input::Release => {
                mouse.write(button(ButtonState::Released));
                events.write(WindowEvent::MouseButtonInput(button(ButtonState::Released)));
            }
        }
    }
}

/// Ce que le joueur peut lire de la partie.
#[derive(SystemParam)]
pub struct PlayerView<'w, 's> {
    map: Res<'w, GameMap>,
    fog: Res<'w, Fog>,
    state: Res<'w, GameState>,
    selection: Res<'w, Selection>,
    log: Res<'w, GameLog>,
    mode: Res<'w, ViewMode>,
    layer: Res<'w, MapLayer>,
    hover_path: Res<'w, HoverPath>,
    hover_text: Single<'w, 's, &'static Text, With<HoverText>>,
    units: Query<'w, 's, (Entity, &'static Unit, &'static HexPos, Has<Hostile>)>,
    cities: Query<'w, 's, (Entity, &'static City, &'static HexPos)>,
    villages: Query<'w, 's, (&'static HexPos, Option<&'static SiteName>), With<Village>>,
    camps: Query<'w, 's, (&'static HexPos, Option<&'static SiteName>), With<Camp>>,
    nodes: Query<'w, 's, (Entity, &'static ComputedNode, &'static UiGlobalTransform, &'static InheritedVisibility), With<UiPointer>>,
    children: Query<'w, 's, &'static Children>,
    texts: Query<'w, 's, &'static Text>,
    window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
}

impl PlayerView<'_, '_> {
    fn place(&self, hex: Hex) -> String {
        coords(&self.map.0, hex)
    }

    fn unit_line(&self, u: &crate::rules::UnitState, at: Hex, hostile: bool) -> String {
        let name = token(u.kind.name());
        if hostile {
            return format!("ennemi {name} en {} pv={}/{MAX_HP}", self.place(at), u.hp);
        }
        let mut state = Vec::new();
        if u.needs_orders() {
            state.push("attend-ordres".to_string());
        }
        if u.sleeping {
            state.push("endormie".to_string());
        }
        if u.auto_explore {
            state.push("exploration-auto".to_string());
        }
        if u.immobilized {
            state.push("immobilisee".to_string());
        }
        if let Some(&end) = u.path.last() {
            state.push(format!("ordre-vers {}", self.place(end)));
        }
        format!("unite {name} en {} pv={}/{MAX_HP} pm={}/{} dette={} {}", self.place(at), u.hp, u.moves_left, u.kind.max_moves(), u.move_debt, state.join(" ")).trim_end().to_string()
    }

    fn city_line(&self, c: &City, at: Hex) -> String {
        let s = &c.state;
        format!(
            "ville {} en {} habitants={} nourriture={} production={} produit={}",
            s.name,
            self.place(at),
            s.population,
            s.food_stock,
            s.production_stock,
            s.producing.map_or("rien".to_string(), |k| token(k.name()))
        )
    }

    /// Sélection actuelle, en une ligne.
    fn selection_line(&self) -> String {
        let Some(e) = self.selection.0 else { return "selection=aucune".into() };
        if let Ok((_, u, p, hostile)) = self.units.get(e) {
            return format!("selection={}", self.unit_line(&u.0, p.0, hostile));
        }
        if let Ok((_, c, p)) = self.cities.get(e) {
            return format!("selection={}", self.city_line(c, p.0));
        }
        "selection=aucune".into()
    }

    /// Réponse à `etat`.
    fn state_lines(&self, replay: &TurnReplay) -> Vec<String> {
        let scale = self.window.scale_factor();
        let waiting = self.units.iter().filter(|(_, u, _, hostile)| !hostile && u.0.needs_orders()).count();
        let mut lines = vec![format!(
            "OK tour={} or={} attente={waiting} passage-tour={} vue={} carte={} explorees={} fenetre={}x{} echelle={scale}",
            self.state.turn,
            self.state.gold,
            if replay.playing { "oui" } else { "non" },
            if self.mode.is_3d { "3d" } else { "2d" },
            token(self.layer.short_name()),
            self.fog.explored.len(),
            self.window.resolution.width(),
            self.window.resolution.height(),
        )];
        lines.push(self.selection_line());
        let mut rest: Vec<(u8, String)> = Vec::new();
        for (_, u, p, hostile) in &self.units {
            if !hostile {
                rest.push((0, self.unit_line(&u.0, p.0, false)));
            } else if self.fog.visible.contains(&p.0) {
                rest.push((2, self.unit_line(&u.0, p.0, true)));
            }
        }
        for (_, c, p) in &self.cities {
            rest.push((1, self.city_line(c, p.0)));
        }
        let site = |kind: &str, p: &HexPos, name: Option<&SiteName>| format!("{kind} {} en {}", name.map_or("?", |n| n.0.as_str()), self.place(p.0));
        for (p, name) in &self.villages {
            if self.fog.explored.contains(&p.0) {
                rest.push((3, site("village", p, name)));
            }
        }
        for (p, name) in &self.camps {
            if self.fog.explored.contains(&p.0) {
                rest.push((4, site("camp", p, name)));
            }
        }
        rest.sort();
        lines.extend(rest.into_iter().map(|(_, l)| l));
        for e in self.log.entries() {
            lines.push(format!("message: {}", e.text.plain()));
        }
        lines
    }

    /// Réponse à `alentours` : cases explorées autour d'une case, avec ce qui s'y trouve.
    fn around_lines(&self, center: Option<Hex>, radius: i32) -> Vec<String> {
        let selected = self.selection.0.and_then(|e| self.units.get(e).map(|(_, _, p, _)| p.0).ok().or_else(|| self.cities.get(e).map(|(_, _, p)| p.0).ok()));
        let Some(center) = center.or(selected) else { return vec!["ERREUR rien de sélectionné : préciser col lig".into()] };
        let radius = radius.clamp(0, MAX_RADIUS);
        let m = &self.map.0;
        let (col, row) = center.to_offset();
        let mut lines = vec![format!("OK cases explorées à {radius} cases au plus de {}", self.place(center))];
        let mut unknown = 0;
        for r in row - radius..=row + radius {
            for c in col - radius - 1..=col + radius + 1 {
                let h = m.canonical(Hex::from_offset(c, r));
                if !m.contains(h) || m.distance(center, h) > radius {
                    continue;
                }
                if !self.fog.explored.contains(&h) {
                    unknown += 1;
                    continue;
                }
                let cost = m.move_cost(h).map_or("infranchissable".to_string(), |c| format!("{c} PM"));
                let mut line = format!("{} : {}, {cost}", self.place(h), m.describe(h));
                for (_, u, p, hostile) in &self.units {
                    if p.0 == h && (!hostile || self.fog.visible.contains(&h)) {
                        line += &format!(" [{} {}]", if hostile { "ennemi" } else { "unite" }, token(u.0.kind.name()));
                    }
                }
                if let Some((_, city, _)) = self.cities.iter().find(|(_, _, p)| p.0 == h) {
                    line += &format!(" [ville {}]", city.state.name);
                }
                if self.villages.iter().any(|(p, _)| p.0 == h) {
                    line += " [village]";
                }
                if self.camps.iter().any(|(p, _)| p.0 == h) {
                    line += " [camp]";
                }
                lines.push(line);
            }
        }
        lines.push(format!("inexplorees={unknown}"));
        lines
    }

    fn label_of(&self, entity: Entity) -> Option<String> {
        if let Ok(text) = self.texts.get(entity)
            && !text.0.trim().is_empty()
        {
            return Some(text.0.trim().replace('\n', " "));
        }
        self.children.get(entity).ok()?.iter().find_map(|child| self.label_of(child))
    }

    /// Boutons visibles (texte, centre en pixels de la fenêtre), de haut en bas.
    fn buttons(&self) -> Vec<(String, Vec2)> {
        let scale = self.window.scale_factor();
        let size = Vec2::new(self.window.resolution.width(), self.window.resolution.height());
        let mut found: Vec<(String, Vec2)> = self
            .nodes
            .iter()
            .filter(|(_, node, _, visible)| visible.get() && node.size().min_element() > 0.0)
            .map(|(e, _, at, _)| (self.label_of(e).unwrap_or_else(|| "(sans texte)".into()), at.translation / scale))
            .filter(|(_, p)| p.x >= 0.0 && p.y >= 0.0 && p.x < size.x && p.y < size.y)
            .collect();
        found.sort_by(|a, b| (a.1.y, a.1.x).partial_cmp(&(b.1.y, b.1.x)).unwrap_or(std::cmp::Ordering::Equal));
        found
    }

    /// Trouve un bouton par son numéro dans `interface` ou par son texte.
    fn find_button(&self, wanted: &str) -> Result<Vec2, String> {
        let buttons = self.buttons();
        if let Ok(n) = wanted.parse::<usize>() {
            return buttons.get(n).map(|b| b.1).ok_or(format!("pas de bouton numéro {n} ({} visibles)", buttons.len()));
        }
        let wanted = token(wanted);
        let matching: Vec<&(String, Vec2)> = buttons.iter().filter(|(label, _)| token(label).contains(&wanted)).collect();
        match matching[..] {
            [] => Err(format!("aucun bouton « {wanted} » (voir interface)")),
            [one] => Ok(one.1),
            _ => match matching.iter().find(|(label, _)| token(label) == wanted) {
                Some(exact) => Ok(exact.1),
                None => Err(format!("plusieurs boutons « {wanted} » : {}", matching.iter().map(|b| b.0.as_str()).collect::<Vec<_>>().join(" ; "))),
            },
        }
    }
}

/// Joue l'ordre en cours : entrées réparties sur quelques images (le temps que le survol et le
/// picking suivent), puis réponse une fois le passage de tour éventuel terminé.
pub fn run_orders(
    mut orders: ResMut<PlayerOrders>,
    mut pilot: ResMut<Pilot>,
    view: PlayerView,
    end_turn: Res<EndTurnRequested>,
    replay: Res<TurnReplay>,
    mut camera: Query<&mut Projection, (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
) {
    if !orders.active {
        return;
    }
    // La vraie souris n'agit pas sur la carte : la partie reste reproductible
    if !pilot.active {
        pilot.active = true;
    }
    let last_cursor = orders.last_cursor;
    let Some(run) = orders.current.as_mut() else { return };
    run.frame += 1;
    let frame = run.frame;
    let order = run.order.clone();
    let mut inputs: Vec<Input> = Vec::new();
    let mut done_after = 0;
    let mut failure = None;
    match &order {
        Order::State | Order::Around(..) | Order::Buttons | Order::Zoom(_) => {}
        Order::Hover(h) => {
            if frame == 1 {
                pilot.cursor = Some(*h);
                pilot.point = None;
            }
            done_after = 5;
        }
        Order::Click(h, button, side) => {
            match frame {
                1 => {
                    pilot.cursor = Some(*h);
                    // (le bord de la case de ce côté : une arête de rivière, la rive d'un fleuve)
                    pilot.point = side.map(|d| Vec2::from(h.to_world()) + Vec2::from_angle((d as f32 * 60.0).to_radians()) * crate::hex::HEX_SIZE * 0.75);
                }
                4 => pilot.click = Some(*button),
                _ => {}
            }
            done_after = 8;
        }
        Order::Key(code, key) => {
            match frame {
                1 => inputs.push(Input::KeyDown(*code, key.clone())),
                2 => inputs.push(Input::KeyUp(*code, key.clone())),
                _ => {}
            }
            done_after = 6;
        }
        Order::Button(_) | Order::ScreenClick(_) => {
            if frame == 1 {
                pilot.cursor = None;
                let target = match &order {
                    Order::Button(wanted) => view.find_button(wanted),
                    Order::ScreenClick(p) => Ok(*p),
                    _ => unreachable!(),
                };
                match target {
                    Ok(p) => run.target = Some(p),
                    Err(e) => failure = Some(e),
                }
            }
            if let Some(p) = run.target {
                match frame {
                    1 => inputs.push(Input::Move(p)),
                    4 => inputs.push(Input::Press),
                    6 => inputs.push(Input::Release),
                    8 => inputs.push(Input::Move(last_cursor)),
                    _ => {}
                }
            }
            done_after = 10;
        }
    }
    orders.inputs.extend(inputs);
    let settled = !end_turn.0 && !replay.playing;
    if failure.is_none() && (frame < done_after || (!settled && frame < MAX_FRAMES)) {
        return;
    }
    let line = run_line(&orders);
    let mut lines = match (&order, failure) {
        (_, Some(e)) => vec![format!("ERREUR {e}")],
        (Order::State, _) => view.state_lines(&replay),
        (Order::Around(center, radius), _) => view.around_lines(*center, *radius),
        (Order::Buttons, _) => {
            let mut lines = vec!["OK boutons visibles (numéro, texte, x y en pixels)".to_string()];
            lines.extend(view.buttons().iter().enumerate().map(|(i, (label, p))| format!("{i} : {label} ({:.0} {:.0})", p.x, p.y)));
            lines
        }
        (Order::Zoom(scale), _) => {
            if let Ok(mut projection) = camera.single_mut()
                && let Projection::Orthographic(ortho) = projection.as_mut()
            {
                ortho.scale = *scale;
            }
            vec![format!("OK zoom {scale}")]
        }
        (Order::Hover(h), _) => {
            let mut lines = vec![format!("OK survol de {}", view.place(*h)), format!("infobulle: {}", view.hover_text.0.replace('\n', " | "))];
            if let (Some(&end), Some(&turn)) = (view.hover_path.path.last(), view.hover_path.turns.last()) {
                lines.push(format!("trajet: {} cases jusqu'à {}, arrivée au tour {turn} de l'ordre", view.hover_path.path.len(), view.place(end)));
            }
            lines
        }
        (Order::Click(h, button, _), _) => vec![format!("OK clic {} sur {}", if *button == MouseButton::Left { "gauche" } else { "droit" }, view.place(*h)), view.selection_line()],
        (Order::Key(..), _) | (Order::Button(_), _) | (Order::ScreenClick(_), _) => vec![format!("OK tour={}", view.state.turn), view.selection_line()],
    };
    // Messages parus depuis la réponse précédente (`etat` donne déjà tout le journal)
    let seen = orders.log_seen;
    if order != Order::State {
        lines.extend(view.log.entries().iter().filter(|e| seen.is_none_or(|s| e.id > s)).map(|e| format!("message: {}", e.text.plain())));
    }
    orders.log_seen = view.log.entries().last().map(|e| e.id).or(seen);
    write_reply(&orders.out, &line, &lines);
    orders.current = None;
}

fn run_line(orders: &PlayerOrders) -> String {
    orders.current.as_ref().map_or(String::new(), |r| r.line.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touches_connues_et_inconnues() {
        assert_eq!(key_of("entree").map(|k| k.0), Some(KeyCode::Enter));
        assert_eq!(key_of("Entrée").map(|k| k.0), Some(KeyCode::Enter));
        assert_eq!(key_of("e").map(|k| k.0), Some(KeyCode::KeyE));
        assert_eq!(key_of("3").map(|k| k.0), Some(KeyCode::Digit3));
        assert_eq!(key_of("f1").map(|k| k.0), Some(KeyCode::F1));
        assert_eq!(key_of("f13"), None);
        assert_eq!(key_of("²"), None);
    }

    #[test]
    fn le_fichier_suivi_ne_rend_que_les_lignes_completes() {
        let path = std::env::temp_dir().join(format!("civbuilder_pilote_{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut live = LiveFile::new(path.clone());
        assert!(live.poll().is_empty(), "le fichier n'existe pas encore");
        std::fs::write(&path, "etat\nclic 3").unwrap();
        assert_eq!(live.poll(), vec!["etat".to_string()]);
        assert!(live.poll().is_empty(), "ligne incomplète gardée pour plus tard");
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b" 4\r\ntouche e\n").unwrap();
        drop(f);
        assert_eq!(live.poll(), vec!["clic 3 4".to_string(), "touche e".to_string()]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn la_console_du_joueur_ne_garde_ni_triche_ni_lecture_cachee() {
        for allowed in CONSOLE_COMMANDS {
            assert!(!matches!(allowed, "info" | "trouver" | "verifier" | "terrain" | "relief" | "unite" | "ville" | "chemin" | "regenerer"));
        }
    }
}
