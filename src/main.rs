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
    /// Initialize a new .cerebrum/ Brain in the current directory
    Init,
    /// Add a new entry to episodic memory
    Add { content: String },
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
        Commands::Init => init_brain(),
        Commands::Add { content } => add_entry(content),
        Commands::Status => show_status(),
        Commands::Template { list, apply } => handle_template(list, apply),
        Commands::Watch => watch_daemon().await,
    }
}

fn brain_path() -> PathBuf {
    std::env::current_dir()
        .expect("could not read current directory")
        .join(BRAIN_DIR)
}

fn episodic_filepath(brain: &PathBuf, source: &str) -> PathBuf {
    let ts: String = Utc::now()
        .to_rfc3339()
        .chars()
        .map(|c| if c == ':' { '_' } else { c })
        .collect();
    let stem = &ts[..ts.find('.').unwrap_or(19).min(19)];
    brain.join("memory/episodic").join(format!("{stem}-{source}.md"))
}

fn write_entry(path: &PathBuf, entry: &Entry) -> Result<()> {
    let yaml = serde_yaml::to_string(entry)?;
    fs::write(path, format!("---\n{yaml}---\n"))?;
    Ok(())
}

fn init_brain() -> Result<()> {
    let path = brain_path();

    if path.exists() {
        println!("{} Brain already exists at .cerebrum/", "✓".green().bold());
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

    println!("{}", "Brain initialized!".bright_cyan().bold());
    println!("   {} .cerebrum/ with 5 memory layers", "→".dimmed());
    println!("   {} cerebrum add \"your first rule\"", "→".dimmed());

    Ok(())
}

fn add_entry(content: String) -> Result<()> {
    let path = brain_path();
    if !path.exists() {
        anyhow::bail!("No Brain found. Run {} first.", "cerebrum init".yellow());
    }

    let timestamp = Utc::now().to_rfc3339();
    let entry = Entry {
        timestamp: timestamp.clone(),
        source_tool: "cli".to_string(),
        salience_score: 0.6,
        bias_flag: false,
        provenance: "manual".to_string(),
        content,
    };

    let safe_ts: String = timestamp
        .chars()
        .map(|c| if c == ':' { '_' } else { c })
        .collect();
    let stem = &safe_ts[..safe_ts.find('.').unwrap_or(19).min(19)];
    let filepath = path.join("memory/episodic").join(format!("{stem}.md"));

    write_entry(&filepath, &entry)?;

    println!("{} Added to episodic memory", "✓".green().bold());
    println!("   {} {}", "→".dimmed(), filepath.display());

    Ok(())
}

fn show_status() -> Result<()> {
    let path = brain_path();
    if !path.exists() {
        anyhow::bail!("No Brain found. Run {} first.", "cerebrum init".yellow());
    }

    println!("{}\n", "Cerebrumma Brain Status".bright_cyan().bold());

    for (name, rel) in &[
        ("Working", "memory/working"),
        ("Episodic", "memory/episodic"),
        ("Semantic", "memory/semantic"),
        ("Skills", "memory/procedural/skills"),
        ("Protocols", "memory/procedural/protocols"),
        ("Personal / Equity", "memory/personal"),
    ] {
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

    println!("\n   Use {} to grow it.", "cerebrum add \"...\"".yellow());

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

async fn watch_daemon() -> Result<()> {
    let brain = brain_path();
    if !brain.exists() {
        anyhow::bail!("No Brain found. Run {} first.", "cerebrum init".yellow());
    }

    let cwd = std::env::current_dir()?;

    println!("{}", "Cerebrumma Watcher running".bright_green().bold());
    println!("   {} watching {}", "→".dimmed(), cwd.display());
    println!("   {} Ctrl+C to stop\n", "→".dimmed());

    // Debounce: track last event time per file (2s window)
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

            // Skip ignored directories
            if IGNORED_PATHS.iter().any(|seg| path_str.contains(seg)) {
                continue;
            }

            // Only capture meaningful source files
            if !WATCHED_EXTENSIONS.iter().any(|ext| path_str.ends_with(ext)) {
                continue;
            }

            // Debounce: skip if same file touched within 2s
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

            let filepath = episodic_filepath(&brain, "watch");
            let entry = Entry {
                timestamp: Utc::now().to_rfc3339(),
                source_tool: "watcher".to_string(),
                salience_score: 0.4,
                bias_flag: false,
                provenance: "auto".to_string(),
                content: content.clone(),
            };

            match write_entry(&filepath, &entry) {
                Ok(_) => println!("{} {}", "→".bright_blue(), content),
                Err(e) => eprintln!("{} {}", "error:".red(), e),
            }
        }
    }

    Ok(())
}
