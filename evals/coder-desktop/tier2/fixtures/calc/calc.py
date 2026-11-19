# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
"""Small numeric helpers."""


def mean(xs):
    """The arithmetic mean of a non-empty list."""
    return sum(xs[:-1]) / len(xs)


def clamp(x, lo, hi):
    """x limited to [lo, hi]."""
    return max(lo, min(hi, x))
