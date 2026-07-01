//! graphify integration: cotrex keeps the code map fresh so agents only **read** it
//! (`graphify-out/GRAPH_REPORT.md`, `graphify-out/wiki/`) and never spend a turn updating it.
//!
//! graphify can run as either:
//! 1. An embedded standalone binary (preferred, built with PyInstaller)
//! 2. A Python package (`pip install graphifyy`, run via `python -m graphify ...`)
//!
//! cotrex auto-installs it once and **registers its skill for the agent actually in use**
//! (not just Claude): resolved from config, else env auto-detect, else by asking the user.
//! Everything here is best-effort: it never blocks or fails a cotrex run.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::embedded_graphify;

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

/// Resolve the graphify binary. Order: embedded → PATH → Python module fallback.
/// Returns (binary_path, is_standalone) where is_standalone=true means it's a standalone
/// executable (not `python -m graphify`).
fn graphify_bin() -> (PathBuf, bool) {
    // 1. Try embedded graphify binary
    if let Some(path) = embedded_graphify::extract_graphify() {
        if path.is_file() {
            return (path, true);
        }
    }

    // 2. Try graphify on PATH
    let graphify_name = if cfg!(windows) {
        "graphify.exe"
    } else {
        "graphify"
    };
    if run_quiet(graphify_name, &["--version"]) {
        return (PathBuf::from(graphify_name), true);
    }

    // 3. Fall back to Python module
    let py = py();
    (PathBuf::from(py), false)
}

/// Run graphify command with the appropriate binary. Returns true if successful.
fn run_graphify(args: &[&str], inherit_stdio: bool) -> bool {
    let (bin, is_standalone) = graphify_bin();
    let mut cmd_args = Vec::new();

    if is_standalone {
        cmd_args.extend_from_slice(args);
    } else {
        cmd_args.push("-m");
        cmd_args.push("graphify");
        cmd_args.extend_from_slice(args);
    }

    let mut cmd = Command::new(&bin);
    cmd.args(&cmd_args);

    if inherit_stdio {
        cmd.stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    cmd.status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run graphify command and capture stdout. Returns (success, stdout_text).
fn run_graphify_capture(args: &[&str]) -> (bool, String) {
    let (bin, is_standalone) = graphify_bin();
    let mut cmd_args = Vec::new();

    if is_standalone {
        cmd_args.extend_from_slice(args);
    } else {
        cmd_args.push("-m");
        cmd_args.push("graphify");
        cmd_args.extend_from_slice(args);
    }

    let output = Command::new(&bin)
        .args(&cmd_args)
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            (o.status.success(), stdout)
        }
        Err(_) => (false, String::new()),
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

/// Make graphify available, auto-`pip install` once (cached via the package marker).
/// Returns true if graphify is available (either embedded or as a Python package).
fn ensure_package(py: &str, verbose: bool) -> bool {
    // Check if embedded binary is available
    if let Some(path) = embedded_graphify::extract_graphify() {
        if path.is_file() {
            return true;
        }
    }

    // Check if graphify is on PATH
    let graphify_name = if cfg!(windows) {
        "graphify.exe"
    } else {
        "graphify"
    };
    if run_quiet(graphify_name, &["--version"]) {
        return true;
    }

    // Check if Python package is installed
    if exists(data_file(".graphify-ok")) {
        return true;
    }
    let mut importable = run_quiet(py, &["-c", "import graphify"]);
    if !importable {
        // Try installing from vendored source first, fall back to PyPI
        let vendored_path = find_vendored_graphify();
        if let Some(path) = vendored_path {
            if verbose {
                eprintln!(
                    "cotrex: installing graphifyy from vendored source ({}) …",
                    path.display()
                );
            }
            importable = run_quiet(py, &["-m", "pip", "install", "--quiet", &path.to_string_lossy()])
                && run_quiet(py, &["-c", "import graphify"]);
        }
        // Fall back to PyPI if vendored source not available or failed
        if !importable {
            if verbose {
                eprintln!("cotrex: installing graphifyy from PyPI (one-time) …");
            }
            importable = run_quiet(py, &["-m", "pip", "install", "--quiet", "graphifyy"])
                && run_quiet(py, &["-c", "import graphify"]);
        }
    }
    if importable {
        touch(data_file(".graphify-ok"));
        true
    } else {
        false
    }
}

/// Find the vendored graphify source directory.
/// Looks for `vendor/graphify/` relative to the current directory or parent directories.
fn find_vendored_graphify() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("vendor").join("graphify");
        if candidate.is_dir() && candidate.join("pyproject.toml").exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
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
fn register_skill(_py: &str, verbose: bool, prompt_when_unknown: bool) {
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
        run_graphify(&["install"], true)
    } else {
        run_graphify(&["install", "--platform", &platform], true)
            || run_graphify(&[&platform, "install"], true)
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
    if ensure_package(&py(), false) {
        // Run graphify update in background (fire-and-forget)
        let _ = run_graphify(&["update", "."], false);
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
                if run_graphify(&["update", "."], true) {
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
    if !ensure_package(&py, verbose) {
        return Err("graphify unavailable — need Python + pip to install graphifyy".into());
    }
    register_skill(&py, verbose, prompt_when_unknown);
    if verbose {
        eprintln!("cotrex: refreshing graphify code map in {}", cwd.display());
    }
    if run_graphify(&["update", "."], true) {
        Ok(())
    } else {
        Err("graphify update failed".into())
    }
}

/// `graphify query` — BFS/DFS traversal of the knowledge graph.
pub fn query_graph(question: &str, dfs: bool, budget: u32) -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let mode = if dfs { "dfs" } else { "bfs" };
    let budget_str = budget.to_string();
    let mut args = vec!["query", question, "--mode", mode, "--budget", &budget_str];
    // graphify query uses positional question, but --dfs flag for DFS mode
    if dfs {
        args = vec!["query", question, "--dfs", "--budget", &budget_str];
    }
    let (ok, output) = run_graphify_capture(&args);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify query failed".into())
    } else {
        Err(output)
    }
}

/// `graphify path` — shortest path between two concepts.
pub fn path_between(node_a: &str, node_b: &str) -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let (ok, output) = run_graphify_capture(&["path", node_a, node_b]);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify path failed".into())
    } else {
        Err(output)
    }
}

/// `graphify explain` — plain-language explanation of a node.
pub fn explain_node(node_name: &str) -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let (ok, output) = run_graphify_capture(&["explain", node_name]);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify explain failed".into())
    } else {
        Err(output)
    }
}

/// `graphify add` — fetch a URL and add it to the corpus.
pub fn add_url(url: &str, author: &str, contributor: &str) -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let mut args = vec!["add", url];
    if !author.is_empty() {
        args.push("--author");
        args.push(author);
    }
    if !contributor.is_empty() {
        args.push("--contributor");
        args.push(contributor);
    }
    let (ok, output) = run_graphify_capture(&args);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify add failed".into())
    } else {
        Err(output)
    }
}

/// `graphify --cluster-only` — re-cluster existing graph without re-extraction.
pub fn cluster_only() -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let (ok, output) = run_graphify_capture(&["--cluster-only", "."]);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify cluster-only failed".into())
    } else {
        Err(output)
    }
}

/// `graphify --svg` — export graph as SVG.
pub fn export_svg() -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let (ok, output) = run_graphify_capture(&["--svg", "."]);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify svg export failed".into())
    } else {
        Err(output)
    }
}

/// `graphify --graphml` — export graph as GraphML.
pub fn export_graphml() -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let (ok, output) = run_graphify_capture(&["--graphml", "."]);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify graphml export failed".into())
    } else {
        Err(output)
    }
}

/// `graphify --neo4j` — generate cypher.txt for Neo4j import.
pub fn export_neo4j() -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let (ok, output) = run_graphify_capture(&["--neo4j", "."]);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify neo4j export failed".into())
    } else {
        Err(output)
    }
}

/// `graphify --neo4j-push` — push graph directly to a Neo4j instance.
pub fn push_neo4j(uri: &str, user: &str, password: &str) -> Result<String, String> {
    if current_project_dir().is_none() {
        return Err("not in a project directory".into());
    }
    let push_arg = format!("--neo4j-push={uri}");
    let user_arg = format!("--neo4j-user={user}");
    let pass_arg = format!("--neo4j-password={password}");
    let (ok, output) = run_graphify_capture(&[".", &push_arg, &user_arg, &pass_arg]);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify neo4j-push failed".into())
    } else {
        Err(output)
    }
}

/// `graphify save-result` — save a Q&A back into the graph for future queries.
pub fn save_result(
    question: &str,
    answer: &str,
    result_type: &str,
    nodes: &[&str],
) -> Result<String, String> {
    let mut args = vec![
        "save-result",
        "--question", question,
        "--answer", answer,
        "--type", result_type,
    ];
    if !nodes.is_empty() {
        args.push("--nodes");
        args.extend_from_slice(nodes);
    }
    let (ok, output) = run_graphify_capture(&args);
    if ok {
        Ok(output)
    } else if output.is_empty() {
        Err("graphify save-result failed".into())
    } else {
        Err(output)
    }
}

/// `graphify --watch` — watch folder and auto-rebuild on code changes. Blocks until interrupted.
pub fn watch(path: &str, debounce: u32) -> Result<(), String> {
    let debounce_str = debounce.to_string();
    let args = vec!["--watch", path, "--debounce", &debounce_str];
    // watch needs inherit_stdio=true to show live output
    if run_graphify(&args, true) {
        Ok(())
    } else {
        Err("graphify watch failed".into())
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
