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
mod plan;

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
    /// Recommend a tech stack for a task.
    PlanStack {
        /// Free-text task description.
        task: String,
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

fn main() {
    let cli = Cli::parse();
    let mut out = io::stdout();
    let mut err = io::stderr();

    let mut intent = match cli.cmd {
        Some(Cmd::Run { llm, command }) => {
            let mut i = Intent::from_command(command);
            i.llm = llm;
            i
        }
        Some(Cmd::PlanStack { task }) => {
            let p = plan::plan(&task);
            println!("{}", serde_json::to_string_pretty(&p).unwrap());
            return;
        }
        Some(Cmd::Setup) => {
            if let Err(e) = config::run_setup() {
                eprintln!("tokex: setup failed: {e}");
                exit(1);
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

    // Apply configured modes. `--llm` and a JSON `"llm": true` both force the insight on; otherwise
    // the configured compression mode decides.
    let cfg = config::load();
    intent.llm = intent.llm || cfg.compression == "llm";
    let opts = orchestrate::Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
    };

    // Load LLM config only when needed; fail fast on missing setup rather than after running.
    let llm_cfg = if intent.llm {
        match llm::LlmConfig::from_config(&cfg) {
            Some(c) => Some(c),
            None => {
                eprintln!("tokex: LLM compression needs an API key — run `tokex setup`");
                exit(2);
            }
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
