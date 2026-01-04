//! Algorithm implementations for VDOM operations.
//!
//! - `diff`: VDOM diff algorithm with edit operations
//! - `myers`: Myers diff algorithm for efficient LCS
//! - `hash`: Stable hashing utilities

mod diff;
mod hash;
mod myers;

pub use diff::{diff, diff_with_config, Anchor, DiffConfig, DiffResult, DiffStats, Patch, PatchOp};
pub use hash::StableHasher;
// Use Myers algorithm (better for hot reload scenarios)
pub use myers::{diff_sequences, Edit, LcsResult, LcsStats};
