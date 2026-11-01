# 101. The Verse and the wallet

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

The Verse is where people meet. This chapter plans how realms use Nostr for
presence and how payments reach the wallet.

Part XII, Nostr in OpenAgents. Result: Presence in realms with NIP-MV pose
frames, Verse identities as Nostr keys, and zaps and Wallet Connect over the
Spark wallet's existing payment rules.

## What this chapter teaches

- Presence: NIP-MV frames feeding [Aethyr chapter
  49](../../aethyr/book/ch049-rooms-and-presence.md)'s rooms.
- Identity: a player's key, profile and follows from chapter 45.
- Payments: zaps and NWC requests decided by `verse/wallet.coax`'s rules;
  words and keys never enter that file.

## What exists today

- `verse/wallet.coax` decides payments but has no Nostr input yet. No realm
  reads NIP-MV events.

## Dependencies

- [85. Live media, worlds and
  experience](ch085-live-media-worlds-and-experience.md)
- [71. Lightning zaps and goals](ch071-lightning-zaps-and-goals.md)
- [72. Wallet Connect](ch072-wallet-connect.md)
- [Aethyr chapter 49, Rooms and
  presence](../../aethyr/book/ch049-rooms-and-presence.md)

## Done means

- Two players see each other's presence through a Coax relay within the
  latency budget.
- A zap request over the wallet's limits is refused by the wallet rules.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-MV](../nips/openagents/NIP-MV.md)
- [NIP-57](../nips/official/57.md)
- [NIP-47](../nips/official/47.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
