---
name: tokex
description: "Tokex RTK orchestration skills for MVP. Run commands, inspect projects, and get normalized output."
---

# Tokex Skills

## Available commands

### Run a command
```bash
tokex run "cargo test"
tokex cargo test          # shorthand
```

### Show project structure
```bash
tokex "show the project tree"
tokex "give me the directory layout"
```

### Install skills for this project
```bash
tokex install opencode     # reinstall/update skills
```

## Installed for: MVP

Skills are in `.tokex/skills/`. Your agent detects them automatically.
