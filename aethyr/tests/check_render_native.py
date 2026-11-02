#!/usr/bin/env python3
"""One bounded native admission profile; no GPU, browser or app build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    data = path.read_bytes()
    return {"path": str(path), "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}


def closure(entry):
    seen = set()
    def visit(path):
        path = path.resolve()
        path.relative_to(ROOT)
        if path in seen:
            return
        seen.add(path)
        for name in re.findall(r'^\s*#load\s+"([^"]+)"', path.read_text(), re.M):
            visit(path.parent / name)
    visit(ROOT / entry)
    return seen


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--coax", required=True)
    p.add_argument("--cc", default=os.environ.get("COAX_CC", os.environ.get("CC", "cc")))
    p.add_argument("--mode", choices=("debug", "release", "sanitized"), required=True)
    p.add_argument("--out", required=True)
    args = p.parse_args()
    out = Path(args.out)
    if out.exists() or out.is_symlink():
        p.error("--out must be new; existing evidence is never overwritten")
    compiler = Path(args.coax).resolve(strict=True)
    cc = Path(shutil.which(args.cc) or args.cc).resolve(strict=True)
    files = closure("aethyr/tests/render-native.coax") | {Path(__file__).resolve(), ROOT / "aethyr/tests/render-native.c", compiler, cc}
    provenance = compiler.parent / "provenance.record"
    if provenance.is_file():
        files.add(provenance)
    before = [digest(path) for path in sorted(files)]
    out.mkdir(parents=True)
    out = out.resolve()
    observations = []
    def run(argv, label, env=None):
        start = time.monotonic()
        result = subprocess.run([str(x) for x in argv], cwd=ROOT, env=env, capture_output=True, timeout=600)
        (out / (label + ".stdout")).write_bytes(result.stdout)
        (out / (label + ".stderr")).write_bytes(result.stderr)
        observations.append({"argv": [str(x) for x in argv], "exit_status": result.returncode, "seconds": time.monotonic()-start, "stdout": label+".stdout", "stderr": label+".stderr"})
        if result.returncode:
            raise RuntimeError(label+" failed")
        return result
    passed = False
    error = None
    try:
        run([compiler, "--version"], "compiler-version")
        run([cc, "--version"], "cc-version")
        argv = [compiler, "c", "aethyr/tests/render-native.coax", "--library", "--header", out/"render.h"]
        if args.mode == "release":
            argv.append("--release")
        emitted = run(argv, "emit")
        (out/"render.c").write_bytes(emitted.stdout)
        flags = ["-O0", "-g"] if args.mode == "debug" else ["-O2"]
        if args.mode == "sanitized":
            flags = ["-O1", "-g", "-fno-omit-frame-pointer", "-fsanitize=address,undefined"]
        run([cc, "-std=c11", *flags, "-ffp-contract=off", "-fno-fast-math", "-I"+str(out), out/"render.c", ROOT/"aethyr/tests/render-native.c", "-lm", "-o", out/"render-native"], "compile")
        env = dict(os.environ, ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
        result = run([out/"render-native"], "native", env)
        if result.stderr:
            raise RuntimeError("unexpected native diagnostics")
        passed = True
    except Exception as exc:
        error = str(exc)
    after = [digest(path) for path in sorted(files)]
    unchanged = before == after
    artifacts = [digest(path) for path in sorted(out.iterdir()) if path.is_file()]
    receipt = {"schema_version": 1, "mode": args.mode, "passed": passed and unchanged, "error": error,
               "scope": "CPU metadata and actual Grid projection boundary only; no browser/GPU/raster/playable-route qualification. Caller-owned resource/arena preconditions remain.",
               "source_and_tool_before": before, "source_and_tool_after": after, "unchanged": unchanged,
               "commands": observations, "artifacts": artifacts,
               "sanitizer_limit": "Apple leak detection disabled; ASan/UBSan enabled only in sanitized mode."}
    (out/"receipt.json").write_text(json.dumps(receipt, indent=2)+"\n")
    print(json.dumps({"passed": receipt["passed"], "receipt": str(out/"receipt.json")}))
    return 0 if receipt["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
