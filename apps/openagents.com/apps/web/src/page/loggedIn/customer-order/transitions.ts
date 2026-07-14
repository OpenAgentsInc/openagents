import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedLoadCustomerFulfillmentArtifacts,
  FailedLoadCustomerOrder,
  FailedLoadCustomerOrders,
  FailedSubmitCustomerOrder,
  Message,
  SucceededLoadCustomerFulfillmentArtifacts,
  SucceededLoadCustomerOrder,
  SucceededLoadCustomerOrders,
  SucceededSubmitCustomerOrder,
} from '../message'
import {
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
  Model,
} from '../model'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const customerOrderIdForRoute = (model: Model): string | null =>
  model.route._tag === 'OrderDetail' ? model.route.orderId : null

const customerOrderRequestPath = (orderId: string | null): string =>
  orderId === null
    ? '/api/customer-orders/active'
    : `/api/customer-orders/${encodeURIComponent(orderId)}`

const customerFulfillmentArtifactsRequestPath = (orderId: string): string =>
  `/api/customer-orders/${encodeURIComponent(orderId)}/fulfillment-artifacts`

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
        }),
        response.order === null
          ? []
          : [
              LoadCustomerFulfillmentArtifacts({ orderId: response.order.id }),
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
    }),
    M.orElse(() => [model, [], Option.none()]),
  )
