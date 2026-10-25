//! CivBuilder — un petit 4X au tour par tour en Rust avec Bevy.

// Lints courants avec Bevy (les systèmes ont naturellement beaucoup de paramètres)
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod fonts;
mod basins;
mod battle;
mod climate;
mod combat;
mod explore;
mod game;
mod hex;
mod highlight;
mod hostile;
mod hydro;
mod input;
mod labels;
mod map;
mod options;
mod menu;
mod pathfinding;
mod plates;
mod render;
mod relief;
mod rules;
mod seam;
mod sites;
mod tooltip;
mod ui;
mod units;
mod view3d;
mod waterways;
mod wiki;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "CivBuilder".into(),
                resolution: (1400, 860).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((fonts::FontsPlugin, game::GamePlugin, hostile::HostilePlugin, render::RenderPlugin, input::InputPlugin, ui::UiPlugin, view3d::View3dPlugin, labels::LabelsPlugin, highlight::HighlightPlugin, options::OptionsPlugin, wiki::WikiPlugin, menu::MenuPlugin, seam::SeamPlugin))
        .run();
}
