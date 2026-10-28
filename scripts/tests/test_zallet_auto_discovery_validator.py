"""Fail-closed validation of ShieldSpec's actual historical Zallet campaign."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

MODULE = Path(__file__).resolve().parents[1] / "verify_zallet_auto_discovery.py"
spec = importlib.util.spec_from_file_location("zallet_auto_verify", MODULE)
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


def observed(decision):
    return {"policy": "zallet_full_privacy_v1", "decision": decision}


def report(before, after, difference=None):
    return {
        "baseline_wallet_policy": observed(before),
        "candidate_wallet_policy": observed(after),
        "wallet_policy_difference": difference,
    }


class ZalletDiscoveryValidatorTests(unittest.TestCase):
    def test_real_policy_controls_require_both_named_observations(self):
        verify.policy_pair(report("accepted", "accepted"), "accepted", "accepted")
        for bad in (
            report("accepted", "rejected"),
            {"candidate_wallet_policy": observed("accepted")},
            {"baseline_wallet_policy": observed("accepted")},
            report("accepted", "accepted") | {
                "baseline_wallet_policy": {"policy": "unrelated", "decision": "accepted"}
            },
        ):
            with self.subTest(bad=bad), self.assertRaises(AssertionError):
                verify.policy_pair(bad, "accepted", "accepted")

    def test_runner_failure_is_not_accepted_as_a_real_policy_decision(self):
        bad = subprocess.CompletedProcess(["runner"], 1, stdout='{"status":"observed"}', stderr="failed")
        with tempfile.TemporaryDirectory() as tmp, patch.object(verify.subprocess, "run", return_value=bad):
            with self.assertRaisesRegex(RuntimeError, "unexpected exit"):
                verify.invoke(["runner"], 0, Path(tmp) / "result.json")

    def test_runner_timeout_is_operational_failure(self):
        expired = subprocess.TimeoutExpired(["runner"], timeout=300)
        with tempfile.TemporaryDirectory() as tmp, \
                patch.object(verify.subprocess, "run", side_effect=expired):
            with self.assertRaisesRegex(RuntimeError, "timed out"):
                verify.invoke(["runner"], 2, Path(tmp) / "result.json")

    def test_malformed_json_is_not_accepted_as_discovery(self):
        bad = subprocess.CompletedProcess(["runner"], 2, stdout="not json", stderr="")
        with tempfile.TemporaryDirectory() as tmp, patch.object(verify.subprocess, "run", return_value=bad):
            with self.assertRaisesRegex(RuntimeError, "non-JSON"):
                verify.invoke(["runner"], 2, Path(tmp) / "result.json")

    def test_wrong_zallet_revision_fails_before_any_policy_execution(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": 1,
                "runners": {
                    "before": {"commit": "wrong"},
                    "after": {"commit": verify.EXPECTED_REVISIONS["after"]},
                },
            }))
            with patch.object(verify, "invoke") as invoke:
                with self.assertRaisesRegex(AssertionError, "pinned Zallet before"):
                    verify.execute(Path("fuzzer"), manifest, Path(tmp) / "out", 2026)
                invoke.assert_not_called()

    def test_real_controls_discovery_and_replay_are_all_mandatory(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": 1,
                "runners": {
                    side: {"commit": commit}
                    for side, commit in verify.EXPECTED_REVISIONS.items()
                },
            }))
            discovery = Path(tmp) / "out" / "minimized-discovery.json"
            observations = [
                {"status": "no_regression", "report": report("accepted", "accepted")},
                {"status": "no_regression", "report": report("rejected", "rejected")},
                {
                    "status": "wallet_policy_difference",
                    "ordinal": 2,
                    "cases_executed": 3,
                    "comparable_wallet_policy_cases": 3,
                    "report": report("rejected", "accepted", verify.EXPECTED_DIFFERENCE),
                },
                {
                    "status": "wallet_policy_difference_reproduced",
                    "recorded_rule_reproduced": True,
                    "provenance_verified": True,
                    "report": report("rejected", "accepted", verify.EXPECTED_DIFFERENCE),
                },
            ]

            def simulate(command, expected_exit, output):
                if "fuzz" in command:
                    discovery.write_text('{"saved":"real"}')
                return observations.pop(0)

            with patch.object(verify, "invoke", side_effect=simulate) as invoke, \
                    patch("builtins.print"):
                verify.execute(Path("fuzzer"), manifest, Path(tmp) / "out", 2026)
                self.assertEqual(invoke.call_count, 4)
            self.assertEqual(observations, [])


if __name__ == "__main__":
    unittest.main()
