//! Embedded RTK binary support.
//!
//! When cotrex is built with an RTK binary available at compile time, the binary is embedded
//! directly into the cotrex executable. At runtime, `extract_rtk()` writes it to the data
//! directory and returns the path. If no embedded binary is available, the module is a no-op
//! and `extract_rtk()` returns `None`.

use std::fs;
use std::path::PathBuf;

/// RTK binary name for the current platform.
fn rtk_bin_name() -> &'static str {
    if cfg!(windows) {
        "rtk.exe"
    } else {
        "rtk"
    }
}

/// Where the extracted RTK binary lives: `<data_dir>/cotrex/embedded-rtk[.exe]`.
fn embedded_rtk_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("cotrex").join(format!("embedded-{}", rtk_bin_name())))
}

/// Marker file to avoid re-extracting on every run.
fn marker_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("cotrex").join("embedded-rtk.version"))
}

/// The embedded RTK binary (only available when built with `rtk_embedded` cfg).
///
/// When RTK is embedded, this constant contains the raw binary bytes. The `build.rs` script
/// sets `RTK_BINARY_PATH` env var pointing to the RTK binary at compile time, and we use
/// `include_bytes!` to embed it.
#[cfg(rtk_embedded)]
const RTK_BINARY: &[u8] = include_bytes!(env!("RTK_BINARY_PATH"));

/// Version string for the embedded RTK (used to detect when re-extraction is needed).
/// This is set at build time by the release workflow or build.rs.
#[cfg(rtk_embedded)]
fn rtk_version() -> &'static str {
    // RTK_VERSION is set by build.rs from env or file modification time
    option_env!("RTK_VERSION").unwrap_or("dev")
}

/// Placeholder when RTK is not embedded.
#[cfg(rtk_not_embedded)]
fn rtk_version() -> &'static str {
    "external"
}

/// Check if an embedded RTK binary is available.
pub fn is_embedded() -> bool {
    cfg!(rtk_embedded)
}

/// Extract the embedded RTK binary to the data directory.
///
/// Returns `Some(path)` if an embedded binary was extracted (or already exists),
/// `None` if no embedded binary is available.
pub fn extract_rtk() -> Option<PathBuf> {
    if !is_embedded() {
        return None;
    }

    let dest = embedded_rtk_path()?;
    let marker = marker_path()?;

    let version = rtk_version();

    // Check if already extracted with current version
    if dest.is_file() {
        if let Ok(existing) = fs::read_to_string(&marker) {
            if existing == version {
                return Some(dest);
            }
        }
    }

    // Extract embedded binary
    #[cfg(rtk_embedded)]
    {
        let parent = dest.parent()?;
        fs::create_dir_all(parent).ok()?;

        fs::write(&dest, RTK_BINARY).ok()?;

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

    #[cfg(not(rtk_embedded))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rtk_path_is_deterministic() {
        let a = embedded_rtk_path();
        let b = embedded_rtk_path();
        assert_eq!(a, b);
    }

    #[test]
    fn is_embedded_matches_cfg() {
        // This test just verifies the function compiles and returns a value
        let _ = is_embedded();
    }
}
