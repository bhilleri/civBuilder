//! Écran de nouvelle partie (tâche 42, maquette v4 validée le 11/11/2026).
//!
//! Il s'ouvre au lancement normal du jeu (sans graine, sauvegarde ni configuration imposées :
//! `mode_accueil`). On y règle le monde (code, taille ; terres, climat et relief à venir) et la
//! partie (barbares ; IA, difficulté, rythme, hégémonie et premiers tours guidés à venir), on
//! retrouve ses configurations (enregistrées ou jouées, favorites) et on lance : le jeu se relance
//! avec la configuration (`CIVBUILDER_CONFIG`). La carte reste cachée tant qu'on ne la révèle pas
//! (elle gâcherait la découverte) ; la révéler génère le monde en tâche de fond.
//!
//! L'interface est reconstruite à chaque changement (un clic) : simple et sans état caché ; le
//! texte des champs est recopié dans l'état à chaque frappe pour survivre aux reconstructions.
//!
//! Vérification sans écran : `CIVBUILDER_ACCUEIL_CAPTURE=<dossier>` enregistre l'écran, la
//! fenêtre d'enregistrement et la question au chargement en PNG, puis ferme.

use crate::config_partie::{Barbares, Bibliotheque, ConfigPartie, Cran, Difficulte, Origine, Rythme, Taille, disponible, part_des_terres};
use crate::hex::Hex;
use crate::map::WorldMap;
use crate::pointer::UiPointer;
use bevy::asset::RenderAssetUsages;
use bevy::input::keyboard::Key;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::{EditableText, TextCursorStyle, TextEdit};
use bevy::ui_widgets::TextInput;
use std::sync::{Arc, Mutex};

const FOND: Color = Color::srgb(0.051, 0.063, 0.094);
const PANNEAU: Color = Color::srgb(0.082, 0.102, 0.141);
const PANNEAU2: Color = Color::srgb(0.11, 0.133, 0.192);
const CHOIX: Color = Color::srgb(0.137, 0.165, 0.22);
const BLEU: Color = Color::srgb(0.184, 0.384, 0.839);
const GRIS_ON: Color = Color::srgb(0.227, 0.259, 0.322);
const TEXTE: Color = Color::srgb(0.902, 0.91, 0.933);
const LIBELLE: Color = Color::srgb(0.812, 0.831, 0.871);
const AIDE: Color = Color::srgb(0.639, 0.671, 0.737);
const GRISE: Color = Color::srgb(0.486, 0.518, 0.588);
const OR: Color = Color::srgb(0.886, 0.722, 0.29);
const ERREUR: Color = Color::srgb(0.94, 0.45, 0.42);

/// L'écran de nouvelle partie s'ouvre-t-il ? Oui, sauf si une graine, une sauvegarde, une
/// configuration ou un mode de test est imposé.
pub fn mode_accueil() -> bool {
    if std::env::var_os("CIVBUILDER_ACCUEIL_CAPTURE").is_some() {
        return true;
    }
    ["CIVBUILDER_SEED", "CIVBUILDER_CHARGER", "CIVBUILDER_CONFIG", "CIVBUILDER_CAPTURE", "CIVBUILDER_SCRIPT", "CIVBUILDER_PILOTE", "CIVBUILDER_FILM"]
        .iter()
        .all(|v| std::env::var_os(v).is_none())
}

pub struct AccueilPlugin;

impl Plugin for AccueilPlugin {
    fn build(&self, app: &mut App) {
        let mut config = ConfigPartie::avec_graine(rand::random::<u32>() % 1_000_000);
        // (la capture de vérification montre une configuration fixe)
        if std::env::var_os("CIVBUILDER_ACCUEIL_CAPTURE").is_some() {
            config = ConfigPartie::default();
        }
        app.insert_resource(Accueil { code: config.code(), config, biblio: Bibliotheque::lire(), ..default() })
            .init_resource::<Apercu>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((Camera2d, IsDefaultUiCamera, bevy::ui::picking_backend::UiPickingCamera));
            })
            .add_systems(Update, (clics, clavier, champs, suivre_apercu, reconstruire, capture_de_verification).chain());
    }
}

/// Filtre de « Mes configurations ».
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Filtre {
    #[default]
    Toutes,
    Favorites,
    Jouees,
}

/// Fenêtre ouverte par-dessus l'écran.
#[derive(Clone, Debug, PartialEq)]
enum Dialogue {
    /// Enregistrer la configuration (ou modifier l'entrée `Some(i)`).
    Enregistrer { modifie: Option<usize> },
    /// Charger l'entrée i : même graine ou nouvelle ?
    Charger { entree: usize, meme_monde: bool },
    /// Effacer l'entrée i.
    Effacer { entree: usize },
}

/// État de l'écran.
#[derive(Resource, Default)]
struct Accueil {
    config: ConfigPartie,
    biblio: Bibliotheque,
    filtre: Filtre,
    dialogue: Option<Dialogue>,
    /// Texte du champ du code (appliqué par Entrée).
    code: String,
    erreur_code: Option<String>,
    /// Champs de la fenêtre d'enregistrement.
    nom: String,
    commentaire: String,
    favorite: bool,
    /// Message bref (« Configuration enregistrée »).
    message: Option<String>,
    /// L'interface est à reconstruire.
    sale: bool,
    /// Appliquer le code du champ à la prochaine image (après un « Coller »).
    appliquer_code: bool,
}

/// Aperçu du monde : caché, en calcul (tâche de fond), prêt, ou en erreur.
#[derive(Resource, Default)]
struct Apercu {
    /// Graine et taille de l'aperçu demandé.
    pour: Option<(u32, Taille)>,
    /// Résultat de la tâche de fond.
    resultat: Arc<Mutex<Option<Result<WorldMap, String>>>>,
    debut: Option<std::time::Instant>,
    duree: Option<f32>,
    image: Option<Handle<Image>>,
    resume: Option<String>,
}

/// Action d'un élément cliquable.
#[derive(Component, Clone, Debug, PartialEq)]
enum Action {
    Taille(Taille),
    Barbares(Barbares),
    Hasard,
    Copier,
    Coller,
    Defaut,
    OuvrirEnregistrer,
    Reveler,
    Lancer,
    Quitter,
    Filtre(Filtre),
    Charger(usize),
    Favori(usize),
    Modifier(usize),
    Effacer(usize),
    Annuler,
    ValiderEnregistrer,
    BasculerFavorite,
    MemeMonde(bool),
    ValiderCharger,
    ValiderEffacer,
}

/// Champ de saisie.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Champ {
    Code,
    Nom,
    Commentaire,
}

/// Racine de l'interface (reconstruite à chaque changement).
#[derive(Component)]
struct Racine;

/// Texte du temps écoulé pendant le calcul de l'aperçu.
#[derive(Component)]
struct Attente;

// ---------------------------------------------------------------- Construction

fn texte(s: impl Into<String>, taille: f32, couleur: Color) -> impl Bundle {
    (Text::new(s.into()), TextFont::from_font_size(taille), TextColor(couleur))
}

fn arrondi(px: f32) -> BorderRadius {
    BorderRadius::all(Val::Px(px))
}

/// Bouton ; sans action (`None`), il est grisé.
fn bouton(p: &mut ChildSpawnerCommands, libelle: &str, action: Option<Action>, fond: Color) {
    let couleur = if action.is_some() { TEXTE } else { GRISE };
    let mut e = p.spawn((
        Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, border_radius: arrondi(8.0), flex_shrink: 0.0, ..default() },
        BackgroundColor(fond),
    ));
    e.with_children(|b| {
        b.spawn((texte(libelle, 15.0, couleur), TextLayout::no_wrap()));
    });
    if let Some(a) = action {
        e.insert((a, UiPointer::default()));
    }
}

/// Choix segmenté : (libellé, sous-libellé, choisi, action). Grisé : aucune action, mention « à venir ».
fn segments(p: &mut ChildSpawnerCommands, choix: Vec<(String, Option<String>, bool, Option<Action>)>, grise: bool) {
    p.spawn((Node { column_gap: Val::Px(3.0), padding: UiRect::all(Val::Px(3.0)), border_radius: arrondi(8.0), ..default() }, BackgroundColor(CHOIX))).with_children(|s| {
        for (libelle, sous, choisi, action) in choix {
            let fond = match (choisi, grise) {
                (true, false) => BLEU,
                (true, true) => GRIS_ON,
                _ => Color::NONE,
            };
            let couleur = if grise && !choisi { GRISE } else if choisi { Color::WHITE } else { LIBELLE };
            let mut e = s.spawn((
                Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(4.0), Val::Px(6.0)), border_radius: arrondi(6.0), ..default() },
                BackgroundColor(fond),
            ));
            e.with_children(|c| {
                c.spawn(texte(libelle, 14.0, couleur));
                if let Some(sous) = sous {
                    c.spawn(texte(sous, 12.0, if couleur == Color::WHITE { Color::srgb(0.86, 0.9, 1.0) } else { GRISE }));
                }
            });
            if let (Some(a), false) = (action, grise) {
                e.insert((a, UiPointer::default()));
            }
        }
    });
}

/// Libellé d'un réglage, avec la mention « à venir » s'il n'est pas encore pris en compte.
fn libelle(p: &mut ChildSpawnerCommands, s: &str, a_venir: bool) {
    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }).with_children(|l| {
        l.spawn(texte(s, 14.0, if a_venir { GRISE } else { LIBELLE }));
        if a_venir {
            l.spawn((Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), border_radius: arrondi(5.0), ..default() }, BackgroundColor(Color::srgb(0.17, 0.18, 0.23))))
                .with_children(|b| {
                    b.spawn(texte("à venir", 12.0, Color::srgb(0.79, 0.70, 0.48)));
                });
        }
    });
}

/// Bascule Oui/Non (grisée : sans action).
fn bascule(p: &mut ChildSpawnerCommands, titre: &str, sous: Option<&str>, oui: bool, action: Option<Action>) {
    let grise = action.is_none();
    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }).with_children(|l| {
        l.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
            t.spawn(texte(titre, 14.0, if grise { GRISE } else { LIBELLE }));
            if let Some(sous) = sous {
                t.spawn(texte(sous, 13.0, if grise { GRISE } else { AIDE }));
            }
        });
        let mut e = l.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() });
        e.with_children(|b| {
            b.spawn(texte(if oui { "Oui" } else { "Non" }, 14.0, if grise { GRISE } else { LIBELLE }));
            let fond = match (oui, grise) {
                (true, false) => BLEU,
                (true, true) => GRIS_ON,
                _ => CHOIX,
            };
            b.spawn((Node { width: Val::Px(42.0), height: Val::Px(24.0), border_radius: arrondi(12.0), justify_content: if oui { JustifyContent::FlexEnd } else { JustifyContent::FlexStart }, padding: UiRect::all(Val::Px(3.0)), ..default() }, BackgroundColor(fond)))
                .with_children(|k| {
                    k.spawn((Node { width: Val::Px(18.0), height: Val::Px(18.0), border_radius: arrondi(9.0), ..default() }, BackgroundColor(if grise { GRISE } else { Color::WHITE })));
                });
        });
        if let Some(a) = action {
            e.insert((a, UiPointer::default()));
        }
    });
}

/// Champ de saisie (texte initial `valeur`).
fn champ(p: &mut ChildSpawnerCommands, quoi: Champ, valeur: &str, hauteur: f32, max: usize, police: f32) {
    p.spawn((
        Node { flex_grow: 1.0, min_width: Val::Px(if quoi == Champ::Code { 160.0 } else { 0.0 }), min_height: Val::Px(hauteur), padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: arrondi(8.0), ..default() },
        BackgroundColor(Color::srgb(0.039, 0.051, 0.078)),
        BorderColor::all(Color::srgb(0.227, 0.263, 0.337)),
    ))
    .with_children(|c| {
        let mut editable = EditableText { max_characters: Some(max), ..default() };
        editable.editor_mut().set_text(valeur);
        c.spawn((
            quoi,
            TextInput,
            UiPointer::default(),
            bevy::input_focus::tab_navigation::TabIndex::default(),
            editable,
            TextFont::from_font_size(police),
            TextColor(TEXTE),
            TextCursorStyle { color: Color::WHITE, selection_color: Color::srgb(0.20, 0.40, 0.75), ..default() },
            Node { width: Val::Percent(100.0), ..default() },
        ));
    });
}

fn panneau(p: &mut ChildSpawnerCommands, grandir: bool, contenu: impl FnOnce(&mut ChildSpawnerCommands)) {
    p.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(12.0),
            padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
            border_radius: arrondi(10.0),
            flex_grow: if grandir { 1.0 } else { 0.0 },
            min_height: Val::Px(0.0),
            ..default()
        },
        BackgroundColor(PANNEAU),
    ))
    .with_children(contenu);
}

fn titre(p: &mut ChildSpawnerCommands, s: &str) {
    p.spawn(texte(s, 19.0, Color::WHITE));
}

fn aide(p: &mut ChildSpawnerCommands, s: &str) {
    p.spawn(texte(s, 14.0, AIDE));
}

/// Libellés d'un réglage à trois crans (terres, climat, relief).
fn crans(quoi: &str) -> [(String, Option<String>); 3] {
    match quoi {
        "terres" => [Cran::Moins, Cran::CommeLaTerre, Cran::Plus].map(|c| {
            let nom = match c {
                Cran::Moins => "Peu",
                Cran::CommeLaTerre => "Comme la Terre",
                Cran::Plus => "Beaucoup",
            };
            (nom.to_string(), Some(format!("≈ {} %", part_des_terres(c))))
        }),
        "climat" => [("Plus froid", "−4 °C"), ("Comme la Terre", "15 °C en moyenne"), ("Plus chaud", "+4 °C")].map(|(a, b)| (a.to_string(), Some(b.to_string()))),
        _ => ["Doux", "Comme la Terre", "Accidenté"].map(|a| (a.to_string(), None)),
    }
}

fn construire(commands: &mut Commands, a: &Accueil, apercu: &Apercu) {
    let c = &a.config;
    commands
        .spawn((Racine, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), padding: UiRect::axes(Val::Px(36.0), Val::Px(24.0)), ..default() }, BackgroundColor(FOND)))
        .with_children(|r| {
            // En-tête
            r.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }).with_children(|t| {
                t.spawn(texte("Nouvelle partie", 28.0, Color::WHITE));
                t.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|b| {
                    let defaut = ConfigPartie { graine: c.graine, ..Default::default() };
                    let etat = if *c == defaut { "Configuration par défaut" } else { "Configuration personnalisée" };
                    b.spawn(texte(a.message.clone().unwrap_or_else(|| etat.to_string()), 14.0, AIDE));
                    bouton(b, "↺ Rétablir par défaut", Some(Action::Defaut), CHOIX);
                    bouton(b, "Enregistrer la configuration…", Some(Action::OuvrirEnregistrer), CHOIX);
                });
            });
            // Trois colonnes
            r.spawn(Node { flex_grow: 1.0, column_gap: Val::Px(20.0), min_height: Val::Px(0.0), ..default() }).with_children(|cols| {
                // Monde
                cols.spawn(Node { width: Val::Percent(31.0), min_width: Val::Px(400.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), ..default() }).with_children(|col| {
                    panneau(col, false, |p| {
                        titre(p, "Monde");
                        libelle(p, "Code du monde", false);
                        p.spawn(Node { column_gap: Val::Px(6.0), ..default() }).with_children(|l| {
                            champ(l, Champ::Code, &a.code, 38.0, 32, 15.0);
                            bouton(l, "Hasard", Some(Action::Hasard), CHOIX);
                            bouton(l, "Copier", Some(Action::Copier), CHOIX);
                            bouton(l, "Coller", Some(Action::Coller), CHOIX);
                        });
                        match &a.erreur_code {
                            Some(e) => {
                                p.spawn(texte(format!("⚠ {e}"), 14.0, ERREUR));
                            }
                            None => aide(p, &format!("Graine {} et réglages ci-dessous. Collez un code reçu, puis Entrée, pour jouer le même monde.", c.graine)),
                        }
                        libelle(p, "Taille", !disponible("taille"));
                        segments(
                            p,
                            Taille::TOUTES
                                .iter()
                                .map(|&t| {
                                    let (w, h) = t.cases();
                                    let sous = if t.experimentale() { "expérimentale".to_string() } else { format!("{w} × {h} · ≈ {} s", t.duree_generation()) };
                                    (t.nom().to_string(), Some(sous), c.taille == t, Some(Action::Taille(t)))
                                })
                                .collect(),
                            false,
                        );
                        for (quoi, nom, valeur) in [("terres", "Terres émergées", c.terres), ("climat", "Climat", c.climat), ("relief", "Relief", c.relief)] {
                            libelle(p, nom, !disponible(quoi));
                            let options = crans(quoi);
                            segments(p, Cran::TOUS.iter().zip(options).map(|(&k, (l, s))| (l, s, k == valeur, None)).collect(), true);
                        }
                    });
                });
                // Aperçu et configurations
                cols.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), min_width: Val::Px(0.0), ..default() }).with_children(|col| {
                    panneau(col, true, |p| {
                        p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }).with_children(|l| {
                            titre(l, &format!("Le monde {}", c.graine));
                            let etat = match (&apercu.image, apercu.debut, apercu.duree) {
                                (Some(_), _, Some(d)) if apercu.pour == Some((c.graine, c.taille)) => format!("Monde prêt · calculé en {d:.0} s"),
                                (_, Some(_), _) => "Calcul du monde…".to_string(),
                                _ => format!("Le monde sera généré au lancement (≈ {} s)", c.taille.duree_generation()),
                            };
                            l.spawn((texte(etat, 14.0, LIBELLE), Attente));
                        });
                        let image_a_jour = apercu.image.is_some() && apercu.pour == Some((c.graine, c.taille));
                        p.spawn((Node { flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: Val::Px(14.0), border_radius: arrondi(8.0), ..default() }, BackgroundColor(Color::srgb(0.07, 0.09, 0.13))))
                            .with_children(|z| {
                                if let (true, Some(img)) = (image_a_jour, &apercu.image) {
                                    z.spawn((ImageNode::new(img.clone()), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }));
                                } else if apercu.debut.is_some() {
                                    z.spawn(texte("Génération du monde en cours…", 20.0, TEXTE));
                                    aide(z, "Elle continue si vous lancez la partie.");
                                } else {
                                    z.spawn(texte("Carte cachée", 22.0, TEXTE));
                                    z.spawn((texte("Votre peuple découvrira ce monde en l'explorant. L'aperçu montrerait toute la planète avant la partie.", 14.0, AIDE), Node { max_width: Val::Px(520.0), ..default() }, TextLayout::justify(Justify::Center)));
                                    bouton(z, "Révéler la carte (gâche la découverte)", Some(Action::Reveler), CHOIX);
                                }
                            });
                        if let (true, Some(resume)) = (image_a_jour, &apercu.resume) {
                            aide(p, resume);
                        }
                    });
                    panneau(col, false, |p| {
                        p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }).with_children(|l| {
                            titre(l, "Mes configurations");
                            l.spawn(Node { width: Val::Px(360.0), ..default() }).with_children(|s| {
                                segments(
                                    s,
                                    [(Filtre::Toutes, "Toutes"), (Filtre::Favorites, "Favorites"), (Filtre::Jouees, "Jouées")]
                                        .into_iter()
                                        .map(|(f, n)| (n.to_string(), None, a.filtre == f, Some(Action::Filtre(f))))
                                        .collect(),
                                    false,
                                );
                            });
                        });
                        let visibles: Vec<(usize, &crate::config_partie::Entree)> = a
                            .biblio
                            .entrees
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| match a.filtre {
                                Filtre::Toutes => true,
                                Filtre::Favorites => e.favorite,
                                Filtre::Jouees => matches!(e.origine, Origine::Jouee { .. }),
                            })
                            .take(3)
                            .collect();
                        if visibles.is_empty() {
                            aide(p, "Aucune configuration : enregistrez celle-ci, ou lancez une partie, elle s'ajoutera ici.");
                        }
                        for (i, e) in visibles {
                            p.spawn((Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)), border_radius: arrondi(8.0), ..default() }, BackgroundColor(PANNEAU2))).with_children(|l| {
                                l.spawn((texte(if e.favorite { "★" } else { "☆" }, 18.0, if e.favorite { OR } else { AIDE }), Action::Favori(i), UiPointer::default()));
                                l.spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, min_width: Val::Px(0.0), ..default() }, Action::Charger(i), UiPointer::default())).with_children(|t| {
                                    t.spawn(texte(e.nom.clone(), 14.0, TEXTE));
                                    t.spawn(texte(e.config.resume(), 13.0, AIDE));
                                    if !e.commentaire.is_empty() {
                                        t.spawn(texte(e.commentaire.clone(), 13.0, LIBELLE));
                                    }
                                });
                                let origine = match (&e.origine, e.parties) {
                                    (Origine::Enregistree, 0) => "aucune partie".to_string(),
                                    (Origine::Enregistree, 1) => "1 partie".to_string(),
                                    (Origine::Enregistree, n) => format!("{n} parties"),
                                    (Origine::Jouee { tour }, _) => format!("jouée · tour {tour}"),
                                };
                                l.spawn((texte(origine, 13.0, AIDE), TextLayout::no_wrap()));
                                bouton(l, "Modifier", Some(Action::Modifier(i)), CHOIX);
                                bouton(l, "Effacer", Some(Action::Effacer(i)), CHOIX);
                            });
                        }
                    });
                });
                // Partie
                cols.spawn(Node { width: Val::Percent(27.0), min_width: Val::Px(360.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
                    panneau(col, true, |p| {
                        titre(p, "Partie");
                        let (conseil, max) = c.taille.peuples_ia();
                        libelle(p, "Peuples dirigés par l'ordinateur (IA)", !disponible("ia"));
                        p.spawn(texte(format!("{} peuples (conseillé : {conseil}, au plus {max})", c.peuples_ia), 16.0, GRISE));
                        libelle(p, "Difficulté des IA", !disponible("difficulte"));
                        segments(
                            p,
                            Difficulte::TOUTES.iter().map(|&d| (d.nom().to_string(), (d == Difficulte::Expert).then(|| "bonus affiché".to_string()), c.difficulte == d, None)).collect(),
                            true,
                        );
                        libelle(p, "Rythme", !disponible("rythme"));
                        segments(
                            p,
                            [(Rythme::UnTourParMois, "1 tour par mois"), (Rythme::DeuxToursParMois, "2 tours par mois")].into_iter().map(|(r, n)| (n.to_string(), None, c.rythme == r, None)).collect(),
                            true,
                        );
                        libelle(p, "Barbares", !disponible("barbares"));
                        segments(p, Barbares::TOUS.iter().map(|&b| (b.nom().to_string(), None, c.barbares == b, Some(Action::Barbares(b)))).collect(), false);
                        aide(p, &format!("{} : un camp pour environ {} cases de terre.", c.barbares.nom(), c.barbares.terres_par_camp()));
                        bascule(p, "Victoire par hégémonie (à venir)", Some("80 % de la population mondiale"), c.hegemonie, None);
                        bascule(p, "Premiers tours guidés (à venir)", None, c.tours_guides, None);
                        p.spawn(Node { flex_grow: 1.0, ..default() });
                        p.spawn((Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(12.0)), border_radius: arrondi(8.0), ..default() }, BackgroundColor(PANNEAU2))).with_children(|n| {
                            n.spawn(texte("Votre peuple commence nomade. Son nom, sa langue et ses valeurs naîtront de la région où il s'installera.", 14.0, LIBELLE));
                        });
                    });
                });
            });
            // Pied
            r.spawn((Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)), border_radius: arrondi(10.0), ..default() }, BackgroundColor(PANNEAU))).with_children(|f| {
                bouton(f, "Quitter (Échap)", Some(Action::Quitter), CHOIX);
                f.spawn(texte("500 tours · Néolithique → Moyen Âge · Victoire : meilleur score d'héritage au tour 500, puis partie libre", 15.0, LIBELLE));
                bouton(f, "Lancer la partie (Entrée)", Some(Action::Lancer), BLEU);
            });
            // Fenêtre par-dessus
            if let Some(d) = &a.dialogue {
                r.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.72)),
                    GlobalZIndex(10),
                ))
                .with_children(|v| {
                    v.spawn((
                        Node { width: Val::Px(620.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(16.0), padding: UiRect::axes(Val::Px(28.0), Val::Px(24.0)), border: UiRect::all(Val::Px(1.0)), border_radius: arrondi(12.0), ..default() },
                        BackgroundColor(Color::srgb(0.09, 0.114, 0.161)),
                        BorderColor::all(Color::srgb(0.184, 0.22, 0.314)),
                    ))
                    .with_children(|w| dialogue(w, a, d));
                });
            }
        });
}

fn dialogue(w: &mut ChildSpawnerCommands, a: &Accueil, d: &Dialogue) {
    let boutons = |w: &mut ChildSpawnerCommands, valider: &str, action: Action| {
        w.spawn(Node { justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(10.0), ..default() }).with_children(|b| {
            bouton(b, "Annuler (Échap)", Some(Action::Annuler), CHOIX);
            bouton(b, valider, Some(action), BLEU);
        });
    };
    match d {
        Dialogue::Enregistrer { modifie } => {
            w.spawn(texte(if modifie.is_some() { "Modifier la configuration" } else { "Enregistrer la configuration" }, 22.0, Color::WHITE));
            libelle(w, "Nom", false);
            w.spawn(Node::default()).with_children(|l| champ(l, Champ::Nom, &a.nom, 38.0, 60, 15.0));
            libelle(w, "Commentaire", false);
            w.spawn(Node::default()).with_children(|l| champ(l, Champ::Commentaire, &a.commentaire, 80.0, 200, 15.0));
            w.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(12.0), ..default() }).with_children(|l| {
                l.spawn((texte(if a.favorite { "★ Favorite" } else { "☆ Ajouter aux favorites" }, 15.0, if a.favorite { OR } else { LIBELLE }), Action::BasculerFavorite, UiPointer::default()));
                l.spawn(texte(format!("{} (graine gardée pour rejouer le même monde)", a.config.resume()), 13.0, AIDE));
            });
            boutons(w, "Enregistrer (Entrée)", Action::ValiderEnregistrer);
        }
        Dialogue::Charger { entree, meme_monde } => {
            let Some(e) = a.biblio.entrees.get(*entree) else { return };
            w.spawn(texte(format!("Charger « {} »", e.nom), 22.0, Color::WHITE));
            aide(w, &e.config.resume());
            w.spawn(texte("Quel monde voulez-vous jouer ?", 15.0, LIBELLE));
            w.spawn(Node { column_gap: Val::Px(12.0), ..default() }).with_children(|l| {
                for (meme, titre_, sous) in [(true, "Le même monde", format!("Graine {} : la même planète.", e.config.graine)), (false, "Un nouveau monde", "Une nouvelle graine, avec les mêmes réglages.".to_string())] {
                    let on = *meme_monde == meme;
                    l.spawn((
                        Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(16.0), Val::Px(14.0)), border: UiRect::all(Val::Px(2.0)), border_radius: arrondi(10.0), ..default() },
                        BackgroundColor(if on { Color::srgb(0.114, 0.165, 0.278) } else { CHOIX }),
                        BorderColor::all(if on { BLEU } else { Color::NONE }),
                        Action::MemeMonde(meme),
                        UiPointer::default(),
                    ))
                    .with_children(|c| {
                        c.spawn(texte(titre_, 15.0, TEXTE));
                        c.spawn(texte(sous, 13.0, AIDE));
                    });
                }
            });
            boutons(w, "Charger (Entrée)", Action::ValiderCharger);
        }
        Dialogue::Effacer { entree } => {
            let Some(e) = a.biblio.entrees.get(*entree) else { return };
            w.spawn(texte(format!("Effacer « {} » ?", e.nom), 22.0, Color::WHITE));
            aide(w, "La configuration disparaît de la liste. Les sauvegardes des parties ne sont pas touchées.");
            boutons(w, "Effacer (Entrée)", Action::ValiderEffacer);
        }
    }
}

// ---------------------------------------------------------------- Comportement

fn reconstruire(mut commands: Commands, mut a: ResMut<Accueil>, apercu: Res<Apercu>, racines: Query<Entity, With<Racine>>, mut focus: ResMut<InputFocus>) {
    if !a.sale && !racines.is_empty() {
        return;
    }
    a.sale = false;
    for e in &racines {
        commands.entity(e).despawn();
    }
    focus.clear();
    construire(&mut commands, &a, &apercu);
}

/// Applique l'action d'un élément cliqué.
#[allow(clippy::too_many_arguments)]
fn clics(
    mut a: ResMut<Accueil>,
    mut apercu: ResMut<Apercu>,
    elements: Query<(Ref<UiPointer>, &Action)>,
    mut champs: Query<(Entity, &Champ, &mut EditableText)>,
    mut focus: ResMut<InputFocus>,
    mut clipboard: ResMut<bevy::clipboard::Clipboard>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(action) = elements.iter().find(|(p, _)| p.is_changed() && **p == UiPointer::Pressed).map(|(_, a)| a.clone()) else { return };
    agir(&mut a, &mut apercu, action, &mut champs, &mut focus, &mut clipboard, &mut exit);
}

#[allow(clippy::too_many_arguments)]
fn agir(
    a: &mut Accueil,
    apercu: &mut Apercu,
    action: Action,
    champs: &mut Query<(Entity, &Champ, &mut EditableText)>,
    focus: &mut InputFocus,
    clipboard: &mut bevy::clipboard::Clipboard,
    exit: &mut MessageWriter<AppExit>,
) {
    a.message = None;
    a.sale = true;
    match action {
        Action::Taille(t) => {
            a.config.taille = t;
            a.config.peuples_ia = t.peuples_ia().0;
        }
        Action::Barbares(b) => a.config.barbares = b,
        Action::Hasard => {
            a.config.graine = rand::random::<u32>() % 1_000_000;
            a.erreur_code = None;
        }
        Action::Copier => {
            let code = a.config.code();
            a.message = Some(match clipboard.set_text(code.clone()) {
                Ok(()) => format!("Code {code} copié"),
                Err(e) => format!("Copie impossible : {e:?}"),
            });
        }
        Action::Coller => {
            // Colle dans le champ (tout remplacé), puis l'applique à l'image suivante
            if let Some((entity, _, mut editable)) = champs.iter_mut().find(|(_, c, _)| **c == Champ::Code) {
                editable.queue_edit(TextEdit::SelectAll);
                editable.queue_edit(TextEdit::Paste);
                focus.set(entity, FocusCause::Navigated);
            }
            a.appliquer_code = true;
            a.sale = false;
            return;
        }
        Action::Defaut => {
            a.config = ConfigPartie { graine: a.config.graine, ..Default::default() };
            a.message = Some("Réglages par défaut rétablis (graine gardée)".into());
        }
        Action::OuvrirEnregistrer => {
            a.nom = format!("Monde {}", a.config.graine);
            a.commentaire.clear();
            a.favorite = false;
            a.dialogue = Some(Dialogue::Enregistrer { modifie: None });
        }
        Action::Reveler => lancer_apercu(apercu, &a.config),
        Action::Lancer => lancer_partie(&a.config, exit, &mut a.message),
        Action::Quitter => {
            exit.write(AppExit::Success);
        }
        Action::Filtre(f) => a.filtre = f,
        Action::Charger(i) => a.dialogue = Some(Dialogue::Charger { entree: i, meme_monde: true }),
        Action::Favori(i) => {
            if let Some(e) = a.biblio.entrees.get_mut(i) {
                e.favorite = !e.favorite;
            }
            ecrire(a);
        }
        Action::Modifier(i) => {
            if let Some(e) = a.biblio.entrees.get(i) {
                (a.nom, a.commentaire, a.favorite) = (e.nom.clone(), e.commentaire.clone(), e.favorite);
                a.dialogue = Some(Dialogue::Enregistrer { modifie: Some(i) });
            }
        }
        Action::Effacer(i) => a.dialogue = Some(Dialogue::Effacer { entree: i }),
        Action::Annuler => a.dialogue = None,
        Action::BasculerFavorite => a.favorite = !a.favorite,
        Action::ValiderEnregistrer => valider_enregistrer(a),
        Action::MemeMonde(m) => {
            if let Some(Dialogue::Charger { meme_monde, .. }) = a.dialogue.as_mut() {
                *meme_monde = m;
            }
        }
        Action::ValiderCharger => {
            if let Some(Dialogue::Charger { entree, meme_monde }) = a.dialogue.clone()
                && let Some(e) = a.biblio.entrees.get(entree)
            {
                a.config = e.config.clone();
                if !meme_monde {
                    a.config.graine = rand::random::<u32>() % 1_000_000;
                }
                a.message = Some(format!("« {} » chargée", e.nom));
            }
            a.dialogue = None;
        }
        Action::ValiderEffacer => {
            if let Some(Dialogue::Effacer { entree }) = a.dialogue
                && entree < a.biblio.entrees.len()
            {
                let e = a.biblio.entrees.remove(entree);
                a.message = Some(format!("« {} » effacée", e.nom));
                ecrire(a);
            }
            a.dialogue = None;
        }
    }
    a.code = a.config.code();
}

fn valider_enregistrer(a: &mut Accueil) {
    let nom = a.nom.trim().to_string();
    if nom.is_empty() {
        a.message = Some("Donnez un nom à la configuration".into());
        return;
    }
    let date = crate::config_partie::date_du_jour();
    match a.dialogue {
        Some(Dialogue::Enregistrer { modifie: Some(i) }) => {
            if let Some(e) = a.biblio.entrees.get_mut(i) {
                (e.nom, e.commentaire, e.favorite) = (nom.clone(), a.commentaire.trim().to_string(), a.favorite);
            }
        }
        _ => {
            let (commentaire, favorite, config) = (a.commentaire.trim().to_string(), a.favorite, a.config.clone());
            a.biblio.enregistrer(&nom, &commentaire, favorite, &config, &date);
        }
    }
    ecrire(a);
    a.message = Some(format!("Configuration « {nom} » enregistrée"));
    a.dialogue = None;
}

fn ecrire(a: &mut Accueil) {
    if let Err(e) = a.biblio.ecrire() {
        a.message = Some(format!("Enregistrement impossible : {e}"));
    }
}

/// Relance le jeu sur la partie configurée (le monde y est généré), puis ferme cet écran.
fn lancer_partie(config: &ConfigPartie, exit: &mut MessageWriter<AppExit>, message: &mut Option<String>) {
    let (w, h) = config.taille.cases();
    let lance = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .env("CIVBUILDER_CONFIG", crate::config_partie::texte_pour_lancement(config))
            .env("CIVBUILDER_SEED", config.graine.to_string())
            .env("CIVBUILDER_MAP", format!("{w}x{h}"))
            .env_remove("CIVBUILDER_CHARGER")
            .env_remove("CIVBUILDER_ACCUEIL_CAPTURE")
            .spawn()
    });
    match lance {
        Ok(_) => {
            exit.write(AppExit::Success);
        }
        Err(e) => *message = Some(format!("Lancement impossible : {e}")),
    }
}

/// Génère le monde de l'aperçu en tâche de fond.
fn lancer_apercu(apercu: &mut Apercu, config: &ConfigPartie) {
    let (graine, taille) = (config.graine, config.taille);
    *apercu = Apercu { pour: Some((graine, taille)), debut: Some(std::time::Instant::now()), ..default() };
    let resultat = apercu.resultat.clone();
    std::thread::spawn(move || {
        let (w, h) = taille.cases();
        let region = crate::climate::Region::planet(crate::climate::CELL_KM, h);
        let monde = std::panic::catch_unwind(|| WorldMap::generate_in(w, h, graine, region)).map_err(|_| "la génération a échoué".to_string());
        if let Ok(mut r) = resultat.lock() {
            *r = Some(monde);
        }
    });
}

/// Image de la carte (une case par pixel, colonnes et lignes de la grille).
fn image_du_monde(m: &WorldMap) -> Image {
    let (w, h) = (m.width as u32, m.height as u32);
    let mut octets = Vec::with_capacity((w * h * 4) as usize);
    for r in 0..m.height {
        for c in 0..m.width {
            let (rr, gg, bb) = m.get(Hex::from_offset(c, r)).map_or((0.0, 0.0, 0.0), |t| t.color());
            octets.extend([(rr * 255.0) as u8, (gg * 255.0) as u8, (bb * 255.0) as u8, 255]);
        }
    }
    Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, octets, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())
}

/// Suit la génération de l'aperçu (texte du temps écoulé, image une fois prête).
fn suivre_apercu(mut a: ResMut<Accueil>, mut apercu: ResMut<Apercu>, mut images: ResMut<Assets<Image>>, mut attente: Query<&mut Text, With<Attente>>) {
    let Some(debut) = apercu.debut else { return };
    let fini = apercu.resultat.lock().ok().and_then(|mut r| r.take());
    match fini {
        None => {
            for mut t in &mut attente {
                t.0 = format!("Calcul du monde… {:.0} s", debut.elapsed().as_secs_f32());
            }
        }
        Some(Ok(monde)) => {
            let terres = monde.hexes().filter(|&h| monde.get(h).is_some_and(|t| !t.is_water())).count() as f32 / monde.hexes().count().max(1) as f32;
            apercu.resume = Some(format!("{:.0} % de terres", terres * 100.0));
            apercu.image = Some(images.add(image_du_monde(&monde)));
            apercu.duree = Some(debut.elapsed().as_secs_f32());
            apercu.debut = None;
            a.sale = true;
        }
        Some(Err(e)) => {
            apercu.debut = None;
            a.message = Some(format!("Aperçu impossible : {e}"));
            a.sale = true;
        }
    }
}

/// Recopie le texte des champs dans l'état ; Entrée applique le code.
fn champs(mut a: ResMut<Accueil>, champs: Query<(&Champ, &EditableText), Changed<EditableText>>, tous: Query<(&Champ, &EditableText)>, keys: Res<ButtonInput<Key>>, focus: Res<InputFocus>, focalise: Query<&Champ>) {
    for (c, t) in &champs {
        let v = t.value().to_string();
        match c {
            Champ::Code => a.code = v,
            Champ::Nom => a.nom = v,
            Champ::Commentaire => a.commentaire = v,
        }
    }
    let code_focalise = focus.get().and_then(|e| focalise.get(e).ok()) == Some(&Champ::Code);
    if a.appliquer_code || (code_focalise && keys.just_pressed(Key::Enter)) {
        if a.appliquer_code {
            // (après « Coller », le champ a reçu le texte : on le relit)
            if let Some((_, t)) = tous.iter().find(|(c, _)| **c == Champ::Code) {
                a.code = t.value().to_string();
            }
        }
        a.appliquer_code = false;
        let code = a.code.clone();
        let mut config = a.config.clone();
        match config.appliquer_code(&code) {
            Ok(()) => {
                a.config = config;
                a.erreur_code = None;
                a.message = Some("Code appliqué".into());
            }
            Err(e) => a.erreur_code = Some(e),
        }
        a.sale = true;
    }
}

/// Entrée : valide la fenêtre ouverte ou lance la partie ; Échap : ferme la fenêtre ou quitte.
#[allow(clippy::too_many_arguments)]
fn clavier(
    mut a: ResMut<Accueil>,
    mut apercu: ResMut<Apercu>,
    keys: Res<ButtonInput<KeyCode>>,
    focalise: Query<&Champ>,
    mut champs: Query<(Entity, &Champ, &mut EditableText)>,
    mut focus_mut: ResMut<InputFocus>,
    mut clipboard: ResMut<bevy::clipboard::Clipboard>,
    mut exit: MessageWriter<AppExit>,
) {
    let champ = focus_mut.get().and_then(|e| focalise.get(e).ok()).copied();
    let action = if keys.just_pressed(KeyCode::Escape) {
        Some(if a.dialogue.is_some() { Action::Annuler } else { Action::Quitter })
    } else if keys.just_pressed(KeyCode::Enter) && champ != Some(Champ::Code) && champ != Some(Champ::Commentaire) {
        Some(match a.dialogue {
            Some(Dialogue::Enregistrer { .. }) => Action::ValiderEnregistrer,
            Some(Dialogue::Charger { .. }) => Action::ValiderCharger,
            Some(Dialogue::Effacer { .. }) => Action::ValiderEffacer,
            None => Action::Lancer,
        })
    } else {
        None
    };
    if let Some(action) = action {
        agir(&mut a, &mut apercu, action, &mut champs, &mut focus_mut, &mut clipboard, &mut exit);
    }
}

/// `CIVBUILDER_ACCUEIL_CAPTURE=<dossier>` : enregistre l'écran, puis les deux fenêtres, puis ferme.
fn capture_de_verification(mut commands: Commands, mut a: ResMut<Accueil>, mut image: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Some(dossier) = std::env::var_os("CIVBUILDER_ACCUEIL_CAPTURE").map(std::path::PathBuf::from) else { return };
    *image += 1;
    let prise = |commands: &mut Commands, nom: &str| {
        let _ = std::fs::create_dir_all(&dossier);
        commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(dossier.join(nom)));
    };
    match *image {
        30 => prise(&mut commands, "accueil.png"),
        40 => {
            a.nom = "Grand continent froid".into();
            a.commentaire = "Beau continent, départ difficile.".into();
            a.dialogue = Some(Dialogue::Enregistrer { modifie: None });
            a.sale = true;
        }
        60 => prise(&mut commands, "accueil-enregistrer.png"),
        70 if !a.biblio.entrees.is_empty() => {
            a.dialogue = Some(Dialogue::Charger { entree: 0, meme_monde: true });
            a.sale = true;
        }
        90 => prise(&mut commands, "accueil-charger.png"),
        120 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
