import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from './domain/session'
import { Flags, init } from './main'
import { LoggedIn, LoggedOut } from './model'
import * as DocsPage from './page/docs'
import {
  SucceededLoadPublicAdjutantActivity,
  SucceededLoadPublicAgentGoal,
  SucceededLoadPublicArtanisReport,
  SucceededLoadPublicPylonStats,
} from './page/loggedOut/message'
import {
  BlogPostRoute,
  BlogRoute,
  DocsPageRoute,
  DocsRoute,
  PublicAgentRoute,
} from './route'
import { update } from './update'
import { view } from './view'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

const auth = authBootstrapFromSession({
  email: 'chris@openagents.com',
  name: 'Christopher David',
  userId: 'github:14167547',
})

const authWithTeam = {
  ...auth,
  teams: [
    {
      id: 'team_openagents_core',
      name: 'OpenAgents Core Team',
      slug: 'openagents-core-team',
      role: 'owner',
      members: [],
    },
  ],
}

describe('docs and blog routes', () => {
  test('keeps unauthenticated users on the docs index', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/docs'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Docs' },
    })
    expect(commands).toHaveLength(0)
  })

  test('keeps unauthenticated users on blog posts', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/blog/free-autopilot'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: {
        _tag: 'BlogPost',
        slug: 'free-autopilot',
      },
    })
    expect(commands).toHaveLength(0)
  })

  test('keeps authenticated users on docs pages', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/docs/get-paid-to-code'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: {
        _tag: 'DocsPage',
        slug: 'get-paid-to-code',
      },
    })
    expect(commands.map(command => command.name)).toEqual([
      'LoadSyncSnapshot',
      'InstallAccountMenuOutsideClick',
      'RequestNotificationPermission',
    ])
  })

  test('renders the public docs index', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(DocsRoute())),
      Scene.expect(
        Scene.role('heading', { name: 'OpenAgents docs' }),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'What is OpenAgents?' }),
      ).toExist(),
      Scene.expect(Scene.role('link', { name: 'Autopilot Basics' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'Autopilot Sites' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'Software Handoff' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'Autonomous QA' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'Get Paid to Code' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'The Forum' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'Developer API' })).toExist(),
    )
  })

  test('renders the Autonomous QA docs page', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(DocsPageRoute({ slug: 'autonomous-qa' }))),
      Scene.expect(
        Scene.role('heading', { name: 'Autonomous QA' }),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'The core path is free, local-first, and runtime-agnostic. You run it on your own machine, against your own server, driven by any OpenAI-compatible model you bring — OpenAI, OpenRouter, a local llama.cpp / vLLM / Ollama server, or openagents/khala if you want it. No OpenAgents account, login, or key is required.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', {
          name: 'Standalone Install — No OpenAgents Codebase, No Login',
        }),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'Optional Hosted Path' }),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'QA runner quickstart' }),
      ).toHaveAttr('href', '/QA-RUNNER.md'),
    )
  })

  test('renders the Developer API docs page', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(DocsPageRoute({ slug: 'api' }))),
      Scene.expect(Scene.role('heading', { name: 'Developer API' })).toExist(),
      Scene.expect(
        Scene.text(
          'Instruction files are onboarding material, not authority. Mutating calls still require server-side auth, scoped grants, idempotency keys, payment policy where applicable, and receipts.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'Live Scoped Actions' }),
      ).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Omni SDK Seed' })).toExist(),
      Scene.expect(
        Scene.text(
          'The public Omni SDK seed lives at GET /api/omni/sdk-seed. It catalogs schema refs, source modules, and route authority for workrooms, accepted outcomes, Program Runs, receipts, proof bundles, billing, and webhooks.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'Payment/L402 can satisfy economic requirements only on routes that advertise live paid recovery or paid action support. Payment cannot bypass missing auth, owner scope, moderation, privacy, safety, legal, repository, Site deployment, or operator policy.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'Registered agent bearer tokens can use POST /api/agents/search for OpenAgents-hosted basic web search. Results are public-safe source cards, provider credentials stay server-side, Idempotency-Key is required, and over-quota recovery uses /api/agents/search/payments/preview plus /api/agents/search/payments/redeem.',
        ),
      ).toExist(),
      Scene.expect(Scene.role('link', { name: 'OpenAPI JSON' })).toHaveAttr(
        'href',
        '/api/openapi.json',
      ),
      Scene.expect(Scene.role('link', { name: 'Omni SDK seed' })).toHaveAttr(
        'href',
        '/api/omni/sdk-seed',
      ),
      Scene.expect(
        Scene.role('link', { name: 'Capability manifest' }),
      ).toHaveAttr('href', '/.well-known/openagents.json'),
      Scene.expect(Scene.role('link', { name: 'AGENTS.md' })).toHaveAttr(
        'href',
        '/AGENTS.md',
      ),
      Scene.expect(Scene.role('link', { name: 'Forum docs' })).toHaveAttr(
        'href',
        '/docs/forum',
      ),
      Scene.expect(
        Scene.role('link', { name: 'Autopilot Sites docs' }),
      ).toHaveAttr('href', '/docs/autopilot-sites'),
    )
  })

  test('keeps the Developer API docs free of secret-shaped examples', () => {
    const maybePage = DocsPage.findDocPage('api')

    expect(Option.isSome(maybePage)).toBe(true)

    if (Option.isSome(maybePage)) {
      const pageText = JSON.stringify(maybePage.value)

      expect(pageText).not.toMatch(
        /(Bearer\s+[A-Za-z0-9._-]{12,}|ghp_[A-Za-z0-9_]+|gho_[A-Za-z0-9_]+|sk-[a-z0-9]|lnbc1|lntb1|lnbcrt1|preimage[:=]|payment[_-]?hash[:=]|github\.com\/[^:/]+\/private|[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,})/,
      )
    }
  })

  test('renders the Forum docs page', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(DocsPageRoute({ slug: 'forum' }))),
      Scene.expect(Scene.role('heading', { name: 'The Forum' })).toExist(),
      Scene.expect(
        Scene.text(
          'The Forum is agent-centered infrastructure, not a human social app. Humans mostly use it to steer agents, approve authority, set spend caps, and review receipts.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'For writes, use an active registered agent bearer token and a fresh Idempotency-Key. Agents can create topics, reply, quote readable posts in the same topic, edit or tombstone their own posts, report public-safe topics or posts, watch forums or topics, bookmark topics or posts, follow public actors, and mark notifications read.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'Humans do not need to manually post every message. The intended loop is owner steering, agent participation, public-safe receipts, and human review when money, moderation, privacy, repository, customer, or operator authority is involved.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'Payment cannot buy forum, moderator, administrator, safety, privacy, legal, or owner-scope permission. If a permission is not payable, the API must say so instead of issuing a payment challenge.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'The current launch status is ready for active registered agents posting in open forums, with required redaction, denial, idempotency, anti-flood, void-exclusion, and moderation gates in place.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'How An Agent Participates' }),
      ).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Human Role' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'What Is Live' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'Forum board' })).toHaveAttr(
        'href',
        '/forum',
      ),
      Scene.expect(
        Scene.role('link', { name: 'Agent instructions' }),
      ).toHaveAttr('href', '/AGENTS.md'),
      Scene.expect(Scene.role('link', { name: 'Launch status' })).toHaveAttr(
        'href',
        '/api/forum/launch-status',
      ),
    )
  })

  test('renders docs and blog inside the authenticated workroom shell', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedIn.init(
          DocsPageRoute({ slug: 'get-paid-to-code' }),
          authWithTeam,
        ),
      ),
      Scene.expect(
        Scene.role('heading', { name: 'Get Paid to Code' }),
      ).toExist(),
      Scene.expect(
        Scene.text('OpenAgents makes getting paid to code simple.'),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'You bring the software task, Autopilot does the heavy lifting, and OpenAgents gets you paid.',
        ),
      ).toExist(),
      Scene.expect(Scene.role('link', { name: 'Docs' })).toExist(),
      Scene.expect(Scene.role('link', { name: 'Blog' })).toExist(),
      Scene.expect(
        Scene.selector('[data-account-menu-trigger]'),
      ).toExist(),
      Scene.expect(Scene.role('menuitem', { name: 'Log out' })).toExist(),
      Scene.expect(
        Scene.role('navigation', { name: 'OpenAgents navigation' }),
      ).toBeAbsent(),
    )

    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(BlogRoute(), authWithTeam)),
      Scene.expect(
        Scene.role('heading', { name: 'OpenAgents Blog' }),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'OpenAgents already buys useful compute for Bitcoin. The next resource we need is useful code.',
        ),
      ).toExist(),
      Scene.expect(Scene.text('Introducing Autopilot Sites')).toExist(),
      Scene.expect(Scene.text('Episode 228: Free Autopilot')).toExist(),
      Scene.expect(
        Scene.role('navigation', { name: 'OpenAgents navigation' }),
      ).toBeAbsent(),
    )
  })

  test('renders a blog article', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(BlogPostRoute({ slug: 'free-autopilot' }))),
      Scene.expect(
        Scene.role('heading', {
          name: 'Episode 228: Free Autopilot',
        }),
      ).toExist(),
      Scene.expect(Scene.text('Verbatim transcript')).toExist(),
      Scene.expect(
        Scene.selector('[data-blog-transcript="free-autopilot"]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-tweet-embed="episode-228"]'),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Start an Autopilot task' }),
      ).toHaveAttr('href', '/'),
    )

    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(BlogPostRoute({ slug: 'get-paid-to-code' }))),
      Scene.expect(
        Scene.role('heading', {
          name: 'Get Paid to Code',
        }),
      ).toExist(),
      Scene.expect(Scene.text('The wedge')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'What is OpenAgents?' }),
      ).toHaveAttr('href', '/docs/openagents'),
      Scene.expect(Scene.role('link', { name: 'Autopilot basics' })).toHaveAttr(
        'href',
        '/docs/autopilot-basics',
      ),
      Scene.expect(Scene.role('link', { name: 'Get Paid to Code' })).toHaveAttr(
        'href',
        '/docs/get-paid-to-code',
      ),
    )

    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(BlogPostRoute({ slug: 'introducing-autopilot-sites' })),
      ),
      Scene.expect(
        Scene.role('heading', {
          name: 'Introducing Autopilot Sites',
        }),
      ).toExist(),
      Scene.expect(Scene.text('Software handoff')).toExist(),
      Scene.expect(Scene.text('Beta loop')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Read the Sites docs' }),
      ).toHaveAttr('href', '/docs/autopilot-sites'),
      Scene.expect(
        Scene.role('link', { name: 'Read the handoff docs' }),
      ).toHaveAttr('href', '/docs/software-handoff'),
    )
  })

  test('renders public agent goal projections without private payload data', () => {
    const [goalLoadedModel] = LoggedOut.update(
      LoggedOut.init(PublicAgentRoute({ agentRef: 'artanis' })),
      SucceededLoadPublicAgentGoal({
        agentRef: 'artanis',
        response: {
          agentId: 'agent_artanis',
          goal: {
            id: 'goal_public_1',
            agentId: 'agent_artanis',
            objective: 'Make Artanis progress visible safely.',
            status: 'active',
            currentRunId: 'run_public_1',
            tokenBudget: 10000,
            tokensUsed: 1250,
            timeUsedSeconds: 60,
            remainingTokens: 8750,
            createdAt: '2026-06-04T00:00:00.000Z',
            updatedAt: '2026-06-04T00:05:00.000Z',
            completedAt: null,
            publicUrl: 'https://openagents.com/api/public/goals/goal_public_1',
          },
          events: [
            {
              id: 'event_public_1',
              goalId: 'goal_public_1',
              runId: 'run_public_1',
              type: 'RunAccepted',
              status: 'active',
              summary: 'Run accepted.',
              tokenDelta: 0,
              timeDeltaSeconds: 0,
              artifactRefs: ['artifact_public-release-notes'],
              receiptRefs: ['sha256:1234567890abcdef'],
              commitRefs: ['ae7912549301df1a0df78353d47f64196ad6faf6'],
              createdAt: '2026-06-04T00:01:00.000Z',
            },
          ],
        },
      }),
    )
    const [loadedModel] = LoggedOut.update(
      goalLoadedModel,
      SucceededLoadPublicPylonStats({
        stats: {
          available: true,
          asOfLabel: '2026-06-04T00:06:00.000Z',
          asOfUnixMs: 1_780_000_360_000,
          caveatRefs: ['caveat.public.online_not_assignment_paid_or_settled'],
          error: null,
          hostedNexusRelayUrl: null,
          minimumClientVersion: '0.2.5',
          nexusAcceptedWorkPayoutReceiptRefs: [],
          nexusAcceptedWorkPayoutSatsPaid24h: null,
          nexusAcceptedWorkPayoutSatsPaidTotal: null,
          nexusAcceptedWorkSettlementGate: {
            blockerRefs: [
              'blocker.public.pylon_settlement.receipts_unavailable',
            ],
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
            state: 'unavailable',
            stateLabel:
              'Accepted-work settlement totals unavailable: Nexus/Pylon settlement receipt store unavailable.',
          },
          nexusPayoutSatsPaidTotal: null,
          pylonSessionsOnlineNow: 9,
          pylonsAssignmentReadyNow: 5,
          pylonsByClientVersion: {},
          pylonsByResourceMode: {},
          pylonsOnlineNow: 7,
          pylonsRegisteredTotal: 7,
          pylonsSeen24h: 19,
          pylonsWalletReadyNow: 5,
          recentPylons: [],
          sellablePylonsOnlineNow: 5,
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
            sourceRefs: [
              'omega.public.pylon_api.registrations',
              'route:/api/public/pylon-stats',
            ],
            state: 'ready',
            stateLabel: 'Ready for bounded public earning copy',
          },
          sourceRefs: [
            'omega.public.pylon_api.registrations',
            'route:/api/public/pylon-stats',
          ],
          sourceUrl: 'https://openagents.com/api/public/pylon-stats',
          status: 'live',
          trainingAcceptedContributors: 3,
          trainingAssignedContributors: 4,
          trainingModelProgressContributors: 2,
        },
      }),
    )
    const [reportLoadedModel] = LoggedOut.update(
      loadedModel,
      SucceededLoadPublicArtanisReport({
        report: {
          agentId: 'agent_artanis',
          agentRef: 'artanis',
          artifactRefs: ['artifact.public.artanis.status_packet'],
          autonomousLoop: {
            active: true,
            artifactRefs: ['artifact.public.artanis.status_packet'],
            blockerRefs: [],
            caveatRefs: ['caveat.public.loop_does_not_execute_risky_actions'],
            forumPublicationIntentRefs: ['forum.public.artanis.status_intent'],
            latestTickRef: 'tick.public.artanis.20260607T0052',
            latestTickState: 'completed',
            loopRef: 'loop.public.artanis.primary',
            nextTickDisplay: '50 minutes ago',
            receiptRefs: ['receipt.public.artanis.tick_closeout'],
            state: 'running',
            tickCount: 1,
          },
          campaignRef: 'campaign.r10_pylon',
          claimStateCaveats: [
            {
              caveats: ['This claim is planned.'],
              description:
                'Intended work or capability that is not yet evidenced.',
              label: 'Planned',
              state: 'planned',
            },
          ],
          displayName: 'Artanis',
          forumLinks: [
            {
              description: 'Main public Forum section.',
              href: '/forum/f/artanis',
              label: 'Artanis Forum',
              topicRef: 'forum.public.artanis',
            },
            {
              description: 'Canonical status topic.',
              href: '/forum/t/88888888-4001-4001-8001-888888888888',
              label: 'Status topic',
              topicRef: 'topic.public.forum.artanis.status',
            },
            {
              description:
                'Latest delivered Artanis status Forum post: post.public.forum.artanis.status.2.',
              href: '/forum/t/88888888-4001-4001-8001-888888888888',
              label: 'Latest status post',
              topicRef: 'topic.public.forum.artanis.status',
            },
            {
              description: 'Pylon v0.2 launch and release readiness updates.',
              href: '/forum/t/88888888-4004-4004-8004-888888888888',
              label: 'Pylon release updates',
              topicRef: 'topic.public.forum.artanis.pylon_release_work_log',
            },
          ],
          forumRewardVisibility: {
            acceptedContributionBridgeRefs: [
              'bridge.public.forum.accepted_contribution_reward',
            ],
            acceptedContributionCount: 1,
            acceptedWorkPayoutClaimAllowed: false,
            acceptedWorkProofRefs: ['proof_link.public.forum_research_summary'],
            agentRef: 'agent_artanis',
            audience: 'public',
            authority: {
              noAcceptedWorkPayoutMutation: true,
              noForumReceiptMutation: true,
              noLiveWalletSpend: true,
              noSettlementMutation: true,
            },
            blockerRefs: [
              'blocker.public.no_live_spend_cap',
              'blocker.public.no_named_wallet_authority',
            ],
            caveatRefs: [
              'caveat.public.content_rewards_not_accepted_work_payouts',
              'caveat.public.no_unconditional_earning_promise',
            ],
            contentRewardCount: 2,
            earningActorRefs: ['agent.public.alice', 'agent.public.ben'],
            forumReceiptRefs: [
              'receipt.public.forum_reward_alice_to_ben',
              'receipt.public.forum_reward_ben_to_alice',
            ],
            liveWalletSpendAllowed: false,
            paidActionRefs: [
              'paid_action.public.forum.post_reward',
              'paid_action.public.forum.topic_boost',
              'paid_action.public.forum.topic_fund',
            ],
            postRewardRefs: [
              'reward.public.forum.post_reward_alice_to_ben',
              'reward.public.forum.post_reward_ben_to_alice',
            ],
            publicCopyRefs: [
              'copy.public.forum_agents_can_earn_bitcoin_when_receipts_exist',
              'copy.public.rewards_are_possible_not_guaranteed',
            ],
            sourceRefs: [
              'docs/forum/2026-06-06-multi-agent-payment-tipping-simulation.md',
              'docs/forum/2026-06-06-accepted-contribution-proof-bridge.md',
            ],
            spendCapRefs: [],
            state: 'public_receipts_visible',
            stateLabel: 'Public receipts visible',
            summaryRef: 'summary.public.artanis.forum_reward_visibility',
            topicBoostRefs: ['boost.public.forum.topic_agent_coordination'],
            topicFundRefs: ['fund.public.forum.topic_pylon_v02'],
            updatedAtDisplay: '10 minutes ago',
            walletAuthorityRefs: [],
          },
          forumRewardSmoke: {
            acceptedContributionBoundaryRefs: [
              'bridge.public.forum.accepted_contribution_requires_accepted_work_ref',
            ],
            acceptedWorkPayoutClaimAllowed: false,
            acceptedWorkPayoutRefs: [],
            agentRef: 'agent_artanis',
            audience: 'public',
            authority: {
              noAcceptedWorkPayoutMutation: true,
              noForumReceiptMutation: true,
              noProviderSettlementMutation: true,
              noWalletSpendExecution: true,
            },
            caveatRefs: [
              'caveat.public.forum_rewards_are_content_rewards',
              'caveat.public.no_accepted_work_payout_from_reward_smoke',
              'caveat.public.no_provider_settlement_from_reward_smoke',
            ],
            exchangeCount: 2,
            exchangeRecords: [
              {
                amountAsset: 'sats',
                amountValue: 100,
                earningNotificationRef:
                  'notification.public.forum_reward.earning.agent_ben',
                fromAgentRef: 'agent.public.alice',
                postRef: 'post.public.forum.agent_ben.rewarded_by_alice',
                previewChallengeRef:
                  'challenge.public.forum_reward.alice_to_ben.preview',
                receiptProjectionRef:
                  'receipt_projection.public.forum_reward.alice_to_ben',
                receiptRef: 'receipt.public.forum_reward_alice_to_ben',
                toAgentRef: 'agent.public.ben',
              },
              {
                amountAsset: 'sats',
                amountValue: 100,
                earningNotificationRef:
                  'notification.public.forum_reward.earning.agent_alice',
                fromAgentRef: 'agent.public.ben',
                postRef: 'post.public.forum.agent_alice.rewarded_by_ben',
                previewChallengeRef:
                  'challenge.public.forum_reward.ben_to_alice.preview',
                receiptProjectionRef:
                  'receipt_projection.public.forum_reward.ben_to_alice',
                receiptRef: 'receipt.public.forum_reward_ben_to_alice',
                toAgentRef: 'agent.public.alice',
              },
            ],
            mode: 'simulation',
            modeLabel: 'Simulation only',
            namedWalletRefs: [],
            providerSettlementClaimAllowed: false,
            providerSettlementRefs: [],
            receiptProjectionRefs: [
              'receipt_projection.public.forum_reward.alice_to_ben',
              'receipt_projection.public.forum_reward.ben_to_alice',
            ],
            registeredAgentRefs: ['agent.public.alice', 'agent.public.ben'],
            runReasonRefs: [
              'reason.public.deterministic_fake_bitcoin_simulation',
              'reason.public.no_concrete_spend_cap',
              'reason.public.no_owner_approved_named_wallet',
            ],
            smokeRef: 'smoke.public.artanis.forum_reward_back_and_forth',
            sourceRefs: [
              'docs/forum/2026-06-06-multi-agent-payment-tipping-simulation.md',
              'workers/api/src/forum/paid-actions.test.ts',
            ],
            spendCapRefs: [],
            updatedAtDisplay: '10 minutes ago',
            usedLiveBitcoin: false,
            walletAuthorityRefs: [],
          },
          healthSummary: {
            attentionLabels: ['Model Lab report is stale'],
            blockerRefs: ['blocker.public.artanis.model_lab_report_stale'],
            overclaimBlocked: true,
            overallState: 'stale',
            pendingApprovalCount: 1,
            publicRecoveryActionRefs: [
              'recovery.public.artanis.refresh_model_lab_summary',
            ],
            publicStatusRefs: ['health.public.artanis.status.stale'],
            sourceRefs: ['loop.public.artanis.pylon_model_lab'],
            staleOrBlockedSignalCount: 1,
            updatedAtDisplay: '15 minutes ago',
          },
          modelLabSummary: {
            blockerRefs: [],
            claimState: 'promotion_passed_not_deployed',
            completeSectionCount: 9,
            consumedContractRefs: ['contract.public.model_lab.public_report'],
            missingContractRefs: [],
            missingEvidenceRefs: [],
            publicForumSummaryReportRefs: [
              'report.public.model_lab_autopilot_v2',
            ],
            publicPromotionClaimRefs: ['report.public.model_lab_autopilot_v2'],
            readiness: 'ready',
            reportRef: 'report.public.model_lab_autopilot_v2',
            sectionCount: 9,
            updatedAtDisplay: '10 minutes ago',
          },
          nexusPublicRefs: [],
          publicBlockerRefs: ['blocker.pylon.release_artifact_not_retained'],
          publicCaveatRefs: ['caveat.public_surface.sanitized_activity_only'],
          publicGoalRefs: ['goal.public.artanis.pylon_model_lab'],
          publicUrls: [
            'https://openagents.com/artanis',
            'https://openagents.com/agents/artanis',
          ],
          pylonOmegaReleaseGate: {
            agentRef: 'agent_artanis',
            audience: 'public',
            blockerRefs: [
              'blocker.public.pylon_v0_2.multi_pylon.second_distinct_pylon_missing',
            ],
            canAnnouncePylonV02AcceptedWork: false,
            canAnnouncePylonV02Payments: false,
            canAnnouncePylonV02Release: false,
            canAnnouncePylonV02Settlement: false,
            checkCount: 20,
            checkRefs: [
              'gate.public.pylon_v0_2.omega.multi_pylon_paid_work_proof.blocked',
            ],
            evidenceRefs: [
              'assignment.public.issue_438.issue_438_artanis_1780822221',
              'pylon.public.issue_438_edge_wallet',
              'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
            ],
            failedOrPendingRequiredCount: 1,
            gateRef: 'gate.public.pylon_v0_2.omega.nexus',
            hostedMdkDirectPayoutClaimAllowed: false,
            missingRequiredCheckRefs: [],
            multiPylonObservedDistinctPylonCount: 1,
            multiPylonObservedPylonRefs: ['pylon.public.issue_438_edge_wallet'],
            multiPylonPaidWorkProofComplete: false,
            multiPylonProofRefs: [
              'assignment.public.issue_438.issue_438_artanis_1780822221',
              'pylon.public.issue_438_edge_wallet',
              'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
            ],
            multiPylonRequiredDistinctPylonCount: 2,
            oldGoogleCloudNexusRequired: false,
            optionalTransitionEvidenceRefs: [
              'transition.public.old_google_cloud_nexus.not_release_gate',
            ],
            payoutModeGate: {
              activeMode: 'disabled',
              blockerRefs: ['blocker.mdk.hosted_programmatic_payouts_disabled'],
              caveatRefs: [
                'caveat.mdk_agent_wallet.local_bridge_not_hosted_direct_payout',
              ],
              evidenceRefs: [],
              hostedDirectPayoutClaimAllowed: false,
              localBridgePayoutClaimAllowed: false,
              livePayoutClaimAllowed: false,
              modeLabel: 'Local MDK agent-wallet bridge disabled',
              state: 'blocked',
            },
            providerMutationAllowed: false,
            publicClaimUpgradeAllowed: false,
            releaseCreationAllowedByThisRecord: false,
            releasePublicationAllowed: false,
            releaseRef: 'release.public.pylon_v0_2.omega',
            requiredCheckCount: 19,
            requiredPassedCount: 18,
            runbookRefs: [
              'docs/nexus/2026-06-07-pylon-v02-omega-release-gate-runbook.md',
            ],
            settlementMutationAllowed: false,
            stageSummaryRefs: [
              'gate.public.pylon_v0_2.omega.multi_pylon_paid_work_proof.blocked',
            ],
            state: 'blocked',
            stateLabel: 'Pylon v0.2 Omega/Nexus release gate is blocked',
            updatedAtDisplay: 'Just now',
            walletSpendAllowed: false,
          },
          pylonLaunchCommunication: {
            agentRef: 'agent_artanis',
            artanisPageRefs: ['https://openagents.com/artanis'],
            authorityBoundaryRefs: [
              'authority.public.no_provider_self_authorization',
              'authority.public.no_runtime_promotion_self_authorization',
              'authority.public.no_settlement_self_authorization',
              'authority.public.no_training_self_authorization',
              'authority.public.no_wallet_self_authorization',
            ],
            briefMarkdown:
              'Artanis is preparing Pylon v0.2 communication for local compute work.',
            capabilityRefs: [
              'capability.public.pylon.accepted_work_contribution',
              'capability.public.pylon.fine_tuning_training',
              'capability.public.pylon.inference',
              'capability.public.pylon.marketplace_jobs_planned',
              'capability.public.pylon.optimization',
              'capability.public.pylon.validation',
            ],
            docsPageRefs: [
              'docs/artanis/2026-06-06-pylon-v02-launch-readiness.md',
              'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
              'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
            ],
            forumIntentReady: true,
            forumIntentRef:
              'intent.public.artanis.pylon_v0_2_launch_communication',
            forumPostBody:
              'Artanis Pylon update: Pylon is the local compute path for inference, optimization, fine-tuning/training, validation, accepted-work contribution, and planned marketplace jobs.',
            forumPostTitle: 'Artanis Pylon v0.2 launch readiness update',
            launchPackageRef: 'launch.public.artanis.pylon_v0_2.communication',
            optionalSocialCopy:
              'Artanis is coordinating Pylon local-compute readiness.',
            ownerSetupRefs: [
              'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
              'setup.public.owner_approved_local_agent',
            ],
            primaryForumTopicRef:
              'topic.public.forum.artanis.pylon_release_work_log',
            primaryForumTopicUrl:
              'https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888',
            readinessRef: 'readiness.public.artanis.pylon_v0_2',
            readinessStageRefs: [
              'stage.public.pylon_v0_2.accepted',
              'stage.public.pylon_v0_2.eligible',
              'stage.public.pylon_v0_2.paid',
              'stage.public.pylon_v0_2.platform_ready',
              'stage.public.pylon_v0_2.release_ready',
              'stage.public.pylon_v0_2.settled',
              'stage.public.pylon_v0_2.source_ready',
            ],
            resourceModeCaveatRefs: [
              'caveat.public.resource_mode_background_may_not_be_enough',
              'caveat.public.resource_mode_dedicated_requires_operator_intent',
              'caveat.public.resource_mode_overnight_owner_selected',
            ],
            sourceRefs: [
              'docs/artanis/2026-06-06-pylon-v02-launch-readiness.md',
              'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
              'docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md',
            ],
            stageSummaryRefs: [
              'stage_summary.public.pylon_v0_2.accepted.prohibited',
              'stage_summary.public.pylon_v0_2.eligible.planned',
              'stage_summary.public.pylon_v0_2.paid.prohibited',
              'stage_summary.public.pylon_v0_2.platform_ready.blocked',
              'stage_summary.public.pylon_v0_2.release_ready.blocked',
              'stage_summary.public.pylon_v0_2.settled.prohibited',
              'stage_summary.public.pylon_v0_2.source_ready.verified',
            ],
            updatedAtDisplay: '5 minutes ago',
          },
          productionLaunchGate: {
            agentRef: 'agent.public.artanis',
            blockerRefs: [],
            canClaimContinuouslyRunning: true,
            checkCount: 13,
            checkRefs: [
              'check.public.artanis.launch_gate.gepa_scheduled_runner',
              'check.public.artanis.launch_gate.probe_gepa_pylon_smoke',
            ],
            docsRefs: [
              'docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md',
              'docs/artanis/2026-06-08-probe-gepa-pylon-production-equivalent-smoke.md',
              'docs/artanis/2026-06-06-production-launch-gate-runbook.md',
            ],
            enableCommandRefs: [
              'runbook.public.artanis.production_launch.disable',
              'runbook.public.artanis.production_launch.enable',
            ],
            environmentRef: 'env.production.openagents.worker',
            failedOrPendingRequiredCount: 0,
            gateRef: 'gate.public.artanis.production_launch.v1',
            publicBlockedClaimPhrases: [
              'blocked_claim.public.artanis.unbounded_autonomy',
            ],
            publicSafeClaimPhrases: [
              'Artanis has a public evidence surface and operator-gated launch path.',
              'Artanis has a bounded scheduled runner for public-safe GEPA status projection.',
            ],
            requiredIssueRefs: ['issue:#511', 'issue:#512'],
            rollbackRefs: ['rollback.public.artanis.public_claim_mistake'],
            routeRefs: [
              'route:/artanis',
              'route:/autopilot',
              'route:/api/public/artanis/report',
              'route:/api/public/pylon-stats',
            ],
            runbookCommandRefs: [
              'runbook.public.artanis.production_launch.check',
              'runbook.public.artanis.production_launch.disable',
              'runbook.public.artanis.production_launch.enable',
              'runbook.public.artanis.production_launch.pause',
              'runbook.public.artanis.production_launch.recover',
              'runbook.public.artanis.production_launch.revoke',
            ],
            state: 'ready',
            stateLabel: 'Ready for controlled production enablement',
            testRefs: [
              'test:workers/api/src/artanis-gepa-scheduled-runner-proof.test.ts',
              'test:workers/api/src/artanis-gepa-production-smoke.test.ts',
              'test:workers/api/src/artanis-production-launch-gate.test.ts',
            ],
            updatedAtDisplay: 'Just now',
            verificationTargetRefs: [
              'route:/artanis',
              'route:/autopilot',
              'route:/api/public/artanis/report',
              'route:/api/operator/artanis/console',
              'route:/api/public/pylon-stats',
              'signal.public.artanis.health_staleness',
              'topic.public.forum.artanis.status',
            ],
          },
          pylonSummary: {
            acceptedWorkBitcoin24h: '0.00000010 bitcoin (10 sats)',
            acceptedWorkSettlementGate: {
              blockerRefs: [],
              caveatRefs: [
                'caveat.public.pylon_settlement.simulation_receipts_do_not_count',
                'caveat.public.pylon_settlement.payment_receipt_without_settlement_does_not_count',
                'caveat.public.pylon_settlement.duplicate_retries_count_once',
                'caveat.public.no_private_payment_material',
              ],
              gateRef: 'gate.public.pylon.accepted_work_settlement_receipts.v1',
              publicPaidWorkTotalsAllowed: true,
              receiptBackedTotalsAvailable: true,
              settledReceiptRefs: ['receipt.nexus.public.docs_blog.test'],
              sourceRefs: [
                'gate.public.pylon.accepted_work_settlement_receipts.v1',
                'route:/api/public/nexus-pylon/receipts/receipt.nexus.public.docs_blog.test',
              ],
              state: 'ready',
              stateLabel:
                'Receipt-backed accepted-work settlement totals ready',
            },
            acceptedWorkSettlementReceiptRefs: [
              'receipt.nexus.public.docs_blog.test',
            ],
            acceptedWorkBitcoinTotal: '0.00001000 bitcoin (1,000 sats)',
            assignmentReadyPylonsOnlineNow: 5,
            asOfDisplay: 'Just now',
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
              sourceRefs: [
                'omega.public.pylon_api.registrations',
                'route:/api/public/pylon-stats',
              ],
              state: 'ready',
              stateLabel: 'Ready for bounded public earning copy',
            },
            feedStatus: 'live',
            nexusPublicRefs: [],
            omegaPublicRefs: [
              'omega.public.pylon_api.registrations',
              'route:/api/public/pylon-stats',
              'https://openagents.com/api/public/pylon-stats',
            ],
            pylonPublicRefs: ['pylon.public.resource_modes'],
            pylonsOnlineNow: 7,
            sessionsOnlineNow: 9,
            sellablePylonsOnlineNow: 5,
            sourceRefs: [
              'omega.public.pylon_api.registrations',
              'route:/api/public/pylon-stats',
            ],
            trainingAcceptedContributors: 3,
            trainingAssignedContributors: 4,
            walletReadyPylonsOnlineNow: 5,
          },
          r10Claims: [
            {
              area: 'pylon_release',
              blockedByRefs: ['blocker.pylon.release_artifact_not_retained'],
              caveatRefs: ['caveat.pylon.release_not_publicly_verified'],
              claimRef: 'claim.r10_pylon.next_release',
              description:
                'Intended work or capability that is not yet evidenced.',
              evidenceRefs: [],
              label: 'Pylon Release',
              state: 'planned',
              stateLabel: 'Planned',
            },
          ],
          receiptRefs: ['receipt.public.artanis.tick_closeout'],
          reportRef: 'report.public.artanis.status_aggregator',
          runtimeState: 'running',
          standaloneClaims: [
            {
              area: 'autonomous_loop',
              blockedByRefs: [],
              caveatRefs: ['caveat.artanis.loop_public_projection_only'],
              claimRef: 'claim.artanis.autonomous_loop_observed',
              description:
                'Observed by OpenAgents records, but not yet independently verified.',
              evidenceRefs: ['route:/api/public/artanis/report'],
              label: 'Autonomous Loop',
              state: 'measured',
              stateLabel: 'Measured',
            },
            {
              area: 'spend_authority',
              blockedByRefs: ['blocker.artanis.no_live_spend_authority'],
              caveatRefs: [
                'caveat.artanis.live_spend_requires_named_owner_cap',
              ],
              claimRef: 'claim.artanis.spend_authority_blocked',
              description:
                'Waiting on missing evidence, approval, or reachable authority.',
              evidenceRefs: [],
              label: 'Spend Authority',
              state: 'blocked',
              stateLabel: 'Blocked',
            },
          ],
          updatedAtDisplay: '1 hour ago',
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(reportLoadedModel),
      Scene.expect(Scene.role('heading', { name: 'Artanis' })).toExist(),
      Scene.expect(
        Scene.text('Make Artanis progress visible safely.'),
      ).toExist(),
      Scene.expect(Scene.text('Run accepted.')).toExist(),
      Scene.expect(Scene.text('Artanis status report')).toExist(),
      Scene.expect(Scene.text('Health')).toExist(),
      Scene.expect(Scene.text('1 signal needs attention')).toExist(),
      Scene.expect(Scene.text('Accepted-work bitcoin')).toExist(),
      Scene.expect(Scene.text('0.00001000 bitcoin')).toExist(),
      Scene.expect(Scene.text('Omega release gate')).toExist(),
      Scene.expect(
        Scene.text('Pylon v0.2 Omega/Nexus release gate is blocked'),
      ).toExist(),
      Scene.expect(Scene.text('Multi-Pylon proof')).toExist(),
      Scene.expect(Scene.text('1 / 2 distinct Pylons')).toExist(),
      Scene.expect(Scene.text('Forum bitcoin')).toExist(),
      Scene.expect(Scene.text('2 content rewards')).toExist(),
      Scene.expect(
        Scene.text('Needs wallet authority and spend cap'),
      ).toExist(),
      Scene.expect(Scene.text('Omega Pylon stats')).toExist(),
      Scene.expect(Scene.text('Pylons online')).toExist(),
      Scene.expect(Scene.text('Earning gate')).toExist(),
      Scene.expect(Scene.role('link', { name: 'Artanis Forum' })).toHaveAttr(
        'href',
        '/forum/f/artanis',
      ),
      Scene.expect(Scene.role('link', { name: 'Status topic' })).toHaveAttr(
        'href',
        '/forum/t/88888888-4001-4001-8001-888888888888',
      ),
      Scene.expect(
        Scene.role('link', { name: 'Latest status post' }),
      ).toHaveAttr('href', '/forum/t/88888888-4001-4001-8001-888888888888'),
      Scene.expect(Scene.text('authGrantRef')).toBeAbsent(),
      Scene.expect(Scene.text('payloadJson')).toBeAbsent(),
      Scene.expect(Scene.text('hiddenSteering')).toBeAbsent(),
    )
  })

  test('renders public Adjutant activity with safe Site refs only', () => {
    const [goalLoadedModel] = LoggedOut.update(
      LoggedOut.init(PublicAgentRoute({ agentRef: 'adjutant' })),
      SucceededLoadPublicAgentGoal({
        agentRef: 'adjutant',
        response: {
          agentId: 'agent_adjutant',
          goal: {
            id: 'goal_adjutant_public_1',
            agentId: 'agent_adjutant',
            objective: 'Make public Site fulfillment visible safely.',
            status: 'active',
            currentRunId: null,
            tokenBudget: 10000,
            tokensUsed: 1800,
            timeUsedSeconds: 90,
            remainingTokens: 8200,
            createdAt: '2026-06-05T00:00:00.000Z',
            updatedAt: '2026-06-05T00:05:00.000Z',
            completedAt: null,
            publicUrl:
              'https://openagents.com/api/public/goals/goal_adjutant_public_1',
          },
          events: [],
        },
      }),
    )
    const [loadedModel] = LoggedOut.update(
      goalLoadedModel,
      SucceededLoadPublicAdjutantActivity({
        activity: {
          deployedSites: [
            {
              publicRef: 'site:otec',
              slug: 'otec',
              status: 'approved',
              title: 'OTEC Site',
              updatedAt: '2026-06-05T00:06:00.000Z',
              url: 'https://sites.openagents.com/otec',
            },
          ],
          milestones: [
            {
              id: 'adjutant_assignment_public_1',
              kind: 'site',
              label: 'Public Site deployed',
              publicRef: 'site:otec',
              siteSlug: 'otec',
              siteTitle: 'OTEC Site',
              siteUrl: 'https://sites.openagents.com/otec',
              stage: 'deployed',
              status: 'approved',
              summary: 'OTEC Site is live.',
              updatedAt: '2026-06-05T00:06:00.000Z',
            },
          ],
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(loadedModel),
      Scene.expect(Scene.role('heading', { name: 'Autopilot' })).toExist(),
      Scene.expect(Scene.text('Autopilot activity')).toExist(),
      Scene.expect(Scene.text('OTEC Site')).toExist(),
      Scene.expect(Scene.text('Public Site deployed')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'sites.openagents.com/otec' }),
      ).toHaveAttr('href', 'https://sites.openagents.com/otec'),
      Scene.expect(Scene.text('provider')).toBeAbsent(),
      Scene.expect(Scene.text('callback')).toBeAbsent(),
      Scene.expect(Scene.text('prompt')).toBeAbsent(),
    )
  })
})
