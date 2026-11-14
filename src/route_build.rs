//! Traceur de route (voir `docs/conception/conception-routes.md`).
//!
//! Le bouton « Chemin… » du catalogue de la ville met la carte en mode tracé (l'écran de la ville
//! se ferme) : le tracé le plus rapide une fois construit va de la ville à la case survolée. Un
//! clic fixe l'arrivée ; chaque clic suivant ajoute un point de passage (le tracé y passe) ; clic
//! droit : retire le dernier point (ou l'arrivée, ou sort du mode) ; Entrée : la ville lance le
//! chantier, construit tronçon par tronçon au fil de sa production (`game::end_turn`). Un panneau
//! en haut de l'écran donne le coût, les tours de production et le gain des porteurs.

use crate::feedback::TurnReplay;
use crate::game::{City, GameLog, GameMap, HexPos, LogText, Selection, ShownFog};
use crate::hex::Hex;
use crate::map::WorldMap;
use crate::pointer::UiPointer;
use crate::routes::{self, NIVEAU_CHEMIN};
use crate::view3d::{Gizmos3d, Relief, ViewMode};
use bevy::prelude::*;

/// Ville qui trace une route, ses points (arrivée en dernier), la case survolée et le tracé.
#[derive(Resource, Default)]
pub struct TraceRoute {
    pub city: Option<Entity>,
    pub centre: Hex,
    /// Points de passage, l'arrivée en dernier (vide : l'arrivée suit le curseur).
    pub points: Vec<Hex>,
    pub survol: Option<Hex>,
    /// Tracé calculé (de la ville à l'arrivée), et la clé qui l'a donné.
    pub trace: Option<Vec<Hex>>,
    cle: Vec<Hex>,
}

impl TraceRoute {
    pub fn active(&self) -> bool {
        self.city.is_some()
    }

    /// Les points du tracé : la ville, les points de passage, l'arrivée (fixée, sinon le survol).
    fn cle(&self) -> Vec<Hex> {
        let mut v = vec![self.centre];
        v.extend(self.points.iter().copied());
        if self.points.is_empty() {
            v.extend(self.survol);
        }
        v
    }
}

/// Hors du mode tracé (les clics et les touches de la carte reprennent leur rôle habituel).
pub fn not_tracing(trace: Res<TraceRoute>) -> bool {
    !trace.active()
}

/// Bouton « Chemin… » du catalogue de la ville.
#[derive(Component)]
pub struct RouteButton;

/// Panneau d'aperçu du tracé (haut de l'écran).
#[derive(Component)]
struct PanneauTrace;

pub struct RouteBuildPlugin;

impl Plugin for RouteBuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TraceRoute>().add_systems(Startup, creer_panneau).add_systems(
            Update,
            (route_button, (survol, calculer, cliquer, panneau).chain().after(crate::input::InputSet), routes_2d, routes_3d),
        );
    }
}

fn creer_panneau(mut commands: Commands) {
    commands.spawn((
        PanneauTrace,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(64.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-330.0)),
            width: Val::Px(660.0),
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(crate::charte::SURFACE),
        GlobalZIndex(90),
        children![crate::charte::texte("", 14.0, crate::charte::TEXTE)],
    ));
}

/// Clic sur le bouton « Chemin… » : entre dans le mode tracé (l'écran de la ville se ferme).
fn route_button(
    buttons: Query<&UiPointer, (Changed<UiPointer>, With<RouteButton>)>,
    selection: Res<Selection>,
    replay: Res<TurnReplay>,
    cities: Query<(Entity, &HexPos), With<City>>,
    mut trace: ResMut<TraceRoute>,
    mut ecran: ResMut<crate::ecran_ville::EcranVille>,
) {
    if !buttons.iter().any(|i| *i == UiPointer::Pressed) || replay.playing {
        return;
    }
    let Some((city, pos)) = selection.0.and_then(|e| cities.get(e).ok()) else { return };
    *trace = TraceRoute { city: Some(city), centre: pos.0, ..default() };
    ecran.ville = None;
    ecran.catalogue = false;
}

/// Case survolée, en mode tracé.
fn survol(hovered: Res<crate::input::HoveredHex>, mut trace: ResMut<TraceRoute>) {
    if trace.active() && trace.survol != hovered.0 {
        trace.survol = hovered.0;
    }
}

/// Tracé recalculé quand ses points changent (par les cases explorées seulement).
fn calculer(map: Res<GameMap>, fog: Res<ShownFog>, mut trace: ResMut<TraceRoute>) {
    if !trace.active() {
        return;
    }
    let cle = trace.cle();
    if cle == trace.cle {
        return;
    }
    let connu = |h: Hex| fog.explored.contains(&h);
    trace.trace = if cle.len() >= 2 { routes::tracer(&map.0, &cle, NIVEAU_CHEMIN, &connu) } else { None };
    trace.cle = cle;
}

/// Où insérer un point de passage : là où il allonge le moins le chemin à vol d'oiseau.
pub fn rang_du_point(map: &WorldMap, centre: Hex, points: &[Hex], p: Hex) -> usize {
    let d = |a: Hex, b: Hex| map.wrap().distance(a, b);
    let mut tous = vec![centre];
    tous.extend_from_slice(points);
    (0..points.len()).min_by_key(|&i| d(tous[i], p) + d(p, tous[i + 1]) - d(tous[i], tous[i + 1])).unwrap_or(points.len())
}

/// Clic gauche : fixe l'arrivée, puis ajoute des points de passage ; clic droit : retire le dernier
/// point (ou sort du mode) ; Entrée : lance le chantier.
#[allow(clippy::too_many_arguments)]
fn cliquer(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    map: Res<GameMap>,
    over_ui: Res<crate::input::PointerOverUi>,
    mut trace: ResMut<TraceRoute>,
    mut cities: Query<(&mut City, &HexPos)>,
    mut log: ResMut<GameLog>,
    mut pilot: ResMut<crate::input::Pilot>,
) {
    let Some(city_entity) = trace.city else { return };
    let click = pilot.click.take();
    let pressed = |b: MouseButton| mouse.just_pressed(b) || click == Some(b);
    if selection.0 != Some(city_entity) {
        *trace = TraceRoute::default();
        return;
    }
    if pressed(MouseButton::Right) {
        if trace.points.pop().is_none() {
            *trace = TraceRoute::default();
        }
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let Some(t) = trace.trace.clone().filter(|t| t.len() >= 2 && !trace.points.is_empty()) else { return };
        let total = routes::cout_trace(&map.0, &t, NIVEAU_CHEMIN).round() as i32;
        if let Ok((mut city, pos)) = cities.get_mut(city_entity) {
            if total > 0 {
                city.state.lancer_route(t.clone(), NIVEAU_CHEMIN, total);
                let fin = map.0.describe(*t.last().unwrap()).to_lowercase();
                log.push(LogText::new().city(&city.state.name, city_entity, pos.0).text(format!(" met en file : chemin jusqu'à {fin} ({} tronçons, {total}).", t.len() - 1)));
            } else {
                log.push(LogText::new().city(&city.state.name, city_entity, pos.0).text(" : ce chemin existe déjà."));
            }
        }
        *trace = TraceRoute::default();
        return;
    }
    if !pressed(MouseButton::Left) || over_ui.0 {
        return;
    }
    let Some(h) = trace.survol.map(|h| map.0.canonical(h)) else { return };
    if h == trace.centre || trace.points.contains(&h) {
        return;
    }
    // (l'arrivée, puis les points de passage, rangés entre les points voisins)
    if trace.points.is_empty() {
        trace.points.push(h);
    } else {
        let i = rang_du_point(&map.0, trace.centre, &trace.points, h);
        trace.points.insert(i, h);
    }
}

/// Texte du panneau d'aperçu.
fn texte_apercu(map: &WorldMap, trace: &TraceRoute, production: f64) -> String {
    let aide = "clic : arrivée, puis points de passage · clic droit : retirer · Entrée : lancer";
    let Some(t) = trace.trace.as_ref().filter(|t| t.len() >= 2) else {
        return match trace.survol {
            Some(_) if !trace.cle.is_empty() && trace.cle.len() >= 2 => format!("Chemin : pas de tracé jusque-là (cases inconnues, eau, pic ou falaise)\n{aide}"),
            _ => format!("Chemin : survolez la case d'arrivée\n{aide}"),
        };
    };
    let cout = routes::cout_trace(map, t, NIVEAU_CHEMIN);
    let tours = crate::rules::tours_restants(cout.round() as i32, 0.0, production).map_or("—".to_string(), |n| n.to_string());
    let ponts = t.windows(2).filter(|w| routes::pont_requis(map, w[0], w[1]).is_some()).count();
    let porteur = match (routes::tours_porteur(map, t), routes::tours_porteur_avec_route(map, t)) {
        (Some(a), Some(b)) => format!(" · porteur sur ce tracé : {} → {} tour(s)", crate::rules::decimal(a), crate::rules::decimal(b)),
        _ => String::new(),
    };
    let fixe = if trace.points.is_empty() { "" } else { " (arrivée fixée)" };
    let pont = if ponts > 0 { format!(", {ponts} pont(s) de fleuve compris") } else { String::new() };
    format!(
        "Chemin jusqu'à {}{fixe} : {} tronçons · coût {}{pont} · {tours} tour(s) de production{porteur}\n{aide}",
        map.describe(*t.last().unwrap()).to_lowercase(),
        t.len() - 1,
        cout.round()
    )
}

/// Panneau d'aperçu : visible en mode tracé.
fn panneau(
    trace: Res<TraceRoute>,
    map: Res<GameMap>,
    cities: Query<&City>,
    mut panneaux: Query<(&mut Node, &Children), With<PanneauTrace>>,
    mut textes: Query<&mut Text>,
) {
    for (mut node, children) in &mut panneaux {
        let display = if trace.active() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        if !trace.active() {
            continue;
        }
        let production = trace.city.and_then(|e| cities.get(e).ok()).map_or(0.0, |c| c.travail.rendements.production);
        let s = texte_apercu(&map.0, &trace, production);
        if let Some(mut t) = children.first().and_then(|c| textes.get_mut(*c).ok())
            && t.0 != s
        {
            t.0 = s;
        }
    }
}

/// Couleur d'un chemin construit.
const COULEUR_CHEMIN: Color = Color::srgb(0.86, 0.68, 0.42);
/// Couleur du tracé en cours (à construire), et de ce qui est déjà fait sur le tracé.
const COULEUR_TRACE: Color = Color::srgb(1.0, 0.92, 0.4);
const COULEUR_DEJA: Color = Color::srgb(0.85, 0.85, 0.85);

/// Segments à dessiner (repère 2D du monde) : les routes construites (cases explorées) et le
/// tracé en cours, chacun avec sa couleur.
fn segments(map: &WorldMap, fog: &ShownFog, trace: &TraceRoute) -> Vec<(Vec2, Vec2, Color)> {
    let wrap = map.wrap();
    let mut v = Vec::new();
    for (at, _) in map.routes() {
        let (a, b) = at.ends(wrap);
        if !(fog.explored.contains(&map.canonical(a)) && fog.explored.contains(&map.canonical(b))) {
            continue;
        }
        let pa = Vec2::from(a.to_world());
        let pb = Vec2::from(wrap.nearest(a, b).to_world());
        v.push((pa, pb, COULEUR_CHEMIN));
    }
    if let Some(t) = trace.trace.as_ref().filter(|_| trace.active()) {
        let mut prec = t[0];
        for w in t.windows(2) {
            let b = wrap.nearest(prec, w[1]);
            let couleur = if routes::cout_troncon(map, w[0], w[1], NIVEAU_CHEMIN) == 0.0 { COULEUR_DEJA } else { COULEUR_TRACE };
            v.push((Vec2::from(prec.to_world()), Vec2::from(b.to_world()), couleur));
            prec = b;
        }
    }
    v
}

/// Routes et tracé en 2D (le tracé en trois traits, plus visible).
fn routes_2d(map: Res<GameMap>, fog: Res<ShownFog>, trace: Res<TraceRoute>, mode: Res<ViewMode>, mut gizmos: Gizmos) {
    if mode.is_3d {
        return;
    }
    for (a, b, c) in segments(&map.0, &fog, &trace) {
        // (le chemin en deux traits serrés, le tracé en trois traits plus écartés)
        let (ecart, traits): (f32, &[f32]) = if c == COULEUR_CHEMIN { (0.7, &[-1.0, 1.0]) } else { (1.5, &[-1.0, 0.0, 1.0]) };
        let cote = (b - a).normalize_or_zero().perp() * ecart;
        for &k in traits {
            gizmos.line_2d(a + cote * k, b + cote * k, c);
        }
    }
    for &p in trace.points.iter().filter(|_| trace.active()) {
        gizmos.circle_2d(Vec2::from(p.to_world()), 6.0, COULEUR_TRACE);
    }
}

/// Routes et tracé en 3D, posés sur le relief.
fn routes_3d(map: Res<GameMap>, fog: Res<ShownFog>, trace: Res<TraceRoute>, mode: Res<ViewMode>, relief: Option<Res<Relief>>, mut gizmos: Gizmos<Gizmos3d>) {
    let Some(relief) = relief else { return };
    if !mode.is_3d {
        return;
    }
    for (a, b, c) in segments(&map.0, &fog, &trace) {
        let n = ((b - a).length() / 6.0).ceil().max(1.0) as usize;
        let lift = if c == COULEUR_CHEMIN { 0.6 } else { 2.0 };
        let cote = (b - a).normalize_or_zero().perp() * 0.5;
        for k in [-1.0f32, 0.0, 1.0] {
            gizmos.linestrip((0..=n).map(|i| a.lerp(b, i as f32 / n as f32) + cote * k).map(|p| relief.on_ground(p.x, p.y, lift)), c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;

    #[test]
    fn le_point_de_passage_se_range_entre_ses_voisins() {
        let m = WorldMap::filled(30, 12, Terrain::Grassland);
        let centre = Hex::from_offset(2, 5);
        let points = vec![Hex::from_offset(10, 5), Hex::from_offset(20, 5)];
        assert_eq!(rang_du_point(&m, centre, &points, Hex::from_offset(6, 7)), 0);
        assert_eq!(rang_du_point(&m, centre, &points, Hex::from_offset(15, 3)), 1);
    }
}
