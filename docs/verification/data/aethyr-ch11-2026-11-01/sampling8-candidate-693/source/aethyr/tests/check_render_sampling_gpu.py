#!/usr/bin/env python3
"""Actual shared eight-tap sampling and a planted missing-demotion control."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tarfile
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def stop_group(process):
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=3)


def run(command, output, *, error=None, stdin=None, timeout=240):
    began = time.monotonic()
    with open(output, "wb") as stdout, open(error or str(output) + ".stderr", "wb") as stderr:
        process = subprocess.Popen(command, cwd=ROOT, stdin=stdin, stdout=stdout,
                                   stderr=stderr, start_new_session=True)
        try:
            code = process.wait(timeout=timeout)
        except BaseException:
            stop_group(process)
            raise
    if code:
        raise RuntimeError(f"command exited {code}: {command}; see {error or output}")
    return {"command": command, "exit_code": code, "seconds": round(time.monotonic() - began, 3)}


def inputs(coax, source):
    with tempfile.TemporaryDirectory(prefix="render-source-inventory-") as temporary:
        # deps requires a program entry even for shader/library-only sources.
        # This carrier adds no production declarations and is excluded below.
        carrier = Path(temporary) / "inputs.coax"
        carrier.write_text('#load ' + json.dumps(str(source)) + ';\nmain :: () {}\n')
        result = subprocess.run([str(coax), "deps", str(carrier)], cwd=ROOT,
                                capture_output=True, text=True, timeout=30)
        if result.returncode:
            raise RuntimeError(result.stderr)
        data = json.loads(result.stdout)
        return {p: sha(p) for p in data["sources"] + data["descriptors"] if Path(p).resolve() != carrier.resolve()}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("output", type=Path)
    parser.add_argument("--coax", type=Path, default=ROOT / "coax/bin/coax")
    parser.add_argument("--reuse-emissions", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    coax = args.coax.resolve()
    compiler_hash = sha(coax)
    fixture = ROOT / "aethyr/tests/render-sampling-shader.coax"
    source_inputs = inputs(coax, fixture)
    for path in ["aethyr/tests/render-sampling-gpu.c", "aethyr/tests/render-capabilities-bridge.coax",
                 "verse/gpu/vks.c", "verse/gpu/vks.h", "verse/gpu/vkc.c", "verse/gpu/vkc.h",
                 "aethyr/tests/check_render_sampling_gpu.py"]:
        source_inputs[str(ROOT / path)] = sha(ROOT / path)
    source_inputs.update(inputs(coax, ROOT / "aethyr/tests/render-capabilities-bridge.coax"))
    commands = []
    receipt = {"kind": "candidate-native-sampling-gpu", "status": "running", "compiler": str(coax),
               "compiler_sha256": compiler_hash, "source_inputs": source_inputs, "commands": commands}
    try:
        if args.reuse_emissions:
            previous = args.reuse_emissions.resolve()
            old = json.loads((previous / "qualification.json").read_text())
            closure = inputs(coax, fixture)
            closure.update(inputs(coax, ROOT / "aethyr/tests/render-capabilities-bridge.coax"))
            if old["compiler_sha256"] != compiler_hash or any(old["source_inputs"].get(p) != digest for p, digest in closure.items()):
                raise RuntimeError("reused emission compiler/source mismatch")
            emissions = [c for c in old["commands"] if len(c["command"]) > 1 and c["command"][1] in {"spirv", "c"}]
            if len(emissions) != 4 or any(c["exit_code"] != 0 for c in emissions):
                raise RuntimeError("reused emissions are incomplete or failed")
            files = ["filtered.spv", "invalid.spv", "fault.spv", "filtered.reflection", "invalid.reflection", "fault.reflection",
                     "RenderCaps.c", "RenderCaps.h", "sampling-fault.coax", "fault-source.coax"]
            for name in files:
                if sha(previous / name) != old["artifacts"].get(name):
                    raise RuntimeError("reused emission artifact mismatch: " + name)
                (out / name).write_bytes((previous / name).read_bytes())
            receipt["reused_emissions"] = {"receipt": str(previous / "qualification.json"), "sha256": sha(previous / "qualification.json"), "commands": emissions}
        else:
            for name, fragment in [("filtered", "render_sampling_fs"), ("invalid", "render_sampling_invalid_fs")]:
                commands.append(run([str(coax), "spirv", str(fixture), "--vertex", "render_sampling_vs",
                                     "--fragment", fragment, "-o", str(out / (name + ".spv"))],
                                    out / (name + ".stdout"), error=out / (name + ".reflection")))
            adapter = ROOT / "aethyr/render/sampling.coax"
            text = adapter.read_text().replace('#load "sampling-core.coax";',
                                               '#load "' + str(adapter.parent / "sampling-core.coax") + '";')
            guard = "if !rs_result.valid {gpu_demote_helper();}"
            if text.count(guard) != 2:
                raise RuntimeError("sampling adapter shape changed; fault needs explicit review")
            (out / "sampling-fault.coax").write_text(text.replace(guard, "if !rs_result.valid {}", 1))
            fault = out / "fault-source.coax"
            fault.write_text(fixture.read_text().replace('#load "../render/sampling.coax";',
                             '#load "' + str(out / "sampling-fault.coax") + '";'))
            commands.append(run([str(coax), "spirv", str(fault), "--vertex", "render_sampling_vs",
                                 "--fragment", "render_sampling_invalid_fs", "-o", str(out / "fault.spv")],
                                out / "fault.stdout", error=out / "fault.reflection"))
            commands.append(run([str(coax), "c", "aethyr/tests/render-capabilities-bridge.coax", "--library",
                                 "--header", str(out / "RenderCaps.h")], out / "RenderCaps.c"))
        for relative in ["aethyr/tests/render-sampling-gpu.c", "verse/gpu/vks.c", "verse/gpu/vks.h", "verse/gpu/vkc.c", "verse/gpu/vkc.h"]:
            (out / Path(relative).name).write_bytes((ROOT / relative).read_bytes())
        (out / "remote.sh").write_text('''set -euo pipefail
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json
timeout 300 nix-shell -p gcc vulkan-headers vulkan-loader vulkan-validation-layers spirv-tools --run '
set -e
for file in filtered.spv invalid.spv fault.spv; do spirv-val --target-env vulkan1.1 "$file"; done
gcc -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unknown-pragmas -I. render-sampling-gpu.c vks.c vkc.c RenderCaps.c -lvulkan -lm -o render-sampling-gpu
gcc --version >gcc-version.txt
command -v gcc >gcc-path.txt
sha256sum "$(command -v gcc)" render-sampling-gpu >native-hashes.txt
export VK_ADD_LAYER_PATH=$(dirname $(find /nix/store -maxdepth 6 -name VkLayer_khronos_validation.json -path "*validation-layers*" | head -1))
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
./render-sampling-gpu filtered.spv invalid.spv >positive.log 2>&1
if ./render-sampling-gpu filtered.spv fault.spv >fault.log 2>&1; then echo "missing-demotion control escaped"; exit 1; fi
grep -q "wrong=16 validation_errors=0 validation_warnings=0" fault.log
'
''')
        # No environment dump; only selected compiler identity and fixture logs.
        remote = "set -e; d=$(mktemp -d); trap 'rm -rf \"$d\"' EXIT; tar -xf - -C \"$d\"; cd \"$d\"; status=0; bash remote.sh >remote.log 2>&1 || status=$?; printf '%s\\n' \"$status\" >remote-status.txt; tar -cf - --ignore-failed-read remote.log remote-status.txt positive.log fault.log gcc-version.txt gcc-path.txt native-hashes.txt render-sampling-gpu 2>/dev/null"
        files = ["filtered.spv", "invalid.spv", "fault.spv", "RenderCaps.c", "RenderCaps.h", "render-sampling-gpu.c", "vks.c", "vks.h", "vkc.c", "vkc.h", "remote.sh"]
        with tarfile.open(out / "send.tar", "w") as archive:
            for name in files:
                archive.add(out / name, arcname=name)
        with open(out / "send.tar", "rb") as stdin:
            commands.append(run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", "-o", "ServerAliveInterval=15",
                                 "-o", "ServerAliveCountMax=2", args.host, remote], out / "returned.tar", stdin=stdin, timeout=360))
        with tarfile.open(out / "returned.tar") as archive:
            for member in archive.getmembers():
                if member.name not in {"remote.log", "remote-status.txt", "positive.log", "fault.log", "gcc-version.txt", "gcc-path.txt", "native-hashes.txt", "render-sampling-gpu"} or not member.isfile():
                    raise RuntimeError("unexpected returned artifact")
                stream = archive.extractfile(member)
                (out / member.name).write_bytes(stream.read())
        if (out / "remote-status.txt").read_text().strip() != "0":
            raise RuntimeError("native GPU command failed; see remote.log and retained pixel logs")
        if "wrong=0 validation_errors=0 validation_warnings=0" not in (out / "positive.log").read_text():
            raise RuntimeError("positive pixel or validation oracle did not pass")
        if "wrong=16 validation_errors=0 validation_warnings=0" not in (out / "fault.log").read_text():
            raise RuntimeError("planted demotion control did not fail for exactly the expected pixels")
        final_closure = inputs(coax, fixture)
        final_closure.update(inputs(coax, ROOT / "aethyr/tests/render-capabilities-bridge.coax"))
        changed = [p for p, digest in source_inputs.items() if sha(p) != digest]
        if set(final_closure) - set(source_inputs):
            changed.extend(sorted(set(final_closure) - set(source_inputs)))
        if sha(coax) != compiler_hash:
            changed.append(str(coax))
        receipt["source_changes"] = changed
        if changed:
            raise RuntimeError("inputs changed during qualification")
        receipt["status"] = "passed"
    except BaseException as error:
        receipt["status"] = "failed"
        receipt["error"] = str(error)
        raise
    finally:
        receipt["artifacts"] = {p.name: sha(p) for p in out.iterdir() if p.is_file() and p.name != "qualification.json"}
        (out / "qualification.json").write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    main()
