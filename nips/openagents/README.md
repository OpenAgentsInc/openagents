# OpenAgents protocols

OpenAgents defines general agent infrastructure. Coding agents are the first
specialization. These specifications describe how agents share capabilities,
workflows, context, and work across tools, models, and machines, and how they
improve their behavior through measured, bounded optimization.

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

Nostr supplies signed events, public-key identities, relay discovery and
delivery, and encrypted communication. These specifications define application
meanings: a package release, a task, a context view, an optimization study,
or an execution record. Clients and workers can exchange those records through
relays without relying on one application's private message format.

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
| [NIP-EXT](NIP-EXT.md) | Component packages, immutable releases, discovery, revocation, and namespace transfer. | `3184`–`3186`, `30184`, `30185`. |
| [NIP-RUN](NIP-RUN.md) | Encrypted durable journals, fencing, evidence, and recovery. | `3187`, `30186`. |
| [NIP-CJ](NIP-CJ.md) | Conversation, typed-decision, and recoverable execution jobs. | `25900`/`26900`/`27000`, `25910`/`26910`/`27010`, `25920`/`26920`/`27020`. |
| [NIP-CTX](NIP-CTX.md) | Task frames, snapshots, context views, representations, and expansion. | Shared `3188`; CJ/RUN references. |
| [NIP-POL](NIP-POL.md) | Instructions, approvals, disclosure, routing, and adoption authority. | Shared `3188`; CJ/RUN references. |
| [NIP-COORD](NIP-COORD.md) | Tasks, fenced claims, shared budgets, background findings, and trial coordination. | Shared `3188`; CJ/RUN references. |
| [NIP-EVAL](NIP-EVAL.md) | Workload evaluation, comparisons, and scoped promotion evidence. | Shared `3188`; public declaration `3189`. |
| [NIP-OPT](NIP-OPT.md) | AI signatures, implementations, studies, data partitions, candidates, materialization, trials, and results. | Shared `3188`; EXT/EVAL declarations and CJ/RUN execution. |

Discovery heads are mutable. Exact signed records and artifact digests pin
execution. Publication, installation, enablement, selection, grants, admission,
and promotion are separate actions. Private evidence and derived examples
remain scoped even when the resulting implementation is useful to others.

Kind allocations are draft assignments, not upstream registrations. Named
extensions belong in NIP-11 `supported_extensions`, not numeric
`supported_nips`. Advertise only tested, configured roles. A relay forwarding
an envelope cannot claim to execute programs, isolate an evaluator, enforce
spending, or establish semantic correctness.
