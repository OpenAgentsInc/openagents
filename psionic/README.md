# Psionic

Psionic is a machine-learning library written in Coax, built one runnable book
chapter at a time. The first milestone trains a one-parameter model on
`[1, 2] → [2, 4]`, then predicts for an unseen input `3`.

Start with [the book](book/README.md) and
[Chapter 1: Learning a number](book/ch01-learning-a-number.md).
[The design notes](../docs/psionic/README.md) explain the larger plan.

From the repository root:

```shell
psionic/check.sh
coax/bin/coax run psionic/examples/learn-number.coax
```

The check command builds the compiler if it is absent, runs authored Coax
checks and the authority mutation gate, compares independent native numerical
oracles in debug/release with and without ASan/UBSan, and verifies book listings,
local links, example output and exit status. It uses a C11 compiler and Python
3 for book tooling. Neither the library nor the example requires Python,
models, services, credentials, or downloads at run time.

| Path | Role |
| --- | --- |
| [lib/learn.coax](lib/learn.coax) | Checks first, admission, loss/gradient, fixed-rate SGD, prediction. |
| [platform/memory.c](platform/memory.c) | Observes native span overlap and calls Coax; no numerical implementation. |
| [platform/memory.h](platform/memory.h) | Native caller interface, alongside the generated `learn.h`. |
| [examples/](examples/) | Successful training and a short-buffer refusal, using the same library. |
| [tests/native.c](tests/native.c) | Independent closed-form/quadratic oracles and actual storage/representation cases. |
| [book/examples.json](book/examples.json) | Complete runnable example inventory and expected statuses. |
| [fixtures/](fixtures/) | Stable teaching transcripts. |

Current scope is synchronous CPU execution on the tested Apple LP64 host:
1–16 paired samples, 1–64 updates, a fixed learning rate `1/8`, and inputs,
targets, initial and candidate weights in `[-16, 16]`. Prediction accepts a
weight and input in that range. The book documents capacities, failure
publication, arithmetic, and lifetime obligations. Tensors, autodiff, imported
checkpoints, GPU execution, and serving remain planned.
