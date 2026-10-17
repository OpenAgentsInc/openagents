#!/usr/bin/env python3
"""Apply maintained deployment settings to pinned upstream chart output."""
import argparse
import json
from pathlib import Path
import yaml

PINS = json.loads(Path(__file__).with_name("image-pins.json").read_text())
IMAGE_ROOT = "us-central1-docker.pkg.dev/openagentsgemini/oa-space-service/native/"

LABELS = {"openagents-service": "space", "openagents-issue": "11814"}


def environment(container, name, value):
    entries = container.setdefault("env", [])
    entries[:] = [entry for entry in entries if entry["name"] != name]
    entries.append({"name": name, **value})


def transform(documents):
    for document in documents:
        if not document:
            continue
        metadata = document.setdefault("metadata", {})
        metadata.setdefault("labels", {}).update(LABELS)
        if document["kind"] not in ("Deployment", "DaemonSet", "Job"):
            yield document
            continue
        template = document["spec"]["template"]
        template.setdefault("metadata", {}).setdefault("labels", {})["openagents-native-service"] = "true"
        pod = template["spec"]
        pod["imagePullSecrets"] = [{"name": "space-image-reader"}]
        database = False
        for container in pod.get("containers", []) + pod.get("initContainers", []):
            image = container.get("image", "")
            if image.startswith(IMAGE_ROOT):
                component = image.removeprefix(IMAGE_ROOT).split(":", 1)[0]
                if component not in PINS["native"]:
                    raise ValueError("An unqualified native component was selected.")
                container["image"] = PINS.get("native_overrides", {}).get(component, IMAGE_ROOT + component + "@sha256:" + PINS["native"][component])
                for name, value in {
                    "SPACE_OBJECT_STORE_MULTIPART_PART_SIZE_BYTES": "8388608",
                    "SPACE_OBJECT_STORE_MULTIPART_CONCURRENCY": "2",
                    "SPACE_OBJECT_STORE_DOWNLOAD_READER_CONCURRENCY": "2",
                    "SPACE_OBJECT_STORE_DOWNLOAD_CHUNK_SIZE_BYTES": "8388608",
                    "SPACE_OBJECT_STORE_DOWNLOAD_BUFFER_BUDGET_BYTES": "268435456",
                }.items():
                    environment(container, name, {"value": value})
            if document["kind"] == "Job" and metadata["name"] == "space-db-migrate":
                container["image"] = PINS["migrator"]["image"]
            entries = container.get("env", [])
            for entry in entries:
                if entry["name"] == "SPACE_FIRECRACKER_VENDOR_DIR":
                    if entry.get("value") != "/space/bin/runtime/firecracker":
                        raise ValueError("Review the upstream runtime layout before changing its vendor root.")
                    entry["value"] = "/space/vendor/firecracker"
            if any(entry["name"] == "SPACE_K8S_CONTROL_PLANE" and entry.get("value") != "true" for entry in entries):
                raise ValueError("The maintained controller requires the Kubernetes substrate.")
            connection = next((entry for entry in entries if entry["name"] in ("POSTGRES_CONNECTION_STRING", "DATABASE_URL")), None)
            if connection is None:
                continue
            database = True
            if document["kind"] == "Job" and metadata["name"] == "space-db-migrate":
                connection["valueFrom"]["secretKeyRef"]["name"] = "space-postgres-bootstrap"
                # Cloud SQL cannot mute statement logging per bootstrap session.
                # Create roles and grants here; the SQL Admin API sets passwords.
                container["env"] = [entry for entry in container.get("env", []) if not entry["name"].startswith("SPACE_DB_BOOTSTRAP_")]
                if "bootstrap-roles" in container.get("command", []):
                    container["command"].append("--allow-missing-passwords")
                environment(container, "PGPASSWORD", {"valueFrom": {"secretKeyRef": {"name": "space-postgres-bootstrap", "key": "PGPASSWORD"}}})
            elif any(entry["name"] == "PGPASSWORD" for entry in entries):
                raise ValueError("A runtime service must not receive the bootstrap password.")
            environment(container, "POSTGRES_POOL_MAX_CONNECTIONS", {"value": "4"})
            container.setdefault("volumeMounts", []).append({"name": "oa-postgres-ca", "mountPath": "/var/run/space-postgres-ca", "readOnly": True})
        if database:
            pod.setdefault("volumes", []).append({"name": "oa-postgres-ca", "secret": {"secretName": "space-postgres-ca", "defaultMode": 292}})
        yield document


def metadata_policy():
    # Only native service templates receive this selector. Matching sandbox
    # pods here would broaden their stricter upstream egress policy by union.
    return {"apiVersion": "networking.k8s.io/v1", "kind": "NetworkPolicy",
        "metadata": {"name": "oa-native-no-metadata", "namespace": "space", "labels": LABELS},
        "spec": {"podSelector": {"matchLabels": {"openagents-native-service": "true"}}, "policyTypes": ["Egress"],
                 "egress": [{"to": [{"ipBlock": {"cidr": "0.0.0.0/0", "except": ["169.254.0.0/16"]}}]}]}}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input")
    parser.add_argument("output")
    parser.add_argument("--metadata-policy", action="store_true")
    args = parser.parse_args()
    documents = list(transform(yaml.safe_load_all(Path(args.input).read_text())))
    if args.metadata_policy:
        documents.append(metadata_policy())
    Path(args.output).write_text(yaml.safe_dump_all(documents, sort_keys=False))
