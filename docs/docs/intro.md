---
id: intro
title: Introduction
slug: /
---

<p align="center">
  <img src="/Tokex/img/tokex.png" alt="Tokex" width="180" />
</p>

# Tokex

**RTK executes. Tokex makes execution consumable by agents.**

Tokex is a deterministic [RTK](https://github.com/rtk-ai/rtk) orchestration layer. It takes an
agent's intent, forwards it to RTK — the execution truth layer — and returns a **normalized,
dual-channel** result. Tokex never runs a raw command itself; it invokes `rtk <subcommand>` and
tags what RTK emits.

- **Machine channel** (`stdout`): newline-delimited JSON, one event per line.
- **Human channel** (`stderr`): a short readable summary.

The model reads small, structured events instead of noisy logs; a human still gets a glanceable
trace. It's infrastructure, not an agent.

## How it works

```
agent intent  ──▶  parse  ──▶  map to rtk  ──▶  spawn rtk  ──▶  normalize lines  ──▶  dual output
 (CLI | JSON       Intent     first token →     2 threads        {type,line,         stdout: NDJSON
  | MCP)                      rtk subcommand     + mpsc            severity}          stderr: summary
```

The command's first token picks the RTK invocation: a known tool (`git`, `cargo`, `npm`, …) routes
to that dedicated rtk filter (`cargo test` → `rtk cargo test`); anything else falls back to
`rtk run -c "<command>"`.

## Three front-ends, one core

- **CLI** — `tokex run "cargo test"`
- **stdin-JSON** — `echo '{"tool":"rtk","cmd":"git status"}' | tokex`
- **MCP** — `tokex mcp` exposes a `run` tool agents call natively

All three funnel into the same execution pipeline.

Next: [Installation](installation).
