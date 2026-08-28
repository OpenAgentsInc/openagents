"""Write candidate artifacts and the optimizer run envelope.

Writes only under the run's output directory. Never lands on `surfaces/coder`.
"""

from __future__ import annotations

import json
from difflib import unified_diff
from pathlib import Path
from typing import Any

from .candidate import parse_candidate
from .metric import DEV_SUITE_ID, HOLDOUT_SUITE_ID, JobScore
from .surfaces import SURFACE_IDS, surface_filename


def surface_diffs(seed: dict[str, str], candidate: dict[str, str]) -> list[dict[str, str]]:
    diffs: list[dict[str, str]] = []
    for surface in SURFACE_IDS:
        before = seed.get(surface, "")
        after = candidate.get(surface, before)
        if after == before:
            continue
        filename = f"surfaces/coder/{surface_filename(surface)}"
        diff = "".join(
            unified_diff(
                before.splitlines(keepends=True),
                after.splitlines(keepends=True),
                fromfile=f"a/{filename}",
                tofile=f"b/{filename}",
            )
        )
        diffs.append({"surface": surface, "diff": diff or after})
    return diffs


def acceptance_argument(*, suite: str, trials: int, harbor: str) -> dict[str, Any]:
    """Ledger O6: promotion gate names trial count and floor.

    On `tb2-quick` the available rates are 0, .5, and 1, so the floor is
    structural, not statistical (ledger M5).
    """
    floor = 0.5 if suite == DEV_SUITE_ID else 1.0 / max(1, trials)
    return {
        "statement": (
            f"Promote only if the candidate beats the incumbent by ≥{floor} "
            f"over K={trials} trials on {suite}."
        ),
        "suite": suite,
        "trials": trials,
        "floor": floor,
        "floorKind": "structural" if suite == DEV_SUITE_ID else "stated",
        "holdout": HOLDOUT_SUITE_ID,
        "holdoutRole": "confirm survivors; never screen",
        "harbor": harbor,
        "note": (
            "Dev-set narrowness: tb2-quick's two tasks admit rates of 0, .5, and 1 only. "
            "A candidate that improves the dev set but not the holdout is reported, not promoted."
        ),
    }


def build_candidate(
    *,
    seed: dict[str, str],
    proposed: dict[str, str],
    job_score: JobScore,
    produced_by: str,
    transfer_label: dict[str, str],
    parent: str | None,
    summary: str,
    risk: str,
    extra_evidence: list[dict[str, str]] | None = None,
) -> dict[str, Any]:
    surfaces = surface_diffs(seed, proposed)
    evidence = list(extra_evidence or [])
    for trial in job_score.trials:
        evidence.append(
            {
                "ref": f"trial:{trial.task}#outcome",
                "note": (
                    f"accepted={trial.accepted} ungraded={trial.ungraded} "
                    f"score={trial.score:.6f} tokens={trial.tokens} "
                    f"wall_seconds={trial.wall_seconds}"
                ),
            }
        )
        evidence.append(
            {
                "ref": f"trial:{trial.task}#step-1",
                "note": "ATIF trajectory was present on the fixture/job used as the metric.",
            }
        )
    evidence.append(
        {
            "ref": "ledger:O1",
            "note": "This object is a candidate, not a deployment.",
        }
    )
    evidence.append(
        {
            "ref": "ledger:O4",
            "note": (
                "Objective is acceptance minus 0.005×tokens_per_million minus "
                "0.001×wall_hours; unknown USD is not zero."
            ),
        }
    )
    evidence.append(
        {
            "ref": "ledger:O3",
            "note": f"Screened on {job_score.suite}; {HOLDOUT_SUITE_ID} was not used as valset.",
        }
    )
    evidence.append(
        {
            "ref": "ledger:O6",
            "note": acceptance_argument(
                suite=job_score.suite,
                trials=max(1, job_score.metric_calls),
                harbor="fixture",
            )["statement"],
        }
    )
    raw = {
        "schema": "openagents.coder_candidate.v1",
        "lever": {"axis": "optimizer", "summary": summary},
        "surfaces": surfaces,
        "lineage": {"origin": "optimizer", "parent": parent, "producedBy": produced_by},
        "transferLabel": transfer_label,
        "evidence": evidence,
        "risk": risk,
        "verification": {
            "suite": job_score.suite,
            "metric": "objectiveScore",
            "expectedDirection": "up",
        },
    }
    return parse_candidate(raw)


def write_run(
    output_dir: Path,
    *,
    run: dict[str, Any],
    candidates: list[dict[str, Any]],
) -> Path:
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    candidates_dir = output_dir / "candidates"
    candidates_dir.mkdir(exist_ok=True)
    written_ids: list[str] = []
    for candidate in candidates:
        parsed = parse_candidate(candidate)
        path = candidates_dir / f"{parsed['candidateId'].replace(':', '-')}.json"
        path.write_text(json.dumps(parsed, indent=2, ensure_ascii=False) + "\n")
        written_ids.append(parsed["candidateId"])
    envelope = dict(run)
    envelope["candidates"] = written_ids
    envelope_path = output_dir / "run.json"
    envelope_path.write_text(json.dumps(envelope, indent=2, ensure_ascii=False) + "\n")
    return envelope_path
