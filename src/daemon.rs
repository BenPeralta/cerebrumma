use anyhow::Result;
use chrono::Utc;
use colored::*;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::cli::DaemonAction;
use crate::dreamer::dream_cycle;
use crate::memory::{active_brain, make_episodic_path, write_entry, Entry};

const WATCHED_EXTENSIONS: &[&str] = &[
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".go", ".py", ".md", ".toml", ".yaml", ".yml", ".json",
];

const IGNORED_PATHS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    ".next",
    "build",
    ".cerebrum",
    ".venv",
    "__pycache__",
];

pub async fn watch_daemon() -> Result<()> {
    let brain = active_brain().ok_or_else(|| {
        anyhow::anyhow!("No Brain found. Run {} first.", "cerebrum init".yellow())
    })?;

    let cwd = std::env::current_dir()?;

    println!("{}", "Cerebrumma Watcher running".bright_green().bold());
    println!("   {} {}", "→".dimmed(), cwd.display());
    println!("   {} Ctrl+C to stop\n", "→".dimmed());

    let debounce: Arc<Mutex<HashMap<PathBuf, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(200);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.blocking_send(res);
        },
        notify::Config::default(),
    )?;

    watcher.watch(&cwd, RecursiveMode::Recursive)?;

    while let Some(res) = rx.recv().await {
        let Ok(Event {
            kind: EventKind::Modify(_) | EventKind::Create(_),
            paths,
            ..
        }) = res
        else {
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

            match write_entry(
                &filepath,
                &Entry {
                    timestamp: Utc::now().to_rfc3339(),
                    source_tool: "watcher".to_string(),
                    salience_score: 0.4,
                    bias_flag: false,
                    provenance: "auto".to_string(),
                    content: content.clone(),
                },
            ) {
                Ok(_) => {
                    println!("{} {}", "→".bright_blue(), content);
                    let _ = check_dream_saturation(&brain).await;
                }
                Err(e) => eprintln!("{} {}", "error:".red(), e),
            }
        }
    }

    Ok(())
}

pub async fn check_dream_saturation(brain: &PathBuf) -> Result<()> {
    let episodic_dir = brain.join("memory/episodic");
    if !episodic_dir.exists() {
        return Ok(());
    }

    let count = fs::read_dir(&episodic_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".md"))
        .count();

    if count >= 10 {
        println!(
            "\n{} Brain is saturated with {} new experiences. Triggering auto dream cycle...",
            "✧".bright_magenta().bold(),
            count
        );
        // We use auto=true here because this is usually called from background/daemon
        // If config is missing, it will just log an error in the daemon output
        if let Err(e) = dream_cycle(None, true).await {
            eprintln!("{} Auto-dream failed: {}", "×".red(), e);
        }
    }

    Ok(())
}

pub fn run_daemon_cmd(action: DaemonAction) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let path_str = cwd.to_string_lossy();
    let hash = format!("{:x}", md5::compute(path_str.as_bytes()));
    let label = format!("com.cerebrumma.{}", &hash[..8]);
    let plist_path = dirs::home_dir()
        .unwrap()
        .join(format!("Library/LaunchAgents/{}.plist", label));

    match action {
        DaemonAction::Start => {
            println!(
                "{} Starting persistent intelligence for {}...",
                "→".bright_magenta(),
                cwd.display()
            );

            let exe_path = std::env::current_exe()?;
            let plist_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>watch</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>WorkingDirectory</key>
    <string>{}</string>
    <key>StandardOutPath</key>
    <string>{}/.cerebrum/daemon.log</string>
    <key>StandardErrorPath</key>
    <string>{}/.cerebrum/daemon.log</string>
</dict>
</plist>"#,
                label,
                exe_path.display(),
                path_str,
                path_str,
                path_str
            );

            fs::write(&plist_path, plist_content)?;

            std::process::Command::new("launchctl")
                .arg("load")
                .arg(&plist_path)
                .status()?;

            println!("{} Daemon started! (Label: {})", "✓".green(), label);
            println!("{} Logs available at .cerebrum/daemon.log", "→".dimmed());
        }
        DaemonAction::Stop => {
            println!("{} Stopping daemon {}...", "→".yellow(), label);

            let _ = std::process::Command::new("launchctl")
                .arg("unload")
                .arg(&plist_path)
                .status();

            if plist_path.exists() {
                let _ = fs::remove_file(&plist_path);
            }

            println!("{} Daemon stopped.", "✓".green());
        }
        DaemonAction::Status => {
            let output = std::process::Command::new("launchctl")
                .arg("list")
                .arg(&label)
                .output()?;

            if output.status.success() {
                println!("{} Daemon is RUNNING (Label: {})", "●".green(), label);
            } else {
                println!("{} Daemon is NOT running.", "○".red());
            }
        }
    }
    Ok(())
}
