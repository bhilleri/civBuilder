//! Types d'unités et leurs statistiques (voir la fiche « conception-combat » du projet).

/// Famille d'unité : sert aux bonus (anti-cavalerie, cavalerie contre archers…).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitClass {
    /// Colon, éclaireur : pas faits pour se battre.
    Civilian,
    Infantry,
    AntiCavalry,
    Ranged,
    Cavalry,
}

/// Statistiques d'un type d'unité.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitStats {
    pub cost: i32,
    pub melee_attack: i32,
    pub melee_defense: i32,
    /// Attaque à distance (0 = aucune). Utilisée à l'étape 2 du combat.
    pub ranged_attack: i32,
    pub range: i32,
    pub ranged_defense: i32,
    pub moves: u32,
    pub sight: i32,
    pub class: UnitClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnitKind {
    Settler,
    Scout,
    Militia,
    ShieldWarrior,
    HeavyWarrior,
    Spearman,
    Pikeman,
    ArcherSwordsman,
    ExpertArcher,
    LightCavalry,
    HeavyCavalry,
    HorseArcher,
}

impl UnitKind {
    /// Ordre d'affichage dans le menu de production.
    pub const ALL: [UnitKind; 12] = [
        UnitKind::Settler,
        UnitKind::Scout,
        UnitKind::Militia,
        UnitKind::ShieldWarrior,
        UnitKind::HeavyWarrior,
        UnitKind::Spearman,
        UnitKind::Pikeman,
        UnitKind::ArcherSwordsman,
        UnitKind::ExpertArcher,
        UnitKind::LightCavalry,
        UnitKind::HeavyCavalry,
        UnitKind::HorseArcher,
    ];

    pub fn name(self) -> &'static str {
        match self {
            UnitKind::Settler => "Colon",
            UnitKind::Scout => "Éclaireur",
            UnitKind::Militia => "Milice",
            UnitKind::ShieldWarrior => "Guerrier bouclier",
            UnitKind::HeavyWarrior => "Guerrier arme lourde",
            UnitKind::Spearman => "Lancier",
            UnitKind::Pikeman => "Piquier",
            UnitKind::ArcherSwordsman => "Archer-épéiste",
            UnitKind::ExpertArcher => "Archer expert",
            UnitKind::LightCavalry => "Cavalerie légère",
            UnitKind::HeavyCavalry => "Cavalerie lourde",
            UnitKind::HorseArcher => "Archer monté",
        }
    }

    /// Nom du fichier d'icône (dans assets/icons).
    pub fn icon(self) -> &'static str {
        match self {
            UnitKind::Settler => "icons/settler.png",
            UnitKind::Scout => "icons/scout.png",
            UnitKind::Militia => "icons/militia.png",
            UnitKind::ShieldWarrior => "icons/shield_warrior.png",
            UnitKind::HeavyWarrior => "icons/heavy_warrior.png",
            UnitKind::Spearman => "icons/spearman.png",
            UnitKind::Pikeman => "icons/pikeman.png",
            UnitKind::ArcherSwordsman => "icons/archer_swordsman.png",
            UnitKind::ExpertArcher => "icons/expert_archer.png",
            UnitKind::LightCavalry => "icons/light_cavalry.png",
            UnitKind::HeavyCavalry => "icons/heavy_cavalry.png",
            UnitKind::HorseArcher => "icons/horse_archer.png",
        }
    }

    pub fn stats(self) -> UnitStats {
        use UnitClass::*;
        // (coût, att. mêlée, déf. mêlée, att. distance, portée, déf. distance, PM, vision, famille)
        let (cost, ma, md, ra, range, rd, moves, sight, class) = match self {
            UnitKind::Settler => (40, 0, 2, 0, 0, 2, 2, 2, Civilian),
            UnitKind::Scout => (15, 4, 5, 0, 0, 5, 3, 3, Civilian),
            UnitKind::Militia => (15, 8, 8, 0, 0, 6, 2, 2, Infantry),
            UnitKind::ShieldWarrior => (30, 10, 16, 0, 0, 18, 2, 2, Infantry),
            UnitKind::HeavyWarrior => (35, 20, 10, 0, 0, 8, 2, 2, Infantry),
            UnitKind::Spearman => (30, 12, 15, 0, 0, 14, 2, 2, AntiCavalry),
            UnitKind::Pikeman => (35, 12, 13, 0, 0, 6, 2, 2, AntiCavalry),
            UnitKind::ArcherSwordsman => (35, 12, 11, 12, 2, 9, 2, 2, Ranged),
            UnitKind::ExpertArcher => (40, 5, 6, 20, 2, 7, 2, 2, Ranged),
            UnitKind::LightCavalry => (45, 16, 9, 0, 0, 8, 4, 3, Cavalry),
            UnitKind::HeavyCavalry => (60, 24, 16, 0, 0, 14, 3, 2, Cavalry),
            UnitKind::HorseArcher => (50, 8, 8, 10, 1, 9, 4, 3, Cavalry),
        };
        UnitStats {
            cost,
            melee_attack: ma,
            melee_defense: md,
            ranged_attack: ra,
            range,
            ranged_defense: rd,
            moves,
            sight,
            class,
        }
    }

    pub fn class(self) -> UnitClass {
        self.stats().class
    }

    pub fn cost(self) -> i32 {
        self.stats().cost
    }

    pub fn max_moves(self) -> u32 {
        self.stats().moves
    }

    pub fn sight(self) -> i32 {
        self.stats().sight
    }

    pub fn is_military(self) -> bool {
        self.class() != UnitClass::Civilian
    }

    /// Peut lancer une attaque au corps à corps.
    pub fn can_attack(self) -> bool {
        self.stats().melee_attack > 0
    }

    /// Unités militaires et éclaireurs : peuvent explorer automatiquement.
    pub fn can_auto_explore(self) -> bool {
        self.is_military() || self == UnitKind::Scout
    }

    /// Zone de contrôle : l'infanterie de mêlée (milice, guerriers, lanciers, piquiers).
    pub fn exerts_zoc(self) -> bool {
        matches!(self.class(), UnitClass::Infantry | UnitClass::AntiCavalry)
    }

    /// Charge de cavalerie possible (cavalerie de mêlée uniquement).
    pub fn can_charge(self) -> bool {
        matches!(self, UnitKind::LightCavalry | UnitKind::HeavyCavalry)
    }

    /// Coût d'un pas en montagne pour ce type d'unité, à partir du coût de base (PM) : la
    /// cavalerie le paie double (chevaux menés à la bride sur les sentiers), l'éclaireur, léger
    /// et habitué aux sentiers, deux tiers (arrondi au-dessus).
    pub fn mountain_cost(self, base: u32) -> u32 {
        match self.class() {
            UnitClass::Cavalry => base * 2,
            _ if self == UnitKind::Scout => (base * 2).div_ceil(3),
            _ => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_unite_a_un_nom_et_une_icone_uniques() {
        let names: bevy::platform::collections::HashSet<_> = UnitKind::ALL.iter().map(|k| k.name()).collect();
        let icons: bevy::platform::collections::HashSet<_> = UnitKind::ALL.iter().map(|k| k.icon()).collect();
        assert_eq!(names.len(), UnitKind::ALL.len());
        assert_eq!(icons.len(), UnitKind::ALL.len());
    }

    #[test]
    fn coherence_des_roles() {
        use UnitKind::*;
        // Le guerrier bouclier défend mieux qu'il n'attaque, l'arme lourde l'inverse
        assert!(ShieldWarrior.stats().melee_defense > ShieldWarrior.stats().melee_attack);
        assert!(HeavyWarrior.stats().melee_attack > HeavyWarrior.stats().melee_defense);
        // La cavalerie est plus rapide que l'infanterie
        assert!(LightCavalry.max_moves() > Militia.max_moves());
        // L'archer monté tire moins fort et moins loin que les autres archers
        for archer in [ArcherSwordsman, ExpertArcher] {
            assert!(HorseArcher.stats().ranged_attack < archer.stats().ranged_attack);
            assert!(HorseArcher.stats().range < archer.stats().range);
        }
        assert!(!Settler.can_attack());
        assert!(Militia.can_auto_explore() && Scout.can_auto_explore() && !Settler.can_auto_explore());
        // Zone de contrôle : infanterie de mêlée uniquement
        for k in [Militia, ShieldWarrior, HeavyWarrior, Spearman, Pikeman] {
            assert!(k.exerts_zoc(), "{k:?}");
        }
        for k in [Settler, Scout, ArcherSwordsman, ExpertArcher, LightCavalry, HeavyCavalry, HorseArcher] {
            assert!(!k.exerts_zoc(), "{k:?}");
        }
    }
}
