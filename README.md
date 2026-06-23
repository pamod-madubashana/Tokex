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
agent intent  ──▶  parse  ──▶  map to rtk  ──▶  spawn rtk  ──▶  normalize lines  ──▶  dual output
 (CLI | JSON)      Intent     first token →     2 threads        {type,line,         stdout: NDJSON
                              rtk subcommand     + mpsc            severity}          stderr: summary
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
// stdout (machine)
{"type":"stdout","line":" M src/orchestrate.rs","severity":"info"}
{"type":"result","status":"ok","code":0}
```
```text
# stderr (human)
› rtk git status
‹ ok (exit 0, 0 error line(s))
```

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

**Get a tech-stack recommendation:**

```bash
tokex plan-stack "build a music player app"
```
```json
{
  "task": "build a music player app",
  "stack": "tauri",
  "reason": "cross-platform desktop with Rust core + web UI; small binaries"
}
```

Failing commands classify error lines (`severity: "error"`), set `status: "failed"`, and propagate
the underlying exit code.

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

- **LLM-backed `plan-stack`** — reuse the `--llm` path for stack recommendations.
- **Persisted execution graph** — command/dependency/failure trace of tokex runs themselves.
- **Single self-contained binary** — build `vendor/rtk` in a cargo workspace and have Tokex use the
  vendored binary instead of requiring `rtk` on `PATH`, so there's nothing to install separately.

## License

TBD.
