#!/usr/bin/env python3
"""Host-side managed-sandbox phase-2 driver: checkpoint, restore, fork, delete."""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TARGET_SCHEMA_VERSION = "openagents.managed_sandbox_phase2_target.v1"
ERROR_SCHEMA_VERSION = "openagents.managed_sandbox_phase2_driver_error.v1"
CHECKPOINT_SCHEMA_VERSION = "openagents.managed_sandbox_content_checkpoint.v1"
DELETE_SCHEMA_VERSION = "openagents.managed_sandbox_checkpoint_delete_receipt.v1"
FORK_SCHEMA_VERSION = "openagents.managed_sandbox_fork_receipt.v1"
RESTORE_SCHEMA_VERSION = "openagents.managed_sandbox_restore_receipt.v1"
FORMAT_REF = "format.sbx.content-tar.v1"
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_COMMAND_BYTES = 2 * 1024 * 1024


class DriverError(Exception):
    pass


def sha256_hex(value: str | bytes) -> str:
    if isinstance(value, str):
        value = value.encode("utf-8")
    return hashlib.sha256(value).hexdigest()


def sha256_ref(value: str) -> str:
    return f"sha256:{sha256_hex(value)}"


def evidence_ref(kind: str, value: str) -> str:
    return f"evidence.sbx10.{kind}.{sha256_hex(value)[:32]}"


def canonical_json(value: Any) -> str:
    if isinstance(value, list):
        return "[" + ",".join(canonical_json(item) for item in value) + "]"
    if isinstance(value, dict):
        return (
            "{"
            + ",".join(
                f"{json.dumps(key)}:{canonical_json(value[key])}"
                for key in sorted(value.keys())
            )
            + "}"
        )
    return json.dumps(value, separators=(",", ":"))


def required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise DriverError("configuration_unavailable")
    return value


def iso_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"


def read_request() -> dict[str, Any]:
    raw = sys.stdin.buffer.read()
    if not raw or len(raw) > MAX_COMMAND_BYTES:
        raise DriverError("request_invalid")
    try:
        request = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise DriverError("request_invalid") from error
    if (
        not isinstance(request, dict)
        or request.get("schemaVersion") != TARGET_SCHEMA_VERSION
        or not isinstance(request.get("action"), str)
        or not isinstance(request.get("requestRef"), str)
    ):
        raise DriverError("request_invalid")
    return request


def gcloud_run(gcloud: str, args: list[str], *, timeout: int = 120, max_buffer: int = 2 * 1024 * 1024) -> str:
    result = subprocess.run(
        [gcloud, *args],
        check=False,
        capture_output=True,
        timeout=timeout,
    )
    if result.returncode != 0:
        raise DriverError("gcloud_operation_failed")
    if len(result.stdout) > max_buffer:
        raise DriverError("gcloud_operation_failed")
    return result.stdout.decode("utf-8", errors="replace")


def gcloud_status(gcloud: str, args: list[str], *, timeout: int = 120) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [gcloud, *args],
        check=False,
        capture_output=True,
        timeout=timeout,
    )


class Phase2:
    def __init__(self) -> None:
        self.project = required("OA_MANAGED_SANDBOX_PROJECT_ID")
        self.zone = required("OA_MANAGED_SANDBOX_ZONE")
        self.bucket = required("OA_MANAGED_SANDBOX_PHASE2_BUCKET")
        self.gcloud = os.environ.get("OA_MANAGED_SANDBOX_GCLOUD_BIN", "").strip() or "gcloud"
        if not re.fullmatch(r"[a-z0-9][a-z0-9._-]{1,220}[a-z0-9]", self.bucket):
            raise DriverError("bucket_invalid")

    def instance_name(self, sandbox_ref: str) -> str:
        return f"oa-msb-{sha256_hex(sandbox_ref)[:20]}"

    def generation_marker(self, sandbox_ref: str, generation: int) -> str:
        resource_ref = f"gce-instance-ref://sha256/{sha256_hex(f'resource|{sandbox_ref}')}"
        disk_ref = f"gce-disk-ref://sha256/{sha256_hex(f'disk|{sandbox_ref}')}"
        return sha256_hex(f"{resource_ref}|{disk_ref}|{generation}")[:20]

    def object_uri(self, owner_ref: str, tenant_ref: str, checkpoint_ref: str) -> str:
        return (
            f"gs://{self.bucket}/managed-sandbox-checkpoints/v1/"
            f"{sha256_hex(f'{owner_ref}|{tenant_ref}')[:32]}/{sha256_hex(checkpoint_ref)}.tar"
        )

    def remote_args(self, sandbox_ref: str, command: str) -> list[str]:
        return [
            "compute",
            "ssh",
            f"openagents@{self.instance_name(sandbox_ref)}",
            "--project",
            self.project,
            "--zone",
            self.zone,
            "--internal-ip",
            "--quiet",
            "--ssh-key-expire-after=10m",
            "--ssh-flag=-oStrictHostKeyChecking=no",
            "--ssh-flag=-oUserKnownHostsFile=/dev/null",
            "--command",
            command,
        ]

    def remote_checkpoint(self, request_ref: str) -> dict[str, str]:
        key = sha256_hex(request_ref)[:32]
        directory = f"/var/lib/openagents/managed-sandbox-checkpoints/{key}"
        return {"directory": directory, "archive": f"{directory}/content.tar"}

    def remote_create(self, sandbox_ref: str, request_ref: str) -> dict[str, Any]:
        remote = self.remote_checkpoint(request_ref)
        command = (
            f"set -eu; install -d -m 0700 {remote['directory']}; "
            f"/usr/bin/python3 /opt/openagents-managed-sandbox/managed-sandbox-guest-checkpoint.py "
            f"create {remote['archive']}"
        )
        output = gcloud_run(
            self.gcloud,
            self.remote_args(sandbox_ref, command),
            timeout=10 * 60,
            max_buffer=1024 * 1024,
        )
        try:
            result = json.loads(output)
        except json.JSONDecodeError as error:
            raise DriverError("guest_checkpoint_invalid") from error
        return {"remote": remote, "result": result}

    def remote_cleanup(self, sandbox_ref: str, remote: dict[str, str]) -> None:
        result = gcloud_status(
            self.gcloud,
            self.remote_args(
                sandbox_ref, f"rm -f {remote['archive']}; rmdir {remote['directory']}"
            ),
        )
        if result.returncode != 0:
            raise DriverError("guest_checkpoint_cleanup_failed")

    def copy_from_guest(self, sandbox_ref: str, remote_archive: str, local_archive: str) -> None:
        gcloud_run(
            self.gcloud,
            [
                "compute",
                "scp",
                f"openagents@{self.instance_name(sandbox_ref)}:{remote_archive}",
                local_archive,
                "--project",
                self.project,
                "--zone",
                self.zone,
                "--internal-ip",
                "--quiet",
                "--ssh-key-expire-after=10m",
                "--scp-flag=-oStrictHostKeyChecking=no",
                "--scp-flag=-oUserKnownHostsFile=/dev/null",
            ],
            timeout=5 * 60,
        )

    def copy_to_guest(self, sandbox_ref: str, local_archive: str, remote_archive: str) -> None:
        gcloud_run(
            self.gcloud,
            [
                "compute",
                "scp",
                local_archive,
                f"openagents@{self.instance_name(sandbox_ref)}:{remote_archive}",
                "--project",
                self.project,
                "--zone",
                self.zone,
                "--internal-ip",
                "--quiet",
                "--ssh-key-expire-after=10m",
                "--scp-flag=-oStrictHostKeyChecking=no",
                "--scp-flag=-oUserKnownHostsFile=/dev/null",
            ],
            timeout=5 * 60,
        )

    def remote_prepare(self, sandbox_ref: str, remote: dict[str, str]) -> None:
        gcloud_run(
            self.gcloud, self.remote_args(sandbox_ref, f"install -d -m 0700 {remote['directory']}")
        )

    def remote_restore(self, sandbox_ref: str, request_ref: str, content_digest: str) -> dict[str, Any]:
        remote = self.remote_checkpoint(request_ref)
        command = (
            f"set -eu; install -d -m 0700 {remote['directory']}; "
            f"/usr/bin/python3 /opt/openagents-managed-sandbox/managed-sandbox-guest-checkpoint.py "
            f"restore {remote['archive']} {content_digest}"
        )
        output = gcloud_run(
            self.gcloud,
            self.remote_args(sandbox_ref, command),
            timeout=10 * 60,
            max_buffer=1024 * 1024,
        )
        try:
            return {"remote": remote, "result": json.loads(output)}
        except json.JSONDecodeError as error:
            raise DriverError("guest_restore_invalid") from error

    def hash_file(self, path: str) -> str:
        digest = hashlib.sha256()
        with open(path, "rb") as handle:
            while True:
                chunk = handle.read(1024 * 1024)
                if not chunk:
                    break
                digest.update(chunk)
        return f"sha256:{digest.hexdigest()}"

    def describe_object(self, uri: str) -> dict[str, Any] | None:
        result = gcloud_status(self.gcloud, ["storage", "objects", "describe", uri, "--format=json"])
        if result.returncode != 0:
            return None
        try:
            value = json.loads(result.stdout.decode("utf-8"))
        except json.JSONDecodeError as error:
            raise DriverError("checkpoint_object_metadata_invalid") from error
        if not isinstance(value, dict):
            return None
        metadata = value.get("custom_fields") or value.get("metadata") or {}
        if not isinstance(metadata, dict):
            raise DriverError("checkpoint_object_metadata_invalid")
        return {**value, "metadata": metadata}

    def download_object(self, uri: str, path: str) -> dict[str, Any]:
        obj = self.describe_object(uri)
        advertised_size = int(obj.get("size") or -1) if obj else -1
        if advertised_size < 0 or advertised_size > MAX_ARCHIVE_BYTES:
            raise DriverError("checkpoint_object_size_invalid")
        gcloud_run(self.gcloud, ["storage", "cp", uri, path, "--quiet"], timeout=5 * 60)
        size = Path(path).stat().st_size
        if size > MAX_ARCHIVE_BYTES:
            raise DriverError("checkpoint_object_too_large")
        return {"contentBytes": size, "contentDigest": self.hash_file(path)}

    def checkpoint_result(
        self, command: dict[str, Any], content: dict[str, Any], completed_at: str, verified_at: str
    ) -> dict[str, Any]:
        return {
            "schema": CHECKPOINT_SCHEMA_VERSION,
            "checkpointRef": command["checkpointRef"],
            "ownerRef": command["ownerRef"],
            "tenantRef": command["tenantRef"],
            "sourceSandboxRef": command["sourceSandboxRef"],
            "sourceResourceGeneration": command["sourceResourceGeneration"],
            "sourceImageDigest": command.get("sourceImageDigest"),
            "sourceToolchainDigest": command.get("sourceToolchainDigest"),
            "repositoryRef": command.get("repositoryRef"),
            "repositoryRevisionRef": command.get("repositoryRevisionRef"),
            "repositoryPostImageDigest": command["repositoryPostImageDigest"],
            "contentDigest": content["contentDigest"],
            "contentBytes": content["contentBytes"],
            "formatRef": command["formatRef"],
            "state": "completed",
            "completedAt": completed_at,
            "verifiedAt": verified_at,
            "retainedUntil": command["retainedUntil"],
            "deleteOnExpiry": True,
            "omissions": {
                "credentials": "excluded",
                "accountSecrets": "excluded",
                "providerHiddenState": "excluded",
                "processMemory": "excluded",
                "processTable": "excluded",
                "ptyState": "excluded",
                "sockets": "excluded",
                "ports": "excluded",
                "networkIdentity": "excluded",
            },
            "evidenceRefs": [evidence_ref("checkpoint.object", command["checkpointRef"])],
        }

    def validate_create(self, request: dict[str, Any]) -> dict[str, Any]:
        command = request.get("command")
        if (
            not isinstance(command, dict)
            or command.get("_tag") != "CreateCheckpoint"
            or command.get("commandRef") != request["requestRef"]
            or command.get("formatRef") != FORMAT_REF
            or not isinstance(command.get("ownerRef"), str)
            or not isinstance(command.get("tenantRef"), str)
            or not isinstance(command.get("checkpointRef"), str)
            or not isinstance(command.get("sourceSandboxRef"), str)
            or not isinstance(command.get("sourceResourceGeneration"), int)
            or not isinstance(command.get("repositoryPostImageDigest"), str)
            or not isinstance(command.get("retainedUntil"), str)
            or not isinstance(command.get("requestedAt"), str)
        ):
            raise DriverError("create_request_invalid")
        return command

    def verified_local_content(
        self, path: str, expected_digest: str, expected_bytes: int
    ) -> dict[str, Any]:
        content_bytes = Path(path).stat().st_size
        content_digest = self.hash_file(path)
        if content_bytes != expected_bytes or content_digest != expected_digest:
            raise DriverError("checkpoint_content_mismatch")
        return {"contentBytes": content_bytes, "contentDigest": content_digest}

    def create_checkpoint(self, request: dict[str, Any]) -> dict[str, Any]:
        command = self.validate_create(request)
        retained_at = datetime.fromisoformat(command["retainedUntil"].replace("Z", "+00:00")).timestamp()
        requested_at = datetime.fromisoformat(command["requestedAt"].replace("Z", "+00:00")).timestamp()
        observed_at = max(datetime.now(timezone.utc).timestamp(), requested_at)
        if retained_at <= observed_at:
            raise DriverError("checkpoint_retention_expired")
        completed_at = datetime.fromtimestamp(observed_at, timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%S.%f"
        )[:-3] + "Z"
        fingerprint = sha256_ref(canonical_json(command))
        uri = self.object_uri(command["ownerRef"], command["tenantRef"], command["checkpointRef"])
        local = Path(tempfile.mkdtemp(prefix="oa-msb-phase2-create-"))
        local_archive = str(local / "content.tar")
        remote = None
        try:
            existing = self.describe_object(uri)
            if existing is not None:
                metadata = existing.get("metadata") or {}
                if metadata.get("oa_command_fingerprint") != fingerprint:
                    raise DriverError("checkpoint_idempotency_conflict")
                content = self.download_object(uri, local_archive)
                if (
                    content["contentDigest"] != metadata.get("oa_content_digest")
                    or str(content["contentBytes"]) != metadata.get("oa_content_bytes")
                    or metadata.get("oa_repository_post_image_digest")
                    != command["repositoryPostImageDigest"]
                    or metadata.get("oa_retained_until") != command["retainedUntil"]
                    or not isinstance(metadata.get("oa_completed_at"), str)
                    or not isinstance(metadata.get("oa_verified_at"), str)
                ):
                    raise DriverError("checkpoint_object_corrupt")
                return self.checkpoint_result(
                    command, content, metadata["oa_completed_at"], metadata["oa_verified_at"]
                )
            created = self.remote_create(command["sourceSandboxRef"], request["requestRef"])
            remote = created["remote"]
            result = created["result"]
            if (
                result.get("formatRef") != FORMAT_REF
                or result.get("repositoryPostImageDigest") != command["repositoryPostImageDigest"]
                or not isinstance(result.get("contentDigest"), str)
                or not isinstance(result.get("contentBytes"), int)
                or result["contentBytes"] < 0
                or result["contentBytes"] > MAX_ARCHIVE_BYTES
            ):
                raise DriverError("guest_checkpoint_scope_conflict")
            self.copy_from_guest(command["sourceSandboxRef"], remote["archive"], local_archive)
            content = self.verified_local_content(
                local_archive, result["contentDigest"], result["contentBytes"]
            )
            metadata = ",".join(
                [
                    f"oa_command_fingerprint={fingerprint}",
                    f"oa_content_digest={content['contentDigest']}",
                    f"oa_content_bytes={content['contentBytes']}",
                    f"oa_repository_post_image_digest={command['repositoryPostImageDigest']}",
                    f"oa_completed_at={completed_at}",
                    f"oa_verified_at={completed_at}",
                    f"oa_retained_until={command['retainedUntil']}",
                ]
            )
            upload = gcloud_status(
                self.gcloud,
                [
                    "storage",
                    "cp",
                    local_archive,
                    uri,
                    "--if-generation-match=0",
                    f"--custom-metadata={metadata}",
                    "--quiet",
                ],
                timeout=5 * 60,
            )
            if upload.returncode != 0:
                raced = self.describe_object(uri)
                if (raced or {}).get("metadata", {}).get("oa_command_fingerprint") != fingerprint:
                    raise DriverError("checkpoint_idempotency_conflict")
            stored = self.describe_object(uri)
            stored_metadata = (stored or {}).get("metadata") or {}
            if (
                stored_metadata.get("oa_command_fingerprint") != fingerprint
                or stored_metadata.get("oa_content_digest") != content["contentDigest"]
                or stored_metadata.get("oa_content_bytes") != str(content["contentBytes"])
                or stored_metadata.get("oa_repository_post_image_digest")
                != command["repositoryPostImageDigest"]
                or stored_metadata.get("oa_retained_until") != command["retainedUntil"]
                or not isinstance(stored_metadata.get("oa_completed_at"), str)
                or not isinstance(stored_metadata.get("oa_verified_at"), str)
            ):
                raise DriverError("checkpoint_object_metadata_invalid")
            readback = self.download_object(uri, str(local / "readback.tar"))
            if (
                readback["contentDigest"] != content["contentDigest"]
                or readback["contentBytes"] != content["contentBytes"]
            ):
                raise DriverError("checkpoint_readback_failed")
            return self.checkpoint_result(
                command,
                content,
                stored_metadata["oa_completed_at"],
                stored_metadata["oa_verified_at"],
            )
        finally:
            if remote is not None:
                self.remote_cleanup(command["sourceSandboxRef"], remote)
            shutil.rmtree(local, ignore_errors=True)

    def validate_checkpoint(self, request: dict[str, Any]) -> dict[str, Any]:
        checkpoint = request.get("checkpoint")
        if (
            not isinstance(checkpoint, dict)
            or checkpoint.get("schema") != CHECKPOINT_SCHEMA_VERSION
            or checkpoint.get("checkpointRef") != request["requestRef"]
            or checkpoint.get("formatRef") != FORMAT_REF
            or not isinstance(checkpoint.get("ownerRef"), str)
            or not isinstance(checkpoint.get("tenantRef"), str)
            or not isinstance(checkpoint.get("contentDigest"), str)
            or not isinstance(checkpoint.get("contentBytes"), int)
            or checkpoint["contentBytes"] < 0
            or checkpoint["contentBytes"] > MAX_ARCHIVE_BYTES
        ):
            raise DriverError("checkpoint_request_invalid")
        return checkpoint

    def verify_checkpoint(self, request: dict[str, Any]) -> dict[str, Any]:
        checkpoint = self.validate_checkpoint(request)
        uri = self.object_uri(
            checkpoint["ownerRef"], checkpoint["tenantRef"], checkpoint["checkpointRef"]
        )
        if self.describe_object(uri) is None:
            raise DriverError("checkpoint_object_missing")
        local = Path(tempfile.mkdtemp(prefix="oa-msb-phase2-verify-"))
        try:
            content = self.download_object(uri, str(local / "content.tar"))
            verified = (
                content["contentDigest"] == checkpoint["contentDigest"]
                and content["contentBytes"] == checkpoint["contentBytes"]
            )
            return {
                "verified": verified,
                "checkpointRef": checkpoint["checkpointRef"],
                "contentDigest": checkpoint["contentDigest"],
                "evidenceRefs": [evidence_ref("checkpoint.readback", checkpoint["checkpointRef"])],
            }
        finally:
            shutil.rmtree(local, ignore_errors=True)

    def observe_generation(self, sandbox_ref: str) -> int:
        serial = gcloud_run(
            self.gcloud,
            [
                "compute",
                "instances",
                "get-serial-port-output",
                self.instance_name(sandbox_ref),
                "--project",
                self.project,
                "--zone",
                self.zone,
                "--port",
                "1",
            ],
            max_buffer=4 * 1024 * 1024,
        )
        generations = []
        for match in re.finditer(r"OA_MSB_(?:READY|PROBE):([a-f0-9]{20}):([0-9]+)", serial):
            marker, generation_text = match.group(1), match.group(2)
            generation = int(generation_text)
            if generation >= 0 and marker == self.generation_marker(sandbox_ref, generation):
                generations.append(generation)
        if not generations:
            raise DriverError("generation_unavailable")
        return max(generations)

    def restore_checkpoint(self, request: dict[str, Any]) -> dict[str, Any]:
        command = request.get("command")
        checkpoint = request.get("checkpoint")
        runtime = request.get("runtimeContext")
        if (
            not isinstance(command, dict)
            or command.get("_tag") != "RestoreCheckpoint"
            or command.get("commandRef") != request["requestRef"]
            or not isinstance(checkpoint, dict)
            or checkpoint.get("schema") != CHECKPOINT_SCHEMA_VERSION
            or command.get("ownerRef") != checkpoint.get("ownerRef")
            or command.get("tenantRef") != checkpoint.get("tenantRef")
            or command.get("checkpointRef") != checkpoint.get("checkpointRef")
            or command.get("expectedSourceResourceGeneration")
            != checkpoint.get("sourceResourceGeneration")
            or not isinstance(command.get("destinationSandboxRef"), str)
            or not isinstance(command.get("admittedServiceRefs"), list)
            or not isinstance(command.get("sourceCapabilityRefs"), list)
            or not isinstance(runtime, dict)
            or runtime.get("schema") != "openagents.managed_sandbox_phase2_restore_context.v1"
            or runtime.get("ownerRef") != command["ownerRef"]
            or runtime.get("tenantRef") != command["tenantRef"]
            or runtime.get("sandboxRef") != command["destinationSandboxRef"]
            or not isinstance(runtime.get("resourceGeneration"), int)
            or runtime["resourceGeneration"] <= checkpoint["sourceResourceGeneration"]
            or not isinstance(runtime.get("restoredCapabilityRefs"), list)
            or not runtime["restoredCapabilityRefs"]
            or any(
                not isinstance(ref, str) or ref in command["sourceCapabilityRefs"]
                for ref in runtime["restoredCapabilityRefs"]
            )
        ):
            raise DriverError("restore_request_invalid")
        uri = self.object_uri(
            checkpoint["ownerRef"], checkpoint["tenantRef"], checkpoint["checkpointRef"]
        )
        local = Path(tempfile.mkdtemp(prefix="oa-msb-phase2-restore-"))
        local_archive = str(local / "content.tar")
        remote = None
        try:
            content = self.download_object(uri, local_archive)
            if (
                content["contentDigest"] != checkpoint["contentDigest"]
                or content["contentBytes"] != checkpoint["contentBytes"]
            ):
                raise DriverError("checkpoint_object_corrupt")
            remote = self.remote_checkpoint(request["requestRef"])
            self.remote_prepare(command["destinationSandboxRef"], remote)
            self.copy_to_guest(command["destinationSandboxRef"], local_archive, remote["archive"])
            restored = self.remote_restore(
                command["destinationSandboxRef"],
                request["requestRef"],
                checkpoint["contentDigest"],
            )
            remote = restored["remote"]
            result = restored["result"]
            if (
                result.get("formatRef") != FORMAT_REF
                or result.get("contentDigest") != checkpoint["contentDigest"]
                or result.get("contentBytes") != checkpoint["contentBytes"]
                or result.get("repositoryPostImageDigest")
                != checkpoint.get("repositoryPostImageDigest")
            ):
                raise DriverError("guest_restore_scope_conflict")
            restored_generation = self.observe_generation(command["destinationSandboxRef"])
            if restored_generation != runtime["resourceGeneration"]:
                raise DriverError("restore_generation_conflict")
            return {
                "schema": RESTORE_SCHEMA_VERSION,
                "receiptRef": evidence_ref("restore", command["commandRef"]),
                "ownerRef": command["ownerRef"],
                "tenantRef": command["tenantRef"],
                "checkpointRef": command["checkpointRef"],
                "sandboxRef": command["destinationSandboxRef"],
                "checkpointSourceGeneration": checkpoint["sourceResourceGeneration"],
                "restoredResourceGeneration": restored_generation,
                "admittedServiceRefs": command["admittedServiceRefs"],
                "restartedServiceRefs": [],
                "sourceCapabilityRefs": command["sourceCapabilityRefs"],
                "restoredCapabilityRefs": runtime["restoredCapabilityRefs"],
                "grantPolicy": "mint_fresh",
                "processSessionContinuity": "discontinuous",
                "processMemoryRestored": False,
                "ptyRestored": False,
                "socketsRestored": False,
                "outcome": "restored",
                "observedAt": iso_now(),
                "evidenceRefs": [evidence_ref("restore.readback", command["commandRef"])],
            }
        finally:
            if remote is not None:
                self.remote_cleanup(command["destinationSandboxRef"], remote)
            shutil.rmtree(local, ignore_errors=True)

    def fork_from_checkpoint(self, request: dict[str, Any]) -> dict[str, Any]:
        command = request.get("command")
        checkpoint = request.get("checkpoint")
        runtime = request.get("runtimeContext")
        if (
            not isinstance(command, dict)
            or command.get("_tag") != "ForkFromCheckpoint"
            or command.get("commandRef") != request["requestRef"]
            or not isinstance(checkpoint, dict)
            or checkpoint.get("schema") != CHECKPOINT_SCHEMA_VERSION
            or command.get("ownerRef") != checkpoint.get("ownerRef")
            or command.get("tenantRef") != checkpoint.get("tenantRef")
            or command.get("checkpointRef") != checkpoint.get("checkpointRef")
            or command.get("expectedSourceSandboxRef") != checkpoint.get("sourceSandboxRef")
            or command.get("expectedSourceResourceGeneration")
            != checkpoint.get("sourceResourceGeneration")
            or not isinstance(command.get("sourceCapabilityRefs"), list)
            or not isinstance(runtime, dict)
            or runtime.get("schema") != "openagents.managed_sandbox_phase2_fork_context.v1"
            or runtime.get("ownerRef") != command["ownerRef"]
            or runtime.get("tenantRef") != command["tenantRef"]
            or runtime.get("sourceSandboxRef") != checkpoint.get("sourceSandboxRef")
            or runtime.get("sourceResourceGeneration") != checkpoint.get("sourceResourceGeneration")
            or not isinstance(runtime.get("forkSandboxRef"), str)
            or runtime["forkSandboxRef"] == checkpoint.get("sourceSandboxRef")
            or runtime.get("forkResourceGeneration") != 1
            or not isinstance(runtime.get("forkCapabilityRefs"), list)
            or not runtime["forkCapabilityRefs"]
            or any(
                not isinstance(ref, str) or ref in command["sourceCapabilityRefs"]
                for ref in runtime["forkCapabilityRefs"]
            )
            or not isinstance(runtime.get("cleanupObligationRef"), str)
        ):
            raise DriverError("fork_request_invalid")
        uri = self.object_uri(
            checkpoint["ownerRef"], checkpoint["tenantRef"], checkpoint["checkpointRef"]
        )
        local = Path(tempfile.mkdtemp(prefix="oa-msb-phase2-fork-"))
        local_archive = str(local / "content.tar")
        remote = None
        try:
            content = self.download_object(uri, local_archive)
            if (
                content["contentDigest"] != checkpoint["contentDigest"]
                or content["contentBytes"] != checkpoint["contentBytes"]
            ):
                raise DriverError("checkpoint_object_corrupt")
            remote = self.remote_checkpoint(request["requestRef"])
            self.remote_prepare(runtime["forkSandboxRef"], remote)
            self.copy_to_guest(runtime["forkSandboxRef"], local_archive, remote["archive"])
            restored = self.remote_restore(
                runtime["forkSandboxRef"], request["requestRef"], checkpoint["contentDigest"]
            )
            remote = restored["remote"]
            result = restored["result"]
            if (
                result.get("formatRef") != FORMAT_REF
                or result.get("contentDigest") != checkpoint["contentDigest"]
                or result.get("contentBytes") != checkpoint["contentBytes"]
                or result.get("repositoryPostImageDigest")
                != checkpoint.get("repositoryPostImageDigest")
            ):
                raise DriverError("guest_restore_scope_conflict")
            fork_generation = self.observe_generation(runtime["forkSandboxRef"])
            if fork_generation != runtime["forkResourceGeneration"]:
                raise DriverError("fork_generation_conflict")
            return {
                "schema": FORK_SCHEMA_VERSION,
                "receiptRef": evidence_ref("fork", command["commandRef"]),
                "ownerRef": command["ownerRef"],
                "tenantRef": command["tenantRef"],
                "checkpointRef": command["checkpointRef"],
                "sourceSandboxRef": checkpoint["sourceSandboxRef"],
                "sourceResourceGeneration": checkpoint["sourceResourceGeneration"],
                "forkSandboxRef": runtime["forkSandboxRef"],
                "forkResourceGeneration": fork_generation,
                "sourceCapabilityRefs": command["sourceCapabilityRefs"],
                "forkCapabilityRefs": runtime["forkCapabilityRefs"],
                "grantPolicy": "mint_fresh",
                "cleanupObligationRef": runtime["cleanupObligationRef"],
                "stateTransfer": {
                    "credentials": "excluded",
                    "accountSecrets": "excluded",
                    "providerHiddenState": "excluded",
                    "processMemory": "excluded",
                    "processTable": "excluded",
                    "ptyState": "excluded",
                    "sockets": "excluded",
                    "ports": "excluded",
                    "networkIdentity": "excluded",
                },
                "processSessionContinuity": "none",
                "outcome": "created",
                "observedAt": iso_now(),
                "evidenceRefs": [evidence_ref("fork.readback", command["commandRef"])],
            }
        finally:
            if remote is not None:
                self.remote_cleanup(runtime["forkSandboxRef"], remote)
            shutil.rmtree(local, ignore_errors=True)

    def delete_checkpoint(self, request: dict[str, Any]) -> dict[str, Any]:
        command = request.get("command")
        checkpoint = request.get("checkpoint")
        if (
            not isinstance(command, dict)
            or command.get("_tag") != "DeleteCheckpoint"
            or command.get("commandRef") != request["requestRef"]
            or not isinstance(checkpoint, dict)
            or checkpoint.get("schema") != CHECKPOINT_SCHEMA_VERSION
            or command.get("ownerRef") != checkpoint.get("ownerRef")
            or command.get("tenantRef") != checkpoint.get("tenantRef")
            or command.get("checkpointRef") != checkpoint.get("checkpointRef")
        ):
            raise DriverError("delete_request_invalid")
        uri = self.object_uri(
            checkpoint["ownerRef"], checkpoint["tenantRef"], checkpoint["checkpointRef"]
        )
        obj = self.describe_object(uri)
        if obj is None or not isinstance(obj.get("generation"), str):
            raise DriverError("checkpoint_object_missing")
        local = Path(tempfile.mkdtemp(prefix="oa-msb-phase2-delete-"))
        try:
            content = self.download_object(uri, str(local / "content.tar"))
            if (
                content["contentDigest"] != checkpoint["contentDigest"]
                or content["contentBytes"] != checkpoint["contentBytes"]
            ):
                raise DriverError("checkpoint_object_corrupt")
            gcloud_run(
                self.gcloud,
                ["storage", "rm", uri, f"--if-generation-match={obj['generation']}", "--quiet"],
            )
            if self.describe_object(uri) is not None:
                raise DriverError("checkpoint_delete_unverified")
            return {
                "schema": DELETE_SCHEMA_VERSION,
                "receiptRef": evidence_ref("checkpoint.delete", command["commandRef"]),
                "ownerRef": command["ownerRef"],
                "tenantRef": command["tenantRef"],
                "checkpointRef": command["checkpointRef"],
                "sourceSandboxRef": checkpoint.get("sourceSandboxRef"),
                "sourceResourceGeneration": checkpoint.get("sourceResourceGeneration"),
                "contentDigest": checkpoint["contentDigest"],
                "contentDeleted": True,
                "outcome": "deleted",
                "reason": command.get("reason"),
                "deletedAt": iso_now(),
                "evidenceRefs": [evidence_ref("checkpoint.object.delete", checkpoint["checkpointRef"])],
            }
        finally:
            shutil.rmtree(local, ignore_errors=True)

    def observe_resource_generation(self, request: dict[str, Any]) -> dict[str, Any]:
        if (
            not isinstance(request.get("ownerRef"), str)
            or not isinstance(request.get("tenantRef"), str)
            or not isinstance(request.get("sandboxRef"), str)
            or request.get("requestRef") != request.get("sandboxRef")
        ):
            raise DriverError("generation_request_invalid")
        return {
            "ownerRef": request["ownerRef"],
            "tenantRef": request["tenantRef"],
            "sandboxRef": request["sandboxRef"],
            "resourceGeneration": self.observe_generation(request["sandboxRef"]),
            "evidenceRefs": [evidence_ref("sandbox.generation", request["sandboxRef"])],
        }

    def execute(self, request: dict[str, Any]) -> Any:
        action = request["action"]
        if action == "create_checkpoint":
            return self.create_checkpoint(request)
        if action == "verify_checkpoint":
            return self.verify_checkpoint(request)
        if action == "observe_resource_generation":
            return self.observe_resource_generation(request)
        if action == "fork_from_checkpoint":
            return self.fork_from_checkpoint(request)
        if action == "restore_checkpoint":
            return self.restore_checkpoint(request)
        if action == "delete_checkpoint":
            return self.delete_checkpoint(request)
        raise DriverError("phase2_action_not_integrated")


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] != "--managed-sandbox-phase2":
        raise SystemExit(2)
    try:
        driver = Phase2()
        request = read_request()
        result = driver.execute(request)
        sys.stdout.write(
            json.dumps(
                {
                    "schemaVersion": TARGET_SCHEMA_VERSION,
                    "action": request["action"],
                    "requestRef": request["requestRef"],
                    "result": result,
                }
            )
        )
        sys.stdout.write("\n")
    except DriverError as error:
        reason = error.args[0] if error.args else "internal_driver_failure"
        if not re.fullmatch(r"[a-z0-9_]{1,80}", reason):
            reason = "internal_driver_failure"
        sys.stdout.write(json.dumps({"schemaVersion": ERROR_SCHEMA_VERSION, "reasonRef": reason}) + "\n")
        raise SystemExit(2) from error


if __name__ == "__main__":
    main()
