//! Stack planner: use the configured LLM when available, with a deterministic keyword fallback.

use serde::{Deserialize, Serialize};

use crate::llm::LlmConfig;

#[derive(Debug, Serialize)]
pub struct StackPlan {
    pub task: String,
    pub stack: String,
    pub reason: String,
    pub init_commands: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LlmStackPlan {
    stack: String,
    reason: String,
    #[serde(default)]
    init_commands: Vec<String>,
}

struct Rule {
    keywords: &'static [&'static str],
    stack: &'static str,
    reason: &'static str,
    init: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        keywords: &[
            "website",
            "site",
            "web app",
            "webapp",
            "dashboard",
            "landing",
            "portfolio",
            "e-commerce",
            "ecommerce",
            "weather app",
        ],
        stack: "next.js",
        reason: "web UI with SSR; largest ecosystem and fastest to ship",
        init: &["npx create-next-app@latest"],
    },
    Rule {
        keywords: &["desktop", "music player", "player", "native app", "tray"],
        stack: "tauri",
        reason: "cross-platform desktop with Rust core + web UI; small binaries",
        init: &["npm create tauri-app@latest"],
    },
    Rule {
        keywords: &["mobile", "ios", "android", "cross-platform app"],
        stack: "flutter",
        reason: "single codebase for iOS + Android with native performance",
        init: &["flutter create app"],
    },
    Rule {
        keywords: &["cli", "tool", "daemon", "parser", "fast", "systems"],
        stack: "rust",
        reason: "deterministic CLI/systems work; single static binary",
        init: &["cargo init"],
    },
    Rule {
        keywords: &["script", "data", "ml", "scrape", "api", "automation"],
        stack: "python",
        reason: "quickest path for scripting/data/automation; rich libraries",
        init: &["python -m venv .venv"],
    },
];

const SYSTEM: &str = "You recommend practical application tech stacks for developers. Output ONLY \
minified JSON with keys: stack (short lowercase stack name), reason (one concise sentence), \
init_commands (array of 1-4 shell commands). No markdown, no prose.";

pub fn plan(task: &str, llm: Option<&LlmConfig>) -> Result<StackPlan, String> {
    match llm {
        Some(cfg) => llm_plan(task, cfg),
        None => Ok(heuristic_plan(task)),
    }
}

pub fn heuristic_plan(task: &str) -> StackPlan {
    let t = task.to_ascii_lowercase();
    for r in RULES {
        if r.keywords.iter().any(|k| t.contains(k)) {
            return StackPlan {
                task: task.to_string(),
                stack: r.stack.to_string(),
                reason: r.reason.to_string(),
                init_commands: r.init.iter().map(|s| s.to_string()).collect(),
            };
        }
    }
    // Default: when nothing matches, Python is the lowest-friction starting point.
    StackPlan {
        task: task.to_string(),
        stack: "python".to_string(),
        reason: "no strong signal in the task; Python is the lowest-friction default".to_string(),
        init_commands: vec!["python -m venv .venv".to_string()],
    }
}

fn llm_plan(task: &str, cfg: &LlmConfig) -> Result<StackPlan, String> {
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0.1,
        "messages": [
            {"role": "system", "content": SYSTEM},
            {"role": "user", "content": format!("Task: {task}")},
        ],
    });
    let resp = ureq::post(&cfg.url)
        .set("Authorization", &format!("Bearer {}", cfg.key))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;
    let v: serde_json::Value = resp.into_json().map_err(|e| format!("bad response: {e}"))?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("response missing message content")?;
    let raw = parse_llm_plan(content)?;
    if raw.stack.trim().is_empty() {
        return Err("response missing stack".into());
    }
    if raw.reason.trim().is_empty() {
        return Err("response missing reason".into());
    }
    Ok(StackPlan {
        task: task.to_string(),
        stack: raw.stack.trim().to_string(),
        reason: raw.reason.trim().to_string(),
        init_commands: raw
            .init_commands
            .into_iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .take(4)
            .collect(),
    })
}

fn parse_llm_plan(content: &str) -> Result<LlmStackPlan, String> {
    let start = content.find('{').ok_or("no JSON object in llm output")?;
    let end = content.rfind('}').ok_or("no JSON object in llm output")?;
    serde_json::from_str(&content[start..=end]).map_err(|e| format!("JSON parse: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_keyword_picks_stack() {
        assert_eq!(heuristic_plan("build a music player app").stack, "tauri");
        assert_eq!(heuristic_plan("portfolio site").stack, "next.js");
        assert_eq!(heuristic_plan("e-commerce site").stack, "next.js");
        assert_eq!(heuristic_plan("a fast CLI tool").stack, "rust");
        assert_eq!(heuristic_plan("scrape some data").stack, "python");
    }

    #[test]
    fn unknown_defaults_to_python() {
        assert_eq!(heuristic_plan("zzzqqq").stack, "python");
    }

    #[test]
    fn parses_fenced_llm_plan() {
        let p = parse_llm_plan(
            "```json\n{\"stack\":\"next.js\",\"reason\":\"good web default\",\"init_commands\":[\"npx create-next-app@latest\"]}\n```",
        )
        .unwrap();
        assert_eq!(p.stack, "next.js");
        assert_eq!(p.init_commands, vec!["npx create-next-app@latest"]);
    }
}
