# Granite 4.2 8B on Metal: parity and speed (2026-11-04)

This record covers issue #12870, steps 4 and 5. It compares Psionic's
Granite decoder (`psionic/platform/granite-metal.c`, kernels in
`psionic/lib/granite/kernels.coax`) with llama.cpp on the same GGUF files.
llama.cpp is the reference only; Psionic does not link it.

- **Machine:** Apple M5 Max, 40-core GPU, macOS 26.4.
- **Files:** IBM's GGUF revision `93f3f6a8938e` of `ibm-granite/granite-4.2-8b`,
  `granite-4.2-8b-Q4_K_M.gguf` and `granite-4.2-8b-Q8_0.gguf`, checked
  against `SHA256SUMS` in `~/models/psionic/granite-4.2-8b-a7042144125c/`.
- **Reference:** Homebrew llama.cpp b10330 (`687e77892`), Metal backend, all
  layers on the GPU.
- **Reproduce:** `PSIONIC_GRANITE_PARITY=1 psionic/tests/check_granite.sh`.

## Parity

The prompt is the chat render of "Move the oak tree two meters to the left
of the well." (28 tokens). Both sides start from the same token IDs, which
Psionic's tokenizer produced. `psionic/tests/granite_llama.c` reads
llama.cpp's `l_out-N` tensors (each layer's residual stream) and logits, and
`psionic/tests/granite_parity.py` compares the two dumps. Relative RMS is
`||a - b|| / ||b||` over every value of a layer or of the 100,352 logits.

llama.cpp feeds the prompt one token at a time here, so both sides use f32
activations. Its batched prefill rounds activations and dequantized weights
to f16. Against that path, the last layer differs by 3.7e-2 relative RMS on
Q4_K_M and 2.0e-3 on Q8_0, and the greedy tokens still agree.

| Check | Q4_K_M | Q8_0 |
|---|---|---|
| Token IDs | 28 of 28 equal | 28 of 28 equal |
| Worst layer, relative RMS | 5.7e-4 (layer 39) | 6.1e-4 (layer 39) |
| Prompt logits, relative RMS | 5.7e-4 | 4.2e-4 |
| Prompt logits, KL(llama.cpp ‖ Psionic) | 4.8e-7 | 5.0e-7 |
| Top 5 tokens of the prompt | equal | equal |
| Greedy continuation | 16 of 16 equal | 16 of 16 equal |
| Worst step logits, relative RMS | 6.0e-4 | 6.4e-4 |

The gate fails above 5e-3 relative RMS or on any greedy difference. The
full per-layer and per-step numbers are in `parity-Q4_K_M.json` and
`parity-Q8_0.json`.

Psionic's Metal kernels also run against their CPU repeat (the same Coax
code compiled to C): the largest residual distance is 3.4e-6 (Q4_K_M) and
2.8e-6 (Q8_0), with the same greedy tokens. Every kernel matches binary64
sums in `psionic/tests/granite-kernel-checks.coax`.

## Decode speed

Each round runs Psionic's 64-token greedy decode, then `llama-bench -p 0
-n 64`, so both see the same load. Other agents were using the GPU (67%
device utilization before the run), so absolute numbers vary by about 2x
over the day.

| File | Psionic, tokens/s | llama.cpp tg64, tokens/s | Ratio |
|---|---|---|---|
| Q4_K_M | 12.67, 12.30, 12.76 | 19.74, 21.37, 19.93 | 0.62 |
| Q8_0 | 11.36, 11.22, 11.23 | 16.52, 16.07, 16.00 | 0.70 |

In a quieter period earlier the same day, Q4_K_M ran at 20.5–22.0 tokens/s
against llama.cpp's 44.4–44.8 (0.48).

Where the time goes (Q4_K_M, quiet GPU): the weight products take about
26 ms a token and everything else about 4 ms. The decode products reach
270–400 GB/s on a single tensor that stays in cache, but much less across
the real sequence of tensors.

Prefill is slow: 27 tokens take about 1.8 s (15 tokens/s), against about
560 tokens/s for llama.cpp's pp64. The batched products take 8 tokens at a
time and do not use simdgroup matrix operations.

## What's left

- Decode products across cold tensors, to reach llama.cpp's decode speed.
- A prefill path built on simdgroup matrices.
- Step 6: the Aethyrforge eval against gpt-oss-20b.
