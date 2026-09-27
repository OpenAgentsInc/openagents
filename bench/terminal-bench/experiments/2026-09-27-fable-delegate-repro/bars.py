"""Bars and delegate deadlines for the #9776 reproduction, frozen before any run.

For each task in the issue's order: Fable 5.1 low's cheapest and fastest
winning runs from ``fable_reference()`` in
``bench/terminal-bench/studies/2026-09-26-out-of-sample/study.py``, and the
delegate deadline, ``floor(fastest-win seconds - 35)``.

Run from ``bench/terminal-bench``::

    python3 experiments/2026-09-27-fable-delegate-repro/bars.py > experiments/2026-09-27-fable-delegate-repro/tasks.json
"""

import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent.parent / "studies/2026-09-26-out-of-sample"))
import study  # noqa: E402

TASKS = [
    "batched-eval-parity", "coq-block-bound", "distributed-dedup", "embedding-drift-monitor",
    "gsea-proteomics", "hof-topology-interpenetration", "interleaved-vigenere", "math-eval-grader",
    "mp-checkpoint-consolidation", "production-planning", "risk-scorer-replay", "shadow-relay",
    "sound-change-cascade", "telecom-entity-resolution",
]
OVERHEAD_SEC = 35


def main() -> None:
    ref = study.fable_reference()
    doc = json.loads(Path(study.REPLAYS).read_text())
    name = {t["id"]: t["trial_name"] for t in doc["trials"]}
    rows = []
    for order, task in enumerate(TASKS, 1):
        r = ref[task]
        cheap, fast = r["cheapest"], r["fastest"]
        rows.append({
            "order": order,
            "task": task,
            "fable_low_passes": r["passes"],
            "fable_low_attempts": r["attempts"],
            "cheapest_win": {"id": cheap["id"], "trial": name[cheap["id"]],
                             "cost_usd": cheap["cost_usd"], "seconds": round(cheap["seconds"], 1)},
            "fastest_win": {"id": fast["id"], "trial": name[fast["id"]],
                            "cost_usd": fast["cost_usd"], "seconds": round(fast["seconds"], 1)},
            "bar_cost_usd": cheap["cost_usd"],
            "bar_seconds": fast["seconds"],
            "delegate_deadline_sec": math.floor(fast["seconds"] - OVERHEAD_SEC),
        })
    json.dump({
        "schema": "openagents.tb.fable_delegate_repro.tasks.v1",
        "issue": 9776,
        "rule": "bar_cost_usd is Fable 5.1 low's cheapest win; bar_seconds is its fastest win, whole trial; "
                "delegate_deadline_sec is floor(fastest-win seconds - 35).",
        "tasks": rows,
    }, sys.stdout, indent=1)
    print()


if __name__ == "__main__":
    main()
