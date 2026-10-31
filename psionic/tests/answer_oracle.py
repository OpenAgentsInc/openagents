#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent answer-serialization and decision-quality references for chapter 12.

The answer formulas are transcribed from OneJev qev/answers.py at
81ce62f1597c91e46767d4d02d6ac2e18534fe94 (read, never imported or run). Rounding
and number printing use Python's own round() and json.dumps(), the functions the
published server applies, so they are independent of the Coax implementation.
Sums run left to right, as Python's sum() did before 3.12.
"""
import json
import math
import random
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'psionic/fixtures/quality/answer-vectors.coax'
QUALITY = ROOT / 'psionic/fixtures/quality/quality-vectors.coax'


def total(values):
    s = 0.0
    for v in values:
        s += v
    return s


def normalize(p):
    t = total(p)
    return [1.0 / len(p)] * len(p) if t <= 0 else [v / t for v in p]


def first_max(p):
    return max(range(len(p)), key=p.__getitem__)


def choice_confidence(p):
    if len(p) == 1:
        return 1.0
    p = normalize(p)
    u = 1.0 / len(p)
    return (max(p) - u) / (1.0 - u)


def score_confidence(p):
    if len(p) == 1:
        return 1.0
    p = normalize(p)
    k = len(p)
    mode = first_max(p)
    distance = total([pi * abs(i - mode) for i, pi in enumerate(p)])
    center = (k - 1) / 2
    mad = total([abs(i - center) for i in range(k)]) / k
    return max(0.0, 1.0 - distance / mad)


def expected(p):
    p = normalize(p)
    return total([i * pi for i, pi in enumerate(p)])


def answer(kind, fields, probs, names):
    r = lambda x: float(round(x, 6))
    if kind == 1:
        obj = {'type': 'noul', 'noul': r(min(1.0, max(0.0, probs[0])))}
    elif kind == 0:
        p = normalize(probs)
        obj = {'type': 'choice', 'choice': names[first_max(p)], 'probabilities': {o: r(v) for o, v in zip(names, p)}}
        if fields == 0:
            obj['confidence'] = r(choice_confidence(p))
    else:
        p = normalize(probs)
        obj = {'type': 'score', 'score': r(expected(p))}
        if fields == 0:
            obj['legend'] = {str(i): (None if n == '' else n) for i, n in enumerate(names)}
        obj['probabilities'] = {str(i): r(v) for i, v in enumerate(p)}
        if fields == 0:
            obj['confidence'] = r(score_confidence(p))
    return json.dumps(obj, ensure_ascii=False, separators=(',', ':'))


def ece(conf, correct, bins=15):
    n, t = len(conf), 0.0
    for b in range(bins):
        lo, hi = b / bins, (b + 1) / bins
        idx = [i for i, c in enumerate(conf) if (lo < c <= hi) or (b == 0 and c <= lo)]
        if idx:
            t += len(idx) / n * abs(total([conf[i] for i in idx]) / len(idx) - total([correct[i] for i in idx]) / len(idx))
    return t


def quality(items, probs):
    kinds = [[0, 0, 0.0] for _ in range(3)]
    out = dict(items=len(items), correct=0, brier=0.0, error=0.0, offered=0, abstained=0, answered=0, answered_correct=0)
    conf, hit = [], []
    for kind, count, start, gold, abstain in items:
        p = probs[start:start + count]
        best = first_max(p)
        ok = best == gold
        b = total([(v - (1.0 if i == gold else 0.0)) ** 2 for i, v in enumerate(p)])
        kinds[kind][0] += 1; kinds[kind][1] += ok; kinds[kind][2] += b
        out['brier'] += b; out['correct'] += ok
        if kind == 2:
            out['error'] += abs(total([i * v for i, v in enumerate(p)]) - gold)
        out['offered'] += abstain >= 0
        if abstain >= 0 and best == abstain:
            out['abstained'] += 1
        else:
            out['answered'] += 1; out['answered_correct'] += ok
        conf.append(p[best]); hit.append(1.0 if ok else 0.0)
    out['ece'] = ece(conf, hit)
    out['brier'] /= len(items)
    for k in kinds:
        if k[0]:
            k[2] /= k[0]
    if kinds[2][0]:
        out['error'] /= kinds[2][0]
    out['kinds'] = kinds
    return out


def bits(x):
    return f'f64_from_bits(0x{struct.unpack("<Q", struct.pack("<d", x))[0]:016x})'


def literal(s):
    assert all(32 <= ord(c) < 127 or c in '\n\t' for c in s), s
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n').replace('\t', '\\t') + '"'


def answer_cases():
    rng = random.Random(1229)
    cases = []
    # Two-slot vectors that sum to exactly one, at and beside six-decimal midpoints.
    for n in [0, 1, 2, 3, 41, 99, 100, 757196, 999999]:
        mid = (n + 0.5) / 1e6
        for x in [math.nextafter(mid, 0), mid, math.nextafter(mid, 1)]:
            if 0 < x < 1 and x + (1 - x) == 1:
                cases.append((1, 0, [x, 1 - x], []))
                cases.append((0, 0, [x, 1 - x], ['first', 'second']))
    for index in range(36):
        kind = index % 3
        count = 2 if kind == 1 else rng.choice([1, 2, 3, 5, 10] if kind == 2 else [1, 2, 3, 7, 26])
        raw = [rng.random() ** rng.choice([1, 4, 12]) for _ in range(count)]
        p = [v / total(raw) for v in raw]
        if index % 5 == 0 and count > 1:
            p[-1] = p[0]
        names = [f'opt {i} "q"\\' if kind == 0 else ('' if i % 3 == 2 else f'level\t{i}\nnote') for i in range(count)]
        cases.append((kind, index % 2, p, names if kind != 1 else []))
    return cases


def quality_case():
    rng = random.Random(1230)
    items, probs = [], []
    for i in range(40):
        kind = i % 3
        count = 2 if kind == 1 else rng.choice([1, 2, 3, 4, 10] if kind == 2 else [1, 2, 3, 5])
        raw = [rng.random() ** 3 for _ in range(count)]
        p = [v / total(raw) for v in raw]
        abstain = count - 1 if kind == 0 and count > 1 and i % 4 == 0 else -1
        items.append((kind, count, len(probs), rng.randrange(count), abstain))
        probs.extend(p)
    return items, probs


def fixture():
    lines = ['// Generated by psionic/tests/answer_oracle.py; Python round() and json.dumps() decide every byte.']
    for index, (kind, fields, p, names) in enumerate(answer_cases()):
        want = answer(kind, fields, p, names)
        if kind == 0:
            opts = ','.join(f'PtOption.{{name={literal(n)}}}' for n in names)
        else:
            opts = ','.join('PtOption.{}' if n == '' else f'PtOption.{{description=PtEntry.{{kind=PtEntryKind.text,value={literal(n)}}}}}' for n in names)
        lines += [f'#test "independent answer fixture {index}" {{',
                  f'    p:=[{len(p)}]f64.[' + ','.join(bits(v) for v in p) + '];',
                  f'    options:[{max(len(names), 1)}]PtOption;' if not names else f'    options:=[{len(names)}]PtOption.[{opts}];',
                  f'    out:[{len(want.encode()) + 8}]u8;',
                  f'    r:=pj_answer(cast(QrKind){kind},{fields},p[..],options[0..<{len(names)}],out[..],false);',
                  f'    #assert r.status==PjStatus.ok && cast(string)out[0..<r.written]=={literal(want)};',
                  '}']
    return '\n'.join(lines) + '\n'


def quality_fixture():
    items, probs = quality_case()
    q = quality(items, probs)
    lines = ['// Generated by psionic/tests/answer_oracle.py from a transcription of qev/calibrate.py.']
    lines += ['#test "independent quality fixture over 40 random items" {',
              f'    p:=[{len(probs)}]f64.[' + ','.join(bits(v) for v in probs) + '];',
              f'    items:=[{len(items)}]PqItem.[' + ','.join(f'PqItem.{{kind=cast(QrKind){k},count={c},start={s},gold={g},abstain={a}}}' for k, c, s, g, a in items) + '];',
              '    r:=pq_measure(items[..],p[..]);',
              f'    #assert r.status==PqStatus.ok && r.items=={q["items"]} && r.correct=={q["correct"]};',
              f'    #assert r.offered=={q["offered"]} && r.abstained=={q["abstained"]} && r.answered=={q["answered"]} && r.answered_correct=={q["answered_correct"]};',
              f'    #assert abs(r.brier-{q["brier"]!r})<0.000000000001 && abs(r.ece-{q["ece"]!r})<0.000000000001 && abs(r.score_error-{q["error"]!r})<0.000000000001;']
    for k, (n, c, b) in enumerate(q['kinds']):
        lines.append(f'    #assert r.kinds[{k}].items=={n} && r.kinds[{k}].correct=={c} && abs(r.kinds[{k}].brier-{b!r})<0.000000000001;')
    lines.append('}')
    return '\n'.join(lines) + '\n'


if __name__ == '__main__':
    files = {OUT: fixture(), QUALITY: quality_fixture()}
    if sys.argv[1:] == ['--write']:
        for path, text in files.items():
            path.write_text(text)
    else:
        assert not sys.argv[1:], 'usage: answer_oracle.py [--write]'
        for path, text in files.items():
            assert path.read_text() == text, f'{path} drifted'
        print('chapter 12 answer and quality fixtures match the independent references')
