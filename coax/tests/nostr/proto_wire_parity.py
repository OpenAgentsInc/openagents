#!/usr/bin/env python3
"""Ensure both descriptor-prefix suites retain the same wire evidence."""
from pathlib import Path
root=Path(__file__).resolve().parent
f=(root/'proto_wire_F.coax').read_text().replace('F__pb_', 'P__pb_')
n=(root/'proto_wire_N.coax').read_text().replace('N__pb_', 'P__pb_')
if f!=n: raise SystemExit('descriptor wire check suites have drifted')
print('descriptor-prefix wire check suites match')
