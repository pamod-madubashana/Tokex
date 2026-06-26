//! Persistent config + the interactive `tokex setup` flow.
//!
//! The key lives in the user's config dir (e.g. %APPDATA%\tokex\config.toml), set *after* install
//! via `tokex setup` — not a project `.env`. Env vars still override for power users / CI.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub provider: String,
    pub llm_url: String,
    pub llm_key: String,
    pub llm_model: String,
    /// off | heuristic | llm — default compression for command output.
    pub compression: String,
    /// normal | ultra-compact — rtk output verbosity.
    pub rtk_verbosity: String,
    /// Keep the graphify code map fresh automatically after code-changing runs.
    pub graph_auto: bool,
    /// graphify platform id for skill registration (e.g. claude, codex, cursor). Blank = auto-detect.
    pub agent: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            provider: String::new(),
            llm_url: String::new(),
            llm_key: String::new(),
            llm_model: String::new(),
            compression: "heuristic".into(),
            rtk_verbosity: "normal".into(),
            graph_auto: true,
            agent: String::new(),
        }
    }
}

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tokex").join("config.toml"))
}

/// Load config from disk (or defaults), then apply env overrides.
pub fn load() -> Config {
    let mut cfg = config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str::<Config>(&s).ok())
        .unwrap_or_default();
    if let Ok(v) = std::env::var("TOKEX_LLM_URL") {
        cfg.llm_url = v;
    }
    if let Ok(v) = std::env::var("TOKEX_LLM_KEY") {
        cfg.llm_key = v;
    }
    if let Ok(v) = std::env::var("TOKEX_LLM_MODEL") {
        cfg.llm_model = v;
    }
    if let Ok(v) = std::env::var("TOKEX_COMPRESSION") {
        cfg.compression = v;
    }
    if let Ok(v) = std::env::var("TOKEX_RTK_VERBOSITY") {
        cfg.rtk_verbosity = v;
    }
    if let Ok(v) = std::env::var("TOKEX_GRAPH_AUTO") {
        cfg.graph_auto = v == "true" || v == "1" || v == "yes";
    }
    cfg
}

pub fn save(cfg: &Config) -> Result<PathBuf, String> {
    let path = config_path().ok_or("cannot determine config dir")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let s = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, s).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Interactive setup. Pretty prompts via `inquire`; writes the config file.
pub fn run_setup() -> Result<(), String> {
    use inquire::{Password, PasswordDisplayMode, Select, Text};

    let llm_key = Password::new("NVIDIA NIM API key")
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation()
        .prompt()
        .map_err(|e| e.to_string())?;

    let compression = Select::new(
        "Default compression",
        vec![
            "heuristic (rtk filter)",
            "llm (rtk + AI insight)",
            "off (raw output)",
        ],
    )
    .prompt()
    .map_err(|e| e.to_string())?
    .split_whitespace()
    .next()
    .unwrap()
    .to_string();

    let rtk_verbosity = Select::new("RTK output", vec!["normal", "ultra-compact"])
        .prompt()
        .map_err(|e| e.to_string())?
        .to_string();

    let graph_auto = inquire::Confirm::new("Auto-update the graphify code map after code changes?")
        .with_default(true)
        .prompt()
        .map_err(|e| e.to_string())?;

    let agent = if graph_auto {
        let choice = Select::new(
            "Agent for graphify skill",
            vec![
                "opencode",
                "claude",
                "codex",
                "cursor",
                "gemini",
                "windsurf",
                "aider",
                "continue",
                "cline",
                "custom (type your own)",
                "auto-detect",
            ],
        )
        .prompt()
        .map_err(|e| e.to_string())?;

        match choice {
            "custom (type your own)" => Text::new("Agent name")
                .prompt()
                .map_err(|e| e.to_string())?
                .trim()
                .to_string(),
            "auto-detect" => String::new(),
            other => other.to_string(),
        }
    } else {
        String::new()
    };

    let cfg = Config {
        provider: "NVIDIA NIM".into(),
        llm_url: "https://integrate.api.nvidia.com/v1/chat/completions".into(),
        llm_key,
        llm_model: "meta/llama-3.1-8b-instruct".into(),
        compression,
        rtk_verbosity,
        graph_auto,
        agent,
    };
    let path = save(&cfg)?;
    eprintln!("Saved config to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_through_toml() {
        let cfg = Config {
            provider: "Groq".into(),
            llm_url: "https://x/y".into(),
            llm_key: "secret".into(),
            llm_model: "m".into(),
            compression: "llm".into(),
            rtk_verbosity: "ultra-compact".into(),
            graph_auto: true,
            agent: "codex".into(),
        };
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn defaults_are_safe() {
        let c = Config::default();
        assert_eq!(c.compression, "heuristic");
        assert!(c.llm_key.is_empty());
    }
}
