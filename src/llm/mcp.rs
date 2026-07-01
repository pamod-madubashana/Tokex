//! Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0). Exposes Cotrex's execution core
//! as the `run` tool so agents call it natively — RTK executes, Cotrex returns it as structured,
//! agent-consumable events.
//!
//! ponytail: hand-rolled subset (initialize, tools/list, tools/call, ping) to keep the project
//! sync and tokio-free. Swap to the rmcp SDK only if we need the full spec or an async transport.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::config::Config;
use crate::core::intent::Intent;
use crate::core::orchestrate::{self, Options};
use crate::llm::LlmConfig;

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the stdio JSON-RPC loop until stdin closes. stdout is the protocol channel — nothing else
/// may write to it (the execution core writes to in-memory buffers instead).
pub fn serve() -> ! {
    eprintln!("cotrex MCP server (stdio) — protocol {PROTOCOL_VERSION}");
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
                    send(
                        &mut stdout,
                        json!({"jsonrpc":"2.0","id":id,"result":result}),
                    );
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
            "serverInfo": {"name": "cotrex", "version": env!("CARGO_PKG_VERSION")},
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
        "description": "Tell cotrex which AI agent you are so it can install the graphify code-map \
    skill for the right platform. Call this once with your platform id if a run result says the agent \
    is unknown.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "agent": {"type": "string", "description": "graphify platform id, e.g. claude, codex, cursor, gemini, opencode"},
            },
            "required": ["agent"],
        },
    }, {
        "name": "list_roles",
        "description": "List all available roles (planner, coder, assistant, etc.) with their \
    models and capabilities. Use this to see what roles are available before delegating tasks.",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    }, {
        "name": "delegate",
        "description": "Delegate a task to a specific role. The role's model will run commands to \
    gather info and return an analyzed answer. Use list_roles to see available roles.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "The task to delegate, e.g. \"analyze the project structure\""},
                "role": {"type": "string", "description": "Role name (default: assistant). Options: planner, router, orchestrator, coder, assistant"},
            },
            "required": ["task"],
        },
    }, {
        "name": "plan",
        "description": "Create an ordered plan for a task. Shorthand for delegate with the planner role.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "The task to plan, e.g. \"build a music player app\""},
            },
            "required": ["task"],
        },
    }, {
        "name": "usage",
        "description": "Show token usage statistics — how many tokens cotrex has processed across all runs.",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    }, {
        "name": "graphify_query",
        "description": "Query the graphify knowledge graph using BFS or DFS traversal. Returns nodes and edges relevant to the question.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "The question to search for in the graph"},
                "dfs": {"type": "boolean", "description": "Use DFS mode instead of BFS (default: false)"},
                "budget": {"type": "integer", "description": "Token budget for output (default: 2000)"},
            },
            "required": ["question"],
        },
    }, {
        "name": "graphify_path",
        "description": "Find the shortest path between two concepts in the knowledge graph.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "node_a": {"type": "string", "description": "Source concept name"},
                "node_b": {"type": "string", "description": "Target concept name"},
            },
            "required": ["node_a", "node_b"],
        },
    }, {
        "name": "graphify_explain",
        "description": "Get a plain-language explanation of a node and all its connections in the knowledge graph.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "node_name": {"type": "string", "description": "Node name to explain"},
            },
            "required": ["node_name"],
        },
    }, {
        "name": "graphify_add",
        "description": "Fetch a URL (webpage, paper, tweet, PDF) and add it to the graphify corpus.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "URL to fetch"},
                "author": {"type": "string", "description": "Author tag"},
                "contributor": {"type": "string", "description": "Contributor tag"},
            },
            "required": ["url"],
        },
    }, {
        "name": "graphify_save_result",
        "description": "Save a Q&A result back into the knowledge graph to improve future queries.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "The question that was asked"},
                "answer": {"type": "string", "description": "The answer text"},
                "result_type": {"type": "string", "description": "Type: query, path_query, or explain"},
                "nodes": {"type": "array", "items": {"type": "string"}, "description": "Node labels cited in the answer"},
            },
            "required": ["question", "answer"],
        },
    }, {
        "name": "graphify_export",
        "description": "Export the graph in various formats (svg, graphml, neo4j).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "format": {"type": "string", "description": "Export format: svg, graphml, or neo4j"},
            },
            "required": ["format"],
        },
    }]})
}

/// Dispatch a tools/call to the right handler.
fn tools_call(params: &Value, cfg: &Config) -> Value {
    match params.get("name").and_then(Value::as_str).unwrap_or("") {
        "run" => tool_run(params, cfg),
        "set_agent" => tool_set_agent(params),
        "list_roles" => tool_list_roles(),
        "delegate" => tool_delegate(params, cfg),
        "plan" => tool_plan(params, cfg),
        "usage" => tool_usage(),
        "graphify_query" => tool_graphify_query(params),
        "graphify_path" => tool_graphify_path(params),
        "graphify_explain" => tool_graphify_explain(params),
        "graphify_add" => tool_graphify_add(params),
        "graphify_save_result" => tool_graphify_save_result(params),
        "graphify_export" => tool_graphify_export(params),
        other => tool_error(format!("unknown tool: {other}")),
    }
}

/// `set_agent`: the model tells cotrex its own platform (no TTY needed). Persists it and kicks off
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

/// `list_roles`: return all available roles with their models and capabilities.
fn tool_list_roles() -> Value {
    let roles = crate::agent::prompt::roles_list();
    let items: Vec<Value> = roles
        .iter()
        .map(|(name, model, desc)| {
            json!({
                "name": name,
                "model": model,
                "description": desc,
            })
        })
        .collect();
    json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&items).unwrap_or_default()}],
        "isError": false,
    })
}

/// `delegate`: invoke a role with a task and return the answer.
fn tool_delegate(params: &Value, cfg: &Config) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let task = args
        .get("task")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if task.is_empty() {
        return tool_error("missing required argument 'task'".into());
    }
    let role_name = args
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("assistant")
        .trim();

    let Some((model, header, _mode, max_steps)) = crate::agent::prompt::role(role_name) else {
        return tool_error(format!("unknown role: {role_name}"));
    };

    let llm_cfg = match LlmConfig::from_config(cfg) {
        Some(c) => crate::agent::prompt::with_model(&c, model),
        None => return tool_error("LLM not configured — run `cotrex setup` first".into()),
    };

    let opts = Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: cfg.compression == "llm",
        footer: false,
    };

    match crate::agent::prompt::fulfill_and_capture(&task, &llm_cfg, Some(header), &opts, max_steps)
    {
        Ok(answer) => json!({
            "content": [{"type": "text", "text": answer}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

/// `plan`: shorthand for delegate with the planner role.
fn tool_plan(params: &Value, cfg: &Config) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let task = args
        .get("task")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if task.is_empty() {
        return tool_error("missing required argument 'task'".into());
    }

    let Some((model, header, _mode, max_steps)) = crate::agent::prompt::role("planner") else {
        return tool_error("planner role not found".into());
    };

    let llm_cfg = match LlmConfig::from_config(cfg) {
        Some(c) => crate::agent::prompt::with_model(&c, model),
        None => return tool_error("LLM not configured — run `cotrex setup` first".into()),
    };

    let opts = Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: cfg.compression == "llm",
        footer: false,
    };

    match crate::agent::prompt::fulfill_and_capture(&task, &llm_cfg, Some(header), &opts, max_steps)
    {
        Ok(answer) => json!({
            "content": [{"type": "text", "text": answer}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

/// Execute the `run` tool via the shared core, returning MCP tool-result content.
fn tool_run(params: &Value, cfg: &Config) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
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
    let llm_cfg = if intent.llm || opts.llm_on_failure {
        LlmConfig::from_config(cfg)
    } else {
        None
    };

    // Capture the machine channel; discard the human summary. stdout stays the protocol channel.
    let mut machine: Vec<u8> = Vec::new();
    let mut human = io::sink();
    match orchestrate::run(&intent, &mut machine, &mut human, llm_cfg.as_ref(), &opts) {
        Ok(code) => {
            if cfg.graph_auto {
                crate::graphify::auto_update(&intent.command);
            }
            let text = String::from_utf8_lossy(&machine).to_string();
            let input_bytes = intent.command.len();
            let output_bytes = text.len();
            crate::usage::record(&intent.command, input_bytes, output_bytes, code, "mcp");
            let usage_footer =
                crate::usage::footer(&intent.command, input_bytes, output_bytes, code);
            let tokens_in = input_bytes / 4;
            let tokens_out = output_bytes / 4;
            let mut content = vec![
                json!({"type": "text", "text": text}),
                json!({"type": "text", "text": usage_footer}),
            ];
            if cfg.graph_auto && crate::graphify::current_agent().is_none() {
                content.push(json!({"type": "text", "text": "note: cotrex couldn't detect your agent, so the graphify code-map skill isn't installed. Call the set_agent tool with your platform id (e.g. claude, codex, cursor, gemini) to enable it."}));
            }
            json!({
                "content": content,
                "isError": code != 0,
                "usage": {
                    "command": intent.command,
                    "tokens_in": tokens_in,
                    "tokens_out": tokens_out,
                    "input_bytes": input_bytes,
                    "output_bytes": output_bytes,
                    "exit_code": code,
                    "status": if code == 0 { "ok" } else { "failed" },
                }
            })
        }
        Err(e) => tool_error(e),
    }
}

fn tool_error(msg: String) -> Value {
    json!({"content": [{"type": "text", "text": format!("error: {msg}")}], "isError": true})
}

fn tool_usage() -> Value {
    let usage = crate::usage::summary();
    let json = crate::usage::summary_json();
    json!({
        "content": [
            {"type": "text", "text": usage},
            {"type": "text", "text": format!("\n{json}")},
        ],
        "isError": false,
    })
}

fn tool_graphify_query(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if question.is_empty() {
        return tool_error("missing required argument 'question'".into());
    }
    let dfs = args.get("dfs").and_then(Value::as_bool).unwrap_or(false);
    let budget = args.get("budget").and_then(Value::as_u64).unwrap_or(2000) as u32;
    match crate::graphify::query_graph(question, dfs, budget) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_path(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let node_a = args
        .get("node_a")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let node_b = args
        .get("node_b")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if node_a.is_empty() || node_b.is_empty() {
        return tool_error("missing required arguments 'node_a' and 'node_b'".into());
    }
    match crate::graphify::path_between(node_a, node_b) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_explain(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let node_name = args
        .get("node_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if node_name.is_empty() {
        return tool_error("missing required argument 'node_name'".into());
    }
    match crate::graphify::explain_node(node_name) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_add(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let url = args.get("url").and_then(Value::as_str).unwrap_or("").trim();
    if url.is_empty() {
        return tool_error("missing required argument 'url'".into());
    }
    let author = args
        .get("author")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let contributor = args
        .get("contributor")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    match crate::graphify::add_url(url, author, contributor) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_save_result(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let answer = args
        .get("answer")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if question.is_empty() || answer.is_empty() {
        return tool_error("missing required arguments 'question' and 'answer'".into());
    }
    let result_type = args
        .get("result_type")
        .and_then(Value::as_str)
        .unwrap_or("query")
        .trim();
    let nodes: Vec<&str> = args
        .get("nodes")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    match crate::graphify::save_result(question, answer, result_type, &nodes) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_export(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let result = match format {
        "svg" => crate::graphify::export_svg(),
        "graphml" => crate::graphify::export_graphml(),
        "neo4j" => crate::graphify::export_neo4j(),
        _ => {
            return tool_error(format!(
                "unknown export format: {format}. Supported: svg, graphml, neo4j"
            ))
        }
    };
    match result {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_reports_protocol_and_name() {
        let r = dispatch("initialize", &json!({}), &Config::default())
            .unwrap()
            .unwrap();
        assert_eq!(r["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(r["serverInfo"]["name"], "cotrex");
    }

    #[test]
    fn tools_list_exposes_run() {
        let r = dispatch("tools/list", &json!({}), &Config::default())
            .unwrap()
            .unwrap();
        assert_eq!(r["tools"][0]["name"], "run");
    }

    #[test]
    fn tools_list_exposes_set_agent() {
        let r = dispatch("tools/list", &json!({}), &Config::default())
            .unwrap()
            .unwrap();
        let names: Vec<&str> = r["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"set_agent"));
    }

    #[test]
    fn set_agent_requires_agent() {
        let r = tools_call(
            &json!({"name": "set_agent", "arguments": {}}),
            &Config::default(),
        );
        assert_eq!(r["isError"], true);
    }

    #[test]
    fn initialized_is_a_notification() {
        assert_eq!(
            dispatch("notifications/initialized", &json!({}), &Config::default()),
            Ok(None)
        );
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
