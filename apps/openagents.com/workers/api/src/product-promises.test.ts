import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PublicProductPromisesVersion,
  publicProductPromisesAnnouncementReadiness,
  publicProductPromisesDocument,
} from './product-promises'

const ProductPromiseState = S.Literals([
  'degraded',
  'green',
  'planned',
  'red',
  'withdrawn',
  'yellow',
])

const ProductPromise = S.Struct({
  audience: S.Array(S.String),
  authorityBoundary: S.String,
  blockerRefs: S.Array(S.String),
  claim: S.String,
  evidenceRefs: S.Array(S.String),
  productArea: S.String,
  promiseId: S.String,
  reportPath: S.String,
  safeCopy: S.String,
  sourceRefs: S.Array(S.String),
  state: ProductPromiseState,
  unsafeCopy: S.String,
  verification: S.String,
})

const ProductPromiseBlockedSummary = S.Struct({
  blockerRefs: S.Array(S.String),
  promiseId: S.String,
  state: S.String,
})

const ProductPromisesDocument = S.Struct({
  canonicalDocsUrl: S.String,
  currentMonorepoStatus: S.Struct({
    caveats: S.Array(S.String),
    liveDeploymentRefs: S.Array(S.String),
    pylonV03Refs: S.Array(S.String),
    status: S.String,
    summary: S.String,
  }),
  generatedAt: S.String,
  latestGapAuditUrl: S.String,
  lastUpdated: S.String,
  maxStalenessSeconds: S.Number,
  notes: S.Array(S.String),
  promises: S.Array(ProductPromise),
  publicDocsUrl: S.String,
  reportPath: S.Struct({
    defaultForumUrl: S.String,
    forumSlug: S.String,
    forumTopicApi: S.String,
    rule: S.String,
    strictBugForm: S.String,
  }),
  registryVersion: S.String,
  schemaVersion: S.String,
  sourceRefs: S.Array(S.String),
  staleness: S.Struct({
    composition: S.Literal('live_at_read'),
    contractVersion: S.Literal('projection_staleness.v1'),
    maxStalenessSeconds: S.Number,
    rebuildsOn: S.Array(S.String),
  }),
  states: S.Record(S.String, S.String),
  verificationSummary: S.Struct({
    blockedPromiseCount: S.Int,
    evidenceRefCount: S.Int,
    promiseCount: S.Int,
    promisesWithBlockersCount: S.Int,
    topBlockedPromises: S.Array(ProductPromiseBlockedSummary),
    uniqueBlockerCount: S.Int,
    uniqueBlockers: S.Array(S.String),
  }),
  version: S.String,
})

describe('public product promises document', () => {
  test('matches the browser-facing schema', () => {
    const decoded = S.decodeUnknownSync(ProductPromisesDocument)(
      publicProductPromisesDocument(),
    )

    expect(decoded.version).toBe(PublicProductPromisesVersion)
    expect(decoded.registryVersion).toBe(decoded.version)
    expect(Date.parse(decoded.generatedAt)).not.toBeNaN()
    expect(decoded.maxStalenessSeconds).toBe(0)
    expect(decoded.staleness.maxStalenessSeconds).toBe(0)
    expect(decoded.staleness.rebuildsOn).toEqual(
      expect.arrayContaining([
        'product_promise_registry_changed',
        'product_promise_transition_receipt_recorded',
        'product_promise_announcement_preflight',
      ]),
    )
    expect(decoded.sourceRefs.length).toBeGreaterThan(0)
    expect(decoded.sourceRefs).toContain(
      'https://github.com/OpenAgentsInc/openagents',
    )
    const blockerRefs = decoded.promises.flatMap(promise => promise.blockerRefs)
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.cloud_fine_tuning_intake_unbuilt',
    )
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.cloud_fine_tuning_job_runtime_unbuilt',
    )
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.cloud_sandbox_rentable_product_unbuilt',
    )
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.cloud_sandbox_metering_billing_unbuilt',
    )
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.referral_ecosystem_wide_attribution_binding_unbuilt',
    )
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.referral_cross_category_accrual_unbuilt',
    )
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.cloud_sandbox_live_metering_billing_unwired',
    )
    expect(blockerRefs).not.toContain(
      'blocker.product_promises.cloud_primitives_unified_balance_unbuilt',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/launch/vertex-fleet/training.post_training_arc.v1.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/launch/vertex-fleet/training.public_distributed_training_run.v1.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/launch/vertex-fleet/pylon.largest_decentralized_training_claim.v1.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/launch/vertex-fleet/training.public_gradient_windows.v1.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/training/2026-06-20-cs336-a2-same-class-replication-status.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/launch/vertex-fleet/training.data_refinery_corpus.v1.md',
    )
    expect(decoded.sourceRefs).toContain(
      'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
    )
    expect(decoded.promises.length).toBeGreaterThan(0)
    expect(decoded.verificationSummary.promiseCount).toBe(
      decoded.promises.length,
    )
    // receipt. The Episode 239 records (registry 2026-06-19.6) are all
    // red/planned; the 2026-06-19.7 passes (scaffold advancement + training
    // live-run destale), the 2026-06-19.8 weekend-promise-assault pass
    // (evidence docs + an inert capability), the 2026-06-19.9 remote-bridge
    // decision-queue transport pass (a pure composing capability + evidence refs
    // on autopilot.decision_queue.v1 / mobile.autopilot_remote_control.v1), and
    // the 2026-06-19.10 passes — workrooms source-authority + approval-gated
    // business-object writes, the registry completeness audit (two new records:
    // payments.autopilot_credits_purchase.v1 red +
    // autopilot_sites.site_build_and_host.v1 yellow), and the composed-run
    // capstone (a flag-gated INERT execution composition wiring the real metering
    // + referral seams) — and the 2026-06-19.11 pass — the agentic labor-product
    // flow scaffold (a typed post->order->dispatch->deliver->settle flow with a
    // flag-gated INERT, owner-gated settlement seam) advancing
    // autopilot.agentic_labor_products.v1 (stays yellow) — and the 2026-06-19.12
    // pass — the RL-1 Sites referral payout public projection
    // (GET /api/public/site-referral-payouts) turning the referral ledger from
    // "wired in source" into "wired + dereferenceable" on
    // referral.refer_once_earn_forever.v1 (stays red) and
    // autopilot_sites.partner_payout_ledger.v1 (stays red) — flip nothing. The
    // 2026-06-19.13 pass is the FIRST green flip of the assault:
    // agents.nostr_fallback_coordination.v1 yellow -> green (owner-authorized
    // 2026-06-19, outage-coordination drill PR #5535), so green is now exactly
    // 21. The 2026-06-20.1 pass is a Spark-first Forum wallet copy/default
    // update with no state flips, so the count remained 21. The 2026-06-20.2
    // pass flips training.verification_classes.v1 yellow -> green
    // (owner-authorized #4674 per-contribution sampling decision), so green is
    // now exactly 22. The 2026-06-20.3 pass flips
    // pylon.v03_release_candidate.v1 + pylon.release_tomorrow.v1 green (Pylon
    // v1.0.5 signed release shipped + verified, owner-authorized), so green is
    // now exactly 24. The 2026-06-20.4 Pylon green-quality pass and
    // 2026-06-20.5 signature-metering de-stale pass and 2026-06-20.6
    // partner-payout projection de-stale pass flip no promise state, so green
    // remains exactly 24. The 2026-06-20.7 pass clears the Nostr-export blocker
    // on identity.orange_check_forum_signal.v1 (a real dereferenceable kind-1
    // attestation on wss://relay.openagents.com); the promise stays yellow and
    // green remains exactly 24. The 2026-06-20.8 workrooms live integration
    // pass and 2026-06-20.9 mobile approval projection honesty pass move
    // mobile.voice_approval_companion.v1 planned -> yellow without flipping
    // green, so green remains exactly 24. The 2026-06-20.10 pass ships the
    // enterprise claim-upgrade audit panel for proof.claim_upgrade_receipts.v1
    // (GET /api/public/product-promises/audit) and drops that promise's
    // enterprise_audit_panel_missing blocker, but the promise STAYS yellow
    // (green flip is owner-gated), so green remains exactly 24. The
    // 2026-06-20.11 ablation derisking ledger pass flips no promise state, so
    // green remains exactly 24. The 2026-06-20.12 pass builds the Artanis
    // unattended tick-streak counter/projection for
    // artanis.tassadar_evolution_loop.v1 (the missing piece of the
    // artanis_unattended_tick_streak_missing blocker) and exposes it at
    // GET /api/public/artanis/tick-streak; the blocker STAYS (no real 10-tick
    // streak has been driven live), the promise stays yellow, and green remains
    // exactly 24. The 2026-06-20.13 device-capability second-device-class pass
    // drops second_device_class_missing on
    // training.device_capability_dataset.v1 (the dataset gains a genuine
    // measured_unsettled x86_64-Linux/Intel class); the promise STAYS yellow,
    // so green remains exactly 24. The 2026-06-20.14 demand-provenance
    // projection
    // pass moves proof.demand_provenance.v1 planned -> yellow without flipping
    // green, so green remains exactly 24. The 2026-06-20.15 control-center
    // self-serve-fanout pass clears the self_serve_fanout_missing blocker on
    // autopilot.control_center_fanout_marketplace.v1 (customer-initiated
    // single-action fanout planner/route + INERT public projection); the
    // promise STAYS yellow, so green remains exactly 24. The 2026-06-20.16
    // agentic-labor self-serve pass clears not_all_labor_flows_self_serve on
    // autopilot.agentic_labor_products.v1 (deployed self-serve POST order path)
    // without flipping the promise (stays yellow), so green remains exactly 24.
    // The 2026-06-20.18 repo-study customer-private-validation pass drops
    // repo_studying_customer_private_validation_missing on
    // autopilot.repo_study_packets.v1 (refs-only INERT private-holdout
    // validation module + delivery seam); the promise STAYS yellow, so green
    // remains exactly 24. The 2026-06-20.18 two-record artanis-area pass drops
    // artanis_unattended_tick_streak_missing on
    // artanis.tassadar_evolution_loop.v1 (the deployed tick-streak gate is met:
    // longestStreak 12 >= 10, dereferenceable closeout receipts) and adds the
    // external-contributor responder-support projection
    // (GET /api/public/artanis/responder-support) on
    // artanis.pylon_support_responder.v1 while KEEPING its
    // external_contributor_flow_unproven blocker; both promises STAY yellow, so
    // green remains exactly 24. The 2026-06-20.19 custom-hostname self-serve
    // pass clears hostname_customer_self_serve_missing on
    // autopilot_sites.custom_tenant_hostnames.v1 (deployed customer-gated
    // claim/list path at /api/tenant/hostnames that writes only pending rows;
    // live provisioning stays owner-gated and INERT default-OFF), without
    // flipping the promise (stays yellow), so green remains exactly 24.
    // The 2026-06-20.20 training ablation one-delta harness pass clears the
    // ablation_harness_missing blocker on training.ablation_system.v1 while
    // eval reproduction and paid dispatch remain blocked, so green remains
    // exactly 24. The 2026-06-20.21 external-repo-studying pilot
    // customer-private-admission pass drops
    // external_repo_studying_customer_private_admission_missing on
    // autopilot.external_repo_studying_pilot.v1 (refs-only INERT admission
    // module that REUSES the customer-private validation engine and decides
    // whether an external contributor's study may be admitted for a customer,
    // flag-gated default-OFF, admitted/effectsApplied always false); the
    // promise STAYS yellow, so green remains exactly 24.
    // The 2026-06-20.22 Artanis distillation-dataset receipt pass clears
    // tassadar_distillation_dataset_receipt_missing on
    // artanis.tassadar_evolution_loop.v1, but the promise stays yellow pending
    // owner-signed green transition, so green remains exactly 24.
    // The 2026-06-20.23 model-ladder cleanup drops the already-documented
    // rung_economics_gate_format_missing blocker while leaving the R1 rehearsal
    // blocker in place, so green remains exactly 24.
    // The 2026-06-20.24 largest-run cleanup drops the already-documented
    // participant-methodology and comparable-run blockers while leaving the
    // comparable-scale public contributor receipts blocker in place, so green
    // remains exactly 24.
    // The 2026-06-20.25 Tassadar Percepta executor spec pass clears only the
    // model-spec blocker while architecture and CPU-transform training receipts
    // stay blocked, so green remains exactly 24.
    // The 2026-06-20.26 device-capability thermal-classifier pass exposes
    // thermalThrottleSignals/status/blockers from sustained-vs-burst rows but
    // keeps thermal_throttle_detection_missing because no live verified thermal
    // probe row exists, so green remains exactly 24.
    // The 2026-06-20.27 ablation eval-reproduction pass clears only
    // eval_suite_reproduction_missing by projecting the retained Psion
    // checkpoint-eval decision; paid ablation dispatch remains blocked, so
    // green remains exactly 24.
    // The 2026-06-20.28 Tassadar Percepta architecture-receipt pass clears
    // only percepta_executor_architecture_receipts_missing by projecting a
    // refs-only architecture bundle; CPU-transform training receipts remain
    // blocked, so green remains exactly 24.
    // The 2026-06-20.29 post-training instruct-SFT lane receipt pass clears
    // only instruct_sft_lane_missing by replacing it with narrower fixture-sync
    // and paid SFT dispatch blockers; paid dispatch, preference rollout, and
    // vibe-test gates remain blocked, so green remains exactly 24.
    // The 2026-06-20.30 autopilot.decision_queue.v1 receipt-closeout pass
    // adds DecisionCloseoutReceipt (evidence-only, no promise state change);
    // green count stays exactly 24.
    // The 2026-06-20.31 fixture-sync receipt pass clears only the fixture-sync
    // blocker; paid dispatch, preference rollout, and vibe-test gates remain
    // blocked, so green remains exactly 24.
    // The 2026-06-20.32 full-pipeline program pass adds a public stage-status
    // projection but keeps training_pipeline_rails_incomplete active, so green
    // remains exactly 24.
    // The 2026-06-20.33 marathon pass binds durable-checkpoint seal evaluation
    // into live seal/bootstrap authority but keeps real remote checkpoint,
    // standby, and curtailment blockers active, so green remains exactly 24.
    // The 2026-06-20.34 device-capability pass exposes same-class replication
    // status but keeps same-host and thermal blockers active, so green remains
    // exactly 24.
    // The 2026-06-20.35 data-refinery pass wires corpus-provenance receipts
    // into A4 admission/projection but no live paid shard closeout exists, so
    // green remains exactly 24.
    // The 2026-06-20.36 marathon pass wires standby-dispatch preflight but no
    // receipt-backed live standby promotion exists, so green remains exactly 24.
    // The 2026-06-20.42 public distributed-run scale pass adds a public status
    // projection but keeps the network-scale receipt blocker active.
    // The 2026-06-20.43 largest-run claim pass adds a public benchmark
    // projection but keeps the contributor-receipts blocker active.
    // The 2026-06-20.44 vibe-test rubric pass adds a public deterministic
    // closeout projection but keeps the reviewed-artifact blocker active.
    // The 2026-06-20.45 marathon curtailment pass exposes the drill predicate
    // but keeps the drill-receipt blocker active.
    // The 2026-06-20.46 public-gradient pass exposes the intake predicate but
    // keeps the live-runtime blocker active.
    // The 2026-06-20.48 agent-MMORPG / agent-world pass adds five new records
    // (autopilot.agent_world_scene.v1, autopilot.bitcoin_payment_visualization.v1,
    // autopilot.pylon_growth_visualization.v1 yellow; autopilot.agent_character_creation.v1,
    // world.multiplayer_agent_world.v1 planned) and flips NO promise state, so
    // green remains exactly 26.
    // The 2026-06-20.49 owner-directed revenue-loop tightening postpones 7
    // marketplace + advanced/research-training promises (yellow/red -> planned)
    // and flips NO green promise, so green remains exactly 26 (yellow 28, red 15,
    // planned 34, withdrawn 2, total 105).
    // The 2026-06-20.50 business-fulfillment coverage pass adds SEVEN new
    // records to fully cover the OpenAgents Business offering menu
    // (business.intake_quick_win_offering.v1, business.coding_quick_win.v1,
    // inference.free_tier_taste.v1, business.ecommerce_workspace_pack.v1,
    // business.legal_workspace_pack.v1, business.marketing_agency_workspace_pack.v1
    // yellow; inference.batch_processing_jobs.v1 planned) and flips NO promise
    // state, so green remains exactly 26 (yellow 28 -> 34, planned 34 -> 35,
    // red 15, withdrawn 2, total 105 -> 112).
    // The 2026-06-20.54 open-markets de-stale pass clears only the stale
    // unified-surface blocker on markets.open_protocol_markets.v1 because
    // GET /api/public/markets/open-markets and the inert liquidity/risk
    // skeleton routes already exist; liquidity/risk and broad compute/data
    // blockers remain, so green remains exactly 26.
    // The 2026-06-20.55 compose-and-list de-stale pass clears only the stale
    // broad listing/discovery blocker by acknowledging the inert
    // /api/public/marketplace/composed-products read surface. Runtime,
    // self-serve write/install lifecycle, and billing blockers remain, so
    // green remains exactly 26.
    // The agentic-npm runtime pass clears the stale source-level registry and
    // install/use blocker by acknowledging the bounded runtime core plus
    // install/use evidence rows. Paid public marketplace and billing/settlement
    // blockers remain, so green remains exactly 31.
    // The 2026-06-21.3 demand-provenance broad-coverage pass flips
    // proof.demand_provenance.v1 yellow -> green: GET /api/public/demand-provenance
    // now carries the typed internal/external/unlabeled split + reconciliation
    // for ALL revenue-bearing surfaces (AO/kWh, pylon-stats, training
    // leaderboards, training run pages, rung economics gates), clearing
    // blocker.product_promises.demand_provenance_broad_projection_coverage_missing.
    // externalDemandClaimAllowed STAYS false (no external dollar, no demand
    // claim); this is a transparency/coverage flip only. Green is now exactly 27.
    // The 2026-06-21.4 Tassadar CPU-transform receipt-status pass adds a
    // public missing-gate projection without clearing the remaining
    // CPU-transform blocker, so green remains exactly 27.
    // The 2026-06-27.1 Khala reconciliation adds three scoped green records for
    // the free OpenAI-compatible Khala API, the public tokens-served counter,
    // and the shipped Khala terminal client. The broader own-capacity routing,
    // model-mix, trace-capture, and paid-privacy records stay yellow.
    // The 2026-06-27.2 closeout-policy pass flips the scoped
    // khala.own_capacity_codex_delegation.v1 promise green after live dispatch,
    // materialization, trace-status, proof, and no-spend closeout evidence.
    // Green is now exactly 31.
    expect(
      decoded.promises.filter(promise => promise.state === 'green').length,
    ).toBe(31)
    expect(decoded.verificationSummary.evidenceRefCount).toBeGreaterThan(0)
    expect(decoded.verificationSummary.uniqueBlockerCount).toBeGreaterThan(0)
    expect(
      decoded.verificationSummary.topBlockedPromises.length,
    ).toBeGreaterThan(0)
    expect(
      decoded.promises.every(promise => promise.sourceRefs.length > 0),
    ).toBe(true)
    const openMarketsPromise = decoded.promises.find(
      promise => promise.promiseId === 'markets.open_protocol_markets.v1',
    )
    expect(openMarketsPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.open_markets_unified_surface_missing',
    )
    expect(openMarketsPromise?.safeCopy).toContain(
      'GET /api/public/markets/open-markets',
    )
    expect(openMarketsPromise?.verification).toContain(
      'green still requires real market transactions plus settlement receipts for all six markets',
    )
    const composeAndListPromise = decoded.promises.find(
      promise =>
        promise.promiseId === 'marketplace.compose_and_list_products.v1',
    )
    expect(composeAndListPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.marketplace_listing_lifecycle_unbuilt',
    )
    expect(composeAndListPromise?.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.product_promises.marketplace_composition_runtime_unbuilt',
        'blocker.product_promises.marketplace_self_serve_listing_write_install_lifecycle_unbuilt',
        'blocker.product_promises.marketplace_billing_settlement_missing',
      ]),
    )
    expect(composeAndListPromise?.evidenceRefs).toContain(
      'route:/api/public/marketplace/composed-products',
    )
    expect(composeAndListPromise?.safeCopy).toContain(
      'public read-only listing/discovery projection',
    )
    const agenticNpmPromise = decoded.promises.find(
      promise =>
        promise.promiseId === 'marketplace.agentic_npm_module_registry.v1',
    )
    expect(agenticNpmPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.agentic_npm_module_composition_runtime_missing',
    )
    expect(agenticNpmPromise?.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.product_promises.agentic_npm_billing_settlement_missing',
        'blocker.product_promises.agentic_npm_paid_public_marketplace_missing',
      ]),
    )
    expect(agenticNpmPromise?.evidenceRefs).toEqual(
      expect.arrayContaining([
        'apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.ts',
        'apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.test.ts',
      ]),
    )
    expect(agenticNpmPromise?.safeCopy).toContain(
      'bounded source-level registry + install/use runtime core exists',
    )
    const currentCopy = [
      decoded.currentMonorepoStatus.summary,
      ...decoded.currentMonorepoStatus.caveats,
      ...decoded.notes,
    ].join('\n')
    expect(currentCopy).not.toMatch(
      /latest stays 0\.2\.5|only published, installable Pylon|release candidate, not stable 0\.3\.0|Pylon v1\.0 is present in the monorepo as a release candidate/i,
    )
    expect(currentCopy).toContain('Pylon v1.0 has a stable source cut')
    expect(decoded.promises).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          promiseId: 'autopilot.mission_briefing.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          promiseId: 'autopilot.decision_queue.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'workrooms.source_authorized_business_objects.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'mobile.voice_approval_companion.v1',
          state: 'yellow',
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/workers/api/src/mobile-workroom-approval-projection-routes.ts',
            'route:/api/mobile/workroom-approval-projection',
          ]),
          blockerRefs: expect.not.arrayContaining([
            'blocker.product_promises.mobile_projection_missing',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'pylon.no_dark_capacity_accounting.v1',
          state: 'green',
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'promise_transition_cd1c3145-eccd-4985-b48a-99f8b1b20fbe',
            'route:/api/public/pylon-capacity-funnel/history',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'payments.accepted_outcome_economics.v1',
          state: 'red',
          evidenceRefs: expect.arrayContaining([
            'route:/api/public/accepted-outcome/settlement/{economicsId}',
            'apps/openagents.com/workers/api/src/omni-accepted-outcome-settlement-state-machine.ts',
            'apps/openagents.com/workers/api/src/public-accepted-outcome-settlement-routes.test.ts',
          ]),
          blockerRefs: expect.not.arrayContaining([
            'blocker.product_promises.settlement_state_machine_incomplete',
          ]),
          safeCopy: expect.stringContaining('public-safe evidence ref'),
          verification: expect.stringContaining(
            'eight distinct public-safe evidenceRef values',
          ),
        }),
        expect.objectContaining({
          promiseId: 'energy.flexible_load_proof.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'training.full_pipeline_program.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md',
            'docs/training/2026-06-20-training-full-pipeline-program-status.md',
            'route:/api/public/training/full-pipeline-program',
            'apps/openagents.com/workers/api/src/training-full-pipeline-program.ts',
            'apps/openagents.com/workers/api/src/training-full-pipeline-program.test.ts',
          ]),
          blockerRefs: [
            'blocker.product_promises.training_pipeline_rails_incomplete',
          ],
          safeCopy: expect.stringContaining(
            '/api/public/training/full-pipeline-program',
          ),
          verification: expect.stringContaining('greenGateSatisfied=false'),
        }),
        expect.objectContaining({
          promiseId: 'training.ablation_system.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/training/2026-06-20-ablation-one-delta-harness.md',
            'docs/training/2026-06-20-ablation-eval-reproduction-receipt.md',
            'https://openagents.com/api/public/training/ablation-derisking-ledger',
            'apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.ts',
            'apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.test.ts',
          ]),
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.paid_ablation_dispatch_missing',
          ]),
          safeCopy: expect.stringContaining(
            '/api/public/training/ablation-derisking-ledger',
          ),
          verification: expect.stringContaining('one-delta manifest harness'),
        }),
        expect.objectContaining({
          promiseId: 'training.data_refinery_corpus.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/launch/vertex-fleet/training.data_refinery_corpus.v1.md',
            'apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.ts',
            'apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.test.ts',
            'apps/openagents.com/workers/api/src/cs336-a4-paid-refinery-shard-dispatch.ts',
            'apps/openagents.com/workers/api/src/cs336-a4-paid-refinery-shard-dispatch.test.ts',
            'apps/openagents.com/workers/api/src/cs336-a4-provenance.ts',
            'apps/openagents.com/workers/api/src/cs336-a4-provenance.test.ts',
            'apps/openagents.com/workers/api/src/training-data-refinery.ts',
            'apps/openagents.com/workers/api/src/training-data-refinery.test.ts',
            'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
          ]),
          blockerRefs: [
            'blocker.product_promises.crawl_scale_corpus_missing',
            'blocker.product_promises.corpus_provenance_receipts_missing',
            'blocker.product_promises.eval_delta_payment_missing',
          ],
          safeCopy: expect.stringContaining('evalDeltaPaymentGate'),
          verification: expect.stringContaining(
            'paid-dispatch receipt builder',
          ),
        }),
        expect.objectContaining({
          promiseId: 'training.marathon_operations.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
            'route:/api/public/training/marathon-operations',
            'apps/openagents.com/workers/api/src/training-marathon-operations.ts',
            'apps/openagents.com/workers/api/src/training-marathon-operations.test.ts',
            'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.ts',
            'apps/openagents.com/workers/api/src/training-run-window-authority.ts',
            'apps/openagents.com/workers/api/src/training-window-bootstrap.ts',
            'apps/openagents.com/workers/api/src/training-standby-dispatch.ts',
            'apps/openagents.com/workers/api/src/training-standby-dispatch.test.ts',
            'apps/openagents.com/workers/api/src/training-curtailment-drill.ts',
            'apps/openagents.com/workers/api/src/training-curtailment-drill.test.ts',
            'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
          ]),
          blockerRefs: [
            'blocker.product_promises.durable_checkpoint_seal_missing',
            'blocker.product_promises.standby_dispatch_missing',
            'blocker.product_promises.curtailment_drill_missing',
          ],
          safeCopy: expect.stringContaining(
            '/api/public/training/marathon-operations',
          ),
          verification: expect.stringContaining(
            'curtailmentSurface.predicateAvailable=true',
          ),
        }),
        expect.objectContaining({
          promiseId: 'training.device_capability_dataset.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/training/2026-06-20-cs336-a2-same-class-replication-status.md',
            'apps/openagents.com/workers/api/src/training-device-capability.ts',
            'apps/openagents.com/workers/api/src/training-device-capability.test.ts',
            'apps/openagents.com/workers/api/src/training-run-window-routes.ts',
          ]),
          blockerRefs: [
            'blocker.product_promises.thermal_throttle_detection_missing',
            'blocker.product_promises.same_host_replication_caveat',
          ],
          safeCopy: expect.stringContaining('sameClassReplicationStatus'),
          verification: expect.stringContaining(
            'legacy settled rows fail closed to same_host_only',
          ),
        }),
        expect.objectContaining({
          promiseId: 'training.post_training_arc.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md',
            'docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md',
            'route:/api/public/training/post-training-arc/instruct-sft-lane',
            'route:/api/public/training/post-training-arc/dpo-preference-workload',
            'route:/api/public/training/post-training-arc/vibe-test-rubric',
            'apps/openagents.com/workers/api/src/training-post-training-instruct-sft.ts',
            'apps/openagents.com/workers/api/src/training-post-training-instruct-sft.test.ts',
            'apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.ts',
            'apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.test.ts',
            'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.ts',
            'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.test.ts',
            'apps/openagents.com/workers/api/src/training-post-training-paid-dispatch.ts',
            'apps/openagents.com/workers/api/src/training-post-training-paid-dispatch.test.ts',
            'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.ts',
            'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.test.ts',
            'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.ts',
            'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.test.ts',
            'https://github.com/OpenAgentsInc/psionic/blob/main/scripts/check-psion-instruct-sft-lane.sh',
            'https://github.com/OpenAgentsInc/psionic/pull/1132',
          ]),
          blockerRefs: [
            'blocker.product_promises.instruct_sft_paid_dispatch_missing',
            'blocker.product_promises.preference_rollout_work_missing',
            'blocker.product_promises.vibe_test_artifact_missing',
          ],
          safeCopy: expect.stringContaining(
            '/api/public/training/post-training-arc/dpo-preference-workload',
          ),
          verification: expect.stringContaining(
            'paidPreferenceDispatchAvailable=false',
          ),
        }),
        expect.objectContaining({
          promiseId: 'artanis.tassadar_evolution_loop.v1',
          state: 'green',
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'route:/api/public/artanis/tassadar-distillation-dataset',
            'apps/openagents.com/workers/api/src/artanis-distillation-dataset-receipt.ts',
            'apps/openagents.com/workers/api/src/artanis-distillation-dataset-receipt.test.ts',
            'docs/training/2026-06-20-artanis-distillation-dataset-receipt.md',
          ]),
          safeCopy: expect.stringContaining(
            '/api/public/artanis/tassadar-distillation-dataset',
          ),
          verification: expect.stringContaining(
            'tassadar_distillation_dataset_receipt_missing are therefore cleared',
          ),
        }),
        expect.objectContaining({
          promiseId: 'training.model_ladder.v1',
          state: 'planned',
          blockerRefs: ['blocker.product_promises.r1_full_rehearsal_missing'],
          evidenceRefs: expect.arrayContaining([
            'docs/training/2026-06-19-model-ladder-rung-economics.md',
            'route:/api/public/training/model-ladder-rungs',
            'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
            'apps/openagents.com/workers/api/src/training-model-ladder-rungs.test.ts',
          ]),
          safeCopy: expect.stringContaining(
            'GET /api/public/training/model-ladder-rungs',
          ),
          verification: expect.stringContaining(
            'rung_economics_gate_format_missing dimension is documented',
          ),
          unsafeCopy: expect.stringContaining(
            'Do not claim any Psion rung above R0 is trained',
          ),
        }),
        expect.objectContaining({
          promiseId: 'training.public_distributed_training_run.v1',
          state: 'planned',
          blockerRefs: [
            'blocker.product_promises.public_distributed_training_run_receipts_missing',
          ],
          evidenceRefs: expect.arrayContaining([
            'docs/transcripts/236.md',
            'docs/launch/vertex-fleet/training.public_distributed_training_run.v1.md',
            'route:/api/public/training/public-distributed-run-scale',
            'apps/openagents.com/workers/api/src/training-public-distributed-run-scale.ts',
            'apps/openagents.com/workers/api/src/training-public-distributed-run-scale.test.ts',
          ]),
          safeCopy: expect.stringContaining(
            'GET /api/public/training/public-distributed-run-scale',
          ),
          verification: expect.stringContaining(
            'networkScaleThresholdMet=false',
          ),
        }),
        expect.objectContaining({
          promiseId: 'training.decentralized_training_launch.v1',
          state: 'green',
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2',
            'docs/promises/2026-06-17-training-monday-simulation-settlement-policy.md',
          ]),
          safeCopy: expect.stringContaining('realBitcoinMoved:false'),
          unsafeCopy: expect.stringContaining('realBitcoinMoved:true'),
          verification: expect.stringContaining(
            'must not be counted as real Bitcoin movement',
          ),
        }),
        expect.objectContaining({
          promiseId: 'pylon.largest_decentralized_training_claim.v1',
          state: 'planned',
          blockerRefs: [
            'blocker.product_promises.public_training_contributor_receipts_missing',
          ],
          evidenceRefs: expect.arrayContaining([
            'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
            'docs/training/2026-06-19-comparable-decentralized-training-runs-research.md',
            'docs/launch/vertex-fleet/pylon.largest_decentralized_training_claim.v1.md',
            'route:/api/public/pylon/largest-decentralized-training-claim',
            'apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status.ts',
            'apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status.test.ts',
          ]),
          safeCopy: expect.stringContaining(
            'GET /api/public/pylon/largest-decentralized-training-claim',
          ),
          verification: expect.stringContaining(
            'transcriptTargetThresholdMet=false',
          ),
        }),
        expect.objectContaining({
          promiseId: 'claims.world_first_ai_training_paid_bitcoin.v1',
          state: 'red',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.world_first_evidence_pack_missing',
            'blocker.product_promises.world_first_owner_signed_upgrade_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/transcripts/238.md',
            'docs/launch/2026-06-18-world-firsts-verification.md',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'claims.world_first_public_llm_computer_training_run.v1',
          state: 'red',
          // The definition-missing blocker is cleared by
          // docs/launch/2026-06-20-llm-computer-training-run-definition.md, and
          // the evidence-pack-missing blocker is cleared by the focused
          // docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md;
          // only the owner-signed-upgrade blocker still stands.
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.world_first_owner_signed_upgrade_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'promise:compute.tassadar_executor_poc.v1',
            'docs/launch/2026-06-20-llm-computer-training-run-definition.md',
            'docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md',
            'apps/openagents.com/workers/api/src/world-first-llm-computer-evidence-pack.test.ts',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'pylon.consumer_compute_earns_bitcoin_self_serve.v1',
          state: 'red',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing',
            'blocker.product_promises.windows_wsl_consumer_install_coverage_missing',
            'blocker.product_promises.spark_helper_autostart_receipt_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements',
            'docs/launch/2026-06-19-autostream-settlement-visibility-capture.md',
            'proof_replay_bundle.public_activity.73e66071',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'marketplace.agentic_npm_module_registry.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.agentic_npm_billing_settlement_missing',
            'blocker.product_promises.agentic_npm_paid_public_marketplace_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.ts',
            'apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.test.ts',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'referral.refer_once_earn_forever.v1',
          state: 'red',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.referral_first_real_payout_pending',
            'blocker.product_promises.referral_purchase_to_payout_receipt_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/transcripts/239.md',
            'apps/openagents.com/workers/api/src/referral-cross-category-accrual.ts',
            'apps/openagents.com/workers/api/src/marketplace-monetize-any-layer-accrual.ts',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'autopilot.all_in_one_business_system.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'cloud.primitives_suite.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.cloud_fine_tuning_live_sellable_service_missing',
            'blocker.product_promises.cloud_sandbox_compute_live_sellable_service_missing',
            'blocker.product_promises.cloud_primitives_live_unified_balance_debit_receipt_missing',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'cloud.fine_tuning_service.v1',
          state: 'red',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.cloud_fine_tuning_live_intake_disabled',
            'blocker.product_promises.cloud_fine_tuning_live_pricing_missing',
            'blocker.product_promises.cloud_fine_tuning_paid_receipt_missing',
            'blocker.product_promises.cloud_fine_tuning_billing_settlement_missing',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'cloud.sandbox_compute_service.v1',
          state: 'red',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.cloud_sandbox_live_rent_surface_disabled',
            'blocker.product_promises.cloud_sandbox_live_pricing_missing',
            // The D1 fixture runtime and receipt ARTIFACT now exist; remaining
            // gates are live rent/pricing plus real renter + owner sign-off.
            'blocker.product_promises.cloud_sandbox_real_renter_demand_provenance_and_owner_signoff_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/workers/api/migrations/0257_cloud_sandbox_runtime.sql',
          ]),
          safeCopy: expect.stringContaining('bounded D1 fixture runtime'),
        }),
        expect.objectContaining({
          promiseId: 'markets.open_protocol_markets.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.liquidity_market_unbuilt',
            'blocker.product_promises.risk_market_unbuilt',
            'blocker.product_promises.compute_data_markets_not_broadly_live',
          ]),
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/workers/api/src/open-markets-surface.ts',
            'apps/openagents.com/workers/api/src/open-markets-routes.ts',
            'apps/openagents.com/workers/api/src/open-markets-skeletons.ts',
            'route:/api/public/markets/open-markets',
            'route:/api/public/markets/liquidity/skeleton',
            'route:/api/public/markets/risk/skeleton',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'marketplace.compose_and_list_products.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.marketplace_composition_runtime_unbuilt',
            'blocker.product_promises.marketplace_self_serve_listing_write_install_lifecycle_unbuilt',
            'blocker.product_promises.marketplace_billing_settlement_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/workers/api/src/marketplace-product-composition.ts',
            'apps/openagents.com/workers/api/src/marketplace-composition-routes.ts',
            'route:/api/public/marketplace/composed-products',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'marketplace.monetize_any_layer_with_referral.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId:
            'claims.pursued_world_first_largest_agentic_sales_force.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.world_first_agentic_sales_force_not_achieved',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'claims.pursued_world_first_largest_sales_force.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.world_first_largest_sales_force_not_achieved',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'models.tasadar_percepta_executor.v1',
          state: 'withdrawn',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.misspelled_promise_id_withdrawn',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'models.tassadar_percepta_executor.v1',
          state: 'planned',
          evidenceRefs: expect.arrayContaining([
            'docs/2026-06-12-episode-236-training-launch-gap-audit.md',
            'docs/tassadar/2026-06-20-tassadar-percepta-executor-model-spec.md',
            'docs/tassadar/2026-06-20-tassadar-percepta-architecture-receipt.md',
            'docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md',
            'route:/api/public/models/tassadar-percepta-executor/architecture-receipts',
            'route:/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts',
            'apps/openagents.com/workers/api/src/tassadar-percepta-architecture-receipts.ts',
            'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.ts',
            'apps/openagents.com/workers/api/src/tassadar-percepta-cpu-transform-training-receipts.test.ts',
          ]),
          blockerRefs: [
            'blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing',
          ],
          safeCopy: expect.stringContaining(
            'CPU-transform receipt status gate',
          ),
          verification: expect.stringContaining(
            'cpuTransformTrainingReceiptAvailable=false',
          ),
        }),
        expect.objectContaining({
          promiseId: 'pylon.v0_3_multi_earning_node.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'training.verification_classes.v1',
          state: 'green',
          evidenceRefs: expect.arrayContaining([
            'receipt.nexus_pylon.settlement.assignment_public_training_validator_recheck_20260611053500',
            'promise_transition_0bfce0c5-e4dd-4d19-9221-4bc9504f2055',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'proof.demand_provenance.v1',
          state: 'green',
          evidenceRefs: expect.arrayContaining([
            'route:/api/public/demand-provenance',
            'apps/openagents.com/workers/api/src/demand-provenance.ts',
            'apps/openagents.com/workers/api/src/demand-provenance-routes.ts',
            'apps/openagents.com/workers/api/src/demand-provenance.test.ts',
            'route:/api/public/pylon-stats',
            'route:/api/training/leaderboards/*',
            'route:/api/public/training/runs/{trainingRunRef}',
            'route:/api/public/training/model-ladder-rungs',
          ]),
          blockerRefs: [],
        }),
        expect.objectContaining({
          promiseId: 'proof.claim_upgrade_receipts.v1',
          state: 'green',
        }),
        expect.objectContaining({
          promiseId: 'repo.open_source_code_map.v1',
          state: 'green',
          evidenceRefs: expect.arrayContaining([
            'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/workers/api',
            'https://github.com/OpenAgentsInc/openagents/tree/main/apps/openagents.com/apps/web',
            'https://github.com/OpenAgentsInc/openagents/tree/main/apps/pylon',
            'https://github.com/OpenAgentsInc/openagents/tree/main/packages/probe',
          ]),
          authorityBoundary: expect.stringContaining(
            'does not grant write, deploy, spend',
          ),
        }),
        expect.objectContaining({
          audience: expect.arrayContaining(['agent', 'operator', 'developer']),
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.repo_studying_privacy_review_missing',
            'blocker.product_promises.repo_studying_marketplace_metering_missing',
            'blocker.product_promises.repo_studying_payout_settlement_gates_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/research/machine-studying/openagents-studybench/runs/2026-06-17-mvp-14-baseline-packet-gepa-comparison.md',
            'packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json',
            'docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md',
            'packages/probe/packages/runtime/src/benchmark/openagents-customer-private-validation.ts',
            'promise:repo.open_source_code_map.v1',
          ]),
          promiseId: 'autopilot.repo_study_packets.v1',
          safeCopy: expect.stringContaining('dogfooding'),
          state: 'yellow',
        }),
        expect.objectContaining({
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
          ]),
          promiseId: 'agents.cursor_forum_wallet.v1',
          state: 'green',
        }),
        expect.objectContaining({
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
            'route:/api/forum/paid-actions/mdk/webhooks',
            'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
            'apps/openagents.com/docs/forum-tip-wallet-onboarding-smoke.md',
            'apps/openagents.com/docs/forum-tip-payout-smoke.md',
            'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
            'apps/openagents.com/docs/forum/2026-06-11-forum-tip-webhook-refund-live-smoke-evidence.md',
            'transition:promise_transition_c106102b-e51b-4d2f-84ed-a588f1a26316',
            'transition:promise_transition_feab90da-aead-49e1-9097-bd0b8bb5c11a',
            'transition:promise_transition_e632649a-acfa-4e69-ad4b-269e92c963b3',
            'transition:promise_transition_0cfba5d7-40ff-48bd-81a3-4b0758b0acd8',
          ]),
          promiseId: 'forum.content_tipping.v1',
          state: 'green',
        }),
        expect.objectContaining({
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.mdk_agent_wallet_send_readiness_insufficient_capacity',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/forum/2026-06-09-forum-mdk-webhook-reconciliation-audit.md',
            'route:/api/forum/paid-actions/mdk/webhooks',
            'script:apps/openagents.com/scripts/forum.mjs tip-post-smoke',
            'apps/openagents.com/docs/forum-tip-payout-smoke.md',
            'apps/openagents.com/docs/mdk-forum-readiness-smoke.md',
          ]),
          promiseId: 'payments.money_dev_kit.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'apps/pylon/docs/2026-06-15-spark-backup-receive-runbook.md',
            'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-734d9003-e177-457e-8e33-757deda644ae',
          ]),
          promiseId: 'payments.offline_receive_spark_fallback.v1',
          state: 'green',
          unsafeCopy: expect.stringContaining(
            'do not call the Spark backup balance a unified MDK spendable balance',
          ),
        }),
        expect.objectContaining({
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/docs/2026-06-10-compliant-usage-labor-policy.md',
          ]),
          promiseId: 'provider.compliant_usage_labor.v1',
          state: 'yellow',
        }),
        expect.objectContaining({
          blockerRefs: [],
          evidenceRefs: expect.arrayContaining([
            'apps/pylon/docs/live-worker-loop-smoke.md',
          ]),
          promiseId: 'pylon.cli_tui_probe_background.v1',
          state: 'green',
        }),
        expect.objectContaining({
          audience: expect.arrayContaining(['agent', 'contributor', 'public']),
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.wasm_plugin_self_serve_marketplace_not_live',
          ]),
          evidenceRefs: expect.arrayContaining([
            'apps/openagents.com/workers/api/src/wasm-plugin-marketplace.ts',
            'apps/openagents.com/workers/api/src/wasm-plugin-marketplace-routes.ts',
            'apps/openagents.com/workers/api/src/wasm-plugin-marketplace.test.ts',
            'route:/api/public/marketplace/wasm-plugins',
          ]),
          promiseId: 'marketplace.wasm_plugins.v1',
          safeCopy: expect.stringContaining('install/uninstall registry'),
          state: 'planned',
        }),
        expect.objectContaining({
          audience: expect.arrayContaining(['contributor', 'operator', 'public']),
          evidenceRefs: expect.arrayContaining([
            'route:/api/public/training/public-gradient-windows',
            'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts',
            'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.test.ts',
            'apps/openagents.com/workers/api/src/training-public-gradient-windows.ts',
            'apps/openagents.com/workers/api/src/training-public-gradient-windows.test.ts',
          ]),
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.public_gradient_live_window_runtime_missing',
            'blocker.product_promises.public_gradient_promoted_window_receipts_missing',
            'blocker.product_promises.public_gradient_settlement_receipts_missing',
          ]),
          promiseId: 'training.public_gradient_windows.v1',
          state: 'planned',
          safeCopy: expect.stringContaining(
            '/api/public/training/public-gradient-windows',
          ),
          verification: expect.stringContaining(
            'intakeSurface.quarantineRouteAvailable=false',
          ),
        }),
        expect.objectContaining({
          audience: expect.arrayContaining(['agent', 'contributor', 'operator']),
          blockerRefs: [],
          promiseId: 'pylon.agent_steerable_cli.v1',
          state: 'green',
        }),
        expect.objectContaining({
          audience: expect.arrayContaining(['user', 'agent', 'operator']),
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.local_apple_fm_signed_installer_recut_missing',
            'blocker.product_promises.local_apple_fm_helper_supervision_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/apple-fm/2026-06-15-current-apple-fm-electrobun-desktop-audit.md',
            'docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-runbook.md',
            'docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-smoke-evidence.md',
            'apps/pylon/packages/runtime/src/backends/apple-fm/client.ts',
            'apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift',
            'apps/pylon/src/node/apple-fm-bridge-helper.ts',
            'apps/pylon/src/node/apple-fm-local-session.ts',
            'apps/pylon/src/node/apple-fm-status.ts',
            'apps/pylon/tests/apple-fm-bridge-helper.test.ts',
            'apps/pylon/tests/apple-fm-control-session.test.ts',
            'apps/pylon/tests/control-protocol.test.ts',
            'apps/autopilot-desktop/src/bun/pylon-control.ts',
            'apps/autopilot-desktop/src/shared/rpc.ts',
            'apps/autopilot-desktop/src/shared/install-readiness.ts',
            'apps/autopilot-desktop/src/ui/view.ts',
            'apps/autopilot-desktop/tests/apple-fm-loopback-integration.test.ts',
            'apps/autopilot-desktop/tests/cl-53-sanitize.test.ts',
          ]),
          promiseId: 'autopilot.local_apple_fm_tool_chat.v1',
          state: 'yellow',
        }),
      ]),
    )
    const mobileApprovalPromise = decoded.promises.find(
      promise => promise.promiseId === 'mobile.voice_approval_companion.v1',
    )
    expect(mobileApprovalPromise?.blockerRefs).toEqual([
      'blocker.product_promises.voice_command_approval_receipts_missing',
      'blocker.product_promises.cross_device_workroom_sync_missing',
    ])
    expect(mobileApprovalPromise?.safeCopy).toContain(
      'Voice/mobile approval is partially wired',
    )
    expect(mobileApprovalPromise?.verification).toContain(
      'Yellow is supported by the live read-only mobile workroom approval projection route',
    )
    const localAppleFmPromise = decoded.promises.find(
      promise => promise.promiseId === 'autopilot.local_apple_fm_tool_chat.v1',
    )
    expect(localAppleFmPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.local_apple_fm_pylon_control_projection_missing',
    )
    expect(localAppleFmPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.local_apple_fm_bridge_helper_missing',
    )
    expect(localAppleFmPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.local_apple_fm_desktop_readiness_ui_missing',
    )
    expect(localAppleFmPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.local_apple_fm_chat_tool_session_missing',
    )
    expect(localAppleFmPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.local_apple_fm_admitted_mac_smoke_missing',
    )
    const mondayTrainingPromise = decoded.promises.find(
      promise =>
        promise.promiseId ===
        'training.decentralized_training_launch.v1',
    )
    expect(mondayTrainingPromise).toMatchObject({
      blockerRefs: [],
      state: 'green',
    })
    expect(mondayTrainingPromise?.claim).not.toMatch(/earn Bitcoin|paid/i)
    expect(mondayTrainingPromise?.safeCopy).toContain(
      'not real Bitcoin movement',
    )
    expect(mondayTrainingPromise?.authorityBoundary).toContain(
      'simulation-backed settlement record',
    )
    const pylonInstallPromise = decoded.promises.find(
      promise => promise.promiseId === 'pylon.install_without_wallet_knowledge.v1',
    )
    expect(pylonInstallPromise).toMatchObject({
      blockerRefs: [],
      state: 'green',
    })
    expect(pylonInstallPromise?.safeCopy).toContain('realBitcoinMoved:false')
    expect(pylonInstallPromise?.unsafeCopy).toContain('realBitcoinMoved:true')
    expect(pylonInstallPromise?.evidenceRefs).toContain(
      'docs/promises/2026-06-17-training-monday-simulation-settlement-policy.md',
    )
    const repoStudyPacketPromise = decoded.promises.find(
      promise => promise.promiseId === 'autopilot.repo_study_packets.v1',
    )
    expect(repoStudyPacketPromise).toMatchObject({
      state: 'yellow',
    })
    expect(
      `${repoStudyPacketPromise?.claim} ${repoStudyPacketPromise?.safeCopy}`,
    ).not.toMatch(
      /trained repo expert|customer repo studying is live|marketplace package|payout eligible/i,
    )
    expect(repoStudyPacketPromise?.unsafeCopy).toContain('trained repo expert')
    expect(repoStudyPacketPromise?.unsafeCopy).toContain(
      'customer repo studying is live',
    )
    expect(repoStudyPacketPromise?.unsafeCopy).toContain('marketplace package')
    expect(repoStudyPacketPromise?.unsafeCopy).toContain('payout eligibility')
    expect(repoStudyPacketPromise?.authorityBoundary).toContain(
      'not paid work',
    )
    const externalRepoStudyPromise = decoded.promises.find(
      promise => promise.promiseId === 'autopilot.external_repo_studying_pilot.v1',
    )
    expect(externalRepoStudyPromise).toMatchObject({
      state: 'yellow',
    })
    expect(
      `${externalRepoStudyPromise?.claim} ${externalRepoStudyPromise?.safeCopy}`,
    ).not.toMatch(
      /customer repo studying is live|private customer repo can be uploaded|trained repo expert|marketplace package|payout eligible/i,
    )
    expect(externalRepoStudyPromise?.unsafeCopy).toContain(
      'customer repo studying is live',
    )
    expect(externalRepoStudyPromise?.unsafeCopy).toContain(
      'private customer repo can be uploaded',
    )
    expect(externalRepoStudyPromise?.unsafeCopy).toContain(
      'trained repo expert',
    )
    expect(externalRepoStudyPromise?.authorityBoundary).toContain(
      'no private repo ingestion authority',
    )
  })

  test('weekend pylon promise assault attaches evidence without flipping any state', () => {
    const document = publicProductPromisesDocument()
    const byId = new Map(
      document.promises.map(promise => [promise.promiseId, promise]),
    )

    // The ten non-green pylon.* promises keep their non-green states: this
    // pass assembled evidence + an inert capability only, never a green flip.
    const nonGreenPylonStates: Record<string, string> = {
      'pylon.v03_release_candidate.v1': 'green',
      'pylon.release_tomorrow.v1': 'green',
      'pylon.first_real_model_training_run.v1': 'planned',
      'pylon.largest_decentralized_training_claim.v1': 'planned',
      'pylon.consumer_compute_earns_bitcoin_self_serve.v1': 'red',
      'pylon.v0_3_multi_earning_node.v1': 'red',
      'pylon.five_bitcoin_revenue_streams.v1': 'planned',
      'pylon.compute_revenue_modes.v1': 'planned',
      'pylon.data_trace_revenue.v1': 'planned',
      'pylon.gepa_worker_loop_v03.v1': 'planned',
    }
    for (const [promiseId, expectedState] of Object.entries(
      nonGreenPylonStates,
    )) {
      const promise = byId.get(promiseId)
      expect(promise, `missing promise ${promiseId}`).toBeDefined()
      expect(promise?.state).toBe(expectedState)
      // Each non-green pylon promise now has the assault assessment as a
      // dereferenceable evidence home.
      expect(promise?.evidenceRefs).toContain(
        'docs/promises/2026-06-19-pylon-non-green-promise-assault-assessment.md',
      )
    }

    expect(
      byId.get('pylon.first_real_model_training_run.v1'),
    ).toMatchObject({
      blockerRefs: [
        'blocker.product_promises.model_ladder_network_rungs_not_run',
      ],
      evidenceRefs: expect.arrayContaining([
        'route:/api/public/training/model-ladder-rungs',
        'apps/openagents.com/workers/api/src/training-model-ladder-rungs.ts',
      ]),
      safeCopy: expect.stringContaining(
        'r2NetworkRungReceiptAvailable=false',
      ),
      verification: expect.stringContaining(
        'model_ladder_network_rungs_not_run',
      ),
    })

    // The participant/scale methodology + comparable-runs research are attached
    // to the largest-run and consumer-self-serve promises.
    expect(
      byId.get('pylon.largest_decentralized_training_claim.v1')?.evidenceRefs,
    ).toEqual(
      expect.arrayContaining([
        'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
        'docs/training/2026-06-19-comparable-decentralized-training-runs-research.md',
        'route:/api/public/pylon/largest-decentralized-training-claim',
        'apps/openagents.com/workers/api/src/pylon-largest-decentralized-training-claim-status.ts',
      ]),
    )
    expect(
      byId.get('pylon.consumer_compute_earns_bitcoin_self_serve.v1')
        ?.evidenceRefs,
    ).toEqual(
      expect.arrayContaining([
        'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
        'apps/pylon/src/spark-helper-autostart.ts',
      ]),
    )
  })

  test('signature monetization records metering evidence while keeping settlement blocked', () => {
    const document = publicProductPromisesDocument()
    const signaturePromise = document.promises.find(
      promise => promise.promiseId === 'marketplace.signature_monetization.v1',
    )

    expect(signaturePromise).toMatchObject({
      state: 'planned',
      blockerRefs: ['blocker.product_promises.signature_settlement_missing'],
      evidenceRefs: expect.arrayContaining([
        'apps/openagents.com/workers/api/src/signature-marketplace-revenue-gate.ts',
        'apps/openagents.com/workers/api/src/signature-marketplace-revenue-gate.test.ts',
        'apps/openagents.com/workers/api/src/signature-usage-metering.ts',
        'apps/openagents.com/workers/api/src/signature-usage-metering-routes.ts',
        'route:/api/public/markets/signature-monetization/metering',
      ]),
    })
    expect(signaturePromise?.blockerRefs).not.toContain(
      'blocker.product_promises.signature_usage_metering_missing',
    )
    expect(signaturePromise?.safeCopy).toContain(
      'tested package activation/pricing/rev-share/settlement-receipt gate logic',
    )
    expect(signaturePromise?.verification).toContain(
      'publish -> activate -> usable refs',
    )
    expect(signaturePromise?.authorityBoundary).toContain(
      'pure gate projections do not by themselves mutate package listings',
    )
  })

  test('partner payout ledger records projection evidence while keeping settlement blocked', () => {
    const document = publicProductPromisesDocument()
    const partnerPromise = document.promises.find(
      promise => promise.promiseId === 'autopilot_sites.partner_payout_ledger.v1',
    )

    expect(partnerPromise).toMatchObject({
      state: 'red',
      blockerRefs: [
        'blocker.product_promises.partner_first_real_payout_pending',
      ],
      evidenceRefs: expect.arrayContaining([
        'apps/openagents.com/workers/api/src/partner-payout-public-projection.ts',
        'apps/openagents.com/workers/api/src/partner-payout-public-routes.ts',
        'apps/openagents.com/workers/api/src/partner-payout-dispatch.ts',
        'apps/openagents.com/workers/api/src/partner-attribution-policy.ts',
        'apps/openagents.com/workers/api/src/partner-agreement-routes.ts',
        'apps/openagents.com/workers/api/src/public-partner-payout-receipt-routes.ts',
        'route:/api/operator/partners/agreements',
        'route:/api/operator/partners/payout-ledger/{payoutRef}/dispatch',
        'route:/api/public/partner-payouts',
        'route:/api/public/partner-payout-receipts/{receiptRef}',
      ]),
    })
    expect(partnerPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.partner_projection_api_missing',
    )
    expect(partnerPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.partner_attribution_policy_missing',
    )
    expect(partnerPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.partner_payout_settlement_not_wired',
    )
    expect(partnerPromise?.safeCopy).toContain(
      'explicit-agreement-only attribution policy',
    )
    expect(partnerPromise?.verification).toContain(
      'source-level settlement-dispatch blockers',
    )
    expect(partnerPromise?.authorityBoundary).toContain(
      'public aggregate projections are not spendable value',
    )
  })

  test('ablation derisking ledger clears projection, harness, and eval-reproduction blockers only', () => {
    const document = publicProductPromisesDocument()
    const ablationPromise = document.promises.find(
      promise => promise.promiseId === 'training.ablation_system.v1',
    )

    expect(ablationPromise).toMatchObject({
      state: 'planned',
      blockerRefs: [
        'blocker.product_promises.paid_ablation_dispatch_missing',
      ],
      evidenceRefs: expect.arrayContaining([
        'docs/training/2026-06-20-ablation-one-delta-harness.md',
        'docs/training/2026-06-20-ablation-eval-reproduction-receipt.md',
        'https://openagents.com/api/public/training/ablation-derisking-ledger',
        'apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.ts',
        'apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.test.ts',
      ]),
    })
    expect(ablationPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.ablation_ledger_projection_missing',
    )
    expect(ablationPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.ablation_harness_missing',
    )
    expect(ablationPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.eval_suite_reproduction_missing',
    )
    expect(ablationPromise?.safeCopy).toContain(
      'retained Psion checkpoint-eval reproduction receipt',
    )
    expect(ablationPromise?.verification).toContain(
      'zero paid ablations and zero accepted verdicts',
    )
    expect(ablationPromise?.authorityBoundary).toContain(
      'no dispatch or spend authority',
    )
  })

  test('post-training arc clears the instruct SFT lane and fixture sync blockers only', () => {
    const document = publicProductPromisesDocument()
    const postTrainingPromise = document.promises.find(
      promise => promise.promiseId === 'training.post_training_arc.v1',
    )

    expect(postTrainingPromise).toMatchObject({
      state: 'planned',
      blockerRefs: [
        'blocker.product_promises.instruct_sft_paid_dispatch_missing',
        'blocker.product_promises.preference_rollout_work_missing',
        'blocker.product_promises.vibe_test_artifact_missing',
      ],
      evidenceRefs: expect.arrayContaining([
        'docs/training/2026-06-20-psion-instruct-sft-lane-receipt.md',
        'docs/training/2026-06-20-psion-instruct-sft-fixture-sync.md',
        'route:/api/public/training/post-training-arc/instruct-sft-lane',
        'route:/api/public/training/post-training-arc/dpo-preference-workload',
        'route:/api/public/training/post-training-arc/vibe-test-rubric',
        'apps/openagents.com/workers/api/src/training-post-training-instruct-sft.ts',
        'apps/openagents.com/workers/api/src/training-post-training-instruct-sft.test.ts',
        'apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.ts',
        'apps/openagents.com/workers/api/src/training-post-training-dpo-preference-workload.test.ts',
        'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.ts',
        'apps/openagents.com/workers/api/src/training-post-training-vibe-test-rubric.test.ts',
        'apps/openagents.com/workers/api/src/training-post-training-paid-dispatch.ts',
        'apps/openagents.com/workers/api/src/training-post-training-paid-dispatch.test.ts',
        'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.ts',
        'apps/openagents.com/workers/api/src/post-training-vibe-test-rubric.test.ts',
        'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.ts',
        'apps/openagents.com/workers/api/src/cs336-a5-dpo-preference-workload.test.ts',
        'https://github.com/OpenAgentsInc/psionic/pull/1132',
        'https://github.com/OpenAgentsInc/psionic/blob/main/fixtures/psion/instruct/psion_instruct_sft_lane_report_v1.json',
      ]),
    })
    expect(postTrainingPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.instruct_sft_lane_missing',
    )
    expect(postTrainingPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.instruct_sft_fixture_sync_missing',
    )
    expect(postTrainingPromise?.safeCopy).toContain(
      'owned versioned chat template',
    )
    expect(postTrainingPromise?.safeCopy).toContain(
      'No paid OpenAgents SFT dispatch exists yet',
    )
    expect(postTrainingPromise?.safeCopy).toContain(
      'deterministicReferenceWorkloadAvailable=true',
    )
    expect(postTrainingPromise?.safeCopy).toContain(
      'vibeTestArtifactAvailable=false',
    )
    expect(postTrainingPromise?.verification).toContain(
      'clears blocker.product_promises.instruct_sft_fixture_sync_missing',
    )
    expect(postTrainingPromise?.verification).toContain(
      'paidPreferenceDispatchAvailable=false',
    )
    expect(postTrainingPromise?.verification).toContain(
      'blocker.product_promises.preference_rollout_work_missing remains',
    )
    expect(postTrainingPromise?.verification).toContain(
      'blocker.product_promises.vibe_test_artifact_missing remains',
    )
    expect(postTrainingPromise?.authorityBoundary).toContain(
      'not a model-quality',
    )
  })

  test('flips demand provenance green on broad projection coverage with no external claim', () => {
    const decoded = S.decodeUnknownSync(ProductPromisesDocument)(
      publicProductPromisesDocument(),
    )
    const demandProvenancePromise = decoded.promises.find(
      promise => promise.promiseId === 'proof.demand_provenance.v1',
    )

    expect(demandProvenancePromise?.state).toBe('green')
    // Broad coverage cleared the only remaining blocker.
    expect(demandProvenancePromise?.blockerRefs).toEqual([])
    expect(demandProvenancePromise?.safeCopy).toContain(
      'GET /api/public/demand-provenance',
    )
    expect(demandProvenancePromise?.safeCopy).toContain(
      'externalDemandClaimAllowed:false',
    )
    // The green is a coverage/labeling completeness flip, NOT an external claim.
    expect(demandProvenancePromise?.unsafeCopy).toContain(
      'externalDemandClaimAllowed is false',
    )
    expect(demandProvenancePromise?.verification).toContain(
      'ALL revenue-bearing public surfaces',
    )
    expect(demandProvenancePromise?.authorityBoundary).toContain(
      'grants no settlement or reporting authority',
    )
  })

  test('keeps Khala model-family mix yellow with explicit staleness and demand caveats', () => {
    const decoded = S.decodeUnknownSync(ProductPromisesDocument)(
      publicProductPromisesDocument(),
    )
    const modelMixPromise = decoded.promises.find(
      promise => promise.promiseId === 'metrics.khala_model_family_mix_public.v1',
    )

    expect(modelMixPromise?.state).toBe('yellow')
    expect(modelMixPromise?.safeCopy).toContain('liveAt/generatedAt')
    expect(modelMixPromise?.safeCopy).toContain('maxStalenessSeconds:0')
    expect(modelMixPromise?.safeCopy).toContain('stable public model-family')
    expect(modelMixPromise?.safeCopy).toContain('Pylon-Codex own-capacity')
    expect(modelMixPromise?.unsafeCopy).toContain(
      'external customer demand, revenue',
    )
    expect(modelMixPromise?.evidenceRefs).toContain(
      'route:/api/public/khala-tokens-served/model-mix',
    )
    expect(modelMixPromise?.blockerRefs).toEqual([
      'blocker.product_promises.model_mix_green_owner_signoff_pending',
    ])
    expect(modelMixPromise?.verification).toContain('issue #6858')
    expect(modelMixPromise?.verification).toContain(
      'Green still requires a live receipt/owner sign-off',
    )
  })

  test('keeps kernel optimization planned while exposing code-backed dispatch and parity receipt machinery', () => {
    const decoded = S.decodeUnknownSync(ProductPromisesDocument)(
      publicProductPromisesDocument(),
    )
    const kernelPromise = decoded.promises.find(
      promise =>
        promise.promiseId ===
        'compute.agentic_kernel_optimization_at_scale.v1',
    )

    expect(kernelPromise).toMatchObject({
      state: 'planned',
      blockerRefs: [
        'blocker.product_promises.agentic_kernel_optimization_at_scale_run_missing',
        'blocker.product_promises.agentic_kernel_optimization_settlement_receipts_missing',
      ],
    })
    expect(kernelPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.agentic_kernel_optimization_throughput_parity_verification_missing',
    )
    expect(kernelPromise?.blockerRefs).not.toContain(
      'blocker.product_promises.agentic_kernel_optimization_market_dispatch_missing',
    )
    expect(kernelPromise?.evidenceRefs).toEqual(
      expect.arrayContaining([
        'packages/tassadar-executor/src/kernel-optimization-dispatch.ts',
        'packages/tassadar-executor/src/kernel-optimization-dispatch.test.ts',
        'packages/tassadar-executor/src/kernel-optimization-parity.ts',
        'packages/tassadar-executor/src/kernel-optimization-parity.test.ts',
      ]),
    )
    expect(kernelPromise?.safeCopy).toContain(
      'buildKernelOptimizationAcceptedWorkReceipt',
    )
    expect(kernelPromise?.verification).toContain(
      'Green still requires a real live market-dispatched optimized kernel',
    )
  })

  test('blocks announcement copy until the live endpoint serves the announced version', () => {
    const document = publicProductPromisesDocument()

    expect(
      publicProductPromisesAnnouncementReadiness(PublicProductPromisesVersion, document),
    ).toMatchObject({
      blockerRefs: [],
      expectedVersion: PublicProductPromisesVersion,
      maxStalenessSeconds: 0,
      servedVersion: PublicProductPromisesVersion,
      status: 'ready',
    })
    expect(
      publicProductPromisesAnnouncementReadiness('2026-06-20.55', document),
    ).toMatchObject({
      blockerRefs: [
        'product-promises-announcement-blocker:expected-version-not-served:2026-06-20.55',
      ],
      expectedVersion: '2026-06-20.55',
      servedVersion: PublicProductPromisesVersion,
      status: 'blocked',
    })
  })
})
