#!/usr/bin/env python3
"""Independent shader-package inventory and planted build-failure controls.

This checks concrete emitted products and publication. It does not certify
shader arithmetic or GPU images; the reference/GPU qualification owns those.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
from pathlib import Path
import re
import shutil
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
PASS_NAMES = "background shadow caster grass surface scene water_transmit water_emit particle_over particle bloom post hud".split()


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def spirv(data: bytes) -> dict:
    assert len(data) >= 20 and len(data) % 4 == 0
    words = struct.unpack("<" + str(len(data) // 4) + "I", data)
    assert words[0] == 0x07230203 and words[1] <= 0x00010300 and words[3] > 0 and words[4] == 0
    types, decorations, variables, stages, instructions = {}, {}, {}, {}, []
    at = 5
    while at < len(words):
        count, opcode = words[at] >> 16, words[at] & 65535
        assert 1 <= count <= len(words) - at
        operands = words[at + 1:at + count]
        instructions.append((opcode, operands))
        if 19 <= opcode <= 33:
            types[operands[0]] = (opcode, operands[1:])
        elif opcode == 71:
            decorations.setdefault(operands[0], {})[operands[1]] = operands[2:]
        elif opcode == 59:
            variables[operands[1]] = (operands[0], operands[2])
        elif opcode == 15:
            # Entry-point names occupy NUL-terminated padded u32 words.
            end = 2
            while b"\0" not in struct.pack("<I", operands[end]):
                end += 1
            stages[operands[0]] = operands[end + 1:]
        at += count
    assert at == len(words) and 0 in stages and set(stages) <= {0, 4}

    def value_type(pointer: int) -> tuple[int, int]:
        opcode, values = types[pointer]
        assert opcode == 32
        opcode, values = types[values[1]]
        width = 1
        if opcode == 23:
            width = values[1]
            opcode, values = types[values[0]]
        if opcode == 22:
            assert values == (32,)
            return 2, width
        assert opcode == 21 and values[0] == 32
        return (3 if values[1] else 4), width

    attributes, targets, depth, resources = [], [], False, []
    for identity in stages[0]:
        pointer, storage = variables[identity]
        decor = decorations.get(identity, {})
        if storage == 1 and 30 in decor:
            scalar, width = value_type(pointer)
            attributes.append((decor[30][0], scalar, width))
    for identity in stages.get(4, ()):
        pointer, storage = variables[identity]
        decor = decorations.get(identity, {})
        if storage == 3:
            if 30 in decor:
                targets.append(decor[30][0])
            depth |= decor.get(11) == (22,)
    for identity, (pointer, storage) in variables.items():
        decor = decorations.get(identity, {})
        if 33 not in decor:
            continue
        assert decor[34] == (0,)
        opcode, values = types[pointer]
        assert opcode == 32
        opcode, values = types[values[1]]
        if storage == 12:
            assert opcode == 30 and len(values) == 1
            array = values[0]
            assert types[array][0] == 29
            resources.append((decor[33][0], 1, 0, decorations[array][6][0]))
        else:
            assert storage == 0
            if opcode == 25:
                resources.append((decor[33][0], 2, 2 if values[2] else (1 if values[3] else 0), 0))
            else:
                assert opcode == 26
                resources.append((decor[33][0], 3, None, 0))
    opcodes = {opcode for opcode, _ in instructions}
    assert sorted(targets) == list(range(len(targets)))
    return dict(attributes=sorted(attributes), fragment_targets=len(targets), fragment_depth=depth,
                resources=sorted(resources), native_int64=(17, (11,)) in instructions,
                derivatives=(bool(opcodes & {207, 208, 209, 213, 214, 215}) | (2 if opcodes & {210, 211, 212} else 0)),
                control=(bool(opcodes & {252}) | (2 if 5380 in opcodes else 0)), has_fragment=4 in stages, words=len(words))


def verify_package(directory: Path, snapshot: dict) -> dict:
    package = json.loads((directory / "render.json").read_text())
    assert package["schema"] == "aethyr.render.package.v1"
    profile = snapshot["profiles"][package["profile_index"]]
    assert package["slug"] == profile["slug"] and package["app"] == profile["module"]
    passes = snapshot["passes"][profile["pass_first"]:profile["pass_first"] + profile["pass_count"]]
    passes += snapshot["passes"][profile["particle_pass_first"]:profile["particle_pass_first"] + profile["particle_pass_count"]]
    expected = {PASS_NAMES.index(row["pass_id"]): row for row in passes}
    if profile["features"]["media"]:
        expected[13] = dict(binding_set=2, vertex="media_vs", fragment="media_fs", wgsl_vertex="vs", wgsl_fragment="fs", uses=[
            dict(resource=0, slot=0, native_kind=1, shape=0, stride=4, stages=3),
            dict(resource=1, slot=1, native_kind=2, shape=0, stride=0, stages=2),
            dict(resource=2, slot=2, native_kind=3, shape=0, stride=0, stages=2)])
    assert len(package["entries"]) == len(expected)
    assert {row["entry_index"] for row in package["entries"]} == set(expected)
    modules = {}
    for row in package["modules"]:
        assert row["binding_set"] not in modules
        assert Path(row["wgsl_file"]).name == row["wgsl_file"]
        data = (directory / row["wgsl_file"]).read_bytes()
        assert sha(data) == row["wgsl_sha256"]
        modules[row["binding_set"]] = data.decode()
    assert set(modules) == {row["binding_set"] for row in expected.values()}
    reports = []
    for row in package["entries"]:
        plan = expected[row["entry_index"]]
        for key in ("binding_set", "vertex", "fragment", "wgsl_vertex", "wgsl_fragment"):
            assert row[key] == plan[key], (row["entry_index"], key)
        for stage in ("vertex", "fragment"):
            alias = row["wgsl_" + stage]
            if stage == "fragment" and not row["fragment"]:
                assert row["entry_index"] == 2 and alias == "", "the catalog's vertex-only caster"
                continue
            assert re.search(r"@" + stage + r"\s+fn\s+" + re.escape(alias) + r"\s*\(", modules[row["binding_set"]]), alias
        first = profile["binding_first"] if row["binding_set"] == 0 else profile["particle_binding_first"]
        media = ["u", "media_image", "media_sampler"]
        expected_uses = []
        for use in plan["uses"]:
            copy = {key: value for key, value in use.items() if key != "resource"}
            copy["name"] = media[use["resource"]] if row["binding_set"] == 2 else snapshot["bindings"][first + use["resource"]]["name"]
            expected_uses.append(copy)
        assert row["uses"] == expected_uses
        assert row["reflection_schema"] == 2 and row["tree_version"] in (2, 3)
        assert row["derivatives"] in (0, 2) and row["control"] in (0, 2)
        assert Path(row["spirv_file"]).name == row["spirv_file"]
        data = (directory / row["spirv_file"]).read_bytes()
        assert sha(data) == row["spirv_sha256"]
        assert Path(row["tree_file"]).name == row["tree_file"]
        tree_data = (directory / row["tree_file"]).read_bytes()
        assert sha(tree_data) == row["tree_sha256"]
        tokens = tree_data.split()
        assert tokens and all(token.isdigit() and int(token) <= 0xffffffff for token in tokens)
        assert int(tokens[0]) == row["tree_version"]
        facts = spirv(data)
        assert facts["has_fragment"] == bool(row["fragment"])
        assert facts["native_int64"] == row["native_int64"]
        assert facts["fragment_targets"] == row["fragment_targets"] and facts["fragment_depth"] == row["fragment_depth"]
        assert facts["attributes"] == [(x["location"], x["scalar"], x["width"]) for x in row["attributes"]]
        offset = 0
        for attribute in row["attributes"]:
            # The frozen catalog's HUD layouts are densely packed scalar/vector
            # fields. This is a check of those concrete layouts, not all structs.
            assert attribute["offset"] == offset
            offset += attribute["width"] * 4
        assert offset == row["vertex_stride"]
        assert len(facts["resources"]) == len(row["uses"])
        for fact, use in zip(facts["resources"], row["uses"]):
            assert fact[0] == use["slot"] and fact[1] == use["native_kind"] and fact[3] == use["stride"]
            assert fact[2] is None or fact[2] == use["shape"]
        assert facts["derivatives"] == row["derivatives"] and facts["control"] == row["control"]
        reports.append(dict(entry_index=row["entry_index"], spirv_words=facts["words"], spirv_sha256=sha(data), tree_words=len(tokens), tree_sha256=sha(tree_data)))
    inputs = package["source_inputs"]
    assert inputs and len({x["path"] for x in inputs}) == len(inputs)
    for row in inputs:
        assert sha((ROOT / row["path"]).read_bytes()) == row["sha256"]
    return dict(profile=package["slug"], modules=len(modules), entries=reports, manifest_sha256=sha((directory / "render.json").read_bytes()))


def fake_compiler(path: Path, compiler: Path, fault: str, dependency: Path) -> None:
    script = '''#!/usr/bin/env python3
import json,subprocess,sys
from pathlib import Path
COMPILER=__COMPILER__
FAULT=__FAULT__
DEP=Path(__DEPENDENCY__)
r=subprocess.run([COMPILER,*sys.argv[1:]],capture_output=True)
out,err=r.stdout,r.stderr
if sys.argv[1]=='deps':
 d=json.loads(out);d['sources'].append(str(DEP));out=json.dumps(d).encode()
if sys.argv[1]=='spirv' and r.returncode==0:
 if FAULT=='name':err=err.replace(b' 2 u\\n',b' 2 changed\\n').replace(b' 3 u\\n',b' 3 changed\\n')
 if FAULT=='target':err=err.replace(b'fragment-targets 1',b'fragment-targets 2')
 if FAULT=='generic':err=err.replace(b'derivatives 1 0',b'derivatives 1 1')
 if FAULT=='framing':
  p=Path(sys.argv[sys.argv.index('-o')+1]);data=bytearray(p.read_bytes());data[20:24]=bytes(4);p.write_bytes(data)
 if FAULT=='source':DEP.write_text(DEP.read_text()+'changed\\n')
 if FAULT=='compiler':
  p=Path(__file__);p.write_text(p.read_text()+'\\n# changed\\n')
 if FAULT=='exit':sys.exit(31)
sys.stdout.buffer.write(out);sys.stderr.buffer.write(err);sys.exit(r.returncode)
'''
    script = script.replace("__COMPILER__", repr(str(compiler))).replace("__FAULT__", repr(fault)).replace("__DEPENDENCY__", repr(str(dependency)))
    path.write_text(script)
    path.chmod(0o755)


def prepare_builder(work: Path, compiler: Path, run) -> tuple[Path, Path]:
    """Retain the exact Coax/native inputs used to compile the effect adapter."""
    directory = work / "builder"
    directory.mkdir(exist_ok=True)
    native = ROOT / "coax/platform"
    roots = [HERE.parent / "build.coax", ROOT / "coax/lib/crypto/entropy.coax"]
    inputs = set()
    for source in roots:
        deps = json.loads(run([compiler, "deps", source, "--library"]).stdout)
        inputs.update(Path(path) for key in ("sources", "descriptors") for path in deps[key])
    effects = [native / (name + ".c") for name in ("tool-native", "process-native", "store-file-native", "socket-native", "entropy-native", "entropy-foreign")]
    inputs.update(effects)
    inputs.update(native.rglob("*.h"))
    before = {str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in sorted(inputs)}
    compiler_hash = sha(compiler.read_bytes())
    entropy, source = directory / "entropy.c", directory / "build.c"
    entropy.write_bytes(run([compiler, "c", roots[1], "--library", "--header", directory / "entropy-policy.h"]).stdout)
    source.write_bytes(run([compiler, "c", roots[0], "--library"]).stdout)
    builder = directory / "build"
    flags = ["-D_DARWIN_C_SOURCE"] if platform.system() == "Darwin" else ["-D_DEFAULT_SOURCE"]
    libraries = ["-framework", "Security"] if platform.system() == "Darwin" else ["-lpthread"]
    command = ["clang", *flags, "-std=c11", "-O2", "-I", directory, "-I", native,
               "-o", builder, source, *effects, entropy, *libraries, "-lm"]
    run(command)
    assert before == {str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in sorted(inputs)}, "builder source changed during compilation"
    assert compiler_hash == sha(compiler.read_bytes()), "compiler changed during builder compilation"
    receipt = directory / "qualification.json"
    receipt.write_text(json.dumps(dict(schema="aethyr.render.builder.native.v1", compiler_sha256=compiler_hash,
                                      builder_sha256=sha(builder.read_bytes()), source_sha256=before,
                                      generated_sha256={path.name: sha(path.read_bytes()) for path in (source, entropy, directory / "entropy-policy.h")},
                                      command=[str(x) for x in command]), indent=2) + "\n")
    return builder, receipt


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", default="the-grid")
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--faults", action="store_true")
    parser.add_argument("--cache-controls", action="store_true")
    parser.add_argument("--builder", type=Path,
                        help="already compiled Coax builder; avoids rebuilding shared tooling during parallel qualification")
    parser.add_argument("--builder-receipt", type=Path,
                        help="source/compiler-bound receipt created with the supplied builder")
    args = parser.parse_args()
    start = time.monotonic()
    qualifier_hash = sha(Path(__file__).read_bytes())
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    compiler = ROOT / "coax/bin/coax"

    def run(argv: list, *, success=True) -> subprocess.CompletedProcess:
        left = 830 - (time.monotonic() - start)
        assert left > 0, "qualification command budget exhausted"
        result = subprocess.run([str(x) for x in argv], cwd=ROOT, capture_output=True, timeout=left)
        if success and result.returncode:
            raise RuntimeError(result.stderr.decode(errors="replace")[-2000:])
        return result

    version = run([compiler, "version"]).stdout.decode().strip()
    current = run([compiler, "source-digest", ROOT]).stdout.decode().strip()
    recorded = (compiler.parent / "source-digest").read_text().strip()
    assert current == recorded, "rebuild the compiler before qualification"
    compiler_hash = sha(compiler.read_bytes())
    if args.builder:
        builder = args.builder.resolve()
        assert args.builder_receipt, "a direct builder requires its source/compiler-bound receipt"
        builder_receipt = args.builder_receipt.resolve()
    else:
        assert not args.builder_receipt, "builder receipt requires --builder"
        builder, builder_receipt = prepare_builder(work, compiler, run)
    builder_hash = sha(builder.read_bytes())
    prepared = json.loads(builder_receipt.read_text())
    assert prepared["compiler_sha256"] == compiler_hash and prepared["builder_sha256"] == builder_hash
    assert all(sha((ROOT / path).read_bytes()) == digest for path, digest in prepared["source_sha256"].items()), "builder source no longer matches"
    builder_receipt_hash = sha(builder_receipt.read_bytes())
    snapshot = json.loads((HERE / "catalog-6445ee0050.json").read_text())
    profiles = run([builder, "--profiles"]).stdout.decode().splitlines()
    assert profiles == [row["slug"] for row in snapshot["profiles"]]
    profile_rows = run([builder, "--profile-table"]).stdout.decode().splitlines()
    assert profile_rows == [f"{index} {slug}" for index, slug in enumerate(profiles)]
    app_rows = run([builder, "--app-table"]).stdout.decode().splitlines()
    assert app_rows == [f"{index} {row['slug']} {row['module']}"
                        for index, row in enumerate(snapshot["profiles"])]
    output = work / args.profile
    result = run([builder, args.profile, output])
    (work / (args.profile + ".build.log")).write_bytes(result.stdout + result.stderr)
    report = verify_package(output, snapshot)
    manifest = output / "render.json"
    before, before_mtime = manifest.read_bytes(), manifest.stat().st_mtime_ns
    reused = run([builder, args.profile, output])
    assert b"reused " in reused.stdout and manifest.read_bytes() == before and manifest.stat().st_mtime_ns == before_mtime
    cache_controls = []
    # A matching source key never excuses a corrupted or missing product.
    for kind, table, field in (("wgsl", "modules", "wgsl_file"), ("spirv", "entries", "spirv_file"), ("tree", "entries", "tree_file")):
        if not (args.cache_controls or args.faults):
            break
        package = json.loads(before)
        product = output / package[table][0][field]
        original = product.read_bytes()
        product.write_bytes(original + b"corrupt")
        repaired = run([builder, args.profile, output])
        assert b"reused " not in repaired.stdout and product.read_bytes() == original and manifest.read_bytes() == before
        cache_controls.append(kind)
    faults = []
    if args.faults:
        assert args.profile == "the-grid", "the planted controls use this declared two-pipeline profile"
        for fault in ("name", "target", "generic", "framing", "source", "compiler", "exit"):
            dest = work / ("fault-" + fault)
            dest.mkdir(exist_ok=True)
            previous = b'{"previous":"must remain byte identical"}\n'
            (dest / "render.json").write_bytes(previous)
            dependency = work / (fault + "-input.txt")
            dependency.write_text("source snapshot\n")
            fake = work / ("compiler-" + fault)
            fake_compiler(fake, compiler, fault, dependency)
            result = run([builder, args.profile, dest, "--coax", fake], success=False)
            (work / (fault + ".log")).write_bytes(result.stdout + result.stderr)
            assert result.returncode != 0, ("undetected compiler fault", fault)
            assert (dest / "render.json").read_bytes() == previous, ("partial publication", fault)
            faults.append(fault)
    assert compiler_hash == sha(compiler.read_bytes())
    assert builder_hash == sha(builder.read_bytes()), "builder changed during qualification"
    assert builder_receipt_hash == sha(builder_receipt.read_bytes()), "builder receipt changed during qualification"
    assert qualifier_hash == sha(Path(__file__).read_bytes()), "qualifier changed during qualification"
    assert all(sha((ROOT / path).read_bytes()) == digest for path, digest in prepared["source_sha256"].items()), "builder source changed during qualification"
    verify_package(output, snapshot)  # checks the final source fence without rebuilding
    receipt = dict(schema="aethyr.render.build.qualification.v1", status="pass", compiler_version=version,
                   compiler_sha256=compiler_hash, compiler_source_digest=current, package=report,
                   builder_sha256=builder_hash,
                   builder_receipt_sha256=builder_receipt_hash,
                   failure_controls=faults, cache_product_controls=cache_controls, enumerated_profiles=len(profiles), elapsed_seconds=round(time.monotonic() - start, 3),
                   qualifier_sha256=qualifier_hash,
                   scope="Actual emitted module/pipeline inventory, independent SPIRV framing/interface/resource/capability observations, catalog resource contract, source closure and atomic manifest publication. Not GPU numerical/image qualification.")
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
