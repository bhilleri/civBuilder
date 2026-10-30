//! Figurines 3D des unités, faites de formes simples (cylindres, sphères, cônes, pavés), comme
//! les villes, camps et villages. Chaque type a sa silhouette, lisible de loin : le colon tire une
//! charrette à bras chargée, l'éclaireur va avec son chien, le guerrier lourd une armure
//! sombre et un grand bouclier, le piquier une très longue pique, les archers un arc haut, les
//! cavaliers montent un cheval (caparaçonné pour le cavalier lourd). Les militaires portent les
//! couleurs de leur camp (tunique, bouclier, plumet) ; colon, éclaireur et milice ont des
//! vêtements de tous les jours. Repère local : pieds en y = 0, la figurine regarde vers +z (vers
//! la caméra au départ), sa droite vers −x.

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
    plume: Handle<Mesh>,
    hood: Handle<Mesh>,
    hat_brim: Handle<Mesh>,
    hat_top: Handle<Mesh>,
    dog_body: Handle<Mesh>,
    dog_leg: Handle<Mesh>,
    dog_head: Handle<Mesh>,
    dog_ear: Handle<Mesh>,
    dog_tail: Handle<Mesh>,
    shield: Handle<Mesh>,
    boss: Handle<Mesh>,
    tower_shield: Handle<Mesh>,
    sword: Handle<Mesh>,
    club: Handle<Mesh>,
    spear: Handle<Mesh>,
    pike: Handle<Mesh>,
    tip: Handle<Mesh>,
    bow: Handle<Mesh>,
    quiver: Handle<Mesh>,
    staff: Handle<Mesh>,
    cart_box: Handle<Mesh>,
    wheel: Handle<Mesh>,
    shaft: Handle<Mesh>,
    sack: Handle<Mesh>,
    bundle: Handle<Mesh>,
    horse_body: Handle<Mesh>,
    horse_leg: Handle<Mesh>,
    horse_neck: Handle<Mesh>,
    horse_head: Handle<Mesh>,
    caparison: Handle<Mesh>,
    skin: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    dark_metal: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    leather: Handle<StandardMaterial>,
    linen: Handle<StandardMaterial>,
    straw: Handle<StandardMaterial>,
    forest: Handle<StandardMaterial>,
    horse: Handle<StandardMaterial>,
    horse_pale: Handle<StandardMaterial>,
    dog: Handle<StandardMaterial>,
    /// Couleurs du camp : joueur, hostile.
    team: [Handle<StandardMaterial>; 2],
}

impl FigureKit {
    /// Formes et matières, créées une fois au démarrage.
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut mat = |c: Color, rough: f32, metallic: f32| materials.add(StandardMaterial { base_color: c, perceptual_roughness: rough, metallic, ..default() });
        let half_turn = std::f32::consts::FRAC_PI_2;
        FigureKit {
            legs: meshes.add(Cylinder::new(1.5, 4.0)),
            torso: meshes.add(Capsule3d::new(2.0, 3.0)),
            torso_wide: meshes.add(Capsule3d::new(2.7, 3.0)),
            head: meshes.add(Sphere::new(1.6)),
            helmet: meshes.add(Cone { radius: 1.9, height: 2.2 }),
            plume: meshes.add(Cuboid::new(0.6, 2.2, 2.6)),
            hood: meshes.add(Cone { radius: 2.1, height: 3.2 }),
            hat_brim: meshes.add(Cylinder::new(3.4, 0.35)),
            hat_top: meshes.add(Cone { radius: 1.8, height: 1.6 }),
            dog_body: meshes.add(Cuboid::new(1.8, 1.8, 4.6)),
            dog_leg: meshes.add(Cylinder::new(0.38, 2.2)),
            dog_head: meshes.add(Cuboid::new(1.6, 1.6, 2.2)),
            dog_ear: meshes.add(Cone { radius: 0.45, height: 0.9 }),
            dog_tail: meshes.add(Cylinder::new(0.25, 2.0)),
            shield: meshes.add(Cylinder::new(3.2, 0.5)),
            boss: meshes.add(Sphere::new(0.8)),
            tower_shield: meshes.add(Cuboid::new(5.0, 7.5, 0.6)),
            sword: meshes.add(Cuboid::new(0.45, 5.5, 0.25)),
            club: meshes.add(Cone { radius: 1.0, height: 5.0 }),
            spear: meshes.add(Cylinder::new(0.28, 14.0)),
            pike: meshes.add(Cylinder::new(0.28, 24.0)),
            tip: meshes.add(Cone { radius: 0.7, height: 2.0 }),
            bow: meshes.add(Cuboid::new(0.4, 4.4, 0.4)),
            quiver: meshes.add(Cylinder::new(0.9, 4.5)),
            staff: meshes.add(Cylinder::new(0.28, 12.0)),
            cart_box: meshes.add(Cuboid::new(6.0, 2.6, 6.5)),
            wheel: meshes.add(Cylinder::new(2.2, 0.6).mesh().resolution(12).build().rotated_by(Quat::from_rotation_z(half_turn))),
            shaft: meshes.add(Cuboid::new(0.4, 0.4, 7.0)),
            sack: meshes.add(Sphere::new(1.7)),
            bundle: meshes.add(Cuboid::new(3.0, 2.0, 2.4)),
            horse_body: meshes.add(Cuboid::new(4.0, 4.0, 10.0)),
            horse_leg: meshes.add(Cylinder::new(0.8, 4.5)),
            horse_neck: meshes.add(Cuboid::new(2.2, 4.5, 2.2)),
            horse_head: meshes.add(Cuboid::new(1.8, 1.8, 3.6)),
            caparison: meshes.add(Cuboid::new(4.6, 3.2, 10.6)),
            skin: mat(Color::srgb(0.85, 0.66, 0.52), 0.8, 0.0),
            metal: mat(Color::srgb(0.70, 0.72, 0.76), 0.3, 0.6),
            dark_metal: mat(Color::srgb(0.30, 0.31, 0.34), 0.35, 0.6),
            wood: mat(Color::srgb(0.45, 0.30, 0.17), 0.9, 0.0),
            leather: mat(Color::srgb(0.52, 0.36, 0.20), 0.85, 0.0),
            linen: mat(Color::srgb(0.86, 0.80, 0.66), 0.9, 0.0),
            straw: mat(Color::srgb(0.86, 0.72, 0.38), 0.9, 0.0),
            forest: mat(Color::srgb(0.20, 0.36, 0.20), 0.85, 0.0),
            horse: mat(Color::srgb(0.38, 0.24, 0.14), 0.8, 0.0),
            horse_pale: mat(Color::srgb(0.70, 0.60, 0.48), 0.8, 0.0),
            dog: mat(Color::srgb(0.58, 0.42, 0.24), 0.85, 0.0),
            team: [mat(Color::srgb(0.20, 0.42, 0.90), 0.7, 0.0), mat(Color::srgb(0.78, 0.18, 0.15), 0.7, 0.0)],
        }
    }
}

/// Place des figurines d'une unité (repère 2D, par rapport au centre de la case) : un petit groupe
/// pour les militaires (trois fantassins, deux cavaliers), une seule figurine pour le colon et
/// l'éclaireur.
pub fn group(kind: UnitKind) -> Vec<Vec2> {
    use UnitKind::*;
    match kind {
        Settler | Scout => vec![Vec2::ZERO],
        LightCavalry | HeavyCavalry | HorseArcher => vec![Vec2::new(-3.4, 0.0), Vec2::new(3.4, 0.5)],
        _ => vec![Vec2::new(0.0, -2.8), Vec2::new(-4.0, 2.2), Vec2::new(4.0, 2.2)],
    }
}

/// Monte la figurine d'une unité sous `parent` (pieds à l'origine du parent).
pub fn spawn_figure(parent: &mut ChildSpawnerCommands, kit: &FigureKit, kind: UnitKind, hostile: bool, layer: &RenderLayers) {
    use UnitKind::*;
    let team = &kit.team[usize::from(hostile)];
    let mut part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, transform: Transform| {
        parent.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform, layer.clone()));
    };
    let at = Transform::from_xyz;
    let tilt_x = Quat::from_rotation_x;
    // Vêtements : de tous les jours pour le colon, l'éclaireur, la milice ; aux couleurs du camp
    // (ou armure) pour les autres
    let tunic = match kind {
        Settler => &kit.linen,
        Scout => &kit.forest,
        Militia => &kit.leather,
        HeavyWarrior | HeavyCavalry => &kit.dark_metal,
        _ => team,
    };
    // Cavalier : cheval (vers +z) et son cavalier dessus ; fantassin : jambes et corps
    let mounted = matches!(kind, LightCavalry | HeavyCavalry | HorseArcher);
    let seat = if mounted {
        let coat = if kind == LightCavalry { &kit.horse_pale } else { &kit.horse };
        part(&kit.horse_body, coat, at(0.0, 6.5, 0.0));
        for (x, z) in [(1.3, 3.6), (-1.3, 3.6), (1.3, -3.6), (-1.3, -3.6)] {
            part(&kit.horse_leg, coat, at(x, 2.25, z));
        }
        part(&kit.horse_neck, coat, at(0.0, 9.2, 5.2).with_rotation(tilt_x(0.45)));
        part(&kit.horse_head, coat, at(0.0, 10.8, 6.8));
        if kind == HeavyCavalry {
            // Caparaçon aux couleurs du camp
            part(&kit.caparison, team, at(0.0, 6.0, 0.0));
        }
        8.5
    } else {
        part(&kit.legs, if matches!(kind, Settler | Scout) { &kit.leather } else { tunic }, at(0.0, 2.0, 0.0));
        4.0
    };
    let wide = matches!(kind, HeavyWarrior | HeavyCavalry);
    part(if wide { &kit.torso_wide } else { &kit.torso }, tunic, at(0.0, seat + 2.5, 0.0));
    let head = seat + 6.4;
    part(&kit.head, &kit.skin, at(0.0, head, 0.0));
    // Coiffure
    match kind {
        Settler => {
            // Chapeau de paille à large bord
            part(&kit.hat_brim, &kit.straw, at(0.0, head + 1.0, 0.0));
            part(&kit.hat_top, &kit.straw, at(0.0, head + 1.8, 0.0));
        }
        Scout | ExpertArcher => part(&kit.hood, &kit.forest, at(0.0, head + 1.4, 0.0)),
        HeavyWarrior | HeavyCavalry => {
            part(&kit.helmet, &kit.dark_metal, at(0.0, head + 1.6, 0.0));
            part(&kit.plume, team, at(0.0, head + 3.2, 0.0));
        }
        Spearman | Pikeman | ShieldWarrior | LightCavalry => part(&kit.helmet, &kit.metal, at(0.0, head + 1.6, 0.0)),
        _ => {}
    }
    // Ce qu'il porte : arme dans la main droite (−x), bouclier ou arc à gauche (+x, en avant)
    let hand = seat + 3.5;
    let round_shield = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform)| {
        let face = tilt_x(std::f32::consts::FRAC_PI_2);
        part(&kit.shield, team, at(2.6, hand, 1.6).with_rotation(face));
        part(&kit.boss, &kit.metal, at(2.6, hand, 1.95));
    };
    let pole = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform), mesh: &Handle<Mesh>, length: f32, lean: f32| {
        let rot = tilt_x(lean);
        let center = Vec3::new(-2.9, hand + length * 0.3, 0.6);
        part(mesh, &kit.wood, Transform::from_translation(center).with_rotation(rot));
        part(&kit.tip, &kit.metal, Transform::from_translation(center + rot * Vec3::Y * (length / 2.0 + 1.0)).with_rotation(rot));
    };
    let bow = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform)| {
        // Arc haut : trois branches en « ( », tenu à gauche
        let (x, z) = (2.8, 1.4);
        part(&kit.bow, &kit.wood, at(x, hand + 0.6, z));
        part(&kit.bow, &kit.wood, at(x, hand + 4.4, z - 0.9).with_rotation(tilt_x(-0.45)));
        part(&kit.bow, &kit.wood, at(x, hand - 3.2, z - 0.9).with_rotation(tilt_x(0.45)));
    };
    let sword = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform)| {
        part(&kit.sword, &kit.metal, at(-2.9, hand + 1.7, 0.8).with_rotation(Quat::from_rotation_z(0.25)));
    };
    let quiver = |part: &mut dyn FnMut(&Handle<Mesh>, &Handle<StandardMaterial>, Transform)| {
        part(&kit.quiver, &kit.leather, at(-0.9, seat + 4.0, -2.4).with_rotation(tilt_x(-0.3)));
    };
    match kind {
        Settler => {
            // Charrette à bras derrière lui, chargée de sacs et d'un ballot
            let back = -6.5;
            part(&kit.cart_box, &kit.wood, at(0.0, 3.2, back));
            for x in [-3.4, 3.4] {
                part(&kit.wheel, &kit.wood, at(x, 2.2, back));
                part(&kit.shaft, &kit.wood, at(x * 0.55, 3.4, back + 5.5));
            }
            part(&kit.sack, &kit.linen, at(-1.3, 5.2, back + 1.2));
            part(&kit.sack, &kit.linen, at(1.4, 5.0, back - 1.4));
            part(&kit.bundle, &kit.leather, at(0.6, 5.4, back + 0.2).with_rotation(Quat::from_rotation_y(0.4)));
        }
        Scout => {
            part(&kit.staff, &kit.wood, at(-3.0, 6.0, 0.6));
            // Son chien, à sa gauche, un peu en avant
            let (x, z) = (4.2, 1.6);
            part(&kit.dog_body, &kit.dog, at(x, 2.9, z));
            for (dx, dz) in [(0.6, 1.6), (-0.6, 1.6), (0.6, -1.6), (-0.6, -1.6)] {
                part(&kit.dog_leg, &kit.dog, at(x + dx, 1.1, z + dz));
            }
            part(&kit.dog_head, &kit.dog, at(x, 4.0, z + 2.9));
            for dx in [0.5, -0.5] {
                part(&kit.dog_ear, &kit.dog, at(x + dx, 5.1, z + 2.6));
            }
            part(&kit.dog_tail, &kit.dog, at(x, 3.9, z - 2.9).with_rotation(tilt_x(-0.8)));
        }
        Militia => part(&kit.club, &kit.wood, at(-3.0, hand + 1.5, 0.8).with_rotation(Quat::from_rotation_z(0.3))),
        ShieldWarrior => {
            round_shield(&mut part);
            sword(&mut part);
        }
        HeavyWarrior => {
            part(&kit.tower_shield, team, at(1.8, hand - 0.5, 2.8));
            sword(&mut part);
        }
        Spearman => {
            round_shield(&mut part);
            pole(&mut part, &kit.spear, 14.0, 0.0);
        }
        Pikeman => pole(&mut part, &kit.pike, 24.0, 0.25),
        ArcherSwordsman => {
            bow(&mut part);
            sword(&mut part);
        }
        ExpertArcher => {
            bow(&mut part);
            quiver(&mut part);
        }
        LightCavalry => pole(&mut part, &kit.spear, 14.0, 0.35),
        HeavyCavalry => {
            round_shield(&mut part);
            pole(&mut part, &kit.spear, 14.0, 1.0);
        }
        HorseArcher => {
            bow(&mut part);
            quiver(&mut part);
        }
    }
}
