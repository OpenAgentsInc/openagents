# ADR-0002: Proto-First Contract Governance

## Status

Accepted

## Date

2026-02-21

## Owner Lane

`owner:contracts-docs`

## Context

Rust-only migration increases pressure to define contracts directly in Rust types. Without explicit governance, this creates schema drift between services/clients and weakens replay compatibility guarantees for Khala and runtime protocols.

OpenAgents requires one canonical contract authority for all boundary-crossing messages and APIs.

## Decision

OpenAgents uses **proto-first governance** for all boundary-crossing contracts.

Normative rules:

1. `proto/` is the single authority for cross-process and client/server wire contracts.
2. Rust wire types must be generated from proto definitions (`crates/openagents-proto` path).
3. Rust domain models are allowed only above the wire boundary and must map explicitly to/from proto types.
4. JSON representations are derived interoperability/debug views only and are never contract authority.
5. Breaking contract changes require package version bumping; additive changes are enforced via Buf policy.

Prohibited patterns:

1. Defining boundary contracts as Rust-native structs first and retrofitting proto later.
2. Hand-authored JSON schema files as primary contract source.
3. Silent wire-breaking field renumber/rename/removal in-place for active package versions.

## Rust-Era Boundary Impact

- Control-plane boundary: proto contract authority for auth/session/control APIs.
- Runtime authority boundary: proto contract authority for runtime/codex/sync cross-service messages.
- Khala delivery boundary: proto envelope/error semantics remain replay-compatible.
- Client/runtime contract boundary: generated wire types enforce shared schema.
- Deployment/runtime ops boundary: proto verification gates become release prerequisites.

## Invariant Gate Mapping

Source: `docs/plans/active/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-01` (primary)
   - `INV-07` (ordering/idempotency contract consistency)
   - `INV-10` (legacy deletion only after mapped parity gates, including contract parity)
2. Preservation/change:
   - Strengthens `INV-01` by making proto authority mandatory for all boundary lanes.
   - Preserves `INV-07` by requiring Khala replay envelope evolution through proto governance.
3. Follow-up gate requirements:
   - Proto checks must run in local CI before merge/release.
   - Contract PRs must include Buf compatibility evidence and generated Rust updates.

## Compatibility and Migration Plan

1. Backward/forward compatibility:
   - Additive changes allowed in-place for active version namespaces.
   - Breaking changes require new version namespace (`v2`, etc.).
2. Rollout sequence:
   - Update proto definitions.
   - Regenerate Rust wire crate outputs.
   - Update domain conversion mappings.
   - Run contract checks and cross-surface harnesses.
3. Migration requirements:
   - Legacy `openagents.protocol.v1` namespaces remain transitional; new contracts follow `proto/PACKAGE_MAP.md`.

## Rollback and Failure Impact

1. Rollback triggers:
   - Buf breaking failures not resolved by version bump strategy.
   - Runtime/client contract mismatches in harness or production gate validation.
2. Rollback procedure:
   - Revert proto change and generated Rust updates together.
   - Restore previous package schema and rerun proto verification gates.
3. Residual risk:
   - Transitional mixed namespaces increase review complexity until migration closure.

## Verification

Required checks:

```bash
./scripts/local-ci.sh proto
./scripts/verify-proto-generate.sh
./scripts/verify-rust-proto-crate.sh
```

Optional strict compatibility gate:

```bash
OA_BUF_BREAKING_MODE=strict OA_BUF_BREAKING_AGAINST='.git#branch=origin/main,subdir=proto' ./scripts/local-ci.sh proto
```

## Consequences

### Positive

- Eliminates contract authority ambiguity.
- Preserves replay/auth compatibility across web/desktop/iOS/runtime boundaries.
- Supports deterministic, mechanical enforcement in CI/gates.

### Negative

- Adds ceremony for rapid experimentation across boundaries.
- Requires disciplined schema versioning and generated-code review.

### Neutral

- Internal crate-only implementation structs remain Rust-native where no boundary crossing occurs.

## Alternatives Considered

1. Rust types-first for all new contracts.
   - Rejected: high long-term drift risk and poor multi-surface compatibility.
2. Mixed proto + JSON authority based on team preference.
   - Rejected: inconsistent enforcement and incompatible evolution semantics.
3. Proto only for external APIs, Rust-only internally.
   - Rejected: runtime/Khala/client boundaries are all contract-critical and need one authority model.

## References

- `proto/README.md`
- `proto/PACKAGE_MAP.md`
- `docs/ARCHITECTURE-RUST.md`
- `docs/plans/active/rust-migration-invariant-gates.md`
- Related issue: `OA-RUST-075` / `#1890`
