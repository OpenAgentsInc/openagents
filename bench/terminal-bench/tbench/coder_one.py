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

Either arm takes a ``policy`` kwarg: a Coder One policy manifest
(``crates/coder-one/policies/*.json``), relative to the repository root
or absolute. The adapter reads it on the host and passes it inline as
``CODER_ONE_POLICY``; the episode resolves its whole configuration from
it and records the manifest and its digest. For the delegate arm the
manifest also decides which CLI and version install, so the kwargs that
would repeat it are refused when they disagree.

By default the delegate CLI and a pinned Node come from prebuilt toolchain
layers (``tbench.toolchain``): built once on the host, copied into each
trial, and linked into ``/usr/local/bin``, with no package manager or
network in the trial. ``toolchain=network`` keeps the network install, with
Node pinned and at most ``install_concurrency`` installs at a time. The
adapter writes ``toolchain-setup.json`` beside its logs with the mode, the
cold or warm cache state, and the install phases.

``CoderOneTunable`` runs the tunable composition (``control.route``,
``control.handoff``, ``control.horizon``, and ``verify``): its manifest can
dispatch to Claude Code and to Codex in one episode, so it installs both
CLIs from prebuilt layers and places both credentials.

Every arm sizes its episode from the task's own agent timeout: the adapter
reads the trial's ``lock.json`` and the task's ``task.toml`` for Harbor's
timeout, runs the episode process 60 seconds inside it, and the episode
keeps its own deadline 60 seconds inside that.
"""

from __future__ import annotations

import asyncio
import json
import os
import shlex
import time
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any, ClassVar

from harbor.environments.base import BaseEnvironment

from tbench.coder_v05 import INSTALL_ROOT, CoderV05, EpisodeContractError
from tbench.paths import PACKAGE_DIR

# The repository root, which a relative policy path is read against.
REPO_ROOT = PACKAGE_DIR.parent.parent
POLICY_SCHEMA = "openagents.coder-one.policy.v1"


def load_policy(path: str) -> dict[str, Any]:
    """Read a Coder One policy manifest; the episode validates it fully."""
    file = Path(path).expanduser()
    if not file.is_absolute():
        file = REPO_ROOT / file
    try:
        manifest = json.loads(file.read_text())
    except (OSError, ValueError) as exc:
        raise EpisodeContractError(f"cannot read policy {file}: {exc}") from exc
    if not isinstance(manifest, dict) or manifest.get("schema") != POLICY_SCHEMA:
        raise EpisodeContractError(f"policy {file} is not a {POLICY_SCHEMA} manifest")
    return manifest


def harbor_agent_timeout_sec(logs_dir: Path) -> float | None:
    """Harbor's agent timeout for this trial, or ``None`` when unknown.

    Harbor writes the trial's ``lock.json`` before it builds the agent. The
    timeout is the agent override or the task's ``[agent] timeout_sec``,
    capped by ``max_timeout_sec`` and scaled by the multiplier, as Harbor's
    trial resolves it. Only the task's configuration is read, never its
    tests.
    """
    try:
        lock = json.loads((Path(logs_dir).parent / "lock.json").read_text())
    except (OSError, ValueError):
        return None
    if not isinstance(lock, dict):
        return None
    agent = lock.get("agent") or {}
    base = agent.get("override_timeout_sec")
    if base is None:
        task_path = (lock.get("task") or {}).get("path")
        if not task_path:
            return None
        try:
            task = tomllib.loads((Path(task_path) / "task.toml").read_text())
        except (OSError, ValueError):
            return None
        base = (task.get("agent") or {}).get("timeout_sec")
    if not isinstance(base, (int, float)) or base <= 0:
        return None
    ceiling = agent.get("max_timeout_sec")
    if isinstance(ceiling, (int, float)) and ceiling > 0:
        base = min(base, ceiling)
    multiplier = lock.get("agent_timeout_multiplier")
    if multiplier is None:
        multiplier = lock.get("timeout_multiplier", 1.0)
    return float(base) * float(multiplier or 1.0)


from tbench.toolchain import (
    NODE_VERSION,
    ToolchainError,
    place_toolchain,
    setup_record,
    write_setup,
)


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
        "CODER_ONE_EPISODE_DEADLINE",
        "CODER_ONE_SPEND_SOFT_USD",
    )

    # Seconds between the episode's own deadline and the harness's exec
    # timeout, so the episode records what it cut short before the harness
    # kills it.
    DEADLINE_MARGIN_SEC: ClassVar[int] = 60

    @staticmethod
    def name() -> str:
        return "coder-one"

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        policy = kwargs.pop("policy", None)
        self._policy: dict[str, Any] | None = load_policy(policy) if policy else None
        super().__init__(*args, **kwargs)
        # Without an explicit exec timeout, the process runs 60 seconds
        # inside Harbor's own agent timeout, so the episode ends and its
        # bundle is collected before Harbor cancels the run.
        self._harbor_timeout_sec = harbor_agent_timeout_sec(self.logs_dir)
        if not self._episode_timeout_sec and self._harbor_timeout_sec:
            self._episode_timeout_sec = max(
                int(self._harbor_timeout_sec) - self.DEADLINE_MARGIN_SEC,
                2 * self.DEADLINE_MARGIN_SEC,
            )

    def _episode_env(self) -> dict[str, str]:
        env = super()._episode_env()
        if self._policy is not None:
            env["CODER_ONE_POLICY"] = json.dumps(self._policy, separators=(",", ":"))
        # With an exec timeout, the episode runs one deadline inside it.
        if self._episode_timeout_sec and "CODER_ONE_EPISODE_DEADLINE" not in env:
            env["CODER_ONE_EPISODE_DEADLINE"] = str(
                max(self._episode_timeout_sec - self.DEADLINE_MARGIN_SEC, 60)
            )
        return env


    def _check_doctor_report(self, report: str) -> None:
        """Refuses an artifact that ignores the arm's policy manifest.

        An artifact built before policy manifests runs its own defaults and
        says nothing, so an arm pointed at it would measure something else.
        A policy-aware artifact's doctor names the policy it resolved.
        """
        super()._check_doctor_report(report)
        if self._policy is None:
            return
        name = str(self._policy.get("name") or "")
        lines = [line.strip() for line in report.splitlines()]
        if not any(line.startswith(f"policy: {name} ") for line in lines):
            raise EpisodeContractError(
                f"the artifact's doctor didn't report resolving policy {name!r}; "
                "it predates policy manifests or ignored CODER_ONE_POLICY"
            )


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
TOOLCHAIN_MODES = ("prebuilt", "network")

# Network installs share one guard per process: Harbor runs a job's
# trials as tasks in one event loop, and eight concurrent bootstrap
# downloads caused the 2026-09-22 setup timeouts.
_INSTALL_GUARDS: dict[int, asyncio.Semaphore] = {}



def _install_guard(limit: int) -> asyncio.Semaphore:
    guard = _INSTALL_GUARDS.get(limit)
    if guard is None:
        guard = _INSTALL_GUARDS[limit] = asyncio.Semaphore(limit)
    return guard


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
    - ``policy``: a policy manifest. It supplies the mode, the agent, the
      model, the pinned version, and the bounds above, so those kwargs
      must agree with it or be left out.
    - ``toolchain``: ``prebuilt`` (the default) or ``network``.
    - ``install_concurrency``: the most network installs at once, 2 by
      default.
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
        "CLAUDE_CODE_PROMPT_CACHE_TTL",
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
        explicit = {
            key: kwargs.get(key)
            for key in (
                "delegate",
                "delegate_agent",
                "delegate_model",
                "delegate_timeout_sec",
                "explore_steps",
                "codex_version",
                "claude_code_version",
            )
        }
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
        self._toolchain = kwargs.pop("toolchain", None) or "prebuilt"
        self._install_concurrency = int(kwargs.pop("install_concurrency", None) or 2)
        self._claude_bin: str | None = None
        self._codex_bin: str | None = None
        policy = kwargs.get("policy")
        if policy:
            self._from_policy(load_policy(policy), explicit)
        super().__init__(*args, **kwargs)

    def _from_policy(self, manifest: dict[str, Any], explicit: dict[str, Any]) -> None:
        """Take the executor and its bounds from the manifest, refusing a
        kwarg that says something else."""
        policy = manifest.get("policy") or {}
        executor = policy.get("executor") or {}
        control = policy.get("control") or {}
        agent = executor.get("agent")
        derived = {
            "delegate": control.get("delegate"),
            "delegate_agent": agent,
            "delegate_model": executor.get("model"),
            "delegate_timeout_sec": executor.get("deadline_sec"),
            "explore_steps": control.get("explore_steps"),
        }
        if executor.get("version"):
            key = "codex_version" if agent == "codex" else "claude_code_version"
            derived[key] = executor["version"]
        for key, value in derived.items():
            given = explicit.get(key)
            if given is not None and value is not None and str(given) != str(value):
                raise EpisodeContractError(
                    f"{key}={given!r} disagrees with the policy manifest's {value!r}"
                )
        self._delegate = derived["delegate"]
        self._delegate_agent = agent or "claude-code"
        self._delegate_model = derived["delegate_model"] or DEFAULT_MODELS.get(
            self._delegate_agent, ""
        )
        if "codex_version" in derived:
            self._codex_version = str(derived["codex_version"])
        if "claude_code_version" in derived:
            self._claude_code_version = str(derived["claude_code_version"])
        # The manifest carries these; the episode reads them from it.
        self._delegate_timeout_sec = None
        self._explore_steps = None

    def _preflight(self) -> None:
        super()._preflight()
        if self._delegate not in DELEGATE_MODES:
            raise EpisodeContractError(
                f"coder-one-delegate needs delegate=always or delegate=auto, "
                f"not {self._delegate!r}"
            )
        if self._toolchain not in TOOLCHAIN_MODES:
            raise EpisodeContractError(
                f"toolchain must be prebuilt or network, not {self._toolchain!r}"
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
            f"  nvm install {NODE_VERSION} && nvm alias default {NODE_VERSION} && npm -v &&"
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
        # A resumed job reads its agent env back from the stored config,
        # where Harbor masks credential-named values (``/hom****son``). A
        # masked path names nothing, so it counts as unset.
        for explicit in (
            self._get_env("CODEX_AUTH_JSON_PATH"),
            os.environ.get("CODEX_AUTH_JSON_PATH"),
        ):
            if explicit and "*" not in explicit:
                return Path(explicit).expanduser()
        force = self._get_env("CODEX_FORCE_AUTH_JSON") or os.environ.get(
            "CODEX_FORCE_AUTH_JSON", ""
        )
        if force.strip().lower() in _TRUTHY:
            return Path.home() / ".codex" / "auth.json"
        return None

    async def _install_codex_network(self, environment: BaseEnvironment) -> None:
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

    async def _install_toolchain(self, environment: BaseEnvironment) -> None:
        """Place the delegate CLI from prebuilt layers, or install it."""
        record = await self._install_toolchain_for(environment, self._delegate_agent)
        write_setup(self.logs_dir, record)

    def _version_of(self, agent: str) -> str:
        return self._codex_version if agent == "codex" else self._claude_code_version

    async def _install_toolchain_for(
        self, environment: BaseEnvironment, agent: str
    ) -> dict[str, Any]:
        """Place one CLI from prebuilt layers, or install it; its setup record."""
        version = self._version_of(agent)
        started = time.monotonic()
        fallback = None
        if self._toolchain == "prebuilt":
            try:
                return await place_toolchain(self, environment, agent, version)
            except ToolchainError as exc:
                fallback = (
                    f"prebuilt layers unavailable, installed from the network: {exc}"
                )
        async with _install_guard(self._install_concurrency):
            waited = time.monotonic()
            if agent == "codex":
                await self._install_codex_network(environment)
            else:
                await self._install_claude_network(environment)
        finished = time.monotonic()
        return setup_record(
            mode="network",
            platform=None,
            layers=[],
            phases_ms={
                "guard_wait": int((waited - started) * 1000),
                "network_install": int((finished - waited) * 1000),
            },
            versions={agent: version},
            note=fallback,
        )

    async def _install_codex(self, environment: BaseEnvironment) -> None:
        auth = self.codex_auth_path()
        if auth is None or not auth.is_file():
            raise EpisodeContractError(
                "the Codex delegate needs CODEX_AUTH_JSON_PATH or CODEX_FORCE_AUTH_JSON "
                "naming an existing auth.json"
            )
        await self._install_toolchain(environment)
        await self._place_codex(environment, auth)

    async def _place_codex(self, environment: BaseEnvironment, auth: Path) -> None:
        """Check the installed Codex and place its credential."""
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

    async def _install_claude_network(self, environment: BaseEnvironment) -> None:
        await self.ensure_system_dependencies(
            environment, ("curl", "bash", "nodejs", "npm", "procps")
        )
        await self.exec_as_agent(environment, command=self.claude_install_command())

    async def install(self, environment: BaseEnvironment) -> None:
        """Install the pinned delegate CLI, then Coder One and its doctor."""
        if self._delegate_agent == "codex":
            await self._install_codex(environment)
            await super().install(environment)
            return
        await self._install_toolchain(environment)
        await self._check_claude(environment)
        # The episode doctor then checks `claude --version` and the
        # credential from inside the episode's own environment.
        await super().install(environment)

    async def _check_claude(self, environment: BaseEnvironment) -> None:
        """Check the installed Claude Code and remember its path."""
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
        if self._policy is None:
            env["CODER_ONE_DELEGATE"] = self._delegate
            env["CODER_ONE_DELEGATE_AGENT"] = self._delegate_agent
            env["CODER_ONE_DELEGATE_MODEL"] = self._delegate_model
            # The version the adapter installs, so the episode records it
            # and its doctor refuses another.
            env["CODER_ONE_EXECUTOR_VERSION"] = (
                self._codex_version
                if self._delegate_agent == "codex"
                else self._claude_code_version
            )
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


def merge_setup(records: list[dict[str, Any]]) -> dict[str, Any]:
    """One setup record for several CLIs: every layer, each phase under its
    executor's name, and the cache warm only when every layer was."""
    layers = [layer for record in records for layer in record.get("layers") or []]
    phases: dict[str, int] = {}
    versions: dict[str, str] = {}
    notes = []
    for record in records:
        executor = ",".join(sorted((record.get("versions") or {}).keys())) or "executor"
        for name, ms in (record.get("phases_ms") or {}).items():
            phases[f"{executor}:{name}"] = ms
        versions.update(record.get("versions") or {})
        if record.get("note"):
            notes.append(record["note"])
    modes = {record.get("mode") for record in records}
    caches = {layer.get("cache") for layer in layers}
    merged = dict(records[0]) if records else {}
    merged.update(
        {
            "mode": modes.pop() if len(modes) == 1 else "mixed",
            "cache": "none" if not layers else ("warm" if caches == {"warm"} else "cold"),
            "platform": next((r.get("platform") for r in records if r.get("platform")), None),
            "layers": layers,
            "phases_ms": phases,
            "install_ms": sum(phases.values()),
            "versions": versions,
            "note": "; ".join(notes) or None,
        }
    )
    return merged


def manifest_tiers(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    """Every executor a manifest can dispatch to: its own, the route's two
    and its family profiles, the handoff's second, and ``verify.second``'s,
    as ``{agent, model, version}``."""
    policy = manifest.get("policy") or {}
    control = policy.get("control") or {}
    tiers = [policy.get("executor") or {}]
    route = control.get("route") or {}
    tiers += [route[key] for key in ("cheap", "strong") if route.get(key)]
    families = route.get("families") or {}
    tiers += list((families.get("profiles") or {}).values())
    handoff = control.get("handoff") or {}
    if handoff.get("to"):
        tiers.append(handoff["to"])
    second = (policy.get("verify") or {}).get("second") or {}
    tiers += list(second.get("to") or [])
    return [tier for tier in tiers if tier.get("agent")]


class CoderOneTunable(CoderOneDelegate):
    """The tunable composition: route, checks, support, escalation, and
    repair, with both Claude Code and Codex installed.

    Adapter kwargs, set by the arm's profile:

    - ``policy``: a policy manifest. Required. Its executor, its
      ``control.route`` tiers, and its ``control.handoff.to`` decide which
      CLIs install, at the versions each tier pins.
    - ``executors``: more CLIs to install beside the manifest's, as
      ``{agent: version}``, so every arm carries both CLIs. A version must
      agree with the manifest's pin for the same agent.
    - ``toolchain``: ``prebuilt`` (the default) or ``network``.
    - ``install_concurrency``: the most network installs at once.
    """

    EPISODE_ENV: ClassVar[tuple[str, ...]] = CoderOneDelegate.EPISODE_ENV

    @staticmethod
    def name() -> str:
        return "coder-one-tunable"

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        policy = kwargs.get("policy")
        if not policy:
            raise EpisodeContractError("coder-one-tunable needs a policy manifest")
        manifest = load_policy(policy)
        extra = kwargs.pop("executors", None) or {}
        self._agents: dict[str, str] = {}
        tiers = manifest_tiers(manifest) + [
            {"agent": agent, "version": version} for agent, version in extra.items()
        ]
        for tier in tiers:
            agent = tier["agent"]
            version = tier.get("version")
            if agent not in DELEGATE_AGENTS:
                raise EpisodeContractError(
                    f"coder-one-tunable runs claude-code and codex, not {agent!r}"
                )
            if not version:
                raise EpisodeContractError(
                    f"the manifest's {agent} tier pins no version; the adapter installs a pinned CLI"
                )
            known = self._agents.setdefault(agent, str(version))
            if known != str(version):
                raise EpisodeContractError(
                    f"the manifest pins {agent} at both {known} and {version}"
                )
        super().__init__(*args, **kwargs)
        if "codex" in self._agents:
            self._codex_version = self._agents["codex"]
        if "claude-code" in self._agents:
            self._claude_code_version = self._agents["claude-code"]

    def _preflight(self) -> None:
        super()._preflight()
        if "claude-code" in getattr(self, "_agents", {}):
            version = _version_tuple(self._agents["claude-code"])
            if version is None or version < CLAUDE_CODE_MIN:
                raise EpisodeContractError(
                    f"claude-code {self._agents['claude-code']!r} is older than "
                    f"{'.'.join(map(str, CLAUDE_CODE_MIN))}; the API refuses Opus 5.5 to it"
                )

    async def install(self, environment: BaseEnvironment) -> None:
        """Install every CLI the manifest can dispatch to, place their
        credentials, then Coder One and its doctor."""
        auth = None
        if "codex" in self._agents:
            auth = self.codex_auth_path()
            if auth is None or not auth.is_file():
                raise EpisodeContractError(
                    "the Codex tier needs CODEX_AUTH_JSON_PATH or CODEX_FORCE_AUTH_JSON "
                    "naming an existing auth.json"
                )
        records = []
        for agent in sorted(self._agents):
            records.append(await self._install_toolchain_for(environment, agent))
        write_setup(self.logs_dir, merge_setup(records))
        if auth is not None:
            await self._place_codex(environment, auth)
        if "claude-code" in self._agents:
            await self._check_claude(environment)
        # Coder One itself, and its doctor, which checks every CLI the
        # manifest names from inside the episode's environment.
        await CoderOne.install(self, environment)

    async def run(self, instruction: str, environment: BaseEnvironment, context) -> None:
        try:
            await CoderOne.run(self, instruction, environment, context)
        finally:
            if "codex" in self._agents:
                try:
                    await self.exec_as_root(
                        environment, command=f"rm -rf {CODEX_SECRETS} {CODEX_HOME}"
                    )
                except Exception:
                    pass

    def _episode_env(self) -> dict[str, str]:
        env = CoderOne._episode_env(self)
        if self._claude_bin:
            env["CODER_ONE_CLAUDE_BIN"] = self._claude_bin
        if "codex" in self._agents:
            env["CODEX_HOME"] = str(CODEX_HOME)
            if self._codex_bin:
                env["CODER_ONE_CODEX_BIN"] = self._codex_bin
        return env
