#!/usr/bin/env python3
"""Create, verify, and restore bounded content-only sandbox checkpoints."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import sys
import tarfile
from pathlib import Path, PurePosixPath
from typing import Any, BinaryIO, Iterable


WORKSPACE = Path("/workspace")
SCRATCH_ROOT = Path("/var/lib/openagents/managed-sandbox-checkpoints")
FORMAT_REF = "format.sbx.content-tar.v1"
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_CONTENT_BYTES = 480 * 1024 * 1024
MAX_FILE_BYTES = 64 * 1024 * 1024
MAX_ENTRIES = 100_000
CHUNK_BYTES = 1024 * 1024

RESTORE_STAGE = ".openagents-checkpoint-restore-stage"
RESTORE_BACKUP = ".openagents-checkpoint-restore-backup"
RESTORE_STATE = ".openagents-checkpoint-restore-state.json"
RESTORE_STATE_TEMP = ".openagents-checkpoint-restore-state.tmp"
INTERNAL_NAMES = frozenset(
    {RESTORE_STAGE, RESTORE_BACKUP, RESTORE_STATE, RESTORE_STATE_TEMP}
)

EXCLUDED_DIRECTORIES = frozenset(
    {
        ".git",
        ".ssh",
        ".gnupg",
        ".aws",
        ".azure",
        ".config",
        ".kube",
        ".docker",
        ".terraform",
    }
)
EXCLUDED_FILES = frozenset(
    {
        ".npmrc",
        ".pypirc",
        ".netrc",
        "auth.json",
        "credentials",
        "credentials.json",
        "application_default_credentials.json",
        "id_rsa",
        "id_ed25519",
    }
)
EXCLUDED_SUFFIXES = (
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    ".jks",
    ".keystore",
)


class CheckpointError(Exception):
    """A fixed public-safe checkpoint refusal."""

    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def _sha256_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(CHUNK_BYTES), b""):
            hasher.update(chunk)
    return f"sha256:{hasher.hexdigest()}"


def _sha256_stream(source: BinaryIO) -> str:
    hasher = hashlib.sha256()
    for chunk in iter(lambda: source.read(CHUNK_BYTES), b""):
        hasher.update(chunk)
    return f"sha256:{hasher.hexdigest()}"


def _manifest_digest(entries: Iterable[dict[str, Any]]) -> str:
    encoded = json.dumps(
        list(entries),
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def _is_excluded(relative: PurePosixPath) -> bool:
    lowered = tuple(part.lower() for part in relative.parts)
    if not lowered:
        return False
    if lowered[0] in INTERNAL_NAMES:
        return True
    if any(part in EXCLUDED_DIRECTORIES for part in lowered):
        return True
    name = lowered[-1]
    if name in EXCLUDED_FILES or name == ".env" or name.startswith(".env."):
        return True
    return name.endswith(EXCLUDED_SUFFIXES)


def _safe_relative(value: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if path.is_absolute() or not path.parts:
        raise CheckpointError("checkpoint_path_invalid")
    if any(part in {"", ".", ".."} for part in path.parts):
        raise CheckpointError("checkpoint_path_invalid")
    if _is_excluded(path):
        raise CheckpointError("checkpoint_contains_excluded_path")
    return path


def _workspace_entries(workspace: Path) -> list[tuple[PurePosixPath, Path, os.stat_result]]:
    if not workspace.is_dir() or workspace.is_symlink():
        raise CheckpointError("checkpoint_workspace_invalid")
    entries: list[tuple[PurePosixPath, Path, os.stat_result]] = []

    def walk(directory: Path, relative_directory: PurePosixPath | None) -> None:
        try:
            children = sorted(os.scandir(directory), key=lambda item: item.name.encode("utf-8"))
        except OSError as error:
            raise CheckpointError("checkpoint_workspace_read_failed") from error
        for child in children:
            relative = (
                PurePosixPath(child.name)
                if relative_directory is None
                else relative_directory / child.name
            )
            if _is_excluded(relative):
                continue
            try:
                metadata = child.stat(follow_symlinks=False)
            except OSError as error:
                raise CheckpointError("checkpoint_workspace_read_failed") from error
            mode = metadata.st_mode
            if stat.S_ISLNK(mode):
                raise CheckpointError("checkpoint_symlink_refused")
            if not stat.S_ISDIR(mode) and not stat.S_ISREG(mode):
                raise CheckpointError("checkpoint_special_file_refused")
            entries.append((relative, Path(child.path), metadata))
            if len(entries) > MAX_ENTRIES:
                raise CheckpointError("checkpoint_entry_limit_exceeded")
            if stat.S_ISDIR(mode):
                walk(Path(child.path), relative)

    walk(workspace, None)
    return entries


def _regular_mode(mode: int) -> int:
    return 0o755 if mode & 0o111 else 0o644


def _entry_manifest(
    entries: Iterable[tuple[PurePosixPath, Path, os.stat_result]],
) -> tuple[list[dict[str, Any]], int]:
    manifest: list[dict[str, Any]] = []
    content_bytes = 0
    for relative, source, metadata in entries:
        if stat.S_ISDIR(metadata.st_mode):
            manifest.append({"mode": 0o755, "path": relative.as_posix(), "type": "directory"})
            continue
        if metadata.st_size > MAX_FILE_BYTES:
            raise CheckpointError("checkpoint_file_limit_exceeded")
        content_bytes += metadata.st_size
        if content_bytes > MAX_CONTENT_BYTES:
            raise CheckpointError("checkpoint_content_limit_exceeded")
        manifest.append(
            {
                "digest": _sha256_file(source),
                "mode": _regular_mode(metadata.st_mode),
                "path": relative.as_posix(),
                "size": metadata.st_size,
                "type": "file",
            }
        )
    return manifest, content_bytes


def _tar_info(relative: PurePosixPath, metadata: os.stat_result) -> tarfile.TarInfo:
    info = tarfile.TarInfo(relative.as_posix())
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    info.pax_headers = {}
    if stat.S_ISDIR(metadata.st_mode):
        info.type = tarfile.DIRTYPE
        info.mode = 0o755
        info.size = 0
    else:
        info.type = tarfile.REGTYPE
        info.mode = _regular_mode(metadata.st_mode)
        info.size = metadata.st_size
    return info


def create_checkpoint(workspace: Path, archive_path: Path) -> dict[str, Any]:
    entries = _workspace_entries(workspace)
    manifest, unpacked_bytes = _entry_manifest(entries)
    archive_path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    temporary = archive_path.with_name(f".{archive_path.name}.partial")
    try:
        with tarfile.open(temporary, "w", format=tarfile.PAX_FORMAT) as archive:
            for relative, source, metadata in entries:
                info = _tar_info(relative, metadata)
                if stat.S_ISDIR(metadata.st_mode):
                    archive.addfile(info)
                else:
                    with source.open("rb") as content:
                        archive.addfile(info, content)
        archive_bytes = temporary.stat().st_size
        if archive_bytes > MAX_ARCHIVE_BYTES:
            raise CheckpointError("checkpoint_archive_limit_exceeded")
        os.chmod(temporary, 0o600)
        os.replace(temporary, archive_path)
    except CheckpointError:
        temporary.unlink(missing_ok=True)
        raise
    except (OSError, tarfile.TarError) as error:
        temporary.unlink(missing_ok=True)
        raise CheckpointError("checkpoint_create_failed") from error
    content_digest = _sha256_file(archive_path)
    inspection = inspect_checkpoint(archive_path, content_digest)
    if (
        inspection["entryCount"] != len(entries)
        or inspection["unpackedBytes"] != unpacked_bytes
        or inspection["repositoryPostImageDigest"] != _manifest_digest(manifest)
    ):
        archive_path.unlink(missing_ok=True)
        raise CheckpointError("checkpoint_source_changed")
    return inspection


def inspect_checkpoint(archive_path: Path, expected_digest: str) -> dict[str, Any]:
    try:
        archive_bytes = archive_path.stat().st_size
    except OSError as error:
        raise CheckpointError("checkpoint_archive_missing") from error
    if archive_bytes > MAX_ARCHIVE_BYTES:
        raise CheckpointError("checkpoint_archive_limit_exceeded")
    if _sha256_file(archive_path) != expected_digest:
        raise CheckpointError("checkpoint_digest_mismatch")

    names: set[str] = set()
    file_bytes = 0
    manifest: list[dict[str, Any]] = []
    try:
        with tarfile.open(archive_path, "r:") as archive:
            members = archive.getmembers()
            if len(members) > MAX_ENTRIES:
                raise CheckpointError("checkpoint_entry_limit_exceeded")
            for member in members:
                relative = _safe_relative(member.name)
                name = relative.as_posix()
                if name in names:
                    raise CheckpointError("checkpoint_duplicate_path")
                names.add(name)
                if member.isdir():
                    manifest.append({"mode": 0o755, "path": name, "type": "directory"})
                    continue
                if not member.isfile() or member.size < 0:
                    raise CheckpointError("checkpoint_special_file_refused")
                if member.size > MAX_FILE_BYTES:
                    raise CheckpointError("checkpoint_file_limit_exceeded")
                file_bytes += member.size
                if file_bytes > MAX_CONTENT_BYTES:
                    raise CheckpointError("checkpoint_content_limit_exceeded")
                source = archive.extractfile(member)
                if source is None:
                    raise CheckpointError("checkpoint_archive_invalid")
                digest = _sha256_stream(source)
                manifest.append(
                    {
                        "digest": digest,
                        "mode": 0o755 if member.mode & 0o111 else 0o644,
                        "path": name,
                        "size": member.size,
                        "type": "file",
                    }
                )
    except CheckpointError:
        raise
    except (OSError, tarfile.TarError) as error:
        raise CheckpointError("checkpoint_archive_invalid") from error
    return {
        "formatRef": FORMAT_REF,
        "contentDigest": expected_digest,
        "contentBytes": archive_bytes,
        "unpackedBytes": file_bytes,
        "entryCount": len(names),
        "repositoryPostImageDigest": _manifest_digest(manifest),
    }


def _remove_path(path: Path) -> None:
    if path.is_symlink() or path.is_file():
        path.unlink(missing_ok=True)
    elif path.exists():
        shutil.rmtree(path)


def _move_children(source: Path, destination: Path) -> None:
    for child in sorted(source.iterdir(), key=lambda value: value.name.encode("utf-8")):
        os.replace(child, destination / child.name)


def _write_restore_state(workspace: Path, content_digest: str, state: str) -> None:
    state_path = workspace / RESTORE_STATE
    temporary = workspace / RESTORE_STATE_TEMP
    with temporary.open("w", encoding="utf-8") as output:
        json.dump(
            {"contentDigest": content_digest, "state": state},
            output,
            separators=(",", ":"),
            sort_keys=True,
        )
        output.flush()
        os.fsync(output.fileno())
    os.chmod(temporary, 0o600)
    os.replace(temporary, state_path)


def recover_restore(workspace: Path) -> bool:
    state_path = workspace / RESTORE_STATE
    backup = workspace / RESTORE_BACKUP
    stage = workspace / RESTORE_STAGE
    if not state_path.exists() and not backup.exists() and not stage.exists():
        return False
    if not state_path.is_file():
        raise CheckpointError("checkpoint_restore_state_invalid")
    try:
        state = json.loads(state_path.read_text(encoding="utf-8"))["state"]
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise CheckpointError("checkpoint_restore_state_invalid") from error
    if state in {"prepared", "backing_up"} and backup.is_dir():
        _move_children(backup, workspace)
    elif state in {"installing", "verifying"} and backup.is_dir():
        for child in list(workspace.iterdir()):
            if child.name not in INTERNAL_NAMES:
                _remove_path(child)
        _move_children(backup, workspace)
    elif state == "committed":
        pass
    else:
        raise CheckpointError("checkpoint_restore_state_invalid")
    _remove_path(stage)
    _remove_path(backup)
    (workspace / RESTORE_STATE_TEMP).unlink(missing_ok=True)
    state_path.unlink(missing_ok=True)
    return True


def restore_checkpoint(workspace: Path, archive_path: Path, expected_digest: str) -> dict[str, Any]:
    inspection = inspect_checkpoint(archive_path, expected_digest)
    workspace.mkdir(parents=True, exist_ok=True, mode=0o700)
    recover_restore(workspace)
    stage = workspace / RESTORE_STAGE
    backup = workspace / RESTORE_BACKUP
    state_path = workspace / RESTORE_STATE
    stage.mkdir(mode=0o700)
    backup.mkdir(mode=0o700)
    _write_restore_state(workspace, expected_digest, "prepared")

    try:
        with tarfile.open(archive_path, "r:") as archive:
            for member in archive.getmembers():
                relative = _safe_relative(member.name)
                destination = stage.joinpath(*relative.parts)
                if member.isdir():
                    destination.mkdir(parents=True, exist_ok=True, mode=0o755)
                    continue
                destination.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
                source = archive.extractfile(member)
                if source is None:
                    raise CheckpointError("checkpoint_archive_invalid")
                descriptor = os.open(
                    destination,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                    0o755 if member.mode & 0o111 else 0o644,
                )
                try:
                    with os.fdopen(descriptor, "wb", closefd=False) as output:
                        shutil.copyfileobj(source, output, CHUNK_BYTES)
                        output.flush()
                        os.fsync(output.fileno())
                finally:
                    os.close(descriptor)

        if inspect_checkpoint(archive_path, expected_digest) != inspection:
            raise CheckpointError("checkpoint_archive_changed")
        _write_restore_state(workspace, expected_digest, "backing_up")
        for child in list(workspace.iterdir()):
            if child.name not in INTERNAL_NAMES:
                os.replace(child, backup / child.name)
        _write_restore_state(workspace, expected_digest, "installing")
        _move_children(stage, workspace)
        _write_restore_state(workspace, expected_digest, "verifying")
        restored_entries = _workspace_entries(workspace)
        restored_manifest, _ = _entry_manifest(restored_entries)
        if _manifest_digest(restored_manifest) != inspection["repositoryPostImageDigest"]:
            raise CheckpointError("checkpoint_restore_verification_failed")
        _write_restore_state(workspace, expected_digest, "committed")
        _remove_path(backup)
        _remove_path(stage)
        state_path.unlink(missing_ok=True)
        return {**inspection, "recoveredInterruptedRestore": False}
    except CheckpointError:
        recover_restore(workspace)
        raise
    except (OSError, tarfile.TarError) as error:
        recover_restore(workspace)
        raise CheckpointError("checkpoint_restore_failed") from error


def _archive_path(value: str) -> Path:
    path = Path(value)
    if not path.is_absolute():
        raise CheckpointError("checkpoint_archive_path_invalid")
    try:
        path.resolve(strict=False).relative_to(SCRATCH_ROOT)
    except ValueError as error:
        raise CheckpointError("checkpoint_archive_path_invalid") from error
    return path


def _write_result(result: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(result, separators=(",", ":"), sort_keys=True))
    sys.stdout.write("\n")


def main() -> int:
    try:
        if len(sys.argv) not in {3, 4}:
            raise CheckpointError("checkpoint_usage_invalid")
        action = sys.argv[1]
        archive_path = _archive_path(sys.argv[2])
        if action == "create" and len(sys.argv) == 3:
            _write_result(create_checkpoint(WORKSPACE, archive_path))
            return 0
        if action == "verify" and len(sys.argv) == 4:
            _write_result(inspect_checkpoint(archive_path, sys.argv[3]))
            return 0
        if action == "restore" and len(sys.argv) == 4:
            _write_result(restore_checkpoint(WORKSPACE, archive_path, sys.argv[3]))
            return 0
        raise CheckpointError("checkpoint_usage_invalid")
    except CheckpointError as error:
        sys.stderr.write(f"managed-sandbox checkpoint refused: {error.code}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
