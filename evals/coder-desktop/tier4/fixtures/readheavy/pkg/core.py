# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
from pkg.m3 import f3
from pkg.m5 import f5


def pipeline(x):
    """f5 after f3, then doubled."""
    return f5(f3(x)) * 3
