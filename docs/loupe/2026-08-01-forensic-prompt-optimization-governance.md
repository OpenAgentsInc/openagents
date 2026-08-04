# Forensic prompt optimization governance

The first forensic prompt optimizer is a bounded offline compiler integrated
with Blueprint governance. It does not grant runtime authority and it does not
claim an optimizer implementation that is not present.

## Candidate identity

Each immutable candidate digest covers its parent prompt, Prompt IR, typed
finding and hypothesis schemas, tool policy, examples, parameters, every
dataset revision, optimizer configuration and kind, and the frozen metric,
T5, censoring, and eligibility definitions. Human curation, ablation, grids,
and retained-failure replay retain those honest names. DSPy or GEPA labels are
accepted only with an independently tested integration receipt.

## Evaluation boundary

Training and development examples are optimizer-visible. Vulnerable and clean
holdouts are evaluator-only, disjoint, non-empty, and absent from candidate
input. The non-empty requirement is not decoration: disjointness is vacuous over
an empty set, so a holdout revision declaring no examples would satisfy every
blindness check while carrying nothing to be blind about. A
candidate is eligible for comparison only when:

- its baseline and candidate scorecards use the same untouched holdout,
  matched population, and frozen metric revision;
- all hard gates pass before the Pareto comparison is derived;
- the candidate improves holdout discovery without a clean-holdout hit;
- generation and evaluation use distinct identities;
- the resolved evidence carries at least two admitted, ready OpenAgents Cloud
  worker placements and at least two resolved source states; and
- mechanical evidence covers scorecard rebuild, source-state resolution, and
  worker-lifecycle resolution.

Evaluation cannot rewrite the candidate's frozen metric definitions after
results are known.

### Resolved evaluation evidence

Fresh-worker and source-state proof is a typed record set, not a list of caller
strings. Worker placements are `ForensicWorkerPlacement` values, so the forensic
contract itself requires admission and readiness receipts for a ready or running
worker, the managed target class, the Google Cloud provider, and the broker-only
network policy. On top of that the evaluation requires:

- distinct placement refs, sandboxes, and work units, all in one owner scope;
- each worker reaching readiness before the scorecards it produced were
  generated;
- resolved source states carrying the candidate's exact untouched holdout and
  clean-holdout dataset digests; and
- distinct mechanical-evidence receipts, each with a sha256 receipt digest and
  each bound to one of the three evaluated scorecards.

The evaluation records a digest over that whole evidence set. This is structural
resolution of typed records. It cannot be satisfied by arbitrary strings, and it
is still evidence the evaluator submits rather than a lookup against a live
placement authority.

### Derived Pareto comparison

The metric freeze binds all seven acceptance axes — hit rate, causal coverage,
time, tokens, cost, false positives, and reviewer load — to a frozen metric ref
and an explicit direction before any result exists. Because the bindings sit
inside the frozen metric digest, they cannot be chosen after results are known.

Every bound metric ref must be one the frozen forensic metric registry
(`registry.openagents.forensic.metrics.2026-08-01.v1`) actually defines, and the
freeze must pin that same registry revision. Without the first check an
evaluator could bind an axis to a ref it invented, supply values for that same
invented ref in its own scorecard, and obtain a `dominates` verdict computed
entirely over its own assertion — which is the thing a derived comparison exists
to remove. Without the second, a freeze could claim it pinned some other metric
definitions while its axes were admitted against these, leaving every downstream
scorecard-revision comparison to check a caller-chosen digest against itself.

`rebuildForensicScorecard` emits a per-run value for all seven bound metrics.

The evaluation aggregates each bound metric across the retained runs of the
baseline and candidate holdout scorecards and derives the verdict. `dominates`
means no axis is worse and at least one is better. An axis with no available
value on either side is reported as `unavailable` and the whole status becomes
`insufficient_evidence`, which is never silently scored as a tie and cannot be
promoted. The per-axis comparison is recorded on the evaluation, so the verdict
is reproducible from the scorecards rather than asserted by the reporter.

## Promotion and rollback

Compiler output is an unpromoted Blueprint optimizer candidate. Promotion
requires an explicit passing Blueprint release gate and a human/operator
decision by an identity distinct from the generator. The release gate,
evaluation, and candidate digests must agree.

Promotion and rollback both read the durable owner-scoped governance state
first. The prior active digest, the rollback anchor, and the transition sequence
come from that stored pointer, never from the promoting caller, so nobody can
choose what they are "rolling back to". Each transition carries a digest over
its own content and the producer of the candidate it activates.

Rollback is held to the same authority boundary as promotion: it reverts exactly
the activation currently in force, verifies that activation's stored digest, and
refuses an operator who produced the reverted candidate. It deliberately does
not require a release gate, because reverting to an already governed prior
prompt is a recovery action.

## Durable governance state

`repositories/forensic-prompt-governance.ts` stores the owner-scoped active
pointer and its append-only transition history in the tables created by
`packages/khala-sync-server/migrations/0135_forensic_prompt_governance.sql`.

An append is admitted only under compare-and-set on the revision and prior
digest the caller observed, so a decision taken against a stale read is rejected
rather than merged. Reversal appends a new transition; a recorded transition is
never edited. Every read re-verifies the stored transition digests and the
pointer/history agreement, so storage drift surfaces as an error instead of
being served as governance truth.

The pointer is null exactly at genesis, before any prompt has been governed, and
again if a rollback returns to that state.

## Implementation and current limits

The implementation lives in
`apps/openagents.com/workers/api/src/blueprint/services/forensic-prompt-compiler.ts`
and `apps/openagents.com/workers/api/src/blueprint/repositories/forensic-prompt-governance.ts`,
with contracts in `schemas/forensic-prompt-optimization.ts`. The tests cover
canonical identity, holdout isolation, honest optimizer naming, resolved worker
and source evidence, derived Pareto verdicts, self-promotion denial, guarded
rollback, durable append-only history, stale-read conflicts, and storage drift.

## Operator route

`GET` and `POST /api/blueprint/forensic-prompt-governance` are the production
caller for the durable store, implemented in
`apps/openagents.com/workers/api/src/blueprint-forensic-prompt-governance-routes.ts`
and mounted from the API worker route table. Both methods require the admin API
token.

`GET ?ownerRef=<ref>` returns the owner's active pointer, revision, and
append-only history. `POST` carries `{ownerRef, decision, transitionRef,
operatorDecisionRef, operatorIdentityRef}` plus, for `promote`, the candidate,
the evaluation, the release gate, and the evaluation ref.

Two properties are deliberate. The decision is computed on the server: the route
reads the durable state, runs `promoteForensicPrompt` or
`rollbackForensicPrompt` against exactly that state, and appends under
compare-and-set on the revision it read. A caller cannot hand in a pre-sealed
transition and skip the gate. And `decidedAt` comes from the server clock, so a
decision cannot be backdated into the append-only history. A gate refusal is
reported as `422 refused` with its exact reason, a stale read as `409`,
unconfigured or unreachable storage as `503`, and storage drift found on read as
`500`.

## Why no campaign has run

The remaining acceptance criterion — a winning candidate improves an untouched
holdout without regressing the clean holdout or any hard gate — is a claim about
a real campaign. It is not evidenced. As of 2026-08-04 two of the four blockers
recorded on 2026-08-03 are closed and two remain, and the campaign criterion is
descoped to the live program gate on
[#9300](https://github.com/OpenAgentsInc/openagents/issues/9300).

### Closed

- **The two unresolvable Pareto axes now resolve.** `rebuildForensicScorecard`
  previously emitted no per-run value for `metric.causal_chain_coverage.v1` or
  `metric.cost_to_identification.v1`, so any scorecard the repository's own
  projector built reported those axes `unavailable`, the derived status was
  `insufficient_evidence`, and promotion refused every candidate regardless of
  merit. Causal-chain coverage is now derived from the run's retained
  adjudications as supported links over required links. Cost to identification
  is derived from retained provider cost through immutable T5 plus a new
  `openagents.forensic_infrastructure_cost_receipt.v1`. The frozen definition
  reads *provider and incremental GCE cost*, and it was not narrowed to provider
  cost alone: a candidate that traded provider tokens for longer paid VM time
  would win a provider-only cost axis while costing more. Without an
  infrastructure receipt the axis stays `unavailable` with
  `unavailable.infrastructure_cost.missing`, and `isolation_class: "none"` is
  how a run with no incremental infrastructure cost records that as a pinned
  zero rather than an omission.
- **`rebuildForensicScorecard` has a production caller.** `BuildScorecard` on
  the mounted forensic worker route (`/api/forensics/workers`) names runs and
  the frozen revisions being scored, then reads every event, adjudication,
  provider usage receipt, and infrastructure cost receipt back out of the
  owner-scoped durable ledger. A caller cannot hand in the measurements its own
  scorecard is judged on, and `generatedAt` is the server clock.

  A gate run over scorecards this projector built now reaches a `dominates`
  verdict with no axis `unavailable`
  (`forensic-prompt-compiler.test.ts`, "reaches a verdict on scorecards this
  repository's own projector built"). The evidence in that test is synthetic and
  labelled as such; it proves the gate is satisfiable, not that any candidate is
  good.

### Still open, and why the campaign criterion is descoped

- **There is no untouched holdout.** `fixtures/forensics/coldcard/dataset-splits.v1.json`
  declares `holdout` and `clean_holdout` as evaluator-only descriptors with
  empty `benchmarkArmRefs`, pointing at
  `dataset.private.untouched-security-holdout.v1` and
  `dataset.private.clean-security-holdout.v1`. No examples, digests, or source
  trees exist for either. The only real forensic corpus is the five Coldcard
  arms, which the benchmark manifest declares `development` and `control` —
  optimizer-visible by construction. Promoting them to a holdout would destroy
  the boundary the gate exists to protect, so it must not be done.

  The compiler now refuses that shape outright rather than passing it
  vacuously: disjointness between optimizer-visible and holdout examples is
  trivially satisfied by an empty holdout, so an empty untouched or clean
  holdout revision is rejected. Building a real one is corpus construction —
  separately owned material the optimizer has never seen, digest-pinned, with
  the contamination boundary enforced mechanically — not promotion-gate work,
  which is why it is descoped.
- **No accepted live worker receipts.** The forensic managed-sandbox dispatch
  route exists and is gated on `MANAGED_SANDBOX_BROKER_ENABLED` and a configured
  runtime, but issues #9289 and #9290 do not carry accepted live receipts, and
  the operator guide holds launch controls behind them.
- **Nothing turns a prompt into retained runs end to end.** `LoupeForensicBackend`
  has no implementation outside a test fake, and no component emits a
  `ForensicEvaluatorAdjudication` from a submitted finding; the six-link causal
  rubric exists only as prose. That surface belongs to the Loupe adapter lane
  (#9294), not to this one.

The one real forensic run in the tree,
`fixtures/forensics/coldcard/historical-import.v1.json` (Episode 264), does not
close the gap either. It is a single pair of runs on the optimizer-visible
development arms, executed by an external tool with verification disabled, with
one prompt rather than a baseline and a candidate, and with wall time and token
usage both `unavailable`. The gate would refuse it on the holdout boundary, the
matched-population requirement, and `insufficient_evidence`.

## Remaining limits

These should not be read as covered:

- No untouched-holdout campaign has been run, for the reasons above. The
  mechanism that would admit a winning candidate is in place and is now
  demonstrably satisfiable by projector-built scorecards; the claim that a
  specific candidate improved an untouched holdout without regressing the clean
  holdout or any hard gate is not evidenced by anything in this repository, and
  is tracked on #9300.
- Infrastructure cost is a retained receipt, not a reading taken from a billing
  authority. The projector requires it before it will report cost to
  identification, and it binds a retained turn window, but the micros figure is
  still an observation whoever ran the worker submits.
- The DSPy/GEPA honesty guard requires only a non-empty caller-supplied
  `integrationReceiptRefs` array. It constrains naming; it does not evidence a
  tested integration.
- Resolved worker and source evidence is structural validation of typed records
  submitted by the evaluator, not a lookup against a live placement authority.
