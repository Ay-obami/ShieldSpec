"""Fast tests for the pinned historical SDK reproducer (no network required)."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from reproduce_librustzcash_2614 import (  # noqa: E402
    classify_test_runs,
    extract_upstream_test,
    patch_affected_source,
)


class HistoricalReproductionTests(unittest.TestCase):
    def test_extracts_only_upstream_test_without_production_fix(self):
        source = (
            "fn unrelated() {}\n"
            "/// Tests that sending all the spendable funds within the given shielded pool to a\n"
            "/// transparent (non-TEX) recipient succeeds.\n"
            "#[cfg(feature = \"transparent-inputs\")]\n"
            "pub fn send_max_spendable_to_transparent() { assert!(true); }\n"
            "/// Tests that a send-max proposal whose total required fee overflows\n"
            "fn another() {}\n"
        )
        extracted = extract_upstream_test(source)
        self.assertIn("send_max_spendable_to_transparent", extracted)
        self.assertNotIn("fn another", extracted)
        self.assertNotIn("fn unrelated", extracted)

    def test_backport_adds_test_only_and_is_idempotent(self):
        backend = "use something;\nfn old_function() {}\n"
        sqlite = "pub(crate) fn existing() {}\n"
        sapling = (
            "    #[test]\n    fn existing_test() {}\n"
            "    #[test]\n    #[cfg(feature = \"transparent-inputs\")]\n"
            "    fn fails_to_send_max_to_transparent_with_memo() {}\n"
        )
        test = "#[cfg(feature = \"transparent-inputs\")]\npub fn send_max_spendable_to_transparent() {}\n"
        patched = patch_affected_source(backend, sqlite, sapling, test)
        self.assertIn(test, patched[0])
        self.assertIn("fn send_max_spendable_to_transparent", patched[1])
        self.assertIn("fn send_max_spendable_to_transparent", patched[2])
        self.assertEqual(patched, patch_affected_source(*patched, test))
        self.assertIn("fn old_function() {}", patched[0])

    def test_specific_bug_before_and_one_passing_test_after(self):
        before = "thread '...' panicked: called Result::unwrap() on an Err value: PaymentPoolsMismatch\ntest result: FAILED"
        after = "test result: ok. 1 passed; 0 failed"
        self.assertEqual(classify_test_runs(101, before, 0, after), "historical_difference_reproduced")

    def test_compilation_failure_is_inconclusive_not_a_reproduction(self):
        self.assertEqual(
            classify_test_runs(101, "error: could not compile zcash_client_sqlite", 0, "1 passed"),
            "inconclusive",
        )
        self.assertEqual(
            classify_test_runs(101, "PaymentPoolsMismatch", 101, "error: compilation failed"),
            "inconclusive",
        )


if __name__ == "__main__":
    unittest.main()
