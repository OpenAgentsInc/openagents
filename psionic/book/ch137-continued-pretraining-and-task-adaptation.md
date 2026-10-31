# 137. Continued pretraining and task adaptation

> **Status: unimplemented — design draft.** Nothing here runs yet.

Continued pretraining trains a base model further on a chosen domain.
nanochat's current speedrun has no such stage, so this chapter is a Psionic
extension, not a stage claimed from the pinned recipe. It needs its own
workload, data and promotion rules.

## What this chapter covers

- A separately specified domain workload, written before training starts.
- A data mixture: domain data mixed with general data in stated proportions,
  each with provenance.
- Retention checks: general held-out metrics measured before and after.
- Task metrics on held-out domain data.
- Promotion criteria, frozen before the confirmation set is read.

## Dependencies

- [Chapter 128](ch128-a-complete-pretraining-run.md): a base model and its
  pretraining pipeline.
- [Chapter 120](ch120-data-and-resumable-state.md): provenance and splits.
- [Chapter 241](ch241-gates-that-can-say-unverifiable.md): the gate that
  decides promotion.

## Done means

- The workload, mixture and promotion criteria are recorded before training.
- The report gives domain task gains and general retention separately.
- A model is promoted only if it passes the frozen criteria. Otherwise the
  chapter reports that it did not.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
