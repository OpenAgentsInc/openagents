# General agent architecture: remaining work

Status: proposed design and implementation work. These local planning IDs are
not filed GitHub issues. The [architecture](README.md) defines shared agent
contracts and domain responsibilities. The [optimization proposals](../optimization/proposed-issues.md)
are the consolidated backlog for semantic contracts, DSPy/GEPA authoring,
Gym evaluation, and measured adoption.

Protocol schemas use v1. Implement only supported roles and advertise them
after conformance is demonstrated. A declared interface does not establish
host enforcement or domain readiness.

## Priority: agent labor

The [agent labor plan](market-infrastructure.md) is a high-priority track
alongside Coder's quality work. These local planning IDs are not GitHub issues.

[NIP-MKT](../../nips/openagents/NIP-MKT.md) and
[NIP-LAB](../../nips/openagents/NIP-LAB.md) now define the first draft contracts.
The strict parsers and a persisted free-only host slice now exist.
[Issue #9679](https://github.com/OpenAgentsInc/openagents/issues/9679) closed with
[retained synthetic relay evidence](../coder/verification/2026-09-26-free-labor/README.md):
one explicitly granted provider command, separate buyer verification, acceptance,
restart, and refusal cases. This completes a bounded implementation slice, not
the independent-operator or paid-market milestones below.

| ID | Work | Completion evidence |
| --- | --- | --- |
| `LABOR-1` | Implemented for the free-only reference profile: strict MKT/LAB parsing, durable role journals, and the admitted order/delivery/check/acceptance path. Review other profiles and historical imports separately. | [Runtime and evidence](../coder/runtime/free-labor.md) bind task/base, parties, exact terms, bounds, and outcomes. Paid profiles and a complete public service remain unsupported. |
| `LABOR-2` | Ship client intake and a bounded provider process with Go online/Pause controls, isolated job execution, retained patches/tests/traces, and acceptance review. | Two independent operators complete real repository jobs in a no-spend rehearsal; a compatible non-Coder executor can participate. |
| `LABOR-3` | Demonstrate restart, relay replacement, duplicate dispatch protection, cancellation, and uncertain-result reconciliation. | Two relay operators, worker crashes, late replies, stale bases, and unavailable buyers produce correct terminal or unknown states without repeated effects. |
| `LABOR-4` | Add an explicit Bitcoin payment adapter and agreed refund/dispute behavior. Keep worker compensation distinct from component royalties and licensed data. | A real outside operator receives payment for an accepted buyer job; confirmation, duplicate payment, timeout, and recovery cases retain exact receipts and liabilities. Test the rail before live use. |
| `LABOR-5` | Measure demand and provider economics, with sponsored work labeled. Connect consented outcomes to KB/EXT/OPT improvement. | Report repeat buyers, accepted jobs, total buyer cost, provider net earnings, subsidies, independent operators, and gains on tasks outside contribution sources. |

Compute and data services can support this labor market when demand exists.
Swap infrastructure, liquidity markets, and financial risk products are not
prerequisites. The general contract work below proceeds where these actual
consumers need it.

## Required architecture

| Area | Contract | Required implementation |
| --- | --- | --- |
| Product scope | General infrastructure with Coder as the first profile; domain-profile terminology without a new package kind. | Keep domain dependencies out of new shared consumers and advertise only proven profiles. |
| Sources and state | Shared `openagents.observation.v1`; CTX/evidence `external` sources with pinned adapters, version tokens, and declared read consistency. | Source adapters, retention, freshness/precondition checks, and parser fixtures. |
| Instructions | POL `scope` authority for independently trusted account/organization/collection policy, retaining `repository` for coding. | Trusted issuer mapping, scope resolution, conflict/precedence rules, and tenant isolation. |
| Findings and adoption | COORD generic typed `proposal`; domain-specific acceptance and integration. | Validate proposal schemas, admission, destination identity, and confirmation. |
| Effects | Shared meaning of external writes, explicit downstream assurance, unknown-effect reconciliation, and separate compensation. | Binding-specific dispatch, provider idempotency, conditional writes, and recovery proofs. |
| Measurement | EVAL workload-specific acceptance and adverse outcomes beyond tests/patches. | Non-code suites and independent complete-task evidence. |

## Work items

| ID and priority | Owner boundary and task | Completion evidence |
| --- | --- | --- |
| `GEN-1` — first | Protocol/domain layer: implement the generalized v1 schemas, resource identity checks, and evidence/observation cross-reference validation. Extend the shared-contract workstream. | Fixtures cover a repository, a document collection, and a mock service record; none silently coerces external observations into Git snapshots. |
| `GEN-2` — first | Host: build a read-only research reference profile using the same CAP/PRG/CTX/POL/RUN path. Add EXT packaging and EVAL fixtures. | A cited report and independent evidence check work with no repository or shell grant; local and remote jobs preserve the same identities and permitted disclosures. |
| `GEN-3` — first | Host/runtime: isolate reusable state, admission, workflow, and receipt interfaces as a second consumer exercises them. | Shared tests run without Coder environment variables, a current Git checkout, terminal state, or coding-only success criteria. Coder remains a consumer of the same engine. |
| `FX-1` — next | Adapter/runtime: implement an external-effect reference binding against a local mock versioned record service. Specify a portable effect-confirmation/reconciliation schema where current binding receipts cannot interoperate. | Concurrent out-of-band updates, dropped replies after commit, duplicate requests, absent idempotency, stale revisions, and failed compensation retain the correct effect/unknown state. No claim of cross-system atomicity. |
| `WAIT-1` — next | PRG/RUN/CJ design: define durable suspension and continuation for human input or external completion. | A wait records its correlation identity, admitted resume actors, deadline, retained pins, authority refresh, reservation treatment, and typed response schema. Duplicate, stale, unauthorized, cancelled, and post-expiry resumes cannot dispatch twice. |
| `EVENT-1` — next | Implement [AUTO](../../nips/openagents/NIP-AUTO.md)'s drafted finite UTC schedules, verified source observations, and checked continuation. Calendar/DST schedules remain a later profile. | Authenticate event source/account, deduplicate, bound and coalesce backlog, revalidate source revisions, retain unavailable checks and unknown effects, and recover after downtime without duplicate dispatch. |
| `AUTH-1` — next | POL/host design: define organization roles, multiple approvers, threshold approval, and revocation freshness when a domain requires them. | Membership changes, conflicting approvals, expiry, actor separation, and withdrawal have deterministic behavior. A package signature, agent-owner relationship, or list of signatures never substitutes for the required authority. |
| `MEDIA-1` — later | Implement [LIVE](../../nips/openagents/NIP-LIVE.md)'s image-region and audio/video-time anchors with admitted capture and source bindings. Table-cell and other domain/spatial profiles remain later designs. | Anchors bind exact bytes plus coordinate/time-base schemas; derived crops/transcripts preserve source provenance, loss, access restrictions, and model capability requirements. |
| `BUDGET-1` — domain-dependent | Host/policy: define enforceable domain quantities beyond compute spend, such as item counts, message volume, inventory, or transaction value. | Units and reservation boundaries are explicit; composing children cannot duplicate allowances; uncertain or irreversible commitments remain reserved/reconciled. No arbitrary new bound is ignored by an older host. |
| `EVAL-1` — with each profile | Gym/domain evaluator: add matched non-code task suites, adverse outcomes, and human-escalation accounting. | Report evidence quality, accepted output, wrong destinations, confirmed effects, refusals, unknowns, latency, and full cost. No transfer of coding calibration without new evidence. |
| `CONTROL-1` — before physical control | Specialized host/profile design: establish timing, units, interlocks, confirmation, and an independent stop mechanism for a concrete device. | Demonstrate required behavior under missed deadlines, disconnects, stale observations, and failed commands. Generic deadlines, approval signatures, and actuator descriptions alone do not establish physical-control suitability. |

The research-profile milestone is `GEN-1` through `GEN-3` plus `EVAL-1`;
it does not block `LABOR-1` through `LABOR-5`. `FX-1` can validate external
mutation semantics with a mock
service before enabling any real account changes. The remaining designs should
be driven by a concrete profile and a consumer, not by adding unused event kinds.

## Boundaries until those items land

PRG v1 remains a bounded acyclic workflow with explicit attempts and composition
limits. Do not represent a months-long human wait as a running subprocess,
extend an expired CJ request silently, or treat a persisted task frame as an
already authorized continuation. A new run can link the prior work and undergo
fresh admission; first-class suspension needs `WAIT-1`.

COORD's current background triggers are `snapshot_changed`,
`operation_completed`, and `task_closed`. A host can ingest external data through
an admitted source, but a signed incoming event is not automatically a job or
permission to act. AUTO now supplies a separate finite-plan specification;
`EVENT-1` implements it without changing COORD's initial trigger enum.
NIP-ER reminders and NIP-PL wakeups do not supply that authority. Webhooks
may wake an admitted source poller; they do not create execution permission.

POL's current approval contract covers a named approver and exact action.
Group membership, departmental authority, multi-party approval, and escalation
must be implemented under explicit trusted policy and their supported contract.
Do not claim threshold approval merely because several signatures are available.

CTX already carries typed binary artifacts. Its existing byte ranges identify
stored content; they are not a universal spatial or temporal interpretation.
LIVE now defines image/time anchors for admitted captures. `MEDIA-1` supplies
their validators and concrete source adapters; other domain interpretations
still require supported schemas.

The [teardown integration plan](../coder/design/teardown-nostr-integration.md)
adds SESS, WS, WORK, and ENV for persistent engine control, workspace resources
and projections, tracked objectives, and runtime placement. Its implementation
gates complement this backlog. Neither those drafts nor AUTO/LIVE silently
add a PRG wait, threshold approval, editor algorithm, or physical-device
guarantee.

General infrastructure is a reusable foundation with explicit limits. Domain
support requires actual bindings, appropriate authority, failure handling, and
workload evidence before it becomes a product claim.
