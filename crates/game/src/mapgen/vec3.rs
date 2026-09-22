//! Minimal 3D vector helpers (no `glam` dep). Operates on `[f64; 3]`.

pub(crate) type V3 = [f64; 3];

#[inline]
pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub(crate) fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
pub(crate) fn length(a: V3) -> f64 {
    dot(a, a).sqrt()
}

#[inline]
pub(crate) fn normalize(a: V3) -> V3 {
    let len = length(a);
    if len < 1e-12 {
        [0.0, 0.0, 0.0]
    } else {
        scale(a, 1.0 / len)
    }
}
