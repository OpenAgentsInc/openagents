#!/usr/bin/env python3
"""Write the example's expected transcript from the SDK endpoint, before running Coax."""
from pathlib import Path
import argparse
import hashlib
import json
import math
import struct

ROOT = Path(__file__).resolve().parents[3]
EXPERIMENT_SHA = '23a533517be639cf4b428683c106b9ce64d26ee75ff88564c50849346beb8d6f'
ROW = struct.Struct('<54dq')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def micro(value):
    rounded = math.floor(abs(value) * 1_000_000 + .5)
    return -rounded if value < 0 else rounded

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    record_path = args.reference / 'record.json'
    record = json.loads(record_path.read_text())
    trace_path = args.reference / 'sdk-trace.bin'
    raw = trace_path.read_bytes()
    require(record['schema'] == 'psionic.panda-joint-pd-prefix.sdk-reference.v1'
            and record['passed'] and record['experiment_sha256'] == EXPERIMENT_SHA,
            'completed frozen SDK prefix')
    require(record['trace'] == dict(bytes=len(raw), sha256=sha(trace_path)), 'exact captured trace')
    require(len(raw) == 8 + 250 * ROW.size and struct.unpack_from('<II', raw) == (0x31525050, 250),
            'exact PPR1 trace')
    rows = [ROW.unpack_from(raw, 8 + i * ROW.size) for i in range(250)]
    require(all(math.isfinite(x) for row in rows for x in row[:54]), 'all captured doubles finite')
    require(all(0 <= row[54] <= 7 for row in rows), 'all PJP clip counts valid')
    endpoint = rows[-1]
    lines = ['own no-contact joint-PD steps: 250']
    lines += [f'hinge {i + 1} q/v (micro-rad, micro-rad/s): {micro(endpoint[i])} {micro(endpoint[9 + i])}' for i in range(7)]
    lines += [f'slide {i + 1} q/v (micro-m, micro-m/s): {micro(endpoint[7 + i])} {micro(endpoint[16 + i])}' for i in range(2)]
    lines += [f'PD clips: {sum(row[54] for row in rows)}']
    require(not args.output.exists(), 'preserve prior reference preparation')
    args.output.mkdir(parents=True)
    stdout = args.output / 'example.stdout'
    stdout.write_text('\n'.join(lines) + '\n')
    proof = dict(schema='psionic.panda-joint-pd-rollout.example-reference.v1', passed=True,
                 source='one SDK prefix endpoint and sum of recorded PJP clipping counts',
                 experiment_sha256=EXPERIMENT_SHA, capture_record_sha256=sha(record_path),
                 trace_sha256=sha(trace_path), generator_sha256=sha(__file__),
                 Coax_example_sha256=sha(ROOT / 'psionic/examples/panda-joint-pd-rollout.coax'),
                 expected_stdout_sha256=sha(stdout), Coax_calls=0, SDK_calls=0,
                 rounding='nearest micro-unit, halfway away from zero',
                 minimum_rounding_margin_micro=min(abs(abs(x) * 1_000_000 % 1 - .5) for x in endpoint[:18]),
                 written_before_first_example_execution=True)
    (args.output / 'record.json').write_text(json.dumps(proof, indent=2) + '\n')
    print(json.dumps(proof))

if __name__ == '__main__':
    main()
