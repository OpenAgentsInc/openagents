# Literal artifact checks: development results

The [artifact lifecycle correction](2026-09-25-literal-lifecycle.md) supersedes
the original checker described below. Synthetic controls exposed temporary-file
and deferred-write false alarms; the correction removes those while preserving
all 72 development calls. Both versions remain retained.

The new deterministic checker catches **three real failures with three failure
calls**, using no model calls. Across all 72 opened archive candidates, its median
end-to-end replay time is **0.747 seconds**, including snapshot restoration and
container setup. This is a narrow, useful component improvement, not held-out
validation or evidence that a live agent finishes sooner.

[#9646](https://github.com/OpenAgentsInc/openagents/issues/9646) implements the
component. [#9584](https://github.com/OpenAgentsInc/openagents/issues/9584) remains
open: the [original confirmation](2026-09-25-archive-check-confirmation.md) missed
its declared bar, and these outcomes were already known when this component was
written. Existing runtime policies and the original contract extractor are
unchanged.

## What changed

The original extractor missed a single-quoted required CSV path. A reasoning
reviewer then demonstrated the missing file, but Jev's scores fell below its
frozen cutoff. File existence and a literal byte ceiling do not need that veto.

The Rust `checks::contract::literals` component reads explicit output declarations
and records necessary conditions in a separate versioned plan. It recognizes
backticks, single quotes, double quotes, and bare filenames. Each obligation
retains its exact instruction span, output declaration, normalized path, and
typed requirement. The plan records the instruction digest and seals its own
contents with a digest.

Supported declarations start with an output verb such as `write`, `create`, or
`save`, optionally preceded by `you must` or `you shall`. A byte limit must name
an independently declared output and say `must be at most N bytes` or `must be
less than N bytes`. The latter normalizes to an inclusive limit of N−1. Valid
comma grouping is accepted; unsupported units, overflow, and impossible negative
limits abstain. This deliberately small grammar does not cover every requirement
expressed in natural language.

Optional, conditional, negative, example, input/reference, and alternative-output
contexts abstain. In particular, `write A or B` does not require A. Paths must be
normalized and lie within the declared working directory. A changed plan is
refused before candidate access. These checks establish artifact identity and
integrity, not the semantic completeness of the extracted contract.

Execution uses the existing host's file metadata operation only. It reads no
candidate file contents, executes no candidate commands, and asks no model.
Missing required paths or oversized files produce `fail`. Unreadable metadata,
unsupported wording, and satisfied necessary conditions produce no failure call.
A match never produces a whole-task `pass`.

```sh
coder-one checks contract literal-plan \
  --instruction instruction.md --task example --workdir /app --out plan.json
coder-one checks contract literal-run --plan plan.json --out report.json
```

The schemas are `openagents.coder-one.literal-artifact-plan.v1` and
`openagents.coder-one.literal-artifact-report.v1`. The old plan reader refuses
the new plan shape, preventing an old consumer from treating matched necessary
conditions as completion. These commands reject inference flags and undocumented
options. They are standalone experimental commands, not an admitted policy.

## Development replay

The population is the same 72 candidates and unchanged official labels from the
12-task archive confirmation. Every snapshot matches its original sealed
identity and stays unchanged during replay. All 72 checks run successfully; none
is dropped because its evidence is inconvenient. Five tasks yield 12 literal
obligations; seven tasks yield an empty plan and therefore abstain.

| Signal | Failure precision, Wilson 95% | Real-failure recall, Wilson 95% | Abstentions |
| --- | --- | --- | --- |
| Original scenario checks | 1/1, 100% (21–100%) | 1/11, 9% (2–38%) | 71 |
| Original combined verdict | 1/9, 11% (2–44%) | 1/11, 9% (2–38%) | 18 |
| Original executed detector | 6/9, 67% (35–88%) | 6/11, 55% (28–79%) | 63 |
| Literal artifacts | 3/3, 100% (44–100%) | 3/11, 27% (10–57%) | 69 |
| Literal artifacts OR original executed detector | 7/10, 70% (40–89%) | 7/11, 64% (35–85%) | 62 |

The three literal detections are:

- `write-compressor__a7ExoaT`: 2,711 bytes against an explicit 2,500-byte ceiling.
- `write-compressor__hfBx6Mh`: 5,000 bytes against that ceiling. Its round trip
  succeeds, so testing the round trip alone misses this failure.
- `financial-document-processor__iUk2vrc`: the explicitly required
  `/app/invoices/summary.csv` does not exist. This is the new detection the
  original reproduced-review verdict missed.

Two detections overlap the earlier reviewer. The OR calculation reuses those
original review predictions without changing their questions, cutoff, commands,
or labels. Its three official false alarms remain the circuit specification
defects documented in the separate audit. It still does not beat the scenario
baseline's failure precision.

Within the four task groups containing both official passes and failures, the
literal detector has 62.1% concordance over 33 dependent pairs; the OR has 81.8%,
against the previous executed detector's 78.8%. Their equal-task means are 61.5%
and 81.3%. These pairs are descriptive, not 33 independent validation trials.
The OR's paired whole-task bootstrap interval against scenario checks is
−75 to 0 percentage points for precision and +8.3 to +100 points for recall.
There are 3,451 undefined precision resamples and 73 undefined recall resamples
out of 10,000. Development fitting and only 12 task groups prevent a confirmation
claim regardless of those intervals.

## Cost, controls, and retained evidence

The final replay uses zero model calls and 56.379 seconds of summed process
time across 72 candidates, with two workers. That sum is not elapsed batch time.
A future cheap-first rule can skip a review when a literal failure already
decides the result. This replay realizes no retrospective savings: the original
72 reviews had already run and cost $12.469 including Jev.

The checker used for this original measurement is built from Rust source at `68fd15388f`, binary SHA-256
`2a797973b9931a186be69db1dee407cc129c02cdb52dc623167c6934015103bf`.
It uses the previously hashed four-library runtime. Validation includes:

- Nine focused Rust tests, including optional/alternative wording, exact size
  boundaries, unsupported units, metadata errors, tampered plans, no content
  reads or subprocesses, and rejection of inference options.
- Five live CLI controls: missing, exact-boundary, oversized, and empty outputs,
  plus an alternative-output declaration. Candidate contents remain unchanged.
- Twelve ordinary contract plans reproduced byte-for-byte in the original
  public images. The old extractor's results are unchanged.
- All five selected manual Rust phases at `68fd15388f`: formatting, strict
  Clippy, feature Clippy, tests, and feature tests for Coder One. This is a scoped
  partial gate, not a full-workspace claim. The initial test run lacked `make`
  on PATH; its failure is retained, and supplying the installed tool fixes it.
- A second complete 72-candidate replay after tightening alternative-output
  handling. All calls match the first replay. The first measurement's abstention
  count used the wrong null representation; that record is retained, corrected,
  and covered by an exhaustive count invariant. Precision and recall did not
  change.

The [measurement](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/records/literal-development-measurement.json)
contains every paired row, executor breakdown, interval, and within-task result.
The [manifest](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/records/truth9646-files.json)
identifies all 732 files in the
[evidence bundle](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/records/truth9646-records.tar.gz),
SHA-256 `742e4ea79dc92d2268557030a502be7b2a4b6b9a84d5ccab4598219f22090b35`.
The bundle was scanned against local credential values, restored separately,
and verified file by file. It includes both replays, all plans and reports,
controls, compatibility results, the original failed gate, and successful gates.

## Next confirmation

Following the [September 25 assessment](../coder/design/2026-09-25-assessment.md),
this work moves from independently supported metadata checks through controls
and retained replay before new agent trials. The
[reserved task selection](../../bench/terminal-bench/experiments/2026-09-25-literal-confirmation/selection.md)
starts environment-only feasibility on 15 new task groups. The final population
and complete combined rule must be committed before generation. At least three
attempts per task, task-group uncertainty, all failures, and total costs remain
required. No controller, best-of-N selector, or stopping policy gains authority
from this development result.
