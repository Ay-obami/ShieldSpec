#!/usr/bin/env python3
"""Build and fingerprint independently compiled historical Zallet policy probes.

Uses identical test-only source added to payments.rs at the module boundary
for both SHA-pinned Zallet revisions. Does not change production policy logic.
The original separately checked-in historical witness is left untouched.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = "https://github.com/zcash/zallet.git"
BEFORE = "f496c9b6b2c6ebb0065b329c11bda4a5fdf73568"
AFTER = "e966780216b7a86b0a1a6618f2fde5b574da7171"
POLICY_SOURCE = "zallet-core/src/components/json_rpc/payments.rs"
FIXTURE = ROOT / "evidence/historical-zallet-677/shieldspec_dynamic_policy_probe.rs"
TEST_MODULE = (
    '\n#[cfg(test)]\n'
    '#[path = "shieldspec_dynamic_policy_probe.rs"]\n'
    'mod shieldspec_dynamic_policy_probe;\n'
)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def insert_test_module(source: str) -> str:
    if source.endswith(TEST_MODULE):
        return source
    if "mod shieldspec_dynamic_policy_probe;" in source:
        raise ValueError("unrecognized existing dynamic module registration")
    return source.rstrip("\n") + "\n" + TEST_MODULE


def checked_command(command: list[str], *, cwd: Path | None = None,
                    env: dict | None = None, timeout: int = 3600) -> str:
    run = subprocess.run(command, cwd=cwd, env=env, timeout=timeout,
                         capture_output=True, text=True, check=False)
    if run.returncode != 0:
        raise RuntimeError(
            f"{command[0]} failed (exit {run.returncode}): {run.stderr[-1200:]}"
        )
    return run.stdout.strip()


def git(repo: Path, *args: str, timeout: int = 600) -> str:
    return checked_command(["git", "-C", str(repo), *args], timeout=timeout)


def prepare_checkout(checkout: Path, revision: str, fixture: bytes) -> None:
    actual = git(checkout, "rev-parse", "HEAD")
    if actual != revision:
        raise ValueError(f"Zallet SHA mismatch: expected {revision}, found {actual}")
    policy = checkout / POLICY_SOURCE
    original = git(checkout, "show", f"HEAD:{POLICY_SOURCE}") + "\n"
    expected = insert_test_module(original)
    current = policy.read_text()
    if current not in (original, expected):
        raise ValueError("existing production policy source modifications cannot be overwritten")
    if current != expected:
        policy.write_text(expected)
    test_path = policy.parent / FIXTURE.name
    if test_path.exists() and test_path.read_bytes() != fixture:
        raise ValueError("different dynamic witness already injected")
    test_path.write_bytes(fixture)
    changed = git(checkout, "diff", "--name-only").splitlines()
    if changed != [POLICY_SOURCE]:
        raise ValueError(f"unexpected tracked Zallet changes: {changed}")
    if policy.read_text() != expected:
        raise ValueError("production policy module body differs from pinned source")


def select_test_executable(log: Path) -> Path:
    """Find the test-only zallet-core lib executable from compiler artifact JSON."""
    matches = []
    with log.open() as stream:
        for line in stream:
            try:
                artifact = json.loads(line)
            except json.JSONDecodeError:
                continue
            if artifact.get("reason") != "compiler-artifact":
                continue
            target = artifact.get("target", {})
            if target.get("name") != "zallet_core":
                continue
            if "lib" not in target.get("kind", []):
                continue
            executable = artifact.get("executable")
            if isinstance(executable, str) and executable:
                matches.append(Path(executable))
    if len(matches) != 1 or not matches[0].is_file():
        raise RuntimeError(f"expected one built zallet-core lib test executable; found {matches}")
    return matches[0]


def compile_probe(checkout: Path, revision: str, output: Path,
                  target: Path, timeout: int) -> dict:
    env = os.environ.copy()
    env["RUSTUP_TOOLCHAIN"] = env.get("RUSTUP_TOOLCHAIN", "1.95.0")
    env["CARGO_INCREMENTAL"] = "0"
    env["CARGO_PROFILE_TEST_DEBUG"] = "0"
    env["CARGO_BUILD_JOBS"] = env.get("CARGO_BUILD_JOBS", "2")
    env["CARGO_TARGET_DIR"] = str(target.resolve())
    side = "before" if revision == BEFORE else "after"
    stdout = output / f"{side}.cargo.jsonl"
    stderr = output / f"{side}.cargo.stderr.log"
    command = ["cargo", "test", "--locked", "-p", "zallet-core", "--lib",
               "--no-run", "--message-format=json"]
    with stdout.open("w") as out_stream, stderr.open("w") as err_stream:
        try:
            result = subprocess.run(
                command, cwd=checkout, env=env, timeout=timeout,
                stdout=out_stream, stderr=err_stream, check=False
            )
        except subprocess.TimeoutExpired as error:
            raise RuntimeError(f"{side} compilation timed out; not a policy finding") from error
    if result.returncode != 0:
        raise RuntimeError(
            f"{side} Zallet compilation failed (exit {result.returncode}); "
            + stderr.read_text()[-2000:]
        )
    compiled = select_test_executable(stdout)
    # Shared Cargo target filenames can be overwritten by next revision.
    # Copy each result immediately to a unique immutable side-specific path.
    persistent = output / f"zallet-core-{side}"
    shutil.copy2(compiled, persistent)
    persistent.chmod(persistent.stat().st_mode | 0o111)
    return {
        "commit": revision,
        "executable": str(persistent.resolve()),
        "sha256": sha256_file(persistent),
        "build_command": command,
        "cargo_stdout": str(stdout.resolve()),
        "cargo_stderr": str(stderr.resolve()),
        "rustc": checked_command(["rustc", "--version"], env=env),
    }


def build(output: Path, manifest: Path, before: Path | None,
          after: Path | None, target: Path | None, timeout: int) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    manifest.parent.mkdir(parents=True, exist_ok=True)
    fixture = FIXTURE.read_bytes()
    fixture_hash = hashlib.sha256(fixture).hexdigest()
    with tempfile.TemporaryDirectory(prefix="shieldspec-zallet-probe-") as tmp:
        temp = Path(tmp)
        if before is None:
            repo = temp / "zallet"
            checked_command(
                ["git", "clone", "--filter=blob:none", "--no-checkout",
                 "--depth=1", UPSTREAM, str(repo)], timeout=timeout
            )
            git(repo, "fetch", "--depth=1", "origin", BEFORE, AFTER, timeout=timeout)
            before, after = temp / "before", temp / "after"
            git(repo, "worktree", "add", "--detach", str(before), BEFORE)
            git(repo, "worktree", "add", "--detach", str(after), AFTER)
        assert before is not None and after is not None
        prepare_checkout(before, BEFORE, fixture)
        prepare_checkout(after, AFTER, fixture)
        target_dir = (target or output / "cargo-target").resolve()
        runners = {}
        for name, checkout, revision in (
            ("before", before, BEFORE), ("after", after, AFTER)
        ):
            runners[name] = compile_probe(
                checkout, revision, output, target_dir, timeout
            )
        result = {
            "schema_version": 1,
            "classification": "documented_historical_zallet_677_not_new_vulnerability",
            "fixture_sha256": fixture_hash,
            "source_url": "https://github.com/zcash/zallet/pull/677",
            "runners": runners,
            "built_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        }
        manifest.write_text(json.dumps(result, indent=2) + "\n")
        return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--before-path", type=Path, help="pre-cloned pinned before checkout")
    parser.add_argument("--after-path", type=Path, help="pre-cloned pinned after checkout")
    parser.add_argument("--cargo-target-dir", type=Path)
    parser.add_argument("--timeout", type=int, default=3600)
    args = parser.parse_args()
    if (args.before_path is None) != (args.after_path is None):
        parser.error("--before-path and --after-path must be specified together")
    if args.timeout < 1:
        parser.error("--timeout must be positive")
    result = build(args.output_dir.resolve(), args.manifest.resolve(),
                   args.before_path, args.after_path,
                   args.cargo_target_dir, args.timeout)
    print(json.dumps({
        "status": "compiled_independent_pinned_zallet_policy_probes",
        "manifest": str(args.manifest.resolve()),
        "before_sha256": result["runners"]["before"]["sha256"],
        "after_sha256": result["runners"]["after"]["sha256"],
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
