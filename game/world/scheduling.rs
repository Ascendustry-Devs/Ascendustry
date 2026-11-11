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

    missing.sort_by_key(|coords| chunk_priority_sq(coords, player_chunk));
    missing
}
