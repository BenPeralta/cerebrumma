# Design taste

Cerebrumma stores named mood boards so a coding agent can generate UI that
looks like a person, not like a default component library. [mockitup](https://github.com/BenPeralta/mockitup)
is the Stripe-billed app that edits these boards. This document is the
Cerebrumma side of that contract. mockitup owns its own schema note; keep the
two aligned against the rules below.

Boards are plain Markdown with YAML frontmatter. Tokens are structured data
in that frontmatter. Prose in the body is guidance, not a second source of
colors.

Schema id: `cerebrumma.design-board.v1`

Example: [`templates/design-board.md`](../templates/design-board.md)

## Where boards live

| Scope | Path |
|---|---|
| Project | `.cerebrum/memory/design/<slug>.md` |
| Global | `~/.cerebrum/memory/design/<slug>.md` |
| Dream proposals | `<brain>/dream/design-proposals/*.md` |

The filename stem is the slug. `warm-editorial.md` must contain
`slug: "warm-editorial"`. A new `cerebrum init` creates `memory/design/`.
Older brains grow the directory the first time a board is saved.

`memory/design/` is not business truth. `cerebrum validate` and
`cerebrum publish` skip it, even if a file copies governance keys such as
`status` or `runtime` into the frontmatter.

## Merge

Same rules as the other memory layers:

- Read the global brain, then the project brain.
- The same slug in the project replaces the global board entirely. Fields are
  not merged inside one slug.
- Different slugs from both brains are all visible.
- `get_design` and `cerebrum design show` with no name use the effective
  default (see below).

## Default board

A board with `default: true` is a candidate. The effective default is chosen
in this order:

1. A project board beats a global board.
2. If several boards in the winning scope are flagged, the latest
   `updated_at` wins.
3. A remaining tie breaks by slug, ascending.

`cerebrum design set-default <slug>` flags that board and clears the flag on
every other board in the same brain. It does not change the other brain.
`cerebrum design list` prints a warning when more than one merged board is
flagged.

## Document shape

```markdown
---
schema: "cerebrumma.design-board.v1"
name: "Warm Editorial"
slug: "warm-editorial"
description: "Warm paper, ink, and one terracotta accent."
default: false
palette:
  background: "#F6F1E8"
  surface: "#FFFBF6"
  ink: "#1C1917"
  accent: "#C2410C"
typography:
  families:
    display: "Fraunces"
    body: "Source Serif 4"
    mono: "IBM Plex Mono"
  scale:
    base: "1rem"
    display: "3.25rem"
  weights:
    regular: 400
    bold: 700
spacing:
  "4": "1rem"
  "8": "2rem"
radius:
  md: "10px"
shadows:
  md: "0 10px 28px rgb(28 25 23 / 0.08)"
motion:
  feel: "calm"
  duration:
    base: "220ms"
  easing:
    standard: "cubic-bezier(0.2, 0, 0, 1)"
components:
  button: "Solid accent fill, md radius, no gradient."
vibe: "Quiet editorial. Generous margins. One accent."
references:
  - kind: "path"
    value: "references/warm-editorial/hero.png"
    note: "Homepage hero crop"
  - kind: "url"
    value: "https://example.com/mood/warm"
    note: "Public mood reference"
provenance: "manual"
created_at: "2026-11-10T16:00:00Z"
updated_at: "2026-11-10T16:00:00Z"
source_tool: "cli"
---

Guidance for a coding agent. Tokens above win if this prose disagrees.
```

Hex colors must be quoted. In YAML, `#F6F1E8` is a comment, so an unquoted
color disappears.

### Fields

| Field | Required on write | Notes |
|---|---|---|
| `schema` | yes | Exactly `cerebrumma.design-board.v1`. Readers treat a missing schema as v1. Any other value is rejected. |
| `name` | yes | 1–80 characters. Display label. |
| `slug` | yes | `^[a-z0-9]+(?:-[a-z0-9]+)*$`, at most 64 characters. Identity. Derived from `name` when omitted on import. |
| `description` | no | 0–600 characters. |
| `default` | no | Boolean. Default `false`. |
| `palette` | yes | At least one role. Recommended roles: `background`, `surface`, `ink`, `accent`. Extra roles are allowed (`muted`, `border`, `danger`, `success`, …). |
| `typography.families` | no | Role → family name. Recommended roles: `display`, `body`, `mono`. A comma separates a fallback stack. |
| `typography.scale` | no | Step → length. |
| `typography.weights` | no | Name → integer 1–1000. |
| `spacing` | no | Step → length. This is the spacing scale. |
| `radius` | no | Name → length. |
| `shadows` | no | Name → CSS box-shadow, or `none`. This is elevation. |
| `motion.feel` | no | Short phrase, at most 80 characters. Suggested values: `calm`, `snappy`, `playful`, `restrained`, `none`. |
| `motion.duration` | no | Name → time (`ms` or `s`). |
| `motion.easing` | no | Name → curve such as `cubic-bezier(0.2, 0, 0, 1)`. |
| `components` | no | Component name → one-line note, at most 300 characters. |
| `vibe` | no | One short paragraph, at most 600 characters. |
| `references` | no | Up to 24 images. |
| `provenance` | yes | Where the board came from. `manual`, `import`, or `reviewed:<person>:design-proposal:<file>` after an approved dream update. |
| `created_at`, `updated_at` | yes | RFC 3339. |
| `source_tool` | yes | `cli` for boards written by the CLI. `[A-Za-z0-9_-]{1,40}`. |
| body | no | Markdown guidance, at most 8000 characters. |

Unknown top-level keys, and unknown keys inside `typography` and `motion`,
are kept and round-tripped as extensions. mockitup can hang an id off the
board (`mockitup_id` in the example) without Cerebrumma dropping it on
`design add --from`. Keys may be 1–64 characters and must not contain
`: # { } [ ] ,` or control characters. Each extension value must serialize
to at most 4000 bytes of YAML.

Token names (`background`, `4`, `2xl`, `button`) are 1–32 ASCII letters,
digits, `_`, or `-`, and must start with a letter or digit. Palette roles
are lowercased and must start with a letter.

Lengths are `0` or a non-negative number plus `px`, `rem`, or `em`.
Durations are a non-negative number plus `ms` or `s`.

### References

```yaml
references:
  - kind: "path"
    value: "references/warm-editorial/hero.png"
    note: "Homepage hero crop"
  - kind: "url"
    value: "https://example.com/mood/warm"
    note: "Public mood reference"
```

`kind` is `path` or `url`. Paths are relative, at most 200 characters, and
must not be absolute or contain `..`. URLs are `http` or `https`, with no
whitespace. `note` is optional.

### Palette colors

Accepted forms are `#RGB`, `#RGBA`, `#RRGGBB`, and `#RRGGBBAA`. The CLI
normalizes them to uppercase `#RRGGBB` or `#RRGGBBAA` (`#c24` becomes
`#CC2244`). Writers should emit the normalized form. Named colors (`red`,
`canvas`) are rejected so an agent cannot invent a second color language.

## Mixing

A resolved board has one base. Each of these flags replaces that facet and
leaves the rest of the base alone:

| Flag | CLI | MCP argument | Tokens replaced |
|---|---|---|---|
| palette | `--palette <slug>` | `palette` | `palette` |
| type | `--type <slug>` (alias `--typography`) | `typography` | `typography` |
| spacing | `--spacing <slug>` | `spacing` | `spacing` |
| radius | `--radius <slug>` | `radius` | `radius` |
| elevation | `--elevation <slug>` | `elevation` | `shadows` |
| motion | `--motion <slug>` | `motion` | `motion` |

The result is not saved. `mixed_from` lists only the facets that came from a
different board. `provenance` becomes `mixed:<base slug>`. Component notes,
vibe, references, and guidance stay with the base board.

## JSON shape

`cerebrum design show --json`, `cerebrum design export --format json`, and
the `get_design` MCP tool return the same document:

```json
{
  "schema": "cerebrumma.design-board.v1",
  "slug": "warm-editorial",
  "name": "Warm Editorial",
  "description": "Warm paper, ink, and one terracotta accent.",
  "brain": "local",
  "default": true,
  "palette": { "accent": "#C2410C", "background": "#F6F1E8", "ink": "#1C1917", "surface": "#FFFBF6" },
  "typography": {
    "families": { "body": "Source Serif 4", "display": "Fraunces" },
    "scale": { "base": "1rem" },
    "weights": { "bold": 700, "regular": 400 }
  },
  "spacing": { "4": "1rem" },
  "radius": { "md": "10px" },
  "shadows": { "md": "0 10px 28px rgb(28 25 23 / 0.08)" },
  "motion": {
    "feel": "calm",
    "duration": { "base": "220ms" },
    "easing": { "standard": "cubic-bezier(0.2, 0, 0, 1)" }
  },
  "components": { "button": "Solid accent fill, md radius, no gradient." },
  "vibe": "Quiet editorial. One accent.",
  "references": [{ "kind": "path", "value": "references/warm-editorial/hero.png", "note": "Homepage hero crop" }],
  "guidance": "Markdown body.",
  "provenance": "manual",
  "created_at": "2026-11-10T16:00:00Z",
  "updated_at": "2026-11-10T16:00:00Z",
  "source_tool": "cli",
  "extensions": { "mockitup_id": "mb_warm" },
  "mixed_from": { "palette": { "slug": "night-studio", "brain": "global" } },
  "agent_instructions": "Apply these tokens when generating UI. ..."
}
```

`extensions` and `mixed_from` are omitted when empty. `default` is false when
the document is a mix, so a mix is not reported as the saved default.
`brain` is `local` or `global` and refers to the base board.
`agent_instructions` tells a coding agent that the tokens are authoritative.

`cerebrum design list --json` returns:

```json
{
  "schema": "cerebrumma.design-index.v1",
  "effective_default": "warm-editorial",
  "boards": [
    {
      "slug": "warm-editorial",
      "name": "Warm Editorial",
      "description": "Warm paper, ink, and one terracotta accent.",
      "brain": "local",
      "default": true,
      "updated_at": "2026-11-10T16:00:00Z",
      "path": "memory/design/warm-editorial.md"
    }
  ]
}
```

`path` is relative to that board's brain. `default` on each row is the flag
stored in the file. `effective_default` is the board `get_design` uses when
no name is passed.

## CLI

```sh
cerebrum design add --name "Warm Editorial" --color background=#F6F1E8 --color accent=#C2410C --default
cerebrum design add --from ./board.md --replace
cerebrum design list
cerebrum design list --json
cerebrum design show                  # effective default
cerebrum design show warm-editorial --json
cerebrum design show warm-editorial --palette night-studio --type warm-editorial
cerebrum design set-default warm-editorial
cerebrum design set-default warm-editorial --global
cerebrum design remove warm-editorial
cerebrum design export --format css
cerebrum design export --format tailwind -o tailwind.theme.js
cerebrum design export --format json -o tokens.json
cerebrum design export --format swiftui -o MoodBoard.swift
cerebrum design proposals
cerebrum design apply dream/design-proposals/<file>.md --approve --reviewed-by "Ada"
```

`add` writes the project brain unless `--global` is set. Repeating a token
flag merges that key onto the board being written. `--from` reads a full
board file first. An existing slug is not overwritten unless `--replace` is
passed. `--replace` keeps `created_at` and, unless `--default` is also
passed, keeps the current default flag.

Export formats:

| `--format` | What you get |
|---|---|
| `json` | The token document above. This is the structured interchange. |
| `css` | A `:root` block of custom properties: `--color-*`, `--font-*`, `--text-*`, `--font-weight-*`, `--space-*`, `--radius-*`, `--shadow-*`, `--motion-feel`, `--duration-*`, `--ease-*`. |
| `tailwind` | A CommonJS `theme.extend` config (`colors`, `fontFamily`, `fontSize`, `fontWeight`, `spacing`, `borderRadius`, `boxShadow`, `transitionDuration`, `transitionTimingFunction`). |
| `swiftui` | A `MoodBoard` enum. `1rem` and `1em` become 16pt. Colors are sRGB `Color` values. Numeric token names are prefixed with `_` (`4` becomes `_4`). |

## MCP

`get_design(board="", palette="", typography="", spacing="", radius="", elevation="", motion="")`

Empty `board` selects the effective default. The return value is the JSON
document above.

`list_designs()`

Returns the design index above.

`read_memory` accepts layer `design` and returns the raw Markdown. `add_entry`
does not write this layer; boards have a schema, so they go through
`cerebrum design`. `get_status` includes a `design` count. The other tools
are unchanged.

## Dream proposals

`cerebrum dream` asks the model for an optional `## Design Taste` section.
Bullets use this shape:

```text
- board: warm-editorial | action: update | note: Episodes keep asking for warm paper.
- board: night-studio | action: create | note: A dark studio palette showed up twice.
- (none)
```

Ingest writes `dream/design-proposals/<timestamp>-<index>-<slug>.md` with
schema `cerebrumma.design-proposal.v1` and `status: proposed`. It does not
create or edit `memory/design/`. A note with no structured `board` / `action`
is stored as `action: note` and cannot be applied until a person edits it.

Applying a proposal requires both:

- `--approve`
- a fenced board inside the proposal

````markdown
```board
---
schema: "cerebrumma.design-board.v1"
name: "Warm Editorial"
slug: "warm-editorial"
palette:
  background: "#F6F1E8"
  surface: "#FFFBF6"
  ink: "#1C1917"
  accent: "#115511"
provenance: "manual"
created_at: "2026-11-10T16:00:00Z"
updated_at: "2026-11-10T16:00:00Z"
source_tool: "cli"
---
```
````

`action: update` replaces an existing board and keeps `created_at`.
`action: create` refuses when the slug already exists. The written board's
provenance is `reviewed:<reviewed-by>:design-proposal:<filename>`. The
proposal file is then `status: applied` and will not apply twice.
`--reviewed-by` defaults to `cli`.

This matches the business-brain rule: reflection may propose, and a person
has to approve before the proposal becomes the stored board.

## What mockitup needs to match

Write and read `cerebrumma.design-board.v1` as Markdown plus YAML
frontmatter, with tokens in the fields above rather than only in prose.

- Use the slug as the filename and as `slug`.
- Quote hex colors. Prefer uppercase `#RRGGBB` / `#RRGGBBAA`.
- Include `background`, `surface`, `ink`, and `accent` when the board has a
  full palette. Additional roles are fine.
- Put font stacks, the type scale, and weights under `typography`. Put the
  spacing scale in `spacing`, corner radii in `radius`, and elevation in
  `shadows`.
- Use `references[].kind` of `path` or `url`. Keep image paths relative.
- Set `default: true` on the board coding agents should load when no name is
  given. A project file replaces a global file of the same slug.
- Keep mockitup-only ids in extension keys such as `mockitup_id`. Cerebrumma
  round-trips them. Do not put them in a second file that the CLI would miss.
- Exchange a single board as the Markdown file, or as `cerebrum design export --format json`.
- Do not import `dream/design-proposals/` as boards. Those are unapproved
  notes. A proposal becomes a board only after `cerebrum design apply --approve`.
- Do not rely on Business Brain keys (`runtime`, `status`, `authority`,
  `owner`, `scope`, `autonomy_level`) for taste. They are not part of this
  schema, and publish ignores this directory.

If mockitup's schema doc uses different names, map them onto this document
before either side writes the other's files. The version string stays
`cerebrumma.design-board.v1` until a field changes meaning.
