# A2A Bridge Child Issue Filing Pack

Use this file as the copy/paste source for the eight child issues under `docs/plans/a2a-nostr-bridge-epic.md`.

## Shared v1 locks

- `Task.id == SA session d-tag`
- `saSessionRef = 39230:<agent_hex_pubkey>:<task_id>`
- `agentProfileRef = 39200:<agent_hex_pubkey>:`
- initial funding mode is by-reference `osceEnvelopeRef`
- generic A2A clients must still work without OpenAgents-aware extension handling

## Filing order

1. Issue 1
2. Issue 2
3. Issue 3
4. Issue 4
5. Issue 5
6. Issue 6
7. Issue 7
8. Issue 8

---

## Issue 1

**Title:** `A2A bridge: add workspace crate and shared protocol types`
**Suggested labels:** `protocol` `a2a` `nostr` `desktop`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** none

### Summary
Add the new `crates/a2a-bridge` workspace crate and establish the shared A2A bridge DTOs and bridge-local error types used by the later implementation slices.

### Scope
- add `crates/a2a-bridge` to the root workspace manifest
- create `Cargo.toml`, `src/lib.rs`, `src/types.rs`, and `src/errors.rs`
- define shared camelCase DTOs for card, task, message, artifact, auth, and stream/event payloads
- keep this slice transport-agnostic; no HTTP serving yet

### Files
- `Cargo.toml`
- `crates/a2a-bridge/Cargo.toml`
- `crates/a2a-bridge/src/lib.rs`
- `crates/a2a-bridge/src/types.rs`
- `crates/a2a-bridge/src/errors.rs`

### Acceptance criteria
- workspace registers the new crate cleanly
- shared bridge DTOs exist with stable JSON naming
- bridge-local error taxonomy exists for later HTTP/auth/funding mapping
- no desktop route mounting is introduced in this issue

### Validation
- `crates/a2a-bridge/tests/types_roundtrip.rs`

---

## Issue 2

**Title:** `A2A bridge: project SA profile and SKL manifests into AgentCard`
**Suggested labels:** `protocol` `a2a` `nostr` `identity`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** Issue 1

### Summary
Implement `AgentCard` projection from sovereign SA profile + SKL manifest state without introducing a parallel profile/config source.

### Scope
- implement `crates/a2a-bridge/src/agent_card.rs`
- project current `39200` profile data into card identity fields
- project `33400` manifests into `skills[]`
- emit OpenAgents extension params for `nip-skl`, `nip-sa`, and optional `nip-ac`
- add small `nostr/core` helpers only if needed for projection cleanliness

### Files
- `crates/a2a-bridge/src/agent_card.rs`
- `crates/nostr/core/src/nip_sa/profile.rs` (if needed)
- `crates/nostr/core/src/nip_skl/manifest.rs` (if needed)
- `docs/A2A_INTEROP_PROFILE.md` (if examples/contract notes need sync)

### Acceptance criteria
- `AgentCard.name` / `description` derive from sovereign profile state
- `skills[]` derive from canonical SKL manifests
- card emits `agentProfileRef = 39200:<agent_hex_pubkey>:`
- canonical skill ids are preserved for round-tripability
- generic A2A clients can consume the card without understanding OpenAgents-specific extensions

### Validation
- `crates/a2a-bridge/tests/agent_card_contract.rs`

---

## Issue 3

**Title:** `A2A bridge: implement SKL challenge-backed Nostr auth bridge`
**Suggested labels:** `protocol` `a2a` `nostr` `auth`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** Issue 1

### Summary
Add the auth bridge that binds A2A access to a verified Nostr identity using the optional SKL challenge/response profile.

### Scope
- implement `crates/a2a-bridge/src/auth.rs`
- add `POST /auth/nostr/challenge` and `POST /auth/nostr/complete` in `crates/a2a-bridge/src/http.rs`
- publish `33410` challenges and verify `33411` responses
- mint short-lived bearer sessions bound to the verified pubkey
- map expiry, bad response, and missing response into clear auth failures

### Files
- `crates/a2a-bridge/src/auth.rs`
- `crates/a2a-bridge/src/http.rs`
- `crates/nostr/core/src/nip_skl/mod.rs` (if helper extraction is needed)

### Acceptance criteria
- challenge issuance produces a relay-readable SKL auth challenge
- completion verifies a valid matching response before issuing a session token
- returned bearer session exposes the verified pubkey for downstream policy checks
- failure modes are deterministic and do not silently degrade to anonymous access

### Validation
- `crates/a2a-bridge/tests/auth_bridge.rs`
- extend `crates/nostr/core/tests/nip_sa_skl_ac_integration_matrix.rs`

---

## Issue 4

**Title:** `A2A bridge: map A2A tasks onto SA trajectory sessions and material events`
**Suggested labels:** `protocol` `a2a` `nostr` `runtime`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** Issue 1, Issue 3

### Summary
Implement the task runtime projection so A2A tasks map directly onto SA sessions and material trajectory events.

### Scope
- implement `crates/a2a-bridge/src/tasks.rs`
- implement `crates/a2a-bridge/src/artifacts.rs`
- back task/session projection with `crates/nostr/core/src/nip_sa/trajectory.rs`
- ensure ordered stream/subscribe behavior for task updates
- emit only material `39231` entries rather than mirroring every stream chunk

### Files
- `crates/a2a-bridge/src/tasks.rs`
- `crates/a2a-bridge/src/artifacts.rs`
- `crates/nostr/core/src/nip_sa/trajectory.rs`

### Acceptance criteria
- `Task.id` equals the `39230` session `d` tag
- each task projection emits `saSessionRef = 39230:<agent_hex_pubkey>:<task_id>`
- material milestones publish `39231` refs suitable for audit
- stream and subscribe paths preserve deterministic ordering

### Validation
- `crates/a2a-bridge/tests/task_projection.rs`
- extend `crates/nostr/core/tests/nip_sa_e2e.rs`

---

## Issue 5

**Title:** `A2A bridge: add by-reference OSCE funding validation and AC settlement flow`
**Suggested labels:** `protocol` `payments` `nostr` `a2a`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** Issue 1, Issue 4

### Summary
Add the first funding slice for the bridge using by-reference `osceEnvelopeRef` validation and AC settlement/cancel projection.

### Scope
- implement `crates/a2a-bridge/src/funding.rs`
- validate envelope refs using `nip_ac` envelope/spend/settlement helpers
- check scope, provider, max spend, expiry, rail, and cancel-window semantics
- emit receipt refs on success and cancel refs when cancellation occurs inside the allowed window

### Files
- `crates/a2a-bridge/src/funding.rs`
- `crates/nostr/core/src/nip_ac/envelope.rs`
- `crates/nostr/core/src/nip_ac/spend.rs`
- `crates/nostr/core/src/nip_ac/settlement.rs`

### Acceptance criteria
- request metadata accepts `osceEnvelopeRef`
- invalid or out-of-policy envelopes reject before compute begins
- successful funded execution returns AC receipt refs in task metadata
- cancel-in-window paths emit AC cancel refs rather than drifting from task state

### Validation
- `crates/a2a-bridge/tests/funding_policy.rs`
- extend `crates/nostr/core/tests/nip_ac_skill_linkage.rs`

---

## Issue 6

**Title:** `A2A bridge: route long-running compute through NIP-90 and project feedback into A2A`
**Suggested labels:** `protocol` `compute` `a2a` `nostr`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** Issue 1, Issue 4, Issue 5

### Summary
Project long-running or billable work onto NIP-90 request/feedback/result flows and map those updates back into A2A tasks and artifacts.

### Scope
- implement `crates/a2a-bridge/src/dvm.rs`
- reuse desktop compute flow where practical instead of duplicating lane logic
- publish request events in the `5000-5999` range
- map `7000` feedback into task status updates
- map `6000-6999` results into final A2A artifacts

### Files
- `crates/a2a-bridge/src/dvm.rs`
- `crates/nostr/core/src/nip90/data_vending.rs`
- `apps/autopilot-desktop/src/nip90_compute_flow.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`

### Acceptance criteria
- funded execution does not publish NIP-90 work until envelope validation succeeds
- long-running work surfaces useful progress to generic A2A clients
- final result projection produces artifacts rather than raw NIP-90 event leakage
- bridge metadata retains the sovereign refs needed for auditability

### Validation
- `crates/a2a-bridge/tests/funded_dvm_flow.rs`
- extend `crates/nostr/core/tests/nip90_integration.rs`

---

## Issue 7

**Title:** `Autopilot Desktop: mount the A2A bridge HTTP surface and local serve mode`
**Suggested labels:** `desktop` `a2a` `http` `runtime`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** Issue 2, Issue 3, Issue 4, Issue 5, Issue 6

### Summary
Host the first bridge surface inside Autopilot Desktop using the existing `axum` stack and add a headless/local serve path for development and smoke testing.

### Scope
- add `apps/autopilot-desktop/src/a2a_server.rs`
- mount card, auth, send, stream, get, list, cancel, and subscribe routes
- wire bridge config for bind address, relay list, and auth TTL
- add a headless serve entrypoint without requiring the full desktop UI path

### Files
- `apps/autopilot-desktop/src/a2a_server.rs`
- `apps/autopilot-desktop/src/lib.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/Cargo.toml`

### Acceptance criteria
- serves `/.well-known/agent-card.json` and the initial auth/task HTTP contract
- desktop host composes the bridge crate cleanly without pulling protocol ownership into the app layer
- local serve mode is usable for development and smoke tests
- Nexus/relay configuration is injectable rather than hardcoded

### Validation
- `apps/autopilot-desktop/src/a2a_server_tests.rs`
- `apps/autopilot-desktop/tests/autopilotctl_a2a.rs`

---

## Issue 8

**Title:** `Docs: publish the A2A bridge implementation plan, endpoint contract, and rollout notes`
**Suggested labels:** `docs` `a2a` `nostr` `protocol`
**Parent tracker:** `docs/plans/a2a-nostr-bridge-epic.md` / GitHub epic `#<epic-number>`
**Depends on:** Issue 1, Issue 2, Issue 3, Issue 4, Issue 5, Issue 6, Issue 7

### Summary
Update the repo docs so the bridge is documented as an implementation-ready surface rather than planning-only prose.

### Scope
- sync the implementation-plan doc with the landed crate/runtime shape
- freeze the endpoint contract and metadata keys in the canonical docs
- document rollout assumptions and the v1 by-reference funding posture
- ensure no contradictions remain with `docs/PROTOCOL_SURFACE.md`

### Files
- `docs/plans/a2a-bridge-pr-implementation-plan.md`
- `docs/A2A_INTEROP_PROFILE.md`
- `docs/plans/a2a-sovereign-agent-integration-plan.md`
- `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md`

### Acceptance criteria
- docs reflect the implemented bridge contract instead of exploratory planning language
- endpoint and metadata names are stable and consistent across docs
- crate boundaries and desktop-hosting assumptions remain explicit
- v1 funding mode is documented as by-reference only until intentionally expanded

### Validation
- doc review
- any contract fixtures or smoke tests added by Issues 1-7