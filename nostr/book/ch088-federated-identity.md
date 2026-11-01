# 88. Federated identity

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Federated identity lets a community accept identities from an outside issuer.
This chapter builds the verifier with an explicit algorithm allowlist.

Part X, Every NIP. Result: NIP-FI stateless assertions with bounded compact
JWS, issuer, audience, community and token-class binding, and NIP-FA browser
login, code exchange and relay assertion.

## What this chapter teaches

- FI: assertion contract, client-attached transport, verification and session
  policy.
- The selected profile `fi-rs256-policy-v1`: RS256 only, pinned IANA address
  refusal, bounded keys.
- JWKS retrieval, rotation, stale-cache and revocation behavior.
- FA: origin, session and code exchange as a credential-bearing flow.

## What exists today

- `coax/lib/nostr/block/fi-*.coax` and `coax/lib/crypto/rsa_verify.coax`
  implement the candidate profile; the authority closure kills 782/782
  mutants. Authenticated provider effects and NIP-42 pairing remain
  ([2026-10-28-fi-rs256-policy-v1.md](../../docs/protocol/2026-10-28-fi-rs256-policy-v1.md)).

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Done means

- `none`, symmetric and confused algorithms refuse.
- A JWS with a wrong audience or community refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [Block NIP-FI](../nips/block/NIP-FI.md)
- [Block NIP-FA](../nips/block/NIP-FA.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
