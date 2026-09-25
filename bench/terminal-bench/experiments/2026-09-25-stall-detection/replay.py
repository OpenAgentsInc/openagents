#!/usr/bin/env python3
"""Replays the #9627 stall measurement from retained inputs, with no Jev call.

It runs `coder-one component replay control.stall --inputs ... --jev
recorded` over both partitions, checks that every row matches the retained
rows, reruns measure.py, and checks the summary matches too.

    python3 bench/terminal-bench/experiments/2026-09-25-stall-detection/replay.py \
        --coder-one /absolute/path/to/coder-one
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


def rows(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--coder-one", required=True)
    a = ap.parse_args()
    with tempfile.TemporaryDirectory() as out:
        subprocess.run([
            a.coder_one, "component", "replay", "control.stall",
            "--inputs", os.path.join(HERE, "records/inputs.jsonl"),
            "--split", os.path.join(HERE, "split.json"),
            "--recorded", os.path.join(HERE, "jev-recorded.json"),
            "--jev", "recorded",
            "--partition", "calibration", "--partition", "evaluation",
            "--out", out,
        ], check=True, stdout=subprocess.DEVNULL)
        replayed = json.load(open(os.path.join(out, "replayed.json")))
        misses = replayed["jev"].get("miss", 0)
        got, want = rows(os.path.join(out, "rows.jsonl")), rows(os.path.join(HERE, "records/rows.jsonl"))
        same = got == want
        print(f"{len(got)} rows replayed, {misses} recorded misses, rows identical: {same}")
        summary = os.path.join(out, "summary.json")
        subprocess.run([
            sys.executable, os.path.join(HERE, "measure.py"),
            "--rows", os.path.join(out, "rows.jsonl"),
            "--labels", os.path.join(HERE, "records/labels-calibration.jsonl"),
            os.path.join(HERE, "records/labels-evaluation.jsonl"),
            "--params", os.path.join(HERE, "selection.json"),
            "--out", summary,
        ], check=True, stdout=subprocess.DEVNULL)
        same_summary = json.load(open(summary)) == json.load(open(os.path.join(HERE, "records/summary.json")))
        print(f"summary identical: {same_summary}")
        sys.exit(0 if same and same_summary and misses == 0 else 1)


if __name__ == "__main__":
    main()
