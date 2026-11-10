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
use game::world::data::block::{BlockId, BlockInstance};
use game::world::data::chunk::{Chunk, CHUNK_SIZE_SQR_USIZE, CHUNK_SIZE_USIZE};
use project_core::math::geometry::corner::SquareCorner;
use project_core::math::geometry::direction::Direction;
use std::mem::swap;
use std::sync::Arc;

use crate::systems::texture_registry::TextureRegistry;
use crate::world::mesh_request::MeshSnapshot;

/// Cellules par côté d'un chunk.
const CHUNK: usize = CHUNK_SIZE_USIZE;
const CHUNK_SQR: usize = CHUNK_SIZE_SQR_USIZE;
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

type UvNormal3D = (i32, i32, i32);
type DirectionUvNormal3D = (UvNormal3D, UvNormal3D, UvNormal3D);

/// Les 3 voisins AO d'un coin de face, en coordonnées 3D.
///
/// Copié à l'identique de l'implémentation de référence : la géométrie de l'AO
/// doit rester rigoureusement la même, y compris l'inversion (u, v) appliquée
/// aux faces autres que Front/Back.
const fn ao_neighbors(face: Direction, corner: SquareCorner) -> [(i32, i32, i32); 3] {
    const UVNORMAL_TABLE: [DirectionUvNormal3D; 6] = [
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

    for (i, row) in rows.iter().enumerate().take(CHUNK) {
        for (j, col) in out.iter_mut().enumerate().take(CHUNK) {
            *col |= ((row >> j) & 1) << i;
        }
    }

    *rows = out;
}

#[inline]
fn transpose32_into(src: &[u32; CHUNK], dst: &mut [u32; CHUNK]) {
    let mut out = [0u32; CHUNK];
    for (i, row) in src.iter().enumerate().take(CHUNK) {
        for (j, element) in out.iter_mut().enumerate().take(CHUNK) {
            *element |= ((row >> j) & 1) << i;
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
    }

    #[cfg(not(target_arch = "x86_64"))]
    build_core_scalar(blocks, out);
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
/// Utilisée sur les cibles non x86_64.
#[cfg(not(target_arch = "x86_64"))]
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

    const KINDS: [usize; 2] = [CHUNK - 1, 0];

    let w = KINDS[kind & 1];

    match kind {
        // Couche x = 31 (négatif) ou x = 0 (positif) : lignes z, bits y.
        NEG_X | POS_X => {
            for z in 0..CHUNK {
                let mut word = 0u32;
                let z_offset = z * CHUNK_SQR;
                for y in 0..CHUNK {
                    let id = ids[w + y * CHUNK + z_offset];
                    word |= ((id != 0) as u32) << y;
                }
                primary.rows[z] = word;
            }
        }
        // Couche y = 31 (négatif) ou y = 0 (positif) : lignes z, bits x.
        NEG_Y | POS_Y => {
            let w_offset = w * CHUNK;
            for z in 0..CHUNK {
                let base = w_offset + z * CHUNK_SQR;
                let mut word = 0u32;
                for x in 0..CHUNK {
                    word |= ((ids[base + x] != 0) as u32) << x;
                }
                primary.rows[z] = word;
            }
        }
        // Couche z = 31 (négatif) ou z = 0 (positif) : lignes y, bits x.
        NEG_Z | POS_Z => {
            let w_offset = w * CHUNK_SQR;
            for y in 0..CHUNK {
                let base = y * CHUNK + w_offset;
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
    /// neg_z, pos_z]`). Un voisin absent est representé par des plans vides
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
    fn solid_block_id(&self, axis: Axis, main: usize, r: usize, c: usize) -> BlockId {
        let index = match axis {
            Axis::X => main + r * CHUNK + c * CHUNK * CHUNK,
            Axis::Y => c + main * CHUNK + r * CHUNK * CHUNK,
            Axis::Z => r + c * CHUNK + main * CHUNK * CHUNK,
        };
        self.main_chunk.blocks[index].id
    }

    /// L'identité du bloc solide situé dans le voisin négatif de l'axe.
    fn neighbor_block_id(&self, axis: Axis, r: usize, c: usize) -> BlockId {
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
            None => BlockId(0),
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

    for (corner, offset) in offsets.iter().enumerate().take(4) {
        let corner_bit = ((plane.rows[(u as i32 + offset[0].0) as usize] >> (v as i32 + offset[0].1)) & 1) as u8;
        let side1_bit = ((plane.rows[(u as i32 + offset[1].0) as usize] >> (v as i32 + offset[1].1)) & 1) as u8;
        let side2_bit = ((plane.rows[(u as i32 + offset[2].0) as usize] >> (v as i32 + offset[2].1)) & 1) as u8;

        let idx = ((corner_bit << 2) | (side1_bit << 1) | side2_bit) as usize;
        packed |= AO_TABLE[idx] << (6 - 2 * corner);
    }

    packed
}

/// Maille le chunk de façon greedy, sur les trois axes.
///
/// Le résultat est ajouté à `vertices`, dans le même ordre que l'implémentation
/// de référence : axes X, puis Y, puis Z.
pub fn make_greedy_chunk(
    snapshot: &MeshSnapshot,
    vertices: &mut Vec<Vertex>,
    chunk_origin: [f32; 3],
    textures: Arc<TextureRegistry>,
) {
    let solidity = ChunkSolidity::new(snapshot);

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
        solidity.build_plane(axis, 0, &mut lo);

        let mut keys_arr = [0u64; CHUNK * CHUNK];
        let mut present_arr = [0u32; CHUNK];

        for d in 0..CHUNK {
            solidity.build_plane(axis, d + 1, &mut hi);

            // skip rapide
            let mut has_faces = false;
            for r in 0..CHUNK {
                if ((lo.rows[r + 1] >> 1) as u32) ^ ((hi.rows[r + 1] >> 1) as u32) != 0 {
                    has_faces = true;
                    break;
                }
            }
            if !has_faces {
                for r in present_arr.iter_mut().take(CHUNK) {
                    *r = 0;
                }
                swap(&mut lo, &mut hi);
                continue;
            }

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
                    let texture = textures
                        .get_block_texture(&block_id)
                        .map_or_default(|texture_id| texture_id.id());
                    // let texture = if bid < texture_lookup.len() { texture_lookup[bid] } else { 0 };
                    keys_arr[r * CHUNK + c] = ((texture as u64) << 16) | ((dir as u64) << 8) | packed_ao as u64;
                }
            }

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
                    let cleared = !(((1u64 << height) - 1) << v) as u32;
                    for rrc in present_arr.iter_mut().skip(u).take(width) {
                        *rrc &= cleared;
                    }
                    v += height;
                }
            }
            swap(&mut lo, &mut hi);
        }
    }
}
