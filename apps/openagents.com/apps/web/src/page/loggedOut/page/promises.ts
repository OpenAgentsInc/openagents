import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  PublicProductPromise,
  PublicProductPromiseState,
  PublicProductPromises,
  PublicProductPromisesModel,
  PublicPromiseTransitionReceipt,
  PublicPromiseTransitions,
  PublicPromiseTransitionsModel,
} from '../model'

type Tone = 'good' | 'muted' | 'warn' | 'bad'

const pageClass = 'min-h-screen bg-black text-[#f1efe8]'
const sectionClass = 'mx-auto w-full max-w-7xl px-4 py-4 sm:px-5 lg:px-6'
const panelClass = 'min-w-0 border border-[#242424] bg-[#050505] p-3 text-left'
const panelTitleClass =
  'm-0 text-[0.72rem] font-semibold uppercase leading-none text-[#f1efe8]'
const panelMetaClass = 'm-0 text-[0.68rem] leading-4 text-white/45'
const rowClass =
  'grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-t border-[#1d1d1d] py-2'
const rowLabelClass =
  'min-w-0 text-[0.72rem] font-medium leading-4 text-[#f1efe8]'
const rowDetailClass = 'mt-1 text-[0.66rem] leading-4 text-white/42'
const valueClass = 'text-right text-[0.72rem] leading-4 tabular-nums'

const stateOrder: ReadonlyArray<PublicProductPromiseState> = [
  'green',
  'yellow',
  'red',
  'degraded',
  'planned',
  'withdrawn',
]

const stateTone = (state: PublicProductPromiseState): Tone =>
  state === 'green'
    ? 'good'
    : state === 'yellow' || state === 'degraded'
      ? 'warn'
      : state === 'red'
        ? 'bad'
        : 'muted'

const toneClass = (tone: Tone): string =>
  tone === 'good'
    ? 'text-[#9ad6b7]'
    : tone === 'warn'
      ? 'text-[#f3c27a]'
      : tone === 'bad'
        ? 'text-[#ee8f8f]'
        : 'text-white/52'

const stateLabel = (state: string): string => state.toUpperCase()

const loadedPromises = (
  model: PublicProductPromisesModel,
): PublicProductPromises | null =>
  model._tag === 'PublicProductPromisesLoaded' ? model.promises : null

const modelStatus = (model: PublicProductPromisesModel): string =>
  model._tag === 'PublicProductPromisesLoaded'
    ? 'Live'
    : model._tag === 'PublicProductPromisesFailed'
      ? 'Unavailable'
      : model._tag === 'PublicProductPromisesLoading'
        ? 'Loading'
        : 'Idle'

const modelMeta = (model: PublicProductPromisesModel): string =>
  model._tag === 'PublicProductPromisesFailed'
    ? model.error
    : 'Backed by /api/public/product-promises.'

const countByState = (
  promises: ReadonlyArray<PublicProductPromise>,
): Readonly<Record<PublicProductPromiseState, number>> =>
  promises.reduce(
    (counts, promise) => ({
      ...counts,
      [promise.state]: counts[promise.state] + 1,
    }),
    {
      degraded: 0,
      green: 0,
      planned: 0,
      red: 0,
      withdrawn: 0,
      yellow: 0,
    },
  )

const productAreas = (
  promises: ReadonlyArray<PublicProductPromise>,
): ReadonlyArray<readonly [string, number]> =>
  Object.entries(
    promises.reduce<Record<string, number>>((areas, promise) => {
      areas[promise.productArea] = (areas[promise.productArea] ?? 0) + 1
      return areas
    }, {}),
  ).sort(([, left], [, right]) => right - left)

const pill = (label: string, tone: Tone = 'muted'): Html => {
  const h = html<Message>()

  return h.span(
    [
      Ui.className<Message>(
        `inline-flex min-h-6 items-center border border-[#2a2a2a] px-2 py-1 text-[0.62rem] uppercase leading-none ${toneClass(tone)}`,
      ),
    ],
    [label],
  )
}

const panelHeader = (input: {
  meta?: string
  status?: string
  title: string
  tone?: Tone
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
        : pill(input.status, input.tone ?? 'muted'),
    ],
  )
}

const metricRow = (input: {
  detail: string
  label: string
  tone?: Tone
  value: string
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
            `${valueClass} ${toneClass(input.tone ?? 'muted')}`,
          ),
        ],
        [input.value],
      ),
    ],
  )
}

const navLink = (href: string, label: string): Html => {
  const h = html<Message>()

  return h.a(
    [
      h.Href(href),
      Ui.className<Message>(
        'text-[0.68rem] font-semibold uppercase leading-none text-white/45 hover:text-[#f1efe8]',
      ),
    ],
    [label],
  )
}

const hero = (model: PublicProductPromisesModel): Html => {
  const h = html<Message>()
  const promises = loadedPromises(model)

  return h.section(
    [Ui.className<Message>(`${sectionClass} pb-6 pt-5`)],
    [
      h.header(
        [
          Ui.className<Message>(
            'flex min-w-0 flex-wrap items-center justify-between gap-3 border-b border-[#242424] pb-3',
          ),
        ],
        [
          h.a(
            [
              h.Href('/'),
              Ui.className<Message>(
                'text-[0.72rem] font-semibold uppercase tracking-normal text-[#f1efe8]',
              ),
            ],
            ['OpenAgents'],
          ),
          h.nav(
            [Ui.className<Message>('flex flex-wrap items-center gap-4')],
            [
              navLink('/docs/product-promises', 'Docs'),
              navLink('/api/public/product-promises', 'JSON'),
              navLink('/forum/f/product-promises', 'Forum'),
            ],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid min-w-0 gap-4 pt-8 lg:grid-cols-[minmax(0,1.3fr)_minmax(300px,0.7fr)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('min-w-0')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-[0.7rem] font-semibold uppercase leading-none text-white/42',
                  ),
                ],
                ['Human-readable promise ledger'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 mt-4 max-w-3xl text-[clamp(2.25rem,5vw,4.75rem)] font-semibold leading-[0.96] tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Product promises'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 mt-5 max-w-2xl text-[0.92rem] leading-6 text-white/58',
                  ),
                ],
                [
                  'A visual map of what OpenAgents says it does, what is live, what is gated, and what should be reported when reality does not match the claim.',
                ],
              ),
            ],
          ),
          h.aside(
            [Ui.className<Message>(panelClass)],
            [
              panelHeader({
                meta: modelMeta(model),
                status: modelStatus(model),
                title: 'Registry Status',
                tone:
                  model._tag === 'PublicProductPromisesLoaded'
                    ? 'good'
                    : 'muted',
              }),
              metricRow({
                detail: 'Versioned public JSON.',
                label: 'Version',
                value: promises?.version ?? 'Loading',
              }),
              metricRow({
                detail: 'Current records in the live registry.',
                label: 'Promises',
                tone: promises === null ? 'muted' : 'good',
                value:
                  promises === null
                    ? 'Loading'
                    : String(promises.promises.length),
              }),
              metricRow({
                detail: 'Default path for loose reports and stale-copy notes.',
                label: 'Report path',
                value: 'Forum',
              }),
            ],
          ),
        ],
      ),
    ],
  )
}

const statePanel = (promises: PublicProductPromises | null): Html => {
  const h = html<Message>()
  const counts = promises === null ? null : countByState(promises.promises)

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'Green is live. Yellow is scoped. Red is blocked. Withdrawn is historical.',
        title: 'State Map',
      }),
      ...stateOrder.map(state =>
        metricRow({
          detail: promises?.states[state] ?? 'Waiting for live registry.',
          label: stateLabel(state),
          tone: stateTone(state),
          value: counts === null ? '-' : String(counts[state]),
        }),
      ),
    ],
  )
}

const areaPanel = (promises: PublicProductPromises | null): Html => {
  const areas =
    promises === null ? [] : productAreas(promises.promises).slice(0, 10)

  return html<Message>().section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'Grouped by current product area.',
        title: 'Product Areas',
      }),
      ...(areas.length === 0
        ? [
            metricRow({
              detail: 'Waiting for /api/public/product-promises.',
              label: 'Loading',
              value: '-',
            }),
          ]
        : areas.map(([area, count]) =>
            metricRow({
              detail: 'Promise records in this area.',
              label: area,
              value: String(count),
            }),
          )),
    ],
  )
}

const caveatPanel = (promises: PublicProductPromises | null): Html => {
  const h = html<Message>()
  const caveats = promises?.currentMonorepoStatus.caveats.slice(0, 6) ?? []

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'These caveats apply before interpreting any individual row.',
        status: promises?.currentMonorepoStatus.status ?? 'Loading',
        title: 'Current Caveats',
      }),
      ...(caveats.length === 0
        ? [
            h.p(
              [
                Ui.className<Message>(
                  `${panelMetaClass} border-t border-[#1d1d1d] pt-2`,
                ),
              ],
              ['Waiting for the live caveat list.'],
            ),
          ]
        : caveats.map(caveat =>
            h.p(
              [
                Ui.className<Message>(
                  'm-0 border-t border-[#1d1d1d] py-2 text-[0.7rem] leading-5 text-white/52',
                ),
              ],
              [caveat],
            ),
          )),
    ],
  )
}

const detailList = (title: string, items: ReadonlyArray<string>): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('border-t border-[#1d1d1d] pt-3')],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 text-[0.62rem] font-semibold uppercase leading-none text-white/34',
          ),
        ],
        [title],
      ),
      items.length === 0
        ? h.p(
            [
              Ui.className<Message>(
                'm-0 mt-2 text-[0.68rem] leading-5 text-white/42',
              ),
            ],
            ['None listed.'],
          )
        : h.ul(
            [Ui.className<Message>('m-0 mt-2 grid gap-1 p-0')],
            items.slice(0, 6).map(item =>
              h.li(
                [
                  Ui.className<Message>(
                    'list-none break-words text-[0.68rem] leading-5 text-white/50',
                  ),
                ],
                [item],
              ),
            ),
          ),
    ],
  )
}

const blockerSummaryPanel = (promises: PublicProductPromises | null): Html => {
  const h = html<Message>()
  const summary = promises?.verificationSummary
  const blockers = summary?.uniqueBlockers.slice(0, 8) ?? []

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'At-a-glance blocker and evidence inventory from the live JSON.',
        status:
          summary === undefined
            ? 'Loading'
            : `${summary.promisesWithBlockersCount} with blockers`,
        title: 'Blockers And Evidence',
        tone:
          summary === undefined
            ? 'muted'
            : summary.promisesWithBlockersCount > 0
              ? 'warn'
              : 'good',
      }),
      metricRow({
        detail: 'Yellow, red, degraded, or planned records.',
        label: 'Not green',
        tone:
          summary === undefined || summary.blockedPromiseCount === 0
            ? 'muted'
            : 'warn',
        value: summary === undefined ? '-' : String(summary.blockedPromiseCount),
      }),
      metricRow({
        detail: 'Unique blocker refs across the registry.',
        label: 'Unique blockers',
        tone:
          summary === undefined || summary.uniqueBlockerCount === 0
            ? 'muted'
            : 'warn',
        value: summary === undefined ? '-' : String(summary.uniqueBlockerCount),
      }),
      metricRow({
        detail: 'Evidence refs attached to promise records.',
        label: 'Evidence refs',
        tone: summary === undefined ? 'muted' : 'good',
        value: summary === undefined ? '-' : String(summary.evidenceRefCount),
      }),
      detailList('Top blocker refs', blockers),
    ],
  )
}

const promiseCard = (promise: PublicProductPromise): Html => {
  const h = html<Message>()
  const blockerPreview =
    promise.blockerRefs.length === 0
      ? 'No blockers listed for this scoped claim.'
      : promise.blockerRefs.slice(0, 2).join(', ')
  const evidencePreview =
    promise.evidenceRefs.length === 0
      ? 'No evidence refs listed.'
      : promise.evidenceRefs.slice(0, 2).join(', ')

  return h.article(
    [Ui.className<Message>(`${panelClass} flex min-h-[18rem] flex-col gap-3`)],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex min-w-0 items-start justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('min-w-0')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 truncate text-[0.62rem] font-semibold uppercase leading-none text-white/36',
                  ),
                ],
                [promise.productArea],
              ),
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 mt-2 break-words text-[0.86rem] font-semibold leading-5 text-[#f1efe8]',
                  ),
                ],
                [promise.promiseId],
              ),
            ],
          ),
          pill(stateLabel(promise.state), stateTone(promise.state)),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-[0.76rem] leading-5 text-white/58')],
        [promise.claim],
      ),
      h.div(
        [Ui.className<Message>('mt-auto border-t border-[#1d1d1d] pt-3')],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 text-[0.66rem] font-semibold uppercase leading-none text-white/34',
              ),
            ],
            ['Safe copy'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 mt-2 text-[0.7rem] leading-5 text-white/50',
              ),
            ],
            [promise.safeCopy],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid grid-cols-2 gap-2')],
        [
          metricChip('Evidence', promise.evidenceRefs.length),
          metricChip('Blockers', promise.blockerRefs.length),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.p(
            [
              Ui.className<Message>(
                `m-0 text-[0.68rem] leading-5 ${
                  promise.blockerRefs.length === 0
                    ? 'text-[#9ad6b7]/75'
                    : 'text-[#f3c27a]/80'
                }`,
              ),
            ],
            [`Blocked by: ${blockerPreview}`],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-[0.68rem] leading-5 text-white/45')],
            [`Evidence: ${evidencePreview}`],
          ),
        ],
      ),
      h.details(
        [Ui.className<Message>('mt-1 border-t border-[#1d1d1d] pt-3')],
        [
          h.summary(
            [
              Ui.className<Message>(
                'cursor-pointer text-[0.66rem] font-semibold uppercase leading-none text-white/50 hover:text-[#f1efe8]',
              ),
            ],
            ['Evidence, blockers, and gates'],
          ),
          h.div(
            [Ui.className<Message>('mt-3 grid gap-3')],
            [
              detailList('Blockers', promise.blockerRefs),
              detailList('Evidence', promise.evidenceRefs),
              detailList('Source refs', promise.sourceRefs),
              detailList('Audience', promise.audience),
              detailList('Verification', [promise.verification]),
              detailList('Authority boundary', [promise.authorityBoundary]),
              detailList('Unsafe copy', [promise.unsafeCopy]),
            ],
          ),
        ],
      ),
    ],
  )
}

const metricChip = (label: string, count: number): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid grid-cols-[minmax(0,1fr)_auto] gap-2 border border-[#1d1d1d] px-2 py-2',
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            'truncate text-[0.62rem] font-semibold uppercase leading-none text-white/34',
          ),
        ],
        [label],
      ),
      h.span(
        [Ui.className<Message>('text-[0.66rem] leading-none text-white/55')],
        [String(count)],
      ),
    ],
  )
}

const promiseGrid = (promises: PublicProductPromises | null): Html => {
  const h = html<Message>()
  const rows = promises?.promises ?? []

  return h.section(
    [Ui.className<Message>(`${sectionClass} pb-10`)],
    [
      h.div(
        [
          Ui.className<Message>(
            'mb-3 flex min-w-0 flex-wrap items-end justify-between gap-3 border-t border-[#242424] pt-4',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('min-w-0')],
            [
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 text-[1.2rem] font-semibold leading-6 text-[#f1efe8]',
                  ),
                ],
                ['Promise records'],
              ),
              h.p(
                [Ui.className<Message>(`${panelMetaClass} mt-1`)],
                ['Cards render from the live public endpoint.'],
              ),
            ],
          ),
          promises === null ? pill('Loading') : pill(`${rows.length} records`),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid min-w-0 gap-3 sm:grid-cols-2 xl:grid-cols-3',
          ),
        ],
        rows.length === 0
          ? [
              h.section(
                [Ui.className<Message>(panelClass)],
                [
                  panelHeader({
                    meta: 'The browser is waiting for /api/public/product-promises.',
                    status: 'Loading',
                    title: 'Promise Records',
                  }),
                ],
              ),
            ]
          : rows.map(promiseCard),
      ),
    ],
  )
}

// ---------------------------------------------------------------------------
// Claim-upgrade audit panel (proof.claim_upgrade_receipts.v1)
//
// Read-only enterprise audit surface over the promise-transition receipts at
// /api/public/product-promises/transitions. Every promise green-flip in the
// registry must be backed by a dereferenceable, registry-versioned transition
// receipt. This panel renders, per receipt: which promise flipped, the
// before -> after state, the mechanical checks the receipt passed, the owner
// signoff (policy-exception approver) when present, and the evidence refs, so
// any auditor can confirm each state change is receipt-backed.
// ---------------------------------------------------------------------------

const loadedTransitions = (
  model: PublicPromiseTransitionsModel,
): PublicPromiseTransitions | null =>
  model._tag === 'PublicPromiseTransitionsLoaded' ? model.transitions : null

const transitionsStatus = (model: PublicPromiseTransitionsModel): string =>
  model._tag === 'PublicPromiseTransitionsLoaded'
    ? 'Live'
    : model._tag === 'PublicPromiseTransitionsFailed'
      ? 'Unavailable'
      : model._tag === 'PublicPromiseTransitionsLoading'
        ? 'Loading'
        : 'Idle'

const transitionsMeta = (model: PublicPromiseTransitionsModel): string =>
  model._tag === 'PublicPromiseTransitionsFailed'
    ? model.error
    : 'Backed by /api/public/product-promises/transitions.'

// Receipt states arrive as plain strings from the transition feed. stateTone
// only does value comparisons, so map through a string-typed wrapper rather
// than asserting the value into the registry state union.
const stateToneFor = (state: string): Tone =>
  state === 'green'
    ? 'good'
    : state === 'yellow' || state === 'degraded'
      ? 'warn'
      : state === 'red'
        ? 'bad'
        : 'muted'

const receiptResultTone = (result: string): Tone =>
  result === 'passed'
    ? 'good'
    : result === 'exception'
      ? 'warn'
      : 'bad'

const checkTone = (result: string): Tone =>
  result === 'passed' ? 'good' : 'bad'

// Owner signoff: a passing receipt is mechanical; an exception receipt is the
// owner-authorized policy override and carries the approver ref. Green flips of
// already-applied state are recorded as exception receipts (from-state equals
// to-state), so the approver ref IS the owner signoff for those flips.
const ownerSignoff = (receipt: PublicPromiseTransitionReceipt): string =>
  receipt.exception === null
    ? receipt.result === 'passed'
      ? 'Mechanical pass (no policy exception).'
      : 'No owner signoff on record.'
    : `Approved by ${receipt.exception.approvedByRef} (expires ${receipt.exception.expiresAt}).`

const isGreenFlip = (receipt: PublicPromiseTransitionReceipt): boolean =>
  receipt.toState === 'green'

const receiptCard = (
  receipt: PublicPromiseTransitionReceipt,
  promisesById: ReadonlyMap<string, PublicProductPromise>,
): Html => {
  const h = html<Message>()
  const promise = promisesById.get(receipt.promiseId)
  const productArea = promise?.productArea ?? 'unknown area'
  const greenFlip = isGreenFlip(receipt)
  // before/after: render the recorded transition direction. An exception
  // receipt for an already-applied flip has fromState === toState and is
  // labeled as such so auditors are not misled.
  const beforeAfter =
    receipt.fromState === receipt.toState
      ? `${stateLabel(receipt.fromState)} (flip already applied; exception receipt)`
      : `${stateLabel(receipt.fromState)} -> ${stateLabel(receipt.toState)}`

  return h.article(
    [
      Ui.className<Message>(
        `${panelClass} flex min-w-0 flex-col gap-3 ${
          greenFlip ? 'border-[#2f4a3c]' : ''
        }`,
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex min-w-0 items-start justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('min-w-0')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 truncate text-[0.62rem] font-semibold uppercase leading-none text-white/36',
                  ),
                ],
                [productArea],
              ),
              h.h3(
                [
                  Ui.className<Message>(
                    'm-0 mt-2 break-words text-[0.82rem] font-semibold leading-5 text-[#f1efe8]',
                  ),
                ],
                [receipt.promiseId],
              ),
            ],
          ),
          pill(receipt.result.toUpperCase(), receiptResultTone(receipt.result)),
        ],
      ),
      metricRow({
        detail: greenFlip
          ? 'Green flip: requires a receipt-backed transition.'
          : 'Recorded state transition.',
        label: 'Before / after',
        tone: stateToneFor(receipt.toState),
        value: beforeAfter,
      }),
      metricRow({
        detail: 'Dereferenceable, registry-versioned receipt id.',
        label: 'Receipt',
        value: receipt.receiptId,
      }),
      metricRow({
        detail: 'Registry version the receipt was checked against.',
        label: 'Registry version',
        value: receipt.registryVersion,
      }),
      metricRow({
        detail: 'Owner / policy signoff binding this transition.',
        label: 'Owner signoff',
        tone: receipt.exception === null ? 'muted' : 'warn',
        value: receipt.exception === null ? 'Mechanical' : 'Exception',
      }),
      h.p(
        [Ui.className<Message>('m-0 text-[0.7rem] leading-5 text-white/55')],
        [ownerSignoff(receipt)],
      ),
      h.div(
        [Ui.className<Message>('border-t border-[#1d1d1d] pt-3')],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 text-[0.62rem] font-semibold uppercase leading-none text-white/34',
              ),
            ],
            ['Checks'],
          ),
          receipt.checks.length === 0
            ? h.p(
                [
                  Ui.className<Message>(
                    'm-0 mt-2 text-[0.68rem] leading-5 text-white/42',
                  ),
                ],
                ['No checks recorded.'],
              )
            : h.ul(
                [Ui.className<Message>('m-0 mt-2 grid gap-1 p-0')],
                receipt.checks.map(check =>
                  h.li(
                    [
                      Ui.className<Message>(
                        `list-none text-[0.68rem] leading-5 ${toneClass(
                          checkTone(check.result),
                        )}`,
                      ),
                    ],
                    [`${check.kind}: ${check.result}`],
                  ),
                ),
              ),
        ],
      ),
      h.details(
        [Ui.className<Message>('border-t border-[#1d1d1d] pt-3')],
        [
          h.summary(
            [
              Ui.className<Message>(
                'cursor-pointer text-[0.66rem] font-semibold uppercase leading-none text-white/50 hover:text-[#f1efe8]',
              ),
            ],
            ['Evidence and signoff detail'],
          ),
          h.div(
            [Ui.className<Message>('mt-3 grid gap-3')],
            [
              detailList('Evidence refs', receipt.evidenceRefs),
              detailList('Checked at', [receipt.checkedAt]),
              detailList(
                'Exception',
                receipt.exception === null
                  ? []
                  : [
                      `Approved by: ${receipt.exception.approvedByRef}`,
                      `Reason: ${receipt.exception.reasonRef}`,
                      `Expires: ${receipt.exception.expiresAt}`,
                    ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const auditSummaryPanel = (
  transitions: PublicPromiseTransitions | null,
): Html => {
  const h = html<Message>()
  const receipts = transitions?.receipts ?? []
  const greenFlips = receipts.filter(isGreenFlip)
  const greenWithSignoff = greenFlips.filter(
    receipt => receipt.exception !== null || receipt.result === 'passed',
  )
  const exceptions = receipts.filter(receipt => receipt.result === 'exception')

  return h.section(
    [Ui.className<Message>(panelClass)],
    [
      panelHeader({
        meta: 'Receipt-backing tally over the live transition receipt feed.',
        status:
          transitions === null ? 'Loading' : `${receipts.length} receipts`,
        title: 'Audit Summary',
        tone: transitions === null ? 'muted' : 'good',
      }),
      metricRow({
        detail: 'Every state change is recorded as one receipt.',
        label: 'Transition receipts',
        tone: transitions === null ? 'muted' : 'good',
        value: transitions === null ? '-' : String(receipts.length),
      }),
      metricRow({
        detail: 'Receipts whose target state is green.',
        label: 'Green flips',
        tone: greenFlips.length === 0 ? 'muted' : 'good',
        value: transitions === null ? '-' : String(greenFlips.length),
      }),
      metricRow({
        detail: 'Green flips backed by a passing or owner-signed receipt.',
        label: 'Green flips receipt-backed',
        tone:
          transitions === null
            ? 'muted'
            : greenFlips.length === greenWithSignoff.length
              ? 'good'
              : 'bad',
        value:
          transitions === null
            ? '-'
            : `${greenWithSignoff.length}/${greenFlips.length}`,
      }),
      metricRow({
        detail: 'Owner-authorized policy-exception receipts.',
        label: 'Owner-signed exceptions',
        tone: exceptions.length === 0 ? 'muted' : 'warn',
        value: transitions === null ? '-' : String(exceptions.length),
      }),
    ],
  )
}

const auditPanel = (
  transitionsModel: PublicPromiseTransitionsModel,
  promises: PublicProductPromises | null,
): Html => {
  const h = html<Message>()
  const transitions = loadedTransitions(transitionsModel)
  const receipts = transitions?.receipts ?? []
  const promisesById = new Map<string, PublicProductPromise>(
    (promises?.promises ?? []).map(promise => [promise.promiseId, promise]),
  )
  const failed = transitionsModel._tag === 'PublicPromiseTransitionsFailed'

  return h.section(
    [Ui.className<Message>(`${sectionClass} pb-10`)],
    [
      h.div(
        [
          Ui.className<Message>(
            'mb-3 flex min-w-0 flex-wrap items-end justify-between gap-3 border-t border-[#242424] pt-4',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('min-w-0')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-[0.7rem] font-semibold uppercase leading-none text-white/42',
                  ),
                ],
                ['proof.claim_upgrade_receipts.v1'],
              ),
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 mt-2 text-[1.2rem] font-semibold leading-6 text-[#f1efe8]',
                  ),
                ],
                ['Claim-upgrade audit panel'],
              ),
              h.p(
                [Ui.className<Message>(`${panelMetaClass} mt-1 max-w-3xl`)],
                [
                  'Every promise state change traces to a dereferenceable, registry-versioned transition receipt: the before/after state, the mechanical checks it passed, and the owner signoff. A passing receipt is evidence for a flip, never the flip itself.',
                ],
              ),
            ],
          ),
          pill(
            transitionsStatus(transitionsModel),
            transitions === null ? 'muted' : 'good',
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'mb-3 grid min-w-0 gap-3 lg:grid-cols-[minmax(0,1fr)]',
          ),
        ],
        [auditSummaryPanel(transitions)],
      ),
      h.div(
        [Ui.className<Message>('mb-3 flex flex-wrap items-center gap-4')],
        [
          navLink(
            '/api/public/product-promises/transitions',
            'Receipt feed JSON',
          ),
          navLink(
            '/api/public/product-promises/audit',
            'Enterprise audit projection JSON',
          ),
        ],
      ),
      failed
        ? h.section(
            [Ui.className<Message>(panelClass)],
            [
              panelHeader({
                meta: transitionsMeta(transitionsModel),
                status: 'Unavailable',
                title: 'Transition Receipts',
              }),
            ],
          )
        : h.div(
            [
              Ui.className<Message>(
                'grid min-w-0 gap-3 sm:grid-cols-2 xl:grid-cols-3',
              ),
            ],
            receipts.length === 0
              ? [
                  h.section(
                    [Ui.className<Message>(panelClass)],
                    [
                      panelHeader({
                        meta:
                          transitions === null
                            ? 'Waiting for /api/public/product-promises/transitions.'
                            : 'No transition receipts have been recorded yet.',
                        status:
                          transitions === null ? 'Loading' : 'Empty',
                        title: 'Transition Receipts',
                      }),
                    ],
                  ),
                ]
              : receipts.map(receipt => receiptCard(receipt, promisesById)),
          ),
      transitions === null
        ? null
        : h.p(
            [
              Ui.className<Message>(
                'mt-4 border-t border-[#1d1d1d] pt-3 text-[0.66rem] leading-5 text-white/42',
              ),
            ],
            [transitions.rule],
          ),
    ],
  )
}

export const view = (
  model: PublicProductPromisesModel,
  transitionsModel: PublicPromiseTransitionsModel,
): Html => {
  const promises = loadedPromises(model)

  return html<Message>().div(
    [Ui.className<Message>(pageClass)],
    [
      hero(model),
      html<Message>().section(
        [
          Ui.className<Message>(
            `${sectionClass} grid min-w-0 gap-3 lg:grid-cols-3`,
          ),
        ],
        [statePanel(promises), areaPanel(promises), caveatPanel(promises)],
      ),
      html<Message>().section(
        [
          Ui.className<Message>(
            `${sectionClass} grid min-w-0 gap-3 lg:grid-cols-[minmax(0,1fr)]`,
          ),
        ],
        [blockerSummaryPanel(promises)],
      ),
      auditPanel(transitionsModel, promises),
      promiseGrid(promises),
    ],
  )
}
