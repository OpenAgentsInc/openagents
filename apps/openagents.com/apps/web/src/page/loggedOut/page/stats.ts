import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type { HomeViewInput } from './home'
import {
  accountingPanel,
  copyBoundaryPanel,
  endpointManifestPanel,
  forumStatsPanel,
  khalaTokensServedHeaderCounter,
  khalaTokensServedChannelMixPanel,
  khalaTokensServedHistoryChart,
  khalaTokensServedModelMixPanel,
  nostrRelayPanel,
  pylonStatsPanel,
} from './home'

// Public /stats page: the full evidence
// panels - pylon counters, forum tipping, accounting strip, claim
// boundaries, endpoint manifest, and relay configuration - in one
// place, leaving the homepage to lead with the agent instructions.

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
          [Ui.className<Message>('mx-auto grid w-full max-w-7xl gap-3')],
          [
            h.div(
              [Ui.className<Message>('flex')],
              [
                h.a(
                  [
                    h.Href('/'),
                    Ui.className<Message>(
                      'inline-flex items-center border border-[#282828] bg-[#0b0b0b] px-2.5 py-2 text-[0.65rem] uppercase leading-none text-white/55 hover:border-[#444] hover:text-[#f1efe8]',
                    ),
                  ],
                  ['Home'],
                ),
              ],
            ),
            h.div(
              [
                Ui.className<Message>(
                  'khala-panel grid gap-4 border border-[#1d2733] bg-[#030712] p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-stretch',
                ),
              ],
              [
                h.div(
                  [
                    Ui.className<Message>(
                      'flex min-w-0 flex-col justify-center',
                    ),
                  ],
                  [
                    h.h1(
                      [
                        Ui.className<Message>(
                          'm-0 text-[1.4rem] font-semibold leading-[1.1] text-[#f1efe8]',
                        ),
                      ],
                      ['Network Stats'],
                    ),
                    h.p(
                      [
                        Ui.className<Message>(
                          'm-0 mt-1.5 max-w-[64ch] text-[0.74rem] leading-5 text-white/55',
                        ),
                      ],
                      [
                        'Live public-safe evidence: receipt-backed counters, launch gates, and claim boundaries. No dummy values; missing evidence is marked unavailable.',
                      ],
                    ),
                  ],
                ),
                khalaTokensServedHeaderCounter(input.publicKhalaTokensServed),
              ],
            ),
            h.div(
              [
                Ui.className<Message>(
                  'grid gap-3 lg:grid-cols-[minmax(0,1.75fr)_minmax(18rem,0.85fr)]',
                ),
              ],
              [
                khalaTokensServedHistoryChart(
                  input.publicKhalaTokensServedHistory,
                  'launch-window',
                  input.publicKhalaTokensServedHistoryGraphMetric ?? 'daily',
                  true,
                ),
                h.section(
                  [Ui.className<Message>('grid content-start gap-3')],
                  [
                    khalaTokensServedModelMixPanel(
                      input.publicKhalaTokensServedModelMix,
                    ),
                    khalaTokensServedChannelMixPanel(
                      input.publicKhalaTokensServedChannelMix,
                    ),
                  ],
                ),
              ],
            ),
            h.div(
              [
                Ui.className<Message>(
                  'grid gap-3 xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
                ),
              ],
              [
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
                  ],
                ),
                h.section(
                  [Ui.className<Message>('grid content-start gap-3')],
                  [
                    copyBoundaryPanel(),
                    endpointManifestPanel(),
                    ...[nostrRelayPanel(input.publicPylonStats)].filter(
                      (panel): panel is Html => panel !== null,
                    ),
                  ],
                ),
              ],
            ),
          ],
        ),
      ],
    ),
  ])
}
