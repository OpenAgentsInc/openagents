# 41. Search

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

NIP-50 lets a filter carry a search string. This chapter defines what the Coax
relay searches and how it orders results.

Part IX, A relay. Result: NIP-50's `search` filter field, results ordered by
relevance, and the `key:value` extensions (`include:spam`, `domain:`,
`language:`, `sentiment:`, `nsfw:`).

## What this chapter teaches

- `search` matches content and, per the relay's choice, some tags.
- Results ordered by score, not by time; `limit` applies after scoring.
- Extensions the relay supports, and ignoring unsupported ones.
- The Rust relay's semantics as the starting point.

## What exists today

- Nothing in Coax. The production relay advertises NIP-50.

## Dependencies

- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)

## Done means

- Search results on a fixture corpus match the Rust relay's, or the difference
  is recorded.
- A filter with an unsupported extension still runs.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-50](../nips/official/50.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
