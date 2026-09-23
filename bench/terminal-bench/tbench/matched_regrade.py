"""Grade the first plain candidate after its session-collection failure.

This copies the original, unchanged collected artifact directory into a fresh
instance of the same task. It makes no model call and does not solve the task.
The original attempt and its exception remain untouched.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from harbor.agents.base import BaseAgent


def digests(directory: Path) -> dict[str, str]:
    return {str(p.relative_to(directory)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob("*")) if p.is_file()}


class ReplayBatchedArtifacts(BaseAgent):
    @staticmethod
    def name() -> str:
        return "matched-artifact-regrade"

    def version(self) -> str:
        return "1"

    def __init__(self, *args, source: str, expected: dict, **kwargs):
        super().__init__(*args, **kwargs)
        self.source = Path(source)
        self.expected = expected
        if not expected or digests(self.source) != expected:
            raise ValueError("The original candidate artifacts changed or are missing")

    async def setup(self, environment):
        pass

    async def run(self, instruction, environment, context):
        await environment.exec(command="rm -rf /app/evalbench && mkdir -p /app/evalbench")
        await environment.upload_dir(self.source, "/app/evalbench")
        command = """python3 - <<'PY'
from pathlib import Path
import hashlib,json
r=Path('/app/evalbench')
print(json.dumps({str(p.relative_to(r)):hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in sorted(r.rglob('*')) if p.is_file()},sort_keys=True))
PY"""
        checked = await environment.exec(command=command)
        if checked.return_code != 0 or json.loads(checked.stdout) != self.expected:
            raise ValueError("The replayed candidate differs from the original artifacts")
        (self.logs_dir / "artifact-replay.txt").write_text(json.dumps({
            "source": str(self.source), "verified_sha256": self.expected,
            "model_calls": 0, "candidate_edits": 0}, indent=2) + "\n")
        context.cost_usd = 0.0
        context.n_input_tokens = context.n_output_tokens = context.n_cache_tokens = 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("original_job", type=Path)
    args = parser.parse_args()
    trials = list(args.original_job.glob("*/result.json"))
    if len(trials) != 1:
        raise ValueError("Expected exactly one original trial")
    original = trials[0].parent
    source = original / "artifacts/app/evalbench"
    config = json.loads((args.original_job / "tbench/job-config.json").read_text())
    job = args.original_job.name + "-regrade"
    config["job_name"] = job
    config["agents"] = [{"import_path": "tbench.matched_regrade:ReplayBatchedArtifacts",
                         "kwargs": {"source": str(source), "expected": digests(source)}}]
    destination = args.original_job.parent.parent / "experiments/matched-20260923"
    path = destination / "regrade-config.json"
    path.write_text(json.dumps(config, indent=2) + "\n")
    subprocess.run(["harbor", "run", "--config", str(path), "--jobs-dir",
                    str(args.original_job.parent), "--job-name", job, "--yes"], check=True)


if __name__ == "__main__":
    main()
