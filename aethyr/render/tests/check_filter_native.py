#!/usr/bin/env python3
"""Qualify checked CPU filters against retained, independent device observations."""
from pathlib import Path
import argparse
import hashlib
import json
import math
import os
import shutil
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
SHA = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()


def run(command, *, cwd=ROOT, env=None, timeout=180):
    p = subprocess.run(list(map(str, command)), cwd=cwd, env=env, capture_output=True, timeout=timeout)
    if p.returncode:
        raise RuntimeError(f"{command}: exit {p.returncode}\n{p.stdout.decode()}\n{p.stderr.decode()}")
    return p.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--coax", type=Path, default=ROOT / "coax/bin/coax")
    parser.add_argument("--reuse-emission", action="store_true",
                        help="Reuse this tool's retained normal-C admission after an effects-driver failure")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    source = ROOT / "aethyr/render/tests/filter-native.coax"
    driver = ROOT / "aethyr/render/tests/filter-native.c"
    before = {str(p): SHA(p) for p in ROOT.rglob("*.coax")}
    compiler = SHA(args.coax)
    driver_hash = SHA(driver)
    command = [args.coax, "c", source, "--library", "--header", out / "Filter.h",
               "--profile-report", out / "emission-profile.json"]
    started = time.monotonic()
    if not args.reuse_emission:
        with (out / "Filter.c").open("wb") as emitted, (out / "emission.log").open("wb") as log:
            p = subprocess.run(list(map(str, command)), cwd=ROOT, stdout=emitted, stderr=log, timeout=180)
        assert p.returncode == 0, (out / "emission.log").read_text()
    profile = json.loads((out / "emission-profile.json").read_text())
    assert profile["success"] and profile["exit_status"] == 0
    assert profile["emitted_sha256"] == SHA(out / "Filter.c")
    sources = {row["path"]: row["sha256"] for row in profile["sources"]}
    assert all(before[p] == h == SHA(p) for p, h in sources.items())
    shutil.copyfile(driver, out / "filter-native.c")
    observed = []
    negative = []
    faults = []
    fixture_sources = {}
    for dataset in ("format", "joint"):
        fixture = ROOT / f"aethyr/tests/render-filter-{dataset}-cases.json"
        raw = json.loads(fixture.read_text())
        metal_path = Path(f"/tmp/aethyr-ch11-filter-{dataset}-webgpu.json")
        native_path = Path(f"/tmp/aethyr-ch11-native-filter-{dataset}/qualification.json")
        metal = json.loads(metal_path.read_text())
        native = json.loads(native_path.read_text())
        assert not metal["errors"] and not metal["source_changes"]
        assert native["validation_errors"] == native["validation_warnings"] == 0
        for file in (fixture, metal_path, native_path):
            fixture_sources[str(file)] = SHA(file)
            shutil.copyfile(file, out / f"{dataset}-{file.name}")
        textures = {}
        for texture in raw["textures"]:
            name = texture["format"]
            byte_path = out / f"{dataset}-{name}.bin"
            meta_path = out / f"{dataset}-{name}.levels"
            byte_path.write_bytes(bytes(texture["bytes"]))
            meta_path.write_bytes(b"".join(struct.pack("<qqQQQ", l["width"], l["height"], l["offset"],
                                                       l["row_bytes"], l["row_bytes"] * l["height"])
                                           for l in texture["levels"]))
            textures[name] = byte_path, meta_path
        bytes8, levels8 = textures["rgba8unorm"]
        byte_half, levels_half = textures["rgba16float"]
        for name, receipt in (("metal-unorm8", metal["results"]),
                              ("nvidia-unorm8-integer", native["observations"])):
            rows = [r for r in receipt if r["format"] == "rgba8unorm"]
            assert len(rows) == len(raw["cases"]) == 20
            for index, (case, expectation) in enumerate(zip(raw["cases"], rows)):
                spec = dict(dataset=dataset, profile=name, format="rgba8unorm", bytes=bytes8, levels=levels8,
                            x=case["x"], y=case["y"], lod=case["lod"], mode="linear", loss="none")
                if name == "nvidia-unorm8-integer" and case["lod"] != math.floor(case["lod"]):
                    negative.append((spec, 0, 3))
                else:
                    observed.append((spec, expectation["rgba"]))
        for name in ("metal-unorm8", "nvidia-unorm8-integer"):
            base = dict(dataset=dataset, profile=name, format="rgba8unorm", bytes=bytes8, levels=levels8,
                        x=.2, y=.3, lod=0, mode="linear", loss="none")
            for mode, lod in (("min-nearest", 1), ("mag-nearest", 0), ("mip-nearest", 0),
                              ("anisotropic", 0), ("compare", 0)):
                negative.append((dict(base, mode=mode, lod=lod), 0, 5 if mode == "compare" else 3))
            negative += [(dict(base, format="rgba16float", bytes=byte_half, levels=levels_half), 0, 3),
                         (dict(base, format="rgba8unorm-srgb"), 0, 3),
                         (dict(base, profile=name.upper()), 0, 3),
                         (dict(base, x="nan"), 0, 5), (dict(base, y="inf"), 0, 5),
                         (dict(base, lod="nan"), 0, 5),
                         (dict(base, loss="short-bytes"), None, -1)]
        nvidia = dict(dataset=dataset, profile="nvidia-unorm8-integer", format="rgba8unorm", bytes=bytes8,
                      levels=levels8, x=.2, y=.3, lod=0, mode="fractional-min", loss="none")
        negative.append((nvidia, 0, 3))
        # Independent producer-loss controls must break a real positive oracle.
        first = next((s, e) for s, e in observed if s["dataset"] == dataset and s["profile"] == "metal-unorm8")
        for loss in ("storage", "profile", "color"):
            faults.append((dict(first[0], loss=loss), first[1]))
        last = next((s, e) for s, e in reversed(observed) if s["dataset"] == dataset and s["profile"] == "metal-unorm8" and s["lod"] == 1) if dataset == "format" else None
        if last:
            faults.append((dict(last[0], loss="mips"), last[1]))

    def invoke(binary, spec, env):
        values = [binary, spec["profile"], spec["format"], spec["bytes"], spec["levels"], spec["x"], spec["y"],
                  spec["lod"], spec["mode"], spec["loss"]]
        return json.loads(run(values, env=env, timeout=30))

    def matches(result, expected):
        return result["decoded"] == 0 and result["sampled"] == 0 and all(
            math.isfinite(a) and abs(a - b) <= 1e-7 for a, b in zip(result["rgba"], expected))

    modes = {}
    for name, options in (("debug", ["-O0", "-g"]), ("release", ["-O2"]),
                          ("sanitized", ["-O1", "-g", "-fsanitize=address,undefined", "-fno-omit-frame-pointer"])):
        binary = out / f"filter-{name}"
        cc = ["cc", "-std=c11", "-ffp-contract=off", "-fno-fast-math", *options, "-I", out,
              out / "Filter.c", out / "filter-native.c", "-lm", "-o", binary]
        run(cc)
        env = dict(os.environ, ASAN_OPTIONS="detect_leaks=0", UBSAN_OPTIONS="halt_on_error=1")
        observations = []
        for spec, expected in observed:
            result = invoke(binary, spec, env)
            assert matches(result, expected), (name, spec, result, expected)
            observations.append(dict(profile=spec["profile"], dataset=spec["dataset"], x=spec["x"], y=spec["y"],
                                     lod=spec["lod"], actual=result["rgba"], expected=expected))
        for spec, decoded, sampled in negative:
            result = invoke(binary, spec, env)
            if decoded is None:
                assert result["decoded"] != 0 and result["sampled"] == -1 and result["output_preserved"], (name, spec, result)
            else:
                assert result["decoded"] == decoded and result["sampled"] == sampled, (name, spec, result)
                assert all(v == 0 for v in result["rgba"]), (name, spec, result)
        for spec, expected in faults:
            assert not matches(invoke(binary, spec, env), expected), (name, "undetected producer loss", spec)
        modes[name] = dict(positive_observations=len(observed), compared_channels=len(observed) * 4,
                           refusal_cases=len(negative), producer_loss_controls=len(faults), executable_sha256=SHA(binary),
                           compiler_command=list(map(str, cc)), observations=observations)
    fence = {p: SHA(p) == h for p, h in sources.items()}
    assert all(fence.values()) and SHA(driver) == driver_hash
    record = dict(schema="aethyr.render.native-filtering.v1", status="pass", compiler_sha256=compiler,
                  elapsed_seconds=time.monotonic() - started, emission_command=list(map(str, command)),
                  c_sha256=SHA(out / "Filter.c"), header_sha256=SHA(out / "Filter.h"), driver_sha256=driver_hash,
                  reused_admitted_emission=args.reuse_emission,
                  source_sha256=sources, source_fence=fence, retained_gpu_expectations=fixture_sources, modes=modes,
                  rule="All RGBA finite; each channel differs from the retained independent GPU observation by at most 1e-7. No mask.",
                  scope="Physical UNORM8 only. Metal covers the authored integer/fractional LOD cases; NVIDIA covers integer bounded LOD only. Other sampled formats/modes and NVIDIA fractional bounded LOD explicitly refuse. No universal GPU precision or new GPU execution claim.")
    (out / "qualification.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({k: record[k] for k in ("status", "elapsed_seconds", "c_sha256", "header_sha256")}))


if __name__ == "__main__":
    main()
