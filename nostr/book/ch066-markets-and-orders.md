# 66. Markets and orders

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Listings and orders are signed offers. This chapter reads them under the
repository's Bitcoin-only rule: amounts are shown as stated, and only Bitcoin
settlement is built.

Part X, Every NIP. Result: NIP-99 classified listings (30402 and 30403 drafts)
with `price` in a currency and frequency, NIP-69 peer-to-peer order events
(38383), and the unrecommended NIP-15 marketplace.

## What this chapter teaches

- NIP-99: `title`, `summary`, `published_at`, `location`, `price` `[amount,
  currency, frequency]`, `status`.
- NIP-69: kind 38383 with `d`, `k` `buy`/`sell`, `f` fiat code, `s` status,
  `amt`, `fa`, `pm`, `premium`, `network`, `layer`.
- NIP-15 (stalls, products, checkout) is `unrecommended`; read only.
- Settlement in this stack is Lightning or on-chain Bitcoin (chapters 71 and
  86).

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- Listings parse with every `price` form in the NIP.
- An order with `layer` other than Bitcoin, Lightning or Spark is shown but
  never settled.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-99](../nips/official/99.md)
- [NIP-69](../nips/official/69.md)
- [NIP-15](../nips/official/15.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
