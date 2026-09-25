def add(a, b):
    """Add two numbers."""
    return a + b


def scale(values, factor):
    """Multiply each value by factor."""
    return [value * factor for value in values]


class Ledger:
    """A running total."""

    def __init__(self):
        self.total = 0

    def post(self, amount):
        self.total = add(self.total, amount)
        return self.total
