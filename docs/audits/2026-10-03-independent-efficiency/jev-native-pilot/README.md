# Jev source selection in native Claude sessions

October 3, 2026. Tracking:
[#10356](https://github.com/OpenAgentsInc/openagents/issues/10356).

**The Jev workflow had lower measured inference cost and elapsed time, but this
round does not establish better accepted coding results.** Across 12 fresh
native Claude sessions, the Jev workflow costs **35.2% less** and takes
**24.4% less time** than bare Claude. Its incremental gain over the same lean
workflow with deterministic preparation is **8.8% on cost and 9.3% on time**.
Every arm passes only **2/4** original acceptances. The registered overall
win criteria fail. Additional executable trace checks then reject four of the
six originally accepted patches, including **both Jev patches**. The savings
do not establish a quality win.

These are two exposed historical development tasks, repeated twice per arm.
All attempts and failures remain in the comparison. The
[recomputed report](report.json), [frozen protocol](protocol.md), and
[native evidence archive](native-evidence.tgz) retain the complete panel.
This is separate from the earlier unsealed 48-session proposal, which still
has no scored attempts.

## What ran

| Arm | Native configuration | Original acceptances | Mean cost per attempt | Mean time through cleanup | Cost per original accepted patch |
| --- | --- | ---: | ---: | ---: | ---: |
| A: bare | Default Claude prompt and tools | 2/4 | $0.5205 | 243.4 s | $1.0410 |
| B: deterministic | Lean prompt, six tools, deterministic 16 KiB pack | 2/4 | $0.3701 | 202.9 s | $0.7401 |
| C: Jev | Same lean workflow and renderer; one Jev pointer-selection call | 2/4 | $0.3375 | 184.1 s | $0.6751 |

All sessions request Sonnet 5.5 at medium effort; every recorded generation
request serves Sonnet 5.5. Served effort is not independently verified. The native CLI,
historical source, task, instructions, required readings, Linux toolchain,
Cargo seeds, allowed paths, and independent checks are bound before execution.
No patch is repaired after checking. All 12 sessions complete, all costs are
known, and all execution and scratch cleanup are confirmed.

B/A is already **28.9% cheaper and 16.6% faster**. C/A changes the prompt,
tool menu, and supplied source together; it cannot identify which part of
that bundle caused the gain. C/B holds those settings fixed and changes
source selection. The six tools narrow the model-facing API. Bash retains
the same operating-system authority as the bare arm inside the common sandbox.

All 12 patches pass scope, formatting, and ordinary package tests. The six
trace-task patches pass the original independent checks; the six SDK patches
fail. The [checker coverage appendix](checker-coverage.md) states what those
finite checks exercise and what they leave untested.

| Task | A mean cost / time | B mean cost / time | C mean cost / time | Original passes per arm |
| --- | --- | --- | --- | --- |
| Trace recovery and evidence integrity, #9425 | $0.3855 / 235.2 s | $0.3358 / 192.0 s | $0.2561 / 176.7 s | 2/2 |
| Typed SDK validation, #9424 A12 | $0.6556 / 251.5 s | $0.4043 / 213.8 s | $0.4190 / 191.4 s | 0/2 |

C is cheaper and faster in all four matched comparisons against A and three
of four against B. Its second SDK attempt is **11.7% more expensive and 6.9%
slower than B**. Across both SDK repetitions, C costs 3.6% more than B. The
whole-panel averages miss both registered 10% C/B improvements, as well as
the requirement that C pass all four attempts. This unfavorable repetition
is part of the result.

## Additional checks expose a quality problem

Source reviewers receive all 12 candidate patches without arm labels or
execution outcomes. Their [retained findings](source-review/mapping.json)
identify malformed-log handling and missing SDK boundary validation that
ordinary tests do not establish. These are static findings; the following
trace cases are also executed after the native panel closes.

The [post hoc diagnostic](posthoc-beta/README.md) checks a clean trace, a step
before its session header, and a form-feed prefix on an interior JSON record.
The historical reference passes all three; the unmodified source fails the
step-order case. All six candidate patches still pass ordinary package tests.

| Trace candidate | Original checks | Added diagnostic | Demonstrated failure |
| --- | --- | --- | --- |
| A, repetition 1 | Pass | Fail | Trims invalid JSON whitespace into valid evidence |
| A, repetition 2 | Pass | Pass | None in these added cases |
| B, repetition 1 | Pass | Pass | None in these added cases |
| B, repetition 2 | Pass | Fail | Trims invalid JSON whitespace into valid evidence |
| C, repetition 1 | Pass | Fail | Accepts a step before its session header |
| C, repetition 2 | Pass | Fail | Trims invalid JSON whitespace into valid evidence |

Only A and B each retain one trace patch that passes both sets of checks; C
retains none. With the six original SDK failures, the conjunction is **1/4,
1/4, and 0/4 for A/B/C**. Original registered scores remain 2/4 for every arm.
This is a post hoc diagnostic prompted by source review, not a prospective
estimate of defect rates. Passing these finite checks also does not prove
complete correctness.

The result narrows the next experiment: improve delivered implementation
context and independent boundary checks before spending on a larger native
panel. Faster, cheaper patches are not sufficient when required behavior is
missing. Neither fewer model requests nor a favorable Jev judgment supplies
the missing evidence.

## Where the savings appear

The [mechanism analysis](mechanisms.md) separates model requests, visible
tools, token use, preparation, and final checks. Actual generation requests
total **50/45/38 for A/B/C**; two additional broker requests only count tokens.
Every generation uses Sonnet. No visible child agent or delegation tool runs.

C saves 19.7 seconds in the mean native phase versus B while adding 1.1
seconds of preparation. Final acceptance time differs by about 0.2 seconds.
The four preparation decisions together cost **$0.004973052**. Fewer cache
writes, cache reads, and output tokens account for the remaining cost change.
Those measurements describe the observed trajectories; fewer requests alone
do not prove less wasted work or a more complete implementation.

The initial provider context averages about 36.5k tokens for A and 24.8k/24.6k
for B/C, despite A receiving a shorter user prompt. That is consistent with
the lean system and tool configuration reducing overhead. B/C start with
similar context sizes. Their difference develops during execution. Identical
SDK Jev briefs produce **8 versus 17 generation requests** in the two runs,
so a small task sample and service/tool variation matter.

## What the briefings actually delivered

The new selector uses Tree-sitter declaration spans, pinned signatures, public
operation admission, exact task clauses, and one batched Choice request. Code
materializes the selected pointers under the same 16 KiB limit as B. Full
required instructions and contracts are separately supplied to every arm.

Compare the readable
[trace deterministic brief](briefings/alternative-beta-deterministic-1.md)
with the [trace Jev brief](briefings/alternative-beta-jev-1.md), and the
[SDK deterministic brief](briefings/alternative-gamma-deterministic-1.md)
with the [SDK Jev brief](briefings/alternative-gamma-jev-1.md).

- For the trace task, Jev adds complete reader and consumer-grading functions
  that B's pack drops. Both still omit the writer's `finish` method.
- For the SDK task, Jev adds complete `SystemOneResponse::decode` and
  `decode_answer` bodies. B delivers only 483 implementation-source bytes.
  C improves that to 3,291, but still spends 7,430 source bytes on tests and
  omits the async and blocking typed callers.
- Those callers exist in the metadata catalog. Selection and packing fail
  to deliver them. A better catalog does not by itself ensure that the final
  brief contains every relevant boundary.

This motivates caller context and explicit implementation budgets. It does
not prove that the omitted callers caused a specific failed patch. The agents
could read the repository, and every arm received the full public requirement
to validate answers at the typed request boundary.

## Jev review after coding

A separate [advisory experiment](clause-review/README.md) uses complete changed
implementation files, full pinned contracts, and available public entry-point
files. It asks one Choice per exact public task clause: demonstrated behavior,
missing handling, insufficient evidence, or a process-only requirement.
Requests are frozen before calls. No response repairs a patch or changes its
native score.

Eleven calls answer **116 clauses** for **$0.014120610**. One patch is skipped
because mandatory source exceeds the 128 KiB request bound; its slot remains
in the records. The answers contain **zero `missing_handling` labels**:
69 demonstrated, 20 insufficient evidence, and 27 process-only requirements.
All six known-failing SDK patches receive demonstrated labels for numeric
validation and request-aware validation. The uncertainty labels do not supply
an executable defect finding. None of the four trace patches rejected by the
post hoc tests receives a `missing_handling` label either. Thus the review
identifies none of the ten reviewed patches with demonstrated failures; this
is a case count, not a general accuracy estimate.

This is another negative result for using Jev to determine coding completion.
Larger, more complete source and clause-specific questions did not provide a
reliable omission detector in this experiment. Jev remains useful to test as
a selector or router; these results do not justify a semantic acceptance gate.

## Timing and cost boundaries

The native panel records **$4.912470852**: $4.907497800 in priced Claude usage
and $0.004973052 reported by the gateway for preparation. Provider requests
are counted once; CLI totals are reconciliation evidence. These are usage
estimates and reported charges, not reconciled subscription invoices.

The primary timer starts before recurring configuration validation and includes
preparation, source validation, native execution, capture, provider drain,
final checks, and confirmed scratch cleanup. It stops before the last summary
receipt serialization and outer process return. Summed primary intervals are
2,521.182 seconds; the serial panel spans about 2,524 seconds.

Separate setup includes **706.235 seconds** to provision the two Cargo seeds,
a Sonnet capability probe costing **$0.157791700** and taking 37.213 seconds,
and one earlier Jev span preflight costing **$0.001152984**. The advisory review
adds $0.014120610. Known inference for this round therefore totals
**[$5.085536146](round-costs.json)**, including these separately reported experiments. Initial
index construction, tool installation, machine/storage charges, orchestration
model use, and engineering time remain outside that number. Warm indexes and
Cargo seeds are explicit assumptions. The separate eight-attempt post hoc
verification takes another 465.981 seconds and makes no model calls; its
time is not added to the registered native endpoint.

The gateway serves the unversioned `typesafe-ai/jev` alias. Retained provider
metadata does not establish a weight/version pin. The review includes one
internal provider failover; its reported aggregate cost is retained, with no
separate failed-attempt invoice. The evidence does not establish free Jev usage.

## Next experiments

The [next context policy](next-context-policy.md) separates changes that can
be tested quickly:

1. Reserve implementation and test budgets within the existing byte ceiling.
2. Reuse the Rust briefing code's bounded dependency extraction, persist its
   supported relationships, and test one-hop caller/callee expansion.
3. Give each exact task clause a delivered/partial/omitted/no-match record;
   test admitting anchors before spending space on surrounding evidence.
4. Compare deterministic and Jev selection with dependency expansion on and
   off, using the same model, tools, context budget, and stronger independent
   acceptance on new tasks.

The current persisted syntax index has no call graph. Existing Rust code
provides limited same-file dependency candidates; cross-file aliases and
method dispatch remain unresolved. Record that uncertainty. A syntax link or
an advisory judgment must not stand in for behavioral verification.

The failures also suggest two inexpensive verification components:

- **Deterministic boundary mutations.** Start from a valid public fixture and
  change one property: record order, non-JSON whitespace, missing termination,
  an empty score legend, or a response option absent from the transmitted
  request. Keep a clean positive control. Check the public entry point and
  its downstream consumer, so a helper that rejects invalid input cannot
  hide a caller that bypasses it. Freeze the oracle against the historical
  public contract before testing new candidate patches.
- **Jev selection of executable checks.** Give Jev a fixed catalog of those
  bounded checks and public task facts, then let code run the selected checks.
  Compare against deterministic selection and the complete catalog. Measure
  omitted known defects, unnecessary checks, selection cost, and elapsed time.
  This tests whether a semantic selector can save verification work; it does
  not ask the selector to certify correctness. First calibrate on public
  synthetic mutations, then evaluate untouched tasks with the full independent
  catalog still acting as the final gate.

The current data supports trying these components, not claiming that either
already improves acceptance. Jev's full-source advisory review failed even
when the relevant SDK caller bodies were present. Adding more of that same
review should not be the next default intervention.

## Inspect and reproduce

The [tool guide](../../../../bench/jev-lifecycle/README.md) documents offline
briefing previews, native execution, recomputation, and advisory reviews.
[The native manifest](native-retained-manifest.json) binds 711 retained files
and the compressed archive. It includes all prompts, candidates, tool streams,
provider ledgers, preparation decisions, check logs, receipts, and fixed inputs.
Executor scratch directories and credentials are excluded.

Extract `native-evidence.tgz` into a new directory, then run
`report_native_pilot.py` with that directory's `plan/plan.json`, `plan/`,
`runs/`, and `panel/panel.json`. This makes no model calls or candidate
executions. [A separate arithmetic audit](independent-numeric-audit.json)
recomputes all 12 rows and prices directly without importing experiment
modules; its [script](independent-recompute.py) expects `collected/` and
`report/report.json` beside it.

The original frozen execution modules remain unchanged. The Python tooling
has 107 passing offline tests across the main and clause-review suites. No
product Rust behavior or routing changes are included.
