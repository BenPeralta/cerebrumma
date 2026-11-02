import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path

from cerebrumma_mcp.server import resolve_layer, select_business_entries


class RetrievalTests(unittest.TestCase):
    def test_layer_allowlist_blocks_traversal(self):
        with tempfile.TemporaryDirectory() as tmp:
            brain = Path(tmp)
            with self.assertRaises(ValueError):
                resolve_layer(brain, "../secrets")
            with self.assertRaises(ValueError):
                resolve_layer(brain, "episodic/../../.ssh")
            path = resolve_layer(brain, "procedural/skills")
            self.assertEqual(path, (brain / "memory" / "procedural" / "skills").resolve())

    def test_business_context_skips_unrelated_entries(self):
        now = datetime(2026, 11, 2, tzinfo=timezone.utc)
        entries = [
            {
                "id": "refunds",
                "scope": ["support"],
                "customer_safe": True,
                "authority": "company_policy",
                "content": "Refunds are allowed within 30 days.",
            },
            {
                "id": "pricing",
                "scope": ["support"],
                "customer_safe": True,
                "authority": "product_truth",
                "content": "The enterprise plan costs 99 dollars.",
            },
        ]
        selected = select_business_entries(
            entries, "how do refunds work", "support", True, 8, now
        )
        self.assertEqual([entry["id"] for entry in selected], ["refunds"])

    def test_empty_query_returns_nothing(self):
        now = datetime(2026, 11, 2, tzinfo=timezone.utc)
        entries = [
            {
                "id": "refunds",
                "scope": ["support"],
                "customer_safe": True,
                "content": "Refunds are allowed within 30 days.",
            }
        ]
        self.assertEqual(
            select_business_entries(entries, "a", "support", True, 8, now),
            [],
        )


if __name__ == "__main__":
    unittest.main()
