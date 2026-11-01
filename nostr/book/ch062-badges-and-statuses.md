# 62. Badges and statuses

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Badges and statuses decorate a profile. This chapter reads both and checks
that each award comes from the badge's issuer.

Part X, Every NIP. Result: NIP-58 badge definitions (30009), awards (8) and
profile badges (10008, formerly 30008), and NIP-38 kind 30315 user statuses
with `expiration`.

## What this chapter teaches

- Kind 30009: `d`, `name`, `image`, `thumb`.
- Kind 8: `a` to the definition and `p` to awardees; only the definition's
  author can award.
- Kind 10008: ordered `a`/`e` pairs a user chooses to show.
- NIP-38: kind 30315 with `d` `general` or `music`, and NIP-40 expiration.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- An award by someone other than the definition's author is not shown.
- An expired status is not shown.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-58](../nips/official/58.md)
- [NIP-38](../nips/official/38.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
