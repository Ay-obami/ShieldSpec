#!/usr/bin/env python3
"""Compare actual pre/post librustzcash SDK behavior for issue #2614.

This script backports ONLY the upstream regression test into the affected
revision; it does not transplant the fixing production changes. Both trees
then execute identical wallet fixture logic against their own SDK code.

The observed distinction is correctness (transparent send-max), NOT an
observed privacy vulnerability. See evidence/historical-2614/README.md.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


UPSTREAM = "https://github.com/zcash/librustzcash.git"
BEFORE = "54ff673169ec8be5a4b023a7ae69bcf5caead2a3"
AFTER = "25766a4c754a847b0a9e3a63801d8f46a270c01b"
ISSUE = "https://github.com/zcash/librustzcash/issues/2614"
FIX = "https://github.com/zcash/librustzcash/pull/2615"
BACKEND = "zcash_client_backend/src/data_api/testing/pool.rs"
SQLITE = "zcash_client_sqlite/src/testing/pool.rs"
SAPLING = "zcash_client_sqlite/src/wallet/sapling.rs"
TEST_NAME = "send_max_spendable_to_transparent"
UPSTREAM_TEST_START = (
    "/// Tests that sending all the spendable funds within the given shielded pool to a"
)
UPSTREAM_TEST_END = (
    "/// Tests that a send-max proposal whose total required fee overflows"
)
SAPLING_ANCHOR = (
    '    #[test]\n    #[cfg(feature = "transparent-inputs")]\n'
    "    fn fails_to_send_max_to_transparent_with_memo"
)
SQLITE_WRAPPER = '''
#[cfg(feature = "transparent-inputs")]
pub(crate) fn send_max_spendable_to_transparent<T: ShieldedPoolTester>() {
    zcash_client_backend::data_api::testing::pool::send_max_spendable_to_transparent::<T>(
        TestDbFactory::default(),
        BlockCache::new(),
    )
}
'''
SAPLING_WRAPPER = '''    #[test]
    #[cfg(feature = "transparent-inputs")]
    fn send_max_spendable_to_transparent() {
        testing::pool::send_max_spendable_to_transparent::<SaplingPoolTester>()
    }

'''


def extract_upstream_test(source: str) -> str:
    """Extract an *unaltered* upstream test from the post-fix commit."""
    start = source.index(UPSTREAM_TEST_START)
    end = source.index(UPSTREAM_TEST_END, start)
    extracted = source[start:end].rstrip() + "\n"
    if "pub fn send_max_spendable_to_transparent" not in extracted:
        raise ValueError("pinned upstream test is missing")
    return extracted


def patch_affected_source(
    backend: str, sqlite: str, sapling: str, test_source: str
) -> tuple[str, str, str]:
    """Copy the upstream test and minimal test registrations, never its fix."""
    if "pub fn send_max_spendable_to_transparent" not in backend:
        backend = backend.rstrip() + "\n\n" + test_source
    if "fn send_max_spendable_to_transparent" not in sqlite:
        sqlite = sqlite.rstrip() + "\n" + SQLITE_WRAPPER
    if "fn send_max_spendable_to_transparent" not in sapling:
        if SAPLING_ANCHOR not in sapling:
            raise ValueError("Sapling test registration anchor changed upstream")
        sapling = sapling.replace(SAPLING_ANCHOR, SAPLING_WRAPPER + SAPLING_ANCHOR, 1)
    return backend, sqlite, sapling


def classify_test_runs(
    before_exit: int, before_log: str, after_exit: int, after_log: str
) -> str:
    """Recognize only the documented, test-asserted SDK difference."""
    expected_failure = (
        before_exit != 0
        and "PaymentPoolsMismatch" in before_log
        and "panicked" in before_log
        and "test result: FAILED" in before_log
    )
    expected_success = after_exit == 0 and re.search(
        r"test result: ok\. 1 passed; 0 failed", after_log
    )
    if expected_failure and expected_success:
        return "historical_difference_reproduced"
    return "inconclusive"


def run(command: list[str], *, cwd: Path | None = None, timeout: int = 2400) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, text=True, capture_output=True, timeout=timeout, check=False)


def git(repo: Path, *args: str) -> str:
    result = run(["git", "-C", str(repo), *args])
    if result.returncode:
        raise RuntimeError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def verify_checkout(path: Path, expected: str) -> None:
    actual = git(path, "rev-parse", "HEAD")
    if actual != expected:
        raise ValueError(f"wrong SDK commit at {path}: expected {expected}, found {actual}")
    if git(path, "status", "--porcelain"):
        raise ValueError(f"SDK checkout is dirty before reproduction: {path}")


def run_reproduction(upstream_repo: Path, output: Path, timeout: int) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="shieldspec-sdk-2614-") as workspace:
        base = Path(workspace)
        repo = upstream_repo
        if not repo:
            repo = base / "librustzcash"
            clone = run(["git", "clone", "--filter=blob:none", "--no-checkout", UPSTREAM, str(repo)], timeout=timeout)
            if clone.returncode:
                raise RuntimeError(f"could not clone SDK history: {clone.stderr.strip()}")

        before = base / "before"
        after = base / "after"
        git(repo, "worktree", "add", "--detach", str(before), BEFORE)
        git(repo, "worktree", "add", "--detach", str(after), AFTER)
        try:
            verify_checkout(before, BEFORE)
            verify_checkout(after, AFTER)

            test_source = extract_upstream_test((after / BACKEND).read_text())
            files = [before / BACKEND, before / SQLITE, before / SAPLING]
            original = [file.read_text() for file in files]
            modified = patch_affected_source(*original, test_source)
            for file, contents in zip(files, modified, strict=True):
                file.write_text(contents)

            command = [
                "cargo", "test", "--locked", "-p", "zcash_client_sqlite",
                "--features", "transparent-inputs", "--lib", TEST_NAME, "--", "--nocapture",
            ]
            results = {}
            for label, checkout in [("before", before), ("after", after)]:
                completed = run(command, cwd=checkout, timeout=timeout)
                log = completed.stdout + "\n" + completed.stderr
                (output / f"{label}.log").write_text(log)
                results[label] = {
                    "sdk_git_commit": BEFORE if label == "before" else AFTER,
                    "cargo_exit_code": completed.returncode,
                    "output_log": f"{label}.log",
                    "matched_test": "test result:" in log,
                }

            before_log = (output / "before.log").read_text()
            after_log = (output / "after.log").read_text()
            status = classify_test_runs(
                results["before"]["cargo_exit_code"], before_log,
                results["after"]["cargo_exit_code"], after_log,
            )

            rustc = run(["rustc", "--version"])
            cargo = run(["cargo", "--version"])
            evidence = {
                "schema_version": 1,
                "classification": "historical_sdk_correctness_difference_not_privacy_regression",
                "status": status,
                "documented_issue": ISSUE,
                "upstream_fix": FIX,
                "test_name": TEST_NAME,
                "test_source_sha256": hashlib.sha256(test_source.encode()).hexdigest(),
                "test_method": "identical upstream post-fix test; test-only backport to affected checkout",
                "test_scenario": {
                    "funding_pool": "sapling", "notes": 1, "funding_zat": 60000,
                    "recipient": "plain_transparent_p2pkh", "mode": "send_max_everything",
                    "fee_rule": "zip317", "expected_before": "PaymentPoolsMismatch",
                    "expected_after": "valid_single_step_transparent_payment",
                },
                "command": command,
                "rustc": rustc.stdout.strip(),
                "cargo": cargo.stdout.strip(),
                "utc_time": dt.datetime.now(dt.timezone.utc).isoformat(),
                "sdk": results,
                "privacy_vulnerability_discovered": False,
            }
            (output / "verdict.json").write_text(json.dumps(evidence, indent=2) + "\n")
            return evidence
        finally:
            # Both worktrees were created by this process in its private tempdir.
            for checkout in (before, after):
                if checkout.exists():
                    git(repo, "worktree", "remove", "--force", str(checkout))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk-repo", type=Path, help="existing local librustzcash clone (saves download)")
    parser.add_argument("--output", type=Path, default=Path("evidence/historical-2614/results"))
    parser.add_argument("--timeout", type=int, default=2400, help="seconds allowed per SDK test")
    args = parser.parse_args()
    evidence = run_reproduction(args.sdk_repo, args.output, args.timeout)
    print(json.dumps({"status": evidence["status"], "report": str(args.output / "verdict.json")}, indent=2))
    return 0 if evidence["status"] == "historical_difference_reproduced" else 1


if __name__ == "__main__":
    raise SystemExit(main())
