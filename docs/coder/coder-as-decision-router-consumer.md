# Coder as a Decision Router consumer

Coder and Coder Terminal are the flagship application of the OpenAgents
Decision Router. A developer should be able to use Coder for real repository
work and see why typed decisions are useful: the right evidence reaches the
right workflow, independent work runs together, uncertain judgments receive
bounded review, and completed work comes with evidence.

The product promise is better work with an understandable cost and execution
record. More model calls, a larger question set, or a confident-looking
interface do not establish that result. Every production decision must earn
its place against a simpler baseline.

Status: target vision and delivery contract. The implementation inventory
below was checked against repository revision `9dd4ddab67` on 2026-09-21.
Existing components do not imply that the full consumer integration is
shipped. Consumer delivery is tracked in [#9501](https://github.com/OpenAgentsInc/openagents/issues/9501); the shared service
roadmap is [#9481](https://github.com/OpenAgentsInc/openagents/issues/9481).

The [TypeSafe-native agent analysis](typesafe-agent-analysis.md) applies the
founder's coding-agent proposal to this architecture. Its
[delivery roadmap](typesafe-agent-roadmap.md) defines the current implementation
order: shared evidence, task-specific context, a useful native coding loop,
then progressive tools, routing, parallel/background work, and durable
composition. The [project snapshot](2026-09-21-project-roadmap-snapshot.md)
records the two boards and their status discrepancies.

## What Decision Router means here

The Decision Router is the shared decision service and its contracts,
described in [the Decision API specification](../decision-models/decision-api.md).
It receives state and typed questions, resolves an authorized model and
policy, and returns judgments, outcomes, identities, usage, and receipts.
The native contract remains `POST /v1/systemone`; classification, batching,
review, and relay transport extend it.

Coder consumes that public contract as another application could. It must
not require a private Coder-only decision endpoint or duplicate tenant,
pricing, model-binding, and receipt rules in its terminal.

Keep three responsibilities distinct:

| Responsibility | Owner |
| --- | --- |
| Judge state, choose among supplied alternatives, score evidence, and apply an admitted review policy | Decision Router |
| Select and interpret a program, authorize effects, schedule tasks, execute commands/delegates, and verify results | Coder host runtime |
| Explain the plan, show progress and evidence, accept operator control, and expose the same state to scripts | Coder Terminal and headless interface |

Generation is a separate capability that produces text, patches, and plans.
Devin is an executor that performs bounded tasks. A model that answers a
decision question does not acquire either capability by answering it.

The [older service proposal](service-spec.md) explored a relay-only hosted
entry point. This document supersedes that proposal's product-wide claims
that HTTP and billing are out of scope. HTTP, relay, own-provider, and local
decision profiles are explicit supported targets. Their protocols retain
their own authentication boundaries; a generation job is not silently
reinterpreted as a decision job.

## The experience to build

A developer starts Coder, chooses a decision profile, and immediately sees
what can run, where repository state can go, and what the current allowance
permits. Existing provider configuration continues to work through a
documented migration path. A missing or broken required profile produces
an actionable explanation.

For a repository question, Coder builds a bounded candidate set from files,
symbols, search results, and observations. The router can rank relevant
evidence or select among valid options. Coder shows the answer with source
references and the decision record that selected the evidence. A small
question that needs no semantic prefilter takes the simpler path.

For a request to work through issues, Coder resolves the authorized source
into a pinned task list, identifies dependencies and conflicts, and shows
the proposed program. The host establishes what the request and session
policy permit. It schedules independent work under declared capacity and
conflict bounds, with one isolated checkout per writing delegate. The terminal
shows queued, running, refused,
unverified, and verified work as different states.

An uncertain judgment can be reviewed under a declared policy with a spend
and time bound. The operator can inspect the original result, the review,
and which result the program used. A capacity error, semantic refusal, or
missing calibration record cannot become an invisible model substitution.

At the end, Coder presents verified results, failed requirements, unresolved
evidence, retained worktrees, and the next authorized action. A successful
delegate exit is only one observation. A merge, push, publication, or issue
closure occurs only under the authority for that effect and after its
required evidence exists.

The same workflow works through the terminal and `coder -p`. The terminal
renders a shared event stream; it does not implement a second agent.

## What exists and what remains

| Area | Existing foundation | Consumer gap |
| --- | --- | --- |
| Shared turn | `coder::turn::run` drives terminal and headless modes | Preserve this single path through all router integrations |
| Decision calls | `Agent` asks turn action and shell outcome; `Runtime` asks program selection and program questions through `jev::Client` | One explicit router profile/client, structured failure policy, and receipt consumption: [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502) |
| Program runtime | File-defined `query`, `decide`, `check`, and `delegate` steps, explicit host grants, and a typed `run-suite` host path; unsupported kinds refuse | Typed composition, full durable program state, a real suite adapter, and additional review contracts |
| Local executor | Approved `devin-local`, resolved executable, bounded subprocesses, worktrees, and filesystem write boundary | Broader workflow verification and recovery; no general arbitrary-backlog safety claim |
| Remote executor | Encrypted Coder jobs reach a worker running its own approved Devin CLI | Separate decision-job transport depends on [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469)/[#9470](https://github.com/OpenAgentsInc/openagents/issues/9470) |
| Task sources | Request/file lists and scoped GitHub tracker snapshots with dependency and freshness checks | Broader source coverage and integration into complete recoverable workflows |
| Scheduling | Prepared task mappings, deterministic conflict/resource admission, refill scheduling, and durable claims in the project supervisor | General semantic task preparation, whole-run accounting, and complete recovery/integration: [#9514](https://github.com/OpenAgentsInc/openagents/issues/9514) |
| Completion | Per-requirement judgments and text checks, plus independent committed-artifact inspection and protected bounded checks | Real suite adapters, complete review programs, and unified consumer outcome semantics |
| Evidence | ATIF decision records, CoderBench, Gym suites, coverage, and retained report commitments | Versioned evidence store, task frames, context manifests, receipt/cost joins, and redacted exports |
| Shared service | Keyed HTTP gateway, partial classification route, tenant registry/keys/quota, execution receipts, caller CLI, backend capabilities, pure relay decision protocol, and monetary ledger foundations | Full Coder consumption, networked relay decisions, remaining classification modes, packed inference, and money/account enforcement |

Registry and quota foundations landed under [#9474](https://github.com/OpenAgentsInc/openagents/issues/9474) and
[#9467](https://github.com/OpenAgentsInc/openagents/issues/9467). Authentication
[#9466](https://github.com/OpenAgentsInc/openagents/issues/9466), gateway
[#9468](https://github.com/OpenAgentsInc/openagents/issues/9468), and caller
[#9476](https://github.com/OpenAgentsInc/openagents/issues/9476) are closed.
The broader cross-transport receipt contract in
[#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) remains open.
The [snapshot](2026-09-21-project-roadmap-snapshot.md) distinguishes landed
classification, capability, and money foundations from their unfinished
service integration. The partial [classification HTTP route](../decision-models/classification-http.md)
now serves Choice, multi-label Noul, and named dimensions serially through
native inference. Additional modes and packing remain open. Follow code and
acceptance scope as well as issue state.

The [observed local episode](measurements/2026-09-20-observed-fanout.md)
and [deployed relay episode](relay-transport.md#deployed) prove specific
six-task read-only paths. Preserve those records without generalizing them
to arbitrary edits, multiple customers, or unmeasured workloads.

## One decision client

Owner: [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502).

Introduce one Rust consumer boundary for every Coder decision call. Reuse
`crates/jev` for compatible native transport and the shared receipt types;
do not fork their schemas inside Coder.

A resolved profile states transport, endpoint or worker identity, credential
reference, workspace, allowed models/artifacts, capacity, policy, disclosure
rules, deadlines, and retry/spend limits. Configuration must distinguish
missing, intentionally disabled, unsupported, and invalid settings.
The current `Client::from_env().ok()` construction loses that distinction.
The SDK's `Config::local(url, model)` now supports explicit loopback without
provider credentials, proxies, or redirects; Coder profile integration is
still pending. Loopback identifies a transport destination, not proof that
the receiving server performs inference locally.

| Profile | Behavior | Service dependencies |
| --- | --- | --- |
| Hosted HTTP | Use the public gateway, tenant key, model authorization, quota, and receipts | [#9468](https://github.com/OpenAgentsInc/openagents/issues/9468), [#9466](https://github.com/OpenAgentsInc/openagents/issues/9466), [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) |
| Direct local | Reach an explicit loopback Kev/Lev-compatible endpoint without a fabricated provider key; preserve available identity evidence | SDK configuration exists; Coder profile integration remains in [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502) |
| Own provider | Preserve explicit direct-provider configuration and its real evidence limits | Existing Jev client; client compatibility in [#9489](https://github.com/OpenAgentsInc/openagents/issues/9489) |
| Relay decisions | Authenticate and encrypt the versioned decision contract to a trusted worker | [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469), [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470), [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) |

Generation and executor configuration remain separate from decision
configuration. A local decision profile alone does not make cloud
generation or a remote Devin session local. The interface shows all data
destinations, including optional reviewers.

Return typed answered, refused, unavailable, unattempted, and
unknown-completion outcomes. Preserve abstention and `none` as judgments
with their own meaning. Keep logical request IDs stable across retries and
attempt IDs distinct; reject a reused idempotency key with changed content.

A required judgment that cannot be obtained stops the dependent step.
An explicitly optional decision may permit ordinary chat to continue in
a visible degraded mode under its existing authority. Neither path grants
new execution permission. Local-only policy forbids hidden hosted fallback.

## Decision functions and policies

Owner: [#9503](https://github.com/OpenAgentsInc/openagents/issues/9503). Shared review policy: [#9485](https://github.com/OpenAgentsInc/openagents/issues/9485).

Treat each useful judgment as a small, versioned decision function with an
input schema, question-set digest, output schema, consuming policy, allowed
model profiles, state limits, and evidence for its admitted scope.

| Decision site | Current or intended use | Admission requirement |
| --- | --- | --- |
| Program selection | Choice among admissible programs and `none` | Measure missed and spurious selections separately; host authority still applies |
| Turn action | Respond, clarify, end, or halt | Preserve the measured baseline and no-match behavior |
| Shell outcome | Judge whether another bounded repair round is useful | Mechanical command status remains evidence; no model override of host bounds |
| Evidence selection | Rank supplied files/spans or filter independent candidates | Measure candidate coverage and downstream answer/task quality |
| Independence | Judge semantic dependencies left after mechanical checks | Never override known path conflicts or unmet prerequisites |
| Executor choice | Select among permitted, available executors when operator policy allows a choice | Respect explicit pins, context availability, capabilities, cost limits, and disclosure |
| Completion/review | Assess requirements that need semantic judgment | Mechanical acceptance evidence remains authoritative |

Use Noul for independent propositions, Choice for mutually exclusive
alternatives, and Score for an explicit ordered rubric. Batch independent
questions over shared state; use another call when a previous answer changes
the evidence. Do not invent new questions to make the architecture look
busier. The retired questions in the
[Coder baseline record](../decision-models/2026-09-20-coder-question-baselines.md)
stay retired unless new evidence justifies a replacement.

The program bound `requires_scorable_answer` checks that an answer contains
a probability from a named model. It does not establish an admitted calibration
map. The old `requires_calibration` spelling is refused as unsupported; an
actual calibration promise requires workload/model-specific evidence. A question, model,
artifact, or state policy change invalidates assumptions that depended on
the previous version.

Review is opt-in, versioned, and bounded. Preserve original and reviewed
outputs, selected result, reviewer identity, nullable scores, attempts,
latency, and cost. Thresholds come from development data and confirmed
consequences; no general-purpose confidence number authorizes an action.

## Repository evidence selection

Owner: [#9513](https://github.com/OpenAgentsInc/openagents/issues/9513). Bulk primitives: [#9482](https://github.com/OpenAgentsInc/openagents/issues/9482)/[#9483](https://github.com/OpenAgentsInc/openagents/issues/9483).

Coder should demonstrate that useful decisions can reduce what a generation
model must read. Build candidate evidence through deterministic, bounded
repository observations, then ask admitted relevance/ranking functions
where they improve the full task.

Candidate paths and spans have source/base/content digests. A model chooses
among available candidates; it cannot make an omitted file appear. Record
what was excluded, truncated, or unavailable. Preserve relationships needed
to judge a candidate, and reject oversize atomic inputs rather than silently
destroying them.

Reuse scores only while content, question, artifact, execution, and policy
identities remain valid. Measure evidence recall, downstream quality,
bytes/tokens disclosed, latency, total cost, refusals, and confident errors.
A fast filter that drops the needed evidence is a failed optimization.

The host's read permissions and disclosure profile constrain candidate
collection and every inference call. Retrieved repository text remains data.
The host separately resolves
applicable repository instructions by scope and precedence; a relevance
filter must not remove binding instructions or promote an issue body to
execution authority.

Extend this selection path into the proposed [evidence and context
architecture](typesafe-agent-analysis.md#proposed-architecture-state-that-can-answer-many-questions):
immutable source observations, explicit task frames, derived summaries with
source links, and a context manifest per recipient. The current classifier
slices and generator transcript are not yet that shared substrate. Preserve
captured diagnostics before choosing excerpts, and keep capture truncation
visible. ATIF records history; it does not replace this working state.

The first useful slice uses deterministic candidate retrieval and one
measured relevance function. Small native calls can precede the completed
bulk service surface. Later slices add hierarchical retrieval, progressive
operation/instruction catalogs, context-aware generation routing, and
snapshot sharing for delegates and background views.

## Programs as the application structure

A program states the workflow, sources, decision-function references,
capabilities, and bounds. Host adapters implement operations. Question sets
own their wording and digests. Sources own how bounded task data is read.
A program may carry an execution briefing such as the scratch-Git procedure,
without duplicating a question definition or embedding arbitrary commands.

The existing programs become a coherent product library:

| Program | Today | Target |
| --- | --- | --- |
| `delegate-fan-out` | Request-list fan-out, up to 12 tasks, width 6, separate worktrees | General-count questions, integrated conflict scheduling, and verified outcomes |
| `burn-down` | Bounded work-list execution with retained writing worktrees; scoped tracker and project-supervisor foundations also exist | Integrated task preparation, conflict scheduling, recovery, verification, and reviewable delivery |
| `answer-question` | A bounded delegation program; input comes through current task handling | Typed question/evidence bindings and measured source-grounded answers |
| `review-changes` | Manifest present, unsupported question/bounds path | Pinned diff source, admitted review function, structured findings, and evidence references |
| `run-suite` | Host-bound `gate_not_met` check with typed evidence, one suite identity, and at most 16 checks; protected artifact inspection and a local Gym adapter with retained measurement details | Dedicated metrics/gate output bindings and complete consumer integration |

Manifests on disk do not establish that every program runs. Unsupported
checks, questions, kinds, or bounds continue to refuse at admission.

### Program authority

Owner: [#9504](https://github.com/OpenAgentsInc/openagents/issues/9504).

Implemented under #9504: the program branch still precedes the ordinary
route-derived `Permit`, but now requires its own explicit host grant.
`CODER_PROGRAMS` or `--programs` names eligible programs; absence grants none.
Effect ceilings narrow that grant. Preserve both paths and the documented
limits in [program authority](program-authority.md).

A selected program proposes work within the operator's authorized scope.
A host permit covers relevant reads, writes, subprocesses, delegation,
network disclosure, and spend. Program selection, confidence, and a
manifest's `enforces` declaration cannot grant those effects.

The existing delegate boundary restricts filesystem writes; it does not
confine reads or network access. A declared read/network restriction needs
an enforcing adapter or boundary with evidence, or the host refuses it.
Displaying a scope in the terminal does not establish that enforcement.

Preview or refuse work when required authority is missing; do not repeatedly
ask for permission already supplied by the request or session policy.
Revalidate revocable grants at dispatch. An in-flight artifact identity
remains pinned even when later configuration changes.

A false program selection on an ordinary message with bullet points must
not start unrequested paid delegation. That negative case belongs in
acceptance alongside the happy-path demonstration.

### Work intake

Owner: [#9507](https://github.com/OpenAgentsInc/openagents/issues/9507).

Implemented under #9507: the [scoped GitHub adapter](tracker-intake.md)
produces bounded, pinned work from approved queries. Issue/base freshness,
native blockers, pagination completeness, and prepared task mappings feed
the project controller's admission. Extend this contract for other trackers
and complete workflow integration. A `query` names the source; it does not
embed a shell command.

Bound fetches, pagination, credentials, subprocesses, and output under
host-approved adapters. Refuse or refresh stale work before dispatch.
Missing dependencies and unknown acceptance criteria remain visible.
A tracker read grants no permission to post a comment or close an issue.

### Independence and scheduling

Owners: [#9508](https://github.com/OpenAgentsInc/openagents/issues/9508) and
[#9514](https://github.com/OpenAgentsInc/openagents/issues/9514).

Compute the conflict facts that are available: dependency order, declared
write/write conflicts, and relevant write/read conflicts. Shared reads
alone do not require serialization. Unknown effect footprints need an
explicit conservative policy.

The [project supervisor](project-supervision.md) already admits prepared
work against dependency/path conflicts and declared resource capacity,
refilling on completion. Extend that scheduler rather than rebuilding a
fixed-wave controller. Ask a semantic independence question only about
uncertainty the mechanical checks cannot settle. A high model probability
cannot override a known conflicting write. Resource declarations are
accounting, not kernel-enforced CPU or memory quotas.

The current fan-out uses v1 wording that names six tasks even though its
input bound allows twelve. `burn-down` uses the generalized v2 wording.
Evaluate a general-count replacement across disjoint, colliding,
ambiguous, and mixed workloads; retain historical digests. Closed
[#9414](https://github.com/OpenAgentsInc/openagents/issues/9414) records relevant model failures, not a universally safe
independence gate.

Inference packing under [#9483](https://github.com/OpenAgentsInc/openagents/issues/9483) and program task scheduling are
different systems. One batches model computation; the other controls
repository effects and dependency order.

### Independent completion

Owner: [#9509](https://github.com/OpenAgentsInc/openagents/issues/9509).

Define required evidence before work starts. Depending on the task, that
includes expected text, changed-file/diff scope, independent workspace
snapshots, bounded tests/builds, artifact checks, and semantic review.
Mechanical failures cannot be overruled by a model's completion judgment.

A delegation's text verdict still compares its returned text with `expects`;
without an expectation it is unverifiable. The separate
[artifact verifier](artifact-verification.md) now inspects committed work
and runs protected bounded host checks. The `run-suite` host path requires
typed evidence for one pinned suite and refuses exit-status-only plans.
A passing verification report still has `integration_accepted: false`.

Complete a real suite adapter, metrics output, `review-changes` contracts,
and consumer outcome integration. A program that ran all its steps can
still contain failed or unverifiable task verdicts. Preserve that distinction
in program state, headless results, terminal presentation, and exit behavior.
An executor's statement that it tested a change is not an independent test
result.

Retain writing worktrees and scratch commits for review. Applying them,
merging, pushing, or closing issues is a separate authorized effect with
fresh base/conflict checks. A verifier runs under its own bounded host
authority; it does not execute arbitrary instructions from a model review.

### Durable program runs

Owner: [#9510](https://github.com/OpenAgentsInc/openagents/issues/9510).

The project supervisor already persists claims and attempts, locks its
single writer, and retains interrupted running claims as unknown. Extend
that foundation to complete program/step/task recovery with source, base,
question, policy, artifact, authority, context, and retained-worktree
references. ATIF records what happened; durable orchestration also owns what
can resume. An evidence store supplies working context and is distinct
from both.

Reconnect to existing decision jobs or executor sessions where supported.
If an executor cannot resume, say so. Never blindly replay an ambiguous
write or other non-idempotent effect after a crash. Reconcile observed
results and outstanding reservations first.

Cancellation propagates through decision requests, queues, child programs,
and supervised processes. Preserve partial work and cleanup failures.
Budgets cover the full run: decision/reviewer calls, generation, delegates,
time, concurrency, and known monetary cost. Unknown external-executor cost
stays unknown; an unenforceable hard spend ceiling cannot be promised.

[#9484](https://github.com/OpenAgentsInc/openagents/issues/9484) provides durable decision batch jobs. It does not persist a
Coder program, its commands, or its worktrees. Reuse its identities and
status semantics without treating the two layers as interchangeable.

### Bounded program composition

Owner: [#9511](https://github.com/OpenAgentsInc/openagents/issues/9511).

Allow a larger program to call smaller programs through typed input/output
bindings and content-pinned references. Preserve parent/child identity,
outcome propagation, cancellation, evidence, and recovery.

Reject cycles and enforce depth, total steps/calls, concurrency, deadline,
and spend bounds. Child authority and budgets narrow from the parent.
A composed program cannot acquire a capability the parent lacks.

Wasm `module` execution remains a separate unimplemented extension. It is
not required to deliver reusable composed Coder workflows, and unsupported
module steps continue to refuse.

### Portable program registry

Owner: [#9512](https://github.com/OpenAgentsInc/openagents/issues/9512). Discovery: [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470)/[#9488](https://github.com/OpenAgentsInc/openagents/issues/9488);
catalog publication: [#9495](https://github.com/OpenAgentsInc/openagents/issues/9495).

Distribute versioned program/question/policy packages with a lock record:
content digests, provenance, publisher, dependencies, compatibility, and
capability/source references. Resolve local files first; add authenticated
remote discovery with explicit inspect, import, update, and rollback.

Discovering or importing a package runs no probe and grants no execution.
The package cannot approve its own adapter, reveal credentials, enable
hosted disclosure, or widen filesystem/network access. Keep machine paths
and secrets outside portable manifests. Pin every resolved reference for
the life of a run and support offline use of previously trusted packages.

## Receipts and replay

Owner: [#9505](https://github.com/OpenAgentsInc/openagents/issues/9505). Service receipts: [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471).

Every decision joins the Coder session, turn, program, step, function, and
work item to the router's request, attempt, optional job, model/artifact,
execution, policy, and registry identities. Keep original/reviewed outputs
and all attempts attributable.

Validate request/result binding and the supported authenticated origin.
A locally self-consistent digest does not prove the issuer or remote
execution. A legacy endpoint without receipt support remains explicitly
limited; do not fabricate a verified receipt.

Account separately for decisions, reviews, generation, and external
executors. Preserve quota reservations and exact versus unknown money.
The service ledger is authoritative for service charges; a local UI total
must not invent the cost of a Devin subscription or unavailable provider
usage.

Current traces are local and can contain full state and outputs. Add
selectable redaction/retention and consented export while retaining private
evidence needed to diagnose a run. A receipt containing only digests does
not make the associated ATIF trace safe to publish.

Offline replay inspects recorded decisions and host transitions without
rerunning commands or delegates. A new model comparison is a separate run,
with new attempts, budgets, and evidence. The Gym's complete-coverage and
independently retained commitment controls remain the publication boundary.

## Terminal and headless experience

Owner: [#9506](https://github.com/OpenAgentsInc/openagents/issues/9506).

Extend the existing terminal frame, rail, and intensity system. The normal
view stays focused on the user's work, with an expandable decision view
that answers:

- What evidence did this function receive, and what did the host omit?
- Which model/artifact and policy answered, and where did the data go?
- What were the raw alternatives, uncertainty, review, and selected result?
- What action did host policy permit?
- What ran, what was verified, and what remains unknown?
- What time, quota, and known cost did the full workflow consume?

Show live program/task states, dependencies, progress, cancellation,
retained worktrees, and acceptance failures. Add selected evidence and
omissions, source freshness, and revision-bound background findings through
the [planned terminal views](terminal.md#planned-evidence-and-task-views).
A normal turn should remain readable without expanding decision details. Keep decision latency separate
from queueing, generation, and delegate time. Distinguish a simulator from
a live metered request.

Expose the same events and outcomes as structured headless output.
Profile/setup diagnostics, optional workspace/account linking, key status,
allowances, and usage consume public contracts from [#9490](https://github.com/OpenAgentsInc/openagents/issues/9490) and
[#9493](https://github.com/OpenAgentsInc/openagents/issues/9493). They do not create a second account or billing system.

A profile can be inspectable without exposing secrets. No API key, Nostr
secret, raw private state, or copied provider credential belongs in an
ordinary status line or public issue.

## How the service roadmap enables Coder

| Service work | What it enables in Coder | Consumer owner or condition |
| --- | --- | --- |
| [#9468](https://github.com/OpenAgentsInc/openagents/issues/9468) gateway; [#9466](https://github.com/OpenAgentsInc/openagents/issues/9466) auth; [#9474](https://github.com/OpenAgentsInc/openagents/issues/9474) registry; [#9467](https://github.com/OpenAgentsInc/openagents/issues/9467) quota | Authorized, bounded shared inference | [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502); gateway, auth, registry, quota, and CLI foundations exist |
| [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) receipts | Per-attempt evidence and attribution across transports | [#9505](https://github.com/OpenAgentsInc/openagents/issues/9505) |
| [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469) decision jobs; [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470) discovery | Router decisions through the relay, separate from current Devin/generation jobs | [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502), [#9512](https://github.com/OpenAgentsInc/openagents/issues/9512) |
| [#9482](https://github.com/OpenAgentsInc/openagents/issues/9482) classification; [#9483](https://github.com/OpenAgentsInc/openagents/issues/9483) inference batching | Bulk evidence filtering, ranking, and multidimensional review | [#9513](https://github.com/OpenAgentsInc/openagents/issues/9513) |
| [#9484](https://github.com/OpenAgentsInc/openagents/issues/9484) durable decision jobs | Long-running decision batches with resumable results | [#9510](https://github.com/OpenAgentsInc/openagents/issues/9510) still owns the enclosing program |
| [#9485](https://github.com/OpenAgentsInc/openagents/issues/9485) review/fallback | Bounded escalation with original and reviewed evidence | [#9503](https://github.com/OpenAgentsInc/openagents/issues/9503), [#9505](https://github.com/OpenAgentsInc/openagents/issues/9505) |
| [#9486](https://github.com/OpenAgentsInc/openagents/issues/9486) backend capabilities | Model-specific profiles, limits, language/modality availability | [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502), [#9503](https://github.com/OpenAgentsInc/openagents/issues/9503); no automatic default replacement |
| [#9489](https://github.com/OpenAgentsInc/openagents/issues/9489) clients; [#9476](https://github.com/OpenAgentsInc/openagents/issues/9476) docs/CLI | A reusable Rust integration another application can follow | [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502), [#9501](https://github.com/OpenAgentsInc/openagents/issues/9501) |
| [#9487](https://github.com/OpenAgentsInc/openagents/issues/9487) MCP; [#9488](https://github.com/OpenAgentsInc/openagents/issues/9488) discovery | Agent-readable service/docs and interoperable discovery | Coder's core uses the shared Rust client; MCP is an integration surface, not a second execution policy |
| [#9490](https://github.com/OpenAgentsInc/openagents/issues/9490) accounts; [#9491](https://github.com/OpenAgentsInc/openagents/issues/9491) money; [#9492](https://github.com/OpenAgentsInc/openagents/issues/9492) plans; [#9493](https://github.com/OpenAgentsInc/openagents/issues/9493) usage | Setup, workspace access, enforced service spend, and understandable balances | [#9506](https://github.com/OpenAgentsInc/openagents/issues/9506), [#9510](https://github.com/OpenAgentsInc/openagents/issues/9510) |
| [#9495](https://github.com/OpenAgentsInc/openagents/issues/9495) skills; [#9496](https://github.com/OpenAgentsInc/openagents/issues/9496) recipes | Publish portable, measured Coder workflows | [#9512](https://github.com/OpenAgentsInc/openagents/issues/9512), [#9501](https://github.com/OpenAgentsInc/openagents/issues/9501) |
| [#9494](https://github.com/OpenAgentsInc/openagents/issues/9494) playground | Share inspectable workflow examples outside the terminal | Reuse contracts and safe exported evidence; do not duplicate Coder's runtime |
| [#9497](https://github.com/OpenAgentsInc/openagents/issues/9497) feedback; [#9498](https://github.com/OpenAgentsInc/openagents/issues/9498) operations | Consented diagnostic reports and tested hosted/self-hosted setup | [#9505](https://github.com/OpenAgentsInc/openagents/issues/9505), [#9506](https://github.com/OpenAgentsInc/openagents/issues/9506) |
| [#9480](https://github.com/OpenAgentsInc/openagents/issues/9480) caller pilot; [#9475](https://github.com/OpenAgentsInc/openagents/issues/9475) public evidence | Validate value and publish reproducible consumer outcomes | [#9501](https://github.com/OpenAgentsInc/openagents/issues/9501); Coder dogfooding does not replace an independent caller |
| [#9472](https://github.com/OpenAgentsInc/openagents/issues/9472) training; [#9473](https://github.com/OpenAgentsInc/openagents/issues/9473) admission | Optional workload-adapted decision functions | Gate on held-out benefit; reuse [#9461](https://github.com/OpenAgentsInc/openagents/issues/9461) for the conditional Coder-specific training question |
| [#9499](https://github.com/OpenAgentsInc/openagents/issues/9499) image decisions | Future screenshot/visual-state judgments over host-supplied candidates | Experimental; visual judgments do not grant computer-action permission |
| [#9500](https://github.com/OpenAgentsInc/openagents/issues/9500) confidential inference | A future hosted profile with a separately established threat model | Experimental; do not label ordinary dedicated hosting confidential |

## Delivery sequence and acceptance

Use the [TypeSafe-native roadmap](typesafe-agent-roadmap.md) as the current
sequence. It replaces the earlier four-release grouping with independently
useful increments:

| Phase | Product increment | Main owners |
| --- | --- | --- |
| 0 | Explicit client/function outcomes and a complete workflow baseline | #9502, #9503, #9505, #9506 |
| 1 | Shared evidence, context manifests, and source-grounded repository answers | #9513, #9505, #9506 |
| 2 | Native coding operations, task-specific generation context, and independent repair checks | #9513, #9509, #9503 |
| 3 | Progressive tools/instructions and measured context-aware generation routing | #9503, #9513, #9512 |
| 4 | Shared task context, conflict-aware parallel work, and bounded background views | #9508, #9514, #9506, #9509 |
| 5 | Complete recovery, whole-run accounting, typed composition, and portable workflows | #9510, #9511, #9512, #9514 |

#9501 owns the complete consumer release. Existing authority and tracker
contracts from #9504/#9507 support these phases. Their completed original
scope is not new work. Hosted commercial capabilities and additional
transports are required for their respective release profiles, not every
local prototype.

The release suite includes program `none`, a wrong program choice,
unavailable/abstaining judgments, revoked credentials, exhausted quotas,
artifact drift, local-only disclosure, reviewer failure, stale sources,
dependency conflicts, missing acceptance evidence, cancelled delegates,
and crash recovery. Use bounded synthetic providers for mechanics and
explicit budgets for live model/Devin demonstrations.

Compare complete workflows against deterministic/no-decision baselines.
Report task success, dangerous error directions, coverage, unnecessary
escalations, evidence recall, wall time, and total known cost. Preserve
unknown costs, refusals, and missing work. Reproducibility comes from pinned
instruments and retained evidence, not a claim that every model is
deterministic.

Keep product code Rust and preserve the shared terminal/headless engine.
Use [the manual verification contract](../verification.md), relevant
consumer tests, and non-GitHub infrastructure. This document authorizes no
new paid run and does not resume deferred measurements in [#9382](https://github.com/OpenAgentsInc/openagents/issues/9382),
[#9393](https://github.com/OpenAgentsInc/openagents/issues/9393), or [#9426](https://github.com/OpenAgentsInc/openagents/issues/9426).

The flagship is ready when another developer can follow Coder's published
integration, reproduce a useful workflow, inspect why it acted, and verify
what it accomplished using the same public service contracts.
