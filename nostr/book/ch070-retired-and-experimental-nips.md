# 70. Retired and experimental NIPs

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Some NIPs are retired or unproven. This chapter states each one's status from
the pinned text and the stack's decision.

Part X, Every NIP. Result: NIP-26 delegation tokens, the NIP-31 `alt` tag and
NIP-BE Bluetooth LE transport, each marked `unrecommended` in the pinned text,
with what this stack reads and what it never builds.

## What this chapter teaches

- NIP-26: `["delegation", pubkey, conditions, sig]`; `unrecommended`: "adds
  unnecessary burden for little gain". Not accepted as authority.
- NIP-31: `["alt", summary]` for unknown kinds; `unrecommended` but common;
  read for display.
- NIP-BE: BLE GATT service for device-to-device sync; `unrecommended`: "only
  implemented once". Not built.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Done means

- An event with a NIP-26 delegation tag is attributed to its signer, not the
  delegator.
- `alt` text is shown for an unknown kind.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-26](../nips/official/26.md)
- [NIP-31](../nips/official/31.md)
- [NIP-BE](../nips/official/BE.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
