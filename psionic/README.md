# Psionic

Psionic is a machine-learning library written in Coax, built one runnable book
chapter at a time. The first milestone trains a one-parameter model on
`[1, 2] → [2, 4]`, then predicts for an unseen input `3`. Chapter 2 adds
bounded shapes, row-major indexing and a staged copy into caller-owned storage.
Chapter 3 adds checked CPU numerical operations over that storage.

Start with [the book](book/README.md) and
[Chapter 1: Learning a number](book/ch01-learning-a-number.md), then
[Chapter 2: Shapes and storage](book/ch02-shapes-and-storage.md) and
[Chapter 3: Numbers and matrices](book/ch03-numbers-and-matrices.md).
[The design notes](../docs/psionic/README.md) explain the larger plan.

From the repository root:

```shell
psionic/check.sh
coax/bin/coax run psionic/examples/learn-number.coax
coax/bin/coax run psionic/examples/shape-storage.coax
coax/bin/coax run psionic/examples/numbers-matrices.coax
```

The check command builds the compiler if it is absent, runs authored Coax
checks in debug/release and the authority mutation gates, compares independent
native numerical oracles in debug/release with and without ASan/UBSan, and
verifies book listings, local links, example output and exit status. It uses
a C11 compiler and Python 3 for book tooling and independent Decimal fixtures.
Neither the library nor its examples require Python, models, services,
credentials, or downloads at run time.

| Path | Role |
| --- | --- |
| [lib/learn.coax](lib/learn.coax) | Checks first, admission, loss/gradient, fixed-rate SGD, prediction. |
| [lib/tensor.coax](lib/tensor.coax) | Checked rank/shape/byte/stride metadata, indexing and staged finite-f32 copy. |
| [lib/ops-contract.coax](lib/ops-contract.coax) | Operation shapes, typed view counts, epsilon rules and matrix-work admission. |
| [lib/ops.coax](lib/ops.coax) | Add, row sums/dots, matmul/matvec, gather, masked softmax, unscaled RMSNorm/LayerNorm; f32 storage with compensated f64 reductions. |
| [platform/ops-memory.h](platform/ops-memory.h) | Native operations wrapper; typed span observations include integer indices and byte masks. |
| [platform/tensor-memory.h](platform/tensor-memory.h) | Native tensor caller interface, alongside generated `tensor.h`; its C adapter observes addresses through shared [spans.h](platform/spans.h). |
| [platform/memory.c](platform/memory.c) | Observes native span overlap and calls Coax; no numerical implementation. |
| [platform/memory.h](platform/memory.h) | Native caller interface, alongside the generated `learn.h`. |
| [examples/](examples/) | Training, storage and numerical examples using the shared library. |
| [tests/native.c](tests/native.c) | Independent closed-form/quadratic oracles and actual storage/representation cases. |
| [tests/tensor.c](tests/tensor.c) | Independent layouts/indices, actual aliases/short allocations, float representations and a maximum-size copy. |
| [tests/ops.c](tests/ops.c), [tests/ops_oracle.py](tests/ops_oracle.py) | Independent Decimal fixtures, analytic magnitude/cancellation cases and real native storage. |
| [book/examples.json](book/examples.json) | Complete runnable example inventory and expected statuses. |
| [fixtures/](fixtures/) | Stable teaching transcripts. |

Current scope is synchronous CPU execution on the tested Apple LP64 host:
1–16 paired samples, 1–64 updates, a fixed learning rate `1/8`, and inputs,
targets, initial and candidate weights in `[-16, 16]`. Prediction accepts a
weight and input in that range. The book documents capacities, failure
publication, arithmetic, and lifetime obligations. Chapter 2 admits contiguous
`f32` shapes of rank 0–4 with positive active extents and at most 1,048,576
elements (4 MiB of logical data). The input view is exactly the logical tensor;
destination/scratch may have spare capacity. Copy accepts all finite binary32
values, preserves unused capacity, and refuses before any write. Chapter 3
implements the named `CPU-f32-wide-v1` profile for bounded rows/matrices, with
a 1,048,576 multiply-add budget per matmul. It validates finite inputs before
staging; arithmetic overflow may dirty scratch while preserving destination.
[Its chapter](book/ch03-numbers-and-matrices.md) defines exact shapes, masks,
normalization, precision and error contracts. Autodiff, model import, GPU
execution and serving remain planned.
