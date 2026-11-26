# Firm reason: 5 (subagent-bench eval fixture/verifier)
from dataclasses import dataclass
from typing import List


@dataclass
class LineItem:
    sku: str
    amount_cents: int
    taxable: bool = True


def allocate_discount(items: List[LineItem], discount_cents: int) -> List[int]:
    """Proportionally allocate discount_cents across items so the allocations sum
    EXACTLY to min(discount_cents, total_cents) using largest-remainder method
    (tie-break by earlier index)."""
    if discount_cents < 0:
        raise ValueError("discount must be non-negative")
    total = sum(it.amount_cents for it in items)
    if total <= 0 or discount_cents == 0:
        return [0 for _ in items]
    cap = min(discount_cents, total)
    # BUG 1: truncating floor division loses remainder cents!
    return [(it.amount_cents * cap) // total for it in items]


class InvoiceLedger:
    def __init__(self, items: List[LineItem], discount_cents: int, tax_bps: int):
        self.items = list(items)
        self.discount_cents = discount_cents
        self.tax_bps = tax_bps
        self.refunds: List[int] = []
        allocs = allocate_discount(self.items, discount_cents)
        self.net_lines = [it.amount_cents - d for it, d in zip(self.items, allocs)]
        self.subtotal_cents = sum(self.net_lines)
        taxable_base = sum(net for it, net in zip(self.items, self.net_lines) if it.taxable)
        # Half-up integer rounding on basis points (10000 bps = 100%)
        self.tax_cents = (taxable_base * tax_bps + 5000) // 10000
        self.total_cents = self.subtotal_cents + self.tax_cents

    @property
    def refunded_cents(self) -> int:
        return sum(self.refunds)

    @property
    def remaining_refundable_cents(self) -> int:
        return self.total_cents - self.refunded_cents

    def record_refund(self, amount_cents: int) -> int:
        if amount_cents <= 0:
            raise ValueError("refund must be positive")
        # BUG 2: compares single refund against total_cents instead of remaining_refundable_cents!
        if amount_cents > self.total_cents:
            raise ValueError("refund exceeds invoice total")
        self.refunds.append(amount_cents)
        return self.remaining_refundable_cents
