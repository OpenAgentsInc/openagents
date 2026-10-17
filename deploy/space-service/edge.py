#!/usr/bin/env python3
"""Deploy the owned private TLS and streaming frontends."""
import argparse
import json
from pathlib import Path
import re
import subprocess
from broker import PROJECT, PREFIX, REGION, MARKER, LABELS, cloud, named_resource


def manifest(image):
    if not re.fullmatch(r"us-central1-docker.pkg.dev/openagentsgemini/oa-space-service/edge@sha256:[0-9a-f]{64}", image):
        raise RuntimeError("Provide the verified owned edge image digest.")
    labels = {"openagents-service": "space", "openagents-issue": "11814", "app": "oa-space-edge", "openagents-native-service": "true"}
    config = Path(__file__).with_name("control-edge.conf").read_text()
    print(json.dumps({"apiVersion": "v1", "kind": "List", "items": [
        {"apiVersion": "v1", "kind": "ConfigMap", "metadata": {"name": "oa-space-edge", "namespace": "space", "labels": labels}, "data": {"default.conf": config}},
        {"apiVersion": "apps/v1", "kind": "Deployment", "metadata": {"name": "oa-space-edge", "namespace": "space", "labels": labels},
         "spec": {"replicas": 1, "strategy": {"type": "Recreate"}, "selector": {"matchLabels": {"app": "oa-space-edge"}},
          "template": {"metadata": {"labels": labels}, "spec": {
            "nodeSelector": {"space.pplx.net/infra": "true"}, "priorityClassName": "space-control-plane",
            "automountServiceAccountToken": False, "imagePullSecrets": [{"name": "space-image-reader"}],
            "containers": [{"name": "edge", "image": image, "command": ["nginx", "-g", "daemon off;"],
                "ports": [{"name": "api", "containerPort": 8443, "hostPort": 8443}, {"name": "data", "containerPort": 8444, "hostPort": 8444}],
                "resources": {"requests": {"cpu": "50m", "memory": "64Mi"}, "limits": {"cpu": "250m", "memory": "128Mi"}},
                "securityContext": {"allowPrivilegeEscalation": False, "capabilities": {"drop": ["ALL"], "add": ["CHOWN", "DAC_OVERRIDE", "SETUID", "SETGID"]}},
                "readinessProbe": {"tcpSocket": {"port": "api"}, "periodSeconds": 5},
                "volumeMounts": [{"name": "config", "mountPath": "/etc/nginx/conf.d", "readOnly": True}, {"name": "tls", "mountPath": "/etc/space-edge", "readOnly": True}]}],
            "volumes": [{"name": "config", "configMap": {"name": "oa-space-edge"}}, {"name": "tls", "secret": {"secretName": "space-management-edge-tls", "defaultMode": 256}}]
          }}}}
    ]}))


def deploy(commit, digest):
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
        raise RuntimeError("Provide full source and image identities.")
    subprocess.run(["git", "fetch", "-q", "origin", "main"], check=True)
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, "origin/main"], check=True)
    tagged = f"us-central1-docker.pkg.dev/{PROJECT}/{PREFIX}/edge:{commit}"
    current = cloud("artifacts", "docker", "images", "describe", tagged)
    if current["image_summary"]["digest"] != digest:
        raise RuntimeError("The published edge digest differs.")
    identity = f"{PREFIX}-edge@{PROJECT}.iam.gserviceaccount.com"
    named_resource(["iam", "service-accounts", "describe", identity],
        ["iam", "service-accounts", "create", f"{PREFIX}-edge", f"--display-name={MARKER}", f"--description={MARKER}"])
    named_resource(["compute", "firewall-rules", "describe", f"{PREFIX}-run-edge"],
        ["compute", "firewall-rules", "create", f"{PREFIX}-run-edge", f"--network={PREFIX}", "--direction=INGRESS", "--action=ALLOW",
         "--rules=tcp:8443,tcp:8444", "--source-ranges=10.219.15.0/24", f"--target-tags={PREFIX}-control", f"--description={MARKER}", "--enable-logging"])
    for role, port in (("api", 8443), ("data", 8444)):
        name = f"{PREFIX}-{role}"
        previous = cloud("run", "services", "describe", name, f"--region={REGION}", missing=True)
        if previous:
            if previous["metadata"].get("labels", {}).get("openagents-issue") != "11814":
                raise RuntimeError("An edge service belongs to another deployment.")
            old_commit = previous["metadata"]["labels"].get("openagents-commit")
            if old_commit:
                subprocess.run(["git", "merge-base", "--is-ancestor", old_commit, commit], check=True)
        result = cloud("run", "deploy", name, f"--region={REGION}",
            f"--image=us-central1-docker.pkg.dev/{PROJECT}/{PREFIX}/edge@{digest}", f"--service-account={identity}",
            "--execution-environment=gen2", "--ingress=internal", "--allow-unauthenticated", "--port=8080",
            "--cpu=1", "--memory=512Mi", "--min-instances=0", "--max-instances=1", "--concurrency=16", "--timeout=3600",
            f"--network={PREFIX}", f"--subnet={PREFIX}-run", "--vpc-egress=private-ranges-only",
            f"--set-env-vars=SPACE_EDGE_PORT={port}", f"--labels={LABELS},openagents-commit={commit}")
        print(json.dumps({"service": name, "url": result["status"]["url"], "image": digest, "source": commit}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="command", required=True)
    render = subs.add_parser("manifest"); render.add_argument("image")
    run = subs.add_parser("deploy"); run.add_argument("commit"); run.add_argument("digest")
    args = parser.parse_args()
    if args.command == "manifest": manifest(args.image)
    else: deploy(args.commit, args.digest)
