//! Configuration d'une partie et bibliothèque des configurations (logique pure, tâche 42).
//!
//! Une configuration réunit le monde (graine, taille, terres, climat, relief) et les règles
//! (peuples d'IA, difficulté, rythme, barbares, hégémonie, premiers tours guidés). Le monde tient
//! dans un **code** court et copiable (`777-M-T29-C0-R0`) : deux joueurs qui partagent un code
//! jouent la même planète. Les configurations enregistrées (nom, commentaire, favorite) et celles
//! des parties jouées forment la **bibliothèque** (`configurations.ron`, à côté des sauvegardes).
//!
//! Certains réglages ne sont pas encore pris en compte par le jeu (`disponible()` : terres, climat,
//! relief, IA, difficulté, rythme, hégémonie, premiers tours guidés) ; l'écran les montre grisés.
//! Voir `docs/conception/conception-interface.md`, « Nouvelle partie, graines et prise en main ».

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Taille de la carte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Taille {
    Petite,
    #[default]
    Moyenne,
    Grande,
}

impl Taille {
    pub const TOUTES: [Taille; 3] = [Taille::Petite, Taille::Moyenne, Taille::Grande];

    /// Largeur et hauteur en cases.
    pub fn cases(self) -> (i32, i32) {
        match self {
            Taille::Petite => (128, 74),
            Taille::Moyenne => (192, 111),
            Taille::Grande => (256, 148),
        }
    }

    /// Taille correspondant à des dimensions (`None` si elles ne sont pas l'une des trois).
    pub fn depuis_cases(cases: (i32, i32)) -> Option<Taille> {
        Taille::TOUTES.into_iter().find(|t| t.cases() == cases)
    }

    /// Lettre de la taille dans le code du monde.
    pub fn lettre(self) -> char {
        match self {
            Taille::Petite => 'P',
            Taille::Moyenne => 'M',
            Taille::Grande => 'G',
        }
    }

    /// Nom affiché.
    pub fn nom(self) -> &'static str {
        match self {
            Taille::Petite => "Petite",
            Taille::Moyenne => "Moyenne",
            Taille::Grande => "Grande",
        }
    }

    /// Durée de génération approchée, en secondes.
    pub fn duree_generation(self) -> u32 {
        match self {
            Taille::Petite => 15,
            Taille::Moyenne => 30,
            Taille::Grande => 60,
        }
    }

    /// Taille pas encore vérifiée par les bilans de génération.
    pub fn experimentale(self) -> bool {
        self == Taille::Grande
    }

    /// Nombre de peuples d'IA conseillé et maximal pour cette taille.
    pub fn peuples_ia(self) -> (u32, u32) {
        match self {
            Taille::Petite => (4, 5),
            Taille::Moyenne => (7, 9),
            Taille::Grande => (10, 12),
        }
    }
}

/// Réglage de génération à trois crans autour de la Terre (terres, climat, relief).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cran {
    Moins,
    #[default]
    CommeLaTerre,
    Plus,
}

impl Cran {
    pub const TOUS: [Cran; 3] = [Cran::Moins, Cran::CommeLaTerre, Cran::Plus];

    /// Valeur dans le code : -1, 0 ou +1.
    fn valeur(self) -> i32 {
        match self {
            Cran::Moins => -1,
            Cran::CommeLaTerre => 0,
            Cran::Plus => 1,
        }
    }

    fn depuis_valeur(v: i32) -> Option<Cran> {
        Cran::TOUS.into_iter().find(|c| c.valeur() == v)
    }
}

/// Part des terres émergées visée, en % (dans le code du monde).
pub fn part_des_terres(cran: Cran) -> u32 {
    match cran {
        Cran::Moins => 20,
        Cran::CommeLaTerre => 29,
        Cran::Plus => 45,
    }
}

/// Difficulté des IA.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulte {
    Decouverte,
    #[default]
    Normal,
    Difficile,
    Expert,
}

impl Difficulte {
    pub const TOUTES: [Difficulte; 4] = [Difficulte::Decouverte, Difficulte::Normal, Difficulte::Difficile, Difficulte::Expert];

    /// Nom affiché.
    pub fn nom(self) -> &'static str {
        match self {
            Difficulte::Decouverte => "Découverte",
            Difficulte::Normal => "Normal",
            Difficulte::Difficile => "Difficile",
            Difficulte::Expert => "Expert",
        }
    }
}

/// Rythme : tours par mois (500 tours dans les deux cas, `conception-saisons.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rythme {
    #[default]
    UnTourParMois,
    DeuxToursParMois,
}

/// Nombre de barbares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Barbares {
    Rares,
    #[default]
    Normaux,
    Nombreux,
}

impl Barbares {
    pub const TOUS: [Barbares; 3] = [Barbares::Rares, Barbares::Normaux, Barbares::Nombreux];

    /// Cases de terre pour un camp hostile.
    pub fn terres_par_camp(self) -> usize {
        match self {
            Barbares::Rares => 320,
            Barbares::Normaux => 160,
            Barbares::Nombreux => 80,
        }
    }

    /// Nom affiché.
    pub fn nom(self) -> &'static str {
        match self {
            Barbares::Rares => "Rares",
            Barbares::Normaux => "Normaux",
            Barbares::Nombreux => "Nombreux",
        }
    }
}

/// Configuration d'une partie.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigPartie {
    pub graine: u32,
    pub taille: Taille,
    pub terres: Cran,
    pub climat: Cran,
    pub relief: Cran,
    pub peuples_ia: u32,
    pub difficulte: Difficulte,
    pub rythme: Rythme,
    pub barbares: Barbares,
    pub hegemonie: bool,
    pub tours_guides: bool,
}

impl Default for ConfigPartie {
    fn default() -> Self {
        Self {
            graine: 777,
            taille: Taille::Moyenne,
            terres: Cran::CommeLaTerre,
            climat: Cran::CommeLaTerre,
            relief: Cran::CommeLaTerre,
            peuples_ia: Taille::Moyenne.peuples_ia().0,
            difficulte: Difficulte::Normal,
            rythme: Rythme::UnTourParMois,
            barbares: Barbares::Normaux,
            hegemonie: true,
            tours_guides: true,
        }
    }
}

/// Réglages déjà pris en compte par le jeu (les autres sont « à venir »).
pub fn disponible(reglage: &str) -> bool {
    matches!(reglage, "graine" | "taille" | "barbares")
}

impl ConfigPartie {
    /// Configuration par défaut avec cette graine.
    pub fn avec_graine(graine: u32) -> Self {
        Self { graine, ..Default::default() }
    }

    /// Code du monde : graine, taille, part des terres, climat, relief (`777-M-T29-C0-R0`).
    pub fn code(&self) -> String {
        let signe = |c: Cran| match c.valeur() {
            v if v > 0 => format!("+{v}"),
            v => v.to_string(),
        };
        format!("{}-{}-T{}-C{}-R{}", self.graine, self.taille.lettre(), part_des_terres(self.terres), signe(self.climat), signe(self.relief))
    }

    /// Applique un code du monde (les règles ne changent pas). Un message dit ce qui ne va pas.
    pub fn appliquer_code(&mut self, code: &str) -> Result<(), String> {
        let morceaux: Vec<&str> = code.trim().split('-').filter(|m| !m.is_empty()).collect();
        // (le signe moins du climat ou du relief coupe le texte : on recolle « C » et « 1 »)
        let mut parts: Vec<String> = Vec::new();
        for m in morceaux {
            match parts.last_mut() {
                Some(dernier) if (dernier == "C" || dernier == "R") && m.chars().all(|c| c.is_ascii_digit()) => dernier.push_str(&format!("-{m}")),
                _ => parts.push(m.to_string()),
            }
        }
        let graine: u32 = parts.first().and_then(|g| g.parse().ok()).ok_or_else(|| format!("code illisible : la graine « {} » n'est pas un nombre", parts.first().map_or("", |s| s.as_str())))?;
        let mut nouveau = Self { graine, ..self.clone() };
        for p in parts.iter().skip(1) {
            let (lettre, reste) = p.split_at(1);
            match lettre {
                "P" | "M" | "G" if reste.is_empty() => {
                    nouveau.taille = Taille::TOUTES.into_iter().find(|t| t.lettre().to_string() == lettre).unwrap_or_default();
                }
                "T" => {
                    let v: u32 = reste.parse().map_err(|_| format!("code illisible : « {p} »"))?;
                    nouveau.terres = Cran::TOUS.into_iter().find(|c| part_des_terres(*c) == v).ok_or_else(|| format!("code illisible : {v} % de terres n'est pas un réglage connu"))?;
                }
                "C" | "R" => {
                    let v: i32 = reste.trim_start_matches('+').parse().map_err(|_| format!("code illisible : « {p} »"))?;
                    let cran = Cran::depuis_valeur(v).ok_or_else(|| format!("code illisible : « {p} »"))?;
                    if lettre == "C" { nouveau.climat = cran } else { nouveau.relief = cran }
                }
                _ => return Err(format!("code illisible : « {p} » inconnu")),
            }
        }
        *self = nouveau;
        Ok(())
    }

    /// Résumé d'une ligne (« 777 · Moyenne · barbares normaux »).
    pub fn resume(&self) -> String {
        format!("{} · {} · barbares {}", self.graine, self.taille.nom(), self.barbares.nom().to_lowercase())
    }
}

/// D'où vient une configuration de la bibliothèque.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origine {
    /// Enregistrée par le joueur.
    Enregistree,
    /// Celle d'une partie jouée (tour atteint à la dernière sauvegarde).
    Jouee { tour: u32 },
}

/// Configuration de la bibliothèque.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entree {
    pub nom: String,
    pub commentaire: String,
    pub favorite: bool,
    pub config: ConfigPartie,
    pub origine: Origine,
    /// Parties lancées avec cette configuration.
    pub parties: u32,
    /// Date (AAAA-MM-JJ) de la dernière utilisation.
    pub date: String,
}

/// Bibliothèque des configurations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bibliotheque {
    pub entrees: Vec<Entree>,
}

/// Nombre maximal de parties jouées gardées (les plus anciennes non favorites s'effacent).
const PARTIES_GARDEES: usize = 30;

impl Bibliotheque {
    /// Fichier de la bibliothèque (à côté des sauvegardes).
    pub fn fichier() -> Option<PathBuf> {
        Some(crate::sauvegarde::dossier()?.parent()?.join("configurations.ron"))
    }

    /// Lit la bibliothèque (vide si le fichier manque ou est illisible).
    pub fn lire() -> Self {
        Self::fichier().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| ron::from_str(&t).ok()).unwrap_or_default()
    }

    /// Écrit la bibliothèque.
    pub fn ecrire(&self) -> Result<(), String> {
        let f = Self::fichier().ok_or("pas de dossier pour les configurations")?;
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let texte = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
        std::fs::write(&f, texte).map_err(|e| format!("{} : {e}", f.display()))
    }

    /// Enregistre une configuration nommée (remplace celle du même nom).
    pub fn enregistrer(&mut self, nom: &str, commentaire: &str, favorite: bool, config: &ConfigPartie, date: &str) {
        let entree = Entree { nom: nom.to_string(), commentaire: commentaire.to_string(), favorite, config: config.clone(), origine: Origine::Enregistree, parties: 0, date: date.to_string() };
        match self.entrees.iter_mut().find(|e| e.origine == Origine::Enregistree && e.nom == nom) {
            Some(e) => *e = Entree { parties: e.parties, ..entree },
            None => self.entrees.insert(0, entree),
        }
    }

    /// Note une partie lancée : sa configuration rejoint la liste (« Partie du JJ/MM ») ; une
    /// configuration enregistrée identique compte une partie de plus.
    pub fn partie_lancee(&mut self, config: &ConfigPartie, date: &str) {
        for e in self.entrees.iter_mut().filter(|e| e.origine == Origine::Enregistree && e.config == *config) {
            e.parties += 1;
            e.date = date.to_string();
        }
        let jour = date.get(8..10).unwrap_or("");
        let mois = date.get(5..7).unwrap_or("");
        let entree = Entree {
            nom: format!("Partie du {jour}/{mois}"),
            commentaire: String::new(),
            favorite: false,
            config: config.clone(),
            origine: Origine::Jouee { tour: 1 },
            parties: 1,
            date: date.to_string(),
        };
        self.entrees.insert(0, entree);
        // On garde les parties jouées récentes et les favorites
        let mut jouees = 0;
        self.entrees.retain(|e| {
            if matches!(e.origine, Origine::Jouee { .. }) && !e.favorite {
                jouees += 1;
                jouees <= PARTIES_GARDEES
            } else {
                true
            }
        });
    }

    /// Met à jour le tour atteint par la partie la plus récente jouée avec cette configuration.
    pub fn partie_avancee(&mut self, config: &ConfigPartie, tour: u32) {
        if let Some(e) = self.entrees.iter_mut().find(|e| matches!(e.origine, Origine::Jouee { .. }) && e.config == *config) {
            e.origine = Origine::Jouee { tour };
        }
    }
}

/// Date du jour (UTC) au format AAAA-MM-JJ.
pub fn date_du_jour() -> String {
    let jours = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Algorithme des dates civiles (Howard Hinnant)
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = yoe + ere * 400 + i64::from(mois <= 2);
    format!("{annee:04}-{mois:02}-{jour:02}")
}

/// Configuration de la partie à lancer, passée par l'écran de nouvelle partie (`CIVBUILDER_CONFIG`,
/// texte RON) ; `None` sans elle.
pub fn config_au_lancement() -> Option<ConfigPartie> {
    std::env::var("CIVBUILDER_CONFIG").ok().and_then(|t| ron::from_str(&t).ok())
}

/// Texte de la configuration pour `CIVBUILDER_CONFIG`.
pub fn texte_pour_lancement(config: &ConfigPartie) -> String {
    ron::to_string(config).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_du_monde_aller_retour() {
        let c = ConfigPartie::default();
        assert_eq!(c.code(), "777-M-T29-C0-R0");
        let autre = ConfigPartie { graine: 4242, taille: Taille::Grande, terres: Cran::Plus, climat: Cran::Moins, relief: Cran::Plus, ..Default::default() };
        assert_eq!(autre.code(), "4242-G-T45-C-1-R+1");
        let mut lu = ConfigPartie::default();
        lu.appliquer_code(&autre.code()).unwrap();
        assert_eq!(lu, autre);
        // Une graine seule suffit (taille et réglages inchangés)
        let mut seule = ConfigPartie::default();
        seule.appliquer_code("12345").unwrap();
        assert_eq!(seule.graine, 12345);
        assert_eq!(seule.taille, Taille::Moyenne);
    }

    #[test]
    fn code_illisible_refuse_sans_rien_changer() {
        let mut c = ConfigPartie::default();
        for faux in ["", "abc", "777-X", "777-T33", "777-C5"] {
            assert!(c.appliquer_code(faux).is_err(), "{faux}");
        }
        assert_eq!(c, ConfigPartie::default());
    }

    #[test]
    fn bibliotheque_enregistrer_et_parties() {
        let mut b = Bibliotheque::default();
        let c = ConfigPartie::default();
        b.enregistrer("Grand continent froid", "départ difficile", true, &c, "2026-11-11");
        b.enregistrer("Grand continent froid", "à rejouer", true, &c, "2026-11-11");
        assert_eq!(b.entrees.len(), 1, "même nom : remplacée");
        assert_eq!(b.entrees[0].commentaire, "à rejouer");
        b.partie_lancee(&c, "2026-11-12");
        assert_eq!(b.entrees.len(), 2);
        assert_eq!(b.entrees[0].nom, "Partie du 12/11");
        assert_eq!(b.entrees[1].parties, 1, "la configuration enregistrée compte la partie");
        b.partie_avancee(&c, 212);
        assert_eq!(b.entrees[0].origine, Origine::Jouee { tour: 212 });
        // Les parties jouées non favorites sont bornées
        for _ in 0..40 {
            b.partie_lancee(&ConfigPartie::avec_graine(1), "2026-11-12");
        }
        assert_eq!(b.entrees.iter().filter(|e| matches!(e.origine, Origine::Jouee { .. })).count(), PARTIES_GARDEES);
        assert!(b.entrees.iter().any(|e| e.nom == "Grand continent froid"));
    }

    #[test]
    fn date_au_bon_format() {
        let d = date_du_jour();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert!(d[0..4].parse::<u32>().unwrap() >= 2024);
    }

    #[test]
    fn configuration_passee_au_lancement() {
        let c = ConfigPartie { graine: 9, barbares: Barbares::Nombreux, ..Default::default() };
        let lu: ConfigPartie = ron::from_str(&texte_pour_lancement(&c)).unwrap();
        assert_eq!(lu, c);
    }
}
