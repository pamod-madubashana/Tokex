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
4. **Normalize** (`normalize.rs`) — classify each rtk line by severity (`error|failed|panic|fatal`
   → error, `warn` → warning, else info). The text passes through verbatim; severity is internal.

## Dual channel

Machine output on **stdout** is the rtk output lines **verbatim**, terminated by a single
`{"type":"result", …}` footer (plus an `{"type":"insight", …}` line when a failure was analyzed).
Per-line JSON wrapping would cost more tokens than the raw command Cotrex is meant to compress. The
human-readable summary goes to **stderr** — human text never lands on stdout. (In MCP mode the
machine channel is captured into the tool result instead, keeping stdout free for JSON-RPC.)

## Modes

Set via [`cotrex setup`](setup), applied per run:

- **compression**: `off` (raw `rtk run -c`) · `heuristic` (filtered) · `llm` (filtered + AI insight
  on failures only — a successful command stays token-free).
- **rtk verbosity**: `normal` · `ultra-compact` (appends `--ultra-compact`).

## Invariants

- **Cotrex never bypasses RTK.** New tool support is a new entry in `RTK_NATIVE` or a new rtk
  subcommand — never a direct `Command::new("cargo")`.
- **stdout is machine-only** (JSON-RPC in MCP mode).
- The 2-threads + mpsc model is deliberate; async only if Cotrex ever multiplexes many concurrent
  executions.

## graphify code map

cotrex keeps a [graphify](https://github.com/safishamsi/graphify) code map fresh so agents only
**read** it (`graphify-out/`) instead of spending turns updating it. After a code-changing
`cotrex run`, cotrex fires a background `graphify update .` (Python, AST-only — no token cost);
read-only commands skip it. The one-time setup runs detached so it never blocks a command: it
auto-installs graphifyy and registers the graphify skill **for the agent you actually use** —
resolved from config, env auto-detect (Claude), or by asking you — then builds the map. `cotrex setup`
runs this bootstrap up front; `cotrex graph` forces a refresh. Toggle with `graph_auto` in
[Setup](setup).

## Vendored dependencies

`rtk` and `graphify` are pinned git submodules under `vendor/`. `cargo build` builds `cotrex` and
`rtk` together via workspace `default-members`, so they ship as one unit.
