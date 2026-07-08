# Cerebrumma

**One Brain. Any Tool.**

Cerebrumma is a portable, git-tracked AI memory layer for developers. It lives in `.cerebrum/` in your repo, works with Claude Code, Cursor, Antigravity and more — with built-in explainability and equity guardrails. No keys. No lock-in. Intelligence that compounds.

## Install

```sh
curl -fsSL http://get.cerebrumma.com | sh
```

Downloads a prebuilt `cerebrum` binary for your platform (no compiler needed), installs the MCP server, and configures Claude Code automatically. Requires macOS (Apple Silicon or Intel) or Linux (x86_64).

Or with [Homebrew](https://brew.sh):

```sh
brew install BenPeralta/cerebrumma/cerebrumma
```

The tap installs the CLI only; run the one-liner above to also set up the MCP server and Claude Code integration.

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

# Start the background gardener (macOS launchd)
cerebrum daemon start

# See what your Brain knows
cerebrum status

# Let the Brain reflect and self-improve (calls LLM directly)
cerebrum dream --auto

# Visualize your Brain as an interactive 3D neural map
cerebrum map --graph
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
├── embeddings.db       ← local vector index (sqlite-vec)
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
| `cerebrum daemon start` | Start the background watcher and auto-dreamer |
| `cerebrum watch` | Watch for file saves (foreground) |
| `cerebrum dream` | Archive episodes + generate LLM reflection prompt |
| `cerebrum dream --auto` | Automatically call LLM (Gemini/Claude) and ingest |
| `cerebrum map` | Open a beautiful HTML visualization of your Brain |
| `cerebrum brief` | Generate a "State of the Union" context file |
| `cerebrum fix` | View actionable technical debt and prune suggestions |
| `cerebrum audit` | Audit brain health and decay |
| `cerebrum search <query>` | Semantic search across the Brain |
| `cerebrum why <query>` | Contextual trace explaining "why" a rule exists |
| `cerebrum config` | Set LLM providers and API keys |

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

## Dream Cycle (Autopilot Gardener)

The Brain can self-improve on autopilot. First, configure your LLM provider (keys are stored securely in `~/.cerebrum/config.json`):

```sh
cerebrum config set provider gemini
cerebrum config set api_key YOUR_API_KEY
```

Then, trigger a dream cycle manually or let the background daemon handle it:

```sh
cerebrum dream --auto                 # runs reflection right now
cerebrum daemon start                 # triggers automatically after 10 saturation points
```

The LLM reflection is parsed into four sections:

| Section | Destination |
|---|---|
| Key Insights | `semantic/` |
| New Rules | `procedural/skills/` |
| Equity & Bias Notes | `personal/` |
| Prune Suggestions | Viewed later using `cerebrum fix` |

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

- [x] CLI: `init`, `add`, `status`, `hook`, `watch`, `dream`, `map`, `fix`, `brief`
- [x] Global + local Brain hierarchy
- [x] MCP server (5 tools, merges both Brains)
- [x] Git hook auto-capture (branch + hash + diff stats)
- [x] Autopilot Dreaming (`dream --auto`) via Gemini/Claude
- [x] Background Daemon (`launchd`)
- [x] SQLite-vec embeddings + semantic search
- [x] Visual Brain Dashboard (`map`)
- [ ] Templates marketplace
- [ ] Cloud sync (Pro — $24/mo)
- [ ] Team Brain (shared repo sync)

## License

MIT OR Apache-2.0
