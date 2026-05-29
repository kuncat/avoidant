//! Constants shared between the Rust game crate and the JS frontend.

pub const MAP_WIDTH: f64 = 100.0;
pub const MAP_HEIGHT: f64 = 100.0;
pub const MAP_AREA: f64 = MAP_WIDTH * MAP_HEIGHT;

/// Width of the pulse sweep's soft edge as a fraction of pulse progress (0.0–1.0). Used by the fragment shader's smoothstep band and by the chord auto-reveal scheduler to time per-cell finalization.
pub const PULSE_SWEEP_BAND: f64 = 0.12;

/// World-space velocity of the pulse sweep band, in world units per millisecond. Each pulse's duration is derived from `max_radius / PULSE_SWEEP_VELOCITY`.
pub const PULSE_SWEEP_VELOCITY: f64 = 0.1;

/// Lower bound on a pulse's duration in milliseconds. Without a floor, single-cell reveals on dense maps would compute a sub-frame duration and visually flash without any sweep.
pub const PULSE_MIN_DURATION_MS: u32 = 80;
