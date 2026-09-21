# Evaluation and throughput

Status: proposed experiment plan. No performance or learning result is claimed.
Minecraft should expose failure modes and make experiments understandable; it
should not turn a cinematic recording into an unqualified benchmark.

## Separate four questions

1. Can the protocol and accounting remain correct under failures and load?
2. Can agents complete the world and coding tasks?
3. Does Jev or skill reuse improve outcomes for a comparable budget?
4. Does earning compute change coordination in a useful way?

Measure these separately. More chat events are not better teamwork. More tokens
are not more progress. A faster relay does not prove a faster coding workflow.

## Frozen experiment identity

Each run pins the arena manifest, world epoch and starting snapshot, roster,
machine/network description, server/client versions, relay configuration,
program/extension locks, model assurance, question artifacts, controller policy,
reward schedule, budgets, and evaluator. Record the actual loaded artifacts.

Retain all admitted tasks and attempts, including refusals, timeouts, failed
builds, cancelled jobs, unknown results, and service outages. State the unit of
analysis: event, call, attempt, task, guild round, or season. An attempted task
does not leave the denominator because it failed before the spectator saw it.

Use distinct world seeds/layout families and quest families for development and
confirmation. Splitting nearly identical snapshots into different files is not
a meaningful holdout. Keep model judges and confirmation answers isolated from
competitors. Record cache policy and execution order.

## Correctness before load

| Fixture | Required result |
| --- | --- |
| Replayed mining observation or duplicated event ID | One award for the registered deposit |
| A gift, chest withdrawal, placed ore, or restored inventory | No mining credit |
| Crash after world effect but before award | Reconcile from evidence or remain unresolved; never guess |
| Two concurrent deposit claims | One admitted exclusive actor; loser cannot dispatch with its stale claim |
| Lease expiry while worker remains alive | Old worker fenced or resource unavailable; no assumed termination |
| Two child jobs racing for the last budget | Combined reservations cannot exceed the parent allowance |
| Lost inference response | Unknown usage remains held; no automatic free retry |
| Same execution request identity and bytes | Existing state returned without duplicate execution |
| Same identity with changed bytes | Idempotency conflict |
| Wrong signer, recipient, family, request reference, or answer schema | Refuse before accepting a result |
| Cancellation acknowledgment before process exit | Display stopping; keep relevant holds until reconciliation |
| Late success from an obsolete controller | Cannot integrate, award XP, or mutate the world |
| Modified tests, hidden side effects, or writes outside scope | Verification fails or is unverifiable; no acceptance reward |
| Duplicate completion label or repeated quest solution | No duplicate XP |
| Reconnect, replay from zero, out-of-order projection update | Totals converge to the authoritative ledger |
| Public query, IDs, COUNT, search, or live subscription for private evidence | Unauthorized caller learns no protected artifact |
| Guild `h` confused with private mailbox `h` | Refuse misrouted envelope; do not expose it as guild chat |
| `39005` CW/group-pin collision | Only the configured, validated meaning is admitted and rendered |
| Server snapshot restored with stale economy state | Detect epoch mismatch; require reconciliation or a new season |

Also test malformed UTF-8/JSON, duplicate keys, unknown semantic fields, excessive
payloads, cycles in definitions, missing locked artifacts, and unsupported hard
bounds. Record typed refusals and their counts. Keep transport conformance
separate from semantic quality.

## Throughput tiers

Run a dedicated local or explicitly authorized test relay before any shared
deployment. Start small and increase load only while correctness holds. The
numbers below define candidate workload sizes, not promised capacity.

| Tier | Workload | What it measures |
| --- | --- | --- |
| T0: codec | Fixed valid and invalid event corpus, no network | Parse, signature, encryption, and schema cost |
| T1: relay | 1, 4, 16, 64, then 256 simulated identities; bounded publish rates and fanout | Admission, delivery, replay, privacy, and queue behavior |
| T2: jobs | CJ decision and execution families with deterministic fixture workers | Correlation, admission, cancellation, durable recovery, and shared reservations |
| T3: live decisions | Small bounded Jev batches through the admitted route | Actual inference latency, usage, refusal, and end-to-end overhead |
| T4: embodied agents | 1, 4, 8, then 16 bots if prior steps remain healthy | Server tick pressure, observation freshness, pathfinding, and agent coordination |
| T5: full quest | Repeated independent guild rounds with live coding | Accepted outcomes, costs, repair behavior, and world integration |

T1 can stress the protocol beyond what one Minecraft server can animate. Label
those identities simulated. T2 fixture answers are not Jev results. T4 bots do
not need expensive generation on every tick. Do not turn a throughput test into
an unbounded inference bill.

For T1, vary payload bytes, number of subscribers, group size, authorization
state, duplicate rate, disconnect/reconnect, and historical query load. Exercise
both one-owner-many-agents and multiple-owner cases; owner-aggregated limits must
not be mistaken for total relay capacity. Test public chat and admitted private
envelopes separately.

## Measure delivery, not just publication

Count offered events, admitted unique events, rejected events by cause, expected
recipient deliveries, observed unique deliveries, duplicates, and missing
deliveries after a declared observation window. Nostr `OK` is admission feedback,
not proof that a subscriber consumed or decrypted a message.

If `R` unique events per second each have `f` intended recipients, the expected
delivery workload is `R × f` recipient copies per second. Report both values,
plus payload bytes and encryption/signature work. A deduplicated event total
cannot substitute for delivery coverage.

Measure publish-to-admission, publish-to-recipient-validation, job admission,
queue wait, first feedback, complete result, and whole-task latency. Report
sample counts and p50/p95/p99 only with enough observations to make the tail
meaningful. Include timeout rate and queue depth; do not omit failures to make
latency look better.

Use monotonic clocks for elapsed time on one host. Across hosts, record clock
synchronization and uncertainty or measure round trips from one clock. An
author-controlled `created_at` is not a trusted high-resolution timer.

Record relay CPU, memory, database/query pressure, open connections, queued
bytes, rejected frames, and subscriber lag. Record Minecraft tick rate, bot
observation age, action completion latency, and spectator frame time separately.
Pause load growth on invariant failure, rising unbounded queues, or a breached
resource limit defined in the run manifest.

## Compare agent behavior

Use matched maps, quests, model access, and total provider budgets. Repeat rounds
and swap guild identities/positions to reduce starting-location effects. Report
run variation and uncertainty; a single video is an illustration.

| Comparison | Hypothesis | Essential outcomes |
| --- | --- | --- |
| Deterministic task rules vs Jev selection | Semantic requests benefit from typed interpretation | Task-match errors, abstention, completed quests, end-to-end cost |
| No library vs supplied skills vs learned skills | Reusable behavior reduces repeated generation | Accepted unseen tasks, generation calls, skill failures, full cost |
| Fixed task order vs bounded curriculum | Better sequencing expands useful capability | Unique verified skills, unseen-task success, prerequisite mistakes |
| Single agent vs guild | Coordination improves useful throughput | Accepted work per wall time and cost, messaging overhead, conflicts |
| Equal free allocation vs earned compute | Scarcity changes allocation productively | Accepted work, idle/mining time, rejected reservations, fairness |
| Authored selector vs OPT candidate | A measured implementation improves the whole task | Confirmation quality, total study/runtime cost, regressions, abstention |

Keep task selection quality distinct from candidate coverage. Keep skill retrieval
quality distinct from skill execution quality. For failure review, retain the
state, candidates, model answer, policy, chosen action, and observed result.
Classify missing evidence, stale state, model error, adapter failure, verifier
failure, and infrastructure failure separately.

## Economy metrics

Replay the ledger and check conservation after every injected failure. Report
issued, available, held, spent, corrected, and unknown amounts for CC and the
separate provider budget. Record award uniqueness and attribution disputes.

Useful efficiency measures include accepted quest value per settled provider
cost, per wall-clock minute, and per generation call. Include decision, review,
failed-attempt, and optimization costs in the appropriate denominators. Do not
divide by zero or exclude unknown cost; mark such ratios unavailable or present
justified bounds. Energy efficiency requires an actual measurement boundary and
energy instrument, not a token-count proxy.

Track concentration of resources and participation by enrolled guild. Test
whether the leading guild monopolizes the deposits before others can progress.
Change map or allocation rules between experiments if needed, and retain that
change as part of the comparison identity.

## Reporting and adoption

NIP-EVAL records the frozen workload, case outcomes, comparator, costs, and
scope of any admission. A public `3189` evaluation declaration needs explicit
publication clearance and the expected evaluator signer. Do not publish raw
private task evidence by attaching its digest to an otherwise public report.

NIP-OPT records the full study, including failed candidates and actual
materialization. Search success does not establish confirmation success.
Confirmation does not install the winner. Operator policy adopts an eligible
EXT release for future rounds and preserves the prior version for an allowed
rollback. An honest no-improvement result is a complete experiment.
