# A2A Interoperability Profile for Sovereign Agents

**Status:** Non-normative, ecosystem-specific profile layered on A2A / NIP-SKL / NIP-SA / NIP-AC.

This document describes a practical way for an **A2A-compliant agent** to expose the OpenAgents sovereign stack without changing A2A itself.

- **A2A** remains the wire protocol, discovery surface, and task orchestration layer.
- **NIP-SKL** remains the skill registry, trust, and optional proof-of-possession layer.
- **NIP-SA** remains the lifecycle, trajectory, guardianship, and delegation layer.
- **NIP-AC** remains the outcome-scoped credit, settlement, and cancel-window layer.

This profile is **optional**:

- It does **not** modify A2A.
- It does **not** redefine NIP-SKL, NIP-SA, or NIP-AC.
- It describes a compatible layering pattern for agents that want both **A2A interoperability** and **Nostr-backed sovereign state**.

---

## 1. Scope and Purpose

This profile exists to define a practical mapping between:

- **A2A** as the discovery, transport, and task protocol, and
- **NIP-SKL / NIP-SA / NIP-AC** as the canonical skill, lifecycle, and funding layers.

The goal is not to replace A2A or restate the NIP drafts. The goal is to let existing and future A2A agents **add sovereign identity, audit, and payment semantics incrementally** while keeping canonical state on Nostr.

### 1.1 What this profile is for

This profile is intended for:

- protocol readers who need a clean A2A-to-sovereign layering model,
- implementers building A2A-facing sovereign agents,
- future coding agents working from repo docs.

### 1.2 What this profile is not

This profile does **not**:

- add new core A2A fields,
- redefine A2A transport bindings or RPC names,
- redefine NIP-SKL, NIP-SA, or NIP-AC wire semantics,
- require every A2A agent to adopt the OpenAgents sovereign stack.

### Related document

For the implementation-oriented planning handoff that complements this profile, see:

- [`docs/plans/a2a-sovereign-agent-integration-plan.md`](plans/a2a-sovereign-agent-integration-plan.md)

---

## 2. Current A2A Alignment Notes

This profile assumes the current A2A wire shape from `specification/a2a.proto` in `/Users/christopherdavid/code/A2A`.

The important constraints are:

- `AgentCard` has **no core `id` field**. Sovereign identity therefore has to be projected through profile-defined extension params rather than a new top-level field.
- Agent-level extensions are declared in `AgentCard.capabilities.extensions[]`, not at the top level of the card.
- Extension-specific card data belongs in each extension's `params` object.
- Request/task/artifact correlation data belongs in A2A `metadata` fields and request-time extension negotiation, not in new core fields.
- `supportedInterfaces[]` entries use `protocolBinding` and `protocolVersion`.
- The standard binding string for the REST binding is `HTTP+JSON`.

Under this profile:

- card-level sovereign declarations live in `capabilities.extensions[].params`,
- request-level sovereign declarations live in `SendMessageRequest.metadata`, `Message.metadata`, `Task.metadata`, `TaskStatusUpdateEvent.metadata`, and `Artifact.metadata`,
- request-time extension behavior uses normal A2A extension negotiation.

---

## 3. Conceptual Mapping

### 3.1 AgentCard -> SA profile + SKL manifests + extension params

An A2A `AgentCard` can act as the public discovery surface for a sovereign agent.

- `AgentCard.name` and `AgentCard.description`
  - SHOULD be derived from the current SA agent profile (`kind:39200`).
- `AgentCard.skills[]`
  - SHOULD be projected from SKL manifests (`kind:33400`).
  - Each projected skill SHOULD remain traceable back to a SKL manifest reference such as `33400:<skill_hex_pubkey>:<d-tag>`.
- `AgentCard.capabilities.extensions[]`
  - SHOULD declare the OpenAgents profile URIs used to expose SKL / SA / AC participation.
  - SHOULD carry sovereign hints in each extension's `params` object.
- Optional organization or NIP-05 identity
  - MAY be surfaced through `provider`, `documentationUrl`, or extension params.

Important boundary:

- A2A discovery starts from the Agent Card URL.
- Sovereign identity binding comes from extension params plus optional SKL auth challenge verification.

### 3.2 Task -> SA session / trajectory

An A2A `Task` maps naturally to an SA trajectory run.

- `Task.id` SHOULD correlate to the SA session identifier.
- For the OpenAgents v1 bridge profile, `Task.id` is the `d` tag of the corresponding `kind:39230` session.
- Task status transitions SHOULD correspond to SA lifecycle transitions and related `kind:39231` trajectory events.
- `Task.metadata` and task update metadata SHOULD carry the sovereign refs needed for correlation.

### 3.3 Message / Part / Artifact -> SA trajectory events + off-Nostr payload refs

- A2A `Message` and `Part` content maps to SA trajectory inputs and outputs.
- A2A `Message.metadata` and `Part.metadata` may carry sovereign refs, hashes, or extension-specific funding metadata.
- A2A `Artifact` objects can represent outputs that are also hashed, referenced, or summarized in SA trajectory events.
- Large or sensitive payloads SHOULD stay off Nostr, with only hashes or minimal refs published where needed.

### 3.4 Authentication / authorization -> A2A auth surface + SKL auth challenge + SA guardians

Under this profile:

- A2A `securitySchemes` remains the declared authentication surface.
- The optional SKL auth challenge profile (`kind:33410` / `kind:33411`) can be used to prove current control of the claimed Nostr key.
- SA guardian policy can be used to gate high-risk operations once the caller is authenticated.

Current boundary:

- This profile does **not** define a portable permission-grant event kind.
- Fine-grained authorization beyond A2A auth plus guardian policy remains implementation-specific unless a later profile standardizes it.

### 3.5 Funding / metering -> NIP-AC OSCE and settlement chain

A2A does not define a sovereign settlement model. Under this profile:

- NIP-AC outcome-scoped credit envelopes fund or constrain work.
- A2A requests, message metadata, or parts may carry envelope refs or funding payloads.
- NIP-AC remains authoritative for spend, settlement, receipt, default, and cancel-window semantics.

---

## 4. AgentCard Examples

The following examples are illustrative and intentionally follow the current A2A card shape:

- sovereign hints live in `capabilities.extensions[].params`,
- transport entries use `protocolBinding` and `protocolVersion`,
- the REST binding is `HTTP+JSON`.

### 4.1 Example 1: Public card with sovereign identity hints

```json
{
  "name": "Research Assistant Agent",
  "description": "A2A research agent that publishes canonical sovereign state on Nostr.",
  "supportedInterfaces": [
    {
      "url": "https://agent.example.com/a2a/v1",
      "protocolBinding": "HTTP+JSON",
      "protocolVersion": "1.0"
    }
  ],
  "version": "1.0.0",
  "documentationUrl": "https://example.com/docs/research-agent",
  "capabilities": {
    "streaming": true,
    "pushNotifications": false,
    "extendedAgentCard": false,
    "extensions": [
      {
        "uri": "https://openagents.org/extensions/nip-skl/v1",
        "description": "Publishes SKL manifests and supports optional SKL auth challenge verification.",
        "required": false,
        "params": {
          "agentNpub": "npub1researchagentexample",
          "nostrRelays": [
            "wss://relay.example.com",
            "wss://relay2.example.com"
          ],
          "sklManifestRefs": [
            "33400:<agent_hex_pubkey>:web-search-v1",
            "33400:<agent_hex_pubkey>:citation-format-v1"
          ],
          "authChallengeSupported": true
        }
      },
      {
        "uri": "https://openagents.org/extensions/nip-sa/v1",
        "description": "Projects sovereign profile and trajectory correlation hints into A2A.",
        "required": false,
        "params": {
          "agentProfileRef": "39200:<agent_hex_pubkey>:",
          "trajectoryAuditAvailable": true
        }
      },
      {
        "uri": "https://openagents.org/extensions/nip-ac/v1",
        "description": "Exposes optional OSCE funding support for A2A tasks.",
        "required": false,
        "params": {
          "osceSupported": false
        }
      }
    ]
  },
  "defaultInputModes": [
    "text/plain"
  ],
  "defaultOutputModes": [
    "text/plain",
    "application/json"
  ],
  "skills": [
    {
      "id": "33400:<agent_hex_pubkey>:web-search-v1",
      "name": "Web Search",
      "description": "Searches public web sources and returns ranked summaries.",
      "tags": [
        "research",
        "search",
        "web"
      ],
      "examples": [
        "Find primary sources about Lightning Network fee markets."
      ]
    },
    {
      "id": "33400:<agent_hex_pubkey>:citation-format-v1",
      "name": "Citation Formatting",
      "description": "Formats references into common academic citation styles.",
      "tags": [
        "citation",
        "formatting",
        "academic"
      ]
    }
  ]
}
```

### 4.2 Example 2: Extended card with auth, trust, and funding hints

```json
{
  "name": "Research Assistant Agent",
  "description": "A2A sovereign agent with evaluated skills, guardian-aware execution, and OSCE-funded task support.",
  "supportedInterfaces": [
    {
      "url": "https://agent.example.com/a2a/v1",
      "protocolBinding": "HTTP+JSON",
      "protocolVersion": "1.0"
    },
    {
      "url": "https://agent.example.com/a2a/jsonrpc",
      "protocolBinding": "JSONRPC",
      "protocolVersion": "1.0"
    }
  ],
  "version": "1.1.0",
  "documentationUrl": "https://example.com/docs/research-agent",
  "capabilities": {
    "streaming": true,
    "pushNotifications": true,
    "extendedAgentCard": true,
    "extensions": [
      {
        "uri": "https://openagents.org/extensions/nip-skl/v1",
        "description": "Publishes SKL manifests, assurance hints, and optional SKL auth challenge support.",
        "required": false,
        "params": {
          "agentNpub": "npub1researchagentexample",
          "nostrRelays": [
            "wss://relay.example.com",
            "wss://relay2.example.com"
          ],
          "sklManifestRefs": [
            "33400:<agent_hex_pubkey>:web-search-v2",
            "33400:<agent_hex_pubkey>:citation-format-v2",
            "33400:<agent_hex_pubkey>:summarize-v1"
          ],
          "authChallengeSupported": true,
          "assuranceSummary": {
            "33400:<agent_hex_pubkey>:web-search-v2": "third-party-evaluated",
            "33400:<agent_hex_pubkey>:citation-format-v2": "red-team-tested",
            "33400:<agent_hex_pubkey>:summarize-v1": "self-assessed"
          }
        }
      },
      {
        "uri": "https://openagents.org/extensions/nip-sa/v1",
        "description": "Projects sovereign profile, trajectory audit hints, delegation support, and security posture summary.",
        "required": false,
        "params": {
          "agentProfileRef": "39200:<agent_hex_pubkey>:",
          "trajectoryAuditAvailable": true,
          "delegationSupported": true,
          "securityPostureSummary": {
            "instructionDataSeparation": true,
            "toolUseRequiresGuardian": true,
            "hijackingResistanceTier": "third-party-evaluated"
          }
        }
      },
      {
        "uri": "https://openagents.org/extensions/nip-ac/v1",
        "description": "Supports OSCE-funded task execution and NIP-AC receipt correlation.",
        "required": false,
        "params": {
          "osceSupported": true,
          "acceptedCurrencies": [
            "sat"
          ],
          "acceptedUnderwriters": [
            "<underwriter_hex_pubkey>"
          ],
          "acceptedSpendRails": [
            "lightning"
          ],
          "cancelWindowSupported": true,
          "osceHandoverMode": "by-reference"
        }
      }
    ]
  },
  "securitySchemes": {
    "nostrChallenge": {
      "httpAuthSecurityScheme": {
        "scheme": "Bearer",
        "bearerFormat": "OpenAgents-SKL-Challenge-Session",
        "description": "Bearer credential bound to a completed SKL auth challenge and the caller's authenticated session."
      }
    }
  },
  "defaultInputModes": [
    "text/plain",
    "application/json"
  ],
  "defaultOutputModes": [
    "text/plain",
    "application/json"
  ],
  "skills": [
    {
      "id": "33400:<agent_hex_pubkey>:web-search-v2",
      "name": "Web Search",
      "description": "Searches public web sources and returns ranked summaries.",
      "tags": [
        "research",
        "search",
        "web"
      ]
    },
    {
      "id": "33400:<agent_hex_pubkey>:citation-format-v2",
      "name": "Citation Formatting",
      "description": "Formats references into common academic citation styles.",
      "tags": [
        "citation",
        "formatting",
        "academic"
      ]
    },
    {
      "id": "33400:<agent_hex_pubkey>:summarize-v1",
      "name": "Summarization",
      "description": "Produces concise summaries from supplied material.",
      "tags": [
        "summary",
        "analysis"
      ]
    }
  ]
}
```

Notes:

- If the agent requires authenticated access, the corresponding A2A security requirement should reference the declared `nostrChallenge` scheme using the current A2A wire shape.
- A2A card signatures and SKL auth challenge verification solve different problems. JWS card signatures cover the card document; SKL auth challenge proves current control of the sovereign Nostr key.

---

## 5. Interoperability Flows

### 5.1 Flow A: discovery and sovereign identity verification

1. The client fetches `/.well-known/agent-card.json`.
2. The client reads `supportedInterfaces`, `capabilities.extensions`, and any declared `securitySchemes`.
3. The client resolves sovereign hints from extension params, especially:
   - `agentProfileRef`
   - `agentNpub`
   - `sklManifestRefs`
   - `nostrRelays`
4. The client may fetch the referenced SA profile and SKL manifests from Nostr relays.
5. If higher confidence is needed, the client issues an SKL auth challenge (`kind:33410`) to the claimed pubkey.
6. The agent responds with a signed SKL auth response (`kind:33411`).
7. The client verifies the challenge, nonce, expiry window, and signer identity.
8. The client can now treat the A2A endpoint and the verified Nostr key as bound for this interoperability profile.

Under this flow:

- **A2A** handles discovery and endpoint advertisement.
- **NIP-SKL** handles optional proof-of-possession verification.
- **NIP-SA** provides the sovereign profile being projected.

### 5.2 Flow B: task execution mapped to SA trajectory

1. The client calls `SendMessage` or `SendStreamingMessage`.
2. The server authenticates the caller using the declared A2A auth surface.
3. If the request depends on extension behavior, the client and server use standard A2A extension negotiation for the relevant URIs.
4. The server creates or resumes an SA trajectory session (`kind:39230`).
5. The server uses the SA session `d` tag as the A2A `Task.id` and emits `saSessionRef = 39230:<agent_hex_pubkey>:<task_id>` in task metadata.
6. The server selects one or more SKL-backed skills that satisfy the request.
7. The server emits `kind:39231` trajectory events for consequential actions, approvals, and outputs.
8. The server returns A2A task state and artifacts, optionally including metadata refs back to the session, skill refs used, and relevant trajectory events.

Under this flow:

- **A2A** handles task transport, streaming, task updates, and artifact delivery.
- **NIP-SKL** identifies the skill manifests backing the work.
- **NIP-SA** owns lifecycle, trajectory, guardianship, and audit-friendly action history.

### 5.3 Flow C: funding and settlement via NIP-AC

1. The client includes an OSCE envelope ref or funding payload in request metadata, message metadata, or an extension-specific data part.
2. The server resolves and validates the AC funding record, including:
   - scope,
   - cap,
   - expiry,
   - issuer / underwriter trust,
   - applicable guardian policy.
3. The server verifies that the requested actions fit within the permitted funding scope.
4. The server executes the task under those budget and authorization constraints.
5. If the envelope uses `cancel_until`, the server honors that cancel window before irreversible delivery or final settlement.
6. Successful completion maps to AC spend authorization and settlement receipt behavior.
7. Cancellation, interruption, or policy-triggered rollback may map to AC cancel or default behavior.
8. The A2A task outcome is returned to the client, but the authoritative funding state remains in NIP-AC events.

Under this flow:

- **A2A** carries the request/response interaction.
- **NIP-AC** remains authoritative for funding, settlement, receipts, defaults, and cancel windows.
- **NIP-SA** may require guardian approval before sensitive funded actions proceed.

---

## 6. Recommended Params and Metadata Conventions

The following conventions are recommended for interoperability and are safe to ignore by generic A2A clients that do not implement this profile.

### 6.1 Identifier conventions

- Nostr address refs SHOULD use raw hex pubkeys in `kind:pubkey:d` form.
- Human-friendly `npub` strings MAY be duplicated in extension params for display or UX.
- SKL manifest ref:
  - `33400:<skill_hex_pubkey>:<d-tag>`
- SA profile ref:
  - implementation-defined stable reference to the current `kind:39200` profile event
- SA session ref:
  - `39230:<agent_hex_pubkey>:<session-d-tag>`
- AC envelope ref:
  - `39242:<issuer_hex_pubkey>:<d-tag>` or a stable event id reference
- Trajectory event ref:
  - event id of a relevant `kind:39231` event
- AC receipt ref:
  - event id of a relevant `kind:39244` receipt

### 6.2 Where sovereign data belongs in A2A

- `AgentCard.capabilities.extensions[].params`
  - card-level sovereign declarations and capability hints
- `SendMessageRequest.metadata`
  - caller-provided sovereign task/funding refs
- `Message.metadata` and `Part.metadata`
  - per-message correlation, hashes, and extension-specific funding/auth data
- `Task.metadata` and `TaskStatusUpdateEvent.metadata`
  - server-emitted lifecycle and session correlation refs
- `Artifact.metadata`
  - server-emitted output correlation refs
- `Message.extensions` and `Artifact.extensions`
  - extension URIs that were actually active or contributed data during the interaction

### 6.3 Recommended extension params

#### SKL extension params

Suggested fields:

- `agentNpub`
- `nostrRelays`
- `sklManifestRefs`
- `authChallengeSupported`
- `assuranceSummary` (optional)

#### SA extension params

Suggested fields:

- `agentProfileRef` (recommended v1 value shape: `39200:<agent_hex_pubkey>:`)
- `trajectoryAuditAvailable`
- `delegationSupported`
- `securityPostureSummary`

#### AC extension params

Suggested fields:

- `osceSupported`
- `acceptedCurrencies`
- `acceptedUnderwriters`
- `acceptedSpendRails`
- `cancelWindowSupported`
- `osceHandoverMode`

### 6.4 Recommended metadata keys

Suggested server-emitted metadata keys:

- `saSessionId`
- `saSessionRef`
- `sklSkillRefsUsed`
- `guardianApprovalRefs`
- `osceEnvelopeRef`
- `acReceiptRefs`
- `trajectoryEventRefs`

Suggested client-provided metadata keys:

- `osceEnvelopeRef`
- `preferredNostrRelays`
- `requestedProfileUris`

These keys are profile-level conventions, not A2A core fields. The names above should be treated as frozen for OpenAgents v1 bridge compatibility.

### 6.5 Recommended OSCE handover modes

Recommended values for `osceHandoverMode`:

- `by-reference`
- `inline-data-part`
- `either`

Recommended starting point:

- prefer **by-reference** first, where the A2A request carries an AC envelope ref and the server resolves canonical state from Nostr.
- the OpenAgents v1 bridge should advertise and implement `by-reference` only until inline payload handling is explicitly added.

---

## 7. Security Considerations

### 7.1 A2A transport security still applies

- Standard A2A transport security remains required.
- Production deployments still need TLS-backed transport.
- This profile does not weaken A2A transport requirements.

### 7.2 Nostr key custody is still the sovereign trust root

- The sovereign Nostr key is the trust root for SKL / SA / AC identity and signed claims.
- Implementers should treat key custody as security-critical.
- Threshold or guardian-backed signing may be appropriate for higher-risk agents.

### 7.3 Trajectory events complement A2A logs

- A2A logs are operationally useful, but `kind:39230` / `kind:39231` remain the canonical signed lifecycle record.
- Implementers should define minimum trajectory emission rules so material execution can be reconstructed with integrity.

### 7.4 Cancel windows complement A2A cancellation

- A2A task cancellation and AC `cancel_until` behavior are related but not identical.
- Task interruption does not by itself define the canonical settlement result.
- Implementers should ensure task state and AC state cannot silently diverge.

### 7.5 Least privilege comes from auth, local policy, and guardianship

- `securitySchemes` should expose the auth surface cleanly.
- SKL auth challenge should be used when Nostr-key proof-of-possession matters.
- SA guardianship should gate higher-risk operations.
- Delegation must never widen the parent authorization or funding scope.

### 7.6 Privacy and public Nostr events

- Public Nostr events are not appropriate for raw sensitive task payloads.
- Sensitive content should generally stay off Nostr.
- On-Nostr refs should prefer hashes, identifiers, and minimal metadata.

---

## 8. Non-Goals

This profile does **not**:

- modify the A2A protocol,
- add new A2A core fields,
- replace NIP-SKL, NIP-SA, or NIP-AC,
- define a new portable authorization-grant event kind,
- require every A2A agent to adopt the OpenAgents sovereign stack,
- require every sovereign agent to expose an A2A interface.

Its purpose is narrower:

- define a stable pattern for agents that want to combine **A2A interoperability** with **sovereign Nostr-backed identity, lifecycle, audit, and payment semantics**.

---

## 9. Summary

Under this profile:

- A2A is the **wire layer**.
- NIP-SKL is the **skill registry / trust / optional auth-challenge layer**.
- NIP-SA is the **lifecycle / trajectory / guardianship / delegation layer**.
- NIP-AC is the **credit / settlement / cancel-window layer**.

An agent that follows this profile can remain A2A-compliant while also exposing a sovereign identity binding, verifiable skill inventory, explicit task-to-trajectory correlation, and outcome-scoped payment rails.
