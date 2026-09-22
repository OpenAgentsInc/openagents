"""The per-attempt result record: one versioned document per trial.

``openagents.tbench.attempt.v1`` joins Harbor's trial identity to what the
benchmark owes: reward and terminal status kept apart, every timing phase,
usage with its coverage named, cost with its provenance named, count
semantics made explicit, and links to the retained evidence. An attempt
that cannot answer a field says ``unknown`` rather than inventing a number.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ATTEMPT_SCHEMA = "openagents.tbench.attempt.v1"
MANIFEST_SCHEMA = "openagents.tbench.episode-manifest.v1"

# How a trial ended, in the benchmark's own words. Upstream reward and this
# status stay separate fields: an agent can exit 0 and still fail the
# protected verifier.
TERMINAL_STATUSES = (
    "completed",
    "failed",
    "timeout",
    "cancelled",
    "setup_failure",
    "install_failure",
    "verifier_failure",
    "provider_refusal",
    "agent_error",
    "unverifiable",
    "unknown",
)

# Where a cost number came from. These are different claims and are never
# pooled.
COST_PROVENANCES = (
    "provider_reported",
    "price_estimate",
    "billing_verified",
    "mixed",
    "none",
    "unknown",
)


def sha256_file(path: Path) -> str:
    """The hex digest of a file's bytes, for evidence references."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _ms(timing: dict[str, Any] | None) -> int | None:
    """Elapsed milliseconds of one Harbor TimingInfo object, or None."""
    if not timing:
        return None
    started, finished = timing.get("started_at"), timing.get("finished_at")
    if not started or not finished:
        return None
    try:
        delta = datetime.fromisoformat(
            finished.replace("Z", "+00:00")
        ) - datetime.fromisoformat(started.replace("Z", "+00:00"))
    except ValueError:
        return None
    return int(delta.total_seconds() * 1000)


def _terminal_status(result: dict[str, Any]) -> str:
    """Map a Harbor TrialResult onto the benchmark's terminal statuses."""
    exception = result.get("exception_info")
    if exception:
        etype = (exception.get("exception_type") or "").lower()
        if "install" in etype or "setup" in etype:
            return "install_failure"
        if "timeout" in etype:
            return "timeout"
        if "cancel" in etype:
            return "cancelled"
        if "verifier" in etype:
            return "verifier_failure"
        if "refusal" in etype:
            return "provider_refusal"
        return "agent_error"
    verifier = result.get("verifier_result")
    if verifier is None:
        return "unverifiable"
    return "completed"


def _verifier_result(result: dict[str, Any]) -> dict[str, Any] | None:
    """The trial's verifier result, single- or multi-step."""
    verifier = result.get("verifier_result")
    if verifier is not None:
        return verifier
    for step in result.get("step_results") or []:
        if step.get("verifier_result") is not None:
            return step["verifier_result"]
    return None


def _usage_totals(result: dict[str, Any]) -> dict[str, int | None]:
    """Sum usage across agent_result contexts, matching Harbor's own rule:
    single-step trials carry one ``agent_result``; multi-step trials carry
    one per ``step_results[i].agent_result``. A context's input count
    includes its cache tokens."""
    agent_result = result.get("agent_result")
    if agent_result is not None:
        contexts = [agent_result]
    else:
        contexts = [
            step["agent_result"]
            for step in result.get("step_results") or []
            if step.get("agent_result") is not None
        ]
    totals: dict[str, int | None] = {
        "input_tokens": None,
        "cache_tokens": None,
        "output_tokens": None,
    }
    key_map = {
        "input_tokens": "n_input_tokens",
        "cache_tokens": "n_cache_tokens",
        "output_tokens": "n_output_tokens",
    }
    for ctx in contexts:
        for out_key, ctx_key in key_map.items():
            value = ctx.get(ctx_key)
            if value is not None:
                totals[out_key] = (totals[out_key] or 0) + value
    return totals


def _cost_total(result: dict[str, Any]) -> float | None:
    """Sum cost_usd the same way usage is summed; None stays absent."""
    agent_result = result.get("agent_result")
    contexts = (
        [agent_result]
        if agent_result is not None
        else [
            step["agent_result"]
            for step in result.get("step_results") or []
            if step.get("agent_result") is not None
        ]
    )
    total: float | None = None
    for ctx in contexts:
        value = ctx.get("cost_usd")
        if value is not None:
            total = (total or 0.0) + value
    return total


def attempt_record(
    trial_result: dict[str, Any],
    *,
    job_name: str,
    trial_dir: Path,
    arm: str,
    profile_id: str,
    auth_mode: str | None,
    declared_cost_provenance: str,
    pin: dict[str, str] | None = None,
    counts: dict[str, Any] | None = None,
    evidence: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build the ``openagents.tbench.attempt.v1`` record for one trial.

    ``declared_cost_provenance`` is the arm profile's claim about what its
    cost field means; ``unknown`` overrides it whenever no number exists.
    ``pin`` is the panel's declared upstream identity: a local-checkout
    task reports no git fields of its own, so the pin carries the revision
    the run was locked to alongside the observed checksum.
    """
    config = trial_result.get("config") or {}
    agent_cfg = config.get("agent") or {}
    agent_info = trial_result.get("agent_info") or {}
    verifier = _verifier_result(trial_result) or {}
    rewards = verifier.get("rewards") if verifier else None
    reward = None
    if isinstance(rewards, dict):
        reward = rewards.get("reward", next(iter(rewards.values()), None))

    cost_usd = _cost_total(trial_result)
    provenance = (
        declared_cost_provenance if cost_usd is not None else "unknown"
    )

    timing = {
        "started_at": trial_result.get("started_at"),
        "finished_at": trial_result.get("finished_at"),
        "environment_setup_ms": _ms(trial_result.get("environment_setup")),
        "agent_setup_ms": _ms(trial_result.get("agent_setup")),
        "agent_execution_ms": _ms(trial_result.get("agent_execution")),
        "verifier_ms": _ms(trial_result.get("verifier")),
        "total_ms": _ms(
            {
                "started_at": trial_result.get("started_at"),
                "finished_at": trial_result.get("finished_at"),
            }
        ),
    }

    usage_totals = _usage_totals(trial_result)
    usage = {
        "input_tokens": usage_totals["input_tokens"],
        "cache_tokens": usage_totals["cache_tokens"],
        "output_tokens": usage_totals["output_tokens"],
        "input_includes_cache": True,
        "coverage": (
            "full"
            if usage_totals["input_tokens"] is not None
            and usage_totals["output_tokens"] is not None
            else "unknown"
        ),
    }
    if usage["coverage"] == "unknown" and (
        usage["input_tokens"] is not None or usage["output_tokens"] is not None
    ):
        usage["coverage"] = "partial"

    task_id = trial_result.get("task_id") or {}
    record = {
        "schema": ATTEMPT_SCHEMA,
        "attempt": {
            "id": str(trial_result.get("id") or ""),
            "job": job_name,
            "trial": trial_result.get("trial_name") or trial_dir.name,
            "arm": arm,
            "profile": profile_id,
            "auth_mode": auth_mode,
            "kind": "fresh",
        },
        "task": {
            "name": trial_result.get("task_name"),
            "path": task_id.get("path"),
            "git_url": task_id.get("git_url"),
            "git_commit_id": task_id.get("git_commit_id"),
            "checksum": trial_result.get("task_checksum"),
            "source": trial_result.get("source"),
            "pin": pin or {},
        },
        "agent": {
            "selector": agent_cfg.get("name") or agent_cfg.get("import_path"),
            "requested_model": agent_cfg.get("model_name"),
            "observed_name": agent_info.get("name"),
            "observed_version": agent_info.get("version"),
            "observed_model": (agent_info.get("model_info") or {}).get("name"),
            "observed_provider": (agent_info.get("model_info") or {}).get(
                "provider"
            ),
        },
        "outcome": {
            "reward": reward,
            "verifier_rewards": rewards,
            "terminal_status": _terminal_status(trial_result),
            "exception": trial_result.get("exception_info"),
            "verifier_environment_mode": trial_result.get(
                "verifier_environment_mode"
            ),
        },
        "timing": timing,
        "usage": usage,
        "cost": {
            "amount_usd": cost_usd,
            "provenance": provenance,
            "note": (
                "A subscription reference price or a pricing-registry "
                "estimate is not an observed incremental bill."
            ),
        },
        "counts": counts or {"semantics": "unknown"},
        "evidence": evidence or {},
        "completeness": {
            "trace": "unknown",
            "usage": usage["coverage"],
            "cost": "reported" if cost_usd is not None else "unknown",
            "artifacts": "unknown",
        },
    }
    return record


@dataclass
class TrialPaths:
    """The files this package writes beside Harbor's trial output."""

    job_dir: Path

    @property
    def tbench_dir(self) -> Path:
        return self.job_dir / "tbench"

    @property
    def attempts_dir(self) -> Path:
        return self.tbench_dir / "attempts"

    @property
    def manifests_dir(self) -> Path:
        return self.tbench_dir / "manifests"

    @property
    def config_path(self) -> Path:
        return self.tbench_dir / "job-config.json"

    @property
    def context_path(self) -> Path:
        return self.tbench_dir / "context.json"

    @property
    def report_path(self) -> Path:
        return self.tbench_dir / "report.json"


def load_trial_results(job_dir: Path) -> list[tuple[Path, dict[str, Any]]]:
    """Every ``result.json`` under a job dir that parses, oldest first."""
    out = []
    for path in sorted(job_dir.glob("*/result.json")):
        try:
            out.append((path.parent, json.loads(path.read_text())))
        except (OSError, json.JSONDecodeError):
            continue
    return out


def episode_manifest(
    record: dict[str, Any],
    *,
    trial_dir: Path,
    arm: str,
    task_notes: list[str] | None = None,
) -> dict[str, Any]:
    """The episode manifest linking pins, identities, evidence, and usage.

    Content references name a path inside the job dir and a sha256 of the
    bytes found there at collection time; a reference without retained
    bytes is marked unresolved rather than left dangling.
    """
    def entry(path: Path | None, kind: str) -> dict[str, Any]:
        if path is None or not path.exists():
            return {"kind": kind, "resolved": False, "path": None, "sha256": None}
        return {
            "kind": kind,
            "resolved": True,
            "path": str(path),
            "sha256": sha256_file(path),
            "bytes": path.stat().st_size,
        }

    agent_dir = trial_dir / "agent"
    verifier_dir = trial_dir / "verifier"
    artifacts_dir = trial_dir / "artifacts"
    trajectory = next(
        iter(sorted(agent_dir.glob("*.json")) + sorted(agent_dir.glob("trajectory*.json"))),
        None,
    )
    artifacts = [
        entry(path, "artifact")
        for path in sorted(artifacts_dir.rglob("*"))
        if path.is_file()
    ] if artifacts_dir.is_dir() else []

    native = [
        entry(path, "native-trace")
        for pattern in ("*.jsonl", "sessions/**/*.jsonl", "rollout*.jsonl")
        for path in sorted(agent_dir.glob(pattern))
        if path.is_file()
    ]

    return {
        "schema": MANIFEST_SCHEMA,
        "attempt": record["attempt"],
        "task": record["task"],
        "agent": record["agent"],
        "outcome": record["outcome"],
        "timing": record["timing"],
        "usage": record["usage"],
        "cost": record["cost"],
        "task_notes": task_notes or [],
        "evidence": {
            "trial_result": entry(trial_dir / "result.json", "trial-result"),
            "trajectory": entry(trajectory, "trajectory"),
            "native_traces": native,
            "verifier_reward": entry(
                next(
                    (
                        p
                        for p in (
                            verifier_dir / "reward.json",
                            verifier_dir / "reward.txt",
                        )
                        if p.exists()
                    ),
                    None,
                ),
                "verifier-reward",
            ),
            "verifier_report": entry(
                verifier_dir / "ctrf.json", "verifier-report"
            ),
            "artifacts": artifacts,
        },
        "collected_at": datetime.now(timezone.utc).isoformat(
            timespec="milliseconds"
        ),
    }
