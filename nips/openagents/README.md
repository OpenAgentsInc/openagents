# OpenAgents protocols

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
