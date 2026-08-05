# `@openagentsinc/nip-mkt`

Effect Schema contracts and transport-neutral client operations for NIP-MKT.

The `contract/` tree is vendored byte-for-byte from Immortal commit
`15e77e0c9958b2334a8471c250cf7476f4c28598`. `src/generated.ts` is derived from
that contract. Regenerate it with `pnpm generate`, and use
`pnpm check:generated` to fail when the checked-in output differs.

Production NIP-59 rumors contain one recipient `p` tag and a unique 64-hex
`d` tag. The pinned Immortal deterministic transport fixture predates the
rumor `d` requirement, so its byte-for-byte replay is constructed with the
low-level NIP-59 primitives in its conformance test. Production wrapping and
unwrapping fail closed without `d`.

All live workspace consumers pin `nostr-effect` revision
`46548d34fb1e6502763a951ee4790f37a5d838ad`. It descends from the earlier
`1314ed6ee6cc508ba9a54d03372fe1c71a984815` train through the NIP-29 relay
fixes and the strict NIP-59 verification and unwrap-provenance commits.
NIP-MKT uses those exported transport primitives; it does not maintain a
parallel NIP-44 or NIP-59 implementation. Historical implementation records
and conformance receipts retain the revision that produced them.

The package exports:

- generated per-kind event schemas, required-tag codecs, bounds, enums, identifiers, content-envelope schemas, and relay reason codes;
- duplicate-safe public and private record validation with an explicit profile registry;
- canonical signed-event serialization plus NIP-59 counterparty and sender-recovery wrapping on `nostr-effect`, separating cryptographically verified wrap/seal/rumor IDs from caller-supplied source provenance;
- immutable admission with prior-result replay and relay conflict reasons, idempotency keys, quote/reservation projections, per-signer status gap/fork detection, expiry, authorization, evidence, settlement, recovery, and delivery-deduplication helpers.
- a bounded `probeImmortalRelay` Effect that validates NIP-11 software, exact contract version, and `nip-mkt` advertisement before completing a WebSocket `REQ`/`EOSE` compatibility subscription for an exact random event ID.

The probe reports all relay extensions it observes. Extensions added after the
pinned contract are capability evidence only; this package does not claim
profile-schema support merely because the relay advertises them.

`pnpm check` verifies deterministic regeneration, the vendored manifest hashes, all exported Immortal client-only cases, the pinned NIP-44 vector, and the deterministic Rust/TypeScript transport round trip. The contract artifact, fixture manifest, and fixture bytes remain unformatted so their SHA-256 digests stay authoritative.

Relay acceptance proves transport acceptance only. This package does not provide
custody, settlement, execution, or payment guarantees.
