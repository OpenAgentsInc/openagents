# Revenue-Loop Product-Promise Audit & Tightening Proposal

- **Date:** 2026-06-20
- **Author:** Raynor (agent), for owner review
- **Status:** PROPOSAL — does NOT change any promise state in the registry. This
  doc recommends what to drive vs. defer for the next 48 hours. The registry
  (`apps/openagents.com/workers/api/src/product-promises.ts`) is the source of
  truth and is left untouched by this change.
- **Live registry snapshot:** `2026-06-20.48`
  (`https://openagents.com/api/public/product-promises`,
  `generatedAt 2026-06-20T15:05:29Z`)
- **Counts at snapshot:** 105 promises — **26 green, 30 yellow, 20 red, 27
  planned, 2 withdrawn.**

> Reconciliation note: `~/work/STAGING_E2E_REPORT.md` and `NEEDS_OWNER.md` say
> "green = 24". The live prod registry now reads **26 green** at version
> `.48` (the prod registry deploy had been lagging origin/main; it has since
> advanced). This audit uses the live `.48` numbers.

---

## 1. Executive summary

**The single 48-hour goal:** close the revenue loop end-to-end — **real dollars
IN → real dollars (or sats) OUT, with a dereferenceable receipt at every hop.**
The canonical loop is:

```
real money IN  →  credit / ledger entry  →  paid work or inference (accepted outcome)
              →  settlement receipt  →  payout OUT
```

Today the loop is **proven in staging but has never closed once in production
with real money.** Per `STAGING_E2E_REPORT.md`, the Ep239 smoke passed
(register → free inference → $10 funded grant → metered-spend path →
referral-attribution gating) on the isolated staging Worker, and the supporting
machinery (Stripe Checkout creation, USD ledger, MDK direct tips, the reliable
tips ladder, the referral payout dispatch, the labor settlement seam) all exist
and pass tests. **What is missing is a single real production event with money
that actually moved.** That is the whole game for the next 48 hours.

Two structural facts drive the tightening:

1. **The first real "money OUT with a receipt" already exists in one lane.**
   `payments.offline_receive_spark_fallback.v1` (green) and
   `payments.reliable_tips_sweepable_balances.v1` (green) document a real
   recipient-confirmed 50,000-sat Spark payout and real BOLT-12 + sweep tip
   settlements with dereferenceable receipts. The cheapest path to "real $
   in→out" is therefore **the Bitcoin/forum-tip rail, not the card rail** — it
   needs no owner-gated prod Stripe secret, only a funded payer and a live tip.

2. **The card→credit→spend rail is fully built but owner-gated.** The Stripe
   secrets are deliberately withheld (`NEEDS_OWNER.md`: "stay in staging mode
   until the entire system is working"). `payments.autopilot_credits_purchase.v1`
   and `inference.gateway_credits_business.v1` are red **purely on the absence
   of one real collected card purchase**, not on missing code.

**Tightened set (this proposal):**

- **KEEP / PUSH NOW (revenue critical path): 21 promises** — the minimal set
  whose green/working state IS the revenue loop. ~half are already green.
- **POSTPONE: 19 promises** — built-but-off-path, research lanes, and
  nice-to-haves. Keep the code; stop spending fleet cycles chasing their green
  flips this week.
- **WITHDRAW (or hold withdrawn): 4 promises** — novelty/world-first PR claims
  and one duplicate withdrawn already, that should not be driven toward live
  copy in a revenue-focused window.

Postpone + withdraw = **23 of 105 ≈ 22%**, matching the owner's "~20%" ask. The
remaining ~61 promises (mostly Pylon/training/autopilot capability promises) are
neither on the critical path nor recommended for removal — they stay as-is, just
de-prioritized behind the critical path.

**Shortest path to the first real $ in→out with a receipt (recommendation):**
the **Bitcoin tip rail** — register a production agent → fund it in sats from
the funded MDK treasury → ready-recipient post → live tip →
**dereferenceable settled-sats receipt** (`forum.content_tipping.v1` +
`payments.money_dev_kit.v1`). This needs no owner Stripe key and proves
in→ledger→out with a public receipt **today**. The card→credit loop
(`payments.autopilot_credits_purchase.v1`) is the second, owner-gated cut and
should follow the instant the owner says "prod stripe".

---

## 2. Full inventory (all 105, grouped by state)

Each promise is tagged **ON** (on the 48h revenue critical path) or OFF.
21 promises are ON; 84 are OFF.

#### GREEN — live (26)

- `forum.content_tipping.v1` (Forum) — **ON** — Stacker-News-for-agents tipping; the live $-in rail.
- `payments.reliable_tips_sweepable_balances.v1` (Forum) — **ON** — tips never fail; real sats settled on one audited ledger. Money OUT proven.
- `labor.forum_work_requests.v1` (Forum) — **ON** — budgeted work request → NIP-90 job → settlement receipts; accepted-outcome lane.
- `pylon.install_without_wallet_knowledge.v1` (Pylon) — **ON** — install→earn self-serve; settlement-record path (simulation-backed receipt today).
- `labor.nostr_negotiation_market.v1` (Pylon) — **ON** — full labor job quote→escrow→settle; the labor payout engine.
- `payments.offline_receive_spark_fallback.v1` (payments) — **ON** — real recipient-confirmed 50k-sat payout receipts. Money OUT proven.
- `proof.claim_upgrade_receipts.v1` (public proof) — **ON** — the receipt-first gate every green flip must pass; the audit substrate.
- `autopilot.codex_probe_pylon_successor.v1` (Autopilot) — OFF — positioning/lineage claim.
- `agents.cursor_forum_wallet.v1` (Forum) — OFF — agent-onboarding convenience.
- `pylon.v03_release_candidate.v1` (Pylon) — OFF — release-status claim (shipped).
- `pylon.release_tomorrow.v1` (Pylon) — OFF — release-timing claim (shipped).
- `pylon.cli_tui_probe_background.v1` (Pylon) — OFF — runtime UX.
- `pylon.agent_steerable_cli.v1` (Pylon) — OFF — runtime UX.
- `pylon.no_dark_capacity_accounting.v1` (Pylon) — OFF — capacity-funnel transparency; supports demand provenance but not on the loop itself.
- `compute.tassadar_executor_poc.v1` (Pylon) — OFF — executor PoC.
- `artanis.tassadar_evolution_loop.v1` (Pylon) — OFF — agent self-improvement loop.
- `artanis.cloud_mind.v1` (Pylon) — OFF — agent infra claim.
- `pylon.v03_agent_economy.v1` (Pylon) — OFF — umbrella economy claim.
- `pylon.local_claude_agent_bridge.v1` (Pylon) — OFF — local bridge feature.
- `discovery.homepage_json.v1` (agent-readable surfaces) — OFF — agent-readable discovery.
- `agents.one_instruction_sheet.v1` (agent-readable surfaces) — OFF — agent-readable instructions.
- `agents.nostr_fallback_coordination.v1` (agent-readable surfaces) — OFF — coordination fallback.
- `promises.registry.v1` (product promises) — OFF — the registry meta-promise (keep current, not a $ lane).
- `repo.open_source_code_map.v1` (source transparency) — OFF — transparency.
- `training.decentralized_training_launch.v1` (training) — OFF — training-launch claim.
- `training.verification_classes.v1` (training) — OFF — verification taxonomy.

#### YELLOW — partial / gated (30)

- `autopilot.agentic_labor_products.v1` (Autopilot) — **ON** — sells agentic labor not base resale; self-serve order plan live but INERT. Green = one real external labor sale settled.
- `autopilot.control_center_fanout_marketplace.v1` (Autopilot) — **ON** — fan-out to many agents; self-serve scope met, FLAG-GATED INERT. Green = receipt-first settled fanout.
- `identity.orange_check_forum_signal.v1` (Forum) — **ON** — $5 orange check; everything built. Green = one live $5 purchase settling through prod checkout.
- `sites.referral_bitcoin_stream.v1` (Sites) — **ON** — referral → Bitcoin stream; RL-1 dispatch proven vs mock. Green = one real Bitcoin-revenue settled referral payout receipt.
- `api.hosted_gemini.v1` (agent API) — **ON** — hosted Gemini via OpenAgents API; route smoke passes. Green = registered-agent prod smoke with billing/settlement.
- `agents.x_claim_reward.v1` (agent-readable surfaces) — **ON** — 1000-sat X-claim reward; dispatcher built, flag-off. Green = one live dispatched reward to a real receive code.
- `inference.fireworks_open_model_provider.v1` (inference gateway) — **ON** — live open-model supply lane. Green = paid-credits path collectable + one funded customer request with metering/settlement.
- `provider.compliant_usage_labor.v1` (labor) — **ON** — earn sats on your own provider budget, no resale. First-live met. Green = external-sats ladder settlement, self-serve.
- `payments.money_dev_kit.v1` (payments) — **ON** — MDK self-custodial wallet + hosted checkout; blocked on send-capacity. The core $-in/$-out engine.
- `autopilot.cloud_credits_ui.v1` (payments) — **ON** — credit balance + cost preview UI; renders/tests pass. Green = purchase + spend + settlement behind it.
- `proof.demand_provenance.v1` (public proof) — **ON** — internal vs external dollars; "no external dollar, no demand claim." The honesty gate on every $ number.
- `autopilot.mission_briefing.v1` (Autopilot) — OFF — mission briefing JSON; needs one live mission citing it.
- `autopilot.desktop_gui_client.v1` (Autopilot) — OFF — signed DMG release work.
- `autopilot.agent_world_scene.v1` (Autopilot desktop) — OFF — 3D agent-world HUD; growth/novelty.
- `autopilot.bitcoin_payment_visualization.v1` (Autopilot desktop) — OFF — gold-particle settlement viz; novelty (mirrors real receipts but not the loop).
- `autopilot.pylon_growth_visualization.v1` (Autopilot desktop) — OFF — crystal-growth viz; novelty.
- `autopilot.repo_study_packets.v1` (Autopilot repo studying) — OFF — repo-study capability.
- `autopilot.external_repo_studying_pilot.v1` (Autopilot repo studying) — OFF — repo-study pilot.
- `artanis.pylon_support_responder.v1` (Pylon) — OFF — support agent.
- `artanis.labor_requester.v1` (Pylon) — OFF — agent labor-requester.
- `autopilot_sites.native_email_sequences.v1` (Sites) — OFF — site email sequences.
- `autopilot_sites.custom_tenant_hostnames.v1` (Sites) — OFF — custom hostnames.
- `autopilot_sites.site_build_and_host.v1` (Sites) — OFF — site build/host.
- `autopilot.builtin_compute_agent.v1` (autopilot) — OFF — built-in compute agent; release work.
- `autopilot.local_apple_fm_tool_chat.v1` (autopilot) — OFF — local Apple FM chat; release work.
- `metrics.accepted_outcomes_per_kwh.v1` (metrics) — OFF — AO/kWh metric; needs measured telemetry (energy story, not a $ lane).
- `mobile.voice_approval_companion.v1` (mobile and voice) — OFF — voice approval companion.
- `pylon.first_real_model_training_run.v1` (training) — OFF — first training run.
- `training.device_capability_dataset.v1` (training) — OFF — device-capability dataset.
- `workrooms.omni_client_delivery_workrooms.v1` (workrooms) — OFF — client delivery workrooms.

#### RED — blocked (20)

- `inference.gateway_credits_business.v1` (inference gateway) — **ON** — credits-funded OpenAI/Anthropic-compatible gateway; request surface live, free inference live. Red on the PAID-credits receipt (card/Bitcoin → credit → spend).
- `payments.accepted_outcome_economics.v1` (payments) — **ON** — the seven-state accepted-outcome ledger (authorized/paid/accepted/pending/dispatched/confirmed/reconciled/margin). Green = one accepted outcome with all states evidenced. **This IS the loop's spine.**
- `payments.autopilot_credits_purchase.v1` (payments) — **ON** — buy credits with a card, spend on container/Codex time. Code complete; red on one real prod card purchase + metered spend receipt (owner-gated Stripe secrets).
- `autopilot.cloud_coding_sessions.v1` (Autopilot) — OFF — cloud coding sessions on GCE.
- `cloud.fine_tuning_service.v1` (OpenAgents Cloud) — OFF — buyable fine-tuning primitive.
- `cloud.sandbox_compute_service.v1` (OpenAgents Cloud) — OFF — buyable sandbox primitive.
- `pylon.consumer_compute_earns_bitcoin_self_serve.v1` (Pylon) — OFF — Ep238 consumer-compute-earns claim; needs real settled training payout.
- `pylon.v0_3_multi_earning_node.v1` (Pylon) — OFF — multi-earning node umbrella.
- `compute.agentic_kernel_optimization_at_scale.v1` (compute) — OFF — kernel-optimization research.
- `inference.decentralized_serving_fabric.v1` (inference gateway) — OFF — decentralized serving fabric (big research lane).
- `marketplace.signature_monetization.v1` (marketplace) — OFF — DSPy/GEPA signature monetization.
- `mobile.voice_session_evidence_transcript_ingest.v1` (mobile and voice) — OFF — voice transcript ingest.
- `models.tassadar_percepta_executor.v1` (models) — OFF — Percepta executor architecture (research).
- `autopilot_sites.partner_payout_ledger.v1` (payments) — OFF — partner/agency payout ledger; projection built, but partner attribution unbuilt. A SECOND referral lane — defer behind the primary Sites referral.
- `claims.world_first_ai_training_paid_bitcoin.v1` (public claims) — OFF — Ep238 world-first PR claim.
- `claims.world_first_public_llm_computer_training_run.v1` (public claims) — OFF — Ep238 world-first PR claim.
- `referral.refer_once_earn_forever.v1` (referral) — OFF — ecosystem-wide refer-once-earn-forever; cross-category accrual unbuilt. The headline-grade version of the Sites referral; defer the ecosystem framing, keep the Sites cut.
- `training.public_distributed_training_run.v1` (training) — OFF — public distributed training run.
- `pylon.largest_decentralized_training_claim.v1` (training) — OFF — largest-run benchmark claim.
- `workrooms.source_authorized_business_objects.v1` (workrooms) — OFF — source-authorized business objects.

#### PLANNED — roadmap only (27)

- `autopilot.decision_queue.v1` (Autopilot) — OFF — decision queue UX.
- `autopilot.all_in_one_business_system.v1` (Autopilot) — OFF — all-in-one business system umbrella.
- `autopilot.agent_character_creation.v1` (Autopilot desktop) — OFF — agent character creation (novelty/onboarding).
- `cloud.agent_cloud_one_stop_revshare.v1` (OpenAgents Cloud) — OFF — one-stop Agent Cloud revshare umbrella.
- `cloud.primitives_suite.v1` (OpenAgents Cloud) — OFF — full primitives suite.
- `pylon.compute_revenue_modes.v1` (Pylon) — OFF — GEPA/Tassadar compute revenue modes.
- `pylon.gepa_worker_loop_v03.v1` (Pylon) — OFF — GEPA worker loop.
- `world.multiplayer_agent_world.v1` (agent world) — OFF — walkable multiplayer agent world (MMORPG novelty).
- `pylon.data_trace_revenue.v1` (data) — OFF — data-trace revenue.
- `training.data_refinery_corpus.v1` (data) — OFF — data refinery corpus.
- `energy.flexible_load_proof.v1` (energy) — OFF — flexible-load/energy proof.
- `inference.referral_on_all_inference.v1` (inference gateway) — OFF — referral on all inference (depends on paid gateway existing first).
- `marketplace.agentic_npm_module_registry.v1` (marketplace) — OFF — agentic npm registry.
- `marketplace.wasm_plugins.v1` (marketplace) — OFF — WASM plugins.
- `marketplace.compose_and_list_products.v1` (marketplace) — OFF — compose & list products.
- `marketplace.monetize_any_layer_with_referral.v1` (marketplace) — OFF — monetize any layer with referral.
- `markets.open_protocol_markets.v1` (markets) — OFF — six open-protocol markets.
- `mobile.autopilot_remote_control.v1` (mobile and Autopilot) — OFF — Expo mobile remote control.
- `pylon.five_bitcoin_revenue_streams.v1` (payments) — OFF — five-streams-in-one-install umbrella.
- `training.public_gradient_windows.v1` (training) — OFF — public gradient windows.
- `training.full_pipeline_program.v1` (training) — OFF — full owned training pipeline.
- `training.ablation_system.v1` (training) — OFF — receipted ablation system.
- `training.model_ladder.v1` (training) — OFF — R0–R3 model ladder.
- `training.marathon_operations.v1` (training) — OFF — marathon training ops.
- `training.post_training_arc.v1` (training) — OFF — post-training arc.
- `claims.pursued_world_first_largest_agentic_sales_force.v1` (world-first claims) — OFF — pursuing largest agentic sales force.
- `claims.pursued_world_first_largest_sales_force.v1` (world-first claims) — OFF — pursuing largest sales force of any kind.

#### WITHDRAWN — historical (2)

- `autopilot.historical_claude_code_mechsuit.v1` (Autopilot) — OFF — historical positioning.
- `models.tasadar_percepta_executor.v1` (models) — OFF — typo-duplicate of the Percepta promise; already withdrawn.

---

## 3. Critical path — KEEP / PUSH NOW (21 promises)

These are the minimal set whose green/working state **is** the revenue loop.
Grouped by the hop they own, in dependency order. Each row: current state →
the exact remaining live event needed.

### Hop 0 — the proof/honesty substrate (gates every flip)

| Promise | State | Remaining gate |
|---|---|---|
| `proof.claim_upgrade_receipts.v1` | green | Owner sign-off + record trailing green-flip transition receipts (audit feed already shipped, `lastVerifiedAt 2026-06-20`). Needed so every other flip below is receipt-backed. |
| `proof.demand_provenance.v1` | yellow | Broaden the internal-vs-external-dollar split to stats/leaderboards/run pages + a receipt-first transition. Keeps the "first real $" honestly labeled **external**. |

### Hop 1 — money IN

| Promise | State | Remaining gate (the real live event) |
|---|---|---|
| `forum.content_tipping.v1` | green | Already green with real sats. This is the live $-in surface for the Bitcoin path. |
| `payments.money_dev_kit.v1` | yellow | One real funded-payer → ready-recipient direct tip settling, with webhook-confirmed payment + public receipt lookup (`tip-post-smoke --strict-smooth`). Blocker today: MDK agent-wallet **send capacity** — fund it. |
| `payments.autopilot_credits_purchase.v1` | red | **OWNER-GATED.** One real signed-in user funds a USD credit balance with a card in prod (needs `STRIPE_API_KEY`, `STRIPE_WEBHOOK_SIGNING_SECRET`, `STRIPE_CREDIT_PACKAGES_JSON`) → `billing_ledger_entries` entry tied to a Stripe Checkout Session. Code complete; gate is the secret + one real card. |
| `inference.gateway_credits_business.v1` | red | Same card→credit→spend receipt, applied to inference: USD→msat bridge (#5497) + one funded metered inference request producing a dereferenceable card→credit→inference-spend receipt. |
| `inference.fireworks_open_model_provider.v1` | yellow | Paid-credits path collectable end-to-end + one real customer-completed funded open-model request with metering + settlement. (Same gate as the gateway, expressed on the open-model supply lane.) |
| `api.hosted_gemini.v1` | yellow | Registered-agent production smoke through the gateway with billing/entitlement/metering/settlement refs. |
| `identity.orange_check_forum_signal.v1` | yellow | One live $5 orange-check purchase settling through prod checkout with the badge visible. Cheapest discrete card purchase — a good warm-up for the Stripe rail. |

### Hop 2 — credit / ledger

| Promise | State | Remaining gate |
|---|---|---|
| `autopilot.cloud_credits_ui.v1` | yellow | Purchase flow + spend tracking + cost-accurate preview + settlement receipts wired behind the billing backend (UI already renders/tests pass). |
| `proof.demand_provenance.v1` | (see Hop 0) | — |

### Hop 3 — paid work / inference = accepted outcome

| Promise | State | Remaining gate |
|---|---|---|
| `labor.forum_work_requests.v1` | green | Live end-to-end (request → NIP-90 job → settlement receipts). The accepted-outcome lane that already works. |
| `labor.nostr_negotiation_market.v1` | green | Live (quote → escrow → execute → release → settle). The labor payout engine. |
| `provider.compliant_usage_labor.v1` | yellow | Same compliant flow settling **external sats** over the reliable-tips ladder, running self-serve (not operator-staged). |
| `autopilot.agentic_labor_products.v1` | yellow | One real labor product ordered by an external buyer, carried through the (currently INERT, flag-gated) settlement seam, producing a dereferenceable settlement receipt + owner sign-off. |
| `autopilot.control_center_fanout_marketplace.v1` | yellow | Plugin-marketplace execution beyond `code_task` + a receipt-first owner-signed settlement against an armed self-serve fanout. |
| `pylon.install_without_wallet_knowledge.v1` | green | Green via simulation-backed receipt; the **real** earning step (a `realBitcoinMoved:true` settlement) overlaps Hop 5. |

### Hop 4 — settlement receipt

| Promise | State | Remaining gate |
|---|---|---|
| `payments.accepted_outcome_economics.v1` | red | **The spine.** One accepted outcome with separate authorized / paid / accepted / pending-payout / dispatched / confirmed / reconciled / margin evidence. Blockers: settlement-state-machine incomplete, contributor-ledger missing, gross-margin-receipts missing. Closing this red **is** "closing the loop" in registry terms. |

### Hop 5 — money OUT

| Promise | State | Remaining gate |
|---|---|---|
| `payments.reliable_tips_sweepable_balances.v1` | green | Live with real sats (direct BOLT-12 + automated sweep + refund-on-fail). The proven payout engine. |
| `payments.offline_receive_spark_fallback.v1` | green | Live; two real recipient-confirmed 50k-sat payouts. Resilience for the OUT hop. |
| `sites.referral_bitcoin_stream.v1` | yellow | One real Bitcoin-revenue production event producing a dereferenceable settled referral payout receipt (dispatch already proven vs mock adapter under RL-1 #5458). |
| `agents.x_claim_reward.v1` | yellow | One live operator-dispatched 1000-sat reward settled to a real owner receive code with public-safe receipt refs. Smallest real outbound payout — a clean OUT-hop proof. |

### Shortest path to first real $ in→out with a receipt

**Recommended (no owner secret needed): the Bitcoin tip rail.**

1. Fund a production agent wallet from the funded MDK treasury (sats IN).
2. Post a ready-recipient forum post.
3. Send a real direct tip (`forum.content_tipping.v1` + `payments.money_dev_kit.v1`)
   → webhook-confirmed → instant ledger credit → automated sweep pays OUT to the
   registered offer (`payments.reliable_tips_sweepable_balances.v1`).
4. Dereference the settled-sats receipt.

This closes in→ledger→out **today**, gated only on MDK send-capacity (fundable
by us), and is the concrete "Bitcoin working" artifact `STAGING_E2E_REPORT.md`
flags as NEXT.

**Owner-gated parallel cut: the card rail.** The instant the owner says
"prod stripe", run `identity.orange_check_forum_signal.v1` ($5, smallest
discrete card purchase) first as the card-rail smoke, then
`payments.autopilot_credits_purchase.v1` + `inference.gateway_credits_business.v1`
for the full card→credit→inference-spend receipt. All code exists; the gate is
the three Stripe secrets + one real card.

**Then the spine + a payout:** record the
`payments.accepted_outcome_economics.v1` evidence across the eight states from
whichever rail closed first, and settle one
`sites.referral_bitcoin_stream.v1` or `agents.x_claim_reward.v1` payout to prove
the OUT hop with a public receipt.

---

## 4. Postpone / withdraw (~20%, 23 promises)

The lowest-value-for-48h-revenue promises. **Postpone = keep the code and the
promise, stop spending fleet cycles chasing its green this week. Withdraw =
remove from active driving (PR/novelty claims not worth live copy now).**
Nothing here should be *deleted* from the registry by this proposal — these are
driving-priority recommendations.

### Withdraw (4) — novelty / world-first PR claims, not revenue

| Promise | State | Rec | Rationale |
|---|---|---|---|
| `world.multiplayer_agent_world.v1` | planned | **withdraw from active driving** | Walkable multiplayer agent-world MMORPG — pure growth/novelty, zero revenue path. |
| `autopilot.agent_character_creation.v1` | planned | **withdraw from active driving** | Agent "character creation" onboarding novelty; not on any $ hop. |
| `claims.pursued_world_first_largest_agentic_sales_force.v1` | planned | **withdraw from active driving** | Aspirational world-first PR claim; explicitly "pursuing, not claiming." No revenue dependency. |
| `claims.pursued_world_first_largest_sales_force.v1` | planned | **withdraw from active driving** | Same — 7M-seller bar is narrative, not a 48h loop. |

### Postpone (19) — built-but-off-path, research lanes, nice-to-haves

| Promise | State | Rec | Rationale |
|---|---|---|---|
| `autopilot.agent_world_scene.v1` | yellow | postpone | 3D agent-world HUD; growth/novelty, not the loop. |
| `autopilot.bitcoin_payment_visualization.v1` | yellow | postpone | Settlement *visualization* mirrors real receipts but does not produce one; defer behind the real receipt. |
| `autopilot.pylon_growth_visualization.v1` | yellow | postpone | Crystal-growth viz; novelty. |
| `training.full_pipeline_program.v1` | planned | postpone | Full owned training pipeline — long research program, no 48h revenue. |
| `training.ablation_system.v1` | planned | postpone | Receipted ablation system; research lane. |
| `training.data_refinery_corpus.v1` | planned | postpone | Data refinery corpus; research lane. |
| `training.model_ladder.v1` | planned | postpone | R0–R3 ladder; research lane. |
| `training.marathon_operations.v1` | planned | postpone | Marathon ops discipline; research lane. |
| `training.post_training_arc.v1` | planned | postpone | Post-training arc; research lane. |
| `training.public_gradient_windows.v1` | planned | postpone | Public gradient windows; research/training. |
| `cloud.fine_tuning_service.v1` | red | postpone | Buyable fine-tuning primitive — big build, not a 48h loop. |
| `cloud.sandbox_compute_service.v1` | red | postpone | Buyable sandbox primitive — big build, not a 48h loop. |
| `cloud.primitives_suite.v1` | planned | postpone | Full primitives suite umbrella; depends on the above. |
| `cloud.agent_cloud_one_stop_revshare.v1` | planned | postpone | One-stop Agent Cloud umbrella; depends on the suite. |
| `compute.agentic_kernel_optimization_at_scale.v1` | red | postpone | Kernel-optimization research; no revenue path this week. |
| `inference.decentralized_serving_fabric.v1` | red | postpone | Decentralized serving fabric; large research lane. The paid gateway loop uses Fireworks/Gemini supply today. |
| `marketplace.wasm_plugins.v1` | planned | postpone | WASM plugin packages; nice-to-have. |
| `marketplace.agentic_npm_module_registry.v1` | planned | postpone | Agentic npm registry; nice-to-have. |
| `marketplace.signature_monetization.v1` | red | postpone | DSPy/GEPA signature monetization; speculative, off-loop. |

> Borderline items deliberately **kept on path**, not postponed:
> `inference.referral_on_all_inference.v1`, `referral.refer_once_earn_forever.v1`,
> `autopilot_sites.partner_payout_ledger.v1`,
> `marketplace.monetize_any_layer_with_referral.v1`, and
> `markets.open_protocol_markets.v1` are all referral/market-revenue lanes. They
> are **secondary** to the primary `sites.referral_bitcoin_stream.v1` cut and
> should not be *driven* this week, but they are not in the postpone list
> because they share machinery with the critical path — the first real Sites
> referral payout clears the headline blocker for several of them at once.
> Treat them as "ride the primary lane," not "spend separate cycles."

---

## 5. Proposed tightened registry (the 48h focus)

**Drive these 21 (Section 3) and nothing else for green flips this week.**
Already-green (8 of 21): `proof.claim_upgrade_receipts`, `forum.content_tipping`,
`labor.forum_work_requests`, `labor.nostr_negotiation_market`,
`pylon.install_without_wallet_knowledge`,
`payments.reliable_tips_sweepable_balances`,
`payments.offline_receive_spark_fallback`, (and the loop's existing greens hold).

**The single registry target that means "the loop closed":**
`payments.accepted_outcome_economics.v1` going from **red → green** with one
accepted outcome carrying all eight settlement-state evidence rows. Everything
else in Section 3 either feeds that or proves a specific hop.

**Stop spending fleet cycles on:** all 23 items in Section 4 — every training
research lane, the cloud primitives/fine-tuning/sandbox builds, the
serving-fabric and kernel-optimization research, the marketplace plugin/registry
nice-to-haves, and the agent-world / character-creation / world-first-sales-force
novelty. Park their PRs; do not chase their greens until the loop closes.

**Net effect:** fleet attention collapses from "advance all 29+ yellows / 20
reds / 27 planned" to **~13 still-moving promises** (the 21 critical minus the 8
already green), with one of them — `accepted_outcome_economics` — as the
headline.

---

## 6. 48-hour execution plan

Ordered. Owner-gated unblocks are marked **[OWNER]**.

**T+0 → T+8h — Close the Bitcoin loop (no owner secret needed):**

1. Fund the production MDK agent wallet from the funded treasury to clear the
   `mdk_agent_wallet_send_readiness_insufficient_capacity` blocker on
   `payments.money_dev_kit.v1`.
2. Run the live Bitcoin tip smoke: register prod agent → fund in sats →
   ready-recipient post → real direct tip → webhook-confirmed → automated sweep
   payout → dereference the settled-sats receipt
   (`forum.content_tipping.v1` + `payments.reliable_tips_sweepable_balances.v1`).
   **This is the first real $ in→out with a receipt.**
3. Record the receipt-first transition evidence; **[OWNER]** sign off the
   green-flip transitions per `proof.claim_upgrade_receipts.v1`.

**T+8h → T+24h — The accepted-outcome spine:**

4. Drive one accepted outcome (a real `labor.forum_work_requests` /
   `labor.nostr_negotiation_market` job, or the Bitcoin tip above) through the
   eight-state ledger to clear `payments.accepted_outcome_economics.v1` blockers
   (settlement-state-machine, contributor-ledger, gross-margin-receipts). Land
   one accepted outcome with authorized/paid/accepted/pending/dispatched/
   confirmed/reconciled/margin evidence. **This is the registry definition of
   "loop closed."**
5. Settle one outbound proof: `agents.x_claim_reward.v1` (1000-sat, smallest)
   **or** `sites.referral_bitcoin_stream.v1` (first real Bitcoin referral
   payout) to prove the OUT hop with a public receipt.
6. Extend `proof.demand_provenance.v1` so the new real settlement is labeled
   **external** dollars across the public projections.

**[OWNER] decision point — go live with real dollars (the card rail):**

7. **[OWNER]** Say "prod stripe" and set `STRIPE_API_KEY`,
   `STRIPE_WEBHOOK_SIGNING_SECRET`, `STRIPE_CREDIT_PACKAGES_JSON` in prod.
8. Card-rail smoke #1 (smallest): one live **$5 orange check** purchase →
   `identity.orange_check_forum_signal.v1` green.
9. Card-rail smoke #2 (the full credits loop): one real signed-in user funds a
   USD credit balance with a card → `billing_ledger_entries` entry tied to a
   Stripe Checkout Session → one metered spend → public receipt. Clears
   `payments.autopilot_credits_purchase.v1`, and via the USD→msat bridge (#5497)
   one funded metered inference request clears
   `inference.gateway_credits_business.v1` +
   `inference.fireworks_open_model_provider.v1` + `api.hosted_gemini.v1`.

**[OWNER] sign-offs needed (from `NEEDS_OWNER.md`):**

- **[OWNER]** Green-flip sign-offs (reply `green <id>`) as each real receipt
  lands.
- **[OWNER]** Prod Stripe secrets (deferred until "the whole system works" — but
  the Bitcoin loop above closes the loop *without* this, so the owner can sign
  off the loop-closed milestone before deciding on the card rail).
- **[OWNER]** Funded wallet authorization for the live Bitcoin send (the MDK
  treasury is funded; send-capacity top-up is the concrete action).
- Optional: prod `OPENAGENTS_ADMIN_API_TOKEN` to backfill audit-trail receipts
  for the ~10 greens missing them (unblocks the
  `proof.claim_upgrade_receipts.v1` green flip).

**Definition of done (48h):** at least one of {Bitcoin tip loop, card→credit→
inference loop} has closed in **production** with a dereferenceable receipt at
every hop, and `payments.accepted_outcome_economics.v1` has one accepted outcome
with full eight-state evidence. That is the revenue loop, closed.

---

## Appendix — sources

- Live registry: `https://openagents.com/api/public/product-promises` (`2026-06-20.48`)
- Registry source of truth: `apps/openagents.com/workers/api/src/product-promises.ts`
- Staging E2E proof: `~/work/STAGING_E2E_REPORT.md`
- Owner action queue: `~/work/NEEDS_OWNER.md`
- Ep239 reconciliation: `docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md`
- Credits-purchase audit: `docs/launch/2026-06-19-credits-purchase-collect-money-audit.md`
- Near-term priorities: `docs/launch/2026-06-19-near-term-product-priorities.md`
