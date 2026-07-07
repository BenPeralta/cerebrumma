use anyhow::Result;
use chrono::Utc;
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::daemon::check_dream_saturation;
use crate::git::hook_install;
use crate::ui::run_audit;
use crate::vector::{init_vector_db, store_vector};

pub const BRAIN_DIR: &str = ".cerebrum";

#[derive(Serialize, Deserialize)]
pub struct Entry {
    pub timestamp: String,
    pub source_tool: String,
    pub salience_score: f32,
    pub bias_flag: bool,
    pub provenance: String,
    pub content: String,
}

pub fn global_brain_path() -> PathBuf {
    dirs::home_dir()
        .expect("could not find home directory")
        .join(".cerebrum")
}

pub fn local_brain_path() -> PathBuf {
    std::env::current_dir()
        .expect("could not read current directory")
        .join(BRAIN_DIR)
}

pub fn resolve_brain(global: bool) -> PathBuf {
    if global {
        global_brain_path()
    } else {
        local_brain_path()
    }
}

// Returns the best brain to write to: local if it exists, else global.
pub fn active_brain() -> Option<PathBuf> {
    let local = local_brain_path();
    if local.exists() {
        return Some(local);
    }
    let global = global_brain_path();
    if global.exists() {
        return Some(global);
    }
    None
}

pub fn ts_stem(ts: &str) -> String {
    // 2026-05-07T18:01:03.123+00:00 → 2026-05-07T18_01_03_123
    ts.chars()
        .map(|c| match c {
            ':' | '.' => '_',
            _ => c,
        })
        .collect::<String>()
        .split('+')
        .next()
        .unwrap_or("")
        .trim_end_matches('Z')
        .to_string()
}

pub fn make_episodic_path(brain: &PathBuf, tag: &str) -> PathBuf {
    let stem = ts_stem(&Utc::now().to_rfc3339());
    brain
        .join("memory/episodic")
        .join(format!("{stem}-{tag}.md"))
}

pub fn write_entry(path: &PathBuf, entry: &Entry) -> Result<()> {
    let yaml = serde_yaml::to_string(entry)?;
    fs::write(path, format!("---\n{yaml}---\n"))?;
    Ok(())
}

pub fn init_brain(path: PathBuf) -> Result<()> {
    if path.exists() {
        println!(
            "{} Brain already exists at {}",
            "✓".green().bold(),
            path.display()
        );
        let _ = init_vector_db(&path);
        return Ok(());
    }

    // ── 1. Branding ──────────────────────────────────────────────────────────
    println!("\n  {}", "Cerebrumma".bright_cyan().bold());
    println!("  {}\n", "One Brain. Any Tool.".dimmed());

    // ── 2. Create Folders ────────────────────────────────────────────────────
    println!("   {} Initializing memory layers...", "→".dimmed());
    for dir in &[
        "memory/working",
        "memory/episodic",
        "memory/semantic",
        "memory/procedural/skills",
        "memory/procedural/protocols",
        "memory/personal",
        "embeddings",
        "logs",
        "dream",
    ] {
        fs::create_dir_all(path.join(dir))?;
    }

    // ── 2.5 Vector DB ────────────────────────────────────────────────────────
    println!("   {} Initializing vector store...", "→".dimmed());
    init_vector_db(&path)?;

    fs::write(
        path.join("config.json"),
        include_str!("../config.default.json"),
    )?;

    let now = Utc::now().to_rfc3339();
    let welcome = include_str!("../templates/welcome.md").replace("{{timestamp}}", &now);
    fs::write(path.join("memory/episodic/000-welcome.md"), welcome)?;

    // ── 3. Auto-Hook (if local) ──────────────────────────────────────────────
    if path != global_brain_path() {
        println!("   {} Installing git auto-capture hook...", "→".dimmed());
        let _ = hook_install();
    }

    // ── 4. Auto-Audit (if local) ──────────────────────────────────────────────
    if path != global_brain_path() {
        println!("   {} Performing project audit...", "→".dimmed());
        let _ = run_audit();
    }

    // ── 5. Done ──────────────────────────────────────────────────────────────
    let label = if path == global_brain_path() {
        "~/.cerebrum/ (global)"
    } else {
        ".cerebrum/ (local)"
    };
    println!("\n  {} {}", "Brain Initialized!".green().bold(), label);

    if path != global_brain_path() {
        println!("\n  {}", "Next Step:".bold());
        println!("    Paste the Audit Prompt into your AI to wake up your Brain.");
    } else {
        println!("\n  {}", "Quickstart:".bold());
        println!("    cerebrum add --global \"I always use TypeScript strict mode\"");
    }
    println!();

    Ok(())
}

pub async fn add_entry(content: String, path: PathBuf) -> Result<()> {
    if !path.exists() {
        let flag = if path == global_brain_path() {
            " --global"
        } else {
            ""
        };
        anyhow::bail!(
            "No Brain found at {}. Run {} first.",
            path.display(),
            format!("cerebrum init{flag}").yellow()
        );
    }

    let timestamp = Utc::now().to_rfc3339();
    let stem = ts_stem(&timestamp);
    let filepath = path.join("memory/episodic").join(format!("{stem}.md"));

    write_entry(
        &filepath,
        &Entry {
            timestamp,
            source_tool: "cli".to_string(),
            salience_score: 0.6,
            bias_flag: false,
            provenance: "manual".to_string(),
            content: content.clone(),
        },
    )?;

    // Check for saturation
    let _ = check_dream_saturation(&path).await;

    // Store vector
    let _ = store_vector(&path, &content, &filepath.to_string_lossy());

    println!("{} Added to episodic memory", "✓".green().bold());
    println!("   {} {}", "→".dimmed(), filepath.display());

    Ok(())
}

pub fn load_episodes(brain: &PathBuf) -> Vec<(PathBuf, String)> {
    let dir = brain.join("memory/episodic");
    if !dir.exists() {
        return vec![];
    }
    let mut entries: Vec<(PathBuf, String)> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.ends_with(".md") && !s.starts_with("000-") && !s.contains("-dreamed")
        })
        .filter_map(|e| {
            let path = e.path();
            fs::read_to_string(&path).ok().map(|text| (path, text))
        })
        .collect();
    entries.sort_by_key(|(p, _)| p.clone());
    entries
}

pub fn get_latest_content(dir: PathBuf) -> Option<String> {
    if !dir.exists() {
        return None;
    }
    let mut files: Vec<_> = fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".md"))
        .collect();

    files.sort_by_key(|e| e.file_name());
    let latest = files.last()?;
    fs::read_to_string(latest.path()).ok()
}
