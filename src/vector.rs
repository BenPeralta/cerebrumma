use anyhow::{bail, Result};
use colored::*;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::Path;

use crate::memory::{active_brain, global_brain_path, local_brain_path};

pub fn init_vector_db(brain: &Path) -> Result<()> {
    let db_path = brain.join("embeddings.db");
    let conn = Connection::open(db_path)?;

    // Extension is now auto-loaded via sqlite3_auto_extension in main()

    conn.execute(
        "CREATE TABLE IF NOT EXISTS items (
            id INTEGER PRIMARY KEY,
            content TEXT NOT NULL,
            path TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS vec_items USING vec0(
            embedding float[384]
        )",
        [],
    )?;

    Ok(())
}

pub fn get_embedding(text: &str) -> Result<Vec<f32>> {
    // Keep the model cache GLOBAL (~/.cerebrum/models), never in the project cwd —
    // otherwise `.fastembed_cache/` leaks into the user's repo and their next commit.
    let cache_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".cerebrum")
        .join("models");
    let _ = fs::create_dir_all(&cache_dir);

    let model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::BGESmallENV15)
            .with_show_download_progress(true)
            .with_cache_dir(cache_dir),
    )?;

    let embeddings = model.embed(vec![text], None)?;
    Ok(embeddings[0].clone())
}

pub fn store_vector(brain: &Path, content: &str, path: &str) -> Result<()> {
    let db_path = brain.join("embeddings.db");
    let _ = init_vector_db(brain); // Ensure tables exist
    let conn = Connection::open(db_path)?;

    let embedding = get_embedding(content)?;

    conn.execute(
        "INSERT INTO items (content, path) VALUES (?, ?)",
        params![content, path],
    )?;

    let row_id = conn.last_insert_rowid();

    let embedding_json = serde_json::to_string(&embedding)?;
    conn.execute(
        "INSERT INTO vec_items (rowid, embedding) VALUES (?, ?)",
        params![row_id, embedding_json],
    )?;

    Ok(())
}

#[derive(Serialize)]
struct SearchHit {
    brain: String,
    path: String,
    content: String,
    distance: f32,
}

#[derive(Serialize)]
struct SearchResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    results: Vec<SearchHit>,
}

fn brain_label(brain: &Path) -> String {
    if brain == global_brain_path() {
        "global".to_string()
    } else {
        "local".to_string()
    }
}

fn query_index(brain: &Path, query_json: &str, k: usize) -> Result<Vec<SearchHit>> {
    let db_path = brain.join("embeddings.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let conn = Connection::open(db_path)?;
    // `k` is a sqlite-vec constraint and is interpolated only after clamping to 1..=20.
    let sql = format!(
        "SELECT items.content, items.path, distance
         FROM vec_items
         JOIN items ON items.rowid = vec_items.rowid
         WHERE embedding MATCH ?
         AND k = {k}
         ORDER BY distance"
    );
    let mut stmt = conn.prepare(&sql)?;
    let label = brain_label(brain);
    let rows = stmt.query_map(params![query_json], |row| {
        Ok(SearchHit {
            brain: label.clone(),
            content: row.get(0)?,
            path: row.get(1)?,
            distance: row.get(2)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn search_hits(query: &str, limit: usize) -> Result<SearchResponse> {
    let limit = limit.clamp(1, 20);
    let mut brains = Vec::new();
    let local = local_brain_path();
    let global = global_brain_path();
    if local.exists() {
        brains.push(local);
    }
    if global.exists() && !brains.iter().any(|b| b == &global) {
        brains.push(global);
    }
    if brains.is_empty() {
        return Ok(SearchResponse {
            ok: false,
            reason: Some("no_brain".to_string()),
            results: vec![],
        });
    }
    let indexed = brains
        .iter()
        .any(|brain| brain.join("embeddings.db").exists());
    if !indexed {
        return Ok(SearchResponse {
            ok: false,
            reason: Some("no_index".to_string()),
            results: vec![],
        });
    }

    let query_embedding = get_embedding(query)?;
    let query_json = serde_json::to_string(&query_embedding)?;
    let mut hits = Vec::new();
    for brain in &brains {
        hits.extend(query_index(brain, &query_json, limit)?);
    }
    hits.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit);
    Ok(SearchResponse {
        ok: true,
        reason: None,
        results: hits,
    })
}

pub fn run_search(query: &str, json: bool, limit: usize) -> Result<()> {
    let response = search_hits(query, limit)?;
    if json {
        println!("{}", serde_json::to_string(&response)?);
        return Ok(());
    }

    if !response.ok {
        bail!(
            "{}",
            match response.reason.as_deref() {
                Some("no_index") => "Vector database not found. Try adding some entries first.",
                _ => "No Brain found.",
            }
        );
    }

    println!(
        "{} Semantic search for '{}'...",
        "→".bright_magenta(),
        query.yellow()
    );
    println!();
    if response.results.is_empty() {
        println!("   {} No matches found.", "→".dimmed());
        return Ok(());
    }
    for (count, hit) in response.results.iter().enumerate() {
        println!(
            "  {} {} {} (distance: {:.4})",
            (count + 1).to_string().green(),
            hit.brain.dimmed(),
            hit.path.dimmed(),
            hit.distance
        );
        println!("    {}\n", hit.content.lines().next().unwrap_or("").trim());
    }
    Ok(())
}

/// Index content from stdin against a memory file that already lives in the brain.
pub fn run_embed(brain: &Path, file: &Path) -> Result<()> {
    if !brain.exists() {
        bail!("brain not found: {}", brain.display());
    }
    let brain = brain.canonicalize()?;
    if !file.exists() {
        bail!("memory file not found: {}", file.display());
    }
    let file = file.canonicalize()?;
    if !file.starts_with(&brain) {
        bail!("refusing to index a file outside the brain");
    }

    let mut content = String::new();
    std::io::stdin().read_to_string(&mut content)?;
    if content.len() > 512 * 1024 {
        bail!("content exceeds 512 KiB");
    }
    if content.trim().is_empty() {
        bail!("empty content");
    }
    store_vector(&brain, &content, &file.to_string_lossy())?;
    println!("indexed {}", file.display());
    Ok(())
}

pub fn run_why(query: &str) -> Result<()> {
    println!(
        "{} Tracing origin of '{}'...",
        "→".bright_magenta(),
        query.yellow()
    );

    let brain = active_brain().ok_or_else(|| anyhow::anyhow!("No Brain found."))?;

    let layers = [
        "memory/semantic",
        "memory/procedural/skills",
        "memory/procedural/protocols",
        "memory/personal",
    ];

    let mut matches = 0;

    for layer in &layers {
        let dir = brain.join(layer);
        if !dir.exists() {
            continue;
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };

            if content.to_lowercase().contains(&query.to_lowercase()) {
                matches += 1;
                // Parse YAML frontmatter if present
                println!("\n  {} Layer: {}", "●".bright_cyan(), layer.bold());
                println!("    File: {}", entry.file_name().to_string_lossy().dimmed());

                if content.starts_with("---") {
                    let parts: Vec<&str> = content.split("---").collect();
                    if parts.len() >= 3 {
                        let meta = parts[1];
                        println!("    {}", meta.trim().replace("\n", "\n    "));
                    }
                }
            }
        }
    }

    if matches == 0 {
        println!("   {} No matches found in long-term memory.", "→".dimmed());
        let needle = query.to_lowercase();
        let recent = fs::read_dir(brain.join("memory/episodic"))
            .map(|dir| {
                dir.filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        fs::read_to_string(entry.path())
                            .map(|text| text.to_lowercase().contains(&needle))
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        if recent > 0 {
            println!(
                "   {} {} recent episodic {} it, but `why` only traces rules and facts\n     that a dream cycle has promoted. Run {} to promote them.",
                "→".dimmed(),
                recent,
                if recent == 1 {
                    "entry mentions"
                } else {
                    "entries mention"
                },
                "cerebrum dream".yellow()
            );
        } else {
            println!(
                "   {} Try {} for a fuzzy match across every layer.",
                "→".dimmed(),
                format!("cerebrum search \"{query}\"").yellow()
            );
        }
    }

    Ok(())
}

// ── Neural Graph ────────────────────────────────────────────────────────────
// Materializes the latent connections already stored in embeddings.db: every
// entry is a node, and edges are drawn between entries whose vectors are
// semantically similar. No manual [[wikilinks]] — the Brain draws its own map.

#[derive(Serialize)]
pub struct GraphNode {
    pub id: usize,
    pub label: String,
    pub layer: String,
    pub color: String,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub source: usize,
    pub target: usize,
    pub weight: f32,
}

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Map a stored entry path to a human layer name + accent color.
fn classify_layer(path: &str) -> (&'static str, &'static str) {
    let p = path.to_lowercase();
    if p.contains("episodic") {
        ("Episodic", "#38bdf8") // sky
    } else if p.contains("semantic") {
        ("Semantic", "#c084fc") // accent purple
    } else if p.contains("procedural") || p.contains("skills") || p.contains("protocols") {
        ("Procedural", "#4ade80") // green
    } else if p.contains("personal") || p.contains("equity") {
        ("Personal", "#f472b6") // pink
    } else if p.contains("memory/design") || p.contains("/design/") {
        ("Design", "#f59e0b") // amber
    } else if p.contains("dream") || p.contains("audit") {
        ("Reflection", "#fbbf24") // amber
    } else {
        ("Other", "#94a3b8") // slate
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Read every stored vector, then connect each node to its most similar peers.
pub fn build_graph_data(brain: &Path) -> Result<GraphData> {
    let db_path = brain.join("embeddings.db");
    if !db_path.exists() {
        anyhow::bail!("Vector database not found. Try adding some entries first.");
    }

    let conn = Connection::open(db_path)?;

    // vec0 stores float[384] as a raw little-endian f32 blob; read it straight back.
    let mut stmt = conn.prepare(
        "SELECT items.content, items.path, vec_items.embedding
         FROM items JOIN vec_items ON items.rowid = vec_items.rowid",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut embeddings: Vec<Vec<f32>> = Vec::new();

    for row in rows {
        let (content, path, bytes) = row?;
        let emb: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        if emb.is_empty() {
            continue;
        }

        let (layer, color) = classify_layer(&path);
        let label: String = content
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .chars()
            .take(70)
            .collect();

        nodes.push(GraphNode {
            id: nodes.len(),
            label,
            layer: layer.to_string(),
            color: color.to_string(),
        });
        embeddings.push(emb);
    }

    // Edges: keep each node's top-3 neighbors above the similarity threshold,
    // deduped so we don't draw the same connection twice.
    const THRESHOLD: f32 = 0.55;
    const TOP_K: usize = 3;
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();

    for i in 0..embeddings.len() {
        let mut sims: Vec<(usize, f32)> = Vec::new();
        for j in 0..embeddings.len() {
            if i == j {
                continue;
            }
            let s = cosine(&embeddings[i], &embeddings[j]);
            if s > THRESHOLD {
                sims.push((j, s));
            }
        }
        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (j, s) in sims.into_iter().take(TOP_K) {
            let key = if i < j { (i, j) } else { (j, i) };
            if seen.insert(key) {
                edges.push(GraphEdge {
                    source: key.0,
                    target: key.1,
                    weight: s,
                });
            }
        }
    }

    Ok(GraphData { nodes, edges })
}
