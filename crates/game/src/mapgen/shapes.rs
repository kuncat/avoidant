//! Platonic solid + geodesic icosphere construction.
//!
//! All shapes are returned at unit-vertex-radius then scaled.
//!
//! Platonic solids are built generically from their vertex set via 3D convex
//! hull (`chull`) followed by a coplanar-merge pass that groups adjacent
//! triangles with near-identical outward normals into a single polygonal face.
//! This gives quads for the cube and pentagons for the dodecahedron without
//! hand-maintaining a face table.
//!
//! The geodesic icosphere is built directly by subdividing icosahedron
//! triangles (each iteration: split each triangle into 4 via midpoints, then
//! re-project new vertices onto the sphere of `radius`). Its faces stay
//! triangles — adjacent subdivided triangles on the same original icosahedron
//! face have slightly different normals once projected, so they correctly
//! remain separate faces.

use chull::ConvexHullWrapper;
use std::collections::{HashMap, HashSet};

use super::vec3::{V3, add, cross, dot, normalize, scale, sub};

/// A polyhedron mesh: a list of vertices and a list of faces, where each face
/// references vertices by index. Faces are coplanar convex polygons (triangle,
/// quad, or pentagon) with vertices in CCW order when viewed from outside.
pub(crate) struct Polyhedron {
    pub vertices: Vec<V3>,
    pub faces: Vec<Vec<u32>>,
}

const PHI: f64 = 1.618_033_988_749_895;
const INV_PHI: f64 = 1.0 / PHI;

pub(crate) fn tetrahedron(radius: f64) -> Polyhedron {
    let verts = scale_to_sphere(
        &[
            [1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
        ],
        radius,
    );
    polyhedron_from_convex_hull(verts).expect("tetrahedron hull")
}

pub(crate) fn octahedron(radius: f64) -> Polyhedron {
    let verts = scale_to_sphere(
        &[
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ],
        radius,
    );
    polyhedron_from_convex_hull(verts).expect("octahedron hull")
}

pub(crate) fn cube(radius: f64) -> Polyhedron {
    let verts = scale_to_sphere(
        &[
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
        radius,
    );
    polyhedron_from_convex_hull(verts).expect("cube hull")
}

pub(crate) fn icosahedron(radius: f64) -> Polyhedron {
    let verts = scale_to_sphere(
        &[
            [-1.0, PHI, 0.0],
            [1.0, PHI, 0.0],
            [-1.0, -PHI, 0.0],
            [1.0, -PHI, 0.0],
            [0.0, -1.0, PHI],
            [0.0, 1.0, PHI],
            [0.0, -1.0, -PHI],
            [0.0, 1.0, -PHI],
            [PHI, 0.0, -1.0],
            [PHI, 0.0, 1.0],
            [-PHI, 0.0, -1.0],
            [-PHI, 0.0, 1.0],
        ],
        radius,
    );
    polyhedron_from_convex_hull(verts).expect("icosahedron hull")
}

pub(crate) fn dodecahedron(radius: f64) -> Polyhedron {
    let mut raw: Vec<V3> = Vec::with_capacity(20);
    for sx in [-1.0_f64, 1.0] {
        for sy in [-1.0_f64, 1.0] {
            for sz in [-1.0_f64, 1.0] {
                raw.push([sx, sy, sz]);
            }
        }
    }
    for sa in [-1.0_f64, 1.0] {
        for sb in [-1.0_f64, 1.0] {
            raw.push([0.0, sa * INV_PHI, sb * PHI]);
            raw.push([sa * INV_PHI, sb * PHI, 0.0]);
            raw.push([sb * PHI, 0.0, sa * INV_PHI]);
        }
    }
    let verts = scale_to_sphere(&raw, radius);
    polyhedron_from_convex_hull(verts).expect("dodecahedron hull")
}

/// Subdivide the icosahedron `subdivisions` times. Each iteration replaces
/// every triangular face with 4 sub-triangles split at midpoints, with new
/// vertices reprojected onto the sphere of `radius`.
pub(crate) fn geodesic_icosphere(radius: f64, subdivisions: u32) -> Polyhedron {
    let mut poly = icosahedron(radius);
    for _ in 0..subdivisions {
        poly = subdivide_triangular(&poly, radius);
    }
    poly
}

fn scale_to_sphere(raw: &[V3], radius: f64) -> Vec<V3> {
    raw.iter().map(|v| scale(normalize(*v), radius)).collect()
}

/// Build a convex polyhedron from a set of vertices on its hull, grouping
/// coplanar adjacent triangles into single polygonal faces.
///
/// Steps:
/// 1. 3D convex hull → list of triangles.
/// 2. Reorient each triangle to CCW-outward.
/// 3. Union-find islands of triangles whose normals match across a shared
///    edge (cosine of angle > 0.9999 ≈ coplanar).
/// 4. For each island, extract the boundary as a CCW polygon (any directed
///    edge whose reverse is absent in the island is on the boundary).
fn polyhedron_from_convex_hull(vertices: Vec<V3>) -> Result<Polyhedron, String> {
    let pts: Vec<Vec<f64>> = vertices.iter().map(|v| vec![v[0], v[1], v[2]]).collect();
    let hull =
        ConvexHullWrapper::try_new(&pts, None).map_err(|e| format!("convex hull failed: {e:?}"))?;
    let (_, indices) = hull.vertices_indices();
    if indices.len() % 3 != 0 {
        return Err("convex hull returned non-triangle indices".into());
    }
    let tris: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|c| [c[0] as u32, c[1] as u32, c[2] as u32])
        .collect();

    let mut used: HashSet<u32> = HashSet::new();
    for t in &tris {
        used.insert(t[0]);
        used.insert(t[1]);
        used.insert(t[2]);
    }
    let mut centroid = [0.0_f64; 3];
    for &i in &used {
        centroid = add(centroid, vertices[i as usize]);
    }
    centroid = scale(centroid, 1.0 / used.len() as f64);

    let mut tris_oriented: Vec<[u32; 3]> = Vec::with_capacity(tris.len());
    let mut tri_normals: Vec<V3> = Vec::with_capacity(tris.len());
    for t in &tris {
        let a = vertices[t[0] as usize];
        let b = vertices[t[1] as usize];
        let c = vertices[t[2] as usize];
        let face_centroid = scale(add(add(a, b), c), 1.0 / 3.0);
        let outward = sub(face_centroid, centroid);
        let mut n = normalize(cross(sub(b, a), sub(c, a)));
        let mut t_oriented = *t;
        if dot(n, outward) < 0.0 {
            t_oriented = [t[0], t[2], t[1]];
            n = [-n[0], -n[1], -n[2]];
        }
        tris_oriented.push(t_oriented);
        tri_normals.push(n);
    }

    let mut edge_to_tris: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, t) in tris_oriented.iter().enumerate() {
        for k in 0..3 {
            let a = t[k];
            let b = t[(k + 1) % 3];
            let key = if a < b { (a, b) } else { (b, a) };
            edge_to_tris.entry(key).or_default().push(i);
        }
    }

    let mut parent: Vec<usize> = (0..tris_oriented.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for tri_list in edge_to_tris.values() {
        if tri_list.len() != 2 {
            continue;
        }
        let (i, j) = (tri_list[0], tri_list[1]);
        if dot(tri_normals[i], tri_normals[j]) > 0.9999 {
            let ri = find(&mut parent, i);
            let rj = find(&mut parent, j);
            if ri != rj {
                parent[ri] = rj;
            }
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..tris_oriented.len() {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }

    let mut faces: Vec<Vec<u32>> = Vec::with_capacity(groups.len());
    for members in groups.values() {
        let face = extract_face_boundary(members, &tris_oriented)?;
        faces.push(face);
    }

    faces.sort();
    Ok(Polyhedron { vertices, faces })
}

fn extract_face_boundary(members: &[usize], tris: &[[u32; 3]]) -> Result<Vec<u32>, String> {
    let mut directed: HashMap<u32, u32> = HashMap::new();
    let mut all_directed: HashSet<(u32, u32)> = HashSet::new();
    for &i in members {
        let t = tris[i];
        for k in 0..3 {
            all_directed.insert((t[k], t[(k + 1) % 3]));
        }
    }
    for &(a, b) in &all_directed {
        if !all_directed.contains(&(b, a)) {
            directed.insert(a, b);
        }
    }
    if directed.is_empty() {
        return Err("coplanar group has no boundary edges".into());
    }
    let start = *directed.keys().min().unwrap();
    let mut cycle: Vec<u32> = vec![start];
    let mut current = start;
    loop {
        let Some(&n) = directed.get(&current) else {
            return Err("boundary walk encountered missing next edge".into());
        };
        if n == start {
            break;
        }
        cycle.push(n);
        current = n;
        if cycle.len() > 1024 {
            return Err("boundary walk did not terminate".into());
        }
    }
    Ok(cycle)
}

fn subdivide_triangular(poly: &Polyhedron, radius: f64) -> Polyhedron {
    let mut verts = poly.vertices.clone();
    let mut midpoint_cache: HashMap<(u32, u32), u32> = HashMap::new();
    let mut new_faces: Vec<Vec<u32>> = Vec::with_capacity(poly.faces.len() * 4);

    let mut midpoint = |a: u32, b: u32, verts: &mut Vec<V3>| -> u32 {
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(&idx) = midpoint_cache.get(&key) {
            return idx;
        }
        let mid = scale(add(verts[a as usize], verts[b as usize]), 0.5);
        let projected = scale(normalize(mid), radius);
        let idx = verts.len() as u32;
        verts.push(projected);
        midpoint_cache.insert(key, idx);
        idx
    };

    for face in &poly.faces {
        assert_eq!(face.len(), 3, "geodesic subdivision expects triangle faces");
        let v0 = face[0];
        let v1 = face[1];
        let v2 = face[2];
        let m01 = midpoint(v0, v1, &mut verts);
        let m12 = midpoint(v1, v2, &mut verts);
        let m20 = midpoint(v2, v0, &mut verts);
        new_faces.push(vec![v0, m01, m20]);
        new_faces.push(vec![v1, m12, m01]);
        new_faces.push(vec![v2, m20, m12]);
        new_faces.push(vec![m01, m12, m20]);
    }

    Polyhedron {
        vertices: verts,
        faces: new_faces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face_centroid(verts: &[V3], face: &[u32]) -> V3 {
        let mut sum = [0.0, 0.0, 0.0];
        for &i in face {
            sum = add(sum, verts[i as usize]);
        }
        scale(sum, 1.0 / face.len() as f64)
    }

    fn check_outward(poly: &Polyhedron) {
        for face in &poly.faces {
            assert!(face.len() >= 3);
            let centroid = face_centroid(&poly.vertices, face);
            let e1 = sub(
                poly.vertices[face[1] as usize],
                poly.vertices[face[0] as usize],
            );
            let e2 = sub(
                poly.vertices[face[2] as usize],
                poly.vertices[face[1] as usize],
            );
            let n = cross(e1, e2);
            assert!(
                dot(n, centroid) > 0.0,
                "face winding not outward: face={face:?}"
            );
        }
    }

    #[test]
    fn tetrahedron_has_four_triangles() {
        let p = tetrahedron(1.0);
        assert_eq!(p.faces.len(), 4);
        for f in &p.faces {
            assert_eq!(f.len(), 3);
        }
        check_outward(&p);
    }
    #[test]
    fn octahedron_has_eight_triangles() {
        let p = octahedron(1.0);
        assert_eq!(p.faces.len(), 8);
        for f in &p.faces {
            assert_eq!(f.len(), 3);
        }
        check_outward(&p);
    }
    #[test]
    fn cube_has_six_quads() {
        let p = cube(1.0);
        assert_eq!(p.faces.len(), 6);
        for f in &p.faces {
            assert_eq!(f.len(), 4);
        }
        check_outward(&p);
    }
    #[test]
    fn icosahedron_has_twenty_triangles() {
        let p = icosahedron(1.0);
        assert_eq!(p.faces.len(), 20);
        for f in &p.faces {
            assert_eq!(f.len(), 3);
        }
        check_outward(&p);
    }
    #[test]
    fn dodecahedron_has_twelve_pentagons() {
        let p = dodecahedron(1.0);
        assert_eq!(p.faces.len(), 12);
        for f in &p.faces {
            assert_eq!(f.len(), 5);
        }
        check_outward(&p);
    }
    #[test]
    fn geodesic_subdivision_counts() {
        assert_eq!(geodesic_icosphere(1.0, 0).faces.len(), 20);
        assert_eq!(geodesic_icosphere(1.0, 1).faces.len(), 80);
        assert_eq!(geodesic_icosphere(1.0, 2).faces.len(), 320);
    }
}
