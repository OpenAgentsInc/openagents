# 83. Attested work and sealed data

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Some work must run where the operator cannot read it. This chapter covers
attestation records and sealed data.

Part X, Every NIP. Result: NIP-ATT attested workload releases (3202), release
heads (30202), rebuilds (3203) and endpoints, and NIP-VAULT sealed personal
data with wraps and key slots.

## What this chapter teaches

- ATT: attestation levels, releases, independent rebuilds and attested
  endpoints.
- VAULT: vault objects, wraps, system key certificates and key slots.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [82. Pylons, environments and
  deploys](ch082-pylons-environments-and-deploys.md)

## Done means

- An endpoint whose attestation does not match a release head is refused.
- A vault object opens only with a valid key slot.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-ATT](../nips/openagents/NIP-ATT.md)
- [OpenAgents NIP-VAULT](../nips/openagents/NIP-VAULT.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
