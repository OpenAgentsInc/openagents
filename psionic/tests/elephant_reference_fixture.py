#!/usr/bin/env python3
"""Use the captured independent CPU reference for the original sample request."""
import hashlib, json, math, re, sys
from pathlib import Path

root = Path(__file__).resolve().parents[2] / 'psionic/fixtures/models/elephant'
raw = (root / 'request-tokens.txt').read_bytes()
assert hashlib.sha256(raw).hexdigest() == '3c72fddd8566bdfa0cee7347c29f9e972725005fd3e2c6d5b1a03f5559755226'
numbers = [int(x) for x in raw.split()]
prefix, tokens = numbers[0], numbers[1:]
reference = json.loads((root / 'transformers-reference.json').read_text())
assert len(tokens) == reference['tokens'] == 86 and prefix == reference['prefix'] == 49
assert all(0 <= x < 248320 for x in tokens)
if sys.argv[1:] == ['--header']:
    print('static const int64_t elephant_request_prefix = %d;' % prefix)
    print('static const int64_t elephant_request_tokens[] = {' + ','.join(map(str, tokens)) + '};')
elif len(sys.argv) == 3 and sys.argv[1] == '--check-output':
    output = Path(sys.argv[2]).read_text()
    match = re.search(r'choice=(\w+) p_yes=([0-9.eE+-]+) p_no=([0-9.eE+-]+) slot_mass=([0-9.eE+-]+) confidence=([0-9.eE+-]+)', output)
    if not match or match[1] != 'yes': raise ValueError('unexpected decision output')
    values = [float(match[i]) for i in range(2, 6)]
    expected = reference['decision']['probabilities'] + [reference['decision']['slot_mass'], 2*reference['decision']['probabilities'][0]-1]
    if any(not math.isfinite(v) or abs(v-e) > 0.0005 for v,e in zip(values, expected)):
        raise ValueError('original decision differs from independent CPU reference')
    print('original decision agrees with independent CPU reference within 0.0005')
elif len(sys.argv) == 3 and sys.argv[1] == '--check-cpu':
    # A device result against the recorded CPU reference profile result.
    pattern = r'choice=(\w+) p_yes=([0-9.eE+-]+) p_no=([0-9.eE+-]+) slot_mass=([0-9.eE+-]+) confidence=([0-9.eE+-]+)'
    cpu = re.search(pattern, (root / 'psionic-reference.txt').read_text())
    got = re.search(pattern, Path(sys.argv[2]).read_text())
    if not cpu or not got or got[1] != cpu[1]: raise ValueError('unexpected decision output')
    if any(not math.isfinite(float(got[i])) or abs(float(got[i]) - float(cpu[i])) > 0.0005 for i in range(2, 6)):
        raise ValueError('decision differs from the recorded CPU reference result')
    print('decision agrees with the recorded CPU reference result within 0.0005')
else:
    if sys.argv[1:]: raise SystemExit('usage: elephant_reference_fixture.py [--header | --check-output LOG | --check-cpu LOG]')
    print('Elephant sample request and independent reference fixture pass')
