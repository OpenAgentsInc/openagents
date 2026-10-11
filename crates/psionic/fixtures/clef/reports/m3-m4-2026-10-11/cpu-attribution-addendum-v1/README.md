# Case 014: FFN precision control and native CPU comparison

Changing only the 192 FFN gate, up, and down matrices from their GGUF values to the original BF16 values cast to f32 restores the original answer on case 014. Every other GGUF backbone matrix, both embedding tables, and the official CPU-f32 operators remain unchanged. The stored-negative-A adapter is the same as in the independent GGUF control.

| Result | Maximum probability difference | Same answer |
| --- | ---: | :---: |
| Original FFN control versus original BF16-MPS | 0.018644072 | Yes |
| Original FFN control versus original checkpoint-f32 | 0.014662503 | Yes |
| Original full GGUF control versus original BF16-MPS | 0.091617645 | No |

This control recovers this case within the 0.02 probability bound. It is **not acceptance of the Q4 artifact**, proof that Q6 or Q8 would suffice, or evidence for the full M4 suite. All 192 FFN matrices changed together, so it does not identify particular layers to replace. The receipt names every substituted matrix, its shard, original GGUF type, source dtype, and execution dtype.

The full pass took 49.90 seconds and peaked at 2.863 GiB under the same 12 GiB process-group cap, 24 GiB startup minimum, 12 GiB available-memory floor, two CPU threads, and 30-minute deadline. Every layer output was finite; layer and head inputs remained unchanged. The original quantized head result was reproduced exactly as a control. The task-owned process group stopped successfully.

## Native CPU agreement

The native CPU dump also closely matches the independent official-operator GGUF run over all 64 layers. Final normalized rows have relative L2 error 0.000010538, mean token cosine 0.999999999947, and maximum absolute error 0.00153923 across 1,643,520 values. Raw residual maximum error first exceeds 0.001 at layer 38 and 0.01 at layer 54; it never exceeds 0.1. `native-cpu-layer-errors.csv` includes all 64 residual outputs and the final norm. The JSON comparison includes every source file hash.

These measurements explain this case's accuracy difference without changing the original-BF16 acceptance gate. This bundle contains no weights or tensor dumps. See the base controlled-comparison bundle for the two original controls, importer fixtures, token identities, and cumulative quantization-error table. `retained-hashes.json` preserves original and sanitized-copy hashes; local directory roots use placeholders. No GPU or model runs are needed to inspect these results.
