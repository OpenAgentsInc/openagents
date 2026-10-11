# Full text029 recurrent Delta control

The one fixed D diagnostic completed. D retains completed control C's projection and key32 attention half boundaries and replaces only the official chunk64 Delta function with the pinned official recurrent function. The original same-Q4 f32 40-request, 104-question acceptance gate is unchanged. This diagnostic does not constitute a new acceptance reference or product fix.

## Results

The table gives P(true) and the selected top. Full logits and both probabilities for all three questions are in `summary.json` and `receipt.json`.

| Question | A | C | D | Native staged | Native fused-fields |
| --- | ---: | ---: | ---: | ---: | ---: |
| rate | 0.334822535289 (false) | 0.336054548385 (false) | 0.335623780739 (false) | 0.335898448152 (false) | 0.335898394968 (false) |
| 日本1 | 0.493030686569 (false) | 0.499907143415 (false) | 0.498891340523 (false) | 0.502091090110 (true) | 0.502091105011 (true) |
| rollback_2 | 0.008845312692 (false) | 0.008824806017 (false) | 0.008820136921 (false) | 0.008821207292 (false) | 0.008821208334 (false) |

Final normalized-memory relative L2 errors follow. Each denominator is the right-hand reference. The question spans [192,194), true option [205,223), and false option [229,247). All 64 pre-final-normalization layer comparisons are preserved separately in `receipt.json`.

| Comparison | Whole | Question | True option | False option |
| --- | ---: | ---: | ---: | ---: |
| D − A | 0.0045503917 | 0.0014298848 | 0.0028965839 | 0.0160177644 |
| D − C | 0.0023434560 | 0.0014405084 | 0.0015486591 | 0.0062035902 |
| D − native | 0.0050280870 | 0.0015634005 | 0.0013586631 | 0.0180883632 |

The operator substitution isolates chunked-versus-recurrent arithmetic. It still does not emulate native Metal exactly: official recurrence dots Q with the updated state, while the archived native scan uses `sq + delta * k_dot_q` with its own FMA and reduction order. GEMM and normalization reductions, transcendental functions, and residual accumulation also differ. Interpret the full trajectory and all questions; a single changed top is not a general correction or gate closure.

## Checks and resources

A and C saved layer63 head replays reproduced their normalized hidden, lexical, and logit values exactly. D layer0 matched saved WXR input hashes, Delta output/final-state hashes, and layer output exactly. The independent f64 arithmetic fixtures and scalar/tensor hash self-test passed. All 48 recurrent and 16 attention calls completed with finite outputs, immutable inputs/weights, exact identities, and no cache. All 64 layer arrays and final memory/head inputs remain in scratch, identified by hashes; no raw arrays or weights are included here.

The run took 193.693 seconds. Peak accounted memory was 5,016,748,032 bytes; minimum available memory was 85,397,307,392 bytes. The unchanged limits were a 12 GiB sampled group cap, 24 GiB start requirement, 12 GiB available-memory floor, 300-second deadline, two CPU threads, a 2 GB disk lease, a 2 GiB output cap, and 25 GiB free-disk floor. The guard reported no sampling, resource, or cleanup error. Its owned group stopped and leases were released. No GPU, quiet lease, build, desktop app, or system service was used or controlled. This was one run, without retries or limit changes.

## Identity

The GGUF SHA-256 remains `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`. Native comparisons use the same archived Stage6 HTTP results and observer capture as A/B/C; no later binary has been substituted. The guard checks frozen executed sources and input hashes before and after execution, plus the verified artifact snapshot.

The manifest links the complete previous precision bundle and records original and published hashes. Absolute account paths are replaced with reversible tokens. No full receipt rows were removed. The plan is preserved as originally reviewed; its initial source-assessment status predates implementation and execution.
