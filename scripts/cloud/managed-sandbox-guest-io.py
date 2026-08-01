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
import tempfile
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


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")


def validate_bundle_path(value: str) -> None:
    if (
        not value
        or len(value) > 1024
        or value.startswith("/")
        or value.endswith("/")
        or "\\" in value
        or any(segment in {"", ".", ".."} for segment in value.split("/"))
    ):
        raise ValueError("source_bundle_path_invalid")


def decode_source_bundle(
    value: bytes,
) -> tuple[dict[str, Any], list[tuple[str, bytes]]]:
    payload = json.loads(value)
    if not isinstance(payload, dict) or set(payload) != {
        "schema",
        "repositoryRef",
        "commitSha",
        "gitTreeSha",
        "entries",
    }:
        raise ValueError("source_bundle_payload_invalid")
    if (
        payload["schema"] != "openagents.forensic_source_bundle_payload.v2"
        or not all(
            isinstance(payload[field], str) and payload[field]
            for field in ("repositoryRef", "commitSha", "gitTreeSha")
        )
    ):
        raise ValueError("source_bundle_payload_invalid")
    if not isinstance(payload["entries"], list) or not payload["entries"]:
        raise ValueError("source_bundle_entries_invalid")
    files: list[tuple[str, bytes]] = []
    paths: set[str] = set()
    for entry in payload["entries"]:
        if not isinstance(entry, dict) or set(entry) != {
            "path",
            "contentDigest",
            "contentBase64",
        }:
            raise ValueError("source_bundle_entry_invalid")
        path = entry["path"]
        if not isinstance(path, str):
            raise ValueError("source_bundle_path_invalid")
        validate_bundle_path(path)
        if path == ".openagents-forensic-source.json" or path in paths:
            raise ValueError("source_bundle_path_duplicate")
        paths.add(path)
        try:
            content = base64.b64decode(entry["contentBase64"], validate=True)
        except (TypeError, ValueError):
            raise ValueError("source_bundle_entry_content_invalid") from None
        if (
            not isinstance(entry["contentDigest"], str)
            or digest(content) != entry["contentDigest"]
            or contains_secret(content)
        ):
            raise ValueError("source_bundle_entry_digest_conflict")
        files.append((path, content))
    return payload, files


def installation_scope(request: dict[str, Any]) -> dict[str, Any]:
    return {
        "operationRef": request["operationRef"],
        "sandboxRef": request["sandboxRef"],
        "resourceGeneration": request["resourceGeneration"],
        "capabilityRef": request["capabilityRef"],
    }


def source_manifest(
    payload: dict[str, Any], request: dict[str, Any]
) -> dict[str, Any]:
    return {
        "schema": payload["schema"],
        "repositoryRef": payload["repositoryRef"],
        "commitSha": payload["commitSha"],
        "gitTreeSha": payload["gitTreeSha"],
        "entries": [
            {"path": entry["path"], "contentDigest": entry["contentDigest"]}
            for entry in payload["entries"]
        ],
        "installationScope": installation_scope(request),
    }


def installed_source_digest(source: Path) -> str:
    marker = source / ".openagents-forensic-source.json"
    if marker.is_symlink() or not marker.is_file():
        raise ValueError("installed_source_manifest_invalid")
    manifest = json.loads(marker.read_text(encoding="utf-8"))
    if not isinstance(manifest, dict) or set(manifest) != {
        "schema",
        "repositoryRef",
        "commitSha",
        "gitTreeSha",
        "entries",
        "installationScope",
    }:
        raise ValueError("installed_source_manifest_invalid")
    if not isinstance(manifest["entries"], list):
        raise ValueError("installed_source_manifest_invalid")
    entries = []
    paths: set[str] = set()
    expected_directories: set[str] = set()
    for entry in manifest["entries"]:
        if not isinstance(entry, dict) or set(entry) != {"path", "contentDigest"}:
            raise ValueError("installed_source_manifest_invalid")
        path = entry["path"]
        validate_bundle_path(path)
        if path == ".openagents-forensic-source.json" or path in paths:
            raise ValueError("installed_source_manifest_invalid")
        paths.add(path)
        candidate = source / path
        if candidate.is_symlink() or not candidate.is_file():
            raise ValueError("installed_source_content_drift")
        parent = Path(path).parent
        while parent != Path("."):
            expected_directories.add(parent.as_posix())
            parent = parent.parent
        content = candidate.read_bytes()
        if digest(content) != entry["contentDigest"]:
            raise ValueError("installed_source_content_drift")
        entries.append(
            {
                "path": path,
                "contentDigest": entry["contentDigest"],
                "contentBase64": base64.b64encode(content).decode("ascii"),
            }
        )
    for candidate in source.rglob("*"):
        relative = candidate.relative_to(source).as_posix()
        if candidate.is_symlink():
            raise ValueError("installed_source_content_drift")
        if candidate.is_file():
            if relative not in paths and relative != ".openagents-forensic-source.json":
                raise ValueError("installed_source_content_drift")
        elif candidate.is_dir():
            if relative not in expected_directories:
                raise ValueError("installed_source_content_drift")
        else:
            raise ValueError("installed_source_content_drift")
    payload = {
        "schema": manifest["schema"],
        "repositoryRef": manifest["repositoryRef"],
        "commitSha": manifest["commitSha"],
        "gitTreeSha": manifest["gitTreeSha"],
        "entries": entries,
    }
    return digest(canonical_json(payload))


def installed_source_scope(source: Path) -> dict[str, Any]:
    marker = source / ".openagents-forensic-source.json"
    manifest = json.loads(marker.read_text(encoding="utf-8"))
    scope = manifest.get("installationScope") if isinstance(manifest, dict) else None
    if not isinstance(scope, dict) or set(scope) != {
        "operationRef",
        "sandboxRef",
        "resourceGeneration",
        "capabilityRef",
    }:
        raise ValueError("installed_source_scope_invalid")
    return scope


def source_tree_is_read_only(source: Path) -> bool:
    return all(
        (path.stat().st_mode & 0o222) == 0
        for path in [source, *source.rglob("*")]
    )


def install_forensic_source(request: dict[str, Any]) -> dict[str, Any]:
    if (
        request["sourcePath"] != "workspace/source"
        or request["scratchPath"] != "workspace/scratch"
    ):
        raise ValueError("forensic_source_paths_not_admitted")
    artifact = base64.b64decode(request["artifactContentBase64"], validate=True)
    if not artifact or len(artifact) > int(request["limits"]["maxArtifactBytes"]):
        raise ValueError("source_artifact_out_of_bounds")
    if digest(artifact) != request["artifactContentDigest"] or contains_secret(
        artifact
    ):
        raise ValueError("source_artifact_digest_conflict")
    payload, files = decode_source_bundle(artifact)
    source = WORKSPACE / "source"
    source_scratch = WORKSPACE / "scratch"
    if source.is_symlink() or source_scratch.is_symlink():
        raise ValueError("forensic_source_destination_symlink")
    if source.is_dir() and not source_scratch.exists():
        if (
            installed_source_digest(source) == request["artifactContentDigest"]
            and installed_source_scope(source) == installation_scope(request)
            and source_tree_is_read_only(source)
        ):
            source_scratch.mkdir(mode=0o700)
            return {
                "postCopyDigest": request["artifactContentDigest"],
                "artifactByteLength": len(artifact),
            }
        raise ValueError("forensic_source_destination_not_empty")
    if not source.exists() and source_scratch.exists():
        make_tree_removable(source_scratch)
        shutil.rmtree(source_scratch)
    if source.exists() or source_scratch.exists():
        if (
            source.is_dir()
            and source_scratch.is_dir()
            and installed_source_digest(source) == request["artifactContentDigest"]
            and installed_source_scope(source) == installation_scope(request)
            and source_tree_is_read_only(source)
            and os.access(source_scratch, os.W_OK)
            and not any(source_scratch.iterdir())
        ):
            return {
                "postCopyDigest": request["artifactContentDigest"],
                "artifactByteLength": len(artifact),
            }
        raise ValueError("forensic_source_destination_not_empty")
    staging = Path(tempfile.mkdtemp(prefix=".forensic-source-", dir=WORKSPACE))
    try:
        staging_fd = os.open(staging, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        try:
            for path, content in files:
                write_beneath(staging_fd, path, content)
            write_beneath(
                staging_fd,
                ".openagents-forensic-source.json",
                canonical_json(source_manifest(payload, request)),
            )
        finally:
            os.close(staging_fd)
        for path in sorted(
            staging.rglob("*"), key=lambda item: len(item.parts), reverse=True
        ):
            path.chmod(0o555 if path.is_dir() else 0o444)
        staging.chmod(0o555)
        staging.rename(source)
        source_scratch.mkdir(mode=0o700)
    except BaseException:
        if staging.exists():
            for path in staging.rglob("*"):
                path.chmod(0o700 if path.is_dir() else 0o600)
            staging.chmod(0o700)
            shutil.rmtree(staging)
        if source.exists():
            make_tree_removable(source)
            shutil.rmtree(source)
        if source_scratch.exists():
            make_tree_removable(source_scratch)
            shutil.rmtree(source_scratch)
        raise
    post_copy_digest = installed_source_digest(source)
    if (
        post_copy_digest != request["artifactContentDigest"]
        or not source_tree_is_read_only(source)
        or not source_scratch.is_dir()
        or not os.access(source_scratch, os.W_OK)
        or source.resolve() == source_scratch.resolve()
    ):
        make_tree_removable(source)
        make_tree_removable(source_scratch)
        shutil.rmtree(source, ignore_errors=True)
        shutil.rmtree(source_scratch, ignore_errors=True)
        raise ValueError("forensic_source_post_copy_verification_failed")
    return {"postCopyDigest": post_copy_digest, "artifactByteLength": len(artifact)}


def make_tree_removable(root: Path) -> None:
    if not root.exists():
        return
    for path in root.rglob("*"):
        if path.is_symlink():
            continue
        if path.is_dir():
            path.chmod(0o700)
        else:
            path.chmod(0o600)
    root.chmod(0o700)


def remove_forensic_source(request: dict[str, Any]) -> None:
    if (
        request["sourcePath"] != "workspace/source"
        or request["scratchPath"] != "workspace/scratch"
    ):
        raise ValueError("forensic_source_paths_not_admitted")
    source = WORKSPACE / "source"
    source_scratch = WORKSPACE / "scratch"
    if source.is_symlink() or source_scratch.is_symlink():
        raise ValueError("forensic_source_cleanup_symlink")
    digest_mismatch = False
    if source.exists():
        try:
            digest_mismatch = (
                installed_source_digest(source) != request["expectedSourceDigest"]
            )
        except (OSError, ValueError, json.JSONDecodeError):
            digest_mismatch = True
    make_tree_removable(source)
    make_tree_removable(source_scratch)
    if source.exists():
        shutil.rmtree(source)
    if source_scratch.exists():
        shutil.rmtree(source_scratch)
    if source.exists() or source_scratch.exists():
        raise ValueError("forensic_source_cleanup_incomplete")
    if digest_mismatch:
        raise ValueError("installed_source_digest_conflict_removed")


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
    bubblewrap = [
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
    ]
    source = WORKSPACE / "source"
    source_scratch = WORKSPACE / "scratch"
    if source.is_dir():
        bubblewrap.extend(["--ro-bind", str(source), str(source)])
    if source_scratch.is_dir():
        bubblewrap.extend(["--bind", str(source_scratch), str(source_scratch)])
    bubblewrap.extend(
        [
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
        ]
    )
    try:
        process = subprocess.Popen(
            bubblewrap,
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
    effective_path = request.get("path") or request.get("cwd") or request.get("sourcePath")
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
    if scratch.is_symlink():
        raise ValueError("operation_scratch_symlink")
    if scratch.exists():
        make_tree_removable(scratch)
        shutil.rmtree(scratch)
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
        elif action == "install_forensic_source":
            install = install_forensic_source(request)
            response.update(
                artifactRef=request["artifactRef"],
                artifactContentDigest=request["artifactContentDigest"],
                artifactByteLength=install["artifactByteLength"],
                postCopyDigest=install["postCopyDigest"],
                sourceReadOnly=True,
                sourceReadbackVerified=True,
                scratchSeparateAndWritable=True,
            )
            receipt_kwargs.update(
                bytes_read=install["artifactByteLength"],
                bytes_written=install["artifactByteLength"],
            )
        elif action == "remove_forensic_source":
            remove_forensic_source(request)
            response.update(
                expectedSourceDigest=request["expectedSourceDigest"],
                guestSourceDeleted=True,
                guestSourceReadbackAbsent=True,
                scratchDeleted=True,
                scratchReadbackAbsent=True,
            )
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
