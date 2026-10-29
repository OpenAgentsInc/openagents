# Psionic in Coax

Status: M0 implemented, 2026-10-28; later milestones are proposed. The
[Coax library and first book chapter](../../psionic/README.md) train and run a
bounded one-parameter CPU model. Run `psionic/check.sh` from the repository
root. [The verification record](../verification/psionic-m0-2026-10-28.md)
states the tested scope. The retired Rust workspace remains historical input.

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

The planning documents live here. The root [psionic/](../../psionic/README.md)
directory contains `lib/`, `book/`, `examples/`, `platform/`, `tests/`, and
small fixtures. Its examples call the same library as its native tests.

Keep the ML book separate from [the Coax language book](../../coax/book/README.md).
The language book teaches Coax; the Psionic book teaches building an ML
library with it. Cross-link the relevant language chapters instead of
repeating their reference material. Decide publication after the first
runnable chapters exist; writing the book does not require a new website.

## Where to start

[Chapter 1](../../psionic/book/ch01-learning-a-number.md) answers one question:
**how does a program learn the number 2 from examples of `y = 2x`?** It uses
two training examples, one parameter, an explicit loss, and a hand-derived
gradient. It checks the first update exactly, trains for sixteen steps, and
predicts on a held-out input. The same Coax implementation runs through its
authored checks and a compiled native host.

The next milestone adds bounded tensor shape/storage admission and scalar CPU
matrix operations. Subsequent chapters replace the hand-written gradient with
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
