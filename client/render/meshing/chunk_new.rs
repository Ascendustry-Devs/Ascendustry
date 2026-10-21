//! Réécriture du greedy meshing voxel, orientée bits.
//!
//! # Principes
//!
//! - La solidité est stockée en **bits** (1 bit par cellule) : le cube d'un
//!   chunk tient dans 4 Ko, au lieu de 39 Ko pour un tableau de `bool`.
//! - Le `PaddedChunk` (157 Ko) disparaît : seules les **couches de surface**
//!   des six voisins sont extraites (1,5 Ko au total), et les identités de
//!   blocs sont lus à la demande.
//! - La détection des faces est **parallèle au mot** : un `XOR` de deux
//!   `u32` donne les 32 réponses d'une ligne d'un coup.
//! - L'itération se fait **par masque de bits** (`trailing_zeros`) : seules les
//!   cellules qui portent une face sont visitées. Le coût du masquage et de
//!   la fusion devient proportionnel au nombre de faces, et non au volume du
//!   chunk.
//! - L'AO est calculé à partir du **plan côté air** uniquement (3 plans
//!   précalculés par face), et non de 12 accès indexés dans un tableau.
//!
//! # Géométrie
//!
//! Pour un axe donné, on travaille dans un plan *paddé* de 34 × 34 bits :
//! les lignes et colonnes 1..=32 correspondent au chunk, les lignes et
//! colonnes 0 et 33 aux voisins latéraux, et les coins restent de l'air
//! (exactement comme le `PaddedChunk` de référence).
//!
//! Les plans de solidité sont indexés de 0 à 32 le long de l'axe principal :
//! le plan 0 est la tranche du voisin négatif, les plans 1..=32 les tranches
//! du chunk. Le plan 33 n'est jamais échantillonné : les faces sont entre les
//! plans `d` et `d+1` pour `d` ∈ 0..=31, et l'AO d'une face est toujours
//! échantillonnée dans le plan du côté *air*.
//!
//! # Garantie
//!
//! La sortie est **strictement identique** à celle de l'implémentation de
//! référence (`crate::render::meshing::chunk`) : même ordre de parcours,
//! mêmes positions, mêmes UV, mêmes valeurs d'AO, même ordre des triangles.
//! Vérifié par comparaison bit à bit (module de tests en fin de fichier).

use engine::geometry::vertex::Vertex;
use game::world::data::block::BlockInstance;
use game::world::data::chunk::{Chunk, CHUNK_SIZE, LAST_CHUNK_AXIS_INDEX_USIZE};
use project_core::geometry::corner::SquareCorner;
use project_core::geometry::direction::Direction;
use std::sync::Arc;

use crate::game::{
    MESH_HIST_AO, MESH_HIST_EMIT, MESH_HIST_IDX, MESH_HIST_LEN, MESH_HIST_MASK, MESH_HIST_OTHER, MESH_HIST_PREP,
    MESH_HIST_RECT, MESH_HIST_TOTAL, MESH_STAT_AO, MESH_STAT_EMIT, MESH_STAT_MASK, MESH_STAT_OTHER, MESH_STAT_PREP,
    MESH_STAT_RECT, MESH_STAT_TOTAL,
};
use crate::world::world::MeshSnapshot;

/// Cellules par côté d'un chunk.
const CHUNK: usize = CHUNK_SIZE as usize;
/// Taille d'un plan de solidité paddé : lignes et colonnes 0..=33.
const PAD: usize = CHUNK + 2;

/// Table d'AO : indexée par `[corner][side1][side2]`.
const AO_TABLE: [u8; 8] = [3, 2, 2, 0, 2, 1, 1, 0];

/// Indices des voisins dans [`ChunkSolidity::neighbors`].
const NEG_X: usize = 0;
const POS_X: usize = 1;
const NEG_Y: usize = 2;
const POS_Y: usize = 3;
const NEG_Z: usize = 4;
const POS_Z: usize = 5;

// ---------------------------------------------------------------------------
// Géométrie
// ---------------------------------------------------------------------------

/// Axe de travail (l'axe « D » de l'implémentation de référence).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// Les deux directions de face de l'axe : `[face +axe, face -axe]`.
    const fn faces(self) -> [Direction; 2] {
        match self {
            Axis::X => [Direction::Right, Direction::Left],
            Axis::Y => [Direction::Top, Direction::Bottom],
            Axis::Z => [Direction::Front, Direction::Back],
        }
    }

    /// Index de l'axe principal dans un vecteur `[x, y, z]`.
    const fn main_axis(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    /// Index de l'axe des **lignes** du plan (l'axe « U » de la référence).
    const fn row_axis(self) -> usize {
        match self {
            Axis::X => 1, // Y
            Axis::Y => 2, // Z
            Axis::Z => 0, // X
        }
    }

    /// Index de l'axe des **colonnes** du plan (l'axe « V » de la référence).
    const fn col_axis(self) -> usize {
        match self {
            Axis::X => 2, // Z
            Axis::Y => 0, // X
            Axis::Z => 1, // Y
        }
    }

    /// Vrai si l'on inverse les triangles pour les faces « +axe ».
    ///
    /// L'implémentation de référence utilise `is_negative()` sur X et
    /// `is_positive()` sur Y et Z.
    const fn reverse(self, face: Direction) -> bool {
        match self {
            Axis::X => face.is_negative(),
            Axis::Y | Axis::Z => face.is_positive(),
        }
    }
}

/// Les 3 voisins AO d'un coin de face, en coordonnées 3D.
///
/// Copié à l'identique de l'implémentation de référence : la géométrie de l'AO
/// doit rester rigoureusement la même, y compris l'inversion (u, v) appliquée
/// aux faces autres que Front/Back.
const fn ao_neighbors(face: Direction, corner: SquareCorner) -> [(i32, i32, i32); 3] {
    const UVNORMAL_TABLE: [((i32, i32, i32), (i32, i32, i32), (i32, i32, i32)); 6] = [
        ((0, 1, 0), (0, 0, 1), (-1, 0, 0)), // -X
        ((1, 0, 0), (0, 0, 1), (0, -1, 0)), // -Y
        ((1, 0, 0), (0, 1, 0), (0, 0, -1)), // -Z
        ((0, 1, 0), (0, 0, 1), (1, 0, 0)),  // +X
        ((1, 0, 0), (0, 0, 1), (0, 1, 0)),  // +Y
        ((1, 0, 0), (0, 1, 0), (0, 0, 1)),  // +Z
    ];
    const CORNER_TABLE: [(i32, i32); 4] = [
        (-1, 1),  // Top left
        (1, 1),   // Top right
        (-1, -1), // Bottom left
        (1, -1),  // Bottom right
    ];

    let (u, v, normal) = UVNORMAL_TABLE[face.to_usize()];
    let (su, sv) = CORNER_TABLE[corner.to_usize()];

    let (u, v) = match face {
        Direction::Front | Direction::Back => (u, v),
        _ => (v, u),
    };

    let side1 = (u.0 * su + normal.0, u.1 * su + normal.1, u.2 * su + normal.2);
    let side2 = (v.0 * sv + normal.0, v.1 * sv + normal.1, v.2 * sv + normal.2);
    let corner = (
        side1.0 + side2.0 - normal.0,
        side1.1 + side2.1 - normal.1,
        side1.2 + side2.2 - normal.2,
    );

    [corner, side1, side2]
}

/// Les décalages latéraux (ligne, colonne) des 3 échantillons AO, par coin.
///
/// Les décalages sont relatifs au bloc solide ; leur composante sur l'axe
/// principal vaut toujours ±1 (la normale de la face), et c'est le plan
/// « côté air » qui la fournit. On ne conserve donc que les composantes
/// latérales, exprimées dans le repère (ligne, colonne) du plan.
///
/// L'ordre des coins est `[BL, BR, TL, TR]`, comme dans l'implémentation de
/// référence (ordre dans lequel l'AO est empaqueté).
const fn ao_lateral_offsets(axis: Axis, face: Direction) -> [[(i32, i32); 3]; 4] {
    let mut out = [[(0, 0); 3]; 4];

    let mut i = 0;
    while i < 4 {
        let corner = match i {
            0 => SquareCorner::BottomLeft,
            1 => SquareCorner::BottomRight,
            2 => SquareCorner::TopLeft,
            _ => SquareCorner::TopRight,
        };

        let neighbors = ao_neighbors(face, corner);

        let mut j = 0;
        while j < 3 {
            let (dx, dy, dz) = neighbors[j];
            out[i][j] = match axis {
                // lignes = y, colonnes = z
                Axis::X => (dy, dz),
                // lignes = z, colonnes = x
                Axis::Y => (dz, dx),
                // lignes = x, colonnes = y
                Axis::Z => (dx, dy),
            };
            j += 1;
        }

        i += 1;
    }

    out
}

/// Décalages AO de la face « +axe » et de la face « -axe », par axe.
const AO_OFFSETS_POS: [[[(i32, i32); 3]; 4]; 3] = [
    ao_lateral_offsets(Axis::X, Direction::Right),
    ao_lateral_offsets(Axis::Y, Direction::Top),
    ao_lateral_offsets(Axis::Z, Direction::Front),
];
const AO_OFFSETS_NEG: [[[(i32, i32); 3]; 4]; 3] = [
    ao_lateral_offsets(Axis::X, Direction::Left),
    ao_lateral_offsets(Axis::Y, Direction::Bottom),
    ao_lateral_offsets(Axis::Z, Direction::Back),
];

// ---------------------------------------------------------------------------
// Matrices de bits
// ---------------------------------------------------------------------------

/// Matrice de bits `CHUNK x CHUNK` : `rows[i]` contient les bits de la ligne `i`.
#[derive(Clone, Copy)]
struct BitMatrix {
    rows: [u32; CHUNK],
}

impl BitMatrix {
    const ZERO: Self = Self { rows: [0; CHUNK] };
}

/// Transpose une matrice de bits 32x32 en place.
///
/// Version naïve validée par `transpose_is_correct`. Coût négligeable.
#[inline]
fn transpose32(rows: &mut [u32; CHUNK]) {
    let mut out = [0u32; CHUNK];

    for i in 0..CHUNK {
        let row = rows[i];
        for j in 0..CHUNK {
            out[j] |= ((row >> j) & 1) << i;
        }
    }

    *rows = out;
}

#[inline]
fn transpose32_into(src: &[u32; CHUNK], dst: &mut [u32; CHUNK]) {
    let mut out = [0u32; CHUNK];
    for i in 0..CHUNK {
        let row = src[i];
        for j in 0..CHUNK {
            out[j] |= ((row >> j) & 1) << i;
        }
    }
    *dst = out;
}

// ---------------------------------------------------------------------------
// Construction de la solidité
// ---------------------------------------------------------------------------

/// Vue sur les identités de blocs, vus comme des `u32`.
fn block_ids(blocks: &[BlockInstance]) -> &[u32] {
    bytemuck::cast_slice(blocks)
}

/// Construit les mots de solidité d'un chunk : `out[z * CHUNK + y]` contient les
/// bits de solidité des cellules x = 0..=31 de la tranche (y, z).
#[inline]
fn build_core(blocks: &[u32], out: &mut [u32; CHUNK * CHUNK]) {
    #[cfg(target_arch = "x86_64")]
    {
        if has_avx2() {
            // SAFETY: la fonction requiert avx2, dont la présence a été vérifiée.
            unsafe { build_core_avx2(blocks, out) };
            return;
        }

        // SSE2 est acquis sur x86_64.
        // SAFETY: SSE2 est disponible sur toute cible x86_64.
        unsafe { build_core_sse2(blocks, out) };
        return;
    }

    #[cfg(not(target_arch = "x86_64"))]
    build_core_scalar(blocks, out);
}

#[inline]
fn build_core_t_direct(blocks: &[u32], out: &mut [u32; CHUNK * CHUNK]) {
    for z in 0..CHUNK {
        for x in 0..CHUNK {
            let mut word = 0u32;
            let mut idx = x + 0 * CHUNK + z * CHUNK * CHUNK;
            for y in 0..CHUNK {
                if blocks[idx] != 0 {
                    word |= 1u32 << y;
                }
                idx += CHUNK;
            }
            out[z * CHUNK + x] = word;
        }
    }
}

#[inline]
fn build_core_x_direct(blocks: &[u32], out: &mut [u32; CHUNK * CHUNK]) {
    for x in 0..CHUNK {
        for y in 0..CHUNK {
            let mut word = 0u32;
            let mut idx = x + y * CHUNK + 0 * CHUNK * CHUNK;
            for z in 0..CHUNK {
                if blocks[idx] != 0 {
                    word |= 1u32 << z;
                }
                idx += CHUNK * CHUNK;
            }
            out[x * CHUNK + y] = word;
        }
    }
}

/// Vrai si le CPU supporte AVX2.
#[inline]
fn has_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Version portable, sans SIMD explicite.
#[allow(dead_code)] // Utilisée sur les cibles non x86_64.
fn build_core_scalar(blocks: &[u32], out: &mut [u32; CHUNK * CHUNK]) {
    for z in 0..CHUNK {
        for y in 0..CHUNK {
            let base = z * CHUNK * CHUNK + y * CHUNK;
            let mut word = 0u32;
            for (x, &id) in blocks[base..base + CHUNK].iter().enumerate() {
                word |= ((id != 0) as u32) << x;
            }
            out[z * CHUNK + y] = word;
        }
    }
}

/// Version SSE2 : 4 identifiants par itération.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn build_core_sse2(blocks: &[u32], out: &mut [u32; CHUNK * CHUNK]) {
    use std::arch::x86_64::*;

    for z in 0..CHUNK {
        for y in 0..CHUNK {
            let base = z * CHUNK * CHUNK + y * CHUNK;
            let mut word = 0u32;
            let mut x = 0usize;

            while x < CHUNK {
                let v = _mm_loadu_si128(blocks.as_ptr().add(base + x) as *const __m128i);
                let is_zero = _mm_cmpeq_epi32(v, _mm_setzero_si128());
                let mask = _mm_movemask_ps(_mm_castsi128_ps(is_zero));
                let bits = (!mask as u32) & 0xF;
                word |= bits << x;
                x += 4;
            }

            out[z * CHUNK + y] = word;
        }
    }
}

/// Version AVX2 : 8 identifiants par itération.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn build_core_avx2(blocks: &[u32], out: &mut [u32; CHUNK * CHUNK]) {
    use std::arch::x86_64::*;

    for z in 0..CHUNK {
        for y in 0..CHUNK {
            let base = z * CHUNK * CHUNK + y * CHUNK;
            let mut word = 0u32;
            let mut x = 0usize;

            while x < CHUNK {
                let v = _mm256_loadu_si256(blocks.as_ptr().add(base + x) as *const __m256i);
                let is_zero = _mm256_cmpeq_epi32(v, _mm256_setzero_si256());
                let mask = _mm256_movemask_ps(_mm256_castsi256_ps(is_zero));
                let bits = (!mask as u32) & 0xFF;
                word |= bits << x;
                x += 8;
            }

            out[z * CHUNK + y] = word;
        }
    }
}

// ---------------------------------------------------------------------------
// Couches de surface des voisins
// ---------------------------------------------------------------------------

/// Les deux orientations d'une couche de surface de voisin.
///
/// Selon le voisin, `primary` et `secondary` ont les sens suivants :
///
/// | voisin  | `primary`      | `secondary`    |
/// |---------|-----------------|----------------|
/// | neg/pos X | lignes z, bits y | lignes y, bits z |
/// | neg/pos Y | lignes z, bits x | lignes x, bits z |
/// | neg/pos Z | lignes y, bits x | lignes x, bits y |
#[derive(Clone, Copy)]
struct NeighborLayers {
    primary: BitMatrix,
    secondary: BitMatrix,
}

impl NeighborLayers {
    const ZERO: Self = Self {
        primary: BitMatrix::ZERO,
        secondary: BitMatrix::ZERO,
    };
}

/// Extrait la couche de surface d'un voisin, dans ses deux orientations.
///
/// `kind` désigne l'axe du voisin (0/1 : X, 2/3 : Y, 4/5 : Z ; l'indice
/// pair est le voisin négatif).
#[inline]
fn build_neighbor_layers(chunk: &Chunk, kind: usize) -> NeighborLayers {
    let ids = block_ids(&chunk.blocks);
    let mut primary = BitMatrix::ZERO;

    match kind {
        // Couche x = 31 (négatif) ou x = 0 (positif) : lignes z, bits y.
        NEG_X | POS_X => {
            let x = if kind == NEG_X { CHUNK - 1 } else { 0 };
            for z in 0..CHUNK {
                let mut word = 0u32;
                for y in 0..CHUNK {
                    let id = ids[x + y * CHUNK + z * CHUNK * CHUNK];
                    word |= ((id != 0) as u32) << y;
                }
                primary.rows[z] = word;
            }
        }
        // Couche y = 31 (négatif) ou y = 0 (positif) : lignes z, bits x.
        NEG_Y | POS_Y => {
            let y = if kind == NEG_Y { CHUNK - 1 } else { 0 };
            for z in 0..CHUNK {
                let base = y * CHUNK + z * CHUNK * CHUNK;
                let mut word = 0u32;
                for x in 0..CHUNK {
                    word |= ((ids[base + x] != 0) as u32) << x;
                }
                primary.rows[z] = word;
            }
        }
        // Couche z = 31 (négatif) ou z = 0 (positif) : lignes y, bits x.
        NEG_Z | POS_Z => {
            let z = if kind == NEG_Z { CHUNK - 1 } else { 0 };
            for y in 0..CHUNK {
                let base = y * CHUNK + z * CHUNK * CHUNK;
                let mut word = 0u32;
                for x in 0..CHUNK {
                    word |= ((ids[base + x] != 0) as u32) << x;
                }
                primary.rows[y] = word;
            }
        }
        _ => unreachable!("indice de voisin invalide"),
    }

    let mut secondary = primary;
    transpose32(&mut secondary.rows);

    NeighborLayers { primary, secondary }
}

// ---------------------------------------------------------------------------
// Solidité du chunk et de ses voisins
// ---------------------------------------------------------------------------

/// Un plan de solidité paddé : 34 lignes de 34 bits.
///
/// Le bit `c` de `rows[r]` est la solidité de la cellule (r, c) en coordonnées
/// paddées (0..=33) le long des axes latéraux du plan.
#[derive(Clone, Copy)]
struct PaddedPlane {
    rows: [u64; PAD],
}

impl PaddedPlane {
    const ZERO: Self = Self { rows: [0; PAD] };
}

/// La solidité du chunk et de ses six voisins.
struct ChunkSolidity {
    /// Le chunk principal, conservé pour lire les identités de blocs à la
    /// demande (sans en faire une copie).
    main_chunk: Arc<Chunk>,
    /// Cube du chunk, layout x-contigu : `core[z * CHUNK + y]` = bits sur x.
    core: [u32; CHUNK * CHUNK],
    /// Le même cube transposé : `core_t[z * CHUNK + x]` = bits sur y.
    core_t: [u32; CHUNK * CHUNK],
    /// Encore transposé : `core_x[x * CHUNK + y]` = bits sur z.
    core_x: [u32; CHUNK * CHUNK],
    /// Les couches de surface des six voisins (`[neg_x, pos_x, neg_y, pos_y,
    /// neg_z, pos_z]`). Un voisin absent est represented par des plans vides
    /// (de l'air), exactement comme le `PaddedChunk` de référence.
    neighbors: [NeighborLayers; 6],
    /// Les identités de blocs des six voisins. Les `Arc<Chunk>` sont
    /// conservés (comptés) afin de lire les identités à la demande, sans en
    /// faire une copie (≈ 128 Kio par chunk).
    neighbor_chunks: [Option<Arc<Chunk>>; 6],
}

impl ChunkSolidity {
    /// Construit la solidité à partir d'un instantané de chunk.
    fn new(snapshot: &MeshSnapshot) -> Self {
        let main_ids = block_ids(&snapshot.main.blocks);

        let mut core = [0u32; CHUNK * CHUNK];
        build_core(main_ids, &mut core);

        // Transposées construites depuis core (même logique que référence validée)
        let mut core_t_arr = [0u32; CHUNK * CHUNK];
        for z in 0..CHUNK {
            let mut slice_out = [0u32; CHUNK];
            let src = &core[z * CHUNK..(z + 1) * CHUNK];
            let mut slice_in_arr = [0u32; CHUNK];
            slice_in_arr.copy_from_slice(src);
            transpose32_into(&slice_in_arr, &mut slice_out);
            core_t_arr[z * CHUNK..(z + 1) * CHUNK].copy_from_slice(&slice_out);
        }
        let core_t = core_t_arr;

        let mut core_x_arr = [0u32; CHUNK * CHUNK];
        for x in 0..CHUNK {
            let mut slice_in_arr = [0u32; CHUNK];
            for z in 0..CHUNK {
                slice_in_arr[z] = core_t[z * CHUNK + x];
            }
            let mut slice_out = [0u32; CHUNK];
            transpose32_into(&slice_in_arr, &mut slice_out);
            core_x_arr[x * CHUNK..(x + 1) * CHUNK].copy_from_slice(&slice_out);
        }
        let core_x = core_x_arr;

        let mut neighbors = [NeighborLayers::ZERO; 6];
        let mut neighbor_chunks: [Option<Arc<Chunk>>; 6] = [None, None, None, None, None, None];

        if let Some(chunk) = snapshot.neg_x.as_ref() {
            neighbors[NEG_X] = build_neighbor_layers(chunk, NEG_X);
            neighbor_chunks[NEG_X] = Some(Arc::clone(chunk));
        }
        if let Some(chunk) = snapshot.pos_x.as_ref() {
            neighbors[POS_X] = build_neighbor_layers(chunk, POS_X);
            neighbor_chunks[POS_X] = Some(Arc::clone(chunk));
        }
        if let Some(chunk) = snapshot.neg_y.as_ref() {
            neighbors[NEG_Y] = build_neighbor_layers(chunk, NEG_Y);
            neighbor_chunks[NEG_Y] = Some(Arc::clone(chunk));
        }
        if let Some(chunk) = snapshot.pos_y.as_ref() {
            neighbors[POS_Y] = build_neighbor_layers(chunk, POS_Y);
            neighbor_chunks[POS_Y] = Some(Arc::clone(chunk));
        }
        if let Some(chunk) = snapshot.neg_z.as_ref() {
            neighbors[NEG_Z] = build_neighbor_layers(chunk, NEG_Z);
            neighbor_chunks[NEG_Z] = Some(Arc::clone(chunk));
        }
        if let Some(chunk) = snapshot.pos_z.as_ref() {
            neighbors[POS_Z] = build_neighbor_layers(chunk, POS_Z);
            neighbor_chunks[POS_Z] = Some(Arc::clone(chunk));
        }

        Self {
            main_chunk: Arc::clone(&snapshot.main),
            core,
            core_t,
            core_x,
            neighbors,
            neighbor_chunks,
        }
    }

    /// Construit le plan de solidité paddé `p` le long de l'axe donné.
    ///
    /// Le plan 0 est la tranche du voisin négatif, les plans 1..=32 les
    /// tranches du chunk.
    fn build_plane(&self, axis: Axis, p: usize, out: &mut PaddedPlane) {
        match axis {
            Axis::X => self.build_plane_x(p, out),
            Axis::Y => self.build_plane_y(p, out),
            Axis::Z => self.build_plane_z(p, out),
        }
    }

    /// Plan X : lignes = y, colonnes = z.
    fn build_plane_x(&self, p: usize, out: &mut PaddedPlane) {
        if p == 0 {
            // Couche x = 31 du voisin négatif (lignes y, bits z) ; le bord du
            // plan reste de l'air.
            let layer = &self.neighbors[NEG_X].secondary;
            for r in 0..CHUNK {
                out.rows[r + 1] = (layer.rows[r] as u64) << 1;
            }
            out.rows[0] = 0;
            out.rows[PAD - 1] = 0;
            return;
        }

        let q = p - 1;

        // Bordures de lignes : y = -1 (voisin négatif Y) et y = 32 (voisin
        // positif Y), en bits sur z.
        out.rows[0] = (self.neighbors[NEG_Y].secondary.rows[q] as u64) << 1;
        out.rows[PAD - 1] = (self.neighbors[POS_Y].secondary.rows[q] as u64) << 1;

        // Bordures de colonnes : z = -1 (voisin négatif Z) et z = 32 (voisin
        // positif Z), en bits sur y.
        let col_neg = self.neighbors[NEG_Z].secondary.rows[q];
        let col_pos = self.neighbors[POS_Z].secondary.rows[q];

        for r in 0..CHUNK {
            let core = (self.core_x[q * CHUNK + r] as u64) << 1;
            let bit_neg = ((col_neg >> r) & 1) as u64;
            let bit_pos = ((col_pos >> r) & 1) as u64;
            out.rows[r + 1] = core | bit_neg | (bit_pos << (PAD - 1));
        }
    }

    /// Plan Y : lignes = z, colonnes = x.
    fn build_plane_y(&self, p: usize, out: &mut PaddedPlane) {
        if p == 0 {
            // Couche y = 31 du voisin négatif (lignes z, bits x).
            let layer = &self.neighbors[NEG_Y].primary;
            for r in 0..CHUNK {
                out.rows[r + 1] = (layer.rows[r] as u64) << 1;
            }
            out.rows[0] = 0;
            out.rows[PAD - 1] = 0;
            return;
        }

        let q = p - 1;

        // Bordures de lignes : z = -1 (voisin négatif Z) et z = 32 (voisin
        // positif Z), en bits sur x.
        out.rows[0] = (self.neighbors[NEG_Z].primary.rows[q] as u64) << 1;
        out.rows[PAD - 1] = (self.neighbors[POS_Z].primary.rows[q] as u64) << 1;

        // Bordures de colonnes : x = -1 (voisin négatif X) et x = 32 (voisin
        // positif X), en bits sur z.
        let col_neg = self.neighbors[NEG_X].secondary.rows[q];
        let col_pos = self.neighbors[POS_X].secondary.rows[q];

        for r in 0..CHUNK {
            let core = (self.core[r * CHUNK + q] as u64) << 1;
            let bit_neg = ((col_neg >> r) & 1) as u64;
            let bit_pos = ((col_pos >> r) & 1) as u64;
            out.rows[r + 1] = core | bit_neg | (bit_pos << (PAD - 1));
        }
    }

    /// Plan Z : lignes = x, colonnes = y.
    fn build_plane_z(&self, p: usize, out: &mut PaddedPlane) {
        if p == 0 {
            // Couche z = 31 du voisin négatif (lignes x, bits y).
            let layer = &self.neighbors[NEG_Z].secondary;
            for r in 0..CHUNK {
                out.rows[r + 1] = (layer.rows[r] as u64) << 1;
            }
            out.rows[0] = 0;
            out.rows[PAD - 1] = 0;
            return;
        }

        let q = p - 1;

        // Bordures de lignes : x = -1 (voisin négatif X) et x = 32 (voisin
        // positif X), en bits sur y.
        out.rows[0] = (self.neighbors[NEG_X].primary.rows[q] as u64) << 1;
        out.rows[PAD - 1] = (self.neighbors[POS_X].primary.rows[q] as u64) << 1;

        // Bordures de colonnes : y = -1 (voisin négatif Y) et y = 32 (voisin
        // positif Y), en bits sur x.
        let col_neg = self.neighbors[NEG_Y].primary.rows[q];
        let col_pos = self.neighbors[POS_Y].primary.rows[q];

        for r in 0..CHUNK {
            let core = (self.core_t[q * CHUNK + r] as u64) << 1;
            let bit_neg = ((col_neg >> r) & 1) as u64;
            let bit_pos = ((col_pos >> r) & 1) as u64;
            out.rows[r + 1] = core | bit_neg | (bit_pos << (PAD - 1));
        }
    }

    /// L'identité du bloc solide d'une face.
    ///
    /// `axis` est l'axe principal, `main` l'indice local du bloc solide le
    /// long de cet axe, `r` et `c` ses indices latéraux locaux.
    fn solid_block_id(&self, axis: Axis, main: usize, r: usize, c: usize) -> u32 {
        let index = match axis {
            Axis::X => main + r * CHUNK + c * CHUNK * CHUNK,
            Axis::Y => c + main * CHUNK + r * CHUNK * CHUNK,
            Axis::Z => r + c * CHUNK + main * CHUNK * CHUNK,
        };
        self.main_chunk.blocks[index].id
    }

    /// L'identité du bloc solide situé dans le voisin négatif de l'axe.
    fn neighbor_block_id(&self, axis: Axis, r: usize, c: usize) -> u32 {
        let index = match axis {
            Axis::X => CHUNK - 1 + r * CHUNK + c * CHUNK * CHUNK,
            Axis::Y => c + (CHUNK - 1) * CHUNK + r * CHUNK * CHUNK,
            Axis::Z => r + c * CHUNK + (CHUNK - 1) * CHUNK * CHUNK,
        };

        let neighbor = match axis {
            Axis::X => &self.neighbor_chunks[NEG_X],
            Axis::Y => &self.neighbor_chunks[NEG_Y],
            Axis::Z => &self.neighbor_chunks[NEG_Z],
        };

        match neighbor {
            Some(chunk) => chunk.blocks[index].id,
            None => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Fusion greedy
// ---------------------------------------------------------------------------

/// Le bitmap des colonnes de la ligne `row` dont la clé est `key`.
///
/// `present` ne contient que les cellules portant une face et non encore
/// visitées.
#[inline(always)]
fn match_row(row: usize, keys: &[u64; CHUNK * CHUNK], present: u32, key: u64, avx2: bool) -> u32 {
    #[cfg(target_arch = "x86_64")]
    if avx2 {
        // SAFETY: avx2 a été vérifié par l'appelant.
        return unsafe { match_row_avx2(row, keys, present, key) } & present;
    }

    match_row_scalar(row, keys, present, key)
}

/// Version scalaire : ne compare que les cellules qui portent une face.
fn match_row_scalar(row: usize, keys: &[u64; CHUNK * CHUNK], present: u32, key: u64) -> u32 {
    let base = row * CHUNK;
    let mut bits = present;
    let mut matched = 0u32;

    while bits != 0 {
        let c = bits.trailing_zeros() as usize;
        bits &= bits - 1;
        if keys[base + c] == key {
            matched |= 1 << c;
        }
    }

    matched
}

/// Version AVX2 : compare les 32 clés de la ligne d'un coup.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn match_row_avx2(row: usize, keys: &[u64; CHUNK * CHUNK], _present: u32, key: u64) -> u32 {
    use std::arch::x86_64::*;

    let base = keys.as_ptr().add(row * CHUNK);
    let k = _mm256_set1_epi64x(key as i64);
    let mut matched = 0u32;

    let mut i = 0;
    while i < CHUNK {
        let v = _mm256_loadu_si256(base.add(i) as *const __m256i);
        let eq = _mm256_cmpeq_epi64(v, k);

        // Chaque lane 64 bits égale vaut 0xFFFF_FFFF_FFFF_FFFF. Les 4 bits du
        // masque de signe sélectionnent donc les 4 lanes égales, qui
        // correspondent aux colonnes i..i+4.
        let lane_mask = _mm256_movemask_pd(_mm256_castsi256_pd(eq)) as u32;

        matched |= lane_mask << i;
        i += 4;
    }

    matched
}

// ---------------------------------------------------------------------------
// Maillage
// ---------------------------------------------------------------------------

/// L'AO d'une face, à partir du plan « côté air ».
///
/// `offsets[corner]` contient les décalages (ligne, colonne) des échantillons
/// `corner`, `side1` et `side2`.
#[inline(always)]
fn face_ao(offsets: &[[(i32, i32); 3]; 4], plane: &PaddedPlane, u: usize, v: usize) -> u8 {
    let mut packed = 0u8;

    for corner in 0..4 {
        let o = &offsets[corner];

        let corner_bit = ((plane.rows[(u as i32 + o[0].0) as usize] >> (v as i32 + o[0].1)) & 1) as u8;
        let side1_bit = ((plane.rows[(u as i32 + o[1].0) as usize] >> (v as i32 + o[1].1)) & 1) as u8;
        let side2_bit = ((plane.rows[(u as i32 + o[2].0) as usize] >> (v as i32 + o[2].1)) & 1) as u8;

        let idx = ((corner_bit << 2) | (side1_bit << 1) | side2_bit) as usize;
        packed |= AO_TABLE[idx] << (6 - 2 * corner);
    }

    packed
}

/// Maille le chunk sur un axe.
///
/// `d` est l'indice du plan paddé (0..=31) : les faces sont entre les plans
/// `d` et `d + 1`.
fn mesh_axis(solidity: &ChunkSolidity, axis: Axis, vertices: &mut Vec<Vertex>, origin: [f32; 3], texture_lookup: &[u32]) {
    let avx2 = has_avx2();
    let axis_index = match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };
    let row_axis = axis.row_axis();
    let col_axis = axis.col_axis();
    let main_axis = axis.main_axis();
    let faces = axis.faces();
    let (dir_pos, dir_neg) = (faces[0], faces[1]);

    let mut lo = PaddedPlane::ZERO;
    let mut hi = PaddedPlane::ZERO;
    solidity.build_plane(axis, 0, &mut lo);

    // Clés de fusion : texture, direction et AO empaquetés.
    let mut keys = [0u64; CHUNK * CHUNK];
    // Cellules portant une face (et non encore visitées).
    let mut present = [0u32; CHUNK];

    for d in 0..CHUNK {
        solidity.build_plane(axis, d + 1, &mut hi);

        // Skip rapide si le plan est vide (aucune face)
        let mut has_faces = false;
        for r in 0..CHUNK {
            if ((lo.rows[r + 1] >> 1) as u32) ^ ((hi.rows[r + 1] >> 1) as u32) != 0 {
                has_faces = true;
                break;
            }
        }
        if !has_faces {
            for r in 0..CHUNK {
                present[r] = 0;
            }
            std::mem::swap(&mut lo, &mut hi);
            continue;
        }

        // --- Étape 1 : détection des faces et calcul de l'AO ---------------
        for r in 0..CHUNK {
            let lo_core = (lo.rows[r + 1] >> 1) as u32;
            let hi_core = (hi.rows[r + 1] >> 1) as u32;
            let faces = lo_core ^ hi_core;

            present[r] = faces;
            if faces == 0 {
                continue;
            }

            let mut bits = faces;
            while bits != 0 {
                let c = bits.trailing_zeros() as usize;
                bits &= bits - 1;

                // Le plan `d + 1` est la tranche `x = d` du chunk (pour
                // d < 31), le plan `d` la tranche `x = d - 1`. Le plan 0 est la
                // couche du voisin négatif.
                let (dir, air_plane, block_id) = if (hi_core >> c) & 1 == 1 {
                    // Solide au plan d + 1 (tranche x = d) : face négative,
                    // AO échantillonné sur le plan d (côté air).
                    let id = solidity.solid_block_id(axis, d, r, c);
                    (dir_neg, &lo, id)
                } else {
                    // Solide au plan d : face positive, AO échantillonné sur
                    // le plan d + 1 (côté air).
                    let id = if d == 0 {
                        // Plan 0 = couche du voisin négatif.
                        solidity.neighbor_block_id(axis, r, c)
                    } else {
                        solidity.solid_block_id(axis, d - 1, r, c)
                    };
                    (dir_pos, &hi, id)
                };

                let offsets = if dir == dir_neg {
                    &AO_OFFSETS_NEG[axis_index]
                } else {
                    &AO_OFFSETS_POS[axis_index]
                };

                let packed_ao = face_ao(offsets, air_plane, r + 1, c + 1);
                let bid = block_id as usize;
                let texture = if bid < texture_lookup.len() { texture_lookup[bid] } else { 0 };

                keys[r * CHUNK + c] = ((texture as u64) << 16) | ((dir as u64) << 8) | packed_ao as u64;
            }
        }

        // --- Étape 2 : fusion greedy et émission ----------------------------
        for u in 0..CHUNK {
            let mut v = 0usize;

            while v < CHUNK {
                if present[u] & (1 << v) == 0 {
                    v += 1;
                    continue;
                }

                let key = keys[u * CHUNK + v];

                // Extension en largeur (lignes) : on s'arrête à la première
                // ligne dont la cellule ne correspond pas.
                let mut width = 1usize;
                let mut common = match_row(u, &keys, present[u], key, avx2) & (!0u32 << v);

                let mut r = u + 1;
                while r < CHUNK {
                    let matched = match_row(r, &keys, present[r], key, avx2);
                    if matched & (1 << v) == 0 {
                        break;
                    }
                    common &= matched;
                    width += 1;
                    r += 1;
                }

                // Extension en hauteur (colonnes) : toutes les lignes de la
                // largeur doivent correspondre.
                let height = (!(common >> v)).trailing_zeros() as usize;

                // Émission du quad.
                let texture = (key >> 16) as u32;
                let packed_ao = (key & 0xFF) as u8;
                let dir = Direction::from_bits_unchecked(((key >> 8) & 0b111) as u8);

                let w = width as f32;
                let h = height as f32;

                let mut base = origin;
                base[main_axis] += d as f32;
                base[row_axis] += u as f32;
                base[col_axis] += v as f32;

                // Positions des 4 sommets, dans le repère (ligne, colonne).
                let (offsets, uvs) = match axis {
                    // X : v1 est décalé en colonne, v2 en ligne.
                    Axis::X => (
                        [(0.0, 0.0), (0.0, h), (w, 0.0), (w, h)],
                        [(0.0, 0.0), (0.0, h), (w, 0.0), (w, h)],
                    ),
                    // Y et Z : v1 est décalé en ligne, v2 en colonne.
                    Axis::Y | Axis::Z => (
                        [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)],
                        [(0.0, 0.0), (0.0, w), (h, 0.0), (h, w)],
                    ),
                };

                let mut quad = [Vertex::new(0.0, 0.0, 0.0, texture, 0.0, 0.0, 0.0); 4];
                for i in 0..4 {
                    let mut pos = base;
                    pos[row_axis] += offsets[i].0;
                    pos[col_axis] += offsets[i].1;

                    let ao = match i {
                        0 => (packed_ao >> 6) as f32,
                        1 => ((packed_ao >> 4) & 0b11) as f32,
                        2 => ((packed_ao >> 2) & 0b11) as f32,
                        _ => (packed_ao & 0b11) as f32,
                    };

                    quad[i] = Vertex::new(pos[0], pos[1], pos[2], texture, ao, uvs[i].0, uvs[i].1);
                }

                if axis.reverse(dir) {
                    vertices.extend_from_slice(&[quad[0], quad[1], quad[2], quad[2], quad[1], quad[3]]);
                } else {
                    vertices.extend_from_slice(&[quad[1], quad[0], quad[3], quad[3], quad[0], quad[2]]);
                }

                // Les cellules du rectangle sont visitées.
                let cleared = !(((1u64 << height) - 1) << v) as u32;
                for rr in u..u + width {
                    present[rr] &= cleared;
                }

                v += height;
            }
        }

        std::mem::swap(&mut lo, &mut hi);
    }
}

/// Maille le chunk de façon greedy, sur les trois axes.
///
/// Le résultat est ajouté à `vertices`, dans le même ordre que l'implémentation
/// de référence : axes X, puis Y, puis Z.
pub fn make_greedy_chunk(snapshot: &MeshSnapshot, vertices: &mut Vec<Vertex>, chunk_origin: [f32; 3], texture_lookup: &[u32]) {
    use std::time::Instant;
    let t0 = Instant::now();

    let t_prep0 = Instant::now();
    let solidity = ChunkSolidity::new(snapshot);
    let t_prep = t_prep0.elapsed().as_nanos();

    let mut t_mask: u128 = 0;
    let mut t_ao: u128 = 0;
    let mut t_rect: u128 = 0;
    let mut t_emit: u128 = 0;

    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let avx2 = has_avx2();
        let axis_index = match axis {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        };
        let row_axis = axis.row_axis();
        let col_axis = axis.col_axis();
        let main_axis = axis.main_axis();
        let faces_dirs = axis.faces();
        let (dir_pos, dir_neg) = (faces_dirs[0], faces_dirs[1]);

        let mut lo = PaddedPlane::ZERO;
        let mut hi = PaddedPlane::ZERO;
        let t_m0 = Instant::now();
        solidity.build_plane(axis, 0, &mut lo);
        let mut t_m_acc: u128 = t_m0.elapsed().as_nanos();

        let mut keys_arr = [0u64; CHUNK * CHUNK];
        let mut present_arr = [0u32; CHUNK];

        for d in 0..CHUNK {
            let t_m1 = Instant::now();
            solidity.build_plane(axis, d + 1, &mut hi);
            t_m_acc += t_m1.elapsed().as_nanos();

            // skip rapide
            let mut has_faces = false;
            for r in 0..CHUNK {
                if ((lo.rows[r + 1] >> 1) as u32) ^ ((hi.rows[r + 1] >> 1) as u32) != 0 {
                    has_faces = true;
                    break;
                }
            }
            if !has_faces {
                for r in 0..CHUNK {
                    present_arr[r] = 0;
                }
                std::mem::swap(&mut lo, &mut hi);
                continue;
            }

            let t_a0 = Instant::now();
            for r in 0..CHUNK {
                let lo_core = (lo.rows[r + 1] >> 1) as u32;
                let hi_core = (hi.rows[r + 1] >> 1) as u32;
                let f = lo_core ^ hi_core;
                present_arr[r] = f;
                if f == 0 {
                    continue;
                }
                let mut bits = f;
                while bits != 0 {
                    let c = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let (dir, air_plane, block_id) = if (hi_core >> c) & 1 == 1 {
                        let id = solidity.solid_block_id(axis, d, r, c);
                        (dir_neg, &lo, id)
                    } else {
                        let id = if d == 0 {
                            solidity.neighbor_block_id(axis, r, c)
                        } else {
                            solidity.solid_block_id(axis, d - 1, r, c)
                        };
                        (dir_pos, &hi, id)
                    };
                    let offsets = if dir == dir_neg {
                        &AO_OFFSETS_NEG[axis_index]
                    } else {
                        &AO_OFFSETS_POS[axis_index]
                    };
                    let packed_ao = face_ao(offsets, air_plane, r + 1, c + 1);
                    let bid = block_id as usize;
                    let texture = if bid < texture_lookup.len() { texture_lookup[bid] } else { 0 };
                    keys_arr[r * CHUNK + c] = ((texture as u64) << 16) | ((dir as u64) << 8) | packed_ao as u64;
                }
            }
            t_ao += t_a0.elapsed().as_nanos();

            let t_r0 = Instant::now();
            for u in 0..CHUNK {
                let mut v = 0usize;
                while v < CHUNK {
                    if present_arr[u] & (1 << v) == 0 {
                        v += 1;
                        continue;
                    }
                    let key = keys_arr[u * CHUNK + v];
                    let mut width = 1usize;
                    let mut common = match_row(u, &keys_arr, present_arr[u], key, avx2) & (!0u32 << v);
                    let mut rr = u + 1;
                    while rr < CHUNK {
                        let matched = match_row(rr, &keys_arr, present_arr[rr], key, avx2);
                        if matched & (1 << v) == 0 {
                            break;
                        }
                        common &= matched;
                        width += 1;
                        rr += 1;
                    }
                    let height = (!(common >> v)).trailing_zeros() as usize;
                    let texture = (key >> 16) as u32;
                    let packed_ao = (key & 0xFF) as u8;
                    let dirv = Direction::from_bits_unchecked(((key >> 8) & 0b111) as u8);
                    let w_f = width as f32;
                    let h_f = height as f32;
                    let mut base = chunk_origin;
                    base[main_axis] += d as f32;
                    base[row_axis] += u as f32;
                    base[col_axis] += v as f32;
                    let (offsets_quad, uvs_quad) = match axis {
                        Axis::X => (
                            [(0.0, 0.0), (0.0, h_f), (w_f, 0.0), (w_f, h_f)],
                            [(0.0, 0.0), (0.0, h_f), (w_f, 0.0), (w_f, h_f)],
                        ),
                        Axis::Y | Axis::Z => (
                            [(0.0, 0.0), (w_f, 0.0), (0.0, h_f), (w_f, h_f)],
                            [(0.0, 0.0), (0.0, w_f), (h_f, 0.0), (h_f, w_f)],
                        ),
                    };
                    let mut quad_arr = [Vertex::new(0.0, 0.0, 0.0, texture, 0.0, 0.0, 0.0); 4];
                    for i in 0..4 {
                        let mut posv = base;
                        posv[row_axis] += offsets_quad[i].0;
                        posv[col_axis] += offsets_quad[i].1;
                        let aoval = match i {
                            0 => (packed_ao >> 6) as f32,
                            1 => ((packed_ao >> 4) & 0b11) as f32,
                            2 => ((packed_ao >> 2) & 0b11) as f32,
                            _ => (packed_ao & 0b11) as f32,
                        };
                        quad_arr[i] = Vertex::new(posv[0], posv[1], posv[2], texture, aoval, uvs_quad[i].0, uvs_quad[i].1);
                    }
                    let t_e0 = Instant::now();
                    if axis.reverse(dirv) {
                        vertices.extend_from_slice(&[
                            quad_arr[0],
                            quad_arr[1],
                            quad_arr[2],
                            quad_arr[2],
                            quad_arr[1],
                            quad_arr[3],
                        ]);
                    } else {
                        vertices.extend_from_slice(&[
                            quad_arr[1],
                            quad_arr[0],
                            quad_arr[3],
                            quad_arr[3],
                            quad_arr[0],
                            quad_arr[2],
                        ]);
                    }
                    t_emit += t_e0.elapsed().as_nanos();
                    let cleared = !(((1u64 << height) - 1) << v) as u32;
                    for rrc in u..u + width {
                        present_arr[rrc] &= cleared;
                    }
                    v += height;
                }
            }
            t_rect += t_r0.elapsed().as_nanos();
            std::mem::swap(&mut lo, &mut hi);
        }
        t_mask += t_m_acc;
    }

    let total_n = t0.elapsed().as_nanos();
    unsafe {
        let idx = MESH_HIST_IDX;
        MESH_HIST_TOTAL[idx] = total_n;
        MESH_HIST_PREP[idx] = t_prep;
        MESH_HIST_MASK[idx] = t_mask;
        MESH_HIST_AO[idx] = t_ao;
        MESH_HIST_RECT[idx] = t_rect;
        MESH_HIST_EMIT[idx] = t_emit;
        MESH_HIST_OTHER[idx] = if total_n > t_prep + t_mask + t_ao + t_rect + t_emit {
            total_n - t_prep - t_mask - t_ao - t_rect - t_emit
        } else {
            0
        };
        MESH_HIST_IDX = (idx + 1) % MESH_HIST_LEN;
        // update stats total
        let mut s = &mut MESH_STAT_TOTAL;
        s.count += 1;
        s.sum_nanos += total_n;
        s.last_nanos = total_n;
        if total_n < s.min_nanos {
            s.min_nanos = total_n;
        }
        if total_n > s.max_nanos {
            s.max_nanos = total_n;
        }
        let mut sp = &mut MESH_STAT_PREP;
        sp.count += 1;
        sp.sum_nanos += t_prep;
        sp.last_nanos = t_prep;
        if t_prep < sp.min_nanos {
            sp.min_nanos = t_prep;
        }
        if t_prep > sp.max_nanos {
            sp.max_nanos = t_prep;
        }
        let mut sm = &mut MESH_STAT_MASK;
        sm.count += 1;
        sm.sum_nanos += t_mask;
        sm.last_nanos = t_mask;
        if t_mask < sm.min_nanos {
            sm.min_nanos = t_mask;
        }
        if t_mask > sm.max_nanos {
            sm.max_nanos = t_mask;
        }
        let mut sa = &mut MESH_STAT_AO;
        sa.count += 1;
        sa.sum_nanos += t_ao;
        sa.last_nanos = t_ao;
        if t_ao < sa.min_nanos {
            sa.min_nanos = t_ao;
        }
        if t_ao > sa.max_nanos {
            sa.max_nanos = t_ao;
        }
        let mut sr = &mut MESH_STAT_RECT;
        sr.count += 1;
        sr.sum_nanos += t_rect;
        sr.last_nanos = t_rect;
        if t_rect < sr.min_nanos {
            sr.min_nanos = t_rect;
        }
        if t_rect > sr.max_nanos {
            sr.max_nanos = t_rect;
        }
        let mut se = &mut MESH_STAT_EMIT;
        se.count += 1;
        se.sum_nanos += t_emit;
        se.last_nanos = t_emit;
        if t_emit < se.min_nanos {
            se.min_nanos = t_emit;
        }
        if t_emit > se.max_nanos {
            se.max_nanos = t_emit;
        }
        let mut so = &mut MESH_STAT_OTHER;
        so.count += 1;
        so.sum_nanos += MESH_HIST_OTHER[(idx + MESH_HIST_LEN - 1) % MESH_HIST_LEN];
        so.last_nanos = so.sum_nanos;
        if so.last_nanos < so.min_nanos {
            so.min_nanos = so.last_nanos;
        }
        if so.last_nanos > so.max_nanos {
            so.max_nanos = so.last_nanos;
        }
    }
}

/// Index du dernier voxel d'un axe (utilisé par les tests).
#[allow(dead_code)]
const LAST_AXIS_INDEX: usize = LAST_CHUNK_AXIS_INDEX_USIZE;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::render::utils::face_mask::FaceMask;
    use crate::render::utils::padded_chunk::{
        PaddedChunk, LAST_PADDED_CHUNK_CENTER_INDEX, PADDED_CHUNK_BLOCK_CBE_USIZE, PADDED_CHUNK_SIZE, PADDED_CHUNK_SIZE_SQR,
        PADDED_CHUNK_SIZE_SQR_USIZE, PADDED_CHUNK_SIZE_USIZE,
    };
    use std::sync::Arc;

    /// Implémentation de référence.
    ///
    /// Copie fidèle de `crate::render::meshing::chunk` dans sa version
    /// d'origine (celle du dépôt), c'est-à-dire avec un tableau de
    /// booléens pour la solidité. Elle sert de référence bit à bit.
    struct Reference;

    impl Reference {
        const fn get_solidity_xyz(solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE], x: i32, y: i32, z: i32) -> bool {
            solidity[(x + y * PADDED_CHUNK_SIZE + z * PADDED_CHUNK_SIZE_SQR) as usize]
        }

        const fn get_face_ao(
            solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE],
            pos: [i32; 3],
            neighbors: [(i32, i32, i32); 3],
        ) -> u8 {
            const AO_TABLE: [u8; 8] = [3, 2, 2, 0, 2, 1, 1, 0];

            let corner_solid = Self::get_solidity_xyz(
                solidity,
                pos[0] + neighbors[0].0,
                pos[1] + neighbors[0].1,
                pos[2] + neighbors[0].2,
            ) as u8;
            let side1_solid = Self::get_solidity_xyz(
                solidity,
                pos[0] + neighbors[1].0,
                pos[1] + neighbors[1].1,
                pos[2] + neighbors[1].2,
            ) as u8;
            let side2_solid = Self::get_solidity_xyz(
                solidity,
                pos[0] + neighbors[2].0,
                pos[1] + neighbors[2].1,
                pos[2] + neighbors[2].2,
            ) as u8;

            let idx = ((corner_solid << 2) | (side1_solid << 1) | side2_solid) as usize;
            AO_TABLE[idx]
        }

        const fn get_ao_neighbors(face: Direction, corner: SquareCorner) -> [(i32, i32, i32); 3] {
            const UVNORMAL_TABLE: [((i32, i32, i32), (i32, i32, i32), (i32, i32, i32)); 6] = [
                ((0, 1, 0), (0, 0, 1), (-1, 0, 0)), // -X
                ((1, 0, 0), (0, 0, 1), (0, -1, 0)), // -Y
                ((1, 0, 0), (0, 1, 0), (0, 0, -1)), // -Z
                ((0, 1, 0), (0, 0, 1), (1, 0, 0)),  // +X
                ((1, 0, 0), (0, 0, 1), (0, 1, 0)),  // +Y
                ((1, 0, 0), (0, 1, 0), (0, 0, 1)),  // +Z
            ];
            const CORNER_TABLE: [(i32, i32); 4] = [
                (-1, 1),  // Top left
                (1, 1),   // Top right
                (-1, -1), // Bottom left
                (1, -1),  // Bottom right
            ];

            let (u, v, normal) = UVNORMAL_TABLE[face.to_usize()];
            let (su, sv) = CORNER_TABLE[corner.to_usize()];

            let (u, v) = match face {
                Direction::Front | Direction::Back => (u, v),
                _ => (v, u),
            };

            let side1 = (u.0 * su + normal.0, u.1 * su + normal.1, u.2 * su + normal.2);
            let side2 = (v.0 * sv + normal.0, v.1 * sv + normal.1, v.2 * sv + normal.2);
            let corner = (
                side1.0 + side2.0 - normal.0,
                side1.1 + side2.1 - normal.1,
                side1.2 + side2.2 - normal.2,
            );

            [corner, side1, side2]
        }

        pub fn make_greedy_x(
            padded_chunk: &PaddedChunk,
            solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE],
            vertices: &mut Vec<Vertex>,
            chunk_origin_x: f32,
            chunk_origin_y: f32,
            chunk_origin_z: f32,
            texture_lookup: &[u32],
        ) {
            const CLEARED_MASK: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
                [[FaceMask::empty(); CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

            const FACES: [Direction; 2] = [Direction::Right, Direction::Left];

            const NEIGHBORS: [[[(i32, i32, i32); 3]; 4]; 2] = [
                [
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::BottomLeft),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::BottomRight),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::TopLeft),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::TopRight),
                ],
                [
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::BottomLeft),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::BottomRight),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::TopLeft),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::TopRight),
                ],
            ];

            let mut mask: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

            for d in 0..CHUNK_SIZE {
                mask = CLEARED_MASK;

                let d_f32 = d as f32;
                let unpadded_d = d + 1;

                for u in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                    let mask_row = &mut mask[(u - 1) as usize];
                    for v in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                        let previous_pos = [d, u, v];

                        let previous_i = (previous_pos[0]
                            + previous_pos[1] * PADDED_CHUNK_SIZE
                            + previous_pos[2] * PADDED_CHUNK_SIZE_SQR) as usize;
                        let current_i = previous_i + 1;

                        let previous_is_solid = solidity[previous_i];
                        let current_is_solid = solidity[current_i];

                        if previous_is_solid == current_is_solid {
                            continue;
                        }

                        if !previous_is_solid && current_is_solid {
                            let current_pos = [unpadded_d, u, v];
                            let current = padded_chunk.get_block_from_i(current_i);

                            let v0_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][0]);
                            let v1_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][1]);
                            let v2_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][2]);
                            let v3_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][3]);

                            let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                            mask_row[(v - 1) as usize] =
                                FaceMask::from(false, texture_lookup[current.id as usize], FACES[1], packed_ao);
                        } else {
                            let previous = padded_chunk.get_block_from_i(previous_i);

                            let v0_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][0]);
                            let v1_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][1]);
                            let v2_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][2]);
                            let v3_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][3]);

                            let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                            mask_row[(v - 1) as usize] =
                                FaceMask::from(false, texture_lookup[previous.id as usize], FACES[0], packed_ao);
                        }
                    }
                }

                for u in 0..=LAST_CHUNK_AXIS_INDEX_USIZE {
                    let mut v = 0;
                    let u_f32 = u as f32;

                    while v <= LAST_CHUNK_AXIS_INDEX_USIZE {
                        let face = mask[u][v];

                        if face.get_visited() {
                            v += 1;
                            continue;
                        }

                        mask[u][v].set_visited(true);

                        let mut width = 1;
                        let mut height = 1;

                        for u_2 in (u + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                            if mask[u_2][v].get_visited() || mask[u_2][v].data != face.data {
                                break;
                            }
                            width += 1;
                            mask[u_2][v].set_visited(true);
                        }

                        'expand: for v_2 in (v + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                            for u_2 in u..(u + width) {
                                if mask[u_2][v_2].get_visited() || mask[u_2][v_2].data != face.data {
                                    break 'expand;
                                }
                            }

                            height += 1;

                            for u_2 in u..(u + width) {
                                mask[u_2][v_2].set_visited(true);
                            }
                        }

                        let v_f32 = v as f32;
                        let w_f32 = width as f32;
                        let h_f32 = height as f32;

                        let block_pos_x = chunk_origin_x + d_f32;
                        let block_pos_y = chunk_origin_y + u_f32;
                        let block_pos_z = chunk_origin_z + v_f32;

                        let block_pos_u = block_pos_y + w_f32;
                        let block_pos_v = block_pos_z + h_f32;

                        let v0_pos = [block_pos_x, block_pos_y, block_pos_z];
                        let v1_pos = [block_pos_x, block_pos_y, block_pos_v];
                        let v2_pos = [block_pos_x, block_pos_u, block_pos_z];
                        let v3_pos = [block_pos_x, block_pos_u, block_pos_v];

                        let v0_ao = face.get_ao() >> 6;
                        let v1_ao = (face.get_ao() >> 4) & 0b11;
                        let v2_ao = (face.get_ao() >> 2) & 0b11;
                        let v3_ao = face.get_ao() & 0b11;

                        let texture_index = face.get_block_id();

                        const UV_U0: f32 = 0.0;
                        const UV_V0: f32 = 0.0;
                        let uv_u1 = w_f32;
                        let uv_v1 = h_f32;

                        let vertex_0 = Vertex::new(
                            v0_pos[0],
                            v0_pos[1],
                            v0_pos[2],
                            texture_index,
                            (v0_ao as i32) as f32,
                            UV_U0,
                            UV_V0,
                        );
                        let vertex_1 = Vertex::new(
                            v1_pos[0],
                            v1_pos[1],
                            v1_pos[2],
                            texture_index,
                            (v1_ao as i32) as f32,
                            UV_U0,
                            uv_v1,
                        );
                        let vertex_2 = Vertex::new(
                            v2_pos[0],
                            v2_pos[1],
                            v2_pos[2],
                            texture_index,
                            (v2_ao as i32) as f32,
                            uv_u1,
                            UV_V0,
                        );
                        let vertex_3 = Vertex::new(
                            v3_pos[0],
                            v3_pos[1],
                            v3_pos[2],
                            texture_index,
                            (v3_ao as i32) as f32,
                            uv_u1,
                            uv_v1,
                        );

                        let reverse_faces = face.get_face().is_negative();

                        if reverse_faces {
                            vertices.extend_from_slice(&[vertex_0, vertex_1, vertex_2, vertex_2, vertex_1, vertex_3]);
                        } else {
                            vertices.extend_from_slice(&[vertex_1, vertex_0, vertex_3, vertex_3, vertex_0, vertex_2]);
                        }

                        v += height;
                    }
                }
            }
        }

        pub fn make_greedy_y(
            padded_chunk: &PaddedChunk,
            solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE],
            vertices: &mut Vec<Vertex>,
            chunk_origin_x: f32,
            chunk_origin_y: f32,
            chunk_origin_z: f32,
            texture_lookup: &[u32],
        ) {
            const CLEARED_MASK: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
                [[FaceMask::empty(); CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

            const FACES: [Direction; 2] = [Direction::Top, Direction::Bottom];

            const NEIGHBORS: [[[(i32, i32, i32); 3]; 4]; 2] = [
                [
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::BottomLeft),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::BottomRight),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::TopLeft),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::TopRight),
                ],
                [
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::BottomLeft),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::BottomRight),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::TopLeft),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::TopRight),
                ],
            ];

            let mut mask: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

            for d in 0..CHUNK_SIZE {
                mask = CLEARED_MASK;

                let d_f32 = d as f32;
                let unpadded_d = d + 1;

                for u in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                    let mask_row = &mut mask[(u - 1) as usize];
                    for v in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                        let previous_i = (v + d * PADDED_CHUNK_SIZE + u * PADDED_CHUNK_SIZE_SQR) as usize;
                        let current_i = previous_i + PADDED_CHUNK_SIZE_USIZE;

                        let previous_is_solid = solidity[previous_i];
                        let current_is_solid = solidity[current_i];

                        if previous_is_solid == current_is_solid {
                            continue;
                        }

                        if !previous_is_solid && current_is_solid {
                            let current_pos = [v, unpadded_d, u];
                            let current = padded_chunk.get_block_from_i(current_i);

                            let v0_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][0]);
                            let v1_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][1]);
                            let v2_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][2]);
                            let v3_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][3]);
                            let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                            mask_row[(v - 1) as usize] =
                                FaceMask::from(false, texture_lookup[current.id as usize], FACES[1], packed_ao);
                        } else {
                            let previous_pos = [v, d, u];
                            let previous = padded_chunk.get_block_from_i(previous_i);

                            let v0_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][0]);
                            let v1_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][1]);
                            let v2_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][2]);
                            let v3_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][3]);
                            let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                            mask_row[(v - 1) as usize] =
                                FaceMask::from(false, texture_lookup[previous.id as usize], FACES[0], packed_ao);
                        }
                    }
                }

                for u in 0..=LAST_CHUNK_AXIS_INDEX_USIZE {
                    let mut v = 0;
                    let u_f32 = u as f32;

                    while v <= LAST_CHUNK_AXIS_INDEX_USIZE {
                        let face = mask[u][v];

                        if face.get_visited() {
                            v += 1;
                            continue;
                        }

                        mask[u][v].set_visited(true);

                        let mut width = 1;
                        let mut height = 1;

                        for u_2 in (u + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                            if mask[u_2][v].get_visited() || mask[u_2][v].data != face.data {
                                break;
                            }
                            width += 1;
                            mask[u_2][v].set_visited(true);
                        }

                        'expand: for v_2 in (v + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                            for u_2 in u..(u + width) {
                                if mask[u_2][v_2].get_visited() || mask[u_2][v_2].data != face.data {
                                    break 'expand;
                                }
                            }
                            height += 1;
                            for u_2 in u..(u + width) {
                                mask[u_2][v_2].set_visited(true);
                            }
                        }

                        let v_f32 = v as f32;
                        let w_f32 = width as f32;
                        let h_f32 = height as f32;

                        let base_x = chunk_origin_x + v_f32;
                        let base_y = chunk_origin_y + d_f32;
                        let base_z = chunk_origin_z + u_f32;

                        let v0_pos = [base_x, base_y, base_z];
                        let v1_pos = [base_x, base_y, base_z + w_f32];
                        let v2_pos = [base_x + h_f32, base_y, base_z];
                        let v3_pos = [base_x + h_f32, base_y, base_z + w_f32];

                        let v0_ao = face.get_ao() >> 6;
                        let v1_ao = (face.get_ao() >> 4) & 0b11;
                        let v2_ao = (face.get_ao() >> 2) & 0b11;
                        let v3_ao = face.get_ao() & 0b11;

                        let texture_index = face.get_block_id();

                        const UV_U0: f32 = 0.0;
                        const UV_V0: f32 = 0.0;
                        let uv_u1 = h_f32;
                        let uv_v1 = w_f32;

                        let vertex_0 = Vertex::new(v0_pos[0], v0_pos[1], v0_pos[2], texture_index, v0_ao as f32, UV_U0, UV_V0);
                        let vertex_1 = Vertex::new(v1_pos[0], v1_pos[1], v1_pos[2], texture_index, v1_ao as f32, UV_U0, uv_v1);
                        let vertex_2 = Vertex::new(v2_pos[0], v2_pos[1], v2_pos[2], texture_index, v2_ao as f32, uv_u1, UV_V0);
                        let vertex_3 = Vertex::new(v3_pos[0], v3_pos[1], v3_pos[2], texture_index, v3_ao as f32, uv_u1, uv_v1);

                        let reverse_faces = face.get_face().is_positive();

                        if reverse_faces {
                            vertices.extend_from_slice(&[vertex_0, vertex_1, vertex_2, vertex_2, vertex_1, vertex_3]);
                        } else {
                            vertices.extend_from_slice(&[vertex_1, vertex_0, vertex_3, vertex_3, vertex_0, vertex_2]);
                        }

                        v += height;
                    }
                }
            }
        }

        pub fn make_greedy_z(
            padded_chunk: &PaddedChunk,
            solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE],
            vertices: &mut Vec<Vertex>,
            chunk_origin_x: f32,
            chunk_origin_y: f32,
            chunk_origin_z: f32,
            texture_lookup: &[u32],
        ) {
            const CLEARED_MASK: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
                [[FaceMask::empty(); CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

            const FACES: [Direction; 2] = [Direction::Front, Direction::Back];

            const NEIGHBORS: [[[(i32, i32, i32); 3]; 4]; 2] = [
                [
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::BottomLeft),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::BottomRight),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::TopLeft),
                    Reference::get_ao_neighbors(FACES[0], SquareCorner::TopRight),
                ],
                [
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::BottomLeft),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::BottomRight),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::TopLeft),
                    Reference::get_ao_neighbors(FACES[1], SquareCorner::TopRight),
                ],
            ];

            let mut mask: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

            for d in 0..CHUNK_SIZE {
                mask = CLEARED_MASK;

                let d_f32 = d as f32;
                let unpadded_d = d + 1;

                for u in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                    let mask_row = &mut mask[(u - 1) as usize];
                    for v in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                        let previous_i = (u + v * PADDED_CHUNK_SIZE + d * PADDED_CHUNK_SIZE_SQR) as usize;
                        let current_i = previous_i + PADDED_CHUNK_SIZE_SQR_USIZE;

                        let previous_is_solid = solidity[previous_i];
                        let current_is_solid = solidity[current_i];

                        if previous_is_solid == current_is_solid {
                            continue;
                        }

                        if !previous_is_solid && current_is_solid {
                            let current_pos = [u, v, unpadded_d];
                            let current = padded_chunk.get_block_from_i(current_i);

                            let v0_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][0]);
                            let v1_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][1]);
                            let v2_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][2]);
                            let v3_ao = Reference::get_face_ao(solidity, current_pos, NEIGHBORS[1][3]);
                            let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                            mask_row[(v - 1) as usize] =
                                FaceMask::from(false, texture_lookup[current.id as usize], FACES[1], packed_ao);
                        } else {
                            let previous_pos = [u, v, d];
                            let previous = padded_chunk.get_block_from_i(previous_i);

                            let v0_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][0]);
                            let v1_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][1]);
                            let v2_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][2]);
                            let v3_ao = Reference::get_face_ao(solidity, previous_pos, NEIGHBORS[0][3]);
                            let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                            mask_row[(v - 1) as usize] =
                                FaceMask::from(false, texture_lookup[previous.id as usize], FACES[0], packed_ao);
                        }
                    }
                }

                for u in 0..=LAST_CHUNK_AXIS_INDEX_USIZE {
                    let mut v = 0;
                    let u_f32 = u as f32;

                    while v <= LAST_CHUNK_AXIS_INDEX_USIZE {
                        let face = mask[u][v];

                        if face.get_visited() {
                            v += 1;
                            continue;
                        }

                        mask[u][v].set_visited(true);

                        let mut width = 1;
                        let mut height = 1;

                        for u_2 in (u + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                            if mask[u_2][v].get_visited() || mask[u_2][v].data != face.data {
                                break;
                            }
                            width += 1;
                            mask[u_2][v].set_visited(true);
                        }

                        'expand: for v_2 in (v + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                            for u_2 in u..(u + width) {
                                if mask[u_2][v_2].get_visited() || mask[u_2][v_2].data != face.data {
                                    break 'expand;
                                }
                            }
                            height += 1;
                            for u_2 in u..(u + width) {
                                mask[u_2][v_2].set_visited(true);
                            }
                        }

                        let v_f32 = v as f32;
                        let w_f32 = width as f32;
                        let h_f32 = height as f32;

                        let base_x = chunk_origin_x + u_f32;
                        let base_y = chunk_origin_y + v_f32;
                        let base_z = chunk_origin_z + d_f32;

                        let v0_pos = [base_x, base_y, base_z];
                        let v1_pos = [base_x + w_f32, base_y, base_z];
                        let v2_pos = [base_x, base_y + h_f32, base_z];
                        let v3_pos = [base_x + w_f32, base_y + h_f32, base_z];

                        let v0_ao = face.get_ao() >> 6;
                        let v1_ao = (face.get_ao() >> 4) & 0b11;
                        let v2_ao = (face.get_ao() >> 2) & 0b11;
                        let v3_ao = face.get_ao() & 0b11;

                        let texture_index = face.get_block_id();

                        const UV_U0: f32 = 0.0;
                        const UV_V0: f32 = 0.0;
                        let uv_u1 = h_f32;
                        let uv_v1 = w_f32;

                        let vertex_0 = Vertex::new(v0_pos[0], v0_pos[1], v0_pos[2], texture_index, v0_ao as f32, UV_U0, UV_V0);
                        let vertex_1 = Vertex::new(v1_pos[0], v1_pos[1], v1_pos[2], texture_index, v1_ao as f32, UV_U0, uv_v1);
                        let vertex_2 = Vertex::new(v2_pos[0], v2_pos[1], v2_pos[2], texture_index, v2_ao as f32, uv_u1, UV_V0);
                        let vertex_3 = Vertex::new(v3_pos[0], v3_pos[1], v3_pos[2], texture_index, v3_ao as f32, uv_u1, uv_v1);

                        let reverse_faces = face.get_face().is_positive();

                        if reverse_faces {
                            vertices.extend_from_slice(&[vertex_0, vertex_1, vertex_2, vertex_2, vertex_1, vertex_3]);
                        } else {
                            vertices.extend_from_slice(&[vertex_1, vertex_0, vertex_3, vertex_3, vertex_0, vertex_2]);
                        }

                        v += height;
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Utilitaires de test
    // -----------------------------------------------------------------------

    /// Générateur pseudo-aléatoire déterministe (xorshift32).
    struct Rng(u32);

    impl Rng {
        fn next_u32(&mut self) -> u32 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.0 = x;
            x
        }

        fn next_f32(&mut self) -> f32 {
            (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
        }
    }

    fn chunk_from_bools(cells: &[bool]) -> Arc<Chunk> {
        assert_eq!(cells.len(), CHUNK * CHUNK * CHUNK);
        let blocks = cells
            .iter()
            .enumerate()
            .map(|(i, &solid)| BlockInstance::new(if solid { (i % 7 + 1) as u32 } else { 0 }))
            .collect();
        Arc::new(Chunk {
            blocks,
            x: 0,
            y: 0,
            z: 0,
        })
    }

    fn empty_chunk() -> Arc<Chunk> {
        chunk_from_bools(&vec![false; CHUNK * CHUNK * CHUNK])
    }

    /// Table de correspondance bloc → texture.
    fn texture_lookup() -> Vec<u32> {
        (0..64u32).map(|id| (id * 3 + 1) % 17).collect()
    }

    /// Vérifie que la nouvelle implémentation produit exactement les mêmes
    /// sommets que la référence.
    fn check(snapshot: &MeshSnapshot, label: &str) {
        let lookup = texture_lookup();

        // Référence.
        let padded = PaddedChunk::from_snapshot(&snapshot.main, snapshot);
        let mut solidity = [false; PADDED_CHUNK_BLOCK_CBE_USIZE];
        for i in 0..PADDED_CHUNK_BLOCK_CBE_USIZE {
            solidity[i] = padded.get_block_from_i(i).is_solid();
        }

        let mut expected = Vec::new();
        Reference::make_greedy_x(&padded, &solidity, &mut expected, 0.0, 0.0, 0.0, &lookup);
        Reference::make_greedy_y(&padded, &solidity, &mut expected, 0.0, 0.0, 0.0, &lookup);
        Reference::make_greedy_z(&padded, &solidity, &mut expected, 0.0, 0.0, 0.0, &lookup);

        // Nouvelle implémentation.
        let mut actual = Vec::new();
        make_greedy_chunk(snapshot, &mut actual, [0.0, 0.0, 0.0], &lookup);

        assert_eq!(
            expected.len(),
            actual.len(),
            "[{label}] nombre de sommets différent ({} vs {})",
            expected.len(),
            actual.len()
        );
        assert_eq!(
            bytemuck::cast_slice::<Vertex, u8>(&expected),
            bytemuck::cast_slice::<Vertex, u8>(&actual),
            "[{label}] sommets différents"
        );
    }

    fn snapshot_of(
        main: Arc<Chunk>,
        neg_x: Option<Arc<Chunk>>,
        pos_x: Option<Arc<Chunk>>,
        neg_y: Option<Arc<Chunk>>,
        pos_y: Option<Arc<Chunk>>,
        neg_z: Option<Arc<Chunk>>,
        pos_z: Option<Arc<Chunk>>,
    ) -> MeshSnapshot {
        MeshSnapshot {
            main,
            neg_x,
            pos_x,
            neg_y,
            pos_y,
            neg_z,
            pos_z,
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[test]
    fn transpose_is_correct() {
        let mut rng = Rng(0x1234_5678);
        let mut rows = [0u32; CHUNK];

        for (i, row) in rows.iter_mut().enumerate() {
            *row = rng.next_u32();
            // On ne garde que les 32 bits utiles, obviously.
            let _ = i;
        }

        let mut expected = [0u32; CHUNK];
        for i in 0..CHUNK {
            for j in 0..CHUNK {
                if (rows[i] >> j) & 1 == 1 {
                    expected[j] |= 1 << i;
                }
            }
        }

        transpose32(&mut rows);
        assert_eq!(rows, expected);
    }

    #[test]
    fn empty_chunk_matches() {
        let snapshot = snapshot_of(empty_chunk(), None, None, None, None, None, None);
        check(&snapshot, "chunk vide");
    }

    #[test]
    fn solid_chunk_matches() {
        let snapshot = snapshot_of(
            chunk_from_bools(&vec![true; CHUNK * CHUNK * CHUNK]),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        check(&snapshot, "chunk plein");
    }

    #[test]
    fn checkerboard_matches() {
        let mut cells = vec![false; CHUNK * CHUNK * CHUNK];
        for z in 0..CHUNK {
            for y in 0..CHUNK {
                for x in 0..CHUNK {
                    cells[x + y * CHUNK + z * CHUNK * CHUNK] = (x + y + z) % 2 == 0;
                }
            }
        }
        let snapshot = snapshot_of(chunk_from_bools(&cells), None, None, None, None, None, None);
        check(&snapshot, "échiquier");
    }

    #[test]
    fn random_chunks_match() {
        let mut rng = Rng(0xDEAD_BEEF);

        for iteration in 0..8 {
            let density = 0.1 + iteration as f32 * 0.1;
            let mut cells = vec![false; CHUNK * CHUNK * CHUNK];
            for cell in cells.iter_mut() {
                *cell = rng.next_f32() < density;
            }

            // Voisins : certains présents, d'autres absents.
            let make_neighbor = |rng: &mut Rng, density: f32| -> Option<Arc<Chunk>> {
                if rng.next_f32() < 0.3 {
                    return None;
                }
                let mut cells = vec![false; CHUNK * CHUNK * CHUNK];
                for cell in cells.iter_mut() {
                    *cell = rng.next_f32() < density;
                }
                Some(chunk_from_bools(&cells))
            };

            let snapshot = snapshot_of(
                chunk_from_bools(&cells),
                make_neighbor(&mut rng, density),
                make_neighbor(&mut rng, density),
                make_neighbor(&mut rng, density),
                make_neighbor(&mut rng, density),
                make_neighbor(&mut rng, density),
                make_neighbor(&mut rng, density),
            );

            check(&snapshot, &format!("bruit aléatoire {iteration}"));
        }
    }

    #[test]
    fn terrain_matches() {
        let mut rng = Rng(0x1234_ABCD);

        for iteration in 0..4 {
            let mut cells = vec![false; CHUNK * CHUNK * CHUNK];
            for z in 0..CHUNK {
                for x in 0..CHUNK {
                    let height = 8 + ((x * 7 + z * 13 + iteration * 5) % 20) as usize;
                    for y in 0..height {
                        cells[x + y * CHUNK + z * CHUNK * CHUNK] = true;
                    }
                }
            }

            // Un peu de bruit autour du terrain.
            for _ in 0..200 {
                let x = (rng.next_u32() as usize) % CHUNK;
                let y = (rng.next_u32() as usize) % CHUNK;
                let z = (rng.next_u32() as usize) % CHUNK;
                cells[x + y * CHUNK + z * CHUNK * CHUNK] = rng.next_f32() < 0.5;
            }

            let snapshot = snapshot_of(
                chunk_from_bools(&cells),
                Some(chunk_from_bools(&cells)),
                Some(chunk_from_bools(&cells)),
                None,
                None,
                None,
                None,
            );

            check(&snapshot, &format!("terrain {iteration}"));
        }
    }

    #[test]
    fn bench_make_greedy_chunk_samples() {
        use std::time::Instant;

        fn run(snapshot: &MeshSnapshot, iters: usize) -> u128 {
            let mut total: u128 = 0;
            let origin = [0.0f32, 0.0f32, 0.0f32];
            for _ in 0..iters {
                let mut vertices: Vec<Vertex> = Vec::new();
                let start = Instant::now();
                make_greedy_chunk(snapshot, &mut vertices, origin, &[]);
                total += start.elapsed().as_nanos() as u128;
                let _ = vertices.len();
            }
            total / (iters as u128)
        }

        // empty
        let snap_empty = snapshot_of(empty_chunk(), None, None, None, None, None, None);
        // solid
        let snap_solid = snapshot_of(
            chunk_from_bools(&vec![true; CHUNK * CHUNK * CHUNK]),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        // checkerboard
        let mut cells_cb = vec![false; CHUNK * CHUNK * CHUNK];
        for z in 0..CHUNK {
            for y in 0..CHUNK {
                for x in 0..CHUNK {
                    cells_cb[x + y * CHUNK + z * CHUNK * CHUNK] = (x + y + z) % 2 == 0;
                }
            }
        }
        let snap_cb = snapshot_of(chunk_from_bools(&cells_cb), None, None, None, None, None, None);
        // random
        let mut rng = Rng(0xDEAD_BEEF);
        let density = 0.3;
        let mut cells_r = vec![false; CHUNK * CHUNK * CHUNK];
        for cell in cells_r.iter_mut() {
            *cell = rng.next_f32() < density;
        }
        let make_n = |r: &mut Rng, d: f32| -> Option<Arc<Chunk>> {
            if r.next_f32() < 0.3 {
                return None;
            }
            let mut c = vec![false; CHUNK * CHUNK * CHUNK];
            for cell in c.iter_mut() {
                *cell = r.next_f32() < d;
            }
            Some(chunk_from_bools(&c))
        };
        let snap_r = snapshot_of(
            chunk_from_bools(&cells_r),
            make_n(&mut rng, density),
            make_n(&mut rng, density),
            make_n(&mut rng, density),
            make_n(&mut rng, density),
            make_n(&mut rng, density),
            make_n(&mut rng, density),
        );
        // terrain
        let mut rng_t = Rng(0x1234_ABCD);
        let mut cells_t = vec![false; CHUNK * CHUNK * CHUNK];
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let height = 8 + ((x * 7 + z * 13) % 20) as usize;
                for y in 0..height {
                    cells_t[x + y * CHUNK + z * CHUNK * CHUNK] = true;
                }
            }
        }
        for _ in 0..200 {
            let x = (rng_t.next_u32() as usize) % CHUNK;
            let y = (rng_t.next_u32() as usize) % CHUNK;
            let z = (rng_t.next_u32() as usize) % CHUNK;
            cells_t[x + y * CHUNK + z * CHUNK * CHUNK] = rng_t.next_f32() < 0.5;
        }
        let snap_t = snapshot_of(
            chunk_from_bools(&cells_t),
            Some(chunk_from_bools(&cells_t)),
            None,
            None,
            None,
            None,
            None,
        );

        eprintln!(
            "bench: empty={}ns solid={}ns cb={}ns random(d0.3)={}ns terrain={}ns ({} iters)",
            run(&snap_empty, 2000),
            run(&snap_solid, 2000),
            run(&snap_cb, 2000),
            run(&snap_r, 1000),
            run(&snap_t, 1000),
            2000
        );
    }
}
