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
"""

import os
import shlex
from pathlib import Path
from typing import override

from harbor.agents.installed.base import BaseInstalledAgent, with_prompt_template
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

_TARBALL = Path(__file__).resolve().parent.parent / "openagentsinc-cli-0.3.5.tgz"
_REMOTE_TARBALL = "/installed-agent/openagents-cli.tgz"
_DEFAULT_API_URL = "http://host.docker.internal:4000"
_EXPORT_DIR = "$HOME/.openagents/exports"


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
        if not token:
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
        return self.model_name.split("/", 1)[-1]

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

        await self.exec_as_agent(
            environment,
            command=command,
            env={"OPENAGENTS_TOKEN": self._token},
        )
