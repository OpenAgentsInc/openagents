# 45. Profiles, follows and names

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A person on Nostr is a key with a profile, a follow list and optional names
others can check. This chapter reads and writes all four.

Part X, Every NIP. Result: Kind 0 profiles with NIP-24's extra fields, kind 3
follow lists, NIP-05 DNS identifiers checked through
`/.well-known/nostr.json`, and NIP-39 external identity claims in kind 10011.

## What this chapter teaches

- Kind 0 content: `name`, `about`, `picture`, plus NIP-24's `display_name`,
  `website`, `banner`, `bot`, `birthday`.
- Kind 3: `p` tags with relay hint and petname; replaceable, so the newest
  list replaces the whole set.
- NIP-05: `name@domain` resolves through
  `https://domain/.well-known/nostr.json?name=name`; redirects are not
  followed; the result only says the domain vouches for the key.
- NIP-39: kind 10011 `i` tags such as `github:user` with a proof reference.
- None of these is proof of who a person is.

## What exists today

- `coax/lib/nostr/metadata.coax` reads kind 0 `name`, `about` and `picture`.
  Kind 3, NIP-05 and NIP-39 are not implemented.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [28. Sockets, TLS and relay URLs](ch028-sockets-tls-and-relay-urls.md)

## Done means

- NIP-05 lookups refuse redirects and non-matching keys.
- A newer kind 3 replaces an older one in the store.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-02](../nips/official/02.md)
- [NIP-05](../nips/official/05.md)
- [NIP-24](../nips/official/24.md)
- [NIP-39](../nips/official/39.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
