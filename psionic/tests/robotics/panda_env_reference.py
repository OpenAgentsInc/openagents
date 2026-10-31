#!/usr/bin/env python3
"""Firm reason2: independent original-formula NumPy oracle, not runtime code.

No LIBERO, NVIDIA or robosuite modules are imported. Observation conversion
uses their pinned source equation through NumPy/math.acos; the product's
inverse-cosine rational approximation is not reused here.
"""
import argparse
import ctypes as ct
import hashlib
import json
import math
from pathlib import Path
import numpy as np


class D(ct.Structure):
    _fields_ = [("data", ct.POINTER(ct.c_double)), ("count", ct.c_int64)]


class F(ct.Structure):
    _fields_ = [("data", ct.POINTER(ct.c_float)), ("count", ct.c_int64)]


class Read(ct.Structure):
    _fields_ = [("status", ct.c_uint8), ("written", ct.c_int64),
                ("derived_lag", ct.c_double), ("quaternion_clamped", ct.c_bool)]


class Success(ct.Structure):
    _fields_ = [("status", ct.c_uint8), ("success", ct.c_bool)]


def ds(a):
    assert a.dtype == np.float64 and a.flags.c_contiguous
    return D(a.ctypes.data_as(ct.POINTER(ct.c_double)), a.size)


def fs(a):
    return F(a.ctypes.data_as(ct.POINTER(ct.c_float)), a.size)


def original_axis(q):
    w = np.clip(q[0], -1., 1.)
    den = np.sqrt(1. - w * w)
    return np.zeros(3) if math.isclose(den, 0.) else (q[1:] * 2. * math.acos(w)) / den


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("library", type=Path)
    parser.add_argument("--record", type=Path)
    parser.add_argument("--genuine", type=Path)
    args = parser.parse_args()
    lib = ct.CDLL(str(args.library.resolve()))
    lib.pe_observation.argtypes = [D, D, D, F, ct.c_bool, ct.c_bool]
    lib.pe_observation.restype = Read
    lib.pe_success.argtypes = [D, D, D, D]
    lib.pe_success.restype = Success
    rng = np.random.Generator(np.random.PCG64(169))
    observations = words = predicates = 0

    def observation(p, q, fingers):
        nonlocal observations, words
        expected = np.concatenate((p, original_axis(q), fingers)).astype(np.float32)
        out = np.full(10, -733.25, dtype=np.float32)
        r = lib.pe_observation(ds(p), ds(q), ds(fingers), fs(out), observations % 2, False)
        assert r.status == 0 and r.written == 8
        assert r.derived_lag == (.002 if observations % 2 else 0.)
        assert r.quaternion_clamped == (abs(q[0]) > 1)
        assert np.array_equal(out[:8].view(np.uint32), expected.view(np.uint32)), (q, out, expected)
        assert np.all(out[8:] == np.float32(-733.25))
        observations += 1
        words += 8

    for _ in range(4096):
        q = rng.normal(size=4)
        q /= np.linalg.norm(q)
        observation(rng.uniform(-2, 2, 3), q, rng.uniform(-.04, .04, 2))
    for w in np.linspace(-1, 1, 2001):
        q = np.array([w, np.sqrt(max(0, 1-w*w)), -0., 0.])
        observation(np.zeros(3), q, np.zeros(2))
    for w in [-1.00000001, 1.00000001]:
        observation(np.zeros(3), np.array([w, 0., 0., 0.]), np.zeros(2))
    if args.genuine:
        with np.load(args.genuine) as source:
            observation(source["site_position"], source["body_quat_wxyz"], source["finger_q"])

    def predicate(p, matrix, size, obj):
        nonlocal predicates
        extent = np.abs(matrix @ size)
        lower, upper = p-extent, p+extent
        lower[2] -= .01
        expected = bool(np.all(obj > lower) and np.all(obj < upper))
        r = lib.pe_success(ds(p), ds(matrix.ravel()), ds(size), ds(obj))
        assert r.status == 0 and r.success == expected, (p, matrix, size, obj, r.success, expected)
        predicates += 1

    for _ in range(5000):
        matrix, _ = np.linalg.qr(rng.normal(size=(3, 3)))
        p = rng.uniform(-1, 1, 3)
        size = rng.uniform(.01, .5, 3)
        obj = p+rng.uniform(-2, 2, 3)*size
        predicate(p, matrix, size, obj)
    # Exact source target orientation, with all strict faces and one-ULP edges.
    matrix = np.diag([1., -1., -1.])
    for _ in range(100):
        p = rng.uniform(-1, 1, 3)
        size = rng.uniform(.01, .5, 3)
        for axis in range(3):
            for side in [-1, 1]:
                bound = p[axis]+side*size[axis]-(.01 if axis == 2 and side < 0 else 0)
                for value in [np.nextafter(bound, -np.inf), bound, np.nextafter(bound, np.inf)]:
                    obj = p.copy()
                    obj[axis] = value
                    predicate(p, matrix, size, obj)

    record = {"schema": "psionic-panda-env-numpy-v1", "numpy": np.__version__,
              "observations": observations, "exact_f32_words": words,
              "strict_predicate_cases": predicates, "passed": True,
              "source_checkout_executed": False,
              "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "library_sha256": hashlib.sha256(args.library.read_bytes()).hexdigest()}
    if args.genuine:
        record["genuine_frame_sha256"] = hashlib.sha256(args.genuine.read_bytes()).hexdigest()
    if args.record:
        args.record.write_text(json.dumps(record, indent=2)+"\n")
    print(json.dumps(record, sort_keys=True))


if __name__ == "__main__":
    main()
