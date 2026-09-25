#!/usr/bin/env python3
"""Replays the #9658 failure-localization measurement from the retained logs.

It runs `coder-one component replay evidence.localize` over the protocol's
roots with the eleven mapped tasks excluded, then checks that every row,
the summary, and the pinned sources match the retained records. It runs no
command from the logs and asks no model.

    python3 bench/terminal-bench/experiments/2026-09-25-failure-localization/replay.py \
        --coder-one /absolute/path/to/coder-one
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../../../.."))
HOME = os.path.expanduser("~")

ROOTS = [
    os.path.join(HOME, ".openagents/terminal-bench/jobs"),
    os.path.join(HOME, ".openagents/terminal-bench/microluna-jobs-9585"),
    # Relative to the checkout, as the retained sources name it.
    "bench/terminal-bench/traces",
]

# The tasks the Fable pattern map read; the pattern was learned from them.
EXCLUDED = [
    "embedding-drift-monitor",
    "coq-block-bound",
    "shadow-relay",
    "risk-scorer-replay",
    "mp-checkpoint-consolidation",
    "payments-pipeline-fix",
    "telecom-entity-resolution",
    "fp8-rmsnorm-gemm",
    "distributed-dedup",
    "intrastat-meldung",
    "photonic-waveguide-routing",
]


def rows(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--coder-one", required=True)
    a = ap.parse_args()
    records = os.path.join(HERE, "records")
    with tempfile.TemporaryDirectory() as out:
        command = [a.coder_one, "component", "replay", "evidence.localize"]
        for root in ROOTS:
            command += ["--traces", root]
        for task in EXCLUDED:
            command += ["--exclude-task", task]
        command += ["--out", out]
        subprocess.run(command, check=True, stdout=subprocess.DEVNULL, cwd=REPO)
        same_rows = rows(os.path.join(out, "rows.jsonl")) == rows(os.path.join(records, "rows.jsonl"))
        same_summary = json.load(open(os.path.join(out, "summary.json"))) == json.load(
            open(os.path.join(records, "summary.json"))
        )
        same_sources = json.load(open(os.path.join(out, "sources.json"))) == json.load(
            open(os.path.join(records, "sources.json"))
        )
        print(f"rows identical: {same_rows}")
        print(f"summary identical: {same_summary}")
        print(f"sources identical: {same_sources}")
        sys.exit(0 if same_rows and same_summary and same_sources else 1)


if __name__ == "__main__":
    main()
