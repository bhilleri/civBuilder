//! État du pointeur sur les éléments d'interface : remplace `Interaction`, déprécié en Bevy 0.20.
//! Survol lu dans le picking de Bevy, appui lu sur le bouton gauche de la souris.

use bevy::picking::PickingSystems;
use bevy::picking::hover::DirectlyHovered;
use bevy::prelude::*;
use bevy::ui::picking_backend::UiPickingSettings;

/// État du pointeur sur un élément d'interface. Seuls les éléments qui le portent (et les renvois du
/// journal) comptent pour le picking : le reste de l'interface laisse passer. Les champs de saisie le
/// portent aussi : le clic y donne le focus, et la souris compte comme au-dessus de l'interface.
#[derive(Component, Copy, Clone, Default, PartialEq, Eq, Debug)]
#[require(DirectlyHovered, Pickable)]
pub enum UiPointer {
    #[default]
    None,
    Hovered,
    Pressed,
}

impl UiPointer {
    /// État suivant : l'appui commence sur l'élément survolé et dure jusqu'au relâchement.
    fn next(self, hovered: bool, just_pressed: bool, just_released: bool) -> Self {
        if (hovered && just_pressed) || (self == UiPointer::Pressed && !just_released) {
            UiPointer::Pressed
        } else if hovered {
            UiPointer::Hovered
        } else {
            UiPointer::None
        }
    }
}

pub struct PointerPlugin;

impl Plugin for PointerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UiPickingSettings { require_markers: true })
            .add_systems(PreUpdate, update_ui_pointer.after(PickingSystems::Last));
    }
}

/// Met à jour l'état de chaque élément (sans le marquer changé s'il reste le même).
fn update_ui_pointer(mouse: Res<ButtonInput<MouseButton>>, mut nodes: Query<(&DirectlyHovered, &mut UiPointer)>) {
    let (pressed, released) = (mouse.just_pressed(MouseButton::Left), mouse.just_released(MouseButton::Left));
    for (hovered, mut pointer) in &mut nodes {
        let next = pointer.next(hovered.0, pressed, released);
        pointer.set_if_neq(next);
    }
}

#[cfg(test)]
mod tests {
    use super::UiPointer::{self, Hovered, None, Pressed};

    #[test]
    fn appui_seulement_sur_l_element_survole() {
        assert_eq!(None.next(true, true, false), Pressed);
        assert_eq!(None.next(false, true, false), None);
    }

    #[test]
    fn appui_jusqu_au_relachement_meme_hors_de_l_element() {
        assert_eq!(Pressed.next(false, false, false), Pressed);
        assert_eq!(Pressed.next(true, false, true), Hovered);
        assert_eq!(Pressed.next(false, false, true), UiPointer::None);
    }

    #[test]
    fn survol_sans_appui() {
        assert_eq!(None.next(true, false, false), Hovered);
        assert_eq!(Hovered.next(false, false, false), None);
    }
}
