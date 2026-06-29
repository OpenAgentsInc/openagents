# Episode 239 ("Let's Make Money" / Closing the Revenue Loop) — product-promise registry reconciliation

- Source of truth: `docs/transcripts/239.md`.
- Registry change: `apps/openagents.com/workers/api/src/product-promises.ts`,
  version `2026-06-19.5` → `2026-06-19.6`.
- Rule applied (HARD): a promise is green ONLY if it is fully in place AND has a
  dereferenceable receipt AND honest scope. **No green flips were applied.** The
  green count stays **20** (unchanged). Every Episode 239 record added below is
  `red`, `planned`, or honestly scoped to an existing partial — never green.

## What Episode 239 actually claims

The video frames OpenAgents as the vertically integrated AI lab with a suite of
product, and "closes the revenue loop" by connecting supply (compute, agentic
labor, plugins) to the buy side, with referral incentives layered across
everything. The owner states the system is "about 80 percent done" with the
"last 20 percent ... over the next 48 hours" and a product launch "next week".
The surface:

1. **Referral — refer once, earn forever, on ALL OpenAgents purchases.** Share a
   link → the joiner is your referral forever → any time they ever buy anything
   in the ecosystem, you earn a tiny piece. (Asterisk in the video: "as long as
   OpenAgents remains solvent.")
2. **Autopilot as the all-in-one business system,** composed of the primitives
   below.
3. **OpenAgents Cloud primitives:** inference, fine-tuning, training, agentic
   tasks, sandbox compute, plus standard web services (a layer on top of
   Cloudflare — the Autopilot Sites product).
4. **The Episode 213 markets:** compute, data, labor, liquidity, risk,
   verification — as open protocols / open markets agents can dip into.
5. **Build your own product from the primitives + list it for sale** in the
   marketplace (agents + humans compose and sell).
6. **Monetize / sell access to ANY layer + earn referrals on it** (refer a
   big bulk-inference client → get a piece).
7. **World firsts being PURSUED (NOT achieved):** largest agentic sales force;
   largest sales force (Avon ~6.5M referenced; "as soon as we hit seven million
   agents ... selling or equipped to sell"). The video is explicit these are
   aspirations: "we don't have any relevant world firsts to report to you here
   aside from we're pursuing two world firsts."

The Bitcoin/sound-money framing (real pricing, paying sats on the edge, attacking
margins) is positioning, not a new product claim; it is already represented by
the payments/compute/training records and needs no new promise.

## GAP REPORT (video claim → promise → state → what's missing)

| # | Video claim | Promise record | State | What's in place | What's missing (green gate) |
|---|---|---|---|---|---|
| 1 | Refer once, earn forever, on ALL purchases | `referral.refer_once_earn_forever.v1` (NEW) | red | Sites 5% referral ledger WIRED in source (attribution → eligibility → readiness-gated approved→dispatched→settled via MDK/Spark, Bitcoin-only); attribution capture live | NO real referral payout has ever settled; no permanent cross-category referral binding; no per-purchase accrual across inference/training/marketplace/etc.; no dereferenceable card/Bitcoin-purchase → referral-payout receipt |
| 2 | Autopilot = all-in-one business system composed of primitives | `autopilot.all_in_one_business_system.v1` (NEW) | planned | Autopilot Sites, decision queue, mission briefing, coding-agent lanes exist as separate scoped records | No single composed "business system" product where the primitives are bought/run/billed from one balance; cross-primitive composition is design intent |
| 3 | OpenAgents Cloud primitives (inference·fine-tuning·training·agentic tasks·sandbox·web services) | `cloud.primitives_suite.v1` (NEW, aggregate); `cloud.fine_tuning_service.v1` (NEW); `cloud.sandbox_compute_service.v1` (NEW) | planned / red / red | inference gateway request surface live (free only); training launch green for one bounded settled scope; Autopilot Sites is the web-services layer | Fine-tuning is unbuilt as a sellable service; sandbox compute is unbuilt as a sellable service; no one credit balance spans the primitives; paid loop not collectable end-to-end |
| 4 | Episode 213 markets (compute·data·labor·liquidity·risk·verification) as open protocols | `markets.open_protocol_markets.v1` (NEW) | planned | labor market green (first settled job #4777), NIP-90 compute/data rails in repo history, verification via exact-trace replay (Tassadar PoC), public unified surface scaffold at `/api/public/markets/open-markets` plus inert liquidity/risk skeleton routes | liquidity market and risk market still skeleton-only; compute/data not broadly live as paid markets; no real participant transactions or settled market receipts across the full set |
| 5 | Build your own product + list for sale in marketplace | `marketplace.compose_and_list_products.v1` (NEW) | planned | typed product-definition scaffold plus inert read-only composed-products listing/discovery projection at `/api/public/marketplace/composed-products`; `marketplace.agentic_npm_module_registry.v1` (planned) + `marketplace.wasm_plugins.v1` (planned) are adjacent | no live composition runtime that provisions primitives into a buyable product; no self-serve listing write/install/use lifecycle; no marketplace billing or settlement |
| 6 | Monetize / sell access to ANY layer + earn referrals on it | `marketplace.monetize_any_layer_with_referral.v1` (NEW) | planned | accepted-outcome → receipt → settle spine exists; Sites referral ledger wired | No per-layer access-selling product; no cross-layer referral accrual; no settled receipt for reselling access to any layer |
| 7 | Pursued world first: largest agentic sales force | `claims.pursued_world_first_largest_agentic_sales_force.v1` (NEW) | planned | Video states it as a pursuit, not an achievement; no agentic sales force exists yet | Aspirational by definition — never green; would require a real, sized, verifiable agentic sales force and an independent prior-art / record review |
| 7 | Pursued world first: largest sales force | `claims.pursued_world_first_largest_sales_force.v1` (NEW) | planned | Same; "~7 million agents" target named as the bar (Avon ~6.5M) | Aspirational by definition — never green; would require ~7M selling/sell-equipped agents and an independent record review |

### Existing records cross-referenced (not flipped)

- `sites.referral_bitcoin_stream.v1` (yellow) — the narrow Sites 5% surface; the
  new ecosystem-wide referral record explicitly distinguishes itself from it.
- `inference.referral_on_all_inference.v1` (planned) — the inference-category
  slice of refer-once-earn-forever; the new ecosystem record is the superset.
- `cloud.agent_cloud_one_stop_revshare.v1` (planned) — the one-balance Agent
  Cloud capstone; the new `cloud.primitives_suite.v1` is the primitives list it
  composes, kept as a distinct record so each primitive's gate is legible.
- `marketplace.agentic_npm_module_registry.v1` / `marketplace.wasm_plugins.v1`
  (planned) — adjacent module/plugin marketplace lanes referenced by the new
  compose-and-list record. Agentic-npm has an inert source-level resolver plus
  verification-on-compose core, but no public registry, install/use runtime,
  billing, or settlement.
- `pylon.five_bitcoin_revenue_streams.v1` (planned), `labor.*` (green/yellow),
  `compute.tassadar_executor_poc.v1` (green), `training.decentralized_training_launch.v1`
  (green, bounded scope) — referenced as spine/market evidence without any state
  change.

No existing promise changed state. The decentralized-training-launch green and
the labor-market greens stand on their own prior receipts and are unaffected.

## Brutally honest "in place vs not" summary

- **Genuinely in place (green, with receipts):** open code map, agent
  instruction sheet, the forum/tips rails, the labor market (one settled job),
  the Tassadar executor PoC, one bounded settled decentralized-training run, and
  the coding-agent execution lanes. None of these is the Episode 239 headline.
- **Built but not collectable / not settled (yellow / red):** the inference
  gateway request surface is live but FREE only (no paid card/Bitcoin → credit →
  spend receipt); the Sites referral ledger is wired but has paid NO real
  payout.
- **Not built (red / planned):** the ecosystem-wide refer-once-earn-forever
  product, fine-tuning as a sellable service, sandbox compute as a sellable
  service, the liquidity and risk markets, compose-and-list-your-own-product,
  monetize-any-layer-with-referral, and the all-in-one composed business system.
- **Aspirational by the video's own words (never green):** the two pursued
  world firsts (largest agentic sales force; largest sales force).

The Episode 239 video is a vision/closing-the-loop narrative. The honest registry
state is: the **rails and the spine exist**, but the **revenue loop is not closed
end-to-end for a single paying customer in any category**, and the marketplace /
fine-tuning / sandbox / liquidity / risk / compose / monetize-any-layer surfaces
are not built. The headline gap to make the video real is **wiring the referral
payout end-to-end** (a real paid event → the wired ledger → a dispatched MDK/Spark
settlement → a dereferenceable receipt).

## 48-hour sprint

Filed as EPIC "Bring 'Let's Make Money' (Ep239) to reality — 48h" with prioritized
children. The headline P0 is the end-to-end referral payout. See the EPIC and child
issues for concrete scope, acceptance (dereferenceable receipt), and which promise
each turns green.
