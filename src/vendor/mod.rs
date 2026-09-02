//! Code vendored from https://github.com/acsandmann/rift (Apache-2.0).
//!
//! Upstream rev: b67cf2efc447174ca9e0cd10f558a224ed32b038 (tag v0.5.5)
//!
//! This arrangement is permanent. acsandmann/rift#467 asked for these
//! primitives as a published crate and was closed: vendoring is how rift's CG
//! bindings are meant to be distributed. `rift-client` is IPC-only by design.
//!
//! On a rift upgrade: bump the rev in Cargo.toml, then re-diff these files
//! against upstream at the new tag.
//!
//! Do not add project logic here — only import rewrites and removals.

// Vendored verbatim: upstream keeps helpers this project does not call
// (blur, sublevels, region helpers). Allowing dead code keeps the files
// diffable against upstream instead of trimmed to fit.
#[allow(dead_code)]
pub mod cgs_window;
#[allow(dead_code)]
pub mod render_layer;
#[allow(dead_code)]
pub mod skylight;
