//! Script runner: run idempotent agent scripts through RTK and verify via git diff.

mod script;

pub use script::{ensure_dir, run, INSTRUCTIONS};
