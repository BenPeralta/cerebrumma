import json
from datetime import datetime, timezone
from pathlib import Path

from mcp.server.fastmcp import FastMCP

mcp = FastMCP("cerebrumma")

_LOCAL = Path(".cerebrum")
_GLOBAL = Path.home() / ".cerebrum"


def brain_paths() -> list[Path]:
    """Return active Brain roots, local first (highest priority), then global."""
    return [p for p in [_LOCAL, _GLOBAL] if p.exists()]


def ensure_brain() -> None:
    if not brain_paths():
        raise RuntimeError(
            "No Brain found. Run `cerebrum init` (local) or `cerebrum init --global` first."
        )


def _read_layer(brain: Path, layer: str, limit: int) -> list[dict]:
    layer_path = brain / "memory" / layer
    if not layer_path.exists():
        return []
    entries = []
    for f in sorted(layer_path.glob("*.md"), reverse=True)[:limit]:
        text = f.read_text()
        entries.append({
            "file": f.name,
            "brain": "local" if brain == _LOCAL else "global",
            "content": text[:500] + "…" if len(text) > 500 else text,
        })
    return entries


@mcp.tool()
def read_memory(layer: str = "episodic", limit: int = 10) -> str:
    """Read the most recent entries from a memory layer (merges local + global).

    Args:
        layer: One of working | episodic | semantic | personal
        limit: Max number of entries to return
    """
    ensure_brain()
    results = []
    for brain in brain_paths():
        results.extend(_read_layer(brain, layer, limit))
    if not results:
        return f"Layer '{layer}' is empty."
    # Deduplicate by filename, local wins
    seen: dict[str, dict] = {}
    for entry in results:
        if entry["file"] not in seen:
            seen[entry["file"]] = entry
    return json.dumps(list(seen.values())[:limit], indent=2)


@mcp.tool()
def search_memory(query: str, limit: int = 5) -> str:
    """Keyword search across episodic and semantic memory (local + global).

    Args:
        query: Search string (case-insensitive)
        limit: Max number of results
    """
    ensure_brain()
    results = []
    for brain in brain_paths():
        for layer in ["episodic", "semantic"]:
            layer_path = brain / "memory" / layer
            if not layer_path.exists():
                continue
            for f in sorted(layer_path.glob("*.md"), reverse=True):
                text = f.read_text()
                if query.lower() in text.lower():
                    results.append({
                        "file": f.name,
                        "brain": "local" if brain == _LOCAL else "global",
                        "layer": layer,
                        "snippet": text[:300],
                    })
                if len(results) >= limit:
                    break
    return json.dumps(results, indent=2) if results else "No matches found."


@mcp.tool()
def get_protocols() -> str:
    """Return all active procedural protocols from local + global Brains."""
    ensure_brain()
    all_protocols: list[str] = []
    seen: set[str] = set()
    for brain in brain_paths():
        protocols_dir = brain / "memory" / "procedural" / "protocols"
        if not protocols_dir.exists():
            continue
        for f in sorted(protocols_dir.glob("*.md")):
            if f.name not in seen:
                seen.add(f.name)
                all_protocols.append(f.read_text())
    if not all_protocols:
        return "No protocols defined yet."
    return "\n\n---\n\n".join(all_protocols)


@mcp.tool()
def add_entry(content: str, layer: str = "episodic") -> str:
    """Add a new memory entry. Writes to local Brain if it exists, otherwise global.

    Args:
        content: The text to remember
        layer: Target layer — episodic | semantic | personal
    """
    ensure_brain()
    brain = brain_paths()[0]  # local wins if present, else global
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H_%M_%S_%f")[:-3]  # ms precision
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
    target = brain / "memory" / layer
    target.mkdir(parents=True, exist_ok=True)
    path = target / f"{timestamp}.md"
    path.write_text(entry)
    label = "local" if brain == _LOCAL else "global"
    return f"Saved to {label} {layer}: {path.name}"


@mcp.tool()
def get_status() -> str:
    """Return a count of entries in each memory layer for all active Brains."""
    ensure_brain()
    layers = [
        ("working", "memory/working"),
        ("episodic", "memory/episodic"),
        ("semantic", "memory/semantic"),
        ("skills", "memory/procedural/skills"),
        ("protocols", "memory/procedural/protocols"),
        ("personal", "memory/personal"),
    ]
    result = {}
    for brain in brain_paths():
        label = "local (.cerebrum/)" if brain == _LOCAL else "global (~/.cerebrum/)"
        counts = {}
        for name, rel in layers:
            p = brain / rel
            counts[name] = len(list(p.glob("*.md"))) if p.exists() else 0
        result[label] = counts
    return json.dumps(result, indent=2)


def run() -> None:
    mcp.run()


if __name__ == "__main__":
    run()
