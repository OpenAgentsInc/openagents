# Provenance: local GGUF load slice

- Class: implementation provenance
- Status: issues 344–347, 352–356 landed; #361 bench in this wave
- Date: 2026-08-29

This repository did not copy the Psionic monorepo. `crates/psionic-gguf` is
original OpenAgents code that implements the GGUF load contract in
[LLAMA_CPP_INFERENCE_PIPELINE.md](./LLAMA_CPP_INFERENCE_PIPELINE.md) and
[CLI.md](./CLI.md).

Studied, not vendored:

| Source | Revision | What we took |
| --- | --- | --- |
| llama.cpp (`projects/repos/llama.cpp`) | `3173a56471c1753650cd806694145ffd6dcace67` | GGUF layout, `no_alloc` metadata parse, mmap, Metal `newBufferWithBytesNoCopy` + `StorageModeShared` |
| OpenAgentsInc/psionic | `1b539a48988f1ee8472004dad0eb863a27c53612` | Confirmation that `psionic-serve` is the wrong import unit; header-only vs mmap split; Metal keepalive pattern |

Weights stay out of git. Tests write a tiny GGUF fixture at runtime.

Imported crate graph for this slice: none. `cargo tree -p psionic-gguf` must not name `psionic-train`, `psionic-cluster`, or `psionic-mlx-*`.
