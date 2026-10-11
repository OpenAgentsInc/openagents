# Case 014: controlled backbone comparison

The quantized checkpoint changes this case's answer when executed with the same official CPU-f32 operators as the original checkpoint. The native Metal result closely matches the independent quantized result.

| Comparison | Maximum probability difference | Same answer |
| --- | ---: | :---: |
| Independent GGUF-f32 versus native Metal | 0.000163851 | Yes |
| Original checkpoint-f32 versus original BF16-MPS | 0.008714512 | Yes |
| Independent GGUF-f32 versus original checkpoint-f32 | 0.087636076 | No |
| Quantized hidden rows + original lexical rows versus original-f32 | 0.087903695 | No |
| Original hidden rows + Q6 lexical rows versus original-f32 | 0.000349884 | Yes |

Both controls use the exact same 321 token IDs, question spans, options, official f32 head, and official Transformers 5.10.2 CPU operators. Each holds one decoder layer at a time. The GGUF control reverses the documented value-head permutation, reconstructs ordinary norm parameters exactly, and substitutes stored negative-A values directly for the original expression. The original-checkpoint control uses its unmodified A_log and norm parameters. Its 12 shard file states match the prior verified BF16 reference; the run does not repeat full-file hashing.

The guards allow 12 GiB for the task-owned process group, require 24 GiB available at startup, maintain a 12 GiB floor, and limit CPU work to two threads and 30 minutes. Actual peaks were 2.95 GiB and 2.56 GiB. Both full passes completed with all 64 layer outputs finite, all layer inputs unchanged, and their child groups stopped. No GPU, quiet lease, desktop application, or system service was used or controlled by these runs.

## Layer error growth

`layer-error-growth.csv` compares all 64 saved residual outputs, before the final norm. A difference is already visible after layer 0: relative L2 error 0.01059, mean token cosine 0.99995685. The largest increases in cumulative relative L2 error occur at layers 62 (+0.02304), 50 (+0.01826), 59 (+0.00999), 60 (+0.00993), and 35 (+0.00919). Layer 0 starts at +0.01059 from the different quantized input embedding and first decoder layer.

These are cumulative differences, not proof that those layers caused the increase. Relative error can fall when a residual's norm changes; layer 63 illustrates this. Final normalized rows have relative L2 error 0.23345. Use controlled matrix or layer substitutions before deciding which weights need higher precision.

## Scope and files

This is one diagnostic case, not the full M4 acceptance suite. The result isolates the quantized backbone as the cause of this case's large answer gap; it does not isolate Q4 FFN weights from quantized embeddings and Q8 projections. The strict original-BF16 acceptance gate remains separate.

`summary.json` contains the comparisons, bounds, and selected layer statistics. Full per-layer hashes and checks are in each `results/receipt.json`; guards and lease receipts describe execution. Importer fixtures cover exact stored dense transforms, sampled projection row conventions, all 848 original layer parameter names/shapes, and 384,000 original lexical values. Scripts describe the bounded readers and execution. The independent decoder's source and pinned revision are in `independent-decoder-source.json`.

This bundle excludes weights, raw tensor dumps, and process memory sample streams. The originals remain in the experiment directory. `retained-hashes.json` records each original and publication-copy hash. Local directory roots in publication copies use placeholders, so the scripts require those roots to be restored before reproduction.
