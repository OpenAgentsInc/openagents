#!/usr/bin/env python3
"""One-shot lifecycle for the frozen, already-built neural/owned-query prefix.

Preparation does file/identity work only.  Running requires a separately reviewed
prepared context; it never rebuilds, substitutes a provider, retries, or removes
the experiment-wide launch marker.  The product performs its own 88-file entry
admission.  The independent audit is a separate offline consumer.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import signal
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve()
FIXTURE = ROOT / "psionic/fixtures/robotics/panda-owned-query-neural"
EXPERIMENT_SHA = "bfdd85bf14ed244f9c0b303c1f742a19086e821a229c207f242bc575836845bc"
SCHEMA = "psionic.panda-owned-query-neural.execution.v1"
LAUNCH_MARKER = "/tmp/psionic-panda-owned-query-neural-" + EXPERIMENT_SHA + ".launch-once.json"


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(path):
    data = Path(path).read_bytes()
    return {"bytes": len(data), "sha256": sha(data)}


def stamp():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def save(path, value):
    path = Path(path)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    tmp.replace(path)


def load(path):
    return json.loads(Path(path).read_text())


def frozen():
    experiment = FIXTURE / "experiment.json"
    need(identity(experiment)["sha256"] == EXPERIMENT_SHA, "frozen experiment")
    e = load(experiment)
    need(e["mode"] == "owned-shadow-v1" and e["maximum_action_ticks"] == 9, "fixed mode/cap")
    need(e["timeout_seconds"] == 840 and e["scenario"] == 0, "fixed timeout/scenario")
    need(platform.system() == "Darwin" and platform.machine() == "arm64" and sys.byteorder == "little", "Darwin-arm64 little endian profile")
    for name, expected in e["carried_receipts"].items():
        need(identity(ROOT / name) == expected, "carried receipt: " + name)
    old = load(ROOT / "psionic/fixtures/robotics/panda-query-runtime/source-inventory.json")
    need(old["source_files"] == e["source125_sha256"] and len(old["source_files"]) == 125, "carried source inventory")
    build = load(ROOT / "psionic/fixtures/robotics/panda-query-runtime/build.json")
    need(build["passed"] and build["binary_sha256"] == e["inputs"][e["binary_path"]]["sha256"], "qualified binary binding")
    # Reuse the qualified 125-file tree proof. These seven small, compiled effect
    # sources additionally bind the source-derived lifecycle interpretation here.
    sources = {str(HERE.relative_to(ROOT)): identity(HERE)["sha256"]}
    for name, expected in build["source_sha256"].items():
        actual = identity(ROOT / name)["sha256"]
        need(actual == expected, "compiled effect source: " + name)
        sources[name] = actual
    inputs = {}
    for name, expected in e["inputs"].items():
        inputs[name] = identity(name)
        need(inputs[name] == expected, "small input: " + name)
    weights = []
    for w in e["prior_shards"]:
        p = Path(w["path"])
        s = p.stat()
        need(p.is_file() and s.st_size == w["bytes_observed"], "carried weight size")
        weights.append({"path": str(p), "resolved_path": str(p.resolve()),
                        "bytes": s.st_size, "device": s.st_dev, "inode": s.st_ino,
                        "mtime_ns": s.st_mtime_ns, "ctime_ns": s.st_ctime_ns,
                        "prior_completed_full_sha256": w["prior_completed_full_sha256"],
                        "payload_hash_recomputed": False})
    return e, {"experiment": identity(experiment), "source": sources,
               "inputs": inputs, "weight_stats": weights,
               "carried_receipts": e["carried_receipts"],
               "source125_proof_reused": True,
               "standalone_88_file_admission_executed": False,
               "builds_or_old_gates_executed": 0}


def command(e, run):
    return [e["binary_path"], e["shards_path"], e["PST_path"], e["noise_path"],
            e["shader_directory"], str(run / e["child_output_relative"]),
            e["instruction"], "9", "0", e["pair_path"]]


def artifacts(run):
    result = {}
    for p in sorted(run.rglob("*")):
        if p.is_file() and p not in {run / "record.json", run / "record.json.tmp"}:
            result[str(p.relative_to(run))] = identity(p)
    return result


def prepare(run):
    run.mkdir(parents=True, exist_ok=False)
    r = {"schema": "psionic.panda-owned-query-neural.preparation.v1",
         "passed": False, "started_utc": stamp(), "SDK_loads": 0,
         "SDK_steps": 0, "NN_calls": 0, "render_calls": 0}
    try:
        e, context = frozen()
        r.update(context)
        r["argv"] = command(e, run)
        r["environment_overrides"] = e["native_profile"]["environment"]
        r["timeout_seconds"] = e["timeout_seconds"]
        r["global_launch_marker"] = LAUNCH_MARKER
        (run / "experiment.json").write_bytes((FIXTURE / "experiment.json").read_bytes())
        r["passed"] = True
    except BaseException as exc:
        r["error"] = type(exc).__name__ + ": " + str(exc)
        raise
    finally:
        r["finished_utc"] = stamp()
        save(run / "preparation.json", r)
    print(json.dumps({"prepared": str(run), "preparation": identity(run / "preparation.json"), "effects": 0}))


def basic_result(e, run):
    case = run / e["case_output_relative"]
    r = load(case / "record.json")
    for field, expected in (("schema", e["case_schema"]), ("profile", e["profile"]),
                            ("query_profile", e["query_profile"]), ("policy_profile", e["policy_profile"]),
                            ("case", "demo_000"), ("scenario", 0),
                            ("SDK_geometry_retained", True), ("owned_model_scope", "exact-demo000-scene-reset-assets"),
                            ("fallbacks", 0), ("refusals", 0), ("physics_replay_exact", True)):
        need(r.get(field) == expected, "native record: " + field)
    t = r["ticks"]
    need(type(t) is int and 1 <= t <= 9 and (t == 9 or r["success"] is True), "complete prefix or source-authoritative early success")
    need(type(r["success"]) is bool, "success boolean")
    need(r["model_calls"] == (t + 7) // 8 and r["physics_steps"] == 25 * t and r["trace_records"] == 25 * t, "model/step/trace counts")
    need(type(r["torques_clipped"]) is int and 0 <= r["torques_clipped"] <= 7 * 25 * t, "torque clip count")
    for key in ("wall_seconds", "model_context_open_seconds", "inference_seconds", "inference_max_seconds", "gpu_seconds"):
        need(type(r[key]) in (int, float) and math.isfinite(r[key]) and r[key] >= 0, "finite timing: " + key)
    need((case / "physics.ptr").stat().st_size == 64 + 512 * 25 * t, "complete PTR1 extent")
    decoded = (case / "first-decoded.f32").read_bytes()
    need(len(decoded) == 448 and all(math.isfinite(x) for x in struct.unpack("<112f", decoded)), "finite first decoded array")
    return {"case_record": identity(case / "record.json"), "trace": identity(case / "physics.ptr"),
            "first_decoded": identity(case / "first-decoded.f32"),
            "first_decoded_prior_hash_matches_DIAGNOSTIC_ONLY": sha(decoded) == e["first_decoded_diagnostic"]["prior_SHA256"],
            "ticks": t, "model_calls": r["model_calls"], "task_success": r["success"],
            "physics_steps": 25 * t, "mandatory_replay_steps": 25 * t,
            "source_derived_NOT_instrumented": {"SDK_steps": 50 * t, "SDK_resets": 2,
                "explicit_mj_forward": 2 + 50 * t, "telemetry_samples": 1 + (t + 7) // 8 + 26 * t,
                "owned_sample_paths": 1 + 25 * t, "OFFSCREENimages": 2 * ((t + 7) // 8)},
            "independent_PTR1_audit_separate": True}


def execute(run):
    prep = load(run / "preparation.json")
    need(prep.get("passed") is True, "successful immutable preparation")
    need(not (run / "launch-once.json").exists() and not (run / "episode").exists() and not (run / "record.json").exists(), "no earlier launch or episode")
    e, current = frozen()
    for key, value in current.items():
        need(prep.get(key) == value, "pre-launch unchanged context: " + key)
    need(identity(run / "experiment.json") == current["experiment"], "prepared descriptor copy")
    need(prep["argv"] == command(e, run), "prepared command")
    need(prep["global_launch_marker"] == LAUNCH_MARKER, "fixed experiment-wide launch marker")
    need(prep["environment_overrides"] == e["native_profile"]["environment"] and prep["timeout_seconds"] == 840, "prepared execution environment/cap")
    launch = {"schema": "psionic.panda-owned-query-neural.one-shot.v1", "started_utc": stamp(),
              "experiment": current["experiment"], "preparation": identity(run / "preparation.json"),
              "run_directory": str(run), "argv": prep["argv"], "retry_permitted": False}
    # Exclusive global marker covers accidental alternate output directories too.
    with open(prep["global_launch_marker"], "x") as f:
        json.dump(launch, f, indent=2)
        f.write("\n")
    save(run / "launch-once.json", launch)
    r = {"schema": SCHEMA, "passed": False, "started_utc": stamp(), **current,
         "preparation": launch["preparation"], "launch": identity(run / "launch-once.json"),
         "argv": prep["argv"], "environment_overrides": prep["environment_overrides"],
         "timeout_seconds": 840, "exit_code": None, "timed_out": False,
         "no_retry": True, "independent_audit_required": True}
    save(run / "record.json", r)
    env = os.environ.copy()
    env.pop("DYLD_INSERT_LIBRARIES", None)
    env.update(prep["environment_overrides"])
    start = time.monotonic()
    p = None
    try:
        with open(run / "stdout.txt", "wb") as out, open(run / "stderr.txt", "wb") as err:
            p = subprocess.Popen(prep["argv"], cwd=ROOT, env=env, stdout=out, stderr=err, start_new_session=True)
            r["pid"] = p.pid
            save(run / "record.json", r)
            try:
                r["exit_code"] = p.wait(timeout=840)
            except subprocess.TimeoutExpired:
                r["timed_out"] = True
                os.killpg(p.pid, signal.SIGKILL)
                r["exit_code"] = p.wait()
        need(not r["timed_out"] and r["exit_code"] == 0, "one-shot process completed")
        need((run / "stderr.txt").stat().st_size == 0, "empty stderr / fail-fast sanitizer profile")
        r.update(basic_result(e, run))
        r["passed"] = True
    except BaseException as exc:
        if p is not None and p.poll() is None:
            os.killpg(p.pid, signal.SIGKILL)
            r["exit_code"] = p.wait()
        r["error"] = type(exc).__name__ + ": " + str(exc)
        raise
    finally:
        r["wall_seconds_including_open_live_replay"] = time.monotonic() - start
        r["finished_utc"] = stamp()
        try:
            _, after = frozen()
            need(after == current, "post-run source/input/weight-stat immutability")
            need(identity(run / "preparation.json") == launch["preparation"], "preparation immutable")
            r["context_unchanged_after"] = True
        except BaseException as exc:
            r["passed"] = False
            r["identity_error"] = type(exc).__name__ + ": " + str(exc)
        r["artifacts"] = artifacts(run)
        trace = run / e["case_output_relative"] / "physics.ptr"
        if trace.exists():
            n = trace.stat().st_size
            r["partial_trace_extent"] = {"bytes": n, "complete_512B_blocks_after_seed": max(0, n - 32) // 512,
                                         "remaining_bytes_after_seed": max(0, n - 32) % 512,
                                         "footer_not_assumed_for_partial": True}
        save(run / "record.json", r)
    need(r["passed"], "execution context passed; see immutable record")
    print(json.dumps({"passed": True, "record": identity(run / "record.json"), "ticks": r["ticks"], "model_calls": r["model_calls"]}))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--prepare", type=Path)
    g.add_argument("--run-prepared", type=Path)
    a = p.parse_args()
    if a.prepare is not None:
        prepare(a.prepare.resolve())
    else:
        execute(a.run_prepared.resolve())


if __name__ == "__main__":
    main()
