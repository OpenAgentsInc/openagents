#!/usr/bin/env python3
"""Run evidence.baseline on the task anatomy's 18 untouched workspaces.

Reads the workspaces and instructions that the departures experiment's
build_workspaces.py rebuilt under ~/.openagents/coder-one/departures-offline,
writes one evidence.baseline fixture per task under
~/.openagents/coder-one/baseline-offline/fixtures, runs the component on
each, and writes every result to ~/.openagents/coder-one/baseline-offline/
results.json, which measure.py reads. Nothing it writes goes into the
repository: the workspaces are benchmark content.

Usage: run_offline.py [--bin PATH] [--python-bin DIR]

--python-bin puts a directory first on PATH, for a python3 with the
packages the task images install (NumPy and SciPy for the drift monitor).
The script sets SUPERVISE_MEMORY_MAX=off, because setrlimit(RLIMIT_DATA)
fails on the macOS host this ran on.
"""

import argparse
import json
import os
import subprocess

HOME = os.path.expanduser("~")
SRC = os.path.join(HOME, ".openagents/coder-one/departures-offline")
OUT = os.path.join(HOME, ".openagents/coder-one/baseline-offline")
REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../.."))
# The two tasks whose working directory isn't /app.
WORKDIR = {"vba-userform-port": "/workspace", "intrastat-meldung": "/workspace"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", default=os.path.join(REPO, "target/debug/coder-one"))
    parser.add_argument("--python-bin", default=None)
    args = parser.parse_args()
    env = dict(os.environ, SUPERVISE_MEMORY_MAX="off")
    if args.python_bin:
        env["PATH"] = args.python_bin + os.pathsep + env["PATH"]
    results = {}
    for task in sorted(os.listdir(os.path.join(SRC, "fixtures"))):
        source = json.load(open(os.path.join(SRC, "fixtures", task, "evidence.departures.json")))
        fixture = {
            "schema": "openagents.coder-one.component-fixture.v1",
            "component": "evidence.baseline",
            "source": {"kind": "task-anatomy", "task": task},
            "input": {
                "task": source["input"]["task"],
                "workspace": os.path.join(SRC, "workspaces", task),
                "workdir": WORKDIR.get(task, "/app"),
            },
            "retained": None,
        }
        directory = os.path.join(OUT, "fixtures", task)
        os.makedirs(directory, exist_ok=True)
        with open(os.path.join(directory, "evidence.baseline.json"), "w") as handle:
            json.dump(fixture, handle, indent=2)
        ran = subprocess.run(
            [args.bin, "component", "run", "evidence.baseline", "--fixture", directory, "--json"],
            capture_output=True, text=True, env=env, check=True)
        result = json.loads(ran.stdout)["result"]["fixtures"][0]
        results[task] = result
        baseline = result["output"]["baseline"]
        print(task, len(baseline["runs"]), "ran,", len(baseline["refused"]), "refused")
    with open(os.path.join(OUT, "results.json"), "w") as handle:
        json.dump(results, handle, indent=1)


if __name__ == "__main__":
    main()
