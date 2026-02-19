//! Greedy meshing implementation for `Chunk`.
//!
//! This module implements an axis-sweep greedy mesher that merges adjacent
//! exposed block faces into larger quads to drastically reduce geometry count
//! The algorithm scans each axis, builds a mask of
//! exposed faces for each slice, and greedily grows rectangular regions of
//! identical block types before emitting a single quad for each merged region.
//!
//! # Example
//! ```
//! // Illustrative only; actual code requires a prepared `AtlasUVMap` and block registry
//! use voxel_game::chunk::Chunk;
//! use voxel_game::atlas_builder::AtlasUVMap;
//! let chunk = Chunk::new();
//! let atlas = AtlasUVMap::default();
//! let (_mesh, tri_count) = chunk.build_mesh(&Default::default(), &atlas, 1);
//! println!("built mesh tris={}", tri_count);
//! ```

use super::{CHUNK_SIZE, Chunk};
use crate::atlas_builder::{AtlasUVMap, BlockFace};
use crate::block::{blocks, BlockId};

// Bundle all mutable mesh output buffers to reduce function arity.
pub(crate) struct MeshOutput<'a> {
    pub positions: &'a mut Vec<[f32; 3]>,
    pub normals: &'a mut Vec<[f32; 3]>,
    pub colors: &'a mut Vec<[f32; 4]>,
    pub uvs: &'a mut Vec<[f32; 2]>,
    pub uvs_b: &'a mut Vec<[f32; 2]>,
    pub indices: &'a mut Vec<u32>,
}

// Descriptor for a merged quad emitted by the mesher.
pub(crate) struct QuadDesc {
    slice: usize,
    col: usize,
    row: usize,
    width: usize,
    height: usize,
    axis: usize,
    direction: i32,
    face: BlockFace,
    block_id: BlockId,
}

// Small helper to group per-slice mask buffers so helper arity stays small.
struct SliceMask<'a> {
    mask: &'a mut [Option<BlockId>],
    done: &'a mut [bool],
}

// Bundle mesh inputs that are constant per-mesh so helpers accept fewer args.
struct MeshCtx {
    lod: u8,
}

impl Chunk {
    /// Perform greedy meshing along a single axis.
    /// Uses a compact `MeshOutput` bundle to keep the signature small.
    pub(crate) fn greedy_mesh_axis(
        &self,
        axis: usize,
        out: &mut MeshOutput,
        atlas_map: &AtlasUVMap,
        lod: u8,
        chunk_coords: (i32, i32),
        neighbors: Option<&std::collections::HashMap<(i32, i32), Chunk>>,
    ) {
        for direction in [1, -1] {
            let size = CHUNK_SIZE;

            // Reuse masks across slices to avoid reallocations
            let mut mask: Vec<Option<BlockId>> = vec![None; size * size];
            let mut done: Vec<bool> = vec![false; size * size];
            let mut collected_quads: Vec<QuadDesc> = Vec::new();

            for slice in 0..size {
                // Reset mask and done arrays
                for i in 0..(size * size) {
                    mask[i] = None;
                    done[i] = false;
                }

                // Delegate per-slice work to a helper to keep this function small.
                let mesh_ctx = MeshCtx { lod };
                let mut quads = Self::process_slice(
                    self,
                    axis,
                    slice,
                    direction,
                    &mut SliceMask { mask: &mut mask[..], done: &mut done[..] },
                    &mesh_ctx,
                    chunk_coords,
                    neighbors,
                );

                collected_quads.append(&mut quads);
            }

            // Run a second-pass coalescing step to merge adjacent coplanar quads across slices.
            Self::coalesce_and_emit_quads(axis, direction, &mut collected_quads, out, atlas_map);
        }
    }

    // Helper extracted from `greedy_mesh_axis` to reduce its line count.
    fn process_slice(
        &self,
        axis: usize,
        slice: usize,
        direction: i32,
        ctx: &mut SliceMask<'_>,
        mesh_ctx: &MeshCtx,
        chunk_coords: (i32, i32),
        neighbors: Option<&std::collections::HashMap<(i32, i32), Chunk>>,
    ) -> Vec<QuadDesc> {
        let size = CHUNK_SIZE;
        let u_axis = (axis + 1) % 3;
        let mut slice_quads: Vec<QuadDesc> = Vec::new();

        // Build mask for this slice
        for col in 0..size {
            for row in 0..size {
                let current = self.get(
                    if axis == 0 { slice } else if u_axis == 0 { col } else { row },
                    if axis == 1 { slice } else if u_axis == 1 { col } else { row },
                    if axis == 2 { slice } else if u_axis == 2 { col } else { row },
                );
                if current == blocks::AIR {
                    continue;
                }

                // Check if face is exposed
                let neighbor_pos = if direction == 1 { slice + 1 } else { slice.wrapping_sub(1) };
                let neighbor = if neighbor_pos < CHUNK_SIZE {
                    self.get(
                        if axis == 0 { neighbor_pos } else if u_axis == 0 { col } else { row },
                        if axis == 1 { neighbor_pos } else if u_axis == 1 { col } else { row },
                        if axis == 2 { neighbor_pos } else if u_axis == 2 { col } else { row },
                    )
                } else {
                    // Out-of-bounds neighbor: consult neighbor chunk snapshot if available
                    let mut substituted = blocks::DEFAULT;
                    if let Some(neigh_map) = neighbors {
                        // Map axis to chunk coordinate delta and local coords
                        let (cx, cz) = chunk_coords;
                        let (dx, dz, lx, ly, lz) = if axis == 0 {
                            // X axis: current mapping x=slice, y=col, z=row
                            let nx = if direction == 1 { cx + 1 } else { cx - 1 };
                            let local_x = if direction == 1 { 0 } else { CHUNK_SIZE - 1 };
                            (nx, cz, local_x, col, row)
                        } else if axis == 2 {
                            // Z axis: current mapping x=col, y=row, z=slice
                            let nz = if direction == 1 { cz + 1 } else { cz - 1 };
                            let local_z = if direction == 1 { 0 } else { CHUNK_SIZE - 1 };
                            (cx, nz, col, row, local_z)
                        } else {
                            // Y axis or unexpected: fall back to AIR
                            (cx, cz, 0usize, 0usize, 0usize)
                        };

                        // Only attempt lookup for X/Z neighbor cases
                        if axis == 0 {
                            if let Some(nchunk) = neigh_map.get(&(dx, dz)) {
                                substituted = nchunk.get(lx, ly, lz);
                            }
                        } else if axis == 2 {
                            if let Some(nchunk) = neigh_map.get(&(dx, dz)) {
                                substituted = nchunk.get(lx, ly, lz);
                            }
                        }
                    }
                    substituted
                };

                if neighbor == blocks::AIR {
                    ctx.mask[col + row * size] = Some(current);
                }
            }
        }

        // Make lower LODs more aggressive so distant terrain produces
        // substantially fewer quads. LOD index: 0 = full detail, higher
        // = coarser.
        let max_merge_size = match mesh_ctx.lod {
            0 => 1,              // No merging at LOD 0 (highest detail)
            1 => 8,              // Merge up to 8x8 at LOD 1 (more aggressive)
            2 => CHUNK_SIZE,     // LOD 2 = full-slice merges (very coarse)
            3 => CHUNK_SIZE,     // LOD 3+ remain full-slice
            _ => CHUNK_SIZE,
        };

        for row in 0..size {
            for col in 0..size {
                let idx = col + row * size;
                if ctx.done[idx] || ctx.mask[idx].is_none() {
                    continue;
                }

                let block_id = ctx.mask[idx].unwrap();

                //merge adjacent blocks of same type
                let mut width = 1;
                while col + width < size
                    && width < max_merge_size
                    && !ctx.done[col + width + row * size]
                    && ctx.mask[col + width + row * size] == Some(block_id)
                {
                    width += 1;
                }

                let mut height = 1;
                'outer: while row + height < size && height < max_merge_size {
                    for du in 0..width {
                        let check_idx = col + du + (row + height) * size;
                        if ctx.done[check_idx] || ctx.mask[check_idx] != Some(block_id) {
                            break 'outer;
                        }
                    }
                    height += 1;
                }

                // Mark merged region as done
                for dv in 0..height {
                    for du in 0..width {
                        ctx.done[col + du + (row + dv) * size] = true;
                    }
                }

                let desc = QuadDesc { slice, col, row, width, height, axis, direction, face: if axis == 1 { if direction == 1 { BlockFace::Top } else { BlockFace::Bottom } } else { BlockFace::Side }, block_id };
                slice_quads.push(desc);
            }
        }

        slice_quads
     }

    /// Coalesce collected `QuadDesc`s per plane and emit merged quads.
    ///
    /// Groups quad descriptors by their plane (coplanar quads have the same
    /// plane index) and runs a greedy 2D merge on each plane. Only quads
    /// with identical `BlockId` and `BlockFace` are merged (exact match).
    ///
    /// # Arguments
    /// * `axis` - The axis along which the quads were generated (0=X, 1=Y, 2=Z).
    /// * `direction` - The face direction (1=positive, -1=negative) of the quads.
    /// * `quads` - The list of `QuadDesc`s to coalesce and emit.
    /// * `out` - The `MeshOutput` bundle to append emitted quads to.
    /// * `atlas_map` - The `AtlasUVMap` for looking up UV coordinates
    fn coalesce_and_emit_quads(
        axis: usize,
        direction: i32,
        quads: &mut [QuadDesc],
        out: &mut MeshOutput,
        atlas_map: &AtlasUVMap,
    ) {
        use std::collections::HashMap;
        let size = CHUNK_SIZE;

        // Group quads by plane coordinate (plane = slice + (direction==1 ? 1 : 0)).
        let mut planes: HashMap<usize, Vec<&QuadDesc>> = HashMap::new();
        for q in quads.iter() {
            let plane = if q.direction == 1 { q.slice + 1 } else { q.slice };
            planes.entry(plane).or_default().push(q);
        }

        // For each plane, build a mask grid of merge-keys and run a greedy
        // 2D merge identical to the original per-slice merging logic.
        for (plane_idx, qlist) in planes.into_iter() {
            let mut mask: Vec<Option<(BlockId, BlockFace)>> = vec![None; size * size];
            for q in qlist.iter() {
                for r in q.row..(q.row + q.height) {
                    for c in q.col..(q.col + q.width) {
                        mask[c + r * size] = Some((q.block_id, q.face));
                    }
                }
            }

            let mut done: Vec<bool> = vec![false; size * size];

            for row in 0..size {
                for col in 0..size {
                    let idx = col + row * size;
                    if done[idx] || mask[idx].is_none() {
                        continue;
                    }

                    let (block_id, face) = mask[idx].unwrap();

                    // merge width
                    let mut width = 1;
                    while col + width < size
                        && !done[col + width + row * size]
                        && mask[col + width + row * size] == Some((block_id, face))
                    {
                        width += 1;
                    }

                    // merge height
                    let mut height = 1;
                    'outer_p: while row + height < size {
                        for du in 0..width {
                            let check_idx = col + du + (row + height) * size;
                            if done[check_idx] || mask[check_idx] != Some((block_id, face)) {
                                break 'outer_p;
                            }
                        }
                        height += 1;
                    }

                    for dv in 0..height {
                        for du in 0..width {
                            done[col + du + (row + dv) * size] = true;
                        }
                    }

                    // Map plane index back to a slice value for QuadDesc
                    let slice = if direction == 1 { plane_idx.saturating_sub(1) } else { plane_idx };
                    let desc = QuadDesc { slice, col, row, width, height, axis, direction, face, block_id };
                    Self::add_quad(&desc, out, atlas_map);
                }
            }
        }
    }

    /// Emit a single quad for a merged region.
    ///
    /// This is an associated function that accepts a compact `QuadDesc`
    /// and the `MeshOutput` bundle to reduce function arity.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub(crate) fn add_quad(desc: &QuadDesc, out: &mut MeshOutput, atlas_map: &AtlasUVMap) {
        // keep `add_quad` compact and readable.
        fn compute_corners(desc: &QuadDesc) -> [[f32; 3]; 4] {
            let axis = desc.axis;
            let u_axis = (axis + 1) % 3;
            let v_axis = (axis + 2) % 3;
            let mut corners = [[0.0f32; 3]; 4];
            let slice_val = if desc.direction == 1 { (desc.slice + 1) as f32 } else { desc.slice as f32 };
            corners[0][axis] = slice_val;
            corners[0][u_axis] = desc.col as f32;
            corners[0][v_axis] = desc.row as f32;
            corners[1][axis] = slice_val;
            corners[1][u_axis] = (desc.col + desc.width) as f32;
            corners[1][v_axis] = desc.row as f32;
            corners[2][axis] = slice_val;
            corners[2][u_axis] = (desc.col + desc.width) as f32;
            corners[2][v_axis] = (desc.row + desc.height) as f32;
            corners[3][axis] = slice_val;
            corners[3][u_axis] = desc.col as f32;
            corners[3][v_axis] = (desc.row + desc.height) as f32;
            corners
        }

        fn local_uv_for(desc: &QuadDesc, i: usize, width_f: f32, height_f: f32) -> [f32; 2] {
            // Map pushed-vertex index `i` to the original corner index from `corners`
            // so UVs remain correct regardless of winding (direction).
            let corner_idx = if desc.direction == 1 {
                i
            } else {
                // positions are pushed as [0, 3, 2, 1] when direction != 1
                match i {
                    0 => 0,
                    1 => 3,
                    2 => 2,
                    3 => 1,
                    _ => unreachable!(),
                }
            };

            // local (column,row) offset inside the merged quad
            let (local_x, local_y) = match corner_idx {
                0 => (0.0_f32, 0.0_f32),
                1 => (width_f, 0.0_f32),
                2 => (width_f, height_f),
                3 => (0.0_f32, height_f),
                _ => unreachable!(),
            };

            if desc.face == BlockFace::Side {
                // Decide which atlas-local axis corresponds to world-vertical (Y).
                // `u_axis = (axis + 1) % 3`, `v_axis = (axis + 2) % 3` in compute_corners.
                // If u_axis == 1 then `desc.col` maps to Y (vertical), otherwise `desc.row` does.
                let u_axis_is_vertical = ((desc.axis + 1) % 3) == 1;

                // Map local coords into atlas-local (u,v) then flip V so textures
                // are upright (corrects the upside-down issue reported).
                let (u_val, mut v_val) = if u_axis_is_vertical {
                    (local_y, local_x)
                } else {
                    (local_x, local_y)
                };

                // Flip vertical (V) so the texture top aligns with world-up.
                v_val = height_f - v_val;

                [u_val, v_val]
            } else {
                // Top/Bottom faces use the default orientation
                [local_x, local_y]
            }
        }

        let corners = compute_corners(desc);
        let color = [1.0f32, 1.0f32, 1.0f32, 1.0f32];

        // Safe to cast length -> u32 for mesh indices: meshes don't exceed u32 indices in practice.
        debug_assert!(u32::try_from(out.positions.len()).is_ok());
        let start = out.positions.len() as u32;

        let mut normal = [0.0f32; 3];
        normal[desc.axis] = desc.direction as f32;

        let uv_bounds = atlas_map.get_face_uvs(desc.block_id, desc.face);
        let uv_range = atlas_map.uv_range;


        let quad_size = desc.width.max(desc.height) as f32;
        let width_f = desc.width as f32;
        let height_f = desc.height as f32;

        if desc.direction == 1 {
            out.positions.extend_from_slice(&corners);
            out.indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);

            for i in 0..4 {
                out.normals.push(normal);
                out.colors.push(color); // always push color (default is common case)
                out.uvs_b.push([uv_range, quad_size]);

                let local_uv = local_uv_for(desc, i, width_f, height_f);
                let atlas_u = uv_bounds.min_u + (local_uv[0] / quad_size) * uv_range;
                let atlas_v = uv_bounds.min_v + (local_uv[1] / quad_size) * uv_range;
                out.uvs.push([atlas_u, atlas_v]);
            }
        } else {
            // back face winding
            out.positions.push(corners[0]);
            out.positions.push(corners[3]);
            out.positions.push(corners[2]);
            out.positions.push(corners[1]);
            out.indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);

            for i in 0..4 {
                out.normals.push(normal);
                out.colors.push(color);
                out.uvs_b.push([uv_range, quad_size]);

                let local_uv = local_uv_for(desc, i, width_f, height_f);
                let atlas_u = uv_bounds.min_u + (local_uv[0] / quad_size) * uv_range;
                let atlas_v = uv_bounds.min_v + (local_uv[1] / quad_size) * uv_range;
                out.uvs.push([atlas_u, atlas_v]);
            }
        }
    }
}
