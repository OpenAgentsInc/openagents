#!/usr/bin/env python3
"""Focused current-compile CPU-F64 GAE/PPO native qualification; no learner.

Consumes immutable independently generated tables. It does not generate targets,
run an environment, invoke a model/RNG, or repeat source/mutation gates.
"""
from __future__ import annotations
import argparse
import hashlib
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
FIXTURE = ROOT / "psionic/fixtures/robotics/locomotion"
ENTRY = ROOT / "psionic/lib/robotics/locomotion.coax"
NATIVE = ROOT / "psionic/tests/robotics/locomotion_native.c"
EXPERIMENT = "5a4916124d35b051cc5f408e3cb33a68eebba2b94c765619cedcabb3387b2008"
GAE = struct.Struct("<3d2q")
PPO = struct.Struct("<4dq")


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


def graph(path, result=None):
    result = {} if result is None else result
    path = Path(path).resolve()
    key = str(path.relative_to(ROOT))
    if key in result:
        return result
    result[key] = identity(path)
    for name in re.findall(r'#load\s+"([^\"]+)"', path.read_text()):
        graph(path.parent / name, result)
    return result


def snapshot(oracle):
    e = load(FIXTURE / "experiment.json")
    need(identity(FIXTURE / "experiment.json")["sha256"] == EXPERIMENT, "precomparison experiment")
    ref = load(oracle / "reference.json")
    need(ref["schema"] == "psionic.locomotion-foundation.reference.v1" and ref["passed"] is True, "FIRST independent oracle")
    need(ref["experiment"] == identity(FIXTURE / "experiment.json") and ref["counts"] == e["counts"], "oracle experiment/count association")
    need(load(FIXTURE / "input-freeze.json")["experiment"] == ref["experiment"], "original input freeze")
    pinned = {}
    for name, expected in e["input_files"].items():
        need(identity(FIXTURE / name) == expected and ref["input_files"][name] == expected, "input pin: " + name)
        pinned[str(FIXTURE / name)] = expected
    for name, expected in ref["outputs"].items():
        need(identity(oracle / name) == expected, "expected output: " + name)
        pinned[str(oracle / name)] = expected
    for name, expected in ref["source"].items():
        need(identity(ROOT / name) == expected, "independent producer source")
        pinned[str(ROOT / name)] = expected
    q = load(FIXTURE / "source-qualification.json")
    need(q["passed"] is True and q["survived"] == 0 and q["units"] == identity(FIXTURE / "unit-refusals.json"), "new source/unit proof")
    for name, expected in q["source_files"].items():
        need(identity(ROOT / name) == expected, "frozen qualified Coax source")
    sources = graph(ENTRY)
    sources[str(NATIVE.relative_to(ROOT))] = identity(NATIVE)
    sources[str(Path(__file__).resolve().relative_to(ROOT))] = identity(__file__)
    for name in ("experiment.json", "input-freeze.json", "source-qualification.json", "unit-refusals.json"):
        pinned[str(FIXTURE / name)] = identity(FIXTURE / name)
    pinned[str(oracle / "reference.json")] = identity(oracle / "reference.json")
    return e, {"source": sources, "input_and_oracle": pinned}


def decode(raw, rows, kind):
    fmt = GAE if kind == "gae" else PPO
    numeric = 3 if kind == "gae" else 4
    need(len(raw) == rows * 40, kind + " exact record extent")
    data = [fmt.unpack_from(raw, 40 * i) for i in range(rows)]
    finite([v for row in data for v in row[:numeric]], rows * numeric, kind + " ALL payload words before maxima")
    need(all(v in (0, 1) for row in data for v in row[numeric:]), kind + " canonical mask/active integers")
    return data


def preflight(e, oracle):
    ng, np = e["counts"]["gae_rows"], e["counts"]["PPO_cases"]
    configs = (FIXTURE / "gae-cases.bin").read_bytes()
    steps = (FIXTURE / "gae-steps.bin").read_bytes()
    ppo = (FIXTURE / "ppo-inputs.f64").read_bytes()
    need(len(configs) == 48 * e["counts"]["gae_cases"] and len(steps) == 40 * ng and len(ppo) == 24 * np, "complete input extents")
    sr = [GAE.unpack_from(steps, 40 * i) for i in range(ng)]
    pr = [struct.unpack_from("<3d", ppo, 24 * i) for i in range(np)]
    finite([x for row in sr for x in row[:3]], 3 * ng, "GAE input")
    finite([x for row in pr for x in row], 3 * np, "PPO input")
    need(all(all(abs(x) <= 1e6 for x in r[:3]) and r[3] in (0, 1) and r[4] in (0, 1) for r in sr), "GAE admitted input domains")
    need(all(2.**-20 <= r <= 16 and abs(a) <= 1e9 and 0 <= eps <= 1 for r, a, eps in pr), "PPO admitted input domains")
    want_g_raw = (oracle / "gae-expected.bin").read_bytes()
    want_p_raw = (oracle / "ppo-expected.bin").read_bytes()
    want_g = decode(want_g_raw, ng, "gae")
    want_p = decode(want_p_raw, np, "ppo")
    bound_raw = (oracle / "gae-budgets.f64").read_bytes()
    need(len(bound_raw) == 24 * ng, "row-conditioned bound extent")
    bounds = [struct.unpack_from("<3d", bound_raw, i * 24) for i in range(ng)]
    finite([x for r in bounds for x in r], 3 * ng, "ALL predeclared budgets")
    need(all(r[0] == 0 and r[1] > 0 and r[2] >= r[1] for r in bounds), "predeclared exact-delta/positive outward budgets")
    exact = (oracle / "gae-exact.u8").read_bytes()
    need(len(exact) == ng and all(x in (1, 7) for x in exact), "exact-word mask inventory")
    offset = 0
    for index, case in enumerate(e["gae_cases"]):
        W, H, at, n, gamma, lam = struct.unpack_from("<4q2d", configs, index * 48)
        need((W, H, at, n) == (case["worlds"], case["horizon"], case["offset_steps"], case["count"]), "case header agrees with frozen manifest")
        need(1 <= W <= 16 and 1 <= H <= 128 and n == W * H and at == offset and at + n <= ng, "axes/product/contiguous row partition")
        finite([gamma, lam], 2, "hyperparameters")
        need(struct.pack("<2d", gamma, lam) == struct.pack("<2d", case["gamma"], case["lambda_"]) and 0 <= gamma <= 1 and 0 <= lam <= 1, "exact frozen hyperparameter bits")
        for i in range(n):
            s = sr[at + i]
            b, c = int(not s[3]), int(not (s[3] or s[4] or i % H == H - 1))
            need(want_g[at + i][3:] == (b, c), "oracle masks independently bound to termination/truncation/local tail")
            need(exact[at + i] == (1 if c else 7), "one-row segments have all three exact words")
        offset += n
    need(offset == ng and len(pr) == len(e["PPO_cases"]), "complete case inventories")
    for row, declared in zip(pr, e["PPO_cases"]):
        need(struct.pack("<3d", *row) == struct.pack("<3d", declared["rho"], declared["advantage"], declared["epsilon"]), "exact PPO input bits/row order")
    return {"gae": want_g, "ppo": want_p, "gae_raw": want_g_raw, "ppo_raw": want_p_raw,
            "bounds": bounds, "exact": exact, "ppo_inputs": pr}


def compare(gae_raw, ppo_raw, wanted):
    # Decode every numeric word in both actual streams before ANY maximum.
    a = decode(gae_raw, len(wanted["gae"]), "gae")
    p = decode(ppo_raw, len(wanted["ppo"]), "ppo")
    maxima = [0., 0., 0.]
    ratios = [0., 0., 0.]
    exact_words = 0
    for i, (actual, expected, budget) in enumerate(zip(a, wanted["gae"], wanted["bounds"])):
        need(actual[3:] == expected[3:], "actual GAE bootstrap/trace masks")
        for j in range(3):
            error = abs(actual[j] - expected[j])
            if wanted["exact"][i] & (1 << j):
                need(gae_raw[i * 40 + j * 8:i * 40 + j * 8 + 8] == wanted["gae_raw"][i * 40 + j * 8:i * 40 + j * 8 + 8], "exact GAE word bits")
                exact_words += 1
            else:
                need(error <= budget[j], "predeclared row-conditioned GAE budget")
                ratios[j] = max(ratios[j], error / budget[j])
            maxima[j] = max(maxima[j], error)
    need(ppo_raw == wanted["ppo_raw"], "ALL PPO IEEE words and active integers exact")
    zero_a = knots = active = 0
    for actual, (rho, adv, eps) in zip(p, wanted["ppo_inputs"]):
        lower, upper = 1. - eps, 1. + eps
        flag = int((adv > 0 and rho > upper) or (adv < 0 and rho < lower))
        need(actual[4] == flag, "strict sign/rho active predicate, independent of rounded product ties")
        active += flag
        if adv == 0:
            need(all(struct.pack("<d", x) == bytes(8) for x in actual[:3]) and flag == 0, "canonical zero advantage outputs")
            zero_a += 1
        if rho == lower or rho == upper:
            need(flag == 0, "unclipped exact rounded-knot derivative branch")
            knots += 1
    return {"passed": True, "GAE_rows": len(a), "GAE_finite_words": 3 * len(a),
            "GAE_exact_word_comparisons": exact_words, "GAE_max_absolute_errors": maxima,
            "GAE_max_fraction_of_frozen_budget": ratios, "GAE_all_masks_exact": True,
            "PPO_rows": len(p), "PPO_finite_words_bit_exact": 4 * len(p),
            "PPO_active_flags_exact": True, "PPO_clipped_active_rows": active,
            "PPO_zero_advantage_rows": zero_a, "PPO_exact_rounded_knot_rows": knots}


def reader_guards():
    tested = 0
    for kind, fmt, row, numeric in (("gae", GAE, (1., 2., 3., 1, 0), 3), ("ppo", PPO, (1., -1., -1., 1., 0), 4)):
        good = fmt.pack(*row)
        decode(good, 1, kind)
        bad = [good[:-1], good + b"\0"]
        for j in range(numeric):
            for value in (float("nan"), float("inf"), -float("inf")):
                b = bytearray(good)
                struct.pack_into("<d", b, j * 8, value)
                bad.append(bytes(b))
        for j in range(numeric, 5):
            b = bytearray(good)
            struct.pack_into("<q", b, j * 8, 2)
            bad.append(bytes(b))
        for b in bad:
            try:
                decode(b, 1, kind)
            except ValueError:
                tested += 1
            else:
                raise ValueError("new finite/extent/mask reader admitted poison")
    return {"passed": True, "new_poison_cases": tested, "models_environments_old_gates": 0}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--coax", type=Path, default=Path("/tmp/coax-spatial-fixture-fixes-reconstruction/coax"))
    p.add_argument("--work-dir", type=Path, required=True)
    p.add_argument("--oracle", type=Path, default=FIXTURE)
    p.add_argument("--plan", action="store_true")
    args = p.parse_args()
    out, oracle, compiler = args.work_dir.resolve(), args.oracle.resolve(), args.coax.resolve()
    out.mkdir(parents=True, exist_ok=False)
    record = {"schema": "psionic.locomotion-foundation.native.v1", "passed": False,
              "old_source_or_mutation_gates": 0, "model_environment_RNG_optimizer_calls": 0,
              "commands": [], "modes": {}, "plan_only": args.plan}
    began = time.monotonic()

    def command(label, argv, env=None):
        item = {"label": label, "argv": [str(x) for x in argv]}
        record["commands"].append(item)
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
            write(out / "record.json", record)

    try:
        need(platform.system() == "Darwin" and platform.machine() == "arm64" and sys.byteorder == "little", "first qualified Darwin-arm64 scalar F64 profile")
        e, before = snapshot(oracle)
        record.update(before, experiment=identity(FIXTURE / "experiment.json"), reference=identity(oracle / "reference.json"), compiler=identity(compiler), counts=e["counts"])
        if not args.plan:
            expected = preflight(e, oracle)
            record["reader_guards"] = reader_guards()
            record["host_profile"] = {"system": platform.system(), "machine": platform.machine(), "python": sys.version,
                                       "compiler_version": command("host-compiler-version", [os.environ.get("CC", "cc"), "--version"]).decode()}
            emit = {}
            record["emissions"] = {}
            for mode, extra in (("debug", []), ("release", ["--release"])):
                d = out / mode
                d.mkdir()
                c, h = d / "locomotion.c", d / "locomotion.h"
                c.write_bytes(command("emit-" + mode, [compiler, "c", ENTRY.relative_to(ROOT), "--library", *extra, "--header", h]))
                emit[mode] = (c, h)
                record["emissions"][mode] = {"C": identity(c), "header": identity(h)}
            common = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-ffp-contract=off", "-fno-fast-math"]
            streams = []
            for mode, emission, flags in (("debug", "debug", ["-O0", "-g"]), ("release", "release", ["-O2"]),
                                          ("sanitized", "debug", ["-O1", "-g", "-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"])):
                d = out / ("native-" + mode)
                d.mkdir()
                c, h = emit[emission]
                need(record["emissions"][emission] == {"C": identity(c), "header": identity(h)}, "reused emitted C/H identity")
                cc = os.environ.get("CC", "cc")
                command("compile-generated-" + mode, [cc, *common, "-Wno-unused-parameter", *flags, "-c", c, "-o", d / "coax.o"])
                command("compile-effects-" + mode, [cc, *common, *flags, "-I" + str(h.parent), NATIVE, d / "coax.o", "-lm", "-o", d / "native"])
                env = os.environ.copy()
                env.update(ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
                report = json.loads(command("execute-" + mode, [d / "native", FIXTURE / "gae-cases.bin", FIXTURE / "gae-steps.bin", FIXTURE / "ppo-inputs.f64", d], env))
                need((report["gae_cases"], report["gae_rows"], report["ppo_rows"]) == (e["counts"]["gae_cases"], e["counts"]["gae_rows"], e["counts"]["PPO_cases"]), "native complete row inventory")
                g, q = (d / "gae-actual.bin").read_bytes(), (d / "ppo-actual.bin").read_bytes()
                record["modes"][mode] = {"native": report, "comparison": compare(g, q, expected), "C": identity(c), "header": identity(h),
                                          "object": identity(d / "coax.o"), "binary": identity(d / "native"),
                                          "GAE": identity(d / "gae-actual.bin"), "PPO": identity(d / "ppo-actual.bin")}
                streams.append((g, q))
                write(out / "record.json", record)
            need(all(s == streams[0] for s in streams), "all three changed native contexts bit-identical")
            record["all_native_output_bytes_exact"] = True
            for mode, (c, h) in emit.items():
                need(record["emissions"][mode] == {"C": identity(c), "header": identity(h)}, "final emitted C/H immutability")
        _, after = snapshot(oracle)
        need(after == before and identity(compiler) == record["compiler"], "source/oracle/input/compiler context immutable")
        record["passed"] = True
    except BaseException as exc:
        record["error"] = type(exc).__name__ + ": " + str(exc)
        raise
    finally:
        record["seconds"] = time.monotonic() - began
        write(out / "record.json", record)
    print(json.dumps({"passed": True, "record": str(out / "record.json"), "plan_only": args.plan}))


if __name__ == "__main__":
    main()
