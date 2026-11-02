# Product quality and resident latency

Apple M5 Max, 128 GiB, macOS 26.4. One ordered pass with one model open;
all 29 complete pipelines match actual NeMo tokens, frame IDs and text exactly.
Every feature/final encoder tensor passes the tolerance frozen before inference.

Fresh engine initialization: **20.272077 s**. Filesystem cache was uncontrolled.
The following are per-request complete CPU frontend + Metal encoder + CPU TDT
measurements; no separate stage timing or key-release latency was measured.
The first request may include Metal pipeline warm-up.

| Slice | Audio s | Word errors / words | WER | Character errors / chars | CER | Resident inference ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 6930-75918-0000 | 3.505 | 0 / 8 | 0.00% | 0 / 46 | 0.00% | 1293.412 |
| 6930-75918-0002 | 5.025 | 0 / 11 | 0.00% | 0 / 78 | 0.00% | 1360.674 |
| 6930-75918-0006 | 5.850 | 0 / 17 | 0.00% | 0 / 91 | 0.00% | 1541.441 |
| 6930-75918-0011 | 3.195 | 0 / 10 | 0.00% | 0 / 43 | 0.00% | 742.898 |
| bg | 13.860 | 5 / 34 | 14.71% | 14 / 190 | 7.37% | 3200.380 |
| hr | 8.040 | 1 / 17 | 5.88% | 2 / 99 | 2.02% | 2246.383 |
| cs | 13.320 | 2 / 22 | 9.09% | 3 / 130 | 2.31% | 2962.937 |
| da | 5.100 | 3 / 12 | 25.00% | 4 / 67 | 5.97% | 1317.570 |
| nl | 6.480 | 1 / 15 | 6.67% | 1 / 101 | 0.99% | 1335.280 |
| en | 10.560 | 2 / 19 | 10.53% | 4 / 99 | 4.04% | 1511.245 |
| et | 14.100 | 5 / 19 | 26.32% | 6 / 146 | 4.11% | 2373.417 |
| fi | 18.600 | 2 / 22 | 9.09% | 1 / 215 | 0.47% | 2855.634 |
| fr | 10.200 | 0 / 19 | 0.00% | 0 / 110 | 0.00% | 1356.841 |
| de | 17.940 | 1 / 18 | 5.56% | 1 / 121 | 0.83% | 1963.371 |
| el | 11.040 | 3 / 20 | 15.00% | 3 / 142 | 2.11% | 2185.346 |
| hu | 12.060 | 0 / 17 | 0.00% | 0 / 104 | 0.00% | 1557.591 |
| it | 11.520 | 0 / 23 | 0.00% | 0 / 158 | 0.00% | 1691.593 |
| lv | 12.120 | 5 / 17 | 29.41% | 6 / 105 | 5.71% | 1710.693 |
| lt | 5.760 | 1 / 8 | 12.50% | 2 / 65 | 3.08% | 853.790 |
| mt | 13.740 | 2 / 21 | 9.52% | 4 / 138 | 2.90% | 1700.645 |
| pl | 12.120 | 0 / 20 | 0.00% | 0 / 160 | 0.00% | 1754.001 |
| pt | 17.760 | 1 / 36 | 2.78% | 1 / 225 | 0.44% | 2060.081 |
| ro | 6.300 | 4 / 15 | 26.67% | 7 / 87 | 8.05% | 1000.564 |
| sk | 9.840 | 1 / 16 | 6.25% | 3 / 88 | 3.41% | 1081.537 |
| sl | 7.500 | 7 / 20 | 35.00% | 7 / 105 | 6.67% | 1064.912 |
| es | 12.840 | 2 / 29 | 6.90% | 2 / 169 | 1.18% | 1552.393 |
| sv | 5.760 | 3 / 12 | 25.00% | 6 / 71 | 8.45% | 926.573 |
| ru | 16.920 | 0 / 21 | 0.00% | 0 / 154 | 0.00% | 2075.257 |
| uk | 7.380 | 0 / 14 | 0.00% | 0 / 100 | 0.00% | 1098.453 |

The original English cohort passes its frozen gate: **0/46 word errors and
0/258 character errors**. Each FLEURS language has one utterance; these figures
are smoke observations, not representative language accuracy. The independent
Unicode normalization is recorded in the JSON and differs from the legacy
ASCII-only Coax scorer. No multilingual threshold was selected after the run.

Observed finite cohort: 298.435 s audio / 48.374912 s inference = 6.169 audio-seconds
per inference-second; this is not a performance comparison with a different
computer or NVIDIA published batches. Whole process: 69.040294 s, including load
and artifact recording.
