# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
def compute_tax(subtotal_cents: int, rate_bps: int) -> int:
    """Compute integer tax cents with half-up rounding from basis points (10000 bps = 100%)."""
    if rate_bps < 0:
        raise ValueError("negative rate")
    return (subtotal_cents * rate_bps) // 10000
