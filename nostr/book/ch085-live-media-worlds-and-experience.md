# 85. Live media, worlds and experience

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

The Verse uses Nostr for presence in 3D worlds and for experience points. This
chapter covers those and the live-media session NIP.

Part X, Every NIP. Result: NIP-LIVE voice, camera and screen sessions, NIP-MV
3D-world presence (world 33300, entity 33301, pose frames 23300), and NIP-XP
quests (30193), awards (3193) and revocations (3194).

## What this chapter teaches

- LIVE: session plan and consent, participants, lifecycle, captures and
  transcripts.
- MV: world identifiers, coordinates, cells, entity state and pose frames.
- XP: quests, acceptance rules, awards, revocations and deriving a ledger.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [61. Live activities](ch061-live-activities.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Done means

- A pose frame outside its world's bounds refuses.
- A revoked award is removed from the derived ledger.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-LIVE](../nips/openagents/NIP-LIVE.md)
- [OpenAgents NIP-MV](../nips/openagents/NIP-MV.md)
- [OpenAgents NIP-XP](../nips/openagents/NIP-XP.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
