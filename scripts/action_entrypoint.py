#!/usr/bin/env python3
"""Validated, shell-free command boundary for the ShieldSpec composite Action."""

import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


POLICIES = {"standard", "strict"}
SCENARIOS = {"legacy", "ironwood", "transparent", "zallet-privacy"}


class InputError(Exception):
    pass


def setting(name: str, default: str = "") -> str:
    return os.environ.get("SHIELDSPEC_" + name, default)


def resolve_path(value: str, *, executable: bool = False) -> Path:
    if not value:
        raise InputError("a required path is empty")
    path = Path(value)
    if not path.is_absolute():
        path = Path(os.environ["GITHUB_WORKSPACE"]) / path
    path = path.resolve()
    if not path.is_file():
        raise InputError(f"file does not exist: {path}")
    if executable and not os.access(path, os.X_OK):
        raise InputError(f"runner is not executable: {path}")
    return path


def bounded_integer(name: str, value: str, *, maximum: int, minimum: int = 0) -> str:
    if not re.fullmatch(r"[0-9]+", value) or not minimum <= int(value) <= maximum:
        raise InputError(f"{name} must be a decimal integer from {minimum} to {maximum}")
    return value


def runner_args(value: str, flag: str) -> list[str]:
    arguments = []
    for line in value.splitlines():
        if not line:
            raise InputError(f"{flag} contains an empty line")
        arguments.extend((flag, line))
    return arguments


def tool_path(env_name: str, default: str, action_root: Path) -> Path:
    path = Path(os.environ.get(env_name) or action_root / default).resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise InputError(f"required ShieldSpec executable is unavailable: {path}")
    return path


def run_snapshot(action_root: Path) -> int:
    candidate = resolve_path(setting("CANDIDATE"))
    policy = setting("POLICY", "standard")
    output_format = setting("FORMAT", "human")
    if policy not in POLICIES or output_format not in {"human", "json", "sarif"}:
        raise InputError("invalid snapshot policy or format")
    cli = os.environ.get("SHIELDSPEC_SNAPSHOT_BIN")
    command = [cli] if cli else ["cargo", "run", "--locked", "-p", "shieldspec-cli", "--"]
    if setting("BASELINE"):
        command.extend(("diff", "--baseline", str(resolve_path(setting("BASELINE"))),
                        "--candidate", str(candidate)))
    else:
        command.extend(("check", "--input", str(candidate)))
    command.extend(("--policy", policy, "--format", output_format))
    return subprocess.run(command, cwd=action_root, check=False).returncode


def run_differential(action_root: Path) -> tuple[int, str, str]:
    preset = setting("PRESET", "zcash-sdk")
    if preset not in {"zcash-sdk", "custom"}:
        raise InputError("preset must be zcash-sdk or custom")
    cases = bounded_integer("cases", setting("CASES", "2"), maximum=10000, minimum=1)
    seed = bounded_integer("seed", setting("SEED", "148"), maximum=2**64 - 1)
    scenario = setting("SCENARIO_FAMILY", "legacy")
    policy = setting("POLICY", "standard")
    if scenario not in SCENARIOS or policy not in POLICIES:
        raise InputError("invalid scenario family or policy")

    if preset == "zcash-sdk":
        if setting("BASELINE_RUNNER") or setting("CANDIDATE_RUNNER"):
            raise InputError("zcash-sdk preset cannot be combined with custom runner paths")
        if setting("BASELINE_ARGS") or setting("CANDIDATE_ARGS"):
            raise InputError("zcash-sdk preset cannot be combined with custom runner arguments")
        runner = tool_path("SHIELDSPEC_BUILTIN_RUNNER_BIN", "target/debug/shieldspec-zcash-runner", action_root)
        baseline = candidate = runner
        print("ShieldSpec zcash-sdk preset: first-party SDK negative control, not a test of the caller's wallet", file=sys.stderr)
    else:
        baseline = resolve_path(setting("BASELINE_RUNNER"), executable=True)
        candidate = resolve_path(setting("CANDIDATE_RUNNER"), executable=True)

    temp_root = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir())
    temp_root.mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="shieldspec-action-", dir=temp_root))
    report = evidence / "campaign.json"
    discovery = evidence / "discovery.json"
    fuzzer = tool_path("SHIELDSPEC_FUZZ_BIN", "target/debug/shieldspec-fuzz", action_root)
    command = [str(fuzzer), "fuzz", "--baseline-runner", str(baseline),
               "--candidate-runner", str(candidate), "--scenario-family", scenario,
               "--seed", seed, "--cases", cases, "--policy", policy,
               "--format", "json", "--save", str(discovery)]
    if preset == "custom":
        command.extend(runner_args(setting("BASELINE_ARGS"), "--baseline-arg"))
        command.extend(runner_args(setting("CANDIDATE_ARGS"), "--candidate-arg"))
    with report.open("w", encoding="utf-8") as output:
        completed = subprocess.run(command, stdout=output, cwd=action_root, check=False)
    print(report.read_text(encoding="utf-8"), end="")
    return completed.returncode, str(report), str(discovery) if discovery.is_file() else ""


def write_outputs(status: int, report: str, discovery: str) -> None:
    output_file = os.environ.get("GITHUB_OUTPUT")
    if output_file:
        with open(output_file, "a", encoding="utf-8") as output:
            output.write(f"status={status}\nreport_path={report}\ndiscovery_path={discovery}\n")


def main() -> int:
    status, report, discovery = 1, "", ""
    try:
        root = Path(os.environ["SHIELDSPEC_ACTION_PATH"]).resolve()
        mode = setting("MODE", "snapshot")
        if mode == "snapshot":
            status = run_snapshot(root)
        elif mode == "differential":
            status, report, discovery = run_differential(root)
        else:
            raise InputError("mode must be snapshot or differential")
        if status not in (0, 1, 2):
            print(f"ShieldSpec returned unsupported status {status}", file=sys.stderr)
            status = 1
    except (InputError, OSError) as error:
        print(f"ShieldSpec Action: {error}", file=sys.stderr)
    write_outputs(status, report, discovery)
    # The composite Action uploads evidence before its final step enforces status.
    return 0


if __name__ == "__main__":
    sys.exit(main())
