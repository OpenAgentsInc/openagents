#!/usr/bin/env python3
"""Bit-vector proof that the four-byte u32 accumulator equals the old load (#092b).

The models below are written by hand from the source lines this script pins.
Source-to-model correspondence is a review premise, not something the solver
checks. Each proof is an unsat query. Each control is a deliberately wrong
model (wrong width, a missing initialization, a swapped order) that must be
sat, which shows that the queries can fail.

  PYTHONPATH=<z3 site-packages> python3 tools/coax-nostr/load32/prove.py --report OUT.json
"""
import argparse, hashlib, json
from pathlib import Path
import z3

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'coax/lib/crypto/bytes.coax'
# Exact source lines the models stand for; any change refuses the proof.
NEEDLES = [
    'cb_load_word :: (data: [] u8, at: int, width: int, big: bool) -> CbU64 {',
    '    index := ifx big then at + i else at + width - 1 - i;',
    '    value = (value << 8) | cast(u64) data[index];',
    'cb_load_word32 :: (data: [] u8, at: int, big: bool) -> CbU32 {\n'
    '  result: CbU32;\n'
    '  if !cb_span(data.count, at, 4) { return result; }\n'
    '  value: u32;\n'
    '  for i: 0..<4 {\n'
    '    index := ifx big then at + i else at + 3 - i;\n'
    '    value = (value << 8) | cast,trunc(u32) data[index];\n'
    '  }\n'
    '  result.value = value; result.ok = true;\n'
    '  return result;\n'
    '}',
    'cb_load_be32 :: (data: [] u8, at: int) -> CbU32 #export_c { return cb_load_word32(data, at, true); }',
    'cb_load_le32 :: (data: [] u8, at: int) -> CbU32 #export_c { return cb_load_word32(data, at, false); }',
]

def order(n, big):
    return [i if big else n - 1 - i for i in range(n)]

def old_load(b, big):
    """The 64-bit accumulator of cb_load_word with width 4, then cast(u32)."""
    v = z3.BitVecVal(0, 64)
    for k in order(4, big):v = (v << 8) | z3.ZeroExt(56, b[k])
    return v

def new_load(b, big, n=4, start=None):
    """cb_load_word32; n and start exist only for the wrong-model controls."""
    v = z3.BitVecVal(0, 32) if start is None else start
    steps = []
    for k in order(n, big):
        steps.append(v)
        v = (v << 8) | z3.ZeroExt(24, b[k])
    return v, steps

def solve(name, formula, expected):
    s = z3.Solver();s.add(formula);result = str(s.check())
    if result != expected:raise AssertionError((name, result, expected))
    return {'name': name, 'expected': expected, 'result': result, 'smt_sha256': hashlib.sha256(s.sexpr().encode()).hexdigest()}

def main():
    ap = argparse.ArgumentParser();ap.add_argument('--report', type=Path, required=True);args = ap.parse_args()
    text = SOURCE.read_text()
    for needle in NEEDLES:
        if text.count(needle) != 1:raise ValueError('source shape changed: ' + needle)
    b = [z3.BitVec(f'b{i}', 8) for i in range(5)];x = z3.BitVec('start', 32)
    checks = []
    for big in [True, False]:
        tag = 'be' if big else 'le'
        old = old_load(b, big);new, steps = new_load(b, big)
        # The old checked narrowing never trapped: the 4-byte u64 value fits u32.
        checks.append(solve(f'{tag}_old_fits_u32', z3.UGE(old, z3.BitVecVal(1 << 32, 64)), 'unsat'))
        checks.append(solve(f'{tag}_new_equals_old', new != z3.Extract(31, 0, old), 'unsat'))
        # No lost bits: before each shift the accumulator holds at most 24 bits,
        # and the shifted value never overlaps the incoming byte.
        for i, v in enumerate(steps):
            checks.append(solve(f'{tag}_step{i}_below_2_24', z3.UGE(v, z3.BitVecVal(1 << 24, 32)), 'unsat'))
            k = order(4, big)[i]
            checks.append(solve(f'{tag}_step{i}_disjoint_or', ((v << 8) & z3.ZeroExt(24, b[k])) != 0, 'unsat'))
        # Each byte lands in exactly its own field.
        for k in range(4):
            shift = (3 - k) * 8 if big else k * 8
            checks.append(solve(f'{tag}_byte{k}_field', z3.Extract(7, 0, z3.LShR(new, shift)) != b[k], 'unsat'))
        # Controls: wrong models the same queries must be able to tell apart.
        three, _ = new_load(b, big, n=3)
        checks.append(solve(f'{tag}_control_width3_differs', three != z3.Extract(31, 0, old), 'sat'))
        five, _ = new_load(b, big, n=5);v64 = z3.BitVecVal(0, 64)
        for k in order(5, big):v64 = (v64 << 8) | z3.ZeroExt(56, b[k])
        checks.append(solve(f'{tag}_control_width5_loses_bits', z3.ZeroExt(32, five) != v64, 'sat'))
        # Four 8-bit shifts move any starting accumulator value out of a u32,
        # so the success value does not depend on the initialization...
        started, _ = new_load(b, big, start=x)
        checks.append(solve(f'{tag}_start_value_shifted_out', started != new, 'unsat'))
        # ...so a missing initialization is only observable in a shorter
        # accumulator. The zero-valued refusal path is not modeled here; the
        # Coax refusal check in bytes.coax and mutation cover it.
        three_started, _ = new_load(b, big, n=3, start=x);three_zero, _ = new_load(b, big, n=3)
        checks.append(solve(f'{tag}_control_uninitialized_width3_differs', three_started != three_zero, 'sat'))
        swapped, _ = new_load(b, not big)
        checks.append(solve(f'{tag}_control_swapped_order_differs', swapped != z3.Extract(31, 0, old), 'sat'))
    report = {'schema': 'coax.load32-proof.v1', 'source': 'coax/lib/crypto/bytes.coax',
              'source_sha256': hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
              'driver_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'z3': z3.get_version_string(), 'needles': NEEDLES, 'checks': checks,
              'premise': 'the hand-written models correspond to the pinned source lines (the removed old cast(u32) wrappers were reviewed against the previous source); independently reviewed, not solver-checked; the refusal path is covered by Coax checks and mutation, not by z3'}
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(f"{sum(c['expected'] == 'unsat' for c in checks)} proofs unsat, {sum(c['expected'] == 'sat' for c in checks)} controls sat")

if __name__ == '__main__':main()
