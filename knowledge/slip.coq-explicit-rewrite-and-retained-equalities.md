---
id: slip.coq-explicit-rewrite-and-retained-equalities
version: 1
kind: slip
title: Make Coq rewrites explicit and preserve equations needed by later arithmetic
summary: >-
  When a rewrite or `replace` fails because the intended occurrence is not
  syntactically present, state the structural equality as a separate assertion
  and rewrite it explicitly; use `remember ... eqn:` when later proof steps
  need the defining equation.
tags: [coq, proof-debugging, rewrite, induction]
applies_when: >-
  Coq goals involve nested expressions such as sequences, sums, or logarithms,
  and tactic failures indicate that an expected subterm is absent or a locally
  introduced definition has been unfolded or substituted away.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - coq-block-bound
  cites:
    - The Coq Development Team, *The Coq Reference Manual*, “Tactics” sections on `rewrite`, `replace`, `remember`, and reduction.
    - The Coq Development Team, Coq standard library source `theories/Lists/List.v`, lemma `seq_app`.
evidence: []
---

## Details

A tactic such as `replace ... at n` depends on the exact syntactic occurrence at that location. If the expression is hidden by a definition, has a different association, or occurs at a different location, the tactic may fail even when the intended equality is mathematically true. Prefer proving a named equality (for example, a list decomposition), then rewriting with it. This makes the required shape visible and isolates failures.

Likewise, `set (j := expression) in *` can change many hypotheses and goals at once. If later arithmetic needs the relationship between `j` and the original expression, retain it explicitly with `remember expression as j eqn:Hj`. Split Boolean equality tests with a specification lemma such as `Nat.eqb_spec` when simplification obscures which equality or inequality remains. Use controlled reduction (`cbn [definition]`) when broad `simpl` unfolds more than intended.

These tactics and their behavior are documented in the Coq Reference Manual, “Tactics” (rewrite, replace, remember, and reduction tactics). The standard list decomposition used below is documented by `seq_app` in Coq's list library.

## How to check

This standalone Coq snippet checks a sequence decomposition by naming the equality rather than relying on an occurrence-directed replacement:

```coq
From Coq Require Import List Arith.
Goal forall start offset len,
  seq start (offset + len) = seq start offset ++ seq (start + offset) len.
Proof.
  intros. rewrite seq_app. reflexivity.
Qed.
```

When preserving a definition's relation is important, inspect the generated equation with `remember ... eqn:H` and confirm it remains available before continuing the induction.
