# Laya port baseline

**Date:** 2026-09-22 · **Host:** Apple M5 Max, macOS 26.4 · **Device:**
CPU, fp32 throughout (the reference keeps modules fp32 and only autocasts
on CUDA) · **Build:** `cargo build --release -p laya`

The first run record for the Rust port: conformance against the Python
reference, then the cost of serving it. Classification quality on our own
question suites is not measured here — nothing in this record transfers
an upstream benchmark claim to a local door.

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

## What this does not claim

- No accuracy, calibration, or coverage numbers on real workloads. The
  multilingual checkpoint ships unfitted temperatures; the other two
  ship temperatures fitted on upstream's own dev data. Treat every
  probability as unmeasured locally until a gym suite scores it.
- No GPU numbers. CPU fp32 is the parity path; `--device metal` exists
  but is unmeasured and unrecorded.
- The conformance corpus pins code paths, not model behavior at scale.
