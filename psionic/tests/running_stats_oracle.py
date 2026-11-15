#!/usr/bin/env python3
# Firm reason: 2 (independent exact two-pass statistics and Decimal normalization oracle).
"""Freeze observation statistics independently from the incremental product."""
from decimal import Decimal, localcontext
from fractions import Fraction as F
from pathlib import Path
import json
import sys

ROOT = Path(__file__).resolve().parents[2]

def artifacts():
    rows = [[1, 2, 3], [2, 4, 4], [3, 6, 5]]
    sample = [4, 0, -1]
    means, m2, normalized = [], [], []
    with localcontext() as ctx:
        ctx.prec = 70
        for channel in range(3):
            column = [F(row[channel]) for row in rows]
            mean = sum(column) / len(column)
            squared = sum((x - mean)**2 for x in column)
            means.append(str(mean))
            m2.append(str(squared))
            variance = squared / len(column)
            decimal = Decimal(variance.numerator) / Decimal(variance.denominator)
            delta = F(sample[channel]) - mean
            value = (Decimal(delta.numerator) / Decimal(delta.denominator)) / (decimal + Decimal('1e-8')).sqrt()
            normalized.append(str(value))
    doc = dict(profile='running-statistics-f64-v1', rows=rows, sample=sample, mean=means, m2=m2,
               variance='population, M2 / count', epsilon='1e-8', normalized=normalized,
               tolerance=dict(atol=2e-13, rtol=2e-13))
    vector = '// Independent two-pass Fraction/Decimal reference, frozen before implementation.\n'
    for name, values in [('RN_GOLDEN_MEAN', means), ('RN_GOLDEN_M2', m2), ('RN_GOLDEN_NORMALIZED', normalized)]:
        vector += name + ' :: [3]f64.[' + ','.join(repr(float(F(x))) for x in values) + '];\n'
    return [(ROOT/'psionic/fixtures/running-stats/reference.json', json.dumps(doc, indent=2)+'\n'),
            (ROOT/'psionic/lib/running-stats-golden.coax', vector)]

if __name__ == '__main__':
    assert sys.argv[1:] in ([], ['--write'])
    for path, content in artifacts():
        if sys.argv[1:] == ['--write'] and not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        assert path.read_text() == content, f'frozen running-statistics reference changed: {path}'
    print('running statistics oracle: two-pass Fraction moments and 70-digit Decimal normalization match')
