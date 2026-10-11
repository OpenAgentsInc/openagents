#!/usr/bin/env python3
"""Bound only task-owned CPU build or artifact-generation children."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import threading
import time

HERE = Path(__file__).resolve().parent
ROOT = Path("$OPENAGENTS_CHECKOUT")
PYTHON = Path("$REFERENCE_PYTHON_ENV/bin/python")
sys.path.insert(0, str(ROOT / "crates/psionic/fixtures/clef/tools"))
from mac_quiet_bench import OwnedProcessGuard, available_memory, file_sha, stop_owned_group

GIB = 1024**3


def require(value, message):
    if not value:
        raise ValueError(message)


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["fetch", "build", "checks", "candidate-checks", "generate"])
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--bridge", type=Path)
    parser.add_argument("--bridge-manifest", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    resources = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    required = {"memory", "disk"} | ({"build"} if args.mode == "build" else set())
    require(required <= resources and not ({"quiet", "gpu"} & resources), "CPU task requires its memory/disk/build leases; GPU and quiet leases are forbidden")
    require(not args.run_dir.exists(), "preserve existing run evidence")
    args.run_dir.mkdir(parents=True)
    run = args.run_dir.resolve()
    if args.mode == "fetch":
        command = [str(PYTHON), str(HERE / "fetch_source.py")]
        cap, start, deadline, disk_need = 2, 14, 180, 27
        expected = run / "fetch.json"
        expected_status = "fetched"
    elif args.mode == "checks":
        require(args.bridge_manifest is not None, "checks require built bridge manifest")
        expected = run / "checks.json"
        command = [str(PYTHON), str(HERE / "check_bridge.py"), "--bridge-manifest", str(args.bridge_manifest.resolve()), "--output", str(expected)]
        cap, start, deadline, disk_need = 2, 14, 60, 27
        expected_status = "passed"
    elif args.mode == "candidate-checks":
        require(args.bridge_manifest is not None and args.candidate is not None, "candidate checks require bridge and candidate")
        expected = run / "checks.json"
        command = [str(PYTHON), str(HERE / "check_candidate.py"), "--bridge-manifest", str(args.bridge_manifest.resolve()),
                   "--candidate", str(args.candidate.resolve()), "--output", str(expected)]
        cap, start, deadline, disk_need = 2, 14, 60, 27
        expected_status = "passed"
    elif args.mode == "build":
        require(args.source is not None, "build requires pinned source checkout")
        command = [str(PYTHON), str(HERE / "build_bridge.py"), "--source", str(args.source.resolve()), "--output-dir", str(run / "build")]
        cap, start, deadline, disk_need = 4, 16, 600, 27
        expected = run / "build/bridge.json"
        expected_status = "built"
    else:
        require(args.bridge is not None and args.bridge_manifest is not None, "generation requires the verified bridge")
        command = [str(PYTHON), str(HERE / "regenerate.py"), "generate", "--output", str(run / "Clef-Q4-fixed-down-Q6.gguf"),
                   "--bridge", str(args.bridge.resolve()), "--bridge-manifest", str(args.bridge_manifest.resolve())]
        cap, start, deadline, disk_need = 2, 14, 1800, 45
        expected = run / "Clef-Q4-fixed-down-Q6.json"
        expected_status = "generated_not_accepted"
    environment = dict(os.environ)
    for key in list(environment):
        if key.startswith("DYLD_") or key in {"LD_PRELOAD", "LD_LIBRARY_PATH"}:
            del environment[key]
    overrides = dict(CLEF_Q6_REGEN_GUARDED="1", PYTHONDONTWRITEBYTECODE="1", OMP_NUM_THREADS="1", OPENBLAS_NUM_THREADS="1",
                     MKL_NUM_THREADS="1", VECLIB_MAXIMUM_THREADS="1", CUDA_VISIBLE_DEVICES="")
    environment.update(overrides)
    if args.mode == "fetch":
        environment["CLEF_Q6_FETCH_RECEIPT"] = str(expected)
    receipt = dict(kind="owned_cpu_q6_candidate_" + args.mode, status="preflight", command=command,
                   memory_cap_gib=cap, start_available_gib=start, available_floor_gib=12, deadline_seconds=deadline,
                   disk_minimum_gib=disk_need, free_disk_floor_gib=25, leases=sorted(resources), effective_environment=overrides,
                   dynamic_library_injection_and_search_overrides_cleared=True,
                   script_sha256={p.name: file_sha(p) for p in (HERE / "regenerate.py", HERE / "build_bridge.py", HERE / "guard.py", HERE / "q6_bridge.cpp", HERE / "check_bridge.py", HERE / "check_candidate.py", HERE / "fetch_source.py")},
                   guard_dependency_sha256={p.name: file_sha(p) for p in (ROOT / "crates/psionic/fixtures/clef/tools/mac_quiet_bench.py", ROOT / "crates/psionic/fixtures/clef/tools/mac_memory.py")})
    record_path = run / "guard.json"
    write(record_path, receipt)
    child = monitor = timer = None
    expired = threading.Event()
    old_signals = {}
    began = time.monotonic()

    def interrupt(signum, frame):
        raise KeyboardInterrupt(f"signal {signum}")

    try:
        for signum in (signal.SIGINT, signal.SIGTERM):
            old_signals[signum] = signal.signal(signum, interrupt)
        receipt["before"] = available_memory()
        require(receipt["before"]["available_memory_bytes"] >= start * GIB, "startup available-memory floor not met")
        require(shutil.disk_usage(run).free >= disk_need * GIB, "insufficient disk for bounded output")
        with (run / "stdout.log").open("xb") as log:
            child = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=log, stderr=log, env=environment, start_new_session=True)
            monitor = OwnedProcessGuard(child, cap * GIB, run / "memory.jsonl", minimum_available_memory_bytes=12 * GIB)
            receipt.update(status="running", owned_pid=child.pid)
            write(record_path, receipt)

            def expire():
                expired.set()
                monitor.stop_process(grace=1)

            timer = threading.Timer(deadline, expire)
            timer.start()
            while child.poll() is None:
                monitor.guard()
                require(shutil.disk_usage(run).free >= 25 * GIB, "free disk floor fell below 25 GiB")
                time.sleep(0.25)
            receipt["child_exit_code"] = child.returncode
            monitor.guard()
            require(child.returncode == 0, "owned CPU task failed; inspect stdout.log")
            require(expected.exists() and json.loads(expected.read_text())["status"] == expected_status, "task result is incomplete")
            receipt.update(status="complete", result_sha256=file_sha(expected))
    except BaseException as error:
        receipt.update(status="failed", error_type=type(error).__name__, error=str(error))
    finally:
        if timer is not None:
            timer.cancel()
            timer.join()
        try:
            if monitor is not None:
                try:
                    monitor.close()
                finally:
                    receipt["memory"] = monitor.receipt()
                if monitor.failure:
                    receipt["status"] = "failed"
            elif child is not None:
                stop_owned_group(child)
        except BaseException as error:
            receipt.update(status="failed", cleanup_error=str(error))
        for signum, prior in old_signals.items():
            signal.signal(signum, prior)
        if expired.is_set():
            receipt.update(status="failed", error="deadline exceeded")
        receipt["elapsed_seconds"] = time.monotonic() - began
        write(record_path, receipt)
        print(json.dumps({"status": receipt["status"], "owned_group_stopped": receipt.get("memory", {}).get("owned_group_stopped")}))
    return 0 if receipt["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
