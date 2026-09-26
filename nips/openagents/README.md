# OpenAgents protocols

OpenAgents defines general agent infrastructure. Coding agents are the first
specialization. These specifications describe how agents share capabilities,
workflows, context, and work across tools, models, and machines, and how they
improve their behavior through measured, bounded optimization.

The [agent labor integration plan](../../docs/agents/market-infrastructure.md)
is the next market application: agents negotiate bounded work and earn Bitcoin
for accepted results. The existing contracts supply capability, execution,
evidence, and authority. [NIP-MKT](NIP-MKT.md) now specifies negotiated
offerings, orders, and payment evidence; [NIP-LAB](NIP-LAB.md) specifies
bounded agent labor and acceptance. [NIP-CTRL](NIP-CTRL.md) adds scoped task
control from another client. These are drafts awaiting implementation, not
claims of a deployed market or synchronized Coder clients. The
[coverage review](../../docs/protocol/2026-09-26-openagents-gap-review.md)
explains the gaps and reuse across all three NIP lanes. Episodes 213–215,
266–267, and 275–281 are design inputs, not wire contracts.

The [81-document teardown review](../../docs/protocol/2026-09-26-teardown-coverage.md)
adds six draft profiles: persistent engine sessions (SESS), workspace resources
and synchronized views (WS), tracked work (WORK), bounded automation (AUTO),
environment leases (ENV), and live media/device interaction (LIVE). It also
extends POL's learned-preference lifecycle and EXT's imports and host component
sets. These reuse private artifacts and existing execution kinds. The
[Coder integration plan](../../docs/coder/design/teardown-nostr-integration.md)
defines implementation order and the evidence required before shipping them.

## Why this exists

Agent work should be understandable and controllable. A user should be able
to tell what an agent can do, what it may see or change, which version ran,
what evidence supports its result, and what remains unknown after a failure.
A task should preserve its objective, source evidence, instructions, budget,
and outcomes as it moves between operations or machines.

The same task should also survive changes in models and inference techniques.
A semantic AI contract describes the behavior needed. Its implementation can
use typed decisions, generation, retrieval, or a bounded composition of them.
An optimizer can search for a better implementation against an explicit
objective. Evaluation establishes evidence for adoption; host code controls
permissions, privacy, effects, and budgets throughout.

This supports the programming model associated with DSPy and optimization
approaches such as GEPA without making either a protocol runtime requirement.
Hand-authored implementations and other search methods use the same contracts.
No algorithm is assumed to improve every workload, and no score grants authority.

For users, the intended benefits are portable extensions, explicit control
over private context, understandable approvals, recoverable tasks, and
measured improvements that identify their costs and limitations. An update
can be evaluated and adopted without silently changing a task already running.

## Why use Nostr

Nostr provides an open foundation for an agent ecosystem that anyone can help
build. Its small core of signed events, public-key identities, and relay
subscriptions supports useful applications with relatively little machinery.
Additional NIPs define discovery, encrypted communication, and application
behavior. This gives agents and their users several benefits:

- **Open-source participation.** Developers can inspect, audit, run, modify,
  and share open-source clients, relays, and tools. Public protocol contracts
  also let them build independent implementations. Participation does not
  depend on one vendor's roadmap, private API, or marketplace approval.
- **Easy extensibility.** Developers can describe new behavior through event
  kinds, tags, and content schemas while reusing identity, signing, and
  delivery. Applications can implement a useful subset, document extensions,
  and grow through practical adoption. Each feature can build on shared
  infrastructure without requiring a complete new platform.
- **Interoperability and shared network effects.** Applications that implement
  the same contracts can discover capabilities, exchange tasks, consume
  package releases, and compare evaluation evidence. A contribution can serve
  users across multiple clients. Agents using different applications can find
  each other and coordinate work, increasing the usefulness of the shared
  network. These specifications supply the application meanings needed for
  that interoperability; a Nostr connection alone does not establish support
  for a program or execution interface.
- **Portable identity and user choice.** Public-key identity and signed records
  can remain usable across clients and relay operators. Users can choose
  interfaces and providers, self-host infrastructure, and carry authorized
  records between services while preserving authorship and provenance.
  Private information remains subject to its disclosure and access rules.
- **Resilience through replaceable components.** Independent relay and worker
  operators, replication of authorized records, and independent implementations
  can reduce dependence on a single service or codebase. Applications can
  select providers that support the required contracts and policies; delivery,
  retention, and recovery still require explicit operational choices.

These benefits apply to coding, research, data processing, and other agent
work. The shared formats make programs, extensions, and measured improvements
reusable across an ecosystem of independently built applications.

Hosts enforce access, manage credentials, materialize programs, run tools,
and coordinate effects. Evaluators measure outcomes. Relays store or deliver
events under explicit privacy and retention rules. Signatures establish
attribution, not truth, permission, statistical validity, or remote attestation.
The artifact formats also work locally; every model call or observation need
not become an event or require a relay round trip.

## General infrastructure and coding specialization

The shared core is identity, semantic contracts, typed operations and workflows,
extensions, evidence, authority, execution, coordination, evaluation, and
optimization. Research, document, data-analysis, and business agents use it
with their own sources, schemas, operations, and acceptance criteria.

Coding adds repository snapshots, code search, compiler diagnostics, isolated
execution, patches, tests, and Git integration. A research agent can instead
produce cited findings from documents. A records agent can propose an update
from scoped observations. Neither needs a repository or terminal. Sending a
message or changing an external record requires domain-specific authorization,
version checks, effect confirmation, and reconciliation.

Generality does not make domain guarantees interchangeable. A host supports
only the adapters, policies, and validation it can enforce. A coding benchmark
cannot admit a different domain merely because it uses the same model.

## How the pieces fit together

1. **Define and discover.** OPT identifies semantic AI contracts. CAP describes
   execution interfaces. EXT distributes exact component releases.
2. **Compose the work.** PRG defines typed workflows and bounded component
   invocation. An AI implementation binds a semantic contract to an executable
   entry and its complete dependencies.
3. **Prepare context and authority.** CTX identifies task state and evidence.
   POL resolves instructions, disclosure, approvals, and routing.
4. **Execute and recover.** CJ carries jobs. COORD manages shared tasks and
   claims. RUN records durable outcomes and unresolved effects.
5. **Measure and improve.** OPT bounds candidate search and records what ran.
   EVAL records workload comparisons and scoped admission evidence. Operator
   policy adopts an immutable eligible version for subsequent work.

CTRL connects additional clients to the same task owner with separate
observation, steering, and cancellation rights. MKT and LAB connect buyers
and providers through accepted commercial terms and exact deliverables.
Neither a paired client nor an accepted order bypasses POL, host admission,
independent verification, or separately authorized wallet actions.

SESS supplies the persistent engine boundary shared by terminal, desktop,
web, and mobile clients. WS gives those clients exact resources and bounded,
repairable views. WORK tracks objectives independently of individual runs;
AUTO admits finite occurrences against them. ENV binds actual runtimes and
resource lifetimes, while LIVE scopes media and device operations. These
profiles compose existing authority and execution contracts rather than
introducing another generic job or payment family.

For example, an evidence-selection operation may compare a typed relevance
model with a joint retrieval strategy. Both must preserve required context,
source attribution, and disclosure constraints. Evaluate complete task quality
and total cost before adopting either. The optimizer cannot modify the grader,
read protected confirmation labels, or give itself new permissions.

## Specification reference

All contracts in this set are v1 drafts. They define protocol behavior;
conformance requires validation and enforcement for each advertised role.

| Contract | Responsibility | Kinds |
| --- | --- | --- |
| [Shared contracts](contracts.md) | Encoding, references, schemas, locks, effects, evidence, outcomes, and private artifact envelopes. | Private artifact `3188`. |
| [NIP-CAP](NIP-CAP.md) | Execution descriptions, host bindings, grants, presence, and preferences. | `30180`, `30181`. |
| [NIP-PRG](NIP-PRG.md) | Typed workflows, seven step kinds, bounded composition, and plugin packet ABI. | `30182`, `30183`. |
| [NIP-EXT](NIP-EXT.md) | Component packages, immutable releases, imports, host component sets, discovery, revocation, and namespace transfer. | `3184`–`3186`, `30184`, `30185`; private records on shared `3188`. |
| [NIP-RUN](NIP-RUN.md) | Encrypted durable journals, fencing, evidence, and recovery. | `3187`, `30186`. |
| [NIP-CJ](NIP-CJ.md) | Conversation, typed-decision, and recoverable execution jobs. | `25900`/`26900`/`27000`, `25910`/`26910`/`27010`, `25920`/`26920`/`27020`. |
| [NIP-CTX](NIP-CTX.md) | Task frames, snapshots, context views, representations, and expansion. | Shared `3188`; CJ/RUN references. |
| [NIP-POL](NIP-POL.md) | Instructions, learned preferences, approvals, disclosure, routing, and adoption authority. | Shared `3188`; CJ/RUN references. |
| [NIP-COORD](NIP-COORD.md) | Tasks, fenced claims, shared budgets, background findings, and trial coordination. | Shared `3188`; CJ/RUN references. |
| [NIP-EVAL](NIP-EVAL.md) | Workload evaluation, comparisons, and scoped promotion evidence. | Shared `3188`; public declaration `3189`. |
| [NIP-OPT](NIP-OPT.md) | AI signatures, implementations, studies, data partitions, candidates, materialization, trials, and results. | Shared `3188`; EXT/EVAL declarations and CJ/RUN execution. |
| [NIP-KB](NIP-KB.md) | Shared knowledge entries: immutable versions, current-version heads, withdrawals, and evidence as EVAL publications. Trust is per reader. | `3190`, `30190`, `3191`; evidence on EVAL `3189`. |
| [NIP-CTRL](NIP-CTRL.md) | Client pairing, task-scoped control rights, revocation, acknowledged commands, and bounded catch-up. | Shared `3188`; registered CAP operations over CJ execution. |
| [NIP-MKT](NIP-MKT.md) | Immutable offerings, private negotiation, accepted orders, cancellation, and attributable Bitcoin settlement. | `3192`, `30192`; private records on shared `3188`. |
| [NIP-LAB](NIP-LAB.md) | Agent-labor terms, execution linkage, deliverables, verification, acceptance, rework, disputes, and rights. | Shared `3188`; MKT agreements and CJ/RUN execution. |
| [NIP-SESS](NIP-SESS.md) | Engine capability, persistent sessions, durable input queues, steering, interactions, and native history/imports. | Shared `3188`; CAP/CJ operations and RUN records. |
| [NIP-WS](NIP-WS.md) | Workspace resources, exact document versions, conditional changes, worktrees, checkpoints, and bounded projections. | Shared `3188`; CAP/CJ operations and RUN records. |
| [NIP-WORK](NIP-WORK.md) | Tracked objectives, planning relations, assignments, revision admission, disposition, source imports, and attention. | Shared `3188`; CAP/CJ operations and WS projections. |
| [NIP-AUTO](NIP-AUTO.md) | Finite schedules, source triggers, checked continuation, durable occurrence admission, and recovery. | Shared `3188`; CAP/CJ operations and RUN/COORD admission. |
| [NIP-ENV](NIP-ENV.md) | Environment allocation, exact materialization, bounded leases, participant admission, attachment, and cleanup. | Shared `3188`; CAP/CJ operations and RUN records. |
| [NIP-LIVE](NIP-LIVE.md) | Media participants and consent, input/speaking floors, capture anchors, and observation-bound device input. | Shared `3188`; CAP/CJ operations and admitted media transports. |
| [NIP-MV](NIP-MV.md) | Shared 3D worlds: ephemeral pose frames and gestures, durable entity state, world definitions, and cell-scoped subscriptions. Standalone: it depends on no other contract here. | `23300`, `23301`, `33300`, `33301`. |

Discovery heads are mutable. Exact signed records and artifact digests pin
execution. Publication, installation, enablement, selection, grants, admission,
and promotion are separate actions. Private evidence and derived examples
remain scoped even when the resulting implementation is useful to others.

Kind allocations are draft assignments, not upstream registrations. Named
extensions belong in NIP-11 `supported_extensions`, not numeric
`supported_nips`. Advertise only tested, configured roles. A relay forwarding
an envelope cannot claim to execute programs, isolate an evaluator, enforce
spending, or establish semantic correctness.

MKT and LAB define a new OpenAgents profile, not compatibility with the
historical Immortal market stack. Swaps, escrow, credit, and a general royalty
market are not implied. CTRL reuses current task contracts rather than
treating Block read-state sync or live telemetry as durable task control.
The [implementation plan](../../docs/protocol/implementation-plan.md) tracks
the role-specific validators, host work, and fixtures still required.
