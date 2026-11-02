//! Carte géographique (logique pure) : le découpage du territoire en zones nommées, une famille
//! à la fois (terres, mers, relief, régions naturelles), à deux niveaux.
//!
//! - **Niveau du dessus** (vue lointaine) : les grandes zones (continents, îles et archipels ;
//!   océans et mers ; chaînes, plateaux et collines ; régions naturelles), une couleur chacune.
//! - **Niveau du dessous** (vue proche) : leurs parties (péninsules, isthmes, îles d'un archipel ;
//!   golfes, baies, détroits ; massifs, cols ; sous-régions), en nuances de la couleur de leur
//!   grande zone (on voit qu'elles la composent) ; le reste de la grande zone garde sa couleur, en
//!   teinte sourde.
//! - **Couleurs** : deux grandes zones voisines n'ont jamais la même, ni deux parties voisines
//!   d'une même grande zone la même nuance (coloriage de graphe « DSatur » : la zone qui a déjà le
//!   plus de couleurs différentes autour d'elle choisit d'abord).

use crate::hex::{DIRECTIONS, Hex};
use crate::map::WorldMap;
use bevy::platform::collections::{HashMap, HashSet};

/// Famille de zones montrée par la carte géographique.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GeoFamily {
    #[default]
    Lands,
    Seas,
    Reliefs,
    Regions,
}

impl GeoFamily {
    pub const ALL: [GeoFamily; 4] = [GeoFamily::Lands, GeoFamily::Seas, GeoFamily::Reliefs, GeoFamily::Regions];

    pub fn label(self) -> &'static str {
        match self {
            GeoFamily::Lands => "Terres",
            GeoFamily::Seas => "Mers",
            GeoFamily::Reliefs => "Relief",
            GeoFamily::Regions => "Régions naturelles",
        }
    }
}

/// Nombre de couleurs de zones.
pub const GEO_COLORS: usize = 10;

/// Nuances d'une couleur pour les parties d'une grande zone (0 : la couleur elle-même), puis la
/// teinte sourde (`MUTED`) du reste de la grande zone.
pub const GEO_TONES: u8 = 4;
pub const MUTED: u8 = GEO_TONES;

/// Teinte d'une case sur la carte géographique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoShade {
    /// Dans une zone montrée : la couleur de sa grande zone, et la nuance de la partie (0 au niveau
    /// du dessus), ou `MUTED` pour une grande zone hors de ses parties.
    Zone { color: u8, tone: u8 },
    /// Hors des zones de la famille.
    Land,
    Water,
}

/// Teintes de la carte géographique, et grande zone de chaque case (pour tracer leurs limites au
/// niveau du dessous).
#[derive(Clone, Debug, Default)]
pub struct GeoMap {
    pub shade: Vec<GeoShade>,
    pub top: Vec<Option<usize>>,
}

/// Grande zone et zone montrée d'une case (indices dans les zones de la famille), et si la zone
/// montrée est la grande zone, en teinte sourde, faute de sous-zone.
fn zone_of(map: &WorldMap, family: GeoFamily, deep: bool, c: usize) -> (Option<usize>, Option<(usize, bool)>) {
    let at = |v: &[Option<u16>]| v.get(c).copied().flatten().map(usize::from);
    // (grande zone, sous-zone ; une grande zone sans partie n'est pas en teinte sourde)
    let (top, sub, has_parts): (Option<usize>, Option<usize>, bool) = match family {
        GeoFamily::Lands => {
            let l = map.lands();
            let top = at(&l.group_of).or(at(&l.mass_of));
            // (dans un archipel, ses îles sont ses parties)
            let sub = at(&l.isthmus_of).or(at(&l.part_of)).or(at(&l.group_of).and(at(&l.mass_of)));
            (top, sub, top.is_some_and(|t| l.zones.iter().any(|z| z.parent == Some(t))))
        }
        GeoFamily::Seas => {
            let s = map.seas();
            let top = at(&s.coastal_of).or(at(&s.basin_of));
            let sub = at(&s.strait_of).or(at(&s.bay_of)).or(at(&s.inlet_of));
            (top, sub, top.is_some_and(|t| s.zones.iter().any(|z| z.parent == Some(t))))
        }
        GeoFamily::Reliefs => {
            let r = map.reliefs();
            let top = at(&r.range_of).or(at(&r.plateau_of)).or(at(&r.hills_of));
            let sub = at(&r.pass_of).or(at(&r.massif_of));
            (top, sub, top.is_some_and(|t| r.zones.iter().any(|z| z.parent == Some(t))))
        }
        GeoFamily::Regions => {
            let r = map.regions();
            let top = at(&r.region_of);
            let sub = at(&r.part_of);
            (top, sub, top.is_some_and(|t| r.zones.iter().any(|z| z.parent == Some(t))))
        }
    };
    let shown = match (deep, sub) {
        (true, Some(s)) => Some((s, false)),
        (true, None) => top.map(|t| (t, has_parts)),
        (false, _) => top.map(|t| (t, false)),
    };
    (top, shown)
}

/// Zones voisines : taille de chaque zone et zones qu'elle touche (cases voisines de zones
/// différentes), parmi les cases que `key` range dans une zone.
fn neighbors_of(map: &WorldMap, key: impl Fn(usize) -> Option<usize>) -> (HashMap<usize, usize>, HashMap<usize, HashSet<usize>>) {
    let n = (map.width * map.height) as usize;
    let mut size: HashMap<usize, usize> = HashMap::new();
    let mut touching: HashMap<usize, HashSet<usize>> = HashMap::new();
    for c in 0..n {
        let Some(z) = key(c) else { continue };
        *size.entry(z).or_default() += 1;
        let h = map.hex_at(c);
        for d in DIRECTIONS {
            let nb = map.canonical(h + d);
            if !map.contains(nb) {
                continue;
            }
            if let Some(o) = key(map.cell(nb))
                && o != z
            {
                touching.entry(z).or_default().insert(o);
                touching.entry(o).or_default().insert(z);
            }
        }
    }
    (size, touching)
}

/// Carte géographique d'une famille, au niveau du dessus ou du dessous (`deep`).
pub fn compute(map: &WorldMap, family: GeoFamily, deep: bool) -> GeoMap {
    let n = (map.width * map.height) as usize;
    let mut top = vec![None; n];
    let mut shown: Vec<Option<(usize, bool)>> = vec![None; n];
    for c in 0..n {
        (top[c], shown[c]) = zone_of(map, family, deep, c);
    }
    // Couleurs des grandes zones (voisines : couleurs différentes)
    let (size, touching) = neighbors_of(map, |c| top[c]);
    let colors = color_zones(&size, &touching, GEO_COLORS);
    // Nuances des parties : voisines dans une même grande zone, nuances différentes (des parties
    // de deux grandes zones différentes ont déjà des couleurs différentes)
    let part = |c: usize| shown[c].filter(|&(z, muted)| !muted && Some(z) != top[c]).map(|(z, _)| z);
    let (part_size, mut part_touching) = neighbors_of(map, part);
    let parent_of: HashMap<usize, Option<usize>> = (0..n).filter_map(|c| part(c).map(|z| (z, top[c]))).collect();
    for (z, others) in part_touching.iter_mut() {
        others.retain(|o| parent_of.get(o) == parent_of.get(z));
    }
    let tones = color_zones(&part_size, &part_touching, usize::from(GEO_TONES));
    let shade = (0..n)
        .map(|c| match (shown[c], top[c]) {
            (Some((_, true)), Some(t)) => GeoShade::Zone { color: colors[&t], tone: MUTED },
            (Some((z, false)), Some(t)) => GeoShade::Zone { color: colors[&t], tone: if z == t { 0 } else { tones[&z] } },
            (Some((z, _)), None) => GeoShade::Zone { color: colors.get(&z).copied().unwrap_or(0), tone: 0 },
            (None, _) if map.get(map.hex_at(c)).is_some_and(|t| t.is_water()) => GeoShade::Water,
            (None, _) => GeoShade::Land,
        })
        .collect();
    GeoMap { shade, top }
}

/// Une zone de la famille est-elle une partie d'une grande zone (montrée au niveau du dessous) ?
/// Péninsules, isthmes et îles d'un archipel ; golfes, baies, détroits ; massifs, cols ;
/// sous-régions.
pub fn is_part(map: &WorldMap, family: GeoFamily, zone: usize) -> bool {
    use crate::lands::LandKind;
    use crate::mountains::ReliefKind;
    use crate::seas::SeaKind;
    match family {
        GeoFamily::Lands => map.lands().zones.get(zone).is_some_and(|z| matches!(z.kind, LandKind::Peninsula | LandKind::Isthmus) || (matches!(z.kind, LandKind::Island | LandKind::Islet) && z.parent.is_some())),
        GeoFamily::Seas => map.seas().zones.get(zone).is_some_and(|z| matches!(z.kind, SeaKind::Gulf | SeaKind::Bay | SeaKind::Strait)),
        GeoFamily::Reliefs => map.reliefs().zones.get(zone).is_some_and(|z| matches!(z.kind, ReliefKind::Massif | ReliefKind::Pass)),
        GeoFamily::Regions => map.regions().zones.get(zone).is_some_and(|z| z.parent.is_some()),
    }
}

/// Coloriage des zones sur `count` couleurs (DSatur) : à chaque pas, la zone qui touche le plus
/// de couleurs déjà données (puis la plus grande) prend, parmi les couleurs libres autour d'elle,
/// la moins donnée jusque-là (des zones qui ne se touchent pas, des îles, n'ont pas toutes la
/// même) ; s'il n'y en a pas, la moins présente autour d'elle.
fn color_zones(size: &HashMap<usize, usize>, touching: &HashMap<usize, HashSet<usize>>, count: usize) -> HashMap<usize, u8> {
    let mut colors: HashMap<usize, u8> = HashMap::new();
    let mut given = vec![0usize; count];
    let mut left: Vec<usize> = size.keys().copied().collect();
    left.sort_unstable();
    let empty = HashSet::new();
    while !left.is_empty() {
        let around = |z: usize, colors: &HashMap<usize, u8>| -> Vec<u8> { touching.get(&z).unwrap_or(&empty).iter().filter_map(|o| colors.get(o).copied()).collect() };
        let saturation = |z: usize| around(z, &colors).into_iter().collect::<HashSet<u8>>().len();
        let (k, &z) = left.iter().enumerate().max_by_key(|&(_, &z)| (saturation(z), size[&z], std::cmp::Reverse(z))).unwrap();
        left.swap_remove(k);
        let used = around(z, &colors);
        let color = (0..count as u8)
            .filter(|c| !used.contains(c))
            .min_by_key(|&c| (given[usize::from(c)], c))
            .unwrap_or_else(|| (0..count as u8).min_by_key(|c| used.iter().filter(|u| *u == c).count()).unwrap_or(0));
        given[usize::from(color)] += 1;
        colors.insert(z, color);
    }
    colors
}

/// Limites des grandes zones (niveau du dessous) : arêtes entre deux cases explorées de grandes
/// zones différentes, ou entre une grande zone et le dehors (monde 2D) ; pas les côtes, déjà
/// visibles (une case de terre et une d'eau).
pub fn top_borders(map: &WorldMap, geo: &GeoMap, explored: impl Fn(Hex) -> bool) -> Vec<[bevy::math::Vec2; 2]> {
    let mut edges = Vec::new();
    for (c, &zone) in geo.top.iter().enumerate() {
        let Some(zone) = zone else { continue };
        let hex = map.hex_at(c);
        if !explored(hex) {
            continue;
        }
        for (d, &step) in DIRECTIONS.iter().enumerate() {
            let nb = map.canonical(hex + step);
            if map.contains(nb) && geo.top[map.cell(nb)] == Some(zone) {
                continue;
            }
            let wet = |h: Hex| map.get(h).is_some_and(|t| t.is_water());
            if map.contains(nb) && wet(hex) != wet(nb) {
                continue;
            }
            // (chaque arête une fois : du côté de la plus petite zone, ou de la case de la zone)
            let other = map.contains(nb).then(|| geo.top[map.cell(nb)]).flatten();
            if other.is_some_and(|o| o < zone) && explored(nb) {
                continue;
            }
            let (a, b) = hex.edge_corners(d);
            edges.push([bevy::math::Vec2::from(a), bevy::math::Vec2::from(b)]);
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_zones_voisines_n_ont_pas_la_meme_couleur() {
        let m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        for family in GeoFamily::ALL {
            for deep in [false, true] {
                let geo = compute(&m, family, deep);
                let mut zones = 0;
                for c in 0..geo.shade.len() {
                    let GeoShade::Zone { color, tone } = geo.shade[c] else { continue };
                    zones += 1;
                    let (mine_top, mine) = zone_of(&m, family, deep, c);
                    for d in DIRECTIONS {
                        let nb = m.canonical(m.hex_at(c) + d);
                        if !m.contains(nb) {
                            continue;
                        }
                        let j = m.cell(nb);
                        let (theirs_top, theirs) = zone_of(&m, family, deep, j);
                        let GeoShade::Zone { color: other, tone: other_tone } = geo.shade[j] else { continue };
                        // Grandes zones différentes : couleurs différentes ; même grande zone :
                        // même couleur, et deux parties différentes, nuances différentes
                        if mine_top != theirs_top {
                            assert_ne!(color, other, "{family:?} {deep} : cases {c} et {j}");
                        } else {
                            assert_eq!(color, other, "{family:?} {deep} : cases {c} et {j}");
                            if mine.map(|z| z.0) != theirs.map(|z| z.0) {
                                assert_ne!(tone, other_tone, "{family:?} {deep} : cases {c} et {j}");
                            }
                        }
                    }
                }
                assert!(zones > 0, "{family:?} {deep} : aucune zone");
            }
        }
    }

    #[test]
    fn niveau_du_dessous_les_parties_et_la_grande_zone_en_sourdine() {
        let m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        let geo = compute(&m, GeoFamily::Regions, true);
        let r = m.regions();
        // Une case d'une sous-région : pas en sourdine ; d'une région découpée hors de ses
        // sous-régions : impossible (une région découpée l'est entièrement) ; d'une région non
        // découpée : pleine couleur
        for c in 0..geo.shade.len() {
            if let GeoShade::Zone { tone, .. } = geo.shade[c] {
                assert!(tone != MUTED || r.part_of[c].is_none(), "case {c}");
            }
        }
        let top = compute(&m, GeoFamily::Regions, false);
        assert!(top.shade.iter().all(|s| matches!(s, GeoShade::Zone { tone: 0, .. } | GeoShade::Land | GeoShade::Water)));
    }
}
