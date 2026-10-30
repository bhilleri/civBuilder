//! Toponymie (logique pure) : langues des régions du monde et noms des lieux.
//!
//! - **Langues** : 8 langues en partie inventées. Chacune a les sons d'une vraie famille de
//!   langues (syllabes permises, consonnes fréquentes, terminaisons typiques) et quelques vraies
//!   racines qui ont un sens (froid, eau, montagne, rouge…). Les mots produits n'existent pas,
//!   mais on reconnaît leur ambiance (« ça sonne nordique »), et une racine choisie selon le lieu
//!   peut cacher un sens (« Kaldvik » : *kald*, froid en nordique).
//! - **Aires** : le monde est partagé entre les langues (une graine par langue, répartie sur les
//!   terres, frontières irrégulières) ; chaque langue va là où le climat lui ressemble (nordique au
//!   froid, berbère au chaud et sec, polynésienne parmi les îles…).
//! - **Noms** : génériques et descriptions en français (« Lac », « Mer de », « Rivière Blanche »),
//!   nom propre dans la langue de l'aire, accordé (genre, élision : « bassin de la Vaska »,
//!   « du Taldor », « de l'Ilmar »). Un nom n'est donné qu'une fois.

use crate::basins::{Basins, WaterBody};
use crate::map::Terrain;
use bevy::math::Vec3;
use noise::{NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;

/// Langue d'une aire du monde.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    Nordique,
    Latine,
    Slave,
    Turque,
    Berbere,
    Bantoue,
    Nahua,
    Polynesienne,
}

/// Sens d'une vraie racine glissée dans un nom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Meaning {
    Froid,
    Eau,
    Montagne,
    Rouge,
    Blanc,
    Noir,
    Grand,
    Foret,
    Sable,
    Vent,
    Glace,
    Lac,
    Riviere,
    Ile,
}

impl Meaning {
    pub fn gloss(self) -> &'static str {
        match self {
            Meaning::Froid => "froid",
            Meaning::Eau => "eau",
            Meaning::Montagne => "montagne",
            Meaning::Rouge => "rouge",
            Meaning::Blanc => "blanc",
            Meaning::Noir => "noir",
            Meaning::Grand => "grand",
            Meaning::Foret => "forêt",
            Meaning::Sable => "sable",
            Meaning::Vent => "vent",
            Meaning::Glace => "glace",
            Meaning::Lac => "lac",
            Meaning::Riviere => "rivière",
            Meaning::Ile => "île",
        }
    }
}

/// Origine d'un nom propre : sa langue et le sens de sa racine (s'il en a une).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Etymology {
    pub language: Language,
    pub meaning: Option<Meaning>,
}

impl Etymology {
    /// « nom nordique (« froid ») ».
    pub fn describe(self) -> String {
        match self.meaning {
            Some(m) => format!("nom {}, « {} »", self.language.adjective(), m.gloss()),
            None => format!("nom {}", self.language.adjective()),
        }
    }
}

/// Sons et racines d'une langue.
struct Phonology {
    /// Débuts de syllabe (consonnes).
    onsets: &'static [&'static str],
    vowels: &'static [&'static str],
    /// Fins de syllabe ("" : syllabe ouverte, d'autant plus fréquente qu'il y a de "" dans la
    /// liste).
    codas: &'static [&'static str],
    /// Terminaisons typiques.
    endings: &'static [&'static str],
    /// Préfixes typiques (rares).
    prefixes: &'static [&'static str],
    /// Vraies racines et leur sens.
    roots: &'static [(Meaning, &'static str)],
}

use Meaning::*;

const NORDIQUE: Phonology = Phonology {
    onsets: &["b", "d", "f", "g", "h", "k", "l", "m", "n", "r", "s", "t", "v", "sk", "st", "sv", "br", "dr", "fr", "gr", "kr", "tr", "bj", "hj", "th"],
    vowels: &["a", "e", "i", "o", "u", "y", "ø", "å", "ei", "au"],
    codas: &["", "", "n", "r", "l", "k", "g", "nd", "rd", "ld", "rn", "st", "m"],
    endings: &["vik", "heim", "dal", "ness", "fjord", "borg", "stad", "ey", "by", "holm", "vang", "lund"],
    prefixes: &[],
    roots: &[
        (Froid, "kald"), (Eau, "vatn"), (Montagne, "fjell"), (Rouge, "raud"), (Blanc, "kvit"), (Noir, "svart"), (Grand, "stor"),
        (Foret, "skog"), (Sable, "sand"), (Vent, "vind"), (Glace, "is"), (Lac, "vatn"), (Riviere, "elv"), (Ile, "øy"),
    ],
};

const LATINE: Phonology = Phonology {
    onsets: &["b", "c", "d", "f", "g", "l", "m", "n", "p", "r", "s", "t", "v", "br", "cl", "fl", "gr", "pr", "tr", "qu"],
    vowels: &["a", "e", "i", "o", "u", "ia", "io", "au"],
    codas: &["", "", "", "n", "r", "l", "s", "m", "nt", "rt"],
    endings: &["a", "ia", "ano", "ona", "ium", "ella", "ino", "era", "ento", "ara", "esa", "ago"],
    prefixes: &[],
    roots: &[
        (Froid, "frig"), (Eau, "aqu"), (Montagne, "mont"), (Rouge, "rub"), (Blanc, "alb"), (Noir, "nigr"), (Grand, "magn"),
        (Foret, "silv"), (Sable, "aren"), (Vent, "vent"), (Glace, "glac"), (Lac, "lac"), (Riviere, "flum"), (Ile, "insul"),
    ],
};

const SLAVE: Phonology = Phonology {
    onsets: &["b", "v", "g", "d", "z", "k", "l", "m", "n", "p", "r", "s", "t", "ch", "sh", "zh", "br", "dr", "gr", "kr", "pr", "sl", "sm", "sv", "tv", "vl"],
    vowels: &["a", "e", "i", "o", "u", "y", "ia", "ie"],
    codas: &["", "", "", "v", "n", "k", "r", "l", "m", "ch"],
    endings: &["grad", "ov", "ovo", "ka", "ica", "ets", "ino", "ava", "sk", "nik", "itsa", "evo"],
    prefixes: &[],
    roots: &[
        (Froid, "khol"), (Eau, "vod"), (Montagne, "gor"), (Rouge, "krasn"), (Blanc, "bel"), (Noir, "chern"), (Grand, "velik"),
        (Foret, "les"), (Sable, "pesch"), (Vent, "vetr"), (Glace, "led"), (Lac, "ozer"), (Riviere, "rech"), (Ile, "ostrov"),
    ],
};

const TURQUE: Phonology = Phonology {
    onsets: &["b", "ch", "d", "g", "k", "kh", "m", "n", "s", "sh", "t", "y", "z", "j", "q", "ar", "or"],
    vowels: &["a", "e", "i", "o", "u", "ü", "ö"],
    codas: &["", "n", "r", "l", "k", "sh", "z", "t", "y", "ng", "rt"],
    endings: &["dag", "kul", "su", "tash", "khan", "kent", "abad", "ar", "kol", "tau", "bek", "yurt"],
    prefixes: &[],
    roots: &[
        (Froid, "sogh"), (Eau, "su"), (Montagne, "tagh"), (Rouge, "kizil"), (Blanc, "ak"), (Noir, "kara"), (Grand, "ulu"),
        (Foret, "orman"), (Sable, "kum"), (Vent, "yel"), (Glace, "buz"), (Lac, "kul"), (Riviere, "ögüz"), (Ile, "ada"),
    ],
};

const BERBERE: Phonology = Phonology {
    onsets: &["b", "d", "f", "g", "gh", "k", "kh", "l", "m", "n", "r", "s", "t", "w", "y", "z", "dj", "ch"],
    vowels: &["a", "i", "ou", "e", "a"],
    codas: &["", "", "t", "r", "n", "l", "s", "gh", "z", "m"],
    endings: &["ast", "oult", "ine", "an", "it", "imt", "ount", "ir", "as", "ift"],
    prefixes: &["ta", "ti", "tin", "a", "i", "ou"],
    roots: &[
        (Froid, "semmid"), (Eau, "aman"), (Montagne, "adrar"), (Rouge, "zegg"), (Blanc, "mellal"), (Noir, "berkan"), (Grand, "meqq"),
        (Foret, "tagant"), (Sable, "ijdi"), (Vent, "adou"), (Glace, "agris"), (Lac, "tamda"), (Riviere, "asif"), (Ile, "tigzirt"),
    ],
};

const BANTOUE: Phonology = Phonology {
    onsets: &["b", "ch", "d", "g", "j", "k", "l", "m", "n", "ny", "mb", "nd", "ng", "p", "s", "t", "w", "z", "kw", "mw"],
    vowels: &["a", "e", "i", "o", "u", "a"],
    codas: &[""],
    endings: &["ga", "ni", "nza", "mba", "we", "ro", "la", "ngo", "ji", "ka", "le", "nde"],
    prefixes: &["ki", "ma", "u", "bu", "ka", "lu"],
    roots: &[
        (Froid, "bari"), (Eau, "maji"), (Montagne, "lima"), (Rouge, "kundu"), (Noir, "usi"), (Grand, "kubwa"),
        (Foret, "situ"), (Sable, "changa"), (Vent, "pepo"), (Lac, "ziwa"), (Riviere, "mto"), (Ile, "siwa"),
    ],
};

const NAHUA: Phonology = Phonology {
    onsets: &["c", "ch", "h", "hu", "m", "n", "p", "qu", "t", "tl", "tz", "x", "y", "cu"],
    vowels: &["a", "e", "i", "o", "a"],
    codas: &["", "", "", "n", "l", "c", "z", "x"],
    endings: &["tlan", "co", "pan", "tepec", "huacan", "apan", "ixco", "zinco", "otla", "titlan"],
    prefixes: &[],
    roots: &[
        (Froid, "cec"), (Eau, "atl"), (Montagne, "tepe"), (Rouge, "chichil"), (Blanc, "izta"), (Noir, "tlil"), (Grand, "huey"),
        (Foret, "cuauh"), (Sable, "xal"), (Vent, "eheca"), (Glace, "cetl"), (Lac, "atezca"), (Riviere, "atoya"),
    ],
};

const POLYNESIENNE: Phonology = Phonology {
    onsets: &["h", "k", "l", "m", "n", "p", "r", "t", "w", "wh", "ng", "", ""],
    vowels: &["a", "e", "i", "o", "u", "ai", "au", "ei", "oa"],
    codas: &[""],
    endings: &["nui", "roa", "iti", "moana", "rangi", "motu", "lani", "kai", "tea", "puna"],
    prefixes: &[],
    roots: &[
        (Froid, "anu"), (Eau, "wai"), (Montagne, "maunga"), (Rouge, "ula"), (Blanc, "kea"), (Noir, "ele"), (Grand, "nui"),
        (Foret, "nahele"), (Sable, "one"), (Vent, "hau"), (Glace, "hau"), (Lac, "roto"), (Riviere, "awa"), (Ile, "motu"),
    ],
};

impl Language {
    pub const ALL: [Language; 8] =
        [Language::Nordique, Language::Latine, Language::Slave, Language::Turque, Language::Berbere, Language::Bantoue, Language::Nahua, Language::Polynesienne];

    /// « nordique », « latine »…
    pub fn adjective(self) -> &'static str {
        match self {
            Language::Nordique => "nordique",
            Language::Latine => "latin",
            Language::Slave => "slave",
            Language::Turque => "turc",
            Language::Berbere => "berbère",
            Language::Bantoue => "bantou",
            Language::Nahua => "nahua",
            Language::Polynesienne => "polynésien",
        }
    }

    fn phonology(self) -> &'static Phonology {
        match self {
            Language::Nordique => &NORDIQUE,
            Language::Latine => &LATINE,
            Language::Slave => &SLAVE,
            Language::Turque => &TURQUE,
            Language::Berbere => &BERBERE,
            Language::Bantoue => &BANTOUE,
            Language::Nahua => &NAHUA,
            Language::Polynesienne => &POLYNESIENNE,
        }
    }

    /// Climat préféré : température moyenne (°C), indice d'aridité (pluie / évaporation
    /// possible : 0 désert, 1 et plus humide), goût pour les îles.
    fn climate(self) -> (f32, f32, f32) {
        match self {
            Language::Nordique => (-1.0, 1.0, 0.0),
            Language::Slave => (5.0, 0.8, 0.0),
            Language::Latine => (14.0, 0.7, 0.0),
            Language::Turque => (8.0, 0.3, 0.0),
            Language::Berbere => (23.0, 0.1, 0.0),
            Language::Bantoue => (24.0, 1.3, 0.0),
            Language::Nahua => (19.0, 0.8, 0.0),
            Language::Polynesienne => (24.0, 1.0, 1.0),
        }
    }
}

fn is_vowel(c: char) -> bool {
    "aeiouyøåüöé".contains(c)
}

/// Accole deux morceaux : pas deux voyelles de suite (la seconde tombe), pas trois consonnes de
/// suite (une voyelle s'intercale).
fn join(word: &mut String, part: &str, glue_vowel: &str) {
    let Some(last) = word.chars().last() else {
        word.push_str(part);
        return;
    };
    let first = part.chars().next().unwrap_or('a');
    if is_vowel(last) && is_vowel(first) && part.chars().count() > 1 {
        word.push_str(&part[first.len_utf8()..]);
        return;
    }
    let tail_consonants = word.chars().rev().take_while(|&c| !is_vowel(c)).count();
    let head_consonants = part.chars().take_while(|&c| !is_vowel(c)).count();
    if tail_consonants + head_consonants >= 3 || (tail_consonants >= 1 && head_consonants >= 1 && last == first) {
        word.push_str(glue_vowel);
    }
    word.push_str(part);
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

fn pick<'a>(rng: &mut StdRng, list: &[&'a str]) -> &'a str {
    list[rng.random_range(0..list.len())]
}

/// Un mot de la langue : préfixe (rare), racine du sens voulu (une fois sur deux) ou syllabes,
/// puis souvent une terminaison typique. Rend le mot (avec majuscule) et le sens de sa racine.
pub fn word(language: Language, rng: &mut StdRng, meaning: Option<Meaning>) -> (String, Option<Meaning>) {
    let p = language.phonology();
    let glue = p.vowels[0];
    loop {
        let mut w = String::new();
        if !p.prefixes.is_empty() && rng.random_bool(0.3) {
            w.push_str(pick(rng, p.prefixes));
        }
        let root = meaning.filter(|_| rng.random_bool(0.5)).and_then(|m| p.roots.iter().find(|(x, _)| *x == m)).copied();
        if let Some((_, r)) = root {
            join(&mut w, r, glue);
        }
        let syllables = if root.is_some() { rng.random_range(0..=1) } else { rng.random_range(1..=2) };
        for _ in 0..syllables {
            let mut s = String::new();
            if !(w.is_empty() && rng.random_bool(0.15)) {
                s.push_str(pick(rng, p.onsets));
            }
            s.push_str(pick(rng, p.vowels));
            s.push_str(pick(rng, p.codas));
            join(&mut w, &s, glue);
        }
        if rng.random_bool(0.55) || w.chars().count() < 4 {
            join(&mut w, pick(rng, p.endings), glue);
        }
        let len = w.chars().count();
        if (4..=11).contains(&len) {
            return (capitalize(&w), root.map(|(m, _)| m));
        }
    }
}

/// Un nom propre est féminin s'il finit par un « a » ou un « e » (la Vaska, la Lune ; le Taldor).
pub fn feminine(name: &str) -> bool {
    name.split_whitespace().last().and_then(|w| w.chars().last()).is_some_and(|c| matches!(c, 'a' | 'e' | 'é'))
}

fn starts_with_vowel(name: &str) -> bool {
    // (le « h » est aspiré : « du Hjardal »)
    name.chars().next().is_some_and(|c| is_vowel(c.to_lowercase().next().unwrap_or(c)) || matches!(c, 'É' | 'Î' | 'Ô'))
}

/// Complément de nom : avec article (« de la Vaska », « du Taldor », « de l'Ilmar ») ou sans
/// (« de Vaskar », « d'Ilmar »).
pub fn de(name: &str, fem: bool, article: bool) -> String {
    match (article, starts_with_vowel(name)) {
        (true, true) => format!("de l'{name}"),
        (true, false) if fem => format!("de la {name}"),
        (true, false) => format!("du {name}"),
        (false, true) => format!("d'{name}"),
        (false, false) => format!("de {name}"),
    }
}

/// Distributeur de noms : un nom (et une racine) ne sert qu'une fois.
pub struct Namer {
    rng: StdRng,
    used: HashSet<String>,
}

impl Namer {
    pub fn new(seed: u32) -> Self {
        Self { rng: StdRng::seed_from_u64(u64::from(seed).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x70B0), used: HashSet::new() }
    }

    /// Distributeur qui évite des noms déjà donnés.
    pub fn with_taken(seed: u32, taken: impl IntoIterator<Item = String>) -> Self {
        let mut namer = Self::new(seed);
        namer.used.extend(taken.into_iter().map(|t| t.to_lowercase()));
        namer
    }

    /// Réserve un nom complet ; faux s'il est déjà pris.
    pub fn claim(&mut self, name: &str) -> bool {
        self.used.insert(name.to_lowercase())
    }

    /// Nom propre neuf dans une langue.
    pub fn proper(&mut self, language: Language, meaning: Option<Meaning>) -> (String, Etymology) {
        loop {
            let (w, m) = word(language, &mut self.rng, meaning);
            if self.claim(&w) {
                return (w, Etymology { language, meaning: m });
            }
        }
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.rng.random_bool(p)
    }

    pub fn choose<'a>(&mut self, list: &[&'a str]) -> &'a str {
        pick(&mut self.rng, list)
    }
}

// ------------------------------------------------------------------ Aires des langues

/// Ce qu'il faut savoir d'une case pour la nommer.
#[derive(Clone, Copy, Debug)]
pub struct CellInfo {
    pub language: Language,
    /// Température moyenne (°C).
    pub temperature: f32,
    /// Indice d'aridité (0 désert, 1 et plus humide).
    pub aridity: f32,
    /// Altitude (m).
    pub elevation: f32,
    pub terrain: Terrain,
}

/// Langue de chaque case. `points` : position de chaque case sur le cylindre de la carte (rayon 1) ;
/// `land` : terre ;
/// `climate` : (température, aridité) de chaque case.
pub fn languages(points: &[Vec3], land: &[bool], climate: &[(f32, f32)], seed: u32) -> Vec<Language> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    let mut rng = StdRng::seed_from_u64(u64::from(seed) ^ 0x1A06_0A6E);
    // Frontières irrégulières : chaque point est un peu déplacé par un bruit
    let warp = [Perlin::new(seed.wrapping_add(31)), Perlin::new(seed.wrapping_add(32)), Perlin::new(seed.wrapping_add(33))];
    let warped: Vec<Vec3> = points
        .iter()
        .map(|p| {
            let q = [f64::from(p.x) * 2.5, f64::from(p.y) * 2.5, f64::from(p.z) * 2.5];
            *p + Vec3::new(warp[0].get(q) as f32, warp[1].get(q) as f32, warp[2].get(q) as f32) * 0.22
        })
        .collect();
    // Graines : des cases de terre au hasard (une île isolée n'a pas sa propre graine : elle
    // rejoint l'aire la plus proche)
    let candidates: Vec<usize> = if land.iter().any(|&l| l) { (0..n).filter(|&i| land[i]).collect() } else { (0..n).collect() };
    let mut seeds: Vec<usize> = Vec::new();
    while seeds.len() < Language::ALL.len().min(candidates.len()) {
        let c = candidates[rng.random_range(0..candidates.len())];
        if !seeds.contains(&c) {
            seeds.push(c);
        }
    }
    // Aires de terre comparables : quelques passes de k-moyennes sur les terres (chaque graine
    // va au centre des terres qui lui reviennent)
    let nearest_seed = |p: Vec3, seeds: &[usize]| (0..seeds.len()).min_by(|&a, &b| p.distance(points[seeds[a]]).total_cmp(&p.distance(points[seeds[b]]))).unwrap();
    for _ in 0..12 {
        let mut centers = vec![(Vec3::ZERO, 0usize); seeds.len()];
        for &c in &candidates {
            let s = &mut centers[nearest_seed(points[c], &seeds)];
            s.0 += points[c];
            s.1 += 1;
        }
        for (k, (sum, count)) in centers.into_iter().enumerate() {
            if count == 0 {
                continue;
            }
            let center = sum / count as f32;
            seeds[k] = *candidates.iter().min_by(|&&a, &&b| points[a].distance(center).total_cmp(&points[b].distance(center))).unwrap();
        }
    }
    let region_of: Vec<usize> = warped
        .iter()
        .map(|p| (0..seeds.len()).min_by(|&a, &b| p.distance(points[seeds[a]]).total_cmp(&p.distance(points[seeds[b]]))).unwrap())
        .collect();
    // Climat moyen des terres de chaque aire, et part de mer (îles)
    let mut sums = vec![(0.0f32, 0.0f32, 0usize, 0usize); seeds.len()];
    for i in 0..n {
        let s = &mut sums[region_of[i]];
        if land[i] {
            s.0 += climate[i].0;
            s.1 += climate[i].1.min(2.0);
            s.2 += 1;
        } else {
            s.3 += 1;
        }
    }
    // Chaque langue va à une aire dont le climat lui ressemble : meilleure répartition
    // d'ensemble (toutes les permutations : 8! = 40 320)
    let total_land = sums.iter().map(|s| s.2).sum::<usize>().max(1) as f32;
    let score = |r: usize, l: Language| {
        let (t, a, land_cells, sea_cells) = sums[r];
        // Petite aire de terre (îles éparses) : pour la langue des îles
        let small = land_cells as f32 / total_land < 0.04;
        let (t, a) = if land_cells > 0 { (t / land_cells as f32, a / land_cells as f32) } else { (20.0, 1.0) };
        let sea_share = sea_cells as f32 / (land_cells + sea_cells).max(1) as f32;
        let (lt, la, islands) = l.climate();
        let size = match (small, islands > 0.0) {
            (true, true) => 3.0,
            (true, false) => -3.0,
            _ => 0.0,
        };
        -((t - lt) / 8.0).powi(2) - ((a - la) / 0.5).powi(2) + islands * 4.0 * (sea_share - 0.6) + size
    };
    let table: Vec<Vec<f32>> = (0..seeds.len()).map(|r| Language::ALL.iter().map(|&l| score(r, l)).collect()).collect();
    let mut best = (f32::MIN, Vec::new());
    let mut perm: Vec<usize> = (0..Language::ALL.len()).collect();
    permutations(&mut perm, 0, &mut |p| {
        let total: f32 = p.iter().enumerate().map(|(r, &l)| table[r][l]).sum();
        if total > best.0 {
            best = (total, p.to_vec());
        }
    });
    let language_of: Vec<Language> = best.1.iter().map(|&l| Language::ALL[l]).collect();
    region_of.iter().map(|&r| language_of[r]).collect()
}

/// Appelle `f` sur chaque permutation de `items[k..]` (les premiers restant en place).
fn permutations(items: &mut [usize], k: usize, f: &mut impl FnMut(&[usize])) {
    if k == items.len() {
        f(items);
        return;
    }
    for i in k..items.len() {
        items.swap(k, i);
        permutations(items, k + 1, f);
        items.swap(k, i);
    }
}

// ------------------------------------------------------------------ Noms des eaux

/// Ce qui caractérise un lieu (pour un nom descriptif ou le sens d'une racine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Character {
    Cold,
    High,
    Desert,
    Jungle,
    Forest,
    Marsh,
    Plain,
}

fn character(c: &CellInfo) -> Character {
    match c.terrain {
        _ if c.temperature < -2.0 || matches!(c.terrain, Terrain::Snow | Terrain::Tundra) => Character::Cold,
        _ if c.elevation > 1_800.0 => Character::High,
        Terrain::Desert => Character::Desert,
        _ if c.aridity < 0.25 => Character::Desert,
        Terrain::Jungle => Character::Jungle,
        Terrain::Forest | Terrain::Taiga => Character::Forest,
        Terrain::Marsh => Character::Marsh,
        _ => Character::Plain,
    }
}

/// Le caractère le plus fréquent d'un ensemble de cases.
fn dominant(cells: &[&CellInfo]) -> Character {
    let all = [Character::Cold, Character::High, Character::Desert, Character::Jungle, Character::Forest, Character::Marsh, Character::Plain];
    let count = |k: Character| cells.iter().filter(|c| character(c) == k).count();
    all.into_iter().max_by_key(|&k| (count(k), std::cmp::Reverse(k as u8))).unwrap_or(Character::Plain)
}

/// Langue la plus parlée sur un ensemble de cases.
fn main_language(cells: &[&CellInfo]) -> Language {
    Language::ALL.into_iter().max_by_key(|&l| cells.iter().filter(|c| c.language == l).count()).unwrap_or(Language::Latine)
}

/// Sens de racine qui va à un lieu (pour un cours d'eau ou un lac).
fn water_meaning(k: Character, lake: bool) -> Meaning {
    match k {
        Character::Cold => Glace,
        Character::High => Montagne,
        Character::Desert => Sable,
        Character::Jungle => Noir,
        Character::Forest => Foret,
        Character::Marsh => Eau,
        Character::Plain if lake => Lac,
        Character::Plain => Riviere,
    }
}

/// Nomme les lacs, mers intérieures et cours d'eau d'après les cases qui les portent (`cells` :
/// une par case de la carte). Les bassins prennent le nom de leur fleuve ou de leur lac.
pub fn name_waters(basins: &mut Basins, cells: &[CellInfo], namer: &mut Namer) {
    // Lacs et mers intérieures (du plus grand au plus petit)
    for l in 0..basins.lakes.len() {
        let lake = &basins.lakes[l];
        let infos: Vec<&CellInfo> = lake.cells.iter().filter_map(|&c| cells.get(c)).collect();
        if infos.is_empty() {
            continue;
        }
        let kind = lake.kind;
        let size = lake.cells.len();
        // Bords du lac : ce qui l'entoure le caractérise mieux que l'eau elle-même (l'eau n'a
        // ni forêt ni désert) ; à défaut, le lac
        let k = dominant(&infos);
        let language = main_language(&infos);
        let descriptive: &[&str] = match (kind, k) {
            (WaterBody::InlandSea, Character::Cold) => &["Mer Gelée", "Mer des Glaces"],
            (WaterBody::InlandSea, Character::Desert) => &["Mer des Sables", "Mer Amère"],
            (WaterBody::InlandSea, Character::Jungle) => &["Mer Verte"],
            (WaterBody::InlandSea, _) => &["Mer Amère", "Mer Close"],
            (WaterBody::SaltLake, _) if size >= 20 => &["Grand Lac Salé"],
            (WaterBody::SaltLake, _) => &["Lac Amer", "Lac Salé", "Lac du Mirage"],
            (_, Character::Cold) => &["Lac Gelé", "Lac des Glaces"],
            (_, Character::High) => &["Lac des Cimes", "Lac du Ciel"],
            (_, Character::Forest) => &["Lac des Bois", "Lac Sombre"],
            (_, Character::Jungle) => &["Lac Vert"],
            (_, Character::Marsh) => &["Lac des Roseaux"],
            (_, _) if size >= 30 => &["Grand Lac"],
            _ => &["Lac des Brumes", "Lac Clair", "Lac Tranquille"],
        };
        let pickd = namer.choose(descriptive);
        let (name, etymology) = if namer.chance(0.3) && namer.claim(pickd) {
            (pickd.to_string(), None)
        } else {
            let hint = namer.chance(0.5).then(|| water_meaning(k, true));
            let (w, e) = namer.proper(language, hint);
            let name = match kind {
                WaterBody::InlandSea => format!("Mer {}", de(&w, feminine(&w), false)),
                _ => format!("Lac {w}"),
            };
            (name, Some(e))
        };
        let lake = &mut basins.lakes[l];
        lake.feminine = kind == WaterBody::InlandSea;
        lake.name = name;
        lake.etymology = etymology;
    }
    // Cours d'eau : les fleuves d'abord (du plus abondant au moins abondant), puis les rivières
    let mut order: Vec<usize> = (0..basins.rivers.len()).collect();
    order.sort_by(|&a, &b| {
        let (ra, rb) = (&basins.rivers[a], &basins.rivers[b]);
        rb.fleuve.cmp(&ra.fleuve).then(rb.discharge.total_cmp(&ra.discharge)).then(a.cmp(&b))
    });
    let mut twinned: HashSet<usize> = HashSet::new();
    for r in order {
        let river = &basins.rivers[r];
        let spot_cells: Vec<&CellInfo> = river.spots.iter().flat_map(|&(a, b)| [a, b]).filter_map(|c| cells.get(c)).collect();
        if spot_cells.is_empty() {
            continue;
        }
        let language = main_language(&spot_cells);
        // Source : la partie amont du cours (le dernier quart)
        let upstream: Vec<&CellInfo> = spot_cells[spot_cells.len() * 3 / 4..].to_vec();
        let source = dominant(&upstream);
        let k = dominant(&spot_cells);
        // Affluent presque aussi abondant que le fleuve qu'il rejoint : son jumeau coloré (à la
        // manière du Nil Blanc et du Nil Bleu)
        let twin = river.fleuve
            .then_some(river.flows_into)
            .flatten()
            .filter(|&f| basins.rivers[f].fleuve && !twinned.contains(&f) && basins.rivers[f].etymology.is_some())
            .filter(|&f| river.discharge >= 0.5 * basins.rivers[f].discharge);
        let (name, fem, etymology) = if let Some(f) = twin {
            let base = basins.rivers[f].name.clone();
            let fem = basins.rivers[f].feminine;
            let adjective = match (source, fem) {
                (Character::Cold | Character::High, true) => "Blanche",
                (Character::Cold | Character::High, false) => "Blanc",
                (Character::Jungle | Character::Marsh | Character::Forest, true) => "Noire",
                (Character::Jungle | Character::Marsh | Character::Forest, false) => "Noir",
                (Character::Desert, _) => "Rouge",
                (_, true) => "Petite",
                (_, false) => "Petit",
            };
            let name = if adjective.starts_with("Petit") { format!("{adjective} {base}") } else { format!("{base} {adjective}") };
            if namer.claim(&name) {
                twinned.insert(f);
                (name, fem, basins.rivers[f].etymology)
            } else {
                let (w, e) = namer.proper(language, None);
                (w.clone(), feminine(&w), Some(e))
            }
        } else {
            let descriptive: &[&str] = match source {
                Character::Cold | Character::High => &["Rivière Blanche", "Rivière Froide"],
                Character::Jungle | Character::Marsh => &["Rivière Noire"],
                Character::Desert => &["Rivière Rouge", "Rivière Sèche"],
                Character::Forest => &["Rivière aux Pins"],
                Character::Plain => &["Rivière Claire", "Rivière Lente"],
            };
            let pickd = namer.choose(descriptive);
            if !river.fleuve && namer.chance(0.1) && namer.claim(pickd) {
                (pickd.to_string(), true, None)
            } else {
                let hint = if namer.chance(0.35) { Some(water_meaning(if source == Character::High { source } else { k }, false)) } else { None };
                let (w, e) = namer.proper(language, hint);
                (w.clone(), feminine(&w), Some(e))
            }
        };
        let river = &mut basins.rivers[r];
        river.name = name;
        river.feminine = fem;
        river.etymology = etymology;
    }
}

/// Nom d'un village tribal : un nom propre de la langue du lieu.
pub fn village_name(info: &CellInfo, namer: &mut Namer) -> String {
    namer.proper(info.language, None).0
}

/// Nom d'un camp barbare : souvent une bête du pays (« Repaire des Loups », « Camp des Hyènes »),
/// sinon un nom de la langue du lieu (« Camp de Kaldvik »).
pub fn camp_name(info: &CellInfo, namer: &mut Namer) -> String {
    let beasts: &[&str] = match character(info) {
        Character::Cold => &["des Loups", "des Ours", "du Loup Blanc", "des Corbeaux"],
        Character::High => &["des Aigles", "des Vautours", "du Bouquetin", "des Corbeaux"],
        Character::Desert => &["des Hyènes", "des Scorpions", "des Vautours", "du Chacal"],
        Character::Jungle => &["des Jaguars", "du Serpent", "des Panthères", "du Crocodile"],
        Character::Forest => &["des Loups", "du Sanglier", "des Lynx", "des Ours"],
        Character::Marsh => &["du Crocodile", "des Crapauds", "du Serpent", "des Sangsues"],
        Character::Plain => &["des Chacals", "du Taureau", "des Lions", "des Crânes"],
    };
    let generic = namer.choose(&["Camp", "Repaire", "Antre", "Fort"]);
    if namer.chance(0.65) {
        let name = format!("{generic} {}", namer.choose(beasts));
        if namer.claim(&name) {
            return name;
        }
    }
    let (w, _) = namer.proper(info.language, None);
    let generic = namer.choose(&["Camp", "Repaire", "Fort"]);
    format!("{generic} {}", de(&w, false, false))
}

/// Nom d'une étendue marine, d'après ses côtes (`coast` : cases de terre qui la bordent), sa
/// latitude moyenne, son rang (le plus grand océan) et le fleuve qui s'y jette (golfe, baie).
/// Rend le nom, son genre et l'origine du nom propre.
pub fn sea_name(
    kind: crate::seas::SeaKind,
    coast: &[&CellInfo],
    latitude: f32,
    largest: bool,
    river: Option<(&str, bool)>,
    namer: &mut Namer,
) -> (String, bool, Option<Etymology>) {
    use crate::seas::SeaKind;
    let language = if coast.is_empty() { Language::Polynesienne } else { main_language(coast) };
    let k = if coast.is_empty() { Character::Plain } else { dominant(coast) };
    let cold = k == Character::Cold || latitude.abs() > 60.0;
    let (generic, fem) = match kind {
        SeaKind::Ocean => ("Océan", false),
        SeaKind::Sea => ("Mer", true),
        SeaKind::Gulf => ("Golfe", false),
        SeaKind::Bay => ("Baie", true),
        SeaKind::Strait => ("Détroit", false),
    };
    // Golfe ou baie à l'embouchure d'un fleuve : son nom (une fois sur deux)
    if let Some((name, river_fem)) = river
        && matches!(kind, SeaKind::Gulf | SeaKind::Bay)
        && namer.chance(0.5)
    {
        let full = format!("{generic} {}", de(name, river_fem, true));
        if namer.claim(&full) {
            return (full, fem, None);
        }
    }
    let descriptive: Vec<&str> = match kind {
        SeaKind::Ocean if latitude > 45.0 => vec!["Océan Boréal"],
        SeaKind::Ocean if latitude < -45.0 => vec!["Océan Austral"],
        SeaKind::Ocean if largest => vec!["Grand Océan"],
        SeaKind::Ocean => vec![],
        SeaKind::Sea if cold && latitude >= 0.0 => vec!["Mer des Glaces", "Mer Boréale"],
        SeaKind::Sea if cold => vec!["Mer des Glaces", "Mer Australe"],
        SeaKind::Sea => match k {
            Character::Desert => vec!["Mer Ardente", "Mer de Sel"],
            Character::Jungle => vec!["Mer Émeraude", "Mer des Moussons"],
            Character::Forest => vec!["Mer des Brumes", "Mer Sombre"],
            _ => vec!["Mer des Tempêtes", "Mer Calme", "Mer des Brumes"],
        },
        SeaKind::Gulf if cold => vec!["Golfe Gelé", "Golfe des Glaces"],
        SeaKind::Gulf => match k {
            Character::Desert => vec!["Golfe Ardent"],
            Character::Jungle => vec!["Golfe des Moussons"],
            _ => vec!["Golfe Profond", "Golfe des Tempêtes", "Golfe des Brumes"],
        },
        SeaKind::Bay if cold => vec!["Baie des Phoques", "Baie Gelée"],
        SeaKind::Bay => match k {
            Character::Desert => vec!["Baie des Tortues", "Baie Ardente"],
            Character::Jungle => vec!["Baie des Tortues", "Baie des Palétuviers"],
            Character::Forest => vec!["Baie des Pins"],
            Character::High => vec!["Baie des Falaises"],
            _ => vec!["Baie Calme", "Baie des Naufrages", "Baie des Mouettes"],
        },
        SeaKind::Strait if cold => vec!["Détroit des Glaces"],
        SeaKind::Strait => vec!["Détroit des Brumes", "Passe des Vents", "Détroit des Courants", "Passe Étroite"],
    };
    // Océans polaires et le plus grand : nom descriptif d'office ; ailleurs, 4 fois sur 10
    let p = if kind == SeaKind::Ocean { 0.85 } else { 0.4 };
    if !descriptive.is_empty() && namer.chance(p) {
        let pick = namer.choose(&descriptive);
        if namer.claim(pick) {
            return (pick.to_string(), pick.starts_with("Passe") || fem, None);
        }
    }
    let hint = if namer.chance(0.35) {
        Some(match k {
            _ if cold => Glace,
            Character::Desert => Sable,
            Character::High => Montagne,
            _ if kind == SeaKind::Strait => Vent,
            _ => Eau,
        })
    } else {
        None
    };
    let (w, e) = namer.proper(language, hint);
    let name = match kind {
        SeaKind::Ocean => format!("Océan {w}"),
        _ => format!("{generic} {}", de(&w, feminine(&w), false)),
    };
    let _ = namer.claim(&name);
    (name, fem, Some(e))
}

/// Nom d'un banc de poissons, d'après ce qui le rend riche (`origins`, voir `map::marine`) et
/// l'eau du lieu : souvent les poissons qu'on y pêche (« Bancs des Sardines » dans une remontée
/// d'eaux chaude, « Bancs des Morues » au froid ou à la rencontre de deux courants, « Bancs des
/// Thons » sur un mont sous-marin),
/// sinon « Banc de Kaldvik ».
pub fn bank_name(info: &CellInfo, origins: u8, namer: &mut Namer) -> String {
    use crate::map::marine;
    let cold = info.temperature < 8.0;
    let fish: &[&str] = if origins & marine::SEAMOUNT != 0 {
        &["Bancs des Thons", "Haut-fond des Thons", "Bancs des Espadons"]
    } else if origins & marine::UPWELLING != 0 && !cold {
        &["Bancs des Sardines", "Bancs des Anchois", "Bancs des Maquereaux"]
    } else if cold || origins & marine::FRONT != 0 {
        &["Bancs des Morues", "Bancs des Harengs", "Bancs des Flétans"]
    } else {
        &["Bancs des Bonites", "Bancs des Daurades", "Bancs des Mérous"]
    };
    if namer.chance(0.45) {
        let pick = namer.choose(fish);
        if namer.claim(pick) {
            return pick.to_string();
        }
    }
    let hint = namer.chance(0.3).then_some(if cold { Froid } else { Eau });
    let (w, _) = namer.proper(info.language, hint);
    let generic = if namer.chance(0.5) { "Banc" } else { "Bancs" };
    let name = format!("{generic} {}", de(&w, feminine(&w), false));
    let _ = namer.claim(&name);
    name
}

/// Nom d'une plaque tectonique : « plaque de Vaskar », dans la langue de son centre.
pub fn plate_name(language: Language, namer: &mut Namer) -> String {
    let (w, _) = namer.proper(language, None);
    format!("plaque {}", de(&w, false, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mots_dans_le_style_de_chaque_langue() {
        let mut rng = StdRng::seed_from_u64(1);
        for l in Language::ALL {
            let words: Vec<String> = (0..12).map(|_| word(l, &mut rng, None).0).collect();
            println!("{:<12} {}", l.adjective(), words.join(", "));
            for w in &words {
                let len = w.chars().count();
                assert!((4..=11).contains(&len), "{w}");
                assert!(w.chars().next().unwrap().is_uppercase(), "{w}");
                // Pas trois consonnes de suite au raccord de deux morceaux, pas deux voyelles
                // identiques de suite
                assert!(!w.to_lowercase().contains("aa") && !w.to_lowercase().contains("ii"), "{w}");
            }
        }
        // Une racine demandée apparaît souvent
        let with_root = (0..40).filter(|_| word(Language::Nordique, &mut rng, Some(Froid)).1 == Some(Froid)).count();
        assert!(with_root > 10, "{with_root}");
    }

    #[test]
    fn grammaire_genre_et_elision() {
        assert!(feminine("Vaska") && !feminine("Taldor") && feminine("Petite Vaska"));
        assert_eq!(de("Vaska", true, true), "de la Vaska");
        assert_eq!(de("Taldor", false, true), "du Taldor");
        assert_eq!(de("Ilmar", false, true), "de l'Ilmar");
        assert_eq!(de("Ilmar", false, false), "d'Ilmar");
        assert_eq!(de("Vaskar", false, false), "de Vaskar");
        assert_eq!(de("Hauvik", false, false), "de Hauvik");
    }

    #[test]
    fn un_nom_ne_sert_qu_une_fois() {
        let mut namer = Namer::new(7);
        let names: Vec<String> = (0..400).map(|i| namer.proper(Language::ALL[i % 8], None).0).collect();
        let unique: HashSet<String> = names.iter().map(|n| n.to_lowercase()).collect();
        assert_eq!(unique.len(), names.len());
        assert!(!namer.claim(&names[0]));
        // Même graine, mêmes noms
        let mut again = Namer::new(7);
        assert_eq!(again.proper(Language::ALL[0], None).0, names[0]);
    }

    #[test]
    fn langues_selon_le_climat() {
        // Une bande de cases de l'équateur (chaud et sec à l'ouest, chaud et humide à l'est) au
        // pôle (froid) : le nordique va au froid, le berbère au chaud et sec
        let mut points = Vec::new();
        let mut climate = Vec::new();
        for lat in 0..60 {
            for lon in 0..120 {
                let (la, lo) = ((lat as f32 * 1.4).to_radians(), (lon as f32 * 3.0).to_radians());
                points.push(Vec3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()));
                let t = 26.0 - lat as f32 * 0.5;
                let a = if lon < 60 { 0.1 } else { 1.4 };
                climate.push((t, a));
            }
        }
        let land = vec![true; points.len()];
        let langs = languages(&points, &land, &climate, 3);
        // Ligne et colonne moyennes de l'aire de chaque langue
        let mean = |l: Language| {
            let v: Vec<usize> = (0..langs.len()).filter(|&i| langs[i] == l).collect();
            let n = v.len().max(1) as f32;
            (v.iter().map(|i| i / 120).sum::<usize>() as f32 / n, v.iter().map(|i| i % 120).sum::<usize>() as f32 / n)
        };
        // Le nordique a l'aire la plus froide (la plus proche du pôle) ; le berbère est dans la
        // moitié sèche, le bantou dans la moitié humide
        assert!(Language::ALL.iter().all(|&l| mean(l).0 <= mean(Language::Nordique).0));
        assert!(mean(Language::Berbere).1 < 60.0 && mean(Language::Bantoue).1 >= 60.0);
        let used: HashSet<Language> = langs.iter().copied().collect();
        assert_eq!(used.len(), 8, "toutes les langues ont une aire");
    }

    /// Bilan des noms d'un monde (graine 777) : `cargo test bilan_noms -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bilan_noms() {
        let m = crate::map::WorldMap::generate_in(192, 111, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 111));
        for l in Language::ALL {
            let land = m.hexes().filter(|&h| m.get(h).is_some_and(|t| !t.is_water()) && m.cell_info(h).unwrap().language == l).count();
            println!("{:<11} {land} cases de terre", l.adjective());
        }
        let b = m.basins();
        let describe = |e: Option<Etymology>| e.map_or("descriptif".to_string(), |e| e.describe());
        println!("\nFleuves ({} cours d'eau nommés) :", b.rivers.len());
        for r in b.rivers.iter().filter(|r| r.fleuve).take(20) {
            println!("  {:<22} {:>7.0} m³/s  {}", r.name, r.discharge, describe(r.etymology));
        }
        println!("Rivières :");
        for r in b.rivers.iter().filter(|r| !r.fleuve).take(15) {
            println!("  {:<22} {}", r.name, describe(r.etymology));
        }
        println!("Lacs ({}) :", b.lakes.len());
        for l in b.lakes.iter().take(20) {
            println!("  {:<26} {:>3} cases  {}", l.name, l.cells.len(), describe(l.etymology));
        }
        println!("Bassins :");
        for basin in b.basins.iter().filter(|x| x.major()).take(10) {
            println!("  {}", b.basin_title(basin).unwrap());
        }
        println!("Plaques : {}", m.plates().plates.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "));
        let mut namer = Namer::with_taken(9, m.names_taken());
        let camps: Vec<String> = m.hexes().filter(|&h| m.get(h).is_some_and(|t| !t.is_water())).step_by(997).map(|h| camp_name(&m.cell_info(h).unwrap(), &mut namer)).collect();
        println!("Camps : {}", camps.join(", "));
        let villages: Vec<String> = m.hexes().filter(|&h| m.get(h).is_some_and(|t| !t.is_water())).step_by(1499).map(|h| village_name(&m.cell_info(h).unwrap(), &mut namer)).collect();
        println!("Villages : {}", villages.join(", "));
    }
}
