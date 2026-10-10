# Learning and evaluation

The repository has a real learned file ranker, reproducible corpus construction,
useful replay checks, and a substantial candidate-admission framework. It does
not yet demonstrate a closed, independently measured improvement cycle in which
its own new coding outcomes produce a better deployed policy. The largest gap
is the connection between those components. The ranker's retrain command,
replayed traces, partitioned corpus, Gym admission plan, and production feedback
do not currently form one enforced path.

This chapter audits source at `07805e6a7c3513a057d226b488cb2d40fd974a64`.
It distinguishes current source findings from measurements retained in earlier
reports. No new model training, paid inference, or hardware qualification ran.
[Evidence and experiments](04-evidence-and-experiments.md) describes the local
checks and the experiments needed to support the product claims.

## What exists

| Component | What the evidence establishes | What it does not establish |
| --- | --- | --- |
| File finder | A two-stage gradient-boosted model trained on historical issue-to-file outcomes; historical parent-tree replay and feature ablations | Current production-model performance on an untouched cohort, calibrated relevance probabilities, or a recursive gain |
| Briefed agent | A runnable preparation, execution, and verification path; early comparative results | Equal quality, lower total cost, and equal latency across the product's target workload |
| Trace capture | Diff-derived changed-file labels and clean-checkout replay; seven offline tests pass | Complete behavioral acceptance, independent checker quality, or a mandatory gate on every feedback source |
| File-relevance corpus | 10,234 items from 820 issue groups, time partitions, digested manifests, source attribution, model answers kept separately | A gold relevance dataset, production-distribution calibration, or integration with all incoming traces |
| Tenant training book | Corpus validation, recipe and candidate identities, trials, retention records | An optimizer, enforced training budgets, authenticated data grants, or an automatic train-to-serve loop |
| Gym admission | Pinned candidates, development selection, one-use locked confirmation, calibration and transfer guards | Evidence that the file ranker uses this path or that a particular model has passed it |
| Decision models and Psionic | Serving implementations, model-specific conformance tooling, calibration machinery, substantial research contracts | A useful newly trained repository decision model on Pylons or a general self-improving research system |

The distinction matters commercially. An effective hand-engineered workflow
can save money before any learning happens. A model can learn historical file
co-change without improving accepted patches. A candidate can be sealed without
being served. None of those are failures of the individual component; they are
different claims with different evidence.

## LEARN-01: Ranker publication bypasses the promised admission gate

**Priority: P1. Blocks automatic learning and the S3 claim.**

[`scripts/filefind/retrain.sh`](../../../scripts/filefind/retrain.sh) first
trains a held-out model into its work directory, then invokes
`file-finding-bench.py train --all --reuse-stage2` and writes directly to
`scripts/filefind/model.json`. There is no intervening evaluation, calibration
comparison, locked-read record, improvement test, or rollback decision.
[`cmd_train`](../../../scripts/bench/file-finding-bench.py) explicitly includes
the evaluation cases when `--all` is set. The checked-in model metadata says
1,692 cases, issues `#1..#11208`.

Training a final model on all historical data can be a valid development choice.
It invalidates treating the old evaluation cohort as an untouched test of that
final model. The product's promised locked-set improvement of at least two
standard errors and no calibration regression is not enforced here.

There is also an easy reporting mistake: the retrain script's comment says
running `eval` on the same work directory reports held-out numbers, but the
benchmark's default `--model` is the production model. A caller must explicitly
select the work directory's held-out model. Otherwise the default evaluates a
model fitted on those evaluation cases. This audit does not infer that the
published historical result used the wrong model; it identifies an unsafe
default for reproductions and later cycles.

The model card contains counts and an issue-number range, not the full corpus,
feature-extractor, split, embedding, training-code, and calibration identities.
An issue-number range is not a training manifest. The model file is written with
a normal JSON write, so a local retrain is also not an atomic activation protocol.

**Acceptance:** train to an immutable candidate location; evaluate a pinned
baseline and candidate under a frozen plan; require locked confirmation and
calibration guards; then atomically activate the approved digest. A deliberately
worse candidate must leave the active model unchanged. Evaluation must reject
an overlapping training manifest by default. A locally permitted override must
label the result as development data.

## LEARN-02: Replay admission and the new corpus disagree on partitions

**Priority: P1. Blocks joining the learning feeds.**

[`traces.py::corpus_items`](../../../scripts/bench/traces/traces.py) still
contains `TODO(#11215)` and assigns every admitted item to `training`.
Issue [#11215](https://github.com/OpenAgentsInc/openagents/issues/11215) has
landed with the four-partition corpus. The two paths have not been joined.

This is a concrete overlap, not merely a possible future mistake. Recomputing
the committed [trace manifest](../../coder/traces/2026-10-10-manifest.json)
and joining it to [the corpus issue table](../../../crates/gym/suites/file-relevance-v1/issues.tsv)
shows:

| Issue | Verified traces | N8 corpus partition | Trace item partition |
| --- | ---: | --- | --- |
| #10074 | 18 | Calibration | Training |
| #10228 | 22 | Calibration | Training |
| #10273 | 1 | Development | Training |

All 41 verified traces belong to issue groups already reserved for calibration
or development in this corpus. Combining these rows without remapping or
exclusion violates the intended group separation. This audit found no evidence
that such a combined corpus has already trained a deployed checkpoint.

`tenancy::training::Corpus::validate` would refuse a group spanning partitions
inside one registered corpus. That protection is useful, but it cannot discover
that a separately trained finder, trace store, calibration map, or memory
database has already consumed the same issue.

**Acceptance:** maintain a cross-artifact exposure ledger keyed by issue group,
repository, source revision, and derived examples. Ingest these exact 41 traces
as a regression case: preserve the reserved roles or refuse the join. Never
change a held-out group's role silently. Retire an exposed confirmation cohort
and allocate a fresh one before another claimed recursive improvement.

## LEARN-03: Changed-file labels answer a different question from relevance

**Priority: P1. Blocks claims about calibrated file relevance.**

[`file-relevance-corpus.py`](../../../scripts/bench/file-relevance-corpus.py)
asks, “Is this file relevant to solving the issue?” Its label is true if a
commit mentioning that issue changed the file, and false if no such commit
changed it. A changed file is an observable outcome. It is not a complete
definition of relevance.

A caller can be relevant to understanding an interface while needing no edit.
A useful contract, unchanged test, migration note, or implementation example
can become a negative. Conversely, a formatting change, incidental cleanup, or
overbroad historical patch becomes a positive. A merged patch can be incomplete
or later reverted. `measurement` correctly describes where the label came from;
it does not establish that the label answers the natural-language question.

[`file-finding-dataset.py`](../../../scripts/bench/file-finding-dataset.py)
links issues using subject references, not an acceptance or regression record.
In multi-commit mode it unions files from up to six referenced commits. Shared
commit references can therefore assign one patch's changes to multiple issue
groups. A group key of `issue-N` alone does not cover shared fixes, duplicate
issues, follow-up repairs, or common task templates.

The candidate distribution also changes the problem. Each issue supplies at most
eight positive files, three unmodified siblings, two same-crate files, one
recent file, and one random file. The manifest has 4,649 positives among 10,234
items, about 45.4%; the locked subset is 176/343, about 51.3%. The finder's
reported top-400 precision is about 1.7% on its historical cohort. These are
different sampling designs and denominators. A threshold calibrated on the
balanced corpus cannot be assumed to retain its meaning in the deployed
candidate stream.

**Acceptance:** either rename the target to predict historical edit membership
or annotate relevance separately. Keep edit membership as a useful auxiliary
signal. Evaluate a natural candidate stream, retain its sampling probabilities,
and report issue-level retrieval coverage, file-level precision, calibration,
and missed required boundaries. Group shared commits and near-duplicate task
families together. Include difficult negatives that were inspected but rejected
by an independent reviewer, with uncertainty preserved.

## LEARN-04: Unreplayed observations still influence production retrieval

**Priority: P1. Blocks the “only verified outcomes teach the system” claim.**

[`filefind.py::cmd_feedback`](../../../scripts/filefind/filefind.py) accepts
three kinds of input:

- Raw issue-run summaries and their local patches. Changed files require
  recorded successful checks and no recorded error, but not an independent replay.
- Raw A/B results. Changed-file rows use `tests_pass` and `files_changed` from
  the result record. Files opened outside the briefing are also ingested.
- The optional `--traces` feed, which checks a replay verdict before reading
  the trace.

Outside-briefing reads are accepted even from unsuccessful runs. They receive
half the changed-file weight. `cmd_query` uses same-issue feedback to raise file
scores, and similar-issue lookup also consumes it. This is an actual adaptive
retrieval path, even when no model weights change.

These observations can be useful. A failed run may discover an essential file.
But they must be labeled as observations rather than verified successful
training outcomes. In contrast,
[`file-finding-bench.py::run_case`](../../../scripts/bench/file-finding-bench.py)
clears feedback to avoid future leakage. The historical recall result therefore
does not evaluate the full adaptive production policy.

**Acceptance:** separate observed reads, locally checked changes, replayed
outcomes, and human acceptance. Keep their authority and weighting explicit.
Evaluate feedback on later untouched issues, including poisoned and irrelevant
reads, repeated failures, and reverted fixes. If verified feedback is the
product contract, require its receipt on every path that affects the policy.

## LEARN-05: Historical replay pins code better than it pins information

**Priority: P1 for benchmark claims; P2 for local iteration.**

The finder makes a serious attempt to avoid hindsight: historical parent trees,
co-change cutoffs, issue exclusion, and out-of-fold stage-two features are all
present. These are strengths. They do not fully reproduce what was knowable
when an issue was opened.

The dataset reads current issue titles and bodies from a GitHub export. Bodies
can be edited after the fix to include exact implementation details. The N8
manifest binds the exported text's digest, which establishes repeatability of
that snapshot, not its availability before the fix. `materialize` correctly
fails when current issue text no longer matches; it cannot reconstruct the old
text from a digest. A retained authorized issue snapshot is needed for durable
reproduction after edits, deletion, or repository migration.

The feature cache is keyed by issue number. `cmd_features` reuses a cached row
when that issue already exists, without checking the case's body, parent,
candidate-policy, embedding, or feature-code identity. Reusing a work directory
after any of those change can mix incompatible observations. The production
model metadata is insufficient to detect that mixture.

The corpus drops cross-partition text pairs at token-set Jaccard 0.8. That
detects a particular kind of lexical duplication, not all semantic leakage.
Repeated incident families, renamed files, shared fixes, or a test and its
implementation can carry the same answer without high full-state overlap.

**Acceptance:** bind every cache row to the complete case and extractor identity;
invalidate on drift. Preserve issue snapshots with acquisition times and edit
history when available. Label historical current-body studies accordingly and
use prospective issues for final confirmation. Add group-level checks for
shared commits, repairs, reverts, generated variants, and task families.

## LEARN-06: Calibration is implemented, but not connected to every policy

**Priority: P1 before automatic decisions based on confidence.**

[`gym::calibrate`](../../../crates/gym/src/calibrate.rs) correctly distinguishes
an estimator's selected answer from the calibrated probability that it is
correct. Calibration can lower confidence without changing the selected option.
Its records bind the door and measurement identities. This is a stronger
foundation than treating a scorer's output as an intrinsic probability.

The finder exposes its boosted score as `confidence` and may raise that value
using feedback weights. Its fitting code uses positive weighting as well as
negative sampling. Neither that score nor a feedback-adjusted value is shown to
be a calibrated probability of relevance in the current production stream.

Historical calibration work is a warning about transfer. The
[October 2 shadow study](../../cost/2026-10-02-shadow-baseline-measurement.md)
records 36 of 42 small-task requests classified as hard. The revised class
policy matched 42 measured small-task requests and six authored controls, but
had no measured hard-task positive examples. That is a useful development
calibration, not validation of escalation on real hard work.

The [October 3 calibration audit](../../research/typesafe/2026-10-03-calibration.md)
also distinguished probability calibration from choosing an action threshold.
Later issues add telemetry and policy changes; its old count of hand-tuned
thresholds must not be treated as a current count. The lasting requirement is a
join from each decision's model, question, state shape, raw probability,
calibration map, threshold, and action to a later independent outcome.
Current [TypeSafe confidence guidance](https://docs.typesafe.ai/confidence)
likewise says thresholds depend on the domain and model performance and need
testing against the application's own data; its examples are not validation of
this repository's policy.

**Acceptance:** define the cost of false inclusion, missed files, unnecessary
escalation, and false acceptance separately. Fit maps on representative
calibration data and evaluate them on later groups. Report ECE, Brier, NLL,
confident errors, abstention, coverage, and downstream accepted-task cost.
Require a new check when the model, question, state truncation, candidate pool,
or feedback policy changes.

## LEARN-07: The training book records policy that it does not fully enforce

**Priority: P1 before unattended or customer training.**

[`tenancy::training`](../../../crates/tenancy/src/training.rs) provides useful
structural checks: provenance fields must be present, model-sourced labels need
confirmation, groups cannot cross partitions, duplicates are refused, trials
name frozen recipes, and candidate artifacts must match a kept trial.

Several stronger prose claims exceed those checks:

- The [tenant-training guide](../../decision-models/service/tenant-training.md)
  says headroom is assessed before a recipe may freeze. `Book::freeze_recipe`
  loads and validates the recipe, then writes it. It does not load a corpus or
  require a `train` headroom verdict. The recipe has no corpus or headroom digest.
- `record_trial` enforces allowed seeds, trial count, and nonempty kept
  artifacts. It does not compare reported compute against recipe budgets or
  recompute whether the recorded outcome earned `Kept`. These remain operator
  assertions until an execution controller and admission evidence check them.
- The trial cap is a read-count followed by an append without a surrounding
  lock. Parallel writers can both observe room before appending. The reader
  silently drops malformed JSON lines, which can also make the apparent count
  smaller than the attempted history.
- `Book::candidate` promises a reverified signature in its comment but only
  deserializes. `CandidateDoc::verify` exists separately; callers must invoke it
  before trusting the identity. `tenant-train inspect` uses the plain reader.
- Evidence class is an optional string. Its digest binds the value, but the
  type does not constrain the vocabulary or prove that `measured` came from an
  actual measurement. Nonempty permission prose is not an authenticated grant.

These are boundaries of an operator-controlled book, not evidence of a remote
tenant attack. They become product defects if the book is presented as an
unattended enforcement service.

**Acceptance:** bind recipes to corpus and headroom identities; enforce budgets
at the executor; atomically account every trial, including interrupted and
malformed records; verify candidate identities on reads used for decisions;
and make evidence classes typed. Preserve the useful separation between
recording a candidate and authorizing it to serve.

## LEARN-08: Corpus deletion leaves the new teacher field intact

**Priority: P1 before ingesting customer teacher outputs.**

`CorpusItem` now carries `teacher: Option<Value>`. `Book::delete_corpus` clears
the state, question, label, label rule, and annotations, but leaves `teacher`
unchanged. It also preserves provenance strings. A teacher object can contain
arbitrary JSON, including copied source or customer text. The current N8 corpus
records no teacher answers, so this is a source-confirmed retention hole rather
than evidence of an existing disclosure. **DATA-03** in
[Customer data and authority](06-customer-data-and-authority.md) owns the full
finding and deletion acceptance criteria.

For learning, deletion must also remove future training eligibility and account
for derived stores. Excluding future examples does not remove their influence
from an already trained model. Each recipe needs a reproducible record of which
authorized data was eligible when it ran.

## LEARN-09: A reproducible trace is not a complete acceptance oracle

**Priority: P1 for automatic landing and training labels.**

`traces.py::replay` checks diff bytes, the resulting tree, changed-file paths,
and repeatability of recorded check results. It deliberately verifies failing
runs too. That is correct for an outcome dataset: failures are informative.
The term `exact_replay` establishes repeatability under the captured checks,
not that the patch meets every requirement.

For A/B traces, `accepted` uses `tests_pass`. It does not include the separate
teacher judge score described by the A/B protocol. For command traces it uses
the conjunction of nonempty recorded checks. The outcome label `accepted`
therefore means replayed checks passed, not reviewer acceptance, merge,
production success, or absence of regressions.

The trace record has no separate immutable digest of the entire check policy,
grader implementation, environment, toolchain, and hidden-test-selection rule.
The remote path invokes the host's installed `~/ab/bin/eval.sh`; the local path
duplicates that grader's logic. `cmd_admit` trusts the latest serialized receipt
whose verdict is `verified`, rather than rechecking its digest and binding to
the full trace. Those are local trust assumptions, not remote attestation.

The source retains teacher scores separately, which is good. Keep it that way:
an LLM judge can diagnose or prioritize tests but cannot supply the supposedly
independent ground truth for its own descendants. Tests added by the coding
agent are also insufficient unless their adequacy is checked independently.

**Acceptance:** distinguish `checks_passed`, replay verified, reviewer accepted,
merged, and post-merge healthy. Bind the verifier identity and all check inputs
to each receipt. Exercise wrong-check, missing-check, zero-tests, modified-test,
forged-receipt, and grader-drift cases. Calibrate acceptance with known broken
patches and later boundary mutations, preserving the original result when a
new diagnostic exposes an old miss.

## LEARN-10: Research infrastructure is not evidence of useful learning

**Priority: P1 for product claims; P2 for implementation sequencing.**

The [training-system audit](../2026-10-10-training-system-audit/README.md)
provides detailed source-based limits on Psionic, Lev, and distributed training.
Its earlier snapshot must remain distinct from this audit's base. Two important
claims were rechecked in current source:

- [`ResearchRunner`](../../../crates/psionic/crates/psionic-research/src/runner.rs)
  dispatches two executor families to their specialized paths. Other families
  call `synthesize_scores` and `synthesize_metrics`; scheduler throughput and
  training-policy scores are computed from configuration arithmetic. They can
  produce `Succeeded` receipts without measuring those policies on a workload.
- [`psionic-serve/Cargo.toml`](../../../crates/psionic/crates/psionic-serve/Cargo.toml)
  still has direct, nonoptional dependencies on train, eval, and research.
  Serving, research contracts, and trainability are separate capabilities even
  when they link into one binary.

[`training/tenant-demo/make_artifacts.py`](../../../training/tenant-demo/make_artifacts.py)
plainly identifies itself as a demonstration driver and emits deterministic
initialization-scale tensor bytes. It is valuable contract rehearsal, not a
trained adapter result. The [Lev adapter guide](../../../training/lev-adapter/README.md)
also explicitly preserves an earlier leakage finding: old adapters trained on
98 examples include 20 later reserved calibration items, while the next
conversion excludes them. Relabeling old data as locked cannot undo exposure.

Kev and Laya conformance tests can return successfully when local weights are
absent. That supports ordinary development without model bundles, but a green
test command alone cannot establish current weight-backed parity. Record tested,
skipped, and unavailable hardware/model combinations separately.

**Acceptance:** retain explicit measured, authored, sample, and contract-only
classes through publication. For a new decision model, require a real training
run, a changed weight digest, reproducible export, served-artifact parity,
calibration, independent held-out improvement, and a deployment receipt. Until
then, prefer the existing ranker and constrained evaluation work over counting
research contracts as recursive capability.

## What should remain fixed while the system learns

Learn file order, context selection, routing, retry choices, or candidate
generation first. Keep execution authority, budget enforcement, corpus
permissions, verifier inputs, and activation rules under separately reviewed
code. If the system proposes a change to its own checker, data-selection rule,
or promotion policy, evaluate that as a new policy with an external reference
and an untouched cohort. It must not approve the change using the very rule it
is replacing.

The existing Gym admission design is a useful basis: a frozen plan, declared
identity changes, one winning metric, ECE/Brier/NLL guards, transfer checks,
measured variance, and one original locked read. Connecting the file finder and
trace feeds to that discipline is a smaller and more credible next milestone
than claiming that the entire codebase already improves itself.
