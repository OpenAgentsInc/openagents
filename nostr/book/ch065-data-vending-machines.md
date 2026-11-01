# 65. Data vending machines

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

NIP-90 asked services to sell computation over Nostr. The pinned text now says
it "got totally out of control" and prefers use-case-specific microstandards.
OpenAgents' own job NIPs (chapter 80) follow that advice. This chapter
describes NIP-90 for interoperability.

Part X, Every NIP. Result: NIP-90 job requests (5000–5999), results
(6000–6999) and feedback (7000), job chaining, and why the pinned text marks
it `unrecommended`.

## What this chapter teaches

- Request: `i` inputs with type `url`, `event`, `job` or `text`, `output`,
  `param`, `bid`, `relays`, `p`.
- Result: kind = request kind + 1000, `request` tag with the original,
  `amount` with an optional bolt11.
- Feedback kind 7000 with `status` `payment-required`, `processing`, `error`,
  `success`, `partial`.
- Encrypted parameters with NIP-04 in the spec; this stack would use NIP-44
  only.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Done means

- A result whose `request` tag does not match the request's ID refuses.
- The record
  [gptoss-cloud-nip90-2026-10-30.md](../../docs/verification/gptoss-cloud-nip90-2026-10-30.md)
  is reproduced or retired.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-90](../nips/official/90.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
