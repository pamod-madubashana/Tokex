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

Installed by `tokex install {agent}`. Reinstall with `tokex install {agent}`.
"#,
        agent = agent,
        project_name = project_name
    )
}

fn tokex_skill(_agent: &str, project_name: &str) -> String {
    format!(
        r#"---
name: tokex
description: "Tokex RTK orchestration skills for {project_name}. Run commands, inspect projects, and get normalized output."
---

# Tokex Skills

**IMPORTANT:** You are an AI agent. Always use `tokex -m` (model mode) for clean output.

## Two modes: commands vs prompts

tokex has **commands** and **prompts**. They use different syntax — mixing them up will break.

### Commands (no quotes)
Known CLI commands like `cargo`, `git`, `npm`, `ls`. Pass them **without quotes**:
```bash
tokex -m cargo test
tokex -m git status
tokex -m npm install
tokex -m cargo build --release
```

### Prompts (quoted)
Natural language instructions. Pass them **inside double quotes**:
```bash
tokex -m "show the project tree"
tokex -m "list all rust projects"
tokex -m "explain the architecture"
tokex -m "install requirements and init"
```

## MCP Tools (if available via MCP server)

If tokex is running as an MCP server, these tools are available:

- **`run`**: Execute a shell command through RTK
- **`delegate`**: Delegate a task to a specific role (planner, coder, assistant, etc.)
- **`plan`**: Create an ordered plan for a task (uses planner role)
- **`list_roles`**: List available roles and their capabilities

### Delegation examples
```
delegate(task="analyze the project structure", role="planner")
delegate(task="write a function to parse CSV", role="coder")
plan(task="build a music player app")
```

## Rules

1. Always use `-m` — never bare `tokex` or `tokex run`.
2. **Commands = no quotes. Prompts = quoted.** Never mix.
3. One command at a time. Feed the result back before running the next.
4. Skip vendor/, target/, .git/ — they're noise.

## Installed for: {project_name}
"#,
        project_name = project_name
    )
}

/// Project-local skills go in `.agents/skills/tokex/` — the standard path OpenCode, Amp,
/// and Antigravity use for project-level skills. NOT `.opencode/` (causes BunInstallFailedError)
/// and NOT `.tokex/` (nobody reads it).
fn project_skills_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(".agents").join("skills").join("tokex")
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
        let skill_dir = skills_dir.join("tokex");
        fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("failed to create {}: {e}", skill_dir.display()))?;

        fs::write(
            skill_dir.join("SKILL.md"),
            tokex_skill(&agent_id, &project_name),
        )
        .map_err(|e| format!("failed to write tokex skill: {e}"))?;

        fs::write(
            skill_dir.join("graphify.md"),
            graphify_skill(&agent_id, &project_name),
        )
        .map_err(|e| format!("failed to write graphify skill: {e}"))?;

        eprintln!("tokex: installed skills -> {}", skill_dir.display());
    }

    // 2. Install to project-local .agents/skills/tokex/.
    let project_skills = project_skills_dir(&project_dir);
    fs::create_dir_all(&project_skills)
        .map_err(|e| format!("failed to create {}: {e}", project_skills.display()))?;

    fs::write(
        project_skills.join("SKILL.md"),
        tokex_skill(&agent_id, &project_name),
    )
    .map_err(|e| format!("failed to write tokex skill: {e}"))?;

    fs::write(
        project_skills.join("graphify.md"),
        graphify_skill(&agent_id, &project_name),
    )
    .map_err(|e| format!("failed to write graphify skill: {e}"))?;

    eprintln!("tokex: project skills -> {}", project_skills.display());
    Ok(())
}

pub fn list_installed() -> Result<(), String> {
    let project_dir = current_project_dir().ok_or("not in a project directory")?;

    let skills_dir = project_skills_dir(&project_dir);
    if !skills_dir.exists() {
        eprintln!("No Tokex skills installed in this project.");
        return Ok(());
    }

    eprintln!("Tokex skills in {}:", project_dir.display());
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
