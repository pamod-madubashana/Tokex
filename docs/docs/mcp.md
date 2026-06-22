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

## Protocol notes

Tokex implements a focused subset of MCP: `initialize`, `tools/list`, `tools/call`, and `ping`, over
newline-delimited JSON-RPC 2.0 on stdio. In MCP mode **stdout is the protocol channel** — execution
output is captured internally and only returned inside the tool result.

Next: [Architecture](architecture).
