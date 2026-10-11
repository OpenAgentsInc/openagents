# Text 029 layer 0 precision controls

The independent f32 baseline reproduced the saved CPU layer-0 output bit for bit. Rounding both the linear weights and their inputs through f16 brought that layer output about 11.2 times closer to the archived native Metal output. Changing only the official Delta calculation from chunked to recurrent had little effect. This supports a precision explanation for much of the initial perturbation; it does not establish the cause of the full-model answer change or prove every native operator correct.

The frozen 40-request, 104-question runtime gate remains unchanged and unmet: the previous native result matched 103 of 104 top answers. This diagnostic does not execute the head or revise that result.

## Fixed controls

All six variants use layer 0 of the original Q4 GGUF (SHA-256 `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`) and the same 359 text-029 token IDs. Input embedding rows are reconstructed through the verified independent decoder. The native target is the original Stage 6 test binary (SHA-256 `746950810194cbb9228ab6c49776beb5bbb45415b831e5c94efb821ad2dde566`). Its incoming embedding rows were not captured separately: input equality is based on the verified IDs, decoder evidence, and source, not a direct native-input byte comparison.

Each variant loads fresh original weights. Only `nn.Linear` weights or their inputs are rounded in the named controls. Convolution, normalization parameters, residual rows, Delta preparation, and recurrent state stay f32. The recurrent controls call the independent official operator; they do not port Metal's kernel or match its dot-product/FMA order.

| Variant | Weight rounding | Input rounding | Delta | Whole layer relative L2 versus native | False-option span relative L2 versus native |
| --- | --- | --- | --- | ---: | ---: |
| A | None | None | Chunk64 | 9.054776e-5 | 9.183608e-5 |
| W | f16 | None | Chunk64 | 8.061205e-5 | 8.448810e-5 |
| X | None | f16 | Chunk64 | 5.086797e-5 | 3.895739e-5 |
| WX | f16 | f16 | Chunk64 | 8.106057e-6 | 5.096058e-6 |
| R | None | None | Recurrent | 9.054859e-5 | 9.180863e-5 |
| WXR | f16 | f16 | Recurrent | 7.622964e-6 | 5.052221e-6 |

R minus A has whole-layer relative L2 difference 1.187734e-7. Both half boundaries matter in this experiment, and their combination is much closer to native than either alone. The values do not isolate a single projection or justify an alpha/beta-only product patch. The remaining error includes differences not modeled here: GEMM and RMSNorm reductions, transcendental functions, fused multiply-add, native state-output reduction, and residual accumulation inside GEMM.

A later full independent pass with the explicit native half boundaries could test whether this local perturbation predicts the saved layer trajectory and answer change. Such a control would remain diagnostic; it would not replace the independent f32 acceptance oracle.

## Checks and limits

The source and predictions were frozen before execution. The guard completed once in 27.766 seconds with peak accounted memory 3,884,875,776 bytes and minimum observed available memory 97,004,666,880 bytes. The limits were 12 GiB for the owned group, a 24 GiB starting requirement, a 12 GiB available-memory floor, two threads, a 120-second deadline, a 2 GiB disk lease, and a 25 GiB free-disk floor. These times describe a correctness diagnostic, not inference latency.

Six small fixtures checked both official Delta operators against an independent f64 recurrence, including grouped heads, nonzero initial state, zero q/k, extreme decay, and lengths 1, 17, and 65. All output/state checks passed. Every variant preserved input and parameter bytes. The guard rechecked source/input hashes and the verified GGUF file state after execution. Its owned child group stopped and both leases were released. No GPU, model server, build, quiet lease, desktop app, or system service was used.

The receipt contains all six results, 186 checkpoint hashes and shapes, and 248 comparisons. Raw checkpoints total 1,599,806,976 bytes and remain outside Git. No weights or binaries are included. The original receipts and arrays remain in scratch. The manifest records original and published hashes; account-specific absolute path prefixes are replaced with named tokens and exact reversal was verified. No numerical receipt rows were removed.
