# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Tokex is a **deterministic RTK orchestration layer**. It normalizes
agent intent and stream output **without owning execution**. RTK (`rtk`, an external binary that
must be on PATH) is the execution truth layer; Tokex never runs a raw command — it invokes
`rtk <subcommand>` and normalizes what RTK returns. If `rtk` is not on PATH, `tokex run` fails at
spawn — that dependency is functional, not optional.

## Getting rtk

`rtk_path()` (in `orchestrate.rs`) resolves rtk in order: next to the tokex binary → the data dir →
`PATH`. Three ways to provide it:
- `tokex install-rtk` (`install.rs`) downloads the matching `rtk-ai/rtk` release for the current
  OS/arch into the data dir (extracted via system `tar`).
- `cargo build` builds the vendored `vendor/rtk` next to tokex (workspace `default-members`).
- a system-installed `rtk` on `PATH`.

`rtk` and `graphify` are pinned git submodules under `vendor/`; clone with `--recursive`.

## Commands

```bash
cargo build                       # build (must be warning-clean before committing)
cargo test                        # all self-checks
cargo test native_command_maps    # single test by name (substring match)
cargo run -- run "git status"     # CLI front-end: forward a command through rtk
echo '{"tool":"rtk","cmd":"cargo --version"}' | cargo run --   # stdin-JSON front-end
cargo run -- plan-stack "build a music player app"             # stack planner
```

## Architecture

One pipeline, four stages, shared by every front-end:

1. **Parse intent** (`intent.rs`) — CLI args *and* stdin JSON both collapse to one `Intent`
   (`{tool, action, command, stream}`; accepts `cmd` as an alias for `command`).
2. **Map to RTK** (`Intent::to_rtk_args`) — the command's first token decides the rtk invocation:
   a token in the `RTK_NATIVE` allowlist (git, cargo, npm, …) routes to that dedicated rtk filter
   (`cargo test` → `rtk cargo test`); anything else falls back to `rtk run -c "<command>"`.
3. **Orchestrate** (`orchestrate.rs`) — validate, spawn the `rtk` child, read its stdout+stderr on
   **two threads feeding one mpsc channel** so the streams interleave live. No async runtime —
   stdlib `process` + `thread` + `mpsc` only.
4. **Normalize** (`normalize.rs`) — each rtk text line becomes `{type, line, severity}`; severity is
   a blunt keyword classifier (`error|failed|panic|fatal` → error, `warn` → warning, else info).

**Dual channel (the core contract):** machine output is **NDJSON on stdout** (one `LineEvent` per
line, terminated by a single `{"type":"result", ...}` line); the human-readable summary goes to
**stderr**. Keep these separated by file descriptor — never mix human text into stdout.

`main.rs` is dispatch only: a subcommand (`run`/`plan-stack`/`setup`/`mcp`) or, with no subcommand
and piped stdin, a JSON intent.

**Front-ends share the core.** CLI, stdin-JSON, and the MCP server (`mcp.rs`, `tokex mcp`) all funnel
into the same `orchestrate::run`. MCP is a hand-rolled JSON-RPC 2.0 stdio server (sync, no tokio)
exposing a `run` tool; it captures the machine channel into a buffer and returns the events as the
tool result. **stdout is the JSON-RPC channel in MCP mode** — the core writes to in-memory buffers,
never stdout, so nothing corrupts the protocol.

## Invariants

- **Tokex never bypasses RTK.** New tool support = a new entry in `RTK_NATIVE` or a new rtk
  subcommand, not a direct `Command::new("cargo")`.
- **stdout is machine-only.** Anything a human reads goes to stderr.
- Keep it sync. The 2-threads+mpsc model is deliberate; reach for async only if Tokex ever
  multiplexes many concurrent execs.

## Config & modes (`tokex setup`)

Config lives in the user's OS config dir (`config.rs`: `dirs::config_dir()/tokex/config.toml`), set
post-install via `tokex setup` (interactive `inquire` prompts) — **not** a project `.env`.
`config::load()` reads the file then applies `TOKEX_LLM_*` env overrides. Two modes drive execution
(`main.rs` → `orchestrate::Options`):
- **compression**: `off` (raw `rtk run -c`, no filter) · `heuristic` (filtered subcommand, default) ·
  `llm` (filtered + the AI insight).
- **rtk_verbosity**: `normal` · `ultra-compact` (appends `--ultra-compact` to the rtk args).

`tokex run --llm` (or JSON `"llm": true`) forces the insight on regardless of mode. LLM compression
(`llm.rs`) POSTs the captured output to an OpenAI-compatible endpoint and emits one extra
`{"type":"insight", ...}` event. Missing key when LLM is requested = fail fast (`run tokex setup`).
The call is best-effort: a network/parse failure prints `(llm skipped: …)` and never changes the
exit code.

## graphify code map (`graphify.rs`)

tokex keeps a graphify code map fresh so agents only **read** it (`graphify-out/GRAPH_REPORT.md`,
`graphify-out/wiki/`) and never spend a turn updating it. graphify is a Python tool
(`pip install graphifyy`, invoked as `python -m graphify ...`, AST-only — no token cost).

After a **code-changing** `tokex run` (read-only commands like `git status` skip — see
`touches_code`), `auto_update`:
- if set up → fires a background `graphify update .`;
- if not → runs the one-time bootstrap **detached** (re-spawns `tokex graph`) so it never blocks the
  command.

The one-time bootstrap: `ensure_package` (`pip install graphifyy`, cached via `.graphify-ok`) →
`register_skill` (cached via `.graphify-skill`) → build the map. **Skill registration targets the
agent actually in use**, not just Claude: `resolve_platform` reads `config.agent`, else env
auto-detects Claude (`CLAUDECODE`), else asks the user when interactive (or leaves guidance to run
`tokex setup`). It calls `graphify install` (claude), `graphify install --platform <p>`, or
`graphify <p> install` (fallback for graphify's per-platform subcommands).

`tokex setup` runs the whole bootstrap up front (the "start project" moment); `tokex graph` forces a
blocking refresh. All best-effort — never blocks or fails a tokex run. Gated by `graph_auto`.

## Out of scope (deferred, do not add speculatively)

LLM-backed `plan-stack`.

## Commit & attribution rules (must follow)

- **Never** put the word "claude" in a branch name or commit message.
- **Never** add an AI co-author or attribution trailer (`Co-Authored-By: Claude …`,
  "Generated with…") to commits or PRs.
- **Real-time commits (must follow):** after changing a file, commit that change immediately.
  Don't batch unrelated edits into one commit or leave the tree dirty between steps — one logical
  change, one commit, right away.
- Concise subject line; short body explaining *why* when it isn't obvious.

## Branch & PR workflow (must follow)

**Never push to `main`.** Every change ships through a PR:

1. Branch off `main` with a fresh, descriptive name. **Never** put "claude" in a branch name.
2. Make changes there (real-time commits still apply — commit each logical change immediately).
3. `gh pr create` to open a PR.
4. **Wait for the `CI` workflow to pass** (`.github/workflows/ci.yml` builds + tests `tokex`). Do not
   merge on red.
5. Merge once green (`gh pr merge --squash --delete-branch`).
6. Clean up and sync: `git checkout main && git pull`, and delete the local branch.
