#!/usr/bin/env python3
"""Replay evidence.data_profile and entry-point discovery on task images.

For each task in the local Terminal-Bench task cache with a local
environment image (see protocol.md), this script:

1. copies the image's working directory out of a created, never-started
   container (`docker cp`), skipping one over 2 GiB;
2. runs `coder-one component run evidence.data_profile` on the copy;
3. runs `coder-one component run evidence.baseline` with
   `discover_only`, once with named discovery and once with wide
   discovery;
4. runs every entry point either finds, once each, in a fresh container of
   the image with the network off, bounded to 60 seconds and 16 KiB per
   stream.

Everything it writes goes under ~/.openagents/coder-one/data-profile-offline,
never into the repository: the workspaces and outputs are benchmark
content. measure.py reads results.json and writes the counts the report
publishes.

Usage: replay.py [--bin PATH] [--tasks DIR] [--only TASK ...] [--keep]
                 [--reconstructed]

--reconstructed runs only a supplementary pass, decided after the image
replay and before any label was read: each source or anatomy task with no
local image gets its workspace from the departures experiment's
build_workspaces.py map (files a Dockerfile generates at build time are
missing), and is profiled and searched for entry points, with no runs.

Containers are named dp9654-* and removed after use. --keep keeps the
copied workspaces of the source and anatomy tasks for labeling; every other
copy, and every copy by default, is removed after its task.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time

HOME = os.path.expanduser("~")
TASKS = os.path.join(HOME, ".openagents/terminal-bench/upstream/terminal-bench/tasks")
OUT = os.path.join(HOME, ".openagents/coder-one/data-profile-offline")
REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../.."))
MAX_BYTES = 2 * 1024 ** 3
WALL = 60
STREAM = 16 * 1024

SOURCE = ["embedding-drift-monitor"]
ANATOMY = [
    "session-window-debug", "bun-sourcemap-leak", "atrx-vep-crispr", "data-anonymization",
    "vba-userform-port", "biped-contact-dynamics", "intrastat-meldung",
    "layout-config-recreation", "vf2-speedup-networkx", "ks-solver-cpp", "shadow-relay",
    "interleaved-vigenere", "sound-change-cascade", "fin-saccr-rwa", "gsea-proteomics",
    "coq-block-bound", "html-js-filter",
]
FAMILY = [
    "cumulative-layout-shift", "live-database-cutover", "mp-checkpoint-consolidation",
    "payments-pipeline-fix", "photonic-waveguide-routing", "telecom-entity-resolution",
]


def set_of(task):
    if task in SOURCE:
        return "source"
    if task in ANATOMY:
        return "anatomy"
    if task in FAMILY:
        return "family"
    return "wider"


def sh(args, timeout=None):
    return subprocess.run(args, capture_output=True, text=True, timeout=timeout)


def image_for(task, images):
    for want in (f"accept-env/{task}:latest",):
        if want in images:
            return want
    for prefix in (f"tbench-warm/{task}:environment-",):
        for image in images:
            if image.startswith(prefix):
                return image
    if f"truth9584-review/{task}:public-environment" in images:
        return f"truth9584-review/{task}:public-environment"
    for image in images:
        if image.startswith(f"{task}__") and image.endswith("__env-main:latest"):
            return image
    return None


def component(binary, name, directory, payload):
    os.makedirs(directory, exist_ok=True)
    fixture = {
        "schema": "openagents.coder-one.component-fixture.v1",
        "component": name,
        "source": {"kind": "task-image", "issue": 9654},
        "input": payload,
        "retained": None,
    }
    with open(os.path.join(directory, f"{name}.json"), "w") as handle:
        json.dump(fixture, handle, indent=2)
    ran = sh([binary, "component", "run", name, "--fixture", directory, "--json", "--no-record"])
    if ran.returncode != 0:
        ran = sh([binary, "component", "run", name, "--fixture", directory, "--json"])
    return json.loads(ran.stdout)["result"]["fixtures"][0]


def run_in_image(image, workdir, command, name):
    started = time.time()
    args = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "-w", workdir,
        "--entrypoint", "/bin/sh", image, "-c", command,
    ]
    try:
        ran = subprocess.run(args, capture_output=True, timeout=WALL)
        timed_out = False
        exit_code = ran.returncode
        stdout, stderr = ran.stdout, ran.stderr
    except subprocess.TimeoutExpired as expired:
        sh(["docker", "kill", name], timeout=30)
        sh(["docker", "rm", "-f", name], timeout=30)
        timed_out = True
        exit_code = None
        stdout, stderr = expired.stdout or b"", expired.stderr or b""
    return {
        "command": command,
        "exit": exit_code,
        "timed_out": timed_out,
        "ms": int((time.time() - started) * 1000),
        "stdout_bytes": len(stdout),
        "stderr_bytes": len(stderr),
        "stdout_head": stdout[:STREAM].decode("utf-8", "replace"),
        "stderr_head": stderr[:STREAM].decode("utf-8", "replace"),
    }


def copy_workspace(image, workdir, dest, task):
    name = f"dp9654-cp-{task}"
    sh(["docker", "rm", "-f", name])
    size = sh(["docker", "run", "--rm", "--name", f"dp9654-du-{task}", "--network", "none",
               "--entrypoint", "/bin/sh", image, "-c", f"du -sb {workdir} 2>/dev/null | cut -f1"],
              timeout=120)
    try:
        if int(size.stdout.strip().splitlines()[-1]) > MAX_BYTES:
            return f"the working directory holds more than {MAX_BYTES} bytes"
    except (ValueError, IndexError):
        pass
    # An image with no command needs one to be created; it never starts.
    created = sh(["docker", "create", "--name", name, "--entrypoint", "/bin/sh", image])
    if created.returncode != 0:
        return f"docker create failed: {created.stderr.strip()[:200]}"
    try:
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        # A tar stream, extracted with modes this user can read: the image's
        # files may belong to root and be unreadable to anyone else.
        with tempfile.TemporaryFile() as archive:
            copied = subprocess.run(
                ["docker", "cp", f"{name}:{workdir.rstrip('/')}/.", "-"],
                stdout=archive, stderr=subprocess.PIPE, timeout=600)
            if copied.returncode != 0:
                return f"docker cp failed: {copied.stderr.decode()[:200]}"
            archive.seek(0)
            with tarfile.open(fileobj=archive) as tar:
                members = []
                for member in tar.getmembers():
                    if not (member.isfile() or member.isdir()):
                        continue
                    member.mode = 0o755 if member.isdir() else 0o644
                    member.uid = member.gid = os.getuid()
                    members.append(member)
                # Entries are named ./<path>, relative to the working directory.
                tar.extractall(dest, members=members, filter="data")
    finally:
        sh(["docker", "rm", "-f", name])
    return None


def replay(task, image, binary, out):
    record = {"task": task, "set": set_of(task), "image": image}
    inspect = json.loads(sh(["docker", "inspect", image]).stdout)[0]
    workdir = inspect["Config"].get("WorkingDir") or "/"
    record["workdir"] = workdir
    if workdir == "/":
        record["skipped"] = "the image's working directory is /"
        return record
    dest = os.path.join(out, "workspaces", task)
    started = time.time()
    why = copy_workspace(image, workdir, dest, task)
    record["copy_ms"] = int((time.time() - started) * 1000)
    if why:
        record["skipped"] = why
        shutil.rmtree(dest, ignore_errors=True)
        return record
    with open(os.path.join(TASKS, task, "instruction.md")) as handle:
        instruction = handle.read()
    fixtures = os.path.join(out, "fixtures", task)
    record["profile"] = component(binary, "evidence.data_profile",
                                  os.path.join(fixtures, "profile"), {"workspace": dest})
    for mode in ("named", "wide"):
        record[f"discover_{mode}"] = component(
            binary, "evidence.baseline", os.path.join(fixtures, f"discover-{mode}"),
            {"task": instruction, "workspace": dest, "workdir": workdir,
             "wide": mode == "wide", "discover_only": True})
    commands = []
    for mode in ("named", "wide"):
        for entry in record[f"discover_{mode}"]["output"]["entries"]:
            if entry.get("refused") is None and entry["command"] not in commands:
                commands.append(entry["command"])
    record["runs"] = []
    for i, command in enumerate(commands):
        ran = run_in_image(image, workdir, command, f"dp9654-{task}-{i}")
        if ran["exit"] == 127 and command.startswith("python "):
            again = run_in_image(image, workdir, "python3 " + command[len("python "):],
                                 f"dp9654-{task}-{i}r")
            again["stated"] = command
            ran = again
        ran["entry"] = command
        record["runs"].append(ran)
        print(f"  {task}: {command[:90]} -> exit {ran['exit']} "
              f"{'(timed out)' if ran['timed_out'] else ''} in {ran['ms']} ms", flush=True)
    return record


def reconstructed(task, binary, out, tasks_dir):
    """The supplementary pass: a rebuilt workspace, profile and discovery only."""
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    "../2026-09-25-departures"))
    import build_workspaces  # noqa: PLC0415 - the departures experiment's map
    pairs = build_workspaces.WORKSPACES[task]
    workdir = {"vba-userform-port": "/workspace", "intrastat-meldung": "/workspace"}.get(task, "/app")
    dest = os.path.join(out, "workspaces", task)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    env = os.path.join(tasks_dir, task, "environment")
    for src, dst in pairs:
        build_workspaces.copy(os.path.join(env, src), os.path.normpath(os.path.join(dest, dst)))
    with open(os.path.join(tasks_dir, task, "instruction.md")) as handle:
        instruction = handle.read()
    fixtures = os.path.join(out, "fixtures", task)
    record = {"task": task, "set": set_of(task), "image": None, "workdir": workdir,
              "reconstructed": True, "runs": []}
    record["profile"] = component(binary, "evidence.data_profile",
                                  os.path.join(fixtures, "profile"), {"workspace": dest})
    for mode in ("named", "wide"):
        record[f"discover_{mode}"] = component(
            binary, "evidence.baseline", os.path.join(fixtures, f"discover-{mode}"),
            {"task": instruction, "workspace": dest, "workdir": workdir,
             "wide": mode == "wide", "discover_only": True})
    return record


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", default=os.path.join(REPO, "target/debug/coder-one"))
    parser.add_argument("--tasks", default=TASKS)
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--keep", action="store_true")
    parser.add_argument("--reconstructed", action="store_true")
    args = parser.parse_args()
    os.makedirs(OUT, exist_ok=True)
    images = sh(["docker", "images", "--format", "{{.Repository}}:{{.Tag}}"]).stdout.split()
    tasks = sorted(t for t in os.listdir(args.tasks)
                   if os.path.isfile(os.path.join(args.tasks, t, "instruction.md")))
    if args.only:
        tasks = [t for t in tasks if t in args.only]
    results_file = os.path.join(OUT, "results.json")
    results = {}
    if os.path.exists(results_file):
        results = json.load(open(results_file))
    for task in tasks:
        image = image_for(task, images)
        if args.reconstructed:
            if image is None and set_of(task) in ("source", "anatomy"):
                print(f"{task}: reconstructed", flush=True)
                results[task] = reconstructed(task, args.bin, OUT, args.tasks)
                with open(results_file, "w") as handle:
                    json.dump(results, handle, indent=1)
            continue
        if image is None:
            results[task] = {"task": task, "set": set_of(task), "skipped": "no local image"}
            print(f"{task}: no local image", flush=True)
            continue
        print(f"{task}: {image}", flush=True)
        try:
            results[task] = replay(task, image, args.bin, OUT)
        except Exception as error:  # noqa: BLE001 - one task's failure is its record
            results[task] = {"task": task, "set": set_of(task), "image": image,
                             "skipped": f"replay failed: {error}"}
        with open(results_file, "w") as handle:
            json.dump(results, handle, indent=1)
        if not (args.keep and set_of(task) in ("source", "anatomy")):
            shutil.rmtree(os.path.join(OUT, "workspaces", task), ignore_errors=True)


if __name__ == "__main__":
    main()
