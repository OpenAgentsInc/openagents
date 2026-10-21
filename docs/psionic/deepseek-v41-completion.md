# Completing #12369: native DeepSeek-V4.1-Flash on stage Pylons

Status: the definitive checklist for
[#12369](https://github.com/OpenAgentsInc/openagents/issues/12369), written
2026-10-21. It replaces the scattered "what's left" sections of the
[native V4.1 plan](deepseek-v41-native.md), its review addendum, and the
[issue roadmap](../roadmap/2026-10-20-enter-the-verse-issues.md) as the one
place that says what finishes the epic. Those docs still hold the reasoning
and the evidence. Update this file as items land.

## What "done" means

The epic's goal: **serve DeepSeek-V4.1-Flash with Psionic as the only
inference engine, fanned out across several Pylons (one GPU per stage), so it
shows up in the Grid as a group of Pylons.** No llama.cpp, DeepInfer, or
PyTorch in the serving path. DeepSeek's reference code is a test oracle only.

The epic closes when all eight gates below pass, each with retained
evidence in the repository or the oracle bucket. A gate is about evidence,
not issue state: closing a child issue doesn't pass a gate whose check is
listed as an after-close check.

| Gate | Passes when | Evidence |
| --- | --- | --- |
| **G1. Reference parity on CPU** | The Rust CPU model, on the original weights, matches the oracle: decode against the reference's decode and replay against its replay, at every recorded position of the four main prompts and the edge cases; router ids match except at recorded exact ties; full logits within tolerances set before the comparison runs. | A parity report committed under `testdata/deepseek_v41/` and summarized in the plan |
| **G2. Complete answers on CPU** | Greedy decoding on the original weights reproduces the reference's complete answers for prompts 1 and 2 (122 and 268 tokens), with reasoning and final text split correctly by the #12372 adapter. | The same report |
| **G3. Kernel parity on GPU** | Each CUDA kernel matches the CPU reference on real tensors, on the RTX 4080 (SM89) and on the target GPU. | Per-kernel test results for both GPUs |
| **G4. One stage on one GPU** | A stage process holding a contiguous layer range runs device-resident (no per-token host stepping) and matches G1 for its layers given the oracle's stage inputs. | Stage parity results, memory peak, per-token latency |
| **G5. A full group on one machine** | Stage Pylons, one per GPU, run the whole model over loopback and pass G1's decode checks, G2's complete answers, and DS0's six runtime checks, with speed and per-GPU memory recorded. | Group report from a cloud run through the spend guard |
| **G6. A group across machines** | The same group passes the same checks across separate machines in two genuinely different placements, for example four single-GPU machines and two two-GPU machines. Per-hop latency and stage-state bytes per token are recorded. | Group reports for both placements |
| **G7. Lifecycle and recovery** | A group is advertised only when every stage is loaded and checked. Killing or freezing any stage fails the request visibly, never replays it after tokens reached the caller, and the group recovers under a new generation. A restarted stage can't serve a stale request. Caller cancellation keeps the group busy until work stops. | Fault-injection results on a real group |
| **G8. Visible as a Pylon group** | One launch command starts a cost-capped, expiring group and publishes it. The router sends a V4.1 request to it through Pylon, and the answer comes back complete within the group's declared deadline. The `/home` Pylon layer shows the group's Pylons. | A Pylon transcript, the router log, and a capture of `/home` |

Gate progress (2026-10-21):

- **G1, original-weight CPU parity (W1, #12388): failed.** The fixed same-mode
  policy replaces the earlier cross-mode tolerance rule. Both complete chat
  comparisons reach all 40 layers and the full vocabulary head, but logits and
  router sets differ. The first numerical limit failure is the last prompt
  token's layer-1 attention pre-mix; the 2k and 20k cases also show layer-0 router
  differences. [#12404](https://github.com/OpenAgentsInc/openagents/issues/12404)
  owns the remaining divergence and missing observations. W2 and W3 stay
  blocked on it. Isolated component and CUDA-kernel checks do not pass G1.
- **G5–G7, stage contract (W9, #12396).** One message type carries the
  manifest digest, group generation, request, attempt, stage and layers,
  step, position, tokens, and typed sections. A stage refuses duplicate,
  stale, missing, reordered, cross-request, malformed, and oversize
  messages before it consumes any state. With a synthetic executor that
  keeps V4.1's 40-layer sharing layout, four-stage, eight-stage, uneven, and
  one-layer-per-stage splits match the unsplit run bit for bit.
- **G6, stage-state bytes per decode token,** from encoded messages of the
  real shapes (BF16 residual and cache rows, a 36-byte request ID): 44,300
  bytes at a boundary of 10 (43,788 on average, since layer 8's KV adds a
  row every other token), 41,124 at 20 (the residual only), and 52,772 at
  30. The four-stage forward path is 138,196 bytes per token; eight stages
  are 332,300. Returning full F32 logits to the front adds 517,280 bytes
  per token, more than every forward hop together, so W10 (#12397) decided
  that the last stage samples with the front's settings and returns the
  token ID, 4 bytes. The synthetic geometry's numbers are in the W9 test
  output.
- **G3, block-scaled matrix-vector kernels (W4, #12391), RTX 4080 only.**
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
  6000 (SM120) half of G3 is W8. Results:
  [`sm89-2026-10-20.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_matvec/sm89-2026-10-20.json).
- **G3, mixing, Engram, and routing kernels (W6, #12393), RTX 4080 only.**
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
  cold launch (W16, #12403), which needs W11's stage worker in a bucket,
  W12's stage-to-stage transport, and W14's deadline.
- **Placement from header bytes.** Layers 0–9, 10–19, 20–29, and 30–39
  need 75.41, 74.07, 73.92, and 75.23 GB of GPU weights, as the plan's
  table says, and the Engram tables (101.38 GB each) go to host memory on
  the stages that run layers 1 and 14. Four stages are tried before eight,
  and a plan that fits neither is refused. The runtime reserve is a fixed
  input until W3 measures it.

Out of scope for the epic, tracked in the [Enter the Verse
roadmap](../roadmap/2026-10-20-enter-the-verse.md): streamed tokens and the
public flag, longer job inputs, the Grid's group visuals beyond the existing
Pylon layer, agent tool use on V4.1, batching, expert parallelism, DSpark
speculation, vision, and paid jobs.

## Where things stand

### Done

| Area | Issues | What it gives the epic |
| --- | --- | --- |
| Spend guard | #12370, #12376 | `scripts/dev/cloud-budget.py`: $3,000 cap, expiry, one budget host, reserve, refusal on a damaged ledger. Billing alerts at $1,500 and $2,500. Spent so far: $36.85 compute plus a $200 reserve. |
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

### Phase 2 finish: the CPU model against the real model (G1, G2)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W1 | **Original-weight CPU parity report.** Compared original weights with the frozen policy. Complete chat diagnostics and first-failure runs report every case, mode, and recorded position, including untested positions. G1 fails; #12404 tracks the divergence and missing observations. | M, cloud | #12385–#12387 | Report retained; closing W1 does not pass G1. |
| W2 | **Complete answers on CPU.** Greedy to EOS for prompts 1 and 2 with the #12372 parser. | S, a later cost-capped run | W1, #12404 | G2 passes; wall time per token is recorded. |
| W3 | **Measure the model.** Peak memory per layer range, KV and indexer bytes per token at each compression ratio, Engram lookup cost from RAM, and active parameters per token. | S, a later cost-capped run | W1, #12404 | Numbers replace the estimates in the plan; placement inputs for W9 exist. |

### Phase 3: CUDA (G3, G4)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W4 | **Done on SM89 (#12391).** `psionic_backend_cuda::v41_matvec`: `v41_act_quant_fp8` and `v41_block_matvec` over packed FP8 32×32-block and FP4 per-32 weights, with device-selected matrices from a stack for routed experts and no host round trip. Parity on the RTX 4080 against the CPU path on committed real rows; decode reads 409–591 GB/s (57–82% of the 4080's 717 GB/s). SM120 parity moves to W8. | M | — (checked against the CPU path, which is bit-exact against the reference's dequantization) | Parity with the CPU path on real rows on the 4080 and the target GPU. |
| W5 | **Attention kernels.** Window attention with sink, compressed attention at ratios 1 and 2, the indexer, candidate selection, and the shared-source reads. | L (split per mode) | W1 | Per-kernel parity on both GPUs, including the 127/128/129 cases. |
| W6 | **Done on SM89 (#12393).** `psionic_backend_cuda::v41_mix`: hyper-connection mixes (joint RMS projection, pre/post, 20-iteration Sinkhorn), collapse, and expand; the Engram gather from page-locked host memory and the Engram gate; router logits, routing with ties to the lower id written as `V41Slots` ids, clamped SwiGLU, and the expert sum. Parity with the CPU path on real weights and oracle inputs on the RTX 4080; one decode step's kernels take 1.8–12.7 µs each. SM120 parity moves to W8. | M | — (checked against the CPU components, as W4) | Per-kernel parity on both GPUs. |
| W7 | **Device-resident step.** One stage's whole decode step on the GPU, with embeddings, norms, rotation, and sampling on the device, and CUDA graphs for decode. | L | W4–W6 | G4 passes for a 5- or 10-layer stage on the target GPU; per-token latency and peak memory are recorded. |
| W8 | **Target GPU choice.** Confirm the kernels on the RTX PRO 6000 (SM120, G4 machines) and decide whether B200 (SM100) needs a separate profile. W4's SM120 run: build with `PSI_CUDA_ARCH=sm_120` and run `PSIONIC_REQUIRE_CUDA=1 cargo test -p psionic-backend-cuda --lib v41_matvec`, then the ignored `v41_decode_bandwidth`; W6's SM120 run: `PSIONIC_REQUIRE_CUDA=1 cargo test -p psionic-backend-cuda --lib v41_mix`, then the ignored `v41_mix_decode_latency`. | S, cloud | W7 | A recorded decision with parity on the chosen GPU. |

### Phase 4: stage Pylons (G5, G6, G7)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W9 | **Done (#12396).** Stage-state codec and group manifest: `psionic_models::stage_state` (message, codec, `StageInbox` admission, `StageWorker`) and `psionic_models::deepseek_v41_stage` (V4.1 boundary contracts, wire sizes, `V41GroupManifest`, `plan_group`); `psionic_serve::pipeline_stage` adapts the Gemma 4 step to the same message. Placement reads header-derived bytes through the `PlacementMemory` trait until W3's measured input implements it. | M | W3 for measured placement only | Codec fault tests (duplicate, stale, missing, reordered, cross-request, malformed, oversize) pass with a synthetic 40-layer executor; every split matches the unsplit run bit for bit. |
| W10 | **Group lifecycle.** Done (#12397) on synthetic stages: `pylon::stage_group` runs each stage as a managed worker under the serial supervisor's process-group owner and restart budget. A group is ready only when every stage agrees on weights, runtime, protocol, and generation and passes a check. A replacement bumps the generation and fences the survivors; failures and cancellation hold admission until every stage stops or is reaped. The front's control identity is W9's (generation, request ID, attempt, stage, step). `StageGroup` implements W13's `GroupReadiness`. The last stage samples with the front's settings and returns token IDs, not logits. `crates/pylon/tests/group_lifecycle.rs` covers G7's fault list; G7 still needs it on a real group (W11, W12). | L | W9, #12364 | G7's fault checks pass on a local or synthetic group. |
| W11 | **A full group on one machine.** All stages on one 4- or 8-GPU machine over loopback. | M, cloud (G4 spot, about 20 h) | W7, W10 | G5 passes. |
| W12 | **A group across machines.** Two placements across separate machines in one zone. | M, cloud | W11 | G6 passes; G7's fault checks repeat on the cross-machine group. |

### Phase 5: launch and visibility (G8)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W13 | **Group beacon and routing.** Done (#12400): the group publishes one routable beacon (one serial slot, the exact model artifact, its declared deadline and output limit); stage beacons are visible but not routable. The router matches requested model and group readiness. W10 feeds readiness through `pylon::group::GroupReadiness`, and W15 publishes the beacons. | M | W10 | Router tests pick the group only for V4.1 requests and only when it's ready. |
| W14 | **Deadline and answer profile.** Qualify a V4.1 reasoning profile (output cap, deadline) that finishes complete answers within the deadline the beacon declares. Changing Pylon's 90-second default is a versioned transport change with old-client refusal. | M | W11, W13 | Complete answers for the qualification prompts through Pylon within the declared deadline. |
| W15 | **Built (#12402) on synthetic stages; the cold launch is an after-close check that W16 runs.** `pylon cloud up four-single\|two-dual` plans the machines from Psionic's manifest file, creates each through `scripts/dev/cloud-budget.py` with an expiry and labels, heads the group from the allocating host in group mode (`pylon::group_serve`: W10's `StageGroup`, one W13 head beacon, a beacon per stage, NIP-CJ jobs driven through the stages), and ends it on `cloud down`, expiry, the budget's cap, or a lost machine, deleting every machine through the guard. Stages run over `gcloud compute ssh` with the control connection tunneled back; `scripts/pylon/v41-stage-startup.sh` copies a machine's shards. `tests/group_cloud.rs` covers launch to answer to `down`, dry-run plans, guard refusals, spot loss, expiry, and the cap. | M | W12, W13 | A cold launch to a first answer, then a clean stop, with costs in the ledger. |
| W16 | **Show it.** Run the launched group with the `/home` Pylon layer open and record the transcript, router log, and capture. | S, cloud | W15 | G8's evidence is committed and the epic can close. |


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
| W1–W3 CPU parity and measurements | W1 used $12.46 in compute. Re-estimate the remaining runs after #12404; the earlier under-$60 estimate is unverified for complete long-prompt runs. |
| W8 target GPU check | under $50 |
| W11 one-machine group development and runs | about $300 (a `g4-standard-384` at $13.31/h spot, about 20 h) |
| W12 cross-machine placements | about $200 |
| W15–W16 launch and recording runs | about $150 |
| Expected total | Original estimate: about $1,000. CPU follow-up needs a revised estimate; the spend guard's $3,000 cap still applies. |

## Who can do what in parallel

- **Codex (DeepSeek chat)** holds the CPU work: #12385–#12388, then #12404
  before W2 and W3 can qualify the model.
- **Stage work:** W9 (#12396), W10 (#12397), and W13 (#12400) are closed
  with synthetic or fixture evidence. Their real-model integration remains
  in W11, W12, and W15, with the blockers in the work table above.
- **CUDA (W5–W7)** can use identical-input component checks while G1 is
  unresolved. Each result must name its CPU/oracle inputs and numerical
  bounds. W4 and W6 have such isolated evidence; connected stages still need
  G1 through #12404. W1's closure cannot establish that the CPU model matches
  the original-weight oracle.

## Open questions

- Tolerances for G1: the [fixed CPU policy](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/tolerance-policy.json)
  uses per-component arithmetic limits. Same-mode repeatability is currently
  unmeasured; the cross-mode gap grants no allowance. Do not loosen bounds
  after a failure.
- Whether the RTX PRO 6000 (SM120) runs the kernels at useful speed, or the
  group needs B200 machines.
- Host RAM per stage for the Engram tables (about 101 GB each, layers 1 and
  14) on single-GPU machines.
- Redistribution terms of the checkpoint's license for stage packages
  derived from it, before any stage package leaves our buckets.
- The deadline a V4.1 reasoning answer actually needs through Pylon, which
  sets W14.
