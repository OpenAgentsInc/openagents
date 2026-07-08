# Training Ablation Eval-Reproduction Receipt

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-20

Promise: `training.ablation_system.v1` (stays **planned**; no green flip).
Registry edit: `2026-06-20.27`.

Issue lineage: `OpenAgentsInc/openagents#5528`.

## What this advances

The ablation derisking ledger already verified one-delta candidate manifests.
This pass adds the missing public eval-reproduction evidence leg by projecting a
retained Psion actual-pretraining checkpoint-eval decision into the ablation
ledger:

- `receipt.training_ablation.eval_reproduction.psion_actual_checkpoint_eval.v1`
- source schema: `psion.actual_pretraining_checkpoint_eval_decision.v1`
- benchmark pack:
  `benchmark://psion/actual_pretraining/checkpoint_eval@2026.04.02`
- four metric gates passed
- aggregate pass rate: `10000` bps
- aggregate score: `8532` bps
- decision: `continue`

The source fixture lives in Psionic:

- `fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_eval_decision_v1.json`
- `crates/psionic-eval/src/psion_actual_pretraining_checkpoint_eval_pack.rs`
- `docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md`

## Ledger behavior

`GET /api/public/training/ablation-derisking-ledger` now reports:

- `ablationHarnessAvailable: true`
- `evalSuiteReproductionAvailable: true`
- `paidAblationDispatchAvailable: false`
- `greenGateSatisfied: false`
- `evalReproductionReceipts.length: 1`

The product blocker
`blocker.product_promises.eval_suite_reproduction_missing` is cleared. The
remaining blocker is
`blocker.product_promises.paid_ablation_dispatch_missing`.

## Honest remainder

This receipt proves the retained checkpoint-eval surface and frozen benchmark
pack are reproducible evidence for the eval-suite leg. It does **not** prove a
paid OpenAgents ablation cell executed, that a candidate delta won, that an
ablation verdict was accepted, or that any model change should be promoted.

Green for `training.ablation_system.v1` still requires paid ablation cells
dispatched and verified as assignments, seeded replication/validator receipts,
settlement receipts, accepted ablation verdicts, and ledger entries carrying
the baseline, delta, eval, payment, and verdict receipts.

