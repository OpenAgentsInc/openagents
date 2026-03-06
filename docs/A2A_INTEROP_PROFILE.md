# A2A Interoperability Profile for Sovereign Agents

**Status:** Non-normative, ecosystem-specific profile layered on A2A / NIP-SKL / NIP-SA / NIP-AC.

This document describes how an **A2A-compliant agent** can interoperably layer in the OpenAgents sovereign stack:

- **NIP-SKL** — skill registry, identity, attestation, revocation, auth challenge, permission grants
- **NIP-SA** — sovereign agent identity, guardianship, lifecycle, sessions/trajectories, audit, delegation
- **NIP-AC** — outcome-scoped credit (OSCE), spend authorization, settlement, hold/cancel behavior

The profile is **optional** and **non-normative**:

- It does **not** modify the A2A protocol.
- It does **not** change the canonical semantics of NIP-SKL, NIP-SA, or NIP-AC.
- It describes a common interoperability pattern so A2A agents can expose sovereign identity, trust, lifecycle, and payment rails while preserving existing protocol boundaries.

---

## 1. Scope and Purpose

This profile exists to define a practical mapping between:

- **A2A** as the wire protocol, discovery mechanism, and orchestration surface, and
- **NIP-SKL / NIP-SA / NIP-AC** as the canonical identity, lifecycle, and credit layers.

Under this profile:

- A2A remains the **transport and interoperability layer**.
- NIP-SKL remains the **skill / identity / trust layer**.
- NIP-SA remains the **lifecycle / guardianship / audit / delegation layer**.
- NIP-AC remains the **credit / settlement / rollback layer**.

The goal is not to replace A2A or restate the NIP specifications. The goal is to let existing and future A2A agents **bolt on sovereign capabilities incrementally** while keeping canonical state on Nostr.

### 1.1 What this profile is for

This profile is intended for:

- protocol and spec readers who need a clean interoperability model,
- engineering implementers building A2A-facing sovereign agents,
- future coding agents working from repository docs.

### 1.2 What this profile is not

This profile does **not**:

- redefine A2A operations, data model, or transport rules,
- redefine NIP-SKL, NIP-SA, or NIP-AC wire semantics,
- merge SKL / SA / AC into one layer,
- require all A2A agents to adopt the OpenAgents sovereign stack.

### Related document

For the implementation-oriented planning and sequencing handoff that complements this profile, see:

- [`docs/plans/a2a-sovereign-agent-integration-plan.md`](plans/a2a-sovereign-agent-integration-plan.md)

---

## 2. Conceptual Mapping

### 2.1 AgentCard → SA Agent Profile + SKL manifests + optional org identity

An A2A **AgentCard** can act as the public discovery surface for a sovereign agent.

- `AgentCard.id`
  - SHOULD bind to the sovereign Nostr identity.
  - Recommended format: `nostr:<npub>`.
- `AgentCard.name` and `AgentCard.description`
  - SHOULD be derived from the SA Agent Profile (`kind:39200`).
- `AgentCard.skills[]`
  - SHOULD be projected from SKL manifests (`kind:33400`).
  - Each declared skill SHOULD remain traceable back to a SKL manifest reference such as `33400:<pubkey>:<d-tag>`.
- Optional NIP-05 / organizational identity
  - MAY be surfaced through metadata or extension metadata when the agent also publishes an optional SKL NIP-05 identity binding.
- `extensions[]`
  - MAY declare OpenAgents interoperability URIs indicating SKL / SA / AC participation.

In this model, the AgentCard is a **projection**, not the authoritative identity record.

### 2.2 Task → SA session / trajectory

An A2A **Task** maps naturally to an SA session / trajectory.

- `Task.id` SHOULD correlate to an SA `session` identifier.
- New tasks SHOULD create a new SA trajectory session (`kind:39230`).
- Multi-turn A2A exchanges MAY continue an existing SA session when the underlying work is part of the same lifecycle.
- Task status changes SHOULD correspond to SA session state transitions and related audit records.

### 2.3 Message / Part / Artifact → SA ticks + SKL-backed capabilities + Nostr references

- A2A `Message` content maps to SA tick inputs and outputs.
- A2A `Part` content can carry user input, structured data, or references to externally stored payloads that are processed through SKL-backed capabilities.
- A2A `Artifact` objects can represent outputs that are also hashed, referenced, or summarized in SA trajectory and audit events.
- SKL manifests remain the canonical description of capabilities; A2A message exchange is the execution surface.

### 2.4 securitySchemes / authorization behavior → SKL auth challenge + grants + SA guardians

Under this profile, A2A `securitySchemes` and authorization behavior can be backed by:

- SKL **authentication challenge / response** (`kind:33410` / `kind:33411`) to prove control of a Nostr key,
- SKL **permission grants** (`kind:33420`) to scope allowed tools, actions, data, expiry, and invocation limits,
- SA **guardian policies** and `security_posture` to require additional approval for higher-risk operations.

In other words:

- A2A declares the auth surface.
- SKL and SA provide the sovereign authorization substrate.

### 2.5 Funding / metering expectations → NIP-AC OSCE and settlement chain

A2A does not define a sovereign credit or settlement model. Under this profile:

- NIP-AC **OSCE envelopes** fund or constrain work.
- AC **spend authorizations**, **settlement receipts**, and **cancel behavior** remain canonical.
- A2A messages or task metadata may carry AC references or payloads, but the authoritative payment record remains the AC event chain.

---

## 3. AgentCard Examples

The following examples are illustrative, non-normative, and intended to help implementers converge on compatible patterns.

### 3.1 Example 1: Basic Sovereign Agent A2A card

```json
{
  "id": "nostr:npub1researchagent7m3zq0example0000000000000000000000000",
  "name": "Research Assistant Agent",
  "description": "Sovereign research agent that exposes A2A interfaces while publishing canonical identity and skill state on Nostr.",
  "version": "1.0.0",
  "documentationUrl": "https://example.com/docs/research-agent",
  "supportedInterfaces": [
    {
      "url": "https://agent.example.com/a2a/v1",
      "protocolBinding": "HTTP_JSON",
      "version": "1.0"
    }
  ],
  "capabilities": {
    "streaming": true,
    "pushNotifications": false,
    "extendedAgentCard": false
  },
  "skills": [
    {
      "id": "33400:npub1researchagent7m3zq0example0000000000000000000000000:web-search-v1",
      "name": "Web Search",
      "description": "Searches public web sources and returns ranked summaries.",
      "tags": ["research", "search", "web"]
    },
    {
      "id": "33400:npub1researchagent7m3zq0example0000000000000000000000000:citation-format-v1",
      "name": "Citation Formatting",
      "description": "Formats references into common academic citation styles.",
      "tags": ["citation", "formatting", "academic"]
    }
  ],
  "extensions": [
    {
      "uri": "https://openagents.org/extensions/nip-skl/v1",
      "required": false,
      "metadata": {
        "skl_manifest_refs": [
          "33400:npub1researchagent7m3zq0example0000000000000000000000000:web-search-v1",
          "33400:npub1researchagent7m3zq0example0000000000000000000000000:citation-format-v1"
        ]
      }
    },
    {
      "uri": "https://openagents.org/extensions/nip-sa/v1",
      "required": false,
      "metadata": {
        "agent_profile_ref": "39200:npub1researchagent7m3zq0example0000000000000000000000000:default-profile",
        "audit_trail_available": true
      }
    },
    {
      "uri": "https://openagents.org/extensions/nip-ac/v1",
      "required": false,
      "metadata": {
        "osce_supported": false
      }
    }
  ]
}
```

### 3.2 Example 2: Extended AgentCard with payments and trust

```json
{
  "id": "nostr:npub1researchagent7m3zq0example0000000000000000000000000",
  "name": "Research Assistant Agent",
  "description": "Sovereign research agent with evaluated skills, guardian controls, and OSCE-funded task execution.",
  "version": "1.1.0",
  "documentationUrl": "https://example.com/docs/research-agent",
  "supportedInterfaces": [
    {
      "url": "https://agent.example.com/a2a/v1",
      "protocolBinding": "HTTP_JSON",
      "version": "1.0"
    }
  ],
  "capabilities": {
    "streaming": true,
    "pushNotifications": true,
    "extendedAgentCard": true
  },
  "skills": [
    {
      "id": "33400:npub1researchagent7m3zq0example0000000000000000000000000:web-search-v2",
      "name": "Web Search",
      "description": "Searches public web sources and returns ranked summaries.",
      "tags": ["research", "search", "web"],
      "metadata": {
        "assurance_tier": "third-party-evaluated",
        "evaluator_pubkey": "npub1evaluator000000000000000000000000000000000000000000000",
        "attestation_event_id": "attestation-event-001"
      }
    },
    {
      "id": "33400:npub1researchagent7m3zq0example0000000000000000000000000:citation-format-v2",
      "name": "Citation Formatting",
      "description": "Formats references into common academic citation styles.",
      "tags": ["citation", "formatting", "academic"],
      "metadata": {
        "assurance_tier": "red-team-tested",
        "evaluator_pubkey": "npub1redteam00000000000000000000000000000000000000000000000",
        "attestation_event_id": "attestation-event-002"
      }
    },
    {
      "id": "33400:npub1researchagent7m3zq0example0000000000000000000000000:summarize-v1",
      "name": "Summarization",
      "description": "Produces concise summaries from supplied material.",
      "tags": ["summary", "analysis"],
      "metadata": {
        "assurance_tier": "self-assessed"
      }
    }
  ],
  "securitySchemes": {
    "nostrChallenge": {
      "type": "http",
      "scheme": "bearer",
      "bearerFormat": "Nostr-SKL-Challenge",
      "description": "Bearer credential derived from a successful SKL auth challenge / response flow and bound to the caller's authorized session."
    }
  },
  "security": [
    {
      "nostrChallenge": []
    }
  ],
  "extensions": [
    {
      "uri": "https://openagents.org/extensions/nip-skl/v1",
      "required": false,
      "metadata": {
        "nostr_relays": [
          "wss://relay.example.com",
          "wss://relay2.example.com"
        ],
        "skl_manifest_refs": [
          "33400:npub1researchagent7m3zq0example0000000000000000000000000:web-search-v2",
          "33400:npub1researchagent7m3zq0example0000000000000000000000000:citation-format-v2",
          "33400:npub1researchagent7m3zq0example0000000000000000000000000:summarize-v1"
        ],
        "auth_challenge_supported": true,
        "permission_grants_required": true
      }
    },
    {
      "uri": "https://openagents.org/extensions/nip-sa/v1",
      "required": false,
      "metadata": {
        "agent_profile_ref": "39200:npub1researchagent7m3zq0example0000000000000000000000000:default-profile",
        "guardian_threshold": 2,
        "audit_trail_available": true,
        "delegation_supported": true,
        "security_posture_summary": {
          "instruction_data_separation": true,
          "tool_use_requires_guardian": true,
          "hijacking_resistance_tier": "third-party-evaluated"
        }
      }
    },
    {
      "uri": "https://openagents.org/extensions/nip-ac/v1",
      "required": false,
      "metadata": {
        "osce_supported": true,
        "accepted_currencies": ["sat"],
        "accepted_underwriters": [
          "npub1underwriter0000000000000000000000000000000000000000000"
        ],
        "hold_period_secs_default": 3600,
        "osce_handover_mode": "by-reference"
      }
    }
  ],
  "metadata": {
    "nip05": "research-agent@example.com"
  }
}
```

---

## 4. Interoperability Flows

### 4.1 Flow A — discovery and identity verification

1. The client discovers the agent through A2A by fetching `/.well-known/agent-card.json`.
2. The client reads `AgentCard.id` and sees a sovereign identifier such as `nostr:npub1...`.
3. The client reads `extensions[]` to determine whether the agent advertises SKL / SA / AC participation and to learn relay or manifest hints.
4. If the card declares a Nostr- or SKL-backed `securitySchemes` entry, the client treats that as the advertised authentication surface for authenticated interactions.
5. To verify the server's sovereign identity, the client may issue an SKL auth challenge (`kind:33410`) to the claimed pubkey.
6. The agent responds with a signed SKL auth response (`kind:33411`) proving control of the corresponding Nostr key.
7. The client verifies the challenge, response, nonce, expiry, and signer identity.
8. After verification, the client can treat the A2A endpoint and the Nostr identity as bound for this interoperability profile.

Under this flow:

- **A2A** handles discovery and advertised auth surface.
- **NIP-SKL** handles sovereign proof-of-possession.
- **NIP-SA** remains the canonical identity profile layer.

### 4.2 Flow B — task execution mapped to SA session + SKL skills

1. The client sends `SendMessage` or `SendStreamingMessage` to the A2A endpoint.
2. The server authenticates the caller using the declared A2A auth surface and validates the underlying SKL-backed authorization state.
3. The server creates or resumes an SA session / trajectory for the task.
4. The server correlates the A2A `Task.id` with the SA `session` identifier.
5. The server selects one or more SKL-backed skills that satisfy the request.
6. The server executes the work as SA ticks / actions.
7. The server emits SA audit events (`kind:39250`) for key actions, inputs, outputs, approvals, and references.
8. The server returns A2A task status and artifacts, optionally including metadata that points back to the SA session, SKL skill refs used, and audit refs.

Under this flow:

- **A2A** handles task transport, streaming, and artifact delivery.
- **NIP-SKL** identifies and constrains which skills can be used.
- **NIP-SA** owns lifecycle, session, audit, and guardianship state.

### 4.3 Flow C — funding and settlement via NIP-AC

1. The client presents or references an OSCE envelope before or during task initiation.
2. The server resolves and validates the AC funding record, including:
   - allowed skill scope,
   - budget / maximum amount,
   - underwriter identity,
   - expiry,
   - any guardian approval requirements.
3. The server verifies that the requested SKL-backed actions fit within the permitted funding scope.
4. The server executes the task under those budget and authorization constraints.
5. If the envelope includes hold / reversibility semantics, the server enforces the configured `hold_period_secs` behavior before final settlement.
6. Successful completion maps to AC spend authorization and settlement receipt behavior.
7. Interruption, cancellation, or dispute within the allowed hold window may map to AC cancel behavior.
8. The A2A task outcome is returned to the client, but the authoritative funding and settlement state remains in NIP-AC events.

Under this flow:

- **A2A** carries the task request/response interaction.
- **NIP-AC** remains authoritative for funding, spend, settlement, and rollback semantics.
- **NIP-SA** may require guardian approval before certain funded actions proceed.

---

## 5. Recommended Metadata and Identifier Conventions

The following conventions are recommended for interoperability and are safe to ignore by generic A2A clients that do not implement this profile.

### 5.1 Identifier conventions

- Sovereign agent id:
  - `nostr:<npub>`
- SKL manifest reference:
  - `33400:<pubkey>:<d-tag>`
- SA profile reference:
  - `39200:<pubkey>:<d-tag>`
- SA session reference:
  - implementation-defined session id, optionally exposed directly as `Task.id`
- AC envelope reference:
  - event id or addressable ref such as `39240:<pubkey>:<d-tag>`
- Audit reference:
  - event id or addressable ref for `kind:39250`

### 5.2 Recommended task correlation fields

Implementations MAY include the following fields in task metadata, artifact metadata, or extension-specific metadata:

- `sa_session_id`
- `sa_session_ref`
- `skl_skill_refs_used`
- `permission_grant_refs`
- `guardian_approval_refs`
- `osce_envelope_ref`
- `ac_spend_refs`
- `audit_refs`

These fields make it easier for interoperable clients to correlate A2A-visible behavior with sovereign state.

### 5.3 Recommended extension metadata

#### SKL extension metadata

Suggested fields:

- `nostr_relays`
- `skl_manifest_refs`
- `auth_challenge_supported`
- `permission_grants_required`

#### SA extension metadata

Suggested fields:

- `agent_profile_ref`
- `guardian_threshold`
- `audit_trail_available`
- `delegation_supported`
- `security_posture_summary`

#### AC extension metadata

Suggested fields:

- `osce_supported`
- `accepted_currencies`
- `accepted_underwriters`
- `hold_period_secs_default`
- `osce_handover_mode`

### 5.4 Recommended OSCE handover modes

Recommended values for `osce_handover_mode`:

- `by-reference`
- `data-part`
- `either`

Recommended starting point:

- Prefer **by-reference** first, where the A2A request carries an AC envelope reference and the server resolves canonical state from Nostr.

---

## 6. Security Considerations

### 6.1 TLS and A2A transport security

- Standard A2A transport security remains required.
- HTTPS or other TLS-backed transport is still mandatory for production deployments.
- This profile does not weaken A2A transport requirements.

### 6.2 Nostr key custody and trust

- The Nostr key used by the sovereign agent is the root of trust for SKL / SA / AC identity and signed event claims.
- Implementers should treat key custody as security-critical.
- Threshold or guardian-backed signing arrangements may be appropriate for higher-risk agents.

### 6.3 Audit trails complement A2A logging

- A2A logs are useful operationally, but SA audit events are the canonical signed record of sensitive or material actions.
- Implementers should define minimum audit emission rules so task execution can be reconstructed with integrity.

### 6.4 Hold/cancel rollback semantics complement A2A cancellation

- A2A task cancellation and AC hold/cancel behavior are related but not identical.
- Task interruption does not by itself define the canonical settlement result.
- Implementers should ensure task state and AC state cannot silently diverge.

### 6.5 Least privilege through grants and guardianship

- SKL permission grants should be used to scope skills, tools, actions, boundaries, duration, and invocation counts.
- SA guardianship should be used where autonomous execution exceeds local risk thresholds.
- Delegation should never widen the parent authorization or funding scope.

### 6.6 Privacy and public Nostr events

- Public Nostr events are not appropriate for raw sensitive task payloads.
- Sensitive content should generally stay off-Nostr.
- On-Nostr references should prefer hashes, identifiers, and minimal metadata rather than full confidential payloads.

---

## 7. Non-Goals

This interoperability profile does **not**:

- modify the A2A protocol,
- replace NIP-SKL, NIP-SA, or NIP-AC,
- collapse SKL / SA / AC into one layer,
- require every A2A agent to adopt the OpenAgents sovereign stack,
- require every sovereign agent to expose an A2A interface.

Its purpose is narrower:

- to define a stable and practical pattern for agents that want to combine **A2A interoperability** with **sovereign Nostr-backed identity, lifecycle, audit, and payment semantics**.

---

## 8. Summary

Under this profile:

- A2A is the **wire layer**.
- NIP-SKL is the **identity / trust / skills layer**.
- NIP-SA is the **lifecycle / guardianship / audit / delegation layer**.
- NIP-AC is the **credit / settlement / rollback layer**.

An agent that follows this profile can remain fully A2A-compliant while also exposing a sovereign identity, verifiable skill inventory, explicit permission model, auditable lifecycle, and outcome-scoped payment rails.

That is the intended interoperability result.