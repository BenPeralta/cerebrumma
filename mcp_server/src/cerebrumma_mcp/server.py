import json
from datetime import datetime, timezone
from pathlib import Path

from mcp.server.fastmcp import FastMCP

mcp = FastMCP("cerebrumma")

BRAIN_DIR = Path(".cerebrum")


def ensure_brain() -> None:
    if not BRAIN_DIR.exists():
        raise RuntimeError("No .cerebrum/ found. Run `cerebrum init` first.")


@mcp.tool()
def read_memory(layer: str = "episodic", limit: int = 10) -> str:
    """Read the most recent entries from a memory layer.

    Args:
        layer: One of working | episodic | semantic | personal
        limit: Max number of entries to return
    """
    ensure_brain()
    layer_path = BRAIN_DIR / "memory" / layer
    if not layer_path.exists():
        return f"Layer '{layer}' not found."

    files = sorted(layer_path.glob("*.md"), reverse=True)[:limit]
    entries = []
    for f in files:
        text = f.read_text()
        entries.append({
            "file": f.name,
            "content": text[:500] + "…" if len(text) > 500 else text,
        })
    return json.dumps(entries, indent=2)


@mcp.tool()
def search_memory(query: str, limit: int = 5) -> str:
    """Keyword search across episodic and semantic memory.

    Args:
        query: Search string (case-insensitive)
        limit: Max number of results
    """
    ensure_brain()
    results = []
    for layer in ["episodic", "semantic"]:
        layer_path = BRAIN_DIR / "memory" / layer
        if not layer_path.exists():
            continue
        for f in sorted(layer_path.glob("*.md"), reverse=True):
            text = f.read_text()
            if query.lower() in text.lower():
                results.append({
                    "file": f.name,
                    "layer": layer,
                    "snippet": text[:300],
                })
            if len(results) >= limit:
                break
    return json.dumps(results, indent=2) if results else "No matches found."


@mcp.tool()
def get_protocols() -> str:
    """Return all active procedural protocols (fairness + coding rules)."""
    ensure_brain()
    protocols_dir = BRAIN_DIR / "memory" / "procedural" / "protocols"
    files = sorted(protocols_dir.glob("*.md")) if protocols_dir.exists() else []
    if not files:
        return "No protocols defined yet. Add .md files to .cerebrum/memory/procedural/protocols/."
    return "\n\n---\n\n".join(f.read_text() for f in files)


@mcp.tool()
def add_entry(content: str, layer: str = "episodic") -> str:
    """Add a new memory entry.

    Args:
        content: The text to remember
        layer: Target layer — episodic | semantic | personal
    """
    ensure_brain()
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H_%M_%SZ")
    entry = (
        "---\n"
        f"timestamp: {timestamp}\n"
        "source_tool: mcp\n"
        "salience_score: 0.6\n"
        "bias_flag: false\n"
        "provenance: mcp\n"
        "---\n\n"
        f"{content}\n"
    )
    path = BRAIN_DIR / "memory" / layer / f"{timestamp}.md"
    path.write_text(entry)
    return f"Saved to {layer}: {path.name}"


@mcp.tool()
def get_status() -> str:
    """Return a count of entries in each memory layer."""
    ensure_brain()
    layers = [
        ("working", "memory/working"),
        ("episodic", "memory/episodic"),
        ("semantic", "memory/semantic"),
        ("skills", "memory/procedural/skills"),
        ("protocols", "memory/procedural/protocols"),
        ("personal", "memory/personal"),
    ]
    counts = {}
    for name, rel in layers:
        p = BRAIN_DIR / rel
        counts[name] = len(list(p.glob("*.md"))) if p.exists() else 0
    return json.dumps(counts, indent=2)


def run() -> None:
    mcp.run()


if __name__ == "__main__":
    run()
