#!/usr/bin/env python3
"""Host-side managed-sandbox turn driver. Dispatches, syncs, and interrupts
guest turns over internal GCE SSH."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def fail() -> None:
    raise SystemExit(2)


def required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        fail()
    return value


def digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def iso_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] != "--managed-sandbox-turn":
        fail()
    try:
        request = json.load(sys.stdin)
    except json.JSONDecodeError:
        fail()

    project = required("OA_MANAGED_SANDBOX_PROJECT_ID")
    zone = required("OA_MANAGED_SANDBOX_ZONE")
    control_ip = required("OA_MANAGED_SANDBOX_CONTROL_INTERNAL_IP")
    broker_port = required("OA_MANAGED_SANDBOX_PROVIDER_BROKER_PORT")
    gcloud = os.environ.get("OA_MANAGED_SANDBOX_GCLOUD_BIN", "").strip() or "gcloud"
    instance = f"oa-msb-{digest(request['sandboxRef'])[:20]}"
    turn_key = digest(request["turnRef"])[:24]
    remote_dir = f"/var/lib/openagents/managed-sandbox-turns/{turn_key}"
    state_path = f"{remote_dir}/state.json"
    request_path = f"{remote_dir}/request.json"
    interrupt_driver = "/opt/openagents-managed-sandbox/managed-sandbox-guest-interrupt.py"

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
            timeout=60,
        )
        if completed.returncode != 0:
            fail()
        return completed.stdout

    def response(events: list[Any], interruption_proof: Any = None) -> dict[str, Any]:
        payload = {
            "schemaVersion": "openagents.managed_sandbox_turn_runtime.v1",
            "turnRef": request["turnRef"],
            "resourceGeneration": request["expectedResourceGeneration"],
            "events": events,
        }
        if interruption_proof is not None:
            payload["interruptionProof"] = interruption_proof
        return payload

    def write_response(events: list[Any], interruption_proof: Any = None) -> None:
        sys.stdout.write(json.dumps(response(events, interruption_proof)))
        raise SystemExit(0)

    def read_state() -> Any:
        try:
            return json.loads(ssh(f"test -f {state_path} && cat {state_path}"))
        except (json.JSONDecodeError, SystemExit):
            return None

    if request.get("action") == "sync":
        state = read_state()
        after = request.get("afterTurnSequence") or 0
        events = [
            event
            for event in (state.get("events") if isinstance(state, dict) else []) or []
            if isinstance(event, dict) and event.get("turnEventSequence", 0) > after
        ]
        write_response(events)

    if request.get("action") == "interrupt":
        state = read_state()
        if not isinstance(state, dict) or not isinstance(state.get("events"), list):
            fail()
        after = request.get("afterTurnSequence") or 0
        observed_at = iso_now()
        requested = {
            "_tag": "RuntimeInterruptRequested",
            "turnRef": request["turnRef"],
            "resourceGeneration": request["expectedResourceGeneration"],
            "turnEventSequence": after + 1,
            "observedAt": observed_at,
            "reasonRef": request.get("reasonRef"),
        }
        interrupted = {
            "_tag": "RuntimeInterrupted",
            "turnRef": request["turnRef"],
            "resourceGeneration": request["expectedResourceGeneration"],
            "turnEventSequence": after + 2,
            "observedAt": iso_now(),
            "reasonRef": request.get("reasonRef"),
        }
        try:
            proof = json.loads(ssh(f"{interrupt_driver} {remote_dir}"))
        except json.JSONDecodeError:
            fail()
        if (
            proof.get("schemaVersion") != "openagents.managed_sandbox_interrupt_proof.v1"
            or proof.get("zeroProcessGroup") is not True
            or proof.get("descendantsRemaining") != 0
        ):
            fail()
        settled_state = read_state()
        if not isinstance(settled_state, dict) or not isinstance(
            settled_state.get("events"), list
        ):
            fail()
        settled_events = settled_state["events"]
        settled_sequence = settled_events[-1].get("turnEventSequence") if settled_events else 0
        if settled_sequence != after:
            fail()
        payload = base64.b64encode(
            json.dumps(
                {
                    **settled_state,
                    "events": [*settled_events, requested, interrupted],
                    "interruptionProof": proof,
                }
            ).encode("utf-8")
        ).decode("ascii")
        ssh(
            f"set -eu; printf %s {payload} | base64 -d > {state_path}.tmp; mv {state_path}.tmp {state_path}"
        )
        write_response([requested, interrupted], proof)

    if request.get("action") != "dispatch" or not isinstance(
        request.get("providerCapabilityToken"), str
    ):
        fail()

    observed_at = iso_now()
    started = {
        "_tag": "RuntimeStarted",
        "turnRef": request["turnRef"],
        "resourceGeneration": request["expectedResourceGeneration"],
        "turnEventSequence": 1,
        "observedAt": observed_at,
    }
    initial_state = {
        "schemaVersion": "openagents.managed_sandbox_guest_turn_state.v1",
        "turnRef": request["turnRef"],
        "resourceGeneration": request["expectedResourceGeneration"],
        "events": [started],
    }
    guest_request = {**request, "providerBaseUrl": f"http://{control_ip}:{broker_port}"}
    local = Path(tempfile.mkdtemp(prefix="oa-msb-turn-"))
    try:
        (local / "request.json").write_text(json.dumps(guest_request), encoding="utf-8")
        (local / "state.json").write_text(json.dumps(initial_state), encoding="utf-8")
        os.chmod(local / "request.json", 0o600)
        os.chmod(local / "state.json", 0o600)
        ssh(f"install -d -m 0700 {remote_dir}")
        copy = subprocess.run(
            [
                gcloud,
                "compute",
                "scp",
                str(local / "request.json"),
                str(local / "state.json"),
                f"openagents@{instance}:{remote_dir}/",
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
        ssh(
            f"set -eu; chmod 0600 {request_path} {state_path}; "
            f"nohup /usr/bin/setsid /usr/bin/python3 "
            f"/opt/openagents-managed-sandbox/managed-sandbox-guest-turn.py "
            f"{request_path} {state_path} >/dev/null 2>&1 & workload=$!; "
            f"echo $workload > {remote_dir}/pid; echo $workload > {remote_dir}/pgid"
        )
        write_response([started])
    finally:
        for child in local.glob("*"):
            child.unlink(missing_ok=True)
        local.rmdir()


if __name__ == "__main__":
    main()
