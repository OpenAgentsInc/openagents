---
spec_format_version: "0.1"
title: "Omega Agent Product Contract"
artifact_type: "prd"
spec_revision: 2
author: "OpenAgents"
created_at: "2026-07-25T18:00:00.000Z"
updated_at: "2026-07-27T10:20:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "docs/omega/"
  - path: "docs/omega-agent/"
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
  openagents_assurance_companion: "none at revision 2"
  openagents_revision_1_note: "Rev 1 admits the Omega Agent identity and the router-over-executors shape at the AgentConnection seam. It admits no rename, no router code, and no public claim. The Omega Nostr identity signing question stays open and owner-reserved."
  openagents_revision_2_note: "Rev 2, on the owner direction of 2026-07-27, admits the basic agent: a slim five-tool first-party executor (read, write, edit, bash, delegate) that completes work reliably on the default google/gemini-3.6-flash direct provider with no harness installed, delegates to installed harnesses with typed disclosure, names Exo (exoharness/exo) as an external ACP delegate target, and binds the work-loss law. The router shape, the three executor classes, and the open Nostr identity question from revision 1 stand."
  openagents_revision_2_reconciliation: "Implementation receipts indexed 2026-07-27: OpenAgentsInc/omega c46980f6c4 and OpenAgentsInc/openagents 4c2db79b70. The installed candidate journey and live basic-versus-wide comparison remain pending; these commits authorize no release or public reliability claim."
  openagents_slim_audit: "docs/omega-agent/2026-07-27-slim-agent-audit.md"
  openagents_slim_spec: "docs/omega-agent/2026-07-27-slim-agent-spec.md"
  openagents_exoharness_teardown: "docs/teardowns/2026-07-25-exoharness-exo-teardown.md"
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

Revision 2 adds the reliability problem.
The inherited native executor exposes 25 tools sized for editor
workflows, under a long editor-shaped prompt.
A person with no external harness installed has no simple first-party
agent that just works.
A person with harnesses installed has no agent that knows when to hand
work to them.
The `bash` path can also destroy uncommitted work with one git command,
which the incident record at
`docs/oopsiewoopsies/2026-07-27-git-checkout-destroyed-uncommitted-work-twice.md`
documents.

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

Revision 2 adds the basic-agent hypothesis.
One basic first-party agent works reliably for all people with no
delegation at all.
It has exactly five tools: `read`, `write`, `edit`, `bash`, and
`delegate`.
It completes coding work out of the box on the default
`google/gemini-3.6-flash` direct provider.
Delegation is a capability, never a dependency.

When the person has installed harnesses, the agent knows when to hand
work to them, and it discloses every handoff.
The agent cannot destroy uncommitted work through its own tools without
a typed confirm.

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
  - the basic agent as the default executor, with exactly five model-visible tools read, write, edit, bash, and delegate
  - out-of-box reliability on the default google/gemini-3.6-flash direct provider with no harness installed
  - delegation to installed harnesses as a first-class delegate tool with a typed disclosure record per handoff
  - Exo (exoharness/exo) as a named delegate target in the external ACP executor class
  - the work-loss law that no slim tool discards uncommitted changes without a typed confirm
  - the measured slim system prompt with a mechanical byte-ceiling check
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
  - a sixth model-visible tool in the basic profile without a new spec revision
  - a delegation dependency in the baseline journey, where baseline work fails because no harness is installed
  - runtime tool self-injection into the host process, the Exo pattern the estate refuses
  - silent executor substitution in a delegate call
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

The out-of-box journey is complete on its own.
A person installs Omega with nothing else on the machine.
The basic agent answers with its five tools on the default
`google/gemini-3.6-flash` provider, and finishes real coding work.
No step of that journey requires a harness, an account with another
vendor, or a delegation.

The harness journey extends it.
When the person installs Codex, Claude Code, or Exo, the agent
recognizes work that fits a harness better and offers or performs a
`delegate` handoff.
The handoff names its target, and the result carries the executor
disclosure.
When the named harness is absent or refuses, the agent says so and
completes the work itself.

The agent protects the person's work.
A command that would discard uncommitted changes gets a typed confirm
that names the files at risk.
The agent's own undo restores a snapshot it took.

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
- **OMEGA-AGENT-AC-15:** The basic agent exposes exactly five tools to the
  model: `read`, `write`, `edit`, `bash`, and `delegate`. A sixth
  model-visible tool in the basic profile needs a new spec revision.
- **OMEGA-AGENT-AC-16:** On a fresh install with no external harness, the
  basic agent completes a coding turn with only its five tools. The turn
  runs on the default `google/gemini-3.6-flash` direct provider. The
  baseline journey never depends on a harness.
- **OMEGA-AGENT-AC-17:** A `delegate` call with no installed executor
  returns a typed no-executor result, and the agent completes the work
  itself. The absence of harnesses is a stated condition, never an error
  loop.
- **OMEGA-AGENT-AC-18:** A `delegate` call for a named executor runs that
  executor or fails with a typed reason. No substitution occurs. Every
  delegate result carries a typed executor-disclosure record and a session
  address the parent can read.
- **OMEGA-AGENT-AC-19:** Exo (`exoharness/exo`) is an admitted delegate
  target in the external ACP executor class. A delegated Exo turn can
  itself host a vendor executor. That turn names the full chain in its
  disclosure: Omega Agent, then Exo, then the hosted runtime and model.
- **OMEGA-AGENT-AC-20:** No basic-agent tool discards uncommitted changes
  without a typed confirm. A file-scoped git restore command against a
  dirty scope confirms first and names the files at risk. Every guard
  decision is a typed transcript result.
- **OMEGA-AGENT-AC-21:** The basic agent has a measured system prompt. A
  mechanical check asserts the rendered template's byte ceiling, and a
  ceiling change is a deliberate delta.

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
- **OMEGA-AGENT-SM-06:** After the basic-agent packets land, a fresh
  install with no harness reaches a first completed coding turn on the
  default provider. The proof harness records that journey.
- **OMEGA-AGENT-SM-07:** After the delegate packet lands, every delegate
  result in the proof corpus carries its disclosure record. Zero
  undisclosed handoffs.
- **OMEGA-AGENT-SM-08:** The basic profile stays at five tools across
  rebases. The delta check that asserts the tool set stays green.

## Owner Gates

- **Admission.** The owner admitted revision 1 on 2026-07-25, and that
  admission opened omega#75 through omega#82. Revision 2 records the
  owner direction of 2026-07-27 for the basic agent, and opens the
  slim-agent packets in `docs/omega-agent/2026-07-27-slim-agent-spec.md`
  section 10. Nothing in this revision is a release or public claim.
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
- Slim-agent audit: `docs/omega-agent/2026-07-27-slim-agent-audit.md`
- Slim-agent specification:
  `docs/omega-agent/2026-07-27-slim-agent-spec.md`
- Exoharness teardown, delegate-target evidence:
  `docs/teardowns/2026-07-25-exoharness-exo-teardown.md`
- Work-loss incident record:
  `docs/oopsiewoopsies/2026-07-27-git-checkout-destroyed-uncommitted-work-twice.md`
- Behavior contracts:
  `packages/behavior-contracts/src/omega-agent.ts`, registry version
  `2026-07-27.1`. Five slim-agent contracts are mechanically enforced in the
  Omega repository. The out-of-box completion contract stays `pending` until
  an observer records the candidate-bound installed journey. OpenAgents
  validates the registry but does not execute its cross-repository Rust
  oracles.
- Slim-agent proof protocol:
  `OpenAgentsInc/omega` `c46980f6c4`,
  `docs/src/development/omega-slim-agent-proof.md`, and
  `script/prove-omega-slim-agent`. The protocol validates installed and
  harness journeys, the same-task basic-versus-wide comparison, and
  OMEGA-DELTA-0133 through OMEGA-DELTA-0138. Fixture evidence is not packaged
  evidence.
- Eval Suites: `pending`. `eval-cli` and `zed-eval` now expose explicit
  `basic` and `wide` profiles for the same-task comparison, but no live
  comparison receipt was available for this reconciliation.
- AssuranceSpec companion: `pending`. This revision authors none.

## Promise Links

- This contract alone admits no public product-promise change.
- This revision links no promise identifier.
