import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  checkoutForm,
  commerceLineList,
  descriptionList,
  feedList,
  filterPanel,
  orderSummary,
  productGrid,
  statGrid,
} from './data-display'
import { emptyState } from './feedback'
import { inputClass } from './forms'
import { container, section } from './layout'
import { progressList } from './navigation'
import {
  eyebrowClass,
  kitFamily,
  metaClass,
  motionPaneOpenClass,
  statusDotClass,
  surfaceActiveClass,
  titleClass,
} from './primitives'
import type {
  CommerceFilterGroup,
  CommerceLineItem,
  CommerceProductItem,
  CommerceSummaryLine,
  DescriptionItem,
  DetailScreenSection,
  FeedItem,
  ProgressStep,
  StatItem,
  Tone,
} from './primitives'
import { button, headingBlock, linkButton } from './shared'

export type BillingCreditPackage<Message> = Readonly<{
  id: string
  label: string
  amount: string
  detail: string
  attrs?: ReadonlyArray<Attribute<Message>>
}>

export type BillingLedgerDisplayItem = Readonly<{
  id: string
  description: string
  amountFormatted: string
  source: string
  createdAt: string
}>

export type BillingActiveRunDisplayItem = Readonly<{
  id: string
  title: string
  status: string
  accruedSeconds: number
  estimatedDebitFormatted: string
}>

export type BillingAutoTopUpDisplay<Message> = Readonly<{
  amountFormatted: string
  cardLabel: string
  enabled: boolean
  events: ReadonlyArray<
    Readonly<{
      amountFormatted: string
      createdAt: string
      id: string
      status: string
    }>
  >
  monthlyCapFormatted: string
  pauseReason: string | null
  spentThisMonthFormatted: string
  status: string
  thresholdFormatted: string
  cardSetupAttrs: ReadonlyArray<Attribute<Message>>
  disableAttrs: ReadonlyArray<Attribute<Message>>
  enableAttrs: ReadonlyArray<Attribute<Message>>
  runAttrs: ReadonlyArray<Attribute<Message>>
}>

export type UsageTotalsDisplay = Readonly<{
  inputTokens: string
  outputTokens: string
  reasoningTokens: string
  cacheReadTokens: string
  cacheWriteTokens: string
  totalTokens: string
  usageEvents: string
}>

export type UsageRunDisplayItem = Readonly<{
  id: string
  title: string
  repository: string
  status: string
  runnerId: string
  updatedAt: string
  totals: UsageTotalsDisplay
}>

export type UsageTeamDisplayItem = Readonly<{
  id: string
  name: string
  slug: string | null
  totals: UsageTotalsDisplay
}>

export type UsageUserDisplayItem = Readonly<{
  id: string
  displayName: string
  handle: string
  totals: UsageTotalsDisplay
}>

export const billingCreditsPage = <Message>(input: {
  balanceFormatted: string
  status: string
  minimumRunCreditFormatted: string
  containerRateLabel: string
  codexRateLabel: string
  couponCode: string
  actionStatus: 'idle' | 'busy' | 'success' | 'error'
  actionMessage?: string
  couponFormAttrs: ReadonlyArray<Attribute<Message>>
  couponInputAttrs: ReadonlyArray<Attribute<Message>>
  packages: ReadonlyArray<BillingCreditPackage<Message>>
  recentEntries: ReadonlyArray<BillingLedgerDisplayItem>
  activeRuns: ReadonlyArray<BillingActiveRunDisplayItem>
  autoTopUp: BillingAutoTopUpDisplay<Message>
}): Html => {
  const h = html<Message>()
  const actionTone =
    input.actionStatus === 'success'
      ? 'positive'
      : input.actionStatus === 'error'
        ? 'negative'
        : input.actionStatus === 'busy'
          ? 'accent'
          : 'neutral'

  return container<Message>(
    [
      pageHeader<Message>({
        eyebrow: 'Billing',
        title: 'Usage billing',
        body: 'Credits are the USD balance Autopilot draws down for computer time and Codex token usage.',
      }),
      statGrid<Message>(
        [
          {
            label: 'Available balance',
            value: input.balanceFormatted,
            tone: input.status === 'active' ? 'positive' : 'warning',
          },
          {
            label: 'Computer time',
            value: input.containerRateLabel,
            tone: 'accent',
          },
          {
            label: 'Codex usage',
            value: input.codexRateLabel,
            tone: 'info',
          },
        ],
        [h.Class('mt-4')],
      ),
      h.div(
        [
          h.Class(
            'mt-4 grid gap-4 lg:grid-cols-[minmax(0,1.25fr)_minmax(320px,0.75fr)]',
          ),
        ],
        [
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Add credits',
                title: 'Credit packages',
                body: 'Choose a package to add prepaid credits. Subscription billing is not enabled.',
                level: 2,
              }),
              h.div(
                [
                  h.Class(
                    'mt-4 grid gap-px border border-[#222] bg-[#222] sm:grid-cols-3',
                  ),
                ],
                input.packages.map(item =>
                  h.div(
                    [
                      h.Class(
                        clsx(
                          motionPaneOpenClass,
                          'grid min-h-[180px] gap-4 bg-[#010102] p-4',
                        ),
                      ),
                    ],
                    [
                      h.div(
                        [h.Class('min-w-0')],
                        [
                          h.p(
                            [h.Class(clsx(eyebrowClass, 'mb-2'))],
                            [item.label],
                          ),
                          h.p(
                            [
                              h.Class(
                                'm-0 text-3xl font-semibold text-[#f1efe8]',
                              ),
                            ],
                            [item.amount],
                          ),
                          h.p(
                            [h.Class(clsx(metaClass, 'mt-3'))],
                            [item.detail],
                          ),
                        ],
                      ),
                      h.div(
                        [h.Class('self-end')],
                        [
                          button<Message>({
                            label: 'Add credits',
                            size: 'sm',
                            variant: 'secondary',
                            ...(item.attrs === undefined
                              ? {}
                              : { attrs: item.attrs }),
                            block: true,
                          }),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ],
            [h.Class('min-w-0')],
          ),
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Coupons',
                title: 'Redeem code',
                body: `New runs require at least ${input.minimumRunCreditFormatted} available credit.`,
                level: 2,
              }),
              h.form(
                [...input.couponFormAttrs, h.Class('mt-4 grid gap-3')],
                [
                  h.label(
                    [h.Class('grid gap-1.5')],
                    [
                      h.span([h.Class(eyebrowClass)], ['Coupon code']),
                      h.input([
                        ...input.couponInputAttrs,
                        h.Name('coupon-code'),
                        h.Type('text'),
                        h.Value(input.couponCode),
                        h.Placeholder('OPENAGENTS-TRIAL'),
                        h.Class(inputClass),
                      ]),
                    ],
                  ),
                  button<Message>({
                    label:
                      input.actionStatus === 'busy'
                        ? 'Applying...'
                        : 'Apply coupon',
                    ...(input.actionStatus === 'busy'
                      ? { attrs: [h.Disabled(true)] }
                      : {}),
                    size: 'sm',
                    block: true,
                  }),
                  input.actionMessage === undefined
                    ? null
                    : h.div(
                        [
                          h.Class(
                            clsx(
                              'grid grid-cols-[auto_minmax(0,1fr)] gap-2 border p-3 text-sm leading-5',
                              {
                                'border-[#222] text-white/55':
                                  actionTone === 'neutral',
                                'border-[#ffb400]/70 text-[#ffb400]':
                                  actionTone === 'accent',
                                'border-[#00c853]/70 text-[#00c853]':
                                  actionTone === 'positive',
                                'border-[#d32f2f]/70 text-[#d32f2f]':
                                  actionTone === 'negative',
                              },
                            ),
                          ),
                        ],
                        [
                          h.span([h.Class(statusDotClass(actionTone))], []),
                          h.span([h.Class('min-w-0')], [input.actionMessage]),
                        ],
                      ),
                ],
              ),
            ],
            [h.Class('min-w-0')],
          ),
        ],
      ),
      section<Message>(
        [
          headingBlock<Message>({
            eyebrow: 'Auto top-up',
            title: 'Card on file',
            body: 'Saved cards are stored by Stripe. OpenAgents stores only card metadata and Stripe payment-method IDs.',
            level: 2,
          }),
          h.div(
            [
              h.Class(
                'mt-4 grid gap-px border border-[#222] bg-[#222] lg:grid-cols-[minmax(0,1fr)_minmax(280px,0.8fr)]',
              ),
            ],
            [
              h.div(
                [h.Class('grid gap-4 bg-[#010102] p-4')],
                [
                  descriptionList<Message>([
                    { label: 'Card', value: input.autoTopUp.cardLabel },
                    {
                      label: 'Policy',
                      value: input.autoTopUp.enabled ? 'Enabled' : 'Disabled',
                    },
                    {
                      label: 'Threshold',
                      value: input.autoTopUp.thresholdFormatted,
                    },
                    { label: 'Top-up', value: input.autoTopUp.amountFormatted },
                    {
                      label: 'Monthly cap',
                      value: input.autoTopUp.monthlyCapFormatted,
                    },
                    {
                      label: 'Used this month',
                      value: input.autoTopUp.spentThisMonthFormatted,
                    },
                    {
                      label: 'Status',
                      value:
                        input.autoTopUp.pauseReason === null
                          ? input.autoTopUp.status
                          : `${input.autoTopUp.status} - ${input.autoTopUp.pauseReason}`,
                    },
                  ]),
                  h.div(
                    [h.Class('grid gap-2 sm:grid-cols-4')],
                    [
                      button<Message>({
                        label: 'Manage card',
                        size: 'sm',
                        variant: 'secondary',
                        attrs: input.autoTopUp.cardSetupAttrs,
                      }),
                      button<Message>({
                        label: 'Enable',
                        size: 'sm',
                        variant: 'secondary',
                        attrs: input.autoTopUp.enableAttrs,
                      }),
                      button<Message>({
                        label: 'Disable',
                        size: 'sm',
                        variant: 'secondary',
                        attrs: input.autoTopUp.disableAttrs,
                      }),
                      button<Message>({
                        label: 'Check now',
                        size: 'sm',
                        variant: 'secondary',
                        attrs: input.autoTopUp.runAttrs,
                      }),
                    ],
                  ),
                ],
              ),
              h.div(
                [h.Class('grid gap-px bg-[#222]')],
                input.autoTopUp.events.length === 0
                  ? [
                      h.div(
                        [h.Class('bg-[#010102] p-4 text-sm text-white/45')],
                        ['No auto top-up events yet.'],
                      ),
                    ]
                  : input.autoTopUp.events.map(event =>
                      h.div(
                        [h.Class('grid gap-1 bg-[#010102] p-4')],
                        [
                          h.p([h.Class(titleClass)], [event.status]),
                          h.p(
                            [h.Class(metaClass)],
                            [`${event.amountFormatted} - ${event.createdAt}`],
                          ),
                        ],
                      ),
                    ),
              ),
            ],
          ),
        ],
        [h.Class('mt-4 min-w-0')],
      ),
      h.div(
        [h.Class('mt-4 grid gap-4 lg:grid-cols-2')],
        [
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Usage',
                title: 'Running now',
                body: 'Active runs accrue computer-time debit as runner events are ingested.',
                level: 2,
              }),
              h.div(
                [h.Class('mt-4 grid gap-px border border-[#222] bg-[#222]')],
                input.activeRuns.length === 0
                  ? [
                      h.div(
                        [h.Class('bg-[#010102] p-4 text-sm text-white/45')],
                        ['No active metered runs.'],
                      ),
                    ]
                  : input.activeRuns.map(run =>
                      h.div(
                        [
                          h.Class(
                            'grid gap-2 bg-[#010102] p-4 sm:grid-cols-[minmax(0,1fr)_auto]',
                          ),
                        ],
                        [
                          h.div(
                            [h.Class('min-w-0')],
                            [
                              h.p([h.Class(titleClass)], [run.title]),
                              h.p(
                                [h.Class(metaClass)],
                                [
                                  `${run.status} - ${run.accruedSeconds}s accrued`,
                                ],
                              ),
                            ],
                          ),
                          h.p(
                            [h.Class('m-0 text-sm text-[#ffb400]')],
                            [run.estimatedDebitFormatted],
                          ),
                        ],
                      ),
                    ),
              ),
            ],
            [h.Class('min-w-0')],
          ),
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Ledger',
                title: 'Recent entries',
                body: 'Positive entries add credits. Negative entries are usage debits.',
                level: 2,
              }),
              h.div(
                [h.Class('mt-4 grid gap-px border border-[#222] bg-[#222]')],
                input.recentEntries.length === 0
                  ? [
                      h.div(
                        [h.Class('bg-[#010102] p-4 text-sm text-white/45')],
                        ['No billing ledger entries yet.'],
                      ),
                    ]
                  : input.recentEntries.map(entry =>
                      h.div(
                        [
                          h.Class(
                            'grid gap-2 bg-[#010102] p-4 sm:grid-cols-[minmax(0,1fr)_auto]',
                          ),
                        ],
                        [
                          h.div(
                            [h.Class('min-w-0')],
                            [
                              h.p([h.Class(titleClass)], [entry.description]),
                              h.p(
                                [h.Class(metaClass)],
                                [`${entry.source} - ${entry.createdAt}`],
                              ),
                            ],
                          ),
                          h.p(
                            [
                              h.Class(
                                clsx(
                                  'm-0 text-sm',
                                  entry.amountFormatted.startsWith('-')
                                    ? 'text-[#d32f2f]'
                                    : 'text-[#00c853]',
                                ),
                              ),
                            ],
                            [entry.amountFormatted],
                          ),
                        ],
                      ),
                    ),
              ),
            ],
            [h.Class('min-w-0')],
          ),
        ],
      ),
    ],
    [h.Class('py-4')],
  )
}

const usageBreakdownRows = <Message>(
  totals: UsageTotalsDisplay,
): ReadonlyArray<Html> => {
  const h = html<Message>()
  const rows: ReadonlyArray<readonly [string, string]> = [
    ['Input', totals.inputTokens],
    ['Output', totals.outputTokens],
    ['Reasoning', totals.reasoningTokens],
    ['Cache read', totals.cacheReadTokens],
    ['Cache write', totals.cacheWriteTokens],
    ['Events', totals.usageEvents],
  ]

  return rows.map(([label, value]) =>
    h.div(
      [
        h.Class(
          'grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-t border-[#222] px-3 py-2 text-[0.8125rem]',
        ),
      ],
      [
        h.span([h.Class('text-white/45')], [label]),
        h.span([h.Class('text-[#f1efe8]')], [value]),
      ],
    ),
  )
}

const usageTotalsPane = <Message>(input: {
  eyebrow: string
  title: string
  body: string
  totals: UsageTotalsDisplay
  tone?: Tone
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        clsx(motionPaneOpenClass, 'grid border border-[#222] bg-[#010102]'),
      ),
    ],
    [
      h.div(
        [h.Class('grid gap-2 p-4')],
        [
          h.p([h.Class(clsx(eyebrowClass, 'm-0'))], [input.eyebrow]),
          h.div(
            [h.Class('grid grid-cols-[minmax(0,1fr)_auto] items-end gap-3')],
            [
              h.div(
                [h.Class('min-w-0')],
                [
                  h.h2(
                    [h.Class('m-0 text-lg font-medium text-[#f1efe8]')],
                    [input.title],
                  ),
                  h.p([h.Class(clsx(metaClass, 'mt-1'))], [input.body]),
                ],
              ),
              h.p(
                [
                  h.Class(
                    clsx('m-0 text-2xl font-semibold', {
                      'text-[#f1efe8]':
                        input.tone === undefined || input.tone === 'neutral',
                      'text-[#ffb400]': input.tone === 'accent',
                      'text-[#00c853]': input.tone === 'positive',
                      'text-[#ff6f00]': input.tone === 'warning',
                      'text-[#d32f2f]': input.tone === 'negative',
                      'text-[#2979ff]': input.tone === 'info',
                    }),
                  ),
                ],
                [input.totals.totalTokens],
              ),
            ],
          ),
        ],
      ),
      ...usageBreakdownRows<Message>(input.totals),
    ],
  )
}

const usageRunRow = <Message>(run: UsageRunDisplayItem): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class('grid gap-3 bg-[#010102] p-4 lg:grid-cols-[minmax(0,1fr)_auto]')],
    [
      h.div(
        [h.Class('min-w-0')],
        [
          h.p([h.Class(titleClass)], [run.title]),
          h.p(
            [h.Class(metaClass)],
            [`${run.repository} - ${run.status} - ${run.runnerId}`],
          ),
          h.p([h.Class(clsx(metaClass, 'mt-1'))], [run.updatedAt]),
        ],
      ),
      h.div(
        [h.Class('grid min-w-[16rem] gap-1 text-[0.8125rem] text-white/45')],
        [
          h.div(
            [h.Class('flex justify-between gap-3')],
            [
              h.span([], ['Total']),
              h.span([h.Class('text-[#f1efe8]')], [run.totals.totalTokens]),
            ],
          ),
          h.div(
            [h.Class('flex justify-between gap-3')],
            [
              h.span([], ['In / out']),
              h.span(
                [h.Class('text-[#f1efe8]')],
                [`${run.totals.inputTokens} / ${run.totals.outputTokens}`],
              ),
            ],
          ),
          h.div(
            [h.Class('flex justify-between gap-3')],
            [
              h.span([], ['Reasoning']),
              h.span([h.Class('text-[#f1efe8]')], [run.totals.reasoningTokens]),
            ],
          ),
        ],
      ),
    ],
  )
}

const usageRankRow = <Message>(input: {
  title: string
  detail: string
  totals: UsageTotalsDisplay
}): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class('grid gap-3 bg-[#010102] p-3 sm:grid-cols-[minmax(0,1fr)_auto]')],
    [
      h.div(
        [h.Class('min-w-0')],
        [
          h.p([h.Class(titleClass)], [input.title]),
          h.p([h.Class(metaClass)], [input.detail]),
        ],
      ),
      h.p([h.Class('m-0 text-sm text-[#f1efe8]')], [input.totals.totalTokens]),
    ],
  )
}

export const usageTelemetryPage = <Message>(input: {
  generatedAt: string
  currentUser: UsageTotalsDisplay
  global: UsageTotalsDisplay
  missingUsageSignals: string
  recentRuns: ReadonlyArray<UsageRunDisplayItem>
  teams: ReadonlyArray<UsageTeamDisplayItem>
  users: ReadonlyArray<UsageUserDisplayItem>
}): Html => {
  const h = html<Message>()

  return container<Message>(
    [
      pageHeader<Message>({
        eyebrow: 'Usage',
        title: 'Token telemetry',
        body: 'Provider token usage is ledgered from runner events, not inferred from subscription billing.',
      }),
      statGrid<Message>(
        [
          {
            label: 'My tokens',
            value: input.currentUser.totalTokens,
            tone: 'accent',
          },
          {
            label: 'Global tokens',
            value: input.global.totalTokens,
            tone: 'info',
          },
          {
            label: 'Missing signals',
            value: input.missingUsageSignals,
            tone: input.missingUsageSignals === '0' ? 'positive' : 'negative',
          },
        ],
        [h.Class('mt-4')],
      ),
      h.div(
        [h.Class('mt-4 grid gap-4 xl:grid-cols-2')],
        [
          usageTotalsPane<Message>({
            eyebrow: 'Current account',
            title: 'My usage',
            body: `${input.currentUser.usageEvents} ledgered events`,
            totals: input.currentUser,
            tone: 'accent',
          }),
          usageTotalsPane<Message>({
            eyebrow: 'All accounts',
            title: 'Global usage',
            body: `${input.global.usageEvents} ledgered events`,
            totals: input.global,
            tone: 'info',
          }),
        ],
      ),
      section<Message>(
        [
          headingBlock<Message>({
            eyebrow: 'Threads',
            title: 'Recent token rows',
            body: 'Rows with zero tokens have no ledgered provider usage event yet.',
            level: 2,
          }),
          h.div(
            [h.Class('mt-4 grid gap-px border border-[#222] bg-[#222]')],
            input.recentRuns.length === 0
              ? [
                  h.div(
                    [h.Class('bg-[#010102] p-4 text-sm text-white/45')],
                    ['No thread usage rows yet.'],
                  ),
                ]
              : input.recentRuns.map(usageRunRow<Message>),
          ),
        ],
        [h.Class('mt-4')],
      ),
      h.div(
        [h.Class('mt-4 grid gap-4 xl:grid-cols-2')],
        [
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Teams',
                title: 'Team totals',
                body: 'Team rows are ranked by ledgered total tokens.',
                level: 2,
              }),
              h.div(
                [h.Class('mt-4 grid gap-px border border-[#222] bg-[#222]')],
                input.teams.length === 0
                  ? [
                      h.div(
                        [h.Class('bg-[#010102] p-4 text-sm text-white/45')],
                        ['No team token usage yet.'],
                      ),
                    ]
                  : input.teams.map(team =>
                      usageRankRow<Message>({
                        title: team.name,
                        detail: team.slug ?? team.id,
                        totals: team.totals,
                      }),
                    ),
              ),
            ],
            [h.Class('min-w-0')],
          ),
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Users',
                title: 'User totals',
                body: 'User rows are ranked by ledgered total tokens.',
                level: 2,
              }),
              h.div(
                [h.Class('mt-4 grid gap-px border border-[#222] bg-[#222]')],
                input.users.length === 0
                  ? [
                      h.div(
                        [h.Class('bg-[#010102] p-4 text-sm text-white/45')],
                        ['No user token usage yet.'],
                      ),
                    ]
                  : input.users.map(user =>
                      usageRankRow<Message>({
                        title: user.displayName,
                        detail: user.handle,
                        totals: user.totals,
                      }),
                    ),
              ),
            ],
            [h.Class('min-w-0')],
          ),
        ],
      ),
    ],
    [h.Class('py-4')],
  )
}

export const pageHeader = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.header(
    [
      kitFamily<Message>('headings/page-headings'),
      h.Class(
        'grid gap-4 border-b border-[#222] bg-[#010102] px-4 py-5 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end',
      ),
    ],
    [
      headingBlock<Message>({
        title: input.title,
        ...(input.eyebrow === undefined ? {} : { eyebrow: input.eyebrow }),
        ...(input.body === undefined ? {} : { body: input.body }),
        level: 1,
      }),
      input.action ?? null,
    ],
  )
}

export const actionPanel = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  action?: Html
  tone?: Tone
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('forms/action-panels'),
      h.Class(
        clsx(
          'grid gap-4 border bg-[#010102] p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center',
          {
            'border-[#222]':
              input.tone === undefined || input.tone === 'neutral',
            'border-[#ffb400]': input.tone === 'accent',
            'border-[#00c853]': input.tone === 'positive',
            'border-[#ff6f00]': input.tone === 'warning',
            'border-[#d32f2f]': input.tone === 'negative',
            'border-[#2979ff]': input.tone === 'info',
          },
        ),
      ),
    ],
    [
      headingBlock<Message>({
        eyebrow: input.eyebrow ?? 'Action',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
        level: 3,
      }),
      input.action ?? null,
    ],
  )
}

export const applicationHomeScreen = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  stats?: ReadonlyArray<StatItem>
  steps?: ReadonlyArray<ProgressStep>
  aside?: Html
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('page-examples/home-screens'),
      h.Class(surfaceActiveClass),
    ],
    [
      pageHeader<Message>({
        eyebrow: input.eyebrow ?? 'OpenAgents',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
        ...(input.action === undefined ? {} : { action: input.action }),
      }),
      h.div(
        [
          h.Class(
            'grid gap-4 p-4 lg:grid-cols-[minmax(0,1.3fr)_minmax(280px,0.7fr)]',
          ),
        ],
        [
          h.div(
            [h.Class('grid content-start gap-4')],
            [
              input.stats === undefined ? null : statGrid<Message>(input.stats),
              input.steps === undefined
                ? null
                : progressList<Message>(input.steps),
            ],
          ),
          input.aside ?? null,
        ],
      ),
    ],
  )
}

export const applicationDetailScreen = <Message>(input: {
  eyebrow?: string
  title: string
  body?: string
  stats?: ReadonlyArray<StatItem>
  sections: ReadonlyArray<DetailScreenSection>
  activity?: ReadonlyArray<FeedItem>
  aside?: Html
  action?: Html
  variant?: 'sidebar' | 'stacked'
}): Html => {
  const h = html<Message>()
  const content = [
    h.div(
      [h.Class('grid content-start gap-4')],
      [
        ...(input.stats === undefined || input.stats.length === 0
          ? []
          : [statGrid<Message>(input.stats)]),
        ...input.sections.map(sectionInput =>
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: sectionInput.eyebrow ?? 'Details',
                title: sectionInput.title,
                ...(sectionInput.body === undefined
                  ? {}
                  : { body: sectionInput.body }),
                level: 3,
              }),
              sectionInput.details === undefined ||
              sectionInput.details.length === 0
                ? null
                : h.div(
                    [h.Class('mt-4')],
                    [descriptionList<Message>(sectionInput.details)],
                  ),
              sectionInput.action === undefined
                ? null
                : h.div([h.Class('mt-4')], [sectionInput.action]),
            ],
            [kitFamily<Message>('data-display/description-lists')],
          ),
        ),
        input.activity === undefined || input.activity.length === 0
          ? null
          : section<Message>(
              [
                headingBlock<Message>({
                  eyebrow: 'Activity',
                  title: 'Timeline',
                  level: 3,
                }),
                h.div([h.Class('mt-4')], [feedList<Message>(input.activity)]),
              ],
              [kitFamily<Message>('lists/feeds')],
            ),
      ],
    ),
    input.aside === undefined
      ? null
      : h.aside([h.Class('grid content-start gap-3')], [input.aside]),
  ]

  return h.section(
    [
      kitFamily<Message>('page-examples/detail-screens'),
      h.Class(surfaceActiveClass),
    ],
    [
      pageHeader<Message>({
        eyebrow: input.eyebrow ?? 'Detail',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
        ...(input.action === undefined ? {} : { action: input.action }),
      }),
      h.div(
        [
          h.Class(
            clsx('grid gap-4 p-4', {
              'lg:grid-cols-[minmax(0,1fr)_320px]':
                input.variant !== 'stacked' && input.aside !== undefined,
            }),
          ),
        ],
        content,
      ),
    ],
  )
}

export const settingsScreen = <Message>(input: {
  title: string
  body?: string
  details: ReadonlyArray<DescriptionItem>
  actions?: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('page-examples/settings-screens'),
      h.Class(surfaceActiveClass),
    ],
    [
      pageHeader<Message>({
        eyebrow: 'Settings',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
      }),
      h.div(
        [h.Class('grid gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_320px]')],
        [
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Account',
                title: 'Profile',
                level: 3,
              }),
              h.div(
                [h.Class('mt-4')],
                [descriptionList<Message>(input.details)],
              ),
            ],
            [kitFamily<Message>('data-display/description-lists')],
          ),
          h.aside(
            [h.Class('grid content-start gap-3')],
            input.actions ?? [
              emptyState<Message>({
                title: 'No actions',
                body: 'There are no session actions available.',
              }),
            ],
          ),
        ],
      ),
    ],
  )
}

export type SettingsWorkspaceSectionKey =
  | 'general'
  | 'connections'
  | 'organization'
  | 'members'

export type SettingsWorkspaceTeam = Readonly<{
  id: string
  name: string
  slug: string
  role: string
  memberCount: number
}>

export type SettingsWorkspaceMember = Readonly<{
  id: string
  name: string
  detail: string
  role: string
  avatarUrl?: string
}>

export type SettingsProviderAccount = Readonly<{
  id: string
  providerAccountRef: string
  status: string
  publicStatus: string
  health: string
  hasSecretRef: boolean
  accountLabel?: string | undefined
  planType?: string | undefined
  lastStatusAt: string
}>

export type SettingsProviderConnectionAttempt = Readonly<{
  id: string
  providerAccountRef: string
  status: string
  verificationUrl?: string | undefined
  userCode?: string | undefined
  expiresAt: string
}>

export type SettingsProviderConnectionAction =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'starting' }>
  | Readonly<{ kind: 'polling'; attemptId: string }>
  | Readonly<{ kind: 'succeeded'; message: string }>
  | Readonly<{ kind: 'failed'; error: string }>

export type SettingsProviderAccountAction<Message> = Readonly<{
  providerAccountRef: string
  attrs: ReadonlyArray<Attribute<Message>>
}>

const settingsWorkspaceFallback = (
  value: string | null | undefined,
  empty = 'not available',
): string =>
  value === null || value === undefined || value.trim() === ''
    ? empty
    : value.trim()

const settingsWorkspaceTitle = (
  section: SettingsWorkspaceSectionKey,
): string =>
  section === 'general'
    ? 'General'
    : `${section.slice(0, 1).toUpperCase()}${section.slice(1)}`

const settingsWorkspaceProfileImage = <Message>(input: {
  name: string
  avatarUrl?: string
}): Html => {
  const h = html<Message>()

  if (input.avatarUrl !== undefined && input.avatarUrl !== '') {
    return h.img([
      h.Src(input.avatarUrl),
      h.Alt(''),
      h.Class('size-20 border border-[#333] object-cover'),
    ])
  }

  return h.div(
    [
      h.Class(
        'grid size-20 place-items-center border border-[#333] bg-[#151515] text-2xl text-[#f1efe8]',
      ),
    ],
    [input.name.slice(0, 1).toUpperCase()],
  )
}

const settingsWorkspacePanel = <Message>(input: {
  eyebrow: string
  title: string
  body?: string
  children: ReadonlyArray<Html | string>
}): Html =>
  section<Message>(
    [
      headingBlock<Message>({
        eyebrow: input.eyebrow,
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
        level: 2,
      }),
      html<Message>().div([html<Message>().Class('mt-4')], input.children),
    ],
    [html<Message>().Class('min-w-0')],
  )

const settingsWorkspaceRows = <Message>(
  items: ReadonlyArray<Readonly<{ label: string; value: string | Html }>>,
): Html => {
  const h = html<Message>()

  return h.dl(
    [h.Class('grid border-t border-[#222]')],
    items.map(item =>
      h.div(
        [
          h.Class(
            'grid grid-cols-[minmax(8rem,0.42fr)_minmax(0,1fr)] gap-4 border-b border-[#222] py-3 text-sm',
          ),
        ],
        [
          h.dt([h.Class('min-w-0 text-white/35')], [item.label]),
          h.dd(
            [
              h.Class(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80',
              ),
            ],
            [item.value],
          ),
        ],
      ),
    ),
  )
}

const settingsWorkspaceStatusLine = <Message>(input: {
  title: string
  detail: string
  tone?: Tone
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        'grid grid-cols-[auto_minmax(0,1fr)] items-start gap-3 border-b border-[#222] py-3 text-sm last:border-b-0',
      ),
    ],
    [
      h.span([h.Class(statusDotClass(input.tone ?? 'neutral'))], []),
      h.span(
        [h.Class('grid min-w-0 gap-1')],
        [
          h.span([h.Class('text-[#f1efe8]')], [input.title]),
          h.span([h.Class('text-white/40')], [input.detail]),
        ],
      ),
    ],
  )
}

const settingsWorkspaceTeamList = <Message>(
  teams: ReadonlyArray<SettingsWorkspaceTeam>,
): Html => {
  const h = html<Message>()

  if (teams.length === 0) {
    return h.div([h.Class('text-sm text-white/45')], ['No teams in session.'])
  }

  return h.div(
    [h.Class('grid gap-px border border-[#222] bg-[#222]')],
    teams.map(team =>
      h.div(
        [
          h.Class(
            'grid gap-3 bg-[#010102] p-4 sm:grid-cols-[minmax(0,1fr)_auto]',
          ),
        ],
        [
          h.div(
            [h.Class('min-w-0')],
            [
              h.p(
                [
                  h.Class(
                    'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm text-[#f1efe8]',
                  ),
                ],
                [team.name],
              ),
              h.p(
                [h.Class('m-0 mt-1 text-[0.8125rem] text-white/40')],
                [team.slug],
              ),
            ],
          ),
          h.div(
            [h.Class('text-right text-sm text-white/60')],
            [`${team.memberCount} members`],
          ),
        ],
      ),
    ),
  )
}

const settingsWorkspaceMemberList = <Message>(
  members: ReadonlyArray<SettingsWorkspaceMember>,
): Html => {
  const h = html<Message>()

  if (members.length === 0) {
    return h.div(
      [h.Class('text-sm text-white/45')],
      ['No team members in the current bootstrap.'],
    )
  }

  return h.div(
    [h.Class('grid gap-px border border-[#222] bg-[#222]')],
    members.map(member =>
      h.div(
        [
          h.Class(
            'grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 bg-[#010102] p-3',
          ),
        ],
        [
          member.avatarUrl === undefined || member.avatarUrl === ''
            ? h.div(
                [
                  h.Class(
                    'grid size-9 place-items-center border border-[#333] bg-[#151515] text-sm text-[#f1efe8]',
                  ),
                ],
                [member.name.slice(0, 1).toUpperCase()],
              )
            : h.img([
                h.Src(member.avatarUrl),
                h.Alt(''),
                h.Class('size-9 border border-[#333] object-cover'),
              ]),
          h.div(
            [h.Class('min-w-0')],
            [
              h.p(
                [
                  h.Class(
                    'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm text-[#f1efe8]',
                  ),
                ],
                [member.name],
              ),
              h.p(
                [h.Class('m-0 text-[0.8125rem] text-white/40')],
                [member.detail],
              ),
            ],
          ),
          h.span([h.Class('text-[0.8125rem] text-white/45')], [member.role]),
        ],
      ),
    ),
  )
}

const providerAccountTone = (
  account: SettingsProviderAccount | undefined,
): Tone => {
  if (account === undefined) {
    return 'warning'
  }

  if (
    account.status === 'connected' &&
    account.publicStatus === 'connected' &&
    account.health === 'healthy' &&
    account.hasSecretRef
  ) {
    return 'positive'
  }

  if (account.health === 'requires_reauth' || account.status === 'unhealthy') {
    return 'negative'
  }

  return 'warning'
}

const providerAccountStatus = (
  account: SettingsProviderAccount | undefined,
): string => {
  if (account === undefined) {
    return 'not connected'
  }

  if (account.health === 'requires_reauth') {
    return 'requires reconnect'
  }

  if (!account.hasSecretRef) {
    return 'missing login'
  }

  return account.publicStatus
}

const providerConnectionActionText = (
  action: SettingsProviderConnectionAction,
): string | undefined => {
  if (action.kind === 'starting') {
    return 'Preparing an OpenAI device login. No OpenAgents password entry is expected.'
  }

  if (action.kind === 'polling') {
    return 'Open the OpenAI device page, enter the code, then return here. OpenAgents is checking for completion.'
  }

  if (action.kind === 'succeeded') {
    return action.message
  }

  if (action.kind === 'failed') {
    return action.error
  }

  return undefined
}

const settingsProviderConnectionPanel = <Message>(input: {
  accounts: ReadonlyArray<SettingsProviderAccount>
  attempts: ReadonlyArray<SettingsProviderConnectionAttempt>
  action: SettingsProviderConnectionAction
  startAttrs: ReadonlyArray<Attribute<Message>>
  reconnectAttrs: ReadonlyArray<SettingsProviderAccountAction<Message>>
  pollAttrs: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const attempt =
    input.attempts.find(item => item.status === 'pending') ?? input.attempts[0]
  const actionText = providerConnectionActionText(input.action)
  const isBusy =
    input.action.kind === 'starting' || input.action.kind === 'polling'
  const pendingAttempt =
    attempt !== undefined && attempt.status === 'pending' ? attempt : undefined
  const reconnectAttrsFor = (
    account: SettingsProviderAccount,
  ): ReadonlyArray<Attribute<Message>> =>
    input.reconnectAttrs.find(
      item => item.providerAccountRef === account.providerAccountRef,
    )?.attrs ?? []

  return settingsWorkspacePanel<Message>({
    eyebrow: 'Connections',
    title: 'ChatGPT accounts',
    children: [
      input.accounts.length === 0
        ? settingsWorkspaceStatusLine<Message>({
            title: 'not connected',
            detail: 'No account connected',
            tone: 'warning',
          })
        : h.div(
            [h.Class('grid gap-px border border-[#222] bg-[#222]')],
            input.accounts.map(account =>
              h.div(
                [
                  h.Class(
                    'grid gap-3 bg-[#010102] p-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center',
                  ),
                ],
                [
                  h.div(
                    [h.Class('min-w-0')],
                    [
                      settingsWorkspaceStatusLine<Message>({
                        title: providerAccountStatus(account),
                        detail:
                          account.accountLabel ?? account.providerAccountRef,
                        tone: providerAccountTone(account),
                      }),
                      settingsWorkspaceRows<Message>([
                        { label: 'Health', value: account.health },
                        { label: 'Status', value: account.publicStatus },
                        { label: 'Updated', value: account.lastStatusAt },
                      ]),
                    ],
                  ),
                  button<Message>({
                    label:
                      account.health === 'requires_reauth' ||
                      account.publicStatus !== 'connected'
                        ? 'Reconnect'
                        : 'Refresh login',
                    size: 'sm',
                    variant:
                      providerAccountTone(account) === 'negative'
                        ? 'primary'
                        : 'secondary',
                    attrs: [
                      ...reconnectAttrsFor(account),
                      ...(isBusy ? [h.Disabled(true)] : []),
                    ],
                  }),
                ],
              ),
            ),
          ),
      h.div(
        [h.Class('mt-4 border border-[#222] p-4')],
        [
          h.p([h.Class('m-0 text-sm text-[#f1efe8]')], ['Device code login']),
          h.ul(
            [
              h.Class(
                'm-0 mt-3 grid list-disc gap-2 pl-5 text-sm leading-6 text-white/50',
              ),
            ],
            [
              h.li(
                [],
                ['Enable device code login in ChatGPT security settings.'],
              ),
              h.li(
                [],
                ['Workspace admins enable it in workspace permissions.'],
              ),
              h.li(
                [],
                ['Reconnect here, open the device page, then enter the code.'],
              ),
              h.li(
                [],
                [
                  'If OpenAI rate limits the request, wait a minute and try again.',
                ],
              ),
            ],
          ),
        ],
      ),
      pendingAttempt === undefined
        ? null
        : h.div(
            [
              h.Class(
                'mt-4 grid gap-4 border border-[#ffb400] bg-[#ffb400]/10 p-4',
              ),
            ],
            [
              h.div(
                [h.Class('grid gap-1')],
                [
                  h.p(
                    [h.Class('m-0 text-sm font-medium text-[#f1efe8]')],
                    ['Open the OpenAI device page and enter this code'],
                  ),
                  h.p(
                    [h.Class('m-0 text-sm leading-6 text-white/60')],
                    [
                      'OpenAgents does not ask for your ChatGPT password here. Sign in with OpenAI on the device page, enter the code, then return here.',
                    ],
                  ),
                ],
              ),
              h.div(
                [
                  h.Class(
                    'w-fit border border-[#333] bg-[#010102] px-3 py-2 font-mono text-2xl tracking-normal text-[#f1efe8]',
                  ),
                ],
                [pendingAttempt.userCode ?? 'pending'],
              ),
              settingsWorkspaceRows<Message>([
                {
                  label: 'Device code',
                  value: pendingAttempt.userCode ?? 'pending',
                },
                { label: 'Expires', value: pendingAttempt.expiresAt },
              ]),
              pendingAttempt.verificationUrl === undefined
                ? null
                : linkButton<Message>({
                    href: pendingAttempt.verificationUrl,
                    label: 'Open OpenAI device page',
                    size: 'sm',
                    variant: 'primary',
                    attrs: [
                      h.Attribute('target', '_blank'),
                      h.Attribute('rel', 'noopener noreferrer'),
                    ],
                  }),
            ],
          ),
      h.div(
        [h.Class('mt-4 flex flex-wrap items-center gap-2')],
        [
          button<Message>({
            label:
              input.action.kind === 'starting'
                ? 'Preparing device login...'
                : input.action.kind === 'polling'
                  ? 'Waiting for OpenAI sign-in'
                  : 'Add ChatGPT account',
            size: 'sm',
            variant: 'primary',
            attrs: [...input.startAttrs, ...(isBusy ? [h.Disabled(true)] : [])],
          }),
          pendingAttempt === undefined
            ? null
            : button<Message>({
                label: 'Check status',
                size: 'sm',
                variant: 'secondary',
                attrs: input.pollAttrs,
              }),
        ],
      ),
      actionText === undefined
        ? null
        : h.p(
            [
              h.Class(
                clsx('m-0 mt-3 text-sm', {
                  'text-[#d32f2f]': input.action.kind === 'failed',
                  'text-white/45': input.action.kind !== 'failed',
                }),
              ),
            ],
            [actionText],
          ),
    ],
  })
}

const providerAccountNeedsConnection = (
  account: SettingsProviderAccount | undefined,
): boolean =>
  account === undefined ||
  account.health === 'requires_reauth' ||
  account.publicStatus !== 'connected' ||
  account.status !== 'connected' ||
  !account.hasSecretRef

const settingsProviderConnectionNotice = <Message>(input: {
  account: SettingsProviderAccount | undefined
}): Html | null => {
  const h = html<Message>()

  if (!providerAccountNeedsConnection(input.account)) {
    return null
  }

  return h.div(
    [
      h.Class(
        'grid gap-3 border border-[#ffb400] bg-[#ffb400]/10 p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center',
      ),
    ],
    [
      h.div(
        [h.Class('min-w-0')],
        [
          h.p(
            [h.Class('m-0 text-sm text-[#f1efe8]')],
            [
              input.account === undefined
                ? 'Connect ChatGPT'
                : 'Reconnect ChatGPT',
            ],
          ),
          h.p(
            [h.Class('m-0 mt-1 text-sm leading-6 text-white/55')],
            [
              input.account === undefined
                ? 'Connect an account before launching Autopilot.'
                : 'Saved ChatGPT login is invalidated. Reconnect before launching Autopilot.',
            ],
          ),
        ],
      ),
      linkButton<Message>({
        href: '/settings/connections',
        label:
          input.account === undefined ? 'Connect ChatGPT' : 'Reconnect ChatGPT',
        size: 'sm',
        variant: 'primary',
      }),
    ],
  )
}

export const settingsWorkspacePage = <Message>(input: {
  section: SettingsWorkspaceSectionKey
  userName: string
  userEmail: string
  userId: string
  userAvatarUrl?: string
  githubLogin?: string
  githubId?: string
  teams: ReadonlyArray<SettingsWorkspaceTeam>
  members: ReadonlyArray<SettingsWorkspaceMember>
  providerAccounts: ReadonlyArray<SettingsProviderAccount>
  providerAttempts: ReadonlyArray<SettingsProviderConnectionAttempt>
  providerConnectionAction: SettingsProviderConnectionAction
  currentRepositoryDetail: string
  githubRepositoryPanel?: Html | undefined
  accountPoolPanel?: Html | undefined
  startProviderLoginAttrs: ReadonlyArray<Attribute<Message>>
  reconnectProviderLoginAttrs: ReadonlyArray<
    SettingsProviderAccountAction<Message>
  >
  pollProviderLoginAttrs: ReadonlyArray<Attribute<Message>>
  signOutAttrs: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const firstTeam = input.teams[0]
  const providerConnectionNotice =
    input.section === 'connections'
      ? null
      : settingsProviderConnectionNotice<Message>({
          account: input.providerAccounts[0],
        })
  const general =
    input.section === 'general'
      ? h.div(
          [h.Class('grid gap-4 xl:grid-cols-2')],
          [
            settingsWorkspacePanel<Message>({
              eyebrow: 'Your account',
              title: 'Profile',
              children: [
                h.div(
                  [h.Class('grid gap-4 sm:grid-cols-[auto_minmax(0,1fr)]')],
                  [
                    settingsWorkspaceProfileImage<Message>({
                      name: input.userName,
                      ...(input.userAvatarUrl === undefined
                        ? {}
                        : { avatarUrl: input.userAvatarUrl }),
                    }),
                    settingsWorkspaceRows<Message>([
                      { label: 'Name', value: input.userName },
                      { label: 'Email', value: input.userEmail },
                      { label: 'User ID', value: input.userId },
                      {
                        label: 'GitHub',
                        value: settingsWorkspaceFallback(
                          input.githubLogin,
                          'not connected',
                        ),
                      },
                    ]),
                  ],
                ),
              ],
            }),
            settingsWorkspacePanel<Message>({
              eyebrow: 'Your organization',
              title: firstTeam?.name ?? 'OpenAgents',
              children: [
                settingsWorkspaceRows<Message>([
                  { label: 'Teams', value: String(input.teams.length) },
                  {
                    label: 'Role',
                    value: settingsWorkspaceFallback(firstTeam?.role, 'member'),
                  },
                  {
                    label: 'Members',
                    value: String(firstTeam?.memberCount ?? 1),
                  },
                ]),
              ],
            }),
            settingsWorkspacePanel<Message>({
              eyebrow: 'Status',
              title: 'Connections',
              children: [
                settingsWorkspaceStatusLine<Message>({
                  title: 'GitHub identity',
                  detail:
                    input.githubLogin === undefined
                      ? 'No GitHub login is present in this session.'
                      : `${input.githubLogin} (${settingsWorkspaceFallback(input.githubId, input.userId)})`,
                  tone:
                    input.githubLogin === undefined ? 'warning' : 'positive',
                }),
                settingsWorkspaceStatusLine<Message>({
                  title: 'ChatGPT account',
                  detail: providerAccountStatus(input.providerAccounts[0]),
                  tone: providerAccountTone(input.providerAccounts[0]),
                }),
                settingsWorkspaceStatusLine<Message>({
                  title: 'Default repository',
                  detail: input.currentRepositoryDetail,
                  tone: 'accent',
                }),
              ],
            }),
            settingsWorkspacePanel<Message>({
              eyebrow: 'Session',
              title: 'Browser controls',
              children: [
                button<Message>({
                  label: 'Sign out',
                  variant: 'danger',
                  size: 'sm',
                  attrs: input.signOutAttrs,
                }),
              ],
            }),
          ],
        )
      : null

  return container<Message>(
    [
      pageHeader<Message>({
        eyebrow: 'Settings',
        title: settingsWorkspaceTitle(input.section),
      }),
      h.div(
        [h.Class('mt-4 grid gap-4')],
        [
          providerConnectionNotice,
          general ??
            (input.section === 'connections'
              ? h.div(
                  [h.Class('grid gap-4')],
                  [
                    h.div(
                      [h.Class('grid gap-4 xl:grid-cols-2')],
                      [
                        settingsProviderConnectionPanel<Message>({
                          accounts: input.providerAccounts,
                          attempts: input.providerAttempts,
                          action: input.providerConnectionAction,
                          startAttrs: input.startProviderLoginAttrs,
                          reconnectAttrs: input.reconnectProviderLoginAttrs,
                          pollAttrs: input.pollProviderLoginAttrs,
                        }),
                        settingsWorkspacePanel<Message>({
                          eyebrow: 'Connections',
                          title: 'GitHub',
                          children: [
                            settingsWorkspaceRows<Message>([
                              {
                                label: 'Account',
                                value:
                                  input.githubLogin === undefined
                                    ? 'not connected'
                                    : input.githubLogin,
                              },
                              {
                                label: 'GitHub ID',
                                value: settingsWorkspaceFallback(
                                  input.githubId,
                                ),
                              },
                            ]),
                            ...(input.githubRepositoryPanel === undefined
                              ? []
                              : [input.githubRepositoryPanel]),
                          ],
                        }),
                      ],
                    ),
                    input.accountPoolPanel === undefined
                      ? null
                      : settingsWorkspacePanel<Message>({
                          eyebrow: 'Connections',
                          title: 'Account pool',
                          children: [input.accountPoolPanel],
                        }),
                  ],
                )
              : input.section === 'organization'
                ? settingsWorkspacePanel<Message>({
                    eyebrow: 'Organization',
                    title: 'Teams',
                    children: [settingsWorkspaceTeamList<Message>(input.teams)],
                  })
                : input.section === 'members'
                  ? settingsWorkspacePanel<Message>({
                      eyebrow: 'Members',
                      title: 'Team members',
                      children: [
                        settingsWorkspaceMemberList<Message>(input.members),
                      ],
                    })
                  : null),
        ],
      ),
    ],
    [h.Class('py-4')],
  )
}

export const commerceCategoryPage = <Message>(input: {
  title: string
  body?: string
  filters: ReadonlyArray<CommerceFilterGroup>
  products: ReadonlyArray<CommerceProductItem>
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('page-examples/category-pages'),
      h.Class(surfaceActiveClass),
    ],
    [
      pageHeader<Message>({
        eyebrow: 'Catalog',
        title: input.title,
        ...(input.body === undefined ? {} : { body: input.body }),
      }),
      h.div(
        [h.Class('grid gap-4 p-4 lg:grid-cols-[260px_minmax(0,1fr)]')],
        [
          filterPanel<Message>(input.filters),
          productGrid<Message>({ products: input.products, columns: 3 }),
        ],
      ),
    ],
  )
}

export const commerceCheckoutPage = <Message>(input: {
  title: string
  fields: ReadonlyArray<DescriptionItem>
  lines: ReadonlyArray<CommerceSummaryLine>
  action?: Html
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('page-examples/checkout-pages'),
      h.Class(surfaceActiveClass),
    ],
    [
      pageHeader<Message>({ eyebrow: 'Checkout', title: input.title }),
      h.div(
        [h.Class('grid gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_360px]')],
        [
          checkoutForm<Message>({
            title: 'Mission details',
            fields: input.fields,
            ...(input.action === undefined ? {} : { action: input.action }),
          }),
          orderSummary<Message>({
            title: 'Readiness',
            lines: input.lines,
          }),
        ],
      ),
    ],
  )
}

export const commerceOrderDetailPage = <Message>(input: {
  title: string
  lines: ReadonlyArray<CommerceLineItem>
  summary: ReadonlyArray<CommerceSummaryLine>
}): Html => {
  const h = html<Message>()

  return h.section(
    [
      kitFamily<Message>('page-examples/order-detail-pages'),
      h.Class(surfaceActiveClass),
    ],
    [
      pageHeader<Message>({
        eyebrow: 'Order',
        title: input.title,
        body: 'Operational cart patterns adapted to Autopilot readiness.',
      }),
      h.div(
        [h.Class('grid gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_340px]')],
        [
          section<Message>(
            [
              headingBlock<Message>({
                eyebrow: 'Line items',
                title: 'Prerequisites',
                level: 3,
              }),
              h.div(
                [h.Class('mt-4')],
                [commerceLineList<Message>(input.lines)],
              ),
            ],
            [kitFamily<Message>('components/shopping-carts')],
          ),
          orderSummary<Message>({
            title: 'Total',
            lines: input.summary,
          }),
        ],
      ),
    ],
  )
}
