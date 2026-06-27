---
id: setup
title: Setup
---

# Setup

Configure Cotrex *after* install with one interactive command — no file editing:

```bash
cotrex setup
```

It prompts for:

| Prompt | Options |
| --- | --- |
| **Provider** | Groq · OpenRouter · NVIDIA NIM (presets fill URL + a default model) · Custom |
| **API key** | masked input from any free OpenAI-compatible provider |
| **Compression** | `heuristic` (rtk filter, default) · `llm` (rtk + AI insight) · `off` (raw) |
| **RTK output** | `normal` · `ultra-compact` |
| **Graph auto-update** | keep the graphify code map fresh after code changes (default yes) |
| **Agent** | your graphify platform (`claude`, `codex`, `cursor`, …; blank = auto-detect) |

When graph auto-update is on, `cotrex setup` also installs graphifyy and registers the graphify skill
for your agent, then builds the map — so the assistant just reads it. See
[Architecture](architecture) for details.

Settings are written to your OS config dir — **not** the repo:

- Windows: `%APPDATA%\cotrex\config.toml`
- Linux: `~/.config/cotrex/config.toml`
- macOS: `~/Library/Application Support/cotrex/config.toml`

## Overrides

`COTREX_LLM_URL`, `COTREX_LLM_KEY`, and `COTREX_LLM_MODEL` environment variables override the config
file — handy for CI. `cotrex run --llm …` forces the LLM insight on for a single run regardless of
the configured compression mode.

## Modes

- **Compression**
  - `off` — raw output (`rtk run -c`), no filtering.
  - `heuristic` — rtk's filtered output (default).
  - `llm` — rtk filtering plus an AI insight (`{status, root_cause, important_errors, suggested_fix}`).
- **RTK output**
  - `normal`
  - `ultra-compact` — appends `--ultra-compact` to rtk for the tersest output.

Next: [Usage](usage).
