---
id: tool.lean4-proof-workflow
version: 1
kind: tool
title: Complete and verify Lean 4 proofs without sorry or new axioms
summary: >-
  A Lean file with sorry still builds, with only a warning. Build inside the
  project's pinned toolchain, search Mathlib for existing lemmas with exact?,
  apply?, and simp?, close arithmetic with the decision tactics, and confirm
  with #print axioms that no sorryAx or added axiom remains.
tags: [lean4, mathlib, theorem-proving, lake, proofs, formal-verification]
applies_when: >-
  Filling in or repairing Lean 4 proofs, especially against Mathlib, where the
  grader will reject sorry, changed statements, or extra axioms.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Avigad, de Moura, Kong, Ullrich, Theorem Proving in Lean 4 (lean-lang.org), chapters Tactics and Axioms and Computation"
    - "Mathlib documentation: tactics exact?, apply?, simp?, norm_num, linarith, nlinarith, positivity, omega, ring, field_simp, gcongr, aesop"
    - "Lake README (leanprover/lean4): lake build, lake env, lean-toolchain, lake exe cache get"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Environment.** The `lean-toolchain` file pins the Lean version and must match
the Mathlib revision in `lake-manifest.json`. Build with `lake build` or check
one file with `lake env lean File.lean`. Do not run `lake update` (it moves
dependencies); `lake exe cache get` downloads prebuilt Mathlib and needs
network, so without it reuse the existing `.lake` build. A full Mathlib
rebuild takes hours; import only the modules you need.

**Finding the proof.**

- `exact?` searches for a single lemma that closes the goal; `apply?` lists
  lemmas whose conclusion unifies; `simp?` reports the `simp only [...]` set
  it used, which you should paste back for a stable proof. `#check @name` and
  hovering show exact hypotheses and implicit arguments.
- Decision and normalization tactics: `omega` (linear arithmetic over ℕ and
  ℤ), `decide` (decidable propositions of small size), `norm_num` (numeric
  facts), `ring`/`ring_nf`, `field_simp` then `ring` for fields, `linarith`
  and `nlinarith` for (non)linear inequalities over ordered fields,
  `positivity`, `gcongr` for monotonicity goals.
- Structure: `induction n with | zero => ... | succ n ih => ...`,
  `rcases h with ⟨x, hx⟩`, `obtain`, `by_contra`, `push_neg`, `calc` blocks
  for chains of inequalities.
- Casts between ℕ, ℤ, ℝ are the usual obstacle: `push_cast`, `norm_cast`,
  `exact_mod_cast`.

Keep theorem statements exactly as given; changing a hypothesis or a type
class assumption makes the proof worthless to the grader even if it compiles.

## How to check

After the build succeeds, grep the output for `declaration uses 'sorry'` and
run `#print axioms theoremName` for every required theorem. The only axioms
acceptable in ordinary Mathlib proofs are `propext`, `Classical.choice`, and
`Quot.sound`; `sorryAx` means an unfinished proof, and any other name means an
added axiom. Also grep the sources for `sorry`, `admit`, `axiom`, and
`@[implemented_by]`/`unsafe` additions, and diff the statements against the
originals.
