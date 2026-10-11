# Text 029 layer 35 replay

The unchanged CPU layer exactly reproduced the saved CPU output. Feeding the native input into that same f32 layer reproduced most of the accumulated difference. Modeling the native half-precision boundaries reduced the remaining difference from native Metal, but this one-layer diagnostic does not identify the earlier source of drift or establish that every native operator is correct.

The separate 40-request, 104-question runtime acceptance result remains 103 of 104 matching top answers. Its all-top requirement and 0.02 probability tolerance are unchanged. This diagnostic does not execute the head or generate a new acceptance result.

## Compared executions

All three executions use layer 35 of the original, unchanged GGUF (SHA-256 `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`) and the same 359-token request. The native target is the archived Stage 6 binary (SHA-256 `746950810194cbb9228ab6c49776beb5bbb45415b831e5c94efb821ad2dde566`) with its original 32-key attention update. The newer attention candidate is not this diagnostic's baseline.

- A: saved independent CPU layer-34 rows, unchanged official f32 operators. Layer-35 output matches the saved independent CPU output bit for bit.
- B: saved native layer-34 rows, the same official f32 operators and original decoded weights. B minus A measures propagation of incoming differences.
- C: the same native input as B, with seven linear weights and their inputs rounded through f16, scaled rotary Q and K/V rounded through f16, and the archived 32-key online attention exponential rounding and denominator tree. C minus B measures these combined arithmetic changes. Only the in-memory copies used by C change; artifact files do not.

## Results

Relative L2 error uses the named comparison target's norm. Whole-layer and false-option-span results are both retained; the latter covers tokens 229 through 246.

| Variant | Whole layer versus native | False span versus native | Whole layer versus independent CPU |
| --- | ---: | ---: | ---: |
| A | 0.0016429722 | 0.0061012703 | 0 |
| B | 0.0001870927 | 0.0002406901 | 0.0016315152 |
| C | 0.0000740661 | 0.0000917764 | 0.0016432386 |

B minus A has relative L2 error 0.0040208 at the mixer output and 0.0037227 at the FFN output. C minus B has relative L2 error 0.0001871449 at the layer output. These measurements show substantial propagation of incoming error at this layer and a smaller local arithmetic contribution. They do not justify attributing the whole-model top-answer difference to this layer, one projection, or a single precision choice.

C is not a bitwise Metal emulator: CPU GEMM and RMSNorm reduction order, transcendental implementations, fused multiply-add behavior, and residual addition after GEMM differ from Metal's initial residual accumulator. Its smaller distance from native is diagnostic evidence, not a new acceptance criterion.

## Execution and evidence

The CPU-only guard completed in 10.513 seconds, with peak accounted memory 3,707,338,752 bytes. Its limit was 12 GiB, with a 24 GiB starting requirement and 12 GiB available-memory floor, two CPU threads, a 120-second deadline, and a 25 GiB free-disk floor. The minimum observed available memory was 99,697,229,824 bytes. No GPU, full trunk, model server, build, desktop app, or system service was used. The owned child group stopped and both leases were released. These elapsed times describe this diagnostic, not inference performance.

The receipt includes 25 checkpoints each for A and B and 29 for C, their shapes and hashes, all 50 shared-checkpoint comparisons, parameter hashes, fixture results, and input/artifact checks. The guard rechecked all source and input hashes after execution. Raw tensor checkpoints remain outside Git. Parent evidence bundles contain the preceding full CPU/native captures and full-head observer equivalence check.

The manifest records original and published hashes. Only account-specific absolute path prefixes are replaced with named tokens; exact reversal was checked using the private path map retained in scratch. No numerical receipt rows are omitted. Sources and the plan are included for audit; a reproduction must resolve the named path tokens and obtain the separately stored artifacts and arrays.
