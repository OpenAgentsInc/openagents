import { Effect, Schema as S } from 'effect'

import {
  decodeCustomerOneCohortPrivateRow,
  projectCustomerOneCohort,
} from './customer-one-cohort-projection'
import type {
  CustomerOneCohortRowStore,
  CustomerOneCohortSourceStore,
} from './customer-one-cohort-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

export type CustomerOneCohortRouteInput = Readonly<{
  nowIso?: () => string
  store?: CustomerOneCohortSourceStore
}>

export type CustomerOneCohortOperatorRouteInput = Readonly<{
  nowIso?: () => string
  requireAdminApiToken: (request: Request) => Promise<boolean>
  store: CustomerOneCohortRowStore
}>

const emptyCustomerOneCohortSourceStore: CustomerOneCohortSourceStore = {
  listRows: () => Effect.succeed([]),
}

class CustomerOneCohortRouteDecodeError extends S.TaggedErrorClass<CustomerOneCohortRouteDecodeError>()(
  'CustomerOneCohortRouteDecodeError',
  {
    reason: S.String,
  },
) {}

const decodeError = (error: unknown): CustomerOneCohortRouteDecodeError =>
  new CustomerOneCohortRouteDecodeError({
    reason: error instanceof Error ? error.message : String(error),
  })

const badRequest = (reason: string) =>
  noStoreJsonResponse(
    {
      error: 'customer_one_cohort_bad_request',
      reason,
    },
    { status: 400 },
  )

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const decodeRowRequest = (
  request: Request,
): Effect.Effect<
  | Readonly<{
      row: ReturnType<typeof decodeCustomerOneCohortPrivateRow>
      tag: 'ok'
    }>
  | Readonly<{ reason: string; tag: 'error' }>
> =>
  Effect.tryPromise({
    catch: decodeError,
    try: () => request.json(),
  }).pipe(
    Effect.flatMap(raw =>
      Effect.try({
        catch: decodeError,
        try: () => decodeCustomerOneCohortPrivateRow(raw),
      }),
    ),
    Effect.map(row => ({ row, tag: 'ok' as const })),
    Effect.catch(error =>
      Effect.succeed({ reason: error.reason, tag: 'error' as const }),
    ),
  )

export const handlePublicCustomerOneCohortApi = (
  request: Request,
  input: CustomerOneCohortRouteInput = {},
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : (input.store ?? emptyCustomerOneCohortSourceStore).listRows().pipe(
        Effect.map(rows =>
          noStoreJsonResponse(
            projectCustomerOneCohort({
              generatedAt: input.nowIso?.() ?? currentIsoTimestamp(),
              rows,
            }),
          ),
        ),
      )

const handleOperatorListRows = (input: CustomerOneCohortOperatorRouteInput) =>
  input.store.listRows().pipe(
    Effect.map(rows =>
      noStoreJsonResponse({
        generatedAt: input.nowIso?.() ?? currentIsoTimestamp(),
        kind: 'customer_one_cohort_private_rows',
        rows,
      }),
    ),
  )

const handleOperatorUpsertRow = (
  request: Request,
  input: CustomerOneCohortOperatorRouteInput,
) =>
  decodeRowRequest(request).pipe(
    Effect.flatMap(result =>
      result.tag === 'error'
        ? Effect.succeed(badRequest(result.reason))
        : input.store.upsertRow(result.row).pipe(
            Effect.map(() =>
              noStoreJsonResponse(
                {
                  kind: 'customer_one_cohort_private_row',
                  row: result.row,
                },
                { status: 201 },
              ),
            ),
          ),
    ),
  )

export const handleOperatorCustomerOneCohortRowsApi = (
  request: Request,
  input: CustomerOneCohortOperatorRouteInput,
) =>
  request.method !== 'GET' && request.method !== 'POST'
    ? Effect.succeed(methodNotAllowed(['GET', 'POST']))
    : Effect.promise(() => input.requireAdminApiToken(request)).pipe(
        Effect.flatMap(authorized =>
          authorized
            ? request.method === 'GET'
              ? handleOperatorListRows(input)
              : handleOperatorUpsertRow(request, input)
            : Effect.succeed(unauthorized()),
        ),
      )
