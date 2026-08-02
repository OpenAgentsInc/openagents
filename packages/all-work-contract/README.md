# OpenAgents All Work contract

`@openagentsinc/all-work-contract` owns the encoded boundary for the first
read-only All Work composition slice. It does not own Work lifecycle policy or
replace native Effect and Rust domain models.

The reviewed source is
[`definition/all-work-v1.contract.json`](./definition/all-work-v1.contract.json).
It uses the restricted OpenAgents Contract Profile described in
[`docs/sol/2026-08-02-effect-rust-unified-contract-models-analysis.md`](../../docs/sol/2026-08-02-effect-rust-unified-contract-models-analysis.md).
The generator emits these committed artifacts:

- Effect Schema codecs and TypeScript types in `src/generated.ts`;
- Rust `serde` types and structural validators in
  `generated/rust/all_work_v1.rs`;
- canonical JSON Schema in
  `generated/json-schema/all-work-v1.schema.json`;
- positive, negative, absent/null, integer, unknown-field, and compatibility
  fixtures in `fixtures/`; and
- artifact digests, protocol methods, compatibility posture, and named
  handwritten semantic checks in `generated/compatibility.json`.

Generated code owns structure only. `src/semantic.ts` keeps cross-record rules
explicit: an Issue projection uses the same Work identity and revision, v1 is
an explicit rollback negotiation, Work reads require `omega-effectd.v2`, and
successive projections from one source cannot regress revision or change a
cursor without a revision advance. The boundary also defines typed Work-index
subscription request/event envelopes without claiming a production transport.
Neither layer grants admission, delegation, verification, acceptance, release,
settlement, or public-claim authority.

Canonical JSON uses `openagents-canonical-json-v1`: UTF-8, object keys sorted
by Unicode code point, array order retained, safe integers only, absent fields
omitted, present null encoded as `null`, and no insignificant whitespace. The
Effect and Rust conformance tests compare the same committed byte vector.
The Effect export `encodeAllWorkCanonicalJson` and Rust export
`canonical_json_bytes` enforce that encoding; the Effect encoder also enforces
the boundary byte limit.

## Generate and verify

```bash
pnpm --dir packages/all-work-contract generate
pnpm --dir packages/all-work-contract check:generated
pnpm --dir packages/all-work-contract typecheck
pnpm --dir packages/all-work-contract test
cargo test -p openagents-all-work-contract
```

`check:generated` runs offline, regenerates into a temporary directory, checks
the complete generated-file inventory, and byte-compares every artifact. A
wire change starts in the definition and produces a reviewable generated diff.

## Cross-repository consumption

Omega consumes the generated Rust file, fixtures, and compatibility manifest
as one digest-bound artifact. Omega-specific adapters map native state to these
DTOs. A copied or vendored generated file is valid only when its definition and
artifact digests match the manifest; handwritten Rust mirrors are not allowed.

The first immutable consumer is Omega commit
`6e3f67c6006b0e98eb57047971777eece2fd0f20`. It pins OpenAgents commit
`1ea08b1429cbd888875fef195f9b94bef666e70e` and Rust artifact SHA-256
`298aa826cb7bdf182742251d53c9ab6a436ba8e386fd292a22701a7dec40cefb` in
`crates/omega_effectd/all-work-contract/SOURCE.json`. Omega's Rust supervisor
also has an opt-in cross-repository test that starts the pinned TypeScript
process and negotiates `omega-effectd.v2` before it decodes the typed Work
Index response.
