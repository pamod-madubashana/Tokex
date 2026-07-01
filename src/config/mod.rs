//! Persistent configuration, installation, and self-update.

pub mod download;
pub mod embedded;
pub mod embedded_graphify;
pub mod install;
pub mod install_agent;
pub mod settings;
pub mod update;

pub use settings::{load, run_setup, save, Config};
