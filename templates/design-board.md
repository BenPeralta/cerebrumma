---
schema: "cerebrumma.design-board.v1"
name: "Warm Editorial"
slug: "warm-editorial"
description: "Warm paper, ink, and one terracotta accent."
default: true
palette:
  accent: "#C2410C"
  background: "#F6F1E8"
  ink: "#1C1917"
  surface: "#FFFBF6"
typography:
  families:
    body: "Source Serif 4"
    display: "Fraunces"
    mono: "IBM Plex Mono"
  scale:
    base: "1rem"
    display: "3.25rem"
  weights:
    bold: 700
    regular: 400
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
  button: "Solid accent fill, ink-on-accent label, md radius, no gradient."
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
mockitup_id: "mb_warm"
---

Use the tokens above as the source of truth. This body is guidance for a coding agent: paper-like surfaces, one accent, no glassmorphism.
