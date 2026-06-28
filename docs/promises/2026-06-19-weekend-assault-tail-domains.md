# Weekend Promise Assault — TAIL domains (assault-tail)

Date: 2026-06-19
Branch: `assault-tail`
Scope: the non-green TAIL promises in `proof.*`, `identity.*`, `mobile.*`,
`claims.*`, `models.*`, `workrooms.*`, `agents.*`, `provider.*`, `api.*`,
`metrics.*`, `sites.*` in
`apps/openagents.com/workers/api/src/product-promises.ts`.

Hard rule honored: **no green flips.** Green requires a dereferenceable receipt
plus owner sign-off. This pass builds capability and assembles/verifies
dereferenceable evidence only. The honest green count stays at 20.

## What shipped this pass (real capability)

`proof.demand_provenance.v1` — first revenue-bearing projection now carries a
typed internal/external demand split.

- The AO/kWh metric (`metrics.accepted_outcomes_per_kwh.v1`,
  `GET /api/public/metrics/accepted-outcomes-per-kwh`) now labels every
  datapoint `internal` or `external` and serves a reconciling projection-level
  split under the rule **`no_external_dollar_no_demand_claim`**:
  - per-datapoint `demandProvenance { kind, rationale, evidenceRefs }`
  - projection `demandProvenance { internalAcceptedOutcomeCount,
    externalAcceptedOutcomeCount, externalDemandClaimAllowed, rule, caveatRefs,
    contractRef }`
  - `externalDemandClaimAllowed` is `false` until a real external (dollar)
    accepted outcome exists.
  - The seed (`#4777`) is labeled **internal** (operator-staged, credit-ledger
    1 sat — not external market demand), per
    `provider.compliant_usage_labor.v1`.
  - Copy gate added to the metric `unsafeCopy`: internal demand may not be
    presented as external market demand.
- Files:
  `apps/openagents.com/workers/api/src/accepted-outcomes-per-kwh.ts`,
  `apps/openagents.com/workers/api/src/accepted-outcomes-per-kwh.test.ts`
  (new test `labels demand provenance internal/external and forbids unlabeled
  market-demand claims`),
  `apps/openagents.com/workers/api/src/openagents-openapi.ts` (description),
  `docs/metrics/2026-06-15-accepted-outcomes-per-kwh.md` (section 5b).
- Registry updated (no state change): `proof.demand_provenance.v1` stays
  **planned** with accurate safeCopy/verification noting the first serving
  surface; `metrics.accepted_outcomes_per_kwh.v1` stays **yellow** with the
  demand-provenance evidence ref. Registry version bumped to `2026-06-19.7`.

`proof.demand_provenance.v1` green still requires the *remaining*
revenue-bearing projections (stats, leaderboards, run pages, economics gates) to
carry the same typed split, plus a transition receipt — owner-gated.

## Per-promise state map and green path

### YELLOW (last-mile / owner-gated live receipt) — evidence re-verified this pass

| id | green-ready blocker (receipt needed) | owner-gated |
| --- | --- | --- |
| `proof.claim_upgrade_receipts.v1` | enterprise audit panel surface | yes (build + sign-off) |
| `identity.orange_check_forum_signal.v1` | one live $5 production purchase smoke with badge on the buying agent | yes (live smoke) |
| `agents.x_claim_reward.v1` | one live operator-dispatched reward settled to a real BOLT12 receive code | yes (live dispatch + sign-off) |
| `agents.nostr_fallback_coordination.v1` | end-to-end outage coordination drill (OA HTTP down → NIP-38/02/65/17/29/90 → reconcile) with public-safe evidence | yes (live drill) |
| `api.hosted_gemini.v1` | registered-agent production smoke + executor binding, billing, entitlement, quota, metering, settlement refs | yes (production smoke) |
| `provider.compliant_usage_labor.v1` | same compliant flow settling external sats over the reliable-tips ladder, self-serve (not operator-staged) | yes (external settlement) |
| `metrics.accepted_outcomes_per_kwh.v1` | measured/repeatable per-device energy telemetry + >1 datapoint + transition receipt | yes (telemetry) |
| `workrooms.omni_client_delivery_workrooms.v1` | source authority over connectors + AI on workroom content + approval-gated business writes (== `workrooms.source_authorized_business_objects.v1`) | yes (build) |
| `sites.referral_bitcoin_stream.v1` | a real Bitcoin-revenue production event producing a dereferenceable settled referral payout receipt | yes (real revenue event) |

Evidence test suites re-verified green this pass:
`promise-transition-receipt-routes.test.ts`,
`x-claim-reward-treasury-dispatcher.test.ts`,
`site-referral-payout-wire.test.ts`, `forum-routes.test.ts` (orange check),
`accepted-outcomes-per-kwh.test.ts`, `product-promises.test.ts`.

### RED — large product builds, flagged with owner action

| id | gap / owner action |
| --- | --- |
| `claims.world_first_ai_training_paid_bitcoin.v1` | needs the real training-paid-bitcoin world-first event + receipt; aspirational claim must not go green without it |
| `claims.world_first_public_llm_computer_training_run.v1` | needs the real public LLM-computer training run + receipt |
| `mobile.voice_session_evidence_transcript_ingest.v1` | product decision: pick STT vendor + capture path, then wire ingest endpoint, AI proposal gen, approval UI |
| `models.tassadar_percepta_executor.v1` | executor model capability + benchmark/settlement evidence (substrate in `psionic`) |
| `workrooms.source_authorized_business_objects.v1` | live workroom kind with source refs, proposed updates, approvals, artifacts, closeout receipts |

### PLANNED

| id | note |
| --- | --- |
| `proof.demand_provenance.v1` | advanced this pass (first serving surface); green needs remaining projections + receipt |
| `mobile.autopilot_remote_control.v1` | Pylon remote bridge transport (#5000) → Expo read-only app to TestFlight (#5001) → write actions (#5002-5004). Build per `clients/khala-ios/AutopilotRemoteControl/TESTFLIGHT.md` — NO Expo/EAS cloud. |
| `mobile.voice_approval_companion.v1` | depends on mobile projection + voice command approval receipts + cross-device workroom sync |
| `claims.pursued_world_first_largest_agentic_sales_force.v1` | aspirational — stays planned forever |
| `claims.pursued_world_first_largest_sales_force.v1` | aspirational — stays planned forever |

`models.tasadar_percepta_executor.v1` is `withdrawn` (typo'd duplicate of
`tassadar`).
