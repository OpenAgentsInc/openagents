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
| EXT | Component packages, inert staging, atomic locks, foreign imports, host component-set assessments, publication, revocation, and namespace transfer. | Full functional closure is pinned; unsupported import semantics refuse; partial upgrades retain recovery obligations; active runs retain pins. |
| RUN | Authoritative journal, dispatch intent, fencing, reconciliation, and retention. | Crash/late-result fixtures retain unknown effects and spending; recovery preserves study and confirmation state. |
| CJ | Conversation, typed-decision, and execution families with exact signer/recipient/request binding. | Family isolation, sequence checks, durable idempotency, cancellation, replay, and candidate attribution. |
| CTX | Task frames, scoped snapshots, derivatives, context manifests, hierarchy, and expansion. | Source completeness and mandatory evidence survive transformation; optimizer participants receive only admitted data. |
| POL | Instruction precedence, governed learned preferences, disclosure, approvals, routes, observed usage, and adoption policy. | Learning and activation require separate admission; scope cannot widen during translation; reflection/training/export/deployment grants remain distinct. |
| COORD | Claims, fences, background plans, findings, trial identity, and shared accounting. | Retransmissions deduplicate; independent trials do not; confirmation access serializes across workers. |
| EVAL | Workload suites, exact attempts, comparisons, uncertainty, public/private reports, and scoped admission. | Protected graders/labels, explicit missing outcomes, independent confirmation, and whole-task claims. |
| OPT | Signatures, implementations, study/data/search plans, candidates, materialization, trials, cost, and results. | The actual loaded candidate is measured; frozen meaning and data rights survive search; adoption remains separate. |
| KB | Immutable entries, heads, withdrawals, reader trust, and out-of-source evidence. | Preserve existing publish/sync validation; complete private entries and curated packages with provenance and rights checks. |
| CTRL | Task-scoped device grants, pairing, revocation, command admission, and bounded history views. | Independent rights, stale frames, replay, lost acknowledgments, handoff, and redacted catch-up cannot leak evidence or duplicate effects. |
| MKT | Offerings, private negotiation, mutually accepted orders, cancellation, and settlement evidence. | Exact terms survive restart and relay replacement; wallet authority stays separate; ambiguous payment remains unknown and cannot trigger a duplicate charge. |
| LAB | Execution linkage, deliverable identity, verification, acceptance, rework, disputes, and data rights. | Two independent operators complete a no-spend job, then separately verified payment; a passing test alone neither accepts a contract nor settles an invoice. |
| SESS | Engine feature matrix, persistent sessions, exact configuration, durable input queue, interactions, terminal causes, and native history/imports. | Native/emulated/unsupported remain visible; queue promotion waits for quiescence; imported history grants no execution; reconnect never repeats an unknown effect. |
| WS | Resource/document versions, conditional mutations, worktrees/checkpoints, finite projection cuts/pages/deltas, and command visibility. | Stale edits conflict; cross-store failures retain reconciliation; every client can distinguish stale, partial, inaccessible, and current views. |
| WORK | Native tracked work, planning graph, admitted revisions, delegation, evidence, disposition, imports, and attention. | Two clients agree on exact accepted revisions; reassignment cannot duplicate execution; issue close, verification, commercial acceptance, and payment stay distinct. |
| AUTO | Finite plans, schedule slots, verified source observations, checker continuation, counters, controls, and aggregate accounting. | Sleep, duplicates, cancellation, missing checkers, and uncertain effects produce bounded retained outcomes without automatic backlog bursts. |
| ENV | Allocation/adoption, exact materialization, participant closure, lease admission, required CJ binding, expiry, and cleanup. | No dispatch before attachment admission; unknown create/destroy outcomes retain resource and spending obligations; changed recipients require new admission. |
| LIVE | Participant/consent admission, transport epochs, input/speaking floors, capture anchors, and observation-bound device input. | Wrong-room audio, stale screenshots, revoked participants, lost stop acknowledgments, and cross-context disclosure refuse or remain explicitly unknown. |
| MV | World definitions, entity state, ephemeral poses/gestures, and scoped chat. | Preserve the standalone world's fixtures; this role does not imply physical-control or coding-task authority. |

## Delivery order

Build the inert domain contracts and local host path first. Complete bounded
candidate execution, evaluation isolation, durable accounting, and operator
adoption as one inspectable loop. Then demonstrate the same identities and
privacy guarantees over relay transport. Network conformance must not require
a particular optimizer or model vendor.

The [2026-09-26 coverage review](2026-09-26-openagents-gap-review.md) records
the CTRL/MKT/LAB additions and reuse across official and Block NIPs. Their
initial implementation can proceed alongside coding-quality work. Begin with
local durable state and a no-spend order/control flow; prove privacy and
recovery over relays before payment or cross-device conformance claims.
No new NIP is implemented merely by adding its document to the index.

The subsequent [81-document teardown review](2026-09-26-teardown-coverage.md)
adds SESS/WS/WORK/AUTO/ENV/LIVE and POL/EXT lifecycle contracts. Follow the
[Coder implementation sequence](../coder/design/teardown-nostr-integration.md#implementation-sequence-and-completion-evidence):
strict schemas and local durable admission, then two-client session/workspace
proof, tracked work and labor, bounded automation and placement, and media and
measured contribution reuse. Agent labor proceeds alongside the first local
contracts; it does not wait for every client or media feature.

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
