//! Token usage tracking: record every run, persist to disk, show footers and summaries.
//!
//! Every command routed through cotrex (CLI, MCP, or agent) produces bytes-in / bytes-out.
//! We track these as "tokens" (~4 chars per token) and show the user how much they saved
//! compared to raw command output (which cotrex normalizes).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

const CHARS_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageEntry {
    pub timestamp: String,
    pub command: String,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub tokens_in: usize,
    pub tokens_out: usize,
    pub exit_code: i32,
    pub via: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageStats {
    pub total_runs: u64,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub total_input_bytes: u64,
    pub total_output_bytes: u64,
    #[serde(default)]
    pub entries: Vec<UsageEntry>,
}

impl Default for UsageStats {
    fn default() -> Self {
        UsageStats {
            total_runs: 0,
            total_tokens_in: 0,
            total_tokens_out: 0,
            total_input_bytes: 0,
            total_output_bytes: 0,
            entries: Vec::new(),
        }
    }
}

fn bytes_to_tokens(bytes: usize) -> usize {
    bytes / CHARS_PER_TOKEN
}

fn global_usage_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("cotrex").join("usage.json"))
}

fn project_usage_path() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|d| d.join(".cotrex").join("usage.json"))
}

fn load_from(path: &PathBuf) -> UsageStats {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<UsageStats>(&s).ok())
        .unwrap_or_default()
}

fn save_to(path: &PathBuf, stats: &UsageStats) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    serde_json::to_string_pretty(stats)
        .ok()
        .and_then(|s| fs::write(path, s).ok());
}

static GLOBAL: Mutex<Option<UsageStats>> = Mutex::new(None);

fn get_global() -> std::sync::MutexGuard<'static, Option<UsageStats>> {
    GLOBAL.lock().unwrap()
}

pub fn record(command: &str, input_bytes: usize, output_bytes: usize, exit_code: i32, via: &str) {
    let entry = UsageEntry {
        timestamp: chrono_now(),
        command: command.to_string(),
        input_bytes,
        output_bytes,
        tokens_in: bytes_to_tokens(input_bytes),
        tokens_out: bytes_to_tokens(output_bytes),
        exit_code,
        via: via.to_string(),
    };

    let tokens_in = entry.tokens_in as u64;
    let tokens_out = entry.tokens_out as u64;
    let input_b = entry.input_bytes as u64;
    let output_b = entry.output_bytes as u64;

    // Update in-memory global
    {
        let mut guard = get_global();
        let stats = guard.get_or_insert_with(|| {
            global_usage_path().map_or_else(UsageStats::default, |p| load_from(&p))
        });
        stats.total_runs += 1;
        stats.total_tokens_in += tokens_in;
        stats.total_tokens_out += tokens_out;
        stats.total_input_bytes += input_b;
        stats.total_output_bytes += output_b;
        stats.entries.push(entry.clone());
        // Keep last 500 entries in memory
        if stats.entries.len() > 500 {
            let drain = stats.entries.len() - 500;
            stats.entries.drain(..drain);
        }
        // Persist global
        if let Some(path) = global_usage_path() {
            save_to(&path, stats);
        }
    }

    // Update project-local
    if let Some(path) = project_usage_path() {
        let mut stats = load_from(&path);
        stats.total_runs += 1;
        stats.total_tokens_in += tokens_in;
        stats.total_tokens_out += tokens_out;
        stats.total_input_bytes += input_b;
        stats.total_output_bytes += output_b;
        stats.entries.push(entry);
        if stats.entries.len() > 500 {
            let drain = stats.entries.len() - 500;
            stats.entries.drain(..drain);
        }
        save_to(&path, &stats);
    }
}

pub fn summary() -> String {
    let guard = get_global();
    let stats = match guard.as_ref() {
        Some(s) => s.clone(),
        None => global_usage_path()
            .map(|p| load_from(&p))
            .unwrap_or_default(),
    };

    if stats.total_runs == 0 {
        return "No usage recorded yet.".to_string();
    }

    let raw_output_estimate = stats.total_output_bytes;

    format!(
        "Cotrex Usage\n\
         ─────────────────────────────\n\
         Total runs:        {}\n\
         Tokens in:         {} (~{} chars)\n\
         Tokens out:        {} (~{} chars)\n\
         Raw output saved:  ~{} chars normalized\n\
         Total I/O:         {} in / {} out\n\
         ─────────────────────────────\n\
         Tokens are free — cotrex normalizes output\n\
         so your agent processes less, not more.",
        stats.total_runs,
        stats.total_tokens_in,
        stats.total_input_bytes,
        stats.total_tokens_out,
        stats.total_output_bytes,
        raw_output_estimate,
        stats.total_input_bytes,
        stats.total_output_bytes,
    )
}

pub fn summary_json() -> serde_json::Value {
    let guard = get_global();
    let stats = match guard.as_ref() {
        Some(s) => s.clone(),
        None => global_usage_path()
            .map(|p| load_from(&p))
            .unwrap_or_default(),
    };
    serde_json::json!({
        "total_runs": stats.total_runs,
        "total_tokens_in": stats.total_tokens_in,
        "total_tokens_out": stats.total_tokens_out,
        "total_input_bytes": stats.total_input_bytes,
        "total_output_bytes": stats.total_output_bytes,
        "recent_commands": stats.entries.iter().rev().take(10).map(|e| {
            serde_json::json!({
                "command": e.command,
                "tokens_out": e.tokens_out,
                "exit_code": e.exit_code,
                "via": e.via,
            })
        }).collect::<Vec<_>>(),
    })
}

pub fn footer(_command: &str, input_bytes: usize, output_bytes: usize, exit_code: i32) -> String {
    let tokens_in = bytes_to_tokens(input_bytes);
    let tokens_out = bytes_to_tokens(output_bytes);
    let status = if exit_code == 0 { "ok" } else { "failed" };
    format!(
        "[tokens: in={} out={} | status={}]",
        tokens_in, tokens_out, status
    )
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "0".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_tokens_conversion() {
        assert_eq!(bytes_to_tokens(0), 0);
        assert_eq!(bytes_to_tokens(4), 1);
        assert_eq!(bytes_to_tokens(100), 25);
    }

    #[test]
    fn footer_contains_token_counts() {
        let f = footer("cargo test", 50, 200, 0);
        assert!(f.contains("tokens: in=12 out=50"));
        assert!(f.contains("status=ok"));
    }

    #[test]
    fn footer_shows_failed_status() {
        let f = footer("cargo build", 40, 100, 1);
        assert!(f.contains("status=failed"));
    }
}
