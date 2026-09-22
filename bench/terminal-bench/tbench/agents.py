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
    """One way an agent can be authenticated, stated in env-var names."""

    name: str
    requires_any: tuple[str, ...] = ()
    requires_all: tuple[str, ...] = ()
    force_value: dict[str, str] = field(default_factory=dict)
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
            force_value=dict(body.get("force_value") or {}),
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
    )


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
    Variables the mode forces (such as ``CLAUDE_FORCE_OAUTH=1``) are literal.
    """
    env = os.environ if env is None else env
    out: dict[str, str] = {}
    mode = profile.auth_modes.get(auth_mode) if auth_mode else None
    forced = dict(mode.force_value) if mode else {}
    for name in profile.env_forward:
        if name in forced:
            out[name] = forced.pop(name)
        elif env.get(name):
            out[name] = f"${{{name}}}"
    out.update(forced)
    return out
