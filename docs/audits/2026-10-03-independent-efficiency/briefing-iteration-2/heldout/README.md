# Heldout replay panel

Verdict: **no clear win**.

This is the registered engineering gate on four pairs, not a population-level significance claim.

| Run | Status | Accepted | Attempts | CLI cost ($) | Agent (s) | Checks (s) | Recorded endpoint (s) | Tools |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| heldout-1-control | complete | true | 1 | 1.1020 | 215.7571 | 90.5001 | 334.5135 | 32 |
| heldout-2-treatment | complete | true | 1 | 0.9154 | 174.1668 | 86.9652 | 295.6224 | 33 |
| heldout-3-treatment | complete | true | 1 | 0.9372 | 145.8079 | 87.6265 | 264.2428 | 34 |
| heldout-4-control | complete | true | 1 | 0.8762 | 141.7221 | 82.3156 | 257.7321 | 20 |
| heldout-5-control | complete | true | 1 | 1.1086 | 181.2909 | 83.6820 | 267.9841 | 30 |
| heldout-6-treatment | complete | true | 1 | 0.9947 | 133.6391 | 87.5530 | 224.6861 | 31 |
| heldout-7-treatment | complete | true | 1 | 1.1261 | 155.1865 | 97.0233 | 292.8257 | 37 |
| heldout-8-control | complete | true | 2 | 1.1469 | 212.3463 | 182.2427 | 402.0330 | 28 |

Costs use the last cumulative CLI list-price estimate once, including any repair. They are not verified subscription charges. Incomplete observations remain visible and cannot pass a gate. These are fixed-endpoint costs; failed runs are not a cost through acceptance.

Recorded endpoint wall time is wall_s + preparation_wall_s + warm_brief_wall_s, with warm briefing preparation added only for treatment. It includes source export, instruction checks, prompt setup, the model session, external checks, and executor-process shutdown. The frozen runner records wall_s before final candidate/artifact capture, scratch-workspace deletion, and final result serialization. Those later steps are excluded, so this metric does not measure the entire harness elapsed time. The registered formula and gate remain unchanged.

Paid briefing cost is zero only when preparation records zero model calls. Shared instruction warmup uses the same timing boundary, and cold indexing is reported separately in metrics.json; missing machine costs are not treated as zero.

The JSON includes per-attempt checks, final cumulative model/cache counters, tool counts, and artifact digests. Patches describe file-byte changes; the frozen runner does not record chmod-only changes. Redacted or binary-omitting patches are labeled and are not exact replay artifacts. Raw model text, thinking, tool inputs/results, streams, and init/account metadata are excluded.
