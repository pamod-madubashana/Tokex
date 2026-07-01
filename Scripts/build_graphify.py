#!/usr/bin/env python3
"""
Build graphify as a standalone executable using PyInstaller.

Usage:
    python Scripts/build_graphify.py

Output:
    target/release/graphify[.exe]

Requirements:
    pip install pyinstaller
    pip install graphifyy (or uv tool install graphifyy)
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).parent.parent
VENDOR_DIR = ROOT_DIR / 'vendor' / 'graphify'
SPEC_FILE = ROOT_DIR / 'Scripts' / 'graphify.spec'
TARGET_DIR = ROOT_DIR / 'target' / 'release'


def main():
    # Ensure PyInstaller is installed
    try:
        import PyInstaller
        print(f"PyInstaller version: {PyInstaller.__version__}")
    except ImportError:
        print("PyInstaller not found. Installing...")
        subprocess.run([sys.executable, '-m', 'pip', 'install', 'pyinstaller'], check=True)

    # Ensure graphify package is installed
    try:
        import graphify
        print(f"graphify package found: {getattr(graphify, '__version__', 'unknown')}")
    except ImportError:
        print("graphify package not found. Installing...")
        subprocess.run([sys.executable, '-m', 'pip', 'install', 'graphifyy'], check=True)

    # Clean previous builds
    dist_dir = ROOT_DIR / 'dist'
    build_dir = ROOT_DIR / 'build'
    if dist_dir.exists():
        shutil.rmtree(dist_dir)
    if build_dir.exists():
        shutil.rmtree(build_dir)

    # Run PyInstaller
    print("Building graphify executable...")
    result = subprocess.run(
        [
            sys.executable, '-m', 'PyInstaller',
            '--clean',
            '--noconfirm',
            '--specpath', str(ROOT_DIR / 'Scripts'),
            '--distpath', str(dist_dir),
            '--workpath', str(build_dir),
            str(SPEC_FILE),
        ],
        cwd=str(ROOT_DIR),
    )

    if result.returncode != 0:
        print("PyInstaller build failed!")
        sys.exit(1)

    # Find the built executable
    graphify_name = 'graphify.exe' if sys.platform == 'win32' else 'graphify'
    built_exe = dist_dir / graphify_name

    if not built_exe.exists():
        print(f"Expected executable not found at {built_exe}")
        sys.exit(1)

    # Copy to target/release
    TARGET_DIR.mkdir(parents=True, exist_ok=True)
    dest = TARGET_DIR / graphify_name
    shutil.copy2(built_exe, dest)
    print(f"Built graphify executable: {dest}")
    print(f"Size: {dest.stat().st_size / (1024 * 1024):.1f} MB")

    # Clean up
    shutil.rmtree(dist_dir, ignore_errors=True)
    shutil.rmtree(build_dir, ignore_errors=True)

    return 0


if __name__ == '__main__':
    sys.exit(main())
