# 74. Cashu and Fedimint are excluded

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

AGENTS.md allows Lightning, on-chain Bitcoin, Spark and Taproot Assets, and
excludes Cashu. Fedimint is not on the allowed list either. This chapter
describes the three NIPs so a reader can recognize their events, and states
that the stack does not implement them.

Part X, Every NIP. Result: NIP-60 Cashu wallets, NIP-61 nutzaps and NIP-87
mint discovery, described for reading only; the repository's Bitcoin-only rule
excludes Cashu and Fedimint from implementation.

## What this chapter teaches

- NIP-60: wallet event 17375, token events 7375, history 7376, quote 7374.
- NIP-61: nutzap info 10019 and nutzaps 9321.
- NIP-87: mint announcements 38172 (Cashu) and 38173 (Fedimint),
  recommendations 38000.
- The decision: no wallet, no token handling, no mint discovery. A client
  shows these kinds as unsupported.

## What exists today

- Nothing, by rule: AGENTS.md, "Bitcoin-only money".

## Dependencies

- None. You can read this chapter after Part I.

## Done means

- No Coax file implements these kinds, and a check fails if one appears
  without an owner decision.
- The chapter lists every kind above with its source line.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-60](../nips/official/60.md)
- [NIP-61](../nips/official/61.md)
- [NIP-87](../nips/official/87.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
