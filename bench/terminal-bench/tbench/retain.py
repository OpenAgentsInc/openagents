"""Copy one trial's full evidence closure into the repository.

``tbench retain <job>...`` reads each trial of a local Harbor job and
writes the retained layout the Gym reads under
``bench/terminal-bench/traces/<job>/``:

* ``<trial>.json`` is Harbor's normalized ATIF trajectory.
* ``<trial>.episode/`` mirrors the agent's episode directory (the Coder One
  manifest, the raw ATIF, usage, state, briefings, and the native delegate
  streams), plus ``harbor-result.json`` (a trimmed trial result),
  ``verifier/`` (reward, CTRF report, and per-test output), ``native/``
  (other executors' native traces), ``produced/`` (the files Harbor
  collected from the task container), and ``retention.json``.

``retention.json`` records every copied file with its digest, checks each
digest the episode manifest recorded, names every reference that is
missing or over the size bound, notes how the raw ATIF relates to the
normalized one, and records the credential scan. A trial is written only
when the scan finds no credential value; the scan never prints a value.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import tempfile
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from . import paths

RETENTION_SCHEMA = "openagents.tbench.retention.v1"
EPISODE_CONTRACT = "openagents.coder.episode.v1"
DEFAULT_TRACES_DIR = paths.PACKAGE_DIR / "traces"

# A single retained file above this bound is reported, not copied.
MAX_FILE_BYTES = 4 * 1024 * 1024
# A trial whose retained files would exceed this total keeps its first
# files in closure order and reports the rest.
MAX_TRIAL_BYTES = 16 * 1024 * 1024
# Credential values shorter than this are too generic to scan for.
MIN_SECRET_CHARS = 16

HARBOR_RESULT_KEYS = (
    "trial_name",
    "task_name",
    "started_at",
    "finished_at",
    "environment_setup",
    "agent_setup",
    "agent_execution",
    "verifier",
    "verifier_result",
    "agent_result",
    "exception_info",
)


class RetentionError(RuntimeError):
    """A trial that can't be retained, such as one holding a credential."""


@dataclass
class TrialRetention:
    """What one trial's retention copied, reported, and scanned."""

    job: str
    trial: str
    record: dict[str, Any]
    written: bool
    destination: Path

    @property
    def missing(self) -> list[dict[str, Any]]:
        return self.record["missing"]

    @property
    def retained_bytes(self) -> int:
        return sum(f["bytes"] for f in self.record["files"])


@dataclass
class _Plan:
    """The closure of one trial, before it is copied."""

    files: list[tuple[str, Path, str, str | None]] = field(default_factory=list)
    missing: list[dict[str, Any]] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)
    seen: set[str] = field(default_factory=set)

    def add(
        self,
        kind: str,
        source: Path,
        target: str,
        expected: str | None = None,
        reference: str | None = None,
    ) -> None:
        if target in self.seen:
            return
        if not source.is_file():
            self.missing.append(
                {
                    "kind": kind,
                    "reference": reference or target,
                    "reason": "referenced but not present in the job directory",
                }
            )
            return
        self.seen.add(target)
        self.files.append((kind, source, target, expected))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def known_credentials(home: Path | None = None) -> dict[str, str]:
    """Credential values to scan for, keyed by where each came from.

    Reads the environment and the local credential files. Values stay in
    memory; callers report only the names.
    """
    home = home or Path.home()
    found: dict[str, str] = {}
    for name in (
        "OPENAGENTS_API_KEY",
        "TYPESAFE_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
    ):
        value = os.environ.get(name)
        if value:
            found[f"env {name}"] = value
    bearer = home / ".openagents" / "bearer"
    try:
        value = bearer.read_text().strip()
        if value:
            found["~/.openagents/bearer"] = value
    except OSError:
        pass
    jev = _read_json(home / ".openagents" / "jev.json")
    if isinstance(jev, dict) and isinstance(jev.get("api_key"), str):
        found["~/.openagents/jev.json api_key"] = jev["api_key"]
    claude = _read_json(home / ".claude" / ".credentials.json")
    oauth = claude.get("claudeAiOauth") if isinstance(claude, dict) else None
    if isinstance(oauth, dict):
        for key in ("accessToken", "refreshToken"):
            if isinstance(oauth.get(key), str):
                found[f"~/.claude/.credentials.json {key}"] = oauth[key]
    codex = _read_json(home / ".codex" / "auth.json")

    def walk(value: Any, where: str) -> None:
        if isinstance(value, dict):
            for key, item in value.items():
                walk(item, f"{where}.{key}")
        elif isinstance(value, str):
            found[f"~/.codex/auth.json {where.lstrip('.')}"] = value

    if isinstance(codex, dict):
        walk(codex.get("tokens"), "tokens")
        if isinstance(codex.get("OPENAI_API_KEY"), str):
            found["~/.codex/auth.json OPENAI_API_KEY"] = codex["OPENAI_API_KEY"]
    return {k: v for k, v in found.items() if len(v) >= MIN_SECRET_CHARS}


def scan_for_credentials(
    root: Path, credentials: dict[str, str]
) -> list[dict[str, str]]:
    """Every file under ``root`` holding a credential value, by name only."""
    needles = {name: value.encode() for name, value in credentials.items()}
    matches = []
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        data = path.read_bytes()
        for name, needle in needles.items():
            if needle in data:
                matches.append(
                    {"file": path.relative_to(root).as_posix(), "credential": name}
                )
    return matches


def _atif_summary(path: Path) -> dict[str, Any]:
    value = _read_json(path)
    steps = value.get("steps") if isinstance(value, dict) else None
    return {
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
        "schema_version": value.get("schema_version")
        if isinstance(value, dict)
        else None,
        "steps": len(steps) if isinstance(steps, list) else None,
        "parses": value is not None,
    }


def _conversion(raw: Path | None, normalized: Path | None, native: int) -> dict:
    record: dict[str, Any] = {
        "raw": _atif_summary(raw) if raw else None,
        "normalized": _atif_summary(normalized) if normalized else None,
    }
    if raw and normalized:
        if record["raw"]["sha256"] == record["normalized"]["sha256"]:
            record["conversion"] = (
                "Harbor's trajectory.json is a byte-identical copy of the "
                "episode's raw ATIF; no conversion ran."
            )
        else:
            record["conversion"] = (
                "Harbor's trajectory.json differs from the episode's raw "
                "ATIF; the adapter rewrote it, so compare both."
            )
    elif normalized and native:
        record["conversion"] = (
            "Harbor's agent adapter converted the executor's native traces "
            "(under native/) into trajectory.json."
        )
    elif normalized:
        record["conversion"] = (
            "Only Harbor's normalized trajectory exists; no raw ATIF or "
            "native trace was left in the job directory."
        )
    else:
        record["conversion"] = "No trajectory exists for this trial."
    return record


def _trim_result(result: dict[str, Any]) -> dict[str, Any]:
    trimmed = {key: result.get(key) for key in HARBOR_RESULT_KEYS}
    agent = result.get("agent_result") or {}
    if isinstance(agent, dict):
        trimmed["agent_result"] = {
            key: agent.get(key)
            for key in (
                "n_input_tokens",
                "n_cache_tokens",
                "n_output_tokens",
                "cost_usd",
                "rollout_details",
                "metadata",
            )
        }
    return trimmed


def plan_trial(trial_dir: Path) -> tuple[_Plan, dict[str, Any]]:
    """The trial's evidence closure: what to copy and what is missing."""
    plan = _Plan()
    agent = trial_dir / "agent"
    episode = agent / "episode"
    job_dir = trial_dir.parent
    trial = trial_dir.name
    context: dict[str, Any] = {"raw_atif": None, "normalized_atif": None}

    plan.add("normalized trajectory", agent / "trajectory.json", "@trajectory")
    if (agent / "trajectory.json").is_file():
        context["normalized_atif"] = agent / "trajectory.json"

    manifest = _read_json(episode / "manifest.json")
    if (episode / "manifest.json").is_file():
        plan.add("agent episode manifest", episode / "manifest.json", "manifest.json")
    if isinstance(manifest, dict) and manifest.get("contract") == EPISODE_CONTRACT:
        for key, entry in sorted((manifest.get("files") or {}).items()):
            relative = entry.get("path") if isinstance(entry, dict) else None
            if not relative or ".." in Path(relative).parts:
                continue
            kind = "native delegate stream" if relative.endswith(
                ".stream.jsonl"
            ) else key
            plan.add(kind, episode / relative, relative, entry.get("sha256"))
        stream = ((manifest.get("delegate") or {}).get("delegation") or {}).get(
            "stream"
        ) or {}
        if stream.get("path"):
            plan.add(
                "native delegate stream",
                episode / stream["path"],
                stream["path"],
                stream.get("sha256"),
            )
            if stream.get("truncated"):
                plan.notes.append(
                    f"The episode kept {stream.get('kept_bytes')} of "
                    f"{stream.get('bytes')} stream bytes; the rest was never "
                    "written."
                )
        trajectory = (manifest.get("files") or {}).get("trajectory") or {}
        if trajectory.get("path") and (episode / trajectory["path"]).is_file():
            context["raw_atif"] = episode / trajectory["path"]
    # Episode files the manifest doesn't name, such as late delegate
    # streams or local check output, still belong to the closure.
    if episode.is_dir():
        for path in sorted(p for p in episode.rglob("*") if p.is_file()):
            plan.add("episode file", path, path.relative_to(episode).as_posix())

    native = 0
    for pattern in ("*.jsonl", "*.txt", "sessions/**/*.jsonl", "rollout*.jsonl"):
        for path in sorted(agent.glob(pattern)):
            if path.is_file() and not path.name.startswith("."):
                plan.add(
                    "native trace",
                    path,
                    f"native/{path.relative_to(agent).as_posix()}",
                )
                native += 1
    context["native"] = native
    setup = agent / "toolchain-setup.json"
    if setup.is_file():
        plan.add("setup record", setup, "setup/toolchain-setup.json")

    verifier = trial_dir / "verifier"
    if verifier.is_dir():
        for path in sorted(p for p in verifier.rglob("*") if p.is_file()):
            plan.add(
                "verifier output",
                path,
                f"verifier/{path.relative_to(verifier).as_posix()}",
            )
    else:
        plan.missing.append(
            {
                "kind": "verifier output",
                "reference": "verifier/",
                "reason": "the trial has no verifier directory",
            }
        )
    for name in ("test-stdout.txt", "ctrf.json"):
        if verifier.is_dir() and not (verifier / name).is_file():
            plan.missing.append(
                {
                    "kind": "verifier output",
                    "reference": f"verifier/{name}",
                    "reason": "the verifier left no such file",
                }
            )

    produced = trial_dir / "artifacts"
    if produced.is_dir():
        for path in sorted(p for p in produced.rglob("*") if p.is_file()):
            plan.add(
                "produced artifact",
                path,
                f"produced/{path.relative_to(produced).as_posix()}",
            )
        entries = _read_json(produced / "manifest.json")
        if isinstance(entries, list):
            for entry in entries:
                if isinstance(entry, dict) and entry.get("status") == "empty":
                    plan.notes.append(
                        f"Harbor collected nothing from {entry.get('source')}; "
                        "files the task wrote elsewhere in the container were "
                        "not captured."
                    )

    # The harness manifest names evidence by absolute path; each one it
    # resolved must be in the closure or reported.
    tbench_manifest = _read_json(job_dir / "tbench" / "manifests" / f"{trial}.json")
    if isinstance(tbench_manifest, dict):
        plan.add(
            "harness episode manifest",
            job_dir / "tbench" / "manifests" / f"{trial}.json",
            "tbench-manifest.json",
        )
        for entry in _evidence_entries(tbench_manifest.get("evidence")):
            if not entry.get("resolved") or not entry.get("path"):
                continue
            source = Path(entry["path"])
            if not source.is_file():
                plan.missing.append(
                    {
                        "kind": entry.get("kind", "evidence"),
                        "reference": _job_relative(source, job_dir),
                        "reason": "the harness manifest names it, but the file "
                        "is gone",
                    }
                )
    attempt = job_dir / "tbench" / "attempts" / f"{trial}.json"
    if attempt.is_file():
        plan.add("harness attempt record", attempt, "tbench-attempt.json")
    return plan, context


def _evidence_entries(value: Any) -> list[dict[str, Any]]:
    if isinstance(value, dict) and "kind" in value:
        return [value]
    if isinstance(value, dict):
        return [e for item in value.values() for e in _evidence_entries(item)]
    if isinstance(value, list):
        return [e for item in value for e in _evidence_entries(item)]
    return []


def _job_relative(path: Path, job_dir: Path) -> str:
    try:
        return f"<jobs-dir>/{job_dir.name}/{path.relative_to(job_dir).as_posix()}"
    except ValueError:
        return path.name


def retain_trial(
    trial_dir: Path,
    traces_dir: Path,
    credentials: dict[str, str],
    *,
    max_file_bytes: int = MAX_FILE_BYTES,
    max_trial_bytes: int = MAX_TRIAL_BYTES,
    dry_run: bool = False,
) -> TrialRetention:
    """Retain one trial, or raise when a credential value is found."""
    job = trial_dir.parent.name
    trial = trial_dir.name
    result = _read_json(trial_dir / "result.json")
    if not isinstance(result, dict):
        raise RetentionError(f"{job}/{trial}: no readable result.json")
    plan, context = plan_trial(trial_dir)
    destination = traces_dir / job
    record: dict[str, Any] = {
        "schema": RETENTION_SCHEMA,
        "job": job,
        "trial": trial,
        "source": f"<jobs-dir>/{job}/{trial}",
        "retained_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "bounds": {"file_bytes": max_file_bytes, "trial_bytes": max_trial_bytes},
        "files": [],
        "missing": list(plan.missing),
        "notes": list(plan.notes),
        "atif": _conversion(
            context["raw_atif"], context["normalized_atif"], context["native"]
        ),
    }
    with tempfile.TemporaryDirectory(prefix="tbench-retain-") as scratch:
        stage = Path(scratch)
        episode = stage / f"{trial}.episode"
        episode.mkdir()
        total = 0
        for kind, source, target, expected in plan.files:
            size = source.stat().st_size
            reason = None
            if size > max_file_bytes:
                reason = f"{size} bytes exceeds the {max_file_bytes}-byte file bound"
            elif total + size > max_trial_bytes:
                reason = (
                    f"{size} bytes would exceed the {max_trial_bytes}-byte "
                    "trial bound"
                )
            published = f"{trial}.json" if target == "@trajectory" else target
            if reason:
                record["missing"].append(
                    {"kind": kind, "reference": published, "reason": reason}
                )
                continue
            out = stage / published if target == "@trajectory" else episode / target
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, out)
            total += size
            digest = sha256_file(out)
            record["files"].append(
                {
                    "kind": kind,
                    "path": published,
                    "bytes": size,
                    "sha256": digest,
                    "manifest_sha256": expected,
                    "digest": "none recorded"
                    if expected is None
                    else ("match" if expected == digest else "mismatch"),
                }
            )
        harbor = episode / "harbor-result.json"
        harbor.write_text(json.dumps(_trim_result(result), indent=2) + "\n")
        record["files"].append(
            {
                "kind": "Harbor result (trimmed)",
                "path": "harbor-result.json",
                "bytes": harbor.stat().st_size,
                "sha256": sha256_file(harbor),
                "manifest_sha256": None,
                "digest": "none recorded",
            }
        )
        matches = scan_for_credentials(stage, credentials)
        record["credential_scan"] = {
            "credentials_checked": sorted(credentials),
            "files_scanned": sum(1 for p in stage.rglob("*") if p.is_file()),
            "matches": len(matches),
        }
        if matches:
            where = ", ".join(f"{m['file']} ({m['credential']})" for m in matches)
            raise RetentionError(
                f"{job}/{trial}: credential values found in {where}; nothing "
                "was written"
            )
        (episode / "retention.json").write_text(
            json.dumps(record, indent=2) + "\n"
        )
        if not dry_run:
            destination.mkdir(parents=True, exist_ok=True)
            target_episode = destination / f"{trial}.episode"
            if target_episode.exists():
                shutil.rmtree(target_episode)
            shutil.copytree(episode, target_episode)
            trajectory = stage / f"{trial}.json"
            if trajectory.is_file():
                shutil.copyfile(trajectory, destination / f"{trial}.json")
    return TrialRetention(job, trial, record, not dry_run, destination)


def trial_dirs(job_dir: Path, trials: list[str] | None = None) -> list[Path]:
    """The job's trial directories, those with a Harbor result."""
    found = sorted(p.parent for p in job_dir.glob("*/result.json"))
    if trials:
        found = [p for p in found if p.name in trials]
    return found


def retain_jobs(
    jobs: list[str],
    *,
    jobs_dir: Path | None = None,
    traces_dir: Path | None = None,
    trials: list[str] | None = None,
    credentials: dict[str, str] | None = None,
    **bounds: Any,
) -> tuple[list[TrialRetention], list[str]]:
    """Retain every named job; returns the retentions and the errors."""
    jobs_dir = jobs_dir or paths.jobs_dir()
    traces_dir = traces_dir or DEFAULT_TRACES_DIR
    credentials = known_credentials() if credentials is None else credentials
    retained: list[TrialRetention] = []
    errors: list[str] = []
    for job in jobs:
        job_dir = Path(job)
        if not job_dir.is_dir():
            job_dir = jobs_dir / job
        if not job_dir.is_dir():
            errors.append(f"{job}: no job directory")
            continue
        found = trial_dirs(job_dir, trials)
        if not found:
            errors.append(f"{job_dir.name}: no trial with a result.json")
        for trial_dir in found:
            try:
                retained.append(
                    retain_trial(trial_dir, traces_dir, credentials, **bounds)
                )
            except RetentionError as exc:
                errors.append(str(exc))
    return retained, errors
