# Text 029 independent CPU reference

The single 359-token case runs the same pinned official Transformers operators and independently decoded original Q4 GGUF values in CPU f32, one layer at a time. It uses the official external head weights in CPU f32. It preserves all 64 intermediate arrays outside Git. The layer loop, final RMSNorm, and official head forward are byte-identical to the earlier successful text 014 CPU reference.

The reference completed in 75.081 seconds and peaked at 3,186,196,480 bytes. Bounds were 12 GiB process group, 24 GiB starting available memory, 12 GiB available-memory floor, two CPU threads, 300 seconds, and a 25 GiB free-disk floor. It used no GPU. Source and GGUF file identity checks passed; owned-child cleanup succeeded. This is correctness evidence, not a serving latency measurement.

The CPU-to-MPS screen passes: final hidden relative L2 is 0.0000305640; maximum raw-logit difference is 0.0000400543; maximum probability difference is 0.0000073278; all three top answers match. The bounds were frozen before execution (relative L2 1e-4, logits 1e-3, probabilities 1e-4, all tops).

Both independent CPU and MPS references choose false for `日本1`, while the saved normal-request Stage 6 native result chooses true. CPU-to-native maximum probability difference is 0.0090604. This excludes the accelerated MPS oracle as the explanation for this case. It does not yet identify the native operator responsible. The full 40-request, 104-question gate still requires all top answers and maximum probability difference 0.02; this one-case diagnostic does not replace that gate.

The receipt includes all 64 intermediate hashes, token IDs, typed spans and options, all three head outputs, the unchanged same-GGUF identity, and separate comparisons against the staged and fused-field Stage 6 responses. Raw arrays, weights, executables, and memory samples remain outside Git. Original and path-substituted proof hashes are in the manifest. Shared dependencies are in the preceding same-Q4 publication.
