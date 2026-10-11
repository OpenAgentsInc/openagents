# Exact-input llama.cpp third-runtime diagnostic

This is one fixed text-029 diagnostic using the original same-Q4 GGUF and all 359 exact token IDs, spans, and options. It does not replace the frozen Torch reference, change the all 104-top/0.02 probability gate, or close M4.

The build uses public commit `62a6f74bab748b1755f4cd8dfc0871caf865c6c8`, not M1's `b11538` tag (`79e2e74eb11022c1ba2e438df7f0ca2d4c10f8b6`). Static CPU execution disables GPU backends, dynamic plugins, BLAS, Accelerate, repacking, KleidiAI, and llamafile. It verifies Q4_K uses Q8_K RHS traits and requires 48 fused CPU Delta graph operators and no Flash Attention. Its arithmetic differs from both native f16 and Torch f32.

The upstream graph evaluates an embedded quantized head. Those logits are labeled separately and excluded from head-equivalent comparisons. The saved-row replay instead uses the unchanged official head loaded as CPU f32 and identical GGUF lexical rows; it does not repeat backbone RMSNorm on `result_norm`.

## Stage results

All stages executed successfully, but third-runtime numerical agreement failed: its probabilities substantially differ from both the independent CPU and native. Matching native's top answers does not establish native correctness or resolve the frozen Torch acceptance failure.

| Stage | Status | Seconds | Peak accounted bytes | Minimum available bytes |
| --- | --- | ---: | ---: | ---: |
| build | complete | 38.239 | 635,076,608 | 87,618,650,112 |
| capture | complete | 135.946 | 19,967,180,800 | 71,104,282,624 |
| replay | complete | 10.681 | 1,241,251,840 | 77,547,601,920 |

All 64 layer comparisons, all 3 question logits/probabilities, and the separate native observer replay screen are preserved in `replay/receipt.json`. The A saved-head replay was bitwise exact.

| Question | CPU A P(true), top | Native observer P(true), top | Llama trunk + external head P(true), top |
| --- | ---: | ---: | ---: |
| rate | 0.334822535289 (false) | 0.335898647593 (false) | 0.337947087127 (false) |
| 日本1 | 0.493030686569 (false) | 0.502091268921 (true) | 0.818549422592 (true) |
| rollback_2 | 0.008845312692 (false) | 0.008821183319 (false) | 0.008294968078 (false) |

Whole final-memory relative L2: llama versus CPU 0.1603904893507589, llama versus native 0.15926588669275862. Full metrics and span trajectories remain in the receipt.

## Limits and identity

Build limits were 8 GiB/start20/floor12/600s; capture 32 GiB/start44/floor12/600s; replay 4 GiB/start16/floor12/120s. Each uses two CPU threads, a 2 GB disk lease, 2 GiB output cap, and 25 GiB free-disk floor. Guards are sampled memory limits, not hard allocation limits. The sequence has no retries or raised bounds. Owned groups were cleaned up; no GPU, quiet lease, desktop app, or system service was used or controlled.

The original GGUF SHA-256 is `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`; its previously verified file snapshot is checked before and after, without rehashing 19 GB. Build-to-capture-to-replay source descriptors and binary hashes remain linked. Exact native IDs were reconstructed later from frozen production encoder sources; Stage6 HTTP itself did not export IDs.

The publication manifest records original and path-sanitized hashes with reversible path tokens. Complete receipts are preserved. Raw arrays, model weights, binaries, and raw process-memory samples stay in scratch. This third runtime can help attribute implementation differences; it cannot by itself establish which remaining arithmetic difference causes the original top mismatch.
