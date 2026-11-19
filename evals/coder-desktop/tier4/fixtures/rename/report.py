# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import pricing


def line(items):
    """One report line."""
    return f'total={pricing.calc_total(items)}'
