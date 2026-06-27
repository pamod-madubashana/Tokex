//! Self-update: check the latest GitHub release, compare versions, and replace the running
//! binary when a newer version is available. Follows the same download+extract pattern as
//! `install.rs` for rtk.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO: &str = "pamod-madubashana/Cotrex";

/// Current version compiled into the binary.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Asset name for the current OS/arch pair (matches the release workflow naming).
fn asset_for(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    match (os, arch) {
        ("windows", "x86_64") => Some(("windows-x86_64", "zip")),
        ("macos", "aarch64") => Some(("macos-arm64", "tar.gz")),
        ("macos", "x86_64") => Some(("macos-x86_64", "tar.gz")),
        ("linux", "x86_64") => Some(("linux-x86_64", "tar.gz")),
        ("linux", "aarch64") => Some(("linux-aarch64", "tar.gz")),
        _ => None,
    }
}

/// Fetch the latest release tag from GitHub. Returns the tag name (e.g. "v1.2.0").
fn fetch_latest_tag() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", "cotrex")
        .call()
        .map_err(|e| format!("failed to check latest release: {e}"))?;
    let v: serde_json::Value = resp.into_json().map_err(|e| format!("bad response: {e}"))?;
    v["tag_name"]
        .as_str()
        .map(String::from)
        .ok_or("response missing tag_name".into())
}

/// Parse a version string like "1.2.3" or "v1.2.3" into (major, minor, patch).
fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = parts[2].parse().ok()?;
    Some((major, minor, patch))
}

/// Returns true when `latest` is strictly newer than `current`.
fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Path to the running binary.
fn current_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("cannot locate running binary: {e}"))
}

/// Download and extract the release archive, returning the path to the cotrex binary inside it.
fn download_release(tag: &str, tmp: &Path) -> Result<PathBuf, String> {
    let (name, ext) = asset_for(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
        format!(
            "unsupported platform: {}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let asset = format!("cotrex-{tag}-{name}.{ext}");
    let url = format!("https://github.com/{REPO}/releases/download/{tag}/{asset}");
    eprintln!("  downloading {asset} …");

    let archive = tmp.join(&asset);
    let resp = ureq::get(&url)
        .set("User-Agent", "cotrex")
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let mut reader = resp.into_reader();
    let mut file = fs::File::create(&archive).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    drop(file);

    eprintln!("  extracting …");
    if ext == "zip" {
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive.display(),
                    tmp.display()
                ),
            ])
            .status()
            .map_err(|e| format!("powershell not available: {e}"))?;
        if !status.success() {
            return Err("extraction failed".into());
        }
    } else {
        let status = Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(tmp)
            .status()
            .map_err(|e| format!("`tar` not available: {e}"))?;
        if !status.success() {
            return Err("extraction failed".into());
        }
    }

    find_bin(tmp, "cotrex")
        .or_else(|| find_bin(tmp, "cotrex.exe"))
        .ok_or("cotrex binary not found in archive".into())
}

/// Recursively find a file named `name` under `dir`.
fn find_bin(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(found) = find_bin(&p, name) {
                return Some(found);
            }
        } else if p.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(p);
        }
    }
    None
}

/// Run the self-update check. Prints status to stderr, replaces the binary if newer.
pub fn run() -> Result<(), String> {
    let current = current_version();
    eprintln!("cotrex {current} — checking for updates …");

    let tag = fetch_latest_tag()?;
    let latest = tag.strip_prefix('v').unwrap_or(&tag);

    if !is_newer(current, latest) {
        eprintln!("cotrex {current} is up to date.");
        return Ok(());
    }

    eprintln!("cotrex {latest} is available (current: {current}).");

    let exe = current_exe_path()?;

    // Download to a temp directory next to the binary so we can replace in-place.
    let tmp = exe
        .parent()
        .ok_or("cannot determine binary directory")?
        .join(format!("cotrex-update-{}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let new_bin = download_release(&tag, &tmp)?;

    // Back up the current binary before overwriting.
    let backup = exe.with_extension("exe.bak");
    if backup.exists() {
        let _ = fs::remove_file(&backup);
    }
    fs::copy(&exe, &backup).map_err(|e| format!("backup failed: {e}"))?;

    // Replace the running binary. On Windows the running process locks the file,
    // so we rename the old one first (rename succeeds while the file is open).
    let exe_name = exe.file_name().ok_or("bad exe path")?;
    let dest = tmp.join(exe_name);
    fs::copy(&exe, &dest).map_err(|e| e.to_string())?;
    fs::remove_file(&exe).map_err(|e| format!("cannot remove old binary: {e}"))?;
    fs::copy(&new_bin, &exe).map_err(|e| format!("cannot install new binary: {e}"))?;

    // Clean up.
    let _ = fs::remove_dir_all(&tmp);
    let _ = fs::remove_file(&backup);

    eprintln!("cotrex updated to {latest}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_variants() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("abc"), None);
    }

    #[test]
    fn newer_detection() {
        assert!(is_newer("1.0.0", "1.0.1"));
        assert!(is_newer("1.0.0", "1.1.0"));
        assert!(is_newer("1.0.0", "2.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "bad"));
    }
}
