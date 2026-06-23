<p align="center">
  <img src="assets/tokex.png" alt="Tokex" width="220">
</p>

<p align="center">
  <strong>A deterministic RTK orchestration layer that normalizes agent intent and stream output — without owning execution</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/built_with-Rust-orange.svg" alt="Built with Rust">
  <img src="https://img.shields.io/badge/status-MVP-yellow.svg" alt="Status: MVP">
  <img src="https://img.shields.io/badge/license-TBD-lightgrey.svg" alt="License: TBD">
</p>

<p align="center">
  <a href="#what-is-tokex">About</a> &bull;
  <a href="#installation">Install</a> &bull;
  <a href="#usage">Usage</a> &bull;
  <a href="CLAUDE.md">Architecture</a> &bull;
  <a href="#roadmap">Roadmap</a>
</p>

---

## What is Tokex

Tokex sits between an AI agent and the system. It takes an agent's
**intent**, forwards it to [RTK](https://github.com/rtk-ai/rtk) — the execution truth layer — and
returns a **normalized, dual-channel** result. Tokex never runs a raw command itself; it invokes
`rtk <subcommand>` and tags what RTK emits.

- **Machine channel** (`stdout`): newline-delimited JSON, one event per line.
- **Human channel** (`stderr`): a short readable summary.

This keeps the model's input structured and small while a human still gets a glanceable trace.

## How it works

One pipeline, shared by every front-end:

```
agent intent  ──▶  parse  ──▶  map to rtk  ──▶  spawn rtk  ──▶  classify lines  ──▶  dual output
 (CLI | JSON)      Intent     first token →     2 threads        severity for       stdout: raw lines
                              rtk subcommand     + mpsc           error count        + result footer
                                                                                     stderr: summary
```

The command's first token picks the RTK invocation: a known tool (`git`, `cargo`, `npm`, …) routes
to that dedicated rtk filter (`cargo test` → `rtk cargo test`); anything else falls back to
`rtk run -c "<command>"`.

## Installation

Tokex downloads its `rtk` backend **automatically** on first run — there's nothing else to install.

**With your agent** — paste this to Claude Code / Cursor / Codex:

```text
Install Tokex: download the latest release for my OS/arch from
https://github.com/pamod-madubashana/Tokex/releases/latest, extract the `tokex` binary, put it on
my PATH, and confirm with `tokex --version`. It fetches its rtk backend automatically on first run.
```

**Manual** — download the archive for your platform from
[Releases](https://github.com/pamod-madubashana/Tokex/releases/latest), extract `tokex`, put it on
your `PATH`, and run `tokex --version`. (`tokex install-rtk` pre-fetches rtk for offline/CI use.)

**Build from source** — needs a Rust toolchain. `rtk` and `graphify` are pinned git submodules, so
clone recursively:

```bash
git clone --recursive https://github.com/pamod-madubashana/Tokex
cd Tokex
cargo build --release          # builds tokex + rtk into target/release/
```

(Already cloned flat? `git submodule update --init --recursive`.)

## Usage

**Run a command through RTK:**

```bash
tokex run "git status"
tokex git status        # same thing — the run subcommand is optional
```

```jsonc
// stdout (machine): rtk output verbatim, then one result footer
 M src/orchestrate.rs
{"type":"result","status":"ok","code":0}
```
```text
# stderr (human)
› rtk git status
‹ ok (exit 0, 0 error line(s))
```

Output lines pass through verbatim — Tokex never pays a per-line JSON tax for what's meant to
*compress* command output. Status rides on the single footer (and, on failure, an `insight` line).

**Pipe an intent as JSON** (no subcommand):

```bash
echo '{"tool":"rtk","cmd":"cargo --version"}' | tokex
```

**Compress output with an LLM** (opt-in, fewer tokens for the agent):

```bash
tokex run --llm "cargo test"
```

```jsonc
// after the normal lines + result, one extra event:
{"type":"insight","status":"failed","root_cause":"missing crate serde_json",
 "important_errors":["cannot find crate `serde_json`"],"suggested_fix":"add serde_json to Cargo.toml"}
```

The agent can read just that insight instead of the full log. Needs an API key (below); without
`--llm`, no key is read and no request is made.

**Prompts (a single quoted arg).** Several unquoted args are a command (`tokex git status`); a
single quoted string is a prompt sent to the model. `category: text` uses that category's header;
free text uses a default header; a JSON object runs several categories at once. The model streams
its thinking to stderr while you wait; the answer is JSON on stdout.

```bash
tokex "plan-stack: build a music player app"          # one category
tokex '{"plan-stack":"music player","theme":"glassy"}'  # several at once
tokex "find a python lib for web scraping"            # no category
```
```json
{ "plan-stack": { "stack": "tauri", "reason": "cross-platform desktop; small binaries" } }
```

Failing commands set `status: "failed"` in the footer and propagate the underlying exit code; in
`llm` mode a failure also gets an `insight` line (a successful command stays token-free).

**Scripting (repetitive or multi-file changes).** Don't edit ten files by hand to rename a token —
write one idempotent script under `Scripts/` and let Tokex run + verify it:

```bash
tokex script                      # creates Scripts/ and prints the workflow
# ... write Scripts/rename.sh ...
tokex script Scripts/rename.sh    # runs it through rtk, then shows `git diff`
```

Tokex runs the script through rtk (by extension: `.sh`/`.ps1`/`.py`), then `git diff --stat` so you
**verify from the diff, not by re-reading files**. The agent writes the script; Tokex runs and
verifies it. (`git diff` shows tracked edits; new files show via `git status`.)

## Setup (provider, API key, modes)

Configure Tokex *after* install with one interactive command — no file editing:

```bash
tokex setup
```

It prompts for:
- **Provider** — Groq, OpenRouter, NVIDIA NIM (presets fill the URL + a default model), or Custom.
- **API key** — masked input from any free OpenAI-compatible provider.
- **Compression** — `heuristic` (rtk filter, default) · `llm` (rtk + AI insight) · `off` (raw).
- **RTK output** — `normal` or `ultra-compact`.

Settings are written to your OS config dir (`%APPDATA%\tokex\config.toml` on Windows,
`~/.config/tokex/config.toml` on Linux) — never to the repo. `TOKEX_LLM_URL`/`_KEY`/`_MODEL` env
vars override the file for CI/power use. `tokex run --llm …` forces the insight on for one run
regardless of the configured mode.

## MCP server

Agents that speak MCP can call Tokex natively instead of shelling out. Tokex runs as an MCP server
over stdio and exposes a `run` tool that returns structured execution events.

```bash
tokex mcp        # JSON-RPC 2.0 over stdio
```

Register it with Claude Code:

```bash
claude mcp add tokex -- /absolute/path/to/tokex mcp
```

The `run` tool takes `{ "command": "cargo test", "llm": false }` and returns the normalized event
list (stdout/stderr lines with severity, a result with exit code, and an optional LLM insight) —
the same machine channel as the CLI, just delivered as a tool result.

## Development

```bash
cargo build                       # build (keep it warning-clean)
cargo test                        # run the self-checks
cargo test native_command_maps    # run a single test by name
```

See [CLAUDE.md](CLAUDE.md) for architecture and contributor rules.

## Roadmap

Deliberately out of scope for v1 — added when there's a consumer that needs them:

- **Persisted execution graph** — command/dependency/failure trace of tokex runs themselves.
- **Single self-contained binary** — build `vendor/rtk` in a cargo workspace and have Tokex use the
  vendored binary instead of requiring `rtk` on `PATH`, so there's nothing to install separately.

## License

TBD.
