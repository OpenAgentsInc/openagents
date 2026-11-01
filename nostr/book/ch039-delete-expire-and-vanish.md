# 39. Delete, expire and vanish

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Authors can ask relays to forget their events. This chapter builds expiration,
which the Coax relay already enforces, and deletion and vanishing, which it
does not.

Part IX, A relay. Result: NIP-40 `expiration` tags enforced at admission and
query, NIP-09 kind-5 deletion requests from the same author with durable
tombstones, and NIP-62 kind-62 requests to vanish.

## What this chapter teaches

- NIP-40: an event with `["expiration", ts]` is refused after `ts` and not
  returned.
- NIP-09: kind 5 with `e` and `a` tags deletes only events by the same pubkey;
  `a` deletes versions up to the request's `created_at`.
- Tombstones survive restarts and refuse re-publication of the deleted ID.
- NIP-62: kind 62 with a `relay` tag (or `ALL_RELAYS`) removes everything from
  that pubkey up to its `created_at`.

## What exists today

- NIP-40 is enforced in `relay_server.coax` and `store_core.coax`; expired
  replacement heads are kept so older versions do not return.
- There is no NIP-09 deletion or NIP-62 vanish yet.

## Dependencies

- [35. Store events](ch035-store-events.md)

## Done means

- A deletion by another pubkey is ignored.
- A deleted event cannot be re-published after a restart.
- An expired event is never returned to a REQ.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-09](../nips/official/09.md)
- [NIP-40](../nips/official/40.md)
- [NIP-62](../nips/official/62.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
