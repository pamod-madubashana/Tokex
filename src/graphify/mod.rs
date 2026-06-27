//! Code knowledge graph: auto-refresh after code-changing runs.

mod graphify;

pub use graphify::{
    auto_update, bootstrap_detached, clear_skill_marker, current_agent, setup_steps,
    update_blocking,
};
