# Cerebrumma Roadmap

## Done
- [x] `cerebrum init` — global + local Brain setup
- [x] `cerebrum add` — manual memory entry
- [x] `cerebrum status` — entry counts per layer
- [x] `cerebrum hook install` — git commit auto-capture
- [x] `cerebrum watch` — file save auto-capture
- [x] `cerebrum dream` — generate reflection prompt
- [x] `cerebrum dream --ingest` — parse LLM response into memory layers
- [x] MCP server — 6 tools, including role-scoped `get_business_context`
- [x] Installer script (`install.sh`) — Rust + uv + MCP + Claude Code auto-config
- [x] Public site — cerebrumma.com (Next.js 15, light mode, warm palette)
- [x] Integration guides — Claude Code, Cursor, Windsurf, OpenAI Codex, Google Antigravity
- [x] SEO — GA4, sitemap, robots, JSON-LD, OG tags, canonical URLs
- [x] Privacy policy + Terms of Service
- [x] `cerebrum search` — semantic search across memory layers from CLI
- [x] `cerebrum why` — contextual trace with memory IDs
- [x] `cerebrum audit` — health and decay checks
- [x] `cerebrum stats` — usage and health statistics
- [x] Governed Business Brain — approvals, authority, scope, conflict detection, expiry, supersession
- [x] `cerebrum validate` — reject unsafe or contradictory runtime knowledge
- [x] `cerebrum publish` — deterministic, versioned runtime snapshot for agents

---

## Next Up

### Business Brain Runtime
- [x] **Governed knowledge schema** — approval, ownership, provenance, scope, and autonomy metadata.
- [x] **Deterministic snapshots** — agents can log the exact Brain version and memory IDs used.
- [x] **Customer-safety filtering** — internal-only knowledge stays out of customer-facing tasks.
- [ ] **Signed cloud distribution** — securely deliver approved snapshots to hosted workers.
- [ ] **Evaluation gate** — scenario tests must pass before a new Brain version is promoted.
- [ ] **Outcome ingestion** — proposed learning enters review without silently rewriting policy.

### `cerebrum dream --auto` (Autopilot Gardener) [COMPLETED]
- [x] **Activity threshold** — triggered after 10 saturation points.
- [x] **Universal Provider** — supports Gemini and Anthropic.
- [x] **Secure Config** — keys stored in `~/.cerebrum/config.json`.
- [x] **System Persistence** — background daemon via `launchd`.
- [ ] **Dry-run preview** — `cerebrum dream --auto --preview` shows estimated costs.
- [ ] **Max frequency cap** — prevent excessive API calls.

### `cerebrum brief` & `cerebrum fix` [NEW]
- [x] **Digital Janitor** — surface actionable debt from reflections.
- [x] **Situational Briefing** — catch up AI agents in one command.
- [x] **Visual Brain Map** — premium glassmorphism dashboard.

---

## Backlog

- [ ] `cerebrum init` should auto-run `cerebrum hook install` — one command instead of two


- [ ] `og-image.png` — 1200×630 social sharing image for cerebrumma.com
- [ ] `get.cerebrumma.com` HTTPS — move redirect from name.com to Vercel redirect rule (currently HTTP-only)
- [ ] `cerebrum prune` — interactive review and deletion of stale entries
- [ ] `cerebrum export` — dump Brain to JSON/CSV for portability
- [ ] Windows support — installer + CLI
- [ ] VS Code extension — surface Brain status in status bar
- [ ] Team Brain — shared `.cerebrum/` in a git repo with conflict resolution
