# 96. Formal properties

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Some properties are worth stating exactly. This chapter writes them as checks
and, where it fits, as bounded models.

Part XI, Interop and verification. Result: Properties stated and checked with
`#forall` and models: signature validity matches BIP-340's definition, filter
matching is AND across fields and OR within lists, and indexes never change
results.

## What this chapter teaches

- Signatures: verify accepts exactly the signatures the BIP-340 algorithm
  accepts; sign then verify always succeeds.
- Filters: `match(f, e)` equals the conjunction of field predicates; merging
  filters is a union.
- Store: replacement is a function of `(created_at, id)` only.
- Indexes: query results equal a scan.

## What exists today

- Many `#forall` checks exist in the files listed in Parts II–IX; no property
  is stated across modules.

## Dependencies

- [11. Verify a Schnorr signature](ch011-verify-a-schnorr-signature.md)
- [23. Filters](ch023-filters.md)
- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)

## Done means

- Each property has a check that a deliberate bug breaks.
- Counterexamples from any model become regression tests.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
