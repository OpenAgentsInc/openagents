"""Where the benchmark keeps things.

Three locations, on purpose:

* ``PACKAGE_DIR`` — this checkout. Holds the checked profiles and nothing
  else. Bulk logs never land here.
* ``state_dir()`` — the persistent cache: the upstream task checkout and any
  downloaded inputs. Outside the repository and outside temporary audit
  directories, so a rerun does not re-derive it.
* ``jobs_dir()`` — where Harbor job directories and this package's attempt
  records go. Local disk, never uploaded.
"""

from __future__ import annotations

import os
from pathlib import Path

PACKAGE_DIR = Path(__file__).resolve().parent.parent
PROFILES_DIR = PACKAGE_DIR / "profiles"

# Environment overrides, for tests and for operators who keep the cache on
# another volume.
STATE_ENV = "TBENCH_STATE_DIR"
JOBS_ENV = "TBENCH_JOBS_DIR"


def state_dir() -> Path:
    """The persistent benchmark cache root."""
    override = os.environ.get(STATE_ENV)
    if override:
        return Path(override).expanduser()
    return Path.home() / ".openagents" / "terminal-bench"


def jobs_dir() -> Path:
    """Where job directories are written by default."""
    override = os.environ.get(JOBS_ENV)
    if override:
        return Path(override).expanduser()
    return state_dir() / "jobs"


def upstream_checkout() -> Path:
    """Where the pinned upstream task repository lives in the cache."""
    return state_dir() / "upstream" / "terminal-bench"


def artifacts_dir() -> Path:
    """Where v0.5 artifact bundles are expected by convention."""
    return state_dir() / "artifacts"
