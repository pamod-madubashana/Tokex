---
id: development
title: Development
---

# Development

## Get the source

`rtk` and `graphify` are pinned git submodules under `vendor/`, so clone **recursively**:

```bash
git clone --recursive https://github.com/pamod-madubashana/Cotrex
cd Cotrex

# already cloned without --recursive? pull the submodules in:
git submodule update --init --recursive
```

You need a [Rust toolchain](https://rustup.rs) (and a C compiler — the vendored rtk builds bundled
SQLite).

## Build & test

```bash
cargo build                       # builds cotrex + vendored rtk into target/ (first run is slow)
cargo test                        # run the self-checks
cargo test native_command_maps    # run a single test by name
cargo run -- run "git status"     # try the CLI
```

`cargo build` produces `cotrex` and `rtk` side by side in `target/<profile>/`, so the dev binary
finds rtk with no extra step.

## Contributing workflow

Changes never land directly on `main`:

1. Branch off `main` (descriptive name).
2. Commit each logical change as you go.
3. Open a PR (`gh pr create`).
4. Wait for the `CI` workflow (builds + tests `cotrex`) to pass.
5. Squash-merge, delete the branch, and sync `main`.

## Layout

| Path | Role |
| --- | --- |
| `src/intent.rs` | normalized intent + command → rtk mapping |
| `src/orchestrate.rs` | spawn rtk, stream, normalize |
| `src/normalize.rs` | line → `{type, line, severity}` |
| `src/llm.rs` | optional LLM compression |
| `src/config.rs` | user config + `cotrex setup` |
| `src/mcp.rs` | MCP stdio server |
| `src/main.rs` | CLI dispatch |
| `vendor/rtk` | pinned RTK submodule |
| `docs/` | this site (Docusaurus) |
