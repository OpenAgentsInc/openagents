#!/usr/bin/env python3
"""Build and run only the tiny native Q4_K decoder under explicit leases."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time

ROOT = Path("repo")
HERE = Path(__file__).resolve().parent
TARGET = Path("cargo-target")
OUT = HERE / "native-run-lto"
sys.path.insert(0, str(ROOT / "crates/psionic/fixtures/clef/tools"))
from mac_quiet_bench import OwnedProcessGuard, available_memory, stop_owned_group


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024**2), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write(path, data):
    path.write_text(json.dumps(data, indent=2, allow_nan=False)+"\n")


def run_child(name, command, deadline):
    record = {"command": command, "deadline_seconds": deadline, "max_owned_bytes": 4*1024**3,
              "available_floor_bytes": 12*1024**3, "status": "starting"}
    process = guard = timer = None
    expired = threading.Event()
    began = time.monotonic()
    try:
        with (OUT / (name+".log")).open("xb") as log:
            process = subprocess.Popen(command, cwd=ROOT, stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                       start_new_session=True)
            guard = OwnedProcessGuard(process, 4*1024**3, OUT / (name+"-memory.jsonl"),
                                      minimum_available_memory_bytes=12*1024**3)
            def expire():
                expired.set()
                guard.stop_process(grace=1)
            timer = threading.Timer(deadline, expire)
            timer.start()
            while process.poll() is None:
                guard.guard()
                time.sleep(0.1)
            guard.guard()
            record.update(exit_code=process.returncode, status="complete" if process.returncode == 0 else "failed")
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
        if expired.is_set():
            record.update(status="failed", error="deadline exceeded")
        record["elapsed_seconds"] = time.monotonic()-began
        write(OUT / (name+".json"), record)
    if record["status"] != "complete":
        raise RuntimeError(name+" failed")
    return record


def main():
    leases = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    assert {"build", "memory"} <= leases and not leases.intersection({"gpu", "quiet"})
    OUT.mkdir(exist_ok=False)
    initial = available_memory()
    receipt = {"status": "preflight", "leases": sorted(leases), "before": initial,
               "guard_sha256": sha(Path(__file__)), "checks": []}
    write(OUT / "receipt.json", receipt)
    if initial["available_memory_bytes"] < 16*1024**3:
        receipt.update(status="refused", reason="less than 16 GiB available")
        write(OUT / "receipt.json", receipt)
        return 1
    old_handlers = {}
    def interrupt(signum, _frame):
        raise KeyboardInterrupt(f"signal {signum}")
    try:
        for signum in (signal.SIGINT, signal.SIGTERM):
            old_handlers[signum] = signal.signal(signum, interrupt)
        fingerprints = TARGET / "release/.fingerprint"
        candidates = sorted(fingerprints.glob("psionic-backend-cpu-*/lib-psionic_backend_cpu.json"),
                            key=lambda path: path.stat().st_mtime_ns, reverse=True)
        backend_meta = candidates[0]
        metadata = json.loads(backend_meta.read_text())
        core_dep = next(row[3] for row in metadata["deps"] if row[1] == "psionic_core")
        core_fingerprint = next(path for path in fingerprints.glob("psionic-core-*/lib-psionic_core")
                                if int.from_bytes(bytes.fromhex(path.read_text().strip()), "little") == core_dep)
        backend_hash = backend_meta.parent.name.removeprefix("psionic-backend-cpu-")
        core_hash = core_fingerprint.parent.name.removeprefix("psionic-core-")
        backend = TARGET / f"release/deps/libpsionic_backend_cpu-{backend_hash}.rlib"
        core = TARGET / f"release/deps/libpsionic_core-{core_hash}.rlib"
        files = {"harness": HERE / "native_decode.rs", "packed_samples": HERE / "sample-blocks.q4k",
                 "independent_outputs": HERE / "independent-decoded.f32", "backend_rlib": backend, "core_rlib": core,
                 "backend_fingerprint_json": backend_meta, "core_fingerprint": core_fingerprint,
                 "backend_source": ROOT / "crates/psionic/crates/psionic-backend-cpu/src/lib.rs",
                 "core_source": ROOT / "crates/psionic/crates/psionic-core/src/lib.rs"}
        receipt["files"] = {name:{"path":str(path), "bytes":path.stat().st_size, "sha256":sha(path)} for name,path in files.items()}
        receipt["source_binding"] = "Current source hashes and Cargo fingerprint dependency selection are recorded; cached artifacts are reused without rebuilding crates."
        receipt["checks"].append(run_child("toolchain", ["rustc", "+1.97.1", "-Vv"], 30))
        output = OUT / "native_decode"
        command = ["rustc", "+1.97.1", "--edition=2024", str(HERE / "native_decode.rs"),
                   "-L", "dependency="+str(TARGET / "release/deps"),
                   "--extern", "psionic_backend_cpu="+str(backend), "--extern", "psionic_core="+str(core),
                   "-C", "opt-level=2", "-C", "lto=thin", "-C", "codegen-units=1", "-o", str(output)]
        receipt["checks"].append(run_child("compile", command, 180))
        receipt["binary"] = {"path": str(output), "bytes": output.stat().st_size, "sha256": sha(output)}
        receipt["checks"].append(run_child("run", [str(output), str(HERE)], 60))
        receipt["result"] = json.loads((OUT / "run.log").read_text())
        assert receipt["result"]["bitwise_mismatches"] == 0 and receipt["result"]["values"] == 1327104
        assert all(sha(files[name]) == info["sha256"] for name, info in receipt["files"].items())
        receipt["status"] = "complete"
    except BaseException as error:
        receipt.update(status="failed", error_type=type(error).__name__, error=str(error))
    finally:
        for signum, previous in old_handlers.items():
            signal.signal(signum, previous)
        write(OUT / "receipt.json", receipt)
        print(json.dumps({"status": receipt["status"], "result":receipt.get("result"), "error":receipt.get("error")}),flush=True)
    return 0 if receipt["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
