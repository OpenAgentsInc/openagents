# Granite 4.2 8B prefill on Metal (2026-11-04)

This record covers issue #12881. The decoder used to read a prompt eight
tokens at a time with matrix-vector products. A prompt past eight tokens now
runs in calls of up to 512 tokens:

- **Products:** each weight matrix is widened into BF16 pairs (`gr_widen_q4k`,
  `gr_widen_q6k`, `gr_widen_q8`). Word `r*cols+k` holds the weight rounded to
  BF16 and the rest, rounded to BF16, so each weight keeps about 16 bits. The
  inputs are paired to match (`gr_pair`), and one Metal 4 tensor-op tile
  (`gr_tile`, `am_matmul_bf16`, 64 tokens by 128 rows) adds both halves.
- **Attention:** simdgroup-matrix tiles of 64 tokens with the causal mask
  (`gr_att_scores`, `gr_att_softmax`, `gr_att_values`). A score tile that no
  token sees is skipped.

Decoding a token is unchanged. The code is in
`psionic/lib/granite/kernels.coax` and `psionic/platform/granite-metal.c`.

- **Machine:** Apple M5 Max, 40-core GPU, macOS 26.4. Other agents were using
  the GPU throughout, so each speed below varies by up to 1.5x between runs.
- **Files:** IBM's GGUF revision `93f3f6a8938e` of `ibm-granite/granite-4.2-8b`,
  `granite-4.2-8b-Q4_K_M.gguf` and `granite-4.2-8b-Q8_0.gguf`, in
  `~/models/psionic/granite-4.2-8b-a7042144125c/gguf-93f3f6a8938e/`.
- **Reference:** Homebrew llama.cpp b10330 (`687e77892`), Metal backend.

## Parity

`granite_run compare` runs the chat render of "Move the oak tree" (19 tokens)
in one call, so it takes the prefill path. It compares Metal against its CPU
repeat, and the one call against the same Metal decoder fed a token at a
time. Distance is `max |a - b| / (1 + max |b|)` over every layer's residual
stream or the 100,352 logits. The gate fails above 1e-3 or when a greedy
token differs.

| Check | Q4_K_M | Q8_0 |
|---|---|---|
| One call against a token at a time: residual | 1.2e-5 | 2.0e-5 |
| One call against a token at a time: logits | 1.4e-4 | 1.6e-4 |
| Metal against CPU: residual | 1.9e-5 | 1.8e-5 |
| Metal against CPU: logits | 1.3e-4 | 1.9e-4 |
| Greedy tokens (2 steps, and the next token) | equal | equal |

The llama.cpp parity in `psionic/tests/check_granite.sh` now also takes the
prefill path: Psionic dumps the 28-token prompt in one call, and llama.cpp
feeds it a token at a time.

| Check | Q4_K_M | Q8_0 |
|---|---|---|
| Worst layer, relative RMS | 5.3e-4 (layer 39) | 5.7e-4 (layer 39) |
| Prompt logits, relative RMS | 4.8e-4 | 4.2e-4 |
| Greedy continuation | 16 of 16 equal | 16 of 16 equal |

The earlier record (`granite-4.2-8b-2026-11-04/`) measured 5.7e-4 and 6.1e-4
for the worst layer with the prompt fed a token at a time.

With the low half dropped (plain BF16 weights), the one call differs from
the token-at-a-time decoder by 8.7e-3 (residual) and 2.1e-3 (logits) on
Q4_K_M. That fails the 1e-3 bound, so the pairs stay.

`psionic/tests/granite-kernel-checks.coax` checks the new kernels against
binary64: every widened word for all three formats, a 130-row by 66-token
product, and 70 tokens of attention with a skipped tile.

## Speed

`bench-prefill` evaluates a 512-token prompt (seeded random IDs from 1,000 to
90,000, as `llama-bench` uses random tokens) from an empty cache, three times
after one warm-up, and reports the best run. Each round runs Psionic, then
`llama-bench -m FILE -p 512 -n 0 -r 3`, then Psionic again.

| File | Psionic, tok/s | llama.cpp pp512, tok/s | Ratio |
|---|---|---|---|
| Q4_K_M | 264.2, then 246.3 | 1468.6 ± 266.4 | 0.17 |
| Q8_0 | 288.0, then 235.0 | 1249.2 ± 383.1 | 0.21 |

On other runs today, llama.cpp measured 1387.6, 1369.6 and 1136.5 tok/s on
Q4_K_M. Half of llama.cpp is about 700 tok/s, so this change does not reach
the target. The rest is split into #12894.

Before and after on the same command (`granite_run generate`, 128-token
prompt, first call in the process). The baseline is the runner built from
`main` before this change:

| File | Before, tok/s | After, tok/s |
|---|---|---|
| Q4_K_M | 24.4 | 150.2 |
| Q8_0 | 15.7 | 114.7 |

On a quiet GPU (`GRANITE_PROFILE=1`, 512 tokens on Q4_K_M), the tensor-op
tiles take 1.72 s of 2.0 s of GPU time. Widening takes 0.21 s, attention
0.08 s, and the rest 0.08 s. The tiles do twice the work of the model's
products (8.2 TFLOP for 512 tokens) because of the pairs, at about
9.5 TFLOP/s.

## Why half isn't reached

llama.cpp's batched prefill rounds weights and activations to f16
(the earlier record measured 3.7e-2 relative RMS against the f32 path on
Q4_K_M). Rounding at that level fails the 1e-3 bound here. The only tensor op
in Coax today is BF16 weights times f32 activations, so the 16-bit weights
cost two products. Closing the gap needs a tensor op that keeps the bound at
the hardware's half-precision rate, for example BF16 or f16 inputs split into
high and low halves, fused with the widening. That needs a new primitive in
`coax/lib/compute/apple.coax` and the Coax Metal emitter.

## Granite 4.2 30B

`granite-4.2-30b-Q4_K_M.gguf` (64 layers) loads with the same code and
answers "What is the capital of France? Answer in one sentence." with "The
capital of France is Paris." The prefill speed wasn't measured.

## Reproduce

```
psionic/tests/check_granite.sh
granite_run FILE.gguf k.metal bench-prefill --ids ID,ID,... --steps 3 --capacity 1024
llama-bench -m FILE.gguf -p 512 -n 0 -r 3
```

`check_granite.sh` builds `granite-run` and `k.metal` and runs the compare,
the generate and tool checks, and the llama.cpp parity.
