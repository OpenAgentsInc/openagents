# General agent architecture: remaining work

Status: design and implementation notes, 2026-09-21. These are explicit future
work items, not implemented features or newly created GitHub issues. The
[architecture](README.md) defines the shared core and domain boundary. The
[protocol implementation plan](../protocol/implementation-plan.md) remains the
queue for the existing NIP work; the items here extend its acceptance scope
or identify contracts that need a separate design before implementation.

All current OpenAgents additions remain v1 drafts. Update validators, data,
and fixtures together; older partial readers must refuse newly required
semantics. Existing CJ conversation versions and upstream specifications are
unchanged. No broad crate rename or new transport family is required now.

## Changes specified in this pass

| Area | Contract change | Implementation still required |
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
| `EVENT-1` — next | COORD/host design: define admitted external-event and scheduled triggers beyond the initial snapshot/operation/task events. | Authenticate event source/account, deduplicate, bound backlog, specify ordering and stale-event behavior, handle schedule/time-zone changes, and resume after downtime without duplicate effects or unbounded catch-up. |
| `AUTH-1` — next | POL/host design: define organization roles, multiple approvers, threshold approval, and revocation freshness when a domain requires them. | Membership changes, conflicting approvals, expiry, actor separation, and withdrawal have deterministic behavior. A package signature, agent-owner relationship, or list of signatures never substitutes for the required authority. |
| `MEDIA-1` — later | CTX/source adapters: standardize required image-region, audio/video-time, table-cell, or spatial anchors for an actual consumer. | Anchors bind exact bytes plus coordinate/time-base schemas; derived crops/transcripts preserve source provenance, loss, access restrictions, and model capability requirements. |
| `BUDGET-1` — domain-dependent | Host/policy: define enforceable domain quantities beyond compute spend, such as item counts, message volume, inventory, or transaction value. | Units and reservation boundaries are explicit; composing children cannot duplicate allowances; uncertain or irreversible commitments remain reserved/reconciled. No arbitrary new bound is ignored by an older host. |
| `EVAL-1` — with each profile | Gym/domain evaluator: add matched non-code task suites, adverse outcomes, and human-escalation accounting. | Report evidence quality, accepted output, wrong destinations, confirmed effects, refusals, unknowns, latency, and full cost. No transfer of coding calibration without new evidence. |
| `CONTROL-1` — before physical control | Specialized host/profile design: establish timing, units, interlocks, confirmation, and an independent stop mechanism for a concrete device. | Demonstrate required behavior under missed deadlines, disconnects, stale observations, and failed commands. Generic deadlines, approval signatures, and actuator descriptions alone do not establish physical-control suitability. |

The first implementation milestone is `GEN-1` through `GEN-3` plus `EVAL-1`
for research. `FX-1` can validate external mutation semantics with a mock
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
permission to act. General webhook and recurring-schedule semantics need
`EVENT-1`; NIP-ER reminders and NIP-PL wakeups do not supply that authority.

POL's current approval contract covers a named approver and exact action.
Group membership, departmental authority, multi-party approval, and escalation
must be implemented under explicit trusted policy and their supported contract.
Do not claim threshold approval merely because several signatures are available.

CTX already carries typed binary artifacts. Its existing byte ranges identify
stored content; they are not a universal spatial or temporal interpretation.
Full-image/document captures can be admitted with supported schemas while
`MEDIA-1` defines richer interoperable anchors for a concrete workload.

General infrastructure is a reusable foundation with explicit limits. Domain
support requires actual bindings, appropriate authority, failure handling, and
workload evidence before it becomes a product claim.
