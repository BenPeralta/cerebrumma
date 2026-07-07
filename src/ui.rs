use anyhow::Result;
use colored::*;
use std::fs;

use crate::dreamer::parse_reflection_sections;
use crate::git::{get_current_branch, get_git_status};
use crate::memory::{
    active_brain, get_latest_content, global_brain_path, load_episodes, local_brain_path,
};
use crate::vector::{build_graph_data, store_vector};

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

pub fn show_status() -> Result<()> {
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

    let print_brain = |label: &str, path: &std::path::PathBuf| {
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

pub fn handle_template(list: bool, apply: Option<String>) -> Result<()> {
    if list {
        println!(
            "{}",
            "Available templates (coming soon):"
                .bright_magenta()
                .bold()
        );
        println!("   • equitable-react");
        println!("   • startup-ops");
        println!("   • fair-hiring");
        return Ok(());
    }

    if let Some(name) = apply {
        println!("Applying template: {}", name.yellow());
        println!(
            "   {} (clone + merge into .cerebrum/ — next version)",
            "stub".dimmed()
        );
    }

    Ok(())
}

pub fn run_audit() -> Result<()> {
    println!("{}", "Project Audit starting...".bright_magenta().bold());

    let brain = active_brain().ok_or_else(|| {
        anyhow::anyhow!("No Brain found. Run {} first.", "cerebrum init".yellow())
    })?;

    let cwd = std::env::current_dir()?;
    let mut structure = String::new();
    let mut contexts = String::new();
    let mut file_count = 0;

    println!("   {} Scanning files...", "→".dimmed());

    // 1. Build structure representation
    for entry in walkdir::WalkDir::new(&cwd)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path().to_string_lossy();
            !IGNORED_PATHS.iter().any(|seg| path.contains(seg))
        })
    {
        let depth = entry.depth();
        let name = entry.file_name().to_string_lossy();
        let indent = "  ".repeat(depth);
        structure.push_str(&format!("{}{}\n", indent, name));
    }

    // 2. Capture content for key files
    // Sort logic: Prioritize .md files, then core logic (swift, rs, ts, py), then config (toml, json, plist)
    let mut files_to_read = vec![];
    for entry in walkdir::WalkDir::new(&cwd)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path().to_string_lossy();
            !IGNORED_PATHS.iter().any(|seg| path.contains(seg)) && e.file_type().is_file()
        })
    {
        files_to_read.push(entry.path().to_path_buf());
    }

    // Sort: .md files first!
    files_to_read.sort_by(|a, b| {
        let a_is_md = a.extension().map_or(false, |e| e == "md");
        let b_is_md = b.extension().map_or(false, |e| e == "md");
        if a_is_md && !b_is_md {
            return std::cmp::Ordering::Less;
        }
        if !a_is_md && b_is_md {
            return std::cmp::Ordering::Greater;
        }
        a.cmp(b)
    });

    for path in files_to_read.iter().take(50) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ["md", "toml", "json", "swift", "rs", "ts", "py", "plist"].contains(&ext) {
            if let Ok(content) = fs::read_to_string(path) {
                let relative = path.strip_prefix(&cwd).unwrap_or(path);
                contexts.push_str(&format!("\n--- {} ---\n", relative.display()));
                contexts.push_str(&content.chars().take(2000).collect::<String>());
                contexts.push_str("\n");
                file_count += 1;
            }
        }
    }

    let prompt = include_str!("../templates/audit_prompt.md")
        .replace("{{structure}}", &structure)
        .replace("{{contexts}}", &contexts);

    let audit_dir = brain.join("dream");
    fs::create_dir_all(&audit_dir)?;
    let prompt_path = audit_dir.join("Audit-Prompt.md");
    fs::write(&prompt_path, prompt.clone())?;

    // Store the audit prompt in vector DB for context search
    let _ = store_vector(&brain, &prompt, "dream/Audit-Prompt.md");

    println!("   {} Scanned {} high-salience files", "→".dimmed(), file_count);
    println!("   {} Audit prompt → {}", "→".dimmed(), prompt_path.display());
    println!();
    println!("{}", "Next steps:".bold());
    println!("   1. Open {} and paste it into your AI", prompt_path.display());
    println!("   2. Save the AI's response as a .md file");
    println!("   3. Run: {}", format!("cerebrum dream --ingest <response.md>").yellow());

    Ok(())
}

pub fn run_brief() -> Result<()> {
    println!("{}", "Generating Session Brief...".bright_magenta().bold());

    let brain = active_brain().ok_or_else(|| {
        anyhow::anyhow!("No Brain found. Run {} first.", "cerebrum init".yellow())
    })?;

    let global = global_brain_path();
    let local = local_brain_path();

    // ── 1. Episodic Context (Recent Logs) ───────────────────────────────────
    let mut episodes = vec![];
    episodes.extend(load_episodes(&local));
    episodes.extend(load_episodes(&global));

    // Sort by date (filename) descending
    episodes.sort_by(|a, b| b.0.cmp(&a.0));

    let recent = episodes
        .iter()
        .take(5)
        .map(|(path, text)| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            format!("### {name}\n{text}")
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    // ── 2. Intelligence Context (Insights & Rules) ─────────────────────────
    let latest_insights = get_latest_content(brain.join("memory/semantic")).unwrap_or_default();
    let latest_rules =
        get_latest_content(brain.join("memory/procedural/skills")).unwrap_or_default();
    let latest_equity = get_latest_content(brain.join("memory/personal")).unwrap_or_default();

    let brief = format!(
        r#"# Session Brief: Catch Me Up
You are joining an active coding session. Here is the current state of the project.

## Project Soul (Key Insights)
{insights}

## Operational Rules (The "How")
{rules}

## Equity & Bias Context
{equity}

## Recent Changes (Last 5 events)
{recent}

## Current Context
- **Branch:** {branch}
- **Status:** {status}

## Your Task
1. Acknowledge the project soul and rules.
2. Review the recent changes.
3. Ask the user what the next immediate step is.
"#,
        insights = latest_insights,
        rules = latest_rules,
        equity = latest_equity,
        recent = recent,
        branch = get_current_branch().unwrap_or_else(|_| "unknown".to_string()),
        status = get_git_status().unwrap_or_else(|_| "unknown".to_string())
    );

    let brief_path = brain.join("dream").join("Session-Brief.md");
    fs::create_dir_all(brief_path.parent().unwrap())?;
    fs::write(&brief_path, brief)?;

    println!("   {} Brief generated → {}", "→".dimmed(), brief_path.display());
    println!();
    println!("{}", "Next steps:".bold());
    println!("   1. Paste {} into your new AI session", brief_path.display());

    Ok(())
}

pub fn run_fix() -> Result<()> {
    println!("{} Digital Janitor: Reviewing technical debt...", "→".bright_magenta());

    let brain = active_brain().ok_or_else(|| anyhow::anyhow!("No Brain found."))?;

    // 1. Find latest reflection/seed
    let dream_dir = brain.join("dream");
    if !dream_dir.exists() {
        anyhow::bail!("No dream data found. Run 'cerebrum audit' first.");
    }

    let mut files: Vec<_> = fs::read_dir(&dream_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().map_or(false, |ext| ext == "md")
                && e.file_name() != "Audit-Prompt.md"
                && e.file_name() != "Session-Brief.md"
        })
        .collect();

    files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    let latest = files
        .last()
        .ok_or_else(|| anyhow::anyhow!("No reflection files found. (Ignored Audit-Prompt.md)"))?;

    let content = fs::read_to_string(latest.path())?;
    let sections = parse_reflection_sections(&content);

    let suggestions = sections
        .get("prune suggestions")
        .ok_or_else(|| anyhow::anyhow!("No 'Prune Suggestions' found in latest reflection."))?;

    println!(
        "\n{} {} actionable items identified in the latest audit:\n",
        "✧".bright_cyan(),
        suggestions.len()
    );

    for (i, sug) in suggestions.iter().enumerate() {
        println!(
            "  {} {} {}",
            (i + 1).to_string().bright_black(),
            "[ ]".yellow(),
            sug
        );
    }

    println!(
        "\n{} {} Use 'cerebrum brief' for full situational awareness.",
        "→".dimmed(),
        "Pro-tip:".bold()
    );

    Ok(())
}

pub fn run_map(graph: bool) -> Result<()> {
    if graph {
        return run_map_graph();
    }

    println!("{} Mapping the Brain...", "→".bright_magenta());

    let brain = active_brain().ok_or_else(|| anyhow::anyhow!("No Brain found."))?;

    let cwd = std::env::current_dir()?;
    let project_name = cwd.file_name().unwrap_or_default().to_string_lossy();

    // 1. Gather Data
    let insights = get_latest_content(brain.join("memory/semantic")).unwrap_or_default();
    let rules = get_latest_content(brain.join("memory/procedural/skills")).unwrap_or_default();
    let equity = get_latest_content(brain.join("memory/personal")).unwrap_or_default();

    let mut episodes = vec![];
    episodes.extend(load_episodes(&brain));
    episodes.sort_by(|a, b| b.0.cmp(&a.0));

    let history_html = episodes
        .iter()
        .take(10)
        .map(|(path, text)| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            format!(
                r#"<div class="history-item"><span class="timestamp">{}</span><span>{}</span></div>"#,
                name.strip_suffix(".md").unwrap_or(&name),
                text.lines().next().unwrap_or("").trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let insights_html = insights
        .lines()
        .filter(|l| l.starts_with("- ") || l.starts_with("• "))
        .map(|l| format!("<li>{}</li>", if l.len() > 2 { &l[2..] } else { "" }))
        .collect::<Vec<_>>()
        .join("\n");

    let rules_html = rules
        .lines()
        .filter(|l| l.starts_with("- ") || l.starts_with("• "))
        .map(|l| format!("<li>{}</li>", if l.len() > 2 { &l[2..] } else { "" }))
        .collect::<Vec<_>>()
        .join("\n");

    let equity_html = equity
        .lines()
        .filter(|l| l.starts_with("- ") || l.starts_with("• "))
        .map(|l| format!("<li>{}</li>", if l.len() > 2 { &l[2..] } else { "" }))
        .collect::<Vec<_>>()
        .join("\n");

    // 2. Build HTML
    let template = include_str!("../templates/brain_map.html")
        .replace("{{project_name}}", &project_name)
        .replace("{{insights}}", &insights_html)
        .replace("{{rules}}", &rules_html)
        .replace("{{equity}}", &equity_html)
        .replace("{{history}}", &history_html);

    let output_path = brain.join("brain_map.html");
    fs::write(&output_path, template)?;

    println!(
        "{} Brain Map generated → {}",
        "✓".green(),
        output_path.display()
    );

    // 3. Open it (Mac only)
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg(&output_path)
        .spawn();

    Ok(())
}

pub fn run_map_graph() -> Result<()> {
    println!("{} Weaving the neural map...", "→".bright_magenta());

    let brain = active_brain().ok_or_else(|| anyhow::anyhow!("No Brain found."))?;

    let cwd = std::env::current_dir()?;
    let project_name = cwd.file_name().unwrap_or_default().to_string_lossy();

    let data = build_graph_data(&brain)?;
    println!(
        "   {} {} memories · {} inferred connections",
        "→".dimmed(),
        data.nodes.len().to_string().green(),
        data.edges.len().to_string().green()
    );

    // Escape "</" so a memory containing "</script>" can't break out of the tag
    let json = serde_json::to_string(&data)?.replace("</", "<\\/");

    let template = include_str!("../templates/brain_3d.html")
        .replace("{{project_name}}", &project_name)
        .replace("{{graph_data}}", &json);

    let output_path = brain.join("neural_map.html");
    fs::write(&output_path, template)?;

    println!(
        "{} Neural Map generated → {}",
        "✓".green(),
        output_path.display()
    );

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&output_path).spawn();

    Ok(())
}
