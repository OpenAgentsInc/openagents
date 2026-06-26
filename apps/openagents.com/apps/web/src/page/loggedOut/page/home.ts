import { motionOdometerClass } from '@openagentsinc/ui'
import { Array, Match as M } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  PublicForumLaunchStatus,
  PublicForumLaunchStatusModel,
  PublicForumTipLeaderboards,
  PublicForumTipLeaderboardsModel,
  PublicKhalaTokensServedHistoryModel,
  PublicKhalaTokensServedHistoryPoint,
  PublicKhalaTokensServedModelMixFamily,
  PublicKhalaTokensServedModelMixModel,
  PublicKhalaTokensServedModel,
  PublicPylonStats,
  PublicPylonStatsModel,
  SettledFeedModel,
} from '../model'

export type HomeViewInput = {
  forumLaunchStatus: PublicForumLaunchStatusModel
  forumTipLeaderboards: PublicForumTipLeaderboardsModel
  publicKhalaTokensServed: PublicKhalaTokensServedModel
  publicKhalaTokensServedHistory: PublicKhalaTokensServedHistoryModel
  publicKhalaTokensServedModelMix: PublicKhalaTokensServedModelMixModel
  publicPylonStats: PublicPylonStatsModel
  settledFeed: SettledFeedModel
}

type RowTone = 'good' | 'muted' | 'warn'

const numberFormatter = new Intl.NumberFormat('en-US')

const formatNumber = (value: number): string => numberFormatter.format(value)

const formatSats = (value: number): string => `${formatNumber(value)} sats`

const valueOrUnavailable = (value: number | null): string =>
  value === null ? 'Unavailable' : formatSats(value)

const statsFromModel = (
  model: PublicPylonStatsModel,
): PublicPylonStats | null =>
  model._tag === 'PublicPylonStatsLoaded' ? model.stats : null

// Resolve the public real-settled 24h figure for the settled feed panel.
// `publicRealSatsSettled24h` is the deduped aggregate of accepted-work, market,
// and treasury-outflow real settlements (workers/api public-pylon-stats), so it
// is the right 24h counterpart to the live feed's settled total. Returns null
// when stats are unavailable or the field is absent.
const settled24hSatsFromModel = (
  model: PublicPylonStatsModel,
): number | null => {
  const stats = statsFromModel(model)
  return stats?.publicRealSatsSettled24h ?? null
}

const forumLaunchStatusFromModel = (
  model: PublicForumLaunchStatusModel,
): PublicForumLaunchStatus | null =>
  model._tag === 'PublicForumLaunchStatusLoaded' ? model.status : null

const forumTipLeaderboardsFromModel = (
  model: PublicForumTipLeaderboardsModel,
): PublicForumTipLeaderboards | null =>
  model._tag === 'PublicForumTipLeaderboardsLoaded' ? model.leaderboards : null

const modelStatusText = (
  model:
    | PublicForumLaunchStatusModel
    | PublicForumTipLeaderboardsModel
    | PublicPylonStatsModel,
): string =>
  model._tag.endsWith('Loading')
    ? 'Loading'
    : model._tag.endsWith('Failed')
      ? 'Unavailable'
      : model._tag.endsWith('Loaded')
        ? 'Live'
        : 'Idle'

const modelErrorText = (
  model:
    | PublicForumLaunchStatusModel
    | PublicForumTipLeaderboardsModel
    | PublicPylonStatsModel,
): string | null =>
  model._tag.endsWith('Failed') && 'error' in model ? model.error : null

const panelClass = 'min-w-0 border border-[#242424] bg-[#050505] p-3 text-left'
const panelTitleClass =
  'm-0 text-[0.72rem] font-semibold uppercase leading-none text-[#f1efe8]'
const panelMetaClass = 'm-0 text-[0.68rem] leading-4 text-white/45'
const rowClass =
  'grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-t border-[#1d1d1d] py-2'
const rowLabelClass =
  'min-w-0 text-[0.72rem] font-medium leading-4 text-[#f1efe8]'
const rowDetailClass = 'mt-1 text-[0.66rem] leading-4 text-white/42'
const rowValueClass = 'text-right text-[0.72rem] leading-4 tabular-nums'

const githubLoginHref = '/login/github'

const githubIcon = (): Html => {
  const h = html<Message>()

  return h.svg(
    [
      h.AriaHidden(true),
      Ui.className<Message>('size-4 shrink-0'),
      h.Xmlns('http://www.w3.org/2000/svg'),
      h.ViewBox('0 0 24 24'),
      h.Fill('currentColor'),
    ],
    [
      h.path(
        [
          h.D(
            'M12 2C6.48 2 2 6.58 2 12.25c0 4.53 2.87 8.37 6.84 9.73.5.09.68-.22.68-.49 0-.24-.01-.88-.01-1.73-2.78.62-3.37-1.38-3.37-1.38-.45-1.19-1.11-1.51-1.11-1.51-.91-.64.07-.63.07-.63 1 .07 1.53 1.06 1.53 1.06.89 1.56 2.34 1.11 2.91.85.09-.66.35-1.11.63-1.37-2.22-.26-4.56-1.14-4.56-5.06 0-1.12.39-2.03 1.03-2.75-.1-.26-.45-1.3.1-2.71 0 0 .84-.28 2.75 1.05A9.3 9.3 0 0 1 12 6.97c.85 0 1.7.12 2.5.35 1.91-1.33 2.75-1.05 2.75-1.05.55 1.41.2 2.45.1 2.71.64.72 1.03 1.63 1.03 2.75 0 3.93-2.34 4.8-4.57 5.05.36.32.68.95.68 1.92 0 1.38-.01 2.5-.01 2.84 0 .27.18.59.69.49A10.22 10.22 0 0 0 22 12.25C22 6.58 17.52 2 12 2Z',
          ),
        ],
        [],
      ),
    ],
  )
}

export const githubLoginButton = (): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href(githubLoginHref),
      h.DataAttribute('login-button', 'github'),
      h.AriaBusy(false),
      h.Attribute(
        'onclick',
        "if(this.getAttribute('aria-disabled')==='true'){event.preventDefault();return false;}var label=this.querySelector('[data-login-label]');if(label){label.textContent='Logging in...';}this.setAttribute('aria-disabled','true');this.setAttribute('aria-busy','true');this.classList.add('pointer-events-none','opacity-75');",
      ),
      Ui.className<Message>(
        'inline-flex min-h-9 items-center justify-center gap-2 border border-[#333] bg-[#101010] px-3 py-2 text-[0.72rem] font-semibold text-[#f1efe8] hover:border-[#555] hover:bg-[#151515]',
      ),
    ],
    [
      githubIcon(),
      h.span([h.DataAttribute('login-label', '')], ['Log in with GitHub']),
    ],
  )
}

const toneClass = (tone: RowTone): string =>
  tone === 'good'
    ? 'text-[#9ad6b7]'
    : tone === 'warn'
      ? 'text-[#f3c27a]'
      : 'text-white/52'

const statusPill = (
  label: string,
  tone: RowTone = 'muted',
  attrs: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  const h = html<Message>()

  return h.span(
    [
      ...attrs,
      Ui.className<Message>(
        `inline-flex min-h-6 items-center border border-[#2a2a2a] px-2 py-1 text-[0.62rem] uppercase leading-none ${toneClass(tone)}`,
      ),
    ],
    [label],
  )
}

const panelHeader = (input: {
  title: string
  meta?: string
  status?: string
  tone?: RowTone
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'mb-2 flex min-w-0 flex-wrap items-start justify-between gap-2',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('min-w-0')],
        [
          h.h2([Ui.className<Message>(panelTitleClass)], [input.title]),
          input.meta === undefined
            ? null
            : h.p(
                [Ui.className<Message>(`${panelMetaClass} mt-1`)],
                [input.meta],
              ),
        ],
      ),
      input.status === undefined
        ? null
        : statusPill(input.status, input.tone ?? 'muted'),
    ],
  )
}

const metricRow = (input: {
  label: string
  value: string
  detail: string
  tone?: RowTone
}): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(rowClass)],
    [
      h.div(
        [Ui.className<Message>('min-w-0')],
        [
          h.div([Ui.className<Message>(rowLabelClass)], [input.label]),
          h.div([Ui.className<Message>(rowDetailClass)], [input.detail]),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            `${rowValueClass} ${toneClass(input.tone ?? 'muted')}`,
          ),
        ],
        [input.value],
      ),
    ],
  )
}

const endpointRow = (method: string, href: string, detail: string): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href(href),
      Ui.className<Message>(
        'grid grid-cols-[3rem_minmax(0,1fr)] gap-2 border-t border-[#1d1d1d] py-2 text-white/55 hover:text-[#f1efe8]',
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            'text-[0.65rem] font-semibold uppercase leading-4 text-white/38',
          ),
        ],
        [method],
      ),
      h.span(
        [Ui.className<Message>('min-w-0')],
        [
          h.span(
            [
              Ui.className<Message>(
                'block truncate text-[0.72rem] leading-4 text-current',
              ),
            ],
            [href],
          ),
          h.span(
            [
              Ui.className<Message>(
                'block text-[0.66rem] leading-4 text-white/38',
              ),
            ],
            [detail],
          ),
        ],
      ),
    ],
  )
}

export const endpointManifestPanel = (): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'Public reads are no-store. Mutations require the authority named by the route.',
        status: 'Public',
        title: 'Endpoint Manifest',
      }),
      endpointRow(
        'GET',
        '/.well-known/openagents.json',
        'Capability manifest for agents and operators.',
      ),
      endpointRow('GET', '/api/openapi.json', 'OpenAPI route contract.'),
      endpointRow(
        'GET',
        '/api/public/pylon-stats',
        'Pylon heartbeat and receipt-gated accepted-work counters.',
      ),
      endpointRow(
        'GET',
        '/api/public/product-promises',
        'Versioned promise states: live, scoped, gated, degraded, and planned.',
      ),
      endpointRow(
        'GET',
        '/api/forum/tip-leaderboards',
        'Public tip paid and settled evidence rows.',
      ),
      endpointRow(
        'GET',
        '/api/forum/launch-status',
        'Forum posting and tipping launch gates.',
      ),
      endpointRow(
        'GET',
        '/api/public/adjutant/activity',
        'Public Autopilot activity projection.',
      ),
    ],
  )
}

const relayRows = (stats: PublicPylonStats | null): ReadonlyArray<string> => {
  if (stats === null) {
    return []
  }

  return [
    ...(stats.hostedNexusRelayUrl === null ? [] : [stats.hostedNexusRelayUrl]),
    ...stats.recentPylons.flatMap(pylon => pylon.relayUrls),
  ].filter((value, index, values) => values.indexOf(value) === index)
}

const pubkeyRows = (stats: PublicPylonStats | null): ReadonlyArray<string> =>
  stats?.recentPylons
    .map(pylon => pylon.nostrPubkeyShort)
    .filter((value, index, values) => values.indexOf(value) === index) ?? []

export const nostrRelayPanel = (model: PublicPylonStatsModel): Html | null => {
  const h = html<Message>()
  const stats = statsFromModel(model)
  const relays = relayRows(stats)
  const pubkeys = pubkeyRows(stats)
  const error = modelErrorText(model)

  if (stats !== null && relays.length === 0) {
    return null
  }

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta:
          error ??
          'Pylon registrations may publish relay URLs and short public keys.',
        status: modelStatusText(model),
        tone: model._tag === 'PublicPylonStatsLoaded' ? 'good' : 'muted',
        title: 'Nostr Relay Configuration',
      }),
      metricRow({
        detail: 'Hosted relay plus registered Pylon relays.',
        label: 'Relay URLs',
        value:
          stats === null
            ? 'Unavailable'
            : relays.length === 0
              ? 'None returned'
              : formatNumber(relays.length),
      }),
      metricRow({
        detail: 'Short public-key labels from recent Pylon registrations.',
        label: 'Pubkeys',
        value:
          stats === null
            ? 'Unavailable'
            : pubkeys.length === 0
              ? 'None returned'
              : pubkeys.slice(0, 3).join(', '),
      }),
      h.div(
        [Ui.className<Message>('border-t border-[#1d1d1d] pt-2')],
        [
          h.p(
            [Ui.className<Message>(panelMetaClass)],
            [
              relays.length === 0
                ? 'No relay endpoint list is public in the current response.'
                : relays.slice(0, 3).join(' | '),
            ],
          ),
        ],
      ),
    ],
  )
}

const settledFeedConnectionLabel = (
  connection: SettledFeedModel['connection'],
): { label: string; tone: RowTone } =>
  connection === 'open'
    ? { label: 'Live', tone: 'good' }
    : connection === 'connecting'
      ? { label: 'Connecting', tone: 'muted' }
      : connection === 'failed' || connection === 'closed'
        ? { label: 'Offline', tone: 'warn' }
        : { label: 'Idle', tone: 'muted' }

// Live settled feed (openagents #5311). Renders the public-safe settled total /
// count / latest settlement straight from the streamed sync model, so the panel
// updates in real-time as real Bitcoin settlements stream — no reload. When the
// socket is offline this still shows the last-known totals from the snapshot
// fetch (graceful fallback).
export const liveSettledFeedPanel = (
  model: SettledFeedModel,
  settled24hSats: number | null,
): Html => {
  const connection = settledFeedConnectionLabel(model.connection)
  const latest = model.events[0]

  return html<Message>().section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'Public-safe settled events streamed over the sync engine.',
        status: connection.label,
        title: 'Live Settled Feed',
        tone: connection.tone,
      }),
      metricRow({
        detail: 'Receipt-backed real Bitcoin settlements, updated live.',
        label: 'Settled (total)',
        tone: model.totalSettledSats > 0 ? 'good' : 'muted',
        value: formatSats(model.totalSettledSats),
      }),
      metricRow({
        detail: 'Receipt-backed real Bitcoin settlements in the last 24 hours.',
        label: 'Settled (24h)',
        tone:
          settled24hSats === null
            ? 'muted'
            : settled24hSats > 0
              ? 'good'
              : 'muted',
        value: valueOrUnavailable(settled24hSats),
      }),
      metricRow({
        detail: 'Number of settled events on the live feed.',
        label: 'Settled count',
        value: formatNumber(model.totalSettledCount),
      }),
      metricRow({
        detail:
          latest === undefined
            ? 'No settlement streamed yet this session.'
            : `${latest.party} · ${latest.contributorRef}`,
        label: 'Latest settlement',
        tone: latest === undefined ? 'muted' : 'good',
        value: latest === undefined ? '—' : formatSats(latest.amountSats),
      }),
    ],
  )
}

export const pylonStatsPanel = (model: PublicPylonStatsModel): Html => {
  const stats = statsFromModel(model)
  const error = modelErrorText(model)
  const gate = stats?.earningLaunchGate

  return html<Message>().section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta:
          error ??
          (stats?.asOfLabel === null || stats?.asOfLabel === undefined
            ? 'Heartbeat freshness unavailable.'
            : `Fresh ${stats.asOfLabel}.`),
        status: modelStatusText(model),
        tone: stats?.available === true ? 'good' : 'muted',
        title: 'Pylon Stats',
      }),
      metricRow({
        detail: 'Heartbeat window. Not payment or earning evidence.',
        label: 'Online now',
        value:
          stats === null ? 'Unavailable' : formatNumber(stats.pylonsOnlineNow),
        tone: stats === null ? 'muted' : 'good',
      }),
      metricRow({
        detail: 'Seen in the last 24 hours.',
        label: 'Seen 24h',
        value:
          stats === null ? 'Unavailable' : formatNumber(stats.pylonsSeen24h),
      }),
      metricRow({
        detail: 'Pylons idle and waiting for assignments.',
        label: 'Assigned now',
        value:
          stats === null
            ? 'Unavailable'
            : formatNumber(stats.pylonsAssignmentReadyNow),
      }),
      metricRow({
        detail: gate?.stateLabel ?? 'Earning copy gate not loaded.',
        label: 'Earning gate',
        value:
          gate === undefined
            ? 'Unavailable'
            : gate.publicEarningCopyAllowed
              ? 'Ready'
              : 'Blocked',
        tone:
          gate?.publicEarningCopyAllowed === true
            ? 'good'
            : gate === undefined
              ? 'muted'
              : 'warn',
      }),
    ],
  )
}

const forumTotals = (
  leaderboards: PublicForumTipLeaderboards | null,
): {
  paid: number | null
  settled: number | null
  tips: number | null
} => {
  if (leaderboards === null) {
    return { paid: null, settled: null, tips: null }
  }

  return leaderboards.creators.reduce(
    (totals, row) => ({
      paid: (totals.paid ?? 0) + row.totalPaidSats,
      settled: (totals.settled ?? 0) + row.totalSettledSats,
      tips: (totals.tips ?? 0) + row.tipCount,
    }),
    { paid: 0, settled: 0, tips: 0 } as {
      paid: number
      settled: number
      tips: number
    },
  )
}

export const forumStatsPanel = (
  launchModel: PublicForumLaunchStatusModel,
  leaderboardModel: PublicForumTipLeaderboardsModel,
): Html => {
  const launch = forumLaunchStatusFromModel(launchModel)
  const leaderboards = forumTipLeaderboardsFromModel(leaderboardModel)
  const totals = forumTotals(leaderboards)
  const error = modelErrorText(leaderboardModel) ?? modelErrorText(launchModel)
  const loadedEmpty =
    leaderboards !== null &&
    leaderboards.creators.length === 0 &&
    leaderboards.posts.length === 0
  const paid = totals.paid
  const settled = totals.settled
  const settlementGap =
    paid === null || settled === null ? null : Math.max(0, paid - settled)

  return html<Message>().section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta:
          error ??
          (loadedEmpty
            ? 'No tip evidence rows returned by the public endpoint.'
            : 'Tip rows separate payer-side payment evidence from creator settlement.'),
        status:
          modelStatusText(leaderboardModel) === 'Live' &&
          modelStatusText(launchModel) === 'Live'
            ? 'Live'
            : 'Partial',
        tone: modelStatusText(leaderboardModel) === 'Live' ? 'good' : 'muted',
        title: 'Forum Stats',
      }),
      metricRow({
        detail: 'Payer-side payment evidence only; shown top creator rows.',
        label: 'Tip sats paid',
        value: valueOrUnavailable(paid),
        tone: paid === null ? 'muted' : 'good',
      }),
      metricRow({
        detail: 'Creator settlement evidence only; not inferred from payment.',
        label: 'Tip sats settled',
        value: valueOrUnavailable(settled),
        tone: settled === null ? 'muted' : 'good',
      }),
      metricRow({
        detail: 'Paid sats not yet settlement-backed in shown rows.',
        label: 'Settlement gap',
        value: valueOrUnavailable(settlementGap),
        tone: settlementGap === null || settlementGap === 0 ? 'muted' : 'warn',
      }),
      metricRow({
        detail: launch?.publicTipping.summary ?? 'Forum tip gate not loaded.',
        label: 'Tip gate',
        value: launch?.publicTipping.postTips ?? 'Unavailable',
        tone: launch?.publicTipping.postTips === 'ready' ? 'good' : 'warn',
      }),
      metricRow({
        detail: 'Endpoint returned creator rows, not global forum totals.',
        label: 'Tip count',
        value: totals.tips === null ? 'Unavailable' : formatNumber(totals.tips),
      }),
      metricRow({
        detail:
          'Active \ orange check badges bought by registered agents. Participation signal, not identity verification.',
        label: 'Orange checks sold',
        value:
          launch?.orangeChecksSold === null ||
          launch?.orangeChecksSold === undefined
            ? 'Unavailable'
            : formatNumber(launch.orangeChecksSold),
        tone:
          launch?.orangeChecksSold === null ||
          launch?.orangeChecksSold === undefined
            ? 'muted'
            : 'good',
      }),
    ],
  )
}

export const accountingPanel = (
  pylonModel: PublicPylonStatsModel,
  leaderboardModel: PublicForumTipLeaderboardsModel,
): Html => {
  const stats = statsFromModel(pylonModel)
  const leaderboards = forumTipLeaderboardsFromModel(leaderboardModel)
  const totals = forumTotals(leaderboards)
  const acceptedGate = stats?.nexusAcceptedWorkSettlementGate
  const acceptedPaid =
    stats !== null && acceptedGate?.publicPaidWorkTotalsAllowed === true
      ? (stats.nexusAcceptedWorkPayoutSatsPaidTotal ?? null)
      : null

  return html<Message>().section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'No dummy money values. Missing public-safe evidence is marked unavailable.',
        status: 'Evidence',
        title: 'Accounting Strip',
      }),
      metricRow({
        detail:
          'Receipt-backed Nexus/Treasury accepted-work payout evidence only.',
        label: 'Accepted-work sats paid',
        value: valueOrUnavailable(acceptedPaid),
        tone: acceptedPaid === null ? 'muted' : 'good',
      }),
      metricRow({
        detail: acceptedGate?.stateLabel ?? 'Accepted-work gate unavailable.',
        label: 'Accepted-work gate',
        value: acceptedGate?.state ?? 'Unavailable',
        tone: acceptedGate?.state === 'ready' ? 'good' : 'warn',
      }),
      metricRow({
        detail: 'Settled receipt references, not a sats amount.',
        label: 'Settlement refs',
        value:
          acceptedGate === undefined
            ? 'Unavailable'
            : formatNumber(acceptedGate.settledReceiptRefs.length),
      }),
      metricRow({
        detail:
          'Asset-bound ledger projection. Not a withdrawal promise or settled payout.',
        label: 'Revshare',
        value: 'Unavailable',
      }),
      metricRow({
        detail: 'Forum paid minus settled sats in shown leaderboard rows.',
        label: 'Forum paid vs settled',
        value:
          totals.paid === null || totals.settled === null
            ? 'Unavailable'
            : `${formatSats(totals.paid)} / ${formatSats(totals.settled)}`,
      }),
    ],
  )
}

export const copyBoundaryPanel = (): Html => {
  const h = html<Message>()
  const rows = [
    ['Tip sats paid', 'Payer-side payment evidence only.'],
    ['Tip sats settled', 'Creator settlement evidence only.'],
    [
      'Accepted-work sats paid',
      'Receipt-backed Nexus/Treasury accepted-work payout evidence.',
    ],
    ['Revshare', 'Asset-bound ledger projection, not a withdrawal promise.'],
    ['Settlement gap', 'Paid sats not yet settlement-backed.'],
  ] as const

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'Public copy boundaries for money and earning claims.',
        title: 'Claim Boundary',
      }),
      ...rows.map(([label, detail]) =>
        metricRow({
          detail,
          label,
          value: 'Bounded',
        }),
      ),
    ],
  )
}

export const publicAgentPath = (): Html => {
  const h = html<Message>()
  const instruction =
    'Read https://openagents.com/AGENTS.md. Do a dry-run first. Inspect the manifest and OpenAPI before planning any action.'

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      h.h2(
        [
          Ui.className<Message>(
            'm-0 text-center text-[1.05rem] font-semibold uppercase leading-none text-[#f1efe8]',
          ),
        ],
        ['I am an Agent'],
      ),
      h.textarea(
        [
          h.AriaLabel('Copyable agent instruction'),
          h.Readonly(true),
          h.Value(instruction),
          h.Rows(3),
          Ui.className<Message>(
            'mt-3 min-h-20 w-full resize-none border border-[#242424] bg-black p-3 text-[0.88rem] leading-6 text-white/75 outline-none',
          ),
        ],
        [],
      ),
      h.div(
        [Ui.className<Message>('mt-2 flex justify-center')],
        [
          h.button(
            [
              h.Type('button'),
              h.DataAttribute('copy-text', instruction),
              h.Attribute(
                'onclick',
                "navigator.clipboard?.writeText(this.dataset.copyText || '');this.textContent='Copied';setTimeout(()=>{this.textContent='Copy prompt';},1500);",
              ),
              Ui.className<Message>(
                'cursor-pointer border border-[#282828] bg-[#0b0b0b] px-3 py-2 text-[0.7rem] uppercase leading-none text-white/70 hover:border-[#444] hover:text-[#f1efe8]',
              ),
            ],
            ['Copy prompt'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'mt-3 flex flex-wrap items-center justify-center gap-2 text-[0.72rem] uppercase leading-none',
          ),
        ],
        [
          h.a(
            [
              h.Href('/.well-known/openagents.json'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['Capability manifest'],
          ),
          h.a(
            [
              h.Href('/api/openapi.json'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['OpenAPI'],
          ),
          h.a(
            [
              h.Href('/promises'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['Product promises'],
          ),
          h.a(
            [
              h.Href('/api/public/product-promises'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['Promises JSON'],
          ),
          h.a(
            [
              h.Href('/api/public/proof/otec'),
              Ui.className<Message>(
                'text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['Public proof'],
          ),
        ],
      ),
    ],
  )
}

export const heroIntroLinks = (): Html => {
  const h = html<Message>()
  const links = [
    ['/AGENTS.md', 'Connect an agent'],
    ['/api/public/pylon-stats', 'Pylon stats'],
    ['/forum', 'Forum'],
    ['/promises', 'Product promises'],
  ] as const

  return h.div(
    [
      Ui.className<Message>(
        'mt-3 flex flex-wrap items-center gap-2 text-[0.65rem] uppercase leading-none',
      ),
    ],
    links.map(([href, label]) =>
      h.a(
        [
          h.Href(href),
          Ui.className<Message>(
            'border border-[#282828] bg-[#0b0b0b] px-2.5 py-2 text-white/55 hover:border-[#444] hover:text-[#f1efe8]',
          ),
        ],
        [label],
      ),
    ),
  )
}

// "Khala Tokens Served" live counter (#6227). OpenAgents (the network) serves
// tokens powered by Khala (the engine), so the label is fixed. The number span
// carries `motionOdometerClass` and is KEYED on the value: when the poll brings
// a new total, Foldkit remounts the span and the `oa-odometer-roll` keyframe
// re-fires, animating the count-up between fetched totals. `tabular-nums` keeps
// the digits from jiggling as they change.
const khalaTokensServedFromModel = (
  model: PublicKhalaTokensServedModel,
): number | null =>
  model._tag === 'PublicKhalaTokensServedLoaded'
    ? model.served.tokensServed
    : null

// The shared display string for "Khala Tokens Served" (the same `formatNumber`
// thousands-separator formatter the hero counter uses), so the top-left landing
// pill and the hero counter render byte-identical numbers off the SAME model.
// Falls back to the em-dash placeholder when the value has not loaded yet.
export const formatKhalaTokensServed = (
  model: PublicKhalaTokensServedModel,
): string => {
  const tokensServed = khalaTokensServedFromModel(model)
  return tokensServed === null ? '—' : formatNumber(tokensServed)
}

export const khalaTokensServedCounter = (
  model: PublicKhalaTokensServedModel,
): Html => {
  const h = html<Message>()
  const display = formatKhalaTokensServed(model)
  const live = model._tag === 'PublicKhalaTokensServedLoaded'

  return h.section(
    [
      h.DataAttribute('counter', 'khala-tokens-served'),
      Ui.className<Message>(
        'flex flex-col items-center gap-2 border border-[#242424] bg-[#030303] px-4 py-6 text-center sm:py-7',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex items-center gap-2 text-[0.66rem] uppercase leading-none tracking-[0.08em] text-white/45',
          ),
        ],
        [
          h.span(
            [
              h.DataAttribute(
                'status',
                live ? 'live' : 'pending',
              ),
              Ui.className<Message>(
                `inline-block h-1.5 w-1.5 rounded-full ${
                  live ? 'bg-[#00c853]' : 'bg-white/30'
                }`,
              ),
            ],
            [],
          ),
          h.span([], ['Khala Tokens Served']),
        ],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 text-[2.4rem] font-semibold leading-none tabular-nums text-[#f1efe8] sm:text-[3.25rem]',
          ),
        ],
        [
          // `data-value` carries the AUTHORITATIVE target string (read by tests +
          // the headless proof). `data-counter-display` marks the node the
          // client count-up controller eases between the server's ≤3/sec updates
          // (#6324): on boot `installKhalaTokensServedCountUp` (entry.ts) attaches
          // a MutationObserver that animates this node's text from the currently
          // shown value up to each new `data-value`, capped + reduced-motion-safe.
          // The node is NOT keyed on the value, so it persists across updates and
          // the controller can ease instead of the vdom snapping it. The text
          // child stays the target so SSR / no-JS / headless still show the right
          // number; the controller only overrides the text frame-by-frame when
          // motion is available.
          h.span(
            [
              h.DataAttribute('value', display),
              h.DataAttribute('counter-display', 'khala-tokens-served'),
              h.Class(motionOdometerClass),
            ],
            [display],
          ),
        ],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 text-[0.66rem] leading-4 text-white/35',
          ),
        ],
        ['Total input + output tokens served across the network, powered by Khala.'],
      ),
    ],
  )
}

// "Khala Tokens Served" history graph (#6227): a small hand-rolled SVG bar
// chart of tokens-per-day for the last 30 days, sitting next to the live
// counter on /stats. Public-safe: the series is bare day + sum. No chart
// library — just scaled rect bars (via the foldkit h.rect builder) in the brand
// positive-green over the dark panel. Bars are visible by default (no
// class-gated reveal), so the chart is reduced-motion-safe and renders in
// headless/SSR contexts. Accessibility: the chart root carries role="img" plus
// an aria-label summary, each bar a title element, and a visually-hidden
// table-like text fallback lists every day + value.

const CHART_VIEW_WIDTH = 320
const CHART_VIEW_HEIGHT = 96
const CHART_BASELINE_Y = CHART_VIEW_HEIGHT - 1

const compactNumberFormatter = new Intl.NumberFormat('en-US', {
  notation: 'compact',
  maximumFractionDigits: 1,
})

const formatCompactNumber = (value: number): string =>
  compactNumberFormatter.format(value)

const historyChartHeading = (live: boolean, label: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'flex items-center gap-2 text-[0.66rem] uppercase leading-none tracking-[0.08em] text-white/45',
      ),
    ],
    [
      h.span(
        [
          h.DataAttribute('status', live ? 'live' : 'pending'),
          Ui.className<Message>(
            `inline-block h-1.5 w-1.5 rounded-full ${
              live ? 'bg-[#00c853]' : 'bg-white/30'
            }`,
          ),
        ],
        [],
      ),
      h.span([], [label]),
    ],
  )
}

const historyChartShell = (
  live: boolean,
  body: Html,
  caption: string,
  title = 'Tokens Served / Day',
): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('chart', 'khala-tokens-served-history'),
      Ui.className<Message>(
        'flex flex-col gap-3 border border-[#242424] bg-[#030303] px-4 py-5',
      ),
    ],
    [
      historyChartHeading(live, title),
      body,
      h.p(
        [Ui.className<Message>('m-0 text-[0.66rem] leading-4 text-white/35')],
        [caption],
      ),
    ],
  )
}

const percentFormatter = new Intl.NumberFormat('en-US', {
  maximumFractionDigits: 1,
  minimumFractionDigits: 0,
})

const formatPercent = (value: number): string =>
  `${percentFormatter.format(Math.max(0, value))}%`

const historyChartPlaceholder = (label: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Role('status'),
      Ui.className<Message>(
        'flex h-[96px] items-center justify-center text-[0.7rem] text-white/35',
      ),
    ],
    [label],
  )
}

// The visually-hidden, screen-reader / headless text fallback: every day and
// its served-token count, so the data is never locked inside the SVG.
const historyTextFallback = (
  series: ReadonlyArray<PublicKhalaTokensServedHistoryPoint>,
): Html => {
  const h = html<Message>()

  return h.ul(
    [Ui.className<Message>('sr-only')],
    series.map(point =>
      h.li([], [`${point.day}: ${formatNumber(point.tokensServed)} tokens`]),
    ),
  )
}

const historyChartBars = (
  series: ReadonlyArray<PublicKhalaTokensServedHistoryPoint>,
): Html => {
  const h = html<Message>()
  const maxTokens = series.reduce(
    (max, point) => (point.tokensServed > max ? point.tokensServed : max),
    0,
  )
  // Even gaps; bars take ~70% of each slot. With <=30 points the gap reads as
  // clear separation without crowding.
  const slot = CHART_VIEW_WIDTH / series.length
  const barWidth = Math.max(1, slot * 0.7)

  const bars = series.map((point, index) => {
    // A zero-token day still shows a 1px sliver so the day is not invisible.
    const heightRatio = maxTokens === 0 ? 0 : point.tokensServed / maxTokens
    const barHeight = Math.max(
      point.tokensServed > 0 ? 2 : 0,
      heightRatio * (CHART_VIEW_HEIGHT - 4),
    )
    const x = index * slot + (slot - barWidth) / 2
    const y = CHART_BASELINE_Y - barHeight

    return h.rect(
      [
        h.Attribute('x', x.toFixed(2)),
        h.Attribute('y', y.toFixed(2)),
        h.Attribute('width', barWidth.toFixed(2)),
        h.Attribute('height', barHeight.toFixed(2)),
        h.Attribute('rx', '0.5'),
        h.Fill('#00c853'),
        h.Attribute('fill-opacity', point.tokensServed > 0 ? '0.85' : '0.25'),
      ],
      [
        h.title(
          [],
          [`${point.day}: ${formatNumber(point.tokensServed)} tokens`],
        ),
      ],
    )
  })

  const ariaLabel = `Tokens served per day for the last ${series.length} ${
    series.length === 1 ? 'day' : 'days'
  }. Peak ${formatNumber(maxTokens)} tokens in a day.`

  return h.div(
    [],
    [
      h.svg(
        [
          h.Role('img'),
          h.AriaLabel(ariaLabel),
          Ui.className<Message>('h-[96px] w-full'),
          h.Xmlns('http://www.w3.org/2000/svg'),
          h.ViewBox(`0 0 ${CHART_VIEW_WIDTH} ${CHART_VIEW_HEIGHT}`),
          h.Attribute('preserveAspectRatio', 'none'),
        ],
        [
          // Baseline rule, faint, on-brand.
          h.line(
            [
              h.Attribute('x1', '0'),
              h.Attribute('y1', String(CHART_BASELINE_Y)),
              h.Attribute('x2', String(CHART_VIEW_WIDTH)),
              h.Attribute('y2', String(CHART_BASELINE_Y)),
              h.Attribute('stroke', '#242424'),
              h.Attribute('stroke-width', '1'),
            ],
            [],
          ),
          ...bars,
        ],
      ),
      historyTextFallback(series),
    ],
  )
}

export const khalaTokensServedHistoryChart = (
  model: PublicKhalaTokensServedHistoryModel,
): Html =>
  M.value(model).pipe(
    M.tagsExhaustive({
      PublicKhalaTokensServedHistoryIdle: () =>
        historyChartShell(
          false,
          historyChartPlaceholder('Waiting for data…'),
          'Daily input + output tokens served across the network in America/Chicago.',
        ),
      PublicKhalaTokensServedHistoryLoading: () =>
        historyChartShell(
          false,
          historyChartPlaceholder('Loading history…'),
          'Daily input + output tokens served across the network in America/Chicago.',
        ),
      PublicKhalaTokensServedHistoryFailed: () =>
        historyChartShell(
          false,
          historyChartPlaceholder('History unavailable.'),
          'Daily input + output tokens served across the network in America/Chicago.',
        ),
      PublicKhalaTokensServedHistoryLoaded: ({ history }) =>
        Array.match(history.series, {
          onEmpty: () =>
            historyChartShell(
              true,
              historyChartPlaceholder('No tokens served yet.'),
              `Daily input + output tokens served across the network in ${history.timezone}.`,
            ),
          onNonEmpty: series =>
            historyChartShell(
              true,
              historyChartBars(series),
              `Daily input + output tokens served across the network in ${history.timezone}. Last ${
                series.length
              } ${series.length === 1 ? 'day' : 'days'}, peak ${formatCompactNumber(
                series.reduce(
                  (max, point) =>
                    point.tokensServed > max ? point.tokensServed : max,
                  0,
                ),
              )} in a day.`,
            ),
        }),
    }),
  )

const modelMixPlaceholder = (label: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Role('status'),
      Ui.className<Message>(
        'flex min-h-[9rem] items-center justify-center text-[0.7rem] text-white/35',
      ),
    ],
    [label],
  )
}

const modelMixRows = (
  groups: ReadonlyArray<PublicKhalaTokensServedModelMixFamily>,
): Html => {
  const h = html<Message>()

  return h.ul(
    [Ui.className<Message>('m-0 grid list-none gap-2 p-0')],
    groups.map(group =>
      h.li(
        [
          Ui.className<Message>(
            'grid gap-1 border-t border-[#1d1d1d] pt-2 first:border-t-0 first:pt-0',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3',
              ),
            ],
            [
              h.span(
                [
                  Ui.className<Message>(
                    'min-w-0 text-[0.72rem] font-medium leading-4 text-[#f1efe8]',
                  ),
                ],
                [group.label],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'text-right text-[0.72rem] leading-4 tabular-nums text-[#9ad6b7]',
                  ),
                ],
                [formatPercent(group.pct)],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>('h-1.5 overflow-hidden bg-[#111]'),
            ],
            [
              h.div(
                [
                  Ui.className<Message>('h-full bg-[#00c853]'),
                  h.Attribute(
                    'style',
                    `width: ${Math.max(0, Math.min(100, group.pct)).toFixed(2)}%;`,
                  ),
                ],
                [],
              ),
            ],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-[0.66rem] leading-4 text-white/42')],
            [
              `${formatNumber(group.tokens)} tokens across ${formatNumber(
                group.reqs,
              )} events`,
            ],
          ),
        ],
      ),
    ),
  )
}

export const khalaTokensServedModelMixPanel = (
  model: PublicKhalaTokensServedModelMixModel,
): Html =>
  M.value(model).pipe(
    M.tagsExhaustive({
      PublicKhalaTokensServedModelMixIdle: () =>
        historyChartShell(
          false,
          modelMixPlaceholder('Waiting for model mix…'),
          'Canonical model-family mix from aggregate token usage rows.',
          'Model Family Mix',
        ),
      PublicKhalaTokensServedModelMixLoading: () =>
        historyChartShell(
          false,
          modelMixPlaceholder('Loading model mix…'),
          'Canonical model-family mix from aggregate token usage rows.',
          'Model Family Mix',
        ),
      PublicKhalaTokensServedModelMixFailed: () =>
        historyChartShell(
          false,
          modelMixPlaceholder('Model mix unavailable.'),
          'Canonical model-family mix from aggregate token usage rows.',
          'Model Family Mix',
        ),
      PublicKhalaTokensServedModelMixLoaded: ({ mix }) =>
        Array.match(mix.groups, {
          onEmpty: () =>
            historyChartShell(
              true,
              modelMixPlaceholder('No model-family rows yet.'),
              `Canonical model-family mix for ${mix.window}.`,
              'Model Family Mix',
            ),
          onNonEmpty: groups =>
            historyChartShell(
              true,
              modelMixRows(groups),
              `Canonical model-family mix for ${mix.window}. Total ${formatNumber(
                mix.totalTokens,
              )} tokens served.`,
              'Model Family Mix',
            ),
        }),
    }),
  )

// The paired "Khala Tokens Served" surface: the live counter and the
// tokens-per-day history chart side by side on wide viewports, stacked on
// narrow ones. Used on both the homepage and /stats.
export const khalaTokensServedPanel = (
  counter: PublicKhalaTokensServedModel,
  history: PublicKhalaTokensServedHistoryModel,
  modelMix?: PublicKhalaTokensServedModelMixModel,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        modelMix === undefined
          ? 'grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)]'
          : 'grid gap-3 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.25fr)_minmax(0,1fr)]',
      ),
    ],
    [
      khalaTokensServedCounter(counter),
      khalaTokensServedHistoryChart(history),
      ...(modelMix === undefined
        ? []
        : [khalaTokensServedModelMixPanel(modelMix)]),
    ],
  )
}

const topStatTile = (label: string, value: string, detail: string): Html => {
  const h = html<Message>()
  return h.div(
    [
      Ui.className<Message>(
        'border border-[#242424] bg-[#0b0b0b] p-3 text-center',
      ),
    ],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 text-[1.3rem] font-semibold leading-none text-[#f1efe8]',
          ),
        ],
        [value],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 mt-2 text-[0.66rem] uppercase leading-none text-white/55',
          ),
        ],
        [label],
      ),
      h.p(
        [Ui.className<Message>('m-0 mt-1 text-[0.6rem] leading-4 text-white/35')],
        [detail],
      ),
    ],
  )
}

const topStatsStrip = (input: HomeViewInput): Html => {
  const h = html<Message>()
  const stats = statsFromModel(input.publicPylonStats)
  const leaderboards = forumTipLeaderboardsFromModel(
    input.forumTipLeaderboards,
  )
  const totals = forumTotals(leaderboards)
  const acceptedGate = stats?.nexusAcceptedWorkSettlementGate
  const acceptedPaid =
    stats !== null && acceptedGate?.publicPaidWorkTotalsAllowed === true
      ? (stats.nexusAcceptedWorkPayoutSatsPaidTotal ?? null)
      : null
  const numberOrUnavailable = (value: number | null | undefined): string =>
    value === null || value === undefined ? '—' : formatNumber(value)

  return h.section(
    [Ui.className<Message>('grid gap-2')],
    [
      h.div(
        [
          Ui.className<Message>(
            'grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5',
          ),
        ],
        [
          topStatTile(
            'Pylons online',
            numberOrUnavailable(stats?.pylonsOnlineNow),
            'Live heartbeats now.',
          ),
          topStatTile(
            'Pylons seen 24h',
            numberOrUnavailable(stats?.pylonsSeen24h),
            'Distinct devices in 24h.',
          ),
          topStatTile(
            'Tip sats paid',
            totals.paid === null ? '—' : formatNumber(totals.paid),
            'Payer-side evidence.',
          ),
          topStatTile(
            'Tip sats settled',
            totals.settled === null ? '—' : formatNumber(totals.settled),
            'Creator settlement evidence.',
          ),
          topStatTile(
            'Accepted-work sats',
            acceptedPaid === null ? '—' : formatNumber(acceptedPaid),
            'Receipt-backed payouts.',
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex justify-end')],
        [
          h.a(
            [
              h.Href('/stats-old'),
              Ui.className<Message>(
                'text-[0.66rem] uppercase leading-none text-white/45 underline-offset-2 hover:text-white/70 hover:underline',
              ),
            ],
            ['Detailed network stats →'],
          ),
        ],
      ),
    ],
  )
}

export const view = (input: HomeViewInput): Html => {
  const h = html<Message>()

  return Ui.container<Message>([
    h.main(
      [
        Ui.className<Message>(
          'bg-black px-3 py-4 font-mono text-[#f1efe8] sm:px-4 lg:px-6',
        ),
      ],
      [
        h.div(
          [Ui.className<Message>('mx-auto grid w-full max-w-3xl gap-3')],
          [
            h.section(
              [
                Ui.className<Message>(
                  'grid content-start gap-3 border border-[#242424] bg-[#030303] p-4',
                ),
              ],
              [
                h.div(
                  [
                    Ui.className<Message>(
                      'grid gap-3 border-b border-[#1d1d1d] pb-3 sm:grid-cols-[minmax(0,1fr)_auto]',
                    ),
                  ],
                  [
                    h.div(
                      [Ui.className<Message>('min-w-0')],
                      [
                        h.p(
                          [
                            Ui.className<Message>(
                              'm-0 text-[0.68rem] uppercase leading-4 text-white/45',
                            ),
                          ],
                          ['openagents.com'],
                        ),
                        h.h1(
                          [
                            Ui.className<Message>(
                              'm-0 mt-1 text-[1.65rem] font-semibold leading-[1.05] text-[#f1efe8] sm:text-[2.15rem]',
                            ),
                          ],
                          ['OpenAgents is the agent network.'],
                        ),
                        h.p(
                          [
                            Ui.className<Message>(
                              'm-0 mt-2 max-w-[64ch] text-[0.78rem] leading-5 text-white/55',
                            ),
                          ],
                          [
                            'Connect your agent, inspect the live promises, and follow the paths for earning bitcoin through useful public work.',
                          ],
                        ),
                        heroIntroLinks(),
                      ],
                    ),
                    h.div(
                      [
                        Ui.className<Message>(
                          'flex items-start justify-start sm:justify-end',
                        ),
                      ],
                      [githubLoginButton()],
                    ),
                  ],
                ),
                publicAgentPath(),
              ],
            ),
            khalaTokensServedPanel(
              input.publicKhalaTokensServed,
              input.publicKhalaTokensServedHistory,
            ),
            topStatsStrip(input),
            liveSettledFeedPanel(
              input.settledFeed,
              settled24hSatsFromModel(input.publicPylonStats),
            ),
          ],
        ),
      ],
    ),
  ])
}
