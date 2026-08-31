# Plan: Muse Glimmer in Psionic (`crates/psionic-gguf`)

- Class: implementation plan / work packet (draft for owner acceptance)
- Status: draft 2026-08-29; no slices started; no issues filed yet
- Source: the Muser book (https://highperformanceailab.com/muser-book/), read
  in full 2026-08-29, plus the 2026-08-10 Muse benchmarks already retained in
  this repo
- Base at plan authoring: `be6883f017` (`main`, clean)
- Sibling program: [docs/psionic/](../psionic/) (Qwen 3.8 lane; the pattern
  this plan copies)
- Book capture: full text at `/tmp/muser-book/` (print.html + per-chapter
  markdown). Re-fetch with
  `curl https://highperformanceailab.com/muser-book/print.html` if needed.

## 0. What this is

Extend the in-tree inference engine (`crates/psionic-gguf`, today a Qwen 3.8
GGUF + Metal slice) to load and decode **Muse Glimmer** (52 layers, ~30B,
kquant GGUF) so Coder's local lane can run Muse without llama.cpp, Ollama, or
ExecuTorch. The Muser book is the reference: every architecture constant,
kernel contract, and measured tradeoff below is cited to its chapters. We do
not build the disaggregated GB10 lane; see §3.

## 1. What already exists here (measured, not projected)

The 2026-08-10 program under [docs/muse/benchmarks/](benchmarks/) already ran
the exact pinned artifacts on this machine (M5 Max, 128 GB, `Mac17,6`):

| Artifact | Bytes | SHA-256 (prefix) |
| --- | ---: | --- |
| `muse-glimmer-30B-kquant-17gb.gguf` | 16,756,681,056 | `7e9b74b7…` (matches the book's pin) |
| `dflash-kquant.gguf` (drafter) | 1,631,205,312 | `27d9a805…` |
| `tokenizer.json` (meta-models/Muse-Glimmer-30B) | — | `c9dbee66…` |

Measured on this machine, llama.cpp `62bf73d2` build 10349, ctx 32768,
Q8 KV + Flash Attention, temperature 0, seed 42
([benchmarks/2026-08-10-muse-throughput-optimization.md](benchmarks/2026-08-10-muse-throughput-optimization.md),
[benchmarks/2026-08-10-m5-max-http-runtime-matrix.md](benchmarks/2026-08-10-m5-max-http-runtime-matrix.md)):

- Target-only decode: **12.9–13.5 tok/s** typical compact prompts; **19.65–19.67
  tok/s** in the long-context screening runs; 11.7–22.9 across scenario
  ranges.
- Prefill: **380–418 tok/s** (18k-token prompt), Q8 KV ~3.3% better than F16;
  Q4 KV faster prefill, changed completions, so Q8 is the recorded default.
- DFlash (llama.cpp spec decode): slower than target-only at the maximum
  15-token proposal (6–10 tok/s); the useful operating point is `n_max=3`:
  **37.87 tok/s median** on compact chat (+30.8% over matched target-only),
  but it loses on cold long-context turns (1.974 s slower) and cached repeats.
- ExecuTorch/MLX direct runner (Meta's reference path): **23.57 tok/s** solo;
  Meta's DFlash direct-runner number on an averaged prompt set: **50.2 tok/s**.
  Its HTTP server buffered SSE and was rejected for Omega.

So the bar for a psionic Muse lane is: **beat ~19.7 tok/s dense and ~37.9
tok/s drafted on this machine, with exact-token parity vs the pinned
llama.cpp baseline** (see §6 measurement).

## 2. The Muse Glimmer architecture (Muser book Ch 9, verified table)

Every value below comes from the book's Table 9.1 (parsed from GGUF keys,
cross-asserted by the release gate test in the pinned Muser tree). MuseConfig
must fail closed on any missing key.

| Hyperparameter | Value | GGUF key |
| --- | ---: | --- |
| n_layers | 52 | `muse-glimmer.block_count` |
| hidden_dim | 6,656 | `muse-glimmer.embedding_length` |
| n_heads (query) | 32 | `muse-glimmer.attention.head_count` |
| n_kv_heads | 2 | `muse-glimmer.attention.head_count_kv` |
| head_dim | 128 (independent of hidden/n_heads = 208) | `muse-glimmer.attention.key_length` |
| FFN intermediate | 19,968 | `muse-glimmer.feed_forward_length` |
| vocab | 202,048 | `muse-glimmer.vocab_size` |
| sliding_window | 2,048 | `muse-glimmer.attention.sliding_window` |
| context_length | 131,072 | `muse-glimmer.context_length` |
| SWA/full layers | 39 / 13, collar [SWA,SWA,SWA,FULL]×13 | `muse-glimmer.attention.sliding_window_pattern` = 4 |
| rms_eps | 1e-5 | `muse-glimmer.attention.layer_norm_rms_epsilon` |
| post_norm_eps | 1e-8 (NOT in GGUF — graph constant; landmine) | — |
| rope_base_swa | 500,000 | `muse-glimmer.rope.freq_base_swa` |
| logit_scale | 0.196116 (= 1/√26) | `muse-glimmer.logit_scale` |
| final_logit_softcap | 20 (llama.cpp default is 30 — landmine) | `muse-glimmer.final_logit_softcapping` |
| EOS / EOT | 200,001 / 200,008 | `tokenizer.ggml.{eos,eot,eom}_token_id` |

Derived: attn_dim = 32×128 = 4,096 (Q, gate, o_proj live here); kv_dim =
2×128 = 256 (K, V). Embedding and LM head are untied (two 6,656 × 202,048
tensors).

One block (book Figure 9.1), in execution order:

1. RMSNorm `attn_norm`, eps 1e-5
2. Q/K/V/gate projections as one concurrent group of 4 matvecs
   (Q 6656→4096, gate 6656→4096, K 6656→256, V 6656→256)
3. Per-head QK-norm, eps 1e-5 (Q weight ≈ 3.87, K weight 1.0, must be
   constant per tensor — fail closed otherwise)
4. RoPE **only on sliding layers** (NoPE on full layers), theta 500,000,
   GPT-J interleaved pairing
5. Attention over KV cache: scale 1/√128, causal, GQA 32:2; SWA layers see
   last 2,048 tokens (ring)
6. Sigmoid gate: `attn_out ⊙ σ(gate_proj)`, then o_proj 4096→6656
7. RMSNorm `post_attention_norm`, **eps 1e-8**, then residual add
   (sandwich: norm the sub-block output, not the sum)
8. RMSNorm `ffn_norm`, eps 1e-5; SwiGLU gate/up 6656→19968,
   SiLU(gate)⊙up, down 19968→6656
9. RMSNorm `post_ffw_norm`, **eps 1e-8**, then residual add

Full pass: token embed → weightless entry RMSNorm (all-ones weight) → 52
blocks → final RMSNorm (eps 1e-5, fused into the last block's tail) → LM head
→ × 0.196116 → tanh cap at 20 → sample (book Figure 9.2).

KV cost at n_ctx 4096 (matches psionic's current default): full layers hold
4096 rows × 256 × 2 × F16 = 4 MiB/layer; SWA layers are bounded at 2,048
rows = 2 MiB/layer. Whole cache ≈ 13×4 + 39×2 = **130 MiB F16**. Small.

## 3. Hardware assessment (owner question: no GB10, is a 4080 enough?)

**No GB10 needed.** The book's disaggregated lane (Ch 26–32) exists for one
purpose: NVFP4 prefill of ~100k-token prompts on a GX10/GB10 node, handing KV
to the Mac over Handoff V2. It is 7 of 40 chapters. Everything else — Metal
compute model, kquant family, the decode kernel ladder, SWA ring, kvpack,
scheduler, measurement culture — is single-Mac. The book's own lane matrix
shows the NVFP4 native lane was only ~parity with kquant anyway (35.491 vs
35.440 tok/s on M3 Ultra) and its speculative NVFP4 combination was rejected
fail-closed. We lose almost nothing by skipping it.

**What we build instead:** kquant reference lane + local prefill on the Mac +
(optionally) the DFlash speculative lane. That is the book's
"Speculative + reference lock" row minus the disaggregated column.

**The RTX 4080 (16 GB):** the 16.76 GB target does not fit, so full-GPU
decode is out; partial offload would be CPU/PCIe-bound and likely slower than
this Mac. Its real value is prefill (Ch 29 is the CUDA-versus-Metal porting
map): a 4080 as a LAN prefill node for long prompts — a homebrew, single-node
version of Part VI — is possible later and only pays off for prompt
thousands of tokens long. It is not on the critical path. Decision: Mac-only
first; 4080 lane deferred (see S8).

**Expected tok/s (bytes-read-per-token law, book Ch 1):** decode tok/s ≈
effective bandwidth ÷ 16.76 GB read/token.

| Configuration | Basis | Expectation on this lane |
| --- | --- | --- |
| llama.cpp, this machine, dense | measured 2026-08-10 | 12.9–19.7 tok/s |
| ExecuTorch/MLX direct runner | measured 2026-08-10 | 23.6 solo; 50.2 DFlash |
| Psionic first CPU hybrid port | derived | slower than llama.cpp initially; parity gate before Metal |
| Psionic + Metal kquant matvec | derived | target ≥ 19.7 (beat llama.cpp dense); stretch ~24–26 (Muser's M3 Ultra ran 35.4 at ~594 GB/s effective; M5 Max memory class is lower) |
| Psionic + DFlash draft (n_max≈3) | derived from book ratios 1.20–1.24× | target ≥ 30, stretch 37+ (measured llama.cpp operating point) |

The honest gate is not the absolute number; it is **parity-or-better vs the
pinned llama.cpp baseline on the same prompts, with exact-token equality on
the greedy fixture**.

## 4. Target shape

```text
crates/psionic-gguf
  ├── format.rs        GGUF v3 (exists)
  ├── kquant.rs        NEW  Q4_K/Q5_K/Q6_K block dequant + matvec (book Ch 5–6)
  ├── muse.rs          NEW  MuseConfig (fail-closed) + 52-layer graph (book Ch 9)
  ├── swa.rs           NEW  SWA ring KV (book Ch 15, 23)
  ├── dflash.rs        LATER 1.63 GB drafter + accept/verify loop (book Ch 8, 33)
  ├── qwen35.rs        exists, untouched
  ├── metal_*          exists; add kquant matvec kernels (book Ch 6 dispatch table)
  └── tokenizer.rs     exists; Muse specials EOS 200001 / EOT 200008
```

No new crates. No `psionic-serve` import. The graph follows the `qwen35.rs`
leaf pattern: CPU matvec first, Metal wrap behind the existing session
binding, `DecodeState` with a `Swa`/`Full` layer-state split.

## 5. Slices

Each slice names its gate. Evidence lands under `docs/muse/benchmarks/` with
the same receipt discipline as 2026-08-10. OpenAgents issues get filed at
acceptance, one per slice.

- **S0 — kquant reader (CPU).** Q4_K/Q5_K/Q6_K superblock layouts, 6-bit
  scale packing, dequant + CPU matvec; unit tests against hand-built blocks
  from the book's worked examples; verify the 6-bit scale packing trap.
  Done when: round-trip dequant matches reference bits and matvec matches
  f32 reference within the book's error budget.
- **S1 — MuseConfig + admission.** Parse the GGUF keys of §2, fail closed on
  missing keys, non-constant QK-norm, wrong artifact size; assert the pinned
  SHA `7e9b74b7…` and byte size 16,756,681,056 (already on disk here).
  Done when: `psionic inspect` reports the Muse profile and refuses
  everything else.
- **S2 — Dense graph, CPU, embed→LM head.** Port Figure 9.1 with f32 matvec;
  SWA layers read the ring; full layers read the growing cache. Fixture:
  greedy continuation must byte-match the CPU oracle semantics (book Ch 9's
  "bit-level spec" contract) and then the pinned llama.cpp greedy output.
  Done when: exact-token parity on the 2026-08-10 fixed prompts at
  temperature 0, seed 42.
- **S3 — Metal kquant matvec.** Port `kernel_mul_mv_q4_K` / `q5_K` / `q6_K`
  per the book's kernel dispatch table (Ch 6, Appendix B); preserve llama's
  accumulation-order contract (the book pins `dot_q4_k_f32_llama` doc).
  Done when: bit-consistent with the CPU path and ≥19.7 tok/s decode.
- **S4 — SWA ring + kvpack-lite.** Ring semantics from Ch 23 (wrap only from
  the first chunk crossing 2,048; no ring writes during wrapped prefill);
  F16 KV, Q8 KV option (measured default). Done when: 18k-retrieval prompt
  prefill ≥ 380 tok/s and decode holds at S3 rate.
- **S5 — Coder integration.** `openagents inference` and Coder `/load` admit
  the Muse lane exactly as the Qwen lane does today (map.done … gen.done
  ladder, memory JSON). Done when: Coder `/load` + one generation against
  the pinned GGUF with parity markers.
- **S6 — DFlash speculative lane.** Load the 1.63 GB drafter; propose-verify
  loop per Ch 33 §33.2 (rejection-sampling acceptance, exactness preserved);
  tune `n_max` around 3 per the measured operating point. Done when: ≥30
  tok/s on compact chat with 5/5 exact-token reps vs the dense lane.
- **S7 — Evidence pack.** Rerun the 2026-08-10 harness shape against the
  psionic lane: the three fixed prompts + 18k retrieval + compact chat,
  3–5 reps, medians with ranges, machine-readable receipts. Done when:
  `docs/muse/benchmarks/` has the psionic report and the README links it.
- **S8 — Deferred (owner decision later).** 4080 prefill node (CUDA port per
  Ch 29 + KV handoff lite), NVFP4 anything, GB10 disaggregation.

## 6. Measurement and parity contract

- Comparator: pinned llama.cpp `62bf73d25c53…` build 10349, the exact 2026-08-10
  command shape (ctx 32768, one slot, Q8 KV, FA on, temp 0, seed 42).
- Exactness: greedy temperature-0 outputs must match llama.cpp exactly on the
  fixed prompts; any mismatch is a bug first, a "close enough" never (the
  book's evidence culture, Ch 39).
- Speed: 3–5 reps, medians with ranges, `predicted_per_second`-equivalent
  measured inside psionic, not wall-clock-with-warmup.
- Machine facts stay pinned: `Mac17,6`, M5 Max 18-core, 128 GB, macOS 26.4.

## 7. Known landmines (from the book, each with a chapter)

1. Post-norm eps 1e-8 is a llama.cpp graph constant, **not** GGUF metadata —
   it must live in MuseConfig as a named constant (Ch 9 §9.9).
2. RoPE only on SWA layers; full layers are NoPE. Wrong scope = plausible
   garbage (Ch 14).
3. GPT-J interleaved RoPE pairing, not NeoX halves (Ch 14).
4. Softcap order: scale by 1/√26 **then** tanh cap at 20 (Ch 20).
5. QK-norm weights must be constant per tensor; fail closed (Ch 14).
6. head_dim 128 ≠ hidden/n_heads 208; prefer the declared key (Ch 9).
7. kquant accumulation order must match llama's pinned contract or Metal
   diverges bit-wise from CPU (Ch 6).
8. SWA ring wrap: no ring writes during wrapped prefill until the window is
   crossed (Ch 23).
9. DFlash at max proposal length is a measured trap on this Mac (6 tok/s);
   the operating point is n_max≈3 (2026-08-10 evidence).

## 8. Open questions for the owner

1. Accept the no-GB10 scope (§3) and the S0–S7 order?
2. File the OpenAgents issues per slice now, or after S0/S1 land?
3. S6 DFlash: in scope for the first parity wave or strictly after dense
   parity?
4. Is the 4080 lane (S8) worth a spike at all, or parked until the Mac lane
   ships?
