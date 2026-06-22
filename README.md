<p align="center">
  <!-- TODO: replace with the real project icon -->
  <img src="https://placehold.co/500x150?text=AEM" alt="AEM - Agent Execution Middleware" width="500">
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
  <a href="#what-is-aem">About</a> &bull;
  <a href="#installation">Install</a> &bull;
  <a href="#usage">Usage</a> &bull;
  <a href="CLAUDE.md">Architecture</a> &bull;
  <a href="#roadmap">Roadmap</a>
</p>

---

## What is AEM

AEM (Agent Execution Middleware) sits between an AI agent and the system. It takes an agent's
**intent**, forwards it to [RTK](https://github.com/rtk-ai/rtk) — the execution truth layer — and
returns a **normalized, dual-channel** result. AEM never runs a raw command itself; it invokes
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

**Requirements:** a Rust toolchain. [rtk](https://github.com/rtk-ai/rtk) and
[graphify](https://github.com/safishamsi/graphify) are vendored as git submodules under `vendor/`,
so clone recursively:

```bash
git clone --recursive <repo>
# or, in an existing clone:
git submodule update --init --recursive

cargo build --release
# binary at target/release/aem
```

> Note: vendoring puts the sources in-tree, but the build doesn't compile/bundle them yet — AEM
> currently still spawns `rtk` from your `PATH`. Building a single self-contained binary from
> `vendor/rtk` is a follow-up (see Roadmap).

## Usage

**Run a command through RTK:**

```bash
aem run "git status"
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
echo '{"tool":"rtk","cmd":"cargo --version"}' | aem
```

**Compress output with an LLM** (opt-in, fewer tokens for the agent):

```bash
aem run --llm "cargo test"
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
aem plan-stack "build a music player app"
```
```json
{
  "task": "build a music player app",
  "stack": "tauri",
  "reason": "cross-platform desktop with Rust core + web UI; small binaries",
  "init_commands": ["npm create tauri-app@latest"]
}
```

Failing commands classify error lines (`severity: "error"`), set `status: "failed"`, and propagate
the underlying exit code.

## LLM API key

The `--llm` flag reads config from the environment (or a local `.env`). Copy the template and fill
in a key from any free OpenAI-compatible provider:

```bash
cp .env.example .env
# edit .env:
#   AEM_LLM_URL=https://api.groq.com/openai/v1/chat/completions
#   AEM_LLM_KEY=gsk_...
#   AEM_LLM_MODEL=llama-3.1-8b-instant
```

`.env` is gitignored — your key never lands in a commit. Free endpoints that work out of the box:
Groq, OpenRouter (`:free` models), and NVIDIA NIM. See [.env.example](.env.example) for URLs.

## Development

```bash
cargo build                       # build (keep it warning-clean)
cargo test                        # run the self-checks
cargo test native_command_maps    # run a single test by name
```

See [CLAUDE.md](CLAUDE.md) for architecture and contributor rules.

## Roadmap

Deliberately out of scope for v1 — added when there's a consumer that needs them:

- **MCP server front-end** — agents calling AEM natively as tools. A new dispatch path over the
  same pipeline, not a rewrite.
- **LLM-backed `plan-stack`** — reuse the `--llm` path for stack recommendations.
- **Persisted execution graph** — command/dependency/failure trace (vendored [graphify](vendor/graphify)).
- **Single self-contained binary** — build `vendor/rtk` in a cargo workspace and have AEM use the
  vendored binary instead of requiring `rtk` on `PATH`, so there's nothing to install separately.

## License

TBD.
