import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type { PublicPylonStats, PublicPylonStatsModel } from '../model'

const numberFormatter = new Intl.NumberFormat('en-US')

const formatNumber = (value: number): string => numberFormatter.format(value)

const statsFromModel = (
  model: PublicPylonStatsModel,
): PublicPylonStats | null =>
  model._tag === 'PublicPylonStatsLoaded' ? model.stats : null

const statsStatusText = (model: PublicPylonStatsModel): string =>
  model._tag === 'PublicPylonStatsLoading'
    ? 'Loading'
    : model._tag === 'PublicPylonStatsFailed'
      ? 'Unavailable'
      : model._tag === 'PublicPylonStatsLoaded' && model.stats.available
        ? 'Live'
        : 'Unavailable'

const statValue = (
  stats: PublicPylonStats | null,
  pick: (stats: PublicPylonStats) => string,
): string => (stats === null ? '-' : pick(stats))

const pylonStat = (label: string, value: string, detail: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid min-h-[5.75rem] content-between gap-2 border border-[#222] bg-[#0d0d0d] p-3 text-left',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'truncate text-[0.7rem] uppercase text-white/45',
          ),
        ],
        [label],
      ),
      h.div(
        [
          Ui.className<Message>(
            'min-w-0 truncate text-2xl font-semibold tabular-nums text-[#f1efe8]',
          ),
        ],
        [value],
      ),
      h.div(
        [Ui.className<Message>('min-h-4 text-[0.72rem] text-white/45')],
        [detail],
      ),
    ],
  )
}

const pylonStatsStrip = (model: PublicPylonStatsModel): Html => {
  const h = html<Message>()
  const stats = statsFromModel(model)
  const status = statsStatusText(model)
  const freshness =
    stats === null
      ? 'Freshness pending'
      : stats.available
        ? `Fresh ${stats.asOfLabel ?? 'recently'}`
        : (stats.error ?? 'Stats unavailable')

  return h.section(
    [
      Ui.className<Message>(
        'grid w-full max-w-3xl gap-3 border-t border-[#222] pt-5',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-2 text-left',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 text-sm font-semibold text-[#f1efe8]',
                  ),
                ],
                ['Live Pylons'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-[0.75rem] text-white/45')],
                [freshness],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'min-h-6 border border-[#242424] px-2 py-1 text-[0.7rem] uppercase text-white/50',
              ),
            ],
            [status],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5',
          ),
        ],
        [
          pylonStat(
            'Online now',
            statValue(stats, value => formatNumber(value.pylonsOnlineNow)),
            'Heartbeat window',
          ),
          pylonStat(
            'Seen in 24h',
            statValue(stats, value => formatNumber(value.pylonsSeen24h)),
            'Recent check-ins',
          ),
          pylonStat(
            'Wallet ready',
            statValue(stats, value => formatNumber(value.pylonsWalletReadyNow)),
            'Public readiness',
          ),
          pylonStat(
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
          pylonStat(
            'Version floor',
            statValue(stats, value => `v${value.minimumClientVersion}+`),
            'Pylon line',
          ),
        ],
      ),
    ],
  )
}

export const view = (publicPylonStats: PublicPylonStatsModel): Html => {
  const h = html<Message>()

  return Ui.container<Message>([
    h.main(
      [
        Ui.className<Message>(
          'grid min-h-dvh place-items-center px-4 text-center',
        ),
      ],
      [
        h.div(
          [
            Ui.className<Message>(
              'grid w-full justify-items-center gap-5 text-sm leading-6 text-white/70',
            ),
          ],
          [
            h.p(
              [Ui.className<Message>('m-0 text-white')],
              [
                h.span([Ui.className<Message>('font-bold')], ['Autopilot']),
                ' is a cloud coding agent.',
              ],
            ),
            h.p(
              [Ui.className<Message>('m-0 py-5 text-white/50')],
              ['Now in beta! Get a free coding task back within 24 hours.'],
            ),
            pylonStatsStrip(publicPylonStats),
          ],
        ),
      ],
    ),
  ])
}
