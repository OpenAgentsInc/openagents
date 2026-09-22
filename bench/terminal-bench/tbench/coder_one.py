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
"""

from __future__ import annotations

from typing import ClassVar

from pathlib import PurePosixPath

from tbench.coder_v05 import INSTALL_ROOT, CoderV05


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
