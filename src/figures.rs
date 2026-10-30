//! Figurines 3D des unités, faites de formes simples (cylindres, sphères, cônes, pavés), comme
//! les villes, camps et villages : un personnage (ou un cavalier sur son cheval) et ce qui
//! distingue son type (bouclier, lance, pique, arc, carquois, ballot…). Les vêtements sont aux
//! couleurs du camp (bleu : joueur, rouge : hostile). Repère local : pieds en y = 0, la figurine
//! regarde vers +z (vers la caméra au départ), sa droite vers −x.

use crate::units::UnitKind;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

/// Formes et matières des figurines.
pub struct FigureKit {
    legs: Handle<Mesh>,
    torso: Handle<Mesh>,
    torso_wide: Handle<Mesh>,
    head: Handle<Mesh>,
    helmet: Handle<Mesh>,
    hood: Handle<Mesh>,
    shield: Handle<Mesh>,
    big_shield: Handle<Mesh>,
    sword: Handle<Mesh>,
    club: Handle<Mesh>,
    spear: Handle<Mesh>,
    pike: Handle<Mesh>,
    tip: Handle<Mesh>,
    bow: Handle<Mesh>,
    quiver: Handle<Mesh>,
    sack: Handle<Mesh>,
    staff: Handle<Mesh>,
    horse_body: Handle<Mesh>,
    horse_leg: Handle<Mesh>,
    horse_neck: Handle<Mesh>,
    horse_head: Handle<Mesh>,
    skin: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    leather: Handle<StandardMaterial>,
    horse: Handle<StandardMaterial>,
    /// Vêtements : joueur, hostile.
    cloth: [Handle<StandardMaterial>; 2],
}

impl FigureKit {
    /// Formes et matières, créées une fois au démarrage.
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mat = |materials: &mut Assets<StandardMaterial>, c: Color, rough: f32| materials.add(StandardMaterial { base_color: c, perceptual_roughness: rough, ..default() });
        FigureKit {
            legs: meshes.add(Cylinder::new(1.5, 4.0)),
            torso: meshes.add(Capsule3d::new(2.0, 3.0)),
            torso_wide: meshes.add(Capsule3d::new(2.6, 3.0)),
            head: meshes.add(Sphere::new(1.6)),
            helmet: meshes.add(Cone { radius: 1.9, height: 2.0 }),
            hood: meshes.add(Cone { radius: 2.0, height: 2.8 }),
            shield: meshes.add(Cylinder::new(2.6, 0.5)),
            big_shield: meshes.add(Cuboid::new(4.2, 6.0, 0.6)),
            sword: meshes.add(Cuboid::new(0.4, 5.0, 0.25)),
            club: meshes.add(Cylinder::new(0.55, 4.5)),
            spear: meshes.add(Cylinder::new(0.25, 13.0)),
            pike: meshes.add(Cylinder::new(0.25, 19.0)),
            tip: meshes.add(Cone { radius: 0.6, height: 1.8 }),
            bow: meshes.add(Cuboid::new(0.35, 3.2, 0.35)),
            quiver: meshes.add(Cylinder::new(0.8, 4.0)),
            sack: meshes.add(Sphere::new(1.9)),
            staff: meshes.add(Cylinder::new(0.25, 11.0)),
            horse_body: meshes.add(Cuboid::new(10.0, 4.0, 4.0)),
            horse_leg: meshes.add(Cylinder::new(0.8, 4.5)),
            horse_neck: meshes.add(Cuboid::new(2.2, 4.5, 2.2)),
            horse_head: meshes.add(Cuboid::new(3.6, 1.8, 1.8)),
            skin: mat(materials, Color::srgb(0.85, 0.66, 0.52), 0.8),
            metal: mat(materials, Color::srgb(0.62, 0.64, 0.68), 0.35),
            wood: mat(materials, Color::srgb(0.45, 0.30, 0.17), 0.9),
            leather: mat(materials, Color::srgb(0.50, 0.36, 0.22), 0.85),
            horse: mat(materials, Color::srgb(0.40, 0.26, 0.16), 0.8),
            cloth: [mat(materials, Color::srgb(0.20, 0.42, 0.90), 0.7), mat(materials, Color::srgb(0.78, 0.18, 0.15), 0.7)],
        }
    }
}

/// Monte la figurine d'une unité sous `parent` (pieds à l'origine du parent).
pub fn spawn_figure(parent: &mut ChildSpawnerCommands, kit: &FigureKit, kind: UnitKind, hostile: bool, layer: &RenderLayers) {
    use UnitKind::*;
    let cloth = &kit.cloth[usize::from(hostile)];
    let mut part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, transform: Transform| {
        parent.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform, layer.clone()));
    };
    let at = Transform::from_xyz;
    // Cavalier : cheval (vers +z) et son cavalier dessus ; fantassin : jambes et corps
    let mounted = matches!(kind, LightCavalry | HeavyCavalry | HorseArcher);
    let seat = if mounted {
        let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        part(&kit.horse_body, &kit.horse, at(0.0, 6.5, 0.0).with_rotation(turn));
        for (x, z) in [(1.3, 3.6), (-1.3, 3.6), (1.3, -3.6), (-1.3, -3.6)] {
            part(&kit.horse_leg, &kit.horse, at(x, 2.25, z));
        }
        part(&kit.horse_neck, &kit.horse, at(0.0, 9.2, 5.2).with_rotation(Quat::from_rotation_x(0.45)));
        part(&kit.horse_head, &kit.horse, at(0.0, 10.8, 6.8).with_rotation(turn));
        8.5
    } else {
        part(&kit.legs, cloth, at(0.0, 2.0, 0.0));
        4.0
    };
    let wide = matches!(kind, HeavyWarrior | HeavyCavalry);
    part(if wide { &kit.torso_wide } else { &kit.torso }, cloth, at(0.0, seat + 2.5, 0.0));
    let head = seat + 6.4;
    part(&kit.head, &kit.skin, at(0.0, head, 0.0));
    // Coiffure
    match kind {
        HeavyWarrior | Spearman | Pikeman | HeavyCavalry => part(&kit.helmet, &kit.metal, at(0.0, head + 1.6, 0.0)),
        Scout | ExpertArcher => part(&kit.hood, cloth, at(0.0, head + 1.4, 0.0)),
        _ => {}
    }
    // Ce qu'il porte : arme dans la main droite (−x), bouclier ou arc à gauche (+x, en avant)
    let hand = seat + 3.5;
    let shield = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform)| {
        part(&kit.shield, &kit.wood, at(2.4, hand, 1.4).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)));
    };
    let pole = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform), mesh: &Handle<Mesh>, length: f32, lean: f32| {
        let rot = Quat::from_rotation_x(lean);
        let center = Vec3::new(-2.8, hand + length * 0.3, 0.6);
        part(mesh, &kit.wood, Transform::from_translation(center).with_rotation(rot));
        part(&kit.tip, &kit.metal, Transform::from_translation(center + rot * Vec3::Y * (length / 2.0 + 0.9)).with_rotation(rot));
    };
    let bow = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform)| {
        // Arc : trois branches en « ( », tenu à gauche
        let (x, z) = (2.6, 1.2);
        part(&kit.bow, &kit.wood, at(x, hand + 0.5, z));
        part(&kit.bow, &kit.wood, at(x, hand + 3.2, z - 0.7).with_rotation(Quat::from_rotation_x(-0.5)));
        part(&kit.bow, &kit.wood, at(x, hand - 2.2, z - 0.7).with_rotation(Quat::from_rotation_x(0.5)));
    };
    let sword = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform)| {
        part(&kit.sword, &kit.metal, at(-2.8, hand + 1.5, 0.8).with_rotation(Quat::from_rotation_z(0.25)));
    };
    match kind {
        Settler => {
            part(&kit.sack, &kit.leather, at(0.0, seat + 3.0, -2.6));
            part(&kit.staff, &kit.wood, at(-2.8, 5.5, 0.6));
        }
        Scout => part(&kit.staff, &kit.wood, at(-2.8, 5.5, 0.6)),
        Militia => part(&kit.club, &kit.wood, at(-2.8, hand + 1.2, 0.8).with_rotation(Quat::from_rotation_z(0.35))),
        ShieldWarrior => {
            shield(&mut part);
            sword(&mut part);
        }
        HeavyWarrior => {
            part(&kit.big_shield, &kit.metal, at(1.6, hand, 2.6));
            sword(&mut part);
        }
        Spearman => {
            shield(&mut part);
            pole(&mut part, &kit.spear, 13.0, 0.0);
        }
        Pikeman => pole(&mut part, &kit.pike, 19.0, 0.0),
        ArcherSwordsman => {
            bow(&mut part);
            sword(&mut part);
        }
        ExpertArcher => {
            bow(&mut part);
            part(&kit.quiver, &kit.leather, at(-0.8, seat + 3.5, -2.4).with_rotation(Quat::from_rotation_x(-0.3)));
        }
        LightCavalry => pole(&mut part, &kit.spear, 13.0, 0.35),
        HeavyCavalry => {
            shield(&mut part);
            pole(&mut part, &kit.spear, 13.0, 0.9);
        }
        HorseArcher => bow(&mut part),
    }
}
