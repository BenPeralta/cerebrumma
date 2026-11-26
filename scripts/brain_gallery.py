#!/usr/bin/env python3
"""Build one neural-map page holding every Cerebrumma Brain on this machine.

    python3 scripts/brain_gallery.py [--out FILE] [--root DIR] [--demo-only]

Finds every .cerebrum/ folder under --root (default: home, 4 levels deep),
maps every memory file in each, and stitches the graphs into templates/brain_3d.html with a project switcher. "Demo" is always
included, so the page can be shown without revealing any real memories.
"""
import argparse, html, json, os, re, sqlite3, sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TEMPLATE = REPO / "templates" / "brain_3d.html"
SKIP = {"node_modules", ".git", "Library", ".Trash", "target", "build", "DerivedData"}


def find_brains(root: Path, depth: int):
    for dirpath, dirnames, _ in os.walk(root):
        rel = Path(dirpath).relative_to(root)
        if ".cerebrum" in dirnames:
            yield Path(dirpath)
        if len(rel.parts) >= depth:
            dirnames[:] = []
        else:
            dirnames[:] = [d for d in dirnames if d not in SKIP and (not d.startswith(".") or d == ".cerebrum") and d != ".cerebrum"]


def desk_name(project: Path):
    """Business Desk keeps brains under data/brains/<business id>; name them from desk.sqlite."""
    db = project.parent.parent / "desk.sqlite"
    if project.parent.name != "brains" or not db.exists():
        return None
    try:
        with sqlite3.connect(f"file:{db}?mode=ro", uri=True) as c:
            row = c.execute("select name from businesses where id = ?", (project.name,)).fetchone()
        return row[0] if row else ("Portfolio" if project.name == "portfolio" else None)
    except sqlite3.Error:
        return None


LAYER_OF = [("episodic", "Episodic"), ("semantic", "Semantic"), ("procedural", "Procedural"),
            ("skills", "Procedural"), ("protocols", "Procedural"), ("personal", "Personal"),
            ("equity", "Personal"), ("dream", "Reflection"), ("audit", "Reflection")]
STOP = set("the a an and or of to in on for with is are was be this that it as at by from not use when we you i our".split())


def read_memory(path: Path):
    """Return the memory text, preferring the frontmatter `content:` field."""
    raw = path.read_text(errors="ignore")
    body = raw
    if raw.startswith("---"):
        parts = raw.split("---", 2)
        if len(parts) == 3:
            fm, body = parts[1], parts[2]
            m = re.search(r"^content:\s*(.*?)(?=^\w[\w_]*:|\Z)", fm, re.S | re.M)
            if m and m.group(1).strip():
                body = m.group(1)
    text = re.sub(r"\s+", " ", body).strip().strip("'\"|>").strip()
    return text.replace("''", "'")


def graph_for(project: Path):
    """Every memory file in the Brain becomes a node; edges link each to its
    closest neighbours by TF-IDF similarity (no embeddings needed, so nothing
    is left out of the map)."""
    import math
    root = project / ".cerebrum" / "memory"
    files = sorted(f for f in root.rglob("*.md") if f.is_file()) if root.exists() else []
    nodes, docs = [], []
    for f in files:
        text = read_memory(f)
        if not text or f.name == "000-welcome.md":
            continue
        rel = str(f.relative_to(root)).lower()
        layer = next((name for key, name in LAYER_OF if key in rel), "Other")
        nodes.append({"id": len(nodes), "label": text[:70], "layer": layer})
        docs.append([w for w in re.findall(r"[a-z][a-z0-9_]{2,}", text.lower()) if w not in STOP])
    if not nodes:
        return None
    df = {}
    for d in docs:
        for w in set(d):
            df[w] = df.get(w, 0) + 1
    n = len(docs)
    vecs = []
    for d in docs:
        tf = {}
        for w in d:
            tf[w] = tf.get(w, 0) + 1
        v = {w: c * math.log((1 + n) / (1 + df[w])) for w, c in tf.items()}
        norm = math.sqrt(sum(x * x for x in v.values())) or 1
        vecs.append({w: x / norm for w, x in v.items()})
    edges, seen = [], set()
    for i, a in enumerate(vecs):
        sims = []
        for j, b in enumerate(vecs):
            if i != j:
                small, big = (a, b) if len(a) < len(b) else (b, a)
                sim = sum(x * big.get(w, 0) for w, x in small.items())
                if sim > 0.18:
                    sims.append((sim, j))
        for sim, j in sorted(sims, reverse=True)[:3]:
            key = (min(i, j), max(i, j))
            if key not in seen:
                seen.add(key)
                edges.append({"source": i, "target": j, "weight": round(0.55 + 0.45 * min(sim, 1), 3)})
    return {"nodes": nodes, "edges": edges}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(Path.home()))
    ap.add_argument("--depth", type=int, default=4)
    ap.add_argument("--out", default=str(Path.home() / "Desktop" / "cerebrumma-brains.html"))
    ap.add_argument("--demo-only", action="store_true", help="Ship only the demo brain (safe to share)")
    a = ap.parse_args()

    brains = {"Demo": {"nodes": [], "edges": []}}
    groups = {}
    if not a.demo_only:
        seen = set()
        for project in sorted(find_brains(Path(a.root), a.depth)):
            data = graph_for(project)
            if not data:
                continue
            key = json.dumps(data["nodes"], sort_keys=True)
            if key in seen:  # identical copies (e.g. a repo cloned twice)
                continue
            seen.add(key)
            group = "Business brains" if desk_name(project) else "Project brains"
            name = "Global" if project == Path.home() else (desk_name(project) or project.name)
            while name in brains:
                name += "'"
            brains[name] = data
            groups.setdefault(group, []).append(name)
            print(f"  {name}: {len(data['nodes'])} memories")

    t = TEMPLATE.read_text()
    opt = lambda n: f'<option value="{html.escape(n)}">{html.escape(n)}</option>'
    options = opt("Demo") + "".join(
        f'<optgroup label="{g}">' + "".join(opt(n) for n in sorted(groups[g], key=str.lower)) + "</optgroup>"
        for g in ("Business brains", "Project brains") if g in groups)
    t = t.replace("<title>Cerebrumma · {{project_name}}</title>", "<title>Cerebrumma Brains</title>")
    t = t.replace('<span class="proj">{{project_name}}</span>',
                  f'<select id="brainPick" aria-label="Brain">{options}</select>')
    t = t.replace("Memories in<br>{{project_name}}", 'Memories in<br><span class="pname"></span>')
    picker_css = ("#brainPick{appearance:none;background:rgba(255,255,255,.06);color:#e8eaee;border:1px solid rgba(255,255,255,.12);"
                  "border-radius:8px;padding:5px 26px 5px 10px;font:inherit;font-size:12px;cursor:pointer;"
                  "background-image:linear-gradient(45deg,transparent 50%,#8b8f98 50%),linear-gradient(135deg,#8b8f98 50%,transparent 50%);"
                  "background-position:calc(100% - 13px) 50%,calc(100% - 9px) 50%;background-size:4px 4px;background-repeat:no-repeat}"
                  "#brainPick option{background:#14161b}")
    t = t.replace("</style>", picker_css + "</style>", 1)
    payload = json.dumps(brains, separators=(",", ":")).replace("</", "<\\/")
    boot = f"""<script type="application/json" id="brains">{payload}</script>
<script type="application/json" id="graph-data"></script>
<script>(()=>{{
  const B = JSON.parse(document.getElementById('brains').textContent);
  const names = Object.keys(B);
  let pick = decodeURIComponent(location.hash.slice(1));
  if (!B[pick]) pick = names.filter(n => n !== 'Demo').sort((a, b) => B[b].nodes.length - B[a].nodes.length)[0] || 'Demo';
  document.getElementById('graph-data').textContent = JSON.stringify(B[pick]);
  document.querySelectorAll('.pname').forEach(e => e.textContent = pick);
  document.title = 'Cerebrumma · ' + pick;
  const sel = document.getElementById('brainPick');
  sel.value = pick;
  sel.addEventListener('change', () => {{ location.hash = encodeURIComponent(sel.value); location.reload(); }});
}})();</script>"""
    t = t.replace('<script type="application/json" id="graph-data">{{graph_data}}</script>', boot)
    assert "{{" not in t.split("<script>(()=>{var Kp")[0], "template placeholders left"
    Path(a.out).write_text(t)
    print(f"✓ {len(brains)} brains → {a.out}")


if __name__ == "__main__":
    sys.exit(main())
