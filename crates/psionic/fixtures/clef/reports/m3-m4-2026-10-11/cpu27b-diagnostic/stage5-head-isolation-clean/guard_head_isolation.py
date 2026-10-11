#!/usr/bin/env python3
"""Bound the task-owned CPU head diagnostic to 4 GiB and 300 seconds."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time

ROOT = Path("repo")
BASE = Path("task-scratch/clef-m3-m4")
PYTHON = Path("openagents-scratch/codex-01a128ee-daef-7590-baae-4e932a581a2c/clef-image-reference/bin/python")
SCRIPT = Path(__file__).with_name("head_isolation.py")
sys.path.insert(0, str(ROOT / "crates/psionic/fixtures/clef/tools"))
from mac_quiet_bench import OwnedProcessGuard, available_memory, file_sha, stop_owned_group
from head_isolation import require, write

GIB = 1024**3


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-014", type=Path, required=True)
    parser.add_argument("--native-034", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, default=BASE / "artifacts/clef-bf16")
    parser.add_argument("--reference-dir", type=Path, default=BASE / "reference27b/cpu-head-stage1/results")
    args = parser.parse_args()
    resources = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    require("memory" in resources and not resources.intersection({"quiet", "gpu"}), "requires memory lease only; no GPU or quiet lease")
    require(not args.output_dir.exists(), "output directory already exists; preserve earlier evidence")
    args.output_dir.mkdir(parents=True)
    command = [str(PYTHON), str(SCRIPT), "--checkpoint", str(args.checkpoint.resolve()),
               "--reference-dir", str(args.reference_dir.resolve()), "--native-014", str(args.native_014.resolve()),
               "--native-034", str(args.native_034.resolve()), "--output-dir", str((args.output_dir / "results").resolve())]
    bounds = {name: "2" for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS",
                                      "MKL_NUM_THREADS", "VECLIB_MAXIMUM_THREADS")}
    bounds.update(CLEF_HEAD_ISOLATION_GUARDED="1", PYTORCH_ENABLE_MPS_FALLBACK="0", HF_HUB_OFFLINE="1",
                  TRANSFORMERS_OFFLINE="1", PYTHONDONTWRITEBYTECODE="1", CUDA_VISIBLE_DEVICES="")
    environment = dict(os.environ)
    environment.update(bounds)
    receipt = args.output_dir / "guard.json"
    record = {"kind": "owned_cpu_head_isolation", "status": "preflight", "command": command,
              "script_sha256": file_sha(SCRIPT), "guard_sha256": file_sha(Path(__file__)),
              "memory_helper_sha256": file_sha(ROOT / "crates/psionic/fixtures/clef/tools/mac_quiet_bench.py"),
              "effective_environment": bounds, "max_owned_group_gib": 4, "minimum_start_available_gib": 16,
              "minimum_available_gib": 12, "deadline_seconds": 300, "conditions": "shared machine; CPU diagnostic only",
              "lease_resources": sorted(resources), "load_average_before": os.getloadavg()}
    write(receipt, record)
    process = guard = timer = None
    expired = threading.Event()
    old_signals = {}
    began = time.monotonic()

    def interrupt(signum, _frame):
        raise KeyboardInterrupt(f"signal {signum}")

    try:
        for signum in (signal.SIGINT, signal.SIGTERM):
            old_signals[signum] = signal.signal(signum, interrupt)
        record["before"] = available_memory()
        if record["before"]["available_memory_bytes"] < 16*GIB:
            record.update(status="refused", reason="startup_available_memory")
            return 1
        with (args.output_dir / "head.log").open("xb") as log:
            process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL,
                                       stdout=log, stderr=log, start_new_session=True)
            guard = OwnedProcessGuard(process, 4*GIB, args.output_dir / "memory.jsonl",
                                      minimum_available_memory_bytes=12*GIB)
            record.update(status="running", owned_pid=process.pid)
            write(receipt, record)
            print(json.dumps({"cpu_head_active": True, "owned_pid": process.pid}), flush=True)

            def expire():
                expired.set()
                guard.stop_process(grace=1)

            timer = threading.Timer(300, expire)
            timer.start()
            while process.poll() is None:
                guard.guard()
                time.sleep(0.2)
            record["child_exit_code"] = process.returncode
            guard.guard()
            require(process.returncode == 0, "CPU head diagnostic failed; inspect head.log")
            output = args.output_dir / "results/isolation.json"
            require(output.exists() and json.loads(output.read_text())["status"] == "complete", "result is incomplete")
            record.update(status="complete", result_sha256=file_sha(output))
    except BaseException as error:
        record.update(status="failed", error_type=type(error).__name__, error=str(error))
    finally:
        if timer is not None:
            timer.cancel()
            timer.join()
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
        except BaseException as error:
            record.update(status="failed", cleanup_error=str(error))
        for signum, prior in old_signals.items():
            signal.signal(signum, prior)
        if expired.is_set():
            record.update(status="failed", error="deadline exceeded")
        record["elapsed_seconds"] = time.monotonic()-began
        record["load_average_after"] = os.getloadavg()
        write(receipt, record)
        print(json.dumps({"cpu_head_active": False, "status": record["status"],
                          "owned_group_stopped": record.get("memory", {}).get("owned_group_stopped")}), flush=True)
    return 0 if record["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
