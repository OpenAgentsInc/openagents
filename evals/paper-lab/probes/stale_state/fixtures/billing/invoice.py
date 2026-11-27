import billing


def compute_total(subtotal: float) -> float:
    return round(subtotal * (1.0 + billing.TAX_RATE), 2)


def tax_label() -> str:
    # BUG: hardcoded 5% instead of reading billing.TAX_RATE
    return "tax=5%"
