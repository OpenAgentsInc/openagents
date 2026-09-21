#!/usr/bin/env python3
"""Maintain one manual-gate run record: begin, phase results, finish.

Each run gets `<dir>/<run-id>/run.json`, rewritten atomically after every
call so a killed gate still leaves the evidence it produced. Phase logs
live beside the record; a pass is bound to the tree digest it covered,
never to "the gate" as a standing fact.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile
import uuid
from datetime import datetime, timezone
from pathlib import Path

SCHEMA = "openagents.gate-run.v1"


def git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def tree_state(root: Path) -> dict:
    diff = subprocess.run(
        ["git", "-C", str(root), "diff", "HEAD", "--binary"],
        capture_output=True,
    )
    digest = ""
    if diff.returncode == 0:
        import hashlib

        digest = hashlib.sha256(diff.stdout).hexdigest()
    return {
        "head": git(root, "rev-parse", "HEAD"),
        "dirty": bool(git(root, "status", "--porcelain")),
        "diff_digest": digest,
    }


def record_path(directory: Path, run_id: str) -> Path:
    return directory / run_id / "run.json"


def read(directory: Path, run_id: str) -> dict:
    path = record_path(directory, run_id)
    if not path.exists():
        raise SystemExit(f"no gate record at {path}")
    return json.loads(path.read_text())


def write(record: dict, directory: Path, run_id: str) -> Path:
    path = record_path(directory, run_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(dir=path.parent, prefix=".run", suffix=".json")
    with os.fdopen(fd, "w") as out:
        json.dump(record, out, indent=2)
        out.write("\n")
    os.replace(temporary, path)
    return path


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def begin(args: argparse.Namespace) -> int:
    directory = Path(args.dir)
    run_id = args.run_id or (
        datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        + "-"
        + uuid.uuid4().hex[:6]
    )
    record = {
        "schema": SCHEMA,
        "run_id": run_id,
        "started_utc": utc_now(),
        "finished_utc": None,
        "elapsed_s": None,
        "tree": tree_state(Path(args.root)),
        "requested": json.loads(args.requested),
        "phases": [],
        "skipped": [],
        "result": "running",
    }
    path = write(record, directory, run_id)
    print(json.dumps({"run_id": run_id, "record": str(path)}))
    return 0


def phase(args: argparse.Namespace) -> int:
    directory = Path(args.dir)
    record = read(directory, args.run_id)
    entry = {
        "slug": args.slug,
        "name": args.name,
        "command": json.loads(args.command),
        "status": args.status,
        "attempts": args.attempts,
        "exit": args.exit,
        "elapsed_s": args.elapsed,
        "log": args.log,
    }
    record["phases"].append({key: value for key, value in entry.items() if value is not None})
    write(record, directory, args.run_id)
    return 0


def skip(args: argparse.Namespace) -> int:
    directory = Path(args.dir)
    record = read(directory, args.run_id)
    record["skipped"].append({"slug": args.slug, "reason": args.reason})
    write(record, directory, args.run_id)
    return 0


def finish(args: argparse.Namespace) -> int:
    directory = Path(args.dir)
    record = read(directory, args.run_id)
    started = datetime.strptime(record["started_utc"], "%Y-%m-%dT%H:%M:%SZ")
    elapsed = (datetime.now(timezone.utc) - started.replace(tzinfo=timezone.utc)).total_seconds()
    record["finished_utc"] = utc_now()
    record["elapsed_s"] = round(elapsed, 1)
    record["result"] = args.result
    path = write(record, directory, args.run_id)
    print(str(path))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    start = commands.add_parser("begin", help="start a run record")
    start.add_argument("--dir", required=True)
    start.add_argument("--run-id")
    start.add_argument("--root", default=".")
    start.add_argument("--requested", default="{}",
                       help="JSON describing the requested phases, crates, and flags")
    start.set_defaults(call=begin)

    step = commands.add_parser("phase", help="record one finished phase")
    step.add_argument("--dir", required=True)
    step.add_argument("--run-id", required=True)
    step.add_argument("--slug", required=True)
    step.add_argument("--name", required=True)
    step.add_argument("--command", default="[]", help="JSON argv")
    step.add_argument("--status", required=True,
                      choices=["passed", "failed", "scoped-out"])
    step.add_argument("--attempts", type=int, default=1)
    step.add_argument("--exit", type=int)
    step.add_argument("--elapsed", type=float)
    step.add_argument("--log")
    step.set_defaults(call=phase)

    omitted = commands.add_parser("skip", help="record a skipped phase")
    omitted.add_argument("--dir", required=True)
    omitted.add_argument("--run-id", required=True)
    omitted.add_argument("--slug", required=True)
    omitted.add_argument("--reason", required=True)
    omitted.set_defaults(call=skip)

    end = commands.add_parser("finish", help="close the record")
    end.add_argument("--dir", required=True)
    end.add_argument("--run-id", required=True)
    end.add_argument("--result", required=True,
                     choices=["passed", "failed", "partial", "aborted"])
    end.set_defaults(call=finish)

    args = parser.parse_args()
    return args.call(args)


if __name__ == "__main__":
    sys.exit(main())
