"""Bounded subprocess effects for optional native-check maintenance tooling.

The Coax execution policy owns admission and verdicts. This adapter reports
only raw process, pipe, limit and timing observations.
"""
import importlib.util
import math
import os
from pathlib import Path
import selectors
import signal
import subprocess
import time
from collections.abc import Mapping


_QUIET_PATH = Path(__file__).resolve().parents[1] / "tests" / "quiet_crash.py"
_QUIET_SPEC = importlib.util.spec_from_file_location("coax_bounded_quiet_crash", _QUIET_PATH)
_QUIET = importlib.util.module_from_spec(_QUIET_SPEC)
_QUIET_SPEC.loader.exec_module(_QUIET)
_READ_SIZE = 65536
_POLL_SECONDS = 0.025
_DRAIN_SECONDS = 0.1
_REAP_SECONDS = 0.25


def _caps(value):
    if isinstance(value, Mapping):
        caps = {key: value[key] for key in ("stdout", "stderr", "total")}
    else:
        caps = {key: value for key in ("stdout", "stderr", "total")}
    if any(type(cap) is not int or cap < 0 for cap in caps.values()):
        raise ValueError("output caps must be nonnegative integer byte extents")
    return caps


def run(argv, cwd, env, timeout_s, max_output_bytes, deadline, quiet_faults=False):
    """Observe one POSIX process group without unbounded pipe collection.

    ``deadline`` is an absolute time.monotonic() deadline shared by the caller's
    operations. The caller supplies Coax-admitted budgets. An integer output
    cap bounds each pipe and their retained combined bytes; a mapping supplies
    separate ``stdout``, ``stderr`` and ``total`` extents. Returned byte strings
    contain bounded prefixes, never decoded or interpreted here.
    """
    caps = _caps(max_output_bytes)
    if not math.isfinite(timeout_s) or timeout_s < 0 or not math.isfinite(deadline):
        raise ValueError("process timing extents must be finite and timeout nonnegative")
    original = [os.fspath(part) for part in argv]
    launched = _QUIET.command(original) if quiet_faults else original
    started_at = time.monotonic()
    expires_at = min(started_at + timeout_s, deadline)
    result = {
        "argv": original, "launched_argv": launched,
        "cwd": os.fspath(cwd) if cwd is not None else None,
        "started": False, "pid": None, "returncode": None,
        "launch_errno": None, "launch_error": None,
        "stdout": b"", "stderr": b"", "elapsed_s": 0.0,
        "timeout": False, "deadline_exhausted": False,
        "stdout_limit": False, "stderr_limit": False, "output_limit": False,
        "observed_stdout_bytes": 0, "observed_stderr_bytes": 0,
        "group_killed": False, "kill_errno": None, "read_errno": None,
        "stdout_eof": False, "stderr_eof": False,
        "limits": caps, "timeout_s": timeout_s, "deadline": deadline,
    }

    def observe_clock():
        now = time.monotonic()
        if now >= started_at + timeout_s:
            result["timeout"] = True
        if now >= deadline:
            result["deadline_exhausted"] = True

    def finish():
        result["elapsed_s"] = time.monotonic() - started_at
        return result

    if started_at >= expires_at:
        observe_clock()
        return finish()
    try:
        process = subprocess.Popen(
            launched, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True, close_fds=True,
        )
    except (OSError, ValueError) as error:
        result["launch_errno"] = getattr(error, "errno", None)
        result["launch_error"] = str(error)
        return finish()
    result["started"] = True
    result["pid"] = process.pid
    retained = {"stdout": bytearray(), "stderr": bytearray()}
    selector = selectors.DefaultSelector()
    pipes = {"stdout": process.stdout, "stderr": process.stderr}

    def kill_group():
        # The group remains addressable by the original pid even if its leader
        # exited while a descendant still holds the pipe descriptors open.
        try:
            os.killpg(process.pid, signal.SIGKILL)
            result["group_killed"] = True
        except OSError as error:
            result["kill_errno"] = error.errno

    def close_pipe(pipe):
        try:
            selector.unregister(pipe)
        except (KeyError, ValueError):
            pass
        pipe.close()

    def retain(name, chunk):
        stream_room = caps[name] - len(retained[name])
        total_room = caps["total"] - sum(len(value) for value in retained.values())
        result["observed_" + name + "_bytes"] += len(chunk)
        retained[name].extend(chunk[:min(len(chunk), stream_room, total_room)])
        if len(chunk) > stream_room:
            result[name + "_limit"] = True
        if len(chunk) > total_room:
            result["output_limit"] = True

    stopped = False
    try:
        for name, pipe in pipes.items():
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, name)
        while selector.get_map() or process.poll() is None:
            remaining = expires_at - time.monotonic()
            if remaining <= 0:
                observe_clock()
                kill_group()
                stopped = True
                break
            ready = selector.select(min(_POLL_SECONDS, remaining))
            for key, _ in ready:
                name, pipe = key.data, key.fileobj
                stream_room = caps[name] - len(retained[name])
                total_room = caps["total"] - sum(len(value) for value in retained.values())
                # Read at most one byte beyond either cap, enough to observe
                # exhaustion without collecting an unbounded pending write.
                wanted = min(_READ_SIZE, stream_room + 1, total_room + 1)
                try:
                    chunk = os.read(pipe.fileno(), wanted)
                except BlockingIOError:
                    continue
                except OSError as error:
                    result["read_errno"] = error.errno
                    kill_group()
                    stopped = True
                    break
                if not chunk:
                    result[name + "_eof"] = True
                    close_pipe(pipe)
                    continue
                retain(name, chunk)
                if result[name + "_limit"] or result["output_limit"]:
                    kill_group()
                    stopped = True
                    break
            if stopped:
                break
        # EOF delivery can itself be delayed past a budget. Observe the clock
        # before accepting normal completion, even when both pipes and the
        # group leader finished during the last readiness notification.
        if not stopped and time.monotonic() >= expires_at:
            observe_clock()
            kill_group()
            stopped = True
    except OSError as error:
        result["read_errno"] = error.errno
        kill_group()
        stopped = True
    finally:
        if stopped:
            drain_until = time.monotonic() + _DRAIN_SECONDS
            while selector.get_map() and time.monotonic() < drain_until:
                ready = selector.select(min(_POLL_SECONDS, max(0, drain_until - time.monotonic())))
                for key, _ in ready:
                    name, pipe = key.data, key.fileobj
                    try:
                        chunk = os.read(pipe.fileno(), _READ_SIZE)
                    except BlockingIOError:
                        continue
                    except OSError as error:
                        result["read_errno"] = error.errno
                        close_pipe(pipe)
                        continue
                    if chunk:
                        retain(name, chunk)
                    else:
                        result[name + "_eof"] = True
                        close_pipe(pipe)
        for pipe in pipes.values():
            if not pipe.closed:
                close_pipe(pipe)
        selector.close()
        # SIGKILL cleanup gets a short fixed reap window, never an unbounded
        # communicate()/wait() on descendants that retained pipe handles.
        if stopped or process.poll() is None:
            try:
                result["returncode"] = process.wait(timeout=_REAP_SECONDS)
            except subprocess.TimeoutExpired:
                kill_group()
                result["returncode"] = process.poll()
        else:
            result["returncode"] = process.returncode
        result["stdout"] = bytes(retained["stdout"])
        result["stderr"] = bytes(retained["stderr"])
    return finish()
