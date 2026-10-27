#!/usr/bin/env python3
"""Reproduce the documented zallet #677 Ironwood privacy-policy bypass.

Uses identical, test-only Rust code in pinned before/after zallet-core trees.
The witness invokes the real enforce_privacy_policy on a real Proposal.
No upstream production patch or injected privacy fault is applied.
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

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = "https://github.com/zcash/zallet.git"
ISSUE = "https://github.com/zcash/zallet/pull/677"
BEFORE = "f496c9b6b2c6ebb0065b329c11bda4a5fdf73568"
AFTER = "e966780216b7a86b0a1a6618f2fde5b574da7171"
POLICY_SOURCE = "zallet-core/src/components/json_rpc/payments.rs"
TEST_FIXTURE = ROOT / "evidence/historical-zallet-677/shieldspec_privacy_history.rs"
TEST_MODULE = (
    '\n#[cfg(test)]\n'
    '#[path = "shieldspec_privacy_history.rs"]\n'
    'mod shieldspec_privacy_history;\n'
)
COMMAND = ["cargo", "test", "--locked", "-p", "zallet-core", "--lib", "shieldspec_", "--", "--nocapture"]
MARKER = "SHIELDSPEC_IRONWOOD_TO_SAPLING_FULL_PRIVACY accepted="
CONTROL_CROSS = "SHIELDSPEC_ORCHARD_TO_SAPLING_FULL_PRIVACY accepted=false"
CONTROL_SAME = "SHIELDSPEC_IRONWOOD_TO_IRONWOOD_FULL_PRIVACY accepted=true"


def insert_test_module(source: str) -> str:
    """Append only a cfg(test) module registration; never patch policy code."""
    if source.endswith(TEST_MODULE):
        return source
    if "mod shieldspec_privacy_history;" in source:
        raise ValueError("unrecognized existing test registration")
    return source.rstrip("\n") + "\n" + TEST_MODULE


def classify_runs(before_exit: int, before_log: str, after_exit: int, after_log: str) -> str:
    controls_before = CONTROL_CROSS in before_log and CONTROL_SAME in before_log
    controls_after = CONTROL_CROSS in after_log and CONTROL_SAME in after_log
    old_bypass = (
        before_exit == 101
        and MARKER + "true" in before_log
        and "test result: FAILED. 2 passed; 1 failed" in before_log
        and "Ironwood -> Sapling crosses a publicly visible value balance" in before_log
    )
    new_guard = (
        after_exit == 0
        and MARKER + "false" in after_log
        and "test result: ok. 3 passed; 0 failed" in after_log
    )
    if controls_before and controls_after and old_bypass and new_guard:
        return "historical_privacy_policy_difference_reproduced"
    return "inconclusive"


def run(command: list[str], *, cwd: Path | None = None, env: dict | None = None, timeout: int = 3600):
    return subprocess.run(
        command, cwd=cwd, env=env, capture_output=True, text=True,
        timeout=timeout, check=False,
    )


def git(repo: Path, *args: str) -> str:
    completed = run(["git", "-C", str(repo), *args])
    if completed.returncode:
        raise RuntimeError(f"git {args} failed: {completed.stderr.strip()}")
    return completed.stdout.strip()


def prepare_checkout(path: Path, commit: str, fixture: bytes) -> None:
    actual = git(path, "rev-parse", "HEAD")
    if actual != commit:
        raise ValueError(f"wrong Zallet revision in {path}: expected {commit}, got {actual}")
    policy_path = path / POLICY_SOURCE
    original = git(path, "show", f"HEAD:{POLICY_SOURCE}") + "\n"
    found = policy_path.read_text()
    expected = insert_test_module(original)
    if found not in (original, expected):
        raise ValueError(f"production policy source has unexpected changes in {path}")
    if found == original:
        policy_path.write_text(expected)
    test_path = policy_path.parent / "shieldspec_privacy_history.rs"
    if test_path.exists() and test_path.read_bytes() != fixture:
        raise ValueError(f"test witness differs from pinned fixture in {path}")
    test_path.write_bytes(fixture)
    changed = git(path, "diff", "--name-only")
    if changed.splitlines() != [POLICY_SOURCE]:
        raise ValueError(f"tracked production edits are not isolated: {changed}")


def reproduce(before: Path | None, after: Path | None, output: Path, target_dir: Path | None, timeout: int) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    fixture = TEST_FIXTURE.read_bytes()
    fixture_hash = hashlib.sha256(fixture).hexdigest()
    with tempfile.TemporaryDirectory(prefix="shieldspec-zallet-677-") as tmp:
        sandbox = Path(tmp)
        if before is None:
            repo = sandbox / "zallet"
            clone = run(
                ["git", "clone", "--filter=blob:none", "--no-checkout", "--depth=1", UPSTREAM, str(repo)],
                timeout=timeout,
            )
            if clone.returncode:
                raise RuntimeError(f"upstream clone failed: {clone.stderr.strip()}")
            git(repo, "fetch", "--depth=1", "origin", BEFORE, AFTER)
            before = sandbox / "before"
            after = sandbox / "after"
            git(repo, "worktree", "add", "--detach", str(before), BEFORE)
            git(repo, "worktree", "add", "--detach", str(after), AFTER)
        assert after is not None
        prepare_checkout(before, BEFORE, fixture)
        prepare_checkout(after, AFTER, fixture)
        env = os.environ.copy()
        env["RUSTUP_TOOLCHAIN"] = "1.95.0"
        env["CARGO_BUILD_JOBS"] = "4"
        env["CARGO_TARGET_DIR"] = str(target_dir or sandbox / "target")

        executions = {}
        for side, checkout in (("before", before), ("after", after)):
            completed = run(COMMAND, cwd=checkout, env=env, timeout=timeout)
            log = completed.stdout + "\n" + completed.stderr
            (output / f"{side}.log").write_text(log)
            executable = re.search(r"Running unittests src/lib\.rs \(([^)]+)\)", log)
            if executable is None:
                raise RuntimeError(f"cannot identify compiled Zallet test runner in {side} log")
            runner = Path(executable.group(1))
            if not runner.is_file():
                raise RuntimeError(f"Zallet test runner missing on disk: {runner}")
            executions[side] = {
                "sdk_git_commit": git(checkout, "rev-parse", "HEAD"),
                "runner_binary_name": runner.name,
                "runner_binary_sha256": hashlib.sha256(runner.read_bytes()).hexdigest(),
                "cargo_exit_code": completed.returncode,
                "output_log": f"{side}.log",
                "log_sha256": hashlib.sha256(log.encode()).hexdigest(),
                "full_privacy_accepted": MARKER + "true" in log,
            }

        before_log = (output / "before.log").read_text()
        after_log = (output / "after.log").read_text()
        outcome = classify_runs(
            executions["before"]["cargo_exit_code"], before_log,
            executions["after"]["cargo_exit_code"], after_log,
        )
        version = run(["rustc", "--version"], env=env)
        verdict = {
            "schema_version": 1,
            "status": outcome,
            "classification": "historical_zallet_fullprivacy_cross_pool_bypass_not_new_vulnerability",
            "source": ISSUE,
            "upstream": UPSTREAM,
            "test_source_sha256": fixture_hash,
            "test_method": "identical test-only Rust module in before/after pinned Zallet checkouts",
            "scenario": {
                "input_pool": "ironwood",
                "change_pool": "sapling",
                "input_zat": 20000,
                "change_zat": 10000,
                "fee_zat": 10000,
                "requested_policy": "FullPrivacy",
                "fixture_shape": "single step, one shielded note, one shielded change; no upstream-case auto-shrinking claimed",
                "deterministic_test_vectors": {
                    "spending_key_bytes": "0x2a repeated 32 times (fixed test-only key)",
                    "rho_bytes": "0x00 repeated 32 times",
                    "rseed_bytes": "0x1b repeated 32 times",
                    "txid_bytes": "0x00 repeated 32 times",
                    "note_position": 0,
                    "received_height": 100,
                    "target_height": 101,
                },
                "privacy_property": "cross-pool value balance reveals amount; FullPrivacy must reject",
            },
            "controls": ["orchard_to_sapling_rejected", "ironwood_to_ironwood_accepted"],
            "command": COMMAND,
            "rustc": version.stdout.strip(),
            "cargo": run(["cargo", "--version"], env=env).stdout.strip(),
            "cargo_target_dir": str(target_dir or sandbox / "target"),
            "utc_time": dt.datetime.now(dt.timezone.utc).isoformat(),
            "sdk": executions,
            "is_documented_prior_bug": True,
            "is_new_privacy_vulnerability": False,
            "is_shieldspec_fuzzer_discovery": False,
        }
        (output / "verdict.json").write_text(json.dumps(verdict, indent=2) + "\n")
        return verdict


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", type=Path)
    parser.add_argument("--after", type=Path)
    parser.add_argument("--cargo-target-dir", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "evidence/historical-zallet-677/results")
    parser.add_argument("--timeout", type=int, default=3600)
    args = parser.parse_args()
    if (args.before is None) != (args.after is None):
        parser.error("--before and --after must be given together")
    verdict = reproduce(args.before, args.after, args.output, args.cargo_target_dir, args.timeout)
    print(json.dumps({"status": verdict["status"], "sdk": verdict["sdk"]}, indent=2))
    return 0 if verdict["status"] == "historical_privacy_policy_difference_reproduced" else 1


if __name__ == "__main__":
    raise SystemExit(main())
