//! Mise en scène des actions sur la carte : les unités glissent case par case, l'attaquant
//! s'élance vers sa cible (ou tire un trait), la cible tressaille, les PV perdus s'affichent en
//! chiffres flottants, l'unité dispersée s'efface, la récompense d'un village apparaît sur place.
//!
//! La logique du jeu reste instantanée : les règles posent des **indications** (`Cues`) au fil
//! de leurs actions, et ce module les répartit dans le temps après coup. Les actions du joueur
//! se jouent ensemble, celles de la faction hostile ensuite, l'une après l'autre ; une action
//! hostile hors de vue n'est pas montrée (elle ne coûte aucun temps).

use crate::game::{Fog, GameMap, HexPos, Unit};
use crate::hex::Hex;
use crate::view3d::{ViewMode, shown_height};
use bevy::camera::CameraUpdateSystems;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui::UiSystems;

/// Durée d'un pas d'une case (s), et durée maximale d'un trajet (les longs trajets accélèrent),
/// à la vitesse normale. Toutes les durées d'animation sont multipliées par le rythme choisi
/// dans les paramètres (`AnimationSpeed::pace`), sauf celle des textes flottants (temps de lecture).
pub const STEP_SECS: f64 = 0.2;
pub const MAX_WALK_SECS: f64 = 1.3;
/// Élan d'un corps à corps : aller-retour vers la cible ; le coup porte aux 40 %.
pub const LUNGE_SECS: f64 = 0.36;
const LUNGE_IMPACT: f64 = 0.4;
/// Part du chemin vers la cible parcourue par l'élan.
const LUNGE_REACH: f32 = 0.42;
/// Vol d'un trait (tir) et recul du tireur.
pub const SHOT_SECS: f64 = 0.28;
/// Secousse de la cible touchée, effacement d'une unité dispersée.
pub const SHAKE_SECS: f64 = 0.3;
pub const FADE_SECS: f64 = 0.6;
/// Durée de vie d'un chiffre flottant (apparition, montée, effacement).
pub const POPUP_SECS: f64 = 1.7;
/// Pause entre deux actions hostiles successives.
pub const HOSTILE_GAP: f64 = 0.1;

/// Qui agit : le joueur (actions simultanées) ou la faction hostile (l'une après l'autre).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Player,
    Hostile,
}

/// Style d'un chiffre flottant (couleur, taille, importance).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// PV perdus.
    Damage,
    /// Unité dispersée.
    Lethal,
    /// Or gagné.
    Gold,
    /// Population, ralliement.
    Growth,
    /// Mauvaise surprise (village hostile).
    Danger,
}

/// Un combat tel qu'il doit être montré.
#[derive(Clone, Debug, PartialEq)]
pub struct Strike {
    pub attacker: Entity,
    pub defender: Entity,
    pub from: Hex,
    pub target: Hex,
    pub ranged: bool,
    pub actor: Actor,
    pub to_defender: i32,
    pub to_attacker: i32,
    /// PV après le combat (0 = dispersée).
    pub attacker_hp: i32,
    pub defender_hp: i32,
    /// L'unité du joueur est le défenseur (pour le libellé de sa perte).
    pub defender_is_player: bool,
}

/// Indication posée par les règles, mise en scène par ce module.
#[derive(Clone, Debug, PartialEq)]
pub enum Cue {
    /// Déplacement de `from` le long de `steps` (cases successives).
    Walk { entity: Entity, from: Hex, steps: Vec<Hex>, actor: Actor },
    Strike(Strike),
    /// Récompense ou événement d'un site, montré quand `by` arrive sur la case.
    Reward { at: Hex, by: Option<Entity>, text: String, tone: Tone },
}

/// Indications en attente de mise en scène (vidées à chaque image).
#[derive(Resource, Default)]
pub struct Cues(pub Vec<Cue>);

impl Cues {
    /// Déplacement (ignoré s'il n'y a aucun pas).
    pub fn walk(&mut self, entity: Entity, from: Hex, steps: &[Hex], actor: Actor) {
        if !steps.is_empty() && steps != [from] {
            self.0.push(Cue::Walk { entity, from, steps: steps.to_vec(), actor });
        }
    }

    /// Récompense ou événement montré sur un site.
    pub fn reward(&mut self, at: Hex, by: Option<Entity>, text: impl Into<String>, tone: Tone) {
        self.0.push(Cue::Reward { at, by, text: text.into(), tone });
    }
}

/// Fin de la dernière action hostile mise en scène (les suivantes attendent leur tour).
#[derive(Resource, Default)]
struct Stage {
    hostile_until: f64,
}

/// Unité retirée du jeu (dispersée) mais encore montrée le temps de son effacement.
#[derive(Component)]
pub struct Retired;

/// PV montrés par les barres jusqu'à l'instant du coup (le combat est déjà résolu).
#[derive(Component, Clone, Copy, Debug)]
pub struct HeldHp {
    pub hp: i32,
    pub until: f64,
}

impl HeldHp {
    /// PV à montrer à l'instant `now`.
    pub fn shown(held: Option<&HeldHp>, real: i32, now: f64) -> i32 {
        held.filter(|h| now < h.until).map_or(real, |h| h.hp)
    }
}

/// Forme d'un segment d'animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LegKind {
    /// Marche de `from` à `to`.
    Walk,
    /// Aller-retour de `from` vers `to`.
    Lunge,
    /// Tressaillement sur place.
    Shake,
    /// Effacement sur place (l'unité finit invisible).
    Fade,
}

/// Segment d'animation entre deux instants (secondes depuis le lancement).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub start: f64,
    pub end: f64,
    pub from: Vec2,
    pub to: Vec2,
    pub kind: LegKind,
}

/// Position et taille montrées d'une unité à un instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub at: Vec2,
    pub scale: f32,
}

/// Animation d'une unité : segments successifs, en coordonnées du monde 2D.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Motion {
    pub legs: Vec<Leg>,
}

fn ease_in_out(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn ease_out(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    1.0 - (1.0 - u) * (1.0 - u)
}

impl Motion {
    /// Fin du dernier segment.
    pub fn end(&self) -> f64 {
        self.legs.iter().map(|l| l.end).fold(f64::MIN, f64::max)
    }

    /// Animation terminée à l'instant `now`.
    pub fn finished(&self, now: f64) -> bool {
        self.legs.is_empty() || now >= self.end()
    }

    /// Position et taille à l'instant `now` : avant le premier segment, au départ ; entre deux
    /// segments, à la fin du précédent ; après un effacement, invisible.
    pub fn sample(&self, now: f64) -> Option<Pose> {
        let first = self.legs.first()?;
        let Some(leg) = self.legs.iter().filter(|l| l.start <= now).max_by(|a, b| a.start.total_cmp(&b.start)) else {
            return Some(Pose { at: first.from, scale: 1.0 });
        };
        let span = (leg.end - leg.start).max(1e-6);
        let u = ((now - leg.start) / span).clamp(0.0, 1.0) as f32;
        let pose = match leg.kind {
            LegKind::Walk => Pose { at: leg.from.lerp(leg.to, ease_in_out(u)), scale: 1.0 },
            LegKind::Lunge => {
                // Départ vif, retour plus lent
                let k = LUNGE_IMPACT as f32;
                let reach = if u < k { ease_out(u / k) } else { 1.0 - ease_in_out((u - k) / (1.0 - k)) };
                Pose { at: leg.from.lerp(leg.to, reach), scale: 1.0 }
            }
            LegKind::Shake => {
                let amplitude = 4.0 * (1.0 - u);
                Pose { at: leg.from + Vec2::X * (u * std::f32::consts::TAU * 3.0).sin() * amplitude, scale: 1.0 }
            }
            LegKind::Fade => Pose { at: leg.from - Vec2::Y * 6.0 * u, scale: 1.0 - ease_in_out(u) },
        };
        Some(pose)
    }

    /// Instant où l'unité atteint `point` en marchant (fin du pas qui y mène).
    pub fn arrival(&self, point: Vec2) -> Option<f64> {
        self.legs.iter().filter(|l| l.kind == LegKind::Walk && l.to.distance(point) < 1.0).map(|l| l.end).reduce(f64::min)
    }
}

/// Segments d'une marche : un par pas, partant de `start`, chaque case prise du côté du raccord
/// le plus proche de la précédente (tracé continu autour de la planète). Les longs trajets sont
/// accélérés pour durer au plus `MAX_WALK_SECS`. `pace` multiplie les durées (0 : instantané).
pub fn walk_legs(wrap: crate::hex::Wrap, from: Hex, steps: &[Hex], start: f64, pace: f64) -> Vec<Leg> {
    let step = STEP_SECS.min(MAX_WALK_SECS / steps.len().max(1) as f64) * pace;
    let mut prev = from;
    let mut t = start;
    steps
        .iter()
        .map(|&h| {
            let next = wrap.nearest(prev, h);
            let (a, b) = (prev.to_world(), next.to_world());
            let leg = Leg { start: t, end: t + step, from: Vec2::new(a.0, a.1), to: Vec2::new(b.0, b.1), kind: LegKind::Walk };
            prev = next;
            t += step;
            leg
        })
        .collect()
}

/// Calendrier d'un combat : (segments de l'attaquant, segments du défenseur, instant du coup).
/// `pace` multiplie les durées (0 : instantané).
pub fn strike_legs(wrap: crate::hex::Wrap, s: &Strike, start: f64, pace: f64) -> (Vec<Leg>, Vec<Leg>, f64) {
    let (shot, lunge, shake, fade) = (SHOT_SECS * pace, LUNGE_SECS * pace, SHAKE_SECS * pace, FADE_SECS * pace);
    let (ax, ay) = s.from.to_world();
    let (tx, ty) = wrap.nearest(s.from, s.target).to_world();
    let (a, t) = (Vec2::new(ax, ay), Vec2::new(tx, ty));
    let (dx, dy) = s.target.to_world();
    let d = Vec2::new(dx, dy);
    let (attacker, impact) = if s.ranged {
        // Petit recul du tireur, le trait vole jusqu'à la cible
        let back = a - (t - a).normalize_or_zero() * 4.0;
        (vec![Leg { start, end: start + shot * 0.6, from: a, to: back, kind: LegKind::Lunge }], start + shot)
    } else {
        let reach = a.lerp(t, LUNGE_REACH);
        (vec![Leg { start, end: start + lunge, from: a, to: reach, kind: LegKind::Lunge }], start + lunge * LUNGE_IMPACT)
    };
    let mut attacker = attacker;
    let mut defender = vec![Leg { start: impact, end: impact + shake, from: d, to: d, kind: LegKind::Shake }];
    if s.defender_hp <= 0 {
        defender.push(Leg { start: impact + shake * 0.5, end: impact + shake * 0.5 + fade, from: d, to: d, kind: LegKind::Fade });
    }
    if s.attacker_hp <= 0 {
        let end = attacker.last().map_or(impact, |l| l.end);
        attacker.push(Leg { start: end, end: end + fade, from: a, to: a, kind: LegKind::Fade });
    }
    (attacker, defender, impact)
}

/// Chiffre ou mot flottant au-dessus d'une case.
#[derive(Component)]
struct Popup {
    hex: Hex,
    start: f64,
    /// Décalage initial vers le haut (pixels), pour ne pas chevaucher un autre texte.
    lift: f32,
    color: Color,
}

/// Cercle qui s'élargit au point d'impact (ou d'une unité dispersée).
#[derive(Component)]
struct Burst {
    at: Vec2,
    start: f64,
    end: f64,
    radius: f32,
    color: Color,
}

/// Trait qui vole du tireur à sa cible.
#[derive(Component)]
struct Shot {
    from: Vec2,
    to: Vec2,
    start: f64,
    end: f64,
}

/// Couleur et taille d'un texte flottant.
fn tone_style(tone: Tone) -> (Color, f32) {
    match tone {
        Tone::Damage => (Color::srgb(1.0, 0.42, 0.32), 19.0),
        Tone::Lethal => (Color::srgb(1.0, 0.86, 0.80), 17.0),
        Tone::Gold => (Color::srgb(1.0, 0.84, 0.30), 18.0),
        Tone::Growth => (Color::srgb(0.55, 0.95, 0.55), 18.0),
        Tone::Danger => (Color::srgb(1.0, 0.35, 0.25), 18.0),
    }
}

/// Taille d'un chiffre de dégâts : les gros coups ressortent davantage.
fn damage_size(damage: i32) -> f32 {
    (16.0 + damage as f32 * 0.18).clamp(16.0, 26.0)
}

pub struct FeedbackPlugin;

impl Plugin for FeedbackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cues>()
            .init_resource::<Stage>()
            .add_systems(
                Update,
                (stage_cues, animate_2d, finish_motions, draw_bursts)
                    .chain()
                    .after(crate::game::EndTurnSet)
                    .after(crate::render::sync_transforms),
            )
            .add_systems(PostUpdate, place_popups.after(CameraUpdateSystems).before(UiSystems::Prepare));
    }
}

/// Répartit dans le temps les indications de l'image : actions du joueur tout de suite et
/// ensemble, puis actions hostiles visibles l'une après l'autre, récompenses à l'arrivée de
/// l'unité sur le site.
fn stage_cues(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<GameMap>,
    fog: Res<Fog>,
    settings: Res<crate::settings::Settings>,
    mut cues: ResMut<Cues>,
    mut stage: ResMut<Stage>,
    motions: Query<&Motion>,
) {
    if cues.0.is_empty() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let wrap = map.0.wrap();
    let pace = settings.animation.pace();
    let cues = std::mem::take(&mut cues.0);
    // Segments ajoutés à chaque unité (en plus de son animation en cours)
    let mut plans: HashMap<Entity, Motion> = HashMap::new();
    let busy = |plans: &HashMap<Entity, Motion>, e: Entity| {
        let current = plans.get(&e).cloned().or_else(|| motions.get(e).ok().cloned());
        current.filter(|m| !m.legs.is_empty()).map_or(now, |m| m.end().max(now))
    };
    let add = |plans: &mut HashMap<Entity, Motion>, e: Entity, legs: Vec<Leg>| {
        let entry = plans.entry(e).or_insert_with(|| motions.get(e).ok().filter(|m| !m.finished(now)).cloned().unwrap_or_default());
        entry.legs.extend(legs);
    };
    let visible = |h: Hex| fog.visible.contains(&map.0.canonical(h));
    let mut player_end = now;
    let mut hostile: Vec<&Cue> = Vec::new();
    let mut rewards: Vec<&Cue> = Vec::new();
    let strike = |commands: &mut Commands, plans: &mut HashMap<Entity, Motion>, s: &Strike, start: f64| -> f64 {
        let (att, def, impact) = strike_legs(wrap, s, start, pace);
        // (l'action suivante n'attend pas la fin de l'effacement d'une unité dispersée)
        let end = att.iter().chain(&def).filter(|l| l.kind != LegKind::Fade).map(|l| l.end).fold(impact, f64::max);
        add(plans, s.attacker, att);
        add(plans, s.defender, def);
        show_strike(commands, wrap, s, start, impact, pace);
        end
    };
    for cue in &cues {
        match cue {
            Cue::Walk { entity, from, steps, actor: Actor::Player } => {
                let legs = walk_legs(wrap, *from, steps, busy(&plans, *entity), pace);
                player_end = player_end.max(legs.last().map_or(now, |l| l.end));
                add(&mut plans, *entity, legs);
            }
            Cue::Strike(s) if s.actor == Actor::Player => {
                let start = busy(&plans, s.attacker).max(busy(&plans, s.defender));
                player_end = player_end.max(strike(&mut commands, &mut plans, s, start));
            }
            Cue::Reward { .. } => rewards.push(cue),
            _ => hostile.push(cue),
        }
    }
    // Faction hostile : après le joueur, une action à la fois ; rien n'est montré hors de vue
    let mut cursor = stage.hostile_until.max(player_end);
    for cue in hostile {
        match cue {
            Cue::Walk { entity, from, steps, .. } if visible(*from) || steps.iter().any(|&h| visible(h)) => {
                let legs = walk_legs(wrap, *from, steps, cursor.max(busy(&plans, *entity)), pace);
                cursor = legs.last().map_or(cursor, |l| l.end);
                add(&mut plans, *entity, legs);
            }
            Cue::Strike(s) if visible(s.from) || visible(s.target) => {
                let start = cursor.max(busy(&plans, s.attacker)).max(busy(&plans, s.defender));
                cursor = strike(&mut commands, &mut plans, s, start) + HOSTILE_GAP * pace;
            }
            _ => {}
        }
    }
    stage.hostile_until = cursor;
    // Récompenses : quand l'unité arrive sur le site (sinon tout de suite)
    for cue in rewards {
        let Cue::Reward { at, by, text, tone } = cue else { continue };
        let (x, y) = at.to_world();
        let when = by
            .and_then(|e| plans.get(&e))
            .and_then(|m| {
                // (le site peut être pris de l'autre côté du raccord)
                let w = wrap.world_width();
                [0.0, w, -w].into_iter().find_map(|dx| m.arrival(Vec2::new(x + dx, y)))
            })
            .unwrap_or(now);
        let (color, size) = tone_style(*tone);
        spawn_popup(&mut commands, *at, when, 0.0, text, color, size);
        commands.spawn(Burst { at: Vec2::new(x, y), start: when, end: when + 0.7, radius: crate::hex::HEX_SIZE * 0.9, color });
    }
    for (entity, motion) in plans {
        if motion.legs.is_empty() {
            continue;
        }
        commands.entity(entity).try_insert(motion);
    }
    // (une unité dispersée sans animation est retirée tout de suite par `finish_motions`)
}

/// Textes, impacts et PV retenus d'un combat.
fn show_strike(commands: &mut Commands, wrap: crate::hex::Wrap, s: &Strike, start: f64, impact: f64, pace: f64) {
    let (dx, dy) = s.target.to_world();
    let d = Vec2::new(dx, dy);
    if s.ranged {
        let (ax, ay) = s.from.to_world();
        let (tx, ty) = wrap.nearest(s.from, s.target).to_world();
        commands.spawn(Shot { from: Vec2::new(ax, ay), to: Vec2::new(tx, ty), start, end: impact });
    }
    commands.spawn(Burst { at: d, start: impact, end: impact + 0.45, radius: crate::hex::HEX_SIZE * 0.7, color: Color::srgb(1.0, 0.55, 0.3) });
    let damage = |commands: &mut Commands, hex: Hex, amount: i32, lift: f32| {
        let (color, _) = tone_style(Tone::Damage);
        let text = if amount > 0 { format!("−{amount}") } else { "0".into() };
        let color = if amount > 0 { color } else { Color::srgb(0.85, 0.85, 0.9) };
        spawn_popup(commands, hex, impact, lift, &text, color, damage_size(amount));
    };
    damage(commands, s.target, s.to_defender, 0.0);
    if !s.ranged {
        damage(commands, s.from, s.to_attacker, 0.0);
    }
    // PV montrés d'avant le coup jusqu'à l'impact
    for (entity, hp, lost) in [(s.defender, s.defender_hp, s.to_defender), (s.attacker, s.attacker_hp, s.to_attacker)] {
        if hp > 0 && lost > 0 {
            commands.entity(entity).try_insert(HeldHp { hp: hp + lost, until: impact });
        }
    }
    // Unité dispersée : mot plus haut que les dégâts, impact sombre
    let lethal = |commands: &mut Commands, hex: Hex, player: bool| {
        let (color, size) = tone_style(Tone::Lethal);
        let text = if player { "Unité perdue" } else { "Dispersé !" };
        let color = if player { Color::srgb(1.0, 0.55, 0.50) } else { color };
        let at = impact + 0.15 * pace;
        spawn_popup(commands, hex, at, 32.0, text, color, size);
        let (x, y) = hex.to_world();
        commands.spawn(Burst { at: Vec2::new(x, y), start: at, end: at + 0.8, radius: crate::hex::HEX_SIZE, color: Color::srgb(0.75, 0.1, 0.1) });
    };
    if s.defender_hp <= 0 {
        lethal(commands, s.target, s.defender_is_player);
    }
    if s.attacker_hp <= 0 {
        lethal(commands, s.from, !s.defender_is_player);
    }
}

fn spawn_popup(commands: &mut Commands, hex: Hex, start: f64, lift: f32, text: &str, color: Color, size: f32) {
    commands.spawn((
        Popup { hex, start, lift, color },
        Node { position_type: PositionType::Absolute, ..default() },
        UiTransform::from_translation(Val2::percent(-50.0, -100.0)),
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color.with_alpha(0.0)),
        TextShadow { offset: Vec2::new(1.5, 1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.0) },
        GlobalZIndex(5),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
}

/// Fait suivre aux unités 2D leur animation (après le recalage sur leur case).
fn animate_2d(time: Res<Time>, mut units: Query<(&Motion, &mut Transform), Without<crate::seam::GhostCamera>>) {
    let now = time.elapsed_secs_f64();
    for (motion, mut transform) in &mut units {
        let Some(pose) = motion.sample(now) else { continue };
        transform.translation.x = pose.at.x;
        transform.translation.y = pose.at.y;
        transform.scale = Vec3::new(pose.scale, pose.scale, 1.0);
    }
}

/// Fin des animations : l'unité est recalée sur sa case, une unité dispersée est retirée ;
/// les PV retenus sont relâchés.
fn finish_motions(
    mut commands: Commands,
    time: Res<Time>,
    mut moving: Query<(Entity, &Motion, Option<&HexPos>, &mut Transform, Has<Retired>)>,
    still_retired: Query<Entity, (With<Retired>, Without<Motion>)>,
    held: Query<(Entity, &HeldHp)>,
    mut units: Query<&mut Unit>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, motion, pos, mut transform, retired) in &mut moving {
        if !motion.finished(now) {
            continue;
        }
        if retired {
            commands.entity(entity).despawn();
            continue;
        }
        commands.entity(entity).remove::<Motion>();
        transform.scale = Vec3::ONE;
        if let Some(pos) = pos {
            let (x, y) = pos.0.to_world();
            transform.translation.x = x;
            transform.translation.y = y;
        }
    }
    for entity in &still_retired {
        commands.entity(entity).despawn();
    }
    for (entity, h) in &held {
        if now >= h.until {
            commands.entity(entity).remove::<HeldHp>();
            // (les barres de PV se recalculent sur un changement de l'unité)
            if let Ok(mut unit) = units.get_mut(entity) {
                unit.set_changed();
            }
        }
    }
}

/// Impacts (cercles qui s'élargissent et s'effacent) et traits en vol, en 2D comme en 3D.
fn draw_bursts(
    mut commands: Commands,
    time: Res<Time>,
    mode: Res<ViewMode>,
    relief: Option<Res<crate::view3d::Relief>>,
    bursts: Query<(Entity, &Burst)>,
    shots: Query<(Entity, &Shot)>,
    mut gizmos: Gizmos,
    mut gizmos_3d: Gizmos<crate::view3d::Gizmos3d>,
) {
    let now = time.elapsed_secs_f64();
    let relief = relief.filter(|_| mode.is_3d);
    for (entity, b) in &bursts {
        if now >= b.end {
            commands.entity(entity).despawn();
            continue;
        }
        if now < b.start {
            continue;
        }
        let u = ((now - b.start) / (b.end - b.start)) as f32;
        let r = b.radius * (0.35 + 0.65 * ease_out(u));
        let color = b.color.with_alpha(1.0 - u);
        match &relief {
            Some(relief) => {
                let points = (0..=32).map(|k| {
                    let p = b.at + Vec2::from_angle(k as f32 / 32.0 * std::f32::consts::TAU) * r;
                    relief.on_ground(p.x, p.y, 3.0)
                });
                gizmos_3d.linestrip(points, color);
            }
            None => {
                gizmos.circle_2d(b.at, r, color);
                gizmos.circle_2d(b.at, r * 0.8, color.with_alpha((1.0 - u) * 0.5));
            }
        }
    }
    for (entity, s) in &shots {
        if now >= s.end {
            commands.entity(entity).despawn();
            continue;
        }
        if now < s.start {
            continue;
        }
        let u = ((now - s.start) / (s.end - s.start)) as f32;
        let head = s.from.lerp(s.to, u);
        let tail = s.from.lerp(s.to, (u - 0.25).max(0.0));
        let color = Color::srgb(1.0, 0.95, 0.75);
        match &relief {
            Some(relief) => {
                // Trajectoire en cloche au-dessus du sol
                let lift = |k: f32| 10.0 + 18.0 * (std::f32::consts::PI * k).sin();
                let a = relief.on_ground(tail.x, tail.y, lift((u - 0.25).max(0.0)));
                let b = relief.on_ground(head.x, head.y, lift(u));
                gizmos_3d.line(a, b, color);
            }
            None => gizmos.line_2d(tail, head, color),
        }
    }
}

/// Place les textes flottants à la projection de leur case : apparition avec un léger
/// grossissement, montée, puis effacement.
fn place_popups(
    mut commands: Commands,
    time: Res<Time>,
    mode: Res<ViewMode>,
    map: Res<GameMap>,
    fog: Res<crate::game::ShownFog>,
    cameras: Query<(&Camera, &Transform, Has<Camera3d>), Without<crate::seam::GhostCamera>>,
    mut popups: Query<(Entity, &Popup, &mut Node, &mut UiTransform, &mut TextColor, &mut TextShadow, &mut Visibility)>,
) {
    let now = time.elapsed_secs_f64();
    let camera = cameras.iter().find(|(c, _, is_3d)| *is_3d == mode.is_3d && c.is_active);
    for (entity, popup, mut node, mut ui, mut color, mut shadow, mut vis) in &mut popups {
        let age = now - popup.start;
        if age >= POPUP_SECS {
            commands.entity(entity).despawn();
            continue;
        }
        let Some((camera, transform, _)) = camera.filter(|_| age >= 0.0) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let world = if mode.is_3d {
            crate::view3d::to_3d(popup.hex, shown_height(&map, &fog, popup.hex) + 14.0)
        } else {
            let (x, y) = popup.hex.to_world();
            Vec3::new(x, y + 14.0, 0.0)
        };
        let world = crate::labels::near_camera(&map, world, transform.translation.x);
        let Ok(p) = camera.world_to_viewport(&GlobalTransform::from(*transform), world) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let u = (age / POPUP_SECS) as f32;
        let rise = 34.0 * ease_out(u);
        let alpha = if u < 0.08 { u / 0.08 } else if u > 0.6 { 1.0 - (u - 0.6) / 0.4 } else { 1.0 };
        let pop = 1.0 + 0.35 * (1.0 - ease_out((age / 0.18) as f32));
        node.left = Val::Px(p.x.round());
        node.top = Val::Px((p.y - popup.lift - rise).round());
        ui.scale = Vec2::splat(pop);
        color.0 = popup.color.with_alpha(alpha);
        shadow.color = Color::srgba(0.0, 0.0, 0.0, 0.85 * alpha);
        *vis = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap() -> crate::hex::Wrap {
        crate::hex::Wrap::new(40, true)
    }

    #[test]
    fn une_marche_fait_un_segment_par_pas_dans_l_ordre() {
        let from = Hex::new(5, 5);
        let steps = [Hex::new(6, 5), Hex::new(7, 5), Hex::new(8, 5)];
        let legs = walk_legs(wrap(), from, &steps, 10.0, 1.0);
        assert_eq!(legs.len(), 3);
        assert_eq!(legs[0].start, 10.0);
        for w in legs.windows(2) {
            assert_eq!(w[0].end, w[1].start);
            assert_eq!(w[0].to, w[1].from);
        }
        let (x, y) = steps[2].to_world();
        assert_eq!(legs[2].to, Vec2::new(x, y));
    }

    #[test]
    fn un_long_trajet_est_accelere() {
        let from = Hex::new(0, 5);
        let steps: Vec<Hex> = (1..=30).map(|q| Hex::new(q, 5)).collect();
        let legs = walk_legs(wrap(), from, &steps, 0.0, 1.0);
        let total = legs.last().unwrap().end;
        assert!(total <= MAX_WALK_SECS + 1e-9, "{total}");
    }

    #[test]
    fn la_pose_suit_le_calendrier() {
        let legs = walk_legs(wrap(), Hex::new(5, 5), &[Hex::new(6, 5)], 1.0, 1.0);
        let m = Motion { legs: legs.clone() };
        // Avant le départ : sur la case de départ ; à la fin : sur la case d'arrivée
        assert_eq!(m.sample(0.0).unwrap().at, legs[0].from);
        assert_eq!(m.sample(5.0).unwrap().at, legs[0].to);
        let mid = m.sample(1.0 + STEP_SECS / 2.0).unwrap().at;
        assert!(mid.distance(legs[0].from) > 1.0 && mid.distance(legs[0].to) > 1.0);
        assert!(!m.finished(1.0) && m.finished(1.0 + STEP_SECS));
        assert_eq!(m.arrival(legs[0].to), Some(1.0 + STEP_SECS));
    }

    fn strike(defender_hp: i32, ranged: bool) -> Strike {
        Strike {
            attacker: Entity::from_raw_u32(1).unwrap(),
            defender: Entity::from_raw_u32(2).unwrap(),
            from: Hex::new(5, 5),
            target: Hex::new(6, 5),
            ranged,
            actor: Actor::Player,
            to_defender: 30,
            to_attacker: 10,
            attacker_hp: 90,
            defender_hp,
            defender_is_player: false,
        }
    }

    #[test]
    fn l_elan_revient_a_sa_case_et_le_coup_porte_pendant_l_aller() {
        let (att, def, impact) = strike_legs(wrap(), &strike(40, false), 2.0, 1.0);
        let m = Motion { legs: att.clone() };
        assert!(impact > 2.0 && impact < att[0].end);
        // Au moment du coup, l'attaquant est au plus loin ; il revient ensuite sur sa case
        let at_impact = m.sample(impact).unwrap().at;
        assert!(at_impact.distance(att[0].from) > 10.0);
        assert!(m.sample(att[0].end).unwrap().at.distance(att[0].from) < 1e-3);
        // La cible tressaille au moment du coup, sans effacement (elle survit)
        assert_eq!(def[0].start, impact);
        assert!(def.iter().all(|l| l.kind != LegKind::Fade));
    }

    #[test]
    fn une_unite_dispersee_s_efface() {
        let (_, def, _) = strike_legs(wrap(), &strike(0, false), 0.0, 1.0);
        let m = Motion { legs: def };
        assert_eq!(m.sample(m.end()).unwrap().scale, 0.0);
    }

    #[test]
    fn un_tir_ne_fait_pas_avancer_le_tireur() {
        let (att, _, impact) = strike_legs(wrap(), &strike(40, true), 0.0, 1.0);
        let (tx, ty) = Hex::new(6, 5).to_world();
        let target = Vec2::new(tx, ty);
        let m = Motion { legs: att.clone() };
        // Le tireur recule légèrement au lieu de s'approcher
        assert!(m.sample(att[0].end * 0.4).unwrap().at.distance(target) >= att[0].from.distance(target));
        assert_eq!(impact, SHOT_SECS);
    }

    #[test]
    fn les_pv_retenus_valent_jusqu_au_coup() {
        let held = HeldHp { hp: 80, until: 3.0 };
        assert_eq!(HeldHp::shown(Some(&held), 50, 2.0), 80);
        assert_eq!(HeldHp::shown(Some(&held), 50, 3.0), 50);
        assert_eq!(HeldHp::shown(None, 50, 2.0), 50);
    }

    #[test]
    fn le_rythme_allonge_ou_supprime_les_animations() {
        let steps = [Hex::new(6, 5), Hex::new(7, 5)];
        let slow = walk_legs(wrap(), Hex::new(5, 5), &steps, 0.0, 1.5);
        assert!((slow.last().unwrap().end - 2.0 * STEP_SECS * 1.5).abs() < 1e-9);
        // Instantané : segments de durée nulle, l'unité est aussitôt arrivée
        let instant = walk_legs(wrap(), Hex::new(5, 5), &steps, 4.0, 0.0);
        let m = Motion { legs: instant };
        assert!(m.finished(4.0));
        let (att, def, impact) = strike_legs(wrap(), &strike(0, false), 4.0, 0.0);
        assert_eq!(impact, 4.0);
        assert!(att.iter().chain(&def).all(|l| l.end == 4.0));
    }
}
