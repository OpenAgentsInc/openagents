# OpenAgents CLI: local inference commands and teach status

- Class: owner-accepted CLI and teach-mode contract
- Status: accepted 2026-08-29; slices 0–6 landed (OpenAgents #344, `map.done`)
- Intent: [INTENT.md](./INTENT.md)
- Plan: [PLAN.md](./PLAN.md)
- Bytes behind the statuses: [LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md)

This document is the command surface people type, the order we build it,
and the **exact status lines** the terminal shows from first GGUF lookup
through a finished generation. Implementation fills those lines in from
the top. It does not spawn `llama-server`. It does not call Ollama.

```text
CLAIM
actor/session: psionic issue ledger 344-347 + unload/memory/TUI strings
base: ae754381292a38bbc03051369f427d0d4b713bfe
worktree/branch: detached github/main
scope: issue ledger, inference unload, memory statuses, Coder TUI load strings
paths: docs/psionic/CLI.md, docs/psionic/INTENT.md, docs/psionic/README.md, docs/psionic/PLAN.md
hot files: none
hot contracts: none
verification: docs-only; whitespace and path-local review
claimed_at: 2026-08-29T16:25:00Z
```

## How you watch the work

The spine command is `openagents inference run`. While this program is
under construction, that command defaults to **teach mode**: every
canonical status line prints as it happens, and any step that is not in
the binary yet prints as not built, then the process stops.

You do not wait for Coder before you can see load progress. You run
`inference run` against a GGUF path. Each merged slice lights up more of
the same script until the last line is `Inference complete`.

`--preview` prints the whole script without opening a file. Use it to
read the map. `--until <step-id>` runs only through that step.

## Commands

Two root subcommands join the existing `Commands` enum in
`crates/openagents-cli/src/cli.rs`. Global flags already on the binary
still apply: `--json`, `--verbose`, `--no-color`.

Teach status goes to stderr as text. `--json` emits the same steps as
one JSON object per line on stderr (`id`, `message`, `state`, fields).
Stdout stays for the generation (token text) or a final summary.

### `openagents inference`

Product lifecycle and the visible run loop. No training. No mesh.

| Command | What you do with it |
| --- | --- |
| `inference run` | Walk the load-and-generate path in this process. First command to implement. Teach mode default. |
| `inference models` | List admitted GGUFs in the local store. |
| `inference add <path-or-ref>` | Admit a GGUF, copy it into `~/.openagents/inference/models/<digest>.gguf`, write a manifest. Downloads only for an allowlisted ref with an expected digest. |
| `inference remove <model>` | Delete a store entry. |
| `inference serve` | Same load path, then bind OpenAI-compatible HTTP on `127.0.0.1`. Refuse `0.0.0.0` by default. Optional for first Coder green. |
| `inference status` | Show whether a `serve` child of this CLI is loaded, which model, backend, digest. |
| `inference stop` | Stop that `serve`. An in-process Coder turn does not use this. |
| `inference unload` | Release in-process mmap/Metal weights. Distinct from `inference stop` (serve child). |
| `inference doctor` | Backends, store path, last admitted model, Metal/CPU presence. |

`run` flags:

```text
openagents inference run
  --gguf <path>              Local GGUF (required until the store has a default)
  --model <id>               Admitted store id (later)
  --prompt <text>            Prompt for tokenize and generate steps
  --max-tokens <n>           Decode budget (generate steps)
  --backend auto|metal|cpu   Default auto
  --teach / --no-teach       Teach is default on run until Coder is the product path
  --quiet                    One line per phase, no teach explanations
  --preview                  Print the full status script; do not open a GGUF
  --until <step-id>          Stop after this step succeeds
```

`--verbose` (global) adds debug logs. It does not replace `--teach`.
Teach is the stable, user-facing script. Verbose is for developers
grepping library internals.

### `openagents psionic`

Harness. Use this when you are probing the library rather than running
the product loop.

| Command | What you do with it |
| --- | --- |
| `psionic inspect <artifact>` | Dump GGUF architecture, tensor names, sizes. No mmap of weights required. |
| `psionic admit <artifact>` | Run admission only: family, digest, required tensors, refuse unknown. |
| `psionic backends` | List compiled backends. |
| `psionic doctor` | Library graph, feature flags, provenance pin. |

`inspect` and `admit` share the early GGUF steps with `inference run`.
They stop before mmap. Prefer `inference run --until meta.done` when you
want the same messages the product command will keep.

### `openagents coder`

Load, unload, and memory UI are OpenAgents issues 345–347. `--model
psionic:` generate is still later (stage 4 in [PLAN.md](./PLAN.md)).
While in-process load runs, the Coder session UI shows the load messages
in this file through Weights ready. Do not dump the full teach essay
into the chat transcript unless the user opts in. Throttle like
`prefill.pos`. Ollama stays on `ollama:` until replacement gates pass.

## Build order

Implement from the top of the status script. Each slice is a CLI you can
run. Later slices keep the earlier messages.

| Slice | `--until` | Commands that become useful | Exit when the slice is done |
| --- | --- | --- | --- |
| 0 | `script` | `inference run --preview` | Prints every canonical line as `pending`. No file I/O. |
| 1 | `gguf.found` | `inference run --gguf <path>` | Locates a local file. No parse. |
| 2 | `gguf.open` | same | Opens the fd. Reads magic and version. |
| 3 | `meta.done` | `run`, `psionic inspect` | KV metadata and tensor index. Architecture, shapes, quant. Still no weight mmap. |
| 4 | `tok.done` | `run --prompt …` (tokenize only) | Vocab and BPE merges from GGUF KV. |
| 5 | `admit.done` | `psionic admit`, `inference add` | Required `qwen35` names (and Ollama `mtp.*` rename if needed). Store copy. |
| 6 | `map.done` | `run` | mmap, named tensors, CPU vs Metal placement, shared Metal buffer, `data` pointers. |
| 7 | `ctx.done` | `run` | Context, hybrid KV and Gated DeltaNet caches, scheduler. |
| 8 | `prompt.done` | `run --prompt` | Chat template and tokenize. |
| 9 | `prefill.done` | `run --prompt` | Prefill all prompt positions. |
| 10 | `gen.done` | `run --prompt --max-tokens` | Decode, sample, stream, stop. **Inference complete.** |
| 11 | (product) | `models`, `serve`, `doctor`, Coder | Lifecycle around the same path. |
| 12 | (product) | `inference unload`, `inference status --json` memory fields, Coder load UI | Unload, memory, and Coder TUI load progress (issues 345–347). Parallel to generate. Requires `map.done` (issue 344). Do not skip generate. |

Slice 1 is the first code packet after provenance. It prints `Looking
for GGUF` through `Found GGUF at …`. Slice 3 is the first packet that
proves we parsed a real file. Slice 10 is the first token out. Slice 12
is a product surface on top of `map.done`; it does not replace slices
7–10.

Do not skip ahead to generate while `meta.done` is missing. Do not hide
a missing step by calling Ollama.

**Unload (issue 346)** — ids and messages exactly:

| id | Message |
| --- | --- |
| `unload.start` | `Unloading weights` |
| `unload.mmap` | `Unmapping GGUF` |
| `unload.metal` | `Releasing Metal buffer` (skip on CPU-only) |
| `unload.done` | `Weights unloaded` |
| `unload.fail` | `Unload failed: {reason}` |

**Memory (issue 347)** — ids and messages exactly:

| id | Message |
| --- | --- |
| `mem.mmap` | `mmap resident {rss} / mapped {mapped}` |
| `mem.metal` | `Metal buffer {size}` (skip on CPU-only) |
| `mem.rss` | `Process RSS {size}` |
| `mem.caches` | `Caches KV {kv} GDN {gdn}` |

`inference status --json` also reports `mmap_bytes`, `metal_bytes`,
`rss_bytes`, `cache_kv_bytes`, `cache_gdn_bytes`.

**Coder TUI (issue 345)** — while in-process load runs, the Coder
session UI shows CLI.md load messages through Weights ready. Do not
dump the full teach essay into the chat transcript unless the user
opts in. Throttle like `prefill.pos`.

## Teach output

Each step has a stable `id` (for `--until`, `--json`, and tests) and a
**message** (what you read).

Text form, teach default:

```text
[gguf.look] Looking for GGUF
            Checking the model store and any --gguf path you passed.
```

`state` is `ok`, `pending`, `skip`, or `fail`.

- `ok` — this process did the step. Fill `{placeholders}`.
- `pending` — `--preview`, or a later slice not in the binary. Message
  stays in the canonical wording with placeholders not filled.
- `skip` — step does not apply (no download if the file is already
  local).
- `fail` — stop. Print the fail message. Non-zero exit.

After the last `ok` step, if the next canonical step is not built and
you did not pass `--until` at the last `ok`:

```text
[build.stop] Stopping: next step is not built yet
             Last completed step: meta.done
             Next to build: tok.read
```

`--until` on a built step exits 0 after that step without printing the
rest as pending, unless `--teach` and `--preview` are both set.

## Canonical status script

This is the entire path. Placeholders in `{braces}` are replaced on
`ok`. Quoted messages are the user-visible strings. Do not rephrase them
in code.

Download lines run only when `add` / `run` is given an allowlisted remote
ref and the digest is not already in the store. Otherwise those ids emit
`skip`.

### Slice 0 — script

| id | Message |
| --- | --- |
| `run.start` | `Starting inference run` |
| `run.teach` | `Teach mode on` |
| `run.until` | `Running until {step-id}` |
| `run.preview` | `Preview only; not opening a GGUF` |

### Slice 1 — find the file

| id | Message |
| --- | --- |
| `gguf.look` | `Looking for GGUF` |
| `gguf.store` | `Checking model store at {store-path}` |
| `gguf.path` | `Checking local path {path}` |
| `gguf.missing` | `GGUF not in store` |
| `gguf.download` | `Downloading GGUF from {source} to {dest}` |
| `gguf.download.progress` | `Download progress {bytes}/{total} bytes` |
| `gguf.download.done` | `Download complete` |
| `gguf.hash` | `Verifying SHA-256` |
| `gguf.hash.ok` | `Digest matches {digest}` |
| `gguf.found` | `Found GGUF at {path} ({size})` |

First implementation target: `gguf.look`, `gguf.path`, `gguf.found` for
`--gguf`. Store, download, and hash wait for `inference add` / allowlist
work in slice 5. Until then, if there is no `--gguf`, fail with
`Looking for GGUF` then `No GGUF path given; pass --gguf`.

Fail messages (same id family, `state=fail`):

| id | Message |
| --- | --- |
| `gguf.fail.arg` | `No GGUF path given; pass --gguf` |
| `gguf.fail.missing` | `GGUF not found at {path}` |
| `gguf.fail.hash` | `Digest mismatch: expected {expected}, got {got}` |

### Slice 2 — open header

| id | Message |
| --- | --- |
| `gguf.open` | `Opening GGUF {path}` |
| `gguf.load` | `Loading GGUF` |
| `gguf.magic` | `Reading magic` |
| `gguf.magic.ok` | `Magic is GGUF` |
| `gguf.version` | `Reading version` |
| `gguf.version.ok` | `Version is {version}` |

| id | Fail message |
| --- | --- |
| `gguf.fail.magic` | `Not a GGUF file (magic is {got})` |
| `gguf.fail.version` | `Unsupported GGUF version {version}` |

### Slice 3 — metadata and tensor index

| id | Message |
| --- | --- |
| `gguf.n_tensors` | `Reading tensor count` |
| `gguf.n_tensors.ok` | `Tensor count is {n}` |
| `gguf.n_kv` | `Reading key-value count` |
| `gguf.n_kv.ok` | `Key-value count is {n}` |
| `meta.read` | `Reading metadata` |
| `meta.arch` | `Architecture is {arch}` |
| `meta.ftype` | `File type is {ftype}` |
| `meta.embd` | `Hidden width is {n}` |
| `meta.layers` | `Layer count is {n}` |
| `meta.vocab` | `Vocabulary size is {n}` |
| `meta.ffn` | `FFN width is {n}` |
| `meta.quant` | `Quantization is {quant}` |
| `meta.attn_interval` | `Full-attention interval is {n}` |
| `meta.nextn` | `MTP extra layers is {n}` |
| `meta.ctx` | `Trained context length is {n}` |
| `idx.read` | `Reading tensor index` |
| `idx.ok` | `Indexed {n} tensors` |
| `meta.done` | `Metadata ready` |

| id | Fail message |
| --- | --- |
| `meta.fail.arch` | `Unsupported architecture {arch}` |
| `idx.fail` | `Tensor index is truncated or corrupt` |

This slice is parse-only (`no_alloc`). It does not map weights.

### Slice 4 — tokenizer tables (still KV, not weights)

| id | Message |
| --- | --- |
| `tok.read` | `Reading tokenizer` |
| `tok.model` | `Tokenizer model is {model}` |
| `tok.tokens` | `Loaded {n} tokens` |
| `tok.merges` | `Loaded {n} BPE merges` |
| `tok.special` | `Special tokens bos={bos} eos={eos}` |
| `tok.done` | `Tokenizer ready` |

| id | Fail message |
| --- | --- |
| `tok.fail.model` | `Unsupported tokenizer model {model}` |
| `tok.fail.merges` | `Tokenizer merges missing` |

### Slice 5 — admit and store

| id | Message |
| --- | --- |
| `name.translate` | `Translating Ollama tensor names to llama.cpp names` |
| `name.check` | `Checking required tensor names` |
| `admit.ok` | `Admission passed for {family} digest {digest}` |
| `store.copy` | `Copying GGUF into store {dest}` |
| `store.manifest` | `Wrote manifest {path}` |
| `admit.done` | `Model admitted` |

`name.translate` is `skip` when names are already `blk.N.nextn.*`.

| id | Fail message |
| --- | --- |
| `admit.fail.tensor` | `Missing required tensor {name}` |
| `admit.fail.family` | `Refusing to alias {arch} as a different family` |

### Slice 6 — mmap and Metal (weights)

| id | Message |
| --- | --- |
| `backend.load` | `Loading backends` |
| `backend.cpu` | `CPU backend ready` |
| `backend.metal` | `Metal backend ready` |
| `backend.pick` | `Selected backend is {backend}` |
| `map.mmap` | `Mapping GGUF into memory` |
| `map.mmap.ok` | `mmap size {size}` |
| `map.tensors` | `Creating named tensors` |
| `map.devices` | `Assigning devices: embeddings on CPU, {n_layers} layers on {backend}, output on {backend}` |
| `map.metal` | `Wrapping mmap as Metal shared buffer` |
| `map.bind` | `Binding tensor data pointers` |
| `map.unmap` | `Unmapping unused header and tail` |
| `map.done` | `Weights ready ({size} mapped)` |

`map.metal` is `skip` on CPU-only. Fail if Metal was requested and the
shared wrap cannot be created; do not fall back to Ollama.

| id | Fail message |
| --- | --- |
| `backend.fail` | `No usable backend ({reason})` |
| `map.fail.mmap` | `mmap failed: {reason}` |
| `map.fail.metal` | `Metal shared buffer failed: {reason}` |

### Slice 7 — context and caches

| id | Message |
| --- | --- |
| `ctx.alloc` | `Allocating context` |
| `ctx.length` | `Context length is {n}` |
| `ctx.kv` | `Allocating KV cache for {n} full-attention layers` |
| `ctx.gdn` | `Allocating recurrent state for {n} Gated DeltaNet layers` |
| `ctx.sched` | `Graph scheduler ready` |
| `ctx.done` | `Context ready` |

| id | Fail message |
| --- | --- |
| `ctx.fail.mem` | `Not enough memory for context {n}: {reason}` |

### Slice 8 — prompt

| id | Message |
| --- | --- |
| `prompt.template` | `Applying chat template` |
| `prompt.tokenize` | `Tokenizing prompt` |
| `prompt.done` | `Prompt is {n} tokens` |

| id | Fail message |
| --- | --- |
| `prompt.fail.empty` | `No prompt given; pass --prompt` |
| `prompt.fail.tok` | `Tokenize failed: {reason}` |

### Slice 9 — prefill

| id | Message |
| --- | --- |
| `prefill.start` | `Prefill starting` |
| `prefill.pos` | `Prefill position {i}/{n}` |
| `prefill.done` | `Prefill complete` |

`prefill.pos` may throttle (for example every 32 positions plus last)
so a long prompt does not flood the terminal. `--teach` still shows
start and complete. `--json` may emit every position.

### Slice 10 — generate

| id | Message |
| --- | --- |
| `gen.step` | `Decode step {i}` |
| `gen.logits` | `Computing logits` |
| `gen.sample` | `Sampling token {id} ({piece})` |
| `gen.stream` | `Streaming {piece}` |
| `gen.stop.eos` | `Stop: end of sequence` |
| `gen.stop.length` | `Stop: token budget {n}` |
| `gen.stop.cancel` | `Stop: cancelled` |
| `gen.stats` | `Generated {n} tokens` |
| `gen.done` | `Inference complete` |

Token pieces also write to stdout as they stream (product path). Teach
lines on stderr stay in sync with that stream.

## Example: first slice you can run

When only slice 1 exists:

```text
$ openagents inference run --gguf /models/qwen38.gguf --until gguf.found
[run.start] Starting inference run
[run.teach] Teach mode on
[run.until] Running until gguf.found
[gguf.look] Looking for GGUF
[gguf.path] Checking local path /models/qwen38.gguf
[gguf.found] Found GGUF at /models/qwen38.gguf (29.1 GiB)
```

When slices 1–3 exist and you omit `--until`:

```text
… metadata lines …
[meta.done] Metadata ready
[build.stop] Stopping: next step is not built yet
             Last completed step: meta.done
             Next to build: tok.read
```

`--preview` (works in slice 0):

```text
$ openagents inference run --preview
[run.preview] Preview only; not opening a GGUF
[gguf.look] Looking for GGUF                          pending
[gguf.store] Checking model store at {store-path}     pending
…
[gen.done] Inference complete                         pending
```

## JSON lines

With `--json`, each step is one object:

```text
{"id":"gguf.look","message":"Looking for GGUF","state":"ok"}
{"id":"gguf.found","message":"Found GGUF at /models/qwen38.gguf (29.1 GiB)","state":"ok","path":"/models/qwen38.gguf","size":31226560320}
```

Tests assert `id` and `state`, not wrapping prose.

## Relation to other docs

[PLAN.md](./PLAN.md) still owns crate allowlist, Coder stages, and
replacement gates. This file owns the strings people see and the
`--until` ladder.

[LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md)
owns mmap, Metal `newBufferWithBytesNoCopy`, Q8_0 blocks, and the
`qwen35` graph. Teach messages are the human labels for those steps.

[QWEN38_INFERENCE_PIPELINE.md](./QWEN38_INFERENCE_PIPELINE.md) owns the
decoder math. CLI statuses do not replace that document.

Weights stay out of git. `run --gguf` reads a path you already have.
`add` is what puts a copy under `~/.openagents/inference/`.
