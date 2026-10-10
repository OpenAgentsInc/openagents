# Clef decision-lane fixtures (#11194)

Public, synthetic correctness fixtures for the native Clef lane
(`crates/psionic-serve/src/clef`). The plan is
[`docs/inference/clef-native.md`](../../../../docs/inference/clef-native.md).
No private or production data is here, and no weights.

| Path | What | Made by |
| --- | --- | --- |
| `encoder/corpus.jsonl` | 200 System One request bodies: unicode, emoji, control characters, number spellings (`1E2`, `-0`, `1e400`, big ints), nested states and instructions, 1–13 questions, 2–60 options | `tools/gen_corpus.py` (seed 20261009) |
| `encoder/expected-{000,100}.jsonl` | Token ids and question/option spans from Cloudflare's `encode_record` with the HF `tokenizer.json` (`Cloudflare/clef-flash`), split in two to stay under 1 MB a file | `tools/ref_encode.py` |
| `head/tiny/` | A tiny random `JointSchemaHead` (hidden 64, width 32, 2+2 layers) in f32 safetensors, inputs and the torch f32 logits | `tools/gen_tiny_head.py` (torch 2.14.1) |
| `e2e/requests.jsonl` | The first 40 corpus records of 1,100 tokens or fewer, with a missing/empty `instructions` spelled as the question id (llama.cpp requires it; the prompt is the same) | `tools/e2e.py select` |
| `e2e/llama-b11538-cpu-q4_k_m.jsonl` | llama.cpp b11538 `llama-server` (CPU, `-ngl 0`) answers | `tools/e2e.py run` |
| `e2e/psionic-cpu-q4_k_m.jsonl` | Psionic CPU lane answers (chunk 128) | `tools/e2e.py run` |
| `e2e/reference-hf-f32.jsonl` | Cloudflare's reference `systemone` with the HF weights in f32 on the CPU, unrounded | `tools/ref_systemone.py answers` |
| `e2e/psionic-cuda-{f16,f32}-q4_k_m.jsonl` | Psionic CUDA lane answers (M2, #11195), f16 and f32 GEMM accumulation, chunk 2048 | `tools/e2e.py run` |
| `e2e/cmp-*.json` | Top-answer agreement and \|Δp\| between them | `tools/e2e.py compare` |
| `reports/layer-parity-2026-10-10.json` | Per-layer residual and final-row cosine / normalized RMSE between llama.cpp b11538 (CUDA), Psionic CPU, Psionic CUDA and the HF f32 reference, on encoder records 7 (155 tokens) and 10 (970 tokens) | `tools/lldump.cpp`, `clef::tests::dump_layer_rows`, `tools/layer_parity.py` |
| `reports/cuda-latency-2026-10-10.json` | RTX 4080 decision latency, Psionic CUDA vs llama.cpp, with the command lines | `tools/bench_latency.py` |

The quantized artifact is `ggml-org/Clef-Flash-GGUF` `Clef-Flash-Q4_K_M.gguf`,
sha256 `fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c`.
The tests that need it read `PSIONIC_CLEF_GGUF`:

```sh
PSIONIC_CLEF_GGUF=/path/Clef-Flash-Q4_K_M.gguf PSIONIC_CLEF_CHUNK_CHECK=1 \
  cargo test --release --manifest-path crates/psionic/Cargo.toml -p psionic-serve --lib clef::
```

`tools/lldump.cpp` dumps llama.cpp's per-layer rows through libllama's
`cb_eval` hook; build it against the b11538 release libraries and headers:
`g++ -O2 -std=c++17 -I<llama.cpp include + ggml/include> lldump.cpp -o lldump -L<release dir> -lllama -lggml -lggml-base`.
The CUDA checks need `PSIONIC_CLEF_CUDA_CHECK=1` (`clef::tests::cuda_chunks_and_cpu_agree`).

The Python tools run in a venv with `torch`, `transformers`, `safetensors`
and `accelerate`, beside a download of `Cloudflare/clef-flash` (read
`joint_schema_model.py` first; the tools import it). Run them with
`python -I`.
