---
id: method.ordered-rule-cascade-reconstruction
version: 1
kind: method
title: Recover ordered rewrite cascades with protected intermediate symbols
summary: >-
  Infer context-sensitive sound changes from aligned pairs, then validate an
  ordered cascade against all examples. When an early change must remain
  distinguishable from ordinary output until later rules finish, map it to a
  temporary symbol and convert that symbol only near the end.
tags: [rule-inference, sound-change, ordered-rewriting, phonology]
applies_when: >-
  A task provides input/output word pairs, contextual sequential rewrite
  rules, and an engine where later rules can alter material produced by
  earlier rules.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade-1790402357
  cites:
    - Kenneth R. Beesley and Lauri Karttunen, Finite State Morphology, section 2.1, 'Finite-State Transducers' (sequential composition of transductions).
    - John Goldsmith, The Handbook of Phonological Theory, 2nd ed., chapter 3, 'The Foundations of Generative Phonology' (ordered phonological rules).
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Start by inspecting the rewrite engine, especially the exact meanings of context classes, boundary behavior, scan direction, longest-match behavior, and whether each rule sees the previous rule's output. Align source and target forms to generate candidate correspondences, but treat alignments as hypotheses: deletions and adjacent changes can make local mappings ambiguous. Look for correspondences that recur under the same contexts, then encode and test a complete cascade rather than relying on a training-form lookup.

Order rules according to their dependencies. A change that creates material resembling the input of a later rule can be undone or transformed unless it is ordered carefully. Conversely, a later change may need to see the output of an earlier change. If a segment undergoes an early structural change but must not be treated like its eventual surface symbol by intervening rules, use an unused temporary symbol as an intermediate representation and map it to the surface symbol after the relevant rules. Temporary symbols are a bookkeeping device, not an extra phoneme in the final output.

Use differential checks after each revision: run the actual engine on every supplied pair, report mismatches, and inspect changed intermediate forms. A perfect training fit is necessary but not sufficient for generalization; prefer compact contextual rules supported by multiple examples, and test that the chosen order preserves forms whose surface symbols overlap with rule sources.
