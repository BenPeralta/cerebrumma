# Cerebrumma

**One Brain. Any Tool.**  
Your AI memory — portable, git-tracked, and always yours.

Cerebrumma is a local-first memory layer for developers. It lives in `.cerebrum/` inside your repo, works with any MCP-compatible AI tool (Claude, Cursor, Antigravity, and more), and compounds knowledge over time through automatic capture and nightly dream cycles.

No vendor lock-in. No API keys. Just a folder that makes your AI smarter.

---

## Install

```bash
cargo install cerebrumma
```

Or build from source:

```bash
git clone https://github.com/BenPeralta/cerebrumma
cd cerebrumma
cargo install --path .
```

---

## Quickstart

```bash
# Initialize a Brain in any project
cerebrum init

# Add a rule or context
cerebrum add "We use TypeScript strict mode and functional React components"

# Check what your Brain knows
cerebrum status

# Start the file watcher (auto-captures changes)
cerebrum watch
```

---

## How It Works

Cerebrumma stores memory in five layers inside `.cerebrum/`:

| Layer | Purpose |
|---|---|
| `working/` | Current session context |
| `episodic/` | Time-stamped events and decisions |
| `semantic/` | Facts and domain knowledge |
| `procedural/` | Skills, protocols, coding rules |
| `personal/` | User preferences and equity rules |

Every entry is a Markdown file with YAML frontmatter — human-readable, git-diffable, fully auditable.

---

## MCP Integration

Register Cerebrumma as an MCP server so any compatible AI tool can read and write your Brain automatically:

```bash
claude mcp add cerebrumma /path/to/uv -- run --project mcp_server cerebrumma-mcp
```

Available tools: `read_memory`, `search_memory`, `get_protocols`, `add_entry`, `get_status`

---

## Roadmap

- [x] CLI: `init`, `add`, `status`, `template`
- [x] MCP server (5 tools)
- [ ] File watcher (`cerebrum watch`)
- [ ] Dream cycle (`cerebrum dream`)
- [ ] Cloud sync (Pro)
- [ ] Templates marketplace

---

## License

MIT OR Apache-2.0
