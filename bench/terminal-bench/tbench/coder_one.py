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
installs the delegate CLI at a pinned version inside the task
environment and sets ``CODER_ONE_DELEGATE`` so the episode explores
first and then hands the task over. For Claude Code it installs the CLI
the way Harbor's own ``claude-code`` agent does and forwards the Claude
credential by name. For Codex it installs the CLI and places the host's
``auth.json`` the way Harbor's own ``codex`` agent does, and removes the
file after the run.
"""

from __future__ import annotations

import os
import shlex
from pathlib import Path, PurePosixPath
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
        "CODER_ONE_PROBES",
    )

    @staticmethod
    def name() -> str:
        return "coder-one"


# The oldest Claude Code the delegate arms accept: the API refuses Opus 5.5
# to 2.1.278 (claude_code_version_too_old).
CLAUDE_CODE_MIN = (2, 1, 280)
DELEGATE_MODES = ("always", "auto")
DELEGATE_AGENTS = ("claude-code", "codex")
DEFAULT_MODELS = {"claude-code": "claude-opus-5-5", "codex": "gpt-6-luna"}

# Where Codex's home and its credential live in the container, as in
# Harbor's codex agent. The secrets directory is removed after the run.
CODEX_HOME = PurePosixPath("/tmp/codex-home")
CODEX_SECRETS = PurePosixPath("/tmp/codex-secrets")
CODEX_BIN = PurePosixPath("/usr/local/bin/codex")
_TRUTHY = ("1", "true", "yes", "on")


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
    - ``delegate_agent``: ``claude-code`` (the default) or ``codex``.
    - ``claude_code_version``: the pinned Claude Code, 2.1.280 or newer.
    - ``codex_version``: the pinned Codex CLI, 0.155.1 by default.
    - ``delegate_model``: the delegate's model, ``claude-opus-5-5`` for
      Claude Code and ``gpt-6-luna`` for Codex by default.
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
        "CODER_ONE_DELEGATE_TOOLS",
        "CODER_ONE_DELEGATE_EFFORT",
        "CODER_ONE_PROBE_V2",
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
        self._delegate_agent = kwargs.pop("delegate_agent", None) or "claude-code"
        self._codex_version = str(kwargs.pop("codex_version", None) or "0.155.1")
        self._claude_code_version = str(
            kwargs.pop("claude_code_version", None) or "2.1.280"
        )
        self._delegate_model = kwargs.pop("delegate_model", None) or DEFAULT_MODELS.get(
            self._delegate_agent, ""
        )
        self._delegate_timeout_sec = kwargs.pop("delegate_timeout_sec", None)
        self._explore_steps = kwargs.pop("explore_steps", None)
        self._claude_bin: str | None = None
        self._codex_bin: str | None = None
        super().__init__(*args, **kwargs)

    def _preflight(self) -> None:
        super()._preflight()
        if self._delegate not in DELEGATE_MODES:
            raise EpisodeContractError(
                f"coder-one-delegate needs delegate=always or delegate=auto, "
                f"not {self._delegate!r}"
            )
        if self._delegate_agent not in DELEGATE_AGENTS:
            raise EpisodeContractError(
                f"delegate_agent must be claude-code or codex, not {self._delegate_agent!r}"
            )
        if self._delegate_agent == "codex":
            return
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

    def codex_install_command(self) -> str:
        """The command Harbor's codex agent installs the CLI with."""
        spec = shlex.quote(f"@openai/codex@{self._codex_version}")
        return (
            "set -euo pipefail; "
            "if ldd --version 2>&1 | grep -qi musl || [ -f /etc/alpine-release ]; then"
            f"  npm install -g {spec};"
            " else"
            "  curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.2/install.sh | bash &&"
            '  export NVM_DIR="$HOME/.nvm" &&'
            '  \\. "$NVM_DIR/nvm.sh" || true &&'
            "  command -v nvm &>/dev/null || { echo 'Error: NVM failed to load' >&2; exit 1; } &&"
            "  nvm install 22 && nvm alias default 22 && npm -v &&"
            f"  npm install -g {spec};"
            " fi && "
            "codex --version"
        )

    def codex_auth_path(self) -> Path | None:
        """The host auth.json to place, chosen as Harbor's codex agent does.

        ``CODEX_AUTH_JSON_PATH`` names a file; a truthy
        ``CODEX_FORCE_AUTH_JSON`` selects ``~/.codex/auth.json``. The path
        is read; the file's contents never are.
        """
        explicit = self._get_env("CODEX_AUTH_JSON_PATH") or os.environ.get(
            "CODEX_AUTH_JSON_PATH"
        )
        if explicit:
            return Path(explicit).expanduser()
        force = self._get_env("CODEX_FORCE_AUTH_JSON") or os.environ.get(
            "CODEX_FORCE_AUTH_JSON", ""
        )
        if force.strip().lower() in _TRUTHY:
            return Path.home() / ".codex" / "auth.json"
        return None

    async def _install_codex(self, environment: BaseEnvironment) -> None:
        auth = self.codex_auth_path()
        if auth is None or not auth.is_file():
            raise EpisodeContractError(
                "the Codex delegate needs CODEX_AUTH_JSON_PATH or CODEX_FORCE_AUTH_JSON "
                "naming an existing auth.json"
            )
        await self.ensure_system_dependencies(
            environment, ("curl", "bash", "nodejs", "npm", "ripgrep")
        )
        await self.exec_as_agent(
            environment,
            command=self.codex_install_command(),
            env={"NVM_NODEJS_ORG_MIRROR": "https://nodejs.org/dist"},
        )
        # The episode runs outside nvm's shell setup, so node and codex go
        # on the default PATH, as Harbor's codex agent links them.
        await self.exec_as_root(
            environment,
            command=(
                "if [ -s ~/.nvm/nvm.sh ]; then . ~/.nvm/nvm.sh; fi; "
                "for bin in node codex; do"
                '  BIN_PATH="$(which "$bin" 2>/dev/null || true)";'
                '  if [ -n "$BIN_PATH" ] && [ "$BIN_PATH" != "/usr/local/bin/$bin" ]; then'
                '    ln -sf "$BIN_PATH" "/usr/local/bin/$bin";'
                "  fi;"
                " done"
            ),
        )
        found = await environment.exec(command=f"{CODEX_BIN} --version")
        words = (found.stdout or "").strip().split()
        if found.return_code != 0 or len(words) < 2:
            raise EpisodeContractError(
                "Codex CLI did not install: "
                f"{(found.stderr or found.stdout or '').strip()[-500:]}"
            )
        if words[-1] != self._codex_version:
            raise EpisodeContractError(
                f"Codex CLI {words[-1]} installed, not the pinned {self._codex_version}"
            )
        self._codex_bin = str(CODEX_BIN)
        remote_auth = CODEX_SECRETS / "auth.json"
        await self.exec_as_root(
            environment, command=f"mkdir -p {CODEX_HOME} {CODEX_SECRETS}"
        )
        await environment.upload_file(str(auth), str(remote_auth))
        owner = (
            f"chown {environment.default_user} {remote_auth} {CODEX_HOME} && "
            if environment.default_user is not None
            else ""
        )
        await self.exec_as_root(
            environment,
            command=(
                f"{owner}chmod 600 {remote_auth} && "
                f"ln -sf {remote_auth} {CODEX_HOME / 'auth.json'}"
            ),
        )

    async def install(self, environment: BaseEnvironment) -> None:
        """Install the pinned delegate CLI, then Coder One and its doctor."""
        if self._delegate_agent == "codex":
            await self._install_codex(environment)
            await super().install(environment)
            return
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

    async def run(self, instruction: str, environment: BaseEnvironment, context) -> None:
        try:
            await super().run(instruction, environment, context)
        finally:
            if self._delegate_agent == "codex":
                # The credential leaves the container with the run; the
                # bundle under /opt/openagents/episode never held it.
                try:
                    await self.exec_as_root(
                        environment, command=f"rm -rf {CODEX_SECRETS} {CODEX_HOME}"
                    )
                except Exception:
                    pass

    def _episode_env(self) -> dict[str, str]:
        env = super()._episode_env()
        env["CODER_ONE_DELEGATE"] = self._delegate
        env["CODER_ONE_DELEGATE_AGENT"] = self._delegate_agent
        env["CODER_ONE_DELEGATE_MODEL"] = self._delegate_model
        if self._claude_bin:
            env["CODER_ONE_CLAUDE_BIN"] = self._claude_bin
        if self._delegate_agent == "codex":
            env["CODEX_HOME"] = str(CODEX_HOME)
            if self._codex_bin:
                env["CODER_ONE_CODEX_BIN"] = self._codex_bin
        if self._delegate_timeout_sec:
            env["CODER_ONE_DELEGATE_TIMEOUT"] = str(int(self._delegate_timeout_sec))
        # Zero is a real bound: it skips the explore phase entirely.
        if self._explore_steps is not None:
            env["CODER_ONE_EXPLORE_STEPS"] = str(int(self._explore_steps))
        return env
