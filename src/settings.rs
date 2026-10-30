//! Paramètres du jeu (menu Échap → Paramètres), gardés d'une partie à l'autre dans un petit
//! fichier texte (`clé = valeur`) du dossier de configuration de l'utilisateur. Un nouveau monde
//! relance le jeu : les paramètres le suivent grâce à ce fichier.
//!
//! En mode capture (`CIVBUILDER_CAPTURE`), le fichier n'est ni lu ni écrit : les vues restent
//! les mêmes quels que soient les réglages de la personne qui les lance.

use bevy::prelude::*;
use std::path::PathBuf;

/// Vitesse de la mise en scène sur la carte (déplacements, combats, récompenses).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnimationSpeed {
    Slow,
    #[default]
    Normal,
    Fast,
    /// Pas d'animation : les unités sont aussitôt à leur place, seuls les chiffres s'affichent.
    Instant,
}

impl AnimationSpeed {
    /// Toutes les vitesses, de la plus lente à la plus rapide (ordre du sélecteur).
    pub const ALL: [AnimationSpeed; 4] = [AnimationSpeed::Slow, AnimationSpeed::Normal, AnimationSpeed::Fast, AnimationSpeed::Instant];

    /// Facteur appliqué aux durées des animations (0 : instantané).
    pub fn pace(self) -> f64 {
        match self {
            AnimationSpeed::Slow => 1.5,
            AnimationSpeed::Normal => 1.0,
            AnimationSpeed::Fast => 0.5,
            AnimationSpeed::Instant => 0.0,
        }
    }

    /// Nom affiché dans le sélecteur.
    pub fn label(self) -> &'static str {
        match self {
            AnimationSpeed::Slow => "Lente",
            AnimationSpeed::Normal => "Normale",
            AnimationSpeed::Fast => "Rapide",
            AnimationSpeed::Instant => "Instantanée",
        }
    }

    /// Ce que fait la vitesse choisie (ligne d'aide sous le sélecteur).
    pub fn hint(self) -> &'static str {
        match self {
            AnimationSpeed::Slow => "Animations une fois et demie plus longues : pour suivre chaque coup.",
            AnimationSpeed::Normal => "Un pas de case en 0,2 s, un long trajet en 1,3 s au plus.",
            AnimationSpeed::Fast => "Animations deux fois plus courtes.",
            AnimationSpeed::Instant => "Aucune animation : les unités sont aussitôt à leur place, seuls les chiffres et les récompenses s'affichent.",
        }
    }

    /// Valeur écrite dans le fichier des paramètres.
    fn key(self) -> &'static str {
        match self {
            AnimationSpeed::Slow => "lente",
            AnimationSpeed::Normal => "normale",
            AnimationSpeed::Fast => "rapide",
            AnimationSpeed::Instant => "instantanee",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }
}

/// Paramètres du jeu.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub animation: AnimationSpeed,
}

impl Settings {
    /// Lit le texte du fichier : lignes `clé = valeur`, `#` pour un commentaire ; une clé ou une
    /// valeur inconnue est ignorée (le paramètre garde sa valeur par défaut).
    pub fn parse(text: &str) -> Self {
        let mut settings = Settings::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            if key.trim() == "vitesse_animations"
                && let Some(speed) = AnimationSpeed::from_key(value.trim())
            {
                settings.animation = speed;
            }
        }
        settings
    }

    /// Texte du fichier.
    pub fn to_text(&self) -> String {
        format!(
            "# Paramètres de CivBuilder (menu Échap → Paramètres)\n\
             # vitesse_animations : lente, normale, rapide ou instantanee\n\
             vitesse_animations = {}\n",
            self.animation.key()
        )
    }
}

/// Les paramètres sont lus et écrits (pas en mode capture).
pub fn persistent() -> bool {
    std::env::var_os("CIVBUILDER_CAPTURE").is_none()
}

/// Fichier des paramètres : `CIVBUILDER_SETTINGS` s'il est donné, sinon `%APPDATA%\CivBuilder`
/// sous Windows, `$XDG_CONFIG_HOME/civbuilder` ou `~/.config/civbuilder` ailleurs.
pub fn settings_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CIVBUILDER_SETTINGS") {
        return Some(PathBuf::from(path));
    }
    let dir = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?).join("CivBuilder")
    } else if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(config).join("civbuilder")
    } else {
        PathBuf::from(std::env::var_os("HOME")?).join(".config").join("civbuilder")
    };
    Some(dir.join("parametres.txt"))
}

/// Paramètres enregistrés (ceux par défaut s'il n'y a pas de fichier ou en mode capture).
pub fn load() -> Settings {
    if !persistent() {
        return Settings::default();
    }
    settings_path().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Settings::default, |t| Settings::parse(&t))
}

/// Enregistre les paramètres (rien en mode capture).
pub fn save(settings: &Settings) -> std::io::Result<()> {
    if !persistent() {
        return Ok(());
    }
    let Some(path) = settings_path() else { return Ok(()) };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, settings.to_text())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_parametres_se_relisent_a_l_identique() {
        for speed in AnimationSpeed::ALL {
            let s = Settings { animation: speed };
            assert_eq!(Settings::parse(&s.to_text()), s);
        }
    }

    #[test]
    fn une_valeur_inconnue_garde_la_valeur_par_defaut() {
        assert_eq!(Settings::parse("vitesse_animations = turbo\nautre = 3\n"), Settings::default());
        assert_eq!(Settings::parse("  vitesse_animations=rapide  # commentaire").animation, AnimationSpeed::Fast);
        assert_eq!(Settings::parse("").animation, AnimationSpeed::Normal);
    }

    #[test]
    fn les_vitesses_vont_de_la_plus_lente_a_l_instantanee() {
        let paces: Vec<f64> = AnimationSpeed::ALL.iter().map(|s| s.pace()).collect();
        assert!(paces.windows(2).all(|w| w[0] > w[1]), "{paces:?}");
        assert_eq!(AnimationSpeed::Instant.pace(), 0.0);
    }
}
