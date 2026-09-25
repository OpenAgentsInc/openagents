#!/usr/bin/env python3
"""Lists every tool call each oracle writer made, and flags any that
reached outside its own directory.

Usage: audit_writers.py RECORDS

A writer's workspace held only spec.json and cases.json. A call is flagged
when a command or a path names a parent directory, an absolute path other
than a temporary directory, or the home directory.
"""

import glob
import json
import os
import re
import sys

ALLOWED = ("/tmp", "/dev/null", "/usr/bin/env", "/bin/sh", "/usr/bin/python3")


def flags(text):
    found = []
    if ".." in text:
        found.append("names a parent directory")
    if "~" in text or "$HOME" in text:
        found.append("names the home directory")
    for path in re.findall(r"(?<![\w.])/[A-Za-z_][\w./-]*", text):
        if not path.startswith(ALLOWED) and not path.startswith("/app"):
            found.append(f"absolute path {path}")
    return found


def main():
    root = sys.argv[1]
    report = {}
    for trace in sorted(glob.glob(os.path.join(root, "*", "oracle-writer-*.atif.jsonl"))):
        task = os.path.basename(os.path.dirname(trace))
        calls = []
        for line in open(trace):
            try:
                record = json.loads(line)
            except ValueError:
                continue
            call = (record.get("step") or {}).get("call")
            if not call:
                continue
            args = call.get("arguments") or {}
            text = args.get("command") or args.get("path") or ""
            calls.append({"tool": call.get("name"), "what": text[:300], "flags": flags(text)})
        report[task] = {
            "calls": len(calls),
            "flagged": [c for c in calls if c["flags"]],
        }
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
