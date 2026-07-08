# Psion Instruct SFT Lane Receipt

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-20

Promise: `training.post_training_arc.v1` (stays **planned**; no green flip).
Registry edit: `2026-06-20.29`.

Issue lineage: `OpenAgentsInc/openagents#5528`.

Supersession note: registry edit `2026-06-20.31` and
`docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md` clear the
fixture-sync blocker recorded below. The paid SFT dispatch, preference rollout,
and vibe-test blockers remain.

Public route:
`/api/public/training/post-training-arc/instruct-sft-lane`

Receipt ref:
`receipt.training.post_training_arc.instruct_sft_lane.psion_fixture.v1`

## What this clears

This clears only:

- `blocker.product_promises.instruct_sft_lane_missing`

The promise stays planned because these blockers remain:

- `blocker.product_promises.instruct_sft_paid_dispatch_missing`
- `blocker.product_promises.instruct_sft_fixture_sync_missing`
- `blocker.product_promises.preference_rollout_work_missing`
- `blocker.product_promises.vibe_test_artifact_missing`

## Receipt contents

The public projection binds a bounded Psionic fixture-scale instruct SFT lane to
public-safe refs and digests:

- lane: `psion_instruct_sft_v1`
- smoke run: `psion-instruct-sft-smoke-001`
- chat template digest:
  `sha256:7337ec749e64dbf1b23dbfeb3478788846c67e8247813f386d97b1ed1076fca3`
- corpus manifest digest:
  `sha256:1ce60a17a18975a729fd7d9d81baab556541af6fd280c0fadfb29e09b7e18cc7`
- report digest:
  `sha256:76b5524234b4dd6507560c0cda6f28e782fe097c1fb022108aaaae40794d6871`
- completed steps: `8`
- trainable tokens: `93`
- masked tokens: `65`
- scheduler: `cosine_annealing`
- learning-rate ratio vs pretraining reference: `1000` bps
- resume drill: checkpoint at step `3`, resume for `5` steps, bit-exact final
  parameters, and matching post-resume receipt digests

The fixture source lives in Psionic:

- `scripts/check-psion-instruct-sft-lane.sh`
- `fixtures/psion/instruct/psion_chat_template_v1.json`
- `fixtures/psion/instruct/psion_instruct_corpus_manifest_v1.json`
- `fixtures/psion/instruct/psion_instruct_generation_mask_fixture_v1.json`
- `fixtures/psion/instruct/psion_instruct_sft_lane_report_v1.json`
- `crates/psionic-train/src/psion_instruct_sft_lane.rs`
- `crates/psionic-train/examples/psion_instruct_sft_lane_fixtures.rs`

As of this registry edit, two local generator runs produced the deterministic
report digest above, but `../psionic/scripts/check-psion-instruct-sft-lane.sh`
still exits nonzero because the committed Psionic
`psion_instruct_sft_lane_report_v1.json` report fixture has older report digest
rows. That is why `blocker.product_promises.instruct_sft_fixture_sync_missing`
remains even though the old generic lane-missing blocker is cleared.

## Boundaries

This is not a paid OpenAgents SFT assignment. It is not a trained instruct
Psion model, a fine-tuning service, a model-quality claim, a settlement claim,
or a green product-promise transition.

Green for `training.post_training_arc.v1` still requires Psionic committed
fixture synchronization, a paid SFT dispatch such as `cs336_a5_sft_packing` or
equivalent, paid preference/DPO pairwise rollout work, decontamination receipts,
GRPO reward shaping with the overlong-completion penalty, and a reviewed
vibe-test artifact referenced in a closeout, followed by owner-signed
receipt-first upgrade under
`proof.claim_upgrade_receipts.v1`.
