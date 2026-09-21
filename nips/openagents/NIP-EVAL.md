# NIP-EVAL — Workload evaluation evidence

`draft` `optional` — v1, 2026-09-21. The [shared contracts](contracts.md)
are normative. This NIP lets hosts compare attributable workload evidence
for models, context policies, tools, plugins, programs, and complete agents.
It does not create a global leaderboard, universal confidence threshold,
automatic training pipeline, or remote attestation.

All artifacts include `v`, `requires`, and optional inert `meta`. Evaluate
locally by default. Private reports use the scoped `3188` envelope; approved
public reports may use `3189` below. Reports retain and reference the exact
execution, decision, trajectory, and evaluator record identities.

Suites are domain-specific and reports are general. A coding suite may check
tests and patches; research may check citations and coverage; a records workflow
may check correct fields, recipients, and confirmed effects. Task success,
harmful error directions, human escalation, and irreversible outcomes must be
defined for the actual workload. A coding benchmark cannot admit an agent for
another domain merely because both use the same model or NIP schemas.

## Suite identity and intended claim

A suite has `v: "openagents.eval-suite.v1"`, `id` (qualified component ID),
`purpose`, `workload`, `cases`, `partition`, `labels`, `metrics`,
`acceptance` (DefinitionRef), and `environment` (ArtifactRef). Workload, cases,
partition, labels, and metrics are ArtifactRefs. Purpose is
`decision`, `context`, `routing`, `operation`, or `agent`. Case and partition
artifacts list stable case IDs and exact input/snapshot/expected-evidence
references. Workload describes collection method, task families, sampling,
known exclusions, and whether cases are synthetic or observed. Labels name
their source, rubric, annotator/procedure, and uncertainty. Environment pins
the runner, toolchain, execution policy, and relevant hardware/configuration.

The partition artifact contains `development`, `held_out`, and `excluded`
case-ID arrays, with no overlap or duplicate IDs. Membership refers only to
cases in the suite. Record tuning access and contamination disclosures;
different partitions with the same display name are different artifacts.
A spent held-out partition cannot silently remain evidence of unseen quality.
Sources, cases, labels, and credentials stay private unless explicitly cleared
for release. A suite cannot acquire permission to mirror production traffic.

Metric definitions contain unique `id`, `unit`, `direction` (`higher`, `lower`,
or `descriptive`), `population` (case/attempt selection rule), `aggregation`
(registered operation DefinitionRef), and `missing` (`count_as_failure`,
`report_separately`, or `refuse`). Unsupported aggregation refuses; a free-form
formula is not executable. Acceptance references a host-supported policy
definition with workload-specific error tolerances and abstention rules.
No probability threshold or price is fixed universally by this protocol.

## Reports, comparisons, and unknowns

A report has `v: "openagents.eval-report.v1"` and these fields:

| Field | Contract |
| --- | --- |
| `suite`, `partition` | Exact ArtifactRefs. |
| `subject` | `{definition, lock, configuration}` with DefinitionRef and ArtifactRefs. Model-backed subjects additionally pin the actually served model in execution receipts. |
| `baseline` | Same subject shape or null; claims of improvement require a baseline. |
| `evaluator` | Exact signer pubkey or local provenance ID. |
| `started_at`, `ended_at` | Observed Unix seconds, ordered. |
| `runs` | ArtifactRef to ordered `{arm, case, attempt, outcome, receipts, artifacts}` entries, including failures, refusals, and unknowns. Arm is `subject` or `baseline`; the latter requires a declared baseline. |
| `coverage` | `{subject, baseline}` with baseline null when absent. Each arm contains `{planned, attempted, completed, refused, failed, cancelled, unknown, excluded}` counts. |
| `measurements` | Entries `{arm, metric, value, denominator, unknown_count, uncertainty, evidence}`. Arm is `subject`, `baseline`, or `comparison`. |
| `verdict` | `pass`, `fail`, or `inconclusive` under the pinned acceptance policy. |
| `limitations` | ArtifactRef to bounded explicit limitations and exclusions. |

Attempts count admitted executions; planned/excluded counts describe cases.
For each arm, the five terminal outcome counts sum to attempted. Completed does not imply
verification or integration acceptance; those remain in receipts and metrics.
Report repeated attempts and selected-best policies explicitly. Each measurement
names a suite metric. Value is a finite number or null; denominator and
unknown count are nonnegative integers; uncertainty is a pinned method/result
ArtifactRef or null; evidence contains receipt/artifact references. Missing
observations cannot be counted as zero cost or successful cases.

Runs establish matched-case comparisons through exact case/input identities.
If baseline and candidate differ in sources, budgets, recipients, hardware,
verification, or offered tools, disclose that in limitations and narrow the
claim. A reduction in supplied tokens is not task success. Correct output shape
does not establish judgment accuracy, calibration, or safe execution.

## Required measurement profiles

A report claiming one of these benefits MUST include the listed populations
and adverse outcomes, with explicitly unknown values when unmeasured:

| Claim | Required observations |
| --- | --- |
| Better semantic decisions | Label coverage, unavailable/refused answers, harmful error directions, per-family results, and abstention outcomes. Calibration claims need a pinned calibration metric and labeled partition. |
| Better context | Candidate retrieval recall and selected-evidence recall separately; missing mandatory constraints; stale inputs; expansion rate; decision overhead; resulting task outcomes. |
| Better compression or representation | Source completeness, summary sufficiency failures, expansion/recovery, and downstream correctness alongside bytes/tokens. |
| Better tool discovery | Eligible catalog and shortlist size, omitted needed operations, false activations, argument failures, schema/manual bytes, and full-task outcomes. |
| Better model routing | Entire matched task cost/time, observed versus estimated cache usage, context rebuilds, failed/escalated attempts, quality, and disclosure refusals. |
| Better parallelism or reuse | Preparation/assimilation cost, duplicate work avoided, invalid reuse, contention, stale integration, unknown effects, and independent final verification. |
| Better background assistance | Foreground latency, incremental compute/disclosure/spend, useful/noisy/stale findings, and accepted versus merely proposed outcomes. |

Reports can reference finer-grained measurements and trajectories as artifacts.
Do not invent a success rate for a profile with no labeled denominator. Cost, token-efficiency, and hierarchical-search complexity claims require
measurement on the stated workload.

## Optimization evaluation

For an [OPT](NIP-OPT.md) study, a report additionally requires
`optimization: {study, candidate, phase, materializations}`. Study is an
ArtifactRef; candidate is a candidate ArtifactRef or null for a baseline-only
report; phase is `search`, `selection`, or `confirmation`; materializations
is an array of ai-materialization ArtifactRefs, one per loaded attempt.
Unloaded/refused attempts remain in the report with no fabricated materialization.
Per-trial reports do not reference the enclosing trial record, avoiding a
cyclic digest; trial records may reference their reports.

The suite and phase case membership MUST match the frozen study/data plan.
The report subject must equal the candidate implementation, or the study
baseline when candidate is null. A comparison baseline must equal the study
baseline. Match each materialization to its own subject/baseline arm, case,
and attempt; one loaded execution cannot stand for several independent trials.
Every scored attempt binds the actual loaded functional closure,
host binding, model observations, and environment to its execution receipts.
A mismatched candidate or insufficient required identity assurance cannot
establish a passing quality claim.

Search and selection results are development evidence. Neither can be labeled
unseen confirmation. Confirmation requires recorded candidate selection and
policy commitment before protected exposure, with allowance consumption and
known contamination disclosed. A search budget, favorable example, or reused
validation score is not a confidence interval.

Freeze labels, graders, metrics, aggregation, missing-result rules, and the
acceptance policy outside candidate control. Record model judges and their
uncertainty/bias; a judge optimized on candidate outcomes cannot independently
confirm them. System-level reports include task success, adverse outcomes,
escalation, and total latency/cost alongside module metrics. Account separately
for the cost of optimization and runtime performance. Include failed builds,
refusals, cancelled trials, unknown spend, and unselected candidates in study
evidence under the retention policy.

Improvement claims must state comparison design, repetitions, uncertainty,
workload scope, model assurance, and minimum detectable effect where relevant.
No optimizer or model family receives a universal acceptance threshold.
A result may be inconclusive or show no improvement without invalidating the
study's execution. EVAL admission never resets data exposure or modifies an
active implementation.

## Publication and disclosure

Kind `3189` is a regular public evaluation declaration with exactly
`t: oa:eval:v1` and `x` equal to the report ArtifactRef's 64-hex digest suffix.
Body is `{v: "openagents.eval-publication.v1", requires: [], report,
subject, supersedes}` with report ArtifactRef, exact subject DefinitionRef,
and a list of prior publication EventRefs, possibly empty. The resolved report
subject must match. `supersedes` asserts a revision relationship; it does not
delete unfavorable history or give a later report more credibility.

The publication signer must be the report's evaluator pubkey. Reports with
only local evaluator provenance require a new signed evaluator attestation
before publication; a mirror may redistribute the exact original signed
event without claiming to have run the evaluation. Relays validate envelope
syntax and indexed digest agreement. Clients verify report bytes, evaluator
identity, suite/subject closure, and evidence available under their authority.

Public release is an explicitly authorized disclosure operation. Review case
names, repository identifiers, source hashes, derived text, and reachable
locators as well as top-level prose. Redaction produces a new report identity
with retained provenance, limitations, and correct aggregate denominators.
Publish only references intentionally cleared for public discovery. If detailed
evidence is unavailable to a reader, label the result an unverified claim at
that reader's assurance level, never independently reproduced evidence.

EXT operation descriptors may link signed evaluation publications or private
report ArtifactRefs under policy. A package publisher's report is attributable
evidence, not independent certification. Users choose trusted evaluators and
workload fit. Conflicting reports remain separate; no latest-event rule picks
a universally best model, plugin, or policy.

## Promotion and learning

A promotion decision has `v: "openagents.eval-admission.v1"`, `subject`
(DefinitionRef), `reports` (ArtifactRefs), `policy` (DefinitionRef), `scope`
(ArtifactRef describing workload/model/recipient limits), `decision`
(`admit`, `reject`, or `inconclusive`), `issuer` (pubkey), and `expires_at`.
It is private by default and requires an independently trusted host/operator
issuer. Admission applies only to the exact scoped subject and does not grant
execution. Report publication never mutates an active lock, deployment, question
set, or threshold. New versions require explicit measured promotion.

Background evaluation, traffic shadowing, data labeling, model training, and
export are separate effectful jobs with their own grants, retention policy,
recipients, and budget. Reusing source captures does not authorize training on
them. The protocol defines records for the evidence; evaluators, host schedulers,
and operators own running experiments and consuming results.

## Conformance

Fixtures cover duplicate/overlapping partitions, mismatched subject/model,
missing or counted-twice attempts, cherry-picked exclusions, unknown costs,
invalid denominators, unavailable receipts, forged evaluators, disclosure
through locators, redaction identity changes, and unauthorized promotion.
Advertise `nip-eval-v1` only for the tested publication/client validation role.
A relay cannot certify task quality, statistical validity, or calibration by
storing a signed result.
