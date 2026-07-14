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
  SubmittedCustomerOrder,
  UpdatedCustomerOrderDraft,
} from '../message'
import type {
  AdjutantUsageReceiptBillingMode,
  AdjutantUsageReceiptCategory,
  CustomerOrder,
  CustomerOrderAdjutantStage,
  CustomerOrderStatus,
  Model,
} from '../model'

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
    M.when('submitted', () => 'Your request is in the public beta intake queue.'),
    M.when('scoping', () => 'OpenAgents is scoping the first useful slice.'),
    M.when('free_slice_ready', () => 'A free public slice is ready for operator review.'),
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
      ...(usage === null ? [] : [usage]),
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
