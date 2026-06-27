# AGENTS.md

Guidance for agentic coding agents working in this repository.

## Agent Rules (MUST follow)

- **Connect to tokex via MCP.** Tokex exposes a JSON-RPC 2.0 stdio MCP server with tools
  (`run`, `delegate`, `plan`, `list_roles`, `set_agent`). Use the `run` tool to execute commands.
- **If MCP is not available**, fall back to CLI: `tokex -m <command>` (e.g. `tokex -m cargo build`).
- **If tokex is not on PATH**, fall back to `rtk <command>` directly. Raw shell commands are
  a last resort — they lose the normalization pipeline.
- **Never bypass the pipeline.** Tokex normalizes output for agents — raw commands lose that.

## MCP Server

Start tokex as an MCP server: `tokex mcp`

Communicates over stdin/stdout using newline-delimited JSON-RPC 2.0. The agent sends requests,
tokex responds with structured tool results.

### Available Tools

| Tool | Description |
|------|-------------|
| `run` | Execute a shell command through RTK. Returns normalized stdout/stderr with severity, exit code, and optional LLM insight. |
| `set_agent` | Identify your platform (claude/codex/cursor/gemini/opencode) so graphify installs the right code-map skill. Call once. |
| `list_roles` | List available roles (planner, coder, assistant, etc.) with their models. |
| `delegate` | Offload a task to a role — the role's model runs commands and returns an analyzed answer. |
| `plan` | Shorthand for `delegate` with the planner role. |

### MCP Config by Agent Platform

**Claude Code** — add to `~/.claude/settings.json`:
```json
{
  "mcpServers": {
    "tokex": { "command": "tokex", "args": ["mcp"] }
  }
}
```

**Codex / OpenCode** — add to agent config:
```json
{
  "mcpServers": {
    "tokex": { "command": "tokex", "args": ["mcp"] }
  }
}
```

**Cursor** — add to `.cursor/mcp.json`:
```json
{
  "mcpServers": {
    "tokex": { "command": "tokex", "args": ["mcp"] }
  }
}
```

**Gemini CLI** — add to agent config:
```json
{
  "mcpServers": {
    "tokex": { "command": "tokex", "args": ["mcp"] }
  }
}
```

### Example MCP Interaction

```json
→ {"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"tokex","version":"1.2.0"}}}

→ {"jsonrpc":"2.0","method":"notifications/initialized"}
  (no response — notification)

→ {"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
← {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"run",...},{"name":"set_agent",...},...]}}

→ {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"run","arguments":{"command":"cargo test"}}}
← {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"ok 2286 passed\n{\"type\":\"result\",\"status\":\"ok\",\"code\":0}"}],"isError":false}}
```

## CLI Fallback (when MCP is unavailable)

```bash
tokex -m cargo build                       # build (must be warning-clean before committing)
tokex -m cargo test                        # all tests
tokex -m cargo test native_command_maps    # run a single test by name (substring match)
tokex -m cargo test -p tokex               # CI-style: test only our crate, not vendored rtk
tokex -m cargo run -- run "git status"     # forward a command through rtk
tokex -m cargo run -- git status           # same — run subcommand optional
tokex -m script Scripts/rename.sh          # run a script via rtk
tokex -m update                            # check for newer release and install if available
```

## What this is

Tokex is a **deterministic RTK orchestration layer** written in Rust. It normalizes agent intent and
stream output **without owning execution**. RTK (`rtk`) is bundled next to the tokex binary;
no separate install needed.

## Build & Test Commands

```bash
tokex -m cargo build                       # build (must be warning-clean before committing)
tokex -m cargo test                        # all tests
tokex -m cargo test -p tokex               # CI-style: test only our crate, not vendored rtk
```

**CI** (`.github/workflows/ci.yml`): runs `cargo test -p tokex` on ubuntu-latest with
submodules checked out. Wait for green before merging.

**Dependencies**: `rtk` and `graphify` are pinned git submodules under `vendor/`; clone with
`--recursive`. The `Cargo.toml` workspace includes `vendor/rtk` as a default member.
`cargo build` builds both tokex and rtk into `target/release/`.

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

## Invariants

- **Tokex never bypasses RTK.** New tool support = a new entry in `RTK_NATIVE` or a new rtk
  subcommand, not a direct `Command::new("cargo")`.
- **Keep it sync.** The 2-threads+mpsc model is deliberate.
- **Never add async** unless Tokex ever multiplexes many concurrent execs.

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
  3. `tokex -m gh pr create` to open a PR.
  4. Wait for CI to pass before merging.
  5. `tokex -m gh pr merge --squash --delete-branch`.
  6. `tokex -m git checkout main && git pull`, delete the local branch.

## Module Map

| Path | Purpose |
|---|---|
| `src/main.rs` | Entry point only — parse args, delegate to dispatch |
| `src/cli.rs` | CLI type definitions (Cli, Cmd, SUBCOMMANDS) |
| `src/dispatch/` | Subcommand dispatch and task routing |
| `src/core/intent.rs` | Normalize CLI/JSON to `Intent`; map to rtk args via `RTK_NATIVE` |
| `src/core/orchestrate.rs` | Spawn rtk, read pipes on 2 threads, write normalized events |
| `src/core/normalize.rs` | Classify output lines by severity (error/warning/info) |
| `src/config/settings.rs` | Persistent config in OS config dir; `tokex -m setup` flow |
| `src/config/install.rs` | Download + extract pinned rtk release for current platform |
| `src/config/install_agent.rs` | Install tokex skills into agent-specific directories |
| `src/config/update.rs` | Self-update: check GitHub release, download+install if newer |
| `src/llm/compress.rs` | Optional LLM compression — POST output, get structured insight |
| `src/llm/mcp.rs` | JSON-RPC 2.0 stdio MCP server (sync, no tokio) |
| `src/agent/prompt.rs` | Agentic task loop — model decides run-vs-answer, step loop |
| `src/agent/tool.rs` | Minimal tool registry (read/write/edit/glob/grep) for agentic loop |
| `src/agent/permission.rs` | Pattern-based permission rules for risky command gating |
| `src/script/` | `tokex -m script` — run agent scripts through rtk, verify via git diff |
| `src/graphify/` | Auto-refresh code map after code-changing runs |
