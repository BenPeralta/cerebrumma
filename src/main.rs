use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use colored::*;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "cerebrum", author, version, about = "Portable, git-tracked AI Brain for developers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new .cerebrum/ Brain in the current directory (or globally with --global)
    Init {
        #[arg(long, help = "Create the global Brain at ~/.cerebrum/")]
        global: bool,
    },
    /// Add a new entry to episodic memory
    Add {
        content: String,
        #[arg(long, help = "Write to the global Brain (~/.cerebrum/) instead of local")]
        global: bool,
    },
    /// Show Brain status
    Status,
    /// List or apply community templates
    Template {
        #[arg(short, long)]
        list: bool,
        #[arg(short, long)]
        apply: Option<String>,
    },
    /// Watch for file changes and auto-capture to episodic memory
    Watch,
    /// Install or remove the git post-commit hook
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Reflect on episodic memory and promote insights (dream cycle)
    Dream {
        /// Ingest an LLM reflection file and promote it into the Brain
        #[arg(long, value_name = "FILE")]
        ingest: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum HookAction {
    /// Install the post-commit hook into .git/hooks/
    Install,
    /// Remove the post-commit hook
    Remove,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    timestamp: String,
    source_tool: String,
    salience_score: f32,
    bias_flag: bool,
    provenance: String,
    content: String,
}

const BRAIN_DIR: &str = ".cerebrum";

const WATCHED_EXTENSIONS: &[&str] = &[
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".go", ".py", ".md", ".toml", ".yaml", ".yml", ".json",
];

const IGNORED_PATHS: &[&str] = &[
    ".git", "node_modules", "target", "dist", ".next", "build", ".cerebrum", ".venv", "__pycache__",
];

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { global } => init_brain(resolve_brain(global)),
        Commands::Add { content, global } => add_entry(content, resolve_brain(global)),
        Commands::Status => show_status(),
        Commands::Template { list, apply } => handle_template(list, apply),
        Commands::Watch => watch_daemon().await,
        Commands::Hook { action } => match action {
            HookAction::Install => hook_install(),
            HookAction::Remove => hook_remove(),
        },
        Commands::Dream { ingest } => dream_cycle(ingest),
    }
}

fn global_brain_path() -> PathBuf {
    dirs::home_dir()
        .expect("could not find home directory")
        .join(".cerebrum")
}

fn local_brain_path() -> PathBuf {
    std::env::current_dir()
        .expect("could not read current directory")
        .join(BRAIN_DIR)
}

fn resolve_brain(global: bool) -> PathBuf {
    if global { global_brain_path() } else { local_brain_path() }
}

// Returns the best brain to write to: local if it exists, else global.
fn active_brain() -> Option<PathBuf> {
    let local = local_brain_path();
    if local.exists() { return Some(local); }
    let global = global_brain_path();
    if global.exists() { return Some(global); }
    None
}

fn ts_stem(ts: &str) -> String {
    // 2026-05-07T18:01:03.123+00:00 → 2026-05-07T18_01_03_123
    ts.chars()
        .map(|c| match c { ':' | '.' => '_', _ => c })
        .collect::<String>()
        .split('+')
        .next()
        .unwrap_or("")
        .trim_end_matches('Z')
        .to_string()
}

fn make_episodic_path(brain: &PathBuf, tag: &str) -> PathBuf {
    let stem = ts_stem(&Utc::now().to_rfc3339());
    brain.join("memory/episodic").join(format!("{stem}-{tag}.md"))
}

fn write_entry(path: &PathBuf, entry: &Entry) -> Result<()> {
    let yaml = serde_yaml::to_string(entry)?;
    fs::write(path, format!("---\n{yaml}---\n"))?;
    Ok(())
}

fn init_brain(path: PathBuf) -> Result<()> {
    if path.exists() {
        println!("{} Brain already exists at {}", "✓".green().bold(), path.display());
        return Ok(());
    }

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

    fs::write(path.join("config.json"), include_str!("../config.default.json"))?;

    let now = Utc::now().to_rfc3339();
    let welcome = include_str!("../templates/welcome.md").replace("{{timestamp}}", &now);
    fs::write(path.join("memory/episodic/000-welcome.md"), welcome)?;

    let label = if path == global_brain_path() { "~/.cerebrum/ (global)" } else { ".cerebrum/ (local)" };
    println!("{}", "Brain initialized!".bright_cyan().bold());
    println!("   {} {} with 5 memory layers", "→".dimmed(), label);
    println!("   {} cerebrum add \"your first rule\"", "→".dimmed());
    println!("   {} cerebrum hook install  (auto-capture git commits)", "→".dimmed());

    Ok(())
}

fn add_entry(content: String, path: PathBuf) -> Result<()> {
    if !path.exists() {
        let flag = if path == global_brain_path() { " --global" } else { "" };
        anyhow::bail!("No Brain found at {}. Run {} first.", path.display(), format!("cerebrum init{flag}").yellow());
    }

    let timestamp = Utc::now().to_rfc3339();
    let stem = ts_stem(&timestamp);
    let filepath = path.join("memory/episodic").join(format!("{stem}.md"));

    write_entry(&filepath, &Entry {
        timestamp,
        source_tool: "cli".to_string(),
        salience_score: 0.6,
        bias_flag: false,
        provenance: "manual".to_string(),
        content,
    })?;

    println!("{} Added to episodic memory", "✓".green().bold());
    println!("   {} {}", "→".dimmed(), filepath.display());

    Ok(())
}

fn show_status() -> Result<()> {
    let global = global_brain_path();
    let local = local_brain_path();

    if !global.exists() && !local.exists() {
        anyhow::bail!(
            "No Brain found. Run {} for a global Brain or {} in a project folder.",
            "cerebrum init --global".yellow(),
            "cerebrum init".yellow()
        );
    }

    let layers = [
        ("Working", "memory/working"),
        ("Episodic", "memory/episodic"),
        ("Semantic", "memory/semantic"),
        ("Skills", "memory/procedural/skills"),
        ("Protocols", "memory/procedural/protocols"),
        ("Personal / Equity", "memory/personal"),
    ];

    let print_brain = |label: &str, path: &PathBuf| {
        println!("{} {}", "Brain".bright_cyan().bold(), label.bold());
        for (name, rel) in &layers {
            let full = path.join(rel);
            let count = if full.exists() {
                fs::read_dir(&full).map(|d| d.count()).unwrap_or(0)
            } else {
                0
            };
            let count_str = if count == 0 {
                "0".dimmed().to_string()
            } else {
                count.to_string().green().to_string()
            };
            println!("   {:<22} {} {}", name, "→".dimmed(), count_str);
        }
        println!();
    };

    if global.exists() {
        print_brain("~/.cerebrum/ (global)", &global);
    }
    if local.exists() {
        print_brain(".cerebrum/ (local)", &local);
    }

    println!("   Use {} to grow it.", "cerebrum add \"...\"".yellow());
    Ok(())
}

fn handle_template(list: bool, apply: Option<String>) -> Result<()> {
    if list {
        println!("{}", "Available templates (coming soon):".bright_magenta().bold());
        println!("   • equitable-react");
        println!("   • startup-ops");
        println!("   • fair-hiring");
        return Ok(());
    }

    if let Some(name) = apply {
        println!("Applying template: {}", name.yellow());
        println!("   {} (clone + merge into .cerebrum/ — next version)", "stub".dimmed());
    }

    Ok(())
}

// ── Git Hook ─────────────────────────────────────────────────────────────────

const HOOK_MARKER: &str = "# cerebrumma-hook";

const HOOK_SCRIPT: &str = r#"#!/bin/sh
# cerebrumma-hook
BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
HASH=$(git rev-parse --short HEAD 2>/dev/null)
MSG=$(git log -1 --pretty=%B 2>/dev/null | head -3 | tr '\n' ' ')
FILES=$(git diff-tree --no-commit-id -r --name-only HEAD 2>/dev/null | head -10 | tr '\n' ' ')
STATS=$(git diff-tree --no-commit-id -r --stat HEAD 2>/dev/null | tail -1)
cerebrum add "commit ${HASH} on ${BRANCH}: ${MSG}| files: ${FILES}| ${STATS}"
"#;

fn hook_install() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let hooks_dir = cwd.join(".git/hooks");
    let hook_path = hooks_dir.join("post-commit");

    if !hooks_dir.exists() {
        anyhow::bail!("No .git/ found. Run this inside a git repository.");
    }

    if hook_path.exists() {
        let existing = fs::read_to_string(&hook_path).unwrap_or_default();
        if !existing.contains(HOOK_MARKER) {
            anyhow::bail!(
                "A post-commit hook already exists and wasn't created by Cerebrumma.\n   \
                 Inspect {} and add the hook manually.",
                hook_path.display()
            );
        }
        println!("{} Hook already installed — updating", "↺".yellow());
    }

    fs::write(&hook_path, HOOK_SCRIPT)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
    }

    println!("{} Git hook installed", "✓".green().bold());
    println!("   {} Commits will now auto-capture branch + hash + diff stats", "→".dimmed());
    println!("   {} {}", "→".dimmed(), hook_path.display());

    Ok(())
}

fn hook_remove() -> Result<()> {
    let hook_path = std::env::current_dir()?.join(".git/hooks/post-commit");

    if !hook_path.exists() {
        println!("{} No hook found", "→".dimmed());
        return Ok(());
    }

    let contents = fs::read_to_string(&hook_path).unwrap_or_default();
    if !contents.contains(HOOK_MARKER) {
        anyhow::bail!(
            "Hook at {} wasn't created by Cerebrumma — not removing it.\n   Remove manually if needed.",
            hook_path.display()
        );
    }

    fs::remove_file(&hook_path)?;
    println!("{} Git hook removed", "✓".green().bold());

    Ok(())
}

// ── Dream Cycle ───────────────────────────────────────────────────────────────

fn load_episodes(brain: &PathBuf) -> Vec<(PathBuf, String)> {
    let dir = brain.join("memory/episodic");
    if !dir.exists() { return vec![]; }
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

fn build_reflection_prompt(episodes: &[(PathBuf, String)]) -> String {
    let body = episodes.iter().take(20)
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

fn dream_cycle(ingest: Option<PathBuf>) -> Result<()> {
    // ── Ingest mode: parse LLM reflection and promote ──────────────────────────
    if let Some(ref ingest_path) = ingest {
        return ingest_reflection(ingest_path);
    }

    // ── Generate mode: load episodes + write prompt ────────────────────────────
    println!("{}", "Dream cycle starting...".bright_magenta().bold());

    let global = global_brain_path();
    let local  = local_brain_path();

    let mut all_entries: Vec<(PathBuf, String)> = vec![];
    all_entries.extend(load_episodes(&local));
    all_entries.extend(load_episodes(&global));

    if all_entries.is_empty() {
        println!("   {} No new episodic entries to reflect on.", "→".dimmed());
        println!("   {} Add some with {}.", "→".dimmed(), "cerebrum add \"...\"".yellow());
        return Ok(());
    }

    println!("   {} {} episodic entries loaded", "→".dimmed(), all_entries.len());

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
        let new_path = path.parent().unwrap().join(format!("{stem_name}-dreamed.md"));
        fs::rename(path, new_path)?;
        archived += 1;
    }

    println!("   {} {} entries archived", "→".dimmed(), archived);
    println!("   {} Reflection prompt → {}", "→".dimmed(), prompt_path.display());
    println!();
    println!("{}", "Next steps:".bold());
    println!("   1. Open {} and paste it into Claude/Grok", prompt_path.display());
    println!("   2. Save the LLM's response as a .md file");
    println!("   3. Run: {}", format!("cerebrum dream --ingest <response.md>").yellow());

    Ok(())
}

fn ingest_reflection(path: &PathBuf) -> Result<()> {
    let brain = active_brain()
        .ok_or_else(|| anyhow::anyhow!("No Brain found. Run {} first.", "cerebrum init".yellow()))?;

    let content = fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", path.display()))?;

    println!("{}", "Ingesting reflection...".bright_magenta().bold());

    let stem = ts_stem(&Utc::now().to_rfc3339());
    let mut promoted = 0;

    // Parse each section and route to the right layer
    let sections = parse_reflection_sections(&content);

    if let Some(insights) = sections.get("Key Insights") {
        let p = brain.join("memory/semantic").join(format!("{stem}-insights.md"));
        let entry = Entry {
            timestamp: Utc::now().to_rfc3339(),
            source_tool: "dream".to_string(),
            salience_score: 0.85,
            bias_flag: false,
            provenance: format!("dream-ingest:{}", path.file_name().unwrap_or_default().to_string_lossy()),
            content: insights.join("\n"),
        };
        write_entry(&p, &entry)?;
        println!("   {} {} insights → semantic/", "→".dimmed(), insights.len());
        promoted += insights.len();
    }

    if let Some(rules) = sections.get("New Rules") {
        let p = brain.join("memory/procedural/skills").join(format!("{stem}-rules.md"));
        let entry = Entry {
            timestamp: Utc::now().to_rfc3339(),
            source_tool: "dream".to_string(),
            salience_score: 0.9,
            bias_flag: false,
            provenance: format!("dream-ingest:{}", path.file_name().unwrap_or_default().to_string_lossy()),
            content: rules.join("\n"),
        };
        write_entry(&p, &entry)?;
        println!("   {} {} rules → procedural/skills/", "→".dimmed(), rules.len());
        promoted += rules.len();
    }

    if let Some(equity) = sections.get("Equity & Bias Notes") {
        let p = brain.join("memory/personal").join(format!("{stem}-equity.md"));
        let entry = Entry {
            timestamp: Utc::now().to_rfc3339(),
            source_tool: "dream".to_string(),
            salience_score: 0.9,
            bias_flag: false,
            provenance: format!("dream-ingest:{}", path.file_name().unwrap_or_default().to_string_lossy()),
            content: equity.join("\n"),
        };
        write_entry(&p, &entry)?;
        println!("   {} {} notes → personal/", "→".dimmed(), equity.len());
        promoted += equity.len();
    }

    if let Some(prune) = sections.get("Prune Suggestions") {
        println!("   {} {} prune suggestions (review manually):", "→".dimmed(), prune.len());
        for item in prune {
            println!("      • {}", item.dimmed());
        }
    }

    println!("\n{} {} items promoted into your Brain.", "Dream complete.".bright_magenta().bold(), promoted);
    Ok(())
}

fn parse_reflection_sections(content: &str) -> std::collections::HashMap<String, Vec<String>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // Detect section headers: ## Key Insights or **Key Insights**
        if let Some(header) = trimmed.strip_prefix("## ") {
            current_section = header.trim().to_string();
        } else if trimmed.starts_with("- ") || trimmed.starts_with("• ") || trimmed.starts_with("* ") {
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

// ── Watcher ───────────────────────────────────────────────────────────────────

async fn watch_daemon() -> Result<()> {
    let brain = active_brain()
        .ok_or_else(|| anyhow::anyhow!("No Brain found. Run {} first.", "cerebrum init".yellow()))?;

    let cwd = std::env::current_dir()?;

    println!("{}", "Cerebrumma Watcher running".bright_green().bold());
    println!("   {} {}", "→".dimmed(), cwd.display());
    println!("   {} Ctrl+C to stop\n", "→".dimmed());

    let debounce: Arc<Mutex<HashMap<PathBuf, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(200);

    let mut watcher = RecommendedWatcher::new(
        move |res| { let _ = tx.blocking_send(res); },
        notify::Config::default(),
    )?;

    watcher.watch(&cwd, RecursiveMode::Recursive)?;

    while let Some(res) = rx.recv().await {
        let Ok(Event { kind: EventKind::Modify(_) | EventKind::Create(_), paths, .. }) = res else {
            continue;
        };

        for path in paths {
            let path_str = path.to_string_lossy();

            if IGNORED_PATHS.iter().any(|seg| path_str.contains(seg)) {
                continue;
            }

            if !WATCHED_EXTENSIONS.iter().any(|ext| path_str.ends_with(ext)) {
                continue;
            }

            {
                let mut map = debounce.lock().unwrap();
                let now = Instant::now();
                if let Some(last) = map.get(&path) {
                    if now.duration_since(*last) < Duration::from_secs(2) {
                        continue;
                    }
                }
                map.insert(path.clone(), now);
            }

            let relative = path.strip_prefix(&cwd).unwrap_or(&path);
            let content = format!("Modified: {}", relative.display());
            let filepath = make_episodic_path(&brain, "watch");

            match write_entry(&filepath, &Entry {
                timestamp: Utc::now().to_rfc3339(),
                source_tool: "watcher".to_string(),
                salience_score: 0.4,
                bias_flag: false,
                provenance: "auto".to_string(),
                content: content.clone(),
            }) {
                Ok(_) => println!("{} {}", "→".bright_blue(), content),
                Err(e) => eprintln!("{} {}", "error:".red(), e),
            }
        }
    }

    Ok(())
}
