#!/usr/bin/env python3
"""Initialize owned nodes without placing join credentials in logs or arguments."""
import argparse
from pathlib import Path
import subprocess
from broker import PROJECT, PREFIX, cloud


def zone_for(role):
    if role != "builder":
        return {"control": "us-central1-a", "worker": "us-central1-b"}[role]
    nodes = cloud("compute", "instances", "list", f"--filter=name={PREFIX}-builder")
    if len(nodes) != 1 or nodes[0].get("labels", {}).get("openagents-issue") != "11814":
        raise RuntimeError("Choose the single owned builder before initializing it.")
    return nodes[0]["zone"].rsplit("/", 1)[-1]


def ssh(role, command, data=None):
    zone = zone_for(role)
    result = subprocess.run(["gcloud", "compute", "ssh", f"{PREFIX}-{role}", f"--project={PROJECT}", f"--zone={zone}",
        "--tunnel-through-iap", "--quiet", "--ssh-flag=-T", "--command=" + command],
        input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if result.returncode:
        # Secret transfer output is never included in diagnostics.
        raise RuntimeError(f"The {role} initialization command failed. Check its private setup log.")
    return result.stdout


def initialize(role):
    script = Path(__file__).with_name("node-init.sh")
    subprocess.run(["git", "diff", "--quiet", "HEAD", "--", str(script)], check=True)
    subprocess.run(["git", "fetch", "-q", "origin", "main"], check=True)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, "origin/main"], check=True)
    zone = zone_for(role)
    instance = cloud("compute", "instances", "describe", f"{PREFIX}-{role}", f"--zone={zone}")
    if instance.get("labels", {}).get("openagents-issue") != "11814":
        raise RuntimeError("This node belongs to another deployment.")
    previous = instance["labels"].get("openagents-commit")
    if previous:
        subprocess.run(["git", "merge-base", "--is-ancestor", previous, commit], check=True)
    subprocess.run(["gcloud", "compute", "scp", str(script), f"{PREFIX}-{role}:~/node-init-11814.sh",
        f"--project={PROJECT}", f"--zone={zone}", "--tunnel-through-iap", "--quiet"], check=True)
    if role != "control":
        token = ssh("control", "sudo cat /space/k3s/server/node-token")
        if not token.startswith(b"K10") or not 50 < len(token) < 1024:
            raise RuntimeError("The cluster join credential is unavailable.")
        ssh(role, "sudo install -d -m 0700 /run/oa-space && sudo sh -c 'umask 077; cat >/run/oa-space/k3s-join'", token)
        del token
    output = ssh(role, "sudo bash ~/node-init-11814.sh " + role + " > ~/node-init-11814.log 2>&1 && sha256sum ~/node-init-11814.sh")
    cloud("compute", "instances", "add-labels", f"{PREFIX}-{role}", f"--zone={zone}", f"--labels=openagents-commit={commit}")
    print(f"{role} initialized from {commit}: " + output.decode().strip())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("role", choices=["control", "worker", "builder"])
    initialize(parser.parse_args().role)
