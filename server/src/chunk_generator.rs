//! Générateur parallèle de chunks.
//!
//! Ce module utilise le WorkerPool custom (parallel.rs) pour générer
//! plusieurs chunks en parallèle sur plusieurs threads.

use shared::parallel::{Parallelizable, WorkerPool};
use shared::world::data::chunk::Chunk;
use shared::world::generation::chunk::ChunkWithChecksum;
use std::collections::HashMap;

/// Contexte passé à chaque worker thread.
/// Contient la seed nécessaire pour générer les chunks de façon déterministe.
#[derive(Clone)]
struct ChunkGeneratorContext {
    seed: u32,
}

/// Implémentation du trait Parallelizable pour la génération de chunks.
/// Chaque worker reçoit des coordonnées (cx, cy, cz) et retourne un chunk généré.
struct ChunkGenerator;

impl Parallelizable for ChunkGenerator {
    /// Type d'entrée: coordonnées du chunk (x, y, z)
    type Input = (i32, i32, i32);
    /// Type de sortie: chunk avec son checksum
    type Output = ChunkWithChecksum;
    /// Contexte: seed pour la génération déterministe
    type Context = ChunkGeneratorContext;

    /// Génère un chunk à partir des coordonnées et du contexte (seed).
    fn process(input: Self::Input, ctx: &Self::Context) -> Self::Output {
        let (cx, cy, cz) = input;
        let chunk = Chunk::generate(cx, cy, cz, ctx.seed);
        let checksum = chunk.compute_checksum();
        let chunk_data = shared::world::data::chunk::ChunkData::new(chunk);
        ChunkWithChecksum { chunk_data, checksum }
    }
}

/// Génère plusieurs chunks en parallèle.
///
/// # Arguments
/// * `seed` - Seed pour la génération déterministe du monde
/// * `coords` - Liste des coordonnées de chunks à générer
///
/// # Retour
/// HashMap associant les coordonnées (x, y, z) au chunk généré avec son checksum
pub fn generate_chunks_parallel(seed: u32, coords: Vec<(i32, i32, i32)>) -> HashMap<(i32, i32, i32), ChunkWithChecksum> {
    let mut result_map = HashMap::new();

    // Retour immédiat si rien à générer
    if coords.is_empty() {
        return result_map;
    }

    // Crée un pool de workers avec autant de threads que de CPUs
    let num_cpus = num_cpus::get();
    let pool = WorkerPool::<ChunkGenerator>::new(num_cpus, ChunkGeneratorContext { seed });

    // Soumet toutes les tâches de génération au pool
    for coord in &coords {
        let _ = pool.submit(*coord, *coord);
    }

    // Recolle les résultats au fur et à mesure
    let mut received = 0;
    while received < coords.len() {
        if let Some(result) = pool.try_recv() {
            result_map.insert(result.coords, result.output);
            received += 1;
        }
    }

    // Drop le pool pour attendre la fin des workers
    drop(pool);

    result_map
}
