# A2A ↔ OpenAgents Sovereign Stack Integration Plan

**Status:** Planning / design handoff (non-normative)  
**Audience:** Chris, engineering leads, implementers, coding agents  
**Primary layers:** A2A, NIP-SKL, NIP-SA, NIP-AC

---

## 1. Purpose and Scope

This document consolidates the proposed integration model for letting an **A2A-compliant agent** incrementally adopt the OpenAgents sovereign stack:

- **NIP-SKL** — skill registry, identity, attestation, revocation, auth challenge, permission grants
- **NIP-SA** — sovereign agent identity, guardianship, lifecycle, sessions/trajectories, audit, delegation
- **NIP-AC** — outcome-scoped credit (OSCE), spend authorization, settlement, hold/cancel behavior

This is an **implementation-oriented handoff**, not a replacement for either the A2A spec or the NIP specs. Its purpose is to give the team a concrete layering model, implementation phases, interface expectations, and ticketable workstreams.

### Intended readers

- Chris and product/spec reviewers deciding what to schedule
- Engineers implementing the server, identity, audit, or payments pieces
- Coding agents that need a practical source of truth for sequencing work

### In scope

- How an A2A agent can expose **sovereign identity, skills, guardrails, and payment rails** without changing A2A itself
- How to map A2A concepts onto SKL / SA / AC responsibilities
- Recommended metadata shapes and identifiers to support implementation
- A phased rollout plan that preserves the current layering model

### Out of scope

- Rewriting A2A
- Collapsing SKL / SA / AC into one protocol
- Replacing Nostr as the canonical event substrate for identity, lifecycle, audit, or credit
- Immediate product commitment to ship the full interop surface in MVP
- Deep UI design or specific runtime crate ownership changes

### Scope guardrail

This repo is currently MVP-oriented and desktop-first. This document is therefore best treated as a **planning and architecture handoff** that can be implemented incrementally and only where it supports the retained product direction.

### Related document

For the protocol-facing, non-normative interoperability reference that complements this implementation handoff, see:

- [`docs/A2A_INTEROP_PROFILE.md`](../A2A_INTEROP_PROFILE.md)

---

## 2. Architectural Summary

### Core position

- **A2A remains the wire protocol and interoperability layer.**
- **NIP-SKL remains the canonical skill / identity / trust layer.**
- **NIP-SA remains the canonical lifecycle / guardianship / audit / delegation layer.**
- **NIP-AC remains the canonical credit / settlement / rollback layer.**

The integration goal is **layering**, not replacement:

1. An agent speaks **A2A** to clients and peer agents.
2. That same agent publishes and verifies canonical state through **Nostr events**.
3. The A2A server surface projects selected sovereign capabilities into A2A structures such as `AgentCard`, `Task`, and `securitySchemes`.

### Canonical sources of truth

| Concern | Canonical layer | A2A role |
| --- | --- | --- |
| Agent identity | NIP-SA Agent Profile (`kind:39200`) + NIP-SKL identity semantics | Discovery and transport-facing presentation |
| Skill catalog | NIP-SKL manifests (`kind:33400`) and related attestations | AgentCard skill projection |
| Authorization substrate | NIP-SKL auth challenge + permission grants | Declared through A2A `securitySchemes` and enforced at request handling |
| Lifecycle / task execution | NIP-SA sessions, ticks, trajectories, audit events | Task and message exchange |
| Delegation / sub-agents | NIP-SA delegation events | Optional orchestration behavior behind A2A tasks |
| Funding / settlement | NIP-AC OSCE envelopes, spends, receipts, cancel semantics | Optional payment/metering integration carried alongside A2A task execution |

### Architectural principle

The A2A-facing surface should be understood as a **projection layer** over sovereign state, not as an independent source of truth.

---

## 3. Conceptual Mapping

### 3.1 A2A `AgentCard` → SA Agent Profile + SKL manifests + optional org identity

Recommended mapping:

- `AgentCard.id`
  - SHOULD bind to the sovereign agent's Nostr identity.
  - Preferred form: `nostr:<npub>`.
- `AgentCard.name` / `description`
  - SHOULD be derived from the SA Agent Profile (`kind:39200`).
- `AgentCard.skills[]`
  - SHOULD be projected from SKL manifests (`kind:33400`).
  - Each skill entry SHOULD remain traceable back to a SKL manifest reference such as `33400:<pubkey>:<d-tag>`.
- `AgentCard.metadata.nip05` or extension metadata
  - MAY surface optional NIP-05 organizational identity already supported by SKL.
- `AgentCard.extensions[]`
  - SHOULD declare any OpenAgents-specific layering URIs used to signal SKL / SA / AC participation.

### 3.2 A2A `Task` → SA session / trajectory

Recommended mapping:

- A2A `Task.id` SHOULD correlate to an SA `session` identifier.
- New tasks SHOULD create a new SA trajectory session (`kind:39230`).
- Multi-turn tasks SHOULD continue the same session where appropriate.
- Task status transitions SHOULD map to SA lifecycle state and be mirrored in audit / trajectory events.

### 3.3 A2A `Message` / `Part` / `Artifact` → SA ticks + SKL-described capabilities

- A2A request messages correspond to SA tick inputs and/or trajectory input material.
- The skill/tooling actually used SHOULD be selected from SKL-described capabilities.
- Outputs, files, and structured results MAY be exposed as A2A artifacts while being hashed or referenced in SA audit events.
- Large or sensitive outputs SHOULD live off-Nostr, with only references / hashes published where needed.

### 3.4 A2A `securitySchemes` / authorization model → SKL auth challenge + grants + SA guardians

- A2A authentication can declare a custom scheme that is backed by SKL auth challenge semantics.
- A2A authorization decisions SHOULD be enforced using:
  - SKL auth challenge proof-of-possession
  - SKL permission grants (`kind:33420`)
  - SA guardian thresholds and security posture for sensitive operations

### 3.5 A2A payment / metering expectations → NIP-AC OSCE and settlement

- A2A does not standardize sovereign payment rails; NIP-AC provides the canonical payment substrate.
- Funding for work SHOULD be represented by OSCE envelopes.
- Spend authorization, settlement receipt, and cancel behavior remain NIP-AC responsibilities.
- A2A request/response bodies or metadata may carry OSCE references or funding payloads, but AC events remain authoritative.

---

## 4. Implementation Flows

The flows below are implementation-oriented and explicitly identify which layer handles which step.

### 4.1 Discovery and identity verification flow

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Client fetches `/.well-known/agent-card.json` | A2A |
| 2 | Client reads `AgentCard.id`, supported interfaces, and extension URIs | A2A |
| 3 | If `AgentCard.id` is `nostr:<npub>`, client resolves that as the sovereign identity anchor | A2A + SA |
| 4 | Client reads SKL/SA/AC extension metadata (relays, manifest refs, capability hints) | A2A projection of SKL/SA/AC |
| 5 | Client optionally verifies server identity via SKL auth challenge / response against the claimed pubkey | NIP-SKL |
| 6 | Client caches the verified sovereign identity and its discovered interfaces | A2A + NIP-SKL |

Implementation notes:

- Discovery should work even for clients that ignore the OpenAgents extensions.
- Verification via SKL challenge should be optional for basic discovery, but recommended for higher-trust use cases.

### 4.2 Authentication and authorization flow

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Server declares supported `securitySchemes` in AgentCard | A2A |
| 2 | Client acquires or derives credentials using the declared scheme | A2A + NIP-SKL |
| 3 | Client submits A2A request with credential/token/header material | A2A |
| 4 | Server validates that the presented credential maps to a Nostr identity or session authorized by the auth challenge flow | NIP-SKL |
| 5 | Server checks applicable SKL permission grants for requested skill, tool, action, and data scope | NIP-SKL |
| 6 | Server evaluates SA guardian policy if the operation is high-risk or exceeds local policy thresholds | NIP-SA |
| 7 | Server accepts or rejects the request and records the decision in audit logs if required | NIP-SA |

Implementation notes:

- Treat A2A `securitySchemes` as the discovery surface, not the authoritative policy layer.
- SKL grants should be enforced before expensive work begins.

### 4.3 Task execution and audit logging flow

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Client calls `SendMessage` or `SendStreamingMessage` | A2A |
| 2 | Server creates or resumes an SA session / trajectory | NIP-SA |
| 3 | Server binds A2A `Task.id` to the SA session ID | A2A + NIP-SA |
| 4 | Server selects the SKL-backed skills that satisfy the request | NIP-SKL |
| 5 | Server performs ticks / actions and emits trajectory or result events as needed | NIP-SA |
| 6 | Server emits SA audit events (`kind:39250`) for key actions, approvals, and outputs | NIP-SA |
| 7 | Server returns A2A task status and artifacts, optionally with metadata linking back to session, grant, or audit refs | A2A projection of SA/SKL |

Implementation notes:

- The A2A task must be understandable on its own.
- The SA session and audit trail must remain sufficient to reconstruct what happened.

### 4.4 Funding and settlement flow

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Client includes an OSCE envelope or reference before or during task initiation | NIP-AC carried via A2A |
| 2 | Server resolves and validates the envelope, currency, amount, skill scope, expiry, and underwriter trust | NIP-AC + NIP-SKL |
| 3 | Server checks whether guardian approval is required before spending | NIP-SA |
| 4 | Task executes under the allowed budget and permitted skill set | NIP-SA + NIP-AC |
| 5 | Server issues spend authorization / settlement events as work progresses or completes | NIP-AC |
| 6 | If `hold_period_secs` is active, server honors hold/cancel window before final settlement | NIP-AC |
| 7 | A2A task concludes with success, failure, interruption, or cancellation, with funding outcome reflected in AC events | A2A + NIP-AC |

Implementation notes:

- Settlement correctness should never depend solely on A2A response payloads.
- The AC event chain is the payment record.

### 4.5 Optional delegation / sub-agent flow

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Primary agent decides to delegate part of the task | NIP-SA |
| 2 | Server checks delegation is allowed under guardian policy, permission grant, and OSCE scope | NIP-SA + NIP-SKL + NIP-AC |
| 3 | Server emits an SA delegation event (`kind:39260`) describing sub-agent, scope, and expiry | NIP-SA |
| 4 | Delegated work may be handled via internal orchestration or another A2A call | NIP-SA + optional A2A |
| 5 | Parent audit chain records the delegation relationship and downstream outputs | NIP-SA |
| 6 | Budget and settlement remain bounded by the parent OSCE and delegation scope | NIP-AC |

Implementation notes:

- Delegation should be treated as advanced scope.
- It is not required for initial A2A interop.

---

## 5. Recommended Data Shapes / Interface Contracts

The following are recommended implementation contracts, not normative protocol changes.

### 5.1 Identifier conventions

| Concept | Recommended form | Notes |
| --- | --- | --- |
| Sovereign agent ID | `nostr:<npub>` | Used in `AgentCard.id` |
| SKL manifest ref | `33400:<pubkey>:<d-tag>` | Matches addressable SKL manifest reference |
| SA session ID | implementation-defined opaque string | Must be stable enough to correlate with `Task.id` |
| AC envelope ref | `39240:<pubkey>:<d-tag>` or event id reference | Choose one consistent implementation strategy |
| Audit ref | `39250:<pubkey>:<d-tag>` or event id reference | Used in metadata correlation |
| Permission grant ref | event id or addressable reference | Needed for traceability in audits |

### 5.2 Recommended AgentCard extension URIs

- `https://openagents.org/extensions/nip-skl/v1`
- `https://openagents.org/extensions/nip-sa/v1`
- `https://openagents.org/extensions/nip-ac/v1`

These URIs should be treated as capability hints that the agent speaks A2A while backing those semantics with sovereign Nostr events.

### 5.3 Recommended AgentCard extension metadata

#### SKL extension metadata

Suggested fields:

- `nostr_relays: string[]`
- `skl_manifest_refs: string[]`
- `auth_challenge_supported: boolean`
- `permission_grants_required: boolean`
- `assurance_summary: { [skill_ref]: tier }` (optional)

#### SA extension metadata

Suggested fields:

- `agent_profile_ref: string`
- `guardian_threshold: number`
- `audit_trail_available: boolean`
- `delegation_supported: boolean`
- `security_posture_summary: object`

#### AC extension metadata

Suggested fields:

- `osce_supported: boolean`
- `accepted_currencies: string[]`
- `accepted_underwriters: string[]`
- `max_envelope_amounts: object` or `max_amount_by_currency`
- `hold_period_secs_default: number`
- `osce_handover_mode: "by-reference" | "data-part" | "either"`

### 5.4 Recommended A2A task metadata correlation fields

Suggested server-added fields:

- `sa_session_id`
- `sa_session_ref`
- `skl_skill_refs_used`
- `permission_grant_refs`
- `guardian_approval_refs`
- `osce_envelope_ref`
- `ac_spend_refs`
- `audit_refs`

These fields should be safe to ignore by generic A2A clients while giving interoperable clients enough information to correlate back to sovereign state.

### 5.5 Recommended handover modes for OSCE funding

Preferred options:

1. **By reference**
   - Client passes an AC envelope ref in task metadata or a data part.
   - Server resolves envelope from Nostr.
2. **Inline data part**
   - Client passes a signed envelope payload in an A2A data part.
   - Server still resolves or verifies against canonical AC events.
3. **Hybrid**
   - Client passes both ref and payload for lower round-trip latency.

Recommendation: start with **by reference** for simpler trust boundaries.

---

## 6. Phased Implementation Plan

This sequence is designed so an existing A2A agent can bolt on sovereign capabilities incrementally.

### Phase 1 — Discovery and identity binding

**Objectives**

- Publish an A2A AgentCard that binds to a sovereign Nostr identity.
- Establish extension URIs and minimal correlation metadata.

**Dependencies**

- Existing A2A server surface
- SA Agent Profile publication path

**Deliverables**

- `AgentCard.id = nostr:<npub>`
- AgentCard generation from SA profile name/description
- Basic SKL / SA extension declarations
- Relay and manifest references exposed via metadata

**Risks / open questions**

- Exact metadata field names should be kept stable once clients depend on them.
- Need a clear rule for whether `npub` or raw hex pubkey is preferred in different contexts.

### Phase 2 — SKL skill projection into AgentCard

**Objectives**

- Make A2A `skills[]` a usable projection of SKL manifests.

**Dependencies**

- Phase 1 identity binding
- SKL manifests available for targeted skills

**Deliverables**

- Skill projection pipeline from `kind:33400` to `AgentCard.skills[]`
- Optional exposure of assurance tiers and evaluator refs
- Clear mapping from skill id back to manifest reference

**Risks / open questions**

- Need to decide how much SKL detail belongs in public card vs extended card.
- Revocation semantics must be reflected cleanly in AgentCard refresh/update behavior.

### Phase 3 — Auth challenge and permission grant enforcement

**Objectives**

- Back A2A auth and authorization with SKL primitives.

**Dependencies**

- SKL auth challenge implementation
- Permission grant issuance and validation path

**Deliverables**

- A2A `securitySchemes` entry for Nostr / SKL-backed auth
- Server-side auth challenge verification
- Grant enforcement middleware for request handling
- Authorization failure mapping into A2A errors

**Risks / open questions**

- Token/session format bridging A2A request auth to Nostr proof may need a local profile decision.
- Need to define how long challenge-derived credentials live.

### Phase 4 — SA session and audit integration

**Objectives**

- Ensure each A2A task has a corresponding sovereign lifecycle record.

**Dependencies**

- Phase 1 discovery
- Phase 2 skill mapping
- Phase 3 auth and grants

**Deliverables**

- Session creation / resume path for each A2A task
- `Task.id` ↔ `session` correlation strategy
- Audit event emission (`kind:39250`) for key actions
- Task metadata linking to session and audit refs

**Risks / open questions**

- Need a stable policy for when to emit audit entries to avoid noisy or incomplete chains.
- Need to decide whether every streaming chunk is audited or only material transitions.

### Phase 5 — NIP-AC funding and settlement

**Objectives**

- Allow funded A2A work using OSCE envelopes and settlement semantics.

**Dependencies**

- AC event production / verification
- Guardian policy integration for spend-sensitive actions

**Deliverables**

- Envelope resolution / validation path
- Budget enforcement during task execution
- Spend authorization and settlement emission
- Hold/cancel support mapped to task outcome handling

**Risks / open questions**

- Need a clear model for partial completion, partial settlement, and interrupted tasks.
- Need policy for whether unfunded tasks are rejected, rate-limited, or allowed in limited mode.

### Phase 6 — Extended cards, delegation, and advanced trust metadata

**Objectives**

- Add richer interop only after the base flow is stable.

**Dependencies**

- Phases 1–5 complete or sufficiently stable

**Deliverables**

- Extended AgentCard support for authenticated clients
- Delegation/sub-agent metadata and flows
- Richer trust metadata (assurance, org identity, evaluation refs)
- Better client guidance on capability tiers

**Risks / open questions**

- Extended card content must avoid leaking sensitive infrastructure details.
- Delegation creates more complex audit and budget inheritance logic.

---

## 7. Engineering Work Breakdown

These workstreams should be easy to convert into tickets.

### 7.1 A2A server surface

- Implement or refine HTTP/JSON A2A endpoints
- Add support for required request metadata and extension header handling
- Add task metadata injection hooks for sovereign correlation fields

### 7.2 AgentCard generation

- Generate card from SA profile + SKL manifests
- Support public card and optional extended card variants
- Add extension metadata serialization and versioning

### 7.3 Nostr event publishing / retrieval

- Resolve SKL manifests, SA profiles, audit refs, AC envelope refs from relays
- Publish SA session / audit / delegation events as needed
- Cache and refresh strategies for relay-backed data

### 7.4 Auth challenge verification

- Implement SKL auth challenge request / response flow
- Bind challenge success to A2A auth/session handling
- Define and implement credential bridging format

### 7.5 Permission grant enforcement

- Grant lookup and validation middleware
- Policy checks for tools, actions, data scope, expiration, and invocation counts
- Failure mapping into A2A response semantics

### 7.6 Session and audit correlation

- Session id generation / correlation policy
- Audit emission points for task lifecycle and tool use
- Correlation between A2A task ids, session ids, and audit refs

### 7.7 OSCE handling and settlement logic

- Envelope parsing and validation
- Budget tracking during execution
- Spend authorization / settlement / cancel support
- Reconciliation between task outcomes and AC event chain

### 7.8 Guardian and delegation policy

- Guardian approval checks during request authorization and spend flow
- Delegation scope subset enforcement
- Parent/child session and budget inheritance checks

### 7.9 Client compatibility / docs

- Document extension field meanings
- Provide client examples for basic and extended AgentCard handling
- Clarify required vs optional behavior for generic A2A clients

---

## 8. Open Questions / Decision Points

The team should make explicit decisions on the following:

1. **Credential bridge design**
   - How exactly does a completed SKL auth challenge translate into an A2A request credential?

2. **Public vs extended AgentCard split**
   - Which sovereign details belong in the public card, and which only in authenticated extended cards?

3. **Identifier representation**
   - Where should we prefer `npub` vs raw hex pubkey vs full Nostr event references?

4. **Task/session correlation policy**
   - Should `Task.id` equal the SA session ID directly, or should one wrap the other?

5. **Artifact storage model**
   - Which artifacts are persisted off-Nostr, and how are hashes / URLs linked back into SA audit records?

6. **Funding requirement policy**
   - Are tasks allowed without AC funding, or is OSCE mandatory for certain classes of work?

7. **Partial settlement semantics**
   - How should progressive work, retries, interruptions, and streaming outputs map to AC spends and receipts?

8. **Delegation rollout**
   - Is sub-agent delegation part of the first implementation or explicitly deferred until core interop is stable?

9. **Extension versioning**
   - How will we evolve `nip-skl/v1`, `nip-sa/v1`, and `nip-ac/v1` metadata without breaking clients?

10. **MVP prioritization**
   - Which parts of this plan help the current retained MVP, and which should remain design-ready but unscheduled for now?

---

## 9. Security and Operational Considerations

### 9.1 Transport security

- A2A endpoints must use TLS.
- Standard A2A transport and authentication best practices still apply.
- OpenAgents layering does not weaken A2A security requirements.

### 9.2 Nostr key custody and signing

- The sovereign Nostr key is the trust root for identity and signed event claims.
- Key custody should be treated as security-critical infrastructure.
- Threshold / guardian-backed signing strategies should be considered for higher-risk agents.

### 9.3 Audit trail integrity

- SA audit events provide the durable, signed record of key actions.
- The implementation should define minimum audit emission points for reproducibility.
- A2A logs are useful operationally but are not a substitute for sovereign audit state.

### 9.4 Hold/cancel rollback semantics

- AC hold periods and cancel events are safety mechanisms that complement A2A task cancellation.
- The implementation must keep task state, spend state, and settlement state from drifting apart.
- Cancellation semantics need explicit recovery behavior when task completion and funding state race.

### 9.5 Permission scoping and least privilege

- Permission grants should scope by tool, action, boundary, and duration.
- Guardian approval should be required when work exceeds configured risk boundaries.
- Delegation must never widen the parent scope.

### 9.6 Privacy and public event exposure

- Nostr is not a safe place for raw sensitive payloads.
- Sensitive task content should generally remain off-Nostr.
- Hashes, references, and minimal metadata should be preferred in public events.

### 9.7 Failure and recovery

- The system should tolerate relay outages, delayed reads, and temporary inability to publish.
- Recovery logic must define what happens if:
  - A2A task is accepted but SA session publish fails
  - Task completes but settlement publish fails
  - Spend authorization succeeds but A2A response delivery fails
- Reconciliation jobs may be needed to restore consistency between A2A task state and sovereign state.

---

## 10. Recommended Next Steps

1. **Confirm scope with product and engineering**
   - Decide which phases are immediate engineering work vs design-ready backlog.

2. **Lock the minimal interop profile fields**
   - Freeze initial AgentCard extension URIs, metadata keys, and identifier conventions.

3. **Implement Phase 1 first**
   - Start with `AgentCard.id = nostr:<npub>`, extension declarations, and profile/manifest refs.

4. **Choose the auth bridge strategy**
   - Make an explicit decision on how SKL auth challenge results become A2A request credentials.

5. **Prototype task/session correlation**
   - Build a thin slice where one A2A task creates one SA session and one audit chain.

6. **Decide the first funding mode**
   - Prefer by-reference OSCE envelope handling before inline payload complexity.

7. **Defer delegation unless needed immediately**
   - Keep sub-agent orchestration as a later phase unless a concrete product use case demands it now.

8. **Translate workstreams into tickets**
   - Split the work by server surface, AgentCard generation, Nostr integration, auth/grants, audit/session correlation, and AC settlement.

9. **Document client expectations early**
   - Provide one basic and one extended AgentCard example so client implementers can adopt the profile consistently.

10. **Validate against MVP priorities before scheduling broad work**
   - Preserve the current repo direction by only advancing implementation that clearly supports the retained product path.

---

## Working Summary

The recommended implementation model is simple:

- Speak **A2A** at the edge.
- Keep **SKL / SA / AC** authoritative underneath.
- Project only enough sovereign metadata into A2A to enable discovery, trust, authorization, task correlation, and payment settlement.
- Roll out in phases so an existing A2A agent can bolt on sovereign capabilities without re-architecting the entire system.

That gives the team a path to interoperability without breaking either side of the layering model.