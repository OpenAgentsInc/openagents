---
id: tool.coq-proof-validation
version: 1
kind: tool
title: Validate Coq proofs with compilation and assumption checks
summary: >-
  When a Coq task requires a completed proof without added axioms or admitted
  goals, compile incrementally and inspect the theorem’s assumptions; a
  successful build alone does not establish that the proof is assumption-free.
tags: [coq, proof-checking, axioms, debugging]
applies_when: >-
  A Coq development must compile and a theorem must be proved without relying
  on new axioms, parameters, or admitted lemmas.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - coq-block-bound
  cites:
    - The Coq Development Team, The Coq Proof Assistant Reference Manual, “The coqc command”
    - The Coq Development Team, The Coq Proof Assistant Reference Manual, “Commands for inspecting the environment”
evidence: []
---

## Details

Compile after completing small proof units rather than waiting until the entire development is written. This catches tactic scripts that accidentally close goals early, leave goals unfocused, or depend on an unfolding or rewrite at the wrong arguments. When a rewrite fails because Coq selected the wrong instance, make the intended arguments explicit, for example `rewrite (helper x y)`.

Treat compiler success and assumption-freedom as separate checks. After importing the module containing the theorem, run `Print Assumptions theorem_name.` Coq reports whether the theorem depends on axioms in the global context. A text search for forbidden declaration keywords can be a useful additional audit, but it is not a substitute: it can match comments and does not establish the theorem’s dependency chain.

For command-line builds, preserve the project’s logical-path mapping (`-Q`) and compile from the project directory so imports resolve as intended. The Coq Reference Manual documents compilation with `coqc` and environment-inspection commands including `Print Assumptions` (The Coq Development Team, *The Coq Proof Assistant Reference Manual*, sections “The coqc command” and “Commands for inspecting the environment”).
