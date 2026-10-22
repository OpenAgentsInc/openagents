# Project Maya: notes for Psionic and a consumer V4.1 profile

Sources, read on 2026-10-21:

- [`mw00/project-maya`](https://github.com/mw00/project-maya) at commit
  `451f9cb6a21ea65673bdde2e57b4a670ae2b8859` (release v1.1.21,
  2026-10-21T19:49Z). MIT. The repository was created on 2026-10-06. It
  has 455 commits and 57 releases, from v1.0.0 on 2026-10-07 to v1.1.21.
- [`Niko1221/Strata`](https://github.com/Niko1221/Strata), its parent, at
  commit `011c9e600a93bbc9fd451c563501b5831c32bbca` (v0.2.2, engine
  0.1.47). MIT.
- Maya pull requests
  [#271](https://github.com/mw00/project-maya/pull/271) (first DeepSeek
  engine, merged 2026-10-19 as `0d18775371`) and
  [#276](https://github.com/mw00/project-maya/pull/276) (4- and 8-GPU
  tensor parallelism and packed FP4 KV; open, not merged), and issue
  [#280](https://github.com/mw00/project-maya/issues/280) (slow DeepSeek
  prefill).
- The quant repository
  [`peasantsmith/DeepSeek-V4.1-Flash-1017-Maya-GGUF`](https://huggingface.co/peasantsmith/DeepSeek-V4.1-Flash-1017-Maya-GGUF)
  at revision `b9e25f34b53806b24e5b18085de6ed5967ad9ed5`. Maya v1.1.21 pins
  `276e0badcfb4a6d8f6c2b9d4da1b468cd339c221` for its downloads. MIT.
- [`Indras-Mirror/Xolotl`](https://github.com/Indras-Mirror/Xolotl)
  (formerly Strata-DSV4.1), another MIT Strata fork that serves V4.1 on
  one RTX 4090.
- The MindStudio posts that prompted this note:
  [Project Maya explained](https://www.mindstudio.ai/blog/project-maya-fork-strata-llama-cpp)
  and
  [GLM 5.3 Flash local review](https://www.mindstudio.ai/blog/glm-5-3-local-review-project-maya),
  both dated 2026-10-20.
- Community posts:
  [AlphaSignal, 2026-10-12](https://alphasignal.ai/news/project-maya-runs-321b-glm-5-3-flash-on-a-single-workstation-gpu),
  [PTT AI_Art, 2026-10-11](https://www.ptt.cc/bbs/AI_Art/M.1791697495.A.4E0.html),
  [lcz.me topic 2195, 2026-10-08](https://lcz.me/topic/2195) (now
  deleted, visible to moderators only), and
  [an X post by @ItsmeAjayKV](https://x.com/ItsmeAjayKV/status/2112234657404346724).
  We found no r/LocalLLaMA thread we could open.

We read the Maya and Strata code as data. We built and ran Maya once, on a
rented machine, to measure DeepSeek-V4.1-Flash; see
[Our measurement](#our-measurement). Facts come from the files cited.
Paths are relative to the Maya repository unless they start with `S:`
(Strata). Paragraphs marked **Inference** are our reading of what the
facts mean for us.

**Verdict:** Maya is real, fast-moving, and MIT. For DeepSeek-V4.1 it's a
good engine for one PC with one to nine consumer GPUs and a lot of RAM.
It's not the source of the MindStudio numbers, which none of its files
contain. What Maya does better than Psionic today is single-box serving:
a hot-expert cache in VRAM with the cold experts computed or streamed
from host memory, a 2- to 3-bit expert quant, Engram rows read from the
SSD, and prefill that reads each expert once per layer per prompt. Those
four designs, reimplemented in Rust, are the core of a `consumer` serving
profile and a "DeepSeek V4.1 on a home Pylon" offer. None of them change
the qualified profile, and the quant must stay a separate, unqualified
profile.

## What Maya is

Maya is a C++/CUDA inference engine with a Python server, a browser
dashboard, an OpenAI- and Anthropic-compatible API, and a one-command
setup (`./setup.sh` or `START-MAYA.bat`). It serves two models, each on an
engine of its own:

- GLM-5.3-Flash (321B, about 18B active) on `strata`, from 2026-10-07.
- DeepSeek-V4.1-Flash-1017 on `maya-ds4` (`src/ds4/`), from 2026-10-19.

The README says 748B parameters for V4.1. Our own count is about 544B in
the routed experts plus about 196B in the two Engram tables
(`docs/psionic/deepseek-v41-native.md`, "Memory split"), so Maya's 748B
includes Engram. Xolotl's "552B" leaves it out. Both describe the same
checkpoint.

### Lineage and licenses

- **Maya from Strata.** "Maya's engine started from Strata's and was
  rewritten for GLM-5.3-Flash ... and its server and dashboard grew from
  Strata's" (`README.md:262-264`). The code still uses the `strata::`
  namespace and `STRATA_*` variables. Maya dropped Strata's Metal, SYCL,
  Tauri app, and Qwen3.8-Flash-Next focus.
- **Strata from llama.cpp.** Neither repository is a fork of llama.cpp
  in the Git sense. Both fetch llama.cpp at pinned commit `3cf03257f2` at
  build time (`CMakeLists.txt:872-905`, `third_party/ggml/VERSION.txt`)
  and link ggml for the quant formats, the CPU dot products, and the
  prompt path's MMQ kernels (`README.md:265-267`). Strata transcribed the
  i-quant GPU dot products into `S:src/kernels/cuda/iq_kernels.cu`
  (`S:docs/DETAILS.md:2331-2334`). The engines, the expert tiers, and the
  DeepSeek layer are their own code.
- **Licenses.** `LICENSE` is MIT, "Copyright (c) 2026 Peasant Smith and
  the Project Maya contributors" and "Copyright (c) 2026 Niko1221 and the
  Strata contributors". `LICENSE.deepseek-v41` is MIT, "Copyright (c) 2023
  DeepSeek", for the vendored `serve/ds41_encoding.py`. Third-party code
  is MIT: ggml ("The ggml authors"), exllamav3 ("Turboderp"), and
  beellama.cpp. One optional drafter, DFlash2, is CC BY-NC-ND 4.0 and is
  downloaded only when chosen; don't use it. Xolotl is MIT. The quants
  are MIT, as is DeepSeek's checkpoint.

**Inference:** everything we would take is MIT, so we may copy code with
the copyright notice kept. We won't need to: the designs are what matter,
and they reimplement cleanly in Rust over our own kernels. Where a Psionic
file follows a Maya design closely, name Maya and its commit in the
commit message, as we do for other carried designs.

## The claims, checked

| Claim (MindStudio, 2026-10-20) | Status | Evidence |
| --- | --- | --- |
| Maya forks Strata, which forks llama.cpp | **Partly true** | Maya grew from Strata. Strata links ggml from a pinned llama.cpp commit; it isn't a llama.cpp fork. |
| Own quant format, "Maya-N" | **False as named** | No "Maya-N" exists in either repository, the releases, or Hugging Face. The quants are Maya-S, Maya-S24, Maya-M, Maya-M-Derisked, Maya-L (GLM), and Maya-S and Maya-XS (V4.1). All are ordinary GGUF files in ggml block types. |
| 11.1 tok/s on V4.1 Flash, Dell T5810, 2× RTX 3060, E5-2696 v4, 256 GB | **Unverified** | No T5810, E5-2696 v4, or 2× RTX 3060 appears in the repository, its issues, or its benchmark table. The companion review gives "about 9 to 11" tok/s and pairs it with the UD-IQ1_M quant. |
| 55–60 tok/s on GLM 5.3 Flash | **Plausible, unverified** | The README's nearest rows are 53.0–54.5 tok/s on 2× RTX 3090 and 2× V100 (`README.md:96-136`). Two RTX 3060s have less VRAM than either. |
| Prefill 1,100–1,500 tok/s | **True for GLM on more GPUs** | PTT measured 1,044–1,458 tok/s at 16K–64K on 8× RTX 3060 + 1× 3060 Ti with Maya-S (GLM). V4.1 prefill on consumer rigs ranges from 50 to 1,010 tok/s; see below. |
| Context up to 1M | **GLM only** | GLM reaches 1M with KV streaming (`docs/SETTINGS.md:108`). Maya's V4.1 setup offers 32K–256K (`maya.py:437`). Xolotl reports 768K and 1M V4.1 prompts completing. |
| Faster than stock llama.cpp with UD-IQ1_M (about 250 GB) | **Unverified** | Unsloth's `UD-IQ1_M` is 250.4 GB in 5 files (`unsloth/DeepSeek-V4.1-Flash-1017-GGUF`, revision `60ad3651`), so the size is right. The post gives no llama.cpp number. On GLM, PTT measured Maya-S at 1.20–1.32× llama.cpp b11552 in decode and 1.3–15.3× in prefill, on the same weights. |
| Gains from MoE-aware routing, memory layout, and quantization | **True** | See [How Maya gets its speed](#how-maya-gets-its-speed). |
| V4.1 Flash 1017 support "within hours" | **False for Maya** | DeepSeek published the checkpoint at 2026-10-17T02:40Z. Maya's first DeepSeek pull request (#271) opened 2026-10-18T16:13Z, about 37 hours later, and shipped in v1.1.16 on 2026-10-19T16:55Z, about 62 hours later. It used an EXL3 quant that appeared 11 hours after the checkpoint. |
| A 56-commit browser game built with GLM 5.3 on Maya | **Unverified** | The post links no repository. Not relevant to us. |

The posts don't link Maya's repository and name no tester. Read them as
marketing built on Maya's README and the PTT thread. The repository's own
claims are more careful, and its GLM claims are backed by files: KL
against FP8, same-top-token rates, perplexity, and zero-shot accuracy
(`bench/results/MAYA-S.md`, `MAYA-L.md`).

The V4.1 claims are thinner. The Maya-S model card says it picks the same
next token as the original 92.6% of the time, KL 0.120, on 10 held-out
sequences of 2,047 tokens. The reference was DeepSeek's implementation on
the original weights. The engine itself, unquantized, scores 96.4% and KL
0.047 against that reference. No evaluation file or script for V4.1 is in
the repository. The V4.1 calibration and error-feedback pipeline isn't
either: `tools/maya_quant/` is GLM-only, and `tools/ds4/requant_gguf.py`
calls itself "a quick test model ... until the real Maya quants exist".

## How Maya gets its speed

### Expert tiers and placement

V4.1 has 40 layers of 384 routed experts, 6 active per token. Native FP4
experts are 288.8 GB, about 18.8 MB each. One decode token touches 240 of
them, about 4.5 GB.

- **Tiers.** VRAM holds the hottest experts. The rest are read in place
  from the GGUF through the page cache: from RAM where they fit, else
  from the SSD (`src/ds4/ds4_model.hpp:31-33`, `docs/SETUP.md:25-31`). A
  pinned RAM tier exists but is off by default; the page cache measured
  as fast or faster (`ds4_model.cu:1102-1107`).
- **Initial placement.** `place_experts()` (`ds4_model.cu:4016`) gives
  each layer the same share of VRAM slots and fills them with that
  layer's most-routed experts from `expert-profile.bin`. The profile is
  seeded from a shipped file, `data/expert-profile-ds4.bin`, that weighs
  as much as about 2,000 tokens of use (`docs/SETTINGS.md:439-446`). The
  drafter's experts go first.
- **Online swaps.** A CPU-lane expert replaces the coldest VRAM expert
  when its heat passes the VRAM expert's heat × 1.25 + 1.0. Heat decays by
  0.9 every 256 tokens, with up to 8 swaps in flight
  (`ds4_model.cu:1096-1101`). Swaps are prepared on 4 threads of their own
  and applied between windows (#276). For GLM, the maintainers compared
  policies on recorded routing: LRU 67%, LFU 77.5%, decayed frequency 78%,
  a small learned model 77%, and an oracle 87% hit rate
  (`bench/results/PROGRESS.md`).
- **The CPU lane.** Decode posts each layer's routing to a host mailbox;
  CPU threads compute the non-VRAM experts, and the GPU waits only before
  the expert sum (`ds4_model.cu`, around line 1053). Layers whose experts
  are all in VRAM skip the host entirely (v1.1.21, +9%).
- **CPU kernels.** MXFP4 experts round activations to int8 per 32 values,
  to match the GPU kernel, and use AVX2 `vpmaddubsw` or AVX-VNNI and
  AVX-512 VNNI `vpdpbusd` (`src/ds4/ds4_gguf_cpu.cpp:132-251`). The
  fastest tier is measured at first use. All tiers sum in the same order,
  so they agree bit for bit. Low-bit i-quants use ggml-cpu plus Maya's own
  AVX-512 VBMI IQ3 rows.
- **The PCIe lane** (v1.1.18). Some non-VRAM experts are streamed into
  GPU staging slots instead of computed on the CPU. The split is chosen
  per token from measured CPU, disk, and copy times kept in
  `lane-times.txt`. Streaming is dropped unless it's at least 3% faster
  (`docs/SETTINGS.md:366-387`).
- **Prefetch.** While layer L runs, the GPU applies layer L+1's gate to
  layer L's FFN input to guess L+1's experts, and prefetches the guessed
  experts that live in files. Hit rates by rank are 97, 91, 81, 70, 57,
  and 41% (`ds4_model.cu`, around line 1070). For GLM, the same idea for
  PCIe uploads was measured as a loss.
- **Threads.** One thread per physical core, one pool per NUMA node. SMT
  siblings are left to the disk and Engram readers. The engine times
  layouts at load and keeps one only if it's more than 5% faster
  (`docs/SETTINGS.md:423-437`). For GLM, hyperthreads on the CPU lane cost
  21.8 → 13.3 tok/s.
- **Light-expert skipping** is an opt-in that changes answers
  (`MAYA_DS4_DISK_SKIP_W`). We wouldn't use it.

Issue #280 shows where this design breaks. With 161 GB of experts read in
place on a 2× RTX 3090 Windows PC, decode ran at 18.6–24 tok/s, but the
CPU lane did only 0.4–2% of expert uses once the cache warmed. The first
answer after a start was 13 tok/s.

### The quant

There is no format spec. Every Maya quant is a recipe over ggml block
types with 256-value superblocks, so the files are ordinary GGUF.

V4.1 Maya-S (339.5 GB in 6 files: the model in 178.2 GB, Engram in
161.3 GB):

| Tensors | Format |
| --- | --- |
| Routed experts, gate and up | IQ2_XXS (about 2.06 bits); IQ2_S in the first and last four layers |
| Routed experts, down | IQ3_XXS (about 3.06 bits); IQ3_S in the first and last four layers |
| Attention, shared experts, embeddings, output, Engram value projection | Q8_0 |
| Router, norms, hyper-connection mixing, attention sinks | F32, as released |
| Indexer's small projections | BF16, as released |
| Engram tables | Q6_K, from DeepSeek's FP8 tables |

- **Calibration.** 128 sequences of 2,048 tokens in DeepSeek's chat
  template, thinking included: chat, multilingual chat, reasoning, code,
  and tool calls. An importance matrix for every expert separately, from
  the original model's own activations.
- **Rounding.** GPTQ-style error feedback at the quantizer's 256-weight
  blocks, each expert against its own input statistics, after the
  ISTA-DASLab recipe. The card claims a 35–50% lower expert output error
  than plain importance-weighted rounding at the same size. The GLM
  tooling that does this is `tools/maya_quant/gptq.py`.
- **Outliers.** No separate outlier handling beyond the importance matrix
  and GPTQ.
- **The native FP4 experts** are decoded to F32 (MXFP4: an E8M0 scale and
  16 bytes of E2M1 per 32 values) and requantized through
  `ggml_quantize_chunk` (`tools/ds4/requant_gguf.py:265-339`). The GPU
  path can also run MXFP4 natively, "every product an F32 sum of exactly
  dequantized weights" (`src/ds4/ds4_gguf.cuh:1-12`). The DSpark drafter
  keeps DeepSeek's FP4 experts and moves its FP8 dense weights to Q8_0
  (7.97 GB).
- **Maya-XS** (IQ1_S experts) is listed on Hugging Face with only its
  Engram shards so far.

### Prefill

- **Chunks.** Prompts run in chunks of 2,048 tokens, or 8,192 when
  experts are read in place (`ds4_model.hpp:41-43`). On one GPU, chunks
  grow to 16K or 32K by borrowing expert VRAM. Dense GEMMs use FP16
  reconstructions of the weights on cuBLAS. Expert GEMMs sort rows by
  expert and run in sets of 32 experts.
- **Experts read once per layer.** A multi-chunk prompt runs layer by
  layer through all its chunks, with a per-layer RAM cache, so each
  expert is read once per prompt. A 16K prompt read 264 GB instead of
  736 GB (`CHANGELOG.md:142-144`).
- **Expert streaming.** For each chunk and layer, only the routed experts
  stream: likely ones are queued from the profile while the previous
  layer computes; reads go from disk with `O_DIRECT` into pinned staging,
  then by DMA into a VRAM ring (`ds4_model.cu`, around line 605). Eight
  reader threads per GPU took the copy from 1.3 GB/s on one thread to the
  disk's speed (#271).
- **Prompt skip** (v1.1.20, on by default on one GPU, prompts of about
  3,700 new tokens or more). The repository's argument: layers 20–39 see
  earlier tokens only through layer 20's compressed KV and index keys,
  and each looks back one 128-token window, so they need full work only
  on the last ~2,700 tokens. Before that, layer 20 computes only what
  later layers read, and layers 21–39 do nothing. It claims the same
  answers up to summation order and +30% on a 32K prompt
  (`docs/SETTINGS.md:389-402`). DeepSeek's own serving runs layers 20–39
  on the last 128 tokens only, which changes answers; mode `2` copies it.

**Inference:** our model notes list index sources at layers 24, 28, 32,
and 36 as well as 20. If those layers compute their own index keys for
earlier positions, skipping them isn't exact. Check this against the
oracle before trusting the claim.

The fix that mattered most: until v1.1.17, the V4.1 setup's tensor
parallelism silently disabled chunking, so every prompt ran in windows of
16 tokens. On 7× RTX 3090 with Maya-S, a 32K prompt went from 99 to 1,010
tok/s when chunking came back (#279, `CHANGELOG.md:133-137`).

### Attention, KV, and long context

- The layer follows DeepSeek's `inference/model.py`: lagged-pre
  hyper-connections, sliding-window plus compressed sparse attention,
  the two-level indexer with cross-layer KV, index, and candidate
  sharing, Engram rows, `sqrt(softplus)` routing in F32, and DSpark
  drafting (#271; `ds4_kernels.cuh:59-97`). No hash routing.
- **KV storage** (`ds4_kernels.cuh:18-31`): the window ring in FP8 E4M3
  with a UE8M0 scale per 32; compressed KV in FP4 E2M1 with an E4M3
  scale per 16; index keys in FP4 with a power-of-two scale per 32. #276
  stores these as packed rows, a quarter of BF16's bytes, and its test
  reads them back bit for bit as the BF16 rows (`tests/ds4_kv_fp4_parity.cu`).
  The repository calls this lossless because DeepSeek's reference already
  rounds to these grids.
- **No KV offload for V4.1.** GLM has KV streaming to pinned RAM; V4.1
  doesn't.
- **Engram** rows are hashed as DeepSeek's `engram.py` does
  (`tools/ds4/engram_hash.py`) and read asynchronously, off the critical
  path, through a clock-eviction row cache (`src/ds4/ds4_engram_io.hpp`).
  The tables stay on the SSD; each token reads a few rows.
- **Long context.** #271 found a passphrase 5% into a 116,942-token
  prompt at 262,144 context. Deterministic top-k (#276) and a pinned
  cuBLAS workspace keep long-prompt logits identical between starts.

### Multi-GPU and calibration

- Layers split across GPUs; runs of equal GPUs also split each layer by
  tensor parallelism (`--tp 2` by default, 4 and 8 in #276): heads, output
  groups, and every expert's FFN columns. Members sum through host-mapped
  memory, with no P2P. With three or more members, a reduce-scatter and
  all-gather in FP16 took 156 µs to 35 µs on 8 GPUs at 5 tokens.
- `./maya.sh --calibrate` measures each `--tp` grouping and the CPU
  thread count on the PC, takes a winner only if it beats the baseline by
  more than 5%, and re-runs wins under 15% (#276).
- AMD ROCm builds from the same sources (gfx1100, gfx1201, gfx1151).
  Windows is experimental.

### Maya's own V4.1 numbers

| Machine | Quant | Decode | Prefill | Source |
| --- | --- | ---: | ---: | --- |
| RTX 3090 + 8× RTX 5060 Ti 16 GB, 2× Xeon Gold 6152, 183 GB | EXL3 2.9 bpw | 40–43 tok/s; 70–77 with DSpark on code | 754 tok/s at 16K; 942 at 117K | #271 |
| Same | EXL3 3.5 bpw, `--tp 4` | 54.1 tok/s | 488 tok/s at 16K | #276 |
| One RTX 3090, 64 GB cgroup, 12 threads (simulated) | EXL3 2.9 bpw, experts read in place | 4.9 tok/s | 49.7 tok/s | #271 |
| 2× RTX 4090 48 GB (modded), EPYC 7713P | Maya-S | 26–53 tok/s | 408–786 tok/s | #280 |
| 4× RTX 3090 + RTX 5060 Ti, EPYC 7663, 512 GB | Maya-S | 41.1 tok/s | 85–105 tok/s (before the #279 fix) | #280 |
| 2× RTX 3090, Threadripper 3990X, 256 GB, Windows | EXL3 2.9 bpw | 18.6–24 tok/s | 8–109 tok/s | #280 |
| RX 7900 XTX 24 GB, Ryzen 9 7900X3D, 64 GB, Windows | Maya-S | 4.4–5.8 tok/s | 120–144 tok/s | #280 |
| 7× RTX 3090 | Maya-S | not given | 1,010 tok/s at 32K | `CHANGELOG.md:133-137` |
| Radeon 8065S (Ryzen AI Max), 192 GB | Maya-S | 12.0–12.8 tok/s | not given | `ds4_main.cpp:160-165` |

For comparison, Xolotl reports 26.8 tok/s decode and 178–320 tok/s
prefill on one RTX 4090 with 90 GB of DDR4, using its own 152 GiB "GSQ"
mixed ternary, 2-bit, and 3-bit quant, measured while solving 21
HumanEval, 10 tool-call, and 3 agentic tasks.

**Inference:** the rule across these rows is memory, not GPU speed. With
RAM plus VRAM above the quant's size, a consumer PC decodes V4.1 at
roughly 20–55 tok/s. When experts come off the SSD, decode falls to about
5 tok/s and prefill to about 50–150 tok/s.

## Our measurement

We ran Maya once on a rented Google Compute `g4-standard-48` spot machine
in `asia-northeast1-b`: one RTX PRO 6000 Blackwell (96 GB), an AMD EPYC
9B45 (24 cores, 48 threads, AVX-512), 176 GiB of RAM, and a 900 GB
Hyperdisk Balanced boot disk. It ran Maya at `451f9cb6a2` with Maya-S
(revision `276e0bad`), 65,536 context, through the spend guard
(`scripts/dev/cloud-budget.py`). The machine ran from 01:11Z to 02:53Z on
2026-10-22 and cost $2.84 in compute; it and its disk are deleted. The
steps were `./maya.sh --setup --yes --download-model --model
DeepSeek-V4.1-Flash-1017 --ds4-quant maya-s --context 65536`, then
`./maya.sh --bench`. Setup took 87 minutes, almost all of it the 339.5 GB
download from Hugging Face at about 95 MB/s; the engine built in about 4
minutes. Our regional copies hold DeepSeek's original safetensors, which
Maya-S can't use.

What the engine did with the memory:

- Tensor parallelism fell back to one GPU, as expected with one card.
- The VRAM took 7,393 of the 15,360 routed experts (48%), by the shipped
  profile. The other 7,967 (87.6 GB) stayed on the CPU lane, read in
  place and all read ahead into the page cache, so nothing came from the
  disk after load. Prompts ran in chunks of 32,768.
- Its own timing at start: one 10.6 MB expert took 0.15–0.40 ms on the
  CPU lane (24 threads), 0.34 ms streamed over PCIe, 0.005 ms on the GPU,
  and 5.3 ms from the disk. Decode measured the PCIe lane as slower than
  the CPU lane alone (1.09 against 0.71 ms per request) and stopped
  streaming.
- During decode the CPU lane served 16–26% of expert uses; during prefill
  1.7–5.7%.

| Measurement | Result |
| --- | --- |
| Load time | 56 s (warm page cache) |
| Decode, setup's calibration, no drafting, 3 × 384 tokens | 35.3 tok/s sustained (32.3–37.3) |
| Decode, setup's calibration, DSpark on, 3 × 384 tokens | 53.6 tok/s sustained (49.5–58.9), 72% of drafts kept |
| Decode, `--bench`, DSpark on, greedy, thinking off, 3 × 256 tokens | 48.1, 29.6, and 22.4 tok/s (mean 33.3) for code, prose, and HTML |
| Prefill, 2,048 tokens | 214 tok/s (134 tok/s on the first, cold run) |
| Prefill, 8,192 tokens | 292 tok/s |

Against the claims: one 96 GB card with 176 GB of RAM decodes Maya-S at
22–54 tok/s, well above the 11.1 tok/s claimed for 2× RTX 3060, which has
a quarter of the VRAM. Prefill at 8K is 292 tok/s, a quarter of the
claimed 1,100–1,500, and in line with the repository's own V4.1 rows at
8K–16K. The 1,100–1,500 range is a GLM number. We didn't measure quality
or prompts longer than 8K.

For scale, Psionic's four-stage group on four of these cards decodes at
29.7 tok/s with native weights, no drafting, and about 120 tok/s of
prefill per stage. Maya used one card, a 2–3-bit quant, and DSpark.

## Psionic today, compared

Psionic's V4.1 path (`docs/psionic/deepseek-v41-completion.md`):

- **Stage groups.** Four CUDA stages of 10 layers each on RTX PRO 6000s,
  about 75 GB of native FP8 and FP4 weights per stage, linked by
  `stage-link` (W12, #12399). One request at a time decodes at 29.1–29.7
  tok/s on one or several machines. Engram tables stay in pinned host RAM
  on the stages for layers 1 and 14.
- **Host experts (#12418).** `--experts host` keeps a stage's routed
  experts in page-locked host memory; GPU kernels read them over PCIe
  through mapped memory. No CPU arithmetic is involved, and the outputs
  are bit-equal to device experts. A layer then needs 0.17 GB of VRAM
  instead of 7.4 GB. On a home RTX 4080, a layer took about 7 ms against
  0.8 ms on an RTX PRO 6000. Owner rule (2026-10-21): no CPU stages in
  groups.
- **Metal (#12420).** M1–M5 are done. An M5 Max takes 9.1 ms for a
  two-layer greedy step. A group with four RTX PRO 6000s and the Mac ran
  at 7.8–9.0 tok/s, mostly link time.
- **Kernels.** Portable block-scaled FP8 and FP4 matrix-vector kernels on
  sm_80, sm_89, sm_90, and sm_120: about 1.1 TB/s for FP8 and 1.0 TB/s
  for six FP4 experts on SM120. No tensor-core GEMM path.
- **Prefill.** Serial, about 120 tok/s per stage, about 33 ms per prompt
  token over four stages. Chunked long-prompt prefill (#12414) is
  opt-in and unqualified.
- **Qualification.** G1 v2 compares the served runtime against the B200
  oracle. No profile is qualified yet; the 2.1 run failed on router
  layer 1. No lossy or consumer profile exists.

Where Maya is ahead:

| Area | Maya | Psionic | Gap |
| --- | --- | --- | --- |
| Single box with less VRAM than the model | VRAM hot cache, CPU or PCIe lane for cold experts, SSD tier | All experts on the GPU, or all of a stage's experts behind PCIe | Psionic needs RAM plus VRAM for all 289 GB of native experts and 203 GB of Engram; no home PC has that. |
| Which experts sit on the GPU | Profile-seeded, decayed-frequency swaps | Static: every expert, or none | No cache. |
| Expert bytes | 2–3 bits (178 GB model) | Native FP4 and FP8 (308 GB without Engram) | 1.7× more bytes per token for us, by design. |
| Engram | Q6_K on the SSD, a few rows a token through a row cache | 101 GB per table in pinned RAM | 203 GB of RAM we could free. |
| Prefill | Chunked, layer-major, experts read once per layer, grouped expert GEMMs | Serial matrix-vector, about 120 tok/s | 4–10× on comparable hardware. |
| KV | Packed FP4 compressed KV, FP8 window | Larger records (3,200 bytes per position; 184 KB step buffers per token) | Already proposed in the Flare notes. |
| Multi-GPU in one box | Tensor parallel through host-mapped memory | Layer stages over TCP | Not needed on RTX PRO 6000 groups. |
| Tuning | `--calibrate` per PC | Fixed profiles | Useful for consumer Pylons. |

Where Psionic is ahead: native-precision weights, bit-exact parity tests
against an independent oracle, a qualification contract, groups across
machines, Metal, and Pylon advertising. Maya serves one machine and makes
no exactness promise for its quants.

## Harvest plan: Maya → Psionic

Hardware columns: **PRO** is an RTX PRO 6000 group or a single
`g4-standard-48`; **Mac** is an M5 Max (128 GB unified); **Home** is a
consumer Pylon, for example one 24–32 GB RTX card and 128–192 GB of RAM.
Effort is S (days), M (one to two weeks), or L (more). License column:
"Reimplement" means we take the design only; all sources are MIT, so
copying with the notice kept is allowed but not planned.

| Rank | Technique | PRO | Mac | Home | Effort | Exactness and qualification | License |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 1 | **Hot-expert VRAM cache over host experts.** Per-layer slots, seeded from a routing profile, decayed-frequency swaps off the critical path. Extends `--experts host`. | With rank 3, one `g4-standard-48` serves V4.1 alone, as Maya did in our run; frees cards in groups | None (unified memory) | Makes single-box V4.1 possible; with 80–95% hit rates, PCIe traffic falls 5–20× | M | Exact: same weights, same kernels. Qualifies as a device class. | Reimplement (Maya MIT) |
| 2 | **Layer-major chunked prefill with grouped expert GEMMs.** Sort chunk rows by expert, run each expert's rows as one tensor-core GEMM, stream non-resident experts once per layer per prompt. | 4–10× prefill (120 → 500–1,200 tok/s per stage) | Same idea with Metal matrix units | Turns 50 tok/s SSD-bound prefill into usable TTFT | L | Summation order changes; must pass G1 v2 tolerance like any kernel change. Builds on #12414. | Reimplement |
| 3 | **`consumer` low-bit expert quant.** Per-expert importance matrices and GPTQ error feedback to 2–3 bits for routed experts; dense, router, hyper-connection, and indexer tensors at their native precision. | Lets one 96 GB card plus 128 GB RAM hold the whole model | Fits 178 GB toward a 192 GB Mac Studio | Core of the home offer | L | **Lossy. A separate profile, never qualified by G1 v2.** Needs its own gate: KL and top-1 against the oracle, plus G2 tasks. | Reimplement from the GPTQ paper; Maya's GLM tooling is MIT |
| 4 | **Engram on the SSD with a row cache.** Read the few rows a token needs with direct I/O, prefetch from the hash, keep a clock cache. Optionally Q6_K-style compression of the tables. | Frees 101 GB pinned RAM per Engram stage | Frees unified memory for experts | Required: home PCs can't pin 203 GB | M | Exact with FP8 tables. Compressed tables are lossy and belong to the consumer profile. | Reimplement (also in the Flare notes) |
| 5 | **Packed FP4 compressed KV and index keys, FP8 window.** | 3–4× less KV memory; longer contexts per stage | Same | Long context on 24 GB cards | S | Exact if the reference rounds to the same grids; verify against the oracle. | Reimplement |
| 6 | **Next-layer expert prediction for prefetch.** Apply layer L+1's gate to layer L's FFN input to start loads early. | Small | None | Hides SSD and PCIe latency on cache misses | S | Exact: a hint only. | Reimplement |
| 7 | **Prompt skip for layers 20–39.** | Up to +30% on long prompts | Same | Same | S | Claimed exact; our index-source reading says it may not be. Verify first. | Reimplement |
| 8 | **Per-machine calibration.** Measure CPU, disk, and PCIe rates and the cache size at first start; record them in the Pylon's advertised profile. | Small | Small | Needed for honest advertised rates | S | None | Reimplement |
| 9 | **Tensor parallel inside one box over host-mapped memory**, for PCs with several equal consumer cards. | None | None | 2–4 card home rigs | L | Reduction order changes; G1 v2 tolerance. | Reimplement |

Not worth taking:

- **CPU expert lane, for now.** It's Maya's default cold path, and on
  our test machine it beat PCIe streaming (0.71 against 1.09 ms per
  request of cold experts). It conflicts with the owner rule that groups
  use GPU stages only, though, so rank 1 streams misses over PCIe as
  `--experts host` does today. Ask the owner whether computing cold
  experts on the host inside a GPU stage counts as a CPU stage. If it
  doesn't, Maya's AVX-512 VNNI MXFP4 kernels are the model, and the gain
  is about a third off each cache miss.
- **Light-expert skipping** and DeepSeek's own last-128-tokens prompt
  mode: they change answers.
- **DFlash2**: CC BY-NC-ND.

The top three for the Pylon network are ranks 1, 2, and 3: the cache
makes a single box possible, prefill makes it pleasant, and the quant
makes it fit a home PC.

## A `consumer` profile for a home Pylon

**Inference:** a sketch, not a design.

1. **Profiles.** Keep `qualified` (native weights, G1 v2) as is. Add
   `consumer-exact` (native weights, ranks 1, 2, 4, and 5) for workstations
   with about 300 GB of RAM plus VRAM, and `consumer-q` (rank 3's quant,
   about 180 GB) for home PCs with 128–192 GB of RAM. Both are single
   stage, layers 0–39 on one GPU.
2. **Memory plan at start.** Dense weights, KV, and step buffers go in
   VRAM first; the hot-expert cache takes the rest of VRAM, seeded from a
   shipped profile. Cold experts stay in pinned or page-cache RAM, and
   anything beyond RAM is read from the SSD. Engram is on the SSD.
3. **Decode.** For each layer, look up the 6 routed experts. Hits run from
   VRAM. Misses stream over PCIe into a staging ring (as `--experts host`
   does today) while the next layer's guess prefetches. Swap experts in
   between steps.
4. **Prefill.** Chunked, layer-major; each non-resident expert streams
   once per layer per prompt.
5. **Advertising.** The head beacon's `group` block (kind 30200) gains a
   `profile` value (`qualified`, `consumer-exact`, or `consumer-q`), the
   quant's digest in `artifact`, and the calibrated tok/s and TTFT in
   `answer_profile`. Buyers and the router match on profile, so a request
   for the qualified model never lands on a lossy Pylon.
6. **Gate.** `consumer-exact` qualifies per device class as host experts
   do now. `consumer-q` gets a published quality card (KL and top-1
   against the oracle on held-out text, G2 task scores) and is labeled
   lossy wherever it's shown.

Expected rates, from Maya's and Xolotl's numbers: 20–30 tok/s decode on
one 24–32 GB card with 90–192 GB of RAM and the quant; 22–54 tok/s on one
96 GB card with 176 GB of RAM, as we measured. Prefill of several
hundred tok/s once rank 2 lands.

## Proposed issues

Filed on 2026-10-22 under the Maya harvest epic
[#12462](https://github.com/OpenAgentsInc/openagents/issues/12462) as
#12463–#12471; progress and measurements are in
[DeepSeek-V4.1 on one machine](../../psionic/v41-single-box.md). The drafts
below are kept as written.

### 1. Psionic: hot-expert VRAM cache for V4.1 host experts

> Extend `--experts host` (#12418) with a per-layer VRAM cache of routed
> experts. Seed it from a routing profile shipped with the bundle; swap
> experts by decayed frequency (decay per 256 tokens, a margin over the
> coldest resident) on a separate stream between steps. Record hit rates
> and swap counts in stage metrics.
>
> Done when: a single `g4-standard-48` serves all 40 layers with experts
> in host memory and a cache in the rest of VRAM; outputs equal the
> device-expert run bit for bit; decode tok/s and hit rate are recorded
> for 4K and 32K conversations. Design after Project Maya
> (`mw00/project-maya` `451f9cb6a2`, `src/ds4/ds4_model.cu`), reimplemented.

### 2. Psionic: layer-major chunked prefill with grouped expert GEMMs

> Run prompt chunks layer by layer. Within a layer, sort chunk rows by
> routed expert and run each expert's rows as one block-scaled GEMM on
> tensor cores (sm_120 first). Stream each non-resident expert once per
> layer per prompt. Builds on #12414.
>
> Done when: one RTX PRO 6000 stage prefills at least 4× its current
> 120 tok/s at 8K and 32K; G1 v2 logits stay within tolerance; the
> four-stage group's TTFT for a 2,872-token prompt is recorded.

### 3. Psionic: `consumer-q` low-bit expert quant for V4.1 (unqualified profile)

> Quantize routed experts to 2–3 bits with per-expert importance matrices
> and GPTQ error feedback, keeping dense, router, hyper-connection, and
> indexer tensors native. Add the matching CUDA and Metal expert kernels.
> Ship it as a separate artifact with its own digest. It is never served
> under the qualified profile.
>
> Done when: the artifact is published to our buckets with a quality card
> (KL and top-1 against the B200 oracle on held-out text, G2 suite
> scores); a single 24–32 GB consumer card with 192 GB of RAM serves it.

### 4. Psionic: Engram tables on SSD with a row cache

> Read Engram rows with direct I/O and a clock-eviction row cache instead
> of pinning 101 GB per table. Prefetch from the n-gram hashes as soon as
> tokens are known.
>
> Done when: an Engram stage runs with less than 8 GB of RAM for Engram,
> outputs are bit-equal to pinned tables, and per-token latency is
> recorded on Hyperdisk and on a consumer NVMe drive.

### 5. Psionic: packed FP4 compressed KV and index keys

> Store compressed KV as FP4 E2M1 with an E4M3 scale per 16 and index keys
> as FP4 with a power-of-two scale per 32, packed, when the oracle rounds
> to the same grids.
>
> Done when: oracle parity holds bit for bit on the attention suites and
> per-position KV bytes are recorded.

### 6. Pylon: advertise serving profiles for V4.1

> Add a `profile` field to the group head beacon and route only to
> matching profiles. Advertise calibrated decode and TTFT from a first-start
> measurement.
>
> Done when: a `consumer-q` Pylon and a qualified group run side by side
> and requests reach only the profile they ask for.

### 7. Psionic: check Maya's layer 20–39 prompt skip against the oracle

> Maya claims that layers 21–39 need full work only on a prompt's last
> ~2,700 tokens. Our notes list index sources at layers 24–36. Check the
> claim on the CPU reference; if it's exact, add it to chunked prefill.
>
> Done when: the claim is confirmed or refuted with a recorded logit
> comparison on a 16K prompt.
