//! Écran de la ville (lot 1 de la refonte, tâche 35) : plein écran ouvert depuis la ville (touche C
//! ou bouton), fermé par Échap. En-tête (nom, palier, habitants, croissance, construction), quatre
//! onglets (Rendements, Population, Commerce, Histoire), colonne d'explication à gauche, carte
//! schématique des cases de la ville à droite (survol, clic, zoom, déplacement, légende au-dessus).
//! Référence : `docs/conception/maquettes/ecran-ville.html`. Les systèmes absents du jeu
//! (bâtiments, commerce, histoire, eau…) sont affichés comme tels : « pas encore dans le jeu ».

use crate::charte::{self, Etat, Grandeur, Images, barre, icone, texte};
use crate::game::{Amenagements, City, GameMap, GameState, HexPos, Selection, Territory};
use crate::hex::Hex;
use crate::intendant::{self, Amenagement, Rendements};
use crate::pointer::UiPointer;
use crate::rules;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;

const RACINE_3: f32 = 1.732_050_8;

/// Onglets de l'écran.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Onglet {
    #[default]
    Rendements,
    Population,
    Ressources,
    Histoire,
}

impl Onglet {
    const TOUS: [Onglet; 4] = [Onglet::Rendements, Onglet::Population, Onglet::Ressources, Onglet::Histoire];

    fn nom(self) -> &'static str {
        match self {
            Onglet::Rendements => "Bâtiments",
            Onglet::Population => "Population",
            Onglet::Ressources => "Ressources",
            Onglet::Histoire => "Histoire",
        }
    }
}

/// Couche de la carte du monde pendant que l'écran est ouvert (barre de la carte) ; l'onglet en
/// choisit une par défaut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Couche {
    /// Aménagements, gisements, cases à acheter (couleur : grandeur principale de la case).
    Amenagements,
    /// Travailleurs et places de chaque case.
    Emplois,
    /// Habitants de chaque case.
    Habitants,
    /// Quantité de la grandeur détaillée par case.
    Quantite,
    /// Lieux de l'histoire.
    Histoire,
    /// Gisements connus du territoire et des cases à acheter.
    Gisements,
    /// Échanges du tour avec les autres villes (flèches).
    Echanges,
    /// Type de nourriture de chaque case (céréales, protéines, fruits et légumes).
    Types,
    /// Rendement de chaque case : maintenant et plein (unités), travailleurs sur les places.
    Rendement,
}

impl Couche {
    fn nom(self) -> &'static str {
        match self {
            Couche::Amenagements => "Aménagements",
            Couche::Emplois => "Emplois",
            Couche::Habitants => "Habitants",
            Couche::Quantite => "Quantité",
            Couche::Histoire => "Histoire",
            Couche::Gisements => "Gisements",
            Couche::Echanges => "Échanges",
            Couche::Types => "Types",
            Couche::Rendement => "Rendement",
        }
    }
}

/// Bouton d'une couche dans la barre de la carte.
#[derive(Component)]
struct ChoixCouche(Couche);

/// Horizon de la couche Rendement : la teinte des cases montre ce qu'elles donnent aujourd'hui,
/// dans 10 tours (travailleurs placés par l'intendant avec la population prévue) ou pleines.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Horizon {
    #[default]
    Aujourdhui,
    DixTours,
    Rempli,
}

impl Horizon {
    const TOUS: [Horizon; 3] = [Horizon::Aujourdhui, Horizon::DixTours, Horizon::Rempli];

    /// Nom du bouton.
    fn nom(self) -> &'static str {
        match self {
            Horizon::Aujourdhui => "Aujourd'hui",
            Horizon::DixTours => "Dans 10 tours",
            Horizon::Rempli => "Rempli",
        }
    }
}

/// Bouton d'un horizon de la couche Rendement.
#[derive(Component)]
struct ChoixHorizon(Horizon);

/// État de l'écran : ville affichée (ouvert si `Some`), onglet, case choisie, couches de la carte.
#[derive(Resource, Default)]
pub struct EcranVille {
    pub ville: Option<Entity>,
    pub onglet: Onglet,
    pub case: Option<Hex>,
    /// Couche « aménagements » de la carte (motif et rond vert).
    pub amenagements_caches: bool,
    /// Couche choisie dans la barre de la carte (sinon : celle de l'onglet).
    pub couche: Option<Couche>,
    /// Fiche de droite d'une cause de la croissance (onglet Population).
    pub sujet: Option<Sujet>,
    /// Tiroir du bas ouvert ou replié en une bande, choisi par le joueur pour chaque sorte de sujet
    /// (sinon : ouvert pour la population et les ressources) ; Tab bascule.
    tiroirs: [Option<bool>; 5],
    /// Cases à acheter montrées sur la carte (bouton de la barre de la carte).
    pub achats_caches: bool,
    /// Grandeur détaillée (onglet Rendements) : son graphique, ses origines, la carte à ses couleurs.
    pub grandeur: Option<Grandeur>,
    /// Tours de passé montrés par le graphique (10, 20 ou 40 ; 0 : 40).
    pub periode: usize,
    /// Catalogue de construction ouvert (liste de droite).
    pub catalogue: bool,
    /// Aménagement en cours de placement sur la carte de la ville, et la case choisie.
    pub placement: Option<Amenagement>,
    pub cible: Option<Hex>,
    /// Demande de mise en file de la case choisie (double-clic sur la carte).
    confirmer: bool,
    /// Effectif proposé pour une case (aperçu avant de le forcer).
    forcer: Option<(Hex, u32)>,
    /// Onglet Population : carte de la répartition des habitants (sinon : des emplois).
    pub carte_habitants: bool,
    /// Onglet Histoire : genre d'événements montré (tous si aucun).
    pub histoire_genre: Option<rules::GenreEvenement>,
    /// Élément de la file choisi (sa case est cerclée sur la carte).
    pub file_choisie: Option<usize>,
    /// Autres remèdes de « Ce qui freine » dépliés.
    pub remedes_ouverts: bool,
    /// Graphique du tiroir agrandi (touche F) : le tiroir prend 60 % de la hauteur.
    pub agrandi: bool,
    /// Groupe montré par le catalogue (aménagements, projets, unités) : tout tient à l'écran.
    pub catalogue_groupe: GroupeCatalogue,
    /// Dernière ville montrée (une autre ville s'ouvre sur l'onglet Rendements).
    derniere: Option<Entity>,
    /// Horizon de la couche Rendement (teinte des cases).
    pub horizon: Horizon,
}

/// Groupes du catalogue de construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupeCatalogue {
    #[default]
    Amenagements,
    /// Bâtiments de la ville (puits, grenier, sanctuaire) et ponts.
    Batiments,
    Projets,
    Unites,
}
#[derive(Component)]
struct ChoixGroupe(GroupeCatalogue);

/// Case où se construit un élément de la file (aménagement, pont), s'il en a une.
fn case_de(objet: &rules::Production) -> Option<Hex> {
    match *objet {
        rules::Production::Amenagement { hex, .. } => Some(hex),
        rules::Production::Bridge { at: crate::map::BridgeAt::Edge { hex, .. } | crate::map::BridgeAt::Fleuve { hex, .. }, .. } => Some(hex),
        rules::Production::Unit(_) | rules::Production::Puits | rules::Production::Grenier | rules::Production::Sanctuaire { .. } => None,
    }
}

/// Violet des habitants, du moins au plus peuplé (le plus foncé : beaucoup d'habitants ; Benjamin).
const RAMPE: [(u8, u8, u8); 6] = [(176, 158, 226), (150, 128, 212), (126, 100, 196), (104, 74, 180), (86, 52, 164), (70, 36, 148)];

/// Couleur de la rampe pour `q` de 0 à 1.
fn rampe(q: f32) -> Color {
    let q = q.clamp(0.0, 1.0) * (RAMPE.len() - 1) as f32;
    let i = (q.floor() as usize).min(RAMPE.len() - 2);
    let f = q - i as f32;
    let (a, b) = (RAMPE[i], RAMPE[i + 1]);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f) / 255.0;
    Color::srgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Grandeurs détaillables (les autres ne sont pas encore dans le jeu).
const DETAILLABLES: [Grandeur; 3] = [Grandeur::Nourriture, Grandeur::Production, Grandeur::Deniers];

/// Valeur d'une grandeur dans des rendements.
fn valeur(r: &Rendements, g: Grandeur) -> f64 {
    match g {
        Grandeur::Nourriture => r.nourriture,
        Grandeur::Production => r.production,
        Grandeur::Deniers => r.deniers,
        _ => 0.0,
    }
}

/// Ce que montre la carte de la ville.
#[derive(Clone, Copy, PartialEq)]
enum ModeCarte {
    /// Couleur de la grandeur principale de chaque case.
    Dominante,
    /// Remplissage en travailleurs (onglet Population).
    Remplissage,
    /// Une grandeur, de faible à fort (maximum des cases donné).
    Grandeur(Grandeur, f64),
    /// Placement d'un aménagement : valeur de chaque case (effet réel dans 10 tours).
    Placement(Amenagement, f64),
    /// Habitants par case, du moins au plus peuplé (bornes, en logarithme).
    Habitants(f64, f64),
    /// Lieux de l'histoire (couleurs de la grandeur principale, anneaux dorés).
    Histoire,
    /// Type de nourriture de chaque case.
    Types,
    /// Rendement de chaque case : barres d'unités (maintenant, plein ; la subsistance plus pâle) ;
    /// la teinte suit l'horizon choisi.
    Rendement(Horizon),
}

/// Anneau de la case conseillée par l'intendant (or : le conseil).
const CONSEIL: Color = Color::srgb(0.95, 0.81, 0.45);
/// Couleurs des trois types de nourriture (céréales, protéines, fruits et légumes).
const COULEURS_TYPES: [Color; 3] = [Color::srgb(0.59, 0.77, 0.35), Color::srgb(0.85, 0.63, 0.40), Color::srgb(0.93, 0.58, 0.69)];
/// Couleur des gisements et des échanges de ressources sur la carte.
const GISEMENT: Color = Color::srgb(0.85, 0.63, 0.40);

impl EcranVille {
    /// L'écran est-il ouvert ?
    pub fn ouvert(&self) -> bool {
        self.ville.is_some()
    }

    /// Sorte de sujet du tiroir : cases (0), grandeur (1), population (2), ressources (3), histoire (4).
    fn sorte_tiroir(&self) -> usize {
        match (self.grandeur, self.onglet) {
            (Some(_), _) => 1,
            (None, Onglet::Rendements) => 0,
            (None, Onglet::Population) => 2,
            (None, Onglet::Ressources) => 3,
            (None, Onglet::Histoire) => 4,
        }
    }

    /// Le tiroir montre-t-il un graphique (une grandeur, la population) ?
    fn graphique(&self) -> bool {
        matches!(self.sorte_tiroir(), 1 | 2)
    }

    /// Le graphique du tiroir est-il agrandi ?
    pub fn agrandi(&self) -> bool {
        self.agrandi && self.graphique() && self.tiroir_ouvert()
    }

    /// Le tiroir est-il ouvert (sinon : une bande d'une ligne) ?
    pub fn tiroir_ouvert(&self) -> bool {
        let k = self.sorte_tiroir();
        self.tiroirs[k].unwrap_or(matches!(k, 2 | 3))
    }

    fn basculer_tiroir(&mut self) {
        let k = self.sorte_tiroir();
        self.tiroirs[k] = Some(!self.tiroir_ouvert());
    }
}

/// Cause de la croissance détaillée dans la fiche de droite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sujet {
    BienEtre,
    Salubrite,
    /// Une ressource : son effet, où en est la ville, comment l'obtenir.
    Ressource(crate::gisements::Ressource),
}

/// Jeton ou ligne qui ouvre la fiche d'une ressource (onglet Ressources).
#[derive(Component)]
struct ChoixRessource(crate::gisements::Ressource);

/// Ligne qui ouvre la fiche d'une cause de la croissance.
#[derive(Component)]
struct ChoixSujet(Sujet);

/// Dimensions des panneaux posés sur la carte (pixels logiques), selon la taille de la fenêtre.
#[derive(Resource, Clone, Copy, PartialEq)]
struct Disposition {
    /// Largeur du panneau de gauche et de la colonne de droite.
    gauche: f32,
    droite: f32,
    /// Hauteur du tiroir ouvert.
    tiroir: f32,
    /// Taille de la fenêtre.
    fenetre: Vec2,
}

impl Default for Disposition {
    fn default() -> Self {
        Disposition { gauche: 400.0, droite: 400.0, tiroir: 270.0, fenetre: Vec2::new(1400.0, 860.0) }
    }
}

/// Haut des panneaux (sous la barre du tour) et marge au bord de la fenêtre.
const HAUT: f32 = 56.0;
const MARGE: f32 = 12.0;
/// Hauteur de la bande du tiroir replié ; place du bouton « Fin du tour » en bas à droite.
const BANDE: f32 = 42.0;
const FIN_DU_TOUR: f32 = 70.0;


/// Fin de tour demandée par le joueur (clavier, bouton) : elle est refusée tant qu'une de ses
/// villes n'a rien à produire (ni construction ni projet ; décision de Benjamin).
#[derive(Resource, Default)]
pub struct FinDeTourJoueur(pub bool);

#[derive(Component)]
struct Racine;
#[derive(Component)]
struct EnTete;
#[derive(Component)]
struct BoutonOnglet(Onglet);
#[derive(Component)]
struct Gauche;
#[derive(Component)]
struct Legende;
/// Panneau de gauche (la ville), colonne de droite (la fiche), tiroir du bas (le temps et les
/// tableaux), barre de la carte (couches et caméra) : posés sur la carte du monde.
#[derive(Component)]
struct PanneauGauche;
#[derive(Component)]
struct Droite;
#[derive(Component)]
struct Tiroir;
#[derive(Component)]
struct BarreCarte;
/// Couches proposées par la barre de la carte (remplies à chaque reconstruction).
#[derive(Component)]
struct Couches;
/// Pied du panneau de gauche : la construction, toujours visible.
#[derive(Component)]
struct Pied;
/// Étiquettes posées sur les cases de la carte (sous les panneaux).
#[derive(Component)]
struct Etiquettes;
/// Boutons de la caméra : zoom (+1, −1), recentrer (0).
#[derive(Component)]
struct BoutonZoom(i8);
#[derive(Component)]
struct TitreListe;
#[derive(Component)]
struct Liste;
/// Bouton qui ouvre ou replie le tiroir (Tab).
#[derive(Component)]
struct BasculeTiroir;
/// Bouton « Agrandir (F) » du graphique du tiroir.
#[derive(Component)]
struct BasculeAgrandi;
/// Montrer les cases à acheter sur la carte.
#[derive(Component)]
struct BasculeAchats;
/// Élément du HUD de la carte du monde caché pendant que l'écran de la ville est ouvert.
#[derive(Component)]
pub struct CacheSousEcran;
#[derive(Component)]
struct Bulle;
/// Ligne d'une grandeur de l'en-tête : survolée, l'infobulle décompose son total.
#[derive(Component)]
struct AideGrandeur(Grandeur);
/// Infobulle des lignes de l'en-tête.
#[derive(Component)]
struct BulleAide;
#[derive(Component)]
struct Fermer;
/// Ville précédente (−1) ou suivante (+1), l'onglet gardé.
#[derive(Component)]
struct ChangerVille(i8);
/// Ouvre un onglet (lignes Bien-être et Salubrité, jetons des besoins).
#[derive(Component)]
struct OuvrirOnglet(Onglet);
/// Élément cliquable qui choisit une case (liste) ; survolé, il éclaire la case sur la carte.
#[derive(Component)]
struct ChoixCase(Hex);
#[derive(Component)]
struct FermerFiche;
/// Bouton « Acheter » de la fiche d'une case voisine du territoire.
#[derive(Component)]
struct AcheterCase(Hex);
#[derive(Component)]
struct BasculeAmenagements;
/// Carte ou ligne qui ouvre le détail d'une grandeur.
#[derive(Component)]
struct ChoixGrandeur(Grandeur);
#[derive(Component)]
struct ChoixPeriode(usize);
/// Fil d'Ariane : retour à la vue d'ensemble (0) ou à la grandeur (1).
#[derive(Component)]
struct Fil(u8);
/// Ce que propose le catalogue de construction.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Element {
    Unite(crate::rules::UnitKind),
    Amenagement(Amenagement),
    Projet(crate::rules::Projet),
    /// Puits de la ville (sur le centre, un par ville).
    Puits,
    /// Grenier de la ville (un par ville).
    Grenier,
    /// Sanctuaire (un par ville ; mégalithe avec de la pierre dure).
    Sanctuaire,
}
#[derive(Component)]
struct ChoixCatalogue(Element);
/// Ouvre ou ferme le catalogue (en-tête, liste).
#[derive(Component)]
struct BoutonConstruire;
/// Action sur la file : monter d'un rang, retirer.
#[derive(Component, Clone, Copy)]
enum ActionFile {
    Monter(usize),
    Retirer(usize),
}
#[derive(Component)]
struct MettreEnFile;
#[derive(Component)]
struct ArreterProjet;
/// Depuis la fiche d'une case : placer cet aménagement ici.
#[derive(Component)]
struct AmenagerIci(Amenagement);
/// Réglage de l'intendant : objectif de nourriture, ou priorité de la production (1) ou des deniers (2).
#[derive(Clone, Copy, PartialEq)]
enum Reglage {
    Objectif(intendant::ObjectifNourriture),
    Priorite(usize, intendant::Priorite),
}
#[derive(Component)]
struct ChoixReglage(Reglage);
/// Onglet Population : carte des emplois (faux) ou des habitants (vrai).
#[derive(Component)]
struct ChoixCartePop(bool);
/// Onglet Histoire : filtre par genre (aucun : tout).
#[derive(Component)]
struct ChoixGenre(Option<rules::GenreEvenement>);
/// Remède proposé par le diagnostic : placer un aménagement (case conseillée), ouvrir le détail
/// d'une grandeur (réglages de l'intendant), ou ouvrir le catalogue.
#[derive(Clone, Copy, PartialEq)]
enum Remede {
    Placer(Amenagement),
    /// Placer un aménagement sur une case donnée (l'exploitation d'un gisement).
    PlacerSur(Amenagement, Hex),
    Detail(Grandeur),
    Construire,
}

/// Un remède de « Ce qui freine », avec son effet net sur la croissance (habitants par tour,
/// calculé par le jeu) et ce que fait son bouton.
struct RemedeNet {
    libelle: String,
    net: Option<i64>,
    action: ActionRemede,
}

#[derive(Clone, Copy)]
enum ActionRemede {
    /// Mettre en file une construction (puits…).
    Catalogue(Element),
    Remede(Remede),
}

/// « 2 autres ▾ » : déplie les autres remèdes par-dessus le panneau.
#[derive(Component)]
struct BasculeRemedes;
#[derive(Component)]
struct BoutonRemede(Remede);
/// Élément qui pulse quelques fois quand il apparaît (conseil, bouton de validation).
#[derive(Component)]
struct Pulsation;
/// Barre de validation du placement, en bas de la carte de la ville (là où l'on regarde).
#[derive(Component)]
struct BarreValidation;
/// Ligne de la file de construction (pour l'éclairer quand on vient d'y ajouter un élément).
#[derive(Component)]
struct LigneFile(usize);

/// Ce qui freine la croissance (un seul diagnostic, le plus grave), ses remèdes chiffrés, et les
/// autres alertes de la ville.
struct Frein {
    etat: Etat,
    /// En une ligne (bandeau du catalogue).
    texte: String,
    /// Ce qui coûte des habitants ce tour, à terme (arrêt de la croissance, palier), les emplois.
    ce_tour: (String, Color),
    a_terme: (String, Color),
    emplois: Option<(String, Color)>,
    /// Remèdes, du plus rentable au moins rentable.
    remedes: Vec<RemedeNet>,
    alertes: Vec<(Etat, String, Remede)>,
}

/// Diagnostic de la croissance de la ville : ce qui la freine ce tour et à terme, et les remèdes
/// chiffrés en habitants par tour (la ville simulée avec chacun).
fn frein(vl: &Ville, city: &City, cases: &[CaseVue]) -> Frein {
    let (map, prevision) = (vl.map, vl.prevision);
    let c = &city.state;
    let y = city.travail.rendements;
    let h = c.habitants as f64;
    let cr = c.croissance(y);
    let base = c.variation(y);
    let besoin = c.besoin_nourriture().max(1e-6);
    let couverture = y.nourriture / besoin;
    // Ce tour : la cause qui coûte le plus d'habitants
    let famine = (h * cr.famine).round();
    let maladies = (h * cr.insalubrite).round();
    let ce_tour = if famine < 0.0 {
        (format!("famine −{} hab./tour · nourriture × {}", rules::milliers(famine.abs() as u64), rules::decimal(couverture)), charte::GRAVE)
    } else if maladies < 0.0 {
        (format!("maladies −{} hab./tour · salubrité {:.0}", rules::milliers(maladies.abs() as u64), c.salubrite()), charte::GRAVE)
    } else if c.humeur() == rules::Humeur::Mecontente {
        (format!("bien-être {:.0} : la croissance naturelle × {}", c.bien_etre, rules::decimal(cr.facteur_bien_etre)), charte::ALERTE)
    } else {
        ("rien ne coûte d'habitants".to_string(), charte::OK)
    };
    // À terme : l'arrêt de la croissance (prévision du jeu) ou le palier suivant
    let objectif = c.consignes.objectif;
    let suivant = rules::habitants_du_palier(c.palier() + 1);
    let a_terme = if objectif.cible() < 1.0 + rules::SURPLUS_POUR_CROITRE && (1.0..1.0 + rules::SURPLUS_POUR_CROITRE - 1e-6).contains(&couverture) {
        (format!("l'objectif « {} » de l'intendant limite la croissance", objectif.nom().to_lowercase()), charte::ALERTE)
    } else {
        match (rules::arret_de_croissance(prevision), rules::tours_jusqu_a(prevision, suivant)) {
            (Some(t), None) => {
                let vers = prevision.get(t - 1).map_or(c.habitants, |p| p.habitants);
                let cause = if maladies < 0.0 { "nourriture et maladies" } else { "nourriture" };
                (format!("arrêt vers {} dans {t} tours · {cause}", rules::milliers(arrondi_parlant(vers))), charte::ALERTE)
            }
            (_, Some(t)) => (format!("palier {} dans {t} tours", c.palier() + 1), charte::OK),
            (None, None) => (format!("pas d'arrêt d'ici {} tours", prevision.len()), charte::OK),
        }
    };
    let emplois = if city.travail.chomeurs > 0 {
        Some((format!("{} sans emploi : aménager des cases donne des emplois", rules::milliers(city.travail.chomeurs.into())), charte::ALERTE))
    } else {
        prevision.iter().position(|p| p.chomeurs > 0).map(|t| (format!("places pleines dans {} tours", t + 1), if t < 10 { charte::ALERTE } else { charte::DOUX }))
    };
    // Remèdes chiffrés : la ville simulée avec chacun (effet net sur les habitants par tour)
    let mut remedes: Vec<RemedeNet> = Vec::new();
    if !c.puits && !c.file.iter().any(|k| k.objet == rules::Production::Puits) {
        let mut avec = c.clone();
        avec.puits = true;
        remedes.push(RemedeNet { libelle: "Puits".into(), net: Some(avec.variation(y) - base), action: ActionRemede::Catalogue(Element::Puits) });
    }
    let bareme = Bareme::de(vl);
    for k in cases.iter().filter(|k| k.amenagement != Some(Amenagement::Exploitation) && !k.en_file) {
        let Some(r) = k.gisement else { continue };
        if c.demande(r) <= 0.0 || c.couverture(r) >= 0.95 || possible(map, k, Amenagement::Exploitation).is_err() {
            continue;
        }
        let Ok(g) = gain_amenagement(vl, k, Amenagement::Exploitation) else { continue };
        let mut avec = c.clone();
        avec.couverture.resize(crate::gisements::N_RESSOURCES, 0.0);
        avec.couverture[r.index()] = (1.0 / c.demande(r)).min(1.0).max(c.couverture(r));
        remedes.push(RemedeNet { libelle: format!("{} ({})", capitale(r.exploitation()), r.nom()), net: Some(avec.variation(y + g.dans) - base), action: ActionRemede::Remede(Remede::PlacerSur(Amenagement::Exploitation, k.hex)) });
    }
    for a in [Amenagement::Champ, Amenagement::Paturage] {
        let Some(case) = conseil(vl, cases, a, &bareme).and_then(|h| cases.iter().find(|x| x.hex == h)) else { continue };
        let Ok(g) = gain_amenagement(vl, case, a) else { continue };
        remedes.push(RemedeNet { libelle: capitale(a.nom()), net: Some(c.variation(y + g.dans) - base), action: ActionRemede::Remede(Remede::Placer(a)) });
    }
    if objectif.cible() < 1.0 + rules::SURPLUS_POUR_CROITRE {
        remedes.push(RemedeNet { libelle: "Changer l'objectif de l'intendant".into(), net: None, action: ActionRemede::Remede(Remede::Detail(Grandeur::Nourriture)) });
    }
    remedes.sort_by(|a, b| b.net.unwrap_or(i64::MIN).cmp(&a.net.unwrap_or(i64::MIN)));
    let etat = if famine < 0.0 || maladies < 0.0 { Etat::Grave } else if a_terme.1 == charte::ALERTE { Etat::Alerte } else { Etat::Ok };
    // Autres alertes
    let mut alertes = Vec::new();
    if c.colon_bloque() {
        alertes.push((Etat::Alerte, format!("Colon en tête de file : il faut {} habitants pour le former ; rien d'autre n'avance", rules::milliers(rules::CityState::HABITANTS_POUR_COLON)), Remede::Construire));
    }
    if c.projet.is_some() && c.file.is_empty() && c.tours_en_projet >= rules::RAPPEL_PROJET {
        alertes.push((Etat::Ok, format!("En projet depuis {} tours", c.tours_en_projet), Remede::Construire));
    }
    let texte = format!("ce tour : {} · à terme : {}", ce_tour.0, a_terme.0);
    Frein { etat, texte, ce_tour, a_terme, emplois, remedes, alertes }
}

/// Bouton d'un remède (mettre en file ou placer), avec son effet net.
fn bouton_remede(l: &mut ChildSpawnerCommands, r: &RemedeNet, principal: bool) {
    let t = match r.net {
        Some(n) => format!("{} : net {}{} hab./tour", r.libelle, if n >= 0 { "+" } else { "−" }, rules::milliers(n.unsigned_abs())),
        None => r.libelle.clone(),
    };
    let couleur = if principal { charte::ACTION_FOND } else { charte::SURFACE };
    let texte_c = if r.net.is_some_and(|n| n < 0) { charte::GRAVE } else { charte::TEXTE };
    let mut b = l.spawn((bouton(couleur), children![texte(t, 12.5, texte_c)]));
    match r.action {
        ActionRemede::Catalogue(e) => b.insert(ChoixCatalogue(e)),
        ActionRemede::Remede(x) => b.insert(BoutonRemede(x)),
    };
}

/// Forcer l'effectif d'une case : proposer un autre effectif, confirmer, annuler, rendre à l'intendant.
#[derive(Component, Clone, Copy)]
enum Forcage {
    Proposer(Hex, u32),
    Confirmer(Hex, u32),
    Annuler,
    Rendre(Hex),
    /// Verrou « figer » : l'effectif du moment, forcé.
    Figer(Hex, u32),
    /// Verrou « prioriser » : l'intendant remplit la case d'abord.
    Prioriser(Hex),
}

/// Ce que la fiche d'une case montre de son effectif forcé.
struct VueForcage {
    /// Effectif forcé aujourd'hui (l'intendant ne le change plus).
    actuel: Option<u32>,
    /// Effectif proposé et son effet sur toute la ville.
    propose: Option<(u32, Rendements)>,
}

/// Effet d'un aménagement sur une case (décision B1) : l'effet réel sur la ville, tout de suite et
/// dans `TOURS_EFFET` tours (l'intendant refait sa répartition), et ce qu'il ajoute au maximum de
/// la ville (la case pleine), qu'on ne gagne pas tout de suite.
struct GainAmenagement {
    maintenant: Rendements,
    dans: Rendements,
    plein: Rendements,
    cout: i32,
    defriche: bool,
    capacite: u32,
}

/// Grandeur que vise un aménagement.
fn vise(a: Amenagement) -> Grandeur {
    match a {
        Amenagement::CoupeDeBois => Grandeur::Production,
        _ => Grandeur::Nourriture,
    }
}

/// Première lettre en capitale (« champ » → « Champ »).
fn capitale(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

/// Tours après lesquels se lit l'effet d'un aménagement (décision B1).
const TOURS_EFFET: usize = 10;

/// Durée d'une reconstruction de l'écran, écrite dans la console quand elle dépasse 15 ms.
struct Mesure(std::time::Instant);

impl Drop for Mesure {
    fn drop(&mut self) {
        let ms = self.0.elapsed().as_secs_f64() * 1000.0;
        if ms > 15.0 {
            info!("écran de la ville refait en {ms:.0} ms");
        }
    }
}

/// Simulations de la ville gardées d'une image et d'une reconstruction à l'autre : refaites
/// seulement quand la ville (habitants, file, consignes, projet), ses cases ou leurs aménagements
/// changent.
#[derive(Resource, Default)]
struct CacheVille {
    cle: Option<u64>,
    dispo: Vec<intendant::CaseDispo>,
    prevision: Vec<rules::Pas>,
    /// Base des effets : la ville sans sa file, sur `TOURS_EFFET` tours (l'effet d'un aménagement seul).
    base: Vec<rules::Pas>,
    effets: std::sync::Mutex<std::collections::HashMap<(Hex, Amenagement), (Rendements, Rendements)>>,
}

/// La ville telle que la voit l'intendant : ses cases exploitables, la prévision (décision B4 ; la
/// file avance, décision C1) et les effets des aménagements déjà simulés (une simulation par case et
/// par aménagement, gardée pour le conseil, la carte et les fiches).
struct Ville<'a> {
    map: &'a crate::map::WorldMap,
    centre: Hex,
    etat: &'a rules::CityState,
    dispo: &'a [intendant::CaseDispo],
    prevision: &'a [rules::Pas],
    cache: &'a CacheVille,
}

impl<'a> Ville<'a> {
    /// Cases et prévision de la ville `v` (du cache s'il est à jour).
    fn new(cache: &'a mut CacheVille, map: &'a crate::map::WorldMap, centre: Hex, v: Entity, city: &'a City, territoire: &Territory, amenagements: &Amenagements) -> Self {
        use std::hash::{Hash, Hasher};
        let mut possedees: Vec<Hex> = territoire.0.iter().filter(|(_, e)| **e == v).map(|(h, _)| *h).collect();
        possedees.sort();
        let c = &city.state;
        let mut h = std::hash::DefaultHasher::new();
        (v, centre, c.habitants, c.tours_en_projet, c.en_famine, c.bien_etre.to_bits(), c.puits, c.grenier, c.stock_total().to_bits()).hash(&mut h);
        format!("{:?}{:?}{:?}{:?}", c.file, c.consignes, c.projet, c.site).hash(&mut h);
        for p in &possedees {
            (p, amenagements.0.get(p)).hash(&mut h);
        }
        let cle = h.finish();
        if cache.cle != Some(cle) {
            let dispo = rules::cases_de_la_ville(map, centre, &possedees, &Default::default(), &amenagements.0);
            cache.prevision = rules::prevoir(map, centre, c, &dispo, HORIZON);
            let mut sans_file = c.allege();
            sans_file.file.clear();
            cache.base = rules::prevoir(map, centre, &sans_file, &dispo, TOURS_EFFET);
            cache.dispo = dispo;
            cache.effets.get_mut().unwrap_or_else(|e| e.into_inner()).clear();
            cache.cle = Some(cle);
        }
        let cache: &'a CacheVille = cache;
        Ville { map, centre, etat: c, dispo: &cache.dispo, prevision: &cache.prevision, cache }
    }

    /// Effet réel de l'aménagement `a` sur la case `h` : la ville simulée avec lui, comparée à la
    /// même ville sans lui, tout de suite et dans `TOURS_EFFET` tours (sans la file : l'effet de
    /// l'aménagement seul, même s'il est déjà en file).
    fn effet(&self, h: Hex, a: Amenagement) -> (Rendements, Rendements) {
        let effets = || self.cache.effets.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(e) = effets().get(&(h, a)) {
            return *e;
        }
        let mut dispo = self.dispo.to_vec();
        if let Some(c) = dispo.iter_mut().find(|c| c.hex == h) {
            c.amenager(self.map, Some(a));
        }
        let mut sans_file = self.etat.allege();
        sans_file.file.clear();
        let p = rules::prevoir(self.map, self.centre, &sans_file, &dispo, TOURS_EFFET);
        let (b, k) = (&self.cache.base, TOURS_EFFET - 1);
        let e = (p[0].rendements - b[0].rendements, p[k].rendements - b[k].rendements);
        effets().insert((h, a), e);
        e
    }
}

/// L'aménagement `a` peut-il aller sur la case (sinon, pourquoi) ?
fn possible(map: &crate::map::WorldMap, c: &CaseVue, a: Amenagement) -> Result<(), &'static str> {
    if c.centre {
        return Err("pas sur le centre de la ville");
    }
    if c.amenagement == Some(a) {
        return Err("déjà aménagée ainsi");
    }
    if c.en_file {
        return Err("un aménagement y est déjà en file");
    }
    if a == Amenagement::Exploitation && c.gisement.is_none() {
        return Err("aucun gisement connu");
    }
    // Un champ ou un pâturage sur une forêt l'a défrichée : il n'y a plus d'arbres (Benjamin).
    if a == Amenagement::CoupeDeBois && matches!(c.amenagement, Some(Amenagement::Champ | Amenagement::Paturage | Amenagement::Legumineuses | Amenagement::Maraichage)) {
        return Err("forêt défrichée : plus d'arbres à couper");
    }
    a.possible(map, c.hex)
}

/// Ce que donnerait l'aménagement `a` sur la case (ou pourquoi il est impossible).
fn gain_amenagement(vl: &Ville, c: &CaseVue, a: Amenagement) -> Result<GainAmenagement, &'static str> {
    let map = vl.map;
    possible(map, c, a)?;
    let max = intendant::rendement_max(map, c.hex, Some(a));
    let cap = intendant::capacite(map, c.hex, Some(a));
    let cout = a.cout(map, c.hex);
    let (maintenant, dans) = vl.effet(c.hex, a);
    Ok(GainAmenagement { maintenant, dans, plein: max - c.max, cout, defriche: cout > a.cout_de_base(), capacite: cap })
}

/// Poids de la nourriture utile, entre le besoin et l'objectif de l'intendant (c'est elle qui fait
/// grandir la ville).
const POIDS_FREIN: f64 = 1.5;
/// Couverture du besoin sous laquelle une perte de nourriture menace de famine.
const BORD_FAMINE: f64 = 1.15;
/// Poids en plus de la nourriture sous le besoin : la famine (Benjamin : la prendre en considération).
const POIDS_PERTE_FAMINE: f64 = 3.0;

/// Barème de l'intendant pour juger un aménagement (décision C3 : la nourriture compte selon son
/// utilité) : nourriture et besoin aujourd'hui (risque de famine) et dans `TOURS_EFFET` tours
/// (valeur), objectif de l'intendant (× le besoin) au-delà duquel la nourriture ne compte plus.
#[derive(Clone, Copy, Debug)]
struct Bareme {
    nourriture: f64,
    besoin: f64,
    nourriture_dans: f64,
    besoin_dans: f64,
    plafond: f64,
}

impl Bareme {
    /// Barème de la ville, d'après sa prévision sans file (celle des effets).
    fn de(vl: &Ville) -> Bareme {
        let b = &vl.cache.base;
        let besoin = vl.etat.besoin_nourriture().max(1e-6);
        let k = TOURS_EFFET - 1;
        // (les rendements d'un tour viennent des habitants du début du tour)
        let besoin_dans = (b[k - 1].habitants as f64 / rules::HABITANTS_PAR_NOURRITURE).max(1e-6);
        Bareme { nourriture: b[0].rendements.nourriture, besoin, nourriture_dans: b[k].rendements.nourriture, besoin_dans, plafond: vl.etat.consignes.objectif.cible().max(1.0) }
    }

    /// Valeur d'une quantité de nourriture : ×(1,5 + 3) sous le besoin, ×1,5 jusqu'à l'objectif,
    /// rien au-delà.
    fn utilite(&self, n: f64, besoin: f64) -> f64 {
        let n = n.max(0.0);
        (POIDS_FREIN + POIDS_PERTE_FAMINE) * n.min(besoin) + POIDS_FREIN * (n.clamp(besoin, besoin * self.plafond) - besoin)
    }

    /// Valeur d'un écart de nourriture dans `TOURS_EFFET` tours.
    fn nourriture_utile(&self, ecart: f64) -> f64 {
        self.utilite(self.nourriture_dans + ecart, self.besoin_dans) - self.utilite(self.nourriture_dans, self.besoin_dans)
    }
}

/// Risque de famine d'un aménagement qui retire de la nourriture (couverture du besoin après).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Risque {
    Aucun,
    BordDeFamine(f64),
    Famine(f64),
}

/// Valeur d'un aménagement pour la ville : la même pour le chiffre de la carte, sa couleur et le
/// conseil de l'intendant.
#[derive(Clone, Copy, Debug)]
struct Evaluation {
    valeur: f64,
    /// Part de la nourriture dans la valeur (ce qu'elle vaut une fois comptée selon son utilité).
    nourriture: f64,
    risque: Risque,
    /// Tours pour que le gain rembourse le coût.
    rentabilite: Option<f64>,
}

/// Valeur = nourriture utile + production + deniers de l'effet réel dans `TOURS_EFFET` tours
/// (décision C3) ; une perte de nourriture qui met la ville en famine dès maintenant compte aussi.
fn evaluer(effet: Rendements, maintenant: Rendements, cout: i32, b: &Bareme) -> Evaluation {
    let perte = maintenant.nourriture.min(effet.nourriture);
    let risque = if perte < -0.05 {
        let couverture = (b.nourriture + perte) / b.besoin;
        if couverture < 1.0 {
            Risque::Famine(couverture)
        } else if couverture < BORD_FAMINE {
            Risque::BordDeFamine(couverture)
        } else {
            Risque::Aucun
        }
    } else {
        Risque::Aucun
    };
    let mut nourriture = b.nourriture_utile(effet.nourriture);
    if risque != Risque::Aucun {
        // (la pire des deux : la famine tout de suite, ou dans 10 tours)
        nourriture = nourriture.min(b.utilite(b.nourriture + perte, b.besoin) - b.utilite(b.nourriture, b.besoin));
    }
    let valeur = nourriture + effet.production + effet.deniers;
    Evaluation { valeur, nourriture, risque, rentabilite: (valeur > 0.05).then(|| cout as f64 / valeur) }
}

/// Évaluation d'un aménagement sur une case.
fn evaluation(g: &GainAmenagement, b: &Bareme) -> Evaluation {
    evaluer(g.dans, g.maintenant, g.cout, b)
}

/// Valeur sous laquelle un aménagement n'apporte rien (pas de conseil).
const VALEUR_UTILE: f64 = 0.05;

/// Case conseillée par l'intendant : la plus forte valeur, hors risque de famine (s'il y a mieux) ;
/// aucune si rien n'apporte quelque chose.
fn conseil(vl: &Ville, cases: &[CaseVue], a: Amenagement, b: &Bareme) -> Option<Hex> {
    let notes: Vec<(Hex, Evaluation)> = cases.iter().filter_map(|c| gain_amenagement(vl, c, a).ok().map(|g| (c.hex, evaluation(&g, b)))).filter(|(_, e)| e.valeur >= VALEUR_UTILE).collect();
    let sures: Vec<&(Hex, Evaluation)> = notes.iter().filter(|(_, e)| !matches!(e.risque, Risque::Famine(_))).collect();
    let liste = if sures.is_empty() { notes.iter().collect() } else { sures };
    liste.into_iter().max_by(|x, y| x.1.valeur.total_cmp(&y.1.valeur).then(y.0.cmp(&x.0))).map(|(h, _)| *h)
}

/// Chiffre signé à une décimale (« +4,5 », « −2,0 »).
fn signe(v: f64) -> String {
    // (pas de « −0,0 » : ce qui s'arrondit à zéro n'a pas de signe)
    if v.abs() < 0.05 {
        return rules::decimal(0.0);
    }
    format!("{}{}", if v >= 0.0 { "+" } else { "−" }, rules::decimal(v.abs()))
}

/// Couleur d'une valeur selon son signe (gain, perte, rien).
fn couleur_signe(v: f64) -> Color {
    if v >= VALEUR_UTILE {
        charte::OK
    } else if v <= -VALEUR_UTILE {
        charte::GRAVE
    } else {
        charte::MUET
    }
}

/// Comment l'intendant compte la valeur, en une ligne.
fn regle_valeur(b: &Bareme) -> String {
    format!(
        "valeur = effet réel dans {TOURS_EFFET} tours (l'intendant refait sa répartition) : nourriture utile + production + deniers ; la nourriture compte × {} jusqu'à {} × le besoin (objectif de l'intendant), rien au-delà, × {} sous le besoin (famine)",
        rules::decimal(POIDS_FREIN),
        rules::decimal(b.plafond),
        rules::decimal(POIDS_FREIN + POIDS_PERTE_FAMINE)
    )
}

/// Écarts de rendement en texte (« +2,0 nourriture · −1,0 production »).
fn ecarts(d: &Rendements) -> String {
    let t: Vec<String> = DETAILLABLES
        .iter()
        .filter(|g| valeur(d, **g).abs() >= 0.05)
        .map(|g| {
            let v = valeur(d, *g);
            format!("{}{} {}", if v > 0.0 { "+" } else { "−" }, rules::decimal(v.abs()), g.nom().to_lowercase())
        })
        .collect();
    if t.is_empty() { "rien".into() } else { t.join(" · ") }
}

/// Animations de l'écran : début du placement en cours (pulsation du conseil et du bouton de
/// validation), et ligne de la file ajoutée à l'instant (elle s'éclaire puis s'éteint).
#[derive(Resource, Default)]
struct AnimEcran {
    placement: Option<(Amenagement, f64)>,
    flash_file: Option<(usize, f64)>,
}

/// Pulsations et éclairages ; rien ne bouge si les animations sont « instantanées » (Paramètres).
fn animer(
    time: Res<Time>,
    settings: Res<crate::settings::Settings>,
    ecran: Res<EcranVille>,
    mut anim: ResMut<AnimEcran>,
    mut pulses: Query<(Option<&mut ImageNode>, Option<&mut BorderColor>), With<Pulsation>>,
    mut lignes: Query<(&LigneFile, &mut BackgroundColor)>,
) {
    let t = time.elapsed_secs_f64();
    let statique = settings.animation == crate::settings::AnimationSpeed::Instant;
    // (le placement change : la pulsation repart)
    if ecran.placement != anim.placement.map(|(a, _)| a) {
        anim.placement = ecran.placement.map(|a| (a, t));
    }
    let k = match anim.placement {
        Some((_, debut)) if !statique && t - debut < 2.4 => (0.5 + 0.5 * ((t - debut) * std::f64::consts::TAU / 0.8).cos()) as f32,
        _ => 1.0,
    };
    for (image, bord) in &mut pulses {
        if let Some(mut i) = image {
            let c = i.color.with_alpha(0.35 + 0.65 * k);
            if i.color != c {
                i.color = c;
            }
        }
        if let Some(mut b) = bord {
            let c = charte::SELECTION.with_alpha(0.25 + 0.75 * (1.0 - k));
            if b.top != c {
                *b = BorderColor::all(c);
            }
        }
    }
    if let Some((i, debut)) = anim.flash_file {
        let f = if statique { 0.0 } else { (1.0 - (t - debut) / 1.2).max(0.0) as f32 };
        for (l, mut fond) in &mut lignes {
            if l.0 == i {
                fond.0 = melange(charte::SURFACE, Grandeur::Production.couleur(), 0.35 * f);
            }
        }
        if f <= 0.0 {
            anim.flash_file = None;
        }
    }
}

/// Systèmes de l'écran (avant le menu Échap, qui ne doit pas s'ouvrir quand Échap ferme l'écran).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EcranVilleSet;

pub struct EcranVillePlugin;

impl Plugin for EcranVillePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EcranVille>()
            .init_resource::<CacheVille>()
            .init_resource::<Disposition>()
            .init_resource::<FinDeTourJoueur>()
            .init_resource::<AnimEcran>()
            .add_systems(Update, animer.after(EcranVilleSet))
            .add_systems(Update, fin_de_tour_joueur.after(crate::input::InputSet).before(crate::game::EndTurnSet))
            .add_systems(Startup, construire)
            .add_systems(
                Update,
                (ouvrir_fermer, commandes, cadrage, disposer, carte_souris, aide_grandeurs, defiler, molette_graphique, glisser_file, construire_actions, reconstruire, afficher, cacher_hud, bouton_agrandir)
                    .chain()
                    .in_set(EcranVilleSet)
                    .before(crate::menu::toggle_menu),
            );
    }
}

// ------------------------------------------------------------------ Données d'une case

/// Ce que la carte et les fiches montrent d'une case de la ville.
#[derive(Clone)]
struct CaseVue {
    hex: Hex,
    centre: bool,
    nom: String,
    travailleurs: u32,
    capacite: u32,
    max: Rendements,
    /// Ce que la case donne selon ses travailleurs.
    courbe: intendant::Courbe,
    rendement: Rendements,
    amenagement: Option<Amenagement>,
    /// Habitants de la case : pour l'instant, chacun vit là où il travaille (les sans-emploi au centre).
    habitants: f64,
    /// Un aménagement de la case est déjà dans la file de construction.
    en_file: bool,
    /// Effectif forcé par le joueur (l'intendant ne le change plus).
    force: Option<u32>,
    /// Case prioritaire : l'intendant la remplit d'abord.
    prioritaire: bool,
    /// Gisement connu de la case (5f).
    gisement: Option<crate::gisements::Ressource>,
}

impl CaseVue {
    /// Part de la capacité occupée (0 à 1).
    fn remplissage(&self) -> f32 {
        if self.capacite == 0 { 0.0 } else { (self.travailleurs as f32 / self.capacite as f32).min(1.0) }
    }

    /// Grandeur principale de la case (les deniers comptent un peu plus : ils sont plus rares).
    fn dominante(&self) -> Option<Grandeur> {
        // (case sans travailleurs : ce qu'elle donnerait, en pâle)
        let r = if self.travailleurs > 0 || self.centre { &self.rendement } else { &self.max };
        let poids = [(Grandeur::Nourriture, r.nourriture), (Grandeur::Production, r.production), (Grandeur::Deniers, r.deniers / 0.6)];
        poids.into_iter().filter(|(_, v)| *v > 0.05).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(g, _)| g)
    }
}

/// Gisements connus du joueur, et le mode debug (tous connus).
type Connus<'a> = (&'a crate::game::Gisements, bool);

fn nom_de_case(map: &crate::map::WorldMap, h: Hex) -> String {
    map.describe(h)
}

/// Cases de la ville : son centre et son territoire, avec travailleurs et rendements.
fn cases_vues(map: &crate::map::WorldMap, centre: Hex, ville: Entity, city: &City, territoire: &Territory, amenagements: &Amenagements, connus: &Connus) -> Vec<CaseVue> {
    let possedees: Vec<Hex> = territoire.0.iter().filter(|(_, e)| **e == ville).map(|(h, _)| *h).collect();
    let dispo = rules::cases_de_la_ville(map, centre, &possedees, &Default::default(), &amenagements.0);
    let rc = Rendements::from(rules::center_yields(map, centre));
    let mut cases = vec![CaseVue { hex: centre, centre: true, nom: "Centre de la ville".into(), travailleurs: 0, capacite: 0, max: rc, courbe: intendant::Courbe::default(), rendement: rc, amenagement: None, habitants: 0.0, en_file: false, force: None, prioritaire: false, gisement: None }];
    for c in dispo {
        let w = city.travail.sur(c.hex);
        cases.push(CaseVue {
            hex: c.hex,
            centre: false,
            nom: {
                // (un champ ou un pâturage sur une forêt l'a défrichée)
                let a = amenagements.0.get(&c.hex).copied();
                let defrichee = matches!(a, Some(Amenagement::Champ | Amenagement::Paturage | Amenagement::Legumineuses | Amenagement::Maraichage)) && map.get(c.hex).is_some_and(|t| t.is_wooded());
                if defrichee { format!("{} défrichée", nom_de_case(map, c.hex)) } else { nom_de_case(map, c.hex) }
            },
            travailleurs: w,
            capacite: c.capacite,
            max: c.max,
            courbe: c.courbe,
            rendement: c.courbe.rendement(w),
            amenagement: amenagements.0.get(&c.hex).copied(),
            habitants: 0.0,
            en_file: city.state.file.iter().any(|k| matches!(k.objet, rules::Production::Amenagement { hex, .. } if hex == c.hex)),
            force: city.state.consignes.forcees.iter().find(|(h, _)| *h == c.hex).map(|(_, n)| *n),
            prioritaire: city.state.consignes.prioritaires.contains(&c.hex),
            gisement: connus.0.connu(map, c.hex, connus.1),
        });
    }
    // Habitants répartis comme les emplois (les sans-emploi vivent au centre)
    let emplois = |c: &CaseVue| c.travailleurs as f64 + if c.centre { city.travail.chomeurs as f64 } else { 0.0 };
    let total: f64 = cases.iter().map(emplois).sum();
    if total > 0.0 {
        let hab = city.state.habitants as f64;
        for c in cases.iter_mut() {
            c.habitants = hab * emplois(c) / total;
        }
    }
    cases
}

/// Case voisine du territoire, à acheter : son prix et ce qu'elle rapporterait (pleine de travailleurs).
struct CaseAchat {
    hex: Hex,
    /// Gisement connu de la case.
    gisement: Option<crate::gisements::Ressource>,
    nom: String,
    prix: i32,
    naturel: Rendements,
    /// Le meilleur aménagement possible et ce qu'il rapporterait.
    meilleur: Option<(Amenagement, Rendements)>,
}

/// Cases voisines du territoire de la ville, à personne : celles qu'elle peut acheter.
fn cases_a_acheter(map: &crate::map::WorldMap, centre: Hex, ville: Entity, territoire: &Territory, connus: &Connus) -> Vec<CaseAchat> {
    rules::cases_gagnables(map, ville, &territoire.0)
        .into_iter()
        .map(|h| {
            let meilleur = Amenagement::possibles(map, h).into_iter().map(|a| (a, intendant::rendement_max(map, h, Some(a)))).max_by(|a, b| a.1.total().total_cmp(&b.1.total()));
            CaseAchat { hex: h, gisement: connus.0.connu(map, h, connus.1), nom: nom_de_case(map, h), prix: rules::prix_case(map.distance(centre, h)), naturel: intendant::rendement_max(map, h, None), meilleur }
        })
        .collect()
}

// ------------------------------------------------------------------ Construction des éléments fixes

fn bouton(couleur: Color) -> impl Bundle {
    (
        UiPointer::default(),
        Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)), border_radius: BorderRadius::all(Val::Px(7.0)), border: UiRect::all(Val::Px(1.0)), align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() },
        BackgroundColor(couleur),
        BorderColor::all(charte::TRAIT2),
    )
}

fn panneau() -> Node {
    Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(12.0)), border_radius: BorderRadius::all(Val::Px(10.0)), min_width: Val::Px(0.0), ..default() }
}

/// Panneau posé sur la carte : fond opaque, bord, coins arrondis ; il prend le pointeur (la carte
/// dessous ne reçoit ni clic ni molette).
fn carte_flottante(node: Node) -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(10.0)), ..node },
        BackgroundColor(charte::FOND.with_alpha(0.96)),
        BorderColor::all(charte::TRAIT2),
        UiPointer::default(),
    )
}

fn construire(mut commands: Commands, images: Res<Images>) {
    let hexagone = images.hexagone.clone();
    // Étiquettes des cases : sous les panneaux, au-dessus des noms de la carte
    commands.spawn((
        Etiquettes,
        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), top: Val::Px(0.0), bottom: Val::Px(0.0), display: Display::None, ..default() },
        Pickable::IGNORE,
        GlobalZIndex(60),
    ));
    commands
        .spawn((
            Racine,
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), top: Val::Px(0.0), bottom: Val::Px(0.0), display: Display::None, ..default() },
            // (la racine laisse passer le pointeur vers la carte : seuls les panneaux le prennent)
            Pickable::IGNORE,
            GlobalZIndex(90),
        ))
        .with_children(|r| {
            // Gauche : la ville (en-tête, onglets, contenu qui défile, construction au pied)
            r.spawn((PanneauGauche, carte_flottante(Node { left: Val::Px(MARGE), top: Val::Px(HAUT), bottom: Val::Px(MARGE), width: Val::Px(400.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(12.0)), row_gap: Val::Px(8.0), ..default() })))
                .with_children(|g| {
                    g.spawn((EnTete, Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() }));
                    g.spawn((Node { column_gap: Val::Px(2.0), flex_shrink: 0.0, border: UiRect::bottom(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|o| {
                        for onglet in Onglet::TOUS {
                            o.spawn((
                                BoutonOnglet(onglet),
                                UiPointer::default(),
                                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)), border: UiRect::bottom(Val::Px(2.0)), ..default() },
                                BorderColor::all(Color::NONE),
                                children![texte(onglet.nom(), 14.5, charte::DOUX)],
                            ));
                        }
                    });
                    g.spawn((Gauche, Node { flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default()));
                    g.spawn((Pied, Node { flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::top(Val::Px(8.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT)));
                });
            // Barre de la carte : cases à acheter, caméra, couches
            r.spawn((BarreCarte, Node { position_type: PositionType::Absolute, top: Val::Px(HAUT + 4.0), left: Val::Px(440.0), column_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, row_gap: Val::Px(6.0), ..default() }, Pickable::IGNORE))
                .with_children(|b| {
                    b.spawn((BasculeAchats, bouton(charte::FOND), children![texte("Cases à acheter", 13.0, charte::TEXTE)]));
                    for (sens, signe) in [(1, "+"), (-1, "−"), (0, "Recentrer")] {
                        b.spawn((BoutonZoom(sens), bouton(charte::FOND))).with_children(|l| {
                            if sens == 0 {
                                l.spawn((Node { width: Val::Px(10.0), height: Val::Px(12.0), margin: UiRect::right(Val::Px(5.0)), ..default() }, ImageNode::new(hexagone.clone()).with_color(charte::TEXTE)));
                            }
                            l.spawn(texte(signe, if sens == 0 { 13.0 } else { 15.0 }, charte::TEXTE));
                        });
                    }
                    b.spawn((Couches, Node { column_gap: Val::Px(0.0), align_items: AlignItems::Center, padding: UiRect::left(Val::Px(8.0)), ..default() }));
                });
            // Légende de la carte (en bas à gauche de la carte)
            r.spawn((
                Legende,
                Node { position_type: PositionType::Absolute, left: Val::Px(440.0), bottom: Val::Px(80.0), max_width: Val::Px(700.0), flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(12.0), row_gap: Val::Px(4.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
                BackgroundColor(charte::FOND.with_alpha(0.9)),
                UiPointer::default(),
            ));
            // Validation d'un placement : au-dessus du tiroir, au milieu de la carte
            r.spawn((BarreValidation, Node { position_type: PositionType::Absolute, left: Val::Px(440.0), right: Val::Px(440.0), bottom: Val::Px(130.0), justify_content: JustifyContent::Center, display: Display::None, ..default() }, Pickable::IGNORE));
            // Droite : la fiche de ce qu'on touche ou règle
            r.spawn((
                Droite,
                carte_flottante(Node { right: Val::Px(MARGE), top: Val::Px(HAUT), bottom: Val::Px(FIN_DU_TOUR), width: Val::Px(400.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), padding: UiRect::all(Val::Px(12.0)), overflow: Overflow::scroll_y(), display: Display::None, ..default() }),
                ScrollPosition::default(),
            ));
            // Tiroir du bas : titre (et bascule), puis la liste ou les tableaux
            r.spawn((Tiroir, carte_flottante(Node { left: Val::Px(440.0), right: Val::Px(440.0), bottom: Val::Px(MARGE), height: Val::Px(BANDE), flex_direction: FlexDirection::Column, padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)), row_gap: Val::Px(6.0), ..default() })))
                .with_children(|t| {
                    t.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() }).with_children(|l| {
                        l.spawn((TitreListe, texte("", 14.5, charte::TEXTE), Node { flex_grow: 1.0, ..default() }));
                        l.spawn((BasculeAgrandi, bouton(charte::FOND), children![texte("Agrandir (F)", 12.5, charte::TEXTE)]));
                        l.spawn((BasculeTiroir, bouton(charte::FOND), children![texte("Ouvrir (Tab)", 12.5, charte::TEXTE)]));
                    });
                    t.spawn((Liste, Node { flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default()));
                });
            // Infobulle de la carte
            r.spawn((
                Bulle,
                Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(9.0), Val::Px(6.0)), max_width: Val::Px(280.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), display: Display::None, ..default() },
                BackgroundColor(charte::FOND),
                BorderColor::all(charte::TRAIT2),
                GlobalZIndex(95),
                Pickable::IGNORE,
            ));
            r.spawn((
                BulleAide,
                Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)), width: Val::Px(330.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), display: Display::None, ..default() },
                BackgroundColor(charte::FOND),
                BorderColor::all(charte::TRAIT2),
                GlobalZIndex(96),
                Pickable::IGNORE,
            ));
        });
}

/// Infobulle des grandeurs de l'en-tête (nourriture, production, deniers) : au survol, d'où vient
/// le total et où il va, à droite de la ligne.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn aide_grandeurs(
    ecran: Res<EcranVille>,
    window: Single<&Window, With<PrimaryWindow>>,
    lignes: Query<(&UiPointer, &AideGrandeur, &ComputedNode, &UiGlobalTransform)>,
    bulle: Single<(Entity, &mut Node), With<BulleAide>>,
    (map, territoire, amenagements, gisements, debug): (Res<GameMap>, Res<Territory>, Res<Amenagements>, Res<crate::game::Gisements>, Res<crate::game::DebugMode>),
    cities: Query<(Ref<City>, &HexPos)>,
    mut commands: Commands,
    mut montre: Local<Option<Grandeur>>,
) {
    let (e, mut node) = bulle.into_inner();
    let survol = lignes.iter().find(|(p, ..)| **p != UiPointer::None).map(|(_, a, n, t)| {
        let k = n.inverse_scale_factor();
        (a.0, Vec2::new((t.translation.x + n.size().x / 2.0) * k, (t.translation.y - n.size().y / 2.0) * k))
    });
    let ville = ecran.ville.and_then(|v| cities.get(v).ok().map(|c| (v, c)));
    let (Some((gr, p)), Some((v, (city, pos)))) = (survol.filter(|_| ecran.ouvert()), ville) else {
        if node.display != Display::None {
            node.display = Display::None;
        }
        *montre = None;
        return;
    };
    node.display = Display::Flex;
    node.left = Val::Px((p.x + 14.0).min(window.width() - 340.0));
    node.top = Val::Px((p.y - 8.0).min(window.height() - 380.0).max(0.0));
    if *montre != Some(gr) || city.is_changed() || ecran.is_changed() {
        *montre = Some(gr);
        let cases = cases_vues(&map.0, pos.0, v, &city, &territoire, &amenagements, &(&gisements, debug.0));
        commands.entity(e).despawn_related::<Children>().with_children(|b| bulle_grandeur(b, gr, &city, &cases));
    }
}

/// Place les panneaux selon la taille de la fenêtre et l'état du tiroir et de la fiche ; donne à
/// la carte sa zone libre (où la ville est cadrée).
#[allow(clippy::type_complexity)]
fn disposer(
    ecran: Res<EcranVille>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut dispo: ResMut<Disposition>,
    mut zone: ResMut<crate::ville_carte::ZoneLibre>,
    mut noeuds: ParamSet<(
        Single<&mut Node, With<PanneauGauche>>,
        Single<&mut Node, With<Droite>>,
        Single<&mut Node, With<Tiroir>>,
        Single<&mut Node, With<BarreCarte>>,
        Single<&mut Node, With<Legende>>,
        Single<&mut Node, With<BarreValidation>>,
    )>,
) {
    let w = Vec2::new(window.width(), window.height());
    // (1920 de large : 420 comme la maquette ; une petite fenêtre garde de la place à la carte)
    let large = (w.x * 0.215).clamp(320.0, 440.0).round();
    let voulu = Disposition { gauche: large, droite: large, tiroir: (w.y * 0.3).clamp(200.0, 320.0).round(), fenetre: w };
    if *dispo != voulu {
        *dispo = voulu;
    }
    let d = *dispo;
    let ouvert = ecran.tiroir_ouvert();
    let grand = (w.y * 0.6).round();
    let tiroir_h = if ecran.agrandi() { grand } else if ouvert { d.tiroir } else { BANDE };
    let x0 = MARGE + d.gauche + MARGE;
    let x1 = d.droite + 2.0 * MARGE;
    let ecrire = |node: &mut Node, f: &dyn Fn(&mut Node)| {
        let mut n = node.clone();
        f(&mut n);
        if *node != n {
            *node = n;
        }
    };
    ecrire(&mut noeuds.p0(), &|n| n.width = Val::Px(d.gauche));
    ecrire(&mut noeuds.p1(), &|n| n.width = Val::Px(d.droite));
    ecrire(&mut noeuds.p2(), &|n| {
        n.left = Val::Px(x0);
        n.right = Val::Px(x1);
        n.height = Val::Px(tiroir_h);
    });
    ecrire(&mut noeuds.p3(), &|n| {
        n.left = Val::Px(x0);
        n.max_width = Val::Px((w.x - x0 - x1).max(200.0));
    });
    ecrire(&mut noeuds.p4(), &|n| {
        n.left = Val::Px(x0);
        n.bottom = Val::Px(MARGE + tiroir_h + 8.0);
        n.max_width = Val::Px((w.x - x0 - x1).max(200.0));
    });
    ecrire(&mut noeuds.p5(), &|n| {
        n.left = Val::Px(x0);
        n.right = Val::Px(x1);
        n.top = Val::Px(HAUT + 44.0);
        n.bottom = Val::Auto;
    });
    // La ville est cadrée dans ce qui reste quand la fiche et le tiroir sont ouverts : la carte ne
    // saute pas quand on les ouvre ou les ferme
    let bas = if ecran.agrandi() { grand } else { d.tiroir };
    let libre = Rect::new(x0, HAUT + 44.0, (w.x - x1).max(x0 + 100.0), (w.y - MARGE - bas - 8.0).max(HAUT + 144.0));
    if zone.0 != libre {
        zone.0 = libre;
    }
}

/// Montre « Agrandir (F) » quand le tiroir ouvert a un graphique ; son libellé suit l'état.
fn bouton_agrandir(ecran: Res<EcranVille>, mut boutons: Query<(&mut Node, &Children), With<BasculeAgrandi>>, mut textes: Query<&mut Text>) {
    if !ecran.is_changed() {
        return;
    }
    for (mut node, enfants) in &mut boutons {
        let d = if ecran.graphique() && ecran.tiroir_ouvert() { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
        for &e in enfants {
            if let Ok(mut t) = textes.get_mut(e) {
                let voulu = if ecran.agrandi() { "Réduire (F)" } else { "Agrandir (F)" };
                if t.0 != voulu {
                    t.0 = voulu.into();
                }
            }
        }
    }
}

/// Molette sur le tiroir quand il montre un graphique : la période du passé (10, 20, 40 tours).
fn molette_graphique(mut ecran: ResMut<EcranVille>, scroll: Res<AccumulatedMouseScroll>, pointeurs: Query<(&PointerId, &PointerLocation)>, tiroir: Single<(&ComputedNode, &UiGlobalTransform), With<Tiroir>>) {
    if !ecran.ouvert() || scroll.delta.y == 0.0 || !ecran.graphique() || !ecran.tiroir_ouvert() || ecran.grandeur.is_none() {
        return;
    }
    let Some(souris) = pointeurs.iter().find(|(id, _)| **id == PointerId::Mouse).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position) else { return };
    let (noeud, transform) = *tiroir;
    if !noeud.contains_point(*transform, souris / noeud.inverse_scale_factor()) {
        return;
    }
    let periodes = [10, 20, 40];
    let i = periodes.iter().position(|&p| p == ecran.periode).unwrap_or(2) as i32;
    let j = (i - scroll.delta.y.signum() as i32).clamp(0, 2) as usize;
    if periodes[j] != ecran.periode {
        ecran.periode = periodes[j];
    }
}

/// Cache le HUD de la carte du monde (journal, panneaux, boutons) pendant que l'écran est ouvert.
fn cacher_hud(ecran: Res<EcranVille>, mut hud: Query<&mut Visibility, With<CacheSousEcran>>) {
    if !ecran.is_changed() {
        return;
    }
    let voulu = if ecran.ouvert() { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut hud {
        if *v != voulu {
            *v = voulu;
        }
    }
}

// ------------------------------------------------------------------ Ouverture, commandes

/// Fin de tour du joueur : acceptée si chaque ville a quelque chose à produire ; sinon la première
/// ville sans rien est choisie, son écran s'ouvre sur le catalogue, et le journal l'explique.
#[allow(clippy::too_many_arguments)]
fn fin_de_tour_joueur(
    mut demande: ResMut<FinDeTourJoueur>,
    mut end_turn: ResMut<crate::game::EndTurnRequested>,
    mut selection: ResMut<Selection>,
    mut ecran: ResMut<EcranVille>,
    mut log: ResMut<crate::game::GameLog>,
    cities: Query<(Entity, &City, &HexPos)>,
) {
    if !demande.0 {
        return;
    }
    demande.0 = false;
    let mut oisives: Vec<(Entity, &City, &HexPos)> = cities.iter().filter(|(_, c, _)| c.state.file.is_empty() && c.state.projet.is_none()).collect();
    oisives.sort_by(|a, b| a.1.state.name.cmp(&b.1.state.name));
    let Some((e, city, pos)) = oisives.first().copied() else {
        end_turn.0 = true;
        return;
    };
    let autres = if oisives.len() > 1 { format!(" (et {} autre(s) ville(s))", oisives.len() - 1) } else { String::new() };
    log.notify(
        crate::game::Importance::Warning,
        crate::game::LogText::new().city(&city.state.name, e, pos.0).text(format!(" n'a rien à produire{autres} : choisissez une construction ou un projet de ville pour finir le tour.")),
    );
    selection.0 = Some(e);
    ecran.ville = Some(e);
    ecran.derniere = Some(e);
    ecran.onglet = Onglet::Rendements;
    ecran.case = None;
    ecran.grandeur = None;
    ecran.catalogue = true;
    ecran.placement = None;
    ecran.cible = None;
    ecran.file_choisie = None;
    ecran.forcer = None;
}

/// Ouvre l'écran (touche C, bouton) pour la ville sélectionnée ; le ferme (Échap, bouton, plus de
/// ville sélectionnée).
#[allow(clippy::too_many_arguments)]
fn ouvrir_fermer(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    cities: Query<(), With<City>>,
    (wiki, menu): (Res<crate::wiki::WikiState>, Res<crate::menu::GameMenu>),
    mut ecran: ResMut<EcranVille>,
    mut vue_selection: Local<Option<Entity>>,
    fermer: Query<Ref<UiPointer>, With<Fermer>>,
    (changer, mut selection_mut, noms): (Query<(Ref<UiPointer>, &ChangerVille)>, ResMut<Selection>, Query<(Entity, &City, &crate::peuple::Peuple)>),
    (state, mut tour_vu): (Res<GameState>, Local<u32>),
) {
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    // Ville précédente ou suivante (celles du joueur, par nom) : l'onglet est gardé
    if let Some(v) = ecran.ville
        && let Some(sens) = changer.iter().find(|(i, _)| appui(i)).map(|(_, c)| c.0)
    {
        let mut miennes: Vec<(Entity, &str)> = noms.iter().filter(|(_, _, p)| **p == crate::peuple::Peuple::JOUEUR).map(|(e, c, _)| (e, c.state.name.as_str())).collect();
        miennes.sort_by(|a, b| a.1.cmp(b.1));
        if let Some(i) = miennes.iter().position(|(e, _)| *e == v) {
            let n = miennes.len() as i32;
            let j = (i as i32 + i32::from(sens)).rem_euclid(n) as usize;
            let autre = miennes[j].0;
            if autre != v {
                selection_mut.0 = Some(autre);
                ecran.ville = Some(autre);
                ecran.derniere = Some(autre);
                ecran.case = None;
                ecran.cible = None;
                ecran.placement = None;
                ecran.file_choisie = None;
                ecran.forcer = None;
            }
        }
        return;
    }
    let ville = selection_mut.0.filter(|e| cities.contains(*e));
    // (une ville du joueur qu'on vient de choisir sur la carte : son écran s'ouvre)
    let nouvelle = ville.is_some() && *vue_selection != ville && ville.and_then(|v| noms.get(v).ok()).is_some_and(|(_, _, p)| *p == crate::peuple::Peuple::JOUEUR);
    *vue_selection = ville;
    if *tour_vu != state.turn {
        *tour_vu = state.turn;
        if ecran.file_choisie.is_some() || ecran.forcer.is_some() {
            ecran.file_choisie = None;
            ecran.forcer = None;
        }
    }
    // (l'effectif proposé ne vaut que pour la case ouverte)
    if ecran.forcer.is_some_and(|(h, _)| ecran.case != Some(h)) {
        ecran.forcer = None;
    }
    if ecran.ouvert() {
        let echap = keys.just_pressed(KeyCode::Escape) && !wiki.open;
        if echap || fermer.iter().any(|i| appui(&i)) || ville != ecran.ville || keys.just_pressed(KeyCode::KeyC) && !wiki.open {
            if echap {
                // (Échap ferme d'abord la fiche de case ouverte, puis l'écran ; le menu ne s'ouvre pas)
                keys.reset(KeyCode::Escape);
                if ville == ecran.ville {
                    if ecran.cible.is_some() {
                        ecran.cible = None;
                        return;
                    }
                    if ecran.placement.is_some() {
                        ecran.placement = None;
                        return;
                    }
                    if ecran.catalogue {
                        ecran.catalogue = false;
                        return;
                    }
                }
                if ecran.case.is_some() && ville == ecran.ville {
                    ecran.case = None;
                    return;
                }
                if ecran.agrandi() && ville == ecran.ville {
                    ecran.agrandi = false;
                    return;
                }
                if ecran.sujet.is_some() && ville == ecran.ville {
                    ecran.sujet = None;
                    return;
                }
                if ecran.grandeur.is_some() && ville == ecran.ville {
                    ecran.grandeur = None;
                    return;
                }
            }
            ecran.ville = None;
            ecran.case = None;
            ecran.grandeur = None;
            ecran.catalogue = false;
            ecran.placement = None;
            ecran.cible = None;
            return;
        }
        // B : catalogue de construction ; Entrée : mettre en file la case choisie
        if keys.just_pressed(KeyCode::KeyB) && !wiki.open {
            ecran.catalogue = !ecran.catalogue;
            ecran.placement = None;
            ecran.cible = None;
            ecran.onglet = Onglet::Rendements;
            ecran.case = None;
        }
        if keys.just_pressed(KeyCode::Tab) && !wiki.open {
            ecran.basculer_tiroir();
        }
        if keys.just_pressed(KeyCode::KeyF) && !wiki.open && ecran.graphique() {
            if !ecran.tiroir_ouvert() {
                ecran.basculer_tiroir();
                ecran.agrandi = true;
            } else {
                ecran.agrandi = !ecran.agrandi;
            }
        }
        if (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && ecran.placement.is_some() {
            ecran.confirmer = true;
        }
        // 1 à 3 : détail de la nourriture, de la production, des deniers
        for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].into_iter().enumerate() {
            // (pendant la construction, elles choisiraient une grandeur cachée)
            if keys.just_pressed(k) && !wiki.open && !ecran.catalogue {
                let g = DETAILLABLES[i];
                ecran.grandeur = if ecran.grandeur == Some(g) { None } else { Some(g) };
                ecran.sujet = None;
                ecran.case = None;
            }
        }
    } else if let Some(v) = ville
        && !wiki.open
        && !menu.open
        && (keys.just_pressed(KeyCode::KeyC) || nouvelle)
    {
        if ecran.derniere != Some(v) {
            ecran.onglet = Onglet::Rendements;
            ecran.histoire_genre = None;
            ecran.derniere = Some(v);
        }
        ecran.ville = Some(v);
        ecran.case = None;
        ecran.grandeur = None;
        ecran.catalogue = false;
        ecran.placement = None;
        ecran.cible = None;
        ecran.file_choisie = None;
        ecran.forcer = None;
        ecran.couche = None;
    }
}

/// Cases que le cadrage montre : le territoire de la ville et ses voisines (cases à acheter).
fn cases_du_cadrage(v: Entity, territoire: &Territory, positions: &Query<&HexPos>) -> Vec<Hex> {
    let mut cases: Vec<Hex> = positions.get(v).map(|p| vec![p.0]).unwrap_or_default();
    cases.extend(territoire.0.iter().filter(|(_, e)| **e == v).map(|(h, _)| *h));
    cases
}

/// Cadre la ville dans la zone libre quand l'écran s'ouvre, change de ville ou de vue ; la couche
/// Échanges recule pour montrer aussi les villes avec qui elle échange.
fn cadrage(ecran: Res<EcranVille>, mode: Res<crate::view3d::ViewMode>, map: Res<GameMap>, territoire: Res<Territory>, positions: Query<&HexPos>, cities: Query<(&City, &HexPos)>, mut cadrer: MessageWriter<crate::ville_carte::CadrerVille>, mut vu: Local<(Option<Entity>, bool, bool, bool)>) {
    let echanges = ecran.ouvert() && ecran.placement.is_none() && couche_vue(&ecran) == Couche::Echanges;
    let cle = (ecran.ville, mode.is_3d, echanges, ecran.agrandi());
    if *vu == cle {
        return;
    }
    *vu = cle;
    if let Some(v) = ecran.ville {
        let mut cases = cases_du_cadrage(v, &territoire, &positions);
        if echanges && let Ok((city, _)) = cities.get(v) {
            // (les villes reliées, et celles avec qui elle a échangé ce tour)
        let ici = positions.get(v).map(|p| p.0).ok();
        cases.extend(cities.iter().filter(|(c, p)| c.state.name != city.state.name && (city.state.echanges.iter().any(|x| x.ville == c.state.name) || ici.is_some_and(|h| map.0.distance(h, p.0) <= crate::echanges::CASES_MAX))).map(|(_, p)| p.0));
        }
        cadrer.write(crate::ville_carte::CadrerVille(cases));
    }
}

/// Boutons de l'écran : onglets, zoom, cases de la liste, fiche, couches. Couleurs au survol.
#[allow(clippy::type_complexity)]
fn commandes(
    mut ecran: ResMut<EcranVille>,
    (mode, mut cam3d, mut camera2d, mut cadrer, territoire, positions): (Res<crate::view3d::ViewMode>, ResMut<crate::view3d::Cam3d>, Single<&mut Projection, (With<Camera2d>, Without<crate::seam::GhostCamera>)>, MessageWriter<crate::ville_carte::CadrerVille>, Res<Territory>, Query<&HexPos>),
    (tiroir, achats, choix_couches, ouvrir_onglet, sujets, horizons): (Query<Ref<UiPointer>, With<BasculeTiroir>>, Query<Ref<UiPointer>, With<BasculeAchats>>, Query<(Ref<UiPointer>, &ChoixCouche)>, Query<(Ref<UiPointer>, &OuvrirOnglet)>, Query<(Ref<UiPointer>, &ChoixSujet)>, Query<(Ref<UiPointer>, &ChoixHorizon)>),
    ressources: Query<(Ref<UiPointer>, &ChoixRessource)>,
    bascule_remedes: Query<Ref<UiPointer>, With<BasculeRemedes>>,
    agrandir: Query<Ref<UiPointer>, With<BasculeAgrandi>>,
    mut onglets: Query<(Ref<UiPointer>, &BoutonOnglet, &mut BorderColor, &Children)>,
    mut textes: Query<&mut TextColor>,
    zoom: Query<(Ref<UiPointer>, &BoutonZoom)>,
    choix: Query<(Ref<UiPointer>, &ChoixCase)>,
    fiche: Query<Ref<UiPointer>, With<FermerFiche>>,
    couches: Query<Ref<UiPointer>, With<BasculeAmenagements>>,
    (grandeurs, periodes, fils): (Query<(Ref<UiPointer>, &ChoixGrandeur)>, Query<(Ref<UiPointer>, &ChoixPeriode)>, Query<(Ref<UiPointer>, &Fil)>),
    (catalogue, construire, ici): (Query<(Ref<UiPointer>, &ChoixCatalogue)>, Query<Ref<UiPointer>, With<BoutonConstruire>>, Query<(Ref<UiPointer>, &AmenagerIci)>),
    (cartes_pop, genres, remedes, groupes): (Query<(Ref<UiPointer>, &ChoixCartePop)>, Query<(Ref<UiPointer>, &ChoixGenre)>, Query<(Ref<UiPointer>, &BoutonRemede)>, Query<(Ref<UiPointer>, &ChoixGroupe)>),
    mut survols: Query<(&UiPointer, &mut BackgroundColor), (Or<(With<ChoixCase>, With<OuvrirOnglet>, With<ChoixGrandeur>, With<ChoixSujet>, With<ChoixRessource>, With<Fermer>, With<FermerFiche>, With<BasculeAmenagements>, With<Fil>, With<ChoixCatalogue>, With<ActionFile>, With<ArreterProjet>, With<AmenagerIci>, With<ChoixReglage>, With<BoutonRemede>)>, Changed<UiPointer>)>,
) {
    if !ecran.ouvert() {
        return;
    }
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    for (i, b, mut bord, enfants) in &mut onglets {
        if appui(&i) && ecran.onglet != b.0 {
            ecran.onglet = b.0;
            ecran.sujet = None;
            ecran.grandeur = None;
            ecran.couche = None;
            ecran.catalogue = false;
            ecran.placement = None;
            ecran.cible = None;
        }
        let actif = ecran.onglet == b.0;
        let c = if actif { charte::TEXTE } else { Color::NONE };
        if bord.bottom != c {
            *bord = BorderColor { bottom: c, ..BorderColor::all(Color::NONE) };
        }
        for &e in enfants {
            if let Ok(mut t) = textes.get_mut(e) {
                let c = if actif || *i != UiPointer::None { charte::TEXTE } else { charte::DOUX };
                if t.0 != c {
                    t.0 = c;
                }
            }
        }
    }
    for (i, b) in &zoom {
        if !appui(&i) {
            continue;
        }
        let k = if b.0 > 0 { 1.0 / 1.3 } else { 1.3 };
        match b.0 {
            0 => {
                if let Some(v) = ecran.ville {
                    cadrer.write(crate::ville_carte::CadrerVille(cases_du_cadrage(v, &territoire, &positions)));
                }
            }
            _ if mode.is_3d => cam3d.distance = (cam3d.distance * k).clamp(150.0, 2_800.0),
            _ => {
                if let Projection::Orthographic(o) = camera2d.as_mut() {
                    o.scale = (o.scale * k).clamp(0.3, 8.0);
                }
            }
        }
    }
    if tiroir.iter().any(|i| appui(&i)) {
        ecran.basculer_tiroir();
    }
    if agrandir.iter().any(|i| appui(&i)) {
        ecran.agrandi = !ecran.agrandi;
    }
    for (i, r) in &ressources {
        if appui(&i) {
            let s = Sujet::Ressource(r.0);
            ecran.sujet = if ecran.sujet == Some(s) && ecran.onglet == Onglet::Ressources { None } else { Some(s) };
            ecran.onglet = Onglet::Ressources;
            ecran.grandeur = None;
            ecran.case = None;
            ecran.catalogue = false;
            ecran.placement = None;
        }
    }
    for (i, s) in &sujets {
        if appui(&i) {
            ecran.sujet = if ecran.sujet == Some(s.0) { None } else { Some(s.0) };
            ecran.grandeur = None;
            ecran.case = None;
        }
    }
    for (i, o) in &ouvrir_onglet {
        if appui(&i) {
            ecran.onglet = o.0;
            ecran.couche = None;
            ecran.grandeur = None;
            ecran.case = None;
            ecran.catalogue = false;
            ecran.placement = None;
            ecran.cible = None;
        }
    }
    if achats.iter().any(|i| appui(&i)) {
        ecran.achats_caches = !ecran.achats_caches;
    }
    for (i, c) in &choix_couches {
        if appui(&i) && ecran.couche != Some(c.0) {
            ecran.couche = Some(c.0);
        }
    }
    for (i, h) in &horizons {
        if appui(&i) && ecran.horizon != h.0 {
            ecran.horizon = h.0;
        }
    }
    for (i, c) in &choix {
        if appui(&i) {
            ecran.case = if ecran.case == Some(c.0) { None } else { Some(c.0) };
        }
    }
    if fiche.iter().any(|i| appui(&i)) {
        // (la fiche ouverte se ferme : la case d'abord, puis la cause, puis la grandeur)
        if ecran.case.is_some() {
            ecran.case = None;
        } else if ecran.sujet.is_some() {
            ecran.sujet = None;
        } else {
            ecran.grandeur = None;
        }
    }
    if couches.iter().any(|i| appui(&i)) {
        ecran.amenagements_caches = !ecran.amenagements_caches;
    }
    for (i, g) in &grandeurs {
        if appui(&i) {
            ecran.grandeur = if ecran.grandeur == Some(g.0) && ecran.case.is_none() { None } else { Some(g.0) };
            ecran.sujet = None;
            ecran.case = None;
            ecran.catalogue = false;
            ecran.placement = None;
        }
    }
    for (i, p) in &periodes {
        if appui(&i) && ecran.periode != p.0 {
            ecran.periode = p.0;
        }
    }
    for (i, f) in &fils {
        if appui(&i) {
            ecran.case = None;
            if f.0 == 0 {
                ecran.grandeur = None;
            }
        }
    }
    if construire.iter().any(|i| appui(&i)) {
        ecran.catalogue = !ecran.catalogue;
        ecran.placement = None;
        ecran.cible = None;
        ecran.case = None;
        ecran.onglet = Onglet::Rendements;
    }
    for (i, c) in &catalogue {
        if appui(&i)
            && let Element::Amenagement(a) = c.0
        {
            // (recliquer l'aménagement en cours ne l'annule plus : Échap ou clic droit reviennent)
            if ecran.placement != Some(a) {
                ecran.placement = Some(a);
            }
            ecran.cible = None;
            ecran.case = None;
        }
    }
    for (i, gr) in &groupes {
        if appui(&i) && ecran.catalogue_groupe != gr.0 {
            ecran.catalogue_groupe = gr.0;
        }
    }
    if bascule_remedes.iter().any(|i| appui(&i)) {
        ecran.remedes_ouverts = !ecran.remedes_ouverts;
    }
    for (i, r) in &remedes {
        if !appui(&i) {
            continue;
        }
        ecran.remedes_ouverts = false;
        ecran.onglet = Onglet::Rendements;
        ecran.case = None;
        ecran.cible = None;
        match r.0 {
            Remede::Placer(a) => {
                ecran.catalogue = true;
                ecran.catalogue_groupe = GroupeCatalogue::Amenagements;
                ecran.placement = Some(a);
            }
            Remede::PlacerSur(a, h) => {
                ecran.catalogue = true;
                ecran.catalogue_groupe = GroupeCatalogue::Amenagements;
                ecran.placement = Some(a);
                ecran.cible = Some(h);
            }
            Remede::Detail(g) => {
                ecran.catalogue = false;
                ecran.placement = None;
                ecran.grandeur = Some(g);
            }
            Remede::Construire => {
                ecran.catalogue = true;
                ecran.placement = None;
            }
        }
    }
    for (i, g) in &genres {
        if appui(&i) && ecran.histoire_genre != g.0 {
            ecran.histoire_genre = g.0;
        }
    }
    for (i, c) in &cartes_pop {
        if appui(&i) && ecran.carte_habitants != c.0 {
            ecran.carte_habitants = c.0;
            ecran.couche = Some(if c.0 { Couche::Habitants } else { Couche::Emplois });
        }
    }
    for (i, a) in &ici {
        if appui(&i) {
            ecran.catalogue = true;
            ecran.placement = Some(a.0);
            ecran.cible = ecran.case.take();
            ecran.onglet = Onglet::Rendements;
        }
    }
    for (i, mut fond) in &mut survols {
        let a = fond.0.alpha();
        if a > 0.0 || *i != UiPointer::None {
            fond.0 = if *i == UiPointer::None { Color::NONE } else { charte::SURFACE2 };
        }
    }
}

/// Molette sur une colonne de l'écran (gauche, liste de droite) : la fait défiler.
fn defiler(ecran: Res<EcranVille>, scroll: Res<AccumulatedMouseScroll>, pointeurs: Query<(&PointerId, &PointerLocation)>, mut colonnes: Query<(&mut ScrollPosition, &ComputedNode, &UiGlobalTransform), Or<(With<Gauche>, With<Liste>, With<Droite>)>>) {
    if !ecran.ouvert() || scroll.delta.y == 0.0 {
        return;
    }
    let Some(souris) = pointeurs.iter().find(|(id, _)| **id == PointerId::Mouse).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position) else { return };
    let pas = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y * 48.0,
        MouseScrollUnit::Pixel => scroll.delta.y,
    };
    for (mut position, noeud, transform) in &mut colonnes {
        let k = noeud.inverse_scale_factor();
        if !noeud.contains_point(*transform, souris / k) {
            continue;
        }
        // (pas au-delà du contenu)
        let bout = ((noeud.content_size().y - noeud.size().y) * k).max(0.0);
        let y = (position.y - pas).clamp(0.0, bout);
        if y != position.y {
            position.y = y;
        }
    }
}

/// Souris sur la carte du monde : survol (infobulle, contour blanc), clic (fiche de la case ou de
/// la case à acheter ; placement : choisir, double-clic : mettre en file), clic droit (revenir d'un
/// cran pendant un placement). La caméra garde ses commandes (molette, bouton du milieu, clavier).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn carte_souris(
    mut ecran: ResMut<EcranVille>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    (survole, over_ui, mut survol_carte): (Res<crate::input::HoveredHex>, Res<crate::input::PointerOverUi>, ResMut<crate::ville_carte::SurvolVille>),
    (map, territoire, amenagements, mut cache, gisements, debug): (Res<GameMap>, Res<Territory>, Res<Amenagements>, ResMut<CacheVille>, Res<crate::game::Gisements>, Res<crate::game::DebugMode>),
    cities: Query<(&City, &HexPos)>,
    choix: Query<(&UiPointer, &ChoixCase)>,
    bulle: Single<(Entity, &mut Node), With<Bulle>>,
    mut commands: Commands,
    mut appui: Local<Option<Vec2>>,
    mut montre: Local<Option<Hex>>,
    (time, mut dernier_clic, lignes): (Res<Time>, Local<(f64, Option<Hex>)>, Query<(&UiPointer, &LigneFile)>),
    mut pilot: ResMut<crate::input::Pilot>,
) {
    // (le pilote clique sur une case : le clic est pour l'écran, la carte du monde ne le voit pas)
    let clic_pilote = if pilot.active && ecran.ouvert() { pilot.click.take() } else { None };
    let (bulle_e, mut bulle_node) = bulle.into_inner();
    let Some((city, centre)) = ecran.ville.and_then(|v| cities.get(v).ok().map(|c| (v, c))).map(|(v, (c, p))| ((v, c), p.0)) else {
        *appui = None;
        if survol_carte.0.is_some() {
            survol_carte.0 = None;
        }
        return;
    };
    let cases = cases_vues(&map.0, centre, city.0, city.1, &territoire, &amenagements, &(&gisements, debug.0));
    let achats = if ecran.achats_caches { Vec::new() } else { cases_a_acheter(&map.0, centre, city.0, &territoire, &(&gisements, debug.0)) };
    // (les cases à acheter ne se choisissent pas pendant la construction)
    let construction = ecran.catalogue;
    let souris = window.cursor_position().or(pilot.active.then_some(Vec2::new(-1.0, -1.0)));
    let dessus = !over_ui.0 && souris.is_some();
    let sous = survole.0.filter(|_| dessus);
    let vise = sous.and_then(|h| cases.iter().find(|c| c.hex == h));
    let achat_sous = sous.and_then(|h| achats.iter().find(|a| a.hex == h)).filter(|_| !construction);
    // Clic droit en placement : revenir d'un cran (case choisie, puis aménagement)
    if dessus && (mouse.just_pressed(MouseButton::Right) || clic_pilote == Some(MouseButton::Right)) && ecran.placement.is_some() {
        if ecran.cible.is_some() {
            ecran.cible = None;
        } else {
            ecran.placement = None;
        }
    }
    // Clic : appui et relâché au même endroit (un glisser déplace la caméra, il ne choisit rien)
    if dessus && mouse.just_pressed(MouseButton::Left) {
        *appui = souris;
    }
    if mouse.just_released(MouseButton::Left) || clic_pilote == Some(MouseButton::Left) {
        let clic = clic_pilote == Some(MouseButton::Left) || appui.take().zip(souris).is_some_and(|(a, b)| a.distance(b) < 5.0) && dessus;
        if clic {
            if let Some(a) = ecran.placement {
                // Placement : la case doit pouvoir porter l'aménagement ; double-clic : mise en file
                if let Some(x) = vise.filter(|x| possible(&map.0, x, a).is_ok()) {
                    let maintenant = time.elapsed_secs_f64();
                    if dernier_clic.1 == Some(x.hex) && maintenant - dernier_clic.0 < 0.45 {
                        ecran.confirmer = true;
                    }
                    *dernier_clic = (maintenant, Some(x.hex));
                    if ecran.cible != Some(x.hex) {
                        ecran.cible = Some(x.hex);
                    }
                }
            } else if !ecran.catalogue {
                // (une case voisine à acheter s'ouvre aussi : sa fiche propose l'achat)
                let hex = vise.map(|x| x.hex).or_else(|| achat_sous.map(|x| x.hex));
                if hex.is_some() {
                    ecran.case = if hex == ecran.case { None } else { hex };
                }
            }
        }
    }

    // Survol : par la carte, ou par une ligne de la liste ou de la file
    let par_liste = choix
        .iter()
        .find(|(i, _)| **i != UiPointer::None)
        .map(|(_, c)| c.0)
        .or_else(|| lignes.iter().find(|(i, _)| **i != UiPointer::None).and_then(|(_, l)| city.1.state.file.get(l.0)).and_then(|k| case_de(&k.objet)).map(|h| map.0.canonical(h)));
    let survol = if dessus { vise } else { par_liste.and_then(|h| cases.iter().find(|x| x.hex == h)) };
    let contour = survol.map(|c| c.hex).or(achat_sous.map(|a| a.hex));
    if survol_carte.0 != contour {
        survol_carte.0 = contour;
    }
    // Infobulle (seulement au survol de la carte, sous le vrai pointeur)
    match (dessus, survol, window.cursor_position()) {
        (true, None, Some(p)) if let Some(a) = achat_sous => {
            bulle_node.display = Display::Flex;
            bulle_node.left = Val::Px((p.x + 14.0).min(window.width() - 290.0));
            bulle_node.top = Val::Px((p.y + 14.0).min(window.height() - 140.0));
            if *montre != Some(a.hex) || ecran.is_changed() {
                *montre = Some(a.hex);
                commands.entity(bulle_e).despawn_related::<Children>().with_children(|b| infobulle_achat(b, a));
            }
        }
        (true, Some(c), Some(p)) => {
            bulle_node.display = Display::Flex;
            bulle_node.left = Val::Px((p.x + 14.0).min(window.width() - 290.0));
            bulle_node.top = Val::Px((p.y + 14.0).min(window.height() - 140.0));
            if *montre != Some(c.hex) || ecran.is_changed() {
                *montre = Some(c.hex);
                commands.entity(bulle_e).despawn_related::<Children>();
                let vl = Ville::new(&mut cache, &map.0, centre, city.0, city.1, &territoire, &amenagements);
                let placement = ecran.placement.map(|a| (a, gain_amenagement(&vl, c, a)));
                let couche = couche_vue(&ecran);
                commands.entity(bulle_e).with_children(|b| match placement {
                    Some((a, g)) => infobulle_placement(b, c, a, g, &Bareme::de(&vl)),
                    None if couche == Couche::Histoire => {
                        b.spawn(texte(c.nom.clone(), 14.0, charte::TEXTE));
                        let ev: Vec<&rules::Evenement> = city.1.state.chronique.iter().filter(|e| e.lieu == Some(c.hex)).collect();
                        if ev.is_empty() {
                            b.spawn(texte("aucun événement ici", 12.5, charte::MUET));
                        }
                        for e in ev {
                            b.spawn(texte(format!("tour {} : {}", e.tour, e.texte), 12.5, charte::DOUX));
                        }
                    }
                    None => infobulle(b, c, couche),
                });
            }
        }
        _ => {
            if bulle_node.display != Display::None {
                bulle_node.display = Display::None;
            }
            *montre = None;
        }
    }
}

/// Couche de la carte montrée : celle choisie, sinon celle de l'onglet (et de la grandeur).
fn couche_vue(ecran: &EcranVille) -> Couche {
    let proposees = couches_proposees(ecran);
    ecran.couche.filter(|c| proposees.contains(c)).unwrap_or(proposees[0])
}

/// Couches proposées dans la barre de la carte, la première par défaut.
fn couches_proposees(ecran: &EcranVille) -> Vec<Couche> {
    match (ecran.onglet, ecran.grandeur) {
        (_, Some(Grandeur::Nourriture)) if !ecran.catalogue => vec![Couche::Quantite, Couche::Types, Couche::Amenagements],
        (_, Some(_)) if !ecran.catalogue => vec![Couche::Quantite, Couche::Amenagements, Couche::Emplois],
        (Onglet::Ressources, _) => vec![Couche::Gisements, Couche::Echanges, Couche::Amenagements],
        (Onglet::Rendements, _) => vec![Couche::Rendement, Couche::Amenagements, Couche::Emplois],
        (Onglet::Population, _) => vec![Couche::Emplois, Couche::Habitants, Couche::Amenagements],
        (Onglet::Histoire, _) => vec![Couche::Histoire, Couche::Amenagements],
    }
}

/// Infobulle d'une case voisine à acheter.
fn infobulle_achat(b: &mut ChildSpawnerCommands, a: &CaseAchat) {
    b.spawn(texte(a.nom.clone(), 14.0, charte::TEXTE));
    b.spawn(texte(format!("Hors du territoire : à acheter pour {} deniers", a.prix), 13.0, Grandeur::Deniers.couleur()));
    // (une ligne chacune : nourriture et production, pleine de travailleurs)
    let court = |r: &Rendements| [(r.nourriture, "nourr."), (r.production, "prod."), (r.deniers, "den.")].iter().filter(|(v, _)| *v > 0.05).map(|(v, n)| format!("{} {n}", rules::decimal(*v))).collect::<Vec<_>>().join(" · ");
    b.spawn(texte(format!("naturelle : {}", court(&a.naturel)), 12.5, charte::DOUX));
    if let Some((am, r)) = a.meilleur {
        b.spawn(texte(format!("{} : {}", capitale(am.nom()), court(&r)), 12.5, charte::AMENAGEMENT));
    }
    b.spawn(texte("clic : sa fiche", 12.0, charte::MUET));
}

/// Infobulle d'une case pendant le placement d'un aménagement.
fn infobulle_placement(b: &mut ChildSpawnerCommands, c: &CaseVue, a: Amenagement, g: Result<GainAmenagement, &'static str>, bareme: &Bareme) {
    b.spawn(texte(c.nom.clone(), 14.0, charte::TEXTE));
    match g {
        Ok(g) => {
            let e = evaluation(&g, bareme);
            b.spawn(texte(format!("{} : valeur {}", capitale(a.nom()), signe(e.valeur)), 13.5, charte::AMENAGEMENT));
            b.spawn(texte(format!("tout de suite : {}", ecarts(&g.maintenant)), 12.5, charte::TEXTE));
            b.spawn(texte(format!("dans {TOURS_EFFET} tours : {}", ecarts(&g.dans)), 12.5, charte::TEXTE));
            b.spawn(texte(format!("maximum de la ville (case pleine) : {}", ecarts(&g.plein)), 12.5, charte::DOUX));
            b.spawn(texte(format!("coût {} production{}", g.cout, e.rentabilite.map_or(String::new(), |r| format!(" · rentabilisé en {} tours", r.ceil()))), 12.5, charte::DOUX));
            if g.defriche {
                b.spawn(texte("défrichement : la forêt est perdue pour le bois", 12.5, charte::ALERTE));
            }
            if let Some(x) = c.amenagement {
                b.spawn(texte(format!("remplace : {}", x.nom()), 12.5, charte::ALERTE));
            }
            if let Some((t, couleur)) = conflit_gisement(c, Some(a)) {
                b.spawn(texte(t, 12.5, couleur));
            }
            match e.risque {
                Risque::Famine(k) => {
                    b.spawn(texte(format!("famine : la nourriture tomberait à {} × le besoin", rules::decimal(k)), 12.5, charte::GRAVE));
                }
                Risque::BordDeFamine(k) => {
                    b.spawn(texte(format!("bord de famine : {} × le besoin après", rules::decimal(k)), 12.5, charte::GRAVE));
                }
                Risque::Aucun => {}
            }
            b.spawn(texte("clic : choisir · double-clic ou Entrée : mettre en file", 12.0, charte::MUET));
        }
        Err(e) => {
            b.spawn(texte(format!("impossible : {e}"), 13.0, charte::GRAVE));
        }
    }
}

/// Contenu de l'infobulle d'une case, selon la couche affichée.
fn infobulle(b: &mut ChildSpawnerCommands, c: &CaseVue, couche: Couche) {
    b.spawn(texte(c.nom.clone(), 14.0, charte::TEXTE));
    if couche == Couche::Habitants {
        b.spawn(texte(format!("{} habitants", rules::milliers(c.habitants.round() as u64)), 13.0, Grandeur::Habitants.couleur()));
        b.spawn(texte(format!("{} travailleurs sur la case", rules::milliers(c.travailleurs.into())), 12.5, charte::MUET));
    } else if couche == Couche::Emplois {
        let t = if c.centre { "centre de la ville : rapporte sans travailleurs".into() } else { format!("{} / {} travailleurs ({} %)", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into()), (c.remplissage() * 100.0).round()) };
        b.spawn(texte(t, 13.0, charte::DOUX));
    } else {
        b.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|l| valeurs(l, &c.rendement, 13.0));
        if !c.centre {
            b.spawn(texte(format!("{} / {} travailleurs", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())), 12.5, charte::MUET));
        }
    }
    if let Some(a) = c.amenagement {
        b.spawn(texte(format!("Aménagement : {}", a.nom().to_lowercase()), 12.5, charte::AMENAGEMENT));
    }
    b.spawn(texte("clic : sa fiche", 12.0, charte::MUET));
}

/// Trois valeurs (nourriture, production, deniers) dans leurs couleurs.
fn valeurs(l: &mut ChildSpawnerCommands, r: &Rendements, taille: f32) {
    for (g, v) in [(Grandeur::Nourriture, r.nourriture), (Grandeur::Production, r.production), (Grandeur::Deniers, r.deniers)] {
        if v > 0.05 {
            l.spawn(texte(format!("{} {}", rules::decimal(v), g.nom().to_lowercase()), taille, g.couleur()));
        }
    }
}

fn afficher(ecran: Res<EcranVille>, mut racines: Query<&mut Node, Or<(With<Racine>, With<Etiquettes>)>>) {
    let d = if ecran.ouvert() { Display::Flex } else { Display::None };
    for mut racine in &mut racines {
        if racine.display != d {
            racine.display = d;
        }
    }
}

// ------------------------------------------------------------------ Contenu

/// Refait le contenu quand l'écran, la ville, ses aménagements, le tour, la disposition ou la
/// taille des cases à l'écran changent : panneaux, couche et contours de la carte, étiquettes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reconstruire(
    mut commands: Commands,
    ecran: Res<EcranVille>,
    images: Res<Images>,
    (map, territoire, amenagements, state, mut cache, gisements, debug): (Res<GameMap>, Res<Territory>, Res<Amenagements>, Res<GameState>, ResMut<CacheVille>, Res<crate::game::Gisements>, Res<crate::game::DebugMode>),
    cities: Query<(Ref<City>, &HexPos)>,
    autres_villes: Query<(Entity, &City, &HexPos, &crate::peuple::Peuple)>,
    (entete, gauche, droite, pied, legende, titre, liste): (Single<Entity, With<EnTete>>, Single<Entity, With<Gauche>>, Single<(Entity, &mut Node), With<Droite>>, Single<Entity, With<Pied>>, Single<(Entity, &mut Node), (With<Legende>, Without<Droite>)>, Single<Entity, With<TitreListe>>, Single<Entity, With<Liste>>),
    (barre, couches_barre, etiquettes, bascule): (Single<(Entity, &mut Node), (With<BarreValidation>, Without<Droite>, Without<Legende>)>, Single<Entity, With<Couches>>, Single<Entity, With<Etiquettes>>, Single<&Children, With<BasculeTiroir>>),
    (dispo, taille_case, mut couche_carte): (Res<Disposition>, Res<crate::ville_carte::TailleCase>, ResMut<crate::ville_carte::CoucheVille>),
    mut textes: Query<&mut Text>,
    mut assets: ResMut<Assets<Image>>,
    (mut derniere, mut graphique): (Local<(u32, f32, Option<Disposition>)>, Local<Vec<Handle<Image>>>),
) {
    let Some(v) = ecran.ville else {
        if couche_carte.ouverte {
            couche_carte.ouverte = false;
        }
        return;
    };
    let Ok((city, centre)) = cities.get(v) else { return };
    let s = (taille_case.0 * 10.0).round() / 10.0;
    let cle = (state.turn, s, Some(*dispo));
    let change = ecran.is_changed() || city.is_changed() || amenagements.is_changed() || territoire.is_changed() || gisements.is_changed() || debug.is_changed() || *derniere != cle;
    if !change {
        return;
    }
    *derniere = cle;
    // (une reconstruction lente se lit dans la console)
    let _mesure = Mesure(std::time::Instant::now());
    let cases = cases_vues(&map.0, centre.0, v, &city, &territoire, &amenagements, &(&gisements, debug.0));
    let choisie = ecran.case.and_then(|h| cases.iter().find(|c| c.hex == h));
    let achats = if ecran.achats_caches { Vec::new() } else { cases_a_acheter(&map.0, centre.0, v, &territoire, &(&gisements, debug.0)) };
    let achat_choisi = ecran.case.and_then(|h| achats.iter().find(|a| a.hex == h)).filter(|_| !ecran.catalogue);

    let partenaires = partenaires(&map.0, v, &autres_villes);
    // Une seule prévision pour l'en-tête, le bandeau et les graphiques (décision B4)
    let vl = Ville::new(&mut cache, &map.0, centre.0, v, &city, &territoire, &amenagements);
    let (dispo_ville, prevision) = (vl.dispo, vl.prevision);
    let diagnostic = frein(&vl, &city, &cases);
    commands.entity(*entete).despawn_related::<Children>().with_children(|e| {
        en_tete(e, &city, prevision);
        bandeau_frein(e, &diagnostic, ecran.remedes_ouverts);
        grandeurs(e, &city, state.tresor_joueur(), ecran.grandeur);
        jetons_besoins(e, &city);
    });
    // Graphiques (images refaites ; les anciennes sont libérées)
    for h in graphique.drain(..) {
        assets.remove(&h);
    }
    // Aperçus de l'intendant : la répartition refaite avec d'autres consignes
    let apercu = |cons: &intendant::Consignes| -> Rendements {
        let mut st = city.state.clone();
        st.consignes = cons.clone();
        rules::repartition_ville(&map.0, centre.0, &st, dispo_ville).rendements
    };
    let reglages: Vec<(Reglage, Rendements, i64)> = match ecran.grandeur {
        Some(gr) if choisie.is_none() && !ecran.catalogue => {
            let options: Vec<Reglage> = match gr {
                Grandeur::Nourriture => intendant::ObjectifNourriture::ALL.into_iter().map(Reglage::Objectif).collect(),
                Grandeur::Production => intendant::Priorite::ALL.into_iter().map(|p| Reglage::Priorite(1, p)).collect(),
                _ => intendant::Priorite::ALL.into_iter().map(|p| Reglage::Priorite(2, p)).collect(),
            };
            options
                .into_iter()
                .map(|o| {
                    let mut cons = city.state.consignes.clone();
                    match o {
                        Reglage::Objectif(x) => cons.objectif = x,
                        Reglage::Priorite(i, x) => cons.priorites[i] = x,
                    }
                    let r = apercu(&cons);
                    (o, r, city.state.variation(r))
                })
                .collect()
        }
        _ => Vec::new(),
    };
    let forcage = choisie.filter(|c| !c.centre && c.capacite > 0).map(|c| {
        let actuel = city.state.consignes.forcees.iter().find(|(h, _)| *h == c.hex).map(|(_, n)| *n);
        let propose = ecran.forcer.filter(|(h, _)| *h == c.hex).map(|(_, n)| {
            let mut cons = city.state.consignes.clone();
            cons.forcees.retain(|(h, _)| *h != c.hex);
            cons.forcees.push((c.hex, n));
            (n, apercu(&cons) - city.travail.rendements)
        });
        VueForcage { actuel, propose }
    });
    // (graphiques du tiroir : sa largeur, sa hauteur ouverte moins la ligne du titre)
    let large_tiroir = (dispo.fenetre.x - dispo.gauche - dispo.droite - 4.0 * MARGE - 28.0).max(300.0);
    let haut_tiroir = (if ecran.agrandi() { (dispo.fenetre.y * 0.6).round() } else { dispo.tiroir } - 74.0).max(120.0);
    let mut dessiner = |c: crate::graphique::Courbe, taille: Vec2| {
        let tr = crate::graphique::Trace::new(&c, taille);
        let h = assets.add(crate::graphique::dessiner(&c, &tr, charte::TRAIT, charte::TEXTE));
        graphique.push(h.clone());
        (h, c, tr)
    };
    let ouvert = ecran.tiroir_ouvert();
    let courbe_pop = (ecran.onglet == Onglet::Population && ecran.grandeur.is_none() && ouvert).then(|| dessiner(courbe_population(&city, &cases, prevision), Vec2::new(large_tiroir, haut_tiroir)));
    let courbe_gr = match ecran.grandeur {
        Some(gr) if ouvert => Some(dessiner(courbe(&city, gr, ecran.periode, prevision), Vec2::new((large_tiroir * 0.58).round(), haut_tiroir - 30.0))),
        _ => None,
    };
    let bareme = Bareme::de(&vl);
    let gains: Vec<Option<(GainAmenagement, Evaluation)>> = cases.iter().map(|c| ecran.placement.and_then(|a| gain_amenagement(&vl, c, a).ok()).map(|g| { let e = evaluation(&g, &bareme); (g, e) })).collect();
    let conseillee = ecran.placement.and_then(|a| conseil(&vl, &cases, a, &bareme));
    let cible = ecran.placement.zip(ecran.cible.or(conseillee)).and_then(|(a, h)| cases.iter().find(|c| c.hex == h).map(|c| (a, c)));

    // Gauche : ce que la ville est (onglet), ou le catalogue de construction
    commands.entity(*gauche).despawn_related::<Children>().with_children(|g| {
        if ecran.catalogue {
            g.spawn((Node { column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(6.0)), flex_wrap: FlexWrap::Wrap, ..default() }, BackgroundColor(Color::srgb(0.17, 0.15, 0.07))))
                .with_children(|b| {
                    b.spawn(texte(format!("Ce qui freine {} :", city.state.name), 13.0, CONSEIL));
                    b.spawn(texte(diagnostic.texte.clone(), 13.0, CONSEIL));
                });
            catalogue(g, &vl, &city, &cases, ecran.placement, ecran.catalogue_groupe, &images);
            return;
        }
        if ecran.grandeur.is_some() || choisie.is_some() || achat_choisi.is_some() {
            fil(g, ecran.grandeur, choisie.map(|c| c.nom.as_str()).or(achat_choisi.map(|a| a.nom.as_str())));
        }
        match ecran.onglet {
            Onglet::Rendements => {
                batiments_de_la_ville(g, &city, &images);
                file_et_projet(g, &city, false, &vl, &cases, ecran.file_choisie);
            }
            Onglet::Population => population(g, &city, &cases, prevision, ecran.sujet, ecran.grandeur, diagnostic.remedes.iter().find(|r| r.net.is_some_and(|n| n > 0))),
            Onglet::Ressources => echanges_du_tour(g, &city, &partenaires),
            Onglet::Histoire => histoire(g, &city, &images, ecran.histoire_genre, choisie, state.turn),
        }
    });
    // Droite : la fiche de ce qu'on touche ou règle (case, case à acheter, grandeur, placement)
    let (droite_e, mut droite_node) = droite.into_inner();
    let mut fiche = true;
    commands.entity(droite_e).despawn_related::<Children>().with_children(|d| {
        if ecran.catalogue {
            match cible {
                Some((a, c)) => placement(d, &vl, &city, &cases, a, c, ecran.cible.is_none()),
                None if ecran.placement.is_some_and(|a| cases.iter().any(|c| possible(&map.0, c, a).is_ok())) => {
                    vide(d, "Placement", &format!("Aucune case n'y gagne dans {TOURS_EFFET} tours : pas de conseil. Cliquez une case pour voir sa fiche et la choisir quand même."))
                }
                None if ecran.placement.is_some() => vide(d, "Placement", "Aucune case de la ville ne peut porter cet aménagement."),
                None => fiche = false,
            }
            return;
        }
        if let Some(a) = achat_choisi {
            // (la culture prendra-t-elle cette case d'elle-même ?)
            let par_la_culture = (rules::case_par_la_culture(&map.0, centre.0, v, &territoire.0) == Some(a.hex)).then(|| city.state.tours_avant_case(city.state.culture_du_tour(city.travail.rendements)));
            fiche_achat(d, a, &city, state.tresor_joueur(), par_la_culture);
        } else if let Some(c) = choisie {
            fiche_case(d, c, &vl, &images, ecran.grandeur, forcage.as_ref());
        } else if let Some(Sujet::Ressource(r)) = ecran.sujet {
            fiche_ressource(d, r, &city, &cases, &achats, &partenaires);
        } else if let Some(s) = ecran.sujet {
            fiche_cause(d, s, &city);
        } else if let Some(gr) = ecran.grandeur {
            fiche_grandeur(d, gr, &city, &cases, &images, &reglages);
        } else {
            fiche = false;
        }
    });
    let affiche = if fiche { Display::Flex } else { Display::None };
    if droite_node.display != affiche {
        droite_node.display = affiche;
    }
    // Pied : la construction en bref (sur Rendements, la file entière est au-dessus)
    commands.entity(*pied).despawn_related::<Children>().with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|l| {
            let bref: Vec<String> = city.state.file.iter().enumerate().take(3).map(|(i, k)| {
                let t = city.state.tours_pour(i, city.travail.rendements.production).map_or("—".to_string(), |t| format!("{t} t."));
                format!("{} {t}", capitale(k.objet.name()))
            }).collect();
            let texte_pied = if city.state.file.is_empty() { "File vide".to_string() } else { bref.join(" · ") };
            l.spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }, children![texte("CONSTRUCTION", 11.5, charte::MUET), texte(texte_pied, 13.0, if city.state.file.is_empty() { charte::ALERTE } else { charte::TEXTE })]));
            if ecran.catalogue {
                l.spawn((BoutonConstruire, bouton(charte::SURFACE2), children![texte("Fermer (B)", 13.0, charte::TEXTE)]));
            } else {
                l.spawn((BoutonConstruire, bouton(charte::ACTION_FOND), children![texte("Construire (B)", 13.0, charte::TEXTE)]));
            }
        });
    });

    // Carte : couche choisie, teintes, contours, étiquettes
    let couche = couche_vue(&ecran);
    let mode = match couche {
        _ if let Some(a) = ecran.placement => ModeCarte::Placement(a, gains.iter().flatten().map(|(_, e)| e.valeur).fold(0.01, f64::max)),
        Couche::Histoire => ModeCarte::Histoire,
        Couche::Habitants => {
            let v: Vec<f64> = cases.iter().map(|c| c.habitants).filter(|v| *v > 0.5).collect();
            ModeCarte::Habitants(v.iter().copied().fold(f64::MAX, f64::min).min(1e9), v.iter().copied().fold(1.0, f64::max))
        }
        Couche::Emplois => ModeCarte::Remplissage,
        Couche::Quantite => match ecran.grandeur {
            Some(g) => ModeCarte::Grandeur(g, cases.iter().map(|c| valeur(&c.rendement, g)).fold(0.01, f64::max)),
            None => ModeCarte::Dominante,
        },
        Couche::Amenagements | Couche::Gisements | Couche::Echanges => ModeCarte::Dominante,
        Couche::Types => ModeCarte::Types,
        Couche::Rendement => ModeCarte::Rendement(ecran.horizon),
    };
    commands.entity(*couches_barre).despawn_related::<Children>().with_children(|b| {
        b.spawn((Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), ..default() }, BackgroundColor(charte::FOND), children![texte("COUCHE", 11.5, charte::MUET)]));
        for c in couches_proposees(&ecran) {
            let actif = c == couche && ecran.placement.is_none();
            let nom = if c == Couche::Quantite { ecran.grandeur.map_or("Quantité", |g| g.nom()) } else { c.nom() };
            b.spawn((ChoixCouche(c), UiPointer::default(), Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)), ..default() }, BackgroundColor(if actif { charte::ACTION_FOND } else { charte::FOND }), children![texte(nom, 13.0, charte::TEXTE)]));
        }
        if couche == Couche::Rendement && ecran.placement.is_none() {
            b.spawn((Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), margin: UiRect::left(Val::Px(8.0)), ..default() }, BackgroundColor(charte::FOND), children![texte("HORIZON", 11.5, charte::MUET)]));
            for h in Horizon::TOUS {
                let actif = h == ecran.horizon;
                b.spawn((ChoixHorizon(h), UiPointer::default(), Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)), ..default() }, BackgroundColor(if actif { charte::ACTION_FOND } else { charte::FOND }), children![texte(h.nom(), 13.0, charte::TEXTE)]));
            }
        }
    });
    let (legende_e, _) = legende.into_inner();
    commands.entity(legende_e).despawn_related::<Children>().with_children(|l| legende_carte(l, mode, ecran.amenagements_caches, &bareme));
    // Valider là où l'on regarde : une barre au-dessus du tiroir, qui nomme la case visée
    let (barre_e, mut barre_node) = barre.into_inner();
    let affiche = if cible.is_some() && ecran.catalogue { Display::Flex } else { Display::None };
    if barre_node.display != affiche {
        barre_node.display = affiche;
    }
    commands.entity(barre_e).despawn_related::<Children>().with_children(|b| {
        if let Some((a, c)) = cible {
            let e = gain_amenagement(&vl, c, a).ok().map(|g| evaluation(&g, &bareme));
            let quelle = format!("{}{}", if ecran.cible.is_some() { "case choisie" } else { "conseil de l'intendant" }, e.map_or(String::new(), |e| format!(", valeur {}", signe(e.valeur))));
            b.spawn((MettreEnFile, Pulsation, bouton(charte::ACTION_FOND), children![texte(format!("Mettre en file : {} sur {} ({quelle}) · Entrée", a.nom(), c.nom.to_lowercase()), 14.0, charte::TEXTE)]));
        }
    });
    // Ce que la carte du monde montre : teintes, cases claires, contours
    let en_file = |h: Hex| city.state.file.iter().position(|k| matches!(k.objet, rules::Production::Amenagement { hex, .. } if hex == h));
    let mut voulu = crate::ville_carte::CoucheVille { ouverte: true, centre: centre.0, ..default() };
    voulu.claires = cases.iter().map(|c| c.hex).chain(achats.iter().map(|a| a.hex)).collect();
    voulu.teintes = cases.iter().zip(&gains).map(|(c, g)| (c.hex, teinte(c, mode, g.as_ref()))).collect();
    if mode == ModeCarte::Types {
        // (le type dominant de la nourriture de chaque case travaillée ; sans nourriture : à peine)
        voulu.teintes = cases
            .iter()
            .map(|c| {
                let parts = intendant::parts_de_la_case(&map.0, c.hex, c.amenagement);
                let (i, p) = parts.iter().enumerate().fold((0, 0.0), |m, (i, &p)| if p > m.1 { (i, p) } else { m });
                let a = if c.rendement.nourriture > 0.05 && p > 0.0 { if c.travailleurs > 0 || c.centre { 0.5 } else { 0.22 } } else { 0.0 };
                (c.hex, if a > 0.0 { COULEURS_TYPES[i].with_alpha(a) } else { Color::srgba(0.0, 0.0, 0.0, 0.15) })
            })
            .collect();
    }
    if let ModeCarte::Rendement(horizon) = mode {
        // (ce que donne chaque case à l'horizon choisi, de pâle à vif ; le centre à part)
        let a_horizon: Vec<Rendements> = match horizon {
            Horizon::Aujourdhui => cases.iter().map(|c| c.rendement).collect(),
            Horizon::Rempli => cases.iter().map(|c| if c.centre { c.rendement } else { c.max }).collect(),
            Horizon::DixTours => {
                let mut st = city.state.allege();
                st.habitants = prevision.get(TOURS_EFFET - 1).or(prevision.last()).map_or(st.habitants, |p| p.habitants);
                let rep = rules::repartition_ville(&map.0, centre.0, &st, dispo_ville);
                cases.iter().map(|c| if c.centre { c.rendement } else { c.courbe.rendement(rep.sur(c.hex)) }).collect()
            }
        };
        let max = cases.iter().zip(&a_horizon).filter(|(c, _)| !c.centre).map(|(_, r)| r.total()).fold(0.01, f64::max);
        voulu.teintes = cases
            .iter()
            .zip(&a_horizon)
            .map(|(c, r)| {
                let t = if c.centre {
                    charte::CENTRE_VILLE.with_alpha(0.35)
                } else {
                    let poids = [(Grandeur::Nourriture, r.nourriture), (Grandeur::Production, r.production), (Grandeur::Deniers, r.deniers / 0.6)];
                    match poids.into_iter().filter(|(_, v)| *v > 0.05).max_by(|a, b| a.1.total_cmp(&b.1)) {
                        Some((g, _)) => g.couleur().with_alpha(0.05 + 0.6 * (r.total() / max) as f32),
                        None => Color::srgba(0.0, 0.0, 0.0, 0.2),
                    }
                };
                (c.hex, t)
            })
            .collect();
    }
    if matches!(couche, Couche::Rendement | Couche::Emplois) && ecran.placement.is_none() {
        voulu.occupations = cases.iter().filter(|c| !c.centre && c.capacite > 0).map(|c| (c.hex, c.remplissage())).collect();
    }
    if couche == Couche::Gisements && ecran.placement.is_none() {
        // (les cases d'un gisement en couleur, les autres à peine)
        voulu.teintes = cases.iter().map(|c| (c.hex, if c.gisement.is_some() { GISEMENT.with_alpha(0.45) } else { Color::srgba(0.0, 0.0, 0.0, 0.15) })).collect();
        voulu.teintes.extend(achats.iter().filter(|a| a.gisement.is_some()).map(|a| (a.hex, GISEMENT.with_alpha(0.3))));
    }
    if couche == Couche::Echanges && ecran.placement.is_none() {
        // Une flèche par échange du tour, épaisse selon la quantité ; les villes non reliées, en gris
        for p in &partenaires {
            for x in city.state.echanges.iter().filter(|x| x.ville == p.nom) {
                let (de, vers) = if x.recu { (p.centre, centre.0) } else { (centre.0, p.centre) };
                voulu.fleches.push(crate::ville_carte::Fleche { de, vers, couleur: GISEMENT, epaisseur: (1.0 + x.qte as f32 * 3.0).min(5.0), pointille: false });
            }
            if !p.reliee && p.distance <= 2 * crate::echanges::CASES_MAX {
                voulu.fleches.push(crate::ville_carte::Fleche { de: centre.0, vers: p.centre, couleur: charte::MUET, epaisseur: 1.0, pointille: true });
            }
        }
    }
    let contour = |hex: Hex, couleur: Color, pointille: bool, retrait: f32| crate::ville_carte::Contour { hex, couleur, pointille, retrait };
    for c in &cases {
        voulu.contours.push(contour(c.hex, Color::srgba(1.0, 0.95, 0.78, 0.55), false, 0.03));
        if en_file(c.hex).is_some() {
            voulu.contours.push(contour(c.hex, Grandeur::Production.couleur(), true, 0.12));
        }
    }
    for a in &achats {
        voulu.contours.push(contour(a.hex, if Some(a.hex) == ecran.case { charte::SELECTION } else { Grandeur::Deniers.couleur().with_alpha(0.85) }, true, 0.05));
    }
    for (c, g) in cases.iter().zip(&gains) {
        if let Some((_, e)) = g
            && matches!(e.risque, Risque::Famine(_) | Risque::BordDeFamine(_))
        {
            voulu.contours.push(contour(c.hex, charte::GRAVE, false, 0.08));
        }
    }
    if let Some(h) = conseillee {
        voulu.contours.push(contour(h, CONSEIL, false, 0.08));
    }
    if couche == Couche::Histoire {
        let mut lieux: Vec<Hex> = city.state.chronique.iter().filter_map(|e| e.lieu).collect();
        lieux.sort();
        lieux.dedup();
        for h in lieux {
            voulu.contours.push(contour(h, CONSEIL, false, 0.08));
        }
    }
    let file_visible = ecran.onglet == Onglet::Rendements && ecran.case.is_none() && if ecran.catalogue { ecran.placement.is_none() } else { ecran.grandeur.is_none() };
    if let Some(h) = ecran.file_choisie.filter(|_| file_visible).and_then(|i| city.state.file.get(i)).and_then(|k| case_de(&k.objet)) {
        voulu.contours.push(contour(map.0.canonical(h), Grandeur::Production.couleur(), false, 0.1));
    }
    for h in ecran.cible.into_iter().chain(choisie.map(|c| c.hex)) {
        voulu.contours.push(contour(h, charte::SELECTION, false, 0.14));
    }
    if *couche_carte != voulu {
        *couche_carte = voulu;
    }
    // Étiquettes des cases, posées sur la carte (taillées d'après la taille d'une case à l'écran)
    let wrap = map.0.wrap();
    commands.entity(*etiquettes).despawn_related::<Children>().with_children(|p| {
        if s < 6.0 {
            return;
        }
        for a in &achats {
            hexagone_achat(p, a, s, wrap.nearest(centre.0, a.hex), a.prix <= state.tresor_joueur());
        }
        if couche == Couche::Echanges && ecran.placement.is_none() {
            for x in &partenaires {
                let recu: Vec<String> = city.state.echanges.iter().filter(|e| e.ville == x.nom).map(|e| format!("{} {} {}", if e.recu { "reçu" } else { "donné" }, quantite(e.qte), e.ressource.nom())).collect();
                let etat = if !x.reliee { "non reliée".to_string() } else if recu.is_empty() { "reliée · rien ce tour".to_string() } else { recu.join(" · ") };
                if x.reliee || x.distance <= 2 * crate::echanges::CASES_MAX {
                    p.spawn((
                        crate::ville_carte::SurCase { hex: wrap.nearest(centre.0, x.centre) },
                        Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), margin: UiRect::top(Val::Px(70.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                        BackgroundColor(charte::FOND.with_alpha(0.88)),
                        Pickable::IGNORE,
                        children![texte(format!("{} · {}", x.nom, x.trajet()), 13.0, if x.reliee { charte::TEXTE } else { charte::MUET }), texte(etat, 12.0, if x.reliee { GISEMENT } else { charte::MUET })],
                    ));
                }
            }
        }
        for (c, g) in cases.iter().zip(&gains) {
            hexagone(p, c, s, wrap.nearest(centre.0, c.hex), mode, !ecran.amenagements_caches, &images, g.as_ref(), en_file(c.hex));
        }
    });

    // Tiroir : les cases de la ville (titre et résumé sur la bande)
    let travaillees = cases.iter().filter(|c| !c.centre && c.travailleurs > 0).count();
    let actifs: u32 = cases.iter().map(|c| c.travailleurs).sum();
    let places: u32 = cases.iter().filter(|c| !c.centre).map(|c| c.capacite).sum();
    let y = city.travail.rendements;
    let variation = prevision.first().map_or_else(|| city.state.variation(y), |p| p.variation);
    let titre_liste = match (ecran.onglet, ecran.grandeur) {
        (_, Some(g)) => {
            let suite = if g == Grandeur::Nourriture { format!(" · couverture × {}", rules::decimal(y.nourriture / city.state.besoin_nourriture().max(0.001))) } else { String::new() };
            format!("{} · {} par tour{suite} · courbe et bilan", g.nom(), rules::decimal(valeur(&y, g)))
        }
        (Onglet::Population, _) => {
            let arret = rules::arret_de_croissance(prevision).and_then(|t| prevision.get(t - 1)).map_or(String::new(), |p| format!(" · arrêt vers {}", rules::milliers(arrondi_parlant(p.habitants))));
            format!("Habitants · {}{} par tour{arret}", if variation >= 0 { "+" } else { "−" }, rules::milliers(variation.unsigned_abs()))
        }
        (Onglet::Histoire, _) => format!("Chronique de {} · {} événements depuis la fondation", city.state.name, city.state.chronique.len()),
        (Onglet::Ressources, _) => {
            let manque = crate::gisements::Ressource::TOUTES.iter().filter(|&&r| city.state.demande(r) > 0.0 && city.state.couverture(r) < 0.95).count();
            format!("Ressources de la ville · {manque} besoin{} pas couvert{} · le reste : sans effet ou un bonus à gagner", if manque > 1 { "s" } else { "" }, if manque > 1 { "s" } else { "" })
        }
        _ => format!("Cases de la ville · {} cases · {travaillees} travaillées · {} travailleurs · {} places libres · maintenant → pleine", cases.len(), rules::milliers(actifs.into()), rules::milliers(places.saturating_sub(actifs).into())),
    };
    if let Ok(mut t) = textes.get_mut(*titre) {
        t.0 = titre_liste;
    }
    for &e in *bascule {
        if let Ok(mut t) = textes.get_mut(e) {
            let voulu = if ouvert { "Replier (Tab)" } else { "Ouvrir (Tab)" };
            if t.0 != voulu {
                t.0 = voulu.into();
            }
        }
    }
    commands.entity(*liste).despawn_related::<Children>().with_children(|l| {
        if !ouvert {
            return;
        }
        if let (Some(gr), Some((h, cb, tr))) = (ecran.grandeur, &courbe_gr) {
            bilan_grandeur(l, gr, &city, &cases, h.clone(), cb, tr, ecran.periode);
            return;
        }
        if let Some((h, cb, tr)) = &courbe_pop {
            poser_graphique(l, h.clone(), cb, tr);
            return;
        }
        if ecran.onglet == Onglet::Ressources {
            tableau_ressources(l, &city, &cases, &achats, ecran.sujet);
            return;
        }
        if ecran.onglet == Onglet::Histoire {
            frise(l, &city, &images, state.turn, large_tiroir);
            return;
        }
        if ecran.onglet == Onglet::Rendements {
            tableau_cases(l, &cases, ecran.case);
            ensemble(l, &city, &cases);
            return;
        }
        {
            let gr = None;
            liste_droite(l, ecran.onglet, &cases, ecran.case, gr, couche == Couche::Habitants, &city.state.chronique);
        }
    });
}

/// Barres d'unités d'une case (couche Rendement) : pour la nourriture et la production, une cellule
/// par unité ; pleine pour ce qu'elle donne aujourd'hui (plus pâle : la subsistance), en contour pour
/// ce qu'elle donnerait pleine ; « maintenant / plein » et les travailleurs sur les places.
fn barres_unites(h: &mut ChildSpawnerCommands, c: &CaseVue, s: f32, images: &Images) {
    let sub = c.courbe.subsistance_de(c.travailleurs);
    let lignes: Vec<(Grandeur, f64, f64, f64)> = [(Grandeur::Nourriture, c.rendement.nourriture, sub.nourriture, c.max.nourriture), (Grandeur::Production, c.rendement.production, sub.production, c.max.production)]
        .into_iter()
        .filter(|(_, v, _, m)| *m > 0.05 || *v > 0.05)
        .collect();
    let taille = (s * 0.12).clamp(7.0, 11.0);
    let txt = (s * 0.21).clamp(10.0, 12.5);
    h.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(charte::FOND.with_alpha(0.8)))).with_children(|b| {
        for (g, v, sub, m) in lignes {
            // (une cellule par unité, au plus 10 ; au-delà, chacune en compte plusieurs)
            let unite = (m / 10.0).ceil().max(1.0);
            let n = ((m / unite).ceil() as usize).max(1);
            b.spawn(Node { column_gap: Val::Px(2.0), align_items: AlignItems::Center, ..default() }).with_children(|l| {
                l.spawn(icone(images.grandeur(g), taille + 3.0, g.couleur()));
                for i in 0..n {
                    let plein = ((v - sub) / unite - i as f64).clamp(0.0, 1.0) as f32;
                    let pale = ((v / unite - i as f64).clamp(0.0, 1.0) as f32 - plein).max(0.0);
                    l.spawn((Node { width: Val::Px(taille), height: Val::Px(taille), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BorderColor::all(g.couleur().with_alpha(0.8)))).with_children(|cel| {
                        if plein > 0.0 {
                            cel.spawn((Node { width: Val::Percent(plein * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(g.couleur())));
                        }
                        if pale > 0.0 {
                            cel.spawn((Node { width: Val::Percent(pale * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(g.couleur().with_alpha(0.45))));
                        }
                    });
                }
                l.spawn((texte_carte(format!("{} / {}", rules::decimal(v), rules::decimal(m)), txt, charte::TEXTE), Node { margin: UiRect::left(Val::Px(3.0)), ..default() }));
            });
        }
        if !c.centre && c.capacite > 0 {
            b.spawn(texte_carte(format!("{} / {}", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())), txt, Color::srgb(0.79, 0.70, 1.0)));
        }
    });
}

/// Teinte posée sur une case de la carte du monde (avec son opacité : le terrain reste lisible).
fn teinte(c: &CaseVue, mode: ModeCarte, gain: Option<&(GainAmenagement, Evaluation)>) -> Color {
    match mode {
        ModeCarte::Placement(_, max) => match gain {
            Some((_, e)) if matches!(e.risque, Risque::Famine(_)) => charte::GRAVE.with_alpha(0.4),
            Some((_, e)) => charte::AMENAGEMENT.with_alpha(0.12 + 0.45 * (e.valeur.max(0.0) / max) as f32),
            None => Color::srgba(0.0, 0.0, 0.0, 0.45),
        },
        ModeCarte::Grandeur(g, max) => g.couleur().with_alpha(0.1 + 0.5 * (valeur(&c.rendement, g) / max) as f32),
        ModeCarte::Habitants(lo, hi) => {
            if c.habitants > 0.5 {
                rampe(if hi > lo { ((c.habitants.ln() - lo.ln()) / (hi.ln() - lo.ln())) as f32 } else { 1.0 }).with_alpha(0.7)
            } else {
                Color::srgba(0.0, 0.0, 0.0, 0.25)
            }
        }
        _ if c.centre => charte::CENTRE_VILLE.with_alpha(0.35),
        ModeCarte::Remplissage => {
            let part = c.remplissage();
            (if part >= 0.9 { Color::srgb(0.62, 0.55, 0.92) } else { charte::OK }).with_alpha(0.18 + 0.42 * part)
        }
        ModeCarte::Dominante | ModeCarte::Histoire | ModeCarte::Types | ModeCarte::Rendement(_) => match c.dominante() {
            Some(g) => g.couleur().with_alpha(if c.travailleurs > 0 { 0.32 } else { 0.12 }),
            None => Color::NONE,
        },
    }
}

/// Texte posé sur la carte : ombré pour rester lisible sur tous les terrains.
fn texte_carte(t: impl Into<String>, taille: f32, couleur: Color) -> impl Bundle {
    (texte(t, taille, couleur), TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) })
}

fn melange(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t)
}

/// Un hexagone de la carte de la ville.
#[allow(clippy::too_many_arguments)]
fn hexagone(p: &mut ChildSpawnerCommands, c: &CaseVue, s: f32, sur: Hex, mode: ModeCarte, amenagements: bool, images: &Images, gain: Option<&(GainAmenagement, Evaluation)>, en_file: Option<usize>) {
    let mode_pop = mode == ModeCarte::Remplissage;
    let grand = s >= 30.0;
    p.spawn((
        crate::ville_carte::SurCase { hex: sur },
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(s * RACINE_3),
            height: Val::Px(2.0 * s),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|h| {
        let petit = (s * 0.26).clamp(10.0, 14.0);
        let centre = |b: (Text, TextFont, TextColor)| (b, TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) }, TextLayout::justify(Justify::Center), Node { max_width: Val::Px(s * 1.45), ..default() });
        if mode_pop && !c.centre && grand {
            h.spawn(centre(texte(format!("{} %", (c.remplissage() * 100.0).round()), (s * 0.34).clamp(11.0, 18.0), charte::TEXTE)));
            h.spawn(centre(texte(format!("{} / {}", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())), petit, charte::DOUX)));
        } else if let (ModeCarte::Habitants(lo, hi), true) = (mode, s >= 22.0) {
            if c.habitants > 0.5 {
                let q = if hi > lo { (c.habitants.ln() - lo.ln()) / (hi.ln() - lo.ln()) } else { 1.0 };
                h.spawn(centre(texte(rules::milliers(c.habitants.round() as u64), (s * 0.34).clamp(11.0, 18.0), if q < 0.4 { Color::srgb(0.11, 0.08, 0.22) } else { charte::TEXTE })));
            }
        } else if let (ModeCarte::Placement(..), true) = (mode, s >= 22.0) {
            match gain {
                Some((g, e)) => {
                    // Ce qu'on gagne (vert, en grand), ce qu'on perd (rouge), dans 10 tours ; puis le
                    // gisement de la case et les risques
                    let grand = (s * 0.27).clamp(11.0, 16.0);
                    let petit = (s * 0.22).clamp(10.0, 13.0);
                    let mut rien = true;
                    // (un nombre et l'icône de sa grandeur : en couleur ce qu'on gagne, en rouge ce qu'on perd)
                    let ligne = |h: &mut ChildSpawnerCommands, gains: bool, taille: f32| {
                        let mut vus = Vec::new();
                        for gr in [Grandeur::Nourriture, Grandeur::Production, Grandeur::Deniers] {
                            let v = valeur(&g.dans, gr);
                            if (gains && v >= 0.05) || (!gains && v <= -0.05) {
                                vus.push((gr, v));
                            }
                        }
                        if vus.is_empty() {
                            return false;
                        }
                        h.spawn((Node { column_gap: Val::Px(4.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)), margin: UiRect::vertical(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(charte::FOND.with_alpha(0.78)))).with_children(|r| {
                            for (gr, v) in vus {
                                let couleur = if gains { gr.couleur() } else { charte::GRAVE };
                                r.spawn((texte(signe(v), taille, couleur), TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) }));
                                r.spawn(icone(images.grandeur(gr), taille, couleur));
                            }
                        });
                        true
                    };
                    if ligne(h, true, grand) {
                        rien = false;
                    }
                    if s >= 26.0 {
                        ligne(h, false, petit);
                    }
                    // (et une fois la case remplie : le long terme à côté de l'immédiat)
                    let rempli: Vec<(Grandeur, f64)> = [Grandeur::Nourriture, Grandeur::Production, Grandeur::Deniers].into_iter().map(|gr| (gr, valeur(&g.plein, gr))).filter(|(_, v)| *v >= 0.05).collect();
                    if !rempli.is_empty() && s >= 26.0 {
                        h.spawn((Node { column_gap: Val::Px(3.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)), margin: UiRect::top(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(charte::FOND.with_alpha(0.6)))).with_children(|r| {
                            r.spawn(texte("rempli", petit * 0.9, charte::DOUX));
                            for (gr, v) in rempli {
                                r.spawn(texte(signe(v), petit, gr.couleur()));
                                r.spawn(icone(images.grandeur(gr), petit, gr.couleur()));
                            }
                        });
                    }
                    if rien {
                        h.spawn(centre(texte("rien à gagner", petit, charte::MUET)));
                    }
                    if let Some(t) = conflit_gisement(c, placement_de(mode)) {
                        h.spawn(centre(texte(t.0, petit, t.1)));
                    }
                    let note = match e.risque {
                        Risque::Famine(_) => Some(("famine !", charte::GRAVE)),
                        Risque::BordDeFamine(_) => Some(("bord de famine", charte::GRAVE)),
                        // (le défrichement : la hache, en haut de la case)
                        Risque::Aucun if g.defriche => None,
                        Risque::Aucun => c.amenagement.map(|_| ("remplace l'actuel", charte::ALERTE)),
                    };
                    if let Some((t, couleur)) = note.filter(|_| s >= 30.0) {
                        h.spawn(centre(texte(t, petit, couleur)));
                    }
                    if g.defriche {
                        let z = (s * 0.45).clamp(14.0, 26.0);
                        h.spawn((Node { position_type: PositionType::Absolute, top: Val::Px(s * 0.22), right: Val::Px(s * 0.22), ..default() }, children![icone(images.hache.clone(), z, charte::ALERTE)]));
                    }
                }
                None => {
                    h.spawn(centre(texte("—", (s * 0.3).clamp(10.0, 16.0), charte::MUET)));
                    if let Some(r) = c.gisement {
                        h.spawn(centre(texte(format!("◆ {}", r.nom()), (s * 0.22).clamp(10.0, 13.0), GISEMENT)));
                    }
                }
            }
        } else if let (ModeCarte::Grandeur(g, _), true) = (mode, s >= 22.0) {
            h.spawn(centre(texte(rules::decimal(valeur(&c.rendement, g)), (s * 0.34).clamp(11.0, 18.0), charte::TEXTE)));
        } else if let (ModeCarte::Rendement(_), true) = (mode, s >= 22.0) {
            barres_unites(h, c, s, images);
        } else if s >= 22.0 && !c.centre && mode != ModeCarte::Histoire {
            // (pas le nom du terrain : il est dans l'infobulle ; l'aménagement et le gisement)
            if let Some(a) = c.amenagement.filter(|_| amenagements) {
                h.spawn(centre(texte(a.nom().to_lowercase(), petit, charte::AMENAGEMENT)));
            }
            if let Some(r) = c.gisement {
                let etat = if c.amenagement == Some(Amenagement::Exploitation) { "" } else { " · non exploité" };
                h.spawn(centre(texte(format!("◆ {}{etat}", r.nom().to_lowercase()), petit, charte::TEXTE)));
            }
        }
        if let Some(n) = c.force {
            h.spawn((
                Node { position_type: PositionType::Absolute, bottom: Val::Px(s * 0.12), padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.85)),
                children![texte(format!("forcé {}", rules::milliers(n.into())), (s * 0.2).clamp(9.0, 12.0), charte::DOUX)],
            ));
        }
        if let Some(i) = en_file {
            h.spawn((
                Node { position_type: PositionType::Absolute, top: Val::Px(s * 0.3), padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.85)),
                BorderColor::all(Grandeur::Production.couleur()),
                children![texte(format!("file n°{}", i + 1), (s * 0.2).clamp(9.0, 12.0), Grandeur::Production.couleur())],
            ));
        }
        if amenagements && mode != ModeCarte::Dominante && let Some(a) = c.amenagement {
            let z = (s * 0.42).clamp(12.0, 26.0);
            h.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(s * 0.3),
                    width: Val::Px(z),
                    height: Val::Px(z),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(z / 2.0)),
                    ..default()
                },
                BackgroundColor(charte::AMENAGEMENT_FOND),
                BorderColor::all(charte::AMENAGEMENT),
                children![icone(images.amenagement(a.into()), z * 0.62, charte::AMENAGEMENT)],
            ));
        }
    });
}

/// Une case voisine à acheter sur la carte de la ville : pâle, en pointillé, avec son prix.
fn hexagone_achat(p: &mut ChildSpawnerCommands, a: &CaseAchat, s: f32, sur: Hex, abordable: bool) {
    p.spawn((
        crate::ville_carte::SurCase { hex: sur },
        Node { position_type: PositionType::Absolute, width: Val::Px(s * RACINE_3), height: Val::Px(2.0 * s), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        Pickable::IGNORE,
    ))
    .with_children(|h| {
        if let Some(r) = a.gisement.filter(|_| s >= 22.0) {
            h.spawn((texte_carte(format!("◆ {}", r.nom()), (s * 0.24).clamp(10.0, 13.0), GISEMENT), TextLayout::justify(Justify::Center)));
        }
        if s >= 22.0 {
            let couleur = if abordable { Grandeur::Deniers.couleur() } else { charte::DOUX };
            let prix = if s >= 30.0 { format!("{}\ndeniers", a.prix) } else { a.prix.to_string() };
            h.spawn((texte_carte(prix, (s * 0.26).clamp(10.0, 14.0), couleur), TextLayout::justify(Justify::Center)));
        }
    });
}

/// Fiche d'une case voisine du territoire : ce qu'elle rapporterait, son prix, le bouton d'achat.
/// `par_la_culture` : la culture la prendra d'elle-même (dans tant de tours, si elle avance).
fn fiche_achat(g: &mut ChildSpawnerCommands, a: &CaseAchat, city: &City, tresor: i32, par_la_culture: Option<Option<u32>>) {
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
            t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(a.nom.clone(), 17.0, charte::TEXTE)]));
            t.spawn((FermerFiche, bouton(charte::SURFACE), children![texte("× fermer (Échap)", 13.0, charte::TEXTE)]));
        });
        p.spawn(texte("Hors du territoire de la ville, qu'elle touche : on peut l'acheter.", 13.5, charte::DOUX));
        p.spawn(texte("Ce qu'elle rapporterait, pleine de travailleurs :", 13.0, charte::TEXTE));
        let ligne = |p: &mut ChildSpawnerCommands, libelle: String, r: &Rendements, couleur: Color| {
            p.spawn((Node { column_gap: Val::Px(10.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(libelle, 13.0, couleur)]));
                valeurs(l, r, 13.5);
            });
        };
        ligne(p, "à l'état naturel (cueillette, chasse)".into(), &a.naturel, charte::DOUX);
        if let Some((am, r)) = a.meilleur {
            ligne(p, format!("avec {} (le mieux ici)", article_amenagement(am)), &r, charte::AMENAGEMENT);
        }
        let seuil = rules::seuil_culture(city.state.cases_gagnees);
        let culture = match par_la_culture {
            Some(Some(0)) => "La culture la prendra d'elle-même à la fin du tour.".to_string(),
            Some(Some(n)) => format!("La culture la prendra d'elle-même dans {n} tour{}.", if n > 1 { "s" } else { "" }),
            Some(None) => "C'est la case que la culture prendra, quand elle avancera.".to_string(),
            None => "La culture prendra d'abord une autre case.".to_string(),
        };
        p.spawn(texte(culture, 12.5, charte::MUET));
        p.spawn(texte(format!("L'achat compte comme une case gagnée : la prochaine case par la culture demandera {:.0} au lieu de {seuil:.0}.", rules::seuil_culture(city.state.cases_gagnees + 1)), 12.5, charte::MUET));
        p.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::top(Val::Px(6.0)), ..default() }).with_children(|l| {
            if tresor >= a.prix {
                l.spawn((AcheterCase(a.hex), bouton(charte::ACTION_FOND), children![texte(format!("Acheter pour {} deniers", a.prix), 14.0, charte::TEXTE)]));
            } else {
                l.spawn((Node { padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)), border_radius: BorderRadius::all(Val::Px(7.0)), border: UiRect::all(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT2), children![texte(format!("Acheter pour {} deniers", a.prix), 14.0, charte::MUET)]));
            }
            let couleur = if tresor >= a.prix { charte::DOUX } else { charte::ALERTE };
            let reste = if tresor >= a.prix { format!("trésor : {tresor} deniers") } else { format!("trésor : {tresor} deniers, il en manque {}", a.prix - tresor) };
            l.spawn(texte(reste, 12.5, couleur));
        });
    });
}

/// « un champ », « un pâturage », « une coupe de bois ».
fn article_amenagement(a: Amenagement) -> String {
    let n = a.nom().to_lowercase();
    if matches!(a, Amenagement::CoupeDeBois) { format!("une {n}") } else { format!("un {n}") }
}

/// Une entrée de légende : pastille de couleur et texte.
fn pastille(l: &mut ChildSpawnerCommands, couleur: Color, t: &str) {
    l.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|e| {
        e.spawn((Node { width: Val::Px(18.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(couleur)));
        e.spawn(texte(t, 13.0, charte::TEXTE));
    });
}

/// Légende et couches, au-dessus de la carte.
fn legende_carte(l: &mut ChildSpawnerCommands, mode: ModeCarte, caches: bool, bareme: &Bareme) {
    if mode == ModeCarte::Histoire {
        pastille(l, CONSEIL, "lieu d'un événement");
        pastille(l, charte::SELECTION, "lieu choisi");
        l.spawn(texte("clic sur une case : ne garder que son histoire", 12.5, charte::MUET));
        return;
    }
    if let ModeCarte::Habitants(lo, hi) = mode {
        l.spawn(texte(format!("{} hab.", rules::milliers(lo.round() as u64)), 12.5, charte::TEXTE));
        l.spawn(Node { column_gap: Val::Px(0.0), ..default() }).with_children(|d| {
            for k in 0..12 {
                d.spawn((Node { width: Val::Px(14.0), height: Val::Px(10.0), ..default() }, BackgroundColor(rampe(k as f32 / 11.0))));
            }
        });
        l.spawn(texte(format!("{} hab.", rules::milliers(hi.round() as u64)), 12.5, charte::TEXTE));
        pastille(l, Color::srgb(0.086, 0.106, 0.145), "personne");
        l.spawn(texte("chiffres : habitants de la case (pour l'instant, chacun vit là où il travaille ; les sans-emploi au centre)", 12.5, charte::MUET));
        return;
    }
    if let ModeCarte::Placement(a, _) = mode {
        l.spawn(Node { width: Val::Percent(100.0), column_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|b| {
            b.spawn(texte(format!("Choisir une case pour : {}", capitale(a.nom())), 14.0, charte::AMENAGEMENT));
            b.spawn(texte("— clic : choisir · Entrée ou double-clic : mettre en file · Échap ou clic droit : revenir · B : fermer la construction", 12.5, charte::DOUX));
        });
        pastille(l, melange(charte::SURFACE, charte::AMENAGEMENT, 0.2), "valeur faible");
        pastille(l, melange(charte::SURFACE, charte::AMENAGEMENT, 0.6), "forte");
        pastille(l, Color::srgb(0.06, 0.075, 0.11), "impossible");
        pastille(l, CONSEIL, "conseil de l'intendant");
        pastille(l, charte::SURFACE, "hache : défrichement");
        pastille(l, charte::GRAVE, "risque de famine");
        pastille(l, charte::SELECTION, "case choisie");
        l.spawn(texte(format!("chiffres : ce qu'on gagne (en couleur, avec l'icône de la grandeur) et ce qu'on perd (en rouge) dans {TOURS_EFFET} tours · ◆ gisement de la case · couleur de la case : valeur pour la ville (nourriture utile jusqu'à {} × le besoin)", rules::decimal(bareme.plafond)), 12.0, charte::MUET));
        return;
    }
    if let ModeCarte::Rendement(horizon) = mode {
        let quand = match horizon {
            Horizon::Aujourdhui => "aujourd'hui",
            Horizon::DixTours => "dans 10 tours (population prévue, placée par l'intendant)",
            Horizon::Rempli => "une fois pleine",
        };
        l.spawn(texte(format!("teinte : ce que donne la case {quand}, de pâle à vif ·"), 12.5, charte::MUET));
        pastille(l, Grandeur::Nourriture.couleur(), "unité produite");
        pastille(l, Grandeur::Nourriture.couleur().with_alpha(0.45), "dont la subsistance");
        l.spawn((Node { width: Val::Px(14.0), height: Val::Px(10.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BorderColor::all(Grandeur::Nourriture.couleur())));
        l.spawn(texte("possible, case pleine", 12.5, charte::TEXTE));
        pastille(l, Color::srgb(0.79, 0.70, 1.0), "anneau : travailleurs sur les places");
        l.spawn(texte("chiffres : maintenant / case pleine", 12.5, charte::MUET));
        return;
    }
    if mode == ModeCarte::Types {
        for (couleur, nom) in COULEURS_TYPES.iter().zip(["céréales et féculents", "protéines", "fruits et légumes"]) {
            pastille(l, *couleur, nom);
        }
        l.spawn(texte("type de nourriture de chaque case (pâle : personne n'y travaille) · un type compte à partir de 15 % du besoin", 12.5, charte::MUET));
        return;
    }
    if let ModeCarte::Grandeur(g, _) = mode {
        pastille(l, melange(charte::SURFACE, g.couleur(), 0.15), "faible");
        pastille(l, melange(charte::SURFACE, g.couleur(), 0.68), "fort");
        l.spawn(texte(format!("chiffres : {} par case (centre compris)", g.nom().to_lowercase()), 12.5, charte::MUET));
    } else if mode == ModeCarte::Remplissage {
        pastille(l, charte::OK.with_alpha(0.6), "de la place");
        pastille(l, Color::srgb(0.62, 0.55, 0.92), "pleine (90 % et plus)");
        pastille(l, charte::CENTRE_VILLE, "centre de la ville");
        l.spawn(texte("chiffres : travailleurs / capacité", 12.5, charte::MUET));
    } else {
        for g in [Grandeur::Nourriture, Grandeur::Production, Grandeur::Deniers] {
            pastille(l, melange(charte::SURFACE, g.couleur(), 0.5), &g.nom().to_lowercase());
        }
        pastille(l, charte::CENTRE_VILLE, "centre");
        l.spawn(texte("couleur : grandeur principale (pâle : personne n'y travaille) · pointillé : case à acheter · ◆ gisement", 12.5, charte::MUET));
    }
    let _ = caches;
}

/// En-tête : nom (ville précédente, suivante, fermer), palier, habitants, croissance, et ce qui
/// vient ensuite (palier suivant ou arrêt de la croissance).
fn en_tete(e: &mut ChildSpawnerCommands, city: &City, prevision: &[rules::Pas]) {
    let c = &city.state;
    let y = city.travail.rendements;
    let variation = prevision.first().map_or_else(|| c.variation(y), |p| p.variation);
    e.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).with_children(|l| {
        l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(c.name.clone(), 24.0, charte::TEXTE)]));
        for (sens, t) in [(-1, "←"), (1, "→")] {
            l.spawn((ChangerVille(sens), bouton(charte::SURFACE), children![texte(t, 14.0, charte::TEXTE)]));
        }
        l.spawn((Fermer, bouton(charte::SURFACE), children![texte("×", 15.0, charte::TEXTE)]));
    });
    let (t, couleur) = if variation > 0 {
        (format!("+{} par tour", rules::milliers(variation as u64)), charte::OK)
    } else if variation == 0 {
        ("population stable".into(), charte::ALERTE)
    } else {
        (format!("−{} par tour : la population baisse", rules::milliers(variation.unsigned_abs())), charte::GRAVE)
    };
    e.spawn(Node { column_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|m| {
        m.spawn(texte(format!("palier {} · {} habitants ·", c.palier(), rules::milliers(c.habitants)), 13.5, charte::DOUX));
        m.spawn(texte(t, 13.5, couleur));
    });
    let suivant = rules::habitants_du_palier(c.palier() + 1);
    let palier = format!("palier {} ({})", c.palier() + 1, rules::milliers(suivant));
    let (t, couleur) = match (rules::tours_jusqu_a(prevision, suivant), rules::arret_de_croissance(prevision)) {
        (Some(t), _) => (format!("{palier} dans {t} tour{}", if t > 1 { "s" } else { "" }), charte::DOUX),
        (None, Some(t)) => {
            let vers = prevision.get(t.saturating_sub(1)).map_or(c.habitants, |p| p.habitants);
            (format!("{palier} hors d'atteinte : arrêt vers {} dans {t} tours", rules::milliers(arrondi_parlant(vers))), charte::ALERTE)
        }
        (None, None) if variation > 0 => (format!("{palier} dans plus de {} tours", prevision.len()), charte::DOUX),
        (None, None) => (format!("{palier} hors d'atteinte"), charte::ALERTE),
    };
    e.spawn(texte(t, 13.5, couleur));
}

/// Un nombre d'habitants arrondi pour être dit (à la centaine).
fn arrondi_parlant(h: u64) -> u64 {
    (h + 50) / 100 * 100
}

/// Les six grandeurs de la ville, une ligne chacune (pastille, nom, valeur, note, touche) ; la ligne
/// détaillée est encadrée. Clic : son détail (nourriture, production, deniers) ou son onglet.
fn grandeurs(e: &mut ChildSpawnerCommands, city: &City, tresor: i32, choisie: Option<Grandeur>) {
    let c = &city.state;
    let y = city.travail.rendements;
    let couverture = y.nourriture / c.besoin_nourriture().max(0.001);
    let culture = c.culture_du_tour(y);
    let be = c.bien_etre;
    let vise = c.bien_etre_vise(y, city.travail.chomeurs);
    let s = c.salubrite();
    let maladies = (c.habitants as f64 * c.croissance(y).insalubrite).round();
    let prod = match (c.file.first(), c.projet) {
        (Some(k), _) => (format!("{} : {}", k.objet.name().to_lowercase(), delai(c, y.production).replace(" tour(s)", " t.")), charte::DOUX),
        (None, Some(p)) => (format!("projet : {}", p.nom().to_lowercase()), charte::DOUX),
        (None, None) => ("rien à produire".into(), charte::ALERTE),
    };
    let lignes: [(Grandeur, &str, String, String, Color, Option<Grandeur>, Option<Onglet>); 6] = [
        (Grandeur::Nourriture, "Nourriture", rules::decimal(y.nourriture), format!("× {} le besoin", rules::decimal(couverture)), if couverture < 1.0 { charte::GRAVE } else { charte::DOUX }, Some(Grandeur::Nourriture), None),
        (Grandeur::Production, "Production", rules::decimal(y.production), prod.0, prod.1, Some(Grandeur::Production), None),
        (Grandeur::Deniers, "Deniers", format!("+{}", rules::decimal(y.deniers)), format!("trésor du pays : {tresor}"), charte::DOUX, Some(Grandeur::Deniers), None),
        (Grandeur::Culture, "Culture", format!("+{}", rules::decimal(culture)), c.tours_avant_case(culture).map_or("pas de case gagnée".to_string(), |t| format!("case dans {t} t.")), charte::DOUX, None, None),
        (
            Grandeur::BienEtre,
            "Bien-être",
            format!("{be:.0}"),
            if (vise - be).abs() >= 1.0 { format!("va vers {vise:.0}") } else { c.humeur().nom().to_string() },
            if c.humeur() == rules::Humeur::Mecontente { charte::GRAVE } else { charte::DOUX },
            None,
            Some(Onglet::Population),
        ),
        (
            Grandeur::Eau,
            "Salubrité",
            format!("{s:.0}"),
            if maladies < 0.0 { format!("maladies −{} hab./tour", rules::milliers(maladies.abs() as u64)) } else { "pas de maladie".to_string() },
            if maladies < 0.0 { charte::GRAVE } else { charte::DOUX },
            None,
            Some(Onglet::Population),
        ),
    ];
    e.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() }).with_children(|b| {
        for (i, (g, nom, valeur, note, couleur, detail, onglet)) in lignes.into_iter().enumerate() {
            let actif = detail.is_some() && detail == choisie;
            let mut ligne = b.spawn((
                UiPointer::default(),
                Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                BackgroundColor(Color::NONE),
                BorderColor::all(if actif { g.couleur() } else { Color::NONE }),
            ));
            ligne.insert(AideGrandeur(g));
            match (detail, onglet) {
                (Some(d), _) => ligne.insert(ChoixGrandeur(d)),
                (None, Some(o)) => ligne.insert(OuvrirOnglet(o)),
                _ => ligne.insert(OuvrirOnglet(Onglet::Rendements)),
            };
            ligne.with_children(|l| {
                l.spawn((Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(g.couleur())));
                l.spawn((Node { width: Val::Px(92.0), flex_shrink: 0.0, ..default() }, children![texte(nom, 13.5, g.couleur())]));
                l.spawn((Node { width: Val::Px(52.0), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(valeur, 15.0, charte::TEXTE)]));
                l.spawn((Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() }, children![(texte(note, 12.5, couleur), TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap))]));
                l.spawn((Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(0.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BorderColor::all(charte::TRAIT2), children![texte((i + 1).to_string(), 11.0, charte::MUET)]));
            });
        }
    });
}

/// Jetons des besoins (sel, vêtements, plantes, outils, luxe) : couverture du dernier tour. Clic :
/// l'onglet Ressources.
fn jetons_besoins(e: &mut ChildSpawnerCommands, city: &City) {
    use crate::gisements::Ressource;
    let c = &city.state;
    let luxes: Vec<Ressource> = Ressource::TOUTES.iter().copied().filter(|r| r.luxe().is_some()).collect();
    let luxe_couvert = luxes.iter().filter(|&&r| c.couverture(r) >= 0.5).count();
    let besoin_outils = c.demande_outils_tour > 0.0;
    let jetons: [(&str, f64, String, Ressource); 5] = [
        ("Sel", c.couverture(Ressource::Sel), String::new(), Ressource::Sel),
        ("Vêtements", c.couverture(Ressource::Fibres), if c.part_besoin_vetements() <= 0.0 { "pas encore".into() } else { String::new() }, Ressource::Fibres),
        ("Plantes", c.couverture(Ressource::Plantes), String::new(), Ressource::Plantes),
        ("Outils", c.couverture_outils, if besoin_outils { String::new() } else { "aucun besoin".into() }, Ressource::Silex),
        ("Luxe", luxe_couvert as f64 / luxes.len().max(1) as f64, format!("{luxe_couvert} / {}", luxes.len()), Ressource::Ocre),
    ];
    e.spawn(Node { column_gap: Val::Px(4.0), ..default() }).with_children(|r| {
        for (nom, part, autre, ressource) in jetons {
            let couleur = if !autre.is_empty() && nom != "Luxe" { charte::MUET } else if part >= 0.95 { charte::OK } else if part >= 0.5 { charte::ALERTE } else { charte::GRAVE };
            let valeur = if autre.is_empty() { format!("{:.0} %", part * 100.0) } else { autre };
            r.spawn((
                ChoixRessource(ressource),
                UiPointer::default(),
                Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                BackgroundColor(Color::NONE),
                BorderColor::all(charte::TRAIT),
            ))
            .with_children(|j| {
                j.spawn((texte(nom, 12.0, charte::TEXTE), TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap)));
                j.spawn(barre(part as f32, 54.0, couleur));
                j.spawn((texte(valeur, 11.5, couleur), TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap)));
            });
        }
    });
}

/// Gain réel d'un projet pour la ville aujourd'hui (décision C4), s'il est dans le jeu.
fn gain_projet(city: &City, pj: rules::Projet) -> Option<String> {
    let y = city.travail.rendements;
    if pj == rules::Projet::GrandsJeux {
        let mut sans = city.state.allege();
        sans.file.clear();
        sans.projet = None;
        let mut avec = sans.clone();
        avec.projet = Some(pj);
        let gain = avec.bien_etre_vise(y, city.travail.chomeurs) - sans.bien_etre_vise(y, city.travail.chomeurs);
        return Some(format!("ici : bien-être visé +{gain:.0} (file vide)"));
    }
    if pj != rules::Projet::Peupler {
        return None;
    }
    let mut sans = city.state.allege();
    sans.file.clear();
    sans.projet = None;
    let mut avec = sans.clone();
    avec.projet = Some(pj);
    let gain = avec.variation(y) - sans.variation(y);
    Some(if y.nourriture < city.state.besoin_nourriture() {
        "ici : rien tant que la ville a faim".into()
    } else {
        format!("ici : +{} habitants par tour (file vide)", rules::milliers(gain.max(0) as u64))
    })
}

/// Délai d'une construction : « 3 tour(s) », « prête (attend une case libre) », « jamais au rythme actuel ».
fn delai(c: &rules::CityState, par_tour: f64) -> String {
    let Some(k) = c.file.first() else { return String::new() };
    let attente = if c.colon_bloque() { format!("attend {} habitants (file bloquée)", rules::milliers(rules::CityState::HABITANTS_POUR_COLON)) } else { String::new() };
    match rules::tours_restants(k.objet.cost(), k.investi, par_tour) {
        Some(0) if attente.is_empty() => "prête (attend une case libre)".into(),
        Some(0) => format!("payé, {attente}"),
        Some(t) if attente.is_empty() => format!("{t} tour(s)"),
        Some(t) => format!("{t} tour(s), puis {attente}"),
        None => "jamais au rythme actuel".into(),
    }
}

/// Sous l'en-tête : ce qui freine la croissance (ce tour, à terme, les emplois), le remède le plus
/// rentable, et les autres dépliés par-dessus le panneau.
fn bandeau_frein(e: &mut ChildSpawnerCommands, f: &Frein, ouverts: bool) {
    e.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), border: UiRect::left(Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(charte::SURFACE), BorderColor::all(f.etat.couleur()))).with_children(|b| {
        let ligne = |b: &mut ChildSpawnerCommands, nom: &str, (t, couleur): &(String, Color)| {
            b.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|l| {
                l.spawn((Node { width: Val::Px(58.0), flex_shrink: 0.0, ..default() }, children![texte(nom, 12.0, charte::MUET)]));
                l.spawn((Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }, children![texte(t.clone(), 12.5, *couleur)]));
            });
        };
        ligne(b, "Ce tour", &f.ce_tour);
        ligne(b, "À terme", &f.a_terme);
        if let Some(x) = &f.emplois {
            ligne(b, "Emplois", x);
        }
        for (etat, t, _) in &f.alertes {
            ligne(b, "Aussi", &(t.clone(), etat.couleur()));
        }
        // Le remède le plus rentable ; les autres dépliés par-dessus (la file reste visible)
        let principal = f.remedes.iter().position(|r| r.net.is_some_and(|n| n > 0)).or((!f.remedes.is_empty()).then_some(0));
        b.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, margin: UiRect::top(Val::Px(3.0)), ..default() }).with_children(|l| {
            if let Some(i) = principal {
                bouton_remede(l, &f.remedes[i], true);
            }
            let autres = f.remedes.len().saturating_sub(principal.map_or(0, |_| 1));
            if autres > 0 {
                l.spawn((BasculeRemedes, bouton(Color::NONE), children![texte(format!("{autres} autre{} {}", if autres > 1 { "s" } else { "" }, if ouverts { "−" } else { "+" }), 12.5, charte::DOUX)]));
            }
        });
        if ouverts {
            b.spawn((
                Node { position_type: PositionType::Absolute, top: Val::Percent(100.0), left: Val::Px(0.0), right: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() },
                BackgroundColor(charte::FOND),
                BorderColor::all(charte::TRAIT2),
                GlobalZIndex(97),
            ))
            .with_children(|o| {
                o.spawn(texte("AUTRES REMÈDES · effet net sur la croissance, calculé par le jeu", 11.0, charte::MUET));
                for (i, r) in f.remedes.iter().enumerate() {
                    if Some(i) != principal {
                        bouton_remede(o, r, false);
                    }
                }
            });
        }
    });
}

/// Titre de section.
fn titre(p: &mut ChildSpawnerCommands, t: &str) {
    p.spawn(texte(t, 15.0, charte::TEXTE));
}

/// Panneau qui dit qu'il n'y a rien (et pourquoi).
fn vide(g: &mut ChildSpawnerCommands, t: &str, explication: &str) {
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, t);
        p.spawn(texte(explication, 13.5, charte::MUET));
    });
}

/// Vue d'ensemble : d'où vient chaque grandeur (une ligne par grandeur, cliquable).
fn ensemble(g: &mut ChildSpawnerCommands, city: &City, cases: &[CaseVue]) {
    let c = &city.state;
    let y = city.travail.rendements;
    let centre = cases.iter().find(|c| c.centre).map_or(Rendements::default(), |c| c.rendement);
    let par_cases = y - centre;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, &format!("D'où viennent les rendements de {}", c.name));
        let colonnes = ["", "Centre", "Cases", "Bâtiments", "Commerce", "Total"];
        let ligne = |p: &mut ChildSpawnerCommands, cellules: Vec<(String, Color)>, gr: Option<Grandeur>| {
            let mut e = p.spawn((Node { column_gap: Val::Px(6.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT)));
            if let Some(gr) = gr {
                e.insert((ChoixGrandeur(gr), UiPointer::default()));
            }
            e.with_children(|l| {
                for (i, (t, couleur)) in cellules.into_iter().enumerate() {
                    l.spawn((Node { width: Val::Percent(if i == 0 { 25.0 } else { 15.0 }), justify_content: if i == 0 { JustifyContent::FlexStart } else { JustifyContent::FlexEnd }, ..default() }, children![texte(t, 13.5, couleur)]));
                }
            });
        };
        ligne(p, colonnes.iter().map(|c| (c.to_string(), charte::MUET)).collect(), None);
        for (g, total, c0, cs) in [
            (Grandeur::Nourriture, y.nourriture, centre.nourriture, par_cases.nourriture),
            (Grandeur::Production, y.production, centre.production, par_cases.production),
            (Grandeur::Deniers, y.deniers, centre.deniers, par_cases.deniers),
        ] {
            ligne(p, vec![(g.nom().into(), g.couleur()), (rules::decimal(c0), charte::TEXTE), (rules::decimal(cs), charte::TEXTE), ("—".into(), charte::MUET), ("—".into(), charte::MUET), (rules::decimal(total), charte::TEXTE)], Some(g));
        }
        p.spawn(texte("Bâtiments et commerce : pas encore dans le jeu. Le centre de la ville rapporte sans travailleurs ; les cases, selon les travailleurs que l'intendant y place. Clic sur une grandeur (ou touches 1 à 3) : son détail.", 12.5, charte::MUET));
    });
}

/// Une ligne « libellé … valeur ».
fn ligne_valeur(p: &mut ChildSpawnerCommands, libelle: &str, valeur: String, couleur: Color) {
    p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
        l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(libelle, 13.5, charte::TEXTE)]));
        l.spawn(texte(valeur, 13.5, couleur));
    });
}

/// Onglet Population : indicateurs, évolution et limites, mouvement, cultures et ordres.
fn population(g: &mut ChildSpawnerCommands, city: &City, cases: &[CaseVue], prevision: &[rules::Pas], sujet: Option<Sujet>, grandeur: Option<Grandeur>, remede: Option<&RemedeNet>) {
    let c = &city.state;
    let y = city.travail.rendements;
    let cr = c.croissance(y);
    let h = c.habitants as f64;
    let variation = prevision.first().map_or_else(|| c.variation(y), |p| p.variation);
    let hab = |v: f64| -> String {
        let v = (v * h).round();
        if v.abs() < 0.5 { "0".into() } else { format!("{}{}", if v > 0.0 { "+" } else { "−" }, rules::milliers(v.abs() as u64)) }
    };
    let titre_section = |g: &mut ChildSpawnerCommands, gauche: String, droite: &str| {
        g.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
            t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(gauche, 11.5, charte::MUET)]));
            t.spawn(texte(droite, 11.5, charte::MUET));
        });
    };
    // La croissance du prochain tour, cause par cause (clic : sa fiche)
    titre_section(g, format!("CROISSANCE : {}{} PAR TOUR", if variation >= 0 { "+" } else { "−" }, rules::milliers(variation.unsigned_abs())), "CLIC : LA CAUSE");
    let couverture = y.nourriture / c.besoin_nourriture().max(0.001);
    let naturelle = cr.naturelle / cr.facteur_bien_etre.max(1e-6);
    let s = c.salubrite();
    g.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
        let ligne = |t: &mut ChildSpawnerCommands, nom: String, taux: String, nombre: String, couleur: Color, action: Option<Result<Grandeur, Sujet>>, actif: bool| {
            let mut l = t.spawn((
                Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
                BackgroundColor(Color::NONE),
                BorderColor::all(if actif { charte::SELECTION } else { Color::NONE }),
            ));
            match action {
                Some(Ok(gr)) => {
                    l.insert((ChoixGrandeur(gr), UiPointer::default()));
                }
                Some(Err(s)) => {
                    l.insert((ChoixSujet(s), UiPointer::default()));
                }
                None => {}
            }
            l.with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }, children![texte(nom, 13.5, charte::TEXTE)]));
                l.spawn((Node { width: Val::Px(66.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(taux, 12.5, charte::DOUX)]));
                l.spawn((Node { width: Val::Px(54.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(nombre, 13.5, couleur)]));
            });
        };
        let couleur = |v: f64| if v < -1e-9 { charte::GRAVE } else if v > 1e-9 { charte::OK } else { charte::DOUX };
        ligne(t, format!("Naturelle : nourriture × {} le besoin", rules::decimal(couverture)), pourcent(naturelle), hab(naturelle), couleur(naturelle), Some(Ok(Grandeur::Nourriture)), grandeur == Some(Grandeur::Nourriture));
        ligne(t, format!("Bien-être {:.0}", c.bien_etre), format!("× {}", rules::decimal(cr.facteur_bien_etre)), hab(cr.naturelle - naturelle), couleur(cr.naturelle - naturelle), Some(Err(Sujet::BienEtre)), sujet == Some(Sujet::BienEtre));
        if cr.peupler > 0.0 {
            ligne(t, "Projet « Peupler la ville »".into(), pourcent(cr.peupler), hab(cr.peupler), couleur(cr.peupler), None, false);
        }
        let sous = if s < rules::SALUBRITE_SEUIL { format!(", sous {:.0}", rules::SALUBRITE_SEUIL) } else { String::new() };
        ligne(t, format!("Maladies : salubrité {s:.0}{sous}"), pourcent(cr.insalubrite), hab(cr.insalubrite), couleur(cr.insalubrite), Some(Err(Sujet::Salubrite)), sujet == Some(Sujet::Salubrite));
        ligne(t, "Famine".into(), if cr.famine < 0.0 { pourcent(cr.famine) } else { "—".into() }, hab(cr.famine), couleur(cr.famine), Some(Ok(Grandeur::Nourriture)), false);
        t.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT2))).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("Total", 14.0, charte::TEXTE)]));
            l.spawn(texte(format!("{}{}", if variation >= 0 { "+" } else { "−" }, rules::milliers(variation.unsigned_abs())), 14.0, if variation < 0 { charte::GRAVE } else { charte::TEXTE }));
        });
    });
    // Le remède le plus rentable (le même que sous « Ce qui freine »)
    if let Some(r) = remede.filter(|r| r.net.is_some_and(|n| n > 0)) {
        g.spawn((Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(charte::SURFACE))).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("Le plus rentable :", 12.5, charte::DOUX)]));
            bouton_remede(l, r, true);
        });
    }
    // À terme : la prévision du jeu
    titre_section(g, "À TERME".into(), "PRÉVISION DU JEU");
    let nourriture = limite_nourriture(cases, c);
    let places: u32 = cases.iter().filter(|x| !x.centre).map(|x| x.capacite).sum();
    let bloc = |g: &mut ChildSpawnerCommands, nom: &str, valeur: String, couleur: Color, note: String| {
        g.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|b| {
            b.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(nom, 13.5, charte::TEXTE)]));
                l.spawn(texte(valeur, 13.5, couleur));
            });
            b.spawn(texte(note, 12.0, charte::MUET));
        });
    };
    match rules::arret_de_croissance(prevision) {
        _ if nourriture <= h => bloc(g, "Limite de la nourriture", "dépassée : famine".into(), charte::GRAVE, format!("les cases pleines nourrissent {} habitants", rules::milliers(nourriture as u64))),
        Some(t) => {
            let vers = prevision.get(t - 1).map_or(c.habitants, |p| p.habitants);
            bloc(g, "Arrêt de croissance", format!("vers {}, dans {t} tours", rules::milliers(arrondi_parlant(vers))), charte::ALERTE, format!("nourriture et maladies · les cases pleines nourriraient {} habitants", rules::milliers(nourriture as u64)))
        }
        None => bloc(g, "Arrêt de croissance", format!("pas dans les {} tours", prevision.len()), charte::OK, format!("les cases pleines nourrissent {} habitants", rules::milliers(nourriture as u64))),
    }
    let note = if city.travail.chomeurs > 0 {
        format!("{} sans emploi aujourd'hui : le chômage pèse sur le bien-être", rules::milliers(city.travail.chomeurs.into()))
    } else {
        match prevision.iter().position(|p| p.chomeurs > 0) {
            Some(t) => format!("places pleines dans {} tours : ensuite le chômage pèse sur le bien-être", t + 1),
            None => format!("des places libres pour {} tours au moins", prevision.len()),
        }
    };
    bloc(g, "Actifs · places sur les cases", format!("{} · {}", rules::milliers(city.travail.actifs.into()), rules::milliers(places.into())), charte::TEXTE, note);
}

/// Pourcentage signé : « +1,5 % », « −0,2 % ».
fn pourcent(x: f64) -> String {
    let v = x * 100.0;
    if v.abs() < 0.005 { "0 %".into() } else { format!("{}{} %", if v > 0.0 { "+" } else { "−" }, rules::decimal(v.abs())) }
}

/// Onglet Histoire : les moments marquants de la ville, triés par tour, filtrés par genre et par lieu.
fn histoire(g: &mut ChildSpawnerCommands, city: &City, images: &Images, genre: Option<rules::GenreEvenement>, lieu: Option<&CaseVue>, tour: u32) {
    let c = &city.state;
    // Filtres : un par genre présent, avec son compteur (les genres vides sont cachés)
    g.spawn(Node { column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|f| {
        let filtre = |f: &mut ChildSpawnerCommands, x: Option<rules::GenreEvenement>, t: String| {
            let actif = genre == x;
            f.spawn((
                ChoixGenre(x),
                UiPointer::default(),
                Node { column_gap: Val::Px(5.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(10.0)), ..default() },
                BackgroundColor(if actif { charte::SURFACE2 } else { Color::NONE }),
                BorderColor::all(if actif { charte::TEXTE } else { charte::TRAIT2 }),
            ))
            .with_children(|b| {
                if let Some(x) = x {
                    b.spawn(icone(icone_genre(images, x), 13.0, couleur_genre(x)));
                }
                b.spawn(texte(t, 12.5, if actif { charte::TEXTE } else { charte::DOUX }));
            });
        };
        filtre(f, None, format!("Tout {}", c.chronique.len()));
        for x in rules::GenreEvenement::TOUS {
            let n = c.chronique.iter().filter(|e| e.genre == x).count();
            if n > 0 {
                filtre(f, Some(x), format!("{} {n}", x.nom()));
            }
        }
    });
    if let Some(l) = lieu {
        g.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
            r.spawn(texte(format!("Lieu : {}", l.nom), 13.5, charte::TEXTE));
            r.spawn((FermerFiche, bouton(Color::NONE), children![texte("× tous les lieux", 12.5, charte::DOUX)]));
        });
    }
    // Les 40 derniers tours, du plus récent au plus ancien, par tranches de 10 tours
    let debut = tour.saturating_sub(rules::HISTORIQUE_TOURS as u32 - 1);
    let mut liste: Vec<&rules::Evenement> = c.chronique.iter().filter(|e| genre.is_none_or(|x| e.genre == x)).filter(|e| lieu.is_none_or(|l| e.lieu == Some(l.hex))).filter(|e| e.tour >= debut).collect();
    liste.sort_by_key(|e| std::cmp::Reverse(e.tour));
    let anciens = c.chronique.iter().filter(|e| e.tour < debut).count();
    if liste.is_empty() {
        g.spawn(texte(if lieu.is_some() { "Aucun événement ici dans les 40 derniers tours." } else { "Aucun événement dans les 40 derniers tours." }, 13.0, charte::MUET));
    }
    let mut tranche = None;
    for e in liste {
        let t = (e.tour.saturating_sub(1)) / 10;
        if tranche != Some(t) {
            tranche = Some(t);
            let (a, b) = (t * 10 + 1, (t * 10 + 10).min(tour));
            g.spawn((Node { padding: UiRect::top(Val::Px(4.0)), ..default() }, children![texte(format!("TOURS {a} À {b}"), 11.0, charte::MUET)]));
        }
        g.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::FlexStart, padding: UiRect::vertical(Val::Px(3.0)), ..default() }).with_children(|l| {
            l.spawn((Node { margin: UiRect::top(Val::Px(2.0)), ..default() }, children![icone(icone_genre(images, e.genre), 15.0, couleur_genre(e.genre))]));
            l.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }).with_children(|n| {
                n.spawn(texte(e.texte.clone(), 13.0, charte::TEXTE));
                n.spawn(texte(format!("tour {}{}", e.tour, if e.lieu.is_some() { " · cerclé d'or sur la carte" } else { "" }), 11.5, charte::MUET));
            });
        });
    }
    if anciens > 0 {
        g.spawn(texte(format!("{anciens} événement{} plus ancien{} : la chronique complète est dans le tiroir (Tab).", if anciens > 1 { "s" } else { "" }, if anciens > 1 { "s" } else { "" }), 12.0, charte::MUET));
    }
}

/// Tiroir de l'onglet Histoire : la chronique complète en frise, une ligne par genre, de la
/// fondation à aujourd'hui.
fn frise(l: &mut ChildSpawnerCommands, city: &City, images: &Images, tour: u32, largeur: f32) {
    let c = &city.state;
    let premier = c.chronique.iter().map(|e| e.tour).min().unwrap_or(1).min(tour);
    let marge = 120.0;
    let w = (largeur - marge - 20.0).max(100.0);
    let x = |t: u32| marge + w * if tour > premier { (t - premier) as f32 / (tour - premier) as f32 } else { 1.0 };
    for g in rules::GenreEvenement::TOUS {
        let evs: Vec<&rules::Evenement> = c.chronique.iter().filter(|e| e.genre == g).collect();
        if evs.is_empty() {
            continue;
        }
        l.spawn((Node { height: Val::Px(22.0), border: UiRect::bottom(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|r| {
            r.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(3.0), column_gap: Val::Px(5.0), align_items: AlignItems::Center, ..default() }, children![icone(icone_genre(images, g), 12.0, couleur_genre(g)), texte(format!("{} {}", g.nom(), evs.len()), 12.0, charte::DOUX)]));
            for e in evs {
                r.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x(e.tour) - 4.0), top: Val::Px(7.0), width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(couleur_genre(g))));
            }
        });
    }
    l.spawn(Node { height: Val::Px(18.0), ..default() }).with_children(|r| {
        for t in [premier, (premier + tour) / 2, tour] {
            r.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x(t) - 30.0), width: Val::Px(60.0), justify_content: JustifyContent::Center, ..default() }, children![texte(if t == tour { "aujourd'hui".to_string() } else { format!("tour {t}") }, 11.5, charte::MUET)]));
        }
    });
}

/// Icône d'un genre d'événement (dessinée par la charte).
fn icone_genre(images: &Images, g: rules::GenreEvenement) -> Handle<Image> {
    use rules::GenreEvenement::*;
    match g {
        Fondation | Territoire => images.hexagone.clone(),
        Population => images.grandeur(Grandeur::Habitants),
        Construction => images.grandeur(Grandeur::Production),
        Famine => images.grandeur(Grandeur::Nourriture),
        Bataille => images.grandeur(Grandeur::Defense),
        Projet => images.grandeur(Grandeur::Culture),
    }
}

/// Couleur d'un genre d'événement (filtres, liste, frise).
fn couleur_genre(g: rules::GenreEvenement) -> Color {
    use rules::GenreEvenement::*;
    match g {
        Fondation => Color::srgb(0.95, 0.81, 0.45),
        Population => Color::srgb(0.79, 0.70, 1.0),
        Construction => Color::srgb(0.85, 0.63, 0.40),
        Territoire => Color::srgb(0.93, 0.58, 0.69),
        Famine => Color::srgb(0.94, 0.54, 0.54),
        Bataille => Color::srgb(0.71, 0.70, 0.66),
        Projet => Color::srgb(0.36, 0.77, 0.91),
    }
}

/// Fiche d'une case choisie sur la carte ou dans la liste.
fn fiche_case(g: &mut ChildSpawnerCommands, c: &CaseVue, vl: &Ville, images: &Images, seule: Option<Grandeur>, forcage: Option<&VueForcage>) {
    fiche_case_corps(g, c, vl, images, seule);
    if let (Some(f), false) = (forcage, c.centre) {
        g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| effectif(p, c, f));
    }
}

fn fiche_case_corps(g: &mut ChildSpawnerCommands, c: &CaseVue, vl: &Ville, images: &Images, seule: Option<Grandeur>) {
    let map = vl.map;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
            t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(c.nom.clone(), 17.0, charte::TEXTE)]));
            t.spawn((FermerFiche, bouton(charte::SURFACE), children![texte("× fermer (Échap)", 13.0, charte::TEXTE)]));
        });
        if c.centre {
            p.spawn(texte("Centre de la ville : il rapporte sans travailleurs.", 13.5, charte::DOUX));
        } else {
            ligne_valeur(p, "Aménagement de production", c.amenagement.map_or("aucun (état naturel : cueillette, chasse)".into(), |a| a.nom().to_lowercase()), if c.amenagement.is_some() { charte::AMENAGEMENT } else { charte::DOUX });
            // Roche et sol : la fertilité des champs (5b)
            if let (Some(r), Some(s)) = (map.roche(c.hex), map.sol(c.hex)) {
                let karst = if map.karstique(c.hex) { " karstique" } else { "" };
                ligne_valeur(p, "Sol", format!("{} sur {}{karst} : {}", s.nom(), r.nom(), s.effet_texte()), charte::DOUX);
            }
            // Gisement connu (5f)
            if let Some(r) = c.gisement {
                let etat = if c.amenagement == Some(Amenagement::Exploitation) { "exploité" } else { "à exploiter : aménagement « exploitation »" };
                ligne_valeur(p, "Gisement", format!("{} ({}) · {etat}", r.nom(), r.exploitation()), charte::AMENAGEMENT);
            }
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("Travailleurs", 13.5, charte::TEXTE)]));
                l.spawn(texte(format!("{} / {}", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())), 13.5, charte::TEXTE));
                l.spawn(barre(c.remplissage(), 90.0, if c.remplissage() >= 0.9 { charte::ALERTE } else { charte::OK }));
            });
        }
        if !c.centre && c.capacite > 0 {
            facteurs_case(p, c, images);
        }
        let part = if c.capacite > 0 && c.max.total() > 0.0 { c.rendement.total() / c.max.total() } else { 1.0 };
        // Gain de 100 travailleurs de plus (l'intendant place des blocs de 100)
        let plus = if c.capacite > c.travailleurs { Some(c.courbe.rendement((c.travailleurs + 100).min(c.capacite)) - c.rendement) } else { None };
        for (g, v, m) in [(Grandeur::Nourriture, c.rendement.nourriture, c.max.nourriture), (Grandeur::Production, c.rendement.production, c.max.production), (Grandeur::Deniers, c.rendement.deniers, c.max.deniers)] {
            if m < 0.05 || seule.is_some_and(|s| s != g) {
                continue;
            }
            let suite = match plus {
                _ if c.capacite == 0 => String::new(),
                Some(d) => format!(" · 100 travailleurs de plus : +{}", rules::decimal(valeur(&d, g))),
                None => " · case pleine".into(),
            };
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn(icone(images.grandeur(g), 16.0, g.couleur()));
                l.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                    n.spawn(texte(g.nom(), 13.5, g.couleur()));
                    if c.capacite > 0 {
                        n.spawn(texte(format!("{} % du maximum de la case ({}) : rendements décroissants{suite}", (part * 100.0).round(), rules::decimal(m)), 12.0, charte::MUET));
                    }
                });
                l.spawn(texte(rules::decimal(v), 14.0, charte::TEXTE));
            });
        }
        if !c.centre {
            let possibles: Vec<Amenagement> = Amenagement::possibles(map, c.hex).into_iter().filter(|a| Some(*a) != c.amenagement).collect();
            if c.en_file {
                p.spawn(texte("Un aménagement de cette case est déjà dans la file de construction.", 12.5, charte::MUET));
            } else if possibles.is_empty() {
                p.spawn(texte("Aucun autre aménagement possible ici.", 12.5, charte::MUET));
            } else {
                p.spawn(texte("Aménager cette case (un aménagement par case : il remplace l'actuel) :", 13.0, charte::TEXTE));
                p.spawn(Node { column_gap: Val::Px(8.0), row_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|b| {
                    for a in Amenagement::possibles(map, c.hex).into_iter().filter(|a| Some(*a) != c.amenagement) {
                        let gain = gain_amenagement(vl, c, a).map(|g| format!(" · dans {TOURS_EFFET} tours : {}", ecarts(&g.dans))).unwrap_or_default();
                        b.spawn((AmenagerIci(a), bouton(Color::NONE), children![texte(format!("{} · {} production{gain}", capitale(a.nom()), a.cout(map, c.hex)), 13.0, charte::AMENAGEMENT)]));
                    }
                });
            }
        }
    });
}

/// Tiroir de l'onglet Bâtiments : une ligne par case (travailleurs sur les places, nourriture et
/// production maintenant et pleine, ce que donneraient 100 travailleurs de plus), la plus prometteuse
/// d'abord. Clic : sa fiche.
fn tableau_cases(l: &mut ChildSpawnerCommands, cases: &[CaseVue], choisie: Option<Hex>) {
    let mut lignes: Vec<(&CaseVue, Rendements)> = cases
        .iter()
        .filter(|c| !c.centre && c.capacite > 0)
        .map(|c| (c, c.courbe.rendement((c.travailleurs + 100).min(c.capacite)) - c.rendement))
        .collect();
    lignes.sort_by(|a, b| (b.1.nourriture * 1.5 + b.1.production + b.1.deniers).total_cmp(&(a.1.nourriture * 1.5 + a.1.production + a.1.deniers)).then(a.0.hex.cmp(&b.0.hex)));
    let largeurs = [0.0, 130.0, 150.0, 150.0, 170.0];
    let colonnes = |l: &mut ChildSpawnerCommands, t: [String; 5], couleurs: [Color; 5], taille: f32, choix: Option<Hex>, actif: bool| {
        let mut ligne = l.spawn((
            Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(if actif { charte::SELECTION } else { Color::NONE }),
        ));
        if let Some(h) = choix {
            ligne.insert((ChoixCase(h), UiPointer::default()));
        }
        ligne.with_children(|r| {
            for (i, x) in t.into_iter().enumerate() {
                let w = largeurs[i];
                r.spawn((
                    Node { width: if w > 0.0 { Val::Px(w) } else { Val::Auto }, flex_grow: if w > 0.0 { 0.0 } else { 1.0 }, min_width: Val::Px(0.0), justify_content: if i == 0 { JustifyContent::FlexStart } else { JustifyContent::FlexEnd }, overflow: Overflow::clip(), ..default() },
                    children![(texte(x, taille, couleurs[i]), TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap))],
                ));
            }
        });
    };
    colonnes(l, ["CASE".into(), "TRAVAILLEURS".into(), "NOURRITURE".into(), "PRODUCTION".into(), "100 DE PLUS".into()], [charte::MUET; 5], 11.0, None, false);
    for (c, plus) in lignes {
        let mut nom = match c.amenagement {
            Some(a) => format!("{} · {}", c.nom, a.nom()),
            None => c.nom.clone(),
        };
        if c.force.is_some() {
            nom.push_str(" · figée");
        } else if c.prioritaire {
            nom.push_str(" · prioritaire");
        }
        let gain = if c.travailleurs >= c.capacite {
            "pleine".to_string()
        } else {
            [(plus.nourriture, "nourr."), (plus.production, "prod.")].iter().filter(|(v, _)| *v >= 0.005).map(|(v, n)| format!("+{} {n}", rules::decimal(*v))).collect::<Vec<_>>().join(" ")
        };
        colonnes(
            l,
            [
                nom,
                format!("{} / {}", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())),
                format!("{} → {}", rules::decimal(c.rendement.nourriture), rules::decimal(c.max.nourriture)),
                format!("{} → {}", rules::decimal(c.rendement.production), rules::decimal(c.max.production)),
                gain,
            ],
            [charte::TEXTE, Color::srgb(0.79, 0.70, 1.0), Grandeur::Nourriture.couleur(), Grandeur::Production.couleur(), charte::OK],
            13.0,
            Some(c.hex),
            choisie == Some(c.hex),
        );
    }
}

/// Fiche d'une case : ses trois facteurs (places, rendement plein, travailleurs), ce qu'elle donne
/// selon ses travailleurs (une colonne par tranche : le travail, la subsistance plus pâle, les colonnes
/// au-delà des travailleurs d'aujourd'hui en contour), et un tableau aujourd'hui, +100, pleine.
fn facteurs_case(p: &mut ChildSpawnerCommands, c: &CaseVue, images: &Images) {
    let plein = c.max;
    p.spawn(Node { column_gap: Val::Px(6.0), ..default() }).with_children(|r| {
        for (v, t) in [(rules::milliers(c.capacite.into()), "places"), (rules::decimal(plein.nourriture + plein.production), "rendement plein (nourr. + prod.)"), (rules::milliers(c.travailleurs.into()), "travailleurs aujourd'hui")] {
            r.spawn((Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::all(Val::Px(7.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() }, BackgroundColor(charte::SURFACE)))
                .with_children(|b| {
                    b.spawn(texte(v, 16.0, charte::TEXTE));
                    b.spawn(texte(t, 11.5, charte::MUET));
                });
        }
    });
    // Histogramme : 24 colonnes de 0 aux places ; nourriture et production empilées
    let n = 24usize;
    let haut = 70.0;
    let maxi = (plein.nourriture + plein.production).max(0.01);
    p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).with_children(|b| {
        b.spawn(Node { column_gap: Val::Px(2.0), align_items: AlignItems::FlexEnd, height: Val::Px(haut), ..default() }).with_children(|cols| {
            for i in 1..=n {
                let t = (c.capacite as f64 * i as f64 / n as f64).round() as u32;
                let r = c.courbe.rendement(t);
                let s = c.courbe.subsistance_de(t);
                let actif = t <= c.travailleurs.max(1);
                let morceau = |v: f64| Val::Px((v / maxi * haut as f64) as f32);
                cols.spawn(Node { flex_direction: FlexDirection::ColumnReverse, flex_grow: 1.0, flex_basis: Val::Px(0.0), ..default() }).with_children(|col| {
                    for (g, total, dont) in [(Grandeur::Nourriture, r.nourriture, s.nourriture), (Grandeur::Production, r.production, s.production)] {
                        let k = g.couleur();
                        let (travail, sub) = ((total - dont).max(0.0), dont);
                        for (v, a) in [(travail, 1.0), (sub, 0.5)] {
                            if v > 1e-6 {
                                col.spawn((Node { height: morceau(v), border: UiRect::all(Val::Px(if actif { 0.0 } else { 1.0 })), ..default() }, BackgroundColor(if actif { k.with_alpha(a) } else { Color::NONE }), BorderColor::all(k.with_alpha(0.5 * a))));
                            }
                        }
                    }
                });
            }
        });
        b.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("0", 11.0, charte::MUET)]));
            l.spawn(texte(format!("plein : en couleur aujourd'hui, en contour ce qui reste à remplir · pâle : la subsistance · {} places", rules::milliers(c.capacite.into())), 11.0, charte::MUET));
        });
    });
    // Aujourd'hui, avec 100 travailleurs de plus, pleine ; et ce qu'en donne la subsistance
    let plus = c.courbe.rendement((c.travailleurs + 100).min(c.capacite));
    let sub = c.courbe.subsistance_de(c.capacite);
    p.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
        let ligne = |t: &mut ChildSpawnerCommands, cellules: [String; 4], couleur: Color, taille: f32| {
            t.spawn((Node { column_gap: Val::Px(6.0), padding: UiRect::vertical(Val::Px(3.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                for (i, x) in cellules.into_iter().enumerate() {
                    l.spawn((Node { width: if i == 0 { Val::Auto } else { Val::Px(78.0) }, flex_grow: if i == 0 { 1.0 } else { 0.0 }, justify_content: if i == 0 { JustifyContent::FlexStart } else { JustifyContent::FlexEnd }, ..default() }, children![texte(x, taille, if i == 0 { couleur } else { charte::TEXTE })]));
                }
            });
        };
        ligne(t, ["".into(), "aujourd'hui".into(), "+100".into(), "pleine".into()], charte::MUET, 11.0);
        for g in [Grandeur::Nourriture, Grandeur::Production] {
            if valeur(&plein, g) > 0.05 {
                ligne(t, [g.nom().into(), rules::decimal(valeur(&c.rendement, g)), rules::decimal(valeur(&plus, g)), rules::decimal(valeur(&plein, g))], g.couleur(), 13.0);
            }
        }
        if c.amenagement.is_some() && sub.total() > 0.0 {
            t.spawn(texte(format!("Pleine, dont la subsistance de ceux qui y vivent : {} nourriture, {} production (75 % de la cueillette du terrain)", rules::decimal(sub.nourriture), rules::decimal(sub.production)), 12.0, charte::MUET));
        }
    });
    let _ = images;
}

/// Courbe d'une grandeur : les tours de l'historique, puis aujourd'hui (rendement actuel) ; seuils.
fn courbe(city: &City, g: Grandeur, periode: usize, prevision: &[rules::Pas]) -> crate::graphique::Courbe {
    use crate::graphique::{Courbe, Seuil};
    let n = if periode == 0 { rules::HISTORIQUE_TOURS } else { periode };
    let mut passe: Vec<f64> = city.state.historique.iter().map(|r| valeur(&Rendements::new(r.nourriture, r.production, r.deniers), g)).collect();
    passe.push(valeur(&city.travail.rendements, g));
    let passe = passe[passe.len().saturating_sub(n)..].to_vec();
    let besoin = city.state.besoin_nourriture();
    // Le besoin suit les habitants : passés (historique), prévus (prévision)
    let mut habitants: Vec<f64> = city.state.historique.iter().map(|r| r.habitants as f64).collect();
    habitants.push(city.state.habitants as f64);
    let habitants = habitants[habitants.len().saturating_sub(passe.len())..].to_vec();
    let besoins = |k: f64| -> Option<(Vec<f64>, Vec<f64>)> {
        let par = k / rules::HABITANTS_PAR_NOURRITURE;
        Some((habitants.iter().map(|h| h * par).collect(), prevision.iter().skip(1).take(20).map(|p| p.habitants as f64 * par).collect()))
    };
    let seuils = match g {
        Grandeur::Nourriture => vec![
            Seuil { valeur: besoin, libelle: "besoin".into(), couleur: charte::GRAVE, serie: besoins(1.0) },
            Seuil { valeur: besoin * (1.0 + rules::SURPLUS_POUR_CROITRE), libelle: "croissance pleine (1,5 ×)".into(), couleur: charte::ALERTE, serie: besoins(1.0 + rules::SURPLUS_POUR_CROITRE) },
        ],
        Grandeur::Deniers => vec![Seuil { valeur: 0.0, libelle: "équilibre".into(), couleur: charte::ALERTE, serie: None }],
        _ => Vec::new(),
    };
    // (le premier pas de la prévision est le tour présent, déjà au bout du passé)
    let futur: Vec<f64> = prevision.iter().skip(1).take(20).map(|p| valeur(&p.rendements, g)).collect();
    Courbe { futur: futur.len().max(1), prevision: (!futur.is_empty()).then_some(futur), ..Courbe::simple(passe, 20, seuils, g.couleur()) }
}

/// Fil d'Ariane : vue d'ensemble › grandeur › case (les étapes passées sont des boutons).
fn fil(g: &mut ChildSpawnerCommands, grandeur: Option<Grandeur>, case: Option<&str>) {
    let lien = |p: &mut ChildSpawnerCommands, f: u8, t: &str| {
        p.spawn((Fil(f), UiPointer::default(), Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(Color::NONE), children![texte(t, 13.5, Color::srgb(0.56, 0.71, 0.94))]));
    };
    g.spawn((Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(10.0)), ..default() }, BackgroundColor(charte::PANNEAU))).with_children(|f| {
        lien(f, 0, "Vue d'ensemble");
        if let Some(gr) = grandeur {
            f.spawn(texte("›", 13.5, charte::MUET));
            if case.is_some() {
                lien(f, 1, gr.nom());
            } else {
                f.spawn(texte(gr.nom(), 13.5, charte::TEXTE));
            }
        }
        if let Some(c) = case {
            f.spawn(texte("›", 13.5, charte::MUET));
            f.spawn(texte(c, 13.5, charte::TEXTE));
        }
        f.spawn(Node { flex_grow: 1.0, ..default() });
        f.spawn(texte("Échap : revenir", 12.5, charte::MUET));
    });
}

/// Une ligne d'origine ou d'usage : libellé, barre, valeur.
fn ligne_part(p: &mut ChildSpawnerCommands, libelle: &str, v: Option<f64>, max: f64, couleur: Color) {
    p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
        l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(libelle, 13.0, if v.is_some() { charte::TEXTE } else { charte::MUET })]));
        match v {
            Some(v) => {
                l.spawn(barre((v.abs() / max.max(0.01)) as f32, 60.0, couleur));
                l.spawn((Node { width: Val::Px(44.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(rules::decimal(v), 13.0, charte::TEXTE)]));
            }
            None => {
                l.spawn(texte("—", 13.0, charte::MUET));
            }
        }
    });
}

/// Graphique (image) et ses libellés : graduations, seuils, tours.
fn poser_graphique(p: &mut ChildSpawnerCommands, image: Handle<Image>, cb: &crate::graphique::Courbe, tr: &crate::graphique::Trace) {
    use crate::graphique::{MARGE_DROITE, MARGE_GAUCHE};
    p.spawn(Node { width: Val::Px(tr.taille.x), height: Val::Px(tr.taille.y), flex_shrink: 0.0, ..default() }).with_children(|gph| {
        gph.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(tr.taille.x), height: Val::Px(tr.taille.y), ..default() }, ImageNode::new(image)));
        let poser = |gph: &mut ChildSpawnerCommands, gauche: f32, haut: f32, largeur: f32, droite: bool, t: String, couleur: Color| {
            gph.spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(gauche), top: Val::Px(haut), width: Val::Px(largeur), justify_content: if droite { JustifyContent::FlexEnd } else { JustifyContent::Center }, ..default() },
                children![texte(t, 11.5, couleur)],
            ));
        };
        for v in tr.graduations() {
            let t = if cb.log { rules::milliers(v.round() as u64) } else { rules::decimal(v) };
            poser(gph, 0.0, tr.y(v) - 8.0, MARGE_GAUCHE - 6.0, true, t, charte::MUET);
        }
        // (deux seuils proches : le second à gauche, pour que les libellés ne se chevauchent pas)
        // (libellé à droite : à la hauteur du seuil au bout de la prévision)
        let fin = |s: &crate::graphique::Seuil| s.serie.as_ref().and_then(|(_, b)| b.last().copied()).unwrap_or(s.valeur);
        let mut seuils: Vec<&crate::graphique::Seuil> = cb.seuils.iter().collect();
        seuils.sort_by(|a, b| tr.y(fin(a)).total_cmp(&tr.y(fin(b))));
        let mut dernier: Option<(f32, bool)> = None;
        for s in seuils {
            let y = tr.y(fin(s));
            let droite = !matches!(dernier, Some((y0, true)) if (y - y0).abs() < 16.0);
            let gauche = if droite { MARGE_GAUCHE } else { MARGE_GAUCHE + 4.0 };
            gph.spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(gauche), top: Val::Px(y - 17.0), width: Val::Px(tr.taille.x - MARGE_GAUCHE - MARGE_DROITE - 8.0), justify_content: if droite { JustifyContent::FlexEnd } else { JustifyContent::FlexStart }, ..default() },
                children![texte(s.libelle.clone(), 11.5, s.couleur)],
            ));
            dernier = Some((y, droite));
        }
        let bas = tr.taille.y - 18.0;
        let n = cb.passe.len();
        // (pas de libellé du passé trop près de « aujourd'hui » : ils se chevaucheraient)
        if n > 1 && tr.x(0.0) - tr.x(-(n as f64 - 1.0)) > 90.0 {
            poser(gph, tr.x(-(n as f64 - 1.0)) - 40.0, bas, 80.0, false, format!("−{} tours", n - 1), charte::MUET);
        }
        poser(gph, tr.x(0.0) - 45.0, bas, 90.0, false, "aujourd'hui".into(), charte::TEXTE);
        poser(gph, tr.x(cb.futur as f64) - 80.0, bas, 80.0, true, format!("+{} tours", cb.futur), charte::MUET);
    });
}

/// Prévision de l'écran de la ville, en tours.
const HORIZON: usize = 100;

/// Habitants que nourrirait la ville, toutes ses cases pleines (centre compris) : la limite de sa
/// population (décision B4).
fn limite_nourriture(cases: &[CaseVue], c: &rules::CityState) -> f64 {
    cases.iter().map(|c| c.max.nourriture).sum::<f64>() * rules::HABITANTS_PAR_NOURRITURE * rules::facteur_equilibre(c.types_nourriture)
}

/// Habitants qui ont tous un emploi quand toutes les cases sont pleines (la moitié travaille) : pas
/// une limite (décision B3), mais au-delà, le chômage baissera l'attractivité de la ville.
fn emplois(cases: &[CaseVue]) -> f64 {
    cases.iter().map(|c| c.capacite).sum::<u32>() as f64 / intendant::PART_ACTIVE
}

/// Courbe de la population : le passé, la prévision (l'intendant répartit les habitants à venir),
/// la limite de la nourriture, les emplois et le palier suivant ; échelle logarithmique.
fn courbe_population(city: &City, cases: &[CaseVue], prevision: &[rules::Pas]) -> crate::graphique::Courbe {
    use crate::graphique::{Courbe, Seuil};
    let mut passe: Vec<f64> = city.state.historique.iter().map(|r| r.habitants as f64).collect();
    if passe.last() != Some(&(city.state.habitants as f64)) {
        passe.push(city.state.habitants as f64);
    }
    let prevision: Vec<f64> = prevision.iter().take(40).map(|p| p.habitants as f64).collect();
    let (nourriture, travail) = (limite_nourriture(cases, &city.state), emplois(cases));
    let mut seuils = vec![
        Seuil { valeur: nourriture.max(1.0), libelle: format!("limite (nourriture) : {}", rules::milliers(nourriture as u64)), couleur: Grandeur::Nourriture.couleur(), serie: None },
        Seuil { valeur: travail.max(1.0), libelle: format!("emplois : {}", rules::milliers(travail as u64)), couleur: Grandeur::Production.couleur(), serie: None },
    ];
    let palier = city.state.palier() + 1;
    seuils.push(Seuil { valeur: rules::habitants_du_palier(palier) as f64, libelle: format!("palier {palier}"), couleur: charte::MUET, serie: None });
    Courbe { passe, futur: prevision.len().max(1), seuils, couleur: Grandeur::Habitants.couleur(), prevision: Some(prevision), log: true }
}

/// Détail d'une grandeur : graphique (passé, prévision, seuils), d'où elle vient, où elle va.
#[allow(clippy::too_many_arguments)]
/// En-tête d'une fiche de droite : liseré de couleur, titre, sous-titre, « × Fermer ».
fn entete_fiche(d: &mut ChildSpawnerCommands, couleur: Color, titre_: &str, sous_titre: String) {
    d.spawn((Node { column_gap: Val::Px(8.0), align_items: AlignItems::FlexStart, padding: UiRect::left(Val::Px(10.0)), border: UiRect::left(Val::Px(3.0)), ..default() }, BorderColor::all(couleur))).with_children(|t| {
        t.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, row_gap: Val::Px(2.0), ..default() }).with_children(|n| {
            n.spawn(texte(titre_, 19.0, charte::TEXTE));
            n.spawn(texte(sous_titre, 12.5, charte::DOUX));
        });
        t.spawn((FermerFiche, bouton(charte::SURFACE), children![texte("× Fermer", 12.5, charte::TEXTE)]));
    });
}

/// Fiche d'une grandeur (droite) : ce qu'on règle. Réglages de l'intendant avec leur effet ; pour
/// la nourriture, ses types et le grenier ; pour la production, les gisements de la ville.
fn fiche_grandeur(d: &mut ChildSpawnerCommands, gr: Grandeur, city: &City, cases: &[CaseVue], images: &Images, reglages: &[(Reglage, Rendements, i64)]) {
    let c = &city.state;
    let y = city.travail.rendements;
    let sous = match gr {
        Grandeur::Nourriture => format!("{} par tour · besoin {} · couverture × {}", rules::decimal(y.nourriture), rules::decimal(c.besoin_nourriture()), rules::decimal(y.nourriture / c.besoin_nourriture().max(0.001))),
        _ => format!("{} par tour · la courbe et d'où elle vient : dans le tiroir (Tab)", rules::decimal(valeur(&y, gr))),
    };
    entete_fiche(d, gr.couleur(), gr.nom(), sous);
    let _ = images;
    if !reglages.is_empty() {
        reglages_intendant(d, gr, city, reglages);
    }
    if gr == Grandeur::Nourriture {
        types_de_nourriture(d, city);
        grenier(d, city);
    }
    if gr == Grandeur::Production {
        gisements_de_la_ville(d, city, cases);
    }
}

/// Ce que donnent les cases travaillées, par origine : la cueillette des cases sans aménagement, le
/// travail de chaque sorte d'aménagement, la subsistance de ceux qui vivent sur les cases aménagées,
/// et les outils (le reste jusqu'au total des cases).
fn origines(gr: Grandeur, total_cases: f64, cases: &[CaseVue]) -> Vec<(String, f64)> {
    let (mut cueillette, mut n_cueillette, mut subsistance, mut n_amenagees) = (0.0, 0, 0.0, 0);
    let mut par_amenagement: Vec<(Amenagement, f64, usize)> = Vec::new();
    for c in cases.iter().filter(|c| !c.centre && c.travailleurs > 0) {
        match c.amenagement {
            None => {
                cueillette += valeur(&c.rendement, gr);
                n_cueillette += 1;
            }
            Some(a) => {
                let s = valeur(&c.courbe.subsistance_de(c.travailleurs), gr);
                subsistance += s;
                n_amenagees += 1;
                let t = valeur(&c.rendement, gr) - s;
                match par_amenagement.iter_mut().find(|x| x.0 == a) {
                    Some(x) => {
                        x.1 += t;
                        x.2 += 1;
                    }
                    None => par_amenagement.push((a, t, 1)),
                }
            }
        }
    }
    let nombre = |n: usize| if n > 1 { format!("{n} cases") } else { "1 case".to_string() };
    let mut v = Vec::new();
    if n_cueillette > 0 && cueillette.abs() >= 0.005 {
        v.push((format!("Cueillette ({})", nombre(n_cueillette)), cueillette));
    }
    par_amenagement.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (a, t, n) in par_amenagement {
        if t.abs() >= 0.005 {
            v.push((format!("{} ({})", capitale(a.nom()), nombre(n)), t));
        }
    }
    if subsistance >= 0.005 {
        v.push((format!("Subsistance sur les cases aménagées ({})", nombre(n_amenagees)), subsistance));
    }
    let outils = total_cases - v.iter().map(|x| x.1).sum::<f64>();
    if outils.abs() >= 0.05 {
        v.push(("Outils (cases aménagées)".into(), outils));
    }
    v
}

/// Infobulle d'une grandeur de l'en-tête : d'où vient son total et où il va (culture, bien-être
/// et salubrité : ce qui les fait).
fn bulle_grandeur(b: &mut ChildSpawnerCommands, gr: Grandeur, city: &City, cases: &[CaseVue]) {
    if matches!(gr, Grandeur::Culture | Grandeur::BienEtre | Grandeur::Eau) {
        bulle_cause(b, gr, city);
        return;
    }
    let total = valeur(&city.travail.rendements, gr);
    let (sources, usages) = sources_usages(gr, city, cases);
    let ligne = |b: &mut ChildSpawnerCommands, l: &str, v: f64, couleur: Color| {
        b.spawn((Node { column_gap: Val::Px(10.0), padding: UiRect::vertical(Val::Px(2.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|r| {
            r.spawn((Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }, children![texte(l, 12.5, charte::TEXTE)]));
            r.spawn(texte(rules::decimal(v), 12.5, couleur));
        });
    };
    b.spawn(texte(format!("{} : {} par tour", capitale(gr.nom()), rules::decimal(total)), 14.0, gr.couleur()));
    let (t_src, t_use) = if gr == Grandeur::Deniers { ("Recettes", "Dépenses") } else { ("D'où elle vient", "Où elle va") };
    b.spawn((texte(t_src, 11.5, charte::MUET), Node { margin: UiRect::top(Val::Px(4.0)), ..default() }));
    for (l, v) in &sources {
        if let Some(v) = v {
            ligne(b, l, *v, charte::TEXTE);
        }
    }
    b.spawn((texte(t_use, 11.5, charte::MUET), Node { margin: UiRect::top(Val::Px(4.0)), ..default() }));
    for (l, v, couleur) in &usages {
        if let Some(v) = v {
            ligne(b, l, *v, *couleur);
        }
    }
    b.spawn((texte("Clic : la courbe, le détail et les réglages", 11.5, charte::MUET), Node { margin: UiRect::top(Val::Px(4.0)), ..default() }));
}

/// Infobulle de la culture, du bien-être ou de la salubrité : ce qui les fait, terme à terme.
fn bulle_cause(b: &mut ChildSpawnerCommands, gr: Grandeur, city: &City) {
    let c = &city.state;
    let y = city.travail.rendements;
    let ligne = |b: &mut ChildSpawnerCommands, l: &str, v: f64| {
        b.spawn((Node { column_gap: Val::Px(10.0), padding: UiRect::vertical(Val::Px(2.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|r| {
            r.spawn((Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }, children![texte(capitale(l), 12.5, charte::TEXTE)]));
            r.spawn(texte(format!("{}{}", if v >= 0.0 { "+" } else { "−" }, rules::decimal(v.abs())), 12.5, if v < 0.0 { charte::GRAVE } else { charte::TEXTE }));
        });
    };
    let note = |b: &mut ChildSpawnerCommands, t: String| {
        b.spawn((texte(t, 11.5, charte::MUET), Node { margin: UiRect::top(Val::Px(4.0)), ..default() }));
    };
    match gr {
        Grandeur::Culture => {
            let total = c.culture_du_tour(y);
            b.spawn(texte(format!("Culture : +{} par tour", rules::decimal(total)), 14.0, gr.couleur()));
            note(b, "D'où elle vient".into());
            ligne(b, "la ville", rules::CULTURE_PAR_TOUR);
            if c.sanctuaire.is_some() {
                ligne(b, "sanctuaire", rules::CULTURE_SANCTUAIRE);
            }
            let projet = total - rules::CULTURE_PAR_TOUR - if c.sanctuaire.is_some() { rules::CULTURE_SANCTUAIRE } else { 0.0 };
            if projet > 0.005 {
                ligne(b, "projet Culture (production)", projet);
            }
            let seuil = rules::seuil_culture(c.cases_gagnees);
            let quand = c.tours_avant_case(total).map_or(String::new(), |t| format!(", dans {t} tours"));
            note(b, format!("Accumulée : {} sur {} pour la prochaine case gagnée{quand}.", rules::decimal(c.culture), rules::decimal(seuil)));
        }
        Grandeur::BienEtre => {
            let vise = c.bien_etre_vise(y, city.travail.chomeurs);
            b.spawn(texte(format!("Bien-être : {:.0} · va vers {vise:.0}", c.bien_etre), 14.0, gr.couleur()));
            note(b, "Ce qui le fait (visé)".into());
            ligne(b, "base", rules::BIEN_ETRE_NEUTRE);
            for (n, v) in c.termes_bien_etre(y, city.travail.chomeurs) {
                ligne(b, n, v);
            }
            note(b, "Il s'approche de sa visée d'un cinquième de l'écart par tour. Clic : l'onglet Population.".into());
        }
        _ => {
            b.spawn(texte(format!("Salubrité : {:.0} sur 100", c.salubrite()), 14.0, gr.couleur()));
            note(b, "Ce qui la fait".into());
            ligne(b, "base", rules::SALUBRITE_BASE);
            for (n, v) in c.termes_salubrite() {
                ligne(b, n, v);
            }
            note(b, format!("Sous {:.0}, des habitants meurent de maladies. Clic : l'onglet Population.", rules::SALUBRITE_SEUIL));
        }
    }
}

/// D'où vient une grandeur et où elle va (sources et usages, avec leur valeur).
#[allow(clippy::type_complexity)]
fn sources_usages(gr: Grandeur, city: &City, cases: &[CaseVue]) -> (Vec<(String, Option<f64>)>, Vec<(String, Option<f64>, Color)>) {
    let c = &city.state;
    let total = valeur(&city.travail.rendements, gr);
    let centre = cases.iter().find(|c| c.centre).map_or(0.0, |c| valeur(&c.rendement, gr));
    let besoin = c.besoin_nourriture();
    let mut sources: Vec<(String, Option<f64>)> = vec![("Centre de la ville".into(), Some(centre))];
    sources.extend(origines(gr, total - centre, cases).into_iter().map(|(l, v)| (l, Some(v))));
    sources.push(("Bâtiments : pas encore dans le jeu".into(), None));
    sources.push(("Commerce : pas encore dans le jeu".into(), None));
    let usages: Vec<(String, Option<f64>, Color)> = match gr {
        Grandeur::Nourriture => {
            let par = (rules::HABITANTS_PAR_NOURRITURE * rules::facteur_equilibre(c.types_nourriture)).round() as u64;
            let mut u = vec![(format!("{} habitants (1 pour {}, repas à {} type{})", rules::milliers(c.habitants), rules::milliers(par), c.types_nourriture.max(1), if c.types_nourriture > 1 { "s" } else { "" }), Some(besoin.min(total)), gr.couleur())];
            if total < besoin {
                // (le grenier comble d'abord le manque)
                let puise = c.tire_du_grenier(total);
                if puise > 0.05 {
                    u.push(("Puisé au grenier".into(), Some(puise), charte::ALERTE));
                }
                if besoin - total - puise > 1e-9 {
                    u.push(("Manque : famine".into(), Some(besoin - total - puise), charte::GRAVE));
                }
            } else {
                // Le grenier reçoit ce que la croissance n'utilise pas (la zone d'équilibre, puis
                // au-delà de la croissance pleine), jusqu'à sa capacité
                let place = (c.capacite_grenier() - c.stock_total()).max(0.0);
                let zone = (total - besoin).clamp(0.0, rules::ZONE_EQUILIBRE * besoin);
                let au_dela = (total - besoin * (1.0 + rules::SURPLUS_POUR_CROITRE)).max(0.0);
                let de_la_zone = zone.min(place);
                let du_reste = au_dela.min(place - de_la_zone);
                u.push(("Réserve pour croître (jusqu'à +50 %)".into(), Some((total - besoin).min(besoin * rules::SURPLUS_POUR_CROITRE) - de_la_zone), gr.couleur()));
                if de_la_zone + du_reste > 0.05 {
                    u.push(("Au grenier".into(), Some(de_la_zone + du_reste), charte::OK));
                }
                if au_dela - du_reste > 0.05 {
                    let pourquoi = if c.grenier { "grenier plein, pas encore de commerce" } else { "pas de grenier, pas encore de commerce" };
                    u.push((format!("Excédent : perdu ({pourquoi})"), Some(au_dela - du_reste), charte::ALERTE));
                }
            }
            u
        }
        Grandeur::Production => match (c.file.first(), c.projet) {
            (Some(k), _) => vec![(format!("Construction : {}", capitale(k.objet.name())), Some(total), gr.couleur())],
            (None, Some(p)) => vec![(format!("Projet : {}", p.nom()), Some(total), gr.couleur())],
            (None, None) => vec![("Rien à produire : à choisir pour finir le tour".into(), Some(total), charte::ALERTE)],
        },
        _ => vec![("Versés au trésor".into(), Some(total), gr.couleur()), ("Entretien et solde : pas encore dans le jeu".into(), None, gr.couleur())],
    };
    (sources, usages)
}

/// Tiroir d'une grandeur : la courbe (passé et prévision) à gauche, d'où elle vient et où elle va à
/// droite, côte à côte.
#[allow(clippy::too_many_arguments)]
fn bilan_grandeur(l: &mut ChildSpawnerCommands, gr: Grandeur, city: &City, cases: &[CaseVue], image: Handle<Image>, cb: &crate::graphique::Courbe, tr: &crate::graphique::Trace, periode: usize) {
    let total = valeur(&city.travail.rendements, gr);
    let besoin = city.state.besoin_nourriture();
    let (sources, usages) = sources_usages(gr, city, cases);
    let max = sources.iter().filter_map(|s| s.1).chain(usages.iter().filter_map(|u| u.1)).fold(0.01, f64::max);
    l.spawn(Node { column_gap: Val::Px(16.0), ..default() }).with_children(|r| {
        r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), flex_shrink: 0.0, ..default() }).with_children(|p| {
            p.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|b| {
                b.spawn(texte("passé :", 12.0, charte::MUET));
                for (n, t) in [(10, "10 tours"), (20, "20 tours"), (40, "40 tours")] {
                    let actif = periode == n || (periode == 0 && n == 40);
                    b.spawn((
                        ChoixPeriode(n),
                        UiPointer::default(),
                        Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                        BackgroundColor(if actif { charte::TEXTE } else { Color::NONE }),
                        BorderColor::all(charte::TRAIT2),
                        children![texte(t, 12.0, if actif { charte::FOND } else { charte::DOUX })],
                    ));
                }
                if cb.passe.len() < 3 {
                    b.spawn(texte("· l'historique se remplit à chaque fin de tour", 12.0, charte::MUET));
                }
            });
            poser_graphique(p, image, cb, tr);
        });
        r.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_width: Val::Px(0.0), row_gap: Val::Px(2.0), ..default() }).with_children(|col| {
            let (t_src, t_use) = if gr == Grandeur::Deniers { ("Recettes", "Dépenses") } else { ("D'où elle vient", "Où elle va") };
            col.spawn(texte(t_src, 13.5, charte::TEXTE));
            for (l, v) in &sources {
                ligne_part(col, l, *v, max, gr.couleur());
            }
            col.spawn((Node { height: Val::Px(6.0), ..default() },));
            col.spawn(texte(t_use, 13.5, charte::TEXTE));
            for (l, v, couleur) in &usages {
                ligne_part(col, l, *v, max, *couleur);
            }
            let net = match gr {
                Grandeur::Nourriture => format!("{} × le besoin", rules::decimal(total / besoin.max(0.001))),
                _ => format!("{} par tour", rules::decimal(total)),
            };
            col.spawn((Node { padding: UiRect::vertical(Val::Px(4.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT2))).with_children(|t| {
                t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("Total", 13.5, charte::TEXTE)]));
                t.spawn(texte(net, 13.5, charte::TEXTE));
            });
        });
    });
}

/// Fiche d'une cause de la croissance : bien-être ou salubrité, ce qui la fait et ce qu'elle fait.
fn fiche_cause(d: &mut ChildSpawnerCommands, s: Sujet, city: &City) {
    let c = &city.state;
    let y = city.travail.rendements;
    let cr = c.croissance(y);
    let h = c.habitants as f64;
    let ligne = |d: &mut ChildSpawnerCommands, nom: &str, v: f64| {
        d.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(4.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(capitale(nom), 13.5, charte::TEXTE)]));
            l.spawn(texte(format!("{}{:.0}", if v >= 0.0 { "+" } else { "−" }, v.abs()), 13.5, if v < 0.0 { charte::GRAVE } else { charte::OK }));
        });
    };
    match s {
        Sujet::BienEtre => {
            let vise = c.bien_etre_vise(y, city.travail.chomeurs);
            entete_fiche(d, Grandeur::BienEtre.couleur(), "Bien-être", format!("{:.0} · {} · va vers {vise:.0}", c.bien_etre, c.humeur().nom()));
            d.spawn(texte("CE QUI LE FAIT (VISÉ)", 11.5, charte::MUET));
            ligne(d, "base", rules::BIEN_ETRE_NEUTRE);
            for (n, v) in c.termes_bien_etre(y, city.travail.chomeurs) {
                ligne(d, n, v);
            }
            d.spawn(texte("Le bien-être s'approche de sa visée d'un cinquième de l'écart par tour.", 12.5, charte::MUET));
            d.spawn(texte("CE QU'IL FAIT", 11.5, charte::MUET));
            let gain = cr.naturelle - cr.naturelle / cr.facteur_bien_etre.max(1e-6);
            ligne_valeur(d, "Croissance naturelle", format!("× {} · {}{} hab./tour", rules::decimal(cr.facteur_bien_etre), if gain >= 0.0 { "+" } else { "−" }, rules::milliers((gain * h).abs().round() as u64)), charte::TEXTE);
        }
        Sujet::Salubrite => {
            let sal = c.salubrite();
            let maladies = (h * cr.insalubrite).round();
            let etat = if maladies < 0.0 { format!("maladies −{} hab./tour", rules::milliers(maladies.abs() as u64)) } else { "pas de maladie".to_string() };
            entete_fiche(d, Grandeur::Eau.couleur(), "Salubrité", format!("{sal:.0} sur 100 · {etat}"));
            d.spawn(texte("CE QUI LA FAIT", 11.5, charte::MUET));
            ligne(d, "base", rules::SALUBRITE_BASE);
            for (n, v) in c.termes_salubrite() {
                ligne(d, n, v);
            }
            d.spawn(texte(format!("Sous {:.0}, des habitants meurent de maladies (jusqu'à 1 % par tour à 0). Au-dessus, la salubrité n'a pas d'effet : c'est une marge.", rules::SALUBRITE_SEUIL), 12.5, charte::MUET));
        }
        // (la fiche d'une ressource a la sienne)
        Sujet::Ressource(_) => {}
    }
}

/// Quantité de ressource : deux décimales sous 1 (une usure de 0,04 par tour ne s'écrit pas 0,0).
fn quantite(x: f64) -> String {
    if x.abs() < 1.0 && x != 0.0 { format!("{x:.2}").replace('.', ",") } else { rules::decimal(x) }
}

/// Panneau « Gisements et ressources » : les gisements connus du territoire, ce que chaque
/// exploitation tire par tour, et le stock de la ville (son usage viendra avec les livraisons
/// suivantes).
fn gisements_de_la_ville(g: &mut ChildSpawnerCommands, city: &City, cases: &[CaseVue]) {
    let connus: Vec<&CaseVue> = cases.iter().filter(|c| c.gisement.is_some()).collect();
    let stock: Vec<(crate::gisements::Ressource, f64)> = crate::gisements::Ressource::TOUTES.iter().map(|&r| (r, city.state.ressource(r))).filter(|(_, q)| *q >= 0.005).collect();
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, &format!("Gisements et ressources : {} gisement{} connu{}", connus.len(), if connus.len() > 1 { "s" } else { "" }, if connus.len() > 1 { "s" } else { "" }));
        if connus.is_empty() {
            p.spawn(texte("Aucun gisement connu dans le territoire. Les affleurements (sel, silex, obsidienne, ocre) et les ressources vivantes se voient dès qu'une case est explorée ; la ville prospecte deux cases de son territoire par tour (pierre dure, pierres et parures), un éclaireur prospecte en passant.", 12.5, charte::MUET));
        }
        for c in &connus {
            let r = c.gisement.expect("gisement connu");
            let valeur = if c.amenagement == Some(Amenagement::Exploitation) {
                format!("{} par tour ({} / {} travailleurs)", quantite(intendant::unites_extraites(c.travailleurs, c.capacite)), rules::milliers(u64::from(c.travailleurs)), rules::milliers(u64::from(c.capacite)))
            } else {
                "pas exploité".to_string()
            };
            ligne_valeur(p, &format!("{} · {}", capitale(r.nom()), c.nom.to_lowercase()), valeur, if c.amenagement == Some(Amenagement::Exploitation) { charte::OK } else { charte::MUET });
        }
        if !stock.is_empty() {
            let liste: Vec<String> = stock.iter().map(|(r, q)| format!("{} {}", r.nom(), quantite(*q))).collect();
            p.spawn(texte(format!("Stock de la ville : {}.", liste.join(", ")), 13.0, charte::TEXTE));
        }
        // Outils (5h) : usure par les travailleurs des cases aménagées, part couverte, bonus
        let c = &city.state;
        let amenages: u32 = cases.iter().filter(|k| k.amenagement.is_some()).map(|k| k.travailleurs).sum();
        let d = rules::CityState::demande_outils(amenages);
        if d > 0.0 {
            let couleur = if c.couverture_outils >= 0.999 { charte::OK } else if c.couverture_outils > 0.0 { charte::ALERTE } else { charte::MUET };
            ligne_valeur(
                p,
                &format!("Outils · usure {} par tour ({} travailleurs aux aménagements)", quantite(d), rules::milliers(u64::from(amenages))),
                format!("couvert {:.0} % · cases aménagées +{:.0} %", c.couverture_outils * 100.0, c.bonus_outils * 100.0),
                couleur,
            );
        }
        // Besoins de base et luxe (5g) : demande, part couverte au dernier tour, effet
        for r in crate::gisements::Ressource::TOUTES.into_iter().filter(|&r| c.demande(r) > 0.0) {
            let couv = c.couverture(r);
            let effet = match r {
                crate::gisements::Ressource::Sel => format!("salubrité +{}", quantite(rules::SALUBRITE_SEL * couv)),
                crate::gisements::Ressource::Plantes => format!("salubrité +{}", quantite(rules::SALUBRITE_PLANTES * couv)),
                crate::gisements::Ressource::Fibres => {
                    let manque = rules::SALUBRITE_VETEMENTS_MAX * (1.0 - couv) * c.part_besoin_vetements();
                    if manque > 0.05 { format!("manque de vêtements : salubrité −{}", quantite(manque)) } else { "vêtus".to_string() }
                }
                _ => format!("luxe : bien-être +{}", quantite(r.luxe().unwrap_or(0.0) * couv)),
            };
            let couleur = if couv >= 0.999 { charte::OK } else if couv > 0.0 { charte::ALERTE } else { charte::MUET };
            ligne_valeur(p, &format!("{} · demande {} par tour", capitale(r.nom()), quantite(c.demande(r))), format!("couvert {:.0} % · {effet}", couv * 100.0), couleur);
        }
        // Échanges du dernier tour avec les autres villes (5i)
        for x in &c.echanges {
            let (sens, couleur) = if x.recu { (format!("Reçu de {}", x.ville), charte::OK) } else { (format!("Donné à {}", x.ville), charte::DOUX) };
            let contre = match x.contre {
                Some((r, q)) => format!("contre {} de {}", quantite(q), r.nom()),
                None if x.recu => "un don".to_string(),
                None => "un don (elle n'avait rien à offrir)".to_string(),
            };
            ligne_valeur(p, &format!("{sens} · {} de {}", quantite(x.qte), x.ressource.nom()), contre, couleur);
        }
        let usages: Vec<String> = connus.iter().filter_map(|c| c.gisement).fold(Vec::new(), |mut v, r| {
            if !v.contains(&r) {
                v.push(r);
            }
            v
        }).iter().map(|r| format!("{} : {}", r.nom(), r.usage())).collect();
        if !usages.is_empty() {
            p.spawn(texte(format!("Usages à venir (besoins de base, luxe, outils, matériaux) — {}.", usages.join(" ; ")), 12.5, charte::MUET));
        }
    });
}

/// Une autre ville du joueur, vue pour les échanges.
struct Partenaire {
    nom: String,
    centre: Hex,
    distance: i32,
    /// Reliée à la ville ouverte, de proche en proche (`echanges::liaisons`).
    reliee: bool,
    /// Trajet depuis la ville ouverte (tours, de proche en proche), si elles sont reliées.
    tours: Option<f64>,
    /// Trajet direct à pied, même trop long (villes proches non reliées).
    a_pied: Option<f64>,
    /// Part du trajet faite en pirogue (tours).
    pirogue: f64,
    /// Villes traversées entre les deux.
    relais: usize,
}

impl Partenaire {
    /// Le trajet en mots : tours de trajet, ou la distance en cases si elle n'est pas reliée.
    fn trajet(&self) -> String {
        match self.tours {
            Some(t) => {
                let eau = if self.pirogue > 0.005 { format!(", dont {} en pirogue", rules::decimal(self.pirogue)) } else { String::new() };
                let relais = match self.relais {
                    0 => String::new(),
                    1 => ", par une autre ville".to_string(),
                    n => format!(", par {n} autres villes"),
                };
                format!("{} tour{} de trajet{eau}{relais}", rules::decimal(t), if t >= 2.0 { "s" } else { "" })
            }
            None => match self.a_pied {
                Some(t) => format!("{} tours de trajet à pied (étape de plus de {})", rules::decimal(t), rules::decimal(crate::echanges::TOURS_TRAJET_MAX)),
                None => format!("{} cases", self.distance),
            },
        }
    }
}

/// Les autres villes du joueur, de la plus proche à la plus lointaine, et si elles sont reliées.
fn partenaires(map: &crate::map::WorldMap, v: Entity, villes: &Query<(Entity, &City, &HexPos, &crate::peuple::Peuple)>) -> Vec<Partenaire> {
    let miennes: Vec<(Entity, &City, Hex)> = villes.iter().filter(|(_, _, _, p)| **p == crate::peuple::Peuple::JOUEUR).map(|(e, c, h, _)| (e, c, h.0)).collect();
    let Some(i) = miennes.iter().position(|(e, ..)| *e == v) else { return Vec::new() };
    let reseau = crate::echanges::Reseau::new(miennes.len(), |a, b| crate::echanges::etape(map, miennes[a].2, miennes[b].2));
    let ici = miennes[i].2;
    let mut v: Vec<Partenaire> = miennes
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != i)
        .map(|(j, (_, c, h))| Partenaire { nom: c.state.name.clone(), centre: *h, distance: map.distance(ici, *h), reliee: reseau.liaisons[i][j].is_some(), tours: reseau.liaisons[i][j].as_ref().map(|l| l.tours), pirogue: reseau.liaisons[i][j].as_ref().map_or(0.0, |l| l.tours_pirogue), relais: reseau.liaisons[i][j].as_ref().map_or(0, |l| l.route.len().saturating_sub(2)), a_pied: (reseau.liaisons[i][j].is_none() && map.distance(ici, *h) <= 2 * crate::echanges::CASES_MAX).then(|| crate::echanges::trajet_a_pied(map, ici, *h)).flatten() })
        .collect();
    v.sort_by(|a, b| a.distance.cmp(&b.distance).then(a.nom.cmp(&b.nom)));
    v
}

/// Onglet Ressources, à gauche : les échanges du tour avec chaque autre ville.
fn echanges_du_tour(g: &mut ChildSpawnerCommands, city: &City, partenaires: &[Partenaire]) {
    g.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
        t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("ÉCHANGES DU TOUR", 11.5, charte::MUET)]));
        t.spawn(texte(format!("étapes de {} tours au plus, à pied ou en pirogue", rules::decimal(crate::echanges::TOURS_TRAJET_MAX)), 11.5, charte::MUET));
    });
    if partenaires.is_empty() {
        g.spawn(texte("Aucune autre ville : les échanges viendront avec la deuxième.", 13.0, charte::MUET));
    }
    for p in partenaires {
        g.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|b| {
            b.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(format!("{} · {}", p.nom, p.trajet()), 13.5, charte::TEXTE)]));
                l.spawn(texte(if p.reliee { "reliée" } else { "non reliée" }, 12.5, if p.reliee { charte::OK } else { charte::MUET }));
            });
            let lignes: Vec<String> = city
                .state
                .echanges
                .iter()
                .filter(|x| x.ville == p.nom)
                .map(|x| {
                    let contre = x.contre.map_or(" (don)".to_string(), |(r, q)| format!(" contre {} de {}", quantite(q), r.nom()));
                    let mut frais = Vec::new();
                    if x.paye > 0.005 {
                        frais.push(format!("{} pour le transport", quantite(x.paye)));
                    }
                    if x.perdu > 0.005 {
                        frais.push(format!("{} perdu en route", quantite(x.perdu)));
                    }
                    let perte = if frais.is_empty() { String::new() } else { format!(", dont {}", frais.join(" et ")) };
                    format!("{} : {} {} par tour{perte}{contre}", if x.recu { "reçu" } else { "donné" }, x.ressource.nom(), quantite(x.qte))
                })
                .collect();
            if lignes.is_empty() {
                let t = if p.reliee { "rien ce tour : aucune des deux n'a en trop ce qui manque à l'autre".to_string() } else { "trop loin : une ville entre les deux les relierait".to_string() };
                b.spawn(texte(t, 12.0, charte::MUET));
            }
            for l in lignes {
                b.spawn(texte(l, 12.0, charte::DOUX));
            }
        });
    }
    if !city.state.transit.is_empty() {
        let liste: Vec<String> = city.state.transit.iter().map(|(r, q)| format!("{} {}", quantite(*q), r.nom())).collect();
        g.spawn(texte(format!("En transit par la ville ce tour : {}.", liste.join(", ")), 12.5, charte::DOUX));
    }
    g.spawn(texte("Une ville qui manque d'une ressource la reçoit d'une ville reliée qui en a au-delà de sa réserve, par troc ou par don, de ville en ville si besoin. Le transport se paie en nature : 5 % de la charge par tour à pied, 2 % en pirogue (on charge et décharge dans les villes) ; en route, les fibres et le bois perdent 5 % par tour, les plantes 10 %. L'échange se fait s'il reste rentable : plus une ville manque, plus elle accepte de perte. Une étape porte 4 unités par tour à pied, 12 en pirogue. Ressources du pays : touche G (carte fermée).", 12.0, charte::MUET));
}

/// Effet d'une ressource pour la ville : (actuel, maximal), en mots.
fn effet_ressource(c: &rules::CityState, r: crate::gisements::Ressource) -> (String, String) {
    use crate::gisements::Ressource;
    let couv = c.couverture(r);
    match r {
        Ressource::Sel => (format!("salubrité +{}", quantite(rules::SALUBRITE_SEL * couv)), format!("salubrité +{}", quantite(rules::SALUBRITE_SEL))),
        Ressource::Plantes => (format!("salubrité +{}", quantite(rules::SALUBRITE_PLANTES * couv)), format!("salubrité +{}", quantite(rules::SALUBRITE_PLANTES))),
        Ressource::Fibres if c.part_besoin_vetements() <= 0.0 => ("sans effet sous 2 000 habitants".into(), "évite jusqu'à −10 de salubrité".into()),
        Ressource::Fibres => (format!("salubrité −{}", quantite(rules::SALUBRITE_VETEMENTS_MAX * (1.0 - couv) * c.part_besoin_vetements())), "salubrité 0 (vêtus)".into()),
        Ressource::Silex | Ressource::PierreDure | Ressource::Obsidienne if c.demande_outils_tour > 0.0 => (format!("outils : cases aménagées +{:.0} %", c.bonus_outils * 100.0), format!("cases aménagées +{:.0} %", rules::BONUS_OUTILS[0] * 100.0)),
        Ressource::Silex | Ressource::PierreDure => ("outils : pas de case aménagée".into(), format!("cases aménagées +{:.0} %", rules::BONUS_OUTILS[0] * 100.0)),
        _ if let Some(rang) = r.luxe() => (format!("bien-être +{}", quantite(rang * couv)), format!("bien-être +{}", quantite(rang))),
        _ => ("pas encore d'usage".into(), "matériau des bâtiments à venir".into()),
    }
}

/// « salubrité +0,0 sur +5,0 » : l'effet actuel et le maximal, l'unité dite une fois.
fn actuel_sur_max(actuel: &str, max: &str) -> String {
    match (actuel.split_once(' '), max.split_once(' ')) {
        (Some((u1, _)), Some((u2, reste))) if u1 == u2 => format!("{actuel} sur {reste}"),
        _ => format!("{actuel} · au mieux {max}"),
    }
}

/// Une ressource est-elle « à portée » de la ville : demandée, en stock, reçue, ou d'un gisement
/// connu du territoire ou d'une case à acheter ?
fn a_portee(c: &rules::CityState, r: crate::gisements::Ressource, cases: &[CaseVue], achats: &[CaseAchat]) -> bool {
    c.demande_echange(r) > 0.0 || c.ressource(r) >= 0.005 || c.echanges.iter().any(|x| x.ressource == r) || cases.iter().any(|k| k.gisement == Some(r)) || achats.iter().any(|a| a.gisement == Some(r))
}

/// Tiroir de l'onglet Ressources : une ligne par ressource à portée (demande, couverture, origine,
/// effet actuel sur maximal, action), puis celles à chercher, en une ligne grise.
fn tableau_ressources(l: &mut ChildSpawnerCommands, city: &City, cases: &[CaseVue], achats: &[CaseAchat], sujet: Option<Sujet>) {
    use crate::gisements::Ressource;
    let c = &city.state;
    let colonnes = |l: &mut ChildSpawnerCommands, t: [&str; 5], couleur: Color, taille: f32| {
        l.spawn(Node { column_gap: Val::Px(10.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() }).with_children(|r| {
            for (i, (x, w)) in t.iter().zip([170.0, 80.0, 120.0, 0.0, 230.0]).enumerate() {
                let mut n = Node { width: if w > 0.0 { Val::Px(w) } else { Val::Auto }, flex_grow: if w > 0.0 { 0.0 } else { 1.0 }, flex_shrink: if w > 0.0 { 0.0 } else { 1.0 }, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() };
                if i == 1 {
                    n.justify_content = JustifyContent::FlexEnd;
                }
                r.spawn((n, children![(texte(*x, taille, couleur), TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap))]));
            }
        });
    };
    colonnes(l, ["RESSOURCE", "DEMANDE", "COUVERTURE", "ORIGINE", "EFFET ACTUEL SUR MAXIMAL"], charte::MUET, 11.0);
    let (portee, chercher): (Vec<Ressource>, Vec<Ressource>) = Ressource::TOUTES.iter().partition(|&&r| a_portee(c, r, cases, achats));
    for r in portee {
        let d = c.demande_echange(r);
        let couv = if d > 0.0 { c.couverture(r) } else { 0.0 };
        let gisement = cases.iter().find(|k| k.gisement == Some(r));
        let recu: f64 = c.echanges.iter().filter(|x| x.ressource == r && x.recu).map(|x| x.arrive()).sum::<f64>() + 0.0;
        let origine = match (gisement, recu > 0.0) {
            (Some(k), _) if k.amenagement == Some(Amenagement::Exploitation) => format!("extrait {}", quantite(c.extrait(r))),
            (Some(_), _) => "gisement non exploité".into(),
            (None, true) => format!("reçu {}", quantite(recu)),
            (None, false) if achats.iter().any(|a| a.gisement == Some(r)) => "gisement sur une case à acheter".into(),
            _ if c.ressource(r) >= 0.005 => format!("en stock {}", quantite(c.ressource(r))),
            _ => "rien à portée".into(),
        };
        let (actuel, max) = effet_ressource(c, r);
        let choisie = sujet == Some(Sujet::Ressource(r));
        l.spawn((
            ChoixRessource(r),
            UiPointer::default(),
            Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(if choisie { charte::SELECTION } else { Color::NONE }),
        ))
        .with_children(|ligne| {
            let cellule = |w: f32| Node { width: if w > 0.0 { Val::Px(w) } else { Val::Auto }, flex_grow: if w > 0.0 { 0.0 } else { 1.0 }, flex_shrink: if w > 0.0 { 0.0 } else { 1.0 }, min_width: Val::Px(0.0), overflow: Overflow::clip(), column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() };
            ligne.spawn((cellule(170.0), children![texte("◆", 12.0, GISEMENT), texte(capitale(r.nom()), 13.0, charte::TEXTE)]));
            ligne.spawn((Node { justify_content: JustifyContent::FlexEnd, ..cellule(80.0) }, children![texte(if d > 0.0 { quantite(d) } else { "—".into() }, 13.0, charte::DOUX)]));
            let couleur = if d <= 0.0 { charte::MUET } else if couv >= 0.95 { charte::OK } else if couv >= 0.5 { charte::ALERTE } else { charte::GRAVE };
            ligne.spawn(cellule(120.0)).with_children(|x| {
                if d > 0.0 {
                    x.spawn(barre(couv as f32, 54.0, couleur));
                    x.spawn(texte(format!("{:.0} %", couv * 100.0), 12.5, couleur));
                } else {
                    x.spawn(texte("pas demandée", 12.0, charte::MUET));
                }
            });
            ligne.spawn((cellule(0.0), children![(texte(origine, 12.5, charte::DOUX), TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap))]));
            ligne.spawn((cellule(230.0), children![(texte(actuel_sur_max(&actuel, &max), 12.5, if d > 0.0 && couv < 0.95 { charte::ALERTE } else { charte::DOUX }), TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap))]));
        });
    }
    if !chercher.is_empty() {
        let noms: Vec<&str> = chercher.iter().map(|r| r.nom()).collect();
        l.spawn((Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), column_gap: Val::Px(8.0), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|x| {
            x.spawn(texte(format!("À CHERCHER · {}", chercher.len()), 11.5, charte::MUET));
            x.spawn(texte(format!("{} : rien à faire dans la ville (aucun gisement connu ici)", noms.join(", ")), 12.0, charte::MUET));
        });
    }
}

/// Fiche d'une ressource (droite) : son effet aujourd'hui et au mieux, où en est la ville (demande,
/// extrait et reçu, stock sur la réglette 2 / 6 / 12 tours), le chemin pour l'avoir (du moins cher au
/// plus cher), ce que ça changerait.
fn fiche_ressource(d: &mut ChildSpawnerCommands, r: crate::gisements::Ressource, city: &City, cases: &[CaseVue], achats: &[CaseAchat], partenaires: &[Partenaire]) {
    use crate::gisements::Ressource;
    let c = &city.state;
    let genre = match r {
        Ressource::Sel | Ressource::Plantes | Ressource::Fibres => "besoin de base · salubrité",
        Ressource::Silex | Ressource::PierreDure => "outils et matériaux",
        Ressource::Obsidienne => "outils et luxe",
        Ressource::Ocre | Ressource::Parures => "luxe · bien-être",
        Ressource::BoisOeuvre => "matériau",
    };
    entete_fiche(d, GISEMENT, &capitale(r.nom()), genre.to_string());
    let section = |d: &mut ChildSpawnerCommands, t: &str| {
        d.spawn(texte(t, 11.5, charte::MUET));
    };
    let ligne = |d: &mut ChildSpawnerCommands, nom: &str, note: Option<String>, valeur: String, couleur: Color| {
        d.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(4.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
            l.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                n.spawn(texte(nom, 13.5, charte::TEXTE));
                if let Some(note) = note {
                    n.spawn(texte(note, 12.0, charte::MUET));
                }
            });
            l.spawn(texte(valeur, 13.0, couleur));
        });
    };
    let (actuel, max) = effet_ressource(c, r);
    section(d, "EFFET : ACTUEL SUR MAXIMAL");
    ligne(d, "Aujourd'hui", None, actuel, charte::TEXTE);
    ligne(d, "Au mieux", None, max, charte::OK);
    // Où en est la ville
    let dem = c.demande_echange(r);
    let recu: f64 = c.echanges.iter().filter(|x| x.ressource == r && x.recu).map(|x| x.arrive()).sum::<f64>() + 0.0;
    let stock = c.ressource(r);
    section(d, "OÙ J'EN SUIS");
    let note = match r {
        Ressource::Sel | Ressource::Plantes | Ressource::Fibres => Some("0,1 par millier d'habitants".to_string()),
        _ if r.luxe().is_some() => Some("0,1 par palier".to_string()),
        Ressource::Silex | Ressource::PierreDure | Ressource::Obsidienne => Some("usure des outils des cases aménagées (silex d'abord)".to_string()),
        _ => None,
    };
    ligne(d, "Demande", note, if dem > 0.0 { format!("{} par tour", quantite(dem)) } else { "aucune".into() }, charte::TEXTE);
    ligne(d, "Extrait · reçu", None, format!("{} · {}", quantite(c.extrait(r)), quantite(recu)), charte::TEXTE);
    let tours = if dem > 0.0 { stock / dem } else { 0.0 };
    let etat = if dem <= 0.0 { "rien n'est demandé" } else if tours < crate::echanges::TOURS_VISES { "manque : la ville demande" } else if tours < crate::echanges::TOURS_RESERVE { "suffit" } else if tours < crate::echanges::TOURS_PLAFOND { "offert aux villes reliées" } else { "au plafond : le reste est perdu" };
    ligne(d, "Stock", Some(etat.to_string()), if dem > 0.0 { format!("{} · {} tours de demande", quantite(stock), rules::decimal(tours)) } else { quantite(stock) }, charte::TEXTE);
    if dem > 0.0 {
        // Réglette 0 · 2 · 6 · 12 tours : manque, suffit, offert, perdu
        let largeur = 330.0;
        d.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).with_children(|r_| {
            r_.spawn(Node { width: Val::Px(largeur), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), overflow: Overflow::clip(), ..default() }).with_children(|b| {
                for (a, z, couleur) in [(0.0, 2.0, charte::GRAVE), (2.0, 6.0, charte::OK), (6.0, 12.0, Color::srgb(0.36, 0.65, 0.86))] {
                    b.spawn((Node { width: Val::Px(largeur * (z - a) / 12.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(couleur.with_alpha(0.7))));
                }
            });
            let x = (largeur * (tours.min(12.0) / 12.0) as f32 - 1.0).max(0.0);
            r_.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(-3.0), width: Val::Px(3.0), height: Val::Px(14.0), ..default() }, BackgroundColor(charte::TEXTE)));
            r_.spawn(texte("0 manque · 2 suffit · 6 offert · 12 perdu", 11.5, charte::MUET));
        });
    }
    // Chemin, du moins cher au plus cher
    section(d, "CHEMIN, DU MOINS CHER AU PLUS CHER");
    let mut rien = true;
    if let Some(k) = cases.iter().find(|k| k.gisement == Some(r)) {
        rien = false;
        let (t, bouton_) = if k.amenagement == Some(Amenagement::Exploitation) {
            (format!("Gisement exploité sur {} : {} par tour à pleine capacité", k.nom.to_lowercase(), quantite(1.0)), None)
        } else {
            (format!("Exploiter le gisement sur {} ({} de production) : 1 par tour à pleine capacité ; la case ne donne plus de nourriture", k.nom.to_lowercase(), Amenagement::Exploitation.cout_de_base()), Some(BoutonRemede(Remede::Placer(Amenagement::Exploitation))))
        };
        d.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(t, 12.5, charte::TEXTE)]));
            if let Some(b) = bouton_ {
                l.spawn((b, bouton(charte::ACTION_FOND), children![texte("Placer", 12.5, charte::TEXTE)]));
            }
        });
    }
    for p in partenaires.iter().filter(|p| p.reliee) {
        rien = false;
        d.spawn(texte(format!("{} est reliée : si elle en a au-delà de sa réserve, elle en donnera ou en échangera, sans rien à faire.", p.nom), 12.5, charte::DOUX));
    }
    if let Some(a) = achats.iter().find(|a| a.gisement == Some(r)) {
        rien = false;
        d.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(format!("Acheter la case {} ({} deniers), puis l'exploiter", a.nom.to_lowercase(), a.prix), 12.5, charte::TEXTE)]));
            l.spawn((ChoixCase(a.hex), bouton(charte::SURFACE), children![texte("Voir la case", 12.5, charte::TEXTE)]));
        });
    }
    if rien {
        d.spawn(texte("Aucun gisement connu à portée : la ville prospecte deux cases de son territoire par tour, un éclaireur en passant ; un gisement d'une autre ville du pays arrivera par les échanges si elles sont reliées.", 12.5, charte::MUET));
    }
    // Ce que ça change, ressource couverte en entier (calcul du jeu)
    if dem > 0.0 && matches!(r, Ressource::Sel | Ressource::Plantes | Ressource::Fibres) {
        let mut avec = c.clone();
        avec.couverture.resize(crate::gisements::N_RESSOURCES, 0.0);
        avec.couverture[r.index()] = 1.0;
        let y = city.travail.rendements;
        let h = c.habitants as f64;
        let (s0, s1) = (c.salubrite(), avec.salubrite());
        let (m0, m1) = ((h * c.croissance(y).insalubrite).round(), (h * avec.croissance(y).insalubrite).round());
        section(d, "CE QUE ÇA CHANGE");
        let suite = if (m1 - m0).abs() >= 0.5 { format!(", maladies {} → {} hab./tour", m0, m1) } else if s0 >= rules::SALUBRITE_SEUIL { " : au-dessus de 40, la salubrité n'a pas d'effet, c'est une marge".to_string() } else { String::new() };
        d.spawn(texte(format!("Couvert en entier : salubrité {s0:.0} → {s1:.0}{suite}."), 12.5, charte::TEXTE));
    }
}

/// Panneau « Grenier » : ce qu'il garde, ce qui y entre ou en sort au prochain tour, et ses
/// sortes avec leur perte par tour.
fn grenier(g: &mut ChildSpawnerCommands, city: &City) {
    let c = &city.state;
    let n = city.travail.rendements.nourriture;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        if !c.grenier {
            titre(p, "Grenier : pas encore bâti");
            p.spawn(texte(
                format!(
                    "Un grenier ({} production, dans Construire) garde jusqu'à {:.0} tours de besoin : la ville y met l'excédent que la croissance n'utilise pas et y puise en cas de famine.",
                    rules::GRENIER_COUT,
                    rules::GRENIER_TOURS
                ),
                12.5,
                charte::MUET,
            ));
            return;
        }
        let besoin = c.besoin_nourriture().max(1e-6);
        let total = c.stock_total();
        titre(p, &format!("Grenier : {} sur {} ({} tours de besoin)", rules::decimal(total), rules::decimal(c.capacite_grenier()), rules::decimal(total / besoin)));
        let puise = c.tire_du_grenier(n);
        let (libelle, valeur, couleur) = if puise > 0.0 {
            ("Au prochain tour : la ville y puise", format!("−{}", rules::decimal(puise)), charte::ALERTE)
        } else {
            let entre = c.excedent_pour_grenier(n).min((c.capacite_grenier() - total).max(0.0));
            ("Au prochain tour : il reçoit (avant les pertes)", format!("+{}", rules::decimal(entre)), if entre > 0.0 { charte::OK } else { charte::MUET })
        };
        ligne_valeur(p, libelle, valeur, couleur);
        for (s, q) in c.stock.iter().enumerate().filter(|(_, q)| **q >= 0.005) {
            let sorte = intendant::Sorte::TOUTES[s];
            ligne_valeur(p, &capitale(sorte.nom()), format!("{} · perte {:.0} % par tour", rules::decimal(*q), sorte.perte() * 100.0), charte::DOUX);
        }
        p.spawn(texte(
            "Il reçoit l'excédent que la croissance n'utilise pas (jusqu'à 3 % au-dessus du besoin, et au-delà de 1,5 fois le besoin) : d'abord ce qui se garde (grains, légumineuses, dattes : 2 % de perte par tour ; fruits, légumes et tubercules : 10 % ; viande et poisson : 20 %). En famine, la ville y puise d'abord ce qui se garde le moins.",
            12.5,
            charte::MUET,
        ));
    });
}

/// Panneau « Types de nourriture » : ce que la ville mange de chaque type (le seuil de 15 % porte
/// sur ce qu'elle mange, pas sur ce qu'elle produit), et l'effet de l'équilibre.
fn types_de_nourriture(g: &mut ChildSpawnerCommands, city: &City) {
    let c = &city.state;
    let par_type = city.travail.par_type;
    let besoin = c.besoin_nourriture();
    let mange = besoin.min(par_type.iter().sum::<f64>()).max(1e-6);
    let comptes = intendant::types_comptes(par_type, besoin);
    let couleurs = [Grandeur::Nourriture.couleur(), Grandeur::Production.couleur(), Grandeur::Culture.couleur()];
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, &format!("Types de nourriture : {comptes} sur 3"));
        // Barre de ce qui est mangé, en trois couleurs, avec le repère de 15 %
        p.spawn(Node { height: Val::Px(14.0), border_radius: BorderRadius::all(Val::Px(4.0)), overflow: Overflow::clip(), ..default() }).with_children(|b| {
            let somme: f64 = par_type.iter().sum::<f64>().max(1e-6);
            for (k, n) in par_type.iter().enumerate() {
                b.spawn((Node { width: Val::Percent((n / somme * 100.0) as f32), height: Val::Percent(100.0), ..default() }, BackgroundColor(couleurs[k])));
            }
        });
        for (k, t) in intendant::TypeNourriture::TOUS.iter().enumerate() {
            let part = (par_type[k] / mange).min(1.0);
            let compte = par_type[k] > 1e-9 && par_type[k] >= intendant::SEUIL_TYPE * mange - 1e-9;
            let etat = if compte { "compte".to_string() } else { format!("il en faut {} % : un {} y suffirait", (intendant::SEUIL_TYPE * 100.0) as u32, ["champ", "pâturage ou des légumineuses", "maraîchage"][k]) };
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(4.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn((Node { width: Val::Px(12.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(couleurs[k])));
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(capitale(t.nom()), 13.5, charte::TEXTE)]));
                l.spawn(texte(format!("{} · couvrirait {} % des repas", rules::decimal(par_type[k]), (part * 100.0).round()), 13.5, charte::TEXTE));
                l.spawn((Node { width: Val::Px(250.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(etat, 12.5, if compte { charte::OK } else { charte::MUET })]));
            });
            // Sortes du type : celles qui comptent (5 % des repas) d'abord, les autres en gris
            let mut sortes: Vec<(intendant::Sorte, f64)> = intendant::Sorte::TOUTES.iter().filter(|s| s.type_() == k).map(|&s| (s, city.travail.par_sorte[s.index()])).filter(|(_, n)| *n > 0.005).collect();
            sortes.sort_by(|a, b| b.1.total_cmp(&a.1));
            if !sortes.is_empty() {
                p.spawn(Node { column_gap: Val::Px(10.0), flex_wrap: FlexWrap::Wrap, padding: UiRect::left(Val::Px(20.0)), ..default() }).with_children(|l| {
                    for (s, n) in sortes {
                        let compte = n >= intendant::SEUIL_SORTE * mange - 1e-9;
                        l.spawn(texte(format!("{} {}", s.nom(), rules::decimal(n)), 12.5, if compte { charte::DOUX } else { charte::MUET }));
                    }
                });
            }
        }
        let en_plus = intendant::sortes_en_plus(&city.travail.par_sorte, besoin);
        let bonus = (intendant::BIEN_ETRE_PAR_SORTE * f64::from(en_plus)).min(intendant::BIEN_ETRE_DIVERSITE_MAX);
        p.spawn(texte(format!("Diversité des repas : {en_plus} sorte{} en plus de la première de chaque type → bien-être +{bonus:.0} (au plus +{:.0}). Une sorte compte à partir de 5 % des repas.", if en_plus > 1 { "s" } else { "" }, intendant::BIEN_ETRE_DIVERSITE_MAX), 12.5, if en_plus > 0 { charte::OK } else { charte::MUET }));
        let effet = match c.types_nourriture {
            0 | 1 => "Repas à un type : pas de bonus. Avec deux types, la limite de la ville monte de 10 % et les famines perdent un quart ; avec trois, 20 % et la moitié.".to_string(),
            n => format!("Repas à {n} types : la limite de la ville × {} (1 point de nourriture nourrit {} habitants), pertes de famine × {}.", rules::decimal(rules::facteur_equilibre(n)), rules::milliers((rules::HABITANTS_PAR_NOURRITURE * rules::facteur_equilibre(n)).round() as u64), rules::decimal(rules::facteur_famine(n))),
        };
        p.spawn(texte(effet, 12.5, charte::MUET));
        p.spawn(texte("Un type compte s'il peut couvrir 15 % des repas de la ville (ce qu'elle mange, pas ce qu'elle produit). L'intendant cherche d'abord à nourrir, puis l'équilibre.", 12.5, charte::MUET));
    });
}

/// Réglage de l'intendant pour une grandeur, avec l'effet de chaque choix sur toute la ville.
fn reglages_intendant(g: &mut ChildSpawnerCommands, gr: Grandeur, city: &City, reglages: &[(Reglage, Rendements, i64)]) {
    let cons = &city.state.consignes;
    let actuel = match gr {
        Grandeur::Nourriture => Reglage::Objectif(cons.objectif),
        Grandeur::Production => Reglage::Priorite(1, cons.priorites[1]),
        _ => Reglage::Priorite(2, cons.priorites[2]),
    };
    let base = city.travail.rendements;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        let (titre_r, explication) = if gr == Grandeur::Nourriture {
            ("Objectif de l'intendant", "La nourriture limite la croissance : l'intendant vise d'abord cet objectif, puis répartit le reste des travailleurs selon les priorités de la production et des deniers.")
        } else {
            ("Priorité de l'intendant", "Après l'objectif de nourriture. Grandeurs au même niveau : même poids.")
        };
        p.spawn(texte(titre_r, 15.0, charte::TEXTE));
        p.spawn(texte(explication, 12.5, charte::MUET));
        for (o, r, variation) in reglages {
            let choisi = *o == actuel;
            let (nom, detail) = match o {
                Reglage::Objectif(x) => (x.nom().to_string(), format!("{} × le besoin{}", format!("{}", x.cible()).replace('.', ","), if *x == intendant::ObjectifNourriture::Excedent { " · excédent au grenier, le reste perdu tant que le commerce manque" } else { "" })),
                Reglage::Priorite(_, x) => (capitale(x.nom()), String::new()),
            };
            let effet = if choisi { "réglage actuel".to_string() } else { format!("effet : {}", ecarts(&(*r - base))) };
            let croissance = if *variation >= 0 { format!("+{} habitants par tour", rules::milliers(*variation as u64)) } else { format!("−{} habitants par tour", rules::milliers(variation.unsigned_abs())) };
            p.spawn((
                ChoixReglage(*o),
                UiPointer::default(),
                Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
                BackgroundColor(Color::NONE),
                BorderColor::all(if choisi { charte::SELECTION } else { charte::TRAIT }),
            ))
            .with_children(|l| {
                l.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                    n.spawn(texte(nom, 13.5, charte::TEXTE));
                    if !detail.is_empty() {
                        n.spawn(texte(detail, 12.0, charte::MUET));
                    }
                });
                l.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, ..default() }).with_children(|n| {
                    n.spawn(texte(effet, 12.5, if choisi { charte::MUET } else { charte::TEXTE }));
                    n.spawn(texte(croissance, 12.0, charte::MUET));
                });
            });
        }
    });
}

/// Effectif d'une case : forcé ou non ; proposer un autre effectif, voir son effet, confirmer.
fn effectif(p: &mut ChildSpawnerCommands, c: &CaseVue, f: &VueForcage) {
    if let Some(n) = f.actuel {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(format!("Effectif forcé à {} : l'intendant ne le change plus.", rules::milliers(n.into())), 12.5, charte::ALERTE)]));
            l.spawn((Forcage::Rendre(c.hex), bouton(Color::NONE), children![texte("rendre à l'intendant", 12.5, charte::DOUX)]));
        });
    }
    if c.prioritaire {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("Case prioritaire : l'intendant la remplit avant les autres.", 12.5, charte::ALERTE)]));
            l.spawn((Forcage::Rendre(c.hex), bouton(Color::NONE), children![texte("rendre à l'intendant", 12.5, charte::DOUX)]));
        });
    }
    // Verrous : figer l'effectif du moment, ou faire passer la case avant les autres
    p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|l| {
        l.spawn(texte("Verrou :", 13.0, charte::TEXTE));
        let fige = f.actuel == Some(c.travailleurs);
        let mut b = l.spawn((bouton(if fige { charte::ACTION_FOND } else { Color::NONE }), children![texte(format!("Figer à {}", rules::milliers(c.travailleurs.into())), 13.0, charte::TEXTE)]));
        if !fige {
            b.insert(Forcage::Figer(c.hex, c.travailleurs));
        }
        let mut b = l.spawn((bouton(if c.prioritaire { charte::ACTION_FOND } else { Color::NONE }), children![texte("Prioriser", 13.0, charte::TEXTE)]));
        if !c.prioritaire {
            b.insert(Forcage::Prioriser(c.hex));
        }
        l.spawn(texte("figer : l'intendant n'y touche plus · prioriser : il la remplit d'abord", 11.5, charte::MUET));
    });
    let depart = f.propose.map_or(c.travailleurs, |(n, _)| n);
    p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|l| {
        l.spawn(texte("Forcer l'effectif :", 13.0, charte::TEXTE));
        // (boutons toujours à la même place ; éteints quand ils ne changeraient rien : à zéro, case pleine)
        let pas = |l: &mut ChildSpawnerCommands, t: &str, actif: bool, n: u32| {
            let mut b = l.spawn((bouton(Color::NONE), children![texte(t, 13.0, if actif { charte::TEXTE } else { charte::MUET })]));
            if actif {
                b.insert(Forcage::Proposer(c.hex, n));
            }
        };
        pas(l, "−100", depart > 0, depart.saturating_sub(100));
        l.spawn((Node { min_width: Val::Px(110.0), justify_content: JustifyContent::Center, ..default() }, children![texte(format!("{} / {}", rules::milliers(depart.into()), rules::milliers(c.capacite.into())), 13.5, charte::TEXTE)]));
        pas(l, "+100", depart < c.capacite, (depart + 100).min(c.capacite));
    });
    if let Some((n, d)) = f.propose {
        p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(9.0), Val::Px(7.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() }, BorderColor::all(charte::TRAIT2))).with_children(|a| {
            a.spawn(texte(format!("Aperçu : {} travailleurs ici ; pour toute la ville : {} (les autres cases s'ajustent)", rules::milliers(n.into()), ecarts(&d)), 13.0, charte::TEXTE));
            a.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|b| {
                b.spawn((Forcage::Confirmer(c.hex, n), bouton(charte::ACTION_FOND), children![texte("Confirmer", 13.0, charte::TEXTE)]));
                b.spawn((Forcage::Annuler, bouton(Color::NONE), children![texte("Annuler", 13.0, charte::DOUX)]));
            });
        });
    }
}

/// File de construction (avancer, retirer) et projet suivi quand elle est vide.
/// Fond d'une carte de la file : claire au repos, plus claire choisie, plus encore glissée.
fn fond_ligne(choisie: bool, glissee: bool) -> Color {
    if glissee {
        Color::srgb(0.20, 0.24, 0.33)
    } else if choisie {
        charte::SURFACE2
    } else {
        charte::SURFACE
    }
}

/// Bord d'une carte de la file : fin tout autour, liseré à gauche (orange si choisie ou glissée).
fn bord_ligne(choisie: bool, glissee: bool) -> BorderColor {
    let lisere = if choisie || glissee { Grandeur::Production.couleur() } else { charte::TRAIT2 };
    BorderColor { left: lisere, ..BorderColor::all(if glissee { Grandeur::Production.couleur() } else { charte::TRAIT2 }) }
}

/// Ce qu'apporte le sanctuaire, en une ligne.
fn effet_sanctuaire(megalithe: bool) -> String {
    let pierre = if megalithe { format!("mégalithe : {:.0} de pierre dure, moins cher", rules::SANCTUAIRE_PIERRE_DURE) } else { "en pierre locale (un mégalithe avec 2 de pierre dure en stock, moins cher)".to_string() };
    format!("culture +{:.0} par tour (lieu de culte) · {pierre}", rules::CULTURE_SANCTUAIRE)
}

/// Onglet Bâtiments : les bâtiments de la ville et ce qu'ils lui apportent.
fn batiments_de_la_ville(g: &mut ChildSpawnerCommands, city: &City, images: &Images) {
    let c = &city.state;
    g.spawn(texte("BÂTIMENTS DE LA VILLE", 11.5, charte::MUET));
    let mut bati = Vec::new();
    if c.puits {
        bati.push((Element::Puits, "Puits".to_string(), format!("salubrité +{:.0} (eau plus propre)", rules::SALUBRITE_PUITS)));
    }
    if c.grenier {
        bati.push((Element::Grenier, "Grenier".to_string(), format!("{} de nourriture gardée sur {}", rules::decimal(c.stock_total()), rules::decimal(c.capacite_grenier()))));
    }
    if let Some(m) = c.sanctuaire {
        bati.push((Element::Sanctuaire, if m { "Sanctuaire mégalithique".to_string() } else { "Sanctuaire".to_string() }, format!("culture +{:.0} par tour", rules::CULTURE_SANCTUAIRE)));
    }
    if bati.is_empty() {
        g.spawn(texte("Aucun bâtiment : Construire (B), puis Bâtiments (puits, grenier, sanctuaire).", 12.5, charte::MUET));
    }
    for (e, nom, effet) in bati {
        let (image, couleur) = icone_element(images, e);
        g.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), ..default() }).with_children(|l| {
            l.spawn((
                Node { width: Val::Px(28.0), height: Val::Px(28.0), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.5)), border_radius: BorderRadius::all(Val::Px(14.0)), ..default() },
                BackgroundColor(couleur.with_alpha(0.15)),
                BorderColor::all(couleur),
                children![icone(image, 16.0, couleur)],
            ));
            l.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                n.spawn(texte(nom, 13.5, charte::TEXTE));
                n.spawn(texte(effet, 12.0, charte::DOUX));
            });
        });
    }
}

/// Ce qu'apporte le grenier, en une ligne.
fn effet_grenier(c: &rules::CityState) -> String {
    format!(
        "garde jusqu'à {:.0} tours de besoin ({} de nourriture) contre les famines ; il se remplit de l'excédent que la croissance n'utilise pas",
        rules::GRENIER_TOURS,
        rules::decimal(rules::GRENIER_TOURS * c.besoin_nourriture())
    )
}

/// Ce qu'apporte un élément de la file, en une ligne, et où il se construit.
fn benefice(vl: &Ville, city: &City, cases: &[CaseVue], objet: &rules::Production, colons_avant: usize) -> (Option<String>, String) {
    let map = vl.map;
    let lieu = case_de(objet).map(|h| cases.iter().find(|c| c.hex == h).map_or_else(|| map.describe(h).to_lowercase(), |c| c.nom.to_lowercase()));
    let texte = match *objet {
        // L'élément jugé est lui-même en file : la case est jugée sans lui.
        rules::Production::Amenagement { hex, kind, .. } => match cases.iter().find(|c| c.hex == hex).map(|c| gain_amenagement(vl, &CaseVue { en_file: false, ..c.clone() }, kind)) {
            Some(Ok(g)) => format!("dans {TOURS_EFFET} tours : {} · valeur {}", ecarts(&g.dans), signe(evaluation(&g, &Bareme::de(vl)).valeur)),
            Some(Err(e)) => format!("plus possible : {e}"),
            None => "case hors du territoire".into(),
        },
        rules::Production::Bridge { .. } => "franchir le cours d'eau sans détour ni surcoût".into(),
        rules::Production::Puits => {
            let s = city.state.salubrite();
            format!("salubrité {:.0} → {:.0} (eau plus propre ; plus tard, de l'eau pour grandir)", s, (s + rules::SALUBRITE_PUITS).min(100.0))
        }
        rules::Production::Grenier => effet_grenier(&city.state),
        rules::Production::Sanctuaire { megalithe } => effet_sanctuaire(megalithe),
        rules::Production::Unit(k) => {
            let st = k.stats();
            if st.ranged_attack > 0 {
                format!("unité : tir {} (portée {}), défense {}, {} PM", st.ranged_attack, st.range, st.melee_defense, st.moves)
            } else if k.is_military() {
                format!("unité : attaque {}, défense {}, {} PM", st.melee_attack, st.melee_defense, st.moves)
            } else if k == rules::UnitKind::Settler {
                // (les colons d'avant dans la file sont déjà partis)
                let avant = city.state.habitants.saturating_sub(colons_avant as u64 * rules::HABITANTS_COLON);
                let apres = rules::palier_de(avant.saturating_sub(rules::HABITANTS_COLON));
                format!("unité : {} PM · emmène {} habitants (palier {} → {})", st.moves, rules::milliers(rules::HABITANTS_COLON), rules::palier_de(avant), apres)
            } else {
                format!("unité : {} PM", st.moves)
            }
        }
    };
    (lieu, texte)
}

fn file_et_projet(g: &mut ChildSpawnerCommands, city: &City, catalogue: bool, vl: &Ville, cases: &[CaseVue], choisie: Option<usize>) {
    let c = &city.state;
    let prod = city.travail.rendements.production;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
            t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("File de construction", 15.0, charte::TEXTE)]));
            if !catalogue {
                t.spawn((BoutonConstruire, bouton(charte::ACTION_FOND), children![texte("Construire (B)", 13.0, charte::TEXTE)]));
            }
        });
        if c.file.is_empty() {
            // (ce qui se passe alors : la ligne du projet, plus bas)
            p.spawn(texte("Vide.", 13.0, charte::MUET));
        } else if c.file.len() > 1 {
            p.spawn(texte("Glisser une ligne pour changer l'ordre · survol ou clic : sa case sur la carte", 12.0, charte::MUET));
        }
        for (i, k) in c.file.iter().enumerate() {
            let colons_avant = c.file[..i].iter().filter(|x| x.objet == rules::Production::Unit(rules::UnitKind::Settler)).count();
            let (lieu, apport) = benefice(vl, city, cases, &k.objet, colons_avant);
            let choisi = choisie == Some(i);
            p.spawn((
                LigneFile(i),
                UiPointer::default(),
                BackgroundColor(fond_ligne(choisi, false)),
                Node { column_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)), border: UiRect { left: Val::Px(3.0), top: Val::Px(1.0), right: Val::Px(1.0), bottom: Val::Px(1.0) }, border_radius: BorderRadius::all(Val::Px(6.0)), align_items: AlignItems::Center, ..default() },
                bord_ligne(choisi, false),
            ))
            .with_children(|l| {
                l.spawn(texte(format!("{}.", i + 1), 13.0, charte::MUET));
                l.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                    n.spawn(texte(format!("{}{}", capitale(k.objet.name()), lieu.map_or(String::new(), |l| format!(" · sur {l}"))), 13.5, charte::TEXTE));
                    n.spawn(texte(apport, 12.0, charte::AMENAGEMENT));
                    let tours = c.tours_pour(i, prod).map_or("jamais au rythme actuel".to_string(), |t| if t == 0 { "prête".into() } else { format!("prête dans {t} tour(s)") });
                    n.spawn(texte(format!("{} / {} de production · {tours}", k.investi.floor(), k.objet.cost()), 12.0, charte::MUET));
                });
                l.spawn(barre((k.investi / k.objet.cost().max(1) as f64) as f32, 70.0, Grandeur::Production.couleur()));
                if i > 0 {
                    l.spawn((ActionFile::Monter(i), bouton(Color::NONE), children![texte("avancer", 12.5, charte::DOUX)]));
                }
                // (libellé fixe : le bouton ne bouge pas ; la production investie se lit sur la ligne)
                l.spawn((ActionFile::Retirer(i), bouton(Color::NONE), children![texte("retirer", 12.5, charte::DOUX)]));
            });
        }
        let projet = match c.projet {
            Some(pj) => format!("Quand la file est vide : projet « {} » ({}){}", pj.nom(), pj.effet(), gain_projet(city, pj).map_or(String::new(), |g| format!(" · {g}"))),
            None => "Quand la file est vide : aucun projet (il faut construire ou choisir un projet pour finir le tour)".into(),
        };
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(projet, 12.5, charte::DOUX)]));
            if c.projet.is_some() {
                l.spawn((ArreterProjet, bouton(Color::NONE), children![texte("arrêter le projet", 12.5, charte::DOUX)]));
            }
        });
    });
}

/// Catalogue : conseils de l'intendant, aménagements, unités, projets de ville, ponts.
fn catalogue(l: &mut ChildSpawnerCommands, vl: &Ville, city: &City, cases: &[CaseVue], placement: Option<Amenagement>, groupe_vu: GroupeCatalogue, images: &Images) {
    let prod = city.travail.rendements.production;
    // Onglets du catalogue : un seul groupe à la fois (tout tient à l'écran)
    l.spawn(Node { column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(4.0)), ..default() }).with_children(|o| {
        for (gr, t) in [(GroupeCatalogue::Amenagements, "Aménagements"), (GroupeCatalogue::Batiments, "Bâtiments"), (GroupeCatalogue::Projets, "Projets"), (GroupeCatalogue::Unites, "Unités")] {
            let actif = gr == groupe_vu;
            o.spawn((
                ChoixGroupe(gr),
                UiPointer::default(),
                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                BackgroundColor(if actif { charte::TEXTE } else { Color::NONE }),
                BorderColor::all(charte::TRAIT2),
                children![texte(t, 13.0, if actif { charte::FOND } else { charte::DOUX })],
            ));
        }
    });
    let groupe = |l: &mut ChildSpawnerCommands, t: &str| {
        l.spawn((Node { margin: UiRect::top(Val::Px(6.0)), ..default() }, children![texte(t, 13.5, charte::DOUX)]));
    };
    let item = |l: &mut ChildSpawnerCommands, e: Element, titre: String, detail: String, actif: bool, choisi: bool| {
        let (image, couleur) = icone_element(images, e);
        let mut n = l.spawn((
            Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(if choisi { charte::SELECTION } else { Color::NONE }),
        ));
        if actif {
            n.insert((ChoixCatalogue(e), UiPointer::default()));
        }
        n.with_children(|i| {
            // Icône dans un rond : vert pour un aménagement (sur une case), sinon la couleur de ce qu'il donne
            i.spawn((
                Node { width: Val::Px(30.0), height: Val::Px(30.0), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.5)), border_radius: BorderRadius::all(Val::Px(15.0)), ..default() },
                BackgroundColor(couleur.with_alpha(0.15)),
                BorderColor::all(if actif { couleur } else { charte::MUET }),
                children![icone(image, 18.0, if actif { couleur } else { charte::MUET })],
            ));
            i.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
                t.spawn(texte(titre, 13.5, if actif { charte::TEXTE } else { charte::MUET }));
                t.spawn(texte(detail, 12.0, charte::MUET));
            });
        });
    };
    // Aménagements, avec leur meilleure case
    if groupe_vu == GroupeCatalogue::Amenagements {
    groupe(l, "Aménagements de production : un par case, l'intendant y met ensuite les travailleurs");
    for a in Amenagement::ALL {
        let meilleure = conseil(vl, cases, a, &Bareme::de(vl)).and_then(|h| cases.iter().find(|c| c.hex == h));
        let une_case = cases.iter().any(|c| possible(vl.map, c, a).is_ok());
        let detail = match meilleure.and_then(|c| gain_amenagement(vl, c, a).ok().map(|g| (c, g))) {
            Some((c, g)) => format!("conseil : {} · valeur {} (dans {TOURS_EFFET} tours : {}) · {} production", c.nom.to_lowercase(), signe(evaluation(&g, &Bareme::de(vl)).valeur), ecarts(&g.dans), g.cout),
            None if une_case => format!("aucun aménagement utile maintenant : aucune case n'y gagne dans {TOURS_EFFET} tours"),
            None => "aucune case possible dans la ville".into(),
        };
        let titre = if placement == Some(a) { format!("{} — en placement : choisissez une case sur la carte", capitale(a.nom())) } else { capitale(a.nom()) };
        item(l, Element::Amenagement(a), titre, detail, une_case, placement == Some(a));
    }
    }
    // Bâtiments de la ville et ponts
    if groupe_vu == GroupeCatalogue::Batiments {
    groupe(l, "Bâtiments de la ville : sur le centre, sans travailleurs");
    let c = &city.state;
    let en_file = c.file.iter().any(|k| k.objet == rules::Production::Puits);
    let detail = if c.puits {
        "déjà creusé".to_string()
    } else if en_file {
        "déjà dans la file".to_string()
    } else {
        let s = c.salubrite();
        let tours = rules::tours_restants(rules::PUITS_COUT, 0.0, prod).map_or("—".to_string(), |t| format!("{t} tour(s) seul"));
        format!("salubrité {s:.0} → {:.0} (eau plus propre ; plus tard, de l'eau pour grandir) · {} production · {tours}", (s + rules::SALUBRITE_PUITS).min(100.0), rules::PUITS_COUT)
    };
    item(l, Element::Puits, "Puits".into(), detail, !c.puits && !en_file, false);
    let en_file = c.file.iter().any(|k| k.objet == rules::Production::Grenier);
    let detail = if c.grenier {
        "déjà bâti".to_string()
    } else if en_file {
        "déjà dans la file".to_string()
    } else {
        let tours = rules::tours_restants(rules::GRENIER_COUT, 0.0, prod).map_or("—".to_string(), |t| format!("{t} tour(s) seul"));
        format!("{} · {} production · {tours}", effet_grenier(c), rules::GRENIER_COUT)
    };
    item(l, Element::Grenier, "Grenier".into(), detail, !c.grenier && !en_file, false);
    let en_file = c.file.iter().any(|k| matches!(k.objet, rules::Production::Sanctuaire { .. }));
    let objet = c.sanctuaire_possible();
    let detail = if let Some(m) = c.sanctuaire {
        if m { "déjà bâti (mégalithe)".to_string() } else { "déjà bâti".to_string() }
    } else if en_file {
        "déjà dans la file".to_string()
    } else {
        let tours = rules::tours_restants(objet.cost(), 0.0, prod).map_or("—".to_string(), |t| format!("{t} tour(s) seul"));
        let megalithe = matches!(objet, rules::Production::Sanctuaire { megalithe: true });
        format!("{} · {} production · {tours}", effet_sanctuaire(megalithe), objet.cost())
    };
    item(l, Element::Sanctuaire, objet.name().into(), detail, c.sanctuaire.is_none() && !en_file, false);
    groupe(l, "Ponts : leur emplacement se choisit sur la carte du monde (l'écran se ferme)");
    l.spawn((crate::bridge_build::BridgeButton, bouton(charte::SURFACE), children![texte("Pont…", 13.5, charte::TEXTE)]));
    }
    // Projets de ville
    if groupe_vu == GroupeCatalogue::Projets {
    groupe(l, "Projets de ville : sans fin, ils reçoivent la production quand la file est vide");
    for pj in crate::rules::Projet::TOUS {
        let detail = match (pj.disponible(), gain_projet(city, pj)) {
            (true, Some(g)) => format!("{} · {g}", pj.effet()),
            (true, None) => pj.effet().to_string(),
            (false, _) => format!("{} : pas encore dans le jeu", pj.effet()),
        };
        item(l, Element::Projet(pj), pj.nom().into(), detail, pj.disponible(), city.state.projet == Some(pj));
    }
    }
    // Unités
    if groupe_vu == GroupeCatalogue::Unites {
    groupe(l, "Unités : ajoutées à la fin de la file");
    for k in crate::rules::UnitKind::ALL {
        let tours = rules::tours_restants(k.cost(), 0.0, prod).map_or("—".to_string(), |t| format!("{t} tour(s) seule"));
        let colon = if k == crate::rules::UnitKind::Settler { format!(" · emmène {} habitants", rules::milliers(rules::HABITANTS_COLON)) } else { String::new() };
        item(l, Element::Unite(k), k.name().into(), format!("{} production · {tours}{colon}", k.cost()), true, false);
    }
    }
}

/// Icône et couleur d'un élément du catalogue : l'aménagement lui-même, ou ce que donne un bâtiment,
/// un projet, une unité.
fn icone_element(images: &Images, e: Element) -> (Handle<Image>, Color) {
    use crate::rules::{Projet, UnitKind};
    match e {
        Element::Amenagement(a) => (images.amenagement(a.into()), charte::AMENAGEMENT),
        Element::Puits => (images.grandeur(Grandeur::Eau), Grandeur::Eau.couleur()),
        Element::Grenier => (images.grandeur(Grandeur::Nourriture), Grandeur::Nourriture.couleur()),
        Element::Sanctuaire => (images.grandeur(Grandeur::Culture), Grandeur::Culture.couleur()),
        Element::Projet(Projet::Peupler) => (images.grandeur(Grandeur::Habitants), Grandeur::Habitants.couleur()),
        Element::Projet(Projet::GrandsJeux) => (images.grandeur(Grandeur::BienEtre), Grandeur::BienEtre.couleur()),
        Element::Projet(Projet::Recherche) => (images.grandeur(Grandeur::Recherche), Grandeur::Recherche.couleur()),
        Element::Projet(Projet::Culture) => (images.grandeur(Grandeur::Culture), Grandeur::Culture.couleur()),
        Element::Unite(UnitKind::Settler) => (images.grandeur(Grandeur::Habitants), Grandeur::Habitants.couleur()),
        Element::Unite(_) => (images.grandeur(Grandeur::Defense), Grandeur::Defense.couleur()),
    }
}

/// Aménagement en cours de placement (mode de la carte).
fn placement_de(mode: ModeCarte) -> Option<Amenagement> {
    match mode {
        ModeCarte::Placement(a, _) => Some(a),
        _ => None,
    }
}

/// Le gisement d'une case face à un aménagement : exploité par lui, ou ce qu'on y perd.
fn conflit_gisement(c: &CaseVue, a: Option<Amenagement>) -> Option<(String, Color)> {
    let r = c.gisement?;
    Some(match a {
        Some(Amenagement::Exploitation) => (format!("◆ {} : {} par tour", r.nom(), quantite(1.0)), charte::OK),
        _ if c.amenagement == Some(Amenagement::Exploitation) => (format!("◆ {} : {} perdue", r.nom(), r.exploitation()), charte::GRAVE),
        _ => (format!("◆ {} : plus de {}", r.nom(), r.exploitation()), charte::ALERTE),
    })
}

/// Effet net d'un aménagement sur la croissance (habitants par tour, dans 10 tours) : la ville
/// simulée avec lui ; l'exploitation d'un gisement compte aussi la ressource qu'elle donne.
fn net_habitants(city: &City, c: &CaseVue, a: Amenagement, g: &GainAmenagement) -> i64 {
    let s = &city.state;
    let y = city.travail.rendements;
    let mut avec = s.clone();
    if let (Amenagement::Exploitation, Some(r)) = (a, c.gisement)
        && s.demande(r) > 0.0
    {
        avec.couverture.resize(crate::gisements::N_RESSOURCES, 0.0);
        avec.couverture[r.index()] = (1.0 / s.demande(r)).min(1.0).max(s.couverture(r));
    }
    avec.variation(y + g.dans) - s.variation(y)
}

/// Fiche du placement d'un aménagement sur une case.
fn placement(g: &mut ChildSpawnerCommands, vl: &Ville, city: &City, cases: &[CaseVue], a: Amenagement, c: &CaseVue, conseil_seul: bool) {
    let Ok(gain) = gain_amenagement(vl, c, a) else { return };
    let bareme = Bareme::de(vl);
    let e = evaluation(&gain, &bareme);
    let prod = city.travail.rendements.production;
    // Pourquoi cette case : la comparer à celle qui donne le plus de la grandeur visée
    let g_vise = vise(a);
    let pourquoi = cases
        .iter()
        .filter(|x| x.hex != c.hex)
        .filter_map(|x| gain_amenagement(vl, x, a).ok().map(|gx| (x, gx)))
        .max_by(|p, q| valeur(&p.1.plein, g_vise).total_cmp(&valeur(&q.1.plein, g_vise)))
        .filter(|(_, gx)| valeur(&gx.plein, g_vise) > valeur(&gain.plein, g_vise) + 0.05)
        .map(|(x, gx)| {
            let ex = evaluation(&gx, &bareme);
            if ex.valeur > e.valeur + 0.05 {
                format!("Attention : {} vaut plus ({} contre {} ici).", x.nom, signe(ex.valeur), signe(e.valeur))
            } else {
                let raison = if gx.defriche && !gain.defriche { "elle demande un défrichement" } else if ex.risque != Risque::Aucun { "elle menace de famine" } else { "elle fait perdre ailleurs" };
                format!("Pourquoi cette case ? {} donne plus de {} au maximum ({}), mais sa valeur est {} contre {} ici : {raison}.", x.nom, g_vise.nom().to_lowercase(), ecarts(&gx.plein), signe(ex.valeur), signe(e.valeur))
            }
        });
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU), BorderColor::all(charte::AMENAGEMENT))).insert(Node { border: UiRect::all(Val::Px(1.0)), ..panneau() }).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|t| {
            t.spawn(texte(format!("① Aménagement : {}", capitale(a.nom())), 14.0, charte::TEXTE));
            t.spawn(texte("›", 14.0, charte::MUET));
            t.spawn(texte(format!("② Case : {}", c.nom), 14.0, charte::TEXTE));
            if conseil_seul {
                t.spawn(texte("(conseil de l'intendant)", 13.0, CONSEIL));
            }
            t.spawn(texte("›", 14.0, charte::MUET));
            t.spawn(texte("③ Mettre en file", 14.0, charte::DOUX));
        });
        // Ce qu'on gagne et ce qu'on perd, dans 10 tours, en deux colonnes
        p.spawn(texte(format!("CE QUE ÇA CHANGE, DANS {TOURS_EFFET} TOURS"), 11.5, charte::MUET));
        let mut gagne: Vec<(String, Color)> = Vec::new();
        let mut perdu: Vec<(String, Color)> = Vec::new();
        for (gr, nom) in [(Grandeur::Nourriture, "nourriture"), (Grandeur::Production, "production"), (Grandeur::Deniers, "deniers")] {
            let v = valeur(&gain.dans, gr);
            if v >= 0.05 {
                gagne.push((format!("+{} {nom} par tour", rules::decimal(v)), gr.couleur()));
            } else if v <= -0.05 {
                perdu.push((format!("−{} {nom} par tour", rules::decimal(-v)), charte::GRAVE));
            }
        }
        if gain.capacite > c.capacite {
            gagne.push((format!("+{} emplois sur la case", rules::milliers((gain.capacite - c.capacite).into())), charte::OK));
        }
        if gain.defriche {
            perdu.push((format!("la forêt (bois), défrichée : +{} de coût", intendant::COUT_DEFRICHEMENT), charte::ALERTE));
        }
        if let Some(actuel) = c.amenagement {
            perdu.push((format!("l'aménagement actuel : {}", actuel.nom().to_lowercase()), charte::ALERTE));
        }
        if let Some((t, couleur)) = conflit_gisement(c, Some(a)) {
            if couleur == charte::OK { gagne.push((t, couleur)) } else { perdu.push((t, couleur)) }
        }
        p.spawn(Node { column_gap: Val::Px(12.0), ..default() }).with_children(|r| {
            for (titre_, lignes, couleur) in [("Gagné", &gagne, charte::OK), ("Perdu", &perdu, charte::GRAVE)] {
                r.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), row_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(6.0)), border: UiRect::left(Val::Px(2.0)), ..default() }).insert(BorderColor::all(couleur)).with_children(|col| {
                    col.spawn(texte(titre_, 12.5, couleur));
                    if lignes.is_empty() {
                        col.spawn(texte("rien", 12.5, charte::MUET));
                    }
                    for (t, k) in lignes.iter() {
                        col.spawn(texte(t.clone(), 12.5, *k));
                    }
                });
            }
        });
        let net = net_habitants(city, c, a, &gain);
        ligne_valeur(p, "Habitants (croissance)", format!("{}{} par tour", if net >= 0 { "+" } else { "−" }, rules::milliers(net.unsigned_abs())), if net < 0 { charte::GRAVE } else if net > 0 { charte::OK } else { charte::DOUX });
        ligne_valeur(p, "Valeur pour la ville", format!("{}{}", signe(e.valeur), e.rentabilite.map_or(String::new(), |r| format!(" · rentabilisé en {} tours", r.ceil()))), couleur_signe(e.valeur));
        if let Some(t) = &pourquoi {
            p.spawn(texte(t.clone(), 12.5, if t.starts_with("Attention") { charte::ALERTE } else { CONSEIL }));
        }
        match e.risque {
            Risque::Famine(k) => ligne_valeur(p, "Risque de famine", format!("la nourriture tomberait à {} × le besoin", rules::decimal(k)), charte::GRAVE),
            Risque::BordDeFamine(k) => ligne_valeur(p, "Bord de famine", format!("{} × le besoin après", rules::decimal(k)), charte::GRAVE),
            Risque::Aucun => {}
        }
        ligne_valeur(p, "Tout de suite (l'intendant répartit les travailleurs)", ecarts(&gain.maintenant), charte::DOUX);
        if gain.dans.nourriture.abs() >= 0.05 {
            // (la nourriture au-delà de l'objectif de l'intendant ne fait plus grandir la ville)
            let pleine = POIDS_FREIN * gain.dans.nourriture;
            let note = if gain.dans.nourriture > 0.0 && e.nourriture < pleine - 0.05 { " : au-delà de l'objectif de l'intendant, elle ne fait plus grandir la ville" } else { "" };
            ligne_valeur(p, "Part de la nourriture dans la valeur", format!("{}{note}", signe(e.nourriture)), charte::DOUX);
        }
        ligne_valeur(p, &format!("Maximum de la ville (case pleine, {} travailleurs)", rules::milliers(gain.capacite.into())), format!("{} (pas tout de suite)", ecarts(&gain.plein)), charte::DOUX);
        ligne_valeur(p, "Capacité de la case", format!("{} → {} travailleurs", rules::milliers(c.capacite.into()), rules::milliers(gain.capacite.into())), charte::TEXTE);
        let seul = rules::tours_restants(gain.cout, 0.0, prod).map_or("—".into(), |t| format!("{t} tour(s) seul"));
        let file: i32 = city.state.file.iter().map(|k| (k.objet.cost() as f64 - k.investi).max(0.0).ceil() as i32).sum();
        let apres = rules::tours_restants(gain.cout + file, 0.0, prod).map_or("—".into(), |t| format!("{t} après la file"));
        ligne_valeur(p, "Coût et durée", format!("{} de production · {seul}, {apres}", gain.cout), charte::TEXTE);
        p.spawn(texte(regle_valeur(&bareme), 12.0, charte::MUET));
        p.spawn((MettreEnFile, bouton(charte::ACTION_FOND), children![texte(format!("Mettre en file · {}", c.nom), 14.0, charte::TEXTE)]));
    });
}

/// Glisser-déposer dans la file : appuyer sur une ligne et la glisser. La ligne suit le pointeur
/// (soulevée, contour blanc), les autres s'écartent en douceur pour lui faire place ; relâcher la
/// pose. Un clic sans glisser choisit la ligne (sa case est cerclée sur la carte).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn glisser_file(
    mut commands: Commands,
    mut ecran: ResMut<EcranVille>,
    (mouse, replay, time, settings): (Res<ButtonInput<MouseButton>>, Res<crate::feedback::TurnReplay>, Res<Time>, Res<crate::settings::Settings>),
    pointeurs: Query<(&PointerId, &PointerLocation)>,
    mut lignes: Query<(Entity, &LigneFile, &UiPointer, &ComputedNode, &UiGlobalTransform, &mut Node, &mut BorderColor, &mut BackgroundColor)>,
    mut cities: Query<&mut City>,
    mut glisse: Local<Option<Glisse>>,
) {
    let Some(v) = ecran.ville else {
        *glisse = None;
        return;
    };
    let y = pointeurs.iter().find(|(id, _)| **id == PointerId::Mouse).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position.y);
    if mouse.just_pressed(MouseButton::Left)
        && let Some(y) = y
        && let Some((entite, l, ..)) = lignes.iter().find(|(_, _, p, ..)| **p != UiPointer::None)
    {
        // Centres et hauteurs des lignes au départ (les décalages ne doivent pas les fausser)
        let mut lignes_depart: Vec<(usize, f32, f32)> = lignes.iter().map(|(_, l, _, n, t, ..)| (l.0, t.translation.y * n.inverse_scale_factor(), n.size().y * n.inverse_scale_factor())).collect();
        lignes_depart.sort_by_key(|(i, ..)| *i);
        *glisse = Some(Glisse { de: l.0, entite, y0: y, bouge: false, lignes: lignes_depart, decalages: Default::default() });
    }
    let Some(g) = glisse.as_mut() else { return };
    if !lignes.iter().any(|(e, ..)| e == g.entite) {
        // (l'écran s'est refait pendant le geste : on l'abandonne)
        *glisse = None;
        return;
    }
    if let Some(y) = y
        && (y - g.y0).abs() > 6.0
        && !g.bouge
    {
        g.bouge = true;
        // (la ligne soulevée passe au-dessus des autres)
        if let Some((e, ..)) = lignes.iter().find(|(_, l, ..)| l.0 == g.de) {
            commands.entity(e).insert(GlobalZIndex(96));
        }
    }
    // Rang d'arrivée : parmi les autres lignes, combien ont leur centre au-dessus du pointeur
    let autres: Vec<(usize, f32, f32)> = g.lignes.iter().copied().filter(|(i, ..)| *i != g.de).collect();
    let vers = y.map_or(g.de, |y| autres.iter().filter(|(_, c, _)| *c < y).count());
    let hauteur = g.lignes.iter().find(|(i, ..)| *i == g.de).map_or(40.0, |(_, _, h)| *h + 4.0);
    // Décalage voulu de chaque ligne : la glissée suit le pointeur ; les autres se poussent d'une ligne
    let instant = settings.animation == crate::settings::AnimationSpeed::Instant;
    let k = if instant { 1.0 } else { (time.delta_secs() * 14.0).min(1.0) };
    for (_, l, _, _, _, mut node, mut bord, mut fond) in &mut lignes {
        let voulu = if !g.bouge {
            0.0
        } else if l.0 == g.de {
            // (bornée : la ligne ne sort pas de la file, de la place de la première à celle de la dernière)
            let centre = g.lignes.iter().find(|(i, ..)| *i == g.de).map_or(0.0, |(_, c, _)| *c);
            let haut = g.lignes.first().map_or(centre, |(_, c, _)| *c) - centre;
            let bas = g.lignes.last().map_or(centre, |(_, c, _)| *c) - centre;
            y.map_or(0.0, |y| (y - g.y0).clamp(haut, bas))
        } else {
            let rang = autres.iter().position(|(i, ..)| *i == l.0).unwrap_or(0);
            let nouveau = if rang < vers { rang } else { rang + 1 };
            let ancien = if l.0 < g.de { rang } else { rang + 1 };
            (nouveau as f32 - ancien as f32) * hauteur
        };
        let d = g.decalages.entry(l.0).or_insert(0.0);
        // (la ligne glissée colle au pointeur ; les autres glissent en douceur)
        *d = if l.0 == g.de { voulu } else { *d + (voulu - *d) * k };
        let top = Val::Px(*d);
        if node.top != top {
            node.top = top;
        }
        let glissee = g.bouge && l.0 == g.de;
        let choisie = ecran.file_choisie == Some(l.0);
        let b = bord_ligne(choisie, glissee);
        if *bord != b {
            *bord = b;
        }
        let f = fond_ligne(choisie, glissee);
        if fond.0 != f {
            fond.0 = f;
        }
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (de, bouge) = (g.de, g.bouge);
    *glisse = None;
    if !bouge {
        // Clic : choisir la ligne (ou la quitter)
        ecran.file_choisie = if ecran.file_choisie == Some(de) { None } else { Some(de) };
        return;
    }
    if !replay.playing
        && let Ok(mut city) = cities.get_mut(v)
        && de < city.state.file.len()
        && vers != de
    {
        let k = city.state.file.remove(de);
        let vers = vers.min(city.state.file.len());
        city.state.file.insert(vers, k);
        ecran.file_choisie = Some(vers);
    } else {
        // (relâché à sa place : l'écran se refait, les lignes reviennent)
        ecran.set_changed();
    }
}

/// Glissement en cours dans la file : ligne prise, hauteur du pointeur au départ, a-t-elle bougé,
/// lignes au départ (rang, centre, hauteur), décalages affichés.
struct Glisse {
    de: usize,
    entite: Entity,
    y0: f32,
    bouge: bool,
    lignes: Vec<(usize, f32, f32)>,
    decalages: std::collections::HashMap<usize, f32>,
}

/// Actions qui changent la ville : mettre en file, avancer, retirer, unités et projets du catalogue.
#[allow(clippy::too_many_arguments)]
fn construire_actions(
    mut ecran: ResMut<EcranVille>,
    replay: Res<crate::feedback::TurnReplay>,
    (map, mut territoire, amenagements, mut cache, gisements, debug): (Res<GameMap>, ResMut<Territory>, Res<Amenagements>, ResMut<CacheVille>, Res<crate::game::Gisements>, Res<crate::game::DebugMode>),
    mut cities: Query<(&mut City, &HexPos)>,
    (mut log, mut state): (ResMut<crate::game::GameLog>, ResMut<GameState>),
    catalogue: Query<(Ref<UiPointer>, &ChoixCatalogue)>,
    actions: Query<(Ref<UiPointer>, &ActionFile)>,
    (mettre, arreter, acheter): (Query<Ref<UiPointer>, With<MettreEnFile>>, Query<Ref<UiPointer>, With<ArreterProjet>>, Query<(Ref<UiPointer>, &AcheterCase)>),
    (reglages, forcages): (Query<(Ref<UiPointer>, &ChoixReglage)>, Query<(Ref<UiPointer>, &Forcage)>),
    (mut anim, time): (ResMut<AnimEcran>, Res<Time>),
) {
    let Some(v) = ecran.ville else { return };
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    // (pas d'ordre pendant que le tour se joue)
    if replay.playing {
        if ecran.confirmer {
            ecran.confirmer = false;
        }
        return;
    }
    let Ok((mut city, centre)) = cities.get_mut(v) else { return };
    let centre = centre.0;
    let nom = city.state.name.clone();
    let mut journal = |t: String| log.push(crate::game::LogText::new().city(&nom, v, centre).text(t));
    // Acheter une case voisine (sa fiche reste ouverte : la case est maintenant à la ville)
    for (i, a) in &acheter {
        if appui(&i) {
            let tour = state.turn;
            match crate::game::acheter_case(&map.0, &mut territoire, &mut state, v, centre, &mut city.state, a.0, tour) {
                Ok(prix) => journal(format!(" achète une case pour {prix} deniers : {}.", map.0.describe(a.0).to_lowercase())),
                Err(e) => journal(format!(" : achat impossible, {e}.")),
            }
        }
    }
    // Mettre en file l'aménagement placé (bouton, double-clic, Entrée)
    // (ne toucher à l'état que s'il change : sinon tout l'écran se refait à chaque image)
    let demande = ecran.confirmer;
    if demande {
        ecran.confirmer = false;
    }
    let confirmer = demande || mettre.iter().any(|i| appui(&i));
    if confirmer && let Some(a) = ecran.placement {
        let cases = cases_vues(&map.0, centre, v, &city, &territoire, &amenagements, &(&gisements, debug.0));
        let choix = {
            let vl = Ville::new(&mut cache, &map.0, centre, v, &city, &territoire, &amenagements);
            let cible = ecran.cible.or_else(|| conseil(&vl, &cases, a, &Bareme::de(&vl)));
            cible.and_then(|h| cases.iter().find(|c| c.hex == h)).and_then(|c| gain_amenagement(&vl, c, a).ok().map(|g| (c, g)))
        };
        if let Some((c, g)) = choix {
            let objet = rules::Production::Amenagement { hex: c.hex, kind: a, cost: g.cout };
            if city.state.mettre_en_file(objet) {
                let n = city.state.file.len();
                journal(format!(" met en file : {} ({}), position {n} ; dans {TOURS_EFFET} tours : {}.", a.nom(), c.nom.to_lowercase(), ecarts(&g.dans)));
                anim.flash_file = Some((n - 1, time.elapsed_secs_f64()));
                ecran.placement = None;
                ecran.cible = None;
            } else {
                // (refus : on reste en placement pour choisir une autre case)
                journal(format!(" : un aménagement est déjà en file sur {}.", c.nom.to_lowercase()));
                ecran.cible = None;
            }
        }
    }
    for (i, e) in &catalogue {
        if !appui(&i) {
            continue;
        }
        match e.0 {
            Element::Unite(k) => {
                city.state.mettre_en_file(rules::Production::Unit(k));
                journal(format!(" met en file : {} ({} dans la file).", k.name(), city.state.file.len()));
            }
            Element::Projet(p) => {
                let nouveau = if city.state.projet == Some(p) { None } else { Some(p) };
                city.state.projet = nouveau;
                city.state.tours_en_projet = 0;
                if let Some(p) = nouveau {
                    city.state.noter(state.turn, rules::GenreEvenement::Projet, format!("Projet « {} » lancé", p.nom()), None);
                }
                journal(match nouveau {
                    Some(p) => format!(" se lance dans le projet « {} » quand sa file est vide.", p.nom()),
                    None => " arrête son projet.".into(),
                });
            }
            Element::Puits => {
                if city.state.mettre_en_file(rules::Production::Puits) {
                    journal(format!(" met en file : puits ({} dans la file).", city.state.file.len()));
                }
            }
            Element::Grenier => {
                if city.state.mettre_en_file(rules::Production::Grenier) {
                    journal(format!(" met en file : grenier ({} dans la file).", city.state.file.len()));
                }
            }
            Element::Sanctuaire => {
                let objet = city.state.sanctuaire_possible();
                if city.state.mettre_en_file(objet) {
                    journal(format!(" met en file : {} ({} dans la file).", objet.name().to_lowercase(), city.state.file.len()));
                }
            }
            Element::Amenagement(_) => {}
        }
    }
    for (i, a) in &actions {
        if !appui(&i) {
            continue;
        }
        match *a {
            ActionFile::Monter(k) if k > 0 && k < city.state.file.len() => {
                city.state.file.swap(k - 1, k);
                ecran.file_choisie = Some(k - 1);
            }
            ActionFile::Retirer(k) if k < city.state.file.len() => {
                ecran.file_choisie = None;
                let c = city.state.file.remove(k);
                journal(format!(" retire de la file : {}{}.", c.objet.name(), if c.investi > 0.0 { format!(" ({} de production perdus)", c.investi.floor()) } else { String::new() }));
            }
            _ => {}
        }
    }
    if arreter.iter().any(|i| appui(&i)) {
        city.state.projet = None;
        city.state.tours_en_projet = 0;
        journal(" arrête son projet.".into());
    }
    for (i, r) in &reglages {
        if !appui(&i) {
            continue;
        }
        match r.0 {
            Reglage::Objectif(o) if city.state.consignes.objectif != o => {
                city.state.consignes.objectif = o;
                journal(format!(" : objectif de nourriture « {} ».", o.nom()));
            }
            Reglage::Priorite(k, p) if city.state.consignes.priorites[k] != p => {
                city.state.consignes.priorites[k] = p;
                journal(format!(" : priorité {} pour {}.", p.nom(), if k == 1 { "la production" } else { "les deniers" }));
            }
            _ => {}
        }
    }
    for (i, f) in &forcages {
        if !appui(&i) {
            continue;
        }
        match *f {
            Forcage::Proposer(h, n) => ecran.forcer = Some((h, n)),
            Forcage::Annuler => ecran.forcer = None,
            Forcage::Confirmer(h, n) | Forcage::Figer(h, n) => {
                let cons = &mut city.state.consignes;
                cons.forcees.retain(|(x, _)| *x != h);
                cons.prioritaires.retain(|x| *x != h);
                cons.forcees.push((h, n));
                ecran.forcer = None;
                let quoi = if matches!(*f, Forcage::Figer(..)) { "figé" } else { "forcé" };
                journal(format!(" : effectif {quoi} à {n} travailleurs sur une case."));
            }
            Forcage::Prioriser(h) => {
                let cons = &mut city.state.consignes;
                cons.forcees.retain(|(x, _)| *x != h);
                cons.prioritaires.retain(|x| *x != h);
                cons.prioritaires.push(h);
                ecran.forcer = None;
                journal(" : case prioritaire, l'intendant la remplit d'abord.".into());
            }
            Forcage::Rendre(h) => {
                city.state.consignes.forcees.retain(|(x, _)| *x != h);
                city.state.consignes.prioritaires.retain(|x| *x != h);
                ecran.forcer = None;
                journal(" : case rendue à l'intendant.".into());
            }
        }
    }
}

/// Liste de droite : cases travaillées ou par remplissage ; vide pour le commerce et l'histoire.
#[allow(clippy::too_many_arguments)]
fn liste_droite(l: &mut ChildSpawnerCommands, onglet: Onglet, cases: &[CaseVue], choisie: Option<Hex>, grandeur: Option<Grandeur>, par_habitants: bool, chronique: &[rules::Evenement]) {
    match onglet {
        Onglet::Ressources => {
            l.spawn(texte("Aucun axe : le commerce n'est pas encore dans le jeu.", 13.0, charte::MUET));
            return;
        }
        Onglet::Histoire => {
            let lieux: Vec<&rules::Evenement> = chronique.iter().filter(|e| e.lieu.is_some()).collect();
            if lieux.is_empty() {
                l.spawn(texte("Aucun événement situé pour l'instant.", 13.0, charte::MUET));
            }
            for e in lieux {
                let h = e.lieu.unwrap_or_default();
                let nom = cases.iter().find(|c| c.hex == h).map_or_else(|| "hors du territoire".to_string(), |c| c.nom.clone());
                l.spawn((
                    ChoixCase(h),
                    UiPointer::default(),
                    Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(if choisie == Some(h) { charte::SELECTION } else { Color::NONE }),
                ))
                .with_children(|i| {
                    i.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                        n.spawn(texte(nom, 13.5, charte::TEXTE));
                        n.spawn(texte(format!("tour {} : {}", e.tour, e.texte), 12.0, charte::MUET));
                    });
                });
            }
            return;
        }
        _ => {}
    }
    let mut lst: Vec<&CaseVue> = cases.iter().filter(|c| !c.centre && c.capacite > 0).collect();
    if onglet == Onglet::Population && par_habitants {
        lst = cases.iter().filter(|c| c.habitants > 0.5).collect();
        lst.sort_by(|a, b| b.habitants.total_cmp(&a.habitants).then(a.hex.cmp(&b.hex)));
    } else if let Some(g) = grandeur {
        lst = cases.iter().filter(|c| valeur(&c.rendement, g) > 0.05).collect();
        lst.sort_by(|a, b| valeur(&b.rendement, g).total_cmp(&valeur(&a.rendement, g)).then(a.hex.cmp(&b.hex)));
    } else if onglet == Onglet::Population {
        lst.sort_by(|a, b| b.remplissage().total_cmp(&a.remplissage()).then(a.hex.cmp(&b.hex)));
    } else {
        lst.retain(|c| c.travailleurs > 0);
        lst.sort_by(|a, b| b.rendement.total().total_cmp(&a.rendement.total()).then(a.hex.cmp(&b.hex)));
    }
    if lst.is_empty() {
        l.spawn(texte("Personne ne travaille sur les cases.", 13.0, charte::MUET));
    }
    for c in lst {
        l.spawn((
            ChoixCase(c.hex),
            UiPointer::default(),
            Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(if choisie == Some(c.hex) { charte::SELECTION } else { Color::NONE }),
        ))
        .with_children(|i| {
            i.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                n.spawn(texte(c.nom.clone(), 13.5, charte::TEXTE));
                let a = c.amenagement.map_or(String::new(), |a| format!(" · {}", a.nom().to_lowercase()));
                let t = if c.centre { "sans travailleurs".to_string() } else { format!("{} / {} travailleurs{a}", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())) };
                n.spawn(texte(t, 12.0, charte::MUET));
            });
            if onglet == Onglet::Population && par_habitants {
                i.spawn(texte(rules::milliers(c.habitants.round() as u64), 13.5, Grandeur::Habitants.couleur()));
            } else if let Some(g) = grandeur {
                i.spawn(texte(rules::decimal(valeur(&c.rendement, g)), 13.5, g.couleur()));
            } else if onglet == Onglet::Population {
                i.spawn(barre(c.remplissage(), 80.0, if c.remplissage() >= 0.9 { charte::ALERTE } else { charte::OK }));
            } else {
                i.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|v| valeurs(v, &c.rendement, 12.5));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Barème d'une ville qui a `nourriture` pour un besoin `besoin`, aujourd'hui et dans 10 tours.
    fn bareme(nourriture: f64, besoin: f64) -> Bareme {
        Bareme { nourriture, besoin, nourriture_dans: nourriture, besoin_dans: besoin, plafond: 1.0 + rules::SURPLUS_POUR_CROITRE }
    }

    #[test]
    fn la_valeur_compte_les_pertes_et_le_frein() {
        let b = bareme(5.5, 5.0);
        // Forêt défrichée en champ : +4 nourriture, −2 production ; prairie en champ : +3 nourriture
        let foret = evaluer(Rendements::new(4.0, -2.0, 0.0), Rendements::new(0.8, -2.5, 0.0), 30, &b);
        let prairie = evaluer(Rendements::new(3.0, 0.0, 0.0), Rendements::new(1.0, 0.0, 0.0), 20, &b);
        // (de 5,5 à 7,5 : 2 de nourriture utile × 1,5 ; au-delà de 1,5 × le besoin, rien)
        assert!((foret.valeur - 1.0).abs() < 1e-9 && (prairie.valeur - 3.0).abs() < 1e-9, "{} {}", foret.valeur, prairie.valeur);
        assert!(prairie.valeur > foret.valeur, "la prairie vaut mieux : pas de production perdue");
        assert!((prairie.rentabilite.unwrap() - 20.0 / 3.0).abs() < 1e-9);
        // Ville déjà au plafond : la nourriture ne compte plus, la production si
        let rassasiee = bareme(10.0, 5.0);
        assert!(evaluer(Rendements::new(3.0, 0.0, 0.0), Rendements::new(1.0, 0.0, 0.0), 20, &rassasiee).valeur.abs() < 1e-9);
        assert!(evaluer(Rendements::new(0.0, 2.0, 0.0), Rendements::new(0.0, 1.0, 0.0), 15, &rassasiee).valeur > 1.9);
    }

    #[test]
    fn une_perte_de_nourriture_pres_de_la_famine_pese_lourd() {
        // Coupe de bois sur une case nourricière : −1 nourriture, +3 production
        let (plein, maintenant) = (Rendements::new(-1.0, 3.0, 0.0), Rendements::new(-1.0, 1.5, 0.0));
        let large = evaluer(plein, maintenant, 15, &bareme(10.0, 5.0));
        assert_eq!(large.risque, Risque::Aucun);
        assert!((large.valeur - 3.0).abs() < 1e-9, "au-delà du plafond, la nourriture perdue ne compte pas");
        let juste = evaluer(plein, maintenant, 15, &bareme(5.2, 5.0));
        assert!(matches!(juste.risque, Risque::Famine(k) if k < 1.0));
        assert!(juste.valeur < 0.0, "{}", juste.valeur);
        let bord = evaluer(plein, maintenant, 15, &bareme(6.5, 5.0));
        assert!(matches!(bord.risque, Risque::BordDeFamine(_)));
        assert!(bord.valeur < large.valeur);
    }

    #[test]
    fn pas_de_coupe_de_bois_sur_une_foret_defrichee() {
        let map = crate::map::WorldMap::filled(6, 6, crate::map::Terrain::Forest);
        let h = Hex::from_offset(2, 2);
        let r = Rendements::default();
        let mut c = CaseVue { hex: h, centre: false, nom: String::new(), travailleurs: 0, capacite: 0, max: r, courbe: intendant::Courbe::default(), rendement: r, amenagement: None, habitants: 0.0, en_file: false, force: None, prioritaire: false, gisement: None };
        assert!(possible(&map, &c, Amenagement::CoupeDeBois).is_ok());
        c.amenagement = Some(Amenagement::Champ);
        assert!(possible(&map, &c, Amenagement::CoupeDeBois).is_err(), "plus d'arbres");
        assert!(possible(&map, &c, Amenagement::Paturage).is_ok());
    }
}
