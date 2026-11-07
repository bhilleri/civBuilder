//! Type des embouchures de fleuve (logique pure, testée).
//!
//! Un fleuve apporte des sédiments ; la houle et la marée les reprennent. La forme de l'embouchure
//! dépend de celui des trois qui l'emporte (diagramme de Galloway), mesuré comme Nienhuis et al.
//! (Nature, 2020 ; code `GlobalDeltaChange`) par trois flux de sédiments en kg/s. S'y ajoutent les
//! cas sans place pour un delta : mer profonde tout de suite, vallée noyée, vallée glaciaire.
//! Voir `docs/conception/conception-hydrologie.md`.

use std::f32::consts::TAU;

const G: f32 = 9.81;
/// Pulsation de la marée semi-diurne (rad/s).
const TIDE_OMEGA: f32 = 1.4e-4;
/// Concentration de sédiments (kg/m³) d'une eau sans limon et d'une eau au limon le plus fort : sur
/// Terre, en moyenne 0,4 kg/m³, le Mississippi environ 0,7, les fleuves du nord 0,02.
const CLEAR_WATER_KG_M3: f32 = 0.02;
const MUDDY_WATER_KG_M3: f32 = 2.0;
/// Bornes de la concentration appliquée au débit de marée (Nienhuis : de 10 mg/L à 1 000 mg/L).
const TIDE_CONCENTRATION_RANGE: (f32, f32) = (0.01, 1.0);
/// Pente du chenal au bas d'un fleuve : celle des cases de 40 km n'a pas de sens ici (1 à 3 m/km,
/// contre 0,02 à 0,1 m/km sur Terre). Estimée selon le débit, plus faible pour un grand fleuve.
const CHANNEL_SLOPE_AT_3000: f32 = 6e-5;
const CHANNEL_SLOPE_RANGE: (f32, f32) = (2e-5, 2e-4);
/// La marée de la case de 40 km devant l'embouchure est une moyenne : un chenal qui se resserre
/// l'amplifie dans ses derniers kilomètres, ce que la grille ne résout pas.
const ESTUARY_AMPLIFICATION: f32 = 2.0;
/// Transport de sable par la houle (`GlobalDeltaChange/get_Qwave.m`) : sable de masse volumique
/// 2 650 kg/m³, porosité 0,4 ; tri des fins multiplié par 3.
const WAVE_FINES: f32 = 3.0;
/// Fond (m) sous lequel la mer laisse la place à un delta, juste devant et à trois cases.
const SHALLOW_FRONT_M: f32 = 200.0;
const SHALLOW_FAR_M: f32 = 500.0;
/// Relief (m) proche qui fait une vallée noyée, et fjord : froid (°C) et fond d'une vallée glaciaire (m).
const RIA_RELIEF_M: f32 = 1_200.0;
const FJORD_RELIEF_M: f32 = 800.0;
const FJORD_MAX_C: f32 = 5.0;
const FJORD_MIN_DEPTH_M: f32 = 100.0;
/// Sous cette part du flux de houle, le fleuve n'a pas de quoi bâtir un delta face à la houle : le
/// sable forme un cordon (lagune) sur une côte basse.
const LAGOON_BELOW_RATIO: f32 = 0.12;
/// Au-dessous, relief assez bas pour une lagune (m).
const LAGOON_RELIEF_M: f32 = 500.0;
/// Concentration (kg/m³) à partir de laquelle la marée construit un delta au lieu d'un estuaire.
const TIDE_DELTA_MIN_KG_M3: f32 = 0.15;

/// Forme d'une embouchure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouthKind {
    /// Le fleuve domine : bras qui s'avancent en mer (Mississippi).
    BirdFoot,
    /// La houle domine : delta bombé bordé de plages (Nil).
    Arcuate,
    /// La marée domine, eau chargée : entonnoir et bancs allongés (Gange).
    TidalDelta,
    /// Peu de sédiments ou mer profonde : la vallée s'élargit vers la mer (Gironde, Congo).
    Estuary,
    /// Forte houle, peu de sédiments, côte basse : cordon de sable et étang (Languedoc).
    Lagoon,
    /// Vallée de relief noyée par la mer (Galice).
    Ria,
    /// Vallée glaciaire profonde (Norvège).
    Fjord,
}

impl MouthKind {
    #[cfg(test)]
    pub const ALL: [MouthKind; 7] = [MouthKind::BirdFoot, MouthKind::Arcuate, MouthKind::TidalDelta, MouthKind::Estuary, MouthKind::Lagoon, MouthKind::Ria, MouthKind::Fjord];

    /// Nom affiché.
    pub fn name(self) -> &'static str {
        match self {
            MouthKind::BirdFoot => "delta digité",
            MouthKind::Arcuate => "delta arqué",
            MouthKind::TidalDelta => "delta de marée",
            MouthKind::Estuary => "estuaire",
            MouthKind::Lagoon => "lagune",
            MouthKind::Ria => "ria",
            MouthKind::Fjord => "fjord",
        }
    }
}

/// Ce que le monde dit d'une embouchure.
#[derive(Clone, Copy, Debug)]
pub struct MouthInput {
    /// Débit (m³/s) et limon (0 à 1) du fleuve.
    pub discharge: f32,
    pub murk: f32,
    /// Marnage de vives-eaux (m) devant l'embouchure.
    pub tidal_range: f32,
    /// Hauteur (m) et période (s) de la houle qui arrive vers le rivage.
    pub wave_height: f32,
    pub wave_period: f32,
    /// Profondeur (m) devant l'embouchure et trois cases plus loin.
    pub front_depth: f32,
    pub far_depth: f32,
    /// Altitude la plus haute (m) à deux cases.
    pub relief: f32,
    /// Température moyenne (°C).
    pub temperature: f32,
}

/// Une embouchure : sa forme et les trois flux de sédiments (kg/s).
#[derive(Clone, Copy, Debug)]
pub struct Mouth {
    pub kind: MouthKind,
    pub river: f32,
    pub wave: f32,
    pub tide: f32,
}

/// Concentration de sédiments d'une eau (kg/m³) selon son limon.
fn concentration(murk: f32) -> f32 {
    CLEAR_WATER_KG_M3 * (MUDDY_WATER_KG_M3 / CLEAR_WATER_KG_M3).powf(murk.clamp(0.0, 1.0))
}

/// Flux de sédiments apporté par le fleuve (kg/s).
fn river_flux(discharge: f32, murk: f32) -> f32 {
    discharge * concentration(murk)
}

/// Flux de sable que la houle peut déplacer (kg/s) : formule de transport le long du rivage, à
/// l'angle où elle est la plus forte (`get_Qwave.m` : `2650 (1−0,4) k T^0,2 Hs^2,4 cos^1,2 θ sin θ`,
/// écart entre les deux sens).
fn wave_flux(height: f32, period: f32) -> f32 {
    let k = 5.3e-6 * 1_050.0 * G.powf(1.5) * 0.5f32.powf(1.2) * ((G * 0.78).sqrt() / TAU).powf(0.2);
    // (maximum de cos^1,2 θ sin θ, à 42,4°, dans les deux sens)
    let angle = 2.0 * 0.468;
    WAVE_FINES * 2_650.0 * (1.0 - 0.4) * k * angle * period.max(1.0).powf(0.2) * height.max(0.0).powf(2.4)
}

/// Flux de sédiments mis en mouvement par la marée (kg/s), `get_Qtide.m` : débit de la marée dans
/// un chenal qui s'évase vers la mer, multiplié par la concentration de l'eau.
fn tide_flux(discharge: f32, murk: f32, tidal_range: f32) -> f32 {
    // (amplitude de l'onde semi-diurne seule : le marnage de vives-eaux compte aussi l'onde solaire)
    let amplitude = (ESTUARY_AMPLIFICATION * tidal_range / (2.0 * (1.0 + crate::tides::S2_RATIO))).max(1e-3);
    let q = discharge.max(1.0);
    let (width, depth) = (6.5 * q.sqrt(), 0.586 * q.powf(0.33));
    let beta = width / depth;
    let slope = (CHANNEL_SLOPE_AT_3000 * (3_000.0 / q).sqrt()).clamp(CHANNEL_SLOPE_RANGE.0, CHANNEL_SLOPE_RANGE.1);
    let k = TIDE_OMEGA / (std::f32::consts::PI * (0.2f32 * 1e-4).sqrt() * 1.65 * 55.0);
    let length = 0.5 * (2.0 * amplitude + depth) / 2.0 / slope;
    let tide_discharge = 0.5 * TIDE_OMEGA * k * amplitude * amplitude * length * length * beta * (1.0 + 2.0 * slope / (k * amplitude));
    tide_discharge * concentration(murk).clamp(TIDE_CONCENTRATION_RANGE.0, TIDE_CONCENTRATION_RANGE.1)
}

/// Forme d'une embouchure selon le monde autour d'elle.
pub fn classify(m: &MouthInput) -> Mouth {
    let (river, wave, tide) = (river_flux(m.discharge, m.murk), wave_flux(m.wave_height, m.wave_period), tide_flux(m.discharge, m.murk, m.tidal_range));
    let shallow = m.front_depth < SHALLOW_FRONT_M && m.far_depth < SHALLOW_FAR_M;
    let kind = if m.temperature < FJORD_MAX_C && m.relief >= FJORD_RELIEF_M && m.front_depth >= FJORD_MIN_DEPTH_M {
        MouthKind::Fjord
    } else if m.relief >= RIA_RELIEF_M {
        MouthKind::Ria
    } else if !shallow {
        MouthKind::Estuary
    } else if tide >= river && tide >= wave {
        if concentration(m.murk) >= TIDE_DELTA_MIN_KG_M3 { MouthKind::TidalDelta } else { MouthKind::Estuary }
    } else if river >= wave {
        MouthKind::BirdFoot
    } else if river < wave * LAGOON_BELOW_RATIO {
        if m.relief < LAGOON_RELIEF_M { MouthKind::Lagoon } else { MouthKind::Estuary }
    } else {
        MouthKind::Arcuate
    };
    Mouth { kind, river, wave, tide }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fleuve moyen sur un plateau, mer calme, peu de marée, relief bas, climat doux.
    fn base() -> MouthInput {
        MouthInput { discharge: 5_000.0, murk: 0.6, tidal_range: 0.5, wave_height: 0.3, wave_period: 8.0, front_depth: 30.0, far_depth: 80.0, relief: 200.0, temperature: 18.0 }
    }

    #[test]
    fn ordres_de_grandeur_des_flux() {
        // Mississippi : environ 18 000 m³/s et 12 000 kg/s de sédiments avant les barrages
        let q = river_flux(18_000.0, 0.72);
        assert!((q - 12_000.0).abs() < 6_000.0, "fleuve {q}");
        // Houle de 1,5 m et 8 s : de l'ordre du millier de kg/s ; 10 fois moins à 0,6 m
        let w = wave_flux(1.5, 8.0);
        assert!((400.0..4_000.0).contains(&w), "houle {w}");
        assert!(wave_flux(0.6, 8.0) < w / 5.0);
        // La marée croît très vite avec le marnage
        assert!(tide_flux(5_000.0, 0.6, 4.0) > 5.0 * tide_flux(5_000.0, 0.6, 1.0));
    }

    #[test]
    fn le_fleuve_domine_en_mer_calme() {
        assert_eq!(classify(&base()).kind, MouthKind::BirdFoot);
    }

    #[test]
    fn la_houle_domine_sur_une_cote_exposee() {
        let m = MouthInput { wave_height: 2.5, wave_period: 11.0, discharge: 8_000.0, ..base() };
        assert_eq!(classify(&m).kind, MouthKind::Arcuate);
    }

    #[test]
    fn peu_de_sediments_face_a_la_houle_donne_une_lagune_ou_un_estuaire() {
        let weak = MouthInput { wave_height: 3.0, wave_period: 11.0, discharge: 2_000.0, murk: 0.2, ..base() };
        assert_eq!(classify(&weak).kind, MouthKind::Lagoon);
        assert_eq!(classify(&MouthInput { relief: 800.0, ..weak }).kind, MouthKind::Estuary);
    }

    #[test]
    fn la_maree_domine_a_fort_marnage_en_mer_calme() {
        let muddy = MouthInput { tidal_range: 8.0, wave_height: 0.3, discharge: 2_000.0, murk: 0.7, ..base() };
        assert_eq!(classify(&muddy).kind, MouthKind::TidalDelta);
        let clear = MouthInput { murk: 0.2, ..muddy };
        assert_eq!(classify(&clear).kind, MouthKind::Estuary);
    }

    #[test]
    fn mer_profonde_estuaire_meme_avec_beaucoup_de_sediments() {
        let m = MouthInput { front_depth: 1_500.0, far_depth: 3_000.0, discharge: 30_000.0, ..base() };
        assert_eq!(classify(&m).kind, MouthKind::Estuary);
    }

    #[test]
    fn vallees_noyees() {
        let ria = MouthInput { relief: 1_800.0, wave_height: 2.0, wave_period: 10.0, murk: 0.2, discharge: 2_500.0, ..base() };
        assert_eq!(classify(&ria).kind, MouthKind::Ria);
        // Même un fleuve très chargé : pas de delta au pied d'un massif, il n'a pas de plaine
        assert_eq!(classify(&MouthInput { murk: 0.9, discharge: 20_000.0, wave_height: 0.3, ..ria }).kind, MouthKind::Ria);
        let fjord = MouthInput { temperature: 1.0, relief: 1_000.0, front_depth: 300.0, far_depth: 800.0, ..base() };
        assert_eq!(classify(&fjord).kind, MouthKind::Fjord);
    }
}
