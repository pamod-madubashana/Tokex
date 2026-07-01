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
    /// Graphify code map operations.
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },
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

#[derive(Subcommand)]
pub enum GraphAction {
    /// Refresh the graphify code map (`graphify update .`).
    Update,
    /// Query the knowledge graph (BFS traversal by default).
    Query {
        /// The question to search for.
        question: String,
        /// Use DFS mode instead of BFS.
        #[arg(long)]
        dfs: bool,
        /// Token budget for output (default: 2000).
        #[arg(long, default_value = "2000")]
        budget: u32,
    },
    /// Find shortest path between two concepts.
    Path {
        /// Source concept.
        node_a: String,
        /// Target concept.
        node_b: String,
    },
    /// Explain a node and its connections.
    Explain {
        /// Node name to explain.
        node_name: String,
    },
    /// Fetch a URL and add it to the corpus.
    Add {
        /// URL to fetch.
        url: String,
        /// Author tag.
        #[arg(long, default_value = "")]
        author: String,
        /// Contributor tag.
        #[arg(long, default_value = "")]
        contributor: String,
    },
    /// Re-cluster existing graph without re-extraction.
    ClusterOnly,
    /// Export graph as SVG.
    Svg,
    /// Export graph as GraphML (for Gephi/yEd).
    Graphml,
    /// Generate cypher.txt for Neo4j import.
    Neo4j,
    /// Push graph directly to a Neo4j instance.
    Neo4jPush {
        /// Neo4j bolt URI (e.g. bolt://localhost:7687).
        uri: String,
        /// Neo4j user (default: neo4j).
        #[arg(long, default_value = "neo4j")]
        user: String,
        /// Neo4j password.
        #[arg(long, default_value = "")]
        password: String,
    },
    /// Watch folder and auto-rebuild on code changes.
    Watch {
        /// Path to watch (default: current directory).
        #[arg(default_value = ".")]
        path: String,
        /// Debounce interval in seconds (default: 3).
        #[arg(long, default_value = "3")]
        debounce: u32,
    },
    /// Save a Q&A result back into the graph.
    SaveResult {
        /// The question.
        #[arg(long)]
        question: String,
        /// The answer.
        #[arg(long)]
        answer: String,
        /// Result type (query, path_query, explain).
        #[arg(long, default_value = "query")]
        result_type: String,
        /// Node labels cited.
        #[arg(long, value_delimiter = ',')]
        nodes: Vec<String>,
    },
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
