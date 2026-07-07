use anyhow::Result;
use colored::*;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::memory::active_brain;

pub fn init_vector_db(brain: &PathBuf) -> Result<()> {
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
    let model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true),
    )?;

    let embeddings = model.embed(vec![text], None)?;
    Ok(embeddings[0].clone())
}

pub fn store_vector(brain: &PathBuf, content: &str, path: &str) -> Result<()> {
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

pub fn run_search(query: &str) -> Result<()> {
    println!(
        "{} Semantic search for '{}'...",
        "→".bright_magenta(),
        query.yellow()
    );

    let brain = active_brain().ok_or_else(|| anyhow::anyhow!("No Brain found."))?;

    let db_path = brain.join("embeddings.db");
    if !db_path.exists() {
        anyhow::bail!("Vector database not found. Try adding some entries first.");
    }

    let conn = Connection::open(db_path)?;

    let query_embedding = get_embedding(query)?;
    let query_json = serde_json::to_string(&query_embedding)?;

    let mut stmt = conn.prepare(
        "SELECT items.content, items.path, distance 
         FROM vec_items 
         JOIN items ON items.rowid = vec_items.rowid
         WHERE embedding MATCH ? 
         AND k = 5
         ORDER BY distance",
    )?;

    let rows = stmt.query_map(params![query_json], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f32>(2)?,
        ))
    })?;

    println!();
    let mut count = 0;
    for row in rows {
        let (content, path, distance) = row?;
        count += 1;
        println!(
            "  {} {} (distance: {:.4})",
            count.to_string().green(),
            path.dimmed(),
            distance
        );
        println!("    {}\n", content.lines().next().unwrap_or("").trim());
    }

    if count == 0 {
        println!("   {} No matches found.", "→".dimmed());
    }

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
            let content = fs::read_to_string(entry.path())?;

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
pub fn build_graph_data(brain: &PathBuf) -> Result<GraphData> {
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
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
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
