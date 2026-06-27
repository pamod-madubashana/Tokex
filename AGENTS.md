# AGENTS.md

Guidance for agentic coding agents working in this repository.

## What this is

Tokex is a **deterministic RTK orchestration layer** written in Rust. It normalizes agent intent and
stream output **without owning execution**. RTK (`rtk`, an external binary) is the execution truth
layer; Tokex never runs a raw command directly.

## Build & Test Commands

```bash
tokex cargo build                       # build (must be warning-clean before committing)
tokex cargo test                        # all tests
tokex cargo test native_command_maps    # run a single test by name (substring match)
tokex cargo test -p tokex               # CI-style: test only our crate, not vendored rtk
tokex cargo run -- run "git status"     # CLI: forward a command through rtk
tokex cargo run -- git status           # same — run subcommand optional (several args = command)
tokex echo '{"tool":"rtk","cmd":"cargo --version"}' | cargo run --   # stdin-JSON mode
tokex cargo run -- "plan-stack: music player app"                    # category prompt
tokex cargo run -- script Scripts/rename.sh                          # run a script via rtk
```

**CI** (`.github/workflows/ci.yml`): runs `cargo test -p tokex` on ubuntu-latest with
submodules checked out. Wait for green before merging.

**Dependencies**: `rtk` and `graphify` are pinned git submodules under `vendor/`; clone with
`--recursive`. The `Cargo.toml` workspace includes `vendor/rtk` as a default member.

## Architecture Overview

One pipeline, four stages, shared by every front-end (CLI, stdin-JSON, MCP):

1. **Parse intent** (`intent.rs`) — CLI args and stdin JSON both collapse to one `Intent`.
2. **Map to RTK** (`Intent::to_rtk_args`) — first token in `RTK_NATIVE` allowlist routes to
   that dedicated rtk filter; anything else falls back to `rtk run -c "<command>"`.
3. **Orchestrate** (`orchestrate.rs`) — spawn `rtk`, read stdout+stderr on two threads feeding
   one mpsc channel (no async — stdlib `process` + `thread` + `mpsc` only).
4. **Normalize** (`normalize.rs`) — classify each line by severity (error/warning/info).

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
  3. `gh pr create` to open a PR.
  4. Wait for CI to pass before merging.
  5. `gh pr merge --squash --delete-branch`.
  6. `git checkout main && git pull`, delete the local branch.

## Module Map

| File | Purpose |
|---|---|
| `main.rs` | CLI dispatch only — parse args, route to subcommands or prompts |
| `intent.rs` | Normalize CLI/JSON to `Intent`; map to rtk args via `RTK_NATIVE` |
| `orchestrate.rs` | Spawn rtk, read pipes on 2 threads, write normalized events |
| `normalize.rs` | Classify output lines by severity (error/warning/info) |
| `config.rs` | Persistent config in OS config dir; `tokex setup` flow |
| `llm.rs` | Optional LLM compression — POST output, get structured insight |
| `mcp.rs` | Hand-rolled JSON-RPC 2.0 stdio server (sync, no tokio) |
| `prompt.rs` | Agentic task loop — model decides run-vs-answer, step loop |
| `script.rs` | `tokex script` — run agent scripts through rtk, verify via git diff |
| `tool.rs` | Minimal tool registry (read/write/edit/glob/grep) for agentic loop |
| `permission.rs` | Pattern-based permission rules for risky command gating |
| `graphify.rs` | Auto-refresh code map after code-changing runs |
| `install.rs` | Download + extract pinned rtk release for current platform |
| `install_agent.rs` | Install tokex skills into agent-specific directories |
