//! Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0). Exposes Tokex's execution core
//! as the `run` tool so agents call it natively — RTK executes, Tokex returns it as structured,
//! agent-consumable events.
//!
//! ponytail: hand-rolled subset (initialize, tools/list, tools/call, ping) to keep the project
//! sync and tokio-free. Swap to the rmcp SDK only if we need the full spec or an async transport.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::config::Config;
use crate::intent::Intent;
use crate::llm::LlmConfig;
use crate::orchestrate::{self, Options};

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the stdio JSON-RPC loop until stdin closes. stdout is the protocol channel — nothing else
/// may write to it (the execution core writes to in-memory buffers instead).
pub fn serve() -> ! {
    eprintln!("tokex MCP server (stdio) — protocol {PROTOCOL_VERSION}");
    let cfg = crate::config::load();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(line) else {
            continue; // ignore malformed frames
        };
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));

        match dispatch(method, &params, &cfg) {
            Ok(None) => {} // notification: no reply
            Ok(Some(result)) => {
                if let Some(id) = id {
                    send(&mut stdout, json!({"jsonrpc":"2.0","id":id,"result":result}));
                }
            }
            Err((code, msg)) => {
                if let Some(id) = id {
                    send(
                        &mut stdout,
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}}),
                    );
                }
            }
        }
    }
    std::process::exit(0);
}

fn send(out: &mut impl Write, msg: Value) {
    writeln!(out, "{msg}").ok();
    out.flush().ok();
}

/// Returns Ok(None) for notifications, Ok(Some(result)) for replies, Err((code,msg)) for errors.
fn dispatch(method: &str, params: &Value, cfg: &Config) -> Result<Option<Value>, (i64, String)> {
    match method {
        "initialize" => Ok(Some(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "tokex", "version": env!("CARGO_PKG_VERSION")},
        }))),
        "notifications/initialized" => Ok(None),
        "ping" => Ok(Some(json!({}))),
        "tools/list" => Ok(Some(tools_list())),
        "tools/call" => Ok(Some(tools_call(params, cfg))),
        other => Err((-32601, format!("method not found: {other}"))),
    }
}

fn tools_list() -> Value {
    json!({"tools": [{
        "name": "run",
        "description": "Run a shell command through RTK and return normalized, structured execution \
events (stdout/stderr lines with severity, a result with exit code, and an optional LLM insight).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Command line, e.g. \"cargo test\""},
                "llm": {"type": "boolean", "description": "Compress output into an LLM insight"},
            },
            "required": ["command"],
        },
    }, {
        "name": "set_agent",
        "description": "Tell tokex which AI agent you are so it can install the graphify code-map \
skill for the right platform. Call this once with your platform id if a run result says the agent \
is unknown.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "agent": {"type": "string", "description": "graphify platform id, e.g. claude, codex, cursor, gemini, opencode"},
            },
            "required": ["agent"],
        },
    }]})
}

/// Dispatch a tools/call to the right handler.
fn tools_call(params: &Value, cfg: &Config) -> Value {
    match params.get("name").and_then(Value::as_str).unwrap_or("") {
        "run" => tool_run(params, cfg),
        "set_agent" => tool_set_agent(params),
        other => tool_error(format!("unknown tool: {other}")),
    }
}

/// `set_agent`: the model tells tokex its own platform (no TTY needed). Persists it and kicks off
/// the graphify skill install in the background — never writes to stdout (the JSON-RPC channel).
fn tool_set_agent(params: &Value) -> Value {
    let agent = params
        .get("arguments")
        .and_then(|a| a.get("agent"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if agent.is_empty() {
        return tool_error("missing required argument 'agent'".into());
    }
    let mut cfg = crate::config::load();
    cfg.agent = agent.clone();
    if let Err(e) = crate::config::save(&cfg) {
        return tool_error(format!("could not save config: {e}"));
    }
    crate::graphify::clear_skill_marker();
    crate::graphify::bootstrap_detached();
    json!({
        "content": [{"type": "text", "text": format!("Agent set to '{agent}'. Installing the graphify skill for it in the background.")}],
        "isError": false,
    })
}

/// Execute the `run` tool via the shared core, returning MCP tool-result content.
fn tool_run(params: &Value, cfg: &Config) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let command = args.get("command").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if command.is_empty() {
        return tool_error("missing required argument 'command'".into());
    }
    let llm_requested = args.get("llm").and_then(Value::as_bool).unwrap_or(false);

    let mut intent = Intent::from_command(command);
    intent.llm = llm_requested; // explicit force; llm mode only analyzes failures (below)
    let opts = Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: cfg.compression == "llm",
        footer: true,
    };
    // Best-effort LLM: if requested but unconfigured, just run without the insight.
    let llm_cfg = if intent.llm || opts.llm_on_failure { LlmConfig::from_config(cfg) } else { None };

    // Capture the machine channel; discard the human summary. stdout stays the protocol channel.
    let mut machine: Vec<u8> = Vec::new();
    let mut human = io::sink();
    match orchestrate::run(&intent, &mut machine, &mut human, llm_cfg.as_ref(), &opts) {
        Ok(code) => {
            if cfg.graph_auto {
                crate::graphify::auto_update(&intent.command);
            }
            // The machine channel is raw output lines plus a JSON result footer (and an insight
            // line on failure) — hand it back verbatim; re-wrapping it would only add tokens.
            let text = String::from_utf8_lossy(&machine).to_string();
            let mut content = vec![json!({"type": "text", "text": text})];
            // If we can't tell which agent this is, ask the model to identify itself so the graphify
            // code-map skill can be installed for the right platform.
            if cfg.graph_auto && crate::graphify::current_agent().is_none() {
                content.push(json!({"type": "text", "text": "note: tokex couldn't detect your agent, so the graphify code-map skill isn't installed. Call the set_agent tool with your platform id (e.g. claude, codex, cursor, gemini) to enable it."}));
            }
            json!({"content": content, "isError": code != 0})
        }
        Err(e) => tool_error(e),
    }
}

fn tool_error(msg: String) -> Value {
    json!({"content": [{"type": "text", "text": format!("error: {msg}")}], "isError": true})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_reports_protocol_and_name() {
        let r = dispatch("initialize", &json!({}), &Config::default()).unwrap().unwrap();
        assert_eq!(r["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(r["serverInfo"]["name"], "tokex");
    }

    #[test]
    fn tools_list_exposes_run() {
        let r = dispatch("tools/list", &json!({}), &Config::default()).unwrap().unwrap();
        assert_eq!(r["tools"][0]["name"], "run");
    }

    #[test]
    fn tools_list_exposes_set_agent() {
        let r = dispatch("tools/list", &json!({}), &Config::default()).unwrap().unwrap();
        let names: Vec<&str> =
            r["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"set_agent"));
    }

    #[test]
    fn set_agent_requires_agent() {
        let r = tools_call(&json!({"name": "set_agent", "arguments": {}}), &Config::default());
        assert_eq!(r["isError"], true);
    }

    #[test]
    fn initialized_is_a_notification() {
        assert_eq!(dispatch("notifications/initialized", &json!({}), &Config::default()), Ok(None));
    }

    #[test]
    fn unknown_method_errors() {
        assert!(dispatch("bogus", &json!({}), &Config::default()).is_err());
    }

    #[test]
    fn call_without_command_is_tool_error() {
        let r = tools_call(&json!({"name": "run", "arguments": {}}), &Config::default());
        assert_eq!(r["isError"], true);
    }
}
