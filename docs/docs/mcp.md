---
id: mcp
title: MCP Server
---

# MCP Server

Agents that speak [MCP](https://modelcontextprotocol.io) can call Tokex natively instead of shelling
out. Tokex runs as an MCP server over stdio and exposes a `run` tool that returns structured
execution events.

```bash
tokex mcp        # JSON-RPC 2.0 over stdio
```

## Register with Claude Code

```bash
claude mcp add tokex -- /absolute/path/to/tokex mcp
```

## The `run` tool

Input:

```json
{ "command": "cargo test", "llm": false }
```

- `command` (string, required) — the command line.
- `llm` (boolean, optional) — compress output into an insight for this call.

Result content is the normalized event list — the same machine channel as the CLI, delivered as a
tool result:

```json
{
  "events": [
    { "type": "stdout", "line": "Compiling tokex v0.1.0", "severity": "info" },
    { "type": "result", "status": "ok", "code": 0 }
  ]
}
```

`isError` is set when the command exits non-zero.

## The `set_agent` tool

tokex installs a [graphify](architecture) code-map skill for the agent in use. When it can't detect
which agent that is (no terminal to prompt, nothing in config or the environment), a `run` result
includes a note asking the model to identify itself. The model then calls `set_agent`:

```json
{ "agent": "codex" }
```

tokex persists the platform and installs the graphify skill for it in the background. This is the
no-TTY equivalent of the interactive agent prompt — the model tells tokex what it's running in.

## Protocol notes

Tokex implements a focused subset of MCP: `initialize`, `tools/list`, `tools/call`, and `ping`, over
newline-delimited JSON-RPC 2.0 on stdio. In MCP mode **stdout is the protocol channel** — execution
output is captured internally and only returned inside the tool result.

Next: [Architecture](architecture).
