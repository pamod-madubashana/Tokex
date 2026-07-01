# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec file for graphify standalone executable
# Build with: pyinstaller Scripts/graphify.spec

import os
import sys
from pathlib import Path

# Paths
SPEC_DIR = Path(SPECPATH)
ROOT_DIR = SPEC_DIR.parent
VENDOR_DIR = ROOT_DIR / 'vendor' / 'graphify'

a = Analysis(
    [str(VENDOR_DIR / 'graphify' / '__main__.py')],
    pathex=[str(VENDOR_DIR)],
    binaries=[],
    datas=[
        (str(VENDOR_DIR / 'graphify' / 'always_on'), 'graphify/always_on'),
        (str(VENDOR_DIR / 'graphify' / 'extractors'), 'graphify/extractors'),
        (str(VENDOR_DIR / 'graphify' / 'skills'), 'graphify/skills'),
    ],
    hiddenimports=[
        'graphify',
        'graphify.__main__',
        'graphify.extract',
        'graphify.build',
        'graphify.cluster',
        'graphify.report',
        'graphify.detect',
        'graphify.ingest',
        'graphify.export',
        'networkx',
        'numpy',
        'rapidfuzz',
        'tree_sitter',
        'tree_sitter_python',
        'tree_sitter_javascript',
        'tree_sitter_typescript',
        'tree_sitter_go',
        'tree_sitter_rust',
        'tree_sitter_java',
        'tree_sitter_c',
        'tree_sitter_cpp',
        'tree_sitter_ruby',
        'tree_sitter_c_sharp',
        'tree_sitter_kotlin',
        'tree_sitter_scala',
        'tree_sitter_php',
        'tree_sitter_swift',
        'tree_sitter_lua',
        'tree_sitter_zig',
        'tree_sitter_powershell',
        'tree_sitter_elixir',
        'tree_sitter_objc',
        'tree_sitter_julia',
        'tree_sitter_verilog',
        'tree_sitter_fortran',
        'tree_sitter_bash',
        'tree_sitter_json',
        'tree_sitter_groovy',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'tkinter',
        'matplotlib',
        'PIL',
        'pytest',
        'unittest',
        'torch',
        'torchvision',
        'transformers',
        'tensorflow',
        'pandas',
        'scipy',
        'sympy',
        'IPython',
        'jupyter',
    ],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='graphify',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
