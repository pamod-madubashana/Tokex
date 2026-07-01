# AGENTS.md

## RULE 0: USE COTREX — NO EXCEPTIONS

**Every shell command MUST go through cotrex.** Not sometimes. Not when convenient. Every time.

If you need to run `cargo build`, `git status`, `cargo test`, `git diff`, `gh pr create`, or ANY
other command — use a cotrex MCP tool or the cotrex CLI. Never call the shell directly.

### MCP tools (preferred — available in this session)

| Task | Tool | Example |
|------|------|---------|
| Run any command | `cotrex_run` | `cotrex_run` with `command: "cargo test"` |
| Multi-step work | `cotrex_delegate` | `cotrex_delegate` with `task: "build and test"` |
| Plan a task | `cotrex_plan` | `cotrex_plan` with `task: "add feature X"` |

### CLI fallback (only if MCP tools are unavailable)

```bash
cotrex -m cargo build
cotrex -m cargo test
cotrex -m cargo test -p cotrex
cotrex -m git status
cotrex -m git diff
```

**Fallback chain**: `cotrex_run` → `cotrex -m <cmd>` → `rtk <cmd>` → raw shell (last resort).
Raw shell loses normalization. Only use it when cotrex is completely unavailable.

## RULE 1: GRAPHIFY FIRST

Before reading files or exploring the codebase, query the knowledge graph:
- `cotrex_graphify_query` — search for concepts, relationships, architecture
- `cotrex_graphify_explain` — get details on a specific node
- `cotrex_graphify_path` — find connections between concepts

Reading >3 files manually without trying graphify first wastes tokens.

## What this is

Cotrex is a **deterministic RTK orchestration layer** written in Rust. It normalizes agent intent and
stream output **without owning execution**. RTK (`rtk`) is bundled next to the cotrex binary;
no separate install needed.

## Build & Test

```bash
cotrex -m cargo build                       # build (must be warning-clean before committing)
cotrex -m cargo test                        # all tests
cotrex -m cargo test -p cotrex              # CI-style: test only our crate, not vendored rtk
cotrex -m cargo test native_command_maps    # single test by name (substring match)
```

**CI** (`.github/workflows/ci.yml`): runs `cargo test -p cotrex` on ubuntu-latest with
submodules checked out. Wait for green before merging.

**Dependencies**: `rtk` and `graphify` are pinned git submodules under `vendor/`; clone with
`--recursive`. The `Cargo.toml` workspace includes `vendor/rtk` as a default member.

## Architecture Overview

One pipeline, four stages, shared by every front-end (CLI, stdin-JSON, MCP):

1. **Parse intent** (`core/intent.rs`) — CLI args and stdin JSON both collapse to one `Intent`.
2. **Map to RTK** (`Intent::to_rtk_args`) — first token in `RTK_NATIVE` allowlist routes to
   that dedicated rtk filter; anything else falls back to `rtk run -c "<command>"`.
3. **Orchestrate** (`core/orchestrate.rs`) — spawn `rtk`, read stdout+stderr on two threads feeding
   one mpsc channel (no async — stdlib `process` + `thread` + `mpsc` only).
4. **Normalize** (`core/normalize.rs`) — classify each line by severity (error/warning/info).

**Core contract**: stdout is machine-only (verbatim rtk lines + JSON result footer). Anything a
human reads goes to stderr. Never mix human text into stdout.

## Gotchas (read before building or editing)

### Release builds must build rtk FIRST

`build.rs` embeds the RTK binary into cotrex at compile time via `include_bytes!`. If RTK
hasn't been built yet, cotrex compiles with `rtk_not_embedded` and ships without it.

```bash
cargo build --release -p rtk        # must come first
cargo build --release -p cotrex     # now embeds rtk
```

The release workflow (`.github/workflows/release.yml`) does this automatically. Debug builds
skip embedding entirely (rtk is too large for dev cycles).

### Shell operators on Windows route through PowerShell

Since v2.6.0, commands containing `;`, `&&`, `|`, or `$()` are routed through PowerShell
on Windows — `cmd /C` doesn't support `;` as a command separator. The routing happens in
`intent.rs::to_rtk_args()` via `has_shell_operators()`.

### RTK version pin

`src/config/install.rs` pins `RTK_VERSION` (currently `v0.42.4`). The vendored submodule
(`vendor/rtk`) must match. When bumping rtk, update both the submodule tag AND the constant.

### Graphify auto-refresh

After a code-changing `cotrex run`, the graphify module (`src/graphify/`) auto-refreshes
the knowledge graph in `graphify-out/`. Read-only commands (`git status`) skip this.
The graph lives at `graphify-out/GRAPH_REPORT.md` — agents should read it before answering
architecture or codebase questions.

### Script workflow

For repetitive or multi-file changes, write a script to `Scripts/` and run:
```bash
cotrex script Scripts/name.sh    # runs through rtk, verifies via git diff
```
cotrex picks the interpreter by extension (`.sh`→bash, `.ps1`→PowerShell, `.py`→python).
It does NOT generate the script — the agent does.

## Invariants

- **Cotrex never bypasses RTK.** New tool support = a new entry in `RTK_NATIVE` or a new rtk
  subcommand, not a direct `Command::new("cargo")`.
- **Keep it sync.** The 2-threads+mpsc model is deliberate.
- **Never add async** unless Cotrex ever multiplexes many concurrent execs.

## Code Style

### Rust edition & formatting
- **Rust 2021 edition** (`edition = "2021"` in Cargo.toml).
- Standard `rustfmt` defaults — 4-space indentation, 100-char lines.
- Build must be **warning-clean** before committing.

### Imports
- `use` at the top of each function or module, not scattered inline.
- Standard library imports first, then external crates, then `crate::` internal imports.
- One `use` per line; group with blank lines between std / external / crate.

### Types & naming
- `snake_case` for functions, variables, modules.
- `PascalCase` for types, enums, structs.
- Enum variants: `PascalCase` (e.g. `Severity::Error`, `Action::Allow`).
- Constants: `SCREAMING_SNAKE_CASE` (e.g. `RAW_CAP`, `RTK_NATIVE`, `MAX_STEPS`).
- Private by default; `pub` only when the item is used from another module.
- Prefer `&str` in function signatures over `&String`.
- Use `impl Into<String>` for accepting owned strings that may be cloned.

### Error handling
- Functions return `Result<T, String>` — plain `String` errors, not `anyhow`/`thiserror`.
- Use `.map_err(|e| format!("context: {e}"))` to wrap errors with context.
- Use `.ok()` on I/O writes that must not fail (`writeln!(…).ok()`).
- `exit(code)` is acceptable in `main.rs` top-level dispatch; library modules return `Result`.

### Patterns
- `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]` on data structs.
- Serde: use `#[serde(default)]`, `#[serde(alias = "cmd")]`, `#[serde(rename_all = "lowercase")]`.
- `#[cfg(test)] mod tests` at the bottom of each file with `use super::*;`.
- Test names: `snake_case` describing the behavior (e.g. `native_command_maps_direct`).
- Use `assert_eq!` / `assert!` / `.is_err()` / `.is_ok()` — no custom test harness.
- `unwrap_or` / `unwrap_or_default` / `unwrap_or_else` preferred over bare `.unwrap()` in
  production code. `.unwrap()` is acceptable in tests.

### Documentation
- Module-level `//!` doc comments at the top of each `.rs` file.
- `///` doc comments on public items (structs, functions, constants).
- Inline `//` comments for non-obvious logic, especially around the core contract
  (stdout = machine, stderr = human) and concurrency patterns.
- `ponytail:` comments mark design decisions and known trade-offs.

### Strings & formatting
- Prefer `format!("...")` over manual string building.
- Use `to_ascii_lowercase()` for case-insensitive matching (not `to_lowercase`).
- `serde_json::json!()` macro for building JSON values.

### Concurrency
- No async runtime — stdlib `thread` + `mpsc` only.
- `AtomicBool` + `Ordering::Relaxed` for simple stop flags.
- Channels over mutexes when possible.

## Commit & Branch Rules

- **Never** put "claude" in a branch name or commit message.
- **Never** add AI co-author attribution to commits.
- **Real-time commits**: after changing a file, commit that change immediately. One logical
  change, one commit, right away. Don't batch unrelated edits.
- Concise subject line; short body explaining *why* when it isn't obvious.
- **Never push to `main`.** Every change ships through a PR:
  1. Branch off `main` with a descriptive name.
  2. Commit each logical change immediately.
  3. `cotrex -m gh pr create` to open a PR.
  4. Wait for CI to pass before merging.
  5. `cotrex -m gh pr merge --squash --delete-branch`.
  6. `cotrex -m git checkout main && git pull`, delete the local branch.

## Module Map

| Path | Purpose |
|---|---|
| `src/main.rs` | Entry point only — parse args, delegate to dispatch |
| `src/cli.rs` | CLI type definitions (Cli, Cmd, SUBCOMMANDS) |
| `src/dispatch/` | Subcommand dispatch and task routing |
| `src/core/intent.rs` | Normalize CLI/JSON to `Intent`; map to rtk args via `RTK_NATIVE` |
| `src/core/orchestrate.rs` | Spawn rtk, read pipes on 2 threads, write normalized events |
| `src/core/normalize.rs` | Classify output lines by severity (error/warning/info) |
| `src/config/settings.rs` | Persistent config in OS config dir; `cotrex -m setup` flow |
| `src/config/install.rs` | Download + extract pinned rtk release for current platform |
| `src/config/install_agent.rs` | Install cotrex skills into agent-specific directories |
| `src/config/update.rs` | Self-update: check GitHub release, download+install if newer |
| `src/llm/compress.rs` | Optional LLM compression — POST output, get structured insight |
| `src/llm/mcp.rs` | JSON-RPC 2.0 stdio MCP server (sync, no tokio) |
| `src/agent/prompt.rs` | Agentic task loop — model decides run-vs-answer, step loop |
| `src/agent/tool.rs` | Minimal tool registry (read/write/edit/glob/grep) for agentic loop |
| `src/agent/permission.rs` | Pattern-based permission rules for risky command gating |
| `src/script/` | `cotrex -m script` — run agent scripts through rtk, verify via git diff |
| `src/graphify/` | Auto-refresh code map after code-changing runs |
