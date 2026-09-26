---
id: edge-case.ambiguous-name-variants
version: 2
kind: edge-case
title: Do not treat nickname compatibility as unique identity
summary: >-
  A nickname can correspond to multiple distinct formal names, so nickname
  compatibility is not transitive evidence of identity. Use ambiguous variants
  only as weak corroboration and let independent evidence distinguish people
  who share contact details.
tags: [record-linkage, names, nicknames, ambiguity]
applies_when: >-
  A linkage system expands nicknames, aliases, initials, or fuzzy name
  variants, especially when households can contain multiple people sharing
  names or contact fields.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - telecom-entity-resolution
  cites:
    - "Christen, Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection (2012), chapter 4."
    - Bansal, Blum, and Chawla, “Correlation Clustering,” Machine Learning (2004), §2.
evidence: []
---

## Details

Treat name variation as a relation, not automatically as an equivalence relation. If one short form maps to multiple formal names, matching each formal name to that short form does **not** imply that the formal names match each other. Avoid unioning records solely because their names are compatible through a shared nickname.

Use a conservative hierarchy: exact normalized full-name agreement can support a match; an unambiguous alias may provide supporting evidence; an alias shared by several formal names should be weak evidence or no evidence. Resolve ambiguous cases with independent signals such as date of birth or high-quality identifiers, and reject merges when those signals contradict. Do not expand a nickname dictionary from a few observed pairs without checking whether each mapping is one-to-one in the relevant population.

This is a general record-linkage issue: pairwise similarity does not establish identity, and transitive closure can turn ambiguous compatibility into false cluster merges. See Christen’s discussion of name variation and comparison in record linkage, and Bansal, Blum, and Chawla’s formulation of correlation clustering with pairwise positive and negative evidence.

## How to check

```python
# Build the reverse map once; a short form is ambiguous if it has >1 formal form.
reverse = {}
for formal, variants in nickname_map.items():
    for variant in variants:
        reverse.setdefault(variant, set()).add(formal)

ambiguous = {variant for variant, forms in reverse.items() if len(forms) > 1}

def name_evidence(a, b):
    if normalize(a) == normalize(b):
        return "exact"
    if compatible(a, b):
        shared = normalize(a) if normalize(a) in ambiguous else normalize(b)
        return "weak_ambiguous_alias" if shared in ambiguous else "alias_support"
    return "conflict"
```

Test that two distinct formal names mapping to the same ambiguous short form are not directly linked by name evidence alone.
