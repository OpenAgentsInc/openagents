# llama.cpp weight load and generate

- Class: lowest-level reference walkthrough
- Status: describes the llama.cpp checkout, not OpenAgents code
- Date: 2026-08-29
- Source: `/Users/christopherdavid/work/projects/repos/llama.cpp` at
  `3173a56471c1753650cd806694145ffd6dcace67` (`master`, commit message
  `metal : assert shared memory padding (#27951)`)
- Related: [OLLAMA_INFERENCE_PIPELINE.md](./OLLAMA_INFERENCE_PIPELINE.md)
  (Ollama starts `llama-server`; this document is what that child does),
  [QWEN38_INFERENCE_PIPELINE.md](./QWEN38_INFERENCE_PIPELINE.md),
  [CLI.md](./CLI.md) (user-visible status lines for the same path)
- First Coder target: GGUF `Q8_0`, architecture `qwen35`, Apple Silicon
  Metal (`macos-aarch64`)

This document is the byte-and-pointer path from a GGUF file on disk to one
sampled token. OpenAgents must run this path **inside the `openagents`
binary**. It does not spawn `llama-server`. It does not post to Ollama.
`llama-server` is an HTTP wrapper around the same library calls. The
library calls are what this program has to own.

Ollama's checkout fetches a llama.cpp pin (`b10630`) and applies
`llama/compat/` patches. This document uses the upstream tree at the hash
above. Where Ollama's published GGUF names differ from upstream, that
difference is recorded. It is a packaging seam, not a second loader.

```text
CLAIM
actor/session: llama.cpp lowest-level pipeline doc
base: c36d74ddfe2e196e39a50afd5283773a380f6b48
worktree/branch: llama-cpp-pipeline / detached github/main
scope: document llama.cpp GGUF parse, mmap, Metal shared mapping, graph
paths: docs/psionic/LLAMA_CPP_INFERENCE_PIPELINE.md, docs/psionic/README.md,
       docs/psionic/OLLAMA_INFERENCE_PIPELINE.md
hot files: none
hot contracts: none
verification: docs-only; whitespace and path-local review
claimed_at: 2026-08-29T16:10:00Z
```

## Product constraint

Coder Local today posts `POST /api/chat` to Ollama. Ollama's GGUF engine
starts a `llama-server` child. The child calls `llama_model_load_from_file`
and `llama_decode`. Those two functions, plus ggml backends, are the load
and generate implementation.

OpenAgents replaces that child. The binary must:

1. Parse GGUF metadata and the tensor index.
2. Map the weight blob (or copy it, when mmap is off).
3. Place tensors on CPU and GPU without a subprocess.
4. Build a ggml-style compute graph for `qwen35`.
5. Run quantized matmul kernels (Q8_0 on Metal first).
6. Hold hybrid caches (KV for full-attention layers, recurrent state for
   Gated DeltaNet layers).
7. Sample and detokenize.

Linking ggml as a crate versus rewriting each kernel is a later crate
decision. The memory contract, file layout, tensor names, and ops stay
the same either way. The CLI prints one status line per stage of that
contract; the wording is [CLI.md](./CLI.md).

## The loop in one page

```text
GGUF bytes on disk
    → gguf_init_from_file(..., no_alloc=true)     // header + KV + tensor index only
    → llama_model_create                          // qwen35 object from general.architecture
    → load_hparams, load_vocab                    // KV → structs, not weights
    → load_arch_tensors                           // named ggml_tensor shells
    → mmap(PROT_READ, MAP_SHARED)                 // whole file into the process
    → Metal newBufferWithBytesNoCopy (shared)     // GPU reads the same pages
       or ggml_backend_tensor_set (memcpy)
    → llama_context: hybrid KV + recurrent cache
    → tokenize prompt
    → decode prefill (all prompt positions)
    → decode one new position → logits
    → sample → detokenize → repeat
```

Nothing in that chain is a socket except Ollama's wrapper, which this
program does not keep.

## GGUF on disk

File: `ggml/include/gguf.h`, parser: `ggml/src/gguf.cpp`. Magic
`GGUF` (4 bytes). Current version is `3` (`GGUF_VERSION`). Version 1 is
rejected.

Layout, in order, little-endian:

1. Magic: `'G' 'G' 'U' 'F'`.
2. Version: `uint32`.
3. `n_tensors`: `int64`.
4. `n_kv`: `int64`.
5. For each KV pair:
   - key: `uint64` length, then that many bytes, no NUL.
   - type: `int32` (`gguf_type`: u8/i8/u16/i16/u32/i32/f32/bool/string/array/u64/i64/f64).
   - value: scalar payload, or for arrays: element type, `uint64` count, then
     elements.
6. For each tensor:
   - name: string (same encoding as KV keys).
   - `n_dims`: `uint32` (ggml max 4).
   - `ne[n_dims]`: `int64` each, dimension 0 is the innermost.
   - `ggml_type`: `int32` (`GGML_TYPE_Q8_0` = 8 for this Coder file).
   - `offset`: `uint64`, offset **into the data blob**, not into the file.
7. Pad to alignment. Alignment is KV `general.alignment` (`uint32`) if
   present, else **32** (`GGUF_DEFAULT_ALIGNMENT`).
8. Concatenated tensor payloads. Tensor `i`'s file offset is
   `gguf_get_data_offset(ctx) + tensor[i].offset`.

`gguf_init_from_file` with `no_alloc = true` (the load path) reads steps 1–6
and builds `ggml_tensor` **headers**. It does not copy step 8. The payload
stays on disk until mmap or `read`.

A split model (`split.N.gguf`) repeats this per shard. KV
`split.count` / `split.no` lists shards. Coder's live
`qwen3.8:27b-mtp-q8_0` blob is a single file.

### KV keys this architecture reads

Keys are namespaced. Architecture prefix is `qwen35.` when
`general.architecture` is `qwen35`. The loader maps enum `llm_kv` to
strings in `src/llama-arch.cpp`.

At minimum for this model:

| Key | Use |
| --- | --- |
| `general.architecture` | Dispatch: `"qwen35"` → `LLM_ARCH_QWEN35` |
| `general.file_type` | Informational; majority tensor type also sets `llama_ftype` |
| `qwen35.block_count` | Decoder layers (`n_layer`, 64 for 27B) |
| `qwen35.embedding_length` | Hidden width (5120) |
| `qwen35.feed_forward_length` | FFN width (17408) |
| `qwen35.attention.head_count` | Query heads (24) |
| `qwen35.attention.head_count_kv` | KV heads (4) |
| `qwen35.rope.*` / `rope.dimension_sections` | RoPE sections for full-attn layers |
| `qwen35.ssm.*` | Gated DeltaNet sizes (`d_conv`, `d_inner`, `d_state`, `dt_rank`, `n_group`) |
| `qwen35.full_attention_interval` | Default 4: layers where `(i+1) % 4 == 0` are full attention |
| `qwen35.nextn.predict_layers` | Extra MTP blocks after the main stack |
| `tokenizer.ggml.model` | `"gpt2"` → BPE |
| `tokenizer.ggml.tokens` | String array, vocab |
| `tokenizer.ggml.merges` | BPE merge list |
| `tokenizer.ggml.scores` / `token_type` | Optional per-token metadata |
| `tokenizer.ggml.bos_token_id` and siblings | Special IDs |
| `tokenizer.chat_template` | Used only if the host asks llama.cpp to apply the template |

Ollama's Go side already rendered the chat template for Coder. In-process
OpenAgents still needs the tokenizer arrays from the GGUF (or an equivalent
owned tokenizer). It does not need llama-server's `/completion` JSON.

## In-memory tensor: `ggml_tensor`

Defined in `ggml/include/ggml.h`. This is the object every later step
holds.

```c
struct ggml_tensor {
    enum ggml_type type;
    struct ggml_backend_buffer * buffer;
    int64_t ne[4];   // element counts; ne[0] innermost
    size_t  nb[4];   // byte strides
    enum ggml_op op;
    int32_t op_params[...];
    int32_t flags;
    struct ggml_tensor * src[GGML_MAX_SRC];
    struct ggml_tensor * view_src;
    size_t view_offs;
    void * data;     // payload pointer: mmap, CPU heap, or GPU mapping
    char name[GGML_MAX_NAME];
    void * extra;
    char padding[8];
};
```

Strides (`ggml.c` / comments on the struct):

- `nb[0] = ggml_type_size(type)` — for Q8_0 this is `sizeof(block_q8_0)` = 34.
- `nb[1] = nb[0] * (ne[0] / blck_size) + padding` — `blck_size` is 32 for Q8_0.
- `nb[i] = nb[i-1] * ne[i-1]`.

`ggml_nbytes` is the product along those strides. A Q8_0 matrix with
`ne = {5120, 17408, 1, 1}` is stored as `ceil(5120/32)` blocks along dim 0
times 17408 along dim 1, 34 bytes each. Weights stay in this packed layout
for the whole lifetime of the model. No full f32 dump of the 27B file.

After metadata parse, `data` is NULL. After `load_all_data`, `data` points
at the first byte of that tensor's payload.

## Q8_0 block (this machine's `file_type`)

`ggml/src/ggml-common.h`:

```c
#define QK8_0 32
typedef struct {
    ggml_half d;       // scale, IEEE f16
    int8_t  qs[QK8_0]; // 32 signed quants
} block_q8_0;          // 34 bytes; static_assert
```

Type table (`ggml/src/ggml.c`, `GGML_TYPE_Q8_0`): `blck_size = 32`,
`type_size = 34`, `is_quantized = true`, `to_float = dequantize_row_q8_0`.

Dequant, CPU reference (`ggml/src/ggml-quants.c`):

```c
y[j] = qs[j] * FP16_TO_FP32(d);   // per block of 32
```

Quant (Metal, `ggml/src/ggml-metal/kernels/quantize.h`): scale
`d = amax / 127`, then `qs[j] = round(x[j] / d)`. That is how the GGUF was
written. Load does not requantize.

Matmul kernels dequant **on the fly** inside the kernel. They do not
materialize an f32 weight matrix. Metal (`dequantize.h`):

```c
void dequantize_q8_0(device const block_q8_0 *xb, short il, thread type4x4 & reg) {
    const float d = xb->d;
    // 16 int8s (one half-block) → float4x4, times d
}
```

Host kernel name for the heavy path: `kernel_mul_mm_q8_0_f32` in
`ggml/src/ggml-metal/kernels/mul_mm.metal`. Decode (small batch) often hits
`kernel_mul_mv_ext_q8_0_f32_*` in `mul_mv.metal`. Same block format, different
tiling.

## Load call chain

Public entry: `llama_model_load_from_file` →
`llama_model_load_from_file_impl` (`src/llama.cpp`).

Preconditions: `ggml_backend_load_all()` (or equivalent) has registered
CPU and Metal. Zero backends is a hard error unless `vocab_only`.

`llama_model_load` then:

1. Construct `llama_model_loader`.
2. `llama_model_create(ml, params)` — architecture object.
3. `llama_prepare_model_devices` — enumerate ggml devices.
4. `model->load_hparams(ml)`.
5. `model->load_vocab(ml)`.
6. `model->load_tensors(ml)` — create named tensors, mmap, bind `data`.

Cancel via progress callback returns status `-2`. Any other throw is `-1`.

### Loader constructor: metadata only

`src/llama-model-loader.cpp`. For a path:

```c
gguf_init_params params = { .no_alloc = true, .ctx = &ctx };
metadata = gguf_init_from_file(fname, params);
```

That fills:

- `gguf_context` KV table.
- A `ggml_context` of empty tensors (names, `ne`, `type`, no `data`).
- `files[0] = llama_file(fname, "rb")` — keeps an fd for mmap/`pread`.

Then for each tensor:

```text
weights_map[name] = {
    idx:  0,                                    // shard index
    offs: gguf_get_data_offset + tensor_offset, // absolute file offset
    tensor: ggml_tensor*                        // metadata object
}
```

`offs + ggml_nbytes` must lie inside the file or the load aborts
("corrupted or incomplete").

`general.architecture` → `arch_name` → `llm_arch_from_string`. `"qwen35"`
maps to `LLM_ARCH_QWEN35` (`src/llama-arch.cpp`).

Majority `ggml_type` among tensors sets `ftype`. For this Coder blob that
is `GGML_TYPE_Q8_0` → `LLAMA_FTYPE_MOSTLY_Q8_0`.

Default load mode is mmap (`LLAMA_LOAD_MODE_AUTO` / `MMAP`). Ollama omits
`--load-mode`, so llama-server takes this path.

### Architecture object

`llama_model_create` (`src/llama-model.cpp`) switches on `arch` and
`new llama_model_qwen35(params)`. That class lives in
`src/models/qwen35.cpp`.

### Hyperparameters

Shared hparams (layer count, `n_embd`, heads, vocab) come from generic
`load_hparams`. Qwen 3.5 extras in `llama_model_qwen35::load_arch_hparams`:

- RMSNorm eps.
- RoPE dimension sections (4 ints).
- SSM / Gated DeltaNet: `ssm_d_conv`, `ssm_d_inner`, `ssm_d_state`,
  `ssm_dt_rank`, `ssm_n_group`.
- `n_layer_nextn` (MTP extra blocks). Must be `< n_layer_all`.
- Recurrent mask: KV `attention.recurrent_layers` if present; else
  layer `i` is recurrent iff it is in the main stack and
  `(i + 1) % full_attention_interval != 0`. Interval default 4.

64 layers and `n_embd == 5120` set `LLM_TYPE_27B`.

### Vocabulary

`llama_vocab::impl::load` (`src/llama-vocab.cpp`) reads KV, not tensors.

- `tokenizer.ggml.model == "gpt2"` → `LLAMA_VOCAB_TYPE_BPE`.
- `tokenizer.ggml.merges`: string array of `"left right"` pairs, ranked by
  index.
- `tokenizer.ggml.tokens`: string array, index = token id.
- Scores and token-type arrays if present.
- Special IDs: bos, eos, eot, unk, pad, FIM, …

Tokenize is byte-level GPT-2 BPE (the llama.cpp GPT-2 / Qwen path), then
IDs index `token_embd`. Detokenize walks the same table. OpenAgents needs
this table in-process. It cannot call Ollama's `/api/tokenize` as the
runtime.

### Named tensors (`load_arch_tensors`)

`load_tensors` first assigns devices, then calls
`llama_model_qwen35::load_arch_tensors`. That function **does not read
bytes**. It calls `create_tensor(name, shape, flags)`, which looks up
`weights_map`, checks `ne[]`, and allocates a new `ggml_tensor` in the
backend context for that buffer type. The new tensor still has `data ==
NULL`.

GGUF names (`src/llama-arch.cpp` `LLM_TENSOR_NAMES`, `"weight"` suffix
added by `LLM_TN`):

**Global**

| Name | Shape (27B) |
| --- | --- |
| `token_embd.weight` | `{ n_embd, n_vocab }` = `{5120, 248320}` |
| `output_norm.weight` | `{ n_embd }` |
| `output.weight` | `{ n_embd, n_vocab }` if present; else tied to `token_embd` (`TENSOR_DUPLICATED`) |

**Main stack, `il` in `0 .. n_layer-1`**

Every layer:

- `blk.{il}.attn_norm.weight` `{n_embd}`
- `blk.{il}.attn_post_norm.weight` `{n_embd}`
- `blk.{il}.ffn_gate.weight` `{n_embd, n_ff}`
- `blk.{il}.ffn_up.weight` `{n_embd, n_ff}`
- `blk.{il}.ffn_down.weight` `{n_ff, n_embd}`

Full-attention layers (`!is_recr(il)`):

- `blk.{il}.attn_q.weight`, `attn_k.weight`, `attn_v.weight` (or fused
  `attn_qkv` via `create_tensor_qkv`)
- `blk.{il}.attn_output.weight`
- `blk.{il}.attn_q_norm.weight`, `attn_k_norm.weight` `{head_dim}`

Gated DeltaNet / linear-attn layers (`is_recr(il)`):

- `blk.{il}.attn_qkv.weight` (optional fused)
- `blk.{il}.attn_gate.weight`
- `blk.{il}.ssm_conv1d.weight`
- `blk.{il}.ssm_dt.bias`, `ssm_a`, `ssm_beta.weight`, `ssm_alpha.weight`
- `blk.{il}.ssm_norm.weight`, `ssm_out.weight`

**MTP extra blocks**, `il` in `n_layer .. n_layer_all-1`, names
`blk.{il}.nextn.*` plus a full-attention decoder block. Loaded when
`load_mtp` is true. **The main graph does not execute them.** Comment in
`qwen35.cpp`: "MTP/NextN layers are loaded as extra decoder blocks but not
executed in the main pass."

Ollama's published blobs often store MTP as `mtp.*`. Ollama's
`llama/compat/` renames those to `nextn.*` inside the child **before** this
loader runs. Native llama.cpp at this hash expects `blk.N.nextn.*`. If
OpenAgents opens the live Ollama GGUF without that translation, name lookup
fails. Apply the same rename, or convert/load a llama.cpp-native GGUF.

`create_tensor` flags: `TENSOR_NOT_REQUIRED` (missing is OK),
`TENSOR_SKIP` (do not load MTP), `TENSOR_DUPLICATED` (alias, no second
copy).

### Device assignment

`llama_model_base::load_tensors` (`src/llama-model.cpp`):

- `n_gpu_layers` default `-1` means `n_layer_all + 1` (all repeating
  layers plus the output tensor).
- `i_gpu_start = max(n_layer_all + 1 - n_gpu_layers, 0)`.
- **Input embeddings stay on CPU** (`dev_input = cpu`). Comment in source:
  "very little benefit to offloading the input layer."
- Repeating layers `il` and the output slot `n_layer_all` go to GPU when
  `il >= i_gpu_start`.
- Multi-GPU splits by free memory (or `tensor_split`).

On one Apple GPU with default `-ngl`, every decoder layer and the LM head
are Metal; `token_embd` is a CPU mmap view. Prefill `GGML_OP_GET_ROWS` on
embeddings runs on CPU, then activations move to GPU.

If `LLAMA_LOAD_MODE_AUTO` and any selected device reports
`caps.mmap_support == false`, mmap is turned off for the whole load.

### mmap

`llama_model_loader::init_mappings` → `llama_mmap` (`src/llama-mmap.cpp`).

POSIX (macOS included):

```c
addr = mmap(NULL, file->size(), PROT_READ, MAP_SHARED, fd, 0);
```

Linux may add `MAP_POPULATE` when prefetch is on and there are no lazy
ranges. Prefetch uses `posix_madvise(..., POSIX_MADV_WILLNEED)`. NUMA
forces `POSIX_MADV_RANDOM` and disables prefetch.

This maps the **entire file**, header included. Later unmap drops unused
ends. The process does not `malloc` 29 GiB. First touch of a page is a
minor fault from the filesystem cache.

`llama_mlock` (`mlock`) is optional (`LLAMA_LOAD_MODE_MMAP_MLOCK`). Default
off.

### Backend buffers: wrap mmap or allocate

For each ggml context / buffer type, after mappings exist:

If mmap **and** the device advertises `buffer_from_host_ptr` **and** this
is the device's default buffer type:

1. `get_mapping_range` finds `[first, last)` covering every tensor of this
   context in that file.
2. `ggml_backend_dev_buffer_from_host_ptr(dev, addr + first, last - first,
   max_tensor_size)`.

Metal implementation (`ggml/src/ggml-metal/ggml-metal-device.m`
`ggml_metal_buffer_map`):

1. Page-align `ptr` down; extend `size` by the offset.
2. Round length up to a page.
3. If `size_aligned <= max_buffer_size`:

   ```objc
   [mtl_device newBufferWithBytesNoCopy:ptr
                                 length:size_aligned
                                options:MTLResourceStorageModeShared
                            deallocator:nil];
   ```

4. If the range exceeds Metal's max buffer size, split into overlapping
   views of `max_buffer_size` so any one tensor still fits in a single
   `MTLBuffer`.

`owned = false`, `is_shared = true`. Metal does not free the pages. The
GPU and CPU see the same unified-memory pages. **There is no memcpy of the
weight file onto the GPU.**

CPU backend's `buffer_from_host_ptr` wraps the same pointer as a host
buffer.

If that path is unavailable (CUDA does not implement
`buffer_from_host_ptr` in this tree; mmap off; non-default buft):

- `ggml_backend_alloc_ctx_tensors_from_buft` allocates a device buffer.
- `load_all_data` then **copies** into it.

### `load_all_data`: bind `data` or copy

For each created tensor, look up `weights_map` by name.

**Mmap + host-ptr buffer, `cur->data == NULL`:**

```c
uint8_t * data = mapping->addr() + weight->offs;
ggml_backend_tensor_alloc(buf_mmap, cur, data);
```

`ggml_backend_tensor_alloc` (`ggml/src/ggml-backend.cpp`) only sets:

```c
tensor->buffer = buffer;
tensor->data   = addr;
ggml_backend_buffer_init_tensor(buffer, tensor);
```

No read. The next kernel that uses `tensor` indexes `data` as Q8_0 blocks.

**Mmap but tensor already has a device buffer** (true GPU-private copy):

```c
ggml_backend_tensor_set(cur, data, 0, n_size);
```

That copies `n_size` bytes from mmap into the buffer.

**No mmap, host buffer:** `lseek` + `read` into `cur->data`.

**No mmap, device buffer:** 4 pinned staging buffers, 1 MiB default or
64 MiB when the fd needs alignment (`O_DIRECT`). Loop: wait event, read
chunk into pinned host memory, `ggml_backend_tensor_set_async`. CUDA/ROCm
use this. Apple Silicon mmap path does not.

After every tensor is bound, mmap unmaps the unused prefix (file header)
and unused suffix:

```c
mapping->unmap_fragment(0, mmap_used.first);
mapping->unmap_fragment(mmap_used.second, mapping->size());
```

`unmap_fragment` is `munmap` on page-aligned subranges. The live weight
span stays mapped.

## Context: caches, not weights

`llama_new_context_with_model` allocates:

- ggml backend scheduler (`ggml_backend_sched`) across CPU + Metal.
- Compute graphs (reused when topology matches).
- **Hybrid memory** for `qwen35`: KV cache for full-attention layers,
  recurrent / SSM state for Gated DeltaNet layers
  (`llama_memory_hybrid`, `build_inp_mem_hybrid` in the graph).
- Optional flash-attention workspace.

Weights are already in `llama_model`. The context does not reload them.
Context size (`n_ctx`) sizes the caches. That is the RAM/VRAM that grows
with conversation length. Weight footprint stays the mmap/Metal mapping.

## Generate: `llama_decode`

`src/llama-context.cpp` `llama_context::decode`:

1. Require `batch.token` or `batch.embd`.
2. `balloc->init` splits the `llama_batch` into micro-batches (`n_ubatch`).
3. `memory->init_batch` reserves cache slots (hybrid: attn K/V rows and
   recurrent states). Failure → `-2` (usually context full).
4. For each ubatch: `process_ubatch`.

`process_ubatch`:

1. `mctx->apply()` — bind this ubatch's cache views.
2. If the previous graph's topology still matches, reuse it (`n_reused++`).
   Else `model.build_graph(gparams)` and
   `ggml_backend_sched_alloc_graph`.
3. `res->set_inputs(&ubatch)` — write token ids, positions, mask bits into
   input tensors.
4. `graph_compute` → `ggml_backend_sched_graph_compute_async`.
5. Logits land in `res->t_logits` (`GGML_OP_MUL_MAT` of `output` × last
   hidden). Host reads them after sync when sampling on CPU.

Prefill is the same function with `n_tokens > 1`. Decode is `n_tokens ==
1` (or a small speculative batch). Same graph builder, different cache
update width.

### Qwen 3.5 graph (`qwen35.cpp` `graph::graph`)

Ops, in order, for each main-stack layer `il`:

1. `inpL = GET_ROWS(token_embd, token_ids)` on the first layer (CPU).
2. RMSNorm(`attn_norm`).
3. If recurrent: Gated DeltaNet (`build_layer_attn_linear`) — conv1d,
   QKV/gate projections (`MUL_MAT` on Q8_0 weights), SSM update against
   recurrent cache.
4. Else: full attention (`build_layer_attn`) — Q/K/V `MUL_MAT`, Q/K RMSNorm,
   RoPE with `rope_sections`, attention against KV cache, `attn_output`
   `MUL_MAT`.
5. Residual add.
6. RMSNorm(`attn_post_norm`).
7. FFN: `gate` and `up` `MUL_MAT`, activation (SiLU-style gated FFN),
   `down` `MUL_MAT`.
8. Residual add.
9. Next layer `inpL`.

After the last layer: RMSNorm(`output_norm`), then LM head `MUL_MAT`
(`output`, possibly tied to `token_embd`). Result shape
`{ n_vocab }` per requested output position.

`GGML_OP_MUL_MAT` with `src0` quantized and `src1` f16/f32 activations is
the cost of every projection. Metal picks `kernel_mul_mm_q8_0_f32` or a
`mul_mv` variant from tensor shapes and batch.

MTP graph (`LLM_GRAPH_TYPE_DECODER_MTP`) is a separate builder
(`graph_mtp`). The main decode loop does not enter it unless the host
enables NextN / speculative decoding.

### Sampling

Not part of weight load. llama.cpp samplers (`src/llama-sampling.cpp`)
consume the f32 logit row: temperature, top-k, top-p, min-p, penalties,
grammar. They return one `llama_token`. Detokenize uses the vocab tables
from the GGUF KV. OpenAgents can keep its own sampler; it still needs that
logit row from the graph.

`llama-server` adds HTTP, slots, and chat-template application around this.
None of that is required in-process. Coder already owns streaming and
tool-parse policy.

## What `llama-server` adds (and what you skip)

The binary `llama-server` (`tools/server`):

1. Parses CLI (`--model`, `-c`, `-ngl`, …).
2. Calls `llama_model_load_from_file` / `llama_init_from_model` (the path
   above).
3. Serves `/health`, `/completion`, `/v1/chat/completions`.
4. Tokenizes request text, builds `llama_batch`, loops `llama_decode`,
   samples, streams SSE.

Ollama's Go process is another HTTP hop in front of that. OpenAgents does
steps 2 and the decode/sample loop inside `openagents`. It does not exec
step 3.

## Apple Silicon cost model for this Coder blob

Live install: `qwen3.8:27b-mtp-q8_0`, GGUF ~29 GiB, `Q8_0`.

| Resource | What holds it |
| --- | --- |
| Weight bytes | One `mmap` of the blob; Metal `StorageModeShared` views the same pages |
| Extra weight copy | None on the mmap+Metal path |
| Fault-in | First prefill touches pages; later decode reuses them |
| Compute working set | Activations + hybrid KV/recurrent cache for `n_ctx` |
| CPU | Embedding `GET_ROWS`; scheduler leftover ops |
| GPU | Layer `MUL_MAT`, attention, GDN, FFN, LM head |

Turning mmap off, or running on a backend without `buffer_from_host_ptr`,
adds a full extra copy of the 29 GiB into a private GPU buffer. That is
the path you avoid on this machine.

## Implementation checklist for Psionic / OpenAgents

These are the units the in-process slice must perform. A subprocess that
speaks HTTP is not one of them.

1. **GGUF parser** — magic, version 3, KV, tensor index, alignment, data
   offset. Metadata-only first pass.
2. **Architecture table** — `qwen35` hparams, recurrent mask, tensor name
   list, MTP skip/load policy.
3. **Name translation** — if the file is an Ollama blob: `mtp.*` →
   `blk.N.nextn.*` (and the rest of `llama/compat/` for `qwen35`), or
   refuse and require a native GGUF.
4. **Tokenizer** — GPT-2 BPE from `tokenizer.ggml.tokens` + `merges`.
5. **`ggml_tensor` + Q8_0** — strides, `nbytes`, block layout, dequant
   formula.
6. **mmap** — `PROT_READ` `MAP_SHARED` of the fd; unmap header/tail after
   bind.
7. **Metal shared mapping** — `newBufferWithBytesNoCopy` +
   `MTLResourceStorageModeShared` on the used byte range; split if over
   `maxBufferLength`.
8. **Fallback copy path** — allocate backend buffer +
   `tensor_set` / async staging, for devices that cannot wrap host pointers.
9. **Device split** — embeddings CPU; layers and output Metal when
   `n_gpu_layers` covers them.
10. **Hybrid caches** — KV for full-attn layers; recurrent state for GDN
    layers; `n_ctx` sized.
11. **Graph** — RMSNorm, RoPE, GDN, GQA attention, gated FFN, tied or
    separate LM head; `MUL_MAT` on Q8_0.
12. **Scheduler** — execute the graph on CPU+Metal without an external
    server.
13. **Sample + detokenize + stream** into Coder. No silent Ollama fallback
    (plan already forbids that).

Verification of this document is source-read against the llama.cpp hash
above. It is not a claim that OpenAgents already runs the loop.
