#!/usr/bin/env python3
"""Provision the private key broker; never adopt an unrelated resource."""
import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import time
import uuid

PROJECT = "openagentsgemini"
REGION = "us-central1"
PREFIX = "oa-space-service"
MARKER = "OpenAgents maintained SPACE service (#11814)"
IDENTITY = f"{PREFIX}-keys@{PROJECT}.iam.gserviceaccount.com"
KEY = f"projects/{PROJECT}/locations/{REGION}/keyRings/{PREFIX}/cryptoKeys/intermediate"
LABELS = "openagents-service=space,openagents-issue=11814,openagents-owner=platform"


def cloud(*args, data=None, missing=False):
    result = subprocess.run(
        ["gcloud", *args, "--project", PROJECT, "--quiet", "--format=json"],
        input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if result.returncode:
        error = result.stderr.decode(errors="replace")
        if missing and any(word in error.lower() for word in ("not found", "not_found", "does not exist")):
            return None
        # Commands never place credentials in arguments; secret upload failures
        # still omit stderr to keep provider diagnostics from echoing input.
        if data is None:
            sys.stderr.write(error)
        raise RuntimeError("The GCloud operation failed.")
    return json.loads(result.stdout) if result.stdout.strip() else {}


def named_resource(describe, create, *, label=False, field="description"):
    current = cloud(*describe, missing=True)
    if current is None:
        cloud(*create)
        # IAM can acknowledge a new identity before its describe path sees it.
        for attempt in range(6):
            current = cloud(*describe, missing=True)
            if current is not None:
                break
            time.sleep(2)
        if current is None:
            raise RuntimeError("The new resource is not visible yet. Retry this command.")
    if label:
        owned = current.get("labels", {}).get("openagents-service") == "space"
    else:
        owned = current.get(field) == MARKER
    if not owned:
        raise RuntimeError("A resource with this name belongs to another deployment.")
    return current


def foundation():
    named_resource(
        ["compute", "networks", "describe", PREFIX],
        ["compute", "networks", "create", PREFIX, "--subnet-mode=custom", "--bgp-routing-mode=regional", f"--description={MARKER}"],
    )
    for name, cidr in (("nodes", "10.219.14.0/24"), ("run", "10.219.15.0/24")):
        subnet = named_resource(
            ["compute", "networks", "subnets", "describe", f"{PREFIX}-{name}", f"--region={REGION}"],
            ["compute", "networks", "subnets", "create", f"{PREFIX}-{name}", f"--region={REGION}",
             f"--network={PREFIX}", f"--range={cidr}", "--enable-private-ip-google-access", f"--description={MARKER}"],
        )
        if subnet["ipCidrRange"] != cidr or not subnet["privateIpGoogleAccess"]:
            raise RuntimeError("The private subnet settings differ.")
    named_resource(
        ["compute", "firewall-rules", "describe", f"{PREFIX}-iap"],
        ["compute", "firewall-rules", "create", f"{PREFIX}-iap", f"--network={PREFIX}",
         "--direction=INGRESS", "--action=ALLOW", "--rules=tcp:22", "--source-ranges=35.235.240.0/20",
         f"--target-tags={PREFIX}-operator", f"--description={MARKER}", "--enable-logging"],
    )
    named_resource(
        ["compute", "routers", "describe", PREFIX, f"--region={REGION}"],
        ["compute", "routers", "create", PREFIX, f"--region={REGION}", f"--network={PREFIX}", f"--description={MARKER}"],
    )
    nat = cloud("compute", "routers", "nats", "describe", PREFIX, f"--router={PREFIX}", f"--region={REGION}", missing=True)
    if nat is None:
        cloud("compute", "routers", "nats", "create", PREFIX, f"--router={PREFIX}", f"--region={REGION}",
              "--auto-allocate-nat-external-ips", f"--nat-custom-subnet-ip-ranges={PREFIX}-nodes",
              "--enable-dynamic-port-allocation", "--min-ports-per-vm=32", "--max-ports-per-vm=128",
              "--enable-logging", "--log-filter=ERRORS_ONLY")
    else:
        if nat.get("sourceSubnetworkIpRangesToNat") != "LIST_OF_SUBNETWORKS" or len(nat.get("subnetworks", [])) != 1:
            raise RuntimeError("The NAT allocation differs.")
    named_resource(
        ["iam", "service-accounts", "describe", IDENTITY],
        ["iam", "service-accounts", "create", f"{PREFIX}-keys", f"--display-name={MARKER}", f"--description={MARKER}"],
    )
    cloud("kms", "keyrings", "describe", PREFIX, f"--location={REGION}", missing=True) or cloud(
        "kms", "keyrings", "create", PREFIX, f"--location={REGION}")
    named_resource(
        ["kms", "keys", "describe", "intermediate", f"--keyring={PREFIX}", f"--location={REGION}"],
        ["kms", "keys", "create", "intermediate", f"--keyring={PREFIX}", f"--location={REGION}",
         "--purpose=encryption", "--rotation-period=90d",
         f"--next-rotation-time={(datetime.now(timezone.utc) + timedelta(days=90)).strftime('%Y-%m-%dT%H:%M:%SZ')}", f"--labels={LABELS}"],
        label=True,
    )
    cloud("kms", "keys", "add-iam-policy-binding", "intermediate", f"--keyring={PREFIX}", f"--location={REGION}",
          f"--member=serviceAccount:{IDENTITY}", "--role=roles/cloudkms.cryptoKeyEncrypterDecrypter")


def secret(name, value, *, policy=False):
    fingerprint = hashlib.sha256(value).hexdigest()[:40]
    labels = LABELS + (f",space-policy={fingerprint}" if policy else "")
    current = cloud("secrets", "describe", name, missing=True)
    if current is None:
        current = cloud("secrets", "create", name, "--replication-policy=automatic", f"--labels={labels}", "--data-file=-", data=value)
    if current.get("labels", {}).get("openagents-service") != "space":
        raise RuntimeError("A secret with this name belongs to another deployment.")
    if policy and current["labels"].get("space-policy") != fingerprint:
        raise RuntimeError("The existing policy differs. Review private snapshot references before changing it.")
    cloud("secrets", "add-iam-policy-binding", name, f"--member=serviceAccount:{IDENTITY}", "--role=roles/secretmanager.secretAccessor")


def configure(tenant_file):
    tenants = json.loads(Path(tenant_file).read_text())
    if not isinstance(tenants, list) or not 1 <= len(tenants) <= 128 or any(str(uuid.UUID(t)) != t for t in tenants):
        raise ValueError("Provide a JSON list of canonical organization UUIDs.")
    policy = {"namespace": "urn:openagents:space:org", "tenants": sorted(set(tenants)), "active_key": "v1", "key_routes": {"v1": KEY}}
    name = f"{PREFIX}-key-policy"
    # Retry-safe creation never overwrites a policy or rotates a bearer.
    secret(name, json.dumps(policy, sort_keys=True).encode(), policy=True)
    secret(f"{PREFIX}-key-token", secrets.token_urlsafe(48).encode())


def probe_host():
    """A private, credential-free host that deletes itself after two hours."""
    instance = named_resource(
        ["compute", "instances", "describe", f"{PREFIX}-probe", "--zone=us-central1-a"],
        ["compute", "instances", "create", f"{PREFIX}-probe", "--zone=us-central1-a", "--machine-type=e2-small",
         f"--subnet={PREFIX}-nodes", "--no-address", "--no-service-account", "--no-scopes",
         "--image-project=debian-cloud", "--image=debian-13-trixie-v20261014", "--boot-disk-size=20GB", "--boot-disk-type=pd-balanced",
         "--max-run-duration=7200s", "--instance-termination-action=DELETE", "--no-restart-on-failure",
         f"--tags={PREFIX}-operator", f"--labels={LABELS},openagents-purpose=qualification", f"--description={MARKER}",
         "--metadata=enable-oslogin=TRUE,block-project-ssh-keys=TRUE", "--shielded-secure-boot"],
        label=True,
    )
    if instance.get("serviceAccounts") or any(nic.get("accessConfigs") for nic in instance["networkInterfaces"]):
        raise RuntimeError("The probe host has credentials or public ingress.")
    print(instance["name"])


def deploy(commit, digest, policy_version, token_version):
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
        raise ValueError("Provide the full source commit and immutable image digest.")
    if not all(re.fullmatch(r"[1-9][0-9]*", v) for v in (policy_version, token_version)):
        raise ValueError("Pin numeric secret versions.")
    subprocess.run(["git", "fetch", "-q", "origin", "main"], check=True)
    main = subprocess.check_output(["git", "rev-parse", "origin/main"], text=True).strip()
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, main], check=True)
    current = cloud("run", "services", "describe", f"{PREFIX}-keys", f"--region={REGION}", missing=True)
    if current:
        if current["metadata"].get("labels", {}).get("openagents-service") != "space":
            raise RuntimeError("A service with this name belongs to another deployment.")
        for traffic in current.get("status", {}).get("traffic", []):
            if traffic.get("percent"):
                revision = cloud("run", "revisions", "describe", traffic["revisionName"], f"--region={REGION}")
                previous = revision["metadata"].get("labels", {}).get("openagents-commit")
                if not previous:
                    raise RuntimeError("The running source commit is unavailable.")
                subprocess.run(["git", "merge-base", "--is-ancestor", previous, commit], check=True)
    repo = f"{REGION}-docker.pkg.dev/{PROJECT}/cloud-run-source-deploy/space-key-broker"
    built = cloud("artifacts", "docker", "images", "describe", f"{repo}:{commit}")
    if built.get("image_summary", {}).get("digest") != digest:
        raise RuntimeError("The image digest does not match the source tag.")
    service = cloud("run", "deploy", f"{PREFIX}-keys", f"--region={REGION}", f"--image={repo}@{digest}",
        f"--service-account={IDENTITY}", "--ingress=internal", "--allow-unauthenticated", "--use-http2", "--port=8080",
        "--cpu=1", "--memory=256Mi", "--min-instances=0", "--max-instances=1", "--concurrency=8", "--timeout=30s",
        f"--labels={LABELS},openagents-commit={commit}",
        "--set-env-vars=SPACE_KEY_BROKER_CONFIG=/var/run/space-policy/policy.json,SPACE_KEY_BROKER_TOKEN_FILE=/var/run/space-token/token",
        f"--set-secrets=/var/run/space-policy/policy.json={PREFIX}-key-policy:{policy_version},/var/run/space-token/token={PREFIX}-key-token:{token_version}")
    print(json.dumps({"url": service["status"]["url"], "commit": commit, "image_digest": digest, "ingress": "internal", "max_instances": 1}))


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("foundation")
    commands.add_parser("probe-host")
    config = commands.add_parser("configure")
    config.add_argument("tenants")
    delivery = commands.add_parser("deploy")
    delivery.add_argument("commit")
    delivery.add_argument("digest")
    delivery.add_argument("--policy-version", required=True)
    delivery.add_argument("--token-version", required=True)
    args = parser.parse_args()
    if args.command == "foundation":
        foundation()
    elif args.command == "probe-host":
        probe_host()
    elif args.command == "configure":
        configure(args.tenants)
    else:
        deploy(args.commit, args.digest, args.policy_version, args.token_version)


if __name__ == "__main__":
    main()
