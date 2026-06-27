---
id: usage
title: Usage
---

# Usage

## Run a command

Several args are a command (the `run` subcommand is optional); rtk output passes through verbatim:

```bash
cotrex run "git status"
cotrex git status          # same thing
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

Per-line JSON wrapping would cost more tokens than the raw command, so Cotrex doesn't. Failing
commands set `status: "failed"` in the footer and propagate the underlying exit code.

## Pipe an intent as JSON

```bash
echo '{"tool":"rtk","cmd":"cargo --version"}' | cotrex
```

`cmd` is accepted as an alias for `command`. Add `"llm": true` to request the insight.

## Compress output with an LLM

```bash
cotrex run --llm "cargo test"
```

After the normal lines, one extra event the agent can read instead of the full log:

```json
{"type":"insight","status":"failed","root_cause":"missing crate serde_json",
 "important_errors":["cannot find crate `serde_json`"],"suggested_fix":"add serde_json to Cargo.toml"}
```

The LLM call is best-effort: a network or parse failure prints `(llm skipped: …)` and never changes
the exit code. Requires a key from [Setup](setup).

## Prompts & categories

A single quoted arg is a *prompt*, not a command. **For a task, the model gathers with shell commands
and then SYNTHESIZES an answer** — it inspects step by step, never dumps a raw command log. A safe
read-only command runs unprompted; a risky one (delete, overwrite, install, push, network, sudo…)
asks first. If a command fails, the model reads the error and fixes it or answers from it.
`category: text` (or a JSON object) returns a structured answer instead. Requires a key from
[Setup](setup).

```bash
cotrex "list all rust projects in the current dir"     # → runs a command, prints the list
cotrex "what does the ? operator do?"                  # → answers (rendered markdown)
cotrex "plan-stack: build a music player app"          # category → structured answer
```

Two modes: `cotrex "…"` (User) shows a spinner, streams the model's output live to stderr (thinking,
or the answer text for instruct models), shows a running command's last 5 output lines live in a
```bash viewport, and renders answers as ANSI markdown; `cotrex -m "…"` (Model, for agents) shows none
of that — just raw output on stdout. A risky command's confirmation reads stdin, and no input aborts.
Add a category by adding a row to `CATEGORIES` in `prompt.rs`.

A **project-structure** ask (`"give me the project structure"`, `"show the directory tree"`) is
answered directly as a depth-limited tree from `git ls-files` — honors `.gitignore`, no model needed.

## Roles (offload to a role-specific model)

`cotrex <role> "<task>"` runs the same decide-then-do flow on a model chosen for that role, so a
calling agent offloads work and just waits. With no role, `assistant` is the default.

```bash
cotrex planner "plan releasing this crate to crates.io"   # glm
cotrex coder "write a Rust fn that reverses a string"     # deepseek
cotrex orchestrator "build the release artifacts"         # nemotron-ultra
# also: router (nemotron-nano), assistant (qwen, default)
```

Roles share your configured endpoint + key, swapping in the role's model id. Add or retune a role by
editing the `ROLES` table in `prompt.rs`.

## Install skills for your agent

Cotrex can install project-specific skills for your AI agent. This creates a `.cotrex/` directory in
your project with skill files tailored for your agent:

```bash
cotrex install opencode    # install skills for OpenCode
cotrex install claude      # install skills for Claude Code
cotrex install codex       # install skills for Codex
cotrex install cursor      # install skills for Cursor
```

Supported agents: `opencode`, `claude`, `codex`, `cursor`, `gemini`, `windsurf`, `aider`,
`continue`, `cline`.

The installed skills include:
- **graphify** - Knowledge graph generation from code/docs
- **cotrex-run** - Run commands through RTK with normalized output
- **cotrex-tree** - Show project structure as a tree

Skills are installed in `.cotrex/skills/` and are automatically detected by your agent when
working in this project directory.

To list installed skills in the current project:

```bash
cotrex install --list       # or just: cotrex install
```

## Scripting (repetitive or multi-file changes)

Don't edit many files by hand for the same change. Write one idempotent script under `Scripts/`,
then let cotrex run + verify it:

```bash
cotrex script                      # creates Scripts/ and prints the workflow
# ... write Scripts/rename.sh ...
cotrex script Scripts/rename.sh    # runs it through rtk, then shows `git diff`
```

cotrex runs the script through rtk (by extension: `.sh` / `.ps1` / `.py`), then `git diff --stat` so
you verify the change from the diff instead of re-reading every file. The agent writes the script;
cotrex runs and verifies it. (`git diff` shows tracked edits; new files show via `git status`.)

Next: [MCP](mcp).
