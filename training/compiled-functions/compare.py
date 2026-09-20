"""Pairs a compiled-function run against the doors in the Gym result store.

Reads `crates/gym/results/support-v2-three-way.jsonl`, keeps the `routing`
rows of the `development` split, and for each door counts the items it got
right that the compiled function missed and the reverse. The exact McNemar
p-value on those two counts says whether the difference is more than noise.

    .venv/bin/python compare.py runs/prose.json runs/with-examples.json
"""

from __future__ import annotations

import json
import sys
from math import comb
from pathlib import Path

HERE = Path(__file__).resolve().parent
RESULTS = HERE / "../../crates/gym/results/support-v2-three-way.jsonl"
DOORS = ["lev-adapted@1", "lev-base", "jev (hosted)", "baseline-bge"]


def mcnemar_exact(a: int, b: int) -> float:
    n = a + b
    if n == 0:
        return 1.0
    k = min(a, b)
    tail = sum(comb(n, i) for i in range(k + 1)) / 2**n
    return min(1.0, 2 * tail)


def main(paths: list[str]) -> None:
    rows = [json.loads(line) for line in RESULTS.read_text().splitlines() if line]
    rows = [r for r in rows if r["family"] == "routing" and r["split"] == "development"]
    digests = {r["suite_digest"] for r in rows}
    print(f"store: {RESULTS.name} digest {', '.join(sorted(digests))}")
    for path in paths:
        run = json.loads(Path(path).read_text())
        mine = {i["id"]: i["correct"] for i in run["items"]}
        print(f"\n{path} ({run['spec_kind']})")
        for door in DOORS:
            door_rows = {r["item_id"]: r["correct"] for r in rows if r["door"] == door}
            shared = sorted(set(door_rows) & set(mine))
            door_right = sum(door_rows[i] for i in shared)
            paw_right = sum(mine[i] for i in shared)
            a = sum(door_rows[i] and not mine[i] for i in shared)
            b = sum(mine[i] and not door_rows[i] for i in shared)
            print(
                f"  {door:<14} n={len(shared)} door {door_right}/{len(shared)}"
                f" paw {paw_right}/{len(shared)} door-only {a} paw-only {b}"
                f" p={mcnemar_exact(a, b):.2f}"
            )


if __name__ == "__main__":
    main(sys.argv[1:] or ["runs/prose.json", "runs/with-examples.json"])
