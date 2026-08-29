# Ollama inference pipeline

- Class: reference walkthrough
- Status: describes the Ollama checkout, not OpenAgents code
- Date: 2026-08-29
- Source: `/Users/christopherdavid/work/projects/repos/ollama` at
  `f96e7aa0513b9973a0ccc71be414c2ecb9d65b1a` (`main`, commit message
  `lint fix (#18081)`)
- Related: [QWEN38_INFERENCE_PIPELINE.md](./QWEN38_INFERENCE_PIPELINE.md),
  [LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md),
  [CLI.md](./CLI.md)

This document is how Ollama, in that checkout, takes a model from disk to a
streamed token. It is not a comparison with Psionic. It is not a claim that
OpenAgents already does this in-process.

You talk to Ollama over HTTP. Coder's local lane posts NDJSON to
`POST http://127.0.0.1:11434/api/chat`. That handler is
`server/routes.go` `ChatHandler`. Everything below is what that request
causes.

## The loop in one page

1. Resolve the model name to a local manifest and content-addressed blobs.
2. Choose an engine from the model config: GGUF goes to `llama-server`;
   safetensors goes to the MLX runner.
3. If no runner is already loaded for that model, start a subprocess and
   wait until it reports healthy.
4. Render the chat messages into a prompt string (Go renderer, Go template,
   or llama-server's own chat template).
5. Tokenize the prompt.
6. Prefill: evaluate every prompt token once, filling caches.
7. Decode: for each new token, evaluate one position, sample, detokenize,
   stream.
8. Stop on end-of-sequence, a length budget, a stop string, or client
   cancel.

```text
POST /api/chat
    → resolve ~/.ollama/models/manifests + blobs
    → schedule a runner (reuse or load)
    → GGUF: llama-server subprocess
      or safetensors: ollama runner --mlx-engine
    → render prompt
    → tokenize → prefill → sample decode loop
    → NDJSON ChatResponse lines
```

## Two engines

Ollama does not load weights in the HTTP process for either engine. The
daemon schedules work. A child process owns the tensors.

GGUF (`llama-server`)
: `Model.Config.ModelFormat` is empty or `"gguf"`. `Model.IsMLX()` is
  false. The scheduler calls `llm.LoadModel` to read GGUF *metadata*, then
  `llm.NewLlamaServer`, which starts the bundled `llama-server` binary with
  `--model` pointing at the blob. llama.cpp maps the file, places layers on
  CPU and GPU, and serves `/completion` and `/v1/chat/completions`. The
  bytes of that load and the generate graph are
  [LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md).
  OpenAgents does that work in-process and does not start this child.

Safetensors / MLX (`mlxrunner`)
: `Model.Config.ModelFormat` is `"safetensors"`. `Model.IsMLX()` is true.
  The scheduler calls `mlxrunner.NewClient`. `Client.Load` starts
  `ollama runner --mlx-engine --model <name> --port <p>`. That child loads
  tensor blobs through MLX (`mlx_load_safetensors`), assigns them to a Go
  model (for Qwen 3.5 / 3.8, `x/models/qwen3_5`), and serves a local HTTP
  completion API.

A library tag such as `qwen3.8:27b-mtp-q8_0` is a GGUF package unless its
config blob says `model_format: safetensors`. Format, not the tag string,
selects the engine.

## Where the files live

Default root is `~/.ollama/models`. Override it with `OLLAMA_MODELS`
(`envconfig.Models`).

Manifests
: `manifests/<host>/<namespace>/<name>/<tag>`. JSON. Lists a config digest
  and layers. `manifest.ParseNamedManifest` reads this file.

Blobs
: `blobs/sha256-<64 hex>`. Content-addressed. `manifest.BlobsPath` maps a
  digest to that path.

`GetModel` (`server/images.go`) walks the layers:

| Layer media type | What it becomes |
| --- | --- |
| `application/vnd.ollama.image.model` | `Model.ModelPath` (the GGUF, or the first tensor blob for MLX packages) |
| `application/vnd.ollama.image.template` / `.prompt` | Go chat template |
| `application/vnd.ollama.image.system` | Default system text |
| `application/vnd.ollama.image.params` | Default sampler / context options |
| `application/vnd.ollama.image.adapter` | LoRA path (`--lora`) |
| `application/vnd.ollama.image.projector` | Multimodal projector (`--mmproj`) |
| draft layer | Separate draft GGUF for speculative decoding |

If the model is missing, `parseFromModel` can `PullModel` first. Pull writes
the same manifest-plus-blobs layout. Weights are never in git and never in
the Ollama binary.

The config blob is JSON decoded into `Model.Config`. That is where
`ModelFormat`, `ModelFamily` (for GGUF this is often `qwen35`), `Renderer`,
and `Parser` live.

## From HTTP to a loaded runner

`ChatHandler` does this in order:

1. Bind `api.ChatRequest`. Reject a missing model name.
2. `getModel` → `GetModel`. 404 if the manifest is absent.
3. Empty messages plus `keep_alive: 0` unloads the runner and returns
   `done_reason: unload`. Empty messages otherwise return `done_reason:
   load` after the runner is scheduled.
4. `scheduleRunner` checks capabilities (completion, tools, thinking),
   merges request options with the model's params blob, and asks the
   scheduler for a `LlamaServer`.
5. The scheduler (`server/sched.go`) reuses a loaded runner when the model
   key and options match. Otherwise it calls `Scheduler.load`.

`Scheduler.load` is the engine fork:

```text
if !req.model.IsMLX() {
    f = llm.LoadModel(req.model.ModelPath, 1024)  // GGUF header + KV
    // predict VRAM, pick GPUs, mmap / batch defaults
    llama = NewLlamaServer(..., ModelPath, f, adapters, projectors, ...)
} else {
    llama = mlxrunner.NewClient(modelName, numCtx)
}
llama.Load(...)  // wait until the child is healthy
```

`qwen35` and `qwen35moe` force `num_parallel = 1`. They do not share a
runner across concurrent completions.

Keep-alive defaults to five minutes (`OLLAMA_KEEP_ALIVE`). After expiry the
scheduler unloads the child and the next request loads again.

## Path 1: GGUF and llama-server

This is the path for a typical `qwen3.8:*` GGUF tag, including
`qwen3.8:27b-mtp-q8_0`.

### Metadata only, in Go

`llm.LoadModel` opens the blob and calls `ggml.Decode` (`fs/ggml/ggml.go`).
Decode reads the GGUF magic, key/value metadata, and the tensor *index*. It
does not copy weight payloads into Go. The scheduler uses that metadata to:

- Read architecture (`general.architecture`, for Qwen 3.8 text usually
  `qwen35`).
- Read trained context length and clamp `num_ctx`.
- Estimate VRAM (`llm.PredictServerVRAM`) and decide whether to evict
  another loaded model.
- Detect inline vision tensors (`v.*`) and, for allowlisted arches including
  `qwen35` / `qwen35moe`, pass the same GGUF as `--mmproj`.
- Detect an MTP draft (`hasMTPDraft`) and set `EnableMTP`.

### Start the child

`NewLlamaServerRunner` then `startLlamaServer` (`llm/llama_server.go`)
executes the bundled `llama-server` with a small flag set and lets
llama.cpp auto-detect the rest:

- `--model <blob path>`
- `--host 127.0.0.1 --port <ephemeral> --no-webui --offline`
- `-c <num_ctx * num_parallel>` and `-np <num_parallel>`
- `--lora` / `--mmproj` when those layers exist
- `--cache-type-k` / `--cache-type-v` from `OLLAMA_KV_CACHE_TYPE` when set
- `-ngl` only when the user set `num_gpu` (`0` = CPU only; default `-1`
  omits the flag so llama-server fits layers itself)
- `-t` only when the user set `num_thread`
- `-b` / `-ub` from `num_batch`

Load mode (`appendLoadModeArgs`):

- Default: omit `--load-mode`. llama-server memory-maps the GGUF.
- `use_mmap: false`: `--load-mode none` (copy weights into ordinary
  buffers).
- Linux integrated CUDA or ROCm: `--load-mode dio`.

`Load` on the Go wrapper does not copy tensors. It waits on
`GET /health` until llama-server reports `"status":"ok"` (or `"loading
model"` until ready). llama.cpp, inside that process, maps the file,
registers backends (Metal on Apple Silicon, CUDA, ROCm, Vulkan, CPU), and
places layers. Logs such as `CPU_Mapped` are how Ollama accounts mmap
versus copied VRAM.

Library search: `DYLD_LIBRARY_PATH` / `LD_LIBRARY_PATH` / `PATH` puts
`llama-server`'s directory first (ggml-base, ggml-cpu, libllama), then GPU
backend dirs, and sets `GGML_BACKEND_PATH` to the first `libggml-*` GPU
backend.

llama.cpp itself is not vendored as a tree in the Ollama repo. CMake
fetches the pin in `LLAMA_CPP_VERSION` (this checkout: `b10630`) into the
build and applies `llama/compat/*.patch`.

### Compatibility layer at load

Published Ollama GGUFs often do not match upstream llama.cpp tensor names
and KV keys. `llama/compat/` runs at load inside llama-server:

1. `translate_metadata` rewrites the in-memory `gguf_context` /
   `ggml_context`.
2. `should_skip_tensor` hides vision, projector, or unused MTP tensors from
   the text loader.
3. `maybe_load_text_tensor` applies registered transforms (concat, dtype
   promotion, norm `+1` shifts) because some transforms cannot write into a
   pure mmap view. Those handlers request mmap disablement.
4. Clip / mmproj gets a parallel translation when `--mmproj` is set.

For `qwen35` and `qwen35moe` the handler:

- Fixes Qwen 3.5 / Qwen 3-VL-style text metadata.
- Renames Ollama `mtp.*` tensors onto llama.cpp `nextn.*` names when the
  file is not already native MTP.
- Hides embedded vision and projector tensors from the text graph.
- On the clip side, translates a Qwen3-VL merger-style projector.

Set `OLLAMA_LLAMA_CPP_COMPAT=0` to skip those hooks (used for create-time
validation of already-native files).

### Prompt and generate

After the runner is ready, `ChatHandler` still owns prompt text unless the
model is in *native chat* mode (`chatExecutionModeNative`). For renderer /
parser models, including Qwen 3.8's `qwen3.8` renderer:

1. Merge model-level messages and system text.
2. `chatPrompt` runs the renderer or Go template and tokenizes through
   `r.Tokenize`.
3. `r.Completion` POSTs JSON to `http://127.0.0.1:<port>/completion`.

The completion body includes the prompt string, sampler fields from
`api.Options` (temperature, top_p, top_k, min_p, repeat penalty, seed,
`num_predict`), optional JSON-schema / BNF grammar, and optional media.
llama-server tokenizes again with the GGUF tokenizer, builds the ggml
graph, prefills, then samples.

Native chat models skip the Go prompt and call
`/v1/chat/completions` so llama-server applies `tokenizer.chat_template`
from the GGUF.

Sampling, KV cache, flash attention, and graph execution are llama.cpp's.
Ollama streams SSE from `/completion`, maps chunks to `ChatResponse`, runs
the Qwen 3.5 / 3.8 parser for `<think>` and tool calls when
`Config.Parser` is set, and writes NDJSON to the original `/api/chat`
client.

## Path 2: safetensors and MLX

This path is for packages created or pulled with `model_format: safetensors`.
On macOS it requires Apple Silicon. Linux and Windows use MLX's CUDA path
when those libraries are present.

### Start the child

`mlxrunner.NewClient` reads the manifest and records total tensor bytes
(`TotalTensorSize`). `Client.Load` refuses to start if the first GPU's free
memory, minus overhead, is smaller than that size. Then it runs:

```text
<ollama-executable> runner --mlx-engine --model <short name> --port <p>
```

The child is another copy of the same binary, not llama-server.

### Load tensors

`Runner.Load` (`x/mlxrunner/runner.go`):

1. `model.Open(modelName)` loads the manifest and scans each tensor blob
   header for per-tensor quant type and group size.
2. `base.New(root)` reads `config.json` `architectures` and constructs the
   registered model. Qwen 3.5 / 3.8 text registers as
   `Qwen3_5ForCausalLM`, `Qwen3_5ForConditionalGeneration`,
   `Qwen3NextForCausalLM`, and `Qwen3NextForConditionalGeneration`
   (`x/models/qwen3_5` `init`).
3. `loadTensorsFromManifest` walks every tensor layer, calls `mlx.Load`
   (`mlx_load_safetensors`). On Darwin that load uses the CPU stream so
   file I/O does not land in a Metal command buffer that waits on the
   file. Each blob is a safetensors file; a package is many blobs, often
   one tensor (plus `.scale` / bias) per blob.
4. Rename `.scale` → `*_scale` and quantized `.bias` → `*_qbias` so the
   linear factory can find them.
5. On Metal, `mlx.Eval` every loaded array so data is materialized before
   any weight graph.
6. `m.LoadWeights(tensors)` binds arrays onto Go structs (embeddings,
   per-layer norms, attention or Gated DeltaNet, MLP / MoE, optional
   `lm_head`, optional `mtp.*`).
7. Optional separate draft model, or the inline MTP head (`SelfDraft`).
8. `mlx.Pin` retained arrays, `mlx.Sweep` unused ones, `mlx.Eval` the
   wired set. Tokenizer comes from `tokenizer.json` in the manifest.
9. Build prefix cache, sampler, optional speculation, optional grammar
   engine. Enable MLX compile.

`NewModel` does not read weights. It parses config and tokenizer, then
allocates empty `Layer` shells. `LoadWeights` fills them.

### Qwen 3.5 / 3.8 forward (MLX)

Config fields that matter for the hybrid stack:

- `layer_types` if present: a layer is linear attention unless the type
  string contains `"full"`.
- Else `full_attention_interval` (default 4): layer `i` is full attention
  when `(i+1) % interval == 0`.

Each decoder layer (`Layer.Forward`):

1. RMSNorm on the residual (`input_layernorm`).
2. Either `GatedDeltaNet.Forward` (linear / DeltaNet layers, recurrent
   cache) or full attention with RoPE / M-RoPE and a KV cache.
3. Add residual.
4. RMSNorm (`post_attention_layernorm`) then MLP or MoE.
5. Add residual.

`Model.Forward` embeds token IDs, optionally scatters vision tokens, runs
all layers, then the final RMSNorm. `Unembed` is the LM head (tied
embedding or a separate `lm_head` linear).

Caches: linear layers get `RecurrentCache` (conv tail plus DeltaNet state).
Full-attention layers get `KVCache`.

### Prefill and decode

`TextGenerationPipeline` (`x/mlxrunner/pipeline.go`):

1. `Prepare` encodes the prompt (`tokenizer.Encode`), rejects empty input,
   caps `num_predict` so prompt plus generation stays inside
   `max_position_embeddings`.
2. `prefill` evaluates the prompt in chunks of 2048 tokens, leaving one
   seed token. After each chunk: pin, sweep, materialize caches, optional
   draft commit.
3. Register sampler options for the slot.
4. `decode` pulls sampled token IDs from a pipelined decoder (or the
   speculative decoder when an MTP / draft head loaded), detokenizes, and
   streams `CompletionResponse` chunks. Stop on EOS or `NumPredict`.
   Every 256 generated tokens it clears MLX's free-buffer pool.

The MLX sampler (`x/mlxrunner/sample`) applies temperature, top-k, top-p,
min-p, and repetition / frequency / presence penalties on GPU-resident
logits, then draws a categorical sample.

The HTTP daemon's `ChatHandler` still rendered the prompt in Go first.
Truncate-to-context is disabled for MLX (`truncate = false`).

## Chat rendering for Qwen 3.8

Engine load is independent of chat formatting. Qwen 3.8 packages set
renderer / parser names during create (`x/create/client/create.go`
`qwen35RendererName` → `"qwen3.8"`).

- Renderer: `model/renderers` name `qwen3.8` (Qwen 3.5-family chat
  template, think tags).
- Parser: splits streamed text into content, thinking, and tool calls.

`ChatHandler` uses that Go renderer for `/completion`. Thinking capability
defaults `think` to true when the model advertises it.

## What a generate step costs

After load, one `/api/chat` turn is:

1. Template or renderer → one prompt string.
2. One prefill over all new prompt tokens (prefix cache may skip a shared
   prefix on the MLX runner; llama.cpp has its own cache / slot reuse).
3. One decode step per output token: hidden state at the new position,
   logits over the vocabulary, sample, detokenize a piece of UTF-8.
4. NDJSON write and flush per chunk when `stream` is true (Coder uses
   stream).

The HTTP process never holds the weight tensors. If the child dies
(OOM, abort), the next schedule attempt starts a new child.

## Bounds this document does not cover

- Cloud / remote models (`RemoteHost` / `RemoteModel`): `ChatHandler`
  proxies to another Ollama and never loads local weights.
- `ollama create` conversion from Hugging Face or safetensors into GGUF or
  MLX blobs (`server/create.go`, `x/create`). That is packaging, then the
  load paths above.
- Exact ggml kernel math, mmap, Metal shared mapping, and the `qwen35`
  graph inside llama.cpp. That walkthrough is
  [LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md),
  against the local llama.cpp checkout. Ollama fetches pin `b10630` at
  build time and applies `llama/compat/`; the OpenAgents in-process slice
  implements that library path and does not spawn `llama-server`. The
  CLI labels for that path are [CLI.md](./CLI.md).
- Psionic's loader, admission, or Metal path in this repository.
  Implementation has not landed. The llama.cpp document is the library
  contract. The CLI document is the command and status contract.
