use crate::render::utils::{face_mask::FaceMask, padded_chunk::*};
use bytemuck::cast_slice;
use engine::geometry::vertex::Vertex;
use engine::render::mesh::manager::{AllocError, MeshId, MeshManager};
use game::world::data::chunk::{CHUNK_SIZE, LAST_CHUNK_AXIS_INDEX_USIZE};
use satiscore::geometry::corner::SquareCorner;
use satiscore::geometry::direction::Direction;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ChunkMesh {
    pub id: Option<MeshId>,
    dirty: AtomicBool,
}

impl ChunkMesh {
    pub fn new() -> ChunkMesh {
        return ChunkMesh {
            id: None,
            dirty: AtomicBool::new(true),
        };
    }

    #[inline(always)]
    pub fn is_dirty(&self) -> bool {
        return self.dirty.load(Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn set_dirty(&mut self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn get_debug_infos(&self) -> (Option<u32>, bool) {
        (self.id, self.dirty.load(Ordering::Relaxed))
    }

    #[inline(always)]
    pub fn get_solidity_xyz(solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE], x: i32, y: i32, z: i32) -> bool {
        return solidity[(x + y * PADDED_CHUNK_SIZE + z * PADDED_CHUNK_SIZE_SQR) as usize];
    }

    #[inline(always)]
    fn get_face_ao(solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE], pos: [i32; 3], neighbors: [(i32, i32, i32); 3]) -> u8 {
        // AO for each configuration (range 0-3 both included)
        // Scheme:
        // [side1][side2][corner]
        // [0][0][0] : 3
        // [1][0][0] : 2
        // [0][1][0] : 2
        // [1][1][0] : 0
        // [0][0][1] : 2
        // [1][0][1] : 1
        // [0][1][1] : 1
        // [1][1][1] : 0
        // PS: if both side1 and side2 exists, then we don't care about the corner-block. The face's corner will be de facto completely black.
        const AO_TABLE: [u8; 8] = [3, 2, 2, 0, 2, 1, 1, 0];

        // Check if neighbors exists and if they exists, if they are solid (AKA doesn't let light pass through them).
        let corner_solid =
            ChunkMesh::get_solidity_xyz(solidity, pos[0] + neighbors[0].0, pos[1] + neighbors[0].1, pos[2] + neighbors[0].2) as u8;
        let side1_solid =
            ChunkMesh::get_solidity_xyz(solidity, pos[0] + neighbors[1].0, pos[1] + neighbors[1].1, pos[2] + neighbors[1].2) as u8;
        let side2_solid =
            ChunkMesh::get_solidity_xyz(solidity, pos[0] + neighbors[2].0, pos[1] + neighbors[2].1, pos[2] + neighbors[2].2) as u8;

        // Calc the index and return the corresponding AO
        let idx = ((corner_solid << 2) | (side1_solid << 1) | side2_solid) as usize;
        return AO_TABLE[idx];
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

        // U, V and normal vectors depending on the face
        let (u, v, normal) = UVNORMAL_TABLE[face.to_usize()];
        // U & V signs depending on the corner
        let (su, sv) = CORNER_TABLE[corner.to_usize()];

        // I don't remember why we invert (probably because of orientation and related to local directions, with corners etc), but we need to.
        let (u, v) = match face {
            Direction::Front | Direction::Back => (u, v),
            _ => (v, u),
        };

        // Just maths
        let side1 = (u.0 * su + normal.0, u.1 * su + normal.1, u.2 * su + normal.2);
        let side2 = (v.0 * sv + normal.0, v.1 * sv + normal.1, v.2 * sv + normal.2);
        let corner = (
            side1.0 + side2.0 - normal.0,
            side1.1 + side2.1 - normal.1,
            side1.2 + side2.2 - normal.2,
        );

        [corner, side1, side2]
    }

    /// Makes the greedy mesh for the x axis, in both directions (+, -).
    pub fn make_greedy_x(
        padded_chunk: &PaddedChunk,
        solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE],
        vertices: &mut Vec<Vertex>,
        chunk_origin_x: f32,
        chunk_origin_y: f32,
        chunk_origin_z: f32,
    ) {
        // Local bases
        // D is the main axis (X)
        // U is the secondary axis (Y)
        // V is the tertiary axis (Z)

        // The mask, completely blank
        const CLEARED_MASK: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
            [[FaceMask::empty(); CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

        // Faces directions for the X axis (+, -)
        const FACES: [Direction; 2] = [Direction::Right, Direction::Left];

        // AO Neighbors pre-calculated for each corner and face direction
        const NEIGHBORS: [[[(i32, i32, i32); 3]; 4]; 2] = [
            [
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::BottomLeft),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::BottomRight),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::TopLeft),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::TopRight),
            ],
            [
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::BottomLeft),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::BottomRight),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::TopLeft),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::TopRight),
            ],
        ];

        // The mask we will be using to mark faces (step 1: masking) and merge faces (step 2: meshing)
        // The mask isn't initialized because we clear it every time we start the loop, saving time (c.f. below)
        let mut mask: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

        // D loop must occur CHUNK_SIZE + 1 times since for N blocs there are N + 1 possible faces (pointing out of the chunk and in between each block)
        for d in 0..CHUNK_SIZE {
            // Reset the mask
            mask = CLEARED_MASK;

            let d_f32 = d as f32;
            let unpadded_d = d + 1; // d in the actual chunk, without padding

            // MASKING + AO
            for u in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                // Compute the row we will use a single time, so access to an item saves time (not 2 index calc but a single one)
                let mask_row = &mut mask[(u - 1) as usize];
                for v in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                    let previous_pos = [d, u, v]; // The block before (x-1, y, z) actual coords (x, y, z)

                    let previous_i =
                        (previous_pos[0] + previous_pos[1] * PADDED_CHUNK_SIZE + previous_pos[2] * PADDED_CHUNK_SIZE_SQR) as usize; // index in the solidity array
                    let current_i = previous_i + 1; // current block is previous + x_stride (=1)

                    let previous_is_solid = solidity[previous_i];
                    let current_is_solid = solidity[current_i];

                    if previous_is_solid == current_is_solid {
                        continue;
                    }

                    if !previous_is_solid && current_is_solid {
                        let current_pos = [unpadded_d, u, v]; // unpadded_d = d + 1
                        let current = padded_chunk.get_block_from_i(current_i);

                        let v0_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][0]); // Bottom left
                        let v1_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][1]); // Bottom right
                        let v2_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][2]); // Top left
                        let v3_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][3]); // Top right

                        let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | (v3_ao << 0);

                        // We mark the mask as unvisited so the mesher will know we need to make a face out of this
                        // -1 because v is the index for PADDED_CHUNK, but mask contains indices for a basic CHUNK
                        mask_row[(v - 1) as usize] = FaceMask::from(false, current.texture_index(), FACES[1], packed_ao);
                    } else {
                        let previous = padded_chunk.get_block_from_i(previous_i);

                        let v0_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][0]); // Bottom left
                        let v1_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][1]); // Bottom right
                        let v2_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][2]); // Top left
                        let v3_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][3]); // Top right

                        let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | (v3_ao << 0);

                        // We mark the mask as unvisited so the mesher will know we need to make a face out of this
                        // -1 because v is the index for PADDED_CHUNK, but mask contains indices for a basic CHUNK
                        mask_row[(v - 1) as usize] = FaceMask::from(false, previous.texture_index(), FACES[0], packed_ao);
                    }
                }
            }

            // MESHING
            for u in 0..=LAST_CHUNK_AXIS_INDEX_USIZE {
                let mut v = 0;
                let u_f32 = u as f32;

                while v <= LAST_CHUNK_AXIS_INDEX_USIZE {
                    let face = mask[u][v];

                    // Face either is AIR, or was already meshed
                    if face.get_visited() {
                        v += 1;
                        continue;
                    }

                    mask[u][v].set_visited(true);

                    let mut width = 1;
                    let mut height = 1;

                    // Expansion in the U axis
                    // If face was not meshed and corresponds to the face we want to mesh, enlarge the rect
                    for u_2 in (u + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                        if mask[u_2][v].get_visited() || mask[u_2][v].data != face.data {
                            break;
                        }
                        width += 1;
                        mask[u_2][v].set_visited(true);
                    }

                    // Expansion in the V axis
                    // We must check for each additional height if the whole line corresponds
                    'expand: for v_2 in (v + 1)..=LAST_CHUNK_AXIS_INDEX_USIZE {
                        // For each time we increment in the V axis, we must verify that every block in the U axis is compatible.
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

                    // Block positions respectively with the block's width and block's height, to calc vertices positions
                    let block_pos_u = block_pos_y + w_f32;
                    let block_pos_v = block_pos_z + h_f32;

                    let v0_pos = [block_pos_x, block_pos_y, block_pos_z];
                    let v1_pos = [block_pos_x, block_pos_y, block_pos_v];
                    let v2_pos = [block_pos_x, block_pos_u, block_pos_z];
                    let v3_pos = [block_pos_x, block_pos_u, block_pos_v];

                    // Unpack every vertex AO
                    let v0_ao = face.get_ao() >> 6;
                    let v1_ao = (face.get_ao() >> 4) & 0b11;
                    let v2_ao = (face.get_ao() >> 2) & 0b11;
                    let v3_ao = face.get_ao() & 0b11;

                    let texture_index = face.get_block_id();

                    // UVs
                    const UV_U0: f32 = 0.0;
                    const UV_V0: f32 = 0.0;
                    let uv_u1 = w_f32;
                    let uv_v1 = h_f32;

                    // Vertices
                    let vertex_0 = Vertex::new(v0_pos[0], v0_pos[1], v0_pos[2], texture_index, (v0_ao as i32) as f32, UV_U0, UV_V0);
                    let vertex_1 = Vertex::new(v1_pos[0], v1_pos[1], v1_pos[2], texture_index, (v1_ao as i32) as f32, UV_U0, uv_v1);
                    let vertex_2 = Vertex::new(v2_pos[0], v2_pos[1], v2_pos[2], texture_index, (v2_ao as i32) as f32, uv_u1, UV_V0);
                    let vertex_3 = Vertex::new(v3_pos[0], v3_pos[1], v3_pos[2], texture_index, (v3_ao as i32) as f32, uv_u1, uv_v1);

                    // Because of back culling, we must invert the normal of the face by swaping vertices of the triangles on the horizontal axis
                    let reverse_faces = face.get_face().is_negative();

                    if reverse_faces {
                        vertices.extend_from_slice(&[vertex_0, vertex_1, vertex_2, vertex_2, vertex_1, vertex_3]);
                    } else {
                        vertices.extend_from_slice(&[vertex_1, vertex_0, vertex_3, vertex_3, vertex_0, vertex_2]);
                    }

                    // Skip progress made along the v axis (we can't on the u axis because we do not know if a different block exists in the width * height rect)
                    v += height;
                }
            }
        }
    }

    /// Makes the greedy mesh for the y axis, in both directions (+, -).
    pub fn make_greedy_y(
        padded_chunk: &PaddedChunk,
        solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE],
        vertices: &mut Vec<Vertex>,
        chunk_origin_x: f32,
        chunk_origin_y: f32,
        chunk_origin_z: f32,
    ) {
        // D = Y, U = Z, V = X
        // mask[u][v] = mask[Z][X]
        // stride Y = PADDED_CHUNK_SIZE

        const CLEARED_MASK: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
            [[FaceMask::empty(); CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

        const FACES: [Direction; 2] = [Direction::Top, Direction::Bottom];

        const NEIGHBORS: [[[(i32, i32, i32); 3]; 4]; 2] = [
            [
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::BottomLeft),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::BottomRight),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::TopLeft),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::TopRight),
            ],
            [
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::BottomLeft),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::BottomRight),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::TopLeft),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::TopRight),
            ],
        ];

        let mut mask: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

        for d in 0..CHUNK_SIZE {
            mask = CLEARED_MASK;

            let d_f32 = d as f32;
            let unpadded_d = d + 1;

            // MASKING + AO
            // U = Z : de 1 à LAST_PADDED_CHUNK_CENTER_INDEX
            // V = X : de 1 à LAST_PADDED_CHUNK_CENTER_INDEX
            for u in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                let mask_row = &mut mask[(u - 1) as usize]; // mask[Z-1][...]
                for v in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                    // previous = (X=v, Y=d, Z=u) dans le padded chunk
                    // index = v + d*PS + u*PS²
                    let previous_i = (v + d * PADDED_CHUNK_SIZE + u * PADDED_CHUNK_SIZE_SQR) as usize;
                    // current = (X=v, Y=d+1, Z=u) → stride Y = +PADDED_CHUNK_SIZE
                    let current_i = previous_i + PADDED_CHUNK_SIZE_USIZE;

                    let previous_is_solid = solidity[previous_i];
                    let current_is_solid = solidity[current_i];

                    if previous_is_solid == current_is_solid {
                        continue;
                    }

                    if !previous_is_solid && current_is_solid {
                        // current block est à (X=v, Y=unpadded_d, Z=u) en coords paddées
                        let current_pos = [v, unpadded_d, u];
                        let current = padded_chunk.get_block_from_i(current_i);

                        let v0_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][0]);
                        let v1_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][1]);
                        let v2_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][2]);
                        let v3_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][3]);
                        let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                        // mask[Z][X] — on retire le padding : u-1 et v-1
                        mask_row[(v - 1) as usize] = FaceMask::from(false, current.texture_index(), FACES[1], packed_ao);
                    } else {
                        // previous block est à (X=v, Y=d, Z=u) en coords paddées
                        let previous_pos = [v, d, u];
                        let previous = padded_chunk.get_block_from_i(previous_i);

                        let v0_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][0]);
                        let v1_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][1]);
                        let v2_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][2]);
                        let v3_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][3]);
                        let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                        mask_row[(v - 1) as usize] = FaceMask::from(false, previous.texture_index(), FACES[0], packed_ao);
                    }
                }
            }

            // MESHING
            // mask[u][v] = mask[Z][X]
            // u = Z, v = X, width progresse sur Z, height progresse sur X
            for u in 0..=LAST_CHUNK_AXIS_INDEX_USIZE {
                let mut v = 0;
                let u_f32 = u as f32; // Z local

                while v <= LAST_CHUNK_AXIS_INDEX_USIZE {
                    let face = mask[u][v];

                    if face.get_visited() {
                        v += 1;
                        continue;
                    }

                    mask[u][v].set_visited(true);

                    let mut width = 1; // expansion en U (Z)
                    let mut height = 1; // expansion en V (X)

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

                    let v_f32 = v as f32; // X local
                    let w_f32 = width as f32; // delta Z
                    let h_f32 = height as f32; // delta X

                    // Coin bas-gauche de la face : X=v, Y=d, Z=u
                    let base_x = chunk_origin_x + v_f32;
                    let base_y = chunk_origin_y + d_f32;
                    let base_z = chunk_origin_z + u_f32;

                    // Les 4 coins : on étend en Z (+w) et en X (+h)
                    // v0 = (X,   Y, Z  )
                    // v1 = (X,   Y, Z+w)
                    // v2 = (X+h, Y, Z  )
                    // v3 = (X+h, Y, Z+w)
                    let v0_pos = [base_x, base_y, base_z];
                    let v1_pos = [base_x, base_y, base_z + w_f32];
                    let v2_pos = [base_x + h_f32, base_y, base_z];
                    let v3_pos = [base_x + h_f32, base_y, base_z + w_f32];

                    let v0_ao = face.get_ao() >> 6;
                    let v1_ao = (face.get_ao() >> 4) & 0b11;
                    let v2_ao = (face.get_ao() >> 2) & 0b11;
                    let v3_ao = face.get_ao() & 0b11;

                    let texture_index = face.get_block_id();

                    // UVs : U → Z (width), V → X (height)
                    const UV_U0: f32 = 0.0;
                    const UV_V0: f32 = 0.0;
                    let uv_u1 = h_f32;
                    let uv_v1 = w_f32;

                    let vertex_0 = Vertex::new(v0_pos[0], v0_pos[1], v0_pos[2], texture_index, v0_ao as f32, UV_U0, UV_V0); // Bottom left
                    let vertex_1 = Vertex::new(v1_pos[0], v1_pos[1], v1_pos[2], texture_index, v1_ao as f32, UV_U0, uv_v1); // Bottom right
                    let vertex_2 = Vertex::new(v2_pos[0], v2_pos[1], v2_pos[2], texture_index, v2_ao as f32, uv_u1, UV_V0); // Top left
                    let vertex_3 = Vertex::new(v3_pos[0], v3_pos[1], v3_pos[2], texture_index, v3_ao as f32, uv_u1, uv_v1); // Top right

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

    /// Makes the greedy mesh for the z axis, in both directions (+, -).
    pub fn make_greedy_z(
        padded_chunk: &PaddedChunk,
        solidity: &[bool; PADDED_CHUNK_BLOCK_CBE_USIZE],
        vertices: &mut Vec<Vertex>,
        chunk_origin_x: f32,
        chunk_origin_y: f32,
        chunk_origin_z: f32,
    ) {
        // D = Z, U = X, V = Y
        // mask[u][v] = mask[X][Y]
        // stride Z = PADDED_CHUNK_SIZE_SQR

        const CLEARED_MASK: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
            [[FaceMask::empty(); CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

        const FACES: [Direction; 2] = [Direction::Front, Direction::Back];

        const NEIGHBORS: [[[(i32, i32, i32); 3]; 4]; 2] = [
            [
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::BottomLeft),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::BottomRight),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::TopLeft),
                ChunkMesh::get_ao_neighbors(FACES[0], SquareCorner::TopRight),
            ],
            [
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::BottomLeft),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::BottomRight),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::TopLeft),
                ChunkMesh::get_ao_neighbors(FACES[1], SquareCorner::TopRight),
            ],
        ];

        let mut mask: [[FaceMask; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

        for d in 0..CHUNK_SIZE {
            mask = CLEARED_MASK;

            let d_f32 = d as f32;
            let unpadded_d = d + 1;

            // MASKING + AO
            // U = Y : de 1 à LAST_PADDED_CHUNK_CENTER_INDEX
            // V = Z : de 1 à LAST_PADDED_CHUNK_CENTER_INDEX
            for u in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                let mask_row = &mut mask[(u - 1) as usize]; // mask[Z-1][...]
                for v in 1..=LAST_PADDED_CHUNK_CENTER_INDEX {
                    // previous = (X=u, Y=v, Z=d) dans le padded chunk
                    // index = u + v*PS + d*PS²
                    let previous_i = (u + v * PADDED_CHUNK_SIZE + d * PADDED_CHUNK_SIZE_SQR) as usize;
                    // current = (X=u, Y=v, Z=d+1) → stride Z = +PADDED_CHUNK_SIZE_SQR
                    let current_i = previous_i + PADDED_CHUNK_SIZE_SQR_USIZE;

                    let previous_is_solid = solidity[previous_i];
                    let current_is_solid = solidity[current_i];

                    if previous_is_solid == current_is_solid {
                        continue;
                    }

                    if !previous_is_solid && current_is_solid {
                        // current block est à (X=u, Y=v, Z=unpadded_d) en coords paddées
                        let current_pos = [u, v, unpadded_d];
                        let current = padded_chunk.get_block_from_i(current_i);

                        let v0_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][0]);
                        let v1_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][1]);
                        let v2_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][2]);
                        let v3_ao = ChunkMesh::get_face_ao(solidity, current_pos, NEIGHBORS[1][3]);
                        let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                        // mask[Y][Z] — on retire le padding : u-1 et v-1
                        mask_row[(v - 1) as usize] = FaceMask::from(false, current.texture_index(), FACES[1], packed_ao);
                    } else {
                        // previous block est à (X=u, Y=v, Z=d) en coords paddées
                        let previous_pos = [u, v, d];
                        let previous = padded_chunk.get_block_from_i(previous_i);

                        let v0_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][0]);
                        let v1_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][1]);
                        let v2_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][2]);
                        let v3_ao = ChunkMesh::get_face_ao(solidity, previous_pos, NEIGHBORS[0][3]);
                        let packed_ao = (v0_ao << 6) | (v1_ao << 4) | (v2_ao << 2) | v3_ao;

                        mask_row[(v - 1) as usize] = FaceMask::from(false, previous.texture_index(), FACES[0], packed_ao);
                    }
                }
            }

            // MESHING
            // mask[u][v] = mask[X][Y]
            // u = X, v = Y, width progresse sur X, height progresse sur Y
            for u in 0..=LAST_CHUNK_AXIS_INDEX_USIZE {
                let mut v = 0;
                let u_f32 = u as f32; // X local

                while v <= LAST_CHUNK_AXIS_INDEX_USIZE {
                    let face = mask[u][v];

                    if face.get_visited() {
                        v += 1;
                        continue;
                    }

                    mask[u][v].set_visited(true);

                    let mut width = 1; // expansion en U (X)
                    let mut height = 1; // expansion en V (Y)

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

                    let v_f32 = v as f32; // Z local
                    let w_f32 = width as f32; // delta X
                    let h_f32 = height as f32; // delta Y

                    // Coin bas-gauche de la face : X=u, Y=v, Z=d
                    let base_x = chunk_origin_x + u_f32;
                    let base_y = chunk_origin_y + v_f32;
                    let base_z = chunk_origin_z + d_f32;

                    // Les 4 coins : on étend en X (+w) et en Y (+h)
                    // v0 = (X+w, Y,   Z)
                    // v1 = (X,   Y,   Z)
                    // v2 = (X+w, Y+h, Z)
                    // v3 = (X,   Y+h, Z)
                    let v0_pos = [base_x, base_y, base_z];
                    let v1_pos = [base_x + w_f32, base_y, base_z];
                    let v2_pos = [base_x, base_y + h_f32, base_z];
                    let v3_pos = [base_x + w_f32, base_y + h_f32, base_z];

                    let v0_ao = face.get_ao() >> 6;
                    let v1_ao = (face.get_ao() >> 4) & 0b11;
                    let v2_ao = (face.get_ao() >> 2) & 0b11;
                    let v3_ao = face.get_ao() & 0b11;

                    let texture_index = face.get_block_id();

                    // UVs : U → X (width), V → Y (height)
                    const UV_U0: f32 = 0.0;
                    const UV_V0: f32 = 0.0;
                    let uv_u1 = h_f32;
                    let uv_v1 = w_f32;

                    let vertex_0 = Vertex::new(v0_pos[0], v0_pos[1], v0_pos[2], texture_index, v0_ao as f32, UV_U0, UV_V0); // Bottom left
                    let vertex_1 = Vertex::new(v1_pos[0], v1_pos[1], v1_pos[2], texture_index, v1_ao as f32, UV_U0, uv_v1); // Bottom right
                    let vertex_2 = Vertex::new(v2_pos[0], v2_pos[1], v2_pos[2], texture_index, v2_ao as f32, uv_u1, UV_V0); // Top left
                    let vertex_3 = Vertex::new(v3_pos[0], v3_pos[1], v3_pos[2], texture_index, v3_ao as f32, uv_u1, uv_v1); // Top right

                    let reverse_faces = face.get_face().is_positive();

                    // let flip = (v0_ao + v3_ao) <= (v1_ao + v2_ao);

                    // let (vertex_0, vertex_1, vertex_2, vertex_3) = match flip {
                    //     false => (vertex_0, vertex_1, vertex_2, vertex_3),
                    //     true => (vertex_1, vertex_0, vertex_3, vertex_2),
                    // };

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

    /// Updates a ChunkMesh with the data processed by [GreedyMeshingProcessor]'s threads.
    pub fn update(&mut self, vertices: &Vec<Vertex>, mesh_manager: &mut MeshManager) -> Result<(), AllocError> {
        // It's no longer dirty since we're updating it.
        self.dirty.store(false, Ordering::Relaxed);

        // Empty data, we free the mesh if it exists.
        if vertices.is_empty() {
            return match self.id {
                Some(mesh_id) => {
                    self.id = None;
                    mesh_manager.free(mesh_id)
                }
                None => Ok(()),
            };
        }

        let data = cast_slice(vertices);

        return match self.id {
            // Mesh already exists.
            Some(mesh_id) => mesh_manager.update(mesh_id, data),
            // Creates mesh with new data and associates it with the chunk mesh.
            None => {
                let mesh_id = mesh_manager.add(data)?;
                self.id = Some(mesh_id);
                Ok(())
            }
        };
    }
}
