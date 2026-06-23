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

use std::io::{BufRead, BufReader, Write};
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

// Free-text tasks: turn the request into ONE shell command that tokex then runs.
const TASK_SYSTEM: &str = "Convert the user's request into ONE shell command that accomplishes it. \
Output ONLY the command on a single line — no explanation, no markdown, no code fences. Prefer \
portable POSIX tools.";

/// The header (system prompt) bound to a category, if it is known.
pub fn header(category: &str) -> Option<&'static str> {
    CATEGORIES.iter().find(|(n, _)| *n == category).map(|(_, h)| *h)
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

/// Turn a free-text task into a command, run it through rtk, and return the exit code. The output
/// is the command's output: live on stdout for User mode, captured-and-printed for Model mode.
pub fn run_task(cfg: &LlmConfig, task: &str, mode: Mode, opts: &Options) -> Result<i32, String> {
    let cmd = gen_command(cfg, task, mode)?;
    let intent = Intent::from_command(&cmd);
    // No LLM insight on the run itself — the agent asked for output, not analysis.
    let exec = Options { footer: false, llm_on_failure: false, ..*opts };
    match mode {
        Mode::User => {
            let mut err = std::io::stderr();
            let _ = writeln!(err, "$ {cmd}");
            orchestrate::run(&intent, &mut std::io::stdout(), &mut err, None, &exec)
        }
        Mode::Model => {
            // Output only: discard the human channel, footer already suppressed.
            let mut out = std::io::stdout();
            orchestrate::run(&intent, &mut out, &mut std::io::sink(), None, &exec)
        }
    }
}

fn gen_command(cfg: &LlmConfig, task: &str, mode: Mode) -> Result<String, String> {
    let system = format!("{TASK_SYSTEM} Target OS: {}.", std::env::consts::OS);
    let answer = one_call(cfg, &system, task, mode)?;
    let cmd = extract_command(&answer);
    if cmd.is_empty() {
        return Err("model returned no command".into());
    }
    Ok(cmd)
}

/// Pull a single command line out of the model's answer, tolerating ``` fences or a JSON wrapper.
fn extract_command(text: &str) -> String {
    let mut t = text.trim();
    if let Some(rest) = t.strip_prefix("```") {
        // drop an optional language tag, then the trailing fence
        let rest = rest.trim_start_matches(|c: char| c.is_alphanumeric()).trim();
        t = rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    if t.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
            for k in ["command", "cmd", "answer"] {
                if let Some(c) = v.get(k).and_then(|x| x.as_str()) {
                    return c.trim().to_string();
                }
            }
        }
    }
    // ponytail: first non-empty line; multi-line commands (heredocs) lose their tail — fix if needed.
    t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string()
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
    let resp = ureq::post(&cfg.url)
        .set("Authorization", &format!("Bearer {}", cfg.key))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;
    stream(resp, mode)
}

/// Read an OpenAI-compatible SSE stream and accumulate the answer `content`. In User mode a spinner
/// runs until the first token, then reasoning + text stream live to stderr; in Model mode nothing is
/// shown. stdout is never touched here.
fn stream(resp: ureq::Response, mode: Mode) -> Result<String, String> {
    let mut err = std::io::stderr();
    let mut spinner = (mode == Mode::User).then(|| Spinner::start("thinking"));
    let reader = BufReader::new(resp.into_reader());
    let mut content = String::new();
    let mut shown = false;
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
        let text = delta["content"].as_str().unwrap_or("");
        if mode == Mode::User && (!reasoning.is_empty() || !text.is_empty()) {
            spinner.take(); // drop -> stops + clears the spinner line on first token
            let _ = write!(err, "{reasoning}{text}");
            let _ = err.flush();
            shown = true;
        }
        content.push_str(text);
    }
    spinner.take();
    if shown {
        let _ = writeln!(err);
    }
    Ok(content)
}

/// A tiny stderr spinner that animates until dropped. ponytail: braille frames + an atomic stop
/// flag; no progress crate for a single waiting indicator.
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
            let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let mut err = std::io::stderr();
            let mut i = 0;
            while !flag.load(Ordering::Relaxed) {
                let _ = write!(err, "\r{} {label}...", frames[i % frames.len()]);
                let _ = err.flush();
                i += 1;
                thread::sleep(Duration::from_millis(80));
            }
            let _ = write!(err, "\r\x1b[K"); // clear the line
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
    fn extract_command_unwraps_fences_and_json() {
        assert_eq!(extract_command("find . -name Cargo.toml | sort"), "find . -name Cargo.toml | sort");
        assert_eq!(extract_command("```bash\nls -la\n```"), "ls -la");
        assert_eq!(extract_command(r#"{"answer":"find . -name Cargo.toml"}"#), "find . -name Cargo.toml");
        assert_eq!(extract_command(r#"{"command":"git status"}"#), "git status");
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
}
