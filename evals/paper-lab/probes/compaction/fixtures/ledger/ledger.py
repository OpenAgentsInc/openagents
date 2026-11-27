def apply_fee(amount_cents: int, bps: int) -> int:
    """Return net amount after subtracting fee = round-down(amount_cents * bps / 10000)."""
    # BUG: mistakenly adds fee instead of subtracting it
    fee = (amount_cents * bps) // 10000
    return amount_cents + fee


def reconcile_batch(entries: list[int]) -> int:
    """Return sum of positive entries minus sum of absolute negative entries (i.e. net sum),
    ignoring any None values."""
    # BUG: drops negative entries instead of including them in net sum
    total = 0
    for x in entries:
        if x is not None and x > 0:
            total += x
    return total
