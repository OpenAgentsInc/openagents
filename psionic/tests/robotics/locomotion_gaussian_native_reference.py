#!/usr/bin/env python3
"""Focused scalar Gaussian native qualification using frozen independent data.

No model, environment, learner, optimizer or old RNG/PPO gate is run. Probe
accuracy is checked independently BEFORE conditional exact PPO composition.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
F = ROOT / "psionic/fixtures/robotics/locomotion-gaussian"
ENTRY = ROOT / "psionic/tests/robotics/locomotion-gaussian-native.coax"
NATIVE = ROOT / "psionic/tests/robotics/locomotion_gaussian_native.c"
PRODUCER = ROOT / "psionic/tests/robotics/locomotion_gaussian_reference.py"
EXPERIMENT = "89c762bbb8154e86fdaf69ea3e686f5b4a5b35479e6a726c19a7822ba02b1eb7"
FORMATS = {"density": struct.Struct("<q4d"), "uniform": struct.Struct("<q2d"),
           "seeded": struct.Struct("<q2d2Q"), "ratio": struct.Struct("<q6dq")}
NUMERIC = {"density": 4, "uniform": 2, "seeded": 2, "ratio": 6}
INPUT_NAMES = ["density-inputs.f64", "uniform-inputs.f64", "seeded-inputs.bin", "ratio-inputs.f64", "ratio-eligibility.u8", "ratio-dlog.f64"]


def need(ok, text):
    if not ok:
        raise ValueError(text)


def identity(path):
    b = Path(path).read_bytes()
    return {"bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()}


def load(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def finite(values, count, role):
    need(len(values) == count and all(math.isfinite(x) for x in values), role + " extent/ALL finite")


def records(raw, fmt, count, role):
    need(len(raw) == fmt.size * count, role + " exact extent")
    return list(fmt.iter_unpack(raw))


def graph(path, result=None):
    result = {} if result is None else result
    path = Path(path).resolve()
    name = str(path.relative_to(ROOT))
    if name in result:
        return result
    result[name] = identity(path)
    for child in re.findall(r'#load\s+"([^\"]+)"', path.read_text()):
        graph(path.parent / child, result)
    return result


def snapshot(oracle):
    need(identity(F / "experiment.json")["sha256"] == EXPERIMENT, "sealed experiment")
    e, seal, ref = load(F / "experiment.json"), load(F / "input-freeze.json"), load(oracle / "reference.json")
    need(seal["experiment"] == identity(F / "experiment.json") and seal["producer"] == identity(PRODUCER), "whole producer frozen before expected generation")
    need(ref["schema"] == "psionic.locomotion-gaussian.reference.v1" and ref["passed"] is True and ref["experiment"] == seal["experiment"] and ref["producer"] == seal["producer"], "original FIRST reference association")
    need(ref["counts"] == e["counts"], "frozen count association")
    need(identity(F / "contract.md") == e["contract"], "sealed corrected numerical contract")
    pinned = {}
    for name, expected in e["inputs"].items():
        need(identity(F / name) == expected, "frozen input: " + name)
        pinned[str(F / name)] = expected
    for group in ("carried", "endpoint_original_derivation", "coefficient_correction_before_first_generation"):
        for name, expected in e[group].items():
            path = ROOT / name if group == "carried" else F / name
            need(identity(path) == expected, "carried/corrected evidence: " + name)
            pinned[str(path)] = expected
    for name, expected in ref["outputs"].items():
        need(identity(oracle / name) == expected, "frozen expected output: " + name)
        pinned[str(oracle / name)] = expected
    q = load(F / "source-qualification.json")
    need(q["passed"] is True and q["survived"] == 0 and q["units"] == identity(F / "unit-refusals.json"), "new source/unit proofs")
    for name, expected in q["source_files"].items():
        need(identity(ROOT / name) == expected, "qualified new source")
    for name in ("experiment.json", "input-freeze.json", "contract.md", "source-qualification.json", "unit-refusals.json", "endpoint-enclosure-v1.json", "sampling-call-closure-carry.json"):
        pinned[str(F / name)] = identity(F / name)
    pinned[str(oracle / "reference.json")] = identity(oracle / "reference.json")
    pinned[str(PRODUCER)] = identity(PRODUCER)
    source = graph(ENTRY)
    source[str(NATIVE.relative_to(ROOT))] = identity(NATIVE)
    source[str(Path(__file__).resolve().relative_to(ROOT))] = identity(__file__)
    return e, {"source": source, "frozen_inputs_oracles_evidence": pinned}


def reader():
    spec = importlib.util.spec_from_file_location("gaussian_independent", PRODUCER)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def decode(raw, kind, count):
    result = records(raw, FORMATS[kind], count, kind)
    n = NUMERIC[kind]
    finite([x for row in result for x in row[1:1 + n]], count * n, kind + " ALL payload before maxima")
    for i, row in enumerate(result):
        need(type(row[0]) is int and 0 <= row[0] <= (4 if kind == "seeded" else 3), kind + " status")
        if row[0]:
            need(raw[i * FORMATS[kind].size + 8:(i + 1) * FORMATS[kind].size] == bytes(FORMATS[kind].size - 8), "unavailable transport is canonical zero AFTER native atomicity assertions")
        elif kind == "ratio":
            need(row[-1] in (0, 1), "canonical active flag")
    return result


def preflight(e, oracle, independent):
    counts = {"density": e["counts"]["density"], "uniform": e["counts"]["uniforms"], "seeded": e["counts"]["seeded"], "ratio": e["counts"]["ratio"]}
    formats = {"density": "<3d", "uniform": "<4d", "seeded": "<2Q2d", "ratio": "<4d"}
    input_files = {"density": "density-inputs.f64", "uniform": "uniform-inputs.f64", "seeded": "seeded-inputs.bin", "ratio": "ratio-inputs.f64"}
    inputs = {kind: records((F / input_files[kind]).read_bytes(), struct.Struct(formats[kind]), count, kind + " inputs") for kind, count in counts.items()}
    for kind, rows in inputs.items():
        numeric = [x for row in rows for x in (row[2:] if kind == "seeded" else row)]
        finite(numeric, len(rows) * (2 if kind == "seeded" else 3 if kind == "density" else 4), kind + " input words")
    raw = {k: (oracle / (k + "-expected.bin")).read_bytes() for k in counts}
    expected = {k: decode(raw[k], k, count) for k, count in counts.items()}
    masks = {k: (oracle / (k + "-exact.u8")).read_bytes() for k in ("density", "uniform", "seeded")}
    for k in masks:
        need(len(masks[k]) == counts[k] and all(x in ((8, 15) if k == "density" else (0, 3)) for x in masks[k]), "complete exact-word masks")
    eligible = (F / "ratio-eligibility.u8").read_bytes()
    need(len(eligible) == counts["ratio"] and all(x in (0, 1) for x in eligible), "probe request inventory")
    dlogs = [r[0] for r in records((F / "ratio-dlog.f64").read_bytes(), struct.Struct("<d"), counts["ratio"], "probe dlog")]
    finite(dlogs, counts["ratio"], "probe inputs")
    lo, hi = independent.bounds()
    need(struct.pack("<d", lo).hex() == bytes.fromhex(e["constant_bits"]["lower"])[::-1].hex() and struct.pack("<d", hi).hex() == bytes.fromhex(e["constant_bits"]["upper"])[::-1].hex(), "exact inward endpoint bits")
    for i, (new, old, a, eps) in enumerate(inputs["ratio"]):
        need(abs(a) <= 1e9 and 0 <= eps <= 1, "ratio frozen advantage/epsilon domains")
        d = independent.sub(new, old)
        requested = int(math.isfinite(d) and lo <= d <= hi)
        need(eligible[i] == requested, "probe eligibility from exact RN subtraction and safe interval")
        need(struct.pack("<d", dlogs[i]) == struct.pack("<d", d if requested else 0.), "exact dlog/request bytes")
        need(expected["ratio"][i][0] == (0 if requested else 3), "ideal-rho reference status; actual composition remains conditional")
    for i, (mean, logstd, action) in enumerate(inputs["density"]):
        need(abs(mean) <= 16 and -4 <= logstd <= 2 and abs(action) <= 128, "density corpus domain")
        need(masks["density"][i] == (15 if mean == action else 8), "density equality/entropy exact-word masks")
    for i, (mean, logstd, radial, angle) in enumerate(inputs["uniform"]):
        need(abs(mean) <= 16 and -4 <= logstd <= 2 and 0 <= radial < 1 and 0 <= angle < 1, "uniform corpus domain")
        need(masks["uniform"][i] == (3 if independent.sub(1., radial) == 1 else 0), "literal/tiny radial exact-word mask")
    for i, (seed, counter, mean, logstd) in enumerate(inputs["seeded"]):
        need(abs(mean) <= 16 and -4 <= logstd <= 2, "seeded corpus domain")
        row = expected["seeded"][i]
        need(row[0] == (4 if counter > (1 << 64) - 3 else 0), "two-draw counter headroom")
        if row[0] == 0:
            need(row[3:] == (seed, counter + 2), "exact expected counter/seed transition")
    return {"counts": counts, "inputs": inputs, "raw": raw, "expected": expected,
            "masks": masks, "eligible": eligible, "dlogs": dlogs}


def compare(streams, probe_raw, data, e, independent):
    # Complete all four streams AND the probe finite admission before reductions.
    actual = {k: decode(streams[k], k, n) for k, n in data["counts"].items()}
    probes = [r[0] for r in records(probe_raw, struct.Struct("<d"), data["counts"]["ratio"], "actual exp probe")]
    finite(probes, len(probes), "ALL actual probe outputs")
    maximum = {"density": [0.] * 4, "uniform": [0.] * 2, "seeded": [0.] * 2}
    fractions = {k: [0.] * len(v) for k, v in maximum.items()}
    exact_words = {k: 0 for k in maximum}
    density_zero_sign_checks = 0
    statuses = {k: {} for k in actual}
    for kind in ("density", "uniform", "seeded"):
        budget = e["budgets"]["density" if kind == "density" else "sample"]
        for i, (row, expected) in enumerate(zip(actual[kind], data["expected"][kind])):
            need(row[0] == expected[0], kind + " exact status")
            statuses[kind][str(row[0])] = statuses[kind].get(str(row[0]), 0) + 1
            if row[0]:
                continue
            for j in range(NUMERIC[kind]):
                error = abs(row[1 + j] - expected[1 + j])
                if data["masks"][kind][i] & (1 << j):
                    start = i * FORMATS[kind].size + 8 + 8 * j
                    need(streams[kind][start:start + 8] == data["raw"][kind][start:start + 8], kind + " exact field bits")
                    exact_words[kind] += 1
                else:
                    allowance = budget["absolute"] + budget["relative"] * abs(expected[1 + j])
                    need(error <= allowance, kind + " frozen per-scalar smooth-field budget")
                    fractions[kind][j] = max(fractions[kind][j], error / allowance)
                maximum[kind][j] = max(maximum[kind][j], error)
            if kind == "seeded":
                need(row[3:] == expected[3:], "actual exact next seed/counter")
            if kind == "density":
                mean, _, action = data["inputs"][kind][i]
                delta = 0. if mean == action else independent.sub(action, mean)
                if row[2] == 0:
                    need(independent.bits(row[2]) >> 63 == independent.bits(delta) >> 63, "zero mean-score preserves signed displacement except canonical equality")
                    density_zero_sign_checks += 1
                for j in (1, 3, 4):
                    if row[j] == 0:
                        need(struct.pack("<d", row[j]) == bytes(8), "density cancellation zero is positive")
                        density_zero_sign_checks += 1
    probe_max_relative = 0.
    # Independently bound ALL requested exp outputs before using ANY of them to
    # construct conditional expected Rcp fields or public status.
    for i, requested in enumerate(data["eligible"]):
        if requested:
            reference_rho = data["expected"]["ratio"][i][2]
            error = abs(probes[i] - reference_rho)
            need(error <= e["budgets"]["ratio_rho"]["relative"] * abs(reference_rho), "independent Decimal probe-rho accuracy")
            probe_max_relative = max(probe_max_relative, error / abs(reference_rho))
        else:
            need(probe_raw[i * 8:(i + 1) * 8] == bytes(8), "no probe for unsafe/ineligible row")
    conditional_success = 0
    for i, (row, inp) in enumerate(zip(actual["ratio"], data["inputs"]["ratio"])):
        expected = independent.ratio_expected_from_probe(inp, data["dlogs"][i], bool(data["eligible"][i]), probes[i])
        need(streams["ratio"][i * 64:(i + 1) * 64] == expected, "exact conditional public status/dlog/rho/Rcp composition")
        statuses["ratio"][str(row[0])] = statuses["ratio"].get(str(row[0]), 0) + 1
        if row[0] == 0:
            need(struct.pack("<d", row[2]) == probe_raw[i * 8:(i + 1) * 8], "public rho equals separately bounded probe bytes")
            conditional_success += 1
    return {"passed": True, "maximum_absolute_errors": maximum, "maximum_fraction_of_frozen_budget": fractions,
            "exact_smooth_field_words": exact_words, "statuses": statuses,
            "probe_rows": sum(data["eligible"]), "probe_max_relative_error": probe_max_relative,
            "conditional_ratio_successes": conditional_success,
            "conditional_ratio_scope": "Independent Decimal exp accuracy followed by exact Fraction/RN Rcp on observed rho; not correctly-rounded-libm proof.",
            "density_zero_sign_checks": density_zero_sign_checks,
            "all_seed_counter_transitions_exact": True, "all_finite_before_maxima": True}


def reader_guards():
    n = 0
    for kind, fmt in FORMATS.items():
        numeric = NUMERIC[kind]
        tail = [7, 9] if kind == "seeded" else [0] if kind == "ratio" else []
        good = fmt.pack(0, *([.25] * numeric), *tail)
        decode(good, kind, 1)
        invalid = [good[:-1], good + b"\0"]
        for lane in range(numeric):
            for value in (float("nan"), float("inf"), -float("inf")):
                b = bytearray(good)
                struct.pack_into("<d", b, 8 + lane * 8, value)
                invalid.append(bytes(b))
        b = bytearray(good)
        struct.pack_into("<q", b, 0, 99)
        invalid.append(bytes(b))
        b = bytearray(good)
        struct.pack_into("<q", b, 0, 3)
        invalid.append(bytes(b))
        if kind == "ratio":
            b = bytearray(good)
            struct.pack_into("<q", b, 56, 2)
            invalid.append(bytes(b))
        for raw in invalid:
            try:
                decode(raw, kind, 1)
            except ValueError:
                n += 1
            else:
                raise ValueError("new Gaussian reader accepted poison")
    return {"passed": True, "poisoned_records": n, "old_gates_models_environments": 0}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--coax", type=Path, default=ROOT / "coax/bin/coax")
    p.add_argument("--work-dir", type=Path, required=True)
    p.add_argument("--oracle", type=Path, default=F)
    p.add_argument("--plan", action="store_true")
    args = p.parse_args()
    out, oracle, compiler = args.work_dir.resolve(), args.oracle.resolve(), args.coax.resolve()
    out.mkdir(parents=True, exist_ok=False)
    r = {"schema": "psionic.locomotion-gaussian.native.v1", "passed": False, "plan_only": args.plan,
         "commands": [], "modes": {}, "model_SDK_environment_optimizer_training_calls": 0,
         "old_source_or_native_gates": 0}
    began = time.monotonic()

    def command(label, argv, env=None):
        item = {"label": label, "argv": [str(x) for x in argv]}
        r["commands"].append(item)
        start = time.monotonic()
        try:
            q = subprocess.run(item["argv"], cwd=ROOT, env=env, capture_output=True, timeout=180)
            item.update(exit=q.returncode, seconds=time.monotonic() - start)
            (out / (label + ".stdout")).write_bytes(q.stdout)
            (out / (label + ".stderr")).write_bytes(q.stderr)
            item.update(stdout=identity(out / (label + ".stdout")), stderr=identity(out / (label + ".stderr")))
            need(q.returncode == 0 and not q.stderr, label + " command/empty stderr")
            return q.stdout
        except subprocess.TimeoutExpired as exc:
            item.update(timed_out=True, seconds=time.monotonic() - start)
            (out / (label + ".stdout")).write_bytes(exc.stdout or b"")
            (out / (label + ".stderr")).write_bytes(exc.stderr or b"")
            raise
        finally:
            write(out / "record.json", r)

    try:
        need(platform.system() == "Darwin" and platform.machine() == "arm64" and sys.byteorder == "little", "first scalar backend profile")
        e, before = snapshot(oracle)
        r.update(before, experiment=identity(F / "experiment.json"), independent_reference=identity(oracle / "reference.json"), compiler=identity(compiler), counts=e["counts"])
        if not args.plan:
            independent = reader()
            expected = preflight(e, oracle, independent)
            r["reader_guards"] = reader_guards()
            r["host_profile"] = {"system": platform.system(), "machine": platform.machine(), "python": sys.version,
                                  "C_compiler_version": command("host-compiler-version", [os.environ.get("CC", "cc"), "--version"]).decode()}
            emit = {}
            r["emissions"] = {}
            for mode, extra in (("debug", []), ("release", ["--release"])):
                d = out / mode
                d.mkdir()
                c, h = d / "locomotion-gaussian.c", d / "locomotion-gaussian.h"
                c.write_bytes(command("emit-" + mode, [compiler, "c", ENTRY.relative_to(ROOT), "--library", *extra, "--header", h]))
                emit[mode] = (c, h)
                r["emissions"][mode] = {"C": identity(c), "header": identity(h)}
            common = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-ffp-contract=off", "-fno-fast-math",
                      "-fno-builtin-exp", "-fno-builtin-log", "-fno-builtin-sqrt", "-fno-builtin-cos"]
            payloads = []
            for mode, source_mode, flags in (("debug", "debug", ["-O0", "-g"]), ("release", "release", ["-O2"]),
                                             ("sanitized", "debug", ["-O1", "-g", "-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"])):
                d = out / ("native-" + mode)
                d.mkdir()
                c, h = emit[source_mode]
                need(r["emissions"][source_mode] == {"C": identity(c), "header": identity(h)}, "emitted input identity")
                cc = os.environ.get("CC", "cc")
                command("compile-generated-" + mode, [cc, *common, "-Wno-unused-parameter", *flags, "-c", c, "-o", d / "coax.o"])
                command("compile-effects-" + mode, [cc, *common, *flags, "-I" + str(h.parent), NATIVE, d / "coax.o", "-lm", "-o", d / "native"])
                env = os.environ.copy()
                env.update(ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
                report = json.loads(command("execute-" + mode, [d / "native", *[F / n for n in INPUT_NAMES], d], env))
                need((report["density_rows"], report["uniform_rows"], report["seeded_rows"], report["ratio_rows"], report["probe_calls"]) ==
                     (e["counts"]["density"], e["counts"]["uniforms"], e["counts"]["seeded"], e["counts"]["ratio"], sum(expected["eligible"])), "native complete corpus/probe inventory")
                streams = {k: (d / (k + "-actual.bin")).read_bytes() for k in FORMATS}
                probe = (d / "ratio-exp.bin").read_bytes()
                r["modes"][mode] = {"native": report, "comparison": compare(streams, probe, expected, e, independent),
                                    "C": identity(c), "header": identity(h), "object": identity(d / "coax.o"), "binary": identity(d / "native"),
                                    "outputs": {n: identity(d / n) for n in [*[k + "-actual.bin" for k in FORMATS], "ratio-exp.bin"]}}
                payloads.append((streams, probe))
                write(out / "record.json", r)
            need(all(x == payloads[0] for x in payloads), "all three scalar native output streams bit-identical")
            r["all_mode_output_bytes_exact"] = True
            for mode, (c, h) in emit.items():
                need(r["emissions"][mode] == {"C": identity(c), "header": identity(h)}, "final emitted C/H identities")
        _, after = snapshot(oracle)
        need(before == after and identity(compiler) == r["compiler"], "frozen source/input/reference/compiler context")
        r["passed"] = True
    except BaseException as exc:
        r["error"] = type(exc).__name__ + ": " + str(exc)
        raise
    finally:
        r["seconds"] = time.monotonic() - began
        write(out / "record.json", r)
    print(json.dumps({"passed": True, "record": str(out / "record.json"), "plan_only": args.plan}))


if __name__ == "__main__":
    main()
