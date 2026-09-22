"""The Harbor adapter for Coder One (``crates/coder-one``).

Coder One implements the same headless episode contract as Coder v0.5,
``openagents.coder.episode.v1``, so this adapter reuses the v0.5
adapter's install, digest checks, doctor, run, and bundle collection.
It differs in three ways: the binary installs as
``/opt/openagents/bin/coder-one``, the Jev key and Coder One's bounds
reach the episode by name, and the arm is named ``coder-one`` so its
results never pool with a v0.5 artifact's.

Coder One is a separate comparison arm, not v0.5: a minimal loop that
asks Jev for typed judgments each step and generates one shell action
through the OpenAgents Open Responses door.

``CoderOneDelegate`` is the delegate-mode arm (issue #9532). It also
installs Claude Code at a pinned version inside the task environment,
the way Harbor's own ``claude-code`` agent does, forwards the Claude
credential by name, and sets ``CODER_ONE_DELEGATE`` so the episode
explores first and then hands the task to Claude Code.
"""

from __future__ import annotations

import shlex
from pathlib import PurePosixPath
from typing import Any, ClassVar

from harbor.environments.base import BaseEnvironment

from tbench.coder_v05 import INSTALL_ROOT, CoderV05, EpisodeContractError


class CoderOne(CoderV05):
    """Run one pinned Coder One Linux binary through the episode contract."""

    BINARY_PATH: ClassVar[PurePosixPath] = INSTALL_ROOT / "bin" / "coder-one"
    EPISODE_ENV: ClassVar[tuple[str, ...]] = (
        "OPENAGENTS_API_KEY",
        "OPENAGENTS_DOOR_URL",
        "OPENAGENTS_MODEL",
        "TYPESAFE_API_KEY",
        "CODER_ONE_JEV",
        "CODER_ONE_MAX_STEPS",
        "CODER_ONE_COMMAND_TIMEOUT",
        "CODER_ONE_DEEP",
    )

    @staticmethod
    def name() -> str:
        return "coder-one"


# The oldest Claude Code the delegate arms accept: the API refuses Opus 5.5
# to 2.1.278 (claude_code_version_too_old).
CLAUDE_CODE_MIN = (2, 1, 280)
DELEGATE_MODES = ("always", "auto")


def _version_tuple(text: str) -> tuple[int, ...] | None:
    word = text.strip().split()[0] if text.strip() else ""
    try:
        return tuple(int(part) for part in word.split("."))
    except ValueError:
        return None


class CoderOneDelegate(CoderOne):
    """Coder One in delegate mode, with Claude Code installed beside it.

    Adapter kwargs, set by the arm's profile:

    - ``delegate``: ``always`` or ``auto``. Required.
    - ``claude_code_version``: the pinned Claude Code, 2.1.280 or newer.
    - ``delegate_model``: the delegate's model, ``claude-opus-5-5`` by
      default.
    - ``delegate_timeout_sec``, ``explore_steps``: optional bounds, passed
      to the episode as ``CODER_ONE_DELEGATE_TIMEOUT`` and
      ``CODER_ONE_EXPLORE_STEPS``.
    """

    EPISODE_ENV: ClassVar[tuple[str, ...]] = CoderOne.EPISODE_ENV + (
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "CODER_ONE_BRIEFING_CAP",
    )

    # Harbor's claude-code agent resolves the CLI this way after its
    # native installer puts it under ~/.local/bin.
    _CLAUDE_PATH_COMMAND = (
        'export PATH="$HOME/.local/bin:$PATH"; '
        'readlink -f "$(command -v claude)" && claude --version'
    )

    @staticmethod
    def name() -> str:
        return "coder-one-delegate"

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        self._delegate = kwargs.pop("delegate", None)
        self._claude_code_version = str(
            kwargs.pop("claude_code_version", None) or "2.1.280"
        )
        self._delegate_model = kwargs.pop("delegate_model", None) or "claude-opus-5-5"
        self._delegate_timeout_sec = kwargs.pop("delegate_timeout_sec", None)
        self._explore_steps = kwargs.pop("explore_steps", None)
        self._claude_bin: str | None = None
        super().__init__(*args, **kwargs)

    def _preflight(self) -> None:
        super()._preflight()
        if self._delegate not in DELEGATE_MODES:
            raise EpisodeContractError(
                f"coder-one-delegate needs delegate=always or delegate=auto, "
                f"not {self._delegate!r}"
            )
        version = _version_tuple(self._claude_code_version)
        if version is None or version < CLAUDE_CODE_MIN:
            raise EpisodeContractError(
                f"claude_code_version {self._claude_code_version!r} is older than "
                f"{'.'.join(map(str, CLAUDE_CODE_MIN))}; the API refuses Opus 5.5 to it"
            )

    def claude_install_command(self) -> str:
        """The command Harbor's claude-code agent installs the CLI with."""
        version = self._claude_code_version
        return (
            "set -euo pipefail; "
            "if command -v apk &> /dev/null; then"
            f"  npm install -g @anthropic-ai/claude-code@{shlex.quote(version)};"
            " else"
            " curl -fsSL https://downloads.claude.ai/claude-code-releases/bootstrap.sh"
            f" | bash -s -- {shlex.quote(version)};"
            " fi && "
            "echo 'export PATH=\"$HOME/.local/bin:$PATH\"' >> ~/.bashrc && "
            'export PATH="$HOME/.local/bin:$PATH" && '
            "claude --version"
        )

    async def install(self, environment: BaseEnvironment) -> None:
        """Install the pinned Claude Code, then Coder One and its doctor."""
        await self.ensure_system_dependencies(
            environment, ("curl", "bash", "nodejs", "npm", "procps")
        )
        await self.exec_as_agent(environment, command=self.claude_install_command())
        found = await environment.exec(command=self._CLAUDE_PATH_COMMAND)
        lines = (found.stdout or "").strip().splitlines()
        if found.return_code != 0 or len(lines) < 2:
            raise EpisodeContractError(
                "Claude Code did not install: "
                f"{(found.stderr or found.stdout or '').strip()[-500:]}"
            )
        installed = lines[-1].split()[0]
        if installed != self._claude_code_version:
            raise EpisodeContractError(
                f"Claude Code {installed} installed, not the pinned "
                f"{self._claude_code_version}"
            )
        self._claude_bin = lines[-2].strip()
        # The episode doctor then checks `claude --version` and the
        # credential from inside the episode's own environment.
        await super().install(environment)

    def _episode_env(self) -> dict[str, str]:
        env = super()._episode_env()
        env["CODER_ONE_DELEGATE"] = self._delegate
        env["CODER_ONE_DELEGATE_MODEL"] = self._delegate_model
        if self._claude_bin:
            env["CODER_ONE_CLAUDE_BIN"] = self._claude_bin
        if self._delegate_timeout_sec:
            env["CODER_ONE_DELEGATE_TIMEOUT"] = str(int(self._delegate_timeout_sec))
        if self._explore_steps:
            env["CODER_ONE_EXPLORE_STEPS"] = str(int(self._explore_steps))
        return env
