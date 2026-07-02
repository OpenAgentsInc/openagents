import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from './domain/session'
import { Flags, init } from './main'
import { LoggedIn, LoggedOut } from './model'
import * as DocsPage from './page/docs'
import {
  SucceededLoadPublicActivityTimeline,
  SucceededLoadPublicAdjutantActivity,
  SucceededLoadPublicAgentGoal,
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
        Scene.role('link', { name: 'Khala Code + OpenAgents Overview' }),
      ).toExist(),
      Scene.expect(
        Scene.text(
          'Khala Code is the desktop coding front door for OpenAgents: your own local Codex harness, coordinated into a proof-oriented network.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Autopilot Basics' }),
      ).toBeAbsent(),
      Scene.expect(
        Scene.role('link', { name: 'Autopilot Sites' }),
      ).toBeAbsent(),
      Scene.expect(
        Scene.role('link', { name: 'Software Handoff' }),
      ).toBeAbsent(),
      Scene.expect(Scene.role('link', { name: 'Autonomous QA' })).toBeAbsent(),
      Scene.expect(
        Scene.role('link', { name: 'Get Paid to Code' }),
      ).toBeAbsent(),
      Scene.expect(Scene.role('link', { name: 'The Forum' })).toBeAbsent(),
      Scene.expect(Scene.role('link', { name: 'Developer API' })).toBeAbsent(),
    )
  })

  test('renders the Khala Code overview docs page', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(DocsPageRoute({ slug: 'openagents' }))),
      Scene.expect(
        Scene.role('heading', { name: 'Khala Code + OpenAgents Overview' }),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'Khala Code today' }),
      ).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'OpenAgents network' }),
      ).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Promise state' })).toExist(),
      Scene.expect(
        Scene.text(
          'The current posture is intentionally hedged: Khala Code is buildable from this repo and Episode 245 launched the product direction, but there is no public installer yet. Free-plan desktop trace capture is not live, and the Paid private-data plan is not yet purchasable.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Khala Code plan catalog' }),
      ).toHaveAttr('href', '/api/public/khala-code/plans'),
      Scene.expect(
        Scene.role('link', { name: 'Product promises JSON' }),
      ).toHaveAttr('href', '/api/public/product-promises'),
    )
  })

  test('renders the Autonomous QA docs page', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(DocsPageRoute({ slug: 'autonomous-qa' }))),
      Scene.expect(Scene.role('heading', { name: 'Autonomous QA' })).toExist(),
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
      Scene.expect(Scene.selector('[data-account-menu-trigger]')).toExist(),
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
          'Khala Code wraps your own local Codex install, adds fleet coordination, and connects coding work to the OpenAgents network while the public installer and economics loop are still being brought online.',
        ),
      ).toExist(),
      Scene.expect(Scene.text('Introducing Khala Code')).toExist(),
      Scene.expect(Scene.text('Introducing Autopilot Sites')).toBeAbsent(),
      Scene.expect(Scene.text('Episode 228: Free Autopilot')).toBeAbsent(),
      Scene.expect(Scene.text('Get Paid to Code')).toBeAbsent(),
      Scene.expect(
        Scene.role('navigation', { name: 'OpenAgents navigation' }),
      ).toBeAbsent(),
    )
  })

  test('renders a blog article', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(BlogPostRoute({ slug: 'introducing-khala-code' })),
      ),
      Scene.expect(
        Scene.role('heading', {
          name: 'Introducing Khala Code',
        }),
      ).toExist(),
      Scene.expect(Scene.text('The coding front door')).toExist(),
      Scene.expect(Scene.text('Fleet and swarm coordination')).toExist(),
      Scene.expect(
        Scene.text('Free, paid, and the honest promise state'),
      ).toExist(),
      Scene.expect(Scene.role('link', { name: 'Plan catalog' })).toHaveAttr(
        'href',
        '/api/public/khala-code/plans',
      ),
      Scene.expect(
        Scene.role('link', { name: 'Product-promise registry' }),
      ).toHaveAttr('href', '/api/public/product-promises'),
    )

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
    const [timelineLoadedModel] = LoggedOut.update(
      loadedModel,
      SucceededLoadPublicActivityTimeline({
        envelope: {
          schemaVersion: 'openagents.public_activity_timeline.v1',
          generatedAt: '2026-06-27T17:00:00.000Z',
          staleness: {
            composition: 'live_at_read',
            contractVersion: 'projection_staleness.v1',
            maxStalenessSeconds: 0,
            rebuildsOn: ['route:/api/public/activity-timeline'],
          },
          nextCursor: null,
          sourceLag: [
            {
              sourceKind: 'forum',
              status: 'current',
              latestSourceEventAt: '2026-06-27T16:59:00.000Z',
              observedAt: '2026-06-27T17:00:00.000Z',
              lagSeconds: 60,
              maxStalenessSeconds: 900,
              sourceRefs: ['route:/api/forum'],
              blockerRefs: [],
              caveatRefs: [],
            },
          ],
          events: [
            {
              eventRef: 'forum_post_public_1',
              cursor: '2026-06-27T16:59:00.000Z:forum:forum_post_public_1',
              ts: '2026-06-27T16:59:00.000Z',
              kind: 'forum_posted',
              sourceKind: 'forum',
              refs: ['route:/forum'],
              sourceRefs: ['route:/api/forum'],
              blockerRefs: [],
              caveatRefs: [],
              text: 'Posted a public launch update in the Product Promises forum.',
            },
            {
              eventRef: 'inference_public_1',
              cursor:
                '2026-06-27T16:58:00.000Z:inference_receipt:inference_public_1',
              ts: '2026-06-27T16:58:00.000Z',
              kind: 'khala_inference_served',
              sourceKind: 'inference_receipt',
              refs: [],
              sourceRefs: ['route:/api/public/khala-tokens-served'],
              blockerRefs: [],
              caveatRefs: [],
              text: 'Served a public Khala inference request.',
            },
          ],
        },
      }),
    )

    Scene.scene(
      { update, view },
      Scene.with(timelineLoadedModel),
      Scene.expect(Scene.role('heading', { name: 'Artanis' })).toExist(),
      Scene.expect(Scene.text('ARTANIS console')).toExist(),
      Scene.expect(Scene.text('LIVE')).toExist(),
      Scene.expect(Scene.text('Active 5/7 slots')).toExist(),
      Scene.expect(Scene.text('Daily token pace')).toExist(),
      Scene.expect(
        Scene.text(
          'Release the next version of Pylon, connect it deeply to Omega, and route more inference and fine-tuning work to the live Pylon wave using the new Bitcoin infrastructure.',
        ),
      ).toExist(),
      Scene.expect(Scene.text('Run accepted.')).toBeAbsent(),
      // Live fleet-shipping feed replaces the stale status report + admin ticks.
      Scene.expect(Scene.text('Fleet shipping')).toExist(),
      Scene.expect(Scene.text('What the fleet is doing now')).toExist(),
      Scene.expect(
        Scene.text(
          'Posted a public launch update in the Product Promises forum.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.text('Served a public Khala inference request.'),
      ).toExist(),
      Scene.expect(Scene.text('Shipped (24h)')).toExist(),
      // The stale report and internal ledger jargon are gone.
      Scene.expect(Scene.text('Artanis status report')).toBeAbsent(),
      Scene.expect(Scene.text('Live Artanis decisions')).toBeAbsent(),
      Scene.expect(Scene.text('Accepted-work bitcoin')).toBeAbsent(),
      Scene.expect(Scene.text('Omega release gate')).toBeAbsent(),
      // Recruitment payoff: the Join CTA stays prominent.
      Scene.expect(
        Scene.role('heading', {
          name: 'Have Codex or Claude? Join the fleet.',
        }),
      ).toExist(),
      Scene.expect(Scene.text('khala fleet connect')).toExist(),
      Scene.expect(Scene.text('khala fleet status')).toExist(),
      Scene.expect(Scene.role('link', { name: 'Fleet docs' })).toHaveAttr(
        'href',
        '/docs/connect-codex-fleet',
      ),
      Scene.expect(
        Scene.role('link', { name: 'Read the setup guide' }),
      ).toHaveAttr('href', '/docs/connect-codex-fleet'),
      Scene.expect(Scene.role('link', { name: 'Full activity' })).toHaveAttr(
        'href',
        '/activity',
      ),
      // Live Pylon network stats stay.
      Scene.expect(Scene.text('Omega Pylon stats')).toExist(),
      Scene.expect(Scene.text('Pylons online')).toExist(),
      Scene.expect(Scene.text('Earning gate')).toExist(),
      Scene.expect(Scene.text('authGrantRef')).toBeAbsent(),
      Scene.expect(Scene.text('payloadJson')).toBeAbsent(),
      Scene.expect(Scene.text('hiddenSteering')).toBeAbsent(),
    )
  })

  test('renders the Codex fleet connection docs page', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(DocsPageRoute({ slug: 'connect-codex-fleet' })),
      ),
      Scene.expect(
        Scene.role('heading', { name: 'Connect Your Codex Fleet' }),
      ).toExist(),
      Scene.expect(Scene.text('khala fleet connect')).toExist(),
      Scene.expect(Scene.text('khala fleet status')).toExist(),
      Scene.expect(
        Scene.text(
          'The own-capacity coding path routes only through capacity linked to the same owner scope. It is not third-party pooled labor and it is not a settlement-bearing marketplace path.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Khala CLI fleet docs' }),
      ).toHaveAttr(
        'href',
        'https://github.com/OpenAgentsInc/openagents/blob/main/clients/khala-cli/README.md#connect-your-codex-fleet',
      ),
      Scene.expect(
        Scene.role('link', { name: 'Fleet contribution plan' }),
      ).toHaveAttr(
        'href',
        'https://github.com/OpenAgentsInc/openagents/blob/main/docs/ops/2026-06-27-artanis-as-a-service-multi-tenant-codex-fleet-enablement.md',
      ),
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
