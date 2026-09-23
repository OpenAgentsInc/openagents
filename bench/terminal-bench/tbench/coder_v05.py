"""The Harbor custom-agent adapter for Coder Terminal v0.5.

This adapter is the benchmark's side of the versioned headless episode
contract ``openagents.coder.episode.v1``. It installs one pinned Linux
artifact inside the task environment, runs one headless episode against
it, and collects the episode bundle the contract says the artifact
writes. It never substitutes another executable: a missing binary, a
digest mismatch, an unreachable door, or a missing asset stops the trial
before any inference can be spent.

Contract summary (the normative text lives in
``docs/coder/terminal-bench-contract.md``):

- Install: one artifact, ``artifact_path`` or ``artifact_url`` plus
  ``artifact_sha256``. The digest is checked on the host before upload
  and again inside the environment after upload.
- Assets: ``assets_path`` installs to ``/opt/openagents/assets``, outside
  the task workdir, so packaged registries and question sets never land
  in the workspace the verifier inspects.
- Preflight: ``coder-v05 episode doctor`` runs inside the environment and
  must exit 0. It checks the door reachability and asset presence without
  spending inference.
- Episode: ``coder-v05 episode run --instruction-file F --output-dir D
  --contract openagents.coder.episode.v1`` runs one episode; the artifact
  writes the bundle (``manifest.json``, ``trajectory.atif.json``,
  ``artifacts/``, ``verification/``, ``evaluation/``) under ``D``.
- Accounting: ``evaluation/usage.json`` carries per-model usage with
  provenance; absent usage is ``unknown``, never zero.
- Live tail: while the episode runs, ``tbench.live.LiveTail`` copies the
  new lines of ``episode.atif.jsonl`` to ``live/`` beside the logs every
  ``live_interval_sec`` seconds (10 by default; 0 turns it off), so the
  Gym can follow the trial before the bundle is collected.
"""

from __future__ import annotations

import asyncio
import contextlib
import hashlib
import json
import os
import shlex
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, ClassVar

from harbor.agents.installed.base import (
    BaseInstalledAgent,
    NonZeroAgentExitCodeError,
)
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext
from harbor.models.trajectories import Trajectory

from tbench.live import DEFAULT_INTERVAL_SEC, LOG_NAME, LiveTail

CONTRACT_ID = "openagents.coder.episode.v1"
INSTALL_ROOT = PurePosixPath("/opt/openagents")
BINARY_PATH = INSTALL_ROOT / "bin" / "coder-v05"
ASSETS_PATH = INSTALL_ROOT / "assets"
EPISODE_DIR = INSTALL_ROOT / "episode"
INSTRUCTION_PATH = INSTALL_ROOT / "instruction.txt"


# Exit codes a finished episode reports when it ends without a local
# success; the verifier still grades the environment.
OUTCOME_EXIT_CODES = {
    3: "step_limit",
    4: "generation_failed",
    5: "delegate_failed",
}


class EpisodeContractError(RuntimeError):
    """The artifact broke the episode contract before inference ran."""


class EpisodeTimeoutError(RuntimeError):
    """The episode process hit its exec timeout."""


class ArtifactIdentityError(EpisodeContractError):
    """The pinned artifact's digest did not match."""


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


class CoderV05(BaseInstalledAgent):
    """Run one pinned Coder v0.5 artifact through the episode contract."""

    SUPPORTS_ATIF: bool = True

    # Artifact contents are not secrets; the door key travels by name.
    ENV_VARS: ClassVar = []

    # Where the artifact installs, and which variables reach the episode
    # by name. A subclass for another artifact of the same contract
    # overrides these rather than the install and run paths.
    BINARY_PATH: ClassVar[PurePosixPath] = BINARY_PATH
    EPISODE_ENV: ClassVar[tuple[str, ...]] = (
        "OPENAGENTS_API_KEY",
        "OPENAGENTS_DOOR_URL",
        "OPENAGENTS_MODEL",
    )

    @staticmethod
    def name() -> str:
        return "coder-v05"

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        self._artifact_path = kwargs.pop("artifact_path", None)
        self._artifact_url = kwargs.pop("artifact_url", None)
        self._artifact_sha256 = kwargs.pop("artifact_sha256", None)
        self._artifact_version = kwargs.pop("artifact_version", None)
        self._assets_path = kwargs.pop("assets_path", None)
        self._contract = kwargs.pop("contract", CONTRACT_ID)
        self._episode_timeout_sec = int(
            kwargs.pop("episode_timeout_sec", 0) or 0
        )
        self._live_interval_sec = float(
            kwargs.pop("live_interval_sec", DEFAULT_INTERVAL_SEC) or 0
        )
        super().__init__(*args, **kwargs)
        self._preflight()

    def _preflight(self) -> None:
        """Validate the pin before any environment is created or paid for."""
        if self._contract != CONTRACT_ID:
            raise EpisodeContractError(
                f"adapter implements {CONTRACT_ID}, not {self._contract!r}"
            )
        if not self._artifact_sha256:
            raise EpisodeContractError(
                "coder-v05 needs artifact_sha256; unpinned artifacts are refused"
            )
        if not (self._artifact_path or self._artifact_url):
            raise EpisodeContractError(
                "coder-v05 needs artifact_path or artifact_url"
            )
        if self._artifact_path:
            path = Path(self._artifact_path).expanduser()
            if not path.is_file():
                raise EpisodeContractError(
                    f"artifact not found: {path}; there is no fallback binary"
                )
            digest = _sha256_file(path)
            if digest.lower() != self._artifact_sha256.lower():
                raise ArtifactIdentityError(
                    f"artifact sha256 {digest} != pinned "
                    f"{self._artifact_sha256}; refusing to run an unpinned build"
                )
        if self._assets_path:
            assets = Path(self._assets_path).expanduser()
            if not assets.is_dir():
                raise EpisodeContractError(
                    f"assets_path {assets} is not a directory"
                )

    def version(self) -> str | None:
        return self._artifact_version or "pinned-artifact"

    def get_version_command(self) -> str | None:
        return f"{self.BINARY_PATH} --version"

    def parse_version(self, stdout: str) -> str:
        return stdout.strip().splitlines()[0] if stdout.strip() else "unknown"

    async def install(self, environment: BaseEnvironment) -> None:
        """Stage the pinned artifact and its assets, then preflight them."""
        await self.exec_as_root(
            environment,
            command=f"mkdir -p {INSTALL_ROOT}/bin {EPISODE_DIR} {ASSETS_PATH}",
        )

        with tempfile.TemporaryDirectory() as tmp:
            staged = Path(tmp) / "coder-v05"
            if self._artifact_path:
                staged.write_bytes(
                    Path(self._artifact_path).expanduser().read_bytes()
                )
            else:
                self._fetch_artifact(staged)
            if _sha256_file(staged).lower() != self._artifact_sha256.lower():
                raise ArtifactIdentityError(
                    "staged artifact digest changed between pin and upload"
                )
            await environment.upload_file(str(staged), str(self.BINARY_PATH))

        check = await self.exec_as_root(
            environment,
            command=(
                f"chmod 0755 {self.BINARY_PATH} && "
                f"echo '{self._artifact_sha256}  {self.BINARY_PATH}' | sha256sum -c -"
            ),
        )
        if check.return_code != 0:
            raise ArtifactIdentityError(
                f"in-environment digest check failed: {check.stderr or check.stdout}"
            )

        if self._assets_path:
            await self.exec_as_root(
                environment, command=f"mkdir -p {ASSETS_PATH}"
            )
            await environment.upload_dir(
                str(Path(self._assets_path).expanduser()), str(ASSETS_PATH)
            )

        version = await environment.exec(command=f"{self.BINARY_PATH} --version")
        if version.return_code != 0:
            raise EpisodeContractError(
                f"{self.BINARY_PATH} --version exited {version.return_code}: "
                f"{version.stderr or version.stdout}"
            )

        doctor = await environment.exec(
            command=(
                f"{self.BINARY_PATH} episode doctor --contract {self._contract}"
            ),
            env=self._episode_env(),
        )
        # The doctor's report is install evidence; it prints names and
        # versions, never a credential value.
        try:
            (self.logs_dir / "episode-doctor.txt").write_text(
                f"exit {doctor.return_code}\n{doctor.stdout or ''}{doctor.stderr or ''}"
            )
        except OSError:
            pass
        if doctor.return_code != 0:
            raise EpisodeContractError(
                "episode doctor failed before inference: "
                f"{doctor.stderr or doctor.stdout}"
            )
        self._check_doctor_report(doctor.stdout or "")

    def _check_doctor_report(self, report: str) -> None:
        """Checks the doctor's report against the arm; a subclass refuses
        an artifact that can't run what the arm asks for."""

    def _fetch_artifact(self, target: Path) -> None:
        import urllib.request

        request = urllib.request.Request(
            self._artifact_url, headers={"User-Agent": "tbench-coder-v05"}
        )
        with urllib.request.urlopen(request, timeout=300) as response:
            target.write_bytes(response.read())

    def _episode_env(self) -> dict[str, str]:
        """Forward door credentials by name; values never appear here."""
        env: dict[str, str] = {}
        for name in self.EPISODE_ENV:
            value = self._get_env(name)
            if value:
                env[name] = value
        env["OPENAGENTS_ASSETS_DIR"] = str(ASSETS_PATH)
        env["OPENAGENTS_EPISODE_CONTRACT"] = self._contract
        return env

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        rendered = self.render_instruction(instruction)
        with tempfile.NamedTemporaryFile(
            "w", suffix=".txt", delete=False
        ) as handle:
            handle.write(rendered)
            local_instruction = handle.name
        try:
            await environment.upload_file(
                local_instruction, str(INSTRUCTION_PATH)
            )
        finally:
            os.unlink(local_instruction)

        command = " ".join(
            [
                str(self.BINARY_PATH),
                "episode",
                "run",
                "--instruction-file",
                shlex.quote(str(INSTRUCTION_PATH)),
                "--output-dir",
                shlex.quote(str(EPISODE_DIR)),
                "--contract",
                shlex.quote(self._contract),
            ]
        )
        if self.model_name:
            command += f" --model {shlex.quote(self.model_name)}"

        tail = None
        follower = None
        if self._live_interval_sec > 0:
            tail = LiveTail(
                environment,
                str(EPISODE_DIR / LOG_NAME),
                self.logs_dir / "live",
                self._live_interval_sec,
            )
            follower = asyncio.create_task(tail.follow())
        try:
            result = await environment.exec(
                command=command,
                env=self._episode_env(),
                timeout_sec=self._episode_timeout_sec or None,
            )
        except RuntimeError as exc:
            if "timed out" in str(exc):
                raise EpisodeTimeoutError(str(exc)) from exc
            raise
        finally:
            if follower is not None:
                follower.cancel()
                with contextlib.suppress(asyncio.CancelledError):
                    await follower
            if tail is not None:
                with contextlib.suppress(Exception):
                    await tail.finish()
            await self._collect_bundle(environment)

        # An outcome code says the episode ran to its end without a local
        # success: it hit its step limit (3), its generation failed (4), or
        # its delegate didn't answer (5). The work in the environment is
        # still graded, as Harbor grades other agents whatever their CLI
        # exits with; the code is kept as evidence. Any other nonzero exit
        # is a crash and ends the trial as an agent error.
        if result.return_code in OUTCOME_EXIT_CODES:
            with contextlib.suppress(OSError):
                (self.logs_dir / "episode-exit.txt").write_text(
                    f"exit {result.return_code}: "
                    f"{OUTCOME_EXIT_CODES[result.return_code]}\n"
                )
        elif result.return_code != 0:
            raise NonZeroAgentExitCodeError(
                f"episode exited {result.return_code}: "
                f"{(result.stderr or result.stdout or '')[-2000:]}"
            )

    async def _collect_bundle(self, environment: BaseEnvironment) -> None:
        """Pull the episode bundle home before the environment can be deleted."""
        try:
            await environment.download_dir(
                str(EPISODE_DIR), self.logs_dir / "episode"
            )
        except Exception as exc:  # a failed collection is evidence too
            marker = self.logs_dir / "episode-collection-failed.txt"
            marker.write_text(f"{exc}\n")

    def populate_context_post_run(self, context: AgentContext) -> None:
        """Fold the retained bundle into Harbor's context and trajectory."""
        bundle = self.logs_dir / "episode"
        usage = self._read_json(bundle / "evaluation" / "usage.json")
        if usage:
            tokens = usage.get("tokens") or {}
            context.n_input_tokens = tokens.get("input")
            context.n_cache_tokens = tokens.get("cache")
            context.n_output_tokens = tokens.get("output")
            cost = usage.get("cost") or {}
            context.cost_usd = cost.get("amount_usd")

        trajectory_doc = self._read_json(bundle / "trajectory.atif.json")
        if trajectory_doc is None:
            return
        try:
            trajectory = Trajectory.model_validate(trajectory_doc)
        except ValueError:
            return
        trajectory_path = self.logs_dir / "trajectory.json"
        trajectory_path.write_text(
            json.dumps(trajectory.to_json_dict(), indent=2, ensure_ascii=False)
        )

    @staticmethod
    def _read_json(path: Path) -> dict[str, Any] | None:
        try:
            return json.loads(path.read_text())
        except (OSError, json.JSONDecodeError):
            return None
