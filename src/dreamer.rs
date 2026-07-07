use anyhow::{Context, Result};
use chrono::Utc;
use colored::*;
use std::fs;
use std::path::PathBuf;

use crate::config::{Config, LLMProvider, Stats};
use crate::memory::{
    active_brain, global_brain_path, local_brain_path, load_episodes, ts_stem, write_entry, Entry,
};
use crate::vector::store_vector;

pub fn build_reflection_prompt(episodes: &[(PathBuf, String)]) -> String {
    let body = episodes
        .iter()
        .take(20)
        .map(|(path, text)| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            format!("### {name}\n{text}")
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    format!(
        r#"You are an expert memory curator for Cerebrumma.

Review these recent episodic memories and produce a structured reflection.
Return EXACTLY these four sections (keep the headers verbatim):

## Key Insights
- (3-7 concise bullet points of patterns or facts worth keeping long-term)

## New Rules
- (extract concrete best practices or protocols to follow going forward)

## Equity & Bias Notes
- (any language, assumptions, or patterns to avoid or reinforce for fairness)

## Prune Suggestions
- (filenames or entries that are low-value and can be archived)

---
## Episodic Memories

{body}
"#
    )
}

pub async fn dream_cycle(ingest: Option<PathBuf>, auto: bool) -> Result<()> {
    // ── Ingest mode: parse LLM reflection and promote ──────────────────────────
    if let Some(ref ingest_path) = ingest {
        return ingest_reflection(ingest_path);
    }

    if auto {
        return run_auto_dream().await;
    }

    // ── Generate mode: load episodes + write prompt ────────────────────────────
    println!("{}", "Dream cycle starting...".bright_magenta().bold());

    let global = global_brain_path();
    let local = local_brain_path();

    let mut all_entries: Vec<(PathBuf, String)> = vec![];
    all_entries.extend(load_episodes(&local));
    all_entries.extend(load_episodes(&global));

    if all_entries.is_empty() {
        println!("   {} No new episodic entries to reflect on.", "→".dimmed());
        println!(
            "   {} Add some with {}.",
            "→".dimmed(),
            "cerebrum add \"...\"".yellow()
        );
        return Ok(());
    }

    println!(
        "   {} {} episodic entries loaded",
        "→".dimmed(),
        all_entries.len()
    );

    let stem = ts_stem(&Utc::now().to_rfc3339());

    // Write the reflection prompt
    let dream_dir = active_brain()
        .ok_or_else(|| anyhow::anyhow!("No Brain found."))?
        .join("dream");
    fs::create_dir_all(&dream_dir)?;
    let prompt_path = dream_dir.join(format!("{stem}-reflection-prompt.md"));
    fs::write(&prompt_path, build_reflection_prompt(&all_entries))?;

    // Archive processed episodic entries
    let mut archived = 0;
    for (path, _) in &all_entries {
        let stem_name = path.file_stem().unwrap_or_default().to_string_lossy();
        let new_path = path
            .parent()
            .unwrap()
            .join(format!("{stem_name}-dreamed.md"));
        fs::rename(path, new_path)?;
        archived += 1;
    }

    println!("   {} {} entries archived", "→".dimmed(), archived);
    println!(
        "   {} Reflection prompt → {}",
        "→".dimmed(),
        prompt_path.display()
    );
    println!();
    println!("{}", "Next steps:".bold());
    println!(
        "   1. Open {} and paste it into Claude/Grok",
        prompt_path.display()
    );
    println!("   2. Save the LLM's response as a .md file");
    println!(
        "   3. Run: {}",
        format!("cerebrum dream --ingest <response.md>").yellow()
    );

    Ok(())
}

pub async fn run_auto_dream() -> Result<()> {
    println!("{} Starting Autopilot Dream Cycle...", "→".bright_magenta());

    // 1. Get Config
    let config_path = global_brain_path().join("config.json");
    let config: Config = if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        return Err(anyhow::anyhow!(
            "No config found. Run: cerebrum config set <key> <value>"
        ));
    };

    // 2. Load episodes
    let global = global_brain_path();
    let local = local_brain_path();
    let mut all_entries: Vec<(PathBuf, String)> = vec![];
    all_entries.extend(load_episodes(&local));
    all_entries.extend(load_episodes(&global));

    if all_entries.is_empty() {
        println!("   {} No new episodic entries to reflect on.", "→".dimmed());
        return Ok(());
    }

    // 3. Generate Prompt
    let prompt = build_reflection_prompt(&all_entries);

    // 4. Call Provider
    println!("{} Calling {:?} for synthesis...", "→".dimmed(), config.provider);
    let client = reqwest::Client::new();

    let reflection = match config.provider {
        LLMProvider::Gemini => {
            let api_key = config.gemini_key.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Gemini API key not found. Run: cerebrum config set gemini_key <KEY>")
            })?;

            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}", api_key);
            let payload = serde_json::json!({
                "contents": [{
                    "parts": [{
                        "text": prompt
                    }]
                }]
            });
            let res = client.post(url).json(&payload).send().await?;
            if !res.status().is_success() {
                let err_text = res.text().await?;
                return Err(anyhow::anyhow!("Gemini API error: {}", err_text));
            }
            let json: serde_json::Value = res.json().await?;
            json["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Failed to parse Gemini response"))?
                .to_string()
        }
        LLMProvider::Anthropic => {
            let api_key = config.anthropic_key.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Anthropic API key not found. Run: cerebrum config set anthropic_key <KEY>"
                )
            })?;

            let url = "https://api.anthropic.com/v1/messages";
            let payload = serde_json::json!({
                "model": "claude-3-5-sonnet-20241022",
                "max_tokens": 4096,
                "messages": [{
                    "role": "user",
                    "content": prompt
                }]
            });
            let res = client
                .post(url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&payload)
                .send()
                .await?;
            if !res.status().is_success() {
                let err_text = res.text().await?;
                return Err(anyhow::anyhow!("Anthropic API error: {}", err_text));
            }
            let json: serde_json::Value = res.json().await?;
            json["content"][0]["text"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Failed to parse Anthropic response"))?
                .to_string()
        }
    };

    // 5. Save Reflection to temp file
    let brain = active_brain().ok_or_else(|| anyhow::anyhow!("No Brain found."))?;
    let temp_reflection = brain.join("dream/auto_reflection.md");
    fs::create_dir_all(temp_reflection.parent().unwrap())?;
    fs::write(&temp_reflection, &reflection)?;

    // 6. Ingest
    println!("{} Reflection received. Ingesting...", "→".green());
    ingest_reflection(&temp_reflection)?;

    // 7. Archive processed entries
    for (path, _) in &all_entries {
        let stem_name = path.file_stem().unwrap_or_default().to_string_lossy();
        let new_path = path
            .parent()
            .unwrap()
            .join(format!("{stem_name}-dreamed.md"));
        fs::rename(path, new_path)?;
    }

    // 8. Cleanup
    fs::remove_file(temp_reflection)?;

    // 8. Record Stats
    let stats_path = global_brain_path().join("stats.json");
    let mut stats: Stats = if stats_path.exists() {
        let content = fs::read_to_string(&stats_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Stats::default()
    };

    stats.total_dreams += 1;
    stats.total_tokens_input += prompt.len() as u64 / 4;
    stats.total_tokens_output += reflection.len() as u64 / 4;
    stats.last_dream_at = Some(chrono::Utc::now().to_rfc3339());

    fs::write(&stats_path, serde_json::to_string_pretty(&stats)?)?;

    println!("{} Autopilot Dream Cycle complete!", "✓".green());
    Ok(())
}

pub fn ingest_reflection(path: &PathBuf) -> Result<()> {
    let brain = active_brain().ok_or_else(|| {
        anyhow::anyhow!("No Brain found. Run {} first.", "cerebrum init".yellow())
    })?;
    let mut final_path = path.clone();
    if !final_path.exists() {
        let alt = brain.join("dream").join(path);
        if alt.exists() {
            final_path = alt;
        } else {
            anyhow::bail!("Could not find {} or {}", path.display(), alt.display());
        }
    }

    let content = fs::read_to_string(&final_path)
        .with_context(|| format!("Could not read {}", final_path.display()))?;

    println!("{}", "Ingesting reflection...".bright_magenta().bold());

    let stem = ts_stem(&Utc::now().to_rfc3339());
    let mut promoted = 0;

    // Parse each section and route to the right layer
    let sections = parse_reflection_sections(&content);

    if let Some(insights) = sections.get("key insights") {
        let p = brain
            .join("memory/semantic")
            .join(format!("{stem}-insights.md"));
        let entry = Entry {
            timestamp: Utc::now().to_rfc3339(),
            source_tool: "dream".to_string(),
            salience_score: 0.85,
            bias_flag: false,
            provenance: format!(
                "dream-ingest:{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            content: insights.join("\n"),
        };
        write_entry(&p, &entry)?;
        let _ = store_vector(&brain, &insights.join("\n"), &p.to_string_lossy());
        println!("   {} {} insights → semantic/", "→".dimmed(), insights.len());
        promoted += insights.len();
    }

    if let Some(rules) = sections.get("new rules") {
        let p = brain
            .join("memory/procedural/skills")
            .join(format!("{stem}-rules.md"));
        let entry = Entry {
            timestamp: Utc::now().to_rfc3339(),
            source_tool: "dream".to_string(),
            salience_score: 0.9,
            bias_flag: false,
            provenance: format!(
                "dream-ingest:{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            content: rules.join("\n"),
        };
        write_entry(&p, &entry)?;
        let _ = store_vector(&brain, &rules.join("\n"), &p.to_string_lossy());
        println!(
            "   {} {} rules → procedural/skills/",
            "→".dimmed(),
            rules.len()
        );
        promoted += rules.len();
    }

    if let Some(equity) = sections.get("equity & bias notes") {
        let p = brain
            .join("memory/personal")
            .join(format!("{stem}-equity.md"));
        let entry = Entry {
            timestamp: Utc::now().to_rfc3339(),
            source_tool: "dream".to_string(),
            salience_score: 0.9,
            bias_flag: false,
            provenance: format!(
                "dream-ingest:{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            content: equity.join("\n"),
        };
        write_entry(&p, &entry)?;
        let _ = store_vector(&brain, &equity.join("\n"), &p.to_string_lossy());
        println!("   {} {} notes → personal/", "→".dimmed(), equity.len());
        promoted += equity.len();
    }

    if let Some(prune) = sections.get("prune suggestions") {
        println!(
            "   {} {} prune suggestions (review manually):",
            "→".dimmed(),
            prune.len()
        );
        for item in prune {
            println!("      • {}", item.dimmed());
        }
    }

    println!(
        "\n{} {} items promoted into your Brain.",
        "Dream complete.".bright_magenta().bold(),
        promoted
    );
    Ok(())
}

pub fn parse_reflection_sections(content: &str) -> std::collections::HashMap<String, Vec<String>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(header) = trimmed.strip_prefix("## ") {
            current_section = header.trim().to_lowercase();
        } else if trimmed.starts_with("- ")
            || trimmed.starts_with("• ")
            || trimmed.starts_with("* ")
        {
            if !current_section.is_empty() {
                let bullet = trimmed[2..].trim().to_string();
                if !bullet.is_empty() {
                    map.entry(current_section.clone()).or_default().push(bullet);
                }
            }
        }
    }
    map
}
