use crate::cli::ConfigAction;
use crate::memory::{active_brain, global_brain_path, load_episodes};
use anyhow::{bail, Result};
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.5-flash-lite";
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-5-5";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub enum LLMProvider {
    #[default]
    Gemini,
    Anthropic,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub provider: LLMProvider,
    pub gemini_key: Option<String>,
    pub anthropic_key: Option<String>,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    #[serde(default = "default_anthropic_model")]
    pub anthropic_model: String,
}

fn default_gemini_model() -> String {
    DEFAULT_GEMINI_MODEL.to_string()
}

fn default_anthropic_model() -> String {
    DEFAULT_ANTHROPIC_MODEL.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: LLMProvider::Gemini,
            gemini_key: None,
            anthropic_key: None,
            gemini_model: default_gemini_model(),
            anthropic_model: default_anthropic_model(),
        }
    }
}

pub fn llm_config_path() -> std::path::PathBuf {
    global_brain_path().join("config.json")
}

pub fn load_llm_config() -> Config {
    let Ok(content) = fs::read_to_string(llm_config_path()) else {
        return Config::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Model ids are interpolated into request URLs. Keep them to a single token.
pub fn validate_model_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && !id.contains("..")
        && !id.starts_with('.')
        && !id.ends_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !ok {
        bail!("invalid model id '{id}'");
    }
    Ok(())
}

pub fn effective_model(config: &Config) -> Result<String> {
    let raw = match config.provider {
        LLMProvider::Gemini => {
            let trimmed = config.gemini_model.trim();
            if trimmed.is_empty() {
                DEFAULT_GEMINI_MODEL
            } else {
                trimmed
            }
        }
        LLMProvider::Anthropic => {
            let trimmed = config.anthropic_model.trim();
            if trimmed.is_empty() {
                DEFAULT_ANTHROPIC_MODEL
            } else {
                trimmed
            }
        }
    };
    validate_model_id(raw)?;
    Ok(raw.to_string())
}

pub fn resolve_api_key(config: &Config) -> Result<String> {
    match config.provider {
        LLMProvider::Gemini => first_env_or_stored(
            &["GEMINI_API_KEY", "CEREBRUMMA_GEMINI_KEY"],
            config.gemini_key.as_deref(),
            "Gemini",
            "gemini_key",
        ),
        LLMProvider::Anthropic => first_env_or_stored(
            &["ANTHROPIC_API_KEY", "CEREBRUMMA_ANTHROPIC_KEY"],
            config.anthropic_key.as_deref(),
            "Anthropic",
            "anthropic_key",
        ),
    }
}

fn first_env_or_stored(
    env_keys: &[&str],
    stored: Option<&str>,
    label: &str,
    config_key: &str,
) -> Result<String> {
    for key in env_keys {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Ok(value);
            }
        }
    }
    if let Some(value) = stored.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(value.to_string());
    }
    bail!(
        "{label} API key not found. Set {} or run: cerebrum config set {config_key}",
        env_keys[0]
    )
}

fn read_secret(env_keys: &[&str], prompt: &str) -> Result<String> {
    for key in env_keys {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Ok(value);
            }
        }
    }

    if !io::stdin().is_terminal() {
        bail!(
            "no {} in the environment and stdin is not a terminal. Set {} and retry.",
            prompt,
            env_keys[0]
        );
    }

    eprint!("{prompt}: ");
    io::stderr().flush().ok();
    let _guard = EchoOff::enable();
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    eprintln!();
    let line = line.trim().to_string();
    if line.is_empty() {
        bail!("empty API key");
    }
    Ok(line)
}

struct EchoOff;

impl EchoOff {
    fn enable() -> Self {
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("stty").arg("-echo").status();
        }
        Self
    }
}

impl Drop for EchoOff {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("stty").arg("echo").status();
        }
    }
}

fn write_llm_config(path: &Path, config: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(config)? + "\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn run_config(action: ConfigAction) -> Result<()> {
    let config_path = llm_config_path();
    let mut config = load_llm_config();

    match action {
        ConfigAction::Set { key, value } => {
            match key.to_lowercase().as_str() {
                "provider" => {
                    let value = require_value(value)?;
                    config.provider = match value.to_lowercase().as_str() {
                        "gemini" => LLMProvider::Gemini,
                        "anthropic" => LLMProvider::Anthropic,
                        _ => {
                            bail!("Unsupported provider: {value}. Use 'gemini' or 'anthropic'.")
                        }
                    };
                }
                "gemini_model" => {
                    let value = require_value(value)?;
                    validate_model_id(&value)?;
                    config.gemini_model = value;
                }
                "anthropic_model" => {
                    let value = require_value(value)?;
                    validate_model_id(&value)?;
                    config.anthropic_model = value;
                }
                "gemini_key" => {
                    refuse_cli_secret(value.is_some(), "GEMINI_API_KEY")?;
                    config.gemini_key = Some(read_secret(
                        &["GEMINI_API_KEY", "CEREBRUMMA_GEMINI_KEY"],
                        "Gemini API key",
                    )?);
                }
                "anthropic_key" => {
                    refuse_cli_secret(value.is_some(), "ANTHROPIC_API_KEY")?;
                    config.anthropic_key = Some(read_secret(
                        &["ANTHROPIC_API_KEY", "CEREBRUMMA_ANTHROPIC_KEY"],
                        "Anthropic API key",
                    )?);
                }
                "api_key" => {
                    bail!(
                        "Unknown config key: api_key. Use gemini_key or anthropic_key (read from the environment or a prompt, not the command line)."
                    );
                }
                _ => bail!("Unknown config key: {key}"),
            }
            write_llm_config(&config_path, &config)?;
            println!("{} Config updated: {} set.", "✓".green(), key);
        }
        ConfigAction::Get => {
            println!("{} Current Configuration:", "→".bright_magenta());
            println!("  Active Provider: {:?}", config.provider);
            println!(
                "  Gemini Model: {}",
                effective_or_raw(&config.gemini_model, DEFAULT_GEMINI_MODEL)
            );
            println!(
                "  Anthropic Model: {}",
                effective_or_raw(&config.anthropic_model, DEFAULT_ANTHROPIC_MODEL)
            );
            println!(
                "  Gemini Key: {}",
                if config.gemini_key.as_deref().is_some_and(|k| !k.is_empty()) {
                    "******** (set)"
                } else {
                    "not set"
                }
            );
            println!(
                "  Anthropic Key: {}",
                if config
                    .anthropic_key
                    .as_deref()
                    .is_some_and(|k| !k.is_empty())
                {
                    "******** (set)"
                } else {
                    "not set"
                }
            );
        }
    }
    Ok(())
}

fn require_value(value: Option<String>) -> Result<String> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => bail!("missing value"),
    }
}

fn refuse_cli_secret(passed: bool, env_key: &str) -> Result<()> {
    if passed {
        bail!(
            "Refusing to read an API key from the command line (it would land in shell history). Set {env_key} or run the command without a value to be prompted."
        );
    }
    Ok(())
}

fn effective_or_raw<'a>(value: &'a str, default: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default
    } else {
        trimmed
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Stats {
    pub total_dreams: u64,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
    pub last_dream_at: Option<String>,
}

pub fn run_stats() -> Result<()> {
    let stats_path = global_brain_path().join("stats.json");
    let stats: Stats = if stats_path.exists() {
        let content = fs::read_to_string(&stats_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Stats::default()
    };

    let brain = active_brain().ok_or_else(|| anyhow::anyhow!("No Brain found."))?;
    let episodes = load_episodes(&brain.join("memory/episodic")).len();
    let semantic = fs::read_dir(brain.join("memory/semantic"))
        .map(|d| d.count())
        .unwrap_or(0);
    let rules = fs::read_dir(brain.join("memory/procedural/skills"))
        .map(|d| d.count())
        .unwrap_or(0);

    let label = if brain == global_brain_path() {
        "global ~/.cerebrum/".to_string()
    } else {
        format!("local {}", brain.display())
    };
    println!(
        "\n{} {} Brain Health & Usage {}",
        "🧠".bright_cyan(),
        "Cerebrumma".bold(),
        format!("({label})").dimmed()
    );

    println!("\n  {} Layers", "📂".dimmed());
    println!(
        "    Semantic:   {} insights",
        semantic.to_string().bright_green()
    );
    println!(
        "    Procedural: {} skills/rules",
        rules.to_string().bright_yellow()
    );
    println!(
        "    Episodic:   {} pending entries",
        episodes.to_string().bright_magenta()
    );

    println!(
        "\n  {} Intelligence Synthesis {}",
        "✨".dimmed(),
        "(all projects)".dimmed()
    );
    println!(
        "    Total Dreams:   {}",
        stats.total_dreams.to_string().bright_white()
    );
    println!(
        "    Est. Tokens:    {} (In) / {} (Out)",
        stats.total_tokens_input.to_string().dimmed(),
        stats.total_tokens_output.to_string().dimmed()
    );
    println!(
        "    Last Evolution: {}",
        stats.last_dream_at.as_deref().unwrap_or("Never")
    );

    let ratio = if stats.total_dreams > 0 {
        semantic as f64 / stats.total_dreams as f64
    } else {
        0.0
    };
    let health_icon = if ratio > 2.0 {
        "🟢"
    } else if ratio > 0.5 {
        "🟡"
    } else {
        "🔴"
    };

    println!(
        "\n  {} Status: {} {:.1} insights per dream",
        health_icon,
        "Neural Density:".bold(),
        ratio
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_fields_use_current_defaults() {
        let config: Config = serde_json::from_str(r#"{"provider":"Anthropic"}"#).unwrap();
        assert!(matches!(config.provider, LLMProvider::Anthropic));
        assert_eq!(config.gemini_model, DEFAULT_GEMINI_MODEL);
        assert_eq!(config.anthropic_model, DEFAULT_ANTHROPIC_MODEL);
        assert_eq!(effective_model(&config).unwrap(), DEFAULT_ANTHROPIC_MODEL);
    }

    #[test]
    fn rejects_model_ids_that_could_escape_the_url() {
        assert!(validate_model_id("claude-sonnet-5-5").is_ok());
        assert!(validate_model_id("gemini-3.5-flash-lite").is_ok());
        assert!(validate_model_id("../admin").is_err());
        assert!(validate_model_id("model?key=secret").is_err());
        assert!(validate_model_id("").is_err());
        assert!(validate_model_id(".hidden").is_err());
    }

    #[test]
    fn stored_key_is_used_when_present() {
        let config = Config {
            gemini_key: Some(" stored ".to_string()),
            ..Config::default()
        };
        // Only assert the stored path when the environment is not already set,
        // so a developer shell cannot make this test depend on a real key.
        if std::env::var("GEMINI_API_KEY").is_ok() || std::env::var("CEREBRUMMA_GEMINI_KEY").is_ok()
        {
            return;
        }
        assert_eq!(resolve_api_key(&config).unwrap(), "stored");
    }
}
