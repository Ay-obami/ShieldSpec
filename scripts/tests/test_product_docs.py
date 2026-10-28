from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ProductDocumentationTests(unittest.TestCase):
    def test_main_readme_links_into_permanent_product_docs_and_site(self):
        text = (ROOT / "README.md").read_text()
        self.assertIn("docs/getting-started.md", text)
        self.assertIn("docs/roadmap.md", text)
        self.assertIn("site/README.md", text)

    def test_getting_started_explains_real_demo_and_failure_codes(self):
        text = (ROOT / "docs/getting-started.md").read_text()
        self.assertIn("cargo run --locked", text)
        self.assertIn("shieldspec-fuzz", text)
        self.assertIn("replay", text)
        self.assertIn("exit code 2", text.lower())
        self.assertIn("intentionally injected", text.lower())
        self.assertIn("GitHub Action", text)

    def test_roadmap_documents_verified_historical_auto_discovery_and_remaining_limits(self):
        text = (ROOT / "docs/roadmap.md").read_text()
        self.assertIn("Available", text)
        self.assertIn("Completed sprint issues and current integration gaps", text)
        self.assertIn("release-readiness.md", text)
        self.assertIn("Planned", text)
        self.assertIn("zallet-auto-discovery.md", text)
        self.assertIn("independently compiled Zallet revisions", text)
        self.assertIn("zallet#677", text)


if __name__ == "__main__":
    unittest.main()
