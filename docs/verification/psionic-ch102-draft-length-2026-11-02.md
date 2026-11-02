# Chapter 102: verify drafts by expected gain, 2026-11-02

## What was measured

Greedy decoding of the original gpt-oss-20b (`model.safetensors` at revision
`6cee5e81ee83917806bbde320786a8fb61efebee`), resident on one RTX 4080, on
six paths: target only; chapter 75's prompt n-gram drafts (width 3) verified
by chapter 76's device programs at fixed k = 1, 2, 3 and 4; and adaptive k,
where `dl_choose` (`psionic/lib/draft-length.coax`) picks 0 to 4 drafts per
step from a fitted cost line and a per-position acceptance model.

Workload: chapter 71's two fixture prompts ("Say hi." and "List the integers
from 1 to 100, separated by commas.", greedy, limit 256 IDs; 280 IDs a run).
A calibration pass first runs target only and fixed k = 1..4 once on both
prompts, timing each plain step and each block with the host's monotonic
clock; `dl_fit` and `dl_costs` fit the cost line, and `dl_calibrate` turns
the k = 4 pass's per-position counts into acceptance probabilities. Then
five timed runs, each running both prompts on all six paths, the path order
rotating by one each run. Each path is compared with target only by chapter
76's frozen rule `sg_decide` (5% saving beyond two standard errors, chapter
34's spread rule, identical outputs required).

## Host

| Item | Value |
| --- | --- |
| Machine | coderos-4080, NixOS, Linux 6.18.49 x86-64, GCC |
| GPU | NVIDIA GeForce RTX 4080, 16 GB, sm_89, driver 595.71.05 |
| Compiler | cached coax release build `8925679b5e19…` (binary sha256 `18b90dec5d4e…`); not rebuilt |
| Build words | `coax-18b90dec5d4e-cuda-72df8e2ef7ee4f53-host-97f2cab12cb0a7a1` |
| Commit | `0bd2a85348+uncommitted` (this chapter's files, before their commit) |
| Load | other agents were running on the processors: load average 5.3 to 26 during the timed runs |

The harness binary was built and run with the same commands
`psionic/tests/check_draft_length.sh` uses (`coax cuda` of
`device-kernels.coax`, `coax c --library --release` of
`psionic/tests/draft-length-host.coax`, `gcc -O3 -ffp-contract=off`), with
RUNS = 5.

## Log

```text
device: NVIDIA GeForce RTX 4080 sm_89 16711352320 bytes
original 20b authenticated and resident in 43.987 s
calibration: plain step 15099352 ns over 278 steps
calibration: block of 1 drafts 48322876 ns over 95 blocks
calibration: block of 2 drafts 60628348 ns over 93 blocks
calibration: block of 3 drafts 65637344 ns over 91 blocks
calibration: block of 4 drafts 69278701 ns over 90 blocks
cost line: c(k) = 43997700 + 6787647 k ns; costs 15099352 50785347 57572994 64360641 71148288
acceptance at k=4 by position (kept/reached): 81/90 4/81 3/4 2/3; model 0.8913 0.0602 0.6667 0.6000
with all 4 drafts available: k=0; certain drafts would choose k=4; never pays at any acceptance: no
adaptive hi: ids=24 same=yes plain=23 blocks by k: 0 0 0 0 accepted=0 of 0
adaptive list: ids=256 same=yes plain=255 blocks by k: 0 0 0 0 accepted=0 of 0
greedy outputs on every path equal the target-only outputs; target-only words 447e9a1f919e90b3 7c6705e4ab014dfd
run=0 4221840032 6126076490 7165800511 7610999395 7765579155 4246908245 load1=5.26
run=1 4325675566 6219898313 7302406215 7496150677 7834808286 4414761124 load1=15.89
run=2 4467447756 6200588885 7244798739 7554519394 7841503814 4264759923 load1=22.62
run=3 4286282669 6168827885 7151910299 7516430463 7866060167 4467152221 load1=25.95
run=4 4218739776 6109674494 7149490964 7481949330 7752338315 4221930985 load1=25.15
bound: build=coax-18b90dec5d4e-cuda-72df8e2ef7ee4f53-host-97f2cab12cb0a7a1 commit=0bd2a85348+uncommitted machine=coderos-4080 Linux 6.18.49 driver 595.71.05 device=NVIDIA GeForce RTX 4080 sm_89 16711352320 bytes
target only: mean 4303997160 ns, 65.06 tokens/s
fixed k=1: mean 6165013213 ns, 45.42 tokens/s
fixed k=2: mean 7202881346 ns, 38.87 tokens/s
fixed k=3: mean 7532009852 ns, 37.17 tokens/s
fixed k=4: mean 7812057947 ns, 35.84 tokens/s
adaptive: mean 4323102500 ns, 64.77 tokens/s
fixed k=1 against target only: mean 6165013213 ns against 4303997160 ns; saving -0.4324 of the first, se 50194355 ns; off (none)
fixed k=2 against target only: mean 7202881346 ns against 4303997160 ns; saving -0.6735 of the first, se 54774221 ns; off (none)
fixed k=3 against target only: mean 7532009852 ns against 4303997160 ns; saving -0.7500 of the first, se 51136655 ns; off (none)
fixed k=4 against target only: mean 7812057947 ns against 4303997160 ns; saving -0.8151 of the first, se 50767144 ns; off (none)
adaptive against target only: mean 4323102500 ns against 4303997160 ns; saving -0.0044 of the first, se 67124955 ns; off (none)
adaptive against the best fixed (fixed k=1): mean 4323102500 ns against 6165013213 ns; saving 0.2988 of the first, se 53603973 ns; on (none)
exit 0
```

## Result

| Path | Mean per run | Tokens/s | Against target only |
| --- | ---: | ---: | --- |
| target only | 4.304 s | 65.06 | |
| fixed k = 1 | 6.165 s | 45.42 | −43.2%: off |
| fixed k = 2 | 7.203 s | 38.87 | −67.4%: off |
| fixed k = 3 | 7.532 s | 37.17 | −75.0%: off |
| fixed k = 4 | 7.812 s | 35.84 | −81.5%: off |
| adaptive | 4.323 s | 64.77 | −0.4%: off |

The cost line is c(k) = 44.0 ms + 6.79 ms·k against a 15.1 ms step, and the
acceptance model is 0.891, 0.060, 0.667, 0.600 by position. With those
inputs the rule chooses k = 0 for every step, so the adaptive path ran no
block. It is 29.9% faster than the fastest fixed length (k = 1; `sg_decide`
says on) and not distinguishable from target only. Speculation stays off.

## Checks

| Check | Result |
| --- | --- |
| `coax test psionic/lib/draft-length.coax` (debug, release) | 109 of 109 |
| `coax test psionic/lib/draft-length-run.coax` (debug, release) | 131 of 131 |
| `coax mutate psionic/lib/draft-length.coax --root-only --jobs 8` | 111 mutants, 111 killed |
| `coax mutate psionic/lib/draft-length-run.coax --root-only --jobs 6` | 121 mutants, 121 killed |
| Examples `draft-length` and `draft-length-refuse`, debug and release | transcripts equal |
| Every path's greedy IDs on the RTX 4080 | equal to target only on both prompts |
