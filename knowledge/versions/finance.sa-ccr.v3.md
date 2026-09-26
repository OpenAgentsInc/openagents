---
id: finance.sa-ccr
version: 3
kind: method
title: The Basel standardised approach to counterparty credit risk (SA-CCR)
summary: >-
  Build SA-CCR in stages: trade-level supervisory inputs, hedging-set add-ons,
  netting-set collateral and PFE, then EAD and a separate risk-weight
  calculation. Use the applicable CRR version and preserve the prescribed
  mapping of multi-driver trades.
tags: [sa-ccr, crr3, counterparty-credit-risk, hedging-sets]
applies_when: >-
  Implementing or reviewing SA-CCR exposure calculations, especially where a
  trade contributes to more than one risk driver or EU CRR rules are required.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - fin-saccr-rwa-1790398269
    - fin-saccr-rwa
  cites:
    - European Parliament and Council, Regulation (EU) No 575/2013 as amended by Regulation (EU) 2024/1623, Articles 274–280
    - Basel Committee on Banking Supervision, Basel Framework, CRE52, The standardised approach for counterparty credit risk
evidence: []
---

## Details

Implement the calculation as separate, auditable stages rather than treating each netting set as a single portfolio notional:

1. Map each trade to its asset class, supervisory factor, correlation, supervisory option volatility and supervisory delta. Calculate adjusted notional and maturity factor using the applicable rule and documented day-count conventions.
2. Calculate effective notional and aggregate within the prescribed hedging sets. For a multi-driver cross-currency swap, map each currency leg to its own interest-rate hedging set and include the FX principal-exchange leg as an FX exposure; do not replace this with a single-driver simplification where the applicable framework requires multi-driver treatment.
3. Calculate replacement cost and the PFE multiplier using the netting-set collateral and margin terms. Aggregate the asset-class add-ons, apply the multiplier, and calculate EAD under the applicable SA-CCR formula.
4. Apply the relevant counterparty risk weight to EAD as a distinct step to obtain RWA; calculate capital from RWA using the required capital ratio. Do not feed the risk weight into the exposure/add-on calculation.

Keep desk conventions distinct from regulatory constants. For example, use a prescribed ACT/365 convention for duration if the applicable implementation specifies it, while retaining the fixed 250-business-day denominator in the margined maturity-factor formula rather than substituting a day-count calculation.

Sources: European Parliament and Council, Regulation (EU) No 575/2013, as amended by Regulation (EU) 2024/1623 (CRR3), Articles 274–280; Basel Committee on Banking Supervision, *Basel Framework*, CRE52, “The standardised approach for counterparty credit risk.”
