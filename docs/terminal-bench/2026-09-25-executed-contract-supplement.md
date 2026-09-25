# Executed contract checks: the remaining sixteen candidates

**The measurement for [#9628](https://github.com/OpenAgentsInc/openagents/issues/9628)
is complete; the contract checker remains unsuitable as a completion verdict.**
On the eight-task cohort excluded from its original study, it reproduces two
missing required outputs. Two additional failure calls agree with official
grades but arise from the replay environment's distributed-process startup
failure. Three of its six pass calls are wrong. No runtime policy changes.

This completes the original issue's component-and-measurement checklist, including
negative results. It does not complete #9584, admit a selector, or establish a
Microluna improvement against Fable.

## Population and provenance

The [original study](2026-09-25-executed-contract-checks.md) excluded eight tasks
while #9584's prospective grades were sealed. Those grades were subsequently
published: Luna passed 0/8 and Astra 4/8. This supplement runs the contract checker
on all 16 retained candidates, using the four published CAD regrades as well as
the 12 original grades. Original missing grades and regrade provenance remain
in every joined row.

The [supplemental protocol](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/supplemental-protocol.md)
was written before this replay, after the official labels were public. This is
explicitly **post-label component measurement**, not another blinded confirmation.
It does not change or pool into the original development/held-out split.

Before replay, the runner verified **2,220 retained files** against the published
Luna and Astra trace manifests. A separate jobs directory exposed only the 16
named trial directories. It never inventoried or read the running artifact-v2
cohort's outcomes or the v18 family's outcomes. Every candidate was restored from
its retained snapshot, and every snapshot was identified as the graded workspace.
There were no restoration errors, missing candidates, or unknown final labels.

The binary is the existing `3a25a0ff1f` build, SHA-256
`cdbf781be1c00814ba61bfddb6d69a581f6e3c345215b97afa4bba42b656bcd7`.
The [retained source diff](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/records-supplemental/component-source-diff.patch)
shows the helper visibility and separate component additions since the extractor
freeze at `e37c9a5097`. The contract extraction, Docker comparison, threshold, and
call rules used here are unchanged. Public environment images are pinned by
Docker image ID. All 16 candidate records report network mode `none`.

## Results against official grades

The frozen rule calls a candidate failed if any item differs, passed if an item
matches and none differs, and otherwise makes no call. These are the historical
component's labels; a `pass` is not an assertion that the whole task is solved.

| Measure | Result | Wilson 95% interval |
| --- | ---: | --- |
| Failure calls agreeing with official failures | 4/4 | 51–100% |
| Official failures called failed | 4/12 | 14–61% |
| Pass calls agreeing with official passes | 3/6 | 19–81% |
| Official passes called passed | 3/4 | 30–95% |
| No call | 6/16 | Not applicable |

**The first two rows include two infrastructure failures.** Agreement with the
official grade does not establish that the checker reproduced the candidate's
actual defect. Only the two missing-output detections below establish a literal
requirement violation. Treating those alone as useful detections describes 2/12
official failures; it is a post-audit observation, not a newly validated rule.

| Task | Luna official / checker | Astra official / checker | What the checker observes |
| --- | --- | --- | --- |
| `distributed-dedup` | Fail / unknown | Fail / unknown | No extracted item |
| `formal-crypto` | Fail / pass | Pass / pass | `/app/solve.sage` exists |
| `freecad-impeller` | Fail / unknown | Fail / unknown | No extracted item |
| `freecad-spring-clip` | Fail / unknown | Pass / unknown | No extracted item |
| `math-eval-grader` | Fail / pass | Fail / fail | Required paths and JSON object shape; Astra lacks `results.json` |
| `pretrain-shard-corruption` | Fail / fail | Fail / fail | Training command exits 1 during distributed startup |
| `shadow-relay` | Fail / fail | Pass / pass | Luna lacks `/app/output/flag.txt`; Astra has it |
| `vpp-loss-divergence` | Fail / pass | Pass / pass | `/app/output/loss_trace.pt` exists |

Per executor, the raw rule catches 2/8 Luna failures and 2/4 Astra failures.
Its three Luna pass calls are all wrong; its three Astra pass calls are all right.
Each executor has only one candidate per task. These figures do not show that
the checker can select among multiple Luna attempts.

## Per-kind coverage and discrimination

There are eight extracted items on five tasks: six paths, one JSON-format check,
and one command. Three tasks yield no item, accounting for all six unknown calls.
No interface, example, or exit-code item is extracted. A command's exit comparison
belongs to the command kind, not the separately defined exit-code kind.

| Kind and observation | Official-label agreement | Wilson 95% interval |
| --- | ---: | --- |
| Path differs; candidate fails | 2/2 | 34–100% |
| Paths match; candidate passes | 3/6 | 19–81% |
| Format matches; candidate passes | 0/1 | 0–79% |
| Command differs; candidate fails | 2/2 | 34–100% |

The missing `results.json` also makes its format check `could_not_run`; it is
not counted again as a format mismatch. The command row carries the startup
limitation described below.

Four tasks have a passing and a failing candidate. `shadow-relay` separates them;
`formal-crypto`, `freecad-spring-clip`, and `vpp-loss-divergence` tie. Undefined
scores count as ties, as in the original protocol. Mean within-task concordance
is **0.625**, with a whole-task bootstrap interval of **0.50–0.875**. There are
only four mixed-label pairs, each comparing Luna with Astra.

The supplemental bootstrap preserves the multiplicity of repeated sampled tasks.
The original helper collapses repeated task names when averaging concordance;
its narrower 0.50–0.75 interval is retained under `legacy_task_bootstrap_95`
for inspection, not used as the reported interval. The original study's records
remain unchanged. The supplement uses 10,000 resamples and seed 9628, with 41
undefined resamples that contain no mixed-label task. The 100–100% bootstrap
interval for raw failure precision likewise reflects four agreeing calls, not
certainty; Wilson's 51–100% interval exposes their small denominator.

## Why the calls do and do not help

**Two missing outputs are useful evidence.** The failed Astra math submission
lacks the required `/app/results.json`. The failed Luna relay submission lacks
`/app/output/flag.txt`, while Astra's passing submission supplies that file.
These are direct filesystem observations tied to explicit task requirements.

**Three false pass calls check existence or shape, not the required behavior.**
Luna supplies a solver script for `formal-crypto`, a JSON object and related files
for `math-eval-grader`, and a tensor file for `vpp-loss-divergence`. Those files
exist, but the official tasks still fail. The extractor does not evaluate the
proof, grading correctness, or numerical trace. File presence and parseability
cannot justify a completion verdict.

**The pretraining calls need an environment diagnosis.** The untouched image and
both candidates fail the same command. The component's short retained tail shows
only PyTorch's child-process failure, which is insufficient to identify its cause.
A separate [full-output diagnostic](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/diagnose_pretrain.py)
runs the same command on fresh copies, without changing any candidate or frozen
call. Its logs show `dist.init_process_group` failing to resolve/connect to the
container hostname, ending in a 45-second `DistNetworkError` before training.
This is not evidence about the candidate's shard-corruption fix. The original
four-call result stays retained; the audit prevents calling all four detections
reproduced task defects.

**Coverage is still narrow.** The checker finds no executable item for distributed
deduplication or either CAD task. Nothing in this supplement warrants turning it
into a stop rule or a candidate selector. Better extraction alone also would not
make a matched example or existing output a proof of general correctness.

## Cost, time, and reproduction

The first replay's eight invocations took **166.65 seconds** in total, including
planning, untouched baselines, restoration, and container teardown. Pretraining
accounted for 147.69 seconds. Per-candidate times are retained as whole seconds;
their median is recorded as zero because most metadata checks finish in less
than a second, not because they cost no time.

One unchanged Jev question resolved whether the instruction requires the
pretraining command to succeed: 899 input tokens and 21 output tokens, with
probability 0.94. At the repository's retained rate of $0.042 per million input
tokens, its estimated cost is **$0.000037758**. No usage is missing. No Luna or
Astra candidate generation or official verifier execution ran for this work.

A second replay reuses all eight saved plans with `--jev recorded --reuse-plan`.
All **16 candidate calls, scores, item identities, kinds, and outcomes reproduce**
with no new model calls. Raw command text can contain different timestamps,
container IDs, and local ports; the reproduction comparison does not equate
those incidental strings. Full records of both runs remain retained.

The experiment directory contains:

- [Input identities and 2,220 verified source files](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/records-supplemental/inputs.json).
- [Complete supplemental results](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/records-supplemental/summary.json), with per-task, per-executor, per-kind, uncertainty, timing, usage, and original/regrade provenance.
- [Reproduction receipt](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/records-replay/reproduction.json), plus every repeated plan, report, label, and invocation.
- `records-supplemental/pretrain-diagnostic/`: full startup output from the unchanged image and both restored candidates; these diagnostic invocations are separate from the 166.65-second primary replay.
- `test_supplemental.py`: cohort exclusion, duplicate protection, report/grade identity, missing-results accounting, CAD regrade retention, ungraded-snapshot refusal, and bootstrap multiplicity tests.

To reproduce the join locally:

```sh
python3 bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/supplemental_measure.py \
  bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/records-supplemental \
  --published bench/terminal-bench/experiments/2026-09-25-candidate-review/records/prospective-measurement.json \
  --out /tmp/contract-9628-summary.json
```

For execution replay, `supplemental.py prepare` takes the published record
directory, retained jobs, public task directory, pinned binary, and a fresh output
directory. `supplemental.py run` takes the same arguments plus
`--plans PATH_TO_RETAINED_RECORDS` to reuse these plans without model calls.
The process receipts retain the exact original commands and image IDs.

## Completion checklist

- Extractor, runner, typed outcomes, and component fixtures: delivered by
  `e37c9a5097` and the original measurement.
- Existing snapshots and Microluna candidates, split by task with negative
  results and uncertainty: retained in the original 163-workspace study.
- The eight fresh #9584 tasks: all 16 Luna/Astra candidates now measured,
  joined to their published final grades, and replayed without new model calls.
- Per-kind and within-task analysis, coverage, limitations, costs, timing,
  and reproducible records: published here and in the linked artifacts.
- Policy admission: none. The negative component result is retained rather
  than promoted into a runtime success claim.
