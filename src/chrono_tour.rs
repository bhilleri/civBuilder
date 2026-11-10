//! Durée du passage de tour, système par système (logique pure).
//!
//! Le but fixé par Benjamin : **moins de 2 secondes par tour en fin de partie** (12 peuples,
//! des centaines de villes), avec une tolérance de 4 secondes (`docs/conception/conception-interface.md`,
//! « Performance », tâche 44). Chaque passage de tour est mesuré étape par étape ; la console
//! (`chrono`) en donne le bilan et le terminal signale un tour trop long.

use std::collections::VecDeque;
use std::time::Duration;

/// Durée visée pour un passage de tour.
pub const BUDGET_TOUR: Duration = Duration::from_secs(2);
/// Durée au-delà de laquelle un tour est jugé trop long.
pub const TOLERANCE_TOUR: Duration = Duration::from_secs(4);
/// Nombre de tours gardés dans l'historique.
const TOURS_GARDES: usize = 50;

/// Durées d'un passage de tour, étape par étape (dans l'ordre où elles ont joué).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MesureTour {
    /// Tour qui se termine.
    pub tour: u32,
    pub etapes: Vec<(&'static str, Duration)>,
}

impl MesureTour {
    /// Durée totale du passage de tour.
    pub fn total(&self) -> Duration {
        self.etapes.iter().map(|(_, d)| *d).sum()
    }

    /// Une ligne : « tour 5 : 12,3 ms (villes et unités 8,1 ms ; faction hostile 3,0 ms…) ».
    pub fn texte(&self) -> String {
        let detail: Vec<String> = self.etapes.iter().map(|(nom, d)| format!("{nom} {}", ms(*d))).collect();
        format!("tour {} : {} ({})", self.tour, ms(self.total()), detail.join(" ; "))
    }
}

/// Durée lisible, avec une décimale et la virgule française : en millisecondes, ou en secondes
/// à partir d'une seconde.
pub fn ms(d: Duration) -> String {
    let texte = if d >= Duration::from_secs(1) { format!("{:.1} s", d.as_secs_f64()) } else { format!("{:.1} ms", d.as_secs_f64() * 1000.0) };
    texte.replace('.', ",")
}

/// Derniers passages de tour mesurés.
#[derive(Clone, Debug, Default)]
pub struct Historique {
    tours: VecDeque<MesureTour>,
}

impl Historique {
    /// Ajoute un passage de tour (les plus anciens s'effacent).
    pub fn ajouter(&mut self, mesure: MesureTour) {
        if self.tours.len() == TOURS_GARDES {
            self.tours.pop_front();
        }
        self.tours.push_back(mesure);
    }

    /// Bilan des `n` derniers tours : durée moyenne et pire, en tout puis par étape, et le budget.
    pub fn bilan(&self, n: usize) -> Vec<String> {
        let tours: Vec<&MesureTour> = self.tours.iter().rev().take(n.max(1)).collect();
        if tours.is_empty() {
            return vec!["aucun passage de tour mesuré".to_string()];
        }
        let moyenne = |ds: &[Duration]| ds.iter().sum::<Duration>() / ds.len() as u32;
        let totaux: Vec<Duration> = tours.iter().map(|t| t.total()).collect();
        let pire = totaux.iter().copied().max().unwrap_or_default();
        let etat = if pire > TOLERANCE_TOUR {
            "au-delà de la tolérance"
        } else if pire > BUDGET_TOUR {
            "au-delà du budget"
        } else {
            "dans le budget"
        };
        let mut lignes = vec![format!(
            "{} tour(s) : moyenne {}, pire {} ; budget {} (tolérance {}) : {etat}",
            tours.len(),
            ms(moyenne(&totaux)),
            ms(pire),
            ms(BUDGET_TOUR),
            ms(TOLERANCE_TOUR)
        )];
        // Étapes dans l'ordre du dernier tour
        for (nom, _) in &tours[0].etapes {
            let ds: Vec<Duration> = tours.iter().filter_map(|t| t.etapes.iter().find(|(n, _)| n == nom).map(|(_, d)| *d)).collect();
            let pire = ds.iter().copied().max().unwrap_or_default();
            lignes.push(format!("  {nom} : moyenne {}, pire {}", ms(moyenne(&ds)), ms(pire)));
        }
        lignes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesure(tour: u32, a: u64, b: u64) -> MesureTour {
        MesureTour { tour, etapes: vec![("villes et unités", Duration::from_millis(a)), ("faction hostile", Duration::from_millis(b))] }
    }

    #[test]
    fn texte_d_un_tour() {
        assert_eq!(mesure(5, 8, 3).texte(), "tour 5 : 11,0 ms (villes et unités 8,0 ms ; faction hostile 3,0 ms)");
    }

    #[test]
    fn bilan_moyenne_pire_et_budget() {
        let mut h = Historique::default();
        assert_eq!(h.bilan(10), vec!["aucun passage de tour mesuré".to_string()]);
        h.ajouter(mesure(1, 10, 2));
        h.ajouter(mesure(2, 30, 4));
        let b = h.bilan(10);
        assert!(b[0].starts_with("2 tour(s) : moyenne 23,0 ms, pire 34,0 ms"), "{}", b[0]);
        assert!(b[0].ends_with("dans le budget"));
        assert_eq!(b[1], "  villes et unités : moyenne 20,0 ms, pire 30,0 ms");
        assert_eq!(b[2], "  faction hostile : moyenne 3,0 ms, pire 4,0 ms");
        assert!(b[0].contains("budget 2,0 s (tolérance 4,0 s)"), "{}", b[0]);
        h.ajouter(mesure(3, 2500, 0));
        assert!(h.bilan(10)[0].ends_with("au-delà du budget"));
        assert!(h.bilan(1)[0].starts_with("1 tour(s)"));
    }

    #[test]
    fn historique_borne() {
        let mut h = Historique::default();
        for t in 0..(TOURS_GARDES as u32 + 10) {
            h.ajouter(mesure(t, 1, 1));
        }
        assert_eq!(h.tours.len(), TOURS_GARDES);
        assert_eq!(h.tours.back().map(|m| m.tour), Some(TOURS_GARDES as u32 + 9));
    }
}
