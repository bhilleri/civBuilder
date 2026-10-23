//! Police du jeu : remplace la police par défaut de Bevy (limitée à l'ASCII)
//! par Inter, qui couvre les accents et symboles utilisés dans l'interface.
//! Inter est distribuée sous licence SIL Open Font License 1.1 (voir assets/fonts).

use bevy::prelude::*;

/// Police compilée dans l'exécutable : aucun fichier à fournir à côté du .exe.
const INTER: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.otf");

pub struct FontsPlugin;

impl Plugin for FontsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, install_default_font);
    }
}

/// Tous les textes qui n'indiquent pas de police utilisent la poignée par défaut :
/// on y installe Inter, ce qui change la police partout d'un coup.
fn install_default_font(mut fonts: ResMut<Assets<Font>>) {
    let _ = fonts.insert(&Handle::<Font>::default(), Font::from_bytes(INTER.to_vec()));
}
