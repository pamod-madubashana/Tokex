//! Prompts.
//!
//! A single quoted argument is a *prompt*, not a command. Free text is a **task**: the model turns
//! it into one shell command and tokex *runs* it, returning the command's output (not the command).
//! `category: text` (or a JSON object of several) instead returns a structured answer using that
//! category's header — those aren't runnable commands.
//!
//! Two presentation modes:
//! - **User** (`tokex "…"`): a spinner while the model thinks; between commands it narrates each
//!   step ("Let me check…") in its own words, streams the running command's output, then the answer.
//! - **Model** (`tokex -m "…"`): no spinner, no narration — just the output on stdout.
//!
//! Every call is streamed; the LLM key comes from config.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::intent::Intent;
use crate::llm::LlmConfig;
use crate::orchestrate::{self, Options};

/// Who's reading the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// A human: spinner + live-streamed generation.
    User,
    /// Another model/agent: output only, no thinking or chatter.
    Model,
}

// Each row binds a category to its header (system prompt). Add a row to add a category.
const CATEGORIES: &[(&str, &str)] = &[
    (
        "plan-stack",
        "You recommend the single best application tech stack for a developer's task. Inspect the \
working directory first if it helps you decide. Answer with the stack name and one concise sentence \
of reasoning — nothing more.",
    ),
    (
        "theme",
        "You are a senior UI designer. Given a short style description, answer with a color palette \
(hex colors), a font, a few key effects, and one concise sentence of rationale.",
    ),
];

// Used when a category prompt has no recognized category (rare: a JSON object with an empty key).
const DEFAULT_HEADER: &str = "You are a concise senior software engineer. Answer the developer's \
question briefly and practically. No preamble, no markdown headings.";

// A task either runs a command (and we return the REAL output) or is answered in text. The model
// decides and replies with JSON: {"run":"<command>"} or {"answer":"<text>"}.
const DECISION_SYSTEM: &str = "You are an assistant in a developer's CURRENT working directory. \
FIRST decide whether answering even needs the machine. Each turn, reply with EXACTLY ONE JSON object:\n\
- {\"answer\":\"<text>\"} — answer the user directly. Use this RIGHT AWAY when no local information is \
needed: a greeting, small talk, a general or coding question, or anything you already know. Do NOT \
run a command just to have run one. SYNTHESIZE; never paste raw command output. Wrap any file \
tree/table/aligned layout in a fenced ``` code block.\n\
- {\"run\":\"<command>\",\"say\":\"<one line>\"} — to inspect the project (files, dirs, git, build \
state) OR to CARRY OUT an action the user asked for (build, test, run, format, lint, fix…). You have \
a REAL shell in THIS directory — you are not sandboxed; never say you lack access to the code or \
build system, just run the command. `say` is one short first-person line telling the user what \
you're doing or what you just learned, like a person thinking aloud: \"Let me build it in release.\", \
\"Not there — let me check main.rs.\", \"Found them.\" One command; when inspecting, go ONE level at \
a time, skip vendored/build dirs (vendor, target, node_modules, .git, dist), never dump the whole \
recursive tree.\n\
When getting to know a project, FIRST find what's ignored — read its .gitignore (or just use `git \
ls-files`, which already honors it) — and never list or recurse into ignored paths (node_modules, \
target, dist, build artifacts). A raw recursive listing that walks ignored dirs is wrong.\n\
Answer directly for greetings and general/coding questions; RUN commands to inspect the project or \
to do what the user asked — don't refuse or ask permission for a normal dev command. Use the fewest \
that do the job, and cite the concrete location (path:line) when you find what was asked. Always \
finish with an {\"answer\"} summarizing what you did or found. Output ONLY the JSON.\n\
Examples:\n\
Request: hi → {\"answer\":\"Hi! What would you like to do in this project?\"}\n\
Request: what does the ? operator do in Rust → {\"answer\":\"It propagates errors: on Err it returns \
early, on Ok it unwraps.\"}\n\
Request: build this project in release → {\"run\":\"cargo build --release\",\"say\":\"Building it in \
release mode.\"}\n\
Request: what is this project → {\"run\":\"git ls-files\",\"say\":\"Let me list the tracked files to \
see what this is.\"}\n\
Request: where are user roles implemented → {\"run\":\"Select-String -Path src\\\\*.rs -Pattern \
role\",\"say\":\"Let me search the source for role handling.\"}";

/// The header (system prompt) bound to a category, if it is known.
pub fn header(category: &str) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .find(|(n, _)| *n == category)
        .map(|(_, h)| *h)
}

/// Resolve a category to its agentic persona header. An empty category (a JSON object with an empty
/// key) falls back to the default; an unknown one is an error.
pub fn category_header(category: &str) -> Result<&'static str, String> {
    if category.is_empty() {
        Ok(DEFAULT_HEADER)
    } else {
        header(category).ok_or_else(|| format!("unknown category '{category}'"))
    }
}

// Roles: `tokex <role> "<task>"` offloads a small task to a role-specific model and returns its
// answer, so the calling agent just waits (and spends no tokens thinking). Each row is
// (role, model id, header, mode, max_steps). The model ids are NVIDIA NIM ids served by the
// configured endpoint; add or retune a role by editing a row.
//
// Modes (inspired by OpenCode's agent system):
// - "primary": the main agent that can run commands and answer (default for assistant)
// - "subagent": a specialized agent called by another agent (planner, coder, etc.)
//
// Max steps: how many command iterations the agent can run before forced to answer.
const ROLES: &[(&str, &str, &str, &str, usize)] = &[
    (
        "planner",
        "z-ai/glm-5.1",
        "You are a planning specialist. Given a goal, produce a concise, ordered, actionable plan as \
        numbered steps. No preamble, no code unless essential.",
        "subagent",
        3,
    ),
    (
        "router",
        "nvidia/nemotron-3-nano-30b-a3b",
        "You are a router. Given a request, decide the single best next action, tool, or role and \
        answer in one or two decisive lines. No deliberation in the output.",
        "subagent",
        1,
    ),
    (
        "orchestrator",
        "nvidia/nemotron-3-ultra-550b-a55b",
        "You are an orchestrator. Break the goal into an ordered list of concrete shell commands or \
        steps that accomplish it end to end. Be specific and minimal. No prose beyond the steps.",
        "subagent",
        4,
    ),
    (
        "coder",
        "deepseek-ai/deepseek-v4-flash",
        "You are a senior engineer. Output only the code that solves the task — correct, minimal, \
        idiomatic. No explanation unless the task asks for it.",
        "subagent",
        5,
    ),
    (
        "assistant",
        "qwen/qwen3-next-80b-a3b-instruct",
        "You are a concise developer assistant. Answer the request briefly and practically. No fluff.",
        "primary",
        6,
    ),
];

/// `(model id, header, mode, max_steps)` for a role, if known.
pub fn role(name: &str) -> Option<(&'static str, &'static str, &'static str, usize)> {
    ROLES
        .iter()
        .find(|(n, _, _, _, _)| *n == name)
        .map(|(_, m, h, mode, steps)| (*m, *h, *mode, *steps))
}

/// Build an `LlmConfig` that reuses the configured endpoint + key but swaps in `model`.
pub fn with_model(base: &LlmConfig, model: &str) -> LlmConfig {
    LlmConfig {
        url: base.url.clone(),
        key: base.key.clone(),
        model: model.to_string(),
    }
}

/// How a single bare argument should be handled.
#[derive(Debug, PartialEq)]
pub enum Dispatch {
    /// JSON object of `category -> text` (possibly several). Pass the raw string to `parse_json`.
    Json(String),
    /// `category: text` with a known category — returns a structured answer.
    Category(String, String),
    /// Free-text task for the assistant agent — it decides whether to run a command or just answer.
    Prompt(String),
    /// Project structure request — short-circuits the model and renders a tree directly.
    Structure,
}

/// Check if a prompt is a project-structure request (e.g. "show project structure", "list tree").
/// Short-circuits the model entirely — renders a tree from `git ls-files`.
fn is_structure_request(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let has_structure_word =
        lower.contains("structure") || lower.contains("tree") || lower.contains("layout");
    let has_directory_noun = lower.contains("project")
        || lower.contains("directory")
        || lower.contains("repo")
        || lower.contains("codebase")
        || lower.contains("files");
    has_structure_word && has_directory_noun
}

/// Classify one argument. A single quoted arg reaches here; multi-arg invocations are commands and
/// never get classified. Anything that isn't a JSON object or a `known-category: text` is a prompt —
/// even a lone word like `hi`, so User mode behaves like a normal AI agent. Run a raw command with
/// args (`tokex git status`) or `tokex run <cmd>`.
pub fn classify(arg: &str) -> Dispatch {
    let s = arg.trim();
    if s.starts_with('{') {
        return Dispatch::Json(s.to_string());
    }
    if let Some((cat, rest)) = s.split_once(':') {
        if header(cat.trim()).is_some() {
            return Dispatch::Category(cat.trim().to_string(), rest.trim().to_string());
        }
    }
    if is_structure_request(s) {
        return Dispatch::Structure;
    }
    Dispatch::Prompt(s.to_string())
}

/// Directories to skip (noise/build artifacts).
const SKIP_DIRS: &[&str] = &[
    "vendor",
    "target",
    "node_modules",
    ".git",
    "dist",
    "build",
    ".gitmodules",
];

/// Render a depth-limited project tree from `git ls-files`. Honors .gitignore, leaves submodules
/// unexpanded. Falls back to a shallow directory walk outside a git repo.
pub fn project_tree() -> String {
    // Try git ls-files first (honors .gitignore automatically).
    let git_output = std::process::Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output();
    if let Ok(output) = git_output {
        if output.status.success() {
            let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            if !files.is_empty() {
                return build_tree_from_files(&files);
            }
        }
    }
    // Fallback: shallow walk of current directory.
    build_tree_fallback()
}

/// Build a tree string from a list of file paths (from git ls-files).
fn build_tree_from_files(files: &[String]) -> String {
    // Simple tree builder: just render the file list as a tree structure.
    let mut result = String::from(".\n");
    let mut prev_dir = String::new();

    for file in files {
        let parts: Vec<&str> = file.split('/').collect();
        let depth = parts.len().saturating_sub(1);
        let name = parts.last().unwrap_or(&"");

        if depth == 0 {
            // Root-level file
            let connector = "├── ";
            result.push_str(&format!("{connector}{name}\n"));
        } else {
            // Nested file: show directory prefix if it changed
            let dir = parts[..depth].join("/");
            if dir != prev_dir {
                for d in 0..depth {
                    let prefix: String = "│   ".repeat(d);
                    let dir_name = parts[d];
                    if d == depth - 1 {
                        result.push_str(&format!("{prefix}├── {dir_name}/\n"));
                    }
                }
                prev_dir = dir;
            }
            let prefix: String = "│   ".repeat(depth.saturating_sub(1));
            let connector = "├── ";
            result.push_str(&format!("{prefix}{connector}{name}\n"));
        }
    }
    result
}

/// Fallback tree builder when git is not available.
fn build_tree_fallback() -> String {
    let mut result = String::from(".\n");
    if let Ok(entries) = std::fs::read_dir(".") {
        let mut dirs: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(name);
            } else {
                files.push(name);
            }
        }
        dirs.sort();
        files.sort();
        for (i, dir) in dirs.iter().enumerate() {
            let is_last = i == dirs.len() - 1 && files.is_empty();
            let connector = if is_last { "└── " } else { "├── " };
            result.push_str(&format!("{connector}{dir}/\n"));
        }
        for (i, file) in files.iter().enumerate() {
            let is_last = i == files.len() - 1;
            let connector = if is_last { "└── " } else { "├── " };
            result.push_str(&format!("{connector}{file}\n"));
        }
    }
    result
}

/// Parse the JSON multi-category form into `(category, text)` pairs. Accepts a flat object
/// `{"plan-stack":"…","theme":"…"}` or a `{"task": { … }}` wrapper.
pub fn parse_json(s: &str) -> Result<Vec<(String, String)>, String> {
    let v: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("invalid JSON prompt: {e}"))?;
    let obj = match v.get("task").and_then(|t| t.as_object()) {
        Some(o) => o.clone(),
        None => v
            .as_object()
            .ok_or("JSON prompt must be an object")?
            .clone(),
    };
    let pairs: Vec<(String, String)> = obj
        .iter()
        .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();
    if pairs.is_empty() {
        return Err("JSON prompt has no category:text pairs".into());
    }
    Ok(pairs)
}

/// Fulfill a task: ask the model to decide between running a command or answering, then do it.
/// `role_header` biases the decision (and is the role's persona); `cfg` carries the chosen model.
/// `max_steps` limits how many command iterations the agent can run before forced to answer.
/// Returns the exit code (0 for an answered task). Prints the real command output, or the answer,
/// to stdout.
pub fn fulfill(
    task: &str,
    cfg: &LlmConfig,
    role_header: Option<&str>,
    mode: Mode,
    opts: &Options,
    max_steps: usize,
) -> Result<i32, String> {
    // Generate for the shell we actually run on (see `exec_capture`): PowerShell on Windows, POSIX
    // bash elsewhere. Mismatching them is what makes a Windows run try to execute Linux commands.
    let shell = if cfg!(windows) {
        "Any command runs in Windows PowerShell — use PowerShell cmdlets and syntax (Get-ChildItem, \
Select-String, Measure-Object, Select-Object, Where-Object). Do NOT use bash/POSIX tools (no sed, \
awk, grep, or `find` with -printf)."
    } else {
        "Any command runs in a POSIX bash shell — use POSIX tools (find, grep, sed, awk, wc, ls, \
git); never PowerShell or cmd syntax."
    };
    let system = match role_header {
        Some(h) => format!("{h}\n\n{DECISION_SYSTEM} {shell}"),
        None => format!("{DECISION_SYSTEM} {shell}"),
    };

    // Step loop: the model runs commands to gather info (each output fed back, capped), then
    // finishes with an ANALYZED answer — never a raw command dump. A failure is fed back to fix.
    let mut transcript = String::new();
    let mut seen: Vec<String> = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new(); // (cmd, error) pairs
    for step in 0..max_steps {
        let user = if transcript.is_empty() {
            task.to_string()
        } else {
            let fix_hint = if let Some((_last_cmd, last_err)) = failed.last() {
                format!("\nThe last command failed: {last_err}\nTry a different approach to avoid the same error.")
            } else {
                String::new()
            };
            format!("Request: {task}\n\nCommands run so far:\n{transcript}{fix_hint}\nGather more if needed, else answer.")
        };
        match parse_decision(&one_call(cfg, &system, &user, mode, false)?) {
            Decision::Answer(text) => {
                print_answer(&text, mode);
                return Ok(0);
            }
            // A weak model loops, re-running a command it already ran. That yields no new info, so
            // break to the forced answer instead of spinning.
            Decision::Run { cmd, .. } if seen.contains(&cmd) => {
                // If the command failed before, give the model one more chance with a hint.
                if failed.iter().any(|(c, _)| *c == cmd) && step < max_steps - 1 {
                    transcript.push_str(&format!(
                        "$ {cmd}\n(already failed — try a different command)\n\n"
                    ));
                    continue;
                }
                break;
            }
            Decision::Run { say, cmd } => {
                say_step(say.as_deref(), mode); // the model's own "let me check…" narration
                seen.push(cmd.clone());
                if is_risky(&cmd) {
                    if !confirm(&cmd) {
                        let _ = writeln!(std::io::stderr(), "aborted (not confirmed).");
                        return Ok(130); // 128 + SIGINT
                    }
                } else {
                    let _ = writeln!(std::io::stderr(), "$ {cmd}"); // safe → show what runs
                }
                let (code, out) = exec_capture(&cmd, opts, mode)?;
                if code != 0 {
                    failed.push((cmd.clone(), format!("exit {code}: {}", trunc(&out, 200))));
                }
                transcript.push_str(&format!(
                    "$ {cmd}\n(exit {code})\n{}\n\n",
                    trunc(&out, 1500)
                ));
            }
        }
    }
    // Out of steps: force a final answer from what we've gathered.
    let user = format!(
        "Request: {task}\n\nCommands run so far:\n{transcript}\nGive your final answer now as {{\"answer\":\"...\"}}."
    );
    if let Decision::Answer(text) = parse_decision(&one_call(cfg, &system, &user, mode, false)?) {
        print_answer(&text, mode);
    }
    Ok(0)
}

/// Show the model's one-line narration of a step ("Let me check the routes.") to a human, in User
/// mode only — a light color so it reads as the agent talking, distinct from command output.
fn say_step(say: Option<&str>, mode: Mode) {
    if let (Mode::User, Some(s)) = (mode, say) {
        let _ = writeln!(std::io::stderr(), "{SAY_COLOR}{s}\x1b[0m");
    }
}

/// Light steel-blue for the agent's narration — clearly lighter than the dim it replaced.
const SAY_COLOR: &str = "\x1b[38;5;153m";

// Max commands the model may run to gather info before it must give a final answer.
pub const MAX_STEPS: usize = 6;

/// Maximum number of retry attempts for transient LLM failures.
const MAX_RETRIES: usize = 3;

/// Initial backoff duration in milliseconds.
const INITIAL_BACKOFF_MS: u64 = 500;

/// Print an answer to stdout. User mode renders markdown to ANSI (headers, lists, syntax-highlighted
/// code blocks) so it reads in a terminal instead of showing raw ``` fences; Model mode prints the
/// raw text, since an agent wants plain markdown, not escape codes.
fn print_answer(text: &str, mode: Mode) {
    match mode {
        Mode::User => {
            let opts = markdown_to_ansi::Options {
                syntax_highlight: true,
                // Wrap to the terminal width when known; otherwise let the terminal wrap.
                width: std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()),
                code_bg: true,
            };
            println!("{}", markdown_to_ansi::render(text, &opts));
        }
        Mode::Model => println!("{text}"),
    }
}

enum Decision {
    /// Run a command to gather info (the loop continues and feeds the output back). `say` is the
    /// model's own one-line narration of what it's about to do, shown to a human before the command.
    Run {
        say: Option<String>,
        cmd: String,
    },
    Answer(String),
}

/// Read the model's JSON decision: `{"run":…}` → gather via a command, `{"answer":…}` → final text.
/// An optional `say` narrates the step. Anything that isn't our JSON is treated as a plain answer
/// (graceful when a model strays).
fn parse_decision(content: &str) -> Decision {
    // The model is told to emit ONE JSON object, but a weak one sometimes emits several (or trailing
    // prose). Parse the FIRST complete object from the first `{` — a span of first-`{`..last-`}`
    // would glue multiple objects into invalid JSON and lose the decision entirely.
    if let Some(a) = content.find('{') {
        let mut objs =
            serde_json::Deserializer::from_str(&content[a..]).into_iter::<serde_json::Value>();
        if let Some(Ok(v)) = objs.next() {
            let say = v
                .get("say")
                .and_then(|x| x.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            if let Some(cmd) = v.get("run").and_then(|x| x.as_str()) {
                if !cmd.trim().is_empty() {
                    return Decision::Run {
                        say,
                        cmd: cmd.trim().to_string(),
                    };
                }
            }
            if let Some(ans) = v.get("answer").and_then(|x| x.as_str()) {
                return Decision::Answer(ans.trim().to_string());
            }
        }
    }
    Decision::Answer(content.trim().to_string())
}

/// Run `cmd` via rtk and capture its combined output. Generated commands target the native shell —
/// PowerShell on Windows, bash on Unix — and `rtk run -c` uses the OS shell (cmd.exe on Windows),
/// which mangles pipes/quoting. So write the command to a temp script and invoke the native
/// interpreter on it by path: no inline quoting, no cross-shell mount surprises.
fn exec_capture(cmd: &str, opts: &Options, mode: Mode) -> Result<(i32, String), String> {
    let pid = std::process::id();
    // Fail hard on error so a bad command gets a non-zero exit (PowerShell non-terminating errors
    // and bash mid-pipeline failures otherwise pass silently) — that's what drives the fix retry.
    let (tmp, run_line, content) = if cfg!(windows) {
        let p = std::env::temp_dir().join(format!("tokex-task-{pid}.ps1"));
        let line = format!(
            "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
            p.display()
        );
        (p, line, format!("$ErrorActionPreference = 'Stop'\n{cmd}\n"))
    } else {
        let p = std::env::temp_dir().join(format!("tokex-task-{pid}.sh"));
        let line = format!("bash {}", p.display());
        (p, line, format!("set -e\n{cmd}\n"))
    };
    std::fs::write(&tmp, content).map_err(|e| format!("temp script: {e}"))?;
    // ponytail: temp paths have no spaces, so the unquoted path is safe; quote only if that breaks.
    let exec = Options {
        raw: true,
        footer: false,
        llm_on_failure: false,
        ..*opts
    };
    // Capture every line, and in User mode show the last 5 live in a ```bash viewport (see TailView).
    let mut view = TailView::new(mode);
    let result = orchestrate::run(
        &Intent::from_command(run_line),
        &mut view,
        &mut std::io::sink(),
        None,
        &exec,
    );
    let buf = view.finish();
    let _ = std::fs::remove_file(&tmp);
    let code = result?;
    Ok((code, cap_lines(&String::from_utf8_lossy(&buf), OUTPUT_CAP)))
}

/// Live tail of a running command. Captures every byte (returned to the caller) while redrawing the
/// last `TAIL_ROWS` lines in place on stderr as a markdown ```bash block (rendered to ANSI, so it
/// shows as a styled code box, not raw backticks), so a human watches output stream by without it
/// scrolling the terminal. The final tail is left on screen when the command finishes, so the
/// output stays visible. Off in Model mode or when stderr isn't a TTY (no cursor control).
/// ponytail: track the rows we printed and clear-to-end before each repaint, so a variable-height
/// render stays aligned; throttle repaints so a fast command doesn't flicker.
const TAIL_ROWS: usize = 5;
const REDRAW_INTERVAL: Duration = Duration::from_millis(70);

struct TailView {
    full: Vec<u8>,
    pending: String,
    tail: VecDeque<String>,
    rows: usize, // rows the last repaint printed (to move back up over them)
    shown: bool,
    live: bool,
    last_draw: std::time::Instant,
}

impl TailView {
    fn new(mode: Mode) -> Self {
        let live = mode == Mode::User && std::io::stderr().is_terminal();
        TailView {
            full: Vec::new(),
            pending: String::new(),
            tail: VecDeque::new(),
            rows: 0,
            shown: false,
            live,
            last_draw: std::time::Instant::now(),
        }
    }

    fn push_line(&mut self, line: String) {
        if self.tail.len() == TAIL_ROWS {
            self.tail.pop_front();
        }
        self.tail.push_back(line);
        // Repaint on the first line for instant feedback, then at most every REDRAW_INTERVAL.
        if !self.shown || self.last_draw.elapsed() >= REDRAW_INTERVAL {
            self.redraw();
        }
    }

    /// Render the current tail as an ANSI ```bash block and repaint it in place.
    fn redraw(&mut self) {
        let mut err = std::io::stderr();
        if self.shown {
            let _ = write!(err, "\r\x1b[{}A\x1b[J", self.rows); // up over the last paint, clear down
        }
        let block = render_block(&self.tail);
        let _ = write!(err, "{block}");
        let _ = err.flush();
        self.rows = block.matches('\n').count();
        self.shown = true;
        self.last_draw = std::time::Instant::now();
    }

    /// Return the full captured output, leaving the final tail on screen (so the command's output
    /// stays visible after it finishes) and dropping the cursor below it for what prints next.
    fn finish(self) -> Vec<u8> {
        if self.live && self.shown {
            let _ = writeln!(std::io::stderr()); // move below the retained block, don't erase it
        }
        self.full
    }
}

/// Build the ANSI-rendered ```bash block for the tail (oldest→newest, padded to `TAIL_ROWS`), with no
/// trailing newline so the caller can count rows by counting `\n`.
// Columns left blank on the right of the output box, so it doesn't run to the terminal edge.
const RIGHT_MARGIN: usize = 4;

fn render_block(tail: &VecDeque<String>) -> String {
    let w = term_width().saturating_sub(1 + RIGHT_MARGIN);
    let mut md = String::from("```bash\n");
    for i in 0..TAIL_ROWS {
        md.push_str(&clip(tail.get(i).map(String::as_str).unwrap_or(""), w));
        md.push('\n');
    }
    md.push_str("```");
    let opts = markdown_to_ansi::Options {
        syntax_highlight: true,
        width: Some(w + 1),
        code_bg: true,
    };
    markdown_to_ansi::render(&md, &opts)
        .trim_end_matches('\n')
        .to_string()
}

impl Write for TailView {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.full.extend_from_slice(b);
        if self.live {
            for ch in String::from_utf8_lossy(b).chars() {
                match ch {
                    '\n' => {
                        let line = std::mem::take(&mut self.pending);
                        self.push_line(line);
                    }
                    '\r' => {}
                    _ => self.pending.push(ch),
                }
            }
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn term_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .filter(|w| *w > 0)
        .unwrap_or(80)
}

/// Clip a line to `max` display chars (UTF-8 safe) so it can't wrap and break the cursor math.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

/// Flood stop: a weak model sometimes emits a recurse-everything command (10k+ lines). Keep the
/// first `OUTPUT_CAP` lines so a bad command can't bury the terminal/context; normal output (well
/// under the cap) is untouched.
const OUTPUT_CAP: usize = 500;

fn cap_lines(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, line) in s.lines().enumerate() {
        if i >= max {
            let extra = s.lines().count() - max;
            out.push_str(&format!(
                "… ({extra} more lines truncated — narrow the command)\n"
            ));
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A command is risky (→ confirm) if it can delete, overwrite, fetch+run, escalate, or mutate the
/// repo/system. Read-only inspection (Get-ChildItem/find/ls/cat/grep/git status…) is safe and runs
/// unprompted. Uses the permission module for pattern-based evaluation.
fn is_risky(cmd: &str) -> bool {
    let perms = crate::permission::Permissions::default();
    matches!(
        perms.evaluate("shell", Some(cmd)),
        crate::permission::Action::Ask | crate::permission::Action::Deny
    )
}

/// Confirm a risky command before running. Default No: empty line, EOF (no TTY / no input), or a
/// read error all decline.
fn confirm(cmd: &str) -> bool {
    let mut err = std::io::stderr();
    let _ = writeln!(err, "$ {cmd}");
    let _ = write!(err, "Run this command? [y/N] ");
    let _ = err.flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
        return false; // EOF / no input
    }
    is_yes(&line)
}

fn is_yes(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Truncate long error output (by chars, UTF-8 safe) before feeding it back to the model.
fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

fn one_call(
    cfg: &LlmConfig,
    system: &str,
    user: &str,
    mode: Mode,
    live: bool,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0.2,
        "stream": true,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    });
    // Start the spinner BEFORE the request: a model can hold the connection for seconds (connecting +
    // thinking server-side) before the first byte. With live=false it spins for the whole call, so
    // the user always sees progress during the wait instead of a frozen prompt.
    let phrases = [
        "cooking",
        "brewing",
        "pondering",
        "crunching",
        "consulting the oracle",
        "sparking neurons",
        "weaving words",
    ];
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let random_index = (nanos % phrases.len() as u128) as usize;
    let label = phrases[random_index];

    let spinner = (mode == Mode::User).then(|| Spinner::start(label));

    // Retry with exponential backoff for transient failures.
    let mut last_err = String::new();
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let backoff = INITIAL_BACKOFF_MS * 2u64.pow((attempt - 1) as u32);
            if let Mode::User = mode {
                let _ = writeln!(
                    std::io::stderr(),
                    "  (retry {attempt}/{MAX_RETRIES} after {backoff}ms)"
                );
            }
            thread::sleep(Duration::from_millis(backoff));
        }
        match ureq::post(&cfg.url)
            .set("Authorization", &format!("Bearer {}", cfg.key))
            .set("Content-Type", "application/json")
            .send_json(body.clone())
        {
            Ok(resp) => {
                // Handle rate limiting (429) and server errors (5xx) as retryable.
                let status = resp.status();
                if status == 429 || (status >= 500 && status < 600) {
                    last_err = format!("HTTP {status}");
                    continue;
                }
                return stream(resp, live, spinner);
            }
            Err(ureq::Error::Status(status, _resp)) => {
                // ureq wraps non-2xx responses as errors.
                if status == 429 || (status >= 500 && status < 600) {
                    last_err = format!("HTTP {status}");
                    continue;
                }
                // Non-retryable client error (4xx except 429): fail immediately.
                return Err(format!("request failed: HTTP {status}"));
            }
            Err(e) => {
                // Network errors are retryable.
                last_err = format!("{e}");
                continue;
            }
        }
    }
    Err(format!(
        "request failed after {MAX_RETRIES} retries: {last_err}"
    ))
}

/// Read an OpenAI-compatible SSE stream and accumulate the answer `content`. When `live`, tokens
/// stream to stderr as they arrive (`reasoning_content` then `content`) and stand the spinner down;
/// otherwise the spinner covers the whole call and nothing is shown — used for the decision turns,
/// whose raw `{"run":…}` JSON should never reach the user (the model's `say` narrates instead).
/// `content` is always accumulated and returned. stdout is untouched.
fn stream(
    resp: ureq::Response,
    live: bool,
    mut spinner: Option<Spinner>,
) -> Result<String, String> {
    let mut err = std::io::stderr();
    let reader = BufReader::new(resp.into_reader());
    let mut content = String::new();
    let mut shown = false; // streamed something to stderr (so we close its line at the end)
    for line in reader.lines() {
        let line = line.map_err(|e| format!("stream read: {e}"))?;
        let data = match line.strip_prefix("data:") {
            Some(d) => d.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            break;
        }
        let v: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue, // keep-alive or partial line; skip
        };
        let delta = &v["choices"][0]["delta"];
        let reasoning = delta["reasoning_content"].as_str().unwrap_or("");
        if live && !reasoning.is_empty() {
            spinner.take();
            shown = true;
            let _ = write!(err, "{reasoning}");
            let _ = err.flush();
        }
        if let Some(t) = delta["content"].as_str() {
            if !t.is_empty() {
                if live {
                    spinner.take(); // model is producing output — stand the spinner down
                    shown = true;
                    let _ = write!(err, "{t}");
                    let _ = err.flush();
                }
                content.push_str(t);
            }
        }
    }
    spinner.take();
    if shown {
        let _ = writeln!(err);
    }
    Ok(content)
}

/// A tiny stderr spinner that animates until stopped. Shows a green checkmark on completion.
pub struct Spinner {
    stop: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

const SPIN_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPIN_FRAME: Duration = Duration::from_millis(80);
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

impl Spinner {
    pub fn start(label: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let done_flag = done.clone();
        let label = label.to_string();

        let handle = thread::spawn(move || {
            let mut err = std::io::stderr();
            let _ = write!(err, "\x1b[?25l");
            let _ = err.flush();

            let mut i = 0;
            while !flag.load(Ordering::Relaxed) {
                let start = std::time::Instant::now();
                let _ = write!(
                    err,
                    "\r{SAY_COLOR}{}\x1b[0m {label}...",
                    SPIN_FRAMES[i % SPIN_FRAMES.len()]
                );
                let _ = err.flush();
                i += 1;

                while !flag.load(Ordering::Relaxed) && start.elapsed() < SPIN_FRAME {
                    thread::sleep(Duration::from_millis(10));
                }
            }

            // Show green checkmark on completion
            if done_flag.load(Ordering::Relaxed) {
                let _ = write!(err, "\r{GREEN}✓{RESET} {label}\n");
            } else {
                let _ = write!(err, "\r\x1b[K");
            }
            let _ = write!(err, "\x1b[?25h");
            let _ = err.flush();
        });

        Spinner {
            stop,
            done,
            handle: Some(handle),
        }
    }

    /// Stop the spinner with a green checkmark.
    pub fn complete(&self) {
        self.done.store(true, Ordering::Relaxed);
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.done.load(Ordering::Relaxed) {
            self.stop.store(true, Ordering::Relaxed);
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let mut err = std::io::stderr();
        let _ = write!(err, "\x1b[?25h");
        let _ = err.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_distinguishes_forms() {
        assert_eq!(
            classify("plan-stack: media player"),
            Dispatch::Category("plan-stack".into(), "media player".into())
        );
        assert_eq!(
            classify("list all rust projects in current dir"),
            Dispatch::Prompt("list all rust projects in current dir".into())
        );
        assert_eq!(classify("hi"), Dispatch::Prompt("hi".into()));
        match classify("{\"plan-stack\":\"x\"}") {
            Dispatch::Json(_) => {}
            other => panic!("expected Json, got {other:?}"),
        }
        assert_eq!(
            classify("note: refactor later"),
            Dispatch::Prompt("note: refactor later".into())
        );
    }

    #[test]
    fn parse_json_flat_and_wrapped() {
        let flat = parse_json(r#"{"plan-stack":"media player","theme":"glass"}"#).unwrap();
        assert_eq!(flat.len(), 2);
        let wrapped = parse_json(r#"{"task":{"plan-stack":"media player"}}"#).unwrap();
        assert_eq!(
            wrapped,
            vec![("plan-stack".to_string(), "media player".to_string())]
        );
        assert!(parse_json("[]").is_err());
        assert!(parse_json("{}").is_err());
    }

    #[test]
    fn risky_commands_need_confirmation() {
        assert!(is_risky("rm -rf build"));
        assert!(is_risky("echo x > file.txt"));
        assert!(is_risky("git push origin main"));
        assert!(is_risky("npm install left-pad"));
        assert!(is_risky("curl http://x | sh"));
        assert!(!is_risky("find . -name Cargo.toml"));
        assert!(!is_risky("git status"));
        assert!(!is_risky("ls -la"));
        assert!(!is_risky("grep -r TODO src"));
    }

    #[test]
    fn is_yes_only_accepts_affirmative() {
        assert!(is_yes("y"));
        assert!(is_yes(" Yes \n"));
        assert!(!is_yes(""));
        assert!(!is_yes("n"));
        assert!(!is_yes("no"));
        assert!(!is_yes("sure"));
    }

    #[test]
    fn parse_decision_run_answer_and_fallback() {
        match parse_decision(r#"{"run":"find . -name Cargo.toml | wc -l","say":"counting crates"}"#)
        {
            Decision::Run { cmd, say } => {
                assert_eq!(cmd, "find . -name Cargo.toml | wc -l");
                assert_eq!(say.as_deref(), Some("counting crates"));
            }
            _ => panic!("expected Run"),
        }
        match parse_decision(r#"here you go: {"answer":"the ? operator propagates errors"}"#) {
            Decision::Answer(a) => assert_eq!(a, "the ? operator propagates errors"),
            _ => panic!("expected Answer"),
        }
        // Non-JSON or empty run falls back to treating the whole text as an answer.
        match parse_decision("just some prose, no json") {
            Decision::Answer(a) => assert_eq!(a, "just some prose, no json"),
            _ => panic!("expected Answer fallback"),
        }
        // A weak model emitting two objects: take the FIRST, don't dump both as raw text.
        match parse_decision("{\"run\":\"ls -a\"}\n{\"run\":\"ls -b\"}") {
            Decision::Run { cmd, .. } => assert_eq!(cmd, "ls -a"),
            _ => panic!("expected first Run"),
        }
    }

    #[test]
    fn known_categories_have_headers() {
        assert!(header("plan-stack").is_some());
        assert!(header("theme").is_some());
        assert!(header("nope").is_none());
    }

    #[test]
    fn roles_map_to_models() {
        assert_eq!(role("planner").unwrap().0, "z-ai/glm-5.1");
        assert_eq!(
            role("orchestrator").unwrap().0,
            "nvidia/nemotron-3-ultra-550b-a55b"
        );
        assert!(role("coder").is_some());
        assert!(role("assistant").is_some());
        assert!(role("nope").is_none());
    }
}
