//! État du jeu côté ECS : composants, ressources, initialisation et fin de tour.

use crate::cases::CaseSet;
use crate::explore::{self, ExploreOutcome, ExploreWorld};
use crate::feedback::{Actor, Cue, Cues};
use crate::hex::Hex;
use crate::hostile::{self, Blocked, Danger, EnemyZoc, Hostile, HostileTurnPending, KnownVillages, Occupancy, WorldSeed};
use crate::map::WorldMap;
use crate::peuple::{Peuple, Tresors};
use crate::rules::{self, CityState, MoveOutcome, UnitKind, UnitState};
use crate::search::MapTarget;
use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};

pub const MAP_WIDTH: i32 = 192;
pub const MAP_HEIGHT: i32 = 111;

const CITY_NAMES: &[&str] = &[
    "Paris", "Lyon", "Marseille", "Bordeaux", "Toulouse", "Nantes", "Lille", "Strasbourg",
    "Rennes", "Montpellier", "Nice", "Dijon", "Reims", "Rouen", "Tours", "Limoges",
];

// ---------------------------------------------------------------- Composants

/// Position d'une entité sur la grille.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HexPos(pub Hex);

/// Unité ; son peuple (`Peuple`) est le joueur sauf mention contraire.
#[derive(Component, Debug)]
#[require(Peuple)]
pub struct Unit(pub UnitState);

/// Ville ; son peuple (`Peuple`) est le joueur sauf mention contraire.
#[derive(Component, Debug)]
#[require(Peuple)]
pub struct City {
    pub state: CityState,
    /// Travailleurs placés par l'intendant et rendement de la ville (recalculés à chaque changement).
    pub travail: crate::intendant::Repartition,
}

/// Aménagements ruraux des cases (état du monde, sauvegardé).
#[derive(Resource, Default, Clone, Debug)]
pub struct Amenagements(pub std::collections::BTreeMap<Hex, crate::intendant::Amenagement>);

/// Ensemble du système qui donne les identifiants stables (les systèmes qui en ont besoin passent après).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentSet;

/// Identifiant stable d'un objet de la partie (unité, ville, camp, village). Contrairement à
/// `Entity`, il ne dépend pas de la façon dont Bevy range ses objets : il fixe l'ordre de jeu, et
/// une sauvegarde pourra le garder. Donné dans l'ordre de création (`attribuer_identifiants`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ident(pub u32);

/// Configuration de la partie en cours (écran de nouvelle partie, `config_partie.rs`).
#[derive(Resource, Default)]
pub struct ConfigEnCours(pub crate::config_partie::ConfigPartie);

/// Mesure du passage de tour en cours et des précédents (`chrono_tour.rs`).
#[derive(Resource, Default)]
pub struct ChronoTour {
    /// Passage de tour en cours : début de l'étape courante et durées déjà mesurées.
    en_cours: Option<(std::time::Instant, crate::chrono_tour::MesureTour)>,
    pub historique: crate::chrono_tour::Historique,
}

/// Début de la mesure, quand le joueur vient de demander la fin du tour.
fn chrono_debut(requested: Res<EndTurnRequested>, state: Res<GameState>, mut chrono: ResMut<ChronoTour>) {
    if requested.0 {
        let mesure = crate::chrono_tour::MesureTour { tour: state.turn, etapes: Vec::new() };
        chrono.en_cours = Some((std::time::Instant::now(), mesure));
    }
}

/// Fin d'une étape du passage de tour : sa durée est notée sous `nom`.
fn chrono_etape(nom: &'static str) -> impl FnMut(ResMut<ChronoTour>) {
    move |mut chrono: ResMut<ChronoTour>| {
        if let Some((debut, mesure)) = chrono.en_cours.as_mut() {
            let maintenant = std::time::Instant::now();
            mesure.etapes.push((nom, maintenant - *debut));
            *debut = maintenant;
        }
    }
}

/// Fin de la mesure : le tour rejoint l'historique ; un tour trop long est signalé dans le terminal.
fn chrono_fin(mut chrono: ResMut<ChronoTour>) {
    let Some((_, mesure)) = chrono.en_cours.take() else { return };
    let total = mesure.total();
    if total > crate::chrono_tour::BUDGET_TOUR {
        warn!("Passage de tour trop long (budget {}) : {}", crate::chrono_tour::ms(crate::chrono_tour::BUDGET_TOUR), mesure.texte());
    } else {
        debug!("Passage de tour : {}", mesure.texte());
    }
    chrono.historique.ajouter(mesure);
}

/// Prochain identifiant stable à donner.
#[derive(Resource, Default)]
pub struct ProchainIdent(pub u32);

/// Donne un identifiant stable aux objets de la partie qui n'en ont pas encore. Ceux apparus dans
/// la même image sont rangés par case, puis villes avant unités : surtout pas par `Entity`, dont
/// l'ordre dépend des objets supprimés auparavant (il diffère après le chargement d'une partie).
fn attribuer_identifiants(
    mut commands: Commands,
    mut prochain: ResMut<ProchainIdent>,
    nouveaux: Query<(Entity, Option<&HexPos>, Has<City>), (Without<Ident>, Or<(With<Unit>, With<City>, With<hostile::Camp>, With<hostile::Village>)>)>,
) {
    let mut liste: Vec<(Option<Hex>, bool, Entity)> = nouveaux.iter().map(|(e, p, ville)| (p.map(|p| p.0), !ville, e)).collect();
    liste.sort();
    for (_, _, entity) in liste {
        commands.entity(entity).insert(Ident(prochain.0));
        prochain.0 += 1;
    }
}

/// Clé de l'ordre de jeu : l'identifiant stable (les objets créés dans l'image, qui n'en ont pas
/// encore, passent après, dans leur ordre de création).
pub fn rang(idents: &Query<&Ident>, entity: Entity) -> (u32, Entity) {
    (idents.get(entity).map_or(u32::MAX, |i| i.0), entity)
}

// ---------------------------------------------------------------- Ressources

#[derive(Resource)]
pub struct GameMap(pub WorldMap);

#[derive(Resource, Default)]
pub struct GameState {
    pub turn: u32,
    /// Trésor de chaque peuple.
    pub tresors: Tresors,
    pub founded_cities: usize,
}

impl GameState {
    /// État d'une nouvelle partie : premier tour, trésor de départ du joueur.
    pub fn new() -> Self {
        let mut state = Self { turn: 1, ..Default::default() };
        *state.tresors.get_mut(Peuple::JOUEUR) = 10;
        state
    }

    /// Trésor du joueur.
    pub fn tresor_joueur(&self) -> i32 {
        self.tresors.get(Peuple::JOUEUR)
    }

    /// Verse les deniers d'une ville (fractionnaire) au trésor d'un peuple.
    pub fn ajouter_deniers(&mut self, peuple: Peuple, deniers: f64) {
        self.tresors.ajouter(peuple, deniers);
    }

    /// Trésor d'un peuple, à modifier.
    pub fn tresor_mut(&mut self, peuple: Peuple) -> &mut i32 {
        self.tresors.get_mut(peuple)
    }
}

/// Brouillard de guerre : cases déjà explorées et cases vues ce tour-ci.
#[derive(Resource, Default)]
pub struct Fog {
    pub explored: CaseSet,
    pub visible: CaseSet,
}

/// Brouillard tel qu'il est affiché : celui du joueur, ou la carte entière en mode debug.
/// La logique du jeu (chemins, exploration, IA) lit toujours `Fog`, la connaissance réelle du
/// joueur ; seul l'affichage lit `ShownFog`.
#[derive(Resource, Default)]
pub struct ShownFog {
    pub explored: CaseSet,
    pub visible: CaseSet,
}

/// Gisements connus du joueur et cases déjà prospectées (5f) : un affleurement se voit dès que
/// sa case est explorée ; la pierre dure et les parures se découvrent par la prospection (dans le
/// territoire des villes, ou par un éclaireur qui passe).
#[derive(Resource, Default)]
pub struct Gisements {
    pub connus: HashSet<Hex>,
    pub prospectees: HashSet<Hex>,
}

impl Gisements {
    /// Prospecte une case ; rend la ressource d'un gisement caché qu'on vient d'y trouver.
    pub fn prospecter(&mut self, map: &WorldMap, h: Hex) -> Option<crate::gisements::Ressource> {
        let h = map.canonical(h);
        if !self.prospectees.insert(h) {
            return None;
        }
        let r = map.gisement(h)?;
        (self.connus.insert(h) && !r.affleure()).then_some(r)
    }

    /// Gisement connu de la case (tous en mode debug).
    pub fn connu(&self, map: &WorldMap, h: Hex, debug: bool) -> Option<crate::gisements::Ressource> {
        let h = map.canonical(h);
        map.gisement(h).filter(|_| debug || self.connus.contains(&h))
    }
}

/// Cases du territoire qu'une ville prospecte par tour (décision D3).
pub const PROSPECTION_PAR_TOUR: usize = 2;

/// Mode debug : révèle toute la carte à l'affichage (menu Échap).
#[derive(Resource, Default)]
pub struct DebugMode(pub bool);

/// Propriétaire de chaque case revendiquée.
#[derive(Resource, Default)]
pub struct Territory(pub HashMap<Hex, Entity>);

#[derive(Resource, Default)]
pub struct Selection(pub Option<Entity>);

/// Importance d'un message du journal : règle la force de sa mise en évidence à l'écran.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Importance {
    /// Retour d'une commande (ordre refusé, production choisie) : discret.
    Routine,
    /// Information de la partie (production terminée, croissance).
    Info,
    /// Bonne nouvelle (ville fondée, récompense, ennemi dispersé).
    Success,
    /// À surveiller (combat, famine, production bloquée).
    Warning,
    /// Grave (unité perdue, village hostile).
    Critical,
}

/// Renvoi d'un passage du journal vers la carte : le lieu ou l'unité désigné, et sa case au
/// moment du message (dernière position connue si l'entité disparaît ou sort de la vue).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogLink {
    pub target: MapTarget,
    pub at: Hex,
}

/// Morceau d'un message du journal : texte simple ou renvoi.
#[derive(Clone, Debug, PartialEq)]
pub struct LogSpan {
    pub text: String,
    pub link: Option<LogLink>,
}

/// Message du journal en morceaux ; les noms de lieux et d'unités renvoient vers la carte.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogText(pub Vec<LogSpan>);

impl LogText {
    /// Message vide, à compléter par `text` et `link`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute du texte simple.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        match self.0.last_mut() {
            Some(last) if last.link.is_none() => last.text.push_str(&text),
            _ => self.0.push(LogSpan { text, link: None }),
        }
        self
    }

    /// Ajoute un passage qui renvoie vers un lieu ou une unité (`at` : sa case actuelle).
    pub fn link(mut self, text: impl Into<String>, target: MapTarget, at: Hex) -> Self {
        self.0.push(LogSpan { text: text.into(), link: Some(LogLink { target, at }) });
        self
    }

    /// Renvoi vers une unité.
    pub fn unit(self, text: impl Into<String>, entity: Entity, at: Hex) -> Self {
        self.link(text, MapTarget::Unit(entity), at)
    }

    /// Renvoi vers une ville.
    pub fn city(self, text: impl Into<String>, entity: Entity, at: Hex) -> Self {
        self.link(text, MapTarget::City(entity), at)
    }

    /// Texte complet, sans les renvois.
    pub fn plain(&self) -> String {
        self.0.iter().map(|s| s.text.as_str()).collect()
    }
}

impl From<String> for LogText {
    fn from(text: String) -> Self {
        LogText::new().text(text)
    }
}

impl From<&str> for LogText {
    fn from(text: &str) -> Self {
        LogText::new().text(text)
    }
}

/// Message du journal, numéroté pour que l'interface repère les nouveaux.
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub id: u64,
    pub text: LogText,
    pub importance: Importance,
}

/// Nombre de messages gardés au journal.
pub const LOG_LENGTH: usize = 6;

/// Journal des événements, affiché dans l'interface.
#[derive(Resource, Default)]
pub struct GameLog {
    entries: Vec<LogEntry>,
    next_id: u64,
    /// Combats décisifs (une unité dispersée) en attente d'être inscrits dans l'histoire des villes.
    pub combats: Vec<(Hex, String)>,
}

impl GameLog {
    /// Vide le journal (tournage des animations : une scène sans les messages de la précédente).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Message de routine (retour d'une commande).
    pub fn push(&mut self, msg: impl Into<LogText>) {
        self.notify(Importance::Routine, msg);
    }

    /// Message d'une importance donnée.
    pub fn notify(&mut self, importance: Importance, msg: impl Into<LogText>) {
        self.entries.push(LogEntry { id: self.next_id, text: msg.into(), importance });
        self.next_id += 1;
        if self.entries.len() > LOG_LENGTH {
            self.entries.remove(0);
        }
    }

    /// Messages gardés, du plus ancien au plus récent.
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }
}

/// Demande de fin de tour (posée par le clavier ou le bouton).
#[derive(Resource, Default)]
pub struct EndTurnRequested(pub bool);

/// Systèmes de résolution du tour (exécutés après les commandes du joueur).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EndTurnSet;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        // Partie à charger (`CIVBUILDER_CHARGER`, voir `sauvegarde.rs`) : sa graine et sa taille
        // priment ; illisible, on part sur un nouveau monde
        let charge = match crate::sauvegarde::sauvegarde_au_lancement() {
            Some(Ok(s)) => Some(s),
            Some(Err(e)) => {
                error!("Chargement impossible : {e}");
                None
            }
            None => None,
        };
        // Configuration choisie sur l'écran de nouvelle partie (`CIVBUILDER_CONFIG`)
        let choisie = crate::config_partie::config_au_lancement();
        // Graine : sauvegarde, configuration, ou CIVBUILDER_SEED (pour rejouer une carte)
        let seed = charge
            .as_ref()
            .map(|s| s.graine)
            .or(choisie.as_ref().map(|c| c.graine))
            .or_else(|| std::env::var("CIVBUILDER_SEED").ok().and_then(|s| s.parse().ok()))
            .unwrap_or_else(rand::random::<u32>);
        // Une planète entière, d'un pôle à l'autre, avec des cases de 40 km
        // (taille imposée par CIVBUILDER_MAP, par exemple « 128x74 »)
        let (width, height) = std::env::var("CIVBUILDER_MAP")
            .ok()
            .and_then(|s| {
                let (w, h) = s.split_once(['x', 'X'])?;
                Some((w.trim().parse::<i32>().ok()?, h.trim().parse::<i32>().ok()?))
            })
            .filter(|&(w, h)| w >= 16 && h >= 10)
            .unwrap_or((MAP_WIDTH, MAP_HEIGHT));
        let (width, height) = choisie.as_ref().map_or((width, height), |c| c.taille.cases());
        let (width, height) = charge.as_ref().map_or((width, height), |s| (s.largeur, s.hauteur));
        // Configuration de la partie : celle de la sauvegarde, celle choisie, ou par défaut
        let config = charge.as_ref().map(|s| s.config.clone()).or(choisie.clone()).unwrap_or_else(|| {
            let mut c = crate::config_partie::ConfigPartie::avec_graine(seed);
            c.taille = crate::config_partie::Taille::depuis_cases((width, height)).unwrap_or_default();
            c
        });
        // Une partie lancée depuis l'écran de nouvelle partie rejoint la bibliothèque
        if choisie.is_some() && charge.is_none() {
            let mut b = crate::config_partie::Bibliotheque::lire();
            b.partie_lancee(&config, &crate::config_partie::date_du_jour());
            if let Err(e) = b.ecrire() {
                warn!("Bibliothèque des configurations : {e}");
            }
        }
        let region = crate::climate::Region::planet(crate::climate::CELL_KM, height);
        let started = std::time::Instant::now();
        let map = WorldMap::generate_in(width, height, seed, region);
        info!(
            "Carte générée en {:.1} s (graine {seed}, {width} × {height} cases de {} km)",
            started.elapsed().as_secs_f32(),
            region.cell_km
        );
        let currents = map.currents();
        let fastest = (0..currents.velocity.len()).map(|i| currents.speed(i)).fold(0.0f32, f32::max);
        let around = |yes: bool| if yes { "oui" } else { "non" };
        info!(
            "Courants marins : jusqu'à {fastest:.1} m/s ; courant circumpolaire au nord : {}, au sud : {}",
            around(currents.circumpolar.0),
            around(currents.circumpolar.1)
        );
        app.insert_resource(GameMap(map))
            .insert_resource(WorldSeed(seed as u64))
            .insert_resource(GameState::new())
            .insert_resource(crate::hasard::Hasard::new(seed))
            .init_resource::<Fog>()
            .init_resource::<ShownFog>()
            .init_resource::<DebugMode>()
            .init_resource::<Territory>()
            .init_resource::<Amenagements>()
            .init_resource::<Gisements>()
            .init_resource::<Selection>()
            .init_resource::<GameLog>()
            .init_resource::<EndTurnRequested>()
            .init_resource::<ProchainIdent>()
            .init_resource::<ChronoTour>()
            .insert_resource(crate::sauvegarde::ChargementEnAttente(charge))
            .insert_resource(ConfigEnCours(config))
            .add_systems(Startup, spawn_starting_units)
            .add_systems(PreUpdate, attribuer_identifiants.in_set(IdentSet))
            .add_systems(
                Update,
                (
                    chrono_debut,
                    end_turn,
                    chrono_etape("villes et unités"),
                    hostile::hostile_turn,
                    chrono_etape("faction hostile"),
                    hostile::visit_sites,
                    chrono_etape("villages et camps"),
                    refresh_worked_tiles,
                    chrono_etape("cases exploitées"),
                    update_fog,
                    chrono_etape("brouillard"),
                    decouvrir_gisements,
                    hostile::update_knowledge,
                    chrono_etape("connaissances"),
                    chrono_fin,
                )
                    .chain()
                    .in_set(EndTurnSet),
            )
            .add_systems(Update, sync_shown_fog.after(EndTurnSet))
            .add_systems(Update, chroniquer_combats.after(EndTurnSet));
    }
}

/// Crée une unité à une position donnée (les visuels sont ajoutés par le module de rendu).
pub fn spawn_unit(commands: &mut Commands, kind: UnitKind, at: Hex) -> Entity {
    commands.spawn((Unit(UnitState::new(kind)), HexPos(at))).id()
}

fn spawn_starting_units(
    mut commands: Commands,
    map: Res<GameMap>,
    mut selection: ResMut<Selection>,
    mut log: ResMut<GameLog>,
) {
    let start = map.0.find_start().expect("aucun emplacement de départ");
    let settler = spawn_unit(&mut commands, UnitKind::Settler, start);
    // Milice et éclaireur sur des cases voisines praticables
    let mut free = map.0.neighbors(start).into_iter().filter(|&h| map.0.get(h).and_then(|t| t.move_cost()).is_some());
    spawn_unit(&mut commands, UnitKind::Militia, free.next().unwrap_or(start));
    spawn_unit(&mut commands, UnitKind::Scout, free.next().unwrap_or(start));
    selection.0 = Some(settler);
    log.notify(Importance::Info, "Bienvenue ! Fondez votre capitale avec le colon (touche B).");
}

/// Fonde une ville avec le colon `settler`. Retourne un message d'erreur si impossible.
pub fn found_city(
    commands: &mut Commands,
    settler: Entity,
    at: Hex,
    map: &WorldMap,
    existing: &[Hex],
    state: &mut GameState,
    territory: &mut Territory,
) -> Result<(Entity, String), &'static str> {
    rules::can_found_city(map, at, existing)?;
    let name = CITY_NAMES
        .get(state.founded_cities)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Ville {}", state.founded_cities + 1));
    state.founded_cities += 1;
    commands.entity(settler).despawn();
    let mut etat = CityState::new(name.clone());
    etat.noter(state.turn, rules::GenreEvenement::Fondation, format!("Fondation de {name}"), Some(at));
    let city = commands
        .spawn((City { state: etat, travail: Default::default() }, HexPos(at)))
        .id();
    claim_territory(&mut territory.0, city, at, 1, map);
    Ok((city, name))
}

/// Achète la case `h` pour la ville (4d) : une voisine de son territoire, à personne, payée en
/// deniers (20 + 10 × distance au centre) ; elle relève le seuil de culture comme une case gagnée.
/// Rend le prix payé, ou la raison du refus.
#[allow(clippy::too_many_arguments)]
pub fn acheter_case(map: &WorldMap, territoire: &mut Territory, state: &mut GameState, ville: Entity, centre: Hex, city: &mut rules::CityState, h: Hex, tour: u32) -> Result<i32, String> {
    let h = map.canonical(h);
    if let Some(e) = territoire.0.get(&h) {
        return Err(if *e == ville { "la case est déjà à la ville".into() } else { "la case est à une autre ville".into() });
    }
    if !rules::cases_gagnables(map, ville, &territoire.0).contains(&h) {
        return Err("la case doit toucher le territoire de la ville".into());
    }
    let prix = rules::prix_case(map.distance(centre, h));
    let tresor = state.tresor_mut(Peuple::JOUEUR);
    if *tresor < prix {
        return Err(format!("il faut {prix} deniers (trésor : {})", *tresor));
    }
    *tresor -= prix;
    territoire.0.insert(h, ville);
    // (une case achetée relève aussi le seuil de culture de la suivante ; Benjamin, 12/11/2026)
    city.cases_gagnees += 1;
    let lieu = map.describe(h).to_lowercase();
    city.noter(tour, rules::GenreEvenement::Territoire, format!("Case achetée pour {prix} deniers : {lieu}"), Some(h));
    Ok(prix)
}

pub(crate) fn claim_territory(territory: &mut HashMap<Hex, Entity>, city: Entity, center: Hex, radius: i32, map: &WorldMap) {
    for h in map.range(center, radius).filter(|&h| map.contains(h)) {
        territory.entry(h).or_insert(city);
    }
}

/// Fait avancer une unité le long de son ordre en découvrant le terrain
/// (le trajet est recalculé à chaque pas avec la connaissance à jour).
/// `blocked` : cases connues infranchissables (ennemis visibles, unités alliées) ;
/// `occupied` : occupation réelle de toutes les cases, tenue à jour après le déplacement ;
/// `zoc` : zones de contrôle ennemies (arrêt en y entrant).
pub fn move_unit(
    entity: Entity,
    unit: &mut UnitState,
    pos: &mut Mut<HexPos>,
    map: &WorldMap,
    fog: &mut Fog,
    log: &mut GameLog,
    blocked: &HashSet<Hex>,
    occupied: &mut HashSet<Hex>,
    zoc: &HashSet<Hex>,
    allies: &HashSet<Hex>,
) {
    if unit.path.is_empty() {
        return;
    }
    let before = unit.on_bridge && map.fleuve_bridge(pos.0).is_some();
    let mut hex = pos.0;
    match unit.advance_exploring(&mut hex, map, &mut fog.explored, blocked, occupied, zoc, allies) {
        MoveOutcome::Moved => {}
        MoveOutcome::Unreachable => {
            log.notify(Importance::Info, LogText::new().unit(unit.kind.name(), entity, hex).text(" : destination inaccessible, ordre annulé."));
        }
        MoveOutcome::TooLong => {
            log.notify(Importance::Info, LogText::new().unit(unit.kind.name(), entity, hex).text(" : trajet bien plus long que prévu, ordre suspendu."));
        }
    }
    relocate_unit(pos, before, hex, unit.on_bridge, occupied);
}

/// Déplace une unité et met à jour l'occupation des cases, avec sa place sur le pont d'un fleuve
/// (`map::slot`) : avant (`before` : sur le pont au départ) et après.
pub fn relocate_unit(pos: &mut Mut<HexPos>, before: bool, to: Hex, on_bridge: bool, occupied: &mut HashSet<Hex>) {
    let (from, at) = (crate::map::slot(pos.0, before), crate::map::slot(to, on_bridge));
    if from != at {
        occupied.remove(&from);
        occupied.insert(at);
    }
    if pos.0 != to {
        pos.0 = to;
    }
}

/// Union des cases bloquées par l'ennemi visible et par les unités alliées.
pub fn movement_blocked(hostile: &HashSet<Hex>, own: &HashSet<Hex>) -> HashSet<Hex> {
    hostile.union(own).copied().collect()
}

/// Case libre pour faire apparaître une unité : `at` si libre, sinon une voisine franchissable.
pub fn free_tile_near(map: &WorldMap, at: Hex, occupied: &HashSet<Hex>) -> Option<Hex> {
    std::iter::once(at)
        .chain(map.neighbors(at))
        .find(|h| !occupied.contains(h) && map.get(*h).and_then(|t| t.move_cost()).is_some())
}

/// Cases réservées par les explorateurs autres que `except` (cibles déjà choisies).
pub fn reserved_by_others<'a>(map: &WorldMap, units: impl IntoIterator<Item = (Entity, &'a UnitState)>, except: Entity) -> HashSet<Hex> {
    explore::reserved_by(
        map.wrap(),
        units
            .into_iter()
            .filter(|(e, u)| *e != except && u.auto_explore)
            .filter_map(|(_, u)| u.explore_target.map(|t| (t, u.kind.sight()))),
    )
}

/// Un tour d'exploration automatique ; l'arrête quand il n'y a plus rien à découvrir.
pub fn explore_unit(
    entity: Entity,
    unit: &mut UnitState,
    pos: &mut Mut<HexPos>,
    world: &ExploreWorld,
    occupied: &mut HashSet<Hex>,
    fog: &mut Fog,
    log: &mut GameLog,
) {
    let before = unit.on_bridge && world.map.fleuve_bridge(pos.0).is_some();
    let mut hex = pos.0;
    let outcome = explore::explore_step(unit, &mut hex, &world.occupied(occupied), &mut fog.explored);
    if outcome == ExploreOutcome::Finished {
        unit.auto_explore = false;
        log.push(LogText::new().unit(unit.kind.name(), entity, hex).text(" : plus rien d'accessible à explorer."));
    }
    let after = unit.on_bridge && world.map.fleuve_bridge(hex).is_some();
    relocate_unit(pos, before, hex, after, occupied);
}

/// Recalcule les travailleurs de toutes les villes (une case n'est exploitée que par une ville).
pub fn assign_worked_tiles(map: &WorldMap, territory: &Territory, amenagements: &Amenagements, cities: &mut [(Entity, Hex, Mut<City>)]) {
    let mut taken: HashSet<Hex> = HashSet::new();
    for (entity, center, city) in cities.iter_mut() {
        let owned: Vec<Hex> = territory.0.iter().filter(|(_, e)| *e == entity).map(|(h, _)| *h).collect();
        // Ce que le lieu donne à la salubrité (eau, marais, pâturages : 4c)
        let site = rules::site_de_la_ville(map, *center, &owned, &amenagements.0);
        if city.state.site != site {
            city.state.site = site;
        }
        let cases = rules::cases_de_la_ville(map, *center, &owned, &taken, &amenagements.0);
        let travail = rules::repartition_ville(map, *center, &city.state, &cases);
        taken.extend(travail.cases.iter().map(|(h, _)| *h));
        if city.travail != travail {
            city.travail = travail;
        }
    }
}

fn end_turn(
    mut commands: Commands,
    mut requested: ResMut<EndTurnRequested>,
    mut game_map: ResMut<GameMap>,
    mut state: ResMut<GameState>,
    mut territory: ResMut<Territory>,
    mut log: ResMut<GameLog>,
    mut fog: ResMut<Fog>,
    (blocked, occupancy, enemy_zoc): (Res<Blocked>, Res<Occupancy>, Res<EnemyZoc>),
    (danger, mut amenagements): (Res<Danger>, ResMut<Amenagements>),
    villages: Res<KnownVillages>,
    mut hostile_turn: ResMut<HostileTurnPending>,
    mut cues: ResMut<Cues>,
    mut cities: Query<(Entity, &HexPos, &mut City)>,
    mut units: Query<(Entity, &mut Unit, &mut HexPos), (Without<City>, Without<Hostile>)>,
    idents: Query<&Ident>,
) {
    if !std::mem::take(&mut requested.0) {
        return;
    }
    let map = &game_map.0;
    // (le joueur reprendra la main à la fin des animations du passage de tour)
    cues.0.push(Cue::TurnEnd);

    // 1) Deniers des villes
    let mut list: Vec<(Entity, Hex, Mut<City>)> = cities.iter_mut().map(|(e, p, c)| (e, p.0, c)).collect();
    list.sort_by_key(|(e, _, _)| rang(&idents, *e));
    assign_worked_tiles(map, &territory, &amenagements, &mut list);
    let mut new_units = Vec::new();
    let mut new_bridges = Vec::new();
    let mut new_amenagements = Vec::new();
    let tour = state.turn;
    for (entity, center, city) in list.iter_mut() {
        let yields = city.travail.rendements;
        let humeur = city.state.humeur();
        let (chomeurs, par_type, par_sorte) = (city.travail.chomeurs, city.travail.par_type, city.travail.par_sorte);
        let report = city.state.passer_le_tour(yields, chomeurs, par_type, &par_sorte);
        // Gisements exploités du territoire : leurs ressources vont au stock de la ville (5f)
        for (&h, &a) in amenagements.0.iter() {
            if a == crate::intendant::Amenagement::Exploitation
                && territory.0.get(&h) == Some(entity)
                && let Some(r) = map.gisement(h)
            {
                let unites = crate::intendant::unites_extraites(city.travail.sur(h), crate::intendant::capacite(map, h, Some(a)));
                city.state.ajouter_ressource(r, unites);
            }
        }
        // Outils (5h), puis besoins de base et luxe (5g) : la ville prend dans son stock
        let amenages: u32 = amenagements.0.keys().filter(|h| territory.0.get(*h) == Some(entity)).map(|h| city.travail.sur(*h)).sum();
        city.state.consommer_outils(amenages);
        city.state.consommer_ressources();
        // Histoire de la ville : paliers, famines, constructions marquantes
        if report.grew {
            let (p, h) = (city.state.palier(), city.state.habitants);
            city.state.noter(tour, rules::GenreEvenement::Population, format!("Palier {p} atteint : {} habitants", rules::milliers(h)), None);
        }
        if report.starved != city.state.en_famine {
            let texte = if report.starved { "Début d'une famine : la population baisse" } else { "Fin de la famine" };
            city.state.noter(tour, rules::GenreEvenement::Famine, texte, None);
            city.state.en_famine = report.starved;
        }
        // Salubrité sous le seuil : les maladies font perdre des habitants (4c)
        let insalubre = city.state.salubrite() < rules::SALUBRITE_SEUIL;
        if insalubre != city.state.insalubre {
            let texte = if insalubre { "La salubrité tombe sous 50 : les maladies emportent des habitants" } else { "La salubrité remonte : moins de maladies" };
            city.state.noter(tour, rules::GenreEvenement::Population, texte, None);
            city.state.insalubre = insalubre;
        }
        match report.built {
            Some(rules::Production::Bridge { at, .. }) => {
                let h = match at {
                    crate::map::BridgeAt::Edge { hex, .. } | crate::map::BridgeAt::Fleuve { hex, .. } => hex,
                };
                city.state.noter(tour, rules::GenreEvenement::Construction, format!("{} construit", rules::Production::Bridge { at, cost: 0 }.name()), Some(h));
            }
            Some(rules::Production::Amenagement { hex, kind, .. }) => {
                let texte = format!("Aménagement achevé : {} ({})", kind.nom(), map.describe(hex).to_lowercase());
                city.state.noter(tour, rules::GenreEvenement::Construction, texte, Some(hex));
            }
            Some(rules::Production::Puits) => city.state.noter(tour, rules::GenreEvenement::Construction, "Puits creusé : la salubrité monte", Some(*center)),
            Some(rules::Production::Grenier) => city.state.noter(tour, rules::GenreEvenement::Construction, "Grenier bâti : la ville garde ses excédents", Some(*center)),
            _ => {}
        }
        // (seul le joueur a des villes pour l'instant ; la fraction de deniers est gardée par le trésor)
        state.ajouter_deniers(Peuple::JOUEUR, report.deniers);
        let name = city.state.name.clone();
        if report.grew {
            log.notify(Importance::Info, LogText::new().city(&name, *entity, *center).text(format!(" atteint le palier {}.", city.state.palier())));
        }
        // Territoire par la culture (4d) : une case de plus à chaque seuil (le palier ne le fixe plus)
        while city.state.culture >= rules::seuil_culture(city.state.cases_gagnees) {
            let Some(h) = rules::case_par_la_culture(map, *center, *entity, &territory.0) else { break };
            city.state.culture -= rules::seuil_culture(city.state.cases_gagnees);
            city.state.cases_gagnees += 1;
            territory.0.insert(h, *entity);
            let lieu = map.describe(h).to_lowercase();
            city.state.noter(tour, rules::GenreEvenement::Territoire, format!("La culture gagne une case : {lieu}"), Some(h));
            log.notify(Importance::Info, LogText::new().city(&name, *entity, *center).text(format!(" gagne une case par la culture ({lieu}).")));
        }
        if report.colon_attend {
            log.notify(Importance::Warning, LogText::new().city(&name, *entity, *center).text(format!(" : le colon est prêt, mais il faut {} habitants pour le former ; la file est bloquée.", rules::milliers(rules::CityState::HABITANTS_POUR_COLON))));
        }
        if report.starved {
            log.notify(Importance::Warning, LogText::new().city(&name, *entity, *center).text(" souffre de famine !"));
        } else if report.puise > 0.0 {
            let reste = city.state.stock_total() / city.state.besoin_nourriture().max(1e-6);
            log.notify(Importance::Warning, LogText::new().city(&name, *entity, *center).text(format!(" vit sur son grenier : {} de nourriture puisés, il en reste pour {:.0} tour(s).", rules::decimal(report.puise), reste.floor())));
        }
        if insalubre && !report.starved {
            log.notify(Importance::Warning, LogText::new().city(&name, *entity, *center).text(format!(" : salubrité {:.0} sur 100, les maladies emportent des habitants.", city.state.salubrite())));
        }
        if city.state.humeur() != humeur {
            let importance = if city.state.humeur() == rules::Humeur::Mecontente { Importance::Warning } else { Importance::Info };
            log.notify(importance, LogText::new().city(&name, *entity, *center).text(format!(" est maintenant {} (bien-être {:.0}).", city.state.humeur().nom(), city.state.bien_etre)));
        }
        if report.inactive {
            log.notify(Importance::Warning, LogText::new().city(&name, *entity, *center).text(" n'a rien à produire : sa production est perdue. Choisissez une construction ou un projet (touche C)."));
        }
        if report.rappel_projet
            && let Some(p) = city.state.projet
        {
            log.notify(Importance::Info, LogText::new().city(&name, *entity, *center).text(format!(" suit le projet « {} » depuis {} tours.", p.nom(), city.state.tours_en_projet)));
        }
        match report.built {
            Some(rules::Production::Unit(kind)) => new_units.push((*entity, kind, *center, name)),
            Some(rules::Production::Bridge { at, .. }) => new_bridges.push((*entity, at, *center, name)),
            Some(rules::Production::Amenagement { hex, kind, .. }) => new_amenagements.push((*entity, hex, kind, *center, name)),
            Some(rules::Production::Puits) => log.push(LogText::new().city(&name, *entity, *center).text(" a creusé son puits : la salubrité monte.")),
            Some(rules::Production::Grenier) => log.push(LogText::new().city(&name, *entity, *center).text(" a bâti son grenier : elle gardera ses excédents contre les famines.")),
            None => {}
        }
    }
    drop(list);

    // 2) Unités : soins, PM restaurés, puis poursuite des ordres de déplacement.
    // Les unités jouent l'une après l'autre (une unité par case) ; les explorateurs évitent
    // les cibles déjà choisies par les autres.
    let mut occupied = occupancy.all.clone();
    let mut own = occupancy.own.clone();
    let threats: HashSet<Hex> = occupancy.all.difference(&occupancy.own).copied().collect();
    let mut order: Vec<Entity> = units.iter().map(|(e, _, _)| e).collect();
    order.sort_by_key(|e| rang(&idents, *e));
    for entity in order {
        let reserved = reserved_by_others(map, units.iter().map(|(e, u, _)| (e, &u.0)), entity);
        let Ok((_, mut unit, mut pos)) = units.get_mut(entity) else { continue };
        let friendly = territory.0.contains_key(&pos.0);
        unit.0.start_turn(friendly);
        let before = pos.0;
        let blocked_now = movement_blocked(&blocked.0, &own);
        if unit.0.auto_explore {
            let world = ExploreWorld::new(map)
                .reserved(&reserved)
                .blocked(&blocked_now)
                .villages(&villages.0)
                .danger(&danger.0)
                .threats(&threats);
            explore_unit(entity, &mut unit.0, &mut pos, &world, &mut occupied, &mut fog, &mut log);
        } else {
            // (les alliés se traversent : seuls les ennemis visibles bloquent le trajet)
            move_unit(entity, &mut unit.0, &mut pos, map, &mut fog, &mut log, &blocked.0, &mut occupied, &enemy_zoc.0, &own);
        }
        if pos.0 != before {
            own.remove(&before);
            own.insert(pos.0);
            cues.walk(entity, before, &unit.0.last_steps, Actor::Player);
        }
    }

    // 3) Unités produites : sur la ville, ou une case voisine libre ; sinon la production attend
    for (city_entity, kind, center, name) in new_units {
        match free_tile_near(map, center, &occupied) {
            Some(at) => {
                occupied.insert(at);
                let unit = spawn_unit(&mut commands, kind, at);
                log.notify(Importance::Info, LogText::new().city(&name, city_entity, center).text(" a produit : ").unit(kind.name(), unit, at).text("."));
                if kind == UnitKind::Settler
                    && let Ok((_, _, mut city)) = cities.get_mut(city_entity)
                {
                    let h = city.state.habitants;
                    let texte = format!("Un colon part avec {} habitants : il en reste {} (palier {} → {})", rules::milliers(rules::HABITANTS_COLON), rules::milliers(h), rules::palier_de(h + rules::HABITANTS_COLON), rules::palier_de(h));
                    city.state.noter(tour, rules::GenreEvenement::Population, texte, Some(at));
                    log.notify(Importance::Info, LogText::new().city(&name, city_entity, center).text(format!(" : {} habitants partent avec le colon.", rules::milliers(rules::HABITANTS_COLON))));
                }
            }
            None => {
                if let Ok((_, _, mut city)) = cities.get_mut(city_entity) {
                    // (l'unité revient en tête de file, déjà payée)
                    city.state.file.insert(0, rules::Commande { objet: rules::Production::Unit(kind), investi: kind.cost() as f64 });
                    if kind == UnitKind::Settler {
                        city.state.habitants += rules::HABITANTS_COLON;
                    }
                }
                log.notify(Importance::Warning, LogText::new().city(&name, city_entity, center).text(format!(" : pas de place pour {}, la production attend.", kind.name())));
            }
        }
    }

    // 4) Ponts construits : posés sur la carte (un pont déjà là, posé entre-temps, ne change rien)
    for (city_entity, at, center, name) in new_bridges {
        game_map.0.set_bridge(at, true);
        if let Ok((_, _, mut city)) = cities.get_mut(city_entity) {
            city.state.bridges.push(at);
        }
        let what = match at {
            crate::map::BridgeAt::Edge { .. } => "un pont sur la rivière",
            crate::map::BridgeAt::Fleuve { .. } => "un pont sur le fleuve",
        };
        log.notify(Importance::Info, LogText::new().city(&name, city_entity, center).text(format!(" a construit {what}.")));
    }

    // 5) Aménagements construits : la case change (l'intendant y place ensuite les travailleurs)
    for (city_entity, hex, kind, center, name) in new_amenagements {
        amenagements.0.insert(hex, kind);
        log.notify(Importance::Info, LogText::new().city(&name, city_entity, center).text(format!(" a fini l'aménagement : {} ({}).", kind.nom(), map_name(&game_map.0, hex))));
    }

    // 6) Puis la faction hostile joue (système suivant)
    hostile_turn.0 = true;
    state.turn += 1;
}

/// Nom d'une case pour le journal (« Prairie vallonnée »).
fn map_name(map: &WorldMap, hex: Hex) -> String {
    map.describe(hex).to_lowercase()
}

/// Inscrit les combats décisifs dans l'histoire de la ville dont ils touchent le territoire.
fn chroniquer_combats(mut log: ResMut<GameLog>, territory: Res<Territory>, state: Res<GameState>, map: Res<GameMap>, mut cities: Query<&mut City>) {
    if log.combats.is_empty() {
        return;
    }
    for (at, texte) in std::mem::take(&mut log.combats) {
        let at = map.0.canonical(at);
        if let Some(&ville) = territory.0.get(&at)
            && let Ok(mut city) = cities.get_mut(ville)
        {
            city.state.noter(state.turn, rules::GenreEvenement::Bataille, texte, Some(at));
        }
    }
}

/// Maintient à jour les cases exploitées (nouvelle ville, territoire étendu, nouveau tour).
fn refresh_worked_tiles(
    map: Res<GameMap>,
    state: Res<GameState>,
    territory: Res<Territory>,
    amenagements: Res<Amenagements>,
    mut cities: Query<(Entity, &HexPos, &mut City)>,
    idents: Query<&Ident>,
) {
    // Parcourir des `Mut` sans les déréférencer en écriture ne les marque pas « modifiés ».
    // (une ville changée : habitants, consignes… ; `assign_worked_tiles` n'écrit que si le travail change)
    let any_changed = cities.iter_mut().any(|(_, _, c)| c.is_changed());
    if !any_changed && !territory.is_changed() && !state.is_changed() && !amenagements.is_changed() {
        return;
    }
    let mut list: Vec<(Entity, Hex, Mut<City>)> = cities.iter_mut().map(|(e, p, c)| (e, p.0, c)).collect();
    list.sort_by_key(|(e, _, _)| rang(&idents, *e));
    assign_worked_tiles(&map.0, &territory, &amenagements, &mut list);
}

/// Recalcule les cases visibles à partir des unités et des villes du joueur.
/// Brouillard affiché : copie de celui du joueur, ou toute la carte en mode debug. En quittant le
/// mode debug, on revient exactement à ce que le joueur a exploré.
fn sync_shown_fog(fog: Res<Fog>, debug: Res<DebugMode>, map: Res<GameMap>, mut shown: ResMut<ShownFog>) {
    if !fog.is_changed() && !debug.is_changed() {
        return;
    }
    // En mode debug, toute la carte est déjà affichée : un changement du brouillard du joueur ne
    // change rien à l'écran (sans ce garde-fou, chaque tour redessinait toute la carte)
    if debug.0 && !debug.is_changed() && shown.explored.len() == map.0.hexes().count() {
        return;
    }
    if debug.0 {
        let all: CaseSet = map.0.hexes().collect();
        shown.visible = all.clone();
        shown.explored = all;
    } else {
        shown.explored = fog.explored.clone();
        shown.visible = fog.visible.clone();
    }
}

/// Découverte des gisements : affleurements des cases explorées, prospection des éclaireurs qui
/// passent, puis, à chaque tour, celle des villes dans leur territoire (les cases les plus proches
/// du centre d'abord).
#[allow(clippy::too_many_arguments)]
fn decouvrir_gisements(
    map: Res<GameMap>,
    fog: Res<Fog>,
    state: Res<GameState>,
    territory: Res<Territory>,
    mut gisements: ResMut<Gisements>,
    mut log: ResMut<GameLog>,
    scouts: Query<(&Unit, &HexPos), (Changed<HexPos>, Without<Hostile>)>,
    cities: Query<(Entity, &City, &HexPos)>,
    mut dernier_tour: Local<u32>,
) {
    let m = &map.0;
    if fog.is_changed() {
        let nouveaux: Vec<Hex> = m.cases_a_gisement().filter(|(h, r)| r.affleure() && fog.explored.contains(h) && !gisements.connus.contains(h)).map(|(h, _)| h).collect();
        gisements.connus.extend(nouveaux);
    }
    for (unit, pos) in &scouts {
        if unit.0.kind == UnitKind::Scout
            && let Some(r) = gisements.prospecter(m, pos.0)
        {
            log.notify(Importance::Success, format!("Votre éclaireur a trouvé un gisement : {} ({}).", r.nom(), m.describe(pos.0).to_lowercase()));
        }
    }
    if state.turn == *dernier_tour {
        return;
    }
    *dernier_tour = state.turn;
    let mut villes: Vec<(Entity, &City, &HexPos)> = cities.iter().collect();
    villes.sort_by_key(|(e, ..)| *e);
    for (e, city, pos) in villes {
        let mut cases: Vec<Hex> = territory.0.iter().filter(|(h, v)| **v == e && !gisements.prospectees.contains(*h)).map(|(h, _)| *h).collect();
        cases.sort_by_key(|&h| (m.distance(pos.0, h), h));
        for h in cases.into_iter().take(PROSPECTION_PAR_TOUR) {
            if let Some(r) = gisements.prospecter(m, h) {
                log.notify(Importance::Success, LogText::new().city(&city.state.name, e, pos.0).text(format!(" a trouvé un gisement dans son territoire : {} ({}).", r.nom(), m.describe(h).to_lowercase())));
            }
        }
    }
}

fn update_fog(
    map: Res<GameMap>,
    mut fog: ResMut<Fog>,
    units: Query<(&Unit, &HexPos), Without<Hostile>>,
    cities: Query<&HexPos, With<City>>,
    changed: Query<(), Or<(Changed<HexPos>, Added<HexPos>)>>,
    mut removed: RemovedComponents<HexPos>,
) {
    if changed.is_empty() && removed.read().count() == 0 {
        return;
    }
    let mut visible = CaseSet::new();
    for (unit, pos) in &units {
        visible.extend(map.0.range(pos.0, unit.0.kind.sight()));
    }
    for pos in &cities {
        visible.extend(map.0.range(pos.0, 3));
    }
    fog.explored.extend(visible.iter());
    fog.visible = visible;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gisements_affleurements_prospection_et_exploitation() {
        use crate::intendant::{capacite, rendement_max, unites_extraites, Amenagement};
        let map = WorldMap::generate(60, 40, 42);
        let tous: Vec<(Hex, crate::gisements::Ressource)> = map.cases_a_gisement().collect();
        assert!(!tous.is_empty(), "des gisements sur un petit monde");
        // Même graine, mêmes gisements
        assert_eq!(WorldMap::generate(60, 40, 42).cases_a_gisement().collect::<Vec<_>>(), tous);
        let (h, r) = tous[0];
        // Prospecter : une seule fois par case ; on ne signale qu'un gisement caché
        let mut g = Gisements::default();
        let trouve = g.prospecter(&map, h);
        assert_eq!(trouve.is_some(), !r.affleure());
        assert!(g.connus.contains(&h) && g.prospecter(&map, h).is_none());
        assert_eq!(g.connu(&map, h, false), Some(r));
        let vide = map.hexes().find(|&v| map.gisement(v).is_none() && map.get(v).is_some_and(|t| !t.is_water())).unwrap();
        assert_eq!(g.prospecter(&map, vide), None);
        assert!(g.prospectees.contains(&vide) && g.connu(&map, vide, true).is_none());
        // Exploitation : sur un gisement seulement, 500 travailleurs, une unité par tour pleine
        assert!(Amenagement::Exploitation.possible(&map, h).is_ok());
        assert!(Amenagement::Exploitation.possible(&map, vide).is_err());
        assert_eq!(capacite(&map, h, Some(Amenagement::Exploitation)), 500);
        assert_eq!(rendement_max(&map, h, Some(Amenagement::Exploitation)).nourriture, 0.0);
        assert!((unites_extraites(500, 500) - 1.0).abs() < 1e-9 && unites_extraites(250, 500) < 1.0 && unites_extraites(0, 500) == 0.0);
        let mut ville = rules::CityState::new("Essai".into());
        ville.ajouter_ressource(r, 0.5);
        ville.ajouter_ressource(r, 0.25);
        assert!((ville.ressource(r) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn acheter_une_case_coute_des_deniers_et_releve_le_seuil_de_culture() {
        let map = WorldMap::filled(12, 12, crate::map::Terrain::Grassland);
        let centre = Hex::from_offset(6, 6);
        let ville = Entity::from_raw_u32(1).unwrap();
        let mut territoire = Territory::default();
        claim_territory(&mut territoire.0, ville, centre, 1, &map);
        let mut state = GameState::new();
        let mut city = rules::CityState::new("Essai".into());
        let loin = Hex::from_offset(6, 9);
        let voisine = *rules::cases_gagnables(&map, ville, &territoire.0).first().unwrap();
        // Trésor de départ (10) : trop peu
        assert!(acheter_case(&map, &mut territoire, &mut state, ville, centre, &mut city, voisine, 1).is_err());
        *state.tresor_mut(Peuple::JOUEUR) = 100;
        assert!(acheter_case(&map, &mut territoire, &mut state, ville, centre, &mut city, loin, 1).is_err(), "pas voisine du territoire");
        assert_eq!(acheter_case(&map, &mut territoire, &mut state, ville, centre, &mut city, voisine, 1), Ok(40));
        assert_eq!((state.tresor_joueur(), territoire.0.get(&voisine), city.cases_gagnees), (60, Some(&ville), 1));
        assert_eq!(rules::seuil_culture(city.cases_gagnees), 16.0);
        assert!(acheter_case(&map, &mut territoire, &mut state, ville, centre, &mut city, voisine, 1).is_err(), "déjà à la ville");
    }

    #[test]
    fn message_du_journal_en_morceaux() {
        let city = Entity::from_raw_u32(7).unwrap();
        let unit = Entity::from_raw_u32(9).unwrap();
        let msg = LogText::new()
            .city("Paris", city, Hex::new(3, 4))
            .text(" a produit")
            .text(" : ")
            .unit("Milice", unit, Hex::new(4, 4))
            .text(".");
        assert_eq!(msg.plain(), "Paris a produit : Milice.");
        // Les textes simples consécutifs sont fusionnés ; les renvois restent à part
        assert_eq!(msg.0.len(), 4);
        assert_eq!(msg.0[0].link, Some(LogLink { target: MapTarget::City(city), at: Hex::new(3, 4) }));
        assert_eq!(msg.0[1].text, " a produit : ");
        assert_eq!(msg.0[1].link, None);
        assert_eq!(msg.0[2].link.map(|l| l.target), Some(MapTarget::Unit(unit)));
    }

    #[test]
    fn journal_borne_et_texte_simple() {
        let mut log = GameLog::default();
        for i in 0..LOG_LENGTH + 2 {
            log.push(format!("message {i}"));
        }
        assert_eq!(log.entries().len(), LOG_LENGTH);
        assert_eq!(log.entries()[0].text.plain(), "message 2");
        assert!(log.entries().iter().all(|e| e.text.0.iter().all(|s| s.link.is_none())));
    }
}
