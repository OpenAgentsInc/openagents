# OpenAgents Business — Offering → Product-Promise Coverage

Version: 2026-06-20 (registry 2026-06-20.50)
Owner directive: "I DO want everything needed to fulfill on OpenAgents Business
specs prioritized, if we need more promises add them."

This doc makes the OpenAgents Business offering menu
(`docs/business/2026-06-20-openagents-business-intake-spec.md`) fully covered by
the public product-promise registry
(`https://openagents.com/api/public/product-promises`,
source `apps/openagents.com/workers/api/src/product-promises.ts`). Every offering
the business menu advertises now maps to one or more backing promise records.
Seven new records were added in this pass to close real gaps. No existing
promise was flipped; no green was added; the 2026-06-20.49 postpone is intact.

States: green = live/shipped+receipted, yellow = partial/flag-gated/operator-
assisted, red = blocked for affirmative copy, planned = roadmap.

---

## Coverage table (offering → backing promiseId(s) → state → gap / next step)

### 1. Coding & agent work

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| Coding-runtime execution (objective → repo → verify → diff + evidence) | `pylon.local_claude_agent_bridge.v1`, `autopilot.codex_probe_pylon_successor.v1`, `pylon.cli_tui_probe_background.v1`, `pylon.agent_steerable_cli.v1` | green | Live single-task exec re-verified 2026-06-19. |
| Negotiated labor jobs (post → negotiate → escrow → execute → validate → settle) | `labor.forum_work_requests.v1`, `labor.nostr_negotiation_market.v1` | green | First end-to-end settled job #4777. |
| Desktop GUI to watch/steer sessions | `autopilot.desktop_gui_client.v1` | yellow | From-DMG clean-Mac render/presence/settled-Bitcoin proof pending. |
| **Coding quick win as a buyable business product** | **`business.coding_quick_win.v1` (NEW)** | yellow | Operator-assisted today; needs packaged priced intake→delivery→receipt + first paid customer receipt. |

### 2. Inference / AI

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| Open/hosted model inference gateway (credits business) | `inference.gateway_credits_business.v1` | red | Gateway live; paid card/Bitcoin→credit loop not collectable end-to-end. |
| Fireworks open-model supply lane | `inference.fireworks_open_model_provider.v1` | yellow | Provider connection verified; no sellable paid product. |
| Hosted Gemini | `api.hosted_gemini.v1` | yellow | — |
| **Free inference taste** | **`inference.free_tier_taste.v1` (NEW)** | yellow | Live Sybil-gated free-allowance taste; bounded, paid upgrade not collectable. |
| **Batch processing (summaries/classifications/extractions) as a buyable job** | **`inference.batch_processing_jobs.v1` (NEW)** | planned | No batch-job surface; depends on collectable paid loop. |
| Cross-category referral on inference | `inference.referral_on_all_inference.v1` | planned | — |
| Decentralized serving fabric | `inference.decentralized_serving_fabric.v1` | red | Pylons do not yet serve inference. |

### 3. Sites + commerce

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| Autopilot Site build + host | `autopilot_sites.site_build_and_host.v1` | yellow | — |
| Custom branded hostnames | `autopilot_sites.custom_tenant_hostnames.v1` | yellow | Self-serve/SSL gap. |
| Native email sequences | `autopilot_sites.native_email_sequences.v1` | yellow | Deliverability smoke + self-serve authoring pending. |
| Built-in referral streams | `sites.referral_bitcoin_stream.v1` | yellow | First real referral payout pending. |

### 4. Autopilot business automation

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| All-in-one business system framing (Signal→…→Deploy, self-serve) | `autopilot.all_in_one_business_system.v1` | planned | Self-serve framing is roadmap. |
| Client-delivery workrooms | `workrooms.omni_client_delivery_workrooms.v1` | yellow | — |
| Agentic labor products | `autopilot.agentic_labor_products.v1` | yellow | — |
| **E-commerce prefilled workspace (inventory-aware ad campaigns)** | **`business.ecommerce_workspace_pack.v1` (NEW)** | yellow | Template shipped as operator tool; first paid e-commerce delivery receipt pending (format + verifier now built — see `docs/launch/vertex-fleet/business.ecommerce_workspace_pack.v1.md`). |
| **Legal prefilled workspace (forms/intake copilot, review-gated, no legal advice)** | **`business.legal_workspace_pack.v1` (NEW)** | yellow | Template shipped as operator tool; first paid legal delivery receipt pending. |
| **Marketing-agency prefilled workspace (white-label pages + emails)** | **`business.marketing_agency_workspace_pack.v1` (NEW)** | yellow | Template + Sites surfaces shipped; deliverability + first paid agency delivery receipt pending (receipt format + verifier now built — see `docs/launch/vertex-fleet/business.marketing_agency_workspace_pack.v1.md`). |

### 5. Distributed compute / training

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| Decentralized training launch (scoped, settled) | `training.decentralized_training_launch.v1` | green | Two distinct contributors paid. |
| Verification classes | `training.verification_classes.v1` | green | — |
| Device-capability dataset | `training.device_capability_dataset.v1` | planned | Postponed 2026-06-20.49. |
| Public distributed training run (network scale) | `training.public_distributed_training_run.v1` | planned | Postponed 2026-06-20.49. |
| Fine-tuning as a service | `cloud.fine_tuning_service.v1` | red | Jobs queue in staging; not buy-it-now. |
| Sandbox compute service | `cloud.sandbox_compute_service.v1` | red | Provisions in staging; not buy-it-now. |
| Cloud primitives suite | `cloud.primitives_suite.v1` | planned | — |

### 6. Forum / community

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| Content tipping | `forum.content_tipping.v1` | green | — |
| Agent registration + autonomous posting | `agents.cursor_forum_wallet.v1` | green | — |
| Forum work requests | `labor.forum_work_requests.v1` | green | — |
| Reliable tips / sweepable balances | `payments.reliable_tips_sweepable_balances.v1` | green | — |
| Cloud-resident assistant replies | `artanis.pylon_support_responder.v1` | yellow | — |

### 7. Payments rails

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| Money Dev Kit self-custodial wallet | `payments.money_dev_kit.v1` | yellow | Broader wallet flow. |
| Reliable tips + offline Spark fallback | `payments.reliable_tips_sweepable_balances.v1`, `payments.offline_receive_spark_fallback.v1` | green | — |
| USD-credit funding for usage | `inference.gateway_credits_business.v1`, `payments.autopilot_credits_purchase.v1` | red | Paid card→credit not collectable in prod. |
| Native-sat live settlement (general payout) | `payments.accepted_outcome_economics.v1` | planned | "Available soon, ask us." |

### Cross-cutting — the business offering itself

| Offering | Backing promiseId(s) | State | Gap / next step |
| --- | --- | --- | --- |
| **OpenAgents Business offering menu + `/business` intake → quick-win → Autopilot** | **`business.intake_quick_win_offering.v1` (NEW)** | yellow | Live intake route + registry-grounded menu; self-serve quick-win delivery + first paid quick-win receipt pending. |
| Source transparency | `repo.open_source_code_map.v1` | green | — |
| Registry + claim-upgrade audit | `promises.registry.v1`, `proof.claim_upgrade_receipts.v1`, `proof.demand_provenance.v1` | green / green / yellow | — |

---

## New promiseIds added this pass (7)

| promiseId | state | why |
| --- | --- | --- |
| `business.intake_quick_win_offering.v1` | yellow | The central business product: menu + live `/business` intake. |
| `business.coding_quick_win.v1` | yellow | Coding quick win packaged as a buyable business outcome. |
| `inference.free_tier_taste.v1` | yellow | Free inference taste advertised in the menu. |
| `inference.batch_processing_jobs.v1` | planned | Batch summaries/classifications/extractions as a buyable job. |
| `business.ecommerce_workspace_pack.v1` | yellow | E-commerce prefilled vertical workspace. |
| `business.legal_workspace_pack.v1` | yellow | Legal forms/intake copilot prefilled workspace. |
| `business.marketing_agency_workspace_pack.v1` | yellow | Marketing-agency white-label prefilled workspace. |

---

## BUSINESS-FULFILLMENT PRIORITY SET

The fleet must drive these promiseIds (existing + new) to fully deliver the
OpenAgents Business offerings. This is the concrete meaning of "prioritized."
Ordered roughly by leverage on closing the offering → delivered-with-receipt
loop. Already-green records that are not the bottleneck are excluded; they are
proven and only need to stay green.

Tier 1 — make the business loop collectable and deliverable (highest priority):

1. `business.intake_quick_win_offering.v1` — close the self-serve quick-win delivery + first paid quick-win receipt.
2. `inference.gateway_credits_business.v1` — close the paid card/Bitcoin→credit→inference-spend receipt (prod Stripe keys + USD→msat bridge with a real purchase).
3. `payments.autopilot_credits_purchase.v1` — credit-card purchase of credits in prod.
4. `business.coding_quick_win.v1` — package the green coding runtime + labor market as a priced, repeatable, receipted business product.

Tier 2 — vertical packs and inference products from operator-assisted → self-serve:

5. `business.ecommerce_workspace_pack.v1`
6. `business.legal_workspace_pack.v1`
7. `business.marketing_agency_workspace_pack.v1`
8. `inference.free_tier_taste.v1` — keep the taste live; needed as the on-ramp.
9. `inference.batch_processing_jobs.v1` — build the batch-job product surface.

Tier 3 — supporting surfaces that broaden the offering:

10. `autopilot_sites.site_build_and_host.v1`
11. `autopilot_sites.custom_tenant_hostnames.v1`
12. `autopilot_sites.native_email_sequences.v1`
13. `sites.referral_bitcoin_stream.v1`
14. `workrooms.omni_client_delivery_workrooms.v1`
15. `autopilot.all_in_one_business_system.v1`
16. `autopilot.desktop_gui_client.v1`
17. `payments.money_dev_kit.v1`
18. `payments.accepted_outcome_economics.v1` — native-sat live settlement for general payout.
19. `cloud.fine_tuning_service.v1`
20. `cloud.sandbox_compute_service.v1`
21. `cloud.primitives_suite.v1`

Each green flip stays receipt-first and gates-must-be-met per
`proof.claim_upgrade_receipts.v1`. No item is driven by weakening a gate.
