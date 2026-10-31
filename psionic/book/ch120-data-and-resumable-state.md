# 120. Data and resumable state

> **Status: unimplemented — design draft.** Nothing here runs yet.

A held-out score means nothing if the test data leaked into training. This
chapter makes data provenance and splits explicit, and it makes a run's full
state resumable: model, optimizer, random-number generator and data counters.

## What this chapter covers

- Provenance for every document: source, licence and content identity.
- Disjoint splits made by task, session or time, never by row:
  - repeated episodes stay in one split;
  - paraphrases and near-duplicates stay in one split.
- Preprocessing fitted on training data only.
- Document packing into fixed-length sequences.
- Run state: model, optimizer, RNG and data counters.
- Exact resume under a named profile.
- Planted leakage cases, including a feature that is known only after the
  outcome.

## Dependencies

- [Chapter 112](ch112-bytes-and-a-small-language-model.md): fixed licensed
  splits and the named RNG.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): the
  checkpoint record. Chapter 122 already runs packing, identities and exact
  resume in a bounded profile; this chapter adds split rules and leakage
  checks.
- [Chapter 247](ch247-outcomes-become-training-rows.md): the exposure ledger
  for held-out groups.

## Done means

- Every planted leakage case is detected, including the post-outcome
  feature.
- Near-duplicate and paraphrase fixtures land in one split.
- A run interrupted at a chosen step resumes and reproduces the uninterrupted
  run under the named profile.
- Preprocessing statistics are computed from training data only, checked by
  a fixture whose test data would change them.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
