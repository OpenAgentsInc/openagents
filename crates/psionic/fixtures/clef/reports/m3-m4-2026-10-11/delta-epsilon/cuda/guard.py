"""Bound only this validation's child process group; retain no unrelated process details."""
import argparse
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import time

GIB = 1024**3


def available():
    values = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text().splitlines())
    return int(values["MemAvailable"].split()[0]) * 1024


def members(group):
    found = []
    for path in Path("/proc").iterdir():
        if not path.name.isdigit():
            continue
        try:
            fields = (path / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) != group:
                continue
            status = dict(line.split(":", 1) for line in (path / "status").read_text().splitlines())
            found.append({"pid": int(path.name), "rss_bytes": int(status.get("VmRSS", "0").split()[0]) * 1024})
        except (FileNotFoundError, ProcessLookupError):
            continue
    return found


parser = argparse.ArgumentParser()
parser.add_argument("--name", required=True)
parser.add_argument("--timeout", type=float, required=True)
parser.add_argument("--gpu", action="store_true")
parser.add_argument("command", nargs=argparse.REMAINDER)
args = parser.parse_args()
command = args.command[1:] if args.command[:1] == ["--"] else args.command
receipt = {"command": command, "rss_cap_bytes": 2 * GIB, "gpu_process_cap_bytes": 2 * GIB,
           "available_floor_bytes": 16 * GIB, "timeout_seconds": args.timeout, "samples": []}
receipt["available_before_bytes"] = available()
if receipt["available_before_bytes"] < 18 * GIB:
    raise RuntimeError("host memory preflight refused validation")
started = time.monotonic()
child = None
reason = None
prefix = Path(args.name)
try:
    with prefix.with_suffix(".stdout.log").open("wb") as output, prefix.with_suffix(".stderr.log").open("wb") as errors:
        child = subprocess.Popen(command, stdout=output, stderr=errors, start_new_session=True)
        receipt["owned_pgid"] = child.pid
        while child.poll() is None:
            owned = members(child.pid)
            sample = {"elapsed_seconds": time.monotonic() - started, "rss_bytes": sum(p["rss_bytes"] for p in owned),
                      "available_bytes": available(), "owned_members": owned}
            if args.gpu:
                query = subprocess.run(["nvidia-smi", "--query-compute-apps=pid,used_gpu_memory", "--format=csv,noheader,nounits"],
                                       text=True, capture_output=True, timeout=5, check=True)
                pids = {row["pid"] for row in owned}
                sample["owned_gpu_bytes"] = sum(int(parts[1].strip()) * 1024**2
                    for line in query.stdout.splitlines() if len(parts := line.split(",")) == 2
                    and parts[0].strip().isdigit() and int(parts[0]) in pids)
            receipt["samples"].append(sample)
            if sample["rss_bytes"] > 2 * GIB or sample.get("owned_gpu_bytes", 0) > 2 * GIB:
                reason = "owned memory limit exceeded"
            elif sample["available_bytes"] < 16 * GIB:
                reason = "host available-memory floor crossed"
            elif time.monotonic() - started > args.timeout:
                reason = "owned deadline exceeded"
            if reason:
                break
            time.sleep(0.02)
except Exception as error:
    reason = f"guard error: {type(error).__name__}: {error}"
finally:
    if child is not None:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=3)
        remaining = members(child.pid)
        if remaining:
            os.killpg(child.pid, signal.SIGTERM)
            time.sleep(0.1)
            if members(child.pid):
                os.killpg(child.pid, signal.SIGKILL)
            reason = reason or "owned descendants remained"
        receipt["remaining_owned_members"] = members(child.pid)
        receipt["exit_code"] = child.returncode
receipt["stop_reason"] = reason
receipt["elapsed_seconds"] = time.monotonic() - started
receipt["max_sampled_group_rss_bytes"] = max((row["rss_bytes"] for row in receipt["samples"]), default=0)
receipt["max_sampled_owned_gpu_bytes"] = max((row.get("owned_gpu_bytes", 0) for row in receipt["samples"]), default=0)
receipt["max_waited_child_rss_bytes"] = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * 1024
receipt["available_after_bytes"] = available()
receipt["passed"] = (reason is None and receipt.get("exit_code") == 0 and
                     receipt["max_waited_child_rss_bytes"] <= 2 * GIB and not receipt.get("remaining_owned_members"))
prefix.with_suffix(".json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps({key: value for key, value in receipt.items() if key != "samples"}), flush=True)
raise SystemExit(0 if receipt["passed"] else 1)
