import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { userFacingCopy } from '../../../display-copy'
import { formatIsoDateTime } from '../../../time-format'
import * as Ui from '../../../ui'
import { orderDetailRouter } from '../../../route'
import {
  Message,
  RequestedLoadCustomerOrder,
  RequestedLoadCustomerOrders,
  RequestedLoadCustomerSiteBuilderEvents,
  RequestedOpenCustomerSiteBuilderSession,
  SelectedCustomerSiteBuilderFile,
  SelectedCustomerSiteElementContext,
  SubmittedCustomerOrder,
  SubmittedCustomerSiteFeedback,
  UpdatedCustomerSiteBuilderPromptDraft,
  UpdatedCustomerOrderDraft,
  UpdatedCustomerSiteFeedbackDraft,
} from '../message'
import type {
  AdjutantUsageReceiptBillingMode,
  AdjutantUsageReceiptCategory,
  CustomerFulfillmentArtifact,
  CustomerFulfillmentArtifactKind,
  CustomerFulfillmentArtifactStatus,
  CustomerOrder,
  CustomerOrderAdjutantStage,
  CustomerOrderStatus,
  CustomerSiteBuilderActorKind,
  CustomerSiteBuilderEvent,
  CustomerSiteBuilderFile,
  CustomerSiteBuilderFileTreeItem,
  CustomerSiteBuilderPhase,
  CustomerSiteBuilderPhaseStatus,
  CustomerSiteBuilderSession,
  CustomerSiteBuilderSessionStatus,
  CustomerSiteFeedback,
  CustomerSiteFeedbackStatus,
  CustomerSiteRevision,
  CustomerSiteRevisionReviewState,
  Model,
} from '../model'
import {
  SiteElementContext,
  safeSiteElementContext,
} from '../site-element-context'
import { siteCodeViewerContextFromElement } from '../site-code-context'
import { InstallSitePreviewElementTargetBridge } from '../site-preview-bridge'

const statusLabel = (status: CustomerOrderStatus): string =>
  M.value(status).pipe(
    M.when('submitted', () => 'Submitted'),
    M.when('scoping', () => 'Scoping'),
    M.when('free_slice_ready', () => 'Free slice ready'),
    M.when('quote_ready', () => 'Quote ready'),
    M.when('agent_queued', () => 'Agent queued'),
    M.when('agent_running', () => 'Agent running'),
    M.when('delivered', () => 'Delivered'),
    M.when('needs_customer_input', () => 'Needs input'),
    M.when('declined', () => 'Declined'),
    M.when('unavailable', () => 'Unavailable'),
    M.exhaustive,
  )

const statusTone = (
  status: CustomerOrderStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  M.value(status).pipe(
    M.when('submitted', () => 'accent' as const),
    M.when('scoping', () => 'info' as const),
    M.when('free_slice_ready', () => 'positive' as const),
    M.when('quote_ready', () => 'positive' as const),
    M.when('agent_queued', () => 'accent' as const),
    M.when('agent_running', () => 'info' as const),
    M.when('delivered', () => 'positive' as const),
    M.when('needs_customer_input', () => 'warning' as const),
    M.when('declined', () => 'negative' as const),
    M.when('unavailable', () => 'negative' as const),
    M.exhaustive,
  )

const statusBody = (order: CustomerOrder): string =>
  M.value(order.status).pipe(
    M.when(
      'submitted',
      () => 'Your request is in the public beta intake queue.',
    ),
    M.when('scoping', () => 'OpenAgents is scoping the first useful slice.'),
    M.when(
      'free_slice_ready',
      () => 'A free public slice is ready for operator review.',
    ),
    M.when('quote_ready', () => 'A broader paid scope is ready for review.'),
    M.when('agent_queued', () => 'The agent is queued for public work.'),
    M.when('agent_running', () => 'The agent has started working.'),
    M.when('delivered', () => 'The public result is ready.'),
    M.when('needs_customer_input', () => 'OpenAgents needs input from you.'),
    M.when('declined', () => 'OpenAgents cannot take this request right now.'),
    M.when('unavailable', () => 'This request is unavailable.'),
    M.exhaustive,
  )

const money = (cents: number | null): string =>
  cents === null ? 'Not priced' : `$${(cents / 100).toFixed(0)}`

const siteStatusLabel = (status: string): string => status.replaceAll('_', ' ')

const revisionReviewStateLabel = (
  state: CustomerSiteRevisionReviewState,
): string => state.replaceAll('_', ' ')

const feedbackStatusLabel = (status: CustomerSiteFeedbackStatus): string =>
  status.replaceAll('_', ' ')

const builderSessionStatusLabel = (
  status: CustomerSiteBuilderSessionStatus,
): string => status.replaceAll('_', ' ')

const builderActorLabel = (actorKind: CustomerSiteBuilderActorKind): string =>
  M.value(actorKind).pipe(
    M.when('customer', () => 'You'),
    M.when('agent', () => 'Agent'),
    M.when('operator', () => 'Operator'),
    M.when('system', () => 'System'),
    M.exhaustive,
  )

const builderPhaseStatusLabel = (
  status: CustomerSiteBuilderPhaseStatus,
): string => status.replaceAll('_', ' ')

const builderPhaseTone = (
  status: CustomerSiteBuilderPhaseStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  M.value(status).pipe(
    M.when('queued', () => 'accent' as const),
    M.when('running', () => 'info' as const),
    M.when('succeeded', () => 'positive' as const),
    M.when('failed', () => 'negative' as const),
    M.when('blocked', () => 'warning' as const),
    M.when('skipped', () => 'warning' as const),
    M.exhaustive,
  )

const builderNextAction = (session: CustomerSiteBuilderSession): string =>
  M.value(session.status).pipe(
    M.when('draft', () => 'OpenAgents is ready to plan the next Site pass.'),
    M.when('planning', () => 'The agent is planning the Site build.'),
    M.when('building', () => 'The agent is changing generated Site files.'),
    M.when('preview_ready', () => 'A preview is ready to inspect.'),
    M.when('review_ready', () => 'A review-ready result is available.'),
    M.when('saved', () => 'A deployable revision has been saved.'),
    M.when('deploying', () => 'The saved revision is being deployed.'),
    M.when('deployed', () => 'The latest revision is live.'),
    M.when('failed', () => 'The builder run failed and needs attention.'),
    M.when('archived', () => 'This builder session is archived.'),
    M.exhaustive,
  )

const builderPromptSummary = (order: CustomerOrder): string =>
  order.site === null
    ? order.request
    : `Continue building ${order.request}`.slice(0, 320)

const siteAuthoringPromptSummary = (
  model: Model,
  order: CustomerOrder,
): string => {
  const draft = model.customerSiteBuilderPromptDraft.trim()

  return draft === '' ? builderPromptSummary(order) : draft
}

const SITE_EDITOR_SIDEBAR_WIDTH_PX = 336

const siteElementTargets = [
  safeSiteElementContext({
    attributes: [
      { name: 'class', value: 'button' },
      { name: 'href', value: '#returns' },
    ],
    selector: 'main a[href="#returns"]',
    tag: 'a',
    text: 'Investment case',
  }),
  safeSiteElementContext({
    attributes: [{ name: 'id', value: 'hero-title' }],
    selector: 'main h1#hero-title',
    tag: 'h1',
    text: 'Hero headline',
  }),
  safeSiteElementContext({
    attributes: [
      { name: 'id', value: 'evidence' },
      { name: 'role', value: 'region' },
    ],
    selector: 'main section#evidence',
    tag: 'section',
    text: 'Evidence section',
  }),
].filter((context): context is SiteElementContext => context !== null)

const sitePreviewBridgeOrigin = (order: CustomerOrder): string | null => {
  const url = order.site?.activeUrl

  if (url === null || url === undefined) {
    return null
  }

  try {
    return new URL(url).origin
  } catch {
    return null
  }
}

const artifactKindLabel = (kind: CustomerFulfillmentArtifactKind): string =>
  M.value(kind).pipe(
    M.when('pull_request', () => 'Pull request'),
    M.when('branch', () => 'Branch'),
    M.when('commit', () => 'Commit'),
    M.when('diff', () => 'Diff'),
    M.when('preview', () => 'Preview'),
    M.when('notes', () => 'Notes'),
    M.when('attachment', () => 'Attachment'),
    M.exhaustive,
  )

const artifactStatusLabel = (
  status: CustomerFulfillmentArtifactStatus,
): string =>
  M.value(status).pipe(
    M.when('draft', () => 'Draft'),
    M.when('customer_review_ready', () => 'Ready for review'),
    M.when('customer_accepted', () => 'Accepted'),
    M.when('superseded', () => 'Previous'),
    M.when('rejected', () => 'Rejected'),
    M.exhaustive,
  )

const categoryLabel = (category: AdjutantUsageReceiptCategory): string =>
  category.replaceAll('_', ' ')

const billingModeLabel = (mode: AdjutantUsageReceiptBillingMode): string =>
  M.value(mode).pipe(
    M.when('public_beta_free', () => 'Public beta free'),
    M.when('paid_credits', () => 'Paid credits'),
    M.exhaustive,
  )

const quantityLabel = (quantity: number, unit: string | null): string =>
  unit === null ? String(quantity) : `${quantity} ${unit}`

const adjutantStageLabel = (stage: CustomerOrderAdjutantStage): string =>
  M.value(stage).pipe(
    M.when('queued', () => 'Queued'),
    M.when('running', () => 'Running'),
    M.when('reviewing', () => 'Reviewing'),
    M.when('deployed', () => 'Deployed'),
    M.when('waiting_for_input', () => 'Waiting for input'),
    M.when('unavailable', () => 'Unavailable'),
    M.exhaustive,
  )

const adjutantStageTone = (
  stage: CustomerOrderAdjutantStage,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  M.value(stage).pipe(
    M.when('queued', () => 'accent' as const),
    M.when('running', () => 'info' as const),
    M.when('reviewing', () => 'warning' as const),
    M.when('deployed', () => 'positive' as const),
    M.when('waiting_for_input', () => 'warning' as const),
    M.when('unavailable', () => 'negative' as const),
    M.exhaustive,
  )

const detailRow = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#222] py-3 last:border-b-0 sm:grid-cols-[10rem_minmax(0,1fr)] sm:gap-4',
      ),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], [label]),
      h.div(
        [
          Ui.className<Message>(
            'min-w-0 text-base/7 text-white/75 sm:text-sm/6',
          ),
        ],
        [value],
      ),
    ],
  )
}

const progressDetailRow = (
  label: string,
  children: ReadonlyArray<Html | string>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#222] py-3 last:border-b-0 sm:grid-cols-[9rem_minmax(0,1fr)] sm:gap-4',
      ),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], [label]),
      h.div(
        [
          Ui.className<Message>(
            'min-w-0 text-base/7 text-white/75 sm:text-sm/6',
          ),
        ],
        children,
      ),
    ],
  )
}

const sitePanel = (order: CustomerOrder): Html | null => {
  const h = html<Message>()

  if (order.site === null) {
    return null
  }

  return h.section(
    [Ui.className<Message>('grid gap-3 border border-[#333] bg-[#080808] p-4')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Site']),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-3')],
        [
          h.div(
            [
              Ui.className<Message>(
                'inline-flex items-center border border-[#333] px-2.5 py-1.5 text-xs uppercase text-white/70',
              ),
            ],
            [siteStatusLabel(order.site.status)],
          ),
          order.site.latestSavedVersionId === null
            ? h.span(
                [
                  Ui.className<Message>(
                    'text-base/7 text-white/45 sm:text-sm/6',
                  ),
                ],
                ['No saved revision yet'],
              )
            : h.span(
                [
                  Ui.className<Message>(
                    'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-base/7 text-white/55 sm:text-sm/6',
                  ),
                ],
                [`Latest ${order.site.latestSavedVersionId}`],
              ),
          order.site.activeUrl === null
            ? h.span(
                [
                  Ui.className<Message>(
                    'text-base/7 text-white/45 sm:text-sm/6',
                  ),
                ],
                ['No active URL yet'],
              )
            : h.a(
                [
                  h.Href(order.site.activeUrl),
                  h.Target('_blank'),
                  h.Rel('noreferrer'),
                  Ui.className<Message>(
                    'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-base/7 text-white/80 underline underline-offset-[3px] hover:text-[#ffb400] sm:text-sm/6',
                  ),
                ],
                [order.site.activeUrl],
              ),
        ],
      ),
    ],
  )
}

const revisionRow = (revision: CustomerSiteRevision): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Message>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [
              revision.active
                ? 'Current'
                : siteStatusLabel(revision.buildStatus),
            ],
          ),
          h.span(
            [
              Ui.className<Message>(
                'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/70',
              ),
            ],
            [revision.id],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-1 sm:grid-cols-[8rem_minmax(0,1fr)]')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Review state']),
          h.div(
            [Ui.className<Message>('text-sm/6 text-white/70')],
            [revisionReviewStateLabel(revision.reviewState)],
          ),
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Deployment']),
          h.div(
            [Ui.className<Message>('text-sm/6 text-white/70')],
            [
              revision.deploymentStatus === null
                ? revision.url === null
                  ? 'No deployment yet'
                  : 'Previous revision'
                : siteStatusLabel(revision.deploymentStatus),
            ],
          ),
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Hash']),
          h.div(
            [
              Ui.className<Message>(
                'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm/6 text-white/55',
              ),
            ],
            [revision.sourceHash ?? 'Not recorded'],
          ),
        ],
      ),
      revision.url === null
        ? h.span(
            [Ui.className<Message>('text-sm/6 text-white/45')],
            ['No URL for this revision'],
          )
        : h.a(
            [
              h.Href(revision.url),
              h.Target('_blank'),
              h.Rel('noreferrer'),
              Ui.className<Message>(
                'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm/6 text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
              ),
            ],
            [revision.url],
          ),
    ],
  )
}

const artifactMeta = (
  artifact: CustomerFulfillmentArtifact,
): ReadonlyArray<string> =>
  [
    artifact.repositoryFullName === null
      ? null
      : `Repository ${artifact.repositoryFullName}`,
    artifact.sourceBranch === null ? null : `Source ${artifact.sourceBranch}`,
    artifact.targetBranch === null ? null : `Target ${artifact.targetBranch}`,
    artifact.commitSha === null ? null : `Commit ${artifact.commitSha}`,
  ].filter((value): value is string => value !== null)

const artifactRow = (artifact: CustomerFulfillmentArtifact): Html => {
  const h = html<Message>()
  const meta = artifactMeta(artifact)

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Message>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [artifactKindLabel(artifact.kind)],
          ),
          h.span(
            [
              Ui.className<Message>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [artifactStatusLabel(artifact.status)],
          ),
          h.span(
            [Ui.className<Message>('text-xs text-white/40')],
            [formatIsoDateTime(artifact.createdAt)],
          ),
        ],
      ),
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h3(
          [Ui.className<Message>('m-0 text-base/7 font-medium text-white/85')],
          [artifact.title],
        ),
        h.p(
          [
            Ui.className<Message>(
              'm-0 whitespace-pre-wrap text-base/7 text-white/65 sm:text-sm/6',
            ),
          ],
          [artifact.summary],
        ),
      ]),
      meta.length === 0
        ? h.span([Ui.className<Message>('sr-only')], ['No artifact metadata'])
        : h.div(
            [Ui.className<Message>('flex flex-wrap gap-2 text-xs text-white/45')],
            meta.map(value =>
              h.span([Ui.className<Message>('border border-[#222] px-2 py-1')], [
                value,
              ]),
            ),
          ),
      artifact.url === null
        ? h.span(
            [Ui.className<Message>('text-sm/6 text-white/45')],
            ['No review link yet'],
          )
        : h.a(
            [
              h.Href(artifact.url),
              h.Target('_blank'),
              h.Rel('noreferrer'),
              Ui.className<Message>(
                'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm/6 text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
              ),
            ],
            [artifact.url],
          ),
    ],
  )
}

const feedbackRow = (feedback: CustomerSiteFeedback): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Message>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [feedbackStatusLabel(feedback.status)],
          ),
          h.span(
            [Ui.className<Message>('text-xs text-white/40')],
            [formatIsoDateTime(feedback.createdAt)],
          ),
        ],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 whitespace-pre-wrap text-base/7 text-white/75 sm:text-sm/6',
          ),
        ],
        [feedback.body],
      ),
    ],
  )
}

const currentRevisionFrom = (
  revisions: ReadonlyArray<CustomerSiteRevision>,
): CustomerSiteRevision | null =>
  revisions.find(revision => revision.active) ?? revisions[0] ?? null

const currentRevisionForModel = (model: Model): CustomerSiteRevision | null =>
  model.customerSiteRevisions._tag === 'CustomerSiteRevisionsLoaded'
    ? currentRevisionFrom(model.customerSiteRevisions.revisions)
    : null

const siteAuthoringStageRow = (
  label: string,
  value: string,
  tone: 'accent' | 'positive' | 'warning' | 'negative' | 'info',
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] py-3 last:border-b-0 sm:grid-cols-[7rem_minmax(0,1fr)] sm:gap-4',
      ),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], [label]),
      h.div(
        [Ui.className<Message>('flex min-w-0 items-start gap-2 text-sm/6')],
        [
          h.span([Ui.className<Message>(`${Ui.statusDotClass(tone)} mt-2`)], []),
          h.span(
            [
              Ui.className<Message>(
                'min-w-0 overflow-hidden text-ellipsis text-white/70',
              ),
            ],
            [value],
          ),
        ],
      ),
    ],
  )
}

const siteAuthoringGenerationStage = (
  state: Model['customerSiteBuilderSession'],
): Readonly<{
  tone: 'accent' | 'positive' | 'warning' | 'negative' | 'info'
  value: string
}> =>
  M.value(state).pipe(
    M.tagsExhaustive({
      CustomerSiteBuilderSessionIdle: () => ({
        tone: 'accent' as const,
        value: 'Ready',
      }),
      CustomerSiteBuilderSessionLoading: () => ({
        tone: 'info' as const,
        value: 'Generating',
      }),
      CustomerSiteBuilderSessionFailed: () => ({
        tone: 'negative' as const,
        value: 'Blocked',
      }),
      CustomerSiteBuilderSessionLoaded: ({ session }) =>
        M.value(session.status).pipe(
          M.when('draft', () => ({
            tone: 'accent' as const,
            value: 'Draft',
          })),
          M.when('planning', () => ({
            tone: 'info' as const,
            value: 'Planning',
          })),
          M.when('building', () => ({
            tone: 'info' as const,
            value: 'Building',
          })),
          M.when('preview_ready', () => ({
            tone: 'positive' as const,
            value: 'Preview ready',
          })),
          M.when('review_ready', () => ({
            tone: 'positive' as const,
            value: 'Review ready',
          })),
          M.when('saved', () => ({
            tone: 'positive' as const,
            value: 'Saved',
          })),
          M.when('deploying', () => ({
            tone: 'info' as const,
            value: 'Publishing',
          })),
          M.when('deployed', () => ({
            tone: 'positive' as const,
            value: 'Published',
          })),
          M.when('failed', () => ({
            tone: 'negative' as const,
            value: 'Failed',
          })),
          M.when('archived', () => ({
            tone: 'warning' as const,
            value: 'Archived',
          })),
          M.exhaustive,
        ),
    }),
  )

const siteAuthoringApprovalStage = (
  model: Model,
  order: CustomerOrder,
): Readonly<{
  tone: 'accent' | 'positive' | 'warning' | 'negative' | 'info'
  value: string
}> => {
  const revision = currentRevisionForModel(model)

  if (order.site === null) {
    return { tone: 'accent', value: 'Waiting for Site' }
  }

  if (order.site.activeUrl !== null) {
    return { tone: 'positive', value: 'Approved' }
  }

  if (revision === null) {
    return order.site.latestSavedVersionId === null
      ? { tone: 'accent', value: 'Waiting for saved revision' }
      : { tone: 'warning', value: 'Review required' }
  }

  return M.value(revision.reviewState).pipe(
    M.when('runtime_verified', () => ({
      tone: 'positive' as const,
      value: 'Runtime verified',
    })),
    M.when('customer_accepted', () => ({
      tone: 'positive' as const,
      value: 'Customer accepted',
    })),
    M.when('customer_review_ready', () => ({
      tone: 'warning' as const,
      value: 'Customer review ready',
    })),
    M.when('internal_draft', () => ({
      tone: 'accent' as const,
      value: 'Internal draft',
    })),
    M.exhaustive,
  )
}

const siteAuthoringPublishStage = (
  order: CustomerOrder,
): Readonly<{
  tone: 'accent' | 'positive' | 'warning' | 'negative' | 'info'
  value: string
}> => {
  if (order.site === null) {
    return { tone: 'accent', value: 'Waiting for Site' }
  }

  if (order.site.activeUrl !== null) {
    return { tone: 'positive', value: 'Live on native runtime' }
  }

  if (order.site.activeDeploymentId !== null) {
    return { tone: 'info', value: 'Deployment recorded' }
  }

  if (order.site.latestSavedVersionId !== null) {
    return { tone: 'warning', value: 'Operator review required' }
  }

  return { tone: 'accent', value: 'Waiting for reviewable version' }
}

const siteAuthoringApprovalGate = (model: Model, order: CustomerOrder): string => {
  const approval = siteAuthoringApprovalStage(model, order)

  return approval.tone === 'positive' ? 'Recorded' : 'Required'
}

const siteAuthoringOutputBadge = (label: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      Ui.className<Message>(
        'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
      ),
    ],
    [label],
  )
}

const revisionLiveSummary = (
  current: CustomerSiteRevision,
  revisions: ReadonlyArray<CustomerSiteRevision>,
): string => {
  const deployment =
    current.deploymentStatus === null
      ? 'not deployed'
      : siteStatusLabel(current.deploymentStatus)
  const review = revisionReviewStateLabel(current.reviewState)
  const latestActive = current.active && revisions[0]?.id === current.id

  return latestActive
    ? `Latest revision live / ${review}`
    : `${review} / ${deployment}`
}

const revisionFollowUpDraft = (revision: CustomerSiteRevision): string =>
  `Follow up on ${revision.id}: `

const revisionsBody = (model: Model): ReadonlyArray<Html | string> => {
  const h = html<Message>()

  return M.value(model.customerSiteRevisions).pipe(
    M.tagsExhaustive({
      CustomerSiteRevisionsIdle: () => ['No revisions loaded yet.'],
      CustomerSiteRevisionsLoading: () => ['Loading revisions...'],
      CustomerSiteRevisionsFailed: ({ error }) => [error],
      CustomerSiteRevisionsLoaded: ({ revisions }) => {
        if (revisions.length === 0) {
          return ['No Site revisions yet.']
        }

        const current = currentRevisionFrom(revisions)

        return [
          ...(current === null
            ? []
            : [
                h.div(
                  [
                    Ui.className<Message>(
                      'grid gap-2 border border-[#333] bg-black p-3',
                    ),
                  ],
                  [
                    h.div(
                      [Ui.className<Message>(Ui.eyebrowClass)],
                      ['Latest revision'],
                    ),
                    h.div(
                      [Ui.className<Message>('text-sm/6 text-white/75')],
                      [revisionLiveSummary(current, revisions)],
                    ),
                  ],
                ),
              ]),
          h.div(
            [Ui.className<Message>('border-y border-[#222]')],
            [...revisions.slice(0, 5).map(revisionRow)],
          ),
        ]
      },
    }),
  )
}

const revisionHistoryRow = (
  revision: CustomerSiteRevision,
  index: number,
): Html => {
  const h = html<Message>()
  const originCreatedAt =
    revision.originCreatedAt === null
      ? 'Origin time not recorded'
      : formatIsoDateTime(revision.originCreatedAt)

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-3 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Message>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [revision.active ? 'Current' : `Revision ${index + 1}`],
          ),
          h.span(
            [
              Ui.className<Message>(
                'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/60',
              ),
            ],
            [revision.id],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-1 text-sm/6 text-white/65')],
        [
          h.div(
            [Ui.className<Message>('grid grid-cols-[5.5rem_minmax(0,1fr)] gap-2')],
            [
              h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Created']),
              h.span([Ui.className<Message>('min-w-0')], [
                formatIsoDateTime(revision.createdAt),
              ]),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid grid-cols-[5.5rem_minmax(0,1fr)] gap-2')],
            [
              h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Origin']),
              h.span([Ui.className<Message>('min-w-0')], [
                revision.originSummary ?? 'Origin not recorded',
              ]),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid grid-cols-[5.5rem_minmax(0,1fr)] gap-2')],
            [
              h.span([Ui.className<Message>(Ui.eyebrowClass)], ['When']),
              h.span([Ui.className<Message>('min-w-0')], [originCreatedAt]),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid grid-cols-[5.5rem_minmax(0,1fr)] gap-2')],
            [
              h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Status']),
              h.span([Ui.className<Message>('min-w-0')], [
                `${revisionReviewStateLabel(revision.reviewState)} / ${siteStatusLabel(revision.buildStatus)}`,
              ]),
            ],
          ),
        ],
      ),
      revision.url === null
        ? h.span(
            [Ui.className<Message>('text-sm/6 text-white/45')],
            ['No dedicated version URL'],
          )
        : h.a(
            [
              h.Href(revision.url),
              h.Target('_blank'),
              h.Rel('noreferrer'),
              Ui.className<Message>(
                'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm/6 text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
              ),
            ],
            [revision.url],
          ),
      Ui.button<Message>({
        label: `Start follow-up from ${revision.id}`,
        size: 'sm',
        variant: 'secondary',
        attrs: [
          h.Type('button'),
          h.OnClick(
            UpdatedCustomerSiteFeedbackDraft({
              value: revisionFollowUpDraft(revision),
            }),
          ),
        ],
      }),
    ],
  )
}

const revisionHistoryBody = (model: Model): ReadonlyArray<Html | string> => {
  const h = html<Message>()

  return M.value(model.customerSiteRevisions).pipe(
    M.tagsExhaustive({
      CustomerSiteRevisionsIdle: () => ['No version history loaded yet.'],
      CustomerSiteRevisionsLoading: () => ['Loading version history...'],
      CustomerSiteRevisionsFailed: ({ error }) => [error],
      CustomerSiteRevisionsLoaded: ({ revisions }) =>
        revisions.length === 0
          ? ['No version history yet.']
          : [
              h.div(
                [Ui.className<Message>('border-y border-[#222]')],
                revisions.slice(0, 8).map(revisionHistoryRow),
              ),
            ],
    }),
  )
}

const artifactsBody = (
  model: Model,
  emptyText: string,
): ReadonlyArray<Html | string> => {
  const h = html<Message>()

  return M.value(model.customerFulfillmentArtifacts).pipe(
    M.tagsExhaustive({
      CustomerFulfillmentArtifactsIdle: () => ['No deliverables loaded yet.'],
      CustomerFulfillmentArtifactsLoading: () => ['Loading deliverables...'],
      CustomerFulfillmentArtifactsFailed: ({ error }) => [error],
      CustomerFulfillmentArtifactsLoaded: ({ artifacts }) =>
        artifacts.length === 0
          ? [emptyText]
          : [
              h.div(
                [Ui.className<Message>('border-y border-[#222]')],
                [...artifacts.map(artifactRow)],
              ),
            ],
    }),
  )
}

const artifactsPanel = (model: Model, order: CustomerOrder): Html | null => {
  const h = html<Message>()

  if (
    order.site !== null &&
    model.customerFulfillmentArtifacts._tag ===
      'CustomerFulfillmentArtifactsLoaded' &&
    model.customerFulfillmentArtifacts.artifacts.length === 0
  ) {
    return null
  }

  return h.div(
    [Ui.className<Message>('grid gap-2')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], [
        order.site === null ? 'Deliverables' : 'Other deliverables',
      ]),
      ...artifactsBody(
        model,
        order.site === null
          ? 'No review deliverables yet.'
          : 'No other deliverables yet.',
      ),
    ],
  )
}

const feedbackBody = (model: Model): ReadonlyArray<Html | string> => {
  const h = html<Message>()

  return M.value(model.customerSiteFeedback).pipe(
    M.tagsExhaustive({
      CustomerSiteFeedbackIdle: () => ['No feedback loaded yet.'],
      CustomerSiteFeedbackLoading: () => ['Loading feedback...'],
      CustomerSiteFeedbackFailed: ({ error }) => [error],
      CustomerSiteFeedbackLoaded: ({ feedback }) =>
        feedback.length === 0
          ? ['No follow-up comments yet.']
          : [
              h.div(
                [Ui.className<Message>('border-y border-[#222]')],
                [...feedback.map(feedbackRow)],
              ),
            ],
    }),
  )
}

const feedbackSubmitMessage = (model: Model): ReadonlyArray<Html | string> => {
  const h = html<Message>()

  return M.value(model.customerSiteFeedbackSubmit).pipe(
    M.tagsExhaustive({
      CustomerSiteFeedbackSubmitIdle: () => [],
      CustomerSiteFeedbackSubmitting: () => [
        h.p(
          [Ui.className<Message>('m-0 text-sm/6 text-white/45')],
          ['Submitting...'],
        ),
      ],
      CustomerSiteFeedbackSubmitSucceeded: () => [
        h.p(
          [Ui.className<Message>('m-0 text-sm/6 text-[#00c853]')],
          ['Follow-up submitted.'],
        ),
      ],
      CustomerSiteFeedbackSubmitFailed: ({ error }) => [
        h.p([Ui.className<Message>('m-0 text-sm/6 text-[#d32f2f]')], [error]),
      ],
    }),
  )
}

const revisionLoopPanel = (model: Model, order: CustomerOrder): Html => {
  const h = html<Message>()
  const submitting =
    model.customerSiteFeedbackSubmit._tag === 'CustomerSiteFeedbackSubmitting'
  const deliverables = artifactsPanel(model, order)

  return h.section(
    [Ui.className<Message>('grid gap-4 border border-[#333] bg-[#080808] p-4')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Revisions']),
          order.site === null
            ? h.span(
                [Ui.className<Message>('text-sm/6 text-white/55')],
                ['Follow-ups are queued for the next deliverable.'],
              )
            : h.span(
                [Ui.className<Message>('text-sm/6 text-white/55')],
                [
                  `${order.site.feedbackCount} comments / ${order.site.openFeedbackCount} open`,
                ],
              ),
        ],
      ),
      ...(order.site === null ? [] : revisionsBody(model)),
      ...(deliverables === null ? [] : [deliverables]),
      h.form(
        [
          h.OnSubmit(SubmittedCustomerSiteFeedback({ orderId: order.id })),
          Ui.className<Message>('grid gap-3'),
        ],
        [
          h.label(
            [Ui.className<Message>('grid gap-2')],
            [
              h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Follow-up']),
              h.textarea(
                [
                  h.AriaLabel('Follow-up'),
                  h.Name('followUp'),
                  h.Rows(4),
                  h.Placeholder('Describe the adjustment you want next'),
                  h.Value(model.customerSiteFeedbackDraft),
                  h.OnInput(value =>
                    UpdatedCustomerSiteFeedbackDraft({ value }),
                  ),
                  ...(submitting ? [h.Disabled(true)] : []),
                  Ui.className<Message>(
                    `${Ui.inputClass} min-h-28 resize-y leading-6 max-sm:text-base`,
                  ),
                ],
                [],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              Ui.button<Message>({
                label: submitting ? 'Submitting' : 'Send follow-up',
                size: 'sm',
                variant: 'primary',
                attrs: [
                  h.Type('submit'),
                  ...(submitting ? [h.Disabled(true)] : []),
                ],
              }),
              ...feedbackSubmitMessage(model),
            ],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Feedback']),
          ...feedbackBody(model),
        ],
      ),
    ],
  )
}

const builderPhaseRow = (phase: CustomerSiteBuilderPhase): Html => {
  const h = html<Message>()
  const tone = builderPhaseTone(phase.status)

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        h.span([Ui.className<Message>(Ui.statusDotClass(tone))], []),
        h.span([Ui.className<Message>('text-sm/6 text-white/80')], [
          phase.title,
        ]),
        h.span(
          [Ui.className<Message>('text-xs uppercase text-white/40')],
          [builderPhaseStatusLabel(phase.status)],
        ),
      ]),
      h.p([Ui.className<Message>('m-0 text-sm/6 text-white/55')], [
        phase.summary,
      ]),
    ],
  )
}

const builderEventRow = (event: CustomerSiteBuilderEvent): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        h.span(
          [
            Ui.className<Message>(
              'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
            ),
          ],
          [event.eventKind.replaceAll('_', ' ')],
        ),
        h.span([Ui.className<Message>('text-xs text-white/40')], [
          formatIsoDateTime(event.createdAt),
        ]),
      ]),
      h.h3([Ui.className<Message>('m-0 text-sm/6 font-medium text-white/75')], [
        event.title,
      ]),
      h.p([Ui.className<Message>('m-0 text-sm/6 text-white/55')], [
        event.summary,
      ]),
    ],
  )
}

const builderMessageRow = (
  message: CustomerSiteBuilderSession['messages'][number],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        h.span(
          [
            Ui.className<Message>(
              'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
            ),
          ],
          [builderActorLabel(message.actorKind)],
        ),
        h.span([Ui.className<Message>('text-xs text-white/40')], [
          formatIsoDateTime(message.createdAt),
        ]),
      ]),
      h.p(
        [
          Ui.className<Message>(
            'm-0 whitespace-pre-wrap text-sm/6 text-white/70',
          ),
        ],
        [message.body],
      ),
    ],
  )
}

const builderFileButton = (
  sessionId: string,
  selectedPath: string | null,
  file: CustomerSiteBuilderFile | CustomerSiteBuilderFileTreeItem,
): Html => {
  const h = html<Message>()
  const selected = selectedPath === file.path

  return Ui.button<Message>({
    label: file.path,
    size: 'sm',
    variant: selected ? 'primary' : 'secondary',
    attrs: [
      h.Type('button'),
      h.OnClick(SelectedCustomerSiteBuilderFile({ path: file.path, sessionId })),
    ],
  })
}

const builderFileReadBody = (model: Model): ReadonlyArray<Html | string> => {
  const h = html<Message>()

  return M.value(model.customerSiteBuilderFileRead).pipe(
    M.tagsExhaustive({
      CustomerSiteBuilderFileReadIdle: () => ['Select a generated file.'],
      CustomerSiteBuilderFileReadLoading: ({ path }) => [
        `Loading ${path}...`,
      ],
      CustomerSiteBuilderFileReadFailed: ({ error }) => [error],
      CustomerSiteBuilderFileReadLoaded: ({ file }) => [
        h.div([Ui.className<Message>('grid gap-2')], [
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap items-center justify-between gap-2',
              ),
            ],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], [file.path]),
              h.span([Ui.className<Message>('text-xs text-white/40')], [
                `${file.byteSize} bytes`,
              ]),
            ],
          ),
          h.pre(
            [
              Ui.className<Message>(
                'max-h-80 overflow-auto border border-[#222] bg-[#050505] p-3 text-xs/5 text-white/75',
              ),
            ],
            [file.previewText ?? 'No customer-visible preview text recorded.'],
          ),
        ]),
      ],
    }),
  )
}

const builderFilesBody = (
  model: Model,
  sessionId: string,
): ReadonlyArray<Html | string> => {
  const h = html<Message>()

  return M.value(model.customerSiteBuilderFiles).pipe(
    M.tagsExhaustive({
      CustomerSiteBuilderFilesIdle: () => ['No generated files loaded yet.'],
      CustomerSiteBuilderFilesLoading: () => ['Loading generated files...'],
      CustomerSiteBuilderFilesFailed: ({ error }) => [error],
      CustomerSiteBuilderFilesLoaded: ({ files, fileTree }) =>
        files.length === 0
          ? ['No generated files recorded yet.']
          : [
              h.div([Ui.className<Message>('grid gap-3')], [
                h.div(
                  [
                    Ui.className<Message>(
                      'flex flex-wrap gap-2 border-y border-[#222] py-3',
                    ),
                  ],
                  fileTree.map(file =>
                    builderFileButton(
                      sessionId,
                      model.customerSiteBuilderSelectedFilePath,
                      file,
                    ),
                  ),
                ),
                ...builderFileReadBody(model),
              ]),
            ],
    }),
  )
}

const builderEventsBody = (
  model: Model,
): ReadonlyArray<Html | string> =>
  M.value(model.customerSiteBuilderEvents).pipe(
    M.tagsExhaustive({
      CustomerSiteBuilderEventsIdle: () => ['No event stream loaded yet.'],
      CustomerSiteBuilderEventsLoading: () => ['Loading event stream...'],
      CustomerSiteBuilderEventsFailed: ({ error }) => [error],
      CustomerSiteBuilderEventsLoaded: ({ events }) =>
        events.length === 0
          ? ['No customer-visible events yet.']
          : [
              html<Message>().div(
                [Ui.className<Message>('border-y border-[#222]')],
                events.slice(-8).map(builderEventRow),
              ),
            ],
    }),
  )

const latestBuilderEventCursor = (model: Model): number | undefined =>
  M.value(model.customerSiteBuilderEvents).pipe(
    M.tags({
      CustomerSiteBuilderEventsLoaded: ({ events }) =>
        events.reduce(
          (cursor, event) => Math.max(cursor, event.sequence),
          0,
        ),
    }),
    M.orElse(() => undefined),
  )

const siteAuthoringPanel = (model: Model, order: CustomerOrder): Html | null => {
  const h = html<Message>()

  if (order.site === null) {
    return null
  }

  const opening =
    model.customerSiteBuilderSession._tag ===
    'CustomerSiteBuilderSessionLoading'
  const promptSummary = siteAuthoringPromptSummary(model, order)
  const generation = siteAuthoringGenerationStage(
    model.customerSiteBuilderSession,
  )
  const approval = siteAuthoringApprovalStage(model, order)
  const publish = siteAuthoringPublishStage(order)

  return h.section(
    [
      h.DataAttribute('component', 'customer-site-authoring-panel'),
      Ui.className<Message>('grid gap-4 border border-[#333] bg-[#080808] p-4'),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-start justify-between gap-3',
          ),
        ],
        [
          h.div([Ui.className<Message>('grid gap-2')], [
            h.div([Ui.className<Message>(Ui.eyebrowClass)], [
              'Site authoring',
            ]),
            h.div(
              [Ui.className<Message>('flex flex-wrap gap-2')],
              [
                siteAuthoringOutputBadge('Landing'),
                siteAuthoringOutputBadge('Funnel'),
                siteAuthoringOutputBadge('Thank-you'),
              ],
            ),
          ]),
          Ui.badge<Message>({
            label: publish.value,
            tone: publish.tone,
          }),
        ],
      ),
      h.form(
        [
          h.OnSubmit(
            RequestedOpenCustomerSiteBuilderSession({
              orderId: order.id,
              promptSummary,
              siteId: order.site.id,
            }),
          ),
          Ui.className<Message>('grid gap-3'),
        ],
        [
          h.label(
            [Ui.className<Message>('grid gap-2')],
            [
              h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Site brief']),
              h.textarea(
                [
                  h.AriaLabel('Site brief'),
                  h.Name('siteBrief'),
                  h.Rows(4),
                  h.Placeholder(builderPromptSummary(order)),
                  h.Value(model.customerSiteBuilderPromptDraft),
                  h.OnInput(value =>
                    UpdatedCustomerSiteBuilderPromptDraft({ value }),
                  ),
                  ...(opening ? [h.Disabled(true)] : []),
                  Ui.className<Message>(
                    `${Ui.inputClass} min-h-28 resize-y leading-6 max-sm:text-base`,
                  ),
                ],
                [],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              Ui.button<Message>({
                label: opening ? 'Generating' : 'Generate Site pass',
                size: 'sm',
                variant: 'primary',
                attrs: [
                  h.Type('submit'),
                  ...(opening ? [h.Disabled(true)] : []),
                ],
              }),
              Ui.button<Message>({
                label: 'Refresh Site',
                size: 'sm',
                variant: 'secondary',
                attrs: [
                  h.Type('button'),
                  h.OnClick(RequestedLoadCustomerOrder()),
                ],
              }),
            ],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('border-y border-[#222]')],
        [
          siteAuthoringStageRow('Prompt', promptSummary, 'accent'),
          siteAuthoringStageRow('Generate', generation.value, generation.tone),
          siteAuthoringStageRow('Approve', approval.value, approval.tone),
          siteAuthoringStageRow('Publish', publish.value, publish.tone),
        ],
      ),
      h.div(
        [Ui.className<Message>('border-y border-[#222]')],
        [
          progressDetailRow('Approval gate', [
            siteAuthoringApprovalGate(model, order),
          ]),
          progressDetailRow('Saved version', [
            order.site.latestSavedVersionId ?? 'No saved revision yet',
          ]),
          progressDetailRow('Deployment', [
            order.site.activeDeploymentId ?? 'No deployment yet',
          ]),
          progressDetailRow(
            'Live URL',
            order.site.activeUrl === null
              ? ['No live URL yet']
              : [
                  h.a(
                    [
                      h.Href(order.site.activeUrl),
                      h.Target('_blank'),
                      h.Rel('noreferrer'),
                      Ui.className<Message>(
                        'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
                      ),
                    ],
                    [order.site.activeUrl],
                  ),
                ],
          ),
        ],
      ),
    ],
  )
}

const builderSessionBody = (
  model: Model,
  order: CustomerOrder,
  session: CustomerSiteBuilderSession,
): ReadonlyArray<Html | string> => {
  const h = html<Message>()
  const activePreview = session.activePreview
  const eventCursor = latestBuilderEventCursor(model)

  return [
    h.div([Ui.className<Message>('border-y border-[#222]')], [
      progressDetailRow('Builder status', [
        builderSessionStatusLabel(session.status),
      ]),
      progressDetailRow('Next action', [builderNextAction(session)]),
      progressDetailRow('Prompt', [session.promptSummary]),
      progressDetailRow('Updated', [formatIsoDateTime(session.updatedAt)]),
      progressDetailRow(
        'Preview',
        activePreview?.previewUrl === undefined ||
          activePreview?.previewUrl === null
          ? [
              session.activePreviewId === null
                ? 'No preview recorded yet'
                : `Preview ${session.activePreviewId}`,
            ]
          : [
              h.a(
                [
                  h.Href(activePreview.previewUrl),
                  h.Target('_blank'),
                  h.Rel('noreferrer'),
                  Ui.className<Message>(
                    'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
                  ),
                ],
                [activePreview.previewUrl],
              ),
            ],
      ),
      progressDetailRow(
        'Live Site',
        order.site?.activeUrl === null || order.site?.activeUrl === undefined
          ? ['No live URL yet']
          : [
              h.a(
                [
                  h.Href(order.site.activeUrl),
                  h.Target('_blank'),
                  h.Rel('noreferrer'),
                  Ui.className<Message>(
                    'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
                  ),
                ],
                [order.site.activeUrl],
              ),
            ],
      ),
    ]),
    h.div([Ui.className<Message>('grid gap-2')], [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Phase timeline']),
      session.phases.length === 0
        ? h.p([Ui.className<Message>('m-0 text-sm/6 text-white/45')], [
            'No phases recorded yet.',
          ])
        : h.div(
            [Ui.className<Message>('border-y border-[#222]')],
            session.phases.map(builderPhaseRow),
          ),
    ]),
    h.div([Ui.className<Message>('grid gap-2')], [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-2',
          ),
        ],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Event stream']),
          Ui.button<Message>({
            label: 'Refresh events',
            size: 'sm',
            variant: 'secondary',
            attrs: [
              h.Type('button'),
              h.OnClick(
                eventCursor === undefined
                  ? RequestedLoadCustomerSiteBuilderEvents({
                      sessionId: session.id,
                    })
                  : RequestedLoadCustomerSiteBuilderEvents({
                      cursor: eventCursor,
                      sessionId: session.id,
                    }),
              ),
            ],
          }),
        ],
      ),
      ...builderEventsBody(model),
    ]),
    h.div([Ui.className<Message>('grid gap-2')], [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Generated files']),
      ...builderFilesBody(model, session.id),
    ]),
    h.div([Ui.className<Message>('grid gap-2')], [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Messages']),
      session.messages.length === 0
        ? h.p([Ui.className<Message>('m-0 text-sm/6 text-white/45')], [
            'No messages recorded yet.',
          ])
        : h.div(
            [Ui.className<Message>('border-y border-[#222]')],
            session.messages.slice(-8).map(builderMessageRow),
          ),
    ]),
  ]
}

const siteBuilderPanel = (model: Model, order: CustomerOrder): Html | null => {
  const h = html<Message>()

  if (order.site === null) {
    return null
  }

  const opening =
    model.customerSiteBuilderSession._tag ===
    'CustomerSiteBuilderSessionLoading'

  return h.section(
    [
      h.DataAttribute('component', 'customer-site-builder-panel'),
      Ui.className<Message>('grid gap-4 border border-[#333] bg-[#080808] p-4'),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-start justify-between gap-3',
          ),
        ],
        [
          h.div([Ui.className<Message>('grid gap-1')], [
            h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Site builder']),
            h.p([Ui.className<Message>('m-0 text-sm/6 text-white/55')], [
              'Inspect the current builder run, generated files, previews, and follow-up queue from one place.',
            ]),
          ]),
          Ui.button<Message>({
            label: opening ? 'Opening builder' : 'Start or reconnect builder',
            size: 'sm',
            variant: 'primary',
            attrs: [
              h.Type('button'),
              ...(opening ? [h.Disabled(true)] : []),
              h.OnClick(
                RequestedOpenCustomerSiteBuilderSession({
                  orderId: order.id,
                  promptSummary: builderPromptSummary(order),
                  siteId: order.site.id,
                }),
              ),
            ],
          }),
        ],
      ),
      ...M.value(model.customerSiteBuilderSession).pipe(
        M.tagsExhaustive({
          CustomerSiteBuilderSessionIdle: () => [
            h.p([Ui.className<Message>('m-0 text-sm/6 text-white/45')], [
              'No builder session is open in this browser yet.',
            ]),
          ],
          CustomerSiteBuilderSessionLoading: () => [
            h.p([Ui.className<Message>('m-0 text-sm/6 text-white/45')], [
              'Opening the builder session...',
            ]),
          ],
          CustomerSiteBuilderSessionFailed: ({ error }) => [
            h.p([Ui.className<Message>('m-0 text-sm/6 text-[#d32f2f]')], [
              error,
            ]),
          ],
          CustomerSiteBuilderSessionLoaded: ({ session }) =>
            builderSessionBody(model, order, session),
        }),
      ),
    ],
  )
}

const siteEditorSidebar = (model: Model, order: CustomerOrder): Html => {
  const h = html<Message>()
  const selectedContext = model.customerSiteElementContext
  const revisionVersionRef =
    model.customerSiteRevisions._tag === 'CustomerSiteRevisionsLoaded'
      ? (currentRevisionFrom(model.customerSiteRevisions.revisions)?.id ??
        order.site?.latestSavedVersionId ??
        order.site?.activeVersionId ??
        'version not recorded')
      : (order.site?.latestSavedVersionId ??
        order.site?.activeVersionId ??
        'version not recorded')
  const codeViewerContext =
    selectedContext === null
      ? null
      : siteCodeViewerContextFromElement(selectedContext, revisionVersionRef)

  return h.details(
    [
      h.Attribute('open', ''),
      h.DataAttribute('component', 'site-editor-sidebar'),
      h.DataAttribute('sidebar-width-px', String(SITE_EDITOR_SIDEBAR_WIDTH_PX)),
      Ui.className<Message>(
        'group grid gap-3 border border-[#333] bg-[#080808] p-4 lg:sticky lg:top-4 lg:self-start',
      ),
    ],
    [
      h.summary(
        [
          h.DataAttribute('component', 'site-editor-sidebar-trigger'),
          Ui.className<Message>(
            'grid cursor-pointer list-none grid-cols-[minmax(0,1fr)_auto] items-center gap-3 text-white/80 outline-none focus-visible:ring-2 focus-visible:ring-[#ffb400]/70 [&::-webkit-details-marker]:hidden',
          ),
        ],
        [
          h.span([Ui.className<Message>(Ui.eyebrowClass)], ['Site editor']),
          h.span(
            [
              Ui.className<Message>(
                'text-xs uppercase text-white/35 group-open:hidden',
              ),
            ],
            ['Open'],
          ),
          h.span(
            [
              Ui.className<Message>(
                'hidden text-xs uppercase text-white/35 group-open:inline',
              ),
            ],
            ['Expanded'],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('hidden gap-3 group-open:grid')],
        [
          h.div(
            [Ui.className<Message>('border-y border-[#222]')],
            [
              progressDetailRow('Mode', ['Review']),
              progressDetailRow('Panel width', [
                `${SITE_EDITOR_SIDEBAR_WIDTH_PX}px`,
              ]),
              progressDetailRow('Site', [
                order.site?.latestSavedVersionId ?? 'No saved revision yet',
              ]),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], [
                'Version history',
              ]),
              ...revisionHistoryBody(model),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], [
                'Inspect mode',
              ]),
              selectedContext === null
                ? h.p(
                    [
                      Ui.className<Message>(
                        'm-0 text-sm/6 text-white/45',
                      ),
                    ],
                    ['No element selected'],
                  )
                : h.div(
                    [
                      Ui.className<Message>(
                        'grid gap-1 border border-[#222] bg-black p-3',
                      ),
                    ],
                    [
                      h.div([Ui.className<Message>(Ui.eyebrowClass)], [
                        'Selected element',
                      ]),
                      h.code(
                        [
                          Ui.className<Message>(
                            'break-words text-xs/5 text-white/70',
                          ),
                        ],
                        [selectedContext.htmlSnippet],
                      ),
                      h.div(
                        [
                          Ui.className<Message>(
                            'break-words text-xs/5 text-white/45',
                          ),
                        ],
                        [selectedContext.selector],
                      ),
                    ],
                  ),
              h.div(
                [Ui.className<Message>('flex flex-wrap gap-2')],
                siteElementTargets.map(context =>
                  Ui.button<Message>({
                    label: `Target ${context.text ?? context.tag}`,
                    size: 'sm',
                    variant: 'secondary',
                    attrs: [
                      h.Type('button'),
                      h.OnClick(
                        SelectedCustomerSiteElementContext({ context }),
                      ),
                    ],
                  }),
                ),
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], [
                'Code viewer',
              ]),
              codeViewerContext === null
                ? h.p(
                    [
                      Ui.className<Message>(
                        'm-0 text-sm/6 text-white/45',
                      ),
                    ],
                    ['Select an element to view source context'],
                  )
                : h.div(
                    [
                      Ui.className<Message>(
                        'grid gap-3 border border-[#222] bg-black p-3',
                      ),
                    ],
                    [
                      progressDetailRow('Path', [codeViewerContext.path]),
                      progressDetailRow('Version', [
                        codeViewerContext.versionRef,
                      ]),
                      progressDetailRow('Language', [
                        codeViewerContext.language,
                      ]),
                      h.pre(
                        [
                          Ui.className<Message>(
                            'max-h-56 overflow-auto border border-[#222] bg-[#050505] p-3 text-xs/5 text-white/70',
                          ),
                        ],
                        [codeViewerContext.source],
                      ),
                      Ui.button<Message>({
                        label: 'Copy snippet',
                        size: 'sm',
                        variant: 'secondary',
                        attrs: [
                          h.Type('button'),
                          h.DataAttribute('copy-text', codeViewerContext.source),
                          h.Attribute(
                            'onclick',
                            "navigator.clipboard?.writeText(this.dataset.copyText || '');",
                          ),
                        ],
                      }),
                    ],
                  ),
            ],
          ),
        ],
      ),
    ],
  )
}

const siteEditorShell = (model: Model, order: CustomerOrder): Html => {
  const h = html<Message>()
  const bridgeOrigin = sitePreviewBridgeOrigin(order)

  if (order.site === null) {
    return revisionLoopPanel(model, order)
  }

  return h.section(
    [
      h.DataAttribute('component', 'site-editor-shell'),
      ...(bridgeOrigin === null
        ? []
        : [
            h.OnMount(
              InstallSitePreviewElementTargetBridge({
                allowedOrigin: bridgeOrigin,
              }),
            ),
          ]),
      Ui.className<Message>(
        'grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_21rem]',
      ),
    ],
    [revisionLoopPanel(model, order), siteEditorSidebar(model, order)],
  )
}

const usagePanel = (order: CustomerOrder): Html | null => {
  const h = html<Message>()

  if (order.usageReceipts.length === 0) {
    return null
  }

  return h.section(
    [Ui.className<Message>('grid gap-3 border border-[#333] bg-[#080808] p-4')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Usage']),
      h.div(
        [Ui.className<Message>('border-y border-[#222]')],
        [
          progressDetailRow('Billing', [
            billingModeLabel(order.usageSummary.billingMode),
          ]),
          progressDetailRow('Total', [
            order.usageSummary.totalCreditsChargedFormatted,
          ]),
          ...order.usageSummary.categories.map(category =>
            progressDetailRow(categoryLabel(category.category), [
              `${quantityLabel(category.quantity, category.unit)} / ${category.creditsChargedFormatted}`,
            ]),
          ),
        ],
      ),
    ],
  )
}

const triagePanel = (order: CustomerOrder): Html | null => {
  const h = html<Message>()

  if (order.triage === null) {
    return null
  }

  return h.section(
    [Ui.className<Message>('grid gap-3 border border-[#333] bg-[#080808] p-4')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Review']),
      h.div(
        [Ui.className<Message>('border-y border-[#222]')],
        [
          progressDetailRow('Status', [userFacingCopy(order.triage.status)]),
          progressDetailRow('Summary', [userFacingCopy(order.triage.summary)]),
          progressDetailRow('Next action', [
            userFacingCopy(order.triage.nextAction),
          ]),
        ],
      ),
    ],
  )
}

const progressPanel = (order: CustomerOrder): Html => {
  const h = html<Message>()
  const tone = adjutantStageTone(order.adjutant.stage)
  const activeUrl = order.adjutant.activeUrl

  return h.section(
    [Ui.className<Message>('grid gap-4 border border-[#333] bg-[#080808] p-4')],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Autopilot']),
          h.div(
            [
              Ui.className<Message>(
                'inline-flex items-center gap-2 border border-[#333] px-2.5 py-1.5 text-xs uppercase text-white/70',
              ),
            ],
            [
              h.span([Ui.className<Message>(Ui.statusDotClass(tone))], []),
              adjutantStageLabel(order.adjutant.stage),
            ],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/70 sm:text-sm/6')],
        [userFacingCopy(order.adjutant.nextAction)],
      ),
      h.div(
        [Ui.className<Message>('border-y border-[#222]')],
        [
          progressDetailRow('Order status', [
            statusLabel(order.adjutant.orderStatus),
          ]),
          ...(order.adjutant.siteStatus === null
            ? []
            : [
                progressDetailRow('Site lifecycle', [
                  siteStatusLabel(order.adjutant.siteStatus),
                ]),
              ]),
          ...(order.adjutant.adjustmentStatus === null
            ? []
            : [
                progressDetailRow('Adjustment', [
                  siteStatusLabel(order.adjutant.adjustmentStatus),
                ]),
              ]),
          progressDetailRow(
            'Live URL',
            activeUrl === null
              ? ['No active URL yet']
              : [
                  h.a(
                    [
                      h.Href(activeUrl),
                      h.Target('_blank'),
                      h.Rel('noreferrer'),
                      Ui.className<Message>(
                        'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-white/80 underline underline-offset-[3px] hover:text-[#ffb400]',
                      ),
                    ],
                    [activeUrl],
                  ),
                ],
          ),
        ],
      ),
    ],
  )
}

const repositoryLink = (order: CustomerOrder): Html => {
  const h = html<Message>()

  if (order.repository === null) {
    return detailRow('Repository', 'Not selected')
  }

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#222] py-3 last:border-b-0 sm:grid-cols-[10rem_minmax(0,1fr)] sm:gap-4',
      ),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Repository']),
      h.a(
        [
          h.Href(order.repository.htmlUrl),
          h.Target('_blank'),
          h.Rel('noreferrer'),
          Ui.className<Message>(
            'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-base/7 text-white/80 underline underline-offset-[3px] hover:text-[#ffb400] sm:text-sm/6',
          ),
        ],
        [order.repository.fullName],
      ),
    ],
  )
}

const statusPanel = (model: Model, order: CustomerOrder): Html => {
  const h = html<Message>()
  const tone = statusTone(order.status)
  const email = model.session.email
  const site = sitePanel(order)
  const authoring = siteAuthoringPanel(model, order)
  const builder = siteBuilderPanel(model, order)
  const triage = triagePanel(order)
  const usage = usagePanel(order)

  return h.section(
    [Ui.className<Message>('grid gap-5 border border-[#222] bg-black p-5')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-start justify-between gap-4',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Order']),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 text-2xl font-semibold text-white/90',
                  ),
                ],
                ['Public software request'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-2xl text-base/7 text-white/55 sm:text-sm/6',
                  ),
                ],
                [statusBody(order)],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'inline-flex items-center gap-2 border border-[#333] px-2.5 py-1.5 text-xs uppercase text-white/70',
                  ),
                ],
                [
                  h.span([Ui.className<Message>(Ui.statusDotClass(tone))], []),
                  statusLabel(order.status),
                ],
              ),
              Ui.button<Message>({
                label: 'Refresh',
                size: 'sm',
                variant: 'secondary',
                attrs: [
                  h.Type('button'),
                  h.OnClick(RequestedLoadCustomerOrder()),
                ],
              }),
            ],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'border border-[#333] bg-[#080808] p-4 text-base/7 text-white/75 sm:text-sm/6',
          ),
        ],
        [
          `We'll email you at ${email} within 24 hours with your completed work.`,
        ],
      ),
      ...(triage === null ? [] : [triage]),
      progressPanel(order),
      ...(authoring === null ? [] : [authoring]),
      ...(builder === null ? [] : [builder]),
      siteEditorShell(model, order),
      ...(usage === null ? [] : [usage]),
      ...(site === null ? [] : [site]),
      h.div(
        [Ui.className<Message>('border-y border-[#222]')],
        [
          repositoryLink(order),
          detailRow('Email', email),
          detailRow('Visibility', 'Public'),
          detailRow('Compute', 'OpenAgents paid'),
          detailRow('Provider account', 'Not required'),
          detailRow('Free slice', money(order.freeSliceCents)),
          detailRow('Paid quote', money(order.quoteCents)),
          detailRow('Order ID', order.id),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Request']),
          h.p(
            [
              Ui.className<Message>(
                'm-0 whitespace-pre-wrap text-base/7 text-white/75 sm:text-sm/6',
              ),
            ],
            [order.request],
          ),
        ],
      ),
    ],
  )
}

const emptyOrder = (): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-4 border border-[#222] bg-black p-5')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Order']),
      h.h1(
        [Ui.className<Message>('m-0 text-2xl font-semibold text-white/90')],
        ['No active request'],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/55 sm:text-sm/6')],
        ['Finish onboarding to submit a public software request.'],
      ),
    ],
  )
}

const orderKindLabel = (order: CustomerOrder): string =>
  order.site === null ? 'Software request' : 'Site request'

const orderPrimaryHref = (order: CustomerOrder): string =>
  orderDetailRouter({ orderId: order.id })

const orderPrimaryLinkLabel = (order: CustomerOrder): string =>
  order.site?.activeUrl ?? orderPrimaryHref(order)

const orderDashboardCard = (order: CustomerOrder): Html => {
  const h = html<Message>()
  const href = orderPrimaryHref(order)

  return h.article(
    [
      Ui.className<Message>(
        'grid gap-4 border border-[#222] bg-black p-5 sm:grid-cols-[minmax(0,1fr)_auto]',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('grid min-w-0 gap-3')],
        [
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              h.span([Ui.className<Message>(Ui.eyebrowClass)], [
                orderKindLabel(order),
              ]),
              Ui.badge<Message>({
                label: statusLabel(order.status),
                tone: statusTone(order.status),
              }),
              Ui.badge<Message>({
                label: adjutantStageLabel(order.adjutant.stage),
                tone: adjutantStageTone(order.adjutant.stage),
              }),
            ],
          ),
          h.h2(
            [
              Ui.className<Message>(
                'm-0 line-clamp-2 text-xl font-semibold text-white/90',
              ),
            ],
            [order.request],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-base/7 text-white/55 sm:text-sm/6')],
            [order.adjutant.nextAction],
          ),
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap gap-x-4 gap-y-2 text-sm/6 text-white/45',
              ),
            ],
            [
              `Created ${formatIsoDateTime(order.createdAt)}`,
              order.repository === null
                ? 'No repository selected'
                : `Repository ${order.repository.fullName}`,
              order.site === null
                ? 'No Site yet'
                : `Site ${siteStatusLabel(order.site.status)}`,
            ],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'flex min-w-0 flex-col items-start gap-2 sm:items-end',
          ),
        ],
        [
          h.a(
            [
              h.Href(href),
              Ui.className<Message>(
                'inline-flex min-h-10 items-center border border-white/25 px-3 text-sm font-medium text-white/90 hover:border-white/50',
              ),
            ],
            ['Open request'],
          ),
          h.a(
            [
              h.Href(order.site?.activeUrl ?? href),
              Ui.className<Message>(
                'max-w-[18rem] truncate text-sm/6 text-white/55 underline underline-offset-4 hover:text-white/80',
              ),
            ],
            [orderPrimaryLinkLabel(order)],
          ),
        ],
      ),
    ],
  )
}

const customerOrderCreateStatus = (model: Model): Html | null => {
  const h = html<Message>()

  return M.value(model.customerOrderCreate).pipe(
    M.tags({
      CustomerOrderCreateSucceeded: ({ order }) =>
        h.p(
          [Ui.className<Message>('m-0 text-sm/6 text-[#00c853]')],
          [`Request created. Open ${order.id} to track the workstream.`],
        ),
      CustomerOrderCreateFailed: ({ error }) =>
        h.p([Ui.className<Message>('m-0 text-sm/6 text-[#d32f2f]')], [error]),
    }),
    M.orElse(() => null),
  )
}

const newCustomerOrderForm = (model: Model): Html => {
  const h = html<Message>()
  const submitting = model.customerOrderCreate._tag === 'CustomerOrderCreateSubmitting'

  return h.form(
    [
      Ui.className<Message>('grid gap-3 border border-[#222] bg-black p-5'),
      h.OnSubmit(SubmittedCustomerOrder()),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['New request']),
      h.label(
        [Ui.className<Message>('grid gap-2')],
        [
          h.span(
            [Ui.className<Message>('text-sm font-medium text-white/80')],
            ['Describe the software work'],
          ),
          h.textarea(
            [
              h.Name('request'),
              h.AriaLabel('New software request'),
              h.Value(model.customerOrderDraft),
              h.OnInput(value => UpdatedCustomerOrderDraft({ value })),
              h.Rows(5),
              Ui.className<Message>(
                'min-h-32 resize-y border border-[#333] bg-[#050505] p-3 text-base/7 text-white/85 outline-none focus:border-white/45 sm:text-sm/6',
              ),
            ],
            [],
          ),
        ],
      ),
      customerOrderCreateStatus(model),
      Ui.button<Message>({
        label: submitting ? 'Submitting...' : 'Submit request',
        size: 'sm',
        variant: 'primary',
        attrs: [h.Type('submit'), ...(submitting ? [h.Disabled(true)] : [])],
      }),
    ].filter((node): node is Html => node !== null),
  )
}

const customerOrdersDashboard = (
  model: Model,
  orders: ReadonlyArray<CustomerOrder>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-4')],
    [
      h.section(
        [
          Ui.className<Message>(
            'grid gap-3 border border-[#222] bg-black p-5 sm:grid-cols-[minmax(0,1fr)_auto]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Workstreams']),
              h.h1(
                [Ui.className<Message>('m-0 text-2xl font-semibold text-white/90')],
                ['Software requests'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-3xl text-base/7 text-white/55 sm:text-sm/6',
                  ),
                ],
                [
                  'Track each Site, pull request, preview, and follow-up as a separate workstream.',
                ],
              ),
            ],
          ),
          Ui.button<Message>({
            label: 'Refresh',
            size: 'sm',
            variant: 'secondary',
            attrs: [h.Type('button'), h.OnClick(RequestedLoadCustomerOrders())],
          }),
        ],
      ),
      newCustomerOrderForm(model),
      orders.length === 0
        ? h.section(
            [Ui.className<Message>('grid gap-3 border border-[#222] bg-black p-5')],
            [
              h.h2(
                [Ui.className<Message>('m-0 text-xl font-semibold text-white/85')],
                ['No requests yet'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-base/7 text-white/55 sm:text-sm/6')],
                ['Submit a request above to start a public software workstream.'],
              ),
            ],
          )
        : h.section(
            [Ui.className<Message>('grid gap-3')],
            orders.map(orderDashboardCard),
          ),
    ],
  )
}

const customerOrdersFailure = (error: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#333] bg-black p-5')],
    [
      h.p([Ui.className<Message>('m-0 text-base/7 text-[#d32f2f] sm:text-sm/6')], [
        error,
      ]),
      Ui.button<Message>({
        label: 'Retry',
        size: 'sm',
        variant: 'secondary',
        attrs: [h.Type('button'), h.OnClick(RequestedLoadCustomerOrders())],
      }),
    ],
  )
}

const customerOrdersView = (model: Model): Html => {
  const h = html<Message>()

  return M.value(model.customerOrders).pipe(
    M.tagsExhaustive({
      CustomerOrdersIdle: () =>
        h.div(
          [
            Ui.className<Message>(
              'border border-[#222] bg-black p-5 text-base/7 text-white/45 sm:text-sm/6',
            ),
          ],
          ['Loading requests...'],
        ),
      CustomerOrdersLoading: () =>
        h.div(
          [
            Ui.className<Message>(
              'border border-[#222] bg-black p-5 text-base/7 text-white/45 sm:text-sm/6',
            ),
          ],
          ['Loading requests...'],
        ),
      CustomerOrdersLoaded: ({ orders }) => customerOrdersDashboard(model, orders),
      CustomerOrdersFailed: ({ error }) => customerOrdersFailure(error),
    }),
  )
}

export const view = (model: Model): Html => {
  const h = html<Message>()

  return h.section(
    [
      Ui.className<Message>(
        'mx-auto grid min-h-[calc(100dvh-3rem)] w-[min(100%,76rem)] content-start gap-4 p-4 lg:p-8',
      ),
    ],
    [
      model.route._tag === 'Order'
        ? customerOrdersView(model)
        : M.value(model.customerOrder).pipe(
            M.tagsExhaustive({
              CustomerOrderIdle: () =>
                h.div(
                  [
                    Ui.className<Message>(
                      'border border-[#222] bg-black p-5 text-base/7 text-white/45 sm:text-sm/6',
                    ),
                  ],
                  ['Loading order...'],
                ),
              CustomerOrderLoading: () =>
                h.div(
                  [
                    Ui.className<Message>(
                      'border border-[#222] bg-black p-5 text-base/7 text-white/45 sm:text-sm/6',
                    ),
                  ],
                  ['Loading order...'],
                ),
              CustomerOrderLoaded: ({ order }) =>
                order === null ? emptyOrder() : statusPanel(model, order),
              CustomerOrderFailed: ({ error }) =>
                h.div(
                  [
                    Ui.className<Message>(
                      'grid gap-3 border border-[#333] bg-black p-5',
                    ),
                  ],
                  [
                    h.p(
                      [
                        Ui.className<Message>(
                          'm-0 text-base/7 text-[#d32f2f] sm:text-sm/6',
                        ),
                      ],
                      [error],
                    ),
                    Ui.button<Message>({
                      label: 'Retry',
                      size: 'sm',
                      variant: 'secondary',
                      attrs: [
                        h.Type('button'),
                        h.OnClick(RequestedLoadCustomerOrder()),
                      ],
                    }),
                  ],
                ),
            }),
          ),
    ],
  )
}
