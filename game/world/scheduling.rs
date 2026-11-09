//! Ordonnancement des chunks : priorisation et calcul des chunks manquants.
//!
//! Ce module regroupe la logique d'ordonnancement partagée entre la
//! génération et le maillage des chunks, afin que les deux utilisent
//! **exactement la même métrique de priorité**.
//!
//! La priorité d'un chunk est la **distance au carré** entre ce chunk et la
//! position du joueur : plus la distance est petite, plus le chunk est
//! prioritaire.
//!
//! L'axe vertical est pondéré à 25% (`(dy * dy) >> 2`). Le monde étant très
//! large (17 chunks) et beaucoup moins haut (7 chunks), un chunk situé deux
//! chunks au-dessus du joueur est moins pertinent qu'un chunk situé deux
//! chunks sur le côté : c'est ce que traduit cette pondération.
//!
//! Les distances sont exprimées en **unités de chunk** (et non en blocs) :
//! la mesure reste alors stable et bornée, et le tri reste très rapide.

use rustc_hash::FxHashSet;

use crate::world::chunk_position::ChunkPosition;

/// Décalage appliqué au carré de la composante verticale de la distance.
///
/// `2` => poids de 25%, soit `(dy * dy) >> 2`.
pub const VERTICAL_PRIORITY_SHIFT: u32 = 2;

/// Distance au carré (pondérée verticalement) entre un chunk et le joueur.
///
/// - `chunk` : coordonnées du chunk considéré.
/// - `player_chunk` : coordonnées du chunk dans lequel se trouve le joueur.
///
/// La valeur retournée est **plus petite = plus prioritaire**.
///
/// Les écarts sont calculés en `i64` : un écart signé correct est
/// indispensable, car les coordonnées de chunk sont négatives d'un côté de
/// l'origine et positives de l'autre. Calculer l'écart sur une réinterprétation
/// non signée (`i32 as u32`) donnerait une distance absurde (près de 2^32)
/// entre deux chunks de part et d'autre de l'origine, et donc un ordre de
/// priorité entièrement faux.
///
/// Les multiplications sont saturantes et le résultat final est ramené dans
/// un `u32` : la fonction est donc définie pour toutes les coordonnées.
#[inline(always)]
pub fn chunk_priority_sq(chunk: &ChunkPosition, player_chunk: &ChunkPosition) -> u32 {
    let (cx, cy, cz): (i32, i32, i32) = chunk.into();
    let (px, py, pz): (i32, i32, i32) = player_chunk.into();

    let dx = (cx as i64 - px as i64).unsigned_abs();
    let dy = (cy as i64 - py as i64).unsigned_abs();
    let dz = (cz as i64 - pz as i64).unsigned_abs();

    let horizontal = dx.saturating_mul(dx).saturating_add(dz.saturating_mul(dz));
    let vertical = dy.saturating_mul(dy) >> VERTICAL_PRIORITY_SHIFT;

    horizontal.saturating_add(vertical).min(u32::MAX as u64) as u32
}

/// Priorité d'un chunk, exprimée sur une échelle comparable à la distance
/// euclidienne (racine carrée de [`chunk_priority_sq`]).
///
/// Utile pour l'affichage et le tri "humanement lisible", mais **plus coûteux**
/// que [`chunk_priority_sq`] : à réserver aux usages non critiques.
#[inline]
pub fn chunk_priority(chunk: &ChunkPosition, player_chunk: &ChunkPosition) -> f32 {
    (chunk_priority_sq(chunk, player_chunk) as f32).sqrt()
}

/// Calcule les chunks restant à générer, triés par priorité croissante.
///
/// - `candidate_keys` : l'ensemble des chunks de la zone de simulation.
/// - `existing` : les chunks déjà générés (présents en mémoire).
/// - `in_flight` : les chunks déjà demandés au générateur mais pas encore
///   reçus. Les redemander produirait du travail inutile.
/// - `player_chunk` : position du joueur, en coordonnées de chunk.
///
/// Le tri est **stable** : deux chunks de même priorité conservent leur ordre
/// d'entrée, ce qui rend le résultat déterministe.
pub fn compute_missing_chunks_sorted<'a>(
    candidate_keys: impl Iterator<Item = &'a ChunkPosition>,
    existing: &FxHashSet<ChunkPosition>,
    in_flight: &FxHashSet<ChunkPosition>,
    player_chunk: &ChunkPosition,
) -> Vec<ChunkPosition> {
    let mut missing: Vec<ChunkPosition> = candidate_keys
        .filter_map(|key| match !existing.contains(key) && !in_flight.contains(key) {
            true => Some(*key),
            false => None,
        })
        .collect();

    missing.sort_by_key(|coords| chunk_priority_sq(coords, &player_chunk));
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: ChunkPosition = ChunkPosition::new(0, 0, 0);

    #[test]
    fn test_priority_is_zero_on_player_chunk() {
        assert_eq!(chunk_priority_sq(&ORIGIN, &ORIGIN), 0);
    }

    #[test]
    fn test_priority_is_symmetric() {
        let a = chunk_priority_sq(&ChunkPosition::new(3, 2, -5), &ORIGIN);
        let b = chunk_priority_sq(&ChunkPosition::new(-3, -2, 5), &ORIGIN);
        assert_eq!(a, b);
    }

    #[test]
    fn test_priority_across_origin_is_correct() {
        // Régression : l'écart entre -1 et +1 vaut 2, et non 2^32 - 2.
        assert_eq!(
            chunk_priority_sq(&ChunkPosition::new(1, 0, 0), &ChunkPosition::new(-1, 0, 0)),
            4
        );
        assert_eq!(
            chunk_priority_sq(&ChunkPosition::new(-1, 0, 0), &ChunkPosition::new(1, 0, 0)),
            4
        );
        assert_eq!(
            chunk_priority_sq(&ChunkPosition::new(-3, 0, 0), &ChunkPosition::new(3, 0, 0)),
            36
        );

        // Le signe des coordonnées ne doit pas perturber l'ordre.
        assert_eq!(chunk_priority_sq(&ChunkPosition::new(-2, 0, 0), &ORIGIN), 4);
        assert_eq!(chunk_priority_sq(&ChunkPosition::new(2, 0, 0), &ORIGIN), 4);
    }

    #[test]
    fn test_priority_is_exact_in_a_realistic_volume() {
        // Sur le volume réel de simulation (17 x 7 x 17), la priorité doit
        // être exacte (pas de saturation) et suivre la formule attendue.
        let player = &ORIGIN;
        for x in -8..=8 {
            for y in -3..=3 {
                for z in -8..=8 {
                    let p = chunk_priority_sq(&ChunkPosition::new(x, y, z), player);
                    let dx = (x as i64).unsigned_abs();
                    let dy = (y as i64).unsigned_abs();
                    let dz = (z as i64).unsigned_abs();
                    let expected = dx * dx + dz * dz + ((dy * dy) >> VERTICAL_PRIORITY_SHIFT);
                    assert_eq!(p as u64, expected, "priorité incorrecte pour {:?}", (x, y, z));
                }
            }
        }
    }

    #[test]
    fn test_horizontal_distance_is_not_discounted() {
        // 3 chunks sur X => 9
        assert_eq!(chunk_priority_sq(&ChunkPosition::new(3, 0, 0), &ORIGIN), 9);
        assert_eq!(chunk_priority_sq(&ChunkPosition::new(0, 0, 3), &ORIGIN), 9);
    }

    #[test]
    fn test_vertical_distance_is_discounted() {
        // La composante verticale est pondérée par 1 / 2^VERTICAL_PRIORITY_SHIFT :
        // 4 chunks en Y => 16 >> 2 = 4, 2 chunks en Y => 4 >> 2 = 1.
        assert_eq!(chunk_priority_sq(&ChunkPosition::new(0, 4, 0), &ORIGIN), 4);
        assert_eq!(chunk_priority_sq(&ChunkPosition::new(0, 2, 0), &ORIGIN), 1);
    }

    #[test]
    fn test_vertical_discount_follows_the_constant() {
        // Le poids vertical doit rester cohérent avec la constante : changer
        // le réglage ne doit pas rendre les tests faux.
        for dy in 0..8i32 {
            let expected = (dy as u64 * dy as u64) >> VERTICAL_PRIORITY_SHIFT;
            assert_eq!(chunk_priority_sq(&ChunkPosition::new(0, dy, 0), &ORIGIN) as u64, expected);
        }
    }

    #[test]
    fn test_horizontal_is_prioritary_over_vertical() {
        // 4 chunks à l'horizontale (16) est plus prioritaire que 4 chunks en
        // vertical (4, la composante verticale étant pondérée).
        let horizontal = chunk_priority_sq(&ChunkPosition::new(4, 0, 0), &ORIGIN);
        let vertical = chunk_priority_sq(&ChunkPosition::new(0, 4, 0), &ORIGIN);
        assert!(horizontal > vertical);
    }

    #[test]
    fn test_priority_does_not_overflow() {
        // Coordonnées extrêmes : le calcul doit rester défini (saturation),
        // sans jamais déborder (ce qui fausserait l'ordre).
        let p = chunk_priority_sq(
            &ChunkPosition::new(i32::MAX, i32::MIN, 0),
            &ChunkPosition::new(i32::MIN, i32::MAX, 0),
        );
        assert_eq!(p, u32::MAX, "la priorité doit saturer, pas déborder");

        // Un chunk proche ne doit pas être vu comme plus prioritaire qu'un
        // chunk à l'autre bout du monde à cause d'un débordement.
        let near = chunk_priority_sq(&ChunkPosition::new(1, 0, 0), &ORIGIN);
        let far = chunk_priority_sq(&ChunkPosition::new(i32::MAX, 0, 0), &ORIGIN);
        assert!(near < far);
    }

    #[test]
    fn test_priority_is_monotonic_along_an_axis() {
        let mut previous = 0;
        for i in 1..12 {
            let p = chunk_priority_sq(&ChunkPosition::new(i, 0, 0), &ORIGIN);
            assert!(p > previous, "la priorité doit croître avec la distance (i={i})");
            previous = p;
        }
    }

    #[test]
    fn test_chunk_priority_matches_squared_priority() {
        let p = chunk_priority(&ChunkPosition::new(3, 0, 4), &ChunkPosition::new(0, 0, 0));
        assert!((p - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_compute_missing_excludes_existing_and_in_flight() {
        let existing = FxHashSet::from_iter([ChunkPosition::new(1, 0, 0)].into_iter());
        let in_flight = FxHashSet::from_iter([ChunkPosition::new(2, 0, 0)].into_iter());

        let candidates = [(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0)].map(|(x, y, z)| ChunkPosition::new(x, y, z));
        let missing = compute_missing_chunks_sorted(candidates.iter(), &existing, &in_flight, &ORIGIN);

        assert_eq!(missing, vec![ORIGIN, ChunkPosition::new(3, 0, 0)]);
    }

    #[test]
    fn test_compute_missing_is_sorted_by_priority() {
        let existing: FxHashSet<ChunkPosition> = FxHashSet::default();
        let in_flight: FxHashSet<ChunkPosition> = FxHashSet::default();

        // Insertion volontairement dans le désordre.
        let candidates = [(5, 0, 0), (1, 0, 0), (0, 0, 0), (3, 0, 0), (0, 0, 4)].map(|(x, y, z)| ChunkPosition::new(x, y, z));
        let missing = compute_missing_chunks_sorted(candidates.iter(), &existing, &in_flight, &ORIGIN);

        assert_eq!(
            missing,
            [(0, 0, 0), (1, 0, 0), (3, 0, 0), (0, 0, 4), (5, 0, 0)]
                .map(|(x, y, z)| ChunkPosition::new(x, y, z))
                .to_vec(),
            "les chunks doivent être triés par distance croissante au joueur"
        );
    }

    #[test]
    fn test_compute_missing_uses_vertical_discount() {
        let existing: FxHashSet<ChunkPosition> = FxHashSet::default();
        let in_flight: FxHashSet<ChunkPosition> = FxHashSet::default();

        // 4 chunks en vertical (priorité 4) avant 4 chunks à l'horizontale (16).
        let candidates = [(4, 0, 0), (0, 4, 0)].map(|(x, y, z)| ChunkPosition::new(x, y, z));
        let missing = compute_missing_chunks_sorted(candidates.iter(), &existing, &in_flight, &ChunkPosition::new(0, 0, 0));

        assert_eq!(
            missing,
            [(0, 4, 0), (4, 0, 0)].map(|(x, y, z)| ChunkPosition::new(x, y, z)).to_vec(),
        );
    }

    #[test]
    fn test_compute_missing_is_deterministic() {
        let existing: FxHashSet<ChunkPosition> = FxHashSet::default();
        let in_flight: FxHashSet<ChunkPosition> = FxHashSet::default();

        let candidates = (0..6)
            .flat_map(|x| (0..6).map(move |z| ChunkPosition::new(x, 0, z)))
            .collect::<Vec<ChunkPosition>>();

        let first = compute_missing_chunks_sorted(candidates.iter(), &existing, &in_flight, &ChunkPosition::new(0, 0, 0));
        let second = compute_missing_chunks_sorted(candidates.iter(), &existing, &in_flight, &ChunkPosition::new(0, 0, 0));

        assert_eq!(first, second, "deux appels doivent produire le même ordre");
    }

    #[test]
    fn test_compute_missing_reprioritizes_when_player_moves() {
        let existing: FxHashSet<ChunkPosition> = FxHashSet::default();
        let in_flight: FxHashSet<ChunkPosition> = FxHashSet::default();

        let candidates = [(0, 0, 0), (1, 0, 0), (5, 0, 0)].map(|(x, y, z)| ChunkPosition::new(x, y, z));

        let before = compute_missing_chunks_sorted(candidates.iter(), &existing, &in_flight, &ChunkPosition::new(0, 0, 0));
        assert_eq!(
            before,
            [(0, 0, 0), (1, 0, 0), (5, 0, 0)]
                .map(|(x, y, z)| ChunkPosition::new(x, y, z))
                .to_vec()
        );

        // Après déplacement du joueur vers (5, 0, 0), la priorité change.
        let after = compute_missing_chunks_sorted(candidates.iter(), &existing, &in_flight, &ChunkPosition::new(5, 0, 0));
        assert_eq!(
            after,
            [(5, 0, 0), (1, 0, 0), (0, 0, 0)]
                .map(|(x, y, z)| ChunkPosition::new(x, y, z))
                .to_vec()
        );
    }
}
