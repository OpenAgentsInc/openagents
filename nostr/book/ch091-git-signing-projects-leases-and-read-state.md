# 91. Git signing, projects, leases and read state

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Block's developer NIPs sign Git objects, group repositories, wake executors
and sync read state. This chapter builds their projections and checks their
published vectors.

Part X, Every NIP. Result: NIP-GS Git object signing with Nostr keys and its
test vectors, NIP-MP multi-repository projects (30621), NIP-PL push leases
(30350, 30390) and NIP-RS cross-device read state.

## What this chapter teaches

- GS: how Git calls a signing program, the signature format and the test
  vectors.
- MP: project events across repositories and the fold rules.
- PL: lease events, executor discovery, tenant isolation and wake delivery.
- RS: read-state records and their test vectors.

## What exists today

- `coax/lib/nostr/adapters/{gs,mp,pl,rs}-projection.coax` map these events to
  their proto projections.
- Records:
  [2026-10-31-nostr-block-gs.md](../../docs/verification/2026-10-31-nostr-block-gs.md),
  [2026-10-29-nostr-block-mp.md](../../docs/verification/2026-10-29-nostr-block-mp.md),
  [2026-10-31-nostr-block-pl-rs.md](../../docs/verification/2026-10-31-nostr-block-pl-rs.md).

## Dependencies

- [63. Git on Nostr](ch063-git-on-nostr.md)
- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)

## Done means

- NIP-GS and NIP-RS test vectors pass.
- The NIP-MP fixtures in `nostr/nips/block/NIP-MP.fixtures.json` fold to their
  stated results.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [Block NIP-GS](../nips/block/NIP-GS.md)
- [Block NIP-MP](../nips/block/NIP-MP.md)
- [Block NIP-PL](../nips/block/NIP-PL.md)
- [Block NIP-RS](../nips/block/NIP-RS.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
