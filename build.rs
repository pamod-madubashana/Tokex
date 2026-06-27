//! Build script for cotrex: embed the RTK binary if available.
//!
//! When RTK is built before cotrex (e.g. in the release workflow), its binary is embedded
//! directly into the cotrex executable. At runtime, cotrex extracts it on first use.
//!
//! If RTK isn't found at build time, cotrex falls back to external resolution (PATH or download).

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Register custom cfg names to suppress warnings
    println!("cargo::rustc-check-cfg=cfg(rtk_embedded)");
    println!("cargo::rustc-check-cfg=cfg(rtk_not_embedded)");

    // Only embed on release builds to avoid bloating debug builds
    let profile = env::var("PROFILE").unwrap_or_default();
    if profile != "release" {
        println!("cargo:rustc-cfg=rtk_not_embedded");
        return;
    }

    // Find the workspace root (parent of cotrex package dir)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir;

    // RTK binary name depends on platform
    let rtk_name = if cfg!(windows) { "rtk.exe" } else { "rtk" };

    // Check multiple locations for the RTK binary
    let search_paths = [
        // Same directory as cotrex binary (workspace build)
        workspace_root.join("target/release").join(rtk_name),
        // Windows cross-compile target
        workspace_root.join("target/x86_64-pc-windows-msvc/release").join(rtk_name),
        // Linux target
        workspace_root
            .join("target/x86_64-unknown-linux-musl/release")
            .join(rtk_name),
        // macOS ARM target
        workspace_root
            .join("target/aarch64-apple-darwin/release")
            .join(rtk_name),
    ];

    let rtk_path = search_paths.iter().find(|p| p.is_file());

    if let Some(path) = rtk_path {
        // Embed the RTK binary
        println!(
            "cargo:warning=Embedding RTK binary from {}",
            path.display()
        );
        println!("cargo:rustc-env=RTK_BINARY_PATH={}", path.display());
        println!("cargo:rustc-cfg=rtk_embedded");

        // Set RTK_VERSION for the embedded module (use a hash of the binary as version)
        // In release builds, this is set via the workflow; fallback to file modification time
        if let Ok(version) = env::var("RTK_VERSION") {
            println!("cargo:rustc-env=RTK_VERSION={version}");
        } else {
            // Use file modification time as version for cache invalidation
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    let secs = modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    println!("cargo:rustc-env=RTK_VERSION={secs}");
                }
            }
        }
    } else {
        // RTK not found - fall back to external resolution
        println!("cargo:warning=RTK binary not found at build time; will use external RTK");
        println!("cargo:rustc-cfg=rtk_not_embedded");
    }

    // Rebuild if RTK binary changes
    if let Some(path) = rtk_path {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
