# A router for agentic execution

2026-10-02. Implementation plan, not a shipped public service. This plan expands
[the OpenAgents API design](2026-10-02-openagents-api.md); it does not replace its
owner decisions. New contracts and launch gates below are proposals. The
[PPQ research](../research/ppq.md) motivates payment interoperability, not a
claim that PPQ implements this architecture.

## 1. Product thesis

The October 5 [terminal workbench roadmap](../terminal/workbench-roadmap.md)
makes the Grid overlay and standalone terminal the next client slice, then
connects Everglade's Agent Studio. The contract and local exit evidence
below are current foundations; the public and paid service remains a plan.

**OpenAgents routes work, not only tokens.** A caller describes an outcome. The
router selects an admitted capability, places execution where the caller has
authority, runs it through an existing executor, checks the result, and returns
an attributable record of what happened and what it cost.

A model gateway answers “which model should produce this response?” This service
also answers:

- Does this request need execution, or can an existing answer satisfy it?
- Which plugin, program, model, or Coder route can perform the work?
- Which computer may run it, with which source, tools, and disclosure policy?
- What needs an explicit offer or approval before it runs?
- What evidence establishes completion, and what happens when evidence is missing?
- Who receives the payment for the resources actually used?

The unit of value is a **verified task outcome**, where verification has a
stated scope. A process exit, model judgment, passing test, and buyer acceptance
are different facts. None alone proves every kind of outcome.

### First customer and first task

Start with partner applications that want repository work on a user's paired
computer without implementing engine selection, task supervision, history,
and recovery themselves. Also serve self-hosters and the existing apps through
the same domain contracts.

The first execution slice is narrow: one request, one explicitly granted
computer and repository, one admitted Coder executor, retained artifacts, and
an independent check. For example, “update this repository's installation guide
and return a patch.” Committing, pushing, publishing, and deploying require
those effects to be included in the grant; returning a patch does not imply
permission to publish it.

### Non-goals for the first release

- A catalog of hundreds of models as the primary product.
- Arbitrary paid workers, bidding, escrow, dispute resolution, or paid labor.
- Execution on OpenAgents' computers or the owner's computers by default.
- Recursive agent spawning, unbounded parallel work, or model-selected delegation.
  A fan-out the caller asks for in so many words ("one per agent", "ask all
  three") is not model-selected: it is a bounded, typed dispatch plan (section
  13.2) and belongs in the local slice.
- A new coding loop or a second permission system.
- Claims of confidential computing or remote attestation from signed receipts.
- New limits on the users of OpenAgents' own apps. Follow API decision D16;
  execution grants, physical capacity, and correctness checks are not pricing tiers.

## 2. Existing foundations and missing integration

| Foundation | Reuse | Work still required |
| --- | --- | --- |
| [Chat router](../coder/design/2026-09-28-chat-router.md) | Typed Jev judgments, deterministic route policy, offers, labeled evaluation | An execution admission snapshot and task-level outcome feedback |
| [Task owner](../coder/runtime/task-owner.md) | Durable task identity, command dispositions, retained evidence, independent checks | Public HTTP projection, route-to-task binding, and recovery reconciliation |
| [Host autostart](../coder/runtime/host-autostart.md) | Operator-selected local execution policy | Partner-key scope and current grant checks at dispatch |
| [Microcoder](../coder/guides/microcoder.md) | The existing Rust loop and provider capacity handling | A bounded admitted-route interface, not a fork of the loop |
| [Execution boundary](../coder/verification/2026-09-20-execution-boundary.md) | Filesystem enforcement and workspace snapshots | Explicit per-route enforcement declarations and admission refusal on unsupported hosts |
| [Subprocess supervisor](../coder/runtime/subprocesses.md) | Process groups, cancellation, output bounds | Map transport cancellation to executor disposition |
| [Free labor host](../coder/runtime/free-labor.md) | Separation of order, execution authority, delivery, checking, and acceptance | Paid labor remains unsupported; do not expose it as a working route |
| [API design](2026-10-02-openagents-api.md) | Messages, threads, computers, runs, offers, sats pricing, HTTP conventions | The public agent service is still design work |
| [Payments design](../payments/README.md) | Central receiving, split ledger, author fees, payout reconciliation | The shared paid execution path and its durable settlement integration |

Existing decision-gateway authentication and quota patterns are useful references,
not evidence that the agent API already supports sats accounts or execution.
Keep decision-serving reservations distinct from the agent's money ledger.

## 3. End-to-end architecture

```text
HTTP message + thread + caller constraints
    -> authentication and caller scope
    -> bounded context and admitted capability discovery
    -> Jev semantic judgments
    -> deterministic route policy
    -> immutable route proposal or offer
    -> execution authority + disclosure + funding admission
    -> durable dispatch intent
    -> existing executor on an admitted computer
    -> artifact retention + independent checks
    -> result projection + cost settlement + receipts
```

Implement product logic in Rust. Keep HTTP and Nostr adapters thin; they translate
transport messages into domain operations, not separate workflows. Share the
route policy with app entry points instead of letting each surface interpret
Jev answers independently.

### Suggested ownership

- `coder::router`: semantic route selection and policy over admitted candidates.
- The task owner and host: command execution, lifecycle, current grant checks,
  local state, and uncertain-effect recovery.
- Existing capability, boundary, and supervisor crates: capability trust and
  enforcement; never replace their checks with generated instructions.
- A proposed agent API adapter: HTTP identities, thread projection, offers,
  streams, and translation into existing task operations.
- Wallet, x402, and the proposed payments ledger: payment validation, replay
  claims, accounting, and payout state.
- ATIF and receipts: execution evidence and attributable summaries.

Choose the API crate boundary in the first implementation change. Do not add
execution to the typed-decision gateway merely because it already accepts HTTP.

**Chosen (phase 0, #10205):** the contract is its own crate,
[`crates/route-contract`](../../crates/route-contract/src/lib.rs), with only
`serde`, `serde_json`, and `sha2` as dependencies. Every client (terminal, CLI,
desktop, phone, and the future HTTP adapter) reads the same types without
linking Coder, and neither `coder::router` nor the gateway owns them. Coder
depends on it; the reverse never happens. The HTTP adapter will be a separate
crate that depends on it.

## 4. Admission and route contract

Build the eligible candidate set in code **before** asking Jev to rank it.
A registry entry is not permission to execute its probe or use its tools.
Unknown capability or capacity is not positive evidence of availability.

The proposed immutable admission snapshot binds:

| Field group | Required content |
| --- | --- |
| Identity | Caller, workspace, request, thread, task, and attempt identifiers |
| Input | Exact request digest, source revision or snapshot, instruction digests |
| Route | Capability and adapter digests, executor revision, selected model, policy and question-set versions |
| Placement | Computer identity, workspace binding, grant identity and revocation epoch |
| Effects | Allowed reads, writes, network destinations, commands, and publication effects |
| Disclosure | Permitted recipients, provider identities, allowed context and artifact classes |
| Resources | Operator execution constraints, available capacity, caller-selected ceilings |
| Money | Price-book version, quote, fee recipients, reservation and settlement identities |
| Evidence | Deliverable definitions, checker identity and criteria, retention policy |

Use content digests for immutable definitions and opaque identifiers for public
references. Public summaries omit private paths, secrets, and internal Jev scores.
A signed digest establishes attribution, not execution truth or hardware attestation.

### Keep four authorities separate

1. **Observation:** permission to read a thread, task, host state, or artifact.
2. **Execution:** permission from the computer's operator to run the selected work.
3. **Disclosure:** permission to send specific material to named model or tool providers.
4. **Spending:** permission to reserve and settle the quoted charges.

Pairing, payment, a plugin installation, or creating an inbox task does not grant
the other authorities. A partner key reaches only computers explicitly granted
to that key. Host admission checks current rights again when dispatching;
long-running channels follow the host's current revocation rules.

An offer is an immutable proposal with an expiry and a digest of the effects,
recipients, price terms, and source. Confirming it cannot approve a changed
proposal. If any material field changes, issue a new offer. Existing local
autostart can supply execution authority only within its configured bounds.

## 5. Routing policy

Use Jev for semantic ambiguity: whether the request calls for repository work,
which admitted capability fits, whether evidence supports an answer, and whether
an unresolved request needs clarification. Code handles exact scopes, money,
capacity, identity, retries, placement, and grants.

Follow [TypeSafe's confidence guidance](https://docs.typesafe.ai/confidence.md):
model confidence is a signal to calibrate against labeled outcomes, not an
intrinsic authorization threshold. Do not ship arbitrary probability cutoffs.
Freeze question sets and policy versions, tune on a separate split, and retain
untouched test evidence before promoting an execution route.

### Route families

| Family | Behavior |
| --- | --- |
| Prepared or knowledge answer | Return an answer with checked citations where applicable; no computer execution |
| Model answer | Generate through an admitted provider under the disclosure policy |
| Plugin or program | Run the pinned admitted capability with validated typed arguments |
| Coder | Start or continue a durable task on a granted computer |
| Missing capability | Offer to build or install a capability; do not install it implicitly |
| Clarification or refusal | Explain missing authority, ambiguous intent, or unavailable capability |

### Selection order

1. Honor an explicit caller route if it is admitted. Never silently substitute
   a route excluded by caller constraints.
2. Eliminate candidates that violate authority, disclosure, source requirements,
   caller-selected limits, or enforcement requirements.
3. Apply deterministic capacity and placement rules.
4. Rank remaining candidates for task fit using measured route evidence and
   semantic judgments. Initially use a transparent ordered policy, not an opaque
   learned optimizer.
5. Prefer the least expensive adequate route only when its quality and latency
   evidence satisfy the task class. “Cheapest” is not synonymous with “adequate.”
6. Return a clarification, offer, or refusal when no route meets the contract.

Track quality, latency, and cost by task class and route version. Avoid rewarding
routes for declaring success without artifacts. Separate selection bias from
observed performance; use fixture comparisons before traffic experiments.

### Fallback is a new admission check

A rate-limited model can fall back only to a provider already allowed by the
snapshot's disclosure and price terms. A new recipient, computer, stronger
permission, plugin fee, or publication effect requires a new offer.

Do not restart repository execution elsewhere after a transport timeout. First
reconcile the original task. Once commands can have effects, “retry the request”
is not equivalent to “run the commands again.”

## 6. Durable lifecycle and recovery

Proposed lifecycle states are domain concepts to map onto the task owner's
existing dispositions, not permission to replace its journal:

```text
received -> proposed -> awaiting_authority_or_payment -> admitted
    -> dispatch_pending -> running -> checking -> completed
                                      -> failed
                                      -> cancelled
                                      -> needs_reconciliation
```

Each state transition records its cause, prior revision, attempt identity, and
artifact references. A completed task requires the declared deliverables and
recorded check disposition. A task can finish with a clearly labeled unverifiable
result; it must not present that as verified success.

### Idempotency

- Scope HTTP idempotency keys to caller and operation. Bind them to exact
  accepted request bytes or a documented canonical digest.
- Same key and same request returns the existing resource or result. Same key
  and different request returns a conflict.
- Persist admission and dispatch intent before contacting the executor.
- Bind one execution identity to one admitted task. Redelivery must not spawn
  a second task. Keep transport attempt identifiers separate from task identity.
- Keep payment consumption, task dispatch, and payout obligations separately
  journaled; reconcile crashes between them rather than claiming a distributed
  transaction exists.

### Failure handling

| Failure | Required behavior |
| --- | --- |
| Host offline before admission | Report unavailable or retain an explicitly authorized queue; do not choose an unauthorized host |
| Dispatch acknowledgment lost | Query the same execution identity; mark uncertainty until resolved |
| API restart during a run | Reattach to the task owner and stream from retained evidence |
| Executor crash after a command | Preserve uncertain effects; never replay the command automatically |
| Grant revoked | Stop new admissions and follow current host cancellation rules; retain an honest final disposition |
| Cancellation requested | Record intent, terminate through the supervisor, and report acknowledged cancellation separately from requested cancellation |
| Checker fails | Return failed verification and evidence; rework requires remaining authority and funding |
| Artifact missing | Report delivery incomplete; do not treat a status flag as artifact availability |
| Payout unavailable | Keep a durable payout obligation; task success does not depend on immediate payout routing |

Disconnecting an SSE client does not cancel a task. An explicit cancellation
operation does. Resumption cannot imply exactly-once delivery of stream events;
provide sequence identifiers and explicit gaps when retained events expire.

## 7. Public experience

Reuse the API design's resources: `POST /v1/messages`, threads, computers,
runs, offers, usage, and model discovery. Do not invent a parallel “agent jobs”
API with a different authority model.

A proposed execution flow is:

1. The client submits a message and optional thread, computer, and caller limits.
2. The response identifies the request and route, or returns a confirmable offer.
3. Funding and execution authority admit the exact proposal.
4. The client receives a run identifier and resumes observation through the run
   resource even if the originating stream closes.
5. The final projection includes deliverables, check scope and outcome, selected
   route and model, elapsed time, settled cost, and any remaining uncertainty.

Use the established API error envelope and codes. Proposed subreasons distinguish
missing execution grant, denied disclosure, unavailable host, stale offer,
unverifiable evidence, and recovery in progress. Final code names need review
against the existing error table before implementation.

Show understandable route explanations such as “Coder on your granted computer.”
Expose no internal Jev probabilities. A caller can inspect an attributable route
record without receiving private prompts or another tenant's traces.

## 8. Prices, reservations, and payments

Keep prices in sats. Follow API decisions D3, D7, D9, D10, D12, and D13:
Lightning only, prepaid balances or per-call payment, author-declared plugin
fees paid in full after settlement, and x402 plus MPP from one invoice and
one replay store. Classic L402 is not a first-release dependency.

### Launch pricing

Start with fixed-price task classes or prepaid balance reservations under
caller-selected ceilings. Publish the route's price terms before admission.
Do not advertise an exact upfront price for arbitrary route-dependent work.
Metered MPP Lightning sessions are a later dependency for streaming costs,
not something existing x402 `exact` already provides.

For every charge, retain a versioned breakdown:

- Model or executor resource charge and its pricing basis.
- OpenAgents routing and coordination charge.
- Each plugin's declared fee and author identity.
- Quoted maximum, reserved amount, actual charge, and unused reservation release.
- Separately disclosed Lightning payment fees and payout costs under the
  payments design's rules; do not silently deduct them from an author's fee.

Prevent parallel calls from overspending a balance or caller-set budget through
durable reservations. Unknown costs remain held for reconciliation. Never free
an uncertain hold because a process restarted.

### Payment and execution are separate state machines

An unpaid challenge performs no execution. Bind proof to request body, method,
resource, amount, expiry, and quote identity. Consume its payment hash atomically
across x402 and MPP, but let an exact idempotent retry retrieve the already-funded
resource without consuming or charging again.

Test a crash after payment consumption but before task creation. Recovery must
find the funded request and create or recover only its original task. Test a
crash after settlement but before author payout bookkeeping: a stable settlement
source recreates the same obligation, not a second payout.

Apply the API design's current failure policy: a paid call that fails before any
answer can retry free under the same idempotency key; no automatic refund for a
settled charge. Retrying observation is always safe; retrying execution is allowed
only after the original disposition is reconciled. Release unused balance holds
without describing that as a refund of a settled payment. MPP session remainder
refunds follow their separate session policy and need durable retry tracking.

## 9. Evidence, privacy, and operational visibility

Record route policy and question-set digests, admission identity, execution
attempts, command dispositions, deliverable digests, check results, and settlement
references. Link ATIF evidence rather than copying entire private traces into
public receipts.

Treat repository files, retrieved documents, plugin output, and model text as
untrusted content. None can grant authority or edit the admitted recipient set.
Validate typed arguments in code, pin executable adapters, and refuse unsupported
enforcement. Full-access execution requires an explicit operator choice and
must be visible in the admission record.

Keep secrets out of logs, receipts, URLs, and public statistics. Result URLs for
keyless callers are narrow, expiring bearer capabilities, with separate read and
cancel authority. Redact query credentials from access logs, constrain artifact
size and content types, and define retention and deletion before public launch.
Tenant checks apply to every thread, run, event cursor, and artifact fetch.

Measure:

- Time to route, admission, first execution event, and final checked result.
- Route confusion, unnecessary offers, and missed execution requests.
- Unsupported success claims and verification failures.
- Cost per checked outcome, reservations awaiting reconciliation, and payout age.
- Capacity refusals, provider failover, transport gaps, and cancellation latency.
- Duplicate execution, unauthorized disclosure, and cross-tenant access attempts.

Alert on uncertain dispatches, replay conflicts, ledger disagreement, and stuck
payout obligations. Public live payment views use the payments design's sanitized
aggregates, not private task text or computer identifiers.

## 10. Delivery plan and acceptance gates

Each phase is a small set of independently reviewable Rust changes with targeted
crate tests. Documentation-only changes need no Rust gate. Do not run live engine
or payment probes on the owner's machines as routine acceptance.

| Phase | Deliverables | Exit evidence |
| --- | --- | --- |
| 0. Freeze the first contract | Task class, route/admission DTOs, effect and disclosure schema, failure policy, labeled evaluation split | Reviewed mappings to API decisions and existing host authority; no unexplained duplicate state machine |
| 1. Local vertical slice | One message routed into the existing task owner in a scratch host; one retained patch and checker result | Synthetic fixtures prove successful delivery, denied grant, cancellation, and executor crash without duplicate effects |
| 2. HTTP projection | Caller-scoped threads, offers, run reads, resumable events, idempotent creation | HTTP contract fixtures prove conflict detection, isolation, restart reattachment, and stream gaps |
| 3. Granted remote computer | Partner-key computer binding and current host admission; retained remote artifacts | Scratch identities and hosts prove revoked/stale grants refuse, lost acknowledgment reconciles, and no implicit host substitution occurs |
| 4. Paid admission | Sats reservations, one fixed-price route, x402 and MPP shared replay claims, settlement and author obligations | Fake-wallet and fault-injection tests prove no unpaid dispatch, no cross-scheme double spend, and crash-safe funded-task recovery |
| 5. Capability routing | A small reviewed plugin catalog and model fallback within admitted disclosure | Held-out evaluation plus adversarial fixtures prove excluded routes cannot run and fallback cannot widen authority |
| 6. Metered and composed work | MPP sessions, durable remainder handling, bounded execution graphs, explicit rework | Conservation and restart tests cover every hold, debit, payout, and refund; graph cancellation and per-node evidence stay attributable |

### Phase 0 exit evidence: what was frozen (2026-10-02, #10205)

Contract version 1 lives in
[`crates/route-contract`](../../crates/route-contract/src/lib.rs). Schemas:
`openagents.route.admission-snapshot.v1`, `openagents.route.result.v1`,
`openagents.route.offer.v1`, `openagents.route.transition.v1`, and
`openagents.route.eval-split.v1`. A breaking change is a new version, never an
edit in place; golden digests in the crate's tests fail if a field moves.

- **Admission snapshot** (`snapshot.rs`): the nine field groups of section 4
  plus `defaults_applied` (13.1) and `inherits`. Effects carry reads, writes,
  network, commands, publication, access (`full` visible), and the macOS deny
  set (13.8: Music, Photos, Documents, Desktop, Downloads, Mail, Messages,
  Contacts, Calendars, iCloud Drive, and control of other apps). Money carries
  the BYOK mode (`ours`/`mine`), the payer per resource (OpenAgents, the
  caller's provider key, or the caller's own engine login), funding, quote,
  plugin fees, reservation, settlement, and whether the cost is shown (13.6).
  `AdmissionSnapshot::widens(parent)` is the single check for a fallback
  (section 5) and a continuation (13.3): a new computer, workspace, recipient,
  wider effect, weaker deny set, payer switched off the caller's keys, or new
  plugin fee each needs a new offer.
- **Route result** (`route.rs`): answer (prepared, knowledge, model), local
  command (a command with its effect class, or a screen; read-only runs at
  once, state changes confirm, money and secrets never run from chat), plugin
  (run a pinned capability, or the creation flow of #10177), Coder with a
  **dispatch plan** (task class including issue work, fan-out
  single/one-per-engine/named, 1 to 8 runs each with engine, how it was
  chosen, read-only or write, input digest, and an optional continuation as
  next turn or steer; a summary is composed, never another run), standing rule
  (define/edit/pause/remove a pinned `openagents.background.rule.v1` document),
  missing capability (install, build, or none), clarification (ambiguous, no
  default, crosses authority), and refusal. Refusal reasons map to the API
  error table where a code exists (`computer_not_granted`,
  `route_not_allowed`, `computer_offline`, `limit_reached`); the rest are the
  section 7 subreasons still to review.
- **Offer** (`offer.rs`): action words as the API's (`run.start`, ...),
  expiry, and a digest over action, times, and terms (route digest, snapshot
  digest, computer, effects, recipients, price with fees, source). A
  confirmation of another digest, after expiry, or against changed terms is
  refused.
- **Lifecycle** (`lifecycle.rs`): the router owns only `received`,
  `proposed`, `awaiting_authority_or_payment`, and `admitted`; every later
  state is a pure projection of the task owner's `(status, execution,
  checks)`. No second state machine:

  | Task owner | Lifecycle | Check label |
  | --- | --- | --- |
  | status or execution `unknown` | `needs_reconciliation` | |
  | `queued` | `dispatch_pending` | |
  | `cancelled` | `cancelled` | |
  | `running`/`cancel_requested`, execution `not_started` | `dispatch_pending` | |
  | `running`/`cancel_requested`, execution `running` | `running` (`cancel_requested` flagged) | |
  | `finished`, execution `finished`, checks `running` | `checking` | |
  | ... checks `passed` | `completed` | `verified` |
  | ... checks `not_run` | `completed` | `unchecked` |
  | ... checks `unavailable` | `completed` | `unverifiable` |
  | ... checks `disputed` | `completed` | `disputed` |
  | ... checks `failed` | `failed` | `check_failed` |
  | `finished`, execution `failed` | `failed` | |
  | `finished`, execution `stopped` | `cancelled` | |
  | any other combination | `needs_reconciliation` | |

  The contract's state words match `coder::task::{Status, Execution, Checks}`
  today. The `coder::task::lifecycle` adapter, whose test keeps them equal,
  lands with phase 1 (#10207).
- **Evaluation split**
  ([`fixtures/route-families-v1.json`](../../crates/route-contract/fixtures/route-families-v1.json)):
  197 rows, every family in both `tune` and `test`. Seeded from every
  `wallet-v1` row, every `coder_followup` row of `routes-v4` (held-out stays
  test), and three tune plus three held-out rows of each other chat route; new
  rows cover dispatch plans, issue work, continue and steer, standing rules,
  plugin creation, local commands, and defaults. A test checks every seeded
  row against its source.

Phase 1 (#10207) wires it into the terminal and the CLI; see below.

### Phase 1 exit evidence: the local vertical slice (2026-10-02, #10207)

OpenAgents Terminal and `openagents chat` send every message through the
router contract.

- **One route policy.** [`openagents_chat::route`](../../crates/openagents-chat/src/route.rs)
  (`route-policy-v1`) is the only reader of the worker's typed judgment and
  offers: `propose` turns a reply into a `RouteResult` (Coder with a dispatch
  plan of one run or N, continuing the thread's local task as steer or next
  turn; a local command with its effect read from this computer's own command
  tree; a screen; a Gym program or a step of the plugin-creation flow
  (#10177); missing
  capability; clarification; refusal; answer), and `admit` builds the
  admission snapshot (placement on this computer, effects with access `full`
  visible and the macOS deny set, recipients per engine provider, the payer
  per resource with each engine on the person's own login, cost recorded and
  not shown, the delegate settings' digest as the Coder adapter, defaults
  applied, and `inherits` for a continuation, which widens nothing).
  `delegation::offered`, which the desktop, phone, host, and chat app call, is
  now the policy's `coder_offered`; the terminal and CLI read the family the
  client attaches to each reply instead of interpreting offers.
- **One record per message.** A new document beside the frozen ones,
  [`route_contract::record`](../../crates/route-contract/src/record.rs)
  (`openagents.route.record.v1`), binds the route result and the snapshot by
  digest, the router's own moves (only `Lifecycle::router_step`), each task the
  route started with its lifecycle projected from the task owner, and per-run
  cost and wall time. Cost and time live on the record, bound to the snapshot
  digest, because the snapshot is immutable and digested before dispatch. The
  client keeps it in the thread's route journal
  (`~/.openagents/routes/<thread>.jsonl`) before anything runs and settles it
  when each operation ends ([route records](../cli/chat.md#route-records)).
- **Dispatch through the task owner.** A Coder route starts exactly one task
  through the local runner (the same submission, execution grant, and store a
  host's auto-start uses) or, through a host, its `RunCoder`; a dispatch plan
  starts its N. A request whose record already names tasks is followed, never
  started again (this closes a `run-coder` after a plan starting the plan a
  second time), and a confirmed command runs once per message.
- **The lifecycle adapter.** [`coder::task::lifecycle`](../../crates/coder/src/task/lifecycle.rs)
  maps `coder::task::{Status, Execution, Checks}` onto the contract's words
  with exhaustive matches, and a test keeps the serde words equal both ways.
- **Fixtures.** [`task/owner/route_tests.rs`](../../crates/coder/src/task/owner/route_tests.rs)
  routes one message with the shared policy, journals its admission, and
  dispatches it into the real task owner in a scratch store:
  - delivery: one task, a retained patch, the independent check passes, the
    route ends `completed`/`verified` with the run's wall time and cost (unknown
    for the bounded fixture command, never a stand-in zero); asking again
    follows it, and the effect counter stays at one;
  - denied grant: a grant revoked after admission refuses before the executor
    (`failed`, `missing_grant`, no run), and a grant the owner refuses admits
    no run and ends the task `cancelled`;
  - cancellation: requested while running, reported apart from the
    acknowledged `cancelled`, with the later effect never written;
  - executor crash after dispatch: the route stays unsettled (running or
    `needs_reconciliation`) until recovery ends the run, then `failed`; the same message is followed, and the
    owner refuses a second execution; the effect counter stays at one.

  The policy's own fixtures (`crates/openagents-chat/src/route/tests.rs`) cover
  every family, plans, continuation without widening, command effects, issue
  work, and the journal; `route-contract` tests the record's moves.

Local runs' checks (#10232): every writing run's grant now carries one
host-written suite, frozen and pinned by digest before the candidate exists
(`coder::task::local_checks`). When the run ends its owner lists the
recipe's frozen checks and `cargo test -p` for each Cargo package the run
touched, records the check's intent, and runs the suite through the task
owner's independent check on the exact candidate, read-only; the client
waits for the verdict before it keeps the route record. A live terminal run
so ends `verified` or `check_failed`, and stays `unchecked` only when there
was nothing to run. What phase 1 does not do yet: the issue flow is refined to `issue_work`
after admission and still starts at once rather than as an offer; standing
rules have no reading on the wire (#10157), though their route result admits
and journals like any other.

Do not block the initial local/HTTP slice on paid labor or metered sessions.
Launch paid traffic only after phase 4's accounting and recovery gates pass.
Roll out one task class and a small opt-in partner cohort first. Keep route
promotion reversible without relabeling tasks already in flight.

### Test matrix

Use scripted executors, fake clocks, fake wallets, temporary homes, and scratch
host identities. Pin the route/question versions and keep tuning data separate
from test data. Include:

- Answer-only, execution, ambiguous, and missing-capability requests.
- Explicit route requests that conflict with grants or caller limits.
- Prompt injection that asks a tool to expand permissions or disclose secrets.
- Two concurrent admissions spending the same remaining balance.
- Same idempotency key with both identical and changed request bodies.
- Duplicate payment proofs through different encodings and concurrent instances.
- Process death at every persistence-to-dispatch and ledger-to-payout boundary.
- Model failover to both allowed and forbidden recipients.
- Partial output, missing artifacts, failed checks, and a model's false success claim.
- Revocation and cancellation while execution or artifact delivery is in progress.
- Cross-tenant run identifiers, event cursors, and expired result capabilities.

For hard authority and accounting properties, any fixture violation blocks
promotion. For semantic routing, report confusion by task class and confidence
intervals; establish numeric quality and latency gates from phase 1 measurements
rather than presenting unmeasured targets as facts.

## 11. Later composition

After single-task recovery works, represent composed work as an explicit graph:
each node has an admitted capability, input artifact references, dependencies,
resources, spending authority, and completion criteria. The host controls fan-out.
Models can propose a graph; they cannot authorize or launch it.

Reuse knowledge and checked artifacts across nodes only under source and tenant
permissions. Bound concurrency in the operator's execution grant. Propagate
cancellation while preserving the disposition of already-running nodes. Charge
and verify per node, then summarize the parent without hiding partial failure.

A paid worker market is a separate project. It requires commercial agreements,
delivery and buyer acceptance, settlement, rework, and dispute semantics beyond
the current free-only labor host. This router can eventually admit such a
capability; it must not pretend a remote subprocess is a complete labor contract.

## 12. Decisions to resolve during implementation

These questions do not block writing the plan or the local slice:

1. Which existing thread store should own the public API's retained turns, and
   how do self-hosters configure retention and export?
2. Which initial task classes have a defensible fixed price, and which require
   a prepaid reservation or a metered session?
3. Which checker contracts distinguish patch delivery, repository tests, and
   publication effects without overstating their evidence?
4. How does a keyless caller recover an expired result capability without
   exposing a paid artifact to someone holding only a public request identifier?
5. Which route explanations are useful to partners without exposing private
   policy state or internal judgments?
6. What measured routing error and latency levels justify each task class's
   promotion beyond the opt-in cohort?

## 13. Additions from dogfooding the apps (2026-10-02)

The owner used OpenAgents Terminal, the phone, and the CLI all day on
2026-10-02; each item below is a failure seen in a real chat and the rule the
router must keep. They refine sections 4 to 9 rather than replace them.

### 13.1 Defaults before questions

A clarifying question is a cost the caller pays in time. Before asking, the
router resolves ambiguity with product defaults, recorded in the admission
snapshot as defaults applied: the built-in wallet (Spark on every device,
never "which wallet?"), this computer, the current project, the current
thread's running or last session, every coding agent that is signed in and not
disabled (opt-out, not opt-in). Ask only when no default applies or a default
would cross an authority line. Each default gets a regression eval that fails
if the question comes back: the wallet set
([`wallet-v1.json`](../../crates/coder/fixtures/chat-router/wallet-v1.json),
#10170) is the model.

### 13.2 Dispatch plans, not one run per message

"Do 3 read-only delegations, one per agent" became one Codex run that
refused because it could not start other engines. The route result is a typed
**dispatch plan**: the number of runs, which executors (resolved from the
signed-in, enabled set), the mode (read-only enforced by the executor's
policy, not the prompt), and each run's input. The host starts the runs; an
engine never starts other engines or calls the host's control socket itself.
A requested summary is composed from the runs' results, not by a further run.
This is the first, flat case of section 11's graph.

### 13.3 Continue the session, steer the run

A follow-up to a run is routed (Jev, against what the run did) to: more work
in the same task and engine session, a question the chat answers, or a new
task. Continuing resumes the engine's own session in the same worktree; a
message to a running run is delivered to it at its next step (#10171). The
admission snapshot of a continuation inherits the original grant and
disclosure; anything wider is a new offer.

### 13.4 Direct commands are a route family

Read-only questions about this computer ("what's my balance") are answered by
running a command from the computer's own command tree, checked against that
tree rather than the proposer's claim: read-only runs at once, a state change
shows the exact command and needs Enter, money movement and secrets never run
from chat (#10170). Add **Local command** to section 5's route families,
between "Plugin or program" and "Coder". Its answer reports the result in plain
words, never the tool's internals (node ids, networks, channels).

### 13.5 Work an issue

"Pick an open issue nobody is working on and do it" is a task class with an
existing executor: the issue flow (claim, worktree of `origin/main`, checks,
land, close), started through the host, never by an engine poking the host.
Claims are one record every path writes and reads: the comment marker, an
assignee, and the GitHub Project status when the issue is on one (#10203).
The flow's gate blocks only on real failures: a failing test is retried, then
compared with the base, so flaky or pre-existing failures do not block
(#10145); style findings are advice for review. This is a strong candidate for
the first public task class, since its checks and landing already exist.

### 13.6 Who pays is part of the admission

A caller may send their own provider key ([BYOK](../byok/2026-10-02-byok-openrouter.md),
`OpenAgents-Provider-Key`). The snapshot records the payer per resource. A
fallback can never switch the payer to OpenAgents when the caller chose their
own keys; it fails plainly instead. Costs are recorded for every run on every
path (#10161) and shown to API callers; OpenAgents' own apps record them
without showing them.

### 13.7 Host upgrades are a failure mode

The host restarted for upgrades several times mid-conversation and clients saw
"cannot be reached" and "write failed" (#10168). Add a row to section 6: the host
drains before restarting (finishes in-flight requests, takes no new ones), a
client reconnects without notice for a short gap, and a send that failed on a
dead connection is retried with its idempotency key so it is never stored twice.

### 13.8 Enforcement on macOS

A Coder run once made macOS ask for access to the Music library. Effects in
the admission snapshot include an operating-system deny set: on macOS,
executors run in a sandbox that refuses the privacy-protected locations (Music,
Photos, Documents, Desktop, Mail, Messages, iCloud Drive) and control of other
apps, so the operating system is never asked. A route that needs one of them
is a new, explicit grant, not a dialog.

### 13.9 Standing instructions

"Keep my disk from filling up" is not a run; it is a background rule
([background processes](../background/2026-10-02-background-processes.md)).
Add **Standing rule** as a route family: the router compiles the request into
a typed rule (trigger, condition, host-enforced actions), offers it, and the
host runs it with no model in the loop; a plugin can carry such rules, off
until the person turns them on.

### 13.10 The cost thesis is a measured claim

The reason to route rather than delegate raw is measured, not asserted: four
delegate settings (matched effort, six tools with a short system prompt, a
five-minute prompt cache, a Jev briefing) made the same model 61% cheaper and
37% faster than Claude Code alone at equal passes, and 45% cheaper on 26
Terminal-Bench tasks
([cost audit](../cost/2026-10-02-system-one-cost-efficiency-audit.md)). Make
those settings part of each Coder route's adapter digest, and make section 5's
"measured route evidence" include a raw-delegation baseline: the standing Gym
study (#10162) runs each task class through raw Claude Code, raw Codex, and
the shipped route, so cost per checked outcome against raw delegation is a
number the router reports, not a slogan.

Done for the settings in #10208: the delegate recipe
(`route_contract::recipe`, version `delegate-recipe-v1`) is one table of what
each engine gets (the Jev briefing, Jev-chosen knowledge, effort matched to
the task's class, the lean tools, system prompt and five-minute cache where
the engine allows them, and frozen checks with an early stop once they
pass), applied at dispatch to every task route, and a Coder route's adapter
is the digest of its runs' engines' rows. The per-engine table is the cost
audit's section 5a; the raw-delegation measurement is #10209.

### 13.11 What the caller hears

Acknowledgements start with the verb and name what happens: "Looking through
the latest commits.", "Exploring the repo with Codex, Claude Code, and Grok
Build.", "Picking up #10178." They never narrate an internal component ("We'll
have Coder…") or misread the request as its topic.

**First implementation milestone:** a message creates exactly one authorized
repository task on a scratch computer, produces a retained patch with an
independent check, survives a lost connection, and returns an honest final
record. That is the smallest proof of a router for agentic execution.

## 14. Terminal, studio, and paid cloud placement (2026-10-05)

The [workbench roadmap](../terminal/workbench-roadmap.md) delivers a shared
terminal application in the Grid and a standalone install, then binds
Everglade's existing studio resources. Shell/Request selection is local
input handling; a request then enters this shared route policy. The
terminal reads `RouteResult`, admissions, offers, journals, and task evidence
rather than interpreting generated prose or owning another executor.

An ordinary-shell proposal is a new typed bridge beyond today's CLI
command-tree route. Bind the exact command and proposal revision to the
originating thread, terminal generation, directory, and approval identity.
Every proposal in the MVP waits for Enter, including read-only commands;
report its resulting block to that thread with idempotent acknowledgment.
Uncertain shell execution requires reconciliation, never automatic replay.
Approving that command does not grant a Coder task, studio merge, provider
disclosure, or cloud spend. Studio controls initially use existing typed
host intents; a studio router adapter is a later contract extension.

Separate four choices: the viewing surface, the selected computer, the
executor/model, and the payer. Boat/GCE operator placement and hosted
inference fallback exist today. A credit-funded OpenAgents cloud computer
is a proposed paid execution placement, with a price-book-bound quote,
durable reservation, admitted computer and source, declared disclosures,
metering, checks, settlement, and teardown. Display credits over the shared
prepaid accounting without changing the sats contract or creating a
game-only ledger. Observation, execution, disclosure, and spending remain
separate authorities. A reconnect follows the funded execution identity;
it cannot charge or dispatch again.

Workbench bindings (#10669) extend an admission without editing the frozen
snapshot. [`route_contract::binding`](../../crates/route-contract/src/binding.rs)
(`openagents.route.workbench-binding.v1`) names one snapshot by digest and
adds the host generation and dispatch recipient, the run's task and engine
session, the originating terminal with its generation, and the workbench
[resource references](../terminal/workbench-resources.md) the route uses.
A continuation must keep the computer, generation, recipient, task, engine
session, and terminal, and its snapshot must not widen its parent; a
dispatch or control operation rechecks the host generation, the grant and
its revocation epoch, and the terminal generation, and refuses before
anything runs. A changed computer, recipient, effect, price, fee, or payer
is a new offer, and an expired offer never approves its substitute.

The [complete workbench issue directory](../terminal/issue-roadmap.md)
and [public project](https://github.com/orgs/OpenAgentsInc/projects/20)
track the integration deltas and their native blockers. Delivered local
admissions and lifecycle remain foundations. Owned remote-task admission
does not wait for the terminal snapshot protocol; each new adapter receives
its own retained qualification. Studio routing, the public API, composed
graphs, and additional payment rails remain later extensions rather than
prerequisites for the first Grid or direct studio release.
