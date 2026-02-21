# ADR-0006: Wallet Executor Auth, Key Custody, and Receipt Canonicalization

## Status

Accepted

## Date

2026-02-21

## Owner Lane

`owner:infra`

## Context

Lightning payment execution is a high-risk boundary. The wallet executor can spend funds and emits settlement artifacts consumed by control/runtime layers.

Without explicit policy:

1. control-plane to executor authentication can drift or be bypassed,
2. secret/key custody can become ad hoc,
3. payment receipts can lose deterministic hash compatibility over time.

OA-RUST-095 requires one explicit contract for auth channel, custody lifecycle, and receipt canonicalization.

## Decision

OpenAgents adopts the following normative rules for wallet executor operations:

1. Executor authentication:
   - control-plane calls to wallet executor use bearer token auth only.
   - Spark mode is fail-closed and requires `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`.
   - token comparison is constant-time.
2. Secret/key custody:
   - Spark API key and mnemonic are service secrets; production source is secret manager.
   - executor is the only payment signing authority; control-plane is orchestration only.
3. Receipt contract:
   - payment responses include canonical receipt `openagents.lightning.wallet_receipt.v1`.
   - receipt hash is SHA-256 over canonical JSON field set.
   - `receiptId` is derived from hash prefix for deterministic compatibility.
4. Rotation operations:
   - auth token and secret rotations follow documented, staged runbook order (executor deploy -> control deploy -> verify -> revoke old secrets).

## Rust-Era Boundary Impact

- Control-plane boundary: outbound payment calls require authenticated executor channel.
- Runtime authority boundary: unchanged for write authority; runtime consumes payment results/receipts.
- Khala delivery boundary: no change.
- Client/runtime contract boundary: receipt payload semantics become explicit and deterministic.
- Deployment/runtime ops boundary: key/token rotations are runbook-gated operations.

## Invariant Gate Mapping

Source: `docs/plans/active/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-01` proto-first cross-boundary contracts.
   - `INV-08` auth identity/session boundary discipline.
2. Preservation/change:
   - `INV-01`: wallet receipt/auth contracts are documented and proto-aligned.
   - `INV-08`: executor is explicitly not an identity authority; auth channel is service-to-service only.
3. Follow-up gate requirements:
   - proto lint/generate verification on receipt contract updates.
   - wallet executor integration tests for auth enforcement and receipt stability.

## Compatibility and Migration Plan

1. Backward/forward compatibility:
   - receipt fields are additive-only after `v1` release.
   - hash-critical canonical fields are immutable in `v1`.
2. Rollout sequence:
   - land server/auth enforcement + receipt generation,
   - publish custody/rotation runbook,
   - align proto/docs/tests.
3. Migration requirements:
   - older consumers without receipt parsing remain functional but should be upgraded to consume `receipt`.

## Rollback and Failure Impact

1. Rollback triggers:
   - auth outage due token mismatch after deploy,
   - receipt hash incompatibility detected in tests.
2. Rollback procedure:
   - revert to previous token version/secret set,
   - rollback executor/control service pair,
   - restore previous receipt parsing path.
3. Residual risk:
   - mnemonic rotation can alter wallet identity and requires explicit migration approval.

## Verification

Required checks:

```bash
npm --prefix apps/lightning-wallet-executor run typecheck
npm --prefix apps/lightning-wallet-executor test
./scripts/verify-proto-generate.sh
```

## Consequences

### Positive

- Explicit, enforced service auth path for payment execution.
- Clear custody and rotation operations for high-risk secrets.
- Deterministic receipt hashing contract for audit/replay consumers.

### Negative

- Additional operational rigor required for secret/token rotations.
- Receipt schema changes now require stronger compatibility review.

### Neutral

- Does not change user-facing payment UX.

## Alternatives Considered

1. Keep auth token optional in Spark mode.
   - Rejected: fail-open risk for spend authority path.
2. Use non-canonical receipt JSON with best-effort hashing.
   - Rejected: hash drift risk and poor long-term compatibility.
3. Keep custody policy only in ad hoc runbooks without ADR.
   - Rejected: weak architecture enforcement for payment boundaries.

## References

- `apps/lightning-wallet-executor/docs/AUTH_AND_KEY_CUSTODY.md`
- `apps/lightning-wallet-executor/docs/KEY_ROTATION_RUNBOOK.md`
- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- Related issue: `OA-RUST-095` / `#1930`

