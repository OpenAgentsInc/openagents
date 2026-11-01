# 23. Filters

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A subscription asks for events with filters. A relay and a client must agree
exactly on what matches. This chapter builds filter parsing and matching,
including the single-letter tag queries that NIP-12 introduced.

Part VI, The event model. Result: NIP-01 filters: `ids`, `authors`, `kinds`,
`#<letter>` tags, `since`, `until` and `limit`; AND across fields, OR within a
list, inclusive times, and absent lists distinct from empty ones.

## What this chapter teaches

- Fields and their meaning; `limit` applies only to the initial query.
- AND across fields, OR within a list, OR across filters in one `REQ`.
- Tag filters match any value of a single-letter tag; NIP-01 says only
  single-letter tags are indexed.
- An absent list matches everything; an explicitly empty list matches nothing.
- `since <= created_at <= until`.

## What exists today

- `coax/lib/nostr/filter.coax` matches; `filter_read.coax` parses with bounds.
- Notes: `coax/tests/nostr/FILTER.md`.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- Matching agrees with an independent implementation on a generated corpus of
  events and filters.
- Over-limit filters refuse instead of truncating.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- [NIP-12](../nips/official/12.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
