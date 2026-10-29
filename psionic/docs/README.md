# Psionic in Coax

Status, 2026-10-29: chapters 1–10 implement M0/M1, bounded original-weight
artifacts, text requests, small Qwen-style layers and authenticated
original-weight Elephant text decisions on the tested CPU host. Mammoth and
autodiff remain proposed. The
[Coax library and book](../README.md) train a bounded scalar CPU model
and implement bounded contiguous storage plus dense row/matrix numerics.
Run `psionic/check.sh` from the repository root.
[Chapter 1 verification](../../docs/verification/psionic-m0-2026-10-28.md),
[chapter 2 verification](../../docs/verification/psionic-ch02-2026-10-28.md), and
[chapter 3 verification](../../docs/verification/psionic-ch03-2026-10-28.md), and
[chapter 4 verification](../../docs/verification/psionic-ch04-2026-10-29.md), and
[chapter 5 verification](../../docs/verification/psionic-ch05-2026-10-29.md) state the
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
| [Hybrid kernel progress](hybrid-kernels.md) | Bounded Qwen-style recurrent, attention and original-weight projection primitives; current qualification and remaining work. |
| [Architecture and considerations](architecture.md) | What belongs in Coax, storage and numerical contracts, compiler limits, and the eventual backend boundary. |
| [Starting point and roadmap](roadmap.md) | A concrete first change, a chapter sequence, dependencies, and acceptance gates. |
| [Production inference notes](serving/inference-engineering.md) | What Psionic takes from *Inference Engineering* (Baseten): service metrics, batching, caching, routing, autoscaling, rollout and observability, mapped to Coax chapters, and what it does not adopt. |
| [DeepSeek natively](targets/deepseek.md) | The pre-reset DeepSeek V4.1 Flash work condensed: the architecture, what was achieved and measured, why numerical parity failed, the efficiency techniques to rebuild in Coax, and what Psionic does not adopt. |
| [Mistral target survey](targets/mistral.md) | The Mistral ladder from Ministral 3 to Mistral Large 4 (Le Chonk): pinned revisions, licences, the features each model adds and Large 4's model-as-a-service licence boundary. |
| [Inference across a mesh](serving/mesh.md) | Mesh-LLM lessons carried forward and updated, Darkbloom and private-inference trust lessons, and re-execution checks for peers. |
| [Decisions in production](decisions.md) | Notes from the Jev, System One and decision-model research: calibration as the consumer's job, cost-derived thresholds, outcome joins, checking honest machines against substitutes, millisecond encoder decisions and shadow promotion. |
| [A harness for Psionic models](harness.md) | Notes from DeepSeek Harness: the turn and step loop, a session log that rebuilds every request, the byte-stable prefix that keeps the server's cache hot, tools, sandbox, compaction, forks, plugins and protocols, and where Psionic differs (a simple default loop, Coax, consent before sharing). |
| [What Psionic takes from llama.cpp](llama-cpp.md) | Designs to reimplement in Coax by chapter (attention, chunked DeltaNet, fusion, integer products, GGUF, quantization, tokenizers, serving), independent fixtures, format notes, fair benchmarking and licensing. |
| [gpt-oss target survey](targets/gpt-oss.md) | Pinned gpt-oss-20b/120b and harmony sources: MXFP4 originals, sink/window attention, expert routing, the response format, and what Psionic does not adopt. |
| [NVIDIA breadth and target survey](targets/nvidia-breadth.md) | Proposed retrieval, vision, motion, speech, physics, hybrid/MoE, world-model and robotics studies; pinned sources and data/terms distinctions. |
| [Development process](process.md) | How prose, checks, library code, examples, and evidence move together. |

The planning documents live here, in `psionic/docs/`: project notes at the
top, model surveys in `targets/` and hosting and mesh notes in `serving/`. The
rest of [psionic/](../README.md) contains `lib/`, `book/`, `examples/`,
`platform/`, `tests/`, and small fixtures. Its examples call the same library
as its native tests. Verification records stay with the repository's others in
[docs/verification/](../../docs/verification/).

Keep the ML book separate from [the Coax language book](../../coax/book/README.md).
The language book teaches Coax; the Psionic book teaches building an ML
library with it. Cross-link the relevant language chapters instead of
repeating their reference material. Decide publication after the first
runnable chapters exist; writing the book does not require a new website.

## Current priority

The [Psionic project](https://openagents.com/projects/psionic) tracks work as
signed OpenAgents issues; [the issue map](project.md) links every task and
records the two ready starting points. [Elephant and Mammoth](targets/model-targets.md) are the first
real-weight targets, delivered ahead of general training work. The
[nanochat study](targets/nanochat.md) separates checkpoint inference, pretraining, SFT
and reward training; [the book plan](../book/PLAN.md) maps their
reading order. The expanded contents also proposes NVIDIA studies beyond
language: retrieval, vision, motion, speech, scientific ML, world generation
and robot policies. [The survey](targets/nvidia-breadth.md) explains selection and
qualifications. New breadth is proposed; this update adds no project issues.
Implementation readiness is each branch's prerequisites, not chapter number.

## Where to start

[Chapter 1](../book/ch01-learning-a-number.md) answers one question:
**how does a program learn the number 2 from examples of `y = 2x`?** It uses
two training examples, one parameter, an explicit loss, and a hand-derived
gradient. It checks the first update exactly, trains for sixteen steps, and
predicts on a held-out input. The same Coax implementation runs through its
authored checks and a compiled native host.

[Chapter 2](../book/ch02-shapes-and-storage.md) now implements bounded
shape/storage admission, row-major indexing and staged copy.
[Its verification](../../docs/verification/psionic-ch02-2026-10-28.md) records the tested
scope. [Chapter 3](../book/ch03-numbers-and-matrices.md) now adds
CPU matrix/reduction/gather/softmax/normalization operations under an explicit
wide-accumulation profile. [Chapter 4](../book/ch04-reading-original-weights.md)
now adds original Safetensors/BF16/F32 inventory, exact-byte identities, complete
shard/index/manifest admission and bounded finite decoding.
[Chapter 5](../book/ch05-text-becomes-a-model-request.md) now implements
the frozen text renderer, Unicode-17 NFC, original-Qwen byte-BPE and shared
token-prefix layout under explicit budgets. [Chapter 6](../book/ch06-attention-without-training.md) adds a small causal attention block. [Chapter 7](../book/ch07-a-hybrid-sequence-model.md) composes
causal Qwen-style linear and full-attention layers. [Chapter 8](../book/ch08-decisions-have-a-different-mask.md) adds decision blocks and hybrid state forks; real-checkpoint qualification remains next.
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
