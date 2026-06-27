//! Task dispatch: route intents, commands, and prompts to the execution core.

use std::io::{self, IsTerminal, Read};
use std::process::exit;

use super::cli::{Cli, Cmd, SUBCOMMANDS};
use crate::agent;
use crate::config;
use crate::core::intent::Intent;
use crate::core::orchestrate;
use crate::graphify;
use crate::llm;
use crate::script;

use clap::{CommandFactory, Parser};

/// Top-level entry point: parse args, detect mode, route to the right handler.
pub fn run() {
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
            run_role(first, rest[1..].join(" ").trim(), mode);
        }
        if is_passthrough(first) {
            if rest.len() >= 2 {
                run_intent(Intent::from_command(rest.join(" ")));
            } else {
                dispatch_one(&rest[0], mode);
            }
            return;
        }
        if model_mode {
            eprintln!("cotrex: -m takes a prompt or role, not '{first}'");
            exit(2);
        }
    } else if model_mode {
        eprintln!("cotrex: -m needs a prompt or role, e.g. cotrex -m \"list rust projects\"");
        exit(2);
    }

    let cli = Cli::parse();

    let intent = match cli.cmd {
        Some(cmd) => match dispatch_cmd(cmd) {
            Some(intent) => intent,
            None => return,
        },
        None => read_stdin_intent(),
    };

    run_intent(intent);
}

fn is_passthrough(first: &str) -> bool {
    !first.starts_with('-') && !SUBCOMMANDS.contains(&first)
}

/// Dispatch a parsed CLI subcommand. Returns the intent for commands that fall through to
/// `run_intent`, or exits directly for self-contained subcommands.
pub fn dispatch_cmd(cmd: Cmd) -> Option<Intent> {
    match cmd {
        Cmd::Run { llm, command } => {
            let mut i = Intent::from_command(command);
            i.llm = llm;
            Some(i)
        }
        Cmd::Script { file } => {
            let cfg = config::load();
            let opts = orchestrate::Options {
                raw: cfg.compression == "off",
                ultra_compact: cfg.rtk_verbosity == "ultra-compact",
                llm_on_failure: false,
                footer: true,
            };
            let mut out = io::stdout();
            let mut err = io::stderr();
            match file {
                Some(f) => match script::run(&f, &mut out, &mut err, &opts) {
                    Ok(code) => exit(code),
                    Err(e) => {
                        eprintln!("cotrex: {e}");
                        exit(1);
                    }
                },
                None => {
                    if let Err(e) = script::ensure_dir() {
                        eprintln!("cotrex: cannot create Scripts/: {e}");
                        exit(1);
                    }
                    eprintln!("{}", script::INSTRUCTIONS);
                    exit(0);
                }
            }
        }
        Cmd::Setup => {
            if let Err(e) = config::run_setup() {
                eprintln!("cotrex: setup failed: {e}");
                exit(1);
            }
            if config::load().graph_auto {
                match graphify::setup_steps() {
                    Ok(steps) => {
                        for (label, step) in steps {
                            let spinner = agent::prompt::Spinner::start(label);
                            if let Err(e) = step() {
                                spinner.complete();
                                eprintln!("cotrex: {e}");
                            } else {
                                spinner.complete();
                            }
                        }
                    }
                    Err(e) => eprintln!("cotrex: {e}"),
                }
            }
            exit(0);
        }
        Cmd::Mcp => llm::mcp::serve(),
        Cmd::InstallRtk => {
            match config::install::install() {
                Ok(path) => println!("rtk installed at {}", path.display()),
                Err(e) => {
                    eprintln!("cotrex: install-rtk failed: {e}");
                    exit(1);
                }
            }
            exit(0);
        }
        Cmd::Graph => {
            if let Err(e) = graphify::update_blocking() {
                eprintln!("cotrex: graph update failed: {e}");
                exit(1);
            }
            exit(0);
        }
        Cmd::Install { agent } => {
            match agent {
                Some(a) => {
                    if let Err(e) = config::install_agent::install_agent(&a) {
                        eprintln!("cotrex: install failed: {e}");
                        exit(1);
                    }
                }
                None => {
                    if let Err(e) = config::install_agent::list_installed() {
                        eprintln!("cotrex: {e}");
                        exit(1);
                    }
                }
            }
            exit(0);
        }
        Cmd::Update => {
            if let Err(e) = config::update::run() {
                eprintln!("cotrex: update failed: {e}");
                exit(1);
            }
            exit(0);
        }
    }
}

/// Read a JSON intent from stdin (pipe mode).
pub fn read_stdin_intent() -> Intent {
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

/// Shared run tail: apply config modes, orchestrate through rtk, exit with its code. Used by the
/// `run` subcommand, stdin-JSON mode, and the bare `cotrex <command>` passthrough.
pub fn run_intent(intent: Intent) {
    let cfg = config::load();
    let model_mode = matches!(
        std::env::args().nth(1).as_deref(),
        Some("-m") | Some("--model")
    );

    let opts = orchestrate::Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: cfg.compression == "llm" && !model_mode,
        footer: !model_mode,
    };

    let llm_cfg = if intent.llm || opts.llm_on_failure {
        match llm::LlmConfig::from_config(&cfg) {
            Some(c) => Some(c),
            None if intent.llm => {
                eprintln!("cotrex: LLM compression needs an API key — run `cotrex setup`");
                exit(2);
            }
            None => None,
        }
    } else {
        None
    };

    if model_mode {
        let mut buf = Vec::new();
        let mut err_sink = io::sink();
        let code = match orchestrate::run(&intent, &mut buf, &mut err_sink, llm_cfg.as_ref(), &opts)
        {
            Ok(c) => c,
            Err(e) => {
                let result = serde_json::json!({
                    "type": "result",
                    "status": "failed",
                    "code": -1,
                    "error": e,
                });
                println!("{result}");
                exit(1);
            }
        };
        let output = String::from_utf8_lossy(&buf);

        let mut important: Vec<String> = Vec::new();
        let mut current_block: Vec<String> = Vec::new();
        for line in output.lines() {
            let trimmed = line.trim();
            let starts_new = trimmed.starts_with("warning:")
                || trimmed.starts_with("error[")
                || trimmed.starts_with("error:")
                || trimmed.starts_with("error ");

            if starts_new && !current_block.is_empty() {
                important.push(current_block.join("\n"));
                current_block.clear();
            }
            if starts_new
                || (!current_block.is_empty()
                    && (trimmed.starts_with("--> ")
                        || trimmed.starts_with("  --> ")
                        || trimmed.starts_with("   |")
                        || trimmed.starts_with("   =")))
            {
                current_block.push(trimmed.to_string());
            }
        }
        if !current_block.is_empty() {
            important.push(current_block.join("\n"));
        }

        let status = if code == 0 { "ok" } else { "failed" };

        let result = serde_json::json!({
            "type": "result",
            "status": status,
            "code": code,
        });
        println!("{result}");

        if !important.is_empty() || code != 0 {
            let insight = serde_json::json!({
                "type": "insight",
                "status": status,
                "root_cause": intent.command,
                "important_errors": important,
            });
            println!("{insight}");
        }

        if cfg.graph_auto {
            graphify::auto_update(&intent.command);
        }
        exit(code);
    } else {
        let mut out = io::stdout();
        let mut err = io::stderr();
        match orchestrate::run(&intent, &mut out, &mut err, llm_cfg.as_ref(), &opts) {
            Ok(code) => {
                if cfg.graph_auto {
                    graphify::auto_update(&intent.command);
                }
                exit(code);
            }
            Err(e) => {
                eprintln!("cotrex: {e}");
                exit(1);
            }
        }
    }
}

/// Handle a single bare argument: a free-text task for the assistant agent (which runs commands or
/// answers), or a `category: text` / JSON structured prompt.
pub fn dispatch_one(arg: &str, mode: agent::prompt::Mode) {
    match agent::prompt::classify(arg) {
        agent::prompt::Dispatch::Prompt(task) => run_role("assistant", &task, mode),
        agent::prompt::Dispatch::Json(s) => match agent::prompt::parse_json(&s) {
            Ok(pairs) => run_prompt(pairs, mode),
            Err(e) => {
                eprintln!("cotrex: {e}");
                exit(2);
            }
        },
        agent::prompt::Dispatch::Category(cat, text) => run_prompt(vec![(cat, text)], mode),
        agent::prompt::Dispatch::Structure => {
            let tree = agent::prompt::project_tree();
            match mode {
                agent::prompt::Mode::User => {
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
                agent::prompt::Mode::Model => println!("{tree}"),
            }
            exit(0);
        }
    }
}

pub fn load_llm_or_exit(cfg: &config::Config) -> llm::LlmConfig {
    match llm::LlmConfig::from_config(cfg) {
        Some(c) => c,
        None => {
            eprintln!("cotrex: prompts need an API key — run `cotrex setup`");
            exit(2);
        }
    }
}

pub fn exec_opts(cfg: &config::Config) -> orchestrate::Options {
    orchestrate::Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: false,
        footer: true,
    }
}

/// Role: offload a task to a role-specific model. The model decides whether to run a command (real
/// output) or answer; the role just picks which model and biases it with the role's persona.
pub fn run_role(role: &str, task: &str, mode: agent::prompt::Mode) -> ! {
    if task.is_empty() {
        eprintln!("cotrex: role '{role}' needs a task, e.g. cotrex {role} \"...\"");
        exit(2);
    }
    let (model, header, _role_mode, max_steps) = agent::prompt::role(role).unwrap_or_else(|| {
        eprintln!("cotrex: unknown role '{role}'");
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
    mode: agent::prompt::Mode,
    max_steps: usize,
) -> ! {
    let cfg = config::load();
    let base = load_llm_or_exit(&cfg);
    let model_cfg = agent::prompt::with_model(&base, model);
    match agent::prompt::fulfill(
        task,
        &model_cfg,
        role_header,
        mode,
        &exec_opts(&cfg),
        max_steps,
    ) {
        Ok(code) => exit(code),
        Err(e) => {
            eprintln!("cotrex: {e}");
            exit(1);
        }
    }
}

/// Category / JSON prompts run through the same agentic decide-run-or-answer loop as roles — each
/// pair's category becomes the persona header, on the configured model. No special chat-only path.
fn run_prompt(pairs: Vec<(String, String)>, mode: agent::prompt::Mode) -> ! {
    let cfg = config::load();
    let base = load_llm_or_exit(&cfg);
    let opts = exec_opts(&cfg);
    for (cat, text) in &pairs {
        let header = match agent::prompt::category_header(cat) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("cotrex: {e}");
                exit(2);
            }
        };
        if let Err(e) = agent::prompt::fulfill(
            text,
            &base,
            Some(header),
            mode,
            &opts,
            agent::prompt::MAX_STEPS,
        ) {
            eprintln!("cotrex: {e}");
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
