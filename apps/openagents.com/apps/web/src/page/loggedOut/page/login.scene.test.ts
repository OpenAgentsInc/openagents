import { Scene } from 'foldkit'
import { describe, test } from 'vitest'

import { LoggedOut } from '../../../model'
import {
  HomeRoute,
  OnboardingRoute,
  ProductPromisesRoute,
} from '../../../route'
import { update } from '../../../update'
import { view } from '../../../view'
import {
  type PublicSettledFeedEvent,
  IdlePublicForumLaunchStatus,
  IdlePublicForumTipLeaderboards,
  IdlePublicKhalaTokensServed,
  IdlePublicKhalaTokensServedHistory,
  LoadedPublicPylonStats,
  SettledFeedModel,
  initSettledFeedModel,
} from '../model'
import * as Home from './home'

const loadedStatsFixture = {
  available: true,
  asOfLabel: 'Just now',
  asOfUnixMs: 1_780_927_200_000,
  caveatRefs: [
    'caveat.public.pylon_stats_are_registration_heartbeat_only',
  ],
  error: null,
  hostedNexusRelayUrl: null,
  minimumClientVersion: '0.2.5',
  nexusAcceptedWorkPayoutReceiptRefs: [],
  nexusAcceptedWorkPayoutSatsPaid24h: null,
  nexusAcceptedWorkPayoutSatsPaidTotal: null,
  nexusAcceptedWorkSettlementGate: {
    blockerRefs: ['blocker.public.pylon_settlement.receipts_unavailable'],
    caveatRefs: [
      'caveat.public.pylon_settlement.simulation_receipts_do_not_count',
      'caveat.public.pylon_settlement.payment_receipt_without_settlement_does_not_count',
      'caveat.public.pylon_settlement.duplicate_retries_count_once',
      'caveat.public.no_private_payment_material',
    ],
    gateRef: 'gate.public.pylon.accepted_work_settlement_receipts.v1',
    publicPaidWorkTotalsAllowed: false,
    receiptBackedTotalsAvailable: false,
    settledReceiptRefs: [],
    sourceRefs: [
      'gate.public.pylon.accepted_work_settlement_receipts.v1',
      'route:/api/public/pylon-stats',
    ],
    state: 'unavailable' as const,
    stateLabel:
      'Accepted-work settlement totals unavailable: Nexus/Pylon settlement receipt store unavailable.',
  },
  nexusPayoutSatsPaidTotal: null,
  pylonSessionsOnlineNow: 4,
  pylonsAssignmentReadyNow: 2,
  pylonsByClientVersion: { 'openagents.pylon@0.2.5': 4 },
  pylonsByResourceMode: { balanced: 4 },
  pylonsOnlineNow: 4,
  pylonsRegisteredTotal: 6,
  pylonsSeen24h: 9,
  pylonsWalletReadyNow: 3,
  publicRealSatsSettled24h: 2,
  recentPylons: [],
  sellablePylonsOnlineNow: 2,
  earningLaunchGate: {
    blockedClaimRefs: [],
    blockerRefs: [],
    caveatRefs: [
      'caveat.public.pylon_online_is_not_paid_work',
      'caveat.public.wallet_ready_is_receive_readiness_not_send_ready',
      'caveat.public.assignment_ready_is_not_acceptance_or_settlement',
      'caveat.public.no_unconditional_earning_promise',
    ],
    gateRef: 'gate.public.pylon.earning_network_counters.v1',
    publicEarningCopyAllowed: true,
    requiredAssignmentReadyPylonsPresent: true,
    requiredOnlinePylonsPresent: true,
    requiredWalletReadyPylonsPresent: true,
    sourceRefs: ['route:/api/public/pylon-stats'],
    state: 'ready' as const,
    stateLabel: 'Ready for bounded public earning copy',
  },
  sourceRefs: ['route:/api/public/pylon-stats'],
  sourceUrl: 'https://openagents.com/api/public/pylon-stats',
  status: 'live' as const,
  trainingAcceptedContributors: 0,
  trainingAssignedContributors: 0,
  trainingModelProgressContributors: 0,
}

describe('maintenance landing scene', () => {
  test('root login button exposes a disabled loading state on click', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(
        Scene.selector('[data-login-button="github"][onclick]'),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Log in with GitHub' }),
      ).toExist(),
    )
  })

  test('renders the agent network landing message', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('OpenAgents is the agent network.')).toExist(),
      Scene.expect(
        Scene.text(
          'Connect your agent, inspect the live promises, and follow the paths for earning bitcoin through useful public work.',
        ),
      ).toExist(),
      Scene.expect(Scene.text('Join the waitlist:')).not.toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Log in with GitHub' }),
      ).toExist(),
      Scene.expect(Scene.text('Episode 228: Free Autopilot')).not.toExist(),
      Scene.expect(Scene.text('Launches June 4.')).not.toExist(),
      Scene.expect(Scene.text("We'll be right back")).not.toExist(),
    )
  })

  test('renders the product promises page through the top-level public shell', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ProductPromisesRoute())),
      Scene.expect(
        Scene.role('heading', { name: 'Product promises' }),
      ).toExist(),
      Scene.expect(Scene.text('Human-readable promise ledger')).toExist(),
      Scene.expect(
        Scene.text('Autopilot is a cloud coding agent.'),
      ).not.toExist(),
    )
  })

  test('renders loaded homepage Pylon stats without payment claims', () => {
    Scene.scene(
      {
        update,
        view: () =>
          Home.view({
            forumLaunchStatus: IdlePublicForumLaunchStatus(),
            forumTipLeaderboards: IdlePublicForumTipLeaderboards(),
            publicKhalaTokensServed: IdlePublicKhalaTokensServed(),
            publicKhalaTokensServedHistory: IdlePublicKhalaTokensServedHistory(),
            publicPylonStats: LoadedPublicPylonStats({
              stats: loadedStatsFixture,
            }),
            settledFeed: initSettledFeedModel(),
          }),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      // The moved public homepage carries the headline strip; the detailed
      // panels live on /stats-old.
      Scene.expect(Scene.text('Pylons online')).toExist(),
      Scene.expect(Scene.text('Pylons seen 24h')).toExist(),
      Scene.expect(Scene.text('Tip sats paid')).toExist(),
      Scene.expect(Scene.text('Tip sats settled')).toExist(),
      Scene.expect(Scene.text('Accepted-work sats')).toExist(),
      Scene.expect(Scene.text('Detailed network stats →')).toExist(),
    )
  })

  test('renders the live settled feed and reflects a streamed settlement', () => {
    const settledEvent: PublicSettledFeedEvent = {
      amountSats: 5,
      challengeRef: 'challenge.tassadar.window.0001',
      contributorRef: 'pylon.worker.orrery',
      eventRef: 'settled.0',
      party: 'worker',
      runRef: 'run.tassadar.poc',
      settledAt: '2026-06-17T00:00:00.000Z',
      totalSettledCount: 1,
      totalSettledSats: 5,
      windowRef: 'window.tassadar.0001',
    }

    Scene.scene(
      {
        update,
        view: () =>
          Home.view({
            forumLaunchStatus: IdlePublicForumLaunchStatus(),
            forumTipLeaderboards: IdlePublicForumTipLeaderboards(),
            publicKhalaTokensServed: IdlePublicKhalaTokensServed(),
            publicKhalaTokensServedHistory: IdlePublicKhalaTokensServedHistory(),
            publicPylonStats: LoadedPublicPylonStats({
              stats: loadedStatsFixture,
            }),
            settledFeed: SettledFeedModel({
              connection: 'open',
              cursor: 1,
              events: [settledEvent],
              totalSettledCount: 1,
              totalSettledSats: 5,
            }),
          }),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('Live Settled Feed')).toExist(),
      Scene.expect(Scene.text('Settled (total)')).toExist(),
      Scene.expect(Scene.text('Settled (24h)')).toExist(),
      Scene.expect(Scene.text('Live')).toExist(),
      Scene.expect(Scene.text('worker · pylon.worker.orrery')).toExist(),
    )
  })

  test('renders the compact public agent path', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.role('heading', { name: 'I am an Agent' })).toExist(),
      Scene.expect(
        Scene.role('textbox', { name: 'Copyable agent instruction' }),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Capability manifest' }),
      ).toHaveAttr('href', '/.well-known/openagents.json'),
      Scene.expect(Scene.role('link', { name: 'OpenAPI' })).toHaveAttr(
        'href',
        '/api/openapi.json',
      ),
      Scene.expect(Scene.role('link', { name: 'Public proof' })).toHaveAttr(
        'href',
        '/api/public/proof/otec',
      ),
    )
  })

  test('renders the onboarding landing page without replacing home', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(OnboardingRoute())),
      Scene.expect(
        Scene.role('heading', { name: 'Stop Babysitting Your AI' }),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Log in with GitHub' }),
      ).toHaveAttr('href', '/login/github'),
      Scene.expect(Scene.text('signup sequence')).not.toExist(),
      Scene.expect(Scene.role('heading', { name: 'Repository' })).not.toExist(),
      Scene.expect(Scene.text('Funding amount')).not.toExist(),
    )
  })

  test('renders the onboarding funding demo after the GitHub step', () => {
    const model = LoggedOut.init(OnboardingRoute())

    Scene.scene(
      { update, view },
      Scene.with({
        ...model,
        onboarding: {
          ...model.onboarding,
          step: 'funding',
        },
      }),
      Scene.expect(
        Scene.role('heading', { name: 'Fund your account' }),
      ).toExist(),
      Scene.expect(Scene.text('Funding amount')).toExist(),
      Scene.expect(Scene.text('I have a coupon code')).toExist(),
      Scene.expect(Scene.text('Credit order')).toExist(),
      Scene.expect(Scene.text('signup sequence')).not.toExist(),
    )
  })
})
