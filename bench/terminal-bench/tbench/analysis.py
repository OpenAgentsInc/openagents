"""Write each finished trial's analysis when a job ends.

The Gym computes the analysis: ``gym runs analyze RUN --write`` reads a
trial's records and keeps ``analysis.md`` and ``analysis.json`` in the
trial directory (issue #9593). This module only calls it, once per
finished trial that has no analysis yet, after ``collect`` writes the
attempt records.

It never fails a run. A missing ``gym`` binary, a timeout, or an error
is a warning on stderr, and the trial stays without an analysis until
someone runs ``gym runs analyze`` by hand. Set ``TBENCH_ANALYZE=off`` to
skip it, and ``GYM_BIN`` to name the ``gym`` binary; without it, the
module uses ``gym`` on ``PATH``, then ``cargo run -p gym`` in this
checkout.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

from tbench.paths import PACKAGE_DIR

REPO_ROOT = PACKAGE_DIR.parent.parent
BIN_ENV = "GYM_BIN"
OFF_ENV = "TBENCH_ANALYZE"
MARKDOWN = "analysis.md"
# A first `cargo run` may compile the crate.
TIMEOUT_SEC = 900
CARGO = ("cargo", "run", "--quiet", "--package", "gym", "--bin", "gym", "--")


def command() -> list[str] | None:
    """The command that runs ``gym``, or None when none is found."""
    explicit = os.environ.get(BIN_ENV, "").strip()
    if explicit:
        return [explicit]
    found = shutil.which("gym")
    if found:
        return [found]
    if shutil.which("cargo") and (REPO_ROOT / "crates" / "gym").is_dir():
        return list(CARGO)
    return None


def pending(job_dir: Path) -> list[Path]:
    """The job's finished trials that have no analysis yet."""
    if not job_dir.is_dir():
        return []
    return [
        trial
        for trial in sorted(job_dir.iterdir())
        if trial.is_dir()
        and (trial / "config.json").is_file()
        and (trial / "result.json").is_file()
        and not (trial / MARKDOWN).exists()
    ]


def _warn(message: str) -> None:
    print(f"tbench: no analysis written: {message}", file=sys.stderr)


def analyze_job(
    job_dir: Path, *, runner: Callable[..., Any] = subprocess.run
) -> list[Path]:
    """Runs ``gym runs analyze --write`` for each pending trial.

    Returns the analyses written. Never raises.
    """
    if os.environ.get(OFF_ENV, "").strip().lower() in {"off", "0", "false", "no"}:
        return []
    trials = pending(job_dir)
    if not trials:
        return []
    argv = command()
    if argv is None:
        _warn(f"no gym binary; set {BIN_ENV} or run `gym runs analyze` by hand")
        return []
    written: list[Path] = []
    for trial in trials:
        try:
            done = runner(
                [
                    *argv,
                    "runs",
                    "analyze",
                    f"{job_dir.name}/{trial.name}",
                    "--jobs-dir",
                    str(job_dir.parent),
                    "--no-traces",
                    "--write",
                ],
                capture_output=True,
                text=True,
                timeout=TIMEOUT_SEC,
                cwd=REPO_ROOT,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            _warn(f"{trial.name}: {exc}")
            continue
        if done.returncode != 0:
            detail = (done.stderr or done.stdout or "").strip()[-300:]
            _warn(f"{trial.name}: gym exited {done.returncode}: {detail}")
            continue
        written.append(trial / MARKDOWN)
    return written
