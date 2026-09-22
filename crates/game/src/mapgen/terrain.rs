//! Build a flat-shaded/per-vertex-normal terrain triangle mesh from a list of
//! 3D Voronoi cells (each is a convex polygon with vertices on or near the
//! shape surface). Each cell is fan-triangulated from a per-cell anchor
//! (centroid), then each fan triangle is barycentrically
//! subdivided so terrain detail is decoupled from cell-corner density.

use crate::TerrainTriangles;

use super::noise::vertex_height;
use super::vec3::{V3, add, cross, dot, normalize, scale, sub};

/// Per-cell input to mesh generation.
pub(crate) struct MeshCell {
    /// 3D vertices on the (un-displaced) shape surface, CCW around the
    /// outward normal.
    pub vertices: Vec<V3>,
    /// Outward unit normal used to orient lighting and tangent derivatives.
    pub normal: V3,
    /// Cell centroid on the un-displaced surface (fan apex).
    pub centroid: V3,
}

/// Subdivision factor: each fan triangle inside a cell becomes `s²` sub-tris.
pub(crate) fn generate_terrain_triangles(
    cells: &[MeshCell],
    rng_seed: u64,
    spikiness: f64,
    elevation_range: (f64, f64),
    subdivisions: u32,
    flat: bool,
) -> TerrainTriangles {
    let s = subdivisions.max(1);
    let s_f = s as f64;

    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut cell_indices: Vec<u32> = Vec::new();
    let mut heights: Vec<f32> = Vec::new();

    // Finite-difference step for analytic normals. We perturb along two
    // tangent vectors in the cell's surface plane (perpendicular to the
    // cell normal) and recompute heights — the surface normal is then the
    // cross product of the two tangents.
    let eps = 0.05_f64;

    let direction = |p| if flat { [0.0, 1.0, 0.0] } else { normalize(p) };
    let sample = |p_surface: V3, normal: V3| -> ([f32; 3], [f32; 3], f32) {
        let helper: V3 = if normal[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let tu = normalize(cross(normal, helper));
        let tv = normalize(cross(normal, tu));

        let h = vertex_height(
            p_surface[0],
            p_surface[1],
            p_surface[2],
            rng_seed,
            spikiness,
            elevation_range,
        );
        // A shared point must move identically across faces and cells.
        // Face normals differ at seams; the radial direction does not.
        let p = add(p_surface, scale(direction(p_surface), h));

        let pu = add(p_surface, scale(tu, eps));
        let hu = vertex_height(pu[0], pu[1], pu[2], rng_seed, spikiness, elevation_range);
        let pu_disp = add(pu, scale(direction(pu), hu));

        let pv = add(p_surface, scale(tv, eps));
        let hv = vertex_height(pv[0], pv[1], pv[2], rng_seed, spikiness, elevation_range);
        let pv_disp = add(pv, scale(direction(pv), hv));

        // The displaced surface's tangents at `p`.
        let du = sub(pu_disp, p);
        let dv = sub(pv_disp, p);
        // Cross product gives a normal; orient outward via `normal`.
        let mut n = cross(du, dv);
        if dot(n, normal) < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let n = normalize(n);
        (
            [p[0] as f32, p[1] as f32, p[2] as f32],
            [n[0] as f32, n[1] as f32, n[2] as f32],
            h as f32,
        )
    };

    for (cell_idx, cell) in cells.iter().enumerate() {
        if cell.vertices.len() < 3 {
            continue;
        }
        let cell_idx_u32 = cell_idx as u32;
        let a = cell.centroid;
        let normal = cell.normal;

        let verts = &cell.vertices;
        for j in 0..verts.len() {
            let b = verts[j];
            let c = verts[(j + 1) % verts.len()];

            // Lattice walk over (i, k) with i+k <= s. Weights:
            // a -> (s-i-k)/s, b -> k/s, c -> i/s.
            for i in 0..s {
                for k in 0..(s - i) {
                    let p00 = bary(a, b, c, i, k, s_f);
                    let p01 = bary(a, b, c, i, k + 1, s_f);
                    let p10 = bary(a, b, c, i + 1, k, s_f);

                    let (v00, n00, h00) = sample(p00, normal);
                    let (v01, n01, h01) = sample(p01, normal);
                    let (v10, n10, h10) = sample(p10, normal);
                    positions.extend_from_slice(&v00);
                    positions.extend_from_slice(&v01);
                    positions.extend_from_slice(&v10);
                    normals.extend_from_slice(&n00);
                    normals.extend_from_slice(&n01);
                    normals.extend_from_slice(&n10);
                    heights.extend_from_slice(&[h00, h01, h10]);
                    cell_indices.extend_from_slice(&[cell_idx_u32; 3]);

                    if i + k + 1 < s {
                        let p11 = bary(a, b, c, i + 1, k + 1, s_f);
                        let (v11, n11, h11) = sample(p11, normal);
                        positions.extend_from_slice(&v01);
                        positions.extend_from_slice(&v11);
                        positions.extend_from_slice(&v10);
                        normals.extend_from_slice(&n01);
                        normals.extend_from_slice(&n11);
                        normals.extend_from_slice(&n10);
                        heights.extend_from_slice(&[h01, h11, h10]);
                        cell_indices.extend_from_slice(&[cell_idx_u32; 3]);
                    }
                }
            }
        }
    }

    TerrainTriangles {
        positions,
        normals,
        cell_indices,
        heights,
    }
}

fn bary(a: V3, b: V3, c: V3, i: u32, k: u32, s: f64) -> V3 {
    let wa = (s - i as f64 - k as f64) / s;
    let wb = (k as f64) / s;
    let wc = (i as f64) / s;
    [
        wa * a[0] + wb * b[0] + wc * c[0],
        wa * a[1] + wb * b[1] + wc * c[1],
        wa * a[2] + wb * b[2] + wc * c[2],
    ]
}
