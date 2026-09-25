"""Common departures from the registry's methods, in plain Python.

The conformance tests expect each of these to fail the property named
beside it, and to pass the others where the departure doesn't reach.
"""

import math


def cosine_similarity(a, b):
    # Leaves out the norms: a dot product, not a cosine. Fails `known_angle`.
    return float(sum(x * y for x, y in zip(a, b)))


def cosine_distance(a, b):
    # Returns the similarity itself. Fails `orthogonal`.
    dot = sum(x * y for x, y in zip(a, b))
    return dot / (math.sqrt(sum(x * x for x in a)) * math.sqrt(sum(y * y for y in b)))


def l2_normalize(v):
    # Divides by zero on the zero vector. Fails `zero_vector`.
    norm = math.sqrt(sum(x * x for x in v))
    return [x / norm for x in v]


def euclidean(a, b):
    # The squared distance. Fails `not_squared`.
    return float(sum((x - y) ** 2 for x, y in zip(a, b)))


def rbf(x, y, gamma=1.0):
    return math.exp(-gamma * sum((p - q) ** 2 for p, q in zip(x, y)))


def mmd2_biased(xs, ys, gamma=1.0):
    # Keeps the diagonal: the biased estimate. Fails `identical_below_zero`.
    m, n = len(xs), len(ys)
    kxx = sum(rbf(a, b, gamma) for a in xs for b in xs) / (m * m)
    kyy = sum(rbf(a, b, gamma) for a in ys for b in ys) / (n * n)
    kxy = sum(rbf(a, b, gamma) for a in xs for b in ys) / (m * n)
    return kxx + kyy - 2 * kxy


def ks_statistic(a, b):
    # Compares the sorted samples position by position. Fails `half_overlap`.
    a, b = sorted(a), sorted(b)
    k = min(len(a), len(b))
    return max(abs(a[i] - b[i]) for i in range(k)) / max(max(a), max(b))


def psi(expected, actual):
    # Drops the difference factor: a KL divergence. Fails `known_two_bins`.
    return sum(a * math.log(a / e) for e, a in zip(expected, actual))


def debounce(samples, n=3):
    # Enters after n samples but leaves on the first off sample. Fails
    # `glitch_while_on`.
    state = False
    run = 0
    out = []
    for s in samples:
        if s and not state:
            run += 1
            if run >= n:
                state = True
                run = 0
        elif not s:
            state = False
            run = 0
        out.append(state)
    return out


def softmax(z):
    # No max subtraction: overflows on large scores. Fails `large_scores`.
    e = [math.exp(x) if x < 700 else float("inf") for x in z]
    s = sum(e)
    return [x / s for x in e]


def levenshtein(a, b):
    # A substitution costs 2. Fails `substitution_costs_one`.
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + 2 * (ca != cb)))
        prev = cur
    return prev[-1]


def pearson(x, y):
    # No centering: the cosine of the raw samples. Fails `known`.
    return sum(p * q for p, q in zip(x, y)) / math.sqrt(sum(p * p for p in x) * sum(q * q for q in y))
