# Independent v1 / v2 cohort observations

All 29 complete pipelines pass the original frozen NeMo comparison. Raw text, token IDs and frame IDs remain exact; every feature/final-encoder value meets the unchanged tolerance. Independent Unicode WER/CER counts match v1 in every slice.

One ordered pass per version on the same Apple M5 Max. These are descriptive observations, with cache and scheduling uncontrolled; excluding the first request does not establish steady-state latency. The first request may include Metal allocation/upload or warm-up. `info.inference_ms` is reported recognition/inference time: it starts after the API lock is acquired and ends before successful snapshot publication and caller-output copies. It covers recognition checks, frontend, encoder, decoder and checked text rendering. Lock wait and publication/copy time are excluded; this is not whole C API call wall time. No separate stage or key-release latency was measured.

298.435 audio seconds: v1 **48.374912 s**, v2 **27.199433 s** of reported recognition/inference; observed ratio **1.779×**. Fresh engine open: v1 20.272077 s, v2 15.861997 s (not filesystem-cold). Actual v2 decoder threads: 8.

| Slice | Audio s | Word errors / words | Character errors / chars | V1 recognition ms | V2 recognition ms | Observed ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 6930-75918-0000 | 3.505 | 0 / 8 | 0 / 46 | 1293.412 | 769.477 | 1.681× |
| 6930-75918-0002 | 5.025 | 0 / 11 | 0 / 78 | 1360.674 | 559.329 | 2.433× |
| 6930-75918-0006 | 5.850 | 0 / 17 | 0 / 91 | 1541.441 | 604.150 | 2.551× |
| 6930-75918-0011 | 3.195 | 0 / 10 | 0 / 43 | 742.898 | 387.167 | 1.919× |
| bg | 13.860 | 5 / 34 | 14 / 190 | 3200.380 | 1431.044 | 2.236× |
| hr | 8.040 | 1 / 17 | 2 / 99 | 2246.383 | 819.453 | 2.741× |
| cs | 13.320 | 2 / 22 | 3 / 130 | 2962.937 | 1205.999 | 2.457× |
| da | 5.100 | 3 / 12 | 4 / 67 | 1317.570 | 550.553 | 2.393× |
| nl | 6.480 | 1 / 15 | 1 / 101 | 1335.280 | 595.294 | 2.243× |
| en | 10.560 | 2 / 19 | 4 / 99 | 1511.245 | 836.017 | 1.808× |
| et | 14.100 | 5 / 19 | 6 / 146 | 2373.417 | 1058.883 | 2.241× |
| fi | 18.600 | 2 / 22 | 1 / 215 | 2855.634 | 1453.952 | 1.964× |
| fr | 10.200 | 0 / 19 | 0 / 110 | 1356.841 | 816.415 | 1.662× |
| de | 17.940 | 1 / 18 | 1 / 121 | 1963.371 | 1217.897 | 1.612× |
| el | 11.040 | 3 / 20 | 3 / 142 | 2185.346 | 1067.025 | 2.048× |
| hu | 12.060 | 0 / 17 | 0 / 104 | 1557.591 | 844.465 | 1.844× |
| it | 11.520 | 0 / 23 | 0 / 158 | 1691.593 | 865.030 | 1.956× |
| lv | 12.120 | 5 / 17 | 6 / 105 | 1710.693 | 817.988 | 2.091× |
| lt | 5.760 | 1 / 8 | 2 / 65 | 853.790 | 404.439 | 2.111× |
| mt | 13.740 | 2 / 21 | 4 / 138 | 1700.645 | 1347.626 | 1.262× |
| pl | 12.120 | 0 / 20 | 0 / 160 | 1754.001 | 1406.866 | 1.247× |
| pt | 17.760 | 1 / 36 | 1 / 225 | 2060.081 | 1633.655 | 1.261× |
| ro | 6.300 | 4 / 15 | 7 / 87 | 1000.564 | 742.375 | 1.348× |
| sk | 9.840 | 1 / 16 | 3 / 88 | 1081.537 | 948.004 | 1.141× |
| sl | 7.500 | 7 / 20 | 7 / 105 | 1064.912 | 802.764 | 1.327× |
| es | 12.840 | 2 / 29 | 2 / 169 | 1552.393 | 1145.690 | 1.355× |
| sv | 5.760 | 3 / 12 | 6 / 71 | 926.573 | 590.490 | 1.569× |
| ru | 16.920 | 0 / 21 | 0 / 154 | 2075.257 | 1446.104 | 1.435× |
| uk | 7.380 | 0 / 14 | 0 / 100 | 1098.453 | 831.282 | 1.321× |

The four original English recordings retain 0/46 word errors and 0/258 character errors. Each FLEURS language contains one utterance; those counts do not estimate corpus accuracy. Timings are reported whole-utterance offline recognition observations, not the Coder 300 ms dictation target.
