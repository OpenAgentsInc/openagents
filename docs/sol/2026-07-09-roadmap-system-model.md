# OpenAgents roadmap: a system model

- Date: 2026-07-09
- Status: Sol analysis; conceptual companion, not a dispatch queue
- Canonical roadmap: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)
- Historical source snapshot: `origin/main` at `93bfa6b7e3`

## Thesis

OpenAgents is becoming a **relationship-centered operating system for
delegated work**.

The visible product is Sarah: a persistent conversational relationship with a
live canvas. Behind her is a typed work system that can understand a request,
resolve authority, select capacity, execute through agents and computers,
stream progress, collect exact evidence, and carry the resulting knowledge
forward.

That framing explains why the roadmap contains things that can otherwise look
like separate companies: a voice avatar, a coding fleet, Agent Computers,
Blueprint, Khala Sync, Pylon, a sales engine, a company brain, and a new UI
framework. They are not meant to remain peer products. They are layers of one
loop:

```text
relationship -> intent -> authority -> execution -> evidence -> memory
      ^                                                        |
      +--------------------- next conversation ----------------+
```

The roadmap is therefore not mainly a feature sequence. It is a sequence for
closing this loop under progressively broader capability and authority.

## The seven layers

| Layer | Primary role | Current OpenAgents expression |
| --- | --- | --- |
| Relationship | The stable human-facing identity and continuity | Sarah: voice, text, account relationship, mobile presence |
| Comprehension | Turn conversation into structured needs and candidate work | Khala inference, Sarah's typed tools, semantic selectors, Blueprint drafts |
| Control | Decide what may happen, under whose scope, budget, and approval posture | Typed intents, deal rules, tool policies, owner scopes, approval gates |
| Orchestration | Choose a workflow and capacity without exposing engine-room complexity | Khala routing, fleet policy, Pylon, harness selection, durable streams |
| Execution | Perform bounded work | Codex/Claude/Grok workers, owner-local Pylons, OpenAgents Agent Computers |
| Evidence | Establish what actually happened | Exact token rows, lifecycle receipts, verification results, closeout records |
| Continuity | Make outcomes available to the next turn and every authorized surface | Khala Sync, per-prospect memory, Blueprint Map, eventual company brain |

Effect Native spans these layers vertically. It is how control state,
streaming state, evidence, and memory become one typed interface on web,
mobile, desktop, and canvas.

## What the active phase ladder is doing

### P0: make Sarah Fleet Command operational

An authenticated owner creates one durable FleetRun through Sarah, and a
standing Pylon executes several pinned work units concurrently across named
Codex, Claude, and Grok accounts. Sarah projects resumable progress, handles
steering and approvals, and renders verified closeouts. The first acceptance
burn is owner-local; managed Agent Computers join through the same claim
contract without blocking it.

### P1 parallel: improve the relationship and consolidate its applications

Presentation quality—avatar, voice, opener, latency, fallback, and visual
polish—advances continuously without sitting ahead of the fleet loop. In the
same horizon, the estate contracts into three Effect Native applications:
OpenAgents web, OpenAgents mobile, and OpenAgents Desktop. The public website
retains root, Sarah, Forum, `/promises`, and explicit infrastructure
exceptions. Promise and service-deliverable evidence remains dereferenceable
through the contraction.

### P2: generalize only from a proven daily loop

Standing responsibilities, the company brain, templates, payments, outbound
work, assurance, and broader distribution remain directions rather than an
active issue forest. Each returns as a bounded Sarah capability after the
multi-harness fleet is routine, with the same admission, authority, Sync, and
evidence rails.

## The deep unification

The same abstractions recur at every scale:

- A conversation turn, a coding assignment, and a standing employee run all
  become typed requests with explicit authority.
- A prospect fact, a code verification result, and a business outcome all
  become provenance-bearing state rather than ungrounded model memory.
- A sales checkout, a Git push, and an outbound email all cross separate
  authority gates even if Sarah initiates the conversation.
- A phone, browser, and desktop cockpit display the same typed state and emit
  the same intent vocabulary, even when their renderers differ.
- A public claim is downstream of evidence; it is never created by the agent
  that performed the work.

This recursive reuse is the roadmap's strongest property. It creates a chance
for each new capability to strengthen the whole system instead of spawning a
new vertical stack.

## What is product and what is engine room

The Sarah-first decision becomes clearer if the estate is divided this way:

**Product vocabulary:** Sarah, conversation, Blueprint Map, work, approvals,
results, receipts, roles, and memory.

**Engine-room vocabulary:** model providers, Khala gateway routing, Pylon
leases, harness adapters, Codex homes, GPU render nodes, durable offsets,
token ledgers, and Cloud VM lifecycle.

Expert users and operators may need engine-room power tools. Ordinary users
should not have to assemble the system mentally before asking for an outcome.
The product earns its coherence by hiding implementation plurality while
remaining honest about authority, cost, progress, and failure.

## Current proof posture

The source material shows real substrate, not a paper architecture:

- Agent Computer lifecycle and scoped writeback have live proofs.
- The Khala-to-Pylon coding rail, exact usage accounting, and multiple harness
  contracts exist.
- Sarah is in the monorepo, served at `/sarah`, with a live Effect Native
  Blueprint Map and an owned renderer path.
- Khala Sync and the receipt discipline provide shared continuity and public
  projection machinery.
- Effect Native has working DOM, catalog, mobile, desktop, and graph/canvas
  direction, with the full conversion still underway.

But “substrate exists” is not the same as “product loop is closed.” The
remaining strategic proof is an authenticated owner asking Sarah for a pinned
plan, watching several real harness streams run concurrently, steering or
approving a named unit, receiving verified closeouts, and resuming from another
surface without an operator stitching the experience together.

## My concise reading

The company is not ultimately selling a chatbot, an avatar, a coding wrapper,
or a dashboard. It is selling **trusted delegated capability through a
persistent relationship**.

Sarah is the relationship. Blueprint is the legible memory and control model.
Khala and the fleets are the intelligence and labor. Pylon and Agent Computers
are execution. Receipts are trust. Khala Sync is continuity. Effect Native is
the shared application and interaction boundary.

If those pieces converge, the roadmap is one product. If they remain separate
surfaces with duplicated state and authority, it is a portfolio wearing a
single name.
