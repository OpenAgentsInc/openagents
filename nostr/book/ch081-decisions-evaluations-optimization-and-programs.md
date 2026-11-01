# 81. Decisions, evaluations, optimization and programs

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Decision models answer typed questions; evaluations measure them; optimization
searches for better ones; programs compose them. This chapter covers the four
NIPs.

Part X, Every NIP. Result: NIP-DEC decision requests and answers (25910),
NIP-EVAL suites and reports, NIP-OPT optimization studies, and NIP-PRG typed
programs (30182, 30183).

## What this chapter teaches

- DEC: entry types, requests, answers, status and cancel, HTTP-gateway
  equivalence.
- EVAL: suite identity, reports, comparisons and unknowns.
- OPT: semantic signatures, immutable implementations, study plans and trials.
- PRG: definitions, steps, binding language, Wasm modules.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md)

## Done means

- A decision answer that does not match its request's question set refuses.
- An evaluation report with an unknown suite digest is shown as unverifiable.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-DEC](../nips/openagents/NIP-DEC.md)
- [OpenAgents NIP-EVAL](../nips/openagents/NIP-EVAL.md)
- [OpenAgents NIP-OPT](../nips/openagents/NIP-OPT.md)
- [OpenAgents NIP-PRG](../nips/openagents/NIP-PRG.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
