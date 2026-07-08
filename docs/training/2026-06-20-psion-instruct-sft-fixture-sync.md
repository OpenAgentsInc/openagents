# Psion Instruct SFT Fixture Sync Receipt

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-20

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip).
Registry edit: `2026-06-20.31`.

Issue lineage: `OpenAgentsInc/openagents#5528`.

Psionic receipt PR:
`https://github.com/OpenAgentsInc/psionic/pull/1132`

## What this clears

This clears only:

- `blocker.product_promises.instruct_sft_fixture_sync_missing`

The promise stays planned because these blockers remain:

- `blocker.product_promises.instruct_sft_paid_dispatch_missing`
- `blocker.product_promises.preference_rollout_work_missing`
- `blocker.product_promises.vibe_test_artifact_missing`

## Receipt contents

Psionic PR #1132 synchronizes the committed report fixture:

- `fixtures/psion/instruct/psion_instruct_sft_lane_report_v1.json`

with deterministic generator output from:

- `crates/psionic-train/examples/psion_instruct_sft_lane_fixtures.rs`
- `scripts/check-psion-instruct-sft-lane.sh`

The verified report digest is:

`sha256:76b5524234b4dd6507560c0cda6f28e782fe097c1fb022108aaaae40794d6871`

Local Psionic verification before merge:

- `scripts/check-psion-instruct-sft-lane.sh`
- `cargo test -p psionic-train psion_instruct_sft_lane --lib`

## Boundaries

This is not a paid OpenAgents SFT assignment. It is not a trained instruct
Psion model, a fine-tuning service, a model-quality claim, a settlement claim,
or a green product-promise transition.

Green for `training.post_training_arc.v1` still requires a paid SFT dispatch
such as `cs336_a5_sft_packing` or equivalent, paid preference/DPO pairwise
rollout work, decontamination receipts, GRPO reward shaping with the
overlong-completion penalty, and a reviewed vibe-test artifact referenced in a
closeout, followed by owner-signed receipt-first upgrade under
`proof.claim_upgrade_receipts.v1`.
