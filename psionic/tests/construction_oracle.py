#!/usr/bin/env python3
# Firm reason: 2 (independent network, postfix, artifact-digest, BFS and Q-learning oracle).
"""Read inert chapter-254 artifacts and verify measurements without Coax."""
import csv
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[2]
F = ROOT / 'psionic/fixtures/construction'
MAPS = ['....S/#.#.#/...../....G', 'S#G/.../...', '....S/#.###/...../....G', 'S..../####./G....']


def model(path):
    d = json.loads(path.read_text())
    d['p'] = [struct.unpack('>d', bytes.fromhex(w))[0] for w in d['weights_hex']]
    assert len(d['p']) == d['count'] and all(math.isfinite(x) for x in d['p'])
    data = bytearray()
    def word(n): data.extend(struct.pack('<Q', n & ((1 << 64)-1)))
    def text(s): word(len(s)); data.extend(s.encode())
    if d['mode'] == 0:
        ins, outs = (4, 4) if d['task'] == 3 else (3, 1)
        a, b, c, z = 4*ins, 4*ins+4, 4*ins+4+4*outs, 4*ins+4+4*outs+outs*ins
        text('weight-edit-f64-v1'); text('construct-relu4-f64-v1'); word(5); word(d['count'])
        for name, start, rank, dims in [('hidden.weight', 0, 2, [4, ins, 0, 0]), ('hidden.bias', a, 1, [4, 0, 0, 0]), ('output.weight', b, 2, [outs, 4, 0, 0]), ('skip.weight', c, 2, [outs, ins, 0, 0]), ('output.bias', z, 1, [outs, 0, 0, 0])]:
            text(name); word(start); word(rank)
            for v in dims: word(v)
    else:
        ins, outs = (4, 4) if d['task'] == 3 else (3, 1)
        for v in [d['layers'], d['width'], ins, outs, 0, d['count']]: word(v)
        for v in d['units']: word(v)
        for output in d['readout']:
            word(len(output['slots']))
            for slot, coef in zip(output['slots'], output['coeffs']): word(slot); word(coef)
    for w in d['weights_hex']: word(int(w, 16))
    assert hashlib.sha256(data).hexdigest() == d['digest'], path
    return d


def forward(d, x):
    p = d['p']
    if d['mode'] == 0:
        ins, outs = (4, 4) if d['task'] == 3 else (3, 1)
        a, b, c, z = 4*ins, 4*ins+4, 4*ins+4+4*outs, 4*ins+4+4*outs+outs*ins
        h = [max(0, p[a+k]+sum(p[k*ins+j]*x[j] for j in range(ins))) for k in range(4)]
        return [p[z+o]+sum(p[c+o*ins+j]*x[j] for j in range(ins))+sum(p[b+o*4+k]*h[k] for k in range(4)) for o in range(outs)]
    width = d['width']; v = [1.]+list(x)+[0.]*(width-len(x)-1); offset = 0
    for units in d['units']:
        vproj = [sum(p[offset+u*width+i]*v[i] for i in range(width)) for u in range(units)]
        gates = [max(0, sum(p[offset+units*width+u*width+i]*v[i] for i in range(width))) for u in range(units)]
        v = [v[i]+sum(p[offset+2*units*width+i*units+u]*vproj[u]*gates[u] for u in range(units)) for i in range(width)]
        offset += 3*units*width
    assert offset == len(p)
    return [sum(coef*v[slot] for slot, coef in zip(o['slots'], o['coeffs'])) for o in d['readout']]


def postfix(programs, x):
    result = []
    for tokens in programs:
        s = []
        for t in tokens:
            if 0 <= t < len(x): s.append(x[t]); continue
            if t in (10, 11, 14):
                b, a = s.pop(), s.pop(); s.append(a+b if t == 10 else a-b if t == 11 else a*b)
            elif t == 12: s.append(max(0, s.pop()))
            elif t == 13: s.append(-s.pop())
            elif t == 15: s.append(int(s.pop() >= 1))
            elif t == 16:
                b, a, bit = s.pop(), s.pop(), s.pop(); s.append(a if bit else b)
            else: raise ValueError(t)
            if abs(s[-1]) > 2**26: raise OverflowError
        assert len(s) == 1; result.append(s[0])
    return result


def admit(text, task, mode):
    """Independent format/type admission, before evaluating any output."""
    try:
        value = json.loads(text)
    except (ValueError, TypeError):
        return 102 if mode else 3
    ins, outs = (4, 4) if task == 3 else (3, 1)
    if not isinstance(value, list) or len(value) != (outs if mode else 4*ins+4+4*outs+ins*outs+outs):
        return 102 if mode else 3
    if not mode:
        return 0 if all(type(v) is int and -1024 <= v <= 1024 for v in value) else 2
    for program in value:
        if not isinstance(program, list) or not 1 <= len(program) <= 24: return 101
        stack = []
        for token in program:
            if type(token) is not int: return 103
            if 0 <= token < ins: stack.append(False); continue
            if token not in range(10, 17): return 103
            arity = 3 if token == 16 else 2 if token in (10, 11, 14) else 1
            if len(stack) < arity: return 104
            if token == 16 and not stack[-3]: return 105
            del stack[-arity:]; stack.append(token == 15)
        if len(stack) != 1: return 104
    return 0


def truth(task, x): return max(x[0], x[1]) if task == 0 else abs(x[0]) if task == 1 else x[0] if x[2] else x[1]
def inputs(held):
    if not held:
        return [(a, b, c) for a in range(-6, 7) for b in range(-6, 7) if abs(a) > 4 or abs(b) > 4 for c in (0, 1)]
    edge = [-64, -63, -32, 0, 32, 63, 64]
    return ([(a, b, c) for a in range(-16, 17) for b in range(-16, 17) if abs(a) > 6 or abs(b) > 6 for c in (0, 1)] +
            [(a, b, c) for a in edge for b in edge if a or b for c in (0, 1)])


def arithmetic(d, held, program=None, cases=None):
    exact = close = refused = 0; error = worst = 0.
    cases = inputs(held) if cases is None else cases
    for x in cases:
        if program is not None:
            try: expected = postfix(program, x)
            except OverflowError: refused += 1; continue
        y = forward(d, x)[0]
        if program is not None: assert y == expected[0]
        e = abs(y-truth(d['task'], x)); exact += e <= 1e-9; close += e <= .01; error += e; worst = max(worst, e)
    return exact, len(cases), close, refused, error/(len(cases)-refused), worst


def development(d, program=None):
    if d['task'] < 3:
        r = arithmetic(d, False, program)
        return r[0], r[1], r[4]*(r[1]-r[3])
    exact = total = 0; error = 0.
    for index in [0, 1]:
        cells, goal, nxt, dist = world(index)
        for s in cells:
            if s == goal or s not in dist: continue
            x = [64 if nxt[s, a] == s else dist.get(nxt[s, a], 64) for a in range(4)]
            y = forward(d, x)
            if program is not None: assert y == postfix(program, x)
            exact += max(range(4), key=y.__getitem__) == min(range(4), key=x.__getitem__)
            total += 1; error += sum(abs(a+b) for a, b in zip(x, y))/4
    return exact, total, error


def world(index):
    rows = MAPS[index].split('/'); cells = {r*8+c: v for r, row in enumerate(rows) for c, v in enumerate(row) if v != '#'}
    goal = next(s for s, v in cells.items() if v == 'G')
    nxt = {}
    for s in cells:
        r, c = divmod(s, 8)
        for a, (dr, dc) in enumerate([(-1, 0), (0, 1), (1, 0), (0, -1)]):
            rr, cc = r+dr, c+dc; t = rr*8+cc
            nxt[s, a] = t if 0 <= rr < len(rows) and 0 <= cc < len(rows[0]) and t in cells else s
    dist = {goal: 0}; queue = [goal]
    for t in queue:
        for s in cells:
            if s not in dist and any(nxt[s, a] == t for a in range(4)): dist[s] = dist[t]+1; queue.append(s)
    return cells, goal, nxt, dist


def walks(index, d=None, q=None):
    cells, goal, nxt, dist = world(index); reached = optimal = steps = collisions = 0
    starts = [s for s in cells if s != goal and s in dist]
    for start in starts:
        s = start; n = 0
        while s != goal and n < 128:
            if q is None:
                x = [64 if nxt[s, a] == s else dist.get(nxt[s, a], 64) for a in range(4)]
                logits = forward(d, x); action = max(range(4), key=logits.__getitem__)
            else: action = max(range(4), key=lambda a: q[s*4+a])
            t = nxt[s, action]; collisions += t == s; steps += 1; n += 1; s = t
        reached += s == goal; optimal += s == goal and n == dist[start]
    return len(starts), reached, optimal, steps, collisions


def rng(seed, counter):
    mask = 2**64-1; z = (seed+(counter+1)*0x9e3779b97f4a7c15) & mask
    z = ((z ^ (z >> 30))*0xbf58476d1ce4e5b9) & mask; z = ((z ^ (z >> 27))*0x94d049bb133111eb) & mask
    return z ^ (z >> 31)


def qlearn(index, seed):
    cells, goal, nxt, _ = world(index); starts = sorted(s for s in cells if s != goal); q = [0.]*256; counter = steps = 0
    for _ in range(512):
        s = starts[rng(seed, counter) % len(starts)]; counter += 1
        for _ in range(128):
            if s == goal: break
            action = max(range(4), key=lambda a: q[4*s+a])
            if rng(seed, counter) % 16 < 4: action = rng(seed, counter+1) % 4
            counter += 2; t = nxt[s, action]; q[4*s+action] = -1+(0 if t == goal else .9*max(q[4*t:4*t+4])); steps += 1; s = t
    return q, steps


def verify():
    count = 0
    for row in csv.DictReader((F / 'baselines.csv').open()):
        task = int(row['task']); kind = {'zero': 0, 'hand': 1, 'compiler-reference': 2, 'planted-M64': 6}.get(row['method'], 3+int(row['seed'])-254)
        d = model(F / f'baseline-{task*7+kind}.json')
        dev = development(d)
        assert dev[:2] == tuple(int(row[k]) for k in ['dev_exact', 'dev_total'])
        if task < 3:
            got = arithmetic(d, True)
            assert got[:4] == tuple(int(row[k]) for k in ['held_exact', 'held_total', 'held_close', 'refused']), (row, got)
            assert abs(got[4]-float(row['mae'])) < 1e-7 and abs(got[5]-float(row['worst'])) < 1e-7
        else:
            rs = [walks(i, d) for i in [2, 3]]
            assert sum(r[0] for r in rs) == int(row['held_total']) and sum(r[1] for r in rs) == int(row['held_exact']) and sum(r[2] for r in rs) == int(row['held_close'])
        count += 1
    for row in csv.DictReader((F / 'grid.csv').open()):
        index = int(row['map']); seed = int(row['seed']); name = row['method']
        if name == 'exact-planner':
            cells, goal, nxt, dist = world(index); q = [0.]*256
            for s in cells:
                for a in range(4): q[4*s+a] = -dist[nxt[s, a]]
            got = walks(index, q=q)
        elif name.startswith('q-'):
            q, steps = qlearn(0 if name == 'q-frozen-A' else index, seed); assert steps == int(row['training_transitions']); got = walks(index, q=q)
        else:
            kind = 0 if name == 'zero' else 1 if name == 'hand' else 3+seed-254; got = walks(index, model(F / f'baseline-{21+kind}.json'))
        assert got == tuple(int(row[k]) for k in ['starts', 'reached', 'optimal', 'steps', 'collisions']), (row, got)
    proposals = 0; usage = {k: 0 for k in ['input_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens', 'output_tokens']}
    cost = seconds = 0.; admitted = 0; failed = []; small_counts = []; predictions = selection_forwards = 0
    for task in range(4):
        best = {0: None, 1: None}
        for row in csv.DictReader((F / f'llm-{task}.csv').open()):
            case = int(row['case']); response = json.loads((F / f'llm/{case}.response.json').read_text())
            assert response.get('permission_denials') == [] and response['subagent_stats']['spawned'] == 0
            assert response['subtype'] == 'success' and response['is_error'] is False
            request = json.loads((F / f'llm/{case}.request.json').read_text())
            assert request['tools'] == [] and request['model'] == 'claude-opus-5-5'
            for k in usage: usage[k] += response['usage'][k]
            cost += response['total_cost_usd']; seconds += int((F / f'llm/{case}.milliseconds').read_text())/1000
            reason = admit(response['result'], task, int(row['mode']))
            assert reason == int(row['reason']), (row, reason)
            if row['valid'] != 'true':
                assert reason != 0 and not (F / f'llm/{case}.artifact.json').exists()
                failed.append(case); proposals += 1; continue
            admitted += 1
            d = model(F / f'llm/{case}.artifact.json'); program = json.loads(response['result']) if int(row['mode']) else None
            assert d['count'] == int(row['weights'])
            if not int(row['mode']): assert d['p'] == json.loads(response['result'])
            else: assert sum(len(o['coeffs']) for o in d['readout']) == int(row['readout_coefficients'])
            dev = development(d, program)
            assert dev[:2] == tuple(int(row[k]) for k in ['dev_exact', 'dev_total'])
            predictions += dev[1]
            if int(row['turn']) != 1: selection_forwards += dev[1]
            route = int(row['mode'])
            if int(row['turn']) != 1 and (best[route] is None or (dev[0], -dev[2]) > best[route][0]): best[route] = ((dev[0], -dev[2]), case)
            if task < 3:
                got = arithmetic(d, True, program)
                assert got[:4] == tuple(int(row[k]) for k in ['held_exact', 'held_total', 'held_close', 'held_refused']), (row, got)
                assert abs(got[4]-float(row['mae'])) < 1e-7 and abs(got[5]-float(row['worst'])) < 1e-7
                predictions += got[1]-got[3]
                small = [(a,b,c) for a in range(-6,7) for b in range(-6,7) for c in (0,1)]
                check = arithmetic(d, False, program, small); small_counts.append((case, check[0], check[1]))
                assert check[0] == check[1] and check[3] == 0
            else:
                rs = [walks(i, d) for i in [2, 3]]
                assert sum(r[1] for r in rs) == int(row['held_exact']) and sum(r[2] for r in rs) == int(row['held_close'])
                predictions += sum(r[3] for r in rs)
            proposals += 1
        rows = list(csv.DictReader((F / f'llm-{task}.csv').open()))
        assert {int(r['case']) for r in rows if r['selected'] == 'true'} == {b[1] for b in best.values() if b is not None}
    print(f'construction oracle: {count} baselines, grid/Q-learning rows, {proposals} proposals, artifact digests and compiled weight execution agree')
    print(f'proposals: {admitted} admitted; failures {failed}; small-domain post-selection audit: {small_counts}')
    print(f'model usage: {usage}; reported list USD {cost:.6f}; serial wall seconds {seconds:.3f}; ledger forwards {predictions}, selection forwards {selection_forwards}, total {predictions+selection_forwards}')


if __name__ == '__main__': verify()
