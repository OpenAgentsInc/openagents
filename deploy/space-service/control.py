#!/usr/bin/env python3
"""Provision bounded SPACE control resources, separate from Computers data."""
import argparse
import json
from broker import cloud, named_resource, PROJECT, PREFIX, REGION, MARKER, LABELS

REPOSITORY = f"projects/{PROJECT}/locations/{REGION}/repositories/{PREFIX}"


def images():
    named_resource(
        ["artifacts", "repositories", "describe", PREFIX, f"--location={REGION}"],
        ["artifacts", "repositories", "create", PREFIX, f"--location={REGION}", "--repository-format=docker",
         "--immutable-tags", f"--description={MARKER}", f"--labels={LABELS}"],
        label=True,
    )
    cloud("artifacts", "repositories", "add-iam-policy-binding", PREFIX, f"--location={REGION}",
          f"--member=serviceAccount:oa-mvp-automation@{PROJECT}.iam.gserviceaccount.com", "--role=roles/artifactregistry.writer")
    identity = f"{PREFIX}-control@{PROJECT}.iam.gserviceaccount.com"
    named_resource(
        ["iam", "service-accounts", "describe", identity],
        ["iam", "service-accounts", "create", f"{PREFIX}-control", f"--display-name={MARKER}", f"--description={MARKER}"],
    )
    cloud("artifacts", "repositories", "add-iam-policy-binding", PREFIX, f"--location={REGION}",
          f"--member=serviceAccount:{identity}", "--role=roles/artifactregistry.reader")
    pull_identity = f"{PREFIX}-pull@{PROJECT}.iam.gserviceaccount.com"
    named_resource(
        ["iam", "service-accounts", "describe", pull_identity],
        ["iam", "service-accounts", "create", f"{PREFIX}-pull", f"--display-name={MARKER}", f"--description={MARKER}"],
    )
    cloud("artifacts", "repositories", "add-iam-policy-binding", PREFIX, f"--location={REGION}",
          f"--member=serviceAccount:{pull_identity}", "--role=roles/artifactregistry.reader")
    cloud("iam", "service-accounts", "add-iam-policy-binding", pull_identity,
          f"--member=serviceAccount:{identity}", "--role=roles/iam.serviceAccountTokenCreator")


def database():
    allocation = named_resource(
        ["compute", "addresses", "describe", f"{PREFIX}-sql", "--global"],
        ["compute", "addresses", "create", f"{PREFIX}-sql", "--global", "--purpose=VPC_PEERING",
         "--addresses=10.219.16.0", "--prefix-length=20", f"--network={PREFIX}", f"--description={MARKER}"],
    )
    if allocation.get("address") != "10.219.16.0" or allocation.get("prefixLength") != 20:
        raise RuntimeError("The database address allocation differs.")
    peers = cloud("services", "vpc-peerings", "list", f"--network={PREFIX}", "--service=servicenetworking.googleapis.com")
    if not peers:
        cloud("services", "vpc-peerings", "connect", f"--network={PREFIX}", "--service=servicenetworking.googleapis.com", f"--ranges={PREFIX}-sql")
    elif len(peers) != 1 or peers[0].get("reservedPeeringRanges") != [f"{PREFIX}-sql"]:
        raise RuntimeError("The private service connection differs.")
    instance = cloud("sql", "instances", "describe", PREFIX, missing=True)
    if instance is None:
        operation = cloud("beta", "sql", "instances", "create", PREFIX, f"--region={REGION}", "--database-version=POSTGRES_18",
            "--edition=ENTERPRISE", "--tier=db-custom-1-3840", "--availability-type=zonal", "--storage-type=SSD",
            "--storage-size=20GB", "--storage-auto-increase", "--storage-auto-increase-limit=40", "--no-assign-ip",
            "--ssl-mode=ENCRYPTED_ONLY",
            f"--network=projects/{PROJECT}/global/networks/{PREFIX}", f"--allocated-ip-range-name={PREFIX}-sql",
            "--backup-start-time=04:00", "--enable-point-in-time-recovery", "--retained-backups-count=7",
            "--retained-transaction-log-days=7", "--deletion-protection", f"--labels={LABELS}", "--async")
        print(json.dumps({"database_operation": operation.get("name"), "state": "provisioning"}))
        return
    settings = instance["settings"]
    if settings.get("userLabels", {}).get("openagents-service") != "space":
        raise RuntimeError("A database with this name belongs to another deployment.")
    if (settings["ipConfiguration"].get("ipv4Enabled") or settings["ipConfiguration"].get("sslMode") != "ENCRYPTED_ONLY"
        or settings["tier"] != "db-custom-1-3840" or settings.get("storageAutoResizeLimit") not in (40, "40")):
        raise RuntimeError("The private database capacity settings differ.")
    print(json.dumps({"database": instance["name"], "state": instance["state"], "backups": settings["backupConfiguration"]}))


def nodes():
    named_resource(
        ["compute", "firewall-rules", "describe", f"{PREFIX}-cluster"],
        ["compute", "firewall-rules", "create", f"{PREFIX}-cluster", f"--network={PREFIX}",
         "--direction=INGRESS", "--action=ALLOW", "--rules=tcp:6443,tcp:10250,tcp:9800,udp:51820",
         "--source-ranges=10.219.14.10/31", f"--target-tags={PREFIX}-control,{PREFIX}-worker",
         f"--description={MARKER}", "--enable-logging"],
    )
    identity = f"{PREFIX}-control@{PROJECT}.iam.gserviceaccount.com"
    for role, machine, address, size, zone in (
        ("control", "e2-standard-4", "10.219.14.10", 80, "us-central1-a"),
        ("worker", "n2-standard-8", "10.219.14.11", 300, "us-central1-b"),
    ):
        disk_name = f"{PREFIX}-{role}-data"
        disk = named_resource(
            ["compute", "disks", "describe", disk_name, f"--zone={zone}"],
            ["compute", "disks", "create", disk_name, f"--zone={zone}", "--type=pd-balanced", f"--size={size}GB",
             f"--description={MARKER}", f"--labels={LABELS},openagents-role={role}"],
            label=True,
        )
        if int(disk["sizeGb"]) != size:
            raise RuntimeError("The durable disk capacity differs.")
        args = ["compute", "instances", "create", f"{PREFIX}-{role}", f"--zone={zone}", f"--machine-type={machine}",
            f"--subnet={PREFIX}-nodes", f"--private-network-ip={address}", "--no-address",
            "--image-project=ubuntu-os-cloud", "--image=ubuntu-2404-noble-amd64-v20261016", "--boot-disk-size=40GB", "--boot-disk-type=pd-balanced",
            f"--disk=name={disk_name},device-name=space-data,mode=rw,boot=no,auto-delete=no",
            f"--tags={PREFIX}-operator,{PREFIX}-{role}", f"--labels={LABELS},openagents-role={role}", f"--description={MARKER}",
            "--metadata=enable-oslogin=TRUE,block-project-ssh-keys=TRUE", "--shielded-secure-boot"]
        if role == "worker":
            args += ["--no-service-account", "--no-scopes", "--enable-nested-virtualization", "--min-cpu-platform=Intel Cascade Lake",
                     "--maintenance-policy=TERMINATE"]
        else:
            args += [f"--service-account={identity}", "--scopes=cloud-platform"]
        instance = named_resource(
            ["compute", "instances", "describe", f"{PREFIX}-{role}", f"--zone={zone}"], args, label=True)
        if any(nic.get("accessConfigs") for nic in instance["networkInterfaces"]):
            raise RuntimeError("A node has public ingress.")
        if role == "worker" and instance.get("serviceAccounts"):
            raise RuntimeError("The worker has a Google identity.")
        print(json.dumps({"node": instance["name"], "role": role, "zone": zone, "state": instance["status"], "disk_gib": size}))


def builder():
    """Create one credential-free builder that deletes itself after four hours."""
    named_resource(
        ["compute", "firewall-rules", "describe", f"{PREFIX}-builder-cluster"],
        ["compute", "firewall-rules", "create", f"{PREFIX}-builder-cluster", f"--network={PREFIX}",
         "--direction=INGRESS", "--action=ALLOW", "--rules=tcp:6443,tcp:10250,tcp:9800,udp:51820",
         "--source-ranges=10.219.14.10/31,10.219.14.12/32",
         f"--target-tags={PREFIX}-control,{PREFIX}-worker,{PREFIX}-builder",
         f"--description={MARKER}", "--enable-logging"],
    )
    instance = named_resource(
        ["compute", "instances", "describe", f"{PREFIX}-builder", "--zone=us-central1-b"],
        ["compute", "instances", "create", f"{PREFIX}-builder", "--zone=us-central1-b",
         "--machine-type=n2-standard-8", f"--subnet={PREFIX}-nodes", "--private-network-ip=10.219.14.12",
         "--no-address", "--no-service-account", "--no-scopes", "--enable-nested-virtualization",
         "--min-cpu-platform=Intel Cascade Lake", "--maintenance-policy=TERMINATE",
         "--image-project=ubuntu-os-cloud", "--image=ubuntu-2404-noble-amd64-v20261016",
         "--boot-disk-size=40GB", "--boot-disk-type=pd-balanced",
         "--create-disk=name=oa-space-service-builder-data,size=200GB,type=pd-balanced,device-name=space-data,auto-delete=yes",
         f"--tags={PREFIX}-operator,{PREFIX}-builder", f"--labels={LABELS},openagents-role=builder",
         f"--description={MARKER}", "--metadata=enable-oslogin=TRUE,block-project-ssh-keys=TRUE",
         "--shielded-secure-boot", "--max-run-duration=14400s", "--instance-termination-action=DELETE"], label=True,
    )
    if instance.get("serviceAccounts") or any(nic.get("accessConfigs") for nic in instance["networkInterfaces"]):
        raise RuntimeError("The builder has an unexpected identity or public address.")
    schedule = instance.get("scheduling", {})
    if schedule.get("maxRunDuration", {}).get("seconds") not in (14400, "14400") or schedule.get("instanceTerminationAction") != "DELETE":
        raise RuntimeError("The builder has no four-hour deletion limit.")
    print(json.dumps({"builder": instance["name"], "state": instance["status"], "maximum_hours": 4}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["images", "database", "nodes", "builder"])
    args = parser.parse_args()
    {"images": images, "database": database, "nodes": nodes, "builder": builder}[args.command]()
