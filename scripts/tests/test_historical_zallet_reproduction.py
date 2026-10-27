"""Fail-closed checks for the historical Zallet privacy-policy reproduction."""
import importlib.util
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "reproduce_zallet_677.py"
spec = importlib.util.spec_from_file_location("zallet_history", SOURCE)
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)


class HistoricalZalletTests(unittest.TestCase):
    def test_real_before_after_policy_difference(self):
        before = (
            "SHIELDSPEC_ORCHARD_TO_SAPLING_FULL_PRIVACY accepted=false\n"
            "SHIELDSPEC_IRONWOOD_TO_IRONWOOD_FULL_PRIVACY accepted=true\n"
            "SHIELDSPEC_IRONWOOD_TO_SAPLING_FULL_PRIVACY accepted=true\n"
            "Ironwood -> Sapling crosses a publicly visible value balance\n"
            "test result: FAILED. 2 passed; 1 failed\n"
        )
        after = (
            "SHIELDSPEC_ORCHARD_TO_SAPLING_FULL_PRIVACY accepted=false\n"
            "SHIELDSPEC_IRONWOOD_TO_IRONWOOD_FULL_PRIVACY accepted=true\n"
            "SHIELDSPEC_IRONWOOD_TO_SAPLING_FULL_PRIVACY accepted=false\n"
            "test result: ok. 3 passed; 0 failed\n"
        )
        self.assertEqual(
            history.classify_runs(101, before, 0, after),
            "historical_privacy_policy_difference_reproduced",
        )
        self.assertEqual(history.classify_runs(101, before, 101, after), "inconclusive")
        self.assertEqual(history.classify_runs(0, before, 0, after), "inconclusive")

    def test_compilation_error_is_not_a_privacy_finding(self):
        self.assertEqual(
            history.classify_runs(101, "error[E0277]: not compiled", 0, "test result: ok. 3 passed; 0 failed"),
            "inconclusive",
        )

    def test_source_injection_preserves_existing_production_code(self):
        original = "fn existing_policy() {}\n"
        expected = original + history.TEST_MODULE
        self.assertEqual(history.insert_test_module(original), expected)
        self.assertEqual(history.insert_test_module(expected), expected)
        self.assertRaises(ValueError, history.insert_test_module, "mod shieldspec_privacy_history;\n")


if __name__ == "__main__":
    unittest.main()
