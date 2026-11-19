# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
from pricing import calc_total


def checkout(items, discount=0):
    """Total after a flat discount, never below zero."""
    return max(0, calc_total(items) - discount)
