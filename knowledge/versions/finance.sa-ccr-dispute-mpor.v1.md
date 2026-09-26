---
id: finance.sa-ccr-dispute-mpor
version: 1
kind: edge-case
title: Apply dispute-driven MPOR extensions using qualifying disputes
summary: >-
  A collateral dispute can require a longer margin period of risk than the
  ordinary minimum. Determine the extension from the applicable rule’s
  lookback, count, and duration tests rather than from a generic dispute flag.
tags: [sa-ccr, mpor, collateral, disputes]
applies_when: >-
  Calculating exposure for margined netting sets with collateral disputes
  during the regulatory lookback period.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - fin-saccr-rwa
  cites:
    - European Parliament and Council, Regulation (EU) No 575/2013, Article 285, as amended by Regulation (EU) 2024/1623
    - Basel Committee on Banking Supervision, *Basel Framework*, CRE52, “The standardised approach for measuring counterparty credit risk exposures”
evidence: []
---

## Details

Do not assume that the ordinary MPOR applies whenever a netting set is margined. Check whether disputes satisfy the applicable regulatory trigger for an increased MPOR: the relevant lookback window, qualifying dispute count, and whether disputes remained unresolved beyond the applicable period. Count and measure disputes according to the governing rule and calendar convention, then apply its prescribed extension. Keep the dispute evidence and the resulting MPOR choice in the calculation record.

Do not extend MPOR merely because a dispute exists, and do not disregard an extension trigger because disputes later resolved. The precise trigger and extension can depend on jurisdiction and rule version. Sources: European Parliament and Council, Regulation (EU) No 575/2013, Article 285, particularly the provisions on margin period of risk and disputes, as amended by Regulation (EU) 2024/1623; Basel Committee on Banking Supervision, *Basel Framework*, CRE52, “The standardised approach for measuring counterparty credit risk exposures.”

## How to check

Build a dispute audit table with at least the netting set, dispute dates, resolution date, relevant business-day count, lookback eligibility, and whether it qualifies under the applicable rule. Recompute the MPOR from that table and test that adding a non-qualifying dispute does not change it, while adding a qualifying dispute that crosses the rule’s trigger does.
