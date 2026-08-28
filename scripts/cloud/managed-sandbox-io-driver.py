#!/usr/bin/env python3
"""Host-side managed-sandbox I/O driver. Copies one request onto the guest and
runs the Python guest I/O executor over internal GCE SSH."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def fail() -> None:
    raise SystemExit(2)


def required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        fail()
    return value


def digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] != "--managed-sandbox-guest-io":
        fail()
    try:
        request = json.load(sys.stdin)
    except json.JSONDecodeError:
        fail()

    project = required("OA_MANAGED_SANDBOX_PROJECT_ID")
    zone = required("OA_MANAGED_SANDBOX_ZONE")
    gcloud = os.environ.get("OA_MANAGED_SANDBOX_GCLOUD_BIN", "").strip() or "gcloud"
    instance = f"oa-msb-{digest(request['sandboxRef'])[:20]}"
    operation_key = digest(request["operationRef"])[:24]
    remote_dir = f"/var/lib/openagents/managed-sandbox-turns/io-{operation_key}"
    request_path = f"{remote_dir}/request.json"
    timeout_ms = int(request.get("timeoutMillis") or 30_000)
    ssh_timeout = (timeout_ms + 90_000) / 1000.0

    def ssh_args(command: str) -> list[str]:
        return [
            gcloud,
            "compute",
            "ssh",
            f"openagents@{instance}",
            "--project",
            project,
            "--zone",
            zone,
            "--internal-ip",
            "--quiet",
            "--ssh-key-expire-after=10m",
            "--ssh-flag=-oStrictHostKeyChecking=no",
            "--ssh-flag=-oUserKnownHostsFile=/dev/null",
            "--command",
            command,
        ]

    def ssh(command: str) -> str:
        completed = subprocess.run(
            ssh_args(command),
            check=False,
            capture_output=True,
            text=True,
            timeout=ssh_timeout,
        )
        if completed.returncode != 0:
            fail()
        return completed.stdout

    local = Path(tempfile.mkdtemp(prefix="oa-msb-io-"))
    try:
        local_request = local / "request.json"
        local_request.write_text(json.dumps(request), encoding="utf-8")
        os.chmod(local_request, 0o600)
        ssh(f"install -d -m 0700 {remote_dir}")
        copy = subprocess.run(
            [
                gcloud,
                "compute",
                "scp",
                str(local_request),
                f"openagents@{instance}:{request_path}",
                "--project",
                project,
                "--zone",
                zone,
                "--internal-ip",
                "--quiet",
                "--ssh-key-expire-after=10m",
                "--scp-flag=-oStrictHostKeyChecking=no",
                "--scp-flag=-oUserKnownHostsFile=/dev/null",
            ],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=60,
        )
        if copy.returncode != 0:
            fail()
        sys.stdout.write(
            ssh(
                f"set -eu; trap 'rmdir {remote_dir} 2>/dev/null || true' EXIT; "
                f"chmod 0600 {request_path}; "
                f"/usr/bin/python3 /opt/openagents-managed-sandbox/managed-sandbox-guest-io.py {request_path}"
            )
        )
    except (OSError, subprocess.TimeoutExpired, KeyError):
        fail()
    finally:
        for child in local.glob("*"):
            child.unlink(missing_ok=True)
        local.rmdir()


if __name__ == "__main__":
    main()
