//! Install Tokex skills into the current project directory for a specific agent.
//!
//! `tokex install <agent>` creates a `.tokex/` directory in the project root and installs
//! skill files tailored for the specified agent (opencode, claude, codex, cursor, etc.).
//! This allows per-project skill configuration instead of relying on global registration.

use std::fs;
use std::path::{Path, PathBuf};

/// Supported agents and their skill file formats.
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

/// Check if the current directory is a project directory (has common project markers).
fn is_project_dir(dir: &Path) -> bool {
    const PROJECT_MARKERS: &[&str] = &[
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
    PROJECT_MARKERS.iter().any(|m| dir.join(m).exists())
}

/// Get the current project directory.
fn current_project_dir() -> Option<PathBuf> {
    std::env::current_dir().ok().filter(|d| is_project_dir(d))
}

/// Create the .tokex directory structure in the project.
fn create_tokex_dir(project_dir: &Path) -> Result<PathBuf, String> {
    let tokex_dir = project_dir.join(".tokex");
    fs::create_dir_all(&tokex_dir)
        .map_err(|e| format!("failed to create .tokex directory: {e}"))?;
    Ok(tokex_dir)
}

/// Generate the skill file content for the specified agent.
fn generate_skill_content(agent: &str, project_dir: &Path) -> String {
    let project_name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    match agent {
        "opencode" => format!(
            r#"---
name: tokex-{agent}
description: "Tokex RTK orchestration skills for {project_name}"
---

# Tokex Skills for OpenCode

This directory contains Tokex skills configured for the {project_name} project.

## Available Skills

### /graphify
Turn any folder of files into a navigable knowledge graph with community detection.

### /tokex-run
Run a command through RTK and get normalized output.

### /tokex-tree
Show the project structure as a tree.

## Usage

These skills are available in this project directory. The agent will automatically
detect and use them when working in this project.

## Configuration

Skills are installed in `.tokex/` directory. To update or modify skills:
- Edit the skill files directly
- Run `tokex install {agent}` to reinstall
"#,
            agent = agent,
            project_name = project_name
        ),
        "claude" => format!(
            r#"---
name: tokex-{agent}
description: "Tokex RTK orchestration skills for {project_name}"
---

# Tokex Skills for Claude Code

This directory contains Tokex skills configured for the {project_name} project.

## Available Skills

### /graphify
Turn any folder of files into a navigable knowledge graph with community detection.

### /tokex-run
Run a command through RTK and get normalized output.

### /tokex-tree
Show the project structure as a tree.

## Usage

These skills are available in this project directory. The agent will automatically
detect and use them when working in this project.

## Configuration

Skills are installed in `.tokex/` directory. To update or modify skills:
- Edit the skill files directly
- Run `tokex install {agent}` to reinstall
"#,
            agent = agent,
            project_name = project_name
        ),
        _ => format!(
            r#"---
name: tokex-{agent}
description: "Tokex RTK orchestration skills for {project_name}"
---

# Tokex Skills for {agent}

This directory contains Tokex skills configured for the {project_name} project.

## Available Skills

### /graphify
Turn any folder of files into a navigable knowledge graph with community detection.

### /tokex-run
Run a command through RTK and get normalized output.

### /tokex-tree
Show the project structure as a tree.

## Usage

These skills are available in this project directory. The agent will automatically
detect and use them when working in this project.

## Configuration

Skills are installed in `.tokex/` directory. To update or modify skills:
- Edit the skill files directly
- Run `tokex install {agent}` to reinstall
"#,
            agent = agent,
            project_name = project_name
        ),
    }
}

/// Generate the graphify skill content for the agent.
fn generate_graphify_skill(agent: &str, project_dir: &Path) -> String {
    let project_name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    format!(
        r#"---
name: graphify
description: "any input (code, docs, papers, images) → knowledge graph → clustered communities → HTML + JSON + audit report. Use when user asks any question about a codebase, project content, architecture, or file relationships — especially if graphify-out/ exists. Provides persistent graph with god nodes, community detection, and BFS/DFS query tools."
trigger: /graphify
---

# /graphify

Turn any folder of files into a navigable knowledge graph with community detection, an honest audit trail, and three outputs: interactive HTML, GraphRAG-ready JSON, and a plain-language GRAPH_REPORT.md.

## Usage

```
/graphify                                             # full pipeline on current directory
/graphify <path>                                      # full pipeline on specific path
/graphify <path> --mode deep                          # thorough extraction, richer INFERRED edges
/graphify <path> --update                             # incremental - re-extract only new/changed files
```

## What graphify is for

graphify is built around Andrej Karpathy's /raw folder workflow: drop anything into a folder - papers, tweets, screenshots, code, notes - and get a structured knowledge graph that shows you what you didn't know was connected.

Three things it does that your AI assistant alone cannot:
1. **Persistent graph** - relationships are stored in `graphify-out/graph.json` and survive across sessions.
2. **Honest audit trail** - every edge is tagged EXTRACTED, INFERRED, or AMBIGUOUS.
3. **Cross-document surprise** - community detection finds connections between concepts in different files.

## Project: {project_name}

This skill is installed for the {project_name} project. The graphify tool is configured
to work with this project's structure and codebase.

## Installation

This skill was installed by `tokex install {agent}`. To reinstall or update:
```bash
tokex install {agent}
```
"#,
        agent = agent,
        project_name = project_name
    )
}

/// Install skills for the specified agent into the current project.
pub fn install_agent(agent: &str) -> Result<(), String> {
    // Validate agent name
    let valid_agent = SUPPORTED_AGENTS
        .iter()
        .find(|(name, _)| *name == agent)
        .map(|(_, id)| *id);

    let agent_id = valid_agent.ok_or_else(|| {
        let supported: Vec<&str> = SUPPORTED_AGENTS.iter().map(|(n, _)| *n).collect();
        format!(
            "unsupported agent '{agent}'. Supported agents: {}",
            supported.join(", ")
        )
    })?;

    // Find project directory
    let project_dir = current_project_dir()
        .ok_or("not in a project directory (no Cargo.toml, package.json, etc. found)")?;

    // Create .tokex directory
    let tokex_dir = create_tokex_dir(&project_dir)?;

    // Create skills directory
    let skills_dir = tokex_dir.join("skills");
    fs::create_dir_all(&skills_dir)
        .map_err(|e| format!("failed to create skills directory: {e}"))?;

    // Generate and write main skill file
    let skill_content = generate_skill_content(agent_id, &project_dir);
    let skill_file = skills_dir.join(format!("tokex-{agent_id}.md"));
    fs::write(&skill_file, skill_content)
        .map_err(|e| format!("failed to write skill file: {e}"))?;

    // Generate and write graphify skill
    let graphify_content = generate_graphify_skill(agent_id, &project_dir);
    let graphify_file = skills_dir.join("graphify.md");
    fs::write(&graphify_file, graphify_content)
        .map_err(|e| format!("failed to write graphify skill: {e}"))?;

    // Create a simple config file
    let config_content = format!(
        r#"# Tokex configuration for {project_name}
# Installed by: tokex install {agent}

[skills]
agent = "{agent}"
installed = true

[graphify]
auto_update = true
"#,
        project_name = project_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string()),
        agent = agent_id
    );

    let config_file = tokex_dir.join("config.toml");
    fs::write(&config_file, config_content)
        .map_err(|e| format!("failed to write config file: {e}"))?;

    println!(
        "Installed Tokex skills for '{agent_id}' in {}",
        tokex_dir.display()
    );
    println!("Skills directory: {}", skills_dir.display());
    println!("Configuration: {}", config_file.display());

    Ok(())
}

/// List installed agents in the current project.
pub fn list_installed() -> Result<(), String> {
    let project_dir = current_project_dir()
        .ok_or("not in a project directory (no Cargo.toml, package.json, etc. found)")?;

    let tokex_dir = project_dir.join(".tokex");
    if !tokex_dir.exists() {
        println!("No Tokex skills installed in this project.");
        return Ok(());
    }

    let skills_dir = tokex_dir.join("skills");
    if !skills_dir.exists() {
        println!("No Tokex skills installed in this project.");
        return Ok(());
    }

    println!("Tokex skills installed in {}:", project_dir.display());
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".md") {
                let skill_name = name_str.strip_suffix(".md").unwrap_or(&name_str);
                println!("  - {skill_name}");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_agents_contains_known_agents() {
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "opencode"));
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "claude"));
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "codex"));
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "cursor"));
    }

    #[test]
    fn unsupported_agent_returns_error() {
        let result = install_agent("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported agent"));
    }

    #[test]
    fn skill_content_contains_agent_name() {
        let content = generate_skill_content("opencode", Path::new("/tmp/test-project"));
        assert!(content.contains("opencode"));
        assert!(content.contains("test-project"));
    }

    #[test]
    fn graphify_skill_content_contains_agent_name() {
        let content = generate_graphify_skill("claude", Path::new("/tmp/my-app"));
        assert!(content.contains("claude"));
        assert!(content.contains("my-app"));
    }
}
