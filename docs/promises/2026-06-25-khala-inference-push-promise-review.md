# Khala Inference Push — Product-Promise Review

> Status: **internal promise review, 2026-06-25.** Analysis only. This review
> flips **no** product-promise state — the registry
> (`apps/openagents.com/workers/api/src/product-promises.ts`) is owner-gated, and
> every green transition still needs exact evidence refs, a receipt-first
> transition proof, and owner sign-off per `proof.claim_upgrade_receipts.v1`. It
> is the promise companion to the GTM strategy doc
> [`../inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md).
> Conservative, receipt-first; see also
> [`README.md`](README.md),
> [`checks-and-gates.md`](checks-and-gates.md), and
> [`2026-06-23-khala-public-copy-promise-gate-review.md`](2026-06-23-khala-public-copy-promise-gate-review.md).

## 1. Promises this GTM push directly advances

The inference push is built on the live free OpenAI-compatible API + public
counter. These existing registry promises are the ones it moves:

| promiseId | state | How this push advances it | What it still needs for green |
| --- | --- | --- | --- |
| `inference.free_tier_taste.v1` | yellow | Pillars 1–2 drive real usage of the live free lane (`openagents/khala`, `POST /api/keys/free`, 2,000 req / 2.5M tok/day). The more tools point at the free tier, the stronger the live free-API evidence. | `blocker.product_promises.inference_free_taste_paid_upgrade_not_collectable` — green needs the collectable paid loop so free→paid is a sellable product, plus a receipt-first upgrade. (Owner-gated; out of scope for the GTM push.) |
| `inference.gateway_credits_business.v1` | red | The gateway request surface this promise describes (`POST /v1/chat/completions`, key-auth, balance gate, routing, receipt-first metering) is the exact surface the push grows demand on. More demand = more pressure/evidence for the paid loop. | `..._inference_paid_credits_card_to_credit_not_collectable`, `..._inference_paid_receipt_not_yet_supplied`, `..._inference_mpp_owner_activation_pending`, `public_paid_model_gateway_missing` — needs a real card/MPP→credit→inference-spend settled receipt, owner-signed. |
| `inference.fireworks_open_model_provider.v1` | yellow | Benchmarking (Pillar 3) compares Khala vs the Fireworks open-model lane; the dogfood traffic exercises it as live supply. | `..._inference_paid_credits_card_to_credit_not_collectable`, `..._inference_open_model_paid_product_no_receipt` — needs a real customer-completed funded open-model request with dereferenceable metering + settlement. |
| `api.hosted_gemini.v1` | yellow | Khala's free lane serves Gemini Flash today; dogfood + tools exercise the hosted path. | `..._public_paid_model_gateway_missing`, `..._production_hosted_gemini_executor_binding_missing`. |
| `inference.referral_on_all_inference.v1` | planned | The "every tool points at us" distribution channel is the referral thesis' demand engine; landing tools seeds the accounts a referral would later accrue on. | `..._inference_paid_credits_card_to_credit_not_collectable`, `..._inference_referral_attribution_unbuilt`, `..._inference_referral_accrual_unbuilt`, `referral.first_real_payout_pending`. |
| `inference.decentralized_serving_fabric.v1` | red | The Verse-uses-Khala and gym dogfood lanes (direction) and growing demand build the case for Pylon-served supply later. | `..._pylon_inference_serving_unbuilt`, `..._shard_wan_large_model_serving_psionic_planned`, `..._inference_serving_node_payout_unbuilt`, `..._inference_serving_first_real_payout_owner_armed`, `..._inference_paid_credits_card_to_credit_not_collectable`. |
| `cloud.primitives_suite.v1` / `cloud.agent_cloud_one_stop_revshare.v1` | planned | Inference is the first live primitive of the "Agent Cloud one-stop" capstone; making it the demand anchor is the right sequencing. | Both gated on the unbuilt unified balance + cross-category revshare + the same paid-credits collectable blocker. |
| `metrics.accepted_outcomes_per_kwh.v1` | yellow | The benchmark harness' cost-per-accepted-outcome + verified-rate axes feed this energy/outcome metric. | Its own metric/evidence blockers (separate lane). |
| `training.decentralized_training_launch.v1` (green) / `training.verification_classes.v1` (green) / `compute.tassadar_executor_poc.v1` (green) | green | The gym-trains-Khala lane (direction) sits on this already-green training + verification foundation; the benchmark verification classes reuse it. | Already green; no action. The gym↔Khala *product* wiring is new direction, not covered by these. |

**Net:** the push advances the inference family's evidence and demand, but it does
**not** by itself flip any of these green — every one of them is gated on the same
**collectable paid loop** (`..._inference_paid_credits_card_to_credit_not_collectable`)
plus, where relevant, owner-armed MPP/Stripe activation. That is the real bottleneck
and it is owner-gated. The GTM push is deliberately sequenced to ship the
free-API + demand work that does **not** depend on that gate.

## 2. Missing promise(s) for this push

The registry has **no** promise that cleanly asserts the two things the push is
publicly built on:

1. **"Khala OpenAI-compatible inference is free + live."** Today this is split
   across `inference.free_tier_taste.v1` (yellow, framed as a bounded taste on a
   paid loop) and the gateway-live evidence buried inside
   `inference.gateway_credits_business.v1` (red, framed as a *credits business*).
   Neither is a clean, standalone, green-eligible "the free live API exists"
   assertion — which is exactly the claim the GTM push leans on hardest.
2. **"Tokens served per day is a public metric."** No promise asserts the live
   `khala-tokens-served` counter + history as a public, agent-readable metric,
   even though it ships and is the push's north-star number. (The only adjacent
   metric promise is `metrics.accepted_outcomes_per_kwh.v1`, which is a different
   measure.)

### Proposed new promise A — free live OpenAI-compatible API

> Conservative, receipt-first wording. **Suggestion only — do not create or flip;
> owner-gated.** This is plausibly **green-eligible today** because its evidence is
> already live and dereferenceable, which is precisely why it should be its own
> record rather than hiding inside two non-green promises.

- **promiseId (proposed):** `inference.khala_free_openai_compatible_api.v1`
- **productArea:** inference
- **state (proposed):** green *(eligible — all evidence is live; owner sign-off +
  receipt-first transition still required)*
- **safeCopy (proposed):** "Khala is a free, live, OpenAI-compatible inference
  API. Point any OpenAI-compatible client at `https://openagents.com/api/v1` with
  model `openagents/khala`, mint a free key at `POST /api/keys/free`, and run
  inference free within a per-key daily quota (2,000 requests / 2,500,000 tokens per
  UTC day). Over-quota and premium lanes require credits. Free inference serves
  own-infra / non-premium models only; the `claude`/premium lanes are never free."
- **unsafeCopy (must-not-say):** "Paid Khala is generally launched"; "you can fund
  inference end-to-end via card/Bitcoin/MPP" (those belong to the red/yellow paid
  promises); "unlimited free inference."
- **evidence (proposed):** `POST /api/v1/chat/completions` live (`index.ts`);
  `POST /api/keys/free` live (`free-key-mint-routes.ts`,
  `inference/inference-free-tier-key.ts`, quota constants
  `FREE_TIER_MAX_REQUESTS_PER_DAY=2_000`, `FREE_TIER_MAX_TOKENS_PER_DAY=2_500_000`);
  free lane policy (own-infra only, premium excluded) in the same module; capability
  manifest + OpenAPI entries.
- **rationale:** Splitting the free-API claim out of the credits-business red
  promise lets us make the honest, currently-true claim cleanly without implying
  the paid loop. It is the single most-used claim in the GTM push.

### Proposed new promise B — public tokens-served metric

- **promiseId (proposed):** `metrics.khala_tokens_served_public.v1`
- **productArea:** metrics (or inference)
- **state (proposed):** green *(eligible — counter is live and verified)*
- **safeCopy (proposed):** "OpenAgents publishes a live public count of Khala
  inference tokens served, with per-day history, at
  `GET /api/public/khala-tokens-served` and `/history` (also shown on `/stats` and
  `/khala`). The counter increments on every completion."
- **unsafeCopy:** any claim that the counter implies paid revenue, external-only
  demand, or a specific business outcome — it counts served tokens (internal +
  external), nothing more.
- **evidence (proposed):** `public-khala-tokens-served-routes.ts`,
  `public-khala-tokens-served-history-routes.ts`, `token-usage-ledger.ts`, route
  registration in `index.ts`; the verified-to-the-token-under-24-wide-concurrency
  test note.
- **caveat to bake in:** the copy must not let the number be read as external
  traction; per the GTM doc, keep internal vs external distinguishable in analytics.

> Both proposed promises are green-*eligible* but still owner-gated: creating them
> and (if chosen) flipping them green requires recording the backing transition
> receipts against the deployed registry per `proof.claim_upgrade_receipts.v1`.
> This review only proposes the wording.

## 3. Suggested deprioritizations (park while we focus on inference)

Suggestions only — no state flips, nothing withdrawn. These are promises/areas
**not on the inference push's critical path** that could be parked so attention and
agent lanes concentrate on tokens-served growth. One-line rationale each. (Note:
several are already `planned`, so "deprioritize" here means *do not pull them into
active lanes during the push*, not "change their state.")

| Area / promiseId | Current state | Why park during the inference push |
| --- | --- | --- |
| `cloud.fine_tuning_service.v1` | red | Separate sellable product (OpenAI-shaped FT lifecycle); inert scaffold; no demand wedge vs inference. Park until inference demand justifies it. |
| `cloud.sandbox_compute_service.v1` | red | Same — separate rentable-compute product, inert scaffold; not on the tokens-served path. |
| `inference.batch_processing_jobs.v1` | planned | Batch surface is unbuilt and additive; single-request gateway is the wedge. Defer until interactive demand is exponential. |
| `training.public_gradient_windows.v1`, `training.full_pipeline_program.v1`, `training.ablation_system.v1`, `training.data_refinery_corpus.v1`, `training.model_ladder.v1`, `training.marathon_operations.v1`, `training.post_training_arc.v1`, `training.device_capability_dataset.v1` | planned | The broad training program is large and not the GTM wedge. Keep only the gym→Khala dogfood slice that *uses/improves Khala*; park the rest of the training-promise expansion. |
| `pylon.largest_decentralized_training_claim.v1`, `pylon.first_real_model_training_run.v1`, `claims.world_first_*` | planned/red | World-first/largest *claims* are owner-signed, high-risk, and orthogonal to inference adoption. Do not spend lanes chasing these during the push. |
| `marketplace.agentic_npm_module_registry.v1`, `marketplace.signature_monetization.v1`, `marketplace.wasm_plugins.v1`, `marketplace.compose_and_list_products.v1`, `marketplace.monetize_any_layer_with_referral.v1` | planned | The plugin/module marketplace is the long-run Khala-Blueprint direction but unbuilt; it is downstream of having demand. Park until tokens-served is growing. |
| `markets.open_protocol_markets.v1` | planned | Liquidity/risk/compute-data markets are far-future relative to "be adoptable today." |
| `autopilot_sites.*` (e.g. `autopilot_sites.native_email_sequences.v1`, `custom_tenant_hostnames.v1`, `partner_payout_ledger.v1`, `site_build_and_host.v1`) | yellow/red | Sites is a distinct product line; not on the inference wedge. Keep maintenance only. |
| `mobile.*` (`voice_approval_companion.v1`, `autopilot_remote_control.v1`, `voice_session_evidence_transcript_ingest.v1`) | yellow/red/planned | Mobile operator surfaces are orthogonal to inference adoption; park new lanes. |
| `world.multiplayer_agent_world.v1` & broad Verse work | planned | Keep only the thin "Verse visualizes Khala traffic" dogfood slice (Pillar 1.5); park the broader multiplayer-world promise during the push. |
| `energy.flexible_load_proof.v1`, `compute.agentic_kernel_optimization_at_scale.v1` | planned | Compute/energy demonstrations are not the adoption wedge; defer. |

**Explicitly NOT deprioritized (stay hot):** the inference family
(`inference.*`), the credits/billing seam that unblocks the paid loop
(`payments.autopilot_credits_purchase.v1` red, `autopilot.cloud_credits_ui.v1`
yellow), the benchmark harness work, and the green training/verification
foundation the gym dogfood reuses. The paid-loop blocker
(`..._inference_paid_credits_card_to_credit_not_collectable`) is the one
owner-gated item worth keeping warm because it unblocks the most inference
promises at once — but it does not block the free-API + demand + ecosystem work,
which is why the GTM push ships those first.

## 4. Bottom line

- The push advances the whole `inference.*` family's **evidence and demand** but
  flips nothing green by itself; the shared bottleneck is the owner-gated
  collectable paid loop.
- Two **green-eligible** promises are **missing** and should exist: a clean
  "free live OpenAI-compatible Khala API" promise and a "public tokens-served
  metric" promise. Wording proposed above; creation/flip is owner-gated.
- Park the fine-tuning/sandbox/batch, broad-training, marketplace, world,
  mobile, sites, and world-first-claim lanes during the push; concentrate lanes
  on tokens-served growth (dogfood + OpenCode + benchmark) and on keeping the
  paid-loop blocker warm.
