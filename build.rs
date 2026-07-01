//! Build script for cotrex: embed the RTK and graphify binaries if available.
//!
//! When RTK/graphify are built before cotrex (e.g. in the release workflow), their binaries
//! are embedded directly into the cotrex executable. At runtime, cotrex extracts them on first use.
//!
//! If binaries aren't found at build time, cotrex falls back to external resolution.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Register custom cfg names to suppress warnings
    println!("cargo::rustc-check-cfg=cfg(rtk_embedded)");
    println!("cargo::rustc-check-cfg=cfg(rtk_not_embedded)");
    println!("cargo::rustc-check-cfg=cfg(graphify_embedded)");
    println!("cargo::rustc-check-cfg=cfg(graphify_not_embedded)");

    // Only embed on release builds to avoid bloating debug builds
    let profile = env::var("PROFILE").unwrap_or_default();
    if profile != "release" {
        println!("cargo:rustc-cfg=rtk_not_embedded");
        println!("cargo:rustc-cfg=graphify_not_embedded");
        return;
    }

    // Find the workspace root (parent of cotrex package dir)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir;

    // === RTK embedding ===
    let rtk_name = if cfg!(windows) { "rtk.exe" } else { "rtk" };
    let rtk_search_paths = [
        workspace_root.join("target/release").join(rtk_name),
        workspace_root
            .join("target/x86_64-pc-windows-msvc/release")
            .join(rtk_name),
        workspace_root
            .join("target/x86_64-unknown-linux-musl/release")
            .join(rtk_name),
        workspace_root
            .join("target/aarch64-apple-darwin/release")
            .join(rtk_name),
    ];
    let rtk_path = rtk_search_paths.iter().find(|p| p.is_file());

    if let Some(path) = rtk_path {
        println!("cargo:warning=Embedding RTK binary from {}", path.display());
        println!("cargo:rustc-env=RTK_BINARY_PATH={}", path.display());
        println!("cargo:rustc-cfg=rtk_embedded");

        if let Ok(version) = env::var("RTK_VERSION") {
            println!("cargo:rustc-env=RTK_VERSION={version}");
        } else if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                let secs = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                println!("cargo:rustc-env=RTK_VERSION={secs}");
            }
        }
    } else {
        println!("cargo:warning=RTK binary not found at build time; will use external RTK");
        println!("cargo:rustc-cfg=rtk_not_embedded");
    }

    if let Some(path) = rtk_path {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    // === Graphify embedding ===
    let graphify_name = if cfg!(windows) {
        "graphify.exe"
    } else {
        "graphify"
    };
    let graphify_search_paths = [
        workspace_root.join("target/release").join(graphify_name),
        workspace_root
            .join("target/x86_64-pc-windows-msvc/release")
            .join(graphify_name),
        workspace_root
            .join("target/x86_64-unknown-linux-musl/release")
            .join(graphify_name),
        workspace_root
            .join("target/aarch64-apple-darwin/release")
            .join(graphify_name),
    ];
    let graphify_path = graphify_search_paths.iter().find(|p| p.is_file());

    if let Some(path) = graphify_path {
        println!(
            "cargo:warning=Embedding graphify binary from {}",
            path.display()
        );
        println!("cargo:rustc-env=GRAPHIFY_BINARY_PATH={}", path.display());
        println!("cargo:rustc-cfg=graphify_embedded");

        if let Ok(version) = env::var("GRAPHIFY_VERSION") {
            println!("cargo:rustc-env=GRAPHIFY_VERSION={version}");
        } else if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                let secs = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                println!("cargo:rustc-env=GRAPHIFY_VERSION={secs}");
            }
        }
    } else {
        println!(
            "cargo:warning=graphify binary not found at build time; will use external graphify"
        );
        println!("cargo:rustc-cfg=graphify_not_embedded");
    }

    if let Some(path) = graphify_path {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
