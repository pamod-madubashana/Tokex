//! graphify integration: cotrex keeps the code map fresh so agents only **read** it
//! (`graphify-out/GRAPH_REPORT.md`, `graphify-out/wiki/`) and never spend a turn updating it.
//!
//! graphify is a Python tool (`pip install graphifyy`, run via `python -m graphify ...`, AST-only —
//! no token cost). cotrex auto-installs it once and **registers its skill for the agent actually in
//! use** (not just Claude): resolved from config, else env auto-detect, else by asking the user.
//! Everything here is best-effort: it never blocks or fails a cotrex run.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Does this command plausibly change source the map should re-read?
/// ponytail: a skiplist of obvious read-only commands; everything else triggers an update. Not
/// precise change detection — upgrade to a git-diff check if redundant updates ever bite.
pub fn touches_code(command: &str) -> bool {
    let mut t = command.split_whitespace();
    let first = t.next().unwrap_or("");
    let second = t.next().unwrap_or("");
    const READ_ONLY: &[&str] = &[
        "ls", "tree", "cat", "echo", "pwd", "which", "find", "grep", "wc", "head", "tail", "env",
    ];
    if READ_ONLY.contains(&first) {
        return false;
    }
    if matches!(first, "git" | "gh")
        && matches!(
            second,
            "status"
                | "log"
                | "diff"
                | "show"
                | "branch"
                | "remote"
                | "fetch"
                | "blame"
                | "ls-files"
        )
    {
        return false;
    }
    true
}

fn py() -> &'static str {
    if run_quiet("python", &["--version"]) {
        "python"
    } else {
        "python3"
    }
}

fn run_quiet(prog: &str, args: &[&str]) -> bool {
    Command::new(prog)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run inheriting this process's stdio (visible under `cotrex graph`/`cotrex setup`, silent when the
/// bootstrap runs detached with null stdio).
fn run_inherit(prog: &str, args: &[&str]) -> bool {
    Command::new(prog)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run silently (output captured), returning success.
fn run_capture(prog: &str, args: &[&str]) -> bool {
    Command::new(prog)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn data_file(name: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("cotrex").join(name))
}

fn exists(p: Option<PathBuf>) -> bool {
    p.map(|m| m.exists()).unwrap_or(false)
}

fn touch(p: Option<PathBuf>) {
    if let Some(m) = p {
        if let Some(d) = m.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&m, b"ok");
    }
}

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

fn current_project_dir() -> Option<PathBuf> {
    std::env::current_dir().ok().filter(|d| is_project_dir(d))
}

/// Make graphifyy importable, auto-`pip install` once (cached via the package marker).
fn ensure_package(py: &str, verbose: bool) -> bool {
    if exists(data_file(".graphify-ok")) {
        return true;
    }
    let mut importable = run_quiet(py, &["-c", "import graphify"]);
    if !importable {
        if verbose {
            eprintln!("cotrex: installing graphifyy (one-time) …");
        }
        importable = run_quiet(py, &["-m", "pip", "install", "--quiet", "graphifyy"])
            && run_quiet(py, &["-c", "import graphify"]);
    }
    if importable {
        touch(data_file(".graphify-ok"));
        true
    } else {
        false
    }
}

/// graphify platform id: explicit config, else env auto-detect (only Claude Code is reliably
/// identifiable from the environment), else None.
pub fn current_agent() -> Option<String> {
    let a = crate::config::load().agent;
    if !a.trim().is_empty() {
        return Some(a.trim().to_string());
    }
    if std::env::var_os("CLAUDECODE").is_some()
        || std::env::var_os("CLAUDE_CODE_ENTRYPOINT").is_some()
    {
        return Some("claude".into());
    }
    None
}

/// Register the graphify skill for the agent in use (once, via the skill marker). Resolves the
/// platform from config/env; if unknown, asks the user when interactive, otherwise leaves guidance.
fn register_skill(py: &str, verbose: bool, prompt_when_unknown: bool) {
    if exists(data_file(".graphify-skill")) {
        return;
    }
    let platform = match current_agent() {
        Some(p) => p,
        None => {
            if prompt_when_unknown && std::io::stdin().is_terminal() {
                match inquire::Text::new(
                    "Which agent are you using? (graphify platform id like claude, codex, cursor; blank to skip)",
                )
                .prompt()
                {
                    Ok(s) if !s.trim().is_empty() => {
                        let p = s.trim().to_string();
                        let mut cfg = crate::config::load();
                        cfg.agent = p.clone();
                        let _ = crate::config::save(&cfg);
                        p
                    }
                    _ => return,
                }
            } else {
                if verbose {
                    eprintln!("cotrex: couldn't detect your agent — run `cotrex setup` (or `cotrex graph` in a terminal) to register the graphify skill for it.");
                }
                return;
            }
        }
    };
    if verbose {
        eprintln!("cotrex: registering graphify skill for '{platform}' …");
    }
    // graphify's CLI is inconsistent: some platforms use `--platform`, others a subcommand. Claude
    // is the bare default. Try the most likely form, then fall back.
    let ok = if platform == "claude" {
        run_inherit(py, &["-m", "graphify", "install"])
    } else {
        run_inherit(py, &["-m", "graphify", "install", "--platform", &platform])
            || run_inherit(py, &["-m", "graphify", &platform, "install"])
    };
    if ok {
        touch(data_file(".graphify-skill"));
    }
}

/// Best-effort refresh after a code-changing run — never blocks the run. If set up, fire a cheap
/// incremental update in the background; if not, run the one-time bootstrap detached (via
/// `cotrex graph`) so install + skill-register + first build never stall the command.
/// ponytail: no lock — a rare double-bootstrap is idempotent.
pub fn auto_update(command: &str) {
    if !touches_code(command) {
        return;
    }
    if current_project_dir().is_none() {
        return;
    }
    if exists(data_file(".graphify-ok")) {
        let _ = Command::new(py())
            .args(["-m", "graphify", "update", "."])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    } else {
        bootstrap_detached();
    }
}

/// Run the one-time bootstrap (`cotrex graph`) detached, with no stdio — safe to call from MCP mode
/// where stdout is the JSON-RPC channel.
pub fn bootstrap_detached() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe)
            .arg("graph")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

/// Forget that the skill was registered, so the next bootstrap re-registers (e.g. after the agent
/// changes).
pub fn clear_skill_marker() {
    if let Some(m) = data_file(".graphify-skill") {
        let _ = std::fs::remove_file(m);
    }
}

/// `cotrex graph` and the post-`setup` bootstrap: install the package, register the skill for the
/// agent, and refresh the map. Blocking, with visible output.
pub fn update_blocking() -> Result<(), String> {
    update_blocking_with_prompt(true, true)
}

/// Step-based bootstrap for setup: caller supplies spinners per step.
pub fn setup_steps() -> Result<Vec<(&'static str, Box<dyn FnOnce() -> Result<(), String>>)>, String>
{
    let _cwd = current_project_dir().ok_or_else(|| "not in a project directory".to_string())?;
    let py = py();

    let ensure_py = py.to_string();
    let register_py = py.to_string();
    let update_py = py.to_string();

    Ok(vec![
        (
            "installing graphify package",
            Box::new(move || {
                if !ensure_package(&ensure_py, false) {
                    return Err(
                        "graphify unavailable — need Python + pip to install graphifyy".into(),
                    );
                }
                Ok(())
            }),
        ),
        (
            "registering graphify skill",
            Box::new(move || {
                register_skill(&register_py, false, false);
                Ok(())
            }),
        ),
        (
            "building code map",
            Box::new(move || {
                if run_capture(&update_py, &["-m", "graphify", "update", "."]) {
                    Ok(())
                } else {
                    Err("graphify update failed".into())
                }
            }),
        ),
    ])
}

fn update_blocking_with_prompt(prompt_when_unknown: bool, verbose: bool) -> Result<(), String> {
    let cwd = current_project_dir().ok_or_else(|| {
        "not in a project directory; skipping graphify code-map refresh".to_string()
    })?;
    let py = py();
    if !ensure_package(py, verbose) {
        return Err("graphify unavailable — need Python + pip to install graphifyy".into());
    }
    register_skill(py, verbose, prompt_when_unknown);
    if verbose {
        eprintln!("cotrex: refreshing graphify code map in {}", cwd.display());
    }
    if run_inherit(py, &["-m", "graphify", "update", "."]) {
        Ok(())
    } else {
        Err("graphify update failed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_commands_skip_update() {
        assert!(!touches_code("git status"));
        assert!(!touches_code("ls -la"));
        assert!(!touches_code("git log --oneline"));
    }

    #[test]
    fn building_or_vcs_writes_trigger_update() {
        assert!(touches_code("cargo build"));
        assert!(touches_code("git commit -m x"));
        assert!(touches_code("npm install"));
    }

    #[test]
    fn project_markers_gate_graphify_updates() {
        let root = std::env::temp_dir().join(format!("cotrex-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert!(!is_project_dir(&root));

        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert!(is_project_dir(&root));

        let _ = std::fs::remove_dir_all(&root);
    }
}
