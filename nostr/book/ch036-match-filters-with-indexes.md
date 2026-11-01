# 36. Match filters with indexes

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A relay answers each filter from its store. A scan is correct but slow. This
chapter adds indexes and proves they return exactly what a scan returns.

Part IX, A relay. Result: Answering REQ from the store newest first with query
cost limits, then indexes by ID, author, kind and tag that never change which
events match.

## What this chapter teaches

- Query order: newest `created_at` first, then lowest ID.
- Limits: 32 subscriptions, 16 filters, `max_limit` 127, query cost 100,000.
- Indexes: by ID, by `(pubkey, kind)`, by single-letter tag value, by time.
- Choosing an index per filter, and the property that an index never changes
  the result set.

## What exists today

- `coax/lib/nostr/filter.coax` and `store.coax` answer by scan; there are no
  indexes yet.

## Dependencies

- [23. Filters](ch023-filters.md)
- [35. Store events](ch035-store-events.md)

## Done means

- A `#forall` property: for generated stores and filters, indexed and scanned
  results are equal.
- The five project REQs match relay.openagents.com's recording.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
