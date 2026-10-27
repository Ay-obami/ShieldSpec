#!/usr/bin/env python3
"""Build a captioned MP4 from *executed* ShieldSpec demo output.

No simulated upstream discovery: the Zallet history reproduces a previously
fixed real privacy-policy difference; the separate CLI demo has a labeled fault.
Requires Pillow and ffmpeg on the local machine. Video contains no voiceover.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
GOLD = "#f5c04e"
BG = "#111827"
PANEL = "#202b3d"
WHITE = "#f3f4f6"
MUTED = "#b6c4d5"
GREEN = "#8ee0be"
RED = "#ffaaa5"

def font(size: int, mono: bool = False):
    family = "DejaVuSansMono.ttf" if mono else "DejaVuSans.ttf"
    return ImageFont.truetype(f"/usr/share/fonts/truetype/dejavu/{family}", size)

def run_demo(binary: Path, arguments: list[str]) -> str:
    result = subprocess.run([str(binary), *arguments], cwd=ROOT, capture_output=True, text=True, timeout=300, check=False)
    if result.returncode != 2 or "SS003_TRANSPARENT_CHANGE" not in result.stdout:
        raise RuntimeError(f"ShieldSpec demo did not reproduce the expected policy failure: {result.returncode}\n{result.stderr}")
    return result.stdout

def draw_card(draw, xy, label, lines, accent=GOLD):
    left, top, right, bottom = xy
    draw.rounded_rectangle(xy, radius=22, fill=PANEL)
    draw.text((left + 30, top + 22), label, fill=accent, font=font(32))
    y = top + 88
    for line in lines:
        draw.text((left + 30, y), line[:75], fill=WHITE, font=font(26, True))
        y += 43
        if y > bottom - 36:
            break

def slide(number: int, total: int, title: str, subtitle: str, cards: list[tuple], note: str, path: Path):
    im = Image.new("RGB", (1920, 1080), BG)
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((70, 65, 330, 116), radius=16, fill=GOLD)
    d.text((91, 75), "SHIELDSPEC", font=font(27), fill="#151515")
    d.text((1520, 80), "ZECATHON · CORE & TOOLING", font=font(23), fill=MUTED)
    d.text((95, 158), title, font=font(66), fill=WHITE)
    d.text((98, 260), subtitle, font=font(28), fill=MUTED)
    if len(cards) == 2:
        positions = [(95, 355, 933, 870), (957, 355, 1820, 870)]
    else:
        positions = [(95, 355, 1820, 870)]
    for position, card in zip(positions, cards):
        draw_card(d, position, *card)
    d.text((98, 923), note, font=font(25), fill=GREEN)
    d.line((95, 1001, 1820, 1001), fill="#445065", width=2)
    d.rectangle((95, 998, 95 + int(1725 * number / total), 1004), fill=GOLD)
    d.text((95, 1020), "Real Zcash proposal execution  •  Deterministic regression evidence  •  No secrets", font=font(22), fill=MUTED)
    im.save(path)

def build(outdir: Path) -> Path:
    verdict = json.loads((ROOT / "evidence/historical-zallet-677/results/verdict.json").read_text())
    before = (ROOT / "evidence/historical-zallet-677/results/before.log").read_text()
    after = (ROOT / "evidence/historical-zallet-677/results/after.log").read_text()
    if (verdict["status"] != "historical_privacy_policy_difference_reproduced"
            or "SHIELDSPEC_IRONWOOD_TO_SAPLING_FULL_PRIVACY accepted=true" not in before
            or "test result: FAILED. 2 passed; 1 failed" not in before
            or "SHIELDSPEC_IRONWOOD_TO_SAPLING_FULL_PRIVACY accepted=false" not in after
            or "test result: ok. 3 passed; 0 failed" not in after):
        raise RuntimeError("Historical Zallet privacy-policy proof is absent or inconclusive")
    binary = ROOT / "target/debug/shieldspec-fuzz"
    if not binary.is_file():
        raise RuntimeError("First build the real CLI: cargo build --locked -p shieldspec-zcash-runner --bins")
    demo_args = ["demo", "--format", "human"]
    replay_args = ["replay", "--seed", "148", "--ordinal", "0", "--policy", "standard", "--demo-regression", "transparent-change", "--format", "human"]
    demo = run_demo(binary, demo_args)
    replay = run_demo(binary, replay_args)
    if "status: privacy_regression_reproduced" not in replay:
        raise RuntimeError("Saved-seed differential did not replay")

    outdir.mkdir(parents=True, exist_ok=True)
    (outdir / "actual-demo-stdout.txt").write_text(demo)
    (outdir / "actual-replay-stdout.txt").write_text(replay)
    manifest = {
        "type": "captioned_technical_demo_from_real_cli_outputs",
        "voiceover": "not_recorded",
        "historical_classification": verdict["classification"],
        "historical_implementation": "zcash/zallet zallet-core privacy-policy implementation",
        "before_zallet_git_commit": verdict["sdk"]["before"]["sdk_git_commit"],
        "after_zallet_git_commit": verdict["sdk"]["after"]["sdk_git_commit"],
        "historical_test_sha256": verdict["test_source_sha256"],
        "demo_command": [str(binary), *demo_args],
        "replay_command": [str(binary), *replay_args],
        "demo_stdout_sha256": hashlib.sha256(demo.encode()).hexdigest(),
        "replay_stdout_sha256": hashlib.sha256(replay.encode()).hexdigest(),
        "demo_fault": "deliberately_injected_after_real_sdk_proposal_normalization",
        "claim_of_new_upstream_privacy_bug": False,
    }
    (outdir / "video-evidence.json").write_text(json.dumps(manifest, indent=2) + "\n")

    sequence = [
        ("Zcash privacy testing, made reproducible",
         "A developer tool for wallet proposal regressions",
         [("REAL SDK EXECUTION", ["Generated wallet states", "Real librustzcash proposal machinery", "Separate baseline and candidate processes", "Privacy invariants → shrink → replay"])],
         "Every example is reproducible; no keys or mainnet transactions.", 11),
        ("A real historical privacy-policy bug",
         "zcash/zallet #677 · FullPrivacy missed Ironwood→Sapling value crossing",
         [("BEFORE · f496c9b6", ["Real Proposal: 20,000-zat Ironwood note", "Sapling change: 10,000 zat", "Policy: FullPrivacy", "Observed: accepted=true (wrong)", "Result: 2 pass; 1 expected test fail"], RED),
          ("AFTER · e9667802", ["Same Rust fixture and real Proposal", "Same Ironwood→Sapling crossing", "Observed: accepted=false (correct)", "Result: all 3 tests pass", "Two controls agree across versions"], GREEN)],
         "Previously documented and fixed; NOT a new bug or auto-discovery.", 21),
        ("ShieldSpec executes real proposals",
         "Transparent change is caught by a policy-aware differential",
         [("ACTUAL COMMAND OUTPUT", [
             "shieldspec-fuzz demo --format human",
             "status: privacy_regression",
             "seed: 148   ordinal: 0",
             "baseline findings: 0",
             "candidate findings: 2",
             "SS003_TRANSPARENT_CHANGE [Error]",
             "SS005_PRIVACY_BOUNDARY [Warning]",
         ])],
         "The candidate fault is injected after SDK normalization for this deterministic demo.", 23),
        ("A minimal case you can replay",
         "Shrink the wallet state while preserving the finding",
         [("MINIMIZED REAL-PROPOSAL SCENARIO", [
             "funding_zat: 30001",
             "payment_zat: 1",
             "note_count: 1   confirmations: 1",
             "funding_pool: orchard",
             "change_pool: orchard",
             "recipient_seed: 0",
         ]), ("DETERMINISTIC REPLAY", [
             "replay --seed 148 --ordinal 0",
             "--demo-regression transparent-change",
             "status: privacy_regression_reproduced",
             "SS003_TRANSPARENT_CHANGE",
             "SS005_PRIVACY_BOUNDARY",
             "runner SHA-256 verifies saved discoveries",
         ], GREEN)],
         "The same seed and runner binaries reproduce the same configured privacy findings.", 22),
        ("Understand Zcash-specific semantics",
         "Correct behavior is not automatically a regression",
         [("REAL ZIP-320 TEST", [
             "Orchard shielded input",
             "→ ephemeral transparent bridge (step 0)",
             "→ prior-step transparent input (step 1)",
             "→ TEX recipient",
             "Bridge != ordinary transparent wallet change",
             "Expected recipient exposure remains explicit",
         ])],
         "False-positive control: preserve the intended multi-step bridge provenance.", 19),
        ("Built for developer workflows",
         "Run locally and in GitHub Actions",
         [("INTEGRATION", [
             "Rust library + privacy policy engine",
             "CLI: human / JSON / SARIF",
             "Runner-isolated differential fuzzing",
             "Minimized scenarios with SHA-256 provenance",
             "CI smoke + historical SDK verification",
         ])],
         "Zcash-only evidence: zallet #677/#693, librustzcash #2614/#2775.", 16),
        ("ShieldSpec",
         "Privacy regressions should become repeatable tests before release",
         [("REPOSITORY & EVIDENCE", [
             "github.com/Ay-obami/ShieldSpec",
             "evidence/historical-zallet-677/results/",
             "docs/submission.md",
             "docs/demo-video-script.md",
         ])],
         "Historical fixed Zallet bug; CLI finding is explicitly injected. No new vuln.", 12),
    ]
    with tempfile.TemporaryDirectory(prefix="shieldspec-video-") as tmp:
        root = Path(tmp)
        listing = []
        for index, (title, subtitle, cards, note, duration) in enumerate(sequence, start=1):
            filename = root / f"slide-{index:02d}.png"
            slide(index, len(sequence), title, subtitle, cards, note, filename)
            listing.append(f"file '{filename}'\n")
            listing.append(f"duration {duration}\n")
        listing.append(f"file '{root / ('slide-' + str(len(sequence)).zfill(2) + '.png')}'\n")
        concat = root / "slides.ffconcat"
        concat.write_text("ffconcat version 1.0\n" + "".join(listing))
        video = outdir / "shieldspec-zecathon-captioned-demo.mp4"
        command = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-safe", "0", "-f", "concat", "-i", str(concat), "-r", "24", "-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(video)]
        subprocess.run(command, check=True, timeout=300)
    return video

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, default=ROOT / "demo-artifacts")
    args = parser.parse_args()
    print(build(args.output_dir))
