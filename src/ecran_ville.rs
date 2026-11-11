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
}

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

/// Systèmes de l'écran (avant le menu Échap, qui ne doit pas s'ouvrir quand Échap ferme l'écran).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EcranVilleSet;

pub struct EcranVillePlugin;

impl Plugin for EcranVillePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EcranVille>()
            .init_resource::<VueCarte>()
            .add_systems(Startup, construire)
            .add_systems(
                Update,
                (ouvrir_fermer, commandes, carte_souris, reconstruire, placer_plan, afficher)
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
    let mut cases = vec![CaseVue { hex: centre, local: Hex::new(0, 0), centre: true, nom: "Centre de la ville".into(), travailleurs: 0, capacite: 0, max: rc, rendement: rc, amenagement: None }];
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
        });
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
                if ecran.case.is_some() && ville == ecran.ville {
                    ecran.case = None;
                    return;
                }
            }
            ecran.ville = None;
            ecran.case = None;
        }
    } else if let Some(v) = ville
        && !wiki.open
        && !menu.open
        && (keys.just_pressed(KeyCode::KeyC) || ouvrir.iter().any(|i| appui(&i)))
    {
        ecran.ville = Some(v);
        ecran.case = None;
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
    mut survols: Query<(&UiPointer, &mut BackgroundColor), (Or<(With<ChoixCase>, With<BoutonZoom>, With<Fermer>, With<FermerFiche>, With<BasculeAmenagements>)>, Changed<UiPointer>)>,
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
            let hex = cases.iter().find(|x| x.local == local).map(|x| x.hex);
            ecran.case = if hex.is_some() && hex == ecran.case { None } else { hex };
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
                commands.entity(bulle_e).with_children(|b| infobulle(b, c, ecran.onglet));
            }
        }
        _ => {
            bulle_node.display = Display::None;
            *montre = None;
        }
    }
}

/// Contenu de l'infobulle d'une case, selon la carte affichée.
fn infobulle(b: &mut ChildSpawnerCommands, c: &CaseVue, onglet: Onglet) {
    b.spawn(texte(c.nom.clone(), 14.0, charte::TEXTE));
    if onglet == Onglet::Population {
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
    cadre: Single<&ComputedNode, With<Cadre>>,
    mut textes: Query<&mut Text>,
    mut derniere: Local<(Vec2, f32, u32)>,
) {
    let Some(v) = ecran.ville else { return };
    let Ok((city, centre)) = cities.get(v) else { return };
    let taille = cadre.size() * cadre.inverse_scale_factor();
    let cle = (taille, vue.zoom, state.turn);
    let change = ecran.is_changed() || city.is_changed() || amenagements.is_changed() || territoire.is_changed() || *derniere != cle;
    if !change || taille.x < 10.0 {
        return;
    }
    *derniere = cle;
    let cases = cases_vues(&map.0, centre.0, v, &city, &territoire, &amenagements);
    let choisie = ecran.case.and_then(|h| cases.iter().find(|c| c.hex == h));

    commands.entity(*entete).despawn_related::<Children>().with_children(|e| en_tete(e, &city, &images));
    commands.entity(*gauche).despawn_related::<Children>().with_children(|g| match (ecran.onglet, choisie) {
        (_, Some(c)) => fiche_case(g, c, &map.0, &images),
        (Onglet::Rendements, None) => rendements(g, &city, &cases, &images),
        (Onglet::Population, None) => population(g, &city, &images),
        (Onglet::Commerce, None) => vide(g, "Commerce", "Aucun axe de commerce : les échanges entre villes ne sont pas encore dans le jeu."),
        (Onglet::Histoire, None) => vide(g, "Histoire", "Aucun événement enregistré : la chronique des villes n'est pas encore tenue."),
    });
    let mode_pop = ecran.onglet == Onglet::Population;
    commands.entity(*legende).despawn_related::<Children>().with_children(|l| legende_carte(l, mode_pop, ecran.amenagements_caches));
    let s = rayon_hex(taille, &cases, vue.zoom);
    commands.entity(*plan).despawn_related::<Children>().with_children(|p| {
        for c in &cases {
            hexagone(p, c, s, mode_pop, !ecran.amenagements_caches, &images);
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
    let titre_liste = match ecran.onglet {
        Onglet::Rendements => "Cases travaillées",
        Onglet::Population => "Cases par remplissage",
        Onglet::Commerce => "Axes de commerce",
        Onglet::Histoire => "Lieux de l'histoire",
    };
    if let Ok(mut t) = textes.get_mut(*titre) {
        t.0 = titre_liste.into();
    }
    commands.entity(*liste).despawn_related::<Children>().with_children(|l| liste_droite(l, ecran.onglet, &cases, ecran.case));
}

fn melange(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t)
}

/// Un hexagone de la carte de la ville.
fn hexagone(p: &mut ChildSpawnerCommands, c: &CaseVue, s: f32, mode_pop: bool, amenagements: bool, images: &Images) {
    let pos = position(c.local, s);
    let fond = if c.centre {
        charte::CENTRE_VILLE
    } else if mode_pop {
        let part = c.remplissage();
        melange(charte::SURFACE, if part >= 0.9 { charte::ALERTE } else { charte::OK }, 0.15 + 0.45 * part)
    } else {
        match c.dominante() {
            Some(g) => melange(charte::SURFACE, g.couleur(), if c.travailleurs > 0 { 0.45 } else { 0.18 }),
            None => charte::SURFACE,
        }
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
fn legende_carte(l: &mut ChildSpawnerCommands, mode_pop: bool, caches: bool) {
    if mode_pop {
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
    let variation = rules::variation_des_habitants(c.habitants, y.nourriture);
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
        let en_cours = match c.producing {
            Some(k) => {
                let tours = rules::tours_restants(k.cost(), c.production_stock, y.production).map_or("jamais au rythme actuel".to_string(), |t| format!("{t} tour(s)"));
                format!("En construction : {} · {}", k.name(), tours)
            }
            None => "Rien en construction".into(),
        };
        l.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|b| {
            b.spawn(icone(images.grandeur(Grandeur::Production), 18.0, Grandeur::Production.couleur()));
            b.spawn(texte(en_cours, 14.0, charte::TEXTE));
        });
        l.spawn((Fermer, bouton(charte::SURFACE), children![texte("Fermer (Échap)", 14.0, charte::TEXTE)]));
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
fn carte_grandeur(r: &mut ChildSpawnerCommands, images: &Images, g: Grandeur, valeur: String, unite: &str, etat: Etat, ligne: String) {
    r.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, flex_basis: Val::Px(0.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(9.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() }, BackgroundColor(charte::SURFACE)))
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

/// Onglet Rendements : les trois grandeurs principales, celles qui ne sont pas encore dans le jeu,
/// et d'où vient chaque grandeur.
fn rendements(g: &mut ChildSpawnerCommands, city: &City, cases: &[CaseVue], images: &Images) {
    let c = &city.state;
    let y = city.travail.rendements;
    let couverture = y.nourriture / c.besoin_nourriture().max(0.001);
    let etat_n = if couverture < 1.0 { Etat::Grave } else if couverture < 1.5 { Etat::Alerte } else { Etat::Ok };
    let suite_n = if couverture < 1.0 { " : famine" } else { "" };
    let prod = match c.producing {
        Some(k) => format!("{} dans {}", k.name(), rules::tours_restants(k.cost(), c.production_stock, y.production).map_or("—".into(), |t| format!("{t} tour(s)"))),
        None => "rien en construction".into(),
    };
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|r| {
            carte_grandeur(r, images, Grandeur::Nourriture, rules::decimal(y.nourriture), "par tour", etat_n, format!("{} × le besoin{suite_n}", rules::decimal(couverture)));
            carte_grandeur(r, images, Grandeur::Production, rules::decimal(y.production), "par tour", Etat::Ok, prod);
            carte_grandeur(r, images, Grandeur::Deniers, format!("+{}", rules::decimal(y.deniers)), "par tour", Etat::Ok, "versés au trésor".into());
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
    // D'où viennent les grandeurs
    let centre = cases.iter().find(|c| c.centre).map_or(Rendements::default(), |c| c.rendement);
    let par_cases = y - centre;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        titre(p, &format!("D'où viennent les rendements de {}", c.name));
        let colonnes = ["", "Centre", "Cases", "Bâtiments", "Commerce", "Total"];
        let ligne = |p: &mut ChildSpawnerCommands, cellules: Vec<(String, Color)>| {
            p.spawn((Node { column_gap: Val::Px(6.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                for (i, (t, couleur)) in cellules.into_iter().enumerate() {
                    l.spawn((Node { width: Val::Percent(if i == 0 { 25.0 } else { 15.0 }), justify_content: if i == 0 { JustifyContent::FlexStart } else { JustifyContent::FlexEnd }, ..default() }, children![texte(t, 13.5, couleur)]));
                }
            });
        };
        ligne(p, colonnes.iter().map(|c| (c.to_string(), charte::MUET)).collect());
        for (g, total, c0, cs) in [
            (Grandeur::Nourriture, y.nourriture, centre.nourriture, par_cases.nourriture),
            (Grandeur::Production, y.production, centre.production, par_cases.production),
            (Grandeur::Deniers, y.deniers, centre.deniers, par_cases.deniers),
        ] {
            ligne(p, vec![(g.nom().into(), g.couleur()), (rules::decimal(c0), charte::TEXTE), (rules::decimal(cs), charte::TEXTE), ("—".into(), charte::MUET), ("—".into(), charte::MUET), (rules::decimal(total), charte::TEXTE)]);
        }
        p.spawn(texte("Bâtiments et commerce : pas encore dans le jeu. Le centre de la ville rapporte sans travailleurs ; les cases, selon les travailleurs que l'intendant y place.", 12.5, charte::MUET));
    });
}

/// Une ligne « libellé … valeur ».
fn ligne_valeur(p: &mut ChildSpawnerCommands, libelle: &str, valeur: String, couleur: Color) {
    p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
        l.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(libelle, 13.5, charte::TEXTE)]));
        l.spawn(texte(valeur, 13.5, couleur));
    });
}

/// Onglet Population.
fn population(g: &mut ChildSpawnerCommands, city: &City, images: &Images) {
    let c = &city.state;
    let t = &city.travail;
    let employes = t.actifs - t.chomeurs;
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
            r.spawn(icone(images.grandeur(Grandeur::Habitants), 20.0, Grandeur::Habitants.couleur()));
            titre(r, &format!("Population de {}", c.name));
        });
        ligne_valeur(p, "Habitants de l'aire (ville et campagnes)", rules::milliers(c.habitants), charte::TEXTE);
        ligne_valeur(p, "Palier", format!("{} (le suivant à {} habitants)", c.palier(), rules::milliers(rules::habitants_du_palier(c.palier() + 1))), charte::TEXTE);
        ligne_valeur(p, "Actifs (la moitié des habitants)", rules::milliers(t.actifs.into()), charte::TEXTE);
        ligne_valeur(p, "Au travail sur les cases", rules::milliers(employes.into()), charte::TEXTE);
        let (v, couleur) = if t.chomeurs > 0 { (format!("{} : aménager des cases", rules::milliers(t.chomeurs.into())), charte::ALERTE) } else { ("aucun".into(), charte::OK) };
        ligne_valeur(p, "Sans emploi", v, couleur);
        p.spawn(texte("Pour l'instant, les habitants vivent là où ils travaillent.", 12.5, charte::MUET));
    });
    vide(g, "Cultures et religions", "Une seule culture pour l'instant : les cultures et les religions ne sont pas encore dans le jeu.");
    vide(g, "Ordres de la société", "Paysans, artisans, marchands, prêtres, guerriers : pas encore dans le jeu.");
}

/// Fiche d'une case choisie sur la carte ou dans la liste.
fn fiche_case(g: &mut ChildSpawnerCommands, c: &CaseVue, map: &crate::map::WorldMap, images: &Images) {
    g.spawn((panneau(), BackgroundColor(charte::PANNEAU))).with_children(|p| {
        p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|t| {
            t.spawn((Node { flex_grow: 1.0, ..default() }, children![texte(c.nom.clone(), 17.0, charte::TEXTE)]));
            t.spawn((FermerFiche, bouton(charte::SURFACE), children![texte("× retour (Échap)", 13.0, charte::TEXTE)]));
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
        }
        let part = if c.capacite > 0 { (c.remplissage() as f64).powf(intendant::EXPOSANT) } else { 1.0 };
        for (g, v, m) in [(Grandeur::Nourriture, c.rendement.nourriture, c.max.nourriture), (Grandeur::Production, c.rendement.production, c.max.production), (Grandeur::Deniers, c.rendement.deniers, c.max.deniers)] {
            if m < 0.05 {
                continue;
            }
            p.spawn((Node { column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(5.0)), border: UiRect::top(Val::Px(1.0)), align_items: AlignItems::Center, ..default() }, BorderColor::all(charte::TRAIT))).with_children(|l| {
                l.spawn(icone(images.grandeur(g), 16.0, g.couleur()));
                l.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|n| {
                    n.spawn(texte(g.nom(), 13.5, g.couleur()));
                    if c.capacite > 0 {
                        n.spawn(texte(format!("{} % du maximum de la case ({}) : rendements décroissants", (part * 100.0).round(), rules::decimal(m)), 12.0, charte::MUET));
                    }
                });
                l.spawn(texte(rules::decimal(v), 14.0, charte::TEXTE));
            });
        }
        if !c.centre {
            let possibles: Vec<String> = Amenagement::possibles(map, c.hex).into_iter().filter(|a| Some(*a) != c.amenagement).map(|a| format!("{} ({} production)", a.nom().to_lowercase(), a.cout(map, c.hex))).collect();
            let t = if possibles.is_empty() { "Aucun autre aménagement possible ici.".to_string() } else { format!("Aménagements possibles : {}. Leur construction viendra avec la file de construction.", possibles.join(", ")) };
            p.spawn(texte(t, 12.5, charte::MUET));
        }
    });
}

/// Liste de droite : cases travaillées ou par remplissage ; vide pour le commerce et l'histoire.
fn liste_droite(l: &mut ChildSpawnerCommands, onglet: Onglet, cases: &[CaseVue], choisie: Option<Hex>) {
    match onglet {
        Onglet::Commerce => {
            l.spawn(texte("Aucun axe : le commerce n'est pas encore dans le jeu.", 13.0, charte::MUET));
            return;
        }
        Onglet::Histoire => {
            l.spawn(texte("Aucun lieu : l'histoire de la ville n'est pas encore tenue.", 13.0, charte::MUET));
            return;
        }
        _ => {}
    }
    let mut lst: Vec<&CaseVue> = cases.iter().filter(|c| !c.centre && c.capacite > 0).collect();
    if onglet == Onglet::Population {
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
                n.spawn(texte(format!("{} / {} travailleurs{a}", rules::milliers(c.travailleurs.into()), rules::milliers(c.capacite.into())), 12.0, charte::MUET));
            });
            if onglet == Onglet::Population {
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
