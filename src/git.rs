use anyhow::Result;
use colored::*;
use std::fs;

const HOOK_MARKER: &str = "# cerebrumma-hook";

const HOOK_SCRIPT: &str = r#"#!/bin/sh
# cerebrumma-hook
# Commit fields are passed as environment variables. The cerebrum command line
# itself contains no commit message, filenames, or diff stats, so those values
# are not re-parsed by the shell.
BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
HASH=$(git rev-parse --short HEAD 2>/dev/null)
MSG=$(git log -1 --pretty=%B 2>/dev/null | head -3 | tr '\n' ' ')
ALL=$(git diff-tree --root --no-commit-id -r --name-only HEAD 2>/dev/null | grep -v "^\.cerebrum/")
COUNT=$(printf '%s\n' "$ALL" | grep -c .)
FILES=$(printf '%s\n' "$ALL" | head -5 | tr '\n' ' ')
[ "$COUNT" -gt 5 ] && FILES="${FILES}(+$((COUNT - 5)) more) "
STATS=$(git diff-tree --root --no-commit-id -r --stat HEAD 2>/dev/null | tail -1)
CEREBRUMMA_HOOK_BRANCH="$BRANCH" \
CEREBRUMMA_HOOK_HASH="$HASH" \
CEREBRUMMA_HOOK_MSG="$MSG" \
CEREBRUMMA_HOOK_FILES="$FILES" \
CEREBRUMMA_HOOK_STATS="$STATS" \
cerebrum add --from-hook
"#;

pub fn format_hook_memory(branch: &str, hash: &str, msg: &str, files: &str, stats: &str) -> String {
    format!("commit {hash} on {branch}: {msg}| files: {files}| {stats}")
}

pub fn hook_entry_from_env() -> Result<String> {
    let branch = std::env::var("CEREBRUMMA_HOOK_BRANCH").unwrap_or_default();
    let hash = std::env::var("CEREBRUMMA_HOOK_HASH").unwrap_or_default();
    let msg = std::env::var("CEREBRUMMA_HOOK_MSG").unwrap_or_default();
    let files = std::env::var("CEREBRUMMA_HOOK_FILES").unwrap_or_default();
    let stats = std::env::var("CEREBRUMMA_HOOK_STATS").unwrap_or_default();
    if hash.trim().is_empty() && msg.trim().is_empty() {
        anyhow::bail!(
            "hook payload missing. Expected CEREBRUMMA_HOOK_HASH or CEREBRUMMA_HOOK_MSG."
        );
    }
    Ok(format_hook_memory(
        branch.trim(),
        hash.trim(),
        msg.trim(),
        files.trim(),
        stats.trim(),
    ))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_script_does_not_put_the_commit_message_on_the_command_line() {
        assert!(HOOK_SCRIPT.contains("cerebrum add --from-hook"));
        assert!(HOOK_SCRIPT.contains("CEREBRUMMA_HOOK_MSG"));
        assert!(!HOOK_SCRIPT.contains("cerebrum add \""));
    }

    #[test]
    fn hook_memory_keeps_quotes_and_substitutions_as_text() {
        let text = format_hook_memory(
            "main",
            "abc123",
            "fix\"; $(reboot)",
            "src/$(id).rs",
            "1 file changed",
        );
        assert_eq!(
            text,
            "commit abc123 on main: fix\"; $(reboot)| files: src/$(id).rs| 1 file changed"
        );
    }
}
