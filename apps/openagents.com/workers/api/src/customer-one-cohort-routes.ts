import { Effect } from 'effect'

import {
  type CustomerOneCohortPrivateRow,
  projectCustomerOneCohort,
} from './customer-one-cohort-projection'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

export type CustomerOneCohortSourceStore = Readonly<{
  listRows: () => Effect.Effect<ReadonlyArray<CustomerOneCohortPrivateRow>>
}>

export type CustomerOneCohortRouteInput = Readonly<{
  nowIso?: () => string
  store?: CustomerOneCohortSourceStore
}>

const emptyCustomerOneCohortSourceStore: CustomerOneCohortSourceStore = {
  listRows: () => Effect.succeed([]),
}

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
