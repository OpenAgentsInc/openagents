# ADR-0005: Compatibility Negotiation and Support Window Policy

## Status

Accepted

## Date

2026-02-21

## Owner Lane

`owner:contracts-docs`

## Context

Proto lint and breaking checks protect schema evolution at source control boundaries, but live runtime safety requires explicit client/server negotiation semantics at connection time.

Without a single compatibility policy, different surfaces can apply inconsistent behavior for:

1. `schema_version` validation,
2. protocol version compatibility,
3. client build support windows,
4. deterministic upgrade-required failures.

This creates operational risk during deploys, canaries, and rollback where web/desktop/iOS clients run mixed versions.

## Decision

OpenAgents defines a single compatibility negotiation contract across control APIs and Khala WS.

Normative rules:

1. Every client handshake must include:
   - `client_build_id`
   - `protocol_version`
   - `schema_version`
2. Server-side compatibility windows are explicit and environment-bound:
   - `min_client_build_id`
   - optional `max_client_build_id`
   - `min_schema_version`
   - `max_schema_version`
   - required `protocol_version`
3. Negotiation is fail-closed. Any missing/unsupported field returns deterministic upgrade-required semantics.
4. Failure codes are canonical and shared across surfaces:
   - `invalid_client_build`
   - `unsupported_protocol_version`
   - `unsupported_schema_version`
   - `upgrade_required`
   - `unsupported_client_build`
5. Control APIs and Khala WS must expose machine-readable failure payloads with the active support window so clients can show deterministic upgrade UX.

Surface contract requirements:

1. Control API failures use HTTP error payloads with code + support-window fields.
2. Khala WS failures use join/error payloads with the same code set + support-window fields.
3. Clients treat compatibility failures as non-retryable until upgraded or support window changes.

## Rust-Era Boundary Impact

- Control-plane boundary: handshake metadata validation is now mandatory for client entrypoints.
- Runtime authority boundary: unchanged (no authority mutation semantics changed).
- Khala delivery boundary: WS subscribe/join compatibility checks are standardized.
- Client/runtime contract boundary: upgrade-required semantics are deterministic and shared.
- Deployment/runtime ops boundary: support-window updates become release-gated operations.

## Invariant Gate Mapping

Source: `docs/plans/active/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-01` (Rust-only product/runtime contract authority)
   - `INV-05` (proto-first schema compatibility discipline)
   - `INV-07` (deterministic client apply/replay behavior)
2. Preservation/change:
   - Preserves proto-first governance by making `schema_version` enforced negotiation input.
   - Preserves deterministic behavior by requiring explicit failure codes and window metadata.
3. Follow-up gate requirements:
   - OA-RUST-084 implements runtime/control enforcement gates using this policy.
   - Release runbooks must include support-window update sequencing.

## Compatibility and Migration Plan

1. Backward/forward compatibility:
   - New compatibility fields are additive and can be introduced before hard enforcement.
   - During migration, unsupported clients receive upgrade-required payloads rather than ambiguous disconnects.
2. Rollout sequence:
   - Publish policy + fixtures (this ADR + protocol docs).
   - Introduce shared negotiation helper/tests.
   - Implement enforcement in control + Khala (OA-RUST-084).
3. Migration requirements:
   - Web manifest/service-worker update policy must carry `buildId` + compatibility window.
   - Control/Khala docs must point to this policy as the single compatibility authority.

## Rollback and Failure Impact

1. Rollback triggers:
   - Unexpected rejection spikes after support-window changes.
   - Client compatibility false negatives in canary.
2. Rollback procedure:
   - Widen support window (`min_client_build_id` down / `max_client_build_id` up or unset).
   - Redeploy compatibility config first, then reassess client reconnect behavior.
3. Residual risk:
   - If build IDs are malformed/non-monotonic, negotiation ordering can be unreliable; build ID format remains a hard requirement.

## Verification

Required policy-level checks:

```bash
cargo test -p openagents-client-core compatibility::
node --test apps/openagents.com/web-shell/host/update-policy.test.mjs
apps/openagents.com/web-shell/scripts/sw-policy-verify.sh
```

## Consequences

### Positive

- One compatibility doctrine for control APIs and Khala WS.
- Deterministic client upgrade behavior during mixed-version deploy windows.
- Clear operational process for canary/rollback support-window changes.

### Negative

- Additional release discipline: support-window updates must be coordinated across static assets and server configs.
- Temporary increase in doc/contracts surface until full enforcement is complete.

### Neutral

- Does not change authority write-path ownership or data-plane boundaries.

## Alternatives Considered

1. Keep compatibility logic per-surface with ad hoc rules.
   - Rejected: inconsistent failure semantics and rollout risk.
2. Enforce schema version only, ignore build windows.
   - Rejected: does not cover mixed-bundle rollout/rollback safety.
3. Build-window enforcement only, ignore protocol/schema negotiation.
   - Rejected: insufficient for wire evolution safety.

## References

- `docs/ARCHITECTURE-RUST.md`
- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`
- `docs/protocol/OA_SYNC_WS_MAPPING.md`
- `apps/openagents.com/service/docs/SW_ASSET_PINNING_ROLLBACK_RUNBOOK.md`
- Related issue: `OA-RUST-083` / `#1918`
