# 16. NIP-19 entities and nostr: URIs

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

People share Nostr keys and events as Bech32 strings, and NIP-21 puts them in
`nostr:` URIs. This chapter builds every NIP-19 form, including the TLV
records, and the NIP-21 rules about which forms a URI may carry.

Part IV, Encodings. Result: `npub`, `nsec` and `note`, and the TLV forms
`nprofile`, `nevent` and `naddr` with relay hints, plus the `nostr:` URI
scheme of NIP-21.

## What this chapter teaches

- Bare forms: `npub`, `nsec`, `note`, each exactly 32 bytes.
- TLV forms: type 0 special, 1 relay, 2 author, 3 kind; `nprofile`, `nevent`,
  `naddr`.
- Unknown TLV types are skipped, as NIP-19 says; lengths past the input
  refuse.
- NIP-21: `nostr:npub1…`; `nsec` is not allowed in a URI.
- Display only: a decoded entity is a pointer, not a verified event.

## What exists today

- `coax/lib/nostr/nip19-core.coax` encodes and decodes every form with bounded
  TLVs (commit `c29005554f`).
- `apps/openagents-web/project-names.coax` uses the public forms on project
  pages.

## Dependencies

- [15. Bech32](ch015-bech32.md)

## Done means

- The NIP-19 examples in `nostr/nips/official/19.md` decode to their stated
  values.
- A `nostr:nsec1…` URI refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-19](../nips/official/19.md)
- [NIP-21](../nips/official/21.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
