#!/usr/bin/env python3
"""NEW token mutants: actual unity apply/emit/undo plus independent native C.

This consumes a private frontend build; it does not install or qualify a
compiler release, and never executes downloaded source or old model gates.
"""
from pathlib import Path
import argparse
import concurrent.futures
import hashlib
import json
import os
import platform
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frontend-build", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    build, output = args.frontend_build.resolve(), args.output.resolve()
    if args.jobs < 1 or args.jobs > 16:
        parser.error("jobs must be in 1..16")
    if output.exists():
        parser.error("output exists; preserve earlier evidence and choose another directory")
    for name in ("proto_importer.o", "proto_importer.h", "proto_runtime.inc"):
        if not (build / name).is_file():
            parser.error("missing private frontend input: " + name)
    output.mkdir(parents=True)
    start = time.monotonic()
    native = ROOT / "coax/tests/spatial-operator-oracle.c"
    unity = ROOT / "coax/tests/spatial-operator-native.cpp"
    source = ROOT / "coax/tests/spatial/operators.coax"
    source_files = [unity, native, Path(__file__).resolve(), source,
                    ROOT / "coax/tests/spatial/operator-types.coax",
                    ROOT / "coax/tests/spatial/operator-native-wrappers.coax"]
    record = {"schema": "coax.spatial-operator-native.v1", "passed": False,
              "platform": platform.platform(), "machine": platform.machine(),
              "sources_sha256": {str(p.relative_to(ROOT)): sha(p) for p in source_files},
              "frontend_inputs_sha256": {name: sha(build / name) for name in
                                          ("proto_importer.o", "proto_importer.h", "proto_runtime.inc")},
              "commands": [], "profiles": [],
              "scope": "Twelve NEW binary token mutants; source-local callee swap, checked expression/initializer/type-description snapshots and exact C/header undo, then actual checked C. This does not snapshot every statement/procedure/compile-time heap object. No full old mutation, numeric, SDK or release gate."}
    frontend_sources = sorted((ROOT / "coax/src").rglob("*"))
    record["frontend_source_sha256"] = {str(p.relative_to(ROOT)): sha(p) for p in frontend_sources if p.is_file()}
    execution_log = []
    execution_lock = threading.Lock()

    def save():
        (output / "receipt.json").write_text(json.dumps(record, indent=2) + "\n")

    def run(name, command, *, directory=output):
        result = subprocess.run([str(a) for a in command], cwd=ROOT,
                                capture_output=True, timeout=180,
                                env={**os.environ, "ASAN_OPTIONS": "detect_leaks=0:abort_on_error=1",
                                     "UBSAN_OPTIONS": "halt_on_error=1:print_stacktrace=1"})
        (directory / (name + ".stdout")).write_bytes(result.stdout)
        (directory / (name + ".stderr")).write_bytes(result.stderr)
        row = {"name": name, "arguments": [str(a) for a in command],
               "artifact_directory": str(directory),
               "exit_status": result.returncode, "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
               "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
               "stderr_bytes": len(result.stderr)}
        with execution_lock:
            execution_log.append(row)
        if result.returncode or result.stderr:
            raise RuntimeError(f"{name}: exit {result.returncode}: {result.stderr.decode(errors='replace')}")
        return result, row

    try:
        executable = output / "unity-probe"
        _, row = run("unity-build", [os.environ.get("CXX", "c++"), "-std=c++20", "-O1", "-g",
                                      "-Wall", "-Wextra", "-Wno-missing-field-initializers", "-I", build,
                                      unity, build / "proto_importer.o", "-o", executable])
        record["commands"].append(row)
        _, row = run("apply-emit-undo", [executable, source.relative_to(ROOT), output / "emitted"])
        record["commands"].append(row)
        emission = json.loads((output / "emitted/emission.json").read_text())
        assert (emission["eligible"], emission["no_alternate"], emission["unsupported"]) == (12, 6, 2)
        variants = [{"index": -1, "name": "baseline", "width": 0, "segment": -1}]
        variants += [{**v, "name": "variant-" + str(v["index"])} for v in emission["variants"]]
        assert len(variants) == 13
        record["source_inventory"] = emission
        record["emitted_sha256"] = {v["name"]: {ext: sha(output / "emitted" / v["name"] / ("operators." + ext))
                                                for ext in ("c", "h")} for v in variants}

        def one(task):
            mode, flags, variant = task
            directory = output / mode / variant["name"]
            directory.mkdir(parents=True)
            emitted = output / "emitted" / variant["name"]
            binary = directory / "probe"
            _, compile_record = run("build", [os.environ.get("CC", "cc"), "-std=c11", "-fno-fast-math",
                                                "-ffp-contract=off", *flags, "-I", emitted,
                                                emitted / "operators.c", native, "-lm", "-o", binary], directory=directory)
            result, execution_record = run("execute", [binary, variant["width"], variant["segment"]], directory=directory)
            assert len(result.stdout) == 360
            return {"mode": mode, "variant": variant["name"], "width": variant["width"],
                    "segment": variant["segment"], "observations": 60,
                    "differs_from_original_coordinates": 3 if variant["index"] >= 0 else 0,
                    "transcript_sha256": hashlib.sha256(result.stdout).hexdigest(),
                    "commands": [compile_record, execution_record]}

        profiles = [("debug", ["-O0"]), ("optimized", ["-O2"]),
                    ("asan-ubsan", ["-O1", "-g", "-fsanitize=address,undefined",
                                     "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"])]
        tasks = [(mode, flags, v) for mode, flags in profiles for v in variants]
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            for result in pool.map(one, tasks):
                record["profiles"].append(result)
                save()
        for variant in variants:
            hashes = [r["transcript_sha256"] for r in record["profiles"] if r["variant"] == variant["name"]]
            assert len(hashes) == 3 and len(set(hashes)) == 1
        record["native_observations"] = 13 * 3 * 60
        record["actual_source_token_native_witnesses"] = 12
        assert all(sha(ROOT / p) == digest for p, digest in record["frontend_source_sha256"].items())
        record["passed"] = True
    finally:
        record["execution_log"] = execution_log
        record["duration_seconds"] = time.monotonic() - start
        save()
    print("spatial token native: twelve site-local apply/emit/undo witnesses; 2340 exact coordinates across three host profiles")


if __name__ == "__main__":
    main()
