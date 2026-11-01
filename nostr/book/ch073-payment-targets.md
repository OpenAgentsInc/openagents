# 73. Payment targets

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

NIP-A3 lets a profile list where to pay it. This chapter reads the list and
acts only on Bitcoin targets.

Part X, Every NIP. Result: NIP-A3 kind 10133 `payto` tags, with only the
Bitcoin and Lightning target types acted on under the repository rule.

## What this chapter teaches

- `["payto", type, authority, ...]` following RFC 8905's `payto:` URIs.
- Types `bitcoin` and `lightning` are acted on; other types are shown as
  unsupported.
- The list is a statement by its signer; the client confirms an address before
  paying.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- Every example in the pinned text parses.
- A non-Bitcoin target is never offered as a payment option.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-A3](../nips/official/A3.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
