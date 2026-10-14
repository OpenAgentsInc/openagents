# Calibrating decision checks for Clef-Flash (2026-10-14)

This note records where the limits in `crates/pylon/src/verify.rs`
(`CLEF_FLASH`) come from. They decide when a pylon's decision answers pass
a check, lose a round, or strike. The method is REAX's (see
[reax.md](reax.md)): measure the honest noise between machines and
backends, measure the cheapest substitutes, and put the limits between
them. The numbers are ours. Issue:
[#11632](https://github.com/OpenAgentsInc/openagents/issues/11632).

## What was measured

**The distance.** For two probability vectors over the same options,

```text
d(p, q) = max_i | (ln p_i - mean_j ln p_j) - (ln q_i - mean_j ln q_j) |
```

with probabilities floored at 1e-6, in natural-log units. A noul is
compared as `[p(true), p(false)]`, and options are matched by key. A
round's statistic is the mean `d` over its verified items (all 8).

**The items.**

- 400 items from the seeded generator (`crates/pylon/src/items.rs`, seed
  20261014): support messages, code-review notes, calendar requests,
  expenses and agent reports, each with one `choice` (2–8 options), `noul`
  or `score` (3–5 levels) question.
- The 40 hand-made end-to-end requests (104 questions) already in
  `crates/psionic/fixtures/clef/e2e/`, which carry Psionic CPU, CUDA (f16
  and f32 accumulation), llama.cpp, and the full-precision Hugging Face
  reference.

**The model.** Clef-Flash Q4_K_M, `ggml-org/Clef-Flash-GGUF` revision
`6b4fe4c5`, sha256 `36261166…6868a2`, served by `psionic-openai-server`
(built 2026-10-12) on an Apple M5 Max with 128 GB. The machine was shared
with other agents' work (load average 40–66), so the times are upper
bounds.

**The tools.**

- `openagents pylon calibrate run` asks the items one at a time and writes
  JSON lines.
- `openagents pylon calibrate compare` prints the spread, top-answer
  agreement, and the share of 20,000 simulated rounds that pass or strike.
- The runs are checked in as `crates/pylon/data/calibration/*.jsonl` (the
  answers' probabilities only). `crates/pylon/tests/calibration.rs`
  recomputes everything below on every test run.

## Results

Distances against the reference run in each pair. "Rounds" are simulated
checker rounds under the chosen limits (8 verified items; lost above a
mean of 0.05; strike above a mean of 0.15 or any item above 0.5).

| Pair | Items | Median | p90 | p99 | Max | Top answer agrees | Rounds pass | Rounds strike |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Honest** | | | | | | | | |
| Metal, the same 400 items again | 400 | 0 | 0 | 0 | 0 | 100 % | 100 % | 0 |
| Metal with `--decision-accumulate f16` vs f32 | 400 | 0 | 0 | 0 | 0 | 100 % | 100 % | 0 |
| CPU vs Metal | 64 | 0.0004 | 0.0012 | 0.0023 | 0.0035 | 100 % | 100 % | 0 |
| CUDA f32 accumulation vs CPU (fixture) | 104 | 0.0007 | 0.0017 | 0.0025 | 0.0037 | n/a | 100 % | 0 |
| CUDA f16 accumulation vs CPU (fixture) | 104 | 0.0054 | 0.018 | 0.026 | 0.063 | n/a | 100 % | 0 |
| **Substitutes** | | | | | | | | |
| Q8_0 of the same weights (Metal) | 400 | 0.069 | 0.26 | 0.55 | 0.84 | 97.8 % | 3.6 % | 20 % |
| Full-precision Hugging Face reference (what an upstream bf16/f32 API answers) | 104 | 0.11 | 0.30 | 0.46 | 0.58 | n/a | 0.6 % | 36 % |
| llama.cpp b11538 on the same GGUF (its prompt differs on 21 of 37 requests) | 95 | 0.048 | 0.28 | 0.82 | 0.88 | n/a | 18 % | 30 % |
| Uniform answers; always the first option; the reference's own top choice as a one-hot | 400 each | | | | | | 0 | > 99 % |

## The limits chosen

| | Clef-Flash |
| --- | --- |
| Items per round / verified | 8 / 8 (drawn after every answer arrives) |
| Round lost if mean > | 0.05 |
| Strike if mean > | 0.15 |
| Strike if any item > | 0.5 |
| Self-check passes when | ≥ 99 % of simulated rounds pass and none strike |

Why these limits:

- **Honest pairs never lose a round.** The largest honest item is 0.063
  (CUDA with f16 accumulation), eight times below the item bound. Honest
  round means sit far below 0.05. Psionic's fixed-order kernels make Metal
  bit-reproducible, and the f32-accumulating backends agree to 0.004.
- **Substitutes almost never earn.**
  - Q8_0 of the same weights scores 3.6 % of rounds and strikes 20 %.
  - The full-precision model scores 0.6 % and strikes 36 %. So **relaying
    to an upstream bf16 or f32 API is caught**, even when it serves the
    "same" model: the pinned 4-bit artifact has its own fingerprint.
  - llama.cpp on the same file scores 18 % and strikes 30 %. It is another
    runtime with another prompt encoder, which a pylon may not substitute.
    It loses most rounds.
- **Model-free answers strike every round**, including a one-hot of the
  true top choice. Knowing the answer is not enough without the
  probabilities' shape.
- **Every item is verified.** On a Mac a reference run costs about 0.2 s.
  REAX's spot-check option (a secret 3 of 8) stays available through
  `Limits::verified` for a larger model.

Raising the mean bound to 0.08 would let Q8_0 score about 26 % of rounds.
That is why it is 0.05.

## Self-check

`openagents pylon selfcheck` answers the 128 reference decisions that ship
in the CLI (`crates/pylon/data/selfcheck/clef-flash-q4_k_m.json`, seed
20261015, measured on Metal) and applies the self-check rule:

- Metal, the server used for the reference: 128 items, every distance 0,
  pass.
- CPU: 12 items, median 0.0003, max 0.0012, pass.
- A fresh Psionic started by `openagents pylon start`: 64 items, every
  distance 0, pass, 5.0 decisions a second including the first requests.
- The Q8_0 run under the same rule passes 3.6 % of rounds and fails
  (asserted in `crates/pylon/tests/calibration.rs`).

## Not measured yet (follow-ups)

- **CUDA on generated items.** The CUDA numbers above are the 104 fixture
  questions, not the 400 generated items. On the RTX 4080 (or an H100), run:

  ```sh
  openagents pylon calibrate run --decide http://127.0.0.1:18096 --seed 20261014 --count 400 --out cuda.jsonl
  openagents pylon calibrate compare crates/pylon/data/calibration/metal-a.jsonl cuda.jsonl
  ```

  Check it in beside the others. If CUDA with f16 accumulation ever
  reaches an item near 0.5, raise the item bound rather than strike honest
  pylons.
- **Clef 27B.** It needs its own run and its own `Limits`.
- **A model distilled on the public generator.** Not measured. The
  generator is public, as REAX's is; the mitigation is a checker-private
  item pool, which we can add without changing the protocol.
- **Single hosts drifting more than their hardware type.** REAX found one
  RTX 4090 host that scored 93 % of rounds. The self-check exists for that
  host, so every machine runs it before serving.
