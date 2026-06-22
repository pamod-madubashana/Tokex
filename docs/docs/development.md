---
id: development
title: Development
---

# Development

```bash
cargo build                       # build tokex + vendored rtk (keep it warning-clean)
cargo test                        # run the self-checks
cargo test native_command_maps    # run a single test by name
```

## Contributing workflow

Changes never land directly on `main`:

1. Branch off `main` (descriptive name).
2. Commit each logical change as you go.
3. Open a PR (`gh pr create`).
4. Wait for the `CI` workflow (builds + tests `tokex`) to pass.
5. Squash-merge, delete the branch, and sync `main`.

## Layout

| Path | Role |
| --- | --- |
| `src/intent.rs` | normalized intent + command → rtk mapping |
| `src/orchestrate.rs` | spawn rtk, stream, normalize |
| `src/normalize.rs` | line → `{type, line, severity}` |
| `src/llm.rs` | optional LLM compression |
| `src/config.rs` | user config + `tokex setup` |
| `src/mcp.rs` | MCP stdio server |
| `src/main.rs` | CLI dispatch |
| `vendor/rtk` | pinned RTK submodule |
| `docs/` | this site (Docusaurus) |
