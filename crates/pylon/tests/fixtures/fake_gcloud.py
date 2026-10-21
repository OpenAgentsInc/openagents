#!/usr/bin/env python3
"""A stand-in for gcloud in Pylon's cloud tests: no machine is ever created.

Every call is appended to calls.jsonl in FAKE_GCLOUD_ROOT. `compute instances
create NAME` records NAME as running, `delete` removes it, `describe` reports
it, and `list` reports nothing, so the spend guard's sync adopts nothing.
"""

import json
import os
import sys
import time
from pathlib import Path

root = Path(os.environ["FAKE_GCLOUD_ROOT"])
root.mkdir(parents=True, exist_ok=True)
args = sys.argv[1:]
with (root / "calls.jsonl").open("a") as calls:
    calls.write(json.dumps(args) + "\n")
words = [a for a in args if not a.startswith("--")]
machines = root / "machines"
machines.mkdir(exist_ok=True)
if words[:2] == ["compute", "instances"] and len(words) >= 3:
    verb = words[2]
    name = words[3] if len(words) > 3 else ""
    if verb == "create":
        (machines / name).write_text(time.strftime("%Y-%m-%dT%H:%M:%S+00:00", time.gmtime()))
        sys.exit(0)
    if verb == "delete":
        (machines / name).unlink(missing_ok=True)
        sys.exit(0)
    if verb == "describe":
        if (machines / name).exists():
            print(json.dumps({"status": "RUNNING", "creationTimestamp": (machines / name).read_text()}))
            sys.exit(0)
        print(f"ERROR: The resource '{name}' was not found", file=sys.stderr)
        sys.exit(1)
    if verb == "list":
        print("[]")
        sys.exit(0)
if words[:2] == ["compute", "operations"]:
    print("[]")
    sys.exit(0)
if os.environ.get("FAKE_SSH_ERROR"):
    # Stands in for ssh to a machine: what ssh says when it can't connect.
    print(os.environ["FAKE_SSH_ERROR"], file=sys.stderr)
    sys.exit(255)
print("fake gcloud: unexpected call " + " ".join(args), file=sys.stderr)
sys.exit(1)
