# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
"""Stock levels as a dict of item name -> count."""


def remove(items, name, qty):
    """Takes qty of name out of stock; raises ValueError when there is not enough."""
    have = items.get(name, 0)
    if qty > have:
        raise ValueError(f"only {have} {name} in stock")
    items[name] = have - qty
    return items
