# Epic: A2A ↔ Nostr bridge implementation tracker

## Labels
`epic` `a2a` `nostr` `nip-sa` `nip-skl` `nip-ac` `nip-90` `desktop` `tracking`

## Summary
Deliver the first production-ready A2A ↔ Nostr bridge for OpenAgents, hosted initially in `apps/autopilot-desktop` and implemented primarily in a new `crates/a2a-bridge` workspace crate.

## Canonical model
- A2A is the wire/discovery/orchestration surface.
- Nostr remains canonical for identity, skill, lifecycle, funding, and settlement state.
- The bridge is a projection layer, not a second source of truth.

## Scope
In scope:
- `AgentCard` projection from SA profile + SKL manifests
- SKL challenge-backed auth bridge
- task/session projection onto SA trajectory
- by-reference AC funding validation
- NIP-90 routing for long-running compute
- desktop HTTP hosting and headless serve mode
- docs, rollout notes, and tests

Out of scope for v1:
- gRPC transport
- push-notification webhooks
- full delegation/sub-agent inheritance
- mirroring every stream chunk into `39231`
- any change to A2A core wire semantics or NIP kind allocations

## Locked invariants
- `Task.id == SA session d-tag`
- `saSessionRef = 39230:<agent_hex_pubkey>:<task_id>`
- `agentProfileRef = 39200:<agent_hex_pubkey>:`
- initial funding mode is by-reference via `osceEnvelopeRef`
- generic A2A clients must still function without OpenAgents-aware extension handling

## New workspace unit
- add `crates/a2a-bridge` to the root `Cargo.toml`
- mount the initial HTTP surface from `apps/autopilot-desktop`

## Child issues
1. Add workspace crate and shared A2A bridge types
2. Project SA profile + SKL manifests into `AgentCard`
3. Implement SKL challenge-backed Nostr auth bridge
4. Map A2A tasks onto SA trajectory sessions
5. Add by-reference OSCE funding validation and AC settlement flow
6. Route long-running compute through NIP-90 and project feedback into A2A
7. Mount the A2A bridge HTTP surface in Autopilot Desktop
8. Update docs with implementation plan, endpoint contract, and rollout notes

## Execution order
Phase 1:
- Issue 1
- Issue 2
- Issue 3

Phase 2:
- Issue 4
- Issue 5
- Issue 6

Phase 3:
- Issue 7
- Issue 8

## Files and references
- `docs/A2A_INTEROP_PROFILE.md`
- `docs/plans/a2a-sovereign-agent-integration-plan.md`
- `docs/PROTOCOL_SURFACE.md`
- `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md`
- `docs/plans/a2a-bridge-pr-implementation-plan.md`
- `docs/plans/a2a-nostr-bridge-crate-breakdown.md`
- `docs/plans/a2a-nostr-bridge-issues.md`

## Acceptance criteria
- A maintainer can understand scope, sequencing, ownership, and invariants from repo docs alone.
- The epic preserves the architecture rule that A2A is the edge protocol and Nostr is canonical state.
- The issue pack is detailed enough to file child issues without rewriting.
- The spec clearly identifies crate boundaries, file targets, test expectations, and desktop hosting assumptions.

## Suggested footer for filing
## Linked Issues
- Tracker only; child issues carry implementation detail.
- Child 1: #<issue-number>
- Child 2: #<issue-number>
- Child 3: #<issue-number>
- Child 4: #<issue-number>
- Child 5: #<issue-number>
- Child 6: #<issue-number>
- Child 7: #<issue-number>
- Child 8: #<issue-number>

