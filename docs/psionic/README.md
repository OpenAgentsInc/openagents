# Psionic in Coax

Status: proposal, 2026-10-28. This folder plans a fresh implementation. It
does not restore the Rust workspace deleted by `0b401561a0`, and no Coax ML
runtime is implemented by these documents.

Build Psionic as a Coax library whose book explains and exercises the actual
library as it grows. Start on the CPU with bounded storage, explicit numerical
contracts, and a tiny model that both learns and predicts. Grow toward reusable
tensor operations, automatic differentiation, text models, and acceleration.
Preserve the older Psionic's useful discipline around artifacts, capabilities,
refusals, and execution evidence throughout that progression.

## Read this folder

| Document | Purpose |
| --- | --- |
| [Research and lessons](research.md) | What we read in this repository's history, `~/work/psionic`, `~/work/ml-book`, and `coax/book/`; pinned sources and lessons. |
| [Architecture and considerations](architecture.md) | What belongs in Coax, storage and numerical contracts, compiler limits, and the eventual backend boundary. |
| [Starting point and roadmap](roadmap.md) | A concrete first change, a chapter sequence, dependencies, and acceptance gates. |
| [Development process](process.md) | How prose, checks, library code, examples, and evidence move together. |

The planning documents live here. Once implementation starts, the proposed
home is a root `psionic/` directory with `lib/`, `book/`, `examples/`,
`platform/`, `tests/`, and small fixtures. Those paths are proposals, not
existing commands or a new workspace scaffold in this change.

Keep the ML book separate from [the Coax language book](../../coax/book/README.md).
The language book teaches Coax; the Psionic book teaches building an ML
library with it. Cross-link the relevant language chapters instead of
repeating their reference material. Decide publication after the first
runnable chapters exist; writing the book does not require a new website.

## Where to start

Write the first chapter around one question: **how does a program learn the
number 2 from examples of `y = 2x`?** Use two training examples, one parameter,
an explicit loss, and a hand-derived gradient. Check the first update exactly,
train for a fixed number of steps, and predict on a held-out input. Run the
same Coax implementation through its authored checks and a compiled native
host. This is small enough to explain every operation and reaches training
and inference before framework construction dominates the work.

Alongside it, establish bounded shape/storage admission and a scalar CPU
matrix operation. Subsequent chapters replace the hand-written gradient with
autodiff over the shared operations, then build a byte-level language model.
The [roadmap](roadmap.md) specifies the boundaries of each step.

## Decisions for the first iteration

- One authored Coax implementation per responsibility. Chapter programs call
  the library; small derivations are clearly identified as teaching steps.
- CPU first, explicit `f32` operations, caller-owned storage, synchronous
  execution, and a narrow contiguous layout. Generality follows working uses.
- Training and inference share operations and model state. A usable serving
  wrapper does not establish a complete ML framework.
- Every implemented claim has a command and a bounded meaning. New backends,
  file formats, and model families earn their own evidence.
- Implement the first chapter and its verification loop before writing a
  large body of future-tense chapters. Later chapters begin as short plans.

Metal, larger imported models, quantization, serving, and distributed training
remain part of the direction. Each depends on earlier numerical and lifetime
contracts. The current [Coax feature inventory](../../coax/STATUS.md) also
places real limits on their implementation.
