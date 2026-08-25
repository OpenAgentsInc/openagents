# bench-results

The append-only record of graded coder runs, one file per suite, one JSON line
per run. Issue
[#34](https://openagents.com/OpenAgentsInc/openagents/issues/34) asks for
results that append here with receipts; this directory is that store.

Every row must come from a real Harbor run. There are no seeded or example
rows: a fabricated measurement sitting in the file the trend line reads would
be worse than an empty file.

## What is in here

`tb2-quick.jsonl` holds three runs of the two-task `tb2-quick` suite on the
local Ollama lane, executed on 2026-08-25 through the real `openagents coder`
against real models, one trial at a time.

| Recorded    | Model                  | Accepted | Success | Wall clock | Gate       |
| ----------- | ---------------------- | -------- | ------- | ---------- | ---------- |
| `22:57:20Z` | `qwen3.8:27b-mtp-q8_0` | 1 of 2   | 50.0%   | 1395.8s    | passed     |
| `23:27:15Z` | `qwen3.8:27b-mtp-q8_0` | 1 of 2   | 50.0%   | 1395.8s    | passed     |
| `23:33:52Z` | `qwen3:0.6b`           | 0 of 2   | 0.0%    | 143.9s     | **failed** |

The first two are the same run twice: same suite digest, same run digest, same
model, same CLI version, and the same outcome on both tasks —
`openssl-selfsigned-cert` accepted, `regex-log` rejected on an agent timeout.
That is what a comparable pair looks like, and the trend between them reads
`unchanged`.

**The third row is a deliberate regression, and it is in the file on purpose.**
The lane was degraded to a much smaller model and nothing else changed. The
floor caught it — `success_rate>=0.500` breached at 0.000, gate failed, the
report exited 1 — and the trend shows `-100.0% worse` with `model also varies`
named as the confounder, which is exactly right: the model change _is_ the
regression.

It stays recorded rather than being cleaned up afterwards. It is a real graded
run of a real configuration, it says which model produced it, and a store whose
chain skips the runs somebody would rather not have in the trend is the thing
the receipts exist to prevent.

Note what the third row does to cost. The degraded lane is nearly ten times
faster per run and accepts nothing, so cost per accepted outcome moves from
"one accepted outcome for this much work" to undefined — `no_accepted_outcomes`,
not zero and not infinity. An agent that gets cheaper per attempt while
accepting less is the regression this metric exists to catch, and it is the one
a per-attempt average would report as an improvement.

All three rows report cost as `null`. The local lane bills no metered tokens,
so there is no per-token rate to apply — `unmetered_local_lane`, which is not
the same as free. No run has yet been recorded on a priced lane.

## Appending a run

```sh
# 1. Run the suite. Harbor grades the trials.
bench/run-suite.sh bench/suites/tb2-cross-section.suite.json \
  --model openai/gpt-5.6-luna --jobs-dir /tmp/gym-jobs-run

# 2. Score it and record it.
pnpm run effectiveness:report -- /tmp/gym-jobs-run/<job-dir> \
  --suite tb2-cross-section --lane proxy \
  --suite-manifest bench/suites/tb2-cross-section.suite.json \
  --thresholds packages/coder-effectiveness/thresholds/tb2-cross-section.json \
  --append bench-results/tb2-cross-section.jsonl
```

The report's exit code is unchanged by `--append`: `0` the gate passed, `1` a
floor was breached, `2` the gate was unverifiable. A fourth code, `3`, means the
run was scored but the store refused to record it, and it only ever replaces a
`0`.

## Only a full run of a named suite gets in

`--suite-manifest` is not optional here. The store refuses two shapes outright,
and neither refusal has a flag:

- **`unclassified_run`** — the run named no manifest, so nothing records which
  pinned task list it was supposed to cover. A row whose task list is only
  "whatever ran" can be compared to a later row that ran less, and the trend
  would read the difference as the coder changing.
- **`smoke_run`** — the run did not cover every task the manifest pins, or the
  manifest declares itself a fast lane. Either way the figures are over a
  different set of work than the suite's other rows.

That is how "a fast/smoke run is never a published score" is enforced rather
than advised. Publishing means reaching this directory, and the coverage check
reads the trial directories on disk, so a run cannot describe itself as
complete when it is not. A refused run still prints its full report; it just
does not become a row, and it exits `3`.

## Reading it

```sh
pnpm run effectiveness:compare -- bench-results/tb2-cross-section.jsonl
```

Compare verifies the chain first and refuses to compare a store that does not
verify. It then produces two views:

- **Lanes** — the same suite across lanes at their most recent runs, each
  measured against a baseline lane.
- **Trend** — the same suite on one lane over time, the shape #34's acceptance
  clause asks for and the shape a regression appears in.

## Receipts, and why they are a chain

Each row carries a `receipt`, a `receipt:<sha256>` digest over that row's own
fields **and** the receipt of the row before it.

- Edit a figure in an existing row and that row's contents stop matching its
  receipt: a `receipt_mismatch` naming the row.
- Insert, remove, or reorder rows and a row's `previousReceipt` stops naming the
  row before it: a `chain_broken` naming the row.

That is what makes the file append-only in practice rather than by convention.
A benchmark history nobody can quietly rewrite is worth more than one that
merely has not been rewritten yet.

It is a hash chain, not a signature. A signature would answer "who wrote this",
which needs a key the tooling has no business holding. The chain answers "has
this been rewritten since it was written", which is the question a trend
actually asks. If a signing seam arrives later, it signs the head receipt and
this chain still holds underneath it.

## Unknown costs stay unknown

A row's `costPerAcceptedOutcomeUsd` is `null` when the run could not be priced,
and it carries the `costDisposition` and `costCoverage` that say why. It is
never `0`. `gpt-5.6-luna` — the lane the graded runs use most — is deliberately
unpriced in the forge model catalog, so writing a zero here would launder an
unmeasured lane into a free one at exactly the point where the figure stops
being read next to its reason.

A comparison follows the same rule: a delta against an unpriced side is
`unpriced`, never `0` and never "improved".

## Row fields

| Field                         | Meaning                                                                                                                                                                                                |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `runDigest`                   | The report's pin over suite, lane, tasks, CLI version, model, rates.                                                                                                                                   |
| `suiteKey`                    | The narrower pin two rows must share to be comparable: suite, suite digest, sorted task list, rate catalog. Lane, model, and CLI version are excluded, because those are the axes a comparison varies. |
| `suiteId` / `suiteDigest`     | The manifest the run claimed, and the digest over its pinned tasks — dataset, git url, commit, and path per task, so the pin is over content and not over names.                                       |
| `tier`                        | Always `score`. Written down anyway, so a reader of the file never has to know the store's refusal rule to trust what the rows are.                                                                    |
| `jobId`                       | The Harbor job. A store refuses a job it already holds — re-scoring a run does not make it a second run.                                                                                               |
| `costPerAcceptedOutcomeUsd`   | Total run cost over accepted outcomes, failures included, or `null`.                                                                                                                                   |
| `gateStatus`                  | `passed`, `failed`, `unverifiable`, or `null` when no thresholds file was given.                                                                                                                       |
| `previousReceipt` / `receipt` | The chain.                                                                                                                                                                                             |

The full type is `BenchResultRow` in
`packages/coder-effectiveness/src/results-store.ts`.
