#!/usr/bin/env python
"""CLAUDE.md is the single source of truth; AGENTS.md is a byte-for-byte copy of it.

Edit CLAUDE.md, then run:  tokex script Scripts/sync-docs.py
ponytail: plain copy, no transform — keeps CLAUDE.md's exact bytes (incl. line endings).
"""
import shutil
from pathlib import Path

root = Path(__file__).resolve().parent.parent
shutil.copyfile(root / "CLAUDE.md", root / "AGENTS.md")
print("synced AGENTS.md from CLAUDE.md")
