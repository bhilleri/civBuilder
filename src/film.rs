//! Tournage des animations en GIF (mode capture, pour la documentation) : avec
//! `CIVBUILDER_CAPTURE=<dossier> CIVBUILDER_CAPTURE_GIF=1`, le jeu joue l'une après l'autre des
//! scènes courtes — déplacement, combats, fin de tour, visites de villages et de camp — en
//! pilotant une souris dessinée à l'écran (`input::Pilot`) : les ordres passent par les vraies
//! commandes du joueur, l'interface (sélection, aperçu du chemin, prévision de combat, journal,
//! bouton de fin de tour) est filmée avec la carte. Chaque scène est jouée en 2D puis en 3D.
//!
//! Le temps du jeu avance d'une durée fixe par image (`IMAGES_PAR_SECONDE`), quelle que soit la
//! vitesse du rendu : chaque image est enregistrée en PNG, puis `ffmpeg` assemble les images
//! d'une scène en un GIF `<scène>_<2d|3d>.gif` dans le dossier (images effacées ensuite, sauf avec
//! `CIVBUILDER_CAPTURE_KEEP=1`). `CIVBUILDER_CAPTURE_GIF=tir,camp` ne tourne que ces scènes.
//! Voir `scripts/animations.sh` et `docs/ANIMATIONS.md`.

use crate::feedback::{Motion, TurnReplay};
use crate::game::{City, EndTurnRequested, Fog, GameMap, GameState, HexPos, Selection, ShownFog, Territory, Unit, found_city};
use crate::hex::Hex;
use crate::hostile::{Camp, Hostile, HostileHome, NextVillageEvent, SiteName, Village};
use crate::input::Pilot;
use crate::map::Relief;
use crate::rules::UnitState;
use crate::sites::VillageEvent;
use crate::units::UnitKind;
use crate::view3d::{Cam3d, ViewMode, shown_height, tile_height, to_3d};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;
use std::path::PathBuf;
use std::time::Duration;

/// Cadence des GIF : le temps du jeu avance de 1/`IMAGES_PAR_SECONDE` s par image.
const IMAGES_PAR_SECONDE: f64 = 20.0;
/// Taille de la fenêtre pendant le tournage (les GIF gardent cette taille).
const LARGEUR: u32 = 1280;
const HAUTEUR: u32 = 760;
/// Zoom de la vue 2D, et part de la hauteur de l'écran dont la scène est remontée (les panneaux
/// du bas — sélection, infobulle — ne la cachent pas).
const ECHELLE_2D: f32 = 0.62;
const DECALAGE: f32 = 0.14;
/// Images laissées à la scène pour s'installer avant d'être filmée (3D : relief à construire
/// autour de la zone, la première fois).
const INSTALLATION_2D: u32 = 20;
const INSTALLATION_3D: u32 = 40;
const PREMIERE_3D: u32 = 220;
/// Images attendues après une scène, le temps que les PNG soient écrits.
const ECRITURE: u32 = 30;
/// Durée du glissement du curseur vers une case, et de l'appui d'un clic (s).
const GLISSEMENT: f64 = 0.45;
const APPUI: f64 = 0.15;
/// Durée d'affichage d'une légende (« Clic droit : ordre ») après l'action (s).
const LEGENDE: f64 = 1.3;

/// Une étape d'une scène.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// Pause (s).
    Wait(f64),
    /// Le curseur glisse jusqu'à la case (décalage axial depuis le centre de la zone).
    Point(i32, i32),
    /// Clic gauche (sélection) ou droit (ordre) sur la case pointée.
    Click(MouseButton),
    /// Fin du tour (touche Entrée).
    EndTurn,
    /// Attend la fin des animations et du passage de tour.
    Idle,
}

/// Une unité de la scène.
#[derive(Clone, Copy, Debug)]
struct Role {
    kind: UnitKind,
    at: (i32, i32),
    hostile: bool,
    hp: i32,
}

const fn joueur(kind: UnitKind, q: i32, r: i32) -> Role {
    Role { kind, at: (q, r), hostile: false, hp: crate::rules::MAX_HP }
}

const fn hostile(kind: UnitKind, q: i32, r: i32) -> Role {
    Role { kind, at: (q, r), hostile: true, hp: crate::rules::MAX_HP }
}

const fn blesse(role: Role, hp: i32) -> Role {
    Role { hp, ..role }
}

/// Une scène : nom du fichier, unités, site éventuel (village et son événement, ou camp vide),
/// étapes filmées.
struct Scene {
    name: &'static str,
    cast: &'static [Role],
    village: Option<((i32, i32), VillageEvent)>,
    camp: Option<(i32, i32)>,
    steps: &'static [Step],
}

const GAUCHE: Step = Step::Click(MouseButton::Left);
const DROIT: Step = Step::Click(MouseButton::Right);

/// Toutes les scènes, dans l'ordre de `docs/ANIMATIONS.md`.
const SCENES: &[Scene] = &[
    // Sélection, aperçu du chemin, ordre : l'unité glisse case par case
    Scene {
        name: "deplacement",
        cast: &[joueur(UnitKind::LightCavalry, -2, 1)],
        village: None,
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-2, 1), GAUCHE, Step::Wait(0.5), Step::Point(0, 0), Step::Wait(0.4), Step::Point(2, -1), Step::Wait(1.0), DROIT, Step::Idle, Step::Wait(1.0)],
    },
    // Trajet de plusieurs tours : l'unité avance à chaque fin de tour
    Scene {
        name: "deplacement_plusieurs_tours",
        cast: &[joueur(UnitKind::Settler, -3, 1)],
        village: None,
        camp: None,
        steps: &[
            Step::Wait(0.5),
            Step::Point(-3, 1),
            GAUCHE,
            Step::Wait(0.4),
            Step::Point(3, -1),
            Step::Wait(1.4),
            DROIT,
            Step::Idle,
            Step::Wait(0.8),
            Step::EndTurn,
            Step::Idle,
            Step::Wait(0.8),
            Step::EndTurn,
            Step::Idle,
            Step::Wait(1.2),
        ],
    },
    // Échange de place entre deux unités voisines
    Scene {
        name: "echange",
        cast: &[joueur(UnitKind::Militia, -1, 0), joueur(UnitKind::Spearman, 0, 0)],
        village: None,
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-1, 0), GAUCHE, Step::Wait(0.5), Step::Point(0, 0), Step::Wait(0.6), DROIT, Step::Idle, Step::Wait(1.0)],
    },
    // Corps à corps : élan, secousse, PV perdus des deux côtés
    Scene {
        name: "corps_a_corps",
        cast: &[joueur(UnitKind::Spearman, -1, 0), hostile(UnitKind::Militia, 0, 0)],
        village: None,
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-1, 0), GAUCHE, Step::Wait(0.4), Step::Point(0, 0), Step::Wait(1.6), DROIT, Step::Idle, Step::Wait(1.6)],
    },
    // Charge : approche (la cible doit être en vue : 2 cases), attaque, l'ennemi dispersé s'efface, le vainqueur avance
    Scene {
        name: "charge_victoire",
        cast: &[joueur(UnitKind::HeavyCavalry, -2, 1), blesse(hostile(UnitKind::Militia, 0, 0), 30)],
        village: None,
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-2, 1), GAUCHE, Step::Wait(0.4), Step::Point(0, 0), Step::Wait(1.6), DROIT, Step::Idle, Step::Wait(1.8)],
    },
    // Tir : le trait vole jusqu'à la cible, le tireur ne subit rien
    Scene {
        name: "tir",
        cast: &[joueur(UnitKind::ExpertArcher, -2, 0), hostile(UnitKind::Militia, 0, 0)],
        village: None,
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-2, 0), GAUCHE, Step::Wait(0.4), Step::Point(0, 0), Step::Wait(1.6), DROIT, Step::Idle, Step::Wait(1.6)],
    },
    // Attaque perdue : l'attaquant trop faible est dispersé
    Scene {
        name: "attaque_perdue",
        cast: &[blesse(joueur(UnitKind::Militia, -1, 0), 10), hostile(UnitKind::ShieldWarrior, 0, 0)],
        village: None,
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-1, 0), GAUCHE, Step::Wait(0.4), Step::Point(0, 0), Step::Wait(1.6), DROIT, Step::Idle, Step::Wait(1.8)],
    },
    // Fin de tour : un guerrier hostile s'approche et attaque, l'unité du joueur est perdue
    Scene {
        name: "attaque_hostile",
        cast: &[blesse(joueur(UnitKind::Scout, -1, 0), 12), hostile(UnitKind::Militia, 1, 0)],
        village: None,
        camp: None,
        steps: &[Step::Wait(1.0), Step::EndTurn, Step::Idle, Step::Wait(1.8)],
    },
    // Visites de villages : les trois issues possibles
    Scene {
        name: "village_commerce",
        cast: &[joueur(UnitKind::Scout, -2, 1)],
        village: Some(((1, 0), VillageEvent::Trade { gold: 30 })),
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-2, 1), GAUCHE, Step::Wait(0.4), Step::Point(1, 0), Step::Wait(1.0), DROIT, Step::Idle, Step::Wait(1.8)],
    },
    Scene {
        name: "village_habitants",
        cast: &[joueur(UnitKind::Scout, -2, 1)],
        village: Some(((1, 0), VillageEvent::JoinCity)),
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-2, 1), GAUCHE, Step::Wait(0.4), Step::Point(1, 0), Step::Wait(1.0), DROIT, Step::Idle, Step::Wait(1.8)],
    },
    Scene {
        name: "village_embuscade",
        cast: &[joueur(UnitKind::Scout, -2, 1)],
        village: Some(((1, 0), VillageEvent::Hostile { warriors: 2 })),
        camp: None,
        steps: &[Step::Wait(0.5), Step::Point(-2, 1), GAUCHE, Step::Wait(0.4), Step::Point(1, 0), Step::Wait(1.0), DROIT, Step::Idle, Step::Wait(1.8)],
    },
    // Camp vide pris : de l'or
    Scene {
        name: "camp",
        cast: &[joueur(UnitKind::Scout, -2, 1)],
        village: None,
        camp: Some((1, 0)),
        steps: &[Step::Wait(0.5), Step::Point(-2, 1), GAUCHE, Step::Wait(0.4), Step::Point(1, 0), Step::Wait(1.0), DROIT, Step::Idle, Step::Wait(1.8)],
    },
];

/// Où en est le tournage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Attente des unités de départ, puis ville et zone de tournage.
    Boot,
    /// Mise en place de la scène suivante.
    Setup,
    /// La scène s'installe (images non filmées).
    Settle,
    /// La scène se joue et est filmée.
    Play,
    /// Écriture des dernières images, puis assemblage du GIF.
    Flush,
}

#[derive(Resource)]
struct Film {
    dir: PathBuf,
    keep: bool,
    /// Scènes à tourner (indices dans `SCENES`).
    scenes: Vec<usize>,
    /// 0 : vue 2D, 1 : vue 3D.
    round: u32,
    index: usize,
    phase: Phase,
    frame: u32,
    wait: u32,
    /// Centre de la zone de tournage.
    patch: Option<Hex>,
    /// Entités de la scène (retirées à la scène suivante).
    spawned: Vec<Entity>,
    step: usize,
    step_start: f64,
    step_begun: bool,
    /// Images filmées de la scène en cours.
    recorded: u32,
    built_3d: bool,
}

impl Film {
    fn scene(&self) -> &'static Scene {
        &SCENES[self.scenes[self.index]]
    }

    fn view(&self) -> &'static str {
        if self.round == 0 { "2d" } else { "3d" }
    }

    fn frames_dir(&self) -> PathBuf {
        self.dir.join(format!("images_{}_{}", self.scene().name, self.view()))
    }
}

/// Curseur dessiné (la souris pilotée) : glisse de `from` vers la case visée.
#[derive(Resource, Default)]
struct Pointer {
    /// Position à l'écran au début du glissement et case visée.
    from: Option<Vec2>,
    to: Option<Hex>,
    start: f64,
    /// Dernière position montrée.
    shown: Option<Vec2>,
    /// Fin de l'appui d'un clic.
    pressed_until: f64,
}

#[derive(Component)]
struct PointerNode;

/// Légende de l'action en cours (« Clic droit : ordre »).
#[derive(Resource, Default)]
struct Caption {
    text: String,
    until: f64,
}

#[derive(Component)]
struct CaptionNode;

#[derive(Component)]
struct CaptionText;

pub struct FilmPlugin;

impl Plugin for FilmPlugin {
    fn build(&self, app: &mut App) {
        let Some(dir) = std::env::var_os("CIVBUILDER_CAPTURE").map(PathBuf::from) else { return };
        let Some(wanted) = std::env::var("CIVBUILDER_CAPTURE_GIF").ok() else { return };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            error!("Tournage : dossier {} impossible à créer ({e})", dir.display());
            return;
        }
        let wanted: Vec<&str> = wanted.split(',').map(str::trim).filter(|w| !w.is_empty() && *w != "1").collect();
        let scenes: Vec<usize> = (0..SCENES.len()).filter(|&k| wanted.is_empty() || wanted.contains(&SCENES[k].name)).collect();
        if scenes.is_empty() {
            let names: Vec<&str> = SCENES.iter().map(|s| s.name).collect();
            error!("Tournage : aucune scène parmi {wanted:?} (scènes : {})", names.join(", "));
            return;
        }
        info!("Tournage : {} scènes, GIF dans {}", scenes.len(), dir.display());
        let keep = std::env::var_os("CIVBUILDER_CAPTURE_KEEP").is_some();
        app.insert_resource(Film {
            dir,
            keep,
            scenes,
            round: 0,
            index: 0,
            phase: Phase::Boot,
            frame: 0,
            wait: 0,
            patch: None,
            spawned: Vec::new(),
            step: 0,
            step_start: 0.0,
            step_begun: false,
            recorded: 0,
            built_3d: false,
        })
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / IMAGES_PAR_SECONDE)))
        .init_resource::<Pointer>()
        .init_resource::<Caption>()
        .add_systems(Startup, setup_film)
        .add_systems(Update, run_film.before(crate::input::InputSet))
        .add_systems(PostUpdate, place_pointer.after(bevy::camera::CameraUpdateSystems).before(bevy::ui::UiSystems::Prepare));
    }
}

/// Flèche de souris classique : `X` contour, `.` intérieur, espace transparent.
const ARROW: [&str; 17] = [
    "X          ",
    "XX         ",
    "X.X        ",
    "X..X       ",
    "X...X      ",
    "X....X     ",
    "X.....X    ",
    "X......X   ",
    "X.......X  ",
    "X........X ",
    "X.....XXXXX",
    "X..X..X    ",
    "X.X X..X   ",
    "XX  X..X   ",
    "X    X..X  ",
    "     X..X  ",
    "      XX   ",
];

fn arrow_image() -> Image {
    let (w, h) = (ARROW[0].len() as u32, ARROW.len() as u32);
    let size = Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let mut image = Image::new_fill(size, TextureDimension::D2, &[0, 0, 0, 0], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    image.sampler = bevy::image::ImageSampler::nearest();
    if let Some(data) = image.data.as_mut() {
        for (y, row) in ARROW.iter().enumerate() {
            for (x, c) in row.bytes().enumerate() {
                let px = match c {
                    b'X' => [0, 0, 0, 255],
                    b'.' => [255, 255, 255, 255],
                    _ => continue,
                };
                let at = (y * w as usize + x) * 4;
                data[at..at + 4].copy_from_slice(&px);
            }
        }
    }
    image
}

/// Fenêtre à la taille des GIF, curseur dessiné et légende.
fn setup_film(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.resolution.set(LARGEUR as f32, HAUTEUR as f32);
    let arrow = images.add(arrow_image());
    commands.spawn((
        PointerNode,
        Node { position_type: PositionType::Absolute, width: Val::Px(ARROW[0].len() as f32 * 2.0), height: Val::Px(ARROW.len() as f32 * 2.0), ..default() },
        ImageNode::new(arrow),
        GlobalZIndex(20),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
    commands.spawn((
        CaptionNode,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(84.0),
            left: Val::Percent(50.0),
            padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
        BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.85)),
        GlobalZIndex(20),
        Visibility::Hidden,
        Pickable::IGNORE,
        children![(CaptionText, Text::new(""), TextFont::from_font_size(18.0), TextColor(Color::srgb(1.0, 0.93, 0.75)))],
    ));
}

/// Zone de tournage : terre franchissable plate (ou vallonnée) sans fleuve sur 3 cases autour,
/// loin du départ et des sites hostiles ; de préférence en prairie, plaine, savane ou steppe
/// (couleurs lisibles) sur 2 cases autour.
fn choose_patch(m: &crate::map::WorldMap, start: Hex, avoid: &[Hex]) -> Option<Hex> {
    use crate::map::Terrain;
    let good = |h: Hex, strict: bool| {
        let h = m.canonical(h);
        m.contains(h)
            && m.get(h).is_some_and(|t| t.move_cost().is_some())
            && match m.relief(h) {
                Relief::Flat => true,
                Relief::Hills => !strict,
                _ => false,
            }
            && !m.is_fleuve(h)
    };
    let open = |h: Hex| matches!(m.get(m.canonical(h)), Some(Terrain::Grassland | Terrain::Plains | Terrain::Savanna | Terrain::Mediterranean | Terrain::Steppe));
    let mut by_distance: Vec<Hex> = m.hexes().filter(|&h| m.distance(h, start) >= 8).collect();
    by_distance.sort_by_key(|&h| m.distance(h, start));
    let far = |h: Hex| avoid.iter().all(|&a| m.distance(a, h) >= 8);
    // (exigences relâchées peu à peu : relief plat seulement, rayon des terrains ouverts)
    let fits = |h: Hex, strict: bool, open_radius: Option<i32>| {
        far(h) && m.range(h, 3).all(|n| good(n, strict)) && open_radius.is_none_or(|r| m.range(h, r).all(open))
    };
    [(true, Some(2)), (false, Some(2)), (true, Some(1)), (false, Some(1)), (true, None), (false, None)]
        .into_iter()
        .find_map(|(strict, open_radius)| by_distance.iter().copied().find(|&h| fits(h, strict, open_radius)))
}

#[allow(clippy::too_many_arguments)]
fn run_film(
    mut commands: Commands,
    mut film: ResMut<Film>,
    map: Res<GameMap>,
    time: Res<Time>,
    (mut mode, mut cam, mut pilot, mut selection, mut end_turn, mut next_village): (
        ResMut<ViewMode>,
        ResMut<Cam3d>,
        ResMut<Pilot>,
        ResMut<Selection>,
        ResMut<EndTurnRequested>,
        ResMut<NextVillageEvent>,
    ),
    (mut state, mut territory, mut fog, replay): (ResMut<GameState>, ResMut<Territory>, ResMut<Fog>, Res<TurnReplay>),
    (mut pointer, mut caption, mut log): (ResMut<Pointer>, ResMut<Caption>, ResMut<crate::game::GameLog>),
    units: Query<(Entity, &Unit, &HexPos)>,
    sites: Query<(Entity, &HexPos), Or<(With<Village>, With<Camp>, With<Hostile>)>>,
    cities: Query<&HexPos, With<City>>,
    moving: Query<(), With<Motion>>,
    retired: Query<Entity, With<crate::feedback::Retired>>,
    camera: Single<(&mut Transform, &mut Projection), (With<Camera2d>, Without<crate::seam::GhostCamera>)>,
    mut exit: MessageWriter<AppExit>,
) {
    film.frame += 1;
    if film.wait > 0 {
        film.wait -= 1;
        return;
    }
    let m = &map.0;
    let now = time.elapsed_secs_f64();
    match film.phase {
        Phase::Boot => {
            if film.frame < 3 {
                return;
            }
            // Une ville au départ (pour l'issue « les habitants rejoignent la ville »)
            let Some((settler, _, start)) = units.iter().find(|(_, u, _)| u.0.kind == UnitKind::Settler) else { return };
            let start = start.0;
            let existing: Vec<Hex> = cities.iter().map(|p| p.0).collect();
            if let Err(e) = found_city(&mut commands, settler, start, m, &existing, &mut state, &mut territory) {
                warn!("Tournage : pas de ville au départ ({e})");
            }
            let avoid: Vec<Hex> = sites.iter().map(|(_, p)| p.0).collect();
            let Some(patch) = choose_patch(m, start, &avoid) else {
                error!("Tournage : pas de zone de terre pour les scènes");
                exit.write(AppExit::Success);
                return;
            };
            info!("Tournage : scènes autour de {patch:?}");
            // Zone connue du joueur (sans le mode debug : le brouillard reste celui du jeu)
            fog.explored.extend(m.range(patch, 10).map(|h| m.canonical(h)));
            film.patch = Some(patch);
            pilot.active = true;
            film.phase = Phase::Setup;
        }
        Phase::Setup => {
            let patch = film.patch.expect("zone choisie");
            let at = |(q, r): (i32, i32)| m.canonical(patch + Hex::new(q, r));
            // Scène précédente retirée (y compris les guerriers surgis d'un village)
            for e in std::mem::take(&mut film.spawned) {
                commands.entity(e).try_despawn();
            }
            for (e, _, p) in &units {
                if m.distance(p.0, patch) <= 6 {
                    commands.entity(e).try_despawn();
                }
            }
            for (e, p) in &sites {
                if m.distance(p.0, patch) <= 6 {
                    commands.entity(e).try_despawn();
                }
            }
            for e in &retired {
                commands.entity(e).try_despawn();
            }
            let scene = film.scene();
            let mut spawned = Vec::new();
            for role in scene.cast {
                let mut unit = UnitState::new(role.kind);
                unit.hp = role.hp;
                let hex = at(role.at);
                let mut e = commands.spawn((Unit(unit), HexPos(hex)));
                if role.hostile {
                    e.insert((Hostile, HostileHome(hex)));
                }
                spawned.push(e.id());
            }
            if let Some((h, event)) = scene.village {
                spawned.push(commands.spawn((Village, SiteName("Kaldvik".into()), HexPos(at(h)))).id());
                next_village.0 = Some(event);
            }
            if let Some(h) = scene.camp {
                spawned.push(commands.spawn((Camp { cooldown: crate::hostile::CAMP_SPAWN_INTERVAL }, SiteName("Camp des Loups".into()), HexPos(at(h)))).id());
            }
            film.spawned = spawned;
            selection.0 = None;
            log.clear();
            pilot.cursor = None;
            pilot.click = None;
            *pointer = Pointer::default();
            caption.until = 0.0;
            // Caméra : la scène dans le haut de l'écran, au-dessus des panneaux du bas
            mode.is_3d = film.round == 1;
            let (x, y) = patch.to_world();
            let (mut t, mut projection) = camera.into_inner();
            (t.translation.x, t.translation.y) = (x, y - DECALAGE * HAUTEUR as f32 * ECHELLE_2D);
            if let Projection::Orthographic(o) = projection.as_mut() {
                o.scale = ECHELLE_2D;
            }
            let below = m.canonical(patch + Hex::new(0, 1));
            cam.target = to_3d(below, tile_height(m, below));
            cam.distance = 330.0;
            cam.yaw = 0.0;
            film.wait = match film.round {
                0 => INSTALLATION_2D,
                _ if !film.built_3d => PREMIERE_3D,
                _ => INSTALLATION_3D,
            };
            film.built_3d |= film.round == 1;
            info!("Tournage : scène {} ({})", scene.name, film.view());
            film.phase = Phase::Settle;
        }
        Phase::Settle => {
            let dir = film.frames_dir();
            let _ = std::fs::remove_dir_all(&dir);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                error!("Tournage : dossier {} impossible à créer ({e})", dir.display());
                exit.write(AppExit::Success);
                return;
            }
            film.step = 0;
            film.step_start = now;
            film.step_begun = false;
            film.recorded = 0;
            film.phase = Phase::Play;
            run_film_play(&mut commands, &mut film, m, now, &mut pilot, &mut end_turn, &mut pointer, &mut caption, &replay, moving.is_empty());
        }
        Phase::Play => run_film_play(&mut commands, &mut film, m, now, &mut pilot, &mut end_turn, &mut pointer, &mut caption, &replay, moving.is_empty()),
        Phase::Flush => {
            let dir = film.frames_dir();
            let written = std::fs::read_dir(&dir).map(|d| d.count() as u32).unwrap_or(0);
            if written < film.recorded {
                film.wait = 5;
                return;
            }
            let gif = film.dir.join(format!("{}_{}.gif", film.scene().name, film.view()));
            assemble_gif(&dir, &gif);
            if !film.keep {
                let _ = std::fs::remove_dir_all(&dir);
            }
            film.index += 1;
            if film.index == film.scenes.len() {
                film.index = 0;
                film.round += 1;
            }
            if film.round == 2 {
                info!("Tournage : terminé");
                exit.write(AppExit::Success);
                return;
            }
            film.phase = Phase::Setup;
        }
    }
}

/// Une image de la scène : capture, puis étape en cours.
#[allow(clippy::too_many_arguments)]
fn run_film_play(
    commands: &mut Commands,
    film: &mut Film,
    m: &crate::map::WorldMap,
    now: f64,
    pilot: &mut Pilot,
    end_turn: &mut EndTurnRequested,
    pointer: &mut Pointer,
    caption: &mut Caption,
    replay: &TurnReplay,
    still: bool,
) {
    let path = film.frames_dir().join(format!("{:04}.png", film.recorded));
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    film.recorded += 1;
    let patch = film.patch.expect("zone choisie");
    let Some(&step) = film.scene().steps.get(film.step) else {
        film.phase = Phase::Flush;
        film.wait = ECRITURE;
        return;
    };
    let elapsed = now - film.step_start;
    let mut say = |text: &str| {
        caption.text = text.into();
        caption.until = now + LEGENDE;
    };
    if !film.step_begun {
        film.step_begun = true;
        match step {
            Step::Point(q, r) => {
                pointer.from = pointer.shown;
                pointer.to = Some(patch + Hex::new(q, r));
                pointer.start = now;
            }
            Step::Click(button) => {
                pilot.click = Some(button);
                pointer.pressed_until = now + APPUI;
                say(if button == MouseButton::Left { "Clic gauche : sélectionner" } else { "Clic droit : ordre (déplacer, attaquer)" });
            }
            Step::EndTurn => {
                end_turn.0 = true;
                say("Entrée : fin du tour");
            }
            Step::Wait(_) | Step::Idle => {}
        }
    }
    let done = match step {
        Step::Wait(d) => elapsed >= d,
        // (la case n'est survolée qu'à l'arrivée du curseur : aperçu du chemin ou du combat)
        Step::Point(..) => {
            if elapsed >= GLISSEMENT {
                pilot.cursor = pointer.to.map(|h| m.canonical(h));
                true
            } else {
                false
            }
        }
        Step::Click(_) => elapsed >= APPUI,
        Step::EndTurn => elapsed >= 0.25,
        Step::Idle => elapsed >= 0.2 && still && !replay.playing_at(now),
    };
    if done {
        film.step += 1;
        film.step_start = now;
        film.step_begun = false;
    }
}

/// Assemble les images d'une scène en GIF (palette calculée sur la scène).
fn assemble_gif(frames: &std::path::Path, gif: &std::path::Path) {
    let filter = "split[a][b];[a]palettegen=max_colors=200:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle";
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-framerate"])
        .arg(IMAGES_PAR_SECONDE.to_string())
        .arg("-i")
        .arg(frames.join("%04d.png"))
        .args(["-vf", filter, "-loop", "0"])
        .arg(gif)
        .status();
    match status {
        Ok(s) if s.success() => info!("Tournage : {}", gif.display()),
        Ok(s) => error!("Tournage : ffmpeg a échoué ({s}) pour {}", gif.display()),
        Err(e) => error!("Tournage : ffmpeg introuvable ({e}) ; images gardées dans {}", frames.display()),
    }
}

/// Place le curseur dessiné (projection de la case visée, glissement) et la légende.
#[allow(clippy::too_many_arguments)]
fn place_pointer(
    time: Res<Time>,
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    mut pointer: ResMut<Pointer>,
    caption: Res<Caption>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut node: Single<(&mut Node, &mut UiTransform, &mut Visibility), (With<PointerNode>, Without<CaptionNode>)>,
    mut caption_node: Single<&mut Visibility, (With<CaptionNode>, Without<PointerNode>)>,
    mut caption_text: Single<&mut Text, With<CaptionText>>,
) {
    let now = time.elapsed_secs_f64();
    // Légende
    let shown = now < caption.until;
    let vis = if shown { Visibility::Inherited } else { Visibility::Hidden };
    if **caption_node != vis {
        **caption_node = vis;
    }
    if shown && caption_text.0 != caption.text {
        caption_text.0.clone_from(&caption.text);
    }
    // Curseur
    let (node, ui, visibility) = &mut *node;
    let camera = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active);
    let target = pointer.to.zip(camera).and_then(|(hex, (camera, transform, _))| {
        let world = if mode.is_3d {
            to_3d(hex, shown_height(&map, &fog, hex) + 4.0)
        } else {
            let (x, y) = hex.to_world();
            Vec3::new(x, y, 0.0)
        };
        let world = crate::labels::near_camera(&map, world, transform.translation.x);
        camera.world_to_viewport(&GlobalTransform::from(*transform), world).ok()
    });
    let Some(target) = target else {
        **visibility = Visibility::Hidden;
        return;
    };
    // (première apparition : arrive d'en bas à droite)
    let from = pointer.from.unwrap_or(target + Vec2::new(90.0, 110.0));
    let u = ((now - pointer.start) / GLISSEMENT).clamp(0.0, 1.0) as f32;
    let u = u * u * (3.0 - 2.0 * u);
    let at = from.lerp(target, u);
    pointer.shown = Some(at);
    node.left = Val::Px(at.x.round());
    node.top = Val::Px(at.y.round());
    ui.scale = Vec2::splat(if now < pointer.pressed_until { 0.8 } else { 1.0 });
    **visibility = Visibility::Inherited;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rayon((q, r): (i32, i32)) -> i32 {
        Hex::new(0, 0).distance(Hex::new(q, r))
    }

    #[test]
    fn les_scenes_tiennent_dans_la_zone_de_tournage() {
        // La zone choisie est franchissable sur 3 cases autour de son centre
        for scene in SCENES {
            let points = scene.steps.iter().filter_map(|s| if let Step::Point(q, r) = *s { Some((q, r)) } else { None });
            let places = scene.cast.iter().map(|r| r.at).chain(scene.village.map(|v| v.0)).chain(scene.camp).chain(points);
            for at in places {
                assert!(rayon(at) <= 3, "{} : {at:?} hors de la zone", scene.name);
            }
            // Pas deux unités sur une case
            for (k, a) in scene.cast.iter().enumerate() {
                assert!(scene.cast[k + 1..].iter().all(|b| b.at != a.at), "{} : deux unités en {:?}", scene.name, a.at);
            }
        }
    }

    #[test]
    fn chaque_scene_a_un_nom_unique_et_sa_place_dans_la_doc() {
        let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/ANIMATIONS.md")).unwrap();
        for (k, scene) in SCENES.iter().enumerate() {
            assert!(SCENES[k + 1..].iter().all(|s| s.name != scene.name), "scène {} en double", scene.name);
            for view in ["2d", "3d"] {
                assert!(doc.contains(&format!("animations/{}_{view}.gif", scene.name)), "{}_{view}.gif absent de docs/ANIMATIONS.md", scene.name);
            }
        }
    }

    #[test]
    fn la_fleche_du_curseur_est_rectangulaire() {
        assert!(ARROW.iter().all(|row| row.len() == ARROW[0].len()));
        let image = arrow_image();
        assert_eq!(image.data.as_ref().map(Vec::len), Some(ARROW[0].len() * ARROW.len() * 4));
    }
}
