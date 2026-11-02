# Psionic chapter 175: continuous batching on the RTX 4080, 2026-11-02

The log of `psionic/tests/check_device_batch.sh` on the original
gpt-oss-20b: every request alone, then one warm-up and five timed runs at
concurrency 1, 4 and 8. The commit is e03a37181b with this change not yet
committed; the build words name the Coax compiler (the cached release
build), the CUDA module and the host library. The host was shared with other
work; the one-minute load average is recorded with every run.

```text
load average before: 39.02 35.34 34.90
device: NVIDIA GeForce RTX 4080 sm_89 16711352320 bytes
original 20b authenticated and resident in 91.282 s
regions: 8 of 106954816 bytes; batch 128 rows; device bytes in use=15634530304 of 16711352320
alone request=0 prompt=59 ids=24 word=447e9a1f919e90b3
alone request=1 prompt=70 ids=256 word=7c6705e4ab014dfd
alone request=2 prompt=96 ids=64 word=b16727dc42b92149
alone request=3 prompt=160 ids=64 word=b91d8a3b119b5673
alone request=4 prompt=224 ids=64 word=bb424cb11c007604
alone request=5 prompt=288 ids=64 word=4cba083f5db66e32
alone request=6 prompt=352 ids=64 word=4274d6dfe6c1eabd
alone request=7 prompt=416 ids=64 word=031e3591723afe5a
alone: 664 output IDs in 15.285 s
fixture prompts alone equal the reference words
concurrency=1 run=-1 steps=673 rows=2321 ns=29488717934 tok_s=22.517 latency50_ns=16136925679 latency95_ns=29488693918 first50_ns=13800141166 first95_ns=27102015233 same=yes load1=36.21
concurrency=1 run=0 steps=673 rows=2321 ns=29495421290 tok_s=22.512 latency50_ns=16142796311 latency95_ns=29495386249 first50_ns=13806673045 first95_ns=27109107950 same=yes load1=34.25
concurrency=1 run=1 steps=673 rows=2321 ns=29491350467 tok_s=22.515 latency50_ns=16142183562 latency95_ns=29491327615 first50_ns=13804844915 first95_ns=27105413487 same=yes load1=33.39
concurrency=1 run=2 steps=673 rows=2321 ns=29509286321 tok_s=22.501 latency50_ns=16139093762 latency95_ns=29509265270 first50_ns=13803495205 first95_ns=27115555393 same=yes load1=32.76
concurrency=1 run=3 steps=673 rows=2321 ns=29589131310 tok_s=22.441 latency50_ns=16197134339 latency95_ns=29589100718 first50_ns=13853892009 first95_ns=27195464838 same=yes load1=32.42
concurrency=1 run=4 steps=673 rows=2321 ns=29584478050 tok_s=22.444 latency50_ns=16193476532 latency95_ns=29584450524 first50_ns=13849069497 first95_ns=27191416847 same=yes load1=32.03
concurrency 1: n=5 mean_ns=29533933488 sd_ns=48748175 tok_s=22.483 latency P50 16.163 s P95 29.534 s first token P50 13.824 s P95 27.143 s
chapter 71's gate at concurrency 1: none
concurrency=4 run=-1 steps=257 rows=2321 ns=20323248733 tok_s=32.672 latency50_ns=10647160828 latency95_ns=20323227851 first50_ns=1201570884 first95_ns=11615641595 same=yes load1=32.28
concurrency=4 run=0 steps=257 rows=2321 ns=20389351880 tok_s=32.566 latency50_ns=10647471207 latency95_ns=20389325969 first50_ns=1201831737 first95_ns=11616911429 same=yes load1=32.05
concurrency=4 run=1 steps=257 rows=2321 ns=20355554072 tok_s=32.620 latency50_ns=10671494368 latency95_ns=20355522991 first50_ns=1211498810 first95_ns=11640512355 same=yes load1=32.04
concurrency=4 run=2 steps=257 rows=2321 ns=20331718358 tok_s=32.658 latency50_ns=10649236769 latency95_ns=20331693009 first50_ns=1202361177 first95_ns=11618350562 same=yes load1=31.96
concurrency=4 run=3 steps=257 rows=2321 ns=20336042045 tok_s=32.651 latency50_ns=10653481510 latency95_ns=20336014498 first50_ns=1203508793 first95_ns=11622436683 same=yes load1=31.75
concurrency=4 run=4 steps=257 rows=2321 ns=20328050500 tok_s=32.664 latency50_ns=10648824240 latency95_ns=20328029715 first50_ns=1201833416 first95_ns=11617467785 same=yes load1=31.61
concurrency 4: n=5 mean_ns=20348143371 sd_ns=25362405 tok_s=32.632 latency P50 10.654 s P95 20.348 s first token P50 1.204 s P95 11.623 s
chapter 71's gate at concurrency 4: none
concurrency 4 against 1 (chapter 34): ratio 1.4514, se 24574931 ns: faster
concurrency=8 run=-1 steps=257 rows=2321 ns=19342235751 tok_s=34.329 latency50_ns=11908051570 latency95_ns=19342204569 first50_ns=1498358133 first95_ns=5437097951 same=yes load1=31.60
concurrency=8 run=0 steps=257 rows=2321 ns=19327775399 tok_s=34.355 latency50_ns=11908149595 latency95_ns=19327739628 first50_ns=1497855130 first95_ns=5436779099 same=yes load1=31.87
concurrency=8 run=1 steps=257 rows=2321 ns=19323208406 tok_s=34.363 latency50_ns=11909659730 latency95_ns=19323183812 first50_ns=1498667338 first95_ns=5436643272 same=yes load1=31.82
concurrency=8 run=2 steps=257 rows=2321 ns=19323795279 tok_s=34.362 latency50_ns=11908866285 latency95_ns=19323764656 first50_ns=1498458195 first95_ns=5436297672 same=yes load1=31.72
concurrency=8 run=3 steps=257 rows=2321 ns=19326847475 tok_s=34.356 latency50_ns=11908815469 latency95_ns=19326823472 first50_ns=1497777087 first95_ns=5436469495 same=yes load1=31.52
concurrency=8 run=4 steps=257 rows=2321 ns=19322136946 tok_s=34.365 latency50_ns=11909267341 latency95_ns=19322111658 first50_ns=1497975178 first95_ns=5436597033 same=yes load1=31.39
concurrency 8: n=5 mean_ns=19324752701 sd_ns=2432512 tok_s=34.360 latency P50 11.909 s P95 19.325 s first token P50 1.498 s P95 5.437 s
chapter 71's gate at concurrency 8: none
concurrency 8 against 1 (chapter 34): ratio 1.5283, se 21827971 ns: faster
batched IDs equal every request's IDs alone in every run
bound: build=coax-18b90dec5d4e-cuda-72df8e2ef7ee4f53-host-78032818a4303c48 commit=e03a37181b+uncommitted machine=coderos-4080 Linux 6.18.49 driver 595.71.05
load average after: 31.44 32.67 33.89
exit=0
```
