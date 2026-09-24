"""Memory caps for the harness's own processes (OpenAgentsInc/openagents#9596).

Twice, one Python analysis process grew to 118 to 124 GB on a 125 GB host
with no swap, and the out-of-memory killer took the desktop session with
it. Two bounds keep a runaway harness process to itself:

- A process that starts others, the detached scheduler and each trial it
  launches, runs in a transient systemd scope with ``MemoryMax``. The
  cgroup counts the whole tree, and the kernel kills inside it alone. The
  scope stays under the slice the caller runs in, such as ``agents.slice``,
  rather than moving out from under its ceiling.
- An analysis command, which starts nothing, caps itself with
  ``RLIMIT_DATA``, so an input too large to hold raises ``MemoryError``
  instead of taking the host.

Task containers are Docker's, under Docker's own cgroup, so neither bound
touches what a task's own ``memory`` budget allows.

Each cap has a variable that overrides it, read as a byte count with an
optional ``K``, ``M``, ``G``, or ``T`` suffix (powers of 1024), or ``none``
for no cap. ``TBENCH_MEMORY_SCOPE=off`` starts children without a scope.
"""

from __future__ import annotations

import os
import resource
import shutil
import sys
from collections.abc import Mapping

TRIAL_ENV = "TBENCH_TRIAL_MEMORY_MAX"
SCHEDULER_ENV = "TBENCH_SCHEDULER_MEMORY_MAX"
ANALYSIS_ENV = "TBENCH_ANALYSIS_MEMORY_MAX"
SCOPE_ENV = "TBENCH_MEMORY_SCOPE"

GIB = 1024**3
# One trial's host side: `tbench run`, Harbor, and whatever Harbor starts on
# the host. The same figure as `crates/supervise`'s per-job default.
TRIAL_MAX = 16 * GIB
# The scheduler holds the plan and the status file; its trials are scoped
# on their own, beside it rather than inside it.
SCHEDULER_MAX = 8 * GIB
# A report over retained trials. The runaway was one of these.
ANALYSIS_MAX = 16 * GIB

_SHIFTS = {"k": 10, "m": 20, "g": 30, "t": 40}


def parse_bytes(text: str) -> int | None:
    """Reads a byte count, or ``none``, ``off``, or ``0`` for no cap."""
    value = text.strip().lower()
    if value in ("none", "off", "0"):
        return None
    for suffix in ("ib", "b"):
        if value.endswith(suffix) and value[:-len(suffix)][-1:] in _SHIFTS:
            value = value[: -len(suffix)]
            break
    shift = _SHIFTS.get(value[-1:], 0)
    digits = value[:-1] if shift else value
    if not digits.strip().isdigit():
        raise ValueError(f"{text!r} is not a byte count")
    count = int(digits) << shift
    return count or None


def cap(variable: str, default: int, environ: Mapping[str, str] | None = None) -> int | None:
    """The cap ``variable`` sets, or ``default`` when it is unset or unreadable."""
    text = (os.environ if environ is None else environ).get(variable)
    if text is None:
        return default
    try:
        return parse_bytes(text)
    except ValueError:
        return default


def own_slice(cgroup_file: str = "/proc/self/cgroup") -> str | None:
    """The innermost slice this process runs in under a user manager."""
    try:
        with open(cgroup_file) as handle:
            path = next(
                (line[3:].strip() for line in handle if line.startswith("0::")), ""
            )
    except OSError:
        return None
    _, marker, managed = path.partition(".service/")
    if not marker:
        return None
    parents = managed.split("/")[:-1]
    return next((part for part in reversed(parents) if part.endswith(".slice")), None)


def scopes_available(environ: Mapping[str, str] | None = None) -> bool:
    """Whether a child can be started in a user scope here."""
    environ = os.environ if environ is None else environ
    return (
        sys.platform.startswith("linux")
        and environ.get(SCOPE_ENV, "").lower() != "off"
        and bool(environ.get("XDG_RUNTIME_DIR"))
        and shutil.which("systemd-run") is not None
    )


def scoped(
    argv: list[str], limit: int | None, environ: Mapping[str, str] | None = None
) -> list[str]:
    """``argv`` wrapped to run in a scope capped at ``limit`` bytes.

    ``systemd-run --scope`` executes the command in its own process, so the
    process identifier, the session, and the environment the caller passes
    are the command's. Where no scope is available, or there is no limit,
    ``argv`` comes back as it was.
    """
    if limit is None or not scopes_available(environ):
        return list(argv)
    wrapper = [
        "systemd-run",
        "--user",
        "--scope",
        "--quiet",
        "--collect",
        f"--property=MemoryMax={limit}",
        # Swap only slows a runaway on its way to the cap, and takes the
        # room the desktop needs.
        "--property=MemorySwapMax=0",
        # The whole tree ends together, not one process in it.
        "--property=OOMPolicy=kill",
    ]
    slice_name = own_slice()
    if slice_name:
        wrapper.append(f"--slice={slice_name}")
    return [*wrapper, "--", *argv]


def limit_self(limit: int | None) -> None:
    """Caps this process's data segment at ``limit`` bytes.

    Only lowers the limit: a tighter one already in place stays. Children
    inherit it, so call this only in a process that starts nothing heavy.
    """
    if limit is None:
        return
    soft, hard = resource.getrlimit(resource.RLIMIT_DATA)
    ceiling = limit if hard == resource.RLIM_INFINITY else min(limit, hard)
    if soft != resource.RLIM_INFINITY and soft <= ceiling:
        return
    try:
        resource.setrlimit(resource.RLIMIT_DATA, (ceiling, hard))
    except (ValueError, OSError):
        pass
