# A2A ↔ OpenAgents Sovereign Stack Integration Plan

**Status:** Planning / design handoff (non-normative)  
**Audience:** Chris, engineering leads, implementers, coding agents  
**Primary layers:** A2A, NIP-SKL, NIP-SA, NIP-AC

---

## 1. Purpose and Scope

This document describes how an **A2A-compliant agent** can incrementally adopt the OpenAgents sovereign stack without changing A2A itself.

- **NIP-SKL** provides skill registry, trust, and optional auth-challenge semantics.
- **NIP-SA** provides sovereign lifecycle, trajectory, guardianship, and delegation semantics.
- **NIP-AC** provides outcome-scoped credit, settlement, and cancel-window semantics.

This is an **implementation-oriented handoff**, not a replacement for the A2A spec or the NIP drafts. Its purpose is to give the team a concrete layering model, interface expectations, and a ticketable rollout path.

### Intended readers

- product/spec reviewers deciding what to schedule,
- engineers implementing server, identity, audit, or payments work,
- coding agents that need a current sequencing source of truth.

### In scope

- how an A2A agent can expose sovereign identity, skills, audit, and payment rails,
- how to map A2A concepts onto SKL / SA / AC responsibilities,
- recommended extension params and metadata shapes,
- a phased rollout plan that preserves the current layering model.

### Out of scope

- rewriting A2A,
- adding new A2A core fields,
- collapsing SKL / SA / AC into one protocol,
- replacing Nostr as the canonical event substrate,
- committing the retained MVP to ship the full interop surface immediately.

### Scope guardrail

This repo is currently MVP-oriented and desktop-first. Treat this document as a **planning and architecture handoff** that should only be implemented where it supports the retained product direction.

### Related document

For the protocol-facing interoperability reference that complements this implementation handoff, see:

- [`docs/A2A_INTEROP_PROFILE.md`](../A2A_INTEROP_PROFILE.md)

---

## 2. A2A Guardrails for This Plan

This plan follows the current A2A wire shape from `/Users/christopherdavid/code/A2A/specification/a2a.proto`.

That implies several hard guardrails:

- `AgentCard` has **no core `id` field**.
- Agent-level extension declarations live in `AgentCard.capabilities.extensions[]`.
- Extension-specific card data belongs in each extension's `params` object.
- Sovereign request/task/artifact correlation belongs in A2A `metadata` fields and standard A2A extension negotiation.
- `supportedInterfaces[]` entries use `protocolBinding` and `protocolVersion`.
- The REST binding string is `HTTP+JSON`.

Current sovereign protocol assumptions for this plan:

- SKL auth challenge is the optional `kind:33410` / `kind:33411` profile.
- SA audit-friendly history is expressed through `kind:39230` / `kind:39231`.
- AC reversibility uses `cancel_until`.
- AC envelopes are `kind:39242`, not `kind:39240`.

When the human-written A2A markdown examples lag the wire schema, follow `a2a.proto`.

---

## 3. Architectural Summary

### Core position

- **A2A remains the wire protocol and interoperability layer.**
- **NIP-SKL remains the canonical skill / trust / optional auth-challenge layer.**
- **NIP-SA remains the canonical lifecycle / trajectory / guardianship / delegation layer.**
- **NIP-AC remains the canonical credit / settlement / cancel-window layer.**

The integration goal is **layering**, not replacement:

1. An agent speaks **A2A** to clients and peer agents.
2. That same agent publishes and verifies canonical sovereign state through **Nostr events**.
3. The A2A server surface projects selected sovereign capabilities into A2A structures such as `AgentCard`, `Task`, `Message`, `Artifact`, and request metadata.

### Canonical sources of truth

| Concern | Canonical layer | A2A role |
| --- | --- | --- |
| Agent identity | NIP-SA agent profile (`kind:39200`) plus profile-defined extension params | Discovery and transport-facing presentation |
| Skill catalog | NIP-SKL manifests (`kind:33400`) and related labels | AgentCard skill projection |
| Authentication bridge | A2A `securitySchemes` plus optional SKL auth challenge | Declared at the A2A edge, verified against sovereign key material |
| Lifecycle / task execution | NIP-SA sessions and trajectory events (`39230` / `39231`) | Task, task updates, and message exchange |
| Delegation / sub-agents | NIP-SA delegation events (`39260`) | Optional orchestration behind A2A tasks |
| Funding / settlement | NIP-AC envelopes, spends, receipts, cancel/default events | Optional payment integration carried alongside A2A task execution |

### Architectural principle

The A2A-facing surface should be treated as a **projection layer** over sovereign state, not as an independent source of truth.

---

## 4. Conceptual Mapping

### 4.1 A2A `AgentCard` -> SA profile + SKL manifests + extension params

Recommended mapping:

- `AgentCard.name` / `description`
  - SHOULD be derived from the current SA profile.
- `AgentCard.skills[]`
  - SHOULD be projected from SKL manifests.
  - Each skill SHOULD remain traceable back to a manifest ref such as `33400:<skill_hex_pubkey>:<d-tag>`.
- `AgentCard.capabilities.extensions[]`
  - SHOULD declare the OpenAgents profile URIs used for SKL / SA / AC participation.
  - SHOULD carry sovereign identity hints in `params`.
- `provider` / `documentationUrl`
  - MAY surface organization or documentation hints, but they are not the sovereign identity anchor.

### 4.2 A2A `Task` -> SA session / trajectory

Recommended mapping:

- A2A `Task.id` SHOULD correlate to the SA session identifier.
- For the first OpenAgents bridge implementation, `Task.id` is the `d` tag of the `kind:39230` session.
- Task status transitions SHOULD map to SA lifecycle state and be mirrored by `kind:39231` events where material.

### 4.3 A2A `Message` / `Part` / `Artifact` -> SA trajectory material

- A2A request messages correspond to SA trajectory inputs and outputs.
- Sovereign refs, hashes, and funding hints SHOULD live in `metadata`, not new A2A core fields.
- Large or sensitive outputs SHOULD stay off Nostr, with only refs or hashes published where needed.

### 4.4 A2A auth surface -> `securitySchemes` + SKL auth challenge + SA guardian checks

- A2A `securitySchemes` remains the public auth declaration surface.
- The optional SKL auth challenge profile can bind that A2A surface to a Nostr key.
- SA guardian policy can gate sensitive operations after caller authentication succeeds.
- Fine-grained authorization beyond this remains implementation-specific unless a later profile standardizes it.

### 4.5 A2A funding expectations -> NIP-AC envelopes and receipts

- A2A does not standardize sovereign payment rails; NIP-AC provides the canonical funding substrate.
- Funding for work SHOULD be represented by OSCE envelope refs or equivalent AC payloads.
- Spend authorization, settlement receipt, default, and cancel behavior remain NIP-AC responsibilities.

---

## 5. Implementation Flows

### 5.1 Discovery and identity verification

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Client fetches `/.well-known/agent-card.json` | A2A |
| 2 | Client reads `supportedInterfaces`, `capabilities.extensions`, and any declared auth schemes | A2A |
| 3 | Client resolves sovereign hints such as `agentProfileRef`, `agentNpub`, `sklManifestRefs`, and `nostrRelays` from extension params | A2A projection of SA/SKL |
| 4 | Client optionally fetches referenced SA profile and SKL manifests from Nostr relays | NIP-SA + NIP-SKL |
| 5 | Client optionally verifies key control via SKL auth challenge / response | NIP-SKL |
| 6 | Client caches the verified A2A endpoint to Nostr identity binding | A2A + NIP-SKL |

Implementation notes:

- Discovery must still work for clients that ignore the OpenAgents extensions.
- Verification via SKL challenge is optional for basic discovery, but recommended for higher-trust use cases.

### 5.2 Authentication and authorization bridge

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Server declares supported `securitySchemes` in the public Agent Card | A2A |
| 2 | Client acquires credentials using the declared scheme | A2A |
| 3 | Client submits A2A request with credential material | A2A |
| 4 | Server optionally binds the caller session to a verified Nostr key using SKL auth challenge state | NIP-SKL |
| 5 | Server evaluates local authorization policy for requested skill, tool, or boundary | Implementation-local |
| 6 | Server evaluates SA guardian policy if the operation is high-risk | NIP-SA |
| 7 | Server accepts or rejects the request and records material decisions in trajectory or audit-friendly sovereign state as needed | NIP-SA |

Implementation notes:

- Treat A2A `securitySchemes` as the discovery surface, not the authoritative policy engine.
- The first interoperable version does not require a portable grant event kind.

### 5.3 Task execution and trajectory correlation

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Client calls `SendMessage` or `SendStreamingMessage` | A2A |
| 2 | Server creates or resumes an SA trajectory session (`kind:39230`) | NIP-SA |
| 3 | Server binds A2A `Task.id` to the SA session identifier | A2A + NIP-SA |
| 4 | Server selects SKL-backed skills that satisfy the request | NIP-SKL |
| 5 | Server performs work and emits `kind:39231` trajectory events for consequential actions and outputs | NIP-SA |
| 6 | Server returns A2A task state and artifacts, optionally with metadata linking back to session, skill refs used, and trajectory refs | A2A projection of SA/SKL |

Implementation notes:

- The A2A task should remain understandable on its own.
- The sovereign session and trajectory chain should remain sufficient to reconstruct what happened.

### 5.4 Funding and settlement

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Client includes an OSCE envelope ref or funding payload in request metadata, message metadata, or an extension-specific data part | A2A carrying NIP-AC |
| 2 | Server resolves and validates the envelope, amount cap, scope, expiry, and issuer / underwriter trust | NIP-AC |
| 3 | Server checks whether guardian approval is required before spend-sensitive execution | NIP-SA |
| 4 | Task executes under the allowed budget and permitted scope | NIP-SA + NIP-AC |
| 5 | If `cancel_until` is present, server honors the cancel window before irreversible delivery or final settlement | NIP-AC |
| 6 | Server emits spend / receipt / cancel / default events as required by outcome | NIP-AC |
| 7 | A2A task concludes with success, failure, interruption, or cancellation, with authoritative funding outcome reflected in AC events | A2A + NIP-AC |

Implementation notes:

- Settlement correctness must never depend solely on A2A response payloads.
- `kind:39242` envelopes and `kind:39244` receipts are the durable payment anchors.

### 5.5 Optional delegation / sub-agent flow

| Step | Description | Primary layer |
| --- | --- | --- |
| 1 | Primary agent decides to delegate part of the task | NIP-SA |
| 2 | Server checks delegation is allowed under local auth policy, guardian policy, and funding scope | Implementation-local + NIP-SA + NIP-AC |
| 3 | Server emits an SA delegation event (`kind:39260`) describing sub-agent, scope, and expiry | NIP-SA |
| 4 | Delegated work may be handled via internal orchestration or another A2A call | NIP-SA + optional A2A |
| 5 | Parent trajectory records the delegation relationship and downstream outputs | NIP-SA |
| 6 | Budget and settlement remain bounded by the parent envelope and delegation scope | NIP-AC |

Implementation notes:

- Delegation is advanced scope.
- It is not required for the first interoperable slice.

---

## 6. Recommended Data Shapes / Interface Contracts

The following are recommended implementation contracts, not normative protocol changes.

### 6.1 Identifier conventions

| Concept | Recommended form | Notes |
| --- | --- | --- |
| SKL manifest ref | `33400:<skill_hex_pubkey>:<d-tag>` | Canonical SKL manifest reference |
| SA profile ref | `39200:<agent_hex_pubkey>:` | Stable replaceable-address form for the current `kind:39200` profile |
| SA session ref | `39230:<agent_hex_pubkey>:<session-d-tag>` | For v1, `<session-d-tag>` is the A2A `Task.id` |
| AC envelope ref | `39242:<issuer_hex_pubkey>:<d-tag>` or event id | `39240` is credit intent, not the envelope |
| Trajectory event ref | event id of `kind:39231` | Use for audit-friendly action refs |
| AC receipt ref | event id of `kind:39244` | Durable settlement reference |

Additional convention:

- Use raw hex pubkeys inside Nostr-style refs.
- Duplicate `npub` strings in extension params only as a human-friendly convenience.

### 6.2 Recommended AgentCard extension URIs

- `https://openagents.org/extensions/nip-skl/v1`
- `https://openagents.org/extensions/nip-sa/v1`
- `https://openagents.org/extensions/nip-ac/v1`

These URIs should be treated as capability hints that the agent speaks A2A while backing selected semantics with sovereign Nostr state.

### 6.3 Recommended extension params

#### SKL extension params

Suggested fields:

- `agentNpub`
- `nostrRelays: string[]`
- `sklManifestRefs: string[]`
- `authChallengeSupported: boolean`
- `assuranceSummary: { [skillRef]: tier }` (optional)

#### SA extension params

Suggested fields:

- `agentProfileRef: string` (`39200:<agent_hex_pubkey>:` in v1)
- `trajectoryAuditAvailable: boolean`
- `delegationSupported: boolean`
- `securityPostureSummary: object`

#### AC extension params

Suggested fields:

- `osceSupported: boolean`
- `acceptedCurrencies: string[]`
- `acceptedUnderwriters: string[]`
- `acceptedSpendRails: string[]`
- `cancelWindowSupported: boolean`
- `osceHandoverMode: "by-reference" | "inline-data-part" | "either"` (v1 should advertise `by-reference` only)

### 6.4 Recommended A2A metadata keys

Suggested server-emitted keys:

- `saSessionId`
- `saSessionRef`
- `sklSkillRefsUsed`
- `guardianApprovalRefs`
- `osceEnvelopeRef`
- `acReceiptRefs`
- `trajectoryEventRefs`

Suggested client-provided keys:

- `osceEnvelopeRef`
- `preferredNostrRelays`
- `requestedProfileUris`

These are profile conventions, not A2A core fields.

For the first bridge implementation, the client-provided and server-emitted key names above should be treated as frozen compatibility keys.

### 6.5 Recommended OSCE handover modes

Preferred options:

1. **By reference**
   - Client passes an AC envelope ref in request or message metadata.
   - Server resolves the envelope from Nostr.
2. **Inline data part**
   - Client passes signed funding payload in a data part or metadata.
   - Server still verifies against canonical AC state.
3. **Hybrid**
   - Client passes both ref and payload for lower round-trip latency.

Recommendation: start with **by reference** for simpler trust boundaries.

---

## 7. Phased Implementation Plan

This sequence is designed so an existing A2A agent can bolt on sovereign capabilities incrementally.

### Phase 1: Discovery and identity binding

**Objectives**

- Publish an A2A Agent Card that binds to sovereign state through extension params.
- Establish stable extension URIs and minimal correlation metadata.

**Dependencies**

- Existing A2A server surface
- SA profile publication path

**Deliverables**

- `capabilities.extensions[]` declarations for SKL / SA
- `agentProfileRef`, `agentNpub`, and relay hints exposed via extension params
- AgentCard generation from SA profile name / description

**Risks / open questions**

- Exact extension param names should be kept stable once clients depend on them.
- Need a clear rule for which identity hints are mandatory vs optional.

### Phase 2: SKL skill projection into AgentCard

**Objectives**

- Make A2A `skills[]` a usable projection of SKL manifests.

**Dependencies**

- Phase 1 identity binding
- SKL manifests available for targeted skills

**Deliverables**

- Skill projection pipeline from `kind:33400` to `AgentCard.skills[]`
- Optional exposure of assurance tiers in SKL extension params
- Clear mapping from `skills[].id` back to manifest refs

**Risks / open questions**

- Need to decide how much SKL detail belongs in the public card vs extended card.
- Revocation semantics must be reflected cleanly in AgentCard refresh behavior.

### Phase 3: Auth bridge and local authorization enforcement

**Objectives**

- Back A2A authentication with a sovereign key-binding path where needed.
- Enforce local authorization policy before expensive work begins.

**Dependencies**

- A2A auth surface for the deployment
- Optional SKL auth challenge support

**Deliverables**

- A2A `securitySchemes` entries for the chosen auth bridge
- Server-side SKL auth challenge verification where Nostr binding matters
- Local authorization middleware for skills, tools, and sensitive boundaries
- Authorization failure mapping into A2A errors

**Risks / open questions**

- Token/session format bridging A2A auth to Nostr proof may need a local profile decision.
- Need to decide whether a future portable grant model is worth standardizing or whether local policy is sufficient.

### Phase 4: SA session and trajectory integration

**Objectives**

- Ensure each A2A task has a corresponding sovereign lifecycle record.

**Dependencies**

- Phase 1 discovery
- Phase 2 skill mapping
- Phase 3 auth bridge

**Deliverables**

- Session creation / resume path for each A2A task
- `Task.id` to `kind:39230` correlation strategy
- `kind:39231` emission for key actions, approvals, and outputs
- Task and artifact metadata linking to session and trajectory refs

**Risks / open questions**

- Need a stable policy for when to emit trajectory entries to avoid noisy or incomplete chains.
- Need to decide whether every streaming chunk is reflected or only material transitions.

### Phase 5: NIP-AC funding and settlement

**Objectives**

- Allow funded A2A work using OSCE envelopes and NIP-AC settlement semantics.

**Dependencies**

- AC envelope / receipt production and verification
- Guardian policy integration for spend-sensitive actions

**Deliverables**

- `kind:39242` envelope resolution / validation path
- Budget enforcement during task execution
- Receipt / cancel / default handling aligned with task outcomes
- `cancel_until` support for reversible consequential spends

**Risks / open questions**

- Need a clear model for partial completion, partial settlement, and interrupted tasks.
- Need policy for whether unfunded tasks are rejected, rate-limited, or allowed in limited mode.

### Phase 6: Extended cards, delegation, and advanced trust metadata

**Objectives**

- Add richer interop only after the base flow is stable.

**Dependencies**

- Phases 1 through 5 complete or sufficiently stable

**Deliverables**

- Extended Agent Card support for authenticated clients
- Delegation / sub-agent metadata and flows
- Richer trust metadata and security posture hints
- Better client guidance on capability tiers

**Risks / open questions**

- Extended card content must avoid leaking sensitive infrastructure details.
- Delegation creates more complex trajectory and budget inheritance logic.

---

## 8. Engineering Work Breakdown

These workstreams should be easy to convert into tickets.

### 8.1 A2A server surface

- Implement or refine HTTP+JSON / JSON-RPC A2A endpoints
- Add support for request metadata and extension activation handling
- Add task / artifact metadata injection hooks for sovereign correlation fields

### 8.2 AgentCard generation

- Generate card from SA profile plus SKL manifests
- Serialize `capabilities.extensions[]` with stable `params` keys
- Support public card and optional extended card variants

### 8.3 Nostr event publishing / retrieval

- Resolve SKL manifests, SA profiles, trajectory refs, and AC envelope refs from relays
- Publish SA sessions, trajectory events, and delegation events as needed
- Add cache and refresh strategies for relay-backed data

### 8.4 Auth challenge verification

- Implement SKL auth challenge request / response flow
- Bind challenge success to the A2A auth/session model where needed
- Define the credential-bridge format used by the deployment

### 8.5 Authorization bridge / policy enforcement

- Add local authorization middleware for tools, boundaries, and sensitive actions
- Evaluate guardian requirements during request authorization
- Map local policy failures into A2A response semantics

### 8.6 Session and trajectory correlation

- Define session id generation / correlation policy
- Emit `kind:39231` for task lifecycle and consequential actions
- Correlate A2A task ids, session ids, and trajectory refs

### 8.7 OSCE handling and settlement logic

- Parse and validate `kind:39242` envelopes
- Track budget during execution
- Emit or verify receipts / cancel / default behavior
- Reconcile A2A task outcomes with AC state

### 8.8 Guardian and delegation policy

- Perform guardian approval checks during request authorization and spend flow
- Enforce delegation scope subset rules
- Validate parent / child session and budget inheritance

### 8.9 Client compatibility / docs

- Document extension param meanings
- Provide client examples for public and extended cards
- Clarify required vs optional behavior for generic A2A clients

---

## 9. Open Questions / Decision Points

The team should make explicit decisions on the following:

The following decisions are already locked for the first bridge slice and are not open questions: `agentProfileRef = 39200:<agent_hex_pubkey>:`, `Task.id == session d-tag`, and initial funding handoff is by-reference `osceEnvelopeRef`.

1. **Credential bridge design**
   - How does the chosen A2A auth surface bind to a verified Nostr identity when SKL auth challenge is used?

2. **Identity anchor shape**
   - Which extension params are required for sovereign identity binding in the first interoperable version?

3. **Profile ref refresh policy**
   - How should clients refresh or invalidate cached profile material while the stable `39200:<agent_hex_pubkey>:` projection remains constant?

4. **Legacy correlation fallback**
   - Do we need any additional metadata beyond `Task.id == session d-tag` for older clients, migrations, or operational debugging?

5. **Artifact storage model**
   - Which artifacts are persisted off Nostr, and how are hashes / URLs linked back into trajectory events?

6. **Funding requirement policy**
   - Are tasks allowed without AC funding, or is OSCE mandatory for certain classes of work?

7. **Partial settlement semantics**
   - How should progressive work, retries, interruptions, and streaming outputs map to AC receipts and defaults?

8. **Delegation rollout**
   - Is sub-agent delegation part of the first implementation or explicitly deferred until core interop is stable?

9. **Extension versioning**
   - How will we evolve the OpenAgents extension URIs and `params` keys without breaking clients?

10. **Future grant model**
   - Do we eventually want a portable authorization-grant profile, or is local policy plus guardian checks sufficient?

11. **MVP prioritization**
   - Which parts of this plan help the retained MVP, and which should remain design-ready but unscheduled?

---

## 10. Security and Operational Considerations

### 10.1 Transport security

- A2A endpoints must use TLS-backed transport.
- Standard A2A auth and transport best practices still apply.
- OpenAgents layering does not weaken A2A security requirements.

### 10.2 Nostr key custody and signing

- The sovereign Nostr key is the trust root for identity and signed event claims.
- Key custody should be treated as security-critical infrastructure.
- Threshold or guardian-backed signing should be considered for higher-risk agents.

### 10.3 Trajectory integrity

- `kind:39230` / `kind:39231` provide the durable signed lifecycle record.
- The implementation should define minimum emission points for reproducibility.
- A2A logs are operationally useful but not a substitute for sovereign state.

### 10.4 Cancel-window semantics

- AC `cancel_until` and A2A task cancellation are complementary, not identical.
- The implementation must keep task state, spend state, and settlement state from drifting apart.
- Recovery behavior must be explicit when task completion and funding state race.

### 10.5 Least privilege

- A2A auth should expose the caller identity clearly enough for policy enforcement.
- Local authorization policy should scope tools, actions, boundaries, and duration.
- Guardian approval should gate work that exceeds configured risk thresholds.
- Delegation must never widen the parent scope.

### 10.6 Privacy and public event exposure

- Nostr is not a safe place for raw sensitive payloads.
- Sensitive task content should generally remain off Nostr.
- Hashes, refs, and minimal metadata should be preferred in public events.

### 10.7 Failure and recovery

- The system should tolerate relay outages, delayed reads, and temporary publish failures.
- Recovery logic must define what happens if:
  - A2A task is accepted but sovereign session publish fails
  - task completes but receipt publish fails
  - receipt succeeds but A2A response delivery fails
- Reconciliation jobs may be needed to restore consistency between A2A-visible task state and sovereign state.

---

## 11. Recommended Next Steps

1. **Confirm scope with product and engineering**
   - Decide which phases are immediate work vs design-ready backlog.

2. **Lock the minimal extension params and metadata keys**
   - Freeze the first set of OpenAgents extension URIs, `params` keys, and task metadata keys.

3. **Implement Phase 1 first**
   - Start with extension-based identity hints, profile refs, and manifest refs.

4. **Choose the auth bridge strategy**
   - Make an explicit decision on how A2A auth binds to verified Nostr identity.

5. **Prototype direct task/session correlation**
   - Build a thin slice where one A2A task creates one SA session, uses the session `d` tag as `Task.id`, and emits one trajectory chain.

6. **Keep the first funding mode by-reference**
   - Prefer `osceEnvelopeRef` handoff before inline payload complexity.

7. **Defer delegation unless needed immediately**
   - Keep sub-agent orchestration as a later phase unless a concrete product use case demands it now.

8. **Translate workstreams into tickets**
   - Split the work by server surface, card generation, Nostr integration, auth bridge, trajectory correlation, and AC settlement.

9. **Document client expectations early**
   - Provide one public and one extended Agent Card example built on the current A2A card shape.

10. **Validate against MVP priorities before scheduling broad work**
   - Preserve the current repo direction by only advancing implementation that clearly supports the retained product path.

---

## Working Summary

The recommended implementation model is:

- speak **A2A** at the edge,
- keep **SKL / SA / AC** authoritative underneath,
- project sovereign identity, trust, lifecycle, and funding state into A2A through **extension params**, **metadata**, and standard A2A auth surfaces,
- roll out in phases so an existing A2A agent can bolt on sovereign capabilities without re-architecting the whole system.

That gives the team a path to interoperability without breaking either side of the layering model.
