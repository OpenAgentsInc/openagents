# Lev adapters outside the support domain

The choice, band, and permutation adapters' observed gains on support items
do not persist as gains over base on the retained coding workload. All three
refuse 35 of its 130 questions. On external public labels, base answers
79/121 correctly while refusing 39 of 160 requests. These are coverage and
transfer measurements, not new calibration grants.

This record completes the Lev measurements requested by #9380. It keeps the
new runs, historical comparisons, full probability panels, and refusal
denominators separate. The accompanying
[evidence directory](../2026-09-20-lev-domain-gap/) retains their inputs and outputs.

## What was compared

The out-of-domain workload is the unchanged development partition of
`coder-turns-v1`: 130 questions over recorded coding sessions. The suite
contains author labels and outcome labels, which have different evidentiary
strength. The pooled table names that mixture; the retained panels and run logs
separate families and label evidence. The external run uses `external-v1`'s
public labels.

In-domain comparisons use the 157 open items of `support-v2-three-way` and,
separately, the 79 of those items that were in the original `support-v2`
evaluation split. The other 78 open items overlap the adapters' training
split. The 157-item result describes the open suite, not generalization to
unseen examples. The 79-item subset is the more relevant in-domain comparator.
No locked item was sent to a model for this record.

The band in-domain panel is a retrospective selection from retained seed-block
0 draws, not a new run. All 196 historical and current item IDs, state texts,
questions, truths, families, and kinds match exactly. Only partition metadata
changed. Selection does not rewrite the historical rows' suite or split.
Base and choice in-domain panels come from their retained Gym rows. The band
block has eight samples per item and no refused draws; it retains the package
identifier and base-signature prefix. Its raw top share and correctness are
sufficient for Gym's full probability panel. None of these historical runs
is represented as newly measured on today's machine.

## Full panels

ECE, Brier, NLL, and confident-error counts describe answered questions. Brier
and NLL score the winning option's outcome, as Gym defines them; NLL is not
the multiclass log probability of the truth. Accuracy with refusals counted
as unsuccessful is a separate column. The unit is a question, not a turn.

| Run and selection | Correct / answered / asked | Answered accuracy | All-item accuracy | ECE | Brier | NLL | Confident errors |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `historical/lev-base/open157` | 123 / 157 / 157 | 0.783 | 0.783 | 0.107 | 0.161 | 2.353 | 12 |
| `historical/lev-adapted@1/open157` | 148 / 157 / 157 | 0.943 | 0.943 | 0.065 | 0.062 | 1.443 | 8 |
| `historical/lev-band/block0/open157` | 145 / 157 / 157 | 0.924 | 0.924 | 0.064 | 0.064 | 1.446 | 8 |
| `historical/lev-base/untrained79` | 61 / 79 / 79 | 0.772 | 0.772 | 0.106 | 0.161 | 2.024 | 5 |
| `historical/lev-adapted@1/untrained79` | 70 / 79 / 79 | 0.886 | 0.886 | 0.127 | 0.120 | 2.861 | 8 |
| `historical/lev-band/block0/untrained79` | 69 / 79 / 79 | 0.873 | 0.873 | 0.112 | 0.118 | 2.847 | 8 |
| `historical/lev-base/coder-turns-v1` | 70 / 95 / 130 | 0.737 | 0.538 | 0.145 | 0.175 | 1.276 | 3 |
| `historical/lev-adapted/coder-turns-v1` | 67 / 95 / 130 | 0.705 | 0.515 | 0.217 | 0.243 | 3.551 | 11 |
| `new/lev-band/coder-turns-v1` | 69 / 95 / 130 | 0.726 | 0.531 | 0.180 | 0.213 | 2.432 | 7 |
| `new/lev-permutation/coder-turns-v1` | 69 / 95 / 130 | 0.726 | 0.531 | 0.134 | 0.206 | 1.365 | 3 |
| `new/lev-base/external-v1` | 79 / 121 / 160 | 0.653 | 0.494 | 0.222 | 0.247 | 3.148 | 12 |
| `new/lev-permutation/open157` | 145 / 157 / 157 | 0.924 | 0.924 | 0.063 | 0.060 | 0.959 | 5 |
| `new/lev-permutation/untrained79` | 69 / 79 / 79 | 0.873 | 0.873 | 0.106 | 0.105 | 1.866 | 5 |

## Band out-of-domain run

The run retained every one of the 130 development item IDs, each exactly once.
The receipt chain and typed row checks pass, and suite digests match. Gym
reported 95 scored, 35 refused, and zero lost to the harness. Refusals were
32 `branch_too_long` and three `invalid_request`. The band answered 69 questions
correctly: 69/95 = 0.726 among answers, or 69/130 = 0.531 over the full workload.
The historical choice result is 67/95 = 0.705 among answers and 67/130 = 0.515
over the full workload. Neither denominator should be omitted.

## Domain gaps and the historical floor

The in-domain denominator below is the same 79 original evaluation items for
every door. The out-of-domain workload contains 130 different questions. Base,
choice, band, and permutation answer the same 95 item IDs on that workload;
their other 35 items are retained refusals. Thus the within-domain comparisons use matched
items, but an in-domain-to-out-of-domain gap is not a paired-item comparison.

| Door | In-domain correct / 79 | OOD correct / answered / asked | Answered-only gap | All-item gap | Answered gap magnitude / 0.056 |
| --- | --- | --- | --- | --- | --- |
| Base | 61 / 79 | 70 / 95 / 130 | -0.035 | -0.234 | 0.63 |
| Choice | 70 / 79 | 67 / 95 / 130 | -0.181 | -0.371 | 3.23 |
| Band | 69 / 79 | 69 / 95 / 130 | -0.147 | -0.343 | 2.63 |
| Permutation | 69 / 79 | 69 / 95 / 130 | -0.147 | -0.343 | 2.63 |

A gap is out-of-domain accuracy minus in-domain accuracy. The answered-only
gap conditions on a door producing an answer. The all-item gap counts every
refusal as unsuccessful and describes coverage as well as correctness. ECE,
Brier, NLL, and confident-error counts remain answer-only metrics; a refusal
does not acquire a fabricated probability to enter those calculations.

The historical 0.056 reference came from eight seed blocks on one unchanged
base door and 98 unchanged support evaluation items. Their accuracy standard
deviation was 0.0197; the published two-door, one-block-per-door calculation
was `2 × sqrt(2) × 0.0197 ≈ 0.056`. Its derivation holds the suite, items,
machine, and door fixed while changing seeds. See the retained
[seed-variance record](../../lev/measurements/2026-09-19-seed-variance.md).

The last column answers #9380's requested comparison to that reference. It is
only a ratio of effect magnitudes. It is not a sigma count, a significance
test, or evidence that the support-suite noise estimate transfers to 79
support items, 130 coding questions, or their difference. The new domains
change question families, labels, item difficulty, coverage, and the sampled
population. No cross-domain uncertainty floor was measured here. An accuracy
floor also supplies no floor for ECE, Brier, NLL, or confident-error counts.

The matched 79-item in-domain panel gives choice nine more correct answers
than base (+0.114), and band and permutation each eight more (+0.101). On the common 95 answered
coding items, choice gives three fewer correct answers than base (-0.032),
and band and permutation each give one fewer (-0.011). These are descriptive paired differences
on their respective item sets, not proof of statistical significance. They
show that the observed in-domain gains do not persist as observed gains on
this workload; they do not identify a general transfer error rate.

## Band run environment

The band run used an Apple M5 Max, macOS 26.4 build 25E246, eight L2 samples,
seed base 0, and four helpers. It started at 2026-09-20T17:53:12.671996Z and
finished at 18:15:30.218830Z, taking 1,337.495 seconds including startup and
controller overhead. Load averages moved from 3.592/3.725/3.704 to
2.790/3.224/3.071. Compilations and desktop applications were active. This is
an accuracy run, not a quiet latency measurement.

The controller recorded checkout `f3989d1ced94cd87c46f3e50d585468620e581ba`.
It separately retained binary hashes and verified adapter weight and metadata
hashes against `lev-adapted-v2.json` before launching. The published identity
was `fmadapter-levband-9799725`, not `lev-adapted@2`; retained rows keep that
identity unchanged.

```sh
lev-serve --port 11457 \
  --adapter "$HOME/code/lev-adapter-work/runs/lev-band/levband.fmadapter"
gym eval --suite crates/gym/suites/coder-turns-v1.json \
  --partition development --timeout 300 \
  --door lev-band=http://127.0.0.1:11457 --record ood-band.jsonl
```

The completed raw store SHA-256 is
`bc7d017b4fb2a8d55aeda188effa4352daf08073630c5b2fb7c06a3e7b0951e7`.

### Interrupted attempt

A previous attempt retained 125 rows before the user interrupted the active
tool wait. The controller and Gym client no longer existed; the remaining
orphan server was terminated. There was no terminal result. Those 125 rows
remain unchanged, with SHA-256
`3b8933f690c968ba390995fc784e991a950db5c1033b20abfc18c83b164ab21c`.
The complete unchanged suite was then rerun into a fresh store. The partial
attempt is neither merged into the completed run nor silently discarded; it
is not counted as a second complete trial or chosen based on its score.

## Permutation out-of-domain run

All 130 development items are retained exactly once, with valid receipts and
row checks. Suite and question digests match the band run. It answered 95,
refused 35 (32 `branch_too_long`, three `invalid_request`), and lost zero to
the harness. It got 69 correct: 0.726 among answers, 0.531 over all requests.
Its equal aggregate correctness count with band does not mean identical
answers; its ECE, Brier, NLL, and confident-error count are different.
The additional in-domain run below supplies the permutation gap. Its
answered-only accuracy falls from 69/79 = 0.873 on untrained support items to
69/95 = 0.726 on coding questions, a descriptive gap of -0.147.

The run used the same Apple M5 Max, macOS 26.4 build 25E246, L2 eight samples,
seed 0, and four helpers. It ran from 2026-09-20T18:15:30.263543Z to
18:39:55.942349Z, 1,465.631 seconds including controller overhead. Load moved
from 2.790/3.224/3.071 to 3.588/5.776/6.920. This was not a quiet latency run.
The recorded checkout was `fc7441708d84700fbbe2ee07f9d04223ee811726`; the
controller retained binary and artifact hashes. The published adapter identity
was `fmadapter-levperm-9799725`.

```sh
lev-serve --port 11457 \
  --adapter "$HOME/code/lev-adapter-work/runs/lev-perm/levperm.fmadapter"
gym eval --suite crates/gym/suites/coder-turns-v1.json \
  --partition development --timeout 300 \
  --door lev-permutation=http://127.0.0.1:11457 --record ood-permutation.jsonl
```

Raw-store SHA-256:
`af71a8649fe9b2cfc885b0d0c85dbc4a03100b9caef13a480946991320d5c178`.

## Base run on external labels

All 160 open items are retained exactly once: 80 calibration and 80 development
items, with the 40 locked items unread. The receipt chain and typed row checks
pass. There were 121 answers, 39 `guardrail` refusals, and zero harness losses.
The base got 79 correct: 79/121 = 0.653 among answers, but 79/160 = 0.494 over
the full workload. BoolQ contributes 45 correct out of 62 answers and 80
requests; MNLI contributes 34 correct out of 59 answers and 80 requests.
Their published agreement references are 0.90 and 0.887 respectively; these
are label-agreement references, not estimated agreement for this selected set.

Compared with its 61/79 = 0.772 on untrained support items, the base's
answered-only external gap is -0.119, or 2.13 times the borrowed 0.056
reference. The all-item gap is -0.278. These are descriptive differences under
the same cross-domain caveats above. Guardrail selection changes which
examples enter the answer-only panel. This is base-model evidence only;
no adapted external run is implied.

The run used the same machine and estimator settings, from
2026-09-20T18:39:56.001485Z to 18:43:54.401158Z, 238.374 seconds including
controller overhead. Load moved from 3.588/5.776/6.920 to 3.950/4.810/6.207.
It is not a quiet latency measurement. The checkout was
`8ffdae9becd1c2e7b0662caafe200c8a46000713`. The server binary changed between
the permutation and external runs; the retained controller results record
both hashes. The external base published signature prefix `9799725`, no
adapter, eight samples, and seed 0. Rows retain that prefix without replacing
it with a retrospective full signature.

```sh
lev-serve --port 11457
gym eval --suite crates/gym/suites/external-v1.json --timeout 300 \
  --door lev-base=http://127.0.0.1:11457 --record ood-external.jsonl
```

Raw-store SHA-256:
`b06562ba82cbe99e54e12a7d363b5726a9cb5e98ddd30b1573659b6c71f37665`.

### OOD family panels

These rows keep refusals in the requested-item denominator. The probability
panel scores answers only; it does not hide refusals or count them as confident
errors. The coding suite mixes author and outcome labels, so its aggregate
panel should be read beside the label-evidence panels retained in each run log.

| Door and suite | Family | Correct / answered / asked | ECE | Brier | NLL | Confident errors |
| --- | --- | --- | --- | --- | --- | --- |
| `new/lev-band/coder-turns-v1` | `action` | 6 / 8 / 16 | 0.312 | 0.227 | 0.659 | 0 |
| `new/lev-band/coder-turns-v1` | `damage` | 21 / 21 / 22 | 0.060 | 0.016 | 0.070 | 0 |
| `new/lev-band/coder-turns-v1` | `needs_code` | 5 / 8 / 16 | 0.406 | 0.363 | 7.248 | 2 |
| `new/lev-band/coder-turns-v1` | `progress` | 4 / 8 / 16 | 0.500 | 0.430 | 4.314 | 1 |
| `new/lev-band/coder-turns-v1` | `risk` | 4 / 8 / 16 | 0.266 | 0.271 | 0.773 | 0 |
| `new/lev-band/coder-turns-v1` | `shell_outcome` | 18 / 21 / 22 | 0.113 | 0.093 | 1.428 | 1 |
| `new/lev-band/coder-turns-v1` | `useful` | 11 / 21 / 22 | 0.327 | 0.362 | 4.553 | 3 |
| `new/lev-permutation/coder-turns-v1` | `action` | 5 / 8 / 16 | 0.188 | 0.152 | 0.461 | 0 |
| `new/lev-permutation/coder-turns-v1` | `damage` | 19 / 21 / 22 | 0.036 | 0.055 | 0.160 | 0 |
| `new/lev-permutation/coder-turns-v1` | `needs_code` | 4 / 8 / 16 | 0.266 | 0.338 | 4.043 | 1 |
| `new/lev-permutation/coder-turns-v1` | `progress` | 3 / 8 / 16 | 0.484 | 0.400 | 1.078 | 0 |
| `new/lev-permutation/coder-turns-v1` | `risk` | 6 / 8 / 16 | 0.234 | 0.244 | 0.675 | 0 |
| `new/lev-permutation/coder-turns-v1` | `shell_outcome` | 20 / 21 / 22 | 0.173 | 0.096 | 0.298 | 0 |
| `new/lev-permutation/coder-turns-v1` | `useful` | 12 / 21 / 22 | 0.345 | 0.347 | 3.333 | 2 |
| `new/lev-base/external-v1` | `boolq` | 45 / 62 / 80 | 0.224 | 0.222 | 4.617 | 10 |
| `new/lev-base/external-v1` | `mnli` | 34 / 59 / 80 | 0.229 | 0.273 | 1.605 | 2 |

## Permutation in-domain run

The missing permutation panel now has a complete current-harness run: 157
open `support-v2-three-way` items, 157 answers, no refusals, and zero harness
losses. The store contains each expected item exactly once and passes receipt,
row, estimator, identity, and suite-digest checks. The adapter gets 145/157
correct overall and 69/79 on the original evaluation items that remain open.
The 78 other open items overlap its training split; their inclusion explains
why the full open panel is not a clean generalization estimate. No locked
item was submitted and no new calibration map was fitted.

This run used an Apple M5 Max, macOS 26.4 build 25E246, eight L2 samples,
seed 0, and four helpers. It ran from 2026-09-20T18:48:08.919123Z to
18:52:19.714983Z, 250.777 seconds including controller overhead. Load moved
from 3.393/4.082/5.524 to 5.246/4.630/5.380. This was not a quiet latency run.
The checkout was `4edf4f1d16573801411a6f4b02d62e7e3d607045`; the controller
retains server and Gym binary hashes, matching adapter hashes, and the
published package identity `fmadapter-levperm-9799725`.

```sh
lev-serve --port 11459 \
  --adapter "$HOME/code/lev-adapter-work/runs/lev-perm/levperm.fmadapter"
gym eval --suite crates/gym/suites/support-v2-three-way.json --timeout 300 \
  --door lev-permutation=http://127.0.0.1:11459 \
  --record in-domain-permutation.jsonl
```

The raw-store SHA-256 is
`c0a522b3bbcfe9756ff657fac8298d307e13731cfd22a2485f26cfa4a92eb3e0`.

## Label agreement beside the support panels

The retained support family panels use the same authored labels as the
historical records. Their independent second-reader agreement estimates
remain routing 0.980 (51 items; Wilson 95% interval 0.897–0.997), urgency
0.935 (31 items; 0.793–0.982), and severity 1.000 (18 items; 0.824–1.000).
The second reader was automated; agreement measures reproducibility of labels,
not their objective correctness. These estimates are historical, not new
annotations of the 79-item subset. See the
[instrument-validity record](2026-09-20-instrument-validity.md).

## Admission and evidence references

Current calibration references admit routing on `lev-base@1` and the choice
release `lev-adapted@1`. Band and permutation admit no families. These grants
rest on in-domain calibration evidence and do not establish out-of-domain
admission. Raw transfer measurements are retained through `observationRef`,
which validates a digest-pinned result store without creating a calibration
map or a grant. Existing `evalRef` entries are preserved.

## Retained evidence and reproduction

[The full machine-readable panels](../2026-09-20-lev-domain-gap/panels.jsonl)
include every door's aggregate and family metrics. `historical-*.jsonl`
files are byte-for-byte snapshots of committed stores at
`4edf4f1d16573801411a6f4b02d62e7e3d607045`; they are not fresh trials. The
historical sources are the
[coding-workload record](2026-09-19-coder-turns.md),
[choice-adapter record](../../lev/measurements/2026-09-19-adapter-v1.md), and
[calibration-variance record](../../lev/measurements/2026-09-19-calibration-variance.md).
The band comparison selects block 0 once; it does not average repeated blocks
or select the most favorable block.

For each new run, `*-result.json` retains the exact command, checkout, chip,
OS, loads, elapsed time, and binary hashes. `*-models.json` retains the actual
published identity and estimator; `*-run.txt` retains Gym's report with trailing
blank lines removed. Raw JSONL stores are unchanged.
The controller's checkout value identifies its working tree, while the binary
hash identifies the executed program; these are recorded separately. None of
the new runs loaded calibration maps. Old raw rows remain on their historical
gate metadata; new rows name `probability-v2`. This report computes raw panels
and does not transfer an old admission verdict to a new gate.

`interrupted-band/` preserves the incomplete attempt separately. The
[checksum manifest](../2026-09-20-lev-domain-gap/SHA256SUMS) pins every retained
file. The standalone Rust analyzer calls Gym's scoring functions rather than
reimplementing their formulas. It checks receipt chains and typed rows,
unique coverage, suite identity, published door identity, estimator settings,
and equality of historical/current support inputs before taking subsets.

From the repository root, with the pinned toolchain:

```sh
cd docs/decision-models/2026-09-20-lev-domain-gap
shasum -a 256 -c SHA256SUMS
cd ../../..
CARGO_TARGET_DIR=/tmp/lev-domain-gap-analysis cargo run --locked \
  --manifest-path docs/decision-models/2026-09-20-lev-domain-gap/analysis/Cargo.toml -- \
  docs/decision-models/2026-09-20-lev-domain-gap \
  docs/decision-models/2026-09-20-lev-domain-gap/ood-band.jsonl \
  docs/decision-models/2026-09-20-lev-domain-gap/ood-permutation.jsonl \
  docs/decision-models/2026-09-20-lev-domain-gap/ood-external.jsonl \
  docs/decision-models/2026-09-20-lev-domain-gap/in-domain-permutation.jsonl \
  > /tmp/lev-domain-gap-panels.jsonl
cmp /tmp/lev-domain-gap-panels.jsonl \
  docs/decision-models/2026-09-20-lev-domain-gap/panels.jsonl
```

Offline checks for this publication: workspace and standalone-analyzer
formatting, strict Lev Clippy with `serve` and all targets, and all committed
manifest tests. These checks validate references and reproduction; they do
not stand in for the full hardware verification run tracked separately.
