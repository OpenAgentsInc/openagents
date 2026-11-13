#!/usr/bin/env python3
# Firm reason: 2 (independent unbounded-integer interval and referenced-index audit of actual producer records and admitted domains; not an independent derivation of all geometry domains).
"""Actual retained-producer bytes through normal-C Coax geometry admission.

The independent Python audit checks the wire's addressed intervals/indices and
direct counts. Sharp Coax tests qualify the app-specific encoding rules; this
does not certify arbitrary shader arithmetic, assets' BVH truth or GPU pixels.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
FIELDS = {
    "FrameCounts": "width height hud_vertices vertex_stride draw_count water_draws grass_instances particle_instances surface_vertices caster_vertices caster_instances".split(),
    "RenderGeometryBuffer": "binding_set resource words".split(),
    "RenderGeometryResult": "status bad_draw bad_resource work direct".split(),
    "RenderDirectDomains": "hud_vertices grass_instances particle_instances surface_vertices caster_vertices caster_instances".split(),
    "RenderDrawDomain": "vertex_begin vertices vertex_alt_begin vertex_alt_end instance_begin instances indices".split(),
    "RenderFrameResult": "status bad_upload bad_draw bad_direct checked_indices admitted_draws".split(),
}


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def header_inventory(header: str) -> None:
    for name, fields in FIELDS.items():
        match = re.search(r"struct coax_t_" + name + r" \{([^}]+)\};", header)
        assert match, name
        actual = [re.search(r"\b(\w+)\s*$", value.strip())[1] for value in match[1].split(";") if value.strip()]
        assert actual == fields, (name, actual, fields)


def words(path: Path) -> tuple:
    data = path.read_bytes() if path.exists() else b""
    assert len(data) % 4 == 0, path
    return struct.unpack("<" + str(len(data) // 4) + "I", data)


def wire_audit(result: dict, counts: dict, draws: tuple, indices: tuple) -> None:
    """Arbitrary precision interval audit, independently of Coax's u64 checks."""
    assert result["geometry_status"] == result["frame_status"] == 0, result
    assert result["bad_draw"] == result["bad_resource"] == -1
    assert result["frame_bad_draw"] == result["frame_bad_upload"] == result["frame_bad_direct"] == -1
    assert len(result["domains"]) == counts["draws"]
    active = checked = 0
    for record, bounds in enumerate(result["domains"]):
        first, amount, encoded, instances = draws[record * 4:record * 4 + 4]
        if not amount or not instances:
            assert bounds == [0] * 7
            continue
        active += 1
        v0, v1, a0, a1, i0, i1, index_end = bounds
        assert 0 <= v0 <= v1 <= 2**32 and 0 <= a0 <= a1 <= 2**32
        assert 0 <= i0 < i1 <= 2**32 and 0 <= index_end <= 2**32
        base = encoded & 0x7fffffff if encoded & 0x80000000 else encoded
        assert i0 <= base and base + instances <= i1

        def inside(start: int, stop: int) -> bool:
            return v0 <= start <= stop <= v1 or a0 <= start <= stop <= a1

        if encoded & 0x80000000:
            assert inside(first, first + amount), (record, bounds)
        else:
            assert first + amount <= min(index_end, len(indices))
            fetched = indices[first:first + amount]
            assert all(inside(value, value + 1) for value in fetched), record
            checked += amount
    requested = [counts[name] for name in ("hud", "grass", "particles", "surface", "caster_vertices", "caster_instances")]
    assert len(result["direct"]) == len(requested) == 6
    assert all(0 <= used <= cap <= 2**32 for used, cap in zip(requested, result["direct"]))
    assert result["frame_admitted_draws"] == active and result["frame_checked_indices"] == checked
    assert result["work"] >= checked + active


def atomic_refusal(result: dict, count: int) -> None:
    assert result["geometry_status"] != 0, result
    assert result["domains"] == [[0, 99, 0, 0, 0, 0, 0] for _ in range(count)], "partial output publication"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--captures", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--modes", nargs="+", choices=["debug", "release", "sanitized"], default=["debug", "release", "sanitized"])
    parser.add_argument("--explore-stale-compiler", action="store_true")
    args = parser.parse_args()
    started = time.monotonic()
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    compiler = ROOT / "coax/bin/coax"

    def run(argv: list, *, env=None) -> subprocess.CompletedProcess:
        remaining = 830 - (time.monotonic() - started)
        if remaining <= 0:
            raise TimeoutError("geometry qualification reached its bounded command deadline")
        result = subprocess.run([str(x) for x in argv], cwd=ROOT, env=env, capture_output=True, timeout=remaining)
        if result.returncode:
            raise RuntimeError(f"command failed ({result.returncode}): {argv!r}\n" + result.stderr.decode(errors="replace")[-2000:])
        return result

    compiler_hash = sha(compiler)
    version = run([compiler, "version"]).stdout.decode().strip()
    recorded = (compiler.parent / "source-digest").read_text().strip()
    current = run([compiler, "source-digest", ROOT]).stdout.decode().strip()
    stale = recorded != current
    if stale and not args.explore_stale_compiler:
        raise RuntimeError("compiler recorded/current source digests differ; rebuild before final qualification")
    fixture = HERE / "geometry-native.coax"
    deps = json.loads(run([compiler, "deps", fixture]).stdout)
    source_paths = [Path(x) for x in deps["sources"]] + [HERE / "geometry-native.c", HERE / "catalog-6445ee0050.json", Path(__file__).resolve()]
    closure = {str(path.relative_to(ROOT)): sha(path) for path in source_paths}
    snapshot = json.loads((HERE / "catalog-6445ee0050.json").read_text())
    cases = []
    inputs = {}
    for profile, meta in enumerate(snapshot["profiles"]):
        source = args.captures.resolve() / meta["slug"]
        capture_path = source / "capture.json"
        capture = json.loads(capture_path.read_text())
        assert capture["schema"] == "aethyr.render.producer-capture.v1"
        dest = work / "inputs" / meta["slug"]
        dest.mkdir(parents=True, exist_ok=True)
        # Snapshot all bytes and their producer receipt before any native work.
        for name, info in capture["files"].items():
            assert Path(name).name == name, name
            data = (source / name).read_bytes()
            assert len(data) == info["bytes"] and hashlib.sha256(data).hexdigest() == info["sha256"], (meta["slug"], name)
            (dest / name).write_bytes(data)
        (dest / "capture.json").write_bytes(capture_path.read_bytes())
        inputs[meta["slug"]] = dict(capture_sha256=sha(capture_path), files=capture["files"], counts=capture["counts"], options=capture["options"], wasm=capture["wasm"], tool=capture["tool"], inputs=capture["inputs"])
        bindings = []
        store = 0
        first = meta["binding_first"]
        for resource, binding in enumerate(snapshot["bindings"][first:first + meta["binding_count"]]):
            if binding["kind"] != "storage":
                continue
            if binding["name"] in ("vertices", "mesh") and (dest / (binding["name"] + ".bin")).exists():
                path = dest / (binding["name"] + ".bin")
            else:
                path = dest / ("s" + str(store) + ".bin")
                store += 1
            if path.exists():
                bindings.append((0, resource, path))
        first = meta["particle_binding_first"]
        for resource, binding in enumerate(snapshot["bindings"][first:first + meta["particle_binding_count"]]):
            if binding["kind"] == "storage" and (dest / "puffs.bin").exists():
                bindings.append((1, resource, dest / "puffs.bin"))
        c = capture["counts"]
        assert capture["vertex_stride"] == meta["vertex_stride"]
        draw_words = words(dest / "draws.bin")
        index_words = words(dest / "index.bin")
        assert len(draw_words) == c["draws"] * 4
        assert (dest / "hud.bin").stat().st_size >= c["hud"] * capture["vertex_stride"] * 4
        cases.append(dict(profile=profile, meta=meta, capture=capture, dest=dest, bindings=bindings, draw_words=draw_words, index_words=index_words))
    if args.receipt.exists():
        old = json.loads(args.receipt.read_text())
        if old.get("source_sha256") == closure and old.get("compiler_sha256") == compiler_hash and old.get("producer_inputs") == inputs:
            raise RuntimeError("this unchanged source/compiler/producer inventory is already qualified; retain its receipt")

    def config(case: dict, name: str, *, counts=None, uniform=None, draws=None, bindings=None) -> Path:
        cap = case["capture"]
        c = cap["counts"] if counts is None else counts
        vals = [case["profile"], 320, 180, c["hud"], cap["vertex_stride"], c["draws"], c["water"], c["grass"], c["particles"], c["surface"], c["caster_vertices"], c["caster_instances"]]
        dest = case["dest"]
        paths = [uniform or dest / "u.bin", dest / "hud.bin", draws or dest / "draws.bin", dest / "index.bin"]
        rows = case["bindings"] if bindings is None else bindings
        text = " ".join(map(str, vals)) + "\n" + "\n".join(str(path) if path.exists() else "-" for path in paths) + "\n" + str(len(rows)) + "\n" + "\n".join(f"{group} {resource} {path}" for group, resource, path in rows) + "\n"
        path = dest / (name + ".config")
        path.write_text(text)
        return path

    emitted = {}
    modes = []
    for mode in args.modes:
        release = mode == "release"
        if release not in emitted:
            part = work / ("wrapping" if release else "checked")
            part.mkdir(exist_ok=True)
            header = part / "geometry.h"
            emitted_run = run([compiler, "c", fixture, "--library", "--header", header] + (["--release"] if release else []))
            (part / "emission.log").write_bytes(emitted_run.stderr)
            source = part / "geometry.c"
            source.write_bytes(emitted_run.stdout)
            header_inventory(header.read_text())
            emitted[release] = (source, header)
        source, header = emitted[release]
        exe = work / ("geometry-" + mode)
        flags = ["-O2"] if release else ["-O0", "-g"]
        if mode == "sanitized":
            flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        run(["clang", "-std=c11", *flags, "-I", header.parent, source, HERE / "geometry-native.c", "-lm", "-o", exe])
        env = dict(os.environ)
        if mode == "sanitized":
            env["ASAN_OPTIONS"] = "detect_leaks=0:halt_on_error=1"
            env["UBSAN_OPTIONS"] = "halt_on_error=1"
        records = []
        for case in cases:
            path = config(case, "good")
            result = json.loads(run([exe, path], env=env).stdout)
            wire_audit(result, case["capture"]["counts"], case["draw_words"], case["index_words"])
            faults = []
            short_u = case["dest"] / "short-u.bin"
            short_u.write_bytes(b"\0" * 4)
            bad = json.loads(run([exe, config(case, "short-u", uniform=short_u)], env=env).stdout)
            atomic_refusal(bad, case["capture"]["counts"]["draws"])
            assert bad["geometry_status"] == 3
            faults.append("short-uniform-view")
            if case["bindings"]:
                bad = json.loads(run([exe, config(case, "duplicate-binding", bindings=case["bindings"] + case["bindings"][:1])], env=env).stdout)
                atomic_refusal(bad, case["capture"]["counts"]["draws"])
                assert bad["geometry_status"] == 4
                faults.append("duplicate-resource")
            active = next((i for i in range(len(case["draw_words"]) // 4) if case["draw_words"][i * 4 + 1] and case["draw_words"][i * 4 + 3]), None)
            if active is not None:
                broken = list(case["draw_words"])
                broken[active * 4] = 0xffffffff
                broken[active * 4 + 1] = 2
                bad_draws = case["dest"] / "bad-draws.bin"
                bad_draws.write_bytes(struct.pack("<" + str(len(broken)) + "I", *broken))
                bad = json.loads(run([exe, config(case, "draw-overrun", draws=bad_draws)], env=env).stdout)
                atomic_refusal(bad, case["capture"]["counts"]["draws"])
                faults.append("addressed-draw-overrun")
            c = dict(case["capture"]["counts"], hud=result["direct"][0] + 1)
            bad = json.loads(run([exe, config(case, "hud-overrun", counts=c)], env=env).stdout)
            assert bad["geometry_status"] == 0 and bad["frame_status"] == 10 and bad["frame_bad_direct"] == 0 and bad["frame_admitted_draws"] == 0
            faults.append("direct-hud-overrun")
            records.append(dict(slug=case["meta"]["slug"], observed=result, negative_inputs_detected=faults))
        transcript = work / (mode + ".json")
        transcript.write_text(json.dumps(records, indent=2) + "\n")
        modes.append(dict(mode=mode, profiles=len(records), generated_c_sha256=sha(source), generated_header_sha256=sha(header), executable_sha256=sha(exe), transcript=str(transcript), transcript_sha256=sha(transcript), negative_inputs=sum(len(record["negative_inputs_detected"]) for record in records)))
    assert sha(compiler) == compiler_hash, "compiler changed during qualification"
    assert closure == {str(path.relative_to(ROOT)): sha(path) for path in source_paths}, "source changed during qualification"
    receipt = dict(schema="aethyr.render.geometry.producers.v1", status="exploratory" if stale else "pass", compiler_version=version, compiler_sha256=compiler_hash, compiler_recorded_source_digest=recorded, compiler_current_source_digest=current, compiler_current_sources_match=not stale, source_sha256=closure, header_fields_checked=FIELDS, producer_inputs=inputs, modes=modes, elapsed_seconds=round(time.monotonic() - started, 3), scope="18 actual retained producer payloads through normal-C Coax geometry and frame admission, independent wire-range audit and atomic-refusal negative inputs. App encoding rules are qualified by the sharp Coax checks. Does not prove shader arithmetic, asset BVH truth, GPU execution or images.")
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(dict(status=receipt["status"], profiles=len(cases), modes=[mode["mode"] for mode in modes], receipt=str(args.receipt))))


if __name__ == "__main__":
    main()
