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

A single quoted arg is a *prompt*, not a command. **Free text is a task: the model turns it into one
shell command and tokex runs it, returning the output** — not the command. `category: text` (or a
JSON object of several) instead returns a structured answer. Requires a key from [Setup](setup).

```bash
tokex "list all rust projects in the current dir"     # task → command, run, output
tokex "plan-stack: build a music player app"          # category → structured answer
tokex '{"plan-stack":"music player","theme":"glassy"}'
```

```json
// tokex "plan-stack: …"
{ "plan-stack": { "stack": "tauri", "reason": "…" } }
```

Two modes: `tokex "…"` (User) shows a spinner then streams the model's thinking to stderr; `tokex -m
"…"` (Model, for agents) shows neither — just the output on stdout. Add a category by adding a row to
`CATEGORIES` in `prompt.rs`.

## Scripting (repetitive or multi-file changes)

Don't edit many files by hand for the same change. Write one idempotent script under `Scripts/`,
then let tokex run + verify it:

```bash
tokex script                      # creates Scripts/ and prints the workflow
# ... write Scripts/rename.sh ...
tokex script Scripts/rename.sh    # runs it through rtk, then shows `git diff`
```

tokex runs the script through rtk (by extension: `.sh` / `.ps1` / `.py`), then `git diff --stat` so
you verify the change from the diff instead of re-reading every file. The agent writes the script;
tokex runs and verifies it. (`git diff` shows tracked edits; new files show via `git status`.)

Next: [MCP](mcp).
