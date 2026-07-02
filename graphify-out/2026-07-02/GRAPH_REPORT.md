# Graph Report - MVP  (2026-07-02)

## Corpus Check
- 33 files · ~26,017 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 426 nodes · 922 edges · 24 communities
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `f83d56b8`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_prompt.rs|prompt.rs]]
- [[_COMMUNITY_Result_|Result_]]
- [[_COMMUNITY_mcp.rs|mcp.rs]]
- [[_COMMUNITY_dispatch.rs|dispatch.rs]]
- [[_COMMUNITY_intent.rs|intent.rs]]
- [[_COMMUNITY_mod.rs|mod.rs]]
- [[_COMMUNITY_install.rs|install.rs]]
- [[_COMMUNITY_permission.rs|permission.rs]]
- [[_COMMUNITY_tool.rs|tool.rs]]
- [[_COMMUNITY_install_agent.rs|install_agent.rs]]
- [[_COMMUNITY_update.rs|update.rs]]
- [[_COMMUNITY_CLAUDE|CLAUDE.md]]
- [[_COMMUNITY_LlmConfig|LlmConfig]]
- [[_COMMUNITY_embedded.rs|embedded.rs]]
- [[_COMMUNITY_embedded_graphify.rs|embedded_graphify.rs]]
- [[_COMMUNITY_README|README.md]]
- [[_COMMUNITY_script.rs|script.rs]]
- [[_COMMUNITY_AGENTS|AGENTS.md]]
- [[_COMMUNITY_normalize.rs|normalize.rs]]

## God Nodes (most connected - your core abstractions)
1. `Result_` - 48 edges
2. `tools_call()` - 19 edges
3. `Config` - 17 edges
4. `current_project_dir()` - 16 edges
5. `fulfill()` - 15 edges
6. `tool_error()` - 14 edges
7. `run_graphify_capture()` - 13 edges
8. `TailView` - 12 edges
9. `Intent` - 12 edges
10. `LlmConfig` - 12 edges

## Surprising Connections (you probably didn't know these)
- `run()` --calls--> `normalize()`  [INFERRED]
  src/core/orchestrate.rs → src/core/normalize.rs
- `dispatch_one()` --references--> `Mode`  [EXTRACTED]
  src/dispatch/dispatch.rs → src/agent/prompt.rs
- `fulfill()` --references--> `Mode`  [EXTRACTED]
  src/dispatch/dispatch.rs → src/agent/prompt.rs
- `run_prompt()` --references--> `Mode`  [EXTRACTED]
  src/dispatch/dispatch.rs → src/agent/prompt.rs
- `run_role()` --references--> `Mode`  [EXTRACTED]
  src/dispatch/dispatch.rs → src/agent/prompt.rs

## Import Cycles
- None detected.

## Communities (24 total, 0 thin omitted)

### Community 0 - "prompt.rs"
Cohesion: 0.08
Nodes (47): Arc, AtomicBool, Drop, Instant, JoinHandle, Response, build_tree_fallback(), build_tree_from_files() (+39 more)

### Community 1 - "Result_"
Cohesion: 0.14
Nodes (41): Box, FnOnce, Result_, add_url(), auto_update(), bootstrap_detached(), clear_skill_marker(), cluster_only() (+33 more)

### Community 2 - "mcp.rs"
Cohesion: 0.11
Nodes (38): Default, Config, config_path(), defaults_are_safe(), load(), Option, PathBuf, Self (+30 more)

### Community 3 - "dispatch.rs"
Cohesion: 0.09
Nodes (31): Cli, Cmd, Cmd, Option, Intent, Msg, Option, String (+23 more)

### Community 4 - "intent.rs"
Cohesion: 0.13
Nodes (16): Into, Item, Iterator, cli_and_json_agree(), default_action(), default_tool(), gh_pr_create_maps_direct(), has_shell_operators() (+8 more)

### Community 5 - "mod.rs"
Cohesion: 0.19
Nodes (23): MutexGuard, bytes_to_tokens(), chrono_now(), footer(), footer_contains_token_counts(), footer_shows_failed_status(), format_cost(), get_global() (+15 more)

### Community 6 - "install.rs"
Cohesion: 0.16
Nodes (20): ProgressBar, download_with_progress(), format_bytes(), Path, String, spinner(), asset_for(), asset_name() (+12 more)

### Community 7 - "permission.rs"
Cohesion: 0.20
Nodes (11): HashMap, Action, default_permissions_allow_read_tools(), default_permissions_ask_on_write_tools(), is_command_risky(), Permissions, Option, Self (+3 more)

### Community 8 - "tool.rs"
Cohesion: 0.24
Nodes (13): Regex, builtins(), resolve_path(), Path, PathBuf, String, Value, Vec (+5 more)

### Community 9 - "install_agent.rs"
Cohesion: 0.26
Nodes (14): agent_skills_dir(), cotrex_skill(), current_project_dir(), graphify_skill(), inject_agents_md_rules(), install_agent(), is_project_dir(), list_installed() (+6 more)

### Community 10 - "update.rs"
Cohesion: 0.26
Nodes (14): asset_for(), cleanup_old_backups(), current_exe_path(), current_version(), download_release(), fetch_latest_tag(), find_bin(), is_newer() (+6 more)

### Community 11 - "CLAUDE.md"
Cohesion: 0.14
Nodes (12): Architecture, Branch & PR workflow (must follow), Commands, Commit & attribution rules (must follow), Config & modes (`cotrex setup`), Getting rtk, graphify code map (`graphify.rs`), Invariants (+4 more)

### Community 12 - "LlmConfig"
Cohesion: 0.23
Nodes (10): with_model(), compress(), Insight, LlmConfig, parse_insight(), parses_fenced_json(), Option, Self (+2 more)

### Community 13 - "embedded.rs"
Cohesion: 0.38
Nodes (9): embedded_rtk_path(), embedded_rtk_path_is_deterministic(), extract_rtk(), is_embedded(), is_embedded_matches_cfg(), marker_path(), Option, PathBuf (+1 more)

### Community 14 - "embedded_graphify.rs"
Cohesion: 0.38
Nodes (9): embedded_graphify_path(), embedded_graphify_path_is_deterministic(), extract_graphify(), graphify_version(), is_embedded(), is_embedded_matches_cfg(), marker_path(), Option (+1 more)

### Community 15 - "README.md"
Cohesion: 0.20
Nodes (9): Ask a question, Installation, License, Manual install, Quick install (recommended), Run a command, Setup, Usage (+1 more)

### Community 16 - "script.rs"
Cohesion: 0.33
Nodes (7): ensure_dir(), exec_command(), PathBuf, String, Write, run(), scripts_dir()

### Community 17 - "AGENTS.md"
Cohesion: 0.22
Nodes (7): Build & Test, Commit Rules, Conventions, Core Contract, Module Map, RULE 0: USE COTREX — NO EXCEPTIONS, RULE 1: GRAPHIFY FIRST

### Community 18 - "normalize.rs"
Cohesion: 0.46
Nodes (6): classify(), LineEvent, normalize(), normalize_keeps_line_verbatim(), String, Severity

## Knowledge Gaps
- **29 isolated node(s):** `Cmd`, `Msg`, `Cmd`, `RULE 0: USE COTREX — NO EXCEPTIONS`, `RULE 1: GRAPHIFY FIRST` (+24 more)
  These have ≤1 connection - possible missing edges or undocumented components.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Result_` connect `Result_` to `prompt.rs`, `mcp.rs`, `dispatch.rs`, `intent.rs`, `install.rs`, `tool.rs`, `install_agent.rs`, `update.rs`, `LlmConfig`, `script.rs`?**
  _High betweenness centrality (0.342) - this node is a cross-community bridge._
- **Why does `Config` connect `mcp.rs` to `dispatch.rs`, `LlmConfig`?**
  _High betweenness centrality (0.051) - this node is a cross-community bridge._
- **Why does `dispatch()` connect `mcp.rs` to `Result_`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **What connects `Cmd`, `Msg`, `Cmd` to the rest of the system?**
  _29 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `prompt.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07720782654680064 - nodes in this community are weakly interconnected._
- **Should `Result_` be split into smaller, more focused modules?**
  _Cohesion score 0.13623188405797101 - nodes in this community are weakly interconnected._
- **Should `mcp.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10909090909090909 - nodes in this community are weakly interconnected._