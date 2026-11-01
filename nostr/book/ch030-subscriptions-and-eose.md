# 30. Subscriptions and EOSE

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A subscription returns stored events, then `EOSE`, then live events. This
chapter builds subscription bookkeeping and the rules for what `EOSE` does and
does not mean.

Part VIII, A client. Result: Subscription IDs that name a slot and its serial,
EOSE as the end of stored events and never a completeness claim, NIP-67
completeness hints, and paginated catch-up.

## What this chapter teaches

- IDs like `s3.17`: frames for a closed or old subscription are unknown.
- `EOSE` ends the stored phase; short pages and tied timestamps mean more may
  exist.
- NIP-67: the relay may say whether its stored results were complete; the
  client treats a missing hint as unknown.
- Paginating backward with `until` and handling `created_at` ties.

## What exists today

- `coax/lib/nostr/relay.coax` and `relay_read.coax`; `message.coax` accepts
  bounded trailing `EOSE` values.
- Record:
  [2026-10-30-relay-read.md](../../docs/verification/2026-10-30-relay-read.md).

## Dependencies

- [29. A relay connection](ch029-a-relay-connection.md)
- [23. Filters](ch023-filters.md)

## Done means

- A capped relay (37 events per page) still yields the full set by pagination.
- A missing NIP-67 hint is never shown as complete.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- [NIP-67](../nips/official/67.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
