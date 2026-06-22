//! Execution Orchestrator: validate intent, forward to RTK, normalize the stream.
//!
//! Tokex does not run the raw command — it spawns `rtk <args>` and reads RTK's pipes. Two reader
//! threads feed an mpsc channel so stdout and stderr interleave live.
//! ponytail: 2 threads + mpsc; swap to async only if we ever multiplex many concurrent execs.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::intent::Intent;
use crate::llm::LlmConfig;
use crate::normalize::{normalize, Channel, LineEvent, Severity};

/// Keep the last N output lines for the LLM. RTK already pre-filters, so this is a token guard.
/// ponytail: last 200 lines; raise only if compression misses context past that window.
const RAW_CAP: usize = 200;

#[derive(serde::Serialize)]
struct Result_ {
    #[serde(rename = "type")]
    kind: &'static str,
    status: &'static str,
    code: i32,
}

enum Msg {
    Line(LineEvent),
    Done,
}

/// Resolve the rtk binary: prefer one vendored next to our own exe (workspace build drops both in
/// the same target dir), else fall back to `rtk` on PATH.
fn rtk_path() -> std::path::PathBuf {
    let name = if cfg!(windows) { "rtk.exe" } else { "rtk" };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(cand) = exe.parent().map(|d| d.join(name)) {
            if cand.is_file() {
                return cand;
            }
        }
    }
    std::path::PathBuf::from("rtk")
}

/// Execution options derived from config modes.
pub struct Options {
    /// compression=off: bypass rtk filtering (raw `rtk run -c`).
    pub raw: bool,
    /// rtk_verbosity=ultra-compact: pass `--ultra-compact` to rtk.
    pub ultra_compact: bool,
}

/// Run the intent through RTK. Writes normalized NDJSON events to `machine` (stdout) and a
/// human summary to `human` (stderr). Returns the process exit code.
pub fn run(
    intent: &Intent,
    machine: &mut impl Write,
    human: &mut impl Write,
    llm: Option<&LlmConfig>,
    opts: &Options,
) -> Result<i32, String> {
    intent.validate()?;
    let mut args = if opts.raw {
        vec!["run".to_string(), "-c".to_string(), intent.command.trim().to_string()]
    } else {
        intent.to_rtk_args()
    };
    if opts.ultra_compact {
        args.push("--ultra-compact".to_string());
    }

    // PROCESS_START on the human channel only; machine channel is pure line/result events.
    writeln!(human, "› rtk {}", args.join(" ")).ok();

    let mut child = Command::new(rtk_path())
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn rtk (is it on PATH?): {e}"))?;

    let (tx, rx) = mpsc::channel();
    let reader = |ch: Channel, pipe: Option<Box<dyn std::io::Read + Send>>, tx: mpsc::Sender<Msg>| {
        thread::spawn(move || {
            if let Some(p) = pipe {
                for line in BufReader::new(p).lines() {
                    match line {
                        Ok(l) => {
                            tx.send(Msg::Line(normalize(ch, l))).ok();
                        }
                        Err(_) => break,
                    }
                }
            }
            tx.send(Msg::Done).ok();
        })
    };

    let out = child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>);
    let err = child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>);
    reader(Channel::Stdout, out, tx.clone());
    reader(Channel::Stderr, err, tx);

    let mut errors = 0usize;
    let mut raw: Vec<String> = Vec::new();
    let mut open = 2;
    while open > 0 {
        match rx.recv() {
            Ok(Msg::Line(ev)) => {
                if ev.severity == Severity::Error {
                    errors += 1;
                }
                writeln!(machine, "{}", serde_json::to_string(&ev).unwrap()).ok();
                raw.push(ev.line);
                if raw.len() > RAW_CAP {
                    raw.remove(0);
                }
            }
            Ok(Msg::Done) => open -= 1,
            Err(_) => break,
        }
    }

    let code = child.wait().map_err(|e| format!("wait failed: {e}"))?.code().unwrap_or(-1);
    let status = if code == 0 { "ok" } else { "failed" };
    let result = Result_ { kind: "result", status, code };
    writeln!(machine, "{}", serde_json::to_string(&result).unwrap()).ok();
    writeln!(human, "‹ {status} (exit {code}, {errors} error line(s))").ok();

    // Optional LLM compression: best-effort. A failed call never fails the exec.
    if intent.llm {
        if let Some(cfg) = llm {
            match crate::llm::compress(cfg, &intent.command, code, &raw.join("\n")) {
                Ok(ins) => {
                    let mut ev = serde_json::to_value(&ins).unwrap();
                    ev["type"] = serde_json::json!("insight");
                    writeln!(machine, "{ev}").ok();
                    writeln!(human, "  ⟐ {}", ins.root_cause).ok();
                    if !ins.suggested_fix.is_empty() {
                        writeln!(human, "  → {}", ins.suggested_fix).ok();
                    }
                }
                Err(e) => {
                    writeln!(human, "  (llm skipped: {e})").ok();
                }
            }
        }
    }
    Ok(code)
}
