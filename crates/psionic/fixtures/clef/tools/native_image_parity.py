#!/usr/bin/env python3
"""Start one bounded native Clef server and compare its image/text answers."""

import argparse
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import threading
import time

import image_parity
from mac_quiet_bench import OwnedProcessGuard, available_memory, file_sha, stop_owned_group


def run(config):
    if not {"gpu", "memory"} <= set(os.environ.get("OPENAGENTS_LEASES", "").split(",")):
        raise ValueError("native validation requires caller-held memory and GPU leases")
    for name, maximum in [("max_rss_gib", 84), ("minimum_start_available_gib", 128),
                          ("minimum_available_gib", 128), ("startup_timeout", 1800),
                          ("deadline_seconds", 7200), ("request_timeout", 1800)]:
        value = config[name]
        if type(value) not in (float, int) or not math.isfinite(value) or not 0 < value <= maximum:
            raise ValueError(f"invalid native validation limit: {name}")
    if config["minimum_start_available_gib"] < config["max_rss_gib"] + config["minimum_available_gib"]:
        raise ValueError("startup memory must cover the RSS limit plus the available-memory margin")
    output = Path(config["output"])
    output.mkdir(parents=True, exist_ok=False)
    checks = config["checks"]
    if not checks or any(check["suite"] not in ("images", "text") for check in checks):
        raise ValueError("checks must include an image or text reference")
    # Check reference completeness before the expensive native model load.
    for check in checks:
        reference = json.loads(Path(check["reference"]).read_text())
        if not reference.get("completed") or reference.get("dtype") != "bfloat16":
            raise ValueError("native validation requires completed BF16 reference files")
    record = {"config": config, "status": "refused", "artifacts": {}, "checks": []}
    paths = {key: Path(config[key]) for key in ("binary", "gguf", "head", "vision_manifest")}
    for key, path in paths.items():
        record["artifacts"][key] = {"path": str(path), "sha256": file_sha(path), "bytes": path.stat().st_size}
    record["head_source"] = "external official BF16 safetensors, converted by the native head loader"
    before = available_memory()
    record["preflight"] = before
    if before["available_memory_bytes"] < config["minimum_start_available_gib"] * 1024 ** 3:
        record["reason"] = "insufficient available memory before native model startup"
        image_parity.write(output / "run.json", record)
        print(json.dumps(record), flush=True)
        return 1
    command = [str(paths["binary"]), "--decision-model", str(paths["gguf"]),
               "--clef-head", str(paths["head"]), "--decision-device", "metal",
               "--decision-vision-manifest", str(paths["vision_manifest"]),
               "--decision-vision-blobs", config["vision_blobs"],
               "--host", "127.0.0.1", "--port", "0", "--decision-chunk", str(config.get("chunk", 512))]
    environment = {key: value for key, value in os.environ.items() if not key.startswith("PSIONIC_CLEF_")}
    environment.update(RAYON_NUM_THREADS="2", VECLIB_MAXIMUM_THREADS="2", OMP_NUM_THREADS="2",
                       PSIONIC_CLEF_PREFIX_CACHE_BYTES="0")
    environment.update(config.get("env", {}))
    if any(key in environment for key in ("PSIONIC_CLEF_PROFILE", "PSIONIC_CLEF_SKIP")):
        raise ValueError("profile and skip modes cannot establish answer correctness")
    record["command"] = command
    process = guard = timer = None
    deadline_hit = threading.Event()
    began = time.monotonic()
    previous_handlers = {}
    def interrupt(signum, _frame):
        raise KeyboardInterrupt(f"signal {signum}")
    try:
        for signum in (signal.SIGINT, signal.SIGTERM):
            previous_handlers[signum] = signal.signal(signum, interrupt)
        log_path = output / "server.log"
        with log_path.open("wb") as log:
            process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL,
                                       stdout=log, stderr=log, start_new_session=True)
            guard = OwnedProcessGuard(process, int(config["max_rss_gib"] * 1024 ** 3),
                                      output / "memory.jsonl",
                                      minimum_available_memory_bytes=int(config["minimum_available_gib"] * 1024 ** 3))
            def deadline():
                deadline_hit.set()
                guard.stop_process(grace=1)
            timer = threading.Timer(config.get("deadline_seconds", 1800), deadline)
            timer.start()
            endpoint = None
            while endpoint is None:
                guard.guard()
                if process.poll() is not None:
                    raise RuntimeError("owned native server exited before readiness")
                if time.monotonic() - began > config.get("startup_timeout", 600):
                    raise TimeoutError("native server startup timed out")
                match = re.search(r"listening on (http://127\.0\.0\.1:\d+)", log_path.read_text(errors="replace"))
                if match:
                    with image_parity.HTTP.open(match[1] + "/health", timeout=2) as response:
                        if response.status == 200:
                            endpoint = match[1]
                time.sleep(0.2)
            record["endpoint"] = endpoint
            for index, check in enumerate(checks):
                args = argparse.Namespace(
                    reference=Path(check["reference"]), endpoint=endpoint, model="clef",
                    cases=Path(check.get("cases", image_parity.DEFAULT_CASES)),
                    text_suite=Path(check["requests"]) if check["suite"] == "text" else None,
                    output=output / f"{index}-{check['suite']}.json", timeout=config.get("request_timeout", 600),
                )
                code = image_parity.native(args)
                record["checks"].append({"suite": check["suite"], "output": args.output.name, "exit_code": code})
                guard.guard()
            record["status"] = "complete" if all(row["exit_code"] == 0 for row in record["checks"]) else "failed"
    except BaseException as error:
        record.update(status="interrupted" if isinstance(error, KeyboardInterrupt) else "failed",
                      error_type=type(error).__name__, reason=str(error))
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
        except Exception as error:
            record.update(status="failed", cleanup_error=str(error))
        for signum, previous in previous_handlers.items():
            signal.signal(signum, previous)
        if deadline_hit.is_set():
            record.update(status="failed", reason="native validation deadline exceeded")
        record["elapsed_seconds"] = time.monotonic() - began
        image_parity.write(output / "run.json", record)
    print(json.dumps(record), flush=True)
    return 0 if record["status"] == "complete" else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    raise SystemExit(run(json.loads(parser.parse_args().config.read_text())))
