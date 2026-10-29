# Psionic in Coax

Status, 2026-10-29: chapters 1–4 implement M0/M1 and the bounded original-weight
artifact slice of M5 on the tested CPU host. Autodiff and full model execution
remain proposed. The
[Coax library and book](../../psionic/README.md) train a bounded scalar CPU model
and implement bounded contiguous storage plus dense row/matrix numerics.
Run `psionic/check.sh` from the repository root.
[Chapter 1 verification](../verification/psionic-m0-2026-10-28.md),
[chapter 2 verification](../verification/psionic-ch02-2026-10-28.md), and
[chapter 3 verification](../verification/psionic-ch03-2026-10-28.md), and
[chapter 4 verification](../verification/psionic-ch04-2026-10-29.md) state the
tested scopes. The retired Rust workspace remains historical input.

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
| [NVIDIA breadth and target survey](nvidia-breadth.md) | Proposed retrieval, vision, motion, speech, physics, hybrid/MoE, world-model and robotics studies; pinned sources and data/terms distinctions. |
| [Development process](process.md) | How prose, checks, library code, examples, and evidence move together. |

The planning documents live here. The root [psionic/](../../psionic/README.md)
directory contains `lib/`, `book/`, `examples/`, `platform/`, `tests/`, and
small fixtures. Its examples call the same library as its native tests.

Keep the ML book separate from [the Coax language book](../../coax/book/README.md).
The language book teaches Coax; the Psionic book teaches building an ML
library with it. Cross-link the relevant language chapters instead of
repeating their reference material. Decide publication after the first
runnable chapters exist; writing the book does not require a new website.

## Current priority

The [Psionic project](https://openagents.com/projects/psionic) tracks work as
signed OpenAgents issues; [the issue map](project.md) links every task and
records the two ready starting points. [Elephant and Mammoth](model-targets.md) are the first
real-weight targets, delivered ahead of general training work. The
[nanochat study](nanochat.md) separates checkpoint inference, pretraining, SFT
and reward training; [the book plan](../../psionic/book/PLAN.md) maps their
reading order. The expanded contents also proposes NVIDIA studies beyond
language: retrieval, vision, motion, speech, scientific ML, world generation
and robot policies. [The survey](nvidia-breadth.md) explains selection and
qualifications. New breadth is proposed; this update adds no project issues.
Implementation readiness is each branch's prerequisites, not chapter number.

## Where to start

[Chapter 1](../../psionic/book/ch01-learning-a-number.md) answers one question:
**how does a program learn the number 2 from examples of `y = 2x`?** It uses
two training examples, one parameter, an explicit loss, and a hand-derived
gradient. It checks the first update exactly, trains for sixteen steps, and
predicts on a held-out input. The same Coax implementation runs through its
authored checks and a compiled native host.

[Chapter 2](../../psionic/book/ch02-shapes-and-storage.md) now implements bounded
shape/storage admission, row-major indexing and staged copy.
[Its verification](../verification/psionic-ch02-2026-10-28.md) records the tested
scope. [Chapter 3](../../psionic/book/ch03-numbers-and-matrices.md) now adds
CPU matrix/reduction/gather/softmax/normalization operations under an explicit
wide-accumulation profile. [Chapter 4](../../psionic/book/ch04-reading-original-weights.md)
now adds original Safetensors/BF16/F32 inventory, exact-byte identities, complete
shard/index/manifest admission and bounded finite decoding. Next are tokenizer
semantics and the early Elephant/Mammoth forward path.
The learning path replaces the hand-written gradient with autodiff over those
same operations, then builds small physical and language models.
The [roadmap](roadmap.md) specifies the boundaries of each step.

## Decisions for the first iteration

- One authored Coax implementation per responsibility. Chapter programs call
  the library; small derivations are clearly identified as teaching steps.
- CPU first, `f32` storage with named accumulation profiles, caller-owned
  buffers, synchronous execution, and a narrow contiguous layout. Generality
  follows working uses.
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
