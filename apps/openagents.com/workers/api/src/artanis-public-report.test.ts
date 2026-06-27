import { Effect } from 'effect'
import { afterEach, describe, expect, test, vi } from 'vitest'

import {
  type ArtanisForumPublicationQueueRecord,
  exampleArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import { ArtanisLoopTickRecord } from './artanis-loop'
import {
  artanisPublicReportHasPrivateMaterial,
  artanisPublicReportSnapshot,
  publicNexusPylonReceiptRouteRefsFromRefs,
} from './artanis-public-report'
import { handlePublicArtanisReportApi } from './artanis-public-report-routes'
import { publicPylonStatsFromNexusPayload } from './public-pylon-stats'
import { publicScannerSafeRef } from './public-ref-scanner-safety'

const nowIso = '2026-06-07T02:00:00.000Z'
const scannerShapedBridgeRef = 'artanis-mdk-bridge-8b378373002501f3e896dcd3'
const scannerSafeBridgeRef = publicScannerSafeRef(
  'evidence.public.pylon_v0_2.omega_gate',
  scannerShapedBridgeRef,
)

const deliveredForumQueue = (): ArtanisForumPublicationQueueRecord => {
  const queue = exampleArtanisForumPublicationQueue()

  return {
    ...queue,
    intents: queue.intents.map(intent => ({
      ...intent,
      deliveredAtIso: '2026-06-07T01:24:00.000Z',
      deliveryReceiptRefs: ['receipt.public.artanis.forum_status_delivered'],
      deliveryState: 'delivered',
      postRef: 'post.public.forum.artanis.status.2',
      updatedAtIso: '2026-06-07T01:24:00.000Z',
    })),
    updatedAtIso: '2026-06-07T01:24:00.000Z',
  }
}

describe('Artanis public report', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('links Artanis admin closeout refs through the public receipt route', () => {
    expect(
      publicNexusPylonReceiptRouteRefsFromRefs([
        'assignment.artanis_admin.20260611011429',
        'receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260611011429',
        'receipt.public.artanis.tick_closeout',
      ]),
    ).toEqual([
      'route:/api/public/nexus-pylon/receipts/assignment.artanis_admin.20260611011429',
      'route:/api/public/nexus-pylon/receipts/receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260611011429',
    ])
  })

  test('aggregates public-safe Artanis, Pylon, R10, Model Lab, Forum, and blocker state', () => {
    const report = artanisPublicReportSnapshot({
      nowIso,
      pylonStats: publicPylonStatsFromNexusPayload({
        as_of_unix_ms: Date.parse(nowIso),
        hosted_nexus_relay_url: 'wss://nexus.openagents.com/',
        nexus_accepted_work_payout_receipt_refs: [
          'receipt.nexus.public.artanis.accepted_work_settlement',
        ],
        nexus_accepted_work_payout_sats_paid_24h: 55,
        nexus_accepted_work_payout_sats_paid_total: 4100,
        pylon_sessions_online_now: 9,
        pylons_online_now: 7,
        recent_pylons: [
          {
            client_version: 'openagents.pylon@0.2.0',
            eligible_product_count: 2,
            node_label: 'public-pylon-one',
            nostr_pubkey_short: 'npub-public-one',
            products: ['training', 'inference'],
            ready_model: 'gemma4:e4b',
            relay_urls: ['wss://nexus.openagents.com/'],
            runtime_state: 'online',
          },
        ],
        sellable_pylons_online_now: 5,
        training_accepted_contributors: 3,
        training_assigned_contributors: 4,
      }),
    })
    const serialized = JSON.stringify(report)

    expect(report).toMatchObject({
      agentId: 'agent_artanis',
      agentRef: 'artanis',
      displayName: 'Artanis',
      reportRef: 'report.public.artanis.status_aggregator',
      runtimeState: 'running',
      authoritySummary: {
        dispatchAuthorityAllowed: false,
        dispatcherGateGreen: false,
        forumAutoPublishAllowed: false,
        greenLaunchCopyAllowed: true,
        operatorApprovalRequired: true,
        providerMutationAuthorityAllowed: false,
        scheduledRunnerDispatchAllowed: false,
        settlementAuthorityAllowed: false,
        spendAuthorityAllowed: false,
        statusProjectionAllowed: true,
      },
      autonomousLoop: {
        active: true,
        latestTickState: 'completed',
        state: 'running',
      },
      activityLog: expect.arrayContaining([
        expect.objectContaining({
          actorRef: 'Codex-1',
          publicIssueNumber: 6415,
          repoRef: 'OpenAgentsInc/openagents',
          title: 'Loop tick projected',
        }),
        expect.objectContaining({
          actorRef: 'Codex-2',
          publicIssueNumber: 6359,
          title: 'Capacity reviewed',
        }),
      ]),
      brainSummary: {
        decisionLog: expect.arrayContaining([
          expect.objectContaining({
            publicIssueNumber: 6415,
            title: 'Dispatch gate',
          }),
        ]),
        failureModes: expect.arrayContaining([
          expect.objectContaining({
            failureModeRef: 'failure_mode.public.artanis.authority_blockers',
            publicIssueNumber: 6415,
          }),
        ]),
        sourceRefs: expect.arrayContaining([
          'gate.public.artanis.production_launch.v1',
        ]),
      },
      modelLabSummary: {
        readiness: 'ready',
        reportRef: 'report.public.model_lab_autopilot_v2',
      },
      gepaScheduledRunner: {
        assignmentDispatchAllowed: false,
        budgetMode: 'unpaid_smoke_no_spend',
        forumAutoPublishAllowed: false,
        mutationAuthorityAllowed: false,
        productionSmokeCheckPassed: true,
        proofRef: 'proof.public.artanis.gepa_scheduled_runner.bounded_001',
        state: 'retained',
      },
      probeGepaProductionSmoke: {
        acceptedCloseoutCount: 1,
        completedMetricCalls: 2,
        mutationAuthorityAllowed: false,
        payoutClaimAllowed: false,
        pylonAssignmentRefs: [
          'assignment.public.pylon_gepa.live_stage0.demo_1',
          'assignment.public.pylon_gepa.live_stage0.demo_2',
        ],
        pylonRefs: ['pylon.demo.stage0.one', 'pylon.demo.stage0.two'],
        rejectedCloseoutCount: 1,
        state: 'retained',
      },
      probeGepaSummary: {
        acceptedOutcomeRefs: [],
        candidateState: 'shadow',
        claimText:
          'Benchmark validation only; no paid customer outcome improvement claim.',
        productOutcomeClaimAllowed: false,
        publicProofRefs: [],
        routeScorecardRefs: [
          'route_scorecard.probe_gepa.live_stage0.demo_1',
          'route_scorecard.probe_gepa.live_stage0.demo_2',
        ],
        workroomComparisonRefs: [],
        workroomOutcomeRefs: [],
      },
      healthSummary: {
        overclaimBlocked: false,
        overallState: 'healthy',
        pendingApprovalCount: 0,
        staleOrBlockedSignalCount: 0,
      },
      pylonSummary: {
        acceptedWorkBitcoin24h: '0.00000055 bitcoin (55 sats)',
        acceptedWorkSettlementGate: {
          publicPaidWorkTotalsAllowed: true,
          receiptBackedTotalsAvailable: true,
          state: 'ready',
        },
        acceptedWorkSettlementReceiptRefs: [
          'receipt.nexus.public.artanis.accepted_work_settlement',
        ],
        acceptedWorkBitcoinTotal: '0.00004100 bitcoin (4,100 sats)',
        assignmentReadyPylonsOnlineNow: 5,
        earningLaunchGate: {
          blockerRefs: [],
          publicEarningCopyAllowed: true,
          requiredAssignmentReadyPylonsPresent: true,
          requiredOnlinePylonsPresent: true,
          requiredWalletReadyPylonsPresent: true,
          state: 'ready',
        },
        feedStatus: 'live',
        nexusPublicRefs: [
          'nexus.public.accepted_work_payout_receipts',
          'receipt.nexus.public.artanis.accepted_work_settlement',
          'route:/api/public/nexus-pylon/receipts/receipt.nexus.public.artanis.accepted_work_settlement',
          'wss://nexus.openagents.com/',
        ],
        omegaPublicRefs: [
          'https://nexus.openagents.com/api/stats',
          'nexus.public.stats',
          'omega.public.pylon_api.registrations',
        ],
        pylonsOnlineNow: 7,
        walletReadyPylonsOnlineNow: 5,
      },
      pylonLaunchCommunication: {
        forumIntentReady: true,
        forumPostTitle: 'Artanis Pylon v0.2 launch readiness update',
        primaryForumTopicRef:
          'topic.public.forum.artanis.pylon_release_work_log',
        primaryForumTopicUrl:
          'https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888',
        readinessRef: 'readiness.public.artanis.pylon_v0_2',
      },
      pylonReleaseParity: {
        acceptedWorkClaimAllowed: false,
        generalAvailabilityClaimAllowed: false,
        packageVersionMatched: false,
        platformReady: false,
        releaseReady: false,
        shippedClaimAllowed: false,
        sourceLevelSupportVisible: true,
        state: 'blocked',
      },
      pylonOmegaReleaseGate: {
        canAnnouncePylonV02Release: true,
        hostedMdkDirectPayoutClaimAllowed: false,
        failedOrPendingRequiredCount: 0,
        multiPylonObservedDistinctPylonCount: 2,
        multiPylonPaidWorkProofComplete: true,
        multiPylonRequiredDistinctPylonCount: 2,
        payoutModeGate: {
          activeMode: 'local_mdk_agent_wallet_bridge',
          hostedDirectPayoutClaimAllowed: false,
          localBridgePayoutClaimAllowed: true,
          livePayoutClaimAllowed: true,
          state: 'ready',
        },
        releasePublicationAllowed: false,
        state: 'limited_launcher_release_shipped',
        stateLabel:
          'Pylon v0.2 package launcher is shipped with listed platform and authority limits',
      },
      productionLaunchGate: {
        canClaimBoundedStatusProjection: true,
        canClaimContinuouslyRunning: false,
        dispatchAuthorityAllowed: false,
        failedOrPendingRequiredCount: 0,
        forumAutoPublishAllowed: false,
        gateRef: 'gate.public.artanis.production_launch.v1',
        providerMutationAuthorityAllowed: false,
        settlementAuthorityAllowed: false,
        state: 'ready',
        stateLabel: 'Ready for controlled production enablement',
        walletSpendAuthorityAllowed: false,
      },
      forumRewardVisibility: {
        acceptedContributionCount: 1,
        acceptedWorkPayoutClaimAllowed: false,
        contentRewardCount: 2,
        liveWalletSpendAllowed: false,
        state: 'public_receipts_visible',
      },
      forumRewardSmoke: {
        acceptedWorkPayoutClaimAllowed: false,
        exchangeCount: 2,
        mode: 'simulation',
        modeLabel: 'Simulation only',
        providerSettlementClaimAllowed: false,
        usedLiveBitcoin: false,
      },
    })
    expect(report.forumLinks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          href: '/forum/f/artanis',
          label: 'Artanis Forum',
        }),
        expect.objectContaining({
          href: '/forum/t/88888888-4001-4001-8001-888888888888',
          label: 'Status topic',
        }),
        expect.objectContaining({
          label: 'Model Lab',
        }),
        expect.objectContaining({
          href: '/forum/t/88888888-4004-4004-8004-888888888888',
          label: 'Pylon release updates',
        }),
      ]),
    )
    expect(report.r10Claims.map(claim => claim.state)).toEqual(
      expect.arrayContaining([
        'blocked',
        'measured',
        'modeled',
        'planned',
        'prohibited',
        'verified',
      ]),
    )
    expect(
      report.standaloneClaims.map(claim => [claim.area, claim.state]),
    ).toEqual(
      expect.arrayContaining([
        ['autonomous_loop', 'measured'],
        ['operator_steering', 'verified'],
        ['forum_communication', 'verified'],
        ['pylon_campaign', 'measured'],
        ['nexus_pylon_administration', 'planned'],
        ['model_lab_stewardship', 'verified'],
        ['work_routing', 'modeled'],
        ['spend_authority', 'blocked'],
        ['bitcoin_rewards', 'blocked'],
        ['accepted_work_payout', 'prohibited'],
        ['settlement', 'prohibited'],
      ]),
    )
    expect(report.publicBlockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.artanis.no_live_spend_authority',
        'blocker.artanis.nexus_pylon_admin_adapters_not_live',
        'blocker.public.no_live_spend_cap',
        'blocker.public.no_named_wallet_authority',
        'blocker.no_approved_live_spend_cap',
        'blocker.no_public_settlement_receipt_chain',
        'blocker.pylon.release_artifact_not_retained',
        'blocker.public.pylon_v0_2.release_tag_missing',
        'blocker.public.pylon_v0_2.package_version_mismatch',
        'blocker.public.pylon_v0_2.runtime_smoke_missing',
        'blocker.public.pylon_v0_2.eligibility_telemetry_missing',
        'blocker.public.pylon_v0_2.accepted_work_proof_missing',
        'blocker.mdk.hosted_programmatic_payouts_disabled',
        'blocker.public.artanis.dispatch_authority_not_granted',
        'blocker.public.artanis.forum_auto_publish_not_granted',
        'blocker.public.artanis.provider_mutation_not_granted',
        'blocker.public.artanis.settlement_authority_not_granted',
        'blocker.public.artanis.spend_authority_not_granted',
      ]),
    )
    expect(report.publicBlockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.production_e2e_smoke.blocked',
    )
    expect(report.publicBlockerRefs).not.toContain(
      'blocker.public.artanis.launch_gate.scheduled_runner.blocked',
    )
    expect(report.productionLaunchGate.verificationTargetRefs).toEqual(
      expect.arrayContaining([
        'route:/artanis',
        'route:/autopilot',
        'route:/api/public/artanis/report',
        'route:/api/operator/artanis/console',
        'route:/api/public/pylon-stats',
        'signal.public.artanis.health_staleness',
        'topic.public.forum.artanis.status',
      ]),
    )
    expect(report.productionLaunchGate.publicBlockedClaimPhrases).toEqual(
      expect.arrayContaining([
        'blocked_claim.public.artanis.unbounded_autonomy',
        'blocked_claim.public.pylon_v0_2.shipped',
      ]),
    )
    expect(report.healthSummary.attentionLabels).toEqual([])
    expect(report.healthSummary.publicRecoveryActionRefs).toEqual([])
    expect(report.authoritySummary.authorityBlockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.artanis.dispatch_authority_not_granted',
        'blocker.public.artanis.forum_auto_publish_not_granted',
        'blocker.public.artanis.provider_mutation_not_granted',
        'blocker.public.artanis.settlement_authority_not_granted',
        'blocker.public.artanis.spend_authority_not_granted',
      ]),
    )
    expect(report.authoritySummary.authorityBlockerRefs).not.toContain(
      'blocker.public.artanis.green_launch_copy.health_stale',
    )
    expect(report.authoritySummary.forumIntentIdempotencyRefs).toEqual([
      'dedupe.public.artanis.forum_intent_idempotency_key',
    ])
    expect(report.authoritySummary.runbookCommandRefs).toEqual(
      expect.arrayContaining([
        'runbook.public.artanis.production_launch.disable',
        'runbook.public.artanis.production_launch.pause',
        'runbook.public.artanis.production_launch.revoke',
      ]),
    )
    expect(report.forumRewardVisibility.caveatRefs).toEqual(
      expect.arrayContaining([
        'caveat.public.content_rewards_not_accepted_work_payouts',
        'caveat.public.no_unconditional_earning_promise',
      ]),
    )
    expect(report.forumRewardVisibility.paidActionRefs).toEqual(
      expect.arrayContaining([
        'paid_action.public.forum.post_reward',
        'paid_action.public.forum.topic_boost',
        'paid_action.public.forum.topic_fund',
      ]),
    )
    expect(report.forumRewardSmoke.runReasonRefs).toEqual(
      expect.arrayContaining([
        'reason.public.deterministic_fake_bitcoin_simulation',
        'reason.public.no_concrete_spend_cap',
        'reason.public.no_owner_approved_named_wallet',
      ]),
    )
    expect(report.forumRewardSmoke.receiptProjectionRefs).toEqual(
      expect.arrayContaining([
        'receipt_projection.public.forum_reward.alice_to_ben',
        'receipt_projection.public.forum_reward.ben_to_alice',
      ]),
    )
    expect(report.pylonLaunchCommunication.stageSummaryRefs).toEqual(
      expect.arrayContaining([
        'stage_summary.public.pylon_v0_2.source_ready.verified',
        'stage_summary.public.pylon_v0_2.release_ready.blocked',
        'stage_summary.public.pylon_v0_2.platform_ready.blocked',
        'stage_summary.public.pylon_v0_2.eligible.planned',
        'stage_summary.public.pylon_v0_2.accepted.prohibited',
        'stage_summary.public.pylon_v0_2.paid.prohibited',
        'stage_summary.public.pylon_v0_2.settled.prohibited',
      ]),
    )
    expect(report.pylonReleaseParity.stageSummaryRefs).toEqual(
      expect.arrayContaining([
        'stage_summary.public.pylon_v0_2.release_parity.source_support.verified',
        'stage_summary.public.pylon_v0_2.release_parity.release_assets.blocked',
        'stage_summary.public.pylon_v0_2.release_parity.platform_smoke.blocked',
        'stage_summary.public.pylon_v0_2.release_parity.settlement.blocked',
      ]),
    )
    expect(report.receiptRefs).toEqual(
      expect.arrayContaining([
        'receipt.public.artanis.tick_closeout.gepa_status_projection',
        'receipt_projection.public.forum_reward.alice_to_ben',
        'receipt_projection.public.forum_reward.ben_to_alice',
        'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
        'receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
      ]),
    )
    expect(report.publicCaveatRefs).toEqual(
      expect.arrayContaining([
        'caveat.public.retained_smoke_not_public_score',
        'caveat.public.pylon_work_unpaid_smoke_no_settlement_claim',
        'caveat.public.gepa_text_optimization_not_model_training',
        'caveat.mdk_agent_wallet.local_bridge_not_hosted_direct_payout',
      ]),
    )
    expect(report.pylonOmegaReleaseGate.multiPylonProofRefs).toEqual(
      expect.arrayContaining([
        'assignment.public.issue_438.issue_438_artanis_1780822221',
        'pylon.public.issue_438_edge_wallet',
        'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
        scannerSafeBridgeRef,
        'pylon.public.artanis.bridge.8b378373',
        'receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3',
      ]),
    )
    expect(artanisPublicReportHasPrivateMaterial(report)).toBe(false)
    expect(serialized).not.toContain(scannerShapedBridgeRef)
    expect(serialized).not.toContain('https://openagents.com/autopilot')
    expect(serialized).toContain('route:/autopilot')
    expect(serialized).not.toContain('authGrantRef')
    expect(serialized).not.toContain('payloadJson')
    expect(serialized).not.toContain('hiddenSteering')
    expect(serialized).not.toContain('evidence.private')
    expect(serialized).not.toContain('Pylon v0.2 is shipped')
    expect(serialized).not.toContain('Pylon v0.2 is ready for everyone')
    expect(serialized).not.toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/)
  })

  test('serves the same public-safe projection to anonymous and authenticated visitors', async () => {
    const store = { listRegistrations: () => Promise.resolve([]) }

    const anonymousResponse = await Effect.runPromise(
      handlePublicArtanisReportApi(
        new Request('https://openagents.com/api/public/artanis/report'),
        { store },
      ),
    )
    const authenticatedResponse = await Effect.runPromise(
      handlePublicArtanisReportApi(
        new Request('https://openagents.com/api/public/artanis/report', {
          headers: { authorization: 'Bearer oa_agent_test' },
        }),
        { store },
      ),
    )
    const anonymous = (await anonymousResponse.json()) as Record<
      string,
      unknown
    >
    const authenticated = (await authenticatedResponse.json()) as Record<
      string,
      unknown
    >

    expect(anonymousResponse.status).toBe(200)
    expect(authenticatedResponse.status).toBe(200)
    expect(anonymousResponse.headers.get('cache-control')).toBe('no-store')
    expect(anonymous).toMatchObject({
      agentId: 'agent_artanis',
      forumLinks: expect.arrayContaining([
        expect.objectContaining({ href: '/forum/f/artanis' }),
      ]),
    })
    expect(authenticated).toMatchObject({
      agentId: anonymous.agentId,
      reportRef: anonymous.reportRef,
      publicBlockerRefs: anonymous.publicBlockerRefs,
    })
  })

  test('links a delivered canonical status post when the Forum queue records delivery', () => {
    const report = artanisPublicReportSnapshot({
      forumPublicationQueue: deliveredForumQueue(),
      nowIso,
      pylonStats: publicPylonStatsFromNexusPayload({}),
    })

    expect(report.forumLinks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          description:
            'Latest delivered Artanis status Forum post: post.public.forum.artanis.status.2.',
          href: '/forum/t/88888888-4001-4001-8001-888888888888',
          label: 'Latest status post',
          topicRef: 'topic.public.forum.artanis.status',
        }),
      ]),
    )
    expect(artanisPublicReportHasPrivateMaterial(report)).toBe(false)
  })

  // Epic #4751 (instance 6, #4745): the report declares its staleness
  // contract, carries generatedAt, and flags a loop summary it cannot
  // back with a fresh persisted tick instead of asserting June-7
  // example state as current.
  describe('projection staleness declaration', () => {
    const persistedTick = (
      overrides: Partial<{
        nextTickAtIso: string | null
        updatedAtIso: string
      }> = {},
    ): ArtanisLoopTickRecord =>
      new ArtanisLoopTickRecord({
        actionProposals: [],
        approvalRequirements: [],
        artifactRefs: ['artifact.public.artanis.status_packet'],
        blockerRefs: [],
        caveatRefs: ['caveat.public.tick_evidence_only'],
        closeoutReceiptRefs: ['receipt.public.artanis.tick_closeout'],
        createdAtIso: '2026-06-11T00:00:00.000Z',
        forumPublicationIntentRefs: ['forum.public.artanis.status_intent'],
        goalRef: 'goal.public.artanis.pylon_model_lab',
        idempotencyKey: 'tick.public.artanis.staleness_probe',
        loopRef: 'loop.public.artanis.primary',
        nextTickAtIso: '2026-06-11T02:00:00.000Z',
        receiptRefs: ['receipt.public.artanis.context_loaded'],
        selectedContextRefs: ['context.public.artanis.pylon_readiness'],
        state: 'completed',
        tickRef: 'tick.public.artanis.staleness_probe',
        updatedAtIso: '2026-06-11T00:30:00.000Z',
        ...overrides,
      })

    test('declares the report contract and generation time', () => {
      const report = artanisPublicReportSnapshot({
        nowIso,
        pylonStats: publicPylonStatsFromNexusPayload({}),
      })

      expect(report.generatedAtUnixMs).toBe(Date.parse(nowIso))
      expect(report.staleness).toMatchObject({
        composition: 'live_at_read',
        maxStalenessSeconds: 0,
        rebuildsOn: expect.arrayContaining(['artanis_loop_tick_closeout']),
      })
      expect(report.autonomousLoop.staleness).toMatchObject({
        composition: 'rebuilt_on_transition',
        maxStalenessSeconds: 86_400,
        rebuildsOn: ['artanis_loop_tick_closeout'],
      })
      expect(artanisPublicReportHasPrivateMaterial(report)).toBe(false)
    })

    test('labels the typed-example fallback stale instead of serving it as live state', () => {
      const report = artanisPublicReportSnapshot({
        nowIso,
        pylonStats: publicPylonStatsFromNexusPayload({}),
      })

      expect(report.autonomousLoop.source).toBe('typed_example_fallback')
      expect(report.autonomousLoop.projectionStale).toBe(true)
      expect(report.autonomousLoop.latestTickAgeSeconds).toBe(null)
      expect(report.autonomousLoop.caveatRefs).toEqual(
        expect.arrayContaining([
          'caveat.public.artanis.loop_projection_example_fallback_not_live_state',
          'caveat.public.artanis.loop_tick_projection_exceeds_declared_staleness',
        ]),
      )
      expect(report.publicCaveatRefs).toEqual(
        expect.arrayContaining([
          'caveat.public.artanis.loop_projection_example_fallback_not_live_state',
        ]),
      )
    })

    test('a fresh persisted tick projects without stale flags', () => {
      const freshNowIso = '2026-06-11T01:00:00.000Z'
      const report = artanisPublicReportSnapshot({
        loopTicks: [persistedTick()],
        nowIso: freshNowIso,
        pylonStats: publicPylonStatsFromNexusPayload({
          as_of_unix_ms: Date.parse(freshNowIso),
        }),
      })

      expect(report.autonomousLoop.source).toBe('persisted_loop_ticks')
      expect(report.autonomousLoop.latestTickAgeSeconds).toBe(1_800)
      expect(report.autonomousLoop.nextTickOverdue).toBe(false)
      expect(report.autonomousLoop.projectionStale).toBe(false)
      expect(report.autonomousLoop.caveatRefs).not.toContain(
        'caveat.public.artanis.loop_tick_projection_exceeds_declared_staleness',
      )
      expect(artanisPublicReportHasPrivateMaterial(report)).toBe(false)
    })

    test('a tick older than the declared bound flags the projection stale', () => {
      const staleNowIso = '2026-06-11T01:00:00.000Z'
      const report = artanisPublicReportSnapshot({
        loopTicks: [
          persistedTick({
            nextTickAtIso: '2026-06-07T01:10:00.000Z',
            updatedAtIso: '2026-06-07T00:56:00.000Z',
          }),
        ],
        nowIso: staleNowIso,
        pylonStats: publicPylonStatsFromNexusPayload({
          as_of_unix_ms: Date.parse(staleNowIso),
        }),
      })

      expect(report.autonomousLoop.source).toBe('persisted_loop_ticks')
      expect(report.autonomousLoop.latestTickAgeSeconds).toBeGreaterThan(86_400)
      expect(report.autonomousLoop.nextTickOverdue).toBe(true)
      expect(report.autonomousLoop.projectionStale).toBe(true)
      expect(report.autonomousLoop.caveatRefs).toContain(
        'caveat.public.artanis.loop_tick_projection_exceeds_declared_staleness',
      )
      expect(report.publicCaveatRefs).toContain(
        'caveat.public.artanis.loop_tick_projection_exceeds_declared_staleness',
      )
    })

    test('a tick that misses only its own next-tick promise is overdue and stale', () => {
      const overdueNowIso = '2026-06-11T03:00:00.000Z'
      const report = artanisPublicReportSnapshot({
        loopTicks: [persistedTick()],
        nowIso: overdueNowIso,
        pylonStats: publicPylonStatsFromNexusPayload({
          as_of_unix_ms: Date.parse(overdueNowIso),
        }),
      })

      expect(report.autonomousLoop.latestTickAgeSeconds).toBeLessThan(86_400)
      expect(report.autonomousLoop.nextTickOverdue).toBe(true)
      expect(report.autonomousLoop.projectionStale).toBe(true)
    })
  })
})
