# Proposed table of contents

Proposal, updated 2026-10-29. [Chapter 1](ch001-learning-a-number.md) and
[Chapter 2](ch002-shapes-and-storage.md) and
[Chapter 3](ch003-numbers-and-matrices.md) and
[Chapter 4](ch004-reading-original-weights.md) and
[Chapter 5](ch005-text-becomes-a-model-request.md) and
[Chapter 6](ch006-attention-without-training.md) and
[Chapter 7](ch007-a-hybrid-sequence-model.md) and
[Chapter 8](ch008-decisions-have-a-different-mask.md) and
[Chapter 9](ch009-read-the-options.md) and
[Chapter 10](ch010-execute-elephant.md) and
[Chapter 11](ch011-execute-mammoth.md) and
[Chapter 12](ch012-are-the-probabilities-any-good.md) and
[Chapter 13](ch013-qualify-honestly.md) and
[Chapter 16](ch016-serve-systemone.md) are runnable on the tested CPU host.
[Chapter 26](ch026-a-coax-compute-subset.md) and
[Chapter 30](ch030-elephant-on-cuda.md) are runnable on the tested NVIDIA host
(Elephant, and Mammoth in [chapter 30b](ch030b-mammoth-on-cuda.md)), and their
examples run without a GPU.
[Chapter 31](ch031-elephant-on-metal.md) is runnable on the tested Apple M5 Max
(Elephant only), and [chapter 45h](ch045h-gpt-oss-on-metal.md) runs and serves
gpt-oss-20b there.
The [first gpt-oss format chapter, 41](ch041-original-mxfp4-weights.md), is also
runnable as a bounded CPU decoder. [Chapter 42](ch042-a-few-experts-then-a-router.md)
adds the bounded top-k router and tiny affine experts.
[Chapter 43](ch043-attention-with-sinks-and-a-window.md) adds bounded sink/window
attention, split-half YaRN, biased projection and clamped activation. Chapters
45–48 now provide CPU reference execution, sampling, speculative verification
and a finite-schema tool round trip. Chapters
[165](ch165-a-machine-whose-state-is-a-trace.md) and
[166](ch166-attention-as-exact-memory.md) open Part XXIII with a reference
trace machine and attention as exact memory,
[167](ch167-arithmetic-and-branches-from-relu.md) and
[168](ch168-compile-a-gate-graph-into-a-block.md) compile exact arithmetic into
feed-forward weights, [169](ch169-a-harness-that-catches-the-compiler.md) catches
planted compiler bugs, and
[172](ch172-from-hard-max-to-softmax.md) measures when a floating-point softmax
keeps that memory exact. **GPU execution and useful throughput are
not complete.** The device sequence below is the next GPT-OSS delivery path.
[Chapter 56](ch056-execute-a-deepseek-r1-distill.md) adds a bounded original
DeepSeek R1-Distill-Qwen-1.5B CPU reference and a complete greedy greeting.
[Chapters 70](ch070-learning-with-arrays.md) and
[71](ch071-gradients-as-a-program.md) are runnable on the CPU: learning on
simulated deadline data, first with hand-written gradients, then with a tape.
[Chapter 156](ch156-fit-our-own-calibration.md) fits and gates its own
calibration maps, and refuses them for another model version.
[Chapter 157](ch157-thresholds-from-written-costs.md) derives a cascade's
thresholds from written costs on synthetic probabilities.
[Chapter 158](ch158-record-readings-join-outcomes-watch-drift.md) logs every
decision, joins its outcome, and watches calibration drift.
[Chapter 178](ch178-a-suite-is-three-digests.md) pins a suite as three
digests and keeps results in a receipt chain.
[Chapter 179](ch179-how-far-a-metric-moves-by-itself.md) measures noise floors
from seed blocks and task-clustered bootstraps, and refuses partitions with no
headroom.
[Chapter 180](ch180-gates-that-can-say-unverifiable.md) turns them into
passed, failed or unverifiable verdicts for chapters 12, 156 and 157.
[Chapter 182](ch182-fit-settings-on-recorded-answers.md) fits a stall
detector's settings on recorded answers with no model calls.
[Chapter 183](ch183-search-a-policy-cheapest-first.md) searches 32 policies
by successive halving and finds the full measurement's winner for a quarter
of its spend.
[Chapter 164a](ch164a-learn-which-model-to-call.md) uses chapter 70's trainer
to choose among three simulated models and to judge a new router from an old
one's logs.
The remaining chapters below are outlines, not implemented capabilities. A
chapter ships with shared Coax library code, authored checks, independent
native tests, source-linked examples and an exact supported scope. Coax
listings use `jai` Markdown fences.

**The first delivery path is 1–3 → 4–10 (Elephant) → 11 (Mammoth) → 12–13
(published decision semantics, qualified honestly) → 14–18 (a live decision
API) → 19–21 (a first mesh of Pylons serving the same API).**
Throughput and device work in 22–35 begins as soon as it has concrete
operations and CPU oracles. Neither generic training, gpt-oss, Mistral,
DeepSeek, the mesh, a smaller imported model nor the NVIDIA branches blocks
those two decision models. [Model targets](../docs/targets/model-targets.md)
defines their exact architecture and acceptance gates.

The contents give a reading order; they do not require finishing every earlier
part before starting a branch. The prerequisite map below governs that choice.
Several chapters need Coax compiler features that do not exist yet; the
[Coax feature map](#coax-features-each-branch-needs) names them so Psionic can
act as their first real consumer.
The [roadmap](../docs/roadmap.md) sets delivery priorities, while the
[NVIDIA survey](../docs/targets/nvidia-breadth.md) pins the proposed external
targets and explains artifact, data and licensing qualifications. New breadth
is proposed here; this revision does not add or renumber live project issues.

## I. Small numbers, real contracts

| Chapter | What we build and check |
| --- | --- |
| [1. Learning a number](ch001-learning-a-number.md) — runnable | Learn `y = 2x`; derive the loss and gradient, verify the first update, and predict a held-out value. |
| [2. Shapes and storage](ch002-shapes-and-storage.md) — runnable | Admit bounded dimensions, caller-owned buffers, scratch and aliases; refuse malformed work before publishing output. |
| [3. Numbers and matrices](ch003-numbers-and-matrices.md) — runnable | Dot/matmul, gather, stable reductions, softmax and normalization, with independent numerical contracts. |
| [2a. A tensor is a view of memory](ch002a-a-tensor-is-a-view-of-memory.md) — runnable: bounded CPU views | Admitted ranks, strides, slicing, transposes, broadcasting, contiguous and noncontiguous layouts, zero-element tensors, storage ownership, and mutation and alias tracking. Every view's offsets are proved, and a mutation that would corrupt a saved value for backward is detected. Framework track. |
| [3a. A standard library of tensor operators](ch003a-a-standard-library-of-tensor-operators.md) — runnable: 27 CPU f32 operators and explicit value VJPs | Elementwise arithmetic and comparisons, reductions, indexed selection and updates, concatenation, sorting and top-k, matrix operations, convolution, pooling, interpolation and losses, each with a CPU reference, shape and dtype inference, and gradient coverage where differentiable. A machine-readable operator manifest records each operator's shapes, dtypes, backends, numerical profile and differentiation status, so support is measured, not inferred from running one architecture. Framework track. |
| [3b. One tensor, several devices and dtypes](ch003b-one-tensor-several-devices-and-dtypes.md) — runnable: tensor-device-v1, CPU/CUDA/Metal | Owned placement and transfers, immutable shared views and reference/reader tracking, explicit completion/cancellation, f32/f16/bf16 casts and promotion, seeded uniform tensors and dropout. One shared add/ReLU/dropout program is qualified on all three devices, with independent byte/numerical checks and reported differences; no implicit CPU fallback. The 3a registry remains CPU f32. Framework track. |

## II. Open weights early: Elephant and Mammoth

This part introduces inference before automatic differentiation. It depends
on the shared numerics in I and supplies the forward primitives reused later.

| Chapter | What we build and check |
| --- | --- |
| [4. Reading original weights](ch004-reading-original-weights.md) — runnable | Bounded original-weights-v1 Safetensors/shard-index parsing, exact identities, BF16/F32 window decoding and typed manifests; complete inventory and malformed-artifact refusals. |
| [4a. Models travel between frameworks](ch004a-models-travel-between-frameworks.md) — runnable | Bounded named F32/BF16 Safetensors writing/inspection, a versioned C ABI and zero-copy leased CPU f32 DLPack; a NumPy model continues training in Coax and reloads independently for inference. ONNX graph exchange remains 161/161a; arbitrary optimizer conversion remains separate. |
| [5. Text becomes a model request](ch005-text-becomes-a-model-request.md) — runnable | qwen35-text-v1: frozen Base Labs rendering, Unicode-17 NFC, original Qwen byte-BPE tables, special tokens and exact shared-prefix layouts; explicit resource and structured-entry bounds. |
| [6. Attention without training](ch006-attention-without-training.md) — runnable | A bounded dense causal block, interleaved rotary tables, grouped-query attention and caches; full-sequence versus incremental results. |
| [7. A hybrid sequence model](ch007-a-hybrid-sequence-model.md) — runnable: bounded causal CPU profile | Qwen-style dense blocks, causal convolution and Gated DeltaNet state transitions alongside causal full attention; upstream and real-weight qualification remain separate. |
| [8. Decisions have a different mask](ch008-decisions-have-a-different-mask.md) — runnable: bounded CPU | Block-bidirectional full attention and complete hybrid prefix snapshots, with sequential isolated suffix forks; whole-model qualification remains separate. |
| [9. Read the options](ch009-read-the-options.md) — runnable: bounded CPU | Slot LM-head probabilities, vocabulary mass and typed statistics with explicit temperature; serialization and calibration loading remain separate. |
| [10. Execute Elephant](ch010-execute-elephant.md) — runnable: original weights, CPU | Authenticated original Elephant files and a typed text decision through the complete 32-layer hybrid stack. Compared with an independent CPU implementation at an intermediate layer and at the output for the recorded request; a request corpus under chapter 13's rules, calibration (12) and measured resource admission remain open gates. |
| [11. Execute Mammoth](ch011-execute-mammoth.md) — runnable: original weights, CPU | The chapter 10 engine with Mammoth's pinned dimensions, two authenticated shards and a library storage plan (`ps_plan`); one typed text decision through all 64 layers, compared with an independent layer-streamed float32 reference for the recorded request. A request corpus under chapter 13's rules, a Mammoth quality run under chapter 12 and measured resource admission remain open gates. |
| [12. Are the probabilities any good?](ch012-are-the-probabilities-any-good.md) — runnable: CPU; 12-request Elephant run | Load exactly the calibration the pinned bundle supplies (or state that none exists and keep temperature explicit); reproduce the published serialized answers; Brier score, calibration error, abstention and per-kind accuracy on a frozen held-out request corpus, kept separate from numerical conformance. Every result names the model identity and the corpus it was measured on. Neither pinned bundle has a calibration file, so temperature is 1.0; a larger corpus remains open. |
| [13. Qualify honestly](ch013-qualify-honestly.md) — runnable: CPU, chapter 10 found unknown | Compare with a reference whose arithmetic cannot be reproduced exactly: only like modes (decode with decode, replay with replay); distribution checks (top-1 agreement, total variation, router-margin and near-tie regret) frozen before any confirmation set is read and never loosened after; values measured on the device that ran, not a surrogate. Every result binds the exact build, libraries and arithmetic actually executed; the command's exit status is the verdict; an absent qualification reads as unknown; failures are quoted with their units and counterexamples. |

Text support is the first scope. Image inputs, quantized variants and serving
integration each receive separate qualification. A layer fixture or imported
tensor inventory does not complete either real-model chapter. Chapter 12
measures decision quality; a matching logit is not evidence that a decision
is useful, and a good held-out score is not evidence of numerical parity.

## III. A live decision API

Mammoth and Elephant become something people and agents can call. This part
builds the first production deployment on purpose small: one model resident
in one process, one decision at a time, behind the same `/v1/systemone`
contract the decision jobs use ([NIP-DEC](../../nips/openagents/NIP-DEC.md)).
It establishes how to measure a decision and how to make the numbers move;
Part XIX later grows the same service into batching, caching, autoscaling and
rollout.

| Chapter | What we build and check |
| --- | --- |
| [14. Time a decision](ch014-time-a-decision.md) — runnable | Measured monotonic phases, observed cold/warm file pages, peak process RSS and reproducible logical-token rates and P50/P95/P99 samples. |
| [15. Keep the model resident](ch015-keep-the-model-resident.md) — runnable | One authenticated session and reusable bounded execution arenas, descriptor-bound verified files and typed capacity refusals; device requests avoid redundant CPU state. |
| [16. Serve `/v1/systemone`](ch016-serve-systemone.md) — runnable: loopback, toy and original Elephant, on the CPU or in about 148 ms end to end on an RTX 4080 (`--elephant-gpu`) | A small HTTP host does sockets; Coax decides everything else. Bounded NIP-DEC bodies, the same body as a decision job (the HTTP carriage of the protocol specification's `decision.v1.SystemOneRequest`, which has no `proto/` file yet), typed refusals, a deadline per request, cancellation, a busy refusal instead of an unbounded queue, and the served model identity (the authenticated checkpoint digest) in every answer. No request or answer content is logged. |
| [16b. The protobuf wire](ch016b-the-protobuf-wire.md) — runnable: toy engine and native answers | `openagents.decision.v1alpha1`, a protobuf projection of the frozen `openagents.systemone.v1` JSON contract, served by the same endpoint: a protobuf request is transcoded to its NIP-DEC body and admitted by chapter 16's decoder, and a protobuf answer re-encodes the rendered JSON answer. Checked against the official Python protobuf runtime in both directions. |
| [17. Run it live](ch017-run-it-live.md) — runnable | Golden warm-up before readiness, versioned health, bounded Coax restart supervision, private usage records and an owned loopback deployment. |
| [18. Make the numbers move](ch018-make-the-numbers-move.md) — runnable | Exact per-request CPU and device prefix snapshots, isolated suffixes and device host-arena elimination, with parity and measured changes. The [CUDA/Metal prefix work](../../docs/verification/psionic-ch31-fast-2026-10-29.md) retains its bit-identical whole-decision comparison (M5 Max Lab 4.1–5.4 s → 2.3–2.5 s; RTX 4080 1.84 → 1.17 s). Deterministic CPU worker pools remain chapter 24 and wider decoded-cache work remains later. |

The hosting machine, domain, budget and any public access are owner
decisions recorded in `NEEDS_OWNER.md`. The first owned loopback deployment is now
checked on Linux/CUDA, with its emitted-C producer, native compiler and
operational evidence in the [Part III record](../../docs/verification/psionic-partIII-2026-10-29.md).

## IV. A first mesh of Pylons

The same API fans out to connected machines. A Pylon is one provider machine
that opted in; it advertises what it serves and takes jobs over Nostr, as
specified in [NIP-PYLON](../../nips/openagents/NIP-PYLON.md) and the decision
jobs of [NIP-DEC](../../nips/openagents/NIP-DEC.md). This part routes whole
decisions to whole machines. Splitting one model across machines waits for
Part XX.

| Chapter | What we build and check |
| --- | --- |
| [19. A Pylon answers decisions](ch019-a-pylon-answers-decisions.md) — prerequisites [below](#before-chapter-19-the-nostr-prerequisites) | A Coax Pylon with its own key: a beacon (kind `30200`) advertising a decision service and the served model identity, free slots and a declared deadline; decision jobs received encrypted over relays (NIP-44) and answered by the resident model of Part III, with the answer naming the same identity. Built on the Nostr event code in `coax/lib/nostr`. |
| 20. The API fans out | `/v1/systemone` reads fresh beacons, chooses a Pylon with a free slot that serves the exact model identity, sends the job, checks that the answer's identity matches the beacon, and falls back to the local model or refuses when none answers in time. Beacon expiry, generation counters and draining keep stale machines out of the route. |
| 21. Check the mesh | Buyers' signed receipts (kind `3201`) for completed jobs, a checker that re-runs a sampled fraction of decisions on a trusted node and publishes its verdict, and pool totals any reader can recompute. Decision work starts free; paying Pylons in Bitcoin is a later owner decision. |

Nothing here makes a peer's answer private: a Pylon that serves a request
reads it. Part XX adds trust levels, attested machines and model splits.

### Before chapter 19: the Nostr prerequisites

Chapter 19 needs a Pylon that signs, encrypts and holds a relay connection.
The decision side is done: chapter 16 serves Elephant on `/v1/systemone`
(on the GPU beside gpt-oss, chapters 30a and 48c), and NIP-DEC's job body is
the same `openagents.systemone.v1` shape. As of 2026-10-29 the crypto and
codecs exist (BIP-340 verification, secp256k1, ECDH, ChaCha20, HMAC, HKDF,
XChaCha20-Poly1305, entropy, JSON, filters, NIP-19) and these remain, in
order. Board aliases are on the [signed queue](../../coax/web/project/QUEUE.md).

| # | Prerequisite | Who | State |
| --- | --- | --- | --- |
| 1 | Complete BIP-340 signer (`coax/lib/crypto/schnorr_sign.coax`) and a fixed-work self-check (`schnorr_self_verify.coax`) | openagents-d7 Fork A, claude-sigcheck (#51f4, #89db) | signer landed `0cc86015e7` (3,384 mutants, 0 survivors; the 8 BIP-340 signing rows); self-check wiring follows; test keys only until the #4828 secret audit (openagents-20: partial in about 1.5 h, full later) and the review gate |
| 2 | NIP-01 event builder: fields, id, sign, serialize (`coax/lib/nostr/event_sign.coax`) | openagents-d7 Fork A (#bab4) | landed `7bef91d26c`: 5,194 mutants, 0 survivors; native under ASan/UBSan; test keys only |
| 3 | NIP-44 v2 (`coax/lib/nostr/nip44.coax`): padding, message keys, MAC before plaintext, every official vector | claude-nip44 (#4878); x-only ECDH from codex-nostr-profiles (#71e7) | landed `f74988b29a` (1,700 mutants, 0 survivors) and the conversation key from two keys `a7589e2c90` (`nip44-key.coax`, 35/35 valid and 8/8 invalid vectors, 3,434 mutants, 0 survivors; interpreter only so far); the signed-event round trip waits on the event builder |
| 4a | Relay messages: EVENT/OK/EOSE/CLOSED/NOTICE readers, EVENT/REQ/CLOSE writers (`coax/lib/nostr/message.coax`) | openagents-d7 Fork B (#6015) | mutation gate running |
| 4b | Native relay client: RFC 6455 WebSocket in Coax, the C socket effect, the relay state machine with reconnect and catch-up, an end-to-end local relay test | claude-relay (#7de9) | landed `d7373bcc82`: SHA-1, `coax/lib/net/ws.coax`, `coax/lib/nostr/relay.coax` (0 survivors each) and `coax/platform/socket-native.c`; the end-to-end local-relay test, entropy for mask keys and wiring `message.coax` remain; `ws://` only |
| 4c | NIP-42 AUTH (`coax/lib/nostr/nip42.coax`): AUTH frames, the kind 22242 template, the relay-side check | claude-nip42 (#f426) | landed `9a655d3736`, `8e552b89dd`: 1,940 mutants, 0 survivors; 219 real-signature checks. Signing plugs in through `nostr_event_sign_step`; the relay answers AUTH once R1 wires it |
| 4d | `wss://`: TLS in the C host socket, certificate verification, the end-to-end test through TLS | claude-tls | landed `604767080e`: OpenSSL 3.6.3 through Nix (`coax/platform/tls-env.sh`), TLS 1.2 minimum, hostname and chain checks, `relay_uri.coax` (0 survivors); the local relay behind TLS and public REQ/EOSE on nos.lol and relay.primal.net pass. The Apple branch is a stub; the relay driver still has to pick TLS from `relay_uri` |
| 4e | Relay end-to-end test against a local relay, entropy for mask keys, wiring `message.coax` | claude-relay-e2e | landed `32269ea6a4`: connect, REQ/EVENT/EOSE, OK true and false, ping/pong, drop → backoff → resubscribe with `since` → resend, clean close, under ASan/UBSan in 2.2 s; OS entropy for masks and keys. Wiring `message.coax` (one call) waits on #6015; the connect loop moves into `coax/lib/` for the Pylon |
| 5 | Beacon (kind `30200`) template, checker and lifecycle (`psionic/lib/pylon/`) | claude-pylon | landed `2f489e00d9`: writer (JCS content), reader-side checker, lifecycle and republish schedule; 0 survivors |
| 6 | Decision jobs: `25910` plaintext to the server's request, the answer to `26910`, `27010` status, cancel onto the server's deadline | claude-pylon | landed `2f489e00d9`: built to the deployed wire (`outcome: "answered"`, refusals as `27010`); 2,346 mutants, 0 survivors. The execution receipt is still the caller's |
| 7 | NIP-65 relay list (kind `10002`) | claude-pylon | landed `2f489e00d9`; 1,237 mutants, 0 survivors |

All four owner decisions are made (`NEEDS_OWNER.md`): the `auth` signature
over the Pylon key is made by the agent that creates the real key, once
signing is cleared for real keys; the key is a 0600 file in `~/work/.secrets/` on the
owner's Mac; the relays are relay.openagents.com and every Coax relay we run;
the C host may link a TLS library. The owner allowed a
TLS library in the C host for `wss://` on 2026-10-29 (AGENTS.md).

## V. Make execution measurable, parallel and portable

These chapters accompany real consumers in II; their placement is explanatory.
The device path comes first, because sub-second decisions need it: a compute
subset in Coax, whole-prompt attention and DeltaNet kernels, fusion and graph
replay, then CUDA on the RTX 4080 and Metal on Apple GPUs. Several designs come
from [llama.cpp](../docs/llama-cpp.md), reimplemented in Coax with their own
checks; llama.cpp itself is a benchmark comparator, never a dependency.

| Chapter | What we build and check |
| --- | --- |
| 22. Measure the work and plan the memory | Fixed CPU numerical references, peak memory, latency samples and a reproducible workload before optimizing. Measured compute and memory bandwidth for the host, and the arithmetic intensity of prefill and decode against that roofline, so every later optimization names the bottleneck it targets. A residency plan for each model: compute directly from mapped BF16 rows without f32 expansion, separate persistent/state/scratch bytes, and refuse requests whose plan does not fit. |
| 23. Benchmark against llama.cpp and cuBLAS | The same model, the same quantized file and the same prompt shapes run through Psionic and a packaged llama.cpp build: prompt processing and generation at fixed lengths, decision-shaped runs (a shared prefix plus suffixes), warm-up then at least five repetitions reported as mean and standard deviation, on a quiet machine with recorded clocks; cuBLAS products on the same shapes as an opt-in comparator for Psionic's own kernels, never a dependency. |
| 24. Use every core | Fixed work partitioning and tree-ordered merges so every output is byte-identical at 1, 2, 4 and 8 workers; a host thread pool calling Coax kernels on disjoint ranges until Coax jobs exist, then the same replay gate on jobs. |
| 25. Many questions at once | Batched suffix forks from one prefix snapshot, proven equal to the sequential forks of chapter 8; reuse of prefix state across requests keyed by model identity and exact token prefix, with bounded eviction. |
| [26. A Coax compute subset](ch026-a-coax-compute-subset.md) — runnable: `coax cuda`, BF16 projection kernels on an RTX 4080; group kernels (threadgroup memory and barriers) and Metal 4 tensor-op tiles on Apple GPUs | The compiler's missing buffer, integer index, bounded loop, work-item and binding subset, proven on one small matrix-vector kernel: generated source compiles, real device execution matches the CPU oracle, negative admission refuses. |
| [26a. A tensor program becomes an execution plan](ch026a-a-tensor-program-becomes-an-execution-plan.md) — runnable: tensor-ir-f32-v1, CPU/CUDA/Metal | Bounded typed straight-line IR with matmul, broadcasting, ReLU, axis reduction, last-axis LayerNorm, read-only strided views, contiguous reshapes and constant gathers. One feed-forward block runs through CPU reference procedures, a reusable workspace plan and generated device kernels; root liveness protects aliases, actual allocation tracking measures memory, and exact graph/compiler/backend/module identity invalidates stale plans. f32 only; explicit graph construction and synchronous private arenas. Fusion, dynamic shapes and differentiation build on this representation. |
| [26b. Compile a general tensor program](ch026b-compile-a-general-tensor-program.md) — runnable | Bounded symbolic shapes and declared branches/loops, guarded specialization, four-entry LRU plans, aggregate payload admission and one shared backend compilation across batch sizes. |
| [26c. Add your own operator](ch026c-add-your-own-operator.md) — runnable | Public unary f32 schemas, CPU/reference callbacks, generated GPU lowering, input VJPs, batching and leased registry identities integrated into compiled IR models; squared-ReLU trains without new core operator cases. |
| 27. Attention in tiles | Online-softmax attention over key/value tiles that never materializes the full score matrix, with chunked prefill. The changed reduction order gets its own named profile and stated tolerance against the CPU reference, and the block-bidirectional mask, sinks and windows must still hold. Grouped-query attention loads each key/value head once for every query head that shares it. |
| 28. DeltaNet in chunks | Whole-prompt Gated DeltaNet prefill in chunks (the WY form: the chunk's triangular system solved by forward substitution, state carried in f32 between chunks), proven equal within a named profile to chapter 7's token-by-token recurrence, with state snapshots at chunk boundaries that chapter 8's forks and speculative rollback can reuse. |
| 29. Fuse and replay | Fused device operations (normalization with its scale, gate and up projections with their activation, expert routing, a recurrent step with its cache write), each admitted only when its buffers provably do not overlap, and a decode or prefill step recorded once and replayed as a device graph; each fusion keeps its unfused result within tolerance. |
| [30. Elephant and Mammoth on CUDA](ch030-elephant-on-cuda.md) — runnable for Elephant: about 147 ms per decision with tensor-core projections (about 120 ms for a forked question's suffix), layer-major, feed-forward matrices streamed when memory is short; Mammoth runnable in 30b | The first device backend, on an RTX 4080: Coax-generated PTX or CUDA C compiled by the driver or NVRTC, device buffers, launches and completion; whole-prompt prefill with chapters 27–29, the weights that do not fit in memory streamed layer by layer, and the complete decision qualified against the CPU reference. The prior bar on this GPU is about 0.2 s for a 1,000-token decision on a 9B hybrid. |
| [30a. Share the card](ch030a-share-the-card.md) — runnable: every Elephant matrix can stream | Elephant beside a resident gpt-oss-20b on the same 16 GB card: past the feed-forward matrices, the head streams in 21 chunks and then the mixer matrices, so only the workspace, the vectors and two 96 MiB slots must fit (283 MiB for the 86-token sample). A plan checker shows a streamed plan launches what the resident plan launches. The decision becomes PCIe-bound; quantized weights (34) halve the bytes and a card scheduler is a separate service. |
| [30b. Mammoth on CUDA](ch030b-mammoth-on-cuda.md) — runnable: the plan takes the model's shape as data; Mammoth's decision on the 4080 in about 1.6 s with 12.5 GiB resident and 37.8 GB streamed per decision, within 0.0005 of chapter 11's CPU result | Mammoth's 51.3 GB of text weights through chapter 30's engine on a 16 GB card: most of every decision streams over PCIe, so the time is the link's (23.7 GB/s). Elephant's plans are unchanged digest for digest. A forked Lab request is slower than whole decisions when every pass streams; batched suffixes and quantized weights (34) are what would help. |
| [31. Elephant and Mammoth on Metal](ch031-elephant-on-metal.md) — runnable for Elephant: the chapter 30 kernels through `coax metal-compute` on an Apple M5 Max, float-float f64 qualified exhaustively, the decision within 4.6e-6 of the CPU; with Metal 4 tensor-op tiles for the projections about 0.4 s warm (was 2.7–2.9 s) and the Lab's five questions about 2.4 s with forks (was 23–31 s); served by `psionic-serve --elephant-metal`; Mammoth remains planned | The same generated kernels for Apple GPUs, Metal bindings and completion/lifetime rules for the full decision path; qualify Elephant, then Mammoth, against the same CPU references. Apple GPUs have no double precision and flush subnormals, so the device profile states its own accumulation (compensated float-float sums, exhaustively checked transcendentals, host-built rotary tables) rather than inheriting chapter 3's binary64 profile. Tuning tables keyed by Apple GPU family and command buffers encoded in parallel. |
| 32. Integer matrix products | Activations quantized per block to 8 bits with stored sums, dot products accumulated exactly in 32-bit integers, then scaled: a row kernel for a few tokens and a tiled int8 tensor-core kernel for whole prompts, exact on the integer part and within a stated tolerance overall against chapter 3. |
| 33. Read a GGUF file | A bounded GGUF reader: magic, version, counts, typed metadata, tensor descriptors, alignment and data offsets, with every count, product and padding checked before use; checked on pinned upstream files and malformed headers. It opens most published quantized checkpoints. |
| 34. Quantized weights and caches | Q8_0 first (about 8.5 bits per weight, small enough to keep Elephant resident on a 16 GB GPU), then the 4- and 6-bit K-quants with importance-weighted quantization and per-tensor mixes (chapter 41's MXFP4 decoder is also a candidate), then an optional quantized KV cache; independent decode vectors, partial-row refusal, separate model profiles, and measured memory against quality: perplexity on fixed text, chapter 12's decision metrics and a product evaluation, each compared with the original weights against a quality loss declared before measuring, with an uncertainty estimate over the workload (a deterministic evaluation has no run-to-run noise, but it still samples future requests). Aggressive 2–3-bit consumer profiles carry a quality card (top-1 agreement and KL against the original, task scores) and never inherit the original's evidence. |
| 35. K-quants and lattice quants | Decoders for the 2- to 6-bit K-quant blocks and the non-uniform and lattice-codebook formats (IQ4, IQ3, IQ2), each with independent decode vectors and the quality card of chapter 34 (mean KL divergence, same-top-token rate, perplexity with its error), measured against the original weights. |

NVIDIA model compatibility does not require CUDA. CUDA and Metal are separate
backend projects; neither upstream speed claims nor a reference runtime
establishes Psionic execution. CUDA comes first because it is the fastest
device on hand. Chapter 30 relies on Coax running on Linux, which works on the
development machine but is not yet a qualified Coax target; cuBLAS appears only
in chapter 23's comparisons. Quantized profiles (32–35) never inherit the
original weights' evidence. Chapters 24 and 25 change scheduling, never
results: each must reproduce its single-worker or sequential output exactly
under the named CPU profile before it counts.

## VI. Useful inference beyond decisions

| Chapter | What we build and check |
| --- | --- |
| 36. Tokenizers and chat templates | The other tokenizer families (SentencePiece, WordPiece, Unigram) beside chapter 5's byte-pair encoder, and a bounded subset of the Jinja chat-template language, checked against published tokenizer vectors whose expected ids come from the upstream tokenizers and against real model templates. |
| 37. Choosing the next token | A named counter-based RNG with explicit seed/counter progression; temperature, top-k and top-p with stated tie rules; stop conditions and streamed tokens with cancellation; constrained decoding that masks the vocabulary to a grammar or JSON schema using chapter 5's tokenizer tables. |
| 38. Search the Coax book | Nemotron-3-Embed-1B-BF16: exact query/document formatting, bidirectional attention, padding-aware average pooling, normalization and held-out retrieval. Documents and chunks get stable identities, and two references come first: a BM25 lexical baseline and exhaustive vector search, which every faster index is measured against. |
| 38a. Search without scanning everything | One approximate index built from scratch (HNSW: graph construction, hierarchical search, bounded exploration, persistence and stated memory), introduced only once the corpus is large enough to need it; recall against exhaustive search at fixed budgets. |
| 38b. Rerank the candidates | Lexical and dense results merged, then a small shortlist scored more carefully; index recall, retrieval relevance and answer quality reported separately, so a gain in one is never credited to another. |
| 38c. Answer from current sources | A Coax documentation assistant that answers from retrieved evidence and shows the exact passages, with their source spans and versions; a corrected or deleted document stops appearing in results, and answers are checked for using the evidence they cite. |
| 39. Continue a passage | Llama-3.1-Minitron-4B-Width-Base: exact head dimensions/position rules, full-versus-cached logits, and deterministic and seeded continuation through chapter 37. |
| 40. Run in the browser | Psionic CPU kernels compiled to Wasm through Coaxial, then WebGPU compute once Coax emits it; serve chapter 38's search on openagents.com with natively precomputed document embeddings and a separately selected query encoder small enough to download. A smaller browser query encoder must be shown compatible with the stored document embeddings, by a measured alignment or distillation or by a matched query and document encoder pair; equal vector dimensions prove nothing. |

These are forward-only applications of I–II, with their own architecture
profiles; training the retriever itself on contrastive pairs and hard
negatives is a later extension on Part XII's machinery. Constrained decoding is a sampling contract, not a parser of model
output after the fact: a token outside the grammar is never offered. Chapter 40
is a separate target with its own ABI, memory-growth and numerical evidence;
a Mac result does not qualify it. The optional small import lab in appendix A
can help readers with limited memory; it is not an extra dependency of
Elephant or Mammoth.

## VII. An open reasoning model: gpt-oss

OpenAI's [gpt-oss](../docs/targets/gpt-oss.md) is the first general
assistant model in the plan: Apache-2.0 weights, a mixture-of-experts
decoder and a structured response format built for tools and reasoning. It
needs no training and no hybrid recurrence, only the forward core of I–II,
the memory plan of 22 and the sampling of 30.

| Chapter | What we build and check |
| --- | --- |
| [41. Original MXFP4 weights](ch041-original-mxfp4-weights.md) — runnable: bounded CPU format decoder | The checkpoint's own MoE format: 32 FP4 (E2M1) values packed into 16 bytes per block, with one biased power-of-two scale per block in a separate `.scales` tensor. Decode every code at every admitted scale exactly into `f32` against an independent table, refuse partial blocks and out-of-range scales, and keep this original format distinct from chapter 34's derived quantization profile. |
| [42. A few experts, then a router](ch042-a-few-experts-then-a-router.md) — runnable: bounded CPU | A tiny MoE with directly checkable choices, ties, weighting, dispatch/gather and expert-residency budgets; then gpt-oss's selected-only softmax over sorted top-four logits, under an explicit wide CPU and lowest-ID-tie profile. Original gate projection and BF16 parity remain separate. |
| [43. Attention with sinks and a window](ch043-attention-with-sinks-and-a-window.md) — runnable: bounded wide CPU | Learned per-head sink logits that join each softmax denominator without contributing a value; a 128-token banded window on alternate layers beside full attention, with a window-bounded cache; YaRN rotary scaling (base 150,000, factor 32 over 4,096 original positions); a reusable bounded biased projection; and the clamped SwiGLU (alpha 1.702, limit 7, plus-one linear term). Projected-Q/K/V execution supports 64/8 heads, windows through 128 and absolute positions through 131,071; full attention is limited to 128 tokens. Full-size original projections and whole-block/model qualification remain chapter 45 gates. |
| [44. The harmony format](ch044-the-harmony-format.md) — runnable: bounded text profile | Original o200k byte ranks and case-sensitive splitting with Unicode-17 properties, shared byte-BPE and separately pinned table identity (201,088 vocabulary entries). Typed system/developer/user/assistant/tool framing, low/medium/high reasoning templates, ordinary-text marker isolation and strict full/completion parsing of analysis/commentary/final, with exact return/call stops and atomic publication. Bounds: 64 messages, 16,384 IDs and 65,536 decoded bytes; optional header extensions and tool execution remain separate. |
| [45. Execute gpt-oss-20b](ch045-execute-gpt-oss-20b.md) — CPU reference complete; GPU throughput open | Complete authenticated original BF16/MXFP4 execution, seeded temperature/top-k/top-p sampling, independent dense intermediate/router/logit references, exact cached/replay checks and measured CPU residency. Implements the sampling primitives needed here without completing chapter 37’s broader scope. |
| [46. Execute gpt-oss-120b](ch046-execute-gpt-oss-120b.md) — CPU reference complete; GPU throughput open | The same CPU engine with 36 layers and 128 experts, separately authenticated seven-shard originals, independent full-stack references and measured resident memory on the 128 GiB host. Offload remains a separate profile for smaller hosts. |
| [47. Let the small model guess](ch047-let-the-small-model-guess.md) — correctness reference complete; fast device verification open | Standard acceptance, positive-residual correction and all-match bonus, complete KV snapshots/replay and prompt n-grams. Exact tiny sampled checks and original 20b→120b greedy parity for draft limits 1/2/4; measured serial target batch 1 is slower than target alone, so no acceleration claim. |
| [48. Call a tool](ch048-call-a-tool.md) — bounded reference complete; device serving integration open | Commentary handoffs with a bounded root-object JSON-schema enum masking the vocabulary before sampling; explicit host grant, named tool result and final-only display. Original 20b forced named calculator round trip; general schemas and autonomous tool choice remain separate profiles. |


### GPT-OSS device delivery order

The CPU chapters establish a numerical reference. They do not complete the
usable-inference goal. Advance this track before adding another model family.
The lettered chapters preserve existing chapter numbers and other agents' rows;
26, 27 and 29 supply shared device infrastructure as it becomes available.

| Order | Chapter / milestone | Completion gate |
| --- | --- | --- |
| 1 | [45a. Original weights stay on the GPU](ch045a-original-weights-stay-on-the-gpu.md) — qualified: original 20b / RTX 4080 | A pure Coax memory/dispatch plan admits the complete original 20b payload, immutable packed MXFP4 experts, persistent activations and a bounded KV cache; actual allocation on the 16 GiB RTX 4080 succeeds with measured headroom. Authenticate/upload once. Coax kernels qualify BF16 and fused original-MXFP4 products on small fixtures and original-shaped projections. |
| 2 | [45b. A complete CUDA decoder](ch045b-a-complete-cuda-decoder.md) — qualified: greedy original 20b | Norm, projections, YaRN, sink/window attention, routing, selected experts, residuals and head execute on device. Original 20b logits/routes meet a tolerance frozen before comparison with recorded CPU references. A complete Harmony turn runs with no per-layer activation/route downloads or weight transfers. |
| 3 | [45c. Prefill together](ch045c-prefill-together.md) — runnable: 128-token grouped SIMT chunks | Batch prompt tokens into matrix products; group tokens by expert; tiled sink/window attention and fused small operations. Shared memory/reductions/matrix instructions extend Coax compute where measurements require them. Report time to first token separately from steady decode, with exact model/context/build identities. |
| 4 | **45d. Coax kernels cooperate** | Extend the compute contract with explicit cooperative tiles, uniform participation/barriers, lane exchange, device capability admission and packed BF16/FP16 matrix primitives. Coax owns semantics and admission; the native backend lowers them. CPU scalar oracles, negative participation/capability cases and small actual-device checks precede any full model. No hand-written model kernels. |
| 5 | **45e. Original MXFP4 products use matrix hardware** | Stage bounded original coefficients exactly, split f32 activations into checked BF16 limbs where needed and accumulate tile products under a separately named numerical profile. Freeze its bounds before comparing with independent tiny and retained original vectors; preserve exact route/top-token gates. Measure the selected-expert bottleneck before/after; do not silently replace the f32 reference with a looser profile. |
| 6 | [45f. Replay the device program](ch045f-replay-the-device-program.md) — synchronous CUDA replay complete | Coax constructs a replayable device program with immutable admitted bindings and bounded dynamic token/position control. Native CUDA graph effects own node creation/update/replay/destruction. Failed/cancelled steps retain transaction boundaries; compare cache/logits/stop behavior to the unfused path and measure host enqueue savings. |
| 6a | [45h. gpt-oss-20b on Metal](ch045h-gpt-oss-on-metal.md) — qualified: original 20b / Apple M5 Max | The same Coax launch programs and kernels through `coax metal-compute` and `gptoss-metal.c`, with every weight resident and no CPU fallback. Chapter 45b's frozen rule passes against the recorded CPU vectors, and greedy token IDs equal chapter 45's CPU engine on both fixture prompts. A per-launch CPU repeat differs only in f32 `exp` ulps and float-float sums. Measured on a shared machine: 34.8 tokens/s decode and 6.44 s for the 1,024-token prefill, against the 4080's 53.2 tokens/s and 3.21 s. Served by `serve-gptoss.sh --metal` on port 8794; the desktop app uses it with `PSIONIC_GPTOSS_HOST=local`. Faster Metal products wait on 45d/45e. [Record](../../docs/verification/psionic-gptoss-metal-2026-10-29.md). |
| 7 | **45g. Throughput is a gate** | On the 4080, fixed 1,024-token input/128-token output workload, seeded requests, warm-up and at least five repeats. Goals: median single-stream decode at least 25 tokens/s and 1,024-token warm prefill at most 2 s. The first measured SIMT baseline reaches 28.454 tokens/s and 4.325127 s prefill; qualified attention/launch tuning reaches 29.586 tokens/s and 3.219239 s: decode passes, prefill remains open. Record P50/P95, allocation/peak device memory, transfers and phase times; compare an installed packaged reference on the same device. Add bounded continuous batching and KV allocation, and measure aggregate plus per-request rates at concurrency 1/4/8. |
| 8 | **46a. Fit 120b on the right device** | Use the same device engine on enough resident memory: qualify the Metal engine of 45h on the 128 GiB Mac, or an explicitly available larger/multiple CUDA device configuration. The 4080 cannot keep the complete original 120b weights resident. Hardware acquisition and new hosted machines remain owner decisions. |
| 9 | **47a. Verify a draft block in parallel** | Batched device target verification and rollback of only speculative state, preserving overwritten sliding slots. Greedy parity and tiny sampled-law checks remain required; measure actual acceptance and a net latency gain over the qualified target-only device path before enabling speculation by default. |
| 10 | **48a. Device generation calls a tool** | Reuse the same resident engine, bounded schema masks, grant checks, cancellation and final-only display in the streamed device path. No CPU fallback hidden in throughput measurements. |
| 11 | [48b. Serve Responses](ch048b-serve-responses.md) — resident original 20b, bounded text/function profile | Responses-native HTTP and semantic SSE, separate raw reasoning replay, sequential external function handoffs, cancellation, usage and explicit unsupported-feature errors. Start after 45c; keep the engine resident. Qualify owned HTTP/SDK/schema tests and real original-model requests. [Decode qualification](../../docs/verification/psionic-decode-speed-2026-10-29.md) records the first improvement; the [second pass](../../docs/verification/psionic-decode-pass2-2026-10-29.md) adds 15.7–35.5% decode throughput over it, with exact state/output. The [performance notebook](../docs/serving/performance.md) retains the process and backlog. Storage, strict decoding, continuous batching and production ingress remain later gates. |
| 11a | [48c. Two models, one card](ch048c-two-models-one-card.md) — runnable: gpt-oss-20b and Elephant on one RTX 4080 | Start order and memory budget for both servers on one 16 GB card (gpt-oss resident, Elephant streaming through chapter 30a), `--device-mib` for the other order, verified starts, and a client that chains them: gpt-oss reads an issue, Elephant ranks candidate files, gpt-oss writes the report. No card scheduler; concurrent use is unmeasured. |

The [45a–45c record](../../docs/verification/psionic-gptoss-device-2026-10-29.md)
retains independent-vector qualification, complete Harmony generation, bitwise
serial/batched GPU parity over 129 positions and five warm core measurements.
The selected original-MXFP4 products dominate diagnostic timing. This is why
cooperative/matrix support and graph replay now precede the final throughput
gate. The initial implementation is useful resident GPU inference, with further
throughput work explicitly open.

Check-first development uses small owned fixtures, recorded original answers and
focused changed-code checks. Full CPU model execution and whole mutation roots
are not routine iteration gates. Small real-device qualification precedes each
full-model GPU measurement. No hand-written arithmetic kernels or served BLAS
runtime is introduced; Coax owns kernels and decisions, and C owns device effects.

The torch reference upcasts MXFP4 to BF16, so a converted BF16 checkpoint is
not the original artifact; chapter 41 decodes the original blocks. The
reference torch, Triton and Metal implementations are read as untrusted
source, not executed, and their browser and Python tools are explicitly
educational upstream, so Psionic does not adopt them. The 20b model's
16 GB figure is an upstream claim until chapter 22's plan measures it.
Text input is the whole scope. This part does not block Elephant or Mammoth.

## VIII. Mistral models, from Ministral to Le Chonk

[Mistral](../docs/targets/mistral.md) publishes a ladder of open models
whose architectures add one feature at a time on the way to Mistral Large 4
("Le Chonk"): 1T total and about 52B active parameters, 128 routed experts plus
a shared expert, latent and sparse attention, FP8 weights and a built-in draft
head. This part qualifies each feature on the smallest model that has it. Large 4's
1,060.6 GB of original files fit no single machine, so it is executed in
Part XX, across a mesh.

| Chapter | What we build and check |
| --- | --- |
| 49. Execute Ministral 3 | Ministral-3-3B (Apache-2.0, BF16 originals): the Tekken tokenizer through chapter 5's byte-BPE engine with its own pinned tables, Mistral's chat and reasoning format, and a dense grouped-query decoder with its long-context rotary scaling; token, intermediate and logit references. |
| 50. Original FP8 weights | E4M3 values with one scale per 128×128 block, including Large 4's power-of-two (UE8M0) scales: exact decode of every code at every admitted scale against an independent table, partial-block and scale refusals, and FP8 originals kept distinct from chapter 34's derived profiles. The reference's dynamic FP8 activations are a separate approximate profile. |
| 51. A coding agent | Devstral-Small-2-24B (Apache-2.0, FP8): Mistral's function-calling format with arguments constrained by chapter 37, and long-context requests through chapters 27 and 119; first measured against a fixed coding-task set. |
| 52. Attention through a latent | Multi-head latent attention: queries and keys split into rotary and non-rotary parts, keys and values reconstructed from a low-rank latent, and the absorbed projections that let decode attend in latent space. The compressed cache's size is accounted exactly, and the absorbed form is compared with the expanded one. |
| 53. Execute Mistral Small 4 | Mistral-Small-4-119B (Apache-2.0, FP8, 241.9 GB of originals): latent attention plus 128 routed experts with four selected and one shared expert, extending chapter 42's router. Measured residency on one large-memory machine, or the expert paging of chapter 46. |
| 54. Sparse attention and shared layers | Large 4's attention features on tiny fixtures and selected real layers: a lightning indexer that picks the top 2,048 keys per query (FP8 indexer), indexer and KV sharing across layers from the pinned source map, no rotary positions on every fourth layer, tanh soft-capping of logits at 30, and YaRN scaling (factor 2048 over 512 positions) to 1,048,576. |
| 55. The model's own draft head | Large 4's embedded multi-token-prediction draft head as a speculator under chapter 47's acceptance rule: identical greedy output, exact acceptance probabilities on tiny fixtures, and measured acceptance and draft length. |

Ministral 3, Devstral Small 2, Mistral Small 4 and Mistral Large 3 are Apache-2.0.
Large 4 uses the Mistral Ecosystem Open-Weight License. It permits internal
use and embedded product features, but offering the model as a service needs a
separate commercial licence from Mistral, and that is an owner decision. Text is the first
scope; the Pixtral-style vision encoders are separate image profiles in
Part XIII. The NVFP4 variants are derived checkpoints for Blackwell GPUs, not originals.

## IX. DeepSeek, natively

DeepSeek publishes both open models and unusually detailed accounts of how it
serves them. This part builds our own version of the
[pre-reset DeepSeek V4.1 Flash work](../docs/targets/deepseek.md) in Coax:
the model ladder from a small distill to V4.1 Flash, V4.1's distinctive
components, and the engine techniques behind its efficiency. Each technique
is exact unless its profile says otherwise. The old Rust attempt ran V4.1 end
to end but never passed numerical parity; chapter 13's rules come from that
failure. It also served V4.1 Flash from one RTX PRO 6000 the way
[Project Maya](https://github.com/mw00/project-maya) does on home PCs (chapter
68a).

| Chapter | What we build and check |
| --- | --- |
| [56. Execute a DeepSeek R1 distill](ch056-execute-a-deepseek-r1-distill.md) — runnable: bounded original BF16 CPU reference | R1-Distill-Qwen-1.5B through the shared dense path: the distill's own tokenizer, BOS from the tokenizer rather than the config, a template that strips earlier reasoning, and separate reasoning and final output. |
| [57. DeepSeek’s front door](ch057-deepseeks-front-door.md) — runnable: original V4.1 text tokenizer, bounded recipe and stream protocol | The recipe layer in Coax. Special tokens are inserted by boundary, never by spelling, so user or tool text that spells a marker stays text. Segments between specials encode independently. A streaming parser carries incomplete UTF-8 and multi-token delimiters across chunks in linear time and emits tool names and escaped arguments as they arrive. A stream that ends without a finish event is reported incomplete. |
| [58. Execute DeepSeek V2-Lite](ch058-execute-deepseek-v2-lite.md) — runnable: complete original BF16 CPU reference | 16B total, 2.4B active: DeepSeek's latent attention (sharing chapter 52's math) with 64 routed and 2 shared experts, compared in compressed and expanded form with router and combine parity and cache continuation. Fits one Mac. |
| [59. Routers that balance themselves](ch059-routers-that-balance-themselves.md) — runnable: CPU wide routing equations | V3-style selection with a bias used only for choosing experts and unbiased scores for weighting, V4.1's square-root-softplus scores and route scale, explicit lowest-id tie rules, and router-margin records so near ties are visible in every comparison. |
| [60. Compressed and shared attention](ch060-compressed-and-shared-attention.md) — runnable: projected CPU wide attention and shared state | V4.1's attention: a 128-token window ring, a gated pooling compressor (ratio 2) and an ungated one (ratio 1), a single shared KV head with an attention sink, KV and indexer state reused across layers from pinned source layers, and candidate blocks chosen at one layer and read by the layers after it. Chunked and whole prompts give identical state. |
| [61. Many residual streams](ch061-many-residual-streams.md) — runnable: CPU wide hyper-connection equations | Hyper-connections: four parallel residual streams mixed by pre, post and Sinkhorn-normalized 4×4 coefficients (20 iterations), with the collapse that reads the previous sub-block's mix, which must cross any stage boundary. |
| [62. Memory by lookup](ch062-memory-by-lookup.md) — runnable: original token map, exact hashes and bounded file rows | Engram: hashed n-gram tables (up to 4-grams over a normalized, compressed vocabulary) with prime bucket layouts and the reference's fixed per-layer multipliers reproduced exactly; tables of about 101 GB each read a few rows per token from mapped files or disk, never loaded whole. |
| [63. V4.1's expert weights](ch063-v41s-expert-weights.md) — runnable: CPU wide equations and original MoE component | 384 routed experts in low-nibble-first FP4 E2M1 with separate UE8M0 scales per row and 32 K elements, plus one shared FP8 E4M3 expert with 32×32 scales, as confirmed by the pinned source and raw header. A 0xFF scale is NaN. Clamped SwiGLU and route weights before the down projection. Exact decode vectors for every code and scale. |
| [64. Hot experts in fast memory](ch064-hot-experts-in-fast-memory.md) — runnable: exact CPU cache core; device integration open | An exact expert cache: residency seeded from a recorded routing profile, decayed-frequency heat with a swap threshold, copies between steps, misses served by a host lane or from mapped files, and next-layer gate prefetch. The cache changes only where an expert runs, never its result. |
| [65. Prefill layer by layer](ch065-prefill-layer-by-layer.md) — runnable: CPU chunks, original MoE batch and exact CUDA products | Layer-major chunked prefill that reads each expert once per chunk instead of once per token, then grouped block-scaled matrix products; chunk size from a per-step time budget, and a check that no configuration silently disables chunking. |
| [66. Pack the cache exactly](ch066-pack-the-cache-exactly.md) — runnable: exact CPU BF16 grid codec and cache/work inventory | Window KV in FP8 and compressed and index keys in FP4 where the reference already rounds to those grids, bit-checked against it; the memory of step buffers, which scale with chunk size, budgeted separately from the cache. |
| 67. A decode step with no host waits | The whole decode step as one recorded device command sequence: expert ids that stay on the device, sampling on the device so a stage returns a token rather than logits, and a host that schedules step N+1 while step N runs, with buffer lifetimes that allow speculative lengths to be learned a step late. |
| 68. Execute DeepSeek V4.1 Flash on one machine | The complete model, 510.3 GB of originals: one large GPU or a large-memory Mac with experts from the cache of chapter 64 and Engram from disk, qualified under chapter 13 against a reference capture and measured for time to first token, tokens per second and memory. |
| 68a. Maya's recipe: V4.1 Flash on one card | [Project Maya](https://github.com/mw00/project-maya) (MIT) serves V4.1 Flash from one PC with a hot-expert cache and 2–3-bit experts. The pre-reset Rust stage did the same on one RTX PRO 6000 (96 GB, 176 GiB of RAM, a rented `g4-standard-48`), not on an RTX 4080. Rebuild that box in Coax from chapters 62, 64, 65 and 66, plus routed experts requantized to about 2.6 bits a weight (178 GB) by importance-weighted GPTQ, the exact skip of layers 21–39 for all but the last 19 × 127 + 1 = 2,414 prompt tokens, and a per-machine calibration of cache size and miss lane. The quant is a separate `consumer-q` profile: never qualified, never in a qualified group, scored by its own card against the reference (top-1 agreement, KL, G2 answers). Targets from the old run: 24.5 tokens/s decode, 91% top-1 and mean KL 0.117; Maya's own engine on the same machine reached 35 tokens/s, 54 with drafts. Then the same build on a 16 GB card such as the 4080: what fits beside its host RAM, the decode it reaches, or a stated refusal. |
| 69. Verify drafts by expected gain | The checkpoint's own draft layers, fixed-length verification first, then variable-length verification chosen from a trained confidence head and a measured cost curve, reported as committed tokens per unit time rather than projected acceptance. |

The reference implementation's exact arithmetic depended on its hardware and
parallel layout, so qualification compares distributions, not bits; chapter
13 sets the rules. The serving facts DeepSeek has published (prefill and
decode on different groups, expert parallelism only inside a fast domain, a
shared prefix cache of 128-token pages) inform chapters 119, 120, 125 and 126.
DeepSeek's own engine, kernels and libraries are references read as source,
not dependencies. V3/R1-scale checkpoints and V4.1 split into stages run in
Part XX.

## X. Learn with the operations we already own

| Chapter | What we build and check |
| --- | --- |
| [70. Learning with arrays](ch070-learning-with-arrays.md) — runnable: cpu-f64-sgd-v1, simulator only | A small supervised path on one dataset: a line with several inputs, logistic regression, then one hidden layer, each compared with a constant predictor and a handwritten rule, with explicit gradients and SGD. The project predicts whether a request finishes before its deadline from a controlled simulator (prompt length, queue depth, model choice, memory headroom), later from chapter 14's measurements. Feature scaling, grouped train/validation/test splits, regularization, class imbalance and threshold choice are measured, and one case shows the larger model losing. It can follow chapter 3 directly. |
| [71. Gradients as a program](ch071-gradients-as-a-program.md) — runnable: cpu-f64-tape-v1 | A bounded reverse-mode tape, saved values, accumulation and parameter identity; analytic and finite-difference checks. |
| 71a. Make a model fail, then find out why | A working classifier broken on purpose (gradients that accumulate across steps, targets paired with the wrong examples, a loss mask that includes padding, a learning rate too large, initialization that makes hidden units identical, a training-only random operation left on at evaluation), and diagnostics for each: activation distributions, gradient norms, update sizes, train and validation curves, and overfitting one small batch first. Each fault must produce a diagnostic that points at it, and repairing it must restore the expected behavior. |
| 71b. Differentiate any tensor program | General reverse mode over 26a's IR: vector-Jacobian rules per operator, shared subexpressions, a parameter used more than once, unused gradients, custom derivatives, gradient-mode control and saved-tensor lifetimes. 71's tape stays the teaching version; this is the permanent one. An MLP built from registered operators, including a custom one, trains with checked gradients. Framework track. |
| 71c. Transform functions, not just tensors | `grad`, `value_and_grad`, `vjp`, `jvp`, `vmap`, Jacobians, Hessian-vector products and their composition. Per-example gradients and Jacobians are computed without a hand-written loop and checked against explicit calculation; robotics, differentiable physics and batched optimization use them. Framework track. |
| 71d. Choose the next example to learn from | An active-learning loop over an owned pool: a small classifier chooses examples by uncertainty, diversity or a mixture, then an independent checker supplies the requested labels; unqueried labels stay hidden from the selector. Compare with random selection using the same trainer, initial labeled set and total label budget, including the initial labels, over several fixed seeds. Keep held-out groups outside acquisition and tuning; record each query and label, quality versus labels acquired, and acquisition and training cost. Include a confidently wrong region that uncertainty alone can miss. The chapter reports whether choosing better data helps, including a negative result; the selector never supplies its own labels. |
| 72. Learn a physical system | A supervised oscillator or cooling surrogate from our own deterministic solver; held-out parameters, one-step error and repeated rollout stability. |
| 72a. A grid world that learns, plans and recovers | A headless grid or 2D navigation scene with obstacles, goals and resettable state. An exact planner supplies demonstrations for a small policy, which then collects labels on the states it actually visits (DAgger); a second route plans with chapter 72's learned model, executes one action and replans, and shows the optimizer exploiting the model's errors. Whole episodes on held-out maps and parameters, completion, collisions, recovery and computation per decision, with tiny cases checked against exhaustive search. A Verse character that navigates and recovers follows from here. |
| 72b. Recover the physics from the motion | Differentiate through an owned, bounded smooth simulator to infer a spring constant and damping from observed motion, with mass and the other simulator constants fixed. Check parameter gradients against independent finite differences and parameter recovery against known synthetic ground truth, then predict trajectories under held-out initial conditions and forcing. Compare gradient-based fitting with a bounded grid search under a stated compute budget, and measure sensitivity to observation noise and sampling. Include observations that cannot distinguish two parameter choices: report the ambiguity rather than claiming a unique recovery. This learns the simulator's parameters directly, complementing chapter 72's learned surrogate. |
| 73. Bytes and a small language model | A real byte vocabulary, bigram likelihood, fixed licensed splits, named RNG and complete tiny training/generation. |
| 74. Differentiate a decoder | Backward operations for the complete causal attention/FFN path; causal isolation and gradient coverage. |
| 74a. Compile the backward pass | Differentiate the IR itself, plan saved activations, choose recomputation, fuse backward operations and compile the optimizer update. Eager and compiled training agree on loss, gradients and updates over several steps, with memory and time measured. Framework track. |
| 75. Train the whole tiny decoder | A complete small transformer with training, held-out likelihood and generation; keep it as a permanent regression. |
| 75a. Train within a memory budget | Where training memory goes (parameters, gradients, optimizer state, saved activations and workspace, each with its own lifetime), gradient accumulation with correct loss normalization and update timing, activation checkpointing that recomputes chosen blocks, and a precision policy per operation (storage, multiplication, accumulation, gradients, optimizer state) with loss scaling. The tiny decoder trained under a fixed memory cap three ways, with gradients and updates against the reference, random-number replay for recomputed operations, measured savings and extra time, and overflow and underflow fixtures. |
| 75b. Models are compositions of modules | `Module` and `Parameter`, named submodules and parameter trees, persistent and non-persistent buffers, train and eval modes, initialization, freezing and parameter sharing. The gate: someone who has never worked on Psionic builds an MLP, a CNN, a small residual network and a transformer from the documented public API without touching runtime code. Framework track. |
| 75c. One trainer for every model | One model, loss and optimizer interface with epochs and steps, gradient clipping and accumulation, validation, schedulers, callbacks and cancellation; an image classifier and a tiny language model train through the same interface. Framework track. |
| 75d. Change an activation, change the answer | Train a tiny decoder on an owned task with a known shortcut, then capture and replace activations at named layers on paired clean and corrupted inputs. Ablate a candidate feature, patch it from the paired run, and try to remove and restore the shortcut's effect. No-op patches and unrelated-feature controls come first; restoring the original activation must reproduce the original output under the declared profile. Choose sites on development examples, then measure effects and retained task ability on held-out counterfactuals. Reuse appendix D's bounded inspection tools, but judge the explanation by interventions rather than an attention picture; report any failure to isolate the behavior. |

Chapter 72 needs 70–71, not language-model training. It is our first physics
chapter, much earlier than pretrained geometry or weather models. It uses
ordinary supervised learning; 72b differentiates through a smooth solver and
can start after 71 and 72, without navigation or higher-order derivatives.
Higher-order physics-informed derivatives are a separate extension in
appendix C. Active learning in 71d needs only 70–71, not the general framework
of 71b–71c; 75d needs the trained tiny decoder in 75 and bounded activation
capture and replacement. These experiments begin on owned CPU fixtures and
do not require another pretrained model or a GPU.

## XI. Data, tokenizers, state and optimization

| Chapter | What we build and check |
| --- | --- |
| 76. Data and resumable state | Provenance, disjoint splits, document packing, model/optimizer/RNG/data counters and exact resume under a named profile. Splits are made by task, session or time, never by row: repeated episodes, paraphrases and near-duplicates stay in one split, preprocessing is fitted on training data only, and planted leakage (including a feature known only after the outcome) must be detected. |
| 76a. Feed the accelerator | Indexed and streaming datasets, samplers, batching and collation, shuffling, transforms, workers, prefetching, pinned host memory, backpressure and resumable iterator state; a reproducible image and text pipeline, with the GPU time lost to loading measured. Framework track. |
| 76b. A checkpoint holds the whole experiment | A versioned record of parameters, optimizer and scheduler state, RNG, data position, precision policy, compilation identity and provenance. Training interrupted at a chosen step resumes and reproduces the declared continuation profile; 129c restores it into a different shard layout. Framework track. |
| 77. Learn a tokenizer | Deterministic byte-BPE training in Coax with explicit tie rules and a pinned pretokenization pattern (nanochat's two-digit number groups); the trained table round-trips through chapter 5's encoder, and an independent oracle reproduces the merges. |
| 78. AdamW, then Muon | Independent update derivations, parameter groups, precision and the exact pinned nanochat optimizer behavior. |
| 78a. Optimizers are reusable components | One optimizer interface for SGD with momentum, AdamW and Muon, schedulers, parameter groups, weight decay and complete state serialization; optimizers swap on an unchanged model, each update checked against an independent calculation. Framework track. |

Data admission begins with the first trainer. These chapters consolidate it
before longer runs rather than postponing provenance until this part.

## XII. Rebuild nanochat, then extend the training pipeline

| Chapter | What we build and check |
| --- | --- |
| 79. Execute nanochat exactly | Pinned BPE/conversation rules, inert checkpoint export, every enabled forward feature and cached inference. |
| 80. Differentiate nanochat | Gradients for every enabled feature; tiny independent comparisons and complete parameter coverage. |
| 81. A complete pretraining run | Tokenizer training from 77, initialization, schedules, accumulation, token budgets, packing, resume and held-out loss/bits-per-byte; tiny first, measured scale later. The same budget spent two ways (a smaller model trained longer against a larger one trained for fewer steps), including the deployed inference cost. |
| 81a. Train a whole model on one GPU | GPU forward and backward, gradient accumulation and optimizer updates on device, BF16 and FP32 policies, and the backward kernels inference never needed: matrix products, normalization, attention and convolution gradients. An MLP, a CNN and a tiny transformer train on the RTX 4080 with all trainable state on the device between steps, and a 10–50M-parameter decoder runs a complete training job with no PyTorch, no unintended CPU fallback and independently verified gradients. Framework track. |
| 81b. Profile a training step | Operator-level CPU and GPU timing, kernel launches, GPU idle time, transfers, allocation lifetimes, peak activations, backward cost and compile cost; one bottleneck found, fixed and its improvement measured. Framework track. |
| 81c. Train the same model eager and compiled | Whole-step compilation with bounded dynamic batches, fusion, activation liveness and the optimizer step; the same model, data order and RNG policy compared on gradients, updates, final metrics, memory, compile cost and steady-state speed. Framework track. |
| 82. Supervised conversations and tools | Assistant/tool loss masks and validation; bounded Nemotron-Agentic-v1/OpenCodeReasoning subsets with provenance and base-skill regression. |
| 83. Continued pretraining and task adaptation | A separately specified domain workload, data mixture, retention checks and promotion criteria. This is a Psionic extension, not a stage claimed from the pinned nanochat speedrun. |
| 84. Preferences and reward models | HelpSteer3 pairwise data, ranking/reward losses and an optional separately specified DPO experiment; held-out comparisons and reward-model limitations. |
| 85. Learn from rollouts | The pinned nanochat on-policy, mean-centered-reward recipe; bounded rollouts, independent gradient/reward checks and held-out task results. Rollouts come from chapter 181's environments. |
| 86. Post-train a decision model | Qualified hybrid backward and LoRA adapters (W + (α/r)·BA with W frozen, so only A and B carry gradients and optimizer state while backward still passes through the frozen layers), selected trainable regions, cross-entropy/Brier/abstention, resume and held-out promotion. Promotion passes both of chapter 180's decision and probability gates. A zero adapter reproduces the base model exactly, only the intended parameters change, the adapter records the base-model identity it is compatible with and loads and unloads through the serving path, and task gains and retained abilities are evaluated separately. A quantized-base variant names its full recipe before it is called QLoRA. |
| 87. Distill Mammoth into Elephant | Mammoth's slot distributions as soft targets on a recorded request corpus; KL/cross-entropy over slots, held-out agreement and chapter 12's quality metrics, with Elephant's cost measured against Mammoth's. |

The [nanochat study](../docs/targets/nanochat.md) is the compatibility
contract. Its current speedrun trains a tokenizer and a base model and runs
SFT; RL is a separate program. Chapter 85 depends on SFT and its own reward
contracts, not on completing preference learning or continued pretraining.
Chapters 86–87 need the decision forward path, chapter 12's evaluation and the
shared training machinery, not nanochat RL. A distilled student is a new
model profile with its own evidence; it inherits neither model's results.
No generic PPO, KL penalty or broad RLHF parity is implied.

## XIII. Images become features and latent representations

| Chapter | What we build and check |
| --- | --- |
| 88. Pixels, convolutions and patches | Exact resize/channel/range conventions, convolution, sampling and patch projection with tiny independent fixtures. |
| 89. Elephant and Mammoth see | The pinned Qwen3.5 vision encoder and projector, exact image preprocessing, image token layout and multimodal rotary positions; image-bearing decision requests with their own reference captures. |
| 90. See with C-RADIO | C-RADIOv3-B feature outputs, visual-asset retrieval and a small Psionic-trained head over frozen features. |
| 91. Compress and reconstruct an image | The older Cosmos-0.1-Tokenizer-CI8x8, including wavelets, normalization and decoder; inspect latent/reconstruction numerics. |
| 92. Noise, diffusion and flow | Train a tiny generative example; distinguish objectives, schedules, conditioning and reproducible sampling. |

Chapter 89 extends the priority decision models and needs only 88 and the
Elephant/Mammoth forward path; text-only qualification does not carry over.
Chapter 90 needs the shared forward core and 88; its trained head also needs
70–71. Chapter 91 is an older teaching target, not a promise of Cosmos3 codec
compatibility. Chapter 92 introduces the methods later world/robotics models
need; it does not qualify their architectures.

## XIV. Motion for Verse

| Chapter | What we build and check |
| --- | --- |
| 93. Poses have conventions | Rotation representations, skeleton hierarchy, root coordinates, units, timing and retargeting fixtures. |
| 94. Motion tokens become poses | A tiny codebook/VQ decoder and masked-token generation; known token-to-pose and pose-continuity checks. |
| 95. Execute MotionBricks | One pinned SOMA package: root timing/trajectory, masked motion-token transformer and VQ-VAE decoder, with full trajectory references. |
| 96. Put generated motion in Verse | An explicit skeleton/target interface, bounded generation and animation handoff; a reproducible destination-to-pose scene. |

This is the preferred first major Verse ML demonstration after useful vision
features. It reuses attention, common image/codebook operators where required,
and motion math; it does not depend on speech, DoMINO, weather or nanochat RL.
Unitree G1 is a separate embodiment/profile, not an interchangeable asset.

## XV. Audio becomes words, and words become audio

| Chapter | What we build and check |
| --- | --- |
| 97. Waveforms and spectral features | Sample/rate/channel admission, windowing, tiny DFT → FFT/inverse, spectrogram/mel and exact feature normalization. |
| 98. A speech encoder | FastConformer convolution/attention, subsampling and length/mask propagation, compared against fixed intermediate tensors. |
| 99. Transcribe with Parakeet | parakeet-tdt-0.6b-v3, token-duration-transducer decoding and bounded utterances; WER/CER and language slices. |
| 100. Transcribe while someone speaks | Chunked streaming input (Voxtral-Mini-4B-Realtime, Apache-2.0, is a candidate alongside Parakeet) with carried encoder/decoder state, partial and final hypotheses, measured per-chunk latency against a stated budget, and integration with the voice engine's end-of-turn and barge-in through `verse/platform/voice.h`. |
| 101. Speak the answer | One separately selected and pinned text-to-speech checkpoint (Voxtral-4B-TTS is a candidate, but its CC BY-NC 4.0 weights exclude commercial use): text normalization, acoustic model, vocoder, streamed audio frames into the voice engine's playback path; intelligibility (ASR round trip through 92) and latency evidence. |

This branch starts from shared convolution/attention and spectral operations;
it need not wait for motion. Spectral primitives are shared with physics.
Offline transcription in 99 does not establish streaming latency; 100 does,
for its named host and chunk size. The text-to-speech target still needs a
survey entry with its weights, code and data terms before 101 is selected.
Neither chapter changes the voice engine's consent or floor rules.

## XVI. Fields, geometry and physical prediction

| Chapter | What we build and check |
| --- | --- |
| 102. A spectral neural operator | Tiny DFT/FFT references, complex arithmetic, truncation and spectral layers on a small field; reuse the spectral core from 97. |
| 103. Learn Darcy flow | Reimplement the PhysicsNeMo FNO architecture/recipe on bounded reproducibly generated data; compare against a solver on held-out fields. |
| 104. A field around a shape | Signed distances, point clouds, spatial neighborhood queries, point convolutions and local aggregation with tiny geometric oracles. |
| 104a. Learn a scene from its views | A coordinate network fitting an image, then a signed-distance field, then differentiable ray compositing on tiny fixtures, then a small radiance field trained on rendered views of a synthetic Verse scene and rendered from held-out cameras. Compositing derivatives, held-out image quality, depth where ground truth exists, memory and render time; good novel views are not evidence of an accurate collision surface or a usable game asset. |
| 105. Execute DoMINO | domino_drivaerml, a bounded DrivAerML subset, exact preprocessing, surface/volume outputs and physical error measures. |
| 106. Forecast on a sphere | Optional FourCastNet3 branch: spherical operators, variable/grid conventions, geographic weighting and multi-step forecast evaluation. |

The first physics result is already in 72. FNO adds field learning; DoMINO
adds geometry-specific operations. Chapter 106 is an alternative specialty,
not a successor that must wait for DoMINO. Its field/activation budgets and
ERA5 acquisition/terms need their own qualification. The Darcy recipe is not
a pretrained checkpoint. None of these models replaces Verse's authoritative
simulation without a separate consumer contract.

## XVII. More sequence architectures

| Chapter | What we build and check |
| --- | --- |
| 107. A recurrence you can inspect | Tiny state-space transitions, causal convolution, chunked/whole-sequence versus token-step execution and independent state reset. |
| 108. Execute Nemotron Nano | NVIDIA-Nemotron-3-Nano-4B-BF16: the complete Mamba-2/Transformer hybrid and all recurrent/cache state. |
| 109. Execute Nemotron Lightning | NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16: combine the exact hybrid and MoE profiles; floating point before NVFP4 or speculation. |

This branch can follow dense inference and performance work without waiting
for motion or physics. Qwen Gated DeltaNet support does not establish Mamba-2
support. The router is chapter 42 in Part VII. Active parameter count is not total checkpoint residency. Bolt's
different Gemma-4 architecture is an optional study in appendix B.

## XVIII. World models and embodied policies

| Chapter | What we build and check |
| --- | --- |
| 110. Sequences of observations | Temporal/multimodal representations, a tiny conditioned generation task and codec/scheduler contracts. |
| 111. One complete Cosmos3-Edge path | Qualify one bounded conditioned generation path across its autoregressive/diffusion towers and exact codecs; numerical and physical evaluations stay distinct. |
| 112. Imitation before robotics scale | A tiny policy trained/evaluated in simulation, with action/state normalization and a bounded PhysicalAI SingleArm data exercise. |
| 113. Execute a GR00T policy | GR00T-N1.7-3B as a candidate, subject to its unresolved artifact-license inconsistency; exact VLA/flow-matching action path for one embodiment. |
| 114. Close the simulated control loop | Action chunks, observation timing, resets, held-out task success and failure recovery under the selected embodiment. |

Cosmos needs mature vision, temporal representations and generative methods.
The robotics path needs vision/language, shared training, action conventions
and flow matching; it does not require Cosmos inference first. GR00T is not a
commercial default while its card and included license disagree. Real robot
control would be a separately authorized integration after simulation.

## XIX. Host inference in production

This part turns a qualified model into a service that other people and
agents depend on. It follows the production practice described in
[Inference Engineering](../docs/serving/inference-engineering.md), but
builds the infrastructure in Coax: admission, scheduling, routing, scaling,
rollout and metering decide in Coax, with `#authority` files and
zero-survivor mutation where they decide work, money or policy. The host
does network, process and device effects only, as the
[Coaxial dev server](../../coax/web.md) already does for the website. It
grows Part III's first deployment: chapter 115 generalizes the timing of
14, chapter 116 the server of 16, and chapters 118–124 add what one resident
model serving one decision at a time leaves out.

| Chapter | What we build and check |
| --- | --- |
| [115. Benchmark a service, not a kernel](ch115-benchmark-a-service-not-a-kernel.md) — runnable: bench-us-v1, loopback load generator | Time to first token, inter-token latency, per-user and total tokens per second, and end-to-end versus inference-only time at P50, P90, P95 and P99. A load generator with a named RNG reproduces input/output length distributions, request contents, sampling parameters and jittered arrival patterns; recorded traffic can be replayed as a shadow. A fixed baseline is required, and only one variable changes per run. |
| 116. Serve a model with an honest capability | Shared System One protocol integration, versioned manifests, bounded requests, cancellation and evidence for each model/backend/profile; request and response schemas start in `proto/`. |
| 117. Clients, streams and background jobs | Streamed tokens and audio over a bidirectional connection, session reuse, cancellation that reaches the scheduler, and asynchronous jobs that are acknowledged at once and complete later under their own deadline; an OpenAI-compatible Responses edge is translated into the System One contract, not the other way round. |
| 118. Continuous batching and a fair scheduler | Token-level batching that admits and retires requests every step under a state-memory budget, chunked prefill interleaved with decode, preemption and priority classes. Every request's output must be identical to running it alone (batch invariance), which the fixed reduction order makes testable. |
| 119. Paged state and a tiered prefix cache | Fixed-size pages for KV cache and for hybrid recurrent snapshots; a prefix cache across requests that spills from device to host memory to local disk, with measured hit rate, eviction and time to first token on hits versus misses. Prefixes are found through a content-hashed chain of 128-token pages; pages never depend on a compute machine staying alive, restore overlaps layer by layer with compute, and retention is set per traffic type. Any approximate reconstruction, such as bounded sliding-window replay, is its own profile and never enters an exact cache identity. |
| 120. Route, queue and scale replicas | A cache-aware and adapter-aware router, a bounded priority queue, and a replica autoscaler (minimum and maximum replicas, window, scale-down delay, concurrency target, scale to zero). The policies are pure Coax, checked against a deterministic discrete-event simulation of recorded and synthetic traffic before any machine runs them. Prefill cost is modelled from uncached tokens and the prefix they attend to, with length buckets and engines that pull work; decode balances both request count and total cache length per replica. |
| 121. Start fast | Cold start split into machine, process, weights and kernel startup and measured separately; content-addressed weight delivery from chapter 4's identities, mapped loading, and generated-kernel caches keyed by device, compiler and source identity. |
| 122. Ship without downtime | Canary rollout with gradual traffic ramps and automatic revert, driven by both latency objectives and golden requests whose outputs must match retained evidence; draining in-flight requests before a replica stops. |
| 123. Observe, meter and price | Volume, input/output lengths, status counts, latency percentiles, replicas, utilization and queue depth; audit logs that record changes and no user content by default; per-request usage records bound to execution evidence and a pricing policy in an `#authority` file. Settlement stays in the Bitcoin wallet, outside Psionic. |
| 124. Survive failures | Health checks, cordoning a failing device or machine, idempotent retries, active-active and active-passive failover between machines or regions, data-residency routing, isolation of tool execution and protection of weights. |
| 125. Disaggregate prefill and decode | Conditional disaggregation: separate prefill and decode workers exchange a complete state snapshot (chapter 8's identity-bound claim, extended for transfer) as a versioned message; a prefill queue, a local-prefill threshold and an xPyD ratio that adjusts to traffic. Only after chapters 115 and 118 measure that prefill interference is the bottleneck. DeepSeek's published layout (small prefill groups, large decode groups, a separate vision encoder role, scaling prefill on unserved tokens and decode on requests in flight) is the reference design. |
| 126. Split a model across devices | Tensor parallelism for latency and expert parallelism for gpt-oss, DeepSeek and other MoE throughput: collectives, sharded weights and state, and results compared with the single-device reference. Expert parallelism stays inside one fast interconnect domain, with data-parallel attention, redundant experts rebalanced from a load window and balance reported as busiest over mean, measured by time rather than token count. |
| 127. A pipeline of models | A compound voice workload (voice activity, transcription, decision model, speech) with each stage scaled separately in one cluster, an end-to-end latency budget per stage and a measured cost for each network hop. |
| 128. A decision model steers voice | Elephant as the voice engine's `decisions:<model>` decider: proposals pass the existing never-awaited gate (clamping, confidence floor, epoch and deadline fences, recorded drop reasons); a slow or absent model can only fail to steer, never stall a frame. |
| 129. Train across devices | One real homogeneous topology: gradient collectives (shared with 119), sharding/resume, numerical comparisons and measured scaling. |
| 129a. A tensor spread over devices | A device mesh, replicated, sharded and partially reduced layouts, collectives, placement propagation and redistribution; one operation runs under several layouts and matches a single-device reference. Framework track. |
| 129b. Data-parallel training | Gradient buckets, all-reduce, accumulation boundaries, communication overlapped with computation, and handling a failed rank; one and two devices compared on gradient semantics, throughput and restart. Framework track. |
| 129c. Fully sharded training | Sharded parameters, gradients and optimizer state, prefetching all-gather and reduce-scatter, and checkpoints restored into a different shard layout; a model whose training state exceeds one GPU's memory trains and resumes. Framework track. |

Serving can begin after the first qualified decision model and an actual
consumer contract. Chapter 128 is the first consumer: it needs 12's published
semantics and 108's measured latency, and the gate stays the authority over
what a decision may change. The more traffic a deployment has, the more of
this part it can justify: disaggregation, multi-device inference and global
caches each wait for a measurement that shows the need, not for a chapter
number. Hosted machines need a qualified Linux target and an owner-approved
budget; buying GPUs, choosing providers and compliance certification are
owner decisions recorded in `NEEDS_OWNER.md`, not chapters. Distributed
training can begin after a correct resumable single-device trainer and the
selected backend.

## XX. Inference across a mesh

A mesh pools memory and compute across machines that are loosely connected,
over ordinary networks rather than device interconnects. Part IV's Pylons
route whole decisions to whole machines; this part splits a model that no
single machine holds. Chapter 126 is the tightly coupled case in one machine
or rack. The design follows the
[Mesh-LLM lessons](../docs/serving/mesh.md) recorded before the reset, with
the trust rules from the earlier private-inference work. Coax decides
placement, admission, routing, verification and price; the host provides
transport, files and devices.

| Chapter | What we build and check |
| --- | --- |
| 130. Split a model into stages | A stage boundary contract derived from the model: a typed list of everything that crosses it (hidden state, residual streams, shared-KV and recurrent sidebands), bound to the artifact, stage range and protocol version. Two stages must match the whole model on identical tokens: logits, greedy output, cached continuation. Dense Ministral 3 first, with raw activations. |
| 131. Stage packages | Each stage fetches and loads only its own tensors and required shared assets, verified chunk by chunk against chapter 4's identities. Fetched bytes, materialized bytes, peak memory and time to readiness are measured separately. |
| 132. Peers, transport and discovery | Encrypted peer connections with the direct or relayed path recorded and measured; private meshes joined by invitation, public ones discovered through Part IV's NIP-PYLON beacons, whose model groups advertise a head and its stages; an owner control plane separate from the inference plane. |
| 133. Place the model | Run whole on one machine if it fits, else route the whole request to a peer, else split. Splitting uses per-layer memory and measured stage and link costs, and states its objective: the sum of stage times for one request's latency, or the slowest stage for throughput. The placement planner is pure Coax, checked in simulation. |
| 134. Stage lifecycle and fencing | Load downstream stages first and publish the entry route only when every stage is ready. A topology generation fences out stale commands, and partial failures withdraw the route and release memory. Each live sequence declares whether it drains, fails or restores. |
| 135. Experts across machines and from disk | Expert parallelism over a mesh with routed tokens and gathered outputs, and expert streaming from local disk for MoE models larger than memory; measured against chapter 53's single-machine results. |
| 136. Route by saved work | Mesh-wide cache-aware routing that compares queue delay plus restore and suffix-prefill cost against cold prefill. Cache advertisements are bounded, short-lived, salted hints that never contain raw tokens; claimed hits that turn out to be missing fall back, and the realized saving is recorded. |
| 137. Compress activations exactly | Codec negotiation with a mandatory raw fallback and round-trip equality of each frame, with the full encode, decode and copy cost measured. Lossy activation formats are a separate profile with their own quality evaluation. |
| 138. Who can read the prompt | Trust levels computed from evidence: a peer serving the request reads it; a hardened process on a machine its owner controls is harder to snoop on, not private; only an attested hardware enclave running a logged release is private. Requests are routed by sensitivity and fail closed rather than falling back. |
| 139. Check a peer's work | Sampled re-execution of a peer's work on a trusted node. Results match exactly only when both sides run the same backend profile, and are otherwise compared within that profile's tolerance. Signed usage receipts per stage, a pricing policy in an `#authority` file, and settlement in Bitcoin through the wallet, outside Psionic. |
| 140. Work with Mesh-LLM | A Psionic server attached to an existing Mesh-LLM mesh through its plugin endpoint interface, serving the OpenAI-compatible and `/systemone` decision contracts so the mesh can route to it. Payments interoperate with its token-metered Lightning exchange (deposit first, then settle from reported usage) through our own wallet. Its code is read as untrusted source; joining a public mesh is an owner decision. |
| 141. DeepSeek V4.1 Flash across stages | Contiguous layer stages on several devices or machines. The boundary carries the four residual streams, the previous mix, source KV and indexer state, chosen candidate blocks and Engram history, in BF16 rather than F32. Generation fencing and stage recovery apply, and every split must match chapter 68's one-machine result on identical tokens, with per-hop latency recorded. |
| 142. Mistral Large 3 across a mesh | Mistral-Large-3-675B (Apache-2.0, 681.5 GB of originals): latent attention and 135-expert routing placed across machines, with parity against the whole-model reference on selected layers and tokens, per-hop latency and per-stage memory. |
| 143. Le Chonk across a mesh | Mistral Large 4 with all of Part VI: 1,060.6 GB of FP8 originals placed across machines, sparse attention and shared layers crossing stage boundaries, the built-in draft head, and recorded parity, latency and cost. Serving it to others is subject to its licence. |

A mesh multiplies failure modes: placement plans, advertisements and
handshakes are not evidence of correct execution. Each split earns its own
parity evidence on the machines that ran it. Joining a public mesh, paying
peers or offering Large 4 as a service each need owner approval.

## XXI. A harness for Psionic models

A harness turns a served model into an agent: it assembles each request, runs
the tools the model asks for, keeps the session's history and decides when a
turn is done. DeepSeek publishes its models, its serving system and
[its harness](../docs/harness.md), and the three are designed together; this
part builds the Psionic versions in Coax, on Part III's API and Part XIX's
server. The default is the simplest loop that works. Every addition (a guard,
a condenser, a plugin) earns its place with a measured gain on tasks it was not
built from, and is removed otherwise.

| Chapter | What we build and check |
| --- | --- |
| 144. The simplest loop | One agent loop over Psionic's API: assemble the request, stream the answer, run the tools it calls, repeat until nothing is owed. Turns and steps have exact boundaries, every turn has a deadline, and cancellation stops the model and the tools together. |
| 145. Model-visible means logged | An append-only session log of durable events (turns, steps, user and assistant messages, tool calls and results); the model's history is derived from the log, so every request can be rebuilt byte for byte and replayed. The log format is versioned and old sessions still open. |
| 146. Keep the prefix stable | Prompt assembly in a fixed order (system sections, tool schemas, then history) so each turn only appends to the model-visible bytes; request metadata stays outside the model's input. Measured against chapter 119's prefix cache: hit rate, prefill saved and time to first token across a long session. |
| 147. Tools as typed capabilities | A tool registry whose schemas come from `proto/`, a guarded pipeline (before, run, after) with structured errors, arguments constrained by chapter 37 so a malformed call is never generated, and filesystem and process tools that share one execution world. |
| 148. A sandbox and approvals | Spawned processes confined to declared roots and resources, an approval policy for anything outside them, per-tool timeouts and a guard against repeating the same failing call. No tool inherits the host's authority by default. |
| 148a. A document tries to become the system prompt | A replayable prompt-injection lab for chapter 38c's documentation assistant and the harness: plant hostile instructions in retrieved passages and tool results, including forged role markers, requests to reveal a synthetic sentinel and attempts to expand a tool grant. Local simulated tools, disposable files and a captured outbound sink make every effect inspectable. Deterministic checks send unauthorized calls directly through the capability gate and require zero unauthorized effects; real model runs report attempted calls, admitted effects, answer leakage and legitimate-task completion separately. Compare paired benign and hostile runs, freezing grants, budgets, model/build identities, seeds, acceptance bounds and held-out attack variants before comparison. Retrieved and tool bytes cannot grant authority, and refusing every task cannot pass the utility gate. |
| 149. Condense the history | Older history replaced by one marked summary when the context fills, bracketed in the log so a crash leaves a visible record; tool results pruned by rule. Each condensation breaks the stable prefix, so it is scheduled to minimize recomputed prefill, and that cost is measured. |
| 150. Programs as tool calls | A model-written program run once against host-provided functions in a bounded, sandboxed runtime, returning one structured result instead of many round trips; output, time and memory limits refuse rather than truncate. |
| 151. Forks, subagents and jobs | A session forked at a turn boundary from its own log, reusing the parent's cached prefix state on the server; subagents with their own scope and budget; background jobs that outlive a turn; a goal that a session works toward across turns. |
| 152. Plugins with owned lifecycles | Composition from ordered profiles; capabilities with a definition, a provider and consumers; every registration owned by the plugin that made it and revoked when it unloads; visible states (installed, enabled, admitted, active, failed, draining). Third-party plugins run in bounded Wasm, distributed as signed Nostr events. |
| 153. Speak the protocols | An MCP client for external tools and resources, ACP for editors, native function tools over the Open Responses API, and System One decisions available to the loop as a tool. |
| 154. Measure the harness | Benchmarks for the harness itself (opening a session, folding a long conversation, reconnecting a stream, continuing an agent) and end-to-end task suites with tokens, cost, cache hits and wall-clock per task, each change measured one at a time against the simplest loop. Results report through chapters 178–180. |
| 154a. Generate candidates, then check them | Three strategies on independently testable Coax tasks under one total budget: one candidate; several independent candidates and a selection; and generate, run, read the failures and repair. Executable checks come first, and a learned verifier only once labels are trustworthy. Reported separately: whether any candidate was correct, whether selection chose one, and whether a verifier accepted a wrong one, with compilation recorded apart from correctness, visible tests for repair and held-out checks for scoring, against tokens, wall-clock time and execution cost. |
| 155. Consent before sharing | No session content leaves the machine unless the person opts in, per session and revocably; attribution metadata is explicit and inspectable. DeepSeek's harness sends session-log suffixes to its API by default; Psionic's never does. |
| 155a. Remember a fact, then revise it | A source-linked memory store for sessions: every remembered fact names where it came from and its project scope, can be corrected or deleted, and is checked by retrieval tests across sessions that a corrected fact never returns in its old form. |

The harness is product logic, so it is Coax, with the host doing processes,
files and network. Coder's own agent loop can consume these chapters; nothing
here replaces its simple loop with more machinery.
Chapter 148a starts after 38c and 144–148; its fixtures and capability gates
stay local, and its behavioral measurements use a pinned Psionic-served model.
Any policy change is checked in Coax before the attacks are run. A clean run
establishes the measured scope, not immunity to every future instruction.

## XXII. Decisions in production

A decision model is only as useful as the thresholds, evidence and checks
around it. These chapters come from the pre-reset work on Jev, System One and
small decision models ([notes](../docs/decisions.md)). Each extends an early
chapter and can start as soon as that chapter exists: 156 after 12, 157 after 12
with synthetic probabilities (live traffic after 16 and 17), 158 after 16 and 17, 159 and 160 after 19 and 20, and 161 to 164 after Part III.

| Chapter | What we build and check |
| --- | --- |
| [156. Fit our own calibration, and refuse a stale one](ch156-fit-our-own-calibration.md) — runnable: cpu-f64-calibrate-v1, synthetic probabilities | Per-question temperatures and binned or isotonic maps fitted on a calibration split and scored on held-out data (reliability curve, expected calibration error, Brier, log loss, confident errors; no bin reported below 20 rows); a map is served only if it passes its frozen gate, and refused for any other model identity or question set. |
| [157. Thresholds from written costs](ch157-thresholds-from-written-costs.md) — runnable: cpu-f64-threshold-v1, synthetic probabilities | A versioned threshold record derived from the cost of a wrong action and the cost of falling through, with three bands (act, abstain to a person or a stronger model, refuse), checked by held-out cost per decision with confidence intervals, never tuned on raw numbers by hand. The first project is an Elephant and Mammoth cascade with three actions (use Elephant's answer, escalate to Mammoth, abstain or ask for more), driven by an explicit table of consequences: each kind of mistake, added latency and inference cost, worked by hand first on synthetic probabilities. It is compared with always Elephant, always Mammoth and a fixed escalation rule, reporting error against coverage, escalation rate, latency and cost; the operating point is chosen on validation data before the confirmation set is opened. Two consumers, a voice interruption and a background classification, rationally choose differently from the same probabilities. |
| [158. Record readings, join outcomes, watch drift](ch158-record-readings-join-outcomes-watch-drift.md) — runnable: cpu-f64-drift-v1, simulated drift | For every decision, the raw and calibrated probabilities, the threshold and the model identity, with no content; each joined to its later outcome or a small judged sample, maps refitted on a schedule, and an alert when calibration error rises past a stated bound. Each record also names the eligible actions, the chosen action, its selection probability and the policy version, its measured cost, and whether the outcome is still pending, so later policies can be evaluated from the log. |
| 159. A Pylon checks itself before serving | A read-only doctor and a self-check against shipped reference decisions that the Pylon must pass before it advertises a slot; an update that would change the served model identity is refused unless declared. |
| [160. How far apart may two honest answers be?](ch160-how-far-apart-may-two-honest-answers-be.md) — runnable: cpu-f64-distance-v1, simulated backends | A distance between two machines' answer distributions (centred log probabilities), measured for honest pairs across backends and for cheap substitutes (a quantized copy, another runtime, a sibling model), with round and strike limits set between them; fresh seeded items with no answer key, the checked subset chosen after answers arrive. |
| 161. Read a bounded ONNX graph | A Coax reader for a fixed opset and a small set of operators, mapping anonymous initializers to named tensors by walking the graph and refusing anything outside the set; first consumers are a static-embedding model and a tiny image classifier. |
| 161a. Write a bounded ONNX graph | Export of 4a's portable inference subset as ONNX, re-read by 161 and by an independent checker, so a model trained in Psionic leaves as an inert artifact. Framework track. |
| 162. A decision in milliseconds | A small encoder with fixed decision heads (a ModernBERT-family model with local and global attention, GeGLU and quantized embedding rows, per-head temperatures) answering a typed question in milliseconds, with identical argmaxes and a stated distance from its reference, and the p50 and p99 latency at 512 tokens on CPU and GPU. |
| 163. Distill Elephant into a small router | An encoder student trained on Elephant's answer distributions over owned synthetic requests, with temperatures fitted on a development split; promoted only above stated agreement with Elephant, calibration error and cheap-answer precision. |
| 164. Shadow a new model, then promote it | A candidate model answering beside the current one on live traffic, both logged; promoted only within stated disagreement limits and with no regression in joined outcomes. Chapter 122 rolls out new builds of one model; this rolls out a different model. Candidates are immutable, activation is atomic by approved digest, and a deliberately worse candidate must leave the active one unchanged. |
| [164a. Learn which model to call](ch164a-learn-which-model-to-call.md) — runnable: cpu-f64-route-v1, simulator only | First with full information: every candidate model runs on the same bounded tasks, results are scored independently, and a small logistic model or MLP learns to choose. Then with partial feedback, where only the chosen model's outcome is seen: a simulated router over three models with different strengths, costs and latency, whose true rewards the simulator knows but the learner does not. Naive, inverse-propensity and doubly robust estimates of a new policy's value are compared with the true value, including an action the old policy never tried, whose value the logs cannot recover. A small model trained in Psionic then improves how OpenAgents uses other models. |

A probability is never permission: the consumer of each answer owns its
threshold, abstention and interpretation, and these chapters make those
choices explicit, versioned and measured.

## XXIII. Programs compiled into weights

A transformer can execute a program exactly when its weights are compiled
rather than trained: attention becomes an exact memory, ReLU gates become
arithmetic and branches, and the model's own output is the program's trace,
so the answer carries its proof and anyone can check it by replay. This part
rebuilds that idea, which the pre-reset work reproduced from Percepta's
published construction ([notes](../docs/exact-computation.md)). It needs only
chapters 1–3 and 6 for 165 to 173; 174 to 176 also need chapter 75's training
and chapter 150's programs as tool calls. A CPU runs these programs orders of
magnitude faster, and every chapter says so beside its own numbers: the point
is computation inside a model that can be verified, not a faster computer.

| Chapter | What we build and check |
| --- | --- |
| [165. A machine whose state is a trace](ch165-a-machine-whose-state-is-a-trace.md) — runnable | A small integer stack machine (constants, locals, add, subtract, multiply, load, store, conditional branch, output, return), a CPU reference interpreter, and a trace format of a few tokens per instruction whose digest identifies the run; anything outside the machine's window is a typed refusal. |
| [166. Attention as exact memory](ch166-attention-as-exact-memory.md) — runnable | Keys placed on a parabola and a hard-max read that returns exactly the matching entry, latest write winning, with cumulative sums from uniform keys; a near miss is a typed refusal, checked against a map oracle on generated read and write streams. |
| [167. Arithmetic and branches from ReLU](ch167-arithmetic-and-branches-from-relu.md) — runnable | Products, step-function conditionals and residual wiring that are exact inside a checked integer range (64-bit integers, then binary64 within 2^53) and refused outside it. |
| [168. Compile a gate graph into a block](ch168-compile-a-gate-graph-into-a-block.md) — runnable | A small gate-graph language and its evaluator, a scheduler that packs gates into layers and reuses residual slots, and weights emitted for an exact block profile: chapter 6's residual stream and gated feed-forward layer without normalization, with ReLU gating, in binary64 (RMSNorm and SiLU cannot be exact, and chapter 172 rules out narrower formats). The block's forward pass must reproduce the evaluator bit for bit. |
| [169. A harness that catches the compiler](ch169-a-harness-that-catches-the-compiler.md) — runnable | A seeded differential run over generated graphs across the evaluator, a plan interpreter and the compiled weights, which catches planted compiler bugs in the families the pre-reset harness found (same-step writes resolved in schedule order: early reuse and same-layer scheduling; accumulation across layers: a missing or wrong-signed clear) and places each in the schedule or the emitter. |
| 170. An interpreter in weights, or one program in weights | A universal interpreter compiled once, against a single program baked into weights (the first Futamura projection, growing with the program's length); sizes and steps per second for both, always beside the native interpreter's. |
| 171. Decode in logarithmic time | A convex-hull lookup that replaces the linear scan over the trace, checked equal to it with comparison counts; wall-clock figures come only from running the weights, never from an interpreter standing in for them. |
| [172. From hard-max to softmax](ch172-from-hard-max-to-softmax.md) — runnable | The analytic bound on how far softmax can stray from hard-max, then real softmax execution under chapter 27's attention profile, measuring the trace length and number format at which exactness breaks. |
| 173. Exact replay as a verification class | Full replay and window spot checks that name the first divergent step, tamper fixtures for each digest, and a verdict (exact replay, failed, unverifiable) used by the mesh's re-execution checks (21, 139) and the GPU kernels' acceptance (29–30). |
| 174. Can gradient descent learn exact execution? | Students trained on verified traces and scored by their first divergent token and by held-out program families at two, four and eight times the training length, never by loss alone; the pre-reset attempts all failed (0 of 2 exact at best), and the chapter reports whatever this one finds. Its training rows are admitted by chapter 186. |
| 175. A frozen exact core with a learned interface | A compiled module behind fixed interface tokens, with a trained controller that prepares its inputs and reads its outputs, judged by replay acceptance on held-out programs: learning does the routing, the compiled core keeps the exactness. |
| 176. Composable weight modules | Two digest-pinned compiled modules linked into one, with composition that must replay exactly, conformance cases, and a label for how a user can check a module: exact replay, deterministic tests, statistical evidence or none. |
| 177. Pay for verified work | A run that pays its contributors for traces a validator has replayed: the worker's capability declared only after a digest-checked self-test, worker and validator on different devices and both paid, receipts before settlement, per-payout and daily caps under owner approval, settled in Bitcoin through the wallet outside Psionic; counts include only real, verified payments. |

Every claim in this part names the path that produced it, reports partial
exactness as partial, uses benchmarks that are what their names say, and ships
the check that would falsify it. Verified traces are also training data whose
labels are provably correct; chapter 174 tests how far that helps.

## XXIV. Measure, then improve

Systems that improve themselves need a measurement they cannot fool. This part
rebuilds the pre-reset Gym as library code, then the loops that use it:
fitting settings on recorded answers, searching policies, reflective prompt
evolution, model judges, and outcomes turned into training rows
([notes](../docs/self-improvement.md)). The prior is sobering: ten pre-reset
prompt-optimization attempts produced no measured win, while fitting settings
on recorded answers and choosing a configuration per task did. Every loop
here keeps its held-out data unread until its choice is written down, never
lets a component grade its own work, and keeps an addition only with a
measured gain over the simplest baseline. 178 to 180 can start right after 12.

The result the book builds toward joins most of its parts: a small Psionic
model that chooses a cheaper inference path (163, 164a), evaluated on
independently checked outcomes (158, 181), served through the API (16) and
promoted only when it improves a quality-and-cost objective written down in
advance (164, 180).

| Chapter | What we build and check |
| --- | --- |
| [178. A suite is three digests](ch178-a-suite-is-three-digests.md) — runnable: sha256-suite-v1, chapter 12's held-out suite | Items with their partitions, the question set and the gate each carry a digest; results go to an append-only store with a receipt chain, and a locked partition can be read once. Rewording a question changes only the question digest; a run over different items is refused as a comparison. |
| [179. How far a metric moves by itself](ch179-how-far-a-metric-moves-by-itself.md) — runnable: cpu-f64-noise-v1, synthetic suite and chapter 12's Elephant run | The noise floor from seed blocks and a task-clustered paired bootstrap, and a headroom check that refuses a partition already at its ceiling; reproduced on an Elephant suite before any optimization claims a gain. |
| [180. Gates that can say unverifiable](ch180-gates-that-can-say-unverifiable.md) — runnable: cpu-f64-verdict-v1, chapters 12, 156, 157 and 179 reporting | Comparison gates as `#authority` Coax: a decision gate requiring a gain beyond the noise floor, a probability gate that ranks log loss and confident errors above calibration error and accuracy, and an evaluation gate where cost and time can block a result but never earn one; verdicts are passed, failed or unverifiable. Chapters 12, 156 and 157 report through them. |
| [181. Environments with checkers](ch181-environments-with-checkers.md) — runnable: cpu-int-env-v1, a simulated agent | A task, a checker, a budget and a replay as one record; the checker must fail on empty work and pass on the reference solution before the environment is admitted; rollouts use Psionic-served models. |
| [182. Fit settings on recorded answers](ch182-fit-settings-on-recorded-answers.md) — runnable: cpu-f64-settings-v1, a simulated stall detector | Thresholds, cut points and weights fitted from answers already recorded, with zero model calls: midpoint candidates, acceptance only when strictly better on the fit set and on grouped cross-validation; the first optimization to try, before any text changes. |
| [183. Search a policy, cheapest first](ch183-search-a-policy-cheapest-first.md) — runnable: cpu-f64-search-v1, a simulated world | Policies as digested manifests, operators named for what they do, successive halving across tiers from replay to full measurement, a per-task Pareto frontier, and the held-out read only after the winner is written down. |
| 184. Reflective prompt evolution | GEPA in Coax: candidates whose fields are fixed by the seed, per-instance Pareto sampling, a reflector served by Psionic that is never the grader, refusal of a malformed reflection, a check that the answer space did not drift, and a canary proving the candidate reached the model. Measured honestly against chapter 182's settings fitting. |
| 185. Judges are components | Elephant or Mammoth inside a search as a pinned judge or router: agreement with real outcomes measured, a grader whose identity differs from the student's and the reflector's, cost per judgment recorded, and a teacher's opinion never stored as a label. |
| 186. Outcomes become training rows | Decision outcomes and verified traces admitted with their replay verdicts; labels from diffs and checks, never from an agent's own summary; an exposure ledger across artifacts so held-out groups are never trained on; splits by time. These rows feed chapters 85 to 87, 163 and 174. |
| 187. Is the loop improving? | Cost per accepted result per cycle on a fixed set, an ablation for every component, a complexity ledger, and shared budgets for proposer, student and grader; a loop that does not improve is reported as such. |
| 188. An agent optimizes a Psionic kernel | First a conventional bounded autotuner over tile and workgroup sizes, vector widths and a few fusion choices; then a harness agent proposing new implementations inside chapter 26's compute subset for a normalization kernel or a small gated feed-forward block. The agent receives the reference operation and its input contract, compilation, correctness results, profiler measurements and a fixed budget; the evaluator and its independent tests are outside its writable scope, with held-out shapes, awkward dimensions, extreme values and memory checks. A winner must keep the declared numerical behavior and improve measured performance on the declared workload, with its source, compiler identity, device, memory and timings kept. The whole search cost, failures included, is reported, and a run that finds nothing is still a result. |
| 189. Can someone use Psionic instead of PyTorch? | An externally consumable package: public APIs, operator documentation, examples, diagnostics and reproducible benchmarks, and a command that audits a proposed model (its operators, dtypes, gradients and backends) before allocating large GPU memory. Three published PyTorch tutorials (a classifier, a convolutional model and a transformer training example) are rebuilt with public APIs only, recording model-specific lines, unsupported operations, runtime modifications, effort, cold-start compile time, throughput, memory and final quality. The gate is zero engine modifications for an ordinary new model; each one needed names a missing public abstraction, which is then added. |

The loops improve the system around the weights first (settings, prompts,
routers, harness configuration); weight updates follow only when both the
decision and probability gates pass, since the pre-reset fine-tuning raised
accuracy while making confidence worse.

## A framework, not only engines

Most of this plan teaches how to run and train particular architectures. To
replace PyTorch inside OpenAgents, Psionic also has to make a new
architecture easy to write. The rows marked "Framework track" above add that.
Their graduation requirement is one sentence:

> A developer defines, trains, fine-tunes, evaluates, saves, exports and
> deploys a new neural network entirely in Coax and Psionic, on CPU and GPU,
> without installing or importing PyTorch.

That is a concrete decision for OpenAgents' own workflows. Replacing PyTorch
as everyone's ecosystem needs far more operators, tools, hardware and
integrations, and is not claimed.

**The tensor IR is the center.** One representation, 26a's, carries a model
from Coax source to eager execution (3a), differentiation (71b, 71c), compiled
forward and backward programs (26b, 74a, 81c), custom operators (26c),
distributed layouts (129a) and exported artifacts (4a, 161a). Because Coax is
compiled and typed, Psionic derives these from the program instead of tracing
it at run time, as PyTorch has to. Every special model implementation should
contribute reusable operators to this path where it can.

**Four acceptance projects** (each reports the quality reached and the
resources used; divergence from a PyTorch reference is measured, not assumed
harmless):

| Project | Path | Gate |
| --- | --- | --- |
| Train from scratch | A small MLP on MNIST, then ResNet-18 on CIFAR-10 (2a–3b, 71b, 75b–75c, 76a, 78a, 81a) | Raw dataset to a saved model that reloads independently |
| A language model without PyTorch | A 10–50M-parameter decoder with tokenization, batched mixed-precision training, checkpoints, compiled backward and inference (74a, 75a, 76b, 81a–81c) | A full RTX 4080 run with tokens per second, peak memory, a loss curve and an independently implemented PyTorch baseline |
| Import, fine-tune, serve | A published model read from inert tensors, LoRA adapters trained in Psionic, the adapter saved and served (4a, 86, 116) | Original model to useful adaptation to deployed inference, with PyTorch needed for nothing |
| Train across GPUs and recover | Replicated, then sharded training, interrupted and restored (76b, 129a–129c) | Verified synchronization, declared numerical consistency, correct resume and measured scaling |

**Order.**
1. **A usable framework:** tensor semantics, modules, general autodiff,
   custom operators, optimizers and a data loader. Graduation: a new MLP and
   CNN train without modifying the runtime.
2. **Competitive single-GPU training:** unified IR and compiled autodiff,
   GPU backward kernels, whole-step training, mixed precision and profiling.
   Graduation: a small language model trains end to end on the 4080 with a
   reproducible PyTorch comparison.
3. **PyTorch leaves OpenAgents:** import and export, LoRA fine-tuning,
   reproducible checkpoints and deployment. Graduation: the selected internal
   training and deployment workflows need no PyTorch installation.
4. **Broader adoption:** distributed tensors, sharded training, public model
   packages and compatibility tests (189).

After the current gpt-oss delivery milestones, this track is proposed to take
priority over adding more large pretrained targets. Executing another
architecture proves compatibility; letting a developer define and train a new
one proves the framework. A Python frontend is out of scope: AGENTS.md keeps
Python to tooling. The stable C ABI and DLPack (4a) let others build one.

## Optional laboratories

| Appendix | Scope and entry point |
| --- | --- |
| A. A small imported decoder | SmolLM2-135M after 4–6; a deliberately non-NVIDIA, smaller-memory tokenizer/checkpoint lab. |
| B. Other assistant architectures | Nemotron-Mini-4B-Instruct after 32, with an inert conversion of its original `.bin`; Bolt-E4B as a distinct Gemma-4-derived profile. Neither is automatic dense-model compatibility. |
| C. Derivatives inside derivatives | Higher-order/input derivatives and a tiny PINN after 71–72; independent derivative and PDE-residual checks. |
| D. Look inside a model | Bounded activation capture at named layers, logit lens over the residual stream, attention and recurrent-state inspection; doubles as the tool for localizing a reference mismatch in 10–11. Chapter 75d extends this instrumentation with checked activation replacement and causal experiments. |

## Prerequisites, not a waiting list

| Path | Prerequisites | First complete payoff |
| --- | --- | --- |
| Urgent decisions | 1–3, then 4–9 | Elephant 10, Mammoth 11, published semantics 12; Metal 26–31 advances alongside. |
| A first model of your own | 1–3 | Deadline prediction 70 → gradients 71 → failure diagnosis 71a; a grid-world policy 72a after 72. |
| Active learning | 70–71 and an owned pool with independently checked labels | Choose examples 71d → compare quality at equal label budgets against random selection, with held-out groups never queried. |
| Causal activation experiments | 75's trained tiny decoder and appendix D's bounded inspection tools, extended with activation replacement | Remove and restore a known shortcut in 75d, with control patches and held-out counterfactuals. |
| A framework, not only engines | 2–3 and 26a; 71 and 75 for training; 30 and 26 for the GPU; 129 for distribution | Tensors and operators 2a–3b → modules, autodiff and custom operators 75b, 71b–71c, 26c → trainer, data, optimizers 75c, 76a, 78a → GPU training and compilation 81a–81c, 26b, 74a → checkpoints and interchange 76b, 4a, 161a → distribution 129a–129c → can someone use it 189. |
| Live decision API | 10–11 and 13; an owner-approved host | Timing 14 → resident model 15 → `/v1/systemone` 16 → running live 17 → measured optimizations 18. |
| First Pylon mesh | Part III's API and `coax/lib/nostr` | A Pylon answering decisions 19 → the API fanning out 20 → receipts and checks 21. |
| Decision throughput | 10 and 22; Coax jobs for the second half of 24 | Byte-identical parallel CPU 24; batched forks and prefix reuse 25. |
| Generation | Shared dense inference and 5's tokenizer tables | Sampling and constrained decoding 37 → continuation 39. |
| Mistral | 4–6, 22, 37 and the router in 42; 47 for drafting | Ministral 3 49 → FP8 50 → Devstral 51 → latent attention 52 → Small 4 53 → Large 4 components 54–55. |
| DeepSeek | 4–6, 13, 22, 37 and the router in 42; latent attention 52 | R1 distill 56 → recipe 57 → V2-Lite 58 → V4.1 components 59–63 → engine techniques 64–67 → V4.1 Flash on one machine 68 → drafts 69; across stages 141. |
| gpt-oss | 4–6, 22 and 37; the router in 42 | MXFP4 41 → 20b 45 → 120b 46 → speculative decoding 47; tool calls 48. |
| Retrieval / dense continuation | Shared storage, artifacts, tokenizer and attention; each exact model profile | 38 → approximate index 38a → reranking 38b → grounded answers 38c / 39; browser search 40 once Coax Wasm/WebGPU compute exists. |
| Training / nanochat | 70–71, 73–78 and exact nanochat forward/backward | Tiny pretraining 81 → SFT 82 → the specific rollout recipe 85. |
| Decision post-training | 10–12, 71 and hybrid backward | Adaptation 86; Mammoth-to-Elephant distillation 87. |
| Early physics | 70–71 and an owned deterministic solver; a differentiable smooth solver for 72b | Surrogate and rollout checks 72 → recover physical parameters from motion 72b, including ambiguous observations; navigation 72a branches independently. |
| Vision | Shared forward core and 88; gradients for a trained head | Elephant/Mammoth images 89; C-RADIO 90; learned image codec 91 is a separate branch. |
| Verse motion | Shared forward core, required codec operators and 93–94 | MotionBricks 95 → Verse 96. |
| Speech | Shared convolution/attention and 97–98 | Parakeet 99 → streaming 100; text-to-speech 101 after its target is selected. |
| Scientific ML | Shared training/spectral core for FNO; geometry operators for DoMINO | Darcy 103 or DoMINO 105; spherical weather 106 branches independently. |
| Hybrid / MoE language | Shared dense inference, state-space 107 and exact profiles; router 42 (Part VII) for Lightning | Nano 108 → Lightning 109. |
| World generation | Shared vision/codec, diffusion/flow 92 and temporal 110 | One Cosmos3-Edge path 111. |
| Robot policies | Shared vision/language, training, flow 92 and imitation/embodiment 112 | GR00T 113 → simulated closed loop 114, conditional on artifact terms. |
| Production hosting | A qualified model, 22's measurements and a consumer contract | Benchmark 115 → server and clients 116–117 → batching and cache 118–119 → routing, scaling, rollout, metering 120–124. |
| Large-scale serving | Measured need from 115 and 118; two or more devices | Disaggregation 125; tensor/expert parallelism 126. |
| Voice / training scale | 12, 115 and the voice gate / a resumable trainer plus backend | Voice pipeline 127 and decision steering 128 / distributed training 129. |
| Mesh inference | 130's two-stage parity, 22's measurements and network transport | Packages and placement 131–134 → experts and routing 135–137 → trust and verification 138–139 → Mesh-LLM interoperation 140 → DeepSeek V4.1 across stages 141 → Mistral Large 3 142 → Le Chonk 143. |
| Harness | Part III's API; chapter 37's constrained decoding; chapter 119's prefix cache for the stable-prefix measurements | The simplest loop 144 → session log 145 → stable prefix 146 → tools, sandbox and condensation 147–149 → programs, forks and plugins 150–152 → protocols, measurement and consent 153–155. |
| Adversarial harness | 38c and 144–148; a pinned served model for behavioral measurements | Hostile retrieved passages and tool results 148a → checked capability boundaries plus measured legitimate-task completion on held-out attacks. |
| Decisions in production | 12 and 16–17 for calibration, thresholds and outcomes; 19–21 for Pylon checks; Part III for the encoder branch | Own calibration 156 → cost thresholds 157 → outcomes and drift 158; Pylon self-check 159 → honest-distance limits 160; ONNX 161 → millisecond encoder 162 → distilled router 163 → shadow promotion 164. |
| Programs compiled into weights | 1–3 and 6; 75 and 150 for the learned chapters; the wallet and Pylon mesh for 177 | Trace machine 165 → exact memory and arithmetic 166–167 → compiler and harness 168–169 → interpreter or baked program, fast decode, softmax limits 170–172 → replay verification 173 → learned execution and a frozen core 174–175 → composable modules 176 → paid verified work 177. |
| Measure, then improve | 12, then 144, 158 and 173 | Suites, noise floors and gates 178–180 → environments, settings fitting and policy search 181–183 → prompt evolution and judges 184–185 → outcomes as rows and loop accounting 186–187. |
| Choosing and improving the system | 12 for synthetic utilities; 16–17 and 158 for logged traffic; 26–30, 150 and 181–183 for the kernel agent | Cascade 157 → learned model choice 164a → promotion 164; candidates and checks 154a; an agent-optimized kernel 188. |

## Coax features each branch needs

Psionic is a consumer of the [Coax roadmap](../../coax/ROADMAP.md), not a
place to work around it. The current compiler (see
[Coax book chapter 21](../../coax/book/ch21-road-ahead.md)) lacks these; a
chapter that needs one names the smallest language change, its real consumer
case and its verification gate, and stays planned until the feature runs.

| Missing in Coax today | Where Psionic needs it | Until then |
| --- | --- | --- |
| Buffer/index/loop/dispatch GPU compute; `coax metal` emits scalar `f32`/`bool` helpers only ([ch14](../../coax/book/ch14-gpu.md)) | 26–31, 30, 40 | CPU references; no hand-written tensor shaders. |
| WGSL emission | 40 | Wasm CPU kernels only. |
| Jobs, threads and a memory model ([ch12](../../coax/book/ch12-jobs.md), stage 5 not started) | 24, 25, 67, 116–120, 130–139 | A host thread pool calling Coax on disjoint ranges, with the replay gate. |
| `f16`/`bf16`, FP8 and FP4 scalar types | 22, 34, 41, 50, 63, every BF16 kernel | Decode from `u8`/`u16` bits, as chapter 4 does. |
| Generics over numeric types ([ch08](../../coax/book/ch08-generics.md)) | Every multi-dtype kernel, 34 | One concrete procedure per dtype. |
| SIMD lanes (`for_simd`) and vector/matrix types ([ch11](../../coax/book/ch11-data-layout.md)) | 22–24, 93–96 | Scalar loops; measure before asking. |
| Pointers, dynamic arrays and allocators ([ch06](../../coax/book/ch06-memory.md)) | 22, 25 caches, 46, 64 and 135 expert paging, 76 | Host-owned views with explicit capacities. |
| A qualified Linux target ([ch18](../../coax/book/ch18-targets.md)) and asynchronous I/O with deadlines and cancellation ([ch23](../../coax/book/ch23-asynchronous-work.md)) | 16–21, 30, 116–129 on hosted machines, 130–143 on mixed peers | Apple CPU and Metal only. |

For the framework track, six of these block everything else: safe
tensor-shaped dynamic storage and views (pointers, dynamic arrays and
allocators); generic numerical procedures and the narrow float types; GPU
matrix instructions, reductions and synchronization; a typed tensor IR the
compiler can see; jobs, asynchronous device effects and buffer lifetimes;
and qualified Linux and accelerator targets. Coax GPU work should be driven
by a training consumer as well as an inference one, or Psionic can tune twenty
architectures for inference without ever gaining a general GPU training
backend.

Stable issue slugs describe existing work; chapter numbers are provisional.
When a branch is selected, split it into concrete issues with the same
code/checks/book/evidence acceptance loop. The remaining outlines stay proposals
until their workloads and acceptance gates are selected.
