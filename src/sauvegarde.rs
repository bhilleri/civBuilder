//! Sauvegarde et chargement d'une partie (tâche 13).
//!
//! Le monde n'est pas enregistré : il se recalcule depuis sa graine et sa taille. La sauvegarde
//! garde seulement ce que la partie a changé : tour, trésors, hasard (`hasard.rs`), unités,
//! villes, camps et villages restants, cases explorées, territoire, ponts construits. Elle est
//! écrite en texte (format RON), lisible et versionnée.
//!
//! Une **empreinte** du monde généré accompagne la sauvegarde : si la génération a changé depuis
//! (nouvelle version du jeu), le chargement est refusé avec un message clair plutôt que de poser
//! des villes dans la mer.
//!
//! Charger relance le jeu (comme « Nouveau monde ») avec `CIVBUILDER_CHARGER=<fichier>` : le
//! monde est regénéré depuis la graine de la sauvegarde, puis la partie est remise en place.
//!
//! Limites : le journal n'est pas gardé ; les retouches de la carte faites à la console de debug
//! (terrain, relief…) non plus, seulement les ponts.

use crate::game::{City, Fog, GameLog, GameMap, GameState, HexPos, Ident, Importance, ProchainIdent, Territory, Unit};
use crate::hasard::Hasard;
use crate::hex::Hex;
use crate::hostile::{Camp, HostileHome, SiteName, Village};
use crate::map::{BridgeAt, WorldMap};
use crate::peuple::{Peuple, Tresors};
use crate::rules::{CityState, UnitState};
use crate::settlements::CampKind;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Version du format : à augmenter quand le contenu change (avec une conversion des anciennes).
pub const VERSION_FORMAT: u32 = 1;

/// Unité enregistrée.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UniteSauvee {
    pub ident: u32,
    pub peuple: Peuple,
    pub case: Hex,
    pub etat: UnitState,
    /// Base d'un guerrier hostile (autour de laquelle il erre).
    pub foyer: Option<Hex>,
}

/// Ville enregistrée.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VilleSauvee {
    pub ident: u32,
    pub peuple: Peuple,
    pub case: Hex,
    pub etat: CityState,
}

/// Camp hostile enregistré.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CampSauve {
    pub ident: u32,
    pub case: Hex,
    pub nom: String,
    pub delai: u32,
    pub genre: CampKind,
}

/// Village enregistré.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VillageSauve {
    pub ident: u32,
    pub case: Hex,
    pub nom: String,
}

/// Contenu d'une sauvegarde.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sauvegarde {
    pub version: u32,
    /// Graine et taille du monde (il se regénère à l'identique).
    pub graine: u32,
    pub largeur: i32,
    pub hauteur: i32,
    /// Empreinte du monde généré, pour refuser une sauvegarde d'une autre génération.
    pub empreinte: u64,
    pub tour: u32,
    pub tresors: Tresors,
    pub villes_fondees: usize,
    pub prochain_ident: u32,
    pub hasard: Hasard,
    pub unites: Vec<UniteSauvee>,
    pub villes: Vec<VilleSauvee>,
    pub camps: Vec<CampSauve>,
    pub villages: Vec<VillageSauve>,
    pub explorees: Vec<Hex>,
    /// Cases du territoire et identifiant de leur ville.
    pub territoire: Vec<(Hex, u32)>,
    pub ponts: Vec<BridgeAt>,
}

impl Sauvegarde {
    /// Texte de la sauvegarde (RON, indenté).
    pub fn texte(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|e| format!("écriture impossible : {e}"))
    }

    /// Lit une sauvegarde ; refuse une version du format inconnue.
    pub fn lire(texte: &str) -> Result<Self, String> {
        let s: Sauvegarde = ron::from_str(texte).map_err(|e| format!("sauvegarde illisible : {e}"))?;
        if s.version != VERSION_FORMAT {
            return Err(format!("sauvegarde au format {} (ce jeu lit le format {VERSION_FORMAT})", s.version));
        }
        Ok(s)
    }

    /// Le monde regénéré est-il bien celui de la sauvegarde ?
    pub fn verifier_monde(&self, map: &WorldMap) -> Result<(), String> {
        if (map.width, map.height) != (self.largeur, self.hauteur) {
            return Err(format!("taille du monde différente : {} × {} au lieu de {} × {}", map.width, map.height, self.largeur, self.hauteur));
        }
        if empreinte_monde(map) != self.empreinte {
            return Err("le monde généré par cette version du jeu n'est plus le même (la génération a changé depuis la sauvegarde)".to_string());
        }
        Ok(())
    }
}

/// Empreinte du monde généré : terrain, relief et niveau de chaque case (FNV-1a, stable d'une
/// exécution et d'une machine à l'autre).
pub fn empreinte_monde(map: &WorldMap) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut ajouter = |octets: &[u8]| {
        for &o in octets {
            h ^= o as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    ajouter(&map.width.to_le_bytes());
    ajouter(&map.height.to_le_bytes());
    for hex in map.hexes() {
        ajouter(format!("{:?}{:?}{:?}", map.get(hex), map.relief(hex), map.level(hex)).as_bytes());
    }
    h
}

/// Dossier des sauvegardes : `CIVBUILDER_SAUVEGARDES` s'il est donné, sinon `sauvegardes` à côté
/// du fichier des paramètres (`%APPDATA%\CivBuilder` sous Windows).
pub fn dossier() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CIVBUILDER_SAUVEGARDES") {
        return Some(PathBuf::from(d));
    }
    Some(crate::settings::settings_path()?.parent()?.join("sauvegardes"))
}

/// Fichier de la dernière sauvegarde (celle que « Charger » reprend).
pub fn derniere() -> Option<PathBuf> {
    Some(dossier()?.join("derniere.ron"))
}

// ---------------------------------------------------------------- Dans le jeu

/// Demande d'enregistrement (traitée au début de l'image suivante, quand tous les objets ont leur
/// identifiant) : `Some(fichier)`, ou `Some(None)` pour le fichier par défaut.
#[derive(Resource, Default)]
pub struct DemandeSauvegarde(pub Option<Option<PathBuf>>);

/// Partie lue au lancement (`CIVBUILDER_CHARGER`), à remettre en place après la création du monde.
#[derive(Resource, Default)]
pub struct ChargementEnAttente(pub Option<Sauvegarde>);

/// Lit la sauvegarde demandée au lancement (`CIVBUILDER_CHARGER`), s'il y en a une.
pub fn sauvegarde_au_lancement() -> Option<Result<Sauvegarde, String>> {
    let fichier = std::env::var_os("CIVBUILDER_CHARGER")?;
    Some(std::fs::read_to_string(&fichier).map_err(|e| format!("{} : {e}", Path::new(&fichier).display())).and_then(|t| Sauvegarde::lire(&t)))
}

pub struct SauvegardePlugin;

impl Plugin for SauvegardePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DemandeSauvegarde>()
            .add_systems(PostStartup, remettre_en_place)
            .add_systems(PreUpdate, enregistrer.after(crate::game::IdentSet));
    }
}

/// Construit la sauvegarde de la partie en cours.
#[allow(clippy::too_many_arguments)]
fn photographier(
    map: &WorldMap,
    state: &GameState,
    fog: &Fog,
    territory: &Territory,
    hasard: &Hasard,
    prochain: &ProchainIdent,
    units: &Query<(&Ident, &Peuple, &Unit, &HexPos, Option<&HostileHome>)>,
    cities: &Query<(Entity, &Ident, &Peuple, &City, &HexPos)>,
    camps: &Query<(&Ident, &Camp, &SiteName, &HexPos)>,
    villages: &Query<(&Ident, &SiteName, &HexPos), With<Village>>,
) -> Sauvegarde {
    let mut unites: Vec<UniteSauvee> = units
        .iter()
        .map(|(i, p, u, pos, foyer)| UniteSauvee { ident: i.0, peuple: *p, case: pos.0, etat: u.0.clone(), foyer: foyer.map(|f| f.0) })
        .collect();
    unites.sort_by_key(|u| u.ident);
    let mut villes: Vec<VilleSauvee> = cities.iter().map(|(_, i, p, c, pos)| VilleSauvee { ident: i.0, peuple: *p, case: pos.0, etat: c.state.clone() }).collect();
    villes.sort_by_key(|v| v.ident);
    let mut camps_s: Vec<CampSauve> =
        camps.iter().map(|(i, c, n, pos)| CampSauve { ident: i.0, case: pos.0, nom: n.0.clone(), delai: c.cooldown, genre: c.kind }).collect();
    camps_s.sort_by_key(|c| c.ident);
    let mut villages_s: Vec<VillageSauve> = villages.iter().map(|(i, n, pos)| VillageSauve { ident: i.0, case: pos.0, nom: n.0.clone() }).collect();
    villages_s.sort_by_key(|v| v.ident);
    let ident_ville = |e: Entity| cities.get(e).ok().map(|(_, i, ..)| i.0);
    let mut territoire: Vec<(Hex, u32)> = territory.0.iter().filter_map(|(h, e)| Some((*h, ident_ville(*e)?))).collect();
    territoire.sort();
    let mut ponts: Vec<BridgeAt> = map.bridges().collect();
    ponts.sort_by_key(|b| format!("{b:?}"));
    Sauvegarde {
        version: VERSION_FORMAT,
        graine: map.seed(),
        largeur: map.width,
        hauteur: map.height,
        empreinte: empreinte_monde(map),
        tour: state.turn,
        tresors: state.tresors.clone(),
        villes_fondees: state.founded_cities,
        prochain_ident: prochain.0,
        hasard: hasard.clone(),
        unites,
        villes,
        camps: camps_s,
        villages: villages_s,
        explorees: fog.explored.iter().collect(),
        territoire,
        ponts,
    }
}

/// Enregistre la partie quand on le demande (menu, console).
#[allow(clippy::too_many_arguments)]
fn enregistrer(
    mut demande: ResMut<DemandeSauvegarde>,
    map: Res<GameMap>,
    (state, fog, territory, hasard, prochain): (Res<GameState>, Res<Fog>, Res<Territory>, Res<Hasard>, Res<ProchainIdent>),
    mut log: ResMut<GameLog>,
    units: Query<(&Ident, &Peuple, &Unit, &HexPos, Option<&HostileHome>)>,
    cities: Query<(Entity, &Ident, &Peuple, &City, &HexPos)>,
    camps: Query<(&Ident, &Camp, &SiteName, &HexPos)>,
    villages: Query<(&Ident, &SiteName, &HexPos), With<Village>>,
) {
    let Some(fichier) = demande.0.take() else { return };
    let s = photographier(&map.0, &state, &fog, &territory, &hasard, &prochain, &units, &cities, &camps, &villages);
    match ecrire(&s, fichier) {
        Ok(chemin) => {
            info!("Partie enregistrée : {}", chemin.display());
            log.notify(Importance::Success, format!("Partie enregistrée (tour {}).", s.tour));
        }
        Err(e) => {
            warn!("Sauvegarde impossible : {e}");
            log.notify(Importance::Warning, format!("Sauvegarde impossible : {e}"));
        }
    }
}

/// Écrit la sauvegarde dans `fichier`, ou par défaut sous un nom daté par la graine et le tour ;
/// elle devient aussi la dernière sauvegarde (`derniere.ron`). Retourne le fichier écrit.
pub fn ecrire(s: &Sauvegarde, fichier: Option<PathBuf>) -> Result<PathBuf, String> {
    let texte = s.texte()?;
    let chemin = match fichier {
        Some(f) => f,
        None => dossier().ok_or("pas de dossier de sauvegarde")?.join(format!("partie-{}-tour-{}.ron", s.graine, s.tour)),
    };
    if let Some(parent) = chemin.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
    }
    std::fs::write(&chemin, &texte).map_err(|e| format!("{} : {e}", chemin.display()))?;
    if let Some(d) = derniere().filter(|d| *d != chemin) {
        if let Some(parent) = d.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&d, &texte).map_err(|e| format!("{} : {e}", d.display()))?;
    }
    Ok(chemin)
}

/// Relance le jeu sur une sauvegarde (le monde est regénéré depuis sa graine).
pub fn relancer_sur(fichier: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(exe);
    command.args(std::env::args_os().skip(1)).env("CIVBUILDER_CHARGER", fichier).env_remove("CIVBUILDER_SEED").env_remove("CIVBUILDER_MAP");
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Remet en place la partie lue au lancement : retire les unités, villes et sites de départ,
/// puis recrée ceux de la sauvegarde.
#[allow(clippy::too_many_arguments)]
fn remettre_en_place(
    mut commands: Commands,
    mut attente: ResMut<ChargementEnAttente>,
    mut map: ResMut<GameMap>,
    (mut state, mut fog, mut territory, mut hasard, mut prochain): (ResMut<GameState>, ResMut<Fog>, ResMut<Territory>, ResMut<Hasard>, ResMut<ProchainIdent>),
    mut log: ResMut<GameLog>,
    mut selection: ResMut<crate::game::Selection>,
    mut edited: MessageWriter<crate::console::MapEdited>,
    existants: Query<Entity, Or<(With<Unit>, With<City>, With<Camp>, With<Village>)>>,
) {
    let Some(s) = attente.0.take() else { return };
    if let Err(e) = s.verifier_monde(&map.0) {
        error!("Chargement refusé : {e}");
        log.notify(Importance::Warning, format!("Chargement refusé : {e}. Nouvelle partie sur ce monde."));
        return;
    }
    for e in &existants {
        commands.entity(e).despawn();
    }
    selection.0 = None;
    state.turn = s.tour;
    state.tresors = s.tresors.clone();
    state.founded_cities = s.villes_fondees;
    *hasard = s.hasard.clone();
    prochain.0 = s.prochain_ident;
    fog.explored = s.explorees.iter().copied().collect();
    fog.visible = Default::default();
    for u in &s.unites {
        let mut e = commands.spawn((Unit(u.etat.clone()), HexPos(u.case), Ident(u.ident), u.peuple));
        if let Some(foyer) = u.foyer {
            e.insert((crate::hostile::Hostile, HostileHome(foyer)));
        }
    }
    let mut villes = std::collections::HashMap::new();
    for v in &s.villes {
        let e = commands.spawn((City { state: v.etat.clone(), worked: Vec::new() }, HexPos(v.case), Ident(v.ident), v.peuple)).id();
        villes.insert(v.ident, e);
    }
    territory.0.clear();
    for (h, ident) in &s.territoire {
        if let Some(&e) = villes.get(ident) {
            territory.0.insert(*h, e);
        }
    }
    for c in &s.camps {
        commands.spawn((Camp { cooldown: c.delai, kind: c.genre }, SiteName(c.nom.clone()), HexPos(c.case), Ident(c.ident)));
    }
    for v in &s.villages {
        commands.spawn((Village, SiteName(v.nom.clone()), HexPos(v.case), Ident(v.ident)));
    }
    let mut refaire = Vec::new();
    for b in &s.ponts {
        if map.0.set_bridge(*b, true) {
            let (a, c) = b.ends(map.0.wrap());
            refaire.extend([a, c]);
        }
    }
    if !refaire.is_empty() {
        edited.write(crate::console::MapEdited(refaire));
    }
    info!("Partie chargée : graine {}, tour {}, {} unités, {} villes", s.graine, s.tour, s.unites.len(), s.villes.len());
    log.notify(Importance::Success, format!("Partie chargée (tour {}).", s.tour));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;
    use crate::units::UnitKind;

    fn exemple() -> Sauvegarde {
        let mut tresors = Tresors::default();
        *tresors.get_mut(Peuple::JOUEUR) = 42;
        let mut hasard = Hasard::new(777);
        hasard.tirage(crate::hasard::Flux::Combat, Peuple::JOUEUR);
        let mut etat = UnitState::new(UnitKind::Militia);
        etat.hp = 63;
        etat.path = vec![Hex::from_offset(3, 3), Hex::from_offset(4, 3)];
        Sauvegarde {
            version: VERSION_FORMAT,
            graine: 777,
            largeur: 20,
            hauteur: 10,
            empreinte: 12345,
            tour: 9,
            tresors,
            villes_fondees: 1,
            prochain_ident: 7,
            hasard,
            unites: vec![UniteSauvee { ident: 2, peuple: Peuple::JOUEUR, case: Hex::from_offset(2, 3), etat, foyer: None }],
            villes: vec![VilleSauvee { ident: 0, peuple: Peuple::JOUEUR, case: Hex::from_offset(5, 5), etat: CityState::new("Kaldra".into()) }],
            camps: vec![CampSauve { ident: 3, case: Hex::from_offset(9, 1), nom: "Repaire des Loups".into(), delai: 2, genre: CampKind::Encampment }],
            villages: vec![VillageSauve { ident: 4, case: Hex::from_offset(1, 8), nom: "Kaldvik".into() }],
            explorees: vec![Hex::from_offset(2, 3), Hex::from_offset(5, 5)],
            territoire: vec![(Hex::from_offset(5, 5), 0)],
            ponts: vec![BridgeAt::Edge { hex: Hex::from_offset(4, 4), dir: 1 }],
        }
    }

    #[test]
    fn aller_retour_en_texte() {
        let s = exemple();
        let texte = s.texte().unwrap();
        let relue = Sauvegarde::lire(&texte).unwrap();
        // Le texte relu redonne exactement le même texte
        assert_eq!(relue.texte().unwrap(), texte);
        assert_eq!(relue.unites[0].etat.hp, 63);
        assert_eq!(relue.unites[0].etat.path.len(), 2);
        assert_eq!(relue.tresors.get(Peuple::JOUEUR), 42);
        assert_eq!(relue.hasard, s.hasard);
        assert_eq!(relue.villes[0].etat.name, "Kaldra");
    }

    #[test]
    fn format_inconnu_refuse() {
        let mut s = exemple();
        s.version = VERSION_FORMAT + 1;
        let err = Sauvegarde::lire(&s.texte().unwrap()).unwrap_err();
        assert!(err.contains("format"), "{err}");
        assert!(Sauvegarde::lire("pas une sauvegarde").is_err());
    }

    #[test]
    fn empreinte_et_verification_du_monde() {
        let a = WorldMap::filled(20, 10, Terrain::Plains);
        let mut b = WorldMap::filled(20, 10, Terrain::Plains);
        assert_eq!(empreinte_monde(&a), empreinte_monde(&b));
        b.set(Hex::from_offset(3, 3), Terrain::Forest);
        assert_ne!(empreinte_monde(&a), empreinte_monde(&b));
        let mut s = exemple();
        s.empreinte = empreinte_monde(&a);
        assert!(s.verifier_monde(&a).is_ok());
        assert!(s.verifier_monde(&b).is_err());
        assert!(s.verifier_monde(&WorldMap::filled(21, 10, Terrain::Plains)).is_err());
    }
}
