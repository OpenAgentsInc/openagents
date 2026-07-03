import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { parseJsonRecord } from '../../../json-boundary'
import {
  ChatApiHttpError,
  errorFromUnknown,
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedLoadCustomerFulfillmentArtifacts,
  FailedLoadCustomerOrder,
  FailedLoadCustomerOrders,
  FailedLoadCustomerSiteBuilderEvents,
  FailedLoadCustomerSiteBuilderFile,
  FailedLoadCustomerSiteBuilderFiles,
  FailedLoadCustomerSiteBuilderSession,
  FailedLoadCustomerSiteFeedback,
  FailedLoadCustomerSiteRevisions,
  FailedOpenCustomerSiteBuilderSession,
  FailedSubmitCustomerOrder,
  FailedSubmitCustomerSiteFeedback,
  Message,
  SucceededLoadCustomerSiteBuilderEvents,
  SucceededLoadCustomerSiteBuilderFile,
  SucceededLoadCustomerSiteBuilderFiles,
  SucceededLoadCustomerSiteBuilderSession,
  SucceededLoadCustomerFulfillmentArtifacts,
  SucceededLoadCustomerOrder,
  SucceededLoadCustomerOrders,
  SucceededLoadCustomerSiteFeedback,
  SucceededLoadCustomerSiteRevisions,
  SucceededOpenCustomerSiteBuilderSession,
  SucceededSubmitCustomerOrder,
  SucceededSubmitCustomerSiteFeedback,
} from '../message'
import {
  CustomerSiteBuilderEvent,
  CustomerSiteBuilderEventsFailed,
  CustomerSiteBuilderEventsIdle,
  CustomerSiteBuilderEventsLoaded,
  CustomerSiteBuilderEventsLoading,
  CustomerSiteBuilderEventsResponse,
  CustomerSiteBuilderFileReadFailed,
  CustomerSiteBuilderFileReadIdle,
  CustomerSiteBuilderFileReadLoaded,
  CustomerSiteBuilderFileReadLoading,
  CustomerSiteBuilderFileReadResponse,
  CustomerSiteBuilderFilesFailed,
  CustomerSiteBuilderFilesIdle,
  CustomerSiteBuilderFilesLoaded,
  CustomerSiteBuilderFilesLoading,
  CustomerSiteBuilderFileListResponse,
  CustomerSiteBuilderFileTreeResponse,
  CustomerSiteBuilderSessionFailed,
  CustomerSiteBuilderSessionIdle,
  CustomerSiteBuilderSessionLoaded,
  CustomerSiteBuilderSessionLoading,
  CustomerSiteBuilderSessionResponse,
  CustomerFulfillmentArtifactsFailed,
  CustomerFulfillmentArtifactsIdle,
  CustomerFulfillmentArtifactsLoaded,
  CustomerFulfillmentArtifactsLoading,
  CustomerFulfillmentArtifactsResponse,
  CustomerOrder,
  CustomerOrderCreateFailed,
  CustomerOrderCreateSucceeded,
  CustomerOrderCreateSubmitting,
  CustomerOrderFailed,
  CustomerOrderLoaded,
  CustomerOrderLoading,
  CustomerOrderResponse,
  CustomerOrdersFailed,
  CustomerOrdersLoaded,
  CustomerOrdersLoading,
  CustomerOrdersResponse,
  CustomerSiteFeedback,
  CustomerSiteFeedbackFailed,
  CustomerSiteFeedbackIdle,
  CustomerSiteFeedbackLoaded,
  CustomerSiteFeedbackLoading,
  CustomerSiteFeedbackResponse,
  CustomerSiteFeedbackSubmitFailed,
  CustomerSiteFeedbackSubmitIdle,
  CustomerSiteFeedbackSubmitState,
  CustomerSiteFeedbackSubmitSucceeded,
  CustomerSiteFeedbackSubmitting,
  CustomerSiteRevisionsFailed,
  CustomerSiteRevisionsIdle,
  CustomerSiteRevisionsLoaded,
  CustomerSiteRevisionsLoading,
  CustomerSiteRevisionsResponse,
  Model,
  SubmitCustomerSiteFeedbackResponse,
} from '../model'
import { siteElementContextDraft } from '../site-element-context'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const customerOrderIdForRoute = (model: Model): string | null =>
  model.route._tag === 'OrderDetail' ? model.route.orderId : null

const customerOrderRequestPath = (orderId: string | null): string =>
  orderId === null
    ? '/api/customer-orders/active'
    : `/api/customer-orders/${encodeURIComponent(orderId)}`

const customerSiteRevisionsRequestPath = (orderId: string): string =>
  `/api/customer-orders/${encodeURIComponent(orderId)}/site-revisions`

const customerFulfillmentArtifactsRequestPath = (orderId: string): string =>
  `/api/customer-orders/${encodeURIComponent(orderId)}/fulfillment-artifacts`

const customerSiteFeedbackRequestPath = (orderId: string): string =>
  `/api/customer-orders/${encodeURIComponent(orderId)}/site-feedback`

const siteBuilderSessionRequestPath = (sessionId: string): string =>
  `/api/sites/builder-sessions/${encodeURIComponent(sessionId)}`

const siteBuilderFilesRequestPath = (sessionId: string): string =>
  `/api/sites/builder-sessions/${encodeURIComponent(sessionId)}/files`

const siteBuilderFileTreeRequestPath = (sessionId: string): string =>
  `/api/sites/builder-sessions/${encodeURIComponent(sessionId)}/files/tree`

const siteBuilderFileReadRequestPath = (
  sessionId: string,
  path: string,
): string =>
  `/api/sites/builder-sessions/${encodeURIComponent(sessionId)}/files/read?path=${encodeURIComponent(path)}`

const siteBuilderEventsRequestPath = (
  sessionId: string,
  cursor: number | undefined,
): string => {
  const base = `/api/sites/builder-sessions/${encodeURIComponent(sessionId)}/events`

  return cursor === undefined ? base : `${base}?cursor=${cursor}`
}

const customerBuilderIdempotencyKey = (orderId: string): string =>
  `customer-site-builder:${orderId}`

const parseSiteBuilderEventStream = (
  sessionId: string,
  text: string,
): CustomerSiteBuilderEventsResponse => {
  const events = text
    .split('\n')
    .filter(line => line.startsWith('data: '))
    .map(line => parseJsonRecord(line.slice('data: '.length)) ?? {})
    .map(payload => {
      const eventPayload = S.decodeUnknownSync(
        S.Struct({ event: CustomerSiteBuilderEvent }),
      )(payload)

      return eventPayload.event
    })

  return { events, siteBuilderSessionId: sessionId }
}

export const LoadCustomerOrder = Command.define(
  'LoadCustomerOrder',
  {
    orderId: S.NullOr(S.String),
  },
  SucceededLoadCustomerOrder,
  FailedLoadCustomerOrder,
)(({ orderId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.active.load',
      request: customerOrderRequestPath(orderId),
      schema: CustomerOrderResponse,
    })

    return SucceededLoadCustomerOrder({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerOrder({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerOrders = Command.define(
  'LoadCustomerOrders',
  {},
  SucceededLoadCustomerOrders,
  FailedLoadCustomerOrders,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrders.list.load',
      request: '/api/customer-orders',
      schema: CustomerOrdersResponse,
    })

    return SucceededLoadCustomerOrders({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerOrders({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const SubmitCustomerOrder = Command.define(
  'SubmitCustomerOrder',
  {
    request: S.String,
  },
  SucceededSubmitCustomerOrder,
  FailedSubmitCustomerOrder,
)(({ request }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ request }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.customerOrder.create',
      request: '/api/customer-orders',
      schema: CustomerOrderResponse,
    })

    return SucceededSubmitCustomerOrder({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSubmitCustomerOrder({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerSiteRevisions = Command.define(
  'LoadCustomerSiteRevisions',
  {
    orderId: S.String,
  },
  SucceededLoadCustomerSiteRevisions,
  FailedLoadCustomerSiteRevisions,
)(({ orderId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.siteRevisions.load',
      request: customerSiteRevisionsRequestPath(orderId),
      schema: CustomerSiteRevisionsResponse,
    })

    return SucceededLoadCustomerSiteRevisions({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerSiteRevisions({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerFulfillmentArtifacts = Command.define(
  'LoadCustomerFulfillmentArtifacts',
  {
    orderId: S.String,
  },
  SucceededLoadCustomerFulfillmentArtifacts,
  FailedLoadCustomerFulfillmentArtifacts,
)(({ orderId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.fulfillmentArtifacts.load',
      request: customerFulfillmentArtifactsRequestPath(orderId),
      schema: CustomerFulfillmentArtifactsResponse,
    })

    return SucceededLoadCustomerFulfillmentArtifacts({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerFulfillmentArtifacts({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerSiteFeedback = Command.define(
  'LoadCustomerSiteFeedback',
  {
    orderId: S.String,
  },
  SucceededLoadCustomerSiteFeedback,
  FailedLoadCustomerSiteFeedback,
)(({ orderId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.siteFeedback.load',
      request: customerSiteFeedbackRequestPath(orderId),
      schema: CustomerSiteFeedbackResponse,
    })

    return SucceededLoadCustomerSiteFeedback({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerSiteFeedback({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const OpenCustomerSiteBuilderSession = Command.define(
  'OpenCustomerSiteBuilderSession',
  {
    orderId: S.String,
    promptSummary: S.String,
    siteId: S.String,
  },
  SucceededOpenCustomerSiteBuilderSession,
  FailedOpenCustomerSiteBuilderSession,
)(({ orderId, promptSummary, siteId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ orderId, promptSummary, siteId }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
          'idempotency-key': customerBuilderIdempotencyKey(orderId),
        },
        method: 'POST',
      },
      name: 'loggedIn.customerOrder.siteBuilder.open',
      request: '/api/sites/builder-sessions',
      schema: CustomerSiteBuilderSessionResponse,
    })

    return SucceededOpenCustomerSiteBuilderSession({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedOpenCustomerSiteBuilderSession({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerSiteBuilderSession = Command.define(
  'LoadCustomerSiteBuilderSession',
  {
    sessionId: S.String,
  },
  SucceededLoadCustomerSiteBuilderSession,
  FailedLoadCustomerSiteBuilderSession,
)(({ sessionId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.siteBuilder.session.load',
      request: siteBuilderSessionRequestPath(sessionId),
      schema: CustomerSiteBuilderSessionResponse,
    })

    return SucceededLoadCustomerSiteBuilderSession({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerSiteBuilderSession({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerSiteBuilderFiles = Command.define(
  'LoadCustomerSiteBuilderFiles',
  {
    sessionId: S.String,
  },
  SucceededLoadCustomerSiteBuilderFiles,
  FailedLoadCustomerSiteBuilderFiles,
)(({ sessionId }) =>
  Effect.gen(function* () {
    const filesResponse = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.siteBuilder.files.load',
      request: siteBuilderFilesRequestPath(sessionId),
      schema: CustomerSiteBuilderFileListResponse,
    })
    const treeResponse = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.siteBuilder.fileTree.load',
      request: siteBuilderFileTreeRequestPath(sessionId),
      schema: CustomerSiteBuilderFileTreeResponse,
    })

    return SucceededLoadCustomerSiteBuilderFiles({
      filesResponse,
      treeResponse,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerSiteBuilderFiles({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadCustomerSiteBuilderFile = Command.define(
  'LoadCustomerSiteBuilderFile',
  {
    path: S.String,
    sessionId: S.String,
  },
  SucceededLoadCustomerSiteBuilderFile,
  FailedLoadCustomerSiteBuilderFile,
)(({ path, sessionId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.customerOrder.siteBuilder.file.load',
      request: siteBuilderFileReadRequestPath(sessionId, path),
      schema: CustomerSiteBuilderFileReadResponse,
    })

    return SucceededLoadCustomerSiteBuilderFile({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerSiteBuilderFile({
          error: errorMessageFromUnknown(error),
          path,
        }),
      ),
    ),
  ),
)

export const LoadCustomerSiteBuilderEvents = Command.define(
  'LoadCustomerSiteBuilderEvents',
  {
    cursor: S.optionalKey(S.Number),
    sessionId: S.String,
  },
  SucceededLoadCustomerSiteBuilderEvents,
  FailedLoadCustomerSiteBuilderEvents,
)(({ cursor, sessionId }) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(siteBuilderEventsRequestPath(sessionId, cursor), {
          cache: 'no-store',
          credentials: 'include',
          headers: { accept: 'text/event-stream' },
        }),
      catch: errorFromUnknown,
    })

    if (!response.ok) {
      return yield* new ChatApiHttpError({
        payload: { error: `OpenAgents API returned HTTP ${response.status}.` },
        status: response.status,
      })
    }

    const text = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: errorFromUnknown,
    })

    return SucceededLoadCustomerSiteBuilderEvents({
      response: parseSiteBuilderEventStream(sessionId, text),
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadCustomerSiteBuilderEvents({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const SubmitCustomerSiteFeedback = Command.define(
  'SubmitCustomerSiteFeedback',
  {
    body: S.String,
    orderId: S.String,
  },
  SucceededSubmitCustomerSiteFeedback,
  FailedSubmitCustomerSiteFeedback,
)(({ body, orderId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ body }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.customerOrder.siteFeedback.submit',
      request: customerSiteFeedbackRequestPath(orderId),
      schema: SubmitCustomerSiteFeedbackResponse,
    })

    return SucceededSubmitCustomerSiteFeedback({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedSubmitCustomerSiteFeedback({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const feedbackWithSubmitted = (model: Model, feedback: CustomerSiteFeedback) =>
  M.value(model.customerSiteFeedback).pipe(
    M.tags({
      CustomerSiteFeedbackLoaded: ({ feedback: existing }) =>
        CustomerSiteFeedbackLoaded({ feedback: [feedback, ...existing] }),
    }),
    M.orElse(() => CustomerSiteFeedbackLoaded({ feedback: [feedback] })),
  )

const submitStateAfterDraft = (
  draft: string,
): CustomerSiteFeedbackSubmitState =>
  draft.trim() === ''
    ? CustomerSiteFeedbackSubmitFailed({
        error: 'Enter an adjustment request.',
      })
    : CustomerSiteFeedbackSubmitting()

const ordersWithSubmitted = (model: Model, order: CustomerOrder) =>
  M.value(model.customerOrders).pipe(
    M.tags({
      CustomerOrdersLoaded: ({ orders }) =>
        CustomerOrdersLoaded({
          orders: [order, ...orders.filter(existing => existing.id !== order.id)],
        }),
    }),
    M.orElse(() => CustomerOrdersLoaded({ orders: [order] })),
  )

export const updateCustomerOrder = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadCustomerOrder: () => [
        evo(model, {
          customerOrder: () => CustomerOrderLoading(),
          customerFulfillmentArtifacts: () =>
            CustomerFulfillmentArtifactsIdle(),
          customerSiteBuilderEvents: () => CustomerSiteBuilderEventsIdle(),
          customerSiteBuilderFileRead: () => CustomerSiteBuilderFileReadIdle(),
          customerSiteBuilderFiles: () => CustomerSiteBuilderFilesIdle(),
          customerSiteBuilderPromptDraft: () => '',
          customerSiteBuilderSelectedFilePath: () => null,
          customerSiteBuilderSession: () => CustomerSiteBuilderSessionIdle(),
          customerSiteFeedback: () => CustomerSiteFeedbackIdle(),
          customerSiteFeedbackSubmit: () => CustomerSiteFeedbackSubmitIdle(),
          customerSiteRevisions: () => CustomerSiteRevisionsIdle(),
        }),
        [LoadCustomerOrder({ orderId: customerOrderIdForRoute(model) })],
        Option.none(),
      ],
      SucceededLoadCustomerOrder: ({ response }) => [
        evo(model, {
          customerOrder: () => CustomerOrderLoaded({ order: response.order }),
          customerFulfillmentArtifacts: () =>
            response.order === null
              ? CustomerFulfillmentArtifactsIdle()
              : CustomerFulfillmentArtifactsLoading(),
          customerSiteBuilderEvents: () => CustomerSiteBuilderEventsIdle(),
          customerSiteBuilderFileRead: () => CustomerSiteBuilderFileReadIdle(),
          customerSiteBuilderFiles: () => CustomerSiteBuilderFilesIdle(),
          customerSiteBuilderPromptDraft: () => '',
          customerSiteBuilderSelectedFilePath: () => null,
          customerSiteBuilderSession: () => CustomerSiteBuilderSessionIdle(),
          customerSiteFeedback: () =>
            response.order === null
              ? CustomerSiteFeedbackIdle()
              : CustomerSiteFeedbackLoading(),
          customerSiteRevisions: () =>
            response.order === null
              ? CustomerSiteRevisionsIdle()
              : CustomerSiteRevisionsLoading(),
        }),
        response.order === null
          ? []
          : [
              LoadCustomerFulfillmentArtifacts({ orderId: response.order.id }),
              LoadCustomerSiteRevisions({ orderId: response.order.id }),
              LoadCustomerSiteFeedback({ orderId: response.order.id }),
            ],
        Option.none(),
      ],
      FailedLoadCustomerOrder: ({ error }) => [
        evo(model, { customerOrder: () => CustomerOrderFailed({ error }) }),
        [],
        Option.none(),
      ],
      RequestedLoadCustomerOrders: () => [
        evo(model, { customerOrders: () => CustomerOrdersLoading() }),
        [LoadCustomerOrders({})],
        Option.none(),
      ],
      SucceededLoadCustomerOrders: ({ response }) => [
        evo(model, {
          customerOrders: () =>
            CustomerOrdersLoaded({ orders: response.orders }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerOrders: ({ error }) => [
        evo(model, { customerOrders: () => CustomerOrdersFailed({ error }) }),
        [],
        Option.none(),
      ],
      UpdatedCustomerOrderDraft: ({ value }) => [
        evo(model, { customerOrderDraft: () => value }),
        [],
        Option.none(),
      ],
      SubmittedCustomerOrder: () => {
        const request = model.customerOrderDraft.trim()

        if (request === '') {
          return [
            evo(model, {
              customerOrderCreate: () =>
                CustomerOrderCreateFailed({
                  error: 'Enter the software request.',
                }),
            }),
            [],
            Option.none(),
          ]
        }

        return [
          evo(model, {
            customerOrderCreate: () => CustomerOrderCreateSubmitting(),
          }),
          [SubmitCustomerOrder({ request })],
          Option.none(),
        ]
      },
      SucceededSubmitCustomerOrder: ({ response }) => {
        const order = response.order

        return order === null
          ? [
              evo(model, {
                customerOrderCreate: () =>
                  CustomerOrderCreateFailed({
                    error: 'The request was created but could not be loaded.',
                  }),
              }),
              [],
              Option.none(),
            ]
          : [
              evo(model, {
                customerOrderCreate: () =>
                  CustomerOrderCreateSucceeded({ order }),
                customerOrderDraft: () => '',
                customerOrders: () => ordersWithSubmitted(model, order),
              }),
              [],
              Option.none(),
            ]
      },
      FailedSubmitCustomerOrder: ({ error }) => [
        evo(model, {
          customerOrderCreate: () => CustomerOrderCreateFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SucceededLoadCustomerFulfillmentArtifacts: ({ response }) => [
        evo(model, {
          customerFulfillmentArtifacts: () =>
            CustomerFulfillmentArtifactsLoaded({
              artifacts: response.artifacts,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerFulfillmentArtifacts: ({ error }) => [
        evo(model, {
          customerFulfillmentArtifacts: () =>
            CustomerFulfillmentArtifactsFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SucceededLoadCustomerSiteRevisions: ({ response }) => [
        evo(model, {
          customerSiteRevisions: () =>
            CustomerSiteRevisionsLoaded({ revisions: response.revisions }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerSiteRevisions: ({ error }) => [
        evo(model, {
          customerSiteRevisions: () => CustomerSiteRevisionsFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SucceededLoadCustomerSiteFeedback: ({ response }) => [
        evo(model, {
          customerSiteFeedback: () =>
            CustomerSiteFeedbackLoaded({ feedback: response.feedback }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerSiteFeedback: ({ error }) => [
        evo(model, {
          customerSiteFeedback: () => CustomerSiteFeedbackFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      UpdatedCustomerSiteFeedbackDraft: ({ value }) => [
        evo(model, { customerSiteFeedbackDraft: () => value }),
        [],
        Option.none(),
      ],
      UpdatedCustomerSiteBuilderPromptDraft: ({ value }) => [
        evo(model, { customerSiteBuilderPromptDraft: () => value }),
        [],
        Option.none(),
      ],
      SelectedCustomerSiteElementContext: ({ context }) => [
        evo(model, {
          customerSiteElementContext: () => context,
          customerSiteFeedbackDraft: () => siteElementContextDraft(context),
        }),
        [],
        Option.none(),
      ],
      SubmittedCustomerSiteFeedback: ({ orderId }) => {
        const body = model.customerSiteFeedbackDraft.trim()
        const submitState = submitStateAfterDraft(body)

        return [
          evo(model, { customerSiteFeedbackSubmit: () => submitState }),
          submitState._tag === 'CustomerSiteFeedbackSubmitting'
            ? [SubmitCustomerSiteFeedback({ body, orderId })]
            : [],
          Option.none(),
        ]
      },
      SucceededSubmitCustomerSiteFeedback: ({ response }) => [
        evo(model, {
          customerSiteFeedback: () =>
            feedbackWithSubmitted(model, response.feedback),
          customerSiteFeedbackDraft: () => '',
          customerSiteFeedbackSubmit: () =>
            CustomerSiteFeedbackSubmitSucceeded({
              feedback: response.feedback,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedSubmitCustomerSiteFeedback: ({ error }) => [
        evo(model, {
          customerSiteFeedbackSubmit: () =>
            CustomerSiteFeedbackSubmitFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedOpenCustomerSiteBuilderSession: ({
        orderId,
        promptSummary,
        siteId,
      }) => [
        evo(model, {
          customerSiteBuilderEvents: () => CustomerSiteBuilderEventsIdle(),
          customerSiteBuilderFileRead: () => CustomerSiteBuilderFileReadIdle(),
          customerSiteBuilderFiles: () => CustomerSiteBuilderFilesIdle(),
          customerSiteBuilderSelectedFilePath: () => null,
          customerSiteBuilderSession: () => CustomerSiteBuilderSessionLoading(),
        }),
        [
          OpenCustomerSiteBuilderSession({ orderId, promptSummary, siteId }),
        ],
        Option.none(),
      ],
      SucceededOpenCustomerSiteBuilderSession: ({ response }) => [
        evo(model, {
          customerSiteBuilderEvents: () => CustomerSiteBuilderEventsLoading(),
          customerSiteBuilderFiles: () => CustomerSiteBuilderFilesLoading(),
          customerSiteBuilderSession: () =>
            CustomerSiteBuilderSessionLoaded({
              session: response.siteBuilderSession,
            }),
        }),
        [
          LoadCustomerSiteBuilderFiles({
            sessionId: response.siteBuilderSession.id,
          }),
          LoadCustomerSiteBuilderEvents({
            sessionId: response.siteBuilderSession.id,
          }),
        ],
        Option.none(),
      ],
      FailedOpenCustomerSiteBuilderSession: ({ error }) => [
        evo(model, {
          customerSiteBuilderSession: () =>
            CustomerSiteBuilderSessionFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadCustomerSiteBuilderSession: ({ sessionId }) => [
        evo(model, {
          customerSiteBuilderSession: () => CustomerSiteBuilderSessionLoading(),
        }),
        [LoadCustomerSiteBuilderSession({ sessionId })],
        Option.none(),
      ],
      SucceededLoadCustomerSiteBuilderSession: ({ response }) => [
        evo(model, {
          customerSiteBuilderSession: () =>
            CustomerSiteBuilderSessionLoaded({
              session: response.siteBuilderSession,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerSiteBuilderSession: ({ error }) => [
        evo(model, {
          customerSiteBuilderSession: () =>
            CustomerSiteBuilderSessionFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadCustomerSiteBuilderFiles: ({ sessionId }) => [
        evo(model, {
          customerSiteBuilderFiles: () => CustomerSiteBuilderFilesLoading(),
        }),
        [LoadCustomerSiteBuilderFiles({ sessionId })],
        Option.none(),
      ],
      SucceededLoadCustomerSiteBuilderFiles: ({
        filesResponse,
        treeResponse,
      }) => {
        const selectedPath =
          filesResponse.files.find(file => file.hasPreview)?.path ??
          filesResponse.files[0]?.path ??
          null
        const sessionId = filesResponse.siteBuilderSessionId

        return [
          evo(model, {
            customerSiteBuilderFileRead: () =>
              selectedPath === null
                ? CustomerSiteBuilderFileReadIdle()
                : CustomerSiteBuilderFileReadLoading({ path: selectedPath }),
            customerSiteBuilderFiles: () =>
              CustomerSiteBuilderFilesLoaded({
                files: filesResponse.files,
                fileTree: treeResponse.fileTree,
              }),
            customerSiteBuilderSelectedFilePath: () => selectedPath,
          }),
          selectedPath === null
            ? []
            : [
                LoadCustomerSiteBuilderFile({
                  path: selectedPath,
                  sessionId,
                }),
              ],
          Option.none(),
        ]
      },
      FailedLoadCustomerSiteBuilderFiles: ({ error }) => [
        evo(model, {
          customerSiteBuilderFiles: () =>
            CustomerSiteBuilderFilesFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      SelectedCustomerSiteBuilderFile: ({ path, sessionId }) => [
        evo(model, {
          customerSiteBuilderFileRead: () =>
            CustomerSiteBuilderFileReadLoading({ path }),
          customerSiteBuilderSelectedFilePath: () => path,
        }),
        [LoadCustomerSiteBuilderFile({ path, sessionId })],
        Option.none(),
      ],
      SucceededLoadCustomerSiteBuilderFile: ({ response }) => [
        evo(model, {
          customerSiteBuilderFileRead: () =>
            CustomerSiteBuilderFileReadLoaded({ file: response.file }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerSiteBuilderFile: ({ error, path }) => [
        evo(model, {
          customerSiteBuilderFileRead: () =>
            CustomerSiteBuilderFileReadFailed({ error, path }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadCustomerSiteBuilderEvents: ({ cursor, sessionId }) => [
        evo(model, {
          customerSiteBuilderEvents: () => CustomerSiteBuilderEventsLoading(),
        }),
        [
          LoadCustomerSiteBuilderEvents({
            ...(cursor === undefined ? {} : { cursor }),
            sessionId,
          }),
        ],
        Option.none(),
      ],
      SucceededLoadCustomerSiteBuilderEvents: ({ response }) => [
        evo(model, {
          customerSiteBuilderEvents: () =>
            CustomerSiteBuilderEventsLoaded({ events: response.events }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadCustomerSiteBuilderEvents: ({ error }) => [
        evo(model, {
          customerSiteBuilderEvents: () =>
            CustomerSiteBuilderEventsFailed({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => [model, [], Option.none()]),
  )
