"""``tbench envstart``: a trial's environments, with no agent and no verifier.

Issue #9631 asks where a trial's time goes before and after its agent
runs. ``envstart`` answers that without an agent run, a model call, or a
verifier: it builds a Harbor trial for a task exactly as a suite job
would, with the arm's allowed hosts and ``tbench.netpolicy``'s allowlist
enforcement, and then drives only the environment phases through Harbor's
own trial methods:

1. ``env``: the agent environment's start (image build or reuse, the
   egress sidecar, and ``docker compose up``).
2. ``policy``: switching the agent phase to the allowlist and back. With
   ``--probe``, the probe from ``tbench.netprobe`` runs in between and the
   result says whether the allowlist held.
3. ``artifact``: with ``--artifact``, the Coder One binary's upload and its
   in-environment digest check, as the adapter installs it.
4. ``env_stop``: the agent environment's stop.
5. ``verifier_env`` and ``verifier_stop``: a task with a separate verifier
   starts and stops that environment too. No test runs.

``--mode harbor`` uses ``tbench.warm_docker.TimedDockerEnvironment``,
which starts and stops as Harbor does; ``--mode warm`` uses
``WarmDockerEnvironment``, which starts from kept images and stops with a
one-second grace. Each start's own ``phases_ms`` come from the
environment's ``tbench-environment.jsonl``.

Every trial is named ``tbench-envstart-<task>-<hex>``, so its compose
projects and containers are easy to find, and Harbor removes them, their
networks, their volumes, and any image it built for them when the
environment stops. The kept ``tbench-warm/*`` images are the cache.
Results go to ``<state>/envstart/<stamp>/envstart.json``.
"""

from __future__ import annotations

import asyncio
import contextlib
import hashlib
import json
import shlex
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any

from . import looptime, netpolicy, paths

SCHEMA = "openagents.tbench.envstart.v1"
MODES = {
    "harbor": "tbench.warm_docker:TimedDockerEnvironment",
    "warm": "tbench.warm_docker:WarmDockerEnvironment",
}
PHASES = ("env", "policy", "probe", "artifact", "env_stop", "verifier_env", "verifier_stop")
ARTIFACT_PATH = PurePosixPath("/opt/openagents/bin/coder-one")


def trial_name(task: str) -> str:
    return f"tbench-envstart-{task}-{uuid.uuid4().hex[:8]}"


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _ms(started: float) -> int:
    return int((time.monotonic() - started) * 1000)


async def _probe(environment: Any) -> dict[str, Any]:
    """The netprobe agent's requests, from inside the agent environment."""
    from .netprobe import _PROBE, BLOCKED_URLS, parse_probe, verdict

    policy_obj = environment.network_policy
    policy = {
        "network_mode": policy_obj.network_mode.value,
        "allowed_hosts": list(policy_obj.allowed_hosts),
    }
    urls = [(f"https://{host}/", "reachable") for host in policy["allowed_hosts"]] + [
        (url, "unreachable") for url in BLOCKED_URLS
    ]
    script = _PROBE + "\n".join(f"probe {shlex.quote(url)}" for url, _ in urls)
    result = await environment.exec(command=f"sh -c {shlex.quote(script)}", timeout_sec=180)
    expected = dict(urls)
    rows = parse_probe(result.stdout or "")
    for row in rows:
        row["expected"] = expected.get(row["url"], "unknown")
    return {"policy": policy, "results": rows, "verdict": verdict(policy, rows)}


async def _place_artifact(environment: Any, artifact: Path, digest: str) -> None:
    """The Coder One adapter's upload and in-environment digest check."""
    await environment.exec(command=f"mkdir -p {ARTIFACT_PATH.parent}", user="root")
    await environment.upload_file(str(artifact), str(ARTIFACT_PATH))
    check = await environment.exec(
        command=(
            f"chmod 0755 {ARTIFACT_PATH} && "
            f"echo '{digest}  {ARTIFACT_PATH}' | sha256sum -c -"
        ),
        user="root",
    )
    if check.return_code != 0:
        raise RuntimeError(f"digest check failed: {check.stderr or check.stdout}")


async def start_once(
    task_dir: Path,
    *,
    mode: str,
    out_dir: Path,
    allowed_hosts: list[str],
    probe: bool = False,
    artifact: Path | None = None,
    verifier: bool = True,
) -> dict[str, Any]:
    """One environment-only startup of ``task_dir``; its phase times."""
    from harbor.models.task.verifier_mode import (
        VerifierEnvironmentMode,
        resolve_effective_verifier_env_config,
        resolve_task_verifier_mode,
    )
    from harbor.models.trial.config import (
        AgentConfig,
        EnvironmentConfig,
        TaskConfig,
        TrialConfig,
        VerifierConfig,
    )
    from harbor.trial.trial import Trial

    netpolicy.install("allowlist" if allowed_hosts else "harbor")
    name = trial_name(task_dir.name)
    config = TrialConfig(
        task=TaskConfig(path=task_dir),
        trial_name=name,
        trials_dir=out_dir,
        agent=AgentConfig(name="nop", extra_allowed_hosts=allowed_hosts),
        environment=EnvironmentConfig(import_path=MODES[mode], delete=True),
        verifier=VerifierConfig(disable=True),
    )
    row: dict[str, Any] = {
        "task": task_dir.name,
        "mode": mode,
        "trial": name,
        "ms": {},
        "error": None,
    }
    ms = row["ms"]
    trial = await Trial.create(config)
    environment = trial.agent_environment
    started_env = False
    try:
        started = time.monotonic()
        started_env = True
        await trial._start_agent_environment()
        ms["env"] = _ms(started)

        plan = trial._network_plan(None)
        row["agent_phase"] = netpolicy.policy_dict(plan.agent_phase)
        started = time.monotonic()
        async with trial._phase_network_policy(
            environment,
            baseline_policy=plan.agent_env_baseline,
            phase_policy=plan.agent_phase,
        ):
            ms["policy"] = _ms(started)
            if probe:
                probing = time.monotonic()
                row["probe"] = await _probe(environment)
                ms["probe"] = _ms(probing)
            if artifact is not None:
                placing = time.monotonic()
                await _place_artifact(environment, artifact, _sha256(artifact))
                ms["artifact"] = _ms(placing)
            restoring = time.monotonic()
        ms["policy"] += _ms(restoring)

        started = time.monotonic()
        await environment.stop(delete=True)
        started_env = False
        ms["env_stop"] = _ms(started)

        if verifier and resolve_task_verifier_mode(trial.task.config) == (
            VerifierEnvironmentMode.SEPARATE
        ):
            env_config = resolve_effective_verifier_env_config(trial.task.config, None)
            vplan = trial._network_plan(None, env_config=env_config)
            started = time.monotonic()
            async with trial._separate_verifier_env(
                env_config, key="trial", plan=vplan
            ):
                ms["verifier_env"] = _ms(started)
                stopping = time.monotonic()
            ms["verifier_stop"] = _ms(stopping)
    except Exception as exc:  # the row keeps the failure as evidence
        row["error"] = f"{type(exc).__name__}: {str(exc)[:500]}"
    finally:
        if started_env:
            with contextlib.suppress(Exception):
                await environment.stop(delete=True)
        with contextlib.suppress(Exception):
            trial._close_logger_handler()
    trial_dir = out_dir / name
    row["records"] = looptime.environment_records(trial_dir)
    row["setup_ms"] = sum(ms.get(key, 0) for key in ("env", "verifier_env"))
    row["teardown_ms"] = sum(ms.get(key, 0) for key in ("env_stop", "verifier_stop"))
    return row


def out_root(now: datetime | None = None) -> Path:
    stamp = (now or datetime.now(timezone.utc)).strftime("%Y%m%dT%H%M%SZ")
    return paths.state_dir() / "envstart" / stamp


def run(
    task_dirs: list[Path],
    *,
    modes: list[str],
    repeat: int,
    allowed_hosts: list[str],
    probe: bool = False,
    artifact: Path | None = None,
    verifier: bool = True,
    out_dir: Path | None = None,
) -> dict[str, Any]:
    """Every startup, one at a time: each task, each mode, ``repeat`` times."""
    out_dir = out_dir or out_root()
    out_dir.mkdir(parents=True, exist_ok=True)
    rows = []
    for task_dir in task_dirs:
        for mode in modes:
            for index in range(repeat):
                row = asyncio.run(
                    start_once(
                        task_dir,
                        mode=mode,
                        out_dir=out_dir,
                        allowed_hosts=allowed_hosts,
                        probe=probe,
                        artifact=artifact,
                        verifier=verifier,
                    )
                )
                row["repeat"] = index
                rows.append(row)
                print(render_row(row), flush=True)
    report = {
        "schema": SCHEMA,
        "at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "allowed_hosts": allowed_hosts,
        "artifact": str(artifact) if artifact else None,
        "rows": rows,
    }
    (out_dir / "envstart.json").write_text(json.dumps(report, indent=2) + "\n")
    report["path"] = str(out_dir / "envstart.json")
    return report


def _seconds(value: int | None) -> str:
    return "-" if value is None else f"{value / 1000:.1f}s"


def render_row(row: dict[str, Any]) -> str:
    starts = [r for r in row.get("records") or [] if r.get("event", "start") == "start"]
    caches = ",".join(f"{r.get('role')}={r.get('cache')}" for r in starts) or "-"
    cells = " ".join(f"{key} {_seconds(row['ms'].get(key))}" for key in PHASES if key in row["ms"])
    held = ""
    if row.get("probe"):
        verdict = row["probe"]["verdict"]
        held = (
            f" allowlist {'held' if verdict['allowlist_enforced'] else 'LEAKED'}"
            f" ({verdict['reached_allowed']}/{verdict['allowed']} allowed reached)"
        )
    error = f" error {row['error']}" if row.get("error") else ""
    return f"{row['task']} [{row['mode']} #{row.get('repeat', 0)}] {cells} ({caches}){held}{error}"
