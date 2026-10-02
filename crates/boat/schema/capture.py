#!/usr/bin/env python3
"""Record redacted read-only Boat responses into fixtures/recorded/.

Only GET requests are sent; nothing billable is created. The key comes from
BOAT_API_KEY and is never printed. Every string is replaced unless its field
name is an enum-like allowlisted key, ids keep their prefix shape, arrays keep
at most two items, and request ids are dropped.

    set -a; . ~/work/.secrets/boat.env; set +a
    python3 crates/boat/schema/capture.py
"""
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = os.environ.get("BOAT_API_BASE", "https://boat.dev/api/v1").rstrip("/")
LEGACY = "https://boat.dev/api/box/v1"
READS = {
    "me": "/me",
    "listOrganizations": "/orgs",
    "limits": "/limits",
    "sandboxes": "/sandboxes?limit=2",
    "environments": "/environments",
    "listNamedSnapshots": "/named-snapshots",
    "apiKeys": "/api-keys",
}
KEEP = {
    "type", "state", "status", "health", "machineProvider", "sandboxType", "kind",
    "sort", "mode", "code", "plan", "tier", "encoding", "role",
}
ID = re.compile(r"^([a-z]{2,6})_[A-Za-z0-9]+$")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}T")


def redact(value, key=None):
    if isinstance(value, dict):
        return {k: redact(v, k) for k, v in value.items() if k not in {"requestId"}}
    if isinstance(value, list):
        return [redact(v, key) for v in value[:2]]
    if isinstance(value, str):
        if key in KEEP:
            return value
        if DATE.match(value):
            return "2026-01-01T00:00:00.000Z"
        match = ID.match(value)
        if match:
            return f"{match.group(1)}_23456789"
        if "@" in value:
            return "user@example.com"
        return "redacted"
    return value


def get(base, path, key):
    request = urllib.request.Request(
        base + path,
        headers={"Authorization": f"Bearer {key}", "User-Agent": "openagents-boat-capture"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status, json.loads(response.read())
    except urllib.error.HTTPError as error:
        return error.code, None


def main():
    key = os.environ.get("BOAT_API_KEY", "").strip()
    if not key:
        sys.exit("BOAT_API_KEY is not set.")
    out = ROOT / "fixtures/recorded"
    out.mkdir(parents=True, exist_ok=True)
    for operation, path in READS.items():
        status, body = get(BASE, path, key)
        print(f"{operation}: HTTP {status}")
        if status == 200 and body is not None:
            record = {"operation_id": operation, "method": "GET", "path": path.split("?")[0], "status": status, "response": redact(body)}
            (out / f"{operation}.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    status, _ = get(LEGACY, "/limits", key)
    print(f"legacy base /limits: HTTP {status}")


if __name__ == "__main__":
    main()
