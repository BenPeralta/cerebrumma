import json
import tempfile
import unittest
from pathlib import Path

from cerebrumma_mcp.design_layer import (
    DESIGN_SCHEMA,
    index_designs,
    load_boards,
    normalize_hex,
    parse_board_text,
    resolve_design,
)
from cerebrumma_mcp.server import _WRITABLE_LAYERS, resolve_layer

ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = (ROOT / "templates" / "design-board.md").read_text()


def write_board(brain: Path, slug: str, text: str) -> None:
    directory = brain / "memory" / "design"
    directory.mkdir(parents=True, exist_ok=True)
    (directory / f"{slug}.md").write_text(text)


class DesignLayerTests(unittest.TestCase):
    def test_template_round_trip_fields(self):
        board = parse_board_text(TEMPLATE)
        self.assertEqual(board["schema"], DESIGN_SCHEMA)
        self.assertEqual(board["slug"], "warm-editorial")
        self.assertEqual(board["palette"]["accent"], "#C2410C")
        self.assertEqual(board["palette"]["background"], "#F6F1E8")
        self.assertEqual(board["typography"]["families"]["body"], "Source Serif 4")
        self.assertEqual(board["typography"]["weights"]["bold"], 700)
        self.assertEqual(board["spacing"]["4"], "1rem")
        self.assertEqual(board["extensions"]["mockitup_id"], "mb_warm")
        self.assertTrue(board["guidance"].startswith("Use the tokens above"))

    def test_hex_and_parent_paths(self):
        self.assertEqual(normalize_hex("#c24"), "#CC2244")
        broken = TEMPLATE.replace(
            'value: "references/warm-editorial/hero.png"',
            'value: "../../.ssh/id_rsa"',
        )
        with self.assertRaises(ValueError):
            parse_board_text(broken)

    def test_local_board_replaces_global_and_default_is_local(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            local = root / "local"
            remote = root / "global"
            write_board(remote, "warm-editorial", TEMPLATE)
            local_text = TEMPLATE.replace('default: true', "default: false").replace(
                'accent: "#C2410C"',
                'accent: "#00AA00"',
            )
            write_board(local, "warm-editorial", local_text)
            night = TEMPLATE.replace("warm-editorial", "night-studio").replace(
                "Warm Editorial", "Night Studio"
            )
            write_board(local, "night-studio", night)
            boards = load_boards([("local", local), ("global", remote)])
            warm = next(board for board in boards if board["slug"] == "warm-editorial")
            self.assertEqual(warm["brain"], "local")
            self.assertEqual(warm["palette"]["accent"], "#00AA00")
            self.assertEqual(index_designs(boards)["effective_default"], "night-studio")

    def test_mix_palette_and_type_and_default_lookup(self):
        with tempfile.TemporaryDirectory() as tmp:
            brain = Path(tmp)
            write_board(brain, "warm-editorial", TEMPLATE)
            night = (
                TEMPLATE.replace("warm-editorial", "night-studio")
                .replace("Warm Editorial", "Night Studio")
                .replace('default: true', "default: false")
                .replace('accent: "#C2410C"', 'accent: "#88CCFF"')
                .replace('display: "Fraunces"', 'display: "Geist"')
            )
            write_board(brain, "night-studio", night)
            boards = load_boards([("local", brain)])
            mixed = resolve_design(
                boards,
                "warm-editorial",
                palette="night-studio",
                typography="warm-editorial",
            )
            self.assertEqual(mixed["palette"]["accent"], "#88CCFF")
            self.assertEqual(mixed["typography"]["families"]["display"], "Fraunces")
            self.assertEqual(mixed["spacing"]["4"], "1rem")
            self.assertEqual(mixed["mixed_from"]["palette"]["slug"], "night-studio")
            self.assertNotIn("typography", mixed["mixed_from"])
            self.assertIn("authoritative", mixed["agent_instructions"])
            self.assertEqual(resolve_design(boards)["slug"], "warm-editorial")

    def test_design_is_readable_but_not_a_freeform_write_layer(self):
        self.assertNotIn("design", _WRITABLE_LAYERS)
        with tempfile.TemporaryDirectory() as tmp:
            brain = Path(tmp)
            self.assertEqual(
                resolve_layer(brain, "design"),
                (brain / "memory" / "design").resolve(),
            )

    def test_tools_return_json(self):
        from cerebrumma_mcp import server

        with tempfile.TemporaryDirectory() as tmp:
            brain = Path(tmp)
            write_board(brain, "warm-editorial", TEMPLATE)
            original_paths = server.brain_paths
            original_local = server._LOCAL
            server.brain_paths = lambda: [brain]
            server._LOCAL = brain
            try:
                listed = json.loads(server.list_designs())
                shown = json.loads(server.get_design())
            finally:
                server.brain_paths = original_paths
                server._LOCAL = original_local
            self.assertEqual(listed["effective_default"], "warm-editorial")
            self.assertEqual(shown["schema"], DESIGN_SCHEMA)
            self.assertEqual(shown["palette"]["ink"], "#1C1917")


if __name__ == "__main__":
    unittest.main()
