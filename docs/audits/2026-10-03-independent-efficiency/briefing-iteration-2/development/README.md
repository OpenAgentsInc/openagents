# Development replay panel

Verdict: **advance to heldout**.

This is the registered engineering gate on four pairs, not a population-level significance claim.

| Run | Status | Accepted | Attempts | CLI cost ($) | Agent (s) | Checks (s) | Recorded endpoint (s) | Tools |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| development-1-control | complete | true | 2 | 1.0502 | 224.4872 | 32.5097 | 260.9816 | 29 |
| development-2-treatment | complete | true | 2 | 0.9127 | 143.4734 | 28.0561 | 175.5520 | 31 |
| development-3-treatment | complete | true | 2 | 0.7097 | 116.2527 | 27.1857 | 147.6405 | 24 |
| development-4-control | complete | true | 2 | 0.7586 | 133.9721 | 27.7986 | 165.5408 | 27 |
| development-5-control | complete | true | 2 | 0.7278 | 150.0285 | 29.0646 | 182.9750 | 24 |
| development-6-treatment | complete | true | 2 | 0.7461 | 129.2794 | 28.5594 | 161.8147 | 22 |
| development-7-treatment | complete | true | 2 | 0.7667 | 138.8332 | 30.4512 | 173.5100 | 22 |
| development-8-control | complete | true | 2 | 0.9677 | 171.3554 | 32.2714 | 277.5965 | 32 |

Costs use the last cumulative CLI list-price estimate once, including any repair. They are not verified subscription charges. Incomplete observations remain visible and cannot pass a gate. These are fixed-endpoint costs; failed runs are not a cost through acceptance.

Recorded endpoint wall time is wall_s + preparation_wall_s + warm_brief_wall_s, with warm briefing preparation added only for treatment. It includes source export, instruction checks, prompt setup, the model session, external checks, and executor-process shutdown. The frozen runner records wall_s before final candidate/artifact capture, scratch-workspace deletion, and final result serialization. Those later steps are excluded, so this metric does not measure the entire harness elapsed time. The registered formula and gate remain unchanged.

Paid briefing cost is zero only when preparation records zero model calls. Shared instruction warmup uses the same timing boundary, and cold indexing is reported separately in metrics.json; missing machine costs are not treated as zero.

The JSON includes per-attempt checks, final cumulative model/cache counters, tool counts, and artifact digests. Patches describe file-byte changes; the frozen runner does not record chmod-only changes. Redacted or binary-omitting patches are labeled and are not exact replay artifacts. Raw model text, thinking, tool inputs/results, streams, and init/account metadata are excluded.
