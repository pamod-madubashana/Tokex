//! Script runner: run idempotent agent scripts through RTK and verify via git diff.

#[allow(clippy::module_inception)]
mod script;

pub use script::{ensure_dir, run, INSTRUCTIONS};
