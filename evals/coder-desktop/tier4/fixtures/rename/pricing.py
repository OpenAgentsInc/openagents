# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
def calc_total(items):
    """Sum of price times quantity."""
    return sum(i['price'] * i['qty'] for i in items)
