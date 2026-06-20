import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
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

    expect(decoded.version).toBe('2026-06-20.2')
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
    // update with no state flips, so the count remained 21. The 2026-06-20.2 pass flips training.verification_classes.v1 yellow -> green (owner-authorized #4674 per-contribution sampling decision), so green is now exactly 22.
    expect(
      decoded.promises.filter(promise => promise.state === 'green').length,
    ).toBe(22)
    expect(decoded.verificationSummary.evidenceRefCount).toBeGreaterThan(0)
    expect(decoded.verificationSummary.uniqueBlockerCount).toBeGreaterThan(0)
    expect(
      decoded.verificationSummary.topBlockedPromises.length,
    ).toBeGreaterThan(0)
    expect(
      decoded.promises.every(promise => promise.sourceRefs.length > 0),
    ).toBe(true)
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
          state: 'planned',
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
          ]),
        }),
        expect.objectContaining({
          promiseId: 'training.model_ladder.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.r1_full_rehearsal_missing',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'training.public_distributed_training_run.v1',
          state: 'red',
          evidenceRefs: expect.arrayContaining(['docs/transcripts/236.md']),
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
          state: 'red',
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
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.llm_computer_training_run_definition_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'promise:compute.tassadar_executor_poc.v1',
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
            'blocker.product_promises.agentic_npm_registry_not_live',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'referral.refer_once_earn_forever.v1',
          state: 'red',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.referral_first_real_payout_pending',
            'blocker.product_promises.referral_cross_category_accrual_unbuilt',
          ]),
          evidenceRefs: expect.arrayContaining(['docs/transcripts/239.md']),
        }),
        expect.objectContaining({
          promiseId: 'autopilot.all_in_one_business_system.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'cloud.primitives_suite.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'cloud.fine_tuning_service.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'cloud.sandbox_compute_service.v1',
          state: 'red',
        }),
        expect.objectContaining({
          promiseId: 'markets.open_protocol_markets.v1',
          state: 'planned',
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.liquidity_market_unbuilt',
            'blocker.product_promises.risk_market_unbuilt',
          ]),
        }),
        expect.objectContaining({
          promiseId: 'marketplace.compose_and_list_products.v1',
          state: 'planned',
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
          state: 'red',
          evidenceRefs: expect.arrayContaining([
            'docs/2026-06-12-episode-236-training-launch-gap-audit.md',
          ]),
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
          state: 'planned',
        }),
        expect.objectContaining({
          promiseId: 'proof.claim_upgrade_receipts.v1',
          state: 'yellow',
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
            'blocker.product_promises.repo_studying_customer_private_validation_missing',
            'blocker.product_promises.repo_studying_marketplace_metering_missing',
            'blocker.product_promises.repo_studying_payout_settlement_gates_missing',
          ]),
          evidenceRefs: expect.arrayContaining([
            'docs/research/machine-studying/openagents-studybench/runs/2026-06-17-mvp-14-baseline-packet-gepa-comparison.md',
            'packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json',
            'docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md',
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
            'blocker.product_promises.wasm_plugin_marketplace_not_live',
          ]),
          promiseId: 'marketplace.wasm_plugins.v1',
          state: 'planned',
        }),
        expect.objectContaining({
          audience: expect.arrayContaining(['contributor', 'operator', 'public']),
          blockerRefs: expect.arrayContaining([
            'blocker.product_promises.public_gradient_promoted_window_receipts_missing',
          ]),
          promiseId: 'training.public_gradient_windows.v1',
          state: 'planned',
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
      'pylon.v03_release_candidate.v1': 'yellow',
      'pylon.release_tomorrow.v1': 'yellow',
      'pylon.first_real_model_training_run.v1': 'yellow',
      'pylon.largest_decentralized_training_claim.v1': 'red',
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

    // The participant/scale methodology + comparable-runs research are attached
    // to the largest-run and consumer-self-serve promises.
    expect(
      byId.get('pylon.largest_decentralized_training_claim.v1')?.evidenceRefs,
    ).toEqual(
      expect.arrayContaining([
        'docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md',
        'docs/training/2026-06-19-comparable-decentralized-training-runs-research.md',
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

  test('blocks announcement copy until the live endpoint serves the announced version', () => {
    const document = publicProductPromisesDocument()

    expect(
      publicProductPromisesAnnouncementReadiness('2026-06-20.2', document),
    ).toMatchObject({
      blockerRefs: [],
      expectedVersion: '2026-06-20.2',
      maxStalenessSeconds: 0,
      servedVersion: '2026-06-20.2',
      status: 'ready',
    })
    expect(
      publicProductPromisesAnnouncementReadiness('2026-06-12.1', document),
    ).toMatchObject({
      blockerRefs: [
        'product-promises-announcement-blocker:expected-version-not-served:2026-06-12.1',
      ],
      expectedVersion: '2026-06-12.1',
      servedVersion: '2026-06-20.2',
      status: 'blocked',
    })
  })
})
