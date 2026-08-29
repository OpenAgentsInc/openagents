# Initial plan: Psionic Qwen 3.8 local inference in `openagents`

- Class: owner-accepted implementation plan / work packet
- Status: accepted 2026-08-29; slices 0–6 (`map.done`) landed; slice 12 (unload, memory, Coder `/load`) landed in OpenAgents #345–#347
- Intent: [INTENT.md](./INTENT.md)
- CLI statuses: [CLI.md](./CLI.md)
- Base at plan authoring: `8c0989a5a3f82029a020330c5b18b311a20d1efc` (`github/main`)
- Sibling reference at plan authoring: local `psionic` checkout (not a pin)

```text
CLAIM
actor/session: docs/psionic plan authoring
base: 8c0989a5a3f82029a020330c5b18b311a20d1efc
worktree/branch: psionic-inference-docs / main
scope: owner intent and first plan for in-binary Qwen 3.8 local inference
paths: docs/psionic/
hot files: none
hot contracts: none
verification: docs-only; whitespace and path-local review
claimed_at: 2026-08-29T15:47:00Z
```

## Current facts

Coder local is Ollama. `Lane::Local` in `crates/openagents-cli` talks to
`GET /api/tags` and `POST /api/chat`. There is no `inference` or `psionic`
subcommand.

Psionic already has a Qwen 3.8 program (CPU and CUDA serving
`implemented_early`, Metal `partial` as of the 2026-08-28 audit). That code
lives in the sibling repo. This repo does not depend on it.

`psionic-serve` as a crate is the wrong import unit. Its `Cargo.toml` pulls
cluster, train, eval, research, router, catalog, and every backend. Importing
it would recreate the 2026-08-26 size and crash-sharing objections. Extract a
**leaf library** instead.

## Target shape

```text
openagents (one binary)
  ├── inference   product: run (teach path), models, serve, status, Coder
  ├── psionic     harness: inspect, admit, backends, library diagnostics
  └── coder       in-process LocalInferenceProvider → psionic-qwen38
                        │
                        ▼
              crates/psionic-*  (Apache-2.0, narrow graph)
                        │
                        ▼
         ~/.openagents/inference/models/<digest>.gguf
```

In-process is the Coder path. `openagents inference serve` may still bind a
loopback OpenAI-compatible HTTP server **in this process** so other tools and
protocol tests share the same library. That HTTP surface is optional for the
first Coder green. It is not a second engine distribution.

## CLI surface

Normative command list, teach mode, `--until` ladder, and the exact
status strings are [CLI.md](./CLI.md). This section is the summary.

Add two root commands next to the existing `Commands` enum in
`crates/openagents-cli/src/cli.rs`.

### `openagents inference`

Product lifecycle and the visible in-process loop. No training. No mesh.

```text
openagents inference run [--gguf <path>] [--prompt <text>] [--until <step>] [--preview]
openagents inference models
openagents inference add <path-or-approved-ref>
openagents inference remove <model>
openagents inference serve [--model <id>] [--backend auto|metal|cpu|cuda] [--port <n>]
openagents inference status
openagents inference stop
openagents inference unload
openagents inference doctor
```

`run` is the first command to implement. It prints the canonical status
script (Looking for GGUF, Loading GGUF, Reading metadata, … through
Inference complete). Teach mode is the default on `run` until Coder is
the product path. Each slice implements the next `--until` step. A step
that is not in the binary prints as not built and stops.

`add` inspects with the imported GGUF admission code, writes a manifest
(digest, family, backend list, license ack), and copies into the model
store. It does not download unless the reference is on an allowlist with
an expected digest.

`serve` loads the admitted artifact on the chosen backend and binds
`127.0.0.1`. Default refuse `0.0.0.0`. Coder does not have to use this
HTTP port; the same load path is a Rust API.

`stop` applies to a background serve started by this command. An
in-process Coder turn does not need a separate daemon.

### `openagents psionic`

Library and harness. This is how we grow the imported ML code without stuffing
every diagnostic into `inference`.

```text
openagents psionic inspect <artifact>
openagents psionic admit <artifact>
openagents psionic backends
openagents psionic doctor
```

Later, without expanding product scope: fixture decode, tokenizer parity,
short greedy probes. Those stay under `psionic` until they are productized.
The product-visible loop, including teach statuses, stays on
`inference run` ([CLI.md](./CLI.md)).

### Coder selection

During migration:

| User input | Engine |
| --- | --- |
| `--model ollama:<name>`, `--lane local` as today | Ollama |
| `--model psionic:<id>` | In-process Psionic |
| `--local` after the default switch | Psionic, once gates pass |

Do not make `Lane::Local` mean a vendor name forever. Prefer `local` as
"configured local engine" once Psionic is default.

## What to import

Copy selected crates into this workspace (path deps), keep Apache-2.0 on those
crates, and record source repo + commit in `docs/psionic/PROVENANCE.md` in the
same commit as the first copy.

**Candidate include** (trim after a compile graph is measured; do not copy a
crate that is only needed because `psionic-serve` pulled the world):

| Area | Likely crates | Why |
| --- | --- | --- |
| Core / IR / runtime | `psionic-core`, `psionic-ir`, `psionic-runtime` | Shared execution types |
| Models / Qwen 3.8 | `psionic-models` **or an extracted qwen38 module** | Family, tokenizer, GGUF layout |
| CPU | `psionic-backend-cpu` | Correctness and unsupported-GPU hosts |
| Metal | `psionic-backend-metal` | First laptop backend (Apple Silicon) |
| Serve leaf | new `psionic-local-infer` in this repo | Chat/stream/tool API with no cluster/train |

**Exclude on sight** unless a later packet names them:

`psionic-train`, `psionic-cluster`, `psionic-distributed`, `psionic-collectives`,
`psionic-eval`, `psionic-research`, `psionic-tassadar-student`, `psionic-sandbox`,
`psionic-mlx-*`, `psionic-csm-speech`, `psionic-vad`, `psionic-apple-fm`,
`psionic-router` (mesh), `psionic-catalog` if it discovers every family
rather than one artifact.

CUDA (`psionic-backend-cuda`) is a second-platform packet. Do not make it a
default feature of the macOS CLI artifact.

If `psionic-models` cannot be copied without dragging training, split a
`psionic-qwen38` crate in this repo and port only the Qwen 3.8 decoder,
tokenizer, and GGUF admission. Prefer a split over a silent whole-monorepo
import.

## Artifact contract

Before Coder claims "same as Ollama":

1. Inspect the live Ollama blob for `qwen3.8:27b-mtp-q8_0` (size, SHA-256,
   GGUF architecture, MTP tensors).
2. Compare to Psionic's admitted Unsloth `Qwen3.8-27B-UD-Q3_K_XL.gguf`
   (different quant; not interchangeable by name).
3. Pick **one** first OpenAgents artifact. Record digest, license, context
   limit, and whether MTP is used or ignored.
4. `inference add` refuses unknown required tensors. Do not alias the file to
   Qwen 3.5 because the architecture string looks close.

The first Coder-green artifact may be a small Qwen 3.8 (or shared-family)
fixture if the 27B file is too large for CI. The 27B laptop path is a later
gate on development hardware, not a unit-test blob.

## Binary and release

- Workspace license remains MIT for original OpenAgents crates.
- Imported crates stay Apache-2.0. Ship `NOTICE` (or equivalent) from the CLI
  package.
- Cargo features on `openagents-cli`, for example `inference-metal` and
  `inference-cpu`. macOS aarch64 release enables Metal+CPU. Other targets
  enable CPU or compile a stub that `inference doctor` explains.
- `ops/release-cli.sh` stays one CLI artifact per platform. Inference code
  rides inside that artifact. Model weights do not.
- First **supported** local-inference platform: `macos-aarch64`. Naming a
  platform in the seven-target matrix is not the same as supporting Psionic
  there.

## Coder integration

Refactor the local tool loop around a provider trait **before** deleting
Ollama:

- discovery, stream events, tool-call IDs, usage, cancel
- shared: history, max tool steps, ATIF, final-answer ordering

Implement `PsionicLocalProvider` in-process (Rust API). Keep
`OllamaLocalProvider` on HTTP. Coder does not print teach lines unless
you opt in; `inference run` remains the teaching surface.

ATIF records: canonical model id, artifact digest, backend, engine version
(CLI version is enough while the library is in-tree), locality `local`, token
counts, finish reason.

Streaming: re-verify Psionic Qwen 3.8 against the 2026-08-26 "completed then
wrap" finding before Coder depends on token-time events. If the imported
executor still completes first, either fix it in the imported crate or refuse
Coder interactive mode until it streams.

OpenAgents issues 344–347 track the product surfaces on this path.
Issue 344 is `inference run` through `map.done`. Issue 345 is Coder TUI
load progress through Weights ready (throttle like `prefill.pos`; the
full teach essay stays out of the chat transcript unless the user opts
in). Issue 346 is unload. Issue 347 is memory visualization. `--model
psionic:` generate remains stage 4.

## Stages

### 0. This folder

Intent, plan, llama.cpp byte path, CLI teach script.

### 1. Provenance and compile graph

Pin a Psionic revision. List every crate that must copy for a Qwen 3.8 CPU
fixture to compile. Write `PROVENANCE.md`. CLI may land a `--preview`
stub that prints pending statuses with no library yet.

**Exit:** `cargo tree` on the leaf crate has no train/cluster/mlx crates.

### 2. Import, find GGUF, read metadata

Copy the allowlisted crates. Implement `openagents inference run --gguf`
through `--until meta.done` (Looking for GGUF → Loading GGUF → Reading
metadata → tensor index). `openagents psionic inspect` and `admit` on a
fixture GGUF. Tests: malformed GGUF refuses; admitted fixture reports
family and digest.

**Exit:** fixture admission is deterministic in CI. `run --until
meta.done` prints the canonical statuses on a fixture file.

Stage 2 still owns parse. OpenAgents issue 344 continues from here
through `map.done`.

### 3. `openagents inference` + in-process generate

Tokenizer, mmap/Metal, context, prefill, decode (`--until gen.done`).
Stage 3 still owns mmap. OpenAgents issue 344 is `inference run` through
`map.done`. Issues 345–347 (Coder TUI load UI, unload, memory) sit on
top of `map.done` and run in parallel with generate. Those three landed
(`inference unload`, `status --json` memory fields, Coder `/load` and
`/unload`). Generate (`ctx.done` through `gen.done`) is still open.

Model store, `add`/`models`/`doctor`. Optional loopback `serve` if it is
cheaper than a second API.

**Exit:** `openagents inference run --prompt …` prints `Inference
complete` on the fixture. `openagents inference doctor` is green on a
machine with the fixture.

### 4. Coder tool loop

`PsionicLocalProvider`. Multi-round shell-tool fixture. Ollama tests still
pass.

**Exit:** headless Coder completes a tool turn with `--model psionic:<fixture>`.

### 5. Laptop Qwen 3.8

Admit the chosen 27B-class GGUF on Metal (or CPU if Metal is still partial).
Memory admission before load. Token-time streaming and cancel. Measure against the
current Ollama path on the same host. No default flip yet.

**Exit:** recorded latency, memory, and a Coder tool-loop receipt on the
development Mac.

### 6. Default local

`--local` → Psionic. Keep explicit `ollama:`. Remove Ollama only after a
deprecation window and the replacement gates below.

## Replacement gates (default local)

All must be true for the chosen Qwen 3.8 artifact:

1. Admitted under a Qwen 3.8 family id, digest published.
2. Tokenizer, prompt, short greedy, tools, and continuation tests pass.
3. Token-time streaming and cancel work in Coder interactive, plain, and
   headless.
4. Metal (or the admitted laptop backend) stays within memory policy on the
   supported Mac class.
5. No silent fallback. ATIF names the engine that ran.
6. NOTICE / Apache-2.0 attribution ships with the binary.
7. Explicit `ollama:` still works until the deprecation commit.

## Verification

Implementation packets run the repository completion gate:
`cargo fmt --all -- --check` then `cargo test --workspace`.

Slice tests while iterating (named, not called "the gate"):

- `cargo test -p openagents-cli --test …` for CLI/Coder
- crate-local tests on imported `psionic-*` crates
- status-script tests: `inference run --json` emits the `id` values in
  [CLI.md](./CLI.md) for the `--until` step under test

Do not put 27B weights in CI. Use a tiny fixture plus one optional
operator-only laptop receipt.

## AGENTS.md and Cloud

When the first crate lands, update `AGENTS.md` in that commit: this repo may
contain the narrow Psionic local-inference crates, and they are distinct from
Cloud `oa-node` worker attachment.

Do not route Qwen 3.8 Coder work through `oa-node psionic attach`. That
adapter stays Cloud capacity advertising.

## Out of scope for the first landing

- Training, adapters, GEPA, Tassadar
- Mesh / cluster serving
- Making Psionic the Cloud inference product
- Deleting Ollama in the same commit as the first import
- Publishing a second engine tarball
- Claiming Psionic is faster than Ollama on Qwen 3.8 until stage 5 measures it
