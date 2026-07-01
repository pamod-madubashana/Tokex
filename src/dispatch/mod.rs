//! Task dispatch: route intents, commands, and prompts to the execution core.

pub mod cli;
#[allow(clippy::module_inception)]
mod dispatch;

pub use dispatch::run;
