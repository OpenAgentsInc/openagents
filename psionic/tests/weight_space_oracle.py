#!/usr/bin/env python3
# Firm reason: 2 (independent SHA-256 artifact encoding and full-prefix decoder oracle).
"""Recompute chapter 253's recorded metrics without loading any Coax code.

The scalar equations use Python lists and math, no training framework. The
source fixtures are inert lists of IEEE-754 encodings, parsed as data only.
"""
import csv
import hashlib
import json
import math
from pathlib import Path
import re
import struct

ROOT = Path(__file__).resolve().parents[2]
FIX = ROOT / 'psionic/fixtures/weight-space'


def artifacts(name):
    text = (FIX / name).read_text()
    return [[struct.unpack('<d', struct.pack('<Q', int(n)))[0] for n in row.split(',')]
            for row in re.findall(r'\[664\]u64\.\[([0-9,]+)\]', text)]


def identity():
    data = bytearray()
    def word(n): data.extend(struct.pack('<Q', n))
    def string(s): word(len(s)); data.extend(s.encode('ascii'))
    string('weight-edit-f64-v1'); string('linear-v1'); word(2); word(4)
    for name, first, rank, dims in [('matrix', 0, 1, [3, 0, 0, 0]), ('bias', 3, 1, [1, 0, 0, 0])]:
        string(name); word(first); word(rank)
        for d in dims: word(d)
    data.extend(struct.pack('<4d', 1, 2, 3, 4))
    assert hashlib.sha256(data).hexdigest() == 'e03f72deb73177195f723b83c8fd7542ddb7f6e44254a67114dee02266d8b173'


def dot(a, b): return sum(x*y for x, y in zip(a, b))
def mv(p, offset, rows, x): return [dot(p[offset+8*r:offset+8*r+8], x) for r in range(rows)]
def norm(x, g):
    scale = 1/math.sqrt(dot(x, x)/8+1e-6)
    return [a*b*scale for a, b in zip(x, g)]


def rope(x, t):
    result = list(x)
    for h in range(2):
        for pair in range(2):
            i = h*4+2*pair; angle = t*10000**(-pair/2)
            result[i] = x[i]*math.cos(angle)-x[i+1]*math.sin(angle)
            result[i+1] = x[i]*math.sin(angle)+x[i+1]*math.cos(angle)
    return result


def logits(p, answer, hint):
    # Independently evaluate the whole prefix, then select the query logits.
    xs = [p[t*8:t*8+8] for t in [15, answer, hint, 14]]
    normalized = [norm(x, p[128:136]) for x in xs]
    qs = [rope(mv(p, 136, 8, x), t) for t, x in enumerate(normalized)]
    ks = [rope(mv(p, 200, 8, x), t) for t, x in enumerate(normalized)]
    vs = [mv(p, 264, 8, x) for x in normalized]
    context = []
    for head in range(2):
        sl = slice(head*4, head*4+4)
        scores = [dot(qs[3][sl], k[sl])/2 for k in ks]
        exps = [math.exp(s-max(scores)) for s in scores]
        probs = [e/sum(exps) for e in exps]
        context.extend(sum(probs[t]*vs[t][head*4+j] for t in range(4)) for j in range(4))
    mid = [a+b for a, b in zip(xs[3], mv(p, 328, 8, context))]
    hidden = [max(0, x)**2 for x in mv(p, 400, 8, norm(mid, p[392:400]))]
    final = [a+b for a, b in zip(mid, mv(p, 464, 8, hidden))]
    return mv(p, 536, 16, norm(final, p[528:536]))


def nll(values, answer):
    top = max(values)
    return math.log(sum(math.exp(x-top) for x in values))+top-values[answer]


def measure(p, first=4, last=6):
    right = retained = confident = 0; loss = effect = 0.
    for a in range(first, last+1):
        clean = logits(p, a, a+6)
        retained += max(range(16), key=clean.__getitem__) == a
        plain = logits(p, a, 13)
        retained += max(range(16), key=plain.__getitem__) == a
        for b in range(1, 7):
            if a == b: continue
            lg = logits(p, a, b+6); guess = max(range(16), key=lg.__getitem__)
            right += guess == a
            confident += guess != a and nll(lg, guess) < -math.log(.9)
            loss += nll(lg, a); effect += lg[b]-lg[a]-clean[b]+clean[a]
    return right, retained, confident, loss/15, effect/15


def edited(p, site):
    q = p.copy(); cols = range(site*4, site*4+4) if site < 2 else [site-2]; base = 328 if site < 2 else 464
    for row in range(8):
        for col in cols: q[base+row*8+col] = 0.
    return q


def initialized(seed):
    mask = (1 << 64)-1
    def draw(i):
        z = (seed+(i+1)*0x9e3779b97f4a7c15) & mask
        z = ((z ^ (z >> 30))*0xbf58476d1ce4e5b9) & mask
        z = ((z ^ (z >> 27))*0x94d049bb133111eb) & mask
        return ((z ^ (z >> 31)) >> 11)/2**53
    p = [(2*draw(i)-1)/math.sqrt(8) for i in range(664)]
    for at in [128, 392, 528]: p[at:at+8] = [1.]*8
    return p


def run():
    identity(); fresh = artifacts('fresh.coax'); assert len(fresh) == 10 and all(len(p) == 664 for p in fresh)
    rows = list(csv.DictReader((FIX / 'matrix.csv').open()))
    checked = 0
    for row in rows:
        model = int(row['model']); method = row['method']; site = int(row['site'])
        if model < 10: p = fresh[model]
        elif model < 15: p = initialized(model+109)
        else: p = fresh[model-15][:536]+[0.]*128
        if method == 'development-best':
            baseline = measure(p, 1, 3); best = -1; best_loss = baseline[3]
            for candidate in range(10):
                trial = measure(edited(p, candidate), 1, 3)
                if trial[1] >= baseline[1] and trial[3] < best_loss:
                    best = candidate; best_loss = trial[3]
            assert site == best, ('development selection', model, site, best)
        if method in ['head', 'development-best'] and site >= 0: p = edited(p, site)
        elif method == 'random32':
            p = p.copy()
            for i in range(32): p[(site*127+i*37+19) % 664] = 0.
        got = measure(p)
        assert got[:3] == tuple(int(row[k]) for k in ['right', 'retained', 'confident_errors']), (model, method, got, row)
        for actual, key in zip(got[3:], ['nll', 'hint_effect']): assert abs(actual-float(row[key])) < 2e-9, (model, method, key, actual, row[key])
        checked += 1
    fits = artifacts('fit.coax'); assert len(fits) == 5
    expected = [(12, 6, 350), (14, 6, 349), (15, 6, 20), (8, 6, 2085), (13, 6, 470)]
    for p, want in zip(fits, expected):
        m = measure(p); assert (m[0], m[1], math.floor(m[3]*1000+.5)) == want
    llm_rows = list(csv.DictReader((FIX / 'llm.csv').open())); assert len(llm_rows) == 32
    for row in llm_rows:
        case = int(row['case']); raw = json.loads((FIX / f'llm/{case}.response.json').read_text())
        final = [c['text'] for x in raw.get('output', []) if x.get('type') == 'message'
                 for c in x.get('content', []) if c.get('type') == 'output_text']
        proposal = final[0] if raw.get('status') == 'completed' and final else 'INVALID'
        assert row['proposal'] == proposal
        if case < 12:
            prompt = json.loads((FIX / f'llm/{case}.request.json').read_text())['input']
            numbers = re.search(r'Input weights \(4x2\): ([^\n]+)\nHidden biases \(4\): ([^\n]+)\nOutput weights \(2x4\): ([^\n]+)', prompt)
            w1 = [float(x) for x in numbers[1].split(',') if x]
            b1 = [float(x) for x in numbers[2].split(',') if x]
            w2 = [float(x) for x in numbers[3].split(',') if x]
            def score(weights):
                correct = 0
                for x in [-3, -2, -1, 1, 2, 3]:
                    for y in range(-3, 4):
                        h = [max(0, w1[2*i]*x+w1[2*i+1]*y+b1[i]) for i in range(4)]
                        correct += (dot(weights[4:], h) > dot(weights[:4], h)) == (x > 0)
                return correct
            assert score(w2) == int(row['base_right'])
            if re.fullmatch(r'[0-3],[0-3]', proposal) and proposal[0] != proposal[2]:
                for i in [int(proposal[0]), int(proposal[2])]: w2[i] = w2[4+i] = 0
            assert score(w2) == int(row['edited_right'])
        else:
            at = (case-12) % 10; model, perm = divmod(at, 2); p = fresh[model]
            assert measure(p)[0] == int(row['base_right'])
            if proposal in ['0', '1']: p = edited(p, 1-int(proposal) if perm else int(proposal))
            assert measure(p)[0] == int(row['edited_right'])
    print(f'weight-space oracle: artifact SHA-256, {checked} decoder rows, development choices, 5 fine-tunes and 32 saved model trials agree')


if __name__ == '__main__': run()
