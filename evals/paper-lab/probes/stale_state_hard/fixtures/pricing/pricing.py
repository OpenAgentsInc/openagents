# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
def discounted_total(price_cents: int, pct: int) -> int:
    """Apply integer percentage discount pct (0..100) to price_cents."""
    if pct < 0 or pct > 100:
        raise ValueError("pct out of range")
    return price_cents - (price_cents * pct) // 100
