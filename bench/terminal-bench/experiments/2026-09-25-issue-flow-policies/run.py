#!/usr/bin/env python3
"""Run the registered issue-flow comparison, preserving each attempt."""

import argparse
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess


ORDER = [
    ("9450", "lean"), ("9450", "requirements"),
    ("9446", "requirements"), ("9446", "lean"),
    ("9451", "lean"), ("9451", "requirements"),
    ("9597", "requirements"), ("9597", "lean"),
    ("9450", "requirements"), ("9450", "lean"),
    ("9446", "lean"), ("9446", "requirements"),
    ("9451", "requirements"), ("9451", "lean"),
    ("9597", "lean"), ("9597", "requirements"),
]
POLICIES = {"lean": "issue-flow-lean.json", "requirements": "issue-flow.json"}


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def write(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    pins = json.loads((here / "pins.json").read_text())
    assert digest(args.binary) == pins["binary_sha256"], "binary changed"
    assert json.loads((here / "records/preflight/graders.json").read_text())["set_digest"] == pins["set_digest"]
    graders = json.loads((here / "records/preflight/graders.json").read_text())["entries"]
    assert len(graders) == 4 and all(row["discriminates"] for row in graders)
    for name, expected in pins["policy_files"].items():
        assert digest(args.source / "crates/coder-one/policies" / name) == expected
    args.out.mkdir(parents=True, exist_ok=True)
    with (args.out / "driver.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        journal = args.out / "runs.jsonl"
        rows = [json.loads(line) for line in journal.read_text().splitlines()] if journal.exists() else []
        # An interrupted attempt is never replaced automatically.
        assert all(row.get("manifest") for row in rows), "inspect retained incomplete attempt before continuing"
        assert len(rows) <= len(ORDER)
        for i, row in enumerate(rows):
            assert (row["entry"], row["arm"]) == ORDER[i] and row["slot"] == i + 1
        env = os.environ.copy()
        env.pop("CARGO_TARGET_DIR", None)
        env.update(pins["environment"])
        state = {"started_at": now(), "pid": os.getpid(), "status": "running"}
        for index in range(len(rows), len(ORDER)):
            costs = [row["counted_usd"] for row in rows]
            spent, largest = sum(costs), max(costs, default=0.0)
            if index == 8 and spent > 2.5:
                state.update(status="stopped", reason="round 1 exceeded $2.50")
                break
            if index % 2 == 0 and spent + 2 * largest > 5.0:
                state.update(status="stopped", reason="next pair exceeds the registered budget rule")
                break
            entry, arm = ORDER[index]
            slot = index + 1
            slot_dir = args.out / f"{slot:02d}-{entry}-{arm}"
            # Creating the directory reserves this slot even if the driver dies.
            slot_dir.mkdir(exist_ok=False)
            command = [str(args.binary), "issue-eval", "run", entry,
                       "--policy", POLICIES[arm], "--jev", "live", "--model", "gpt-6-luna",
                       "--network", "off", "--source", str(args.source),
                       "--set", str(args.source / "crates/coder-one/issues-eval"),
                       "--target-dir", str(args.target_dir), "--out", str(slot_dir / "runs"), "--json"]
            attempt = {"slot": slot, "entry": entry, "arm": arm, "started_at": now(), "command": command}
            write(slot_dir / "launch.json", attempt)
            state.update(slot=slot, entry=entry, arm=arm, counted_usd=spent, updated_at=now())
            write(args.out / "state.json", state)
            print(f"{now()} slot {slot}/16: {entry} {arm}; counted ${spent:.6f}", flush=True)
            with (slot_dir / "stdout.json").open("w") as stdout, (slot_dir / "stderr.log").open("w") as stderr:
                result = subprocess.run(command, cwd=args.source, env=env, stdout=stdout, stderr=stderr)
            attempt.update(ended_at=now(), exit_code=result.returncode)
            manifests = list((slot_dir / "runs").glob("*/manifest.json"))
            if len(manifests) != 1:
                attempt.update(manifest=None, reason="missing or ambiguous final manifest")
                write(slot_dir / "attempt.json", attempt)
                with journal.open("a") as stream:
                    stream.write(json.dumps(attempt) + "\n")
                state.update(status="needs-inspection", reason=attempt["reason"])
                break
            manifest = json.loads(manifests[0].read_text())
            cost = manifest["cost"]
            counted = cost["total_usd"] if cost["total_usd"] is not None else cost["lower_bound_usd"]
            assert isinstance(counted, (int, float)) and counted >= 0, "cost unavailable; stop"
            attempt.update(manifest=str(manifests[0]), counted_usd=counted,
                           cost_complete=cost["total_usd"] is not None,
                           verdict=manifest["grade"]["verdict"], contaminated=manifest["contaminated"],
                           policy=manifest["policy"], milliseconds=manifest["milliseconds"])
            write(slot_dir / "attempt.json", attempt)
            with journal.open("a") as stream:
                stream.write(json.dumps(attempt) + "\n")
                stream.flush()
                os.fsync(stream.fileno())
            rows.append(attempt)
            assert manifest["sealed"]["network_off"] and manifest["sealed"]["github_withheld"]
            assert manifest["task"]["set_digest"] == pins["set_digest"]
            assert manifest["policy"]["digest"] == pins["policy_digests"][arm]
            print(f"{now()} slot {slot}: {attempt['verdict']}, ${counted:.6f}, {manifest['milliseconds']/1000:.1f}s", flush=True)
        else:
            state.update(status="complete")
        state.update(updated_at=now(), completed=len(rows), counted_usd=sum(row["counted_usd"] for row in rows))
        write(args.out / "state.json", state)
        print(json.dumps(state), flush=True)


if __name__ == "__main__":
    main()
