import { clsx } from 'clsx'
import type { Attribute } from 'foldkit/html'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { iconView } from './icon'
import { button } from './shared'
import { kitFamily, motionPaneOpenClass } from './primitives'

// ---------------------------------------------------------------------------
// Pro operator console primitives (issue 6179)
// ---------------------------------------------------------------------------
//
// Command surfaces — strips, registers, panes — for the /pro power-user operator
// console. Dark-only pure black, panels #010102, subtle #222 borders, warm
// off-white #f1efe8, mono-first. Semantic accents for STATE only. No cards, no
// hero, no gradients, no eyebrows (apps/openagents.com/DESIGN.md).
//
// These are the shared, class-bearing registry components so the app page stays
// composed through the Foldkit UI system rather than carrying raw Tailwind.

export type ProConsoleSection = Readonly<{
  // Section label, e.g. 'Overview'.
  label: string
  // The single live section is active; the rest are honest disabled
  // placeholders (rendered as non-interactive, never fake links).
  active?: boolean
  disabled?: boolean
  // href is only used for an active, live section.
  href?: string
}>

// Top strip: compact, mono, 1px subtle bottom border. Left = PRO mark + a thin
// breadcrumb. Right = a QUIET usage/credits indicator placeholder + account.
export const proTopStrip = <Message>(input: {
  homeHref: string
  breadcrumb: string
  // The credits/usage indicator is an honest placeholder: it never shows a real
  // balance. `creditsHint` is the title shown on hover.
  creditsLabel: string
  creditsState: string
  creditsHint: string
  accountLabel: string
}): Html => {
  const h = html<Message>()

  return h.header(
    [
      kitFamily<Message>('navigation/navbars'),
      h.DataAttribute('component', 'pro-top-strip'),
      h.Class(
        'flex min-h-11 items-center justify-between gap-4 border-b border-[#222] bg-[#010102] px-4 text-sm',
      ),
    ],
    [
      h.div(
        [h.Class('flex min-w-0 items-center gap-3')],
        [
          h.a(
            [
              h.Href(input.homeHref),
              h.AriaLabel('Pro console'),
              h.Class(
                'font-semibold uppercase tracking-[0.18em] text-[#f1efe8] no-underline',
              ),
            ],
            ['PRO'],
          ),
          h.span([h.AriaHidden(true), h.Class('text-white/25')], ['/']),
          h.span([h.Class('truncate text-white/45')], [input.breadcrumb]),
        ],
      ),
      h.div(
        [h.Class('flex shrink-0 items-center gap-3')],
        [
          h.span(
            [
              h.DataAttribute('component', 'pro-credits-indicator'),
              h.Title(input.creditsHint),
              h.Class(
                'inline-flex items-center gap-1.5 border border-dashed border-white/15 px-2 py-0.5 text-[0.6875rem] uppercase tracking-[0.08em] text-white/35',
              ),
            ],
            [
              h.span([h.Class('text-white/25')], [input.creditsLabel]),
              h.span([h.Class('text-white/45')], [input.creditsState]),
            ],
          ),
          h.span(
            [h.Class('hidden truncate text-white/45 sm:inline')],
            [input.accountLabel],
          ),
        ],
      ),
    ],
  )
}

const proRegisterItem = <Message>(section: ProConsoleSection): Html => {
  const h = html<Message>()

  if (section.disabled === true || section.href === undefined) {
    return h.div(
      [
        h.AriaDisabled(true),
        h.Title(`${section.label} is coming to Pro.`),
        h.Class(
          'grid cursor-default grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-transparent px-2.5 py-1.5 text-sm text-white/25',
        ),
      ],
      [
        h.span([h.Class('truncate')], [section.label]),
        h.span(
          [h.Class('text-[0.625rem] uppercase tracking-[0.08em] text-white/20')],
          ['soon'],
        ),
      ],
    )
  }

  const activeAttrs: ReadonlyArray<Attribute<Message>> =
    section.active === true ? [h.AriaCurrent('page')] : []

  return h.a(
    [
      h.Href(section.href),
      ...activeAttrs,
      h.Class(
        clsx(
          'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border border-transparent px-2.5 py-1.5 text-sm no-underline',
          'transition-[border-color,background-color,color] duration-150 motion-reduce:transition-none',
          {
            'border-[#333] bg-[#141414] text-[#f1efe8]': section.active === true,
            'text-white/60 hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8] focus-visible:border-[#333] focus-visible:text-[#f1efe8] focus-visible:outline-none':
              section.active !== true,
          },
        ),
      ),
    ],
    [h.span([h.Class('truncate')], [section.label])],
  )
}

// Left register/nav: narrow vertical list, muted -> active. The live section is
// active; the rest are honest disabled placeholders pointing at future features.
export const proRegister = <Message>(
  sections: ReadonlyArray<ProConsoleSection>,
): Html => {
  const h = html<Message>()

  return h.nav(
    [
      kitFamily<Message>('navigation/vertical-navigation'),
      h.DataAttribute('component', 'pro-register'),
      h.AriaLabel('Pro sections'),
      h.Class(
        'grid auto-rows-min content-start gap-0.5 border-r border-[#222] bg-[#010102] p-2 sm:w-44',
      ),
    ],
    sections.map(section => proRegisterItem<Message>(section)),
  )
}

// A disabled forward affordance rendered honestly: it looks like an action but
// is non-interactive and labelled with its hint. Never a working button.
export const proComingAffordance = <Message>(input: {
  label: string
  hint: string
}): Html => {
  const h = html<Message>()

  return button<Message>({
    label: input.label,
    variant: 'secondary',
    size: 'sm',
    attrs: [
      h.Disabled(true),
      h.AriaDisabled(true),
      h.Title(input.hint),
      h.DataAttribute('state', 'coming'),
      h.Class('cursor-not-allowed opacity-50'),
    ],
  })
}

// A LIVE forward affordance: a real navigable link styled like the secondary
// console buttons, with a decorative aria-hidden arrow that nudges on hover. The
// label is the only text node, so the accessible name is exactly `label`. Used
// by the /pro console to link out to the public shareable `/trace` surfaces.
export const proLinkAffordance = <Message>(input: {
  label: string
  href: string
}): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href(input.href),
      h.DataAttribute('component', 'pro-link-affordance'),
      h.Class(
        'group inline-flex min-h-8 items-center justify-center gap-1.5 border border-[#333] bg-[#080808] px-3 text-sm text-[#f1efe8] no-underline transition-[border-color,background-color,color] duration-150 hover:border-[#555] hover:bg-[#141414] focus-visible:border-[#555] focus-visible:outline-none motion-reduce:transition-none',
      ),
    ],
    [
      input.label,
      h.svg(
        [
          h.AriaHidden(true),
          h.Class(
            'size-3.5 text-white/45 transition-transform duration-150 group-hover:translate-x-0.5 motion-reduce:transition-none',
          ),
          h.ViewBox('0 0 24 24'),
          h.Fill('none'),
          h.Attribute('stroke', 'currentColor'),
          h.Attribute('stroke-width', '2'),
          h.Attribute('stroke-linecap', 'round'),
          h.Attribute('stroke-linejoin', 'round'),
        ],
        [h.path([h.D('M5 12h14M13 6l6 6-6 6')], [])],
      ),
    ],
  )
}

// The Overview EMPTY STATE that teaches: one line on what Pro is, plus the
// forward affordances (passed in) shown honestly. No hero, no cards.
export const proTeachingEmptyState = <Message>(input: {
  title: string
  body: string
  footnote: string
  affordances: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('component', 'pro-overview-empty'),
      h.Class(motionPaneOpenClass),
    ],
    [
      h.div(
        [
          h.Class(
            'mx-auto grid max-w-[64ch] gap-5 border border-[#222] bg-[#010102] p-6',
          ),
        ],
        [
          h.div(
            [h.Class('grid gap-2')],
            [
              h.h1(
                [
                  h.Class(
                    'm-0 text-base font-semibold tracking-[0.01em] text-[#f1efe8]',
                  ),
                ],
                [input.title],
              ),
              h.p(
                [h.Class('m-0 text-sm leading-[1.6] text-white/60')],
                [input.body],
              ),
            ],
          ),
          h.div(
            [
              h.Class(
                'flex flex-wrap items-center gap-2 border-t border-[#222] pt-4',
              ),
            ],
            input.affordances,
          ),
          h.p(
            [h.Class('m-0 text-xs leading-[1.55] text-white/35')],
            [input.footnote],
          ),
        ],
      ),
    ],
  )
}

// LOADING STATE: skeleton rows (not a center spinner), reduced-motion aware.
export const proSkeletonRows = <Message>(
  widthClasses: ReadonlyArray<string> = ['w-2/3', 'w-1/2', 'w-3/5', 'w-2/5'],
): Html => {
  const h = html<Message>()
  const row = (widthClass: string): Html =>
    h.div(
      [h.Class('flex items-center gap-3 border border-[#222] bg-[#010102] p-3')],
      [
        h.span(
          [
            h.Class(
              'block h-3 w-3 shrink-0 animate-pulse bg-white/15 motion-reduce:animate-none',
            ),
          ],
          [],
        ),
        h.span(
          [
            h.Class(
              clsx(
                'block h-3 animate-pulse bg-white/12 motion-reduce:animate-none',
                widthClass,
              ),
            ),
          ],
          [],
        ),
      ],
    )

  return h.div(
    [
      h.DataAttribute('component', 'pro-overview-loading'),
      h.AriaBusy(true),
      h.Class('mx-auto grid max-w-[64ch] gap-2'),
    ],
    widthClasses.map(row),
  )
}

// ERROR STATE: a compact inline error strip (not a full-page error).
export const proErrorStrip = <Message>(detail: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('component', 'pro-overview-error'),
      h.Role('alert'),
      h.Class(
        'mx-auto flex max-w-[64ch] items-center gap-3 border border-[#d32f2f]/70 bg-[#010102] px-3 py-2 text-sm text-[#f1efe8]',
      ),
    ],
    [
      iconView<Message>('Warning', 'size-4 text-[#d32f2f]'),
      h.span([h.Class('min-w-0 flex-1 truncate text-white/70')], [detail]),
    ],
  )
}

// The Pro console shell: top-strip + left-register + main pane. Owns the full
// layout independent of the workroom sidebar shell.
export const proConsoleShell = <Message>(input: {
  topStrip: Html
  register: Html
  main: Html
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      kitFamily<Message>('application-shells/stacked'),
      h.DataAttribute('component', 'pro-console'),
      h.Class('grid h-dvh grid-rows-[auto_minmax(0,1fr)] bg-[#000]'),
    ],
    [
      input.topStrip,
      h.div(
        [h.Class('grid min-h-0 grid-cols-[auto_minmax(0,1fr)]')],
        [input.register, input.main],
      ),
    ],
  )
}

// The scrollable main pane wrapper for the console body.
export const proMainPane = <Message>(children: ReadonlyArray<Html>): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('component', 'pro-main-pane'),
      h.Class('min-w-0 overflow-y-auto p-4'),
    ],
    children,
  )
}

export type ProAgentState = 'working' | 'blocked' | 'waiting' | 'done'

export type ProAgentStateHistoryEntry = Readonly<{
  state: ProAgentState
  label: string
  at: string
}>

export type ProAgentStatusEntry = Readonly<{
  id: string
  agentLabel: string
  worktreeLabel: string
  state: ProAgentState
  prompt: string
  updatedAt: string
  stateStartedAt: string
  acknowledgedAt: string
  unread: boolean
  toolName: string
  lastAssistantMessage: string
  stateHistory: ReadonlyArray<ProAgentStateHistoryEntry>
}>

export type ProDiffComment = Readonly<{
  id: string
  filePath: string
  lineLabel: string
  body: string
  selectedText: string
  targetAgentLabel: string
  sentAt: string
}>

export type ProAgentDashboardSnapshot = Readonly<{
  generatedAt: string
  liveEntries: ReadonlyArray<ProAgentStatusEntry>
  retainedEntries: ReadonlyArray<ProAgentStatusEntry>
  diffComments: ReadonlyArray<ProDiffComment>
}>

const proAgentStateLabel = (state: ProAgentState): string =>
  state === 'working'
    ? 'working'
    : state === 'blocked'
      ? 'blocked'
      : state === 'waiting'
        ? 'waiting'
        : 'done'

const proAgentStateClass = (state: ProAgentState): string =>
  state === 'working'
    ? 'border-[#2979ff]/45 text-[#8fb6ff]'
    : state === 'blocked'
      ? 'border-[#ff6f00]/55 text-[#ffb400]'
      : state === 'waiting'
        ? 'border-white/20 text-white/45'
        : 'border-[#00c853]/45 text-[#00c853]'

const proAgentStatePill = <Message>(state: ProAgentState): Html => {
  const h = html<Message>()

  return h.span(
    [
      h.DataAttribute('component', 'pro-agent-state-pill'),
      h.DataAttribute('state', state),
      h.Class(
        clsx(
          'inline-flex items-center border px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase tracking-[0.1em]',
          proAgentStateClass(state),
        ),
      ),
    ],
    [proAgentStateLabel(state)],
  )
}

const proAgentStatusRow = <Message>(
  entry: ProAgentStatusEntry,
  retention: 'live' | 'retained',
): Html => {
  const h = html<Message>()

  return h.li(
    [
      h.DataAttribute('component', 'pro-agent-status-row'),
      h.DataAttribute('agent-status-retention', retention),
      h.DataAttribute('agent-state-started-at', entry.stateStartedAt),
      h.DataAttribute('agent-updated-at', entry.updatedAt),
      h.Class(
        clsx(
          'grid gap-3 border bg-[#010102] px-3 py-2.5',
          entry.unread === true ? 'border-[#ffb400]/60' : 'border-[#222]',
        ),
      ),
    ],
    [
      h.div(
        [
          h.Class(
            'grid gap-2 md:grid-cols-[minmax(0,1fr)_auto] md:items-start',
          ),
        ],
        [
          h.div(
            [h.Class('grid min-w-0 gap-1')],
            [
              h.div(
                [h.Class('flex min-w-0 flex-wrap items-center gap-2')],
                [
                  h.h3(
                    [h.Class('m-0 truncate text-sm font-semibold text-[#f1efe8]')],
                    [entry.agentLabel],
                  ),
                  proAgentStatePill<Message>(entry.state),
                  ...(entry.unread === true
                    ? [
                        h.span(
                          [
                            h.DataAttribute('component', 'pro-agent-unread'),
                            h.Class(
                              'border border-[#ffb400]/45 px-1.5 py-0.5 text-[0.625rem] uppercase tracking-[0.1em] text-[#ffb400]',
                            ),
                          ],
                          ['unread'],
                        ),
                      ]
                    : []),
                ],
              ),
              h.p([h.Class('m-0 text-xs leading-[1.5] text-white/55')], [
                entry.prompt,
              ]),
            ],
          ),
          h.dl(
            [
              h.Class(
                'm-0 grid gap-1 text-[0.6875rem] text-white/45 sm:grid-cols-2 md:min-w-72',
              ),
            ],
            [
              h.div([h.Class('grid gap-0.5')], [
                h.dt([h.Class('uppercase tracking-[0.08em] text-white/25')], [
                  'worktree',
                ]),
                h.dd([h.Class('m-0 truncate text-white/60')], [
                  entry.worktreeLabel,
                ]),
              ]),
              h.div([h.Class('grid gap-0.5')], [
                h.dt([h.Class('uppercase tracking-[0.08em] text-white/25')], [
                  'stateStartedAt',
                ]),
                h.dd([h.Class('m-0 text-white/60')], [entry.stateStartedAt]),
              ]),
              h.div([h.Class('grid gap-0.5')], [
                h.dt([h.Class('uppercase tracking-[0.08em] text-white/25')], [
                  'updatedAt',
                ]),
                h.dd([h.Class('m-0 text-white/60')], [entry.updatedAt]),
              ]),
              h.div([h.Class('grid gap-0.5')], [
                h.dt([h.Class('uppercase tracking-[0.08em] text-white/25')], [
                  'ack',
                ]),
                h.dd([h.Class('m-0 text-white/60')], [entry.acknowledgedAt]),
              ]),
            ],
          ),
        ],
      ),
      h.div(
        [
          h.Class(
            'grid gap-2 border-t border-[#222] pt-2 md:grid-cols-[minmax(0,1fr)_minmax(18rem,0.85fr)]',
          ),
        ],
        [
          h.div([h.Class('grid gap-1')], [
            h.span(
              [
                h.Class(
                  'text-[0.625rem] font-semibold uppercase tracking-[0.1em] text-white/25',
                ),
              ],
              ['last assistant'],
            ),
            h.p([h.Class('m-0 text-xs leading-[1.5] text-white/55')], [
              entry.lastAssistantMessage,
            ]),
          ]),
          h.div([h.Class('grid gap-1')], [
            h.span(
              [
                h.Class(
                  'text-[0.625rem] font-semibold uppercase tracking-[0.1em] text-white/25',
                ),
              ],
              ['stateHistory'],
            ),
            h.ol(
              [h.Class('m-0 grid list-none gap-1 p-0')],
              entry.stateHistory.map(history =>
                h.li(
                  [
                    h.DataAttribute('component', 'pro-agent-history-entry'),
                    h.Class(
                      'grid grid-cols-[4.5rem_minmax(0,1fr)_5.5rem] gap-2 text-[0.6875rem]',
                    ),
                  ],
                  [
                    h.span([h.Class('text-white/35')], [history.at]),
                    h.span([h.Class('truncate text-white/60')], [history.label]),
                    h.span([h.Class('text-right text-white/35')], [
                      proAgentStateLabel(history.state),
                    ]),
                  ],
                ),
              ),
            ),
          ]),
        ],
      ),
    ],
  )
}

const proAgentStatusList = <Message>(input: {
  label: string
  retention: 'live' | 'retained'
  entries: ReadonlyArray<ProAgentStatusEntry>
}): Html => {
  const h = html<Message>()

  return proConsoleSection2<Message>(input.label, [
    h.ul(
      [
        h.DataAttribute('component', `pro-agent-status-${input.retention}`),
        h.Class('m-0 grid list-none gap-2 p-0'),
      ],
      input.entries.map(entry =>
        proAgentStatusRow<Message>(entry, input.retention),
      ),
    ),
  ])
}

const proDiffCommentRow = <Message>(comment: ProDiffComment): Html => {
  const h = html<Message>()

  return h.li(
    [
      h.DataAttribute('component', 'pro-diff-comment-row'),
      h.Class(
        'grid gap-2 border border-[#222] bg-[#010102] px-3 py-2.5 md:grid-cols-[minmax(0,1fr)_minmax(14rem,0.55fr)]',
      ),
    ],
    [
      h.div([h.Class('grid min-w-0 gap-1.5')], [
        h.div([h.Class('flex min-w-0 flex-wrap items-center gap-2')], [
          proCodeRef<Message>(`${comment.filePath}:${comment.lineLabel}`),
          h.span([h.Class('text-xs text-white/35')], [
            `target ${comment.targetAgentLabel}`,
          ]),
        ]),
        h.p([h.Class('m-0 text-sm leading-[1.5] text-[#f1efe8]')], [
          comment.body,
        ]),
        h.blockquote(
          [
            h.Class(
              'm-0 border border-[#222] bg-[#080808] px-2 py-1.5 text-xs leading-[1.45] text-white/45',
            ),
          ],
          [comment.selectedText],
        ),
      ]),
      h.div([h.Class('grid content-between gap-3')], [
        h.div([h.Class('grid gap-1 text-xs text-white/45')], [
          h.span(
            [
              h.Class(
                'text-[0.625rem] font-semibold uppercase tracking-[0.1em] text-white/25',
              ),
            ],
            ['ship-back queue'],
          ),
          h.span([], [`sentAt ${comment.sentAt}`]),
        ]),
        button<Message>({
          label: 'Send comments',
          variant: 'secondary',
          size: 'sm',
          attrs: [
            h.Disabled(true),
            h.AriaDisabled(true),
            h.Title('Staged until the live agent-send endpoint is connected.'),
            h.DataAttribute('component', 'pro-diff-send-action'),
            h.Class('justify-self-start cursor-not-allowed opacity-50'),
          ],
        }),
      ]),
    ],
  )
}

const proDiffCommentQueue = <Message>(
  comments: ReadonlyArray<ProDiffComment>,
): Html => {
  const h = html<Message>()

  return proConsoleSection2<Message>('Annotate diff -> ship back', [
    h.div(
      [
        h.DataAttribute('component', 'pro-diff-comment-queue'),
        h.Class('grid gap-2'),
      ],
      [
        h.p([h.Class('m-0 max-w-[72ch] text-xs leading-[1.55] text-white/45')], [
          'Line comments are retained as review intent and grouped by target agent. Live sending stays disabled until the owner-scoped agent-send endpoint is wired.',
        ]),
        h.ul(
          [h.Class('m-0 grid list-none gap-2 p-0')],
          comments.map(proDiffCommentRow<Message>),
        ),
      ],
    ),
  ])
}

const proDashboardSummary = <Message>(
  snapshot: ProAgentDashboardSnapshot,
): Html => {
  const h = html<Message>()
  const activeCount = snapshot.liveEntries.length.toString()
  const retainedCount = snapshot.retainedEntries.length.toString()
  const commentCount = snapshot.diffComments.length.toString()

  return h.div(
    [
      h.DataAttribute('component', 'pro-agent-dashboard-summary'),
      h.Class(
        'grid gap-2 border-b border-[#222] pb-4 md:grid-cols-[minmax(0,1fr)_auto]',
      ),
    ],
    [
      h.div([h.Class('grid gap-1')], [
        h.h1(
          [h.Class('m-0 text-base font-semibold tracking-[0.01em] text-[#f1efe8]')],
          ['Agent operations'],
        ),
        h.p([h.Class('m-0 max-w-[72ch] text-sm leading-[1.55] text-white/55')], [
          'Live and retained agent status entries use stateStartedAt for unread tracking, keep a bounded stateHistory, and stage diff annotations for operator review.',
        ]),
      ]),
      h.dl(
        [
          h.Class(
            'm-0 grid grid-cols-3 gap-2 text-center text-xs sm:min-w-[24rem]',
          ),
        ],
        [
          h.div([h.Class('border border-[#222] bg-[#010102] px-2 py-2')], [
            h.dt([h.Class('text-white/30')], ['live']),
            h.dd([h.Class('m-0 text-sm font-semibold text-[#f1efe8]')], [
              activeCount,
            ]),
          ]),
          h.div([h.Class('border border-[#222] bg-[#010102] px-2 py-2')], [
            h.dt([h.Class('text-white/30')], ['retained']),
            h.dd([h.Class('m-0 text-sm font-semibold text-[#f1efe8]')], [
              retainedCount,
            ]),
          ]),
          h.div([h.Class('border border-[#222] bg-[#010102] px-2 py-2')], [
            h.dt([h.Class('text-white/30')], ['comments']),
            h.dd([h.Class('m-0 text-sm font-semibold text-[#f1efe8]')], [
              commentCount,
            ]),
          ]),
        ],
      ),
      h.p([h.Class('m-0 text-xs text-white/35 md:col-span-2')], [
        `Generated ${snapshot.generatedAt}. Public-safe owner-scoped status data only; no private prompts, raw logs, wallet material, or provider payloads are rendered.`,
      ]),
    ],
  )
}

export const proAgentDashboard = <Message>(
  snapshot: ProAgentDashboardSnapshot,
): Html =>
  proConsoleStack<Message>([
    proDashboardSummary<Message>(snapshot),
    proAgentStatusList<Message>({
      label: 'Live agents',
      retention: 'live',
      entries: snapshot.liveEntries,
    }),
    proAgentStatusList<Message>({
      label: 'Retained agents',
      retention: 'retained',
      entries: snapshot.retainedEntries,
    }),
    proDiffCommentQueue<Message>(snapshot.diffComments),
  ])

// ---------------------------------------------------------------------------
// Runs + evals surfaces (issue 6184)
// ---------------------------------------------------------------------------
//
// Command surfaces — strips, tables, a video pane — for the /pro runs and evals
// pages. Mono-first, dark, no cards, no hero. Semantic accents carry STATE only
// (pass = positive, fail = negative). `not_measured` renders literally, never a
// fabricated 0, mirroring the qa-runner honesty contract.

// A pass/fail status pill. Tiny, mono, uppercase; accent reserved for state.
export const proStatusPill = <Message>(status: 'pass' | 'fail'): Html => {
  const h = html<Message>()
  const pass = status === 'pass'
  return h.span(
    [
      h.DataAttribute('component', 'pro-status-pill'),
      h.DataAttribute('status', status),
      h.Class(
        clsx(
          'inline-flex items-center border px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase tracking-[0.1em]',
          pass
            ? 'border-[#00c853]/40 text-[#00c853]'
            : 'border-[#d32f2f]/50 text-[#d32f2f]',
        ),
      ),
    ],
    [pass ? 'pass' : 'fail'],
  )
}

// The VERIFY VERDICT pill (#6192): the investigator verdict on a run/eval.
// CONFIRMED = positive (green), REFUTED = negative (red), INCONCLUSIVE = caution
// (amber). Slightly louder than the pass/fail pill (it carries the headline
// judgement) but still mono, uppercase, accent-for-state-only — no card, no
// gradient (apps/openagents.com/DESIGN.md). Never inflate: the amber INCONCLUSIVE
// is its own honest state, distinct from CONFIRMED.
export type ProVerdict = 'CONFIRMED' | 'REFUTED' | 'INCONCLUSIVE'

export const proVerdictPill = <Message>(verdict: ProVerdict): Html => {
  const h = html<Message>()
  const tone =
    verdict === 'CONFIRMED'
      ? 'border-[#00c853]/50 bg-[#00c853]/10 text-[#00c853]'
      : verdict === 'REFUTED'
        ? 'border-[#d32f2f]/60 bg-[#d32f2f]/10 text-[#d32f2f]'
        : 'border-[#ffb400]/50 bg-[#ffb400]/10 text-[#ffb400]'
  return h.span(
    [
      h.DataAttribute('component', 'pro-verdict-pill'),
      h.DataAttribute('verdict', verdict),
      h.Title('Verify verdict (commitments checked against observed evidence).'),
      h.Class(
        clsx(
          'inline-flex items-center gap-1 border px-2 py-0.5 text-[0.625rem] font-semibold uppercase tracking-[0.12em]',
          tone,
        ),
      ),
    ],
    [
      h.span(
        [h.AriaHidden(true), h.Class('text-[0.6875rem] leading-none')],
        [verdict === 'CONFIRMED' ? '✓' : verdict === 'REFUTED' ? '✕' : '?'],
      ),
      h.span([], [verdict]),
    ],
  )
}

// A verify EVIDENCE list: per-commitment findings (the claim + the observed
// evidence summary), each prefixed by its own verdict pill. This is the
// "verify the output before you post" detail — a reviewer sees WHY the verdict
// landed without re-running anything. No card; thin rows.
export type ProVerdictFinding = Readonly<{
  id: string
  claim: string
  verdict: ProVerdict
  evidenceSummary: string
}>

export const proVerdictEvidence = <Message>(
  findings: ReadonlyArray<ProVerdictFinding>,
): Html => {
  const h = html<Message>()
  return h.ul(
    [
      h.DataAttribute('component', 'pro-verdict-evidence'),
      h.Class('m-0 grid list-none gap-1.5 p-0'),
    ],
    findings.map(f =>
      h.li(
        [
          h.Class(
            'grid grid-cols-[auto_minmax(0,1fr)] items-start gap-2.5 border border-[#222] bg-[#010102] px-3 py-2',
          ),
        ],
        [
          proVerdictPill<Message>(f.verdict),
          h.div(
            [h.Class('grid min-w-0 gap-0.5')],
            [
              h.span([h.Class('text-sm text-[#f1efe8]')], [f.claim]),
              h.span(
                [h.Class('text-xs text-white/40')],
                [f.evidenceSummary],
              ),
            ],
          ),
        ],
      ),
    ),
  )
}

// A back link to the parent /pro section (e.g. "← Runs"). Quiet, no underline.
export const proBackLink = <Message>(input: {
  href: string
  label: string
}): Html => {
  const h = html<Message>()
  return h.a(
    [
      h.Href(input.href),
      h.DataAttribute('component', 'pro-back-link'),
      h.Class(
        'inline-flex items-center gap-1.5 text-xs text-white/45 no-underline transition-colors duration-150 hover:text-[#f1efe8] focus-visible:text-[#f1efe8] focus-visible:outline-none motion-reduce:transition-none',
      ),
    ],
    [iconView<Message>('ArrowLeft', 'size-3.5'), h.span([], [input.label])],
  )
}

// A page header: a back link, a title, and a thin meta strip (key/value pairs).
// No card; a single subtle bottom border frames it.
export const proPageHeader = <Message>(input: {
  back?: { href: string; label: string }
  title: string
  status?: 'pass' | 'fail'
  // The verify investigator verdict (#6192), shown alongside the status pill.
  verdict?: ProVerdict
  meta: ReadonlyArray<{ label: string; value: string }>
  // An optional honest note (e.g. illustrative-only) shown under the meta.
  note?: string
}): Html => {
  const h = html<Message>()
  return h.div(
    [
      h.DataAttribute('component', 'pro-page-header'),
      h.Class('grid gap-3 border-b border-[#222] pb-4'),
    ],
    [
      ...(input.back !== undefined ? [proBackLink<Message>(input.back)] : []),
      h.div(
        [h.Class('flex flex-wrap items-center gap-3')],
        [
          h.h1(
            [
              h.Class(
                'm-0 text-base font-semibold tracking-[0.01em] text-[#f1efe8]',
              ),
            ],
            [input.title],
          ),
          ...(input.status !== undefined
            ? [proStatusPill<Message>(input.status)]
            : []),
          ...(input.verdict !== undefined
            ? [proVerdictPill<Message>(input.verdict)]
            : []),
        ],
      ),
      h.dl(
        [
          h.Class(
            'm-0 flex flex-wrap gap-x-6 gap-y-1 text-xs text-white/45',
          ),
        ],
        input.meta.flatMap(item => [
          h.div(
            [h.Class('flex items-baseline gap-1.5')],
            [
              h.dt([h.Class('text-white/30')], [item.label]),
              h.dd([h.Class('m-0 text-white/60')], [item.value]),
            ],
          ),
        ]),
      ),
      ...(input.note !== undefined
        ? [
            h.p(
              [h.Class('m-0 text-xs leading-[1.5] text-[#ffb400]/80')],
              [input.note],
            ),
          ]
        : []),
    ],
  )
}

// A vertical stack with consistent rhythm for a page body. No card.
export const proConsoleStack = <Message>(
  children: ReadonlyArray<Html>,
): Html => {
  const h = html<Message>()
  return h.div(
    [
      h.DataAttribute('component', 'pro-console-stack'),
      h.Class(clsx('mx-auto grid max-w-4xl gap-6', motionPaneOpenClass)),
    ],
    children,
  )
}

// A labelled sub-section: a heading + its content, separated by rhythm only.
export const proConsoleSection2 = <Message>(
  label: string,
  children: ReadonlyArray<Html>,
): Html => {
  const h = html<Message>()
  return h.section(
    [
      h.DataAttribute('component', 'pro-console-section'),
      h.Class('grid gap-2.5'),
    ],
    [proSectionHeading<Message>(label), ...children],
  )
}

// A monospace reference to a committed path/file (e.g. a distilled test).
export const proCodeRef = <Message>(path: string): Html => {
  const h = html<Message>()
  return h.code(
    [
      h.DataAttribute('component', 'pro-code-ref'),
      h.Class(
        'inline-block break-all border border-[#222] bg-[#010102] px-2 py-1 text-xs text-white/70',
      ),
    ],
    [path],
  )
}

// A heading for a sub-section within a page (e.g. "Steps", "Variants").
export const proSectionHeading = <Message>(label: string): Html => {
  const h = html<Message>()
  return h.h2(
    [
      h.DataAttribute('component', 'pro-section-heading'),
      h.Class(
        'm-0 text-[0.6875rem] font-semibold uppercase tracking-[0.12em] text-white/35',
      ),
    ],
    [label],
  )
}

// A list/index of links to runs or evals. Each row is a thin strip (no card):
// a status pill, a title, and a muted meta line. Empty -> honest empty strip.
export type ProIndexRow = Readonly<{
  href: string
  title: string
  status?: 'pass' | 'fail'
  meta: string
}>

export const proIndexList = <Message>(input: {
  rows: ReadonlyArray<ProIndexRow>
  emptyLabel: string
}): Html => {
  const h = html<Message>()
  if (input.rows.length === 0) {
    return h.div(
      [
        h.DataAttribute('component', 'pro-index-empty'),
        h.Class(
          'border border-[#222] bg-[#010102] px-3 py-6 text-center text-sm text-white/35',
        ),
      ],
      [input.emptyLabel],
    )
  }
  return h.ul(
    [
      h.DataAttribute('component', 'pro-index-list'),
      h.Class('m-0 grid list-none gap-1 p-0'),
    ],
    input.rows.map(row =>
      h.li(
        [h.Class('contents')],
        [
          h.a(
            [
              h.Href(row.href),
              h.Class(
                'grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border border-[#222] bg-[#010102] px-3 py-2 no-underline transition-[border-color,background-color] duration-150 hover:border-[#333] hover:bg-[#080808] focus-visible:border-[#333] focus-visible:outline-none motion-reduce:transition-none',
              ),
            ],
            [
              row.status !== undefined
                ? proStatusPill<Message>(row.status)
                : h.span([h.Class('w-10')], []),
              h.span(
                [h.Class('truncate text-sm text-[#f1efe8]')],
                [row.title],
              ),
              h.span(
                [h.Class('shrink-0 text-xs text-white/35')],
                [row.meta],
              ),
            ],
          ),
        ],
      ),
    ),
  )
}

// A playable video pane. A real <video controls> with the public-safe source.
// `prefers-reduced-motion` is respected by NOT autoplaying (it never autoplays).
export const proVideoPane = <Message>(input: {
  src: string
  format: 'mp4' | 'webm'
  label?: string
}): Html => {
  const h = html<Message>()
  return h.figure(
    [
      h.DataAttribute('component', 'pro-video-pane'),
      h.Class('m-0 grid gap-1.5'),
    ],
    [
      h.video(
        [
          h.Attribute('controls', 'controls'),
          h.Attribute('preload', 'metadata'),
          h.Attribute('playsinline', 'playsinline'),
          h.Class('w-full max-w-2xl border border-[#222] bg-black'),
        ],
        [
          h.source([
            h.Src(input.src),
            h.Attribute(
              'type',
              input.format === 'mp4' ? 'video/mp4' : 'video/webm',
            ),
          ]),
        ],
      ),
      ...(input.label !== undefined
        ? [
            h.figcaption(
              [h.Class('text-xs text-white/35')],
              [input.label],
            ),
          ]
        : []),
    ],
  )
}

// A run STEP table: index, kind, label, status. Mono, dense, no card.
export type ProStepRow = Readonly<{
  index: number
  kind: string
  label: string
  status: 'ok' | 'failed'
}>

export const proRunStepTable = <Message>(
  steps: ReadonlyArray<ProStepRow>,
): Html => {
  const h = html<Message>()
  const headCell = (label: string): Html =>
    h.th(
      [
        h.Class(
          'border-b border-[#222] px-2 py-1.5 text-left text-[0.625rem] font-semibold uppercase tracking-[0.1em] text-white/30',
        ),
      ],
      [label],
    )
  return h.table(
    [
      h.DataAttribute('component', 'pro-step-table'),
      h.Class('w-full border-collapse text-sm'),
    ],
    [
      h.thead(
        [],
        [
          h.tr(
            [],
            [headCell('#'), headCell('kind'), headCell('step'), headCell('status')],
          ),
        ],
      ),
      h.tbody(
        [],
        steps.map(step =>
          h.tr(
            [h.Class('border-b border-[#141414]')],
            [
              h.td([h.Class('px-2 py-1.5 text-white/35')], [String(step.index)]),
              h.td(
                [h.Class('px-2 py-1.5 text-white/45')],
                [step.kind],
              ),
              h.td([h.Class('px-2 py-1.5 text-[#f1efe8]')], [step.label]),
              h.td(
                [
                  h.Class(
                    clsx(
                      'px-2 py-1.5 font-semibold',
                      step.status === 'ok'
                        ? 'text-[#00c853]'
                        : 'text-[#d32f2f]',
                    ),
                  ),
                ],
                [step.status],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

// The EVAL variant COMPARISON table: the headline artifact. Per-variant
// pass-rate, p50/p90 latency, and deltas vs the baseline, side by side. The
// baseline row is marked; deltas are signed; `not_measured` is literal.
export type ProComparisonRow = Readonly<{
  label: string
  note?: string
  baseline: boolean
  passRate: number // 0..1
  passCount: number
  runCount: number
  latencyP50: string // already-formatted ("2140ms" | "not_measured")
  latencyP90: string
  deltaPass: string // signed ("+50%" | "0%" | "—")
  deltaP50: string // signed ("-160ms" | "not_measured" | "—")
}>

export const proEvalComparisonTable = <Message>(
  rows: ReadonlyArray<ProComparisonRow>,
): Html => {
  const h = html<Message>()
  const headCell = (label: string, alignRight = false): Html =>
    h.th(
      [
        h.Class(
          clsx(
            'border-b border-[#222] px-2.5 py-2 text-[0.625rem] font-semibold uppercase tracking-[0.1em] text-white/30',
            alignRight ? 'text-right' : 'text-left',
          ),
        ),
      ],
      [label],
    )
  const num = (value: string, accent?: 'pos' | 'neg'): Html =>
    h.td(
      [
        h.Class(
          clsx(
            'px-2.5 py-2 text-right tabular-nums',
            accent === 'pos'
              ? 'text-[#00c853]'
              : accent === 'neg'
                ? 'text-[#d32f2f]'
                : 'text-white/60',
          ),
        ),
      ],
      [value],
    )
  const deltaAccent = (value: string): 'pos' | 'neg' | undefined => {
    if (value.startsWith('+')) return 'pos'
    if (value.startsWith('-')) return 'neg'
    return undefined
  }
  return h.table(
    [
      h.DataAttribute('component', 'pro-eval-comparison'),
      h.Class('w-full border-collapse text-sm'),
    ],
    [
      h.thead(
        [],
        [
          h.tr(
            [],
            [
              headCell('variant'),
              headCell('pass-rate', true),
              headCell('p50', true),
              headCell('p90', true),
              headCell('Δ pass', true),
              headCell('Δ p50', true),
            ],
          ),
        ],
      ),
      h.tbody(
        [],
        rows.map(row =>
          h.tr(
            [
              h.DataAttribute('baseline', row.baseline ? 'true' : 'false'),
              h.Class(
                clsx(
                  'border-b border-[#141414]',
                  row.baseline ? 'bg-[#080808]' : '',
                ),
              ),
            ],
            [
              h.td(
                [h.Class('px-2.5 py-2')],
                [
                  h.div(
                    [h.Class('grid gap-0.5')],
                    [
                      h.div(
                        [h.Class('flex items-center gap-2')],
                        [
                          h.span(
                            [h.Class('text-[#f1efe8]')],
                            [row.label],
                          ),
                          ...(row.baseline
                            ? [
                                h.span(
                                  [
                                    h.Class(
                                      'text-[0.5625rem] uppercase tracking-[0.1em] text-white/30',
                                    ),
                                  ],
                                  ['baseline'],
                                ),
                              ]
                            : []),
                        ],
                      ),
                      ...(row.note !== undefined
                        ? [
                            h.span(
                              [h.Class('text-xs text-white/30')],
                              [row.note],
                            ),
                          ]
                        : []),
                    ],
                  ),
                ],
              ),
              num(
                `${Math.round(row.passRate * 100)}% (${row.passCount}/${row.runCount})`,
                row.passRate === 1 ? 'pos' : row.passRate === 0 ? 'neg' : undefined,
              ),
              num(row.latencyP50),
              num(row.latencyP90),
              num(row.deltaPass, deltaAccent(row.deltaPass)),
              num(row.deltaP50, deltaAccent(row.deltaP50)),
            ],
          ),
        ),
      ),
    ],
  )
}

// A small RESTRICTION badge (#6190): marks a target's policy. `read-only` is the
// headline (prod — never create data); `writable` is the quiet neutral state.
// Accent for the restricted state only (amber = caution), per DESIGN.md.
export const proRestrictionBadge = <Message>(readOnly: boolean): Html => {
  const h = html<Message>()
  return h.span(
    [
      h.DataAttribute('component', 'pro-restriction-badge'),
      h.DataAttribute('restriction', readOnly ? 'read-only' : 'writable'),
      h.Title(
        readOnly
          ? 'Read-only target: the runner refuses mutating steps (never create data).'
          : 'Writable target: mutating steps are allowed.',
      ),
      h.Class(
        clsx(
          'inline-flex items-center border px-1.5 py-0.5 text-[0.5625rem] font-semibold uppercase tracking-[0.1em]',
          readOnly
            ? 'border-[#ffb400]/50 text-[#ffb400]'
            : 'border-[#222] text-white/35',
        ),
      ),
    ],
    [readOnly ? 'read-only' : 'writable'],
  )
}

// The MULTI-TARGET MATRIX table (#6190): the SAME scenario run across N targets
// (dev / staging / prod / selfhost), side by side. Per target: name + base URL,
// the restriction badge, status pill, verify verdict, and duration. A read-only
// target that a scenario tried to mutate shows `fail` with its failure reason
// inline — honest, never a silent skip. Mono, dense, no card (DESIGN.md).
export type ProTargetMatrixRow = Readonly<{
  targetName: string
  targetBaseUrl: string
  readOnly: boolean
  status: 'pass' | 'fail'
  durationMs: number
  verdict?: ProVerdict
  failure?: string
}>

export const proTargetMatrixTable = <Message>(
  rows: ReadonlyArray<ProTargetMatrixRow>,
): Html => {
  const h = html<Message>()
  const headCell = (label: string, alignRight = false): Html =>
    h.th(
      [
        h.Class(
          clsx(
            'border-b border-[#222] px-2.5 py-2 text-[0.625rem] font-semibold uppercase tracking-[0.1em] text-white/30',
            alignRight ? 'text-right' : 'text-left',
          ),
        ),
      ],
      [label],
    )
  return h.table(
    [
      h.DataAttribute('component', 'pro-target-matrix'),
      h.Class('w-full border-collapse text-sm'),
    ],
    [
      h.thead(
        [],
        [
          h.tr(
            [],
            [
              headCell('target'),
              headCell('policy'),
              headCell('status'),
              headCell('verdict'),
              headCell('duration', true),
            ],
          ),
        ],
      ),
      h.tbody(
        [],
        rows.flatMap(row => {
          const main = h.tr(
            [
              h.DataAttribute('target', row.targetName),
              h.Class('border-b border-[#141414]'),
            ],
            [
              h.td(
                [h.Class('px-2.5 py-2')],
                [
                  h.div(
                    [h.Class('grid gap-0.5')],
                    [
                      h.span([h.Class('text-[#f1efe8]')], [row.targetName]),
                      h.span(
                        [h.Class('break-all text-xs text-white/30')],
                        [row.targetBaseUrl],
                      ),
                    ],
                  ),
                ],
              ),
              h.td(
                [h.Class('px-2.5 py-2')],
                [proRestrictionBadge<Message>(row.readOnly)],
              ),
              h.td([h.Class('px-2.5 py-2')], [proStatusPill<Message>(row.status)]),
              h.td(
                [h.Class('px-2.5 py-2')],
                row.verdict !== undefined
                  ? [proVerdictPill<Message>(row.verdict)]
                  : [h.span([h.Class('text-white/25')], ['—'])],
              ),
              h.td(
                [h.Class('px-2.5 py-2 text-right tabular-nums text-white/60')],
                [`${row.durationMs}ms`],
              ),
            ],
          )
          // An honest failure reason row directly under a failed target (e.g. the
          // read-only restriction refusal). Spans the table so the reason is
          // legible without truncation.
          if (row.failure === undefined) return [main]
          const reason = h.tr(
            [
              h.DataAttribute('component', 'pro-target-failure'),
              h.Class('border-b border-[#141414]'),
            ],
            [
              h.td(
                [
                  h.Attribute('colspan', '5'),
                  h.Class('px-2.5 pb-2 text-xs leading-[1.5] text-[#d32f2f]/85'),
                ],
                [row.failure],
              ),
            ],
          )
          return [main, reason]
        }),
      ),
    ],
  )
}
