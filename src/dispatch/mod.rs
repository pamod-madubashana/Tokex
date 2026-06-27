//! Task dispatch: route intents, commands, and prompts to the execution core.

pub mod cli;
mod dispatch;

pub use dispatch::run;
