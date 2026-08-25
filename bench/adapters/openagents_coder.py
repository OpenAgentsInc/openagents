"""Harbor installed-agent adapter for `openagents coder`.

OpenAgentsInc/openagents#35, first slice. Loads out-of-tree by import path:

    PYTHONPATH=bench harbor run \
      --dataset terminal-bench@2.0 \
      --agent adapters.openagents_coder:OpenAgentsCoder \
      --model openai/gpt-5.6-luna \
      --n-concurrent 1

The coder runs headless (`--plain`) on its thread lane against an
OpenAgents server: `OPENAGENTS_TOKEN` carries authority and
`OPENAGENTS_CODER_API_URL` names the server (default
`http://host.docker.internal:4000`, the dev forge on the container's host,
which needs `PHX_LISTEN_ALL=true`). The server holds the provider keys; the
container holds only the scoped OpenAgents token. Harbor's `--model`
provider prefix is dropped: `openai/gpt-5.6-luna` becomes catalog id
`gpt-5.6-luna` on `POST /api/v3/threads`.

The CLI installs from a tarball packed beside this file (`npm pack` in
`packages/openagents-cli`), so the run measures the working tree, not the
published npm version. The trajectory is the coder's own ATIF export: the
run pipes `/export` after the instruction and copies the newest export to
the trial's `trajectory.json`.

When `OPENAGENTS_GYM_RUN_ID` is set (`bench/run-suite.sh` registers the run
against the Gym lifecycle API, OpenAgentsInc/openagents#38), the adapter
reports each trial to the run from the host side: state `running` at
agent-phase start, the thread link as soon as the coder announces it (a
watcher polls the host-mirrored `coder.txt` for the `[oa:thread <uuid>]`
line while the agent runs, so the transcript is watchable live on /gym),
and a final backstop report after the agent phase. Reporting never fails a
trial: a refused or unreachable Gym is logged and the trial continues.
"""

import asyncio
import contextlib
import json
import os
import re
import shlex
import sys
import urllib.request
from pathlib import Path
from typing import override

from harbor.agents.installed.base import BaseInstalledAgent, with_prompt_template
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

def _find_tarball() -> Path:
    """The packed CLI, whatever version it is.

    Pinning the version here meant a version bump silently broke every graded
    run: the adapter looked for a tarball nobody packs any more and every trial
    errored in install with a message about the old number. The pack step
    produces exactly one tarball, so find it rather than predict its name; if
    there are several, take the newest, because that is the one just built.
    """
    bench = Path(__file__).resolve().parent.parent
    candidates = sorted(
        bench.glob("openagentsinc-cli-*.tgz"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    return candidates[0] if candidates else bench / "openagentsinc-cli-<version>.tgz"


_TARBALL = _find_tarball()
_REMOTE_TARBALL = "/installed-agent/openagents-cli.tgz"
_DEFAULT_API_URL = "http://host.docker.internal:4000"
_EXPORT_DIR = "$HOME/.openagents/exports"

# The coder's --plain thread announcement (OpenAgentsInc/openagents#39). The
# format is a contract between the CLI and this adapter; do not loosen it.
_THREAD_LINE = re.compile(r"\[oa:thread ([0-9a-fA-F-]{36})\]")


class OpenAgentsCoder(BaseInstalledAgent):
    SUPPORTS_ATIF = True

    @staticmethod
    @override
    def name() -> str:
        return "openagents-coder"

    @override
    def get_version_command(self) -> str | None:
        return "openagents --version"

    @property
    def _api_url(self) -> str:
        return os.environ.get("OPENAGENTS_CODER_API_URL", _DEFAULT_API_URL)

    @property
    def _token(self) -> str:
        token = os.environ.get("OPENAGENTS_TOKEN", "")
        if not token and not self._local_lane:
            raise ValueError(
                "OPENAGENTS_TOKEN is not set. The coder's thread lane needs a "
                "chat:account token for the server at OPENAGENTS_CODER_API_URL."
            )
        return token

    @property
    def _catalog_model(self) -> str | None:
        if not self.model_name:
            return None
        # Harbor spells models provider/name; the catalog id is the name.
        # An `ollama/<name>` model selects the coder's local lane instead:
        # the CLI's `--model ollama:<name>` shape, answered by an Ollama
        # server the container reaches on its host.
        provider, _, name = self.model_name.partition("/")
        if provider == "ollama":
            return f"ollama:{name}"
        return name or provider

    @property
    def _local_lane(self) -> bool:
        return bool(self.model_name) and self.model_name.startswith("ollama/")

    @property
    def _task_name(self) -> str:
        # `logs_dir` is the host-side `<job>/<trial>/agent` directory, and
        # Harbor names the trial directory `<task>__<shortuuid>`. The task
        # half is the upsert key the run's trials endpoint keeps per task.
        trial = Path(self.logs_dir).parent.name
        return trial.rsplit("__", 1)[0] or trial

    def _thread_id_from_log(self) -> str | None:
        """The thread id the coder announced, or None when it ran offline."""
        try:
            text = (Path(self.logs_dir) / "coder.txt").read_text(errors="replace")
        except OSError:
            return None
        match = _THREAD_LINE.search(text)
        return match.group(1) if match else None

    def _report_trial(self, state: str, thread_id: str | None = None) -> None:
        """Report this trial to the registered Gym run, from the host side.

        `run()` executes on the host, so the POST goes to the host-side
        `OPENAGENTS_GYM_API_URL` on `OPENAGENTS_TOKEN`. Reporting must never
        fail the trial: every failure is logged and swallowed.
        """
        run_id = os.environ.get("OPENAGENTS_GYM_RUN_ID", "")
        api_url = os.environ.get("OPENAGENTS_GYM_API_URL", "")
        token = os.environ.get("OPENAGENTS_TOKEN", "")
        if not run_id or not api_url or not token:
            return
        payload: dict[str, str] = {"task": self._task_name, "state": state}
        if thread_id:
            payload["thread_id"] = thread_id
        request = urllib.request.Request(
            f"{api_url}/api/v1/gym/runs/{run_id}/trials",
            data=json.dumps(payload).encode(),
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Bearer {token}",
            },
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=10):
                pass
        except Exception as error:  # noqa: BLE001 - reporting never fails the trial
            print(f"gym trial report failed for {self._task_name}: {error}", file=sys.stderr)

    @override
    async def install(self, environment: BaseEnvironment) -> None:
        if not _TARBALL.exists():
            raise FileNotFoundError(
                f"CLI tarball missing at {_TARBALL}. Run `pnpm build && npm pack "
                "--pack-destination ../../bench` in packages/openagents-cli first."
            )

        await self.ensure_system_dependencies(environment, ("curl", "bash"))

        # Node >= 20 (the CLI's engines floor). Task images rarely carry it,
        # so provision Node 22 through NodeSource on apt images and fail
        # loudly elsewhere rather than running on a node that cannot.
        await self.exec_as_root(
            environment,
            command=(
                "set -euo pipefail; "
                "if command -v node >/dev/null 2>&1 && "
                '[ "$(node -e \'process.stdout.write(String(process.versions.node.split(".")[0]))\')" -ge 20 ]; '
                "then echo 'node present'; "
                "elif command -v apt-get >/dev/null 2>&1; then "
                "curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && "
                "apt-get install -y nodejs; "
                "else echo 'no node >= 20 and no apt-get' >&2; exit 1; fi; "
                "node --version"
            ),
        )

        await environment.upload_file(_TARBALL, _REMOTE_TARBALL)
        await self.exec_as_root(
            environment,
            command=f"npm install -g {shlex.quote(_REMOTE_TARBALL)} && openagents --version",
        )

    @override
    @with_prompt_template
    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        model_flag = ""
        if self._catalog_model:
            model_flag = f"--model {shlex.quote(self._catalog_model)} "

        # The instruction goes down stdin, then /export so the session writes
        # its ATIF trajectory before exiting on end-of-file. stdin carries the
        # instruction verbatim; nothing here re-quotes its content.
        command = (
            "set -uo pipefail; "
            f"printf '%s\\n/export\\n' {shlex.quote(instruction)} | "
            "openagents coder --plain "
            f"--api-url {shlex.quote(self._api_url)} "
            f"{model_flag}"
            f"2>&1 | tee {shlex.quote(str(self.environment_logs_dir))}/coder.txt; "
            "status=$?; "
            f"latest=$(ls -t {_EXPORT_DIR}/*.json 2>/dev/null | head -1 || true); "
            'if [ -n "$latest" ]; then '
            f'cp "$latest" {shlex.quote(str(self.environment_logs_dir))}/trajectory.json; '
            "fi; "
            "exit $status"
        )

        env = {"OPENAGENTS_TOKEN": self._token}
        if self._local_lane:
            env["OLLAMA_HOST"] = os.environ.get(
                "OPENAGENTS_CODER_OLLAMA_HOST", "http://host.docker.internal:11434"
            )

        # The trial exists before the agent phase does anything, and the
        # thread link lands while the agent still works: the container writes
        # `coder.txt` through Harbor's host mirror, so the `[oa:thread …]`
        # announcement is greppable on the host within seconds of the session
        # opening, and posting it then is what makes the trial's transcript
        # watchable live on /gym rather than only after the trial ends. The
        # `finally` report stays as the backstop — a session that never
        # announced (offline lane) or a watcher that lost a race still links
        # its trial before the exception continues to Harbor.
        self._report_trial("running")

        async def _link_when_announced() -> None:
            while True:
                thread_id = self._thread_id_from_log()
                if thread_id:
                    await asyncio.to_thread(self._report_trial, "running", thread_id)
                    return
                await asyncio.sleep(2)

        watcher = asyncio.create_task(_link_when_announced())
        try:
            await self.exec_as_agent(
                environment,
                command=command,
                env=env,
            )
        finally:
            watcher.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await watcher
            self._report_trial("running", self._thread_id_from_log())
