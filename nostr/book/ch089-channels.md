# 89. Channels

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Block's channel NIPs extend NIP-29 groups. This chapter builds their
membership, visibility and paging rules.

Part X, Every NIP. Result: NIP-CL channel identity and discovery (44100,
44101), NIP-DV DM visibility, NIP-CW channel and thread windows with composite
cursors, and NIP-WP workspace profiles.

## What this chapter teaches

- CL: identity tags, labels, finding and choosing a channel, system channels.
- DV: visibility kinds and the relay processing algorithm.
- CW: requests, top-level classification, composite timestamp and ID cursors,
  bounded rows.
- WP: workspace profile kinds and relay processing.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [59. Groups and communities](ch059-groups-and-communities.md)

## Done means

- A missing page never shows an empty channel.
- A hidden DM is not returned to a non-participant.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [Block NIP-CL](../nips/block/NIP-CL.md)
- [Block NIP-DV](../nips/block/NIP-DV.md)
- [Block NIP-CW](../nips/block/NIP-CW.md)
- [Block NIP-WP](../nips/block/NIP-WP.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
