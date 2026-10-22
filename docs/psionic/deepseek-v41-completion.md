# Completing #12369: native DeepSeek-V4.1-Flash on stage Pylons

Status: the definitive checklist for
[#12369](https://github.com/OpenAgentsInc/openagents/issues/12369), written
2026-10-21. It replaces the scattered "what's left" sections of the
[native V4.1 plan](deepseek-v41-native.md), its review addendum, and the
[issue roadmap](../roadmap/2026-10-20-enter-the-verse-issues.md) as the one
place that says what finishes the epic. Those docs still hold the reasoning
and the evidence. Update this file as items land.

**Current status, 2026-10-22:** the owner reopened qualification after the
[closed-issue audit](../audits/2026-10-22-deepseek-closed-issues.md).
[#12369](https://github.com/OpenAgentsInc/openagents/issues/12369) is open
again, owned by Codex for the remaining G1–G8 acceptance work. Its earlier
closure authorized an experimental preview; it did not pass the original
gates. Policy 2.3 failed, including one 38-token Engram case, and remains
failed. The pinned `06e5158778` worker passes the frozen G2 suite at 5/6
against the oracle's 5/6 on all three placements, with identical answers and
the reasoning/completion floor held. G3 has current SM89 and SM120 component
receipts. The private one-quad, two-dual, and four-single Pylons each pass the same
eight runtime checks. Stage crash, freeze, deadline, and full-VM watcher
recovery checks are retained. G1 and G4 numerical release acceptance and
qualified G8 evidence remain outstanding: rule 2.4 is not frozen, and no
fresh confirmation oracle has been captured. The separate Maya Pylons run
Foreman's browser-editor work with an experimental consumer-q profile.
G5 and G6 still depend on numerical acceptance and paired direct-pipeline
answer checks; their runtime checks alone do not pass those gates.

The entries below retain earlier checks; later sections record their reruns
and the current worker's evidence.

**Qualification repair progress, 2026-10-22:** the qualification CLI now
returns a failure status when its report fails, and `--release` prints the
release verdict. The report keeps the older component verdict separately as
`diagnostic_passed`; score-budget violations affect its primary verdict.
G2's floor now requires completion and the reasoning split on every task,
independently of the task pass count. Its parent checker compares returned
values saved after all calls finish; printed `G2-RESULT` lines do not count.
All 18 G2 controls and all 40 stage-worker tests pass (one timing test is
ignored). Rescoring the unchanged retained answers still gives 5/6 versus
5/6, with the same disclosed `fib(186)` failure and the floor held; the
[new report](verification/deepseek-v41-gates-2026-10-22/g2-rescored.json)
records the checker hash. This repairs the checker and verdict; it does not
pass G1 or qualify a different runtime bundle.

**Numerical repair progress, 2026-10-22:** the optional
`fp8-mma-tp8` CUDA diagnostic reproduces the retained B200 FP8 row, agrees
exactly with the independent CPU sums on 37 mixed rows at both real
projection widths, and passes the formerly failing 38-token Engram case
under the unchanged policy 2.3. Layer 15's worst score error falls from
0.862554 to 0.668646, below its 0.836315 budget; the case takes 8.08 s
versus 6.85 s. The [complete diagnostic report](verification/deepseek-v41-gates-2026-10-22/gpu-fp8-c4-diagnostic.json)
is a regression result, not release qualification. The separate profile
requires recording and cannot pass `--release`; default serving arithmetic
is unchanged. The [corrected SM120 package](verification/deepseek-v41-gates-2026-10-22/sm120-fp8-components.json)
passes 114 tests, with six ignored.

The expanded B200 capture retains every tracked index row and full inputs
at cuts 10, 20, and 30. Its [comparison with the previous oracle](verification/deepseek-v41-gates-2026-10-22/oracle-capture-comparison.json)
finds all 31,325 shared tensor hashes unchanged, none missing, and 8,742
additional tensors. It uses the exposed confirmation-2.3 corpus for
calibration only. Full-model index evaluation, isolated-stage checks, and
fresh confirmation remain outstanding; no policy has been loosened or
promoted. The CPU and CUDA recorders now expose raw causal scores,
selections, and packed candidate flags; CPU controls preserve outputs bit
for bit. The next policy must evaluate that evidence rather than repeat
the historical claim that no index evidence exists. Within this launch's
8,192-token context, at most 1,024 candidate blocks are visible, below the
model's 2,048-block allowance; the candidate-mask check must still hold,
while competition among candidate blocks belongs to the longer profile.

**Recovery repair, 2026-10-22:** `scripts/pylon/v41-live.py` discovers every
allocation carrying its run ID directly from the guard ledger, including a
lost create reply before `cloud.json` exists. After a launch ends, it stops
remaining allocations through the guard with `stop --record ID`; another
run's allocation is excluded. It refuses another launch until cleanup
settles, recalculates time and money before every zone attempt, rounds the
booking down to whole provider minutes, and terminates a blocked launch at
the absolute end time. Unreadable ledger records stop admission. Nine
isolated watcher controls and all 42 guard tests pass. The
[audit controls rerun](verification/deepseek-v41-gates-2026-10-22/audit-controls-fixed.json)
now reject both forged outputs and the missing reasoning split, and keep
the retry booking within $300. This source repair has not yet replaced the
running preview watcher; rotation is part of G8's pinned launch check.

**Direct router evidence, 2026-10-22:** an observed CUDA stage now saves all
384 biased scores directly from the kernel that selects experts. Recording
adds no host inference or score reconstruction. The harness requires those
stage scores for release and rejects a score table that selects different
experts, including the older F64 proxy's disagreements. Nonfinite recorded
scores are refused. The recording kernel has controls for ties, bias,
two-token row order, and unchanged IDs and route weights. The dedicated SM120 component run passes, and full-model direct-score regressions are retained; the first run exposed and fixed the synthetic reader's width assumption. Existing 2.1–2.3 reports are unchanged.

The first dedicated SM120 package run on bundle `986a5a89…` passed all 64
CUDA backend checks, including the direct-score recording control. The
[complete package log](verification/deepseek-v41-gates-2026-10-22/sm120-4fc0cfdc44-first.log)
retains one failed synthetic qualification test: the harness assumed 384
expert scores for the 16-expert model. The reader now derives the width
from the recorded rows, checks oracle IDs against it, and refuses malformed
or nonfinite rows. Seven local qualification controls pass; the corrected
GPU package rerun remains required. The original-weight model still has
384 experts. Calibration is running on dedicated CPU and G4 machines;
these regressions do not supply fresh confirmation evidence.

## What "done" means

The epic's goal: **serve DeepSeek-V4.1-Flash with Psionic as the only
inference engine, fanned out across several Pylons (one GPU per stage), so it
shows up in the Grid as a group of Pylons.** No llama.cpp, DeepInfer, or
PyTorch in the serving path. DeepSeek's reference code is a test oracle only.

The epic closes when all eight gates below pass, each with retained
evidence in the repository or the oracle bucket. A gate is about evidence,
not issue state: closing a child issue doesn't pass a gate whose check is
listed as an after-close check.

**Qualification revision, 2026-10-21:** the table now describes the planned
v2 release contract: qualify the served GPU runtime directly against the
oracle. The [review addendum](#qualification-review-addendum-2026-10-21)
defines the required evidence and the transition from v1. G1 v1 remains
failed; G1 v2 failed under frozen policies 2.1 and 2.3. A successor requires
independent calibration, complete evidence, and fresh confirmation data.
No existing result becomes a pass through this revision.

**Launch scope revision, 2026-10-21 (owner decision):** the launch uses the
short served profile: prompts of up to **2,872 tokens** (the checked
four-stage CUDA budget, #12408), an 8,192-position context, a 4,096-token
output cap, a 75 s step limit (150 s for the prompt's step, a correction
recorded under the gate progress below), and a 600 s deadline (W14). For G1 and G2,
"complete declared coverage" means coverage of that profile: the oracle's
chat, code and 2k prompts (p1–p3, all within 2,872 tokens), the edge cases,
and the holdout, each within the profile. The 19,989-token p4 prompt and
chunked prefill are out of the launch scope and move to
[#12414](https://github.com/OpenAgentsInc/openagents/issues/12414). The
served profile and the beacon's `request_limits` must advertise the
2,872-token bound, and a 2,873-token request must be refused before any
stage runs it. Revised before any G1 v2 result was observed; no failed
result becomes a pass through it.

| Gate | Passes when | Evidence |
| --- | --- | --- |
| **G1. Served-runtime numerical qualification** | The original-weight GPU runtime passes a versioned, prospectively frozen policy against same-mode oracle tokens: full-vocabulary distribution error, margin-aware token and routing checks, exact structural checks, and complete declared coverage. The four main prompts, edge cases, and a separate holdout are required. | Policy and corpus hashes, full comparison report, and runtime identity; v1 CPU failures retained separately |
| **G2. Complete answers and task quality** | The served runtime finishes the original chat and code prompts and a frozen task suite to EOS within its output cap, passes task assertions and the declared quality budget, and splits reasoning and final text correctly with the #12372 adapter. CPU generation is a separately reported diagnostic. | Complete token/text traces, stop reasons, task results, and paired oracle results |
| **G3. Kernel parity on GPU** | Each CUDA kernel matches the CPU reference on real tensors, on the RTX 4080 (SM89) and on the target GPU. | Per-kernel test results for both GPUs |
| **G4. One stage on one GPU** | A stage process holding a contiguous layer range runs device-resident (no per-token host stepping), passes its stated arithmetic bounds and G1's structural and routing checks on identical oracle stage inputs, and reports connected error separately. | Stage comparison results, memory peak, per-token latency |
| **G5. A full group on one machine** | Stage Pylons, one per GPU, run the whole model over loopback and pass G1's decode checks, G2's complete answers, and DS0's six runtime checks, with speed and per-GPU memory recorded. | Group report from a cloud run through the spend guard |
| **G6. A group across machines** | The same group passes the same checks across separate machines in two genuinely different placements, for example four single-GPU machines and two two-GPU machines. Per-hop latency and stage-state bytes per token are recorded. | Group reports for both placements |
| **G7. Lifecycle and recovery** | A group is advertised only when every stage is loaded and checked. Killing or freezing any stage fails the request visibly, never replays it after tokens reached the caller, and the group recovers under a new generation. A restarted stage can't serve a stale request. Caller cancellation keeps the group busy until work stops. | Fault-injection results on a real group |
| **G8. Visible as a Pylon group** | One launch command starts a cost-capped, expiring group and publishes it. The router sends a V4.1 request to it through Pylon, and the answer comes back complete within the group's declared deadline. The `/home` Pylon layer shows the group's Pylons. | A Pylon transcript, the router log, and a capture of `/home` |

Gate progress (2026-10-21):

- **G1 v1, original-weight CPU parity (W1, #12388; #12404): failed.
  G1 v2, served GPU qualification: failed under policies 2.1 and 2.3;
  2.2 couldn't be frozen. See the native doc's 2026-10-22 sections.** The v1 runner checks the
  `ReferenceBf16Tp8` CPU profile against the eight-B200 TP8 oracle. #12404 got a
  new B200 capture, showed that same-mode oracle reruns are bit-identical,
  measured the tensor-core, `MUFU.EX2`, and `expf` arithmetic on the device,
  and made layer-0 attention equal the oracle bit for bit. The first limit
  failure is now the layer-1 attention pre-mix, caused by layer-0
  hyper-connection mix values that differ by about one F32 ulp.
  [#12405](https://github.com/OpenAgentsInc/openagents/issues/12405) owns the
  remaining investigation. Exact B200 arithmetic emulation is no longer a
  prerequisite for GPU qualification or provisional W3 measurements; the
  review addendum specifies the new work and issue changes.
- **G5–G7, stage contract (W9, #12396).** One message type carries the
  manifest digest, group generation, request, attempt, stage and layers,
  step, position, tokens, and typed sections. A stage refuses duplicate,
  stale, missing, reordered, cross-request, malformed, and oversize
  messages before it consumes any state. With a synthetic executor that
  keeps V4.1's 40-layer sharing layout, four-stage, eight-stage, uneven, and
  one-layer-per-stage splits match the unsplit run bit for bit. W7
  (#12394) and W11 (#12398) each found that the hyper-connection pre-mix
  must cross beside the residual (`SectionKind::PreMix`, four F32 weights
  per token): the next layer collapses the streams with it and can't
  recompute it. With it, the CPU port's layer ranges
  (`deepseek_v41_stage_cpu`) match the unsplit transformer bit for bit at
  every split, one layer per stage included.
- **G6, stage-state bytes per decode token,** from encoded messages of the
  real shapes (BF16 residual and cache rows, a 36-byte request ID): 44,348
  bytes at a boundary of 10 (43,836 on average, since layer 8's KV adds a
  row every other token), 41,172 at 20 (the residual and its pre-mix only),
  and 52,820 at 30, each including the pre-mix's 48 bytes. The four-stage
  forward path is 138,340 bytes per token; eight stages are 332,636.
  Returning full F32 logits to the front adds 517,280 bytes per token,
  more than every forward hop together, so W10 (#12397) decided that the
  last stage samples with the front's settings and returns the
  token ID, 4 bytes. The synthetic geometry's numbers are in the W9 test
  output.
- **G3, block-scaled matrix-vector kernels (W4, #12391), RTX 4080; SM120 below.**
  FP8 and FP4 weight kernels and the FP8 activation quantizer match the CPU
  path on real rows of `layers.2.attn.wq_a`, the layer-2 shared expert's w1
  and w2, and routed expert 222's w1 and w2. The tolerance, fixed before the
  first GPU run, is the summation bound for the two addition orders:
  `(gamma(K - 1) + gamma(K / 32 + 30)) * sum |w_i x_i|`. The largest real-row
  error is 0.0074 of that bound, and every BF16 output equals the CPU's (160
  of 160); the expert rows also equal the reference's own `fp4_gemm` outputs
  (64 of 64). Activation codes and scales equal the reference's `act_quant`
  output bit for bit, and the device matches a host model of its arithmetic
  bit for bit in every case, including NaN and extreme scales. The RTX PRO
  6000 (SM120) half passed in W8. Results:
  [`sm89-2026-10-20.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_matvec/sm89-2026-10-20.json).
- **G3, mixing, Engram, and routing kernels (W6, #12393), RTX 4080; SM120 below.**
  Tolerances were fixed before the first GPU run. Collapse, expand, the
  Engram gather, and the expert sum repeat the CPU's operations in its order
  and equal it exactly. Reductions in another order use the same summation
  bound as W4; `exp` and `log1p` (rounded once from double precision on the
  device) may differ from the CPU's float functions by 2 ulp, and later
  steps carry that difference through their derivatives or, for the
  Sinkhorn iterations, through a weighted norm that grows by at most
  `sqrt(1 / s)` per normalization. Every kernel also equals a host model
  bit for bit. Results: real mixes (layer 0 attention, layer 39 FFN) reach
  0.0008 of their bound; collapse and expand at all seven recorded layers
  equal the CPU, and their BF16 outputs equal the oracle's; the 24 Engram
  rows at layers 1 and 14 equal the oracle's rows; router ids equal the CPU
  and the oracle at layers 0, 2, 14, 20, and 39 (no recorded exact ties at
  this step); the SwiGLU output of experts 222 and 180 equals the oracle's
  BF16 down-projection input in 2,304 of 2,304 values. Results:
  [`sm89-2026-10-20.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_mix/sm89-2026-10-20.json).
- **G8, launch command (W15, #12402), local evidence only.** `pylon cloud
  up` starts a cost-capped, expiring group through the spend guard and
  publishes it; on fake stages and a fixture relay, the router's
  `Settings::want` request for the V4.1 artifact reaches the head and the
  last stage's tokens come back. Spot loss, expiry, the cap, and `down`
  each drop the group's beacons and delete its machines through the guard.
  Each dry run plans two g4-standard-96 or four g4-standard-48 spot machines
  at $26.62 for four hours. Placements are Psionic manifests shared as
  JSON (`crates/pylon/placements/deepseek-v41/`). G8 still needs a real
  cold launch (W16, #12403), which needs W11's stage worker in a bucket
  (speaking W12's direct transport across machines), W12's cross-machine
  run, and W14's qualified deadline (its transport, `pylon.deadline.v1`,
  landed in #12401).
- **G6 and G7, stage-to-stage transport (W12 part 1, #12399), loopback
  only.** With the direct transport a decode hop at layer 10 carries
  44,389 bytes (44,300 of stage state, 74 of framing and MAC, and the
  request ID), about 44,460 with the head's sampling settings. A release
  build moves three such hops plus the token back in about 0.6 ms on
  loopback (about 150 µs per hop). Every hop is authenticated with a key
  the head makes for each generation, so a stage from an older
  generation is refused at the handshake. On fake stages, a dropped link,
  a partitioned middle stage, and a stalled stage behind a full queue
  each fail the request visibly with the tokens already delivered and
  never replay it, and the group relinks under a new generation. With
  W15's head on the budget host each token crossed the wide-area network
  twice per stage; `--head stage0` leaves the first stage's step and the
  last stage's token as the only hops through the head, both inside the
  zone. G6 and G7 still need the real cross-machine run (W12 part 2).
- **G6, a group across machines (W12 part 2, #12399): runs in both
  placements on the unqualified runtime; numerical acceptance waits for
  G1 v2.** `pylon cloud up four-single|two-dual --head stage0 --transport
  direct` ran the four CUDA stages on four spot `g4-standard-48` machines
  and on two spot `g4-standard-96` machines in us-west3-a, at $6.65/h each.
  Both placements answered all nine W14 qualification prompts through Pylon
  with the same greedy text as the one-machine group. Each stage now reports
  its own time per step, so the head splits a decode token: 33.9 ms in both
  placements, of which the stages compute 7.9–8.8 ms each and the links plus
  the head's round trip take 0.87 ms (four-single, three machine crossings)
  or 0.83 ms (two-dual, one). A crossing costs about 20 µs a token; a 44 KB
  TCP round trip between the machines takes 55–60 µs against 9 µs on
  loopback. Short answers decode at 29.1–29.7 tokens/s in both placements,
  the same as one-quad's 29.7. The three hops carry 261,032 bytes of stage
  state a token, nearly twice W9's BF16 estimate, because the CUDA stages
  send residuals and pre-mixes as F32. Receipt:
  [`deepseek-v41-g6-g7-cross-machine-2026-10-21.json`](verification/deepseek-v41-g6-g7-cross-machine-2026-10-21.json).
- **G7, lifecycle on the cross-machine groups (W12 part 2, #12399):
  passed on the unqualified runtime.** A killed stage failed the request
  visibly with the tokens already delivered (327 and 322) and no replay; a
  frozen stage failed it after its 10 s health limit (260, 265, and 275
  tokens); late messages from the old generation were refused; and each
  group came back under a new generation and answered again with the same
  text. A replaced stage reloads in about 15 s when its weights stay in the
  page cache, and in 6–8.5 minutes on a machine that also holds a 101 GB
  Engram table. An 8 s buyer deadline cancelled a long answer, and the slot
  freed only after every stage stopped. Deleting a stage machine, as a spot
  reclaim does, used up the restart budget within a minute; the group went
  offline and `up` deleted the other machine. `pylon cloud down` drained the
  head, published offline beacons, and deleted every machine. The run found
  one defect: a killed CUDA stage on the head's own machine was not reaped
  within the 3 s stop limit, so the head stopped the whole group; cloud
  groups now wait 30 s, and the rerun recovered. The runs cost $7.84.
- **G8, the deadline and answer profile (W14 part 2, #12401): frozen and
  met on the unqualified runtime.** The head declares a 600 s deadline and
  a 4,096-token output limit, needs `--step-secs 75` (it times the whole
  prompt step), and supports prompts of up to 2,872 tokens, the checked
  budget of #12408 for `four-single` and `two-dual`, which the stages
  announce and the head's beacon carries as `request_limits`; the 20k
  case is not claimed. The receipt recorded a 3,022-token prompt; the
  recount (W2, #12389) found that the prompt sent was the first 6,110
  characters of the retained file, 1,526 rendered tokens, and the receipt
  is corrected. A 3,022-token prompt encodes to 282,436,409 bytes into the
  layer-30 stage on the earlier binary, over the codec's 268,435,456, so
  #12408's 2,872-token budget stands. All nine qualification prompts passed in both placements:
  extended, `stop`, a final answer split off at the model's own token, and
  complete in 3.1–9.2 s against the 600 s deadline. A 1,072-token prompt
  took 36 s to the first token and 64 s in all; a 1,526-token prompt took
  52 s and 131 s. An answer whose reasoning reached 4,096 tokens took
  156 s and was refused `limit_exceeded`. The last stage slows from 8.8 ms
  to 10.9 ms a token over 4,096 tokens, consistent with `AnswerParser::push`
  reparsing the prefix. The head's beacon carries its measured profile as
  `answer_profile` in its inert `meta`. These are thinking-mode answers, so
  none pairs with the oracle's chat-mode p2. Receipt:
  [`deepseek-v41-w14-answer-profile-2026-10-21.json`](verification/deepseek-v41-w14-answer-profile-2026-10-21.json).
- **Correction to W14's frozen profile (W16, #12403, 2026-10-21): the
  prompt step gets its own 150 s limit.** On the pinned one-quad group
  (worker bundle `185eb68c…`), an exact 2,872-token thinking-mode prompt
  failed after 79.8 s: "step 0 made no progress before its deadline",
  with no token delivered, and the head replaced the last stage. An exact
  2,873-token prompt was refused before any stage ran, as specified. The
  prompt is one step that the four stages prefill in turn, at about 33 ms
  a prompt token in all, so 2,872 tokens need about 95 s, while the step
  limit is 75 s. `pylon cloud up --prefill-secs N` (1 to 300, at least the
  step limit) now bounds the steps before the first answer token
  separately; decode steps keep 75 s. The launch uses 150 s. The worst
  case, about 95 s of prompt plus 4,096 tokens at about 27 tokens/s, is
  about 250 s, inside the 600 s deadline. The prompt bound (2,872 tokens),
  the output bound (4,096 tokens), and the deadline (600 s) are unchanged.
  This correction precedes the qualified W14 rerun.
- **G3, attention kernels (W5, #12392), RTX 4080; SM120 below.** Checked against
  the CPU path's default `ReferenceBf16` order (window rows oldest first);
  the `ReferenceBf16Tp8` order is W7's choice. Tolerances were fixed before
  the first GPU run. Kernels without a transcendental repeat the CPU's
  sequential sums and equal it: the cache quantizers, window store, dense
  products, RMS norms, index scores, top-512 selection, and candidate
  blocks. RoPE differs only through `cos` and `sin` (`(|a| + |b|) 4u +
  2u |out|`, u = 2^-24), the ratio-2 pool and the attention only through
  `exp`, and every kernel equals a host model bit for bit. Results: RoPE at 20,952
  head cases reaches 0.47 of its bound; 316 window tokens (prefill and a
  wrapping 128-row ring) and 26 compressed cases (ratios 1 and 2, 0 to 700
  rows, masked slots) equal the CPU's BF16 output; real compressor latents
  at layers 2 and 20 (28 groups from the oracle's inputs and real weights)
  equal the CPU and sit within 0.44 of the W1 policy of the oracle's;
  selections from the oracle's own scores equal its `topk_idxs` at all
  eight index sources and its layer-20 candidate blocks at 28 steps, up to
  20,021 positions, with no tied index differing; a 600-token prefill
  through sources 2 and 20 and consumers 3, 21, and 24 reads the sources on
  the device and equals the CPU components stage by stage. Results:
  [`sm89-2026-10-20.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_attention/sm89-2026-10-20.json).
- **G4, one stage on one GPU (W7, #12394): v1 comparison failed;
  v2 qualification pending.** The stage runs device-resident: per decode step one graph
  launch, one wait, and about six small copies, with no kernel launched
  outside the graph. Layers 0–4 on the RTX PRO 6000 decode in 4.13 ms per
  token (4.21 ms after a 2,124-token prompt) and prefill 242 tokens/s, in
  39.5 GB; layers 0–9 take 8.06 ms and 122 tokens/s, in 76.6 GB. Against
  the CPU stage in the same default `ReferenceBf16` order, layer 0, the
  Engram ids and rows, and layer 2's selections pass the frozen policy, but
  from layer 1 a float32 mix exceeds it (3.8e-4 against 3.5e-4) and router
  sets then differ. Against the oracle, the GPU fails 180 of 625 checks and
  the CPU stage 191, with the same first failure (layer 1's router set at
  position 0). Direct qualification of this stage remains required; an
  isolated per-layer check must separate amplification from a defect.
  Sequential dense products and norms take 2.1 ms of a 4.06 ms step; a
  bounded parallel sum is the first speedup. Results:
  [`sm120-2026-10-21.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_stage/sm120-2026-10-21.json).
- **G5, a full group on one machine (W11, #12398): runs end to end;
  numerical and runtime acceptance pending.** `pylon cloud up one-quad --head stage0
  --transport direct` ran the four CUDA stages on one spot
  `g4-standard-192` (4 RTX PRO 6000, 720 GiB) in us-west3-a, with the head
  on the same machine and jobs sent over the relay with `pylon ask`. The
  machine copied 467 GB of shards in about 3 minutes, and the stages
  loaded in under 2.5 minutes with 72.6–74.2 GB on each GPU and both
  Engram tables in host memory (209 GiB used). Greedy answers: the
  oracle's p1 prompt (41 tokens) answered in 121 tokens with the first
  token after 1.3 s, then 29.7 tokens/s, the same text twice; a 66-token
  code prompt in 851 tokens at 28.3 tokens/s; and a 1,072-token prompt
  with the first token after 35.6 s, since prefill runs stage after stage
  at about 120 tokens/s each. Against the oracle, the group renders p1's
  prompt ids exactly, and its final answer diverges at the 10th content
  token ("scattered by molecules" against "scattered by gas molecules"),
  the divergence W7 recorded for the default arithmetic order. G1's decode
  checks, DS0's runtime checks, and G7's faults were not run. Runtime and
  fault checks can proceed before numerical qualification, with the runtime
  labeled unqualified. The run cost $1.47 ($2.27 with the failed attempts).
  Receipt:
  [`deepseek-v41-g5-one-quad-2026-10-21.json`](verification/deepseek-v41-g5-one-quad-2026-10-21.json).
- **G3 on SM120 (W8, #12395): passed.** On W7's `g4-standard-48` (one
  RTX PRO 6000, CUDA 12.9, driver 580.178.04, `PSI_CUDA_ARCH=sm_120`), W4's
  9, W6's 15, W5's 14, and W7's 7 tests pass with the tolerances fixed
  before the SM89 runs. That includes every bit-for-bit host-model check,
  so the double-precision `exp`, `cos`, `sin`, and `log1p` round as on the
  host, and float32 subnormals aren't flushed. Decode matrix-vector
  products read 1,086–1,139 GB/s on the large FP8 matrices (`wq_b`,
  `wo_b`) and 1,001 GB/s for six FP4 experts in one launch, about 60% of
  the card's ~1.8 TB/s; small matrices read 515–824 GB/s. Both GPUs build
  from one source: `quantized_matvec.cu` takes CUB from CUDA 13's
  `include/cccl` or CUDA 12's `include/cub`, so the stage machines no
  longer need an include shim. Numbers and logs are on #12395.
- **Placement from header bytes, then measured bytes (W3, #12390;
  provisional).** Layers 0–9, 10–19, 20–29, and 30–39 need 75.41, 74.07,
  73.92, and 75.23 GB of GPU weights by the shard headers, and the Engram
  tables (101.38 GB each) go to host memory on the stages that run layers 1
  and 14. Four stages are tried before eight, and a plan that fits neither
  is refused. `MeasuredMemory` now implements the same `PlacementMemory`
  trait from the CUDA runtime's measured bytes
  ([`placement_memory_measured.json`](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41_weights/placement_memory_measured.json)):
  at the served 8,192-token context the four stages peak at 78.10, 76.75,
  76.54, and 77.85 GB of a card's 101.97 GB, and the model covers all 40
  measured stage peaks within 1 GiB. See W3 below.

Out of scope for the epic, tracked in the [Enter the Verse
roadmap](../roadmap/2026-10-20-enter-the-verse.md): streamed tokens and the
public flag, longer job inputs, the Grid's group visuals beyond the existing
Pylon layer, agent tool use on V4.1, batching, expert parallelism, DSpark
speculation, vision, and paid jobs.

## Where things stand

### Done

| Area | Issues | What it gives the epic |
| --- | --- | --- |
| Spend guard | #12370, #12376 | `scripts/dev/cloud-budget.py`: $3,000 cap, expiry, one budget host, reserve, refusal on a damaged ledger. Billing alerts at $1,500 and $2,500. Spent so far (2026-10-21 09:00Z): $102.18 compute plus a $200 reserve. |
| Protocol | #12372 | V4.1 prompt rendering, token encoding, and streaming reasoning/tool parsing from the vendored `deepseek-recipe`, byte-exact against the checkpoint's tests. Protocol only. |
| Weights | #12373 | `psionic_models::deepseek_v41_weights`: exact FP8/FP4/E8M0 views, layer-range selection, identity. On the real checkpoint: 96,085 tensors open, 48 shard hashes match, 19 of 19 dequantization cases bit-exact. |
| Oracle | #12371 | 18,177 reference tensors for four prompts plus edge cases, in decode, replay, and chunked modes, with full logits, precision notes, tie rules, and complete answers. Slices in `testdata/deepseek_v41/oracle/`, read by `psionic_models::deepseek_v41_oracle`; the rest in `gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/`. Weights in `gs://openagents-psionic-models/`. |
| CPU components | #12377–#12384 | `deepseek_v41_cpu`: hyper-connections, routing and experts, attention primitives, Engram, projection wiring, RoPE/YaRN, activation quantization, and request-owned shared KV/indexer state across the 40-layer owner schedule. 45 tests on synthetic fixtures; the Engram token map matches the reference over all 129,280 entries. |
| Native serving shape | #12358, #12364, #12367, #12368 | `--single-generation`, Pylon's managed worker recovery (now for native Psionic workers too), the `pylon up` text profile, and native Qwen2 CUDA for the R1 distill. |
| Group beacon and routing | #12400 | W13: NIP-PYLON v2 group head and stage beacons (one serial slot, exact artifact, deadline, output limit, readiness, generation) with v1 fixtures in both directions; `pylon::group` readiness source for W10; the router and `client::choose_model` route a request to a group only when it names that model and the group is ready, never to a stage, never substituting. `pylon cloud up` (W15, #12402) publishes them. |

### CPU assembly completed (Codex, DeepSeek chat)

| Issue | Scope |
| --- | --- |
| #12385 | Full block wiring: hyper-connection handoff, norms, experts, Engram, with float32 and BF16-reference arithmetic |
| #12386 | The full 40-layer CPU transformer to logits |
| #12387 | Greedy decoding and an offline runner for the pinned checkpoint |

These three issues are closed on `main` through `d912ed9392`. The complete
model and greedy runner pass 57 CPU tests and two argument tests on synthetic
fixtures. W1 (#12388) now reports original-weight comparisons and numerical
fixes; G1 still fails. [#12404](https://github.com/OpenAgentsInc/openagents/issues/12404)
tracks the remaining connected divergence and missing observations. The
[W1 report](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/w1-2026-10-20.json)
distinguishes complete runs, first-failure runs, and untested positions.

## Lessons that shape the remaining work

- **Compare like with like.** The reference's own decode and replay of the
  same tokens differ by about 1 to 5 in logits, and one top token flips. That gap is context only: it does not set or relax same-mode tolerances.
  Fix per-component bounds from the stated arithmetic before comparison,
  measure same-mode repeatability separately when available, and never compare
  Psionic's decode with the reference's replay.
- **Exact router ties exist.** The 6th and 7th expert scores are sometimes
  exactly equal, and the reference's order between them is unspecified. Skip
  exact ties, report near ties, and keep Psionic's tie rule (lowest id)
  documented.
- **V4.1 is not an encoder/decoder stack.** All 40 layers are one block type
  and run on every decode step. Layers 21–39 reuse layer 20's compressed KV
  and candidates; layers 3–19 reuse layers 2, 8, and 14. A stage boundary
  must carry whatever later layers reuse.
- **Host-stepped GPU decoding is too slow.** The R1 native CUDA path keeps
  normalization, rotation, activation, and sampling on the host. It passed
  its checks, but a long greedy answer took 83 seconds, past Pylon's
  75-second limit. V4.1 has far more per-layer work (four-stream mixing,
  three attention modes, indexers, Engram), so its CUDA path must keep a
  whole step on the device, with CUDA graphs where the shapes allow.
- **Cloud capacity moves.** us-central1 had no A4 spot capacity on
  2026-10-20; us-east1-b did, and the spot machine was reclaimed after 49
  minutes. Upload results as they're produced, and keep the guard's expiry
  on everything.

## Remaining work, in order

Each item is a proposed issue under #12369. Sizes: S under a day, M one to
three days, L longer (split before claiming). "Cloud" items run through the
spend guard from the one budget host.

### Phase 2 finish: qualification and model measurements (G1, G2)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W1 | **Original-weight CPU parity report.** Compared original weights with the frozen policy. Complete chat diagnostics and first-failure runs report every case, mode, and recorded position, including untested positions. G1 fails; #12404 tracks the divergence and missing observations. | M, cloud | #12385–#12387 | Report retained; closing W1 does not pass G1. |
| W2 | **Complete answers and quality. Evidence collected; acceptance pending the frozen G2 v2 policy.** On the served CUDA runtime, p1 (thinking) and p2 (chat, the oracle's exact 40 prompt IDs) finish at the end token in 121 and 289 tokens, identically when repeated, and split correctly with the #12372 parser; p2 through Pylon equals the chained answer byte for byte. p1 passes its assertions; p2's code is the oracle's own loop and fails the F(186) boundary (30ba21cb1e), as the oracle's answer does. CPU diagnostics (`ReferenceBf16Tp8`): p1 64 tokens, p2 288, neither equal to the oracle. [Receipt](verification/deepseek-v41-w3-w2-2026-10-21.json). | S, cloud; quality suite may need a separate estimate | GPU runner; frozen G2 policy for acceptance | G2 v2 passes; CPU evidence and its limitations are reported separately. |
| W3 | **Done, provisional (#12390).** Measured on four RTX PRO 6000s (commit 6e59f4de54, contexts 8,192 to 1,048,576): a 10-layer stage peaks at 76.5–78.1 GB at 8,192 positions; a stored KV-source row is 1,280 bytes (KV 1,024, indexer keys 256), so 640 bytes a position at ratio 2 and 1,280 at ratio 1; a group's one-step prefill sizes step buffers at 184,356 bytes a token, which dominates long contexts (24.2 GB a stage at 131,072); an Engram decode lookup reads 6,336 bytes from pinned host memory in 6.2 µs in stream (14 µs waited); a decode token reads 16.13 billion parameters, 13.07 GB. `MeasuredMemory` gives W9's placement these inputs. [Receipt](verification/deepseek-v41-w3-w2-2026-10-21.json). | S, cloud | Existing loader and running stages; no numerical-parity blocker | Measured placement inputs name the runtime and context range; provisional measurements are repeated if qualification fixes change allocation or performance. |

### Phase 3: CUDA (G3, G4)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W4 | **Done on SM89 (#12391).** `psionic_backend_cuda::v41_matvec`: `v41_act_quant_fp8` and `v41_block_matvec` over packed FP8 32×32-block and FP4 per-32 weights, with device-selected matrices from a stack for routed experts and no host round trip. Parity on the RTX 4080 against the CPU path on committed real rows; decode reads 409–591 GB/s (57–82% of the 4080's 717 GB/s). SM120 parity moves to W8. | M | — (checked against the CPU path, which is bit-exact against the reference's dequantization) | Parity with the CPU path on real rows on the 4080 and the target GPU. |
| W5 | **Done on SM89 (#12392).** `psionic_backend_cuda::v41_attention`: tail RoPE and YaRN, the window ring and cache quantizers, window and compressed latent attention with the sink, the ratio-1 and ratio-2 compressor and compressed-row store, dense products in the CPU's order, index scores, top-512 selection, and layer-20 candidate blocks, with consumers reading the KV sources on the device. Parity on the RTX 4080 against the CPU path's default `ReferenceBf16` order, including the 127/128/129 prompts and 20,021 positions; selections equal the oracle's at all eight index sources. One decode step's attention takes 82–84 µs on a layer with 512 selected rows. SM120 parity moves to W8. | L (split per mode) | W1 | Per-kernel parity on both GPUs, including the 127/128/129 cases. |
| W6 | **Done on SM89 (#12393).** `psionic_backend_cuda::v41_mix`: hyper-connection mixes (joint RMS projection, pre/post, 20-iteration Sinkhorn), collapse, and expand; the Engram gather from page-locked host memory and the Engram gate; router logits, routing with ties to the lower id written as `V41Slots` ids, clamped SwiGLU, and the expert sum. Parity with the CPU path on real weights and oracle inputs on the RTX 4080; one decode step's kernels take 1.8–12.7 µs each. SM120 parity moves to W8. | M | — (checked against the CPU components, as W4) | Per-kernel parity on both GPUs. |
| W7 | **Built (#12394); G4 not passed.** `psionic_backend_cuda::v41_stage::V41Stage` runs a contiguous layer range on one GPU: embeddings, Engram (page-locked host tables, hashing on the device), mixes, attention, MoE, and on the last stage the head and sampling. All request state stays on the device; each token is one captured CUDA graph, and a decode step is one launch and one wait (`nsys`). W9's contract gained the pre-mix. On the RTX PRO 6000: layers 0–4 decode in 4.13 ms per token and prefill 242 tokens/s in 39.5 GB; layers 0–9 in 8.06 ms and 122 tokens/s in 76.6 GB. Default `ReferenceBf16` order; the connected comparison with the CPU stage fails the frozen policy from layer 1 (mixes, then router sets), and GPU and CPU fail the oracle alike. | L | W4–W6 | G4 passes for a 5- or 10-layer stage on the target GPU; per-token latency and peak memory are recorded. |
| W8 | **Done (#12395): the RTX PRO 6000 (SM120, G4 machines) is the target GPU; B200 gets no separate profile.** On SM120, the `v41_matvec`, `v41_mix`, `v41_attention`, and `v41_stage` suites pass (9, 15, 14, 7 tests), bit-exact host-model checks included; large FP8 matrices decode at about 1.1 TB/s; W7's layers 0–4 take 4.13 ms per token in 39.5 GB and layers 0–9 8.06 ms in 76.6 GB, so a 10-layer stage fits one 96 GB card. A `g4-standard-48` costs $1.66/h spot ($4.50 on demand) and a `g4-standard-384` $13.31/h spot, against $39.63/h spot for an `a4-highgpu-8g`; spot quota is 16 RTX PRO 6000 GPUs per region. The portable kernels use no SM100-only feature, and B200 was needed only for the reference oracle. Reopen a B200 profile for throughput work (batching or expert parallelism, V9), for kernels that use `tcgen05` or other `sm_100a` features, or if G4 spot capacity or quota can't hold a group. Pinned toolkit: image family `common-cu129-ubuntu-2404-nvidia-580` (CUDA 12.9, driver 580), checked by `stage_vm.sh`. `quantized_matvec.cu` now builds with CUDA 12.x and 13.x include layouts. | S, cloud | W7 | A recorded decision with parity on the chosen GPU. |

### Phase 4: stage Pylons (G5, G6, G7)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W9 | **Done (#12396).** Stage-state codec and group manifest: `psionic_models::stage_state` (message, codec, `StageInbox` admission, `StageWorker`) and `psionic_models::deepseek_v41_stage` (V4.1 boundary contracts, wire sizes, `V41GroupManifest`, `plan_group`); `psionic_serve::pipeline_stage` adapts the Gemma 4 step to the same message. Placement reads header-derived bytes through the `PlacementMemory` trait until W3's measured input implements it. | M | W3 for measured placement only | Codec fault tests (duplicate, stale, missing, reordered, cross-request, malformed, oversize) pass with a synthetic 40-layer executor; every split matches the unsplit run bit for bit. |
| W10 | **Group lifecycle.** Done (#12397) on synthetic stages: `pylon::stage_group` runs each stage as a managed worker under the serial supervisor's process-group owner and restart budget. A group is ready only when every stage agrees on weights, runtime, protocol, and generation and passes a check. A replacement bumps the generation and fences the survivors; failures and cancellation hold admission until every stage stops or is reaped. The front's control identity is W9's (generation, request ID, attempt, stage, step). `StageGroup` implements W13's `GroupReadiness`. The last stage samples with the front's settings and returns token IDs, not logits. `crates/pylon/tests/group_lifecycle.rs` covers G7's fault list; G7 still needs it on a real group (W11, W12). | L | W9, #12364 | G7's fault checks pass on a local or synthetic group. |
| W11 | **A full group on one machine.** All stages on one 4- or 8-GPU machine over loopback. **Part 1 done (#12398): the stage worker.** `psionic-v41-stage` (`crates/psionic/crates/psionic-v41-stage`) takes `pylon cloud up`'s arguments (`--weights DIR --layers A-B --gpu N --manifest D [--embeddings] [--head]`, then `--stage --generation --front --token`), speaks W10's control protocol, and announces `psionic-v41-stage/1`. Stage state is W9's message, sent as length-prefixed frames to the next stage's loopback listener, whose `listen` address the front now passes as `next` with each step; the front also passes the previous token to stage 0. Stage 0 encodes the conversation with the #12372 adapter; the last stage samples greedily or with the front's temperature, top-p, and seed, and returns the token, its text, the end flag, and (W14) reasoning and final text from the adapter's streaming parser with a `stop` or `length` finish. The executor is W9's `StageExecutor`; `--backend cpu` runs `psionic_models::deepseek_v41_stage_cpu`, the CPU port cut at a layer range. Tests: Psionic process groups of 2–4 stages and Pylon's real front over loopback match the unsplit model's tokens; W10's lifecycle faults (kill, stop, cancel, other weights) pass against it; refusals cover wrong generation, stale state, missing weights, and a layer range that doesn't match the manifest. **Part 2a done (#12398):** `--backend cuda` (the `cuda` feature) runs W7's `V41Stage` as W9's executor: residuals and pre-mixes cross as `F32`, KV rows and index keys as BF16, selections as `I32`, and layer-20 candidates as `U32`, and the last stage samples on the device and returns the token. A CUDA stage announces `psionic-v41-stage/1+cuda` and a CPU stage `psionic-v41-stage/1`, so the front never makes a mixed group ready. The worker also speaks W12's direct transport (`--data-listen`, `--data-advertise`; `link`, `linked`, `link_failed`, `forwarded`, `refused`, and a cancel frame from stage 0) over `crates/stage-link`, which Psionic links by path. A checkpoint stage opens only its own shards (`DeepSeekV41Checkpoint::open_stage`) and hashes them against the pin. `v41-stage-startup.sh` also copies the group manifest beside the shards and the tokenizer files, and refuses a worker built for another GPU; `scripts/pylon/v41-stage-build.sh` builds the worker for linux/x86_64 in Docker with CUDA 13.0 on a GPU host and uploads it, with its CUDA libraries, under `gs://openagents-psionic-models/bin/psionic-v41-stage/COMMIT/sm_NN/`; its `test` mode runs the crate's tests on that host's GPU. On the RTX 4080 (sm_89), CUDA process groups of two and three stages answer like the unsplit CUDA stage, greedy and sampled. The process tests bound every wait, so a stage that exits or never connects fails the test with its log. **Part 2b done (#12398):** the `one-quad` placement puts all four stages on one `g4-standard-192`, and `pylon cloud up one-quad --head stage0 --transport direct` ran the whole model there and answered jobs over the relay (G5 above: 1.3 s to the first token of a 41-token prompt, then about 29.7 tokens/s). The run fixed the zone head's launch (the budget host no longer checks stage programs that live on the stage machine, and the head starts once the machine's weights are in place), priced the `g4-standard-192` in the guard, provisioned boot-disk throughput, added `--temperature`, `--top-p`, `--ready-secs`, and `--step-secs`, and logs each answer's first-token time and decode rate. `v41-stage-build.sh` reuses a persistent source folder, so an unchanged worker rebuilds in under a second, and its `pylon` mode builds the head's Linux binary. Direct oracle qualification remains pending under G1 v2; runtime and fault checks can proceed independently. | M, cloud (G4 spot, about 20 h) | W7, W10 | G5 passes. |
| W12 | **A group across machines.** Two placements across separate machines in one zone. **Part 1 done (#12399), locally:** `crates/stage-link` sends W9's stage-state bytes straight from stage to stage over an authenticated, bounded TCP link keyed per group generation, and `pylon::stage_group`'s `direct` transport uses it: the head sends each step to the first stage only and hears only the last stage's token, while it keeps admission, sampling, cancellation, fencing, and readiness. `pylon cloud up --head stage0\|vm --transport direct` runs the head on the stage-0 machine or a small machine in the zone, with the budget host keeping the spend guard and talking only to the head. `tests/group_transport.rs` covers a dropped link, an old generation's link, a duplicated frame, a partitioned middle stage, a slow stage behind a bounded queue, a stalled stage, and cancellation down the chain on fake stages in separate processes. W11's worker speaks `stage_link` with `--data-listen` and `--data-advertise` (#12398 part 2a). **Part 2 done (#12399):** `four-single` and `two-dual` ran across machines in us-west3-a with the CUDA worker and the direct transport, answered every qualification prompt, and passed G7's kill, freeze, cancellation, machine-loss, and `down` checks (G6 and G7 above). Stages report each step's own time, and the head's answer log splits a decode token into stage compute and link time. A killed CUDA stage now gets 30 s to be reaped. G6's numerical acceptance waits for the G1 v2 runtime. | M, cloud | W11 | G6 passes; G7's fault checks repeat on the cross-machine group. |

### Phase 5: launch and visibility (G8)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W13 | **Group beacon and routing.** Done (#12400): the group publishes one routable beacon (one serial slot, the exact model artifact, its declared deadline and output limit); stage beacons are visible but not routable. The router matches requested model and group readiness. W10 feeds readiness through `pylon::group::GroupReadiness`, and W15 publishes the beacons. | M | W10 | Router tests pick the group only for V4.1 requests and only when it's ready. |
| W14 | **Deadline and answer profile.** Part 1, the transport, is done (#12401): `pylon.deadline.v1` in NIP-PYLON. The buyer states its budget; the job runs for the smaller of that and the beacon's ceiling, enforced inside execution from admission; heartbeats renew nothing; a missed deadline holds a group's slot until its stages stop; old providers and long models refuse the other side's form at once, with recorded fixtures both ways; ordinary jobs keep 90 seconds; group mode declares up to 3,600 seconds. The harness (`run.py --v41-group`, through the router's `Settings::want`) passes a slow synthetic group and fails one past its deadline. **Part 2 done (#12401):** the frozen profile is a 600 s deadline, a 4,096-token output limit, a 75 s step limit, and prompts of up to 2,872 tokens (#12408's checked budget; the receipt's 3,022-token prompt needs recounting); every qualification prompt answered completely within it in both cross-machine placements (G8 above). The head publishes its measured `answer_profile` in its beacon's `meta` and counts complete answers only. Requalify it on the G1 v2 runtime or after any arithmetic or prefill change. | M | W11, W13 | Complete answers for the qualification prompts through Pylon within the declared deadline. |
| W15 | **Built (#12402) on synthetic stages; the cold launch is an after-close check that W16 runs.** `pylon cloud up four-single\|two-dual` plans the machines from Psionic's manifest file, creates each through `scripts/dev/cloud-budget.py` with an expiry and labels, heads the group from the allocating host in group mode (`pylon::group_serve`: W10's `StageGroup`, one W13 head beacon, a beacon per stage, NIP-CJ jobs driven through the stages), and ends it on `cloud down`, expiry, the budget's cap, or a lost machine, deleting every machine through the guard. Stages run over `gcloud compute ssh` with the control connection tunneled back; `scripts/pylon/v41-stage-startup.sh` copies a machine's shards. `tests/group_cloud.rs` covers launch to answer to `down`, dry-run plans, guard refusals, spot loss, expiry, and the cap. | M | W12, W13 | A cold launch to a first answer, then a clean stop, with costs in the ledger. |
| W16 | **Show it.** Run the launched group with the `/home` Pylon layer open and record the transcript, router log, and capture. | S, cloud | W15 | G8's evidence is committed and the epic can close. |

**Experimental preview (owner, 2026-10-22, #12403).** The one-quad group
serves on the site as an experimental preview before qualification: G1 v2
failed under policy 2.1 (#12405), and G2 is being reworked. This doesn't
pass G1, G2, or G8, and the epic stays open. The launch is labeled on
every surface: its beacons' name is "DeepSeek V4.1 Flash · Experimental
preview" (`pylon cloud up --label`), every Grid terminal answer from it says
"Experimental preview", and the Grid's Pylon Bay shows it with a preview tag
and the note that its accuracy checks haven't passed yet.

- It runs the pinned bundles and the launch profile unchanged: worker
  `185eb68c…`, head `f1882776…`, `--step-secs 75 --prefill-secs 150`, an
  8,192-token context, 2,872-token prompts, and the regional weights in
  `gs://openagents-psionic-models-ane1/`.
- `scripts/pylon/v41-live.py` keeps it served. It runs `pylon cloud up`
  and, when a spot machine is reclaimed, launches again, zone by zone,
  until its end time (`--until`) or its own spend limit (`--cap`). The
  watcher is the root of an agent run of its own, so it outlives the
  session that starts it. `v41-live.py status` and `v41-live.py stop` check
  and end it; `stop` drains the group and deletes its machines through
  the spend guard.


## Issues

Filed 2026-10-21 as children of #12369, with blockers recorded on the
board. W9 (#12396), W10 (#12397), and W13 (#12400) have no blockers and
started first.

| Item | Issue |
| --- | --- |
| W1 | [#12388](https://github.com/OpenAgentsInc/openagents/issues/12388) |
| W2 | [#12389](https://github.com/OpenAgentsInc/openagents/issues/12389) |
| W3 | [#12390](https://github.com/OpenAgentsInc/openagents/issues/12390) |
| W4 | [#12391](https://github.com/OpenAgentsInc/openagents/issues/12391) |
| W5 | [#12392](https://github.com/OpenAgentsInc/openagents/issues/12392) |
| W6 | [#12393](https://github.com/OpenAgentsInc/openagents/issues/12393) |
| W7 | [#12394](https://github.com/OpenAgentsInc/openagents/issues/12394) |
| W8 | [#12395](https://github.com/OpenAgentsInc/openagents/issues/12395) |
| W9 | [#12396](https://github.com/OpenAgentsInc/openagents/issues/12396) |
| W10 | [#12397](https://github.com/OpenAgentsInc/openagents/issues/12397) |
| W11 | [#12398](https://github.com/OpenAgentsInc/openagents/issues/12398) |
| W12 | [#12399](https://github.com/OpenAgentsInc/openagents/issues/12399) |
| W13 | [#12400](https://github.com/OpenAgentsInc/openagents/issues/12400) |
| W14 | [#12401](https://github.com/OpenAgentsInc/openagents/issues/12401) |
| W15 | [#12402](https://github.com/OpenAgentsInc/openagents/issues/12402) |
| W16 | [#12403](https://github.com/OpenAgentsInc/openagents/issues/12403) |

## Budget

| Phase | Estimate |
| --- | --- |
| Spent: oracle and weights | $36.85, plus a $200 reserve for storage and transfer |
| W1–W3 qualification and measurements | W1 used $12.46 and #12404 $3.30 plus $48.78 (one A4 spot capture and probe session, one CPU machine) in compute. A complete 20k-prompt CPU run is estimated at more than a day (more than $130 per mode). Re-estimate direct GPU qualification before allocating; exhaustive CPU emulation is not a launch requirement. |
| W8 target GPU check | under $50 |
| W11 one-machine group development and runs | about $300 (a `g4-standard-384` at $13.31/h spot, about 20 h) |
| W12 cross-machine placements | about $200 |
| W15–W16 launch and recording runs | about $150 |
| Expected total | Original estimate: about $1,000. The v2 qualification work needs a revised estimate; the spend guard's $3,000 cap still applies. |

## Who can do what in parallel

- **CPU investigation:** #12385–#12388 are closed; #12404 and #12405 retain
  the failed CPU comparisons and remaining arithmetic investigation. Check
  the current issue claim before starting work. These issues no longer
  block provisional W3 measurements or independent GPU/runtime checks.
- **Stage work:** W9 (#12396), W10 (#12397), and W13 (#12400) are closed
  with synthetic or fixture evidence. Their real-model integration remains
  in W11, W12, and W15, with the blockers in the work table above.
- **CUDA (W5–W7)** can use identical-input component checks while G1 is
  unresolved. Each result must name its CPU/oracle inputs and numerical
  bounds. W4 and W6 have such isolated evidence; connected stages still need
  direct oracle qualification under G1 v2. W1's closure cannot establish
  that the CPU model or the served runtime matches the original-weight oracle.

## Open questions

- Tolerances for G1 v2: freeze the new policy before qualification as
  specified in the review addendum. The [fixed v1 CPU policy](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/tolerance-policy.json)
  stays unchanged. Same-mode B200 repeatability is now measured as exact;
  neither that result nor the cross-mode gap determines cross-hardware limits.
- Host RAM per stage for the Engram tables (101.38 GB of pinned host memory
  each, measured by W3, layers 1 and 14) on single-GPU machines. On the
  one-machine `g4-standard-192`, both tables and the page cache fit in its
  720 GiB.
- Redistribution terms of the checkpoint's license for stage packages
  derived from it, before any stage package leaves our buckets.
- The deadline a V4.1 reasoning answer actually needs through Pylon:
  W14's frozen profile (600 s, 4,096 tokens, prompts up to 2,872 tokens)
  holds on the current runtime, and the G1 v2 runtime must requalify it.

## CPU follow-up status (2026-10-21 UTC)

[#12404](https://github.com/OpenAgentsInc/openagents/issues/12404) found and
fixed the first divergence; G1 still fails on the next one, which
[#12405](https://github.com/OpenAgentsInc/openagents/issues/12405) owns. The
[native plan](deepseek-v41-native.md#12404-b200-capture-device-arithmetic-and-the-next-divergence-2026-10-21-utc)
has the capture, measurements, and causes; the
[retained report](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/w12404b-2026-10-21.json)
has every run.

- **Capacity.** A4 spot capacity came back in us-east1-b on the second guarded
  attempt (us-central1-b was stocked out). The spend guard now also takes
  `--flex-start`.
- **Oracle repeatability.** A full rerun on the same B200 setup equals the
  #12371 dump in all 18,000 shared tensors, every residual hash, and every
  logit hash. This measures repeatability, not a zero cross-implementation
  error budget; the frozen v1 float limits remain nonzero.
- **Fixed.** Layer-0 attention now equals the oracle bit for bit: the TP8
  profile reproduces the B200's BF16 tensor-core sums, `MUFU.EX2`, CUDA `expf`,
  TileLang's reductions, FP8 GEMM blocks, and the `wo_a` einsum (b390516b13).
  The 2k prompt's layer-0 router mismatch at position 421 is gone.
- **Next divergence.** Hyper-connection mix values differ by about one F32
  ulp at layer 0 (cuBLAS F32 GEMM order and CUDA `rsqrtf` are unmeasured), so
  BF16 values flip and the difference grows; router sets differ from layer 1.
  The 2k prompt's first router mismatch is now position 1199 and the 20k
  prompt's stays at position 191 (oracle margin 1.1e-4, not a tie).
- **Coverage.** The chat and code prompts and the four edge cases ran completely in both modes with full
  logits and no unobserved tensor; the connected expert FP8 codes and scales
  are now compared. The 2k and 20k prompts ran to their first failure in both
  modes. A complete 20k run is estimated at more than a day on one CPU
  machine; #12405 budgets it after the next fix.

At the end of that run, W2 (#12389) and W3 (#12390) were blocked on #12405.
The review below changes the proposed dependencies. Cost for that session:
$26.55 for the A4 spot machine (0.67 h) and $22.23 for the
`n2d-highmem-96` CPU machine (4.06 h), both deleted.

## Qualification review addendum (2026-10-21)

**Decision: qualify the runtime that serves users directly.** A CPU model
that emulates every B200 instruction and collective is useful for diagnosis,
but it is not a launch prerequisite. Retain the CPU components, original-weight
loader, and existing arithmetic regressions. Investigate additional B200
instructions when a bounded experiment can explain a failed accuracy check;
do not make exhaustive hardware emulation the default next task.

### Correct the premise and preserve the failed evidence

The v1 policy never required every floating-point value to match bit for bit.
Its rule is `abs(actual-reference) <= atol + rtol*abs(reference)`:

| Family | Absolute limit | Relative limit |
| --- | --- | --- |
| F32 mixing and router scores | 0.0002 | 0.0002 |
| BF16 activations and residuals | 0.015625 | 0.015625 |
| F32 logits | 0.03125 | 0.015625 |

Exact checks apply to embeddings, integers, and specified quantized conversions
on identical inputs. Selection sets use only recorded exact-tie exceptions.
The oracle's exact rerun adds no observed stochastic variation; it does not
replace the table with zeros. Floating-point operation order can differ across
devices and batch shapes even without randomness, as described in
[PyTorch's numerical accuracy notes](https://docs.pytorch.org/docs/2.14/notes/numerical_accuracy.html).

The connected CPU model genuinely fails those nonzero bounds. Neither the
plausible sky answer nor the zero-repeatability correction passes v1. Keep the
v1 policy with SHA-256
`38ac6a059ae1385838d0188506912850bc29ba4ffc29f28dea592365deb036c5`
and all W1/#12404 reports unchanged. The former G1 checked original-weight CPU
parity, and the former G2 required the reference's exact complete chat/code
answers. Record new results as **G1 v2/G2 v2**, never relabel a v1 failure.

### Freeze a measurable v2 contract before qualification

The next deliverable is a machine-readable policy and a GPU comparison harness,
not a passing badge. Numerical thresholds are **not established by this review**.
A missing threshold or required observation leaves the gate pending.

1. **Identify the execution.** Record checkpoint, tokenizer and template hashes,
   source and binary hashes, GPU architecture, compiler/libraries, precision,
   quantizers, reduction profile, batch size, prefill chunks, cache layout,
   placement, and decode settings. Compare decode with decode and replay with
   replay. A changed chunk schedule is a separately labeled configuration; a
   decode/replay gap never supplies an error allowance.
2. **Separate calibration from qualification.** The four main prompts and four
   edge cases have already guided fixes; they are regression/calibration data.
   Retain complete declared coverage of them, and add a separate frozen holdout
   with short/long contexts, window/compression boundaries, Engram, chat, and
   code. Freeze sample counts, positions, seeds, scoring rules, numerical
   limits, and the policy/corpus hashes before inspecting holdout results. Use
   local arithmetic error bounds, independently validated reference controls,
   and a stated task-quality budget to justify limits. Candidate failures and
   the old cross-mode gap cannot set them. If a policy changes, retain the
   failure and use a new holdout for the revised claim.
3. **Compare full distributions on a common prefix.** Force the same oracle
   token sequence through both executions, retaining full-vocabulary logits at
   every required recorded position. Compute temperature-1 probabilities with
   stable log-softmax, total variation `0.5*sum(abs(p-q))`, Jensen–Shannon
   divergence, reference-token log-probability error, and top-1 agreement.
   Freeze per-position ceilings and per-prompt/tail limits, not only an overall
   mean. Report centered-logit errors separately so a uniform logit offset is
   distinguishable from changed probabilities. Top-k dumps cannot establish a
   full-distribution bound. Invalid shapes, nonfinite model logits, and missing
   required positions fail.
4. **Use margins without waiving routing.** On identical component inputs,
   derive a score-error budget `epsilon` before the comparison. If the oracle's
   selection-boundary gap exceeds `2*epsilon`, require the same selected set.
   Otherwise permit only substitutions inside that uncertainty band, with a
   frozen cap on their frequency and oracle-score regret: the oracle's sum of
   selected scores minus the sum for Psionic's selected IDs. Always check
   cardinality, unique valid IDs, weights, masks, and candidate reuse. Apply the
   same principle to attention selection and top-1 logits, with their own
   justified budgets. Connected routing also needs measured upstream error;
   an identical-input local bound cannot justify connected-state changes. A
   margin of 1.1e-4 at the known 20k failure is not automatically an allowed
   near tie. Unknown uncertainty leaves that check unresolved.
5. **Keep exact structural checks.** Check tokenizer/template IDs, shard hashes,
   packed-weight decoding and specified conversions on identical inputs,
   Engram hashes and gathered rows, positions, causal/window masks, cache owner
   and lifetime, and stage/request/generation identity exactly. Exact CPU
   split-versus-unsplit tests remain valuable. Keep same-implementation
   deterministic checks where operation order is unchanged; use declared
   arithmetic bounds where a GPU schedule changes it. Do not demand identical
   connected FP8 codes after already different floating-point inputs.
6. **Check free-running answers separately.** Teacher forcing isolates numerical
   error but cannot establish answer quality after a different token. Run the
   same frozen tasks independently to EOS on the oracle and served runtime.
   Require all designated must-pass task assertions, code compilation/execution
   tests, correct stop/cap behavior, and token-level reasoning/final parsing.
   Predeclare a paired quality noninferiority margin and uncertainty method for
   the broader suite; report prompt-level outcomes and sample size. Preserve
   exact token match as a diagnostic, not the only definition of correctness.

For the token-10 sky divergence, capture both logits on the common prefix and
measure its margin and distribution error. The text alone establishes neither
a defect nor acceptable numerical drift. The one-machine code answer used
thinking mode whereas the oracle's p2 used chat mode, so that answer is not
paired p2 qualification evidence.

There is also a concrete code-quality failure in that receipt. Compiling and
running its generated Fibonacci function gives `fib(186) == None`, although
`F(186) = 332825110087067562321196029789634457848` fits in `u128`. The loop
attempts to compute F(187) before returning F(186). The local check confirms
that `fib(185)` succeeds and `fib(186)` fails; an independent integer recurrence
confirms the representable boundary. See the
[code-answer review](verification/deepseek-v41-code-answer-review-2026-10-21.json).
Add inputs 0, 1, 185, 186, and 187 to the answer checks, requiring the correct
value whenever F(n) fits. This is a generated-answer defect, not a demonstrated
GPU arithmetic defect. Agreement with an oracle answer would not excuse a
failed must-pass task assertion.

The current CUDA path's 180/625 failed v1 stage checks and CPU's 191/625 failures
also remain evidence to investigate. Similar failure counts do not prove equal
quality. Run direct, identical-input stage checks to locate arithmetic defects,
then connected full-model checks to measure amplification. Validate the harness
with deliberately wrong weights, RoPE positions, routing, and stale cache state;
these negative controls must fail. The policy must detect implementation bugs
as well as tolerate justified arithmetic differences.

### Apply the review to all eight open issues

This is the issue snapshot at review time; existing claims continue to apply.
Comments should explain these changes before a claimed issue's owner updates
its implementation scope.

| Issue | Change or next action |
| --- | --- |
| #12404 | Retain the failed CPU report and missing long-context coverage. Add this policy correction; do not close it as a G1 pass. Track the v2 policy/harness and complete coverage explicitly with #12405. |
| #12405, claimed | Replace exhaustive B200 emulation as the launch prerequisite with a bounded v2 policy/harness and direct SM120 qualification. Preserve useful measured arithmetic fixes. The owner should revise the issue body, estimate, and acceptance checklist before continuing; changing policy alone does not finish the issue. |
| #12389 (W2) | Keep CPU generation as an explicit diagnostic deliverable. Add complete GPU answers and the G2 v2 task suite, with separate status for CPU evidence and served-runtime acceptance. Do not require exact B200 token identity for release. |
| #12390 (W3) | Remove #12404/#12405 as numerical-parity blockers. Measure the current original-weight runtime now, label results provisional, and provide allocation inputs for placement. Repeat affected measurements after fixes. |
| #12399 (W12), claimed | Continue transport, placement, and real-group fault checks on an explicitly unqualified runtime. G6's final numerical acceptance still needs the qualified runtime in both placements. |
| #12401 (W14), claimed | Continue answer, cancellation, and deadline measurements now. Pair the actual oracle chat/code modes; freeze caps/deadlines before acceptance. Report TTFT and total answer time for short and long inputs. Final G2/quality evidence still needs the qualified runtime. |
| #12403 (W16) | Keep the launch blocked on G1–G8 evidence under the versioned contract, not CPU hardware emulation. A visible group and a plausible answer alone do not pass it. |
| #12369 | Update the epic's gate summary to G1/G2 v2 and retain the failed v1 record. Psionic remains the only serving engine; CPU component checks remain part of G3. |

Priority: freeze the v2 policy and build the direct GPU comparator first;
run provisional measurements and lifecycle/deadline checks independently;
then complete numerical/quality qualification and both placements before launch.
New B200 time should buy missing oracle evidence, calibration controls, or a
specific diagnostic experiment, rather than an open-ended instruction inventory.

### Billing and prefill follow-ups

The billing guard fix already landed in `d85e58b2f0`: flex-start settlement
uses the machine's start time rather than its queued-request time. The native
plan's earlier sentence that queue time is charged is superseded. Google's
[capacity documentation](https://docs.cloud.google.com/ai-hypercomputer/docs/obtain-capacity)
states that flex-start resources are charged when in use. This does not verify
the disputed $136.53 against the bill. Reconcile
`oa-dsv41-12405-europewest4b` using its instance ID, audit start/stop events,
and billing export by SKU/time, separating GPU/CPU usage from disks and other
charges. Keep budget reservations, estimated settlement, and actual billed
usage distinct; do not refund a ledger entry solely from an absent running VM.

The [one-machine receipt](verification/deepseek-v41-g5-one-quad-2026-10-21.json)
records 35.6 s TTFT for 1,072 tokens and about 120 tokens/s per serial stage.
First test batched prefill within one stage, then bounded chunk overlap across
the four stages. These are separate optimizations: measure GPU utilization,
memory, stage/link time, TTFT, and total answer latency for each. With one
request's dependent chunks, respect KV/compressor/indexer and Engram state
order and the serial group's cancellation/admission rules. Requalify any
changed arithmetic schedule. Freeze the W14 deadline target and context range
first; make a prefill change a launch blocker only if that profile misses its
declared budget. Keep larger-context throughput work separately scoped.

## Build and arithmetic identity (#12407, 2026-10-21)

Audit findings F1, F2, F3, F5, and F9 of the
[six-hour review](../audits/2026-10-21-deepseek-six-hour-review.md) are fixed
on `main`. A V4.1 result now names the executable, libraries, target, and
arithmetic it ran:

- **Target (F1).** `psionic-backend-cuda` rebuilds its kernels when
  `CUDAARCHS` or `PSI_CUDA_ARCH` changes and records the architecture it
  compiled for. `scripts/pylon/v41-stage-build.sh` always passes an explicit
  architecture (the host GPU's for `test`), loads
  `scripts/dev/cargo-build-env.sh` in the container, and refuses a worker
  whose fatbinary (`cuobjdump --list-elf`) holds anything but that
  architecture. `build --no-upload` runs the same checks without uploading.
- **Executable (F2).** `BUILD.json` schema v2 lists the worker's and each CUDA
  library's SHA-256, the embedded architecture, the worker's `build-info`
  and profile, the manifests by digest (taken from `--commit`), the tree, the
  base image digest, and the builder image ID; `GENERATIONS.json` records the
  uploaded objects' generations. The stage startup script installs a build
  only when it matches `BUILD.json` and the optional pin
  `pylon cloud up --worker-build SHA256` (the SHA-256 of `BUILD.json`;
  `--pylon-build` pins the head). `stage-run` and `head-run` recheck the
  hashes before they start. Old v1 builds are refused: rebuild before the
  next group launch.
- **Arithmetic (F3).** `--arithmetic` is parsed for the backend before
  anything loads. The CPU backend takes `f32`, `bf16`, and `bf16-tp8`; CUDA
  takes only `device`, its own kernels' arithmetic. A stage's hello reports
  `build` (its executable's SHA-256 and CUDA architecture) and `profile`
  (such as `cuda-device-sm_120`), and Pylon's front refuses a stage whose
  build or profile differs from the group's.
- **Readiness (F5).** Startup removes the ready mark before changing
  anything and writes it last, naming the boot ID, manifest, and build.
  `stage-run` and the head's ssh probe accept only a mark from the current
  boot.
- **Synthetic data (F9).** The synthetic generator's draws are in
  `[-1, 1)`. Synthetic CPU, GPU, and process comparisons from before this
  change used the old distribution; label them as the old fixture.
- **GPU numbering.** `--gpu N` counts the devices the process may see: the
  physical index without `CUDA_VISIBLE_DEVICES`, and the Nth entry of an
  inherited mask otherwise.

Checks on `coderos-4080` (RTX 4080, `sm_89`): three `build --no-upload` runs of
`a12ffcb141` on one cache, `sm_89`, then `sm_120`, then `sm_89`, each
recompiled the kernels, and `cuobjdump` found exactly the requested
architecture each time. The worker's bytes differ between the two `sm_89`
builds, so compare builds by `BUILD.json`, not by expecting a bit-identical
rebuild. `v41-stage-build.sh test` passed every CUDA test on the 4080
(`PSIONIC_REQUIRE_CUDA=1`), including the synthetic split-against-unsplit
comparisons with the corrected generator. Running the `sm_120` build on an
RTX PRO 6000 is an after-close check.

### Bundle pins (#12410, review R1)

The [remediation review](../audits/2026-10-21-deepseek-six-hour-review.md#review-of-the-remediation-response-2026-10-21)
found that the identity above covered only the executable. Now the
**bundle hash**, the SHA-256 of `BUILD.json`, identifies a build. Because
`BUILD.json` lists the worker and every library, two bundles with the same
executable and different libraries have different hashes.

- A worker verifies the `BUILD.json` beside it against its own executable
  and `lib/`, and announces the result as `build.bundle_sha256` in its
  hello and `build-info`. An unverified build announces `null` and the
  reason in `build.bundle`.
- `pylon cloud up` requires `--worker-build`, plus `--pylon-build` for a
  head in the zone. `--development` launches without the pins. The launch
  record, receipt, dry run, and `status` say whether the launch is
  `pinned` or `development`. The group configuration carries the pin as
  `build`, and the front refuses any stage that doesn't announce that
  bundle. A development group only requires every stage to match the first.
- `psionic-v41-stage qualify` requires `--worker-build` or `--development`.
  The report records `stage_bundle_sha256`, `stage_executable_sha256`,
  `expected_bundle_sha256`, and `launch`.
- `stage-run` checks the installed `BUILD.json` against the build hash in
  its path and ready mark before it checks the files, so a library replaced
  together with `BUILD.json` is refused. `head-run` checks the pylon's
  installed `BUILD.json` in the same way. Startup logs an unpinned build
  as a development launch.
- `scripts/pylon/v41-build-check.py` is the single checker.
  `pylon cloud up` passes it to each machine as `oa-verify`. Check any
  other download, such as the calibration setup's, with
  `v41-build-check.py build DIR MANIFEST --pin SHA256`.

The review's follow-up fixture
(`follow-up-reproduce.py`) now passes its `checks_12410`. Running the pinned
`sm_120` bundle on GPUs is an after-close check.

### Raw text stream (#12412, review R3)

`TextStream::push` decodes only the tokens since the last emitted text,
plus that text's tokens for context. It no longer decodes the whole answer
on each call. At 4,096 tokens it passes 13,997 token IDs to the tokenizer
instead of 8,390,656. On an Apple-silicon CPU with the fixture tokenizer,
the raw text stream and the answer parser together take about 4.8 ms for
4,096 tokens, down from about 400 ms; the answer parser alone takes
3.4 ms (`cargo test -p psionic-v41-stage --lib measure_parsing -- --ignored
--nocapture`). Both the answer parser and the text stream are now
incremental. Each token's text matches the whole-answer decode, including
split characters and special tokens. The W14 per-token slowdown on GPU has
not been measured again.

## Supported context and mode profile (#12408)

This section answers audit findings F6, F7, and F8 of the
[six-hour review](../audits/2026-10-21-deepseek-six-hour-review.md).

### Request budget

The prompt is one prefill step, so the largest prompt a group serves is the
largest prefill that every stage boundary carries under the codec's 256 MiB
message limit. `StageContract::max_step_tokens` computes it from each
boundary's real section sizes, with the codec's longest (128-byte) request
ID. `psionic_v41_stage::budget` takes the smallest over the group's
boundaries, then bounds it by the context and the operator's cap:

| Wire | Split | Largest prefill (tokens) |
| --- | --- | --- |
| CUDA (F32 residuals, BF16 cache rows) | 0-20, 20-40 | 3,275 |
| CUDA | four or eight even stages | 2,872 |
| CPU (F32 everywhere) | 0-20, 20-40 | 3,275 |
| CPU | four or eight even stages | 2,833 |

The residual alone, `[tokens, 4, 5120]` in F32, passes 256 MiB at 3,277
tokens. A boundary past layer 20 with an index source still ahead also
carries layer 20's candidate blocks (8 KiB a token) and an index selection,
which lower the bound to 2,872.
`crates/psionic/crates/psionic-v41-stage/tests/budget.rs` pins these numbers,
and `deepseek_v41_stage_tests::prefill_sizes_match_encoded_messages_and_bound_the_step`
checks the computed sizes against encoded messages.

A stage serves a request when:

- the prompt has at most `max_input_tokens`, which is the smallest of the
  largest prefill, `--max-context` minus one, and `--max-input-tokens`;
- the prompt plus the front's answer limit fits `--max-context` (8,192 by
  default).

With the default context, the four-stage CUDA profile takes prompts up to
2,872 tokens, and W14's frozen profile uses that bound. The oracle's p3 prompt (2,124 tokens) fits; its p4 prompt
(19,989 tokens) does not, and the served profile does not cover it. This is
a wire bound, not a deadline bound. W14 measured a prompt step of 52 s for
its longest prompt, inside the 75 s step limit; a later runtime or
placement that prefills more slowly sets `--max-input-tokens` to the
length it qualifies.

Chunked prefill isn't implemented. Each front step is one inbox step today,
so a chunk schedule needs a step mapping between the front and the stages,
cancellation across chunks, and its own qualification. The owner decided on
2026-10-21 to launch with the 2,872-token profile; longer prompts, p4, and
step buffers sized by chunk are follow-up work in
[#12414](https://github.com/OpenAgentsInc/openagents/issues/12414).

### Advertising and refusing

- Every stage announces `limits` in its `hello`: `max_context_tokens`,
  `max_input_tokens`, `max_prefill_tokens`, and `modes`. The front reports
  the tightest limits of its stages in `StageGroupStatus::limits`, which the
  group head's status and `live.json` carry, and the head's beacon carries
  them in its inert `meta` as `request_limits` beside `answer_profile`.
- The front sends the answer limit with step 0 (`max_tokens`). Stage 0
  encodes the prompt and checks the budget before any stage runs the
  request. A request over a limit, or a prompt the stage can't read, ends
  with `rejected` (`code` `request_limit` or `invalid_request`). A stage
  whose own state can't be encoded rejects the request the same way. The
  front fails only that request, with `Failure::request_limit` for a size
  limit, and replaces no stage. The front also refuses at admission an
  answer limit that can't fit the context.
- The group head answers the buyer with `limit_exceeded` and the reason.

### Stage-state handoff

A stage's readers hand stage state to its worker through one queue bounded
by `InboxBounds`: 4 frames and 512 MiB by default (one frame may exceed the
byte bound when nothing else waits). A reader that finds the queue full
stops reading its connection, so the previous stage's bounded link sender
fails within its send deadline; the reader drops its own link after 30 s.
Front messages, link cancellations, and link reports skip the bound and go
before queued state, so a cancellation or fence runs next. Decoded frames
the stage keeps for a later step are bounded by the same bytes.
`tests/handoff.rs`
stalls the real CPU executor, floods its authenticated link with valid
690 KB frames, and checks that the sender fails within its 1 s deadline,
that fewer than 40 frames are accepted, that the process grows less than
64 MiB, and that cancellation, a fence, and a new request then work.

### Prompt modes

The stage renders the conversation and parses the answer in thinking mode
(the default, as before) or chat mode, which the oracle's p2 code prompt
uses. The front sends `mode` with every step, and a direct link carries it
in each frame's settings. To request chat mode:

- in a stage group, call `StageGroup::generate_prompt_in(prompt,
  PromptMode::Chat, steps, sampling)`;
- for a group head, set `"mode": "chat"` in the share file's group lane;
- for the unsplit reference, run
  `psionic-v41-stage reference --weights DIR --prompt JSON --mode chat`.

`tests/answer.rs` checks that chat mode renders the p2 prompt to exactly the
oracle's 40 token IDs, and that a chat-mode continuation matches the
checkpoint's second encoding test. The reduced tokenizer fixture now
includes the p2 prompt's vocabulary; `refresh_deepseek_v41.py` checks the
oracle's IDs against the full tokenizer.

### Qualification runs

`crates/pylon/tests/group_stage_worker.rs` skips without a worker unless
`PSIONIC_V41_STAGE_REQUIRED=1`. In that mode a missing or unreadable worker,
a worker whose SHA-256 differs from `PSIONIC_V41_STAGE_SHA256`, or, with
`PSIONIC_V41_STAGE_BACKEND=cuda`, a machine without a GPU fails every test.
Each test prints the executed, skipped, and failed counts, and
`PSIONIC_V41_STAGE_REPORT=FILE` collects one JSON line per test.

## Inference Engineering reading addendum (2026-10-21)

The [book adaptation](inference-engineering-lessons.md) maps Philip Kiely's
*Inference Engineering* to this implementation and the existing issues.
Measure internal first-token time separately from complete buyer answers;
our group engine currently buffers the answer and keeps reasoning private.
For prefill, qualify bounded chunks, batched computation, and cross-stage
overlap as separate changes. Prefix reuse must restore V4.1's complete stage
state, including compressors, indexer/candidates, and Engram history.
Keep the serial profile and current gates. The narrower context release now
has that explicit revision (see "Launch scope revision" under the gates). Multiuser batching, expert
parallelism, and prefill/decode disaggregation remain workload-driven
follow-ups. The reading introduces no new benchmark result or passing gate.


## Serial long-prompt implementation (#12414, 2026-10-21)

The stage worker and Pylon front support opt-in serial chunked prefill on
both transports. Each chunk has a physical step number and an absolute token
position. The generated-token index advances only when the last stage returns
an answer token. Intermediate chunks update the transformer, cache, and Engram
history; the CUDA head and sampler do not run. Cancellation keeps the group
busy until every stage stops, and a failed link fences the whole request.

The worker derives the chunk ceiling from every boundary's encoded message,
section, and token limits, reserving the worst compression phase for appended
rows. Stage 0 checks the complete rendered prompt plus the output allowance
before running the first chunk. Chunks and single-token decode steps share the
existing duplicate, ordering, request, and generation checks. Prefill timing
sums all chunks; decode counts exclude every prompt chunk.

`--max-context`, `--max-step-tokens`, `--prefill-chunk`, and
`--max-input-tokens` reach each worker through `pylon cloud up`. Workers announce
both capacities and the effective chunk limit; the head carries them in
`request_limits`, including on its beacon. Defaults retain the short launch
profile. To prepare a separate long-prompt development profile, add:

```text
--max-context 32768 --max-step-tokens 2048 --prefill-chunk 2048
--max-input-tokens 20480 --max-output-tokens 4096
--deadline-secs 1800 --step-secs 75
```

The 1,800-second deadline is a proposed measurement profile, not a W14 pass.
Do not advertise this as qualified until the original-weight checks below
pass. The 2,872-token launch and its 600-second deadline remain independent.

`MeasuredMemory::flash_sized` separates context-dependent cache storage from
step buffers. Psionic generates
`crates/pylon/placements/deepseek-v41-chunked-32k-memory.json` for 32,768 context
positions and 2,048 step tokens. Its four runtime estimates are approximately
1.576 GB each and fit the existing 11 GB runtime reserve in all three named
placements. These estimates derive from W3's provisional measurements; they
are not new device peak measurements. Pylon checks this envelope before cloud
allocation and refuses larger capacities. Regenerate it with
`PSIONIC_REGENERATE_CHUNK_MEMORY=1 cargo test -p psionic-v41-stage --test budget`
in the Psionic workspace. The test also checks each placement's reserved bytes.

`psionic-v41-stage qualify` accepts the same chunk and step flags. It drives
physical chunks in order, retains observations from every chunk and full logits
from output steps, and records requested schedule, announced limits, worker
bundle, and executable hashes. Logical oracle positions remain unchanged.
The thread-based `chain` measurement command also skips intermediate heads.
Batching and cross-stage overlap remain separate later experiments.

Local checks pass on the synthetic model: full 2,048-token and 19,989-token
prompts through four real worker processes; greedy and sampled answers against
the unsplit CPU reference with chunks 1, 3, 127, 128, and 129 on both transports;
cancellation and link loss during prefill; refusal before execution; codec and
memory bounds; and strict-policy qualification with three-token chunks. These
checks establish implementation behavior, not original-weight G1 acceptance.
GPU and original-weight evidence is recorded separately when available.


### The encrypted request path

The long profile also needs more than the stage wire changes. The router now
keeps the latest user message intact instead of cutting it to 6,144 bytes.
Pylon accepts up to 96 KiB of conversation text and at most 256 KiB of encoded
JSON, including repeated task text and escapes, under the existing extended
NIP-44 implementation. The head advertises `max_input_bytes` alongside its
token limits. Clients validate the body before sending or buying the job.
Oversized text fails visibly. A loopback relay test sends a prompt longer than
65,535 bytes through routing, encryption, provider admission, and an engine,
and checks its final question byte for byte.

For W14 measurements, `scripts/bench/pylon-models/run.py --v41-group` accepts
`--v41-cases-json FILE`. Each row contains `name`, the full `prompt`, and an
existing executable `score_case` (`extract`, `arithmetic`, `percent`, `grounding`,
or `injection`). The report retains prompt byte counts and hashes. Freeze
these long-prompt cases before a run; no prompt padding or clipping happens in
the harness. This measures complete answers under the proposed deadline; it
does not replace forced-token p3/p4 comparisons or the long holdout.

GPU checks on the RTX 4080 pass for the implementation in `b0f5af34b5`:
`v41-stage-build.sh test --host coderos-4080 --arch sm_89` runs the new
intermediate-head regression and the stage crate's tests with
`PSIONIC_REQUIRE_CUDA=1`. The head regression checks exact final logits and
token equality across chunks, no intermediate token, and no intermediate
output readback. Original-weight SM120 qualification and fresh placement peak
measurements still need capacity and the frozen G1 v2 policy. Five guarded
four-GPU spot attempts (us-west3-a, us-east1-b, us-central1-b, us-east4-c, and
us-west4-c) failed with stockouts; an on-demand us-west3-a attempt hit the
project's zero quota. No machine started and the guard recorded zero cost.


The final SM89 run (`41c07961ad`) also compares chunked CUDA qualification
observations against an unchunked synthetic oracle under the strict test
policy. All chunk files contribute router rows, and exact captured rows are
read at their absolute prompt position without loading the full observation
into memory. The stage/backend run passes 37 checks (one unrelated ignored
test). Two additional real Pylon group tests pass on CUDA for both transports:
chunked greedy/sampled parity, cancellation during prefill, and link-loss
recovery. They run with a required worker and CUDA backend, with zero skips.

[Retained results and raw GPU logs](verification/deepseek-v41-long-prefill-2026-10-21.json)
distinguish these development checks from the still-pending original-weight
p3/p4, long holdout, fresh device peaks, and larger W14 deadline. Those live
checks are listed in the master delegation document's After-close checks.

## Mixed-device groups (#12418, 2026-10-21)

A V4.1 group no longer has to run one GPU type. Each stage is checked
against its own entry, so a group can combine RTX PRO 6000 (`sm_120`), H100
(`sm_90`), A100 (`sm_80`), and RTX 4080 (`sm_89`) stages.

**No CPU stages in groups (owner, 2026-10-21, standing rule).** Groups run
on GPUs only, for evidence, demos, and defaults. The CPU port stays for unit
tests and reference diagnostics. `psionic-v41-stage plan` refuses a CPU
device unless `--diagnostic-cpu` marks the plan as a diagnostic.
Every such group is a **development group** until each device class is
qualified under G1 v2; the After-close checks list that work.

- **Per-stage identity.** A lane stage (`GroupStage.identity`, Pylon's
  `StageIdentity`) declares its device class (`cuda-sm_NN` or
  `cpu-ARCH`), and, when they differ from the group's, its runtime, bundle
  pin, and numerical profile. The front checks each stage against its own
  entry; only stages that declare nothing still have to match the first
  such stage (#12407). Every stage must announce the same stage-state
  dtypes (`wire`). A group is `pinned`, and so can be qualified, only when
  every stage has a bundle pin, its own or the group's (#12410). Psionic's
  manifest may name each stage's device (`V41StageEntry.device`), a
  stage refuses an entry planned for another class or expert placement,
  and Pylon's cloud plans carry the class into the lane. Manifests without
  devices keep their digests.
- **Hello and beacons.** A stage's hello announces `device` (class, GPU
  name, memory, and where its routed experts live) and `wire`. Each stage
  beacon carries what its stage announced in its inert `meta` as
  `stage_device`, and the field shows it per stage, so the Grid can show a
  mixed group.
- **Small GPUs.** `--experts host` keeps a CUDA stage's routed experts in
  page-locked host memory, which kernels read over PCIe through unified
  addressing (`CudaBackend::mapped_buffer`). A layer then needs about
  0.17 GB of device memory instead of 7.4 GB, so an RTX 4080 with 5.5 GB
  free holds four layers and the head. Tokens and logits equal device
  experts bit for bit (`host_experts_equal_device_experts`).
- **CPU stages, diagnostics only.** `--wire cuda` makes a CPU stage send
  and take BF16 cache rows like a CUDA stage, so a reference diagnostic
  can put one between CUDA stages. Groups don't use CPU stages.
- **Placement.** `plan_devices` (and `psionic-v41-stage plan`) places
  layers over GPUs of any architecture, GPUs with host experts, and CPU
  stages from W3's measured memory, counting host memory for Engram tables
  and host experts, and a class's own fixed cost per stage when measured.
  It balances memory only; give an explicit `--split` weighted by each
  class's measured time per layer.
- **Architectures.** `v41-stage-build.sh build --arch sm_90` and `sm_80`
  build bundles (an `sm_90` worker may also hold the Hopper kernels'
  `sm_90a`). `tests --arch sm_NN` packs the GPU tests for a machine
  without build tools. All four kernel suites pass on each architecture
  with the tolerances fixed before the SM89 runs: W4 matvec 9, W6 mix 15,
  W5 attention 14, and W7 stage 9 (including host experts), plus the stage
  worker's CUDA tests, on an A100-SXM4-40GB (`sm_80`), an H100 80GB HBM3
  (`sm_90`), and the RTX 4080 (`sm_89`). The G4 target's `sm_120` result is
  W8's.

**A mixed group answered V4.1 chat through Pylon** (development group,
unqualified; this first run used a CPU stage, which the rule above now
excludes, and the GPU-only rerun below replaces it as the evidence). Five stages on four device classes, each pinned to its own
bundle: layers 0–12 and 12–24 on the two RTX PRO 6000s of a spot
`g4-standard-96` (us-west3-a, the only US zone with G4 capacity that hour),
24–34 on a flex-start H100 (us-central1-c), 34–36 on a CPU stage on an
`m1-ultramem-40` (us-central1-c), and 36–40 with the head on the RTX 4080
at home, with its routed experts in host memory beside the production
services (about 3.7 GB of device memory, 28.9 GB pinned). The head ran on
the budget host. Three questions through `pylon ask` answered correctly
("…scattered by gas molecules…", "391", "Red, blue, yellow") at 1.07–1.15
tokens/s, 25.6–30.5 s to the first token. A decode token took 872–933 ms:
the stages 9.5, 9.7, 13.8, 569–577 (CPU), and 27.5 ms (RTX 4080), and the
links and head 243–300 ms, since the token crosses home and two regions
(head to G4 about 60 ms round trip, G4 to H100 20 ms, H100 to CPU 0.5 ms,
CPU to the RTX 4080 through ssh tunnels via the budget host). 362,705
bytes of stage state cross per token. Each stage beacon carried its
`stage_device`. Prefill on the CPU stage took 21–25 s of the first-token
time, so CPU stages suit a layer or two, not a share of the model. The run
cost $10.05. Receipt:
[`deepseek-v41-mixed-device-2026-10-21.json`](verification/deepseek-v41-mixed-device-2026-10-21.json).

**Metal.** [`v41-metal-plan.md`](v41-metal-plan.md) scopes a Metal stage
backend from what Psionic's Metal backend already has (MXFP4 expert
kernels, top-k, norms, RoPE, command reuse) and estimates 15–20 agent-days;
[#12420](https://github.com/OpenAgentsInc/openagents/issues/12420) tracks
it.

**GPU-only rerun** (development group, unqualified). Five GPU stages, each
pinned to its own bundle, all cloud stages in one zone (asia-southeast1-c,
the only zone with G4 spot capacity in the sweep): layers 0–12 and 12–24
on two RTX PRO 6000s, 24–34 on an H100, 34–38 on an A100 40GB, and 38–40
with the head on the RTX 4080 at home with `--experts host`. The split
came from `psionic-v41-stage plan --split`, weighted by the measured time
per layer (RTX PRO 6000 0.80 ms, H100 1.36 ms, A100 2.68 ms, RTX 4080 with
host experts about 7 ms), so no stage dominates: 9.5, 9.8, 13.6, 10.7, and
14.9 ms a token, 58.6 ms in all. The same three questions answered
correctly at 2.50–2.91 tokens/s, 4.1–4.9 s to the first token, against
1.07–1.15 tokens/s with the CPU stage and 29.1–29.7 tokens/s for the
all-RTX PRO 6000 groups (33.9 ms a token). Of the 309–367 ms gap a token,
about 92% is network distance: the links and head took 285–342 ms against
0.87 ms, because each token crosses the Pacific twice (the head at home to
stage 0, and the A100 to the home stage, about 225 ms round trip) plus
Wi-Fi to the 4080. The RTX 4080 with host experts adds about 13 ms (4%),
and the H100 and A100 about 13 ms more than RTX PRO 6000s would. With the
4080's layers and the head in the zone, the same cloud stages would decode
at about 21 tokens/s. The run cost $6.28. Receipt:
[`deepseek-v41-gpu-mixed-2026-10-21.json`](verification/deepseek-v41-gpu-mixed-2026-10-21.json).

### Gate repairs and original-input calibration, 2026-10-22 17:00 UTC

The current gate work remains open under #12369. The repaired SM120 package
passes **121 checks, with no failures and six ignored measurement checks**,
including the CUDA qualifier's positive case and all four negative controls.
The synthetic oracle now includes full index observations; a stale-cache
control saves a failed report when its indices are invalid. The
[component receipt](verification/deepseek-v41-gates-2026-10-22/sm120-index-components.json)
names the exact diagnostic source, archive, and retained log.

`isolate` has collected all four native CUDA stages on original oracle
boundary inputs for C4 and the 1,025-token window case. These are calibration
measurements, not G4 passes. A separate plain BF16 CPU collection is running
on the same inputs. `isolated-check` verifies saved output hashes, complete
boundary shapes, original input-message hashes, and decode/replay coverage
before recomputing numerical differences. Complete index records also retain
raw substitutions, margins, and regrets so a later frozen policy can rejudge
them without trusting an earlier verdict. Index selections and candidate
blocks use separate changed-decision fractions.

Rule 2.4's derivation is implemented and locally tested, **not frozen**. It
accepts uncontrolled plain BF16 CPU references, requires matching recorded
index inputs and bundles, and checks all four numerical controls. It derives
no threshold from a CUDA candidate. Historical policies and failed reports
remain unchanged. Release qualification additionally requires an exact frozen
corpus, all its cases, hashed oracle run metadata, and the actual worker
bundle, numerical profile, manifest, and execution configuration.

Pylon now separates numerical qualification from the legacy execution label
`qualified`. The head publishes a numerical claim only when every ready stage
matches the evidence's bundle, profile, manifest, configuration, and limits.
An explicit request for qualified service requires that claim. Legacy default
routing remains available for previews. These changes have local routing,
lifecycle, cloud-launch, and website-consumer checks; they do not qualify the
running preview or pass G1, G2, G4, or G8 for a new worker bundle.

The follow-up implements `isolated-policy` and `qualification-receipt`. Stage
limits come only from rechecked plain BF16 CPU outputs, with short and
near-capacity calibration coverage for every stage. Maximum absolute error,
relative L2 error, and index decision limits must each pass; one cannot offset
another. Publication rejudges G1 and G4 measurements, checks the G2 quality
floor and completed answers, verifies frozen checker/suite/answer hashes, and
requires the same worker configuration across the gates. These commands do
not turn incomplete or failed reports into a qualification claim.

The explicit `fp8-mma-tp8` resident CUDA mode can now run without recording,
so the eventual qualified service can use the measured arithmetic. It remains
opt-in, with its own identity; the default arithmetic and running preview are
unchanged. Release checks require complete index acceptance and the frozen
release scope. The prospective
[2.4 confirmation inputs](verification/deepseek-v41-gates-2026-10-22/confirmation-corpus-2.4.json)
are prepared, but no 2.4 policy or confirmation result is frozen yet. Every
case requests all 32 generated rows in both modes; the largest fixed replay
fits the 2,872-token step. A storage HTTP 503 interrupted calibration after
seven saved cases; bounded upload retries and resumption preserve those
records. The [retry controls](verification/deepseek-v41-gates-2026-10-22/collection-retry-controls.json)
cover transient success and persistent failure.


The rebuilt `06e5158778` SM120 package passes **144 checks, with no failures
and six ignored optional checks** on the dedicated RTX PRO 6000. The
[receipt](verification/deepseek-v41-gates-2026-10-22/sm120-receipt-components.json)
binds its source, worker, test archive, and retained log. This is component
evidence; full-model qualification remains open.

Independent review found that the G4 policy command accepted edited recheck
summaries with missing boundary sections, shortened continuations, or missing
stage identities. Both `isolated-policy` and `qualification-receipt` now
recompute G4 measurements from each original `report.json`, its saved binary
outputs, and the original oracle input messages. They require the complete
stage-specific shapes, one valid matching oracle hash per stage, and the
frozen weight identity. The frozen corpus and hashed oracle metadata determine
prompt and continuation lengths independently of the candidate report. Local
negative controls cover each omission and a freshly hashed oracle with a
shortened continuation. The policy remains unfrozen; no limits were changed.

`isolated-policy` takes `--weights DIR --oracle DIR` and raw report paths in
`--reference` or `--candidate`; derivation additionally takes `--manifest`,
and confirmation takes `--corpus`. `qualification-receipt` also requires
`--weights DIR --oracle DIR --corpus FILE --g4-records PATHS` and refuses a
G4 summary that differs from the rechecked output bytes.

GCP preempted the long connected CPU reference at 17:32 UTC before a complete
case was saved. A bounded on-demand replacement is collecting that case;
separate CPU machines continue the original-input stage references. An
interrupted run supplies no passing evidence.


For distributed qualification, `qualify --front-host IP --worker PATH` binds
the front to the head's reachable private address and starts the pinned
workers through a placement-specific launcher. Remote workers write to the
same private shared recording directory. The report retains the actual
manifest and stage configurations. A new shutdown check closes every control
connection, including reader clones, and waits for every worker launcher to
exit before saving a passing result. Startup failures also close accepted
connections and reap local launchers. A local subprocess test confirms that a
worker behind a launcher receives EOF and exits before its launcher is reaped;
the full CPU qualifier and greedy-answer tests still pass. Real distributed
runs remain required for G6.


A second independent review reproduced a connected G1 coverage gap in the
publication checker. Release and publication now require the frozen prompt
and continuation lengths and the exact unique labels `last`, `g1`, …, `gN`
in each mode, including the logits after the final forced token. For the
prospective 16-case, 32-token corpus, that is 1,056 distribution comparisons.
Every router layer must cover all `L + N` rows, with consistent totals across
all 40 layers. Malformed metrics fail instead of disappearing during parsing.
Oracle preflight checks each label's absolute row, both modes' required logit
tensors, and the post-final reference token. Declared oracle rows that are
missing during collection are reported as missing. These checks change no
numerical limits; the policy remains unfrozen.

The pinned `06e5158778` worker also completed a private two-machine C4 run
through the qualification front, with all workers confirmed stopped and the
unchanged 2.3 diagnostic rule passing. Its actual configuration is
`6f85bdd612b7cc3cabfd4974b1d119201c6eae54950c0d5fcdb15eb7881c7f7b`.
The native Pylon head then served `17 × 23` as `391`, with finish `stop`, in
924 ms through the existing private qualification relay and authenticated
direct stage links. This is preflight evidence, not final G6 or release
acceptance. The private head's legacy `qualified` profile label does not grant
numerical qualification. Pinned crash and freeze drills are in progress.

### Router publication recheck (2026-10-22)

The independent re-review found that receipt checking could accept explicit
failed or unresolved router decisions, or malformed numeric fields. The checker
now requires valid counts, measured score errors, and consistent mismatch
fractions. Every substituted row needs its own unique change record. Existing
margin, regret, substitution, and fraction limits apply again at publication;
within-margin swaps remain allowed. The stage package passes 61 tests, with
one optional test ignored. The worker bundle and numerical limits do not change.
See [the checker evidence](verification/deepseek-v41-gates-2026-10-22/router-publication-fix.json).

The complete-answer acceptance rules were committed in `3244d4f7b0` at
2026-10-22 19:12:12 UTC, before the candidate run. The `frozen_at` value of
19:20 in `g2-quality-freeze.json` is a timestamp transcription error; the
commit time records the actual freeze. The frozen rules and file remain intact.

### Current candidate answer results and calibration rejudging (2026-10-22)

The pinned original-weight `06e5158778` worker passed G2 on one quad:
**5/6 runtime tasks, 5/6 oracle tasks, all mandatory checks held**. Every
answer reached EOS within 4096 tokens. The shared `fib(186)` failure remains
reported. The suite and scorer were fixed before this candidate run;
[the result](verification/deepseek-v41-gates-2026-10-22/g2-candidate-06-quad.json)
binds the actual manifest, worker bundle, and execution configuration. Other
placements must repeat it under their own configuration.

Independent review confirms the publication checker repair and valid
within-margin swap controls; see
[the review record](verification/deepseek-v41-gates-2026-10-22/router-independent-review.json).
This is checker evidence, not numerical model acceptance.

CPU calibration reports recorded before margins were set have verdict counters
from that earlier policy. Calibration self-check now rejudges their retained
score differences and every changed row under the newly derived rule, instead
of copying the earlier verdict. A regression test rejects a changed row whose
regret exceeds the derived rule. Publication still requires zero failed and
unresolved decisions under the frozen release policy. All 33 stage unit tests
pass, with one optional test ignored. The inference worker and limits stay
unchanged.

### Current CUDA and cross-machine evidence (2026-10-22)

The pinned `06e5158778` source now passes **144 component tests on each of
SM89 and SM120**, with no failures and six optional tests ignored on each.
The SM89 run includes the B200 FP8 row and 37 independent CPU mixed-row
controls. See [the SM89 result](verification/deepseek-v41-gates-2026-10-22/sm89-receipt-components.json).

The same pinned worker passed the frozen G2 suite on two dual-GPU machines:
**5/6 versus the oracle's 5/6, all mandatory checks held, every answer ended**.
The token sequences match the one-quad run. See
[the two-dual result](verification/deepseek-v41-gates-2026-10-22/g2-candidate-06-two-dual.json).

A guarded `pylon cloud up four-single` cold launch also completed its private
Pylon runtime checks: arithmetic, repeated-request state, continuation from the
actual previous answer, an honest 4096-token refusal, and a fresh answer after
that refusal. The capped answer ran for 197 seconds. All four signed stage
beacons report the same original-weight bundle and actual execution
fingerprint `07649fd2bcb8580c8573ecd0f64332b4f1a70828fc095c0dd3dfb8d161fb7c6e`;
[the signed observations](verification/deepseek-v41-gates-2026-10-22/four-single-stage-beacons.json)
retain their identities. The read-only `qualification_beacons` example uses
the existing verified relay reader and a temporary identity. It includes
stage beacons, which ordinary routing excludes. All 328 Pylon tests pass.
These are runtime and identity checks; four-single numerical acceptance is
still pending.

Prospective calibration now discards the recording policy's verdict counters
for negative controls too, then rejudges their measured errors and changed
rows. A pending verdict alone cannot detect a control. Missing local selection
counts are classified as malformed evidence. The stage package passes 62
tests, with one optional test ignored. The current diagnostic correctly
refuses the historical stale-cache control for insufficient numerical evidence;
new controls on the pinned candidate are required before the final freeze.
The model worker, thresholds, and publication requirements do not change.

### Native recovery matrix and machine loss (2026-10-22)

The pinned original-weight worker passed private two-dual recovery tests for
all four stage positions, with both `SIGKILL` and `SIGSTOP`. Each interrupted
request failed visibly, the group recovered under a new generation, and a
fresh request returned `391`. The deadline test also passed: the concurrent
request stayed blocked until all four stages stopped, then a fresh request
succeeded. Earlier harness failures remain retained separately. The native
controller shut down with status 0, and both machines had no worker process
before deletion. See [the recovery matrix](verification/deepseek-v41-gates-2026-10-22/two-dual-recovery.json).

The four-single cloud test also passed a whole-machine loss drill during an
active request. The buyer received a failure, late stage messages were
refused, and the bounded restart attempts ended with all four allocations
deleted. See [the runtime and loss record](verification/deepseek-v41-gates-2026-10-22/four-single-runtime-and-loss.json).
This establishes recovery behavior; G1 and G4 numerical acceptance remain
pending.

The owner's two Maya consumer-q Pylons for Foreman's #12500 work are separate
services with their own artifact and provider identities. They retain their
lossy profile. This original-weight qualification run does not grant those
services a numerical qualification claim. The quota-retrying private quad
watcher was stopped to give the Maya services priority; the completed
two-dual recovery placement has been deleted.

### Isolated index padding correction (2026-10-22)

Rechecking the retained CPU outputs exposed a reader mismatch: the native wire
uses `SELECTION_PAD` (`i32::MIN`) for unused index slots, while the oracle uses
`-1`. The isolated checker now accepts either supported representation for a
candidate's unused slots. Other negative IDs, repeated IDs, and IDs beyond the
current causal range still fail. The regression covers both representations
and those refusals. All 63 stage package tests pass, with one optional test
ignored. The pinned model worker and numerical limits stay unchanged.

The new stale-cache attempts remain retained as structural refusals: the
all-stage attempt exported shifted KV rows, and the head-only attempt rejected
incoming rows at the old request's offset. The control adapter now maps incoming
row positions into its deliberately retained cache, without changing their
values. A focused test checks both compression ratios, unchanged row values,
misalignment, and overflow. A separate control build must demonstrate numerical
detection before the policy can freeze; these earlier refusals do not count.
The uncontrolled `06e5158778` candidate remains pinned.

The `qualification_prompts` example generates the original-weight launch's
2,872-token boundary and 2,873-token overflow request with the actual
`Vocabulary::V41` chat template. Running it against the checkpoint tokenizer
produces exact counts and records tokenizer, prompt, and token-ID hashes.
The private one-quad Pylon run uses those saved requests; its live results
remain separate from this construction check.

### Current one-quad runtime and raw rechecks (2026-10-22)

The private original-weight one-quad group passed complete and repeated answers,
continuation from its actual answer, an honest 4096-token refusal, and fresh
requests after refusals. The exact 2,872-token request completed as `OK` in
127.3 seconds; the 2,873-token request was refused in 1.18 seconds. The capped
workload decoded at 21.12 tokens/s. Observed memory after these checks and
during an active recovery request was 73,319, 72,029, 71,687, and 72,939 MiB
across the four GPUs. These are sampled observations, not lifetime allocation
peaks. See [the runtime record](verification/deepseek-v41-gates-2026-10-22/one-quad-runtime.json).

The corrected reader also rechecked all seven available CPU records and all
four saved one-quad GPU stage records against their original input messages
and output bytes. See [the raw recheck summary](verification/deepseek-v41-gates-2026-10-22/g4-raw-rechecks.json).
The long stage-2 CPU reference is still pending. These rechecks establish the
recording's structure and measurements; the numerical release policy remains
unfrozen.

The prompt helper also supports `TOKENIZER_DIR --count MESSAGES_JSON` to count
retained chat requests with the serving template, without running inference.
Its original boundary generation remains byte-for-byte reproducible; malformed
arguments return an error. Raw private request text stays outside the repository.

### Three-placement answer quality and VM replacement (2026-10-22)

The frozen G2 suite now passes on four single-GPU machines too: 5/6 runtime
and 5/6 oracle tasks, all mandatory floors held, with every answer ending
within 4096 tokens. All six token sequences, reasoning, and final text match
the one-quad and two-dual runs. The shared `fib(186)` failure remains visible.
See [the four-single result](verification/deepseek-v41-gates-2026-10-22/g2-candidate-06-four.json).

The bounded private one-quad watcher also replaced a lost VM automatically.
The old allocation stopped before the new allocation started; the replacement
used a new provider identity and returned a fresh `391` with a normal stop.
The interrupted request returned no answer after its 600-second budget and
10-second relay allowance, as the current NIP-PYLON contract specifies. Both
allocations were deleted when the test ended, with estimated compute cost
$4.45. See [the replacement record](verification/deepseek-v41-gates-2026-10-22/one-quad-watcher-recovery.json).
Numerical release acceptance remains pending.

### Current two-dual runtime checks (2026-10-22)

The replacement private two-dual group passed the same eight runtime checks:
complete and repeated answers, actual conversation history, the 4096-token
output refusal, the exact 2,872-token input, the 2,873-token refusal, and fresh
answers after both refusals. The accepted boundary request completed in
128.9 seconds. The native controller exited successfully, and all four workers
stopped before subsequent qualification work. See [the runtime record](verification/deepseek-v41-gates-2026-10-22/two-dual-runtime.json).

The corrected head-only stale-cache control produced full measurements, but
the short Engram case still fails only on noncausal index data under the
provisional CPU-derived policy. That does not establish numerical detection.
Natural-language calibration controls are running separately; the earlier
attempts remain retained. Rule 2.4 remains unfrozen, and no fresh confirmation
oracle has been captured.

The control harness now accepts `--control-warm-case ID` to test leakage from
an unrelated saved request. It records the warm-up case, token count, token-ID
hash, and whether the warm-up differs from the measured prompt. A missing or
empty warm-up fails before workers start. The option is restricted to forced
stale-cache controls and cannot run as answer collection or release
qualification. The parser regression and stage package checks pass. Existing
same-prompt attempts remain retained; the candidate worker and acceptance
limits stay unchanged.

### Provisional fault detection and CPU repeat (2026-10-22)

The short CPU-only calibration now accepts its independent references and
numerically rejects wrong weights, routing, RoPE, and stale cache state. The
natural-language stale-cache case has 79 numerical violations, separately from
its malformed index evidence. The short Engram attempt still lacks numerical
detection and remains retained. See [the provisional self-check](verification/deepseek-v41-gates-2026-10-22/provisional-control-self-check.json).
This policy is not frozen: the long references, short conversation/code CPU
cases, and isolated-stage envelopes must finish, and all controls must still
fail under the resulting limits before fresh confirmation starts.

The repeated C4 collection on C3D and N2D also matches all 1,836 saved tensor
files byte for byte. See [the CPU repeat comparison](verification/deepseek-v41-gates-2026-10-22/cpu3-repeat-comparison.json).
The repeat checks consistency; it does not count as another calibration case.

### Complete runtime checks and calibration coverage (2026-10-22)

Four-single now passes the same eight native Pylon runtime checks as one-quad
and two-dual. Its exact 2,872-token request completed in 128.8 seconds; the
2,873-token request was refused, and the next request answered normally.
All four workers stopped. The earlier missing test-launcher attempt is retained
as a failed setup attempt. See [the four-single runtime record](verification/deepseek-v41-gates-2026-10-22/four-single-runtime.json).
The two cross-machine capped workloads measured 20.16 and 20.14 decode tokens/s;
their recorded link-and-head overhead is aggregate, not a separate per-hop
latency measurement.

The unrelated-request cache control also fails numerically, with 80 numerical
violations under the provisional short-reference policy. Replacing the fault
report leaves every derived threshold identical. Its [record](verification/deepseek-v41-gates-2026-10-22/contrasting-cache-control.json)
fixes the required fault cases before the remaining CPU calibration completes;
every required fault must still fail the final CPU-only policy.

All four stages now have complete CPU raw rechecks for the short Engram case,
the 1,025-token boundary, and the 2,726-token long case. The redundant long-stage
backup was cancelled after the first complete result uploaded and rechecked;
its partial result is excluded. See [the complete CPU coverage](verification/deepseek-v41-gates-2026-10-22/g4-complete-cpu-rechecks.json).

The `qualification_answers` example runs the unchanged frozen G2 suite through
Pylon's direct `StageGroup` pipeline, including each case's prompt mode, native
token IDs, and parsed reasoning. `v41-stage-build.sh group-answers` packages
that separate tool. These native paired runs remain pending. The passive
`observe_vm.py` sampler is ready for active GPU-memory and per-link TCP RTT
observations; its [wiring checks](verification/deepseek-v41-gates-2026-10-22/observer-wiring-checks.json)
use an idle GPU VM and existing CPU sockets, so they do not establish an active
memory peak or GPU-link latency.

The [additional CPU references](verification/deepseek-v41-gates-2026-10-22/additional-cpu-calibration.json)
now retain three natural conversation/code cases with 32 generated positions
and the 1,025-token boundary case with eight. Both decode and replay are
complete for each case. Rechecking the hashed recordings finds no causal
index inconsistency. These are independent plain-BF16 CPU calibration
measurements; their old-policy failures remain visible. The connected
2,726-token CPU reference still needs to finish before rule 2.4 can freeze.
