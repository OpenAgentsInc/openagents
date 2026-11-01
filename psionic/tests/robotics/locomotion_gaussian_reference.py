#!/usr/bin/env python3
"""Frozen normalized-Gaussian fixtures and independent decimal reference.

The input-freeze phase evaluates no Gaussian, RNG, product or host math.  The
separate generate phase is one-shot tooling: Decimal smooth density functions,
independently reduced cosine, exact local Fraction rounding, and the published
counter mapping only to compose the new two-draw sampler.  No learner exists.
"""
from __future__ import annotations
import argparse
from decimal import Decimal, localcontext
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import struct
import time

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "psionic/fixtures/robotics/locomotion-gaussian"
SOURCE = Path(__file__).resolve()
LOWER_BITS = 0xC02BB9D3BEB8C86B
UPPER_BITS = 0x40062E42FEFA39EF
MAX64 = (1 << 64) - 1
C = 0.9189385332046728
TWO_PI = 6.283185307179586
PI = Decimal("3.141592653589793238462643383279502884197169399375105820974944592307816406286208998628034825342117067982148086513282306647")
DENSITY = struct.Struct("<q4d")
UNIFORM = struct.Struct("<q2d")
SEEDED = struct.Struct("<q2d2Q")
RATIO = struct.Struct("<q6dq")


def need(ok, message):
    if not ok:
        raise ValueError(message)


def identity(path):
    data = Path(path).read_bytes()
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def bits(value):
    return struct.unpack("<Q", struct.pack("<d", value))[0]


def from_bits(value):
    return struct.unpack("<d", struct.pack("<Q", value))[0]


def negative(value):
    return bool(bits(value) >> 63)


def rn(value, negative_zero=False):
    if value == 0:
        return -0.0 if negative_zero else 0.0
    try:
        result = float(value)
    except OverflowError:
        return math.inf if value > 0 else -math.inf
    if result == 0:
        return -0.0 if value < 0 else 0.0
    return result


def add(a, b):
    return rn(Fraction(a) + Fraction(b), a == 0 and b == 0 and negative(a) and negative(b))


def sub(a, b):
    return add(a, -b)


def mul(a, b):
    return rn(Fraction(a) * Fraction(b), negative(a) != negative(b))


def finite(values, count, role):
    need(len(values) == count and all(math.isfinite(x) for x in values), role + " extent/ALL finite")


def records(data, fmt):
    fmt = struct.Struct(fmt)
    need(len(data) % fmt.size == 0, "whole records")
    return list(fmt.iter_unpack(data))


def ppo_from_ratio(rho, advantage, epsilon):
    """Independent exact RN composition on independently bounded OBSERVED rho."""
    need(all(math.isfinite(x) for x in (rho, advantage, epsilon)), "composition finite")
    need(2**-20 <= rho <= 16 and abs(advantage) <= 1e9 and 0 <= epsilon <= 1, "composition domain")
    lower, upper = sub(1.0, epsilon), add(1.0, epsilon)
    clipped = min(max(rho, lower), upper)
    active = int((advantage > 0 and rho > upper) or (advantage < 0 and rho < lower))
    if advantage == 0:
        return (0.0, 0.0, 0.0, clipped, 0)
    plain, limited = mul(rho, advantage), mul(clipped, advantage)
    surrogate = min(plain, limited)
    return (surrogate, -surrogate, 0.0 if active else -plain, clipped, active)


def ratio_expected_from_probe(inputs, dlog, eligible, actual_rho):
    """64B exact conditional output; caller must FIRST Decimal-check probe rho."""
    new, old, advantage, epsilon = inputs
    if not eligible or not math.isfinite(actual_rho) or not 2**-20 <= actual_rho <= 16:
        return RATIO.pack(3, *([0.0] * 6), 0)
    need(bits(sub(new, old)) == bits(dlog), "exact carried dlog")
    return RATIO.pack(0, dlog, actual_rho, *ppo_from_ratio(actual_rho, advantage, epsilon))


def bounds():
    return from_bits(LOWER_BITS), from_bits(UPPER_BITS)


def build_inputs():
    """Data-only arithmetic lattice; no Gaussian/RNG/transcendental calls."""
    tiny, nz = from_bits(1), -0.0
    means = [-16.0, -1.0, -2**-20, nz, 0.0, 2**-20, 1.0, 16.0]
    logs = [-4.0, -1.0, -tiny, nz, 0.0, tiny, 1.0, 2.0]
    actions = [-128.0, -16.0, -1.0, -tiny, nz, 0.0, tiny, 1.0, 16.0, 128.0]
    density = [(m, s, a) for m in means for s in logs for a in actions]
    density += [(m, s, m) for m in means for s in logs]
    density += [(m, s, math.nextafter(m, direction)) for m in means for s in logs for direction in (-math.inf, math.inf)]
    for i in range(128):
        m = -16 + 32 * ((37 * i + 11) % 257) / 256
        s = -4 + 6 * ((71 * i + 29) % 257) / 256
        a = -128 + 256 * ((101 * i + 7) % 257) / 256
        if i % 2:
            a = math.nextafter(a, 0.0)
        density.append((m, s, a))
    radials = [nz, 0.0, tiny, 2**-54, 2**-53, .5, math.nextafter(1., 0.), math.nextafter(math.nextafter(1., 0.), 0.)]
    angles = [nz, 0.0, tiny, math.nextafter(.25, 0.), .25, math.nextafter(.25, 1.), .5, math.nextafter(1., 0.)]
    sm, ss = [-16., nz, 0., 1., 16.], [-4., nz, 0., 2.]
    uniforms = [(m, s, u, v) for m in sm for s in ss for u in radials for v in angles]
    for i in range(64):
        uniforms.append((sm[i % 5], ss[i % 4], ((53*i+17) % 511)/512, ((89*i+31) % 511)/512))
    seeds = [0, 1, 0x61C8864680B583EB, MAX64, 1 << 63, (1 << 32)-1, 1 << 32, 7]
    counters = [0, 1, 17, 1 << 32, 1 << 63, MAX64-2, MAX64-1, MAX64]
    seeded = [(seed, counter, mean, log) for seed in seeds for counter in counters for mean, log in [(-16., -4.), (16., 2.)]]
    seeded += [(seed, counter, nz, 0.) for seed in seeds for counter in [0, MAX64-2, MAX64-1, MAX64]]
    advantages = [-1e9, -1., -tiny, nz, 0., tiny, 1., 1e9]
    epsilons = [0., .125, .2, .5, 1.]
    ratios = [(d, 0., a, e) for d in [-12., -4., -1., nz, 0., .25, 1., 2.] for e in epsilons for a in advantages]
    lower, upper = bounds()
    neighbors = [math.nextafter(lower, -math.inf), lower, math.nextafter(lower, math.inf), math.nextafter(upper, -math.inf), upper, math.nextafter(upper, math.inf)]
    ratios += [(d, 0., a, e) for d in neighbors for e in epsilons for a in advantages]
    largest = from_bits(0x7FEFFFFFFFFFFFFF)
    pairs = [(largest, largest), (-largest, -largest), (largest, -largest), (-largest, largest),
             (largest, math.nextafter(largest, 0.)), (-1e8, -1e8-.5), (-1e8-.5, -1e8),
             (nz, 0.), (tiny, 0.), (-tiny, 0.)]
    ratios += [(new, old, a, e) for new, old in pairs for a in [-1., 1., nz, 0.] for e in [0., .2]]
    need(tuple(map(len, (density, uniforms, seeded, ratios))) == (960, 1344, 160, 640), "frozen counts")
    dlogs, eligibility = [], bytearray()
    for new, old, advantage, epsilon in ratios:
        d = sub(new, old)
        admitted = math.isfinite(d) and lower <= d <= upper
        eligibility.append(int(admitted))
        dlogs.append(d if admitted else 0.)
    files = {
        "density-inputs.f64": b"".join(struct.pack("<3d", *row) for row in density),
        "uniform-inputs.f64": b"".join(struct.pack("<4d", *row) for row in uniforms),
        "seeded-inputs.bin": b"".join(struct.pack("<2Q2d", *row) for row in seeded),
        "ratio-inputs.f64": b"".join(struct.pack("<4d", *row) for row in ratios),
        "ratio-dlog.f64": struct.pack("<640d", *dlogs),
        "ratio-eligibility.u8": bytes(eligibility),
    }
    return files


def freeze(fixture, contract):
    need(not (fixture / "experiment.json").exists(), "immutable input freeze already exists")
    fixture.mkdir(parents=True, exist_ok=True)
    files = build_inputs()
    for name, data in files.items():
        (fixture / name).write_bytes(data)
    (fixture / "contract.md").write_bytes(contract.read_bytes())
    inherited = ["psionic/lib/counter-rng.coax", "psionic/lib/robotics/locomotion/ppo-surrogate.coax",
                 "psionic/lib/robotics/locomotion/types.coax", "psionic/fixtures/robotics/locomotion/native-qualification.json"]
    experiment = {
        "schema": "psionic.locomotion-gaussian.experiment.v1", "frozen_before_expected_and_product_comparison": True,
        "contract": identity(fixture / "contract.md"),
        "counts": {"density": 960, "uniforms": 1344, "seeded": 160, "ratio": 640},
        "partitions": {"density": [640,64,128,128], "uniforms": [1280,64], "seeded": [128,32], "ratio": [320,240,80]},
        "input_layouts": {"density-inputs.f64": "<3d mean,logstd,action", "uniform-inputs.f64": "<4d mean,logstd,radial,angle",
                          "seeded-inputs.bin": "<2Q2d seed,counter,mean,logstd", "ratio-inputs.f64": "<4d new,old,advantage,epsilon",
                          "ratio-dlog.f64": "<d exact RN(new-old),positive-zero when ineligible; eligibility byte is authoritative", "ratio-eligibility.u8": "one 0/1 safe test-probe request per ratio row"},
        "output_layouts": {"density-expected.bin": "<q4d status,logp,scoremean,scorelogstd,entropy",
                           "uniform-expected.bin": "<q2d status,action,z", "seeded-expected.bin": "<q2d2Q status,action,z,nextseed,nextcounter",
                           "ratio-expected.bin": "<q6dq ideal-status,dlog,ideal-rho,S,loss,gradient,clippedrho,active; CONDITIONAL composition uses observed probe instead"},
        "exact_mask_bits": {"density": ["logp","score_mean","score_logstd","entropy"], "uniform_and_seeded": ["action","z"]},
        "domains": {"mean": [-16,16], "action": [-128,128], "log_std": [-4,2], "uniform": "[0,1)", "advantage": [-1e9,1e9], "epsilon": [0,1]},
        "constant_bits": {"C": f"{bits(C):016x}", "H": f"{bits(add(.5,C)):016x}", "TWO_PI": f"{bits(TWO_PI):016x}", "lower": f"{LOWER_BITS:016x}", "upper": f"{UPPER_BITS:016x}"},
        "budgets": {"density": {"absolute": 4e-12, "relative": 8e-15}, "sample": {"absolute": 8e-14, "relative": 8e-15}, "ratio_rho": {"absolute": 0., "relative": 4e-15}},
        "exact": ["entropy ALL words", "density-at-equal-action ALL words", "literal radial-zero mean copy/z positivezero", "nonzero radial with rounded w1 follows exact zero recipe", "seed/counter and exhausted status", "dlog/domain eligibility", "conditional status and Fraction-RN Rcp payload from independently bounded observed rho"],
        "oracle": {"density": "Decimal80 smooth closed-form exp(-2logstd), not the production inverse-sigma intermediate graph; exact normative equality/entropy overrides",
                   "sample": "Decimal80 log/sqrt/exp, independent high-precision pi reduction and cosine series at EXACT rounded theta, Fraction RN local operations",
                   "seeded": "published SplitMix64-top53 mapping composed exactly twice; existing RNG qualification carried, not repeated standalone",
                   "ratio": "Decimal80 exp independently bounds test-probe rho, followed by exact Fraction RN on observed rho; this is conditional composition, not correctly-rounded-libm proof"},
        "unavailable": "Failed actual output is unchanged; only after native poison/spare/canary assertions may transport serialize status plus all-positive-zero payload. No zero result was published.",
        "inputs": {name: identity(fixture/name) for name in files}, "carried": {name: identity(ROOT/name) for name in inherited},
        "endpoint_original_derivation": {name: identity(fixture/name) for name in ["endpoint-derivation.py","endpoint-original-output.json","endpoint-derivation.json","endpoint-archive-scope-v1.json"]},
        "coefficient_correction_before_first_generation": {name: identity(fixture/name) for name in ["coefficient-correction.json","coefficient-inline.py"]},
        "workload": {"models":0,"SDK":0,"physics":0,"training":0,"optimizer":0,"standalone_old_RNG_or_PPO_gates":0},
    }
    write_json(fixture / "experiment.json", experiment)
    write_json(fixture / "input-freeze.json", {"schema":"psionic.locomotion-gaussian.input-freeze.v1", "experiment":identity(fixture/"experiment.json"), "producer":identity(SOURCE), "expected_generation_executions":0})
    print(json.dumps({"experiment":identity(fixture/"experiment.json"),"producer":identity(SOURCE),"counts":experiment["counts"]}))


def decimal_exp(value):
    with localcontext() as context:
        context.prec = 80
        return float(Decimal.from_float(value).exp())


def decimal_cos(value):
    with localcontext() as context:
        context.prec = 100
        x = Decimal.from_float(value)
        # Angle range is [-0, rounded 2pi); reduction uses independent 120-digit pi.
        if x < 0:
            x = -x
        if x > PI:
            x = 2*PI-x
        sign = 1
        if x > PI/2:
            x, sign = PI-x, -1
        term = total = Decimal(1)
        k = 1
        while True:
            term = -term*x*x / ((2*k-1)*(2*k))
            total += term
            if abs(term) < Decimal("1e-100"):
                break
            k += 1
        return float(sign*total)


def density_reference(mean, logstd, action):
    h = add(.5, C)
    if action == mean:
        return (sub(sub(-0.0, logstd), C), 0., -1., add(logstd, h))
    with localcontext() as context:
        context.prec = 80
        # Use EXACT real action-mean, rather than the host rounded subtraction.
        exact_d = Fraction(action)-Fraction(mean)
        d = Decimal(exact_d.numerator)/Decimal(exact_d.denominator)
        s = Decimal.from_float(logstd)
        variance_inverse = (-2*s).exp()
        square = d*d*variance_inverse
        return (float(-square/2-s-Decimal.from_float(C)), float(d*variance_inverse), float(square-1), add(logstd,h))


def uniform_reference(mean, logstd, radial, angle):
    if radial == 0:
        return mean, 0.
    w, theta = sub(1., radial), mul(TWO_PI, angle)
    with localcontext() as context:
        context.prec = 80
        logarithm = float(Decimal.from_float(w).ln())
        radius = float(Decimal.from_float(mul(-2., logarithm)).sqrt())
    z = mul(radius, decimal_cos(theta))
    return add(mean, mul(decimal_exp(logstd), z)), z


def splitmix_top53(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter+1)) & MAX64
    z = ((z ^ (z >> 30))*0xBF58476D1CE4E5B9) & MAX64
    z = ((z ^ (z >> 27))*0x94D049BB133111EB) & MAX64
    return ((z ^ (z >> 31)) >> 11) / 2**53


def generate(fixture, output):
    need(not output.exists(), "one-shot reference directory already exists")
    freeze_record = json.loads((fixture/"input-freeze.json").read_text())
    experiment = json.loads((fixture/"experiment.json").read_text())
    need(freeze_record["producer"] == identity(SOURCE), "WHOLE producer frozen before generation")
    need(freeze_record["experiment"] == identity(fixture/"experiment.json"), "frozen experiment")
    for name, expected in experiment["inputs"].items():
        need(identity(fixture/name) == expected, "frozen input "+name)
    for name, expected in experiment["carried"].items():
        need(identity(ROOT/name) == expected, "carried source/proof "+name)
    output.mkdir(parents=True)
    started = time.monotonic()
    result = {"schema":"psionic.locomotion-gaussian.reference.v1", "passed":False, "producer":identity(SOURCE), "experiment":identity(fixture/"experiment.json"), "inputs":experiment["inputs"], "counts":experiment["counts"], "product_calls":0,"model_SDK_environment_optimizer_calls":0}
    write_json(output/"record.json", result)
    density_rows = records((fixture/"density-inputs.f64").read_bytes(), "<3d")
    uniform_rows = records((fixture/"uniform-inputs.f64").read_bytes(), "<4d")
    seed_rows = records((fixture/"seeded-inputs.bin").read_bytes(), "<2Q2d")
    ratio_rows = records((fixture/"ratio-inputs.f64").read_bytes(), "<4d")
    dlogs = records((fixture/"ratio-dlog.f64").read_bytes(), "<d")
    eligible = (fixture/"ratio-eligibility.u8").read_bytes()
    density, uniform, seeded, ratio = bytearray(), bytearray(), bytearray(), bytearray()
    density_exact, uniform_exact, seeded_exact = bytearray(), bytearray(), bytearray()
    for mean, logstd, action in density_rows:
        values = density_reference(mean,logstd,action)
        finite(values,4,"density expected")
        density += DENSITY.pack(0,*values)
        density_exact.append(15 if action == mean else 8)
    for mean, logstd, radial, angle in uniform_rows:
        values = uniform_reference(mean,logstd,radial,angle)
        finite(values,2,"uniform expected")
        uniform += UNIFORM.pack(0,*values)
        uniform_exact.append(3 if sub(1.,radial) == 1. else 0)
    for seed, counter, mean, logstd in seed_rows:
        if counter > MAX64-2:
            seeded += SEEDED.pack(4,0.,0.,0,0)
            seeded_exact.append(3)
            continue
        u, v = splitmix_top53(seed,counter), splitmix_top53(seed,counter+1)
        values = uniform_reference(mean,logstd,u,v)
        finite(values,2,"seeded expected")
        seeded += SEEDED.pack(0,*values,seed,counter+2)
        seeded_exact.append(3 if u == 0 else 0)
    for row, (dlog,), requested in zip(ratio_rows,dlogs,eligible):
        if requested:
            rho = decimal_exp(dlog)
            finite([rho],1,"ratio ideal expected")
            ratio += ratio_expected_from_probe(row,dlog,True,rho)
        else:
            ratio += RATIO.pack(3,*([0.]*6),0)
    outputs = {"density-expected.bin":density,"uniform-expected.bin":uniform,"seeded-expected.bin":seeded,"ratio-expected.bin":ratio,
               "density-exact.u8":density_exact,"uniform-exact.u8":uniform_exact,"seeded-exact.u8":seeded_exact}
    for name, data in outputs.items():
        (output/name).write_bytes(data)
    result.update(passed=True,elapsed_seconds=time.monotonic()-started,outputs={name:identity(output/name) for name in outputs},
                  exact_mask_counts={"density_all4":sum(x==15 for x in density_exact),"uniform_all2":sum(x==3 for x in uniform_exact),"seeded_all2_or_unavailable":sum(x==3 for x in seeded_exact)},
                  ratio_eligible=sum(eligible), ratio_conditional_composition="Use observed separately Decimal-bounded probe rho, not this ideal rho, for exact public status/Rcp payload; successful public rho must equal probe bits.")
    write_json(output/"record.json",result)
    print(json.dumps({"passed":True,"record":identity(output/"record.json"),"counts":result["counts"]}))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--freeze", action="store_true")
    parser.add_argument("--fixture", type=Path, default=FIXTURE)
    parser.add_argument("--contract", type=Path, default=Path("/tmp/psionic171-gaussian-contract.md"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.freeze:
        freeze(args.fixture,args.contract)
    else:
        need(args.output is not None,"--output required")
        generate(args.fixture,args.output)


if __name__ == "__main__":
    main()
