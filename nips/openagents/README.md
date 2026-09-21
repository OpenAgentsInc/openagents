# OpenAgents protocols

These specifications describe how OpenAgents Coder can share capabilities,
workflows, context, and work across tools, models, and machines. They give
clients and workers a common way to answer practical questions: What can this
agent do? What is it allowed to see or change? Which version ran? What evidence
supports its answer? What happens if the connection breaks?

## Why this exists

The goal is a coding agent whose work you can inspect, reuse, and control.
A task should retain its objective, instructions, source evidence, budget,
and results as it moves between operations or machines. That makes it possible
to give different models relevant context, share observations with parallel
tasks, and recover interrupted work while preserving what is still unknown.

This supports the TypeSafe coding-agent design: small typed model judgments
help select relevant evidence and useful operations, while ordinary code owns
permissions, scheduling, budgets, and execution. Programs describe reusable
workflows. Extensions distribute their tools, Wasm plugins, skills, and other
components. The protocols define how these pieces identify themselves and
exchange information so that different implementations can work together.

For users, the intended benefits are portable extensions, explicit control
over where private context goes, understandable approvals, recoverable tasks,
and results linked to the code and checks that produced them. Evaluation
records let you assess whether context selection, model routing, or background
assistance improves the complete task, including its cost and failures.

## Why use Nostr

Nostr supplies signed identities and events, relay-based discovery and delivery,
and encrypted communication. These NIPs define the application contracts on
top: the meaning of a package release, a job, a context view, or a task record.
Shared contracts let clients and workers communicate through compatible relays
without depending on one application's private message format.

Hosts enforce access, run tools, manage credentials, and coordinate effects.
Relays store and deliver the records under their configured privacy and
retention policies. The same artifact formats also work locally; using Coder
on one machine does not require publishing each observation or making a relay
round trip for each action.

## How the pieces fit together

1. **Discover and install.** CAP describes execution capabilities. EXT packages
   and identifies exact releases so you can see what an extension provides.
2. **Describe the work.** PRG defines typed workflows, their dependencies, and
   their limits, including operations supplied by native tools or Wasm plugins.
3. **Prepare context and authority.** CTX records the task and relevant evidence.
   POL records applicable instructions, approvals, allowed recipients, and
   routing decisions.
4. **Execute and coordinate.** CJ carries jobs to workers. COORD describes
   shared tasks and background work. RUN retains the execution history needed
   to explain results and recover interrupted work.
5. **Measure and improve.** EVAL describes comparable workload evidence so
   hosts can make explicit, informed choices about which versions to adopt.

For example, a parser repair could reuse one captured test failure for the
repairing agent, a separate reviewer, and a background explanation. Each would
receive its own permitted context. Their results would identify the source
revision, consumed resources, verification, and whether the proposed change
was accepted into the workspace.

## Specification reference

This lane is authored here and is not synced from upstream. These contracts
define the program and extension system. They are drafts; source text and
kind allocation do not establish implementation or deployment.

| Contract | Responsibility | Kinds |
| --- | --- | --- |
| [Shared contracts](contracts.md) | References, schemas, locks, effects, evidence, context, and outcomes. | No new kinds. |
| [NIP-CAP](NIP-CAP.md) | Execution definitions, local bindings, grants, and preferences. | `30180`, `30181`. |
| [NIP-PRG](NIP-PRG.md) | Typed programs, seven step kinds, composition, and plugin packet ABI. | `30182`, `30183`. |
| [NIP-EXT](NIP-EXT.md) | Releases, descriptors, packages, revocation, and namespace migration. | `3184`–`3186`, `30184`, `30185`. |
| [NIP-RUN](NIP-RUN.md) | Encrypted journals, evidence, fencing, recovery, and retention. | `3187`, `30186`. |
| [NIP-CJ](NIP-CJ.md) | Conversation, decision, and recoverable execution transports. | Conversation `25900`/`26900`/`27000`; decision `25910`/`26910`/`27010`; execution `25920`/`26920`/`27020`. |
| [NIP-CTX](NIP-CTX.md) | Task frames, evidence representations, context views, and hierarchical expansion. | Shared private artifact `3188`; existing CJ/RUN. |
| [NIP-POL](NIP-POL.md) | Scoped instructions, action approval, disclosure, and route/cache/cost records. | Shared private artifact `3188`; existing CJ/RUN. |
| [NIP-COORD](NIP-COORD.md) | Shared tasks, fenced claims, background plans, and findings. | Shared private artifact `3188`; existing CJ/RUN. |
| [NIP-EVAL](NIP-EVAL.md) | Workload comparisons, evaluation reports, and scoped promotion evidence. | Shared private artifact `3188`; public declaration `3189`. |

CAP and PRG are revised in place as v1. New EXT, RUN, shared artifact schemas,
and execution jobs also begin at v1. Existing CJ conversation integer versions
1/2 and decision `openagents.systemone.v1` retain their separate contracts.
Earlier repository CAP/PRG objects require coordinated reader/data changes;
an unchanged `v` value does not make the earlier draft shape conformant.
CTX, POL, COORD, and EVAL also start at v1. The
[TypeSafe agent addendum](../../docs/coder/design/typesafe-agent-protocol-addendum.md)
maps every source recommendation to these protocols and the host/client work
that remains. Artifact-only contracts do not require a new event for every
local action or a relay connection for local execution.

Addresses identify mutable heads; exact signed records and artifact digests
identify execution. Discovery, publication, installation, enablement, grants,
selection, and admission are distinct. Private evidence stays scoped; every
local artifact or judgment need not become a Nostr event.

Kind numbers are local draft assignments checked against the pinned lanes,
not upstream registrations. Conformance belongs separately to domain parsers,
relays, host execution, and configured clients/workers. Only proven roles and
features may be advertised. [Implementation tracking](../../docs/protocol/implementation-plan.md)
maps this lane and the pinned official/Block lanes to completion work.
Named draft extensions belong in NIP-11 `supported_extensions`, not numeric
`supported_nips`. Name the configured role; forwarding an envelope does not
claim host admission, coordination, or semantic correctness.
