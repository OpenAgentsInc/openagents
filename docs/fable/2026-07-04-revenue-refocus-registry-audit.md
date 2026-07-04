# Revenue-Refocus Registry Audit — Everything Points at the Loop

Date: 2026-07-04
Status: owner-directed registry pass + company-state audit in the Fable
lane. This pass ships registry `2026-07-04.8` (29 owner-directed demotions
to planned, **zero green flips** — green stays exactly 34). The demotion
authority is the owner's 2026-07-04 direction: everything outside Khala
Code and the active revenue plan that is not already green moves to
planned; QA Swarm and everything directly monetizable in this push is
preserved.

## 1. What was done (the registry pass, exactly)

**Registry `2026-07-04.8`** (canonical:
`apps/openagents.com/workers/api/src/product-promises.ts`; narrative
mirror `docs/promises/registry.md`; served at
`GET /api/public/product-promises`).

State distribution, before → after:

| State | Before (.7) | After (.8) |
| --- | --- | --- |
| green | 34 | **34 (untouched)** |
| yellow | 42 | 23 |
| red | 15 | 5 |
| planned | 47 | 76 |
| withdrawn | 3 | 3 |
| total | 141 | 141 |

**Demoted to planned (29 — 19 yellow, 10 red):**

- *Legacy Autopilot desktop feature family (12):*
  `autopilot.desktop_gui_client.v1`, `autopilot.mission_briefing.v1`,
  `autopilot.builtin_compute_agent.v1`, `autopilot.cloud_credits_ui.v1`,
  `autopilot.control_center_fanout_marketplace.v1` (the plugin-marketplace
  surface — named in the owner directive),
  `autopilot.repo_study_packets.v1`,
  `autopilot.external_repo_studying_pilot.v1`,
  `autopilot.agent_character_creation.v1`,
  `autopilot.agentic_labor_products.v1`,
  `autopilot.bitcoin_payment_visualization.v1`,
  `autopilot.pylon_growth_visualization.v1`,
  `autopilot.local_apple_fm_tool_chat.v1`
- *Training-run claims (2):* `claims.world_first_ai_training_paid_bitcoin.v1`,
  `claims.world_first_public_llm_computer_training_run.v1`
- *Pylon compute-mining (2):*
  `pylon.consumer_compute_earns_bitcoin_self_serve.v1`,
  `pylon.v0_3_multi_earning_node.v1`
- *Cloud service suite (2):* `cloud.fine_tuning_service.v1`,
  `cloud.sandbox_compute_service.v1`
- *Artanis labor lanes (2):* `artanis.labor_requester.v1`,
  `artanis.pylon_support_responder.v1`
- *Mobile companions (2):* `mobile.voice_approval_companion.v1`,
  `mobile.voice_session_evidence_transcript_ingest.v1`
- *Referral bitcoin streams (2):* `sites.referral_bitcoin_stream.v1`,
  `autopilot_sites.partner_payout_ledger.v1`
- *Singles (5):* `inference.decentralized_serving_fabric.v1`,
  `contributors.bounties_surface.v1`, `provider.compliant_usage_labor.v1`,
  `metrics.accepted_outcomes_per_kwh.v1`, `agents.x_claim_reward.v1`

**Explicitly preserved non-green (in-focus):** the full `khala_code.*`
family; `qa.agentic_qa_runner.v1` + all `qa_swarm.*`; all `business.*`
packs and intake; `workrooms.*` (client fulfillment surface); the capture/
privacy pair (`data.*`, `privacy.khala_paid_capture_optout.v1`);
free-tier/provider/gateway-credits inference records; `payments.*`
(credits purchase red, accepted-outcome economics red — both staged to
owner arming); the `autopilot_sites` site/email/hostname fulfillment
surfaces; `identity.orange_check_forum_signal.v1`;
`autopilot.cloud_coding_sessions.v1` (Reactor-adjacent isolated-compute
lane); and `referral.refer_once_earn_forever.v1`, which **stays red on
purpose** as the standing overclaim marker the Lead Gen plans depend on.

**Already covered, no change needed:** every `training.*` and
`marketplace.*` record was already planned; the Reactor records
(`reactor.private_deployment.v1`, `reactor.model_provenance.v1`,
`reactor.model_policy.v1`) landed planned in `2026-07-04.7` (RX-1 #8271).
RX-2 (#8272) later added `packages/reactor-contracts` as source evidence for
the provenance catalog and policy resolver while keeping those records planned.
The one current-plan record still missing at the time of this audit was
`autopilot.lead_gen.v1` - deliberately left to LG-7 (#8268), whose scope
already included it.

**Demotion semantics (important):** planned here is a *focus statement*,
not an evidence judgment. Every demoted record keeps its evidence refs,
blocker refs, claim text, and public-claim lineage; the former-red records
(world-first training claims, consumer-mining, partner payouts, mobile
evidence ingest, bounties) carry on-camera claim history and **must not be
re-marketed without receipt-first re-promotion**. The `2026-07-04.8`
registry note records all of this in the public feed.

**Test/mirror hygiene shipped in the same pass:** 20 state assertions
across `product-promises.test.ts` and
`promise-transition-receipt-routes.test.ts` updated to the demoted states
(each annotated with the pass); one **pre-existing** broken assertion fixed
(the RL-8 `.not.toContain` on decoded `blockerRefs`, which is not a plain
array — now `Array.from(...).includes(...)`); `docs/promises/registry.md`
header rewritten for `.8`. Verification: all 19 registry tests + 4
transition-receipt tests green; `workers/api` typecheck clean.

## 2. Where we are (the honest inventory, post-refocus)

The registry now says with one glance what the company is doing:

- **34 green records** — the substrate that already works: the free
  OpenAI-compatible Khala API, exact token accounting + public counters,
  own-capacity fleet delegation, Bitcoin rails (tips, MPP, payouts),
  forum/identity, open-source posture, demand-provenance discipline.
- **23 yellow + 5 red records** — now almost entirely the *revenue
  surface*: Khala Code (wrapper yellow pending the outside-user receipt;
  RL-1..9 closed the release artifact, download surface, outside-user
  evidence intake, paid-plan payment leg, in-app payment connection,
  consented capture, plugin-precedent spine, QA Swarm intake, and
  first-dollar provenance automation), the business packs and rate card,
  QA Swarm's four records, workrooms, capture/privacy, and the payment
  reds staged to single owner arming actions.
- **76 planned records** — everything else, honestly parked: training
  runs, marketplaces, mining, mobile, cloud suite, world/energy ambitions,
  and now the legacy Autopilot desktop family. Parked is not dead: each
  keeps its blockers and can be re-promoted receipt-first when focus
  returns.

Execution machinery state: the fable epics landed in sequence —
ROADMAP_QA (#8051) closed, ROADMAP_BIZ (#8073, BF-1..9) closed,
background agents (#8187, BA-A..H) closed, revenue loop (#8244, RL-1..9)
closed. Open is exactly one lane: **SELL (#8261)** — LG-1..9 (Autopilot
Lead Gen: prober, pipeline queue, starter credits, sequence tooling,
renderer, attribution, standing agent, affiliate attribution, partner
routing) and RX-1..11 (Reactor: records ✅, provenance/policy schemas,
policy-enforced Hydralisk-lane serving, harness-attributed eval receipts,
air-gap install, dogfood node, need-to-know layer, data liberation,
improvement ladder, gated pilot, model-custody segment).

## 3. The focus: closing the revenue loop, independence-first

The company objective (Episode 246: "close the gap between what I've been
saying and what we're shipping"; Episode 247: sell-in-public) reduces to
one loop, and the registry is now shaped exactly like it:

1. **Campaign B — "Own your AI" — leads.** Founder-personal outbound to
   15–25 data-rich/regulated accounts (apollo plan §11): declare
   independence from the big labs — your data is your moat, closed
   providers forcing data retention are mining it (Mistral CEO, Friedberg
   as third-party validation), and owning your AI is now practical.
   Deal shapes: Own-your-AI Assessment $7.5–15k → Reactor Pilot +
   internal code forge $25–75k → sovereignty retainer $5–20k/mo. This is
   the highest-ACV honest offer in the portfolio and it sells
   *assessments and scoped pilots deliverable today* while RX-2..6 land.
2. **Campaign A — agent-readiness audits — fills the funnel underneath.**
   95 report-led emails, $100 starter credits, Agent-Ready Quick Wins,
   with QA Swarm and coding quick wins as the four-figure conversion
   layer and as sweeteners inside Campaign B deals.
3. **Khala Code carries the product motion.** RL-1..9 means the loop's
   product leg is staged: signed artifact, download truth, outside-user
   evidence, a paid plan one arming away, consented capture, and the
   n=1 trace→plugin→payout precedent spine. The tool funnels users; the
   users become fleet supply; the services engine sells their outcomes.
4. **Receipts all the way down.** Every stage lands in the BF-9.2
   pipeline queue with source attribution; closes become `business.*`
   receipts (A0.1); revenue events carry internal/external provenance
   (RL-9); the registry scoreboard moves only on receipts; and
   sell-in-public publishes the graphs from receipt-backed numbers only.

What is deliberately *not* the focus, and now visibly so: training runs,
plugin marketplaces, compute mining, mobile companions, cloud primitive
suites, and legacy Autopilot desktop surfaces — all planned, all
preserved, none consuming attention until the loop is closed and paying.

## 4. Owner-decision queue (unchanged by this pass, restated)

The distance from here to receipts remains a short list of owner sittings:
Campaign B target-list approval + personal sends; Campaign A template
approval; the paid-plan/credits arming (`KHALA_CODE_PAID_PLANS_ENABLED` +
Stripe/Lightning credentials); QA Swarm first-engagement send (QS7 gate);
and per-deal pricing authority inside the modeled bands. Everything else
is fleet-shaped and filed under #8261.

## 5. Cross-references

- Registry pass: `product-promises.ts` `2026-07-04.8` + note;
  `docs/promises/registry.md` header.
- Plans: `2026-07-03-apollo-outbound-sales-plan.md` (v3, Campaigns A+B),
  `2026-07-04-reactor-open-model-private-deployment-plan.md` (incl. §4.1
  Hydralisk/Psionic lane policy, §10 Mistral read),
  `2026-07-04-harness-optimization-evolve-the-harness-audit.md`
  (docs/research).
- Epic: [#8261](https://github.com/OpenAgentsInc/openagents/issues/8261)
  (LG #8262–#8270, RX #8271–#8281).
- External validation corpus: `docs/transcripts/external/2026-07-03-friedberg.md`,
  `docs/transcripts/external/2026-07-04-mistral-ceo-enterprise-ai.md`.
- Episodes: 246 (two-product frame, say/ship gap), 247 (sell-in-public,
  Autopilot Lead Gen, coding agent pool).
