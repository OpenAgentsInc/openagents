import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  PublicForumLaunchStatus,
  PublicForumLaunchStatusModel,
  PublicForumTipLeaderboards,
  PublicForumTipLeaderboardsModel,
  PublicPylonStats,
  PublicPylonStatsModel,
} from '../model'

type HomeViewInput = {
  forumLaunchStatus: PublicForumLaunchStatusModel
  forumTipLeaderboards: PublicForumTipLeaderboardsModel
  publicPylonStats: PublicPylonStatsModel
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

const forumLaunchStatusFromModel = (
  model: PublicForumLaunchStatusModel,
): PublicForumLaunchStatus | null =>
  model._tag === 'PublicForumLaunchStatusLoaded' ? model.status : null

const forumTipLeaderboardsFromModel = (
  model: PublicForumTipLeaderboardsModel,
): PublicForumTipLeaderboards | null =>
  model._tag === 'PublicForumTipLeaderboardsLoaded'
    ? model.leaderboards
    : null

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

const panelClass =
  'min-w-0 border border-[#242424] bg-[#050505] p-3 text-left'
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

const githubLoginButton = (): Html => {
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
            : h.p([Ui.className<Message>(`${panelMetaClass} mt-1`)], [
                input.meta,
              ]),
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
            [Ui.className<Message>('block text-[0.66rem] leading-4 text-white/38')],
            [detail],
          ),
        ],
      ),
    ],
  )
}

const codeBlock = (code: string): Html => {
  const h = html<Message>()

  return h.pre(
    [
      Ui.className<Message>(
        'm-0 overflow-x-auto border-t border-[#1d1d1d] bg-black p-3 text-[0.67rem] leading-5 text-white/55',
      ),
    ],
    [h.code([], [code])],
  )
}

const endpointManifestPanel = (): Html => {
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

const taskDispatchPanel = (): Html => {
  const curl = [
    'curl -X POST https://openagents.com/api/agents/goals \\',
    '  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \\',
    '  -H "Content-Type: application/json" \\',
    '  -d \'{"objective":"Audit docs/refactor and report blockers","tokenBudget":12000}\'',
  ].join('\n')

  return html<Message>().section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'Task dispatch is token-bound. Public docs do not grant write authority.',
        status: 'Token',
        title: 'cURL Task Dispatch',
      }),
      codeBlock(curl),
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

const nostrRelayPanel = (model: PublicPylonStatsModel): Html => {
  const h = html<Message>()
  const stats = statsFromModel(model)
  const relays = relayRows(stats)
  const pubkeys = pubkeyRows(stats)
  const error = modelErrorText(model)

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
          h.p([Ui.className<Message>(panelMetaClass)], [
            relays.length === 0
              ? 'No relay endpoint list is public in the current response.'
              : relays.slice(0, 3).join(' | '),
          ]),
        ],
      ),
    ],
  )
}

const pylonStatsPanel = (model: PublicPylonStatsModel): Html => {
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
        detail: 'Wallet receive readiness, not spend authority.',
        label: 'Wallet ready',
        value:
          stats === null
            ? 'Unavailable'
            : formatNumber(stats.pylonsWalletReadyNow),
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

const forumStatsPanel = (
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
        tone:
          modelStatusText(leaderboardModel) === 'Live' ? 'good' : 'muted',
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
        value:
          totals.tips === null ? 'Unavailable' : formatNumber(totals.tips),
      }),
    ],
  )
}

const accountingPanel = (
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

const copyBoundaryPanel = (): Html => {
  const h = html<Message>()
  const rows = [
    [
      'Tip sats paid',
      'Payer-side payment evidence only.',
    ],
    [
      'Tip sats settled',
      'Creator settlement evidence only.',
    ],
    [
      'Accepted-work sats paid',
      'Receipt-backed Nexus/Treasury accepted-work payout evidence.',
    ],
    [
      'Revshare',
      'Asset-bound ledger projection, not a withdrawal promise.',
    ],
    [
      'Settlement gap',
      'Paid sats not yet settlement-backed.',
    ],
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

const publicAgentPath = (): Html => {
  const h = html<Message>()
  const instruction =
    'Read https://openagents.com/AGENTS.md. Do a dry-run first. Inspect the manifest and OpenAPI before planning any action.'

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      h.h2(
        [
          Ui.className<Message>(
            'm-0 text-center text-[0.72rem] font-semibold uppercase leading-none text-[#f1efe8]',
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
            'mt-3 min-h-16 w-full resize-none border border-[#242424] bg-black p-2 text-[0.66rem] leading-4 text-white/55 outline-none',
          ),
        ],
        [],
      ),
      h.div(
        [
          Ui.className<Message>(
            'mt-3 flex flex-wrap items-center justify-center gap-2 text-[0.65rem] uppercase leading-none',
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

export const view = (input: HomeViewInput): Html => {
  const h = html<Message>()

  return Ui.container<Message>([
    h.main(
      [
        Ui.className<Message>(
          'min-h-dvh bg-black px-3 py-4 font-mono text-[#f1efe8] sm:px-4 lg:px-6',
        ),
      ],
      [
        h.div(
          [
            Ui.className<Message>(
              'mx-auto grid w-full max-w-7xl gap-3 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]',
            ),
          ],
          [
            h.section(
              [
                Ui.className<Message>(
                  'grid content-start gap-3 border border-[#242424] bg-[#030303] p-3',
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
                          ['Autopilot is a cloud coding agent.'],
                        ),
                        h.p(
                          [
                            Ui.className<Message>(
                              'm-0 mt-2 max-w-[64ch] text-[0.78rem] leading-5 text-white/55',
                            ),
                          ],
                          [
                            'Now in beta! Get a free coding task back within 24 hours.',
                          ],
                        ),
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
                endpointManifestPanel(),
                taskDispatchPanel(),
                nostrRelayPanel(input.publicPylonStats),
                publicAgentPath(),
              ],
            ),
            h.section(
              [Ui.className<Message>('grid content-start gap-3')],
              [
                pylonStatsPanel(input.publicPylonStats),
                forumStatsPanel(
                  input.forumLaunchStatus,
                  input.forumTipLeaderboards,
                ),
                accountingPanel(
                  input.publicPylonStats,
                  input.forumTipLeaderboards,
                ),
                copyBoundaryPanel(),
              ],
            ),
          ],
        ),
      ],
    ),
  ])
}
