# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## What this is

Tokex is a **deterministic RTK orchestration layer**. It normalizes
agent intent and stream output **without owning execution**. RTK (`rtk`, an external binary that
must be on PATH) is the execution truth layer; Tokex never runs a raw command — it invokes
`rtk <subcommand>` and normalizes what RTK returns. If `rtk` is not on PATH, `tokex run` fails at
spawn — that dependency is functional, not optional.

## Getting rtk

`rtk_path()` (in `orchestrate.rs`) resolves rtk in order: next to the tokex binary → the data dir →
`PATH`. Three ways to provide it:
- `tokex install-rtk` (`install.rs`) downloads the matching `rtk-ai/rtk` release for the current
  OS/arch into the data dir (extracted via system `tar`).
- `cargo build` builds the vendored `vendor/rtk` next to tokex (workspace `default-members`).
- a system-installed `rtk` on `PATH`.

`rtk` and `graphify` are pinned git submodules under `vendor/`; clone with `--recursive`.

## Commands

```bash
cargo build                       # build (must be warning-clean before committing)
cargo test                        # all self-checks
cargo test native_command_maps    # single test by name (substring match)
cargo run -- run "git status"     # CLI front-end: forward a command through rtk
cargo run -- git status           # same thing — run subcommand optional (several args = command)
echo '{"tool":"rtk","cmd":"cargo --version"}' | cargo run --   # stdin-JSON front-end
cargo run -- "plan-stack: music player app"                    # category prompt (single quoted arg)
cargo run -- script Scripts/rename.sh                          # run a script through rtk + verify via git diff
```

## Architecture

One pipeline, four stages, shared by every front-end:

1. **Parse intent** (`intent.rs`) — CLI args *and* stdin JSON both collapse to one `Intent`
   (`{tool, action, command, stream}`; accepts `cmd` as an alias for `command`).
2. **Map to RTK** (`Intent::to_rtk_args`) — the command's first token decides the rtk invocation:
   a token in the `RTK_NATIVE` allowlist (git, cargo, npm, …) routes to that dedicated rtk filter
   (`cargo test` → `rtk cargo test`); anything else falls back to `rtk run -c "<command>"`.
3. **Orchestrate** (`orchestrate.rs`) — validate, spawn the `rtk` child, read its stdout+stderr on
   **two threads feeding one mpsc channel** so the streams interleave live. No async runtime —
   stdlib `process` + `thread` + `mpsc` only.
4. **Normalize** (`normalize.rs`) — classify each rtk line by severity (`error|failed|panic|fatal`
   → error, `warn` → warning, else info). The line text passes through **verbatim**; severity is
   internal (error count + insight gating), not serialized per line.

**Dual channel (the core contract):** machine output on **stdout** is the rtk output lines
**verbatim**, terminated by a single `{"type":"result", ...}` footer (plus a `{"type":"insight"…}`
line when a failure was analyzed). Wrapping every line in JSON would cost the agent more tokens than
the raw command it's meant to compress, so don't. The human-readable summary goes to **stderr**.
Keep these separated by file descriptor — never mix human text into stdout.

`main.rs` is dispatch only. With a first arg that isn't a subcommand: several args = a command
(`tokex git status`), a single (quoted) arg = a prompt (`tokex "list rust projects"`, see
`prompt.rs`). `tokex -m "…"` is the same prompt in **Model mode** (output only, for agents) vs the
default **User mode** (spinner + live-streamed thinking, for humans). Otherwise a subcommand
(`run`/`script`/`setup`/`mcp`/…) or, with no subcommand and piped stdin, a JSON intent.

**Front-ends share the core.** CLI, stdin-JSON, and the MCP server (`mcp.rs`, `tokex mcp`) all funnel
into the same `orchestrate::run`. MCP is a hand-rolled JSON-RPC 2.0 stdio server (sync, no tokio)
exposing `run` (captures the machine channel, returns it verbatim) and `set_agent` (the model identifies
its platform when there's no TTY — persists `config.agent` and installs the graphify skill in the
background). **stdout is the JSON-RPC channel in MCP mode** — the core and the `set_agent` bootstrap
write to buffers / detached null stdio, never stdout, so nothing corrupts the protocol.

## Invariants

- **Tokex never bypasses RTK.** New tool support = a new entry in `RTK_NATIVE` or a new rtk
  subcommand, not a direct `Command::new("cargo")`.
- **stdout is machine-only.** Anything a human reads goes to stderr.
- Keep it sync. The 2-threads+mpsc model is deliberate; reach for async only if Tokex ever
  multiplexes many concurrent execs.

## Config & modes (`tokex setup`)

Config lives in the user's OS config dir (`config.rs`: `dirs::config_dir()/tokex/config.toml`), set
post-install via `tokex setup` (interactive `inquire` prompts) — **not** a project `.env`.
`config::load()` reads the file then applies `TOKEX_LLM_*` env overrides. Two modes drive execution
(`main.rs` → `orchestrate::Options`):
- **compression**: `off` (raw `rtk run -c`, no filter) · `heuristic` (filtered subcommand, default) ·
  `llm` (filtered + the AI insight).
- **rtk_verbosity**: `normal` · `ultra-compact` (appends `--ultra-compact` to the rtk args).

`tokex run --llm` (or JSON `"llm": true`) forces the insight on regardless of mode. LLM compression
(`llm.rs`) POSTs the captured output to an OpenAI-compatible endpoint and emits one extra
`{"type":"insight", ...}` event. Missing key when LLM is requested = fail fast (`run tokex setup`).
The call is best-effort: a network/parse failure prints `(llm skipped: …)` and never changes the
exit code.

## graphify code map (`graphify.rs`)

tokex keeps a graphify code map fresh so agents only **read** it (`graphify-out/GRAPH_REPORT.md`,
`graphify-out/wiki/`) and never spend a turn updating it. graphify is a Python tool
(`pip install graphifyy`, invoked as `python -m graphify ...`, AST-only — no token cost).

After a **code-changing** `tokex run` (read-only commands like `git status` skip — see
`touches_code`), `auto_update`:
- if set up → fires a background `graphify update .`;
- if not → runs the one-time bootstrap **detached** (re-spawns `tokex graph`) so it never blocks the
  command.

The one-time bootstrap: `ensure_package` (`pip install graphifyy`, cached via `.graphify-ok`) →
`register_skill` (cached via `.graphify-skill`) → build the map. **Skill registration targets the
agent actually in use**, not just Codex: `resolve_platform` reads `config.agent`, else env
auto-detects Codex (`Codex`), else asks the user when interactive (or leaves guidance to run
`tokex setup`). It calls `graphify install` (Codex), `graphify install --platform <p>`, or
`graphify <p> install` (fallback for graphify's per-platform subcommands).

`tokex setup` runs the whole bootstrap up front (the "start project" moment); `tokex graph` forces a
blocking refresh. All best-effort — never blocks or fails a tokex run. Gated by `graph_auto`.

## Prompts & categories (`prompt.rs`)

A single quoted arg is a *prompt*, not a command. `prompt::classify` routes it:
- **free text → a task** (`Prompt`): handled by the **`assistant` role** (the default when none is
  named). `prompt::fulfill` asks the model to **decide** (`DECISION_SYSTEM`): reply
  `{"run":"<cmd>"}` or `{"answer":"<text>"}`. A `run` is executed and the **real output** returned;
  an `answer` is printed (markdown→ANSI in User mode). Headline path: `tokex "list all rust
  projects"`.
- `<known-category>: text` (`Category`) or a JSON object (`Json`) → a **structured answer** using
  that category's header (`plan-stack`, `theme`, …). These aren't runnable commands.
- a lone token → a command.

**Execution of a chosen command** (`run_command` → `exec_capture`): commands are generated for the
**native shell** — PowerShell on Windows, bash on Unix (the decision prompt is OS-specific) — and run
by writing the command to a temp script (`.ps1`/`.sh`, with `$ErrorActionPreference='Stop'`/`set -e`
so errors get a non-zero exit) and invoking the interpreter on it *by path* via rtk (raw). This
avoids cmd.exe's pipe/quoting mangling and cross-shell mount mismatches. **Safe (read-only) commands
run unprompted**; only a **risky** one (`is_risky`: delete/overwrite/install/push/network/sudo, POSIX
*and* PowerShell cmdlets) is confirmed (default No). On failure the model gets the error and **fixes
the command or answers** from it, up to `MAX_FIXES` (2).

Each **category** binds a name to a *header* in the `CATEGORIES` table — **add a category by adding a
row**. Prompts require an LLM key.

**Roles** (`tokex <role> "<task>"`, e.g. `tokex planner "…"`, `tokex coder "…"`) pick a
**role-specific model** for the same decide-run-or-answer flow, so a calling agent offloads work and
just waits. The `ROLES` table binds each role to `(model id, header)` — `planner` (glm),
`router`/`orchestrator` (nemotron nano/ultra), `coder` (deepseek), `assistant` (qwen, the default).
Same endpoint + key as the configured LLM, role's model id swapped in. A role wins dispatch when
it's the first arg.

Two modes (`prompt::Mode`): **User** (default) shows a stderr spinner until the first token then
streams the model's thinking live; **Model** (`tokex -m "…"`, for agents) shows neither — just the
output on stdout (task exec runs with `footer:false`, human channel suppressed).

## Scripting (`script.rs`)

**Repetitive or multi-file change? Write a script, don't edit each file.** For things like renaming
a token across many files, the agent writes ONE idempotent script under `Scripts/` (created if
missing) and runs `tokex script Scripts/<name>.sh`. tokex runs it through rtk (never raw — `rtk run
-c "bash …"`, picked by extension: `.sh`/`.ps1`/`.py`), then runs `git diff --stat` so the change is
**verified from the diff, not by re-reading files**. Exit code is the script's; success = it ran.
tokex does not generate the script — the agent does; tokex provides the instruction, run, and verify.
`tokex script` with no file just creates `Scripts/` and prints the workflow. (`git diff` only shows
tracked modifications — new/untracked files show via `git status`.)

## Out of scope (deferred, do not add speculatively)

(nothing currently deferred)

## Commit & attribution rules (must follow)

- **Never** put the word "Codex" in a branch name or commit message.
- **Never** add an AI co-author or attribution trailer (`Co-Authored-By: Codex …`,
  "Generated with…") to commits or PRs.
- **Real-time commits (must follow):** after changing a file, commit that change immediately.
  Don't batch unrelated edits into one commit or leave the tree dirty between steps — one logical
  change, one commit, right away.
- Concise subject line; short body explaining *why* when it isn't obvious.

## Branch & PR workflow (must follow)

**Never push to `main`.** Every change ships through a PR:

1. Branch off `main` with a fresh, descriptive name. **Never** put "Codex" in a branch name.
2. Make changes there (real-time commits still apply — commit each logical change immediately).
3. `gh pr create` to open a PR.
4. **Wait for the `CI` workflow to pass** (`.github/workflows/ci.yml` builds + tests `tokex`). Do not
   merge on red.
5. Merge once green (`gh pr merge --squash --delete-branch`).
6. Clean up and sync: `git checkout main && git pull`, and delete the local branch.
