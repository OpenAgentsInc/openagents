#!/usr/bin/env python3
"""Firm reason2: retrospectively check saved clocks with native Coax ps_time.

No inference, renderer, controller or simulator is rerun. A trace satisfying
this check is not relabeled as having executed the newly added guard live.
"""
import argparse
import ctypes as ct
import hashlib
import json
import math
from pathlib import Path
import re
import struct


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("library", type=Path)
    parser.add_argument("pairs", type=Path)
    parser.add_argument("cases", nargs="+", type=Path)
    parser.add_argument("--compiled-c", type=Path, required=True)
    parser.add_argument("--compiled-header", type=Path, required=True)
    parser.add_argument("--record", type=Path, required=True)
    args = parser.parse_args()
    lib = ct.CDLL(str(args.library.resolve()))
    lib.ps_time.argtypes = [ct.c_uint64, ct.c_uint64, ct.c_int64, ct.c_int64]
    lib.ps_time.restype = ct.c_bool
    tolerance = 2.**-26
    cases = []
    for case in args.cases:
        match = re.fullmatch(r"(demo_\d{3})-fault\d", case.name)
        assert match, case.name
        init_path = args.pairs/match[1]/"init.f64"
        initial_bytes = init_path.read_bytes()
        assert len(initial_bytes) == 45*8
        initial, = struct.unpack_from("<d", initial_bytes)
        initial_bits, = struct.unpack_from("<Q", initial_bytes)
        assert math.isfinite(initial) and 0 <= initial <= 100
        record = json.loads((case/"record.json").read_text())
        trace_path = case/"physics.ptr"
        trace = trace_path.read_bytes()
        assert (len(trace)-64) % 512 == 0
        count = (len(trace)-64)//512
        assert count == record["physics_steps"] == record["trace_records"]
        previous = trace[:32]
        assert previous.hex() == record["trace_seed_sha256"]
        worst = 0.
        for i in range(count):
            data = trace[32+i*512:32+(i+1)*512]
            assert data[:4] == b"PTR1" and data[4:6] == b"\x01\x00"
            assert data[40:48] == bytes(8) and data[508:] == b"END1"
            tick, receipt = struct.unpack_from("<QQ", data, 24)
            substep = data[7]
            assert tick == i//25 and substep == i%25+1
            assert receipt == 25*tick+substep
            current_bits, = struct.unpack_from("<Q", data, 148)
            current, = struct.unpack_from("<d", data, 148)
            assert lib.ps_time(initial_bits, current_bits, tick, substep), (case.name, i)
            expected = initial+(25*tick+substep)*.002
            error = abs(current-expected)
            assert math.isfinite(current) and error <= tolerance
            worst = max(worst, error)
            previous = hashlib.sha256(previous+data).digest()
        assert previous == trace[-32:] and previous.hex() == record["trace_final_sha256"]
        cases.append({"case": match[1], "clock_values_checked": count,
                      "ticks": record["ticks"], "initial_seconds": initial,
                      "initial_state_sha256": sha(init_path),
                      "trace_sha256": sha(trace_path), "trace_bytes": len(trace),
                      "final_chain_sha256": previous.hex(),
                      "maximum_clock_error_seconds": worst,
                      "native_ps_time_all_passed": True,
                      "stdlib_clock_and_sha_all_passed": True})
    root = Path(__file__).resolve().parents[3]
    result = {"schema": "psionic-panda-retrospective-clock-v1", "passed": True,
              "source_session_sha256": sha(root/"psionic/tests/robotics/panda-session.coax"),
              "compiled_aggregate_c_sha256": sha(args.compiled_c),
              "compiled_header_sha256": sha(args.compiled_header),
              "native_library_sha256": sha(args.library),
              "oracle_sha256": sha(Path(__file__)),
              "initial_seconds_range": [0, 100], "physics_dt_seconds": .002,
              "nominal_elapsed_limit_seconds": 36, "tolerance_seconds": tolerance,
              "clock_values_checked": sum(c["clock_values_checked"] for c in cases),
              "cases": cases,
              "scope": "Retrospective native guard and independent stdlib validation of existing traces. These episodes predate the live guard; no neural/simulator replay occurred. Plain F64/integer SDK clock, not compiler typed clocks or wall deadlines."}
    args.record.write_text(json.dumps(result, indent=2)+"\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
