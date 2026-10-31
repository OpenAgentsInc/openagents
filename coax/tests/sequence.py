#!/usr/bin/env python3
"""Qualify borrowed sequence operations and their actual native consumers.

No server, GPU, browser or model inference runs. Consumer host-effect imports
abort if called: only the actual emitted private formatting/rotation functions
are exercised. --baseline-dir accepts frozen C/headers from the same compiler;
without it, only current artifacts and independent oracles are qualified.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import platform
import re
import shlex
import signal
import subprocess
import time

from quiet_crash import run as run_expected_fault

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().with_suffix("")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--baseline-dir", type=Path)
    parser.add_argument("--reuse-sequence-o0", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    if (out / "result.json").exists():
        raise SystemExit("use a fresh output directory; keep prior passing evidence")
    coax = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax")).resolve()
    cc = shlex.split(os.environ.get("CC", "cc"))
    deadline = time.monotonic() + 840
    commands = []

    def run(command, name, *, expected=0):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("qualification reached its fourteen-minute command budget")
        start = time.monotonic()
        runner = run_expected_fault if expected is None else subprocess.run
        result = runner([str(x) for x in command], cwd=ROOT, capture_output=True,
                                text=True, timeout=min(150, remaining), env={
                                    **os.environ,
                                    "ASAN_OPTIONS": "detect_leaks=0:halt_on_error=1",
                                    "UBSAN_OPTIONS": "halt_on_error=1:print_stacktrace=1",
                                })
        (out / (name + ".stdout")).write_text(result.stdout)
        (out / (name + ".stderr")).write_text(result.stderr)
        commands.append({"name": name, "argv": [str(x) for x in command],
                         "exit_status": result.returncode,
                         "seconds": time.monotonic() - start})
        if expected is not None and result.returncode != expected:
            raise RuntimeError(f"{name}: exit {result.returncode}\n{result.stderr[-4000:]}")
        return result

    version = run([coax, "--version"], "compiler-version").stdout.strip()
    compiler_sha = digest(coax)
    toolchain = run(cc + ["--version"], "native-compiler-version").stdout.splitlines()[0]
    sources = {
        "sequence": ROOT / "coax/tests/sequence/fixture.coax",
        "verse": ROOT / "verse/gpu/everglade-pass.coax",
        "psionic": ROOT / "psionic/lib/serve-engine.coax",
    }
    generated = {}
    for name, source in sources.items():
        header, c_file = out / (name + "-after.h"), out / (name + "-after.c")
        result = run([coax, "c", source.relative_to(ROOT), "--library", "--header", header,
                      "--timings", out / (name + "-timings.json")], "emit-" + name)
        c_file.write_text(result.stdout)
        generated[name] = {"after": {"c": c_file, "header": header}}
        if args.baseline_dir and name != "sequence":
            base = args.baseline_dir.resolve()
            generated[name]["before"] = {"c": base / (name + "-before.c"),
                                         "header": base / (name + "-before.h")}

    observations = []
    artifacts = []
    for kind, revisions in generated.items():
        for revision, artifact in revisions.items():
            source = artifact["c"]
            header = artifact["header"]
            assert source.is_file() and header.is_file()
            foreign = out / (kind + "-" + revision + "-foreign.h")
            # Only host effects are stubbed, never executed Coax policy/overlap.
            section = header.read_text().split("/* Provided by the host (#foreign). */", 1)
            stubs = []
            if len(section) == 2:
                for result, name, parameters in re.findall(
                        r"^(int64_t|uint64_t|int32_t|uint32_t|double|float|_Bool|void) "
                        r"([a-zA-Z_][a-zA-Z_0-9]*)\(([^;]*)\);", section[1], re.M):
                    stubs.append(f"{result} {name}({parameters}) {{ abort(); }}")
            foreign.write_text("\n".join(stubs) + "\n")
            for opt, sanitized in ((0, False), (2, False), (0, True), (2, True)):
                tag = f"{kind}-{revision}-O{opt}" + ("-sanitized" if sanitized else "")
                driver = HERE / ("native.c" if kind == "sequence" else "consumer-native.c")
                obj, exe = out / (tag + ".o"), out / tag
                command = cc + ["-std=c11", f"-O{opt}", "-g", "-ffp-contract=off",
                                "-fno-fast-math", "-fstack-usage", "-Wno-unused-function",
                                "-Wno-unused-parameter", "-Wno-unknown-pragmas",
                                "-D_DARWIN_C_SOURCE" if platform.system() == "Darwin"
                                else "-D_POSIX_C_SOURCE=200809L",
                                "-DSEQUENCE_INPUT_C=" + json.dumps(str(source)),
                                "-DSEQUENCE_FOREIGN_STUBS=" + json.dumps(str(foreign)),
                                "-DSEQUENCE_CONSUMER_VERSE=" + str(int(kind == "verse"))]
                if sanitized:
                    command += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
                run(command + ["-c", driver, "-o", obj], "compile-" + tag)
                link = cc + [f"-O{opt}", str(obj), "-lm", "-o", str(exe)]
                if sanitized:
                    link += ["-fsanitize=address,undefined"]
                run(link, "link-" + tag)
                if kind == "sequence" and opt == 0 and not sanitized and args.reuse_sequence_o0:
                    previous = json.loads(args.reuse_sequence_o0.read_text())
                    assert previous["compiler_sha256"] == compiler_sha
                    assert previous["driver_sha256"] == digest(driver)
                    assert previous["generated_c_sha256"] == digest(source)
                    observation = previous["observation"]
                    reused = str(args.reuse_sequence_o0.resolve())
                else:
                    result = run([exe], "run-" + tag)
                    observation = json.loads(result.stdout)
                    reused = None
                assert observation["allocations"] == 0
                observations.append({"kind": kind, "revision": revision,
                                     "optimization": opt, "sanitized": sanitized,
                                     "observation": observation, "reused": reused})
                traps = ("negative", "past-end") if kind == "sequence" else (("min",) if kind == "verse" else ())
                for trap in traps:
                    failure = run([exe, trap], "trap-" + tag + "-" + trap, expected=None)
                    reason = "integer overflow" if kind == "verse" else "index out of bounds"
                    assert failure.returncode == -signal.SIGABRT, (tag, trap, failure.returncode)
                    assert reason in failure.stderr, (tag, trap, failure.stderr)
                    assert "ERROR: AddressSanitizer" not in failure.stderr
                    assert "runtime error:" not in failure.stderr
                stack_path = obj.with_suffix(".su")
                stack = [line for line in stack_path.read_text().splitlines()
                         if re.search(r"coax_p_(put_int|sv_reverse|.*swap__|.*reverse__)", line)]
                size = run(["size", obj], "size-" + tag).stdout.strip()
                artifacts.append({"tag": tag, "object_bytes": obj.stat().st_size,
                                  "executable_bytes": exe.stat().st_size,
                                  "object_sha256": digest(obj), "executable_sha256": digest(exe),
                                  "native_size": size, "stack_usage": stack,
                                  "generated_c_bytes": source.stat().st_size,
                                  "generated_c_sha256": digest(source),
                                  "header_sha256": digest(header)})

    api = {}
    for name in ("verse", "psionic"):
        if "before" in generated[name]:
            before, after = generated[name]["before"]["header"], generated[name]["after"]["header"]
            assert before.read_bytes() == after.read_bytes(), name
            api[name] = {"public_header_bytes_identical": True, "sha256": digest(after)}
    record = {"schema_version": 1, "outcome": "passed", "compiler": version,
              "compiler_sha256": compiler_sha, "native_compiler": toolchain,
              "platform": platform.platform(), "observations": observations,
              "artifacts": artifacts, "public_api": api, "commands": commands,
              "source_sha256": {str(p.relative_to(ROOT)): digest(p) for p in
                  [ROOT / "coax/lib/sequence.coax", *sources.values(),
                   HERE / "native.c", HERE / "consumer-native.c", Path(__file__)]},
              "limits": ["Native compiler stack reports are per function, not measured whole-process peak stack.",
                         "Allocation counters observe explicit generated-C malloc/calloc/realloc calls, not allocations inside system libraries.",
                         "Consumer host-effect imports abort if called; no network, server, GPU or model inference is exercised.",
                         "This is finite native qualification, not general lifetime or cross-target safety."]}
    (out / "result.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"sequence qualification: {len(observations)} native profiles passed; {out / 'result.json'}")


if __name__ == "__main__":
    main()
