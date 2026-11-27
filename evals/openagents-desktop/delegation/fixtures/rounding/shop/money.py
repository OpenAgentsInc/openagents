# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
def round_cents(amount):
    """The amount rounded to the cent."""
    return round(amount, 2)


def add_tax(amount, rate):
    """The amount with tax at rate (0.07 is 7%), rounded to the cent."""
    return round_cents(amount * (1 + rate))
