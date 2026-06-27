//! Normalized agent intent + the command -> RTK invocation mapping.
//!
//! Both front-ends (CLI args and stdin JSON) collapse to one `Intent`. Cotrex never runs a raw
//! command: `to_rtk_args` decides which `rtk` subcommand carries it.

use serde::{Deserialize, Serialize};

/// One normalized request. `tool`/`action` exist for forward-compat with non-rtk tools later;
/// v1 only handles `tool = "rtk"`, `action = "run"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Intent {
    #[serde(default = "default_tool")]
    pub tool: String,
    #[serde(default = "default_action")]
    pub action: String,
    /// The shell command line, e.g. "cargo test". Accept "command" or the shorthand "cmd".
    #[serde(alias = "cmd")]
    pub command: String,
    #[serde(default = "default_true")]
    pub stream: bool,
    /// Run output through the LLM compressor for a compact insight. Opt-in.
    #[serde(default)]
    pub llm: bool,
}

fn default_tool() -> String {
    "rtk".into()
}
fn default_action() -> String {
    "run".into()
}
fn default_true() -> bool {
    true
}

impl Intent {
    /// Build an intent from a CLI command string.
    pub fn from_command(command: impl Into<String>) -> Self {
        Intent {
            tool: default_tool(),
            action: default_action(),
            command: command.into(),
            stream: true,
            llm: false,
        }
    }

    /// Parse an intent from stdin JSON.
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| format!("invalid intent JSON: {e}"))
    }

    /// Validate the intent. Cheap trust-boundary check, not a sandbox (RTK owns isolation).
    pub fn validate(&self) -> Result<(), String> {
        if self.tool != "rtk" {
            return Err(format!(
                "unsupported tool '{}' (v1 only routes to rtk)",
                self.tool
            ));
        }
        if self.action != "run" {
            return Err(format!(
                "unsupported action '{}' (v1 only supports 'run')",
                self.action
            ));
        }
        if self.command.trim().is_empty() {
            return Err("empty command".into());
        }
        Ok(())
    }

    /// Map the command to a concrete `rtk` argv. The first token picks a specialized RTK filter
    /// when one exists; everything else falls back to `rtk run -c "<command>"`.
    ///
    /// Returns the args to pass to `rtk` (the program name itself is not included).
    pub fn to_rtk_args(&self) -> Vec<String> {
        let cmd = self.command.trim();

        // Shell operators (&&, ||, ;, |, backticks) require shell interpretation.
        // On Unix, sh -c handles them natively. On Windows, cmd /C doesn't support ;
        // as a command separator, so route through PowerShell instead.
        if has_shell_operators(cmd) {
            if cfg!(windows) {
                return vec![
                    "run".into(),
                    "-c".into(),
                    format!("powershell -NoProfile -Command {cmd}"),
                ];
            }
            return vec!["run".into(), "-c".into(), cmd.into()];
        }

        let first = shell_split(cmd).next().unwrap_or_default();
        // Subcommands rtk has a dedicated filter for. Keep in sync with `rtk --help`.
        // ponytail: a flat allowlist; expand as RTK adds filters, no need for a trait/registry.
        const RTK_NATIVE: &[&str] = &[
            "git",
            "gh",
            "glab",
            "cargo",
            "npm",
            "npx",
            "pnpm",
            "docker",
            "kubectl",
            "ls",
            "tree",
            "find",
            "grep",
            "wc",
            "wget",
            "curl",
            "dotnet",
            "tsc",
            "next",
            "lint",
            "prettier",
            "jest",
            "vitest",
            "prisma",
            "playwright",
            "ruff",
            "pytest",
            "mypy",
            "psql",
            "aws",
        ];
        if RTK_NATIVE.contains(&first.as_str()) {
            // rtk git status   ->  ["git", "status"]
            shell_split(cmd).collect()
        } else {
            // rtk run -c "python foo.py"
            vec!["run".into(), "-c".into(), cmd.into()]
        }
    }
}

/// Split a shell command string into tokens, respecting single and double quotes.
/// Unlike `split_whitespace()`, this keeps quoted strings intact.
fn shell_split(s: &str) -> impl Iterator<Item = String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens.into_iter()
}

/// Check if a command contains shell operators that require shell interpretation.
/// Operators: &&, ||, ;, |, backticks, $()
fn has_shell_operators(cmd: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '&' if !in_single && !in_double => {
                if chars.peek() == Some(&'&') {
                    chars.next();
                    return true;
                }
            }
            '|' if !in_single && !in_double => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                    return true;
                }
                return true; // single pipe
            }
            ';' if !in_single && !in_double => return true,
            '`' if !in_single && !in_double => return true,
            '$' if !in_single && !in_double => {
                if chars.peek() == Some(&'(') {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_and_json_agree() {
        let a = Intent::from_command("git status");
        let b = Intent::from_json(r#"{"tool":"rtk","cmd":"git status"}"#).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn native_command_maps_direct() {
        assert_eq!(
            Intent::from_command("cargo test").to_rtk_args(),
            vec!["cargo", "test"]
        );
    }

    #[test]
    fn unknown_command_falls_back_to_run() {
        assert_eq!(
            Intent::from_command("python foo.py").to_rtk_args(),
            vec!["run", "-c", "python foo.py"]
        );
    }

    #[test]
    fn validation_rejects_junk() {
        assert!(Intent::from_command("").validate().is_err());
        let mut i = Intent::from_command("ls");
        i.tool = "bash".into();
        assert!(i.validate().is_err());
    }

    #[test]
    fn shell_split_respects_double_quotes() {
        let args: Vec<String> =
            shell_split(r#"gh pr create --title "feat: hello world" --body "body text""#)
                .collect();
        assert_eq!(
            args,
            vec![
                "gh",
                "pr",
                "create",
                "--title",
                "feat: hello world",
                "--body",
                "body text"
            ]
        );
    }

    #[test]
    fn shell_split_respects_single_quotes() {
        let args: Vec<String> = shell_split("echo 'hello world' foo").collect();
        assert_eq!(args, vec!["echo", "hello world", "foo"]);
    }

    #[test]
    fn gh_pr_create_maps_direct() {
        let args = Intent::from_command(r#"gh pr create --title "my title""#).to_rtk_args();
        assert_eq!(args, vec!["gh", "pr", "create", "--title", "my title"]);
    }

    #[test]
    fn has_shell_operators_detects_and() {
        assert!(has_shell_operators("git add foo && git commit"));
        assert!(has_shell_operators("cmd1 || cmd2"));
        assert!(has_shell_operators("cmd1; cmd2"));
        assert!(has_shell_operators("cmd1 | cmd2"));
        assert!(has_shell_operators("echo `date`"));
        assert!(has_shell_operators("echo $(date)"));
        assert!(!has_shell_operators("git status"));
        assert!(!has_shell_operators(r#"echo "hello && world""#));
    }

    #[test]
    fn shell_operators_route_to_sh() {
        let args = Intent::from_command("git add foo && git commit -m msg").to_rtk_args();
        if cfg!(windows) {
            assert_eq!(args, vec!["run", "-c", "cmd /c git add foo && git commit -m msg"]);
        } else {
            assert_eq!(args, vec!["run", "-c", "sh -c 'git add foo && git commit -m msg'"]);
        }
    }
}
