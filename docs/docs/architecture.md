---
id: architecture
title: Architecture
---

# Architecture

One pipeline, four stages, shared by every front-end (CLI, stdin-JSON, MCP).

1. **Parse intent** (`intent.rs`) — every front-end collapses to one `Intent`
   (`{tool, action, command, stream, llm}`; `cmd` is an alias for `command`).
2. **Map to RTK** (`Intent::to_rtk_args`) — the command's first token decides the rtk invocation: a
   token in the `RTK_NATIVE` allowlist routes to that dedicated rtk filter; anything else falls back
   to `rtk run -c "<command>"`.
3. **Orchestrate** (`orchestrate.rs`) — validate, spawn the `rtk` child, read stdout + stderr on
   **two threads feeding one mpsc channel** so the streams interleave live. No async runtime —
   stdlib `process` + `thread` + `mpsc`.
4. **Normalize** (`normalize.rs`) — each rtk line becomes `{type, line, severity}`; severity is a
   keyword classifier (`error|failed|panic|fatal` → error, `warn` → warning, else info).

## Dual channel

Machine output is **NDJSON on stdout** (one event per line, terminated by a single
`{"type":"result", …}`); the human-readable summary goes to **stderr**. They are separated by file
descriptor — human text never lands on stdout. (In MCP mode the machine channel is captured into the
tool result instead, keeping stdout free for JSON-RPC.)

## Modes

Set via [`tokex setup`](setup), applied per run:

- **compression**: `off` (raw `rtk run -c`) · `heuristic` (filtered) · `llm` (filtered + AI insight).
- **rtk verbosity**: `normal` · `ultra-compact` (appends `--ultra-compact`).

## Invariants

- **Tokex never bypasses RTK.** New tool support is a new entry in `RTK_NATIVE` or a new rtk
  subcommand — never a direct `Command::new("cargo")`.
- **stdout is machine-only** (JSON-RPC in MCP mode).
- The 2-threads + mpsc model is deliberate; async only if Tokex ever multiplexes many concurrent
  executions.

## graphify code map

tokex keeps a [graphify](https://github.com/safishamsi/graphify) code map fresh so agents only
**read** it (`graphify-out/`) instead of spending turns updating it. After a code-changing
`tokex run`, tokex fires a background `graphify update .` (Python, AST-only — no token cost);
read-only commands skip it. graphifyy is auto-installed once. `tokex graph` forces a refresh; toggle
with `graph_auto` in [Setup](setup).

## Vendored dependencies

`rtk` and `graphify` are pinned git submodules under `vendor/`. `cargo build` builds `tokex` and
`rtk` together via workspace `default-members`, so they ship as one unit.
