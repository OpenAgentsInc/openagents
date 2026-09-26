---
id: slip.exact-rules-in-cascades
version: 1
kind: slip
title: Assuming exact-form rules are isolated lookup entries
summary: >-
  In a rewrite cascade, a rule that looks like a whole-form mapping may match
  inside a larger input, and later rules may rewrite text produced by earlier
  rules. Check the engine’s matching and ordering semantics before treating
  such rules as safe fallbacks.
tags: [rewrite-rules, cascades, ordering, testing]
applies_when: >-
  A system applies ordered string-rewrite rules, particularly when using
  complete observed forms as rules or appending broad substitutions after
  them.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade-1790395393
  cites:
    - Kaplan and Kay, “Regular Models of Phonological Rule Systems,” Computational Linguistics 20(3), 1994, §2
evidence: []
---

## Details

Do not infer that a rule is anchored to an entire input merely because its source string is a complete observed form. Some engines search for the source as a substring; cascade semantics may also let later rules operate on earlier rules' output. Thus an apparent lookup rule can unexpectedly affect a novel, longer input, and a later fallback can corrupt a previously produced target. Inspect the implementation or documentation, then test the composed cascade—not each rule in isolation—on held-out and adversarial inputs.

Reference: Kaplan and Kay, “Regular Models of Phonological Rule Systems,” *Computational Linguistics* 20(3), 1994, §2, on composing rewrite rules.

## How to check

This miniature literal cascade shows why order and substring matching matter:

```python
rules = [("ab", "X"), ("X", "Y")]

def cascade(text, rules):
    for source, target in rules:
        text = text.replace(source, target)
    return text

assert cascade("zabz", rules) == "zYz"  # Not an anchored whole-form lookup.
assert cascade("ab", rules) == "Y"     # Later rules rewrite earlier output.
```

Adapt the check to the actual engine’s semantics. Include probes that contain a rule source inside a larger string, and verify final outputs after the entire ordered cascade.
