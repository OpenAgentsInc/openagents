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
  instead of taking the host. macOS refuses an ``RLIMIT_DATA`` below the
  process's mapped address space, which starts at hundreds of gibibytes, so
  there a watcher process samples the process's physical footprint instead
  and kills it with a message when it passes the cap, as a scope would. Where neither holds,
  ``limit_self`` raises ``CapRefused`` rather than running uncapped.

Task containers are Docker's, under Docker's own cgroup, so neither bound
touches what a task's own ``memory`` budget allows.

Each cap has a variable that overrides it, read as a byte count with an
optional ``K``, ``M``, ``G``, or ``T`` suffix (powers of 1024), or ``none``
for no cap. ``TBENCH_MEMORY_SCOPE=off`` starts children without a scope.
"""

from __future__ import annotations

import ctypes
import ctypes.util
import functools
import os
import resource
import shutil
import sys
import signal
import subprocess
import time
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

# How often the macOS watch samples this process's footprint.
WATCH_EVERY = 0.025


class CapRefused(RuntimeError):
    """This system refused the cap, and the process would run without one."""


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


def limit_self(limit: int | None, variable: str = ANALYSIS_ENV) -> None:
    """Caps this process's memory at ``limit`` bytes.

    Only lowers the cap: a tighter one already in place stays. On Linux and
    other systems that accept it, the cap is ``RLIMIT_DATA``, which children
    inherit, so call this only in a process that starts nothing heavy. On
    macOS a watcher process holds this process alone to the cap.

    ``variable`` names the override the refusal message points to.

    Raises ``CapRefused`` when the system refuses the limit.
    """
    if limit is None:
        return
    if sys.platform == "darwin":
        _watch_self(limit, variable)
        return
    soft, hard = resource.getrlimit(resource.RLIMIT_DATA)
    ceiling = limit if hard == resource.RLIM_INFINITY else min(limit, hard)
    if soft != resource.RLIM_INFINITY and soft <= ceiling:
        return
    try:
        resource.setrlimit(resource.RLIMIT_DATA, (ceiling, hard))
    except (ValueError, OSError) as error:
        raise CapRefused(_refusal(ceiling, variable, error)) from error


def _refusal(limit: int, variable: str, error: BaseException) -> str:
    return (
        f"couldn't cap this process's memory at {limit} bytes: this system "
        f"refused RLIMIT_DATA ({error}). Set {variable} to a cap this system "
        "accepts, or to none to run without one"
    )


class _RusageInfoV0(ctypes.Structure):
    _fields_ = [("ri_uuid", ctypes.c_uint8 * 16)] + [
        (name, ctypes.c_uint64)
        for name in (
            "ri_user_time",
            "ri_system_time",
            "ri_pkg_idle_wkups",
            "ri_interrupt_wkups",
            "ri_pageins",
            "ri_wired_size",
            "ri_resident_size",
            "ri_phys_footprint",
            "ri_proc_start_abstime",
            "ri_proc_exit_abstime",
        )
    ]


@functools.cache
def _libproc() -> ctypes.CDLL | None:
    if sys.platform != "darwin":
        return None
    library = ctypes.util.find_library("proc")
    return ctypes.CDLL(library) if library else None


def footprint(pid: int | None = None) -> int | None:
    """The physical footprint of ``pid``, this process by default, in bytes.

    macOS only; ``None`` elsewhere or when the process can't be read.
    """
    libproc = _libproc()
    if libproc is None:
        return None
    info = _RusageInfoV0()
    read = libproc.proc_pid_rusage(
        os.getpid() if pid is None else pid, 0, ctypes.byref(info)
    )
    return info.ri_phys_footprint if read == 0 else None


_watched_at: int | None = None


def _watch_self(limit: int, variable: str) -> None:
    """Starts a process that holds this one to ``limit`` bytes.

    The watch is a process rather than a thread because a thread needs the
    interpreter lock to sample, and a loop that allocates keeps it long
    enough to pass the cap by gigabytes first.
    """
    global _watched_at
    if footprint() is None:
        raise CapRefused(
            f"couldn't cap this process's memory at {limit} bytes: this system "
            "refuses RLIMIT_DATA and its footprint can't be read. Set "
            f"{variable} to none to run without a cap"
        )
    if _watched_at is not None and _watched_at <= limit:
        return
    subprocess.Popen(
        [sys.executable, os.path.abspath(__file__), str(os.getpid()), str(limit), variable],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        close_fds=True,
    )
    _watched_at = limit


def _watch(pid: int, limit: int, variable: str) -> None:
    """Kills ``pid`` once its footprint passes ``limit``, and says why.

    Runs until ``pid`` is gone: this process is its child, and a parent that
    exits leaves it with another.
    """
    # An interrupt at the terminal reaches the whole group; the process
    # being watched decides what it means.
    signal.signal(signal.SIGINT, signal.SIG_IGN)
    while os.getppid() == pid:
        used = footprint(pid)
        if used is None:
            return
        if used > limit:
            os.kill(pid, signal.SIGKILL)
            os.write(
                2,
                (
                    f"the process passed its memory cap of {limit} bytes with "
                    f"{used} bytes in use, and was stopped. Set {variable} to a "
                    "larger cap, or to none to run without one.\n"
                ).encode(),
            )
            return
        time.sleep(WATCH_EVERY)


if __name__ == "__main__":
    _watch(int(sys.argv[1]), int(sys.argv[2]), sys.argv[3])
