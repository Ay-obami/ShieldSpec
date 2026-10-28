#!/usr/bin/env python3
"""Verify a genuine, pinned, differential rediscovery of historical Zallet #677.

This script does not decide policy outcomes. Both compiled Zallet revisions
independently evaluate FullPrivacy through the ShieldSpec subprocess adapter.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys

EXPECTED_DIFFERENCE = {
    "policy": "zallet_full_privacy_v1",
    "baseline": "rejected",
    "candidate": "accepted",
}
EXPECTED_REVISIONS = {
    "before": "f496c9b6b2c6ebb0065b329c11bda4a5fdf73568",
    "after": "e966780216b7a86b0a1a6618f2fde5b574da7171",
}
ROOT = Path(__file__).resolve().parents[1]


def invoke(command: list[str], expected_exit: int, output: Path) -> dict:

    try:
        completed = subprocess.run(
            command, cwd=ROOT, text=True, capture_output=True,
            check=False, timeout=300,
        )
    except subprocess.TimeoutExpired as exc:
        raise RuntimeError(f"Zallet differential runner timed out: {command[0]}") from exc
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(completed.stdout, encoding="utf-8")
    output.with_suffix(".stderr.txt").write_text(completed.stderr, encoding="utf-8")
    if completed.returncode != expected_exit:
        raise RuntimeError(
            f"unexpected exit {completed.returncode}, expected {expected_exit}: "
            f"{' '.join(command)}\nstdout={completed.stdout[-4000:]}\n"
            f"stderr={completed.stderr[-4000:]}"
        )
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"non-JSON ShieldSpec response in {output}") from exc


def policy_pair(report: dict, expected_baseline: str, expected_candidate: str) -> None:
    for side, expected in (("baseline", expected_baseline), ("candidate", expected_candidate)):
        observation = report.get(f"{side}_wallet_policy")
        if not isinstance(observation, dict):
            raise AssertionError(f"missing real wallet-policy observation: {side}")
        if observation.get("policy") != EXPECTED_DIFFERENCE["policy"]:
            raise AssertionError(f"wrong wallet policy name in {side}: {observation}")
        if observation.get("decision") != expected:
            raise AssertionError(f"wrong observed {side} verdict: {observation}")


def execute(fuzzer: Path, manifest: Path, output_dir: Path, seed: int) -> None:
    source = json.loads(manifest.read_text(encoding="utf-8"))
    if source.get("schema_version") != 1:
        raise AssertionError("unsupported runner manifest schema")
    for side, revision in EXPECTED_REVISIONS.items():
        if source.get("runners", {}).get(side, {}).get("commit") != revision:
            raise AssertionError(f"missing actual pinned Zallet {side} revision")

    # Fuzzer provenance hashes executable launchers, each of which pins the
    # manifest and adapter bytes; the manifest then pins the Zallet binaries.
    launchers = manifest.parent / "launchers"
    shared_args = [
        "--baseline-runner", str(launchers / "zallet-after-runner.py"),
        "--candidate-runner", str(launchers / "zallet-before-runner.py"),
    ]
    output_dir.mkdir(parents=True, exist_ok=True)
    for ordinal, decision in ((0, "accepted"), (1, "rejected")):
        result = invoke(
            [str(fuzzer), "replay", "--scenario-family", "zallet-privacy",
             "--seed", str(seed), "--ordinal", str(ordinal),
             "--format", "json", *shared_args],
            0, output_dir / f"control-{ordinal}.json",
        )
        if result.get("status") != "no_regression":
            raise AssertionError(f"control {ordinal} showed a false differential: {result.get('status')}")
        policy_pair(result["report"], decision, decision)

    discovery = output_dir / "minimized-discovery.json"
    result = invoke(
        [str(fuzzer), "fuzz", "--scenario-family", "zallet-privacy",
         "--seed", str(seed), "--cases", "3", "--format", "json",
         "--save", str(discovery),
         "--baseline-sdk", EXPECTED_REVISIONS["after"],
         "--candidate-sdk", EXPECTED_REVISIONS["before"],
         *shared_args],
        2, output_dir / "campaign.json",
    )
    if result.get("status") != "wallet_policy_difference":
        raise AssertionError(f"no actual wallet-policy divergence: {result.get('status')}")
    if result.get("ordinal") != 2 or result.get("cases_executed") != 3:
        raise AssertionError("expected controls did not precede historical discovery")
    if result.get("comparable_wallet_policy_cases") != 3:
        raise AssertionError("one or more cases lacked comparable real wallet-policy observations")
    if result["report"].get("wallet_policy_difference") != EXPECTED_DIFFERENCE:
        raise AssertionError("actual FullPrivacy difference is not the historical direction")
    policy_pair(result["report"], "rejected", "accepted")
    if not discovery.is_file():
        raise AssertionError("minimized discovery was not persisted")

    replay = invoke([str(fuzzer), "replay", "--input", str(discovery),
                     "--format", "json"], 2, output_dir / "replay.json")
    if replay.get("status") != "wallet_policy_difference_reproduced":
        raise AssertionError("saved discovery did not replay a policy difference")
    if replay.get("recorded_rule_reproduced") is not True:
        raise AssertionError("replay failed to reproduce the precise recorded rule")
    if replay.get("provenance_verified") is not True:
        raise AssertionError("replay did not verify immutable runner provenance")
    if replay["report"].get("wallet_policy_difference") != EXPECTED_DIFFERENCE:
        raise AssertionError("replay changed the real policy difference")
    policy_pair(replay["report"], "rejected", "accepted")
    print("Verified: 2 historical controls, genuine Zallet policy difference, "
          "SHA-pinned minimized replay; no newly discovered vulnerability.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fuzzer", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=2026)
    args = parser.parse_args()
    execute(args.fuzzer.resolve(), args.manifest.resolve(), args.output_dir.resolve(), args.seed)
    return 0


if __name__ == "__main__":
    sys.exit(main())
