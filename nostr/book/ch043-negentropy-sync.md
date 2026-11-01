# 43. Negentropy sync

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Two parties with large sets can find their differences without sending every
ID. This chapter builds Negentropy V1 as the protocol appendix in NIP-77
specifies it.

Part IX, A relay. Result: NIP-77's `NEG-OPEN`, `NEG-MSG`, `NEG-CLOSE` and
`NEG-ERR`, and Negentropy protocol V1: range-based set reconciliation over
`(created_at, id)` with fingerprints.

## What this chapter teaches

- Items sorted by `created_at`, then ID.
- Ranges with bounds, modes `Skip`, `Fingerprint` and `IdList`.
- The fingerprint: SHA-256 of the sum of IDs mod 2^256 and the count,
  truncated to 16 bytes.
- Varints and the frame size limit.
- Use in replication between Coax relays.

## What exists today

- Nothing in Coax. The production relay advertises NIP-77.

## Dependencies

- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)
- [4. SHA-256](ch004-sha-256.md)

## Done means

- The reconciliation of two random sets finds exactly the symmetric
  difference.
- Messages interoperate with an independent Negentropy implementation.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-77](../nips/official/77.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
