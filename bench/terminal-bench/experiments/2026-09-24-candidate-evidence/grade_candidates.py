"""Grade retained candidates after a trial ends, without another model call.

Run in the frozen experiment's tbench environment on the execution host.
Results stay outside the agent's workspace and never enter its briefing.
"""

import argparse
import json
import tempfile
from pathlib import Path

from tbench import replay


def grade(root, output):
    output.mkdir(parents=True, exist_ok=True)
    for job in sorted((root / "jobs").glob("tb4*--candidate-evidence-9607-r*")):
        for result_path in sorted(job.glob("*/result.json")):
            result = json.loads(result_path.read_text())
            if not result.get("finished_at") or result.get("verifier_result") is None:
                continue
            trial = result_path.parent
            for selection in sorted(trial.glob("agent/episode/artifacts/lean-*/selection.json")):
                moves = json.loads(selection.read_text())
                for move in moves:
                    if not move.get("candidate") or move.get("snapshot_error"):
                        continue
                    candidate = selection.parent / Path(move["candidate"]).name
                    if not candidate.is_dir():
                        raise ValueError(f"Missing retained candidate: {candidate}")
                    dest = output / job.name / candidate.name
                    record_path = dest / "grade.json"
                    if record_path.exists():
                        continue
                    with tempfile.TemporaryDirectory(prefix="candidate-9607-grade-") as scratch:
                        workspace = replay.candidate_workspace(candidate, replay.workdir_of(trial), Path(scratch))
                        graded = replay.run_verifier(replay.task_dir(trial), workspace, dest)
                    record = {"job": job.name, "trial": trial.name,
                              "candidate": str(candidate.relative_to(trial)),
                              "workspace_files": move.get("workspace_files"),
                              "self_score": move.get("score"), "kept": move.get("kept"),
                              "grade": graded}
                    record_path.write_text(json.dumps(record, indent=2) + "\n")
                    print(json.dumps({"job": job.name, "candidate": candidate.name,
                                      "reward": graded["reward"], "exception": graded["exception"]}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-root", type=Path, default=Path.home() / ".openagents/terminal-bench")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    grade(args.state_root, args.output)
