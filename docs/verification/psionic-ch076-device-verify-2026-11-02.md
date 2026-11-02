# Chapter 76: verify a draft block in parallel, 2026-11-02

## What was measured

Greedy speculative decoding on the original gpt-oss-20b (`model.safetensors`
at revision `6cee5e81ee83917806bbde320786a8fb61efebee`), resident on one RTX
4080. Drafts: chapter 75's prompt n-gram (`gv_draft`, suffix width 3, k = 4,
fewer near the end of the context). Each block (the pending ID plus the
drafts) is verified in one chapter 67 batched prefill on the existing cache
with f32 expert products; rows are read out one at a time until the first
mismatch (`gv_readout`, `gv_more`); `gv_commit` commits the matching prefix
plus the correction or bonus; `gv_restore` puts back only the speculative
cache rows saved by `gv_save`; `gv_control` sets the control words. Positions
with no n-gram match take one ordinary decode step.

Workload per run: the two fixture prompts of chapter 71 ("Say hi." and "List
the integers from 1 to 100, separated by commas.", greedy, limit 256 IDs),
each on both paths, timed from after the prompt prefill to the last ID with
the host's monotonic clock. One untimed warm-up and five timed runs; the order
of the two paths alternates between runs. Rule (`sg_decide`, frozen before
any measurement): on only when the saving minus two standard errors of the
difference is at least 5% of the target-only mean, with chapter 34's Welford
statistics and its spread rule, and only when every speculative output equals
the target-only output.

## Host

| Item | Value |
| --- | --- |
| Machine | coderos-4080, NixOS, Linux 6.18.49 x86-64, GCC (`COAX_CC=gcc`) |
| GPU | NVIDIA GeForce RTX 4080, 16 GB, sm_89, driver 595.71.05 |
| Compiler | cached coax release build `8925679b5e19…` (binary sha256 `18b90dec5d4e…`) |
| Build words | `coax-18b90dec5d4e-cuda-72df8e2ef7ee4f53-host-acb383a95b409815` |
| Commit | `70615f1710+uncommitted` (this chapter's files, before their commit) |

The f32 decode module (`72df8e2ef7ee4f53`) is the one chapter 71 measured.

## Final receipt

`COAX_CC=gcc psionic/tests/check_device_verify.sh ORIGINAL 5 4`, exit 0:

```text
220 checks, 220 passed, 0 failed
10 checks, 10 passed, 0 failed
220 checks, 220 passed, 0 failed
10 checks, 10 passed, 0 failed
load average before: 2.71 3.20 3.42
device: NVIDIA GeForce RTX 4080 sm_89 16711352320 bytes
original 20b authenticated and resident in 40.249 s; prefill expert products: gpt-oss-cuda-f32-v1
speculation: prompt n-gram drafts (width 3), k=4, verified in one batched prefill per block
check hi: ids=24 same=yes blocks=3 drafted=12 accepted=4 rows=7 plain_steps=16 base_ns=0 spec_ns=0
check list: ids=256 same=yes blocks=87 drafted=348 accepted=86 rows=173 plain_steps=82 base_ns=0 spec_ns=0
greedy outputs with speculation equal the target-only outputs; target-only words 447e9a1f919e90b3 7c6705e4ab014dfd
cache bytes after rollback equal the target-only cache: 6 of 6 positions past the window
warm-up hi: ids=24 same=yes blocks=3 drafted=12 accepted=4 rows=7 plain_steps=16 base_ns=0 spec_ns=0
warm-up list: ids=256 same=yes blocks=87 drafted=348 accepted=86 rows=173 plain_steps=82 base_ns=0 spec_ns=0
run=-1 base_ns=4207764518 spec_ns=7720427937 base_tok_s=66.544 spec_tok_s=36.267 load1=2.30
run=0 base_ns=4207729177 spec_ns=7719235490 base_tok_s=66.544 spec_tok_s=36.273 load1=2.46
run=1 base_ns=4208397202 spec_ns=7718488468 base_tok_s=66.534 spec_tok_s=36.277 load1=2.54
run=2 base_ns=4216995015 spec_ns=7743290363 base_tok_s=66.398 spec_tok_s=36.160 load1=2.50
run=3 base_ns=4217715324 spec_ns=7744302239 base_tok_s=66.387 spec_tok_s=36.156 load1=2.49
run=4 base_ns=4217657975 spec_ns=7740821589 base_tok_s=66.388 spec_tok_s=36.172 load1=2.46
bound: build=coax-18b90dec5d4e-cuda-72df8e2ef7ee4f53-host-acb383a95b409815 commit=70615f1710+uncommitted machine=coderos-4080 Linux 6.18.49 driver 595.71.05 device=NVIDIA GeForce RTX 4080 sm_89 16711352320 bytes
base: n=5 mean_ns=4213698939 sd_ns=5157904; spec: mean_ns=7733227630 sd_ns=13177601; gain=-0.8353 se_ns=6328557 threshold=5%
speculation: off (none)
load average after: 2.70 2.96 3.29
```

The four `checks` lines are `coax test` of `device-verify.coax` and
`speculation-gate.coax` in debug and release; between them and the device
lines the script also compared both example transcripts in both modes and
the tiny-model CPU harness (plain and ASan/UBSan) with
`psionic/fixtures/device-verify/tiny.out`.

Verdict: target-only 66.45 tokens/s, speculative 36.21 tokens/s; the saving is
−83.5% of the target-only time. **Speculation off.**

## Earlier run

One earlier run of the same harness, before three refactors made for the
mutation check (`gv_room` and the copy-run length as counting loops, the
readout's capacity guard removed; the programs it emits are unchanged on these
shapes), gave the same parity and cache results and the same counts:
target-only mean 4.218249 s (sd 3.3 ms), speculative mean 7.744757 s (sd
11.4 ms), off. Its build word was `…-host-1dcd65b7968fcc3c`.

## Checks

| Check | Result |
| --- | --- |
| `coax test psionic/lib/gptoss/device-verify.coax` (debug, release) | 220 of 220 (includes chapter 75's and the device planner's checks it loads) |
| `coax test psionic/lib/speculation-gate.coax` (debug, release) | 10 of 10 |
| `coax mutate psionic/lib/gptoss/device-verify.coax --root-only` | 342 mutants, 342 killed |
| `coax mutate psionic/lib/speculation-gate.coax --root-only` | 42 mutants, 42 killed |
| Tiny owned model, CPU (device programs compiled to C), plain and ASan/UBSan | 9 generations equal target-only; 13 of 13 caches after rollback equal target-only |
| Same harness with the restore removed (a deliberate break) | "cache after rollback at position 3 DIFFERS from the target-only cache", exit 1 |
| Examples `device-verify` and `device-verify-refuse`, debug and release | transcripts equal |

All checks used the cached compiler named above; the compiler was not rebuilt.
