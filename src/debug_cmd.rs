//! Console de debug : langage de commandes en texte, sans Bevy (testé ici).
//!
//! Le même langage sert à la console du jeu (touche ², en mode debug), aux scripts lancés avec
//! `CIVBUILDER_SCRIPT=<fichier>` (agents IA, cas à rejouer) et aux tests. Une commande est un mot
//! suivi d'arguments séparés par des espaces ; la réponse commence par `OK` ou `ERREUR`, suivie de
//! lignes `clé=valeur` faciles à relire par un programme.
//!
//! Les cases sont données en coordonnées de la carte : colonne (0 à gauche, ouest) puis ligne
//! (0 en haut, nord), comme dans l'infobulle en mode debug.
//!
//! Les modifications sont **brutes** : seul le champ demandé change, rien n'est recalculé (ni le
//! biome à partir du climat, ni les cours d'eau, ni le climat des voisines). Précédées de `~`,
//! elles sont **locales** : ce qui en découle directement est recalculé sur la case (température
//! selon l'altitude, biome selon le climat, altitude minimale d'une montagne ou d'un pic) et ses
//! voisines (relief selon la dénivelée, comme à la génération ; jamais deux pics côte à côte). `verifier` liste les incohérences de la carte. Le
//! rendu des cases voisines suit (relief 3D partagé, décor, eau), voir `console.rs`.
//!
//! Ce module décode, complète et explique les commandes, applique celles qui touchent la carte
//! et renvoie les autres (caméra, captures…) sous forme d'`Effect` à la console.

use crate::climate::LAPSE;
use crate::hex::Hex;
use crate::map::{BiomeClimate, MARSH_ARIDITY, MARSH_MAX_LEVEL, MOUNTAIN_MIN_M, PEAK_MIN_M, Relief, Terrain, WorldMap, biome};
use crate::render::MapLayer;
use crate::search::fold;

/// Nombre de modifications gardées pour `annuler`.
pub const UNDO_DEPTH: usize = 20;
/// Résultats affichés par défaut par `trouver`.
pub const FIND_DEFAULT_MAX: usize = 10;
/// Rayon maximal du pinceau (cases).
pub const BRUSH_MAX: i32 = 6;
/// Incohérences détaillées par `verifier` (les plus proches du centre de la vue).
pub const VERIFY_SHOWN: usize = 20;
/// Variation locale de l'aridité à la génération (sols, exposition : ±25 %, voir `generate_in`),
/// avec une marge : un biome est cohérent s'il est celui d'une aridité dans cet intervalle.
const ARIDITY_JITTER: [f32; 2] = [0.7, 1.3];
/// Écart d'altitude toléré (m) entre une case et sa case aval, ou entre deux cases d'un même lac,
/// et profondeur à partir de laquelle une cuvette retient l'eau.
const WATER_TOLERANCE_M: f32 = 1.0;
/// Cuvette sans lac signalée : traversée par plus de ce débit (m³/s ; une cuvette sèche, en climat
/// aride, ne reçoit presque rien) et plus profonde que cela (m ; la génération laisse un grand
/// fleuve traverser une cuvette peu profonde de terre aride, comme un désert sous le niveau de la
/// mer).
const POOL_MIN_FLOW: f32 = 1.0;
const POOL_MIN_DEPTH_M: f32 = 20.0;

// ---------------------------------------------------------------- Description des commandes

/// Rubrique de l'aide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Observe,
    Camera,
    Edit,
    Console,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Observe, Group::Edit, Group::Camera, Group::Console];

    pub fn title(self) -> &'static str {
        match self {
            Group::Observe => "Observer",
            Group::Camera => "Caméra et captures",
            Group::Edit => "Modifier la carte (brut ; « ~ » devant : recalcul local)",
            Group::Console => "Console",
        }
    }
}

/// Nature d'un argument : sert à décoder, à compléter (Tab) et à l'aide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arg {
    Col,
    Row,
    Terrain,
    Relief,
    Number,
    Layer,
    View,
    Command,
    Criteria,
    Text,
}

/// Une commande : nom, arguments (nom affiché, nature, facultatif), effet, rubrique.
pub struct Spec {
    pub name: &'static str,
    pub args: &'static [(&'static str, Arg, bool)],
    pub help: &'static str,
    pub group: Group,
}

impl Spec {
    /// Forme d'appel : « terrain <col> <lig> <terrain> », facultatifs entre crochets.
    pub fn usage(&self) -> String {
        let mut s = self.name.to_string();
        for &(name, arg, optional) in self.args {
            let shown = if arg == Arg::Criteria { format!("{name}…") } else { name.to_string() };
            s += &if optional { format!(" [{shown}]") } else { format!(" <{shown}>") };
        }
        s
    }
}

const AT: [(&str, Arg, bool); 2] = [("col", Arg::Col, false), ("lig", Arg::Row, false)];
const AT_OPT: [(&str, Arg, bool); 2] = [("col", Arg::Col, true), ("lig", Arg::Row, true)];

/// Toutes les commandes, dans l'ordre de l'aide.
pub const SPECS: &[Spec] = &[
    Spec { name: "info", args: &AT_OPT, help: "Toutes les données d'une case (sans coordonnées : la case sous le curseur) : terrain, relief, altitude, climat, eau, coût d'un pas vers chaque voisine.", group: Group::Observe },
    Spec { name: "verifier", args: &[("col", Arg::Col, true), ("lig", Arg::Row, true), ("rayon", Arg::Number, true)], help: "Incohérences de la carte (toute la carte, ou autour d'une case) : pics côte à côte, montagne ou pic trop bas, relief sur un fleuve, biome que le climat ne donne pas, rivière qui remonte, lac à deux niveaux.", group: Group::Observe },
    Spec { name: "trouver", args: &[("critère", Arg::Criteria, false)], help: "Cases qui remplissent tous les critères, les plus proches d'abord. Critères : terrain=, relief=, voisin= (terrain ou relief d'une voisine), fleuve=oui|non, eau=oui|non, alt<, alt>, temp<, temp>, pluie<, pluie>, pres=col,lig (sinon le centre de la vue), max=n.", group: Group::Observe },
    Spec { name: "terrain", args: &[AT[0], AT[1], ("terrain", Arg::Terrain, false)], help: "Change le terrain (une case d'eau devient plate, la mer descend au niveau 0). Avec ~, le terrain imposé reste, même s'il ne suit pas le climat.", group: Group::Edit },
    Spec { name: "relief", args: &[AT[0], AT[1], ("relief", Arg::Relief, false)], help: "Change le relief d'une case de terre (un fleuve reste plat). Avec ~ : une montagne monte au moins à 1 500 m, un pic à 2 400 m (température et biome suivent), les pics voisins n'en sont plus ; le relief des voisines suit leur nouvelle dénivelée.", group: Group::Edit },
    Spec { name: "altitude", args: &[AT[0], AT[1], ("mètres", Arg::Number, false)], help: "Change l'altitude du sol d'une case de terre, et sa classe d'altitude (falaises). Avec ~ : la température suit (-6,5 °C par 1 000 m), puis le biome ; le relief de la case et de ses voisines est recalculé comme à la génération (dénivelée avec les voisines, altitude), pics compris.", group: Group::Edit },
    Spec { name: "temperature", args: &[AT[0], AT[1], ("°C", Arg::Number, false)], help: "Change la température moyenne annuelle et celle des saisons (limite des neiges en 3D). Avec ~, le biome suit.", group: Group::Edit },
    Spec { name: "pluie", args: &[AT[0], AT[1], ("mm", Arg::Number, false)], help: "Change les précipitations annuelles et celles des saisons (carte du climat). Avec ~, le biome suit ; les cours d'eau, jamais.", group: Group::Edit },
    Spec { name: "pinceau", args: &[("rayon", Arg::Number, false)], help: "Les modifications suivantes s'appliquent aussi aux cases à moins de « rayon » cases (0 : la case seule).", group: Group::Edit },
    Spec { name: "annuler", args: &[], help: "Annule la dernière modification de la carte.", group: Group::Edit },
    Spec { name: "aller", args: &AT, help: "Centre la vue sur la case.", group: Group::Camera },
    Spec { name: "vue", args: &[("2d|3d", Arg::View, false)], help: "Passe en vue 2D ou 3D.", group: Group::Camera },
    Spec { name: "camera", args: &[("distance", Arg::Number, false), ("angle", Arg::Number, true)], help: "Vue 3D : distance de la caméra (250 à 2 800 ; 300 : une case de près) et orientation en degrés (0 : vers le nord).", group: Group::Camera },
    Spec { name: "carte", args: &[("carte", Arg::Layer, false)], help: "Carte affichée (normale, altitude, relief, climat…).", group: Group::Camera },
    Spec { name: "capture", args: &[("fichier", Arg::Text, false)], help: "Enregistre l'écran en PNG (sans la console). Chemin relatif : dossier du script, sinon dossier temporaire « civbuilder ».", group: Group::Camera },
    Spec { name: "attendre", args: &[("images", Arg::Number, false)], help: "Attend un nombre d'images avant la commande suivante (relief 3D à construire).", group: Group::Camera },
    Spec { name: "aide", args: &[("commande", Arg::Command, true)], help: "Liste des commandes, ou détail d'une commande (valeurs possibles).", group: Group::Console },
    Spec { name: "journal", args: &[("fichier", Arg::Text, true)], help: "Commandes réussies de la séance ; avec un fichier, les y écrit en script à rejouer (graine en tête).", group: Group::Console },
    Spec { name: "effacer", args: &[], help: "Vide l'affichage de la console.", group: Group::Console },
    Spec { name: "quitter", args: &[], help: "Ferme le jeu (fin d'un script).", group: Group::Console },
];

/// Commande décrite par son nom.
pub fn spec(name: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.name == name)
}

// ---------------------------------------------------------------- Noms des valeurs

/// Nom d'une valeur en un mot, sans accent ni majuscule : « Forêt tempérée » → `foret-temperee`.
pub fn token(name: &str) -> String {
    let s: String = name.chars().map(fold).collect();
    s.split_whitespace().collect::<Vec<_>>().join("-")
}

fn terrain_tokens() -> Vec<(String, Terrain)> {
    Terrain::ALL.iter().map(|&t| (token(t.name()), t)).collect()
}

fn relief_tokens() -> Vec<(String, Relief)> {
    Relief::ALL.iter().map(|&r| (token(r.name()), r)).collect()
}

fn layer_tokens() -> Vec<(String, MapLayer)> {
    MapLayer::ALL.iter().map(|&l| (token(l.short_name()), l)).collect()
}

/// Critères de `trouver` (avec leur opérateur) : complétion et aide.
const CRITERIA: [&str; 13] = ["terrain=", "relief=", "voisin=", "fleuve=", "eau=", "alt<", "alt>", "temp<", "temp>", "pluie<", "pluie>", "pres=", "max="];

/// Valeur choisie par son nom : nom exact, sinon début de nom s'il est le seul à commencer ainsi.
fn pick<T: Copy>(word: &str, choices: &[(String, T)], what: &str) -> Result<T, String> {
    let w = token(word);
    if let Some((_, v)) = choices.iter().find(|(n, _)| *n == w) {
        return Ok(*v);
    }
    let starts: Vec<&(String, T)> = choices.iter().filter(|(n, _)| n.starts_with(&w)).collect();
    match starts.as_slice() {
        [(_, v)] => Ok(*v),
        [] => Err(format!("{what} inconnu : « {word} » (possibles : {})", choices.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "))),
        many => Err(format!("{what} ambigu : « {word} » ({})", many.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "))),
    }
}

fn number(word: &str, what: &str) -> Result<f32, String> {
    word.replace(',', ".").parse::<f32>().ok().filter(|v| v.is_finite()).ok_or_else(|| format!("{what} : nombre attendu, pas « {word} »"))
}

/// Case donnée par sa colonne et sa ligne (la colonne fait le tour de la planète).
fn cell(map: &WorldMap, col: &str, row: &str) -> Result<Hex, String> {
    let (Ok(c), Ok(r)) = (col.parse::<i32>(), row.parse::<i32>()) else {
        return Err(format!("coordonnées entières attendues, pas « {col} {row} »"));
    };
    let c = if map.wrap().world_width() > 0.0 { c.rem_euclid(map.width) } else { c };
    if !(0..map.width).contains(&c) || !(0..map.height).contains(&r) {
        return Err(format!("case {col} {row} hors de la carte ({} colonnes × {} lignes)", map.width, map.height));
    }
    Ok(Hex::from_offset(c, r))
}

/// Colonne et ligne d'une case, telles qu'on les tape.
pub fn coords(map: &WorldMap, hex: Hex) -> String {
    let (c, r) = map.canonical(hex).to_offset();
    format!("{c} {r}")
}

// ---------------------------------------------------------------- Commandes décodées

/// Champ modifié d'une case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    Terrain(Terrain),
    Relief(Relief),
    Altitude(f32),
    Temperature(f32),
    Rain(f32),
}

/// Critère de recherche.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Criterion {
    Terrain(Terrain),
    Relief(Relief),
    NeighborTerrain(Terrain),
    NeighborRelief(Relief),
    Fleuve(bool),
    Water(bool),
    /// Altitude, température, pluie : borne et sens (`true` : au-dessus).
    Altitude(f32, bool),
    Temperature(f32, bool),
    Rain(f32, bool),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Info(Option<Hex>),
    /// Zone vérifiée (case et rayon), sinon toute la carte.
    Verify(Option<(Hex, i32)>),
    Find { criteria: Vec<Criterion>, near: Option<Hex>, max: usize },
    /// Case, champ, recalcul local (`~`).
    Edit(Hex, Field, bool),
    Brush(i32),
    Undo,
    Goto(Hex),
    View(bool),
    Camera { distance: f32, yaw_deg: Option<f32> },
    Layer(MapLayer),
    Capture(String),
    Wait(u32),
    Help(Option<String>),
    Journal(Option<String>),
    Clear,
    Quit,
}

fn yes_no(word: &str) -> Result<bool, String> {
    match token(word).as_str() {
        "oui" | "o" | "1" | "vrai" => Ok(true),
        "non" | "n" | "0" | "faux" => Ok(false),
        _ => Err(format!("oui ou non attendu, pas « {word} »")),
    }
}

fn criterion(map: &WorldMap, word: &str) -> Result<(Option<Criterion>, Option<Hex>, Option<usize>), String> {
    let Some(at) = word.find(['=', '<', '>']) else {
        return Err(format!("critère sans opérateur : « {word} » (exemple : relief=montagne, alt>2000)"));
    };
    let (key, op, value) = (token(&word[..at]), &word[at..=at], &word[at + 1..]);
    let above = op == ">";
    let c = match (key.as_str(), op) {
        ("terrain", "=") => Criterion::Terrain(pick(value, &terrain_tokens(), "terrain")?),
        ("relief", "=") => Criterion::Relief(pick(value, &relief_tokens(), "relief")?),
        ("voisin", "=") => match pick(value, &relief_tokens(), "relief") {
            Ok(r) => Criterion::NeighborRelief(r),
            Err(_) => Criterion::NeighborTerrain(pick(value, &terrain_tokens(), "terrain ou relief")?),
        },
        ("fleuve", "=") => Criterion::Fleuve(yes_no(value)?),
        ("eau", "=") => Criterion::Water(yes_no(value)?),
        ("alt" | "altitude", "<" | ">") => Criterion::Altitude(number(value, "altitude")?, above),
        ("temp" | "temperature", "<" | ">") => Criterion::Temperature(number(value, "température")?, above),
        ("pluie", "<" | ">") => Criterion::Rain(number(value, "pluie")?, above),
        ("pres", "=") => {
            let (c, r) = value.split_once([',', ';']).ok_or_else(|| format!("pres=col,lig attendu, pas « {word} »"))?;
            return Ok((None, Some(cell(map, c, r)?), None));
        }
        ("max", "=") => {
            let n = value.parse::<usize>().map_err(|_| format!("max : entier attendu, pas « {value} »"))?;
            return Ok((None, None, Some(n.max(1))));
        }
        _ => return Err(format!("critère inconnu : « {word} » (critères : {})", CRITERIA.join(" "))),
    };
    Ok((Some(c), None, None))
}

/// Ligne sans le `~` d'une modification locale, et ce `~`.
fn split_local(line: &str) -> (&str, bool) {
    let t = line.trim_start();
    match t.strip_prefix('~') {
        Some(rest) => (rest, true),
        None => (line, false),
    }
}

/// Décode une ligne de commande.
pub fn parse(map: &WorldMap, line: &str) -> Result<Command, String> {
    let (line, local) = split_local(line);
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some((&first, args)) = words.split_first() else { return Err("commande vide".into()) };
    let name = token(first);
    let Some(spec) = spec(&name) else {
        let close: Vec<&str> = SPECS.iter().map(|s| s.name).filter(|n| n.starts_with(&name)).collect();
        return Err(if close.is_empty() { format!("commande inconnue : « {first} » (aide : liste des commandes)") } else { format!("commande inconnue : « {first} » (voulez-vous dire : {} ?)", close.join(", ")) });
    };
    let required = spec.args.iter().filter(|a| !a.2).count();
    let free = spec.args.iter().any(|a| a.1 == Arg::Criteria);
    if args.len() < required || (!free && args.len() > spec.args.len()) {
        return Err(format!("usage : {}", spec.usage()));
    }
    if local && spec.group != Group::Edit || local && matches!(spec.name, "pinceau" | "annuler") {
        return Err(format!("« ~ » ne s'applique qu'aux modifications d'une case, pas à « {} »", spec.name));
    }
    let at = || cell(map, args[0], args[1]);
    Ok(match spec.name {
        "info" => match args {
            [] => Command::Info(None),
            [c, r] => Command::Info(Some(cell(map, c, r)?)),
            _ => return Err(format!("usage : {}", spec.usage())),
        },
        "verifier" => match args {
            [] => Command::Verify(None),
            [c, r] => Command::Verify(Some((cell(map, c, r)?, 3))),
            [c, r, radius] => Command::Verify(Some((cell(map, c, r)?, number(radius, "rayon")?.round().clamp(0.0, 50.0) as i32))),
            _ => return Err(format!("usage : {}", spec.usage())),
        },
        "trouver" => {
            let (mut criteria, mut near, mut max) = (Vec::new(), None, FIND_DEFAULT_MAX);
            for w in args {
                let (c, n, m) = criterion(map, w)?;
                criteria.extend(c);
                near = n.or(near);
                max = m.unwrap_or(max);
            }
            Command::Find { criteria, near, max }
        }
        "terrain" => Command::Edit(at()?, Field::Terrain(pick(args[2], &terrain_tokens(), "terrain")?), local),
        "relief" => Command::Edit(at()?, Field::Relief(pick(args[2], &relief_tokens(), "relief")?), local),
        "altitude" => Command::Edit(at()?, Field::Altitude(number(args[2], "altitude")?), local),
        "temperature" => Command::Edit(at()?, Field::Temperature(number(args[2], "température")?), local),
        "pluie" => Command::Edit(at()?, Field::Rain(number(args[2], "pluie")?.max(0.0)), local),
        "pinceau" => Command::Brush((number(args[0], "rayon")?.round() as i32).clamp(0, BRUSH_MAX)),
        "annuler" => Command::Undo,
        "aller" => Command::Goto(at()?),
        "vue" => match token(args[0]).as_str() {
            "2d" => Command::View(false),
            "3d" => Command::View(true),
            _ => return Err(format!("vue : 2d ou 3d, pas « {} »", args[0])),
        },
        "camera" => Command::Camera {
            distance: number(args[0], "distance")?.clamp(250.0, 2_800.0),
            yaw_deg: args.get(1).map(|a| number(a, "angle")).transpose()?,
        },
        "carte" => Command::Layer(pick(&args.join(" "), &layer_tokens(), "carte")?),
        "capture" => Command::Capture(args[0].to_string()),
        "attendre" => Command::Wait(number(args[0], "images")?.clamp(0.0, 10_000.0) as u32),
        "aide" => Command::Help(args.first().map(|a| token(a))),
        "journal" => Command::Journal(args.first().map(|a| a.to_string())),
        "effacer" => Command::Clear,
        "quitter" => Command::Quit,
        _ => unreachable!("commande décrite sans décodage : {}", spec.name),
    })
}

// ---------------------------------------------------------------- Complétion et aide

/// Valeurs proposées pour un argument.
fn values(arg: Arg) -> Vec<String> {
    match arg {
        Arg::Terrain => terrain_tokens().into_iter().map(|(n, _)| n).collect(),
        Arg::Relief => relief_tokens().into_iter().map(|(n, _)| n).collect(),
        Arg::Layer => layer_tokens().into_iter().map(|(n, _)| n).collect(),
        Arg::View => vec!["2d".into(), "3d".into()],
        Arg::Command => SPECS.iter().map(|s| s.name.to_string()).collect(),
        Arg::Criteria => CRITERIA.iter().map(|s| s.to_string()).collect(),
        _ => Vec::new(),
    }
}

/// Résultat de la touche Tab : ligne complétée (partie commune des propositions) et propositions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Completion {
    pub line: String,
    pub candidates: Vec<String>,
}

fn common_prefix(words: &[String]) -> String {
    let Some(first) = words.first() else { return String::new() };
    let mut end = first.len();
    for w in &words[1..] {
        end = first.char_indices().zip(w.chars()).take_while(|((_, a), b)| a == b).last().map_or(0, |((i, a), _)| (i + a.len_utf8()).min(end));
    }
    first[..end].to_string()
}

/// Complète le dernier mot de la ligne : nom de commande, puis valeurs possibles de l'argument.
pub fn complete(line: &str) -> Completion {
    let (rest, local) = split_local(line);
    if local {
        let c = complete(rest);
        return Completion { line: format!("~{}", c.line), candidates: c.candidates };
    }
    let ends_with_space = line.is_empty() || line.ends_with(' ');
    let mut words: Vec<&str> = line.split_whitespace().collect();
    let current = if ends_with_space { "" } else { words.pop().unwrap_or("") };
    let (choices, prefix_kept): (Vec<String>, &str) = if words.is_empty() {
        (SPECS.iter().map(|s| s.name.to_string()).collect(), "")
    } else {
        let Some(spec) = spec(&token(words[0])) else { return Completion { line: line.to_string(), candidates: Vec::new() } };
        let index = words.len() - 1;
        let arg = spec.args.get(index).or(spec.args.last().filter(|a| a.1 == Arg::Criteria)).map(|a| a.1);
        match arg {
            // Critère : la clé, puis la valeur après l'opérateur
            Some(Arg::Criteria) => match current.find(['=', '<', '>']) {
                Some(at) => {
                    let key = token(&current[..at]);
                    let vals = match key.as_str() {
                        "terrain" => values(Arg::Terrain),
                        "relief" => values(Arg::Relief),
                        "voisin" => values(Arg::Relief).into_iter().chain(values(Arg::Terrain)).collect(),
                        "fleuve" | "eau" => vec!["oui".into(), "non".into()],
                        _ => Vec::new(),
                    };
                    (vals, &current[..=at])
                }
                None => (values(Arg::Criteria), ""),
            },
            Some(a) => (values(a), ""),
            None => (Vec::new(), ""),
        }
    };
    let typed = token(&current[prefix_kept.len()..]);
    let candidates: Vec<String> = choices.into_iter().filter(|c| c.starts_with(&typed)).collect();
    let head = if words.is_empty() { String::new() } else { format!("{} ", words.join(" ")) };
    let line = match candidates.as_slice() {
        [] => line.to_string(),
        // Une seule possibilité : mot complet, puis un espace (sauf après une clé de critère)
        [one] => {
            let space = if one.ends_with(['=', '<', '>']) { "" } else { " " };
            format!("{head}{prefix_kept}{one}{space}")
        }
        many => {
            let common = common_prefix(many);
            if common.len() > typed.len() { format!("{head}{prefix_kept}{common}") } else { line.to_string() }
        }
    };
    let candidates = if candidates.len() > 1 { candidates.into_iter().map(|c| format!("{prefix_kept}{c}")).collect() } else { Vec::new() };
    Completion { line, candidates }
}

/// Aide en une ligne pour la saisie en cours : forme d'appel et effet de la commande tapée.
pub fn hint(line: &str) -> String {
    let (line, _) = split_local(line);
    let Some(first) = line.split_whitespace().next() else { return "Tab : compléter · ↑ ↓ : historique · aide : liste des commandes".into() };
    match spec(&token(first)) {
        Some(s) => format!("{} — {}", s.usage(), s.help),
        None => {
            let close: Vec<&str> = SPECS.iter().map(|s| s.name).filter(|n| n.starts_with(&token(first))).collect();
            if close.is_empty() { "commande inconnue (aide : liste des commandes)".into() } else { close.join(" · ") }
        }
    }
}

/// Texte de `aide` (liste) ou de `aide <commande>` (détail et valeurs possibles).
pub fn help(about: Option<&str>) -> Result<Vec<String>, String> {
    let Some(name) = about else {
        let mut lines = vec![
            "OK aide — Tab complète, « aide <commande> » détaille ; cases : colonne puis ligne (infobulle)".to_string(),
            "~ devant une modification : recalcul local (température, biome, pics) ; sans : brut".to_string(),
        ];
        for g in Group::ALL {
            lines.push(format!("{} : {}", g.title(), SPECS.iter().filter(|s| s.group == g).map(|s| s.name).collect::<Vec<_>>().join(", ")));
        }
        return Ok(lines);
    };
    let s = spec(name).ok_or_else(|| format!("commande inconnue : « {name} »"))?;
    let mut lines = vec![format!("OK {}", s.usage()), s.help.to_string()];
    for &(label, arg, _) in s.args {
        let v = values(arg);
        if !v.is_empty() && arg != Arg::Command {
            lines.push(format!("{label} : {}", v.join(", ")));
        }
    }
    if s.name == "trouver" {
        lines.push(format!("terrain= / voisin= : {}", values(Arg::Terrain).join(", ")));
        lines.push(format!("relief= / voisin= : {}", values(Arg::Relief).join(", ")));
        lines.push("exemple : trouver relief=montagne terrain=foret voisin=pic max=5".into());
    }
    Ok(lines)
}

// ---------------------------------------------------------------- Lecture de la carte

/// Toutes les données d'une case, en lignes `clé=valeur`.
pub fn info(map: &WorldMap, hex: Hex) -> Vec<String> {
    let hex = map.canonical(hex);
    let Some(t) = map.get(hex) else { return vec![format!("ERREUR case {} hors de la carte", coords(map, hex))] };
    let i = map.cell(hex);
    let opt = |v: Option<f32>, unit: &str| v.map_or("—".to_string(), |v| format!("{v:.1}{unit}"));
    let mut lines = vec![
        format!("OK case {} — {}", coords(map, hex), map.describe(hex)),
        format!(
            "terrain={} relief={} altitude={:.0}m niveau={} temperature={} pluie={} aridite={}",
            token(t.name()),
            token(map.relief(hex).name()),
            map.elevation(hex).unwrap_or(0.0),
            map.level(hex).unwrap_or(0),
            opt(map.temperature(hex), "°C"),
            opt(map.rain(hex), "mm"),
            map.aridity(hex).map_or("—".to_string(), |a| format!("{a:.2}")),
        ),
    ];
    let edges: Vec<String> = (0..6).filter(|&d| map.river_flow(hex, d) > 0.0).map(|d| format!("{d}:{:.0}", map.river_flow(hex, d))).collect();
    let mut water = format!(
        "fleuve={} petit-lac={} eau-douce={} debit={:.0}m3/s aval={} rivieres-aretes=[{}]",
        if map.is_fleuve(hex) { "oui" } else { "non" },
        if map.small_lake(hex) { "oui" } else { "non" },
        if map.fresh_water(hex) { "oui" } else { "non" },
        map.discharge(hex),
        map.downstream(hex).map_or("—".to_string(), |d| d.to_string()),
        edges.join(" "),
    );
    if t.is_water() {
        water += &format!(" surface={:.0}m profondeur={}", map.surface_elevation(hex).unwrap_or(0.0), opt(map.depth(hex), "m"));
    }
    lines.push(water);
    if let Some(name) = map.named_river(hex).map(|r| r.name.clone()) {
        lines.push(format!("cours-d-eau={name}"));
    }
    let costs = map.step_costs()[i];
    let enter = map.enter_cost(hex).map_or("infranchissable".to_string(), |c| format!("{} PM", c.total()));
    lines.push(format!("entree={enter} ; pas vers les voisines (direction:PM, x = infranchissable) :"));
    for (d, &cost) in costs.iter().enumerate() {
        let n = map.canonical(map.wrap().neighbor(hex, d));
        let what = match map.get(n) {
            Some(nt) => format!("{} {} {:.0}m", token(nt.name()), token(map.relief(n).name()), map.elevation(n).unwrap_or(0.0)),
            None => "hors carte".into(),
        };
        let cost = if cost == u8::MAX { "x".to_string() } else { cost.to_string() };
        let cliff = if map.cliff_between(hex, n) { " falaise" } else { "" };
        lines.push(format!("  {d}:{cost} → {} {what}{cliff}", coords(map, n)));
    }
    lines
}

fn matches(map: &WorldMap, hex: Hex, c: &Criterion) -> bool {
    let cmp = |v: Option<f32>, bound: f32, above: bool| v.is_some_and(|v| if above { v > bound } else { v < bound });
    let neighbors = || map.neighbors(hex).into_iter().map(|n| map.canonical(n)).filter(|&n| map.contains(n));
    match *c {
        Criterion::Terrain(t) => map.get(hex) == Some(t),
        Criterion::Relief(r) => map.relief(hex) == r && map.get(hex).is_some_and(|t| !t.is_water()),
        Criterion::NeighborTerrain(t) => neighbors().any(|n| map.get(n) == Some(t)),
        Criterion::NeighborRelief(r) => neighbors().any(|n| map.relief(n) == r && map.get(n).is_some_and(|t| !t.is_water())),
        Criterion::Fleuve(yes) => map.is_fleuve(hex) == yes,
        Criterion::Water(yes) => map.get(hex).is_some_and(|t| t.is_water()) == yes,
        Criterion::Altitude(b, above) => cmp(map.elevation(hex), b, above),
        Criterion::Temperature(b, above) => cmp(map.temperature(hex), b, above),
        Criterion::Rain(b, above) => cmp(map.rain(hex), b, above),
    }
}

/// Cases qui remplissent tous les critères, les plus proches de `near` d'abord (au plus `max`),
/// et leur nombre total.
pub fn find(map: &WorldMap, criteria: &[Criterion], near: Hex, max: usize) -> (Vec<Hex>, usize) {
    let mut found: Vec<Hex> = map.hexes().filter(|&h| criteria.iter().all(|c| matches(map, h, c))).collect();
    let total = found.len();
    found.sort_by_key(|&h| (map.distance(h, near), h.to_offset()));
    found.truncate(max);
    (found, total)
}

// ---------------------------------------------------------------- Modifications

/// Applique un champ à une case ; `Err` si le champ ne s'applique pas à cette case.
fn set_field(map: &mut WorldMap, hex: Hex, field: Field) -> Result<(), &'static str> {
    let water = map.get(hex).is_some_and(|t| t.is_water());
    match field {
        Field::Terrain(t) => {
            let was = map.get(hex);
            map.set(hex, t);
            // Un nouveau lac prend la surface de son sol (pas celle d'un ancien lac ailleurs)
            if t == Terrain::Lake && was != Some(Terrain::Lake) {
                let ground = map.elevation(hex).unwrap_or(0.0);
                map.set_lake_level(hex, ground);
            }
        }
        Field::Relief(_) | Field::Altitude(_) if water => return Err("case d'eau"),
        Field::Relief(r) => map.set_relief(hex, r),
        Field::Altitude(m) => map.set_elevation(hex, m.max(0.0)),
        Field::Temperature(c) => map.set_temperature(hex, c),
        Field::Rain(mm) => map.set_rain(hex, mm),
    }
    Ok(())
}

fn field_text(field: Field) -> String {
    match field {
        Field::Terrain(t) => format!("terrain={}", token(t.name())),
        Field::Relief(r) => format!("relief={}", token(r.name())),
        Field::Altitude(m) => format!("altitude={m:.0}m"),
        Field::Temperature(c) => format!("temperature={c:.1}°C"),
        Field::Rain(mm) => format!("pluie={mm:.0}mm"),
    }
}

// ---------------------------------------------------------------- Cohérence

/// Climat d'une case pour le choix du biome (`None` : eau, ou carte sans climat simulé).
fn biome_climate(map: &WorldMap, hex: Hex) -> Option<BiomeClimate> {
    if map.get(hex)?.is_water() {
        return None;
    }
    let i = map.cell(hex);
    let c = map.climate();
    let temperature = *c.temperature.get(i)?;
    let (coldest, _) = c.coldest_warmest(i)?;
    Some(BiomeClimate {
        temperature,
        aridity: c.aridity(i)?,
        coldest,
        dry_seasons: c.dry_seasons(i)?.iter().filter(|&&d| d).count() as u8,
        dry_summer: c.dry_summer(i)?,
    })
}

/// Le terrain d'une case de terre est celui que donne son climat (à la variation locale de
/// l'aridité près) ; un marais demande un sol humide qui ne gèle pas, bas et plat.
fn biome_fits(map: &WorldMap, hex: Hex, t: Terrain) -> bool {
    let Some(c) = biome_climate(map, hex) else { return true };
    if t == Terrain::Marsh {
        return c.aridity >= MARSH_ARIDITY * ARIDITY_JITTER[0] && c.temperature > 0.0 && map.relief(hex) == Relief::Flat && map.level(hex).is_some_and(|l| l <= MARSH_MAX_LEVEL);
    }
    let steps = 12;
    (0..=steps).any(|k| {
        let f = ARIDITY_JITTER[0] + (ARIDITY_JITTER[1] - ARIDITY_JITTER[0]) * k as f32 / steps as f32;
        biome(BiomeClimate { aridity: c.aridity * f, ..c }) == t
    })
}

/// Recalcul local après la modification d'une case (`before` : son altitude d'avant) : cases
/// changées en plus (voisines) et ce qui a changé, en clair.
fn settle(map: &mut WorldMap, hex: Hex, field: Field, before: f32) -> (Vec<Hex>, Vec<String>) {
    let mut changed = Vec::new();
    let mut notes = Vec::new();
    let at = coords(map, hex);
    let land = map.get(hex).is_some_and(|t| !t.is_water());
    // Altitude imposée par le relief, puis température selon l'altitude
    let mut ground = map.elevation(hex).unwrap_or(0.0);
    if let Field::Relief(r) = field
        && land
    {
        let floor = match map.relief(hex) {
            Relief::Peak => PEAK_MIN_M,
            Relief::Mountain => MOUNTAIN_MIN_M,
            _ => 0.0,
        };
        if ground < floor {
            map.set_elevation(hex, floor);
            notes.push(format!("{at} : altitude {ground:.0} → {floor:.0} m ({} demande au moins {floor:.0} m)", token(r.name())));
            ground = floor;
        }
        // Jamais deux pics côte à côte
        if map.relief(hex) == Relief::Peak {
            let around: Vec<Hex> = map.neighbors(hex).into_iter().map(|n| map.canonical(n)).filter(|&n| map.contains(n)).collect();
            for n in around {
                if map.relief(n) == Relief::Peak {
                    map.set_relief(n, Relief::Mountain);
                    changed.push(n);
                    notes.push(format!("{} : pic voisin → montagne", coords(map, n)));
                }
            }
        }
    }
    // (carte sans règle du relief : seulement les altitudes minimales ; sinon voir `reshape`)
    if let Field::Altitude(_) = field
        && land
        && map.natural_relief(hex).is_none()
    {
        let demoted = match map.relief(hex) {
            Relief::Peak if ground < PEAK_MIN_M => Some(if ground < MOUNTAIN_MIN_M { Relief::Rugged } else { Relief::Mountain }),
            Relief::Mountain if ground < MOUNTAIN_MIN_M => Some(Relief::Rugged),
            _ => None,
        };
        if let Some(r) = demoted {
            notes.push(format!("{at} : relief {} → {} (trop bas)", token(map.relief(hex).name()), token(r.name())));
            map.set_relief(hex, r);
        }
    }
    if land
        && (ground - before).abs() > 0.5
        && let Some(t) = map.temperature(hex)
    {
        let t2 = t - LAPSE * (ground - before);
        map.set_temperature(hex, t2);
        notes.push(format!("{at} : température {t:.1} → {t2:.1} °C (altitude {before:.0} → {ground:.0} m)"));
    }
    // Biome selon le climat (un terrain imposé reste)
    if land
        && !matches!(field, Field::Terrain(_))
        && let Some(t) = map.get(hex)
        && !biome_fits(map, hex, t)
        && let Some(c) = biome_climate(map, hex)
    {
        let new = biome(c);
        map.set(hex, new);
        notes.push(format!("{at} : biome {} → {}", token(t.name()), token(new.name())));
    }
    if let Field::Terrain(t) = field
        && !t.is_water()
        && !biome_fits(map, hex, t)
    {
        notes.push(format!("{at} : {} imposé, le climat donnerait {}", token(t.name()), biome_climate(map, hex).map_or("—".into(), |c| token(biome(c).name()))));
    }
    (changed, notes)
}

/// Relief recalculé comme à la génération autour des cases dont l'altitude (ou la terre) a changé
/// (`moved`) : la dénivelée avec les voisines change aussi la leur. Les cases au relief imposé
/// (`fixed`) le gardent. Puis les pics : un pic qui n'est plus un sommet assez haut redevient
/// montagne, une montagne qui l'est devient pic (les plus hautes d'abord, jamais côte à côte).
fn reshape(map: &mut WorldMap, moved: &[Hex], fixed: &[Hex]) -> (Vec<Hex>, Vec<String>) {
    let mut changed = Vec::new();
    let mut notes = Vec::new();
    let around: Vec<Hex> = render_area(map, moved, 1).into_iter().filter(|h| !fixed.contains(h)).collect();
    // (toutes les notes d'abord, sur les altitudes nouvelles, puis les reliefs)
    let wanted: Vec<(Hex, Relief)> = around.iter().filter_map(|&h| map.natural_relief(h).map(|r| (h, r))).collect();
    for &(h, r) in &wanted {
        let now = map.relief(h);
        let base = if now == Relief::Peak { Relief::Mountain } else { now };
        if base != r {
            map.set_relief(h, r);
            changed.push(h);
            notes.push(format!("{} : relief {} → {} (dénivelée avec les voisines)", coords(map, h), token(now.name()), token(r.name())));
        }
    }
    let mut candidates: Vec<Hex> = wanted.iter().map(|&(h, _)| h).filter(|&h| map.relief(h) == Relief::Mountain || map.relief(h) == Relief::Peak).collect();
    candidates.sort_by(|&a, &b| map.elevation(b).unwrap_or(0.0).total_cmp(&map.elevation(a).unwrap_or(0.0)));
    for h in candidates {
        let was_peak = map.relief(h) == Relief::Peak;
        // (examinée comme une montagne : elle-même n'empêche pas d'être un pic)
        map.set_relief(h, Relief::Mountain);
        let peak_near = map.neighbors(h).iter().any(|&n| map.relief(map.canonical(n)) == Relief::Peak);
        let peak = map.could_be_peak(h) && !peak_near;
        if peak {
            map.set_relief(h, Relief::Peak);
        }
        if peak != was_peak {
            changed.push(h);
            let (a, b) = if peak { ("montagne", "pic") } else { ("pic", "montagne") };
            notes.push(format!("{} : relief {a} → {b} ({})", coords(map, h), if peak { "sommet local assez haut" } else { "plus un sommet assez haut" }));
        }
    }
    (changed, notes)
}

/// Une incohérence de la carte.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub hex: Hex,
    pub kind: &'static str,
    pub detail: String,
}

/// Incohérences de la carte, ou de la zone (case, rayon) : ce qu'une carte générée respecte.
pub fn verify(map: &WorldMap, area: Option<(Hex, i32)>) -> Vec<Issue> {
    let cells: Vec<Hex> = match area {
        Some((h, r)) => render_area(map, &[h], r),
        None => map.hexes().collect(),
    };
    let mut out = Vec::new();
    let filled = filled_surface(map);
    let mut issue = |hex: Hex, kind: &'static str, detail: String| out.push(Issue { hex, kind, detail });
    let neighbors = |h: Hex| map.neighbors(h).into_iter().map(|n| map.canonical(n)).filter(|&n| map.contains(n)).collect::<Vec<_>>();
    // Cuvettes (cases sous le niveau rempli) traversées par de l'eau, sans lac : l'eau y monterait
    let in_area: bevy::platform::collections::HashSet<Hex> = cells.iter().copied().collect();
    let pooled = |h: Hex| map.get(h).is_some_and(|t| !t.is_water()) && filled[map.cell(h)] > map.surface_elevation(h).unwrap_or(0.0) + WATER_TOLERANCE_M;
    let mut seen = bevy::platform::collections::HashSet::new();
    for &h in &cells {
        if !pooled(h) || !seen.insert(h) {
            continue;
        }
        let mut group = vec![h];
        let mut k = 0;
        while k < group.len() {
            for n in neighbors(group[k]) {
                if pooled(n) && seen.insert(n) {
                    group.push(n);
                }
            }
            k += 1;
        }
        let wet = group.iter().any(|&g| map.small_lake(g) || neighbors(g).iter().any(|&n| map.get(n) == Some(Terrain::Lake)));
        let flow = group.iter().map(|&g| map.discharge(g)).fold(0.0, f32::max);
        let deepest = *group.iter().max_by(|&&a, &&b| (filled[map.cell(a)] - map.surface_elevation(a).unwrap_or(0.0)).total_cmp(&(filled[map.cell(b)] - map.surface_elevation(b).unwrap_or(0.0)))).unwrap();
        let level = filled[map.cell(deepest)];
        let depth = level - map.surface_elevation(deepest).unwrap_or(0.0);
        if !wet && flow > POOL_MIN_FLOW && depth > POOL_MIN_DEPTH_M && in_area.contains(&deepest) {
            issue(deepest, "eau-retenue", format!("cuvette de {} case(s) traversée par {flow:.0} m³/s, sans lac (l'eau monterait à {level:.0} m)", group.len()));
        }
    }
    for &h in &cells {
        let Some(t) = map.get(h) else { continue };
        let relief = map.relief(h);
        let ground = map.elevation(h).unwrap_or(0.0);
        if !t.is_water() {
            if relief == Relief::Peak {
                // (une paire n'est signalée qu'une fois)
                for n in neighbors(h).into_iter().filter(|&n| map.relief(n) == Relief::Peak && n.to_offset() > h.to_offset()) {
                    issue(h, "pics-voisins", format!("pic voisin en {}", coords(map, n)));
                }
                if ground < PEAK_MIN_M {
                    issue(h, "relief-altitude", format!("pic à {ground:.0} m (au moins {PEAK_MIN_M:.0} m)"));
                }
            }
            if relief == Relief::Mountain && ground < MOUNTAIN_MIN_M {
                issue(h, "relief-altitude", format!("montagne à {ground:.0} m (au moins {MOUNTAIN_MIN_M:.0} m)"));
            }
            if map.is_fleuve(h) && relief != Relief::Flat {
                issue(h, "relief-fleuve", format!("fleuve au relief {}", token(relief.name())));
            }
            if !biome_fits(map, h, t) {
                let c = biome_climate(map, h);
                let expected = c.map_or("—".into(), |c| token(biome(c).name()));
                let (temp, arid) = c.map_or((0.0, 0.0), |c| (c.temperature, c.aridity));
                issue(h, "biome", format!("{} ; le climat donne {expected} ({temp:.1} °C, aridité {arid:.2})", token(t.name())));
            }
            // (dans une cuvette, l'eau monte jusqu'au seuil : c'est le niveau rempli qui compte)
            if let Some(d) = map.downstream(h) {
                let n = map.canonical(map.wrap().neighbor(h, d));
                let (here, there) = (filled[map.cell(h)], filled.get(map.cell(n)).copied().unwrap_or(f32::MAX));
                if map.contains(n) && there > here + WATER_TOLERANCE_M {
                    issue(h, "riviere-remonte", format!("l'eau va vers {} plus haut ({here:.0} → {there:.0} m)", coords(map, n)));
                }
            }
        }
        if t == Terrain::Lake {
            let here = map.surface_elevation(h).unwrap_or(0.0);
            for n in neighbors(h).into_iter().filter(|&n| map.get(n) == Some(Terrain::Lake) && n.to_offset() > h.to_offset()) {
                let there = map.surface_elevation(n).unwrap_or(0.0);
                if (there - here).abs() > WATER_TOLERANCE_M {
                    issue(h, "lac-niveaux", format!("surface {here:.0} m, voisine {} à {there:.0} m", coords(map, n)));
                }
            }
        }
    }
    out
}

/// Niveau de l'eau qui remplirait chaque cuvette jusqu'à son seuil (m) : surface de chaque case
/// relevée au plus bas passage vers la mer (inondation depuis la mer, file de priorité).
fn filled_surface(map: &WorldMap) -> Vec<f32> {
    use std::cmp::Reverse;
    let n = (map.width * map.height) as usize;
    let mut filled = vec![f32::MAX; n];
    let mut queue = std::collections::BinaryHeap::new();
    // (clé entière : millimètres, pour l'ordre)
    let key = |m: f32| Reverse((m * 1000.0) as i64);
    for h in map.hexes() {
        if map.get(h).is_some_and(|t| t.is_sea()) {
            let i = map.cell(h);
            filled[i] = 0.0;
            queue.push((key(0.0), i));
        }
    }
    while let Some((Reverse(k), i)) = queue.pop() {
        if (k as f32 / 1000.0) > filled[i] + 0.001 {
            continue;
        }
        let h = map.hex_at(i);
        for nb in map.neighbors(h) {
            let nb = map.canonical(nb);
            if !map.contains(nb) {
                continue;
            }
            let j = map.cell(nb);
            let level = map.surface_elevation(nb).unwrap_or(0.0).max(filled[i]);
            if level < filled[j] {
                filled[j] = level;
                queue.push((key(level), j));
            }
        }
    }
    // Terres sans passage vers la mer (carte sans mer) : leur propre surface
    for h in map.hexes() {
        let i = map.cell(h);
        if filled[i] == f32::MAX {
            filled[i] = map.surface_elevation(h).unwrap_or(0.0);
        }
    }
    filled
}

// ---------------------------------------------------------------- Séance

/// Ce que la console doit faire en plus de la réponse (commandes qui touchent au jeu).
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Goto(Hex),
    View(bool),
    Camera { distance: f32, yaw_deg: Option<f32> },
    Layer(MapLayer),
    Capture(String),
    Wait(u32),
    SaveJournal(String),
    Clear,
    Quit,
}

/// Réponse à une commande.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    pub ok: bool,
    /// Première ligne : `OK …` ou `ERREUR …`.
    pub lines: Vec<String>,
    /// Cases dont les données ont changé (rendu à refaire, voisines comprises).
    pub changed: Vec<Hex>,
    pub effect: Option<Effect>,
}

impl Reply {
    fn ok(lines: Vec<String>) -> Self {
        Reply { ok: true, lines, ..Default::default() }
    }
    fn error(message: impl std::fmt::Display) -> Self {
        Reply { ok: false, lines: vec![format!("ERREUR {message}")], ..Default::default() }
    }
    fn effect(line: String, effect: Effect) -> Self {
        Reply { ok: true, lines: vec![line], effect: Some(effect), ..Default::default() }
    }
}

/// État de la console : pinceau, modifications à annuler, commandes réussies (journal).
#[derive(Default)]
pub struct Session {
    pub brush: i32,
    history: Vec<(WorldMap, Vec<Hex>)>,
    pub journal: Vec<String>,
}

impl Session {
    /// Exécute une ligne : `cursor` est la case sous le curseur, `focus` le centre de la vue.
    pub fn run(&mut self, map: &mut WorldMap, line: &str, cursor: Option<Hex>, focus: Hex) -> Reply {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Reply { ok: true, ..Default::default() };
        }
        let reply = match parse(map, line) {
            Ok(cmd) => self.execute(map, cmd, cursor, focus),
            Err(e) => Reply::error(e),
        };
        // (le journal garde ce qui se rejoue : pas l'aide, ni la lecture du journal)
        let replayable = !matches!(token(split_local(line).0.split_whitespace().next().unwrap_or("")).as_str(), "aide" | "journal" | "effacer" | "info" | "trouver" | "verifier");
        if reply.ok && replayable {
            self.journal.push(line.to_string());
        }
        reply
    }

    fn execute(&mut self, map: &mut WorldMap, cmd: Command, cursor: Option<Hex>, focus: Hex) -> Reply {
        match cmd {
            Command::Info(at) => match at.or(cursor) {
                Some(h) if map.contains(map.canonical(h)) => Reply::ok(info(map, h)),
                _ => Reply::error("aucune case sous le curseur : info <col> <lig>"),
            },
            Command::Find { criteria, near, max } => {
                let near = near.unwrap_or(focus);
                let (found, total) = find(map, &criteria, near, max);
                let mut lines = vec![format!("OK {total} case(s), {} affichée(s), depuis {}", found.len(), coords(map, near))];
                lines.extend(found.iter().map(|&h| format!("{} — {} {:.0}m (à {} cases)", coords(map, h), map.describe(h), map.elevation(h).unwrap_or(0.0), map.distance(h, near))));
                Reply::ok(lines)
            }
            Command::Verify(area) => {
                let issues = verify(map, area);
                let near = area.map_or(focus, |(h, _)| h);
                let mut counts: Vec<(&str, usize)> = Vec::new();
                for i in &issues {
                    match counts.iter_mut().find(|(k, _)| *k == i.kind) {
                        Some((_, n)) => *n += 1,
                        None => counts.push((i.kind, 1)),
                    }
                }
                let zone = area.map_or("toute la carte".to_string(), |(h, r)| format!("rayon {r} autour de {}", coords(map, h)));
                let summary = counts.iter().map(|(k, n)| format!("{k}={n}")).collect::<Vec<_>>().join(" ");
                let mut lines = vec![format!("OK {} incohérence(s), {zone}{}{summary}", issues.len(), if summary.is_empty() { "" } else { " : " })];
                let mut sorted = issues;
                sorted.sort_by_key(|i| (map.distance(i.hex, near), i.hex.to_offset()));
                lines.extend(sorted.iter().take(VERIFY_SHOWN).map(|i| format!("{} — {} — {}", coords(map, i.hex), i.kind, i.detail)));
                if sorted.len() > VERIFY_SHOWN {
                    lines.push(format!("… {} autre(s) (verifier <col> <lig> <rayon> pour une zone)", sorted.len() - VERIFY_SHOWN));
                }
                Reply::ok(lines)
            }
            Command::Edit(at, field, local) => {
                let snapshot = map.clone();
                let cells: Vec<Hex> = map.range(at, self.brush).map(|h| map.canonical(h)).filter(|&h| map.contains(h)).collect();
                let mut changed = Vec::new();
                let mut notes = Vec::new();
                let mut skipped = 0;
                // Pic local au pinceau : un seul sommet, au centre, entouré de montagnes (jamais deux
                // pics côte à côte)
                let center = map.canonical(at);
                let spread = local && field == Field::Relief(Relief::Peak) && cells.len() > 1;
                if spread {
                    notes.push(format!("pinceau : pic en {}, montagnes autour", coords(map, center)));
                }
                let mut moved = Vec::new();
                for &h in &cells {
                    let before = map.elevation(h).unwrap_or(0.0);
                    let was_water = map.get(h).is_some_and(|t| t.is_water());
                    let field = if spread && h != center { Field::Relief(Relief::Mountain) } else { field };
                    match set_field(map, h, field) {
                        Ok(()) => {
                            changed.push(h);
                            if local {
                                let (more, said) = settle(map, h, field, before);
                                changed.extend(more);
                                notes.extend(said);
                                if map.elevation(h).unwrap_or(0.0) != before || map.get(h).is_some_and(|t| t.is_water()) != was_water {
                                    moved.push(h);
                                }
                            }
                        }
                        Err(_) => skipped += 1,
                    }
                }
                if !moved.is_empty() {
                    let fixed: &[Hex] = if matches!(field, Field::Relief(_)) { &cells } else { &[] };
                    let (more, said) = reshape(map, &moved, fixed);
                    changed.extend(more);
                    notes.extend(said);
                }
                let edited = cells.len() - skipped;
                changed.sort_by_key(|h| h.to_offset());
                changed.dedup();
                if changed.is_empty() {
                    return Reply::error(format!("{} ne s'applique pas à la case {} (case d'eau)", field_text(field), coords(map, at)));
                }
                self.history.push((snapshot, changed.clone()));
                if self.history.len() > UNDO_DEPTH {
                    self.history.remove(0);
                }
                let skipped = if skipped > 0 { format!(", {skipped} case(s) d'eau laissée(s)") } else { String::new() };
                let mode = if local { "local : température, biome et pics recalculés" } else { "brut : rien d'autre n'est recalculé" };
                let mut lines = vec![format!("OK {} sur {edited} case(s) autour de {} (pinceau {}){skipped} ; {mode}", field_text(field), coords(map, at), self.brush)];
                if local && notes.is_empty() {
                    lines.push("rien d'autre à changer".into());
                }
                lines.extend(notes);
                // (un fleuve garde un relief plat : on le dit plutôt que de laisser croire au changement)
                if let Field::Relief(r) = field
                    && let Some(&h) = cells.iter().find(|&&h| map.is_fleuve(h) && map.relief(h) != r)
                {
                    lines.push(format!("note : {} reste {} (fleuve)", coords(map, h), token(map.relief(h).name())));
                }
                Reply { ok: true, lines, changed, effect: None }
            }
            Command::Brush(r) => {
                self.brush = r;
                Reply::ok(vec![format!("OK pinceau={r} ({} case(s) par modification)", 3 * r * (r + 1) + 1)])
            }
            Command::Undo => match self.history.pop() {
                Some((before, changed)) => {
                    *map = before;
                    Reply { ok: true, lines: vec![format!("OK annulé ({} case(s) remises comme avant, {} annulation(s) restante(s))", changed.len(), self.history.len())], changed, effect: None }
                }
                None => Reply::error("rien à annuler"),
            },
            Command::Goto(h) => Reply::effect(format!("OK vue sur {}", coords(map, h)), Effect::Goto(h)),
            Command::View(is_3d) => Reply::effect(format!("OK vue {}", if is_3d { "3d" } else { "2d" }), Effect::View(is_3d)),
            Command::Camera { distance, yaw_deg } => Reply::effect(format!("OK camera distance={distance:.0}{}", yaw_deg.map_or(String::new(), |y| format!(" angle={y:.0}"))), Effect::Camera { distance, yaw_deg }),
            Command::Layer(l) => Reply::effect(format!("OK carte={}", token(l.short_name())), Effect::Layer(l)),
            Command::Capture(path) => Reply::effect(format!("OK capture demandée : {path}"), Effect::Capture(path)),
            Command::Wait(n) => Reply::effect(format!("OK attente de {n} image(s)"), Effect::Wait(n)),
            Command::Help(about) => help(about.as_deref()).map_or_else(Reply::error, Reply::ok),
            Command::Journal(None) => {
                let mut lines = vec![format!("OK {} commande(s) au journal", self.journal.len())];
                lines.extend(self.journal.iter().cloned());
                Reply::ok(lines)
            }
            Command::Journal(Some(path)) => Reply::effect(format!("OK journal de {} commande(s) à écrire : {path}", self.journal.len()), Effect::SaveJournal(path)),
            Command::Clear => Reply::effect("OK".into(), Effect::Clear),
            Command::Quit => Reply::effect("OK fermeture".into(), Effect::Quit),
        }
    }
}

/// Cases dont le rendu est à refaire après la modification de `changed` : le relief 3D d'une case
/// déborde sur ses voisines (bosses, arêtes entre sommets), et l'eau d'une case dépend des rives.
pub fn render_area(map: &WorldMap, changed: &[Hex], reach: i32) -> Vec<Hex> {
    let mut out: Vec<Hex> = changed.iter().flat_map(|&h| map.range(h, reach)).map(|h| map.canonical(h)).filter(|&h| map.contains(h)).collect();
    out.sort_by_key(|h| h.to_offset());
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> WorldMap {
        let mut m = WorldMap::filled(12, 8, Terrain::Plains);
        m.set(Hex::from_offset(0, 0), Terrain::Ocean);
        m
    }

    #[test]
    fn noms_en_un_mot_sans_accents() {
        assert_eq!(token("Forêt tempérée"), "foret-temperee");
        assert_eq!(token("Taïga"), "taiga");
        assert_eq!(token("vallonné"), "vallonne");
        let m = small();
        assert_eq!(parse(&m, "terrain 3 4 foret"), Ok(Command::Edit(Hex::from_offset(3, 4), Field::Terrain(Terrain::Forest), false)));
        // « savane » est un nom entier : pas d'ambiguïté avec « savane-arboree »
        assert_eq!(parse(&m, "terrain 3 4 Savane"), Ok(Command::Edit(Hex::from_offset(3, 4), Field::Terrain(Terrain::Savanna), false)));
        assert!(parse(&m, "terrain 3 4 sa").unwrap_err().contains("ambigu"));
        assert!(parse(&m, "terrain 3 4").unwrap_err().starts_with("usage"));
        assert!(parse(&m, "terrain 3 40 foret").unwrap_err().contains("hors de la carte"));
        assert!(parse(&m, "teren 1 1 foret").unwrap_err().contains("inconnue"));
    }

    #[test]
    fn completion_des_commandes_et_des_valeurs() {
        assert_eq!(complete("ter").line, "terrain ");
        let c = complete("a");
        assert_eq!(c.line, "a");
        assert!(c.candidates.contains(&"altitude".to_string()) && c.candidates.contains(&"aller".to_string()));
        assert_eq!(complete("terrain 3 4 tai").line, "terrain 3 4 taiga ");
        assert_eq!(complete("relief 3 4 mo").line, "relief 3 4 montagne ");
        // Partie commune de plusieurs propositions
        assert_eq!(complete("terrain 3 4 sav").line, "terrain 3 4 savane");
        // Critères : clé, puis valeur
        assert_eq!(complete("trouver rel").line, "trouver relief=");
        assert_eq!(complete("trouver relief=pi").line, "trouver relief=pic ");
        assert_eq!(complete("trouver relief=p").candidates, ["relief=plat", "relief=pic"]);
        assert_eq!(complete("carte cl").line, "carte climat ");
        assert!(hint("terrain 3").starts_with("terrain <col> <lig> <terrain>"));
    }

    #[test]
    fn aide_de_chaque_commande() {
        for s in SPECS {
            let lines = help(Some(s.name)).unwrap();
            assert!(lines[0].starts_with("OK "), "{}", s.name);
            assert!(!s.help.is_empty());
        }
        // Chaque commande décrite se décode (avec des arguments plausibles)
        let m = small();
        for s in SPECS {
            let args: Vec<&str> = s.args.iter().filter(|a| !a.2).map(|a| match a.1 {
                Arg::Col | Arg::Row => "2",
                Arg::Terrain => "prairie",
                Arg::Relief => "pic",
                Arg::Number => "300",
                Arg::Layer => "relief",
                Arg::View => "3d",
                Arg::Command => "aide",
                Arg::Criteria => "eau=non",
                Arg::Text => "x",
            }).collect();
            assert!(parse(&m, &format!("{} {}", s.name, args.join(" "))).is_ok(), "{}", s.name);
        }
    }

    #[test]
    fn modification_au_pinceau_puis_annulation() {
        let mut m = small();
        let mut s = Session::default();
        let c = Hex::from_offset(5, 4);
        assert!(s.run(&mut m, "pinceau 1", None, c).ok);
        let r = s.run(&mut m, "relief 5 4 montagne", None, c);
        assert!(r.ok, "{:?}", r.lines);
        assert_eq!(r.changed.len(), 7);
        assert!(m.neighbors(c).iter().all(|&n| m.relief(n) == Relief::Mountain));
        // Altitude : la classe d'altitude suit
        s.run(&mut m, "pinceau 0", None, c);
        assert!(s.run(&mut m, "altitude 5 4 3200", None, c).ok);
        assert_eq!(m.elevation(c), Some(3200.0));
        assert_eq!(m.level(c), Some(crate::map::level_of_meters(3200.0)));
        // Pas de relief sur l'eau
        assert!(!s.run(&mut m, "relief 0 0 pic", None, c).ok);
        assert!(s.run(&mut m, "annuler", None, c).ok);
        assert_ne!(m.elevation(c), Some(3200.0));
        assert_eq!(m.relief(c), Relief::Mountain);
        let r = s.run(&mut m, "annuler", None, c);
        assert_eq!(r.changed.len(), 7);
        assert_eq!(m.relief(c), Relief::Flat);
        assert!(!s.run(&mut m, "annuler", None, c).ok);
        // Le journal garde les commandes réussies qui se rejouent
        assert_eq!(s.journal, ["pinceau 1", "relief 5 4 montagne", "pinceau 0", "altitude 5 4 3200", "annuler", "annuler"]);
    }

    #[test]
    fn recherche_par_criteres_et_info() {
        let mut m = small();
        let mut s = Session::default();
        let c = Hex::from_offset(5, 4);
        s.run(&mut m, "relief 2 2 pic", None, c);
        s.run(&mut m, "relief 3 2 montagne", None, c);
        s.run(&mut m, "terrain 3 2 foret", None, c);
        let r = s.run(&mut m, "trouver relief=montagne voisin=pic terrain=foret", None, c);
        assert!(r.lines[0].starts_with("OK 1 case(s)"), "{:?}", r.lines);
        assert!(r.lines[1].starts_with("3 2"), "{:?}", r.lines);
        // La plus proche d'abord, au plus `max`
        let (found, total) = find(&m, &[Criterion::Water(false)], c, 3);
        assert_eq!((found.len(), total), (3, 12 * 8 - 1));
        assert_eq!(found[0], c);
        let lines = info(&m, Hex::from_offset(3, 2));
        assert!(lines[1].contains("terrain=foret-temperee relief=montagne"), "{lines:?}");
        assert_eq!(lines.len(), 4 + 6);
        assert!(s.run(&mut m, "info", Some(c), c).ok);
        assert!(!s.run(&mut m, "info", None, c).ok);
    }

    #[test]
    #[ignore]
    fn bilan_verifier() {
        for seed in [777, 2, 5, 4242] {
            let m = WorldMap::generate_in(128, 74, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 74));
            let issues = verify(&m, None);
            let mut kinds: Vec<&str> = issues.iter().map(|i| i.kind).collect();
            kinds.sort();
            kinds.dedup();
            println!("graine {seed} : {} incohérence(s)", issues.len());

            for k in kinds {
                let of: Vec<&Issue> = issues.iter().filter(|i| i.kind == k).collect();
                println!("  {k} : {}", of.len());
                for i in of.iter().take(4) {
                    println!("    {} {}", coords(&m, i.hex), i.detail);
                }
            }
        }
    }

    #[test]
    fn carte_generee_sans_incoherence_puis_modifications_locales() {
        let mut m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        assert_eq!(verify(&m, None), Vec::new(), "une carte générée est cohérente");
        let mut s = Session::default();
        let focus = Hex::from_offset(48, 28);
        let plain = |m: &WorldMap| {
            m.hexes()
                .find(|&h| {
                    m.get(h).is_some_and(|t| !t.is_water()) && m.relief(h) == Relief::Flat && !m.is_fleuve(h) && m.elevation(h).unwrap() < 600.0 && m.temperature(h).unwrap() > 15.0
                        && m.neighbors(h).iter().all(|&n| m.get(n).is_some_and(|t| !t.is_water()) && m.relief(n) != Relief::Peak)
                })
                .unwrap()
        };
        let h = plain(&m);
        let at = coords(&m, h);
        let (t0, e0) = (m.temperature(h).unwrap(), m.elevation(h).unwrap());
        // Brut : la température ne suit pas l'altitude
        assert!(s.run(&mut m, &format!("altitude {at} {}", e0 + 2000.0), None, focus).ok);
        assert_eq!(m.temperature(h), Some(t0));
        s.run(&mut m, "annuler", None, focus);
        // Local : -6,5 °C par 1 000 m, biome selon le nouveau climat
        let r = s.run(&mut m, &format!("~altitude {at} {}", e0 + 2000.0), None, focus);
        assert!(r.ok, "{:?}", r.lines);
        assert!((m.temperature(h).unwrap() - (t0 - 13.0)).abs() < 0.01);
        assert!(biome_fits(&m, h, m.get(h).unwrap()), "{:?}", r.lines);
        // Pic posé bas : il monte à 2 400 m ; un pic voisin n'en est plus un
        let n = m.canonical(m.neighbors(h)[0]);
        let nat = coords(&m, n);
        s.run(&mut m, &format!("relief {nat} montagne"), None, focus);
        s.run(&mut m, &format!("altitude {nat} 3000"), None, focus);
        s.run(&mut m, &format!("relief {nat} pic"), None, focus);
        let r = s.run(&mut m, &format!("~relief {at} pic"), None, focus);
        assert!(r.ok, "{:?}", r.lines);
        assert_eq!(m.relief(h), Relief::Peak);
        assert!(m.elevation(h).unwrap() >= PEAK_MIN_M);
        // (puis son relief est recalculé selon sa dénivelée : montagne ou accidenté, plus un pic)
        assert_ne!(m.relief(n), Relief::Peak);
        assert!(r.changed.contains(&n));
        assert!(verify(&m, Some((h, 1))).iter().all(|i| i.kind != "pics-voisins" && i.kind != "relief-altitude"));
        // Brut : deux pics côte à côte et un pic trop bas sont signalés
        s.run(&mut m, &format!("relief {nat} pic"), None, focus);
        s.run(&mut m, &format!("altitude {at} 1000"), None, focus);
        let kinds: Vec<&str> = verify(&m, Some((h, 1))).iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&"pics-voisins") && kinds.contains(&"relief-altitude"), "{kinds:?}");
        // Pic local au pinceau : un pic au centre, des montagnes autour
        s.run(&mut m, "pinceau 1", None, focus);
        assert!(s.run(&mut m, &format!("~relief {at} pic"), None, focus).ok);
        assert_eq!(m.relief(h), Relief::Peak);
        assert!(m.neighbors(h).iter().map(|&n| m.canonical(n)).filter(|&n| !m.is_fleuve(n) && m.get(n).is_some_and(|t| !t.is_water())).all(|n| m.relief(n) == Relief::Mountain && m.elevation(n).unwrap() >= MOUNTAIN_MIN_M));
        s.run(&mut m, "pinceau 0", None, focus);
        // Plateau relevé d'un bloc : le centre n'a plus de dénivelée, il n'est pas montagne ; les
        // bords, au-dessus de la plaine, sont escarpés
        let c = plain(&m);
        let cat = coords(&m, c);
        s.run(&mut m, "pinceau 2", None, focus);
        let r = s.run(&mut m, &format!("~altitude {cat} 2600"), None, focus);
        assert!(r.ok, "{:?}", r.lines);
        assert!(m.relief(c) != Relief::Mountain && m.relief(c) != Relief::Peak, "centre du plateau : {:?}", m.relief(c));
        let edge: Vec<Hex> = m.range(c, 2).map(|h| m.canonical(h)).filter(|&h| m.distance(h, c) == 2 && !m.is_fleuve(h) && m.get(h).is_some_and(|t| !t.is_water())).collect();
        assert!(edge.iter().any(|&h| matches!(m.relief(h), Relief::Mountain | Relief::Rugged)), "{:?}", r.lines);
        assert_eq!(verify(&m, Some((c, 3))).iter().filter(|i| i.kind == "pics-voisins" || i.kind == "relief-altitude").count(), 0);
        s.run(&mut m, "pinceau 0", None, focus);
        // « ~ » réservé aux modifications d'une case
        assert!(!s.run(&mut m, "~aller 3 3", None, focus).ok);
        assert!(!s.run(&mut m, "~pinceau 2", None, focus).ok);
        assert_eq!(complete("~ter").line, "~terrain ");
    }

    #[test]
    fn barrage_sur_un_fleuve_signale() {
        let mut m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        // Case de fleuve dont l'aval est un fleuve de terre : on la surélève de 1 500 m
        let h = m
            .hexes()
            .find(|&h| m.is_fleuve(h) && m.downstream(h).is_some_and(|d| { let n = m.canonical(m.wrap().neighbor(h, d)); m.is_fleuve(n) && m.discharge(h) > 50.0 }))
            .unwrap();
        let d = m.downstream(h).unwrap();
        let n = m.canonical(m.wrap().neighbor(h, d));
        let mut s = Session::default();
        let (nat, hat, high) = (coords(&m, n), coords(&m, h), m.elevation(n).unwrap() + 1500.0);
        let r = s.run(&mut m, &format!("altitude {nat} {high}"), None, h);
        assert!(r.ok);
        let kinds: Vec<&str> = verify(&m, Some((h, 2))).iter().map(|i| i.kind).collect();
        assert!(kinds.iter().any(|k| *k == "eau-retenue" || *k == "riviere-remonte"), "{kinds:?}");
        let r = s.run(&mut m, &format!("verifier {hat} 2"), None, h);
        assert!(r.lines[0].starts_with("OK ") && !r.lines[0].starts_with("OK 0 "), "{:?}", r.lines);
    }

    #[test]
    fn relief_recalcule_comme_a_la_generation() {
        for seed in [777, 2] {
            let m = WorldMap::generate_in(96, 56, seed, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
            let land: Vec<Hex> = m.hexes().filter(|&h| m.get(h).is_some_and(|t| !t.is_water())).collect();
            let mut wrong = Vec::new();
            for &h in &land {
                let generated = if m.relief(h) == Relief::Peak { Relief::Mountain } else { m.relief(h) };
                if m.natural_relief(h) != Some(generated) {
                    wrong.push((coords(&m, h), generated, m.natural_relief(h)));
                }
                if m.relief(h) == Relief::Peak {
                    assert!(m.could_be_peak_ignoring_relief(h), "pic {} non reconnu", coords(&m, h));
                }
            }
            println!("graine {seed} : {} / {} cases de relief différent : {:?}", wrong.len(), land.len(), &wrong[..wrong.len().min(8)]);
            assert!(wrong.is_empty(), "graine {seed} : {} cases", wrong.len());
        }
    }
}
