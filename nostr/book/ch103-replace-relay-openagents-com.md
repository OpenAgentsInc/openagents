# 103. Replace relay.openagents.com

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

relay.openagents.com still runs the Rust relay from before the reset. This
chapter plans phase R3 of coax/nostr.md section 17: moving its data and
traffic to the Coax relay.

Part XII, Nostr in OpenAgents. Result: Exporting the Rust relay's Postgres
read-only, importing with re-verification of every event, a shadow period
comparing both relays, and DNS cutover only with owner sign-off.

## What this chapter teaches

- Read-only export and import with every ID and signature checked again.
- Shadow writes to both relays and readers that compare.
- Event-set equality at cutover and a rehearsed rollback.
- Owner sign-off before any DNS or production change.

## What exists today

- R1 runs on the tailnet; R2 and R3 have not started. The production relay is
  unchanged.

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [35. Store events](ch035-store-events.md)
- [37. NIP-11 relay information](ch037-nip-11-relay-information.md)
- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [39. Delete, expire and vanish](ch039-delete-expire-and-vanish.md)
- [40. Count events](ch040-count-events.md)
- [41. Search](ch041-search.md)
- [42. Protected events](ch042-protected-events.md)
- [43. Negentropy sync](ch043-negentropy-sync.md)
- [44. Limits, policy and floods](ch044-limits-policy-and-floods.md)
- [94. Differential tests](ch094-differential-tests.md)

## Done means

- Event-set equality between the two relays at cutover.
- A rollback rehearsal restores service from the Rust relay.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-11](../nips/official/11.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
