"""Retain and grade this frozen suite only after all six attempts finish."""
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

from tbench import candidates
from tbench.retain import known_credentials, retain_jobs, scan_for_credentials


def main():
    root = Path.home() / ".openagents/terminal-bench"
    out = root / "experiments/iteration-speed-9618"
    status_path = root / "suites/tb4--coder-one-microluna-v13-retained/status.json"
    deadline = time.monotonic() + 3600
    while time.monotonic() < deadline:
        status = json.loads(status_path.read_text())
        rows = status["trials"]
        assert len(rows) == 6, "The declared six-attempt suite changed"
        if all(row["state"] == "finished" for row in rows):
            break
        if any(row["state"] in ("failed", "skipped", "blocked") for row in rows):
            raise RuntimeError("A trial needs attention; no partial batch is silently accepted")
        time.sleep(15)
    else:
        raise RuntimeError("The suite did not finish within the observation bound")
    shutil.copy2(status_path, out / "status.json")
    shutil.copy2(status_path.with_name("scheduler.log"), out / "scheduler.log")
    subprocess.run([sys.executable, str(Path(__file__).with_name("collect.py")),
                    "--output", str(out / "results.json")], check=True)
    trials = []
    for row in rows:
        found = list((root / "jobs" / row["job"]).glob("*/result.json"))
        if len(found) != 1:
            raise RuntimeError(f"Expected one completed trial for {row['job']}")
        trials.append(found[0].parent)
    credentials = known_credentials()
    retained, errors = retain_jobs([row["job"] for row in rows], jobs_dir=root / "jobs",
                                  traces_dir=out / "retained", credentials=credentials,
                                  max_file_bytes=32 * 1024 * 1024,
                                  max_trial_bytes=256 * 1024 * 1024)
    record = {"errors": errors, "trials": [{"job": item.job, "trial": item.trial,
               "missing": item.missing, "bytes": item.retained_bytes,
               "destination": str(item.destination)} for item in retained]}
    (out / "retention-summary.json").write_text(json.dumps(record, indent=2) + "\n")
    if errors or any(item.missing for item in retained):
        raise RuntimeError("Retained evidence is incomplete; inspect retention-summary.json")
    result = candidates.batch(trials, out / "grading-fresh", jobs=2, deduplicate=True)
    scan = {"credentials_checked": sorted(credentials),
            "matches": scan_for_credentials(out / "grading-fresh", credentials)}
    (out / "fresh-grading-credential-scan.json").write_text(json.dumps(scan, indent=2) + "\n")
    print(json.dumps({key: result[key] for key in ("wall_seconds", "verifier_executions",
                      "reused_grades", "invalid_candidates", "errors", "oracle")}), flush=True)
    if scan["matches"] or result["errors"] or result["invalid_candidates"]:
        raise RuntimeError("Grading evidence needs inspection before publication")


if __name__ == "__main__":
    main()
