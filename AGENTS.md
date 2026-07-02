# AGENTS.md

## RULE 0: USE COTREX — NO EXCEPTIONS

**Every shell command MUST go through cotrex.** Never call the shell directly.

| Task | Tool |
|------|------|
| Run any command | `cotrex_run` with `command: "..."` |
| Multi-step work | `cotrex_delegate` with `task: "..."` |
| Plan a task | `cotrex_plan` with `task: "..."` |

**Fallback chain**: `cotrex_run` → `cotrex -m <cmd>` → `rtk <cmd>` → raw shell (last resort).

## RULE 1: GRAPHIFY FIRST

Before reading files or exploring the codebase, read `graphify-out/GRAPH_REPORT.md` first.
For deeper questions, run graphify CLI commands (NOT MCP tools):

```bash
cotrex -m graphify query "architecture overview"    # BFS traversal
cotrex -m graphify explain "Intent"                  # node details
cotrex -m graphify path "Intent" "RTK"              # shortest path
```

Reading >3 files manually without trying graphify first wastes tokens.

## Build & Test

```bash
cotrex -m cargo build
cotrex -m cargo test
cotrex -m cargo test -p cotrex
```

Release: `cotrex -m cargo build --release -p rtk` then `cotrex -m cargo build --release -p cotrex`.

## Core Contract

stdout = machine-only (verbatim rtk lines + JSON result footer). stderr = human text. Never mix.

## Commit Rules

- No "claude" in branch/commit. No AI co-author.
- One logical change, one commit, right away.
- Never push to `main`. PR flow: branch → commit → `gh pr create` → CI green → merge → delete branch.
- Concise subject; body explains *why*.

## Module Map

| Path | Purpose |
|---|---|
| `src/main.rs` | Entry point |
| `src/cli.rs` | CLI types |
| `src/dispatch/` | Subcommand dispatch |
| `src/core/intent.rs` | Intent normalization, RTK mapping |
| `src/core/orchestrate.rs` | Spawn rtk, 2-thread mpsc pipeline |
| `src/core/normalize.rs` | Severity classification |
| `src/config/` | Settings, install, update |
| `src/llm/` | LLM compression, MCP server |
| `src/agent/` | Agentic loop, tools, permissions |
| `src/script/` | Script runner |
| `src/graphify/` | Auto-refresh code map |

## Conventions

- Rust 2021, `rustfmt` defaults, warning-clean.
- `Result<T, String>` errors. `unwrap_or`/`unwrap_or_else` preferred.
- `#[cfg(test)] mod tests` at bottom. `assert_eq!`/`assert!`/`.is_err()`.
- No async — stdlib `thread` + `mpsc` only.

For detailed code style, architecture, or gotchas — query graphify or read the relevant source.
