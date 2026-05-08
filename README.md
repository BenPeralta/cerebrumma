# Cerebrumma

**One Brain. Any Tool.**

Cerebrumma is a portable, git-tracked AI memory layer for developers. It lives in `.cerebrum/` in your repo, works with Claude Code, Cursor, Antigravity and more — with built-in explainability and equity guardrails. No keys. No lock-in. Intelligence that compounds.

## Install

```sh
curl -fsSL https://get.cerebrumma.com | sh
```

Installs the `cerebrum` CLI, MCP server, and configures Claude Code automatically. Requires macOS or Linux — Rust and uv are installed for you if missing.

## Quickstart

```sh
# Create your global Brain (once, ever)
cerebrum init --global
cerebrum add --global "I always use TypeScript strict mode"
cerebrum add --global "Never use gendered language in UI copy"

# Add a local Brain to any project
cd ~/my-project
cerebrum init
cerebrum hook install        # auto-capture every git commit

# See what your Brain knows
cerebrum status

# Run a dream cycle (compress + reflect + self-improve)
cerebrum dream
```

## How It Works

Memory lives in five layers inside `.cerebrum/`:

```
.cerebrum/
├── memory/
│   ├── episodic/       ← what happened (auto-captured by git hook)
│   ├── semantic/       ← distilled facts and knowledge
│   ├── procedural/     ← skills and protocols (rules you follow)
│   └── personal/       ← equity rules and preferences
├── embeddings/         ← local vector index (coming soon)
├── logs/               ← audit trail
└── dream/              ← reflection staging area
```

Every entry is plain Markdown with YAML frontmatter — human-readable, git-diffable, no database.

## Commands

| Command | What it does |
|---|---|
| `cerebrum init` | Create `.cerebrum/` in the current project |
| `cerebrum init --global` | Create `~/.cerebrum/` shared across all projects |
| `cerebrum add "..."` | Add an episodic memory entry |
| `cerebrum add --global "..."` | Add to the global Brain |
| `cerebrum status` | Show entry counts across all memory layers |
| `cerebrum hook install` | Install git post-commit hook (auto-captures commits) |
| `cerebrum hook remove` | Remove the hook |
| `cerebrum watch` | Watch for file saves and auto-capture |
| `cerebrum dream` | Archive episodes + generate LLM reflection prompt |
| `cerebrum dream --ingest <file>` | Parse LLM reflection into the right memory layers |
| `cerebrum template --list` | List available community templates |

## Global + Local Brain Hierarchy

```sh
cerebrum init --global   # ~/.cerebrum/ — your rules, everywhere
cerebrum init            # .cerebrum/   — this project's memory
```

When both exist, they merge automatically. Your personal rules always travel with you. Project memory stays scoped to the repo.

## MCP Integration

Cerebrumma runs as a local MCP server. Claude Code, Cursor, and Antigravity can call your Brain automatically — no manual `cerebrum inject` needed.

**Claude Code** (`~/.claude/settings.json`):
```json
{
  "mcpServers": {
    "cerebrumma": {
      "command": "uv",
      "args": ["run", "--project", "~/.cerebrumma/mcp_server", "cerebrumma-mcp"],
      "cwd": "${workspaceFolder}"
    }
  }
}
```

Available MCP tools: `read_memory` · `search_memory` · `get_protocols` · `add_entry` · `get_status`

## Dream Cycle

The self-improvement loop that turns a raw log into a learning system:

```sh
cerebrum dream                        # archives episodes, writes reflection prompt
# paste prompt into Claude / Grok / any LLM, save response as response.md
cerebrum dream --ingest response.md   # promotes insights into the right layers
```

The LLM reflection is parsed into four sections:

| Section | Destination |
|---|---|
| Key Insights | `semantic/` |
| New Rules | `procedural/skills/` |
| Equity & Bias Notes | `personal/` |
| Prune Suggestions | Printed for manual review — never auto-deleted |

## Memory Format

```markdown
---
timestamp: 2026-05-08T18:00:00Z
source_tool: cli
salience_score: 0.6
bias_flag: false
provenance: manual
---

Always prefer functional components in React.
```

## Roadmap

- [x] CLI: `init`, `add`, `status`, `hook`, `watch`, `dream`, `template`
- [x] Global + local Brain hierarchy
- [x] MCP server (5 tools, merges both Brains)
- [x] Git hook auto-capture (branch + hash + diff stats)
- [x] Dream cycle with `--ingest` parsing
- [ ] SQLite-vec embeddings + semantic search
- [ ] `cerebrum why` — full trace with memory IDs
- [ ] Templates marketplace
- [ ] Cloud sync (Pro — $24/mo)

## License

MIT OR Apache-2.0
