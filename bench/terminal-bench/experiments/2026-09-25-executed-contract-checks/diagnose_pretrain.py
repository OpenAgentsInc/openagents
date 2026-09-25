#!/usr/bin/env python3
"""Retain the full startup error hidden by the contract report's short tail.

This post-measurement audit changes no frozen call, candidate, or grade.
"""

import argparse
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    inputs = json.loads((args.root / "inputs.json").read_text())
    image = inputs["images"]["pretrain-shard-corruption"]["id"]
    selected = [r for r in inputs["candidates"] if r["task"].endswith("/pretrain-shard-corruption")]
    out = args.root / "pretrain-diagnostic"
    out.mkdir(exist_ok=False)
    for row in [None, *selected]:
        name = row["trial"] if row else "untouched"
        container = docker("create", "--network", "none", "-w", "/app", "--entrypoint", "sleep", image, "infinity")
        command = ["docker", "exec", "-w", "/app", container, "sh", "-c", "bash /app/run_pretrain.sh"]
        started = time.monotonic()
        try:
            docker("start", container)
            if row:
                archive = args.root / "jobs" / row["job"] / row["trial"] / "agent/episode/snapshot/workspace.tar.gz"
                with tempfile.TemporaryDirectory(prefix="contract-9628-diagnostic-") as scratch:
                    with tarfile.open(archive) as bundle:
                        bundle.extractall(scratch, filter="data")
                    owner = docker("exec", container, "sh", "-c", 'echo "$(id -u):$(id -g)"')
                    docker("exec", "-u", "0", container, "sh", "-c", "find /app -mindepth 1 -maxdepth 1 -exec rm -rf {} +")
                    docker("cp", scratch + "/.", container + ":/")
                    docker("exec", "-u", "0", container, "chown", "-R", owner, "/app")
            with (out / (name + ".log")).open("w") as stream:
                result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, timeout=310, check=False)
            receipt = {"trial": name, "image": image, "command": command,
                       "exit": result.returncode, "seconds": time.monotonic() - started}
            (out / (name + ".json")).write_text(json.dumps(receipt, indent=2) + "\n")
            print(name, result.returncode, round(receipt["seconds"], 1), flush=True)
        finally:
            docker("rm", "-f", container)


if __name__ == "__main__":
    main()
