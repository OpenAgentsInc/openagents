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
| Group beacon and routing | #12400 | W13: NIP-PYLON v2 group head and stage beacons (one serial slot, exact artifact, deadline, output limit, readiness, generation) with v1 fixtures in both directions; `pylon::group` readiness source for W10; the router and `client::choose_model` route a request to a group only when it names that model and the group is ready, never to a stage, never substituting. Fixtures only; nothing publishes a group yet. |

### CPU assembly completed (Codex, DeepSeek chat)

| Issue | Scope |
| --- | --- |
| #12385 | Full block wiring: hyper-connection handoff, norms, experts, Engram, with float32 and BF16-reference arithmetic |
| #12386 | The full 40-layer CPU transformer to logits |
| #12387 | Greedy decoding and an offline runner for the pinned checkpoint |

These three issues are closed on `main` through `d912ed9392`. The complete
model and greedy runner pass 57 CPU tests and two argument tests on synthetic
fixtures. Original-weight comparison is W1 (#12388), now claimed by this chat.

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
| W1 | **Original-weight CPU parity run.** On a cloud CPU machine with enough RAM (for example `n2d-highmem-96`, 768 GB, about $5.47/h), copy the checkpoint from the bucket to local SSD and run the #12387 runner against the oracle prompts. Compare decode-to-decode and replay-to-replay at every recorded position with tolerances fixed in advance. | M, cloud (about 10 h, under $60) | #12385–#12387 | G1's parity report is committed. A mismatch is reported with the first diverging layer and tensor, from the oracle's per-layer hashes. |
| W2 | **Complete answers on CPU.** Greedy to EOS for prompts 1 and 2 with the #12372 parser. | S, same machine | W1 | G2 passes; wall time per token is recorded. |
| W3 | **Measure the model.** Peak memory per layer range, KV and indexer bytes per token at each compression ratio, Engram lookup cost from RAM, and active parameters per token. | S, same machine | W1 | Numbers replace the estimates in the plan; placement inputs for W9 exist. |

### Phase 3: CUDA (G3, G4)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W4 | **Block-scaled matrix-vector kernels.** FP4 E2M1 and FP8 E4M3 weights with UE8M0 scales, FP8 activations, as the reference's numerical boundaries require. | M | W1 | Parity with the CPU path on real rows on the 4080 and the target GPU. |
| W5 | **Attention kernels.** Window attention with sink, compressed attention at ratios 1 and 2, the indexer, candidate selection, and the shared-source reads. | L (split per mode) | W1 | Per-kernel parity on both GPUs, including the 127/128/129 cases. |
| W6 | **Mixing, Engram, and routing kernels.** Sinkhorn mixing, the Engram gather from host-pinned memory, the gate and top-k with the documented tie rule, SwiGLU with clamps. | M | W1 | Per-kernel parity on both GPUs. |
| W7 | **Device-resident step.** One stage's whole decode step on the GPU, with embeddings, norms, rotation, and sampling on the device, and CUDA graphs for decode. | L | W4–W6 | G4 passes for a 5- or 10-layer stage on the target GPU; per-token latency and peak memory are recorded. |
| W8 | **Target GPU choice.** Confirm the kernels on the RTX PRO 6000 (SM120, G4 machines) and decide whether B200 (SM100) needs a separate profile. | S, cloud | W7 | A recorded decision with parity on the chosen GPU. |

### Phase 4: stage Pylons (G5, G6, G7)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W9 | **Stage-state codec and group manifest.** One typed message carrying the four residual streams, shared KV and indexer state, layer-20 candidates, Engram history, position, request, attempt, and group generation, generalizing the Gemma 4 step/reset path. A manifest names each stage's layers, GPU, and Engram location; placement fits stages to measured memory from W3 and evaluates four stages before eight. | M | W3 | Codec fault tests (duplicate, stale, missing, reordered, cross-request) pass with a synthetic executor before any rental. |
| W10 | **Group lifecycle.** Stage processes as managed Pylon workers, readiness before advertising, generation fencing, per-stage crash and freeze recovery, cancellation that holds the group until work stops. Prototype on the native R1 worker first. | L | W9, #12364 | G7's fault checks pass on a local or synthetic group. |
| W11 | **A full group on one machine.** All stages on one 4- or 8-GPU machine over loopback. | M, cloud (G4 spot, about 20 h) | W7, W10 | G5 passes. |
| W12 | **A group across machines.** Two placements across separate machines in one zone. | M, cloud | W11 | G6 passes; G7's fault checks repeat on the cross-machine group. |

### Phase 5: launch and visibility (G8)

| ID | Work | Size | Depends on | Done when |
| --- | --- | --- | --- | --- |
| W13 | **Group beacon and routing.** Done (#12400): the group publishes one routable beacon (one serial slot, the exact model artifact, its declared deadline and output limit); stage beacons are visible but not routable. The router matches requested model and group readiness. W10 feeds readiness through `pylon::group::GroupReadiness`, and W15 publishes the beacons. | M | W10 | Router tests pick the group only for V4.1 requests and only when it's ready. |
| W14 | **Deadline and answer profile.** Qualify a V4.1 reasoning profile (output cap, deadline) that finishes complete answers within the deadline the beacon declares. Changing Pylon's 90-second default is a versioned transport change with old-client refusal. | M | W11, W13 | Complete answers for the qualification prompts through Pylon within the declared deadline. |
| W15 | **`pylon cloud up` for a group.** One command starts a group in a named placement through the spend guard, waits for readiness, publishes it, and stops it on expiry, cap, or `down`. Spot loss shows as Pylons leaving. | M | W12, W13 | A cold launch to a first answer, then a clean stop, with costs in the ledger. |
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
| W1–W3 CPU parity and measurements | under $60 |
| W8 target GPU check | under $50 |
| W11 one-machine group development and runs | about $300 (a `g4-standard-384` at $13.31/h spot, about 20 h) |
| W12 cross-machine placements | about $200 |
| W15–W16 launch and recording runs | about $150 |
| Expected total | about $1,000, inside the $3,000 cap |

## Who can do what in parallel

- **Codex (DeepSeek chat)** holds the CPU lane: #12385–#12387, then W1–W3.
- **A second lane** can start W9's codec and fault fixtures and W10's
  lifecycle on the R1 native worker now; neither needs V4.1 weights.
- **A third lane** can start W13's beacon and router tests with fixtures.
- **CUDA (W4–W7)** starts after W1, because kernel parity is checked against
  a CPU path that already matches the real model.

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
