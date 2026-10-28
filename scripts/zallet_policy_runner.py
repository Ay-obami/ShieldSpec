#!/usr/bin/env python3
"""One-case ShieldSpec runner backed by the real, pinned Zallet FullPrivacy policy.

No historical verdict is embedded in this adapter. Each call executes the selected
Zallet-core test executable, which constructs a typed Proposal and invokes the
private production enforce_privacy_policy function in its source module.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

MARKER = "SHIELDSPEC_POLICY_PROBE_JSON="
POLICY = "zallet_full_privacy_v1"
MAX_MONEY = 21_000_000 * 100_000_000
REVISIONS = {
    "before": "f496c9b6b2c6ebb0065b329c11bda4a5fdf73568",
    "after": "e966780216b7a86b0a1a6618f2fde5b574da7171",
}
FIXTURE = Path(__file__).resolve().parents[1] / (
    "evidence/historical-zallet-677/shieldspec_dynamic_policy_probe.rs"
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_case(case: object) -> dict:
    """Canonicalize the supplied fuzzer case; no hidden default pool or verdict."""
    if not isinstance(case, dict) or set(case) != {"kind", "params"}:
        raise ValueError("expected exactly kind and params")
    if case["kind"] != "zallet_privacy_probe":
        raise ValueError("not a zallet_privacy_probe scenario")
    params = case["params"]
    if not isinstance(params, dict) or set(params) != {
        "input_pool", "change_pool", "input_zat", "fee_zat"
    }:
        raise ValueError("missing or unknown policy scenario fields")
    if params["input_pool"] not in ("ironwood", "orchard"):
        raise ValueError("unsupported input pool: only real Orchard-family notes supported")
    if params["change_pool"] not in ("sapling", "orchard", "ironwood"):
        raise ValueError("unsupported change pool")
    input_zat, fee_zat = params["input_zat"], params["fee_zat"]
    if type(input_zat) is not int or type(fee_zat) is not int:
        raise ValueError("amount and fee must be exact integers")
    if not 0 < fee_zat < input_zat <= MAX_MONEY:
        raise ValueError("amount must be positive, fund both fee and positive change")
    return {
        "input_pool": params["input_pool"],
        "change_pool": params["change_pool"],
        "input_zat": input_zat,
        "fee_zat": fee_zat,
        "change_zat": input_zat - fee_zat,
    }


def parse_probe_output(stdout: str, case: dict) -> dict:
    """Require exactly one genuine Rust marker with all actual proposal facts."""
    params = validate_case(case)
    candidates = re.findall(re.escape(MARKER) + r"(\{[^\r\n]*\})", stdout)
    if len(candidates) != 1:
        raise ValueError(f"expected exactly one policy probe marker, got {len(candidates)}")
    try:
        record = json.loads(candidates[0])
    except json.JSONDecodeError as error:
        raise ValueError("malformed Rust policy probe JSON") from error
    if not isinstance(record, dict) or record.get("schema_version") != 1:
        raise ValueError("invalid policy probe schema")
    if record.get("policy") != POLICY or type(record.get("accepted")) is not bool:
        raise ValueError("no real named FullPrivacy decision")
    proposal = record.get("observed_proposal")
    if not isinstance(proposal, dict):
        raise ValueError("missing typed Proposal observation")
    for field, expected in (
        ("step_count", 1), ("input_count", 1), ("change_count", 1),
        ("recipient_count", 0), *params.items()
    ):
        value = proposal.get(field)
        if type(value) is not type(expected) or value != expected:
            raise ValueError(f"typed Proposal {field} mismatch: expected {expected!r}, got {value!r}")
    return record


def protocol_observation(record: dict) -> dict:
    proposal = record["observed_proposal"]
    return {
        "protocol_version": 1,
        "status": "observed",
        "snapshot": {
            "schema_version": 1,
            "network": "local-test",
            "steps": [{
                "index": 0,
                "inputs": [{
                    "pool": proposal["input_pool"],
                    "value_zat": proposal["input_zat"],
                }],
                "outputs": [{
                    "pool": proposal["change_pool"],
                    "role": "change",
                    "value_zat": proposal["change_zat"],
                }],
                "fee_zat": proposal["fee_zat"],
            }],
        },
        "wallet_policy": {
            "policy": POLICY,
            "decision": "accepted" if record["accepted"] else "rejected",
        },
    }


def verify_runner(manifest: dict, revision: str) -> Path:
    if revision not in REVISIONS:
        raise ValueError("unknown historical Zallet revision")
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise ValueError("invalid runner manifest")
    if manifest.get("fixture_sha256") != sha256_file(FIXTURE):
        raise ValueError("runner fixture source hash does not match checked-in source")
    runner = manifest.get("runners", {}).get(revision)
    if not isinstance(runner, dict) or runner.get("commit") != REVISIONS[revision]:
        raise ValueError("wrong Zallet source commit in runner manifest")
    if not isinstance(runner.get("executable"), str):
        raise ValueError("missing executable path")
    executable = Path(runner["executable"])
    if not executable.is_absolute() or not executable.is_file():
        raise ValueError("historical Zallet runner binary unavailable")
    claimed = runner.get("sha256")
    if not isinstance(claimed, str) or not re.fullmatch(r"[0-9a-f]{64}", claimed):
        raise ValueError("missing compiled binary fingerprint")
    actual = sha256_file(executable)
    if actual != claimed:
        raise ValueError(f"Zallet {revision} executable sha256 mismatch")
    return executable


def execute(executable: Path, case: dict, timeout: int) -> dict:
    # Both pinned revisions receive byte-equivalent canonical case parameters.
    canonical = validate_case(case)
    env = os.environ.copy()
    env["SHIELDSPEC_ZALLET_SCENARIO_JSON"] = json.dumps(canonical, sort_keys=True)
    try:
        result = subprocess.run(
            [str(executable), "shieldspec_policy_probe_dynamic", "--nocapture", "--test-threads=1"],
            env=env, capture_output=True, text=True, timeout=timeout, check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ValueError("Zallet runner execution failed") from error
    if result.returncode != 0:
        raise ValueError(
            f"Zallet runner failed operationally (exit {result.returncode}); "
            + (result.stderr or result.stdout)[-500:]
        )
    return parse_probe_output(result.stdout, case)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--revision", choices=tuple(REVISIONS), required=True)
    parser.add_argument("--timeout", type=int, default=90)
    args = parser.parse_args()
    try:
        if args.timeout < 1 or args.timeout > 3600:
            raise ValueError("invalid timeout")
        # The generic engine writes exactly one JSON case to standard input.
        incoming = sys.stdin.read()
        case = json.loads(incoming)
        validate_case(case)
        manifest = json.loads(args.manifest.read_text())
        executable = verify_runner(manifest, args.revision)
        record = execute(executable, case, args.timeout)
        print(json.dumps(protocol_observation(record), separators=(",", ":")))
        return 0
    except (ValueError, KeyError, TypeError, OSError, json.JSONDecodeError) as error:
        print(f"Zallet FullPrivacy runner operational error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
