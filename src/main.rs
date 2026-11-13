//! CivBuilder — un petit 4X au tour par tour en Rust avec Bevy.

// Lints courants avec Bevy (les systèmes ont naturellement beaucoup de paramètres)
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod accueil;
mod fonts;
mod basins;
mod battle;
mod bridge_build;
mod capture;
mod cases;
mod charte;
mod chrono_tour;
mod climate;
mod combat;
mod config_partie;
mod console;
mod currents;
mod debug_cmd;
mod echanges;
mod ecran_ville;
mod erosion;
mod explore;
mod feedback;
mod film;
mod figures;
mod game;
mod groupes;
mod hasard;
mod geography;
mod geologie;
mod gisements;
mod graphique;
mod hex;
mod highlight;
mod hostile;
mod hydro;
mod input;
mod intendant;
mod labels;
mod lands;
mod map;
mod options;
mod menu;
mod mountains;
mod mouths;
mod pathfinding;
mod peuple;
mod pilote;
mod plates;
mod pointer;
mod render;
mod regions;
mod relief;
mod rules;
mod sauvegarde;
mod seam;
mod search;
mod seas;
mod settings;
mod settlement_mesh;
mod settlements;
mod sites;
mod tableau;
mod tides;
mod tooltip;
mod toponymy;
mod ui;
mod units;
mod view3d;
mod ville_carte;
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

/// Fenêtre du jeu (taille, écran).
fn fenetre() -> Window {
    Window {
        title: "CivBuilder".into(),
        resolution: window_size().into(),
        // `CIVBUILDER_ECRAN=2` : ouvre la fenêtre sur cet écran (tests sans gêner le bureau)
        position: std::env::var("CIVBUILDER_ECRAN").ok().and_then(|e| e.parse::<usize>().ok()).filter(|&e| e >= 1).map_or(WindowPosition::Automatic, |e| {
            WindowPosition::Centered(MonitorSelection::Index(e - 1))
        }),
        ..default()
    }
}

fn main() {
    // Lancement normal : l'écran de nouvelle partie, qui relance le jeu sur la partie choisie
    if accueil::mode_accueil() {
        App::new()
            .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(fenetre()), ..default() }))
            .add_plugins((fonts::FontsPlugin, pointer::PointerPlugin, accueil::AccueilPlugin))
            .run();
        return;
    }
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(fenetre()), ..default() }))
        .add_plugins((fonts::FontsPlugin, game::GamePlugin, hostile::HostilePlugin, render::RenderPlugin, input::InputPlugin, ui::UiPlugin, view3d::View3dPlugin, labels::LabelsPlugin, highlight::HighlightPlugin, options::OptionsPlugin, wiki::WikiPlugin, menu::MenuPlugin, seam::SeamPlugin, search::SearchPlugin, capture::CapturePlugin))
        .add_plugins(console::ConsolePlugin)
        .add_plugins(tableau::TableauPlugin)
        .add_plugins(bridge_build::BridgeBuildPlugin)
        .add_plugins(feedback::FeedbackPlugin)
        .add_plugins(film::FilmPlugin)
        .add_plugins(pointer::PointerPlugin)
        .add_plugins((sauvegarde::SauvegardePlugin, groupes::GroupesPlugin, charte::ChartePlugin, ecran_ville::EcranVillePlugin, ville_carte::VilleCartePlugin))
        .run();
}
