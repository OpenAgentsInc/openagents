"""Enforce and record the agent phase's network policy (issue #9589).

Harbor resolves each trial's network plan from the task. The agent phase
takes the task's ``[agent]`` policy or its ``[environment]`` baseline,
which is ``public`` unless the task says otherwise. An agent's
``extra_allowed_hosts`` only widen a policy that is already restricted,
so on a public task Harbor warns "Run-specific allowlist host(s) ... are
ignored because the effective network policy is public", and the agent's
commands have open internet.

``AgentNetworkPlugin`` is a Harbor job plugin (``harbor run --plugin``).
With ``enforce=allowlist``, it narrows a public agent phase to exactly
the agent's ``extra_allowed_hosts``: the model and Jev endpoints. Setup
and the verifier keep the task's baseline, because an install may need
the network and a verifier installs its test tools from it. Harbor's
Docker environment applies the phase policy through its egress-control
sidecar, a transparent proxy with nftables rules that the task container
shares a network namespace with. A trial whose environment can't enforce
an allowlist fails before it starts rather than running open.

With either mode, the plugin writes ``network-policy.json`` into each
trial directory: the policy Harbor resolved, the policy the agent phase
ran under, and whether that phase had public network.
"""

from __future__ import annotations

import dataclasses
import json
import warnings
from pathlib import Path
from typing import Any

RECORD_NAME = "network-policy.json"
RECORD_SCHEMA = "openagents.tbench.network-policy.v1"
PLUGIN = "tbench.netpolicy:AgentNetworkPlugin"
# ``harbor`` keeps Harbor's own resolution and only records it;
# ``allowlist`` narrows a public agent phase to the agent's allowed hosts.
MODES = ("harbor", "allowlist")

_state: dict[str, Any] = {"mode": None, "original": None}


def narrow(policy: Any, hosts: list[str] | tuple[str, ...]) -> Any:
    """The agent phase policy under ``allowlist`` enforcement.

    A public phase becomes an allowlist of exactly ``hosts``. A phase the
    task already restricts keeps Harbor's resolution, which has merged the
    hosts in. With no hosts there is nothing to allow, so the phase is
    left as Harbor resolved it.
    """
    from harbor.models.task.config import NetworkMode, NetworkPolicy

    hosts = list(dict.fromkeys(hosts))
    if not hosts or policy.network_mode != NetworkMode.PUBLIC:
        return policy
    return NetworkPolicy(network_mode=NetworkMode.ALLOWLIST, allowed_hosts=hosts)


def policy_dict(policy: Any) -> dict[str, Any] | None:
    if policy is None:
        return None
    return {
        "network_mode": policy.network_mode.value,
        "allowed_hosts": list(policy.allowed_hosts),
    }


def record(harbor_plan: Any, plan: Any, mode: str, step: str | None = None) -> dict[str, Any]:
    """What one trial's network plan was, and what the agent phase ran under."""
    agent = policy_dict(plan.agent_phase) or {}
    return {
        "schema": RECORD_SCHEMA,
        "enforce": mode,
        "step": step,
        "agent_phase": agent,
        "harbor_agent_phase": policy_dict(harbor_plan.agent_phase),
        "agent_env_baseline": policy_dict(plan.agent_env_baseline),
        "verifier_phase": policy_dict(plan.verifier_phase),
        "agent_phase_public": agent.get("network_mode") == "public",
    }


def _write(trial_dir: Path, body: dict[str, Any]) -> None:
    try:
        (trial_dir / RECORD_NAME).write_text(json.dumps(body, indent=2) + "\n")
    except OSError:
        pass


def install(mode: str) -> None:
    """Route every trial's network plan through the enforcement mode.

    Wraps ``Trial._network_plan``, which every phase reads its policy
    from, once per process; a later call only changes the mode.
    """
    if mode not in MODES:
        raise ValueError(f"enforce must be one of {', '.join(MODES)}, not {mode!r}")
    from harbor.trial.trial import Trial

    _state["mode"] = mode
    if _state["original"] is not None:
        return
    original = Trial._network_plan
    _state["original"] = original

    def _network_plan(self: Any, step_cfg: Any = None, *, env_config: Any = None) -> Any:
        enforcing = _state["mode"] == "allowlist"
        with warnings.catch_warnings():
            if enforcing:
                warnings.filterwarnings(
                    "ignore", message="Run-specific allowlist host", category=UserWarning
                )
            harbor_plan = original(self, step_cfg, env_config=env_config)
        plan = harbor_plan
        if enforcing:
            plan = dataclasses.replace(
                harbor_plan,
                agent_phase=narrow(
                    harbor_plan.agent_phase, self.config.agent.extra_allowed_hosts
                ),
            )
        if env_config is None:
            step = getattr(step_cfg, "name", None)
            _write(self.paths.trial_dir, record(harbor_plan, plan, _state["mode"], step))
        return plan

    Trial._network_plan = _network_plan


def uninstall() -> None:
    """Restore Harbor's own resolution; for tests."""
    if _state["original"] is None:
        return
    from harbor.trial.trial import Trial

    Trial._network_plan = _state["original"]
    _state["original"] = None
    _state["mode"] = None


class AgentNetworkPlugin:
    """A Harbor job plugin: ``--plugin tbench.netpolicy:AgentNetworkPlugin
    --plugin-kwarg enforce=allowlist``."""

    def __init__(self, enforce: str = "harbor", **_: Any) -> None:
        self.enforce = str(enforce)
        install(self.enforce)

    async def on_job_start(self, job: Any) -> None:
        install(self.enforce)

    async def on_job_end(self, job_result: Any) -> None:
        return None


def plugin_args(mode: str) -> list[str]:
    """The ``harbor run`` or ``harbor job resume`` arguments for a mode."""
    if mode not in MODES:
        raise ValueError(f"agent_network must be one of {', '.join(MODES)}, not {mode!r}")
    return ["--plugin", PLUGIN, "--plugin-kwarg", f"enforce={mode}"]


def trial_network(trial_dir: Path) -> dict[str, Any]:
    """A trial's network record for its attempt record.

    A trial without ``network-policy.json`` ran without the plugin, so
    nothing says what its agent phase could reach.
    """
    try:
        body = json.loads((Path(trial_dir) / RECORD_NAME).read_text())
    except (OSError, ValueError):
        return {
            "recorded": False,
            "agent_phase_public": None,
            "note": "no network-policy.json: the trial ran without tbench's network plugin",
        }
    return {
        "recorded": True,
        "enforce": body.get("enforce"),
        "agent_phase": body.get("agent_phase"),
        "harbor_agent_phase": body.get("harbor_agent_phase"),
        "verifier_phase": body.get("verifier_phase"),
        "agent_phase_public": bool(body.get("agent_phase_public")),
    }
