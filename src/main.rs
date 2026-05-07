use anyhow::Result;
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
    Dream,
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
        Commands::Dream => dream_cycle(),
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

fn dream_cycle() -> Result<()> {
    let brain = active_brain()
        .ok_or_else(|| anyhow::anyhow!("No Brain found. Run {} first.", "cerebrum init".yellow()))?;

    println!("{}", "Dream cycle starting...".bright_magenta().bold());

    let episodic_dir = brain.join("memory/episodic");
    let semantic_dir = brain.join("memory/semantic");
    let dream_dir = brain.join("dream");

    // Collect all episodic entries (skip welcome + already-dreamed)
    let mut entries: Vec<(PathBuf, String)> = fs::read_dir(&episodic_dir)?
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

    if entries.is_empty() {
        println!("   {} No new episodic entries to reflect on.", "→".dimmed());
        println!("   {} Add entries with {} first.", "→".dimmed(), "cerebrum add".yellow());
        return Ok(());
    }

    println!("   {} Found {} episodic entries to reflect on", "→".dimmed(), entries.len());

    // Build a digest of all entries
    let digest: String = entries
        .iter()
        .map(|(path, text)| {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            format!("### {filename}\n{text}\n")
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Write digest to dream/ staging area
    let ts: String = Utc::now()
        .to_rfc3339()
        .chars()
        .map(|c| if c == ':' { '_' } else { c })
        .collect();
    let stem = &ts[..ts.find('.').unwrap_or(19).min(19)];
    let dream_file = dream_dir.join(format!("{stem}-digest.md"));

    let dream_content = format!(
        "---\ntimestamp: {}\ntype: dream-digest\nentry_count: {}\n---\n\n{digest}",
        Utc::now().to_rfc3339(),
        entries.len()
    );
    fs::write(&dream_file, &dream_content)?;

    println!("   {} Digest written → {}", "→".dimmed(), dream_file.display());

    // Promote a summary to semantic memory
    let summary_path = semantic_dir.join(format!("{stem}-dream-summary.md"));
    let summary = Entry {
        timestamp: Utc::now().to_rfc3339(),
        source_tool: "dream".to_string(),
        salience_score: 0.8,
        bias_flag: false,
        provenance: format!("dream-digest:{stem}"),
        content: format!(
            "Dream summary: reflected on {} episodic entries. Digest stored at dream/{}-digest.md. Review and promote key insights to semantic/ or procedural/ manually.",
            entries.len(),
            stem
        ),
    };
    write_entry(&summary_path, &summary)?;

    // Mark source entries as dreamed (rename with -dreamed suffix)
    let mut promoted = 0;
    for (path, _) in &mut entries {
        let new_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + "-dreamed.md";
        let new_path = episodic_dir.join(new_name);
        fs::rename(path, new_path)?;
        promoted += 1;
    }

    println!("   {} {} entries archived (renamed -dreamed)", "→".dimmed(), promoted);
    println!("   {} Summary promoted to semantic memory", "→".dimmed());
    println!("\n{}", "Dream complete.".bright_magenta().bold());
    println!("   Next: review {} and promote key insights manually.", "dream/".yellow());

    Ok(())
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
