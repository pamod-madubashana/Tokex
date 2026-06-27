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

const OPENCODE_USAGE_PLUGIN: &str = r#"/** @jsxImportSource @opentui/solid */
import type { TuiPlugin, TuiPluginModule, TuiSlotPlugin } from "@opencode-ai/plugin/tui"
import { readFileSync, existsSync } from "fs"
import { join } from "path"
import { homedir } from "os"

interface UsageEntry {
  command: string
  tokens_in: number
  tokens_out: number
  exit_code: number
  via: string
}

interface UsageStats {
  total_runs: number
  total_tokens_in: number
  total_tokens_out: number
  total_input_bytes: number
  total_output_bytes: number
  paid_model_cost?: string
  entries: UsageEntry[]
}

const PAID_MODEL_COST_PER_TOKEN = 3.0 / 1_000_000.0

function readUsage(): UsageStats | null {
  const paths = [
    join(homedir(), ".local", "share", "cotrex", "usage.json"),
    join(homedir(), ".config", "cotrex", "usage.json"),
    join(process.cwd(), ".cotrex", "usage.json"),
  ]
  for (const p of paths) {
    try {
      if (existsSync(p)) {
        return JSON.parse(readFileSync(p, "utf-8"))
      }
    } catch {}
  }
  return null
}

function formatNum(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return String(n)
}

function formatCost(cost: number): string {
  if (cost < 0.01) return `$${cost.toFixed(4)}`
  if (cost < 1.0) return `$${cost.toFixed(3)}`
  return `$${cost.toFixed(2)}`
}

const sidebarBlock: TuiSlotPlugin = {
  order: 150,
  slots: {
    sidebar_content(ctx, _value) {
      const usage = readUsage()

      if (!usage || usage.total_runs === 0) {
        return (
          <box
            border
            borderColor={ctx.theme.current.border}
            flexDirection="column"
            gap={1}
            paddingTop={1}
            paddingBottom={1}
            paddingLeft={2}
            paddingRight={2}
          >
            <text fg={ctx.theme.current.primary}>
              <b>Cotrex Usage</b>
            </text>
            <text fg={ctx.theme.current.text dim}>No commands run yet</text>
          </box>
        )
      }

      const recent = usage.entries.slice(-3).reverse()
      const statusColor = ctx.theme.current.primary
      const cost = usage.total_tokens_out * PAID_MODEL_COST_PER_TOKEN
      const savedColor = cost > 0 ? ctx.theme.current.primary : ctx.theme.current.text

      return (
        <box
          border
          borderColor={ctx.theme.current.border}
          flexDirection="column"
          gap={1}
          paddingTop={1}
          paddingBottom={1}
          paddingLeft={2}
          paddingRight={2}
        >
          <text fg={ctx.theme.current.primary}>
            <b>Cotrex Usage</b>
          </text>

          <box flexDirection="column" gap={0}>
            <text fg={ctx.theme.current.text}>
              Runs: {formatNum(usage.total_runs)}
            </text>
            <text fg={statusColor}>
              Tokens: {formatNum(usage.total_tokens_out)} out
            </text>
          </box>

          <box flexDirection="column" gap={0}>
            <text fg={savedColor}>
              Saved: {formatCost(cost)}
            </text>
            <text fg={ctx.theme.current.text dim}>
              vs paid model ($3/1M tokens)
            </text>
          </box>

          {recent.length > 0 && (
            <box flexDirection="column" gap={0}>
              <text fg={ctx.theme.current.text dim}>Recent:</text>
              {recent.map((e: UsageEntry) => (
                <text fg={ctx.theme.current.text}>
                  {" "}{e.command.slice(0, 28)}{e.command.length > 28 ? ".." : ""} [{e.tokens_out}]
                </text>
              ))}
            </box>
          )}
        </box>
      )
    },
  },
}

const tui: TuiPlugin = async (api) => {
  api.slots.register(sidebarBlock)
}

const plugin: TuiPluginModule & { id: string } = {
  id: "cotrex-usage",
  tui,
}

export default plugin
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

    // 3. For opencode: install the TUI sidebar usage plugin.
    if agent_id == "opencode" {
        let opencode_dir = project_dir.join(".opencode");
        let plugins_dir = opencode_dir.join("plugins");
        fs::create_dir_all(&plugins_dir)
            .map_err(|e| format!("failed to create {}: {e}", plugins_dir.display()))?;

        fs::write(plugins_dir.join("cotrex-usage.tsx"), OPENCODE_USAGE_PLUGIN)
            .map_err(|e| format!("failed to write cotrex usage plugin: {e}"))?;

        let tui_config = opencode_dir.join("tui.json");
        if !tui_config.exists() {
            fs::write(
                &tui_config,
                r#"{
  "plugin": [
    ["./plugins/cotrex-usage.tsx", {}]
  ]
}
"#,
            )
            .map_err(|e| format!("failed to write {}: {e}", tui_config.display()))?;
        }
        eprintln!(
            "cotrex: opencode sidebar plugin -> {}",
            plugins_dir.display()
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

        // Add mcp.cotrex if not present
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

            // Write back
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
