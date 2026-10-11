#!/usr/bin/env python3
"""Run one task-owned reference process with memory and time limits.

The caller must hold memory and GPU leases. The GPU lease serializes the
reference with native model runs even when the reference uses the CPU.
"""

import argparse
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import time

from mac_quiet_bench import OwnedProcessGuard, available_memory, stop_owned_group


def positive(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("limit must be finite and positive")
    return number


def run(args):
    leases = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    if not {"memory", "gpu"} <= leases:
        raise ValueError("reference execution requires caller-held memory and GPU leases")
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        raise ValueError("missing reference command")
    if args.max_rss_gib > 80:
        raise ValueError("reference RSS limit cannot exceed 80 GiB")
    if args.minimum_start_available_gib < args.max_rss_gib + args.minimum_available_gib:
        raise ValueError("startup memory must cover the RSS limit plus the available-memory margin")
    args.output.mkdir(parents=True, exist_ok=False)
    before = available_memory()
    record = {"command": command, "leases": sorted(leases), "preflight": before,
              "max_rss_gib": args.max_rss_gib, "deadline_seconds": args.deadline_seconds,
              "minimum_start_available_gib": args.minimum_start_available_gib,
              "minimum_available_gib": args.minimum_available_gib,
              "status": "refused", "exit_code": None}
    if before["available_memory_bytes"] < args.minimum_start_available_gib * 1024 ** 3:
        record["reason"] = "insufficient available memory before model startup"
        (args.output / "guard.json").write_text(json.dumps(record, indent=2) + "\n")
        print(json.dumps(record), flush=True)
        return 1
    environment = dict(os.environ)
    environment.update(HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", TOKENIZERS_PARALLELISM="false",
                       OMP_NUM_THREADS="2", OPENBLAS_NUM_THREADS="2", VECLIB_MAXIMUM_THREADS="2",
                       MKL_NUM_THREADS="2", PYTORCH_ENABLE_MPS_FALLBACK="0")
    process = guard = None
    began = time.monotonic()
    old_handlers = {}
    def interrupt(signum, _frame):
        raise KeyboardInterrupt(f"signal {signum}")
    try:
        for signum in (signal.SIGINT, signal.SIGTERM):
            old_handlers[signum] = signal.signal(signum, interrupt)
        with (args.output / "reference.log").open("wb") as log:
            process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL,
                                       stdout=log, stderr=log, start_new_session=True)
            guard = OwnedProcessGuard(process, int(args.max_rss_gib * 1024 ** 3),
                                      args.output / "memory.jsonl",
                                      minimum_available_memory_bytes=int(args.minimum_available_gib * 1024 ** 3))
            while process.poll() is None:
                guard.guard()
                if time.monotonic() - began > args.deadline_seconds:
                    raise TimeoutError("reference deadline exceeded")
                time.sleep(0.2)
            guard.guard()
            record["exit_code"] = process.returncode
            record["status"] = "complete" if process.returncode == 0 else "failed"
    except BaseException as error:
        record.update(status="interrupted" if isinstance(error, KeyboardInterrupt) else "failed",
                      error_type=type(error).__name__, reason=str(error))
    finally:
        try:
            if guard is not None:
                try:
                    guard.close()
                finally:
                    record["memory"] = guard.receipt()
                if guard.failure:
                    record["status"] = "failed"
            elif process is not None:
                stop_owned_group(process)
        except Exception as error:
            record.update(status="failed", cleanup_error=str(error))
        for signum, previous in old_handlers.items():
            signal.signal(signum, previous)
        record["elapsed_seconds"] = time.monotonic() - began
        (args.output / "guard.json").write_text(json.dumps(record, indent=2, allow_nan=False) + "\n")
    print(json.dumps(record), flush=True)
    return 0 if record["status"] == "complete" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--max-rss-gib", type=positive, default=68)
    parser.add_argument("--minimum-start-available-gib", type=positive, default=80)
    parser.add_argument("--minimum-available-gib", type=positive, default=12)
    parser.add_argument("--deadline-seconds", type=positive, default=900)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
