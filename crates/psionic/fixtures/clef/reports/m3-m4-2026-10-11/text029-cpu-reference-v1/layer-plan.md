# Text 029 layer capture plan

The completed CPU head replay shows that replacing the false-option pooled context for question `日本1` is enough to change its top answer. The next diagnostic locates the first material difference before the head. It changes no weights, questions, thresholds, or product source.

## Independent CPU reference

`cpu_reference.py` independently decodes the pinned original Q4 GGUF and executes the pinned official Transformers operators in CPU f32. It reads the exact 359 token IDs from the previously verified native encoding and checks them against the official encoder and the saved MPS run. It keeps the same stored-negative-A adapter. The CPU layer loop, final RMSNorm, and official head forward are byte-identical to the successful text 014 CPU reference; `cpu-source-check.json` records that check. Only the case selection and diagnostic comparisons change.

One layer stays resident at a time. The script writes all 64 intermediate arrays as it goes, then final rows and the three unchanged head outputs. It compares final rows and logits with the saved independent MPS text 029. It reports the previously frozen CPU/MPS screen (relative L2 1e-4, raw logits 1e-3, probabilities 1e-4, all tops) separately from execution completion. This screen is not the full runtime acceptance gate.

Bounds: memory lease 12 GiB, start 24 GiB, available-memory floor 12 GiB, disk lease 2 GiB with 25 GiB floor, two CPU threads, 300 seconds, no GPU or build lease. The prior 321-token pass took 141.103 seconds and peaked at 3.164 GB. This 359-token pass is estimated at 150–180 seconds, excluding any build contention. It writes about 0.5 GB. It may overlap the parent's 24 GiB build for correctness, but not a timed microprobe.

## Archived native Metal capture

`native_layers.py` invokes only the verified Stage 6 `psionic-serve-tests` executable (SHA-256 `746950810194cbb9228ab6c49776beb5bbb45415b831e5c94efb821ad2dde566`). It never uses the current target directory or current backend shader. The archived source manifest identifies the old 32-key fused attention implementation. The selected test is exactly `clef::tests::dump_layer_rows`, with Metal selected, chunk 512, fused TensorOps attention, weight-shapes GEMM, sequential DeltaNet, Q4 FFN weights on demand, and prefix caching off.

The test loads the embedded head but does not execute it; all observer rows precede the head hidden LayerNorm. The single 359-token input fits one chunk. The observer introduces per-layer completion waits and CPU readback, so this is a correctness capture only. The next CPU analysis must compare its final rows and head results with the saved normal-request native pooled rows and raw logits before treating observer dispatch as equivalent.

Bounds: GPU lease, memory lease 52 GiB, start 64 GiB, available-memory floor 12 GiB, disk lease 2 GiB with 25 GiB floor, 600 seconds. It writes exactly 65 arrays of 7,352,320 bytes each. The Rust observer retains about 449 MiB of intermediate rows until it writes them. All files must have the expected dimensions and finite values. Run only after both the independent CPU pass and the parent's build stop, and after the parent coordinates an exclusive task-owned GPU window. No quiet lease or application/service changes.

## Comparison and limits

The saved full MPS run has per-layer hashes, not per-layer arrays. They cannot establish a numerical first-divergence location. The new CPU arrays permit a direct all-64-layer comparison against native Metal. The final CPU-to-MPS comparison checks whether the accelerated oracle itself explains the discrepancy. A material difference must be reported with its layer type, token rows, norms, and option-span context; it must not be attributed to a particular kernel from a final answer alone.

The unchanged full-runtime gate remains 40 requests, 104 questions, identical IDs/options, all top answers, and maximum probability difference 0.02. This diagnostic cannot replace that suite. The existing native request was observed in Stage 6; exact IDs were reconstructed from matching encoder sources and exact request bytes afterward, not exported by its old HTTP process.

`guard_layers.py` freezes executed source/input hashes before and after the run, rechecks the historical GGUF file state, strips inherited inference overrides, monitors only its child process group, and cleans up only that group. Outputs are exclusive and failures remain in place. No retry or raised bound is automatic. Both capture commands still require the parent's source review and execution handoff.
