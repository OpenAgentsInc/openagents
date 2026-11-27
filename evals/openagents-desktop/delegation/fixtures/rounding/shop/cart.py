# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
from shop.money import round_cents


def line_total(price, quantity, discount=0.0):
    """One line: price times quantity, less a discount (0.5 is half off), rounded to the cent."""
    return round_cents(price * quantity * (1 - discount))


def subtotal(lines):
    """The sum of the lines' totals. lines is a list of (price, quantity, discount)."""
    return round_cents(sum(line_total(p, q, d) for p, q, d in lines))
