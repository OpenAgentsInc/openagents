# Psionic for Humans: A Tour

A first-principles tour of Psionic — what it is, the ideas behind it, and how the codebase is organized. No prior ML framework experience required.

---

## What is Psionic?

**Psionic** is a Rust library that runs AI models on your machine. It can do two main things:

1. **Text generation (inference)** — You give it a prompt; it runs a language model and returns generated text. Think “chat” or “complete this sentence.”
2. **Embeddings** — You give it text; it runs a model and returns a list of numbers (a vector) that represents the meaning of that text. Other code uses those numbers for search, similarity, etc.

Psionic is meant to replace our dependency on Ollama: same job (run models locally), but in our own code, with clear contracts so we can plug it into the OpenAgents compute market and tell the truth about what ran and how.

---

## First principles: the AI concepts

### What is a language model?

A **language model** is a big mathematical function that was trained on lots of text. It takes a sequence of **tokens** (chunks of text, often words or subwords) and predicts “what token comes next?” When you “run” the model, you’re doing a lot of multiplications and additions on huge arrays of numbers — that’s **inference**.

### What are tokens?

**Tokens** are the units the model actually sees. Words get split into tokens (e.g. “running” might be “run” + “ning”). A **tokenizer** is the piece that turns your text into token IDs and back. Psionic has to load the tokenizer that matches the model so prompts and outputs are correct.

### What is “text generation” or “inference”?

**Inference** = running the model once to get one next-token prediction. **Text generation** = doing inference over and over: feed the prompt in, get one token out, append it, feed again, repeat until you hit a stop condition or length limit. So “inference” is the core operation; “text generation” is the loop that uses it.

### What are embeddings?

An **embedding** is a fixed-length list of numbers that represents the meaning of a piece of text. You send a string in; the model (or a part of it) produces that vector. Different models have different embedding sizes (e.g. 768 or 1024 numbers). Embeddings are used for search (find similar text), classification, RAG, etc. Psionic supports embedding models as a first-class product.

### What is a “model” on disk?

A **model file** (in our world, often **GGUF**) is basically:

- **Weights** — the learned numbers that define the mathematical function. Huge (billions of numbers for big models).
- **Metadata** — model architecture (how many layers, sizes), tokenizer info, sometimes chat templates.

**Quantization** means storing those weights in a compressed form (e.g. 4 bits per number instead of 32). That shrinks the file and can speed up inference, but the math to use quantized weights is a bit special — Psionic has to **decode** those blocks correctly. GGUF is a format that can store both full-precision and quantized weights plus metadata.

### What is a backend?

A **backend** is “where the math runs.” Same model, same operation, but:

- **CPU** — runs on your processor. Portable, no GPU needed; slower for big models.
- **Metal** — runs on Apple GPUs.
- **CUDA** — runs on NVIDIA GPUs.
- **AMD** — runs on AMD GPUs (we have KFD and userspace paths).

Psionic keeps backends separate (one crate per backend). Each backend discovers devices, allocates memory, and runs the low-level ops (matmul, activations, etc.). The rest of Psionic stays backend-agnostic and **selects** a backend based on what’s available and what the model needs. We never want to “say Metal” but silently run on CPU — so backend selection and capability reporting are explicit and truthful.

### What is a KV-cache?

During text generation, the model reuses the key/value pairs it already computed for previous tokens so it doesn’t recompute them every time. That stored state is the **KV-cache**. It grows with sequence length. Psionic will need explicit KV-cache ownership, sizing, and (for long context) paging policy so we can report and limit resource use honestly.

### What is “provider” and “receipt”?

In the OpenAgents world, a **provider** is a machine offering compute (e.g. inference or embeddings) to the market. **psionic-provider** is the crate that adapts Psionic’s execution to the provider contract: it produces **receipts** and **capability envelopes** — proof of what ran, how long it took, what backend was used, and what the buyer gets to pay for. So “provider” here = the interface between “Psionic ran something” and “the market gets a truthful record of it.”

---

## The codebase: crates and what they do

Psionic is split into several crates. Here’s the map.

### Foundation (types and math)

| Crate | Role in plain English |
|-------|------------------------|
| **psionic-core** | Building blocks: tensors (shapes, dtypes), devices, quantization modes, block layouts. No execution — just the types the rest of the stack share. |
| **psionic-ir** | **IR** = intermediate representation. A **graph** of operations (add, matmul, reshape, etc.) and an **execution plan** — a sequence of steps to run that graph. The plan is deterministic and can be hashed for receipts and replay. |
| **psionic-compiler** | Turns a graph into an execution plan. **Lowering** = deciding how each op becomes backend-specific steps. Stays small and inspectable on purpose. |
| **psionic-runtime** | Runtime **traits**: device discovery, allocation, execution, health. Defines what a “backend” must implement. Also device descriptors, quantization support, and the interface the compiler’s plan gets executed on. |

### Models and data

| Crate | Role in plain English |
|-------|------------------------|
| **psionic-models** | **Model** = weights + metadata. This crate knows about: model descriptors (architecture), weight formats (e.g. SafeTensors, GGUF), loaders, tokenizers, and **bundles** (a loaded model ready to use). Embedding vs decoder (text-generation) models are both represented. Quantized tensor storage and metadata live here. |

### Serving and product surface

| Crate | Role in plain English |
|-------|------------------------|
| **psionic-serve** | **Serving** = the API you’d call to “run this model.” Request/response types for **embeddings** (input text → vector) and **text generation** (prompt + options → stream or full response). Session and KV-cache handling for generation. Implementations like `CpuModelEmbeddingsService`, `MetalModelEmbeddingsService`, `CpuModelTextGenerationService` — these are the executors that actually run the model and return results. |
| **psionic-provider** | **Provider** = the adapter to the compute market. Capability envelopes (what this node can do), execution receipts (what ran, how long, which backend), and adapter traits so the rest of the stack can plug Psionic into the provider substrate (e.g. Pylon) without lying about capability or delivery. |

### Backends (where the math runs)

| Crate | Role in plain English |
|-------|------------------------|
| **psionic-backend-cpu** | Runs the execution plan on the CPU. Reference implementation and fallback. |
| **psionic-backend-metal** | Runs on Apple GPUs (Metal). Discovery, allocation, and kernels for the ops we need (e.g. embeddings today; text generation as we add it). |
| **psionic-backend-amd-kfd** | AMD GPUs via the standard KFD/amdgpu path. Discovery and readiness; execution as we land it. |
| **psionic-backend-amd-userspace** | AMD via a userspace driver path. Higher risk; explicitly gated and separate from KFD. |
| **psionic-backend-cuda** | (Planned.) NVIDIA GPUs. Same idea: discovery, allocation, run the plan. |

---

## How a request flows

### Embeddings

1. Caller sends an **embedding request** (model id, input text, options).
2. **psionic-serve** finds the right **embedding executor** (e.g. CPU or Metal) and the loaded model bundle.
3. The executor tokenizes the input, runs the model’s embedding path (often a single forward pass), normalizes the vector if needed, and returns an **embedding response** (vector + metadata).
4. If this is for the compute market, **psionic-provider** wraps that in a receipt and capability truth.

### Text generation

1. Caller sends a **generation request** (model id, prompt, options like temperature, max tokens, stop).
2. **psionic-serve** finds the **text generation executor** and the loaded model. It may create or reuse a **session** (and KV-cache).
3. The executor runs the model step by step: encode prompt, then decode loop — each step produces the next token until stop or limit.
4. Tokens are streamed or collected into a **generation response** (text, usage, termination reason).
5. **psionic-provider** can again attach receipts and evidence for the market.

In both flows, the **backend** (CPU, Metal, etc.) is chosen up front based on what’s available and what the model supports; the rest of the path is backend-agnostic.

---

## Concepts summary

| Term | Simple meaning |
|------|----------------|
| **Inference** | Running the model once (or many times) to get predictions. |
| **Embeddings** | Turning text into a fixed-size vector of numbers that represents its meaning. |
| **Model / weights** | The learned numbers that define the model; stored in a file (e.g. GGUF). |
| **Quantization** | Storing weights in reduced precision to save space and sometimes speed. |
| **Tokenizer** | Converts text ↔ token IDs the model uses. |
| **Backend** | The hardware/runtime that runs the math (CPU, Metal, CUDA, AMD). |
| **KV-cache** | Cached key/value state during generation so we don’t recompute past tokens. |
| **IR / graph / plan** | A description of the computation (graph) and the steps to run it (plan); stable and receiptable. |
| **Provider / receipt** | The interface to the compute market and the proof of what ran. |

---

## Where to look for what

- **“How do we load a model?”** → `psionic-models`: loaders, bundles, weight formats, quantization.
- **“How do we run inference or embeddings?”** → `psionic-serve`: executors, requests, responses, sessions.
- **“How do we talk to the compute market?”** → `psionic-provider`: capability envelopes, receipts, adapter traits.
- **“What are the low-level ops and the execution plan?”** → `psionic-ir` (graph, plan), `psionic-compiler` (lowering), `psionic-runtime` (execution interface).
- **“How does CPU vs Metal vs AMD get chosen?”** → `psionic-runtime` (discovery, selection), then the specific `psionic-backend-*` crates.
- **“What’s the roadmap?”** → `docs/ROADMAP.md`. **“What’s the deeper design?”** → `docs/plan.md`.

---

## One-line crate cheat sheet

- **psionic-core** — Tensors, shapes, dtypes, devices, quantization types.
- **psionic-ir** — Graph of ops + execution plan (deterministic, digestible).
- **psionic-compiler** — Graph → plan (lowering).
- **psionic-runtime** — Backend traits: discover, allocate, execute, health.
- **psionic-models** — Model format, loaders, bundles, tokenizers.
- **psionic-serve** — Embedding and generation API; executors and sessions.
- **psionic-provider** — Market-facing capability and receipts.
- **psionic-backend-cpu** — Run on CPU.
- **psionic-backend-metal** — Run on Apple GPU.
- **psionic-backend-amd-kfd** / **psionic-backend-amd-userspace** — Run on AMD GPU (two paths).

That’s the tour. For implementation order and open work, see [ROADMAP.md](./ROADMAP.md).
