//! Tool registry for the agentic prompt loop.
//!
//! OpenCode has a full tool registry with typed tools (shell, read, write, edit, glob, grep).
//! Tokex's agentic loop currently only runs shell commands through RTK. This module adds a minimal
//! tool abstraction so the model can call structured tools directly, improving reliability and
//! reducing token waste from shell command generation.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// A tool that the agentic loop can invoke.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: &'static str, // JSON Schema as string
    pub execute: fn(&ToolContext, &serde_json::Value) -> Result<String, String>,
}

/// Context passed to tool execution.
pub struct ToolContext {
    pub workdir: PathBuf,
}

/// Get the list of built-in tools available to the agentic loop.
pub fn builtins() -> Vec<&'static Tool> {
    vec![&READ_TOOL, &WRITE_TOOL, &EDIT_TOOL, &GLOB_TOOL, &GREP_TOOL]
}

/// Read a file's contents.
static READ_TOOL: Tool = Tool {
    name: "read",
    description: "Read a file's contents. Returns the full text content.",
    parameters: r#"{"type":"object","properties":{"path":{"type":"string","description":"File path relative to workdir"}},"required":["path"]}"#,
    execute: |ctx, args| {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("missing 'path'")?;
        let full = resolve_path(&ctx.workdir, path);
        std::fs::read_to_string(&full).map_err(|e| format!("read {path}: {e}"))
    },
};

/// Write content to a file (creates or overwrites).
static WRITE_TOOL: Tool = Tool {
    name: "write",
    description: "Write content to a file. Creates parent directories if needed.",
    parameters: r#"{"type":"object","properties":{"path":{"type":"string","description":"File path relative to workdir"},"content":{"type":"string","description":"Content to write"}},"required":["path","content"]}"#,
    execute: |ctx, args| {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("missing 'path'")?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or("missing 'content'")?;
        let full = resolve_path(&ctx.workdir, path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        std::fs::write(&full, content).map_err(|e| format!("write {path}: {e}"))?;
        Ok(format!("wrote {} bytes to {path}", content.len()))
    },
};

/// Edit a file by replacing a string.
static EDIT_TOOL: Tool = Tool {
    name: "edit",
    description: "Edit a file by replacing an exact string match with new content.",
    parameters: r#"{"type":"object","properties":{"path":{"type":"string","description":"File path relative to workdir"},"old":{"type":"string","description":"Exact string to find (must match uniquely)"},"new":{"type":"string","description":"Replacement string"}},"required":["path","old","new"]}"#,
    execute: |ctx, args| {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("missing 'path'")?;
        let old = args
            .get("old")
            .and_then(|v| v.as_str())
            .ok_or("missing 'old'")?;
        let new = args
            .get("new")
            .and_then(|v| v.as_str())
            .ok_or("missing 'new'")?;
        let full = resolve_path(&ctx.workdir, path);
        let content = std::fs::read_to_string(&full).map_err(|e| format!("read {path}: {e}"))?;
        let count = content.matches(old).count();
        if count == 0 {
            return Err(format!("'{old}' not found in {path}"));
        }
        if count > 1 {
            return Err(format!(
                "'{old}' matches {} times in {path} — provide more context",
                count
            ));
        }
        let updated = content.replacen(old, new, 1);
        std::fs::write(&full, &updated).map_err(|e| format!("write {path}: {e}"))?;
        Ok(format!("edited {path}"))
    },
};

/// Glob for files matching a pattern.
static GLOB_TOOL: Tool = Tool {
    name: "glob",
    description: "Find files matching a glob pattern (e.g. '**/*.rs', 'src/**/*.ts').",
    parameters: r#"{"type":"object","properties":{"pattern":{"type":"string","description":"Glob pattern relative to workdir"}},"required":["pattern"]}"#,
    execute: |ctx, args| {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("missing 'pattern'")?;
        let full = resolve_path(&ctx.workdir, pattern);
        let mut results = Vec::new();
        if let Ok(entries) = glob::glob(&full.to_string_lossy()) {
            for entry in entries.flatten() {
                if let Ok(rel) = entry.strip_prefix(&ctx.workdir) {
                    results.push(rel.to_string_lossy().to_string());
                } else {
                    results.push(entry.to_string_lossy().to_string());
                }
            }
        }
        if results.is_empty() {
            Ok("no files found".to_string())
        } else {
            results.sort();
            Ok(results.join("\n"))
        }
    },
};

/// Search file contents with a regex pattern.
static GREP_TOOL: Tool = Tool {
    name: "grep",
    description: "Search file contents for a regex pattern. Returns matching lines with file:line.",
    parameters: r#"{"type":"object","properties":{"pattern":{"type":"string","description":"Regex pattern to search for"},"path":{"type":"string","description":"Directory or file to search in (relative to workdir, default: .)"}},"required":["pattern"]}"#,
    execute: |ctx, args| {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("missing 'pattern'")?;
        let search_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let full = resolve_path(&ctx.workdir, search_path);
        let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
        let mut results = Vec::new();
        let walk = if full.is_dir() {
            walk_dir(&full, &mut results, &re, &ctx.workdir)
        } else {
            search_file(&full, &re, &ctx.workdir, &mut results)
        };
        if let Err(e) = walk {
            return Err(format!("grep error: {e}"));
        }
        if results.is_empty() {
            Ok("no matches found".to_string())
        } else {
            results.sort();
            Ok(results.join("\n"))
        }
    },
};

fn resolve_path(workdir: &Path, relative: &str) -> PathBuf {
    let p = Path::new(relative);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        workdir.join(p)
    }
}

fn walk_dir(
    dir: &Path,
    results: &mut Vec<String>,
    re: &regex::Regex,
    workdir: &Path,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "target" || name == "node_modules" || name == ".git" || name == "vendor" {
                continue;
            }
            walk_dir(&path, results, re, workdir)?;
        } else if let Ok(content) = std::fs::read_to_string(&path) {
            search_file_content(&path, &content, re, workdir, results);
        }
    }
    Ok(())
}

fn search_file(
    file: &Path,
    re: &regex::Regex,
    workdir: &Path,
    results: &mut Vec<String>,
) -> std::io::Result<()> {
    let content = std::fs::read_to_string(file)?;
    search_file_content(file, &content, re, workdir, results);
    Ok(())
}

fn search_file_content(
    file: &Path,
    content: &str,
    re: &regex::Regex,
    workdir: &Path,
    results: &mut Vec<String>,
) {
    let rel = file.strip_prefix(workdir).unwrap_or(file);
    for (i, line) in content.lines().enumerate() {
        if re.is_match(line) {
            results.push(format!("{}:{}:{}", rel.display(), i + 1, line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tool_requires_path() {
        let ctx = ToolContext {
            workdir: std::env::temp_dir(),
        };
        let args = serde_json::json!({});
        assert!((READ_TOOL.execute)(&ctx, &args).is_err());
    }

    #[test]
    fn write_tool_creates_file() {
        let dir = std::env::temp_dir().join("tokex-tool-test");
        let _ = std::fs::create_dir_all(&dir);
        let ctx = ToolContext {
            workdir: dir.clone(),
        };
        let args = serde_json::json!({"path": "test.txt", "content": "hello"});
        let result = (WRITE_TOOL.execute)(&ctx, &args);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(dir.join("test.txt"));
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn edit_tool_finds_and_replaces() {
        let dir = std::env::temp_dir().join("tokex-edit-test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("test.rs"), "fn main() {}").unwrap();
        let ctx = ToolContext {
            workdir: dir.clone(),
        };
        let args = serde_json::json!({"path": "test.rs", "old": "fn main() {}", "new": "fn main() { println!(\"hi\"); }"});
        let result = (EDIT_TOOL.execute)(&ctx, &args);
        assert!(result.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
