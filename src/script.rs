//! Scripting workflow.
//!
//! For a repetitive or multi-file change (rename a token across the repo, the same edit in several
//! files), the agent writes ONE idempotent script under `Scripts/` instead of editing each file by
//! hand. `tokex script <file>` runs it through rtk and then shows `git diff` — the change is verified
//! from the diff, not by re-reading every file. tokex itself never generates the script (the agent
//! does); it only provides the instruction, the run, and the verify.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::intent::Intent;
use crate::orchestrate::{self, Options};

pub const INSTRUCTIONS: &str = "\
Scripts/ is ready. For a repetitive or multi-file change, don't edit files one by one:
  1. Write an idempotent script to Scripts/<name>.sh (or .ps1 / .py), operating from the repo root.
  2. Run it:  tokex script Scripts/<name>.sh
tokex runs the script through rtk, then shows `git diff` — verify the change from the diff instead of
re-reading every file.";

fn scripts_dir() -> PathBuf {
    PathBuf::from("Scripts")
}

/// Ensure the `Scripts/` folder exists; return its path.
pub fn ensure_dir() -> std::io::Result<PathBuf> {
    let d = scripts_dir();
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// The shell command that executes `file`, chosen by extension. Defaults to bash.
fn exec_command(file: &str) -> String {
    let lower = file.to_ascii_lowercase();
    if lower.ends_with(".ps1") {
        format!("powershell -ExecutionPolicy Bypass -File {file}")
    } else if lower.ends_with(".py") {
        format!("python {file}")
    } else {
        format!("bash {file}")
    }
}

/// Run a script through rtk and verify with `git diff`. Returns the script's exit code.
pub fn run(
    file: &str,
    out: &mut impl Write,
    err: &mut impl Write,
    opts: &Options,
) -> Result<i32, String> {
    ensure_dir().map_err(|e| format!("cannot create Scripts/: {e}"))?;
    if !Path::new(file).exists() {
        writeln!(err, "{INSTRUCTIONS}").ok();
        writeln!(err, "\nNo script at {file} yet — write it there, then rerun.").ok();
        return Ok(1);
    }

    // Run the script through rtk (tokex never executes raw — orchestrate spawns `rtk run -c`).
    // No LLM here: a script is verified by its diff, not by a model insight.
    let code = orchestrate::run(&Intent::from_command(exec_command(file)), out, err, None, opts)?;

    // Verify: show what changed. The agent reads the diff, not the files.
    // Use fixed options, not the caller's: `--ultra-compact` would leak through to `git diff`
    // (rtk doesn't filter `diff`) and error out.
    // ponytail: a plain `git diff` includes any pre-existing uncommitted changes too; commit or
    // stash first if you need the script's changes in isolation.
    writeln!(err, "— verify (git diff) —").ok();
    let verify_opts =
        Options { raw: false, ultra_compact: false, llm_on_failure: false, footer: true };
    orchestrate::run(&Intent::from_command("git diff --stat"), out, err, None, &verify_opts)?;

    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_command_picks_runner_by_extension() {
        assert_eq!(exec_command("Scripts/x.sh"), "bash Scripts/x.sh");
        assert_eq!(exec_command("Scripts/x.PS1"), "powershell -ExecutionPolicy Bypass -File Scripts/x.PS1");
        assert_eq!(exec_command("Scripts/x.py"), "python Scripts/x.py");
        assert_eq!(exec_command("Scripts/noext"), "bash Scripts/noext");
    }
}
