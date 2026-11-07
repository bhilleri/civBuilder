//! CivBuilder — un petit 4X au tour par tour en Rust avec Bevy.

// Lints courants avec Bevy (les systèmes ont naturellement beaucoup de paramètres)
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod fonts;
mod basins;
mod battle;
mod capture;
mod climate;
mod combat;
mod console;
mod currents;
mod debug_cmd;
mod erosion;
mod explore;
mod feedback;
mod film;
mod figures;
mod game;
mod geography;
mod hex;
mod highlight;
mod hostile;
mod hydro;
mod input;
mod labels;
mod lands;
mod map;
mod options;
mod menu;
mod mountains;
mod mouths;
mod pathfinding;
mod pilote;
mod plates;
mod pointer;
mod render;
mod regions;
mod relief;
mod rules;
mod seam;
mod search;
mod seas;
mod settings;
mod settlement_mesh;
mod settlements;
mod sites;
mod tides;
mod tooltip;
mod toponymy;
mod ui;
mod units;
mod view3d;
mod waterways;
mod waves;
mod wiki;

use bevy::prelude::*;

/// Taille de la fenêtre au lancement : `CIVBUILDER_FENETRE=1920x1080`, sinon 1400 × 860.
fn window_size() -> (u32, u32) {
    std::env::var("CIVBUILDER_FENETRE").ok().and_then(|s| {
        let (w, h) = s.split_once('x')?;
        Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
    }).unwrap_or((1400, 860))
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "CivBuilder".into(),
                resolution: window_size().into(),
                // `CIVBUILDER_ECRAN=2` : ouvre la fenêtre sur cet écran (tests sans gêner le bureau)
                position: std::env::var("CIVBUILDER_ECRAN").ok().and_then(|e| e.parse::<usize>().ok()).filter(|&e| e >= 1).map_or(WindowPosition::Automatic, |e| {
                    WindowPosition::Centered(MonitorSelection::Index(e - 1))
                }),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((fonts::FontsPlugin, game::GamePlugin, hostile::HostilePlugin, render::RenderPlugin, input::InputPlugin, ui::UiPlugin, view3d::View3dPlugin, labels::LabelsPlugin, highlight::HighlightPlugin, options::OptionsPlugin, wiki::WikiPlugin, menu::MenuPlugin, seam::SeamPlugin, search::SearchPlugin, capture::CapturePlugin))
        .add_plugins(console::ConsolePlugin)
        .add_plugins(feedback::FeedbackPlugin)
        .add_plugins(film::FilmPlugin)
        .add_plugins(pointer::PointerPlugin)
        .run();
}
