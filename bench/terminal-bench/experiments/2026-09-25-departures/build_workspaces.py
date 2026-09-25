#!/usr/bin/env python3
"""Rebuild the untouched workspaces of the task anatomy's 18 tasks.

Each workspace is what the task's Dockerfile puts in the agent's working
directory, copied from the Terminal-Bench task cache. Files a Dockerfile
generates at build time aren't reconstructed; see WORKSPACES below. The
script also writes one `evidence.departures` fixture per task, whose input
is the task's instruction and the workspace path.

Usage: build_workspaces.py [--tasks DIR] [--out DIR]

The defaults read ~/.openagents/terminal-bench/upstream/terminal-bench/tasks
and write ~/.openagents/coder-one/departures-offline. Nothing it writes
goes into the repository: the workspaces are benchmark content.
"""

import argparse
import json
import os
import shutil

HOME = os.path.expanduser("~")
TASKS = os.path.join(HOME, ".openagents/terminal-bench/upstream/terminal-bench/tasks")
OUT = os.path.join(HOME, ".openagents/coder-one/departures-offline")

# Task -> (source under environment/, destination under the workspace).
# An empty list is an empty working directory. The comment says why.
WORKSPACES = {
    # COPY app/ app/ into WORKDIR /app.
    "session-window-debug": [("app", "app")],
    # COPY package.json tsconfig.json visibility.json scripts/ src/.
    "bun-sourcemap-leak": [
        ("package.json", "package.json"),
        ("tsconfig.json", "tsconfig.json"),
        ("visibility.json", "visibility.json"),
        ("scripts", "scripts"),
        ("src", "src"),
    ],
    # COPY data /app/data; the VEP archive is removed and the cache is
    # unpacked at build time, which isn't reconstructed.
    "atrx-vep-crispr": [
        ("data/CDS-information.txt", "data/CDS-information.txt"),
        ("data/vep_plugins", "data/vep_plugins"),
    ],
    # The builder stage generates input/; only policy.yaml is copied as is.
    "data-anonymization": [("data/policy.yaml", "policy.yaml")],
    # WORKDIR /workspace is empty; the legacy app is under /shared.
    "vba-userform-port": [],
    # COPY data/ /app/.
    "biped-contact-dynamics": [("data", ".")],
    # The agent image's WORKDIR /workspace is empty; SOPs are under /shared.
    "intrastat-meldung": [],
    # COPY render.py and data into /app.
    "layout-config-recreation": [("render.py", "render.py"), ("data", "data")],
    # WORKDIR /app is empty; the task installs NetworkX itself.
    "vf2-speedup-networkx": [],
    # COPY oracle.hpp /app/oracle.hpp.
    "ks-solver-cpp": [("oracle.hpp", "oracle.hpp")],
    # COPY . /app.
    "embedding-drift-monitor": [("drift_monitor", "drift_monitor"), ("data", "data")],
    # setup_challenge.py generates data files and is removed; not rebuilt.
    "shadow-relay": [],
    # Two generators write data files and are removed; not rebuilt.
    "interleaved-vigenere": [],
    # COPY engine/ and data/train.tsv.
    "sound-change-cascade": [("engine", "engine"), ("data/train.tsv", "data/train.tsv")],
    # COPY data/ /app/inputs/.
    "fin-saccr-rwa": [("data", "inputs")],
    # COPY data /app/data: a GSEA archive and a spreadsheet, no source.
    "gsea-proteomics": [],
    # COPY Main.v and _CoqProject.
    "coq-block-bound": [("Main.v", "Main.v"), ("_CoqProject", "_CoqProject")],
    # WORKDIR /app is empty.
    "html-js-filter": [],
}


def copy(src, dst):
    if os.path.isdir(src):
        shutil.copytree(src, dst, dirs_exist_ok=True)
    else:
        os.makedirs(os.path.dirname(dst) or ".", exist_ok=True)
        shutil.copy2(src, dst)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tasks", default=TASKS)
    parser.add_argument("--out", default=OUT)
    args = parser.parse_args()
    for task, pairs in WORKSPACES.items():
        env = os.path.join(args.tasks, task, "environment")
        ws = os.path.join(args.out, "workspaces", task)
        shutil.rmtree(ws, ignore_errors=True)
        os.makedirs(ws)
        for src, dst in pairs:
            copy(os.path.join(env, src), os.path.normpath(os.path.join(ws, dst)))
        with open(os.path.join(args.tasks, task, "instruction.md")) as f:
            instruction = f.read()
        fixture_dir = os.path.join(args.out, "fixtures", task)
        os.makedirs(fixture_dir, exist_ok=True)
        fixture = {
            "schema": "openagents.coder-one.component-fixture.v1",
            "component": "evidence.departures",
            "source": {"kind": "task-anatomy", "task": task},
            "input": {"task": instruction, "workspace": ws},
            "retained": None,
        }
        with open(os.path.join(fixture_dir, "evidence.departures.json"), "w") as f:
            json.dump(fixture, f, indent=2)
            f.write("\n")
        print(task, sum(len(files) for _, _, files in os.walk(ws)))


if __name__ == "__main__":
    main()
