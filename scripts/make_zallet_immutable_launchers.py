#!/usr/bin/env python3
"""Create immutable, SHA-linked process runners for ShieldSpec discovery replay.

ShieldSpec hashes the launcher executable itself. Each launcher pins the bytes
of the manifest and adapter, while the adapter pins the real Zallet executable.
This avoids a misleading SHA-256 of the generic Python interpreter.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

LAUNCHER_TEMPLATE = '''#!/usr/bin/env python3
"""Generated, SHA-pinned Zallet FullPrivacy process adapter."""
import hashlib
import os
from pathlib import Path
import sys

MANIFEST = Path({manifest!r})
EXPECTED_MANIFEST_SHA = {manifest_digest!r}
ADAPTER = Path({adapter!r})
EXPECTED_ADAPTER_SHA = {adapter_digest!r}
REVISION = {revision!r}


def sha256(path):
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(64 * 1024), b""):
                digest.update(block)
    except OSError as exc:
        sys.stderr.write(f"missing pinned runner dependency: {{exc}}\\n")
        return None
    return digest.hexdigest()


if sha256(MANIFEST) != EXPECTED_MANIFEST_SHA:
    sys.stderr.write("pinned Zallet manifest SHA-256 mismatch\\n")
    sys.exit(1)
if sha256(ADAPTER) != EXPECTED_ADAPTER_SHA:
    sys.stderr.write("pinned Zallet adapter SHA-256 mismatch\\n")
    sys.exit(1)
os.execv(sys.executable, [
    sys.executable, str(ADAPTER),
    "--manifest", str(MANIFEST), "--revision", REVISION,
])
'''


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(64 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def make_launchers(manifest: Path, adapter: Path, output_dir: Path) -> dict[str, Path]:
    manifest, adapter = manifest.resolve(strict=True), adapter.resolve(strict=True)
    manifest_sha = sha256_file(manifest)
    adapter_sha = sha256_file(adapter)
    output_dir.mkdir(parents=True, exist_ok=True)
    paths = {}
    for revision in ("before", "after"):
        launcher = output_dir / f"zallet-{revision}-runner.py"
        launcher.write_text(
            LAUNCHER_TEMPLATE.format(
                manifest=str(manifest), manifest_digest=manifest_sha,
                adapter=str(adapter), adapter_digest=adapter_sha,
                revision=revision,
            ),
            encoding="utf-8",
        )
        launcher.chmod(0o755)
        paths[revision] = launcher.resolve(strict=True)
    return paths


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--adapter", type=Path, default=Path(__file__).with_name("zallet_policy_runner.py"))
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    for revision, path in make_launchers(args.manifest, args.adapter, args.output_dir).items():
        print(f"{revision}: {path} sha256={sha256_file(path)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
