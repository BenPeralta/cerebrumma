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


def _runtime_snapshot() -> tuple[Path, dict]:
    """Load the highest-priority published Business Brain snapshot."""
    for brain in brain_paths():
        snapshot_path = brain / "runtime" / "brain.json"
        if snapshot_path.exists():
            try:
                return snapshot_path, json.loads(snapshot_path.read_text())
            except (OSError, json.JSONDecodeError) as exc:
                raise RuntimeError(
                    f"Published Business Brain is unreadable: {snapshot_path}: {exc}"
                ) from exc
    raise RuntimeError(
        "No published Business Brain found. Run `cerebrum validate` and "
        "`cerebrum publish` first."
    )


def _parse_time(value) -> datetime | None:
    if not value:
        return None
    try:
        return datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    except ValueError:
        return None


def _is_live(entry: dict, now: datetime) -> bool:
    """Re-check the approval window at read time.

    A snapshot can outlive an entry's expiry (or predate its effective date),
    so publication-time filtering alone is not enough.
    """
    expires_at = _parse_time(entry.get("expires_at"))
    effective_at = _parse_time(entry.get("effective_at"))
    if expires_at and expires_at <= now:
        return False
    if effective_at and effective_at > now:
        return False
    return True


def _query_terms(value: str) -> set[str]:
    return {
        token.strip(".,:;!?()[]{}\"'").lower()
        for token in value.split()
        if len(token.strip(".,:;!?()[]{}\"'")) > 2
    }


@mcp.tool()
def get_business_context(
    query: str,
    role: str,
    customer_facing: bool = True,
    limit: int = 8,
) -> str:
    """Retrieve approved, versioned Business Brain context for an agent task.

    Only entries emitted by `cerebrum publish` are eligible. Results include
    memory IDs and the Brain version so downstream agents can cite exactly
    which approved knowledge governed an action.

    Args:
        query: The task or question the agent is working on
        role: Agent role/scope, for example sales, support, or engineering
        customer_facing: Exclude entries not approved for customer-facing use
        limit: Maximum approved entries to return (1-20)
    """
    ensure_brain()
    snapshot_path, snapshot = _runtime_snapshot()
    query_terms = _query_terms(query)
    normalized_role = role.strip().lower()
    ranked = []
    now = datetime.now(timezone.utc)

    for entry in snapshot.get("entries", []):
        if not _is_live(entry, now):
            continue
        scopes = {str(scope).lower() for scope in entry.get("scope", [])}
        if normalized_role not in scopes and "all" not in scopes:
            continue
        if customer_facing and not entry.get("customer_safe", False):
            continue

        searchable = " ".join([
            str(entry.get("id", "")),
            str(entry.get("authority", "")),
            str(entry.get("conflict_key", "")),
            str(entry.get("content", "")),
        ])
        overlap = len(query_terms & _query_terms(searchable))
        authority_bonus = 1 if entry.get("authority") in {
            "company_policy", "product_truth", "pricing_truth", "legal_policy"
        } else 0
        ranked.append((overlap + authority_bonus, entry))

    ranked.sort(key=lambda item: (-item[0], item[1].get("id", "")))
    selected = [entry for _, entry in ranked[:max(1, min(limit, 20))]]
    return json.dumps({
        "schema_version": snapshot.get("schema_version"),
        "brain_version": snapshot.get("brain_version"),
        "snapshot": str(snapshot_path),
        "role": normalized_role,
        "customer_facing": customer_facing,
        "entries": selected,
    }, indent=2)


_WRITABLE_LAYERS = {"episodic", "semantic", "personal"}


@mcp.tool()
def add_entry(content: str, layer: str = "episodic") -> str:
    """Add a new memory entry. Writes to local Brain if it exists, otherwise global.

    Args:
        content: The text to remember
        layer: Target layer — episodic | semantic | personal
    """
    ensure_brain()
    if layer not in _WRITABLE_LAYERS:
        raise ValueError(f"layer must be one of: {', '.join(sorted(_WRITABLE_LAYERS))}")
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
