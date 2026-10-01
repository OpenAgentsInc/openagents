"""Order totals."""

FIELDS = {"sku": str, "quantity": int, "unit_price": float}


def line_total(row):
    """The total for one order line, in dollars."""
    price = row["unit_price"]
    return price * row["qty"]
