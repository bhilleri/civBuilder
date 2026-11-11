//! Charte graphique commune des écrans : couleurs, grandeurs (une couleur et une icône chacune),
//! icônes et hexagones dessinés par le code au démarrage (pas de fichier d'image à fournir).
//! Voir `docs/conception/conception-interface.md` (« Charte graphique ») et la maquette
//! `docs/conception/maquettes/ecran-ville.html`.

use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Couleur d'après ses composantes 0-255.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

pub const FOND: Color = rgb(0x0b, 0x0e, 0x15);
pub const PANNEAU: Color = rgb(0x13, 0x18, 0x24);
pub const SURFACE: Color = rgb(0x1a, 0x20, 0x30);
pub const SURFACE2: Color = rgb(0x22, 0x2a, 0x3c);
pub const TRAIT: Color = rgb(0x2a, 0x32, 0x45);
pub const TRAIT2: Color = rgb(0x3a, 0x43, 0x58);
pub const TEXTE: Color = rgb(0xe7, 0xe9, 0xef);
pub const DOUX: Color = rgb(0xb0, 0xb7, 0xc6);
pub const MUET: Color = rgb(0x8a, 0x92, 0xa5);
/// États : tout va bien, à surveiller, grave.
pub const OK: Color = rgb(0x4f, 0xc3, 0xa1);
pub const ALERTE: Color = rgb(0xf0, 0xa0, 0x3c);
pub const GRAVE: Color = rgb(0xf0, 0x8a, 0x8a);
/// Sélection (seul usage du blanc pur sur la carte).
pub const SELECTION: Color = Color::WHITE;
/// Bouton d'action principal (construire, mettre en file).
pub const ACTION_FOND: Color = rgb(0x3b, 0x6f, 0xe0);
/// Aménagement de production (rond vert).
pub const AMENAGEMENT: Color = rgb(0xa6, 0xdc, 0x8e);
pub const AMENAGEMENT_FOND: Color = rgb(0x1f, 0x33, 0x20);
/// Centre de la ville sur la carte de la ville.
pub const CENTRE_VILLE: Color = rgb(0x2a, 0x3a, 0x5c);

/// État d'une grandeur ou d'une alerte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etat {
    Ok,
    Alerte,
    Grave,
}

impl Etat {
    pub const fn couleur(self) -> Color {
        match self {
            Etat::Ok => OK,
            Etat::Alerte => ALERTE,
            Etat::Grave => GRAVE,
        }
    }
}

/// Les grandeurs de la ville : chacune a sa couleur et son icône partout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grandeur {
    Nourriture,
    Production,
    Deniers,
    Eau,
    Recherche,
    Culture,
    Defense,
    BienEtre,
    Habitants,
}

impl Grandeur {
    pub const TOUTES: [Grandeur; 9] = [
        Grandeur::Nourriture,
        Grandeur::Production,
        Grandeur::Deniers,
        Grandeur::Eau,
        Grandeur::Recherche,
        Grandeur::Culture,
        Grandeur::Defense,
        Grandeur::BienEtre,
        Grandeur::Habitants,
    ];

    pub const fn nom(self) -> &'static str {
        match self {
            Grandeur::Nourriture => "Nourriture",
            Grandeur::Production => "Production",
            Grandeur::Deniers => "Deniers",
            Grandeur::Eau => "Eau",
            Grandeur::Recherche => "Recherche",
            Grandeur::Culture => "Culture",
            Grandeur::Defense => "Défense",
            Grandeur::BienEtre => "Bien-être",
            Grandeur::Habitants => "Habitants",
        }
    }

    pub const fn couleur(self) -> Color {
        match self {
            Grandeur::Nourriture => rgb(0x97, 0xc4, 0x59),
            Grandeur::Production => rgb(0xd9, 0xa0, 0x66),
            Grandeur::Deniers => rgb(0xf2, 0xd1, 0x6b),
            Grandeur::Eau => rgb(0x5d, 0xc4, 0xe8),
            Grandeur::Recherche => rgb(0x5d, 0xca, 0xa5),
            Grandeur::Culture => rgb(0xed, 0x93, 0xb1),
            Grandeur::Defense => rgb(0xb4, 0xb2, 0xa9),
            Grandeur::BienEtre | Grandeur::Habitants => rgb(0xc9, 0xb3, 0xff),
        }
    }
}

/// Icônes d'aménagement (rond vert sur la carte de la ville).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconeAmenagement {
    Champ,
    Paturage,
    CoupeDeBois,
}

impl From<crate::intendant::Amenagement> for IconeAmenagement {
    fn from(a: crate::intendant::Amenagement) -> Self {
        match a {
            crate::intendant::Amenagement::Champ => IconeAmenagement::Champ,
            crate::intendant::Amenagement::Paturage => IconeAmenagement::Paturage,
            crate::intendant::Amenagement::CoupeDeBois => IconeAmenagement::CoupeDeBois,
        }
    }
}

/// Images dessinées au démarrage : icônes des grandeurs et des aménagements, hexagone plein et
/// contour d'hexagone (teintés à l'affichage).
#[derive(Resource)]
pub struct Images {
    grandeurs: HashMap<Grandeur, Handle<Image>>,
    amenagements: HashMap<IconeAmenagement, Handle<Image>>,
    pub hexagone: Handle<Image>,
    pub contour: Handle<Image>,
}

impl Images {
    pub fn grandeur(&self, g: Grandeur) -> Handle<Image> {
        self.grandeurs[&g].clone()
    }

    pub fn amenagement(&self, a: IconeAmenagement) -> Handle<Image> {
        self.amenagements[&a].clone()
    }
}

pub struct ChartePlugin;

impl Plugin for ChartePlugin {
    fn build(&self, app: &mut App) {
        let images = {
            let mut assets = app.world_mut().resource_mut::<Assets<Image>>();
            let mut ajouter = |forme: &dyn Fn(f32, f32) -> bool, cote: u32| assets.add(dessiner(cote, forme));
            let grandeurs = Grandeur::TOUTES.iter().map(|&g| (g, ajouter(&|x, y| icone_grandeur(g, x, y), 48))).collect();
            let amenagements = [IconeAmenagement::Champ, IconeAmenagement::Paturage, IconeAmenagement::CoupeDeBois]
                .iter()
                .map(|&a| (a, ajouter(&|x, y| icone_amenagement(a, x, y), 48)))
                .collect();
            let hexagone = ajouter(&|x, y| dans_hexagone(x, y, 1.0), 128);
            let contour = ajouter(&|x, y| dans_hexagone(x, y, 1.0) && !dans_hexagone(x, y, 0.88), 128);
            Images { grandeurs, amenagements, hexagone, contour }
        };
        app.insert_resource(images);
    }
}

// ------------------------------------------------------------------ Dessin

/// Image carrée de `cote` pixels, blanche là où `forme` (coordonnées de −1 à 1, y vers le bas) est
/// vraie, avec un lissage par sur-échantillonnage (4 × 4 points par pixel).
fn dessiner(cote: u32, forme: &dyn Fn(f32, f32) -> bool) -> Image {
    let mut data = Vec::with_capacity((cote * cote * 4) as usize);
    for py in 0..cote {
        for px in 0..cote {
            let mut n = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = ((px as f32 + (sx as f32 + 0.5) / 4.0) / cote as f32) * 2.0 - 1.0;
                    let y = ((py as f32 + (sy as f32 + 0.5) / 4.0) / cote as f32) * 2.0 - 1.0;
                    if forme(x, y) {
                        n += 1;
                    }
                }
            }
            data.extend_from_slice(&[255, 255, 255, (n * 255 / 16) as u8]);
        }
    }
    Image::new(
        Extent3d { width: cote, height: cote, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Hexagone « pointe en haut » inscrit dans le carré : largeur √3/2 de la hauteur, centré.
fn dans_hexagone(x: f32, y: f32, k: f32) -> bool {
    // Hauteur : de −k à k ; demi-largeur : k·√3/2. On ramène l'hexagone au rectangle de l'image
    // (largeur √3/2 de la hauteur) en étirant x.
    let x = x * 0.866;
    let (ax, ay) = (x.abs(), y.abs());
    ax <= k * 0.866 && ay + ax / 1.732 <= k
}

fn disque(x: f32, y: f32, cx: f32, cy: f32, r: f32) -> bool {
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

fn anneau(x: f32, y: f32, cx: f32, cy: f32, r1: f32, r2: f32) -> bool {
    let d = (x - cx).powi(2) + (y - cy).powi(2);
    d <= r2 * r2 && d >= r1 * r1
}

/// Segment épais (capsule) de `a` à `b`, de rayon `r`.
fn capsule(x: f32, y: f32, a: (f32, f32), b: (f32, f32), r: f32) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    disque(x, y, a.0 + t * dx, a.1 + t * dy, r)
}

fn rectangle(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    x >= x0 && x <= x1 && y >= y0 && y <= y1
}

/// Triangle (sommets dans n'importe quel ordre).
fn triangle(x: f32, y: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let s = |p: (f32, f32), q: (f32, f32)| (q.0 - p.0) * (y - p.1) - (q.1 - p.1) * (x - p.0);
    let (d1, d2, d3) = (s(a, b), s(b, c), s(c, a));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// Ellipse de centre `c`, demi-axes `rx`, `ry`, tournée de `angle` radians.
fn ellipse(x: f32, y: f32, c: (f32, f32), rx: f32, ry: f32, angle: f32) -> bool {
    let (s, co) = angle.sin_cos();
    let (dx, dy) = (x - c.0, y - c.1);
    let (u, v) = (dx * co + dy * s, -dx * s + dy * co);
    (u / rx).powi(2) + (v / ry).powi(2) <= 1.0
}

/// Formes simples des icônes des grandeurs.
fn icone_grandeur(g: Grandeur, x: f32, y: f32) -> bool {
    match g {
        // Épi : tige et grains de part et d'autre
        Grandeur::Nourriture => {
            capsule(x, y, (0.0, 0.85), (0.0, -0.35), 0.07)
                || (0..4).any(|i| {
                    let cy = 0.35 - i as f32 * 0.3;
                    ellipse(x, y, (-0.2, cy), 0.13, 0.24, 0.6) || ellipse(x, y, (0.2, cy), 0.13, 0.24, -0.6)
                })
                || ellipse(x, y, (0.0, -0.72), 0.12, 0.2, 0.0)
        }
        // Marteau : manche en biais, tête
        Grandeur::Production => {
            capsule(x, y, (-0.65, 0.75), (0.25, -0.15), 0.1) || {
                let (s, c) = (-0.785f32).sin_cos();
                let (u, v) = ((x - 0.3) * c + (y + 0.25) * s, -(x - 0.3) * s + (y + 0.25) * c);
                rectangle(u, v, -0.55, -0.2, 0.55, 0.2)
            }
        }
        // Pièce : anneau et disque
        Grandeur::Deniers => anneau(x, y, 0.0, 0.0, 0.62, 0.8) || disque(x, y, 0.0, 0.0, 0.42),
        // Goutte : disque et pointe
        Grandeur::Eau => disque(x, y, 0.0, 0.3, 0.52) || triangle(x, y, (0.0, -0.85), (-0.47, 0.12), (0.47, 0.12)),
        // Fiole : panse et col
        Grandeur::Recherche => {
            triangle(x, y, (0.0, -0.3), (-0.7, 0.8), (0.7, 0.8)) && y > -0.1
                || rectangle(x, y, -0.16, -0.8, 0.16, -0.05)
                || rectangle(x, y, -0.3, -0.85, 0.3, -0.7)
        }
        // Plume : barbe en ellipse, tuyau
        Grandeur::Culture => ellipse(x, y, (0.12, -0.15), 0.25, 0.72, 0.6) || capsule(x, y, (-0.6, 0.8), (0.2, -0.25), 0.05),
        // Bouclier : haut droit, pointe en bas
        Grandeur::Defense => rectangle(x, y, -0.65, -0.75, 0.65, 0.05) || triangle(x, y, (-0.65, 0.05), (0.65, 0.05), (0.0, 0.85)),
        // Visage souriant
        Grandeur::BienEtre => {
            (anneau(x, y, 0.0, 0.0, 0.68, 0.84))
                || disque(x, y, -0.27, -0.2, 0.11)
                || disque(x, y, 0.27, -0.2, 0.11)
                || (anneau(x, y, 0.0, 0.0, 0.35, 0.48) && y > 0.12)
        }
        // Deux personnes
        Grandeur::Habitants => {
            disque(x, y, -0.32, -0.38, 0.22)
                || disque(x, y, 0.32, -0.38, 0.22)
                || (ellipse(x, y, (-0.32, 0.45), 0.38, 0.38, 0.0) && y < 0.62)
                || (ellipse(x, y, (0.32, 0.45), 0.38, 0.38, 0.0) && y < 0.62)
        }
    }
}

/// Formes des icônes d'aménagement.
fn icone_amenagement(a: IconeAmenagement, x: f32, y: f32) -> bool {
    match a {
        // Champ : trois sillons en biais
        IconeAmenagement::Champ => (0..3).any(|i| capsule(x, y, (-0.7 + i as f32 * 0.5, 0.7), (-0.2 + i as f32 * 0.5, -0.7), 0.1)),
        // Pâturage : clôture
        IconeAmenagement::Paturage => {
            rectangle(x, y, -0.6, -0.6, -0.4, 0.7) || rectangle(x, y, 0.4, -0.6, 0.6, 0.7) || rectangle(x, y, -0.8, -0.3, 0.8, -0.15) || rectangle(x, y, -0.8, 0.2, 0.8, 0.35)
        }
        // Coupe de bois : sapin et tronc
        IconeAmenagement::CoupeDeBois => triangle(x, y, (0.0, -0.85), (-0.6, 0.4), (0.6, 0.4)) || rectangle(x, y, -0.12, 0.4, 0.12, 0.8),
    }
}

// ------------------------------------------------------------------ Petits éléments communs

/// Texte de l'interface.
pub fn texte(s: impl Into<String>, taille: f32, couleur: Color) -> (Text, TextFont, TextColor) {
    (Text::new(s.into()), TextFont::from_font_size(taille), TextColor(couleur))
}

/// Icône carrée de `taille` pixels, teintée.
pub fn icone(image: Handle<Image>, taille: f32, couleur: Color) -> impl Bundle {
    (Node { width: Val::Px(taille), height: Val::Px(taille), flex_shrink: 0.0, ..default() }, ImageNode::new(image).with_color(couleur))
}

/// Barre de remplissage (part de 0 à 1).
pub fn barre(part: f32, largeur: f32, couleur: Color) -> impl Bundle {
    (
        Node { width: Val::Px(largeur), height: Val::Px(6.0), border_radius: BorderRadius::all(Val::Px(3.0)), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
        BackgroundColor(TRAIT),
        children![(Node { width: Val::Percent(part.clamp(0.0, 1.0) * 100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(couleur))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexagone_plein_au_centre_vide_aux_coins() {
        assert!(dans_hexagone(0.0, 0.0, 1.0));
        assert!(dans_hexagone(0.0, 0.95, 1.0));
        assert!(!dans_hexagone(0.95, 0.95, 1.0));
        assert!(!dans_hexagone(-0.95, -0.95, 1.0));
    }

    #[test]
    fn chaque_icone_dessine_quelque_chose() {
        for g in Grandeur::TOUTES {
            let image = dessiner(16, &|x, y| icone_grandeur(g, x, y));
            let pleins = image.data.as_ref().unwrap().chunks(4).filter(|p| p[3] > 128).count();
            assert!(pleins > 20, "{} : {pleins} pixels", g.nom());
        }
    }
}
