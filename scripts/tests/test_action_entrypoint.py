"""Behavioral tests for the composite Action's command boundary."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
ENTRYPOINT = ROOT / "scripts" / "action_entrypoint.py"


class ActionEntrypointTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.home = Path(self.tmp.name)
        self.workspace = self.home / "wallet project"
        self.workspace.mkdir()
        self.outputs = self.home / "outputs"
        self.capture = self.home / "argv.json"
        self.fake_bin = self.home / "fake-tool"
        self.fake_bin.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, pathlib, sys\n"
            "a=sys.argv[1:]\n"
            "pathlib.Path(os.environ['ACTION_CAPTURE_PATH']).write_text(json.dumps(a))\n"
            "if '--save' in a and os.environ.get('FAKE_DISCOVERY') == 'yes':\n"
            " pathlib.Path(a[a.index('--save')+1]).write_text('{\"discovery\":true}\\n')\n"
            "print('{\"status\":\"controlled\"}')\n"
            "sys.exit(int(os.environ.get('FAKE_EXIT', '0')))\n"
        )
        self.fake_bin.chmod(0o755)
        self.env = {
            **os.environ,
            "SHIELDSPEC_ACTION_PATH": str(ROOT),
            "GITHUB_WORKSPACE": str(self.workspace),
            "RUNNER_TEMP": str(self.home),
            "GITHUB_OUTPUT": str(self.outputs),
            "SHIELDSPEC_FUZZ_BIN": str(self.fake_bin),
            "SHIELDSPEC_SNAPSHOT_BIN": str(self.fake_bin),
            "SHIELDSPEC_BUILTIN_RUNNER_BIN": str(self.fake_bin),
            "ACTION_CAPTURE_PATH": str(self.capture),
            "SHIELDSPEC_MODE": "differential",
            "SHIELDSPEC_PRESET": "zcash-sdk",
            "SHIELDSPEC_CASES": "2",
            "SHIELDSPEC_SEED": "148",
            "SHIELDSPEC_SCENARIO_FAMILY": "legacy",
            "SHIELDSPEC_POLICY": "standard",
            "SHIELDSPEC_FORMAT": "json",
        }

    def run_action(self, **updates):
        env = {**self.env, **updates}
        result = subprocess.run(
            [sys.executable, str(ENTRYPOINT)],
            env=env, capture_output=True, text=True, check=False,
        )
        outputs = {}
        if self.outputs.exists():
            for line in self.outputs.read_text().splitlines():
                key, value = line.split("=", 1)
                outputs[key] = value
        return result, outputs

    def test_builtin_preset_runs_first_party_negative_control_and_saves_report(self):
        result, outputs = self.run_action()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["status"], "0")
        argv = json.loads(self.capture.read_text())
        self.assertEqual(argv[:2], ["fuzz", "--baseline-runner"])
        self.assertEqual(argv[2], str(self.fake_bin))
        self.assertEqual(argv[4], str(self.fake_bin))
        self.assertEqual(argv[argv.index("--cases") + 1], "2")
        self.assertTrue(Path(outputs["report_path"]).is_file())
        self.assertIn("negative control", result.stderr.lower())

    def test_custom_runner_arguments_preserve_spaces_without_shell_evaluation(self):
        base = self.workspace / "base runner"
        candidate = self.workspace / "candidate runner"
        base.write_text("#!/bin/sh\nexit 0\n")
        candidate.write_text("#!/bin/sh\nexit 0\n")
        base.chmod(0o755)
        candidate.chmod(0o755)
        marker = self.home / "should-not-exist"
        argument = f"--label=$(touch {marker})"
        result, outputs = self.run_action(
            SHIELDSPEC_PRESET="custom",
            SHIELDSPEC_BASELINE_RUNNER=base.name,
            SHIELDSPEC_CANDIDATE_RUNNER=candidate.name,
            SHIELDSPEC_BASELINE_ARGS=f"--wallet-policy\norchard-only\n{argument}",
            SHIELDSPEC_CANDIDATE_ARGS="--wallet-policy\norchard-only",
            FAKE_EXIT="2", FAKE_DISCOVERY="yes",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["status"], "2")
        self.assertFalse(marker.exists())
        argv = json.loads(self.capture.read_text())
        self.assertIn(argument, argv)
        self.assertIn(str(base), argv)
        self.assertTrue(Path(outputs["report_path"]).is_file())
        self.assertTrue(Path(outputs["discovery_path"]).is_file())

    def test_missing_custom_runner_fails_closed(self):
        _, outputs = self.run_action(SHIELDSPEC_PRESET="custom")
        self.assertEqual(outputs["status"], "1")
        self.assertFalse(self.capture.exists())

    def test_preset_rejects_ignored_runner_override(self):
        _, outputs = self.run_action(SHIELDSPEC_BASELINE_RUNNER="ignored")
        self.assertEqual(outputs["status"], "1")
        self.assertFalse(self.capture.exists())

    def test_bad_case_count_and_scenario_fail_before_execution(self):
        for bad in ("0", "10001", "1;touch /tmp/no", "not-a-number"):
            with self.subTest(cases=bad):
                self.outputs.unlink(missing_ok=True)
                _, outputs = self.run_action(SHIELDSPEC_CASES=bad)
                self.assertEqual(outputs["status"], "1")
                self.assertFalse(self.capture.exists())
        _, outputs = self.run_action(SHIELDSPEC_SCENARIO_FAMILY="unknown")
        self.assertEqual(outputs["status"], "1")

    def test_snapshot_mode_retains_check_and_diff_commands(self):
        candidate = self.workspace / "candidate.json"
        candidate.write_text("{}")
        result, outputs = self.run_action(
            SHIELDSPEC_MODE="snapshot", SHIELDSPEC_CANDIDATE=candidate.name,
        )
        self.assertEqual(outputs["status"], "0", result.stderr)
        self.assertEqual(json.loads(self.capture.read_text())[:3],
                         ["check", "--input", str(candidate)])
        baseline = self.workspace / "baseline.json"
        baseline.write_text("{}")
        self.outputs.unlink()
        _, outputs = self.run_action(
            SHIELDSPEC_MODE="snapshot", SHIELDSPEC_CANDIDATE=candidate.name,
            SHIELDSPEC_BASELINE=baseline.name,
        )
        self.assertEqual(outputs["status"], "0")
        self.assertEqual(json.loads(self.capture.read_text())[0], "diff")

    def test_snapshot_without_candidate_is_operational_error(self):
        _, outputs = self.run_action(SHIELDSPEC_MODE="snapshot")
        self.assertEqual(outputs["status"], "1")
        self.assertFalse(self.capture.exists())


if __name__ == "__main__":
    unittest.main()
