//! Géologie de surface et sols (étape 5, livraisons 5a et 5b ; `docs/conception/conception-ressources.md`).
//!
//! Logique pure : la roche d'une case se déduit de ce que la génération connaît déjà (plaques et
//! leurs frontières, points chauds, altitude, relief, climat, cours d'eau), le sol de la roche et du
//! climat. Rien n'est tiré au hasard ici : un bruit lent, donné par la carte, départage seulement
//! les roches des bassins sédimentaires. Ces couches ne changent ni le terrain, ni le relief, ni
//! les biomes : même graine, même géologie.

use crate::map::{Relief, Terrain};
use crate::plates::Boundary;

/// Roche de surface d'une case de terre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Roche {
    /// Granite et gneiss : socles anciens, cœur des chaînes de collision.
    Granite,
    /// Roches volcaniques d'arc (andésite) : au-dessus des subductions.
    Volcanique,
    /// Basalte : rifts, points chauds, îles volcaniques.
    Basalte,
    /// Calcaire (et craie) : bassins bas, anciennes mers chaudes.
    Calcaire,
    /// Grès : piémonts des chaînes, bassins arides.
    Gres,
    /// Argile et marnes : bassins bas et humides.
    Argile,
    /// Alluvions : plaines des fleuves, deltas.
    Alluvions,
    /// Lœss : limons portés par le vent, au bord des steppes et des déserts froids.
    Loess,
}

impl Roche {
    pub const TOUTES: [Roche; 8] = [Roche::Granite, Roche::Volcanique, Roche::Basalte, Roche::Calcaire, Roche::Gres, Roche::Argile, Roche::Alluvions, Roche::Loess];

    pub const fn nom(self) -> &'static str {
        match self {
            Roche::Granite => "granite et gneiss",
            Roche::Volcanique => "roches volcaniques d'arc",
            Roche::Basalte => "basalte",
            Roche::Calcaire => "calcaire",
            Roche::Gres => "grès",
            Roche::Argile => "argile et marnes",
            Roche::Alluvions => "alluvions",
            Roche::Loess => "lœss",
        }
    }
}

/// Classe de sol (sept classes du pédologue), qui fixe la fertilité des champs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sol {
    /// Plaines des fleuves et deltas, rajeunies par les crues.
    Alluvial,
    /// Sols jeunes des volcans (cendres, laves récentes).
    VolcaniqueJeune,
    /// Sols noirs des steppes tempérées et sols de lœss.
    Noir,
    /// Sols bruns : la plupart des bassins calcaires et marneux.
    Brun,
    /// Sols lourds (argiles, vertisols) : fertiles mais durs à labourer.
    Lourd,
    /// Sols pauvres et acides : granite, grès ou froid humide.
    PauvreAcide,
    /// Sols lessivés des tropiques très arrosés et des vieux socles.
    LessiveTropical,
    /// Sols arides du désert (sable, regs) : pas de culture sans eau.
    Aride,
    /// Sols gelés de la toundra et des neiges (pergélisol) : pas de culture.
    Gele,
}

impl Sol {
    pub const TOUS: [Sol; 9] = [Sol::Alluvial, Sol::VolcaniqueJeune, Sol::Noir, Sol::Brun, Sol::Lourd, Sol::PauvreAcide, Sol::LessiveTropical, Sol::Aride, Sol::Gele];

    pub const fn nom(self) -> &'static str {
        match self {
            Sol::Alluvial => "sol alluvial",
            Sol::VolcaniqueJeune => "sol volcanique jeune",
            Sol::Noir => "sol noir",
            Sol::Brun => "sol brun",
            Sol::Lourd => "sol lourd",
            Sol::PauvreAcide => "sol pauvre et acide",
            Sol::LessiveTropical => "sol lessivé tropical",
            Sol::Aride => "sol aride",
            Sol::Gele => "sol gelé",
        }
    }

    /// Un champ y est-il possible (le désert et le froid l'interdisent, voir `Amenagement::possible`) ?
    pub const fn cultivable(self) -> bool {
        !matches!(self, Sol::Aride | Sol::Gele)
    }

    /// Effet pour l'affichage : « champs × 1,4 », ou « pas de culture ».
    pub fn effet_texte(self) -> String {
        if self.cultivable() { format!("champs × {}", self.fertilite_texte()) } else { "pas de culture".to_string() }
    }

    /// Multiplicateur de la nourriture que le champ tire du terrain (décision du 13/11/2026).
    pub const fn fertilite(self) -> f64 {
        match self {
            Sol::Alluvial => 1.4,
            Sol::VolcaniqueJeune | Sol::Noir => 1.3,
            Sol::Brun => 1.0,
            Sol::Lourd => 0.85,
            Sol::PauvreAcide => 0.7,
            Sol::LessiveTropical => 0.65,
            // (sans champ ; les pâturages de la toundra restent comme avant)
            Sol::Aride | Sol::Gele => 1.0,
        }
    }

    /// Couleur sur la carte de la fertilité (sRGB) : du vert sombre (très fertile) au rouge
    /// (pauvre), en passant par le jaune (sol brun, × 1).
    pub const fn couleur(self) -> (f32, f32, f32) {
        match self {
            Sol::Alluvial => (0.10, 0.42, 0.20),
            Sol::VolcaniqueJeune => (0.30, 0.52, 0.22),
            Sol::Noir => (0.22, 0.46, 0.30),
            Sol::Brun => (0.78, 0.74, 0.36),
            Sol::Lourd => (0.80, 0.58, 0.30),
            Sol::PauvreAcide => (0.84, 0.42, 0.24),
            Sol::LessiveTropical => (0.72, 0.24, 0.20),
            Sol::Aride => (0.62, 0.60, 0.56),
            Sol::Gele => (0.80, 0.83, 0.88),
        }
    }

    /// Multiplicateur écrit pour l'affichage : « 1,4 », « 0,85 ».
    pub fn fertilite_texte(self) -> String {
        let t = format!("{:.2}", self.fertilite());
        t.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
    }

    /// Multiplicateur des pâturages : un tiers de l'écart seulement (l'herbe suit la pluie).
    pub fn fertilite_paturage(self) -> f64 {
        1.0 + (self.fertilite() - 1.0) / 3.0
    }
}

/// Ce que la génération sait d'une case de terre, pour en déduire la roche et le sol.
#[derive(Clone, Copy, Debug)]
pub struct Lieu {
    /// Petite terre sur croûte océanique (île volcanique).
    pub ile_oceanique: bool,
    /// Frontière de plaques la plus proche : nature, distance (km), et la case est du côté de la
    /// plaque qui reste en surface (pas de celle qui plonge).
    pub frontiere: Option<(Boundary, f32, bool)>,
    /// Distance au point chaud le plus proche (km).
    pub point_chaud_km: f32,
    /// Altitude du sol (m).
    pub altitude: f32,
    pub relief: Relief,
    /// Température moyenne annuelle (°C) et précipitations annuelles (mm).
    pub temperature: f32,
    pub pluie: f32,
    /// Saisons sèches dans l'année (0 à 4).
    pub saisons_seches: u8,
    pub terrain: Terrain,
    /// Fleuve sur la case, ou terre gagnée par un delta.
    pub fleuve_ou_delta: bool,
    /// Une grande rivière borde la case (débit fort).
    pub grande_riviere: bool,
    /// Bruit lent de -1 à 1 (même valeur sur une région) : départage calcaire et argile.
    pub bruit: f32,
}

/// Distance (km) à une frontière de collision sous laquelle les montagnes sont du granite (cœur de
/// la chaîne), puis du grès (piémont, molasse).
pub const COLLISION_COEUR_KM: f32 = 350.0;
pub const COLLISION_PIEMONT_KM: f32 = 700.0;
/// Arc volcanique au-dessus d'une subduction, côté de la plaque qui reste en surface (km).
pub const ARC_KM: f32 = 350.0;
/// Basalte près d'un rift continental et près d'un point chaud (km) ; une petite terre (moins de
/// `ILE_CASES` cases d'un seul tenant) sur croûte océanique est une île volcanique. (Pas de règle
/// des dorsales : la croûte « océanique » de la génération porte aussi de grandes terres.)
pub const RIFT_KM: f32 = 150.0;
pub const ILE_CASES: usize = 60;
pub const POINT_CHAUD_KM: f32 = 250.0;
/// Au-delà, loin de toute frontière, une terre haute est un vieux socle (granite).
pub const BOUCLIER_KM: f32 = 1_500.0;
/// Pluie (mm/an) au-dessus de laquelle un calcaire devient karstique.
pub const KARST_PLUIE_MM: f32 = 800.0;

/// Roche de surface d'une case de terre (règles dans l'ordre : la première qui s'applique).
pub fn roche(l: &Lieu) -> Roche {
    let haut = matches!(l.relief, Relief::Rugged | Relief::Mountain | Relief::Peak);
    // 1) Alluvions : fleuves, deltas, marais, plaines basses le long des grandes rivières
    if l.fleuve_ou_delta || matches!(l.terrain, Terrain::Marsh | Terrain::Riverbank) || (l.grande_riviere && l.relief == Relief::Flat && l.altitude < 500.0) {
        return Roche::Alluvions;
    }
    // 2) Volcanisme : arcs des subductions, rifts et dorsales, points chauds, îles océaniques
    if let Some((b, km, dessus)) = l.frontiere {
        if matches!(b, Boundary::Subduction | Boundary::IslandArc) && km < ARC_KM && dessus {
            return Roche::Volcanique;
        }
        if b == Boundary::Rift && km < RIFT_KM {
            return Roche::Basalte;
        }
    }
    if l.point_chaud_km < POINT_CHAUD_KM || l.ile_oceanique {
        return Roche::Basalte;
    }
    // 3) Chaînes de collision : granite au cœur, grès sur les piémonts
    if let Some((Boundary::Collision, km, _)) = l.frontiere {
        if km < COLLISION_COEUR_KM && (haut || l.altitude > 1_500.0) {
            return Roche::Granite;
        }
        if km < COLLISION_PIEMONT_KM {
            return Roche::Gres;
        }
    }
    // 4) Lœss : steppes et prairies fraîches et semi-arides, pas en montagne
    let herbe = l.terrain == Terrain::Steppe || (matches!(l.terrain, Terrain::Grassland | Terrain::Plains) && l.temperature < 14.0);
    if herbe && (250.0..=700.0).contains(&l.pluie) && matches!(l.relief, Relief::Flat | Relief::Hills) && l.altitude < 1_500.0 {
        return Roche::Loess;
    }
    // 5) Vieilles montagnes et socles anciens loin des frontières : granite
    let loin = l.frontiere.is_none_or(|(_, km, _)| km > BOUCLIER_KM);
    if haut || (loin && (l.altitude > 300.0 || l.relief == Relief::Hills)) {
        return Roche::Granite;
    }
    // 6) Bassins sédimentaires : grès au sec, puis calcaire ou argile selon la région et le climat
    if l.pluie < 400.0 {
        Roche::Gres
    } else if l.bruit > 0.15 {
        Roche::Calcaire
    } else if l.bruit < -0.15 {
        Roche::Argile
    } else if l.temperature > 15.0 {
        Roche::Calcaire
    } else {
        Roche::Argile
    }
}

/// Le calcaire arrosé est karstique : grottes, gouffres, rivières qui se perdent (seulement
/// affiché ; l'hydrologie n'en tient pas compte).
pub fn karstique(r: Roche, pluie: f32) -> bool {
    r == Roche::Calcaire && pluie >= KARST_PLUIE_MM
}

/// Classe de sol d'une case de terre, d'après sa roche et son climat (la première règle qui
/// s'applique ; voir la conception, « Fertilité »).
pub fn sol(r: Roche, l: &Lieu) -> Sol {
    // Désert sans eau, toundra et neiges : pas de culture
    match l.terrain {
        Terrain::Desert => return Sol::Aride,
        Terrain::Tundra | Terrain::Snow => return Sol::Gele,
        _ => {}
    }
    if r == Roche::Alluvions {
        return Sol::Alluvial;
    }
    if matches!(r, Roche::Volcanique | Roche::Basalte) {
        return Sol::VolcaniqueJeune;
    }
    // Tropiques très arrosés, et vieux socles arrosés : sols lessivés
    let vieux_socle = r == Roche::Granite && l.frontiere.is_none_or(|(_, km, _)| km > BOUCLIER_KM);
    if l.terrain == Terrain::Jungle || (l.temperature > 20.0 && l.pluie > 1_500.0) || (vieux_socle && l.temperature > 18.0 && l.pluie > 1_200.0) {
        return Sol::LessiveTropical;
    }
    if r == Roche::Loess || (matches!(l.terrain, Terrain::Steppe | Terrain::Grassland) && (350.0..=700.0).contains(&l.pluie) && matches!(l.relief, Relief::Flat | Relief::Hills)) {
        return Sol::Noir;
    }
    if l.terrain == Terrain::Taiga || (l.temperature < 5.0 && l.pluie > 500.0) || (matches!(r, Roche::Granite | Roche::Gres) && l.pluie > 800.0) {
        return Sol::PauvreAcide;
    }
    if r == Roche::Argile && l.relief == Relief::Flat && l.saisons_seches >= 1 {
        return Sol::Lourd;
    }
    Sol::Brun
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plaine() -> Lieu {
        Lieu {
            ile_oceanique: false,
            frontiere: Some((Boundary::Transform, 900.0, true)),
            point_chaud_km: 5_000.0,
            altitude: 150.0,
            relief: Relief::Flat,
            temperature: 11.0,
            pluie: 750.0,
            saisons_seches: 0,
            terrain: Terrain::Grassland,
            fleuve_ou_delta: false,
            grande_riviere: false,
            bruit: 0.0,
        }
    }

    #[test]
    fn les_roches_suivent_les_plaques_l_eau_et_le_climat() {
        let p = plaine();
        // Plaine tempérée banale : argile (bassin humide et frais)
        assert_eq!(roche(&p), Roche::Argile);
        assert_eq!(roche(&Lieu { bruit: 0.5, ..p }), Roche::Calcaire);
        assert_eq!(roche(&Lieu { pluie: 300.0, terrain: Terrain::Desert, ..p }), Roche::Gres);
        // Fleuve, marais, plaine d'une grande rivière : alluvions
        assert_eq!(roche(&Lieu { fleuve_ou_delta: true, ..p }), Roche::Alluvions);
        assert_eq!(roche(&Lieu { terrain: Terrain::Marsh, ..p }), Roche::Alluvions);
        // Rives vertes du désert : alluvions et sol alluvial ; désert, toundra : pas de culture
        let rives = Lieu { pluie: 100.0, temperature: 24.0, terrain: Terrain::Riverbank, ..p };
        assert_eq!(roche(&rives), Roche::Alluvions);
        assert_eq!(sol(roche(&rives), &rives), Sol::Alluvial);
        let desert = Lieu { pluie: 100.0, temperature: 24.0, terrain: Terrain::Desert, ..p };
        assert_eq!(sol(roche(&desert), &desert), Sol::Aride);
        assert_eq!(Sol::Aride.effet_texte(), "pas de culture");
        let toundra = Lieu { temperature: -8.0, terrain: Terrain::Tundra, ..p };
        assert_eq!(sol(roche(&toundra), &toundra), Sol::Gele);
        assert_eq!(roche(&Lieu { grande_riviere: true, ..p }), Roche::Alluvions);
        // Subduction : arc volcanique du côté qui reste en surface seulement
        let sub = Some((Boundary::Subduction, 200.0, true));
        assert_eq!(roche(&Lieu { frontiere: sub, ..p }), Roche::Volcanique);
        assert_ne!(roche(&Lieu { frontiere: Some((Boundary::Subduction, 200.0, false)), ..p }), Roche::Volcanique);
        // Rift, point chaud, île océanique : basalte
        assert_eq!(roche(&Lieu { frontiere: Some((Boundary::Rift, 100.0, true)), ..p }), Roche::Basalte);
        assert_eq!(roche(&Lieu { point_chaud_km: 120.0, ..p }), Roche::Basalte);
        assert_eq!(roche(&Lieu { ile_oceanique: true, ..p }), Roche::Basalte);
        assert_ne!(roche(&Lieu { frontiere: Some((Boundary::Ridge, 100.0, true)), ..p }), Roche::Basalte, "dorsale : sous la mer");
        // Collision : granite au cœur de la chaîne, grès au piémont
        let col = |km| Some((Boundary::Collision, km, true));
        assert_eq!(roche(&Lieu { frontiere: col(200.0), relief: Relief::Mountain, altitude: 2_500.0, ..p }), Roche::Granite);
        assert_eq!(roche(&Lieu { frontiere: col(500.0), ..p }), Roche::Gres);
        // Steppe fraîche et semi-aride : lœss
        assert_eq!(roche(&Lieu { terrain: Terrain::Steppe, pluie: 450.0, ..p }), Roche::Loess);
        // Vieux socle loin des frontières : granite
        assert_eq!(roche(&Lieu { frontiere: Some((Boundary::Ridge, 2_500.0, true)), altitude: 600.0, ..p }), Roche::Granite);
        // Karst : calcaire arrosé seulement
        assert!(karstique(Roche::Calcaire, 1_000.0));
        assert!(!karstique(Roche::Calcaire, 500.0) && !karstique(Roche::Gres, 1_000.0));
    }

    #[test]
    fn les_sols_suivent_la_roche_et_le_climat() {
        let p = plaine();
        assert_eq!(sol(Roche::Alluvions, &p), Sol::Alluvial);
        assert_eq!(sol(Roche::Basalte, &p), Sol::VolcaniqueJeune);
        assert_eq!(sol(Roche::Loess, &p), Sol::Noir);
        assert_eq!(sol(Roche::Calcaire, &p), Sol::Brun);
        // Argile plate à saison sèche : sol lourd ; humide toute l'année : brun
        assert_eq!(sol(Roche::Argile, &Lieu { saisons_seches: 2, ..p }), Sol::Lourd);
        assert_eq!(sol(Roche::Argile, &p), Sol::Brun);
        // Granite ou grès arrosés, taïga : pauvres et acides
        assert_eq!(sol(Roche::Granite, &Lieu { pluie: 1_000.0, ..p }), Sol::PauvreAcide);
        assert_eq!(sol(Roche::Calcaire, &Lieu { terrain: Terrain::Taiga, ..p }), Sol::PauvreAcide);
        // Tropiques très arrosés : lessivés, sauf sur les volcans (rajeunis par les cendres)
        let tropique = Lieu { temperature: 26.0, pluie: 2_500.0, terrain: Terrain::Jungle, ..p };
        assert_eq!(sol(Roche::Argile, &tropique), Sol::LessiveTropical);
        assert_eq!(sol(Roche::Volcanique, &tropique), Sol::VolcaniqueJeune);
        // Fertilité : un tiers de l'écart pour les pâturages
        assert_eq!(Sol::Alluvial.fertilite(), 1.4);
        assert_eq!((Sol::Alluvial.fertilite_texte(), Sol::Lourd.fertilite_texte(), Sol::Brun.fertilite_texte()), ("1,4".to_string(), "0,85".to_string(), "1".to_string()));
        assert!((Sol::PauvreAcide.fertilite_paturage() - 0.9).abs() < 1e-9);
    }
}
