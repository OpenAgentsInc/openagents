# Laya port baseline

**Date:** 2026-09-22 · **Host:** Apple M5 Max, macOS 26.4 · **Device:**
CPU, fp32 throughout (the reference keeps modules fp32 and only autocasts
on CUDA) · **Build:** `cargo build --release -p laya`

The first run record for the Rust port: conformance against the Python
reference, the cost of serving it, and scored gym runs against three
committed suites. Nothing in this record transfers an upstream
benchmark claim to a local door.

## Declared workload

- Requests: the four conformance cases per checkpoint —
  `support_mixed` (three questions: noul + choice + score), `structured_state`
  (three questions over a non-string state), `choice_cardinalities`
  (choice questions across temperature buckets), `noul_defaults`.
- Latency: 15 warm iterations of `model.system_one` per case, from
  `cargo run --release --example measure`; medians reported.
- Memory: peak resident set under `/usr/bin/time -l`, load plus all
  iterations.
- Conformance: `cargo test -p laya --test conformance` with
  `LAYA_BUNDLE_DIR=~/work/laya-artifacts` — sequence ids and marker
  positions exact, answer fields within the fixture tolerances.

## Conformance

| Checkpoint | Sequence parity | Max answer |delta| |
| --- | --- | --- |
| `english` | exact | 0.000000 |
| `multilingual` | exact | 0.000000 |
| `typed-decisions` | exact | 0.000000 |

Every answer field — probabilities, scores, confidences, act
probabilities, model id, token accounting — matches the reference's
reported values exactly on this host. Detail and the RoPE-resolution
caveat in [`../conformance.md`](../conformance.md).

## Load and warm latency

| Checkpoint | Load | `support_mixed` | `structured_state` | `choice_cardinalities` | `noul_defaults` | Peak RSS |
| --- | --- | --- | --- | --- | --- | --- |
| `english` | 1.83 s | 316 ms | 359 ms | 338 ms | 177 ms | 3.28 GB |
| `multilingual` | 1.67 s | 125 ms | 136 ms | 129 ms | 77 ms | 2.89 GB |
| `typed-decisions` | 1.81 s | 289 ms | 307 ms | 360 ms | 177 ms | 3.26 GB |

One call is one forward — every question in the request is a row of the
same batch, so a three-question case costs about what three single
questions cost. The multilingual checkpoint runs its 22-layer, 768-wide
encoder at roughly 2.5× the English model's speed; `noul_defaults` is
cheapest because its sequences are shortest.

Throughput is one forward at a time per slot: `laya-serve` admits
`--concurrency` forwards across all variants (default 2) inside a
measured memory budget, and refuses `busy` immediately when every slot
is taken rather than queueing without bound.

## Memory accounting

Peak RSS covers tokenizer, fp32 weights (~842 MB per ModernBERT-large
checkpoint, ~644 MB for mmBERT-base), and forward working memory.
`laya-serve` budgets working memory separately: one forward of N
question rows at `max_len` tokens is priced by `Variant::forward_bytes`
(attention scores plus residual stream), and the per-variant slot count
is the budget divided by that estimate. The budget itself is the host's
measured available memory at startup, or `--memory-budget-mib`.

## Refusal and coverage surface

Exercised in `cargo test -p laya` and the validation tests: empty
question map, unsupported `type`, missing or mis-shaped criteria, >255
options per question, >64 questions or >1024 total options per request
(defaults, configurable), options that cannot fit markers inside
`head_max_len`, unknown model names, saturated forward slots, and
question fields the model never reads (`deny_unknown_fields` semantics
via the hand-written `Question` deserializer). Each answers with the
shared refusal envelope — `{"detail", "error": {"code", "message",
"question"}}` — at the status its class carries.

## Gym runs

Scored with `gym eval --door <variant>=http://127.0.0.1:8735 --fit`
against `laya-serve` holding all three checkpoints (concurrency 4,
CPU fp32). Row records live in `crates/gym/results/` beside the other
run ledgers; the locked partitions were not asked in any suite.

### `support-v2-three-way` — 157 items asked, 0 refused

Support-desk judgments authored in this repository, so no upstream
training set contains them. The labels are the author's best reading
of deliberately arguable items; 0.60 here is not 0.60 on a cleaner
set.

| Checkpoint | Calibration acc / ECE | Development acc / ECE | Confident errors |
| --- | --- | --- | --- |
| `english` | 0.58 / 0.134 | 0.73 / 0.112 | 1 |
| `multilingual` | 0.58 / 0.252 | 0.59 / 0.263 | 21 |
| `typed-decisions` | 0.65 / 0.140 | 0.71 / 0.130 | 0 |

### `support-v2-unseen` — 79 items asked, 0 refused

The 98 items no Lev adapter trained on, under the same partitions.
This is the suite the ledger points adapter confirmations at.

| Checkpoint | Calibration acc / ECE | Development acc / ECE | Confident errors |
| --- | --- | --- | --- |
| `english` | 0.50 / 0.215 | 0.69 / 0.197 | 1 |
| `multilingual` | 0.57 / 0.298 | 0.62 / 0.276 | 10 |
| `typed-decisions` | 0.62 / 0.100 | 0.64 / 0.128 | 0 |

### `external-v1` — 160 items asked, 0 refused

BoolQ and MultiNLI validation items nobody here labelled; the one
score our own authorship cannot have fitted.

| Checkpoint | Calibration acc / ECE | Development acc / ECE | Confident errors |
| --- | --- | --- | --- |
| `english` | 0.80 / 0.125 | 0.80 / 0.112 | 17 |
| `multilingual` | 0.78 / 0.131 | 0.72 / 0.230 | 21 |
| `typed-decisions` | 0.80 / 0.048 | 0.80 / 0.056 | 1 |

## What the runs show

- `typed-decisions` is the strongest checkpoint on every suite and is
  genuinely calibrated on `external-v1` (ECE 0.048–0.056, one confident
  error in 160 items).
- `multilingual` arrives uncalibrated — raw ECE 0.23–0.30 everywhere,
  the highest confident-error counts — consistent with upstream
  shipping it with unfitted temperatures. Its fitted maps recover
  well where enough calibration items exist (routing ECE 0.270 to
  0.029, boolq 0.254 to 0.044, mnli 0.206 to 0.043), so the door
  should be served behind a fitted map or not trusted for
  probabilities.
- `english` is middling in-domain and decent out-of-domain; its
  routing map *worsened* log loss (0.506 to 0.578), so its raw
  probabilities stand as shipped.
- Every run scored every asked item: no refusals, no harness losses.
  The `urgency` and `severity` families are too small on the unseen
  suite for a map verdict (8–24 fitted items against a floor of 30).

## What this does not claim

- No locked-partition reads. Both suites kept their locked items
  unasked; a locked read is a one-time spend, not a baseline.
- No GPU numbers. CPU fp32 is the parity path; `--device metal` exists
  but is unmeasured and unrecorded.
- The conformance corpus pins code paths, not model behavior at scale.
