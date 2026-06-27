//! Permission system for the agentic prompt loop.
//!
//! OpenCode uses pattern-based rules with wildcard matching for tool permissions.
//! Cotrex's current `is_risky` uses a substring blocklist which has false positives/negatives.
//! This module provides a proper permission system with pattern matching.

#![allow(dead_code)]

use glob::Pattern;

/// Permission action for a tool or command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Allow without asking.
    Allow,
    /// Ask for confirmation before running.
    Ask,
    /// Deny completely.
    Deny,
}

/// A permission rule matching tool names or command patterns.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Pattern to match (glob syntax for tool names, or substring for commands).
    pub pattern: String,
    /// Action to take when the pattern matches.
    pub action: Action,
}

/// Permission set for an agent.
#[derive(Debug, Clone)]
pub struct Permissions {
    rules: Vec<Rule>,
}

impl Permissions {
    /// Create a default permission set (allow safe commands, ask on risky ones).
    pub fn default() -> Self {
        let rules = vec![
            // Read-only tools: always allow
            Rule {
                pattern: "read".into(),
                action: Action::Allow,
            },
            Rule {
                pattern: "glob".into(),
                action: Action::Allow,
            },
            Rule {
                pattern: "grep".into(),
                action: Action::Allow,
            },
            // Write tools: ask by default
            Rule {
                pattern: "write".into(),
                action: Action::Ask,
            },
            Rule {
                pattern: "edit".into(),
                action: Action::Ask,
            },
            // Shell: ask by default (will be checked against command content)
            Rule {
                pattern: "shell".into(),
                action: Action::Ask,
            },
        ];
        Permissions { rules }
    }

    /// Create permissions from config.
    pub fn from_config(config: &std::collections::HashMap<String, String>) -> Self {
        let mut perms = Self::default();
        for (pattern, action_str) in config {
            let action = match action_str.as_str() {
                "allow" => Action::Allow,
                "ask" => Action::Ask,
                "deny" => Action::Deny,
                _ => continue,
            };
            perms.rules.push(Rule {
                pattern: pattern.clone(),
                action,
            });
        }
        perms
    }

    /// Evaluate whether a tool/command is allowed, needs confirmation, or is denied.
    pub fn evaluate(&self, tool: &str, command: Option<&str>) -> Action {
        // For shell commands, check the command content first (more specific)
        if tool == "shell" {
            if let Some(cmd) = command {
                return self.evaluate_command(cmd);
            }
        }

        // Check tool-specific rules
        for rule in &self.rules {
            if Pattern::new(&rule.pattern)
                .map(|p| p.matches(tool))
                .unwrap_or(false)
            {
                return rule.action;
            }
        }

        // Default: ask
        Action::Ask
    }

    /// Evaluate a shell command against permission rules.
    fn evaluate_command(&self, command: &str) -> Action {
        let cmd_lower = command.to_ascii_lowercase();

        // Check for explicitly denied patterns
        for rule in &self.rules {
            if rule.action == Action::Deny {
                if let Ok(pattern) = Pattern::new(&rule.pattern) {
                    if pattern.matches(&cmd_lower) {
                        return Action::Deny;
                    }
                }
            }
        }

        // Check for explicitly allowed patterns
        for rule in &self.rules {
            if rule.action == Action::Allow {
                if let Ok(pattern) = Pattern::new(&rule.pattern) {
                    if pattern.matches(&cmd_lower) {
                        return Action::Allow;
                    }
                }
            }
        }

        // Check against known risky patterns
        if is_command_risky(&cmd_lower) {
            Action::Ask
        } else {
            Action::Allow
        }
    }
}

/// Check if a command is risky (needs confirmation). More precise than substring matching.
fn is_command_risky(cmd: &str) -> bool {
    // Destructive operations
    if cmd.contains("rm ") || cmd.contains("rmdir") || cmd.contains("del ") {
        return true;
    }
    // Permission/ownership changes
    if cmd.contains("chmod")
        || cmd.contains("chown")
        || cmd.contains("sudo")
        || cmd.contains(" su ")
    {
        return true;
    }
    // Network operations that could exfiltrate data
    if cmd.contains("curl") || cmd.contains("wget") || cmd.contains("invoke-webrequest") {
        return true;
    }
    // Git operations that modify history or push
    if cmd.contains("git push")
        || cmd.contains("git reset")
        || cmd.contains("git clean")
        || cmd.contains("git checkout")
        || cmd.contains("git rebase")
        || cmd.contains("git commit")
    {
        return true;
    }
    // Package management
    if cmd.contains("install")
        || cmd.contains("uninstall")
        || cmd.contains("apt ")
        || cmd.contains("brew ")
    {
        return true;
    }
    // PowerShell dangerous cmdlets
    if cmd.contains("remove-item")
        || cmd.contains("move-item")
        || cmd.contains("set-content")
        || cmd.contains("invoke-expression")
        || cmd.contains("start-process")
    {
        return true;
    }
    // Shell redirections that could overwrite files
    if cmd.contains(">") && !cmd.contains(">>") {
        return true;
    }
    // Read-only commands are safe (allow without confirmation)
    if cmd.starts_with("find ")
        || cmd.starts_with("ls ")
        || cmd.starts_with("grep ")
        || cmd.starts_with("git status")
        || cmd.starts_with("git log")
        || cmd.starts_with("git diff")
        || cmd.starts_with("cat ")
        || cmd.starts_with("head ")
        || cmd.starts_with("tail ")
        || cmd.starts_with("wc ")
        || cmd.starts_with("echo ")
        || cmd.starts_with("pwd")
        || cmd.starts_with("which ")
        || cmd.starts_with("where ")
        || cmd.starts_with("file ")
        || cmd.starts_with("stat ")
        || cmd.starts_with("du ")
        || cmd.starts_with("df ")
    {
        return false;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_permissions_allow_read_tools() {
        let perms = Permissions::default();
        assert_eq!(perms.evaluate("read", None), Action::Allow);
        assert_eq!(perms.evaluate("glob", None), Action::Allow);
        assert_eq!(perms.evaluate("grep", None), Action::Allow);
    }

    #[test]
    fn default_permissions_ask_on_write_tools() {
        let perms = Permissions::default();
        assert_eq!(perms.evaluate("write", None), Action::Ask);
        assert_eq!(perms.evaluate("edit", None), Action::Ask);
    }

    #[test]
    fn risky_commands_need_confirmation() {
        assert!(is_command_risky("rm -rf build"));
        assert!(is_command_risky("echo x > file.txt"));
        assert!(is_command_risky("git push origin main"));
        assert!(is_command_risky("npm install left-pad"));
        assert!(!is_command_risky("find . -name Cargo.toml"));
        assert!(!is_command_risky("git status"));
        assert!(!is_command_risky("ls -la"));
    }
}
