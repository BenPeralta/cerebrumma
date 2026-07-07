use crate::cli::ConfigAction;
use crate::memory::{active_brain, global_brain_path, load_episodes};
use anyhow::Result;
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub enum LLMProvider {
    #[default]
    Gemini,
    Anthropic,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub provider: LLMProvider,
    pub gemini_key: Option<String>,
    pub anthropic_key: Option<String>,
}

pub fn run_config(action: ConfigAction) -> Result<()> {
    let config_path = global_brain_path().join("config.json");
    let mut config = if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Config::default()
    };

    match action {
        ConfigAction::Set { key, value } => {
            match key.to_lowercase().as_str() {
                "provider" => {
                    config.provider = match value.to_lowercase().as_str() {
                        "gemini" => LLMProvider::Gemini,
                        "anthropic" => LLMProvider::Anthropic,
                        _ => {
                            return Err(anyhow::anyhow!(
                                "Unsupported provider: {}. Use 'gemini' or 'anthropic'.",
                                value
                            ))
                        }
                    };
                }
                "gemini_key" => config.gemini_key = Some(value),
                "anthropic_key" => config.anthropic_key = Some(value),
                _ => return Err(anyhow::anyhow!("Unknown config key: {}", key)),
            }
            fs::create_dir_all(config_path.parent().unwrap())?;
            fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
            println!("{} Config updated: {} set.", "✓".green(), key);
        }
        ConfigAction::Get => {
            println!("{} Current Configuration:", "→".bright_magenta());
            println!("  Active Provider: {:?}", config.provider);
            println!(
                "  Gemini Key: {}",
                if config.gemini_key.is_some() {
                    "******** (set)"
                } else {
                    "not set"
                }
            );
            println!(
                "  Anthropic Key: {}",
                if config.anthropic_key.is_some() {
                    "******** (set)"
                } else {
                    "not set"
                }
            );
        }
    }
    Ok(())
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

    println!(
        "\n{} {} Brain Health & Usage:",
        "🧠".bright_cyan(),
        "Cerebrumma".bold()
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

    println!("\n  {} Intelligence Synthesis", "✨".dimmed());
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
