#!/usr/bin/env python3
"""Self-check for bench/replay_weka.py.

Run with the Python that ships the monorepo bench scripts:

    python3 bench/test_replay_weka.py
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import List

_BENCH = Path(__file__).resolve().parent
_TOOL = _BENCH / "replay_weka.py"
_FIXTURE = _BENCH / "fixtures" / "weka-trace-v1-sample.json"


def _run(args: List[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(_TOOL), "--json", *args],
        capture_output=True,
        text=True,
        cwd=str(_BENCH.parent),
    )


def test_sample_reuse_ratio() -> None:
    proc = _run([str(_FIXTURE)])
    if proc.returncode != 0:
        raise AssertionError(f"tool failed: {proc.stderr}")
    data = json.loads(proc.stdout)
    corpus = data["corpus"]
    assert corpus["turns"] == 3
    assert corpus["total_blocks"] == 12
    assert corpus["repeated_blocks"] == 6
    assert corpus["prefix_reuse_ratio"] == 0.5
    assert corpus["context_growth"] == 6


def test_malformed_and_empty_do_not_block_valid() -> None:
    with tempfile.TemporaryDirectory() as d:
        empty = Path(d) / "empty.json"
        empty.write_text("")
        bad = Path(d) / "bad.json"
        bad.write_text("not json")
        proc = _run([str(empty), str(bad), str(_FIXTURE)])
        assert proc.returncode == 1, f"expected exit 1, got {proc.returncode}"
        data = json.loads(proc.stdout)
        errors = [f for f in data["files"] if "error" in f]
        assert len(errors) == 2
        corpus = data["corpus"]
        assert corpus["turns"] == 3
        assert corpus["total_blocks"] == 12
        assert corpus["prefix_reuse_ratio"] == 0.5


if __name__ == "__main__":
    failures = 0
    for test in (test_sample_reuse_ratio, test_malformed_and_empty_do_not_block_valid):
        name = test.__name__
        try:
            test()
            print(f"{name}: PASS")
        except Exception as exc:
            print(f"{name}: FAIL {exc}")
            failures += 1
    raise SystemExit(1 if failures else 0)
