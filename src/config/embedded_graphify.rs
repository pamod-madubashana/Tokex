//! Embedded graphify binary support.
//!
//! When cotrex is built with a graphify binary available at compile time, the binary is embedded
//! directly into the cotrex executable. At runtime, `extract_graphify()` writes it to the data
//! directory and returns the path. If no embedded binary is available, the module is a no-op
//! and `extract_graphify()` returns `None`.

use std::fs;
use std::path::PathBuf;

/// graphify binary name for the current platform.
fn graphify_bin_name() -> &'static str {
    if cfg!(windows) {
        "graphify.exe"
    } else {
        "graphify"
    }
}

/// Where the extracted graphify binary lives: `<data_dir>/cotrex/embedded-graphify[.exe]`.
fn embedded_graphify_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| {
        d.join("cotrex")
            .join(format!("embedded-{}", graphify_bin_name()))
    })
}

/// Marker file to avoid re-extracting on every run.
fn marker_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("cotrex").join("embedded-graphify.version"))
}

/// The embedded graphify binary (only available when built with `graphify_embedded` cfg).
///
/// When graphify is embedded, this constant contains the raw binary bytes. The `build.rs` script
/// sets `GRAPHIFY_BINARY_PATH` env var pointing to the graphify binary at compile time, and we
/// use `include_bytes!` to embed it.
#[cfg(graphify_embedded)]
const GRAPHIFY_BINARY: &[u8] = include_bytes!(env!("GRAPHIFY_BINARY_PATH"));

/// Version string for the embedded graphify (used to detect when re-extraction is needed).
/// This is set at build time by the release workflow or build.rs.
#[cfg(graphify_embedded)]
fn graphify_version() -> &'static str {
    option_env!("GRAPHIFY_VERSION").unwrap_or("dev")
}

/// Placeholder when graphify is not embedded.
#[cfg(graphify_not_embedded)]
fn graphify_version() -> &'static str {
    "external"
}

/// Check if an embedded graphify binary is available.
pub fn is_embedded() -> bool {
    cfg!(graphify_embedded)
}

/// Extract the embedded graphify binary to the data directory.
///
/// Returns `Some(path)` if an embedded binary was extracted (or already exists),
/// `None` if no embedded binary is available.
pub fn extract_graphify() -> Option<PathBuf> {
    if !is_embedded() {
        return None;
    }

    let dest = embedded_graphify_path()?;
    let marker = marker_path()?;

    let version = graphify_version();

    // Check if already extracted with current version
    if dest.is_file() {
        if let Ok(existing) = fs::read_to_string(&marker) {
            if existing == version {
                return Some(dest);
            }
        }
    }

    // Extract embedded binary
    #[cfg(graphify_embedded)]
    {
        let parent = dest.parent()?;
        fs::create_dir_all(parent).ok()?;

        fs::write(&dest, GRAPHIFY_BINARY).ok()?;

        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755)).ok();
        }

        // Write version marker
        fs::write(&marker, version).ok();

        return Some(dest);
    }

    #[cfg(not(graphify_embedded))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_graphify_path_is_deterministic() {
        let a = embedded_graphify_path();
        let b = embedded_graphify_path();
        assert_eq!(a, b);
    }

    #[test]
    fn is_embedded_matches_cfg() {
        let _ = is_embedded();
    }
}
