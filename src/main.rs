//! Cotrex.
//! A deterministic RTK orchestration layer: normalize agent intent, forward to RTK, normalize
//! the stream. Cotrex does not own execution; RTK does.

mod agent;
mod config;
mod core;
mod dispatch;
mod graphify;
mod llm;
mod script;

fn main() {
    // All routing lives in dispatch — main.rs is just the module tree.
    dispatch::run();
}
