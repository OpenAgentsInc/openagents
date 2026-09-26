---
id: phonology.ordered-sound-change-inference
version: 2
kind: method
title: Infer ordered sound changes by aligning, testing, and refining cascades
summary: >-
  A disciplined workflow for inferring sound-change cascades from paired
  forms: inspect the rule engine, derive correspondence hypotheses from
  alignments and contexts, then validate the entire ordered cascade and
  investigate residual mismatches. Applies when many ordered rules jointly map
  source forms to exact target forms.
tags: [phonology, sound-change, rule-ordering, validation]
applies_when: >-
  Recovering a sequential phonological rule system from paired forms,
  especially when contexts and rule order affect later rule application.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade
  cites:
    - "Campbell, Lyle. *Historical Linguistics: An Introduction*, 3rd ed., ch. 9, “Sound Change.”"
evidence: []
---

## Details

Sound changes are treated as systematic correspondences, and a cascade applies changes in an order that can affect their environments (Campbell, *Historical Linguistics: An Introduction*, 3rd ed., ch. 9, “Sound Change”). Start by reading the rule engine and its schema: determine when contexts are checked, whether rules see intermediate outputs, how matching proceeds, and which operations are unsupported. Do this before interpreting apparent correspondences.

Use alignments as evidence, not as rules. A character-level edit alignment can suggest substitutions, deletions, or insertions, but ambiguous alignments should be checked against multiple forms and their contexts. Group correspondence observations by neighboring sounds and word position. Prefer a small set of context-sensitive rules that explains recurring patterns over exact-form exceptions.

Build and run the cascade incrementally. Track exact-match coverage across the full paired dataset, inspect the remaining mismatches, and use those residuals to refine context and ordering hypotheses. When a rule must apply before a later change destroys its conditioning environment, preserve the relevant distinction with a temporary symbol and restore it later; ensure intermediate symbols cannot be mistaken for ordinary sounds. A perfect fit to observed pairs is evidence of coverage, not by itself evidence that the rules generalize.

## How to check

After each revision, apply the complete cascade to every source form and report exact matches and a small set of residual mismatches. Check that each proposed rule accounts for a recurring correspondence in multiple contexts or forms, that the ordering is executable as written, and that no temporary symbols remain in final outputs. Evaluate any available held-out forms separately from the forms used to infer the rules.
