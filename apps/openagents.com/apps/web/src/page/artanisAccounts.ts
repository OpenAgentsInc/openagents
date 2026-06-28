import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

type AccountStatus = 'ready' | 'cooldown' | 'needs_attention'

type AccountGridRow = {
  readonly ref: string
  readonly provider: string
  readonly status: AccountStatus
  readonly activeSlots: number
  readonly queuedTurns: number
  readonly resetWindow: string
  readonly evidence: string
}

const rows: ReadonlyArray<AccountGridRow> = [
  {
    ref: 'codex-1',
    provider: 'Codex',
    status: 'ready',
    activeSlots: 1,
    queuedTurns: 0,
    resetWindow: '< 1m',
    evidence: 'quota-ledger + heartbeat',
  },
  {
    ref: 'codex-2',
    provider: 'Codex',
    status: 'cooldown',
    activeSlots: 0,
    queuedTurns: 2,
    resetWindow: '18m',
    evidence: 'rate-limit header observed',
  },
  {
    ref: 'claude-1',
    provider: 'Claude',
    status: 'ready',
    activeSlots: 2,
    queuedTurns: 1,
    resetWindow: '< 1m',
    evidence: 'local session refreshed',
  },
  {
    ref: 'codex-3',
    provider: 'Codex',
    status: 'needs_attention',
    activeSlots: 0,
    queuedTurns: 0,
    resetWindow: 'manual',
    evidence: 'credentials-missing blocker',
  },
]

const statusLabel = (status: AccountStatus): string =>
  status === 'ready'
    ? 'Ready'
    : status === 'cooldown'
      ? 'Cooldown'
      : 'Needs attention'

const statusClass = (status: AccountStatus): string =>
  status === 'ready'
    ? 'border-emerald-400/30 bg-emerald-400/10 text-emerald-200'
    : status === 'cooldown'
      ? 'border-amber-400/30 bg-amber-400/10 text-amber-200'
      : 'border-red-400/30 bg-red-400/10 text-red-200'

const metric = <Message>(label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid min-h-16 content-between border border-white/10 bg-[#050505] p-3',
      ),
    ],
    [
      h.div([Ui.className<Message>('text-[0.68rem] uppercase text-white/45')], [
        label,
      ]),
      h.div(
        [
          Ui.className<Message>(
            'text-xl font-semibold tabular-nums text-[#f1efe8]',
          ),
        ],
        [value],
      ),
    ],
  )
}

const rowView = <Message>(row: AccountGridRow): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Role('row'),
      Ui.className<Message>(
        'grid grid-cols-1 gap-3 border-t border-white/10 px-4 py-4 text-sm text-white/70 md:grid-cols-[minmax(8rem,1fr)_minmax(6rem,0.8fr)_minmax(8rem,0.9fr)_minmax(7rem,0.8fr)_minmax(7rem,0.8fr)_minmax(10rem,1fr)] md:items-center',
      ),
    ],
    [
      h.div(
        [h.Role('cell'), Ui.className<Message>('min-w-0')],
        [
          h.div(
            [Ui.className<Message>('truncate font-semibold text-[#f1efe8]')],
            [row.ref],
          ),
          h.div([Ui.className<Message>('text-xs text-white/40')], [
            'public-safe ref',
          ]),
        ],
      ),
      h.div([h.Role('cell')], [row.provider]),
      h.div(
        [h.Role('cell')],
        [
          h.span(
            [
              Ui.className<Message>(
                `inline-flex min-h-7 items-center rounded border px-2 text-xs font-semibold ${statusClass(row.status)}`,
              ),
            ],
            [statusLabel(row.status)],
          ),
        ],
      ),
      h.div([h.Role('cell'), Ui.className<Message>('tabular-nums')], [
        String(row.activeSlots),
      ]),
      h.div([h.Role('cell'), Ui.className<Message>('tabular-nums')], [
        String(row.queuedTurns),
      ]),
      h.div([h.Role('cell'), Ui.className<Message>('min-w-0')], [
        h.div([Ui.className<Message>('truncate text-[#f1efe8]')], [
          row.resetWindow,
        ]),
        h.div([Ui.className<Message>('truncate text-xs text-white/40')], [
          row.evidence,
        ]),
      ]),
    ],
  )
}

const headerCell = <Message>(label: string): Html => {
  const h = html<Message>()

  return h.div(
    [h.Role('columnheader'), Ui.className<Message>('text-white/45')],
    [label],
  )
}

const gridView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.AriaLabel('Per-account rate-limit observability grid'),
      Ui.className<Message>('border border-white/10 bg-[#010102]'),
    ],
    [
      h.div(
        [
          h.Role('row'),
          Ui.className<Message>(
            'hidden grid-cols-[minmax(8rem,1fr)_minmax(6rem,0.8fr)_minmax(8rem,0.9fr)_minmax(7rem,0.8fr)_minmax(7rem,0.8fr)_minmax(10rem,1fr)] gap-3 px-4 py-3 text-[0.68rem] uppercase md:grid',
          ),
        ],
        [
          headerCell<Message>('Account'),
          headerCell<Message>('Provider'),
          headerCell<Message>('State'),
          headerCell<Message>('Active'),
          headerCell<Message>('Queued'),
          headerCell<Message>('Reset / evidence'),
        ],
      ),
      h.div([h.Role('rowgroup')], Array.map(rows, rowView<Message>)),
    ],
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('min-h-screen bg-black text-[#f1efe8]')],
    [
      PublicHeader.view(authState),
      h.main(
        [
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,1120px)] gap-6 px-4 py-8 sm:px-6 lg:px-8',
          ),
        ],
        [
          h.header(
            [Ui.className<Message>('grid gap-3 border-b border-white/10 pb-5')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'text-[0.7rem] uppercase tracking-wide text-white/45',
                  ),
                ],
                ['Artanis / accounts'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 text-2xl font-semibold tracking-normal text-[#f1efe8] sm:text-3xl',
                  ),
                ],
                ['Per-account rate-limit grid'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[72ch] text-sm leading-6 text-white/60',
                  ),
                ],
                [
                  'Public-safe operator view for connected coding accounts. Rows expose refs, readiness, queue pressure, reset hints, and evidence labels only.',
                ],
              ),
            ],
          ),
          h.section(
            [
              h.AriaLabel('Account grid summary'),
              Ui.className<Message>('grid gap-3 sm:grid-cols-3'),
            ],
            [
              metric<Message>('ready accounts', '2'),
              metric<Message>('active slots', '3'),
              metric<Message>('queued turns', '3'),
            ],
          ),
          gridView<Message>(),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[76ch] border border-white/10 bg-[#050505] p-3 text-xs leading-5 text-white/45',
              ),
            ],
            [
              'This surface is evidence-only. It does not grant dispatch, spend, settlement, provider-account mutation, or cross-owner routing authority.',
            ],
          ),
        ],
      ),
    ],
  )
}
