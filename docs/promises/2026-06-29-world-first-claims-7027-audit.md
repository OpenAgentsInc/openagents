# World-First Claims #7027 Audit

- Date: 2026-06-29
- Scope: public issue #7027, covering the four `claims.world_first_*` and
  `claims.pursued_world_first_*largest*_sales_force.v1` product-promise records.
- Result: no state upgrades. The two world-first claims stay red; the two
  largest-force claims stay planned pursuits.

## Claim Gates

| Promise | State | Allowed wording | Refuse-list wording | Current dated blocker |
| --- | --- | --- | --- | --- |
| `claims.world_first_ai_training_paid_bitcoin.v1` | red | Only the fully qualified formulation from `docs/launch/2026-06-18-world-firsts-verification.md`: first AI model training run that pays independent contributors in Bitcoin for replay-verified training compute on their own consumer devices. | Bare "world first", "first AI training run paid in Bitcoin", "first to pay Bitcoin for AI", network-scale paid-training claims, or any copy that drops Bitcoin, replay verification, training compute, or own consumer devices. | `blocker.product_promises.world_first_evidence_pack_missing` remains because the prior-art review is not yet a single dereferenceable evidence pack tying each qualifier to live receipts; `blocker.product_promises.world_first_owner_signed_upgrade_missing` remains because no owner-signed transition receipt exists. |
| `claims.world_first_public_llm_computer_training_run.v1` | red | Only the qualified public/open-contributor LLM-computer training-run wording, with Percepta credited as the paradigm originator and "training run" limited to executor construction / exact-trace work. | Bare "world first", "we invented the LLM-computer", gradient-descent model-training claims, general LLM-computer capability, performance parity, or transformers-as-a-served-product claims. | The definition and evidence-pack blockers are cleared by `docs/launch/2026-06-20-llm-computer-training-run-definition.md` and `docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md`; `blocker.product_promises.world_first_owner_signed_upgrade_missing` remains because no owner-signed transition receipt exists. |
| `claims.pursued_world_first_largest_agentic_sales_force.v1` | planned | Only a clearly labeled pursuit/aspiration to build the largest agentic sales force. | "OpenAgents has the largest agentic sales force", "record held", "achieved", "verified", or copy implying a sized agentic sales force already exists. | `blocker.product_promises.world_first_agentic_sales_force_not_achieved`, `blocker.product_promises.world_first_agentic_sales_force_no_sized_verifiable_force`, and `blocker.product_promises.world_first_owner_signed_upgrade_missing` remain because there is no real, sized, independently countable agentic sales force, no record review, and no owner-signed upgrade. |
| `claims.pursued_world_first_largest_sales_force.v1` | planned | Only a clearly labeled pursuit/aspiration toward a roughly seven-million selling-or-sell-equipped-agent bar. | "OpenAgents has the largest sales force", "~7M bar met", "world record held/achieved/verified", or copy treating the cited Avon comparison as OpenAgents-verified. | `blocker.product_promises.world_first_largest_sales_force_not_achieved`, `blocker.product_promises.world_first_largest_sales_force_seven_million_bar_unmet`, and `blocker.product_promises.world_first_owner_signed_upgrade_missing` remain because no independently verified count crosses the stated bar, no independently sourced comparison pack exists, and no owner-signed upgrade exists. |

## Receipt Discipline

The current receipt refs support only bounded underlying facts: a scoped
decentralized training launch, exact-trace executor proof-of-concept work, and
the dated prior-art/evidence documents already named in the registry. They do
not authorize bare world-first wording, record-holder wording, largest-force
wording, network-scale paid-training copy, payout/settlement authority, or any
green product-promise transition.

Public and operator copy must continue to use the registry `safeCopy` and
`unsafeCopy` fields as the copy gate until a future owner-signed
`proof.claim_upgrade_receipts.v1` transition lands for the exact claim wording.
