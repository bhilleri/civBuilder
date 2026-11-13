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
const ZOOM_MAX: f32 = 4.0;

/// Onglets de l'écran.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Onglet {
    #[default]
    Rendements,
    Population,
    Commerce,
    Histoire,
}

impl Onglet {
    const TOUS: [Onglet; 4] = [Onglet::Rendements, Onglet::Population, Onglet::Commerce, Onglet::Histoire];

    fn nom(self) -> &'static str {
        match self {
            Onglet::Rendements => "Rendements",
            Onglet::Population => "Population",
            Onglet::Commerce => "Commerce",
            Onglet::Histoire => "Histoire",
        }
    }
}

/// État de l'écran : ville affichée (ouvert si `Some`), onglet, case choisie, couches de la carte.
#[derive(Resource, Default)]
pub struct EcranVille {
    pub ville: Option<Entity>,
    pub onglet: Onglet,
    pub case: Option<Hex>,
    /// Couche « aménagements » de la carte (motif et rond vert).
    pub amenagements_caches: bool,
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
    /// Groupe montré par le catalogue (aménagements, projets, unités) : tout tient à l'écran.
    pub catalogue_groupe: GroupeCatalogue,
    /// Dernière ville montrée (une autre ville s'ouvre sur l'onglet Rendements).
    derniere: Option<Entity>,
}

/// Groupes du catalogue de construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupeCatalogue {
    #[default]
    Amenagements,
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
        rules::Production::Unit(_) | rules::Production::Puits | rules::Production::Grenier => None,
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

/// « la nourriture », « la production », « les deniers ».
fn article(g: Grandeur) -> &'static str {
    match g {
        Grandeur::Deniers => "les deniers",
        Grandeur::Production => "la production",
        _ => "la nourriture",
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
}

/// Anneau de la case conseillée par l'intendant (or : le conseil).
const CONSEIL: Color = Color::srgb(0.95, 0.81, 0.45);

impl EcranVille {
    /// L'écran est-il ouvert ?
    pub fn ouvert(&self) -> bool {
        self.ville.is_some()
    }
}

/// Zoom et déplacement de la carte de la ville (à part : les changer ne refait pas tout l'écran).
#[derive(Resource)]
struct VueCarte {
    zoom: f32,
    decalage: Vec2,
}

impl Default for VueCarte {
    fn default() -> Self {
        VueCarte { zoom: 1.0, decalage: Vec2::ZERO }
    }
}

/// Bouton qui ouvre l'écran de la ville sélectionnée (panneau de production).
#[derive(Component)]
pub struct BoutonEcranVille;

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
#[derive(Component)]
struct Cadre;
#[derive(Component)]
struct Plan;
#[derive(Component)]
struct ContourSurvol;
#[derive(Component)]
struct BoutonZoom(i8);
#[derive(Component)]
struct TitreListe;
#[derive(Component)]
struct Liste;
#[derive(Component)]
struct Bulle;
#[derive(Component)]
struct Fermer;
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
#[derive(Clone, Copy, PartialEq)]
enum Element {
    Unite(crate::rules::UnitKind),
    Amenagement(Amenagement),
    Projet(crate::rules::Projet),
    /// Puits de la ville (sur le centre, un par ville).
    Puits,
    /// Grenier de la ville (un par ville).
    Grenier,
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
    Detail(Grandeur),
    Construire,
}
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
    texte: String,
    remedes: Vec<(Remede, String)>,
    alertes: Vec<(Etat, String, Remede)>,
}

/// Diagnostic de la croissance de la ville.
fn frein(vl: &Ville, city: &City, cases: &[CaseVue]) -> Frein {
    let (map, prevision) = (vl.map, vl.prevision);
    let c = &city.state;
    let y = city.travail.rendements;
    let besoin = c.besoin_nourriture().max(1e-6);
    let couverture = y.nourriture / besoin;
    let limite = limite_nourriture(cases, c);
    // Remèdes pour la nourriture : les aménagements qui en donnent le plus, à leur case conseillée
    let mut nourrir: Vec<(f64, Remede, String)> = [Amenagement::Champ, Amenagement::Paturage]
        .into_iter()
        .filter_map(|a| {
            let case = conseil(vl, cases, a, &Bareme::de(vl)).and_then(|h| cases.iter().find(|x| x.hex == h))?;
            let g = gain_amenagement(vl, case, a).ok()?;
            (g.plein.nourriture > 0.05).then(|| {
                (g.plein.nourriture, Remede::Placer(a), format!("{} : limite +{} habitants (+{} nourriture case pleine)", capitale(a.nom()), rules::milliers((g.plein.nourriture * rules::HABITANTS_PAR_NOURRITURE) as u64), rules::decimal(g.plein.nourriture)))
            })
        })
        .collect();
    nourrir.sort_by(|a, b| b.0.total_cmp(&a.0));
    let nourrir: Vec<(Remede, String)> = nourrir.into_iter().map(|(_, r, t)| (r, t)).collect();
    let objectif = c.consignes.objectif;
    let (etat, texte, mut remedes) = if couverture < 1.0 {
        (Etat::Grave, format!("la nourriture manque ({} × le besoin) : famine, la population baisse", rules::decimal(couverture)), nourrir)
    } else if objectif.cible() < 1.0 + rules::SURPLUS_POUR_CROITRE && couverture < 1.0 + rules::SURPLUS_POUR_CROITRE - 1e-6 {
        (Etat::Alerte, format!("l'objectif de l'intendant, « {} », limite la croissance", objectif.nom().to_lowercase()), vec![(Remede::Detail(Grandeur::Nourriture), "Changer l'objectif".to_string())])
    } else {
        // Tours avant que la croissance ne s'arrête, d'après la prévision (l'intendant répartit
        // les habitants à venir ; la nourriture des cases pleines borne la population)
        let suivant = rules::habitants_du_palier(c.palier() + 1);
        let palier = if (suivant as f64) > limite { format!(" · palier {} ({}) hors de portée sans remède", c.palier() + 1, rules::milliers(suivant)) } else { String::new() };
        match rules::arret_de_croissance(prevision) {
            Some(t) => (
                if t <= 20 { Etat::Alerte } else { Etat::Ok },
                format!("la nourriture : limite {} habitants (cases pleines), croissance arrêtée dans ~{t} tours vers {}{palier}", rules::milliers(limite as u64), rules::milliers(prevision[t - 1].habitants)),
                nourrir,
            ),
            None => (Etat::Ok, format!("rien pour l'instant ; à long terme, la nourriture : limite {} habitants (cases pleines, plus de {} tours){palier}", rules::milliers(limite as u64), prevision.len()), nourrir),
        }
    };
    remedes.truncate(3);
    // (palier suivant hors de portée : les remèdes restent, le texte y renvoie)
    let hors_de_portee = rules::habitants_du_palier(c.palier() + 1) as f64 > limite;
    if etat == Etat::Ok && !hors_de_portee {
        remedes.clear();
    }
    let etat = if etat == Etat::Ok && hors_de_portee { Etat::Alerte } else { etat };
    // Autres alertes (« rien à produire » est déjà dans l'en-tête et sur la carte de la production)
    let mut alertes = Vec::new();
    if c.colon_bloque() {
        alertes.push((Etat::Alerte, format!("Colon en tête de file : il faut {} habitants pour le former ; rien d'autre n'avance", rules::milliers(rules::CityState::HABITANTS_POUR_COLON)), Remede::Construire));
    }
    if city.travail.chomeurs > 0 {
        let a = if cases.iter().any(|x| possible(map, x, Amenagement::Champ).is_ok()) { Remede::Placer(Amenagement::Champ) } else { Remede::Construire };
        // (Benjamin : pas une limite ; le chômage baissera l'attractivité de la ville, à venir)
        alertes.push((Etat::Ok, format!("{} sans emploi : aménager des cases donne des emplois", rules::milliers(city.travail.chomeurs.into())), a));
    }
    if c.projet.is_some() && c.file.is_empty() && c.tours_en_projet >= rules::RAPPEL_PROJET {
        alertes.push((Etat::Ok, format!("En projet depuis {} tours", c.tours_en_projet), Remede::Construire));
    }
    Frein { etat, texte, remedes, alertes }
}
/// Forcer l'effectif d'une case : proposer un autre effectif, confirmer, annuler, rendre à l'intendant.
#[derive(Component, Clone, Copy)]
enum Forcage {
    Proposer(Hex, u32),
    Confirmer(Hex, u32),
    Annuler,
    Rendre(Hex),
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
            c.max = intendant::rendement_max(self.map, h, Some(a));
            c.capacite = intendant::capacite(self.map, h, Some(a));
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
    if a == Amenagement::CoupeDeBois && matches!(c.amenagement, Some(Amenagement::Champ | Amenagement::Paturage | Amenagement::Legumineuses | Amenagement::Jardin)) {
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
            .init_resource::<VueCarte>()
            .init_resource::<FinDeTourJoueur>()
            .init_resource::<AnimEcran>()
            .add_systems(Update, animer.after(EcranVilleSet))
            .add_systems(Update, fin_de_tour_joueur.after(crate::input::InputSet).before(crate::game::EndTurnSet))
            .add_systems(Startup, construire)
            .add_systems(
                Update,
                (ouvrir_fermer, commandes, carte_souris, defiler, glisser_file, construire_actions, reconstruire, placer_plan, afficher)
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
    /// Position par rapport au centre (en tenant compte du tour du monde).
    local: Hex,
    centre: bool,
    nom: String,
    travailleurs: u32,
    capacite: u32,
    max: Rendements,
    rendement: Rendements,
    amenagement: Option<Amenagement>,
    /// Habitants de la case : pour l'instant, chacun vit là où il travaille (les sans-emploi au centre).
    habitants: f64,
    /// Un aménagement de la case est déjà dans la file de construction.
    en_file: bool,
    /// Effectif forcé par le joueur (l'intendant ne le change plus).
    force: Option<u32>,
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
    let wrap = map.wrap();
    let possedees: Vec<Hex> = territoire.0.iter().filter(|(_, e)| **e == ville).map(|(h, _)| *h).collect();
    let dispo = rules::cases_de_la_ville(map, centre, &possedees, &Default::default(), &amenagements.0);
    let rc = Rendements::from(rules::center_yields(map, centre));
    let mut cases = vec![CaseVue { hex: centre, local: Hex::new(0, 0), centre: true, nom: "Centre de la ville".into(), travailleurs: 0, capacite: 0, max: rc, rendement: rc, amenagement: None, habitants: 0.0, en_file: false, force: None, gisement: None }];
    for c in dispo {
        let w = city.travail.sur(c.hex);
        let near = wrap.nearest(centre, c.hex);
        cases.push(CaseVue {
            hex: c.hex,
            local: Hex::new(near.q - centre.q, near.r - centre.r),
            centre: false,
            nom: {
                // (un champ ou un pâturage sur une forêt l'a défrichée)
                let a = amenagements.0.get(&c.hex).copied();
                let defrichee = matches!(a, Some(Amenagement::Champ | Amenagement::Paturage | Amenagement::Legumineuses | Amenagement::Jardin)) && map.get(c.hex).is_some_and(|t| t.is_wooded());
                if defrichee { format!("{} défrichée", nom_de_case(map, c.hex)) } else { nom_de_case(map, c.hex) }
            },
            travailleurs: w,
            capacite: c.capacite,
            max: c.max,
            rendement: intendant::rendement(c.max, w, c.capacite),
            amenagement: amenagements.0.get(&c.hex).copied(),
            habitants: 0.0,
            en_file: city.state.file.iter().any(|k| matches!(k.objet, rules::Production::Amenagement { hex, .. } if hex == c.hex)),
            force: city.state.consignes.forcees.iter().find(|(h, _)| *h == c.hex).map(|(_, n)| *n),
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
    local: Hex,
    nom: String,
    prix: i32,
    naturel: Rendements,
    /// Le meilleur aménagement possible et ce qu'il rapporterait.
    meilleur: Option<(Amenagement, Rendements)>,
}

/// Cases voisines du territoire de la ville, à personne : celles qu'elle peut acheter.
fn cases_a_acheter(map: &crate::map::WorldMap, centre: Hex, ville: Entity, territoire: &Territory) -> Vec<CaseAchat> {
    let wrap = map.wrap();
    rules::cases_gagnables(map, ville, &territoire.0)
        .into_iter()
        .map(|h| {
            let near = wrap.nearest(centre, h);
            let meilleur = Amenagement::possibles(map, h).into_iter().map(|a| (a, intendant::rendement_max(map, h, Some(a)))).max_by(|a, b| a.1.total().total_cmp(&b.1.total()));
            CaseAchat { hex: h, local: Hex::new(near.q - centre.q, near.r - centre.r), nom: nom_de_case(map, h), prix: rules::prix_case(map.distance(centre, h)), naturel: intendant::rendement_max(map, h, None), meilleur }
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

fn construire(mut commands: Commands, images: Res<Images>) {
    let hexagone = images.hexagone.clone();
    commands
        .spawn((
            Racine,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(14.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(charte::FOND),
            GlobalZIndex(90),
            // Bloque les clics et la molette vers la carte du monde
            UiPointer::default(),
        ))
        .with_children(|r| {
            r.spawn((EnTete, panneau(), BackgroundColor(charte::PANNEAU)));
            // Onglets
            r.spawn(Node { column_gap: Val::Px(4.0), border: UiRect::bottom(Val::Px(1.0)), ..default() }).insert(BorderColor::all(charte::TRAIT)).with_children(|o| {
                for onglet in Onglet::TOUS {
                    o.spawn((
                        BoutonOnglet(onglet),
                        UiPointer::default(),
                        Node { padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)), border: UiRect::bottom(Val::Px(3.0)), ..default() },
                        BorderColor::all(Color::NONE),
                        children![texte(onglet.nom(), 17.0, charte::DOUX)],
                    ));
                }
            });
            // Zone : explication à gauche, carte et liste à droite
            r.spawn(Node { flex_grow: 1.0, min_height: Val::Px(0.0), column_gap: Val::Px(12.0), ..default() }).with_children(|z| {
                z.spawn((Gauche, Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), width: Val::Percent(46.0), min_width: Val::Px(0.0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default()));
                z.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), width: Val::Percent(54.0), min_width: Val::Px(0.0), ..default() }).with_children(|d| {
                    d.spawn((Node { flex_grow: 1.0, min_height: Val::Px(0.0), ..panneau() }, BackgroundColor(charte::PANNEAU))).with_children(|p| {
                        p.spawn((Legende, Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(14.0), row_gap: Val::Px(6.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() }, BackgroundColor(charte::SURFACE)));
                        p.spawn((Cadre, Node { flex_grow: 1.0, min_height: Val::Px(200.0), overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() }, BackgroundColor(Color::srgb(0.06, 0.075, 0.11))))
                            .with_children(|c| {
                                c.spawn((Plan, Node { position_type: PositionType::Absolute, ..default() }));
                                c.spawn((BarreValidation, Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(10.0), justify_content: JustifyContent::Center, display: Display::None, ..default() }));
                                c.spawn(Node { position_type: PositionType::Absolute, right: Val::Px(8.0), top: Val::Px(8.0), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: Val::Px(4.0), ..default() }).with_children(|b| {
                                    for (sens, signe) in [(1, "+"), (-1, "−"), (0, "")] {
                                        let mut bouton = b.spawn((
                                            BoutonZoom(sens),
                                            UiPointer::default(),
                                            Node { width: Val::Px(if sens == 0 { 76.0 } else { 30.0 }), height: Val::Px(28.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, column_gap: Val::Px(4.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                                            BackgroundColor(charte::SURFACE),
                                            BorderColor::all(charte::TRAIT2),
                                        ));
                                        if sens == 0 {
                                            // Recentrer : un petit hexagone
                                            bouton.with_children(|r| {
                                                r.spawn((Node { width: Val::Px(10.0), height: Val::Px(12.0), ..default() }, ImageNode::new(hexagone.clone()).with_color(charte::TEXTE)));
                                                r.spawn(texte("Recentrer", 11.5, charte::TEXTE));
                                            });
                                        } else {
                                            bouton.with_child(texte(signe, 16.0, charte::TEXTE));
                                        }
                                    }
                                });
                            });
                    });
                    d.spawn((Node { height: Val::Percent(34.0), flex_shrink: 0.0, ..panneau() }, BackgroundColor(charte::PANNEAU))).with_children(|p| {
                        p.spawn((TitreListe, texte("", 15.0, charte::TEXTE)));
                        p.spawn((Liste, Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default()));
                    });
                });
            });
            // Infobulle de la carte
            r.spawn((
                Bulle,
                Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(9.0), Val::Px(6.0)), max_width: Val::Px(280.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), display: Display::None, ..default() },
                BackgroundColor(charte::FOND),
                BorderColor::all(charte::TRAIT2),
                GlobalZIndex(95),
            ));
        });
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
    mut vue: ResMut<VueCarte>,
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
    *vue = VueCarte::default();
}

/// Ouvre l'écran (touche C, bouton) pour la ville sélectionnée ; le ferme (Échap, bouton, plus de
/// ville sélectionnée).
#[allow(clippy::too_many_arguments)]
fn ouvrir_fermer(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    cities: Query<(), With<City>>,
    (wiki, menu): (Res<crate::wiki::WikiState>, Res<crate::menu::GameMenu>),
    mut ecran: ResMut<EcranVille>,
    mut vue: ResMut<VueCarte>,
    ouvrir: Query<Ref<UiPointer>, With<BoutonEcranVille>>,
    fermer: Query<Ref<UiPointer>, With<Fermer>>,
    (state, mut tour_vu): (Res<GameState>, Local<u32>),
) {
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    let ville = selection.0.filter(|e| cities.contains(*e));
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
        if (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && ecran.placement.is_some() {
            ecran.confirmer = true;
        }
        // 1 à 3 : détail de la nourriture, de la production, des deniers
        for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].into_iter().enumerate() {
            // (pendant la construction, elles choisiraient une grandeur cachée)
            if keys.just_pressed(k) && !wiki.open && !ecran.catalogue {
                let g = DETAILLABLES[i];
                ecran.grandeur = if ecran.grandeur == Some(g) && ecran.onglet == Onglet::Rendements { None } else { Some(g) };
                ecran.onglet = Onglet::Rendements;
                ecran.case = None;
            }
        }
    } else if let Some(v) = ville
        && !wiki.open
        && !menu.open
        && (keys.just_pressed(KeyCode::KeyC) || ouvrir.iter().any(|i| appui(&i)))
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
        *vue = VueCarte::default();
    }
}

/// Boutons de l'écran : onglets, zoom, cases de la liste, fiche, couches. Couleurs au survol.
#[allow(clippy::type_complexity)]
fn commandes(
    mut ecran: ResMut<EcranVille>,
    mut vue: ResMut<VueCarte>,
    mut onglets: Query<(Ref<UiPointer>, &BoutonOnglet, &mut BorderColor, &Children)>,
    mut textes: Query<&mut TextColor>,
    zoom: Query<(Ref<UiPointer>, &BoutonZoom)>,
    choix: Query<(Ref<UiPointer>, &ChoixCase)>,
    fiche: Query<Ref<UiPointer>, With<FermerFiche>>,
    couches: Query<Ref<UiPointer>, With<BasculeAmenagements>>,
    (grandeurs, periodes, fils): (Query<(Ref<UiPointer>, &ChoixGrandeur)>, Query<(Ref<UiPointer>, &ChoixPeriode)>, Query<(Ref<UiPointer>, &Fil)>),
    (catalogue, construire, ici): (Query<(Ref<UiPointer>, &ChoixCatalogue)>, Query<Ref<UiPointer>, With<BoutonConstruire>>, Query<(Ref<UiPointer>, &AmenagerIci)>),
    (cartes_pop, genres, remedes, groupes): (Query<(Ref<UiPointer>, &ChoixCartePop)>, Query<(Ref<UiPointer>, &ChoixGenre)>, Query<(Ref<UiPointer>, &BoutonRemede)>, Query<(Ref<UiPointer>, &ChoixGroupe)>),
    mut survols: Query<(&UiPointer, &mut BackgroundColor), (Or<(With<ChoixCase>, With<BoutonZoom>, With<Fermer>, With<FermerFiche>, With<BasculeAmenagements>, With<Fil>, With<ChoixCatalogue>, With<ActionFile>, With<ArreterProjet>, With<AmenagerIci>, With<ChoixReglage>, With<BoutonRemede>)>, Changed<UiPointer>)>,
) {
    if !ecran.ouvert() {
        return;
    }
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    for (i, b, mut bord, enfants) in &mut onglets {
        if appui(&i) && ecran.onglet != b.0 {
            ecran.onglet = b.0;
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
        if appui(&i) {
            match b.0 {
                0 => *vue = VueCarte::default(),
                s => vue.zoom = (vue.zoom * if s > 0 { 1.4 } else { 1.0 / 1.4 }).clamp(1.0, ZOOM_MAX),
            }
        }
    }
    for (i, c) in &choix {
        if appui(&i) {
            ecran.case = if ecran.case == Some(c.0) { None } else { Some(c.0) };
        }
    }
    if fiche.iter().any(|i| appui(&i)) {
        ecran.case = None;
    }
    if couches.iter().any(|i| appui(&i)) {
        ecran.amenagements_caches = !ecran.amenagements_caches;
    }
    for (i, g) in &grandeurs {
        if appui(&i) {
            ecran.grandeur = if ecran.grandeur == Some(g.0) && ecran.case.is_none() { None } else { Some(g.0) };
            ecran.onglet = Onglet::Rendements;
            ecran.case = None;
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
    for (i, r) in &remedes {
        if !appui(&i) {
            continue;
        }
        ecran.onglet = Onglet::Rendements;
        ecran.case = None;
        ecran.cible = None;
        match r.0 {
            Remede::Placer(a) => {
                ecran.catalogue = true;
                ecran.catalogue_groupe = GroupeCatalogue::Amenagements;
                ecran.placement = Some(a);
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

/// Géométrie de la carte de la ville : taille d'un hexagone (rayon, pixels) pour la place donnée.
fn rayon_hex(cadre: Vec2, cases: &[CaseVue], achats: &[CaseAchat], zoom: f32) -> f32 {
    let r = cases.iter().map(|c| c.local).chain(achats.iter().map(|a| a.local)).map(|l| Hex::new(0, 0).distance(l)).max().unwrap_or(1).max(1) as f32;
    let s = (cadre.x / (RACINE_3 * (2.0 * r + 1.0))).min(cadre.y / (3.0 * r + 2.0)) * 0.94;
    s.max(8.0) * zoom
}

/// Centre (pixels, y vers le bas) d'une case dans le plan de la carte.
fn position(local: Hex, s: f32) -> Vec2 {
    Vec2::new(s * RACINE_3 * (local.q as f32 + local.r as f32 / 2.0), s * 1.5 * local.r as f32)
}

/// Case (relative au centre) sous un point du plan.
fn case_sous(p: Vec2, s: f32) -> Hex {
    let q = (RACINE_3 / 3.0 * p.x - p.y / 3.0) / s;
    let r = (2.0 / 3.0 * p.y) / s;
    let (x, z) = (q, r);
    let y = -x - z;
    let (mut rx, ry, mut rz) = (x.round(), y.round(), z.round());
    let (dx, dy, dz) = ((rx - x).abs(), (ry - y).abs(), (rz - z).abs());
    if dx > dy && dx > dz {
        rx = -ry - rz;
    } else if dy <= dz {
        rz = -rx - ry;
    }
    Hex::new(rx as i32, rz as i32)
}

/// Molette sur une colonne de l'écran (gauche, liste de droite) : la fait défiler.
fn defiler(ecran: Res<EcranVille>, scroll: Res<AccumulatedMouseScroll>, pointeurs: Query<(&PointerId, &PointerLocation)>, mut colonnes: Query<(&mut ScrollPosition, &ComputedNode, &UiGlobalTransform), Or<(With<Gauche>, With<Liste>)>>) {
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

/// Souris sur la carte : survol (infobulle, contour), clic (fiche de la case), glisser (déplacer),
/// molette (zoom autour du pointeur).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn carte_souris(
    mut ecran: ResMut<EcranVille>,
    mut vue: ResMut<VueCarte>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window, With<PrimaryWindow>>,
    cadre: Single<(&UiGlobalTransform, &ComputedNode), With<Cadre>>,
    pointeurs: Query<(&PointerId, &PointerLocation)>,
    (map, territoire, amenagements, mut cache, gisements, debug): (Res<GameMap>, Res<Territory>, Res<Amenagements>, ResMut<CacheVille>, Res<crate::game::Gisements>, Res<crate::game::DebugMode>),
    cities: Query<(&City, &HexPos)>,
    choix: Query<(&UiPointer, &ChoixCase)>,
    mut contour: Query<(&mut Node, &mut ImageNode), (With<ContourSurvol>, Without<Bulle>)>,
    bulle: Single<(Entity, &mut Node), With<Bulle>>,
    mut commands: Commands,
    mut glisse: Local<Option<(Vec2, bool)>>,
    mut montre: Local<Option<Hex>>,
    (time, mut dernier_clic, boutons_carte, lignes): (Res<Time>, Local<(f64, Option<Hex>)>, Query<&UiPointer, Or<(With<MettreEnFile>, With<BoutonZoom>)>>, Query<(&UiPointer, &LigneFile)>),
) {
    let (bulle_e, mut bulle_node) = bulle.into_inner();
    let Some((city, centre)) = ecran.ville.and_then(|v| cities.get(v).ok().map(|c| (v, c))).map(|(v, (c, p))| ((v, c), p.0)) else {
        *glisse = None;
        return;
    };
    let (transform, computed) = *cadre;
    let taille = computed.size() * computed.inverse_scale_factor();
    let cases = cases_vues(&map.0, centre, city.0, city.1, &territoire, &amenagements, &(&gisements, debug.0));
    let achats = cases_a_acheter(&map.0, centre, city.0, &territoire);
    let s = rayon_hex(taille, &cases, &achats, vue.zoom);
    // (les cases à acheter ne se choisissent pas pendant la construction)
    let construction = ecran.catalogue;
    let achat_sous = |local: Hex| achats.iter().find(|x| x.local == local).filter(|_| !construction);
    // Pointeur de la souris lu dans le picking (comme les boutons ; marche aussi avec le pilote)
    let souris = pointeurs.iter().find(|(id, _)| **id == PointerId::Mouse).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position);
    let normalise = souris.and_then(|p| computed.normalize_point(*transform, p / computed.inverse_scale_factor()));
    let curseur = normalise.map(|n| n * taille);
    // (un bouton posé sur la carte prend le clic : il ne choisit pas la case dessous)
    let sur_bouton = boutons_carte.iter().any(|b| *b != UiPointer::None);
    let dessus = normalise.is_some_and(|n| n.x.abs() <= 0.5 && n.y.abs() <= 0.5) && !sur_bouton;
    // Clic droit en placement : revenir d'un cran (case choisie, puis aménagement)
    if dessus && mouse.just_pressed(MouseButton::Right) && ecran.placement.is_some() {
        if ecran.cible.is_some() {
            ecran.cible = None;
        } else {
            ecran.placement = None;
        }
    }

    // Molette : zoom autour du pointeur
    if dessus && scroll.delta.y != 0.0 {
        let pas = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y.signum() * 0.25,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.004,
        };
        let z2 = (vue.zoom * (1.0 + pas)).clamp(1.0, ZOOM_MAX);
        if let Some(c) = curseur {
            let k = z2 / vue.zoom;
            vue.decalage = c - (c - vue.decalage) * k;
        }
        vue.zoom = z2;
    }
    // Glisser pour déplacer ; clic sans glisser pour choisir une case
    if dessus && mouse.just_pressed(MouseButton::Left) && let Some(c) = curseur {
        *glisse = Some((c, false));
    }
    if let Some((depart, bouge)) = glisse.as_mut()
        && let Some(c) = curseur
    {
        let d = c - *depart;
        if *bouge || d.length() > 4.0 {
            *bouge = true;
            vue.decalage += d;
            // (bornée : la ville ne sort pas du cadre)
            let borne = taille * vue.zoom / 2.0;
            vue.decalage = vue.decalage.clamp(-borne, borne);
            *depart = c;
        }
    }
    if mouse.just_released(MouseButton::Left) {
        if let Some((_, false)) = *glisse
            && dessus
            && let Some(c) = curseur
        {
            let local = case_sous(c - vue.decalage, s);
            let vise = cases.iter().find(|x| x.local == local);
            let hex = vise.map(|x| x.hex);
            if let Some(a) = ecran.placement {
                // Placement : la case doit pouvoir porter l'aménagement ; double-clic : mise en file
                if let Some(x) = vise.filter(|x| possible(&map.0, x, a).is_ok()) {
                    let maintenant = time.elapsed_secs_f64();
                    if dernier_clic.1 == Some(x.hex) && maintenant - dernier_clic.0 < 0.45 {
                        ecran.confirmer = true;
                    }
                    *dernier_clic = (maintenant, Some(x.hex));
                    ecran.cible = Some(x.hex);
                }
            } else if !ecran.catalogue {
                // (une case voisine à acheter s'ouvre aussi : sa fiche propose l'achat)
                let hex = hex.or_else(|| achat_sous(local).map(|x| x.hex));
                ecran.case = if hex.is_some() && hex == ecran.case { None } else { hex };
            }
        }
        *glisse = None;
    }
    if vue.zoom <= 1.0 {
        vue.decalage = Vec2::ZERO;
    }

    // Survol : par la carte, ou par une ligne de la liste
    let par_liste = choix
        .iter()
        .find(|(i, _)| **i != UiPointer::None)
        .map(|(_, c)| c.0)
        .or_else(|| lignes.iter().find(|(i, _)| **i != UiPointer::None).and_then(|(_, l)| city.1.state.file.get(l.0)).and_then(|k| case_de(&k.objet)).map(|h| map.0.canonical(h)));
    let survol = if dessus && glisse.is_none_or(|g| !g.1) {
        curseur.and_then(|c| {
            let local = case_sous(c - vue.decalage, s);
            cases.iter().find(|x| x.local == local)
        })
    } else {
        par_liste.and_then(|h| cases.iter().find(|x| x.hex == h))
    };
    let survol_achat = if survol.is_none() && dessus && glisse.is_none_or(|g| !g.1) { curseur.and_then(|c| achat_sous(case_sous(c - vue.decalage, s))) } else { None };
    for (mut node, mut image) in &mut contour {
        match survol.map(|c| c.local).or(survol_achat.map(|a| a.local)) {
            Some(local) => {
                let p = position(local, s);
                node.display = Display::Flex;
                node.left = Val::Px(p.x - s * RACINE_3 / 2.0);
                node.top = Val::Px(p.y - s);
                node.width = Val::Px(s * RACINE_3);
                node.height = Val::Px(2.0 * s);
                image.color = Color::srgba(1.0, 1.0, 1.0, 0.55);
            }
            None => node.display = Display::None,
        }
    }
    // Infobulle (seulement au survol de la carte)
    let sur_carte = dessus && (survol.is_some() || survol_achat.is_some());
    match (sur_carte, survol, souris) {
        (true, None, Some(p)) if let Some(a) = survol_achat => {
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
                commands.entity(bulle_e).with_children(|b| match placement {
                    Some((a, g)) => infobulle_placement(b, c, a, g, &Bareme::de(&vl)),
                    None if ecran.onglet == Onglet::Histoire => {
                        b.spawn(texte(c.nom.clone(), 14.0, charte::TEXTE));
                        let ev: Vec<&rules::Evenement> = city.1.state.chronique.iter().filter(|e| e.lieu == Some(c.hex)).collect();
                        if ev.is_empty() {
                            b.spawn(texte("aucun événement ici", 12.5, charte::MUET));
                        }
                        for e in ev {
                            b.spawn(texte(format!("tour {} : {}", e.tour, e.texte), 12.5, charte::DOUX));
                        }
                    }
                    None => infobulle(b, c, ecran.onglet, ecran.carte_habitants),
                });
            }
        }
        _ => {
            bulle_node.display = Display::None;
            *montre = None;
        }
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

/// Contenu de l'infobulle d'une case, selon la carte affichée.
fn infobulle(b: &mut ChildSpawnerCommands, c: &CaseVue, onglet: Onglet, par_habitants: bool) {
    b.spawn(texte(c.nom.clone(), 14.0, charte::TEXTE));
    if onglet == Onglet::Population && par_habitants {
        b.spawn(texte(format!("{} habitants", rules::milliers(c.habitants.round() as u64)), 13.0, Grandeur::Habitants.couleur()));
        b.spawn(texte(format!("{} travailleurs sur la case", rules::milliers(c.travailleurs.into())), 12.5, charte::MUET));
    } else if onglet == Onglet::Population {
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

/// Place le plan de la carte (centre du cadre plus le déplacement).
fn placer_plan(vue: Res<VueCarte>, cadre: Single<&ComputedNode, With<Cadre>>, mut plan: Single<&mut Node, With<Plan>>) {
    let taille = cadre.size() * cadre.inverse_scale_factor();
    let (l, t) = (Val::Px(taille.x / 2.0 + vue.decalage.x), Val::Px(taille.y / 2.0 + vue.decalage.y));
    if plan.left != l || plan.top != t {
        plan.left = l;
        plan.top = t;
    }
}

fn afficher(ecran: Res<EcranVille>, mut racine: Single<&mut Node, With<Racine>>) {
    let d = if ecran.ouvert() { Display::Flex } else { Display::None };
    if racine.display != d {
        racine.display = d;
    }
}

// ------------------------------------------------------------------ Contenu

/// Refait le contenu quand l'écran, la ville, ses aménagements, le tour ou la taille de la carte changent.
#[allow(clippy::too_many_arguments)]
fn reconstruire(
    mut commands: Commands,
    ecran: Res<EcranVille>,
    vue: Res<VueCarte>,
    images: Res<Images>,
    (map, territoire, amenagements, state, mut cache, gisements, debug): (Res<GameMap>, Res<Territory>, Res<Amenagements>, Res<GameState>, ResMut<CacheVille>, Res<crate::game::Gisements>, Res<crate::game::DebugMode>),
    cities: Query<(Ref<City>, &HexPos)>,
    (entete, gauche, legende, plan, titre, liste): (Single<Entity, With<EnTete>>, Single<Entity, With<Gauche>>, Single<Entity, With<Legende>>, Single<Entity, With<Plan>>, Single<Entity, With<TitreListe>>, Single<Entity, With<Liste>>),
    (cadre, largeur_gauche, barre): (Single<&ComputedNode, With<Cadre>>, Single<&ComputedNode, With<Gauche>>, Single<(Entity, &mut Node), With<BarreValidation>>),
    mut textes: Query<&mut Text>,
    mut assets: ResMut<Assets<Image>>,
    (mut derniere, mut en_attente): (Local<(Vec2, f32, u32, f32)>, Local<bool>),
    mut graphique: Local<Option<Handle<Image>>>,
) {
    let Some(v) = ecran.ville else { return };
    let Ok((city, centre)) = cities.get(v) else { return };
    let taille = cadre.size() * cadre.inverse_scale_factor();
    let large = largeur_gauche.size().x * largeur_gauche.inverse_scale_factor();
    let cle = (taille, vue.zoom, state.turn, large);
    let change = ecran.is_changed() || city.is_changed() || amenagements.is_changed() || territoire.is_changed() || gisements.is_changed() || debug.is_changed() || *derniere != cle;
    if taille.x < 10.0 {
        // (écran pas encore mis en page : refaire dès qu'il le sera)
        *en_attente |= change;
        return;
    }
    if !change && !*en_attente {
        return;
    }
    *en_attente = false;
    *derniere = cle;
    // (une reconstruction lente se lit dans la console)
    let _mesure = Mesure(std::time::Instant::now());
    let cases = cases_vues(&map.0, centre.0, v, &city, &territoire, &amenagements, &(&gisements, debug.0));
    let choisie = ecran.case.and_then(|h| cases.iter().find(|c| c.hex == h));
    let achats = cases_a_acheter(&map.0, centre.0, v, &territoire);
    let achat_choisi = ecran.case.and_then(|h| achats.iter().find(|a| a.hex == h)).filter(|_| !ecran.catalogue);

    // Une seule prévision pour l'en-tête, le bandeau et les graphiques (décision B4)
    let vl = Ville::new(&mut cache, &map.0, centre.0, v, &city, &territoire, &amenagements);
    let (dispo, prevision) = (vl.dispo, vl.prevision);
    let diagnostic = frein(&vl, &city, &cases);
    commands.entity(*entete).despawn_related::<Children>().with_children(|e| {
        en_tete(e, &city, &images, ecran.catalogue, prevision);
        bandeau_frein(e, &diagnostic);
    });
    // Graphique de la grandeur détaillée (image refaite ; l'ancienne est libérée)
    if let Some(h) = graphique.take() {
        assets.remove(&h);
    }
    // Aperçus de l'intendant : la répartition refaite avec d'autres consignes
    let apercu = |cons: &intendant::Consignes| -> Rendements {
        let mut st = city.state.clone();
        st.consignes = cons.clone();
        rules::repartition_ville(&map.0, centre.0, &st, dispo).rendements
    };
    let reglages: Vec<(Reglage, Rendements, i64)> = match ecran.grandeur {
        Some(gr) if ecran.onglet == Onglet::Rendements && choisie.is_none() && !ecran.catalogue => {
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
    let courbe = match (ecran.onglet, ecran.grandeur, choisie) {
        (Onglet::Population, _, None) if !ecran.catalogue => {
            let c = courbe_population(&city, &cases, prevision);
            let tr = crate::graphique::Trace::new(&c, Vec2::new((large - 30.0).max(200.0), 230.0));
            let h = assets.add(crate::graphique::dessiner(&c, &tr, charte::TRAIT, charte::TEXTE));
            *graphique = Some(h.clone());
            Some((h, c, tr))
        }
        (Onglet::Rendements, Some(gr), None) => {
            let c = courbe(&city, gr, ecran.periode, prevision);
            let tr = crate::graphique::Trace::new(&c, Vec2::new((large - 30.0).max(200.0), 200.0));
            let h = assets.add(crate::graphique::dessiner(&c, &tr, charte::TRAIT, charte::TEXTE));
            *graphique = Some(h.clone());
            Some((h, c, tr))
        }
        _ => None,
    };
    commands.entity(*gauche).despawn_related::<Children>().with_children(|g| match (ecran.onglet, choisie) {
        _ if ecran.catalogue => {}
        _ if let Some(a) = achat_choisi => {
            // (la culture prendra-t-elle cette case d'elle-même ?)
            let par_la_culture = (rules::case_par_la_culture(&map.0, centre.0, v, &territoire.0) == Some(a.hex)).then(|| city.state.tours_avant_case(city.state.culture_du_tour(city.travail.rendements)));
            fiche_achat(g, a, &city, state.tresor_joueur(), par_la_culture);
        }
        (Onglet::Rendements, c) => {
            cartes(g, &city, &images, ecran.grandeur);
            if ecran.grandeur.is_some() || c.is_some() {
                fil(g, ecran.grandeur, c.map(|c| c.nom.as_str()));
            }
            match (c, ecran.grandeur, &courbe) {
                (Some(c), gr, _) => fiche_case(g, c, &vl, &images, gr, forcage.as_ref()),
                (None, Some(gr), Some((h, cb, tr))) => synthese(g, gr, &city, &cases, &images, h.clone(), cb, tr, ecran.periode, &reglages),
                _ => {
                    ensemble(g, &city, &cases);
                    file_et_projet(g, &city, false, &vl, &cases, ecran.file_choisie);
                }
            }
        }
        (Onglet::Histoire, c) => histoire(g, &city, ecran.histoire_genre, c),
        (_, Some(c)) => fiche_case(g, c, &vl, &images, None, forcage.as_ref()),
        (Onglet::Population, None) => population(g, &city, &cases, &images, courbe.as_ref(), prevision),
        (Onglet::Commerce, None) => vide(g, "Commerce", "Aucun axe de commerce : les échanges entre villes ne sont pas encore dans le jeu."),
    });
    let bareme = Bareme::de(&vl);
    let gains: Vec<Option<(GainAmenagement, Evaluation)>> = cases.iter().map(|c| ecran.placement.and_then(|a| gain_amenagement(&vl, c, a).ok()).map(|g| { let e = evaluation(&g, &bareme); (g, e) })).collect();
    let conseillee = ecran.placement.and_then(|a| conseil(&vl, &cases, a, &bareme));
    // Cases dont un aménagement est déjà en file (pastille « file n° »)
    let en_file = |h: Hex| city.state.file.iter().position(|k| matches!(k.objet, rules::Production::Amenagement { hex, .. } if hex == h));
    let mode = match (ecran.onglet, ecran.grandeur) {
        (Onglet::Histoire, _) => ModeCarte::Histoire,
        _ if let Some(a) = ecran.placement => ModeCarte::Placement(a, gains.iter().flatten().map(|(_, e)| e.valeur).fold(0.01, f64::max)),
        (Onglet::Population, _) if ecran.carte_habitants => {
            let v: Vec<f64> = cases.iter().map(|c| c.habitants).filter(|v| *v > 0.5).collect();
            ModeCarte::Habitants(v.iter().copied().fold(f64::MAX, f64::min).min(1e9), v.iter().copied().fold(1.0, f64::max))
        }
        (Onglet::Population, _) => ModeCarte::Remplissage,
        (Onglet::Rendements, Some(g)) => ModeCarte::Grandeur(g, cases.iter().map(|c| valeur(&c.rendement, g)).fold(0.01, f64::max)),
        _ => ModeCarte::Dominante,
    };
    commands.entity(*legende).despawn_related::<Children>().with_children(|l| legende_carte(l, mode, ecran.amenagements_caches, &bareme));
    let s = rayon_hex(taille, &cases, &achats, vue.zoom);
    // Valider là où l'on regarde : une barre en bas de la carte, qui nomme la case visée
    let (barre_e, mut barre_node) = barre.into_inner();
    let visee = ecran.placement.zip(ecran.cible.or(conseillee)).and_then(|(a, h)| cases.iter().find(|c| c.hex == h).map(|c| (a, c)));
    barre_node.display = if visee.is_some() && ecran.catalogue { Display::Flex } else { Display::None };
    commands.entity(barre_e).despawn_related::<Children>().with_children(|b| {
        if let Some((a, c)) = visee {
            let e = gain_amenagement(&vl, c, a).ok().map(|g| evaluation(&g, &bareme));
            let quelle = format!("{}{}", if ecran.cible.is_some() { "case choisie" } else { "conseil de l'intendant" }, e.map_or(String::new(), |e| format!(", valeur {}", signe(e.valeur))));
            b.spawn((MettreEnFile, Pulsation, bouton(charte::ACTION_FOND), children![texte(format!("Mettre en file : {} sur {} ({quelle}) · Entrée", a.nom(), c.nom.to_lowercase()), 14.0, charte::TEXTE)]));
        }
    });
    commands.entity(*plan).despawn_related::<Children>().with_children(|p| {
        // Cases voisines à acheter : pâles, en pointillé, avec leur prix (pas pendant la construction)
        if !ecran.catalogue {
            for a in &achats {
                hexagone_achat(p, a, s, &images, a.prix <= state.tresor_joueur(), Some(a.hex) == ecran.case);
            }
        }
        for (c, g) in cases.iter().zip(&gains) {
            hexagone(p, c, s, mode, !ecran.amenagements_caches, &images, g.as_ref(), en_file(c.hex));
        }
        let anneau = |p: &mut ChildSpawnerCommands, h: Hex, couleur: Color| {
            if let Some(c) = cases.iter().find(|c| c.hex == h) {
                let pos = position(c.local, s);
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(pos.x - s * RACINE_3 / 2.0), top: Val::Px(pos.y - s), width: Val::Px(s * RACINE_3), height: Val::Px(2.0 * s), ..default() },
                    ImageNode::new(images.contour.clone()).with_color(couleur),
                ));
            }
        };
        // Placement : risques de famine cerclés de rouge, conseil cerclé d'or (il pulse)
        for (c, g) in cases.iter().zip(&gains) {
            if let Some((_, e)) = g
                && matches!(e.risque, Risque::Famine(_) | Risque::BordDeFamine(_))
            {
                anneau(p, c.hex, charte::GRAVE);
            }
        }
        if let Some(h) = conseillee
            && let Some(c) = cases.iter().find(|c| c.hex == h)
        {
            let pos = position(c.local, s);
            p.spawn((
                Pulsation,
                Node { position_type: PositionType::Absolute, left: Val::Px(pos.x - s * RACINE_3 / 2.0), top: Val::Px(pos.y - s), width: Val::Px(s * RACINE_3), height: Val::Px(2.0 * s), ..default() },
                ImageNode::new(images.contour.clone()).with_color(CONSEIL),
            ));
        }
        if ecran.onglet == Onglet::Histoire {
            let mut lieux: Vec<Hex> = city.state.chronique.iter().filter_map(|e| e.lieu).collect();
            lieux.sort();
            lieux.dedup();
            for h in lieux {
                anneau(p, h, CONSEIL);
            }
        }
        if let Some(h) = ecran.cible {
            anneau(p, h, charte::SELECTION);
        }
        let file_visible = ecran.onglet == Onglet::Rendements && ecran.case.is_none() && if ecran.catalogue { ecran.placement.is_none() } else { ecran.grandeur.is_none() };
        if let Some(h) = ecran.file_choisie.filter(|_| file_visible).and_then(|i| city.state.file.get(i)).and_then(|k| case_de(&k.objet)) {
            anneau(p, map.0.canonical(h), Grandeur::Production.couleur());
        }

        if let Some(c) = choisie {
            let pos = position(c.local, s);
            p.spawn((
                Node { position_type: PositionType::Absolute, left: Val::Px(pos.x - s * RACINE_3 / 2.0), top: Val::Px(pos.y - s), width: Val::Px(s * RACINE_3), height: Val::Px(2.0 * s), ..default() },
                ImageNode::new(images.contour.clone()).with_color(charte::SELECTION),
            ));
        }
        p.spawn((ContourSurvol, Node { position_type: PositionType::Absolute, display: Display::None, ..default() }, ImageNode::new(images.contour.clone())));
    });
    // (le catalogue et le placement prennent la place de la colonne de gauche et de la liste)
    if ecran.catalogue {
        let cible = ecran.placement.zip(ecran.cible.or(conseillee)).and_then(|(a, h)| cases.iter().find(|c| c.hex == h).map(|c| (a, c)));
        commands.entity(*gauche).despawn_related::<Children>().with_children(|g| {
            cartes(g, &city, &images, None);
            match cible {
                Some((a, c)) => placement(g, &vl, &city, &cases, a, c, ecran.cible.is_none()),
                None if ecran.placement.is_some_and(|a| cases.iter().any(|c| possible(&map.0, c, a).is_ok())) => {
                    vide(g, "Placement", &format!("Aucune case n'y gagne dans {TOURS_EFFET} tours : pas de conseil. Cliquez une case pour voir sa fiche et la choisir quand même."))
                }
                None if ecran.placement.is_some() => vide(g, "Placement", "Aucune case de la ville ne peut porter cet aménagement."),
                None => file_et_projet(g, &city, true, &vl, &cases, ecran.file_choisie),
            }
        });
        if let Ok(mut t) = textes.get_mut(*titre) {
            t.0 = "Construire".into();
        }
        commands.entity(*liste).despawn_related::<Children>().with_children(|l| {
            l.spawn((Node { column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(6.0)), flex_wrap: FlexWrap::Wrap, ..default() }, BackgroundColor(Color::srgb(0.17, 0.15, 0.07))))
                .with_children(|b| {
                    b.spawn(texte(format!("Ce qui freine {} :", city.state.name), 13.0, CONSEIL));
                    b.spawn(texte(diagnostic.texte.clone(), 13.0, CONSEIL));
                });
            catalogue(l, &vl, &city, &cases, ecran.placement, ecran.catalogue_groupe);
        });
        return;
    }
    let titre_liste: String = match (ecran.onglet, ecran.grandeur) {
        (Onglet::Rendements, Some(g)) => format!("Cases · {}", g.nom().to_lowercase()),
        (Onglet::Rendements, None) => format!("Cases travaillées ({})", cases.iter().filter(|c| !c.centre && c.travailleurs > 0).count()),
        (Onglet::Population, _) if ecran.carte_habitants => "Cases par habitants".into(),
        (Onglet::Population, _) => "Cases par remplissage".into(),
        (Onglet::Commerce, _) => "Axes de commerce".into(),
        (Onglet::Histoire, _) => "Lieux de l'histoire".into(),
    };
    if let Ok(mut t) = textes.get_mut(*titre) {
        t.0 = titre_liste;
    }
    let gr = if ecran.onglet == Onglet::Rendements { ecran.grandeur } else { None };
    commands.entity(*liste).despawn_related::<Children>().with_children(|l| liste_droite(l, ecran.onglet, &cases, ecran.case, gr, ecran.carte_habitants, &city.state.chronique));
}

fn melange(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t)
}

/// Un hexagone de la carte de la ville.
#[allow(clippy::too_many_arguments)]
fn hexagone(p: &mut ChildSpawnerCommands, c: &CaseVue, s: f32, mode: ModeCarte, amenagements: bool, images: &Images, gain: Option<&(GainAmenagement, Evaluation)>, en_file: Option<usize>) {
    let pos = position(c.local, s);
    let mode_pop = mode == ModeCarte::Remplissage;
    let fond = match mode {
        ModeCarte::Placement(_, max) => match gain {
            Some((_, e)) if matches!(e.risque, Risque::Famine(_)) => melange(charte::SURFACE, charte::GRAVE, 0.3),
            Some((_, e)) => melange(charte::SURFACE, charte::AMENAGEMENT, 0.12 + 0.5 * (e.valeur.max(0.0) / max) as f32),
            None => Color::srgb(0.06, 0.075, 0.11),
        },
        ModeCarte::Grandeur(g, max) => melange(charte::SURFACE, g.couleur(), 0.08 + 0.6 * (valeur(&c.rendement, g) / max) as f32),
        ModeCarte::Habitants(lo, hi) => {
            if c.habitants > 0.5 {
                rampe(if hi > lo { ((c.habitants.ln() - lo.ln()) / (hi.ln() - lo.ln())) as f32 } else { 1.0 })
            } else {
                Color::srgb(0.086, 0.106, 0.145)
            }
        }
        _ if c.centre => charte::CENTRE_VILLE,
        ModeCarte::Remplissage => {
            let part = c.remplissage();
            melange(charte::SURFACE, if part >= 0.9 { charte::ALERTE } else { charte::OK }, 0.15 + 0.45 * part)
        }
        ModeCarte::Dominante | ModeCarte::Histoire => match c.dominante() {
            Some(g) => melange(charte::SURFACE, g.couleur(), if c.travailleurs > 0 { 0.45 } else { 0.18 }),
            None => charte::SURFACE,
        },
    };
    let grand = s >= 30.0;
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(pos.x - s * RACINE_3 / 2.0),
            top: Val::Px(pos.y - s),
            width: Val::Px(s * RACINE_3),
            height: Val::Px(2.0 * s),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        ImageNode::new(images.hexagone.clone()).with_color(fond),
    ))
    .with_children(|h| {
        // (le contour sombre sépare les cases)
        h.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode::new(images.contour.clone()).with_color(Color::srgba(0.04, 0.05, 0.08, 0.9))));
        let petit = (s * 0.26).clamp(9.0, 14.0);
        let centre = |b: (Text, TextFont, TextColor)| (b, TextLayout::justify(Justify::Center), Node { max_width: Val::Px(s * 1.45), ..default() });
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
                    // La valeur (en grand), puis ce qui la compose (vert : gain, rouge : perte)
                    h.spawn(centre(texte(signe(e.valeur), (s * 0.36).clamp(11.0, 19.0), couleur_signe(e.valeur))));
                    if s >= 30.0 {
                        h.spawn(Node { column_gap: Val::Px(4.0), flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, max_width: Val::Px(s * 1.5), ..default() }).with_children(|r| {
                            for (gr, court) in [(Grandeur::Nourriture, "nourr."), (Grandeur::Production, "prod."), (Grandeur::Deniers, "den.")] {
                                let v = valeur(&g.dans, gr);
                                if v.abs() >= 0.05 {
                                    r.spawn(texte(format!("{} {court}", signe(v)), (s * 0.22).clamp(9.0, 12.0), if v > 0.0 { gr.couleur() } else { charte::GRAVE }));
                                }
                            }
                        });
                        let note = match e.risque {
                            Risque::Famine(_) => Some(("famine !", charte::GRAVE)),
                            Risque::BordDeFamine(_) => Some(("bord de famine", charte::GRAVE)),
                            Risque::Aucun if g.defriche => Some(("défrichement", charte::ALERTE)),
                            Risque::Aucun => c.amenagement.map(|_| ("remplace l'actuel", charte::ALERTE)),
                        };
                        if let Some((t, couleur)) = note {
                            h.spawn(centre(texte(t, (s * 0.22).clamp(9.0, 12.0), couleur)));
                        }
                    }
                    if g.defriche {
                        let z = (s * 0.45).clamp(14.0, 26.0);
                        h.spawn((Node { position_type: PositionType::Absolute, top: Val::Px(s * 0.22), right: Val::Px(s * 0.22), ..default() }, children![icone(images.hache.clone(), z, charte::ALERTE)]));
                    }
                }
                None => {
                    h.spawn(centre(texte("—", (s * 0.3).clamp(10.0, 16.0), charte::MUET)));
                }
            }
        } else if let (ModeCarte::Grandeur(g, _), true) = (mode, s >= 22.0) {
            h.spawn(centre(texte(rules::decimal(valeur(&c.rendement, g)), (s * 0.34).clamp(11.0, 18.0), charte::TEXTE)));
            if grand {
                h.spawn(centre(texte(if c.centre { "centre".to_string() } else { c.nom.clone() }, petit, charte::DOUX)));
            }
        } else if grand {
            h.spawn(centre(texte(if c.centre { "centre".to_string() } else { c.nom.clone() }, petit, charte::DOUX)));
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
        if amenagements && let Some(a) = c.amenagement {
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
fn hexagone_achat(p: &mut ChildSpawnerCommands, a: &CaseAchat, s: f32, images: &Images, abordable: bool, choisie: bool) {
    let pos = position(a.local, s);
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(pos.x - s * RACINE_3 / 2.0),
            top: Val::Px(pos.y - s),
            width: Val::Px(s * RACINE_3),
            height: Val::Px(2.0 * s),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        ImageNode::new(images.hexagone.clone()).with_color(Color::srgba(0.10, 0.125, 0.19, 0.4)),
    ))
    .with_children(|h| {
        let trait_ = if choisie { charte::SELECTION } else { Color::srgba(0.69, 0.72, 0.78, 0.55) };
        h.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode::new(images.pointille.clone()).with_color(trait_)));
        if s >= 22.0 {
            let couleur = if abordable { Grandeur::Deniers.couleur() } else { charte::MUET };
            let prix = if s >= 30.0 { format!("{} deniers", a.prix) } else { a.prix.to_string() };
            h.spawn((texte(prix, (s * 0.24).clamp(9.0, 13.0), couleur), TextLayout::justify(Justify::Center)));
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
    if matches!(mode, ModeCarte::Remplissage | ModeCarte::Habitants(..)) {
        // (onglet Population : deux cartes)
        let habitants = matches!(mode, ModeCarte::Habitants(..));
        for (v, t) in [(false, "Emplois"), (true, "Répartition de la population")] {
            let actif = v == habitants;
            l.spawn((
                ChoixCartePop(v),
                UiPointer::default(),
                Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                BackgroundColor(if actif { charte::TEXTE } else { Color::NONE }),
                BorderColor::all(charte::TRAIT2),
                children![texte(t, 12.5, if actif { charte::FOND } else { charte::DOUX })],
            ));
        }
    }
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
        l.spawn(texte(format!("chiffres : valeur (effet réel dans {TOURS_EFFET} tours : nourriture utile, jusqu'à {} × le besoin, + production + deniers), puis ce qui la compose", rules::decimal(bareme.plafond)), 12.0, charte::MUET));
        return;
    }
    if let ModeCarte::Grandeur(g, _) = mode {
        pastille(l, melange(charte::SURFACE, g.couleur(), 0.15), "faible");
        pastille(l, melange(charte::SURFACE, g.couleur(), 0.68), "fort");
        l.spawn(texte(format!("chiffres : {} par case (centre compris)", g.nom().to_lowercase()), 12.5, charte::MUET));
    } else if mode == ModeCarte::Remplissage {
        pastille(l, melange(charte::SURFACE, charte::OK, 0.5), "de la place");
        pastille(l, melange(charte::SURFACE, charte::ALERTE, 0.6), "presque pleine (90 % et plus)");
        pastille(l, charte::CENTRE_VILLE, "centre de la ville");
        l.spawn(texte("chiffres : travailleurs / capacité", 12.5, charte::MUET));
    } else {
        for g in [Grandeur::Nourriture, Grandeur::Production, Grandeur::Deniers] {
            pastille(l, melange(charte::SURFACE, g.couleur(), 0.5), &g.nom().to_lowercase());
        }
        pastille(l, charte::CENTRE_VILLE, "centre");
        l.spawn(texte("couleur : grandeur principale de la case (pâle : personne n'y travaille)", 12.5, charte::MUET));
    }
    l.spawn((
        BasculeAmenagements,
        UiPointer::default(),
        Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(2.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(12.0)), column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() },
        BackgroundColor(Color::NONE),
        BorderColor::all(charte::TRAIT2),
        children![
            (Node { width: Val::Px(12.0), height: Val::Px(12.0), border: UiRect::all(Val::Px(1.5)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(charte::AMENAGEMENT_FOND), BorderColor::all(charte::AMENAGEMENT)),
            // (« Aménagements » seul se confondait avec l'onglet du catalogue)
            texte("Voir les aménagements", 13.0, if caches { charte::MUET } else { charte::TEXTE }),
        ],
    ));
}

/// En-tête : nom, palier, habitants, croissance, construction en cours, bouton Fermer.
fn en_tete(e: &mut ChildSpawnerCommands, city: &City, images: &Images, catalogue: bool, prevision: &[rules::Pas]) {
    let c = &city.state;
    let y = city.travail.rendements;
    let variation = prevision.first().map_or_else(|| c.variation(y), |p| p.variation);
    e.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(18.0), ..default() }).with_children(|l| {
        l.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).with_children(|n| {
            n.spawn(texte(c.name.clone(), 26.0, charte::TEXTE));
            n.spawn(Node { column_gap: Val::Px(14.0), align_items: AlignItems::Center, ..default() }).with_children(|m| {
                m.spawn(icone(images.grandeur(Grandeur::Habitants), 16.0, Grandeur::Habitants.couleur()));
                m.spawn(texte(format!("palier {} · {} habitants", c.palier(), rules::milliers(c.habitants)), 14.0, charte::DOUX));
                let (t, couleur) = if variation > 0 {
                    let suivant = rules::habitants_du_palier(c.palier() + 1);
                    let (quand, couleur) = match rules::tours_jusqu_a(prevision, suivant) {
                        Some(t) => (format!("palier {} dans {t} tour{}", c.palier() + 1, if t > 1 { "s" } else { "" }), charte::OK),
                        // (même couleur que dans le bandeau : il faut agir)
                        None if rules::arret_de_croissance(prevision).is_some() => (format!("palier {} hors de portée (nourriture)", c.palier() + 1), charte::ALERTE),
                        None => (format!("palier {} dans plus de {} tours", c.palier() + 1, prevision.len()), charte::OK),
                    };
                    (format!("+{} par tour · {quand}", rules::milliers(variation as u64)), couleur)
                } else if variation == 0 {
                    ("population stable".into(), charte::ALERTE)
                } else {
                    (format!("−{} par tour : famine, la population baisse", rules::milliers(variation.unsigned_abs())), charte::GRAVE)
                };
                m.spawn(texte(t, 14.0, couleur));
            });
        });
        l.spawn(Node { flex_grow: 1.0, ..default() });
        // Construction en cours
        let en_cours = en_cours(c, y.production);
        l.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|b| {
            b.spawn(icone(images.grandeur(Grandeur::Production), 18.0, Grandeur::Production.couleur()));
            b.spawn(texte(en_cours, 14.0, charte::TEXTE));
        });
        if catalogue {
            l.spawn((BoutonConstruire, bouton(charte::SURFACE2), children![texte("Fermer la construction (B)", 14.0, charte::TEXTE)])).insert(BorderColor::all(charte::SELECTION));
        } else {
            l.spawn((BoutonConstruire, bouton(charte::ACTION_FOND), children![texte("Construire (B)", 14.0, charte::TEXTE)]));
        }
        l.spawn((Fermer, bouton(charte::SURFACE), children![texte("Fermer (Échap)", 14.0, charte::TEXTE)]));
    });
}

/// Ce que fait la ville de sa production, pour l'en-tête.
fn en_cours(c: &rules::CityState, production: f64) -> String {
    match (c.file.first(), c.projet) {
        (Some(k), _) => {
            let suite = if c.file.len() > 1 { format!(" · puis {} en file", c.file.len() - 1) } else { String::new() };
            format!("En construction : {} · {}{suite}", capitale(k.objet.name()), delai(c, production))
        }
        (None, Some(p)) if c.tours_en_projet == 0 => format!("Projet : {}", p.nom()),
        (None, Some(p)) => format!("Projet : {} (depuis {} tour{})", p.nom(), c.tours_en_projet, if c.tours_en_projet > 1 { "s" } else { "" }),
        (None, None) => "Rien à produire : à choisir pour finir le tour".into(),
    }
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

/// Sous l'en-tête : ce qui freine la croissance, ses remèdes, les autres alertes.
fn bandeau_frein(e: &mut ChildSpawnerCommands, f: &Frein) {
    e.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() }, BackgroundColor(charte::SURFACE))).with_children(|b| {
        b.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|l| {
            l.spawn((Node { width: Val::Px(9.0), height: Val::Px(9.0), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(f.etat.couleur())));
            l.spawn(texte("Ce qui freine la croissance :", 13.5, charte::TEXTE));
            l.spawn(texte(f.texte.clone(), 13.5, f.etat.couleur()));
        });
        // (ligne toujours là : les onglets dessous ne bougent pas quand des remèdes apparaissent)
        {
            b.spawn(Node { column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, min_height: Val::Px(26.0), ..default() }).with_children(|l| {
                if !f.remedes.is_empty() {
                    l.spawn(texte("Remèdes :", 12.5, charte::MUET));
                }
                for (r, t) in &f.remedes {
                    l.spawn((BoutonRemede(*r), bouton(Color::NONE), children![texte(t.clone(), 12.5, charte::AMENAGEMENT)]));
                }
                for (etat, t, r) in &f.alertes {
                    l.spawn((BoutonRemede(*r), bouton(Color::NONE), children![texte(t.clone(), 12.5, etat.couleur())]));
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

/// Carte d'une grandeur : icône, nom, valeur, état.
#[allow(clippy::too_many_arguments)]
fn carte_grandeur(r: &mut ChildSpawnerCommands, images: &Images, g: Grandeur, valeur: String, unite: &str, etat: Etat, ligne: String, choisie: bool) {
    let mut carte = r.spawn((
        UiPointer::default(),
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(9.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() },
        BackgroundColor(charte::SURFACE),
        BorderColor::all(if choisie { charte::SELECTION } else { Color::NONE }),
    ));
    // (seules les grandeurs détaillées s'ouvrent au clic)
    if DETAILLABLES.contains(&g) {
        carte.insert(ChoixGrandeur(g));
    }
    carte
        .with_children(|c| {
            c.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|t| {
                t.spawn(icone(images.grandeur(g), 18.0, g.couleur()));
                t.spawn(texte(g.nom(), 13.5, g.couleur()));
            });
            c.spawn(Node { column_gap: Val::Px(4.0), align_items: AlignItems::Baseline, ..default() }).with_children(|v| {
                v.spawn(texte(valeur, 24.0, charte::TEXTE));
                v.spawn(texte(unite, 13.0, charte::MUET));
            });
            c.spawn(texte(ligne, 12.5, etat.couleur()));
        });
}

/// Onglet Rendements, en haut : les trois grandeurs principales (cliquables : leur détail), et
/// celles qui ne sont pas encore dans le jeu.
fn cartes(g: &mut ChildSpawnerCommands, city: &City, images: &Images, choisie: Option<Grandeur>) {
    let c = &city.state;
    let y = city.travail.rendements;
    let couverture = y.nourriture / c.besoin_nourriture().max(0.001);
    let etat_n = if couverture < 1.0 { Etat::Grave } else if couverture < 1.5 { Etat::Alerte } else { Etat::Ok };
    let suite_n = if couverture < 1.0 { " : famine".to_string() } else { format!(" · {} types sur 3", intendant::types_comptes(city.travail.par_type, c.besoin_nourriture())) };
    let prod = match (c.file.first(), c.projet) {
        (Some(k), _) => format!("{} : {}", capitale(k.objet.name()), delai(c, y.production)),
        (None, Some(p)) => format!("projet : {}", p.nom().to_lowercase()),
        (None, None) => "rien à produire : à choisir".into(),
    };
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|r| {
            carte_grandeur(r, images, Grandeur::Nourriture, rules::decimal(y.nourriture), "par tour", etat_n, format!("{} × le besoin{suite_n}", rules::decimal(couverture)), choisie == Some(Grandeur::Nourriture));
            let etat_p = if c.file.is_empty() && c.projet.is_none() { Etat::Alerte } else { Etat::Ok };
            carte_grandeur(r, images, Grandeur::Production, rules::decimal(y.production), "par tour", etat_p, prod, choisie == Some(Grandeur::Production));
            carte_grandeur(r, images, Grandeur::Deniers, format!("+{}", rules::decimal(y.deniers)), "par tour", Etat::Ok, "versés au trésor".into(), choisie == Some(Grandeur::Deniers));
            // Culture (4d) : son stock fait gagner une case au seuil
            let par_tour = c.culture_du_tour(y);
            let seuil = rules::seuil_culture(c.cases_gagnees);
            let quand = c.tours_avant_case(par_tour).map_or(String::new(), |t| if t <= 1 { " · une case au prochain tour".into() } else { format!(" · une case dans {t} tours") });
            carte_grandeur(r, images, Grandeur::Culture, format!("+{}", rules::decimal(par_tour)), "par tour", Etat::Ok, format!("{} / {}{quand}", rules::decimal(c.culture), rules::decimal(seuil)), false);
        });
        // Bien-être (en mots), salubrité quand elle est basse ; les autres grandeurs n'existent pas encore
        p.spawn(Node { column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, ..default() }).with_children(|r| {
            let puce = |r: &mut ChildSpawnerCommands, g: Grandeur, nom: &str, valeur: String, couleur: Color| {
                r.spawn((Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(11.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(14.0)), ..default() }, BackgroundColor(charte::SURFACE))).with_children(|t| {
                    t.spawn(icone(images.grandeur(g), 16.0, g.couleur()));
                    t.spawn(texte(nom.to_string(), 13.0, charte::DOUX));
                    t.spawn(texte(valeur, 13.0, couleur));
                });
            };
            let be = c.bien_etre;
            let vise = c.bien_etre_vise(y, city.travail.chomeurs);
            let couleur = match c.humeur() {
                rules::Humeur::Mecontente => charte::GRAVE,
                rules::Humeur::Satisfaite => charte::TEXTE,
                rules::Humeur::Prospere => charte::OK,
            };
            let tendance = if vise > be + 1.0 { format!(" ↗ {vise:.0}") } else if vise < be - 1.0 { format!(" ↘ {vise:.0}") } else { String::new() };
            puce(r, Grandeur::BienEtre, "Bien-être", format!("{be:.0} · {}{tendance}", c.humeur().nom()), couleur);
            let s = c.salubrite();
            if s < rules::SALUBRITE_SEUIL {
                puce(r, Grandeur::BienEtre, "Salubrité", format!("{s:.0} : maladies"), charte::GRAVE);
            }
            for g in [Grandeur::Eau, Grandeur::Recherche, Grandeur::Defense] {
                r.spawn((Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(11.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(14.0)), ..default() }, BackgroundColor(charte::SURFACE))).with_children(|t| {
                    t.spawn(icone(images.grandeur(g), 16.0, g.couleur()));
                    t.spawn(texte(g.nom(), 13.0, charte::DOUX));
                    t.spawn(texte("—", 13.0, charte::MUET));
                });
            }
            r.spawn(texte("pas encore dans le jeu", 12.5, charte::MUET));
        });
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

/// Indicateur (non cliquable) : titre, valeur, ligne.
fn indicateur(r: &mut ChildSpawnerCommands, titre: &str, valeur: String, ligne: String, couleur: Color) {
    r.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(9.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() }, BorderColor::all(charte::TRAIT)))
        .with_children(|c| {
            c.spawn(texte(titre, 13.0, charte::MUET));
            c.spawn(texte(valeur, 22.0, charte::TEXTE));
            c.spawn(texte(ligne, 12.5, couleur));
        });
}

/// Onglet Population : indicateurs, évolution et limites, mouvement, cultures et ordres.
fn population(g: &mut ChildSpawnerCommands, city: &City, cases: &[CaseVue], images: &Images, graphique: Option<&(Handle<Image>, crate::graphique::Courbe, crate::graphique::Trace)>, prevision: &[rules::Pas]) {
    let c = &city.state;
    let t = &city.travail;
    let employes = t.actifs - t.chomeurs;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
            r.spawn(icone(images.grandeur(Grandeur::Habitants), 20.0, Grandeur::Habitants.couleur()));
            titre(r, &format!("Population de {}", c.name));
        });
        p.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|r| {
            indicateur(r, "Habitants de l'aire", rules::milliers(c.habitants), format!("palier {} · le suivant à {}", c.palier(), rules::milliers(rules::habitants_du_palier(c.palier() + 1))), charte::DOUX);
            indicateur(r, "Actifs (la moitié)", rules::milliers(t.actifs.into()), format!("{} au travail sur les cases", rules::milliers(employes.into())), charte::DOUX);
            let (l, e) = if t.chomeurs > 0 { ("aménager des cases donne des emplois".to_string(), charte::DOUX) } else { ("tous employés".to_string(), charte::OK) };
            indicateur(r, "Sans emploi", rules::milliers(t.chomeurs.into()), l, e);
            indicateur(r, "Taux d'urbanisation", "—".into(), "pas encore de quartiers urbains".into(), charte::MUET);
        });
        p.spawn(texte("Pour l'instant, les habitants vivent là où ils travaillent.", 12.5, charte::MUET));
    });
    // Évolution et limites
    let (nourriture, travail) = (limite_nourriture(cases, c), emplois(cases));
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, "Évolution et limites de la ville");
        p.spawn(Node { column_gap: Val::Px(4.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|l| {
            l.spawn(texte("Échelle logarithmique. Trait plein : le passé ; pointillé : la prévision (l'intendant répartit les habitants à venir). La", 12.5, charte::MUET));
            l.spawn(texte("nourriture", 12.5, Grandeur::Nourriture.couleur()));
            l.spawn(texte("de toutes les cases pleines borne la population ; les", 12.5, charte::MUET));
            l.spawn(texte("emplois", 12.5, Grandeur::Production.couleur()));
            l.spawn(texte("ne sont pas une limite.", 12.5, charte::MUET));
        });
        if let Some((h, cb, tr)) = graphique {
            poser_graphique(p, h.clone(), cb, tr);
        }
        let ligne = |p: &mut ChildSpawnerCommands, nom: &str, couleur: Color, valeur: String, note: String, note_couleur: Color| {
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(nom, 13.5, couleur)]));
                l.spawn(texte(valeur, 13.5, charte::TEXTE));
                l.spawn((Node { width: Val::Px(230.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(note, 12.5, note_couleur)]));
            });
        };
        let h = c.habitants as f64;
        let (note, couleur) = match rules::arret_de_croissance(prevision) {
            _ if nourriture <= h => ("dépassée : famine".to_string(), charte::GRAVE),
            Some(t) => (format!("croissance arrêtée dans ~{t} tours vers {}", rules::milliers(prevision[t - 1].habitants)), charte::MUET),
            None => (format!("marge {}", rules::milliers((nourriture - h) as u64)), charte::MUET),
        };
        ligne(p, "Limite : la nourriture (cases pleines)", Grandeur::Nourriture.couleur(), format!("{} habitants", rules::milliers(nourriture as u64)), note, couleur);
        let note = if city.travail.chomeurs > 0 {
            format!("{} sans emploi aujourd'hui", rules::milliers(city.travail.chomeurs.into()))
        } else {
            match prevision.iter().position(|p| p.chomeurs > 0) {
                Some(t) => format!("plus aucun emploi libre dans ~{} tours", t + 1),
                None => format!("emplois libres pour {} tours au moins", prevision.len()),
            }
        };
        ligne(p, "Emplois (cases pleines)", Grandeur::Production.couleur(), format!("{} habitants", rules::milliers(travail as u64)), note, charte::MUET);
        p.spawn(texte("Au-delà des emplois, la ville grandit encore, mais le chômage baissera son attractivité (migrations : pas encore dans le jeu).", 12.5, charte::MUET));
        ligne_valeur(p, "Eau", "pas encore dans le jeu".into(), charte::MUET);
    });
    // Bien-être et salubrité (4c) : ce qui les fait, ce qu'ils font à la croissance
    bien_etre_et_salubrite(g, city, images);
    // Mouvement de la population
    let dernier = c.historique.iter().rev().take(2).map(|r| r.habitants as i64).collect::<Vec<_>>();
    let solde = if dernier.len() == 2 { Some(dernier[0] - dernier[1]) } else { None };
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, "Mouvement de la population");
        ligne_valeur(p, "Solde du dernier tour", solde.map_or("—".into(), |v| format!("{}{} habitants", if v >= 0 { "+" } else { "−" }, rules::milliers(v.unsigned_abs()))), if solde.is_some_and(|v| v < 0) { charte::GRAVE } else { charte::TEXTE });
        ligne_valeur(p, "Prochain tour (prévision)", { let v = prevision.first().map_or_else(|| c.variation(t.rendements), |p| p.variation); format!("{}{} habitants", if v >= 0 { "+" } else { "−" }, rules::milliers(v.unsigned_abs())) }, charte::TEXTE);
        p.spawn(texte("Naissances, décès et migrations : pas encore distingués dans le jeu (seul le solde compte).", 12.5, charte::MUET));
    });
    vide(g, "Cultures et religions", "Une seule culture pour l'instant : les cultures et les religions ne sont pas encore dans le jeu.");
    vide(g, "Ordres de la société", "Paysans, artisans, marchands, prêtres, guerriers : pas encore dans le jeu.");
}

/// Pourcentage signé : « +1,5 % », « −0,2 % ».
fn pourcent(x: f64) -> String {
    let v = x * 100.0;
    if v.abs() < 0.005 { "0 %".into() } else { format!("{}{} %", if v > 0.0 { "+" } else { "−" }, rules::decimal(v.abs())) }
}

/// Termes d'une grandeur : « base 60 · famine −15 · chômage −4 ».
fn termes(base: f64, t: &[(&str, f64)]) -> String {
    std::iter::once(format!("base {base:.0}")).chain(t.iter().map(|(n, v)| format!("{n} {}{:.0}", if *v >= 0.0 { "+" } else { "−" }, v.abs()))).collect::<Vec<_>>().join(" · ")
}

/// Panneau « Bien-être et salubrité » de l'onglet Population : valeurs, causes, effet sur la croissance.
fn bien_etre_et_salubrite(g: &mut ChildSpawnerCommands, city: &City, images: &Images) {
    let c = &city.state;
    let y = city.travail.rendements;
    let cr = c.croissance(y);
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
            r.spawn(icone(images.grandeur(Grandeur::BienEtre), 20.0, Grandeur::BienEtre.couleur()));
            titre(r, "Bien-être et salubrité");
        });
        let vise = c.bien_etre_vise(y, city.travail.chomeurs);
        let couleur = if c.humeur() == rules::Humeur::Mecontente { charte::GRAVE } else { charte::TEXTE };
        ligne_valeur(p, "Bien-être", format!("{:.0} sur 100 · {} (visé : {vise:.0})", c.bien_etre, c.humeur().nom()), couleur);
        p.spawn(texte(format!("Visé : {} · le luxe viendra avec les ressources. Le bien-être s'en approche d'un cinquième de l'écart par tour.", termes(rules::BIEN_ETRE_NEUTRE, &c.termes_bien_etre(y, city.travail.chomeurs))), 12.5, charte::MUET));
        let s = c.salubrite();
        ligne_valeur(p, "Salubrité", format!("{s:.0} sur 100{}", if s < rules::SALUBRITE_SEUIL { " : les maladies emportent des habitants" } else { "" }), if s < rules::SALUBRITE_SEUIL { charte::GRAVE } else { charte::TEXTE });
        p.spawn(texte(format!("{} · sous {:.0}, des habitants meurent de maladies (jusqu'à 1 % par tour à 0).", termes(rules::SALUBRITE_BASE, &c.termes_salubrite()), rules::SALUBRITE_SEUIL), 12.5, charte::MUET));
        // La croissance du prochain tour, cause par cause
        let mut parts = Vec::new();
        if cr.famine < 0.0 {
            parts.push(format!("famine {}", pourcent(cr.famine)));
        } else {
            parts.push(format!("naturelle {} (× {} par le bien-être)", pourcent(cr.naturelle), rules::decimal(cr.facteur_bien_etre)));
        }
        if cr.peupler > 0.0 {
            parts.push(format!("Peupler {}", pourcent(cr.peupler)));
        }
        if cr.insalubrite < 0.0 {
            parts.push(format!("maladies {}", pourcent(cr.insalubrite)));
        }
        ligne_valeur(p, "Croissance du prochain tour", pourcent(cr.total()), if cr.total() < 0.0 { charte::GRAVE } else { charte::TEXTE });
        p.spawn(texte(parts.join(" · "), 12.5, charte::MUET));
    });
}

/// Onglet Histoire : les moments marquants de la ville, triés par tour, filtrés par genre et par lieu.
fn histoire(g: &mut ChildSpawnerCommands, city: &City, genre: Option<rules::GenreEvenement>, lieu: Option<&CaseVue>) {
    let c = &city.state;
    let mut liste: Vec<&rules::Evenement> = c.chronique.iter().filter(|e| genre.is_none_or(|x| e.genre == x)).filter(|e| lieu.is_none_or(|l| e.lieu == Some(l.hex))).collect();
    liste.sort_by_key(|e| e.tour);
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, &format!("Histoire de {}", c.name));
        p.spawn(texte("Les moments marquants de la ville ; ceux qui ont un lieu sont cerclés d'or sur la carte.", 12.5, charte::MUET));
        p.spawn(Node { column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|f| {
            for (x, t) in std::iter::once((None, "Tout")).chain(rules::GenreEvenement::TOUS.into_iter().map(|x| (Some(x), x.nom()))) {
                let actif = genre == x;
                f.spawn((
                    ChoixGenre(x),
                    UiPointer::default(),
                    Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                    BackgroundColor(if actif { charte::TEXTE } else { Color::NONE }),
                    BorderColor::all(charte::TRAIT2),
                    children![texte(t, 12.5, if actif { charte::FOND } else { charte::DOUX })],
                ));
            }
        });
        if let Some(l) = lieu {
            p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                r.spawn(texte(format!("Lieu : {}", l.nom), 13.5, charte::TEXTE));
                r.spawn((FermerFiche, bouton(Color::NONE), children![texte("× tous les lieux", 12.5, charte::DOUX)]));
            });
        }
        if liste.is_empty() {
            p.spawn(texte(if lieu.is_some() { "Aucun événement de ce genre en ce lieu." } else { "Aucun événement de ce genre pour l'instant." }, 13.0, charte::MUET));
        }
        for e in liste {
            p.spawn((Node { column_gap: Val::Px(10.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::left(Val::Px(2.0)), ..default() }, BorderColor::all(couleur_genre(e.genre)))).with_children(|l| {
                l.spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::left(Val::Px(10.0)), ..default() }).with_children(|n| {
                    n.spawn(texte(format!("tour {} · {}", e.tour, e.genre.nom().to_lowercase()), 12.0, couleur_genre(e.genre)));
                    n.spawn(texte(e.texte.clone(), 13.5, charte::TEXTE));
                });
            });
        }
        p.spawn(texte("Pas encore inscrits : commerce, guerres (leurs systèmes ne sont pas encore dans le jeu).", 12.5, charte::MUET));
    });
}

/// Couleur d'un genre d'événement (frise de l'histoire).
fn couleur_genre(g: rules::GenreEvenement) -> Color {
    use rules::GenreEvenement::*;
    match g {
        Fondation => Color::srgb(0.95, 0.81, 0.45),
        Population => Grandeur::Habitants.couleur(),
        Construction => Grandeur::Production.couleur(),
        Famine => charte::GRAVE,
        Bataille => charte::ALERTE,
        Projet => Grandeur::Culture.couleur(),
        Territoire => charte::AMENAGEMENT,
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
        let part = if c.capacite > 0 { (c.remplissage() as f64).powf(intendant::EXPOSANT) } else { 1.0 };
        // Gain de 100 travailleurs de plus (l'intendant place des blocs de 100)
        let plus = if c.capacite > c.travailleurs { Some(intendant::rendement(c.max, (c.travailleurs + 100).min(c.capacite), c.capacite) - c.rendement) } else { None };
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
fn synthese(g: &mut ChildSpawnerCommands, gr: Grandeur, city: &City, cases: &[CaseVue], images: &Images, image: Handle<Image>, cb: &crate::graphique::Courbe, tr: &crate::graphique::Trace, periode: usize, reglages: &[(Reglage, Rendements, i64)]) {
    // (les réglages de l'intendant d'abord : ce sont les décisions, ils doivent tenir à l'écran)
    if !reglages.is_empty() {
        reglages_intendant(g, gr, city, reglages);
    }
    let c = &city.state;
    let total = valeur(&city.travail.rendements, gr);
    let centre = cases.iter().find(|c| c.centre).map_or(0.0, |c| valeur(&c.rendement, gr));
    let travaillees = cases.iter().filter(|c| !c.centre && c.travailleurs > 0).count();
    let besoin = c.besoin_nourriture();
    let sources: Vec<(String, Option<f64>)> = vec![
        ("Centre de la ville".into(), Some(centre)),
        (format!("Cases travaillées ({travaillees})"), Some(total - centre)),
        ("Bâtiments : pas encore dans le jeu".into(), None),
        ("Commerce : pas encore dans le jeu".into(), None),
    ];
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
    let max = sources.iter().filter_map(|s| s.1).chain(usages.iter().filter_map(|u| u.1)).fold(0.01, f64::max);
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|t| {
            t.spawn(icone(images.grandeur(gr), 20.0, gr.couleur()));
            let verbe = if gr == Grandeur::Deniers { "viennent" } else { "vient" };
            t.spawn(texte(format!("D'où {verbe} {} ?", article(gr)), 16.0, gr.couleur()));
        });
        // Période et explication
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("À gauche le passé, à droite la prévision (l'intendant répartit les habitants à venir)", 12.5, charte::DOUX)]));
            // (le passé montré ; la prévision reste de 20 tours)
            l.spawn(texte("passé :", 12.5, charte::MUET));
            for (n, t) in [(10, "10 tours"), (20, "20 tours"), (40, "40 tours")] {
                let actif = periode == n || (periode == 0 && n == 40);
                l.spawn((
                    ChoixPeriode(n),
                    UiPointer::default(),
                    Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                    BackgroundColor(if actif { charte::TEXTE } else { Color::NONE }),
                    BorderColor::all(charte::TRAIT2),
                    children![texte(t, 12.5, if actif { charte::FOND } else { charte::DOUX })],
                ));
            }
        });
        // Le graphique et ses libellés
        poser_graphique(p, image, cb, tr);
        if cb.passe.len() < 3 {
            p.spawn(texte("L'historique se remplit à chaque fin de tour (40 tours au plus).", 12.5, charte::MUET));
        }
        // D'où elle vient, où elle va
        p.spawn(Node { column_gap: Val::Px(16.0), ..default() }).with_children(|d| {
            let (t_src, t_use) = if gr == Grandeur::Deniers { ("Recettes", "Dépenses") } else { ("Sources", "Usages") };
            d.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), ..default() }).with_children(|col| {
                col.spawn(texte(t_src, 14.0, charte::TEXTE));
                for (l, v) in &sources {
                    ligne_part(col, l, *v, max, gr.couleur());
                }
            });
            d.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: Val::Px(0.0), ..default() }).with_children(|col| {
                col.spawn(texte(t_use, 14.0, charte::TEXTE));
                for (l, v, couleur) in &usages {
                    ligne_part(col, l, *v, max, *couleur);
                }
            });
        });
        let net = match gr {
            Grandeur::Nourriture => format!("{} × le besoin", rules::decimal(total / besoin.max(0.001))),
            _ => format!("{} par tour", rules::decimal(total)),
        };
        p.spawn((Node { padding: UiRect::vertical(Val::Px(6.0)), border: UiRect::top(Val::Px(2.0)), ..default() }, BorderColor::all(charte::TRAIT2))).with_children(|l| {
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("Total", 14.0, charte::TEXTE)]));
            l.spawn(texte(net, 14.0, charte::TEXTE));
        });
        p.spawn(texte("Sur la carte : la grandeur de chaque case.", 12.5, charte::MUET));
    });
    if gr == Grandeur::Nourriture {
        types_de_nourriture(g, city);
        grenier(g, city);
    }
    if gr == Grandeur::Production {
        gisements_de_la_ville(g, city, cases);
    }
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
                format!("{} par tour ({} / {} travailleurs)", rules::decimal(intendant::unites_extraites(c.travailleurs, c.capacite)), rules::milliers(u64::from(c.travailleurs)), rules::milliers(u64::from(c.capacite)))
            } else {
                "pas exploité".to_string()
            };
            ligne_valeur(p, &format!("{} · {}", capitale(r.nom()), c.nom.to_lowercase()), valeur, if c.amenagement == Some(Amenagement::Exploitation) { charte::OK } else { charte::MUET });
        }
        if !stock.is_empty() {
            let liste: Vec<String> = stock.iter().map(|(r, q)| format!("{} {}", r.nom(), rules::decimal(*q))).collect();
            p.spawn(texte(format!("Stock de la ville : {}.", liste.join(", ")), 13.0, charte::TEXTE));
        }
        // Besoins de base et luxe (5g) : demande, part couverte au dernier tour, effet
        let c = &city.state;
        for r in crate::gisements::Ressource::TOUTES.into_iter().filter(|&r| c.demande(r) > 0.0) {
            let couv = c.couverture(r);
            let effet = match r {
                crate::gisements::Ressource::Sel => format!("salubrité +{}", rules::decimal(rules::SALUBRITE_SEL * couv)),
                crate::gisements::Ressource::Plantes => format!("salubrité +{}", rules::decimal(rules::SALUBRITE_PLANTES * couv)),
                crate::gisements::Ressource::Fibres => {
                    let manque = rules::SALUBRITE_VETEMENTS_MAX * (1.0 - couv) * c.part_besoin_vetements();
                    if manque > 0.05 { format!("manque de vêtements : salubrité −{}", rules::decimal(manque)) } else { "vêtus".to_string() }
                }
                _ => format!("luxe : bien-être +{}", rules::decimal(r.luxe().unwrap_or(0.0) * couv)),
            };
            let couleur = if couv >= 0.999 { charte::OK } else if couv > 0.0 { charte::ALERTE } else { charte::MUET };
            ligne_valeur(p, &format!("{} · demande {} par tour", capitale(r.nom()), rules::decimal(c.demande(r))), format!("couvert {:.0} % · {effet}", couv * 100.0), couleur);
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
            let etat = if compte { "compte".to_string() } else { format!("il en faut {} % : un {} y suffirait", (intendant::SEUIL_TYPE * 100.0) as u32, ["champ", "pâturage ou des légumineuses", "jardin"][k]) };
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
fn catalogue(l: &mut ChildSpawnerCommands, vl: &Ville, city: &City, cases: &[CaseVue], placement: Option<Amenagement>, groupe_vu: GroupeCatalogue) {
    let prod = city.travail.rendements.production;
    // Onglets du catalogue : un seul groupe à la fois (tout tient à l'écran)
    l.spawn(Node { column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(4.0)), ..default() }).with_children(|o| {
        for (gr, t) in [(GroupeCatalogue::Amenagements, "Aménagements"), (GroupeCatalogue::Projets, "Projets de ville"), (GroupeCatalogue::Unites, "Unités")] {
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
        let mut n = l.spawn((
            Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(if choisi { charte::SELECTION } else { Color::NONE }),
        ));
        if actif {
            n.insert((ChoixCatalogue(e), UiPointer::default()));
        }
        n.with_children(|i| {
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
    groupe(l, "Constructions de la ville : sur le centre, sans travailleurs");
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
    groupe(l, "Unités : ajoutées à la fin de la file ; ponts : leur emplacement se choisit sur la carte du monde");
    for k in crate::rules::UnitKind::ALL {
        let tours = rules::tours_restants(k.cost(), 0.0, prod).map_or("—".to_string(), |t| format!("{t} tour(s) seule"));
        let colon = if k == crate::rules::UnitKind::Settler { format!(" · emmène {} habitants", rules::milliers(rules::HABITANTS_COLON)) } else { String::new() };
        item(l, Element::Unite(k), k.name().into(), format!("{} production · {tours}{colon}", k.cost()), true, false);
    }
    }
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
        ligne_valeur(p, "Valeur pour la ville", format!("{}{}", signe(e.valeur), e.rentabilite.map_or(String::new(), |r| format!(" · rentabilisé en {} tours", r.ceil()))), couleur_signe(e.valeur));
        if let Some(t) = &pourquoi {
            p.spawn(texte(t.clone(), 12.5, if t.starts_with("Attention") { charte::ALERTE } else { CONSEIL }));
        }
        match e.risque {
            Risque::Famine(k) => ligne_valeur(p, "Risque de famine", format!("la nourriture tomberait à {} × le besoin", rules::decimal(k)), charte::GRAVE),
            Risque::BordDeFamine(k) => ligne_valeur(p, "Bord de famine", format!("{} × le besoin après", rules::decimal(k)), charte::GRAVE),
            Risque::Aucun => {}
        }
        ligne_valeur(p, "Tout de suite (l'intendant répartit les travailleurs)", ecarts(&gain.maintenant), charte::TEXTE);
        ligne_valeur(p, &format!("Dans {TOURS_EFFET} tours (avec la croissance)"), ecarts(&gain.dans), charte::AMENAGEMENT);
        if gain.dans.nourriture.abs() >= 0.05 {
            // (la nourriture au-delà de l'objectif de l'intendant ne fait plus grandir la ville)
            let pleine = POIDS_FREIN * gain.dans.nourriture;
            let note = if gain.dans.nourriture > 0.0 && e.nourriture < pleine - 0.05 { " : au-delà de l'objectif de l'intendant, elle ne fait plus grandir la ville" } else { "" };
            ligne_valeur(p, "Part de la nourriture dans la valeur", format!("{}{note}", signe(e.nourriture)), charte::DOUX);
        }
        ligne_valeur(p, &format!("Maximum de la ville (case pleine, {} travailleurs)", rules::milliers(gain.capacite.into())), format!("{} (pas tout de suite)", ecarts(&gain.plein)), charte::DOUX);
        ligne_valeur(p, "Capacité de la case", format!("{} → {} travailleurs", rules::milliers(c.capacite.into()), rules::milliers(gain.capacite.into())), charte::TEXTE);
        if gain.defriche {
            ligne_valeur(p, "Défrichement", format!("la forêt est perdue pour le bois (+{} de coût)", intendant::COUT_DEFRICHEMENT), charte::ALERTE);
        }
        if let Some(actuel) = c.amenagement {
            ligne_valeur(p, "Remplace", actuel.nom().to_string(), charte::ALERTE);
        }
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
            Forcage::Confirmer(h, n) => {
                let forcees = &mut city.state.consignes.forcees;
                forcees.retain(|(x, _)| *x != h);
                forcees.push((h, n));
                ecran.forcer = None;
                journal(format!(" : effectif forcé à {n} travailleurs sur une case."));
            }
            Forcage::Rendre(h) => {
                city.state.consignes.forcees.retain(|(x, _)| *x != h);
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
        Onglet::Commerce => {
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
        let mut c = CaseVue { hex: h, local: Hex::new(1, 0), centre: false, nom: String::new(), travailleurs: 0, capacite: 0, max: r, rendement: r, amenagement: None, habitants: 0.0, en_file: false, force: None, gisement: None };
        assert!(possible(&map, &c, Amenagement::CoupeDeBois).is_ok());
        c.amenagement = Some(Amenagement::Champ);
        assert!(possible(&map, &c, Amenagement::CoupeDeBois).is_err(), "plus d'arbres");
        assert!(possible(&map, &c, Amenagement::Paturage).is_ok());
    }

    #[test]
    fn la_case_sous_le_pointeur_est_celle_qu_on_a_dessinee() {
        let s = 30.0;
        for h in Hex::new(0, 0).range(3) {
            let p = position(h, s);
            assert_eq!(case_sous(p, s), h);
            // près du bord de l'hexagone, toujours la même case
            assert_eq!(case_sous(p + Vec2::new(s * 0.8, 0.0), s), h);
            assert_eq!(case_sous(p + Vec2::new(0.0, s * 0.9), s), h);
        }
    }
}
