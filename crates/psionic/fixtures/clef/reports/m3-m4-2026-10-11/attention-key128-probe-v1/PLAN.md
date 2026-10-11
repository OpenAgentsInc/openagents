# Fixed 128-key attention experiment

This is a source-only experiment. No compiler or GPU run has occurred. The candidate changes the existing fused attention kernel's key tile from 32 to 128, keeping 16 queries, 128 threads, and the complete original TensorOps and online-softmax code. The kernel name and comments also change. `manifest.json` records the original production source hash; `build.py` checks it before compiling. Product sources remain unchanged.

At a 16k prompt, the Flash attention stack's calculated key-loop count falls from 33,619,968 to 8,454,144. Each iteration contains two TensorOps calls and three threadgroup barriers. This is a loop-count calculation, not a predicted speedup. The wider tile can reduce occupancy or increase register pressure. It increases declared threadgroup scratch from 19,456 to 28,672 bytes. The harness reports each compiled pipeline's static scratch, maximum threads, and the device's threadgroup limit, and refuses unsupported dispatches. Those limits do not reveal achieved occupancy.

Both kernels use the same half Q/K/V tensors, half tile probabilities, float score/output accumulation, and float final output. The larger tile changes the locations of online-softmax rescaling and half-probability rounding. Therefore it does not promise bitwise equivalence or complete model parity.

## Fixed checks

- Twelve small cases cover deterministic random inputs, absolute positions around 32/128-key boundaries, query tails, grouped query heads, uniform attention, large increasing/decreasing score maxima, and large finite masked future values. Both kernels are compared on every output with an independent dense f64 implementation reading the stored half inputs. Every absolute error must be at most `0.001`, with no relative allowance.
- Each kernel repeats exactly on a 65-row request starting at absolute position 127. Chunks of 15, 17, 31, and 33 rows must satisfy the existing chunk comparison bound, `2e-5 + 5e-5 * abs(reference)`. These checks expose changed final key-tile sizes and nonaligned query starts.
- Six larger cases cover Flash's 16 query heads and 27B's 24 query heads, each with four KV heads and dimension 256 (`flash-layout.json` verifies the pinned Flash GGUF metadata; the staged official27B config declares24/4/256), at 1k, 4k, and 16k histories. The operation is the final chunk of at most 2,048 queries. Every output is compared with the unchanged 32-key kernel at absolute `0.001`. Six predetermined row/head pairs per shape each get an independent 256-channel f64 check for both kernels.
- Outputs are poisoned before every command. All outputs must be finite, inputs and immutable expected results keep their hashes, and prefix/suffix redzones stay unchanged. Each output has 16 query rows of suffix protection. Every timed output must match its own immutable, previously finite-checked output bit for bit after the command completes. A full `memcmp` checks every byte; a mismatch triggers detailed numerical diagnostics and failure. This avoids repeating a slow scalar error scan on identical results.

Benchmark mode has five fixed alternating baseline/candidate pairs per large case, for 60 timing rows. The 128 commands include correctness work; check-only mode has 68 commands. GPU timestamps and host command time are recorded separately. Input preparation, poisoning, CPU reference calculations, scans, and JSON output occur outside the recorded command interval. Those between-command checks can affect subsequent GPU behavior; this is a paired shared-machine kernel comparison, not sustained throughput or whole-model latency.

## Bounds and proposed commands

The largest explicit live-buffer calculation is 244,451,584 bytes (233.1 MiB). Explicit allocations are capped at 1 GiB, point-in-time Metal allocation at 1.5 GiB, and the complete owned process group at 2 GiB. The runtime requires 14 GiB available before launch and 12 GiB throughout. It has a 50-second internal deadline and a 60-second external deadline. A refusal or numerical failure ends the attempt; the plan has no automatic retry or larger bound. There are no weights, models, desktop controls, system setting changes, or quiet leases.

After source review and an explicit build handoff, these commands would create new output directories:

```sh
openagents lease build --keep-target-dir --receipt build-lease-1.json -- openagents lease memory --amount 4 --receipt build-memory-lease-1.json -- python3 guard.py build --output build-1
```

After a separate explicit GPU handoff:

```sh
openagents lease gpu --no-wait --receipt bench-gpu-lease-1.json -- openagents lease memory --amount 2 --no-wait --receipt bench-memory-lease-1.json -- python3 guard.py bench --output bench-1
python3 summarize.py bench-1
```

The build has a 4 GiB process-group cap, 16 GiB starting margin, 12 GiB floor, and 180-second deadline. The guard preserves source and executable hashes, memory samples, stdout, stderr, failures, and cleanup receipts. No command above has been executed. An improvement would still require same-input model raw-logit checks at the unchanged `0.001` bound, all top answers unchanged, and new paired whole-model measurements before product integration.
