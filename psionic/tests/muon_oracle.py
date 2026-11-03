#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent nanochat MuonAdamW, written from the pinned reference.

Source: karpathy/nanochat 92d63d4e8bb4df75c3b71618f31ddde2378b2bcd,
nanochat/optim.py (sha256 7c9b714c...afed8f) and GPT.setup_optimizer in
nanochat/gpt.py (sha256 8cdedc2e...04e3). Read, never executed.

This oracle keeps nanochat's own structure: named parameters as nested Python
lists, setup_optimizer's param_groups as dictionaries, optimizer state keyed by
parameter name (AdamW per parameter, Muon per group), and torch semantics
spelled out: lerp_ takes torch's two-branch form, norms are square roots of
sums of squares, clamp_min is max. Everything is a Python float (binary64), the
widest reading of nanochat's fp32 path; the bf16 cast of X is not modeled.

sin, cos and every power are correctly rounded (cr_math.py), not the C
library's, so the reference is the same on every platform (#12695); sqrt is
IEEE-exact everywhere. sum() is Python 3.12's compensated sum.

--write regenerates psionic/fixtures/muon/{reference.json,reference.coax};
the default verifies the committed files reproduce; --header prints C goldens.
"""
import argparse, json, math, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import cr_math  # noqa: E402

assert sys.version_info >= (3, 12), "sum() must be the compensated sum of Python 3.12+"

D = Path(__file__).resolve().parents[1] / 'fixtures/muon'

POLAR = [
    (8.156554524902461, -22.48329292557795, 15.878769915207462),
    (4.042929935166739, -2.808917465908714, 0.5000178451051316),
    (3.8916678022926607, -2.772484153217685, 0.5060648178503393),
    (3.285753657755655, -2.3681294933425376, 0.46449024233003106),
    (2.3465413258596377, -1.7097828382687081, 0.42323551169305323),
]

def lerp(start, end, weight):
    # torch: weight < 0.5 ? start + weight*(end-start) : end - (end-start)*(1-weight)
    return start + weight * (end - start) if weight < 0.5 else end - (end - start) * (1 - weight)

def mat(rows, cols, f):
    return [[f(i, j) for j in range(cols)] for i in range(rows)]

def total(values):
    # Left to right. Python's sum() of floats is compensated (3.12+), torch's is not.
    s = 0.0
    for v in values:
        s += v
    return s

def matmul(a, b):
    bt = list(zip(*b))
    return [[total(x * y for x, y in zip(row, col)) for col in bt] for row in a]

def transpose(a):
    return [list(r) for r in zip(*a)]

def fro(a):
    return math.sqrt(sum(x * x for r in a for x in r))

def scale(a, s):
    return [[x * s for x in r] for r in a]

def polar(X, ns_steps):
    """The Polar Express loop of muon_step_fused alone, on a normalized X."""
    r, c = len(X), len(X[0])
    for a, b, cc in POLAR[:ns_steps]:
        if r > c:
            A = matmul(transpose(X), X)
            AA = matmul(A, A)
            B = [[b * A[i][j] + cc * AA[i][j] for j in range(c)] for i in range(c)]
            XB = matmul(X, B)
            X = [[a * X[i][j] + XB[i][j] for j in range(c)] for i in range(r)]
        else:
            A = matmul(X, transpose(X))
            AA = matmul(A, A)
            B = [[b * A[i][j] + cc * AA[i][j] for j in range(r)] for i in range(r)]
            BX = matmul(B, X)
            X = [[a * X[i][j] + BX[i][j] for j in range(c)] for i in range(r)]
    return X

# Polar Express alone on square, tall and wide inputs: these must match exactly.
POLAR_CASES = [(4, 4), (5, 3), (3, 5)]

def polar_case(r, c):
    X = mat(r, c, lambda i, j: cr_math.cos(0.9 * i - 0.4 * j + r) * (1 + 0.3 * j))
    X = scale(X, 1 / (fro(X) * 1.01 + 1e-6))
    return dict(rows=r, cols=c, input=[x for row in X for x in row], output=[x for row in polar(X, 5) for x in row])

def muon_fused(grad, param, buf, second, momentum, lr, wd, beta2, ns_steps, red_dim):
    """muon_step_fused for one matrix of the stack (every reduction is per matrix)."""
    r, c = len(grad), len(grad[0])
    buf = [[lerp(buf[i][j], grad[i][j], 1 - momentum) for j in range(c)] for i in range(r)]
    g = [[lerp(grad[i][j], buf[i][j], momentum) for j in range(c)] for i in range(r)]
    X = g
    target = fro(X) / cr_math.pow(r, 0.5)
    X = [[x * (target / max(math.sqrt(sum(y * y for y in row)), 1e-6)) for x in row] for row in X]
    X = scale(X, 1 / (fro(X) * 1.01 + 1e-6))
    for a, b, cc in POLAR[:ns_steps]:
        if r > c:
            A = matmul(transpose(X), X)
            AA = matmul(A, A)
            B = [[b * A[i][j] + cc * AA[i][j] for j in range(c)] for i in range(c)]
            XB = matmul(X, B)
            X = [[a * X[i][j] + XB[i][j] for j in range(c)] for i in range(r)]
        else:
            A = matmul(X, transpose(X))
            AA = matmul(A, A)
            B = [[b * A[i][j] + cc * AA[i][j] for j in range(r)] for i in range(r)]
            BX = matmul(B, X)
            X = [[a * X[i][j] + BX[i][j] for j in range(c)] for i in range(r)]
    g = X
    target_norm = cr_math.pow(min(r, c), 0.5)
    g = scale(g, target_norm / max(fro(g), 1e-6))
    if red_dim == -1:
        v_mean = [sum(x * x for x in row) / c for row in g]
        size = c
    else:
        v_mean = [sum(cr_math.pow(g[i][j], 2) for i in range(r)) / r for j in range(c)]
        size = r
    v_norm = math.sqrt(sum(v_mean) * size)
    second = [lerp(s, v, 1 - beta2) for s, v in zip(second, v_mean)]
    step = [1 / math.sqrt(max(s, 1e-10)) for s in second]
    v_norm_new = math.sqrt(sum(v * size * st * st for v, st in zip(v_mean, step)))
    final = [st * (v_norm / max(v_norm_new, 1e-10)) for st in step]
    if red_dim == -1:
        g = [[g[i][j] * final[i] for j in range(c)] for i in range(r)]
    else:
        g = [[g[i][j] * final[j] for j in range(c)] for i in range(r)]
    param = [[param[i][j] - (lr * g[i][j] + lr * wd * param[i][j] * (1.0 if g[i][j] * param[i][j] >= 0 else 0.0))
              for j in range(c)] for i in range(r)]
    return param, buf, second

def adamw_fused(p, grad, m, v, step, lr, b1, b2, eps, wd):
    p = [x * (1 - lr * wd) for x in p]
    m = [lerp(a, g, 1 - b1) for a, g in zip(m, grad)]
    v = [lerp(a, g * g, 1 - b2) for a, g in zip(v, grad)]
    bias1 = 1 - cr_math.pow(b1, step)
    bias2 = 1 - cr_math.pow(b2, step)
    p = [x - (lr / bias1) * (a / (math.sqrt(s / bias2) + eps)) for x, a, s in zip(p, m, v)]
    return p, m, v

def setup_optimizer(model, model_dim, unembedding_lr=0.004, embedding_lr=0.2, matrix_lr=0.02, weight_decay=0.0, scalar_lr=0.5):
    """GPT.setup_optimizer: names grouped exactly as nanochat groups modules."""
    by = lambda role: [name for name, (r, shape) in model if r == role]
    s = cr_math.pow(model_dim / 768, -0.5)
    groups = [
        dict(kind='adamw', params=by('lm_head'), lr=unembedding_lr * s, betas=(0.8, 0.96), eps=1e-10, weight_decay=0.01),
        dict(kind='adamw', params=by('embedding'), lr=embedding_lr * s, betas=(0.8, 0.995), eps=1e-10, weight_decay=0.001),
        dict(kind='adamw', params=by('value_embed'), lr=embedding_lr * s * 0.5, betas=(0.8, 0.995), eps=1e-10, weight_decay=0.01),
        dict(kind='adamw', params=by('resid'), lr=scalar_lr * 0.01, betas=(0.8, 0.95), eps=1e-10, weight_decay=0.05),
        dict(kind='adamw', params=by('x0'), lr=scalar_lr, betas=(0.96, 0.95), eps=1e-10, weight_decay=0.0),
        dict(kind='adamw', params=by('smear_gate') + by('smear_lambda') + by('backout'), lr=0.2, betas=(0.8, 0.95), eps=1e-10, weight_decay=0.0),
    ]
    shapes = dict(model)
    matrices = by('matrix')
    for shape in sorted({shapes[n][1] for n in matrices}):
        groups.append(dict(kind='muon', params=[n for n in matrices if shapes[n][1] == shape], lr=matrix_lr,
                           momentum=0.95, ns_steps=5, beta2=0.9, weight_decay=weight_decay))
    return groups

def numel(shape):
    return shape[0] * shape[1]

def opt_step(groups, shapes, params, grads, state):
    """MuonAdamW.step on one rank: every rank-owned path, no communication."""
    for group in groups:
        if group['kind'] == 'adamw':
            for name in group['params']:
                st = state.setdefault(name, dict(step=0, exp_avg=[0.0] * numel(shapes[name]), exp_avg_sq=[0.0] * numel(shapes[name])))
                st['step'] += 1
                params[name], st['exp_avg'], st['exp_avg_sq'] = adamw_fused(
                    params[name], grads[name], st['exp_avg'], st['exp_avg_sq'], st['step'],
                    group['lr'], group['betas'][0], group['betas'][1], group['eps'], group['weight_decay'])
        else:
            names = group['params']
            r, c = shapes[names[0]]
            key = names[0]
            red = r if r >= c else c
            st = state.setdefault(key, dict(momentum_buffer=[[0.0] * (r * c) for _ in names], second_momentum_buffer=[[0.0] * red for _ in names]))
            red_dim = -1 if r >= c else -2
            lr = group['lr'] * cr_math.pow(max(1.0, r / c), 0.5)
            for k, name in enumerate(names):
                as_mat = lambda flat: [flat[i * c:(i + 1) * c] for i in range(r)]
                p, buf, second = muon_fused(as_mat(grads[name]), as_mat(params[name]), as_mat(st['momentum_buffer'][k]),
                                            st['second_momentum_buffer'][k], group['momentum'], lr,
                                            group['weight_decay'], group['beta2'], group['ns_steps'], red_dim)
                params[name] = [x for row in p for x in row]
                st['momentum_buffer'][k] = [x for row in buf for x in row]
                st['second_momentum_buffer'][k] = second

# A one-layer nanochat-shaped model (width 4, vocabulary 6, two kv heads),
# in GPT's registration order: wte, the block's attention (c_q, c_k, c_v,
# c_proj, ve_gate) and MLP (c_fc, c_proj), lm_head, the scalars, the smear and
# backout parameters, the value embedding. Shapes are (out, in), as in torch.
NANOCHAT = [
    ('transformer.wte.weight', ('embedding', (6, 4))),
    ('transformer.h.0.attn.c_q.weight', ('matrix', (4, 4))),
    ('transformer.h.0.attn.c_k.weight', ('matrix', (4, 4))),
    ('transformer.h.0.attn.c_v.weight', ('matrix', (4, 4))),
    ('transformer.h.0.attn.c_proj.weight', ('matrix', (4, 4))),
    ('transformer.h.0.attn.ve_gate.weight', ('matrix', (2, 12))),
    ('transformer.h.0.mlp.c_fc.weight', ('matrix', (16, 4))),
    ('transformer.h.0.mlp.c_proj.weight', ('matrix', (4, 16))),
    ('lm_head.weight', ('lm_head', (6, 4))),
    ('resid_lambdas', ('resid', (1, 1))),
    ('x0_lambdas', ('x0', (1, 1))),
    ('smear_gate.weight', ('smear_gate', (1, 24))),
    ('smear_lambda', ('smear_lambda', (1, 1))),
    ('backout_lambda', ('backout', (1, 1))),
    ('value_embeds.0.weight', ('value_embed', (6, 4))),
]
ROLES = ['lm_head', 'embedding', 'value_embed', 'resid', 'x0', 'smear_gate', 'smear_lambda', 'backout', 'matrix']
RANK1 = {'resid_lambdas', 'x0_lambdas', 'smear_lambda', 'backout_lambda'}

def initial(name, shape, salt):
    n = numel(shape)
    return [((i * 7 + len(name) + salt) % 11 - 5) / 4 for i in range(n)]

def gradient(name, shape, step, salt):
    n = numel(shape)
    out = [cr_math.sin(0.37 * i + 1.3 * step + len(name) + salt) * (1 + 0.5 * step) for i in range(n)]
    if name.endswith('c_fc.weight') and step == 0:
        # a zero row and a row of norm ~1e-9: both row-norm clamps act
        for j in range(4):
            out[3 * 4 + j] = 0.0
            out[5 * 4 + j] = 5e-10 * (j + 1)
    return out

def layout(groups, shapes):
    rows, offsets, first, second = [], {}, 0, 0
    for g in groups:
        if not g['params']:
            continue  # an empty nanochat group updates nothing
        count = sum(numel(shapes[n]) for n in g['params'])
        for n in g['params']:
            offsets[n] = first + sum(numel(shapes[m]) for m in g['params'][:g['params'].index(n)])
        if g['kind'] == 'adamw':
            rows.append(dict(kind='adamw', first=first, count=count, rows=0, cols=0, lr=g['lr'], beta1=g['betas'][0],
                             beta2=g['betas'][1], eps=g['eps'], decay=g['weight_decay'], momentum=0.0, ns=0))
            second += count
        else:
            r, c = shapes[g['params'][0]]
            rows.append(dict(kind='muon', first=first, count=count, rows=r, cols=c, lr=g['lr'], beta1=0.0, beta2=g['beta2'],
                             eps=0.0, decay=g['weight_decay'], momentum=g['momentum'], ns=g['ns_steps']))
            second += len(g['params']) * max(r, c)
        first += count
    return rows, offsets, first, second

def flat(groups, shapes, params, state):
    p, m, s = [], [], []
    for g in groups:
        if not g['params']:
            continue
        if g['kind'] == 'adamw':
            for n in g['params']:
                p += params[n]
                st = state.get(n)
                m += st['exp_avg'] if st else [0.0] * numel(shapes[n])
                s += st['exp_avg_sq'] if st else [0.0] * numel(shapes[n])
        else:
            st = state.get(g['params'][0])
            for k, n in enumerate(g['params']):
                p += params[n]
                m += st['momentum_buffer'][k] if st else [0.0] * numel(shapes[n])
                s += st['second_momentum_buffer'][k] if st else [0.0] * max(shapes[n])
    return p, m, s

def run(name, model, hyper, steps, salt, override=None):
    shapes = {n: shape for n, (r, shape) in model}
    groups = setup_optimizer(model, **hyper)
    if override:
        for g in groups:
            if g['kind'] == 'muon':
                g.update(override)
    rows, offsets, n, second = layout(groups, shapes)
    params = {nm: initial(nm, shapes[nm], salt) for nm, _ in model}
    state = {}
    start, _, _ = flat(groups, shapes, params, state)
    trace = []
    for t in range(steps):
        grads = {nm: gradient(nm, shapes[nm], t, salt) for nm, _ in model}
        opt_step(groups, shapes, params, grads, state)
        g, _, _ = flat(groups, shapes, grads, {})
        p, m, s = flat(groups, shapes, params, state)
        trace.append(dict(grads=g, params=p, m=m, s=s))
    tensors = [dict(name=nm, role=r, rank=1 if nm in RANK1 else 2, rows=shape[0], cols=shape[1], offset=offsets[nm]) for nm, (r, shape) in model]
    return dict(name=name, hyper=hyper, override=override or {}, n=n, second=second, steps=steps, tensors=tensors,
                groups=rows, start=start, trace=trace)

CASES = [
    # the planner's own groups at nanochat's defaults with base_train's 0.28 Muon decay
    ('nanochat', NANOCHAT, dict(model_dim=4, weight_decay=0.28), 4, 0, None),
    # Muon only (one matrix of each kind), other hyperparameters and three Polar steps
    ('muon', [('a', ('matrix', (5, 3))), ('b', ('matrix', (3, 7))), ('c', ('matrix', (3, 3))), ('c2', ('matrix', (3, 3))),
              ('e', ('embedding', (2, 2))), ('h', ('lm_head', (2, 2))), ('r', ('resid', (1, 1))), ('x', ('x0', (1, 1))),
              ('g', ('smear_gate', (1, 2))), ('l', ('smear_lambda', (1, 1))), ('o', ('backout', (1, 1)))],
     dict(model_dim=768, matrix_lr=0.05, weight_decay=0.5), 5, 3, dict(momentum=0.85, beta2=0.95, ns_steps=3)),
]

def build():
    return dict(schema=1, profile='cpu-muon-v1', master='binary64', compute='binary64',
                source='karpathy/nanochat@92d63d4e8bb4df75c3b71618f31ddde2378b2bcd nanochat/optim.py, nanochat/gpt.py setup_optimizer',
                cases=[run(*c) for c in CASES], polar=[polar_case(r, c) for r, c in POLAR_CASES])

ARRAYS = ['grads', 'params', 'm', 's']

def coax(ref):
    lines = ['// Generated by psionic/tests/muon_oracle.py --write. Independent nanochat MuonAdamW values; do not edit.\n']
    for c in ref['cases']:
        p = 'MC_' + c['name'].upper()
        def arr(label, values, kind='f64'):
            text = ','.join(repr(float(x)) if kind == 'f64' else str(x) for x in values)
            lines.append(f"{p}_{label} :: [{len(values)}]{kind}.[{text}];\n")
        arr('START', c['start'])
        for label in ARRAYS:
            arr(label.upper(), [x for row in c['trace'] for x in row[label]])
        arr('ROLES', [ROLES.index(t['role']) for t in c['tensors']], 'int')
        arr('SHAPES', [x for t in c['tensors'] for x in (t['rank'], t['rows'], t['cols'])], 'int')
        arr('PLACES', [t['offset'] for t in c['tensors']], 'int')
        arr('GROUPS', [x for g in c['groups'] for x in (1 if g['kind'] == 'muon' else 0, g['first'], g['count'], g['rows'], g['cols'], g['ns'])], 'int')
        arr('SETTINGS', [x for g in c['groups'] for x in (g['lr'], g['beta1'], g['beta2'], g['eps'], g['decay'], g['momentum'])])
    for k, c in enumerate(ref['polar']):
        for label in ['input', 'output']:
            lines.append(f"MC_POLAR_{label.upper()}_{k} :: [{len(c[label])}]f64.[{','.join(repr(float(x)) for x in c[label])}];\n")
    return ''.join(lines)

def main():
    a = argparse.ArgumentParser()
    a.add_argument('--write', action='store_true')
    a.add_argument('--header', action='store_true')
    args = a.parse_args()
    ref = build()
    text = json.dumps(ref, indent=1) + '\n'
    if args.write:
        D.mkdir(parents=True, exist_ok=True)
        (D / 'reference.json').write_text(text)
        (D / 'reference.coax').write_text(coax(ref))
        print('Captured', ', '.join(f"{c['name']} ({c['n']} parameters, {len(c['groups'])} groups, {c['steps']} steps)" for c in ref['cases']))
        return
    assert (D / 'reference.json').read_text() == text, 'reference.json does not reproduce'
    assert (D / 'reference.coax').read_text() == coax(ref), 'reference.coax does not reproduce'
    if args.header:
        for c in ref['cases']:
            p = 'mc_' + c['name']
            def arr(label, values, kind='double'):
                text = ','.join(repr(float(x)) if kind == 'double' else str(x) for x in values)
                print(f"static const {kind} {p}_{label}[]={{{text}}};")
            arr('start', c['start'])
            for label in ARRAYS:
                arr(label, [x for row in c['trace'] for x in row[label]])
            arr('settings', [x for g in c['groups'] for x in (g['lr'], g['beta1'], g['beta2'], g['eps'], g['decay'], g['momentum'])])
            arr('groups', [x for g in c['groups'] for x in (1 if g['kind'] == 'muon' else 0, g['first'], g['count'], g['rows'], g['cols'], g['ns'])], 'long long')
            print(f"enum {{{p}_n={c['n']},{p}_second={c['second']},{p}_steps={c['steps']},{p}_groups_count={len(c['groups'])}}};")
    else:
        print('Muon reference reproduces:', ', '.join(f"{c['name']} {c['n']}x{c['steps']} ({len(c['groups'])} groups)" for c in ref['cases']))

if __name__ == '__main__':
    main()
