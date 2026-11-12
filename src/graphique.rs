//! Graphiques de l'interface, dessinés par le code dans une image : courbe du passé, prévision au
//! rythme actuel (avec une bande d'incertitude), seuils. Le calcul (échelle, tendance, positions)
//! est séparé du dessin et testé ; les libellés sont posés par l'interface aux positions données.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Ligne horizontale de repère (besoin, limite, équilibre…).
#[derive(Clone, Debug)]
pub struct Seuil {
    /// Valeur d'aujourd'hui (le libellé s'y pose).
    pub valeur: f64,
    pub libelle: String,
    pub couleur: Color,
    /// Seuil qui change avec le temps : ses valeurs passées (alignées sur `Courbe::passe`) et
    /// prévues (alignées sur la prévision) ; sinon une droite horizontale.
    pub serie: Option<(Vec<f64>, Vec<f64>)>,
}

/// Ce que montre un graphique : le passé (le plus ancien d'abord, le dernier est « aujourd'hui »),
/// la prévision sur `futur` tours, les seuils.
#[derive(Clone, Debug)]
pub struct Courbe {
    pub passe: Vec<f64>,
    pub futur: usize,
    pub seuils: Vec<Seuil>,
    pub couleur: Color,
    /// Prévision donnée tour par tour (sinon : la tendance des derniers tours, avec sa bande).
    pub prevision: Option<Vec<f64>>,
    /// Échelle logarithmique (valeurs strictement positives), bornée autour des valeurs utiles.
    pub log: bool,
}

impl Courbe {
    /// Courbe à échelle linéaire, prévision par la tendance.
    pub fn simple(passe: Vec<f64>, futur: usize, seuils: Vec<Seuil>, couleur: Color) -> Courbe {
        Courbe { passe, futur, seuils, couleur, prevision: None, log: false }
    }
}

/// Marges du tracé dans l'image (pixels) : à gauche pour les valeurs, en bas pour les tours.
pub const MARGE_GAUCHE: f32 = 46.0;
pub const MARGE_DROITE: f32 = 12.0;
pub const MARGE_HAUT: f32 = 14.0;
pub const MARGE_BAS: f32 = 22.0;

/// Pente par tour au rythme actuel : moindres carrés sur les 10 derniers tours au plus.
pub fn tendance(valeurs: &[f64]) -> f64 {
    let v = &valeurs[valeurs.len().saturating_sub(10)..];
    let n = v.len() as f64;
    if v.len() < 2 {
        return 0.0;
    }
    let mx = (n - 1.0) / 2.0;
    let my = v.iter().sum::<f64>() / n;
    let (mut num, mut den) = (0.0, 0.0);
    for (i, y) in v.iter().enumerate() {
        let dx = i as f64 - mx;
        num += dx * (y - my);
        den += dx * dx;
    }
    num / den
}

/// Mise en page d'une courbe dans une image de `taille` pixels.
pub struct Trace {
    pub taille: Vec2,
    pub min: f64,
    pub max: f64,
    pub pente: f64,
    n: usize,
    futur: usize,
    log: bool,
}

impl Trace {
    pub fn new(c: &Courbe, taille: Vec2) -> Trace {
        let pente = tendance(&c.passe);
        let dernier = c.passe.last().copied().unwrap_or(0.0);
        let fin = dernier + pente * c.futur as f64;
        let inc = incertitude(dernier, c.futur);
        let prevues: Vec<f64> = match &c.prevision {
            Some(p) => p.clone(),
            None => vec![fin + inc, fin - inc],
        };
        let series = c.seuils.iter().filter_map(|s| s.serie.as_ref()).flat_map(|(a, b)| a.iter().chain(b.iter()).copied());
        let valeurs = c.passe.iter().copied().chain(prevues).chain(c.seuils.iter().map(|s| s.valeur)).chain(series);
        let (mut min, mut max) = valeurs.filter(|v| !c.log || *v > 0.0).fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
        if c.log {
            // (bornée autour des valeurs utiles, pas depuis zéro)
            min = (min * 0.8).max(1e-6);
            max = (max * 1.25).max(min * 2.0);
        } else {
            min = min.min(0.0);
            if max <= min {
                max = min + 1.0;
            }
            max += (max - min) * 0.1;
        }
        Trace { taille, min, max, pente, n: c.passe.len(), futur: c.futur, log: c.log }
    }

    /// Abscisse du tour `t` (0 : aujourd'hui, négatif : passé, positif : prévision).
    pub fn x(&self, t: f64) -> f32 {
        let total = (self.n.max(1) - 1 + self.futur).max(1) as f64;
        let k = (t + (self.n.max(1) - 1) as f64) / total;
        MARGE_GAUCHE + k as f32 * (self.taille.x - MARGE_GAUCHE - MARGE_DROITE)
    }

    /// Ordonnée d'une valeur (y vers le bas).
    pub fn y(&self, v: f64) -> f32 {
        let k = if self.log { (v.max(self.min).ln() - self.min.ln()) / (self.max.ln() - self.min.ln()) } else { (v - self.min) / (self.max - self.min) };
        MARGE_HAUT + (1.0 - k as f32) * (self.taille.y - MARGE_HAUT - MARGE_BAS)
    }

    /// Valeurs à graduer : bas, milieu, haut ; en échelle logarithmique, les 1, 2, 5 de chaque puissance de 10.
    pub fn graduations(&self) -> Vec<f64> {
        if self.log {
            let mut g = Vec::new();
            let (e0, e1) = (self.min.log10().floor() as i32, self.max.log10().ceil() as i32);
            for e in e0..=e1 {
                for m in [1.0, 2.0, 5.0] {
                    let v = m * 10f64.powi(e);
                    if v >= self.min && v <= self.max {
                        g.push(v);
                    }
                }
            }
            return g;
        }
        let haut = self.min + (self.max - self.min) / 1.1;
        vec![self.min, (self.min + haut) / 2.0, haut]
    }
}

/// Demi-largeur de la bande d'incertitude après `t` tours.
fn incertitude(v: f64, t: usize) -> f64 {
    (v.abs() * 0.012 + 0.004) * t as f64
}

/// Toile RGBA où l'on peint avec lissage.
struct Toile {
    l: u32,
    h: u32,
    px: Vec<[f32; 4]>,
}

impl Toile {
    fn new(l: u32, h: u32) -> Toile {
        Toile { l, h, px: vec![[0.0; 4]; (l * h) as usize] }
    }

    /// Mélange une couleur d'opacité `a` sur le pixel.
    fn peindre(&mut self, x: i32, y: i32, c: Srgba, a: f32) {
        if x < 0 || y < 0 || x >= self.l as i32 || y >= self.h as i32 || a <= 0.0 {
            return;
        }
        let p = &mut self.px[(y as u32 * self.l + x as u32) as usize];
        let a = a.min(1.0);
        let out = a + p[3] * (1.0 - a);
        if out > 0.0 {
            for (k, v) in [c.red, c.green, c.blue].into_iter().enumerate() {
                p[k] = (v * a + p[k] * p[3] * (1.0 - a)) / out;
            }
        }
        p[3] = out;
    }

    /// Segment épais lissé, éventuellement en tirets (longueurs `tiret`, `blanc`).
    fn segment(&mut self, a: Vec2, b: Vec2, epaisseur: f32, c: Color, alpha: f32, tirets: Option<(f32, f32)>, depart: f32) {
        let c = c.to_srgba();
        let r = epaisseur / 2.0;
        let (x0, x1) = ((a.x.min(b.x) - r - 1.0).floor() as i32, (a.x.max(b.x) + r + 1.0).ceil() as i32);
        let (y0, y1) = ((a.y.min(b.y) - r - 1.0).floor() as i32, (a.y.max(b.y) + r + 1.0).ceil() as i32);
        let d = b - a;
        let l2 = d.length_squared().max(1e-6);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let t = ((p - a).dot(d) / l2).clamp(0.0, 1.0);
                if let Some((plein, vide)) = tirets {
                    let s = depart + t * l2.sqrt();
                    if s.rem_euclid(plein + vide) > plein {
                        continue;
                    }
                }
                let dist = (p - (a + d * t)).length();
                let cov = (r + 0.5 - dist).clamp(0.0, 1.0);
                self.peindre(x, y, c, cov * alpha);
            }
        }
    }

    /// Ligne brisée (les tirets continuent d'un segment à l'autre).
    fn ligne(&mut self, pts: &[Vec2], epaisseur: f32, c: Color, alpha: f32, tirets: Option<(f32, f32)>) {
        let mut s = 0.0;
        for w in pts.windows(2) {
            self.segment(w[0], w[1], epaisseur, c, alpha, tirets, s);
            s += (w[1] - w[0]).length();
        }
    }

    /// Bande entre deux bords (même abscisses, de gauche à droite) remplie d'une couleur.
    fn bande(&mut self, haut: &[Vec2], bas: &[Vec2], c: Color, alpha: f32) {
        let c = c.to_srgba();
        for w in 0..haut.len().saturating_sub(1) {
            let (a, b) = (haut[w], haut[w + 1]);
            for x in a.x.floor() as i32..b.x.ceil() as i32 {
                let k = ((x as f32 + 0.5 - a.x) / (b.x - a.x).max(1e-3)).clamp(0.0, 1.0);
                let yh = a.y + (b.y - a.y) * k;
                let yb = bas[w].y + (bas[w + 1].y - bas[w].y) * k;
                for y in yh.floor() as i32..=yb.ceil() as i32 {
                    self.peindre(x, y, c, alpha);
                }
            }
        }
    }

    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color, alpha: f32) {
        let c = c.to_srgba();
        for y in y0 as i32..y1 as i32 {
            for x in x0 as i32..x1 as i32 {
                self.peindre(x, y, c, alpha);
            }
        }
    }

    fn image(self) -> Image {
        let data = self.px.iter().flat_map(|p| [(p[0] * 255.0) as u8, (p[1] * 255.0) as u8, (p[2] * 255.0) as u8, (p[3] * 255.0) as u8]).collect();
        Image::new(Extent3d { width: self.l, height: self.h, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())
    }
}

/// Dessine la courbe : zone de prévision, graduations, seuils, passé, bande et ligne de la
/// prévision, trait d'aujourd'hui. Les libellés sont à poser à part (voir `Trace`).
pub fn dessiner(c: &Courbe, tr: &Trace, grille: Color, aujourdhui: Color) -> Image {
    let (l, h) = (tr.taille.x.max(2.0) as u32, tr.taille.y.max(2.0) as u32);
    let mut t = Toile::new(l, h);
    let (haut, bas) = (MARGE_HAUT, tr.taille.y - MARGE_BAS);
    t.rect(tr.x(0.0), haut, tr.x(c.futur as f64), bas, Color::WHITE, 0.035);
    for v in tr.graduations() {
        t.segment(Vec2::new(MARGE_GAUCHE, tr.y(v)), Vec2::new(tr.taille.x - MARGE_DROITE, tr.y(v)), 1.0, grille, 1.0, None, 0.0);
    }
    let n = c.passe.len();
    for s in &c.seuils {
        match &s.serie {
            Some((avant, apres)) => {
                // (le passé finit aujourd'hui, la prévision part de là)
                let k0 = avant.len() as f64 - 1.0;
                let pts: Vec<Vec2> = avant
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Vec2::new(tr.x(i as f64 - k0), tr.y(*v)))
                    .chain(apres.iter().enumerate().map(|(k, v)| Vec2::new(tr.x(k as f64 + 1.0), tr.y(*v))))
                    .collect();
                t.ligne(&pts, 1.2, s.couleur, 1.0, Some((6.0, 4.0)));
            }
            None => t.segment(Vec2::new(MARGE_GAUCHE, tr.y(s.valeur)), Vec2::new(tr.taille.x - MARGE_DROITE, tr.y(s.valeur)), 1.2, s.couleur, 1.0, Some((6.0, 4.0)), 0.0),
        }
    }
    let passe: Vec<Vec2> = c.passe.iter().enumerate().map(|(i, v)| Vec2::new(tr.x(i as f64 - (n as f64 - 1.0)), tr.y(*v))).collect();
    let v0 = c.passe.last().copied().unwrap_or(0.0);
    match &c.prevision {
        Some(p) => {
            let pts: Vec<Vec2> = std::iter::once(v0).chain(p.iter().copied()).enumerate().map(|(k, v)| Vec2::new(tr.x(k as f64), tr.y(v))).collect();
            t.ligne(&pts, 2.0, c.couleur, 1.0, Some((6.0, 4.0)));
        }
        None => {
            let futur: Vec<f64> = (0..=c.futur).map(|k| k as f64).collect();
            let bord = |signe: f64| -> Vec<Vec2> { futur.iter().map(|&k| Vec2::new(tr.x(k), tr.y(v0 + tr.pente * k + signe * incertitude(v0, k as usize)))).collect() };
            t.bande(&bord(1.0), &bord(-1.0), c.couleur, 0.16);
            t.ligne(&[Vec2::new(tr.x(0.0), tr.y(v0)), Vec2::new(tr.x(c.futur as f64), tr.y(v0 + tr.pente * c.futur as f64))], 2.0, c.couleur, 1.0, Some((6.0, 4.0)));
        }
    }
    t.ligne(&passe, 2.4, c.couleur, 1.0, None);
    t.segment(Vec2::new(tr.x(0.0), haut), Vec2::new(tr.x(0.0), bas), 1.0, aujourdhui, 0.8, None, 0.0);
    let p0 = Vec2::new(tr.x(0.0), tr.y(v0));
    t.segment(p0, p0, 8.0, Color::WHITE, 1.0, None, 0.0);
    t.image()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tendance_d_une_droite() {
        assert!((tendance(&[1.0, 2.0, 3.0, 4.0]) - 1.0).abs() < 1e-9);
        assert_eq!(tendance(&[5.0]), 0.0);
        // seuls les 10 derniers tours comptent
        let v: Vec<f64> = (0..30).map(|i| if i < 20 { 100.0 - i as f64 } else { 2.0 * i as f64 }).collect();
        assert!((tendance(&v) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn l_echelle_contient_la_courbe_les_seuils_et_zero() {
        let c = Courbe::simple(vec![3.0, 4.0, 5.0], 20, vec![Seuil { valeur: 8.0, libelle: "besoin".into(), couleur: Color::WHITE, serie: None }], Color::WHITE);
        let tr = Trace::new(&c, Vec2::new(400.0, 200.0));
        assert!(tr.min <= 0.0 && tr.max >= 8.0);
        assert!(tr.x(-2.0) < tr.x(0.0) && tr.x(0.0) < tr.x(20.0));
        assert!((tr.x(20.0) - (400.0 - MARGE_DROITE)).abs() < 1e-3);
        assert!(tr.y(8.0) < tr.y(3.0), "plus haut pour une valeur plus grande");
    }

    #[test]
    fn echelle_logarithmique_bornee_autour_des_valeurs() {
        let c = Courbe { passe: vec![1_000.0, 1_500.0, 2_400.0], futur: 3, seuils: vec![Seuil { valeur: 9_000.0, libelle: "nourriture".into(), couleur: Color::WHITE, serie: None }], couleur: Color::WHITE, prevision: Some(vec![2_600.0, 2_800.0, 3_000.0]), log: true };
        let tr = Trace::new(&c, Vec2::new(400.0, 200.0));
        assert!(tr.min > 0.0 && tr.min <= 1_000.0 && tr.max >= 9_000.0);
        // à rapport égal, même écart à l'écran
        let (a, b, d) = (tr.y(1_000.0), tr.y(2_000.0), tr.y(4_000.0));
        assert!(((a - b) - (b - d)).abs() < 1e-3);
        assert!(tr.graduations().contains(&2_000.0) && tr.graduations().contains(&5_000.0));
    }

    #[test]
    fn le_dessin_a_la_taille_demandee_et_n_est_pas_vide() {
        let c = Courbe::simple(vec![1.0, 2.0, 2.5, 3.0], 10, Vec::new(), Color::srgb(0.6, 0.8, 0.3));
        let tr = Trace::new(&c, Vec2::new(200.0, 100.0));
        let img = dessiner(&c, &tr, Color::BLACK, Color::WHITE);
        assert_eq!((img.width(), img.height()), (200, 100));
        let peints = img.data.as_ref().unwrap().chunks(4).filter(|p| p[3] > 100).count();
        assert!(peints > 200, "{peints}");
    }
}
