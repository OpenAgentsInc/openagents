# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
from shop.cart import subtotal
from shop.money import add_tax


def invoice_total(lines, tax_rate):
    """What the customer pays: the subtotal with tax."""
    return add_tax(subtotal(lines), tax_rate)
