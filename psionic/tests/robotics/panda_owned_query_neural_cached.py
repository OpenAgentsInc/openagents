#!/usr/bin/env python3
"""Portable OFFLINE audit of the committed FIRST neural/owned-query prefix.

This adapter never opens a model, weight, SDK, private build, or original input.
It carries the measured admission/source-context proof and imports only the
hash-bound independent reader's pure functions. --plan checks associations only;
the numerical path was deliberately not repeated during publication packaging.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct
import time

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "psionic/fixtures/robotics/panda-owned-query-neural"
EXPERIMENT_SHA = "bfdd85bf14ed244f9c0b303c1f742a19086e821a229c207f242bc575836845bc"


def need(ok, text):
    if not ok:
        raise ValueError(text)


def unique(pairs):
    out = {}
    for key, value in pairs:
        need(key not in out, "duplicate JSON key")
        out[key] = value
    return out


def load(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=unique)


def identity(path):
    b = Path(path).read_bytes()
    return {"bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()}


def metadata(f):
    pub = load(f / "core-publication.json")
    need(pub["schema"] == "psionic.panda-owned-query-neural.core-publication.v1", "publication schema")
    for name, expected in pub["artifacts"].items():
        need(identity(ROOT / name) == expected, "committed Core artifact: " + name)
    e = load(f / "experiment.json")
    need(identity(f / "experiment.json")["sha256"] == EXPERIMENT_SHA, "original pre-effect experiment")
    n = load(f / "execution.json")
    a = load(f / "independent-qualification.json")
    g = load(f / "reader-qualification.json")
    p = load(f / "preparation.json")
    offline = load(f / "offline-input-v1.json")
    for name, expected in pub["independent_bindings"].items():
        need(identity(ROOT / name) == expected, "committed independent artifact: " + name)
    need(n["schema"] == "psionic.panda-owned-query-neural.execution.v1" and n["passed"] is True and n["exit_code"] == 0 and n["timed_out"] is False, "original successful invocation")
    need(n["context_unchanged_after"] is True and p["passed"] is True, "measured private context admitted and unchanged")
    need(n["experiment"] == a["experiment"] == identity(f / "experiment.json"), "native/independent experiment association")
    need(n["preparation"] == identity(f / "preparation.json") and n["launch"] == identity(f / "launch-once.json"), "unchanged original preparation/launch")
    need(n["source"] == p["source"] and n["inputs"] == p["inputs"] and n["weight_stats"] == p["weight_stats"], "carried original source/input/stat proof")
    need(n["carried_receipts"] == p["carried_receipts"] == e["carried_receipts"], "original qualification associations")
    need(a["schema"] == "psionic.panda-owned-query-neural.independent.v1" and a["passed"] is True and a["native_receipt"] == identity(f / "execution.json"), "FIRST independent proof of same invocation")
    need(a["reader_guards"] == identity(f / "reader-qualification.json") and g["passed"] is True, "original independent reader guards")
    auditor = ROOT / "psionic/tests/robotics/panda_owned_query_neural_audit.py"
    need(g["source"] == identity(auditor) and a["source"][str(auditor.relative_to(ROOT))] == identity(auditor), "source-bound pure reader")
    driver = "psionic/tests/robotics/panda_owned_query_neural_run.py"
    need(identity(ROOT / driver)["sha256"] == n["source"][driver], "unchanged original lifecycle tooling")
    pre = load(f / "prelaunch-audit-contract.json")
    verified = load(f / "prelaunch-verified.json")
    need(verified["passed"] is True and verified["contract"] == identity(f / "prelaunch-audit-contract.json") and verified["targets"] == pre["targets"], "pre-effect audit association")
    for role, path in (("independent_auditor", auditor), ("reader_guards", f / "reader-qualification.json"),
                       ("driver", ROOT / driver), ("preparation", f / "preparation.json"),
                       ("experiment", f / "experiment.json"), ("launcher", f / "launch-authorized.py")):
        need(pre["targets"][role]["identity"] == identity(path), "original prelaunch binding: " + role)
    case = f / e["case_output_relative"]
    for key, name in (("case_record", "record.json"), ("trace", "physics.ptr"), ("first_decoded", "first-decoded.f32")):
        need(n[key] == a[key] == identity(case / name), "native/independent committed payload: " + key)
    for name, expected in n["artifacts"].items():
        need(identity(f / name) == expected, "byte-exact original run artifact: " + name)
    need(offline["experiment"] == identity(f / "experiment.json") and offline["execution"] == identity(f / "execution.json"), "offline pin derivation association")
    need(offline["initial_time_u64_hex"] == "3fd0000000000003", "frozen full-reset initial time bits")
    for key, path in (("xml", e["pair_path"] + "/scene.xml"), ("reset", e["pair_path"] + "/init.f64"), ("noise", e["noise_path"])):
        need(offline["seed_input_sha256"][key] == e["inputs"][path]["sha256"], "public seed pin: " + key)
    need(offline["seed_input_sha256"]["profile"] == hashlib.sha256(e["profile"].encode()).hexdigest(), "profile seed pin")
    return e, n, a, offline, auditor, case


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--fixtures", type=Path, default=FIXTURE)
    p.add_argument("--output", type=Path)
    p.add_argument("--plan", action="store_true")
    args = p.parse_args()
    out = args.output.resolve() if args.output else None
    if out:
        out.mkdir(parents=True, exist_ok=False)
    r = {"schema": "psionic.panda-owned-query-neural.cached-audit.v1", "passed": False,
         "SDK_loads": 0, "SDK_steps": 0, "NN_calls": 0, "render_calls": 0,
         "build_calls": 0, "private_input_files_opened": 0, "plan_only": args.plan}
    began = time.monotonic()
    try:
        f = args.fixtures.resolve()
        e, native, independent, offline, auditor, case = metadata(f)
        r.update(experiment=identity(f / "experiment.json"), execution=identity(f / "execution.json"),
                 independent=identity(f / "independent-qualification.json"),
                 source={str(Path(__file__).resolve().relative_to(ROOT)): identity(__file__),
                         str(auditor.relative_to(ROOT)): identity(auditor)},
                 carried_private_admission_and_source125_proof=True,
                 fresh_private_admission_or_product_numeric_qualification_claimed=False)
        if not args.plan:
            spec = importlib.util.spec_from_file_location("owned_query_reader", auditor)
            reader = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(reader)
            native_case = reader.read_case((case / "record.json").read_bytes())
            words = reader.first_decoded((case / "first-decoded.f32").read_bytes())
            pins = offline["seed_input_sha256"]
            seed = hashlib.sha256(b"".join(bytes.fromhex(pins[k]) for k in ("xml", "reset", "noise", "profile"))).digest()
            initial = struct.unpack("<d", bytes.fromhex(offline["initial_time_u64_hex"])[::-1])[0]
            need(math.isfinite(initial), "finite frozen initial time")
            proof = reader.trace((case / "physics.ptr").read_bytes(), native_case, initial, words, seed)
            need(proof == independent["trace_audit"], "same canonical committed trace proof")
            r["trace_audit"] = proof
            r["task_success_observed"] = native_case["success"]
            r["first_hash_matches_prior_DIAGNOSTIC_ONLY"] = identity(case / "first-decoded.f32")["sha256"] == e["first_decoded_diagnostic"]["prior_SHA256"]
        metadata(f)
        r["passed"] = True
    except BaseException as exc:
        r["error"] = type(exc).__name__ + ": " + str(exc)
        raise
    finally:
        r["seconds"] = time.monotonic() - began
        if out:
            (out / "record.json").write_text(json.dumps(r, indent=2) + "\n")
    print(json.dumps({"passed": True, "plan_only": args.plan, "SDK_NN_render_calls": 0,
                      "output": str(out) if out else None}))


if __name__ == "__main__":
    main()
