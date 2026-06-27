//! Tokex.
//! A deterministic RTK orchestration layer: normalize agent intent, forward to RTK, normalize
//! the stream. Tokex does not own execution; RTK does.

mod agent;
mod cli;
mod config;
mod core;
mod dispatch;
mod graphify;
mod llm;
mod script;

use std::process::exit;

use clap::Parser;

use crate::cli::{Cli, SUBCOMMANDS};
use crate::core::intent::Intent;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let model_mode = matches!(
        args.get(1).map(String::as_str),
        Some("-m") | Some("--model")
    );
    let rest: &[String] = if model_mode { &args[2..] } else { &args[1..] };
    let mode = if model_mode {
        agent::prompt::Mode::Model
    } else {
        agent::prompt::Mode::User
    };

    if let Some(first) = rest.first() {
        if agent::prompt::role(first).is_some() {
            dispatch::run_role(first, rest[1..].join(" ").trim(), mode);
        }
        if is_passthrough(first) {
            if rest.len() >= 2 {
                dispatch::run_intent(Intent::from_command(rest.join(" ")));
            } else {
                dispatch::dispatch_one(&rest[0], mode);
            }
            return;
        }
        if model_mode {
            eprintln!("tokex: -m takes a prompt or role, not '{first}'");
            exit(2);
        }
    } else if model_mode {
        eprintln!("tokex: -m needs a prompt or role, e.g. tokex -m \"list rust projects\"");
        exit(2);
    }

    let cli = Cli::parse();

    let intent = match cli.cmd {
        Some(cmd) => match dispatch::dispatch_cmd(cmd) {
            Some(intent) => intent,
            None => return,
        },
        None => dispatch::read_stdin_intent(),
    };

    dispatch::run_intent(intent);
}

fn is_passthrough(first: &str) -> bool {
    !first.starts_with('-') && !SUBCOMMANDS.contains(&first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_routes_commands_not_subcommands() {
        assert!(is_passthrough("git"));
        assert!(is_passthrough("ls"));
        assert!(!is_passthrough("run"));
        assert!(!is_passthrough("setup"));
        assert!(!is_passthrough("--help"));
        assert!(!is_passthrough("-V"));
    }
}
