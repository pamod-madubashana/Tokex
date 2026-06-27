//! CLI type definitions.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cotrex",
    version,
    about = "Deterministic RTK orchestration layer for AI agents",
    after_help = "Stdin mode: pipe a JSON intent instead of a subcommand, e.g.\n  echo '{\"tool\":\"rtk\",\"cmd\":\"git status\"}' | cotrex"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Run a command through RTK and stream normalized events.
    Run {
        /// Force the LLM insight on for this run (overrides the configured compression mode).
        #[arg(long)]
        llm: bool,
        /// The command line, e.g. "cargo test".
        command: String,
    },
    /// Run a script from Scripts/ through rtk and verify with git diff.
    Script {
        /// Path to the script (e.g. Scripts/rename.sh). Omit to create Scripts/ and print the flow.
        file: Option<String>,
    },
    /// Interactive setup: choose provider, enter API key, pick modes.
    Setup,
    /// Run as an MCP server over stdio (for agents that call tools natively).
    Mcp,
    /// Pre-fetch the pinned rtk release for this OS (also happens automatically on first run).
    InstallRtk,
    /// Refresh the graphify code map now (`graphify update .`).
    Graph,
    /// Install Cotrex skills into the current project for a specific agent.
    Install {
        /// Agent name (opencode, claude, codex, cursor, gemini, windsurf, aider, continue, cline).
        agent: Option<String>,
    },
    /// Check for a newer release and install it if available.
    Update,
    /// Show token usage statistics.
    Usage,
}

pub const SUBCOMMANDS: &[&str] = &[
    "run",
    "script",
    "setup",
    "mcp",
    "install-rtk",
    "graph",
    "install",
    "update",
    "usage",
    "help",
];
