# 60. Private messages

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

NIP-17 is Nostr's private messaging: NIP-59 wraps around unsigned kind 14
rumors, delivered to each recipient's DM relays. This chapter builds it on
Part V.

Part X, Every NIP. Result: NIP-17 kind 14 chat and kind 15 file messages sent
as gift wraps, kind 10050 DM relay lists, NIP-A4 kind 24 public messages, and
the unrecommended NIP-EE MLS groups.

## What this chapter teaches

- Kind 14 rumor with `p` tags for every participant; the chat room is the set
  of pubkeys.
- One gift wrap per recipient and one for the sender.
- Kind 10050 lists the relays to deliver to; no list means do not send.
- Kind 15 file messages with `file-type`, `encryption-algorithm`,
  `decryption-key`, `x`.
- NIP-A4: kind 24 public messages, not encrypted.
- NIP-EE is `unrecommended`, superseded by the Marmot protocol; not
  implemented.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [25. Gift wrap](ch025-gift-wrap.md)
- [32. Many relays and the outbox
  model](ch032-many-relays-and-the-outbox-model.md)

## Done means

- A message from Coax opens in an independent NIP-17 client and the reverse.
- A recipient without kind 10050 receives nothing.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-17](../nips/official/17.md)
- [NIP-A4](../nips/official/A4.md)
- [NIP-EE](../nips/official/EE.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
