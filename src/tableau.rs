//! Tableau de bord des ressources du pays (étape 5, décision de Benjamin du 13/11/2026 : « où en est
//! chaque ressource dans son pays et dans les villes », sans masquer la carte). Un panneau à droite,
//! repliable (bouton « Ressources (G) » ou touche G) : une ligne par ressource (stock du pays, extrait
//! et demandé par tour, part couverte) ; un clic sur une ressource la choisit : le détail par ville
//! s'ouvre, et la carte montre sa disponibilité (ses gisements connus, l'état de chaque ville).

use crate::game::{City, GameState};
use crate::gisements::Ressource;
use crate::pointer::UiPointer;
use bevy::prelude::*;

/// Le tableau de bord : ouvert ou non, et la ressource choisie (sa carte de disponibilité).
#[derive(Resource, Default)]
pub struct Tableau {
    pub ouvert: bool,
    pub choisie: Option<Ressource>,
}

/// Bouton du tableau de bord (colonne de droite).
#[derive(Component)]
pub struct TableauBouton;
/// Panneau du tableau de bord.
#[derive(Component)]
struct TableauPanneau;
/// Ligne d'une ressource (clic : la choisir).
#[derive(Component)]
struct LigneRessource(Ressource);
/// Bouton de fermeture.
#[derive(Component)]
struct FermerTableau;

const FOND: Color = Color::srgba(0.07, 0.09, 0.13, 0.94);
const LIGNE: Color = Color::srgba(1.0, 1.0, 1.0, 0.06);
const LIGNE_SURVOL: Color = Color::srgba(1.0, 1.0, 1.0, 0.14);
const LIGNE_CHOISIE: Color = Color::srgba(0.42, 0.62, 1.0, 0.28);
const TEXTE: Color = Color::srgb(0.92, 0.92, 0.95);
const MUET: Color = Color::srgb(0.62, 0.65, 0.72);
const OK: Color = Color::srgb(0.31, 0.76, 0.63);
const GRAVE: Color = Color::srgb(0.94, 0.54, 0.54);

/// État d'une ville pour une ressource : en trop, suffit, manque, sans objet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EtatRessource {
    EnTrop,
    Suffit,
    Manque,
    SansObjet,
}

impl EtatRessource {
    /// D'après le stock et la demande par tour (réserve et seuil des échanges).
    pub fn de(stock: f64, demande: f64) -> Self {
        if demande <= 0.0 {
            if stock > 0.005 { EtatRessource::EnTrop } else { EtatRessource::SansObjet }
        } else if stock < crate::echanges::TOURS_VISES * demande - 1e-9 {
            EtatRessource::Manque
        } else if stock > crate::echanges::TOURS_RESERVE * demande + 1e-9 {
            EtatRessource::EnTrop
        } else {
            EtatRessource::Suffit
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            EtatRessource::EnTrop => "en trop",
            EtatRessource::Suffit => "suffit",
            EtatRessource::Manque => "manque",
            EtatRessource::SansObjet => "—",
        }
    }

    pub fn couleur(self) -> Color {
        match self {
            EtatRessource::EnTrop => OK,
            EtatRessource::Suffit => TEXTE,
            EtatRessource::Manque => GRAVE,
            EtatRessource::SansObjet => MUET,
        }
    }
}

pub struct TableauPlugin;

impl Plugin for TableauPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tableau>().add_systems(Startup, creer_panneau).add_systems(Update, (commandes, remplir).chain());
    }
}

fn creer_panneau(mut commands: Commands) {
    commands.spawn((
        TableauPanneau,
        UiPointer::default(),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(8.0),
            bottom: Val::Px(380.0),
            width: Val::Px(430.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::all(Val::Px(10.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(FOND),
    ));
}

/// Bouton, touche G, fermeture, choix d'une ressource.
#[allow(clippy::type_complexity)]
fn commandes(
    keys: Res<ButtonInput<KeyCode>>,
    mut tableau: ResMut<Tableau>,
    ecran: Res<crate::ecran_ville::EcranVille>,
    mut bouton: Query<(Ref<UiPointer>, &mut BackgroundColor), With<TableauBouton>>,
    fermer: Query<Ref<UiPointer>, With<FermerTableau>>,
    mut lignes: Query<(Ref<UiPointer>, &LigneRessource, &mut BackgroundColor), Without<TableauBouton>>,
) {
    let appui = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    if keys.just_pressed(KeyCode::KeyG) && !ecran.ouvert() {
        tableau.ouvert = !tableau.ouvert;
    }
    for (i, mut bg) in &mut bouton {
        if appui(&i) {
            tableau.ouvert = !tableau.ouvert;
        }
        let c = if tableau.ouvert { Color::srgb(0.24, 0.36, 0.62) } else if *i == UiPointer::None { Color::srgba(0.12, 0.14, 0.2, 0.92) } else { Color::srgba(0.2, 0.24, 0.32, 0.95) };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    if fermer.iter().any(|i| appui(&i)) {
        tableau.ouvert = false;
    }
    if !tableau.ouvert && tableau.choisie.is_some() {
        tableau.choisie = None;
    }
    let mut choix = None;
    for (i, ligne, mut bg) in &mut lignes {
        if appui(&i) {
            choix = Some(ligne.0);
        }
        let c = if tableau.choisie == Some(ligne.0) { LIGNE_CHOISIE } else if *i == UiPointer::None { LIGNE } else { LIGNE_SURVOL };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    if let Some(r) = choix {
        tableau.choisie = if tableau.choisie == Some(r) { None } else { Some(r) };
    }
}

fn texte(s: impl Into<String>, taille: f32, couleur: Color) -> impl Bundle {
    (Text::new(s.into()), TextFont::from_font_size(taille), TextColor(couleur))
}

fn decimal(x: f64) -> String {
    if x.abs() < 1.0 && x != 0.0 { format!("{x:.2}").replace('.', ",") } else { format!("{x:.1}").replace('.', ",") }
}

/// Contenu du panneau, refait quand le tableau, le tour ou une ville change.
fn remplir(
    mut commands: Commands,
    tableau: Res<Tableau>,
    state: Res<GameState>,
    cities: Query<Ref<City>>,
    panneau: Single<(Entity, &mut Node), With<TableauPanneau>>,
    mut dernier: Local<Option<(bool, Option<Ressource>, u32)>>,
) {
    let (panneau, mut node) = panneau.into_inner();
    let display = if tableau.ouvert { Display::Flex } else { Display::None };
    if node.display != display {
        node.display = display;
    }
    let cle = (tableau.ouvert, tableau.choisie, state.turn);
    if !tableau.ouvert || (*dernier == Some(cle) && !cities.iter().any(|c| c.is_changed())) {
        return;
    }
    *dernier = Some(cle);
    commands.entity(panneau).despawn_related::<Children>();
    let mut villes: Vec<&City> = cities.iter().map(|c| c.into_inner()).collect();
    villes.sort_by(|a, b| a.state.name.cmp(&b.state.name));
    commands.entity(panneau).with_children(|p| {
        p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, margin: UiRect::bottom(Val::Px(4.0)), ..default() }).with_children(|t| {
            t.spawn(texte("Ressources du pays", 17.0, TEXTE));
            t.spawn((FermerTableau, UiPointer::default(), Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(LIGNE), children![texte("Fermer (G)", 12.5, MUET)]));
        });
        if villes.is_empty() {
            p.spawn(texte("Aucune ville : fondez-en une pour exploiter les gisements.", 13.0, MUET));
            return;
        }
        p.spawn(Node { column_gap: Val::Px(6.0), ..default() }).with_children(|l| {
            l.spawn((Node { width: Val::Px(150.0), ..default() }, children![texte("", 11.5, MUET)]));
            for t in ["stock", "extrait", "demande", "couvert"] {
                l.spawn((Node { width: Val::Px(58.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(t, 11.5, MUET)]));
            }
        });
        for r in Ressource::TOUTES {
            let stock: f64 = villes.iter().map(|c| c.state.ressource(r)).sum();
            let extrait: f64 = villes.iter().map(|c| c.state.extrait(r)).sum();
            let demande: f64 = villes.iter().map(|c| c.state.demande_echange(r)).sum();
            let manquent = villes.iter().filter(|c| EtatRessource::de(c.state.ressource(r), c.state.demande_echange(r)) == EtatRessource::Manque).count();
            let couvert = if demande > 0.0 { format!("{:.0} %", (stock / demande / crate::echanges::TOURS_VISES).min(1.0) * 100.0) } else { "—".to_string() };
            let couleur = if demande <= 0.0 { MUET } else if manquent > 0 { GRAVE } else { OK };
            p.spawn((LigneRessource(r), UiPointer::default(), Node { column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), align_items: AlignItems::Center, ..default() }, BackgroundColor(LIGNE)))
                .with_children(|l| {
                    let (cr, cg, cb) = r.couleur();
                    l.spawn((Node { width: Val::Px(150.0), column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }, children![
                        (Node { width: Val::Px(10.0), height: Val::Px(10.0), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(Color::srgb(cr, cg, cb))),
                        texte(capitale(r.nom()), 13.0, TEXTE),
                    ]));
                    for (v, c) in [(decimal(stock), TEXTE), (format!("+{}", decimal(extrait)), if extrait > 0.0 { OK } else { MUET }), (if demande > 0.0 { format!("−{}", decimal(demande)) } else { "—".to_string() }, MUET), (couvert, couleur)] {
                        l.spawn((Node { width: Val::Px(58.0), justify_content: JustifyContent::FlexEnd, ..default() }, children![texte(v, 13.0, c)]));
                    }
                });
            if tableau.choisie == Some(r) {
                for c in &villes {
                    let (s, d) = (c.state.ressource(r), c.state.demande_echange(r));
                    let etat = EtatRessource::de(s, d);
                    let echanges: Vec<String> = c.state.echanges.iter().filter(|x| x.ressource == r).map(|x| format!("{} {} {}", if x.recu { "reçu de" } else { "donné à" }, x.ville, decimal(x.qte))).collect();
                    let suite = if echanges.is_empty() { String::new() } else { format!(" · {}", echanges.join(", ")) };
                    p.spawn((Node { padding: UiRect::new(Val::Px(22.0), Val::Px(4.0), Val::Px(1.0), Val::Px(1.0)), column_gap: Val::Px(8.0), ..default() }, children![
                        texte(format!("{} · stock {} · extrait {} · demande {}{suite}", c.state.name, decimal(s), decimal(c.state.extrait(r)), decimal(d)), 12.0, TEXTE),
                        texte(etat.nom(), 12.0, etat.couleur()),
                    ]));
                }
                p.spawn((Node { padding: UiRect::left(Val::Px(22.0)), ..default() }, children![texte(format!("Usage : {}. Sur la carte : ses gisements connus et l'état de chaque ville.", r.usage()), 11.5, MUET)]));
            }
        }
        p.spawn((Node { margin: UiRect::top(Val::Px(4.0)), ..default() }, children![texte("Clic sur une ressource : le détail par ville et sa carte de disponibilité. « Couvert » : part de 2 tours de demande en stock. Une ville manque sous 2 tours de sa demande et partage au-delà de 6.", 11.5, MUET)]));
    });
}

fn capitale(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etat_d_une_ville_pour_une_ressource() {
        assert_eq!(EtatRessource::de(0.1, 0.1), EtatRessource::Manque);
        assert_eq!(EtatRessource::de(0.3, 0.1), EtatRessource::Suffit);
        assert_eq!(EtatRessource::de(0.7, 0.1), EtatRessource::EnTrop);
        assert_eq!(EtatRessource::de(2.0, 0.0), EtatRessource::EnTrop);
        assert_eq!(EtatRessource::de(0.0, 0.0), EtatRessource::SansObjet);
    }
}
