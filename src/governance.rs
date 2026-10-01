use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::*;
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::memory::active_brain;

const SNAPSHOT_SCHEMA_VERSION: &str = "cerebrumma.business-brain.v1";
const VALID_STATUSES: &[&str] = &["draft", "approved", "deprecated", "rejected"];
const VALID_AUTONOMY_LEVELS: &[&str] = &["observe", "draft", "act", "autopilot"];

#[derive(Debug, Clone, Serialize)]
pub struct GovernedEntry {
    pub id: String,
    pub source_path: String,
    pub layer: String,
    pub status: String,
    pub authority: String,
    pub owner: String,
    pub scope: Vec<String>,
    pub customer_safe: bool,
    pub autonomy_level: String,
    pub effective_at: Option<String>,
    pub expires_at: Option<String>,
    pub supersedes: Vec<String>,
    pub conflict_key: Option<String>,
    pub provenance: Option<String>,
    pub content: String,
    pub checksum: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    entry: GovernedEntry,
    runtime: bool,
    reviewed_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub governed_entries: usize,
    pub publishable_entries: usize,
    pub errors: usize,
    pub warnings: usize,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Serialize)]
struct RuntimeSnapshot {
    schema_version: &'static str,
    brain_version: String,
    generated_at: String,
    entry_count: usize,
    entries: Vec<GovernedEntry>,
}

fn yaml_key(name: &str) -> Value {
    Value::String(name.to_string())
}

fn string_value(map: &Mapping, name: &str) -> Option<String> {
    map.get(&yaml_key(name))
        .and_then(|value| match value {
            Value::String(value) => Some(value.trim().to_string()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

fn bool_value(map: &Mapping, name: &str) -> Option<bool> {
    map.get(&yaml_key(name)).and_then(Value::as_bool)
}

fn string_list(map: &Mapping, name: &str) -> Vec<String> {
    match map.get(&yaml_key(name)) {
        Some(Value::Sequence(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        Some(Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => vec![],
    }
}

fn split_frontmatter(text: &str) -> Result<(Mapping, String)> {
    if !text.starts_with("---") {
        return Ok((Mapping::new(), text.trim().to_string()));
    }

    let rest = text.strip_prefix("---").unwrap_or(text);
    let delimiter = rest
        .find("\n---")
        .context("frontmatter opens with --- but has no closing ---")?;
    let yaml = &rest[..delimiter];
    let body = rest[delimiter + 4..].trim();
    let value: Value = serde_yaml::from_str(yaml).context("invalid YAML frontmatter")?;
    let map = value.as_mapping().cloned().unwrap_or_default();
    let content = if body.is_empty() {
        string_value(&map, "content").unwrap_or_default()
    } else {
        body.to_string()
    };
    Ok((map, content))
}

fn layer_for(relative_path: &Path) -> String {
    let mut components = relative_path
        .components()
        .map(|part| part.as_os_str().to_string_lossy());
    let _memory = components.next();
    components
        .next()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "other".to_string())
}

fn checksum(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

fn parse_candidate(brain: &Path, path: &Path) -> Result<Option<Candidate>> {
    let text = fs::read_to_string(path)?;
    let (map, content) = split_frontmatter(&text)?;
    let runtime = bool_value(&map, "runtime").unwrap_or(false);
    let has_governance = runtime
        || ["status", "authority", "owner", "scope", "autonomy_level"]
            .iter()
            .any(|key| map.contains_key(&yaml_key(key)));
    if !has_governance {
        return Ok(None);
    }

    let relative = path.strip_prefix(brain).unwrap_or(path);
    let source_path = relative.to_string_lossy().replace('\\', "/");
    let id = string_value(&map, "id").unwrap_or_else(|| source_path.clone());
    let status = string_value(&map, "status").unwrap_or_else(|| "draft".to_string());
    let authority = string_value(&map, "authority").unwrap_or_default();
    let owner = string_value(&map, "owner").unwrap_or_default();
    let scope = string_list(&map, "scope");
    let customer_safe = bool_value(&map, "customer_safe").unwrap_or(false);
    let autonomy_level =
        string_value(&map, "autonomy_level").unwrap_or_else(|| "observe".to_string());
    let provenance = string_value(&map, "provenance");

    Ok(Some(Candidate {
        entry: GovernedEntry {
            id,
            source_path,
            layer: layer_for(relative),
            status,
            authority,
            owner,
            scope,
            customer_safe,
            autonomy_level,
            effective_at: string_value(&map, "effective_at"),
            expires_at: string_value(&map, "expires_at"),
            supersedes: string_list(&map, "supersedes"),
            conflict_key: string_value(&map, "conflict_key"),
            provenance,
            checksum: checksum(&content),
            content,
        },
        runtime,
        reviewed_by: string_value(&map, "reviewed_by"),
    }))
}

fn is_expired(value: Option<&str>, now: DateTime<Utc>) -> bool {
    value
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|date| date.with_timezone(&Utc) <= now)
        .unwrap_or(false)
}

fn is_effective(value: Option<&str>, now: DateTime<Utc>) -> bool {
    value
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|date| date.with_timezone(&Utc) <= now)
        .unwrap_or(true)
}

fn load_candidates(brain: &Path) -> (Vec<Candidate>, Vec<ValidationIssue>) {
    let memory = brain.join("memory");
    let mut candidates = vec![];
    let mut issues = vec![];

    for entry in WalkDir::new(&memory)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }
        match parse_candidate(brain, path) {
            Ok(Some(candidate)) => candidates.push(candidate),
            Ok(None) => {}
            Err(error) => issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid_frontmatter".to_string(),
                path: path
                    .strip_prefix(brain)
                    .unwrap_or(path)
                    .display()
                    .to_string(),
                message: error.to_string(),
            }),
        }
    }
    candidates.sort_by(|a, b| a.entry.id.cmp(&b.entry.id));
    (candidates, issues)
}

fn push_issue(
    issues: &mut Vec<ValidationIssue>,
    severity: Severity,
    code: &str,
    entry: &GovernedEntry,
    message: String,
) {
    issues.push(ValidationIssue {
        severity,
        code: code.to_string(),
        path: entry.source_path.clone(),
        message,
    });
}

fn validate_candidates(
    candidates: &[Candidate],
    mut issues: Vec<ValidationIssue>,
) -> ValidationReport {
    let now = Utc::now();
    let mut ids = HashSet::new();
    let all_ids: HashSet<&str> = candidates
        .iter()
        .map(|candidate| candidate.entry.id.as_str())
        .collect();
    let mut active_conflicts: HashMap<(&str, &str), &GovernedEntry> = HashMap::new();

    for candidate in candidates {
        let entry = &candidate.entry;
        if !ids.insert(entry.id.clone()) {
            push_issue(
                &mut issues,
                Severity::Error,
                "duplicate_id",
                entry,
                format!("duplicate governed-memory id '{}'", entry.id),
            );
        }
        if !VALID_STATUSES.contains(&entry.status.as_str()) {
            push_issue(
                &mut issues,
                Severity::Error,
                "invalid_status",
                entry,
                format!("status must be one of: {}", VALID_STATUSES.join(", ")),
            );
        }
        if !VALID_AUTONOMY_LEVELS.contains(&entry.autonomy_level.as_str()) {
            push_issue(
                &mut issues,
                Severity::Error,
                "invalid_autonomy",
                entry,
                format!(
                    "autonomy_level must be one of: {}",
                    VALID_AUTONOMY_LEVELS.join(", ")
                ),
            );
        }
        if candidate.runtime && entry.status == "approved" {
            for (field, missing) in [
                ("authority", entry.authority.is_empty()),
                ("owner", entry.owner.is_empty()),
                ("scope", entry.scope.is_empty()),
                ("content", entry.content.is_empty()),
            ] {
                if missing {
                    push_issue(
                        &mut issues,
                        Severity::Error,
                        "missing_required_field",
                        entry,
                        format!("approved runtime entry is missing {field}"),
                    );
                }
            }
        }
        if entry
            .provenance
            .as_deref()
            .unwrap_or_default()
            .contains("dream")
            && entry.status == "approved"
            && candidate.reviewed_by.is_none()
        {
            push_issue(
                &mut issues,
                Severity::Error,
                "unreviewed_reflection",
                entry,
                "dream-generated knowledge cannot be approved without reviewed_by".to_string(),
            );
        }
        for superseded in &entry.supersedes {
            if !all_ids.contains(superseded.as_str()) {
                push_issue(
                    &mut issues,
                    Severity::Error,
                    "unknown_supersedes",
                    entry,
                    format!("supersedes unknown id '{superseded}'"),
                );
            }
        }
        if entry.expires_at.is_some()
            && DateTime::parse_from_rfc3339(entry.expires_at.as_deref().unwrap()).is_err()
        {
            push_issue(
                &mut issues,
                Severity::Error,
                "invalid_expiry",
                entry,
                "expires_at must be RFC 3339".to_string(),
            );
        } else if entry.status == "approved" && is_expired(entry.expires_at.as_deref(), now) {
            push_issue(
                &mut issues,
                Severity::Warning,
                "expired_approval",
                entry,
                "approved entry is expired and will not be published".to_string(),
            );
        }
        if entry.effective_at.is_some()
            && DateTime::parse_from_rfc3339(entry.effective_at.as_deref().unwrap()).is_err()
        {
            push_issue(
                &mut issues,
                Severity::Error,
                "invalid_effective_at",
                entry,
                "effective_at must be RFC 3339".to_string(),
            );
        }

        if candidate.runtime
            && entry.status == "approved"
            && !is_expired(entry.expires_at.as_deref(), now)
            && is_effective(entry.effective_at.as_deref(), now)
        {
            if let Some(key) = entry.conflict_key.as_deref() {
                for scope in &entry.scope {
                    let conflict = (key, scope.as_str());
                    if let Some(previous) = active_conflicts.insert(conflict, entry) {
                        if !entry.supersedes.contains(&previous.id)
                            && !previous.supersedes.contains(&entry.id)
                        {
                            push_issue(&mut issues, Severity::Error, "active_conflict", entry,
                                format!("conflicts with '{}' for key '{}' and scope '{}' without supersession", previous.id, key, scope));
                        }
                    }
                }
            }
        }
    }

    issues.sort_by(|a, b| a.path.cmp(&b.path).then(a.code.cmp(&b.code)));
    let errors = issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .count();
    let warnings = issues.len() - errors;
    let superseded_ids: HashSet<&str> = candidates
        .iter()
        .filter(|candidate| {
            candidate.runtime
                && candidate.entry.status == "approved"
                && !is_expired(candidate.entry.expires_at.as_deref(), now)
                && is_effective(candidate.entry.effective_at.as_deref(), now)
        })
        .flat_map(|candidate| candidate.entry.supersedes.iter().map(String::as_str))
        .collect();
    let publishable_entries = candidates
        .iter()
        .filter(|candidate| {
            candidate.runtime
                && candidate.entry.status == "approved"
                && !superseded_ids.contains(candidate.entry.id.as_str())
                && !is_expired(candidate.entry.expires_at.as_deref(), now)
                && is_effective(candidate.entry.effective_at.as_deref(), now)
        })
        .count();
    ValidationReport {
        valid: errors == 0,
        governed_entries: candidates.len(),
        publishable_entries,
        errors,
        warnings,
        issues,
    }
}

pub fn validate_brain(brain: &Path) -> ValidationReport {
    let (candidates, issues) = load_candidates(brain);
    validate_candidates(&candidates, issues)
}

pub fn run_validate(json: bool) -> Result<()> {
    let brain = active_brain().context("No Brain found. Run `cerebrum init` first.")?;
    let report = validate_brain(&brain);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let badge = if report.valid {
            "✓".green().bold()
        } else {
            "✗".red().bold()
        };
        println!("{} Business Brain validation", badge);
        println!(
            "   governed: {}  publishable: {}  errors: {}  warnings: {}",
            report.governed_entries, report.publishable_entries, report.errors, report.warnings
        );
        for issue in &report.issues {
            let label = match issue.severity {
                Severity::Error => "ERROR".red().bold(),
                Severity::Warning => "WARN".yellow().bold(),
            };
            println!(
                "   {} {} [{}] {}",
                label, issue.path, issue.code, issue.message
            );
        }
    }
    if !report.valid {
        bail!(
            "Business Brain validation failed with {} error(s)",
            report.errors
        );
    }
    Ok(())
}

fn publishable_entries(brain: &Path) -> Result<Vec<GovernedEntry>> {
    let (candidates, issues) = load_candidates(brain);
    let report = validate_candidates(&candidates, issues);
    if !report.valid {
        bail!(
            "Business Brain validation failed with {} error(s); run `cerebrum validate`",
            report.errors
        );
    }
    let now = Utc::now();
    let superseded_ids: HashSet<String> = candidates
        .iter()
        .filter(|candidate| {
            candidate.runtime
                && candidate.entry.status == "approved"
                && !is_expired(candidate.entry.expires_at.as_deref(), now)
                && is_effective(candidate.entry.effective_at.as_deref(), now)
        })
        .flat_map(|candidate| candidate.entry.supersedes.iter().cloned())
        .collect();
    Ok(candidates
        .into_iter()
        .filter(|candidate| {
            candidate.runtime
                && candidate.entry.status == "approved"
                && !superseded_ids.contains(&candidate.entry.id)
                && !is_expired(candidate.entry.expires_at.as_deref(), now)
                && is_effective(candidate.entry.effective_at.as_deref(), now)
        })
        .map(|candidate| candidate.entry)
        .collect())
}

pub fn publish_runtime(output: Option<PathBuf>) -> Result<()> {
    let brain = active_brain().context("No Brain found. Run `cerebrum init` first.")?;
    let entries = publishable_entries(&brain)?;
    if entries.is_empty() {
        bail!("No approved runtime entries found. Mark governed entries with runtime: true and status: approved.");
    }

    let canonical = serde_json::to_vec(&entries)?;
    let brain_version = checksum(std::str::from_utf8(&canonical)?);
    let snapshot = RuntimeSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        brain_version: brain_version.clone(),
        generated_at: Utc::now().to_rfc3339(),
        entry_count: entries.len(),
        entries,
    };
    let output = output.unwrap_or_else(|| brain.join("runtime/brain.json"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&snapshot)?),
    )?;
    println!(
        "{} Published Business Brain {}",
        "✓".green().bold(),
        &brain_version[..12]
    );
    println!("   {} entries → {}", snapshot.entry_count, output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, conflict_key: Option<&str>) -> Candidate {
        Candidate {
            runtime: true,
            reviewed_by: None,
            entry: GovernedEntry {
                id: id.to_string(),
                source_path: format!("memory/semantic/{id}.md"),
                layer: "semantic".to_string(),
                status: "approved".to_string(),
                authority: "product_truth".to_string(),
                owner: "founder".to_string(),
                scope: vec!["sales".to_string()],
                customer_safe: true,
                autonomy_level: "draft".to_string(),
                effective_at: None,
                expires_at: None,
                supersedes: vec![],
                conflict_key: conflict_key.map(ToOwned::to_owned),
                provenance: Some("manual".to_string()),
                content: "A verified fact".to_string(),
                checksum: checksum("A verified fact"),
            },
        }
    }

    #[test]
    fn rejects_two_active_truths_for_the_same_key_and_scope() {
        let report = validate_candidates(
            &[
                candidate("pricing-v1", Some("pricing.trial_days")),
                candidate("pricing-v2", Some("pricing.trial_days")),
            ],
            vec![],
        );
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "active_conflict"));
    }

    #[test]
    fn accepts_explicit_supersession() {
        let old = candidate("pricing-v1", Some("pricing.trial_days"));
        let mut new = candidate("pricing-v2", Some("pricing.trial_days"));
        new.entry.supersedes.push(old.entry.id.clone());
        let report = validate_candidates(&[old, new], vec![]);
        assert!(report.valid);
        assert_eq!(report.publishable_entries, 1);
    }

    #[test]
    fn dream_knowledge_requires_human_review_before_approval() {
        let mut generated = candidate("dream-fact", None);
        generated.entry.provenance = Some("dream-ingest:reflection.md".to_string());
        let report = validate_candidates(&[generated], vec![]);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "unreviewed_reflection"));
    }

    #[test]
    fn parses_body_style_and_embedded_content_style() {
        let (body_map, body) =
            split_frontmatter("---\nid: body\nstatus: draft\n---\n\nBody text").unwrap();
        assert_eq!(string_value(&body_map, "id").as_deref(), Some("body"));
        assert_eq!(body, "Body text");

        let (_, embedded) =
            split_frontmatter("---\nid: embedded\ncontent: Embedded text\n---\n").unwrap();
        assert_eq!(embedded, "Embedded text");
    }
}
