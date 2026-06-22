---
id: setup
title: Setup
---

# Setup

Configure Tokex *after* install with one interactive command — no file editing:

```bash
tokex setup
```

It prompts for:

| Prompt | Options |
| --- | --- |
| **Provider** | Groq · OpenRouter · NVIDIA NIM (presets fill URL + a default model) · Custom |
| **API key** | masked input from any free OpenAI-compatible provider |
| **Compression** | `heuristic` (rtk filter, default) · `llm` (rtk + AI insight) · `off` (raw) |
| **RTK output** | `normal` · `ultra-compact` |

Settings are written to your OS config dir — **not** the repo:

- Windows: `%APPDATA%\tokex\config.toml`
- Linux: `~/.config/tokex/config.toml`
- macOS: `~/Library/Application Support/tokex/config.toml`

## Overrides

`TOKEX_LLM_URL`, `TOKEX_LLM_KEY`, and `TOKEX_LLM_MODEL` environment variables override the config
file — handy for CI. `tokex run --llm …` forces the LLM insight on for a single run regardless of
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
