#!/usr/bin/env python3
"""Attribute the whole clean and warm iOS development loop (R0).

Stages, each timed for a clean run (fresh outputs and derived data) and a
warm one (unchanged inputs, then one edited Coax unit):

- coax: load, lex, parse, check, compile-time evaluation and C emission per
  unit of apps/openagents-ios/generate.sh, from `coax c --timings`, with
  generated C bytes and the compiler's peak resident memory;
- C compilation of the generated units for the iOS Simulator;
- the Xcode build (its own timing summary: script phase, C, Swift, link,
  assets and resources);
- launch: install and `simctl launch` on a task-owned Simulator, to process
  start (not first frame);
- GPU preparation: `MTLDevice.makeLibrary(source:)` on the app's shader text,
  the same call HomeRenderer makes at start, on this Mac's GPU.

Stages overlap in Xcode; the Xcode rows are its own task sums, not additive
with the standalone coax/C rows. Missing measurements are recorded as absent.
"""
import argparse, json, os, platform, re, shutil, statistics, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
IOS = ROOT / "apps/openagents-ios"


def run(cmd, **kw):
    p = subprocess.run([str(c) for c in cmd], capture_output=True, **kw)
    if p.returncode:
        tail = (p.stdout + p.stderr).decode(errors="replace")
        errors = "\n".join(l for l in tail.splitlines() if "error:" in l)[-2000:]
        raise RuntimeError(f"{cmd[0]} failed: {errors or tail[-2000:]}")
    return p


def timed(cmd, **kw):
    t = time.monotonic()
    p = run(cmd, **kw)
    return time.monotonic() - t, p


def units():
    text = (IOS / "generate.sh").read_text()
    names = re.search(r"for name in ([a-z -]+); do", text).group(1).split()
    out = [f"verse/{n}.coax" for n in names]
    out += [f"verse/{m}.coax" for m in re.findall(r"(?<![a-z])(haunt-buffer|phone-signin)\.coax", text)]
    out += [f"verse/{m.split(':')[0]}.coax" for m in re.search(r"for pair in ([^;]+); do", text).group(1).split()]
    shaders = re.findall(r'"\$compiler" metal (verse/[a-z-]+\.coax)', text)
    return sorted(set(out)), shaders


def coax_stage(compiler, files, work):
    rows = {}
    for f in files:
        name = Path(f).stem
        timings = work / f"{name}.json"
        cmd = ["/usr/bin/time", "-l", compiler, "c", ROOT / f, "--library", "--header", work / f"{name}.h", "--timings", timings]
        wall, p = timed(cmd)
        (work / f"{name}.c").write_bytes(p.stdout)
        peak = int(re.search(rb"(\d+)\s+maximum resident set size", p.stderr).group(1))
        t = json.loads(timings.read_text())
        rows[f] = {"wall_seconds": wall, "peak_rss_bytes": peak, "generated_c_bytes": len(p.stdout), "phases_seconds": t["seconds"]}
    phases = {}
    for r in rows.values():
        for k, v in r["phases_seconds"].items():
            phases[k] = phases.get(k, 0) + v
    return {"units": rows, "phase_totals_seconds": phases, "wall_seconds": sum(r["wall_seconds"] for r in rows.values()),
            "generated_c_bytes": sum(r["generated_c_bytes"] for r in rows.values()),
            "peak_rss_bytes": max(r["peak_rss_bytes"] for r in rows.values())}


def c_stage(files, work):
    sdk = run(["xcrun", "--sdk", "iphonesimulator", "--show-sdk-path"]).stdout.decode().strip()
    cc = run(["xcrun", "--sdk", "iphonesimulator", "--find", "clang"]).stdout.decode().strip()
    total, rows = 0.0, {}
    for f in files:
        name = Path(f).stem
        wall, _ = timed([cc, "-std=c11", "-O0", "-g", "-isysroot", sdk, "-target", "arm64-apple-ios17.0-simulator",
                         "-I", work, "-I", ROOT / "verse/platform", "-c", work / f"{name}.c", "-o", work / f"{name}.o"])
        rows[f] = wall
        total += wall
    return {"units_seconds": rows, "wall_seconds": total}


SUMMARY = re.compile(r"^(.+?) \((\d+) tasks?\) \| ([\d.]+) seconds", re.M)


def xcode_build(derived, destination):
    cmd = ["xcodebuild", "-project", IOS / "OpenAgents.xcodeproj", "-scheme", "OpenAgents", "-destination", destination,
           "-derivedDataPath", derived, "CODE_SIGNING_ALLOWED=NO", "ARCHS=arm64", "ONLY_ACTIVE_ARCH=YES",
           "-showBuildTimingSummary", "build"]
    wall, p = timed(cmd)
    tasks = {m[0]: {"tasks": int(m[1]), "seconds": float(m[2])} for m in SUMMARY.findall(p.stdout.decode(errors="replace"))}
    return {"wall_seconds": wall, "tasks": tasks}


def launch(app, repeats):
    udid = run(["xcrun", "simctl", "create", "R0 loop attribution", "iPhone 17 Pro Max"]).stdout.decode().strip()
    try:
        boot, _ = timed(["xcrun", "simctl", "bootstatus", udid, "-b"])
        install, _ = timed(["xcrun", "simctl", "install", udid, app])
        starts = []
        for _ in range(repeats):
            t, _ = timed(["xcrun", "simctl", "launch", udid, "com.openagents.app"])
            starts.append(t)
            run(["xcrun", "simctl", "terminate", udid, "com.openagents.app"])
        return {"simulator_boot_seconds": boot, "install_seconds": install, "launch_to_process_seconds": starts,
                "launch_median_seconds": statistics.median(starts)}
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", udid], capture_output=True)
        subprocess.run(["xcrun", "simctl", "delete", udid], capture_output=True)


GPU = r'''
import Metal
import Foundation
let src = try! String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let device = MTLCreateSystemDefaultDevice()!
var out: [Double] = []
for _ in 0..<Int(CommandLine.arguments[2])! {
  let t = DispatchTime.now().uptimeNanoseconds
  _ = try! device.makeLibrary(source: src, options: nil)
  out.append(Double(DispatchTime.now().uptimeNanoseconds - t) / 1e9)
}
print(out.map { String($0) }.joined(separator: " "))
print(device.name)
'''


def gpu_stage(compiler, shaders, work, repeats):
    text = "".join(run([compiler, "metal", ROOT / s]).stdout.decode() for s in shaders)
    text += (IOS / "OpenAgents/Platform/Stages.metal").read_text()
    src = work / "HomeShaders.txt"
    src.write_text(text)
    script = work / "gpu.swift"
    script.write_text(GPU)
    p = run(["xcrun", "swift", script, src, str(repeats)])
    lines = p.stdout.decode().split("\n")
    seconds = [float(x) for x in lines[0].split()]
    return {"device": lines[1], "shader_bytes": len(text), "make_library_seconds": seconds,
            "first_seconds": seconds[0], "repeat_median_seconds": statistics.median(seconds[1:]) if len(seconds) > 1 else None,
            "scope": "Mac GPU; a phone's own shader compile is not measured here"}


def main(argv=None):
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("--output", required=True)
    a.add_argument("--skip-xcode", action="store_true", help="only coax, C and GPU stages")
    a.add_argument("--launches", type=int, default=3)
    a = a.parse_args(argv)
    out = Path(a.output)
    out.mkdir(parents=True, exist_ok=True)
    compiler = ROOT / "coax/bin/coax"
    files, shaders = units()
    report = {"schema": "coax.r0.loop.v1", "source_commit": run(["git", "-C", ROOT, "rev-parse", "HEAD"]).stdout.decode().strip(),
              "compiler": run([compiler, "version"]).stdout.decode().strip(), "machine": platform.platform(),
              "cpu": run(["sysctl", "-n", "machdep.cpu.brand_string"]).stdout.decode().strip(), "units": files, "shaders": shaders,
              "load_average_start": os.getloadavg(), "cache_scope": "clean: fresh outputs and derived data; OS file caches uncontrolled; coax has no incremental cache"}
    for phase in ("clean", "warm"):
        work = out / phase
        shutil.rmtree(work, ignore_errors=True)
        work.mkdir()
        row = {"coax": coax_stage(compiler, files, work), "c_compile": c_stage(files, work), "gpu_preparation": gpu_stage(compiler, shaders, work, 5)}
        report[phase] = row
    if not a.skip_xcode:
        derived = Path(tempfile.mkdtemp(prefix="r0-loop-"))
        try:
            dest = "generic/platform=iOS Simulator"
            report["clean"]["xcode"] = xcode_build(derived, dest)
            report["warm"]["xcode_unchanged"] = xcode_build(derived, dest)
            edited = ROOT / "verse/cast.coax"
            original = edited.read_bytes()
            try:
                edited.write_bytes(original + b"\n")
                report["warm"]["xcode_one_unit_edited"] = xcode_build(derived, dest)
            finally:
                edited.write_bytes(original)
            app = next(derived.glob("Build/Products/*-iphonesimulator/*.app"))
            report["launch"] = launch(app, a.launches)
        finally:
            shutil.rmtree(derived, ignore_errors=True)
    # Name the largest bottleneck among the stages a developer waits for.
    waits = {"coax (all units)": report["clean"]["coax"]["wall_seconds"], "C compile (generated units)": report["clean"]["c_compile"]["wall_seconds"],
             "GPU preparation (first makeLibrary)": report["clean"]["gpu_preparation"]["first_seconds"]}
    for k, v in report["clean"].get("xcode", {}).get("tasks", {}).items():
        waits[f"Xcode {k}"] = v["seconds"]
    if "launch" in report:
        waits["launch to process"] = report["launch"]["launch_median_seconds"]
    report["load_average_end"] = os.getloadavg()
    report["clean_waits_seconds"] = dict(sorted(waits.items(), key=lambda kv: -kv[1]))
    report["largest_clean_bottleneck"] = next(iter(report["clean_waits_seconds"]))
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    for k, v in list(report["clean_waits_seconds"].items())[:8]:
        print(f"{v:9.3f} s  {k}")
    print(out / "report.json")


if __name__ == "__main__":
    main()
