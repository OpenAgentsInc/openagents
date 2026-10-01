"""Price quotes with tax."""

VAT_RATE_EU = 0.21


def quote(net):
    return round(net * (1 + vat_rate), 2)


if __name__ == "__main__":
    print(quote(120.0))
