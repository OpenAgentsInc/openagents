#!/usr/bin/env python3
"""Use the captured independent CPU reference for the original Mammoth sample request.

Mammoth and Elephant pin the same tokenizer.json, so the typed sample renders to
the same checked token stream as chapter 10; this fixture reuses that file.
"""
import hashlib, json, math, re, sys
from pathlib import Path

models = Path(__file__).resolve().parents[2] / 'psionic/fixtures/models'
raw = (models / 'elephant/request-tokens.txt').read_bytes()
assert hashlib.sha256(raw).hexdigest() == '3c72fddd8566bdfa0cee7347c29f9e972725005fd3e2c6d5b1a03f5559755226'
numbers = [int(x) for x in raw.split()]
prefix, tokens = numbers[0], numbers[1:]
reference = json.loads((models / 'mammoth/transformers-reference.json').read_text())
assert len(tokens) == reference['tokens'] == 86 and prefix == reference['prefix'] == 49 and reference['layers'] == 64
assert all(0 <= x < 248320 for x in tokens)
decision = reference['decision']
assert len(decision['probabilities']) == 2 and abs(sum(decision['probabilities']) - 1) < 1e-6 and 0 < decision['slot_mass'] <= 1
# Fixed before the Mammoth run, as for Elephant: absolute difference per statistic.
TOLERANCE = 0.0005
if sys.argv[1:] == ['--header']:
    print('static const int64_t mammoth_request_prefix = %d;' % prefix)
    print('static const int64_t mammoth_request_tokens[] = {' + ','.join(map(str, tokens)) + '};')
elif len(sys.argv) == 3 and sys.argv[1] == '--check-output':
    output = Path(sys.argv[2]).read_text()
    match = re.search(r'choice=(\w+) p_yes=([0-9.eE+-]+) p_no=([0-9.eE+-]+) slot_mass=([0-9.eE+-]+) confidence=([0-9.eE+-]+)', output)
    expected_choice = 'yes' if decision['probabilities'][0] >= decision['probabilities'][1] else 'no'
    if not match or match[1] != expected_choice: raise ValueError('unexpected decision output')
    values = [float(match[i]) for i in range(2, 6)]
    p = decision['probabilities']
    expected = p + [decision['slot_mass'], abs(p[0] - p[1])]
    if any(not math.isfinite(v) or abs(v - e) > TOLERANCE for v, e in zip(values, expected)):
        raise ValueError('original Mammoth decision differs from independent CPU reference')
    print(f'original Mammoth decision agrees with independent CPU reference within {TOLERANCE}')
else:
    if sys.argv[1:]: raise SystemExit('usage: mammoth_reference_fixture.py [--header | --check-output LOG]')
    print('Mammoth sample request and independent reference fixture pass')
