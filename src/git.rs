use anyhow::Result;
use colored::*;
use std::fs;

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

pub fn hook_install() -> Result<()> {
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
    }

    fs::write(&hook_path, HOOK_SCRIPT)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

pub fn hook_remove() -> Result<()> {
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

pub fn get_current_branch() -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn get_git_status() -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["status", "--short"])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
