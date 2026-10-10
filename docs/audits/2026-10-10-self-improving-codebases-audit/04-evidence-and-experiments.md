# Evidence and experiments

The evidence supports continued investment in bounded preparation and cheaper
execution. It does not yet support a general claim that recursive learning
makes accepted changes cheaper, faster, or more reliable. Published results
include genuine cost wins, slower routed execution, and executable defects that
initial acceptance missed. Preserve all three when explaining the product.

This chapter uses repository artifacts and issue records, not new paid model
runs. Its current-source base is
`07805e6a7c3513a057d226b488cb2d40fd974a64`. Numerical historical results below
are attributed to their retained studies; the local recomputations performed
for this audit are listed separately.

## EVAL-01: Trace counts overstate task diversity unless grouped

**Priority: P1 for the next learning claim.**

The product spec's 41 admitted traces are real entries in the
[October 10 manifest](../../coder/traces/2026-10-10-manifest.json). This audit
recomputed its SHA-256 digest successfully and counted every row:

| Measurement | Count |
| --- | ---: |
| Retained replay receipts | 45 |
| Verified replays | 41 |
| Verified replays whose checks pass | 22 |
| Verified replays whose checks fail | 19 |
| Unverifiable receipts, all missing a recorded diff | 4 |
| Distinct issues among verified replays | 3 |
| Distinct issues among all receipts | 5 |

The verified rows comprise 18 runs of #10074, 22 of #10228, and one of #10273.
The four unverifiable issue-run records concern #11121 and #11159. The
[#11218 closing comment](https://github.com/OpenAgentsInc/openagents/issues/11218#issuecomment-6097838645)
accurately distinguishes 22 accepted and 19 rejected outcomes, although its
summary names only the two main A/B issues. The manifest includes the third.

Repeated runs are useful for replay fidelity and within-task variance. They do
not constitute 41 independent coding tasks. Nor do 41 verified traces mean 41
correct or merged patches. The failure rows are legitimate measured outcomes,
and should stay in the outcome inventory and experiment denominator. They may
enter training only when their issue groups and data-use permissions allow it.

The admitted traces also precede the current issue-run capture hook: all four
retained issue-run examples are unverifiable. A new prospective demonstration
must show the current production path capturing and replaying its own results,
not only the retrospective A/B pipeline.

**Required reporting:** count issued tasks, attempts, successful checks,
verified replays, reviewer acceptances, merges, and post-merge regressions
separately. Show unique issue and task-family counts beside trial counts.
Publish exclusions and failed captures before presenting success percentages.

## EVAL-02: Finder recall is a component result with a narrow denominator

**Priority: P1 for retrieval and product claims.**

The [file-finding study](../../inference/file-finding-bench.md) reports 0.952
recall of existing handwritten changed files in a top-400 shortlist over 100
historical issues, with 0.017 precision and all existing changed files present
for 80/100 cases. Its denominator is 702 existing handwritten files. The same
cases add 172 handwritten files and touch 33 derived files; reported recall over
all handwritten changed files is 0.764.

This is useful evidence that a cheap ranker can assemble a broad map. It is not
95.2% patch correctness, 95.2% complete task coverage, or 95.2% retrieval of every
semantically relevant file. The candidate union's reported 0.973 recall also
limits what reranking alone can recover. Existing files absent from the pool
need another retrieval stage, not a better ranker over the same pool.

The separate eight-newer-fix check contains 100 existing changed files and
reports 0.85 recall at 400. The study discusses misses and subsequent policy
changes. That cohort is useful development evidence once those results inform
the method; it is no longer untouched confirmation. N8 correctly excludes this
set and the ten Clef/Jev threshold-tuning issues from its new corpus.

The benchmark selects closed issues with recognizable fix references and
bounded change size. It excludes or underrepresents unresolved bugs, sweeping
refactors, work with no clean issue reference, newly created files, difficult
integration work, and tasks abandoned before a fix. The measured population is
therefore easier to reconstruct than the full backlog a customer may submit.

Reported warm query latency excludes some startup and network work. The study
separately documents embedding calls, issue fetching, historical indexing, and
cache construction. Keep those boundaries in the product's cost model. A
subsecond scoring stage is not necessarily a subsecond cold first run.

Finally, a compiler is a partial check on a retrieval miss. It can reveal missing
types or imports; it cannot guarantee discovery of omitted documentation,
runtime-only behavior, a missing assertion, or a required change that compiles
without being implemented. The proposed map-plus-verification loop needs its
own measured end-to-end coverage result.

**Required reporting:** file-level recall by existing/new/derived category,
issue-level complete coverage, precision and bytes delivered, candidate-pool
misses, cold and warm phase timing, and later accepted-patch outcomes. Cluster
uncertainty by issue, not by treating hundreds of correlated files as
independent observations.

## EVAL-03: Earlier studies expose both real gains and checker failures

**Priority: P1 for independent acceptance design.**

The following cohorts must stay separate. They differ in task selection,
executor, preparation, tools, acceptance, and accounting. They must not be
pooled into a headline improvement percentage.

| Cohort | Retained observation | Implication |
| --- | --- | --- |
| October 2 shipped-route shadow study, seven tasks and three trials per arm | Original Claude routed loop cost 1.68 times raw Claude and took 1.39 times as long at the same recorded pass count | A preparation recipe does not automatically improve the path people actually use |
| Same study, later lean Claude session arm | 21/21 recorded passes at 0.61 times raw Claude's cost; wall-time interval included equality | A constrained session package can provide a bounded cost win |
| October 3 model/briefing factorial, historical #9989 | Cheaper unbriefed executor won its registered cost comparison; one briefed patch missed a notification despite passing the frozen checker | Model choice and briefing effects must be separated; checker calibration on base/reference is necessary but insufficient |
| October 3 Jev native pilot, 12 sessions on two exposed tasks | Each arm passed 2/4 original checks; later executable diagnostics leave A 1/4, B 1/4, C 0/4 | Lower inference cost does not establish better accepted changes |
| Same pilot's separate Jev review | 116 clauses in 11 calls; zero `missing_handling` labels for ten reviewed patches with executable failures | A model judge cannot currently replace the independent behavior checks |
| October 3 Codex validation, six new tasks, four arms, three trials | Lean Claude 18/18, raw Claude 18/18, cost ratio 0.58; routed Codex 17/18 versus raw 18/18 and time ratio 2.29 | The Claude cost result transfers to this cohort; the Codex loop has a clear time problem in it |
| Current briefed-agent study | Early pilot and main-study machinery; documented results still in progress | Do not promote the pilot to the completed S2 milestone |

Sources: [shadow measurement](../../cost/2026-10-02-shadow-baseline-measurement.md),
[#10282 results comment](https://github.com/OpenAgentsInc/openagents/issues/10282#issuecomment-5966542904),
[native pilot](../2026-10-03-independent-efficiency/jev-native-pilot/README.md),
[Codex validation](../2026-10-03-independent-efficiency/codex-validation/results.md),
and [current A/B protocol](../../inference/briefed-agent-ab.md).

The original native pilot scores must remain unchanged; the added tests are
post hoc diagnostics, not a retroactively registered estimate of defect rates.
Their value is to demonstrate concrete holes in acceptance and define stronger
future checks. The reference patch passing and the original source failing do
not prove that a checker rejects every alternative bad patch.

The Codex validation's raw and routed medians are about 43 and 75 seconds, while
its reported paired aggregate time ratio is 2.29. These are different statistics,
not inconsistent calculations. Its cost estimates came from usage and list
prices, not reconciled subscription invoices. The six-task study cannot support
all-language, all-repository, or current-version generalization.

**Required acceptance design:** exercise independently authored boundary
mutations, preserve positive controls, test public callers as well as helpers,
and include cases where a patch disables or bypasses checks. A learned selector
may choose from a fixed test catalog, but the full independent catalog must
still measure how often that selector misses a defect.

## EVAL-04: The current A/B treatment mixes several mechanisms

**Priority: P1 before attributing a win to learning or System One.**

The [briefed-agent study](../../inference/briefed-agent-ab.md) compares a stock
Claude arm with top-file briefing, a custom prompt, changed tools, and a verify
interface. It also has Bash-enabled and oracle-file variants. A package-level
comparison is legitimate if the claim is about the entire package. It cannot
identify the incremental effect of learned retrieval, a decision model, a
smaller prompt, or verification.

Task eligibility depends on historical fixes with usable test changes. The
preparation notes record 21 of 29 usable tasks after qualification. Hidden-test
overlay behavior and supplied interface signatures changed during development;
the study retains earlier results and describes regrading. These are sensible
pilot repairs, but the final treatment, test overlay, timeout, and acceptance
rule must be frozen before a fresh confirmation cohort begins.

The remote grader in
[`scripts/bench/briefed-ab/remote/eval.sh`](../../../scripts/bench/briefed-ab/remote/eval.sh)
applies the patch, checks the target package, overlays reference tests, and runs
named tests. It records compilation separately from test success. It does not
establish whole-repository correctness, nor is that an appropriate default gate
for every small change. The correct scope is the behavior promised by the task
and its relevant consumers, with explicit untested boundaries.

Warm shared targets and trial order affect time. Usage-limit trials that are
discarded and rerun affect reliability and cost denominators. Keep a conditional
solver-success measure if useful, but also report the outcome for every task
submitted to the system, including provisioning, throttling, timeout, and lost
worker states. Unknown cost must remain unknown; it must not be silently added
as zero.

**Required experiment:** compare the frozen product package with a current bare
executor using the same task, model identity, effort, host class, acceptance,
and resource budget. Separately compare deterministic retrieval with the learned
finder while holding the rest fixed. Add the semantic decision step only in a
third arm. Treat oracle files as a diagnostic upper bound, never as an available
customer product.

## EVAL-05: Neither repetition nor a new checkpoint proves recursion

**Priority: P1. The central missing product result.**

The product's S3 milestone requires at least two retrain-and-remeasure cycles
whose gains survive held-out evaluation. The current evidence establishes
historical supervised fitting and some adaptive feedback, but no such sequence
of independently confirmed deployed improvements.

There are several weaker results that must not substitute for it:

- Training loss decreases while accepted patch quality stays unchanged.
- The same benchmark improves after inspecting its misses.
- A second model benefits from a better prompt, newer provider model, or larger
  context window, while the gain is credited to repository learning.
- More attempts solve more tasks at an unreported additional cost.
- A model agrees more often with the teacher that generated its labels.
- Retrieval improves while all misses still require expensive manual rescue.
- A verifier becomes more permissive and therefore accepts more candidates.
- A model memorizes an issue whose earlier attempts appeared in calibration.

A self-improvement result needs a causal chain: new eligible outcomes enter a
pinned corpus; a specific candidate changes; an untouched evaluator measures
the candidate against its predecessor; the candidate earns promotion; the next
cycle starts from that exact promoted identity. Gains should appear in a
business endpoint as well as a component metric, or be labeled as component
improvements.

The counterfactual is essential. Compare a policy allowed to learn with a frozen
policy that receives the same new repository state and can still use ordinary
search. Otherwise normal code evolution, cache warming, and better historical
coverage can look like learned improvement.

## EVAL-06: The next evidence set needs task-level power and full accounting

**Priority: P1 for S2, S3, and customer economics.**

The product's 20-issue minimum and three trials per issue are a useful operational
floor, not an automatic statistical guarantee. Repeated attempts on the same
issue are correlated. The N8 locked partition has 343 rows but only 24 issue
groups. Report uncertainty and practical effect sizes using those groups.

Predeclare one primary comparison, the direction and size of the required win,
the quality noninferiority margin, and the allowed cost of extra abstentions or
reviews. “No statistically significant loss” is not evidence of equivalence
when a sample is small. The proposed two-standard-error rule also needs a
defined estimator, unit of independence, and measured variance; it is not a
universal pass formula for repeated selection over many candidates.

Include these costs in a task ledger: issue acquisition, initial indexing,
incremental indexing, embeddings, preparation, model generation, decision calls,
retries, queue waits, builds, verification, independent replay, human review,
failed captures, storage, training, calibration, and recovery. Separate actual
charges from usage-price estimates, subscription allocation, prepaid credits,
and unknown expenses. Report initial onboarding cost and its amortization
assumption rather than hiding it in warm-run comparisons.

The primary denominator should be an independently accepted task, not an API
request, an attempted patch, or a self-reported completion. Keep cost per attempt
and latency components as diagnostics. Report both median and tail time, and
time to recovery after failure. A cheaper median that strands a minority of
tasks may be worse for the customer.

## Proposed experiments and release evidence

These are proposed acceptance experiments, not completed measurements. Run them
in this order so paid execution follows evidence plumbing that can preserve and
explain its outcomes.

### Experiment 1: Close the evidence path without model calls

Use a temporary repository and synthetic run folders to exercise capture,
replay, admission, partition assignment, feedback, candidate recording, and
activation refusal. Include a passing patch, reproducibly failing patch,
missing diff, mismatched result tree, changed check command, missing test,
forged receipt, and edited teacher field. Kill and restart writers during
capture and trial append. Two simultaneous final trial submissions must not
exceed the budget.

Use the actual #10074/#10228/#10273 partition conflict as a fixture. Joining
trace training rows to the N8 corpus must fail or preserve the reserved role.
Verify that removing a teacher-bearing corpus removes content from the active
training view and that future recipes cannot silently reuse it.

**Deliverable:** one offline report identifying every accepted and refused
transition, the verifier and data identities, and the specific guard that made
the decision. This validates plumbing; it is not learning evidence.

### Experiment 2: Measure the present finder prospectively

Freeze the model, embedding backend, candidate stages, feedback policy, and
thresholds. Select new issues before their fixes are known. Retain the original
issue text and all exclusions. Include fixes with new files, cross-crate callers,
fixtures, documentation, build changes, and runtime-only failures.

Compare the frozen finder, lexical/history retrieval, and an otherwise identical
finder with verified feedback. Use both retrieval targets: historical eventual
edit membership and independently reviewed relevance. Record what the agent
actually receives after truncation, not just what the candidate index contains.

**Deliverable:** complete issue-group coverage and calibration tables, cold and
warm cost, missed-boundary examples, and an explicit statement of which scores
are ranking values rather than calibrated probabilities.

### Experiment 3: Confirm the complete coding package

Use the product's S2 floor of at least 20 distinct issues and three attempts per
arm, subject to a prospective power calculation. Randomize matched blocks and
retain all attempts. The named S2 comparison is bare Claude Code versus the
proposed package. Other executors can have separately declared comparisons.
A smaller factorial or component panel separates deterministic
briefing, learned retrieval, and semantic decisions.

Freeze acceptance before execution. Validate it against the base, the historical
reference when one exists, and plausible incorrect alternatives. Keep the
acceptance checkout and check definitions outside the candidate's writable
workspace. Review a blinded sample of accepted and rejected patches, and carry
later regressions into a separate follow-up endpoint.

**Deliverable:** quality and cost per independently accepted task, task-clustered
intervals, all-in cost boundaries, median and tail time, and failure causes.
Pass the specified 30% cost improvement only with equal or better success and
no worse median time under the predeclared analysis. A proposed noninferiority
margin that allows a quality loss is a spec amendment, not an automatic S2 pass.
Do not substitute fewer tokens for the required result.

### Experiment 4: Prove two successive learning cycles

Start with frozen baseline B0 and a control that never trains. Collect eligible
outcomes under explicit data policy. Train C1 only from the training partition,
calibrate on separate groups, select on development data, and spend a fresh
confirmation cohort once. Promote only if the frozen gate passes.

The spec currently calls for a fixed held-out issue set across two cycles.
Gym's existing admission evaluator instead permits one original locked read
for one plan and candidate. This proposal resolves that conflict explicitly:
use a fixed exposed set for a development trend and fresh protected groups for
each promotion. Reusing one confirmation set would require a separately
reviewed sequential evaluation protocol, not merely resetting its read ledger.

After C1 is deployed, collect a second prospective cohort and produce C2. Reserve
new confirmation groups before development decisions. Compare C2 with C1 and
with the nonlearning control at the same repository and provider revisions.
Record every rejected candidate and full search budget; restarting the trial
counter must not erase failed experiments.

**Deliverable:** two complete links from outcomes to candidate to admission to
activation, with gains meeting the spec's two-standard-error rule under a
declared estimator and no quality, calibration, or transfer regression. Gym's
`ab::Rule::effect_size` uses a configurable multiple of measured suite-block
spread, scaled by the numbers of blocks; two is not hardcoded into every gate.
Report issue-group uncertainty as well as seed-block variation. A failed candidate that leaves B0 or C1
active is a correct system outcome. Do not require every cycle to produce a win.

### Experiment 5: Establish local model and serving readiness

For replacing hosted decisions, first choose one bounded decision with measured
headroom. Compare exact Rust rules, the current ranker, the existing decision
model, and a small candidate on the same frozen state and question contract.
Train real weights, export them, and verify that the serving endpoint returns
the evaluated artifact identity. Measure actual hardware execution, queueing,
memory, cold start, abstentions, calibration, and cost under load.

**Deliverable:** a model-specific useful checkpoint and served-parity evidence.
Psionic contracts, deterministic demo weights, missing-artifact test skips, and
synthetic research scores cannot satisfy this step. A result that exact Rust
rules are cheaper and sufficient should stop that training attempt.

### Experiment 6: Test transfer and customer isolation

Repeat the package comparison on a second repository with a different language
or structure. Account for onboarding, fresh indexing, missing history, and the
absence of OpenAgents-specific issue conventions. Evaluate a global baseline,
tenant-only adaptation, and unchanged control. Do not expose the confirmation
tasks through shared memory, feedback, or another tenant's training.

**Deliverable:** a scoped external-repository result, explicit training consent,
an exercised deletion and rollback path, and a price estimate based on actual
support and review effort. This is evidence toward S5/S6, not automatic S7
self-service readiness.

## Checks performed for this audit

The following command ran in the audit worktree with temporary files redirected
to the repository's designated scratch area:

```sh
TMPDIR=/Users/christopherdavid/.openagents/scratch/codex-01a12620-209f-7ec3-9846-7e94fc1b965b \
  python3 -m unittest discover -s scripts/bench/traces -p test_traces.py
```

Result: **7 tests passed in 2.185 seconds**. They exercise honest replay and
admission, tampered diff and tree, a false recorded check result, summary/diff
disagreement, missing diffs, and immutable first capture. They execute tiny
temporary Git/Python fixtures; they do not run Cargo or a model.

The audit also recomputed:

- The trace manifest's canonical rows digest:
  `sha256:2b074fb2ccce61450443dd1d46db34675863128263f118386e831f1ef5588e36`.
- The N8 `items.tsv.gz` and `issues.tsv` hashes, both matching their committed
  manifest values.
- Every trace verdict, check-success count, distinct-issue count, and the join
  of verified trace groups against N8 partitions described above.

The raw trace blobs, remote build-host receipts, full corpus text, model bundles,
and paid experiment executions were not independently replayed in this audit.
Matching digests establishes consistency of retained artifacts, not authenticity
of an external measurement or completeness of a behavioral checker. Source
inspection supplies the implementation findings; historical studies supply the
measured performance claims, each with its original scope and limitations.
