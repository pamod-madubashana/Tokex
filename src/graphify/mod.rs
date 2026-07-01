//! Code knowledge graph: auto-refresh after code-changing runs.

#[allow(clippy::module_inception)]
mod graphify;

pub use graphify::{
    add_url, auto_update, bootstrap_detached, clear_skill_marker, cluster_only, current_agent,
    explain_node, export_graphml, export_neo4j, export_svg, path_between, push_neo4j, query_graph,
    save_result, setup_steps, update_blocking, watch,
};
