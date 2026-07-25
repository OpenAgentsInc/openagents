---
spec_format_version: "0.1"
title: "Omega Agent Product Contract"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-25T18:00:00.000Z"
updated_at: "2026-07-25T18:00:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "docs/omega/"
  - path: "specs/omega/"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_issue: "OpenAgentsInc/omega#74 (OMEGA-AGENT-00)"
  openagents_epic: "OpenAgentsInc/omega#73 (OMEGA-AGENT)"
  openagents_roadmap: "docs/omega/2026-07-25-omega-agent-roadmap.md"
  openagents_shape_record: "docs/omega/2026-07-25-omega-agent-shape-record.md"
  openagents_severability_trace: "docs/omega/2026-07-25-omega-agent-cloud-severability-trace.md"
  openagents_design_analysis: "docs/fable/2026-07-25-omega-agent-analysis.md"
  openagents_omega_source_pin: "OpenAgentsInc/omega b768854c56 (0.2.0-rc10)"
  openagents_assurance_companion: "none at revision 1"
  openagents_revision_1_note: "Rev 1 admits the Omega Agent identity and the router-over-executors shape at the AgentConnection seam. It admits no rename, no router code, and no public claim. The Omega Nostr identity signing question stays open and owner-reserved."
---

## Problem

Omega runs three classes of agent executors today.
It presents no first-party agent identity above them.
The reachable label for the inherited native loop is `Zed Agent`.

A user who opens Omega meets a picker of competing lanes.
The user does not meet one agent that answers for a result.
The fork forbids a relabel of that tile before an admitted product contract.

Without an admitted contract, three failures follow.
A rename produces a rebranded upstream tile beside `codex-acp` and
`claude-acp`.
A router lands with no rule against a second durable run authority.
A routed result reaches the user with no statement of which executor ran it.

## Hypothesis

Omega can present one named agent with disclosed routing when these
conditions hold.
Omega Agent implements the existing `AgentConnection` trait and owns routing,
disclosure, and receipts.
It owns no execution and no durable run state.
`omega-effectd` stays the only Omega mutation path for run authority.
Omega-only capabilities travel by an extension trait behind the existing
`into_any` downcast, and never fork `AcpThread`.
Every thread names its actual executor.

## Scope

```productspec-scope
in:
  - the product identity Omega Agent for the first-party agent of Omega
  - Omega Agent as an AgentConnection implementation that routes and does not execute
  - the three admitted executor classes native loop, external ACP agent, omega-effectd engine lane
  - the disclosure obligation that every thread names runtime class, agent id, provider, model, and run ref where one applies
  - the front-door expectation that the welcome state and the global new-thread keybinding reach one New Agent Thread surface
  - typed fail-closed route selection from declared capability, readiness, and user pin
  - Omega-only capability transport by extension trait behind the existing into_any downcast
  - the record boundary that agent history projects onto the workroom record and never takes command authority
  - the Khala boundary that Omega is a producer into the Khala ledger and never a second accountant
  - the severability statement of what Omega Agent does when no OpenAgents cloud service answers
out:
  - the identity surface rename, which OMEGA-AGENT-01 owns
  - router implementation code, which OMEGA-AGENT-04 owns
  - keymap changes, which OMEGA-AGENT-02 owns after omega#69
  - a numbered delta allocation or any edit to OMEGA_DELTAS.md
  - model-advisory routing and the offline policy loop
  - Khala outbound routing, inbound presentation, and receipt corpus routing
  - release admission, packaged-journey claims, and public product claims
  - a decision on whether Omega Agent signs with the Omega Nostr identity
cut:
  - a fork of AcpThread or a second thread projection
  - a GPUI-owned durable run store or a second run lifecycle enumeration
  - a Rust copy of khala-cloud-runtime-dispatch policy
  - a second home, credential copy, or provider-configuration copy for an external agent
  - keyword matching or a silent model guess in route selection
  - presentation of the inherited telemetry identifier as an OpenAgents service identity
```

## User Experience

The user opens Omega and reaches a New Agent Thread surface.
The composer holds focus, so the user types at once.
One agent answers, and that agent is Omega Agent.

The thread states which executor did the work.
The statement gives the runtime class, the agent id, the provider, and the
model.
Engine work also gives the run reference.

An executor selector sits beside the model and profile selectors.
The selector is present and subordinate to the agent identity.
A user pin on that selector always wins over the router.

When no engine answers, the agent falls back to the native loop and says so.
When no cloud service answers, the agent states which functions stopped.
The agent never presents routed work as unattributed first-party output.

## Acceptance Criteria

- **OMEGA-AGENT-AC-01:** `Omega Agent` names the product identity and the
  router. It does not name the inherited native loop. The native loop in
  `crates/agent` is one executor under that identity.
- **OMEGA-AGENT-AC-02:** Omega Agent implements the existing `AgentConnection`
  trait at `crates/acp_thread/src/connection.rs`. No packet forks `AcpThread`
  or adds a second thread projection.
- **OMEGA-AGENT-AC-03:** Omega-only capabilities travel by an extension trait
  behind the existing `into_any` downcast at
  `crates/acp_thread/src/connection.rs`. `crates/agent_ui` keeps its current
  panel and thread surfaces.
- **OMEGA-AGENT-AC-04:** The admitted executor class set at this revision is
  exactly three. The classes are the native loop in `crates/agent`, external
  ACP agents through `crates/agent_servers`, and `omega-effectd` engine lanes.
  A fourth class needs a new spec revision.
- **OMEGA-AGENT-AC-05:** Every thread names its actual executor. The
  disclosure states runtime class, agent id, provider, model, and the run
  reference where one applies. The disclosure survives a restart.
- **OMEGA-AGENT-AC-06:** Omega Agent holds no durable run authority.
  `omega-effectd` stays the only Omega mutation path for run authority. A
  native `Thread` is never the source of truth for Full Auto.
- **OMEGA-AGENT-AC-07:** The router selects a route from a typed, fail-closed
  decision. The decision reads declared capability, readiness, and user pin.
  The router uses no keyword match and no silent model guess. A user pin
  always wins. An unavailable engine fails closed to the native loop with a
  visible statement.
- **OMEGA-AGENT-AC-08:** The welcome state and the global new-thread
  keybinding reach the same New Agent Thread surface. The composer holds
  focus on open.
- **OMEGA-AGENT-AC-09:** Omega Agent never creates a second home for an
  external agent. It copies no credential, home directory, or provider
  configuration.
- **OMEGA-AGENT-AC-10:** The inherited telemetry identifier is never
  presented as an OpenAgents service identity.
- **OMEGA-AGENT-AC-11:** Omega Agent does not duplicate
  `khala-cloud-runtime-dispatch` policy in Rust. Khala keeps the public
  gateway, hosted and cloud lanes, fleet admission, Khala Sync, metering, the
  optimization corpus, and settlement. Omega is a producer into that ledger.
- **OMEGA-AGENT-AC-12:** Agent history projects onto the workroom record.
  The record carries history and never takes command authority. Omega Agent
  consumes the workroom lane and does not fork it.
- **OMEGA-AGENT-AC-13:** The native executor completes a turn with a direct
  provider when no OpenAgents cloud service answers. The severability trace
  records which functions stop in that condition.
- **OMEGA-AGENT-AC-14:** Every default change, removal, and keymap change in
  this program lands as a numbered delta in `OMEGA_DELTAS.md` with a
  mechanical check in `crates/omega_deltas`.

## Success Metrics

- **OMEGA-AGENT-SM-01:** Every `OMEGA-AGENT` implementation packet cites this
  ProductSpec path and revision, or a later admitted revision.
- **OMEGA-AGENT-SM-02:** No landed `OMEGA-AGENT` packet introduces a
  GPUI-owned durable run store or a second run lifecycle enumeration.
- **OMEGA-AGENT-SM-03:** No landed `OMEGA-AGENT` packet introduces a Rust copy
  of Khala dispatch policy.
- **OMEGA-AGENT-SM-04:** After the front-door packet lands, a fresh Omega
  start reaches a focused composer with no click.
- **OMEGA-AGENT-SM-05:** After the disclosure packet lands, each thread states
  its executor after a restart.

## Owner Gates

- **Admission.** The owner admits this revision. Admission opens omega#75
  through omega#82. Nothing in this revision is a release or public claim.
- **Nostr identity signing, open.** The owner has not answered whether the
  first-party agent signs with the Omega Nostr identity. This revision does
  not answer it. The two branches and their costs are in
  `docs/omega/2026-07-25-omega-agent-shape-record.md` section 6.
- **Reserved Khala packets.** `OMEGA-AGENT-K1` through `OMEGA-AGENT-K3` need
  their own owner admission. This revision mints none of them.
- **Reserved acts.** Packaged-journey observation, AssuranceSpec admission,
  and public product claims stay owner-reserved.

## Receipts

- Shape record: `docs/omega/2026-07-25-omega-agent-shape-record.md`
- Cloud-coupling severability trace:
  `docs/omega/2026-07-25-omega-agent-cloud-severability-trace.md`
- Implementation roadmap: `docs/omega/2026-07-25-omega-agent-roadmap.md`
- Design analysis, strategic evidence only:
  `docs/fable/2026-07-25-omega-agent-analysis.md`
- Behavior contracts: `pending`. No Omega behavior-contract registry exists in
  `packages/behavior-contracts`. `OMEGA-AGENT-02` (omega#76) owns the two
  front-door contracts and the registry entry.
- Eval Suites: `pending`. This program has no Eval Suite at this revision.
- AssuranceSpec companion: `pending`. This revision authors none.

## Promise Links

- This contract alone admits no public product-promise change.
- This revision links no promise identifier.
