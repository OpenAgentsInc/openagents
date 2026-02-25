# ADR-0007: Spacetime-Only Sync Transport Hard Mandate

## Status

Accepted

## Date

2026-02-26

## Owner Lane

`owner:protocol-runtime`

## Context

Earlier migration ADRs established Spacetime as canonical sync doctrine, but retained transitional wording
allowed legacy interpretation in implementation and docs. The active plan and issue program for
final convergence now requires hard deletion semantics, not "default unless overridden" behavior.

Observed retained gaps at issue start:

1. Desktop sync still targeted legacy websocket pathing and Phoenix-shaped frame handling.
2. Runtime publisher still retained in-memory publication behavior in core state bootstrap.
3. Control sync token routes and docs retained alias drift.

This ADR formalizes the final posture needed for auditable closure.

## Decision

OpenAgents mandates Spacetime-only sync transport for retained runtime/client surfaces.

Normative requirements:

1. Desktop/runtime sync transport must target Spacetime subscribe semantics at
   `/v1/database/:name_or_identity/subscribe`.
2. Runtime publication must write through Spacetime reducer calls against configured Spacetime
   host/database targets; in-memory-only publisher paths are forbidden in retained runtime
   execution.
3. Legacy websocket transport contracts (`/sync/socket/websocket`, Phoenix frame protocol,
   topic-scoped poll/fanout compatibility lanes) are prohibited in retained production paths.
4. Control service exposes one canonical sync token issuance contract; aliases are compatibility
   debt and are prohibited unless approved via the exception process below.
5. Replay/idempotency invariants remain required with stream-sequence keys `(stream_id, seq)`.
6. Active docs and runbooks must match shipped behavior before issue/program closure.

## Exception Process (Firm Technical Hurdle)

A temporary exception is allowed only if all conditions are satisfied:

1. Concrete blocker that cannot be solved safely in scope.
2. Documented owner, expiry date, and linked deletion issue.
3. Explicit blast radius and rollback steps.
4. Maximum age 14 days without explicit renewal evidence.

Exceptions are non-compliant by default after expiry.

## Invariant Gate Mapping

Source: `docs/plans/rust-migration-invariant-gates.md`

1. `INV-01` proto-first contracts: preserved.
2. `INV-02` HTTP-only authority mutations: preserved.
3. `INV-03` Spacetime-only live transport: hardened and now enforced.
4. `INV-06` sync as delivery, not authority: preserved.
5. `INV-07` replay/idempotency: preserved and required in acceptance gates.
6. `INV-10` legacy removal ordering: finalized under no-legacy closure criteria.

## Verification

Required evidence classes:

1. Runtime publisher tests proving no retained in-memory lane in runtime bootstrap path.
2. Desktop transport/parser tests proving no Phoenix frame compatibility handling.
3. Integration/chaos gates proving reconnect, stale cursor, and dedupe behavior.
4. Repo guard checks proving prohibited legacy symbols are absent from retained paths.

## Consequences

### Positive

1. Removes transport ambiguity and closes doctrine loopholes.
2. Makes final deletion and regression prevention testable.
3. Simplifies operator posture to one sync model.

### Negative

1. Requires coordinated updates across runtime, desktop, control, protocol, and docs.
2. Raises immediate failure visibility for misconfigured Spacetime environment state.

### Neutral

1. Legacy behavior may exist in archived material, but cannot remain in retained execution paths.
