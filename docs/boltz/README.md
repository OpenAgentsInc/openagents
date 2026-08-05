# Boltz incident and liquidity-SPOF documentation

This directory holds **incident assessment** and discourse evidence for the
2026-08 Boltz swap pause and the broader “single swap provider” failure mode.

These documents do **not** authorize operating Boltz infrastructure, deploying
a fork, publishing exploits, or claiming OpenAgents already replaces production
swap liquidity.

## Reading order

1. [`2026-08-05-boltz-ai-attack-pause-assessment.md`](2026-08-05-boltz-ai-attack-pause-assessment.md)
   — **canonical incident ledger**: claims B1–B12, timeline, operator
   statements, wallet cascade, scenarios, assessment, watchlist.
2. Machine receipt:
   [`receipts/2026-08-05-72h-discourse-sweep.json`](receipts/2026-08-05-72h-discourse-sweep.json)
   (~336 X posts in the ~72h window around the pause).

## Architecture companions (not this incident’s authority)

Product/protocol teardowns and market NIPs live elsewhere; use them for *what to
build*, not for *what happened on Aug 3*:

| Doc | Role |
| --- | --- |
| [`../teardowns/2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md`](../teardowns/2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md) | Boltz architecture + Nostr multi-provider rebuild thesis |
| [`../teardowns/2026-08-04-satora-lendaswap-outage-teardown.md`](../teardowns/2026-08-04-satora-lendaswap-outage-teardown.md) | Second coordinator outage same week |
| [`../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md`](../teardowns/2026-08-04-tbdex-liquidity-protocol-teardown.md) | Provider-neutral negotiation grammar |
| [`../nips/MKT.md`](../nips/MKT.md), [`../nips/MKT-SWP.md`](../nips/MKT-SWP.md) | Negotiated markets / swap profile drafts |
| [`../transcripts/266.md`](../transcripts/266.md) | Episode 266: SPOF + Nostr markets (machine transcript) |

## Evidence labels

- **Operator claim:** Boltz (or another named operator) said it publicly.
- **Wallet claim:** a dependent product account stated user impact/safety.
- **External observation:** press or social discourse; not independent proof.
- **Architecture note:** points into teardowns/NIPs; not incident proof.
- **Speculation:** must not be promoted without new primary evidence
  (e.g. “state-sponsored” without attribution).
