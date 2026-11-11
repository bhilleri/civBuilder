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
    /// Placement d'un aménagement : gain à pleine capacité de chaque case (maximum donné).
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

/// Ce qui freine la croissance (un seul diagnostic, le plus grave), ses remèdes chiffrés, et les
/// autres alertes de la ville.
struct Frein {
    etat: Etat,
    texte: String,
    remedes: Vec<(Remede, String)>,
    alertes: Vec<(Etat, String, Remede)>,
}

/// Diagnostic de la croissance de la ville.
fn frein(map: &crate::map::WorldMap, city: &City, cases: &[CaseVue]) -> Frein {
    let c = &city.state;
    let y = city.travail.rendements;
    let besoin = c.besoin_nourriture().max(1e-6);
    let couverture = y.nourriture / besoin;
    let limite = y.nourriture * rules::HABITANTS_PAR_NOURRITURE;
    // Remèdes pour la nourriture : les aménagements qui en donnent le plus, à leur case conseillée
    let mut nourrir: Vec<(f64, Remede, String)> = [Amenagement::Champ, Amenagement::Paturage]
        .into_iter()
        .filter_map(|a| {
            let case = conseil(map, cases, a).and_then(|h| cases.iter().find(|x| x.hex == h))?;
            let g = gain_amenagement(map, case, a).ok()?;
            (g.plein.nourriture > 0.05).then(|| {
                (g.plein.nourriture, Remede::Placer(a), format!("{} : +{} nourriture, limite +{} habitants", capitale(a.nom()), rules::decimal(g.plein.nourriture), rules::milliers((g.plein.nourriture * rules::HABITANTS_PAR_NOURRITURE) as u64)))
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
        // Tours avant que la nourriture n'arrête la croissance, avec les rendements actuels
        let mut st = c.clone();
        let tours = (1..=60).find(|_| {
            let v = st.variation(y);
            st.habitants = (st.habitants as i64 + v).max(0) as u64;
            st.habitants as f64 >= limite * 0.98 || v <= 0
        });
        let suivant = rules::habitants_du_palier(c.palier() + 1);
        let palier = if (suivant as f64) > limite { format!(" · palier {} ({}) hors de portée sans remède", c.palier() + 1, rules::milliers(suivant)) } else { String::new() };
        match tours {
            Some(t) => (if t <= 20 { Etat::Alerte } else { Etat::Ok }, format!("la nourriture : limite {} habitants, atteinte dans ~{t} tours{palier}", rules::milliers(limite as u64)), nourrir),
            None => (Etat::Ok, format!("rien pour l'instant ; à long terme, la nourriture : limite {} habitants (plus de 60 tours){palier}", rules::milliers(limite as u64)), nourrir),
        }
    };
    remedes.truncate(3);
    // Autres alertes
    let mut alertes = Vec::new();
    if c.file.is_empty() && c.projet.is_none() {
        alertes.push((Etat::Alerte, "Rien à produire : à choisir pour finir le tour".to_string(), Remede::Construire));
    }
    if city.travail.chomeurs > 0 {
        let a = if cases.iter().any(|x| gain_amenagement(map, x, Amenagement::Champ).is_ok()) { Remede::Placer(Amenagement::Champ) } else { Remede::Construire };
        alertes.push((Etat::Alerte, format!("{} sans emploi : aménager des cases (plus de place)", rules::milliers(city.travail.chomeurs.into())), a));
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

/// Effet d'un aménagement sur une case : tout de suite (mêmes travailleurs) et à pleine capacité.
struct GainAmenagement {
    maintenant: Rendements,
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

/// Ce que donnerait l'aménagement `a` sur la case (ou pourquoi il est impossible).
fn gain_amenagement(map: &crate::map::WorldMap, c: &CaseVue, a: Amenagement) -> Result<GainAmenagement, &'static str> {
    if c.centre {
        return Err("pas sur le centre de la ville");
    }
    if c.amenagement == Some(a) {
        return Err("déjà aménagée ainsi");
    }
    a.possible(map, c.hex)?;
    let max = intendant::rendement_max(map, c.hex, Some(a));
    let cap = intendant::capacite(Some(a));
    let cout = a.cout(map, c.hex);
    Ok(GainAmenagement { maintenant: intendant::rendement(max, c.travailleurs.min(cap), cap) - c.rendement, plein: max - c.max, cout, defriche: cout > a.cout_de_base(), capacite: cap })
}

/// Case conseillée par l'intendant : le plus fort gain total à pleine capacité.
fn conseil(map: &crate::map::WorldMap, cases: &[CaseVue], a: Amenagement) -> Option<Hex> {
    cases.iter().filter_map(|c| gain_amenagement(map, c, a).ok().map(|g| (c.hex, g.plein.total()))).max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0))).map(|(h, _)| h)
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

/// Systèmes de l'écran (avant le menu Échap, qui ne doit pas s'ouvrir quand Échap ferme l'écran).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EcranVilleSet;

pub struct EcranVillePlugin;

impl Plugin for EcranVillePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EcranVille>()
            .init_resource::<VueCarte>()
            .init_resource::<FinDeTourJoueur>()
            .add_systems(Update, fin_de_tour_joueur.after(crate::input::InputSet).before(crate::game::EndTurnSet))
            .add_systems(Startup, construire)
            .add_systems(
                Update,
                (ouvrir_fermer, commandes, carte_souris, construire_actions, reconstruire, placer_plan, afficher)
                    .chain()
                    .in_set(EcranVilleSet)
                    .before(crate::menu::toggle_menu),
            );
    }
}

// ------------------------------------------------------------------ Données d'une case

/// Ce que la carte et les fiches montrent d'une case de la ville.
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

fn nom_de_case(map: &crate::map::WorldMap, h: Hex) -> String {
    map.describe(h)
}

/// Cases de la ville : son centre et son territoire, avec travailleurs et rendements.
fn cases_vues(map: &crate::map::WorldMap, centre: Hex, ville: Entity, city: &City, territoire: &Territory, amenagements: &Amenagements) -> Vec<CaseVue> {
    let wrap = map.wrap();
    let possedees: Vec<Hex> = territoire.0.iter().filter(|(_, e)| **e == ville).map(|(h, _)| *h).collect();
    let dispo = rules::cases_de_la_ville(map, centre, &possedees, &Default::default(), &amenagements.0);
    let rc = Rendements::from(rules::center_yields(map, centre));
    let mut cases = vec![CaseVue { hex: centre, local: Hex::new(0, 0), centre: true, nom: "Centre de la ville".into(), travailleurs: 0, capacite: 0, max: rc, rendement: rc, amenagement: None, habitants: 0.0 }];
    for c in dispo {
        let w = city.travail.sur(c.hex);
        let near = wrap.nearest(centre, c.hex);
        cases.push(CaseVue {
            hex: c.hex,
            local: Hex::new(near.q - centre.q, near.r - centre.r),
            centre: false,
            nom: nom_de_case(map, c.hex),
            travailleurs: w,
            capacite: c.capacite,
            max: c.max,
            rendement: intendant::rendement(c.max, w, c.capacite),
            amenagement: amenagements.0.get(&c.hex).copied(),
            habitants: 0.0,
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
                                c.spawn(Node { position_type: PositionType::Absolute, right: Val::Px(8.0), top: Val::Px(8.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).with_children(|b| {
                                    for (sens, signe) in [(1, "+"), (-1, "−"), (0, "")] {
                                        let mut bouton = b.spawn((
                                            BoutonZoom(sens),
                                            UiPointer::default(),
                                            Node { width: Val::Px(30.0), height: Val::Px(28.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
                                            BackgroundColor(charte::SURFACE),
                                            BorderColor::all(charte::TRAIT2),
                                        ));
                                        if sens == 0 {
                                            // Recentrer : un petit hexagone
                                            bouton.with_child((Node { width: Val::Px(12.0), height: Val::Px(14.0), ..default() }, ImageNode::new(hexagone.clone()).with_color(charte::TEXTE)));
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
) {
    if !std::mem::take(&mut demande.0) {
        return;
    }
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
    ecran.onglet = Onglet::Rendements;
    ecran.case = None;
    ecran.grandeur = None;
    ecran.catalogue = true;
    ecran.placement = None;
    ecran.cible = None;
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
) {
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    let ville = selection.0.filter(|e| cities.contains(*e));
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
        if (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && ecran.cible.is_some() {
            ecran.confirmer = true;
        }
        // 1 à 3 : détail de la nourriture, de la production, des deniers
        for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].into_iter().enumerate() {
            if keys.just_pressed(k) && !wiki.open {
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
        ecran.ville = Some(v);
        ecran.case = None;
        ecran.grandeur = None;
        ecran.catalogue = false;
        ecran.placement = None;
        ecran.cible = None;
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
    (cartes_pop, genres, remedes): (Query<(Ref<UiPointer>, &ChoixCartePop)>, Query<(Ref<UiPointer>, &ChoixGenre)>, Query<(Ref<UiPointer>, &BoutonRemede)>),
    mut survols: Query<(&UiPointer, &mut BackgroundColor), (Or<(With<ChoixCase>, With<BoutonZoom>, With<Fermer>, With<FermerFiche>, With<BasculeAmenagements>, With<Fil>, With<ChoixCatalogue>, With<ActionFile>, With<ArreterProjet>, With<AmenagerIci>, With<ChoixReglage>, With<ChoixGenre>, With<BoutonRemede>)>, Changed<UiPointer>)>,
) {
    if !ecran.ouvert() {
        return;
    }
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    for (i, b, mut bord, enfants) in &mut onglets {
        if appui(&i) && ecran.onglet != b.0 {
            ecran.onglet = b.0;
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
            ecran.placement = if ecran.placement == Some(a) { None } else { Some(a) };
            ecran.cible = None;
            ecran.case = None;
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
fn rayon_hex(cadre: Vec2, cases: &[CaseVue], zoom: f32) -> f32 {
    let r = cases.iter().map(|c| Hex::new(0, 0).distance(c.local)).max().unwrap_or(1).max(1) as f32;
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
    (map, territoire, amenagements): (Res<GameMap>, Res<Territory>, Res<Amenagements>),
    cities: Query<(&City, &HexPos)>,
    choix: Query<(&UiPointer, &ChoixCase)>,
    mut contour: Query<(&mut Node, &mut ImageNode), (With<ContourSurvol>, Without<Bulle>)>,
    bulle: Single<(Entity, &mut Node), With<Bulle>>,
    mut commands: Commands,
    mut glisse: Local<Option<(Vec2, bool)>>,
    mut montre: Local<Option<Hex>>,
    (time, mut dernier_clic): (Res<Time>, Local<(f64, Option<Hex>)>),
) {
    let (bulle_e, mut bulle_node) = bulle.into_inner();
    let Some((city, centre)) = ecran.ville.and_then(|v| cities.get(v).ok().map(|c| (v, c))).map(|(v, (c, p))| ((v, c), p.0)) else {
        *glisse = None;
        return;
    };
    let (transform, computed) = *cadre;
    let taille = computed.size() * computed.inverse_scale_factor();
    let cases = cases_vues(&map.0, centre, city.0, city.1, &territoire, &amenagements);
    let s = rayon_hex(taille, &cases, vue.zoom);
    // Pointeur de la souris lu dans le picking (comme les boutons ; marche aussi avec le pilote)
    let souris = pointeurs.iter().find(|(id, _)| **id == PointerId::Mouse).and_then(|(_, l)| l.location.as_ref()).map(|l| l.position);
    let normalise = souris.and_then(|p| computed.normalize_point(*transform, p / computed.inverse_scale_factor()));
    let curseur = normalise.map(|n| n * taille);
    let dessus = normalise.is_some_and(|n| n.x.abs() <= 0.5 && n.y.abs() <= 0.5);

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
                if let Some(x) = vise.filter(|x| gain_amenagement(&map.0, x, a).is_ok()) {
                    let maintenant = time.elapsed_secs_f64();
                    if dernier_clic.1 == Some(x.hex) && maintenant - dernier_clic.0 < 0.45 {
                        ecran.confirmer = true;
                    }
                    *dernier_clic = (maintenant, Some(x.hex));
                    ecran.cible = Some(x.hex);
                }
            } else {
                ecran.case = if hex.is_some() && hex == ecran.case { None } else { hex };
            }
        }
        *glisse = None;
    }
    if vue.zoom <= 1.0 {
        vue.decalage = Vec2::ZERO;
    }

    // Survol : par la carte, ou par une ligne de la liste
    let par_liste = choix.iter().find(|(i, _)| **i != UiPointer::None).map(|(_, c)| c.0);
    let survol = if dessus && glisse.is_none_or(|g| !g.1) {
        curseur.and_then(|c| {
            let local = case_sous(c - vue.decalage, s);
            cases.iter().find(|x| x.local == local)
        })
    } else {
        par_liste.and_then(|h| cases.iter().find(|x| x.hex == h))
    };
    for (mut node, mut image) in &mut contour {
        match survol {
            Some(c) => {
                let p = position(c.local, s);
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
    let sur_carte = dessus && survol.is_some();
    match (sur_carte, survol, souris) {
        (true, Some(c), Some(p)) => {
            bulle_node.display = Display::Flex;
            bulle_node.left = Val::Px((p.x + 14.0).min(window.width() - 290.0));
            bulle_node.top = Val::Px((p.y + 14.0).min(window.height() - 140.0));
            if *montre != Some(c.hex) || ecran.is_changed() {
                *montre = Some(c.hex);
                commands.entity(bulle_e).despawn_related::<Children>();
                let placement = ecran.placement.map(|a| (a, gain_amenagement(&map.0, c, a)));
                commands.entity(bulle_e).with_children(|b| match placement {
                    Some((a, g)) => infobulle_placement(b, c, a, g),
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

/// Infobulle d'une case pendant le placement d'un aménagement.
fn infobulle_placement(b: &mut ChildSpawnerCommands, c: &CaseVue, a: Amenagement, g: Result<GainAmenagement, &'static str>) {
    b.spawn(texte(c.nom.clone(), 14.0, charte::TEXTE));
    match g {
        Ok(g) => {
            b.spawn(texte(format!("{} : {} à pleine capacité", capitale(a.nom()), ecarts(&g.plein)), 13.0, charte::AMENAGEMENT));
            b.spawn(texte(format!("tout de suite : {} · {} production", ecarts(&g.maintenant), g.cout), 12.5, charte::DOUX));
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
    (map, territoire, amenagements, state): (Res<GameMap>, Res<Territory>, Res<Amenagements>, Res<GameState>),
    cities: Query<(Ref<City>, &HexPos)>,
    (entete, gauche, legende, plan, titre, liste): (Single<Entity, With<EnTete>>, Single<Entity, With<Gauche>>, Single<Entity, With<Legende>>, Single<Entity, With<Plan>>, Single<Entity, With<TitreListe>>, Single<Entity, With<Liste>>),
    (cadre, largeur_gauche): (Single<&ComputedNode, With<Cadre>>, Single<&ComputedNode, With<Gauche>>),
    mut textes: Query<&mut Text>,
    mut assets: ResMut<Assets<Image>>,
    mut derniere: Local<(Vec2, f32, u32, f32)>,
    mut graphique: Local<Option<Handle<Image>>>,
) {
    let Some(v) = ecran.ville else { return };
    let Ok((city, centre)) = cities.get(v) else { return };
    let taille = cadre.size() * cadre.inverse_scale_factor();
    let large = largeur_gauche.size().x * largeur_gauche.inverse_scale_factor();
    let cle = (taille, vue.zoom, state.turn, large);
    let change = ecran.is_changed() || city.is_changed() || amenagements.is_changed() || territoire.is_changed() || *derniere != cle;
    if !change || taille.x < 10.0 {
        return;
    }
    *derniere = cle;
    let cases = cases_vues(&map.0, centre.0, v, &city, &territoire, &amenagements);
    let choisie = ecran.case.and_then(|h| cases.iter().find(|c| c.hex == h));

    let diagnostic = frein(&map.0, &city, &cases);
    commands.entity(*entete).despawn_related::<Children>().with_children(|e| {
        en_tete(e, &city, &images);
        bandeau_frein(e, &diagnostic);
    });
    // Graphique de la grandeur détaillée (image refaite ; l'ancienne est libérée)
    if let Some(h) = graphique.take() {
        assets.remove(&h);
    }
    // Aperçus de l'intendant : la répartition refaite avec d'autres consignes
    let dispo = {
        let possedees: Vec<Hex> = territoire.0.iter().filter(|(_, e)| **e == v).map(|(h, _)| *h).collect();
        rules::cases_de_la_ville(&map.0, centre.0, &possedees, &Default::default(), &amenagements.0)
    };
    let apercu = |cons: &intendant::Consignes| -> Rendements {
        let mut st = city.state.clone();
        st.consignes = cons.clone();
        rules::repartition_ville(&map.0, centre.0, &st, &dispo).rendements
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
            let c = courbe_population(&city, &cases);
            let tr = crate::graphique::Trace::new(&c, Vec2::new((large - 30.0).max(200.0), 230.0));
            let h = assets.add(crate::graphique::dessiner(&c, &tr, charte::TRAIT, charte::TEXTE));
            *graphique = Some(h.clone());
            Some((h, c, tr))
        }
        (Onglet::Rendements, Some(gr), None) => {
            let c = courbe(&city, gr, ecran.periode);
            let tr = crate::graphique::Trace::new(&c, Vec2::new((large - 30.0).max(200.0), 200.0));
            let h = assets.add(crate::graphique::dessiner(&c, &tr, charte::TRAIT, charte::TEXTE));
            *graphique = Some(h.clone());
            Some((h, c, tr))
        }
        _ => None,
    };
    commands.entity(*gauche).despawn_related::<Children>().with_children(|g| match (ecran.onglet, choisie) {
        _ if ecran.catalogue => {}
        (Onglet::Rendements, c) => {
            cartes(g, &city, &images, ecran.grandeur);
            if ecran.grandeur.is_some() || c.is_some() {
                fil(g, ecran.grandeur, c.map(|c| c.nom.as_str()));
            }
            match (c, ecran.grandeur, &courbe) {
                (Some(c), gr, _) => fiche_case(g, c, &map.0, &images, gr, forcage.as_ref()),
                (None, Some(gr), Some((h, cb, tr))) => synthese(g, gr, &city, &cases, &images, h.clone(), cb, tr, ecran.periode, &reglages),
                _ => {
                    ensemble(g, &city, &cases);
                    file_et_projet(g, &city);
                }
            }
        }
        (Onglet::Histoire, c) => histoire(g, &city, ecran.histoire_genre, c),
        (_, Some(c)) => fiche_case(g, c, &map.0, &images, None, forcage.as_ref()),
        (Onglet::Population, None) => population(g, &city, &cases, &images, courbe.as_ref()),
        (Onglet::Commerce, None) => vide(g, "Commerce", "Aucun axe de commerce : les échanges entre villes ne sont pas encore dans le jeu."),
    });
    let gains: Vec<Option<GainAmenagement>> = cases.iter().map(|c| ecran.placement.and_then(|a| gain_amenagement(&map.0, c, a).ok())).collect();
    let conseillee = ecran.placement.and_then(|a| conseil(&map.0, &cases, a));
    let mode = match (ecran.onglet, ecran.grandeur) {
        (Onglet::Histoire, _) => ModeCarte::Histoire,
        _ if let Some(a) = ecran.placement => ModeCarte::Placement(a, gains.iter().flatten().map(|g| g.plein.total()).fold(0.01, f64::max)),
        (Onglet::Population, _) if ecran.carte_habitants => {
            let v: Vec<f64> = cases.iter().map(|c| c.habitants).filter(|v| *v > 0.5).collect();
            ModeCarte::Habitants(v.iter().copied().fold(f64::MAX, f64::min).min(1e9), v.iter().copied().fold(1.0, f64::max))
        }
        (Onglet::Population, _) => ModeCarte::Remplissage,
        (Onglet::Rendements, Some(g)) => ModeCarte::Grandeur(g, cases.iter().map(|c| valeur(&c.rendement, g)).fold(0.01, f64::max)),
        _ => ModeCarte::Dominante,
    };
    commands.entity(*legende).despawn_related::<Children>().with_children(|l| legende_carte(l, mode, ecran.amenagements_caches));
    let s = rayon_hex(taille, &cases, vue.zoom);
    commands.entity(*plan).despawn_related::<Children>().with_children(|p| {
        for (c, g) in cases.iter().zip(&gains) {
            hexagone(p, c, s, mode, !ecran.amenagements_caches, &images, g.as_ref());
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
        if let Some(h) = conseillee {
            anneau(p, h, CONSEIL);
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
                Some((a, c)) => placement(g, &map.0, &city, a, c, ecran.cible.is_none()),
                None if ecran.placement.is_some() => vide(g, "Placement", "Aucune case de la ville ne peut porter cet aménagement."),
                None => file_et_projet(g, &city),
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
            catalogue(l, &map.0, &city, &cases, ecran.placement);
        });
        return;
    }
    let titre_liste: String = match (ecran.onglet, ecran.grandeur) {
        (Onglet::Rendements, Some(g)) => format!("Cases · {}", g.nom().to_lowercase()),
        (Onglet::Rendements, None) => "Cases travaillées".into(),
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
fn hexagone(p: &mut ChildSpawnerCommands, c: &CaseVue, s: f32, mode: ModeCarte, amenagements: bool, images: &Images, gain: Option<&GainAmenagement>) {
    let pos = position(c.local, s);
    let mode_pop = mode == ModeCarte::Remplissage;
    let fond = match mode {
        ModeCarte::Placement(_, max) => match gain {
            Some(g) => melange(charte::SURFACE, charte::AMENAGEMENT, 0.12 + 0.5 * (g.plein.total().max(0.0) / max) as f32),
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
        } else if let (ModeCarte::Placement(a, _), true) = (mode, s >= 22.0) {
            match gain {
                Some(g) => {
                    let v = valeur(&g.plein, vise(a));
                    h.spawn(centre(texte(format!("{}{}", if v >= 0.0 { "+" } else { "−" }, rules::decimal(v.abs())), (s * 0.36).clamp(11.0, 19.0), charte::TEXTE)));
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

/// Une entrée de légende : pastille de couleur et texte.
fn pastille(l: &mut ChildSpawnerCommands, couleur: Color, t: &str) {
    l.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|e| {
        e.spawn((Node { width: Val::Px(18.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(couleur)));
        e.spawn(texte(t, 13.0, charte::TEXTE));
    });
}

/// Légende et couches, au-dessus de la carte.
fn legende_carte(l: &mut ChildSpawnerCommands, mode: ModeCarte, caches: bool) {
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
        l.spawn(texte("chiffres : habitants de la case (pour l'instant, chacun vit là où il travaille)", 12.5, charte::MUET));
        return;
    }
    if let ModeCarte::Placement(a, _) = mode {
        pastille(l, melange(charte::SURFACE, charte::AMENAGEMENT, 0.2), "gain faible");
        pastille(l, melange(charte::SURFACE, charte::AMENAGEMENT, 0.6), "fort");
        pastille(l, Color::srgb(0.06, 0.075, 0.11), "impossible");
        pastille(l, CONSEIL, "conseil de l'intendant");
        pastille(l, charte::SELECTION, "case choisie");
        l.spawn(texte(format!("chiffres : {} gagnée à pleine capacité", vise(a).nom().to_lowercase()), 12.5, charte::MUET));
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
            texte(if caches { "Aménagements : masqués" } else { "Aménagements" }, 13.0, if caches { charte::MUET } else { charte::TEXTE }),
        ],
    ));
}

/// En-tête : nom, palier, habitants, croissance, construction en cours, bouton Fermer.
fn en_tete(e: &mut ChildSpawnerCommands, city: &City, images: &Images) {
    let c = &city.state;
    let y = city.travail.rendements;
    let variation = c.variation(y);
    e.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(18.0), ..default() }).with_children(|l| {
        l.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).with_children(|n| {
            n.spawn(texte(c.name.clone(), 26.0, charte::TEXTE));
            n.spawn(Node { column_gap: Val::Px(14.0), align_items: AlignItems::Center, ..default() }).with_children(|m| {
                m.spawn(icone(images.grandeur(Grandeur::Habitants), 16.0, Grandeur::Habitants.couleur()));
                m.spawn(texte(format!("palier {} · {} habitants", c.palier(), rules::milliers(c.habitants)), 14.0, charte::DOUX));
                let (t, couleur) = if variation > 0 {
                    let suivant = rules::habitants_du_palier(c.palier() + 1);
                    let tours = (suivant.saturating_sub(c.habitants) as f64 / variation as f64).ceil();
                    (format!("+{} par tour · palier {} dans {} tours", rules::milliers(variation as u64), c.palier() + 1, tours), charte::OK)
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
        l.spawn((BoutonConstruire, bouton(charte::ACTION_FOND), children![texte("Construire (B)", 14.0, charte::TEXTE)]));
        l.spawn((Fermer, bouton(charte::SURFACE), children![texte("Fermer (Échap)", 14.0, charte::TEXTE)]));
    });
}

/// Ce que fait la ville de sa production, pour l'en-tête.
fn en_cours(c: &rules::CityState, production: f64) -> String {
    match (c.file.first(), c.projet) {
        (Some(k), _) => {
            let suite = if c.file.len() > 1 { format!(" · puis {} en file", c.file.len() - 1) } else { String::new() };
            format!("En construction : {} · {}{suite}", capitale(k.objet.name()), delai(k.objet.cost(), k.investi, production))
        }
        (None, Some(p)) => format!("Projet : {} (depuis {} tours)", p.nom(), c.tours_en_projet),
        (None, None) => "Rien à produire : à choisir pour finir le tour".into(),
    }
}

/// Délai d'une construction : « 3 tour(s) », « prête (attend une case libre) », « jamais au rythme actuel ».
fn delai(cout: i32, stock: f64, par_tour: f64) -> String {
    match rules::tours_restants(cout, stock, par_tour) {
        Some(0) => "prête (attend une case libre)".into(),
        Some(t) => format!("{t} tour(s)"),
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
        if !f.remedes.is_empty() || !f.alertes.is_empty() {
            b.spawn(Node { column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|l| {
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
    r.spawn((
        ChoixGrandeur(g),
        UiPointer::default(),
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(9.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() },
        BackgroundColor(charte::SURFACE),
        BorderColor::all(if choisie { charte::SELECTION } else { Color::NONE }),
    ))
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
    let suite_n = if couverture < 1.0 { " : famine" } else { "" };
    let prod = match (c.file.first(), c.projet) {
        (Some(k), _) => format!("{} : {}", capitale(k.objet.name()), delai(k.objet.cost(), k.investi, y.production)),
        (None, Some(p)) => format!("projet : {}", p.nom().to_lowercase()),
        (None, None) => "rien à produire : à choisir".into(),
    };
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|r| {
            carte_grandeur(r, images, Grandeur::Nourriture, rules::decimal(y.nourriture), "par tour", etat_n, format!("{} × le besoin{suite_n}", rules::decimal(couverture)), choisie == Some(Grandeur::Nourriture));
            let etat_p = if c.file.is_empty() && c.projet.is_none() { Etat::Alerte } else { Etat::Ok };
            carte_grandeur(r, images, Grandeur::Production, rules::decimal(y.production), "par tour", etat_p, prod, choisie == Some(Grandeur::Production));
            carte_grandeur(r, images, Grandeur::Deniers, format!("+{}", rules::decimal(y.deniers)), "par tour", Etat::Ok, "versés au trésor".into(), choisie == Some(Grandeur::Deniers));
        });
        // Les autres grandeurs : affichées pour dire qu'elles n'existent pas encore
        p.spawn(Node { column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, ..default() }).with_children(|r| {
            for g in [Grandeur::Eau, Grandeur::Recherche, Grandeur::Culture, Grandeur::Defense, Grandeur::BienEtre] {
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
fn population(g: &mut ChildSpawnerCommands, city: &City, cases: &[CaseVue], images: &Images, graphique: Option<&(Handle<Image>, crate::graphique::Courbe, crate::graphique::Trace)>) {
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
            let (l, e) = if t.chomeurs > 0 { ("aménager des cases".to_string(), charte::ALERTE) } else { ("tous employés".to_string(), charte::OK) };
            indicateur(r, "Sans emploi", rules::milliers(t.chomeurs.into()), l, e);
            indicateur(r, "Taux d'urbanisation", "—".into(), "pas encore de quartiers urbains".into(), charte::MUET);
        });
        p.spawn(texte("Pour l'instant, les habitants vivent là où ils travaillent.", 12.5, charte::MUET));
    });
    // Évolution et limites
    let lim = limites(city, cases);
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, "Évolution et limites de la ville");
        if let Some((nom, _, couleur, quoi)) = lim.iter().min_by(|a, b| a.1.total_cmp(&b.1)) {
            p.spawn(Node { column_gap: Val::Px(4.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|l| {
                l.spawn(texte("Échelle logarithmique. Trait plein : le passé ; pointillé : la prévision avec les rendements actuels ; lignes de couleur : ce que la ville peut porter. La plus basse est la limite, ici", 12.5, charte::MUET));
                l.spawn(texte(nom.to_lowercase(), 12.5, *couleur));
                l.spawn(texte(format!("({quoi})."), 12.5, charte::MUET));
            });
        }
        if let Some((h, cb, tr)) = graphique {
            poser_graphique(p, h.clone(), cb, tr);
        }
        for (nom, v, couleur, _) in &lim {
            let marge = if *v > c.habitants as f64 { format!("marge {}", rules::milliers((*v - c.habitants as f64) as u64)) } else { "dépassée".into() };
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(*nom, 13.5, *couleur)]));
                l.spawn(texte(format!("{} habitants", rules::milliers(*v as u64)), 13.5, charte::TEXTE));
                l.spawn((Node { width: Val::Px(110.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(marge, 12.5, if *v > c.habitants as f64 { charte::MUET } else { charte::GRAVE })]));
            });
        }
        ligne_valeur(p, "Eau", "pas encore dans le jeu".into(), charte::MUET);
    });
    // Mouvement de la population
    let dernier = c.historique.iter().rev().take(2).map(|r| r.habitants as i64).collect::<Vec<_>>();
    let solde = if dernier.len() == 2 { Some(dernier[0] - dernier[1]) } else { None };
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, "Mouvement de la population");
        ligne_valeur(p, "Solde du dernier tour", solde.map_or("—".into(), |v| format!("{}{} habitants", if v >= 0 { "+" } else { "−" }, rules::milliers(v.unsigned_abs()))), if solde.is_some_and(|v| v < 0) { charte::GRAVE } else { charte::TEXTE });
        ligne_valeur(p, "Prochain tour (rendements actuels)", { let v = c.variation(t.rendements); format!("{}{} habitants", if v >= 0 { "+" } else { "−" }, rules::milliers(v.unsigned_abs())) }, charte::TEXTE);
        p.spawn(texte("Naissances, décès et migrations : pas encore distingués dans le jeu (seul le solde compte).", 12.5, charte::MUET));
    });
    vide(g, "Cultures et religions", "Une seule culture pour l'instant : les cultures et les religions ne sont pas encore dans le jeu.");
    vide(g, "Ordres de la société", "Paysans, artisans, marchands, prêtres, guerriers : pas encore dans le jeu.");
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
        p.spawn(texte("Pas encore inscrits : commerce, guerres, culture (leurs systèmes ne sont pas encore dans le jeu).", 12.5, charte::MUET));
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
    }
}

/// Fiche d'une case choisie sur la carte ou dans la liste.
fn fiche_case(g: &mut ChildSpawnerCommands, c: &CaseVue, map: &crate::map::WorldMap, images: &Images, seule: Option<Grandeur>, forcage: Option<&VueForcage>) {
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
            t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(c.nom.clone(), 17.0, charte::TEXTE)]));
            t.spawn((FermerFiche, bouton(charte::SURFACE), children![texte("× fermer (Échap)", 13.0, charte::TEXTE)]));
        });
        if c.centre {
            p.spawn(texte("Centre de la ville : il rapporte sans travailleurs.", 13.5, charte::DOUX));
        } else {
            ligne_valeur(p, "Aménagement de production", c.amenagement.map_or("aucun (état naturel : cueillette, chasse)".into(), |a| a.nom().to_lowercase()), if c.amenagement.is_some() { charte::AMENAGEMENT } else { charte::DOUX });
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("Travailleurs", 13.5, charte::TEXTE)]));
                l.spawn(texte(format!("{} / {}", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())), 13.5, charte::TEXTE));
                l.spawn(barre(c.remplissage(), 90.0, if c.remplissage() >= 0.9 { charte::ALERTE } else { charte::OK }));
            });
            if let Some(f) = forcage {
                effectif(p, c, f);
            }
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
            if possibles.is_empty() {
                p.spawn(texte("Aucun autre aménagement possible ici.", 12.5, charte::MUET));
            } else {
                p.spawn(texte("Aménager cette case (un aménagement par case : il remplace l'actuel) :", 13.0, charte::TEXTE));
                p.spawn(Node { column_gap: Val::Px(8.0), row_gap: Val::Px(6.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|b| {
                    for a in Amenagement::possibles(map, c.hex).into_iter().filter(|a| Some(*a) != c.amenagement) {
                        let gain = gain_amenagement(map, c, a).map(|g| ecarts(&g.plein)).unwrap_or_default();
                        b.spawn((AmenagerIci(a), bouton(Color::NONE), children![texte(format!("{} · {} production · {gain}", capitale(a.nom()), a.cout(map, c.hex)), 13.0, charte::AMENAGEMENT)]));
                    }
                });
            }
        }
    });
}

/// Courbe d'une grandeur : les tours de l'historique, puis aujourd'hui (rendement actuel) ; seuils.
fn courbe(city: &City, g: Grandeur, periode: usize) -> crate::graphique::Courbe {
    use crate::graphique::{Courbe, Seuil};
    let n = if periode == 0 { rules::HISTORIQUE_TOURS } else { periode };
    let mut passe: Vec<f64> = city.state.historique.iter().map(|r| valeur(&Rendements::new(r.nourriture, r.production, r.deniers), g)).collect();
    passe.push(valeur(&city.travail.rendements, g));
    let passe = passe[passe.len().saturating_sub(n)..].to_vec();
    let besoin = city.state.besoin_nourriture();
    let seuils = match g {
        Grandeur::Nourriture => vec![
            Seuil { valeur: besoin, libelle: "besoin".into(), couleur: charte::GRAVE },
            Seuil { valeur: besoin * (1.0 + rules::SURPLUS_POUR_CROITRE), libelle: "croissance pleine (1,5 ×)".into(), couleur: charte::ALERTE },
        ],
        Grandeur::Deniers => vec![Seuil { valeur: 0.0, libelle: "équilibre".into(), couleur: charte::ALERTE }],
        _ => Vec::new(),
    };
    Courbe::simple(passe, 20, seuils, g.couleur())
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
        for s in &cb.seuils {
            poser(gph, MARGE_GAUCHE, tr.y(s.valeur) - 17.0, tr.taille.x - MARGE_GAUCHE - MARGE_DROITE - 4.0, true, s.libelle.clone(), s.couleur);
        }
        let bas = tr.taille.y - 18.0;
        let n = cb.passe.len();
        if n > 1 {
            poser(gph, tr.x(-(n as f64 - 1.0)) - 40.0, bas, 80.0, false, format!("−{} tours", n - 1), charte::MUET);
        }
        poser(gph, tr.x(0.0) - 45.0, bas, 90.0, false, "aujourd'hui".into(), charte::TEXTE);
        poser(gph, tr.x(cb.futur as f64) - 80.0, bas, 80.0, true, format!("+{} tours", cb.futur), charte::MUET);
    });
}

/// Ce que la ville peut porter : habitants que la nourriture nourrit, habitants qui ont un emploi
/// (la moitié travaille) ; l'eau n'est pas encore dans le jeu.
fn limites(city: &City, cases: &[CaseVue]) -> Vec<(&'static str, f64, Color, &'static str)> {
    let capacite: u32 = cases.iter().map(|c| c.capacite).sum();
    vec![
        ("Nourriture", city.travail.rendements.nourriture * rules::HABITANTS_PAR_NOURRITURE, Grandeur::Nourriture.couleur(), "habitants que la nourriture peut nourrir (1 pour 1 000)"),
        ("Emplois", capacite as f64 / intendant::PART_ACTIVE, Grandeur::Production.couleur(), "au-delà, les nouveaux habitants n'ont pas de travail (la moitié travaille)"),
    ]
}

/// Courbe de la population : le passé, la prévision tour par tour avec les rendements actuels
/// (la nourriture arrête la croissance), les limites et le palier suivant ; échelle logarithmique.
fn courbe_population(city: &City, cases: &[CaseVue]) -> crate::graphique::Courbe {
    use crate::graphique::{Courbe, Seuil};
    let mut passe: Vec<f64> = city.state.historique.iter().map(|r| r.habitants as f64).collect();
    if passe.last() != Some(&(city.state.habitants as f64)) {
        passe.push(city.state.habitants as f64);
    }
    let y = city.travail.rendements;
    let mut st = city.state.clone();
    let prevision: Vec<f64> = (0..40)
        .map(|_| {
            let v = st.variation(y);
            st.habitants = (st.habitants as i64 + v).max(rules::HABITANTS_PLANCHER as i64) as u64;
            st.habitants as f64
        })
        .collect();
    let mut seuils: Vec<Seuil> = limites(city, cases).into_iter().map(|(n, v, c, _)| Seuil { valeur: v.max(1.0), libelle: format!("{n} : {}", rules::milliers(v as u64)), couleur: c }).collect();
    let palier = city.state.palier() + 1;
    seuils.push(Seuil { valeur: rules::habitants_du_palier(palier) as f64, libelle: format!("palier {palier}"), couleur: charte::MUET });
    Courbe { passe, futur: 40, seuils, couleur: Grandeur::Habitants.couleur(), prevision: Some(prevision), log: true }
}

/// Détail d'une grandeur : graphique (passé, prévision, seuils), d'où elle vient, où elle va.
#[allow(clippy::too_many_arguments)]
fn synthese(g: &mut ChildSpawnerCommands, gr: Grandeur, city: &City, cases: &[CaseVue], images: &Images, image: Handle<Image>, cb: &crate::graphique::Courbe, tr: &crate::graphique::Trace, periode: usize, reglages: &[(Reglage, Rendements, i64)]) {
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
            let mut u = vec![(format!("{} habitants (1 pour 1 000)", rules::milliers(c.habitants)), Some(besoin.min(total)), gr.couleur())];
            if total < besoin {
                u.push(("Manque : famine".into(), Some(besoin - total), charte::GRAVE));
            } else {
                u.push(("Réserve pour croître (jusqu'à +50 %)".into(), Some((total - besoin).min(besoin * rules::SURPLUS_POUR_CROITRE)), gr.couleur()));
                let reste = total - besoin * (1.0 + rules::SURPLUS_POUR_CROITRE);
                if reste > 0.05 {
                    u.push(("Excédent : perdu (pas encore de commerce)".into(), Some(reste), charte::ALERTE));
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
            l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("À gauche le passé, à droite la prévision au rythme actuel (bande : incertitude)", 12.5, charte::DOUX)]));
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
    if !reglages.is_empty() {
        reglages_intendant(g, gr, city, reglages);
    }
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
                Reglage::Objectif(x) => (x.nom().to_string(), format!("{} × le besoin{}", format!("{}", x.cible()).replace('.', ","), if *x == intendant::ObjectifNourriture::Excedent { " · excédent perdu tant que le commerce manque" } else { "" })),
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
        l.spawn((Forcage::Proposer(c.hex, depart.saturating_sub(100)), bouton(Color::NONE), children![texte("−100", 13.0, charte::TEXTE)]));
        l.spawn(texte(rules::milliers(depart.into()), 13.5, charte::TEXTE));
        l.spawn((Forcage::Proposer(c.hex, (depart + 100).min(c.capacite)), bouton(Color::NONE), children![texte("+100", 13.0, charte::TEXTE)]));
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
fn file_et_projet(g: &mut ChildSpawnerCommands, city: &City) {
    let c = &city.state;
    let prod = city.travail.rendements.production;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
            t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte("File de construction", 15.0, charte::TEXTE)]));
            t.spawn((BoutonConstruire, bouton(charte::ACTION_FOND), children![texte("Construire (B)", 13.0, charte::TEXTE)]));
        });
        if c.file.is_empty() {
            p.spawn(texte("Vide : la ville suit son projet ; sans projet, le tour ne peut pas finir.", 13.0, charte::MUET));
        }
        for (i, k) in c.file.iter().enumerate() {
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(4.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn(texte(format!("{}.", i + 1), 13.0, charte::MUET));
                l.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                    n.spawn(texte(capitale(k.objet.name()), 13.5, charte::TEXTE));
                    let tours = c.tours_pour(i, prod).map_or("jamais au rythme actuel".to_string(), |t| if t == 0 { "prête".into() } else { format!("prête dans {t} tour(s)") });
                    n.spawn(texte(format!("{} / {} de production · {tours}", k.investi.floor(), k.objet.cost()), 12.0, charte::MUET));
                });
                l.spawn(barre((k.investi / k.objet.cost().max(1) as f64) as f32, 70.0, Grandeur::Production.couleur()));
                if i > 0 {
                    l.spawn((ActionFile::Monter(i), bouton(Color::NONE), children![texte("avancer", 12.5, charte::DOUX)]));
                }
                let retirer = if k.investi > 0.0 { "retirer (production perdue)" } else { "retirer" };
                l.spawn((ActionFile::Retirer(i), bouton(Color::NONE), children![texte(retirer, 12.5, charte::DOUX)]));
            });
        }
        let projet = match c.projet {
            Some(pj) => format!("Quand la file est vide : projet « {} » ({})", pj.nom(), pj.effet()),
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
fn catalogue(l: &mut ChildSpawnerCommands, map: &crate::map::WorldMap, city: &City, cases: &[CaseVue], placement: Option<Amenagement>) {
    let prod = city.travail.rendements.production;
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
    groupe(l, "Aménagements de production : un par case, l'intendant y met ensuite les travailleurs");
    for a in Amenagement::ALL {
        let meilleure = conseil(map, cases, a).and_then(|h| cases.iter().find(|c| c.hex == h));
        let detail = match meilleure.and_then(|c| gain_amenagement(map, c, a).ok().map(|g| (c, g))) {
            Some((c, g)) => format!("conseil : {} · {} à pleine capacité · {} production", c.nom.to_lowercase(), ecarts(&g.plein), g.cout),
            None => "aucune case possible dans la ville".into(),
        };
        item(l, Element::Amenagement(a), capitale(a.nom()), detail, meilleure.is_some(), placement == Some(a));
    }
    // Projets de ville
    groupe(l, "Projets de ville : sans fin, ils reçoivent la production quand la file est vide");
    for pj in crate::rules::Projet::TOUS {
        let detail = if pj.disponible() { pj.effet().to_string() } else { format!("{} : pas encore dans le jeu", pj.effet()) };
        item(l, Element::Projet(pj), pj.nom().into(), detail, pj.disponible(), city.state.projet == Some(pj));
    }
    // Unités
    groupe(l, "Unités : ajoutées à la fin de la file");
    for k in crate::rules::UnitKind::ALL {
        let tours = rules::tours_restants(k.cost(), 0.0, prod).map_or("—".to_string(), |t| format!("{t} tour(s) seule"));
        item(l, Element::Unite(k), k.name().into(), format!("{} production · {tours}", k.cost()), true, false);
    }
    groupe(l, "Ponts : l'emplacement se choisit sur la carte du monde (bouton « Pont… » du panneau de production)");
}

/// Fiche du placement d'un aménagement sur une case.
fn placement(g: &mut ChildSpawnerCommands, map: &crate::map::WorldMap, city: &City, a: Amenagement, c: &CaseVue, conseil_seul: bool) {
    let Ok(gain) = gain_amenagement(map, c, a) else { return };
    let prod = city.travail.rendements.production;
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
        ligne_valeur(p, &format!("Tout de suite ({} travailleurs)", rules::milliers(c.travailleurs.into())), ecarts(&gain.maintenant), charte::TEXTE);
        ligne_valeur(p, &format!("Case pleine ({} travailleurs)", rules::milliers(gain.capacite.into())), ecarts(&gain.plein), charte::AMENAGEMENT);
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
        p.spawn(texte("Sur la carte : survol d'une case pour son gain, clic pour la choisir, double-clic ou Entrée pour la mettre en file. Échap : étape précédente.", 12.5, charte::MUET));
        p.spawn((MettreEnFile, bouton(charte::ACTION_FOND), children![texte(format!("Mettre en file · {}", c.nom), 14.0, charte::TEXTE)]));
    });
}

/// Actions qui changent la ville : mettre en file, avancer, retirer, unités et projets du catalogue.
#[allow(clippy::too_many_arguments)]
fn construire_actions(
    mut ecran: ResMut<EcranVille>,
    replay: Res<crate::feedback::TurnReplay>,
    (map, territoire, amenagements): (Res<GameMap>, Res<Territory>, Res<Amenagements>),
    mut cities: Query<(&mut City, &HexPos)>,
    (mut log, state): (ResMut<crate::game::GameLog>, Res<GameState>),
    catalogue: Query<(Ref<UiPointer>, &ChoixCatalogue)>,
    actions: Query<(Ref<UiPointer>, &ActionFile)>,
    (mettre, arreter): (Query<Ref<UiPointer>, With<MettreEnFile>>, Query<Ref<UiPointer>, With<ArreterProjet>>),
    (reglages, forcages): (Query<(Ref<UiPointer>, &ChoixReglage)>, Query<(Ref<UiPointer>, &Forcage)>),
) {
    let Some(v) = ecran.ville else { return };
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    // (pas d'ordre pendant que le tour se joue)
    if replay.playing {
        ecran.confirmer = false;
        return;
    }
    let Ok((mut city, centre)) = cities.get_mut(v) else { return };
    let centre = centre.0;
    let nom = city.state.name.clone();
    let mut journal = |t: String| log.push(crate::game::LogText::new().city(&nom, v, centre).text(t));
    // Mettre en file l'aménagement placé (bouton, double-clic, Entrée)
    let confirmer = std::mem::take(&mut ecran.confirmer) || mettre.iter().any(|i| appui(&i));
    if confirmer && let Some(a) = ecran.placement {
        let cases = cases_vues(&map.0, centre, v, &city, &territoire, &amenagements);
        let cible = ecran.cible.or_else(|| conseil(&map.0, &cases, a));
        if let Some(c) = cible.and_then(|h| cases.iter().find(|c| c.hex == h))
            && let Ok(g) = gain_amenagement(&map.0, c, a)
        {
            let objet = rules::Production::Amenagement { hex: c.hex, kind: a, cost: g.cout };
            if city.state.mettre_en_file(objet) {
                journal(format!(" met en file : {} ({}), position {}.", a.nom(), c.nom.to_lowercase(), city.state.file.len()));
            } else {
                journal(format!(" : un aménagement est déjà en file sur {}.", c.nom.to_lowercase()));
            }
            ecran.placement = None;
            ecran.cible = None;
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
            Element::Amenagement(_) => {}
        }
    }
    for (i, a) in &actions {
        if !appui(&i) {
            continue;
        }
        match *a {
            ActionFile::Monter(k) if k > 0 && k < city.state.file.len() => city.state.file.swap(k - 1, k),
            ActionFile::Retirer(k) if k < city.state.file.len() => {
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
