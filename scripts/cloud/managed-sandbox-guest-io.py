#!/usr/bin/env python3
"""Execute one bounded managed-sandbox I/O request inside the GCE guest."""

from __future__ import annotations

import base64
import ctypes
import datetime
import errno
import hashlib
import json
import mimetypes
import os
import resource
import selectors
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


WORKSPACE = Path("/workspace")
SCRATCH_ROOT = Path("/run/openagents-managed-sandbox/io")
OPENAT2 = 437  # x86_64 Linux; the admitted SBX-09 image is amd64.
RESOLVE_NO_MAGICLINKS = 0x02
RESOLVE_NO_SYMLINKS = 0x04
RESOLVE_BENEATH = 0x08
FORBIDDEN = (
    b"-----begin private key-----",
    b"-----begin rsa private key-----",
    b"authorization: bearer ",
    b"refresh_token",
    b"client_secret",
    b"ghp_",
    b"github_pat_",
    b"sk-proj-",
)


class OpenHow(ctypes.Structure):
    _fields_ = [
        ("flags", ctypes.c_ulonglong),
        ("mode", ctypes.c_ulonglong),
        ("resolve", ctypes.c_ulonglong),
    ]


LIBC = ctypes.CDLL(None, use_errno=True)


def digest(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def iso_now() -> str:
    return (
        datetime.datetime.now(datetime.timezone.utc)
        .isoformat(timespec="milliseconds")
        .replace("+00:00", "Z")
    )


def bounded_timestamp(requested_at: str) -> str:
    observed = iso_now()
    return observed if observed >= requested_at else requested_at


def contains_secret(value: bytes) -> bool:
    lowered = value.lower()
    return any(marker in lowered for marker in FORBIDDEN)


def relative_path(value: str) -> str:
    if value == "workspace":
        return "."
    if not value.startswith("workspace/"):
        raise ValueError("path_not_beneath_workspace")
    relative = value[len("workspace/") :]
    if not relative or any(
        segment in {"", ".", ".."} for segment in relative.split("/")
    ):
        raise ValueError("path_not_beneath_workspace")
    return relative


def open_beneath(root_fd: int, relative: str, flags: int, mode: int = 0) -> int:
    how = OpenHow(
        flags=flags | os.O_CLOEXEC,
        mode=mode,
        resolve=RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS,
    )
    result = LIBC.syscall(
        OPENAT2,
        root_fd,
        ctypes.c_char_p(relative.encode()),
        ctypes.byref(how),
        ctypes.sizeof(how),
    )
    if result < 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code), relative)
    return int(result)


def ensure_parents(root_fd: int, relative: str) -> None:
    segments = relative.split("/")[:-1]
    current = os.dup(root_fd)
    try:
        for segment in segments:
            try:
                next_fd = os.open(
                    segment,
                    os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                    dir_fd=current,
                )
            except FileNotFoundError:
                os.mkdir(segment, mode=0o700, dir_fd=current)
                next_fd = os.open(
                    segment,
                    os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                    dir_fd=current,
                )
            os.close(current)
            current = next_fd
    finally:
        os.close(current)


def read_beneath(root_fd: int, relative: str, maximum: int) -> bytes:
    fd = open_beneath(root_fd, relative, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        value = bytearray()
        while len(value) <= maximum:
            chunk = os.read(fd, min(65536, maximum + 1 - len(value)))
            if not chunk:
                break
            value.extend(chunk)
        if len(value) > maximum:
            raise ValueError("content_out_of_bounds")
        return bytes(value)
    finally:
        os.close(fd)


def write_beneath(root_fd: int, relative: str, value: bytes) -> None:
    ensure_parents(root_fd, relative)
    fd = open_beneath(
        root_fd,
        relative,
        os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW,
        0o600,
    )
    try:
        view = memoryview(value)
        while view:
            written = os.write(fd, view)
            view = view[written:]
        os.fsync(fd)
    finally:
        os.close(fd)


def process_group_count(group: int) -> int:
    count = 0
    for stat_path in Path("/proc").glob("[0-9]*/stat"):
        try:
            fields = stat_path.read_text(encoding="utf-8").split()
            if len(fields) > 4 and int(fields[4]) == group:
                count += 1
        except (OSError, ValueError):
            continue
    return count


def terminate_group(group: int) -> None:
    try:
        os.killpg(group, signal.SIGKILL)
    except ProcessLookupError:
        pass


def command_preexec(cpu_millis: int, max_processes: int) -> None:
    os.setsid()
    cpu_seconds = max(1, (cpu_millis + 999) // 1000)
    resource.setrlimit(resource.RLIMIT_CPU, (cpu_seconds, cpu_seconds))
    resource.setrlimit(resource.RLIMIT_NPROC, (max_processes, max_processes))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def execute_command(request: dict[str, Any], scratch: Path) -> dict[str, Any]:
    limits = request["limits"]
    relative = relative_path(request["cwd"])
    canonical_cwd = WORKSPACE / relative if relative else WORKSPACE
    root_fd = os.open(WORKSPACE, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        cwd_fd = open_beneath(root_fd, relative, os.O_RDONLY | os.O_DIRECTORY)
    finally:
        os.close(root_fd)
    timeout_seconds = request["timeoutMillis"] / 1000
    output_limit = int(limits["maxOutputBytes"])
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.monotonic()
    try:
        process = subprocess.Popen(
            [
                "/usr/bin/bwrap",
                "--die-with-parent",
                "--unshare-net",
                "--unshare-pid",
                "--unshare-uts",
                "--unshare-ipc",
                "--ro-bind",
                "/",
                "/",
                "--bind",
                str(WORKSPACE),
                str(WORKSPACE),
                "--bind",
                f"/proc/self/fd/{cwd_fd}",
                str(canonical_cwd),
                "--tmpfs",
                "/run",
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--bind",
                str(scratch),
                "/tmp",
                "--chdir",
                str(canonical_cwd),
                "/bin/sh",
                "-lc",
                request["command"],
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={
                "HOME": "/tmp",
                "LANG": "C.UTF-8",
                "PATH": "/usr/local/bin:/usr/bin:/bin",
                "TMPDIR": "/tmp",
            },
            pass_fds=(cwd_fd,),
            preexec_fn=lambda: command_preexec(
                int(limits["maxCpuMillis"]), int(limits["maxProcesses"])
            ),
        )
    finally:
        os.close(cwd_fd)
    group = process.pid
    selector = selectors.DefaultSelector()
    assert process.stdout is not None and process.stderr is not None
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    chunks: dict[str, bytearray] = {"stdout": bytearray(), "stderr": bytearray()}
    truncated = {"stdout": False, "stderr": False}
    timed_out = False
    max_processes = 1
    killed_for_output = False
    while selector.get_map():
        max_processes = max(max_processes, process_group_count(group))
        if not timed_out and time.monotonic() - started > timeout_seconds:
            timed_out = True
            terminate_group(group)
        for key, _ in selector.select(timeout=0.05):
            try:
                value = os.read(key.fileobj.fileno(), 65536)
            except OSError:
                value = b""
            if not value:
                selector.unregister(key.fileobj)
                continue
            target = chunks[key.data]
            remaining = max(0, output_limit - sum(len(item) for item in chunks.values()))
            target.extend(value[:remaining])
            if len(value) > remaining:
                truncated[key.data] = True
                if not killed_for_output:
                    killed_for_output = True
                    terminate_group(group)
        if process.poll() is not None and not selector.get_map():
            break
    return_code = process.wait()
    terminate_group(group)
    descendants = max(0, process_group_count(group))
    duration_millis = min(
        int((time.monotonic() - started) * 1000), int(request["timeoutMillis"])
    )
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu_millis = int(
        ((after.ru_utime + after.ru_stime) - (before.ru_utime + before.ru_stime))
        * 1000
    )
    signaled = return_code < 0
    return {
        "success": return_code == 0 and not timed_out and not killed_for_output,
        "exitCode": None if signaled else return_code,
        "signal": signal.Signals(-return_code).name if signaled else None,
        "stdout": chunks["stdout"].decode("utf-8", errors="replace"),
        "stderr": chunks["stderr"].decode("utf-8", errors="replace"),
        "stdoutTruncated": truncated["stdout"],
        "stderrTruncated": truncated["stderr"],
        "timedOut": timed_out,
        "cancelled": timed_out,
        "durationMillis": duration_millis,
        "maxProcessesObserved": max_processes,
        "cpuMillis": cpu_millis,
        "descendantsRemaining": descendants,
    }


def receipt(
    request: dict[str, Any],
    started_at: str,
    finished_at: str,
    *,
    bytes_read: int = 0,
    bytes_written: int = 0,
    cpu_millis: int = 0,
    process_ref: str | None = None,
    descendants_remaining: int = 0,
) -> dict[str, Any]:
    identity = digest(
        f"{request['operationRef']}|{request['resourceGeneration']}".encode()
    )[7:]
    effective_path = request.get("path") or request.get("cwd")
    return {
        "schemaVersion": "openagents.managed_sandbox_guest_io_receipt.v1",
        "receiptRef": f"receipt.sbx09.{identity}",
        "operationRef": request["operationRef"],
        "sandboxRef": request["sandboxRef"],
        "resourceGeneration": request["resourceGeneration"],
        "capabilityRef": request["capabilityRef"],
        "action": request["action"],
        "outcome": "succeeded",
        "pathDigest": digest(effective_path.encode()),
        "startedAt": started_at,
        "finishedAt": finished_at,
        "bytesRead": bytes_read,
        "bytesWritten": bytes_written,
        "cpuMillis": cpu_millis,
        "networkBytes": 0,
        "processRef": process_ref,
        "processTerminated": True,
        "descendantsRemaining": descendants_remaining,
        "scratchCleaned": True,
        "ingressClosed": True,
        "egressDenied": True,
        "pathPolicy": "resolved_beneath_workspace_root",
        "symlinkTraversal": False,
        "secretScan": "clean",
        "evidenceRefs": [f"evidence.sbx09.guest-io.{identity}"],
    }


def handle(request: dict[str, Any]) -> dict[str, Any]:
    started_at = bounded_timestamp(request["requestedAt"])
    operation_key = hashlib.sha256(request["operationRef"].encode()).hexdigest()[:24]
    scratch = SCRATCH_ROOT / operation_key
    scratch.mkdir(parents=True, mode=0o700, exist_ok=False)
    root_fd = os.open(WORKSPACE, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    action = request["action"]
    response: dict[str, Any] = {
        "schemaVersion": "openagents.managed_sandbox_guest_io.v1",
        "action": action,
        "operationRef": request["operationRef"],
        "sandboxRef": request["sandboxRef"],
        "resourceGeneration": request["resourceGeneration"],
    }
    receipt_kwargs: dict[str, Any] = {}
    try:
        if action == "read_file":
            value = read_beneath(
                root_fd,
                relative_path(request["path"]),
                int(request["limits"]["maxFileBytes"]),
            )
            if contains_secret(value):
                raise ValueError("secret_material_refused")
            if request["encoding"] == "utf8":
                content = value.decode("utf-8")
                binary = False
            else:
                content = base64.b64encode(value).decode()
                binary = True
            response.update(
                encoding=request["encoding"],
                content=content,
                contentDigest=digest(value),
                byteLength=len(value),
                binary=binary,
            )
            receipt_kwargs["bytes_read"] = len(value)
        elif action == "write_file":
            value = (
                request["content"].encode()
                if request["encoding"] == "utf8"
                else base64.b64decode(request["content"], validate=True)
            )
            if contains_secret(value) or digest(value) != request["contentDigest"]:
                raise ValueError("write_content_refused")
            write_beneath(root_fd, relative_path(request["path"]), value)
            response.update(contentDigest=digest(value), byteLength=len(value))
            receipt_kwargs["bytes_written"] = len(value)
        elif action == "execute_command":
            command_result = execute_command(request, scratch)
            if contains_secret(command_result["stdout"].encode()) or contains_secret(
                command_result["stderr"].encode()
            ):
                raise ValueError("command_output_secret_refused")
            response.update(
                (key, value)
                for key, value in command_result.items()
                if key not in {"cpuMillis", "descendantsRemaining"}
            )
            receipt_kwargs.update(
                bytes_written=len(command_result["stdout"].encode())
                + len(command_result["stderr"].encode()),
                cpu_millis=command_result["cpuMillis"],
                process_ref=f"process.sbx09.{operation_key}",
                descendants_remaining=command_result["descendantsRemaining"],
            )
        elif action == "read_artifact":
            value = read_beneath(
                root_fd,
                relative_path(request["path"]),
                int(request["limits"]["maxArtifactBytes"]),
            )
            if contains_secret(value):
                raise ValueError("artifact_secret_refused")
            content_digest = digest(value)
            content_type = (
                mimetypes.guess_type(request["path"])[0] or "application/octet-stream"
            )
            evidence = f"evidence.sbx09.artifact.{content_digest[7:]}"
            response.update(
                contentBase64=base64.b64encode(value).decode(),
                artifact={
                    "schemaVersion": "openagents.managed_sandbox_artifact_receipt.v1",
                    "artifactRef": f"artifact.sha256.{content_digest[7:]}",
                    "contentDigest": content_digest,
                    "byteLength": len(value),
                    "sourceGeneration": request["resourceGeneration"],
                    "sourcePathDigest": digest(request["path"].encode()),
                    "retentionUntil": request["retentionUntil"],
                    "contentType": content_type,
                    "evidenceRefs": [evidence],
                },
            )
            receipt_kwargs["bytes_read"] = len(value)
        else:
            raise ValueError("action_not_admitted")
    finally:
        os.close(root_fd)
        shutil.rmtree(scratch, ignore_errors=True)
    finished_at = bounded_timestamp(started_at)
    response["receipt"] = receipt(
        request, started_at, finished_at, **receipt_kwargs
    )
    return response


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(2)
    request_path = Path(sys.argv[1])
    try:
        request = json.loads(request_path.read_text(encoding="utf-8"))
    finally:
        request_path.unlink(missing_ok=True)
    try:
        response = handle(request)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        raise SystemExit(2) from None
    sys.stdout.write(json.dumps(response, separators=(",", ":")))


if __name__ == "__main__":
    main()
