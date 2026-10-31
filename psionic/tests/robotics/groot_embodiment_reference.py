#!/usr/bin/env python3
"""Firm reason2: independent development NumPy oracle, never a runtime dependency.

Read original selected statistics; do not import or execute NVIDIA code. Explicit
array dtypes reproduce the source NumPy1.26 rounding under newer NumPy as well.
Synthetic vectors qualify conversion arithmetic, not learned VLA capability.
"""
import ctypes as ct
import hashlib
import json
from pathlib import Path
import sys
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
STATISTICS = ROOT / "fixtures/robotics/groot-libero/libero-statistics.json"
assert hashlib.sha256(STATISTICS.read_bytes()).hexdigest() == "e97cd73beef2b81eca32eae792f88538c6b3681187709f069dc943f4bb41419b"
stats = json.loads(STATISTICS.read_text())
groups = ["x", "y", "z", "roll", "pitch", "yaw", "gripper"]

def bounds(kind, quantile):
    return np.concatenate([np.asarray(stats[kind][key][quantile], dtype=np.float64) for key in groups])

sl, sh = bounds("state", "q01"), bounds("state", "q99")
al, ah = bounds("action", "q01"), bounds("action", "q99")
assert sl.shape == sh.shape == (8,) and al.shape == ah.shape == (7,)

class Span(ct.Structure):
    _fields_ = [("data", ct.POINTER(ct.c_float)), ("count", ct.c_int64)]

class Read(ct.Structure):
    _fields_ = [("status", ct.c_uint8), ("written", ct.c_int64), ("clipped", ct.c_int64)]

lib = ct.CDLL(str(Path(sys.argv[1]).resolve()))
for name in ["ge_normalize", "ge_decode"]:
    fn = getattr(lib, name)
    fn.argtypes = [Span, Span, ct.c_bool]
    fn.restype = Read

def span(x):
    assert x.dtype == np.float32 and x.flags.c_contiguous
    return Span(x.ctypes.data_as(ct.POINTER(ct.c_float)), x.size)

words = 0
def exact(actual, expected):
    global words
    a, e = actual.view(np.uint32), expected.view(np.uint32)
    mismatch = np.flatnonzero(a.ravel() != e.ravel())
    if mismatch.size:
        i = int(mismatch[0])
        raise AssertionError(f"F32 word{i}: {a.ravel()[i]:08x} != {e.ravel()[i]:08x}")
    words += a.size

def check_state(x):
    with np.errstate(over="ignore"):
        intermediate = np.zeros_like(x)
        intermediate[...] = (x.astype(np.float64)-sl)/(sh-sl)
        normalized = np.float32(2)*intermediate-np.float32(1)
        normalized = np.clip(normalized, np.float32(-1), np.float32(1))
    expected = np.zeros(132, dtype=np.float32)
    expected[:8] = normalized
    out = np.full(137, -713.25, dtype=np.float32)
    r = lib.ge_normalize(span(x), span(out), False)
    assert (r.status, r.written, r.clipped) == (0, 132, int(np.count_nonzero((x < sl) | (x > sh))))
    exact(out[:132], expected)
    exact(out[132:], np.full(5, -713.25, dtype=np.float32))

def check_action(model):
    selected = model.reshape(40,132)[:16,:7]
    clipped = np.clip(selected, np.float32(-1), np.float32(1))
    unit = (clipped+np.float32(1))/np.float32(2)
    expected = (unit.astype(np.float64)*(ah-al)+al).astype(np.float32)
    expected[:,6] = -np.sign(np.float32(2)*expected[:,6]-np.float32(1))
    out = np.full(117, -713.25, dtype=np.float32)
    r = lib.ge_decode(span(model), span(out), False)
    assert (r.status, r.written, r.clipped) == (0,112,int(np.count_nonzero((selected < -1) | (selected > 1))))
    exact(out[:112], expected.ravel())
    exact(out[112:], np.full(5, -713.25, dtype=np.float32))

vectors = [np.zeros(8,dtype=np.float32), sl.astype(np.float32), sh.astype(np.float32)]
for boundary in [sl,sh]:
    for axis in range(8):
        for direction in [np.float32(-np.inf),np.float32(np.inf)]:
            x = boundary.astype(np.float32)
            x[axis] = np.nextafter(x[axis],direction)
            vectors.append(x)
for word in [0,0x80000000,1,0x80000001,0x7f7fffff,0xff7fffff]:
    vectors.append(np.full(8,word,dtype=np.uint32).view(np.float32))
for i in range(257):
    unit = ((np.arange(8)*17+i*37)%281-12)/256
    vectors.append((sl+(sh-sl)*unit).astype(np.float32))
for x in vectors:
    check_state(x)
for i in range(257):
    model = np.full((40,132),np.nan,dtype=np.float32)
    model[:16,:7] = (((np.arange(16)[:,None]*17+np.arange(7)[None,:]*5+i*37)%521-260)/128).astype(np.float32)
    check_action(model.ravel())
for word in [0,0x80000000,1,0x80000001,0x3f800000,0xbf800000,0x7f7fffff,0xff7fffff]:
    model = np.full((40,132),np.nan,dtype=np.float32)
    model[:16,:7] = np.asarray(word,dtype=np.uint32).view(np.float32)
    check_action(model.ravel())
print(f"GR00T LIBERO NumPy {np.__version__}: {len(vectors)} states +265 actions, {words} exact F32 words passed")
