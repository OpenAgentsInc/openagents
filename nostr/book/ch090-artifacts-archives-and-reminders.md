# 90. Artifacts, archives and reminders

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Channels carry files, archives and reminders. This chapter covers the three
NIPs.

Part X, Every NIP. Result: NIP-AR channel artifacts (45010) with
compare-and-swap revisions, NIP-IA identity archival with consent, and NIP-ER
event reminders (30300).

## What this chapter teaches

- AR: identity and edits, access, moving and deleting, current state.
- IA: request authorization and relay processing; archival consent is not
  revocation.
- ER: reminder content, state and relay behavior.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [89. Channels](ch089-channels.md)

## Done means

- A revision against a stale version refuses.
- A reminder is delivered once at its time.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [Block NIP-AR](../nips/block/NIP-AR.md)
- [Block NIP-IA](../nips/block/NIP-IA.md)
- [Block NIP-ER](../nips/block/NIP-ER.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
