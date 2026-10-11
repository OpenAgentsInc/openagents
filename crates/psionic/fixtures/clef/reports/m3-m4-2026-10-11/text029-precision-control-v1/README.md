# Full text029 precision controls

The fixed CPU diagnostic completed. It shows that projection precision can move the disputed decision across 0.5, but it does not reproduce every native operation or resolve the unchanged full-suite acceptance failure.

A is the saved independent CPU-f32 same-Q4 reference. B rounds every trunk `nn.Linear` weight and input through f16. C adds the reviewed original 32-key attention Q/K/V and half-exponential boundaries. Both retain the official chunk64 Delta operator, the same original GGUF, unchanged f32 head, exact 359 input tokens, question spans, option ordering, and lexical embeddings. Separate B=1 hidden states have no cache. This is one fixed diagnostic case, not a new acceptance reference.

## Results

The values below are P(true); P(false) is its complement. Complete logits, both probabilities, and tops are in `precision-2-summary-complete.json` and `receipt.json`.

| Question | A | B | C | Native staged | Native fused-fields |
| --- | ---: | ---: | ---: | ---: | ---: |
| rate | 0.334822535289 (false) | 0.335526207653 (false) | 0.336054548385 (false) | 0.335898448152 (false) | 0.335898394968 (false) |
| 日本1 | 0.493030686569 (false) | 0.505646920005 (true) | 0.499907143415 (false) | 0.502091090110 (true) | 0.502091105011 (true) |
| rollback_2 | 0.008845312692 (false) | 0.008826444273 (false) | 0.008824806017 (false) | 0.008821207292 (false) | 0.008821208334 (false) |

B matches the three native tops, with maximum probability difference 0.00355583 from native staged. C is closer in maximum probability difference (0.00218395), but its disputed P(true) remains below 0.5 and its top differs. Selecting B on that basis would confuse a diagnostic control with acceptance. Neither result changes the original 40-request, 104-question all-top gate, which remains 103/104.

Final backbone-normalized memory relative L2 errors follow. Each denominator is the right-hand reference. The question spans tokens [192,194), true option [205,223), and false option [229,247). `receipt.json` also retains all 64 pre-final-normalization layer rows, hashes, and whole/question/option comparisons; do not conflate those with normalized final memory.

| Comparison | Whole memory | Question | True option | False option |
| --- | ---: | ---: | ---: | ---: |
| B − A | 0.0047820684 | 0.0012889523 | 0.0012332294 | 0.0177904115 |
| C − A | 0.0036207032 | 0.0011709258 | 0.0021643161 | 0.0124979566 |
| C − B | 0.0026248171 | 0.0013522296 | 0.0016322503 | 0.0077644778 |
| B − native | 0.0053409654 | 0.0011850633 | 0.0019622917 | 0.0198804965 |
| C − native | 0.0055026626 | 0.0012694822 | 0.0011269733 | 0.0212819960 |

Projection rounding matters, but this does not establish a complete cause. CPU f32 GEMM and norm reductions, transcendental functions, fused multiply-add behavior, residual accumulation, and Delta order still differ from Metal. C's closer head probabilities do not imply uniformly closer hidden rows. No product fix or question-specific rule is proposed by this evidence.

## Checks and bounds

The saved A head replay reproduced hidden, lexical, and logit values exactly. B/C layer 0 matched the prior WX control bitwise, and both trajectories were identical before the first attention layer. Scalar/tensor byte-hash fixtures, attention and recurrence fixtures, finite values, input immutability, parameter bytes/versions, source hashes, and the original artifact snapshot passed. Complete hidden/memory/head-input arrays remain outside Git, identified by their hashes. Raw layer output totals 941,096,960 bytes.

The run took 203.784 seconds under a 300-second deadline, with two CPU threads, a 12 GiB sampled process-group cap, 24 GiB start requirement, 12 GiB available-memory floor, 2 GiB disk lease, and 25 GiB free-disk floor. Peak accounted memory was 5,238,472,704 bytes; minimum available memory was 94,505,287,680 bytes. Sampling reported no error or refusal. Owned-group cleanup succeeded and all leases were released. No GPU, quiet lease, build, desktop app, or system service was used or controlled.

The first attempt failed before a trunk forward because scalar head-parameter hashing attempted a rank-zero dtype view. Its sources and failure are preserved in the separately published failure bundle and linked by manifest hash here. The reviewed repair only flattens the hash byte view while preserving original shape metadata; no numerical operator or bound changed. Attempt 2 ran that hash self-check before loading weights.

The compact summary is derived from completed receipts and hash-verified saved rows with the Python standard library, without model execution. Its source and the earlier false-span-only summary are included. The full result receipt is below the publisher's one-megabyte file limit and has not been split or pruned.

## Identity and reproduction

The original GGUF SHA-256 is `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`. Native comparison targets the previously recorded Stage6 HTTP server, SHA-256 `77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`, and its verified observer capture. It does not relabel results from a later attention build. Full immutable source/input identities and effective settings appear in `guard.json`.

Absolute account paths are replaced with tokens described in `manifest.json`; replacement was verified reversible against every original. The manifest records original and published hashes and sizes. Raw weights, arrays, binaries, and process-memory logs remain in scratch. Restoring the local path map and required pinned dependencies allows reproduction; this bundle alone does not contain model weights or every upstream dependency.
