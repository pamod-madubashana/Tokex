//! Tokex.
//! A deterministic RTK orchestration layer: normalize agent intent, forward to RTK, normalize
//! the stream. Tokex does not own execution; RTK does.

mod config;
mod graphify;
mod install;
mod install_agent;
mod intent;
mod llm;
mod mcp;
mod normalize;
mod orchestrate;
mod permission;
mod prompt;
mod script;
mod tool;

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
    /// Install Tokex skills into the current project for a specific agent.
    Install {
        /// Agent name (opencode, claude, codex, cursor, gemini, windsurf, aider, continue, cline).
        agent: String,
    },
}

/// Top-level subcommands. Anything else as the first arg is treated as a command to run, so
/// `tokex git status` works like `tokex run "git status"` (mirrors how rtk itself is invoked).
const SUBCOMMANDS: &[&str] = &[
    "run",
    "script",
    "setup",
    "mcp",
    "install-rtk",
    "graph",
    "install",
    "help",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `-m` / `--model` selects Model mode (output only, for agents); default is User mode
    // (spinner + live-streamed thinking, for humans). The rest of argv follows.
    let model_mode = matches!(
        args.get(1).map(String::as_str),
        Some("-m") | Some("--model")
    );
    let rest: &[String] = if model_mode { &args[2..] } else { &args[1..] };
    let mode = if model_mode {
        prompt::Mode::Model
    } else {
        prompt::Mode::User
    };

    if let Some(first) = rest.first() {
        // Role: `tokex <role> "<task>"` — offload a task to a role's model, return its answer.
        if prompt::role(first).is_some() {
            run_role(first, rest[1..].join(" ").trim(), mode);
        }
        // Otherwise, when the first arg isn't a subcommand/flag:
        //   several args -> a command (`tokex git status`), run through rtk
        //   one arg      -> a prompt (`tokex "list all rust projects"`, see prompt::classify)
        if is_passthrough(first) {
            if rest.len() >= 2 {
                run_intent(Intent::from_command(rest.join(" ")));
            } else {
                dispatch_one(&rest[0], mode);
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
        Some(Cmd::Install { agent }) => {
            if let Err(e) = install_agent::install_agent(&agent) {
                eprintln!("tokex: install failed: {e}");
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

/// Handle a single bare argument: a free-text task for the assistant agent (which runs commands or
/// answers), or a `category: text` / JSON structured prompt.
fn dispatch_one(arg: &str, mode: prompt::Mode) {
    match prompt::classify(arg) {
        // No role given → default to the `assistant` role. The agent decides how to answer — run a
        // command (`git ls-files`/`tree`) when the task needs it, or just answer (a bare `hi`).
        prompt::Dispatch::Prompt(task) => run_role("assistant", &task, mode),
        prompt::Dispatch::Json(s) => match prompt::parse_json(&s) {
            Ok(pairs) => run_prompt(pairs, mode),
            Err(e) => {
                eprintln!("tokex: {e}");
                exit(2);
            }
        },
        prompt::Dispatch::Category(cat, text) => run_prompt(vec![(cat, text)], mode),
        // Structure request: short-circuit the model, render tree directly.
        prompt::Dispatch::Structure => {
            let tree = prompt::project_tree();
            match mode {
                prompt::Mode::User => {
                    let opts = markdown_to_ansi::Options {
                        syntax_highlight: true,
                        width: std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()),
                        code_bg: true,
                    };
                    println!(
                        "{}",
                        markdown_to_ansi::render(&format!("```\n{tree}```"), &opts)
                    );
                }
                prompt::Mode::Model => println!("{tree}"),
            }
            exit(0);
        }
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

/// Role: offload a task to a role-specific model. The model decides whether to run a command (real
/// output) or answer; the role just picks which model and biases it with the role's persona.
fn run_role(role: &str, task: &str, mode: prompt::Mode) -> ! {
    if task.is_empty() {
        eprintln!("tokex: role '{role}' needs a task, e.g. tokex {role} \"...\"");
        exit(2);
    }
    let (model, header, _role_mode, max_steps) = prompt::role(role).unwrap_or_else(|| {
        eprintln!("tokex: unknown role '{role}'");
        exit(2);
    });
    fulfill(task, model, Some(header), mode, max_steps);
}

/// Shared task fulfilment: pick the endpoint/key from config, swap in `model`, and let `prompt`
/// decide run-vs-answer.
fn fulfill(
    task: &str,
    model: &str,
    role_header: Option<&str>,
    mode: prompt::Mode,
    max_steps: usize,
) -> ! {
    let cfg = config::load();
    let base = load_llm_or_exit(&cfg);
    let model_cfg = prompt::with_model(&base, model);
    match prompt::fulfill(
        task,
        &model_cfg,
        role_header,
        mode,
        &exec_opts(&cfg),
        max_steps,
    ) {
        Ok(code) => exit(code),
        Err(e) => {
            eprintln!("tokex: {e}");
            exit(1);
        }
    }
}

/// Category / JSON prompts run through the same agentic decide-run-or-answer loop as roles — each
/// pair's category becomes the persona header, on the configured model. No special chat-only path.
fn run_prompt(pairs: Vec<(String, String)>, mode: prompt::Mode) -> ! {
    let cfg = config::load();
    let base = load_llm_or_exit(&cfg);
    let opts = exec_opts(&cfg);
    for (cat, text) in &pairs {
        let header = match prompt::category_header(cat) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("tokex: {e}");
                exit(2);
            }
        };
        if let Err(e) = prompt::fulfill(text, &base, Some(header), mode, &opts, prompt::MAX_STEPS) {
            eprintln!("tokex: {e}");
            exit(1);
        }
    }
    exit(0);
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
