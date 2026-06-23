---
id: usage
title: Usage
---

# Usage

## Run a command

Several args are a command (the `run` subcommand is optional); rtk output passes through verbatim:

```bash
tokex run "git status"
tokex git status          # same thing
```

```json
// stdout (machine) — rtk output verbatim, then one result footer
 M src/orchestrate.rs
{"type":"result","status":"ok","code":0}
```

```text
# stderr (human)
› rtk git status
‹ ok (exit 0, 0 error line(s))
```

Per-line JSON wrapping would cost more tokens than the raw command, so Tokex doesn't. Failing
commands set `status: "failed"` in the footer and propagate the underlying exit code.

## Pipe an intent as JSON

```bash
echo '{"tool":"rtk","cmd":"cargo --version"}' | tokex
```

`cmd` is accepted as an alias for `command`. Add `"llm": true` to request the insight.

## Compress output with an LLM

```bash
tokex run --llm "cargo test"
```

After the normal lines, one extra event the agent can read instead of the full log:

```json
{"type":"insight","status":"failed","root_cause":"missing crate serde_json",
 "important_errors":["cannot find crate `serde_json`"],"suggested_fix":"add serde_json to Cargo.toml"}
```

The LLM call is best-effort: a network or parse failure prints `(llm skipped: …)` and never changes
the exit code. Requires a key from [Setup](setup).

## Prompts & categories

A single quoted arg is a *prompt*, not a command. `category: text` uses that category's header
(system prompt); free text uses a default header; a JSON object runs several categories at once. The
model streams its thinking to stderr while you wait; the answer is JSON on stdout. Requires a key
from [Setup](setup).

```bash
tokex "plan-stack: build a music player app"
tokex '{"plan-stack":"music player","theme":"glassy"}'
tokex "find a python lib for web scraping"
```

```json
{ "plan-stack": { "stack": "tauri", "reason": "…" } }
```

Add a category by adding a row to `CATEGORIES` in `prompt.rs`.

Next: [MCP](mcp).
