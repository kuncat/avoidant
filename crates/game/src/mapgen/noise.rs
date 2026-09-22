//! Layered 3D value noise used for per-vertex terrain displacement.
//!
//! Evaluating in 3D world space (rather than per-face 2D) is critical: it
//! guarantees that two adjacent polyhedron faces evaluating the noise at the
//! exact same point on their shared edge get identical displacement values,
//! so the rendered seam is wiggly but seamless.

use rand::RngCore;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

/// Layered value noise sampled in world space.
///
/// # Arguments
/// * `(x, y, z)` - World-space coordinates of the point to evaluate.
/// * `seed` - Deterministic seed controlling the generated pattern.
/// * `spikiness` - Shape control in `[0.0, 1.0]`; lower is smoother/broader,
///   higher is tighter/spikier.
/// * `elevation_range` - Output bounds as `(min, max)`.
pub(crate) fn vertex_height(
    x: f64,
    y: f64,
    z: f64,
    seed: u64,
    spikiness: f64,
    elevation_range: (f64, f64),
) -> f64 {
    let (elev_min, elev_max) = elevation_range;
    let mid = (elev_min + elev_max) / 2.0;
    let amplitude = (elev_max - elev_min) / 2.0;
    let scale = 25.0 - 22.0 * spikiness;
    let detail_scale = scale / 3.0;
    let detail_amplitude = amplitude * 0.08;
    mid + value_noise_3d(x, y, z, seed, scale) * amplitude
        + value_noise_3d(x, y, z, seed ^ 0x9e37_79b9_7f4a_7c15, detail_scale) * detail_amplitude
}

fn value_noise_3d(x: f64, y: f64, z: f64, seed: u64, scale: f64) -> f64 {
    let fx = x / scale;
    let fy = y / scale;
    let fz = z / scale;

    let x0 = fx.floor() as i64;
    let y0 = fy.floor() as i64;
    let z0 = fz.floor() as i64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;

    let tx = smoothstep(fx - x0 as f64);
    let ty = smoothstep(fy - y0 as f64);
    let tz = smoothstep(fz - z0 as f64);

    let c000 = lattice_random(x0, y0, z0, seed);
    let c100 = lattice_random(x1, y0, z0, seed);
    let c010 = lattice_random(x0, y1, z0, seed);
    let c110 = lattice_random(x1, y1, z0, seed);
    let c001 = lattice_random(x0, y0, z1, seed);
    let c101 = lattice_random(x1, y0, z1, seed);
    let c011 = lattice_random(x0, y1, z1, seed);
    let c111 = lattice_random(x1, y1, z1, seed);

    let x00 = lerp(c000, c100, tx);
    let x10 = lerp(c010, c110, tx);
    let x01 = lerp(c001, c101, tx);
    let x11 = lerp(c011, c111, tx);
    let y0_ = lerp(x00, x10, ty);
    let y1_ = lerp(x01, x11, ty);
    lerp(y0_, y1_, tz)
}

fn lattice_random(ix: i64, iy: i64, iz: i64, seed: u64) -> f64 {
    let mixed = seed
        ^ (ix as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (iy as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ (iz as u64).wrapping_mul(0x165667b1_9e3779f9);
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(mixed);
    next_unit_f64(&mut rng) * 2.0 - 1.0
}

fn next_unit_f64(rng: &mut impl RngCore) -> f64 {
    let value = rng.next_u64() >> 11;
    (value as f64) * (1.0 / ((1u64 << 53) as f64))
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::vertex_height;

    #[test]
    fn vertex_height_is_reproducible() {
        let h1 = vertex_height(12.345, 67.89, -4.2, 4242, 0.4, (-0.4, 0.4));
        let h2 = vertex_height(12.345, 67.89, -4.2, 4242, 0.4, (-0.4, 0.4));
        let h3 = vertex_height(12.345, 67.89, -4.2, 4243, 0.4, (-0.4, 0.4));

        assert_eq!(h1, h2);
        assert!((h1 - h3).abs() > f64::EPSILON);
    }
}
