#!/usr/bin/env python3
"""Chapter 70's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/tabular.coax. Sums use math.fsum (exactly
rounded) where the library uses chapter 3's compensated accumulator, and
log loss uses a separately written softplus. Integer draws, the split, the
initialization and the shuffle are part of the specification and must match
exactly. `--header` writes the C expectations the native harness compares.
"""
import math
import sys

MASK = (1 << 64) - 1
GOLDEN = 0x9E3779B97F4A7C15
SESSION_COUNTER = 1 << 20
DEADLINE = 9.0
REQUESTS_PER_SESSION = 8
COST_PER_TOKEN = (0.0003, 0.0009)


def bits(seed, counter):
    z = (seed + GOLDEN * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return z ^ (z >> 31)


def uniform(seed, counter):
    return (bits(seed, counter) >> 11) / 2.0**53


def simulate(seed, sessions):
    """deadline-sim-v1: features [prompt tokens, queue depth, large model, headroom GiB]."""
    x, y, latency, groups = [], [], [], []
    for g in range(sessions):
        headroom = 0.5 + 7.5 * uniform(seed, SESSION_COUNTER + 2 * g)
        slowness = 0.7 + 0.6 * uniform(seed, SESSION_COUNTER + 2 * g + 1)
        for r in range(REQUESTS_PER_SESSION):
            i = g * REQUESTS_PER_SESSION + r
            prompt = 64 + bits(seed, 4 * i) % 8129
            queue = bits(seed, 4 * i + 1) % 16
            large = bits(seed, 4 * i + 2) % 2
            noise = uniform(seed, 4 * i + 3) - 0.5
            seconds = (0.2 + 0.15 * queue
                       + prompt * COST_PER_TOKEN[large] * slowness * (1 + 1.5 / headroom)
                       + 0.6 * noise)
            x.append([float(prompt), float(queue), float(large), headroom])
            latency.append(seconds)
            y.append(1.0 if seconds <= DEADLINE else 0.0)
            groups.append(g)
    return x, y, latency, groups


def part(group):
    k = group % 5
    return 0 if k < 3 else (1 if k == 3 else 2)


def rows_of(groups, which):
    return [i for i, g in enumerate(groups) if part(g) == which]


def fit_scale(x, rows):
    d = len(x[0])
    mean = [math.fsum(x[i][j] for i in rows) / len(rows) for j in range(d)]
    sd = [math.sqrt(math.fsum((x[i][j] - mean[j]) ** 2 for i in rows) / len(rows)) for j in range(d)]
    return mean, sd


def apply_scale(x, mean, sd):
    return [[(v[j] - mean[j]) / sd[j] for j in range(len(v))] for v in x]


def sigmoid(z):
    if z >= 0:
        return 1.0 / (1.0 + math.exp(-z))
    e = math.exp(z)
    return e / (1.0 + e)


def softplus(z):
    return max(z, 0.0) + math.log1p(math.exp(-abs(z)))


def count(d, hidden):
    return d + 1 if hidden == 0 else hidden * d + 2 * hidden + 1


def initial(d, hidden, seed):
    """Weights draw U(-1/sqrt(fan_in), 1/sqrt(fan_in)) at counter = parameter slot; biases start at zero."""
    p = [0.0] * count(d, hidden)
    if hidden == 0:
        return p
    for k in range(hidden):
        for j in range(d):
            s = k * d + j
            p[s] = (2 * uniform(seed, s) - 1) / math.sqrt(d)
    base = hidden * d + hidden
    for k in range(hidden):
        p[base + k] = (2 * uniform(seed, base + k) - 1) / math.sqrt(hidden)
    return p


def forward(p, d, hidden, v):
    if hidden == 0:
        return math.fsum([p[d]] + [p[j] * v[j] for j in range(d)]), []
    h = []
    for k in range(hidden):
        a = math.fsum([p[hidden * d + k]] + [p[k * d + j] * v[j] for j in range(d)])
        h.append(math.tanh(a))
    base = hidden * d + hidden
    return math.fsum([p[base + hidden]] + [p[base + k] * h[k] for k in range(hidden)]), h


def train(x, y, rows, d, hidden, logistic, epochs, batch, rate, decay, init_seed, shuffle_seed):
    p = initial(d, hidden, init_seed)
    n = len(p)
    order = list(rows)
    counter = 0
    weight = [True] * n
    if hidden == 0:
        weight[d] = False
    else:
        for k in range(hidden):
            weight[hidden * d + k] = False
        weight[n - 1] = False
    for _ in range(epochs):
        for i in range(len(order) - 1, 0, -1):
            j = bits(shuffle_seed, counter) % (i + 1)
            counter += 1
            order[i], order[j] = order[j], order[i]
        for start in range(0, len(order), batch):
            chunk = order[start:start + batch]
            terms = [[] for _ in range(n)]
            for i in chunk:
                z, h = forward(p, d, hidden, x[i])
                dz = (sigmoid(z) if logistic else z) - y[i]
                if hidden == 0:
                    for j in range(d):
                        terms[j].append(dz * x[i][j])
                    terms[d].append(dz)
                else:
                    base = hidden * d + hidden
                    terms[n - 1].append(dz)
                    for k in range(hidden):
                        terms[base + k].append(dz * h[k])
                        dh = dz * p[base + k] * (1 - h[k] * h[k])
                        terms[hidden * d + k].append(dh)
                        for j in range(d):
                            terms[k * d + j].append(dh * x[i][j])
            m = len(chunk)
            q = []
            for s in range(n):
                g = math.fsum(terms[s]) / m
                if weight[s]:
                    g += decay * p[s]
                q.append(p[s] - rate * g)
            p = q
    return p


def outputs(p, x, d, hidden):
    return [forward(p, d, hidden, v)[0] for v in x]


def log_loss(z, y, rows):
    return math.fsum(softplus(z[i]) - y[i] * z[i] for i in rows) / len(rows)


def mse(z, target, rows):
    return math.fsum((z[i] - target[i]) ** 2 for i in rows) / len(rows)


def classify(on_time, y, rows, threshold, miss, alarm):
    """on_time: probabilities (or 0/1 decisions). A request is flagged late when on_time < threshold."""
    correct = cost = late = caught = 0
    for i in rows:
        flagged = on_time[i] < threshold
        if flagged == (y[i] == 0.0):
            correct += 1
        if y[i] == 0.0:
            late += 1
            if flagged:
                caught += 1
            else:
                cost += miss
        elif flagged:
            cost += alarm
    return correct / len(rows), cost / len(rows), late, caught


def choose(on_time, y, rows, miss, alarm):
    best = None
    for k in range(1, 20):
        t = k / 20
        c = classify(on_time, y, rows, t, miss, alarm)[1]
        if best is None or c < best[1]:
            best = (t, c)
    return best


def rule(x):
    """The handwritten rule: known per-token cost and queue cost, a safety factor of two, no memory term."""
    out = []
    for p, q, large, _ in x:
        estimate = 0.2 + 0.15 * q + p * COST_PER_TOKEN[int(large)]
        out.append(1.0 if 2 * estimate <= DEADLINE else 0.0)
    return out


def experiment(seed):
    """The chapter's complete measurement for one simulator seed."""
    x, y, latency, groups = simulate(seed, 300)
    tr, va, te = rows_of(groups, 0), rows_of(groups, 1), rows_of(groups, 2)
    mean, sd = fit_scale(x, tr)
    xs = apply_scale(x, mean, sd)
    r = {"seed": seed, "rows": len(x), "on_time": sum(y), "train": len(tr), "val": len(va), "test": len(te)}
    base = math.fsum(y[i] for i in tr) / len(tr)
    zc = [math.log(base / (1 - base))] * len(x)
    rv = rule(x)
    lin = train(xs, latency, tr, 4, 0, False, 40, 16, 0.05, 0.0, 1, 2)
    zl = outputs(lin, xs, 4, 0)
    lin_on = [1.0 if v <= DEADLINE else 0.0 for v in zl]
    mean_latency = math.fsum(latency[i] for i in tr) / len(tr)
    lg = train(xs, y, tr, 4, 0, True, 40, 16, 0.5, 0.0, 1, 2)
    zg = outputs(lg, xs, 4, 0)
    ml = train(xs, y, tr, 4, 8, True, 40, 16, 0.5, 0.0, 1, 2)
    zm = outputs(ml, xs, 4, 8)
    raw = []
    for rate in (0.5, 0.001, 0.000001):
        pr = train(x, y, tr, 4, 0, True, 40, 16, rate, 0.0, 1, 2)
        raw.append(log_loss(outputs(pr, x, 4, 0), y, va))
    r["raw"] = raw
    r["scaled_val"] = log_loss(zg, y, va)
    r["linear_params"] = lin
    r["logistic_params"] = lg
    r["mlp_params"] = ml
    r["mse"] = (mse([mean_latency] * len(x), latency, te), mse(zl, latency, te))
    pg = [sigmoid(v) for v in zg]
    pm = [sigmoid(v) for v in zm]
    pc = [sigmoid(v) for v in zc]
    r["log_loss"] = (log_loss(zc, y, te), log_loss(zg, y, te), log_loss(zm, y, te))
    r["classify"] = [classify(v, y, te, 0.5, 5, 1) for v in (pc, rv, lin_on, pg, pm)]
    tg, tm = choose(pg, y, va, 5, 1), choose(pm, y, va, 5, 1)
    r["choose"] = (tg, tm)
    r["chosen"] = (classify(pg, y, te, tg[0], 5, 1), classify(pm, y, te, tm[0], 5, 1))
    small = [i for i in tr if groups[i] < 40]
    ms, ss = fit_scale(x, small)
    xq = apply_scale(x, ms, ss)
    a = train(xq, y, small, 4, 0, True, 200, 16, 0.5, 0.0, 1, 2)
    b = train(xq, y, small, 4, 8, True, 200, 16, 0.5, 0.0, 1, 2)
    c = train(xq, y, small, 4, 8, True, 200, 16, 0.5, 0.01, 1, 2)
    r["small_rows"] = len(small)
    r["small"] = tuple(log_loss(outputs(p, xq, 4, h), y, te) for p, h in ((a, 0), (b, 8), (c, 8)))
    r["x"], r["y"], r["latency"], r["groups"] = x, y, latency, groups
    return r


def show(r):
    print(f"seed {r['seed']}: {r['rows']} requests, {int(r['on_time'])} on time; train/val/test {r['train']}/{r['val']}/{r['test']}")
    print("  raw validation log loss at rates 0.5, 1e-3, 1e-6: " + ", ".join(f"{v:.4f}" for v in r["raw"]) + f"; scaled {r['scaled_val']:.4f}")
    print(f"  latency MSE: mean {r['mse'][0]:.4f}, line {r['mse'][1]:.4f}")
    print("  test log loss constant/logistic/mlp: " + ", ".join(f"{v:.4f}" for v in r["log_loss"]))
    for name, c in zip(("constant", "rule", "line", "logistic", "mlp"), r["classify"]):
        print(f"  {name:9s} accuracy {c[0]:.4f} cost {c[1]:.4f} caught {c[3]}/{c[2]}")
    print(f"  chosen thresholds {r['choose'][0][0]}, {r['choose'][1][0]}: test cost {r['chosen'][0][1]:.4f}, {r['chosen'][1][1]:.4f}")
    print(f"  small ({r['small_rows']} rows) test log loss logistic/mlp/mlp+decay: " + ", ".join(f"{v:.4f}" for v in r["small"]))


def c_double(v):
    return repr(float(v))


def header(r, seeds):
    out = ["/* Generated by psionic/tests/tabular_oracle.py --header. Do not edit. */",
           "#include <stdint.h>"]
    x, y, latency, groups = r["x"], r["y"], r["latency"], r["groups"]
    out.append(f"#define TB_ORACLE_ROWS {len(x)}")
    out.append("static const double tb_oracle_x[] = {" + ",".join(c_double(v) for row in x for v in row) + "};")
    out.append("static const double tb_oracle_y[] = {" + ",".join(c_double(v) for v in y) + "};")
    out.append("static const double tb_oracle_latency[] = {" + ",".join(c_double(v) for v in latency) + "};")
    out.append("static const int64_t tb_oracle_groups[] = {" + ",".join(str(v) for v in groups) + "};")
    for name in ("linear_params", "logistic_params", "mlp_params"):
        out.append(f"static const double tb_oracle_{name}[] = {{" + ",".join(c_double(v) for v in r[name]) + "};")
    out.append("static const double tb_oracle_raw[] = {" + ",".join(c_double(v) for v in r["raw"]) + "};")
    out.append(f"static const double tb_oracle_scaled_val = {c_double(r['scaled_val'])};")
    out.append("static const double tb_oracle_mse[] = {" + ",".join(c_double(v) for v in r["mse"]) + "};")
    out.append("static const double tb_oracle_log_loss[] = {" + ",".join(c_double(v) for v in r["log_loss"]) + "};")
    out.append("static const double tb_oracle_classify[][4] = {" + ",".join(
        "{" + ",".join(c_double(v) for v in c) + "}" for c in r["classify"]) + "};")
    out.append("static const double tb_oracle_choose[][2] = {" + ",".join(
        "{" + ",".join(c_double(v) for v in c) + "}" for c in r["choose"]) + "};")
    out.append("static const double tb_oracle_chosen[][4] = {" + ",".join(
        "{" + ",".join(c_double(v) for v in c) + "}" for c in r["chosen"]) + "};")
    out.append(f"static const int64_t tb_oracle_small_rows = {r['small_rows']};")
    out.append("static const double tb_oracle_small[] = {" + ",".join(c_double(v) for v in r["small"]) + "};")
    out.append("static const uint64_t tb_oracle_seeds[] = {" + ",".join(str(s["seed"]) for s in seeds) + "};")
    out.append("static const double tb_oracle_seed_small[][3] = {" + ",".join(
        "{" + ",".join(c_double(v) for v in s["small"]) + "}" for s in seeds) + "};")
    out.append("static const double tb_oracle_seed_log_loss[][3] = {" + ",".join(
        "{" + ",".join(c_double(v) for v in s["log_loss"]) + "}" for s in seeds) + "};")
    print("\n".join(out))


if __name__ == "__main__":
    main = experiment(70)
    if "--header" in sys.argv:
        header(main, [experiment(s) for s in (70, 72, 74, 76, 78)])
    else:
        show(main)
        for s in (72, 74, 76, 78):
            show(experiment(s))
