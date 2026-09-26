---
id: phonology.ordered-sound-change-inference
version: 1
kind: method
title: Infer sound-change cascades from aligned forms and contexts
summary: >-
  Use alignments to generate candidate proto-to-reflex correspondences, then
  test them against phonological contexts and an ordered cascade. This applies
  when recovering productive sound changes from paired forms rather than
  memorizing individual mappings.
tags: [historical-linguistics, phonology, rule-ordering, alignment]
applies_when: >-
  A task provides paired older and newer forms and asks for an ordered set of
  context-sensitive sound changes.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade
  cites:
    - "Lyle Campbell, Historical Linguistics: An Introduction, 3rd ed., Chapter 5, “Sound Change”"
evidence: []
---

## Details

Start with a rough segment alignment for each pair, using an edit-distance cost scheme that favors plausible correspondences. Treat the alignment as a hypothesis, not as the sound-change analysis: repeated or ambiguous segments can be aligned differently, and one alignment can make a context-sensitive process look like several unrelated mappings.

Aggregate the proposed correspondences by segment and inspect their neighboring environments. Separate regular conditioned changes from unconditioned changes, mergers, deletions, and apparent exceptions. Check candidate rules against multiple forms and contexts before adding them to the cascade.

Implement the candidate rules in the engine's actual semantics and order. A rule's output can create or remove the context for a later rule, so a rule set that looks plausible in isolation may behave differently when composed. Iterate by examining mismatches, revising the analysis, and rerunning the complete corpus. Exact training fit is a necessary check, not proof that the rules generalize; prefer compact, contextually supported rules and separately probe forms not used to formulate them.

Source: Lyle Campbell, *Historical Linguistics: An Introduction*, 3rd ed., Chapter 5, “Sound Change” (conditioned sound change and regularity).
