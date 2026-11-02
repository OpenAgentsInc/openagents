#!/usr/bin/env python3
"""Scores Coder's engine decision (route.coax, book chapter 13) on the
labelled set route-labels.tsv: each message goes through `coder route
--json` and the engine it picks is compared with the label. Prints each
miss, then the accuracy for the half asked and the mean probability of the
picks.

    python3 coder/tui/route_eval.py            # the held-out half
    python3 coder/tui/route_eval.py train      # the tuning half

The labels came from the desktop Lab's router (apps/openagents-desktop/
router-labels.tsv), with claude_code as claude and other as micro. Live:
each message is one System One call on mammoth-1.

Test-only harness; the program under test is Coax.
"""
import csv, json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
split = sys.argv[1] if len(sys.argv) > 1 else "held"
rows = [r for r in csv.DictReader(open(os.path.join(HERE, "route-labels.tsv")), delimiter="\t") if r["split"] == split]
right, probs = 0, []
for r in rows:
    p = subprocess.run([os.path.join(HERE, "coder.sh"), "route", r["message"], "--json"], cwd=REPO, capture_output=True, text=True, timeout=300)
    try:
        d = json.loads(p.stdout.strip().splitlines()[-1])
    except (IndexError, ValueError):
        d = {"ok": False, "text": p.stderr.strip()}
    if d.get("ok") and d["engine"] == r["label"]:
        right += 1
        probs.append(d["probability"])
    else:
        print("miss  %-6s %-30s %s" % (r["label"], json.dumps(d.get("engine", d.get("text", ""))), r["message"]))
print("%s: %d/%d right on mammoth-1, mean probability of the right picks %.2f" % (split, right, len(rows), sum(probs) / len(probs) if probs else 0))
sys.exit(0 if right == len(rows) else 1)
