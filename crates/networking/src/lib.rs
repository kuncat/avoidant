//! Native networking crate for future iroh integration.
//!
//! Keep wasm-facing code in the root crate and put iroh-backed logic here.

use iroh::SecretKey;

/// Placeholder type to anchor the crate and verify native iroh linkage.
pub struct NetIdentity {
    pub secret_key: SecretKey,
}
