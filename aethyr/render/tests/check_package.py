#!/usr/bin/env python3
"""Normal-C package transport, borrowed ownership and actual byte admission.

The independent oracle is Python's JSON tree and the supplied concrete bytes.
This qualifies the package boundary, not shader arithmetic or GPU images.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
FIELDS = {
    "RenderPackage": "status profile_index slug app module_count entry_count modules entries".split(),
    "RenderPackageModule": "present binding_set wgsl_file wgsl_sha256".split(),
    "RenderPackageEntry": "present binding_set vertex fragment wgsl_vertex wgsl_fragment spirv_file spirv_sha256 tree_file tree_sha256 native_int64 reflection".split(),
    "RenderBuildReflection": "ok schema tree_version derivatives control use_count uses vertex_stride attribute_count attributes fragment_targets fragment_depth".split(),
    "RenderReflectedUse": "name slot native_kind shape stride stages".split(),
    "RenderVertexInput": "location scalar width offset".split(),
}
FAULTS = ["profile", "module_count", "entry_count", "resource", "attribute", "borrow"]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def header_inventory(text: str) -> None:
    for name, fields in FIELDS.items():
        record = re.search(r"struct coax_t_" + name + r" \{([^}]+)\};", text)
        assert record, name
        actual = [re.search(r"\b(\w+)\s*$", x.strip())[1] for x in record[1].split(";") if x.strip()]
        assert actual == fields, (name, actual, fields)


def expected(package: dict, *, payload=False) -> dict:
    modules = [{"present": False} for _ in range(3)]
    for row in package["modules"]:
        modules[row["binding_set"]] = dict(row, present=True)
    entries = [dict(entry_index=i, present=False) for i in range(14)]
    for row in package["entries"]:
        entries[row["entry_index"]] = dict(row, present=True, reflection_ok=True, use_count=len(row["uses"]), attribute_count=len(row["attributes"]))
    result = dict(parse=0, admit=0, entry_view_count=14, guards=True, borrowed_strings=True,
                  profile_index=package["profile_index"], slug=package["slug"], app=package["app"],
                  module_count=len(package["modules"]), entry_count=len(package["entries"]), modules=modules, entries=entries)
    if payload:
        result.update(module_bytes=0, spirv_bytes=0, tree_bytes=0)
    return result


def negative_manifests(package: dict) -> list[tuple[str, bytes, int, int]]:
    """Malformed transport and well-formed declarations that violate the catalog."""
    cases = []
    def changed(name, path, value, parse, admit):
        candidate = copy.deepcopy(package)
        target = candidate
        for key in path[:-1]:
            target = target[key]
        target[path[-1]] = value
        cases.append((name, json.dumps(candidate).encode(), parse, admit))
    cases.append(("duplicate-root-key", b'{"profile_index":0,' + json.dumps(package).encode()[1:], 1, 1))
    cases.append(("truncated-json", json.dumps(package).encode()[:-2], 1, 1))
    changed("unknown-schema", ["schema"], "aethyr.render.package.v0", 1, 1)
    changed("fractional-profile", ["profile_index"], 1.5, 1, 1)
    changed("unsafe-product-path", ["modules", 0, "wgsl_file"], "../outside.wgsl", 1, 1)
    changed("uppercase-hash", ["entries", 0, "tree_sha256"], "A" * 64, 1, 1)
    changed("null-fragment", ["entries", 0, "fragment"], None, 1, 1)
    changed("negative-stride", ["entries", 0, "vertex_stride"], -1, 1, 1)
    changed("wrong-slug", ["slug"], "other-realm", 0, 3)
    changed("wrong-entry-name", ["entries", 0, "vertex"], "other_vertex", 0, 3)
    changed("wrong-resource-slot", ["entries", 0, "uses", 0, "slot"], package["entries"][0]["uses"][0]["slot"] + 1, 0, 3)
    changed("wrong-target-count", ["entries", 0, "fragment_targets"], package["entries"][0]["fragment_targets"] + 1, 0, 3)
    changed("generic-derivative", ["entries", 0, "derivatives"], 1, 0, 4)
    changed("generic-discard", ["entries", 0, "control"], 1, 0, 4)
    return cases


class QualificationStopped(RuntimeError):
    pass


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--packages", type=Path, required=True)
    parser.add_argument("--profiles", nargs="+")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--modes", nargs="+", choices=["debug", "release", "sanitized"], default=["debug", "release", "sanitized"])
    args = parser.parse_args()
    start = time.monotonic()
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    compiler = ROOT / "coax/bin/coax"
    child = None

    def stopped(signum, frame):
        # RuntimeError deliberately avoids InterruptedError/OSError, which a
        # Python selector may retry inside subprocess.communicate.
        raise QualificationStopped("qualification interrupted by signal " + str(signum))

    signal.signal(signal.SIGTERM, stopped)
    signal.signal(signal.SIGINT, stopped)

    def run(argv, *, env=None):
        nonlocal child
        left = 830 - (time.monotonic() - start)
        if left <= 0:
            raise TimeoutError("package qualification command deadline")
        child = subprocess.Popen([str(x) for x in argv], cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            out, err = child.communicate(timeout=left)
            if child.returncode:
                raise RuntimeError(f"command failed ({child.returncode}): {argv!r}\n" + err.decode(errors="replace")[-2000:])
            return out, err
        except BaseException:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.communicate(timeout=3)
            except BaseException:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.communicate()
            raise
        finally:
            child = None

    version = run([compiler, "version"])[0].decode().strip()
    fingerprint = run([compiler, "source-digest", ROOT])[0].decode().strip()
    assert fingerprint == (compiler.parent / "source-digest").read_text().strip(), "source-matching compiler required"
    compiler_hash = sha(compiler)
    fixture = HERE / "package-native.coax"
    deps = json.loads(run([compiler, "deps", fixture, "--library"])[0])
    paths = [Path(x) for x in deps["sources"]] + [HERE / "package-native.c", Path(__file__)]
    closure = {str(x.relative_to(ROOT)): sha(x) for x in paths}
    snapshot = json.loads((HERE / "catalog-6445ee0050.json").read_text())
    profiles = args.profiles or [row["slug"] for row in snapshot["profiles"]]
    packages = []
    for slug in profiles:
        source = args.packages.resolve() / slug
        dest = work / "inputs" / slug
        dest.mkdir(parents=True, exist_ok=True)
        raw = (source / "render.json").read_bytes()
        package = json.loads(raw)
        profile = next(i for i, row in enumerate(snapshot["profiles"]) if row["slug"] == slug)
        assert package["profile_index"] == profile and package["slug"] == slug
        assert package["app"] == snapshot["profiles"][profile]["module"]
        (dest / "render.json").write_bytes(raw)
        for rows, file_key, hash_key in ((package["modules"], "wgsl_file", "wgsl_sha256"), (package["entries"], "spirv_file", "spirv_sha256"), (package["entries"], "tree_file", "tree_sha256")):
            for row in rows:
                assert Path(row[file_key]).name == row[file_key]
                data = (source / row[file_key]).read_bytes()
                assert hashlib.sha256(data).hexdigest() == row[hash_key]
                (dest / row[file_key]).write_bytes(data)
        packages.append((package, dest))
    modes, emitted = [], {}
    for mode in args.modes:
        release = mode == "release"
        if release not in emitted:
            directory = work / ("wrapping" if release else "checked")
            directory.mkdir(exist_ok=True)
            header = directory / "package-native.h"
            source = directory / "package-native.c"
            out, err = run([compiler, "c", fixture, "--library", "--header", header] + (["--release"] if release else []))
            source.write_bytes(out)
            (directory / "emission.log").write_bytes(err)
            header_inventory(header.read_text())
            emitted[release] = source, header
        source, header = emitted[release]
        executable = work / ("package-" + mode)
        flags = ["-O2"] if release else ["-O0", "-g"]
        if mode == "sanitized":
            flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        run(["clang", "-std=c11", "-Wall", "-Wextra", *flags, "-I", header.parent, source, HERE / "package-native.c", "-lm", "-o", executable])
        env = dict(os.environ)
        if mode == "sanitized":
            env.update(ASAN_OPTIONS="detect_leaks=0:detect_stack_use_after_return=1:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
        observations = []
        for package, directory in packages:
            manifest = directory / "render.json"
            base = [executable, manifest, package["profile_index"]]
            value = json.loads(run(base, env=env)[0])
            assert value == expected(package), (mode, package["slug"], "complete parsed receipt")
            observations.append(value)
            for entry in package["entries"]:
                module = next(row for row in package["modules"] if row["binding_set"] == entry["binding_set"])
                value = json.loads(run(base + ["", directory / module["wgsl_file"], directory / entry["spirv_file"], entry["entry_index"], entry["binding_set"], directory / entry["tree_file"]], env=env)[0])
                assert value == expected(package, payload=True), (mode, package["slug"], entry["entry_index"], "actual fetched bytes")
                observations.append(value)
            short = json.loads(run(base + ["short"], env=env)[0])
            assert short == dict(parse=2, admit=2, entry_view_count=0, guards=True, borrowed_strings=True)
            observations.append(short)
        # Planted transport losses are checked against the whole independent
        # receipt, not just against a convenient nonzero status.
        package, directory = packages[0]
        for fault in FAULTS:
            value = json.loads(run([executable, directory / "render.json", package["profile_index"], fault], env=env)[0])
            assert value != expected(package), ("undetected producer loss", mode, fault)
        negative_controls = []
        for name, raw, parsed, admitted in negative_manifests(package):
            path = work / ("negative-" + name + ".json")
            path.write_bytes(raw)
            value = json.loads(run([executable, path, package["profile_index"]], env=env)[0])
            if parsed:
                want = dict(parse=parsed, admit=admitted, entry_view_count=0, guards=True, borrowed_strings=True)
            else:
                want = expected(json.loads(raw))
                want["admit"] = admitted
            assert value == want, (mode, name, value, want)
            observations.append(value)
            negative_controls.append(name)
        # The exact admitted package must also reject each fetched product when
        # transport changes its bytes, before a host consumes it.
        entry = package["entries"][0]
        module = next(row for row in package["modules"] if row["binding_set"] == entry["binding_set"])
        originals = [directory / module["wgsl_file"], directory / entry["spirv_file"], directory / entry["tree_file"]]
        for index, key in enumerate(("module_bytes", "spirv_bytes", "tree_bytes")):
            corrupt = work / ("corrupt-" + key)
            corrupt.write_bytes(originals[index].read_bytes() + b"changed")
            inputs = list(originals)
            inputs[index] = corrupt
            value = json.loads(run([executable, directory / "render.json", package["profile_index"], "", inputs[0], inputs[1], entry["entry_index"], entry["binding_set"], inputs[2]], env=env)[0])
            want = expected(package, payload=True)
            want[key] = 5
            assert value == want, (mode, key, "corrupt fetched product")
            observations.append(value)
            negative_controls.append("corrupt-" + key)
        transcript = work / (mode + ".json")
        transcript.write_text(json.dumps(observations, indent=2) + "\n")
        modes.append(dict(mode=mode, observations=len(observations), producer_loss_controls=FAULTS,
                          negative_controls=negative_controls,
                          transcript_sha256=sha(transcript), generated_c_sha256=sha(source), header_sha256=sha(header), executable_sha256=sha(executable)))
    assert compiler_hash == sha(compiler), "compiler changed during qualification"
    assert closure == {str(x.relative_to(ROOT)): sha(x) for x in paths}, "source closure changed during qualification"
    receipt = dict(schema="aethyr.render.package.native.v1", status="pass", compiler_version=version,
                   compiler_sha256=compiler_hash, compiler_source_digest=fingerprint, source_sha256=closure,
                   profiles=profiles, header_fields_checked=FIELDS, modes=modes,
                   elapsed_seconds=round(time.monotonic() - start, 3),
                   scope="Normal-C package JSON/catalog admission, every record/string/attribute/resource field, borrowed JSON and caller-owned guarded entries, actual fetched WGSL/SPIRV/tree hash and SPIRV requirement admission. Supplied package bytes are pinned independently of their build provenance. Not shader arithmetic, full SPIRV validity, or GPU image qualification.")
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
