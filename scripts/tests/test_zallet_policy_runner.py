"""The historical Zallet adapter must fail closed on unverifiable observations."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import zallet_policy_runner as adapter
import build_zallet_policy_runners as builder


VALID = {
    "kind": "zallet_privacy_probe",
    "params": {"input_pool": "ironwood", "change_pool": "sapling",
               "input_zat": 20000, "fee_zat": 10000},
}


def marker(accepted: bool = False, **changes):
    observed = {"step_count": 1, "input_count": 1, "change_count": 1,
                "recipient_count": 0, "input_pool": "ironwood",
                "change_pool": "sapling", "input_zat": 20000,
                "change_zat": 10000, "fee_zat": 10000}
    observed.update(changes)
    return "test zallet::shieldspec_policy_probe_dynamic ... " + adapter.MARKER + json.dumps(
        {"schema_version": 1, "observed_proposal": observed,
         "policy": "zallet_full_privacy_v1", "accepted": accepted}
    ) + "\n"


class ZalletRunnerTest(unittest.TestCase):
    def test_valid_case_becomes_balanced(self):
        self.assertEqual(adapter.validate_case(VALID)["change_zat"], 10000)

    def test_rejects_unsupported_pool_and_unbalanced_values(self):
        for params in (
            {"input_pool": "sapling", "change_pool": "orchard", "input_zat": 20000, "fee_zat": 10000},
            {"input_pool": "ironwood", "change_pool": "transparent", "input_zat": 20000, "fee_zat": 10000},
            {"input_pool": "ironwood", "change_pool": "sapling", "input_zat": 10000, "fee_zat": 10000},
            {"input_pool": "ironwood", "change_pool": "sapling", "input_zat": -1, "fee_zat": 10000},
            {"input_pool": "ironwood", "change_pool": "sapling", "input_zat": True, "fee_zat": 10000},
        ):
            with self.subTest(params=params), self.assertRaises(ValueError):
                adapter.validate_case({"kind": "zallet_privacy_probe", "params": params})

    def test_result_requires_one_machine_marker(self):
        with self.assertRaises(ValueError):
            adapter.parse_probe_output("test result: ok\n", VALID)
        with self.assertRaises(ValueError):
            adapter.parse_probe_output(marker() + marker(), VALID)
        self.assertEqual(adapter.parse_probe_output(marker(True), VALID)["accepted"], True)

    def test_observed_proposal_must_equal_submitted_case(self):
        for change in ({"input_pool": "orchard"}, {"change_zat": 9999},
                       {"fee_zat": 1}, {"recipient_count": 1}, {"step_count": 2}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                adapter.parse_probe_output(marker(False, **change), VALID)

    def test_emits_complete_wallet_policy_snapshot(self):
        observed = adapter.parse_probe_output(marker(False), VALID)
        protocol = adapter.protocol_observation(observed)
        self.assertEqual(protocol["status"], "observed")
        self.assertEqual(protocol["protocol_version"], 1)
        self.assertEqual(protocol["wallet_policy"], {
            "policy": "zallet_full_privacy_v1", "decision": "rejected"
        })
        step, = protocol["snapshot"]["steps"]
        self.assertEqual(step["fee_zat"], 10000)
        self.assertEqual(step["inputs"], [{"pool": "ironwood", "value_zat": 20000}])
        self.assertEqual(step["outputs"], [{"pool": "sapling", "role": "change", "value_zat": 10000}])

    def test_manifest_hash_mismatch_must_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "binary"
            exe.write_bytes(b"test-executable")
            manifest = {"schema_version": 1, "fixture_sha256": adapter.sha256_file(adapter.FIXTURE),
                        "runners": {"before": {"commit": builder.BEFORE,
                                               "executable": str(exe), "sha256": "1" * 64}}}
            with self.assertRaises(ValueError):
                adapter.verify_runner(manifest, "before")

    def test_builder_injection_is_test_only_and_idempotent(self):
        source = "fn enforce_privacy_policy() {}\n"
        self.assertEqual(builder.insert_test_module(
            builder.insert_test_module(source)), builder.insert_test_module(source))
        self.assertIn(source.strip(), builder.insert_test_module(source))
        with self.assertRaises(ValueError):
            builder.insert_test_module("mod shieldspec_dynamic_policy_probe;\n")


if __name__ == "__main__":
    unittest.main()
