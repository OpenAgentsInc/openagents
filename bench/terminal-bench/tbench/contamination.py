"""Run Coder One's contamination guard around a trial (issue #9590).

Facts learned from Terminal-Bench runs must never reach a live run's
instructions. ``coder-one contamination check`` (``crates/coder-one``)
compares the policy manifests, the guidance text, and a run's briefings
against the task ids, the verifier test names, and the task anatomy.
This module runs it on the host, never in the task container:

- ``static_check`` before a Coder One trial's setup, over the arm's
  policy manifest and the guidance in the checkout. A finding refuses the
  trial before any model is called.
- ``run_check`` after the trial, over the collected episode bundle. It
  records what it found and never fails the trial, which has already run.

Both write their report beside the agent's logs, and ``trial_summary``
folds them into the attempt record.

The checker is the first of these that answers ``contamination help``:
``CODER_ONE_CONTAMINATION_BIN``, the arm's pinned artifact when it runs
on this host, or ``cargo run -p coder-one`` in the checkout.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from tbench.coder_v05 import EpisodeContractError
from tbench.paths import PACKAGE_DIR

REPO_ROOT = PACKAGE_DIR.parent.parent
STATIC_RECORD = "contamination-static.json"
RUN_RECORD = "contamination-run.json"
BIN_ENV = "CODER_ONE_CONTAMINATION_BIN"
# A first `cargo run` may compile the crate.
RESOLVE_TIMEOUT_SEC = 900
CHECK_TIMEOUT_SEC = 300
CARGO = ("cargo", "run", "--quiet", "--package", "coder-one", "--bin", "coder-one", "--")

_resolved: dict[tuple[str, str], list[str]] = {}


class ContaminationError(EpisodeContractError):
    """The contamination guard found benchmark facts, or couldn't run."""


def _answers(argv: list[str]) -> bool:
    try:
        probe = subprocess.run(
            [*argv, "contamination", "help"],
            capture_output=True,
            text=True,
            timeout=RESOLVE_TIMEOUT_SEC,
            cwd=REPO_ROOT,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return probe.returncode == 0 and "contamination check" in probe.stdout


def checker(artifact: str | None = None) -> list[str]:
    """The command that runs ``coder-one`` with the contamination check."""
    explicit = os.environ.get(BIN_ENV, "")
    key = (explicit, artifact or "")
    if key in _resolved:
        return _resolved[key]
    candidates: list[list[str]] = []
    if explicit:
        candidates.append([explicit])
    else:
        if artifact and Path(artifact).expanduser().is_file():
            candidates.append([str(Path(artifact).expanduser())])
        if shutil.which("cargo"):
            candidates.append(list(CARGO))
    for argv in candidates:
        if _answers(argv):
            _resolved[key] = argv
            return argv
    raise ContaminationError(
        "no contamination checker runs on this host: "
        + (
            f"{BIN_ENV}={explicit} doesn't answer `contamination help`"
            if explicit
            else "the arm's artifact predates `coder-one contamination` or doesn't run "
            "here, and `cargo run -p coder-one` failed"
        )
        + f"; set {BIN_ENV} to a host build of coder-one"
    )


def _check(argv: list[str], args: list[str]) -> dict[str, Any]:
    try:
        done = subprocess.run(
            [*argv, "contamination", "check", "--root", str(REPO_ROOT), "--json", *args],
            capture_output=True,
            text=True,
            timeout=CHECK_TIMEOUT_SEC,
            cwd=REPO_ROOT,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ContaminationError(f"the contamination check didn't finish: {exc}") from exc
    if done.returncode not in (0, 1):
        raise ContaminationError(
            f"the contamination check exited {done.returncode}: "
            f"{(done.stderr or done.stdout).strip()[-500:]}"
        )
    try:
        report = json.loads(done.stdout)
    except ValueError as exc:
        raise ContaminationError(f"the contamination check printed no report: {exc}") from exc
    if not isinstance(report, dict) or "clean" not in report:
        raise ContaminationError("the contamination check printed no report")
    return report


def describe(report: dict[str, Any], limit: int = 5) -> str:
    """The first findings, one clause each."""
    findings = report.get("findings") or []
    clauses = [
        f"{f.get('kind')} {f.get('matched')!r} at {f.get('location')}"
        for f in findings[:limit]
    ]
    more = len(findings) - len(clauses)
    return "; ".join(clauses) + (f"; and {more} more" if more > 0 else "")


def _write(path: Path, body: dict[str, Any]) -> str | None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(body, indent=2) + "\n")
    except OSError as exc:
        return f"cannot retain contamination record {path}: {exc}"
    return None


def static_check(
    logs_dir: Path, *, policy: Path | None, artifact: str | None = None
) -> dict[str, Any]:
    """Refuse the trial when its manifest or the guidance holds benchmark
    facts. Without a manifest, only the guidance is checked."""
    args = ["--policy", str(policy)] if policy else ["--no-policies"]
    try:
        report = _check(checker(artifact), args)
    except ContaminationError as exc:
        _write(Path(logs_dir) / STATIC_RECORD, {"clean": None, "error": str(exc)})
        raise
    record_error = _write(Path(logs_dir) / STATIC_RECORD, report)
    if record_error:
        raise ContaminationError(record_error)
    if not report["clean"]:
        raise ContaminationError(
            f"the contamination check refused this trial: {describe(report)}; "
            f"the report is {Path(logs_dir) / STATIC_RECORD}"
        )
    return report


def run_check(
    logs_dir: Path, *, instruction: str | None, artifact: str | None = None
) -> dict[str, Any]:
    """Check the run's briefings and record the result; never raises."""
    bundle = Path(logs_dir) / "episode"
    if not (bundle / "episode.atif.jsonl").is_file():
        body = {"clean": None, "error": f"no episode log under {bundle}"}
        record_error = _write(Path(logs_dir) / RUN_RECORD, body)
        if record_error:
            body["record_error"] = record_error
        return body
    try:
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as handle:
            handle.write(instruction or "")
            instruction_file = handle.name
        try:
            args = ["--run", str(bundle)]
            if instruction:
                args += ["--instruction", instruction_file]
            body = _check(checker(artifact), args)
        finally:
            os.unlink(instruction_file)
    except (ContaminationError, OSError) as exc:
        body = {"clean": None, "error": str(exc)}
    record_error = _write(Path(logs_dir) / RUN_RECORD, body)
    if record_error:
        return {"clean": None, "error": record_error, "unretained_result": body}
    return body


def _summary(path: Path) -> dict[str, Any] | None:
    try:
        body = json.loads(path.read_text())
    except (OSError, ValueError):
        return None
    findings = body.get("findings") or []
    return {
        "clean": body.get("clean"),
        "findings": len(findings),
        "first": describe(body, limit=3) if findings else None,
        "error": body.get("error"),
    }


def trial_summary(trial_dir: Path) -> dict[str, Any]:
    """Both checks' results for a trial's attempt record; ``None`` for a
    check that didn't run, as on an arm other than Coder One."""
    agent = Path(trial_dir) / "agent"
    return {
        "static": _summary(agent / STATIC_RECORD),
        "run": _summary(agent / RUN_RECORD),
    }
