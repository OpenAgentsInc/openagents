# OpenAgents roadmap: a system model

- Date: 2026-07-09
- Status: Sol analysis; interpretive, non-authoritative
- Source snapshot: `origin/main` at `93bfa6b7e3`,
  [`MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md) rev 6.19

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

## What the phase ladder is really doing

The roadmap's P0–P7 phases accumulate five properties.

### P0: prove the closed work loop

Khala Code mobile must prove that a person can authenticate, select work,
dispatch it to owned infrastructure, observe it, receive writeback, and be
charged correctly. The importance of the mobile MVP is not the app shell. It
is the first end-to-end proof that the system can turn a human request into a
verified result without hidden operator repair.

### P1: establish the relationship

Sarah adds the persistent human-facing identity, conversational intake,
qualification, memory, and sales loop. Under the Sarah-first revision, this
is no longer a temporary acquisition layer. It becomes the product surface
through which later capabilities arrive.

### P2: make delegated coding a daily capability

The Codex lanes move from a single cloud proof to an everyday workflow:
account custody, brokered credentials, target selection, session continuity,
multi-account concurrency, steer/interrupt/resume, and real repository
writeback. Sarah-first changes the entry point, not these mechanics.

### P3: make work persistent

Standing employees add triggers, durable definitions, schedules, budgets, and
unattended operation. The crucial transition is from “run this task” to “hold
this responsibility,” while preserving the same admission and evidence rails.

### P4: make context and authority durable

The Blueprint Map matures from a per-conversation model into the company
brain. Sarah becomes the first formal `ai_employee.v1`, with typed authority
states, identity bindings, provenance-bearing memory, and role-scoped slices.

### P5: make the system repeatable

Templates and integrations turn successful configurations into installable
products. The roadmap correctly requires external outcome receipts before a
template can be marketed as proven.

### P6–P7: make the system governable and scalable

The trust layer adds audited skills, input ceilings, canaries, and data
posture. The final phase expands distribution, assurance tiers, the business
dashboard, partner fulfillment, and network effects without discarding the
receipt and authority model built earlier.

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
remaining strategic proof is an ordinary authenticated relationship in which
a person asks Sarah for meaningful work, watches it happen, approves where
needed, receives a verified outcome, and can resume from another surface
without an operator stitching the experience together.

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
