# ADR-0008: Bounded Vercel SSE Compatibility Lane

## Status

Accepted

## Date

2026-02-22

## Owner Lane

`owner:openagents-web`

## Context

The Laravel web app used Vercel AI SDK stream semantics for chat in several clients. The Rust cutover retired that lane and consolidated authority on codex worker control + Khala WS replay/live delivery.

Some clients still require a temporary HTTP/SSE shape compatible with the Vercel stream contract while migration completes. We need explicit governance so this compatibility lane does not reintroduce Laravel/PHP authority paths or split chat/thread ownership.

## Decision

OpenAgents allows a bounded Vercel-compatible SSE lane only as a presentation adapter over existing codex/Khala authority outputs.

Normative rules:

1. Codex worker control remains command authority for chat/thread mutations.
2. Khala WS remains the only live sync/replay authority lane.
3. SSE compatibility is adapter-only serialization for client consumption.
4. SSE compatibility must not create or persist any independent thread/run/message authority state.
5. Adapter endpoints must be removable without affecting codex/Khala authority models.

Non-goals and prohibitions:

1. No Laravel/PHP runtime reintroduction in active product paths.
2. No second persistence authority for chats, runs, or messages.
3. No Khala SSE transport lane and no WS doctrine exception for live replay authority.

## Rust-Era Boundary Impact

- Control-plane boundary: unchanged; commands remain authenticated HTTP APIs.
- Runtime authority boundary: unchanged; codex worker remains execution authority.
- Khala delivery boundary: unchanged; WS-only replay/live authority delivery remains intact.
- Client/runtime contract boundary: adds adapter serialization contract only.
- Deployment/runtime ops boundary: requires explicit rollback switch for adapter lane without touching authority paths.

## Invariant Gate Mapping

Source: `docs/plans/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-02` (HTTP-only authority mutations)
   - `INV-03` (Khala WS-only live transport)
   - `INV-06` (Khala delivery, not authority)
   - `INV-10` (legacy removal ordering)
2. Preservation/change:
   - Preserves `INV-02` by forbidding adapter writes to authority state.
   - Preserves `INV-03` by restricting SSE to presentation compatibility only.
   - Preserves `INV-06` by keeping Khala in delivery/replay scope only.
   - Updates `INV-10` execution sequencing to allow a temporary compatibility bridge before final retirement.
3. Follow-up gate requirements:
   - Compatibility headers and stream event mapping must be codified and tested.
   - Dual-run drift detection must compare codex-native semantics vs adapter output.
   - Final cleanup removes retired alias headers/behavior once compatibility lane stabilizes.

## Compatibility and Migration Plan

1. Backward/forward compatibility:
   - Existing Vercel-style clients can consume adapter SSE without authority changes.
   - New clients should prefer codex-native contracts where possible.
2. Rollout sequence:
   - Publish mapping contract and fixtures.
   - Enable adapter behind compatibility negotiation.
   - Run staging dual-run diff and production cutover gates.
   - Remove retired alias semantics after stabilization.
3. Migration requirements:
   - No schema additions that create separate chat authority state.
   - OpenAPI and parity docs must mark compatibility as bounded and removable.

## Rollback and Failure Impact

1. Rollback triggers:
   - Event ordering drift versus codex semantics.
   - Elevated stream errors/SLO violations tied to adapter lane.
   - Evidence of authority fork or duplicate persistence path.
2. Rollback procedure:
   - Disable adapter route mode and fall back to codex-native authority responses.
   - Preserve codex + Khala paths unchanged.
   - Keep drift reports for post-incident remediation.
3. Residual risk:
   - Compatibility-only clients may require coordinated client rollout windows during rollback.

## Verification

Required checks:

```bash
./scripts/local-ci.sh docs
```

## Consequences

### Positive

- Enables controlled client migration without restoring legacy authority paths.
- Keeps a single chat/thread authority model.
- Makes rollback straightforward because authority boundaries stay unchanged.

### Negative

- Adds temporary adapter complexity and extra regression surface.
- Requires strict drift testing to avoid semantic mismatch.

### Neutral

- Does not change codex or Khala protocol ownership.

## Alternatives Considered

1. Full rejection of any SSE compatibility lane.
   - Rejected: would force immediate client rewrites with avoidable delivery risk.
2. Reintroduce Laravel AI/Vercel backend lane.
   - Rejected: violates Rust-only active runtime policy and reopens authority split.
3. Allow Khala to expose SSE directly.
   - Rejected: violates WS-only replay/live transport doctrine in `ADR-0003`.

## References

- `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
- `docs/adr/ADR-0002-proto-first-contract-governance.md`
- `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
- `docs/adr/ADR-0005-compatibility-negotiation-and-support-window-policy.md`
- Related issue: `OA-WEBPARITY-069` / `#2039`
