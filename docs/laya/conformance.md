# Laya conformance

How the Rust port is held to the Python reference, what the fixtures pin,
and the one upstream versioning detail the port had to take a side on.

## The fixture protocol

`crates/laya/fixtures/gen_fixtures.py` runs inside the reference's
environment (torch + transformers 4.57.6, CPU, fp32) and emits three
files per checkpoint:

- `requests-<checkpoint>.json` — TypeSafe-shaped request bodies beside
  the reference's full `system_one` answers.
- `sequences-<checkpoint>.json` — `build_sequence` output: input ids and
  marker positions per question.
- `manifest-<checkpoint>.json` — SHA-256 and byte size of every file the
  port loads, so a run proves it read the same bytes the goldens came
  from.

Four request cases per checkpoint: a mixed three-question support call, a
structured (non-string) state, choice questions across the temperature
buckets, and noul defaults. The corpus is small on purpose — it pins
every code path, not a quality claim. Classification quality is a
workload measurement and belongs under `measurements/` when it exists.

The test is `crates/laya/tests/conformance.rs`; it runs unconditionally
and skips the weight-backed half unless `LAYA_BUNDLE_DIR` names a bundle
directory.

## What is pinned

- **Sequence encoding, exact.** Input ids and marker positions must equal
  the reference's — no tolerance. This holds on all three checkpoints,
  including the mmBERT tokenizer and its different special-token set.
- **Answers, at reported precision.** Probabilities, scores, and
  confidences must land within 0.001 of the fixture; confidence and
  `act_probability` get 0.01 because saturated distributions amplify
  tail-mass differences between fp32 attention implementations. `model`
  and `usage` must equal the fixture.

## Observed

On 2026-09-22 (this machine, CPU, fp32, `cargo test -p laya --test
conformance -- --nocapture`):

| Checkpoint | Sequence parity | Max answer |delta| |
| --- | --- | --- |
| `english` | exact | 0.000000 |
| `multilingual` | exact | 0.000000 |
| `typed-decisions` | exact | 0.000000 |

Every reported field is bit-identical to the fixture's four-decimal
values — the tolerances are headroom for other hosts, not room the port
uses today.

## The RoPE config caveat

The checkpoints were saved under transformers 5.x, which writes RoPE
thetas per layer type under a nested `rope_parameters` block. The
reference runs transformers 4.57.6, where `ModernBertConfig` reads only
the flat `global_rope_theta`/`local_rope_theta` fields — the nested block
parses as an opaque attribute and never reaches the attention layers.

This is observable, not academic: `multilingual`'s nested block declares
`sliding_attention.rope_theta = 160000`, but the reference effectively
runs its sliding layers at the ModernBERT default `10000`. A port that
honored the declared block would diverge from the reference's answers —
the first version of this one did, by several logit points.

The port mirrors the reference runtime: flat fields, the `rope_theta`
alias, then ModernBERT defaults; the nested block is not consulted
(`EncoderConfig::global_theta`/`local_theta`). If the reference ever
moves to transformers 5.x, both sides change semantics together and this
note is where to start.
