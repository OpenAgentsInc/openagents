# Research and lessons

Read on 2026-10-28. These are source observations and recommendations for a
new Coax implementation. Historical results do not qualify new code.
The sibling repositories were inspected read-only; their build, training,
download, and deployment scripts were not executed.

## Source pins

| Source | Revision inspected | Entry points |
| --- | --- | --- |
| OpenAgents before the reset | `853417f290e7cae1df18c081b89b0818432e60af` (`0b401561a0^`) | `docs/psionic/`, `docs/psionic-and-pylon.md`, `crates/psionic/README.md`, and selected backend history. |
| Standalone Psionic | `a69c7abb0160efff9b1bbb3c6ace84ee7b0a04d9` | `README.md`, `AGENTS.md`, architecture, workspace map, framework-core acceptance, backends, tensor/core and CPU backend source. |
| ML book | `88610bb1b233e297eced7d630ddc2647ce1e196b` | README, chapter index, authoring instructions, numerical chapter, chapter sources/Makefiles, temporary engine note, and test runner. |
| Current Coax | OpenAgents `21bccc7fd84d5d382e8fe4a666e31eec654d1898` | `coax/README.md`, `coax/STATUS.md`, `coax/book/`, and `INVARIANTS.md`; inventory names coax0 0.16.0. |

Sibling paths are research locations, not build dependencies. Pinned remote
links below let a reader follow the evidence without those checkouts.

## OpenAgents history

The name has been used for different projects. The June 2025 history includes
a Psionic web framework (`43ed67ab25`, `01d867c3d9`). This proposal concerns
the later ML stack.

The [pre-reset import README](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/crates/psionic/README.md)
records a 2026-10-07 import of the serving dependency closure: 30 crates from
standalone revision `02e0bc85fe6a3c87dd45927314e6950c74c642a9`. Training
programs, research lanes, and reports were excluded. The standalone checkout
and the later monorepo serving code therefore represent different scopes;
neither alone describes the whole history.

The [historical overview](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/docs/psionic/README.md)
maps CPU/CUDA/Metal serving, tokenizer/template qualification, native small
models, and later Flint and DeepSeek work. The
[implementation roadmap](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/docs/psionic/implementation-roadmap.md)
distinguishes grouped HTTP requests from actual device batching, and a
streaming interface from tokens delivered while generation is running.
Those are useful warnings for any rewrite: inspect what executes under an
API before attributing a capability to it.

The [single-machine V4.1 notes](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/docs/psionic/v41-single-box.md)
separate exact storage/cache changes from lossy expert requantization and
give them distinct profiles. The
[completion checklist](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/docs/psionic/deepseek-v41-completion.md)
retains failed numerical gates and records cancellation with qualification
unfinished. Runtime recovery checks and plausible answers did not close those
gates. Carry that distinction into the new book's status and release claims.

A smaller example is
[the partial-row fix, `5e867572f3`](https://github.com/OpenAgentsInc/openagents/commit/5e867572f3b585e82722610918470bf83d8d6cab).
The CPU decoder had to reject a quantized row whose byte count was not a whole
number of blocks, without changing its output. This is a good eventual loader
chapter exercise: malformed input must fail before publishing partial state.

Read deleted files without restoring them:

```sh
git show '0b401561a0^:docs/psionic/README.md'
git show '0b401561a0^:docs/psionic/implementation-roadmap.md'
git show '0b401561a0^:crates/psionic/README.md'
git show 5e867572f3 -- crates/psionic/crates/psionic-backend-cpu/src/lib.rs
```

## Standalone Psionic

The [architecture](https://github.com/OpenAgentsInc/psionic/blob/a69c7abb0160efff9b1bbb3c6ace84ee7b0a04d9/docs/ARCHITECTURE.md)
and [workspace map](https://github.com/OpenAgentsInc/psionic/blob/a69c7abb0160efff9b1bbb3c6ace84ee7b0a04d9/docs/WORKSPACE_MAP.md)
cover tensors, IR/compiler/runtime, backends, serving, training, data/eval,
artifacts, and distributed execution. Psionic owns execution; app UX, wallets,
and final settlement have other owners. Keep that boundary in Coax while
starting with far fewer modules.

The [framework-core acceptance matrix](https://github.com/OpenAgentsInc/psionic/blob/a69c7abb0160efff9b1bbb3c6ace84ee7b0a04d9/docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md)
explicitly separates framework completeness from serving and training
benchmark acceptance. Its categories are a useful later coverage map: tensor
semantics, autodiff/optimizers, model/state I/O, lowering/realization,
memory/cache behavior, replay identity, and local multi-device execution.
They should not become seven large subsystems before the first model works.

The [core source](https://github.com/OpenAgentsInc/psionic/blob/a69c7abb0160efff9b1bbb3c6ace84ee7b0a04d9/crates/psionic-core/src/lib.rs)
contains concrete shape, broadcast, stride/span, dtype, and refusal contracts;
the [CPU backend](https://github.com/OpenAgentsInc/psionic/blob/a69c7abb0160efff9b1bbb3c6ace84ee7b0a04d9/crates/psionic-backend-cpu/src/lib.rs)
executes graph operations and tests numerical and quantized paths. Use these
as questions and regression inputs. Re-derive the small Coax contract rather
than translating Rust types, allocation patterns, or the entire crate map.
Old implementation output is a compatibility oracle, and can share bugs with
new output; supplement it with independent arithmetic expectations.

## The year-old ML book

The [chapter index](https://github.com/AtlantisPleb/ml-book/blob/88610bb1b233e297eced7d630ddc2647ce1e196b/chapters/README.md)
progresses from numbers, matrices, attention, tokenization, and small trainers
to GPT-OSS, deployment, and agents. The
[numerical chapter](https://github.com/AtlantisPleb/ml-book/blob/88610bb1b233e297eced7d630ddc2647ce1e196b/chapters/02-numbers-behave/README.md)
starts from understandable operations, states tolerances, and supplies tests.
Reuse that teaching approach, including learning examples early in the book.

The [authoring workflow](https://github.com/AtlantisPleb/ml-book/blob/88610bb1b233e297eced7d630ddc2647ce1e196b/CLAUDE.md)
scaffolds chapter-local headers and source, with shared-library extraction
later. The [temporary engine note](https://github.com/AtlantisPleb/ml-book/blob/88610bb1b233e297eced7d630ddc2647ce1e196b/engine/README.md)
acknowledges a separate runtime used by chapter demos and excluded from the
chapter harness. At the inspected revision,
[22A's source](https://github.com/AtlantisPleb/ml-book/blob/88610bb1b233e297eced7d630ddc2647ce1e196b/chapters/22A-prefill-decode/src/ch22A_prefill_decode.c)
explicitly performs head-only decode without attention/MoE, while
[24's Makefile](https://github.com/AtlantisPleb/ml-book/blob/88610bb1b233e297eced7d630ddc2647ce1e196b/chapters/24-chat-interface/Makefile)
still links the temporary engine. The
[aggregate runner](https://github.com/AtlantisPleb/ml-book/blob/88610bb1b233e297eced7d630ddc2647ce1e196b/run_all_tests.sh)
ends at chapter 25. These observations bound what that harness establishes;
they are not results from rerunning it.

For Coax, use one library from the beginning, an enumerated example manifest,
and a cumulative end-to-end example. Label simplified quantization layouts
and partial-model demonstrations with their exact scope. A top-five logit
demo is useful teaching material but does not establish a full decoder.

## The Coax book

The [book README](../../coax/book/README.md) distinguishes runnable features
from planned syntax. [Its runner](../../coax/book/check-examples.sh) compares
output and exit status in debug/release, executes authored checks and native
host examples, and checks structure. These are strong starting practices for
the Psionic book. Numeric results will additionally need tolerance-aware
comparisons, with exact transcripts reserved for deliberately stable output.

[Chapter 10](../../coax/book/ch10-organizing-programs.md) explains source
composition and physical authority ownership.
[Chapter 15](../../coax/book/ch15-asynchronous-work.md) separates cancellation
from memory lifetime. [Chapter 23](../../coax/book/ch23-verification-boundaries.md)
separates finite checks, mutation sensitivity, native behavior, and actual
authorization. These distinctions matter directly for tensors, GPU buffers,
and model admission.

The [GPU chapter](../../coax/book/ch17-gpu.md) documents a scalar Metal
profile, not a tensor compute backend. The
[current inventory](../../coax/STATUS.md) keeps generics, dynamic arrays,
general ownership, and several target features outside the implemented
subset. Recheck the inventory when a chapter starts: source pins record this
survey, not a promise that the compiler will remain at this version.
