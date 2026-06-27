use std::fs;
use std::path::{Path, PathBuf};

const SUPPORTED_AGENTS: &[(&str, &str)] = &[
    ("opencode", "opencode"),
    ("claude", "claude"),
    ("codex", "codex"),
    ("cursor", "cursor"),
    ("gemini", "gemini"),
    ("windsurf", "windsurf"),
    ("aider", "aider"),
    ("continue", "continue"),
    ("cline", "cline"),
];

fn is_project_dir(dir: &Path) -> bool {
    const MARKERS: &[&str] = &[
        ".git",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "deno.json",
        "composer.json",
        "Gemfile",
    ];
    MARKERS.iter().any(|m| dir.join(m).exists())
}

fn current_project_dir() -> Option<PathBuf> {
    std::env::current_dir().ok().filter(|d| is_project_dir(d))
}

/// Resolve the agent's native skills directory based on its platform.
fn agent_skills_dir(agent: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match agent {
        "opencode" => Some(home.join(".config").join("opencode").join("skills")),
        "claude" => Some(home.join(".claude").join("skills")),
        "codex" => Some(home.join(".codex").join("skills")),
        "cursor" => Some(home.join(".cursor").join("skills")),
        "gemini" => Some(home.join(".gemini").join("skills")),
        "windsurf" => Some(home.join(".windsurf").join("skills")),
        "aider" => Some(home.join(".aider").join("skills")),
        "continue" => Some(home.join(".continue").join("skills")),
        "cline" => Some(home.join(".cline").join("skills")),
        _ => None,
    }
}

fn graphify_skill(agent: &str, project_name: &str) -> String {
    format!(
        r#"---
name: graphify
description: "any input (code, docs, papers, images) -> knowledge graph -> clustered communities -> HTML + JSON + audit report. Use when user asks any question about a codebase, project content, architecture, or file relationships -- especially if graphify-out/ exists."
trigger: /graphify
---

# /graphify

Turn any folder of files into a navigable knowledge graph with community detection, an honest audit trail, and three outputs: interactive HTML, GraphRAG-ready JSON, and a plain-language GRAPH_REPORT.md.

## Usage

```
/graphify                                             # full pipeline on current directory
/graphify <path>                                      # full pipeline on specific path
/graphify <path> --mode deep                          # thorough extraction
/graphify <path> --update                             # incremental re-extraction
```

## Installed for: {project_name}

Installed by `cotrex install {agent}`. Reinstall with `cotrex install {agent}`.
"#,
        agent = agent,
        project_name = project_name
    )
}

// Built plugin: plugins/cotrex-usage/dist/tui.js
// Rebuild with: cd plugins/cotrex-usage && bun install && bun run build
const OPENCODE_USAGE_PLUGIN: &str = r#"// @bun
// src/index.tsx
import { effect as _$effect } from "@opentui/solid";
import { insert as _$insert } from "@opentui/solid";
import { createTextNode as _$createTextNode } from "@opentui/solid";
import { insertNode as _$insertNode } from "@opentui/solid";
import { setProp as _$setProp } from "@opentui/solid";
import { createElement as _$createElement } from "@opentui/solid";
import { readFileSync, existsSync } from "fs";
import { join } from "path";
import { homedir } from "os";
import { createSignal } from "solid-js";
var COLLAPSED_KEY = "cotrex-usage-sidebar.collapsed";
function readUsage() {
  const paths = [join(homedir(), ".local", "share", "cotrex", "usage.json"), join(homedir(), ".config", "cotrex", "usage.json"), join(process.cwd(), ".cotrex", "usage.json")];
  for (const p of paths) {
    try {
      if (existsSync(p)) {
        return JSON.parse(readFileSync(p, "utf-8"));
      }
    } catch {}
  }
  return null;
}
function formatNum(n) {
  if (n >= 1000)
    return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}
var tui = async (api) => {
  const [collapsed, setCollapsed] = createSignal(Boolean(api.kv.get(COLLAPSED_KEY, false)));
  const [usageVersion, setUsageVersion] = createSignal(0);
  const toggleCollapsed = () => {
    const next = !collapsed();
    setCollapsed(next);
    api.kv.set(COLLAPSED_KEY, next);
  };
  api.slots.register({
    order: 150,
    slots: {
      sidebar_content: (_ctx, _props) => {
        usageVersion();
        const usage = readUsage();
        if (!usage || usage.total_runs === 0)
          return null;
        const theme = api.theme.current;
        return (() => {
          var _el$ = _$createElement("box"), _el$2 = _$createElement("text"), _el$4 = _$createElement("text"), _el$5 = _$createTextNode(` runs`), _el$6 = _$createElement("text"), _el$7 = _$createTextNode(` tokens saved`);
          _$insertNode(_el$, _el$2);
          _$insertNode(_el$, _el$4);
          _$insertNode(_el$, _el$6);
          _$setProp(_el$, "flexDirection", "column");
          _$insertNode(_el$2, _$createTextNode(`Cotrex`));
          _$insertNode(_el$4, _el$5);
          _$insert(_el$4, () => formatNum(usage.total_runs), _el$5);
          _$insertNode(_el$6, _el$7);
          _$insert(_el$6, () => formatNum(usage.total_tokens_out), _el$7);
          _$effect((_p$) => {
            var _v$ = {
              fg: theme.text
            }, _v$2 = {
              fg: theme.textMuted
            }, _v$3 = {
              fg: theme.textMuted
            };
            _v$ !== _p$.e && (_p$.e = _$setProp(_el$2, "style", _v$, _p$.e));
            _v$2 !== _p$.t && (_p$.t = _$setProp(_el$4, "style", _v$2, _p$.t));
            _v$3 !== _p$.a && (_p$.a = _$setProp(_el$6, "style", _v$3, _p$.a));
            return _p$;
          }, {
            e: undefined,
            t: undefined,
            a: undefined
          });
          return _el$;
        })();
      }
    }
  });
};
var plugin = {
  id: "cotrex-usage-sidebar",
  tui
};
var src_default = plugin;
export {
  src_default as default
};
"#;

fn cotrex_skill(agent: &str, project_name: &str) -> String {
    let mcp_config = match agent {
        "claude" => {
            r#"Add to `~/.claude/settings.json`:
```json
{
  "mcpServers": {
    "cotrex": { "command": "cotrex", "args": ["mcp"] }
  }
}
```"#
        }
        "codex" | "opencode" => {
            r#"Add to your agent config:
```json
{
  "mcpServers": {
    "cotrex": { "command": "cotrex", "args": ["mcp"] }
  }
}
```"#
        }
        "cursor" => {
            r#"Add to `.cursor/mcp.json`:
```json
{
  "mcpServers": {
    "cotrex": { "command": "cotrex", "args": ["mcp"] }
  }
}
```"#
        }
        "gemini" => {
            r#"Add to your agent config:
```json
{
  "mcpServers": {
    "cotrex": { "command": "cotrex", "args": ["mcp"] }
  }
}
```"#
        }
        _ => {
            r#"Add to your agent's MCP config:
```json
{
  "mcpServers": {
    "cotrex": { "command": "cotrex", "args": ["mcp"] }
  }
}
```"#
        }
    };

    format!(
        r#"---
name: cotrex
description: "Cotrex RTK orchestration skills for {project_name}. Run commands, inspect projects, and get normalized output."
---

# Cotrex Skills

**IMPORTANT:** You are an AI agent. Always use cotrex via MCP (preferred) or CLI fallback.

## 1. MCP (preferred)

Connect to cotrex as an MCP server. This gives you tools: `run`, `delegate`, `plan`, `list_roles`, `set_agent`, `usage`.

**Setup** — {mcp_config}

Once connected, use the `run` tool:
```
run(command="cargo test")
run(command="git status")
run(command="npm install")
```

Every `run` result includes a `usage` JSON object with token counts:
```json
"usage": {{"command": "cargo test", "tokens_in": 12, "tokens_out": 50, "exit_code": 0}}
```

**Show token savings to the user.** After each command, briefly mention the token stats:
- "Ran `cargo test` — 12 tokens in, 50 tokens out"
- Use the `usage` tool to show cumulative totals at any time.

Delegation and planning:
```
delegate(task="analyze the project structure", role="planner")
delegate(task="write a function to parse CSV", role="coder")
plan(task="build a music player app")
```

Identify yourself so graphify installs the right code-map skill:
```
set_agent(agent="{agent}")
```

## 2. CLI fallback (when MCP is not available)

If cotrex is not running as an MCP server, use CLI commands directly.

### Commands (no quotes)
Known CLI commands like `cargo`, `git`, `npm`, `ls`. Pass them **without quotes**:
```bash
cotrex -m cargo test
cotrex -m git status
cotrex -m npm install
cotrex -m cargo build --release
```

### Prompts (quoted)
Natural language instructions. Pass them **inside double quotes**:
```bash
cotrex -m "show the project tree"
cotrex -m "list all rust projects"
cotrex -m "explain the architecture"
```

## Rules

1. **MCP first.** Only use CLI when MCP is not configured.
2. Commands = no quotes. Prompts = quoted. Never mix.
3. One command at a time. Feed the result back before running the next.
4. Skip vendor/, target/, .git/ — they're noise.

## Installed for: {project_name}
"#,
        agent = agent,
        project_name = project_name,
        mcp_config = mcp_config,
    )
}

/// Project-local skills go in `.agents/skills/cotrex/` — the standard path OpenCode, Amp,
/// and Antigravity use for project-level skills. NOT `.opencode/` (causes BunInstallFailedError)
/// and NOT `.cotrex/` (nobody reads it).
fn project_skills_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(".agents").join("skills").join("cotrex")
}

pub fn install_agent(agent: &str) -> Result<(), String> {
    let agent_id = SUPPORTED_AGENTS
        .iter()
        .find(|(name, _)| *name == agent)
        .map(|(_, id)| *id)
        .ok_or_else(|| {
            let names: Vec<&str> = SUPPORTED_AGENTS.iter().map(|(n, _)| *n).collect();
            format!(
                "unsupported agent '{agent}'. Supported: {}",
                names.join(", ")
            )
        })?;

    let project_dir = current_project_dir()
        .ok_or("not in a project directory (no Cargo.toml, package.json, etc.)")?;

    let project_name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());

    // 1. Install to agent's native global skills directory.
    if let Some(skills_dir) = agent_skills_dir(agent_id) {
        let skill_dir = skills_dir.join("cotrex");
        fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("failed to create {}: {e}", skill_dir.display()))?;

        fs::write(
            skill_dir.join("SKILL.md"),
            cotrex_skill(&agent_id, &project_name),
        )
        .map_err(|e| format!("failed to write cotrex skill: {e}"))?;

        fs::write(
            skill_dir.join("graphify.md"),
            graphify_skill(&agent_id, &project_name),
        )
        .map_err(|e| format!("failed to write graphify skill: {e}"))?;

        eprintln!("cotrex: installed skills -> {}", skill_dir.display());
    }

    // 2. Install to project-local .agents/skills/cotrex/.
    let project_skills = project_skills_dir(&project_dir);
    fs::create_dir_all(&project_skills)
        .map_err(|e| format!("failed to create {}: {e}", project_skills.display()))?;

    fs::write(
        project_skills.join("SKILL.md"),
        cotrex_skill(&agent_id, &project_name),
    )
    .map_err(|e| format!("failed to write cotrex skill: {e}"))?;

    fs::write(
        project_skills.join("graphify.md"),
        graphify_skill(&agent_id, &project_name),
    )
    .map_err(|e| format!("failed to write graphify skill: {e}"))?;

    eprintln!("cotrex: project skills -> {}", project_skills.display());

    // 3. For opencode: install TUI sidebar usage plugin (global).
    // TUI plugins must be built with Bun first (cd plugins/cotrex-usage && bun install && bun run build).
    // They are registered in ~/.config/opencode/tui.json (global) and copied to ~/.config/opencode/plugins/.
    if agent_id == "opencode" {
        let home = dirs::home_dir().ok_or("could not find home directory")?;
        let global_opencode = home.join(".config").join("opencode");
        let global_plugins_dir = global_opencode.join("plugins");
        fs::create_dir_all(&global_plugins_dir)
            .map_err(|e| format!("failed to create {}: {e}", global_plugins_dir.display()))?;

        fs::write(global_plugins_dir.join("cotrex-usage.js"), OPENCODE_USAGE_PLUGIN)
            .map_err(|e| format!("failed to write cotrex usage plugin: {e}"))?;

        // TUI plugins go in tui.json (global), not project-local
        let tui_config = global_opencode.join("tui.json");
        let mut tui: serde_json::Value = if tui_config.exists() {
            fs::read_to_string(&tui_config)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({
                "$schema": "https://opencode.ai/tui.json",
                "theme": "opencode"
            })
        };

        let plugins = tui
            .as_object_mut()
            .unwrap()
            .entry("plugin")
            .or_insert_with(|| serde_json::json!([]));

        // Plugin path uses forward slashes for cross-platform compat
        let plugin_path = global_plugins_dir
            .join("cotrex-usage.js")
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(arr) = plugins.as_array_mut() {
            arr.retain(|p| {
                let path = match p {
                    serde_json::Value::String(s) => s.as_str(),
                    serde_json::Value::Array(a) => a.first().and_then(|v| v.as_str()).unwrap_or(""),
                    _ => "",
                };
                !path.contains("cotrex-usage")
            });
            arr.push(serde_json::json!(plugin_path));
        }

        let pretty = serde_json::to_string_pretty(&tui).unwrap_or_default();
        fs::write(&tui_config, format!("{}\n", pretty))
            .map_err(|e| format!("failed to write {}: {e}", tui_config.display()))?;

        eprintln!(
            "cotrex: opencode sidebar plugin -> {}",
            global_plugins_dir.display()
        );

        // 4. Add MCP server config to project opencode.json.
        let project_config = project_dir.join("opencode.json");
        let mut config: serde_json::Value = if project_config.exists() {
            fs::read_to_string(&project_config)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let mcp = config
            .as_object_mut()
            .unwrap()
            .entry("mcp")
            .or_insert_with(|| serde_json::json!({}));

        if mcp.get("cotrex").is_none() {
            mcp.as_object_mut().unwrap().insert(
                "cotrex".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": ["cotrex", "mcp"]
                }),
            );

            let pretty = serde_json::to_string_pretty(&config).unwrap_or_default();
            fs::write(&project_config, format!("{}\n", pretty))
                .map_err(|e| format!("failed to write {}: {e}", project_config.display()))?;

            eprintln!(
                "cotrex: MCP server added to {}",
                project_config.display()
            );
        } else {
            eprintln!("cotrex: MCP server already configured in project");
        }
    }

    // 5. Set up graphify: install package, register skill, build code map.
    // This only runs when `cotrex install agent` is executed inside a project directory.
    match crate::graphify::setup_steps() {
        Ok(steps) => {
            for (label, step) in steps {
                eprintln!("cotrex: {label}...");
                if let Err(e) = step() {
                    eprintln!("cotrex: {e}");
                }
            }
        }
        Err(e) => eprintln!("cotrex: graphify setup skipped: {e}"),
    }

    Ok(())
}

pub fn list_installed() -> Result<(), String> {
    let project_dir = current_project_dir().ok_or("not in a project directory")?;

    let skills_dir = project_skills_dir(&project_dir);
    if !skills_dir.exists() {
        eprintln!("No Cotrex skills installed in this project.");
        return Ok(());
    }

    eprintln!("Cotrex skills in {}:", project_dir.display());
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if let Some(skill_name) = s.strip_suffix(".md") {
                eprintln!("  - {skill_name}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_agents_list() {
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "opencode"));
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "claude"));
    }

    #[test]
    fn unsupported_agent_errors() {
        assert!(install_agent("nonexistent").is_err());
    }

    #[test]
    fn opencode_skills_dir_is_config() {
        let dir = agent_skills_dir("opencode").unwrap();
        assert!(dir.to_string_lossy().contains(".config"));
    }
}
