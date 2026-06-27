import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { userFacingCopy } from '../../../display-copy'
import { friendlyRelativeTime } from '../../../time-format'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  Model,
  PublicAdjutantActivityMilestone,
  PublicAdjutantActivityModel,
  PublicAdjutantDeployedSite,
  PublicAgentGoal,
  PublicAgentGoalEvent,
  PublicArtanisForumRewardSmoke,
  PublicArtanisForumRewardVisibility,
  PublicArtanisProductionLaunchGate,
  PublicArtanisPylonLaunchCommunication,
  PublicArtanisReport,
  PublicArtanisReportClaimSummary,
  PublicArtanisReportModel,
  PublicPylonStats,
  PublicPylonStatsModel,
  PublicPylonV02OmegaReleaseGate,
  PublicRecentPylon,
} from '../model'

const campaignObjective =
  'Release the next version of Pylon, connect it deeply to Omega, and route more inference and fine-tuning work to the live Pylon wave using the new Bitcoin infrastructure.'

const adjutantObjective =
  'Supervise public software-order fulfillment and Autopilot Sites delivery from order assignment through reviewable versions, deployment, customer-visible status, and public-safe progress.'

const numberFormatter = new Intl.NumberFormat('en-US')

const displayName = (agentRef: string): string =>
  agentRef === 'artanis'
    ? 'Artanis'
    : agentRef === 'adjutant'
      ? 'Autopilot'
      : agentRef

const fallbackObjective = (agentRef: string): string =>
  agentRef === 'artanis'
    ? campaignObjective
    : agentRef === 'adjutant'
      ? adjutantObjective
      : 'This public agent has not published a durable goal yet.'

const statusText = (goal: PublicAgentGoal | null): string =>
  goal === null ? 'No public goal' : goal.status.replace(/_/g, ' ')

const usageText = (goal: PublicAgentGoal): string =>
  goal.remainingTokens === null
    ? `${goal.tokensUsed} tokens`
    : `${goal.tokensUsed} / ${goal.tokenBudget ?? 0} tokens`

const formatNumber = (value: number): string => numberFormatter.format(value)

const publicRefsLabel = (
  label: string,
  refs: ReadonlyArray<string>,
): string | null =>
  Array.match(refs, {
    onEmpty: () => null,
    onNonEmpty: values => `${label} ${values.slice(0, 3).join(', ')}`,
  })

const trimProtocol = (url: string | null): string =>
  url === null
    ? 'none'
    : url.replace(/^https?:\/\//, '').replace(/^wss:\/\//, '')

const statsStatusText = (stats: PublicPylonStats | null): string =>
  stats === null ? 'loading' : stats.available ? 'live' : 'down'

const pylonStatsFromModel = (
  model: PublicPylonStatsModel,
): PublicPylonStats | null =>
  model._tag === 'PublicPylonStatsLoaded' ? model.stats : null

const pylonStatsError = (model: PublicPylonStatsModel): string | null =>
  model._tag === 'PublicPylonStatsFailed'
    ? model.error
    : model._tag === 'PublicPylonStatsLoaded'
      ? model.stats.error
      : null

const deployedSiteRow = (site: PublicAdjutantDeployedSite): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-2 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)_auto]',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('min-w-0')],
        [
          h.div(
            [Ui.className<Message>('truncate text-[0.8125rem] text-[#f1efe8]')],
            [site.title],
          ),
          h.div(
            [Ui.className<Message>('truncate text-[0.75rem] text-white/35')],
            [site.publicRef],
          ),
        ],
      ),
      h.a(
        [
          h.Href(site.url),
          h.Target('_blank'),
          h.Rel('noreferrer'),
          Ui.className<Message>(
            'min-w-0 truncate text-[0.75rem] text-white/55 underline underline-offset-[3px] hover:text-[#f1efe8]',
          ),
        ],
        [trimProtocol(site.url)],
      ),
      h.div(
        [
          Ui.className<Message>(
            'tabular-nums text-[0.75rem] text-white/45 sm:text-right',
          ),
        ],
        [site.status],
      ),
    ],
  )
}

const adjutantMilestoneRow = (
  milestone: PublicAdjutantActivityMilestone,
): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.span(
            [Ui.className<Message>('text-[0.8125rem] text-[#f1efe8]')],
            [userFacingCopy(milestone.label)],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] text-white/35')],
            [milestone.stage],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/45')],
        [userFacingCopy(milestone.summary)],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [`${milestone.publicRef} / ${milestone.updatedAt}`],
      ),
      milestone.siteUrl === null
        ? null
        : h.a(
            [
              h.Href(milestone.siteUrl),
              h.Target('_blank'),
              h.Rel('noreferrer'),
              Ui.className<Message>(
                'text-[0.75rem] text-white/55 underline underline-offset-[3px] hover:text-[#f1efe8]',
              ),
            ],
            [trimProtocol(milestone.siteUrl)],
          ),
    ],
  )
}

const adjutantActivityView = (model: PublicAdjutantActivityModel): Html => {
  const h = html<Message>()

  if (model._tag === 'PublicAdjutantActivityLoading') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
        h.p(
          [Ui.className<Message>('text-sm text-white/45')],
          ['Loading public Autopilot activity.'],
        ),
      ],
    )
  }

  if (model._tag === 'PublicAdjutantActivityFailed') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
        h.p([Ui.className<Message>('text-sm text-[#ff6f00]')], [model.error]),
      ],
    )
  }

  if (model._tag !== 'PublicAdjutantActivityLoaded') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
        h.p(
          [Ui.className<Message>('text-sm text-white/45')],
          ['No public Autopilot activity has been published yet.'],
        ),
      ],
    )
  }

  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Autopilot activity'],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [
              `${model.activity.milestones.length} public milestone${model.activity.milestones.length === 1 ? '' : 's'}`,
            ],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid gap-2 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Deployed public Sites'],
              ),
              Array.match(model.activity.deployedSites.slice(0, 4), {
                onEmpty: () =>
                  h.p(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    ['No public Sites are deployed yet.'],
                  ),
                onNonEmpty: sites =>
                  h.ol(
                    [Ui.className<Message>('grid')],
                    sites.map(deployedSiteRow),
                  ),
              }),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid gap-2 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Recent milestones'],
              ),
              Array.match(model.activity.milestones.slice(0, 5), {
                onEmpty: () =>
                  h.p(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    ['No public milestones are available.'],
                  ),
                onNonEmpty: milestones =>
                  h.ol(
                    [Ui.className<Message>('grid')],
                    milestones.map(adjutantMilestoneRow),
                  ),
              }),
            ],
          ),
        ],
      ),
    ],
  )
}

const statsMetric = (label: string, value: string, detail: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid min-h-28 content-between gap-3 border border-[#222] bg-[#010102] p-3',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('truncate text-[0.6875rem] text-white/45')],
        [label],
      ),
      h.div(
        [
          Ui.className<Message>(
            'tabular-nums text-3xl font-semibold tracking-normal text-[#f1efe8]',
          ),
        ],
        [value],
      ),
      h.div([Ui.className<Message>('text-[0.75rem] text-white/35')], [detail]),
    ],
  )
}

const pylonRow = (pylon: PublicRecentPylon): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-2 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('min-w-0')],
        [
          h.div(
            [Ui.className<Message>('truncate text-[0.8125rem] text-[#f1efe8]')],
            [pylon.nodeLabel ?? pylon.nostrPubkeyShort],
          ),
          h.div(
            [Ui.className<Message>('truncate text-[0.75rem] text-white/35')],
            [pylon.nostrPubkeyShort],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('min-w-0 text-[0.75rem] text-white/45')],
        [
          `${pylon.runtimeState ?? 'unknown'} / ${pylon.readyModel ?? 'unknown'}`,
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'tabular-nums text-[0.75rem] text-white/45 sm:text-right',
          ),
        ],
        [formatNumber(pylon.eligibleProductCount)],
      ),
    ],
  )
}

const pylonStatsView = (model: PublicPylonStatsModel): Html => {
  const h = html<Message>()
  const stats = pylonStatsFromModel(model)
  const error = pylonStatsError(model)
  const recentPylons = stats?.recentPylons.slice(0, 4) ?? []

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div(
                [Ui.className<Message>(Ui.eyebrowClass)],
                ['Pylon network'],
              ),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Omega Pylon stats'],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [`Feed ${statsStatusText(stats)}`],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 sm:grid-cols-2 lg:grid-cols-5')],
        [
          statsMetric(
            'Pylons online',
            stats === null ? '-' : formatNumber(stats.pylonsOnlineNow),
            'v0.2.5+ heartbeat window',
          ),
          statsMetric(
            'Wallet ready',
            stats === null ? '-' : formatNumber(stats.pylonsWalletReadyNow),
            'Public readiness',
          ),
          statsMetric(
            'Seen in 24h',
            stats === null ? '-' : formatNumber(stats.pylonsSeen24h),
            'Recent check-ins',
          ),
          statsMetric(
            'Earning gate',
            stats === null
              ? '-'
              : stats.earningLaunchGate.publicEarningCopyAllowed
                ? 'Ready'
                : 'Blocked',
            stats === null
              ? 'Stats loading'
              : stats.earningLaunchGate.publicEarningCopyAllowed
                ? 'Bounded copy'
                : 'Unsafe copy blocked',
          ),
          statsMetric(
            'Version floor',
            stats === null ? '-' : `v${stats.minimumClientVersion}+`,
            stats === null ? 'Stats loading' : (stats.asOfLabel ?? 'Fresh'),
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 border border-[#222] bg-[#010102] p-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.25fr)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2 text-[0.75rem] text-white/45')],
            [
              h.div(
                [Ui.className<Message>('text-[#f1efe8]')],
                [`Source ${trimProtocol(stats?.sourceUrl ?? null)}`],
              ),
              h.div(
                [],
                [`Relay ${trimProtocol(stats?.hostedNexusRelayUrl ?? null)}`],
              ),
              h.div(
                [],
                [
                  stats?.asOfLabel === null || stats?.asOfLabel === undefined
                    ? 'Timestamp unavailable'
                    : `As of ${friendlyRelativeTime(stats.asOfLabel)}`,
                ],
              ),
              h.div(
                [],
                [
                  `Training participants ${formatNumber(stats?.trainingAcceptedContributors ?? 0)} / assigned ${formatNumber(stats?.trainingAssignedContributors ?? 0)}`,
                ],
              ),
              error === null
                ? null
                : h.div([Ui.className<Message>('text-[#ff6f00]')], [error]),
            ],
          ),
          Array.match(recentPylons, {
            onEmpty: () =>
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [
                  stats === null
                    ? 'Loading recent Pylon presence.'
                    : 'No recent Pylon rows published.',
                ],
              ),
            onNonEmpty: rows =>
              h.ol([Ui.className<Message>('grid')], rows.map(pylonRow)),
          }),
        ],
      ),
    ],
  )
}

const compactRefs = (
  refs: ReadonlyArray<string>,
  fallback = 'No public refs',
): string => (refs.length === 0 ? fallback : refs.slice(0, 3).join(', '))

const bitcoinPrimary = (value: string): string => value.replace(/ \(.+\)$/, '')

const bitcoinDenomination = (value: string): string | null =>
  value.match(/\((.+)\)$/)?.[1] ?? null

const artanisFleetCommand = (command: string, label: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border border-[#222] bg-black p-3 sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.4fr)]',
      ),
    ],
    [
      h.div([Ui.className<Message>('text-[0.75rem] text-white/45')], [label]),
      h.code(
        [
          Ui.className<Message>(
            'break-words text-[0.8125rem] leading-6 text-[#f1efe8]',
          ),
        ],
        [command],
      ),
    ],
  )
}

const artanisFleetRecruitmentView = (): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('artanis-fleet-recruitment', ''),
      Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6'),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Join the fleet']),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Have Codex or Claude? Put it to work.'],
              ),
            ],
          ),
          h.a(
            [
              h.Href('/docs/khala-cli'),
              Ui.className<Message>(
                'border border-[#333] px-3 py-2 text-[0.75rem] text-white/70 underline-offset-4 hover:border-white/35 hover:text-[#f1efe8] hover:underline',
              ),
            ],
            ['Fleet docs'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.p(
                [Ui.className<Message>('text-sm leading-6 text-[#f1efe8]')],
                [
                  'Connect your local coding accounts and Artanis can route public issue work through your own Pylon capacity.',
                ],
              ),
              artanisFleetCommand(
                'npm install -g @openagentsinc/khala',
                'Install',
              ),
              artanisFleetCommand('khala fleet connect', 'Connect'),
              artanisFleetCommand('khala fleet status', 'Check'),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              statsMetric(
                'Paste-free login',
                'device code',
                'Codex opens the browser and shows a short code.',
              ),
              statsMetric(
                'More accounts',
                'more slots',
                'Each distinct account adds its own local throughput.',
              ),
              statsMetric(
                'Private material',
                'stays local',
                'Public pages show generic fleet refs and public issues only.',
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const artanisClaimRow = (claim: PublicArtanisReportClaimSummary): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.span(
            [Ui.className<Message>('text-[0.8125rem] text-[#f1efe8]')],
            [claim.label],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] text-white/45')],
            [claim.stateLabel],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('text-[0.75rem] text-white/45')],
        [claim.description],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [compactRefs(claim.blockedByRefs, compactRefs(claim.evidenceRefs))],
      ),
    ],
  )
}

const artanisForumRewardView = (
  visibility: PublicArtanisForumRewardVisibility,
): Html => {
  const h = html<Message>()
  const receiptLabel = `${formatNumber(visibility.contentRewardCount)} content ${
    visibility.contentRewardCount === 1 ? 'reward' : 'rewards'
  }`
  const bridgeLabel = `${formatNumber(visibility.acceptedContributionCount)} accepted ${
    visibility.acceptedContributionCount === 1 ? 'bridge' : 'bridges'
  }`

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Forum bitcoin'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [visibility.stateLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric('Content rewards', receiptLabel, bridgeLabel),
          statsMetric(
            'Live spend',
            visibility.liveWalletSpendAllowed ? 'available' : 'blocked',
            visibility.liveWalletSpendAllowed
              ? 'Wallet authority present'
              : 'Needs wallet authority and spend cap',
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          compactRefs(
            visibility.caveatRefs,
            compactRefs(visibility.blockerRefs),
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          `Receipts ${compactRefs(visibility.forumReceiptRefs)} / actions ${compactRefs(visibility.paidActionRefs)}`,
        ],
      ),
    ],
  )
}

const artanisForumRewardSmokeView = (
  smoke: PublicArtanisForumRewardSmoke,
): Html => {
  const h = html<Message>()
  const exchangeLabel = `${formatNumber(smoke.exchangeCount)} ${
    smoke.exchangeCount === 1 ? 'exchange' : 'exchanges'
  }`

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Reward check'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [smoke.modeLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Registered agents',
            formatNumber(smoke.registeredAgentRefs.length),
            exchangeLabel,
          ),
          statsMetric(
            'Live bitcoin',
            smoke.usedLiveBitcoin ? 'recorded' : 'not used',
            smoke.usedLiveBitcoin
              ? compactRefs(smoke.walletAuthorityRefs)
              : 'Simulation only',
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [compactRefs(smoke.runReasonRefs, compactRefs(smoke.caveatRefs))],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          `Receipts ${compactRefs(smoke.receiptProjectionRefs)} / boundary ${compactRefs(smoke.acceptedContributionBoundaryRefs)}`,
        ],
      ),
    ],
  )
}

const artanisPylonLaunchView = (
  launch: PublicArtanisPylonLaunchCommunication,
): Html => {
  const h = html<Message>()
  const stageCount = launch.stageSummaryRefs.length
  const forumTopicPath = launch.primaryForumTopicUrl.replace(
    'https://openagents.com',
    '',
  )

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Pylon launch'],
          ),
          h.a(
            [
              h.Href(forumTopicPath),
              Ui.className<Message>(
                'text-[0.75rem] text-white/55 underline-offset-4 hover:text-[#f1efe8] hover:underline',
              ),
            ],
            ['Forum update'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Launch brief',
            launch.forumIntentReady ? 'prepared' : 'blocked',
            launch.forumPostTitle,
          ),
          statsMetric(
            'Readiness',
            `${formatNumber(stageCount)} states`,
            compactRefs(launch.stageSummaryRefs),
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          compactRefs(
            launch.resourceModeCaveatRefs,
            compactRefs(launch.authorityBoundaryRefs),
          ),
        ],
      ),
    ],
  )
}

const artanisProductionLaunchGateView = (
  gate: PublicArtanisProductionLaunchGate,
): Html => {
  const h = html<Message>()
  const blockerCount = gate.failedOrPendingRequiredCount
  const firstBlockers = gate.blockerRefs.slice(0, 3)

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Production gate'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [gate.stateLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Autonomy claim',
            gate.canClaimContinuouslyRunning ? 'allowed' : 'blocked',
            gate.canClaimContinuouslyRunning
              ? 'All required gates passed'
              : `${formatNumber(blockerCount)} required ${
                  blockerCount === 1 ? 'gate' : 'gates'
                } not passed`,
          ),
          statsMetric(
            'Verification',
            `${formatNumber(gate.verificationTargetRefs.length)} targets`,
            compactRefs(gate.verificationTargetRefs),
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          firstBlockers.length === 0
            ? compactRefs(gate.docsRefs)
            : compactRefs(firstBlockers, compactRefs(gate.docsRefs)),
        ],
      ),
    ],
  )
}

const artanisOmegaReleaseGateView = (
  gate: PublicPylonV02OmegaReleaseGate,
): Html => {
  const h = html<Message>()
  const blockerCount = gate.failedOrPendingRequiredCount
  const pylonProofLabel = `${formatNumber(gate.multiPylonObservedDistinctPylonCount)} / ${formatNumber(gate.multiPylonRequiredDistinctPylonCount)} distinct Pylons`
  const firstBlockers = gate.blockerRefs.slice(0, 3)

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Omega release gate'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [gate.stateLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Multi-Pylon proof',
            gate.multiPylonPaidWorkProofComplete ? 'complete' : 'blocked',
            pylonProofLabel,
          ),
          statsMetric(
            'Release claim',
            gate.canAnnouncePylonV02Release ? 'allowed' : 'blocked',
            gate.canAnnouncePylonV02Release
              ? 'All required public proof is complete'
              : `${formatNumber(blockerCount)} required ${
                  blockerCount === 1 ? 'item' : 'items'
                } not passed`,
          ),
          statsMetric(
            'Payment mode',
            gate.payoutModeGate.livePayoutClaimAllowed ? 'declared' : 'blocked',
            gate.hostedMdkDirectPayoutClaimAllowed
              ? 'Hosted MDK direct payout'
              : gate.payoutModeGate.modeLabel,
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          firstBlockers.length === 0
            ? compactRefs(gate.runbookRefs)
            : compactRefs(firstBlockers, compactRefs(gate.runbookRefs)),
        ],
      ),
    ],
  )
}

const artanisReportLoadedView = (report: PublicArtanisReport): Html => {
  const h = html<Message>()
  const blockers = report.publicBlockerRefs.slice(0, 5)
  const claims = [...report.standaloneClaims, ...report.r10Claims].slice(0, 7)
  const healthAttentionCount = report.healthSummary.staleOrBlockedSignalCount
  const acceptedWorkDenomination = bitcoinDenomination(
    report.pylonSummary.acceptedWorkBitcoinTotal,
  )
  const acceptedWorkSettlementDetail = report.pylonSummary
    .acceptedWorkSettlementGate.publicPaidWorkTotalsAllowed
    ? `Receipts ${compactRefs(report.pylonSummary.acceptedWorkSettlementReceiptRefs)}`
    : report.pylonSummary.acceptedWorkSettlementGate.stateLabel

  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div(
                [Ui.className<Message>(Ui.eyebrowClass)],
                ['Public report'],
              ),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Artanis status report'],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [`Updated ${report.updatedAtDisplay}`],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 sm:grid-cols-2 lg:grid-cols-5')],
        [
          statsMetric(
            'Autonomous loop',
            report.autonomousLoop.state.replace(/_/g, ' '),
            report.autonomousLoop.latestTickState === null
              ? `${formatNumber(report.autonomousLoop.tickCount)} ticks`
              : `${report.autonomousLoop.latestTickState.replace(/_/g, ' ')} tick`,
          ),
          statsMetric(
            'Health',
            report.healthSummary.overallState.replace(/_/g, ' '),
            report.healthSummary.overclaimBlocked
              ? `${formatNumber(healthAttentionCount)} ${
                  healthAttentionCount === 1 ? 'signal' : 'signals'
                } ${healthAttentionCount === 1 ? 'needs' : 'need'} attention`
              : 'No stale signals',
          ),
          statsMetric(
            'Model Lab',
            report.modelLabSummary.readiness.replace(/_/g, ' '),
            `${formatNumber(report.modelLabSummary.completeSectionCount)} / ${formatNumber(report.modelLabSummary.sectionCount)} sections complete`,
          ),
          statsMetric(
            'Pylon feed',
            report.pylonSummary.feedStatus,
            `${formatNumber(report.pylonSummary.pylonsOnlineNow)} online / ${formatNumber(report.pylonSummary.assignmentReadyPylonsOnlineNow)} assignment-ready / ${
              report.pylonSummary.earningLaunchGate.publicEarningCopyAllowed
                ? 'earning ready'
                : 'earning blocked'
            }`,
          ),
          statsMetric(
            'Accepted-work bitcoin',
            bitcoinPrimary(report.pylonSummary.acceptedWorkBitcoinTotal),
            acceptedWorkDenomination === null
              ? acceptedWorkSettlementDetail
              : `${acceptedWorkDenomination} total / ${acceptedWorkSettlementDetail}`,
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Public blockers'],
              ),
              blockers.length === 0
                ? h.p(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    ['No public blockers are listed.'],
                  )
                : h.ul(
                    [Ui.className<Message>('grid gap-1')],
                    blockers.map(blocker =>
                      h.li(
                        [
                          Ui.className<Message>(
                            'break-words text-[0.75rem] text-white/55',
                          ),
                        ],
                        [blocker],
                      ),
                    ),
                  ),
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [
                  `Receipts ${compactRefs(report.receiptRefs)} / artifacts ${compactRefs(report.artifactRefs)}`,
                ],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Forum refs'],
              ),
              h.div(
                [Ui.className<Message>('flex flex-wrap gap-2')],
                report.forumLinks.map(link =>
                  h.a(
                    [
                      h.Href(link.href),
                      Ui.className<Message>(
                        'border border-[#333] px-3 py-2 text-[0.75rem] text-white/70 underline-offset-4 hover:border-white/35 hover:text-[#f1efe8] hover:underline',
                      ),
                    ],
                    [link.label],
                  ),
                ),
              ),
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [compactRefs(report.publicGoalRefs)],
              ),
            ],
          ),
        ],
      ),
      artanisPylonLaunchView(report.pylonLaunchCommunication),
      artanisOmegaReleaseGateView(report.pylonOmegaReleaseGate),
      artanisProductionLaunchGateView(report.productionLaunchGate),
      artanisForumRewardView(report.forumRewardVisibility),
      artanisForumRewardSmokeView(report.forumRewardSmoke),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 border border-[#222] bg-[#010102] p-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Claim states'],
          ),
          h.ol([Ui.className<Message>('grid')], claims.map(artanisClaimRow)),
        ],
      ),
    ],
  )
}

const artanisReportView = (model: PublicArtanisReportModel): Html => {
  const h = html<Message>()

  if (model._tag === 'PublicArtanisReportLoaded') {
    return artanisReportLoadedView(model.report)
  }

  if (model._tag === 'PublicArtanisReportFailed') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public report']),
        h.p([Ui.className<Message>('text-sm text-[#ff6f00]')], [model.error]),
      ],
    )
  }

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public report']),
      h.p(
        [Ui.className<Message>('text-sm text-white/45')],
        ['Loading Artanis public report.'],
      ),
    ],
  )
}

const eventRow = (event: PublicAgentGoalEvent): Html => {
  const h = html<Message>()
  const publicRefs = [
    publicRefsLabel('commits', event.commitRefs),
    publicRefsLabel('artifacts', event.artifactRefs),
    publicRefsLabel('receipts', event.receiptRefs),
  ].filter((value): value is string => value !== null)

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-3')],
        [
          h.span(
            [Ui.className<Message>('text-[0.8125rem] text-[#f1efe8]')],
            [userFacingCopy(event.summary)],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] text-white/35')],
            [event.status ?? event.type],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          event.runId === null
            ? friendlyRelativeTime(event.createdAt)
            : `${event.runId} / ${friendlyRelativeTime(event.createdAt)}`,
        ],
      ),
      Array.match(publicRefs, {
        onEmpty: () => null,
        onNonEmpty: refs =>
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/35')],
            [refs.join(' / ')],
          ),
      }),
    ],
  )
}

const loadedView = (
  agentRef: string,
  goal: PublicAgentGoal | null,
  events: ReadonlyArray<PublicAgentGoalEvent>,
  pylonStats: PublicPylonStatsModel,
  artanisReport: PublicArtanisReportModel,
  adjutantActivity: PublicAdjutantActivityModel,
): Html => {
  const h = html<Message>()
  const agentName = displayName(agentRef)
  const displayedObjective = userFacingCopy(
    goal?.objective ?? fallbackObjective(agentRef),
  )
  const isArtanis = agentRef === 'artanis'
  const isAdjutant = agentRef === 'adjutant'
  return h.main(
    [
      h.DataAttribute('component', 'public-agent-page'),
      Ui.className<Message>(
        'mx-auto grid min-h-screen max-w-5xl content-start gap-8 px-6 py-10 font-mono text-[#f1efe8] sm:px-8',
      ),
    ],
    [
      h.header(
        [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
        [
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap items-center justify-between gap-3',
              ),
            ],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public agent']),
              h.a(
                [
                  h.Href('/'),
                  Ui.className<Message>(
                    'text-[0.75rem] text-white/45 underline-offset-4 hover:text-[#f1efe8] hover:underline',
                  ),
                ],
                ['Start your own agent'],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap items-end justify-between gap-4',
              ),
            ],
            [
              h.h1(
                [
                  Ui.className<Message>(
                    'text-3xl font-semibold leading-none tracking-normal sm:text-4xl',
                  ),
                ],
                [agentName],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'border border-[#333] px-2.5 py-1 text-[0.75rem] text-white/55',
                  ),
                ],
                [statusText(goal)],
              ),
            ],
          ),
        ],
      ),
      h.section(
        [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
        [
          h.div(
            [Ui.className<Message>(Ui.eyebrowClass)],
            [goal === null ? 'Campaign objective' : 'Current goal'],
          ),
          h.div(
            [Ui.className<Message>('grid gap-3')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'max-w-4xl whitespace-pre-wrap text-base leading-7 text-[#f1efe8]',
                  ),
                ],
                [displayedObjective],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'flex flex-wrap gap-x-6 gap-y-2 text-[0.75rem] text-white/45',
                  ),
                ],
                goal === null
                  ? [`Awaiting the first public durable ${agentName} goal.`]
                  : [
                      h.span([], [usageText(goal)]),
                      h.span(
                        [],
                        [
                          goal.currentRunId === null
                            ? 'no active run'
                            : `current run ${goal.currentRunId}`,
                        ],
                      ),
                      h.span(
                        [],
                        [`updated ${friendlyRelativeTime(goal.updatedAt)}`],
                      ),
                    ],
              ),
            ],
          ),
        ],
      ),
      isArtanis ? artanisReportView(artanisReport) : null,
      isArtanis ? artanisFleetRecruitmentView() : null,
      isArtanis ? pylonStatsView(pylonStats) : null,
      isAdjutant ? adjutantActivityView(adjutantActivity) : null,
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Activity']),
          Array.match(events, {
            onEmpty: () =>
              h.p(
                [Ui.className<Message>('text-sm text-white/45')],
                ['No public activity has been published yet.'],
              ),
            onNonEmpty: events =>
              h.ol([Ui.className<Message>('grid')], events.map(eventRow)),
          }),
        ],
      ),
    ],
  )
}

export const view = (model: Model, agentRef: string): Html => {
  const h = html<Message>()

  if (
    model.publicAgent._tag === 'PublicAgentLoaded' &&
    model.publicAgent.agentRef === agentRef
  ) {
    return loadedView(
      agentRef,
      model.publicAgent.response.goal,
      model.publicAgent.response.events,
      model.publicPylonStats,
      model.publicArtanisReport,
      model.publicAdjutantActivity,
    )
  }

  if (
    model.publicAgent._tag === 'PublicAgentFailed' &&
    model.publicAgent.agentRef === agentRef
  ) {
    return loadedView(
      agentRef,
      null,
      [
        {
          id: 'public-agent-load-error',
          goalId: 'unknown',
          runId: null,
          type: 'LoadFailed',
          status: 'failed',
          summary: model.publicAgent.error,
          tokenDelta: 0,
          timeDeltaSeconds: 0,
          artifactRefs: [],
          receiptRefs: [],
          commitRefs: [],
          createdAt: 'now',
        },
      ],
      model.publicPylonStats,
      model.publicArtanisReport,
      model.publicAdjutantActivity,
    )
  }

  return h.main(
    [
      h.DataAttribute('component', 'public-agent-page'),
      Ui.className<Message>(
        'mx-auto grid min-h-screen max-w-5xl content-start gap-6 px-6 py-10 font-mono text-[#f1efe8] sm:px-8',
      ),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public agent']),
      h.h1(
        [Ui.className<Message>('text-3xl font-semibold')],
        [displayName(agentRef)],
      ),
      h.p(
        [Ui.className<Message>('text-sm text-white/45')],
        ['Loading public goal.'],
      ),
    ],
  )
}
