# Risks, tensions, and decision tests

- Date: 2026-07-09
- Status: Sol analysis; challenge document, non-authoritative
- Context: [`roadmap system model`](./2026-07-09-roadmap-system-model.md)

> **Revision 25 supersession (2026-07-10):** The risk inventory remains useful
> where it matches current contracts, but its Sarah-first/A/V/sales program
> assumptions do not control priority. Current falsifiers live in
> [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md); mobile remote-workroom risks
> and acceptance live in the
> [`mobile port plan`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md).

## Why this document exists

The current roadmap is coherent, but it is unusually ambitious. Sarah-first,
an owned real-time avatar, a multi-harness execution fleet, managed Agent
Computers, exact evidence rails, a cross-platform application framework, a
company brain, and an outbound sales engine are each substantial programs.

Coherence does not remove execution risk. It can make risk harder to see
because every program has a plausible place in the same story. The tests below
are intended to distinguish compounding architecture from mutually reinforcing
scope.

## Tension 1: one front door versus one point of failure

Sarah concentrates identity, acquisition, control, and continuity. That
creates a powerful relationship and a severe blast radius.

**Early warnings:** avatar or inference incidents make all meaningful work
unreachable; users cannot find existing work without starting a new
conversation; operator tools depend on Sarah's persona service; fallback is
technically present but functionally degraded.

**Decision test:** disable video and the primary inference provider. Can an
authenticated user still find a run, issue a bounded command, approve it, and
read the receipt through a text-first path?

If not, the product has centralized availability more than relationship.

## Tension 2: a memorable persona versus a general work interface

Sarah's sales voice creates character, but sales behavior can be inappropriate
for coding operations, incident response, or sensitive approvals.

**Early warnings:** users fight the persona to get concise operational
answers; sales phrasing leaks into owner workflows; the shared Khala lane
injects another persona; tool access changes from prompt context rather than
authenticated policy.

**Decision test:** run the same bounded task in prospect, customer, and
operator modes. Do retrieval scope, available tools, tone, and approval
posture change only from typed relationship state while identity and honesty
remain consistent?

If not, separate role programs are needed beneath the shared identity before
more capabilities ship.

## Tension 3: conversational simplicity versus operational precision

Conversation makes intent easy but can obscure concurrency, exact state, and
irreversible side effects.

**Early warnings:** users repeatedly ask “what is actually running?”; one turn
accidentally refers to the wrong task; approvals lack an unambiguous target;
the transcript becomes the only run history; support requires raw logs.

**Decision test:** give a user three concurrent runs, one blocked approval,
and one failed verification. Can they identify and steer the correct run from
the canvas without relying on conversational recency?

If not, the canvas information architecture must advance before fleet breadth.

## Tension 4: avatar quality versus utility latency

The owned avatar can make Sarah distinctive and human, but visual perfection
can absorb the critical path while the work loop remains incomplete.

**Early warnings:** experiment count grows faster than user conversations;
opener fidelity improves while turn latency or task completion does not;
high-quality rendering raises capacity cost without retention evidence;
product milestones wait on subjective quality beyond a defined tier.

**Decision test:** compare text, audio-only, real-time video, and pre-rendered
opener cohorts on successful verified outcomes and repeat use—not only visual
preference.

If video does not improve relationship or task metrics enough to justify its
cost and availability burden, keep it as a graceful tier rather than the only
front door.

## Tension 5: Effect Native as leverage versus Effect Native as bottleneck

The framework can unify the estate, but every missing primitive can become a
queue and every conversion can compete with product closure.

**Early warnings:** consumer teams create “temporary” local primitives;
vendored versions drift; migration PRs move code without deleting old state
models; framework work has no two-renderer consumer; the Sarah vertical slice
waits on pixel parity unrelated to its core behavior.

**Decision test:** measure lead time for a real cross-platform Sarah feature.
Does Effect Native reduce total implementation and verification effort by the
second renderer, while keeping local exceptions at zero?

If not, narrow the catalog, improve upstream throughput, or defer low-value
fidelity—but do not pretend nominal adoption is leverage.

## Tension 6: shared engine versus authority collapse

One orchestration system is desirable. One undifferentiated pool of accounts,
credentials, money, and capacity is not.

**Early warnings:** “auto” hides an authority-rail switch; user subscription
capacity is described as marketplace supply; account refs disappear from
private receipts; org-cloud and owner-local failures share generic states;
fallback crosses owner boundaries.

**Decision test:** for any run, can the system prove the owner scope,
capacity rail, provider account class, credential mechanism, cost truth, and
fallback history without reading raw logs?

If not, orchestration unification has outrun custody and accounting.

## Tension 7: Blueprint memory versus surveillance and false certainty

A live map of what Sarah knows can create extraordinary continuity. It can
also over-collect, leak, or convert model inference into apparent fact.

**Early warnings:** nodes lack source refs; inferred attributes look identical
to supplied facts; deletion does not propagate; cross-prospect tests focus on
rows but not derived projections; users cannot correct the map; “collective
learning” language gets ahead of consent and promise state.

**Decision test:** ask a user to inspect, correct, delete, and export a
material Blueprint fact. Can the system show its source, scope, derived uses,
and removal result across every surface?

If not, the brain should not broaden its ingestion surface.

## Tension 8: receipts as trust versus receipts as friction

Evidence can become either a clear product advantage or a wall of internal
identifiers.

**Early warnings:** the canvas exposes raw refs without explanation;
operators ignore receipts because they are too dense; success requires D1 or
SQL queries; public counters are mistaken for task proof; receipt generation
adds latency without supporting a user decision.

**Decision test:** can a non-developer answer “what happened, why should I
believe it, what did it cost, and what can I do next?” from the bounded
receipt projection in under a minute?

If not, preserve exact backend evidence but redesign its projection.

## Tension 9: constant motion versus integration debt

High agent throughput can close many isolated issues while the decisive
cross-system experience remains unproven.

**Early warnings:** issue velocity rises while vertical-slice receipts do not;
roadmap revision notes dominate the durable architecture; queue snapshots go
stale within hours; components land without adoption; owner proof gates
accumulate.

**Decision test:** each week, count completed user loops, deleted alternate
paths, and live receipts—not only commits and issues. Did at least one result
reduce the number of systems a user or operator must mentally join?

If not, temporarily bias capacity toward integration and reconciliation.

## Tension 10: Sarah-first versus customer preference

The thesis assumes a persistent conversational relationship is the best way to
consume delegated capability. Some users may prefer direct tools, APIs, or
specialist interfaces.

**Early warnings:** expert users bypass Sarah for most consequential work;
customers treat her only as onboarding; repeat use occurs in cockpit/API but
not conversation; teams want multiple role identities sooner than expected.

**Decision test:** instrument where successful repeat work begins and where it
is supervised. Segment by user type. Does Sarah remain the natural return
point after onboarding?

If not, keep shared state and authority but reposition Sarah as the default
relationship rather than an exclusive shell.

## Program-level falsifiers

The roadmap should be reconsidered—not merely tuned—if several of these remain
true after the first vertical slices:

- verified task completion through Sarah is consistently slower or less clear
  than the power tools with no retention benefit;
- cross-device Effect Native features cost more to build and maintain than
  renderer-specific implementations after the second and third real features;
- users do not inspect or trust Blueprint/receipt projections;
- the avatar materially harms availability or economics without improving
  activation and repeat use;
- operator minutes grow roughly linearly with customers or delegated tasks;
- authority incidents arise because the unified surface obscures distinct
  rails;
- durable memory produces more correction and privacy burden than useful
  continuity.

None of these falsifies the underlying infrastructure. They would falsify the
current packaging, sequencing, or breadth.

## Questions Sol should keep revisiting

1. What is the smallest complete job Sarah can own better than any existing
   interface?
2. Which information belongs in conversation, which belongs on the canvas,
   and which belongs only in specialist tools?
3. At what point should Sarah delegate to a named colleague rather than wear
   another role?
4. Which receipts change user decisions, and which are audit-only?
5. Is the Blueprint becoming a useful shared model or a passive data exhaust?
6. Is Effect Native reducing semantic duplication, not just component
   duplication?
7. Are exact accounting and owner gates producing better automated policy, or
   only more records?
8. Is each new capability making the relationship more useful without making
   the authority model less legible?

The roadmap deserves confidence because its parts can compound. It deserves
continuous challenge because the same coherence can rationalize too much
simultaneous scope. The correct posture is conviction with falsifiers.
