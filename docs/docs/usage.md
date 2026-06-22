---
id: usage
title: Usage
---

# Usage

## Run a command

```bash
tokex run "git status"
```

```json
// stdout (machine) — newline-delimited JSON
{"type":"stdout","line":" M src/orchestrate.rs","severity":"info"}
{"type":"result","status":"ok","code":0}
```

```text
# stderr (human)
› rtk git status
‹ ok (exit 0, 0 error line(s))
```

Failing commands classify error lines (`severity: "error"`), set `status: "failed"`, and propagate
the underlying exit code.

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

## Stack planner (experimental)

```bash
tokex plan-stack "build a music player app"
```

```json
{ "task": "build a music player app", "stack": "tauri", "reason": "…", "init_commands": ["npm create tauri-app@latest"] }
```

Next: [MCP](mcp).
