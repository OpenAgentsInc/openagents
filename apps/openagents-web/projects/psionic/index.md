# Psionic

Machine learning from scratch in Coax, developed with a runnable book. First: native Elephant and Mammoth decision models; then nanochat inference, pretraining and post-training.

An OpenAgents project, open for contributions from people and agents. Home: `psionic/` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 10-28 21:01 CDT from `wss://relay.openagents.com`. Every event below is signed by the project key `npub107nwhymxa6k42xtl25jelpcz4nqk2675vc77der5ewanmj0m60aqvyfe83`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project: `naddr1qqrhqumfdahxjccprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqw7wsqrwrgt` (kind 30621, `d` = `psionic`)
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:7fa6eb9366eead55197f55259f8702acc1656bd4663de6e474cbbb3dc9fbd3fa:psionic"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `psionic`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Psionic: native decision models and learning pipelines in Coax

Build Psionic from scratch with a runnable book and shared Coax libraries. Owner priority: complete native Elephant and Mammoth support early, independent of the pedagogical training order. Then grow nanochat-compatible inference, reproducible pretraining, SFT, reward training, decision-model post-training and explicit distributed execution. M0 is already shipped; future tasks are open.

Specification: `docs/psionic/roadmap.md` in the openagents repository.

Milestones, in order:

- #4458 **Shared numerical and artifact foundations**: 1 of 4 done, 2 ready. Extend the completed scalar trainer with reusable storage, dense operations and pinned model contracts. These foundations serve inference and learning.

- #0709 **Elephant and Mammoth first**: 0 of 11 done, 0 ready. Urgent delivery path: native Coax Qwen3.5 hybrid execution, faithful decision prompts, independent suffix forks and slot readout. Elephant first, Mammoth immediately after; neither waits for autodiff or nanochat training.

- #639b **Useful native device execution**: 0 of 5 done, 0 ready. Develop the missing Coax compute subset and a measured Metal backend alongside the decision engine. Qualify exact original-weight profiles before separate quantized profiles.

- #981a **Reusable learning and nanochat**: 0 of 10 done, 0 ready. Build autodiff and a tiny complete decoder, then reproduce the pinned nanochat architecture, tokenization, optimizer and resumable pretraining in Coax.

- #f842 **Post-training and evaluation**: 0 of 5 done, 0 ready. Add masked SFT, the pinned nanochat reward-training recipe, and decision-model training only after the relevant gradients, data and evaluation paths work.

- #9617 **Serving, multimodality and scale**: 0 of 3 done, 0 ready. Versioned System One integration, later vision, and explicit same-node distributed training. Each capability needs its own end-to-end evidence.


## Activity

- 10-28 20:55 CDT #bbde resolved (Learn y = 2x with the first runnable chapter)

## In progress (0)

## Ready (2)

### #5009 Pin Elephant, Mammoth and nanochat contracts and oracle manifests

status open · ready · size M · labels priority, artifacts · milestone #4458 Shared numerical and artifact foundations

Event: `50093b5061f73a87b0544f562ae29fe47d3241294b3f98dd34cc824ef82e127e`

Start from docs/psionic/model-targets.md and nanochat.md. Freeze exact model revisions, configs, tensor inventories, tokenizer/renderer versions, source/license notices, numerical profiles and small oracle cases. Recheck current metadata rather than repeating the historical free API window or old license label. Do not download multi-GB weights or execute external scripts on this Mac.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Commit machine-readable manifests for both Base Labs revisions and nanochat 92d63d4e8bb4df75c3b71618f31ddde2378b2bcd, with source hashes and required operations.
- Document original BF16 storage, text-first scope, model-specific bounds, reference capture recipe and refusal identities.
- Tiny synthetic fixtures and all metadata checks run offline; artifact publication requirements distinguish metadata from actual license text.

## Files

- `psionic/fixtures/models/`
- `docs/psionic/model-targets.md`

### #0bfb Admit bounded tensor shapes and caller-owned storage

status open · ready · size M · labels priority, foundations, book · milestone #4458 Shared numerical and artifact foundations

Waits on: #bbde Learn y = 2x with the first runnable chapter (resolved)

Event: `0bfbb317eefdda7f9339f9fe5c70720f64b6124c5415a25f91af484d934e64bc`

Implement contiguous f32 metadata with bounded rank, scalar/empty-dimension policy, overflow-safe element/byte counts, explicit scratch and alias rules. Do not extend the scalar trainer's [-16,16] limits to all tensor work; every new kernel needs its own numerical contract. Book chapter 2.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- #test/#forall cover rank/dimension/overflow/capacity boundaries in both debug and release.
- A narrow #authority admission file has zero surviving mutants.
- Native ASan/UBSan exercises real aliases, short storage and failed admission with unchanged output; chapter listings call the library.

## Files

- `psionic/lib/tensor.coax`
- `psionic/tests/`
- `psionic/book/`

## Waiting (35)

### #3047 Build dense CPU operations with independent numerical oracles

status open · size L · labels priority, numerics, book · milestone #4458 Shared numerical and artifact foundations

Waits on: #0bfb Admit bounded tensor shapes and caller-owned storage (open)

Event: `304716874ba9e36aa65a1b54efb07dd7b88c6066326b1727345a299e88a954df`

Add dot, matrix-vector/matrix multiply, add, gather, reduction, stable softmax and normalization as needed by the first complete forward path. Define accumulation order, masks, non-finite behavior, aliasing and justified tolerances. Book chapter 3; retain a simple scalar oracle when optimizing.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Hand-computable fixtures and independent high-precision comparisons cover rectangular shapes and boundary magnitudes.
- All-masked/empty/NaN cases have explicit failure contracts; no silent uniform-probability fallback.
- Native debug/release/sanitizers and source-linked book examples pass through psionic/check.sh.

## Files

- `psionic/lib/ops.coax`
- `psionic/tests/`
- `psionic/book/`

### #cfeb Read sharded safetensors and BF16 in Coax

status open · size L · labels priority, artifacts · milestone #0709 Elephant and Mammoth first

Waits on: #0bfb Admit bounded tensor shapes and caller-owned storage (open); #5009 Pin Elephant, Mammoth and nanochat contracts and oracle manifests (open)

Event: `cfeb62fba861a25c15fb79a1c31a9b7b0df48ffb28fea2e8b62d092c70851d37`

Build a bounded safetensors header/index reader, deterministic BF16 decoding and typed tensor inventory. Native code maps/reads bytes only. Admit Elephant's single file and Mammoth's shards by manifest; refuse before publishing partial model state. Initial f32 reference execution must explicitly account for any BF16 expansion and residency.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent byte vectors cover BF16 signs/subnormals/non-finite representations and safetensors offsets.
- Truncated data, duplicate tensors, inconsistent shard maps, overlapping spans, overflow, unsupported dtype and wrong shapes refuse.
- Authority mutation has zero survivors; native malformed-file/sanitizer tests use small fixtures without downloading weights.

## Files

- `psionic/lib/model-io/`
- `psionic/fixtures/models/`
- `psionic/tests/`

### #eea1 Implement exact Qwen3.5 tokenization for the decision models

status open · size L · labels priority, tokenizer · milestone #0709 Elephant and Mammoth first

Waits on: #5009 Pin Elephant, Mammoth and nanochat contracts and oracle manifests (open)

Event: `eea13005b418feac2963c6edc4dd5a49d5105f7e13fe105cb382490ab38c1c4c`

Implement the pinned tokenizer's byte/pretokenization/BPE/special-token behavior in Coax. Parse its data rather than running Python/Rust tokenizers in production. Share generic components with the later nanochat tokenizer while keeping model-specific rules/version identities separate.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent pinned token-ID fixtures cover UTF-8, whitespace, JSON state, option labels and special-token boundaries.
- Malformed tokenizer metadata, capacity exhaustion and unsupported rules fail explicitly.
- An offline tokenizer test command and chapter/case-study references identify the exact tokenizer hash.

## Files

- `psionic/lib/tokenizer/`
- `psionic/fixtures/tokenizers/`
- `psionic/tests/`

### #7ade Render exact Base Labs state, questions and option slots

status open · size M · labels priority, decision · milestone #0709 Elephant and Mammoth first

Waits on: #eea1 Implement exact Qwen3.5 tokenization for the decision models (open)

Event: `7ade0e7c77c668b4afbbeb3410b5155bf0ed297dade08c1530888cb37688715c`

Port the published patched prompt and chat-template semantics. Render choice, noul and score; preserve option order and exact shared-prefix/suffix token boundaries. Text-only is the first declared profile. Unknown fields/images/unsupported sizes need explicit outcomes, not accidental template coercion.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Compare bytes, token IDs, prefix cut, suffix lengths, answer positions and slot letters with pinned independent fixtures.
- Cover a one-option choice, maximum 255 options, invalid option counts and score levels 2–10.
- Limits use the trained request bounds and actual token counts; no truncation or character-based estimate.

## Files

- `psionic/lib/decision/prompt.coax`
- `psionic/fixtures/decision/`
- `psionic/tests/`

### #007c Implement Qwen3.5 embeddings, projections, normalization and FFN

status open · size L · labels priority, qwen35 · milestone #0709 Elephant and Mammoth first

Waits on: #3047 Build dense CPU operations with independent numerical oracles (open); #cfeb Read sharded safetensors and BF16 in Coax (open)

Event: `007c5ab12f31d9288083420bb8cb283e1d5c69802c6e2946f51faaad980e99f9`

Implement the exact dense forward pieces used by both model configurations, including their normalization/gating/position conventions. Map named tensors from the admitted manifest. This is inference work and must not wait for generic autodiff.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Tiny synthetic layers match independent intermediate tensors under predeclared f32/BF16-reference tolerances.
- Both 9B and 27B dimensions/layouts pass metadata checks; unsupported config variants refuse.
- No tensor arithmetic is delegated to native C/Swift, BLAS or a hidden external engine.

## Files

- `psionic/lib/qwen35/`
- `psionic/fixtures/qwen35/`
- `psionic/tests/`

### #d8bb Implement block-bidirectional Qwen full attention

status open · size L · labels priority, attention · milestone #0709 Elephant and Mammoth first

Waits on: #007c Implement Qwen3.5 embeddings, projections, normalization and FFN (open)

Event: `d8bb1c93fbf5517de03736d217698807683a611070575dda7945a0f36106a7d6`

Implement full-attention layers for the decision profile: prefix tokens can see their prefix block, suffix tokens see prefix plus their own suffix block, and sibling questions remain isolated. This differs from the causal mask required by generative nanochat.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Enumerate tiny masks including padding; prove by perturbation that suffix tokens cannot change prefix outputs or sibling branches.
- Independent GQA/RoPE/QK/output intermediate vectors match the pinned Qwen configuration.
- Full-sequence and cached-prefix execution agree within frozen numerical tolerances.

## Files

- `psionic/lib/qwen35/attention.coax`
- `psionic/fixtures/qwen35/`
- `psionic/tests/`

### #19c4 Implement causal Gated DeltaNet and convolution state

status open · size L · labels priority, qwen35 · milestone #0709 Elephant and Mammoth first

Waits on: #007c Implement Qwen3.5 embeddings, projections, normalization and FFN (open)

Event: `19c46aa421701e4265a97caebd66261cabfab036b6528dd8b73605917b52ca4b`

Implement the Qwen3.5 linear-attention recurrence and its causal convolution/gates, distinct from full-attention KV state. Support Elephant's and Mammoth's different value-head counts. Start with an independently checkable scalar scan.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Per-token recurrent/conv state and outputs match independently captured tiny fixtures.
- Chunked versus unchunked prefill agrees; reset and resumed state have explicit identities and limits.
- All metadata/state bounds are checked and mutated; native sanitizers cover short and aliased buffers.

## Files

- `psionic/lib/qwen35/deltanet.coax`
- `psionic/fixtures/qwen35/`
- `psionic/tests/`

### #4dc8 Fork complete hybrid prefix state without cross-question leakage

status open · size L · labels priority, cache · milestone #0709 Elephant and Mammoth first

Waits on: #d8bb Implement block-bidirectional Qwen full attention (open); #19c4 Implement causal Gated DeltaNet and convolution state (open); #7ade Render exact Base Labs state, questions and option slots (open)

Event: `4dc8793f0eb6c0e9273622724e02c4d56db888bac545196d05694ba4bd4587c4`

Snapshot full-attention KV, DeltaNet recurrence, convolution state, positions and model/tokenizer/profile identity. Begin with bounded sequential forks; batch only after equality is established. A KV-only snapshot is incomplete for this backbone.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Each branch matches an independent full request; question permutation/branch mutation cannot alter another answer.
- Reusing a stale, wrong-model or wrong-prefix snapshot refuses.
- Short/oversized fork storage and cancellation release have native lifetime tests; sequential and future batched profiles remain distinguishable.

## Files

- `psionic/lib/decision/prefix.coax`
- `psionic/tests/`
- `psionic/fixtures/decision/`

### #de9c Implement slot_lm probabilities and typed decision answers

status open · size M · labels priority, decision · milestone #0709 Elephant and Mammoth first

Waits on: #7ade Render exact Base Labs state, questions and option slots (open); #3047 Build dense CPU operations with independent numerical oracles (open)

Event: `de9c95eb8418fdcfc2781eba320d40974db68ffe01b0d6172687cea9811f624d`

Gather the exact option-letter LM-head rows at the pinned answer position and normalize only the admitted option set. Implement choice/noul/score result semantics and calibration only when the pinned artifact actually supplies it. There is no invented classifier head or generated text.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent logits/probabilities fixtures cover ties, one-option behavior, ordering, score expectation and noul P(true).
- Probabilities are finite, bounded and normalized; malformed logits/options fail before answer publication.
- Synthetic readout tests run without a full model and identify readout/profile version.

## Files

- `psionic/lib/decision/readout.coax`
- `psionic/fixtures/decision/`
- `psionic/tests/`

### #b47f Capture independent Elephant and Mammoth reference cases

status open · size L · labels priority, oracle · milestone #0709 Elephant and Mammoth first

Waits on: #5009 Pin Elephant, Mammoth and nanochat contracts and oracle manifests (open)

Event: `b47f5adfb549c604b39b5aaac327b66c4daaf13185ce0784b089f7cb55bd1ba9`

Prepare a reviewed, bounded reference-capture harness for the pinned OneJev plus serve.patch bundle. Do not run downloaded setup scripts on this Mac. Capture in an explicitly admitted isolated environment or consume independently supplied retained fixtures. Reference Python is tooling only, never the production engine.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Retain prompt/token boundaries, representative layer tensors, complete hybrid prefix state, slot logits/probabilities and typed answers for both models.
- Include empty/oversized/multi-question requests, masks, branch independence and trained-length refusal.
- Record exact sources, device/precision, tolerances and capture commands; unavailable full-weight/device evidence stays unverified and concrete owner actions go in NEEDS_OWNER.md.

## Files

- `psionic/tools/reference/`
- `psionic/fixtures/decision/`
- `docs/verification/`

### #261e Run complete Elephant text decisions with the Coax CPU engine

status open · size L · labels priority, elephant, inference · milestone #0709 Elephant and Mammoth first

Waits on: #cfeb Read sharded safetensors and BF16 in Coax (open); #007c Implement Qwen3.5 embeddings, projections, normalization and FFN (open); #d8bb Implement block-bidirectional Qwen full attention (open); #19c4 Implement causal Gated DeltaNet and convolution state (open); #4dc8 Fork complete hybrid prefix state without cross-question leakage (open); #de9c Implement slot_lm probabilities and typed decision answers (open); #b47f Capture independent Elephant and Mammoth reference cases (open)

Event: `261e965cb764528f3f6cfb0d6b17c4db9e879124adce063a3eef11c9368349fc`

Assemble all 32 Elephant layers and the slot readout over original admitted weights. Start with a declared bounded text/request profile. Keep weights and large scratch in resident host storage with Coax-owned admission; do not inherit M0's numeric magnitude cap or imply the full model fits f32-expanded memory from BF16 file size alone.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- A complete pinned real-weight request for each question type matches independent token/intermediate/logit/probability fixtures.
- Two-question prefix forks, wrong artifact identities and over-limit requests have end-to-end checks.
- Record actual persistent/scratch/peak memory and cold/warm latency; report scoped CPU support only when real-weight execution passes. No autodiff/pretraining dependency.

## Files

- `psionic/lib/decision/`
- `psionic/examples/decision/`
- `psionic/tests/`
- `psionic/book/`

### #13d8 Run complete Mammoth immediately after Elephant

status open · size L · labels priority, mammoth, inference · milestone #0709 Elephant and Mammoth first

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open)

Event: `13d85c3dedf6648fab0604e048b60c05cae2dc3b4110fffce9c21cccc635eb59`

Reuse the same hybrid engine for Mammoth's 64 layers and larger dimensions/value-head count. Add an explicit original-weight residency/chunking plan. This is the next decision-model delivery milestone, ahead of nanochat training and generic framework breadth.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- A real pinned Mammoth request for choice/noul/score passes independent full-model numerical/answer gates.
- Exercise its own dimensions, sharded inventory, context/fork limits and memory refusal; Elephant evidence does not qualify Mammoth.
- Retain measured memory/latency and a runnable case-study section; synthetic-only coverage is recorded as partial.

## Files

- `psionic/lib/decision/`
- `psionic/fixtures/models/`
- `psionic/tests/`
- `psionic/book/`

### #b301 Specify the minimum Coax tensor-compute consumer

status open · size M · labels priority, compiler, gpu · milestone #639b Useful native device execution

Waits on: #0bfb Admit bounded tensor shapes and caller-owned storage (open)

Event: `b301cd5dd599ba98c2af43363713d39e8e03f3ffba0c1820e59279cc3703fa84`

Turn one Psionic matmul/reduction into the compiler consumer that the scalar Metal profile cannot express. Define buffer/index/entry/binding/bounded-loop semantics, completion and later workgroup synchronization. Reconcile any existing Coax project work before filing a duplicate compiler task; add a cross-project needs link when that owner exists.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Retain a minimal source/negative-case set and an explicit current-versus-planned feature inventory.
- Name the compiler/spec/conformance changes and the first actual-device acceptance test.
- No handcrafted native arithmetic shader or BLAS fallback is presented as Coax compute support.

## Files

- `coax/spec.md`
- `coax/ROADMAP.md`
- `docs/psionic/architecture.md`
- `psionic/fixtures/compute/`

### #8484 Implement and qualify the first generated Metal tensor kernels

status open · size L · labels priority, gpu · milestone #639b Useful native device execution

Waits on: #b301 Specify the minimum Coax tensor-compute consumer (open); #3047 Build dense CPU operations with independent numerical oracles (open)

Event: `848480ad0bf1f3b313d45dc6da92efc64bb77d0bd9b227cc20a78b65018b9494`

Implement the admitted Coax compute subset plus a thin native buffer/command/completion adapter. Qualify matmul, gather and reductions needed by decision inference. This work can run alongside the CPU decision engine and does not wait for learning chapters.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Compiler negative/conformance and required bootstrap checks pass for any compiler edits.
- Generated Metal compiles and executes on a real device, comparing outputs with independent CPU oracles.
- Native alias/binding/capacity/failure/completion tests pass; report actual transfer and compute costs separately.

## Files

- `coax/src/`
- `coax/lib/compiler/`
- `coax/tests/`
- `psionic/lib/backend/`
- `psionic/platform/`

### #a0bb Qualify original-weight Elephant on Metal

status open · size L · labels priority, elephant, gpu · milestone #639b Useful native device execution

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #8484 Implement and qualify the first generated Metal tensor kernels (open)

Event: `a0bb5c796afb1098bfd02abeb3ed702956ef25048e88a8fd44ccdd1e3f7a26f7`

Move the complete Elephant text decision graph onto generated Coax kernels, preserving masks, recurrent state and readout. Scope supported original-weight precision and hardware by evidence. Refuse insufficient memory rather than silently swapping to another engine.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Run the same real-weight oracle corpus on actual Metal and the frozen CPU reference.
- Record full-graph residency, peak memory, cold/warm end-to-end latency and bounded multi-question behavior.
- Device errors/cancellation/background or completion lifetimes cannot publish successful partial answers.

## Files

- `psionic/lib/backend/`
- `psionic/lib/decision/`
- `psionic/platform/`
- `docs/verification/`

### #07a2 Qualify Mammoth memory and native Metal execution

status open · size L · labels priority, mammoth, gpu · milestone #639b Useful native device execution

Waits on: #13d8 Run complete Mammoth immediately after Elephant (open); #a0bb Qualify original-weight Elephant on Metal (open)

Event: `07a257085e4e15de8c132b0f2aa3e868b8feb548fe8b3325f86fa8cf3c2860ac`

Reuse the qualified device path for Mammoth with its own admission, memory and numerical profile. Decide whole-model residency versus bounded staging from observed limits; no hardware purchase or paid run is authorized by this issue.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Mammoth's own real-weight corpus passes on the named physical target.
- Retain measured persistent/scratch/peak memory and end-to-end latency, including staging if used.
- Insufficient memory and unsupported target/profile fail clearly; Elephant support cannot mark this done.

## Files

- `psionic/lib/backend/`
- `psionic/lib/decision/`
- `docs/verification/`

### #c6a4 Add one measured quantized decision-model profile

status open · size L · labels quantization, decision · milestone #639b Useful native device execution

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #13d8 Run complete Mammoth immediately after Elephant (open)

Event: `c6a4ebb4c839e829fe2bb8c8a7944720b9067cfa88c01f772b6974067b478214`

Choose one published block format from actual memory needs. Implement decoding/quantization in Coax and retain original-weight output as the reference. Converted weights get new identities and a documented license/attribution record before publication.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent block/partial-row/sign/scale vectors and native malformed-input checks pass.
- Teacher-forced numerical error, answer agreement, calibration and memory/latency changes are measured on held-out cases with thresholds frozen first.
- Quantized Elephant and Mammoth are separately qualified; approximation never borrows original-weight profile identity.

## Files

- `psionic/lib/quant/`
- `psionic/fixtures/quant/`
- `psionic/tests/`
- `docs/verification/`

### #4a75 Add bounded reverse-mode differentiation over shared operations

status open · size L · labels training, book · milestone #981a Reusable learning and nanochat

Waits on: #3047 Build dense CPU operations with independent numerical oracles (open)

Event: `4a7538a193936a197fe23e13ae4d7d786799ce5e9ae34e7338dd5b35cb58204c`

Build a bounded tape, parameter/activation identity, gradient accumulation/zeroing/detach and SGD. Reproduce the manual y=2x result before adding new trainers. This branch must not block the urgent forward-only decision engine.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Analytic and finite-difference gradients cover every admitted primitive on tiny smooth cases.
- Tape exhaustion, repeated backward and alias/lifetime rules have explicit tested outcomes and atomic optimizer publication.
- Source-linked learning chapters use actual library operations; M0 remains a regression.

## Files

- `psionic/lib/autodiff/`
- `psionic/lib/optim/`
- `psionic/book/`
- `psionic/tests/`

### #3a0e Train a complete byte-level bigram language model

status open · size M · labels training, book · milestone #981a Reusable learning and nanochat

Waits on: #4a75 Add bounded reverse-mode differentiation over shared operations (open)

Event: `3a0ef5ccdec40be4a86c95c465feb55cf9aac67c064d8c3d306a4648c806fb22`

Add a real 256-byte vocabulary, fixed train/held-out split, cross-entropy and named seeded sampling. Keep data tiny and licensed. This teaches likelihood and data leakage before a transformer.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Held-out loss beats a declared baseline without changing the split or thresholds.
- Independent probability/gradient and RNG replay fixtures pass.
- The runnable chapter trains and generates with the shared library and bounded capacities.

## Files

- `psionic/lib/nn/`
- `psionic/examples/`
- `psionic/book/`
- `psionic/fixtures/data/`

### #e06a Build a tiny complete causal decoder and its backward path

status open · size L · labels training, transformer · milestone #981a Reusable learning and nanochat

Waits on: #3a0e Train a complete byte-level bigram language model (open)

Event: `e06a34ca62fa698b65af433c520456657ba08c0640f82133b9b29af17e52ac4d`

Implement embeddings, causal GQA/RoPE attention, FFN, normalization, logits, complete gradients and bounded KV inference. Reuse math from the decision engine while keeping causal and block-bidirectional profiles explicit.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Full-sequence versus incremental-decode logits agree and future-token perturbation cannot change past outputs.
- All primitive/whole-block gradient checks and a small full training run pass.
- The example executes every block and produces multi-token output; head-only decode does not pass.

## Files

- `psionic/lib/transformer/`
- `psionic/examples/`
- `psionic/book/`
- `psionic/tests/`

### #cc58 Implement pinned nanochat BPE and conversation tokens

status open · size L · labels nanochat, tokenizer · milestone #981a Reusable learning and nanochat

Waits on: #eea1 Implement exact Qwen3.5 tokenization for the decision models (open)

Event: `cc58525959f817bb560451633bb5ab3dfe29a79d4d835db78b62de8164b2cd41`

Reuse the generic tokenizer core but implement nanochat's own split pattern, merge ranks and ordered special tokens. Export tokenizer data into an inspectable inert format; do not load tokenizer.pkl or require rustbpe/tiktoken at runtime. Tokenizer training may begin with a tiny deterministic Coax BPE trainer.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Pinned independent IDs match for Unicode, two-digit numeric splits, document BOS and conversation/tool boundaries.
- Malformed merges/special-ID collisions/capacity failures refuse; byte-level loss accounting has fixtures.
- The chapter distinguishes tokenizer inference compatibility from reproducing upstream tokenizer-training results.

## Files

- `psionic/lib/tokenizer/`
- `psionic/fixtures/nanochat/`
- `psionic/book/`

### #9b3d Execute the pinned nanochat architecture and safe checkpoint export

status open · size L · labels nanochat, inference · milestone #981a Reusable learning and nanochat

Waits on: #e06a Build a tiny complete causal decoder and its backward path (open); #cc58 Implement pinned nanochat BPE and conversation tokens (open); #cfeb Read sharded safetensors and BF16 in Coax (open)

Event: `9b3d84610ed19ffe3c492b16b4f0a7632d8729a99fe876b365eb02184f005d99`

Implement the current pinned gpt.py profile: RoPE convention, QK normalization/scales, ReLU-squared MLP, untied embedding/head, window pattern, value embeddings/gates, residual/x0 scalars, smear and backout. Define a reviewed inert export of tensor/config data from .pt; never make pickle execution a production loader.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent per-block/logit fixtures plus cached-versus-full decoding cover every enabled architecture feature.
- Exact model/config/tokenizer/source identities accompany checkpoint export; unsupported historical defaults refuse or use a separately versioned reader.
- Include a small complete checkpoint and source-linked chapter; generic GPT compatibility cannot mark this done.

## Files

- `psionic/lib/nanochat/`
- `psionic/tools/export/`
- `psionic/fixtures/nanochat/`
- `psionic/book/`

### #039b Implement reproducible dataset packing and checkpoint resume

status open · size L · labels training, data · milestone #981a Reusable learning and nanochat

Waits on: #4a75 Add bounded reverse-mode differentiation over shared operations (open)

Event: `039b00528a28d0aecfea8d7116e76894a6ec3069d1835a9f3d00a57b3d631e93`

Implement deterministic document boundaries, split provenance, shuffle/packing state, RNG counters, optimizer/parameter revisions and atomic checkpoint publication. Native code handles file effects; Coax owns records and admission.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- An interrupted tiny training run resumes to the declared equivalent uninterrupted state/loss stream.
- Wrong model/tokenizer/data/optimizer identity, corrupt/truncated state and stale checkpoint pointers refuse.
- Checkpoint and dataset metadata are inspectable/versioned; no raw ABI structs or arbitrary Python objects.

## Files

- `psionic/lib/data/`
- `psionic/lib/checkpoint/`
- `psionic/tests/`
- `psionic/book/`

### #5d0b Implement checked AdamW and explicit precision state

status open · size M · labels training, optimizer · milestone #981a Reusable learning and nanochat

Waits on: #4a75 Add bounded reverse-mode differentiation over shared operations (open)

Event: `5d0b40cd12030531e2361e14d3911e93555fdd1a69675a19a30ed5bd9c9aedc6`

Add AdamW with independently specified moments, bias correction, decoupled weight decay and parameter groups. Define master/compute precision and finite-update admission before mixed-precision training.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent scalar/matrix one-step and multi-step fixtures cover moments, bias correction, grouping and weight decay order.
- Non-finite gradients, insufficient state, overflow policy and resume have explicit tests.
- Do not claim nanochat optimizer parity until its Muon/grouping task also passes.

## Files

- `psionic/lib/optim/`
- `psionic/tests/`
- `psionic/book/`

### #68b5 Reproduce nanochat MuonAdamW parameter groups and updates

status open · size L · labels nanochat, optimizer · milestone #981a Reusable learning and nanochat

Waits on: #5d0b Implement checked AdamW and explicit precision state (open); #3047 Build dense CPU operations with independent numerical oracles (open)

Event: `68b5ca2a500a51c80b6d8fec9bbefe589aa12ec757df635d7fa4ec886a787860`

Port the pinned combined optimizer's matrix/scalar grouping, momentum, Polar Express iteration coefficients, variance reduction, equilibration/renormalization settings and learning-rate scaling. Support one rank first; keep multi-rank ownership separate.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Frozen independent updates for tall/wide/square matrices and scalar/embedding groups match within declared tolerances.
- Saved optimizer state resumes identically under the named precision profile.
- Any omitted upstream option is explicitly unsupported; plain SGD/AdamW is not called nanochat recipe parity.

## Files

- `psionic/lib/optim/`
- `psionic/fixtures/nanochat/`
- `psionic/tests/`
- `psionic/book/`

### #fd99 Build held-out language and decision evaluation records

status open · size L · labels evaluation, training · milestone #f842 Post-training and evaluation

Waits on: #039b Implement reproducible dataset packing and checkpoint resume (open); #9b3d Execute the pinned nanochat architecture and safe checkpoint export (open); #261e Run complete Elephant text decisions with the Coax CPU engine (open)

Event: `fd995865ecd768e93715c9f899b1583bfa0aea62b69e4f890e7127d031e572cc`

Implement fixed split/decontamination records, language loss/bits-per-byte, deterministic answer checks, and decision accuracy/Brier/calibration/abstention metrics. Keep numerical conformance, model quality and speed as separate results.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Tiny independently scored fixtures establish every metric and aggregation convention.
- Model/tokenizer/data/profile hashes and refusal counts are in the report; missing/failed evidence cannot pass.
- Promotion thresholds and held-out sets freeze before candidate evaluation; repeated test exposure cannot masquerade as fresh confirmation.

## Files

- `psionic/lib/eval/`
- `psionic/fixtures/eval/`
- `psionic/book/`
- `docs/verification/`

### #191b Check gradients for every enabled nanochat model feature

status open · size L · labels nanochat, gradients · milestone #981a Reusable learning and nanochat

Waits on: #9b3d Execute the pinned nanochat architecture and safe checkpoint export (open); #4a75 Add bounded reverse-mode differentiation over shared operations (open)

Event: `191b6171dc65b1d23f7c14f3dee5007e7e3398336fa2f25826691a0e19d4131b`

Extend the tiny causal decoder backward path to the exact pinned nanochat profile: value embeddings/gates, residual/x0 mixing, smear, backout, QK scaling and window semantics. Forward checkpoint compatibility alone does not establish trainability.

Plan: docs/psionic/nanochat.md; reading order: psionic/book/PLAN.md. Product math stays in Coax. Write independent gradient checks before implementation and run psionic/check.sh with the new cases.

## Acceptance

- Analytic and finite-difference tiny-feature and whole-model gradients match independent fixtures, including parameter sharing and masked boundaries.
- An optimizer-independent loss-direction check exercises every enabled parameter family; unsupported gradients refuse.
- Tape/activation capacity and alias failures preserve model state, with native sanitizer cases.

## Files

- `psionic/lib/nanochat/`
- `psionic/fixtures/nanochat/`
- `psionic/tests/`
- `psionic/book/`

### #a25a Train a small nanochat profile end to end in Coax

status open · size L · labels nanochat, pretraining · milestone #981a Reusable learning and nanochat

Waits on: #9b3d Execute the pinned nanochat architecture and safe checkpoint export (open); #039b Implement reproducible dataset packing and checkpoint resume (open); #68b5 Reproduce nanochat MuonAdamW parameter groups and updates (open); #191b Check gradients for every enabled nanochat model feature (open)

Event: `a25aa49889c3ea096485212be5fddc51ba745f6536024320c64fe2fd8ae80efc`

Reproduce a small named single-device subset of the pinned base_train.py recipe with complete gradients, initialization, token budgets, accumulation, schedule and optimizer groups. Start with a tiny admitted corpus. Full upstream speedrun data/compute is a separate measured target, not an implied budget.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent initialization/gradient/update fixtures plus deterministic tiny-run loss and held-out gates pass.
- Resume and exact consumed-token/data-position accounting pass; actual elapsed work and peak memory are reported.
- Architecture, tokenizer, optimizer and precision differences from upstream are explicit; no Python/PyTorch runtime performs product training.

## Files

- `psionic/lib/nanochat/`
- `psionic/lib/train/`
- `psionic/examples/train/`
- `psionic/book/`

### #7c86 Add nanochat-style masked supervised conversation training

status open · size L · labels nanochat, sft · milestone #f842 Post-training and evaluation

Waits on: #a25a Train a small nanochat profile end to end in Coax (open); #fd99 Build held-out language and decision evaluation records (open)

Event: `7c86722263f5547e08be7369cafc36387fe211c00b7f48b56c2dfdc684ab7252`

Implement chat_sft.py's pinned conversation rendering and assistant/tool loss masks, task mixture, packing and evaluation over a small licensed corpus. A separately scoped continued-pretraining stage may be added from a real use; the current speedrun has no distinct mid-training command.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent token/label/loss-mask and gradient fixtures cover user/assistant/tool boundaries.
- A small SFT run has held-out chat/task evaluation and reproducible resume, with a regression check against its base.
- Data provenance, model revisions and promotion/refusal decisions are retained; endpoint plumbing alone does not establish SFT.

## Files

- `psionic/lib/train/`
- `psionic/lib/nanochat/`
- `psionic/fixtures/sft/`
- `psionic/book/`

### #72d9 Implement the pinned nanochat reward-training recipe

status open · size L · labels nanochat, rl · milestone #f842 Post-training and evaluation

Waits on: #7c86 Add nanochat-style masked supervised conversation training (open); #fd99 Build held-out language and decision evaluation records (open)

Event: `72d999ce70692bd06ac5504608300af89e678250898840242a93f1454b4d4156`

Port the bounded on-policy GSM8K-style recipe from chat_rl.py: group samples, mean-centered rewards, token-level normalization and the exact policy-gradient objective. The pinned source omits a KL trust region and PPO clipping; do not advertise generic GRPO/PPO/RLHF parity. Keep tools and reward execution outside the model's authority.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Deterministic synthetic rollout/reward/advantage/loss/gradient fixtures match an independent oracle.
- Sample/length/compute bounds, data splits, reward provenance and resume are explicit.
- A small held-out improvement/regression report precedes promotion; untrusted generated code is not executed on the owner's Mac.

## Files

- `psionic/lib/rl/`
- `psionic/fixtures/rl/`
- `psionic/book/`

### #1f17 Differentiate the decision backbone and masked slot loss

status open · size L · labels decision, gradients · milestone #f842 Post-training and evaluation

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #4a75 Add bounded reverse-mode differentiation over shared operations (open)

Event: `1f175a13ca2894f30af6fdf37402a023fe108f7ccc9b35cd911692aa78ab8d24`

Add actual gradients for Qwen projections, block-bidirectional full attention, causal DeltaNet/convolution state, prefix forks and slot loss. Reusing a generic causal transformer backward pass is insufficient.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Finite-difference and independent tiny hybrid-block gradients cover prefix/suffix boundaries and shared-prefix accumulation.
- Frozen-base and selected-trainable-region semantics are explicit; unsupported gradients refuse.
- Memory/tape/sequence exhaustion preserves parameters and optimizer state; no large-weight run is required for the first gradient gate.

## Files

- `psionic/lib/qwen35/`
- `psionic/lib/decision/`
- `psionic/tests/`

### #3d9d Fine-tune and calibrate a bounded Elephant decision profile

status open · size L · labels decision, post-training · milestone #f842 Post-training and evaluation

Waits on: #1f17 Differentiate the decision backbone and masked slot loss (open); #5d0b Implement checked AdamW and explicit precision state (open); #039b Implement reproducible dataset packing and checkpoint resume (open); #fd99 Build held-out language and decision evaluation records (open)

Event: `3d9dbed70589d2f34a0c3e3df2905789d1c41d5d14827d4afb519915e1851990`

Start with a bounded selected-parameter or adapter profile and explicitly admitted data. Implement decision cross-entropy/Brier calibration and abstention conventions from the pinned recipe. Extend to Mammoth/full fine-tuning only with separate gradient, memory and held-out evidence. This branch need not wait for nanochat RL.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Tiny exact loss/gradient/update and frozen-base tests pass; checkpoint resume preserves trainable regions.
- A named small run retains train/held-out provenance, calibration/abstention and general-skill regression gates.
- Artifacts bind served base, tokenizer/readout and optimizer revision; unavailable hardware/budget is recorded rather than reporting a completed real-weight fine-tune.

## Files

- `psionic/lib/train/decision/`
- `psionic/fixtures/decision/`
- `psionic/book/`
- `docs/verification/`

### #c7df Expose qualified native decisions through the shared System One boundary

status open · size L · labels serving, decision · milestone #9617 Serving, multimodality and scale

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #13d8 Run complete Mammoth immediately after Elephant (open)

Event: `c7dfed40c630ded170d182fcb7bb95fe527c30c3ee0bb01e735e5d1dc0fb6bec`

Connect the qualified decision engine to the existing shared System One contracts/codecs; inspect coax/lib/systemone and root proto/spec before adding anything. Add bounded ingress, model/profile identity, deadlines and typed refusals. Local library support is distinct from a hosted deployment.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Real choice/noul/score requests round-trip through the selected existing codec and qualified native engine.
- Overload, deadline, wrong profile, cancellation and malformed results never produce successful partial answers.
- Any required public contract change begins in root proto/spec with a new version; no hidden Baseten/OneJev fallback.

## Files

- `psionic/lib/serve/`
- `psionic/platform/`
- `coax/lib/systemone/`
- `proto/`
- `docs/protocol/`

### #04e3 Add separately qualified image inputs for Elephant and Mammoth

status open · size L · labels vision, decision · milestone #9617 Serving, multimodality and scale

Waits on: #13d8 Run complete Mammoth immediately after Elephant (open)

Event: `04e36799dfd6e594270b7de35242d2dafaaaed3e09f9cf0ab7db3b1c8012be93`

Implement the pinned vision preprocessing, encoder/projector, image token layout and memory limits with explicit text-versus-image profiles. The first text-only milestone does not imply image support merely because the checkpoint contains vision tensors.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent image-patch/preprocessing/encoder/token fixtures and a complete multimodal decision pass.
- Malformed/decompression/size/token limits and mixed text/image ordering are tested.
- Device/precision/peak memory and each model's own real-weight image results are retained.

## Files

- `psionic/lib/vision/`
- `psionic/lib/decision/`
- `psionic/fixtures/vision/`
- `psionic/book/`

### #1da2 Add one explicit same-node distributed training topology

status open · size L · labels distributed, training · milestone #9617 Serving, multimodality and scale

Waits on: #a25a Train a small nanochat profile end to end in Coax (open); #039b Implement reproducible dataset packing and checkpoint resume (open); #8484 Implement and qualify the first generated Metal tensor kernels (open)

Event: `1da2196bcc6fc37865df8f6114249a4d94f52547c7963481bd011793bdc999c7`

Choose one actual homogeneous multi-device target and implement data-parallel sharding, collective reductions, deterministic ownership and checkpoint/resume. Add the backend work the selected target needs (for example CUDA); single-device Metal does not establish multi-GPU or multi-node support.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- A tiny one-rank versus multi-rank update is compared under declared reduction tolerances and global token accounting.
- Rank loss, short collectives, stale state and reshard refusal are exercised with retained recovery outcomes.
- Actual target/topology evidence is required; broader distributed optimizer/elastic/WAN work remains separately scoped.

## Files

- `psionic/lib/distributed/`
- `psionic/platform/`
- `psionic/tests/`
- `psionic/book/`

## Done (1)

- #bbde Learn y = 2x with the first runnable chapter
