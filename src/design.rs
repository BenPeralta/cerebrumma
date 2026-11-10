//! Named mood boards for the design-taste layer.
//!
//! Boards are Markdown files with YAML frontmatter under `memory/design/`.
//! The schema is `cerebrumma.design-board.v1` (see `docs/design-layer.md`).
//! Dream output never writes this directory; it only leaves proposals behind.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::*;
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::cli::{DesignAction, DesignExportFormat, DesignMix};
use crate::memory::{global_brain_path, local_brain_path, resolve_brain, ts_stem};

pub const DESIGN_SCHEMA: &str = "cerebrumma.design-board.v1";
pub const PROPOSAL_SCHEMA: &str = "cerebrumma.design-proposal.v1";
pub const INDEX_SCHEMA: &str = "cerebrumma.design-index.v1";

const CORE_PALETTE: [&str; 4] = ["background", "surface", "ink", "accent"];
const AGENT_INSTRUCTIONS: &str = "Apply these tokens when generating UI. Palette, typography, spacing, radius, shadows, and motion are authoritative. Component notes, vibe, and guidance are taste, not a second palette. Do not invent a parallel color or type system. When mixed_from is set, take each listed facet from that board and everything else from this board.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainSide {
    Local,
    Global,
}

impl BrainSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Local => 0,
            Self::Global => 1,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Typography {
    pub families: BTreeMap<String, String>,
    pub scale: BTreeMap<String, String>,
    pub weights: BTreeMap<String, u16>,
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Motion {
    pub feel: String,
    pub duration: BTreeMap<String, String>,
    pub easing: BTreeMap<String, String>,
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesignReference {
    pub kind: String,
    pub value: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesignBoard {
    pub schema: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub is_default: bool,
    pub palette: BTreeMap<String, String>,
    pub typography: Typography,
    pub spacing: BTreeMap<String, String>,
    pub radius: BTreeMap<String, String>,
    pub shadows: BTreeMap<String, String>,
    pub motion: Motion,
    pub components: BTreeMap<String, String>,
    pub vibe: String,
    pub references: Vec<DesignReference>,
    pub provenance: String,
    pub created_at: String,
    pub updated_at: String,
    pub source_tool: String,
    pub extensions: BTreeMap<String, Value>,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct StoredBoard {
    pub board: DesignBoard,
    pub side: BrainSide,
    pub relative_path: String,
}

#[derive(Debug, Clone)]
pub struct DesignProposal {
    pub schema: String,
    pub status: String,
    pub action: String,
    pub target: String,
    pub note: String,
    pub provenance: String,
    pub created_at: String,
    pub reviewed_by: String,
    pub applied_at: String,
    pub body: String,
}

#[derive(Debug, Clone)]
struct TasteNote {
    action: String,
    target: String,
    note: String,
}

#[derive(Debug, Clone, Default)]
pub struct BoardInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub palette: BTreeMap<String, String>,
    pub families: BTreeMap<String, String>,
    pub scale: BTreeMap<String, String>,
    pub weights: BTreeMap<String, String>,
    pub spacing: BTreeMap<String, String>,
    pub radius: BTreeMap<String, String>,
    pub shadows: BTreeMap<String, String>,
    pub feel: Option<String>,
    pub durations: BTreeMap<String, String>,
    pub easings: BTreeMap<String, String>,
    pub components: BTreeMap<String, String>,
    pub vibe: Option<String>,
    pub references: Vec<DesignReference>,
    pub guidance: Option<String>,
    pub make_default: bool,
    pub replace: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedDesign {
    pub board: DesignBoard,
    pub side: BrainSide,
    pub mixed_from: BTreeMap<String, FacetSource>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacetSource {
    pub slug: String,
    pub brain: String,
}

pub fn design_dir(brain: &Path) -> PathBuf {
    brain.join("memory/design")
}

pub fn slugify(name: &str) -> Result<String> {
    let mut slug = String::new();
    let mut hyphen = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            hyphen = false;
        } else if !slug.is_empty() && !hyphen {
            slug.push('-');
            hyphen = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if !is_slug(&slug) {
        bail!("name '{name}' cannot be turned into a board slug");
    }
    Ok(slug)
}

fn is_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug
            .split('-')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub fn parse_board(text: &str) -> Result<DesignBoard> {
    if text.len() > 65_536 {
        bail!("design board is larger than 64KB");
    }
    let (value, body) = split_frontmatter(text)?;
    let map = value
        .as_mapping()
        .context("design board frontmatter must be a YAML mapping")?
        .clone();
    board_from_mapping(map, body)
}

fn board_from_mapping(mut map: Mapping, body: String) -> Result<DesignBoard> {
    let schema = take_string(&mut map, "schema")?.unwrap_or_else(|| DESIGN_SCHEMA.to_string());
    if schema != DESIGN_SCHEMA {
        bail!("unsupported design schema '{schema}' (expected {DESIGN_SCHEMA})");
    }
    let name = take_string(&mut map, "name")?.context("design board is missing name")?;
    let explicit_slug = take_string(&mut map, "slug")?;
    let slug = match explicit_slug {
        Some(slug) => slug,
        None => slugify(&name)?,
    };
    if !is_slug(&slug) {
        bail!("invalid board slug '{slug}'");
    }
    let description = take_string(&mut map, "description")?.unwrap_or_default();
    let is_default = take_bool(&mut map, "default")?.unwrap_or(false);
    let palette = take_string_map(&mut map, "palette")?;
    let typography = take_typography(&mut map)?;
    let spacing = take_string_map(&mut map, "spacing")?;
    let radius = take_string_map(&mut map, "radius")?;
    let shadows = take_string_map(&mut map, "shadows")?;
    let motion = take_motion(&mut map)?;
    let components = take_string_map(&mut map, "components")?;
    let vibe = take_string(&mut map, "vibe")?.unwrap_or_default();
    let references = take_references(&mut map)?;
    let provenance = take_string(&mut map, "provenance")?.unwrap_or_else(|| "manual".to_string());
    let created_at = take_string(&mut map, "created_at")?.unwrap_or_default();
    let updated_at = take_string(&mut map, "updated_at")?.unwrap_or_else(|| created_at.clone());
    let source_tool = take_string(&mut map, "source_tool")?.unwrap_or_else(|| "cli".to_string());
    let extensions = take_extensions(map)?;
    let board = DesignBoard {
        schema,
        name,
        slug,
        description,
        is_default,
        palette,
        typography,
        spacing,
        radius,
        shadows,
        motion,
        components,
        vibe,
        references,
        provenance,
        created_at,
        updated_at,
        source_tool,
        extensions,
        body,
    };
    validate_board(&board)?;
    Ok(board)
}

pub fn validate_board(board: &DesignBoard) -> Result<()> {
    if board.schema != DESIGN_SCHEMA {
        bail!("unsupported design schema '{}'", board.schema);
    }
    require_text(&board.name, 1, 80, "name")?;
    if !is_slug(&board.slug) {
        bail!("invalid board slug '{}'", board.slug);
    }
    require_text(&board.description, 0, 600, "description")?;
    require_text(&board.vibe, 0, 600, "vibe")?;
    require_text(&board.body, 0, 8_000, "guidance")?;
    require_text(&board.provenance, 1, 200, "provenance")?;
    if board.provenance.chars().any(|c| c.is_control()) {
        bail!("provenance must be a single line");
    }
    require_text(&board.source_tool, 1, 40, "source_tool")?;
    if !board
        .source_tool
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("invalid source_tool '{}'", board.source_tool);
    }
    require_timestamp(&board.created_at, "created_at")?;
    require_timestamp(&board.updated_at, "updated_at")?;
    if board.palette.is_empty() {
        bail!("palette must include at least one color");
    }
    if board.palette.len() > 32 {
        bail!("palette has too many roles");
    }
    for (role, hex) in &board.palette {
        require_role(role)?;
        normalize_hex(hex).with_context(|| format!("palette.{role}"))?;
        if normalize_hex(hex)? != *hex {
            bail!("palette.{role} must be normalized hex ({hex})");
        }
    }
    validate_families(&board.typography.families)?;
    validate_length_map(&board.typography.scale, "typography.scale")?;
    validate_weights(&board.typography.weights)?;
    validate_length_map(&board.spacing, "spacing")?;
    validate_length_map(&board.radius, "radius")?;
    validate_shadows(&board.shadows)?;
    validate_feel(&board.motion.feel)?;
    validate_duration_map(&board.motion.duration)?;
    validate_easing_map(&board.motion.easing)?;
    validate_components(&board.components)?;
    if board.references.len() > 24 {
        bail!("too many references");
    }
    for reference in &board.references {
        validate_reference(reference)?;
    }
    validate_extension_map(&board.extensions)?;
    validate_extension_map(&board.typography.extensions)?;
    validate_extension_map(&board.motion.extensions)?;
    Ok(())
}

pub fn core_palette_warnings(board: &DesignBoard) -> Vec<String> {
    CORE_PALETTE
        .into_iter()
        .filter(|role| !board.palette.contains_key(*role))
        .map(|role| format!("palette is missing recommended role '{role}'"))
        .collect()
}

pub fn normalize_hex(raw: &str) -> Result<String> {
    let value = raw.trim();
    let digits = value
        .strip_prefix('#')
        .context("color must be a quoted #hex value")?;
    let expanded: String = match digits.len() {
        3 | 4 => digits.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => digits.to_string(),
        _ => bail!("color '{raw}' must be #RGB, #RGBA, #RRGGBB, or #RRGGBBAA"),
    };
    if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("color '{raw}' is not hexadecimal");
    }
    Ok(format!("#{}", expanded.to_ascii_uppercase()))
}

fn render_board(board: &DesignBoard) -> Result<String> {
    validate_board(board)?;
    let mut out = String::from("---\n");
    emit_scalar_line(&mut out, 0, "schema", &yaml_string(&board.schema));
    emit_scalar_line(&mut out, 0, "name", &yaml_string(&board.name));
    emit_scalar_line(&mut out, 0, "slug", &yaml_string(&board.slug));
    emit_scalar_line(&mut out, 0, "description", &yaml_string(&board.description));
    emit_scalar_line(
        &mut out,
        0,
        "default",
        if board.is_default { "true" } else { "false" },
    );
    emit_string_map(&mut out, 0, "palette", &board.palette)?;
    emit_typography(&mut out, &board.typography)?;
    emit_string_map(&mut out, 0, "spacing", &board.spacing)?;
    emit_string_map(&mut out, 0, "radius", &board.radius)?;
    emit_string_map(&mut out, 0, "shadows", &board.shadows)?;
    emit_motion(&mut out, &board.motion)?;
    emit_string_map(&mut out, 0, "components", &board.components)?;
    if !board.vibe.is_empty() {
        emit_scalar_line(&mut out, 0, "vibe", &yaml_string(&board.vibe));
    }
    emit_references(&mut out, &board.references)?;
    emit_scalar_line(&mut out, 0, "provenance", &yaml_string(&board.provenance));
    emit_scalar_line(&mut out, 0, "created_at", &yaml_string(&board.created_at));
    emit_scalar_line(&mut out, 0, "updated_at", &yaml_string(&board.updated_at));
    emit_scalar_line(&mut out, 0, "source_tool", &yaml_string(&board.source_tool));
    emit_extensions(&mut out, 0, &board.extensions)?;
    out.push_str("---\n");
    if !board.body.trim().is_empty() {
        out.push('\n');
        out.push_str(board.body.trim_end());
        out.push('\n');
    }
    Ok(out)
}

pub fn load_side(brain: &Path, side: BrainSide) -> Result<Vec<StoredBoard>> {
    let dir = design_dir(brain);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut boards = vec![];
    let mut seen = BTreeMap::<String, String>::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .with_context(|| format!("could not read {}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string();
        let text = fs::read_to_string(&path)
            .with_context(|| format!("could not read {}", path.display()))?;
        let board = parse_board(&text)
            .with_context(|| format!("invalid design board {}", path.display()))?;
        if board.slug != stem {
            bail!(
                "{} must be named {}.md so the filename matches its slug",
                path.display(),
                board.slug
            );
        }
        if let Some(previous) = seen.insert(board.slug.clone(), path.display().to_string()) {
            bail!("duplicate design board '{}' in {previous}", board.slug);
        }
        boards.push(StoredBoard {
            relative_path: format!("memory/design/{stem}.md"),
            side,
            board,
        });
    }
    Ok(boards)
}

pub fn merge_boards(local: Vec<StoredBoard>, global: Vec<StoredBoard>) -> Vec<StoredBoard> {
    let mut merged: BTreeMap<String, StoredBoard> = BTreeMap::new();
    for board in global {
        merged.insert(board.board.slug.clone(), board);
    }
    for board in local {
        merged.insert(board.board.slug.clone(), board);
    }
    merged.into_values().collect()
}

pub fn load_merged(local: Option<&Path>, global: Option<&Path>) -> Result<Vec<StoredBoard>> {
    let local_boards = match local {
        Some(path) if path.exists() => load_side(path, BrainSide::Local)?,
        _ => vec![],
    };
    let global_boards = match global {
        Some(path) if path.exists() => load_side(path, BrainSide::Global)?,
        _ => vec![],
    };
    Ok(merge_boards(local_boards, global_boards))
}

pub fn effective_default(boards: &[StoredBoard]) -> Option<&StoredBoard> {
    let mut defaults: Vec<&StoredBoard> = boards
        .iter()
        .filter(|board| board.board.is_default)
        .collect();
    if defaults.is_empty() {
        return None;
    }
    defaults.sort_by(|left, right| {
        left.side
            .rank()
            .cmp(&right.side.rank())
            .then(right.board.updated_at.cmp(&left.board.updated_at))
            .then(left.board.slug.cmp(&right.board.slug))
    });
    Some(defaults[0])
}

#[derive(Debug, Clone, Default)]
pub struct MixSpec {
    pub palette: Option<String>,
    pub typography: Option<String>,
    pub spacing: Option<String>,
    pub radius: Option<String>,
    pub elevation: Option<String>,
    pub motion: Option<String>,
}

impl MixSpec {
    fn from_cli(mix: &DesignMix) -> Self {
        Self {
            palette: mix.palette.clone(),
            typography: mix.type_board.clone(),
            spacing: mix.spacing.clone(),
            radius: mix.radius.clone(),
            elevation: mix.elevation.clone(),
            motion: mix.motion.clone(),
        }
    }
}

pub fn resolve(
    boards: &[StoredBoard],
    name: Option<&str>,
    mix: &MixSpec,
) -> Result<ResolvedDesign> {
    let base = match name.map(str::trim).filter(|value| !value.is_empty()) {
        Some(name) => find_board(boards, name)?,
        None => effective_default(boards).context(
            "no default mood board. Pass a board slug or run `cerebrum design set-default <slug>`.",
        )?,
    };
    let mut resolved = base.board.clone();
    let mut mixed_from = BTreeMap::new();
    apply_facet(
        boards,
        &base.board.slug,
        mix.palette.as_deref(),
        "palette",
        &mut mixed_from,
        |donor| {
            resolved.palette.clone_from(&donor.palette);
        },
    )?;
    apply_facet(
        boards,
        &base.board.slug,
        mix.typography.as_deref(),
        "typography",
        &mut mixed_from,
        |donor| {
            resolved.typography = donor.typography.clone();
        },
    )?;
    apply_facet(
        boards,
        &base.board.slug,
        mix.spacing.as_deref(),
        "spacing",
        &mut mixed_from,
        |donor| {
            resolved.spacing.clone_from(&donor.spacing);
        },
    )?;
    apply_facet(
        boards,
        &base.board.slug,
        mix.radius.as_deref(),
        "radius",
        &mut mixed_from,
        |donor| {
            resolved.radius.clone_from(&donor.radius);
        },
    )?;
    apply_facet(
        boards,
        &base.board.slug,
        mix.elevation.as_deref(),
        "elevation",
        &mut mixed_from,
        |donor| {
            resolved.shadows.clone_from(&donor.shadows);
        },
    )?;
    apply_facet(
        boards,
        &base.board.slug,
        mix.motion.as_deref(),
        "motion",
        &mut mixed_from,
        |donor| {
            resolved.motion = donor.motion.clone();
        },
    )?;
    if !mixed_from.is_empty() {
        resolved.provenance = format!("mixed:{}", base.board.slug);
    }
    Ok(ResolvedDesign {
        board: resolved,
        side: base.side,
        mixed_from,
    })
}

fn apply_facet(
    boards: &[StoredBoard],
    base_slug: &str,
    requested: Option<&str>,
    facet: &str,
    mixed_from: &mut BTreeMap<String, FacetSource>,
    apply: impl FnOnce(&DesignBoard),
) -> Result<()> {
    let Some(name) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let donor = find_board(boards, name)?;
    if donor.board.slug != base_slug {
        mixed_from.insert(
            facet.to_string(),
            FacetSource {
                slug: donor.board.slug.clone(),
                brain: donor.side.as_str().to_string(),
            },
        );
    }
    apply(&donor.board);
    Ok(())
}

fn find_board<'a>(boards: &'a [StoredBoard], name: &str) -> Result<&'a StoredBoard> {
    if let Some(board) = boards.iter().find(|board| board.board.slug == name) {
        return Ok(board);
    }
    let by_name: Vec<_> = boards
        .iter()
        .filter(|board| board.board.name == name)
        .collect();
    match by_name.len() {
        1 => Ok(by_name[0]),
        0 => {
            let known = boards
                .iter()
                .map(|board| board.board.slug.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let known = if known.is_empty() {
                "none yet".to_string()
            } else {
                known
            };
            bail!("no mood board '{name}'. Known boards: {known}");
        }
        _ => bail!("more than one mood board is named '{name}'; pass the slug"),
    }
}

pub fn add_board(brain: &Path, input: &BoardInput, from: Option<DesignBoard>) -> Result<PathBuf> {
    if !brain.exists() {
        bail!("no Brain at {}. Run cerebrum init first.", brain.display());
    }
    let now = Utc::now().to_rfc3339();
    let imported = from.is_some();
    let mut base = from.unwrap_or_else(|| empty_board(&now));
    let replacing = input.replace;
    let slug_hint = input
        .slug
        .clone()
        .or_else(|| input.name.as_ref().and_then(|name| slugify(name).ok()))
        .unwrap_or_else(|| base.slug.clone());
    if !is_slug(&slug_hint) && input.slug.is_some() {
        bail!("invalid board slug '{slug_hint}'");
    }
    let existing = if is_slug(&slug_hint) {
        find_in_brain(brain, &slug_hint)?
    } else {
        None
    };
    if existing.is_some() && !replacing {
        bail!(
            "mood board '{slug_hint}' already exists. Pass --replace to overwrite it explicitly."
        );
    }
    if replacing && existing.is_none() && !imported {
        bail!(
            "mood board '{slug_hint}' does not exist in this brain. Drop --replace to create it."
        );
    }
    if replacing && !imported {
        if let Some(current) = &existing {
            base = current.clone();
        }
    }
    if let Some(name) = &input.name {
        base.name.clone_from(name);
    }
    if let Some(slug) = &input.slug {
        base.slug.clone_from(slug);
    } else if base.slug.is_empty() || (input.name.is_some() && !imported && !replacing) {
        base.slug = slugify(&base.name)?;
    }
    let existing = find_in_brain(brain, &base.slug)?;
    if existing.is_some() && !replacing {
        bail!(
            "mood board '{}' already exists. Pass --replace to overwrite it explicitly.",
            base.slug
        );
    }
    if replacing && existing.is_none() && !imported {
        bail!(
            "mood board '{}' does not exist in this brain. Drop --replace to create it.",
            base.slug
        );
    }
    if let Some(description) = &input.description {
        base.description.clone_from(description);
    }
    overlay_map(&mut base.palette, &input.palette);
    overlay_map(&mut base.typography.families, &input.families);
    overlay_map(&mut base.typography.scale, &input.scale);
    for (key, value) in &input.weights {
        let weight: u16 = value
            .parse()
            .with_context(|| format!("weight '{key}' must be an integer from 1 to 1000"))?;
        base.typography.weights.insert(key.clone(), weight);
    }
    overlay_map(&mut base.spacing, &input.spacing);
    overlay_map(&mut base.radius, &input.radius);
    overlay_map(&mut base.shadows, &input.shadows);
    if let Some(feel) = &input.feel {
        base.motion.feel.clone_from(feel);
    }
    overlay_map(&mut base.motion.duration, &input.durations);
    overlay_map(&mut base.motion.easing, &input.easings);
    overlay_map(&mut base.components, &input.components);
    if let Some(vibe) = &input.vibe {
        base.vibe.clone_from(vibe);
    }
    if let Some(guidance) = &input.guidance {
        base.body.clone_from(guidance);
    }
    base.references.extend(input.references.clone());
    normalize_board_tokens(&mut base)?;
    if let Some(current) = &existing {
        if current.slug == base.slug {
            base.created_at.clone_from(&current.created_at);
            if !input.make_default {
                base.is_default = current.is_default;
            }
        }
    }
    if input.make_default {
        base.is_default = true;
    }
    if base.created_at.is_empty() {
        base.created_at.clone_from(&now);
    }
    base.updated_at.clone_from(&now);
    base.schema = DESIGN_SCHEMA.to_string();
    if !imported {
        base.provenance = "manual".to_string();
        base.source_tool = "cli".to_string();
    } else if base.provenance.is_empty() {
        base.provenance = "import".to_string();
    }
    validate_board(&base)?;
    let previous_slug = existing.as_ref().map(|board| board.slug.clone());
    let path = write_board_file(brain, &base)?;
    if let Some(old) = previous_slug {
        if old != base.slug {
            let old_path = design_dir(brain).join(format!("{old}.md"));
            if old_path.exists() {
                fs::remove_file(old_path)?;
            }
        }
    }
    if base.is_default {
        clear_other_defaults(brain, &base.slug, &now)?;
    }
    Ok(path)
}

fn empty_board(now: &str) -> DesignBoard {
    DesignBoard {
        schema: DESIGN_SCHEMA.to_string(),
        name: String::new(),
        slug: String::new(),
        description: String::new(),
        is_default: false,
        palette: BTreeMap::new(),
        typography: Typography::default(),
        spacing: BTreeMap::new(),
        radius: BTreeMap::new(),
        shadows: BTreeMap::new(),
        motion: Motion::default(),
        components: BTreeMap::new(),
        vibe: String::new(),
        references: vec![],
        provenance: "manual".to_string(),
        created_at: now.to_string(),
        updated_at: now.to_string(),
        source_tool: "cli".to_string(),
        extensions: BTreeMap::new(),
        body: String::new(),
    }
}

fn normalize_board_tokens(board: &mut DesignBoard) -> Result<()> {
    let mut palette = BTreeMap::new();
    for (role, hex) in &board.palette {
        let role = require_role(role)?;
        let hex = normalize_hex(hex).with_context(|| format!("palette.{role}"))?;
        if palette.insert(role.clone(), hex).is_some() {
            bail!("palette role '{role}' is repeated");
        }
    }
    board.palette = palette;
    for key in board.typography.families.keys() {
        require_token_name(key, "font family")?;
    }
    for key in board
        .typography
        .scale
        .keys()
        .chain(board.spacing.keys())
        .chain(board.radius.keys())
        .chain(board.shadows.keys())
        .chain(board.motion.duration.keys())
        .chain(board.motion.easing.keys())
        .chain(board.components.keys())
        .chain(board.typography.weights.keys())
    {
        require_token_name(key, "token")?;
    }
    Ok(())
}

fn find_in_brain(brain: &Path, slug: &str) -> Result<Option<DesignBoard>> {
    if !is_slug(slug) {
        return Ok(None);
    }
    let path = design_dir(brain).join(format!("{slug}.md"));
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    Ok(Some(parse_board(&text)?))
}

fn write_board_file(brain: &Path, board: &DesignBoard) -> Result<PathBuf> {
    if !is_slug(&board.slug) {
        bail!("refusing to write invalid slug '{}'", board.slug);
    }
    let dir = design_dir(brain);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", board.slug));
    if path.parent() != Some(dir.as_path()) {
        bail!("refusing to write a design board outside memory/design");
    }
    fs::write(&path, render_board(board)?)?;
    Ok(path)
}

fn clear_other_defaults(brain: &Path, keep: &str, now: &str) -> Result<()> {
    for stored in load_side(brain, BrainSide::Local)? {
        if stored.board.slug != keep && stored.board.is_default {
            let mut board = stored.board;
            board.is_default = false;
            board.updated_at = now.to_string();
            write_board_file(brain, &board)?;
        }
    }
    Ok(())
}

pub fn set_default_board(brain: &Path, slug: &str) -> Result<()> {
    if !brain.exists() {
        bail!("no Brain at {}. Run cerebrum init first.", brain.display());
    }
    let Some(mut board) = find_in_brain(brain, slug)? else {
        bail!("no mood board '{slug}' in this brain");
    };
    let now = Utc::now().to_rfc3339();
    if !board.is_default {
        board.is_default = true;
        board.updated_at.clone_from(&now);
        write_board_file(brain, &board)?;
    }
    clear_other_defaults(brain, &board.slug, &now)?;
    Ok(())
}

pub fn remove_board(brain: &Path, slug: &str) -> Result<PathBuf> {
    if !is_slug(slug) {
        bail!("invalid board slug '{slug}'");
    }
    let path = design_dir(brain).join(format!("{slug}.md"));
    if !path.is_file() {
        bail!("no mood board '{slug}' in this brain");
    }
    fs::remove_file(&path)?;
    Ok(path)
}

#[derive(Serialize)]
struct DesignView {
    schema: String,
    slug: String,
    name: String,
    description: String,
    brain: String,
    default: bool,
    palette: BTreeMap<String, String>,
    typography: TypographyView,
    spacing: BTreeMap<String, String>,
    radius: BTreeMap<String, String>,
    shadows: BTreeMap<String, String>,
    motion: MotionView,
    components: BTreeMap<String, String>,
    vibe: String,
    references: Vec<ReferenceView>,
    guidance: String,
    provenance: String,
    created_at: String,
    updated_at: String,
    source_tool: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    extensions: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    mixed_from: BTreeMap<String, FacetSource>,
    agent_instructions: String,
}

#[derive(Serialize)]
struct TypographyView {
    families: BTreeMap<String, String>,
    scale: BTreeMap<String, String>,
    weights: BTreeMap<String, u16>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    extensions: BTreeMap<String, Value>,
}

#[derive(Serialize)]
struct MotionView {
    feel: String,
    duration: BTreeMap<String, String>,
    easing: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    extensions: BTreeMap<String, Value>,
}

#[derive(Serialize)]
struct ReferenceView {
    kind: String,
    value: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
}

#[derive(Serialize)]
struct IndexView {
    schema: &'static str,
    effective_default: Option<String>,
    boards: Vec<IndexEntry>,
}

#[derive(Serialize)]
struct IndexEntry {
    slug: String,
    name: String,
    description: String,
    brain: String,
    default: bool,
    updated_at: String,
    path: String,
}

pub fn design_json(resolved: &ResolvedDesign) -> Result<String> {
    let board = &resolved.board;
    let view = DesignView {
        schema: DESIGN_SCHEMA.to_string(),
        slug: board.slug.clone(),
        name: board.name.clone(),
        description: board.description.clone(),
        brain: resolved.side.as_str().to_string(),
        default: board.is_default && resolved.mixed_from.is_empty(),
        palette: board.palette.clone(),
        typography: TypographyView {
            families: board.typography.families.clone(),
            scale: board.typography.scale.clone(),
            weights: board.typography.weights.clone(),
            extensions: board.typography.extensions.clone(),
        },
        spacing: board.spacing.clone(),
        radius: board.radius.clone(),
        shadows: board.shadows.clone(),
        motion: MotionView {
            feel: board.motion.feel.clone(),
            duration: board.motion.duration.clone(),
            easing: board.motion.easing.clone(),
            extensions: board.motion.extensions.clone(),
        },
        components: board.components.clone(),
        vibe: board.vibe.clone(),
        references: board
            .references
            .iter()
            .map(|reference| ReferenceView {
                kind: reference.kind.clone(),
                value: reference.value.clone(),
                note: reference.note.clone(),
            })
            .collect(),
        guidance: board.body.trim().to_string(),
        provenance: board.provenance.clone(),
        created_at: board.created_at.clone(),
        updated_at: board.updated_at.clone(),
        source_tool: board.source_tool.clone(),
        extensions: board.extensions.clone(),
        mixed_from: resolved.mixed_from.clone(),
        agent_instructions: AGENT_INSTRUCTIONS.to_string(),
    };
    Ok(serde_json::to_string_pretty(&view)?)
}

pub fn index_json(boards: &[StoredBoard]) -> Result<String> {
    let effective = effective_default(boards).map(|board| board.board.slug.clone());
    let view = IndexView {
        schema: INDEX_SCHEMA,
        effective_default: effective,
        boards: boards
            .iter()
            .map(|stored| IndexEntry {
                slug: stored.board.slug.clone(),
                name: stored.board.name.clone(),
                description: stored.board.description.clone(),
                brain: stored.side.as_str().to_string(),
                default: stored.board.is_default,
                updated_at: stored.board.updated_at.clone(),
                path: stored.relative_path.clone(),
            })
            .collect(),
    };
    Ok(serde_json::to_string_pretty(&view)?)
}

pub fn export_design(resolved: &ResolvedDesign, format: DesignExportFormat) -> Result<String> {
    let text = match format {
        DesignExportFormat::Json => design_json(resolved)?,
        DesignExportFormat::Css => export_css(resolved),
        DesignExportFormat::Tailwind => export_tailwind(resolved),
        DesignExportFormat::Swiftui => export_swiftui(resolved),
    };
    Ok(text)
}

pub fn brief_section(local: Option<&Path>, global: Option<&Path>) -> String {
    let boards = match load_merged(local, global) {
        Ok(boards) if !boards.is_empty() => boards,
        _ => return String::new(),
    };
    let Some(chosen) = effective_default(&boards) else {
        return format!(
            "## Design taste\n{} mood board(s) are saved, and none is the default. Run `cerebrum design set-default <slug>`.\n",
            boards.len()
        );
    };
    let palette = chosen
        .board
        .palette
        .iter()
        .map(|(role, hex)| format!("{role} {hex}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "## Design taste\nDefault board: {} (`{}`, {}).\nVibe: {}\nPalette: {}\nUse `get_design` or `cerebrum design show {}` before inventing colors or type.\n",
        chosen.board.name,
        chosen.board.slug,
        chosen.side.as_str(),
        if chosen.board.vibe.is_empty() {
            chosen.board.description.as_str()
        } else {
            chosen.board.vibe.as_str()
        },
        palette,
        chosen.board.slug
    )
}

pub fn store_design_proposals(brain: &Path, notes: &[String], source_name: &str) -> Result<usize> {
    let dir = brain.join("dream/design-proposals");
    fs::create_dir_all(&dir)?;
    let stamp = ts_stem(&Utc::now().to_rfc3339());
    let source = sanitize_filename(source_name);
    let mut count = 0usize;
    for (index, note) in notes.iter().enumerate() {
        let parsed = parse_taste_note(note);
        if parsed.note.is_empty() || is_empty_taste(&parsed.note) {
            continue;
        }
        let target_file = if parsed.target.is_empty() {
            "note".to_string()
        } else {
            parsed.target.clone()
        };
        let filename = format!("{stamp}-{index}-{target_file}.md");
        let proposal = DesignProposal {
            schema: PROPOSAL_SCHEMA.to_string(),
            status: "proposed".to_string(),
            action: parsed.action,
            target: parsed.target,
            note: parsed.note,
            provenance: format!("unreviewed:dream-ingest:{source}"),
            created_at: Utc::now().to_rfc3339(),
            reviewed_by: String::new(),
            applied_at: String::new(),
            body: String::new(),
        };
        let path = dir.join(&filename);
        fs::write(&path, render_proposal(&proposal)?)?;
        count += 1;
    }
    Ok(count)
}

pub fn apply_proposal(
    brain: &Path,
    file: &Path,
    approve: bool,
    reviewed_by: &str,
) -> Result<PathBuf> {
    if !approve {
        bail!(
            "refusing to apply a design proposal without --approve. Proposals never change a mood board on their own."
        );
    }
    let reviewer = reviewed_by.trim();
    if !is_reviewer(reviewer) {
        bail!("--reviewed-by must be 1-80 letters, numbers, spaces, or ._-");
    }
    if !brain.exists() {
        bail!("no Brain at {}. Run cerebrum init first.", brain.display());
    }
    let text =
        fs::read_to_string(file).with_context(|| format!("could not read {}", file.display()))?;
    let mut proposal = parse_proposal(&text)?;
    if proposal.status != "proposed" {
        bail!(
            "design proposal status is '{}'; only 'proposed' proposals can be approved",
            proposal.status
        );
    }
    if proposal.action == "note" || proposal.target.is_empty() {
        bail!(
            "this proposal has no target board. Edit it, or create the board with `cerebrum design add`."
        );
    }
    let fenced = fenced_board(&proposal.body).context(
        "proposal has no ```board token document, so nothing was written. Add one, or run `cerebrum design add`.",
    )?;
    let mut board = parse_board(fenced)?;
    if board.slug != proposal.target {
        bail!(
            "fenced board slug '{}' does not match proposal target '{}'",
            board.slug,
            proposal.target
        );
    }
    let existing = find_in_brain(brain, &board.slug)?;
    match proposal.action.as_str() {
        "update" => {
            let Some(current) = existing else {
                bail!(
                    "cannot update '{}' because that board is not in this brain",
                    board.slug
                );
            };
            board.created_at = current.created_at;
        }
        "create" => {
            if existing.is_some() {
                bail!(
                    "cannot create '{}' because that board already exists. Use action: update to replace it.",
                    board.slug
                );
            }
        }
        other => bail!("proposal action '{other}' cannot be applied"),
    }
    let now = Utc::now().to_rfc3339();
    let source_name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("proposal.md");
    board.updated_at = now.clone();
    board.provenance = format!("reviewed:{reviewer}:design-proposal:{source_name}");
    board.source_tool = "cli".to_string();
    board.schema = DESIGN_SCHEMA.to_string();
    validate_board(&board)?;
    let path = write_board_file(brain, &board)?;
    if board.is_default {
        clear_other_defaults(brain, &board.slug, &now)?;
    }
    proposal.status = "applied".to_string();
    proposal.reviewed_by = reviewer.to_string();
    proposal.applied_at = now;
    fs::write(file, render_proposal(&proposal)?)?;
    Ok(path)
}

fn parse_proposal(text: &str) -> Result<DesignProposal> {
    let (value, body) = split_frontmatter(text)?;
    let mut map = value
        .as_mapping()
        .context("design proposal frontmatter must be a YAML mapping")?
        .clone();
    let schema = take_string(&mut map, "schema")?.unwrap_or_default();
    if schema != PROPOSAL_SCHEMA {
        bail!("unsupported design proposal schema '{schema}'");
    }
    let proposal = DesignProposal {
        schema,
        status: take_string(&mut map, "status")?.unwrap_or_else(|| "proposed".to_string()),
        action: take_string(&mut map, "action")?.unwrap_or_else(|| "note".to_string()),
        target: take_string(&mut map, "target")?.unwrap_or_default(),
        note: take_string(&mut map, "note")?.unwrap_or_default(),
        provenance: take_string(&mut map, "provenance")?.unwrap_or_default(),
        created_at: take_string(&mut map, "created_at")?.unwrap_or_default(),
        reviewed_by: take_string(&mut map, "reviewed_by")?.unwrap_or_default(),
        applied_at: take_string(&mut map, "applied_at")?.unwrap_or_default(),
        body,
    };
    if !proposal.target.is_empty() && !is_slug(&proposal.target) {
        bail!("proposal target '{}' is not a board slug", proposal.target);
    }
    Ok(proposal)
}

fn render_proposal(proposal: &DesignProposal) -> Result<String> {
    let mut out = String::from("---\n");
    emit_scalar_line(&mut out, 0, "schema", &yaml_string(&proposal.schema));
    emit_scalar_line(&mut out, 0, "status", &yaml_string(&proposal.status));
    emit_scalar_line(&mut out, 0, "action", &yaml_string(&proposal.action));
    emit_scalar_line(&mut out, 0, "target", &yaml_string(&proposal.target));
    emit_scalar_line(&mut out, 0, "note", &yaml_string(&proposal.note));
    emit_scalar_line(
        &mut out,
        0,
        "provenance",
        &yaml_string(&proposal.provenance),
    );
    emit_scalar_line(
        &mut out,
        0,
        "created_at",
        &yaml_string(&proposal.created_at),
    );
    emit_scalar_line(
        &mut out,
        0,
        "reviewed_by",
        &yaml_string(&proposal.reviewed_by),
    );
    if !proposal.applied_at.is_empty() {
        emit_scalar_line(
            &mut out,
            0,
            "applied_at",
            &yaml_string(&proposal.applied_at),
        );
    }
    out.push_str("---\n\n");
    if proposal.body.trim().is_empty() {
        out.push_str(&proposal.note);
        out.push_str("\n\nThis proposal does not change a mood board. Review it, add a fenced `board` document if tokens should change, then run `cerebrum design apply <this-file> --approve`.\n");
    } else {
        out.push_str(proposal.body.trim_end());
        out.push('\n');
    }
    Ok(out)
}

fn fenced_board(body: &str) -> Option<&str> {
    let marker = "```board";
    let start = body.find(marker)?;
    let rest = &body[start + marker.len()..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let end = rest.find("\n```")?;
    let fenced = rest[..end].trim();
    if fenced.is_empty() {
        None
    } else {
        Some(fenced)
    }
}

fn parse_taste_note(raw: &str) -> TasteNote {
    let clean = sanitize_line(raw);
    let parts: Vec<&str> = raw.split('|').map(str::trim).collect();
    if parts.len() >= 3
        && parts[0].to_ascii_lowercase().starts_with("board:")
        && parts[1].to_ascii_lowercase().starts_with("action:")
        && parts[2].to_ascii_lowercase().starts_with("note:")
    {
        let slug = parts[0]
            .split_once(':')
            .map(|(_, value)| value.trim())
            .unwrap_or("");
        let action = parts[1]
            .split_once(':')
            .map(|(_, value)| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        let mut note_parts = vec![parts[2]
            .split_once(':')
            .map(|(_, value)| value.trim())
            .unwrap_or("")];
        note_parts.extend(parts.iter().skip(3).copied());
        let note = sanitize_line(&note_parts.join(" | "));
        if (action == "create" || action == "update") && is_slug(slug) && !note.is_empty() {
            return TasteNote {
                action,
                target: slug.to_string(),
                note,
            };
        }
    }
    TasteNote {
        action: "note".to_string(),
        target: String::new(),
        note: clean,
    }
}

fn is_empty_taste(note: &str) -> bool {
    matches!(
        note.trim().to_ascii_lowercase().as_str(),
        "" | "none" | "(none)" | "n/a" | "na" | "nothing" | "nothing to report" | "-"
    )
}

fn sanitize_line(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if c.is_control() {
            continue;
        }
        out.push(c);
        if out.chars().count() >= 500 {
            break;
        }
    }
    out.trim().to_string()
}

fn sanitize_filename(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            out.push(c);
        }
        if out.len() >= 80 {
            break;
        }
    }
    if out.is_empty() {
        "reflection.md".to_string()
    } else {
        out
    }
}

fn is_reviewer(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    !chars.is_empty()
        && chars.len() <= 80
        && chars
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-'))
}

pub fn run_design(action: DesignAction) -> Result<()> {
    match action {
        DesignAction::Add {
            name,
            slug,
            description,
            palette,
            family,
            scale,
            weight,
            spacing,
            radius,
            shadow,
            motion,
            duration,
            ease,
            component,
            vibe,
            reference,
            guidance,
            from,
            global,
            default,
            replace,
        } => {
            let brain = require_brain(global)?;
            let mut input = BoardInput {
                name,
                slug,
                description,
                feel: motion,
                vibe,
                guidance,
                make_default: default,
                replace,
                ..BoardInput::default()
            };
            for raw in &palette {
                insert_pair(&mut input.palette, raw, "--color")?;
            }
            for raw in &family {
                insert_pair(&mut input.families, raw, "--family")?;
            }
            for raw in &scale {
                insert_pair(&mut input.scale, raw, "--scale")?;
            }
            for raw in &weight {
                insert_pair(&mut input.weights, raw, "--weight")?;
            }
            for raw in &spacing {
                insert_pair(&mut input.spacing, raw, "--space")?;
            }
            for raw in &radius {
                insert_pair(&mut input.radius, raw, "--radius")?;
            }
            for raw in &shadow {
                insert_pair(&mut input.shadows, raw, "--shadow")?;
            }
            for raw in &duration {
                insert_pair(&mut input.durations, raw, "--duration")?;
            }
            for raw in &ease {
                insert_pair(&mut input.easings, raw, "--ease")?;
            }
            for raw in &component {
                insert_pair(&mut input.components, raw, "--component")?;
            }
            for raw in &reference {
                input.references.push(parse_reference_flag(raw)?);
            }
            let from_board = match from {
                Some(path) => Some(parse_board(
                    &fs::read_to_string(&path)
                        .with_context(|| format!("could not read {}", path.display()))?,
                )?),
                None => None,
            };
            let path = add_board(&brain, &input, from_board)?;
            let written = parse_board(&fs::read_to_string(&path)?)?;
            println!("{} Saved mood board {}", "✓".green().bold(), written.slug);
            println!("   {} {}", "→".dimmed(), path.display());
            for warning in core_palette_warnings(&written) {
                println!("   {} {}", "!".yellow(), warning);
            }
            Ok(())
        }
        DesignAction::List { json } => {
            let (local, global) = readable_brains()?;
            let boards = load_merged(local.as_deref(), global.as_deref())?;
            warn_multiple_defaults(&boards);
            if json {
                println!("{}", index_json(&boards)?);
                return Ok(());
            }
            if boards.is_empty() {
                println!(
                    "No mood boards yet. Add one with {}.",
                    "cerebrum design add".yellow()
                );
                return Ok(());
            }
            let effective = effective_default(&boards).map(|board| board.board.slug.clone());
            println!("{}", "Design boards".bright_cyan().bold());
            for stored in &boards {
                let marker = if effective.as_deref() == Some(stored.board.slug.as_str())
                    && stored.board.is_default
                {
                    "default".green().to_string()
                } else if stored.board.is_default {
                    "default flag".yellow().to_string()
                } else {
                    "".to_string()
                };
                println!(
                    "   {:<22} {:<8} {:<14} {}",
                    stored.board.slug.bold(),
                    stored.side.as_str(),
                    marker,
                    stored.board.description.dimmed()
                );
            }
            Ok(())
        }
        DesignAction::Show { name, json, mix } => {
            let resolved = resolve_cli(name.as_deref(), &mix)?;
            if json {
                println!("{}", design_json(&resolved)?);
            } else {
                print_board(&resolved);
            }
            Ok(())
        }
        DesignAction::SetDefault { name, global } => {
            let brain = require_brain(global)?;
            set_default_board(&brain, &name)?;
            println!("{} Default mood board is {}", "✓".green().bold(), name);
            Ok(())
        }
        DesignAction::Remove { name, global } => {
            let brain = require_brain(global)?;
            let path = remove_board(&brain, &name)?;
            println!("{} Removed {}", "✓".green().bold(), path.display());
            Ok(())
        }
        DesignAction::Export {
            format,
            output,
            name,
            mix,
        } => {
            let resolved = resolve_cli(name.as_deref(), &mix)?;
            let text = export_design(&resolved, format)?;
            if let Some(path) = output {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent)?;
                    }
                }
                fs::write(
                    &path,
                    if text.ends_with('\n') {
                        text
                    } else {
                        format!("{text}\n")
                    },
                )?;
                println!("{} Wrote {}", "✓".green().bold(), path.display());
            } else {
                println!("{text}");
            }
            Ok(())
        }
        DesignAction::Proposals { json } => list_proposals(json),
        DesignAction::Apply {
            file,
            approve,
            reviewed_by,
            global,
        } => {
            let brain = require_brain(global)?;
            let reviewer = reviewed_by.unwrap_or_else(|| "cli".to_string());
            let path = apply_proposal(&brain, &file, approve, &reviewer)?;
            println!(
                "{} Applied design proposal to {}",
                "✓".green().bold(),
                path.display()
            );
            Ok(())
        }
    }
}

fn resolve_cli(name: Option<&str>, mix: &DesignMix) -> Result<ResolvedDesign> {
    let (local, global) = readable_brains()?;
    let boards = load_merged(local.as_deref(), global.as_deref())?;
    resolve(&boards, name, &MixSpec::from_cli(mix))
}

fn list_proposals(json: bool) -> Result<()> {
    let (local, global) = readable_brains()?;
    let mut proposals = vec![];
    if let Some(path) = &local {
        proposals.extend(read_proposals(path, BrainSide::Local)?);
    }
    if let Some(path) = &global {
        proposals.extend(read_proposals(path, BrainSide::Global)?);
    }
    if json {
        let view: Vec<_> = proposals
            .iter()
            .map(|(side, path, proposal)| {
                serde_json::json!({
                    "brain": side.as_str(),
                    "file": path.display().to_string(),
                    "status": proposal.status,
                    "action": proposal.action,
                    "target": proposal.target,
                    "note": proposal.note,
                    "created_at": proposal.created_at,
                    "reviewed_by": proposal.reviewed_by,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&view)?);
        return Ok(());
    }
    if proposals.is_empty() {
        println!("No design proposals. Dream leaves them in dream/design-proposals/.");
        return Ok(());
    }
    println!("{}", "Design proposals".bright_cyan().bold());
    for (side, path, proposal) in &proposals {
        println!(
            "   {} {} {} → {} {}",
            proposal.status.yellow(),
            side.as_str(),
            proposal.action,
            if proposal.target.is_empty() {
                "(no target)"
            } else {
                proposal.target.as_str()
            },
            path.display().to_string().dimmed()
        );
        println!("      {}", proposal.note.dimmed());
    }
    println!(
        "\n   Apply one with {}",
        "cerebrum design apply <file> --approve".yellow()
    );
    Ok(())
}

fn read_proposals(
    brain: &Path,
    side: BrainSide,
) -> Result<Vec<(BrainSide, PathBuf, DesignProposal)>> {
    let dir = brain.join("dream/design-proposals");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut files: Vec<_> = fs::read_dir(&dir)?.filter_map(|entry| entry.ok()).collect();
    files.sort_by_key(|entry| entry.file_name());
    let mut out = vec![];
    for entry in files {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        out.push((side, path, parse_proposal(&text)?));
    }
    Ok(out)
}

fn warn_multiple_defaults(boards: &[StoredBoard]) {
    let flagged: Vec<_> = boards
        .iter()
        .filter(|board| board.board.is_default)
        .collect();
    if flagged.len() > 1 {
        let chosen = effective_default(boards)
            .map(|board| board.board.slug.as_str())
            .unwrap_or_default();
        eprintln!(
            "warning: {} boards are flagged default; using '{chosen}' (local wins, then latest updated_at)",
            flagged.len()
        );
    }
}

fn print_board(resolved: &ResolvedDesign) {
    let board = &resolved.board;
    println!(
        "{}  ({}, {})",
        board.name.bright_cyan().bold(),
        board.slug,
        resolved.side.as_str()
    );
    if !board.description.is_empty() {
        println!("{}", board.description);
    }
    if !resolved.mixed_from.is_empty() {
        println!("{}", "Mixed facets".bold());
        for (facet, source) in &resolved.mixed_from {
            println!("   {facet:<12} {} ({})", source.slug, source.brain);
        }
    }
    println!("{}", "Palette".bold());
    for (role, hex) in &board.palette {
        println!("   {role:<14} {hex}");
    }
    if !board.typography.families.is_empty()
        || !board.typography.scale.is_empty()
        || !board.typography.weights.is_empty()
    {
        println!("{}", "Typography".bold());
        for (role, family) in &board.typography.families {
            println!("   {role:<14} {family}");
        }
        for (step, size) in &board.typography.scale {
            println!("   {step:<14} {size}");
        }
        for (name, weight) in &board.typography.weights {
            println!("   {name:<14} {weight}");
        }
    }
    print_map("Spacing", &board.spacing);
    print_map("Radius", &board.radius);
    print_map("Shadows", &board.shadows);
    if !board.motion.feel.is_empty()
        || !board.motion.duration.is_empty()
        || !board.motion.easing.is_empty()
    {
        println!("{}", "Motion".bold());
        if !board.motion.feel.is_empty() {
            println!("   {:<14} {}", "feel", board.motion.feel);
        }
        for (name, value) in board
            .motion
            .duration
            .iter()
            .chain(board.motion.easing.iter())
        {
            println!("   {name:<14} {value}");
        }
    }
    print_map("Components", &board.components);
    if !board.vibe.is_empty() {
        println!("{}", "Vibe".bold());
        println!("   {}", board.vibe);
    }
    if !board.references.is_empty() {
        println!("{}", "References".bold());
        for reference in &board.references {
            println!("   {} {}", reference.kind, reference.value);
        }
    }
    if !board.body.trim().is_empty() {
        println!("{}", "Guidance".bold());
        println!("{}", board.body.trim());
    }
}

fn print_map(title: &str, values: &BTreeMap<String, String>) {
    if values.is_empty() {
        return;
    }
    println!("{}", title.bold());
    for (key, value) in values {
        println!("   {key:<14} {value}");
    }
}

fn readable_brains() -> Result<(Option<PathBuf>, Option<PathBuf>)> {
    let local = local_brain_path();
    let global = global_brain_path();
    let local = local.exists().then_some(local);
    let global = global.exists().then_some(global);
    if local.is_none() && global.is_none() {
        bail!("No Brain found. Run `cerebrum init` or `cerebrum init --global` first.");
    }
    Ok((local, global))
}

fn require_brain(global: bool) -> Result<PathBuf> {
    let path = resolve_brain(global);
    if !path.exists() {
        let hint = if global {
            "cerebrum init --global"
        } else {
            "cerebrum init"
        };
        bail!("No Brain found at {}. Run `{hint}` first.", path.display());
    }
    Ok(path)
}

fn insert_pair(map: &mut BTreeMap<String, String>, raw: &str, flag: &str) -> Result<()> {
    let (key, value) = raw
        .split_once('=')
        .with_context(|| format!("{flag} must be key=value"))?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        bail!("{flag} must be key=value");
    }
    map.insert(key.to_string(), value.to_string());
    Ok(())
}

fn parse_reference_flag(raw: &str) -> Result<DesignReference> {
    let (kind, value) = raw
        .split_once('=')
        .context("--reference must be path=<relative path> or url=<https://...>")?;
    Ok(DesignReference {
        kind: kind.trim().to_string(),
        value: value.trim().to_string(),
        note: String::new(),
    })
}

fn overlay_map(target: &mut BTreeMap<String, String>, incoming: &BTreeMap<String, String>) {
    for (key, value) in incoming {
        target.insert(key.clone(), value.clone());
    }
}

fn export_css(resolved: &ResolvedDesign) -> String {
    let board = &resolved.board;
    let mut out = String::new();
    push_banner(&mut out, "/*", " */", resolved);
    out.push_str(":root {\n");
    for (role, hex) in &board.palette {
        out.push_str(&format!("  --color-{role}: {hex};\n"));
    }
    for (role, family) in &board.typography.families {
        out.push_str(&format!("  --font-{role}: {};\n", css_font_stack(family)));
    }
    for (step, size) in &board.typography.scale {
        out.push_str(&format!("  --text-{step}: {size};\n"));
    }
    for (name, weight) in &board.typography.weights {
        out.push_str(&format!("  --font-weight-{name}: {weight};\n"));
    }
    for (step, size) in &board.spacing {
        out.push_str(&format!("  --space-{step}: {size};\n"));
    }
    for (name, size) in &board.radius {
        out.push_str(&format!("  --radius-{name}: {size};\n"));
    }
    for (name, shadow) in &board.shadows {
        out.push_str(&format!("  --shadow-{name}: {shadow};\n"));
    }
    if !board.motion.feel.is_empty() {
        out.push_str(&format!(
            "  --motion-feel: {};\n",
            css_string(&board.motion.feel)
        ));
    }
    for (name, value) in &board.motion.duration {
        out.push_str(&format!("  --duration-{name}: {value};\n"));
    }
    for (name, value) in &board.motion.easing {
        out.push_str(&format!("  --ease-{name}: {value};\n"));
    }
    out.push_str("}\n");
    out
}

fn export_tailwind(resolved: &ResolvedDesign) -> String {
    let board = &resolved.board;
    let mut out = String::new();
    push_banner(&mut out, "/*", " */", resolved);
    out.push_str("/** @type {import('tailwindcss').Config} */\n");
    out.push_str("module.exports = {\n  theme: {\n    extend: {\n");
    out.push_str(&format!(
        "      colors: {{\n{}      }},\n",
        js_entries(&board.palette, 8, js_string)
    ));
    out.push_str(&format!(
        "      fontFamily: {{\n{}      }},\n",
        js_entries(&board.typography.families, 8, js_font_family)
    ));
    out.push_str(&format!(
        "      fontSize: {{\n{}      }},\n",
        js_entries(&board.typography.scale, 8, js_string)
    ));
    let weights: BTreeMap<String, String> = board
        .typography
        .weights
        .iter()
        .map(|(key, value)| (key.clone(), value.to_string()))
        .collect();
    out.push_str(&format!(
        "      fontWeight: {{\n{}      }},\n",
        js_entries(&weights, 8, |value| value.to_string())
    ));
    out.push_str(&format!(
        "      spacing: {{\n{}      }},\n",
        js_entries(&board.spacing, 8, js_string)
    ));
    out.push_str(&format!(
        "      borderRadius: {{\n{}      }},\n",
        js_entries(&board.radius, 8, js_string)
    ));
    out.push_str(&format!(
        "      boxShadow: {{\n{}      }},\n",
        js_entries(&board.shadows, 8, js_string)
    ));
    out.push_str(&format!(
        "      transitionDuration: {{\n{}      }},\n",
        js_entries(&board.motion.duration, 8, js_string)
    ));
    out.push_str(&format!(
        "      transitionTimingFunction: {{\n{}      }},\n",
        js_entries(&board.motion.easing, 8, js_string)
    ));
    out.push_str("    },\n  },\n};\n");
    out
}

fn export_swiftui(resolved: &ResolvedDesign) -> String {
    let board = &resolved.board;
    let mut out = String::from("import SwiftUI\n\n");
    push_banner(&mut out, "//", "", resolved);
    out.push_str("// 1rem and 1em are emitted as 16pt.\n\n");
    out.push_str("enum MoodBoard {\n");
    out.push_str(&format!(
        "    static let slug = {}\n    static let name = {}\n",
        swift_string(&board.slug),
        swift_string(&board.name)
    ));
    if !board.vibe.is_empty() {
        out.push_str(&format!(
            "    static let vibe = {}\n",
            swift_string(&board.vibe)
        ));
    }
    if !board.motion.feel.is_empty() {
        out.push_str(&format!(
            "    static let motionFeel = {}\n",
            swift_string(&board.motion.feel)
        ));
    }
    out.push_str("\n    enum Palette {\n");
    for (role, hex) in &board.palette {
        out.push_str(&format!(
            "        static let {} = {}\n",
            swift_ident(role),
            swift_color(hex)
        ));
    }
    out.push_str("    }\n\n    enum FontFamily {\n");
    for (role, family) in &board.typography.families {
        out.push_str(&format!(
            "        static let {} = {}\n",
            swift_ident(role),
            swift_string(family)
        ));
    }
    out.push_str("    }\n\n    enum TextSize {\n");
    for (step, size) in &board.typography.scale {
        match length_to_points(size) {
            Some(points) => out.push_str(&format!(
                "        static let {}: CGFloat = {points}\n",
                swift_ident(step)
            )),
            None => out.push_str(&format!(
                "        // {step} left as {size} (not a px, rem, or em length)\n"
            )),
        }
    }
    out.push_str("    }\n\n    enum FontWeightToken {\n");
    for (name, weight) in &board.typography.weights {
        out.push_str(&format!(
            "        static let {} = Font.Weight{}\n",
            swift_ident(name),
            swift_font_weight(*weight)
        ));
    }
    out.push_str("    }\n\n    enum Space {\n");
    emit_points(&mut out, &board.spacing);
    out.push_str("    }\n\n    enum Radius {\n");
    emit_points(&mut out, &board.radius);
    out.push_str("    }\n\n    enum DurationMs {\n");
    for (name, value) in &board.motion.duration {
        if let Some(ms) = duration_to_ms(value) {
            out.push_str(&format!(
                "        static let {}: Double = {ms}\n",
                swift_ident(name)
            ));
        }
    }
    out.push_str("    }\n}\n");
    out
}

fn emit_points(out: &mut String, values: &BTreeMap<String, String>) {
    for (name, value) in values {
        match length_to_points(value) {
            Some(points) => out.push_str(&format!(
                "        static let {}: CGFloat = {points}\n",
                swift_ident(name)
            )),
            None => out.push_str(&format!(
                "        // {name} left as {value} (not a px, rem, or em length)\n"
            )),
        }
    }
}

fn push_banner(out: &mut String, open: &str, close: &str, resolved: &ResolvedDesign) {
    let board = &resolved.board;
    out.push_str(&format!(
        "{open} Generated by Cerebrumma from mood board \"{}\" ({}).{close}\n{open} Schema: {DESIGN_SCHEMA}. Brain: {}.{close}\n",
        board.name.replace('\n', " "),
        board.slug,
        resolved.side.as_str()
    ));
    if !resolved.mixed_from.is_empty() {
        let mix = resolved
            .mixed_from
            .iter()
            .map(|(facet, source)| format!("{facet}={}", source.slug))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("{open} Mixed: {mix}.{close}\n"));
    }
    out.push('\n');
}

fn js_entries(
    values: &BTreeMap<String, String>,
    indent: usize,
    render: impl Fn(&str) -> String,
) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    for (key, value) in values {
        out.push_str(&format!("{pad}{}: {},\n", js_string(key), render(value)));
    }
    out
}

fn js_font_family(family: &str) -> String {
    let parts: Vec<String> = family
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(js_string)
        .collect();
    format!("[{}]", parts.join(", "))
}

fn css_font_stack(family: &str) -> String {
    family
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(css_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn css_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn js_string(value: &str) -> String {
    css_string(value)
}

fn swift_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn swift_color(hex: &str) -> String {
    let digits = hex.trim_start_matches('#');
    let channel = |start: usize| {
        u32::from_str_radix(&digits[start..start + 2], 16).unwrap_or(0) as f64 / 255.0
    };
    let alpha = if digits.len() == 8 { channel(6) } else { 1.0 };
    format!(
        "Color(.sRGB, red: {:.4}, green: {:.4}, blue: {:.4}, opacity: {:.4})",
        channel(0),
        channel(2),
        channel(4),
        alpha
    )
}

fn swift_ident(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for (index, c) in name.chars().enumerate() {
        if c == '-' || c == '_' {
            upper = true;
            continue;
        }
        if !c.is_ascii_alphanumeric() {
            continue;
        }
        if out.is_empty() && c.is_ascii_digit() {
            out.push('_');
        }
        if index == 0 {
            out.push(c.to_ascii_lowercase());
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        out.push_str("token");
    }
    if matches!(
        out.as_str(),
        "default"
            | "class"
            | "enum"
            | "struct"
            | "protocol"
            | "import"
            | "self"
            | "Type"
            | "init"
            | "switch"
            | "case"
            | "return"
            | "var"
            | "let"
            | "func"
            | "in"
            | "where"
            | "true"
            | "false"
            | "nil"
            | "Self"
    ) {
        out.push_str("Token");
    }
    out
}

fn swift_font_weight(weight: u16) -> &'static str {
    match weight {
        0..=150 => ".ultraLight",
        151..=250 => ".thin",
        251..=350 => ".light",
        351..=450 => ".regular",
        451..=550 => ".medium",
        551..=650 => ".semibold",
        651..=750 => ".bold",
        751..=850 => ".heavy",
        _ => ".black",
    }
}

fn length_to_points(value: &str) -> Option<String> {
    if value == "0" {
        return Some("0".to_string());
    }
    let (number, factor) = if let Some(number) = value.strip_suffix("px") {
        (number, 1.0)
    } else if let Some(number) = value.strip_suffix("rem") {
        (number, 16.0)
    } else {
        let number = value.strip_suffix("em")?;
        (number, 16.0)
    };
    let parsed: f64 = number.parse().ok()?;
    Some(format_points(parsed * factor))
}

fn duration_to_ms(value: &str) -> Option<String> {
    let (number, factor) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1.0)
    } else {
        let number = value.strip_suffix('s')?;
        (number, 1000.0)
    };
    let parsed: f64 = number.parse().ok()?;
    Some(format_points(parsed * factor))
}

fn format_points(value: f64) -> String {
    if (value - value.round()).abs() < 0.001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.2}")
    }
}

fn split_frontmatter(text: &str) -> Result<(Value, String)> {
    let text = text.trim_start_matches('\u{feff}');
    if !text.starts_with("---") {
        bail!("design document must start with YAML frontmatter");
    }
    let rest = text.strip_prefix("---").unwrap_or(text);
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let delimiter = rest
        .find("\n---")
        .context("frontmatter opens with --- but has no closing ---")?;
    let yaml = &rest[..delimiter];
    let body = rest[delimiter + 4..].trim().to_string();
    let value: Value = serde_yaml::from_str(yaml).context("invalid YAML frontmatter")?;
    Ok((value, body))
}

fn yaml_key(name: &str) -> Value {
    Value::String(name.to_string())
}

fn take_value(map: &mut Mapping, key: &str) -> Option<Value> {
    map.remove(yaml_key(key))
}

fn take_string(map: &mut Mapping, key: &str) -> Result<Option<String>> {
    match take_value(map, key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(
            scalar_string(&value).with_context(|| key.to_string())?,
        )),
    }
}

fn take_bool(map: &mut Mapping, key: &str) -> Result<Option<bool>> {
    match take_value(map, key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(value)),
        Some(Value::String(value)) if value == "true" || value == "false" => {
            Ok(Some(value == "true"))
        }
        Some(_) => bail!("{key} must be true or false"),
    }
}

fn take_string_map(map: &mut Mapping, key: &str) -> Result<BTreeMap<String, String>> {
    match take_value(map, key) {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(Value::Mapping(nested)) => string_map(&nested).with_context(|| key.to_string()),
        Some(_) => bail!("{key} must be a mapping"),
    }
}

fn take_u16_map(map: &mut Mapping, key: &str) -> Result<BTreeMap<String, u16>> {
    match take_value(map, key) {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(Value::Mapping(nested)) => u16_map(&nested).with_context(|| key.to_string()),
        Some(_) => bail!("{key} must be a mapping"),
    }
}

fn take_typography(map: &mut Mapping) -> Result<Typography> {
    match take_value(map, "typography") {
        None | Some(Value::Null) => Ok(Typography::default()),
        Some(Value::Mapping(mut nested)) => {
            let families = take_string_map(&mut nested, "families")?;
            let scale = take_string_map(&mut nested, "scale")?;
            let weights = take_u16_map(&mut nested, "weights")?;
            Ok(Typography {
                families,
                scale,
                weights,
                extensions: take_extensions(nested)?,
            })
        }
        Some(_) => bail!("typography must be a mapping"),
    }
}

fn take_motion(map: &mut Mapping) -> Result<Motion> {
    match take_value(map, "motion") {
        None | Some(Value::Null) => Ok(Motion::default()),
        Some(Value::Mapping(mut nested)) => {
            let feel = take_string(&mut nested, "feel")?.unwrap_or_default();
            let duration = take_string_map(&mut nested, "duration")?;
            let easing = take_string_map(&mut nested, "easing")?;
            Ok(Motion {
                feel,
                duration,
                easing,
                extensions: take_extensions(nested)?,
            })
        }
        Some(_) => bail!("motion must be a mapping"),
    }
}

fn take_references(map: &mut Mapping) -> Result<Vec<DesignReference>> {
    match take_value(map, "references") {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Sequence(items)) => items.iter().map(reference_from_value).collect(),
        Some(_) => bail!("references must be a list"),
    }
}

fn reference_from_value(value: &Value) -> Result<DesignReference> {
    let map = value
        .as_mapping()
        .context("each reference must be a mapping with kind and value")?;
    let mut map = map.clone();
    let kind = take_string(&mut map, "kind")?.context("reference is missing kind")?;
    let value = take_string(&mut map, "value")?.context("reference is missing value")?;
    let note = take_string(&mut map, "note")?.unwrap_or_default();
    Ok(DesignReference { kind, value, note })
}

fn take_extensions(map: Mapping) -> Result<BTreeMap<String, Value>> {
    let mut extensions = BTreeMap::new();
    for (key, value) in map {
        let key = map_key(&key)?;
        if !extension_key_ok(&key) {
            bail!("cannot keep extension field '{key}'");
        }
        extensions.insert(key, value);
    }
    validate_extension_map(&extensions)?;
    Ok(extensions)
}

fn string_map(map: &Mapping) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (key, value) in map {
        let key = map_key(key)?;
        let text = scalar_string(value).with_context(|| format!("value for '{key}'"))?;
        out.insert(key, text);
    }
    Ok(out)
}

fn u16_map(map: &Mapping) -> Result<BTreeMap<String, u16>> {
    let mut out = BTreeMap::new();
    for (key, value) in map {
        let key = map_key(key)?;
        let number = match value {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.parse::<u64>().ok(),
            _ => None,
        };
        let Some(number) = number else {
            bail!("weight '{key}' must be an integer");
        };
        if !(1..=1000).contains(&number) {
            bail!("weight '{key}' must be from 1 to 1000");
        }
        out.insert(key, number as u16);
    }
    Ok(out)
}

fn scalar_string(value: &Value) -> Result<String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Ok(String::new()),
        _ => bail!("expected a scalar"),
    }
}

fn map_key(value: &Value) -> Result<String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => bail!("unsupported YAML key"),
    }
}

fn emit_scalar_line(out: &mut String, indent: usize, key: &str, rendered: &str) {
    out.push_str(&" ".repeat(indent));
    out.push_str(&emit_key(key));
    out.push_str(": ");
    out.push_str(rendered);
    out.push('\n');
}

fn emit_string_map(
    out: &mut String,
    indent: usize,
    key: &str,
    values: &BTreeMap<String, String>,
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    out.push_str(&format!("{}{}:\n", " ".repeat(indent), emit_key(key)));
    for (name, value) in values {
        emit_scalar_line(out, indent + 2, name, &yaml_string(value));
    }
    Ok(())
}

fn emit_typography(out: &mut String, typography: &Typography) -> Result<()> {
    if typography.families.is_empty()
        && typography.scale.is_empty()
        && typography.weights.is_empty()
        && typography.extensions.is_empty()
    {
        return Ok(());
    }
    out.push_str("typography:\n");
    emit_string_map(out, 2, "families", &typography.families)?;
    emit_string_map(out, 2, "scale", &typography.scale)?;
    if !typography.weights.is_empty() {
        out.push_str("  weights:\n");
        for (name, weight) in &typography.weights {
            emit_scalar_line(out, 4, name, &weight.to_string());
        }
    }
    emit_extensions(out, 2, &typography.extensions)?;
    Ok(())
}

fn emit_motion(out: &mut String, motion: &Motion) -> Result<()> {
    if motion.feel.is_empty()
        && motion.duration.is_empty()
        && motion.easing.is_empty()
        && motion.extensions.is_empty()
    {
        return Ok(());
    }
    out.push_str("motion:\n");
    if !motion.feel.is_empty() {
        emit_scalar_line(out, 2, "feel", &yaml_string(&motion.feel));
    }
    emit_string_map(out, 2, "duration", &motion.duration)?;
    emit_string_map(out, 2, "easing", &motion.easing)?;
    emit_extensions(out, 2, &motion.extensions)?;
    Ok(())
}

fn emit_references(out: &mut String, references: &[DesignReference]) -> Result<()> {
    if references.is_empty() {
        return Ok(());
    }
    out.push_str("references:\n");
    for reference in references {
        out.push_str(&format!("  - kind: {}\n", yaml_string(&reference.kind)));
        out.push_str(&format!("    value: {}\n", yaml_string(&reference.value)));
        if !reference.note.is_empty() {
            out.push_str(&format!("    note: {}\n", yaml_string(&reference.note)));
        }
    }
    Ok(())
}

fn emit_extensions(
    out: &mut String,
    indent: usize,
    extensions: &BTreeMap<String, Value>,
) -> Result<()> {
    for (key, value) in extensions {
        match value {
            Value::Mapping(_) | Value::Sequence(_) => {
                out.push_str(&format!("{}{}:\n", " ".repeat(indent), emit_key(key)));
                out.push_str(&emit_block(value, indent + 2)?);
            }
            other => emit_scalar_line(out, indent, key, &emit_scalar(other)?),
        }
    }
    Ok(())
}

fn emit_block(value: &Value, indent: usize) -> Result<String> {
    let pad = " ".repeat(indent);
    match value {
        Value::Mapping(map) => {
            let mut out = String::new();
            for (key, nested) in map {
                let key = map_key(key)?;
                match nested {
                    Value::Mapping(_) | Value::Sequence(_) => {
                        out.push_str(&format!("{pad}{}:\n", emit_key(&key)));
                        out.push_str(&emit_block(nested, indent + 2)?);
                    }
                    other => {
                        out.push_str(&format!(
                            "{pad}{}: {}\n",
                            emit_key(&key),
                            emit_scalar(other)?
                        ));
                    }
                }
            }
            Ok(out)
        }
        Value::Sequence(items) => {
            let mut out = String::new();
            for item in items {
                match item {
                    Value::Mapping(map) => {
                        let mut first = true;
                        for (key, nested) in map {
                            let key = map_key(key)?;
                            let prefix = if first {
                                format!("{pad}- ")
                            } else {
                                format!("{}  ", pad)
                            };
                            first = false;
                            match nested {
                                Value::Mapping(_) | Value::Sequence(_) => {
                                    out.push_str(&format!("{prefix}{}:\n", emit_key(&key)));
                                    out.push_str(&emit_block(nested, indent + 4)?);
                                }
                                other => out.push_str(&format!(
                                    "{prefix}{}: {}\n",
                                    emit_key(&key),
                                    emit_scalar(other)?
                                )),
                            }
                        }
                    }
                    other => out.push_str(&format!("{pad}- {}\n", emit_scalar(other)?)),
                }
            }
            Ok(out)
        }
        other => Ok(format!("{pad}{}\n", emit_scalar(other)?)),
    }
}

fn emit_scalar(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok("\"\"".to_string()),
        Value::Bool(true) => Ok("true".to_string()),
        Value::Bool(false) => Ok("false".to_string()),
        Value::Number(number) => Ok(number.to_string()),
        Value::String(value) => Ok(yaml_string(value)),
        _ => bail!("unsupported extension value"),
    }
}

fn emit_key(key: &str) -> String {
    if key
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        key.to_string()
    } else {
        yaml_string(key)
    }
}

fn yaml_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn require_text(value: &str, min: usize, max: usize, label: &str) -> Result<()> {
    let len = value.chars().count();
    if len < min || len > max {
        bail!("{label} must be {min}-{max} characters");
    }
    if value
        .chars()
        .any(|c| c != '\n' && c != '\t' && c.is_control())
    {
        bail!("{label} contains unsupported control characters");
    }
    Ok(())
}

fn require_timestamp(value: &str, label: &str) -> Result<()> {
    if DateTime::parse_from_rfc3339(value).is_err() {
        bail!("{label} must be RFC 3339");
    }
    Ok(())
}

fn require_role(role: &str) -> Result<String> {
    let role = role.trim().to_ascii_lowercase();
    if !role.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        || !role
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        || role.len() > 32
    {
        bail!("palette role '{role}' must be a short lowercase name");
    }
    Ok(role)
}

fn require_token_name(name: &str, label: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !ok {
        bail!("invalid {label} name '{name}'");
    }
    Ok(())
}

fn validate_families(families: &BTreeMap<String, String>) -> Result<()> {
    if families.len() > 32 {
        bail!("too many font families");
    }
    for (role, family) in families {
        require_token_name(role, "font family")?;
        if family.is_empty() || family.chars().count() > 80 {
            bail!("font family '{role}' must be 1-80 characters");
        }
        if family.chars().any(|c| {
            c.is_control() || matches!(c, '"' | '\\' | '`' | '{' | '}' | '<' | '>' | ';' | '$')
        }) {
            bail!("font family '{role}' contains unsupported characters");
        }
    }
    Ok(())
}

fn validate_length_map(values: &BTreeMap<String, String>, label: &str) -> Result<()> {
    if values.len() > 32 {
        bail!("{label} has too many steps");
    }
    for (name, value) in values {
        require_token_name(name, label)?;
        validate_length(value).with_context(|| format!("{label}.{name}"))?;
    }
    Ok(())
}

fn validate_length(value: &str) -> Result<()> {
    if value == "0" {
        return Ok(());
    }
    let number = value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("rem"))
        .or_else(|| value.strip_suffix("em"))
        .context("length must be 0 or a px, rem, or em value")?;
    validate_number(number)?;
    Ok(())
}

fn validate_number(raw: &str) -> Result<()> {
    let mut parts = raw.split('.');
    let whole = parts.next().unwrap_or("");
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        bail!("invalid number '{raw}'");
    }
    if let Some(fraction) = parts.next() {
        if fraction.is_empty()
            || !fraction.chars().all(|c| c.is_ascii_digit())
            || parts.next().is_some()
        {
            bail!("invalid number '{raw}'");
        }
    }
    Ok(())
}

fn validate_weights(weights: &BTreeMap<String, u16>) -> Result<()> {
    if weights.len() > 32 {
        bail!("too many font weights");
    }
    for (name, weight) in weights {
        require_token_name(name, "font weight")?;
        if !(1..=1000).contains(weight) {
            bail!("font weight '{name}' must be from 1 to 1000");
        }
    }
    Ok(())
}

fn validate_shadows(shadows: &BTreeMap<String, String>) -> Result<()> {
    if shadows.len() > 32 {
        bail!("too many shadows");
    }
    for (name, value) in shadows {
        require_token_name(name, "shadow")?;
        if value != "none" {
            if value.chars().count() > 200
                || value
                    .chars()
                    .any(|c| c.is_control() || matches!(c, ';' | '{' | '}' | '<' | '>' | '\\'))
            {
                bail!("shadow '{name}' contains unsupported characters");
            }
            let lower = value.to_ascii_lowercase();
            if lower.contains("url(") || lower.contains("expression") || lower.contains("@import") {
                bail!("shadow '{name}' contains an unsupported CSS function");
            }
        }
    }
    Ok(())
}

fn validate_feel(feel: &str) -> Result<()> {
    require_text(feel, 0, 80, "motion.feel")?;
    if feel
        .chars()
        .any(|c| c.is_control() || matches!(c, '"' | '\\' | ';' | '{' | '}' | '<' | '>'))
    {
        bail!("motion.feel contains unsupported characters");
    }
    Ok(())
}

fn validate_duration_map(values: &BTreeMap<String, String>) -> Result<()> {
    if values.len() > 32 {
        bail!("too many motion durations");
    }
    for (name, value) in values {
        require_token_name(name, "duration")?;
        let number = value
            .strip_suffix("ms")
            .or_else(|| value.strip_suffix('s'))
            .context(format!("duration '{name}' must use ms or s"))?;
        validate_number(number)?;
    }
    Ok(())
}

fn validate_easing_map(values: &BTreeMap<String, String>) -> Result<()> {
    if values.len() > 32 {
        bail!("too many easing curves");
    }
    for (name, value) in values {
        require_token_name(name, "easing")?;
        if value.is_empty()
            || value.len() > 80
            || !value.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '(' | ')' | ',' | '.' | '-' | ' ')
            })
        {
            bail!("easing '{name}' contains unsupported characters");
        }
    }
    Ok(())
}

fn validate_components(components: &BTreeMap<String, String>) -> Result<()> {
    if components.len() > 32 {
        bail!("too many component notes");
    }
    for (name, note) in components {
        require_token_name(name, "component")?;
        require_text(note, 1, 300, "component note")?;
        if note.chars().any(|c| c.is_control()) {
            bail!("component note '{name}' must be a single line");
        }
    }
    Ok(())
}

fn validate_reference(reference: &DesignReference) -> Result<()> {
    require_text(&reference.note, 0, 200, "reference note")?;
    if reference.note.chars().any(|c| c.is_control()) {
        bail!("reference note must be a single line");
    }
    match reference.kind.as_str() {
        "path" => {
            let path = Path::new(&reference.value);
            if reference.value.is_empty()
                || reference.value.len() > 200
                || path.is_absolute()
                || reference.value.contains('\\')
                || reference.value.contains('\0')
                || path
                    .components()
                    .any(|component| matches!(component, Component::ParentDir))
            {
                bail!(
                    "reference path '{}' must be relative and must not contain ..",
                    reference.value
                );
            }
        }
        "url" => {
            if !(reference.value.starts_with("https://") || reference.value.starts_with("http://"))
                || reference.value.len() > 500
                || reference
                    .value
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control())
            {
                bail!("reference URL must be an http(s) URL");
            }
        }
        other => bail!("reference kind '{other}' must be path or url"),
    }
    Ok(())
}

fn extension_key_ok(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && !key
            .chars()
            .any(|c| c.is_control() || matches!(c, ':' | '#' | '{' | '}' | '[' | ']' | ','))
}

fn validate_extension_map(extensions: &BTreeMap<String, Value>) -> Result<()> {
    if extensions.len() > 24 {
        bail!("too many extension fields");
    }
    for (key, value) in extensions {
        if !extension_key_ok(key) {
            bail!("cannot keep extension field '{key}'");
        }
        let rendered = serde_yaml::to_string(value).unwrap_or_default();
        if rendered.len() > 4_000 {
            bail!("extension '{key}' is too large");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "cerebrum-design-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(path.join("memory/design")).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_input(name: &str) -> BoardInput {
        let mut input = BoardInput {
            name: Some(name.to_string()),
            description: Some("Warm paper and one accent.".to_string()),
            vibe: Some("Quiet editorial".to_string()),
            feel: Some("calm".to_string()),
            make_default: true,
            ..BoardInput::default()
        };
        input.palette.insert("background".into(), "#F6F1E8".into());
        input.palette.insert("surface".into(), "#fffbf6".into());
        input.palette.insert("ink".into(), "#1C1917".into());
        input.palette.insert("accent".into(), "#C2410C".into());
        input.families.insert("display".into(), "Fraunces".into());
        input
            .families
            .insert("body".into(), "Source Serif 4".into());
        input.scale.insert("base".into(), "1rem".into());
        input.scale.insert("display".into(), "3.25rem".into());
        input.weights.insert("regular".into(), "400".into());
        input.weights.insert("bold".into(), "700".into());
        input.spacing.insert("4".into(), "1rem".into());
        input.spacing.insert("8".into(), "2rem".into());
        input.radius.insert("md".into(), "10px".into());
        input
            .shadows
            .insert("md".into(), "0 10px 28px rgb(28 25 23 / 0.08)".into());
        input.durations.insert("base".into(), "220ms".into());
        input
            .easings
            .insert("standard".into(), "cubic-bezier(0.2, 0, 0, 1)".into());
        input
            .components
            .insert("button".into(), "Solid accent fill, no gradient.".into());
        input.references.push(DesignReference {
            kind: "path".into(),
            value: "references/hero.png".into(),
            note: "Homepage".into(),
        });
        input
    }

    fn write_sample(brain: &Path, name: &str, default: bool) -> DesignBoard {
        let mut input = sample_input(name);
        input.make_default = default;
        let path = add_board(brain, &input, None).unwrap();
        parse_board(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn slugify_lowercases_and_rejects_empty() {
        assert_eq!(slugify("Warm Editorial").unwrap(), "warm-editorial");
        assert_eq!(slugify("  Night / Studio  ").unwrap(), "night-studio");
        assert!(slugify("...").is_err());
    }

    #[test]
    fn hex_normalizes_short_form_and_rejects_junk() {
        assert_eq!(normalize_hex("#c24").unwrap(), "#CC2244");
        assert_eq!(normalize_hex("#c2410cdd").unwrap(), "#C2410CDD");
        assert!(normalize_hex("red").is_err());
        assert!(normalize_hex("#gg0000").is_err());
    }

    #[test]
    fn round_trip_keeps_tokens_and_extension_fields() {
        let text = include_str!("../templates/design-board.md");
        let board = parse_board(text).unwrap();
        assert_eq!(board.slug, "warm-editorial");
        assert_eq!(board.palette.get("accent").unwrap(), "#C2410C");
        assert_eq!(
            board.typography.families.get("body").unwrap(),
            "Source Serif 4"
        );
        assert_eq!(board.typography.weights.get("bold"), Some(&700));
        assert_eq!(board.spacing.get("4").unwrap(), "1rem");
        assert_eq!(
            board.extensions.get("mockitup_id").unwrap().as_str(),
            Some("mb_warm")
        );
        let again = parse_board(&render_board(&board).unwrap()).unwrap();
        assert_eq!(again, board);
    }

    #[test]
    fn rejects_parent_paths_bad_urls_and_css_injection() {
        let dir = TempDir::new("invalid");
        let mut input = sample_input("Unsafe");
        input.references[0].value = "../../.ssh/id_rsa".into();
        assert!(add_board(dir.path(), &input, None).is_err());

        input.references[0].value = "references/hero.png".into();
        input.references.push(DesignReference {
            kind: "url".into(),
            value: "javascript:alert(1)".into(),
            note: String::new(),
        });
        assert!(add_board(dir.path(), &input, None).is_err());

        input.references.pop();
        input
            .families
            .insert("body".into(), "Fraunces\"; color: red".into());
        assert!(add_board(dir.path(), &input, None).is_err());
        assert!(!design_dir(dir.path()).join("unsafe.md").exists());
    }

    #[test]
    fn local_board_replaces_global_and_local_default_wins() {
        let local = TempDir::new("local");
        let global = TempDir::new("global");
        let mut global_board = write_sample(global.path(), "Warm Editorial", true);
        global_board
            .palette
            .insert("accent".into(), "#000000".into());
        global_board.updated_at = "2026-11-10T18:00:00Z".into();
        write_board_file(global.path(), &global_board).unwrap();
        write_sample(global.path(), "Night Studio", false);
        write_sample(local.path(), "Warm Editorial", false);
        let mut night = write_sample(local.path(), "Night Studio", true);
        night.updated_at = "2026-11-09T00:00:00Z".into();
        night.is_default = true;
        write_board_file(local.path(), &night).unwrap();

        let merged = load_merged(Some(local.path()), Some(global.path())).unwrap();
        assert_eq!(merged.len(), 2);
        let warm = merged
            .iter()
            .find(|board| board.board.slug == "warm-editorial")
            .unwrap();
        assert_eq!(warm.side, BrainSide::Local);
        assert_eq!(warm.board.palette.get("accent").unwrap(), "#C2410C");
        let chosen = effective_default(&merged).unwrap();
        assert_eq!(chosen.board.slug, "night-studio");
        assert_eq!(chosen.side, BrainSide::Local);
    }

    #[test]
    fn mix_takes_palette_from_one_board_and_type_from_another() {
        let local = TempDir::new("mix");
        write_sample(local.path(), "Warm Editorial", true);
        let mut night = sample_input("Night Studio");
        night.make_default = false;
        night.palette.insert("background".into(), "#111111".into());
        night.palette.insert("accent".into(), "#88CCFF".into());
        night.families.insert("display".into(), "Geist".into());
        add_board(local.path(), &night, None).unwrap();
        let boards = load_side(local.path(), BrainSide::Local).unwrap();
        let resolved = resolve(
            &boards,
            Some("warm-editorial"),
            &MixSpec {
                palette: Some("night-studio".into()),
                typography: Some("warm-editorial".into()),
                ..MixSpec::default()
            },
        )
        .unwrap();
        assert_eq!(resolved.board.palette.get("accent").unwrap(), "#88CCFF");
        assert_eq!(
            resolved.board.typography.families.get("display").unwrap(),
            "Fraunces"
        );
        assert_eq!(resolved.board.spacing.get("4").unwrap(), "1rem");
        assert_eq!(
            resolved.mixed_from.get("palette").unwrap().slug,
            "night-studio"
        );
        assert!(!resolved.mixed_from.contains_key("typography"));
        let css = export_design(&resolved, DesignExportFormat::Css).unwrap();
        assert!(css.contains("--color-accent: #88CCFF;"));
        assert!(css.contains("--font-display: \"Fraunces\";"));
        let tailwind = export_design(&resolved, DesignExportFormat::Tailwind).unwrap();
        assert!(tailwind.contains("\"accent\": \"#88CCFF\""));
        assert!(tailwind.contains("\"display\": [\"Fraunces\"]"));
        let swift = export_design(&resolved, DesignExportFormat::Swiftui).unwrap();
        assert!(swift.contains("static let accent = Color(.sRGB"));
        assert!(swift.contains("static let _4: CGFloat = 16"));
        let json = export_design(&resolved, DesignExportFormat::Json).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["schema"], DESIGN_SCHEMA);
        assert_eq!(value["mixed_from"]["palette"]["slug"], "night-studio");
        assert!(value["agent_instructions"]
            .as_str()
            .unwrap()
            .contains("authoritative"));
    }

    #[test]
    fn add_refuses_to_overwrite_until_replace_and_preserves_created_at() {
        let dir = TempDir::new("replace");
        let created = write_sample(dir.path(), "Warm Editorial", false);
        let mut again = sample_input("Warm Editorial");
        again.make_default = false;
        again.palette.insert("accent".into(), "#00AA00".into());
        assert!(add_board(dir.path(), &again, None).is_err());
        again.replace = true;
        add_board(dir.path(), &again, None).unwrap();
        let updated = find_in_brain(dir.path(), "warm-editorial")
            .unwrap()
            .unwrap();
        assert_eq!(updated.created_at, created.created_at);
        assert_eq!(updated.palette.get("accent").unwrap(), "#00AA00");
        assert_ne!(updated.updated_at, created.updated_at);
    }

    #[test]
    fn set_default_clears_the_previous_flag_in_that_brain_only() {
        let local = TempDir::new("default-local");
        let global = TempDir::new("default-global");
        write_sample(local.path(), "Warm Editorial", true);
        write_sample(local.path(), "Night Studio", false);
        write_sample(global.path(), "Warm Editorial", true);
        set_default_board(local.path(), "night-studio").unwrap();
        let local_boards = load_side(local.path(), BrainSide::Local).unwrap();
        let defaults: Vec<_> = local_boards
            .iter()
            .filter(|board| board.board.is_default)
            .map(|board| board.board.slug.as_str())
            .collect();
        assert_eq!(defaults, vec!["night-studio"]);
        let global_warm = find_in_brain(global.path(), "warm-editorial")
            .unwrap()
            .unwrap();
        assert!(global_warm.is_default);
    }

    #[test]
    fn dream_proposals_do_not_create_or_overwrite_boards() {
        let dir = TempDir::new("propose");
        write_sample(dir.path(), "Warm Editorial", true);
        let before = fs::read_to_string(design_dir(dir.path()).join("warm-editorial.md")).unwrap();
        let count = store_design_proposals(
            dir.path(),
            &[
                "board: warm-editorial | action: update | note: Episodes keep asking for warm paper.".into(),
                "board: night-studio | action: create | note: A dark studio palette showed up twice.".into(),
                "(none)".into(),
            ],
            "reflection.md",
        )
        .unwrap();
        assert_eq!(count, 2);
        let after = fs::read_to_string(design_dir(dir.path()).join("warm-editorial.md")).unwrap();
        assert_eq!(before, after);
        assert!(find_in_brain(dir.path(), "night-studio").unwrap().is_none());
        let proposals = read_proposals(dir.path(), BrainSide::Local).unwrap();
        assert_eq!(proposals.len(), 2);
        assert!(proposals
            .iter()
            .all(|(_, _, proposal)| proposal.status == "proposed"));
    }

    #[test]
    fn apply_requires_approval_and_a_fenced_board() {
        let dir = TempDir::new("apply");
        write_sample(dir.path(), "Warm Editorial", true);
        let note_only = dir.path().join("note.md");
        fs::write(
            &note_only,
            render_proposal(&DesignProposal {
                schema: PROPOSAL_SCHEMA.into(),
                status: "proposed".into(),
                action: "update".into(),
                target: "warm-editorial".into(),
                note: "Paper backgrounds.".into(),
                provenance: "unreviewed:dream-ingest:reflection.md".into(),
                created_at: "2026-11-10T16:00:00Z".into(),
                reviewed_by: String::new(),
                applied_at: String::new(),
                body: String::new(),
            })
            .unwrap(),
        )
        .unwrap();
        assert!(apply_proposal(dir.path(), &note_only, false, "cli").is_err());
        assert!(apply_proposal(dir.path(), &note_only, true, "Ada").is_err());
        assert_eq!(
            find_in_brain(dir.path(), "warm-editorial")
                .unwrap()
                .unwrap()
                .palette
                .get("accent")
                .unwrap(),
            "#C2410C"
        );

        let current = find_in_brain(dir.path(), "warm-editorial")
            .unwrap()
            .unwrap();
        let mut fenced = current.clone();
        fenced.palette.insert("accent".into(), "#115511".into());
        fenced.vibe = "Still editorial, greener accent.".into();
        let mut body = String::from("Reviewed by hand.\n\n```board\n");
        body.push_str(&render_board(&fenced).unwrap());
        body.push_str("```\n");
        let proposal_path = dir.path().join("ready.md");
        fs::write(
            &proposal_path,
            render_proposal(&DesignProposal {
                schema: PROPOSAL_SCHEMA.into(),
                status: "proposed".into(),
                action: "update".into(),
                target: "warm-editorial".into(),
                note: "Shift the accent.".into(),
                provenance: "unreviewed:dream-ingest:reflection.md".into(),
                created_at: "2026-11-10T16:00:00Z".into(),
                reviewed_by: String::new(),
                applied_at: String::new(),
                body,
            })
            .unwrap(),
        )
        .unwrap();
        apply_proposal(dir.path(), &proposal_path, true, "Ada").unwrap();
        let updated = find_in_brain(dir.path(), "warm-editorial")
            .unwrap()
            .unwrap();
        assert_eq!(updated.palette.get("accent").unwrap(), "#115511");
        assert_eq!(updated.created_at, current.created_at);
        assert!(updated.provenance.contains("reviewed:Ada"));
        let applied = parse_proposal(&fs::read_to_string(&proposal_path).unwrap()).unwrap();
        assert_eq!(applied.status, "applied");
        assert!(apply_proposal(dir.path(), &proposal_path, true, "Ada").is_err());
    }

    #[test]
    fn remove_deletes_only_the_named_brain_file() {
        let local = TempDir::new("remove-local");
        let global = TempDir::new("remove-global");
        write_sample(local.path(), "Warm Editorial", true);
        write_sample(global.path(), "Warm Editorial", true);
        remove_board(local.path(), "warm-editorial").unwrap();
        assert!(find_in_brain(local.path(), "warm-editorial")
            .unwrap()
            .is_none());
        assert!(find_in_brain(global.path(), "warm-editorial")
            .unwrap()
            .is_some());
    }

    #[test]
    fn omitted_board_uses_the_default() {
        let dir = TempDir::new("default-resolve");
        write_sample(dir.path(), "Warm Editorial", false);
        write_sample(dir.path(), "Night Studio", true);
        let boards = load_side(dir.path(), BrainSide::Local).unwrap();
        let resolved = resolve(&boards, None, &MixSpec::default()).unwrap();
        assert_eq!(resolved.board.slug, "night-studio");
    }
}
