#!/usr/bin/env python3
"""Create private SPACE buckets and a bucket-scoped interoperability identity."""
import json
from contextlib import contextmanager
import http.client
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
from broker import cloud, named_resource, PROJECT, PREFIX, REGION, MARKER

IDENTITY = f"{PREFIX}-storage@{PROJECT}.iam.gserviceaccount.com"
FILTER = json.loads(Path(__file__).with_name("bucket-ip-filter.json").read_text())
LABELS = {"openagents-service": "space", "openagents-issue": "11814", "openagents-owner": "platform"}
PRIVATE_PORT = None


@contextmanager
def private_tunnel():
    # Google authentication and TLS stay on the operator's machine. The VM
    # forwards encrypted TCP only; it receives no operator credential.
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    command = ["gcloud", "compute", "ssh", f"{PREFIX}-control", f"--project={PROJECT}", "--zone=us-central1-a",
               "--tunnel-through-iap", "--quiet", "--ssh-flag=-N", "--ssh-flag=-T",
               "--ssh-flag=-oExitOnForwardFailure=yes", f"--ssh-flag=-L127.0.0.1:{port}:storage.googleapis.com:443"]
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError("The private storage tunnel stopped.")
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                    break
            except OSError:
                time.sleep(0.2)
        else:
            raise RuntimeError("The private storage tunnel did not start.")
        yield port
    finally:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        process.stderr.close()


def api(method, path, body=None, *, missing=False):
    token = subprocess.check_output(["gcloud", "auth", "print-access-token"], stderr=subprocess.DEVNULL).decode().strip()
    if PRIVATE_PORT is None:
        raise RuntimeError("Use the private storage connection.")
    connection = http.client.HTTPSConnection("storage.googleapis.com", timeout=30)
    connection._create_connection = lambda address, timeout, *args: socket.create_connection(("127.0.0.1", PRIVATE_PORT), timeout)
    try:
        connection.request(method, "/storage/v1/" + path,
            body=json.dumps(body).encode() if body is not None else None,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        response = connection.getresponse()
        if missing and response.status == 404:
            return None
        if not 200 <= response.status < 300:
            raise RuntimeError(f"Storage configuration failed with HTTP {response.status}.")
        data = response.read(1048577)
        if len(data) > 1048576:
            raise RuntimeError("The storage response exceeded its size limit.")
        return json.loads(data) if data else {}
    finally:
        connection.close()


def main():
    named_resource(
        ["iam", "service-accounts", "describe", IDENTITY],
        ["iam", "service-accounts", "create", f"{PREFIX}-storage", f"--display-name={MARKER}", f"--description={MARKER}"],
    )
    for purpose in ("templates", "snapshots"):
        bucket = f"{PROJECT}-{PREFIX}-{purpose}"
        current = api("GET", f"b/{bucket}", missing=True)
        if current is None:
            current = api("POST", f"b?project={PROJECT}", {
                "name": bucket, "location": REGION, "storageClass": "STANDARD", "labels": LABELS,
                "iamConfiguration": {"uniformBucketLevelAccess": {"enabled": True}, "publicAccessPrevention": "enforced"},
                # New IP-filter permissions may be bucket-scoped. There are no
                # service grants or objects until the filter is verified.
                "softDeletePolicy": {"retentionDurationSeconds": "604800"},
            })
        if any(current.get("labels", {}).get(key) != value for key, value in LABELS.items()):
            raise RuntimeError("A bucket with this name belongs to another deployment.")
        if not current.get("ipFilter"):
            current = api("PATCH", f"b/{bucket}", {"ipFilter": FILTER})
        access = current["iamConfiguration"]
        observed_filter = current.get("ipFilter", {})
        if (not access["uniformBucketLevelAccess"]["enabled"] or access["publicAccessPrevention"] != "enforced"
            or observed_filter.get("mode") != "Enabled" or observed_filter.get("publicNetworkSource")
            or observed_filter.get("allowAllServiceAgentAccess", False)
            or observed_filter.get("allowCrossOrgVpcs", False)
            or observed_filter.get("vpcNetworkSources") != FILTER["vpcNetworkSources"]):
            raise RuntimeError("The private bucket access settings differ.")
        policy = api("GET", f"b/{bucket}/iam?optionsRequestedPolicyVersion=3")
        member = f"serviceAccount:{IDENTITY}"
        grant = next((binding for binding in policy.get("bindings", [])
                      if binding["role"] == "roles/storage.objectUser" and not binding.get("condition")), None)
        if grant is None:
            grant = {"role": "roles/storage.objectUser", "members": []}
            policy.setdefault("bindings", []).append(grant)
        if member not in grant["members"]:
            grant["members"].append(member)
            api("PUT", f"b/{bucket}/iam", policy)
        print(json.dumps({"bucket": bucket, "private_network_only": True, "soft_delete_days": 7}))
    name = f"{PREFIX}-object-credentials"
    existing = cloud("secrets", "describe", name, missing=True)
    if existing is not None:
        if any(existing.get("labels", {}).get(key) != value for key, value in LABELS.items()):
            raise RuntimeError("The object credential belongs to another deployment.")
    else:
        base = f"projects/{PROJECT}/hmacKeys"
        keys = api("GET", base + "?serviceAccountEmail=" + IDENTITY)
        if any(key.get("state") != "DELETED" for key in keys.get("items", [])):
            raise RuntimeError("The storage identity has an unpublished key. Resolve it before creating another.")
        key = api("POST", base + "?serviceAccountEmail=" + IDENTITY)
        metadata = key["metadata"]
        value = {"access_key_id": metadata["accessId"], "secret_access_key": key["secret"]}
        try:
            cloud("secrets", "create", name, "--replication-policy=automatic", "--data-file=-",
                  "--labels=" + ",".join(f"{key}={value}" for key, value in LABELS.items()), data=json.dumps(value).encode())
        except Exception:
            # This new key was never published to a worker. Disable and remove
            # it rather than leaving a credential whose secret cannot be read.
            metadata["state"] = "INACTIVE"
            api("PUT", base + "/" + metadata["accessId"], metadata)
            api("DELETE", base + "/" + metadata["accessId"])
            raise
        del value, key
    cloud("secrets", "add-iam-policy-binding", name,
          f"--member=serviceAccount:{PREFIX}-control@{PROJECT}.iam.gserviceaccount.com", "--role=roles/secretmanager.secretAccessor")
    print("Bucket-scoped storage credentials are in Secret Manager; no worker has a Google identity.")


if __name__ == "__main__":
    with private_tunnel() as PRIVATE_PORT:
        main()
