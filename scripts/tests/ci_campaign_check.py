#!/usr/bin/env python3
"""Fail-closed CI assertions for real SDK policy observations and NU6.3 replays.

This is a negative control comparing the same opt-in SDK runner with itself,
not historical Zallet FullPrivacy reproduction or a claim of no privacy leak.
"""
import argparse
import json
from pathlib import Path
import sys

POLICY = "zcash_spend_orchard_only_v1"
CROSSING = "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"


def expect(actual, expected, field):
    if actual != expected:
        raise ValueError(f"{field}: expected {expected!r}, got {actual!r}")


def check_campaign(data, seed, cases):
    expect(data["status"], "no_regression_found", "status")
    expect(data["scenario_family"], "ironwood", "scenario_family")
    expect(data["seed"], seed, "seed")
    expect(data["cases_executed"], cases, "cases_executed")
    for field in (
        "baseline_wallet_policy_observations",
        "candidate_wallet_policy_observations",
        "comparable_wallet_policy_cases",
    ):
        expect(data[field], cases, field)
    for field in ("ordinal", "report"):
        expect(data.get(field), None, field)
    print(f"verified {cases}/{cases} comparable real SDK wallet-policy observations")


def check_replay(data, seed, ordinal):
    if ordinal not in range(4):
        raise ValueError("CI replay ordinals must be 0, 1, 2, or 3")
    expect(data["status"], "no_regression", "status")
    scenario = data["scenario"]
    expect(scenario["kind"], "nu63_orchard_payment", "scenario.kind")
    params = scenario["params"]
    pool = "orchard" if ordinal < 2 else "sapling"
    activation = "at" if ordinal % 2 == 0 else "before"
    expect(params["activation"], activation, "NU6.3 activation")
    expect(params["payment"]["funding_pool"], pool, "funding pool")
    expect(params["payment"]["change_pool"], pool, "change pool")

    report = data["report"]
    expect(report["regression"]["passes"], True, "regression.passes")
    expect(report["regression"]["findings"], [], "regression.findings")
    expect(report.get("wallet_policy_difference"), None, "wallet_policy_difference")
    verdict = "accepted" if pool == "orchard" else "rejected"
    for side in ("baseline", "candidate"):
        expect(
            report[f"{side}_wallet_policy"],
            {"policy": POLICY, "decision": verdict},
            f"{side} actual SDK policy verdict",
        )

    before = report["baseline"]["findings"]
    after = report["candidate"]["findings"]
    expect(after, before, "identical-runner assessment evidence")
    crossing = [finding for finding in before if finding["rule_id"] == CROSSING]
    if activation == "at":
        if not any(
            finding["evidence"]["source_pool"] == pool
            and finding["evidence"]["destination_pool"] == "ironwood"
            for finding in crossing
        ):
            raise ValueError("activation case lacks real cross-pool Ironwood snapshot evidence")
    elif any(
        finding["evidence"]["destination_pool"] == "ironwood" for finding in crossing
    ):
        raise ValueError("pre-activation snapshot unexpectedly uses Ironwood")

    reproduction = data["reproduction"]
    for fragment in (
        f"--seed {seed} --ordinal {ordinal} --scenario-family ironwood",
        "--baseline-arg --wallet-spend-policy --baseline-arg orchard-only",
        "--candidate-arg --wallet-spend-policy --candidate-arg orchard-only",
    ):
        if fragment not in reproduction:
            raise ValueError(f"replay command loses reproducible context: {fragment}")
    print(f"verified ordinal {ordinal}: {pool}, {activation} NU6.3, {verdict} SDK verdict")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("campaign", "replay"))
    parser.add_argument("input", type=Path)
    parser.add_argument("--seed", required=True, type=int)
    parser.add_argument("--cases", type=int)
    parser.add_argument("--ordinal", type=int)
    args = parser.parse_args()
    try:
        data = json.loads(args.input.read_text(encoding="utf-8"))
        if args.mode == "campaign":
            if args.cases != 4:
                raise ValueError("CI campaign must cover all four NU6.3 cases")
            check_campaign(data, args.seed, args.cases)
        else:
            if args.ordinal is None:
                raise ValueError("replay requires --ordinal")
            check_replay(data, args.seed, args.ordinal)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"CI wallet-policy evidence check FAILED: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
