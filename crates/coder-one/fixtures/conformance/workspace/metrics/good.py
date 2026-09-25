"""Standard implementations of the registry's methods, in plain Python.

The conformance tests run every registry entry against these and expect
every property to pass.
"""

import math


def cosine_similarity(a, b):
    dot = sum(x * y for x, y in zip(a, b))
    return dot / (math.sqrt(sum(x * x for x in a)) * math.sqrt(sum(y * y for y in b)))


def cosine_distance(a, b):
    return 1.0 - cosine_similarity(a, b)


def l2_normalize(v):
    norm = math.sqrt(sum(x * x for x in v))
    if norm == 0:
        return [0.0 for _ in v]
    return [x / norm for x in v]


def euclidean(a, b):
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def rbf(x, y, gamma=1.0):
    return math.exp(-gamma * sum((p - q) ** 2 for p, q in zip(x, y)))


def mmd2_unbiased(xs, ys, gamma=1.0):
    m, n = len(xs), len(ys)
    kxx = sum(rbf(xs[i], xs[j], gamma) for i in range(m) for j in range(m) if i != j) / (m * (m - 1))
    kyy = sum(rbf(ys[i], ys[j], gamma) for i in range(n) for j in range(n) if i != j) / (n * (n - 1))
    kxy = sum(rbf(x, y, gamma) for x in xs for y in ys) / (m * n)
    return kxx + kyy - 2 * kxy


def ks_statistic(a, b):
    a, b = sorted(a), sorted(b)
    values = sorted(set(a) | set(b))
    best = 0.0
    for v in values:
        fa = sum(1 for x in a if x <= v) / len(a)
        fb = sum(1 for x in b if x <= v) / len(b)
        best = max(best, abs(fa - fb))
    return best


def psi(expected, actual):
    return sum((a - e) * math.log(a / e) for e, a in zip(expected, actual))


def debounce(samples, n=3):
    state = False
    run = 0
    out = []
    for s in samples:
        if bool(s) != state:
            run += 1
            if run >= n:
                state = bool(s)
                run = 0
        else:
            run = 0
        out.append(state)
    return out


class Debouncer:
    def __init__(self, n=3):
        self.n = n
        self.state = False
        self.run = 0

    def update(self, sample):
        if bool(sample) != self.state:
            self.run += 1
            if self.run >= self.n:
                self.state = bool(sample)
                self.run = 0
        else:
            self.run = 0
        return self.state


def softmax(z):
    top = max(z)
    e = [math.exp(x - top) for x in z]
    s = sum(e)
    return [x / s for x in e]


def levenshtein(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def pearson(x, y):
    mx, my = sum(x) / len(x), sum(y) / len(y)
    dx = [v - mx for v in x]
    dy = [v - my for v in y]
    return sum(p * q for p, q in zip(dx, dy)) / math.sqrt(sum(p * p for p in dx) * sum(q * q for q in dy))
