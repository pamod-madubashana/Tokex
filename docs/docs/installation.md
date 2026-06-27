---
id: installation
title: Installation
---

# Installation

Two ways to install Cotrex — let your agent do it, or install manually. Either way, Cotrex
**downloads its `rtk` backend automatically** on first run, so there's nothing else to set up.

## Option A — Install with your agent

Paste this one-line prompt to your coding agent (Claude Code, Cursor, Codex, …):

```text
Install Cotrex on my machine: download the latest release for my OS/arch from
https://github.com/pamod-madubashana/Cotrex/releases/latest, extract the `cotrex` binary, put it on
my PATH, and confirm with `cotrex --version`. It fetches its rtk backend automatically on first run.
```

The agent figures out your platform, downloads the right archive, and puts `cotrex` on your PATH.

## Option B — Manual

1. Download the archive for your platform from the
   [**Releases**](https://github.com/pamod-madubashana/Cotrex/releases/latest) page (see
   [Downloads](downloads) for the file names).
2. Extract it. You get `cotrex` (`cotrex.exe` on Windows).
3. Put it on your `PATH` — copy it into a directory that's already on `PATH`, or add its folder.
4. Confirm:

   ```bash
   cotrex --version
   ```

That's it. The first time you run a command, Cotrex fetches the matching `rtk` release into its data
dir automatically. (The release archive also bundles `rtk`, so if you keep them together no download
is needed.)

:::note
Cotrex resolves `rtk` in order: next to its own binary → its data dir → your `PATH` → otherwise it
downloads the pinned release. Run `cotrex install-rtk` to pre-fetch it (handy for offline or CI).
:::

Next: [Setup](setup) to add your LLM provider and pick modes. Building from source instead? See
[Development](development).
