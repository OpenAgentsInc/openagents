"""Targeted experiments: several arms, repeated attempts, interleaved.

``tbench experiment run`` runs two or more arms over a few tasks, three
attempts per task per arm by default, so a pass-rate difference can be
told apart from chance. One attempt per task gives intervals of about
±15 points, wider than every difference measured so far.

The schedule interleaves the arms: attempt 1 of every task, then attempt
2, and so on, and within each task the arm order rotates, so no arm always
runs first, on a colder cache, or later in the day. Each trial is its own
job, ``<profile>--<arm>--<task>--<experiment>-r<attempt>``, started
through ``tbench run`` by the suite scheduler, so it keeps the host
budgets, the host-wide Claude slots, the usage-limit pause, and the
evidence a single run keeps.

Three things never count as a graded attempt, and never enter a
denominator. Each is recorded in the experiment's ``ledger.jsonl`` with
the Claude quota it drew, and the trial runs again:

- **Credentials**: a session that failed to authenticate. No further
  trial on that provider starts until the operator fixes the credential
  and restarts the experiment.
- **Quota**: a session a provider's usage limit stopped. Every arm on the
  provider pauses until the limit resets.
- **Infrastructure**: a setup timeout, or an environment that failed
  before the agent started, such as a registry reset during its build.

Claude trials use the long-lived token in
``~/.openagents/claude-setup-token``. An experiment that has a Claude arm
refuses to start without it, unless ``--allow-login-token`` accepts the
expiring login and the trials a login refresh can lose.

``--quota-usd`` budgets the Claude quota: the list-price value every
Claude Code session reports as ``total_cost_usd``, summed over graded and
lost trials. The scheduler starts no Claude trial once the budget is used,
and none while the running trials, at the mean cost so far, could take
the total past it. The value measures subscription use; it isn't a cash
charge.

``gym terminal-bench experiment report`` reads the status file this
writes and reports each arm's passes with Wilson intervals, the paired
comparison, the losses, and the quota used.
"""

from __future__ import annotations

import json
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from . import credentials, paths, usage_limit
from .panel import Task
from .suite import (
    FINISHED,
    RUNNING,
    Budget,
    Host,
    JobState,
    Launcher,
    Scheduler,
    Trial,
    utc_now,
)

SPEC_SCHEMA = "openagents.tbench.experiment.v1"
STATUS_SCHEMA = "openagents.tbench.experiment-status.v1"
LEDGER_SCHEMA = "openagents.tbench.experiment-ledger.v1"

DEFAULT_ATTEMPTS = 3
DEFAULT_MAX_CLAUDE_CONCURRENT = 3

# What a trial set aside under each label was lost to.
LOSS_CAUSES = {
    "credentials": "credentials",
    "usage-limit": "quota",
    "setup-timeout": "infrastructure",
}


class ExperimentError(RuntimeError):
    pass


def experiment_dir(experiment: str) -> Path:
    """Where one experiment's spec, status, ledger, and logs live."""
    return paths.state_dir() / "experiments" / experiment


def job_name(profile: str, arm: str, task: str, experiment: str, attempt: int) -> str:
    return f"{profile}--{arm}--{task}--{experiment}-r{attempt}"


def interleave(
    tasks: list[str], arms: list[str], attempts: int
) -> list[tuple[int, str, str]]:
    """The dispatch order: ``(attempt, task, arm)``, arms rotating per slot."""
    order = []
    for attempt in range(1, attempts + 1):
        for index, task in enumerate(tasks):
            shift = (attempt - 1 + index) % len(arms)
            for arm in arms[shift:] + arms[:shift]:
                order.append((attempt, task, arm))
    return order


@dataclass
class Spec:
    """What an experiment runs. Pinned on its first start."""

    id: str
    profile: str
    arms: list[str]
    tasks: list[str]
    attempts: int = DEFAULT_ATTEMPTS
    arm_args: dict[str, list[str]] = field(default_factory=dict)
    # The quota budget isn't pinned: an operator may raise it to finish.
    quota_usd: float | None = None
    # An arm named apart from the agent profile it runs, such as a
    # proposal's policy on its base profile: ``{arm: profile}``.
    arm_profiles: dict[str, str] = field(default_factory=dict)

    def profile_of(self, arm: str) -> str:
        """The agent profile an arm runs."""
        return self.arm_profiles.get(arm, arm)

    def validate(self) -> None:
        if not self.id or "--" in self.id or "/" in self.id:
            raise ExperimentError(
                f"experiment id {self.id!r} must be non-empty, without '--' or '/'"
            )
        if len(self.arms) < 2:
            raise ExperimentError("an experiment compares at least two arms (--arm twice)")
        if len(set(self.arms)) != len(self.arms):
            raise ExperimentError("an arm is listed twice")
        if not self.tasks:
            raise ExperimentError("an experiment needs at least one task (--tasks)")
        if len(set(self.tasks)) != len(self.tasks):
            raise ExperimentError("a task is listed twice")
        if self.attempts < 1:
            raise ExperimentError("--attempts must be at least 1")
        if self.quota_usd is not None and self.quota_usd <= 0:
            raise ExperimentError("--quota-usd must be positive")
        for arm, profile in self.arm_profiles.items():
            if arm not in self.arms:
                raise ExperimentError(f"arm {arm!r} runs {profile!r} but isn't an arm")
            if not arm or "--" in arm or "/" in arm or not profile:
                raise ExperimentError(
                    f"arm {arm!r}={profile!r} needs a name without '--' or '/' and a profile"
                )

    def pinned(self) -> dict[str, Any]:
        pinned = {
            "schema": SPEC_SCHEMA,
            "id": self.id,
            "profile": self.profile,
            "arms": self.arms,
            "tasks": self.tasks,
            "attempts": self.attempts,
            "arm_args": self.arm_args,
        }
        # Only when used, so experiments pinned before it still match.
        if self.arm_profiles:
            pinned["arm_profiles"] = self.arm_profiles
        return pinned

    @classmethod
    def from_pinned(cls, data: dict[str, Any], quota_usd: float | None) -> Spec:
        return cls(
            id=data["id"],
            profile=data["profile"],
            arms=list(data["arms"]),
            tasks=list(data["tasks"]),
            attempts=int(data["attempts"]),
            arm_args={k: list(v) for k, v in (data.get("arm_args") or {}).items()},
            quota_usd=quota_usd,
            arm_profiles=dict(data.get("arm_profiles") or {}),
        )


def pin(spec: Spec, directory: Path) -> Spec:
    """Write the spec on the first start; refuse a different one after that."""
    path = directory / "experiment.json"
    wanted = spec.pinned()
    if path.exists():
        existing = json.loads(path.read_text())
        if existing != wanted:
            changed = sorted(k for k in wanted if existing.get(k) != wanted[k])
            raise ExperimentError(
                f"experiment {spec.id} was pinned with different "
                f"{', '.join(changed)}; use a new experiment id"
            )
        return spec
    directory.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(wanted, indent=2) + "\n")
    return spec


def load_spec(experiment: str) -> dict[str, Any] | None:
    path = experiment_dir(experiment) / "experiment.json"
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def read_status(experiment: str) -> dict[str, Any] | None:
    try:
        return json.loads((experiment_dir(experiment) / "status.json").read_text())
    except (OSError, ValueError):
        return None


def _zero_usage() -> dict[str, Any]:
    return {"sessions": 0, "usd": 0.0, "output_tokens": 0, "unpriced": 0}


def _add(total: dict[str, Any], usage: dict[str, Any]) -> None:
    for key in total:
        total[key] += usage.get(key) or 0
    total["usd"] = round(total["usd"], 6)


class ExperimentScheduler(Scheduler):
    """The suite scheduler over several arms, with a Claude quota budget."""

    def __init__(
        self,
        *,
        spec: Spec,
        tasks: dict[str, Task],
        arm_providers: dict[str, frozenset[str]],
        credential_source: str | None,
        budget: Budget,
        jobs_dir: Path,
        directory: Path,
        launcher: Launcher,
        host_: Host,
        pin_: dict[str, Any] | None = None,
        usage: Callable[[Path], dict[str, Any]] = credentials.job_claude_usage,
        **kwargs: Any,
    ) -> None:
        providers = frozenset().union(*arm_providers.values())
        super().__init__(
            profile=spec.profile,
            arm=spec.arms[0],
            pin=pin_ or {},
            tasks=[],
            attempts=0,
            budget=budget,
            jobs_dir=jobs_dir,
            directory=directory,
            launcher=launcher,
            host_=host_,
            providers=providers,
            **kwargs,
        )
        self.spec = spec
        self.arm_providers = arm_providers
        self.credential_source = credential_source
        self.usage_of = usage
        self.trials = [
            Trial(
                tasks[task],
                attempt,
                job_name(spec.profile, arm, task, spec.id, attempt),
                arm=arm,
            )
            for attempt, task, arm in interleave(spec.tasks, spec.arms, spec.attempts)
        ]
        self.ledger = directory / "ledger.jsonl"
        self.usage: dict[str, dict[str, Any]] = {}
        self.losses: dict[str, list[dict[str, Any]]] = {}
        for record in self._read_ledger():
            if record.get("event") == "loss":
                self.losses.setdefault(record["job"], []).append(record)
        self.quota_note: str | None = None

    # -- the ledger ----------------------------------------------------------

    def _read_ledger(self) -> list[dict[str, Any]]:
        try:
            lines = self.ledger.read_text().splitlines()
        except OSError:
            return []
        records = []
        for line in lines:
            try:
                records.append(json.loads(line))
            except ValueError:
                continue
        return records

    def _append(self, record: dict[str, Any]) -> None:
        self.ledger.parent.mkdir(parents=True, exist_ok=True)
        with self.ledger.open("a") as handle:
            handle.write(json.dumps({"schema": LEDGER_SCHEMA, **record}) + "\n")

    # -- what a trial bills and holds ---------------------------------------

    def trial_providers(self, trial: Trial) -> frozenset[str]:
        return self.arm_providers.get(trial.arm or "", frozenset())

    def quota_used(self) -> dict[str, Any]:
        total = _zero_usage()
        for usage in self.usage.values():
            _add(total, usage)
        for losses in self.losses.values():
            for loss in losses:
                _add(total, loss.get("usage") or {})
        return total

    def _mean_claude_trial_usd(self) -> float:
        costs = [
            self.usage[t.job]["usd"]
            for t in self.trials
            if t.job in self.usage and "anthropic" in self.trial_providers(t)
        ]
        return sum(costs) / len(costs) if costs else 0.0

    def hold_reason(self, trial: Trial) -> str | None:
        held = super().hold_reason(trial)
        if held or self.spec.quota_usd is None:
            return held
        if "anthropic" not in self.trial_providers(trial):
            return None
        budget = self.spec.quota_usd
        used = self.quota_used()["usd"]
        if used >= budget:
            return (
                f"the Claude quota budget is used (${used:.2f} of ${budget:.2f}); "
                "raise --quota-usd and restart to finish"
            )
        running = [
            t for t in self.trials
            if t.state == RUNNING and "anthropic" in self.trial_providers(t)
        ]
        mean = self._mean_claude_trial_usd()
        if running and used + mean * (len(running) + 1) > budget:
            return (
                f"{len(running)} running Claude trials at ${mean:.2f} each could "
                f"take ${used:.2f} past the ${budget:.2f} budget"
            )
        return None

    # -- recording outcomes --------------------------------------------------

    def _apply(self, trial: Trial, state: JobState) -> None:
        super()._apply(trial, state)
        if trial.state == FINISHED:
            self.usage[trial.job] = self.usage_of(self.jobs_dir / trial.job)

    def _set_aside(self, trial: Trial, label: str) -> Path:
        job_dir = self.jobs_dir / trial.job
        usage = self.usage_of(job_dir)
        message = None
        if label == "usage-limit":
            message = (usage_limit.job_usage_limit(job_dir) or {}).get("message")
        elif label == "credentials":
            from .runner import trial_dirs

            for trial_dir in trial_dirs(job_dir) if job_dir.is_dir() else []:
                found = credentials.trial_credential_failure(trial_dir)
                if found:
                    message = found.get("message")
                    break
        target = super()._set_aside(trial, label)
        loss = {
            "event": "loss",
            "at": utc_now(),
            "job": trial.job,
            "arm": trial.arm,
            "task": trial.task.id,
            "attempt": trial.attempt,
            "cause": LOSS_CAUSES.get(label, "infrastructure"),
            "label": label,
            "moved_to": str(target),
            "message": message,
            "usage": usage,
        }
        self._append(loss)
        self.losses.setdefault(trial.job, []).append(loss)
        return target

    # -- status ------------------------------------------------------------

    def status(self) -> dict[str, Any]:
        base = super().status()
        trials = []
        for index, trial in enumerate(self.trials):
            row = trial.to_json()
            row["order"] = index
            row["claude_usage"] = self.usage.get(trial.job)
            row["losses"] = self.losses.get(trial.job, [])
            trials.append(row)
        by_arm: dict[str, dict[str, Any]] = {arm: _zero_usage() for arm in self.spec.arms}
        for trial in self.trials:
            arm = by_arm[trial.arm or ""]
            _add(arm, self.usage.get(trial.job) or {})
            for loss in self.losses.get(trial.job, []):
                _add(arm, loss.get("usage") or {})
        return {
            **base,
            "schema": STATUS_SCHEMA,
            "experiment": self.spec.id,
            "arm": None,
            "arms": [
                {"id": arm, "providers": sorted(self.arm_providers.get(arm, frozenset()))}
                for arm in self.spec.arms
            ],
            "tasks": self.spec.tasks,
            "attempts": self.spec.attempts,
            "credential_source": self.credential_source,
            "quota": {
                "budget_usd": self.spec.quota_usd,
                "used": self.quota_used(),
                "by_arm": by_arm,
                "measure": "Claude Code total_cost_usd: list-price value, not a cash charge",
            },
            "trials": trials,
        }


def status_lines(status: dict[str, Any]) -> list[str]:
    """The experiment's status file as a short text summary."""
    from .suite import status_lines as suite_lines

    quota = status.get("quota") or {}
    used = (quota.get("used") or {}).get("usd")
    budget = quota.get("budget_usd")
    lines = [
        f"experiment {status.get('experiment')} ({status.get('profile')}): "
        f"{', '.join(a['id'] for a in status.get('arms') or [])} × "
        f"{len(status.get('tasks') or [])} tasks × {status.get('attempts')} attempts",
        f"  credentials {status.get('credential_source') or 'none needed'} · "
        f"Claude quota ${used or 0:.2f}"
        + (f" of ${budget:.2f}" if budget is not None else " (no budget)"),
    ]
    losses = [loss for t in status.get("trials") or [] for loss in t.get("losses") or []]
    if losses:
        causes: dict[str, int] = {}
        for loss in losses:
            causes[loss["cause"]] = causes.get(loss["cause"], 0) + 1
        lines.append(
            "  lost and rerun: "
            + ", ".join(f"{count} to {cause}" for cause, count in sorted(causes.items()))
        )
    return lines + suite_lines(status)[1:]


__all__ = [
    "DEFAULT_ATTEMPTS",
    "ExperimentError",
    "ExperimentScheduler",
    "Spec",
    "experiment_dir",
    "interleave",
    "job_name",
    "load_spec",
    "pin",
    "read_status",
    "status_lines",
]
