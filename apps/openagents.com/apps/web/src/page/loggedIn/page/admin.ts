import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { userFacingCopy } from '../../../display-copy'
import * as Ui from '../../../ui'
import {
  Message,
  RequestedAdminSiteDeploymentAction,
  RequestedDeployAdminSiteVersion,
  RequestedGenerateAdminSite,
  RequestedLoadAdminAdjutantReview,
  RequestedLoadAdminOverview,
  RequestedReviewAdminAdjutantResearchBrief,
  RequestedReviewAdminAdjutantSourceCard,
  RequestedRunAdminAdjutantEnrichment,
} from '../message'
import type {
  AdjutantUsageReceiptBillingMode,
  AdjutantUsageReceiptCategory,
  AdjutantUsageReceiptSummary,
  AdminAdjutantAssignment,
  AdminAdjutantAssignmentsState,
  AdminAdjutantEnrichmentActionState,
  AdminAdjutantReviewDeployment,
  AdminAdjutantReviewEnrichment,
  AdminAdjutantReviewEvent,
  AdminAdjutantReviewState,
  AdminAdjutantReviewUsageReceipt,
  AdminAdjutantReviewVersion,
  AdminOverviewSoftwareOrder,
  AdminOverviewUser,
  AdminSiteDeploymentActionState,
  Model,
} from '../model'

type SiteSoftwareOrder = AdminOverviewSoftwareOrder & {
  siteProjectId: string
}

type SiteAssignment = AdminAdjutantAssignment

type MarkdownBlock =
  | Readonly<{
      _tag: 'Blockquote'
      text: string
    }>
  | Readonly<{
      _tag: 'Code'
      text: string
    }>
  | Readonly<{
      _tag: 'Heading'
      depth: number
      text: string
    }>
  | Readonly<{
      _tag: 'HorizontalRule'
    }>
  | Readonly<{
      _tag: 'List'
      items: ReadonlyArray<string>
      ordered: boolean
    }>
  | Readonly<{
      _tag: 'Paragraph'
      text: string
    }>
  | Readonly<{
      _tag: 'Table'
      headers: ReadonlyArray<string>
      rows: ReadonlyArray<ReadonlyArray<string>>
    }>

const dateLabel = (value: string | null): string =>
  value === null ? '-' : value.replace('T', ' ').replace('.000Z', 'Z')

const nullableLabel = (value: string | null): string =>
  value === null || value.trim() === '' ? '-' : value

const compactText = (value: string, limit = 96): string =>
  value.length <= limit ? value : `${value.slice(0, limit - 1).trimEnd()}...`

const countLabel = (count: number, singular: string): string =>
  `${count} ${count === 1 ? singular : `${singular}s`}`

const usageCategoryLabel = (category: AdjutantUsageReceiptCategory): string =>
  category.replaceAll('_', ' ')

const usageBillingModeLabel = (mode: AdjutantUsageReceiptBillingMode): string =>
  M.value(mode).pipe(
    M.when('public_beta_free', () => 'Public beta free'),
    M.when('paid_credits', () => 'Paid credits'),
    M.exhaustive,
  )

const usageQuantityLabel = (quantity: number, unit: string | null): string =>
  unit === null ? String(quantity) : `${quantity} ${unit}`

const isFenceLine = (line: string): boolean => line.trim().startsWith('```')

const isHeadingLine = (line: string): boolean => /^#{1,6}\s+/.test(line.trim())

const isListLine = (line: string): boolean =>
  /^\s*[-*]\s+/.test(line) || /^\s*\d+\.\s+/.test(line)

const isBlockquoteLine = (line: string): boolean => line.trim().startsWith('>')

const isHorizontalRuleLine = (line: string): boolean =>
  /^(-{3,}|\*{3,}|_{3,})$/.test(line.trim())

const isTableDelimiterLine = (line: string): boolean =>
  /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(line)

const isTableRowLine = (line: string): boolean =>
  line.includes('|') && line.trim() !== ''

const tableCells = (line: string): ReadonlyArray<string> =>
  line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map(cell => cell.trim())

const collectFence = (
  lines: ReadonlyArray<string>,
  index: number,
  collected: ReadonlyArray<string> = [],
): Readonly<{ nextIndex: number; text: string }> => {
  const line = lines[index]

  if (line === undefined) {
    return { nextIndex: index, text: collected.join('\n') }
  }

  if (isFenceLine(line)) {
    return { nextIndex: index + 1, text: collected.join('\n') }
  }

  return collectFence(lines, index + 1, [...collected, line])
}

const collectWhile = (
  lines: ReadonlyArray<string>,
  index: number,
  predicate: (line: string) => boolean,
  collected: ReadonlyArray<string> = [],
): Readonly<{ nextIndex: number; lines: ReadonlyArray<string> }> => {
  const line = lines[index]

  if (line === undefined || !predicate(line)) {
    return { nextIndex: index, lines: collected }
  }

  return collectWhile(lines, index + 1, predicate, [...collected, line])
}

const isParagraphBoundary = (line: string): boolean =>
  line.trim() === '' ||
  isFenceLine(line) ||
  isHeadingLine(line) ||
  isListLine(line) ||
  isBlockquoteLine(line) ||
  isHorizontalRuleLine(line) ||
  isTableRowLine(line)

const parseMarkdownLines = (
  lines: ReadonlyArray<string>,
  index = 0,
  blocks: ReadonlyArray<MarkdownBlock> = [],
): ReadonlyArray<MarkdownBlock> => {
  const line = lines[index]

  if (line === undefined) {
    return blocks
  }

  if (line.trim() === '') {
    return parseMarkdownLines(lines, index + 1, blocks)
  }

  if (isFenceLine(line)) {
    const code = collectFence(lines, index + 1)

    return parseMarkdownLines(lines, code.nextIndex, [
      ...blocks,
      { _tag: 'Code', text: code.text },
    ])
  }

  if (isHeadingLine(line)) {
    const match = /^(#{1,6})\s+(.+)$/.exec(line.trim())

    return parseMarkdownLines(lines, index + 1, [
      ...blocks,
      {
        _tag: 'Heading',
        depth: match?.[1]?.length ?? 2,
        text: match?.[2] ?? line.trim(),
      },
    ])
  }

  if (isHorizontalRuleLine(line)) {
    return parseMarkdownLines(lines, index + 1, [
      ...blocks,
      { _tag: 'HorizontalRule' },
    ])
  }

  if (
    isTableRowLine(line) &&
    lines[index + 1] !== undefined &&
    isTableDelimiterLine(lines[index + 1]!)
  ) {
    const rows = collectWhile(
      lines,
      index + 2,
      candidate => candidate.trim() !== '' && isTableRowLine(candidate),
    )

    return parseMarkdownLines(lines, rows.nextIndex, [
      ...blocks,
      {
        _tag: 'Table',
        headers: tableCells(line),
        rows: rows.lines.map(tableCells),
      },
    ])
  }

  if (isListLine(line)) {
    const ordered = /^\s*\d+\.\s+/.test(line)
    const items = collectWhile(lines, index, candidate =>
      ordered ? /^\s*\d+\.\s+/.test(candidate) : /^\s*[-*]\s+/.test(candidate),
    )

    return parseMarkdownLines(lines, items.nextIndex, [
      ...blocks,
      {
        _tag: 'List',
        items: items.lines.map(item =>
          item.replace(/^\s*(?:[-*]|\d+\.)\s+/, '').trim(),
        ),
        ordered,
      },
    ])
  }

  if (isBlockquoteLine(line)) {
    const quote = collectWhile(lines, index, isBlockquoteLine)

    return parseMarkdownLines(lines, quote.nextIndex, [
      ...blocks,
      {
        _tag: 'Blockquote',
        text: quote.lines
          .map(item => item.replace(/^\s*>\s?/, '').trim())
          .join('\n'),
      },
    ])
  }

  const paragraph = collectWhile(
    lines,
    index,
    candidate => !isParagraphBoundary(candidate),
  )

  return parseMarkdownLines(lines, paragraph.nextIndex, [
    ...blocks,
    {
      _tag: 'Paragraph',
      text: paragraph.lines.join(' ').trim(),
    },
  ])
}

const markdownBlocks = (text: string): ReadonlyArray<MarkdownBlock> =>
  parseMarkdownLines(text.replace(/\r\n/g, '\n').split('\n'))

const inlineMarkdown = (text: string): ReadonlyArray<Html | string> => {
  const h = html<Message>()
  const codeMatch = /`([^`]+)`/.exec(text)
  const boldMatch = /\*\*([^*]+)\*\*/.exec(text)
  const linkMatch = /\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/.exec(text)
  const matches = [codeMatch, boldMatch, linkMatch].filter(
    (match): match is RegExpExecArray => match !== null,
  )
  const first = matches.reduce<RegExpExecArray | undefined>(
    (current, match) =>
      current === undefined || match.index < current.index ? match : current,
    undefined,
  )

  if (first === undefined) {
    return text === '' ? [] : [text]
  }

  const before = text.slice(0, first.index)
  const matched = first[0]
  const after = text.slice(first.index + matched.length)
  const node =
    first === codeMatch
      ? h.code(
          [
            Ui.className<Message>(
              'border border-[#333] bg-[#080808] px-1 text-white/80',
            ),
          ],
          [first[1] ?? ''],
        )
      : first === boldMatch
        ? h.strong(
            [Ui.className<Message>('font-semibold text-white/85')],
            [first[1] ?? ''],
          )
        : h.a(
            [
              h.Href(first[2] ?? '#'),
              h.Target('_blank'),
              h.Rel('noreferrer'),
              Ui.className<Message>(
                'text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
              ),
            ],
            [first[1] ?? first[2] ?? 'Link'],
          )

  return [...inlineMarkdown(before), node, ...inlineMarkdown(after)]
}

const markdownHeading = (depth: number, text: string): Html => {
  const h = html<Message>()
  const attrs = [
    Ui.className<Message>(
      depth <= 2
        ? 'm-0 pt-2 text-lg font-semibold text-white/90'
        : 'm-0 pt-2 text-base font-semibold text-white/85',
    ),
  ]

  return depth <= 2
    ? h.h3(attrs, inlineMarkdown(text))
    : h.h4(attrs, inlineMarkdown(text))
}

const markdownBlockView = (block: MarkdownBlock): Html => {
  const h = html<Message>()

  return M.value(block).pipe(
    M.tagsExhaustive({
      Blockquote: ({ text }) =>
        h.blockquote(
          [
            Ui.className<Message>(
              'm-0 border-l border-[#ffb400]/50 pl-4 text-base/7 text-white/60',
            ),
          ],
          [text],
        ),
      Code: ({ text }) =>
        h.pre(
          [
            Ui.className<Message>(
              'm-0 overflow-x-auto border border-[#222] bg-[#050505] p-3 text-xs/6 text-white/70',
            ),
          ],
          [h.code([], [text])],
        ),
      Heading: ({ depth, text }) => markdownHeading(depth, text),
      HorizontalRule: () => h.hr([Ui.className<Message>('border-[#222]')]),
      List: ({ items, ordered }) =>
        ordered
          ? h.ol(
              [
                Ui.className<Message>(
                  'm-0 grid list-decimal gap-1 pl-6 text-white/65',
                ),
              ],
              items.map(item => h.li([], inlineMarkdown(item))),
            )
          : h.ul(
              [
                Ui.className<Message>(
                  'm-0 grid list-disc gap-1 pl-6 text-white/65',
                ),
              ],
              items.map(item => h.li([], inlineMarkdown(item))),
            ),
      Paragraph: ({ text }) =>
        h.p(
          [Ui.className<Message>('m-0 text-base/7 text-white/65 sm:text-sm/7')],
          inlineMarkdown(text),
        ),
      Table: ({ headers, rows }) =>
        h.div(
          [Ui.className<Message>('overflow-x-auto whitespace-nowrap')],
          [
            h.table(
              [
                Ui.className<Message>(
                  'w-full border-collapse text-left text-xs/6',
                ),
              ],
              [
                h.thead(
                  [
                    Ui.className<Message>(
                      'border-b border-[#333] text-white/45',
                    ),
                  ],
                  [
                    h.tr(
                      [],
                      headers.map(header =>
                        h.th(
                          [
                            h.Scope('col'),
                            Ui.className<Message>(
                              'whitespace-nowrap py-2 pr-4 font-medium',
                            ),
                          ],
                          inlineMarkdown(header),
                        ),
                      ),
                    ),
                  ],
                ),
                h.tbody(
                  [Ui.className<Message>('divide-y divide-[#222]')],
                  rows.map(row =>
                    h.tr(
                      [],
                      row.map(cell =>
                        h.td(
                          [
                            Ui.className<Message>(
                              'py-2 pr-4 align-top text-white/60',
                            ),
                          ],
                          inlineMarkdown(cell),
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ],
        ),
    }),
  )
}

const markdownView = (text: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid max-w-[80ch] gap-3 whitespace-normal')],
    markdownBlocks(text).map(markdownBlockView),
  )
}

const hasSiteProject = (
  order: AdminOverviewSoftwareOrder,
): order is SiteSoftwareOrder => order.siteProjectId !== null

const assignmentsFromState = (
  state: AdminAdjutantAssignmentsState,
): ReadonlyArray<SiteAssignment> =>
  M.value(state).pipe(
    M.tagsExhaustive({
      AdminAdjutantAssignmentsFailed: () => [],
      AdminAdjutantAssignmentsIdle: () => [],
      AdminAdjutantAssignmentsLoaded: ({ assignments }) => assignments,
      AdminAdjutantAssignmentsLoading: () => [],
    }),
  )

const assignmentForSiteOrder = (
  order: SiteSoftwareOrder,
  assignments: ReadonlyArray<SiteAssignment>,
): SiteAssignment | undefined =>
  assignments.find(
    assignment =>
      assignment.siteId === order.siteProjectId ||
      assignment.softwareOrderId === order.id,
  )

const statusBadge = (status: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      Ui.className<Message>(
        'inline-flex items-center border border-[#333] px-2 py-1 text-xs uppercase text-white/60',
      ),
    ],
    [status.replaceAll('_', ' ')],
  )
}

const repoCell = (repositoryFullName: string | null): Html | string => {
  const h = html<Message>()

  if (repositoryFullName === null || repositoryFullName.trim() === '') {
    return '-'
  }

  return h.a(
    [
      h.Href(`https://github.com/${repositoryFullName}`),
      h.Target('_blank'),
      h.Rel('noreferrer'),
      Ui.className<Message>(
        'text-white/75 underline underline-offset-[3px] hover:text-[#ffb400]',
      ),
    ],
    [repositoryFullName],
  )
}

const tableHeaderCell = (label: string): Html => {
  const h = html<Message>()

  return h.th(
    [
      h.Scope('col'),
      Ui.className<Message>(
        'whitespace-nowrap py-2 pr-6 font-medium text-white/45',
      ),
    ],
    [label],
  )
}

const tableCell = (children: ReadonlyArray<Html | string>): Html => {
  const h = html<Message>()

  return h.td(
    [Ui.className<Message>('max-w-[28rem] py-3 pr-6 align-top text-white/65')],
    children,
  )
}

const compactTableCell = (children: ReadonlyArray<Html | string>): Html => {
  const h = html<Message>()

  return h.td(
    [Ui.className<Message>('py-2 pr-5 align-top text-white/65')],
    children,
  )
}

const isPublicReviewSite = (
  site: Readonly<{ accessMode: string; visibility: string }> | null,
): boolean =>
  site !== null &&
  (site.accessMode === 'public' || site.visibility === 'public')

const savedReviewVersion = (
  versions: ReadonlyArray<AdminAdjutantReviewVersion>,
): AdminAdjutantReviewVersion | undefined =>
  versions.find(version => version.buildStatus === 'saved')

const activeReviewDeployment = (
  deployments: ReadonlyArray<AdminAdjutantReviewDeployment>,
): AdminAdjutantReviewDeployment | undefined =>
  deployments.find(deployment => deployment.status === 'active')

const rollbackReviewDeployment = (
  deployments: ReadonlyArray<AdminAdjutantReviewDeployment>,
): AdminAdjutantReviewDeployment | undefined =>
  deployments.find(deployment => deployment.status === 'rolled_back')

const reviewActionNotice = (
  action: AdminSiteDeploymentActionState,
): Html | null => {
  const h = html<Message>()

  return M.value(action).pipe(
    M.tagsExhaustive({
      AdminSiteDeploymentActionIdle: () => null,
      AdminSiteDeploymentActionPending: ({ action }) =>
        h.p(
          [Ui.className<Message>('m-0 text-xs text-white/45')],
          [`Running ${action}...`],
        ),
      AdminSiteDeploymentActionSucceeded: ({ message }) =>
        h.p(
          [Ui.className<Message>('m-0 text-xs text-[#7ccf8a]')],
          [userFacingCopy(message)],
        ),
      AdminSiteDeploymentActionFailed: ({ error }) =>
        h.p([Ui.className<Message>('m-0 text-xs text-[#d32f2f]')], [error]),
    }),
  )
}

const enrichmentActionNotice = (
  action: AdminAdjutantEnrichmentActionState,
): Html | null => {
  const h = html<Message>()

  return M.value(action).pipe(
    M.tagsExhaustive({
      AdminAdjutantEnrichmentActionIdle: () => null,
      AdminAdjutantEnrichmentActionPending: ({ action }) =>
        h.p(
          [Ui.className<Message>('m-0 text-xs text-white/45')],
          [`Running ${action}...`],
        ),
      AdminAdjutantEnrichmentActionSucceeded: ({ message }) =>
        h.p(
          [Ui.className<Message>('m-0 text-xs text-[#7ccf8a]')],
          [userFacingCopy(message)],
        ),
      AdminAdjutantEnrichmentActionFailed: ({ error }) =>
        h.p([Ui.className<Message>('m-0 text-xs text-[#d32f2f]')], [error]),
    }),
  )
}

const reviewVersionsTable = (
  versions: ReadonlyArray<AdminAdjutantReviewVersion>,
): Html => {
  const h = html<Message>()

  if (versions.length === 0) {
    return h.p(
      [Ui.className<Message>('m-0 text-xs text-white/40')],
      ['No versions yet.'],
    )
  }

  return h.div(
    [
      Ui.className<Message>(
        '-mx-2 -my-2 overflow-x-auto whitespace-nowrap px-2 py-2',
      ),
    ],
    [
      h.table(
        [Ui.className<Message>('w-full border-collapse text-left text-xs/6')],
        [
          h.thead(
            [Ui.className<Message>('border-b border-[#222] text-white/45')],
            [
              h.tr(
                [],
                [
                  tableHeaderCell('Version'),
                  tableHeaderCell('Build'),
                  tableHeaderCell('Source'),
                  tableHeaderCell('Run'),
                  tableHeaderCell('Saved'),
                ],
              ),
            ],
          ),
          h.tbody(
            [Ui.className<Message>('divide-y divide-[#222]')],
            versions.map(version =>
              h.tr(
                [],
                [
                  compactTableCell([version.id]),
                  compactTableCell([statusBadge(version.buildStatus)]),
                  compactTableCell([
                    version.sourceKind,
                    h.div(
                      [Ui.className<Message>('text-white/35')],
                      [nullableLabel(version.sourceCommitSha)],
                    ),
                  ]),
                  compactTableCell([nullableLabel(version.createdByRunId)]),
                  compactTableCell([dateLabel(version.savedAt)]),
                ],
              ),
            ),
          ),
        ],
      ),
    ],
  )
}

const reviewDeploymentsTable = (
  deployments: ReadonlyArray<AdminAdjutantReviewDeployment>,
): Html => {
  const h = html<Message>()

  if (deployments.length === 0) {
    return h.p(
      [Ui.className<Message>('m-0 text-xs text-white/40')],
      ['No deployments yet.'],
    )
  }

  return h.div(
    [
      Ui.className<Message>(
        '-mx-2 -my-2 overflow-x-auto whitespace-nowrap px-2 py-2',
      ),
    ],
    [
      h.table(
        [Ui.className<Message>('w-full border-collapse text-left text-xs/6')],
        [
          h.thead(
            [Ui.className<Message>('border-b border-[#222] text-white/45')],
            [
              h.tr(
                [],
                [
                  tableHeaderCell('Deployment'),
                  tableHeaderCell('Status'),
                  tableHeaderCell('Version'),
                  tableHeaderCell('URL'),
                  tableHeaderCell('Updated'),
                ],
              ),
            ],
          ),
          h.tbody(
            [Ui.className<Message>('divide-y divide-[#222]')],
            deployments.map(deployment =>
              h.tr(
                [],
                [
                  compactTableCell([deployment.id]),
                  compactTableCell([statusBadge(deployment.status)]),
                  compactTableCell([deployment.versionId]),
                  compactTableCell([
                    h.a(
                      [
                        h.Href(deployment.url),
                        h.Target('_blank'),
                        h.Rel('noreferrer'),
                        Ui.className<Message>(
                          'text-white/75 underline underline-offset-[3px] hover:text-[#ffb400]',
                        ),
                      ],
                      [deployment.url],
                    ),
                  ]),
                  compactTableCell([dateLabel(deployment.updatedAt)]),
                ],
              ),
            ),
          ),
        ],
      ),
    ],
  )
}

const reviewEventsList = (
  title: string,
  events: ReadonlyArray<AdminAdjutantReviewEvent>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-2')],
    [
      h.h4(
        [Ui.className<Message>('m-0 text-sm font-medium text-white/75')],
        [title],
      ),
      events.length === 0
        ? h.p(
            [Ui.className<Message>('m-0 text-xs text-white/40')],
            ['No events yet.'],
          )
        : h.ul(
            [Ui.className<Message>('m-0 grid list-none gap-2 p-0')],
            events
              .slice(0, 6)
              .map(event =>
                h.li(
                  [
                    Ui.className<Message>(
                      'grid gap-1 border-t border-[#222] pt-2',
                    ),
                  ],
                  [
                    h.div(
                      [Ui.className<Message>('flex flex-wrap gap-2 text-xs')],
                      [
                        statusBadge(event.type),
                        h.span(
                          [Ui.className<Message>('text-white/35')],
                          [dateLabel(event.createdAt)],
                        ),
                      ],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/60')],
                      [userFacingCopy(event.summary)],
                    ),
                  ],
                ),
              ),
          ),
    ],
  )
}

const reviewUsagePanel = (
  receipts: ReadonlyArray<AdminAdjutantReviewUsageReceipt>,
  summary: AdjutantUsageReceiptSummary,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-2')],
    [
      h.h4(
        [Ui.className<Message>('m-0 text-sm font-medium text-white/75')],
        ['Usage receipts'],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 border border-[#222] p-3')],
        [
          h.div(
            [Ui.className<Message>('grid gap-1 text-xs text-white/55')],
            [
              h.p(
                [Ui.className<Message>('m-0')],
                [`Billing: ${usageBillingModeLabel(summary.billingMode)}`],
              ),
              h.p(
                [Ui.className<Message>('m-0')],
                [`Total: ${summary.totalCreditsChargedFormatted}`],
              ),
              ...summary.categories.map(category =>
                h.p(
                  [Ui.className<Message>('m-0')],
                  [
                    `${usageCategoryLabel(category.category)}: ${usageQuantityLabel(category.quantity, category.unit)} / ${category.creditsChargedFormatted}`,
                  ],
                ),
              ),
            ],
          ),
          receipts.length === 0
            ? h.p(
                [Ui.className<Message>('m-0 text-xs text-white/40')],
                ['No usage receipts yet.'],
              )
            : h.div(
                [Ui.className<Message>('grid gap-2')],
                receipts.map(receipt =>
                  h.div(
                    [
                      Ui.className<Message>(
                        'grid gap-1 border-t border-[#222] pt-2 text-xs text-white/55 first:border-t-0 first:pt-0',
                      ),
                    ],
                    [
                      h.p(
                        [Ui.className<Message>('m-0 text-white/70')],
                        [userFacingCopy(receipt.summary)],
                      ),
                      h.p(
                        [Ui.className<Message>('m-0')],
                        [
                          `${usageCategoryLabel(receipt.category)} / ${usageQuantityLabel(receipt.quantity, receipt.unit)} / ${receipt.creditsChargedFormatted}`,
                        ],
                      ),
                    ],
                  ),
                ),
              ),
        ],
      ),
    ],
  )
}

const adjutantEnrichmentPanel = (
  assignmentId: string,
  enrichment: AdminAdjutantReviewEnrichment,
  actionState: AdminAdjutantEnrichmentActionState,
): Html => {
  const h = html<Message>()
  const brief = enrichment.researchBrief
  const proposedSource = enrichment.sourceCards.find(
    sourceCard => sourceCard.reviewStatus === 'proposed',
  )
  const sourceRows =
    enrichment.sourceCards.length === 0
      ? [
          h.p(
            [Ui.className<Message>('m-0 text-xs text-white/40')],
            ['No source cards yet.'],
          ),
        ]
      : enrichment.sourceCards.slice(0, 8).map(sourceCard =>
          h.div(
            [Ui.className<Message>('grid gap-2 border-t border-[#222] pt-3')],
            [
              h.div(
                [Ui.className<Message>('flex flex-wrap items-center gap-2')],
                [
                  statusBadge(sourceCard.reviewStatus),
                  h.a(
                    [
                      Ui.className<Message>(
                        'text-xs text-white/70 underline underline-offset-[3px] hover:text-white',
                      ),
                      h.Href(sourceCard.url),
                      h.Target('_blank'),
                      h.Rel('noreferrer'),
                    ],
                    [userFacingCopy(compactText(sourceCard.title, 90))],
                  ),
                  h.span(
                    [Ui.className<Message>('text-xs text-white/35')],
                    [sourceCard.domain],
                  ),
                ],
              ),
              sourceCard.highlightText === null
                ? h.p(
                    [Ui.className<Message>('m-0 text-xs text-white/35')],
                    [sourceCard.sourceCategory],
                  )
                : h.p(
                    [Ui.className<Message>('m-0 text-xs text-white/55')],
                    [
                      userFacingCopy(
                        compactText(sourceCard.highlightText, 180),
                      ),
                    ],
                  ),
              h.div(
                [Ui.className<Message>('flex flex-wrap gap-2')],
                [
                  Ui.button<Message>({
                    label: 'Approve',
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [
                      h.Type('button'),
                      ...(sourceCard.reviewStatus === 'proposed'
                        ? [
                            h.OnClick(
                              RequestedReviewAdminAdjutantSourceCard({
                                assignmentId,
                                reviewStatus: 'public_safe',
                                sourceId: sourceCard.id,
                              }),
                            ),
                          ]
                        : [h.Disabled(true)]),
                    ],
                  }),
                  Ui.button<Message>({
                    label: 'Reject',
                    size: 'sm',
                    variant: 'danger',
                    attrs: [
                      h.Type('button'),
                      ...(sourceCard.reviewStatus === 'proposed'
                        ? [
                            h.OnClick(
                              RequestedReviewAdminAdjutantSourceCard({
                                assignmentId,
                                reviewStatus: 'rejected',
                                sourceId: sourceCard.id,
                              }),
                            ),
                          ]
                        : [h.Disabled(true)]),
                    ],
                  }),
                ],
              ),
            ],
          ),
        )
  const briefSourceCount = brief?.sourceCards?.length ?? brief?.sourceCount ?? 0
  const canReviewBrief =
    brief !== null &&
    brief.status === 'needs_review' &&
    proposedSource === undefined
  const refreshResearch =
    enrichment.status === 'stale' ||
    enrichment.status === 'rejected' ||
    enrichment.status === 'failed'

  return h.div(
    [Ui.className<Message>('grid gap-3 border-t border-[#222] pt-4')],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          h.h4(
            [Ui.className<Message>('m-0 text-sm font-medium text-white/75')],
            ['Research'],
          ),
          statusBadge(enrichment.status),
          statusBadge(
            enrichment.exaConfigured ? 'exa_configured' : 'exa_missing',
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-xs text-white/55')],
        [userFacingCopy(enrichment.nextAction)],
      ),
      enrichmentActionNotice(actionState) ??
        h.span([Ui.className<Message>('hidden')], []),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 text-xs text-white/55 sm:grid-cols-3',
          ),
        ],
        [
          h.p(
            [Ui.className<Message>('m-0')],
            [`Run: ${enrichment.latestRun?.id ?? '-'}`],
          ),
          h.p(
            [Ui.className<Message>('m-0')],
            [
              `Requests: ${enrichment.latestRun?.requestCount ?? 0} / ${enrichment.latestRun?.requestBudget ?? 0}`,
            ],
          ),
          h.p(
            [Ui.className<Message>('m-0')],
            [
              `Sources: ${enrichment.sourceCards.length} / ${enrichment.latestRun?.approvedSourceCount ?? 0}`,
            ],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('flex flex-wrap gap-2')],
        [
          Ui.button<Message>({
            label: refreshResearch ? 'Refresh research' : 'Run research',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(enrichment.exaConfigured &&
              enrichment.status !== 'running' &&
              enrichment.status !== 'queued'
                ? [
                    h.OnClick(
                      RequestedRunAdminAdjutantEnrichment({
                        assignmentId,
                        ...(refreshResearch ? { refresh: true } : {}),
                      }),
                    ),
                  ]
                : [h.Disabled(true)]),
            ],
          }),
          Ui.button<Message>({
            label: 'Approve brief',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(canReviewBrief && brief !== null
                ? [
                    h.OnClick(
                      RequestedReviewAdminAdjutantResearchBrief({
                        assignmentId,
                        briefId: brief.id,
                        status: 'approved',
                      }),
                    ),
                  ]
                : [h.Disabled(true)]),
            ],
          }),
          Ui.button<Message>({
            label: 'Reject brief',
            size: 'sm',
            variant: 'danger',
            attrs: [
              h.Type('button'),
              ...(brief !== null && brief.status === 'needs_review'
                ? [
                    h.OnClick(
                      RequestedReviewAdminAdjutantResearchBrief({
                        assignmentId,
                        briefId: brief.id,
                        status: 'rejected',
                      }),
                    ),
                  ]
                : [h.Disabled(true)]),
            ],
          }),
          Ui.button<Message>({
            label: 'Mark stale',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              ...(brief !== null && brief.status !== 'stale'
                ? [
                    h.OnClick(
                      RequestedReviewAdminAdjutantResearchBrief({
                        assignmentId,
                        briefId: brief.id,
                        status: 'stale',
                      }),
                    ),
                  ]
                : [h.Disabled(true)]),
            ],
          }),
        ],
      ),
      brief === null
        ? h.p(
            [Ui.className<Message>('m-0 text-xs text-white/40')],
            ['No brief yet.'],
          )
        : h.div(
            [Ui.className<Message>('grid gap-2 border-t border-[#222] pt-3')],
            [
              h.div(
                [Ui.className<Message>('flex flex-wrap gap-2')],
                [
                  statusBadge(brief.status),
                  h.span(
                    [Ui.className<Message>('text-xs text-white/35')],
                    [`Brief ${brief.id}`],
                  ),
                  h.span(
                    [Ui.className<Message>('text-xs text-white/35')],
                    [countLabel(briefSourceCount, 'source')],
                  ),
                ],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-xs text-white/60')],
                [userFacingCopy(compactText(brief.summary, 220))],
              ),
            ],
          ),
      h.div([Ui.className<Message>('grid gap-2')], sourceRows),
    ],
  )
}

const adjutantAssignmentsTable = (
  assignments: ReadonlyArray<AdminAdjutantAssignment>,
): Html => {
  const h = html<Message>()

  if (assignments.length === 0) {
    return h.p(
      [
        Ui.className<Message>(
          'm-0 border-t border-[#222] py-4 text-sm text-white/45',
        ),
      ],
      ['No Autopilot assignments found.'],
    )
  }

  return tableWrap(
    'Autopilot assignments',
    ['Assignment', 'Status', 'Site', 'Run', 'Updated', 'Actions'],
    h.tbody(
      [Ui.className<Message>('divide-y divide-[#222]')],
      assignments.map(assignment =>
        h.tr(
          [],
          [
            tableCell([
              h.div(
                [Ui.className<Message>('font-medium text-white/80')],
                [assignment.id],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [assignment.assignmentKind],
              ),
            ]),
            tableCell([statusBadge(assignment.status)]),
            tableCell([nullableLabel(assignment.siteId)]),
            tableCell([nullableLabel(assignment.currentRunId)]),
            tableCell([dateLabel(assignment.updatedAt)]),
            tableCell([
              Ui.button<Message>({
                label: 'Review',
                size: 'sm',
                variant: 'secondary',
                attrs: [
                  h.Type('button'),
                  h.OnClick(
                    RequestedLoadAdminAdjutantReview({
                      assignmentId: assignment.id,
                    }),
                  ),
                ],
              }),
            ]),
          ],
        ),
      ),
    ),
  )
}

const adjutantReviewPanel = (
  reviewState: AdminAdjutantReviewState,
  actionState: AdminSiteDeploymentActionState,
  enrichmentActionState: AdminAdjutantEnrichmentActionState,
): Html => {
  const h = html<Message>()

  return M.value(reviewState).pipe(
    M.tagsExhaustive({
      AdminAdjutantReviewIdle: () =>
        h.p(
          [Ui.className<Message>('m-0 text-sm text-white/45')],
          ['Select an assignment to review.'],
        ),
      AdminAdjutantReviewLoading: ({ assignmentId }) =>
        h.p(
          [Ui.className<Message>('m-0 text-sm text-white/45')],
          [`Loading ${assignmentId}...`],
        ),
      AdminAdjutantReviewFailed: ({ error }) =>
        h.p([Ui.className<Message>('m-0 text-sm text-[#d32f2f]')], [error]),
      AdminAdjutantReviewLoaded: ({ assignment, review }) => {
        const savedVersion = savedReviewVersion(review.versions)
        const activeDeployment = activeReviewDeployment(review.deployments)
        const rollbackDeployment = rollbackReviewDeployment(review.deployments)
        const publicSite = isPublicReviewSite(review.site)
        const canDeploy =
          review.site !== null &&
          savedVersion !== undefined &&
          savedVersion.id !== review.site.activeVersionId
        const actionNotice = reviewActionNotice(actionState)

        return h.div(
          [
            Ui.className<Message>(
              'grid gap-5 border border-[#222] bg-[#050505] p-4',
            ),
          ],
          [
            h.div(
              [Ui.className<Message>('grid gap-2')],
              [
                h.div(
                  [Ui.className<Message>('flex flex-wrap items-center gap-2')],
                  [
                    h.h3(
                      [
                        Ui.className<Message>(
                          'm-0 text-base font-medium text-white/85',
                        ),
                      ],
                      [assignment.id],
                    ),
                    statusBadge(assignment.status),
                    statusBadge(assignment.visibility),
                  ],
                ),
                h.p(
                  [Ui.className<Message>('m-0 text-sm text-white/55')],
                  [review.nextAction],
                ),
                actionNotice,
              ].filter((item): item is Html => item !== null),
            ),
            h.div(
              [Ui.className<Message>('grid gap-4 lg:grid-cols-2')],
              [
                h.div(
                  [Ui.className<Message>('grid gap-2')],
                  [
                    h.h4(
                      [
                        Ui.className<Message>(
                          'm-0 text-sm font-medium text-white/75',
                        ),
                      ],
                      ['Order and Site'],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [`Order: ${review.order?.id ?? '-'}`],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [
                        `Repository: ${review.order?.repositoryFullName ?? '-'}`,
                      ],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [
                        `Site: ${review.site?.title ?? '-'} (${review.site?.slug ?? '-'})`,
                      ],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [
                        `Access: ${review.site?.accessMode ?? '-'} / ${review.site?.visibility ?? '-'}`,
                      ],
                    ),
                  ],
                ),
                h.div(
                  [Ui.className<Message>('grid gap-2')],
                  [
                    h.h4(
                      [
                        Ui.className<Message>(
                          'm-0 text-sm font-medium text-white/75',
                        ),
                      ],
                      ['Goal and run'],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [`Goal: ${review.goal?.id ?? '-'}`],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [
                        `Tokens: ${review.goal?.tokensUsed ?? 0} / ${review.goal?.tokenBudget ?? 'unbounded'}`,
                      ],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [`Run: ${review.currentRun?.id ?? '-'}`],
                    ),
                    h.p(
                      [Ui.className<Message>('m-0 text-xs text-white/55')],
                      [
                        `Runtime: ${review.currentRun?.runtime ?? '-'} / ${review.currentRun?.backend ?? '-'}`,
                      ],
                    ),
                  ],
                ),
                reviewUsagePanel(review.usageReceipts, review.usageSummary),
              ],
            ),
            adjutantEnrichmentPanel(
              assignment.id,
              review.enrichment,
              enrichmentActionState,
            ),
            h.div(
              [Ui.className<Message>('flex flex-wrap gap-2')],
              [
                Ui.button<Message>({
                  label: publicSite ? 'Deploy with checklist' : 'Deploy',
                  size: 'sm',
                  variant: 'secondary',
                  attrs: [
                    h.Type('button'),
                    ...(canDeploy &&
                    review.site !== null &&
                    savedVersion !== undefined
                      ? [
                          h.OnClick(
                            RequestedDeployAdminSiteVersion({
                              assignmentId: assignment.id,
                              publicLaunchChecklist: publicSite,
                              siteId: review.site.id,
                              versionId: savedVersion.id,
                            }),
                          ),
                        ]
                      : [h.Disabled(true)]),
                  ],
                }),
                Ui.button<Message>({
                  label: 'Rollback',
                  size: 'sm',
                  variant: 'secondary',
                  attrs: [
                    h.Type('button'),
                    ...(review.site !== null && rollbackDeployment !== undefined
                      ? [
                          h.OnClick(
                            RequestedAdminSiteDeploymentAction({
                              action: 'rollback',
                              assignmentId: assignment.id,
                              deploymentId: rollbackDeployment.id,
                              siteId: review.site.id,
                            }),
                          ),
                        ]
                      : [h.Disabled(true)]),
                  ],
                }),
                Ui.button<Message>({
                  label: 'Disable',
                  size: 'sm',
                  variant: 'danger',
                  attrs: [
                    h.Type('button'),
                    ...(review.site !== null && activeDeployment !== undefined
                      ? [
                          h.OnClick(
                            RequestedAdminSiteDeploymentAction({
                              action: 'disable',
                              assignmentId: assignment.id,
                              deploymentId: activeDeployment.id,
                              siteId: review.site.id,
                            }),
                          ),
                        ]
                      : [h.Disabled(true)]),
                  ],
                }),
              ],
            ),
            publicSite
              ? h.p(
                  [Ui.className<Message>('m-0 text-xs text-white/40')],
                  [
                    'Deploy sends the explicit Sites launch checklist: source, build, audience, secrets, and URL reviewed.',
                  ],
                )
              : h.p(
                  [Ui.className<Message>('m-0 text-xs text-white/40')],
                  ['Deployment actions are recorded as Site events.'],
                ),
            h.div(
              [Ui.className<Message>('grid gap-4')],
              [
                h.div(
                  [Ui.className<Message>('grid gap-2')],
                  [
                    h.h4(
                      [
                        Ui.className<Message>(
                          'm-0 text-sm font-medium text-white/75',
                        ),
                      ],
                      ['Versions'],
                    ),
                    reviewVersionsTable(review.versions),
                  ],
                ),
                h.div(
                  [Ui.className<Message>('grid gap-2')],
                  [
                    h.h4(
                      [
                        Ui.className<Message>(
                          'm-0 text-sm font-medium text-white/75',
                        ),
                      ],
                      ['Deployments'],
                    ),
                    reviewDeploymentsTable(review.deployments),
                  ],
                ),
                h.div(
                  [Ui.className<Message>('grid gap-4 lg:grid-cols-2')],
                  [
                    reviewEventsList(
                      'Assignment events',
                      review.assignmentEvents,
                    ),
                    reviewEventsList('Site events', review.siteEvents),
                  ],
                ),
              ],
            ),
          ],
        )
      },
    }),
  )
}

const adjutantReviewSection = (
  assignmentsState: AdminAdjutantAssignmentsState,
  reviewState: AdminAdjutantReviewState,
  actionState: AdminSiteDeploymentActionState,
  enrichmentActionState: AdminAdjutantEnrichmentActionState,
): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3')],
    [
      h.h2(
        [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
        ['Autopilot reviews'],
      ),
      M.value(assignmentsState).pipe(
        M.tagsExhaustive({
          AdminAdjutantAssignmentsIdle: () =>
            h.p(
              [Ui.className<Message>('m-0 text-sm text-white/45')],
              ['Loading...'],
            ),
          AdminAdjutantAssignmentsLoading: () =>
            h.p(
              [Ui.className<Message>('m-0 text-sm text-white/45')],
              ['Loading...'],
            ),
          AdminAdjutantAssignmentsFailed: ({ error }) =>
            h.p([Ui.className<Message>('m-0 text-sm text-[#d32f2f]')], [error]),
          AdminAdjutantAssignmentsLoaded: ({ assignments }) =>
            adjutantAssignmentsTable(assignments),
        }),
      ),
      adjutantReviewPanel(reviewState, actionState, enrichmentActionState),
    ],
  )
}

const requestTableCell = (order: AdminOverviewSoftwareOrder): Html => {
  const h = html<Message>()

  return h.td(
    [
      Ui.className<Message>(
        'min-w-[28rem] max-w-[40rem] py-3 pr-6 align-top text-white/65',
      ),
    ],
    [
      h.details(
        [Ui.className<Message>('group grid gap-3 whitespace-normal')],
        [
          h.summary(
            [
              Ui.className<Message>(
                'grid cursor-pointer list-none gap-2 text-white/65 outline-none focus-visible:ring-2 focus-visible:ring-[#ffb400]/70',
              ),
            ],
            [
              h.span(
                [Ui.className<Message>('text-white/65')],
                [compactText(order.request, 180)],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'text-xs text-white/40 underline underline-offset-[3px] group-open:hidden',
                  ),
                ],
                ['Open full request'],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'hidden text-xs text-white/40 underline underline-offset-[3px] group-open:inline',
                  ),
                ],
                ['Close full request'],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'mt-3 max-h-[70vh] w-[min(72vw,64rem)] overflow-auto border border-[#222] bg-[#030303] p-4',
              ),
            ],
            [markdownView(order.request)],
          ),
        ],
      ),
    ],
  )
}

const tableWrap = (
  caption: string,
  headers: ReadonlyArray<string>,
  rows: Html,
) => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        '-mx-4 -my-2 overflow-x-auto whitespace-nowrap px-4 py-2',
      ),
    ],
    [
      h.table(
        [
          Ui.className<Message>(
            'w-full border-collapse text-left text-base/7 sm:text-sm/6',
          ),
        ],
        [
          h.caption([Ui.className<Message>('sr-only')], [caption]),
          h.thead(
            [Ui.className<Message>('border-b border-[#222]')],
            [h.tr([], headers.map(tableHeaderCell))],
          ),
          rows,
        ],
      ),
    ],
  )
}

const usersTable = (users: ReadonlyArray<AdminOverviewUser>): Html => {
  const h = html<Message>()

  if (users.length === 0) {
    return h.p(
      [
        Ui.className<Message>(
          'm-0 border-t border-[#222] py-4 text-sm text-white/45',
        ),
      ],
      ['No users found.'],
    )
  }

  return tableWrap(
    'OpenAgents users',
    ['User', 'Email', 'Kind', 'Status', 'Onboarding', 'Orders', 'Updated'],
    h.tbody(
      [Ui.className<Message>('divide-y divide-[#222]')],
      users.map(user =>
        h.tr(
          [],
          [
            tableCell([
              h.div(
                [Ui.className<Message>('font-medium text-white/80')],
                [user.displayName],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [user.githubUsername ?? user.userId],
              ),
            ]),
            tableCell([nullableLabel(user.email)]),
            tableCell([user.kind]),
            tableCell([statusBadge(user.status)]),
            tableCell([
              user.onboardingStep,
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [dateLabel(user.onboardingCompletedAt)],
              ),
            ]),
            tableCell([String(user.softwareOrderCount)]),
            tableCell([dateLabel(user.updatedAt)]),
          ],
        ),
      ),
    ),
  )
}

const softwareOrdersTable = (
  softwareOrders: ReadonlyArray<AdminOverviewSoftwareOrder>,
): Html => {
  const h = html<Message>()

  if (softwareOrders.length === 0) {
    return h.p(
      [
        Ui.className<Message>(
          'm-0 border-t border-[#222] py-4 text-sm text-white/45',
        ),
      ],
      ['No software orders found.'],
    )
  }

  return tableWrap(
    'Software orders',
    [
      'Order',
      'User',
      'Status',
      'Repository',
      'Site',
      'Run',
      'Created',
      'Request',
    ],
    h.tbody(
      [Ui.className<Message>('divide-y divide-[#222]')],
      softwareOrders.map(order =>
        h.tr(
          [],
          [
            tableCell([
              h.div(
                [Ui.className<Message>('font-medium text-white/80')],
                [order.id],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [order.visibility],
              ),
            ]),
            tableCell([
              nullableLabel(order.userDisplayName),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [nullableLabel(order.userEmail)],
              ),
            ]),
            tableCell([statusBadge(order.status)]),
            tableCell([repoCell(order.repositoryFullName)]),
            tableCell([
              order.siteProjectId === null
                ? h.span([Ui.className<Message>('text-white/35')], ['None'])
                : h.div(
                    [],
                    [
                      h.div(
                        [Ui.className<Message>('font-medium text-white/75')],
                        [order.siteSlug ?? order.siteProjectId],
                      ),
                      h.div(
                        [Ui.className<Message>('text-xs text-white/35')],
                        [
                          [
                            order.siteStatus ?? 'unknown',
                            order.siteVisibility ?? 'unknown',
                          ].join(' / '),
                        ],
                      ),
                    ],
                  ),
            ]),
            tableCell([nullableLabel(order.currentRunId)]),
            tableCell([dateLabel(order.createdAt)]),
            requestTableCell(order),
          ],
        ),
      ),
    ),
  )
}

const siteUrlCell = (order: SiteSoftwareOrder): Html => {
  const h = html<Message>()

  if (order.siteActiveUrl === null) {
    return h.div(
      [Ui.className<Message>('text-xs text-white/35')],
      ['No active URL'],
    )
  }

  return h.a(
    [
      h.Href(order.siteActiveUrl),
      h.Target('_blank'),
      h.Rel('noreferrer'),
      Ui.className<Message>(
        'block max-w-[18rem] overflow-hidden text-ellipsis text-white/75 underline underline-offset-[3px] hover:text-[#ffb400]',
      ),
    ],
    [order.siteActiveUrl],
  )
}

const sitesTable = (
  softwareOrders: ReadonlyArray<AdminOverviewSoftwareOrder>,
  assignmentsState: AdminAdjutantAssignmentsState,
  actionState: AdminSiteDeploymentActionState,
): Html => {
  const h = html<Message>()
  const siteOrders = softwareOrders.filter(hasSiteProject)
  const assignments = assignmentsFromState(assignmentsState)
  const actionNotice = reviewActionNotice(actionState)

  if (siteOrders.length === 0) {
    return h.p(
      [
        Ui.className<Message>(
          'm-0 border-t border-[#222] py-4 text-sm text-white/45',
        ),
      ],
      ['No Sites found.'],
    )
  }

  return h.div(
    [Ui.className<Message>('grid gap-3')],
    [
      ...(actionNotice === null ? [] : [actionNotice]),
      tableWrap(
        'Autopilot Sites',
        [
          'Site',
          'Access',
          'Versions',
          'Deployments',
          'Storage',
          'Readiness',
          'Event',
          'Actions',
        ],
        h.tbody(
          [Ui.className<Message>('divide-y divide-[#222]')],
          siteOrders.map(order => {
            const assignment = assignmentForSiteOrder(order, assignments)
            const savedVersionId =
              order.siteLatestVersionStatus === 'saved'
                ? order.siteLatestVersionId
                : null
            const activeDeploymentId =
              order.siteLatestDeploymentStatus === 'active'
                ? order.siteLatestDeploymentId
                : order.siteActiveDeploymentId
            const rollbackDeploymentId =
              order.siteLatestDeploymentStatus === 'rolled_back'
                ? order.siteLatestDeploymentId
                : null
            const publicSite =
              order.siteAccessMode === 'public' ||
              order.siteVisibility === 'public'

            return h.tr(
              [],
              [
            tableCell([
              h.div(
                [Ui.className<Message>('font-medium text-white/80')],
                [order.siteSlug ?? order.siteProjectId],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [order.siteTitle ?? order.siteProjectId],
              ),
              siteUrlCell(order),
            ]),
            tableCell([
              statusBadge(order.siteStatus ?? 'unknown'),
              h.div(
                [Ui.className<Message>('mt-2 text-xs text-white/35')],
                [
                  `${order.siteAccessMode ?? 'unknown'} / ${order.siteVisibility ?? 'unknown'}`,
                ],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [
                  `${order.siteAccessGrantCount} access grant${order.siteAccessGrantCount === 1 ? '' : 's'}`,
                ],
              ),
            ]),
            tableCell([
              `${order.siteVersionCount} version${order.siteVersionCount === 1 ? '' : 's'}`,
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [
                  [
                    order.siteLatestVersionId ?? 'no version',
                    order.siteLatestVersionStatus ?? 'unknown',
                    order.siteLatestVersionSourceKind ?? 'unknown',
                  ].join(' / '),
                ],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [dateLabel(order.siteLatestVersionCreatedAt)],
              ),
            ]),
            tableCell([
              `${order.siteDeploymentCount} deployment${order.siteDeploymentCount === 1 ? '' : 's'}`,
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [
                  [
                    order.siteLatestDeploymentId ?? 'no deployment',
                    order.siteLatestDeploymentStatus ?? 'unknown',
                    order.siteLatestDeploymentRuntimeKind ?? 'unknown',
                  ].join(' / '),
                ],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [dateLabel(order.siteLatestDeploymentUpdatedAt)],
              ),
            ]),
            tableCell([
              `${order.siteStorageBindingCount} binding${order.siteStorageBindingCount === 1 ? '' : 's'}`,
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [nullableLabel(order.siteStorageBindingSummary)],
              ),
              h.div(
                [Ui.className<Message>('mt-2 text-xs text-white/35')],
                [
                  `${order.siteEnvironmentValueCount} env key${order.siteEnvironmentValueCount === 1 ? '' : 's'}`,
                ],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [nullableLabel(order.siteEnvironmentKeySummary)],
              ),
            ]),
            tableCell([
              h.div(
                [Ui.className<Message>('grid gap-2 text-xs text-white/55')],
                [
                  h.div(
                    [],
                    [
                      h.span([Ui.className<Message>('text-white/35')], [
                        'Compatibility ',
                      ]),
                      statusBadge(
                        order.siteLatestCompatibilityStatus ?? 'not checked',
                      ),
                      h.div(
                        [Ui.className<Message>('mt-1 text-white/35')],
                        [
                          `${order.siteLatestCompatibilityBlockerCount} blockers / ${order.siteLatestCompatibilityWarningCount} warnings`,
                        ],
                      ),
                      h.div(
                        [Ui.className<Message>('mt-1 whitespace-normal')],
                        [
                          nullableLabel(
                            order.siteLatestCompatibilityCustomerSafeStatus,
                          ),
                        ],
                      ),
                    ],
                  ),
                  h.div(
                    [],
                    [
                      h.span([Ui.className<Message>('text-white/35')], [
                        'Build ',
                      ]),
                      statusBadge(
                        order.siteLatestBuildValidationStatus ?? 'not checked',
                      ),
                      h.div(
                        [Ui.className<Message>('mt-1 text-white/35')],
                        [
                          `${order.siteLatestBuildValidationBlockerCount} blockers / ${order.siteLatestBuildValidationWarningCount} warnings`,
                        ],
                      ),
                      h.div(
                        [Ui.className<Message>('mt-1 whitespace-normal')],
                        [
                          nullableLabel(
                            order.siteLatestBuildValidationCustomerSafeStatus,
                          ),
                        ],
                      ),
                    ],
                  ),
                  ...(order.siteSlug === 'ben-otec' ||
                  order.repositoryFullName === 'bensilone/openagents'
                    ? [
                        h.a(
                          [
                            h.Href('/api/public/proof/otec'),
                            h.Target('_blank'),
                            h.Rel('noreferrer'),
                            Ui.className<Message>(
                              'text-white/75 underline underline-offset-[3px] hover:text-[#ffb400]',
                            ),
                          ],
                          ['Public proof'],
                        ),
                      ]
                    : []),
                ],
              ),
            ]),
            tableCell([
              h.span(
                [Ui.className<Message>('whitespace-normal text-white/65')],
                [nullableLabel(order.siteLatestEventSummary)],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [nullableLabel(order.siteLatestEventType)],
              ),
              h.div(
                [Ui.className<Message>('text-xs text-white/35')],
                [dateLabel(order.siteLatestEventCreatedAt)],
              ),
            ]),
            tableCell([
              h.div(
                [Ui.className<Message>('flex flex-wrap gap-2')],
                [
                  Ui.button<Message>({
                    label: 'Review',
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [
                      h.Type('button'),
                      ...(assignment === undefined
                        ? [h.Disabled(true)]
                        : [
                            h.OnClick(
                              RequestedLoadAdminAdjutantReview({
                                assignmentId: assignment.id,
                              }),
                            ),
                          ]),
                    ],
                  }),
                  Ui.button<Message>({
                    label: 'Generate',
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [
                      h.Type('button'),
                      h.OnClick(
                        RequestedGenerateAdminSite({
                          siteId: order.siteProjectId,
                        }),
                      ),
                    ],
                  }),
                  Ui.button<Message>({
                    label: 'Save',
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [h.Type('button'), h.Disabled(true)],
                  }),
                  Ui.button<Message>({
                    label: 'Deploy',
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [
                      h.Type('button'),
                      ...(assignment === undefined || savedVersionId === null
                        ? [h.Disabled(true)]
                        : [
                            h.OnClick(
                              RequestedDeployAdminSiteVersion({
                                assignmentId: assignment.id,
                                publicLaunchChecklist: publicSite,
                                siteId: order.siteProjectId,
                                versionId: savedVersionId,
                              }),
                            ),
                          ]),
                    ],
                  }),
                  Ui.button<Message>({
                    label: 'Rollback',
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [
                      h.Type('button'),
                      ...(assignment === undefined ||
                      rollbackDeploymentId === null
                        ? [h.Disabled(true)]
                        : [
                            h.OnClick(
                              RequestedAdminSiteDeploymentAction({
                                action: 'rollback',
                                assignmentId: assignment.id,
                                deploymentId: rollbackDeploymentId,
                                siteId: order.siteProjectId,
                              }),
                            ),
                          ]),
                    ],
                  }),
                  Ui.button<Message>({
                    label: 'Disable',
                    size: 'sm',
                    variant: 'danger',
                    attrs: [
                      h.Type('button'),
                      ...(assignment === undefined ||
                      activeDeploymentId === null
                        ? [h.Disabled(true)]
                        : [
                            h.OnClick(
                              RequestedAdminSiteDeploymentAction({
                                action: 'disable',
                                assignmentId: assignment.id,
                                deploymentId: activeDeploymentId,
                                siteId: order.siteProjectId,
                              }),
                            ),
                          ]),
                    ],
                  }),
                ],
              ),
            ]),
              ],
            )
          }),
        ),
      ),
    ],
  )
}

const header = (users: number, softwareOrders: number): Html => {
  const h = html<Message>()

  return h.header(
    [Ui.className<Message>('flex flex-wrap items-start justify-between gap-4')],
    [
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Admin']),
          h.h1(
            [Ui.className<Message>('m-0 text-2xl font-semibold text-white/90')],
            ['Overview'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 text-base/7 text-white/55 sm:text-sm/6',
              ),
            ],
            [
              `${countLabel(users, 'user')} and ${countLabel(softwareOrders, 'software order')} in the current database.`,
            ],
          ),
        ],
      ),
      Ui.button<Message>({
        label: 'Refresh',
        size: 'sm',
        variant: 'secondary',
        attrs: [h.Type('button'), h.OnClick(RequestedLoadAdminOverview())],
      }),
    ],
  )
}

const overview = (
  users: ReadonlyArray<AdminOverviewUser>,
  softwareOrders: ReadonlyArray<AdminOverviewSoftwareOrder>,
  assignmentsState: AdminAdjutantAssignmentsState,
  reviewState: AdminAdjutantReviewState,
  actionState: AdminSiteDeploymentActionState,
  enrichmentActionState: AdminAdjutantEnrichmentActionState,
): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-8')],
    [
      header(users.length, softwareOrders.length),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          h.h2(
            [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
            ['Sites'],
          ),
          sitesTable(softwareOrders, assignmentsState, actionState),
        ],
      ),
      adjutantReviewSection(
        assignmentsState,
        reviewState,
        actionState,
        enrichmentActionState,
      ),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          h.h2(
            [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
            ['Users'],
          ),
          usersTable(users),
        ],
      ),
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          h.h2(
            [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
            ['Software orders'],
          ),
          softwareOrdersTable(softwareOrders),
        ],
      ),
    ],
  )
}

const loadingView = (): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-4')],
    [
      header(0, 0),
      h.p([Ui.className<Message>('m-0 text-sm text-white/45')], ['Loading...']),
    ],
  )
}

const failedView = (error: string): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-4')],
    [
      header(0, 0),
      h.div(
        [Ui.className<Message>('grid gap-3 border border-[#333] bg-black p-5')],
        [
          h.p([Ui.className<Message>('m-0 text-sm text-[#d32f2f]')], [error]),
          Ui.button<Message>({
            label: 'Retry',
            size: 'sm',
            variant: 'secondary',
            attrs: [h.Type('button'), h.OnClick(RequestedLoadAdminOverview())],
          }),
        ],
      ),
    ],
  )
}

export const view = (model: Model): Html => {
  const h = html<Message>()

  if (!model.auth.isAdmin) {
    return Ui.container<Message>([
      h.section(
        [Ui.className<Message>('grid gap-3 border border-[#333] bg-black p-5')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Admin']),
          h.h1(
            [Ui.className<Message>('m-0 text-2xl font-semibold text-white/90')],
            ['Access required'],
          ),
        ],
      ),
    ])
  }

  return Ui.container<Message>([
    M.value(model.adminOverview).pipe(
      M.tagsExhaustive({
        AdminOverviewIdle: () => loadingView(),
        AdminOverviewLoading: () => loadingView(),
        AdminOverviewLoaded: ({ users, softwareOrders }) =>
          overview(
            users,
            softwareOrders,
            model.adminAdjutantAssignments,
            model.adminAdjutantReview,
            model.adminSiteDeploymentAction,
            model.adminAdjutantEnrichmentAction,
          ),
        AdminOverviewFailed: ({ error }) => failedView(error),
      }),
    ),
  ])
}
