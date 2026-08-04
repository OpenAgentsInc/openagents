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

The package exports:

- generated per-kind event schemas, required-tag codecs, bounds, enums, identifiers, content-envelope schemas, and relay reason codes;
- duplicate-safe public and private record validation with an explicit profile registry;
- canonical signed-event serialization plus NIP-59 counterparty and sender-recovery wrapping on `nostr-effect`, separating cryptographically verified wrap/seal/rumor IDs from caller-supplied source provenance;
- immutable admission with prior-result replay and relay conflict reasons, idempotency keys, quote/reservation projections, per-signer status gap/fork detection, expiry, authorization, evidence, settlement, recovery, and delivery-deduplication helpers.

`pnpm check` verifies deterministic regeneration, the vendored manifest hashes, all exported Immortal client-only cases, the pinned NIP-44 vector, and the deterministic Rust/TypeScript transport round trip. The contract artifact, fixture manifest, and fixture bytes remain unformatted so their SHA-256 digests stay authoritative.

Relay acceptance proves transport acceptance only. This package does not provide
custody, settlement, execution, or payment guarantees.
