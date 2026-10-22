#!/usr/bin/env python3
"""Passively sample a pinned qualification worker's GPU memory and TCP RTT.

Run on a dedicated stage VM before qualification starts. This starts no model
or network probe. GPU observations use NVML through nvidia-smi; TCP RTT is the
local kernel's estimator, not one-way application handoff latency.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import time


def worker_processes(executable):
    """Keep identities and stage ordinals; never retain authentication args."""
    result = {}
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit():
            continue
        try:
            if (proc / "exe").resolve() != executable:
                continue
            args = [a.decode() for a in (proc / "cmdline").read_bytes().split(b"\0") if a]
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        fields = {}
        for flag in ["--stage", "--stage-index", "--gpu", "--gpus"]:
            if flag in args and args.index(flag) + 1 < len(args):
                fields[flag.removeprefix("--")] = args[args.index(flag) + 1]
        result[int(proc.name)] = fields
    return result


def tcp_rows(text, processes):
    """Read ss -tinp's header/metrics pairs for verified worker PIDs only."""
    rows = []
    current = None
    for line in text.splitlines():
        words = line.split()
        if words and words[0].isupper():
            current = None
            if len(words) < 5 or words[0] != "ESTAB":
                continue
            owners = sorted({int(p) for p in re.findall(r"\bpid=(\d+)", line)} & processes.keys())
            if owners:
                current = {"pids": owners, "local": words[3], "peer": words[4]}
            continue
        if current is None:
            continue
        rtt = re.search(r"\brtt:([\d.]+)/([\d.]+)", line)
        if not rtt:
            continue
        mean, variance = map(float, rtt.groups())
        if not all(math.isfinite(v) and v >= 0 for v in [mean, variance]):
            continue
        row = {**current, "rtt_ms": mean, "rtt_variation_ms": variance}
        for field in ["bytes_sent", "bytes_received", "bytes_acked", "bytes_retrans"]:
            match = re.search(rf"\b{field}:(\d+)", line)
            row[field] = int(match[1]) if match else None
        rows.append(row)
        current = None
    return rows


def run(args):
    if not 1 <= args.seconds <= 14400:
        raise ValueError("duration must be between 1 and 14400 seconds")
    executable = args.worker.resolve(strict=True)
    build_path = executable.parent / "BUILD.json"
    build_bytes = build_path.read_bytes()
    if hashlib.sha256(build_bytes).hexdigest() != args.bundle:
        raise ValueError("worker bundle differs from the requested pin")
    build = json.loads(build_bytes)
    if hashlib.sha256(executable.read_bytes()).hexdigest() != build["sha256"]:
        raise ValueError("worker executable differs from its bundle")
    if args.instance_id:
        instance = subprocess.check_output([
            "curl", "-fsS", "--max-time", "2", "-H", "Metadata-Flavor: Google",
            "http://metadata.google.internal/computeMetadata/v1/instance/id",
        ], text=True).strip()
        if instance != args.instance_id:
            raise ValueError("this VM differs from the requested instance")
    args.out.mkdir(parents=True, exist_ok=False)
    started = time.time()
    identity = {
        "purpose": "Passive qualification observations",
        "bundle": args.bundle,
        "instance_id": args.instance_id,
        "arch": build["arch"],
        "executable_sha256": build["sha256"],
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "started_wall": started,
        "gpu_interval_ms": 200,
        "tcp_interval_seconds": 1,
        "tcp_semantics": "Kernel-estimated transport RTT, not one-way application latency",
    }
    (args.out / "identity.json").write_text(json.dumps(identity, indent=2) + "\n")
    count = 0
    error = None
    gpu_status = None
    with (args.out / "gpu.csv").open("w") as gpu, (args.out / "gpu.stderr").open("w") as err:
        monitor = subprocess.Popen([
            "stdbuf", "-oL", "nvidia-smi",
            "--query-gpu=timestamp,index,uuid,memory.used,memory.total,utilization.gpu",
            "--format=csv,noheader,nounits", "--loop-ms=200",
        ], stdout=gpu, stderr=err)
        try:
            until = time.monotonic() + args.seconds
            with (args.out / "tcp.jsonl").open("w") as output:
                while time.monotonic() < until and not (args.stop and args.stop.exists()):
                    if monitor.poll() is not None:
                        raise RuntimeError(f"GPU observer exited with status {monitor.returncode}")
                    processes = worker_processes(executable)
                    sampled = subprocess.run(
                        ["sudo", "-n", "ss", "-tinpH"], capture_output=True, text=True, timeout=5
                    )
                    sampled.check_returncode()
                    output.write(json.dumps({
                        "wall": time.time(), "processes": processes,
                        "sockets": tcp_rows(sampled.stdout, processes),
                    }) + "\n")
                    output.flush()
                    count += 1
                    time.sleep(min(1, max(0, until - time.monotonic())))
        except BaseException as caught:
            error = str(caught)
            raise
        finally:
            if monitor.poll() is None:
                monitor.terminate()
            gpu_status = monitor.wait(timeout=10)
            gpu.flush()
            result = {
                "ended_wall": time.time(), "tcp_samples": count, "error": error,
                "gpu_observer_exit": gpu_status,
                "gpu_rows": sum(1 for _ in (args.out / "gpu.csv").open()),
                "worker_processes_terminated": False,
            }
            (args.out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if result["gpu_rows"] == 0:
        raise RuntimeError("the observer recorded no GPU samples")
    print(json.dumps(result))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--bundle", required=True)
    parser.add_argument("--instance-id")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--seconds", type=int, default=7200)
    parser.add_argument("--stop", type=Path)
    run(parser.parse_args())
