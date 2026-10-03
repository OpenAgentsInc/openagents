# Development replay panel

Verdict: **pending**.

The registered four-pair panel is incomplete.

| Run | Status | Accepted | Attempts | CLI cost ($) | Agent (s) | Checks (s) | Total with preparation (s) | Tools |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| development-1-control | complete | true | 2 | 1.0502 | 224.4872 | 32.5097 | 260.9816 | 29 |
| development-2-treatment | complete | true | 2 | 0.9127 | 143.4734 | 28.0561 | 175.5520 | 31 |
| development-3-treatment | complete | true | 2 | 0.7097 | 116.2527 | 27.1857 | 147.6405 | 24 |
| development-4-control | in_progress_or_interrupted | false | 0 | — | — | — | — | 8 |
| development-5-control | missing | false | 0 | — | — | — | — | 0 |
| development-6-treatment | missing | false | 0 | — | — | — | — | 0 |
| development-7-treatment | missing | false | 0 | — | — | — | — | 0 |
| development-8-control | missing | false | 0 | — | — | — | — | 0 |

Costs use the last cumulative CLI list-price estimate once, including any repair. They are not verified subscription charges. Incomplete observations remain visible and cannot pass a gate. These are fixed-endpoint costs; failed runs are not a cost through acceptance.

Total wall time includes export/setup, the model session, external checks, and the measured warm briefing preparation for treatment. Paid briefing cost is zero only when preparation records zero model calls. Shared instruction warmup and cold indexing are reported separately in metrics.json; missing machine costs are not treated as zero.

The JSON includes per-attempt checks, final cumulative model/cache counters, tool counts, and artifact digests. Patches describe file-byte changes; the frozen runner does not record chmod-only changes. Redacted or binary-omitting patches are labeled and are not exact replay artifacts. Raw model text, thinking, tool inputs/results, streams, and init/account metadata are excluded.
