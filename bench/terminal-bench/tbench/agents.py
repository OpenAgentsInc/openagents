"""Agent profiles: the checked pins each comparison arm runs under.

``profiles/agents.json`` names, for every selectable arm, the Harbor agent
(name or import path), the pinned installed version, the requested model,
adapter kwargs, and the environment variables forwarded into the task
environment *by name*. A profile never carries a credential value: the
materialized job config references host variables as ``${NAME}`` templates,
which Harbor resolves inside its own process and redacts in everything it
persists.
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from . import paths

AGENTS_SCHEMA = "openagents.tbench.agents.v1"


@dataclass(frozen=True)
class AuthMode:
    """One way an agent can be authenticated, stated in env-var names.

    ``model`` overrides the profile's model pin for this mode: provider
    tiers gate models by account type, and a ChatGPT-account session
    cannot serve every model an API key can.

    ``env_overrides`` injects literal values into the job's agent env when
    the host does not already set the variable. ``exclude_env`` strips
    variables from the forwarded whitelist for this mode. Both matter
    because Harbor scrubs every env value whose key names a credential:
    a selector like ``CODEX_FORCE_AUTH_JSON=1`` forwarded literally would
    redact every ``1`` in the retained evidence, so selectors stay on the
    host and the mode forwards the resolved file path instead.
    """

    name: str
    requires_any: tuple[str, ...] = ()
    requires_all: tuple[str, ...] = ()
    env_overrides: dict[str, str] = field(default_factory=dict)
    exclude_env: tuple[str, ...] = ()
    model: str | None = None
    note: str = ""

    def configured(self, env: dict[str, str] | None = None) -> bool:
        """Whether the mode's requirements are present in ``env``.

        Presence of a name is all this checks; a credential's value is never
        read, recorded, or printed.
        """
        env = os.environ if env is None else env
        if self.requires_all and not all(
            env.get(name) for name in self.requires_all
        ):
            return False
        if self.requires_any and not any(
            env.get(name) for name in self.requires_any
        ):
            return False
        return bool(self.requires_all or self.requires_any)


@dataclass(frozen=True)
class AgentProfile:
    """One comparison arm's declared identity."""

    id: str
    role: str  # external-baseline | candidate | control
    harbor_name: str | None
    harbor_import_path: str | None
    version: str | None
    model: str | None
    kwargs: dict[str, Any]
    required_kwargs: tuple[str, ...]
    env_forward: tuple[str, ...]
    auth_modes: dict[str, AuthMode]
    extra_allowed_hosts: tuple[str, ...]
    cost_provenance: str
    notes: tuple[str, ...]
    # ``allowlist`` runs the agent phase with only ``extra_allowed_hosts``
    # reachable, even on a task whose baseline is public; ``harbor`` keeps
    # Harbor's own resolution. Either way the trial records what applied
    # (``tbench.netpolicy``).
    agent_network: str = "harbor"

    @property
    def is_control(self) -> bool:
        return self.role == "control"

    def harbor_selector(self) -> str:
        """What ``harbor run --agent`` or AgentConfig names."""
        return self.harbor_import_path or self.harbor_name or self.id


def _load_auth_modes(raw: dict[str, Any]) -> dict[str, AuthMode]:
    modes = {}
    for name, body in (raw or {}).items():
        modes[name] = AuthMode(
            name=name,
            requires_any=tuple(body.get("requires_any") or ()),
            requires_all=tuple(body.get("requires_all") or ()),
            env_overrides=dict(body.get("env_overrides") or {}),
            exclude_env=tuple(body.get("exclude_env") or ()),
            model=body.get("model"),
            note=body.get("note", ""),
        )
    return modes


def _load_agent(agent_id: str, raw: dict[str, Any]) -> AgentProfile:
    notes = [raw[key] for key in ("note", "cost_note") if raw.get(key)]
    return AgentProfile(
        id=agent_id,
        role=raw.get("role", "candidate"),
        harbor_name=raw.get("harbor_name"),
        harbor_import_path=raw.get("harbor_import_path"),
        version=raw.get("version"),
        model=raw.get("model"),
        kwargs=dict(raw.get("kwargs") or {}),
        required_kwargs=tuple(raw.get("required_kwargs") or ()),
        env_forward=tuple(raw.get("env_forward") or ()),
        auth_modes=_load_auth_modes(raw.get("auth_modes")),
        extra_allowed_hosts=tuple(raw.get("extra_allowed_hosts") or ()),
        cost_provenance=raw.get("cost_provenance", "unknown"),
        notes=tuple(notes),
        agent_network=_agent_network(agent_id, raw),
    )


def _agent_network(agent_id: str, raw: dict[str, Any]) -> str:
    from .netpolicy import MODES

    mode = raw.get("agent_network", "harbor")
    if mode not in MODES:
        raise ValueError(
            f"agent {agent_id!r}: agent_network must be one of {', '.join(MODES)}, not {mode!r}"
        )
    if mode == "allowlist" and not raw.get("extra_allowed_hosts"):
        raise ValueError(
            f"agent {agent_id!r}: agent_network allowlist needs extra_allowed_hosts"
        )
    return mode


def load_agents(path: Path | None = None) -> dict[str, AgentProfile]:
    """Read the checked agent profiles."""
    path = path or (paths.PROFILES_DIR / "agents.json")
    data = json.loads(path.read_text())
    if data.get("schema_version") != AGENTS_SCHEMA:
        raise ValueError(
            f"{path}: schema_version {data.get('schema_version')!r} "
            f"is not {AGENTS_SCHEMA!r}"
        )
    return {
        agent_id: _load_agent(agent_id, raw)
        for agent_id, raw in data["agents"].items()
    }


def model_for(
    profile: AgentProfile, auth_mode: str | None = None
) -> str | None:
    """The model pin in effect: the auth mode's override, else the profile's."""
    mode = profile.auth_modes.get(auth_mode) if auth_mode else None
    if mode and mode.model:
        return mode.model
    return profile.model


def configured_auth_modes(
    profile: AgentProfile, env: dict[str, str] | None = None
) -> list[AuthMode]:
    """The auth modes whose variables are present. Values are never read."""
    return [mode for mode in profile.auth_modes.values() if mode.configured(env)]


def agent_config_env(
    profile: AgentProfile,
    auth_mode: str | None = None,
    env: dict[str, str] | None = None,
) -> dict[str, str]:
    """Build the ``AgentConfig.env`` mapping for one arm.

    Every forwarded variable becomes a ``${NAME}`` template resolved by
    Harbor inside its own process; the materialized config holds no value.
    Mode overrides inject literals — ``~`` is expanded on the host so a
    resolved path is what Harbor scrubs, never a digit — and exclusions
    remove variables the mode must not see (a truthy selector's value
    would otherwise be treated as a credential and corrupt the evidence).
    """
    env = os.environ if env is None else env
    out: dict[str, str] = {}
    mode = profile.auth_modes.get(auth_mode) if auth_mode else None
    excluded = set(mode.exclude_env) if mode else set()
    overrides = dict(mode.env_overrides) if mode else {}
    for name in profile.env_forward:
        if name in excluded:
            continue
        if name in overrides:
            if env.get(name):
                overrides.pop(name)
                out[name] = f"${{{name}}}"
            else:
                out[name] = str(Path(overrides.pop(name)).expanduser())
        elif env.get(name):
            out[name] = f"${{{name}}}"
    for name, value in overrides.items():
        if name not in excluded:
            out[name] = str(Path(value).expanduser())
    return out
