# Nostr protocol implementation plan

Status: proposed implementation and conformance work. The
[OpenAgents protocol set](../../nips/openagents/README.md) contains standalone
v1 specifications for general agent infrastructure. Coding is the first domain
profile. The [consolidated proposed issues](../optimization/proposed-issues.md)
define the unfiled backlog for full semantic-programming and optimization
integration. This plan creates no GitHub issues.

## Scope and boundaries

Protocol validators enforce encoding, identities, schemas, and cross-record
relationships. Relays enforce publication, query visibility, replacement,
retention, and transport behavior. Hosts enforce execution authority, effects,
resource limits, candidate isolation, and recovery. Clients verify the records
they consume and present uncertainty accurately. Evaluators establish measured
workload evidence under a frozen policy.

No role advertises another role's guarantees. A relay cannot attest semantic
quality, a package cannot grant authority, and an optimizer cannot activate a
candidate. General contracts require non-coding fixtures as well as coding
examples.

## Protocol workstreams

| Contract | Required implementation | Completion evidence |
| --- | --- | --- |
| Shared | Strict bounded parsing, references, schema closure, locks, effects, observations, receipts, and encrypted artifacts. | Malformed/unknown input refuses; identity, privacy, and scope hold under every query and execution path. |
| CAP | Definitions, host bindings, grants, presence, probes, and enforcement plans. | Unsupported bounds refuse; discovery stays inert; optimizer/evaluator/runtime roles are distinct. |
| PRG | Typed dataflow, seven step kinds, fan-out, retries, AI implementation dispatch, and Wasm ABI. | Complete graph checks precede effects; nested work shares reservations; protected transitions survive candidate search. |
| EXT | Component packages, inert staging, atomic locks, publication, revocation, and namespace transfer. | Full functional closure is pinned; private examples cannot escape through public releases; active runs retain pins. |
| RUN | Authoritative journal, dispatch intent, fencing, reconciliation, and retention. | Crash/late-result fixtures retain unknown effects and spending; recovery preserves study and confirmation state. |
| CJ | Conversation, typed-decision, and execution families with exact signer/recipient/request binding. | Family isolation, sequence checks, durable idempotency, cancellation, replay, and candidate attribution. |
| CTX | Task frames, scoped snapshots, derivatives, context manifests, hierarchy, and expansion. | Source completeness and mandatory evidence survive transformation; optimizer participants receive only admitted data. |
| POL | Instruction precedence, disclosure, approvals, routes, observed usage, and adoption policy. | Model scores cannot override authority; reflection/training/export/deployment grants are distinct. |
| COORD | Claims, fences, background plans, findings, trial identity, and shared accounting. | Retransmissions deduplicate; independent trials do not; confirmation access serializes across workers. |
| EVAL | Workload suites, exact attempts, comparisons, uncertainty, public/private reports, and scoped admission. | Protected graders/labels, explicit missing outcomes, independent confirmation, and whole-task claims. |
| OPT | Signatures, implementations, study/data/search plans, candidates, materialization, trials, cost, and results. | The actual loaded candidate is measured; frozen meaning and data rights survive search; adoption remains separate. |

## Delivery order

Build the inert domain contracts and local host path first. Complete bounded
candidate execution, evaluation isolation, durable accounting, and operator
adoption as one inspectable loop. Then demonstrate the same identities and
privacy guarantees over relay transport. Network conformance must not require
a particular optimizer or model vendor.

The [proposed issues](../optimization/proposed-issues.md) separate this work
into independently reviewable acceptance slices. Broader strategy/composition
search follows proven materialization and measurement. Online adaptation and
weight training require their own scoped design and authority.

## Conformance across Nostr lanes

Official and application-extension Nostr specifications have distinct role
requirements. Build an explicit fixture matrix for every supported relay,
client, worker, and host role. Implement and advertise optional features only
when configured and proven; a documented event kind is not operational support.

Cover authentication, signatures, encryption, filters, visibility before
limits/counting, replacement, deletion, expiration, replay, retention, and
unsupported-feature refusal. Keep application quality and host enforcement
claims separate from transport conformance.

Use named OpenAgents extensions in NIP-11 for these drafts, with the configured
role and supported feature set. Numeric upstream NIP advertisement applies
only to those upstream contracts. Check kind allocation before public
interoperation; do not treat a local specification as upstream registration.

## Definition of done

A complete release includes schema fixtures, host enforcement evidence,
local/remote interoperability, recovery and privacy cases, domain-specific
evaluation, and accurate client states. It includes a coding workflow and a
non-coding fixture without imposing Git, shell, or terminal requirements on
shared records.

Documentation-only changes need prose, link, and contract consistency checks.
Implementation validation targets the affected behavior and protocol roles.
Paid campaigns, public artifact publication, and deployment require separate
authorization; adopting a plan does not perform them.
