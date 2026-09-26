---
id: numerics.decimal-currency-rounding
version: 1
kind: method
title: Compute money with decimals, the stated rounding mode, and the stated rounding step
summary: >-
  Binary floats cannot represent most cent amounts, and Python's round() uses
  round-half-to-even. Parse amounts as Decimal from strings, quantize to the
  currency's minor unit with the rounding mode the rules specify, and round at
  the step the rules name (per line, per item, or on the total).
tags: [money, decimal, rounding, finance, claims, tax, compliance]
applies_when: >-
  Calculating payouts, reimbursements, invoices, taxes, duties, or any
  monetary figure that is compared to an exact expected value.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Python Software Foundation, decimal: Decimal fixed point and floating point arithmetic (quantize, ROUND_HALF_UP, ROUND_HALF_EVEN, contexts)"
    - "Python Software Foundation, Built-in Functions: round (rounds half to even)"
    - "ISO 4217, Codes for the representation of currencies (minor units)"
    - "David Goldberg, What Every Computer Scientist Should Know About Floating-Point Arithmetic, ACM Computing Surveys 23(1), 1991"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

- **Representation.** `0.1 + 0.2 != 0.3` in binary floating point, and
  `round(2.675, 2)` gives `2.67` because the float is slightly below 2.675.
  Build `Decimal("2.675")` from the source string (not from a float), or
  work in integer minor units.
- **Rounding mode.** `round()` on Python numbers rounds halves to even
  (`round(0.5) == 0`, `round(2.5) == 2`). Commercial and many regulatory
  rules mean half away from zero: `d.quantize(Decimal("0.01"),
  rounding=ROUND_HALF_UP)`. Use `ROUND_HALF_EVEN` only when the rules say
  banker's rounding. Some rules truncate (`ROUND_DOWN`) or always round in
  the payer's favor; read the rule's wording for each figure.
- **Rounding step.** Rounding each line then summing differs from summing
  then rounding by up to half a cent per line. Follow the stated order; when
  totals must equal the sum of rounded lines, round lines first and sum
  them. When an amount must be split exactly (installments, pro rata
  shares), allocate the rounding remainder deterministically (largest
  remainder, or to the last share) so the parts add up.
- **Minor units.** Not every currency has two decimals (JPY has 0, several
  have 3); use ISO 4217 minor units when currencies vary.
- **Percentages and caps.** Apply rates, deductibles, co-payments, caps, and
  floors in the order the rules list; a cap applied before versus after a
  percentage gives different numbers.
- **Output.** Format with fixed decimals (`f"{d:.2f}"` on a quantized
  Decimal) and no thousands separators unless the output format asks for
  them; keep a negative sign convention consistent.

## How to check

Recompute a few records by hand from the written rules, including one amount
ending exactly in a half cent, one negative adjustment, and one that hits a
cap or floor. Check that totals equal the sum of their lines exactly, and
that no value in the output was produced from a float.
