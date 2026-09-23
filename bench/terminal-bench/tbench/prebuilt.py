"""Harbor's Claude Code and Codex agents, installed from prebuilt layers.

``PrebuiltClaudeCode`` and ``PrebuiltCodex`` run Harbor's own agents
unchanged except for the install: they copy the pinned toolchain layers
(``tbench.toolchain``) into the environment first, so Harbor's install
finds the requested version already present and returns without a
package manager or a download. When no layer fits, Harbor's own network
install runs, as it would for the stock agent. Either way the adapter
writes ``toolchain-setup.json`` beside the agent's logs.

Select one with ``harbor_import_path`` in an agent profile, for example
``tbench.prebuilt:PrebuiltClaudeCode``, with the same ``version`` kwarg as
the stock arm.
"""

from __future__ import annotations

import time
from typing import Any

from harbor.agents.installed.claude_code import ClaudeCode
from harbor.agents.installed.codex import Codex
from harbor.environments.base import BaseEnvironment

from tbench.toolchain import ToolchainError, place_toolchain, setup_record, write_setup


async def _prebuilt_install(
    agent: Any, environment: BaseEnvironment, executor: str, stock_install
) -> None:
    version = getattr(agent, "_version", None)
    started = time.monotonic()
    note = None
    if version:
        try:
            record = await place_toolchain(agent, environment, executor, version)
        except ToolchainError as exc:
            note = f"prebuilt layers unavailable, installed from the network: {exc}"
        else:
            # Harbor's install sees the pinned version and returns at once.
            await stock_install(environment)
            record["phases_ms"]["harbor_check"] = int(
                (time.monotonic() - started) * 1000
            ) - record["install_ms"]
            record["install_ms"] = sum(record["phases_ms"].values())
            write_setup(agent.logs_dir, record)
            return
    else:
        note = "no pinned version, so no layer applies; installed from the network"
    await stock_install(environment)
    write_setup(
        agent.logs_dir,
        setup_record(
            mode="network",
            platform=None,
            layers=[],
            phases_ms={"network_install": int((time.monotonic() - started) * 1000)},
            versions={executor: version or "unpinned"},
            note=note,
        ),
    )


class PrebuiltClaudeCode(ClaudeCode):
    """Harbor's Claude Code agent with a prebuilt install."""

    async def install(self, environment: BaseEnvironment) -> None:
        await _prebuilt_install(self, environment, "claude-code", super().install)


class PrebuiltCodex(Codex):
    """Harbor's Codex agent with a prebuilt install."""

    async def install(self, environment: BaseEnvironment) -> None:
        await _prebuilt_install(self, environment, "codex", super().install)
