"""Pinned launcher tests: executable identity must include manifest and adapter."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "make_zallet_immutable_launchers.py"
spec = importlib.util.spec_from_file_location("zallet_launchers", SOURCE)
launchers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launchers)


class ImmutableLauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.adapter = self.root / "adapter.py"
        self.adapter.write_text(
            "import json, sys\n"
            "print(json.dumps(sys.argv[1:]))\n",
            encoding="utf-8",
        )
        self.manifest = self.root / "manifest.json"
        self.manifest.write_text(json.dumps({
            "schema_version": 1,
            "runners": {"before": {"sha256": "a" * 64}, "after": {"sha256": "b" * 64}},
        }), encoding="utf-8")

    def test_launcher_is_executable_and_runs_only_pinned_adapter(self):
        outputs = launchers.make_launchers(self.manifest, self.adapter, self.root / "launchers")
        self.assertEqual(set(outputs), {"before", "after"})
        for side in ("before", "after"):
            runner = outputs[side]
            self.assertTrue(runner.is_file())
            self.assertTrue(runner.stat().st_mode & 0o111)
            run = subprocess.run([str(runner)], capture_output=True, text=True, check=False)
            self.assertEqual(run.returncode, 0, run.stderr)
            args = json.loads(run.stdout)
            self.assertEqual(args, ["--manifest", str(self.manifest), "--revision", side])
            self.assertIn("EXPECTED_MANIFEST_SHA", runner.read_text())
            self.assertIn("EXPECTED_ADAPTER_SHA", runner.read_text())

    def test_tampered_manifest_fails_closed(self):
        outputs = launchers.make_launchers(self.manifest, self.adapter, self.root / "launchers")
        self.manifest.write_text(self.manifest.read_text() + "\n")
        result = subprocess.run([str(outputs["after"])], capture_output=True, text=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("manifest SHA", result.stderr)

    def test_tampered_adapter_fails_closed(self):
        outputs = launchers.make_launchers(self.manifest, self.adapter, self.root / "launchers")
        self.adapter.write_text(self.adapter.read_text() + "# altered\n")
        result = subprocess.run([str(outputs["before"])], capture_output=True, text=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("adapter SHA", result.stderr)


if __name__ == "__main__":
    unittest.main()
