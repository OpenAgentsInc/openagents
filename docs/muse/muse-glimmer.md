# Muse Glimmer local runtime and Omega integration

Date: 2026-08-10

Status: runtime validation complete; Omega integration active

Measured evidence:

- [M5 Max export and direct-runner benchmark](benchmarks/2026-08-10-muse-glimmer-m5-max.md)
- [M5 Max HTTP runtime matrix](benchmarks/2026-08-10-m5-max-http-runtime-matrix.md)

Repositories surveyed:

- OpenAgents `8b7f62de369d7ec1a4a698346a863de0db00ba3b`
- Omega `0eb8a263a20ab511306da8f936fae9657712d4a2`
- Psionic `54201484bb8eb11b528f7038922db02724864523`
- Hydralisk `73991d76e5d753abbbd3f38715b00ba893f80004`

External release pins:

- Muse Glimmer GGUF revision
  `93769bc7ab5ad1e9cd22d857e3138cf5d977ae81`
- Muse Glimmer BF16 revision
  `f84ecc3a0ea984a4c04542a84269e3d065350a6e`
- llama.cpp Muse Glimmer merge
  `62bf73d25c53b8161f8a22894d4f90c4aebbd7d0`
- ExecuTorch source
  `a3bf5568b81483ca1ec30198f846d10eaaab4b58`
- Muse Glimmer ExecuTorch PTE revision
  `b0376783689fb024c95b43a063552f938c678ec2`

Tracking:

- [Runtime benchmarks: omega#307](https://github.com/OpenAgentsInc/omega/issues/307)
- [Isolated local chat: omega#308](https://github.com/OpenAgentsInc/omega/issues/308)

## 1. Purpose

This document records what Meta released as Muse Glimmer, which runtime should
host it for Omega, how to connect it to Omega today, and what belongs in
Psionic or Hydralisk later.

The implementation order is:

1. Establish target-only and DFlash baselines with the official 17 GB GGUF and
   pinned llama.cpp Muse merge.
2. Reproduce Meta's Apple-silicon path with the official ExecuTorch MLX solo
   and DFlash Metal PTEs.
3. Expose llama.cpp's OpenAI-compatible Chat Completions endpoint on localhost
   and add a separate, opt-in `Muse Glimmer (Local)` entry to Omega.
4. Prove text chat and conversation history before adding tools or images.

The measured llama.cpp matrix found target-only Metal faster than its DFlash
implementation on this M5 Max. ExecuTorch MLX accelerated generation-heavy
workloads with DFlash, but its reference HTTP server buffered SSE output and
re-evaluated large anonymous prompts. Target-only llama.cpp is therefore the
first Omega host. Psionic is the long-term home for a Rust-owned native runtime
if that ownership is worth the port. Hydralisk is the later NVIDIA serving and
validation lane.

## 2. Host decision

| Candidate          | Apple silicon             | Muse weights                     | DFlash                                             | Omega-facing API                                 | Recommendation                                      |
| ------------------ | ------------------------- | -------------------------------- | -------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------- |
| llama.cpp          | Metal                     | GGUF                             | Native support                                     | OpenAI-compatible HTTP                           | Omega HTTP baseline; target-only currently wins on this Mac |
| ExecuTorch         | MLX                       | Official prebuilt Metal `.pte`   | Native support; 1.3x–1.8x on generation-heavy local cases | Official runner and buffered OpenAI-compatible serving example | Mac performance lane after streaming/session work |
| Psionic            | Metal substrate exists    | Muse architecture is unsupported | No native speculative loop                         | OpenAI-compatible server after a port            | Long-term native owner                              |
| Hydralisk          | No Mac execution lane     | No Muse profile                  | No Muse DFlash integration                         | Authenticated proxy around supported GPU engines | NVIDIA scale and evidence later                     |
| vLLM               | No Mac backend            | BF16/runtime-matched package     | No DFlash recipe; N-gram speculation is documented | OpenAI-compatible HTTP                           | NVIDIA production serving                           |
| Ollama / LM Studio | Partner support announced | Availability is still landing    | Not yet verified here                              | OpenAI-compatible integrations                   | Re-evaluate after released builds are verified      |

### Why llama.cpp is the Omega HTTP baseline

Meta publishes the local checkpoints as GGUF and documents llama.cpp as the
CPU, Metal, NVIDIA, and AMD path. llama.cpp merged Muse Glimmer architecture,
multimodal projector, chat-parser, and DFlash support in PR 26841 on
2026-08-10. It also provides the HTTP shape Omega already consumes.

This is the smallest integration boundary: llama.cpp owns model loading,
Metal execution, the Muse chat template, ATEM parsing, speculative decoding,
and the KV cache. Omega sends ordinary OpenAI Chat Completions requests and
receives ordinary streamed text and `tool_calls`.

### Why ExecuTorch MLX is required

Meta's published M4 Max and M5 Max speed measurements use ExecuTorch with the
MLX backend. Meta publishes self-contained solo and DFlash Metal `.pte`
programs, `solo_runner` and `dflash_runner`, and a Muse serving example that
provides OpenAI-compatible chat completions with ATEM parsing. The MLX path is
therefore both the Mac performance reference and a viable second host.

It must be measured separately from llama.cpp. The runtimes use different
execution and speculative-decoding implementations, so the DFlash slowdown in
llama.cpp did not predict MLX behavior. The MLX worker improved several decode
medians and produced byte-identical greedy output, but the reference server did
not incrementally stream token deltas and had no addressable warm session for
the released artifact. Omega uses llama.cpp first because its HTTP lifecycle is
proven, it streams incrementally, and it consumes the released GGUF directly.

### Why Psionic is not the first host

Psionic has useful infrastructure for a future native implementation:

- Metal kernels for Q4_K, Q5_K, Q6_K, and other GGUF quantizations.
- KV and prefix caches, continuous batching, and OpenAI-compatible Chat
  Completions and Responses endpoints.
- A Rust-owned runtime boundary aligned with Omega's long-term local model
  needs.

It cannot load or execute Muse Glimmer today:

- `general.architecture = muse-glimmer` is not a recognized GGUF family.
- Generic GPU serving currently admits Gemma 4 and Qwen 3.5, not Muse.
- Prompt rendering uses an exact template-digest allowlist with no Muse/ATEM
  template.
- The decode loop is autoregressive. It has no DFlash block proposal,
  verification, acceptance/rejection, or KV rollback path.
- It has no Muse perception encoder or `mmproj-kquant.gguf` execution path.

A Psionic port would need the Muse tensor layout, alternating local/global
attention, gated attention and normalization behavior, Muse prompt and ATEM
parsing, DFlash verification and KV rollback, the perception encoder, and
Metal/CUDA validation.

### Why Hydralisk is not the Mac host

Hydralisk owns the Python/NVIDIA lane: vLLM, SGLang, TensorRT, CUDA and GCE
profiles, admission, evidence, and an authenticated streaming proxy. It has no
llama.cpp, ExecuTorch, MLX, Metal, or MPS runtime.

Hydralisk can later own a Muse NVIDIA profile and production evidence. It can
also proxy an OpenAI-compatible upstream after its vLLM-specific configuration
and capability reporting are generalized. Current gaps include no Muse
profile, no DFlash execution, no multimodal capability, no ATEM parser, and a
capability receipt that reports tool calling as disabled.

## 3. Model facts

Muse Glimmer is an Apache 2.0 open-weight, dense multimodal model from Meta
Superintelligence Labs. It is designed for local, always-on agent workflows.
It accepts text and images and produces text.

| Property              | Released value                                           |
| --------------------- | -------------------------------------------------------- |
| Parameters            | About 29.6B, including the perception encoder            |
| Language architecture | Dense decoder-only causal transformer                    |
| Layers                | 52                                                       |
| Hidden dimension      | 6,656                                                    |
| Attention             | Repeating local, local, local, global pattern            |
| Sliding window        | 2,048 tokens on local layers                             |
| Attention heads       | 32 query heads, 2 KV heads                               |
| Head dimension        | 128                                                      |
| Feed-forward network  | SwiGLU, intermediate dimension 19,968                    |
| Position encoding     | RoPE with theta 500,000 on local layers                  |
| Perception encoder    | About 1.8B parameter ViT-G/14, 50 layers, width 1,536    |
| Vocabulary            | 202,048 tokens: 200,000 BPE and 2,048 special tokens     |
| Image budget          | Up to 4,096 visual tokens per image                      |
| Default context       | 131,072 tokens; longer contexts are supported            |
| Languages             | Training data from more than 100 languages               |
| Knowledge cutoff      | 2026-01-04                                               |
| Audio                 | Not supported                                            |
| Video                 | Not explicitly optimized; frames are processed as images |

### Training

Meta reports three main phases:

1. Pre-training used logit distillation from Muse Spark outputs with a similar
   data mix to the teacher.
2. Mid-training increased long-context, agent-heavy data and richer reasoning
   traces alongside organic data.
3. Post-training combined supervised fine-tuning, on-policy distillation, and
   reinforcement learning across general, reasoning, coding, and agentic
   domains.

The target capabilities are long-horizon task completion, schema-precise tool
use, multi-step reasoning, recovery after failed tool calls, screenshot and
document understanding, controllable reasoning effort, and multilingual use.

Meta lists local agents, coding agents, function calling, multimodal
reasoning, synthetic-data generation, and LLM-as-a-judge evaluation as the
intended use cases. The launch ecosystem names AMD, Arm, Dell, Fireworks AI,
Hugging Face, Intel, llama.cpp, LM Studio, NVIDIA, Ollama, OpenRouter, SGLang,
RadixArk, Together AI, Unsloth, vLLM, and Inferact. Partner availability and
feature coverage still need verification before Omega depends on them.

### Prompt, reasoning, and tool contract

Muse Glimmer depends on its shipped chat template. The template uses
`<|start|>`, `<|message|>`, `<|eot|>`, `<|eom|>`, and `<|image|>` tokens. Direct
Transformers integrations should use `apply_chat_template` rather than
constructing prompt strings.

The model represents private reasoning as an `assistant to=self` turn and the
answer as a separate `assistant to=user` turn. Reasoning strength is `low`,
`medium`, `high`, or `xhigh`, with `high` as the documented default. Reasoning
traces can consume thousands of tokens, so clients need streaming and enough
output headroom.

Native tool calls use ATEM. A server is responsible for turning an
`assistant to=<tool_name>` ATEM block into the standard OpenAI `tool_calls`
shape. Muse Glimmer supports one tool call per turn. It does not support
parallel tool calls; the tool result must be returned before the model chooses
another tool.

The model-card sampling recommendation is:

- `temperature = 1.0`
- `top_p = 0.95`
- `top_k = 64`

Greedy decoding is still useful for deterministic integration and speculative
equivalence tests.

## 4. Released artifacts and memory

The GGUF repository contains four runtime files:

| File                                   |    Exact bytes | Approximate size | Purpose                                 |
| -------------------------------------- | -------------: | ---------------: | --------------------------------------- |
| `muse-glimmer-30B-kquant-17gb.gguf`    | 16,756,681,056 |         16.76 GB | Recommended 24 GB target                |
| `muse-glimmer-30B-kquant-dynamic.gguf` | 19,653,957,984 |         19.65 GB | Higher-memory 32 GB target              |
| `dflash-kquant.gguf`                   |  1,631,205,312 |          1.63 GB | Quantized speculative drafter           |
| `mmproj-kquant.gguf`                   |  1,400,328,928 |          1.40 GB | Quantized perception encoder for images |

For the first Omega bring-up, download this three-file set:

1. `muse-glimmer-30B-kquant-17gb.gguf` as the required language-model target.
2. `dflash-kquant.gguf` as the recommended speculative drafter.
3. `mmproj-kquant.gguf` for the later vision test; omit it from the server command
   until text and tool calling pass.

Do not also download `muse-glimmer-30B-kquant-dynamic.gguf` for the first run. It
is an alternative language-model target, not an add-on to the 17 GB target. It
uses about 2.90 GB more storage and has lower reported average quantization
degradation, but Meta's published M5 Max DFlash result uses the 17 GB target.
Evaluate the dynamic target as a follow-up A/B test after the Omega integration
works.

The target GGUF files are text-only on their own. Add `mmproj-kquant.gguf` for
image input. Add `dflash-kquant.gguf` for DFlash.

Meta also publishes ready-to-run ExecuTorch PTEs. The MLX reproduction uses
the two 17 GB text-only Metal artifacts from revision
`b0376783689fb024c95b43a063552f938c678ec2`:

| PTE | Exact bytes | SHA-256 | Purpose |
| --- | ---: | --- | --- |
| `muse-glimmer-k-quant-17G-128K-text-solo-metal.pte` | 17,945,894,528 | `aa94d2e3e101c3ad9a49f048e452cac1a6c6b64337a1bf6a85349394ed1029ef` | MLX target-only program |
| `muse-glimmer-k-quant-17G-128K-text-dflash-metal.pte` | 19,641,819,904 | `6fbec8fc06f50e84c1a0e1fb1588bb825b1b5a3bbc314558faec2541a12f42e4` | MLX target and DFlash program |

These PTEs are alternatives to exporting the GGUF files locally. They are
self-contained on MLX and do not need a `.ptd` data file.

Meta reports that a full-precision 30B model needs more than 55 GB for weights.
The released K-quants reduce the language model below 20 GB, leaving room in a
24 GB or 32 GB envelope for KV cache, perception, and drafting.

| Variant         | Reported average degradation across 15 benchmarks | Target memory |
| --------------- | ------------------------------------------------: | ------------: |
| BF16            |                                          Baseline |         64 GB |
| K-Quant-Dynamic |                                              0.2% |         32 GB |
| K-Quant-17GB    |                                              1.0% |         24 GB |

Those averages do not replace evaluation on Omega's tool and coding tasks.

### DFlash

DFlash is a five-layer block-diffusion drafter. It proposes a block, then the
target verifies the candidates in parallel. Rejected tokens are resampled from
the target distribution. The release uses a block size of 16, 32 query heads,
8 KV heads, a 2,048-token sliding window, and target hidden features from
layers 1, 13, 25, 37, and 49.

Speculative decoding does not approximate accepted output. With the same
sampling state, the target verification preserves the target model's output
distribution. The benefit is largest for single-request latency and long
reasoning traces. It diminishes when large batches already saturate a GPU.

Meta reports these batch-one greedy-decoding results for the 17 GB target and
quantized DFlash drafter:

| Device          | No speculation | With DFlash | Speedup | Runtime used by Meta |
| --------------- | -------------: | ----------: | ------: | -------------------- |
| NVIDIA RTX 5090 |     74.9 tok/s | 233.4 tok/s |    3.1x | llama.cpp            |
| Apple M4 Max    |     23.7 tok/s |  37.8 tok/s |    1.5x | ExecuTorch           |
| Apple M5 Max    |     26.6 tok/s |  50.2 tok/s |    1.8x | ExecuTorch           |

The M5 Max number is a target for this machine, not a promised llama.cpp
result.

## 5. Performance evidence

The [export and direct-runner report](benchmarks/2026-08-10-muse-glimmer-m5-max.md)
records reproducible GGUF-to-PTE exports plus independent llama.cpp and
ExecuTorch process runs. The [HTTP runtime matrix](benchmarks/2026-08-10-m5-max-http-runtime-matrix.md)
adds six scenarios, official prebuilt PTEs, a DFlash block-size sweep,
worker-backed OpenAI-compatible serving, real two-call history tests, raw
responses, and server logs.

Meta compares Muse Glimmer with Gemma4-31B and Qwen3.6-27B in thinking modes.
Representative reported scores are:

| Benchmark               | Muse Glimmer | Gemma4-31B | Qwen3.6-27B |
| ----------------------- | -----------: | ---------: | ----------: |
| MCP Atlas               |         75.5 |       54.2 |        62.5 |
| DeepSearch QA           |         74.6 |       61.7 |        71.1 |
| Tau3 Banking            |         23.5 |       15.1 |        16.7 |
| WildClawBench           |         47.6 |       37.6 |        43.2 |
| Gaia2                   |         43.3 |       36.4 |        40.0 |
| SkillsBench with skills |         44.3 |       32.4 |        46.6 |
| OSWorld-Verified        |         65.9 |       58.5 |        75.6 |
| SWE-Bench Pro           |         51.2 |       36.9 |        50.2 |
| SWE-Bench Verified      |         76.0 |       66.6 |        77.2 |
| SciCode                 |         43.6 |       43.4 |        39.8 |
| CharXiv Reasoning       |         78.8 |       77.7 |        78.4 |
| IFBench                 |         77.0 |       76.0 |        70.8 |
| AIME 2026               |         94.7 |       89.2 |        94.1 |
| AA-LCR                  |         80.0 |       68.3 |        73.3 |
| BEAM-128K               |         65.1 |       58.2 |        63.0 |

These are vendor-reported results, not Omega acceptance results. The
methodology mixes Meta reproductions, model self-reports, and Artificial
Analysis results. For comparison models, Meta selects the more favorable of a
self-reported score and its internal reproduction when both are available.
Agentic harnesses and prompts may not be optimized equally for every model.

Meta runs Muse Glimmer at high reasoning strength with temperature 1.0,
top-p 0.95, and top-k 64. Many internal results are averaged over three or four
runs; AIME is averaged over ten. Omega should reproduce the subset that maps
to its work: sequential tool calls, failure recovery, coding, skills,
long-context memory, prompt injection, and computer use.

## 6. Omega's available integration seam

Omega's built-in llama.cpp, Ollama, and LM Studio providers are currently
inside the inherited-provider registration block. Normal Omega starts with
`zed_production_services_enabled()` false and returns before registering those
providers.

Configured `language_models.openai_compatible` providers are registered by a
separate path before that return. A custom provider can also be selected by
`agent.default_model`. This makes the generic OpenAI-compatible provider the
transport seam without adding a Muse-specific HTTP client.

Omega's product integration uses a separate persisted owner and composer entry
named `Muse Glimmer (Local)`. It pins the exact `muse-glimmer/muse-glimmer`
provider/model pair to the inherited native loop without entering the Omega
router. The existing `Omega Agent` remains the first and default entry. Local
threads suppress tools, images, cloud fallback, cloud title generation, and
cloud compaction so an unavailable localhost endpoint fails locally instead
of silently moving conversation data or answers to another provider.

### Required wire behavior

The host must provide:

- `POST /v1/chat/completions`.
- Streaming SSE `data:` chunks.
- OpenAI function definitions and `tool_choice`.
- Streamed `choices[0].delta.tool_calls` with stable indices, IDs, names, and
  argument fragments.
- A `tool_calls` or `function_call` finish reason.
- The `max_tokens` output-limit parameter.
- Bearer authentication, or tolerance for the local `Authorization: Bearer
local` header Omega sends.

llama.cpp owns the translation from Muse's ATEM output to this contract.
DFlash remains entirely inside the host. The first Omega entry advertises text
only and does not send tool definitions; the tool-call requirements above are
for the follow-up that enables Muse's agent surface.

## 7. Reproducible first bring-up

### 7.1 Build a known llama.cpp version

Muse support merged on the release date. Use merge commit
`62bf73d25c53b8161f8a22894d4f90c4aebbd7d0` or a later verified revision:

```sh
git clone https://github.com/ggml-org/llama.cpp.git
git -C llama.cpp checkout 62bf73d25c53b8161f8a22894d4f90c4aebbd7d0
cmake -S llama.cpp -B llama.cpp/build -DGGML_METAL=ON -DCMAKE_BUILD_TYPE=Release
cmake --build llama.cpp/build --config Release -j --target llama-server
```

The installer documented by Meta is also valid after it publishes a build
containing that merge. Record `llama-server --version` with every result.

### 7.2 Download the assets

Install the Hugging Face CLI, authenticate if the repository requests it, and
download the recommended three-file set. The dynamic target is intentionally
absent from this command:

```sh
python3 -m pip install huggingface_hub
hf auth login
hf download meta-models/Muse-Glimmer-30B-GGUF \
  muse-glimmer-30B-kquant-17gb.gguf \
  dflash-kquant.gguf \
  mmproj-kquant.gguf \
  --local-dir "$HOME/models/muse-glimmer"
```

Verify the files against the checksums in the repository before running them.

### 7.3 Start the text and tool server

Start at 32K context to bound KV-cache memory while the path is being proven:

```sh
llama.cpp/build/bin/llama-server \
  --model "$HOME/models/muse-glimmer/muse-glimmer-30B-kquant-17gb.gguf" \
  --spec-draft-model "$HOME/models/muse-glimmer/dflash-kquant.gguf" \
  --spec-type draft-dflash \
  --spec-draft-n-max 15 \
  --alias muse-glimmer \
  --ctx-size 32768 \
  --parallel 1 \
  --api-key local \
  --host 127.0.0.1 \
  --port 8000
```

Keep one server slot for the first test. The released block size is 16 and the
pinned llama.cpp revision documents a maximum draft length of 15. DFlash is
intended for single-request latency, and one agent loop does not need batch
throughput. On the measured M5 Max, however, this llama.cpp DFlash path was
slower than target-only Metal; see the local benchmark before choosing the
default launch command.

For vision, restart with:

```sh
--mmproj "$HOME/models/muse-glimmer/mmproj-kquant.gguf"
```

Do not enable Omega's image capability until an `image_url` request succeeds
through the full server path.

### 7.4 Smoke-test the endpoint

Check the model alias:

```sh
curl http://127.0.0.1:8000/v1/models \
  -H "Authorization: Bearer local"
```

Check streamed text:

```sh
curl -N http://127.0.0.1:8000/v1/chat/completions \
  -H "Authorization: Bearer local" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "muse-glimmer",
    "messages": [
      {"role": "user", "content": "Reply with exactly: muse-ready"}
    ],
    "max_tokens": 256,
    "stream": true
  }'
```

Check ATEM-to-OpenAI tool parsing:

```sh
curl http://127.0.0.1:8000/v1/chat/completions \
  -H "Authorization: Bearer local" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "muse-glimmer",
    "messages": [
      {"role": "user", "content": "What is the weather in Tokyo? Use the tool."}
    ],
    "tools": [
      {
        "type": "function",
        "function": {
          "name": "get_weather",
          "description": "Get weather for a city.",
          "parameters": {
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"]
          }
        }
      }
    ],
    "parallel_tool_calls": false,
    "stream": false
  }'
```

The response should contain one standard `tool_calls` entry for
`get_weather`, not raw ATEM markup.

### 7.5 Reproduce ExecuTorch MLX

Use ExecuTorch commit
`a3bf5568b81483ca1ec30198f846d10eaaab4b58` and Python 3.10 or newer. Download
only the two official 17 GB Metal exports:

```sh
hf download meta-models/Muse-Glimmer-30B-ExecuTorch-PTE \
  --revision b0376783689fb024c95b43a063552f938c678ec2 \
  --include 'muse-glimmer-k-quant-17G-128K-text-solo-metal/*' \
  --include 'muse-glimmer-k-quant-17G-128K-text-dflash-metal/*' \
  --local-dir "$HOME/models/muse-glimmer/executorch"
```

Keep the canonical tokenizer and chat template together:

```sh
hf download meta-models/Muse-Glimmer-30B \
  --revision f84ecc3a0ea984a4c04542a84269e3d065350a6e \
  tokenizer.json tokenizer_config.json chat_template.jinja \
  --local-dir executorch/assets/hf
```

Build the MLX-specific runners and worker:

```sh
git clone https://github.com/pytorch/executorch.git
git -C executorch checkout a3bf5568b81483ca1ec30198f846d10eaaab4b58
(cd executorch/examples/models/muse-glimmer && \
  cmake --workflow --preset muse-glimmer-mlx)
```

Run `solo_runner --help` and `dflash_runner --help` from
`executorch/cmake-out/examples/models/muse-glimmer/` to inspect the pinned
arguments, then execute one direct prompt through each PTE. For the same HTTP
scenario suite used by llama.cpp, install the shared server requirements and
start the official worker-backed serving example:

```sh
python -m pip install -r executorch/examples/llm_server/python/requirements.txt

cd executorch
python -m executorch.examples.models.muse_glimmer.serving.serve \
  --model-path "$HOME/models/muse-glimmer/executorch/muse-glimmer-k-quant-17G-128K-text-solo-metal/muse-glimmer-k-quant-17G-128K-text-solo-metal.pte" \
  --tokenizer-path assets/hf/tokenizer.json \
  --hf-tokenizer assets/hf \
  --worker-bin cmake-out/examples/models/muse-glimmer/muse_glimmer_worker \
  --model-id muse-glimmer \
  --tool-parser none \
  --max-context 32768 \
  --host 127.0.0.1 \
  --port 8000
```

For DFlash, replace both `solo` path components with `dflash`. Do not pass
`--data-path` for MLX. The worker detects the DFlash method contract embedded
in the second PTE and uses `block_length=4`, `n_draft=3` by default; those
defaults beat `8/7` and `16/15` in the local direct-runner sweep.

The released greedy PTEs do not export sampling support. Use temperature zero
and omit `seed`, `top_k`, and `top_p` from requests. The reference server emits
valid SSE but buffers generation into a few events, so preserve its
`llm_turn_stats` log for prefill and decode throughput.

### 7.6 Configure Omega

Add this to the development profile's `settings.json`:

```json
{
  "language_models": {
    "openai_compatible": {
      "muse-glimmer": {
        "api_url": "http://127.0.0.1:8000/v1",
        "available_models": [
          {
            "name": "muse-glimmer",
            "display_name": "Muse Glimmer",
            "max_tokens": 32768,
            "max_output_tokens": 8192,
            "reasoning_effort": "low",
            "capabilities": {
              "tools": false,
              "images": false,
              "parallel_tool_calls": false,
              "prompt_cache_key": false,
              "chat_completions": true,
              "interleaved_reasoning": false,
              "max_tokens_parameter": true
            }
          }
        ]
      }
    }
  }
}
```

The exact provider/model pair makes the separate `Muse Glimmer (Local)` entry
available. It does not replace the existing Omega Agent default. The low
reasoning setting keeps basic chat responsive; use a larger effort only with
enough output headroom for Muse's separate reasoning turn.

Omega maps the provider ID `muse-glimmer` to the environment variable
`MUSE_GLIMMER_API_KEY`. Run an isolated development profile with the same key
as the server:

```sh
MUSE_GLIMMER_API_KEY=local \
  cargo run --profile release-fast -- --user-data-dir "$HOME/.omega-muse-dev"
```

After the vision request passes, change `images` to `true` and validate an
Omega image attachment end to end.

## 8. Acceptance plan

The first proof should record:

1. Server revision, model revision, command line, macOS version, and hardware.
2. Startup time and peak resident memory for target-only, target plus DFlash,
   and target plus DFlash plus perception.
3. Time to first token, decode tokens per second, and end-to-end task latency
   with and without DFlash.
4. Byte-identical greedy output with and without DFlash on a fixed prompt set.
5. A normal text response in an Omega thread.
6. One sequential project-tool call, its result, and the final answer.
7. A failed tool call followed by model recovery and retry.
8. A multi-turn coding task with compaction or pruning before the configured
   32K limit.
9. An image request after the perception projector is enabled.
10. Confirmation behavior for irreversible tool actions and an indirect
    prompt-injection test.

Use the published 50.2 tok/s M5 Max ExecuTorch result as a comparison point.
Do not treat it as the llama.cpp acceptance threshold. The user-facing target
is uninterrupted agent interaction, correct tool calls, and lower end-to-end
task latency.

## 9. Productization after the proof

### Phase 1: Omega surface

- Add Muse as a separate named local executor and leave the existing Omega
  Agent authority unchanged.
- Expose model status, context limit, loaded artifacts, and local/remote
  provenance.
- Keep `parallel_tool_calls` false for Muse.
- Surface startup and inference failures instead of silently falling back to a
  cloud model.

### Phase 2: Psionic decision

Only begin the native port after the llama.cpp proof provides a correctness
and performance baseline. The port is justified if Omega needs one Rust-owned
runtime, tighter lifecycle management, or optimizations that cannot be reached
through the sidecar boundary.

Do not reimplement DFlash in Omega. If Psionic becomes the host, DFlash belongs
in Psionic beside model execution and KV-cache ownership.

### Phase 3: Hydralisk NVIDIA lane

Add a Muse vLLM or SGLang profile after release-matched packages are verified.
Hydralisk should own GPU admission, launch profiles, health, metrics, receipts,
and repeatable throughput evidence. Generalize its capability reporting before
claiming tools, images, or DFlash.

## 10. Risks and limitations

- llama.cpp support merged on the model's release date. Pin a known revision
  until the path has broader release coverage.
- Meta's published Mac speed uses ExecuTorch, so llama.cpp Metal performance
  must be measured locally.
- ExecuTorch's reference server buffers token output and the released artifact
  exposes no addressable warm sessions; fix those before choosing it as the
  interactive default.
- The model advertises 128K context, but KV-cache cost still grows with the
  configured context. Start smaller and increase only after measuring memory.
- Muse supports one tool call per turn. Agent scaffolds must execute and return
  each result before the next call.
- Quantization can change edge-case behavior even when aggregate degradation
  is low.
- The model can produce inaccurate, biased, or unsafe output. Agentic systems
  need confirmation for irreversible actions, data minimization, prompt-
  injection defenses, and evaluations for the deployed tools and languages.
- Meta rates the model moderate-or-lower for chem/bio, cyber, and loss-of-
  control risk under its release assessment. That rating does not replace
  Omega's application-specific safety work.

## 11. Sources

- [Meta research announcement](https://research.meta.ai/blog/introducing-muse-glimmer-open-agentic-model)
- [Muse Glimmer developer documentation](https://dev.meta.ai/docs/muse-glimmer/)
- [Get the model](https://dev.meta.ai/docs/muse-glimmer/get-the-model)
- [Prompting guide](https://dev.meta.ai/docs/muse-glimmer/prompting)
- [Quantization](https://dev.meta.ai/docs/muse-glimmer/quantization)
- [Speculative decoding](https://dev.meta.ai/docs/muse-glimmer/spec-decode)
- [Runtime comparison](https://dev.meta.ai/docs/muse-glimmer/deploy)
- [llama.cpp deployment](https://dev.meta.ai/docs/muse-glimmer/llama-cpp)
- [ExecuTorch deployment](https://dev.meta.ai/docs/muse-glimmer/executorch)
- [Pinned GGUF model card and artifacts](https://huggingface.co/meta-models/Muse-Glimmer-30B-GGUF/blob/93769bc7ab5ad1e9cd22d857e3138cf5d977ae81/README.md)
- [Pinned BF16 model repository](https://huggingface.co/meta-models/Muse-Glimmer-30B/tree/f84ecc3a0ea984a4c04542a84269e3d065350a6e)
- [Evaluation methodology](https://research.meta.ai/static/muse-glimmer-methodology)
- [DFlash paper](https://arxiv.org/abs/2602.06036)
- [Perception Encoder paper](https://arxiv.org/abs/2504.13181)
- [llama.cpp Muse Glimmer PR 26841](https://github.com/ggml-org/llama.cpp/pull/26841)
- [llama.cpp Muse Glimmer merge](https://github.com/ggml-org/llama.cpp/commit/62bf73d25c53b8161f8a22894d4f90c4aebbd7d0)

Repository evidence used for the host decision:

- Omega: `crates/language_models/src/language_models.rs`,
  `crates/app_identity/src/app_identity.rs`,
  `crates/language_models/src/provider/api_compatible.rs`, and
  `crates/language_models/src/provider/open_ai_compatible.rs`.
- Psionic: `crates/psionic-models/src/lib.rs`,
  `crates/psionic-serve/src/openai_http.rs`,
  `crates/psionic-serve/src/qwen35.rs`, and
  `docs/audits/2026-06-27-deepspec-dspark-speculative-decoding-audit.md`.
- Hydralisk: `README.md`, `AGENTS.md`, `hydralisk/serve/config.py`,
  `hydralisk/serve/proxy.py`, and `hydralisk/serve/receipts.py`.
