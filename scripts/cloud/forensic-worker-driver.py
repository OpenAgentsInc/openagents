#!/usr/bin/env python3
"""Guest forensic worker: preflight, usage, and prepare-stop proofs."""

from __future__ import annotations

import json
import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable


DRIVER_REF = "driver.openagents.forensic-worker.v1"
BUBBLEWRAP = "/usr/bin/bwrap"
WORKSPACE = "/workspace"
TURN_ROOT = "/var/lib/openagents/managed-sandbox-turns"
SOURCE_ROOT = f"{WORKSPACE}/source"
ARTIFACT_PATH = f"{WORKSPACE}/forensic-artifact.tar.zst"
RUNTIME_ROOT = "/run/openagents-managed-sandbox"
NETWORK_ROOT = "/sys/class/net"


def refuse(reason: str) -> None:
    sys.stderr.write(json.dumps({"error": "forensic_worker_refused", "reason": reason}) + "\n")
    raise SystemExit(1)


def emit(value: Any) -> None:
    sys.stdout.write(json.dumps(value) + "\n")


def live_identifier(path: Path, process_group: bool) -> bool:
    try:
        identifier = int(path.read_text(encoding="utf-8"))
        if identifier < 1:
            return True
        os.kill(-identifier if process_group else identifier, 0)
        return True
    except ProcessLookupError:
        return False
    except (OSError, ValueError):
        return True


def directories(turn_root: str, include_io: bool) -> list[str]:
    root = Path(turn_root)
    if not root.exists():
        return []
    found: list[str] = []
    for entry in root.iterdir():
        if not entry.is_dir():
            continue
        if not include_io and entry.name.startswith("io-"):
            continue
        found.append(str(entry))
    return found


def workload_is_live_at(directory: str) -> bool:
    pgid = Path(directory) / "pgid"
    pid = Path(directory) / "pid"
    if pgid.exists():
        return live_identifier(pgid, True)
    if pid.exists():
        return live_identifier(pid, False)
    return not Path(directory).name.startswith("io-")


def path_present(path: str) -> bool:
    try:
        os.lstat(path)
        return True
    except OSError:
        return False


def path_is_under(path: str, root: str) -> bool:
    return path == root or path.startswith(f"{root}/")


def observe_guarded_processes_at(roots: list[str]) -> dict[str, Any]:
    if sys.platform != "linux" or not Path("/proc").exists():
        return {"supported": False, "inaccessible": 0, "processes": [], "processGroups": []}
    processes: list[int] = []
    process_groups: set[int] = set()
    self_pid = os.getpid()
    inaccessible = 0
    for entry in Path("/proc").iterdir():
        if not entry.is_dir() or not entry.name.isdigit():
            continue
        pid = int(entry.name)
        if pid == self_pid:
            continue
        references = [f"/proc/{pid}/cwd", f"/proc/{pid}/root", f"/proc/{pid}/exe"]
        opaque = False
        try:
            for descriptor in Path(f"/proc/{pid}/fd").iterdir():
                references.append(str(descriptor))
        except OSError as error:
            if getattr(error, "errno", None) not in (2, 3):
                opaque = True
        referenced = False
        for reference in references:
            try:
                target = os.readlink(reference)
            except OSError as error:
                if getattr(error, "errno", None) not in (2, 3):
                    opaque = True
                continue
            if any(path_is_under(target, root) for root in roots):
                referenced = True
                break
        if referenced:
            processes.append(pid)
            try:
                stat_text = Path(f"/proc/{pid}/stat").read_text(encoding="utf-8")
                close = stat_text.rfind(")")
                fields = [] if close < 0 else stat_text[close + 2 :].split(" ")
                process_group_id = int(fields[2]) if len(fields) > 2 else None
                if process_group_id is None:
                    opaque = True
                else:
                    process_groups.add(process_group_id)
            except (OSError, ValueError, IndexError) as error:
                if getattr(error, "errno", None) not in (2, 3):
                    opaque = True
        if opaque and not referenced:
            inaccessible += 1
    return {
        "supported": True,
        "inaccessible": inaccessible,
        "processes": processes,
        "processGroups": sorted(process_groups),
    }


def file_size(path: str) -> dict[str, Any]:
    if not Path(path).exists():
        return {"bytes": 0, "exact": True}
    try:
        status = os.lstat(path)
        if stat.S_ISREG(status.st_mode):
            return {"bytes": status.st_size, "exact": True}
        return {"bytes": 0, "exact": False}
    except OSError:
        return {"bytes": 0, "exact": False}


def tree_size(root: str) -> dict[str, Any]:
    if not Path(root).exists():
        return {"bytes": 0, "exact": True}
    bytes_total = 0
    exact = True
    pending = [root]
    while pending:
        path = pending.pop()
        try:
            status = os.lstat(path)
            if stat.S_ISLNK(status.st_mode):
                exact = False
            elif stat.S_ISREG(status.st_mode):
                bytes_total += status.st_size
            elif stat.S_ISDIR(status.st_mode):
                pending.extend(str(Path(path) / name) for name in os.listdir(path))
            else:
                exact = False
        except OSError:
            exact = False
    return {"bytes": bytes_total, "exact": exact}


def network_usage() -> dict[str, Any]:
    if sys.platform != "linux" or not Path(NETWORK_ROOT).exists():
        return {"bytes": 0, "exact": False}
    bytes_total = 0
    observed = False
    try:
        for interface_name in os.listdir(NETWORK_ROOT):
            if interface_name == "lo":
                continue
            observed = True
            for counter in ("rx_bytes", "tx_bytes"):
                value = int(
                    Path(NETWORK_ROOT, interface_name, "statistics", counter).read_text(
                        encoding="utf-8"
                    )
                )
                if value < 0:
                    return {"bytes": 0, "exact": False}
                bytes_total += value
    except (OSError, ValueError):
        return {"bytes": 0, "exact": False}
    return {"bytes": bytes_total, "exact": observed}


def preflight() -> None:
    if sys.platform != "linux":
        refuse("linux_required")
    try:
        if not os.access(BUBBLEWRAP, os.X_OK) or not os.access(WORKSPACE, os.R_OK | os.X_OK):
            refuse("runtime_dependency_missing")
        probe = subprocess.run(
            [
                BUBBLEWRAP,
                "--die-with-parent",
                "--unshare-net",
                "--unshare-pid",
                "--unshare-uts",
                "--unshare-ipc",
                "--ro-bind",
                "/",
                "/",
                "--bind",
                WORKSPACE,
                WORKSPACE,
                "--tmpfs",
                "/run",
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--chdir",
                WORKSPACE,
                "/bin/true",
            ],
            cwd=WORKSPACE,
            env={"PATH": "/usr/bin:/bin"},
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        if probe.returncode != 0:
            refuse("bubblewrap_probe_failed")
    except (OSError, subprocess.TimeoutExpired):
        refuse("runtime_dependency_missing")
    emit(
        {
            "schema": "openagents.forensic_worker_preflight.v1",
            "driverRef": DRIVER_REF,
            "linux": True,
            "bubblewrap": True,
            "networkNamespace": "unshared",
            "workspaceRoot": "workspace",
        }
    )


def usage() -> None:
    exact = True
    tokens = 0
    active_turns = 0
    for directory in directories(TURN_ROOT, False):
        if workload_is_live_at(directory):
            active_turns += 1
        try:
            state = json.loads(Path(directory, "state.json").read_text(encoding="utf-8"))
            usage_events = [
                event
                for event in (state.get("events") or [])
                if isinstance(event, dict) and event.get("_tag") == "RuntimeUsageRecorded"
            ]
            if len(usage_events) != 1:
                exact = False
            for event in usage_events:
                usage_value = event.get("usage") or {}
                if usage_value.get("exact") is not True:
                    exact = False
                input_tokens = usage_value.get("inputTokens")
                output_tokens = usage_value.get("outputTokens")
                if not isinstance(input_tokens, int) or not isinstance(output_tokens, int):
                    exact = False
                else:
                    tokens += input_tokens + output_tokens
        except (OSError, json.JSONDecodeError, TypeError):
            exact = False
    source = tree_size(SOURCE_ROOT)
    artifact = file_size(ARTIFACT_PATH)
    network = network_usage()
    emit(
        {
            "exact": exact and source["exact"] and artifact["exact"] and network["exact"],
            "tokens": tokens,
            "sourceBytes": source["bytes"],
            "artifactBytes": artifact["bytes"],
            "networkBytes": network["bytes"],
            "activeTurns": active_turns,
        }
    )


def prepare_stop_at(
    *,
    turn_root: str,
    source_root: str,
    artifact_path: str,
    runtime_root: str,
    observe_guarded_processes: Callable[[list[str]], dict[str, Any]] = observe_guarded_processes_at,
) -> dict[str, Any]:
    guarded_roots = [turn_root, source_root, artifact_path, runtime_root]
    if any(workload_is_live_at(directory) for directory in directories(turn_root, True)):
        refuse("forensic_process_still_active")
    before = observe_guarded_processes(guarded_roots)
    if sys.platform == "linux" and not before["supported"]:
        refuse("forensic_process_observation_unavailable")
    if before["processes"]:
        refuse("forensic_process_still_active")
    shutil.rmtree(turn_root, ignore_errors=True)
    shutil.rmtree(source_root, ignore_errors=True)
    Path(artifact_path).unlink(missing_ok=True)
    shutil.rmtree(runtime_root, ignore_errors=True)
    after = observe_guarded_processes(guarded_roots)
    if sys.platform == "linux" and not after["supported"]:
        refuse("forensic_process_observation_unavailable")
    if sys.platform == "linux" and after["inaccessible"] > 0:
        refuse("forensic_process_observation_incomplete")
    scratch_paths_remaining = len([root for root in guarded_roots if path_present(root)])
    active_process_groups = len(after["processGroups"])
    if after["processes"]:
        refuse("forensic_process_still_active")
    if scratch_paths_remaining != 0:
        refuse("forensic_scratch_still_present")
    return {
        "schema": "openagents.forensic_worker_prepare_stop.v1",
        "driverRef": DRIVER_REF,
        "processObservation": (
            "proc"
            if after["supported"] and after["inaccessible"] == 0
            else "partial"
            if after["supported"]
            else "unavailable"
        ),
        "zeroProcess": len(after["processes"]) == 0 and active_process_groups == 0,
        "zeroScratch": scratch_paths_remaining == 0,
        "activeProcessGroups": active_process_groups,
        "scratchPathsRemaining": scratch_paths_remaining,
    }


def prepare_stop() -> None:
    emit(
        prepare_stop_at(
            turn_root=TURN_ROOT,
            source_root=SOURCE_ROOT,
            artifact_path=ARTIFACT_PATH,
            runtime_root=RUNTIME_ROOT,
        )
    )


def main() -> None:
    if len(sys.argv) != 2:
        refuse("unsupported_operation")
    operation = sys.argv[1]
    if operation == "preflight":
        preflight()
    elif operation == "usage":
        usage()
    elif operation == "prepare-stop":
        prepare_stop()
    else:
        refuse("unsupported_operation")


if __name__ == "__main__":
    main()
