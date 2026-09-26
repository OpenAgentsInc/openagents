---
id: finance.sa-ccr
version: 2
kind: method
title: SA-CCR calculation conventions for multi-driver trades and workbook traceability
summary: >-
  Keep supervisory inputs, time conventions, and hedging-set mapping explicit
  when implementing SA-CCR. This is especially important for options and
  cross-currency swaps, where plausible shortcuts can change add-ons.
tags: [sa-ccr, counterparty-credit-risk, cross-currency, options]
applies_when: >-
  Building trade-level SA-CCR calculations, especially where inputs specify
  day-count conventions or trades have multiple risk drivers.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - fin-saccr-rwa-1790398269
    - fin-saccr-rwa
  cites:
    - Basel Committee on Banking Supervision, *Basel Framework*, CRE52, “The standardised approach for measuring counterparty credit risk exposures”
    - European Parliament and Council, Regulation (EU) No 575/2013, Articles 279a–279b, as amended by Regulation (EU) 2024/1623
evidence: []
---

## Details

Treat desk-specified conventions as inputs to the calculation, not as implicit universal rules. For example, if supervisory duration uses ACT/365 while a margined maturity-factor expression uses a fixed 250-business-day denominator, keep those quantities separate; do not substitute one for the other merely because both involve time.

Map a cross-currency swap using the required risk drivers: place each currency leg in its corresponding interest-rate hedging set and represent the currency exchange exposure in the FX hedging set when the prescribed mapping is multi-driver. Do not replace this with a single-driver simplification unless the applicable rules and calculation instructions call for it.

For options, use the supervisory volatility specified for the capital calculation when deriving supervisory delta. Do not substitute an implied volatility supplied for a separate valuation or daily-P&L purpose.

These implementation choices sit within the SA-CCR replacement-cost and PFE framework; confirm the applicable EU provisions and inputs for the reporting date. Sources: Basel Committee on Banking Supervision, *Basel Framework*, CRE52, “The standardised approach for measuring counterparty credit risk exposures”; European Parliament and Council, Regulation (EU) No 575/2013, Articles 279a–279b, as amended by Regulation (EU) 2024/1623.

## How to check

For every trade, retain an auditable row containing the supervisory inputs, adjusted duration, supervisory delta, maturity factor, effective notional, and hedging-set assignment. Then verify that:

- changing an implied-volatility-only input does not change supervisory delta;
- each currency leg of a multi-driver cross-currency trade contributes to its intended interest-rate hedging set;
- any principal-exchange FX exposure is represented in the FX add-on; and
- the configured day-count basis and any fixed business-day denominator are used in their respective formulas, not interchanged.
