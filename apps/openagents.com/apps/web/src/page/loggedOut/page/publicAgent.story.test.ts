import { describe, expect, test } from 'vitest'
import type { PublicActivityTimelineEnvelope } from '@openagentsinc/public-activity-timeline'

import { PublicAgentRoute } from '../../../route'
import {
  SucceededLoadPublicActivityTimeline,
  SucceededLoadPublicAgentGoal,
  SucceededLoadPublicKhalaTokensServedHistory,
  SucceededLoadPublicPylonStats,
} from '../message'
import {
  PublicKhalaTokensServedHistory,
  init,
  type PublicPylonStats,
} from '../model'
import { update } from '../update'
import * as PublicAgent from './publicAgent'

const sampleHistory = PublicKhalaTokensServedHistory.make({
  window: '30d',
  bucket: 'day',
  timezone: 'America/Chicago',
  generatedAt: '2026-06-27T17:00:00.000Z',
  series: [
    { day: '2026-06-25', tokensServed: 50_000_000 },
    { day: '2026-06-26', tokensServed: 328_100_000 },
    { day: '2026-06-27', tokensServed: 100_000_000 },
  ],
})

const samplePylonStats: PublicPylonStats = {
  available: true,
  asOfLabel: '2026-06-27T17:00:00.000Z',
  asOfUnixMs: 1_782_580_800_000,
  caveatRefs: ['caveat.public.assignment_ready_is_not_payout_evidence'],
  earningLaunchGate: {
    blockedClaimRefs: [],
    blockerRefs: [],
    caveatRefs: [],
    gateRef: 'gate.public.pylon.earning',
    publicEarningCopyAllowed: true,
    requiredAssignmentReadyPylonsPresent: true,
    requiredOnlinePylonsPresent: true,
    requiredWalletReadyPylonsPresent: true,
    sourceRefs: ['source.public.pylon_stats'],
    state: 'ready',
    stateLabel: 'Ready',
  },
  error: null,
  hostedNexusRelayUrl: 'wss://relay.openagents.example',
  minimumClientVersion: '0.2.5',
  nexusAcceptedWorkPayoutReceiptRefs: [],
  nexusAcceptedWorkPayoutSatsPaid24h: null,
  nexusAcceptedWorkPayoutSatsPaidTotal: null,
  nexusAcceptedWorkSettlementGate: {
    blockerRefs: [],
    caveatRefs: ['caveat.public.receipt_backed_totals_only'],
    gateRef: 'gate.public.pylon.accepted_work_settlement',
    publicPaidWorkTotalsAllowed: false,
    receiptBackedTotalsAvailable: false,
    settledReceiptRefs: [],
    sourceRefs: ['source.public.pylon_stats'],
    state: 'blocked',
    stateLabel: 'Blocked',
  },
  nexusPayoutSatsPaidTotal: null,
  publicRealSatsSettled24h: null,
  publicRealSatsSettledTotal: null,
  pylonSessionsOnlineNow: 2,
  pylonsAssignmentReadyNow: 2,
  pylonsByClientVersion: { '0.2.5': 3 },
  pylonsByResourceMode: { coding: 2, training: 1 },
  pylonsOnlineNow: 3,
  pylonsRegisteredTotal: 7,
  pylonsSeen24h: 4,
  pylonsWalletReadyNow: 2,
  recentPylons: [
    {
      assignmentReadyNow: true,
      clientVersion: '0.2.5',
      eligibleProductCount: 3,
      lastHeartbeatAgeSeconds: 12,
      lastSeenAtLabel: '2026-06-27T16:59:48.000Z',
      lastSeenAtUnixMs: 1_782_580_788_000,
      nodeLabel: 'codex-east-1',
      nostrPubkeyShort: 'npub-east',
      onlineNow: true,
      ownerAgentRef: null,
      products: ['codex_agent_task'],
      pylonRef: 'pylon.public.codex-east-1',
      readyModel: 'openagents/pylon-codex',
      relayUrls: ['wss://relay.openagents.example'],
      runtimeState: 'online',
      tippingAvailable: false,
      tipEndpoint: null,
      walletReadyNow: true,
    },
    {
      assignmentReadyNow: true,
      clientVersion: '0.2.5',
      eligibleProductCount: 2,
      lastHeartbeatAgeSeconds: 22,
      lastSeenAtLabel: '2026-06-27T16:59:38.000Z',
      lastSeenAtUnixMs: 1_782_580_778_000,
      nodeLabel: 'codex-west-2',
      nostrPubkeyShort: 'npub-west',
      onlineNow: true,
      ownerAgentRef: null,
      products: ['codex_agent_task'],
      pylonRef: 'pylon.public.codex-west-2',
      readyModel: 'openagents/pylon-codex',
      relayUrls: ['wss://relay.openagents.example'],
      runtimeState: 'online',
      tippingAvailable: false,
      tipEndpoint: null,
      walletReadyNow: true,
    },
    {
      assignmentReadyNow: false,
      clientVersion: '0.2.5',
      eligibleProductCount: 1,
      lastHeartbeatAgeSeconds: 45,
      lastSeenAtLabel: '2026-06-27T16:59:15.000Z',
      lastSeenAtUnixMs: 1_782_580_755_000,
      nodeLabel: 'training-lab-1',
      nostrPubkeyShort: 'npub-lab',
      onlineNow: true,
      ownerAgentRef: null,
      products: ['training_trace'],
      pylonRef: 'pylon.public.training-lab-1',
      readyModel: 'openagents/training',
      relayUrls: ['wss://relay.openagents.example'],
      runtimeState: 'online',
      tippingAvailable: false,
      tipEndpoint: null,
      walletReadyNow: false,
    },
  ],
  sellablePylonsOnlineNow: 2,
  sourceRefs: ['source.public.pylon_stats'],
  sourceUrl: 'https://openagents.example/api/public/pylon-stats',
  status: 'live',
  trainingAcceptedContributors: 9,
  trainingAssignedContributors: 12,
  trainingModelProgressContributors: 4,
  treasuryPayoutCount24h: null,
  treasuryPayoutCountTotal: null,
  treasuryPayoutSatsPaid24h: null,
  treasuryPayoutSatsPaidTotal: null,
}

const sampleActivityTimeline: PublicActivityTimelineEnvelope = {
  events: [
    {
      actorRef: 'pylon.public.codex-east-1',
      blockerRefs: [],
      caveatRefs: [],
      cursor:
        '2026-06-27T16:59:55.000Z:pylon_presence:event.public.assignment_ready.codex-east-1',
      eventRef: 'event.public.assignment_ready.codex-east-1',
      kind: 'assignment_ready',
      refs: ['pylon.public.codex-east-1'],
      sourceKind: 'pylon_presence',
      sourceRefs: ['pylon.public.codex-east-1'],
      state: 'ready',
      text: 'Codex East is ready for a public coding assignment.',
      ts: '2026-06-27T16:59:55.000Z',
    },
    {
      actorRef: 'pylon.public.codex-west-2',
      blockerRefs: [],
      caveatRefs: [],
      cursor:
        '2026-06-27T16:59:45.000Z:training_window:event.public.work_claimed.6656',
      eventRef: 'event.public.work_claimed.6656',
      kind: 'work_claimed',
      refs: ['pylon.public.codex-west-2', 'issue.public.github.6656'],
      runRef: 'run.public.issue.6656',
      sourceKind: 'training_window',
      sourceRefs: ['issue.public.github.6656'],
      state: 'claimed',
      targetRef: 'issue.public.github.6656',
      text: 'Issue 6656 fleet board work claimed.',
      ts: '2026-06-27T16:59:45.000Z',
    },
    {
      blockerRefs: [],
      caveatRefs: [],
      cursor:
        '2026-06-27T16:59:30.000Z:training_verification:event.public.verification_queued.6656',
      eventRef: 'event.public.verification_queued.6656',
      kind: 'verification_queued',
      refs: ['verification.public.issue.6656'],
      runRef: 'run.public.issue.6656',
      sourceKind: 'training_verification',
      sourceRefs: ['verification.public.issue.6656'],
      state: 'queued',
      targetRef: 'verification.public.issue.6656',
      text: 'Issue 6656 public verification queued.',
      ts: '2026-06-27T16:59:30.000Z',
    },
  ],
  generatedAt: '2026-06-27T17:00:00.000Z',
  nextCursor: null,
  range: {
    filterKinds: [],
    from: '2026-06-27T16:59:00.000Z',
    limit: 60,
    since: null,
    to: '2026-06-27T17:00:00.000Z',
  },
  schemaVersion: 'openagents.public_activity_timeline.v1',
  sourceLag: [
    {
      blockerRefs: [],
      caveatRefs: [],
      lagSeconds: 0,
      latestSourceEventAt: '2026-06-27T16:59:55.000Z',
      maxStalenessSeconds: 300,
      observedAt: '2026-06-27T17:00:00.000Z',
      sourceKind: 'pylon_presence',
      sourceRefs: ['pylon.public.codex-east-1'],
      status: 'current',
    },
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['public_activity_timeline_read'],
  },
}

const loadedArtanisModel = () => {
  const [withGoal] = update(
    init(PublicAgentRoute({ agentRef: 'artanis' })),
    SucceededLoadPublicAgentGoal({
      agentRef: 'artanis',
      response: {
        agentId: 'agent_artanis',
        events: [],
        goal: {
          id: 'goal_artanis',
          agentId: 'agent_artanis',
          objective: 'Drive the public Khala improvement loop.',
          status: 'active',
          currentRunId: 'run_artanis',
          tokenBudget: null,
          tokensUsed: 1,
          timeUsedSeconds: 1,
          remainingTokens: null,
          createdAt: '2026-06-27T00:00:00.000Z',
          updatedAt: '2026-06-27T17:00:00.000Z',
          completedAt: null,
          publicUrl: '/artanis',
        },
      },
    }),
  )

  const [withHistory] = update(
    withGoal,
    SucceededLoadPublicKhalaTokensServedHistory({ history: sampleHistory }),
  )

  const [withPylons] = update(
    withHistory,
    SucceededLoadPublicPylonStats({ stats: samplePylonStats }),
  )

  const [withTimeline] = update(
    withPylons,
    SucceededLoadPublicActivityTimeline({ envelope: sampleActivityTimeline }),
  )

  return withTimeline
}

describe('public Artanis Pulse panel', () => {
  test('renders aggregate token-burn sparkline and daily pace targets', () => {
    const markup = JSON.stringify(
      PublicAgent.view(loadedArtanisModel(), 'artanis'),
    )

    expect(markup).toContain('The Pulse')
    expect(markup).toContain('Live token burn')
    expect(markup).toContain('Recent daily token burn sparkline')
    expect(markup).toContain('Behind 4x floor')
    expect(markup).toContain('Today 100M')
    expect(markup).toContain('Projected 200M')
    expect(markup).toContain('Yesterday 328.1M')
    expect(markup).toContain('Daily target')
    expect(markup).toContain('10x yesterday / 4x floor 1.3B')
    expect(markup).toContain('3% of 10x')
    expect(markup).toContain('Gap 1.1B')
    expect(markup).toContain('2026-06-27: 100,000,000 tokens')
    expect(markup).toContain('no user, prompt, or provider rows exposed')
    expect(markup).toContain('artanis-fleet-map-task-board')
    expect(markup).toContain('Fleet map')
    expect(markup).toContain('Pylons, slots, active tasks')
    expect(markup).toContain('codex-east-1')
    expect(markup).toContain('codex-west-2')
    expect(markup).toContain('assignment-ready')
    expect(markup).toContain('3 online / 2 wallet-ready / 2 assignment-ready')
    expect(markup).toContain('Active Task Board')
    expect(markup).toContain('Ready')
    expect(markup).toContain('Claimed')
    expect(markup).toContain('Verifying')
    expect(markup).toContain('Issue 6656 fleet board work claimed.')
    expect(markup).toContain(
      'https://github.com/OpenAgentsInc/openagents/issues/6656',
    )
    expect(markup).toContain('Issue 6656 public verification queued.')
    expect(markup).toContain('Only public activity rows are shown')
    expect(markup).toContain('artanis-virtual-merge-queue')
    expect(markup).toContain('Virtual merge queue')
    expect(markup).toContain('Projected branch base for parallel agents')
    expect(markup).toContain('Actual head')
    expect(markup).toContain('Virtual head')
    expect(markup).toContain('Next branch base')
    expect(markup).toContain('Conflict lane')
    expect(markup).toContain('24 accepted / 0 conflicts')
    expect(markup).toContain('Public-safe only')
    expect(markup).not.toContain('accountRef')
    expect(markup).not.toContain('raw prompt')
    expect(markup).not.toContain('diff --git')
    expect(markup).not.toContain('/Users/')
  })
})
