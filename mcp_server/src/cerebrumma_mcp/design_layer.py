"""Read `cerebrumma.design-board.v1` mood boards.

The on-disk format is Markdown with YAML frontmatter. Writers (the Rust CLI
and mockitup) own the file; this module only reads and merges them. See
`docs/design-layer.md` for the schema both sides have to agree on.
"""

from __future__ import annotations

import re
import unicodedata
from datetime import datetime
from pathlib import Path

import yaml

DESIGN_SCHEMA = "cerebrumma.design-board.v1"
INDEX_SCHEMA = "cerebrumma.design-index.v1"
AGENT_INSTRUCTIONS = (
    "Apply these tokens when generating UI. Palette, typography, spacing, "
    "radius, shadows, and motion are authoritative. Component notes, vibe, "
    "and guidance are taste, not a second palette. Do not invent a parallel "
    "color or type system. When mixed_from is set, take each listed facet "
    "from that board and everything else from this board."
)

_SLUG = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
_TOKEN = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,31}$")
_ROLE = re.compile(r"^[a-z][a-z0-9_-]{0,31}$")
_LENGTH = re.compile(r"^(0|[0-9]+(?:\.[0-9]+)?(?:px|rem|em))$")
_DURATION = re.compile(r"^[0-9]+(?:\.[0-9]+)?(?:ms|s)$")
_EASING = re.compile(r"^[A-Za-z0-9(),.\- ]{1,80}$")
_HEX = re.compile(r"^#(?:[0-9A-Fa-f]{3,4}|[0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$")


def normalize_hex(raw: str) -> str:
    value = str(raw).strip()
    if not _HEX.match(value):
        raise ValueError(f"color '{raw}' must be a #hex value")
    digits = value[1:]
    if len(digits) in (3, 4):
        digits = "".join(char * 2 for char in digits)
    return f"#{digits.upper()}"


def parse_board_text(text: str) -> dict:
    """Parse one mood board document into normalized token data."""
    if len(text.encode("utf-8")) > 65_536:
        raise ValueError("design board is larger than 64KB")
    data, body = _split_frontmatter(text)
    schema = str(data.pop("schema", DESIGN_SCHEMA))
    if schema != DESIGN_SCHEMA:
        raise ValueError(f"unsupported design schema '{schema}'")
    name = _text(data.pop("name", None), "name", 1, 80)
    slug = str(data.pop("slug", "")).strip() or _slugify(name)
    if not _SLUG.match(slug) or len(slug) > 64:
        raise ValueError(f"invalid board slug '{slug}'")
    palette = _palette(data.pop("palette", {}) or {})
    typography = _typography(data.pop("typography", {}) or {})
    motion = _motion(data.pop("motion", {}) or {})
    board = {
        "schema": schema,
        "name": name,
        "slug": slug,
        "description": _text(data.pop("description", ""), "description", 0, 600),
        "default": _flag(data.pop("default", False)),
        "palette": palette,
        "typography": typography,
        "spacing": _length_map(data.pop("spacing", {}) or {}, "spacing"),
        "radius": _length_map(data.pop("radius", {}) or {}, "radius"),
        "shadows": _shadows(data.pop("shadows", {}) or {}),
        "motion": motion,
        "components": _components(data.pop("components", {}) or {}),
        "vibe": _text(data.pop("vibe", ""), "vibe", 0, 600),
        "references": _references(data.pop("references", []) or []),
        "provenance": _line(data.pop("provenance", "manual"), "provenance", 1, 200),
        "created_at": _timestamp(data.pop("created_at", None), "created_at"),
        "updated_at": _timestamp(
            data.pop("updated_at", None) or None,
            "updated_at",
            fallback=True,
        ),
        "source_tool": _source_tool(data.pop("source_tool", "cli")),
        "extensions": _extensions(data),
        "guidance": body.strip()[:8000],
    }
    if not board["updated_at"]:
        board["updated_at"] = board["created_at"]
    return board


def load_boards(brains: list[tuple[str, Path]]) -> list[dict]:
    """Merge boards. `brains` is highest priority first; the first copy of a slug wins."""
    merged: dict[str, dict] = {}
    for label, brain in reversed(brains):
        for stored in _load_side(brain, label):
            merged[stored["slug"]] = stored
    return [merged[slug] for slug in sorted(merged)]


def effective_default(boards: list[dict]) -> dict | None:
    flagged = [board for board in boards if board["default"]]
    if not flagged:
        return None
    flagged.sort(key=lambda board: (
        0 if board["brain"] == "local" else 1,
        tuple(255 - ord(char) for char in board["updated_at"]),
        board["slug"],
    ))
    return flagged[0]


def resolve_design(
    boards: list[dict],
    name: str = "",
    palette: str = "",
    typography: str = "",
    spacing: str = "",
    radius: str = "",
    elevation: str = "",
    motion: str = "",
) -> dict:
    """Return the JSON document `get_design` and `cerebrum design show --json` share."""
    base = _find(boards, name) if name.strip() else effective_default(boards)
    if base is None:
        raise ValueError(
            "no default mood board. Pass a board slug or run `cerebrum design set-default <slug>`."
        )
    mixed_from: dict[str, dict] = {}
    palette_tokens = _facet(boards, base, palette, "palette", mixed_from, "palette")
    type_tokens = _facet(boards, base, typography, "typography", mixed_from, "typography")
    spacing_tokens = _facet(boards, base, spacing, "spacing", mixed_from, "spacing")
    radius_tokens = _facet(boards, base, radius, "radius", mixed_from, "radius")
    shadow_tokens = _facet(boards, base, elevation, "shadows", mixed_from, "elevation")
    motion_tokens = _facet(boards, base, motion, "motion", mixed_from, "motion")
    provenance = f"mixed:{base['slug']}" if mixed_from else base["provenance"]
    view = {
        "schema": DESIGN_SCHEMA,
        "slug": base["slug"],
        "name": base["name"],
        "description": base["description"],
        "brain": base["brain"],
        "default": base["default"] and not mixed_from,
        "palette": palette_tokens,
        "typography": _public_typography(type_tokens),
        "spacing": spacing_tokens,
        "radius": radius_tokens,
        "shadows": shadow_tokens,
        "motion": _public_motion(motion_tokens),
        "components": base["components"],
        "vibe": base["vibe"],
        "references": _public_references(base["references"]),
        "guidance": base["guidance"],
        "provenance": provenance,
        "created_at": base["created_at"],
        "updated_at": base["updated_at"],
        "source_tool": base["source_tool"],
        "agent_instructions": AGENT_INSTRUCTIONS,
    }
    if base["extensions"]:
        view["extensions"] = base["extensions"]
    if mixed_from:
        view["mixed_from"] = mixed_from
    return view


def index_designs(boards: list[dict]) -> dict:
    chosen = effective_default(boards)
    return {
        "schema": INDEX_SCHEMA,
        "effective_default": None if chosen is None else chosen["slug"],
        "boards": [
            {
                "slug": board["slug"],
                "name": board["name"],
                "description": board["description"],
                "brain": board["brain"],
                "default": board["default"],
                "updated_at": board["updated_at"],
                "path": board["path"],
            }
            for board in boards
        ],
    }


def _load_side(brain: Path, label: str) -> list[dict]:
    directory = brain / "memory" / "design"
    if not directory.is_dir():
        return []
    stored = []
    for path in sorted(directory.glob("*.md")):
        board = parse_board_text(path.read_text())
        if board["slug"] != path.stem:
            raise ValueError(f"{path} must be named {board['slug']}.md")
        board["brain"] = label
        board["path"] = f"memory/design/{board['slug']}.md"
        stored.append(board)
    return stored


def _find(boards: list[dict], name: str) -> dict:
    name = name.strip()
    for board in boards:
        if board["slug"] == name:
            return board
    by_name = [board for board in boards if board["name"] == name]
    if len(by_name) == 1:
        return by_name[0]
    known = ", ".join(board["slug"] for board in boards) or "none yet"
    if not by_name:
        raise ValueError(f"no mood board '{name}'. Known boards: {known}")
    raise ValueError(f"more than one mood board is named '{name}'; pass the slug")


def _facet(boards, base, requested, field, mixed_from, facet_name):
    if not str(requested).strip():
        return base[field]
    donor = _find(boards, requested)
    if donor["slug"] != base["slug"]:
        mixed_from[facet_name] = {"slug": donor["slug"], "brain": donor["brain"]}
    return donor[field]


def _public_typography(typography: dict) -> dict:
    view = {
        "families": typography["families"],
        "scale": typography["scale"],
        "weights": typography["weights"],
    }
    if typography["extensions"]:
        view["extensions"] = typography["extensions"]
    return view


def _public_motion(motion: dict) -> dict:
    view = {
        "feel": motion["feel"],
        "duration": motion["duration"],
        "easing": motion["easing"],
    }
    if motion["extensions"]:
        view["extensions"] = motion["extensions"]
    return view


def _public_references(references: list[dict]) -> list[dict]:
    published = []
    for reference in references:
        item = {"kind": reference["kind"], "value": reference["value"]}
        if reference.get("note"):
            item["note"] = reference["note"]
        published.append(item)
    return published


def _split_frontmatter(text: str) -> tuple[dict, str]:
    text = text.lstrip("\ufeff")
    if not text.startswith("---"):
        raise ValueError("design document must start with YAML frontmatter")
    rest = text[3:]
    if rest.startswith("\n"):
        rest = rest[1:]
    marker = rest.find("\n---")
    if marker < 0:
        raise ValueError("frontmatter opens with --- but has no closing ---")
    loaded = yaml.safe_load(rest[:marker])
    if not isinstance(loaded, dict):
        raise ValueError("design board frontmatter must be a YAML mapping")
    return loaded, rest[marker + 4 :].strip()


def _palette(value) -> dict[str, str]:
    colors = _string_map(value, "palette")
    if not colors:
        raise ValueError("palette must include at least one color")
    if len(colors) > 32:
        raise ValueError("palette has too many roles")
    normalized = {}
    for role, hex_value in colors.items():
        role = role.strip().lower()
        if not _ROLE.match(role):
            raise ValueError(f"palette role '{role}' must be a short lowercase name")
        if role in normalized:
            raise ValueError(f"palette role '{role}' is repeated")
        normalized[role] = normalize_hex(hex_value)
    return dict(sorted(normalized.items()))


def _typography(value) -> dict:
    if not isinstance(value, dict):
        raise ValueError("typography must be a mapping")
    data = dict(value)
    families = _string_map(data.pop("families", {}) or {}, "typography.families")
    if len(families) > 32:
        raise ValueError("too many font families")
    for role, family in families.items():
        _token(role, "font family")
        if not family or len(family) > 80 or any(ch in family for ch in '"\\`{}<>;$') or _control(family):
            raise ValueError(f"font family '{role}' contains unsupported characters")
    return {
        "families": dict(sorted(families.items())),
        "scale": _length_map(data.pop("scale", {}) or {}, "typography.scale"),
        "weights": _weights(data.pop("weights", {}) or {}),
        "extensions": _extensions(data),
    }


def _motion(value) -> dict:
    if not isinstance(value, dict):
        raise ValueError("motion must be a mapping")
    data = dict(value)
    feel = _text(data.pop("feel", ""), "motion.feel", 0, 80)
    if any(ch in feel for ch in '"\\;{}<>'):
        raise ValueError("motion.feel contains unsupported characters")
    duration = _string_map(data.pop("duration", {}) or {}, "motion.duration")
    easing = _string_map(data.pop("easing", {}) or {}, "motion.easing")
    if len(duration) > 32 or len(easing) > 32:
        raise ValueError("too many motion tokens")
    for name, item in duration.items():
        _token(name, "duration")
        if not _DURATION.match(item):
            raise ValueError(f"duration '{name}' must use ms or s")
    for name, item in easing.items():
        _token(name, "easing")
        if not _EASING.match(item) or "url" in item.lower():
            raise ValueError(f"easing '{name}' contains unsupported characters")
    return {
        "feel": feel,
        "duration": dict(sorted(duration.items())),
        "easing": dict(sorted(easing.items())),
        "extensions": _extensions(data),
    }


def _weights(value) -> dict[str, int]:
    weights = {}
    if not isinstance(value, dict):
        raise ValueError("typography.weights must be a mapping")
    if len(value) > 32:
        raise ValueError("too many font weights")
    for name, weight in value.items():
        _token(str(name), "font weight")
        if isinstance(weight, bool) or not isinstance(weight, (int, str)):
            raise ValueError(f"weight '{name}' must be an integer")
        number = int(weight)
        if not 1 <= number <= 1000:
            raise ValueError(f"weight '{name}' must be from 1 to 1000")
        weights[str(name)] = number
    return dict(sorted(weights.items()))


def _length_map(value, label: str) -> dict[str, str]:
    items = _string_map(value, label)
    if len(items) > 32:
        raise ValueError(f"{label} has too many steps")
    for name, size in items.items():
        _token(name, label)
        if not _LENGTH.match(size):
            raise ValueError(f"{label}.{name} must be 0 or a px, rem, or em value")
    return dict(sorted(items.items()))


def _shadows(value) -> dict[str, str]:
    items = _string_map(value, "shadows")
    if len(items) > 32:
        raise ValueError("too many shadows")
    for name, shadow in items.items():
        _token(name, "shadow")
        if shadow == "none":
            continue
        lowered = shadow.lower()
        if (
            len(shadow) > 200
            or any(ch in shadow for ch in ";{}<>\\")
            or _control(shadow)
            or "url(" in lowered
            or "expression" in lowered
            or "@import" in lowered
        ):
            raise ValueError(f"shadow '{name}' contains unsupported characters")
    return dict(sorted(items.items()))


def _components(value) -> dict[str, str]:
    items = _string_map(value, "components")
    if len(items) > 32:
        raise ValueError("too many component notes")
    for name, note in items.items():
        _token(name, "component")
        _line(note, "component note", 1, 300)
    return dict(sorted(items.items()))


def _references(value) -> list[dict]:
    if not isinstance(value, list):
        raise ValueError("references must be a list")
    if len(value) > 24:
        raise ValueError("too many references")
    references = []
    for item in value:
        if not isinstance(item, dict):
            raise ValueError("each reference must be a mapping")
        kind = str(item.get("kind", "")).strip()
        raw_value = str(item.get("value", "")).strip()
        note = _line(item.get("note", ""), "reference note", 0, 200)
        if kind == "path":
            path = Path(raw_value)
            if (
                not raw_value
                or len(raw_value) > 200
                or path.is_absolute()
                or "\\" in raw_value
                or ".." in path.parts
            ):
                raise ValueError(f"reference path '{raw_value}' must be relative and must not contain ..")
        elif kind == "url":
            if (
                not raw_value.startswith(("https://", "http://"))
                or len(raw_value) > 500
                or any(ch.isspace() or ord(ch) < 32 for ch in raw_value)
            ):
                raise ValueError("reference URL must be an http(s) URL")
        else:
            raise ValueError(f"reference kind '{kind}' must be path or url")
        references.append({"kind": kind, "value": raw_value, "note": note})
    return references


def _extensions(data: dict) -> dict:
    if len(data) > 24:
        raise ValueError("too many extension fields")
    cleaned = {}
    for key, value in data.items():
        key = str(key)
        if not key or len(key) > 64 or any(ch in key for ch in ":#{}[],") or _control(key):
            raise ValueError(f"cannot keep extension field '{key}'")
        rendered = yaml.safe_dump(value)
        if len(rendered) > 4_000:
            raise ValueError(f"extension '{key}' is too large")
        cleaned[key] = value
    return dict(sorted(cleaned.items()))


def _string_map(value, label: str) -> dict[str, str]:
    if value is None:
        return {}
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a mapping")
    mapped = {}
    for key, item in value.items():
        if isinstance(item, (dict, list)) or item is None:
            raise ValueError(f"{label}.{key} must be a scalar")
        mapped[str(key)] = str(item).strip()
    return mapped


def _token(name: str, label: str) -> None:
    if not _TOKEN.match(name):
        raise ValueError(f"invalid {label} name '{name}'")


def _text(value, label: str, minimum: int, maximum: int) -> str:
    if value is None:
        value = ""
    text = str(value)
    if not minimum <= len(text) <= maximum or _control(text, allow_newline=True):
        raise ValueError(f"{label} must be {minimum}-{maximum} characters")
    return text


def _line(value, label: str, minimum: int, maximum: int) -> str:
    text = _text(value, label, minimum, maximum)
    if any(ch in text for ch in "\n\r"):
        raise ValueError(f"{label} must be a single line")
    return text


def _timestamp(value, label: str, fallback: bool = False) -> str:
    if value in (None, ""):
        if fallback:
            return ""
        raise ValueError(f"{label} must be RFC 3339")
    text = str(value).strip()
    try:
        datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError(f"{label} must be RFC 3339") from exc
    return text


def _source_tool(value) -> str:
    text = str(value).strip()
    if not re.fullmatch(r"[A-Za-z0-9_-]{1,40}", text):
        raise ValueError(f"invalid source_tool '{text}'")
    return text


def _slugify(name: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    if not _SLUG.match(slug):
        raise ValueError(f"name '{name}' cannot be turned into a board slug")
    return slug


def _control(value: str, allow_newline: bool = False) -> bool:
    for char in value:
        if char == "\n":
            if not allow_newline:
                return True
            continue
        if char != "\t" and unicodedata.category(char) == "Cc":
            return True
    return False


def _flag(value) -> bool:
    if value is None or value is False:
        return False
    if value is True:
        return True
    if isinstance(value, str) and value.lower() in {"true", "false"}:
        return value.lower() == "true"
    raise ValueError("default must be true or false")
