//! Shared download helpers with progress bars and spinners.
//!
//! Provides `download_with_progress()` for showing a colorful progress bar during file downloads,
//! and `spinner()` for showing a spinning indicator during extraction or other blocking steps.
//! Style inspired by pip/npm install output.

use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

/// Format bytes as human-readable string (e.g. "12.3 MB").
#[allow(dead_code)]
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Download a URL to a file with a colorful progress bar.
///
/// Shows a pip/npm-style progress bar with bytes downloaded, speed, and ETA.
pub fn download_with_progress(url: &str, dest: &Path) -> Result<(), String> {
    let resp = ureq::get(url)
        .set("User-Agent", "cotrex")
        .call()
        .map_err(|e| format!("download failed: {e}"))?;

    let total: u64 = resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let pb = if total > 0 {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::with_template(
                "  {bar:30.cyan/dim}  {cyan}{bytes:>12}{cyan}/{total_bytes:<12}{cyan}  {green}{bytes_per_sec:>10}{green}  {yellow}ETA {eta}{yellow}",
            )
            .expect("invalid progress template")
            .progress_chars("█▓░"),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        pb
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template(
                "  {cyan}{bytes:>12}{cyan} downloaded  {green}{bytes_per_sec:>10}{green}",
            )
            .expect("invalid spinner template"),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        pb
    };

    let mut reader = resp.into_reader();
    let mut file = fs::File::create(dest).map_err(|e| format!("cannot create file: {e}"))?;
    let mut buf = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("read error: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("write error: {e}"))?;
        pb.inc(n as u64);
    }

    pb.finish_and_clear();
    Ok(())
}

/// Create a spinner for long-running operations (extraction, etc.).
///
/// Returns a `ProgressBar` in spinner mode. Call `.finish()` when done.
pub fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.green} {msg}").expect("invalid spinner template"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}
