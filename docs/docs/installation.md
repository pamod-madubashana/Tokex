---
id: installation
title: Installation
---

# Installation

## Prebuilt binaries

Grab the archive for your platform from [Downloads](downloads). Each archive contains **both**
`tokex` and `rtk` — keep them in the same directory (Tokex looks for `rtk` next to its own binary).

## Build from source

Requires a Rust toolchain. RTK and graphify are vendored as git submodules under `vendor/`, so
clone recursively:

```bash
git clone --recursive https://github.com/pamod-madubashana/Tokex
# or, in an existing clone:
git submodule update --init --recursive

cargo build --release
```

`cargo build` builds both `tokex` and the vendored `rtk` into `target/release/` (the first release
build is slow — rtk compiles a full tree including bundled SQLite).

Put `target/release` on your `PATH`, or copy `tokex` + `rtk` together to a directory that is.

:::note
Tokex spawns `rtk`. It prefers an `rtk` binary sitting next to its own executable and falls back to
`rtk` on your `PATH`. If you move `tokex`, move `rtk` with it.
:::

Next: [Setup](setup).
