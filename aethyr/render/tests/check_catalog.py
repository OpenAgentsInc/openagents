#!/usr/bin/env python3
"""Normal-C catalog inventory against the frozen pre-migration declarations.

This verifies transport/data preservation, not shader arithmetic or GPU images.
The separate emitter compares these declarations to checked shader reflection.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
PASSES = ["background", "shadow", "caster", "grass", "surface", "scene",
          "water_transmit", "water_emit", "particle_over", "particle", "bloom", "post", "hud"]
KINDS = ["uniform", "storage", "image2d", "image_array", "depth", "sampler", "comparison"]
FIELDS = {
    "RenderCatalog": "valid listed retired slug app page assets uniform_vec4s vertex_stride features binding_first binding_count pass_first pass_count particle_binding_first particle_binding_count particle_pass_first particle_pass_count".split(),
    "RenderFeatures": "scene shadow casters grass surface water water_copy post post_depth traced_depth particles particle_depth particle_over media".split(),
    "RenderBinding": "name kind group binding stride uniform_vec4s".split(),
    "RenderEntry": "valid source vertex fragment wgsl_vertex wgsl_fragment binding_set".split(),
    "RenderUse": "resource slot native_kind shape stride stages".split(),
}
FAULTS = ["asset", "feature", "binding", "pipeline", "scope", "slot", "kind"]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected_inventory(snapshot: dict) -> list:
    result = []
    for p in snapshot["profiles"]:
        row = {k: v for k, v in p.items() if k not in ("module", "feature_bits")}
        row.update(valid=True, app=p["module"])
        row["bindings"] = []
        for prefix in ("", "particle_"):
            first, n = p[prefix + "binding_first"], p[prefix + "binding_count"]
            values = []
            for b in snapshot["bindings"][first:first + n]:
                values.append(dict(b, kind=KINDS.index(b["kind"])))
            row["bindings"].append(values)
        all_passes = snapshot["passes"][p["pass_first"]:p["pass_first"] + p["pass_count"]]
        all_passes += snapshot["passes"][p["particle_pass_first"]:p["particle_pass_first"] + p["particle_pass_count"]]
        row["pipelines"] = []
        for name in PASSES:
            matches = [v for v in all_passes if v["pass_id"] == name]
            assert len(matches) <= 1, (p["slug"], name)
            if matches:
                v = matches[0]
                row["pipelines"].append({**{k: v[k] for k in FIELDS["RenderEntry"] if k != "valid"}, "valid": True, "uses": v["uses"]})
            else:
                row["pipelines"].append(dict(valid=False, source="", vertex="", fragment="", wgsl_vertex="", wgsl_fragment="", binding_set=0, uses=[]))
        result.append(row)
    return result


def header_inventory(header: str) -> None:
    for name, fields in FIELDS.items():
        m = re.search(r"struct coax_t_" + name + r" \{([^}]+)\};", header)
        assert m, name
        actual = [re.search(r"\b(\w+)\s*$", d.strip())[1] for d in m[1].split(";") if d.strip()]
        assert actual == fields, (name, actual, fields)


def main() -> None:
    a = argparse.ArgumentParser()
    a.add_argument("--work-dir", type=Path, required=True)
    a.add_argument("--receipt", type=Path, required=True)
    a.add_argument("--modes", nargs="+", choices=["debug", "release", "sanitized"], default=["debug", "release", "sanitized"])
    a.add_argument("--explore-stale-compiler", action="store_true")
    args = a.parse_args()
    started = time.monotonic()
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    compiler = ROOT / "coax/bin/coax"
    records = []

    def run(argv: list, *, env=None) -> subprocess.CompletedProcess:
        remaining = 830 - (time.monotonic() - started)
        if remaining <= 0:
            raise TimeoutError("catalog qualification reached its bounded command deadline")
        r = subprocess.run([str(x) for x in argv], cwd=ROOT, env=env, capture_output=True, timeout=remaining)
        if r.returncode:
            raise RuntimeError(f"command failed ({r.returncode}): {argv!r}\n" + r.stderr.decode(errors="replace")[-2000:])
        return r

    compiler_hash = sha(compiler)
    version = run([compiler, "version"]).stdout.decode().strip()
    recorded_digest = (compiler.parent / "source-digest").read_text().strip()
    current_digest = run([compiler, "source-digest", ROOT]).stdout.decode().strip()
    stale = recorded_digest != current_digest
    if stale and not args.explore_stale_compiler:
        raise RuntimeError("compiler recorded/current source digests differ; rebuild before final qualification")
    fixture = HERE / "catalog-native.coax"
    deps = json.loads(run([compiler, "deps", fixture]).stdout)
    source_paths = [Path(x) for x in deps["sources"]] + [HERE / "catalog-native.c", HERE / "catalog-6445ee0050.json", Path(__file__).resolve()]
    closure = {str(p.relative_to(ROOT)): sha(p) for p in source_paths}
    if args.receipt.exists():
        old = json.loads(args.receipt.read_text())
        if old.get("source_sha256") == closure and old.get("compiler_sha256") == compiler_hash:
            raise RuntimeError("this unchanged source/compiler inventory is already qualified; retain its receipt")
    snapshot = json.loads((HERE / "catalog-6445ee0050.json").read_text())
    # Authenticate the historical claim against git's immutable source blobs.
    for path, digest in snapshot["sources_sha256"].items():
        data = run(["git", "show", snapshot["reference_commit"] + ":" + path]).stdout
        assert hashlib.sha256(data).hexdigest() == digest, ("reference source differs", path)
    expected = expected_inventory(snapshot)
    # Current manifest identity is checked independently of the Coax catalog.
    manifest = json.loads((ROOT / "apps/openagents-web/realms.json").read_text())
    live = [w["realm"] for w in manifest["worlds"] if "realm" in w]
    assert len(live) == 16
    for got, old in zip(live, snapshot["profiles"][:16]):
        assert (got["slug"], got["module"], got.get("page", 0), "\n".join(got.get("assets", []))) == (old["slug"], old["module"], old["page"], old["assets"])

    emitted = {}
    for mode in args.modes:
        release = mode == "release"
        if release not in emitted:
            part = work / ("wrapping" if release else "checked")
            part.mkdir(exist_ok=True)
            header = part / "catalog.h"
            cmd = [compiler, "c", fixture, "--library", "--header", header] + (["--release"] if release else [])
            r = run(cmd)
            (part / "emission.log").write_bytes(r.stderr)
            this_stale = b"compiler is out of date" in r.stderr
            stale |= this_stale
            if this_stale and not args.explore_stale_compiler:
                raise RuntimeError("compiler differs from current source; rebuild before final qualification")
            source = part / "catalog.c"
            source.write_bytes(r.stdout)
            header_inventory(header.read_text())
            emitted[release] = (source, header)
        source, header = emitted[release]
        exe = work / ("catalog-" + mode)
        flags = ["-O2"] if release else ["-O0", "-g"]
        if mode == "sanitized":
            flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        run(["clang", "-std=c11", *flags, "-I", header.parent, source, HERE / "catalog-native.c", "-lm", "-o", exe])
        env = dict(os.environ)
        if mode == "sanitized":
            env["ASAN_OPTIONS"] = "detect_leaks=0:halt_on_error=1"
            env["UBSAN_OPTIONS"] = "halt_on_error=1"
        r = run([exe], env=env)
        transcript = work / (mode + ".json")
        transcript.write_bytes(r.stdout)
        assert json.loads(r.stdout) == expected, (mode, "catalog inventory differs")
        detected = []
        for fault in FAULTS:
            broken = run([exe, fault], env=env)
            assert json.loads(broken.stdout) != expected, (mode, "undetected producer fault", fault)
            detected.append(fault)
        records.append(dict(mode=mode, profiles=18, unique_logical_bindings=len(snapshot["bindings"]), unique_pipelines=len(snapshot["passes"]), unique_binding_uses=sum(len(x["uses"]) for x in snapshot["passes"]), producer_faults_detected=detected, generated_c_sha256=sha(source), generated_header_sha256=sha(header), executable_sha256=sha(exe), transcript=str(transcript), transcript_sha256=sha(transcript)))
    assert sha(compiler) == compiler_hash, "compiler changed during qualification"
    assert closure == {str(p.relative_to(ROOT)): sha(p) for p in source_paths}, "source changed during qualification"
    receipt = dict(schema="aethyr.render.catalog.inventory.v1", status="exploratory" if stale else "pass", reference_commit=snapshot["reference_commit"], compiler=str(compiler), compiler_version=version, compiler_sha256=compiler_hash, compiler_recorded_source_digest=recorded_digest, compiler_current_source_digest=current_digest, compiler_current_sources_match=not stale, source_sha256=closure, header_fields_checked=FIELDS, modes=records, elapsed_seconds=round(time.monotonic() - started, 3), scope="Complete normal-C catalog data/ABI inventory and producer-loss controls. Shader reflection, shader arithmetic, GPU rendering and image parity are separate qualifications.")
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"status": receipt["status"], "profiles": 18, "modes": [x["mode"] for x in records], "receipt": str(args.receipt)}))


if __name__ == "__main__":
    main()
