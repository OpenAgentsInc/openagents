#!/usr/bin/env python3
"""Qualify compiler-owned reflection, registry versions and emitted instructions."""
import argparse
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = "coax/tests/shader/reflection.coax"


def emit(compiler, directory, name, *entries):
    output = directory / (name + ".spv")
    command = [compiler, "spirv", FIXTURE, *entries, "-o", str(output)]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=180)
    if result.returncode:
        raise RuntimeError(result.stderr)
    words = struct.unpack("<" + "I" * (output.stat().st_size // 4), output.read_bytes())
    at, ops, capabilities, extensions = 5, [], [], []
    while at < len(words):
        length, opcode = words[at] >> 16, words[at] & 65535
        if not length or at + length > len(words):
            raise RuntimeError("malformed SPIR-V instruction")
        ops.append(opcode)
        if opcode == 17:
            capabilities.append(words[at + 1])
        if opcode == 10:
            raw = struct.pack("<" + "I" * (length - 1), *words[at + 1:at + length])
            extensions.append(raw.split(b"\0", 1)[0].decode("ascii"))
        at += length
    return result.stderr.splitlines(), ops, capabilities, extensions


def require(lines, expected):
    missing = [line for line in expected if line not in lines]
    if missing:
        raise RuntimeError("missing checked reflection: " + repr(missing) + "\n" + "\n".join(lines))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coax", default=str(ROOT / "coax/bin/coax"))
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="shader-reflection-") as temporary:
        directory = Path(temporary)
        lines, _, caps, _ = emit(args.coax, directory, "shared",
                                 "--vertex", "reflection_vs", "--fragment", "reflection_fs")
        require(lines, ["reflection 2", "tree-version 2",
            "resource 0 1 0 4 3 u", "resource 1 1 0 16 1 instances",
            "resource 2 2 1 0 2 layers", "resource 3 2 2 0 2 depth",
            "resource 4 3 0 0 2 sample", "resource 5 3 1 0 2 comparison",
            "vertex-stride 24", "vertex-input 0 2 4 0", "vertex-input 1 2 2 16",
            "fragment-targets 2", "fragment-depth 0", "derivatives 1 0", "control 1 0",
            "binding 0 storage u", "binding 5 sampler comparison"])
        if 51 in caps or 5379 in caps:
            raise RuntimeError("legacy program acquired a new capability")

        # Real caster entries are vertex-only; no synthetic fragment stage.
        lines, _, _, _ = emit(args.coax, directory, "vertex", "--vertex", "reflection_vs")
        require(lines, ["tree-version 2", "resource 0 1 0 4 1 u",
                        "resource 1 1 0 16 1 instances", "fragment-targets 0"])
        if any(line.startswith("resource 2 ") for line in lines):
            raise RuntimeError("inactive fragment resources leaked into the vertex-only entry")

        lines, ops, caps, _ = emit(args.coax, directory, "fine", "--fragment", "reflection_fine_fs")
        require(lines, ["reflection 2", "tree-version 3", "derivatives 1 2", "control 1 0"])
        if not {210, 211, 212}.issubset(ops) or caps.count(51) != 1:
            raise RuntimeError("fine opcodes/capability missing")

        lines, ops, caps, extensions = emit(args.coax, directory, "helper", "--fragment", "reflection_helper_fs")
        require(lines, ["tree-version 3", "derivatives 1 2", "control 1 2"])
        if 5380 not in ops or 252 in ops or caps.count(5379) != 1:
            raise RuntimeError("helper conversion was replaced by termination")
        if extensions.count("SPV_EXT_demote_to_helper_invocation") != 1:
            raise RuntimeError("helper extension missing")
        lines, ops, caps, _ = emit(args.coax, directory, "array-size", "--fragment", "reflection_array_size_fs")
        require(lines, ["tree-version 2", "resource 0 2 1 0 2 layers", "derivatives 1 0", "control 1 0"])
        if 103 not in ops or caps.count(50) != 1:
            raise RuntimeError("array dimension aliases lost image-query lowering")
        bindings = directory / "array.bindings"
        bindings.write_text("layers 0 0\n")
        wgsl = subprocess.run([args.coax, "wgsl", FIXTURE, "--fragments", "reflection_array_size_fs", "--bindings", str(bindings)],
                              cwd=ROOT, capture_output=True, text=True, timeout=180)
        if wgsl.returncode or "textureDimensions(" not in wgsl.stdout or ").x" not in wgsl.stdout or ").y" not in wgsl.stdout:
            raise RuntimeError("array dimensions were not emitted as WGSL queries: " + wgsl.stderr)
        bindings.write_text("")
        wgsl = subprocess.run([args.coax, "wgsl", FIXTURE, "--fragments", "reflection_fine_fs,reflection_helper_fs", "--bindings", str(bindings)],
                              cwd=ROOT, capture_output=True, text=True, timeout=180)
        if wgsl.returncode or not all(token in wgsl.stdout for token in ["dpdxFine(", "dpdyFine(", "fwidthFine(", "discard;"]):
            raise RuntimeError("fine/helper WGSL bridge changed: " + wgsl.stderr)
        print("reflection2, legacy report/tree2, fine/helper tree3 and vertex-only entry pass")


if __name__ == "__main__":
    main()
