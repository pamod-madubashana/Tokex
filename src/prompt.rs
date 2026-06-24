//! Prompts.
//!
//! A single quoted argument is a *prompt*, not a command. Free text is a **task**: the model turns
//! it into one shell command and tokex *runs* it, returning the command's output (not the command).
//! `category: text` (or a JSON object of several) instead returns a structured answer using that
//! category's header — those aren't runnable commands.
//!
//! Two presentation modes:
//! - **User** (`tokex "…"`): a spinner while waiting, then the model's text streamed live to stderr.
//! - **Model** (`tokex -m "…"`): no spinner, no thinking — just the output on stdout.
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
        "You name the single best application tech stack for a developer's task. Output ONLY \
minified JSON with exactly two keys: stack (short lowercase stack name) and reason (one concise \
sentence). Do NOT output code, commands, file contents, install steps, markdown, or any other field.",
    ),
    (
        "theme",
        "You are a senior UI designer. Given a short style description, output ONLY minified JSON \
with keys: palette (array of hex colors), font (string), effects (array of short phrases), \
rationale (one concise sentence). No code, no markdown, no other fields.",
    ),
];

// Used when a category prompt has no recognized category (rare: a JSON object with an empty key).
const DEFAULT_HEADER: &str = "You are a concise senior software engineer. Answer the developer's \
question briefly and practically. No preamble, no markdown headings.";

// A task either runs a command (and we return the REAL output) or is answered in text. The model
// decides and replies with JSON: {"run":"<command>"} or {"answer":"<text>"}.
const DECISION_SYSTEM: &str = "You fulfill a developer's request from their CURRENT working \
directory by gathering information with shell commands and then ANALYZING it into a clear answer. \
Each turn, reply with EXACTLY ONE JSON object:\n\
- {\"run\":\"<command>\"} — run one command to gather info; you'll see its output and can run more. \
Inspect step by step: ONE level at a time, skip vendored/build dirs (vendor, target, node_modules, \
.git, dist), never dump the whole recursive tree.\n\
- {\"answer\":\"<text>\"} — your final answer for the user. SYNTHESIZE what you found; do NOT just \
paste raw command output. Wrap any file tree/table/aligned layout in a fenced ``` code block.\n\
Always finish with an {\"answer\"}. Use the fewest commands that do the job. ONE simple correct \
command per turn; stay within the current directory tree. Output ONLY the JSON.";


/// The header (system prompt) bound to a category, if it is known.
pub fn header(category: &str) -> Option<&'static str> {
    CATEGORIES.iter().find(|(n, _)| *n == category).map(|(_, h)| *h)
}

// Roles: `tokex <role> "<task>"` offloads a small task to a role-specific model and returns its
// answer, so the calling agent just waits (and spends no tokens thinking). Each row is
// (role, model id, header). The model ids are NVIDIA NIM ids served by the configured endpoint;
// add or retune a role by editing a row.
const ROLES: &[(&str, &str, &str)] = &[
    (
        "planner",
        "z-ai/glm-5.1",
        "You are a planning specialist. Given a goal, produce a concise, ordered, actionable plan as \
numbered steps. No preamble, no code unless essential.",
    ),
    (
        "router",
        "nvidia/nemotron-3-nano-30b-a3b",
        "You are a router. Given a request, decide the single best next action, tool, or role and \
answer in one or two decisive lines. No deliberation in the output.",
    ),
    (
        "orchestrator",
        "nvidia/nemotron-3-ultra-550b-a55b",
        "You are an orchestrator. Break the goal into an ordered list of concrete shell commands or \
steps that accomplish it end to end. Be specific and minimal. No prose beyond the steps.",
    ),
    (
        "coder",
        "deepseek-ai/deepseek-v4-flash",
        "You are a senior engineer. Output only the code that solves the task — correct, minimal, \
idiomatic. No explanation unless the task asks for it.",
    ),
    (
        "assistant",
        "qwen/qwen3-next-80b-a3b-instruct",
        "You are a concise developer assistant. Answer the request briefly and practically. No fluff.",
    ),
];

/// `(model id, header)` for a role, if known.
pub fn role(name: &str) -> Option<(&'static str, &'static str)> {
    ROLES.iter().find(|(n, _, _)| *n == name).map(|(_, m, h)| (*m, *h))
}

/// Build an `LlmConfig` that reuses the configured endpoint + key but swaps in `model`.
pub fn with_model(base: &LlmConfig, model: &str) -> LlmConfig {
    LlmConfig { url: base.url.clone(), key: base.key.clone(), model: model.to_string() }
}

/// How a single bare argument should be handled.
#[derive(Debug, PartialEq)]
pub enum Dispatch {
    /// JSON object of `category -> text` (possibly several). Pass the raw string to `parse_json`.
    Json(String),
    /// `category: text` with a known category — returns a structured answer.
    Category(String, String),
    /// Free-text task — the model produces a command and tokex runs it.
    Prompt(String),
    /// A single bare token — run it as a command, not a prompt.
    Command(String),
}

/// Classify one argument. Quotes (i.e. a single arg) reach here; multi-arg invocations are commands
/// and never get classified.
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
    // Whitespace means it reads as a sentence → a task; a lone token is a command (e.g. `ls`).
    if s.split_whitespace().count() > 1 {
        Dispatch::Prompt(s.to_string())
    } else {
        Dispatch::Command(s.to_string())
    }
}

/// Parse the JSON multi-category form into `(category, text)` pairs. Accepts a flat object
/// `{"plan-stack":"…","theme":"…"}` or a `{"task": { … }}` wrapper.
pub fn parse_json(s: &str) -> Result<Vec<(String, String)>, String> {
    let v: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("invalid JSON prompt: {e}"))?;
    let obj = match v.get("task").and_then(|t| t.as_object()) {
        Some(o) => o.clone(),
        None => v.as_object().ok_or("JSON prompt must be an object")?.clone(),
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
/// Returns the exit code (0 for an answered task). Prints the real command output, or the answer,
/// to stdout.
pub fn fulfill(
    task: &str,
    cfg: &LlmConfig,
    role_header: Option<&str>,
    mode: Mode,
    opts: &Options,
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
    for _ in 0..MAX_STEPS {
        let user = if transcript.is_empty() {
            task.to_string()
        } else {
            format!("Request: {task}\n\nCommands run so far:\n{transcript}\nGather more if needed, else answer.")
        };
        match parse_decision(&one_call(cfg, &system, &user, mode)?) {
            Decision::Answer(text) => {
                print_answer(&text, mode);
                return Ok(0);
            }
            Decision::Run(cmd) => {
                if is_risky(&cmd) {
                    if !confirm(&cmd) {
                        let _ = writeln!(std::io::stderr(), "aborted (not confirmed).");
                        return Ok(130); // 128 + SIGINT
                    }
                } else {
                    let _ = writeln!(std::io::stderr(), "$ {cmd}"); // safe → show what runs
                }
                let (code, out) = exec_capture(&cmd, opts, mode)?;
                transcript.push_str(&format!("$ {cmd}\n(exit {code})\n{}\n\n", trunc(&out, 1500)));
            }
        }
    }
    // Out of steps: force a final answer from what we've gathered.
    let user = format!(
        "Request: {task}\n\nCommands run so far:\n{transcript}\nGive your final answer now as {{\"answer\":\"...\"}}."
    );
    if let Decision::Answer(text) = parse_decision(&one_call(cfg, &system, &user, mode)?) {
        print_answer(&text, mode);
    }
    Ok(0)
}

// Max commands the model may run to gather info before it must give a final answer.
const MAX_STEPS: usize = 6;

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
    /// Run a command to gather info (the loop always continues and feeds the output back).
    Run(String),
    Answer(String),
}

/// Read the model's JSON decision: `{"run":…}` → gather via a command, `{"answer":…}` → final text.
/// Anything that isn't our JSON is treated as a plain answer (graceful when a model strays).
fn parse_decision(content: &str) -> Decision {
    if let (Some(a), Some(b)) = (content.find('{'), content.rfind('}')) {
        if a < b {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content[a..=b]) {
                if let Some(cmd) = v.get("run").and_then(|x| x.as_str()) {
                    if !cmd.trim().is_empty() {
                        return Decision::Run(cmd.trim().to_string());
                    }
                }
                if let Some(ans) = v.get("answer").and_then(|x| x.as_str()) {
                    return Decision::Answer(ans.trim().to_string());
                }
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
        let line = format!("powershell -NoProfile -ExecutionPolicy Bypass -File {}", p.display());
        (p, line, format!("$ErrorActionPreference = 'Stop'\n{cmd}\n"))
    } else {
        let p = std::env::temp_dir().join(format!("tokex-task-{pid}.sh"));
        let line = format!("bash {}", p.display());
        (p, line, format!("set -e\n{cmd}\n"))
    };
    std::fs::write(&tmp, content).map_err(|e| format!("temp script: {e}"))?;
    // ponytail: temp paths have no spaces, so the unquoted path is safe; quote only if that breaks.
    let exec = Options { raw: true, footer: false, llm_on_failure: false, ..*opts };
    // Capture every line, and in User mode show the last 5 live in a ```bash viewport (see TailView).
    let mut view = TailView::new(mode);
    let result = orchestrate::run(&Intent::from_command(run_line), &mut view, &mut std::io::sink(), None, &exec);
    let buf = view.finish();
    let _ = std::fs::remove_file(&tmp);
    let code = result?;
    Ok((code, cap_lines(&String::from_utf8_lossy(&buf), OUTPUT_CAP)))
}

/// Live tail of a running command. Captures every byte (returned to the caller) while redrawing the
/// last `TAIL_ROWS` lines in place on stderr inside a ```bash fence, so a human sees output stream by
/// without it scrolling the whole terminal. The viewport is erased when the command finishes, leaving
/// only the synthesized answer. Off in Model mode or when stderr isn't a TTY (no cursor control).
/// ponytail: fixed-height in-place redraw with relative cursor moves; fine unless it straddles a
/// scroll region — good enough for a transient progress view.
const TAIL_ROWS: usize = 5;

struct TailView {
    full: Vec<u8>,
    pending: String,
    tail: VecDeque<String>,
    shown: bool,
    live: bool,
}

impl TailView {
    fn new(mode: Mode) -> Self {
        let live = mode == Mode::User && std::io::stderr().is_terminal();
        TailView { full: Vec::new(), pending: String::new(), tail: VecDeque::new(), shown: false, live }
    }

    fn push_line(&mut self, line: String) {
        if self.tail.len() == TAIL_ROWS {
            self.tail.pop_front();
        }
        self.tail.push_back(line);
        self.redraw();
    }

    /// Redraw the fixed 7-line block (open fence + 5 rows + close fence) in place.
    fn redraw(&mut self) {
        let mut err = std::io::stderr();
        let w = term_width().saturating_sub(1);
        if self.shown {
            let _ = write!(err, "\r\x1b[{}A", TAIL_ROWS + 1); // back up to the open fence
        }
        let _ = writeln!(err, "\x1b[K```bash");
        for i in 0..TAIL_ROWS {
            let line = self.tail.get(i).map(String::as_str).unwrap_or("");
            let _ = writeln!(err, "\x1b[K{}", clip(line, w));
        }
        let _ = write!(err, "\x1b[K```");
        let _ = err.flush();
        self.shown = true;
    }

    /// Return the full captured output, erasing the viewport first so the final answer stays clean.
    fn finish(self) -> Vec<u8> {
        if self.live && self.shown {
            let mut err = std::io::stderr();
            let _ = write!(err, "\r\x1b[{}A\x1b[J", TAIL_ROWS + 1); // up to the fence, clear downward
            let _ = err.flush();
        }
        self.full
    }
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
    std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()).filter(|w| *w > 0).unwrap_or(80)
}

/// Clip a line to `max` display chars (UTF-8 safe) so it can't wrap and break the cursor math.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
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
            out.push_str(&format!("… ({extra} more lines truncated — narrow the command)\n"));
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A command is risky (→ confirm) if it can delete, overwrite, fetch+run, escalate, or mutate the
/// repo/system. Read-only inspection (Get-ChildItem/find/ls/cat/grep/git status…) is safe and runs
/// unprompted. Covers both POSIX tools and PowerShell cmdlets.
/// ponytail: substring blocklist, not a parser; err toward asking on anything that writes.
fn is_risky(cmd: &str) -> bool {
    const RISKY: &[&str] = &[
        // POSIX
        "rm ", "rmdir", "mv ", "dd ", "mkfs", "chmod", "chown", "kill", "shutdown", "reboot",
        "sudo", " su ", "truncate", "del ", "format ", "fdisk", ">", "tee ", "curl", "wget",
        "git push", "git reset", "git clean", "git checkout", "git rebase", "git commit",
        "install", "uninstall", "apt", "brew",
        // PowerShell cmdlets
        "remove-item", "move-item", "rename-item", "set-content", "add-content", "out-file",
        "new-item", "clear-content", "stop-process", "invoke-webrequest", "invoke-expression",
        "iex ", "start-process",
    ];
    let c = format!(" {} ", cmd.to_ascii_lowercase());
    RISKY.iter().any(|p| c.contains(p))
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

/// Run category prompts and collect the answers into one JSON object keyed by category (`answer`
/// when there's no category). Used for structured categories, not runnable tasks.
pub fn run(pairs: &[(String, String)], cfg: &LlmConfig, mode: Mode) -> Result<serde_json::Value, String> {
    let mut results = serde_json::Map::new();
    for (cat, text) in pairs {
        let system = if cat.is_empty() {
            DEFAULT_HEADER
        } else {
            header(cat).ok_or_else(|| format!("unknown category '{cat}'"))?
        };
        let answer = one_call(cfg, system, text, mode)?;
        let key = if cat.is_empty() { "answer" } else { cat.as_str() };
        results.insert(key.to_string(), as_value(&answer));
    }
    Ok(serde_json::Value::Object(results))
}

/// Embed the model's answer as parsed JSON when it returned an object, else as a plain string.
fn as_value(answer: &str) -> serde_json::Value {
    if let (Some(a), Some(b)) = (answer.find('{'), answer.rfind('}')) {
        if a < b {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&answer[a..=b]) {
                return v;
            }
        }
    }
    serde_json::Value::String(answer.trim().to_string())
}

fn one_call(cfg: &LlmConfig, system: &str, user: &str, mode: Mode) -> Result<String, String> {
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0.2,
        "stream": true,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    });
    // Start the spinner BEFORE the request: a reasoning model can hold the connection for seconds
    // (connecting + thinking server-side) before the first byte. Dropped on error or first token.
    let spinner = (mode == Mode::User).then(|| Spinner::start("thinking"));
    let resp = ureq::post(&cfg.url)
        .set("Authorization", &format!("Bearer {}", cfg.key))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;
    stream(resp, mode, spinner)
}

/// Read an OpenAI-compatible SSE stream and accumulate the answer `content`. In User mode the
/// spinner (started by the caller) runs until the first reasoning token, then the model's *thinking*
/// streams live to stderr — the `content` (the JSON answer/command) is accumulated silently so the
/// caller can print a clean result to stdout. In Model mode nothing is shown. stdout is untouched.
fn stream(resp: ureq::Response, mode: Mode, mut spinner: Option<Spinner>) -> Result<String, String> {
    let mut err = std::io::stderr();
    let reader = BufReader::new(resp.into_reader());
    let mut content = String::new();
    let mut shown_thinking = false;
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
        if mode == Mode::User && !reasoning.is_empty() {
            spinner.take(); // stop + clear the spinner on the first thinking token
            let _ = write!(err, "{reasoning}");
            let _ = err.flush();
            shown_thinking = true;
        }
        if let Some(t) = delta["content"].as_str() {
            content.push_str(t);
        }
    }
    spinner.take(); // stop the spinner if it never showed thinking (e.g. an instruct model)
    if shown_thinking {
        let _ = writeln!(err);
    }
    Ok(content)
}

/// A tiny stderr spinner that animates until dropped. ponytail: ASCII frames (render everywhere,
/// incl. cmd.exe) + an atomic stop flag; cleared by overwriting with spaces, not an ANSI escape.
struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    fn start(label: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let label = label.to_string();
        let handle = thread::spawn(move || {
            let frames = ['|', '/', '-', '\\'];
            let mut err = std::io::stderr();
            let mut i = 0;
            while !flag.load(Ordering::Relaxed) {
                let _ = write!(err, "\r{} {label}...", frames[i % frames.len()]);
                let _ = err.flush();
                i += 1;
                thread::sleep(Duration::from_millis(100));
            }
            let _ = write!(err, "\r{}\r", " ".repeat(label.len() + 6)); // clear the line
            let _ = err.flush();
        });
        Spinner { stop, handle: Some(handle) }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
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
        assert_eq!(classify("ls"), Dispatch::Command("ls".into()));
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
        assert_eq!(wrapped, vec![("plan-stack".to_string(), "media player".to_string())]);
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
        match parse_decision(r#"{"run":"find . -name Cargo.toml | wc -l"}"#) {
            Decision::Run(cmd) => assert_eq!(cmd, "find . -name Cargo.toml | wc -l"),
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
    }

    #[test]
    fn as_value_parses_json_or_keeps_string() {
        assert!(as_value(r#"here: {"stack":"rust","reason":"fast"}"#).is_object());
        assert_eq!(as_value("just text"), serde_json::Value::String("just text".into()));
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
        assert_eq!(role("orchestrator").unwrap().0, "nvidia/nemotron-3-ultra-550b-a55b");
        assert!(role("coder").is_some());
        assert!(role("assistant").is_some());
        assert!(role("nope").is_none());
    }
}
