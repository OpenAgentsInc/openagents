# Independent same-Q4 runtime coverage

Status: plan and encoder source only. No new build, model run, GPU run, or artifact generation has started. This checks runtime agreement with the pinned quantized model. It does not claim BF16 artifact quality or change the separate routes-v5/chat artifact work.

## Scope and fixed inputs

Run all 40 existing text requests and all 104 questions. The suite has 20,894 tokens in total, with 155–1,090 tokens per request. Use the exact Stage 6 request bytes, not newly serialized JSON. Preserve numeric spellings and Unicode separators when preparing input metadata. Re-encode with the pinned official encoder and compare every native token ID, question type, question span, option span, and option ID before executing a model. Compare list lengths explicitly; a zip alone is insufficient.

Use GGUF `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822` and the original external head `a010ac04f078e699988e4049cbea5e62c962393f59fec366640b64e8d69a4953`. Bind the file stat to the prior verified receipt, and check it before and after every layer. If that identity cannot be established, require a fresh hash check in a separately scheduled I/O window. Do not trust a size-only match as model identity.

The existing independent reference decodes one GGUF layer into f32, reverses the published value-head permutation, verifies exact norm-weight `-1/+1` reconstruction, and executes pinned Transformers 5.10.2 operators. Its documented adapter substitutes the stored negative-A value for `-exp(A_log)` to avoid a lossy inverse transform. All other operators stay unchanged. Original Torch 2.9.1 and the independent GGUF decoder remain pinned. The external decision head executes on CPU in f32, using the quantized model's decoded output-embedding rows, with complete option-token remapping checks. No BF16 reference head outputs or original-BF16 trunk weights enter the same-Q4 reference. The official external head weights themselves are BF16 and are loaded as f32.

## First close the input-identity gap

`encoder.rs` is a small standalone Rust program linked to the existing compiled `psionic_serve`, `psionic_models`, `serde_json`, and `sha2` artifacts. It calls the production request parser, tokenizer, and public `encode_record`, without loading a trunk or executing a model. It reads exactly the 40 saved request files and writes IDs, spans, types, option IDs, request hashes, and counts. It refuses media, truncation, unexpected counts, and output replacement. The GGUF reader is the production bounded metadata reader; geometric header reads can include adjacent payload bytes, but no weight tensor is decoded.

Build only this helper with pinned rustc 1.97.1 under the build lease, recording source, compiler, library, and binary hashes. Run it under a CPU-only 2 GiB guard, 14 GiB starting availability, 12 GiB floor, 60 seconds, and disk lease. No GPU or quiet lease. Existing 200-record encoder fixtures overlap only 15 of the exact 40 request objects in this suite, so their passing result does not replace this export.

The old Stage 6 receipt explicitly records `native_ids_observed=false`. The new export establishes IDs for the linked source/library snapshot; it does not retroactively observe an old HTTP process. Reuse old responses only after showing that parser, encoder, tokenizer, head, and relevant trunk sources match the exported/native binary snapshot, or explicitly retain that version limitation and collect a new bounded native run later.

## Prefer a streamed MPS-f32 oracle after a CPU cross-check

Keep one decoder layer resident at a time, with exactly the existing independently decoded f32 weights and official operators. Create the layer on meta, assign CPU f32 weights, then move that layer to MPS without changing dtype. Keep the head on CPU. Keep each request's hidden rows in owned CPU storage between layers. For each loaded layer, process each request separately with batch size one, no padding between requests, no cache, its own positions, and its own causal mask. This reuses read-only weights, not model answers or recurrent state. Do not concatenate independent requests into a causal sequence.

The public forward path writes recurrent/conv state only through a non-None cache. Passing `cache_params=None` and `use_cache=False` leaves no carried request state. Compute each request’s masks and rotary embeddings on CPU exactly as in the existing oracle, then copy them to MPS without changing dtype. Verify module parameter/buffer identities around reuse. Do not introduce a padded multi-request batch in this first reference: it would change GEMM shapes and create another numerical comparison to validate.

Before loading real layers, check exact f32 CPU-to-MPS-to-owned-CPU transport for contiguous and noncontiguous views. Preserve source CPU bytes across later device work to detect aliasing. No dtype-changing readback is allowed. For every loaded layer, compare transferred parameter bytes against the CPU f32 hashes, using bounded copies. Read output back with an explicit same-dtype copy, synchronize, and turn it into immutable bytes before constructing the next CPU tensor. Verify both CPU source bytes and device input bytes remain unchanged after each call. Check every output is finite.

At the end of a layer, synchronize, delete all device layer/state/mask/rotary/output references, collect Python garbage, and call `torch.mps.empty_cache()`. Record current and driver allocations before loading the next layer. With no retained MPS state, require the post-cleanup driver allocation to return within 256 MiB of the established post-self-test baseline. A failed cleanup or rising unexplained allocation ends the run; no higher cap or retry is automatic. This only manages the owned Python process's allocator.

### Proposed fixed acceleration screen

First run text-014 alone through all 64 MPS-f32 layers and the CPU-f32 head. Compare with its saved complete CPU-f32 same-GGUF reference, including every layer's shape/finiteness and final hidden rows. Freeze these screening limits before execution:

- Exact token IDs, spans, option order, input immutability, and weight transfer bytes.
- Final hidden relative L2 error at most `1e-4`; report maximum element error and worst-token cosine separately.
- Maximum absolute head-logit difference at most `1e-3`, maximum probability difference at most `1e-4`, and identical top option.

This is a proposed accelerator-validation screen, not a measured result. If it fails, inspect the discrepancy or use the CPU reference; do not loosen it after observing output. A successful head-only comparison cannot satisfy this screen: the entire independently decoded 64-layer trunk must execute. Save all 64 layer hashes and the full final rows for this pilot. Compare final/selected layer tensors with the existing CPU captures. The full suite can save hashes for intermediate layers and final rows only, avoiding roughly 27 GiB of intermediate tensors.

### Memory and runtime bounds

Use a 12 GiB owned-group cap, 24 GiB starting available memory, 12 GiB host floor, 8 GiB MPS allocator limit, two CPU threads, GPU/memory/disk leases, and no quiet lease. Refuse MPS fallback and optional fused CUDA/FLA kernels. Pilot deadline: 300 seconds. Full-suite deadline: 1,800 seconds, only after pilot review and an observed runtime estimate. Disk reservation: 2 GiB with a 25 GiB free-disk floor. Stop only the owned process group on failure.

Largest decoded layer: 1,533,092,736 bytes. A simultaneous CPU and MPS copy is approximately 2.86 GiB. All 40 CPU hidden states total 427,909,120 bytes. At 1,090 tokens, one 24-head f32 attention-score matrix is 114,057,600 bytes; multiple score/probability and FFN intermediates must be allowed. The 256 MB head is loaded after decoder layers are released. Expected group memory is approximately 4–8 GiB, unmeasured for this mode; the 12 GiB external cap and 8 GiB MPS allocator cap remain the limits. Readback hashes and MPS caching are included in the guard, not assumed free.

The existing CPU case took 141.10 seconds: 69.94 seconds decoding, 60.18 seconds forwarding, with 3,163,324,416-byte sampled peak. MPS does not remove decoding or transfer validation. A one-layer-at-a-time full-suite MPS run is plausibly several minutes to 15 minutes, but this is not a measured estimate. Do not authorize the full pass solely from that estimate; use the pilot's decode, transfer-check, forward, readback, and cleanup timings. Layer reuse across requests amortizes decoding from 40 passes to one pass.

## CPU fallback without new arithmetic

If MPS cannot meet its correctness or memory screen, reuse each CPU decoded layer across separate B=1 request calls. Use six fixed cohorts of at most eight records and 4,096 tokens. Each cohort retains the existing 12 GiB/start24/floor12/1,800-second guard. The cohorts contain 3,609; 3,558; 3,802; 3,360; 3,864; and 2,701 tokens, respectively. Decode once per layer per cohort, not once per request. Keep final rows and hashes, not every intermediate tensor.

Linear extrapolation from the measured CPU case gives approximately 65 minutes of forwards plus seven minutes of repeated decode; allow an unmeasured 70–95 minute total. Full-attention cost is not linear, and CPU contention can extend it. A cohort that reaches its fixed deadline stops without widening bounds. One all40 CPU pass would reduce decode overhead further but would need a newly reviewed longer deadline; six bounded cohorts are easier to resume and audit.

## Compare the unchanged native captures

The existing Stage 6 captures include exact request files, staged and fused-field-head raw logits, HTTP answers, all pooled-row bytes, model/head/binary hashes, and configuration. They used Metal, chunk512, fused attention, weight-shape GEMMs, Q4 FFN on demand, sequential DeltaNet, and disabled prefix/batching. Reuse those results when source identity is established; no new native inference is needed just to generate a reference.

The root-approved frozen full-runtime answer gate preserves the previous answer-level numerical contract while changing only the reference model to the same pinned Q4 artifact: all104 top options match and maximum absolute probability difference is at most0.02. Require exact question/option counts, IDs, ordering, types, token IDs/counts, finite logits/probabilities, HTTP confidence/score consistency, and zero cached-input tokens. Record every raw-logit difference as diagnostic evidence. Do not apply the same-pipeline `1e-3` optimization-logit screen to independent CPU-f32 versus native-f16 arithmetic: the existing independent text014 comparison already measures0.001329958 raw-logit difference despite0.000163851 probability difference. The root froze this answer gate before any new results were read.

Compare both staged and fused field heads to the new independent reference. Do not use the existing104/104 head-to-head result as a substitute. No calibration fitting, recipe selection, checkpoint change, or answer cache is involved.

## Remaining coverage limits

Even a passing40/104 text comparison covers the recorded execution configuration, not every mode. Existing prefix and concurrent-batch comparisons establish invariance against native serial execution; retain their separate receipts. Existing image checks compare against the BF16 vision/backbone reference, not an independent same-Q4 image oracle. The CPU importer currently handles text positions only. Independent same-Q4 image coverage would additionally need the exact image projection and MRoPE sequence, with unchanged image preprocessing and public vision checkpoint. The4080 result remains correct pre-upload memory refusal, not27B execution. None of these distinctions should be collapsed into a claim of universal runtime equivalence.
