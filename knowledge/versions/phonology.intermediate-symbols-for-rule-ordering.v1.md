---
id: phonology.intermediate-symbols-for-rule-ordering
version: 1
kind: method
title: Use temporary symbols to preserve intermediate rule contexts
summary: >-
  When an early change must alter a segment's identity without letting later
  rules treat it as its final surface value, stage the change through a
  temporary symbol. This helps encode feeding, bleeding, and delayed changes
  in sequential rule engines.
tags: [phonology, rule-ordering, intermediate-representations, implementation]
applies_when: >-
  A sequential rewrite engine applies later context tests to the output of
  earlier rules, and an early change would otherwise alter the environment
  needed to analyze a later process.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade
  cites:
    - Noam Chomsky and Morris Halle, The Sound Pattern of English, Chapter 8, “The Evaluation of Phonological Rules”
evidence: []
---

## Details

In an ordered cascade, rules operate on intermediate forms: earlier rules can feed or bleed later rules by creating or removing their environments. If a segment must change early for one purpose but retain a distinct status for later context matching, map it to a temporary symbol, apply the intervening rules, then map that symbol to its intended final segment.

Choose a temporary symbol that cannot occur in the input alphabet or be mistaken for a natural segment. Ensure no unrelated rule matches it, and convert every temporary symbol before producing final outputs. This is a representation technique for implementing the intended ordering; it is not evidence that the temporary symbol is a historical sound.

Source: Noam Chomsky and Morris Halle, *The Sound Pattern of English*, Chapter 8, “The Evaluation of Phonological Rules” (ordered application and rule interactions).

## How to check

After running the cascade, verify both corpus coverage and the invariant that no temporary symbols remain:

```python
outputs = [apply_cascade(proto, rules) for proto, _ in pairs]
assert outputs == [reflex for _, reflex in pairs]
assert all(not any(mark in output for mark in temporary_symbols)
           for output in outputs)
```

Also test a minimal pair of inputs where the relevant later rule's context is present in one intermediate form but absent in the other; confirm the temporary-symbol staging preserves the intended distinction.
