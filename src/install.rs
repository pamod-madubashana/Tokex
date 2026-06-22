//! Fetch the latest `rtk` release for the current platform, so Tokex ships standalone and pulls
//! its execution backend on demand. Extraction shells out to the system `tar` (bsdtar on Windows
//! handles .zip; tar auto-detects gzip on unix) — ponytail: no archive crates for this.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// rtk release pinned to the version this build of Tokex was tested against. Bump deliberately —
/// never `latest`, so a breaking rtk release can't silently break every install.
const RTK_VERSION: &str = "v0.42.4";

fn download_url(asset: &str) -> String {
    format!("https://github.com/rtk-ai/rtk/releases/download/{RTK_VERSION}/{asset}")
}

pub fn rtk_bin_name() -> &'static str {
    if cfg!(windows) {
        "rtk.exe"
    } else {
        "rtk"
    }
}

/// Where a downloaded rtk lives: `<data_dir>/tokex/rtk[.exe]`.
pub fn rtk_install_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("tokex").join(rtk_bin_name()))
}

/// Is `rtk` resolvable on PATH? (Checks for the binary file; doesn't spawn it.)
fn on_path() -> bool {
    let name = rtk_bin_name();
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

/// Resolve rtk, downloading it automatically if it isn't already present. Order:
/// next to our own binary → tokex data dir → PATH → download the pinned release.
pub fn ensure_rtk() -> Result<PathBuf, String> {
    let name = rtk_bin_name();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(c) = exe.parent().map(|d| d.join(name)) {
            if c.is_file() {
                return Ok(c);
            }
        }
    }
    if let Some(c) = rtk_install_path() {
        if c.is_file() {
            return Ok(c);
        }
    }
    if on_path() {
        return Ok(PathBuf::from("rtk"));
    }
    eprintln!("rtk not found — installing it automatically …");
    install()
}

/// RTK release asset for an OS/arch pair (matches rtk-ai/rtk's release naming).
fn asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("rtk-x86_64-pc-windows-msvc.zip"),
        ("macos", "aarch64") => Some("rtk-aarch64-apple-darwin.tar.gz"),
        ("macos", "x86_64") => Some("rtk-x86_64-apple-darwin.tar.gz"),
        ("linux", "x86_64") => Some("rtk-x86_64-unknown-linux-musl.tar.gz"),
        ("linux", "aarch64") => Some("rtk-aarch64-unknown-linux-gnu.tar.gz"),
        _ => None,
    }
}

fn asset_name() -> Result<&'static str, String> {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    asset_for(os, arch).ok_or_else(|| format!("no prebuilt rtk for {os}/{arch}; install rtk manually"))
}

/// Download + extract the latest rtk and install it into the data dir. Returns the install path.
pub fn install() -> Result<PathBuf, String> {
    let asset = asset_name()?;
    let url = download_url(asset);
    eprintln!("Downloading rtk {RTK_VERSION} ({asset}) …");

    let tmp = std::env::temp_dir().join(format!("tokex-rtk-{}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let archive = tmp.join(asset);

    // Download (ureq follows the GitHub release redirects to the CDN).
    let resp = ureq::get(&url)
        .set("User-Agent", "tokex")
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let mut reader = resp.into_reader();
    let mut file = fs::File::create(&archive).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    drop(file);

    // Extract in place.
    let status = Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&tmp)
        .status()
        .map_err(|e| format!("`tar` not available for extraction: {e}"))?;
    if !status.success() {
        return Err("extraction failed".into());
    }

    let bin = find_bin(&tmp, rtk_bin_name()).ok_or("rtk binary not found in archive")?;
    let dest = rtk_install_path().ok_or("cannot determine data dir")?;
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::copy(&bin, &dest).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755)).ok();
    }
    let _ = fs::remove_dir_all(&tmp);
    Ok(dest)
}

/// Recursively find a file named `name` under `dir` (archives may nest the binary in a folder).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_platforms_map_to_assets() {
        assert_eq!(asset_for("windows", "x86_64"), Some("rtk-x86_64-pc-windows-msvc.zip"));
        assert_eq!(asset_for("macos", "aarch64"), Some("rtk-aarch64-apple-darwin.tar.gz"));
        assert_eq!(asset_for("macos", "x86_64"), Some("rtk-x86_64-apple-darwin.tar.gz"));
        assert_eq!(asset_for("linux", "x86_64"), Some("rtk-x86_64-unknown-linux-musl.tar.gz"));
        assert_eq!(asset_for("linux", "aarch64"), Some("rtk-aarch64-unknown-linux-gnu.tar.gz"));
    }

    #[test]
    fn unsupported_platform_is_none() {
        assert_eq!(asset_for("redox", "sparc"), None);
    }

    #[test]
    fn url_is_pinned_not_latest() {
        let u = download_url("rtk-x86_64-pc-windows-msvc.zip");
        assert!(u.contains("/releases/download/v0.42.4/"));
        assert!(!u.contains("latest"));
    }

    #[test]
    fn current_platform_is_supported() {
        assert!(asset_name().is_ok());
    }
}
