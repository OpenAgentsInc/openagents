import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { gymOssRouter, mirrorCodeRouter } from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'

type AnchorAttr = Parameters<ReturnType<typeof html<Message>>['a']>[0][number]

type LeaderboardRow = Readonly<{
  rank: string
  bucket: string
  target: string
  state: string
  passRate: string
  tokens: string
  proof: string
}>

const mirrorCodeRows: ReadonlyArray<LeaderboardRow> = [
  {
    rank: '01',
    bucket: 'S',
    target: 'cal',
    state: 'queued / smoke',
    passRate: 'pending',
    tokens: 'exact rows required',
    proof: 'public run projection',
  },
  {
    rank: '02',
    bucket: 'M',
    target: 'gotree',
    state: 'awaiting owner',
    passRate: 'not published',
    tokens: 'exact rows required',
    proof: 'owner-gated run',
  },
  {
    rank: '03',
    bucket: 'L',
    target: 'ruff',
    state: 'awaiting owner',
    passRate: 'not published',
    tokens: 'exact rows required',
    proof: 'owner-gated run',
  },
]

const tabLink = (
  label: string,
  href: string,
  active: boolean,
  attrs: ReadonlyArray<AnchorAttr> = [],
): Html => {
  const h = html<Message>()

  return h.a(
    [
      ...attrs,
      h.Href(href),
      Ui.className<Message>(
        active
          ? 'border border-[#ffb400]/45 bg-[#130e03] px-3 py-2 text-[0.78rem] font-semibold text-[#ffd884]'
          : 'border border-white/10 bg-black px-3 py-2 text-[0.78rem] font-semibold text-white/60 hover:border-white/20 hover:text-white',
      ),
      ...(active ? [h.AriaCurrent('page')] : []),
    ],
    [label],
  )
}

const metric = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1 border border-white/10 bg-black p-3')],
    [
      h.span([Ui.className<Message>('text-[0.72rem] text-white/45')], [label]),
      h.span([Ui.className<Message>('text-sm font-semibold text-white')], [
        value,
      ]),
    ],
  )
}

const rowView = (row: LeaderboardRow): Html => {
  const h = html<Message>()
  const cell =
    'border-b border-white/5 px-3 py-2 text-left align-top text-[0.8125rem] text-white/75'

  return h.tr([h.DataAttribute('artanis-gym-mirrorcode-row', row.target)], [
    h.td([Ui.className<Message>(`${cell} font-semibold text-white/55`)], [
      row.rank,
    ]),
    h.td([Ui.className<Message>(cell)], [row.bucket]),
    h.td([Ui.className<Message>(`${cell} font-semibold text-white`)], [
      row.target,
    ]),
    h.td([Ui.className<Message>(cell)], [row.state]),
    h.td([Ui.className<Message>(cell)], [row.passRate]),
    h.td([Ui.className<Message>(cell)], [row.tokens]),
    h.td([Ui.className<Message>(cell)], [row.proof]),
  ])
}

const mirrorCodeLeaderboard = (): Html => {
  const h = html<Message>()
  const head =
    'border-b border-white/10 px-3 py-2 text-left text-[0.7rem] font-semibold uppercase tracking-wide text-white/40'

  return h.section(
    [
      h.DataAttribute('artanis-gym-mirrorcode-tab', ''),
      Ui.className<Message>(
        'grid gap-4 border border-white/10 bg-[#050505] p-4',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 lg:grid-cols-[minmax(0,1fr)_18rem] lg:items-start',
          ),
        ],
        [
          h.div([Ui.className<Message>('grid gap-2')], [
            h.p(
              [
                Ui.className<Message>(
                  'm-0 font-mono text-[0.78rem] font-semibold uppercase tracking-wide text-white/55',
                ),
              ],
              ['MirrorCode leaderboard'],
            ),
            h.h2(
              [Ui.className<Message>('m-0 text-2xl font-semibold text-white')],
              ['Public-bucket reproduction ladder'],
            ),
            h.p(
              [
                Ui.className<Message>(
                  'm-0 max-w-[78ch] text-sm leading-6 text-white/62',
                ),
              ],
              [
                'Tracks Khala runs against Epoch Research MirrorCode public tasks. Decision-grade numbers require exact token rows; owner-gated or smoke rows stay labeled instead of becoming leaderboard claims.',
              ],
            ),
          ]),
          h.div(
            [
              h.DataAttribute('artanis-gym-mirrorcode-gate', ''),
              Ui.className<Message>(
                'grid gap-1 border border-[#ffb400]/30 bg-[#120d02] p-3 text-[0.78rem] text-[#ffd884]',
              ),
            ],
            [
              h.span(
                [Ui.className<Message>('font-semibold uppercase tracking-wide')],
                ['Gate'],
              ),
              h.span([], [
                'decision-grade requires exact token usage evidence',
              ]),
            ],
          ),
        ],
      ),
      h.div([Ui.className<Message>('grid gap-3 sm:grid-cols-3')], [
        metric('Benchmark family', 'mirrorcode_public_bucket'),
        metric('Scope', 'public tasks only'),
        metric('Demand source', 'gym_mirrorcode'),
      ]),
      h.div([Ui.className<Message>('min-w-0 overflow-x-auto')], [
        h.table(
          [
            h.DataAttribute('artanis-gym-mirrorcode-leaderboard', ''),
            Ui.className<Message>('w-full min-w-[48rem] border-collapse'),
          ],
          [
            h.thead([], [
              h.tr([], [
                h.th([Ui.className<Message>(head)], ['Rank']),
                h.th([Ui.className<Message>(head)], ['Bucket']),
                h.th([Ui.className<Message>(head)], ['Target']),
                h.th([Ui.className<Message>(head)], ['State']),
                h.th([Ui.className<Message>(head)], ['Pass rate']),
                h.th([Ui.className<Message>(head)], ['Tokens']),
                h.th([Ui.className<Message>(head)], ['Proof']),
              ]),
            ]),
            h.tbody([], mirrorCodeRows.map(rowView)),
          ],
        ),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
        h.a(
          [
            h.Href(mirrorCodeRouter()),
            Ui.className<Message>(
              'border border-white/15 bg-black px-3 py-2 text-[0.78rem] font-semibold text-white/70 hover:border-white/25 hover:text-white',
            ),
          ],
          ['Open public MirrorCode page'],
        ),
        h.a(
          [
            h.Href('/api/public/gym/leaderboard'),
            Ui.className<Message>(
              'border border-white/15 bg-black px-3 py-2 text-[0.78rem] font-semibold text-white/70 hover:border-white/25 hover:text-white',
            ),
          ],
          ['Read ladder JSON'],
        ),
      ]),
    ],
  )
}

export const view = (_model: Model): Html => {
  const h = html<Message>()

  return Ui.container<Message>(
    [
      Ui.pageHeader<Message>({
        eyebrow: 'Artanis / Gym',
        title: 'Artanis Gym',
        body: 'Operator view for benchmark surfaces Artanis uses to track Khala progress. Tabs stay evidence-first: public projections, explicit gates, and no private task material.',
      }),
      h.nav(
        [
          h.AriaLabel('Artanis Gym tabs'),
          Ui.className<Message>('mt-4 flex flex-wrap gap-2'),
        ],
        [
          tabLink('MirrorCode', '/artanis/gym', true, [
            h.DataAttribute('artanis-gym-tab', 'mirrorcode'),
          ]),
          tabLink('GPT-OSS latency', gymOssRouter(), false, [
            h.DataAttribute('artanis-gym-tab', 'gpt-oss'),
          ]),
        ],
      ),
      h.div([Ui.className<Message>('mt-4 grid gap-4')], [
        mirrorCodeLeaderboard(),
      ]),
    ],
    [Ui.className<Message>('py-4')],
  )
}

type Model = import('../model').Model
