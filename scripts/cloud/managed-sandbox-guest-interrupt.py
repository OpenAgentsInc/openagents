#!/usr/bin/env python3
"""Interrupt a managed-sandbox guest turn process group and emit a proof."""

from __future__ import annotations

import json
import os
import re
import signal
import sys
import time
from pathlib import Path
from typing import Any


TURN_ROOT = "/var/lib/openagents/managed-sandbox-turns"
WAIT_SECONDS = 5.0
POLL_SECONDS = 0.05


def fail(reason: str) -> None:
    sys.stderr.write(json.dumps({"error": "managed_sandbox_interrupt_failed", "reason": reason}) + "\n")
    raise SystemExit(2)


def live_processes() -> list[dict[str, int]]:
    if sys.platform != "linux" or not Path("/proc").exists():
        fail("linux_proc_required")
    observed: list[dict[str, int]] = []
    for entry in Path("/proc").iterdir():
        if not entry.is_dir() or not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text(encoding="utf-8")
        except OSError as error:
            if getattr(error, "errno", None) != 2:
                fail("process_observation_failed")
            continue
        close = stat.rfind(")")
        if close < 0:
            fail("process_stat_invalid")
        fields = stat[close + 2 :].split(" ")
        if fields[0] == "Z":
            continue
        try:
            process_group_id = int(fields[2])
            session_id = int(fields[3])
        except (IndexError, ValueError):
            fail("process_stat_invalid")
        observed.append(
            {
                "pid": int(entry.name),
                "processGroupId": process_group_id,
                "sessionId": session_id,
            }
        )
    return observed


def sessions_in(processes: list[dict[str, int]], process_group_id: int) -> list[int]:
    seen: list[int] = []
    for observed in processes:
        if observed["processGroupId"] == process_group_id and observed["sessionId"] not in seen:
            seen.append(observed["sessionId"])
    return seen


def escaped_descendants_in(
    processes: list[dict[str, int]],
    process_group_id: int,
    sessions: list[int],
    self_pid: int,
) -> list[dict[str, int]]:
    return [
        observed
        for observed in processes
        if observed["processGroupId"] != process_group_id
        and observed["sessionId"] in sessions
        and observed["pid"] != self_pid
        and observed["sessionId"] != 0
    ]


def process_group_members(process_group_id: int) -> list[int]:
    return [
        observed["pid"]
        for observed in live_processes()
        if observed["processGroupId"] == process_group_id
    ]


def wait_for_exit(process_group_id: int) -> bool:
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        if not process_group_members(process_group_id):
            return True
        time.sleep(POLL_SECONDS)
    return not process_group_members(process_group_id)


def signal_group(process_group_id: int, signum: int) -> None:
    try:
        os.killpg(process_group_id, signum)
    except ProcessLookupError:
        pass
    except OSError:
        fail("process_group_signal_failed")


def interrupt_process_group(process_group_id: int) -> dict[str, Any]:
    if not isinstance(process_group_id, int) or process_group_id < 2:
        fail("process_group_identity_invalid")
    escalated_to_sigkill = False
    sessions = sessions_in(live_processes(), process_group_id)
    if process_group_members(process_group_id):
        signal_group(process_group_id, signal.SIGTERM)
        if not wait_for_exit(process_group_id):
            escalated_to_sigkill = True
            signal_group(process_group_id, signal.SIGKILL)
            if not wait_for_exit(process_group_id):
                fail("process_group_still_active")
    for escapee in escaped_descendants_in(
        live_processes(), process_group_id, sessions, os.getpid()
    ):
        try:
            os.kill(escapee["pid"], signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError:
            fail("escaped_descendant_signal_failed")
    deadline = time.monotonic() + WAIT_SECONDS
    remaining = escaped_descendants_in(
        live_processes(), process_group_id, sessions, os.getpid()
    )
    while remaining and time.monotonic() < deadline:
        time.sleep(POLL_SECONDS)
        remaining = escaped_descendants_in(
            live_processes(), process_group_id, sessions, os.getpid()
        )
    group_remaining = len(process_group_members(process_group_id))
    descendants_remaining = len(remaining)
    if group_remaining != 0:
        fail("process_group_still_active")
    if descendants_remaining != 0:
        fail("escaped_descendants_still_active")
    return {
        "schemaVersion": "openagents.managed_sandbox_interrupt_proof.v1",
        "processGroupId": process_group_id,
        "zeroProcessGroup": group_remaining == 0,
        "descendantsRemaining": descendants_remaining,
        "escalatedToSigkill": escalated_to_sigkill,
    }


def main() -> None:
    if len(sys.argv) != 2:
        fail("turn_directory_invalid")
    turn_directory = sys.argv[1]
    if not re.fullmatch(rf"{TURN_ROOT}/[a-f0-9]{{24}}", turn_directory):
        fail("turn_directory_invalid")
    try:
        process_group_id = int(Path(turn_directory, "pgid").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        fail("process_group_identity_missing")
    sys.stdout.write(json.dumps(interrupt_process_group(process_group_id)) + "\n")


if __name__ == "__main__":
    main()
