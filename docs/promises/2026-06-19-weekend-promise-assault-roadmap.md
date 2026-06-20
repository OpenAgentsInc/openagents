# Weekend Promise Assault — drive all 78 non-green product promises green

**Date:** 2026-06-19
**Registry baseline:** `2026-06-19.6` (98 promises: 20 green, 2 withdrawn, **76 non-green** — 30 yellow, 19 red, 27 planned)
**Source of truth:** `apps/openagents.com/workers/api/src/product-promises.ts` →
served live at `/api/public/product-promises`
**Owner mandate:** the vertically-integrated OpenAgents vision IS the non-green
product-promise set; the promises interlock and must be driven green — ideally
this weekend — but **only with dereferenceable receipts and owner sign-off**.

> The owner inventory names "78 non-green." The live registry `2026-06-19.6`
> enumerates **76** non-green records once the 2 `withdrawn` records (historical
> source material, never live) are excluded and the 20 `green` records are
> removed. This roadmap drives the **76 enumerable non-green records** plus
> treats the 2 pursued world-first claims as explicit perpetual pursuits, which
> recovers the owner's "78" framing. If the registry count moves before the
> weekend starts, re-run the extraction (`promiseId` count by state) and update
> the per-domain tables; the grouping below is keyed by promise id, not by
> count.

---

## The hard rule (carried verbatim from the registry and from EPIC #5510)

**A promise goes green ONLY when it is fully in place AND has a dereferenceable
receipt AND honest scope.** No green flip lands without an owner-signed,
receipt-first upgrade per `proof.claim_upgrade_receipts.v1`. This roadmap
describes **work to EARN green**; it is not a claim that anything is green.
Most weekend children below do **not** green their target — they remove a
specific named blocker and produce a specific dereferenceable receipt. Green is
a separate, owner-gated transition recorded as a `promise_transition` receipt at
`/api/public/product-promises/transitions`.

"Dereferenceable receipt" = a public-safe URL, endpoint, settled-payment
receipt, deployed route, signed/notarized artifact, committed test/smoke, or
runbook+evidence that a third party (an agent in the Discord vetting flow, or
the owner) can fetch and independently check. No raw secrets, wallet material,
addresses, or customer-private content in any projection (projection gate).

---

## The interlock map (one paragraph)

OpenAgents is **one vertically-integrated stack, not ten products**. The
**open markets** (compute · data · labor · liquidity · risk · verification)
are the substrate; the **Pylon network** turns idle consumer/operator hardware
into supply that those markets clear; the **Agent Cloud primitives**
(inference · fine-tuning · training · sandbox · agentic tasks · storage) are how
that supply is packaged and sold from **one credit balance**; the **products**
(Autopilot coding agent, Autopilot Sites, workrooms, mobile/voice) are the
demand that consumes the primitives and is the anchor buyer; **training /
Tassadar** continuously produces the models and verified-execution architecture
that make the products better and that the Pylon network actually earns on; the
**revenue loop** (collect real money → credits → referral-on-everything →
recycle into training, network, and incentives) is what turns usage into money
and money into growth; and **identity / proof / verification** is the spine that
keeps every claim honest and every payout attributable. The thesis: **markets →
cloud → products → revenue/referral → better product → more markets**. Each
domain-epic below is a slice of that ring; none greens in isolation, and the
sequencing reflects which slices unblock the most others.

```
                         ┌──────────────────────────────────────────┐
                         │   IDENTITY / PROOF / VERIFICATION (DE-8)  │
                         │  (spine: keeps every claim & payout honest)│
                         └──────────────────────────────────────────┘
                                          ▲
                                          │ attests
   OPEN MARKETS (DE-6) ──supply──▶ PYLON NETWORK (DE-4) ──runs──▶ TRAINING / TASSADAR (DE-5)
   compute·data·labor·               consumer + operator             models + exact-execution
   liquidity·risk·verify             hardware = supply               architecture
        │                                   │                               │
        │ cleared by / packaged as          │ packaged & sold as            │ makes better
        ▼                                   ▼                               ▼
   AGENT CLOUD PRIMITIVES (DE-2) ◀──────────┴───────────────────────────────┘
   inference · fine-tune · training · sandbox · tasks · storage
        │  (one credit balance)
        ▼
   PRODUCTS  =  Autopilot coding agent (DE-3) · Autopilot Sites & Workrooms (DE-9) · Mobile/Voice (DE-7)
        │  consume primitives (anchor demand)
        ▼
   REVENUE LOOP (DE-1):  collect real money → credits → referral-on-everything → settle Bitcoin
        │
        └──recycle──▶ training · network incentives · referral payouts ──▶ back to PYLON / MARKETS
                                                                     ▲
   ENERGY / COMPUTE-AT-SCALE / METRICS / WORLD-FIRSTS (DE-10) ──measures & proves the whole ring──┘
```

---

## The 10 vertical-integration domain-epics

Each domain-epic groups its promises by how the stack interlocks, not by code
ownership. The **master EPIC is #5523**; each domain-epic has its own GitHub
EPIC issue (below).

| # | Domain-epic | Promises | EPIC | Net role in the ring |
|---|---|---|---|---|
| **DE-1** | **Revenue Loop — Referral · Payments · Credits collection** | 8 | #5524 | The ring closes here: usage → money → referral → recycle |
| **DE-2** | **Inference Gateway + Agent Cloud primitives** | 9 | #5525 | Packages network supply into sellable, metered products |
| **DE-3** | **Autopilot product surface (coding agent)** | 11 | #5526 | Anchor demand; the headline launch proof point |
| **DE-4** | **Pylon network + multi-earning node** | 9 | #5527 | Turns hardware into market supply |
| **DE-5** | **Training / Tassadar pipeline** | 13 | #5528 | Produces the models + exact-execution architecture |
| **DE-6** | **Open markets + marketplace** | 6 | #5529 | The protocol substrate everything clears on |
| **DE-7** | **Mobile + voice** | 3 | #5530 | Mobile demand & approval surface |
| **DE-8** | **Identity / proof / verification spine** | 6 | #5531 | Keeps every claim & payout honest |
| **DE-9** | **Workrooms · business objects · Sites delivery** | 4 | #5532 | Business-system demand surface |
| **DE-10** | **Energy · compute-at-scale · metrics · world-firsts** | 7 | #5533 | Measures and proves the whole ring |
| | **Total** | **76** | | All enumerable non-green records covered, no overlaps |

Master EPIC: **#5523** — "Weekend Promise Assault — drive all 76 non-green
product promises green".

---

### DE-1 — Revenue Loop: Referral · Payments · Credits collection

**The single highest-leverage slice. Closes the ring.** Nothing about
"make money" is real until one card→credit→spend→referral→settled-Bitcoin path
is dereferenceable end-to-end. This is the Ep239 headline and the first wave is
already filed (EPIC #5510, Phase #5520 staging / #5521 production).

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `referral.refer_once_earn_forever.v1` | red | Cross-category attribution binding + accrual on top of the proven single-category Sites rail | One real referral → one cross-category purchase → accrued cut → settled Bitcoin payout receipt; ecosystem-wide attribution binding live | Owner-gated (real money) |
| `sites.referral_bitcoin_stream.v1` | yellow | First real settled referral payout through the already-wired RL-1 #5458 dispatch | One real paid event → idempotent approved→dispatched→settled receipt over MDK/Spark; `referral_first_real_payout_pending` cleared | Owner-gated (first real payout); rail is built |
| `autopilot_sites.partner_payout_ledger.v1` | red | Partner attribution policy + projection API + first real partner payout | Partner attribution receipt + `/api/public` partner payout projection + one settled partner payout | Owner-gated |
| `payments.money_dev_kit.v1` | yellow | MDK agent-wallet **send** readiness: sufficient inbound/outbound capacity for real outbound payouts | Send-readiness smoke + a real outbound MDK/Spark settlement receipt; capacity blocker cleared | Owner-gated (custody) |
| `payments.accepted_outcome_economics.v1` | red | Complete settlement state machine + contributor ledger + gross-margin receipts | Each accepted outcome dereferenceable through buyer-payment → accepted-value → payout-intent → settlement → reconciliation → gross-margin | Agent-claimable (state machine, ledger schema, tests) |
| `inference.gateway_credits_business.v1` | red | Make the **paid-credits** path collectable: prod Stripe card→credit + USD→msat bridge feeding a real inference spend | One dereferenceable card→credit→inference-spend receipt (Stripe live + #5497 bridge against a real purchase) | Owner-gated (prod Stripe live keys) |
| `autopilot.cloud_credits_ui.v1` | yellow | Wire credit purchase + spend backend behind the existing presentational UI | Real credit purchase receipt + real metered spend receipt surfaced in the UI | Owner-gated (payment); UI is built |
| `proof.demand_provenance.v1` | planned | Demand-provenance projection: internal vs external dollars, modeled vs measured vs settled | `/api/public` demand-provenance projection with the internal/external + modeled/measured/settled split | Agent-claimable (projection + policy) |

**Sequencing:** P0.5 collect real money (Stripe live + USD→msat) → P0 first real
Sites referral payout → cross-category binding → partner ledger → accepted-outcome
economics + demand provenance as the accounting backbone.
**Interlock:** feeds DE-2 (credits fund inference), recycles into DE-4/DE-5
(network + training incentives), proven honest by DE-8.

---

### DE-2 — Inference Gateway + Agent Cloud primitives

**Packages network supply into sellable, metered products from one balance.**
The OpenAI/Anthropic-compatible gateway is BUILT, DEPLOYED, LIVE (free inference
works; Gemini 3.5 Flash served end-to-end; Fireworks is a registered live supply
lane). It stays non-green because the **paid** product is not collectable and the
other primitives are unbuilt. EPIC #5474, sub-EPIC #5475.

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `inference.gateway_credits_business.v1` | red | (shared with DE-1) paid-credits collectable | card→credit→inference-spend receipt | Owner-gated |
| `inference.fireworks_open_model_provider.v1` | yellow | Sellable paid open-model product on the registered Fireworks supply lane | One paid open-model inference receipt (card/Bitcoin → metered spend) | Owner-gated (paid path) |
| `inference.referral_on_all_inference.v1` | planned | Cross-category ongoing referral revshare on inference spend | Referral attribution + accrual + first real inference-referral payout receipt | Owner-gated; depends on DE-1 |
| `inference.decentralized_serving_fabric.v1` | red | Pylons load weights + serve inference (small whole, large sharded over WAN); serving-node payout | First Pylon-served inference + first serving-node payout receipt (owner-armed) | Agent-claimable (serving runtime); payout owner-gated; large-model is Psionic/hardware-blocked |
| `api.hosted_gemini.v1` | yellow | Production hosted-Gemini executor binding behind the paid gateway | Paid hosted-Gemini call → metered spend receipt | Owner-gated (paid path) |
| `cloud.agent_cloud_one_stop_revshare.v1` | planned | One unified credit balance across all categories + cross-category revshare | Unified-balance projection + a multi-category spend reconciled to one balance with revshare receipt | Owner-gated (capstone); depends on DE-1, DE-2, DE-6 |
| `cloud.primitives_suite.v1` | planned | Expose the full primitive set (inference, fine-tune, training, tasks, sandbox, storage) on a unified balance | Each primitive reachable + billed to the unified balance with a receipt | Mostly agent-claimable per primitive |
| `cloud.fine_tuning_service.v1` | red | Fine-tune intake + job runtime + billing/settlement | Submit base+dataset → run on network → use result through gateway → paid receipt | Agent-claimable (intake/runtime MVP); settlement owner-gated; depends on DE-5 |
| `cloud.sandbox_compute_service.v1` | red | Rentable metered isolated sandbox product | Rent → run → metered → paid sandbox receipt | Agent-claimable (sandbox MVP, see `firecracker` reference); settlement owner-gated |

**Sequencing:** paid-credits (DE-1) first → Fireworks/Gemini paid products →
fine-tuning + sandbox MVPs → unified balance + one-stop revshare capstone.
**Interlock:** consumes DE-4 supply, sells to DE-3 demand, billed through DE-1,
fine-tuning/training depend on DE-5.

---

### DE-3 — Autopilot product surface (the coding agent)

**Anchor demand and the headline launch proof point** (near-term P0, week of
June 22). Desktop coding agent EPIC #5461 is feature-complete; the priority is
shipping it and getting first real usage. EPIC #5441 (AO-1..AO-6 auto-onboarding)
is BUILT/tested; the from-DMG clean-Mac proof is owner-gated.

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `autopilot.desktop_gui_client.v1` | yellow | From-DMG clean-Mac render/presence proof + wire live PDF/preview/ingest/browser runtimes | From-DMG proof receipt (rendered window, prod presence, settled Tassadar receipt) + live-runtime smokes | Owner-gated (clean-Mac proof); runtimes agent-claimable |
| `autopilot.builtin_compute_agent.v1` | yellow | Signed/notarized recut + packaged OpenAgents compute creds + metered from-install go-online smoke | From-install go-online smoke + metered-compute receipt on a signed binary | Owner-gated (signing + creds) |
| `autopilot.local_apple_fm_tool_chat.v1` | yellow | Signed-installer recut + helper supervision for the local Apple FM path | Signed-installer smoke + supervised local FM chat/tool session receipt | Owner-gated (signing) |
| `autopilot.cloud_coding_sessions.v1` | red | Real GCE provisioning (not the fake/stub default), 5005 event round-trip, Pylon remote-bridge transport | Real cloud session spawn→watch→approve→settle receipt over real GCE + working bridge | Agent-claimable (transport/events); GCE provisioning owner-gated |
| `autopilot.mission_briefing.v1` | yellow | Complete drilldown artifact refs + cost/risk receipt rollup | Mission briefing with complete artifact refs + cost/risk/receipt rollup, dereferenceable | Agent-claimable |
| `autopilot.decision_queue.v1` | planned | Decision-queue API with cross-client exactly-once + receipt-backed command closeout | Decision-queue API + exactly-once proof + receipt-backed closeout for each command | Agent-claimable |
| `autopilot.agentic_labor_products.v1` | yellow | Make all labor flows self-serve (not operator-gated) | Self-serve labor flow receipt end-to-end (no operator step) | Agent-claimable |
| `autopilot.control_center_fanout_marketplace.v1` | yellow | Self-serve fanout + plugin marketplace beyond code-task | Self-serve fanout receipt + non-code plugin marketplace dispatch receipt | Agent-claimable; depends on DE-6 |
| `autopilot.repo_study_packets.v1` | yellow | Customer-private validation, privacy review, marketplace metering, pricing, payout, copy review | Customer-private study receipt + metered/paid packet receipt | Owner-gated (privacy/pricing); pipeline agent-claimable |
| `autopilot.external_repo_studying_pilot.v1` | yellow | Customer-private admission, self-serve upload, privacy policy, metering, pricing, payout | Self-serve external-repo study → metered → paid receipt | Owner-gated (privacy/pricing) |
| `autopilot.all_in_one_business_system.v1` | planned | Compose the Cloud primitives into a unified business system with unified billing | One real business run composing ≥3 primitives on unified billing with a receipt | Owner-gated (capstone); depends on DE-2, DE-9 |

**Sequencing:** desktop launch + first-usage receipts (highest near-term value)
→ builtin/Apple-FM signed recuts → cloud sessions → briefing/decision-queue
→ labor/fanout self-serve → repo-study paid → all-in-one capstone.
**Interlock:** consumes DE-2 primitives, is the anchor buyer for DE-1 revenue,
runs on DE-4 Pylon, improved by DE-5 training.

---

### DE-4 — Pylon network + multi-earning node

**Turns idle consumer + operator hardware into market supply.** Pylon v1.0 has a
stable source cut and `@openagentsinc/pylon@latest` is on the v1.0 line; broad
earning, paid assignment, every-platform coverage, and settlement stay gated.

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `pylon.v03_release_candidate.v1` | yellow | Complete v1.0 live-network smokes + signed-binary feed rollout | Live-network smoke receipts + signed/notarized feed rollout proof | Owner-gated (signing); smokes agent-claimable |
| `pylon.release_tomorrow.v1` | yellow | Ship the signed-binary feed rollout | Signed release on `updates.openagents.com` with install smoke | Owner-gated (signing) |
| `pylon.consumer_compute_earns_bitcoin_self_serve.v1` | red | Self-serve scale methodology + Windows/WSL coverage + Spark helper autostart | N distinct self-serve consumer installs auto-earning + autostart receipt | Owner-gated (scale claim); install paths agent-claimable |
| `pylon.v0_3_multi_earning_node.v1` | red | Default install fully closed + multi-earning-mode receipts + settlement refs + safe projection | Receipts for ≥2 earning modes from one install + safe public projection | Agent-claimable (modes/projection); settlement owner-gated |
| `pylon.five_bitcoin_revenue_streams.v1` | planned | Make compute + data + tips + referral + labor live in one install | One install with a settled receipt in each of the 5 streams | Owner-gated (breadth); per-stream agent-claimable |
| `pylon.compute_revenue_modes.v1` | planned | Live GEPA-optimization-slice + Tassadar-executor compute network | Settled compute-mode receipt from a live network slice | Agent-claimable (GEPA loop) |
| `pylon.data_trace_revenue.v1` | planned | Consented, redacted, valued trace mining + settled sale | One settled trace-sale receipt with consent/redaction/valuation | Owner-gated (consent policy) |
| `pylon.gepa_worker_loop_v03.v1` | planned | GEPA-first assignment work through the in-repo runtime, paid | One paid GEPA settlement receipt via the runtime | Agent-claimable |
| `provider.compliant_usage_labor.v1` | yellow | External-ladder settlement + self-serve earning for connected provider accounts | Self-serve provider-account labor → settled receipt | Owner-gated (settlement); flow agent-claimable |

**Sequencing:** signed-feed rollout → multi-earning closeout → per-mode live
networks (GEPA, compute, data, labor) → consumer self-serve scale.
**Interlock:** supply for DE-2/DE-6, runs DE-5 training, earns through DE-1.

---

### DE-5 — Training / Tassadar pipeline

**Produces the models and exact-execution (Percepta) architecture the whole
stack runs on.** The decentralized training launch is GREEN on bounded receipts
(`training.decentralized_training_launch.v1`, two distinct independent
contributors paid real Bitcoin). Everything broader (network-scale,
paid-at-scale, largest-run, full pipeline) stays non-green pending its own
receipts.

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `training.public_distributed_training_run.v1` | red | Public distributed run with visible state, verified work, contributor payment at participant scale | Public-run receipts: participants + accepted work + validation + settlement | Owner-gated (scale); runtime agent-claimable |
| `training.public_gradient_windows.v1` | planned | Live gradient-window runtime + promoted-window receipts + settlement | Accepted→promoted→paid gradient-window receipt advancing the shared checkpoint | Agent-claimable (window runtime); promotion owner-gated |
| `training.full_pipeline_program.v1` | planned | Complete the training-pipeline rails (refinery→ablation→derisk→marathon→post-train→infer) | Each stage reachable with a receipt; end-to-end pipeline run | Mostly agent-claimable per stage |
| `training.ablation_system.v1` | planned | One-delta ablation harness + reproduced eval suite + public derisking ledger | Ablation manifest + eval reproduction + `/api/public` derisking ledger | Agent-claimable |
| `training.data_refinery_corpus.v1` | planned | Crawl-scale corpus as paid CPU work with provenance + transform digests + eval-delta payment | Corpus provenance receipts + eval-delta payment receipt | Agent-claimable (refinery on Pylon CPU) |
| `training.model_ladder.v1` | planned | R1 operator-scale full rehearsal + rung-economics gate format | R1 rehearsal receipt + rung-economics gate doc/projection | Owner-gated (rung economics); rehearsal agent-claimable |
| `training.marathon_operations.v1` | planned | Durable checkpoint seal + standby dispatch + curtailment drill | Marathon-discipline receipts: seal, standby, curtailment drill | Agent-claimable |
| `training.post_training_arc.v1` | planned | Instruct-SFT lane + preference rollout + vibe-test artifact | SFT lane receipt + preference rollout receipt + vibe-test artifact | Agent-claimable |
| `training.verification_classes.v1` | yellow | Re-decide aggregate-only policy; name pluggable verification class per stage | Each stage carries a named verification class with a receipt; policy decision recorded | Owner-gated (policy); classes agent-claimable |
| `training.device_capability_dataset.v1` | yellow | Second device class + thermal-throttle detection + same-host replication caveat resolved | Public device-capability dataset across ≥2 device classes with thermal data | Agent-claimable |
| `pylon.first_real_model_training_run.v1` | yellow | Run the model-ladder network rungs | Network-rung run receipt above the tri-host rehearsal | Owner-gated (rung); runtime agent-claimable |
| `pylon.largest_decentralized_training_claim.v1` | red | Participant methodology + comparable-run evidence + public contributor receipts vs 200-contributor bar | Sized verifiable participant count + comparable-run benchmark + receipts | Owner-gated (world-first-class claim) |
| `models.tassadar_percepta_executor.v1` | red | Tassadar model spec + Percepta executor-architecture receipts + Pylon CPU-transform training receipts | Model spec doc + architecture receipts + CPU-transform training receipt | Agent-claimable (spec/architecture); see `docs/tassadar/`, `llm-as-computer` ref |
| `artanis.tassadar_evolution_loop.v1` | yellow | Unattended-tick streak + Tassadar distillation-dataset receipt | N-tick unattended Artanis streak receipt + distillation-dataset receipt | Agent-claimable |

**Sequencing:** verification classes + device dataset (cheapest) → ablation +
refinery + marathon rails → model ladder R1 → public distributed run at scale →
largest-run claim (latest, world-first-class).
**Interlock:** the engine for DE-3 (better agent) and DE-2 (fine-tuning),
runs on DE-4 Pylon supply, paid through DE-1, attested by DE-8.

---

### DE-6 — Open markets + marketplace

**The protocol substrate everything clears on.** The six Episode 213 markets
(compute · data · labor · liquidity · risk · verification) as open protocols,
plus the composable/listable marketplace on top. Labor + (parts of) compute/data
are live/scoped, and the unified surface scaffold now exists at
`/api/public/markets/open-markets`; liquidity and risk are still inert
skeleton-only, and compute/data are not broadly live paid markets. First wave
filed under EPIC #5510 (markets surface + liquidity/risk skeletons;
compose-and-list MVP).

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `markets.open_protocol_markets.v1` | planned | Turn liquidity + risk skeletons into real markets; broaden compute/data live | All six markets dippable via open protocol + dereferenceable participant transaction and settlement receipts | Agent-claimable only for more scaffolding; live transactions/settlement owner-gated; see `packages/nip90` |
| `marketplace.compose_and_list_products.v1` | planned | Composition runtime + listing lifecycle + billing/settlement | Compose primitives → list → sell → settled receipt | Agent-claimable (MVP) |
| `marketplace.agentic_npm_module_registry.v1` | planned | Agentic-npm registry + module composition runtime + billing | Verified module published, composed, and billed with a receipt | Agent-claimable |
| `marketplace.wasm_plugins.v1` | planned | WASM-plugin registry + execution sandbox + billing | WASM plugin published, sandbox-executed, billed with a receipt | Agent-claimable (see `agent-os`, `firecracker` refs) |
| `marketplace.signature_monetization.v1` | red | DSPy/GEPA signature usage metering + settlement | Signature listed, metered on use, settled with a receipt | Agent-claimable |
| `marketplace.monetize_any_layer_with_referral.v1` | planned | Access product over ANY layer + referral accrual + resale receipt | Resell access to any layer → referral cut → settled receipt | Owner-gated (capstone); depends on DE-1, DE-2, DE-6 |

**Sequencing:** liquidity + risk skeletons + unified surface scaffold are done;
next is real liquidity/risk transactions and broad compute/data receipts →
compose-and-list MVP → agentic-npm / WASM / signature monetization →
monetize-any-layer capstone.
**Interlock:** the substrate DE-4 supplies and DE-2 packages; capstone depends
on DE-1 referral.

---

### DE-7 — Mobile + voice

**Mobile demand and the approval/steer surface.** Build/ship policy: NO Expo/EAS
cloud — local `.ipa` compile + Apple-native TestFlight + OTA via
`updates.openagents.com`.

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `mobile.autopilot_remote_control.v1` | planned | Create the Expo app scaffold + Pylon remote-bridge transport + TestFlight distribution | Pair → watch → approve a real session from phone; TestFlight build receipt | Owner-gated (TestFlight/signing); transport agent-claimable |
| `mobile.voice_approval_companion.v1` | planned | Mobile projection + voice command/approval receipts + cross-device workroom sync | Voice command → approval receipt → synced workroom, dereferenceable | Agent-claimable (projection); depends on DE-9 |
| `mobile.voice_session_evidence_transcript_ingest.v1` | red | Voice ingestion endpoint + transcription service + proposal/approval UI | Spoken command → transcribed → approval-gated proposal → receipt | Agent-claimable (endpoint/UI); see `eagle`/LiveKit ref |

**Sequencing:** Pylon remote-bridge transport (shared with DE-3 cloud sessions)
→ Expo scaffold + TestFlight → voice ingestion → voice companion.
**Interlock:** consumes DE-3/DE-9 surfaces over the same bridge transport.

---

### DE-8 — Identity / proof / verification spine

**Keeps every claim and every payout honest.** This is the discipline layer the
owner mandate rests on — these green-with-receipts before broad copy is safe.

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `proof.claim_upgrade_receipts.v1` | yellow | Enterprise audit panel for receipt-first upgrades + sensitive-route policy | Audit-panel projection of every upgrade with its transition receipt | Agent-claimable |
| `agents.x_claim_reward.v1` | yellow | Live dispatch smoke for the 1000-sat X-verification reward | One real X-verified claim → settled 1000-sat reward receipt | Owner-gated (payout); flow agent-claimable |
| `agents.nostr_fallback_coordination.v1` | yellow | Nostr outage-coordination drill + complete agent Nostr-messaging tooling | Outage drill receipt + Nostr coordination tooling smoke | Agent-claimable (see `packages/nip90`, `nostr-effect`) |
| `identity.orange_check_forum_signal.v1` | yellow | Orange-check Nostr export | Orange-check badge exported as a verifiable Nostr signal | Agent-claimable |
| `artanis.pylon_support_responder.v1` | yellow | External-contributor flow proven + 10 unattended responder ticks | External contributor support reply receipt + 10-tick unattended streak | Agent-claimable |
| `artanis.labor_requester.v1` | yellow | Live labor enablement + unattended request receipts | Artanis-originated bounded escrowed labor request → settled receipt | Agent-claimable |

**Sequencing:** claim-upgrade audit panel (underpins all green flips) → Nostr
fallback + tooling → orange-check export → X reward dispatch → Artanis
responder/requester unattended streaks.
**Interlock:** attests every domain; the audit panel is what makes the weekend's
green flips reviewable.

---

### DE-9 — Workrooms · business objects · Sites delivery

**Business-system demand surface.** Turns chat/files into source-authorized
business objects and routes client delivery.

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `workrooms.source_authorized_business_objects.v1` | red | Source-authority model green + connector read receipts + approval-gated business writes | Chat/files → source-authorized objects with read receipts + approval-gated write receipts | Agent-claimable (model + connectors) |
| `workrooms.omni_client_delivery_workrooms.v1` | yellow | Integrate source authority + approval-gated writes into client-delivery workrooms | Client-scoped delivery workroom with source-authority + approval-gated write receipts | Agent-claimable |
| `autopilot_sites.native_email_sequences.v1` | yellow | Customer UI + email-send service integration + proven deliverability | Sequence built in UI → enrolled subscriber → sent email → deliverability receipt | Owner-gated (send service); UI agent-claimable |
| `autopilot_sites.custom_tenant_hostnames.v1` | yellow | Customer self-serve hostname + SSL issuance + rendering context switch | Self-serve custom hostname live with SSL + correct tenant rendering | Agent-claimable (rendering); SSL owner-gated |

**Sequencing:** source-authority model green → workroom integration → email send
+ hostname self-serve.
**Interlock:** demand for DE-3/DE-1; source authority underpins DE-8 honesty.

---

### DE-10 — Energy · compute-at-scale · metrics · world-first claims

**Measures and proves the whole ring.** The metrics/energy spine plus the
explicit world-first claims (kept honest: pursued, not claimed).

| Promise id | State | Work to EARN green | Dereferenceable-receipt acceptance | Claimability |
|---|---|---|---|---|
| `metrics.accepted_outcomes_per_kwh.v1` | yellow | Measured energy telemetry beyond the single modeled seed datapoint | ≥2 measured AO/kWh datapoints from real telemetry, projected | Agent-claimable (telemetry) |
| `energy.flexible_load_proof.v1` | planned | Energy-market ingestion + work-class flex profiles + flexible-load event history + operator proof report | Flexible-load event history + operator proof report comparing states | Agent-claimable (see `projects/ercot`, `oa_aibtc_model` refs) |
| `compute.agentic_kernel_optimization_at_scale.v1` | red | Throughput-parity verification + market dispatch + at-scale run + settlement | At-scale kernel-optimization run with throughput-parity verification + settled receipts | Agent-claimable (kernels); see `triton`/`flashinfer` refs |
| `claims.world_first_ai_training_paid_bitcoin.v1` | red | World-first evidence pack + owner-signed upgrade | Evidence pack (first AI training paid Bitcoin to consumer compute) + owner sign-off | Owner-gated (world-first) |
| `claims.world_first_public_llm_computer_training_run.v1` | red | LLM-computer training-run definition + evidence pack + owner-signed upgrade | Definition doc + evidence pack + owner sign-off | Owner-gated (world-first) |
| `claims.pursued_world_first_largest_agentic_sales_force.v1` | planned | Sized verifiable agentic sales force (perpetual pursuit) | Sized, verifiable force evidence + owner-signed upgrade | Owner-gated (perpetual pursuit — intentionally not a weekend green) |
| `claims.pursued_world_first_largest_sales_force.v1` | planned | Sized verifiable sales force vs ~7M bar (perpetual pursuit) | Sized, verifiable force evidence vs the named bar + owner-signed upgrade | Owner-gated (perpetual pursuit — intentionally not a weekend green) |

**Sequencing:** AO/kWh measured telemetry → energy flex history → kernel-opt at
scale → world-first evidence packs (latest; require the underlying receipts from
DE-4/DE-5 to exist first). The two **pursued** sales-force claims are explicit
perpetual pursuits, never flipped on a weekend.
**Interlock:** the measurement/honesty layer over the whole ring.

---

## Prioritized weekend sequence

The ordering maximizes unblocking: collect money first (it gates the entire
revenue ring), then ship the anchor product, then broaden the rails.

### Wave 0 — close the money ring (P0, buildable now / owner-gated finish)
1. **DE-1 / DE-2: collect real money** — prod Stripe card→credit + USD→msat
   bridge feeding a real inference spend. *(owner-gated: prod Stripe live keys.)*
   Already filed: EPIC #5510, Phase #5520 (staging) → #5521 (production).
2. **DE-1: first real Sites referral payout** — the rail is wired (RL-1 #5458);
   one real paid event → one settled payout receipt. *(owner-gated: first real
   payout.)* Greens `sites.referral_bitcoin_stream.v1`, clears the headline
   blocker on `referral.refer_once_earn_forever.v1`.

### Wave 1 — ship the anchor product (P0, near-term week of June 22)
3. **DE-3: Autopilot coding-agent launch** — from-DMG clean-Mac proof + first
   real-user coding sessions. *(owner-gated: clean-Mac proof + signing.)*
4. **DE-8: claim-upgrade audit panel** — *(agent-claimable)* — so every Wave-0/1
   green flip is reviewable; underpins the whole assault's honesty rule.

### Wave 2 — broaden the rails (agent-claimable, parallelizable now)
5. **DE-6: real liquidity/risk transactions + broad compute/data receipts**
   *(live transaction/settlement owner-gated; scaffolding agent-claimable)*.
6. **DE-2: fine-tuning + sandbox compute MVPs** *(agent-claimable; settlement
   owner-gated)*.
7. **DE-1: accepted-outcome economics state machine + contributor ledger +
   demand provenance** *(agent-claimable)* — the accounting backbone every payout
   needs.
8. **DE-4: per-mode live networks** (GEPA worker loop, compute modes, data
   traces, provider labor self-serve) *(agent-claimable; settlement owner-gated)*.
9. **DE-5: cheapest training receipts** — verification classes, device-capability
   dataset, ablation/refinery/marathon rails *(agent-claimable)*.
10. **DE-9: source-authority model + workroom integration** *(agent-claimable)*.
11. **DE-3: mission briefing, decision queue, self-serve labor/fanout**
    *(agent-claimable)*.
12. **DE-7: Pylon remote-bridge transport + Expo scaffold + voice ingestion**
    *(agent-claimable; TestFlight/signing owner-gated)*.
13. **DE-8: Nostr fallback, orange-check export, Artanis responder/requester
    streaks** *(agent-claimable)*.
14. **DE-10: AO/kWh measured telemetry, energy flex history, kernel-opt**
    *(agent-claimable)*.

### Wave 3 — capstones (depend on Waves 0-2 receipts; mostly owner-gated)
15. **DE-1: cross-category referral binding** → `referral.refer_once_earn_forever.v1`.
16. **DE-2: unified credit balance + one-stop Agent Cloud revshare**.
17. **DE-6: compose-and-list + monetize-any-layer-with-referral**.
18. **DE-3: all-in-one business system**.
19. **DE-5: public distributed run at participant scale → largest-run claim**.
20. **DE-10: world-first evidence packs** (require the underlying receipts to
    exist first). The two **pursued** sales-force claims stay perpetual pursuits.

### Buildable now vs owner-gated vs agent-claimable (summary)
- **Buildable now (agent-claimable, no owner step to ship code):** DE-8 audit
  panel; DE-6 liquidity/risk skeletons + surface; DE-2 fine-tune/sandbox MVPs;
  DE-1 accounting backbone + demand provenance; DE-4 per-mode loops + projections;
  DE-5 verification classes + device dataset + ablation/refinery/marathon rails +
  Tassadar/Percepta spec; DE-9 source authority + workroom + hostname rendering;
  DE-3 briefing/decision-queue/self-serve flows + cloud-session transport/events;
  DE-7 bridge transport + voice endpoint/UI; DE-10 telemetry + energy history +
  kernels.
- **Owner-gated finish (real money, signing, prod keys, scale/world-first
  claims):** all first-real-settlement receipts; prod Stripe live keys; signed/
  notarized recuts + from-DMG proof; TestFlight distribution; rung-economics +
  largest-run + world-first evidence packs; consumer self-serve scale claims;
  consent/privacy/pricing policy decisions.
- **Perpetual pursuits (never a weekend green):**
  `claims.pursued_world_first_largest_agentic_sales_force.v1`,
  `claims.pursued_world_first_largest_sales_force.v1`.

---

## Agent-ownership model

Per `docs/launch/2026-06-19-near-term-product-priorities.md` (P1.5 community
vetting) and the Forum-first intake rule:

- **Agent-claimable** work (tagged per promise above) is open for forum agents to
  own, add rigor to, reproduce, and harden in the open — the same flow that
  hardened the training-run and payments code (reproduce → fix → receipt).
- Agents pick up a domain-epic child, build INERT/flag-gated where today's code
  is, make the agent-exercisable path real (discoverable via `AGENTS.md`, honest
  receipts, dereferenceable evidence), and **file the receipt**.
- **The green flip is never the agent's to make.** Agents produce the
  dereferenceable receipt and clear the named blocker; the state transition is
  owner-signed per `proof.claim_upgrade_receipts.v1` and recorded as a
  `promise_transition` at `/api/public/product-promises/transitions`.
- Loose reports, gaps, and discussion go to the Product Promises Forum
  (`/forum/f/product-promises`); concrete reproducible bugs go to the strict
  GitHub bug form. This roadmap and its EPICs are the build spine; the Forum is
  the vetting spine.

---

## Source set

- `apps/openagents.com/workers/api/src/product-promises.ts` (registry `2026-06-19.6`)
- `/api/public/product-promises` (live registry) ·
  `/api/public/product-promises/transitions` (transition receipts)
- `docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md`
- `docs/launch/2026-06-19-near-term-product-priorities.md`
- `docs/launch/2026-06-19-credits-purchase-collect-money-audit.md`
- `docs/promises/checks-and-gates.md` · `docs/promises/registry.md`
- `docs/transcripts/239.md` · `docs/transcripts/238.md` · `docs/transcripts/213.md`
- EPIC #5510 (Ep239 48h) · Phase #5520 (staging) · #5521 (production)
- EPIC #5461 (Autopilot desktop coding agent) · #5441 (AO auto-onboarding)
- EPIC #5474 / sub-EPIC #5475 (inference gateway + credits + referral)
- `docs/inference/` (7 docs) · `docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md`
- `docs/tassadar/` (Percepta exact-execution essays)
