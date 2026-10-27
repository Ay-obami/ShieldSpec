"""Fail-closed checks for the opt-in Ironwood CI control (synthetic JSON fixtures)."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECKER = Path(__file__).with_name("ci_campaign_check.py")
SEED = 1593835520
RULE = "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"
POLICY = "zcash_spend_orchard_only_v1"


def campaign():
    return {
        "status": "no_regression_found", "scenario_family": "ironwood",
        "seed": SEED, "cases_executed": 4,
        "baseline_wallet_policy_observations": 4,
        "candidate_wallet_policy_observations": 4,
        "comparable_wallet_policy_cases": 4,
        "report": None, "ordinal": None,
    }


def replay(ordinal):
    pool = "orchard" if ordinal < 2 else "sapling"
    activation = "at" if ordinal % 2 == 0 else "before"
    destination = "ironwood" if activation == "at" else "orchard"
    findings = ([{"rule_id": RULE, "evidence": {
        "source_pool": pool, "destination_pool": destination
    }}] if activation == "at" or pool == "sapling" else [])
    verdict = {"policy": POLICY, "decision": "accepted" if pool == "orchard" else "rejected"}
    return {
        "status": "no_regression",
        "scenario": {"kind": "nu63_orchard_payment", "params": {
            "activation": activation, "payment": {"funding_pool": pool, "change_pool": pool}
        }},
        "report": {
            "baseline": {"findings": findings},
            "candidate": {"findings": findings},
            "regression": {"passes": True, "findings": []},
            "baseline_wallet_policy": verdict,
            "candidate_wallet_policy": verdict,
        },
        "reproduction": (
            f"replay --seed {SEED} --ordinal {ordinal} --scenario-family ironwood "
            "--baseline-arg --wallet-spend-policy --baseline-arg orchard-only "
            "--candidate-arg --wallet-spend-policy --candidate-arg orchard-only"
        ),
    }


class CampaignCheckerTests(unittest.TestCase):
    def check(self, mode, document, *, ordinal=None):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            command = [sys.executable, str(CHECKER), mode, str(path), "--seed", str(SEED)]
            if mode == "campaign":
                command.extend(["--cases", "4"])
            else:
                command.extend(["--ordinal", str(ordinal)])
            return subprocess.run(command, capture_output=True, text=True, check=False)

    def test_complete_observed_campaign_passes(self):
        result = self.check("campaign", campaign())
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_snapshot_only_or_partial_observations_fail(self):
        document = campaign()
        document["comparable_wallet_policy_cases"] = 0
        self.assertNotEqual(self.check("campaign", document).returncode, 0)

    def test_regression_status_fails_even_with_full_counters(self):
        document = campaign()
        document["status"] = "privacy_regression"
        self.assertNotEqual(self.check("campaign", document).returncode, 0)

    def test_each_replay_proves_pool_activation_and_sdk_decision(self):
        for ordinal in range(4):
            with self.subTest(ordinal=ordinal):
                result = self.check("replay", replay(ordinal), ordinal=ordinal)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_missing_wallet_verdict_fails_instead_of_claiming_coverage(self):
        document = replay(2)
        del document["report"]["candidate_wallet_policy"]
        self.assertNotEqual(self.check("replay", document, ordinal=2).returncode, 0)

    def test_wrong_policy_label_or_decision_fails(self):
        document = replay(2)
        document["report"]["candidate_wallet_policy"]["decision"] = "accepted"
        self.assertNotEqual(self.check("replay", document, ordinal=2).returncode, 0)
        document = replay(0)
        document["report"]["baseline_wallet_policy"]["policy"] = "FullPrivacy"
        self.assertNotEqual(self.check("replay", document, ordinal=0).returncode, 0)

    def test_wrong_activation_or_ironwood_crossing_fails(self):
        document = replay(0)
        document["scenario"]["params"]["activation"] = "before"
        self.assertNotEqual(self.check("replay", document, ordinal=0).returncode, 0)
        document = replay(0)
        document["report"]["baseline"]["findings"] = []
        document["report"]["candidate"]["findings"] = []
        self.assertNotEqual(self.check("replay", document, ordinal=0).returncode, 0)

    def test_unreproducible_or_nonclean_report_fails(self):
        document = replay(1)
        document["reproduction"] = "replay without a seed or runner flags"
        self.assertNotEqual(self.check("replay", document, ordinal=1).returncode, 0)
        document = replay(1)
        document["report"]["regression"]["passes"] = False
        self.assertNotEqual(self.check("replay", document, ordinal=1).returncode, 0)


if __name__ == "__main__":
    unittest.main()
