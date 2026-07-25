# Omega Agent implementation roadmap

- Status: owner-directed roadmap, 2026-07-25
- Owner: OpenAgents
- Program: `OMEGA-AGENT`
- Tracking epic: omega#73
- Packets: omega#74 through omega#82
- Design authority:
  [Omega Agent analysis](../fable/2026-07-25-omega-agent-analysis.md)
- Superseded design ask: omega#72 (closed, answered by the analysis and this
  roadmap)
- Companion ledgers: [Omega roadmap](./ROADMAP.md),
  [open issues unified completion plan](./2026-07-25-omega-open-issues-unified-completion-plan.md)

This roadmap defines the implementation program for Omega Agent.
Omega Agent is the first-party agent of Omega.
It is also the disclosed meta-harness above the executors Omega already runs.
The user meets one agent and the agent answers for the result.

This document extends the open-issue ledger in the omega repository.
It does not replace the owner-accepted [ROADMAP.md](./ROADMAP.md) packet
families.
The `OMEGA-AGENT` packets specialize `OMEGA-OA-02` (one agent front door) and
`OMEGA-OA-06` (native agent-to-code loop).

## 1. The shape decision

Issue omega#72 asked for a shape decision before code.
This roadmap records the accepted answers.

| Question | Answer |
| --- | --- |
| Shape | A at the seam. Omega Agent is a Rust `AgentConnection` implementation that owns routing, disclosure, and receipts. It does not own execution. |
| Executors | The native loop in `crates/agent`, external ACP agents through `crates/agent_servers`, and `omega-effectd` engine lanes (Full Auto, Agent Computer). |
| Shape B benefit | Arrives later. Packet omega#82 serves Omega Agent over loopback ACP, so external hosts can attach it. |
| Shape C benefit | Arrives through the workroom program (omega#31). The signed Nostr record carries agent history as record, never command authority. |
| Default model | Fork `main` already ships `google/gemini-3.6-flash`. Packet omega#74 records this with test evidence. |
| Identity | Agent history projects onto the workroom record. The Omega Nostr identity binding stays a binding, not a merge. |
| `agent_ui` | Kept. Omega capabilities travel by an extension trait behind a checked downcast, never a fork of `acp_thread`. |

The cloud-coupling severability trace from omega#72 is a deliverable of
omega#74, so the question closes with file-level evidence.

## 2. Program laws

These laws bind every packet.
They restate the accepted Omega design laws for this program.

1. GPUI is projection and command entry. `omega-effectd` stays the only Omega
   mutation path for run authority.
2. A native `Thread` is never the source of truth for Full Auto.
3. Omega never creates a second home for an external agent. No credential,
   home, or provider-configuration copy.
4. Every default change and removal lands as a numbered delta in
   `OMEGA_DELTAS.md` with a check in `crates/omega_deltas`.
5. Confirm on irreversible data loss, never on capability.
6. Routing decisions are typed and fail-closed. No keyword matching. No
   silent model guess. User pins always win.
7. Provider prose never closes work. Only typed outcomes close work.
8. The rename waits on the admitted product contract. No relabel before
   admission.

## 3. Packet ledger

| Packet | Issue | Goal | Depends on |
| --- | --- | --- | --- |
| OMEGA-AGENT-00 | omega#74 | Product contract and shape admission | none |
| OMEGA-AGENT-01 | omega#75 | Identity surface rename | 00 |
| OMEGA-AGENT-02 | omega#76 | Chat-first front door | 00 |
| OMEGA-AGENT-03 | omega#77 | Executor disclosure v0 | 01 |
| OMEGA-AGENT-04 | omega#78 | Deterministic router v1 | 02, 03 |
| OMEGA-AGENT-05 | omega#79 | Steer and queue semantics | 04 |
| OMEGA-AGENT-06 | omega#80 | Receipts and run links in-thread | 03 |
| OMEGA-AGENT-07 | omega#81 | Harness maintenance with provenance | 01 (loose) |
| OMEGA-AGENT-08 | omega#82 | Omega Agent served over ACP | 04 |

### OMEGA-AGENT-00: Product contract and shape admission (omega#74)

Deliver the Omega Agent ProductSpec at
`specs/omega/omega-agent.product-spec.md` in the openagents repository.
Record the shape answers from section 1 with evidence paths.
Trace whether `cloud_api_types` and `cloud_llm_client` are load-bearing at
runtime in `crates/agent` and `crates/agent_ui`.
Reserve the rename delta number and resolve the duplicate 0010 and 0011
identifiers in `OMEGA_DELTAS.md`.
Exit: the owner accepts the spec revision. No rename has landed.
Falsifier: implementation lands before admission.

### OMEGA-AGENT-01: Identity surface rename (omega#75)

Rename the first-party agent identity to Omega Agent across reachable UI.
Touch points: `ZED_AGENT_ID` in `crates/agent`, the panel `Agent` enum label
and icon in `crates/agent_ui`, and the `crates/sidebar` consumers.
The inherited telemetry identifier is never presented as an OpenAgents
service identity.
Land the identity as a numbered delta with a mechanical check.
Falsifier: a rebase reverts the identity without a test failure.

### OMEGA-AGENT-02: Chat-first front door (omega#76)

Owner UX direction: `cmd-shift-a` opens the New Agent Thread screen, and the
application defaults to that screen.
The screen is the welcome state and the global keybinding target.
The composer takes focus on open with the existing mention and slash
machinery.
The executor picker is present but subordinate.
Recent threads and active runs render beneath the composer.
Threads can start without a project and bind on the first workspace action.
Resolve the keymap deliberately: move `agent::ToggleFocus` off `cmd-?`, and
settle the `cmd-shift-a` editor-context collision.
Each keymap change is a delta with a test.
Coordinate with omega#69 for the workroom binding. This packet does not own
it.
Falsifier: a keymap change lands without a delta test.

### OMEGA-AGENT-03: Executor disclosure v0 (omega#77)

Every thread names its actual executor: runtime class, agent id, provider,
model, and run ref where one applies.
Disclosure travels by an extension trait behind a checked downcast.
Disclosure metadata persists with the thread. No new store.
A routed result is never presented as unattributed first-party output.
Falsifier: a thread shows work without its executor line.

### OMEGA-AGENT-04: Deterministic router v1 (omega#78)

Implement `OmegaAgentConnection` on the `AgentConnection` seam.
Route per thread across the three executor classes.
The decision is typed and fail-closed from capabilities, readiness, and pins.
Consult engine capacity through the framed `omega-effectd` protocol.
When the engine is unavailable, fail closed to the native loop and say so.
Record the route decision in thread metadata and the disclosure line.
Model-advisory routing is out of scope for v1.
Falsifier: the router keeps run or policy state in GPUI entities.

### OMEGA-AGENT-05: Steer and queue semantics (omega#79)

Give send-during-turn explicit semantics on every executor class.
Steer and enqueue are distinct typed commands.
Queue admission is durable before the UI acknowledges it.
One thread-owned scheduler promotes the queue head after proven quiescence.
An executor that cannot steer returns a typed refusal or a declared
fallback.
Falsifier: a second send reaches a running provider turn without a guard.

### OMEGA-AGENT-06: Receipts and run links in-thread (omega#80)

Dispatch engine work from the composer as typed commands with linked run
refs.
Render run state and the receipt chain through the receipt-inspector
pattern.
Typed outcomes close work. The engine stays sole run authority.
Inbound host-bridge work presents as sibling run-authority work.
Falsifier: a run's source of truth lands in a panel entity.

### OMEGA-AGENT-07: Harness maintenance with provenance (omega#81)

Typed per-harness maintenance actions on the registry agent path: detect,
resolve, update, re-probe.
Pin versions against a component ledger.
Verify provenance and write a receipt for every swapped binary.
Falsifier: an update swaps a full-permission binary without a provenance
record.

### OMEGA-AGENT-08: Omega Agent served over ACP (omega#82)

Port the loopback ACP server behind `omega-effectd`.
Default off. Loopback only. Deny-by-default permissions. Read-only shape.
The served agent is the router, so external hosts get disclosed routing.
Conformance claims use the pinned conformance artifacts in the openagents
repository.
The fleet-backed v1 server stays deferred.
Falsifier: GPUI owns the socket, or the server ships default-on.

## 4. Sequencing

Wave 1 delivers the product: omega#74, then omega#75, omega#76, and omega#77
in parallel.
Wave 1 is a naming, surface, and honesty release. It contains no new routing
brain, protocol, or store.
Wave 2 delivers the router and accountability: omega#78, omega#80, and
omega#81.
Wave 3 delivers turn semantics and reach: omega#79 and omega#82.

Release proof discipline from the identity and RC track (omega#8, omega#9,
omega#16) applies to any packet that claims a packaged journey.
The Full Auto proof track (omega#26) and the workroom track (omega#31,
omega#45 through omega#49) continue independently.

## 5. The Khala integration shape

By owner direction, this program mints no Khala packets now.
This section fixes the shape so the later addition is natural.

Khala is the collective-intelligence orchestrator behind the OpenAI-
compatible endpoint.
Omega Agent orchestrates one machine. Khala orchestrates the network.
They meet at two typed seams and never merge.

| Seam | Mechanism |
| --- | --- |
| Event vocabulary | `KhalaRuntimeEvent` in `@openagentsinc/agent-runtime-schema`. The harness adapters emit it. The Khala dispatch consumers write it into thread scopes. The schema authority lives in the shared SDK repository, outside both applications. |
| Dispatch | Omega reaches Khala through the framed `omega-effectd` protocol. Khala reaches Omega through the existing Pylon-linked delegation path onto owner-linked local executors. |

Khala appears in the routing table twice.

1. Outbound: Khala is an executor class. The router dispatches work that
   must leave the machine to Khala lanes through the engine, and renders the
   returned events and receipts.
2. Inbound: a Khala request classified as coding work lands on this owner's
   linked local executors. Omega Agent presents that work as sibling
   run-authority work with full disclosure.

Khala keeps what Omega must not absorb: the public gateway and routing
policy, the hosted and cloud lanes, fleet dispatch admission and the
own-capacity invariant, Khala Sync cross-device truth, metering and the
public counter, the optimization corpus, and settlement.
The standing precedent applies to every seam: do not duplicate
`khala-cloud-runtime-dispatch` policy in Rust.
Omega is a producer into Khala's ledger, never a second accountant.

Reserved future packet names: `OMEGA-AGENT-K1` (outbound routing surface),
`OMEGA-AGENT-K2` (inbound work presentation), `OMEGA-AGENT-K3` (receipt
corpus routing).
These packets require their own admission when the owner opens that lane.

## 6. Verification

Every packet runs `cargo test -p omega_deltas` and its own owned tests.
Runtime packets test protocol version skew, stale generations, cancellation,
crash and restart, and bounded buffers, per the ROADMAP proof contract.
GPUI packets test keyboard paths, visible focus, state restoration, and
theme variants.
A fixture pass is not a packaged-release claim.
Packaged claims follow the installed-proof evidence protocol in the
unified completion plan.
