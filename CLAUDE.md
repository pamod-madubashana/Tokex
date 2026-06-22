# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Tokex is a **deterministic RTK orchestration layer**. It normalizes
agent intent and stream output **without owning execution**. RTK (`rtk`, an external binary that
must be on PATH) is the execution truth layer; Tokex never runs a raw command — it invokes
`rtk <subcommand>` and normalizes what RTK returns. If `rtk` is not on PATH, `tokex run` fails at
spawn — that dependency is functional, not optional.

## Vendored dependencies

`rtk` and `graphify` are git submodules under `vendor/`. Clone with `--recursive` (or
`git submodule update --init --recursive`). They are vendored as source; the build does **not** yet
compile or bundle them — Tokex still spawns `rtk` from `PATH`. Producing a single self-contained
binary from `vendor/rtk` is deferred.

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

`main.rs` is dispatch only: a subcommand (`run`/`plan-stack`) or, with no subcommand and piped
stdin, a JSON intent.

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

## Out of scope for v1 (deferred, do not add speculatively)

MCP server front-end, LLM-backed `plan-stack`, and a persisted execution graph. The core is
front-end-agnostic, so MCP is a new dispatch path over the same pipeline, not a rewrite.

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
