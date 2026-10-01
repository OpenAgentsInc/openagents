# NIP-EVAL — Evaluations

`draft` `optional` — v1, 2026-09-21. The [shared contracts](contracts.md)
are normative. This NIP lets hosts compare attributable workload evidence
for models, context policies, programs, plugins, skills, knowledge entries,
and complete agents.
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

Test-time capabilities: the [extension evaluation profile](#extension-evaluation-profile) carries the [capability claim](../../docs/essays/2026-09-29-test-time-capabilities.md#1-test-time-capability-ttcap) and its [delta](../../docs/essays/2026-09-29-test-time-capabilities.md#3-capability-delta) (subject and baseline arms, the claim's scope in `meta.ext_eval`), [reach and restraint](../../docs/essays/2026-09-29-test-time-capabilities.md#4-reach-and-restraint) (`should-fire` and `should-not-fire` cases), [reproduced capability claims](../../docs/essays/2026-09-29-test-time-capabilities.md#7-reproduced-capability-claim) (checks, with the reliance set a rerun shared), [externally validated capability claims](../../docs/essays/2026-09-29-test-time-capabilities.md#8-externally-validated-capability-claim) (`validates` results on an independent second suite), and [capability adoption](../../docs/essays/2026-09-29-test-time-capabilities.md#9-capability-adoption) (the admission's `validation`, `marginal`, `regression`, `reliability`, `authority`, and `stakes`) ([mapping](../../docs/essays/2026-09-29-test-time-capabilities.md#how-the-protocol-carries-test-time-capabilities)).

## Suite identity and intended claim

A suite has `v: "openagents.eval-suite.v1"`, `id` (qualified component ID),
`purpose`, `workload`, `cases`, `partition`, `labels`, `metrics`,
`acceptance` (DefinitionRef), and `environment` (ArtifactRef). Workload, cases,
partition, labels, and metrics are ArtifactRefs. Purpose is
`decision`, `context`, `routing`, `operation`, or `agent`. Case and partition
artifacts list stable case IDs and exact input/snapshot/expected-evidence
references. Workload describes collection method, task families, sampling,
known exclusions, and whether cases are synthetic or observed.

The workload is the **task distribution** a suite claims to sample (the D
of a capability claim); the cases and partition are the **sampled suite**
(the S). They are different objects: a reproduction holds S fixed and
changes who runs it, an external validation holds D and changes S, and a
transfer changes D. A workload MAY name its distribution with a stable
`distribution` ID, so a second suite can say whether it samples the same
population; a suite that names none claims the subject's own definition
ID, "the tasks this component claims to help with". A workload SHOULD also
carry the suite's **sampling story**, because a second author prevents
one kind of overfitting and makes neither suite representative: `frame`
(the source the tasks were drawn from), `inclusion` and `exclusions` (the
rules that decided which were in), `strata` (coverage categories and
their counts), and `method` (`random`, `exhaustive`, or `constructed`).
"Same D" then means the same `distribution` ID and comparable frames; a
workload with no frame claims only what its author says. The reference
runner writes all five, with `method: constructed`, since every starter
case was written by hand. Labels name
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
observations cannot be counted as zero cost or successful cases. A
`comparison` measurement is an estimated treatment effect: its uncertainty,
when given, names an interval and the method that produced it, with the
task as the unit and repeated attempts nested inside it (a paired bootstrap
over per-case differences is a floor, a hierarchical interval the goal).
A verdict is an engineering gate over the estimate, not the estimate.

Runs establish matched-case comparisons through exact case/input identities.
If baseline and candidate differ in sources, budgets, recipients, hardware,
verification, or the operations offered to the model, disclose that in limitations and narrow the
claim. A reduction in supplied tokens is not task success. Correct output shape
does not establish judgment accuracy, calibration, or safe execution.

## Required measurement profiles

A report claiming one of these benefits MUST include the listed populations
and adverse outcomes, with explicitly unknown values when unmeasured:

| Claim | Required observations |
| --- | --- |
| Better semantic decisions | Label coverage, unavailable/refused answers, harmful error directions, per-family results, and abstention outcomes. Calibration claims name `ece` (expected calibration error) and `brier` on a labeled partition, and, for a decision that serves through thresholds, the operating points: precision and coverage above each serving threshold, the abstention or escalation rate, and the risk–coverage curve they lie on. |
| Better context | Candidate retrieval recall and selected-evidence recall separately; missing mandatory constraints; stale inputs; expansion rate; decision overhead; resulting task outcomes. |
| Better compression or representation | Source completeness, summary sufficiency failures, expansion/recovery, and downstream correctness alongside bytes/tokens. |
| Better operation discovery | Eligible catalog and shortlist size, omitted needed operations, false activations, argument failures, schema/manual bytes, and full-task outcomes. |
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

## Gym results publication

This optional profile lets a publisher sign a released Gym leaderboard, the
static `openagents.gym.leaderboard.v1` file the Gym and the web read, so a
reader checks who released it as well as that its bytes match. A leaderboard
is a presentation of committed benchmark evidence: it isn't an
`openagents.eval-report.v1`, and this publication isn't a `3189` evaluation
declaration. It can't establish an independent pass, promote an
implementation, or rank anything outside the boards it names.

Kind `3195` is a regular immutable record, one per released leaderboard.
Its tags are exactly one `t: oa:gym-results:v1`, exactly one `x` equal to
the body's `digest`, and an optional NIP-31 `alt`. The content is a closed
object:

| Field | Contract |
| --- | --- |
| `v` | `"openagents.gym-results-publication.v1"`. |
| `requires` | Empty array. |
| `schema` | `"openagents.gym.leaderboard.v1"`, the leaderboard file's schema. |
| `digest` | The leaderboard's content digest, 64 lowercase hex: the ATIF rule (object keys sorted at every depth, then SHA-256) over its `boards`, as the leaderboard and its `openagents.gym.leaderboard-index.v1` entry record it. |
| `commit` | The 40-hex git commit the evidence was read at, as the index entry records it. |
| `boards` | The board IDs in the leaderboard's order: 1 to 64 unique IDs of lowercase letters, digits, `-`, and `.`, at most 128 characters each. |

The index is the discovery path for HTTP readers, which don't speak to a
relay: the publisher commits the signed event as NIP-01 JSON at
`signatures/<digest>.json` beside `index.json` in the publication
(`bench/terminal-bench/published/` in the OpenAgents repository), after the
leaderboard lands. A publisher MAY also send the same event to a relay; a
relay reader selects it with `{"kinds": [3195], "authors": [<publisher>],
"#x": [<digest>]}`. Signing the same leaderboard again produces another
event with the same statement; readers accept any valid one.

A reader verifies, in order, and only after the leaderboard itself verified
against the index's digest:

1. The event's ID and Schnorr signature, the kind, and the tags above.
2. The body's closed shape, `v`, `requires`, and `schema`.
3. `digest` equals the verified leaderboard's digest, `commit` equals the
   index entry's commit, and `boards` equals the leaderboard's board IDs in
   order. An index entry without a commit can't be matched.
4. The signer is a publisher the reader pinned in its own build or
   configuration. A reader never learns which keys to trust from the host
   that serves the files or from the event.

The outcome is shown beside the numbers, never instead of the digest checks:

- **No event** (the file is absent): "not signed". This isn't a failure;
  the leaderboard is still digest-verified.
- **Any failed check**: the signature is refused with the reason ("signed by
  npub1…, which isn't a pinned publisher", "it signs commit 1111111, not the
  index's 6bfc948"). The digest-verified numbers stay, labeled with the
  refusal.
- **All checks pass**: "signed by <publisher name> (npub1…)".

The OpenAgents publisher key is pinned in `PINNED` in
[`crates/gym-leaderboard/src/signed.rs`](../../crates/gym-leaderboard/src/signed.rs),
the list every reader built from this repository trusts by default. The
signature proves who released the leaderboard, not that its evidence is
correct or independently reproduced; a relay storing the event certifies
nothing. `crates/nostr` (`gym_results`) builds and checks the event,
`gym-leaderboard sign` signs and writes it, and the client in
`gym_leaderboard::client` verifies it (`docs/verse/gym-leaderboard.md`).

## Extension evaluation profile

`draft` — added 2026-09-28. **Partial**: the wire formats are
implemented (2026-09-29, [#9932](https://github.com/OpenAgentsInc/openagents/issues/9932)).
`crates/nostr` builds and checks every record here: `eval_ext` (the case
manifest, the suite and its package, the report, the publication, and
check linkage), `cj_conversation` (the draft, cards, and offers), and
`xp::eval_check` and `xp::eval_adopt` (credit). The schemas are
[`eval-case.v1`](schemas/eval-case.v1.json) and
[`ext-eval.v1`](schemas/ext-eval.v1.json); the fixtures are
`crates/nostr/fixtures/eval-ext/`. The runner, the hosted runner, the
referee, and the chat clients were built the same day (epic
[#9931](https://github.com/OpenAgentsInc/openagents/issues/9931),
[hosted runs](#hosted-runs)); the profile's claim-scope records (the
reliance set, identity strength, distribution, defaults), the `validates`
and `transfer` markers, the admission's validation and marginal fields,
and the cost-primary gate landed under
[#9956](https://github.com/OpenAgentsInc/openagents/issues/9956). This
profile carries the results of `openagents ext eval`
([extension evaluation](../../docs/extensions/evaluation.md)): a suite of
cases run against an agent with one extension admitted (the `subject`
arm) and with it absent (the `baseline` arm). It allocates no kinds. It
uses this NIP's report and `3189` publication, NIP-EXT releases for
suites and subjects, NIP-XP for credit, and NIP-CJ for hosted runs.

**Claim key, records, and policy.** A report is one **evidence record**
on a **claim key** K = (A, B, D, S, E, G, M): the subject arm's lock (A),
the baseline arm's lock and run configuration (B, E, G), the suite's
workload (D) and cases (S), and the suite's metrics and each case's
graders (M). Two reports with the same key are evidence about one claim,
and a check is a second record on the same key. The gate is the
**decision policy** P. Its digest, in `acceptance` and
`meta.ext_eval.gate`, is not part of the key: a replaced gate
reinterprets the records that exist and reruns nothing, and a report
judged by a since-replaced gate is still a record on the same claim,
read under the gate digest it carries. `meta.ext_eval.headline` is the
record's effect estimate (`eval_ext::Effect`: cases passed with the
subject minus without, over the total); no interval is carried yet.
Reports are evidence; claims summarize effects; adoption is policy.

### Suites

An extension suite is an `openagents.eval-suite.v1` with `purpose:
"operation"`. Its `cases` artifact (schema `openagents.eval-case.v1`) is
the **case manifest**, a closed object `{v: "openagents.eval-case.v1",
requires: [], cases}` whose `cases` lists 1 to 256 entries in
lexicographic order of `id`, each a closed object:

| Field | Contract |
| --- | --- |
| `id` | The case directory's name: ASCII letters, digits, `-`, `_`, and `.`, not starting with `.`, at most 128 bytes, and not `results`, `node_modules`, `.git`, or `.openagents`. Unique. |
| `kind` | `should-fire` or `should-not-fire`. |
| `runs` | Runs per arm, 1 to 10. |
| `prompt` | ArtifactRef to `prompt.md`. |
| `config` | ArtifactRef to `case.toml`, or null. |
| `graders` | `{name, artifact}` for each `graders/<name>.md`, in name order, at most 64. A case with no grader files has a `config`. |
| `fixtures` | `{path, artifact}` for each file under `fixtures/`, in path order, at most 256; a path is relative, of plain components, at most 512 bytes. |

Every case file's ArtifactRef (`prompt`, `config`, each grader) carries
schema `openagents.eval-case.v1`, and every file, fixtures included, is at
most 1 MiB. `acceptance` is the DefinitionRef of the Gym gate that decides
the verdict. Every gate declares the claim's **primary outcome** and the
measures it holds **non-inferior** (`rule.primary` and
`rule.non_inferiority` in the gate file, inside its digest): `ext-eval-v2`
is correctness-primary (`cases_passed`, higher; cost and time may not get
materially worse, and improving them alone is never Better);
`ext-eval-cost-v1` is cost-primary (`cost_usd`, lower; cases passed, mean
score, and time held non-inferior), the claim "the same correctness at a
lower cost"; `ext-eval-v1` names neither and reads as `ext-eval-v2` did,
for suites published before 2026-09-29. A cheaper run that is also wrong
is never Better under any of them. `labels` names the
suite author as the label source; a suite written by the extension's
publisher says so, and a reader weighs it as the publisher's own claim.

A public suite travels as a NIP-EXT release: a package whose manifest has
exactly one component of kind `eval-suite`, whose definition is the suite
artifact (schema `openagents.eval-suite.v1`), and whose listed files
include the case manifest and every case file by digest and size. The release signer (the
package root) is the suite author. A suite shipped inside the extension's
own package is an `eval-suite` component of that package. Revoking the
release withdraws the suite from new checks and quests; reports already
published keep their meaning.

### Reports

The report is `openagents.eval-report.v1` with these profile rules:

- `subject.definition` is the subject's DefinitionRef, with `event`
  set to its release when published. The subject is one of five kinds: an
  extension (a NIP-EXT `3184` release: a program, plugin, or skill in its
  package), a
  knowledge entry (NIP-KB's own profile, on a `3190`), a decision service
  (a NIP-CAP `30180` head, with `configuration` pinning the question set
  as `name@digest`), a context-construction policy (a NIP-PRG `30182`
  program head: a program that probes, selects, and orders the evidence
  another capability receives, measured with and without it like any
  other subject), or a delegate (a NIP-CAP operation DefinitionRef with
  the engine's artifact in `subject.lock`, and no event). Credit rules that
  pin a subject release stay on NIP-EXT releases. `subject.lock` is the
  run lock the subject arm held. `baseline` has the same shape with no
  component of the subject's release in its lock. A **standalone**
  report's baseline holds nothing admitted; a **marginal** report's
  baseline holds the current defaults (the `coder-defaults` release named
  in `meta.ext_eval.defaults`), so its delta is the candidate's marginal
  effect on the composition, which is what adoption decides on. A report
  without a baseline can't claim a change and has verdict `inconclusive`.
- `runs` entries carry `{arm, case, attempt, outcome, receipts,
  artifacts}` where `artifacts` includes the run's ATIF log ArtifactRef
  (schema `ATIF-v1.8`, [NIP-ATIF](NIP-ATIF.md)) and the grader answers.
- `measurements` include, per arm, `cases_passed` (denominator: cases
  scored), `mean_score`, `cost_usd` and `seconds` (with `unknown_count`),
  and, for the `comparison` arm, `change` (subject minus baseline).
- `verdict` is the gate's decision; the gate's digest is in `acceptance`
  and repeated in `meta.ext_eval.gate`.
- A runner SHOULD earn the words "treatment effect": interleave the arms
  attempt by attempt, or randomize their order, rather than run one arm
  to completion first; give every attempt its own isolated workspace so
  that neither arm leaves caches, files, or other state the other
  benefits from; pair the same case and, where it means anything, the
  same seed across arms; and stamp each run with its time and the
  provider it reached. The reference runner interleaves, isolates, and
  stamps, and does not randomize. `eval_ext` checks none of this; a
  reader judges it from `runs`.
- `meta.ext_eval` is `{v: "openagents.ext-eval.v1", gate, cases:
  [{id, kind}], headline: {subject_passed, baseline_passed, total},
  requester, reliance?, identity?, distribution?, defaults?}`, closed.
  `gate` is the gate's `sha256:` digest; `cases`
  repeats each case's ID and kind; `total` is the case count and neither
  arm passes more; `baseline_passed` is null exactly when `baseline` is.
  `requester` is null, or the `{id, pubkey, kind: 25920}` of the signed
  NIP-CJ execution request a hosted runner served. The four optional
  records are the rest of the claim's scope, and a report written before
  they existed keeps its bytes:
  - `reliance` is the run's **reliance set**, a closed object `{runner,
    host, door, model, agent, selector, graders}` whose entries are the
    identities the runner could name (a pubkey, a `name@version`, a
    digest of the hostname, never the hostname) or null. A reader compares
    a check's with the original's: what both named alike the rerun
    **shared** (independent as a signing principal, not as that
    platform), what both named differently it **varied**, and what either
    left null is unknown, not independent.
  - `identity` is the strongest identity the subject has: `content`
    (exact bytes under a lock: every extension), `version` (a build or
    weights digest a provider declares), `endpoint` (only a provider,
    model name, or endpoint is known), or `unresolved`. Reproducibility
    cannot be stronger than identity.
  - `distribution` is the task distribution the suite's workload claims
    (its `distribution` ID), or null for the subject's definition ID.
  - `defaults` is the `{id, pubkey, kind: 3184}` of the defaults release
    whose lock the baseline arm held, for a marginal report; null for a
    standalone one.
- The report is closed to NIP-EVAL's fields plus `meta`. `suite` is an
  ArtifactRef with schema `openagents.eval-suite.v1`, and with `event`,
  the suite's NIP-EXT release (kind `3184`), when published.
  `subject.definition.event`, when present, is the extension's release.
  Each arm's coverage sums (the five outcome counts equal `attempted`),
  `coverage.baseline` is null exactly when `baseline` is, and no
  measurement names the `baseline` or `comparison` arm of a report
  without one.
- A **decision-service record** (added 2026-09-29,
  [#9959](https://github.com/OpenAgentsInc/openagents/issues/9959)) is
  this report on a router or another decision service, the first being
  the chat router (`crates/coder/src/router_claim.rs`, the Gym suite
  `chat-router-v2` under the `router-v1` gate). Its subject's
  `configuration` pins the question set and the answer bank as
  `name@digest`, the identities a NIP-CJ `judgment` carries in `set` and
  `bank` and the worker logs at start, so a judgment on the wire, a Gym
  row, and a report name one question by one digest. Its suite's `cases`
  artifact is the Gym suite file (`openagents.gym.suite.v1`) and its
  `partition` the suite's locked rows, with a `calibration` array beside
  `development` naming the rows a calibration map was fitted on. A case
  is `should-fire` when a correct decision acts on its own reading (a
  prepared answer, a stem, an offer, a refusal, a Gym reply, an interview
  step) and `should-not-fire` when it stands back and the model answers
  alone; a case passes when the decision matches the label and made no
  harmful error (a wrong whole answer, an offer or a Gym tier on another
  route's row). `identity` is `version` at best (the question set and
  bank by digest, the judge by name; the model behind the judge is not
  pinned). The measurements are the semantic-decision profile's:
  `cases_passed`, `route_accuracy`, `route.<route>.precision` and
  `.recall` per family, `canned_precision`, `canned_recall`,
  `dispatch_precision`, `abstention_rate`, `<question>.ece` and
  `<question>.brier` raw and `_calibrated` on the held-out rows, and the
  operating points `<question>.precision_at_<t>`, `.coverage_at_<t>`, and
  `.abstention_at_<t>` at each serving threshold, which are the
  risk–coverage curve. `eval_ext` checks it as any report: a first record
  with no baseline is `inconclusive`, and the gate's product floors are
  written into `limitations`.

### Publication

A result is a `3189` publication as above, with these additions:

- Tags: exactly the markers `t: oa:eval:v1` and `t: oa:ext-eval:v1`
  among `oa:` `t` tags; one `x` equal to the report digest; and `e` tags,
  each with a marker in the fourth position (`["e", <id>, <relay or "">,
  <marker>]`): exactly one `suite` (the report's `suite.event`, required
  to publish), one `subject` exactly when the subject is published (its
  `subject.definition.event`), at most one of `check` (the publication
  this one reruns on the same suite), `validates` (the publication this
  one tests on a second suite of the same distribution), and `transfer`
  (a second suite of another distribution), and one `request` exactly
  when the result is hosted. A
  hosted result also carries exactly one `p` tag, the requesting trainer;
  any other result carries none. An `e` tag with another marker, or none,
  refuses.
- `meta` holds `ext_eval_report`: the report's exact bytes, at most
  64 KiB, which a reader checks against `report.digest` before reading.
  A hosted result also holds `ext_eval_request` (added 2026-09-29): the
  trainer's signed NIP-CJ `25920` request, the whole event, at most
  96 KiB as JSON. Relays keep no `25920`, so this is where a reader finds
  it. Any other `meta` key refuses, and so does a request that isn't the
  one the report's `requester` names, whose signature fails, whose one `p`
  isn't the result's signer, or on a result that isn't hosted.
  The body's `subject` is the report's `subject.definition`, and the
  report ArtifactRef's schema is `openagents.eval-report.v1`.
- The signer is the evaluator, as for every `3189`. A hosted runner signs
  its own results; the trainer who asked is named by `requester`, and a
  reader verifies that request's signature before crediting them, from
  `meta.ext_eval_request` (or a copy of the request the reader holds).

A reader lists results with `{"kinds": [3189], "#t": ["oa:ext-eval:v1"]}`
and groups them by subject. It never ranks results from different suites
against each other.

### Checks

A check is a publication whose report has the same suite ArtifactRef and
the same subject DefinitionRef as the publication it cites with the
`check` marker, and a different **trainer**. A result's trainer is the
requester of a hosted run, otherwise its evaluator, so two hosted runs by
one runner are a check when different trainers asked for them. A check
**confirms** when its verdict equals the original's and **disputes**
otherwise; readers show both counts beside the original. A check with a
different lock for the subject arm is not a check of that result; readers
show it as a separate result. `eval_ext::confirms` decides this from the
two signed events alone. A verdict match is the operational
simplification, and it is lossy: two checks can both read **Better**
while estimating +1 and +5 of 6. Readers SHOULD show the two headlines'
effects beside the confirm and dispute counts, and
`eval_ext::Effect::compatible` (the same total, the same direction,
deltas within a stated number of cases) is the comparison until reports
carry intervals.

### Validations

A **validation** is a publication whose report has the same subject
DefinitionRef and the same subject-arm lock as the publication it cites
with the `validates` or `transfer` marker, and a *different* suite. It
answers what a check can't: whether the delta was fitted to the author's
own tests. Two things make it count, and a reader checks both from the
two publications and the two NIP-EXT releases they name
(`eval_ext::validation`): its **distribution** and its **independence**.
A `validates` result whose distribution (its own `meta.ext_eval.distribution`
or the subject's definition ID) equals the original's reads as a
validation; one that names another distribution, or a `transfer` result,
reads as a **transfer**, a new claim rather than a stronger version of the
old one. Independence has two halves: the second suite's release is signed
by someone other than the subject's release signer (**provenance**), and it
was created after the subject's release (**chronology**), so the author
could not have tuned the artifact against it. A same-signer or
earlier-suite result stays visible and validates nothing. A validation
that came out other than **Better** validates nothing either. Readers show
a result's validations beside its checks. Whether one validation makes a
capability a candidate is the host's policy; Coder's asks for one beside three
confirming checks.

### Hosted runs

A hosted runner is a NIP-CJ execution worker. The request's `target` is
the DefinitionRef of the `ext-eval` program, `input` is `{suite, subject,
runs, baseline: true}` with ArtifactRefs (or a chat draft artifact of at
most 64 KiB), and `requirements` names the read-only and sandbox-write
effects only. The worker admits only subjects its operator lists (the
catalog and `coder-defaults`) and suites within its published bounds, and
refuses others as `not_admitted`. A hosted request runs at most 8 cases,
3 runs per arm, and 2 arms. A reader credits the requester only after
checking the request event the report names: its ID and signer, kind
`25920`, its signature, and its one `p` tag naming the runner that signed
the result. The result carries that event in `meta.ext_eval_request`. Its result's body carries the report ArtifactRef; the report stays private (the `3188` envelope to the
requester) until the requester sends a publish request naming it.

Built (2026-09-29, [#9935](https://github.com/OpenAgentsInc/openagents/issues/9935)):
`nostr::eval_ext::hosted` holds the wire and `crates/eval-runner` the
runner. A request's `target` is `<runner>:ext-eval/run` pinned by the
hosted program's description, and `lock`, `context`, and `requirements`
are fixed documents (`requirements` names reads and sandbox writes only).
`input` (`openagents.ext-eval-hosted.v1`) is one of two actions: `run`
(`suite`, a `3184` EventRef or `"draft"`; `subject`, a DefinitionRef or
`"draft"`; `draft`, the NIP-CJ draft exactly when a side is the draft;
`runs`, 1 to 3; `baseline: true`; `check`, the publication a rerun
checks, or null; and, optionally, `validates`, the publication this run
externally validates on a published second suite, never together with
`check`, which the runner publishes with the `validates` marker after
checking that the cited result is on the relay, tested the same tool,
and ran another suite) and `publish` (`report`, the ArtifactRef a run's
result named, sent by the same trainer). Once a `coder-defaults` release
admits anything the runner holds, both arms of every run admit it and
the report names the release in `meta.ext_eval.defaults` (a marginal
report). The runner answers `27020` `accepted`
and `progress` with `meta.ext_eval: {completed, planned}` in case runs,
and one `26920` whose `output` is `{v, action: "run", report, sealed,
headline, verdict, notes}` or `{v, action: "publish", suite_release,
result}`. It refuses `not_admitted` (not a catalog or chat-made capability, a
suite asking for `exec` or `network`, a check of a result not on the
relay, another trainer's report, or admission switched off),
`over_quota` (only when the operator has set an emergency brake on runs
per trainer or turns per day; OpenAgents' runner sets none, and a client
shows it as "try again later" without naming a count), and `too_large`
(over 8 cases, 3 runs, 2 arms, a 64 KiB draft, or a result the relay
can't hold), each before anything runs. A check never counts against a
brake on the trainer's runs.

### Adoption

Adopting an extension into a host's defaults is an
`openagents.eval-admission.v1` decision (above) whose `reports` cite the
extension's published reports and their confirming checks, and whose
`validation` cites at least one externally validating result: an `admit`
with an empty `validation` is refused. The decision may also cite, and a
host's policy may require, `marginal` (the report whose baseline held the
current defaults, once the defaults hold anything), `regression` (the
whole default set with the candidate, on what it passed before),
`reliability` (repeat consistency, robustness to changes that don't change
the task, calibration, tail failures, abstention, and composition depth),
`authority` (`{before, after}`: the default set's effective authority as
two effects objects), and `stakes` (`{severity: low | moderate | severe,
authority: none | read | write | act | spend, reversibility: reversible |
costly | irreversible}`), with the evidence adoption demands rising with
the stakes. Utility establishes the claim; safety and stakes decide
admissibility; neither is traded against the other. Adoption is not
terminal. What a change means depends on which part of the claim key
moved:

| What changed | What it means |
| --- | --- |
| A (the subject or its dependencies), B (including the defaults), E, G, or M | The key changed: revalidate with new reports on the new key, checked and validated like the first |
| S, with D fixed | An external validation, not a trigger |
| D | A transfer: a new claim, never a reopening |
| P (the gate) | Reinterpret the existing reports under the new gate; rerun nothing unless M moved too |

Because B includes the defaults, every adoption changes B for every
adopted subject. A host SHOULD run a whole-default `regression` on every
defaults release and reopen an individual subject's claim only where
that regression or a changed dependency touched it, rather than
revalidate every adopted subject on every adoption. A lapsed claim leads
to quarantine (a later defaults release that no longer depends on the
subject) or revocation (a NIP-EXT `3185`), and active runs keep the lock
they started with (NIP-POL). The admission's `expires_at` is where
identity sets the clock: the weaker the subject's identity
(`meta.ext_eval.identity` on the cited reports), the sooner the evidence
expires. Coder's policy gives a content-addressed subject 365 days, a
version-addressed one 90, and an endpoint-addressed one 14
(`packages/coder-defaults/package.json`, `candidate.expiry_days`); an
`unresolved` subject is never adoptable, and `eval-adopt` refuses an
admission that cites such a result. For Coder's
defaults, the admitted change is then published as a NIP-EXT release of
the `coder-defaults` package that depends on the extension's release, and
whose manifest `provenance.receipts` cites the admission's ArtifactRef
(schema `openagents.eval-admission.v1`), and whose `dependencies` include
the extension's release ID. The admission's `subject` carries the
extension's release as its `event`. The release is the public, checkable
record of adoption; the admission itself
stays with its issuer. NIP-XP's `eval-adopt` rule reads the release. A
runtime that consumes the defaults reads the same release and admits a
dependency only under a live admission it holds (its `decision` is
`admit`, its `subject.event` is that dependency, and its `expires_at`
hasn't passed); it records the defaults lock it admitted under in the
run (`openagents.coder-defaults-lock.v1`, the `defaults` of a NIP-RUN
`created` record's lock). A dependency without such an admission admits
nothing.

## Promotion and learning

A promotion decision has `v: "openagents.eval-admission.v1"`, `subject`
(DefinitionRef), `reports` (ArtifactRefs), `policy` (DefinitionRef), `scope`
(ArtifactRef describing workload/model/recipient limits), `decision`
(`admit`, `reject`, or `inconclusive`), `issuer` (pubkey), and `expires_at`,
and optionally `validation` (ArtifactRefs), `marginal`, `regression`, and
`reliability` (ArtifactRefs or null), `authority` (`{before, after}` or
null), and `stakes` (see [Adoption](#adoption)). An `admit` cites at least
one validation.
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
Extension evaluation fixtures (`crates/nostr/src/eval_ext/tests.rs` and
`crates/nostr/fixtures/eval-ext/`) cover a wrong or missing profile marker,
a report digest mismatch, `meta.ext_eval_report` over 64 KiB, a missing or
wrong suite `e` tag, an unmarked `e` tag, a signer who isn't the
evaluator, a hosted result without its `p` tag or with a request sent to
another worker, a report without a baseline claiming a change, coverage
that doesn't sum, an out-of-order, duplicate, reserved, or oversize case,
a suite of another purpose or gate, a suite package with two suites or an
unlisted case file, a check with another subject lock (not a check), a
self-check, and a hosted check by the same trainer.
Advertise `nip-eval-v1` only for the tested publication/client validation role.
A relay cannot certify task quality, statistical validity, or calibration by
storing a signed result.

## Private Gym boards and admitted recipe control

This optional bounded profile serves live and recent Gym projections to an
explicitly paired client. It uses signed, encrypted private artifacts (`3188`)
for grants, requests, and replies; it allocates no new event kind. A board is
an observation, not an `openagents.eval-report.v1` or a public `3189`
publication. It cannot establish an independent pass, promote an implementation,
or authorize training through possession of an evaluation report.

Unlike general CAP/CJ execution, this profile supports only operator-installed,
fixed local recipes under its own separately admitted grant. It does not claim
CJ worker compatibility, arbitrary operation dispatch, or a RUN journal. Hosts
needing those broader contracts use the existing CAP/CJ/RUN profiles. An existing
SESS history-observation or CTRL task grant MUST NOT be widened into this grant.

### Pairing and authority

An operator selects exact local source roots and optional executable recipes,
then signs an `openagents.gym-grant.v1` body with `host`, `client`, `relay`,
`grant`, `observe`, `sources_digest`, `recipes`, `issued_at`, and `expires_at`.
`v` is required and all objects have closed shapes. `observe` is true;
`recipes` MAY be empty and only the listed exact revisions can launch. Client
and host keys MUST differ. `sources_digest` binds canonical local source records,
including the root's device and inode, without publishing paths in the grant.
Every grant expires within 30 days and is durably revocable by its host.

The public `openagents.gym-connection.v1` body contains `v`, `host`, `client`,
`relay`, `grant`, `expires_at`, and the original signed encrypted grant event
as `authorization`. Its paste representation is `gym-connect:` followed by
base64url without padding of canonical JSON. The authenticated transfer of this
code pins the host; the client verifies every repeated field against the opened
grant and proves its own key by signing each request. No private key is included.
Production relay URLs require exact credential-free `wss`; explicit fixture
policy MAY admit `ws` to numeric loopback addresses only.

Each recipe has `id`, `title`, `revision`, `budget`, and `detail`. A revision is
a Digest over its full local configuration, executable digest, and admitted
working-directory identity. The reference host configures a canonical executable,
fixed arguments, fixed working directory, wall deadline, maximum starts, and
allowlisted environment variable names. Clients cannot amend these values.
Environment values remain private host inputs. A top-level executable pin does
not establish immutable transitive scripts, dependencies, models, or environment
contents; those require a separately pinned experimental closure.

Budget is `{wall_ms, max_starts, spend_limit_usd, spend_enforced}`. The reference
host admits 1–86,400,000 wall milliseconds and 1–32 starts per granted recipe.
It requires null `spend_limit_usd` and false `spend_enforced`; it MUST NOT claim
an enforced dollar ceiling from a nominal provider budget. Grant possession is
explicit authorization for these local commands and their possible spend,
subject to current revocation, launch allowance, and host admission.

### Requests and bounded observations

`openagents.gym-request.v1` has `v`, `request`, `grant`, `authorization`,
`issued_at`, `expires_at`, and `query`. The first three identity references are
random common IDs or an exact original authorization event ID as appropriate.
A request is valid for at most 60 seconds and no longer than its grant.
The envelope's signer, recipient, issue time, retention time, and random `h`
mailbox MUST bind the body; mailbox equals `request`. Signature and NIP-42 relay
authentication are separate checks.

Supported queries are closed objects:

- `{kind: "snapshot"}` requests a current finite board.
- `{kind: "launch", request_id, recipe_id, revision}` requests the exact granted
  recipe. `request_id` is the durable logical launch identity, independent of
  a fresh transport request and its expiration.

`openagents.gym-reply.v1` has `v`, `request`, `request_event`, `grant`,
`issued_at`, `expires_at`, and `response`. The reply signs the exact original
request event ID, logical transport request, grant, and recipient. Its expiration
MUST NOT exceed the request. Response is `{kind, value}` where kind is `snapshot`,
`launch`, or `refused`; the last value is a stable refusal code. Replies with
incorrect routing, signatures, schema, size, or request correlation refuse.
A relay ACK proves transport acceptance only. Clients subscribe before publishing.

A snapshot has `observed_at`, `runs`, `recipes`, and `notices`. Runs contain
`id`, `title`, `category`, `status`, `completed`, `total`, `cost_usd`,
`elapsed_ms`, `metrics`, `source`, and `provenance`. Categories are `evaluation`,
`agent`, and `training`. Status is `queued`, `running`, `completed`, `failed`,
`cancelled`, `unknown`, or `stale`. Completion is not benchmark success.
Nullable measurements MUST remain null when unavailable. A source-observed
running file does not prove live process ownership. Training-summary sources
are operator declarations, not independently verified model-training receipts.

Metrics are `{name, unit, points}`, with points `{step, value}`. Points MUST
have finite values and strictly increasing recorded steps. Missing costs MUST
NOT become zero, and a partial tail MUST NOT be represented as a complete cost
history. The reference Microcoder adapter requires all cost components to be
explicitly known and no unknown charges before returning a known total.

Maximums are 128 KiB canonical application JSON, 64 runs, 16 recipes, four metrics
per run, 64 points per metric, and 16 notices. Text is bounded and excludes
control characters. The reference host bounds source enumeration to 512 entries
and 8 MiB read per board, one MiB per JSON file, and the last 64 KiB/256 events
per Microcoder tail. A partial scan carries an explicit notice. Source roots and
all descendant reads are confined through directory descriptors without following
symlinks. Replaced roots refuse instead of silently admitting new data.

### Dispatch, retries, and recovery

A launch receipt has `request_id`, `run_id`, `recipe_id`, `revision`, `status`,
`submitted_at`, nullable `finished_at`, and nullable `exit_code`. A host MUST
serialize grant validation, current revocation, recipe-pin checks, bounds,
allowance consumption, and durable launch intent before dispatch. Request expiry
is checked again after potentially slow source or executable reads.

An exact `(grant, request_id)` retry returns the retained work identity and its
current receipt without dispatching again. Different recipe semantics under
that identity refuse as conflict. A timeout or invalid reply can occur after
dispatch; clients preserve the launch ID and show delivery as unknown. Only a
verified signed refusal confirms rejection. A new launch ID is new work.

The reference host runs one active or unresolved launch at a time, owns one
foreground service lock, and supervises each command under its declared deadline
with bounded output. Read/revoke transactions use a separate short lock.
On restart, unfinished launch intents become unknown; they are never silently
rerun. Unknown work blocks further dispatch pending operator investigation.
This is durable deduplication, not exactly-once effects or automatic reconciliation.

Revocation denies future reads and dispatches. It does not cancel an already
admitted child or revoke previously disclosed ciphertext. Output stays in the
private host store, outside ordinary board replies. No source scan, recipe
execution, or publication occurs merely because a client starts, enters a scene,
subscribes to Verse presence, or reconnects. The Verse Gym client activates its
finite reads only inside the Gym and stops them on exit or suspension.
