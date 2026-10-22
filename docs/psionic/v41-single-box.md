# DeepSeek-V4.1 on one machine

Status: built and measured on 2026-10-22 under the Maya harvest epic
([#12462](https://github.com/OpenAgentsInc/openagents/issues/12462)). Nothing
here changes the qualified profile or the pinned bundles: every feature is
behind a new flag, and a stage without them loads and runs as before.

The designs follow [Project Maya](../research/deepseek/2026-10-21-project-maya.md)
(`mw00/project-maya` at `451f9cb6a2`, MIT), reimplemented in Rust over
Psionic's own kernels. DFlash2 (CC BY-NC-ND) is not used.

## What a stage can do now

| Flag | Issue | What it does | Exact? |
| --- | --- | --- | --- |
| `--experts file` | #12463 | Reads routed experts in place from the mapped checkpoint shards; the page cache keeps the warm ones. Needs `--expert-cache`. | Yes |
| `--expert-cache auto\|GB\|slots:N` | #12463 | Keeps each layer's hottest routed experts in device slots. `auto` fills the device memory left after everything else, less 3 GiB. | Yes |
| `--miss-lane auto\|bus\|host` | #12463 | Where cache misses run: read over the bus from page-locked memory (`--experts host` only), or on host CPU threads. `auto` measures one expert both ways at load and keeps the bus only when it's at least 3% faster. | Yes |
| `--expert-profile FILE` | #12463 | Seeds each layer's slots with a routing profile's hottest experts. `chain --save-expert-profile FILE` writes one. | Yes |
| `--host-threads N`, `--frozen-cache` | #12463 | Host-lane threads (default: one per core); no swaps after load. | Yes |
| `--engram file` | #12466 | Reads Engram rows from the mapped shards: the host hashes each step's tokens and stages their rows before the step, so neither 101 GB table is pinned. | Yes |
| `--prefill layer-major` | #12464 | Runs a prompt step layer by layer: projections and routed experts batched over the step's tokens, uncached experts streamed once per layer through a device ring. | Yes |
| `--kv packed` | #12471 | Keeps the window ring as FP8 codes with a power-of-two scale per 32, and compressed KV and index keys as FP4 codes with a scale per 16 or 32. A whole-model stage only. | Yes |
| `--experts file --consumer-q DIR` | #12465 | Serves a requantized 2- or 3-bit expert artifact under its own profile name. | **No: lossy, unqualified** |
| `--dspark` | #12469 | Uses the matching checkpoint's three draft blocks and verifies five proposed tokens together. A whole-model CUDA stage or `chain --split 0-40` only; enables layer-major work. | Greedy tokens stay equal to ordinary decode; sampled output uses target verification. |

A stage with any of these flags announces its own profile:
`consumer-exact-<profile>` (bit-equal answers, its own configuration) or
`consumer-q-<profile>`, so it never joins a qualified group (#12467). At
load it logs its memory plan: slots per layer, cache bytes, the routed
experts left to the host tier and whether they fit in available RAM, and
each lane's measured cost per expert. Pylon group beacons carry the
matching `profile` (`qualified`, `consumer-exact`, `consumer-q`), and a
request reaches only the profile it names (#12468).

"Exact" means the tokens, logits, and per-layer observations equal the
ordinary stage's bit for bit. The CUDA tests check this on the synthetic
model (`cached_experts_equal_device_experts`,
`staged_engram_rows_equal_pinned_tables`,
`layer_major_prefill_equals_the_ordinary_step`, and the chain tests in
`psionic-v41-stage/tests/cuda.rs`); the host lane's scalar, AVX2, and
AVX-512 paths equal the device kernels' host model.

### How the cache works

- Each layer gets the same number of device slots. A slot map on the
  device gives every expert a slot, the bus lane, or the host lane, so the
  matrix-vector kernel picks each selected expert's weights itself and a
  decode step stays one captured graph.
- After routing, a kernel posts the token's FP8 activation and the
  selected ids to a mailbox in page-locked memory. A poller thread computes
  the host-lane experts on the host lane's threads, in the GPU kernel's
  summation order, while the GPU runs the cached ones; a second kernel
  waits for the answer before the expert sum.
- Heat counts each expert's uses and decays by 0.9 every 256 tokens. A host
  expert replaces the coldest cached one when its heat passes that
  expert's heat × 1.25 + 1. A thread stages the incoming expert during a
  step; the copy into its slot happens between steps.

## Measurements

One spot `g4-standard-48` in `asia-northeast1-b`: one RTX PRO 6000
(97,887 MiB), 48 vCPUs (AMD EPYC, AVX-512), 176 GiB of RAM, and a 900 GB
Hyperdisk Balanced at 2,400 MiB/s and 40,000 IOPS. Checkpoint copied from
`gs://openagents-psionic-models-ane1/` (476 GB in 5 minutes). All runs are
`psionic-v41-stage chain --split 0-40 --gpus 0` unless noted, greedy, with
`--experts file --expert-cache auto --engram file`.

The cache took 116 slots per layer (30% of the routed experts, 87 GB).
The other 70% (about 204 GB) don't fit the 176 GiB of RAM beside the
system, so some host-lane misses read from disk.

| Run | Prompt | Decode | Hit rate | Notes |
| --- | --- | ---: | ---: | --- |
| a1: no profile, third answer | 51 tokens | 19.8 tok/s | 70% (all rounds) | First answer after load: 4.6 tok/s |
| a2: profile from a1, third answer | 51 tokens | 24.3 tok/s (p50 40.8 ms, min 33.6 ms) | 89% | |
| a3: profile from a2 | 2,129 tokens | 14.3 tok/s (p50 68 ms) | 62% | A document prompt routes differently from the profile's chat |

- **Lanes.** At load one expert took 38 µs from a device slot and
  370–460 µs on the host lane (48 threads, pages partly on disk). A host
  request (a layer with any host-lane expert) averaged 2.9 ms, mostly page
  faults. The bus lane isn't available here: it needs every expert
  page-locked.
- **Engram from the shards.** 48 rows a decode token; with parallel reads
  and random-access advice, 3.1 s across 921 tokens (a2), down from 13.1 s
  with one read after another (a1).
- **Prefill.** Token by token, the single box prefilled the 2,129-token
  prompt in 131 s warm (16 tok/s). Layer-major with uncached experts
  streamed through the ring took 66.7 s warm (32 tok/s), of which the host
  copies of 23,364 streamed experts took 52 s a round.
- **One 10-layer stage** (layers 0–9, experts on the device, 2,129-token
  prompt, `--first-stage-only`):

  | Prefill | tok/s |
  | --- | ---: |
  | Token by token | 121.6 |
  | Layer-major, projections batched | 148 |
  | + token-independent work batched | 192 |
  | + attention batched over the step | 250 (9,449-token prompt: 248) |
  | + dense products tiled over tokens | **541** (4.45×); 9,449-token prompt: **548** (4.5×) |

  Phase times of the last run (two rounds): routed experts on the device
  58%, output projection 13%, index projections 13%, attention 9%. The
  grouped output projection had been 53% before the dense kernel was tiled.
- **Single box, layer-major** with the tiled kernels: the 2,129-token prompt
  in 25.6 s warm (83 tok/s, up from 32), and the 9,449-token prompt in 144 s
  warm (65 tok/s, up from 42), each with the same answer as before. On the
  real model, teacher-forced logits of the oracle's 2K prompt are bit-equal
  between token-by-token and layer-major prefill (four answer steps).
  What remains is mostly copying uncached experts: with 70% of them off the
  device and about 204 GB of them against 176 GiB of RAM, the copies read
  from the disk (2,400 MiB/s).
- **Exact profile ceiling.** A decode token with every expert hit takes
  about 33 ms on this card (four 10-layer stages' worth of the device
  graph), so the exact profile can't pass about 30 tok/s here; Maya's 35
  tok/s used a 2–3-bit quant.

Answers were the same text as the device-expert runtime's ("The sky
appears blue because sunlight is scattered by gas molecules…").

## Packed KV rows (#12471)

The reference already rounds every KV row to a small grid, so the stage can
keep the codes and scales instead of BF16 words and decode them on read to
the same words:

| Rows | BF16 bytes a position | Packed | Share |
| --- | ---: | ---: | ---: |
| Window ring (FP8 E4M3, UE8M0 per 32) | 1,024 | 528 | 52% |
| Compressed KV (FP4 E2M1, E4M3 per 16) | 1,024 | 288 | 28% |
| Index keys (FP4 E2M1, UE8M0 per 32) | 256 | 68 | 27% |

The CUDA tests run the whole synthetic model with packed rows, token by
token and layer-major, and its tokens and logits equal the BF16 rows'. On
the real model, one box, the oracle's 2K prompt teacher-forced through four
answer steps gives bit-equal logits with `--kv packed` and without. A
stage boundary still carries BF16 rows, so only a whole-model stage takes
`--kv packed`. Design after Project Maya PR #276 (`mw00/project-maya` at
`451f9cb6a2`, MIT), reimplemented.

## The one-box placement (#12467)

`pylon cloud up one-box` runs all 40 layers on one `g4-standard-48` through
the Pylon front. Its manifest plans the stage with `experts: file`: the
weight bytes leave out the routed experts, and neither the experts nor the
Engram tables set RAM aside, since both are read from the mapped shards
(`psionic-v41-stage plan --device m0:0:cuda-sm_120:96000000000:file-experts`).
Pylon gives the stage `--experts file --expert-cache auto --engram file
--prefill layer-major --kv packed`, a 700 GB disk for the whole checkpoint,
and declares the `consumer-exact` profile on the group's beacons.

Served once through the Pylon front (head on a computer in the US, the
stage in `asia-northeast1-b`, development build `827a25c6d1`): the group
loaded at its second start (the first start's 900-second limit covered the
checkpoint copy), routed only requests that named `consumer-exact`, and
answered "Why is the sky blue?" in 94 tokens with the first token after
2.3 s (41-token prompt, 2.1 s of prefill). Decode on a cold page cache took
133–146 ms a token in the stage and 169–185 ms in the link to the faraway
head, about 3.3 tok/s end to end; the warm stage alone decodes at about
24 tok/s (above). Prefill is 83 tok/s at 2K and 65 tok/s at 8K, warm.

## Maya's prompt skip (#12470)

Maya skips layers 20–39 for every prompt token but the last ~2,700. The
claim holds, with a bound we can state exactly:

- Layers 21–39 have no compressor. They read earlier tokens only through
  layer 20's compressed KV and index keys, which layer 20 computes from its
  own input, and through their own window rows, which reach `window - 1`
  positions back.
- So layer `L`'s output for a token needs layer `L - 1`'s output for the
  `window - 1` tokens before it, and the last prompt token needs layer 20's
  output for `19 × (window - 1)` tokens before it. Decode steps need no more.
- The skip is exact when layers 0–19, and layer 20's attention input (its
  window rows, compressed KV, and index keys), run for every token, and the
  whole model runs for the last `19 × (window - 1) + 1` tokens: 2,414 at
  V4.1's window of 128. Maya's ~2,700 is above that.

`CpuTransformer::forward_with_deep_skip_check` writes arbitrary values into
the skipped tokens' state after layer 20, and the CPU reference test
`deep_layers_skip_is_exact_past_the_window_reach` checks it on the 40-layer
fixture (window 4, a 72-token prompt, three decode steps): with 58 tokens kept
the logits equal the full run's bit for bit for three different fills; with
20 or 1 kept they don't. Below the bound the change fades with distance (from
37 tokens kept it rounds away on this fixture), so only the bound is exact by
construction. The full model on the CPU reference can't run a prompt past the
bound here; the layer-major prefill may add the skip under that bound, and
its CUDA test compares it with the full prefill.

## `consumer-q` (lossy, unqualified)

`psionic-v41-stage quantize --weights DIR --out DIR --recipe q2q3|q3
[--importance FILE,...]` writes an artifact: per layer one file of 384
experts, each `[w1, w2, w3]` as code bytes, a log-scale byte per 32 values,
and an `f32` scale per row; `manifest.json` records the recipe, formats,
source checkpoint, importance digest, and each file's SHA-256.

- Values are Lloyd–Max levels of a unit normal: 4 levels at 2 bits, 8 at
  3 bits. `q2q3` keeps gate and up at 2 bits and down at 3, with gate and
  up widened to 3 bits in model layers 0 and 39 (about 2.6 bits a weight,
  178 GB); `q3` keeps all three at 3 (about 3.3 bits, 221 GB). Optional
  `layer_formats` overrides set the boundary layers' layouts; older
  manifests without overrides retain their original formats and strides.
- The quantizer picks each block's scale by importance-weighted squared
  error. Calibration (`chain --prefill layer-major --prompts-jsonl FILE
  --calibrate-out PREFIX`) now records full activation second moments
  inside each 32-column group, per expert, for GPTQ feedback. After each
  rounded column, the quantizer adjusts the remaining columns using the
  inverse Hessian's Cholesky factor, with 1% diagonal damping. It keeps the
  feedback result only when it lowers that group's measured activation
  error. Correlations between different groups are not modeled. Old
  `V41IMP01` mean-square files remain readable and use rounding without
  feedback; new `V41IMP02` files carry the cross-column moments.
- Calibration also works with streamed experts. A whole-model calibration
  needs another 13.6 GiB on the device; choose an explicit smaller cache,
  such as `--experts file --expert-cache slots:64 --engram file` on the
  96 GB card. The ordinary serving path allocates none of this storage.
- A stage serving it announces the profile `consumer-q-cuda-device-sm_NN`,
  so a group expecting the exact profile refuses it.
- The quality card (`scripts/bench/deepseek-v41/consumer_q_card.py`)
  teacher-forces the oracle's own tokens (`chain --prompt-ids --force-ids
  --capture`) and scores KL, total variation, top-1 agreement, and the
  oracle token's log-probability error against the oracle's decode logits;
  G2 suite 2 runs through `g2_check.py` beside the oracle's and the exact
  runtime's answers.

For a batch of teacher-forced cases, each `--prompts-jsonl` line can name
its own `prompt_ids`, `mode`, and `force_ids`. `--capture DIR` writes
separate `round-N` folders for multiple rounds. Each folder's
`request.json` records the prompt, forced history, checkpoint index, and
consumer artifact manifest hash; the card refuses a prompt, mode, or
history that differs from the oracle. Nonemitting prefill steps are
recorded separately from the answer's logits. The independent evaluation
inputs are frozen in
[`consumer_q_evaluation.json`](../../scripts/bench/deepseek-v41/consumer_q_evaluation.json).

For a final card, record the SHA-256 hashes of the committed evaluation and
checker before capture. Run the checker with `--final`,
`--evaluation-sha256 SHA`, `--checker-sha256 SHA`, and
`--artifact-manifest PATH`, followed by the oracle directory, output file,
and all eight `CASE=CAPTURE_DIR,LABEL` pairs for each of the `exact` and
`consumer-q` labels. It refuses changed inputs, incomplete cases, or a
capture from another artifact. The card records hashes of each capture's
files and the oracle tensors it scores, and verifies all eight oracle run
records against their frozen hashes. Final output files must be new.
Without `--final`, it writes a partial diagnostic card.

The first emitted logits predict the first generated token and match
`decode.last.logits`; later outputs match `decode.g1.logits`, and so on.
Earlier versions of `consumer_q_card.py` started at `decode.g1.logits`,
so their KL, total variation, top-1 agreement, and log-probability figures
are invalid. Keep any such cards as historical diagnostics and rescore
complete saved captures into new files. Direct sampled-token comparisons
against `run.generated_ids` are unaffected.

Calibration ran on one `g4-standard-192` (64 prompts, layer-major prefill,
39 minutes), and the quantizer then wrote both recipes in 25 s a layer each
(`q2q3` 176 GB, `q3` 221 GB), stored under
`gs://openagents-psionic-models-ane1/.../consumer-q/{q2q3,q3}-00d720cdf6`.
The artifact matches the checkpoint's experts to within the expected error
(relative squared error 14% for the 2-bit gate and up, 3% for the 3-bit
down). Served single box, `q2q3` decodes at 26.4 tok/s (p50 37.7 ms, 48% of
experts in slots, 97% hits), but its answers are garbage from the first
token. Keep this artifact as the failed baseline for #12465.

An eight-position teacher-forced diagnostic on 2026-10-22 matched all eight
oracle top tokens with native weights and seven with the old `q3` artifact,
including the first six. This narrows the original failure without establishing
whole-answer quality. The record is
[`consumer_q_diagnostic.json`](../../scripts/bench/deepseek-v41/consumer_q_diagnostic.json);
its cold-cache timings are not serving-speed measurements.

The 2026-10-22 GPTQ artifact contains all 40 expert layers, with wider
3-bit boundary layers, at 178,394,234,880 bytes (2.62546 bits per expert
weight). Its manifest is
`gs://openagents-psionic-models-ane1/deepseek-ai/DeepSeek-V4.1-Flash-1017/consumer-q/q2q3-gptq-2a20931f9c/manifest.json`,
SHA-256 `97a7bc631ddfeaff1efe0db4fa1b5d0d0a77d74ed0844f238c258335d012360d`.
Every layer's hash, size, format, source and upload generation was checked
before publishing the manifest last.

Calibration used 16 separate input streams, totaling 15,929 prompt tokens.
Both input and hidden covariance tables cover 15,033 of 15,360
expert/layer pairs (97.87%). Unobserved pairs use equal column importance
and rounding without feedback. The saved inputs and overlap check show
no matching frozen heldout or G2 prompt/answer token sequences.
The raw IMP02 file SHA-256 is
`095f6927929294439dc51afe4fb2d24a897146085d692cdd66f5fc8e2f0a56a7`.
The manifest's `importance_sha256`,
`6eb4cfa7800157795dbdfb8398215a4ca939454aae40423ce852f8f91cd35d0c`,
is SHA-256 of that lowercase ASCII hexadecimal file digest without a
newline. Both identities and the full coverage report are saved.

The frozen 336-position card compares 168 matching teacher-forced
positions per profile against the same oracle:

| Profile | Top-1 agreement | Mean KL | G2 suite 2 |
| --- | --- | --- | --- |
| Exact weights | 166/168 (98.81%) | 0.01102 | 5/6; all mandatory checks pass |
| GPTQ consumer-q | 153/168 (91.07%) | 0.11656 | 5/6; all mandatory checks pass |

Both profiles completed all six ordinary G2 answers with EOS within the
unchanged 4,096-token caps. The pinned oracle also scores 5/6 under the
frozen checker (`362c5bf6…`); all three share its nonmandatory
Fibonacci-186 failure. `consumer-q` uses fast reductions, and each profile's actual
command and arithmetic settings are saved. These descriptive results do
not qualify consumer-q. The card, raw answers, frozen scoring rules,
source/build identities, calibration records and artifact receipts are
indexed in
[`consumer_q_result.json`](../../scripts/bench/deepseek-v41/consumer_q_result.json).

The index also links the retained serving binary, BUILD record and exact
CUDA libraries.

On one 96 GB RTX PRO 6000 Blackwell (`g4-standard-48`, about 177 GiB
host RAM), three real repetitions of one 15-token chat prompt ended with
identical 39-token outputs. The second and third answers measured **24.46 tok/s** together
over 76 decode tokens (22.92 and 26.23 tok/s individually). The cache held
178 slots per layer (46.35% of experts), occupying 77.01 GiB; peak device
use was 92.06 GiB. This separate run used no captures, routing seed,
concurrent quantization, scoring or transfers. This is a short,
single-prompt sample. **The 35 tok/s target was not met.**

## DSpark drafting (#12469)

Add `--dspark` to a whole-model CUDA worker's existing single-box flags,
or to `chain --split 0-40 --gpus 0`. It needs at least six step tokens.
The worker announces `consumer-exact-dspark-<cuda-profile>` (or
`consumer-q-dspark-<cuda-profile>` with the lossy experts), so drafting is
an explicit serving choice. A front still sends and receives one token at
a time: the worker keeps the verified answers ready for later steps. A
changed input or sampling setting discards the unused answers and restores
the state for the inputs the front actually sent.

The drafter comes from the pinned MIT `DeepSeek-V4.1-Flash-1017`
checkpoint's `mtp.*` tensors: three window-only CUDA blocks, reading the
backbone's stream means before layers 37–39, with its Markov and confidence
heads. The backbone embedding and output head stay shared. This is a Rust
and CUDA reimplementation of that checkpoint's `inference/model.py`;
the incompatible V4 preview drafter and DFlash2 are not used.

Verification returns the target logits after each of six inputs: the
pending token and five proposals. Greedy decoding accepts only the target
argmax. Sampling accepts with `min(1, p/q)` and, on rejection, draws from
normalized `(p-q)+`. A rejected suffix restores device window rows,
compressor groups, compressed KV and index keys, selections, candidate
flags, the backbone features, and both device and host Engram history,
then replays only the accepted prefix. This first version favors complete
state restoration over avoiding that replay. Invalid verification output
also restores the saved state before returning an error. `chain --dspark`
records proposed and accepted counts, accepted prefix lengths, acceptance
rate, replayed tokens, and measured decode throughput in its answer records.

On 2026-10-22, the [3f32d9abb6 worker](verification/deepseek-v41-dspark-2026-10-22/BUILD.json)
passed [ten CUDA controls](verification/deepseek-v41-dspark-2026-10-22/sm120-controls.log)
on an RTX PRO 6000 Blackwell Server Edition. These include 48 rollback
configurations and serving with changed prefixes and sampling settings.

The same immutable worker then generated 32 greedy tokens for each of the
eight frozen oracle prompts, including the 19,989-token `p4-20k` input.
All 256 output IDs equal ordinary decode. Both paths used native checkpoint
weights, packed KV, 32,768-token context, 4,096-token steps and prefill chunks,
and `--expert-cache 40`: 53 slots per layer across 40 layers,
39,857,418,240 device bytes, and 150,405,120 staging bytes. The device-memory
peak sampled every two seconds was 67,989 MiB.

| Required prompt | Input tokens | Ordinary tokens/s | DSpark tokens/s | Accepted/proposed |
| --- | ---: | ---: | ---: | ---: |
| `e-engram` | 68 | 4.478 | 0.990 | 20/35 |
| `e-window-127` | 127 | 6.335 | 0.758 | 15/55 |
| `e-window-128` | 128 | 6.828 | 0.992 | 20/55 |
| `e-window-129` | 129 | 11.051 | 2.500 | 19/55 |
| `p1-chat-thinking` | 41 | 2.468 | 1.010 | 20/35 |
| `p2-code` | 40 | 4.602 | 2.163 | 24/25 |
| `p3-2k` | 2,124 | 2.672 | 1.224 | 22/40 |
| `p4-20k` | 19,989 | 2.449 | 1.211 | 20/40 |

For these eight cases, ordinary decode took 62.721 seconds and DSpark took
211.811 seconds: **3.954 versus 1.171 tokens/s**. Each rate counts 248
decode tokens, excluding each prompt's first prefill prediction. DSpark
accepted 160 of 340 proposals (47.06%); accepted-prefix counts for lengths
0–5 were `[13, 13, 12, 10, 7, 13]`. Drafting was slower in this trial.
DSpark ran first, then ordinary decode, with other GPU work, quantization,
checkpoint scans, and uploads stopped during the measurement.

A separate DSpark-only repeat of `e-engram` produced the same 32 IDs and
accepted lengths. Its rate was 2.093 tokens/s, compared with 0.990 on the
first DSpark pass. The ordinary command ignores `--repeat-answer` with a
prompt file, so there is no ordinary warm-repeat comparison; the repeat
is excluded from every paired aggregate above. Equality here compares
DSpark with ordinary CUDA decode. Both paths differ from the golden IDs on
`e-window-127` (4 of 8 positions match), `p1-chat-thinking` (26 of 32), and
`p3-2k` (1 of 32); the other five recorded prefixes match. These results do
not establish reference-checkpoint qualification.

The [native summary](verification/deepseek-v41-dspark-2026-10-22/native-summary.json)
retains token arrays, per-case accepted lengths, timings, configuration,
and source, worker, checkpoint-index, and library hashes. The
[raw receipt](verification/deepseek-v41-dspark-2026-10-22/native-pair.tar.gz)
contains both complete reports, answer files, logs, the unchanged runner,
and the exact prompt IDs. Its SHA-256 is
`7319cc62dbee31b8b60c09eba904982dca2051b51243f497c887b0052254b347`;
the local copy and a cloud-storage download matched before the machine
was released. The [summary script](verification/deepseek-v41-dspark-2026-10-22/summarize-paired.py)
checks all eight inputs, modes, token arrays, and build identities.

## Cost

The `g4-standard-48` ran from 04:01Z; see #12463 and #12464 for each run's
cost. The spend guard's `deepseek-v41` budget carries every machine.
