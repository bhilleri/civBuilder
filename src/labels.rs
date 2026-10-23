//! Étiquettes à l'écran ancrées sur la carte : bannières des villes (2D et 3D)
//! et numéros de tour le long des trajets (3D ; la 2D a ses propres pastilles).
//!
//! Ce sont des nœuds d'interface placés chaque image à la projection de leur case par la
//! caméra active : ils restent lisibles quel que soit le zoom ou l'inclinaison.
//! Les barres de PV des unités en 3D suivent le même principe (la 2D dessine les siennes).

use crate::game::{City, GameMap, ShownFog, HexPos, Unit};
use crate::hostile::Hostile;
use crate::rules::MAX_HP;
use crate::hex::Hex;
use crate::input::{HoverPath, OrderPath};
use crate::view3d::{CITY_HEIGHT, ViewMode, shown_height, to_3d};
use bevy::camera::CameraUpdateSystems;
use bevy::prelude::*;
use bevy::ui::UiSystems;

const BANNER_BG: Color = Color::srgba(0.05, 0.06, 0.09, 0.85);
const PLAYER_COLOR: Color = Color::srgb(0.20, 0.45, 0.95);
const ORDER_LABEL: Color = Color::srgb(0.16, 0.42, 0.78);
const HOVER_LABEL: Color = Color::srgba(0.05, 0.05, 0.08, 0.9);

/// Point d'ancrage d'une étiquette.
#[derive(Component)]
struct Anchor {
    hex: Hex,
    /// Décalage vertical en vue 2D (unités du monde, vers le haut de l'écran).
    up_2d: f32,
    /// Hauteur au-dessus du relief en vue 3D.
    up_3d: f32,
    /// Étiquette propre à la vue 3D (masquée en 2D).
    only_3d: bool,
    /// Masquée quel que soit l'écran (unité en pleine santé, hors de vue…).
    hidden: bool,
    /// Barre de PV : posée sous le jeton, à une taille qui suit le zoom.
    under_token: bool,
}

impl Anchor {
    fn new(hex: Hex, up_2d: f32, up_3d: f32, only_3d: bool) -> Self {
        Anchor { hex, up_2d, up_3d, only_3d, hidden: false, under_token: false }
    }
}

/// Barre de PV d'une unité en vue 3D.
#[derive(Component)]
struct HpBar3d(Entity);
#[derive(Component)]
struct HpFill3d;

/// Rayon du jeton 3D d'une unité (unités du monde) et hauteur de son centre.
const TOKEN_RADIUS: f32 = 16.0;
const TOKEN_LIFT: f32 = 3.0;
const HP_GOOD: Color = Color::srgb(0.30, 0.85, 0.35);
const HP_MID: Color = Color::srgb(0.95, 0.75, 0.20);
const HP_LOW: Color = Color::srgb(0.95, 0.25, 0.20);

/// Bannière d'une ville : population + nom.
#[derive(Component)]
struct CityBanner(Entity);
#[derive(Component)]
struct BannerName;
#[derive(Component)]
struct BannerPop;

/// Pastille « tour d'arrivée » d'un trajet, en vue 3D.
#[derive(Component)]
struct TurnLabel3d;

pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_city_banners, update_city_banners, update_turn_labels_3d, spawn_hp_bars_3d, update_hp_bars_3d),
        )
        .add_systems(
            PostUpdate,
            place_labels.after(CameraUpdateSystems).before(UiSystems::Prepare),
        );
    }
}

fn spawn_city_banners(mut commands: Commands, cities: Query<(Entity, &City, &HexPos), Added<City>>) {
    for (entity, city, pos) in &cities {
        commands
            .spawn((
                CityBanner(entity),
                // Au-dessus du jeton d'une unité en garnison
                Anchor::new(pos.0, 24.0, CITY_HEIGHT + 14.0, false),
                Node {
                    position_type: PositionType::Absolute,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::new(Val::Px(3.0), Val::Px(8.0), Val::Px(2.0), Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(11.0)),
                    ..default()
                },
                // Ancre au milieu du bord inférieur
                UiTransform::from_translation(Val2::percent(-50.0, -100.0)),
                BackgroundColor(BANNER_BG),
                BorderColor::all(PLAYER_COLOR),
                GlobalZIndex(-1),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        width: Val::Px(18.0),
                        height: Val::Px(18.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(9.0)),
                        ..default()
                    },
                    BackgroundColor(PLAYER_COLOR),
                    children![(
                        BannerPop,
                        Text::new(city.state.population.to_string()),
                        TextFont::from_font_size(12.0),
                        TextColor(Color::WHITE),
                    )],
                ));
                p.spawn((
                    BannerName,
                    Text::new(city.state.name.clone()),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::WHITE),
                ));
            });
    }
}

/// Population et nom à jour ; la bannière disparaît avec sa ville.
fn update_city_banners(
    mut commands: Commands,
    cities: Query<Ref<City>>,
    banners: Query<(Entity, &CityBanner, &Children)>,
    pop_boxes: Query<&Children, Without<CityBanner>>,
    mut pops: Query<&mut Text, (With<BannerPop>, Without<BannerName>)>,
    mut names: Query<&mut Text, (With<BannerName>, Without<BannerPop>)>,
) {
    for (entity, banner, children) in &banners {
        let Ok(city) = cities.get(banner.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !city.is_changed() {
            continue;
        }
        for child in children.iter() {
            if let Ok(mut name) = names.get_mut(child)
                && name.0 != city.state.name
            {
                name.0 = city.state.name.clone();
            }
            for grandchild in pop_boxes.get(child).into_iter().flatten() {
                if let Ok(mut pop) = pops.get_mut(*grandchild) {
                    let wanted = city.state.population.to_string();
                    if pop.0 != wanted {
                        pop.0 = wanted;
                    }
                }
            }
        }
    }
}

/// Pastilles numérotées sur la dernière case atteinte à chaque tour (même logique qu'en 2D).
fn update_turn_labels_3d(
    mut commands: Commands,
    hover: Res<HoverPath>,
    order: Res<OrderPath>,
    labels: Query<Entity, With<TurnLabel3d>>,
) {
    if !hover.is_changed() && !order.is_changed() {
        return;
    }
    for e in &labels {
        commands.entity(e).despawn();
    }
    spawn_turn_labels(&mut commands, &order.path, &order.turns, ORDER_LABEL);
    if hover.path != order.path {
        spawn_turn_labels(&mut commands, &hover.path, &hover.turns, HOVER_LABEL);
    }
}

fn spawn_turn_labels(commands: &mut Commands, path: &[Hex], turns: &[u32], background: Color) {
    for (i, (&hex, &turn)) in path.iter().zip(turns).enumerate() {
        let last_of_turn = turns.get(i + 1).is_none_or(|&next| next != turn);
        if !last_of_turn {
            continue;
        }
        commands.spawn((
            TurnLabel3d,
            // Juste au-dessus de la ligne du trajet
            Anchor::new(hex, 0.0, 7.0, true),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            BackgroundColor(background),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            GlobalZIndex(-1),
            Visibility::Hidden,
            children![(Text::new(turn.to_string()), TextFont::from_font_size(12.0), TextColor(Color::WHITE))],
        ));
    }
}

fn spawn_hp_bars_3d(mut commands: Commands, units: Query<(Entity, &HexPos), Added<Unit>>) {
    for (unit, pos) in &units {
        commands.spawn((
            HpBar3d(unit),
            Anchor { hidden: true, under_token: true, ..Anchor::new(pos.0, 0.0, TOKEN_LIFT, true) },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(26.0),
                height: Val::Px(6.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
            BackgroundColor(Color::srgb(0.05, 0.05, 0.07)),
            BorderColor::all(Color::srgb(0.05, 0.05, 0.07)),
            GlobalZIndex(-2),
            Visibility::Hidden,
            children![(HpFill3d, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(HP_GOOD))],
        ));
    }
}

/// Suit l'unité (case, toit de ville, brouillard) et règle le remplissage ; la barre disparaît avec l'unité.
fn update_hp_bars_3d(
    mut commands: Commands,
    units: Query<(&Unit, &HexPos, Has<Hostile>)>,
    cities: Query<&HexPos, With<City>>,
    fog: Res<ShownFog>,
    mut bars: Query<(Entity, &HpBar3d, &mut Anchor, &Children)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), With<HpFill3d>>,
) {
    for (entity, bar, mut anchor, children) in &mut bars {
        let Ok((unit, pos, hostile)) = units.get(bar.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let hp = unit.0.hp.clamp(0, MAX_HP);
        let hidden = hp >= MAX_HP || (hostile && !fog.visible.contains(&pos.0));
        let on_roof = if cities.iter().any(|c| c.0 == pos.0) { CITY_HEIGHT } else { 0.0 };
        let up_3d = TOKEN_LIFT + on_roof;
        if anchor.hex != pos.0 || anchor.hidden != hidden || anchor.up_3d != up_3d {
            anchor.hex = pos.0;
            anchor.hidden = hidden;
            anchor.up_3d = up_3d;
        }
        let ratio = hp as f32 / MAX_HP as f32;
        let color = if ratio > 0.6 {
            HP_GOOD
        } else if ratio > 0.3 {
            HP_MID
        } else {
            HP_LOW
        };
        for child in children.iter() {
            if let Ok((mut node, mut bg)) = fills.get_mut(child) {
                let width = Val::Percent(ratio * 100.0);
                if node.width != width {
                    node.width = width;
                }
                if bg.0 != color {
                    bg.0 = color;
                }
            }
        }
    }
}

/// Place chaque étiquette à la projection de sa case par la caméra active.
/// Les transformations des caméras sont lues directement (pas de parent) pour éviter un décalage d'une image.
fn place_labels(
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    fog: Res<ShownFog>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>)>,
    mut labels: Query<(&Anchor, &mut Node, &mut Visibility)>,
) {
    let Some((camera, transform, _)) = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active) else {
        return;
    };
    let cam_global = GlobalTransform::from(*transform);
    // Inclinaison de la caméra : écrase verticalement le disque d'un jeton à l'écran
    let tilt = (-transform.forward().y).clamp(0.2, 1.0);
    for (anchor, mut node, mut vis) in &mut labels {
        let world = if mode.is_3d {
            to_3d(anchor.hex, shown_height(&map, &fog, anchor.hex) + anchor.up_3d)
        } else {
            let (x, y) = anchor.hex.to_world();
            Vec3::new(x, y + anchor.up_2d, 0.0)
        };
        let shown = !anchor.hidden && (mode.is_3d || !anchor.only_3d);
        let mut screen = if shown { camera.world_to_viewport(&cam_global, world).ok() } else { None };
        if anchor.under_token
            && let Some(center) = screen
        {
            // Rayon du jeton à l'écran (dépend du zoom) : barre juste sous le disque, à sa largeur
            let edge = camera.world_to_viewport(&cam_global, world + transform.right() * TOKEN_RADIUS).ok();
            let radius = edge.map_or(16.0, |e| e.distance(center));
            let width = Val::Px((radius * 1.6).clamp(14.0, 48.0).round());
            if node.width != width {
                node.width = width;
            }
            screen = Some(center + Vec2::Y * (radius * tilt + 5.0));
        }
        let wanted_vis = if screen.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted_vis {
            *vis = wanted_vis;
        }
        if let Some(p) = screen {
            let (left, top) = (Val::Px(p.x.round()), Val::Px(p.y.round()));
            if node.left != left || node.top != top {
                node.left = left;
                node.top = top;
            }
        }
    }
}
