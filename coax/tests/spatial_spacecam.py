#!/usr/bin/env python3
"""Changed Spacecam boundary only: native F64 oracle and actual frame refusal."""
from pathlib import Path
import hashlib
import json
import os
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "coax/bin/coax")).resolve()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: spatial_spacecam.py NEW_ARTIFACT_DIRECTORY")
    work = Path(sys.argv[1]).resolve()
    work.mkdir(parents=True, exist_ok=False)
    source = ROOT / "coax/tests/spatial/spacecam-native.coax"
    harness = ROOT / "coax/tests/spatial-spacecam-native.c"
    record = {"schema": "coax-spatial-spacecam-v1", "compiler": str(COMPILER),
              "compiler_sha256": sha(COMPILER), "platform": platform.platform(),
              "source_sha256": {str(p.relative_to(ROOT)): sha(p) for p in
                                 (source, harness, Path(__file__).resolve(), ROOT / "verse/spacecam.coax")},
              "cases_per_mode": 512, "coordinate_observations_per_mode": 1536,
              "non_nan_comparison": "exact F64 bits", "nan_comparison": "classification; canonicalized only in transcript",
              "commands": [], "modes": [], "passed": False}

    def run(name, args, *, success=True):
        result = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, timeout=120)
        (work / (name + ".stdout")).write_bytes(result.stdout)
        (work / (name + ".stderr")).write_bytes(result.stderr)
        record["commands"].append({"name": name, "arguments": [str(a) for a in args],
                                   "exit_status": result.returncode, "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                                   "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()})
        assert (result.returncode == 0) == success, (name, result.stderr.decode(errors="replace"))
        return result

    try:
        refusal = work / "f32-refused.coax"
        refusal.write_text(f'#load "{ROOT / "verse/spacecam.coax"}";\n#load "{ROOT / "coax/lib/quantity.coax"}" as q;\nSCN_refuse_low_precision :: (p:Point3(SpaceCamWorld,q.Metre,f32)) {{}}\n')
        failed = run("actual-world-f32-refusal", [COMPILER, "c", refusal, "--library"], success=False)
        assert failed.stderr.count(b"error[") == 1 and b"E0833" in failed.stderr and b"SpaceCamWorld" in failed.stderr and not failed.stdout
        outputs = []
        for mode, flags in (("debug", ["-O0"]), ("release", ["-O2"]),
                            ("asan-ubsan", ["-O1", "-g", "-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"])):
            directory = work / mode
            directory.mkdir()
            emitted = run(mode + "-emit", [COMPILER, "c", source, "--library", *(["--release"] if mode != "debug" else []), "--header", directory / "spacecam-native.h"])
            assert not emitted.stderr
            c = directory / "generated.c"
            c.write_bytes(emitted.stdout)
            executable = directory / "probe"
            build = run(mode + "-build", [os.environ.get("CC", "cc"), "-std=c11", "-fno-fast-math", "-ffp-contract=off", *flags,
                                          "-I", directory, c, harness, "-lm", "-o", executable])
            assert not build.stderr
            native = run(mode + "-native", [executable])
            assert not native.stderr and len(native.stdout) == 1536 * 8
            outputs.append(native.stdout)
            record["modes"].append({"name": mode, "C_sha256": sha(c), "header_sha256": sha(directory / "spacecam-native.h"),
                                    "transcript_sha256": hashlib.sha256(native.stdout).hexdigest(), "stderr_bytes": 0})
        assert outputs[0] == outputs[1] == outputs[2]
        record["passed"] = True
    finally:
        (work / "receipt.json").write_text(json.dumps(record, indent=2) + "\n")
    print("Spacecam: 1536 exact/classified F64 coordinates per mode; three modes agree; F32 world refused")


if __name__ == "__main__":
    main()
