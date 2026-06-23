//! Tokex.
//! A deterministic RTK orchestration layer: normalize agent intent, forward to RTK, normalize
//! the stream. Tokex does not own execution; RTK does.

mod config;
mod graphify;
mod install;
mod intent;
mod llm;
mod mcp;
mod normalize;
mod orchestrate;
mod prompt;
mod script;

use std::io::{self, IsTerminal, Read};
use std::process::exit;

use clap::{CommandFactory, Parser, Subcommand};

use intent::Intent;

#[derive(Parser)]
#[command(
    name = "tokex",
    version,
    about = "Deterministic RTK orchestration layer for AI agents",
    after_help = "Stdin mode: pipe a JSON intent instead of a subcommand, e.g.\n  echo '{\"tool\":\"rtk\",\"cmd\":\"git status\"}' | tokex"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a command through RTK and stream normalized events.
    Run {
        /// Force the LLM insight on for this run (overrides the configured compression mode).
        #[arg(long)]
        llm: bool,
        /// The command line, e.g. "cargo test".
        command: String,
    },
    /// Run a script from Scripts/ through rtk and verify with git diff. For a repetitive or
    /// multi-file change, write one script here instead of editing files individually.
    Script {
        /// Path to the script (e.g. Scripts/rename.sh). Omit to create Scripts/ and print the flow.
        file: Option<String>,
    },
    /// Interactive setup: choose provider, enter API key, pick modes.
    Setup,
    /// Run as an MCP server over stdio (for agents that call tools natively).
    Mcp,
    /// Pre-fetch the pinned rtk release for this OS (also happens automatically on first run).
    InstallRtk,
    /// Refresh the graphify code map now (`graphify update .`).
    Graph,
}

/// Top-level subcommands. Anything else as the first arg is treated as a command to run, so
/// `tokex git status` works like `tokex run "git status"` (mirrors how rtk itself is invoked).
const SUBCOMMANDS: &[&str] = &["run", "script", "setup", "mcp", "install-rtk", "graph", "help"];

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Model mode: `tokex -m "<prompt>"` — output only, no spinner/thinking (for agents).
    if matches!(args.get(1).map(String::as_str), Some("-m") | Some("--model")) {
        let prompt = args[2..].join(" ");
        if prompt.trim().is_empty() {
            eprintln!("tokex: -m needs a prompt, e.g. tokex -m \"list rust projects\"");
            exit(2);
        }
        dispatch_one(&prompt, prompt::Mode::Model);
        return;
    }

    // Two bare forms when the first arg isn't a subcommand/flag (User mode):
    //   tokex git status                     -> several args -> a command, run through rtk
    //   tokex "list all rust projects"       -> one arg      -> a prompt (see prompt::classify)
    // Quoting is the signal: a quoted string is one arg, so `tokex "git status"` is a prompt.
    if args.get(1).is_some_and(|f| is_passthrough(f)) {
        let rest = &args[1..];
        if rest.len() >= 2 {
            run_intent(Intent::from_command(rest.join(" ")));
        } else {
            dispatch_one(&rest[0], prompt::Mode::User);
        }
        return;
    }

    let cli = Cli::parse();

    let intent = match cli.cmd {
        Some(Cmd::Run { llm, command }) => {
            let mut i = Intent::from_command(command);
            i.llm = llm;
            i
        }
        Some(Cmd::Script { file }) => {
            let cfg = config::load();
            let opts = orchestrate::Options {
                raw: cfg.compression == "off",
                ultra_compact: cfg.rtk_verbosity == "ultra-compact",
                llm_on_failure: false, // scripts are verified by their diff, not a model insight
                footer: true,
            };
            let mut out = io::stdout();
            let mut err = io::stderr();
            match file {
                Some(f) => match script::run(&f, &mut out, &mut err, &opts) {
                    Ok(code) => exit(code),
                    Err(e) => {
                        eprintln!("tokex: {e}");
                        exit(1);
                    }
                },
                None => {
                    if let Err(e) = script::ensure_dir() {
                        eprintln!("tokex: cannot create Scripts/: {e}");
                        exit(1);
                    }
                    eprintln!("{}", script::INSTRUCTIONS);
                    return;
                }
            }
        }
        Some(Cmd::Setup) => {
            if let Err(e) = config::run_setup() {
                eprintln!("tokex: setup failed: {e}");
                exit(1);
            }
            // Bootstrap graphify (install + register skill for the chosen agent + build map) now.
            if config::load().graph_auto {
                if let Err(e) = graphify::update_blocking_after_setup() {
                    eprintln!("tokex: graphify setup skipped: {e}");
                }
            }
            return;
        }
        Some(Cmd::Mcp) => mcp::serve(),
        Some(Cmd::InstallRtk) => {
            match install::install() {
                Ok(path) => println!("rtk installed at {}", path.display()),
                Err(e) => {
                    eprintln!("tokex: install-rtk failed: {e}");
                    exit(1);
                }
            }
            return;
        }
        Some(Cmd::Graph) => {
            if let Err(e) = graphify::update_blocking() {
                eprintln!("tokex: graph update failed: {e}");
                exit(1);
            }
            return;
        }
        // No subcommand: read an intent as JSON from stdin (pipe mode).
        None => {
            // No subcommand and interactive: show full help rather than a cryptic usage line.
            if io::stdin().is_terminal() {
                Cli::command().print_help().ok();
                println!();
                exit(0);
            }
            let mut buf = String::new();
            if io::stdin().read_to_string(&mut buf).is_err() || buf.trim().is_empty() {
                eprintln!("no intent on stdin");
                exit(2);
            }
            match Intent::from_json(buf.trim()) {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("{e}");
                    exit(2);
                }
            }
        }
    };

    run_intent(intent);
}

/// Shared run tail: apply config modes, orchestrate through rtk, exit with its code. Used by the
/// `run` subcommand, stdin-JSON mode, and the bare `tokex <command>` passthrough.
fn run_intent(intent: Intent) {
    let mut out = io::stdout();
    let mut err = io::stderr();

    // Apply configured modes. `--llm` / JSON `"llm": true` force the insight on always; the `llm`
    // compression mode only analyzes failures (a successful command stays token-free).
    let cfg = config::load();
    let opts = orchestrate::Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: cfg.compression == "llm",
        footer: true,
    };

    // Load LLM config when it could be used. Fail fast only when `--llm` explicitly demanded it.
    let llm_cfg = if intent.llm || opts.llm_on_failure {
        match llm::LlmConfig::from_config(&cfg) {
            Some(c) => Some(c),
            None if intent.llm => {
                eprintln!("tokex: LLM compression needs an API key — run `tokex setup`");
                exit(2);
            }
            None => None,
        }
    } else {
        None
    };

    match orchestrate::run(&intent, &mut out, &mut err, llm_cfg.as_ref(), &opts) {
        Ok(code) => {
            if cfg.graph_auto {
                graphify::auto_update(&intent.command);
            }
            exit(code);
        }
        Err(e) => {
            eprintln!("tokex: {e}");
            exit(1);
        }
    }
}

/// A first arg is not a subcommand (so it's a command or a prompt) when it isn't a flag and isn't
/// one of our known subcommands. ponytail: collisions (a binary named `run`) lose to the subcommand.
fn is_passthrough(first: &str) -> bool {
    !first.starts_with('-') && !SUBCOMMANDS.contains(&first)
}

/// Handle a single bare argument: a free-text task (model writes a command, tokex runs it), a
/// `category: text` / JSON structured prompt, or a lone command.
fn dispatch_one(arg: &str, mode: prompt::Mode) {
    match prompt::classify(arg) {
        prompt::Dispatch::Command(cmd) => run_intent(Intent::from_command(cmd)),
        prompt::Dispatch::Prompt(task) => run_task(&task, mode),
        prompt::Dispatch::Json(s) => match prompt::parse_json(&s) {
            Ok(pairs) => run_prompt(pairs, mode),
            Err(e) => {
                eprintln!("tokex: {e}");
                exit(2);
            }
        },
        prompt::Dispatch::Category(cat, text) => run_prompt(vec![(cat, text)], mode),
    }
}

fn load_llm_or_exit(cfg: &config::Config) -> llm::LlmConfig {
    match llm::LlmConfig::from_config(cfg) {
        Some(c) => c,
        None => {
            eprintln!("tokex: prompts need an API key — run `tokex setup`");
            exit(2);
        }
    }
}

fn exec_opts(cfg: &config::Config) -> orchestrate::Options {
    orchestrate::Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: false,
        footer: true,
    }
}

/// Free-text task: the model produces a command and tokex runs it, returning the command's output.
fn run_task(task: &str, mode: prompt::Mode) -> ! {
    let cfg = config::load();
    let llm_cfg = load_llm_or_exit(&cfg);
    match prompt::run_task(&llm_cfg, task, mode, &exec_opts(&cfg)) {
        Ok(code) => exit(code),
        Err(e) => {
            eprintln!("tokex: {e}");
            exit(1);
        }
    }
}

/// Structured category prompts: print the combined JSON answer to stdout (thinking streams to
/// stderr in User mode inside `prompt::run`).
fn run_prompt(pairs: Vec<(String, String)>, mode: prompt::Mode) -> ! {
    let cfg = config::load();
    let llm_cfg = load_llm_or_exit(&cfg);
    match prompt::run(&pairs, &llm_cfg, mode) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            exit(0);
        }
        Err(e) => {
            eprintln!("tokex: {e}");
            exit(1);
        }
    }
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
