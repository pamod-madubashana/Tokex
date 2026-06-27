//! Task dispatch: route intents, commands, and prompts to the execution core.

mod dispatch;

pub use dispatch::{dispatch_cmd, dispatch_one, read_stdin_intent, run_intent, run_role};
