#!/usr/bin/env python3
"""Render the LayerTree research stills and film shots with source captions."""
import argparse
import hashlib
import json
import os
from functools import cache
from pathlib import Path
import subprocess

REPO = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--viewer", required=True, type=Path)
parser.add_argument("--film-shot", type=Path)
parser.add_argument("--scratch", required=True, type=Path)
parser.add_argument("--signet", action="store_true", help="Render the confirmed signet and separate negative-policy views.")
parser.add_argument("--signet-halt", action="store_true", help="Render the measured guardian HALT while its escape waits for CSV.")
parser.add_argument("--signet-halt-paid", action="store_true", help="Render confirmed member payouts after the guardian HALT.")
parser.add_argument("--clips", type=Path, default=Path.home() / "work/film-out/verse")
args = parser.parse_args()
args.scratch.mkdir(parents=True, exist_ok=True)
output = REPO / "docs/research/layertree/captures"
output.mkdir(parents=True, exist_ok=True)


@cache
def read(path):
    return [json.loads(line) for line in (REPO / path).read_text().splitlines() if line.strip()]


regtest = "crates/layertree-tee-lab/fixtures/events/regtest.jsonl"
light = "crates/layertree-tee-lab/fixtures/events/regtest-light.jsonl"
unroll = "crates/layertree-tee-lab/fixtures/events/regtest-two-level.jsonl"
simulated = "docs/verse/fixtures/layertree-transitions.jsonl"
errors = "docs/verse/fixtures/layertree-attestation-errors.jsonl"
plans = [("tree", regtest, lambda e: e.get("op") == "claim", "Regtest tree with its checked sums.")]
for op in ["join", "update", "claim", "exit", "refresh"]:
    plans.append((op, regtest, lambda e, op=op: e.get("op") == op, f"Confirmed regtest {op}."))
for op in ["evict", "split", "merge", "rewrite", "detach"]:
    plans.append((op, simulated, lambda e, op=op: e.get("op") == op,
                  f"Simulated {op}; illustrative hashes and transactions."))
plans += [
    ("rejected", simulated, lambda e: e["kind"] == "rejected", "Simulated refusal; balances do not change."),
    ("attested", light, lambda e: e["kind"] == "attestation", "Passed test-vendor attestation on regtest; no Confidential VM."),
    ("alarm", light, lambda e: e["seq"] == len(read(light)) - 1,
     "Mined regtest theft: output 0 is 50,000 sat short; all three followers report OutputMismatch."),
    ("unroll", unroll, lambda e: e["seq"] == len(read(unroll)) - 1,
     "Confirmed two-level regtest unroll; full member amounts, external anchor fees."),
]
for error in ["UnknownMeasurement", "ReportDataMismatch", "VendorSignature"]:
    plans.append((f"attestation-{error}", errors, lambda e, error=error: e.get("result") == error,
                  f"Simulated {error} display; not a hardware-verifier result."))

if args.signet:
    signet = "crates/layertree-tee-lab/fixtures/events/signet-normal.jsonl"
    negative = "crates/layertree-tee-lab/fixtures/events/signet-negative-policy.jsonl"
    plans = [("signet-tree", signet, lambda e: e["seq"] == len(read(signet)) - 1,
              "Confirmed signet tree: five members, 5,000 sats; Google-backed TDX verification.")]
    for op in ["join", "update", "exit", "refresh"]:
        plans.append((f"signet-{op}", signet, lambda e, op=op: e.get("op") == op,
                      f"Confirmed signet {op}; public transaction and exact amounts checked."))
    plans += [
        ("signet-attested", signet, lambda e: e["kind"] == "attestation" and e["height"] == 326599,
         "Captured signet output verified with Google-backed TDX, release/Rekor proofs, and pinned TLS."),
        ("signet-negative-policy", negative, lambda e: e["seq"] == len(read(negative)) - 1,
         "Negative verifier check: expected tree ID changed for a captured signet token; not a live service failure."),
    ]
if args.signet_halt:
    halted = "crates/layertree-tee-lab/fixtures/events/signet-halt-waiting.jsonl"
    plans = [("signet-halt-waiting", halted, lambda e: e["seq"] == len(read(halted)) - 1,
              "Guardian HALT observed at block 326605; signing refused; escape first valid at 326616; no payouts yet.")]
if args.signet_halt_paid:
    halted = "crates/layertree-tee-lab/fixtures/events/signet-halt-paid.jsonl"
    plans = [("signet-halt-paid", halted, lambda e: e["seq"] == len(read(halted)) - 1,
              "Guardian HALT recovery confirmed at signet block 326616: five exact 1,000-sat payouts; external fee 934 sats.")]
existing = output / "sources.json"
sources = json.loads(existing.read_text()) if existing.exists() else []
replacing = {name + ".jpg" for name, *_ in plans}
sources = [source for source in sources if source["image"] not in replacing]
for name, path, select, caption in plans:
    events = read(path)
    event = next(e for e in events if select(e))
    seq = event["seq"]
    png = args.scratch / f"{name}.png"
    subprocess.run([str(args.viewer.resolve()), "--events", str(REPO / path), "--at", str(seq),
                    "--size", "1920x1080", "--scale", "1.5", "--caption",
                    ("Signet guardian HALT; escape first valid at block 326616; test coins" if name == "signet-halt-waiting" else
                     "Negative check: captured signet token; expected tree ID changed" if name == "signet-negative-policy" else
                     f"Signet: confirmed block {event['height']}; test coins; Google-backed TDX" if event["network"] == "signet" else
                     "Regtest: test keys and coins; test-vendor attestation" if event["network"] == "regtest" else
                     "Simulation: illustrative data; no hardware attestation"), "--capture", str(png)], check=True, cwd=REPO)
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(png),
                    "-frames:v", "1", "-q:v", "2", "-pix_fmt", "yuvj444p", str(output / f"{name}.jpg")], check=True)
    txid = event.get("txid")
    if name == "signet-halt-paid":
        txid = next(e["txid"] for e in events if e["kind"] == "unroll_confirmed")
    if name == "signet-halt-waiting":
        txid = next(e["outpoint"].split(":")[0] for e in events if e["kind"] == "tree_open")
    if name == "alarm":
        txid = next(e["txid"] for e in reversed(events[:seq + 1]) if e["kind"] == "transition")
    if name == "unroll":
        txid = next(e["txid"] for e in reversed(events[:seq + 1]) if e["kind"] == "unroll_confirmed")
    sources.append(dict(image=f"{name}.jpg", log=path, sha256=hashlib.sha256((REPO / path).read_bytes()).hexdigest(),
                        seq=seq, network=event["network"], txid=txid, caption=caption))
(output / "sources.json").write_text(json.dumps(sources, indent=2) + "\n")
lines = ["# LayerTree captures", "", "These 1920×1080 stills come from the named logs and events. Regtest uses test",
         "keys, coins, and a test-vendor attestation key. Simulations are labelled in the",
         "scene and caption. Signet views use confirmed public transactions and historical",
         "Google-backed TDX evidence; the separate negative-policy view changes the expected",
         "tree ID and does not claim a live service failure.", "",
         "See [the viewer commands](../../../verse/layertree-visualizer.md) and",
         "[exact source hashes](sources.json). Re-render with `scripts/layertree-captures.py`.", ""]
for source in sources:
    lines += [f"## {source['image'][:-4].replace('-', ' ')}", "", f"![{source['caption']}]({source['image']})", "",
              source["caption"], "", f"Source: [{source['log']}](../../../../{source['log']}), event {source['seq']}."]
    if source["txid"]:
        lines += [f"{'Signet' if source['network']=='signet' else 'Regtest' if source['network']=='regtest' else 'Illustrative'} transaction: `{source['txid']}`."]
    lines += [""]
lines += ["Normal signet operations and separate guardian HALT recovery are captured above.", "The primary tree’s physical-loss recovery and signet alarm remain #11793/#11794", "measurements; the regtest images are not substitutes.", ""]
(output / "README.md").write_text("\n".join(lines))
if args.film_shot:
    args.clips.mkdir(parents=True, exist_ok=True)
    names = ["signet-halt-paid"] if args.signet_halt_paid else ["signet-halt-waiting"] if args.signet_halt else ["signet-tree", "signet-transitions"] if args.signet else ["tree", "transitions", "alarm", "unroll"]
    for name in names:
        shot = REPO / f"docs/verse/film/verse-shots/layertree-{name}.json"
        subprocess.run([str(args.film_shot.resolve()), str(shot), "--out", str(args.clips / f"layertree-{name}.mp4")],
                       check=True, cwd=REPO, env={**os.environ, "VERSE_QUALITY": "high"})
